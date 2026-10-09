//! Misuse wording: flag combinations are checked before any node is contacted; a run over recorded fixtures
//! accepts no flag it would ignore; human-readable lines follow the system language, and arbitrary user words
//! are never echoed. Real-binary runs against local listeners that count connections.

use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
const KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

struct Ran {
    code: i32,
    out: Vec<u8>,
    err: String,
}

fn run(args: &[&str], env: &[(&str, &str)]) -> Ran {
    let mut c = Command::new(BIN);
    c.args(args).env(zikaron_os::HOME_VAR, own_home());
    for k in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        c.env_remove(k);
    }
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().expect("zikaron runs");
    Ran { code: o.status.code().unwrap_or(-1), out: o.stdout, err: String::from_utf8_lossy(&o.stderr).to_string() }
}

/// A separate user home per run: `anchor` records what it sent under the user's home (`zikaron_cli::sent`), so
/// runs never share a record or touch the real one.
fn own_home() -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!("zk-cli-home-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)))
}

/// A listener that counts connections and answers none.
fn counting_node() -> (String, Arc<AtomicUsize>) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let n = Arc::new(AtomicUsize::new(0));
    let c = n.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            c.fetch_add(1, Ordering::SeqCst);
            drop(s);
        }
    });
    (url, n)
}

fn first_line(r: &Ran) -> &str {
    r.err.lines().next().unwrap_or("")
}

/// `--form registry` without `--registry`, and `--registry` with `--form bare`, are misuse naming `--registry`,
/// refused before any node is contacted (no connection, nothing on stdout).
#[test]
fn the_registry_flag_goes_with_the_registry_form_and_is_judged_before_any_node() {
    let (url, asked) = counting_node();
    let ep = format!("31337={url}");
    let hash = format!("0x{}", "11".repeat(32));
    for (form, extra) in [("registry", vec![]), ("bare", vec!["--registry", "0x5fbdb2315678afecb367f032d93f642f64180aa3"])] {
        let mut args = vec!["anchor", "--endpoint", &ep, "--key", KEY, "--form", form, "--hash", &hash, "--wait-secs", "1"];
        args.extend(extra.iter());
        let r = run(&args, &[]);
        assert_eq!((r.code, r.out.is_empty(), first_line(&r)), (2, true, "E_ARGS --registry"), "{form}: {}", r.err);
    }
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(asked.load(Ordering::SeqCst), 0, "no node asked");
}

/// With `--fixture`, `--basis`, `--adoptions` and `--proxy` are misuse, once or repeated, each named.
#[test]
fn a_run_over_recordings_takes_no_flag_it_would_not_read() {
    for (flag, value) in [("--basis", "b.json"), ("--adoptions", "a.json"), ("--proxy", "none")] {
        for times in [1, 2] {
            let mut args = vec!["scan", "--fixture", "rec.json"];
            for _ in 0..times {
                args.extend([flag, value]);
            }
            let r = run(&args, &[]);
            assert_eq!((r.code, r.out.is_empty(), first_line(&r)), (2, true, format!("E_ARGS {flag}").as_str()), "{flag} ×{times}: {}", r.err);
        }
    }
}

/// The second line follows the locale (first set of `LC_ALL`, `LC_MESSAGES`, `LANG`, `LANGUAGE`): Chinese for
/// `zh…`, English otherwise or when none is set; the first line and the exit code do not change.
#[test]
fn the_lines_for_people_follow_the_systems_language() {
    let args = ["scan", "--fixture", "rec.json", "--basis", "b.json"];
    for (env, zh) in [
        (vec![], false),
        (vec![("LANG", "zh_CN.UTF-8")], true),
        (vec![("LANG", "en_US.UTF-8")], false),
        (vec![("LANG", "C")], false),
        (vec![("LANG", "en_US.UTF-8"), ("LC_ALL", "zh_TW.UTF-8")], true),
        (vec![("LC_MESSAGES", "zh_CN"), ("LANG", "en_US")], true),
        (vec![("LANGUAGE", "zh_CN:en")], true),
        (vec![("LANG", "")], false),
    ] {
        let r = run(&args, &env);
        let second = r.err.lines().nth(1).unwrap_or("");
        assert_eq!((r.code, first_line(&r)), (2, "E_ARGS --basis"), "{env:?}");
        assert_eq!(second == "读录制那一路不用这一旗", zh, "{env:?}: {second}");
        assert_eq!(second == "a run over recordings does not take this flag", !zh, "{env:?}: {second}");
    }
}

