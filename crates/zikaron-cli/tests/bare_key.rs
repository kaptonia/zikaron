use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use zikaron::json::Value;

#[path = "support/old_grant.rs"]
mod old_grant;

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
const WRITER: [u8; 32] = [0x21; 32];
const GRANTEE: [u8; 32] = [0x2a; 32];

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let p = std::env::temp_dir().join(format!("zk-cli-bare-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(p.join("home")).expect("a scratch place");
    Scratch(p)
}

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

fn key_file(dir: &Path, name: &str, key: &[u8; 32]) -> String {
    let p = dir.join(name);
    let mut o = zikaron_os::Options::new();
    o.write(true).create_new(true);
    zikaron_os::owner_only(&mut o).open(&p).expect("the file").write_all(zikaron::hexfmt::encode(key).as_bytes()).expect("its bytes");
    p.display().to_string()
}

fn ledger_with_grant(dir: &Path) -> (String, String, String) {
    let l = dir.join("ledger").display().to_string();
    let k = key_file(dir, "writer.key", &WRITER);
    let (c, out, err) = run(dir, &["init", "--ledger", &l, "--key-file", &k, "--statement", "start"]);
    assert_eq!(c, 0, "{out} {err}");
    let work = format!("0x{}", "c2".repeat(32));
    let grantee = zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(&GRANTEE).expect("an address"));
    let gid = old_grant::old_grant(Path::new(&l), &zikaron::hexfmt::encode(&WRITER), &grantee, &work, &format!("0x{}", "55".repeat(32)));
    (l, k, gid)
}

#[test]
fn a_grant_package_from_a_plain_folder_is_sealed_as_a_record_package_is() {
    let w = scratch("gp-to");
    let (l, _, gid) = ledger_with_grant(&w.0);
    let out = w.0.join("pkg");
    let o = out.display().to_string();
    let addr = zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(&GRANTEE).expect("an address"));
    let forms: Vec<(Vec<&str>, &str)> = vec![
        (vec!["--to", "0x12"], "E_ARGS --to"),
        (vec!["--to", &addr], "E_ARGS --endpoint"),
        (vec!["--to", &addr, "--endpoint", "31337=http://127.0.0.1:1"], "E_ARGS #"),
        (vec!["--endpoint", "11155111=http://127.0.0.1:1"], "E_ARGS --endpoint"),
        (vec!["--proxy", "none"], "E_ARGS --proxy"),
    ];
    for (more, first) in forms {
        let mut args = vec!["grant-package", "--ledger", &l, "--grant", &gid, "--out", &o];
        args.extend(more.iter().copied());
        let (c, so, e) = run(&w.0, &args);
        assert_eq!((c, so.as_str()), (2, ""), "{more:?}: {e}");
        assert!(e.starts_with(first), "{more:?}: {e}");
        assert!(!out.exists(), "{more:?}: nothing at --out");
    }
    let (c, so, e) = run(&w.0, &["grant-package", "--ledger", &l, "--grant", &gid, "--out", &o, "--to", &addr, "--endpoint", "11155111=http://127.0.0.1:1", "--proxy", "none"]);
    assert_eq!((c, reason_token(&so)), (4, (Some("E_ADDRESSED".into()), Some("ADDRESS_CHAIN_UNREAD".into()))), "{so} {e}");
    assert!(std::fs::read_dir(&out).map(|d| d.count() == 0).unwrap_or(true), "zero bytes");
    let (c, so, e) = run(&w.0, &["grant-package", "--ledger", &l, "--grant", &gid, "--out", &o]);
    assert_eq!((c, member(&so, "sealed")), (0, Value::Bool(false)), "{so} {e}");
}

