//! `zks`: a small driver binary for the storage crate.
//!
//! Each verb is one real library call; every refusal, classification, cap and sweep decision is made in
//! `zikaron_store`. This binary passes arguments in and prints results.
//!
//! `put`, `place` and `mkdir` are test scaffolding, not storage features: they write raw bytes under an entry
//! name, raw bytes under any plain name (e.g. an OS side file), or a directory straight into an archive
//! (bypassing append) to build foreign layouts, in-flight temporaries and oversized files.
//!
//! `--stop-at open|tmp|link` exits the process between append stages (exit code 70, nothing printed), so tests
//! can simulate a real process death before, during and after a write.

use std::process::ExitCode;
use zikaron_store::codes::Trouble;
use zikaron_store::layout::{self, EntryName};
use zikaron_store::ledger::{Layout, LedgerDir, Pile, Skip, Survey, Sweep};

/// Exit code when stopping between stages; 0 and 2 belong to verdicts and misuse.
const STOP_CODE: u8 = 70;

fn misuse(m: &str) -> ! {
    eprintln!("zks: {m}");
    std::process::exit(2)
}

// Printing: one-line JSON, keys in byte order.

fn quote(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(2 + bytes.len() * 2);
    s.push_str("0x");
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap_or('0'));
        s.push(char::from_digit((b & 0xf) as u32, 16).unwrap_or('0'));
    }
    s
}

fn strings(v: &[String], out: &mut String) {
    out.push('[');
    for (i, s) in v.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        quote(s, out);
    }
    out.push(']');
}

fn say(line: String) -> ExitCode {
    println!("{line}");
    ExitCode::SUCCESS
}

/// Print a refusal; the details (names, size, cap) always accompany the code.
fn refused(t: &Trouble) -> ExitCode {
    let mut s = String::from("{");
    if let Some(c) = t.cap {
        s.push_str(&format!("\"cap\":{c},"));
    }
    if !t.names.is_empty() {
        s.push_str("\"names\":");
        strings(&t.names, &mut s);
        s.push(',');
    }
    s.push_str("\"ok\":false,\"reason\":");
    quote(t.code.as_str(), &mut s);
    if let Some(n) = t.size {
        s.push_str(&format!(",\"size\":{n}"));
    }
    s.push('}');
    say(s)
}

fn piled(p: &Pile) -> ExitCode {
    let mut s = String::from("{\"ok\":true,\"pile\":[");
    for (i, b) in p.items.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        quote(&hex(b), &mut s);
    }
    s.push_str("]}");
    say(s)
}

fn skips(v: &[Skip], out: &mut String) {
    out.push('[');
    for (i, k) in v.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str("{\"name\":");
        quote(&k.name, out);
        out.push_str(",\"why\":");
        quote(k.why.as_str(), out);
        out.push('}');
    }
    out.push(']');
}

fn surveyed(s: &Survey) -> ExitCode {
    let mut o = String::from("{\"ok\":true,\"pile\":[");
    for (i, b) in s.items.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        quote(&hex(b), &mut o);
    }
    o.push_str("],\"skipped\":");
    skips(&s.skipped, &mut o);
    o.push('}');
    say(o)
}

fn swept(s: &Sweep) -> ExitCode {
    let mut o = String::from("{\"kept\":");
    strings(&s.kept, &mut o);
    o.push_str(&format!(",\"ok\":true,\"swept\":{}}}", s.swept));
    say(o)
}

fn laid_out(l: &Layout) -> ExitCode {
    say(format!(
        "{{\"dirs\":{},\"entries\":{},\"foreign\":{},\"ok\":true,\"tmp\":{}}}",
        l.dirs, l.entries, l.foreign, l.tmp
    ))
}

// Arguments.

struct Args {
    words: Vec<String>,
    zeros: Option<usize>,
    stop_at: Option<String>,
}

fn parse(argv: Vec<String>) -> Args {
    let mut a = Args { words: Vec::new(), zeros: None, stop_at: None };
    let mut i = 0;
    while i < argv.len() {
        match argv[i].as_str() {
            "--zeros" => {
                i += 1;
                let v = argv.get(i).unwrap_or_else(|| misuse("--zeros 后面缺一个数"));
                a.zeros = Some(v.parse().unwrap_or_else(|_| misuse("--zeros 要一个十进制整数")));
            }
            "--stop-at" => {
                i += 1;
                let v = argv.get(i).cloned().unwrap_or_else(|| misuse("--stop-at 后面缺一段"));
                if !matches!(v.as_str(), "open" | "tmp" | "link") {
                    misuse("--stop-at 只认 open / tmp / link");
                }
                a.stop_at = Some(v);
            }
            w if w.starts_with("--") => misuse("不认得的开关"),
            w => a.words.push(w.to_string()),
        }
        i += 1;
    }
    a
}

fn word(a: &Args, i: usize, usage: &str) -> String {
    a.words.get(i).cloned().unwrap_or_else(|| misuse(usage))
}

fn entry_name(s: &str) -> EntryName {
    EntryName::parse(s).unwrap_or_else(|| misuse("条目名不是六十四位小写十六进制"))
}

fn open(dir: &str) -> Result<LedgerDir, Trouble> {
    LedgerDir::open(dir)
}

