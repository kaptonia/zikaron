//! `zkk`: the kit conformance CLI, implementing the commands of `base/zikaron-conformance/HARNESS-KIT.md`.
//!
//! One call writes one canonical JSON value to stdout with no trailing newline. Any answer (a refusal is an
//! answer) exits 0; harness misuse exits 2; there is no other exit code.
//!
//! Positional arguments come first, then options in any order, each option's value as the next token and no
//! option repeated. Any other token sequence (a repeated option, `--now=<int>`, an option before a
//! positional, extra tokens) is misuse.

use std::ffi::{OsStr, OsString};
use std::path::Path;
use std::process::exit;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron_kit::badge;
use zikaron_kit::check::{self, Hop};
use zikaron_kit::doc::{self, Pairing};
use zikaron_kit::kitdir::{self, KitVerdict};
use zikaron_kit::reading;
use zikaron_kit::tokens::{self as t, Attribution, Domain, Key, PairVerdict};

fn die(msg: &str) -> ! {
    eprintln!("zkk: {msg}");
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

/// Builds every command output object; keys come from [`Key`].
fn obj(members: Vec<(Key, Value)>) -> Value {
    Value::Obj(members.into_iter().map(|(k, v)| (k.as_str().to_string(), v)).collect())
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

/// `--now` uses the §3.2 int spelling: decimal digits, unsigned, no leading zero, at most 2^53 − 1; anything
/// else is misuse.
fn parse_now(raw: &str) -> u64 {
    if raw.is_empty() || !raw.bytes().all(|c| c.is_ascii_digit()) {
        die("--now takes a decimal int");
    }
    if raw.len() > 1 && raw.starts_with('0') {
        die("--now carries a leading zero");
    }
    if raw.len() > 16 {
        die("--now is beyond the value universe");
    }
    let v: u64 = raw.parse().unwrap_or(0);
    if v > zikaron::json::MAX_INT {
        die("--now is beyond the value universe");
    }
    v
}

/// Paths are passed to the file system as raw OS bytes (per the harness), not via UTF-8; an unreadable path is
/// misuse.
fn read(path: &OsStr) -> Vec<u8> {
    match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => die(&format!("cannot read {}: {e}", path.to_string_lossy())),
    }
}

/// Text arguments (command, option names, private key, domain, work digest, `--now`): not UTF-8 is misuse.
fn text<'a>(x: &'a OsStr, usage: &str) -> &'a str {
    x.to_str().unwrap_or_else(|| die(usage))
}

fn starts_with_dashes(x: &OsStr) -> bool {
    x.to_str().map(|s| s.starts_with("--")).unwrap_or(false)
}

/// The options of a command: tokens after the positionals, `--audit <f>` and `--now <int>` at most once each.
struct Opts {
    audit: Option<OsString>,
    now: Option<u64>,
}

fn options(tokens: &[OsString], allow_audit: bool, usage: &str) -> Opts {
    let mut audit: Option<OsString> = None;
    let mut now: Option<u64> = None;
    let mut i = 0;
    while i < tokens.len() {
        match text(&tokens[i], usage) {
            "--audit" if allow_audit => {
                if audit.is_some() {
                    die(usage);
                }
                audit = Some(tokens.get(i + 1).unwrap_or_else(|| die(usage)).clone());
                i += 2;
            }
            "--now" => {
                if now.is_some() {
                    die(usage);
                }
                now = Some(parse_now(text(tokens.get(i + 1).unwrap_or_else(|| die(usage)), usage)));
                i += 2;
            }
            _ => die(usage),
        }
    }
    Opts { audit, now }
}

/// Exactly n positionals, then options; an option token among the positionals is misuse.
fn positional<'a>(args: &'a [OsString], n: usize, usage: &str) -> (&'a [OsString], &'a [OsString]) {
    if args.len() < 1 + n {
        die(usage);
    }
    let pos = &args[1..1 + n];
    if pos.iter().any(|x| starts_with_dashes(x)) {
        die(usage);
    }
    (pos, &args[1 + n..])
}

fn no_options(rest: &[OsString], usage: &str) {
    if !rest.is_empty() {
        die(usage);
    }
}

/// Audit input file to audit outcome. An unreadable file is misuse (exit 2, as for other file arguments); a
/// readable file the reader refuses or that fails the §9.4 shape is invalid input per the spec and gives `None`.
fn outcome_of(path: &OsStr) -> Option<zikaron::audit::Outcome> {
    let b = read(path);
    let v = json::parse_tests_1_3(&b).ok()?;
    zikaron::audit::audit_full(&v)
}

