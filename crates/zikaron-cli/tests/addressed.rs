//! A record package addressed to one key on the real binary, the direct road: `kit-export --to-key` seals a
//! plain ledger folder's package to a key given, `--to` looks for an address's key on the networks `--endpoint`
//! gives; `kit-verify --key-file` opens it, verifies it and takes its originals out; anything else is refused by
//! name, the misuse forms with zero bytes on stdout and nothing at `--out`.
//! Victims: the sender (a package anyone could open, or one sealed to no one), the recipient (a package refused for
//! the wrong reason, or bytes not those sealed), whoever scripts the command line (an exit that does not say whether
//! the chain answered).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use zikaron::json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
/// The ledger's key, and the recipient's (the key file opens the package).
const WRITER: [u8; 32] = [0x21; 32];
const RECIPIENT: [u8; 32] = [0x2a; 32];
const STRANGER: [u8; 32] = [0x3b; 32];

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let p = std::env::temp_dir().join(format!("zk-cli-addressed-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(p.join("home")).expect("a scratch place");
    Scratch(p)
}

/// One run in `dir` (its own user home): exit code, stdout, the first two lines of stderr.
fn run(dir: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env(zikaron_os::HOME_VAR, dir.join("home"))
        .env("LANG", "en_US.UTF-8")
        .env("ZKA_TIMEOUT_SECS", "5")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("LANGUAGE")
        .output()
        .expect("runs");
    let err = String::from_utf8_lossy(&o.stderr).lines().take(2).collect::<Vec<_>>().join("\n");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), err)
}

fn member(out: &str, k: &str) -> Value {
    zikaron::json::parse(out.as_bytes()).ok().and_then(|v| v.member(k).cloned()).unwrap_or(Value::Null)
}

fn reason_token(out: &str) -> (Option<String>, Option<String>) {
    (member(out, "reason").as_str().map(str::to_string), member(out, "token").as_str().map(str::to_string))
}

/// A key file, created owner only through `zikaron_os` (the platform calls live in the platform crate).
fn key_file(dir: &Path, name: &str, key: &[u8; 32]) -> String {
    let p = dir.join(name);
    let mut o = zikaron_os::Options::new();
    o.write(true).create_new(true);
    zikaron_os::owner_only(&mut o).open(&p).expect("the file").write_all(zikaron::hexfmt::encode(key).as_bytes()).expect("its bytes");
    p.display().to_string()
}

fn public(key: &[u8; 32]) -> [u8; 33] {
    zikaron_seal::crypto::public_of(key).expect("a key")
}

/// A plain ledger folder with a genesis and one record of `orig`, written by the command line itself.
fn ledger(dir: &Path, orig: &Path) -> String {
    let l = dir.join("ledger").display().to_string();
    let k = key_file(dir, "writer.key", &WRITER);
    let (c, out, err) = run(dir, &["init", "--ledger", &l, "--key-file", &k, "--statement", "start"]);
    assert_eq!(c, 0, "{out} {err}");
    let (c, out, err) = run(dir, &["history", "--ledger", &l, "--key-file", &k, "--file", &orig.display().to_string(), "--note", "for them"]);
    assert_eq!(c, 0, "{out} {err}");
    l
}

/// The tests that count the opened folders left in the system's temporary place take turns, so one test's open
/// package is not counted as another's leftover.
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

fn opened_rooms() -> usize {
    std::fs::read_dir(std::env::temp_dir()).map(|d| d.filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().starts_with("zikaron-opened-")).count()).unwrap_or(0)
}

