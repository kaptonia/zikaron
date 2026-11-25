#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::exit;
use zikaron::json::{self, Value};
use zikaron_kit::tokens::BadgeDecodeToken;
use zikaron_kit::v2::doc;
use zikaron_kit::v2::kit::{self, Enumeration, KitVerdict};
use zikaron_kit::v2::read::{self, Notices, View};
use zikaron_kit::v2::tokens::{Key, KitToken};

fn die(msg: &str) -> ! {
    eprintln!("zkk2: {msg}");
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

fn obj(members: Vec<(Key, Value)>) -> Value {
    Value::Obj(members.into_iter().map(|(k, v)| (k.as_str().to_string(), v)).collect())
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

fn parse_now(x: &OsStr) -> u64 {
    let t = x.to_str().unwrap_or_else(|| die("--now takes a zikaron/2 int"));
    if t.is_empty() || !t.bytes().all(|c| c.is_ascii_digit()) || (t.len() > 1 && t.starts_with('0')) {
        die("--now takes a zikaron/2 int");
    }
    match t.parse::<u64>() {
        Ok(n) if n <= json::MAX_INT => n,
        _ => die("--now is outside the integer universe"),
    }
}

struct Options {
    audit: Option<OsString>,
    notices: Option<OsString>,
    now: Option<u64>,
}

fn options(rest: &[OsString], allow_inputs: bool, usage: &str) -> Options {
    let mut o = Options { audit: None, notices: None, now: None };
    let mut i = 0;
    while i < rest.len() {
        let has_value = i + 1 < rest.len();
        match rest[i].to_str() {
            Some("--audit") if allow_inputs && has_value && o.audit.is_none() => o.audit = Some(rest[i + 1].clone()),
            Some("--notices") if allow_inputs && has_value && o.notices.is_none() => o.notices = Some(rest[i + 1].clone()),
            Some("--now") if has_value && o.now.is_none() => o.now = Some(parse_now(&rest[i + 1])),
            _ => die(usage),
        }
        i += 2;
    }
    o
}

fn view_of(path: &OsStr, g: &[u8]) -> Option<View> {
    let raw = read(path);
    let v = json::parse_tests_1_3(&raw).ok()?;
    read::load_view(&v, Some(g))
}

fn notice_input_of(path: &OsStr) -> Option<Notices> {
    let raw = read(path);
    let v = json::parse_tests_1_3(&raw).ok()?;
    read::load_notices(&v)
}

fn enumeration_of(path: &OsStr) -> Result<Enumeration, String> {
    let p = Path::new(path);
    match std::fs::metadata(p) {
        Ok(m) if m.is_dir() => kit::walk(p),
        Ok(m) if m.is_file() => match std::fs::read(p) {
            Ok(b) => kit::parse_one_file(&b),
            Err(_) => Err(".".to_string()),
        },
        _ => die(&format!("not a directory or a regular file: {}", path.to_string_lossy())),
    }
}

fn unreadable(subject: String) -> Value {
    kit::kit_verdict_value(&KitVerdict::Fail { token: KitToken::Unreadable, subject: Some(subject) })
}

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let cmd = args.first().and_then(|x| x.to_str()).unwrap_or("");
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };
    match cmd {
        "badge-encode" => {
            if rest.is_empty() {
                die("usage: zkk2 badge-encode <entry-path>...");
            }
            let blobs: Vec<Vec<u8>> = rest.iter().map(|p| read(p)).collect();
            match doc::badge_encode(&blobs) {
                Ok(payload) => emit(&obj(vec![(Key::Payload, s(&payload))])),
                Err(r) => {
                    let mut ms = vec![(Key::Ok, Value::Bool(false)), (Key::Token, s(r.token.as_str()))];
                    if let Some(k) = r.index {
                        ms.push((Key::Index, Value::Int(k as u64)));
                    }
                    emit(&obj(ms))
                }
            }
        }
        "badge-decode" => {
            if rest.len() != 1 {
                die("usage: zkk2 badge-decode <path>");
            }
            match doc::badge_decode(&read(&rest[0])) {
                Ok(grants) => emit(&obj(vec![
                    (Key::Ok, Value::Bool(true)),
                    (Key::Grants, Value::Arr(grants.iter().map(|g| s(&g.id_hex())).collect())),
                ])),
                Err(r) => {
                    let mut ms = vec![(Key::Ok, Value::Bool(false)), (Key::Token, s(r.token.as_str()))];
                    let per_segment = matches!(
                        r.token,
                        BadgeDecodeToken::B64 | BadgeDecodeToken::Entry | BadgeDecodeToken::Type | BadgeDecodeToken::Link
                    );
                    if let (true, Some(k)) = (per_segment, r.index) {
                        ms.push((Key::Index, Value::Int(k as u64)));
                    }
                    if let Some(inner) = r.inner {
                        ms.push((Key::Inner, s(inner.as_str())));
                    }
                    emit(&obj(ms))
                }
            }
        }
        "kit-verify" => {
            if rest.len() != 1 {
                die("usage: zkk2 kit-verify <path>");
            }
            match enumeration_of(&rest[0]) {
                Ok(en) => emit(&kit::kit_verdict_value(&kit::verify_kit(&en))),
                Err(subject) => emit(&unreadable(subject)),
            }
        }
        "grant-package" => {
            if rest.len() != 1 {
                die("usage: zkk2 grant-package <path>");
            }
            match enumeration_of(&rest[0]) {
                Ok(en) => emit(&kit::verify_grant_package(&en)),
                Err(subject) => emit(&unreadable(subject)),
            }
        }
        "grant-check" => {
            let usage = "usage: zkk2 grant-check <grant-path> [--audit <f>] [--notices <f>] [--now <int>]";
            if rest.is_empty() {
                die(usage);
            }
            let o = options(&rest[1..], true, usage);
            let g = read(&rest[0]);
            let view = o.audit.as_deref().and_then(|a| view_of(a, &g));
            let notices = o.notices.as_deref().and_then(notice_input_of);
            emit(&read::grant_check(&g, view.as_ref(), notices.as_ref(), o.now).result)
        }
        "chain-check" => {
            let usage = "usage: zkk2 chain-check <hops.json> [--now <int>]";
            if rest.is_empty() {
                die(usage);
            }
            let now = options(&rest[1..], false, usage).now;
            let raw = read(&rest[0]);
            let spec = json::parse_tests_1_3(&raw).unwrap_or_else(|_| die("<hops.json> is not readable JSON"));
            let items = spec.as_arr().unwrap_or_else(|| die("<hops.json> must be an array"));
            let mut hops: Vec<read::Hop> = Vec::with_capacity(items.len());
            for h in items {
                let shape = "each hop is {\"grant\":<path>,\"audit\":<path>|null,\"notices\":<path>|null}";
                let Value::Obj(ms) = h else { die(shape) };
                if ms.len() != 3 {
                    die(shape);
                }
                let path_or_null = |k: &str| match h.member(k) {
                    Some(Value::Str(p)) => Some(p.clone()),
                    Some(Value::Null) => None,
                    _ => die(shape),
                };
                let grant = match h.member("grant") {
                    Some(Value::Str(p)) => p.clone(),
                    _ => die(shape),
                };
                let audit = path_or_null("audit");
                let notices = path_or_null("notices");
                let g = read(OsStr::new(&grant));
                let view = audit.as_deref().and_then(|a| view_of(OsStr::new(a), &g));
                let notices = notices.as_deref().and_then(|n| notice_input_of(OsStr::new(n)));
                hops.push((g, view, notices));
            }
            emit(&read::chain_check(&hops, now))
        }
        _ => die("usage: zkk2 {badge-decode|badge-encode|chain-check|grant-check|grant-package|kit-verify} ..."),
    }
}