/// Audit input file to its value (the six checks add g to the pile and audit again); an unreadable file exits
/// 2.
fn input_value(path: &OsStr) -> Option<Value> {
    let b = read(path);
    json::parse_tests_1_3(&b).ok()
}

fn doc_reject(r: &doc::Reject) -> Value {
    let mut ms: Vec<(Key, Value)> = vec![(Key::Ok, Value::Bool(false)), (Key::Token, s(r.token.as_str()))];
    if let Some(i) = r.index {
        ms.push((Key::Index, Value::Int(i as u64)));
    }
    obj(ms)
}

fn encode_reject(r: &badge::EncodeReject) -> Value {
    let mut ms: Vec<(Key, Value)> = vec![(Key::Ok, Value::Bool(false)), (Key::Token, s(r.token.as_str()))];
    if let Some(i) = r.index {
        ms.push((Key::Index, Value::Int(i as u64)));
    }
    obj(ms)
}

fn decode_reject(r: &badge::DecodeReject) -> Value {
    let mut ms: Vec<(Key, Value)> = vec![(Key::Ok, Value::Bool(false)), (Key::Token, s(r.token.as_str()))];
    if let Some(i) = r.index {
        ms.push((Key::Index, Value::Int(i as u64)));
    }
    if let Some(inner) = r.inner {
        ms.push((Key::Inner, s(inner.as_str())));
    }
    obj(ms)
}

fn pairing_value(p: &Pairing) -> Value {
    match p {
        Pairing::Paired { recipient, variant } => obj(vec![
            (Key::Verdict, s(PairVerdict::Paired.as_str())),
            (Key::Recipient, s(recipient)),
            (Key::Variant, s(variant)),
        ]),
        Pairing::FpmInvalid(r) => obj(vec![(Key::Verdict, s(PairVerdict::FpmInvalid.as_str())), (Key::Token, s(r.token.as_str()))]),
        Pairing::AckInvalid(r) => obj(vec![(Key::Verdict, s(PairVerdict::AckInvalid.as_str())), (Key::Token, s(r.token.as_str()))]),
        other => obj(vec![(Key::Verdict, s(other.verdict().as_str()))]),
    }
}

