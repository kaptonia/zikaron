//! `--key-file`: read the private key from a file rather than the command line, where it would show up in the
//! process table and shell history. Cases: the file gives the same key as `--key` (with prefix, line endings and
//! outer whitespace); exactly one of the two flags; bad paths (missing, a directory); a file readable by other
//! accounts; bad contents (not hex, empty, over the cap, not text, out of range); and every verb that takes
//! `--key` takes `--key-file`.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
const KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
/// The secp256k1 group order: one past the largest valid key.
const ORDER: &str = "0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141";

fn run(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(BIN).args(args).current_dir(dir).env(zikaron_os::HOME_VAR, own_home()).output().expect("runs");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), String::from_utf8_lossy(&o.stderr).into_owned())
}

/// A separate user home per run: `anchor` records what it sent under the user's home (`zikaron_cli::sent`), so
/// runs never share a record or touch the real one.
fn own_home() -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!("zk-cli-home-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)))
}

fn scratch(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("zk-cli-key-file-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("a scratch place");
    p
}

/// Who else may read a test key file.
#[derive(Clone, Copy)]
enum Reach {
    Owner,
    Group,
    Everyone,
}

/// A file holding `bytes`, created owner-only through `zikaron_os`, then opened to others as `reach` says.
fn key_file(dir: &Path, name: &str, bytes: &[u8], reach: Reach) -> String {
    use std::io::Write;
    let p = dir.join(name);
    let mut o = zikaron_os::Options::new();
    o.write(true).create_new(true);
    zikaron_os::owner_only(&mut o).open(&p).expect("the file").write_all(bytes).expect("its bytes");
    match reach {
        Reach::Owner => {}
        Reach::Group => zikaron_os::open_to_others(&p, true).expect("its group reads it"),
        Reach::Everyone => zikaron_os::open_to_others(&p, false).expect("everyone reads it"),
    }
    p.display().to_string()
}

#[test]
fn a_key_file_gives_the_key_the_command_line_gives() {
    let w = scratch("same");
    let (code, by_flag, _) = run(&w, &["init", "--ledger", "a", "--key", KEY, "--statement", "开端"]);
    assert_eq!(code, 0);
    for (form, bytes) in [
        ("asWritten", format!("{KEY}").into_bytes()),
        ("lineEnd", format!("{KEY}\n").into_bytes()),
        ("crlf", format!("{KEY}\r\n").into_bytes()),
        ("whiteSpaceAround", format!("  \t{KEY} \n\n").into_bytes()),
        ("withoutPrefix", KEY.trim_start_matches("0x").as_bytes().to_vec()),
    ] {
        let ledger = format!("b-{form}");
        let f = key_file(&w, &format!("{form}.key"), &bytes, Reach::Owner);
        let (code, by_file, err) = run(&w, &["init", "--ledger", &ledger, "--key-file", &f, "--statement", "开端"]);
        assert_eq!(code, 0, "{form}: {err}");
        let id = |out: &str| zikaron::json::parse(out.as_bytes()).ok().and_then(|v| v.member("entryId").and_then(|x| x.as_str().map(str::to_string)));
        assert!(id(&by_flag).is_some());
        assert_eq!(id(&by_file), id(&by_flag), "{form}: the same entry as `--key` writes");
    }
    let _ = std::fs::remove_dir_all(&w);
}

#[test]
fn every_other_form_is_misuse_by_name() {
    let w = scratch("misuse");
    let good = key_file(&w, "good.key", KEY.as_bytes(), Reach::Owner);
    let open = key_file(&w, "open.key", KEY.as_bytes(), Reach::Everyone);
    let group = key_file(&w, "group.key", KEY.as_bytes(), Reach::Group);
    let not_hex = key_file(&w, "not-hex.key", b"0xzz", Reach::Owner);
    let empty = key_file(&w, "empty.key", b"", Reach::Owner);
    let long = key_file(&w, "long.key", &[b'a'; 4097], Reach::Owner);
    let not_text = key_file(&w, "not-text.key", &[0xff, 0xfe, 0x00], Reach::Owner);
    let bom = key_file(&w, "bom.key", format!("\u{feff}{KEY}").as_bytes(), Reach::Owner);
    let zero = key_file(&w, "zero.key", format!("0x{}", "0".repeat(64)).as_bytes(), Reach::Owner);
    let order = key_file(&w, "order.key", ORDER.as_bytes(), Reach::Owner);
    std::fs::create_dir_all(w.join("a-directory")).expect("a directory");
    let dir = w.join("a-directory").display().to_string();
    let nowhere = w.join("nowhere.key").display().to_string();
    let init = |extra: &[&str]| {
        let mut a = vec!["init", "--ledger", "x", "--statement", "开端"];
        a.extend_from_slice(extra);
        run(&w, &a)
    };
    for (form, extra, head, said) in [
        ("both", vec!["--key", KEY, "--key-file", good.as_str()], "E_ARGS --key --key-file".to_string(), "exactly one of these flags"),
        ("neither", vec![], "E_ARGS --key".to_string(), "this flag is missing"),
        ("notThere", vec!["--key-file", nowhere.as_str()], format!("E_UNREADABLE #7 ({} bytes)", nowhere.len()), ""),
        ("aDirectory", vec!["--key-file", dir.as_str()], format!("E_UNREADABLE #7 ({} bytes)", dir.len()), ""),
        ("readableByOthers", vec!["--key-file", open.as_str()], format!("E_KEY #7 ({} bytes)", open.len()), "other accounts can read this key file"),
        ("readableByTheGroup", vec!["--key-file", group.as_str()], format!("E_KEY #7 ({} bytes)", group.len()), "other accounts can read this key file"),
        ("notHex", vec!["--key-file", not_hex.as_str()], "E_KEY --key-file".to_string(), ""),
        ("empty", vec!["--key-file", empty.as_str()], "E_KEY --key-file".to_string(), ""),
        ("pastTheCap", vec!["--key-file", long.as_str()], "E_KEY --key-file".to_string(), ""),
        ("notText", vec!["--key-file", not_text.as_str()], "E_KEY --key-file".to_string(), ""),
        ("aByteOrderMark", vec!["--key-file", bom.as_str()], "E_KEY --key-file".to_string(), ""),
        ("zero", vec!["--key-file", zero.as_str()], "E_KEY --key-file".to_string(), "not within the curve's range"),
        ("theOrder", vec!["--key-file", order.as_str()], "E_KEY --key-file".to_string(), "not within the curve's range"),
    ] {
        let (code, out, err) = init(&extra);
        assert_eq!((code, out.is_empty()), (2, true), "{form}: misuse, nothing on stdout: {err}");
        assert!(err.starts_with(&head), "{form}: named: {err}");
        assert!(err.contains(said), "{form}: {err}");
        assert!(!err.contains(&KEY[2..]), "{form}: the key is never echoed");
    }
    assert!(!w.join("x").exists(), "nothing written by any of them");
    let _ = std::fs::remove_dir_all(&w);
}

#[test]
fn every_verb_that_takes_the_key_takes_the_key_file() {
    let w = scratch("verbs");
    let open = key_file(&w, "open.key", KEY.as_bytes(), Reach::Everyone);
    for verb in ["init", "history", "grant", "revoke", "adopt", "attest", "succeed", "annotate", "retract", "anchor", "fpm-sign", "ack-sign"] {
        let (_, _, err) = run(&w, &[verb, "--key-file", &open]);
        assert!(!err.contains("not among this verb's flags"), "{verb}: takes --key-file: {err}");
    }
    for verb in ["scan", "audit", "show", "badge", "depth"] {
        let (code, _, err) = run(&w, &[verb, "--key-file", &open]);
        assert_eq!(code, 2, "{verb}");
        assert!(err.starts_with("E_ARGS --key-file") && err.contains("not among this verb's flags"), "{verb}: a verb without --key does not take it: {err}");
    }
    let _ = std::fs::remove_dir_all(&w);
}
