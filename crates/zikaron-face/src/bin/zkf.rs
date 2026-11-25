#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::process::exit;
use zikaron::cryptox;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::v2::entry;
use zikaron_face::reading::{self, NO_TABLE};
use zikaron_face::shapes::{self, Shape};
use zikaron_face::{ENTRY_TYPE, carried, consent, names};

fn die(msg: &str) -> ! {
    eprintln!("zkf: {msg}");
    exit(2)
}

fn emit(v: &Value) -> ! {
    use std::io::Write;
    let bytes = json::canon_bytes(v);
    let mut out = std::io::stdout();
    if out.write_all(&bytes).is_err() || out.flush().is_err() {
        exit(2)
    }
    exit(0)
}

fn obj(members: Vec<(&str, Value)>) -> Value {
    Value::Obj(members.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

const CAP: u64 = 16 * 1024 * 1024;

fn read(path: &OsStr) -> Vec<u8> {
    use std::io::Read;
    let cannot = |why: &dyn std::fmt::Display| -> ! { die(&format!("cannot read {}: {why}", path.to_string_lossy())) };
    let kind = std::fs::metadata(path).unwrap_or_else(|e| cannot(&e)).file_type();
    if !(kind.is_file() || char_device(&kind)) {
        cannot(&"not a regular file or a character device");
    }
    let file = std::fs::File::open(path).unwrap_or_else(|e| cannot(&e));
    let mut bytes = Vec::new();
    file.take(CAP + 1).read_to_end(&mut bytes).unwrap_or_else(|e| cannot(&e));
    if bytes.len() as u64 > CAP {
        cannot(&"more than sixteen mebibytes");
    }
    bytes
}

#[cfg(unix)]
fn char_device(kind: &std::fs::FileType) -> bool {
    use std::os::unix::fs::FileTypeExt;
    kind.is_char_device()
}

#[cfg(not(unix))]
fn char_device(_: &std::fs::FileType) -> bool {
    false
}

fn text<'a>(x: &'a OsStr, usage: &str) -> &'a str {
    x.to_str().unwrap_or_else(|| die(usage))
}

fn hex20_arg<'a>(x: &'a OsStr, usage: &str) -> &'a str {
    let t = text(x, usage);
    if !hexfmt::is_hex20(t) {
        die("<root> and <address> are hex20s");
    }
    t
}

fn arity<'a>(args: &'a [OsString], counts: &[usize], usage: &str) -> &'a [OsString] {
    let rest = &args[1..];
    if !counts.contains(&rest.len()) {
        die(usage);
    }
    rest
}

fn cmd_entry(args: &[OsString]) -> ! {
    let usage = "usage: zkf entry <entry-path> <root>";
    let a = arity(args, &[2], usage);
    let root = hex20_arg(&a[1], usage);
    let b = read(&a[0]);
    let e = match entry::check(&b) {
        Ok(e) => e,
        Err(tok) => emit(&obj(vec![("ok", Value::Bool(false)), ("token", s(tok.as_str()))])),
    };
    if e.entry_type != ENTRY_TYPE {
        emit(&obj(vec![("ok", Value::Bool(true)), ("entry_id", s(&e.id_hex)), ("face", Value::Bool(false))]));
    }
    let shape = shapes::shape_of(&e.body);
    let rooted = shape.map(|_| Value::Bool(shapes::text(&e.body, "root") == root)).unwrap_or(Value::Null);
    let consent = match shape {
        Some(Shape::Key) => Value::Bool(consent::holds(&e.body)),
        _ => Value::Null,
    };
    emit(&obj(vec![
        ("ok", Value::Bool(true)),
        ("entry_id", s(&e.id_hex)),
        ("face", Value::Bool(true)),
        ("shape", shape.map(|x| s(x.as_str())).unwrap_or(Value::Null)),
        ("rooted", rooted),
        ("consent", consent),
    ]))
}

fn cmd_declare(args: &[OsString]) -> ! {
    let usage = "usage: zkf declare <root> <address> <role> [<privkey-hex>]";
    let a = arity(args, &[3, 4], usage);
    let root = hex20_arg(&a[0], usage);
    let address = hex20_arg(&a[1], usage);
    let role = text(&a[2], usage);
    if !names::is_label(role) {
        die("<role> is a label");
    }
    let key = a.get(3).map(|k| {
        let k = hexfmt::scalar32(text(k, usage)).unwrap_or_else(|| die("bad private key"));
        if !cryptox::in_range(&k) {
            die("private key out of range");
        }
        k
    });
    let decl = consent::declaration(root, address, role);
    let mut out = vec![
        ("declaration", s(&hexfmt::encode(&decl))),
        ("message", s(&hexfmt::encode(&consent::message(&decl)))),
        ("digest", s(&hexfmt::encode(&consent::digest(&decl)))),
    ];
    if let Some(k) = key {
        let sig = consent::sign(&k, root, address, role).unwrap_or_else(|| die("signing failed"));
        let signer = cryptox::address_of_privkey(&k).unwrap_or_else(|| die("private key is no secp256k1 key"));
        out.push(("sig", s(&consent::sig_hex(&sig))));
        out.push(("signer", s(&hexfmt::encode(&signer))));
    }
    emit(&obj(out))
}

fn cmd_carried(args: &[OsString]) -> ! {
    let a = arity(args, &[1], "usage: zkf carried <calldata-path>");
    let c = read(&a[0]);
    let id = carried::carried(&c).map(|e| s(&e.id_hex)).unwrap_or(Value::Null);
    emit(&obj(vec![("carried", id)]))
}

fn cmd_table(args: &[OsString]) -> ! {
    let a = arity(args, &[1], "usage: zkf table <face-input.json>");
    let raw = read(&a[0]);
    match reading::table_of(&raw) {
        Ok(t) => emit(&t),
        Err(_) => emit(&obj(vec![("ok", Value::Bool(false)), ("reason", s(NO_TABLE))])),
    }
}

fn cmd_joined(args: &[OsString]) -> ! {
    let a = arity(args, &[1], "usage: zkf joined <pair.json>");
    let raw = read(&a[0]);
    match reading::joined_of(&raw) {
        Ok(j) => emit(&obj(vec![("joined", Value::Bool(j))])),
        Err(_) => emit(&obj(vec![("ok", Value::Bool(false)), ("reason", s(NO_TABLE))])),
    }
}

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let cmd = args.first().and_then(|x| x.to_str()).unwrap_or("");
    match cmd {
        "entry" => cmd_entry(&args),
        "declare" => cmd_declare(&args),
        "carried" => cmd_carried(&args),
        "table" => cmd_table(&args),
        "joined" => cmd_joined(&args),
        _ => die("usage: zkf <entry|declare|carried|table|joined> ..."),
    }
}