fn main() {
    // `args_os`: non-UTF-8 arguments do not panic; exit codes are only 0 and 2.
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let cmd = args.first().and_then(|x| x.to_str()).unwrap_or("");

    match cmd {
        // Documents (kit law §4, §5).
        "fpm-check" => {
            let usage = "usage: zkk fpm-check <path>";
            let (pos, rest) = positional(&args, 1, usage);
            no_options(rest, usage);
            let b = read(&pos[0]);
            match doc::check_fpm(&b) {
                Ok(f) => emit(&obj(vec![
                    (Key::DocId, s(&hexfmt::encode(&f.doc_id))),
                    (Key::Ok, Value::Bool(true)),
                ])),
                Err(r) => emit(&doc_reject(&r)),
            }
        }
        "ack-check" => {
            let usage = "usage: zkk ack-check <path>";
            let (pos, rest) = positional(&args, 1, usage);
            no_options(rest, usage);
            let b = read(&pos[0]);
            match doc::check_ack(&b) {
                Ok(a) => emit(&obj(vec![
                    (Key::DocId, s(&hexfmt::encode(&a.doc_id))),
                    (Key::Ok, Value::Bool(true)),
                ])),
                Err(r) => emit(&doc_reject(&r)),
            }
        }
        // Signing (kit law §3.1), per the `zikaron/1` contract: the file is read in any RFC 8259 spelling,
        // recanonicalized and signed as is with no member dropped; a non-object or a value failing tests 1 to 5
        // is misuse.
        "sign" => {
            let usage = "usage: zkk sign <privkey-hex> <path> <domain>";
            let (pos, rest) = positional(&args, 3, usage);
            no_options(rest, usage);
            let domain = Domain::parse(text(&pos[2], usage)).unwrap_or_else(|| die("domain must be zikaron.fpm/1 or zikaron.ack/1"));
            let key = hexfmt::scalar32(text(&pos[0], usage)).unwrap_or_else(|| die("privkey is not sixty-four hex digits"));
            if !zikaron::cryptox::in_range(&key) {
                die("privkey is out of the curve's range");
            }
            let bytes = read(&pos[1]);
            let parsed = json::parse_tests_1_5(&bytes).unwrap_or_else(|_| die("preimage fails tests 1 through 5"));
            if !parsed.is_obj() {
                die("preimage is not an object");
            }
            let preimage = json::canon_bytes(&parsed);
            let (presig, digest) = zikaron::entry::presig_and_digest(&preimage, domain.as_str());
            let (r, sg, v) = zikaron::cryptox::sign_digest(&key, &digest).unwrap_or_else(|| die("signing failed"));
            let signer = zikaron::cryptox::address_of_privkey(&key).unwrap_or_else(|| die("privkey is not a secp256k1 key"));
            let mut sig = Vec::with_capacity(65);
            sig.extend_from_slice(&r);
            sig.extend_from_slice(&sg);
            sig.push(v);
            emit(&obj(vec![
                (Key::Digest, s(&hexfmt::encode(&digest))),
                (Key::Presig, s(&hexfmt::encode(&presig))),
                (Key::Sig, s(&hexfmt::encode(&sig))),
                (Key::Signer, s(&hexfmt::encode(&signer))),
            ]))
        }
        // Pairing and attribution (kit law §5.3, §5.4).
        "pair" => {
            let usage = "usage: zkk pair <fpm-path> <ack-path>";
            let (pos, rest) = positional(&args, 2, usage);
            no_options(rest, usage);
            let m = read(&pos[0]);
            let a = read(&pos[1]);
            emit(&pairing_value(&doc::pair(&m, &a)))
        }
        "attribute" => {
            let usage = "usage: zkk attribute <fpm-path> <ack-path> <bytes-path>";
            let (pos, rest) = positional(&args, 3, usage);
            no_options(rest, usage);
            let m = read(&pos[0]);
            let a = read(&pos[1]);
            let x = read(&pos[2]);
            let (p, hit) = doc::attribute(&m, &a, &x);
            match hit {
                Some(true) => {
                    let recipient = match &p {
                        Pairing::Paired { recipient, .. } => recipient.clone(),
                        _ => unreachable!(),
                    };
                    emit(&obj(vec![
                        (Key::Attributed, Value::Bool(true)),
                        (Key::Recipient, s(&recipient)),
                        (Key::Verdict, s(Attribution::Attributed.as_str())),
                    ]))
                }
                Some(false) => emit(&obj(vec![
                    (Key::Attributed, Value::Bool(false)),
                    (Key::Verdict, s(Attribution::NotAttributed.as_str())),
                ])),
                None => emit(&obj(vec![
                    (Key::Attributed, Value::Bool(false)),
                    (Key::Verdict, s(p.verdict().as_str())),
                ])),
            }
        }
        // Grant payloads (kit law §6).
        "badge-encode" => {
            let usage = "usage: zkk badge-encode <entry-path>...";
            if args.len() < 2 || args[1..].iter().any(|x| starts_with_dashes(x)) {
                die(usage);
            }
            let entries: Vec<Vec<u8>> = args[1..].iter().map(|p| read(p)).collect();
            match badge::encode(&entries) {
                Ok(payload) => emit(&obj(vec![(Key::Payload, s(&payload))])),
                Err(r) => emit(&encode_reject(&r)),
            }
        }
        "badge-decode" => {
            let usage = "usage: zkk badge-decode <path>";
            let (pos, rest) = positional(&args, 1, usage);
            no_options(rest, usage);
            let b = read(&pos[0]);
            match badge::decode(&b) {
                Ok(grants) => emit(&obj(vec![
                    (
                        Key::Grants,
                        Value::Arr(grants.iter().map(|g| s(&g.id_hex())).collect()),
                    ),
                    (Key::Ok, Value::Bool(true)),
                ])),
                Err(r) => emit(&decode_reject(&r)),
            }
        }
        // Disclosure kits (kit law §7). `<dir>` is the root handed to the reader, not an entry of the walk:
        // symlink or not, if it resolves to a directory, that is the kit. Missing or not a directory is misuse;
        // existing but unlistable is the first walk failure of §7.1.
        "kit-verify" => {
            let usage = "usage: zkk kit-verify <dir>";
            let (pos, rest) = positional(&args, 1, usage);
            no_options(rest, usage);
            let dir = Path::new(&pos[0]);
            match std::fs::metadata(dir) {
                Ok(m) if m.is_dir() => {}
                _ => die("dir is absent or is not a directory"),
            }
            match kitdir::verify_kit(dir) {
                KitVerdict::Ok {
                    entries,
                    files,
                    proofs,
                    invalid,
                    kit_id,
                } => emit(&obj(vec![
                    (
                        Key::Counts,
                        obj(vec![
                            (Key::Entries, Value::Int(entries as u64)),
                            (Key::Files, Value::Int(files as u64)),
                            (Key::Proofs, Value::Int(proofs as u64)),
                        ]),
                    ),
                    (
                        Key::InvalidEntries,
                        Value::Arr(
                            invalid
                                .iter()
                                .map(|(id, tok)| {
                                    obj(vec![(Key::EntryId, s(id)), (Key::Token, s(tok.as_str()))])
                                })
                                .collect(),
                        ),
                    ),
                    (Key::KitId, s(&hexfmt::encode(&kit_id))),
                    (Key::Verdict, s(t::KIT_OK)),
                ])),
                KitVerdict::Fail { verdict, subject } => {
                    let mut ms: Vec<(Key, Value)> = vec![(Key::Verdict, s(verdict.as_str()))];
                    if let Some(x) = &subject {
                        ms.push((Key::Subject, s(x)));
                    }
                    emit(&obj(ms))
                }
            }
        }
        // Depth (kit law §9).
        "depth" => {
            let usage = "usage: zkk depth <audit-input.json> <work-hex32>";
            let (pos, rest) = positional(&args, 2, usage);
            no_options(rest, usage);
            // The work digest is a spec value (hex32, lowercase); any other spelling is misuse, not "not found".
            let work = text(&pos[1], usage);
            if !hexfmt::is_hex32(work) {
                die("work digest is not hex32");
            }
            let o = outcome_of(&pos[0]);
            emit(&reading::depth(o.as_ref(), work))
        }
        // Six checks and chain check (kit law §10).
        "grant-check" => {
            let usage = "usage: zkk grant-check <grant-path> [--audit <f>] [--now <int>]";
            let (pos, rest) = positional(&args, 1, usage);
            let opts = options(rest, true, usage);
            let g = read(&pos[0]);
            let input = opts.audit.as_deref().and_then(input_value);
            emit(&check::grant_check(&g, input.as_ref(), opts.now).value)
        }
        // The hops file: an array whose elements have exactly `grant` (a path) and `audit` (a path or null);
        // any other shape (duplicate member names included) is misuse. Paths resolve against the working
        // directory.
        "chain-check" => {
            let usage = "usage: zkk chain-check <hops.json> [--now <int>]";
            let (pos, rest) = positional(&args, 1, usage);
            let opts = options(rest, false, usage);
            let raw = read(&pos[0]);
            let doc = json::parse_tests_1_3(&raw).unwrap_or_else(|_| die("hops file is no JSON value"));
            let items = match doc.as_arr() {
                Some(a) => a.clone(),
                None => die("hops file is not an array"),
            };
            let mut bytes: Vec<Vec<u8>> = Vec::with_capacity(items.len());
            let mut inputs: Vec<Option<Value>> = Vec::with_capacity(items.len());
            for it in &items {
                let ms = match it {
                    Value::Obj(ms) if ms.len() == 2 => ms,
                    _ => die("a hop is not an object of exactly two members"),
                };
                let gp = match ms.iter().find(|(k, _)| k == "grant") {
                    Some((_, Value::Str(p))) => p.clone(),
                    _ => die("a hop carries no grant path"),
                };
                let audit = match ms.iter().find(|(k, _)| k == "audit") {
                    Some((_, Value::Str(p))) => Some(p.clone()),
                    Some((_, Value::Null)) => None,
                    _ => die("a hop's audit is neither a path nor null"),
                };
                bytes.push(read(OsStr::new(&gp)));
                inputs.push(audit.as_deref().map(OsStr::new).and_then(input_value));
            }
            let hops: Vec<Hop> = bytes
                .iter()
                .zip(inputs.into_iter())
                .map(|(g, input)| Hop { grant: g, input })
                .collect();
            emit(&check::chain_check(&hops, opts.now))
        }
        _ => die("usage: zkk <fpm-check|ack-check|sign|pair|attribute|badge-encode|badge-decode|kit-verify|depth|grant-check|chain-check> ..."),
    }
}
