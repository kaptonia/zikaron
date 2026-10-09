//! Command-line edge cases, on the real binary: the two `--registry` refusals told apart in the human-readable
//! line; which environment variable sets its language; a key file that is not a key or is readable by other
//! accounts; `audit --out` into an unwritable folder.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
const KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

struct Ran {
    code: i32,
    out: Vec<u8>,
    err: Vec<u8>,
}

impl Ran {
    fn line(&self, n: usize) -> String {
        String::from_utf8_lossy(&self.err).lines().nth(n).unwrap_or("").to_string()
    }
}

/// Run in `dir` with the locale variables cleared, then `env` set.
fn run(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Ran {
    let mut c = Command::new(BIN);
    c.args(args).current_dir(dir).env(zikaron_os::HOME_VAR, own_home());
    for k in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        c.env_remove(k);
    }
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().expect("zikaron runs");
    Ran { code: o.status.code().unwrap_or(-1), out: o.stdout, err: o.stderr }
}

/// A separate user home per run: `anchor` records what it sent under the user's home (`zikaron_cli::sent`), so
/// runs never share a record or touch the real one.
fn own_home() -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!("zk-cli-home-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)))
}

fn scratch(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("zk-cli-gaps-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("a scratch place");
    p
}

const EN: [(&str, &str); 1] = [("LANG", "en_US.UTF-8")];

/// The two `--registry` refusals share the first line (`E_ARGS --registry`) and differ on the second:
/// `--form registry` without it is "this flag is missing"; with `--form bare` it "goes with `--form registry`
/// only".
#[test]
fn the_two_registry_refusals_are_told_apart_on_the_second_line() {
    let w = scratch("d1");
    let hash = format!("0x{}", "11".repeat(32));
    let base = ["anchor", "--endpoint", "31337=http://127.0.0.1:1", "--key", KEY, "--hash", hash.as_str(), "--wait-secs", "1"];
    let mut missing = base.to_vec();
    missing.extend(["--form", "registry"]);
    let mut bare = base.to_vec();
    bare.extend(["--form", "bare", "--registry", "0x5fbdb2315678afecb367f032d93f642f64180aa3"]);
    for (form, args, said) in [("registry without --registry", missing, "this flag is missing"), ("--registry with bare", bare, "--registry goes with --form registry only")] {
        let r = run(&w, &args, &EN);
        assert_eq!((r.code, r.out.is_empty(), r.line(0).as_str(), r.line(1).as_str()), (2, true, "E_ARGS --registry", said), "{form}");
    }
    let _ = std::fs::remove_dir_all(&w);
}

/// Language precedence: a set `LANG` wins over `LANGUAGE` (LANG=en with LANGUAGE=zh is English); an empty
/// variable is skipped (LC_ALL="" with LANG=zh is Chinese).
#[test]
fn the_language_is_the_first_variable_set_and_not_empty() {
    let w = scratch("d4");
    let args = ["scan", "--fixture", "rec.json", "--basis", "b.json"];
    for (env, second) in [
        (vec![("LANG", "en_US.UTF-8"), ("LANGUAGE", "zh_CN:en")], "a run over recordings does not take this flag"),
        (vec![("LC_ALL", ""), ("LANG", "zh_CN.UTF-8")], "读录制那一路不用这一旗"),
    ] {
        let r = run(&w, &args, &env);
        assert_eq!((r.code, r.line(0).as_str(), r.line(1).as_str()), (2, "E_ARGS --basis", second), "{env:?}");
    }
    let _ = std::fs::remove_dir_all(&w);
}

/// A key file holding `bytes`, made owner-only, then opened to everyone when `open`.
fn key_file(dir: &Path, name: &str, bytes: &[u8], open: bool) -> String {
    use std::io::Write;
    let p = dir.join(name);
    let mut o = zikaron_os::Options::new();
    o.write(true).create_new(true);
    zikaron_os::owner_only(&mut o).open(&p).expect("the file").write_all(bytes).expect("its bytes");
    if open {
        zikaron_os::open_to_others(&p, false).expect("everyone reads it (0644)");
    }
    p.display().to_string()
}

/// Five kinds of invalid key file (not hex, empty, over the size cap, not text, BOM before the key): each gives
/// `E_KEY --key-file` with "not 64 hex digits", and stderr never contains the file's contents.
#[test]
fn a_key_file_that_is_not_a_key_is_said_without_its_contents() {
    let w = scratch("s1-hex");
    let bom = format!("\u{feff}{KEY}").into_bytes();
    // Each form: its bytes and the part of them stderr must not hold.
    for (form, bytes, telltale) in [
        ("notHex", b"0xzzqq".to_vec(), b"zzqq".to_vec()),
        ("empty", Vec::new(), Vec::new()),
        ("pastTheCap", vec![b'a'; 4097], vec![b'a'; 8]),
        ("notText", vec![0xff, 0xfe, 0x00, b's', b'e', b'c'], vec![0xff, 0xfe]),
        ("aByteOrderMark", bom.clone(), KEY[2..].as_bytes().to_vec()),
    ] {
        let f = key_file(&w, &format!("{form}.key"), &bytes, false);
        let r = run(&w, &["init", "--ledger", "x", "--key-file", &f, "--statement", "s"], &EN);
        assert_eq!((r.code, r.out.is_empty(), r.line(0).as_str(), r.line(1).as_str()), (2, true, "E_KEY --key-file", "not 64 hex digits"), "{form}");
        if !telltale.is_empty() {
            assert!(!r.err.windows(telltale.len()).any(|x| x == telltale.as_slice()), "{form}: the file's contents are never said");
        }
        assert!(!r.err.windows(3).any(|x| x == "\u{feff}".as_bytes()), "{form}: no byte order mark said");
    }
    assert!(!w.join("x").exists(), "nothing written");
    let _ = std::fs::remove_dir_all(&w);
}

/// A key file readable by other accounts (0644): `init` and `annotate` each answer `E_KEY`, naming the path by
/// argument position and length only (never the path or the key).
#[test]
fn a_key_file_others_can_read_is_said_by_its_path_for_init_and_annotate() {
    let w = scratch("s1-open");
    let open = key_file(&w, "open.key", KEY.as_bytes(), true);
    for (verb, args) in [
        ("init", vec!["init", "--ledger", "x", "--key-file", open.as_str(), "--statement", "s"]),
        ("annotate", vec!["annotate", "--ledger", "x", "--key-file", open.as_str(), "--note", "n"]),
    ] {
        let r = run(&w, &args, &EN);
        assert_eq!((r.code, r.out.is_empty(), r.line(0)), (2, true, format!("E_KEY #5 ({} bytes)", open.len())), "{verb}");
        assert!(r.line(1).starts_with("other accounts can read this key file"), "{verb}: {}", r.line(1));
        assert!(!String::from_utf8_lossy(&r.err).contains(&KEY[2..]), "{verb}: the key is never said");
    }
    let _ = std::fs::remove_dir_all(&w);
}

/// `audit --out` into an unwritable folder: `"detail":"E_IO: ` followed by the system's message (operation,
/// path, OS text).
#[test]
fn audit_out_into_an_unwritable_folder_says_the_systems_words() {
    let w = scratch("p5-audit");
    assert_eq!(run(&w, &["init", "--ledger", "b", "--key", KEY, "--statement", "s"], &[]).code, 0);
    let locked = w.join("locked");
    std::fs::create_dir_all(&locked).expect("a folder");
    let shut = |on: bool| {
        let mut p = std::fs::metadata(&locked).expect("the folder").permissions();
        p.set_readonly(on);
        std::fs::set_permissions(&locked, p).expect("its permissions");
    };
    shut(true);
    let r = run(&w, &["audit", "--ledger", "b", "--out", "locked/input.json"], &[]);
    shut(false);
    let said = String::from_utf8_lossy(&r.out).into_owned();
    assert_eq!(r.code, 1, "{said}");
    let detail = said.split("\"detail\":\"E_IO: ").nth(1).expect("detail is E_IO with the system's words");
    let detail = detail.split('"').next().unwrap_or("");
    assert!(detail.contains("locked") && detail.contains("Permission denied"), "the path and the system's words: {detail}");
    assert!(!locked.join("input.json").exists());
    let _ = std::fs::remove_dir_all(&w);
}
