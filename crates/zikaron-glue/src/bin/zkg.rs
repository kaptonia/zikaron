//! `zkg`: a small driver binary for disclosure kit output.
//!
//! No decision is made here: entry selection lives in `select`, laying out and self-verification in `pack`.
//! Two verbs: `pack` (write a kit) and `verify` (ask the kit core whether a kit is valid).

use std::process::ExitCode;
use zikaron::json::{canon_bytes, Value};
use zikaron_glue::names::Key;
use zikaron_glue::pack::{self, Bundle};
use zikaron_glue::select::{self, Selection};

fn misuse(m: &str) -> ! {
    eprintln!("E_ARGS {m}");
    std::process::exit(2)
}

fn unreadable(p: &str) -> ! {
    eprintln!("E_UNREADABLE {p}");
    std::process::exit(2)
}

/// stdout carries one canonical JSON value with no trailing newline, as on the command surface.
fn say(v: &Value, code: u8) -> ExitCode {
    use std::io::Write;
    let mut out = std::io::stdout();
    let _ = out.write_all(&canon_bytes(v));
    let _ = out.flush();
    ExitCode::from(code)
}

fn refused(reason: &str, detail: &str, code: u8) -> ExitCode {
    say(
        &Value::Obj(vec![
            (Key::Detail.as_str().into(), Value::Str(detail.into())),
            (Key::Ok.as_str().into(), Value::Bool(false)),
            (Key::Reason.as_str().into(), Value::Str(reason.into())),
        ]),
        code,
    )
}

struct Flags {
    pairs: Vec<(String, String)>,
}

impl Flags {
    fn parse(rest: &[String]) -> Flags {
        let mut pairs = Vec::new();
        let mut i = 0;
        while i < rest.len() {
            let Some(name) = rest[i].strip_prefix("--") else {
                misuse(&rest[i])
            };
            let Some(v) = rest.get(i + 1).filter(|x| !x.starts_with("--")) else {
                misuse(&format!("--{name} 要一个值"))
            };
            pairs.push((name.to_string(), v.clone()));
            i += 2;
        }
        Flags { pairs }
    }
    /// A flag given twice is misuse; silently taking the first would pack A when the person meant B.
    fn one(&self, k: &str) -> Option<String> {
        let mut hit = self.pairs.iter().filter(|(n, _)| n == k);
        let first = hit.next()?;
        if hit.next().is_some() {
            misuse(&format!("--{k} 给了不止一次"));
        }
        Some(first.1.clone())
    }
    fn need(&self, k: &str) -> String {
        self.one(k).unwrap_or_else(|| misuse(&format!("缺 --{k}")))
    }
    /// Each verb's flag list is closed: a flag outside it is misuse, so a removed flag fails loudly in old
    /// scripts.
    fn close(&self, allowed: &[&str]) {
        for (k, _) in &self.pairs {
            if !allowed.contains(&k.as_str()) {
                misuse(&format!("--{k} 不在这个动词的旗单里"));
            }
        }
    }

    fn many(&self, k: &str) -> Vec<String> {
        self.pairs.iter().filter(|(n, _)| n == k).map(|(_, v)| v.clone()).collect()
    }
    fn u64_of(&self, k: &str) -> Option<u64> {
        self.one(k).map(|x| x.parse().unwrap_or_else(|_| misuse(&format!("--{k} 不是整数"))))
    }
}

fn slurp(p: &str) -> Vec<u8> {
    std::fs::read(p).unwrap_or_else(|_| unreadable(p))
}

/// `<kit path>=<disk path>` (kit paths contain no `=`, so the split is at the first one).
fn pair_at<'a>(spec: &'a str, shape: &str) -> (&'a str, &'a str) {
    spec.split_once('=').unwrap_or_else(|| misuse(shape))
}

