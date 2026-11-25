#![forbid(unsafe_code)]

use std::ffi::{OsStr, OsString};
use std::process::exit;
use zikaron::entry;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::tokens::{Domain, Token};

fn die(msg: &str) -> ! {
    eprintln!("zk1: {msg}");
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

fn rejected(token: Token) -> Value {
    obj(vec![
        ("ok", Value::Bool(false)),
        ("token", Value::Str(token.as_str().to_string())),
    ])
}

fn read(path: &OsStr) -> Vec<u8> {
    match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => die(&format!("cannot read {}: {e}", path.to_string_lossy())),
    }
}

fn text<'a>(x: &'a OsStr, usage: &str) -> &'a str {
    x.to_str().unwrap_or_else(|| die(usage))
}

fn arity(args: &[OsString], lo: usize, hi: usize, usage: &str) {
    let n = args.len().saturating_sub(1);
    if n < lo || n > hi {
        die(usage);
    }
}

fn domain_arg(x: &str) -> &str {
    if !x.bytes().all(|b| b < 0x80 && b != 0x0a) {
        die("domain literal is outside the family's shape");
    }
    x
}

fn main() {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let cmd = args.first().and_then(|x| x.to_str()).unwrap_or("");
    match cmd {
        "check" => {
            arity(&args, 1, 1, "usage: zk1 check <path>");
            let b = read(&args[1]);
            match entry::check(&b) {
                Ok(e) => emit(&obj(vec![
                    ("entry_id", Value::Str(e.id_hex())),
                    ("ok", Value::Bool(true)),
                ])),
                Err(tok) => emit(&rejected(tok.into())),
            }
        }
        "canon" => {
            arity(&args, 1, 1, "usage: zk1 canon <path>");
            let b = read(&args[1]);
            match json::parse_tests_1_5(&b) {
                Ok(v) => {
                    let c = json::canon_bytes(&v);
                    emit(&obj(vec![
                        ("canon", Value::Str(hexfmt::encode(&c))),
                        ("ok", Value::Bool(true)),
                    ]))
                }
                Err(tok) => emit(&rejected(tok.into())),
            }
        }
        "sign" => {
            let usage = "usage: zk1 sign <privkey-hex> <path> [domain]";
            arity(&args, 2, 3, usage);
            let key = hexfmt::scalar32(text(&args[1], usage)).unwrap_or_else(|| die("privkey is not sixty-four hex digits"));
            if !zikaron::cryptox::in_range(&key) {
                die("privkey is out of the curve's range");
            }
            let bytes = read(&args[2]);
            let domain = match args.get(3) {
                Some(d) => domain_arg(text(d, usage)),
                None => Domain::Entry.as_str(),
            };
            let parsed = json::parse_tests_1_5(&bytes).unwrap_or_else(|_| die("preimage fails tests 1 through 5"));
            if !parsed.is_obj() {
                die("preimage is not an object");
            }
            let preimage = json::canon_bytes(&parsed);
            let (presig, digest) = entry::presig_and_digest(&preimage, domain);
            let (r, s, v) = zikaron::cryptox::sign_digest(&key, &digest)
                .unwrap_or_else(|| die("signing failed"));
            let signer = zikaron::cryptox::address_of_privkey(&key)
                .unwrap_or_else(|| die("privkey is not a secp256k1 key"));
            let mut sig = Vec::with_capacity(65);
            sig.extend_from_slice(&r);
            sig.extend_from_slice(&s);
            sig.push(v);
            emit(&obj(vec![
                ("digest", Value::Str(hexfmt::encode(&digest))),
                ("presig", Value::Str(hexfmt::encode(&presig))),
                ("sig", Value::Str(hexfmt::encode(&sig))),
                ("signer", Value::Str(hexfmt::encode(&signer))),
            ]))
        }
        "audit" => {
            arity(&args, 1, 1, "usage: zk1 audit <path>");
            let b = read(&args[1]);
            match json::parse_tests_1_3(&b) {
                Ok(v) => emit(&zikaron::audit::audit(&v)),
                Err(_) => emit(&zikaron::audit::no_label()),
            }
        }
        _ => die("usage: zk1 <check|canon|sign|audit> ..."),
    }
}