/// A word where a flag belongs, an unknown verb and a non-UTF-8 argument are reported by position and length,
/// never echoed.
#[test]
fn a_word_that_may_be_anything_is_never_echoed() {
    let secret = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
    let r = run(&["show", secret], &[]);
    assert_eq!((r.code, first_line(&r)), (2, "E_ARGS #2 (66 bytes)"));
    assert!(!r.err.contains(&secret[2..]), "{}", r.err);
    let r = run(&["show", "--ledger", "x", secret], &[]);
    assert_eq!(first_line(&r), "E_ARGS #4 (66 bytes)");
    let r = run(&[secret], &[]);
    assert_eq!((r.code, first_line(&r)), (2, "E_ARGS #1 (66 bytes)"));
    assert!(!r.err.contains(&secret[2..]));
    #[cfg(unix)]
    {
        // SAFETY: on unix an OS string's encoded bytes are any bytes at all.
        let not_text = unsafe { std::ffi::OsStr::from_encoded_bytes_unchecked(b"\xffsecret") };
        let o = Command::new(BIN).arg("show").arg(not_text).output().expect("runs");
        let err = String::from_utf8_lossy(&o.stderr).to_string();
        assert_eq!((o.status.code(), err.lines().next()), (Some(2), Some("E_ARGS #2 (7 bytes)")));
        assert!(!err.contains("secret"), "{err}");
    }
}

