use std::path::{Path, PathBuf};
use std::process::Command;
use zikaron::json::Value;

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
#[path = "support/old_grant.rs"]
mod old_grant;

const KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

fn scratch(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("zk-one-rule-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("scratch");
    p
}

fn run(dir: &Path, args: &[&str]) -> (i32, Value) {
    let o = Command::new(BIN).args(args).current_dir(dir).env(zikaron_os::HOME_VAR, dir.join("user")).output().expect("zikaron");
    (o.status.code().unwrap_or(-1), zikaron::json::parse(&o.stdout).unwrap_or(Value::Null))
}

fn text(v: &Value, k: &str) -> String {
    v.member(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn h32(b: u8) -> String {
    format!("0x{}", format!("{b:02x}").repeat(32))
}

fn entries(dir: &Path) -> usize {
    std::fs::read_dir(dir.join("book")).map(|d| d.count()).unwrap_or(0)
}

fn ledger(tag: &str) -> (PathBuf, String) {
    let d = scratch(tag);
    assert_eq!(run(&d, &["init", "--ledger", "book", "--key", KEY, "--statement", "开端"]).0, 0);
    let (code, v) = run(&d, &["history", "--ledger", "book", "--key", KEY, "--content", &h32(0xc1), "--mark", "v1", "--toolchain", &h32(0x0b)]);
    assert_eq!(code, 0, "{v:?}");
    let id = text(&v, "entryId");
    (d, id)
}

#[test]
fn a_grant_of_a_deleted_work_is_withheld() {
    let (d, h) = ledger("grant-deleted");
    let grant = |d: &Path| run(d, &["grant", "--ledger", "book", "--key", KEY, "--grantee", &format!("0x{}", "33".repeat(20)), "--work", &h32(0xc1), "--terms", &h32(0x55)]);
    assert_eq!(run(&d, &["retract", "--ledger", "book", "--key", KEY, "--subject", &h]).0, 0);
    let before = entries(&d);
    let (code, v) = grant(&d);
    assert_eq!((code, text(&v, "reason"), text(&v, "token")), (1, "E_GRANT_WITHHELD".to_string(), "E_GRANT_WORK_DELETED".to_string()), "{v:?}");
    assert_eq!(entries(&d), before, "nothing written");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_revocation_names_a_grant_of_this_ledger() {
    let (d, h) = ledger("revoke");
    let before = entries(&d);
    let (code, v) = run(&d, &["revoke", "--ledger", "book", "--key", KEY, "--grant", &h]);
    assert_eq!((code, text(&v, "reason"), text(&v, "token")), (1, "E_REVOCATION".to_string(), "E_REVOKE_NOT_A_GRANT".to_string()), "{v:?}");
    let (code, v) = run(&d, &["revoke", "--ledger", "book", "--key", KEY, "--grant", &h32(0x99)]);
    assert_eq!((code, text(&v, "token")), (1, "E_REVOKE_NOT_IN_LEDGER".to_string()), "{v:?}");
    assert_eq!(entries(&d), before, "nothing written");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn an_annotation_names_an_entry_of_this_ledger() {
    let (d, h) = ledger("annotate");
    let before = entries(&d);
    let (code, v) = run(&d, &["annotate", "--ledger", "book", "--key", KEY, "--subject", &h32(0x99), "--note", "n"]);
    assert_eq!((code, text(&v, "reason"), text(&v, "token")), (1, "E_ANNOTATION".to_string(), "E_NAME_NOT_IN_LEDGER".to_string()), "{v:?}");
    assert_eq!(entries(&d), before, "nothing written");
    assert_eq!(run(&d, &["annotate", "--ledger", "book", "--key", KEY, "--subject", &h, "--note", "n"]).0, 0, "an entry of this ledger");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn succeed_is_no_verb_of_this_command_line() {
    let (d, _) = ledger("succeed");
    let before = entries(&d);
    let to = format!("0x{}", "88".repeat(20));
    let plain = Command::new(BIN).current_dir(&d).args(["succeed", "--ledger", "book", "--key", KEY, "--to", &to, "--kind", "handover", "--effective", "5", "--statement", "s"]).output().expect("runs");
    let homed = Command::new(BIN).current_dir(&d).args(["succeed", "--home", "book"]).output().expect("runs");
    for (road, o) in [("plain", &plain), ("--home", &homed)] {
        assert_eq!((o.status.code(), o.stdout.is_empty()), (Some(2), true), "{road}: {}", String::from_utf8_lossy(&o.stderr));
    }
    assert_eq!(entries(&d), before, "nothing written");
    assert!(zikaron_cli::args::usage().contains("|migrate|") && !zikaron_cli::args::usage().contains("succeed"), "the usage line");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_copy_behind_the_book_is_not_judged_from() {
    let (d, _) = ledger("book");
    let gid = old_grant::old_grant(&d.join("book"), KEY, &format!("0x{}", "33".repeat(20)), &h32(0xc2), &h32(0x55));
    assert_eq!(run(&d, &["revoke", "--ledger", "book", "--key", KEY, "--grant", &gid]).0, 0);
    let file = format!("book/{}.entry", gid.trim_start_matches("0x"));
    let (code, v) = run(&d, &["check-grant", "--grant", &file, "--ledger", "book"]);
    assert_eq!((code, text(&v, "verdict")), (1, "FAIL".to_string()), "judged from the copy: {v:?}");
    let me = zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(&zikaron::hexfmt::decode(KEY).and_then(|b| <[u8; 32]>::try_from(b).ok()).expect("key")).expect("addr"));
    let reading = Value::Obj(vec![("book".into(), Value::Arr(vec![Value::Obj(vec![("key".into(), Value::Str(format!("0x{}", "44".repeat(20)))), ("root".into(), Value::Str(me))])]))]);
    std::fs::write(d.join("book-reading.json"), zikaron::json::canon_bytes(&reading)).expect("write");
    let (code, v) = run(&d, &["check-grant", "--grant", &file, "--ledger", "book", "--book", "book-reading.json"]);
    assert_ne!(text(&v, "verdict"), "FAIL", "behind the book, not judged from the copy: {v:?}");
    assert_eq!(code, 3, "{v:?}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn a_write_tools_home_is_named_by_the_table() {
    fn told(_: &str) -> &'static str {
        "t"
    }
    static OTHER: zikaron_cli::mcp::Table = zikaron_cli::mcp::Table {
        name: "other",
        not_writes: &["init"],
        reads: &[],
        writes_too: &[],
        home_optional: &[],
        as_home: &[],
        paths: &[("", "home")],
        joined: &[],
        sends: &[],
        door_reads: &[],
        alone_home: ("data", ""),
        alone_more: &["--identity", "main"],
        told,
    };
    let home = std::env::temp_dir().join("h");
    let given = vec![("home".to_string(), Value::Str(home.display().to_string())), ("note".to_string(), Value::Str("n".into()))];
    let other = zikaron_cli::mcp::argv_of(&OTHER, "annotate", &given, true).expect("a tool").expect("an argv");
    assert_eq!(other, ["annotate", "--data", &home.display().to_string(), "--identity", "main", "--note", "n"]);
    let own = zikaron_cli::mcp::argv_of(&zikaron_cli::mcptools::TABLE, "annotate", &given, true).expect("a tool").expect("an argv");
    assert_eq!(own, ["annotate", "--ledger", &home.join("ledger").display().to_string(), "--note", "n"]);
}