/// Sealed to a key given, the package holds the format's mark and none of the recipient's bytes; the recipient's key
/// file opens it, verifies its records and takes the original out, the very bytes recorded; the answer is the plain
/// package's (exit 3 with no chain read).
#[test]
fn a_package_sealed_to_a_key_given_opens_with_that_key_alone() {
    let _turn = turn();
    let w = scratch("round");
    let orig = w.0.join("for-them.txt");
    std::fs::write(&orig, "只给收件人".repeat(20_000)).expect("w");
    let l = ledger(&w.0, &orig);
    let out = w.0.join("sealed.zkit");
    let pubk = zikaron::hexfmt::encode(&public(&RECIPIENT));
    let file = format!("for-them.txt={}", orig.display());
    let (c, o, e) = run(&w.0, &["kit-export", "--ledger", &l, "--out", &out.display().to_string(), "--to-key", &pubk, "--file", &file]);
    assert_eq!(c, 0, "{o} {e}");
    assert_eq!(member(&o, "path").as_str(), Some(out.display().to_string().as_str()));
    let bytes = std::fs::read(&out).expect("landed");
    assert!(bytes.starts_with(zikaron_glue::addressed::MAGIC), "the format's mark");
    let addr = zikaron::cryptox::address_of_privkey(&RECIPIENT).expect("an address");
    let full = zikaron_seal::crypto::uncompressed(&public(&RECIPIENT)).expect("a point");
    for needle in [&addr[..], &public(&RECIPIENT)[..], &full[1..]] {
        assert!(!bytes.windows(needle.len()).any(|x| x == needle), "no recipient bytes in the package");
    }
    // The uncompressed form of the same key seals too.
    let (c, o, e) = run(&w.0, &["kit-export", "--ledger", &l, "--out", &w.0.join("full.zkit").display().to_string(), "--to-key", &zikaron::hexfmt::encode(&full)]);
    assert_eq!(c, 0, "{o} {e}");
    // Opened with the recipient's key file, its original taken out.
    let mine = key_file(&w.0, "recipient.key", &RECIPIENT);
    let to = w.0.join("taken");
    std::fs::create_dir_all(&to).expect("a folder");
    let rooms = opened_rooms();
    let (c, o, e) = run(&w.0, &["kit-verify", "--kit", &out.display().to_string(), "--key-file", &mine, "--originals-to", &to.display().to_string()]);
    assert_eq!(c, 3, "{o} {e}");
    assert!(o.contains("\"original\":\"match\""), "{o}");
    assert_eq!(std::fs::read(to.join("for-them.txt")).expect("taken"), std::fs::read(&orig).expect("there"), "the very bytes recorded");
    assert_eq!(opened_rooms(), rooms, "nothing opened is left behind");
    // The same package verified without taking: the same records' answer.
    let (c2, o2, _) = run(&w.0, &["kit-verify", "--kit", &out.display().to_string(), "--key-file", &mine]);
    assert_eq!((c2, member(&o2, "records")), (3, member(&o, "records")));
}

/// No key offered opens it: a stranger's key is refused `E_ADDRESSED` (`ADDRESSED_NOT_RECIPIENT`, exit 1) in the one
/// table's words, naming no recipient; a segment changed after the first is `ADDRESSED_BROKEN`; no key given at all is
/// misuse (exit 2, zero bytes); a reading verb meets it by name (`ADDRESSED_ELSEWHERE`). Nothing opened stays.
#[test]
fn whoever_is_not_the_recipient_is_refused_by_name_and_nothing_stays() {
    let _turn = turn();
    let w = scratch("refused");
    let orig = w.0.join("o.txt");
    std::fs::write(&orig, vec![7u8; 150_000]).expect("w");
    let l = ledger(&w.0, &orig);
    let out = w.0.join("s.zkit");
    let file = format!("o.txt={}", orig.display());
    let (c, o, e) = run(&w.0, &["kit-export", "--ledger", &l, "--out", &out.display().to_string(), "--to-key", &zikaron::hexfmt::encode(&public(&RECIPIENT)), "--file", &file]);
    assert_eq!(c, 0, "{o} {e}");
    let s = out.display().to_string();
    let rooms = opened_rooms();
    let stranger = key_file(&w.0, "stranger.key", &STRANGER);
    let (c, o, e) = run(&w.0, &["kit-verify", "--kit", &s, "--key-file", &stranger]);
    assert_eq!((c, reason_token(&o)), (1, (Some("E_ADDRESSED".into()), Some("ADDRESSED_NOT_RECIPIENT".into()))), "{o}");
    assert_eq!(e, zikaron_seal::addressed::words::NOT_RECIPIENT.1, "the one table's words");
    // The last segment changed.
    let mut bytes = std::fs::read(&out).expect("there");
    let n = bytes.len();
    bytes[n - 3] ^= 1;
    let broken = w.0.join("broken.zkit");
    std::fs::write(&broken, &bytes).expect("w");
    let mine = key_file(&w.0, "mine.key", &RECIPIENT);
    let (c, o, _) = run(&w.0, &["kit-verify", "--kit", &broken.display().to_string(), "--key-file", &mine]);
    assert_eq!((c, reason_token(&o)), (1, (Some("E_ADDRESSED".into()), Some("ADDRESSED_BROKEN".into()))), "{o}");
    assert_eq!(opened_rooms(), rooms, "no plaintext left behind");
    // No key at all.
    let (c, o, e) = run(&w.0, &["kit-verify", "--kit", &s]);
    assert_eq!((c, o.as_str()), (2, ""), "misuse writes nothing");
    assert!(e.starts_with("E_ARGS --key --ledger --home"), "{e}");
    // A reading verb.
    let (c, o, _) = run(&w.0, &["audit", "--ledger", &s]);
    assert_eq!((c, reason_token(&o)), (1, (Some("E_ADDRESSED".into()), Some("ADDRESSED_ELSEWHERE".into()))), "{o}");
    // `--ledger` that is no home's.
    let (c, o, e) = run(&w.0, &["kit-verify", "--kit", &s, "--ledger", &l]);
    assert_eq!((c, o.as_str()), (2, ""), "{e}");
    assert!(e.starts_with("E_ARGS --ledger"), "{e}");
}