/// `--endpoint` uses the shared parser: outer whitespace is accepted; inner whitespace, an empty address and a
/// chain id that is not a u64 are misuse (exit 2), reported by position and length, never echoed (the chain id
/// only when it parses).
#[test]
fn the_endpoint_flag_is_read_by_the_one_reader() {
    let hash = format!("0x{}", "11".repeat(32));
    let anchor = |ep: &str| run(&["anchor", "--endpoint", ep, "--key", KEY, "--form", "bare", "--hash", &hash, "--wait-secs", "1"], &[]);
    let r = anchor("31337=");
    assert_eq!((r.code, first_line(&r)), (2, "E_ARGS #3 (31337= + 0 bytes)"));
    let r = anchor(" x =http://127.0.0.1:1");
    assert_eq!((r.code, first_line(&r)), (2, "E_ARGS #3 (22 bytes)"));
    let r = anchor("31337");
    assert_eq!((r.code, first_line(&r)), (2, "E_ARGS #3 (5 bytes)"));
    // Outer whitespace is accepted (the absent node leaves it unanswered); whitespace around the `=` is inside
    // the value, so misuse.
    let closed = { let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind"); format!("http://{}", l.local_addr().expect("addr")) };
    let r = anchor(&format!(" 31337={closed} "));
    assert_eq!(r.code, 4, "{}", r.err);
    let inside = format!(" 31337 = {closed} ");
    let r = anchor(&inside);
    assert_eq!((r.code, first_line(&r)), (2, format!("E_ARGS #3 ({} bytes)", inside.len()).as_str()));
}

/// An effective time or window end past the spec's integer ceiling (within 64 bits or beyond) is misuse naming
/// the flag and the ceiling; a node address whose port is not a 16-bit number is misuse about the port, not the
/// scheme.
#[test]
fn a_number_past_the_ceiling_and_a_bad_port_are_said_as_that() {
    let dir = std::env::temp_dir().join(format!("zk-cli-e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    let ledger = dir.join("l");
    let l = ledger.to_str().expect("path");
    assert_eq!(run(&["init", "--ledger", l, "--key", KEY, "--statement", "s"], &[]).code, 0);
    let en = [("LANG", "en_US.UTF-8")];
    for value in ["9007199254740992", "18446744073709551615", "18446744073709551616"] {
        let r = run(&["succeed", "--ledger", l, "--key", KEY, "--to", "0x8888888888888888888888888888888888888888", "--kind", "rotation", "--effective", value, "--statement", "x"], &en);
        assert_eq!((r.code, first_line(&r), r.err.lines().nth(1).unwrap_or("")), (2, "E_ARGS --effective", "past the whole-number ceiling 9007199254740991"), "{value}");
        let r = run(&["grant", "--ledger", l, "--key", KEY, "--grantee", "0x3333333333333333333333333333333333333333", "--work", &format!("0x{}", "11".repeat(32)), "--terms", &format!("0x{}", "22".repeat(32)), "--window-from", "0", "--window-to", value], &en);
        assert_eq!((r.code, first_line(&r)), (2, "E_ARGS --window-to"), "{value}");
    }
    let r = run(&["succeed", "--ledger", l, "--key", KEY, "--to", "0x8888888888888888888888888888888888888888", "--kind", "rotation", "--effective", "-1", "--statement", "x"], &en);
    assert_eq!((r.code, r.err.lines().nth(1).unwrap_or("")), (2, "not an integer"), "not a number: its own words");
    for (url, said) in [("https://h:65536", "an endpoint's port must be a whole number from 0 to 65535"), ("https://h:", "an endpoint's port must be a whole number from 0 to 65535"), ("wss://h", "an endpoint is an http:// or https:// address")] {
        let r = run(&["anchor", "--endpoint", &format!("31337={url}"), "--key", KEY, "--form", "bare", "--hash", &format!("0x{}", "11".repeat(32))], &en);
        assert_eq!((r.code, r.err.lines().nth(1).unwrap_or("")), (2, said), "{url}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A misuse's first line never contains user-typed text: known flags by name, anything else by position and
/// length. Cases: `--key=…` where a flag is expected, alone and followed by a value; an unknown flag in both
/// spellings; unparsable values (`--hash`, `--form`, `--entry`); an unreadable path; a word where a flag belongs;
/// an unknown verb. The key, value and path appear on no stderr line.
#[test]
fn no_misuse_line_echoes_a_word_the_person_typed() {
    let secret = &KEY[2..];
    let key_eq = format!("--key={KEY}");
    let nosuch_eq = format!("--nosuch={KEY}");
    let hash_bad = format!("0x{secret}zz");
    let path = format!("/nowhere/{secret}");
    let entry_bad = format!("{secret}0");
    let cases: Vec<(Vec<&str>, String, &str)> = vec![
        (vec!["anchor", &key_eq], format!("E_ARGS #2 ({} bytes)", key_eq.len()), "this flag needs a value"),
        (vec!["anchor", &key_eq, "x"], format!("E_ARGS #2 ({} bytes)", key_eq.len()), "not among this verb's flags"),
        (vec!["history", &key_eq, "x"], format!("E_ARGS #2 ({} bytes)", key_eq.len()), "not among this verb's flags"),
        (vec!["annotate", &nosuch_eq, "x"], format!("E_ARGS #2 ({} bytes)", nosuch_eq.len()), "not among this verb's flags"),
        (vec!["annotate", "--nosuch", KEY], "E_ARGS #2 (8 bytes)".to_string(), "not among this verb's flags"),
        (vec!["contract", "--key", KEY], "E_ARGS --key".to_string(), "not among this verb's flags"),
        (vec!["anchor", "--endpoint", "1=http://127.0.0.1:1", "--key", KEY, "--form", secret, "--hash", "0x00"], format!("E_ARGS #7 ({} bytes)", secret.len()), "--form takes registry or bare"),
        (vec!["anchor", "--endpoint", "1=http://127.0.0.1:1", "--key", KEY, "--form", "bare", "--hash", &hash_bad], format!("E_ARGS #9 ({} bytes)", hash_bad.len()), "not hex"),
        (vec!["show", "--entry", &entry_bad, "--ledger", "x"], format!("E_ARGS #3 ({} bytes)", entry_bad.len()), "not 64 lowercase hex digits"),
        (vec!["badge", "--decode", &path], format!("E_UNREADABLE #3 ({} bytes)", path.len()), "cannot be read"),
        (vec!["annotate", secret], format!("E_ARGS #2 ({} bytes)", secret.len()), "not in the form --name"),
        (vec![secret], format!("E_ARGS #1 ({} bytes)", secret.len()), "not a verb"),
    ];
    for (args, first, second) in cases {
        let r = run(&args, &[]);
        assert_eq!((r.code, r.out.is_empty()), (2, true), "{args:?}: misuse, nothing on stdout: {}", r.err);
        assert_eq!(first_line(&r), first, "{args:?}: {}", r.err);
        assert!(r.err.lines().nth(1).is_some_and(|l| l.starts_with(second)), "{args:?}: {}", r.err);
        assert!(!r.err.contains(secret), "{args:?}: the word typed is said: {}", r.err);
    }
}