/// Bytes to append: a file, or `--zeros N` zero bytes (so the oversized cases need no 16 MiB sample in the
/// tree).
fn bytes_for(a: &Args, idx: usize) -> Vec<u8> {
    match a.zeros {
        Some(n) => vec![0u8; n],
        None => {
            let p = word(a, idx, "usage: zks append <dir> <name> (<path> | --zeros N)");
            std::fs::read(&p).unwrap_or_else(|e| misuse(&format!("读不出 {p}:{e}")))
        }
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let verb = argv.first().cloned().unwrap_or_default();
    let a = parse(argv.into_iter().skip(1).collect());
    match verb.as_str() {
        // Storage features.
        "append" => {
            let dir = word(&a, 0, "usage: zks append <dir> <name> (<path> | --zeros N)");
            let name = entry_name(&word(&a, 1, "usage: zks append <dir> <name> ..."));
            let bytes = bytes_for(&a, 2);
            let led = match LedgerDir::open_or_create(&dir) {
                Ok(l) => l,
                Err(t) => return refused(&t),
            };
            if a.stop_at.as_deref() == Some("open") {
                std::process::exit(STOP_CODE as i32);
            }
            let staged = match led.stage(&name, &bytes) {
                Ok(s) => s,
                Err(t) => return refused(&t),
            };
            if a.stop_at.as_deref() == Some("tmp") {
                std::process::exit(STOP_CODE as i32);
            }
            let linked = match staged.link() {
                Ok(l) => l,
                Err(t) => return refused(&t),
            };
            if a.stop_at.as_deref() == Some("link") {
                std::process::exit(STOP_CODE as i32);
            }
            match linked.seal() {
                Ok(_) => say(String::from("{\"ok\":true,\"stored\":true}")),
                Err(t) => refused(&t),
            }
        }
        "pile" => {
            let dir = word(&a, 0, "usage: zks pile <dir>");
            match open(&dir).and_then(|l| l.pile()) {
                Ok(p) => piled(&p),
                Err(t) => refused(&t),
            }
        }
        "survey" => {
            let dir = word(&a, 0, "usage: zks survey <dir>");
            match open(&dir).and_then(|l| l.survey()) {
                Ok(s) => surveyed(&s),
                Err(t) => refused(&t),
            }
        }
        "read" => {
            let dir = word(&a, 0, "usage: zks read <dir> <file>");
            let file = word(&a, 1, "usage: zks read <dir> <file>");
            match open(&dir).and_then(|l| l.read_named(&file)) {
                Ok(b) => {
                    let mut s = String::from("{\"bytes\":");
                    quote(&hex(&b), &mut s);
                    s.push_str(",\"ok\":true}");
                    say(s)
                }
                Err(t) => refused(&t),
            }
        }
        "sweep" => {
            let dir = word(&a, 0, "usage: zks sweep <dir>");
            match open(&dir).and_then(|l| l.sweep()) {
                Ok(s) => swept(&s),
                Err(t) => refused(&t),
            }
        }
        "layout" => {
            let dir = word(&a, 0, "usage: zks layout <dir>");
            match open(&dir).and_then(|l| l.layout()) {
                Ok(l) => laid_out(&l),
                Err(t) => refused(&t),
            }
        }
        // Scaffolding.
        "put" => {
            let dir = word(&a, 0, "usage: zks put <dir> <name> --zeros N");
            let name = entry_name(&word(&a, 1, "usage: zks put <dir> <name> --zeros N"));
            let bytes = bytes_for(&a, 2);
            std::fs::create_dir_all(&dir).unwrap_or_else(|e| misuse(&format!("建不出 {dir}:{e}")));
            let file = layout::entry_file_name(&name);
            let path = std::path::Path::new(&dir).join(&file);
            std::fs::write(&path, &bytes).unwrap_or_else(|e| misuse(&format!("写不下 {file}:{e}")));
            say(String::from("{\"ok\":true,\"put\":true}"))
        }
        "place" => {
            let dir = word(&a, 0, "usage: zks place <dir> <file> (<path> | --zeros N)");
            let file = word(&a, 1, "usage: zks place <dir> <file> (<path> | --zeros N)");
            if file.is_empty() || file.contains('/') || file.contains('\\') || file == "." || file == ".." {
                misuse("名要一段,不许有路径分隔");
            }
            let bytes = bytes_for(&a, 2);
            std::fs::create_dir_all(&dir).unwrap_or_else(|e| misuse(&format!("建不出 {dir}:{e}")));
            let path = std::path::Path::new(&dir).join(&file);
            std::fs::write(&path, &bytes).unwrap_or_else(|e| misuse(&format!("写不下 {file}:{e}")));
            say(String::from("{\"ok\":true,\"placed\":true}"))
        }
        "mkdir" => {
            let dir = word(&a, 0, "usage: zks mkdir <dir> <name>");
            let name = word(&a, 1, "usage: zks mkdir <dir> <name>");
            if name.contains('/') || name.contains('\\') {
                misuse("名里不许有路径分隔");
            }
            let path = std::path::Path::new(&dir).join(&name);
            std::fs::create_dir_all(&path).unwrap_or_else(|e| misuse(&format!("建不出 {name}:{e}")));
            say(String::from("{\"made\":true,\"ok\":true}"))
        }
        _ => misuse(
            "usage: zks <append|pile|survey|read|sweep|layout|put|place|mkdir> ... \
             [--zeros N] [--stop-at open|tmp|link]",
        ),
    }
}