/// Collect files and directories. A file that cannot be taken is the same failure as a malformed `--file`
/// path, with the same code (1), whether or not it came from walking a directory.
fn gather_into(b: &mut Bundle, f: &Flags) -> Result<(), (String, String)> {
    for spec in f.many("file") {
        let (kit, src) = pair_at(&spec, "--file 的形是 <包内路径>=<盘上的路>");
        b.files.push((kit.to_string(), slurp(src)));
        b.contents.push(kit.to_string());
    }
    for spec in f.many("dir") {
        let (kit, src) = pair_at(&spec, "--dir 的形是 <包内前缀>=<盘上的目录>");
        let mut got: Vec<(String, Vec<u8>)> = Vec::new();
        if let Err(t) = pack::gather(std::path::Path::new(src), kit, &mut got) {
            return Err((t.code().to_string(), t.subject()));
        }
        for (p, bytes) in got {
            b.contents.push(p.clone());
            b.files.push((p, bytes));
        }
    }
    for spec in f.many("proof") {
        let mut parts = spec.splitn(3, '=');
        let (Some(kit), Some(tx), Some(src)) = (parts.next(), parts.next(), parts.next()) else {
            misuse("--proof 的形是 <包内路径>=<tx>=<盘上的路>")
        };
        b.proofs.push((kit.to_string(), tx.to_string(), slurp(src)));
    }
    Ok(())
}

fn selection(f: &Flags) -> Selection {
    Selection {
        from: f.u64_of("from"),
        to: f.u64_of("to"),
        work: f.one("work"),
        ids: Vec::new(),
    }
}

fn pile_of(f: &Flags) -> Vec<Vec<u8>> {
    match f.one("ledger") {
        Some(root) => select::read_ledger(&root)
            .unwrap_or_else(|t| misuse(&format!("账本读不出:{}", t.code.as_str()))),
        None => f.many("entry").iter().map(|p| slurp(p)).collect(),
    }
}

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let verb = argv.first().cloned().unwrap_or_default();
    let f = Flags::parse(&argv.into_iter().skip(1).collect::<Vec<_>>());
    match verb.as_str() {
        // Write a kit.
        "pack" => {
            f.close(&["ledger", "entry", "out", "from", "to", "work", "file", "dir", "proof", "note", "root"]);
            let chosen = select::choose(&pile_of(&f), &selection(&f));
            let mut b = Bundle {
                entries: chosen.items,
                root: f.one("root"),
                note: f.one("note").unwrap_or_default(),
                ..Bundle::default()
            };
            if let Err((code, subj)) = gather_into(&mut b, &f) {
                return refused(&code, &subj, 1);
            }
            let out = f.need("out");
            let pulled = chosen.pulled;
            match pack::export(std::path::Path::new(&out), b) {
                Ok(l) => {
                    let mut v = pack::answer(&l, &out);
                    if let Value::Obj(ms) = &mut v {
                        ms.push((
                            String::from("pulled"),
                            Value::Arr(pulled.into_iter().map(Value::Str).collect()),
                        ));
                        ms.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
                    }
                    say(&v, 0)
                }
                Err(t) => {
                    refused(t.code(), &t.subject(), 1)
                }
            }
        }
        // Recipient side: ask the kit core whether the kit is valid (each tampered part has its own refusal).
        "verify" => {
            f.close(&["kit"]);
            let dir = f.need("kit");
            match zikaron_kit::kitdir::verify_kit(std::path::Path::new(&dir)) {
                zikaron_kit::kitdir::KitVerdict::Ok { entries, files, proofs, invalid, kit_id } => say(
                    &Value::Obj(vec![
                        (Key::Entries.as_str().into(), Value::Int(entries as u64)),
                        (Key::Files.as_str().into(), Value::Int(files as u64)),
                        (Key::KitId.as_str().into(), Value::Str(zikaron::hexfmt::encode(&kit_id))),
                        (String::from("invalid"), Value::Int(invalid.len() as u64)),
                        (Key::Ok.as_str().into(), Value::Bool(true)),
                        (Key::Path.as_str().into(), Value::Str(dir.clone())),
                        (Key::Proofs.as_str().into(), Value::Int(proofs as u64)),
                        (Key::State.as_str().into(), Value::Str(zikaron_kit::tokens::KIT_OK.into())),
                    ]),
                    0,
                ),
                zikaron_kit::kitdir::KitVerdict::Fail { verdict, subject } => say(
                    &Value::Obj(vec![
                        (Key::Detail.as_str().into(), match subject {
                            Some(x) => Value::Str(x),
                            None => Value::Null,
                        }),
                        (Key::Ok.as_str().into(), Value::Bool(false)),
                        (Key::Reason.as_str().into(), Value::Str(String::from("E_KIT"))),
                        (Key::State.as_str().into(), Value::Str(verdict.as_str().into())),
                    ]),
                    1,
                ),
            }
        }
        _ => misuse("usage: zkg <pack|verify> [--flag value ...]"),
    }
}
