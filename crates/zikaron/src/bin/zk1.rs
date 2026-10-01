//! `zk1`: the conformance command surface, the four commands of `base/zikaron-conformance/HARNESS.md`.
//!
//! One call writes one canonical JSON value to stdout with no trailing newline. Any answer (a refusal is an
//! answer) exits 0; harness misuse (an unreadable path, wrong arguments) exits 2; there is no other exit
//! code.

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

/// Paths go to the file system as the OS bytes (HARNESS), without UTF-8; an unreadable path is misuse.
fn read(path: &OsStr) -> Vec<u8> {
    match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => die(&format!("cannot read {}: {e}", path.to_string_lossy())),
    }
}

/// Text arguments (command, private key, domain): bytes that are not UTF-8 make no valid value, so they are
/// misuse.
fn text<'a>(x: &'a OsStr, usage: &str) -> &'a str {
    x.to_str().unwrap_or_else(|| die(usage))
}

/// A command's arguments are exactly its token list; one more or one fewer is misuse.
fn arity(args: &[OsString], lo: usize, hi: usize, usage: &str) {
    let n = args.len().saturating_sub(1);
    if n < lo || n > hi {
        die(usage);
    }
}

/// The domain literal of HARNESS `sign`: signed as given, empty included. Every byte is below 0x80 with no
/// newline (the §5.6 shape); any other literal is misuse.
fn domain_arg(x: &str) -> &str {
    if !x.bytes().all(|b| b < 0x80 && b != 0x0a) {
        die("domain literal is outside the family's shape");
    }
    x
}

fn main() {
    // `args_os`: non-UTF-8 arguments do not panic; exit codes are only 0 and 2.
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let cmd = args.first().and_then(|x| x.to_str()).unwrap_or("");
    match cmd {
        // All of §3.5 and §4.3.
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
        // §3.5 tests 1 to 5; prints the canonical bytes of the parsed value.
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
        // §5.1 to §5.5: RFC 6979 with low s (negation flips v); domain defaults to zikaron/1. The file is
        // read in any RFC 8259 spelling, recanonicalized and signed as is: no member is dropped, a top-level
        // `sig` is signed along.
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
        // The five-input audit of §8, printing the fifteen-item report of §8.7.
        "audit" => {
            arity(&args, 1, 1, "usage: zk1 audit <path>");
            let b = read(&args[1]);
            // The audit input is read by the RFC 8259 reader with five restrictions (HARNESS), i.e. §3.5
            // tests 1 to 3; what that reader refuses and what fails the §9.4 shape both get no label, spelled
            // by `audit::no_label` alone.
            match json::parse_tests_1_3(&b) {
                Ok(v) => emit(&zikaron::audit::audit(&v)),
                Err(_) => emit(&zikaron::audit::no_label()),
            }
        }
        _ => die("usage: zk1 <check|canon|sign|audit> ..."),
    }
}