#[test]
fn a_plain_folder_is_put_on_chain_with_its_key() {
    let w = scratch("anchor");
    let (l, k, _) = ledger_with_grant(&w.0);
    let node = "11155111=http://127.0.0.1:1";
    let forms: Vec<(Vec<&str>, &str)> = vec![
        (vec!["--key-file", &k, "--endpoint", node, "--form", "registry"], "E_ARGS --form"),
        (vec!["--key-file", &k, "--endpoint", node, "--registry", "0x0000000000000000000000000000000000000001"], "E_ARGS --registry"),
        (vec!["--key-file", &k, "--endpoint", node, "--hash", "0x00"], "E_ARGS --hash"),
        (vec!["--key-file", &k, "--endpoint", node, "--calldata", "0x00"], "E_ARGS --calldata"),
        (vec!["--key-file", &k], "E_ARGS --endpoint"),
        (vec!["--key-file", &k, "--endpoint", "31337=http://127.0.0.1:1"], "E_ARGS --endpoint"),
        (vec!["--key-file", &k, "--endpoint", node, "--endpoint", "1=http://127.0.0.1:1"], "E_ARGS --endpoint"),
        (vec!["--endpoint", node], "E_ARGS --key"),
    ];
    for (more, first) in forms {
        let mut args = vec!["anchor", "--ledger", &l];
        args.extend(more.iter().copied());
        let (c, so, e) = run(&w.0, &args);
        assert_eq!((c, so.as_str()), (2, ""), "{more:?}: {e}");
        assert!(e.starts_with(first), "{more:?}: {e}");
    }
    let (c, so, e) = run(&w.0, &["anchor", "--ledger", &l, "--key-file", &k, "--endpoint", node, "--proxy", "none"]);
    assert_eq!((c, member(&so, "reason")), (4, Value::Str("E_UNREACHABLE".into())), "{so} {e}");
    assert!(member(&so, "tx").as_str().is_none(), "nothing signed or sent: {so}");
}

#[test]
fn a_package_copy_behind_the_book_is_not_judged_from() {
    let w = scratch("gp-book");
    let (l, k, gid) = ledger_with_grant(&w.0);
    assert_eq!(run(&w.0, &["revoke", "--ledger", &l, "--key-file", &k, "--grant", &gid]).0, 0, "revoked by its entry");
    let pkg = w.0.join("pkg");
    let (c, so, e) = run(&w.0, &["grant-package", "--ledger", &l, "--grant", &gid, "--out", &pkg.display().to_string()]);
    assert_eq!(c, 0, "{so} {e}");
    let path = member(&so, "path").as_str().expect("the package").to_string();
    let (c, so, e) = run(&w.0, &["grant-package", "--package", &path]);
    assert_eq!((c, verdict(&so)), (1, Value::Str("FAIL".into())), "judged from the copy: {so} {e}");
    let me = zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(&WRITER).expect("an address"));
    let fragment = Value::Obj(vec![("anchors".into(), Value::Arr(Vec::new())), ("basis".into(), Value::Obj(vec![("chains".into(), Value::Arr(Vec::new()))]))]);
    let reading = Value::Obj(vec![("book".into(), Value::Arr(vec![Value::Obj(vec![("key".into(), Value::Str(format!("0x{}", "44".repeat(20)))), ("root".into(), Value::Str(me))])])), ("fragment".into(), fragment)]);
    std::fs::write(w.0.join("book-reading.json"), zikaron::json::canon_bytes(&reading)).expect("write");
    let bare = Value::Obj(vec![("fragment".into(), reading.member("fragment").cloned().expect("a fragment"))]);
    std::fs::write(w.0.join("bare-reading.json"), zikaron::json::canon_bytes(&bare)).expect("write");
    let (c, so, e) = run(&w.0, &["grant-package", "--package", &path, "--fragment", "bare-reading.json"]);
    assert_eq!((c, verdict(&so)), (1, Value::Str("FAIL".into())), "no book: judged from the copy: {so} {e}");
    for flag in ["--book", "--fragment"] {
        let (c, so, e) = run(&w.0, &["grant-package", "--package", &path, flag, "book-reading.json"]);
        assert_ne!(verdict(&so), Value::Str("FAIL".into()), "{flag}: behind the book, not judged from the copy: {so} {e}");
        assert_eq!(c, 3, "{flag}: {so} {e}");
    }
}

fn verdict(out: &str) -> Value {
    member(out, "check").member("verdict").cloned().unwrap_or(Value::Null)
}