/// Misuse, judged before anything is read or asked: both ways to name the recipient, either with `--as folder`, a key
/// off the curve, an address not in its form, `--to` with no `--endpoint`, a chain the known deployments do not name,
/// `--endpoint` or `--proxy` without `--to`. Each exits 2 with zero bytes on stdout and nothing at `--out`.
#[test]
fn misuse_of_the_recipient_writes_nothing() {
    let w = scratch("misuse");
    let orig = w.0.join("o.txt");
    std::fs::write(&orig, b"x").expect("w");
    let l = ledger(&w.0, &orig);
    let out = w.0.join("m.zkit");
    let o = out.display().to_string();
    let pubk = zikaron::hexfmt::encode(&public(&RECIPIENT));
    let addr = zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(&RECIPIENT).expect("an address"));
    let off = format!("0x02{}", "00".repeat(32));
    let forms: Vec<(Vec<&str>, &str)> = vec![
        (vec!["--to", &addr, "--to-key", &pubk], "E_ARGS --to --to-key"),
        (vec!["--to-key", &pubk, "--as", "folder"], "E_ARGS --as --to-key"),
        (vec!["--to", &addr, "--as", "folder"], "E_ARGS --as --to"),
        (vec!["--to-key", &off], "E_ARGS --to-key"),
        (vec!["--to", "0x12"], "E_ARGS --to"),
        (vec!["--to", &addr], "E_ARGS --endpoint"),
        (vec!["--to", &addr, "--endpoint", "31337=http://127.0.0.1:1"], "E_ARGS #"),
        (vec!["--endpoint", "11155111=http://127.0.0.1:1"], "E_ARGS --endpoint"),
        (vec!["--to-key", &pubk, "--proxy", "none"], "E_ARGS --proxy"),
    ];
    for (more, first) in forms {
        let mut args = vec!["kit-export", "--ledger", &l, "--out", &o];
        args.extend(more.iter().copied());
        let (c, so, e) = run(&w.0, &args);
        assert_eq!((c, so.as_str()), (2, ""), "{more:?}: {e}");
        assert!(e.starts_with(first), "{more:?}: {e}");
        assert!(!out.exists(), "{more:?}: nothing at --out");
    }
    // Both ways: the words of the addressed package's one table.
    let (_, _, e) = run(&w.0, &["kit-export", "--ledger", &l, "--out", &o, "--to", &addr, "--to-key", &pubk]);
    assert!(e.ends_with(zikaron_seal::addressed::words::BOTH.1), "{e}");
}

/// An address's key looked for on a network whose nodes do not answer: the chain was not read, which is no answer
/// (`E_ADDRESSED` `ADDRESS_CHAIN_UNREAD`, exit 4), never "not anchored"; zero bytes at `--out`.
#[test]
fn a_chain_not_read_is_no_answer_and_nothing_is_written() {
    let w = scratch("unread");
    let orig = w.0.join("o.txt");
    std::fs::write(&orig, b"x").expect("w");
    let l = ledger(&w.0, &orig);
    let out = w.0.join("u.zkit");
    let addr = zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(&RECIPIENT).expect("an address"));
    let (c, o, e) = run(&w.0, &["kit-export", "--ledger", &l, "--out", &out.display().to_string(), "--to", &addr, "--endpoint", "11155111=http://127.0.0.1:1", "--proxy", "none"]);
    assert_eq!((c, reason_token(&o)), (4, (Some("E_ADDRESSED".into()), Some("ADDRESS_CHAIN_UNREAD".into()))), "{o} {e}");
    assert_eq!(e, zikaron_seal::addressed::words::CHAIN_UNREAD.1);
    assert!(!out.exists(), "zero bytes");
}
