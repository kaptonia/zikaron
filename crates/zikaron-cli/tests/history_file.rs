//! `history --file` fills the history entry by the same recording convention the app uses
//! (`zikaron_glue::recording`): `content` is the sha256 of the file's bytes, `mode` the family mark with the
//! mark's sha256 as its toolchain. Cases: bytes, empty file, the same bytes under another name, a missing path,
//! a directory, and `--file` combined with the flags it fills.

use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
const KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

fn run(dir: &Path, args: &[&str]) -> (i32, String) {
    let o = Command::new(BIN).args(args).current_dir(dir).output().expect("runs");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned())
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("zk-cli-history-file-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("a scratch place");
    p
}

fn member<'a>(v: &'a zikaron::json::Value, path: &[&str]) -> Option<&'a zikaron::json::Value> {
    path.iter().try_fold(v, |at, k| at.member(k))
}

/// The body of the entry `history --file <name>` wrote, read back by `show`.
fn written(dir: &Path, name: &str) -> (i32, Option<zikaron::json::Value>) {
    let (code, out) = run(dir, &["history", "--ledger", "b", "--key", KEY, "--file", name]);
    if code != 0 {
        return (code, None);
    }
    let id = zikaron::json::parse(out.as_bytes()).ok().and_then(|v| v.member("entryId").and_then(|x| x.as_str().map(str::to_string))).unwrap_or_default();
    let (_, shown) = run(dir, &["show", "--ledger", "b", "--entry", &id]);
    let v = zikaron::json::parse(shown.as_bytes()).ok();
    (code, v.as_ref().and_then(|v| member(v, &["value", "body"]).cloned()))
}

fn text<'a>(body: &'a zikaron::json::Value, path: &[&str]) -> &'a str {
    member(body, path).and_then(|x| x.as_str()).unwrap_or("")
}

#[test]
fn a_files_content_is_the_sha256_of_its_bytes_and_its_mode_the_family_mark() {
    let w = scratch("convention");
    assert_eq!(run(&w, &["init", "--ledger", "b", "--key", KEY, "--statement", "开端"]).0, 0);
    let mark = zikaron_glue::recording::FAMILY;
    let toolchain = zikaron::hexfmt::encode(&zikaron::cryptox::sha256(mark.as_bytes()));
    for (form, name, bytes) in [("bytes", "work.bin", b"a file of bytes".to_vec()), ("noBytes", "empty.bin", Vec::new()), ("theSameBytesUnderAnotherName", "copy.bin", b"a file of bytes".to_vec())] {
        std::fs::write(w.join(name), &bytes).expect("the file");
        let (code, body) = written(&w, name);
        let body = body.unwrap_or_else(|| panic!("{form}: exit {code}, nothing written"));
        assert_eq!(text(&body, &["content"]), zikaron::hexfmt::encode(&zikaron::cryptox::sha256(&bytes)), "{form}: the sha256 of its bytes");
        assert_eq!((text(&body, &["mode", "mark"]), text(&body, &["mode", "toolchain"])), (mark, toolchain.as_str()), "{form}: the family mark");
    }
    let _ = std::fs::remove_dir_all(&w);
}

#[test]
fn a_file_that_cannot_be_read_and_the_flags_it_fills_are_misuse() {
    let w = scratch("misuse");
    assert_eq!(run(&w, &["init", "--ledger", "b", "--key", KEY, "--statement", "开端"]).0, 0);
    std::fs::create_dir_all(w.join("a-directory")).expect("a directory");
    std::fs::write(w.join("work.bin"), b"bytes").expect("the file");
    let h32 = format!("0x{}", "ab".repeat(32));
    for (form, args) in [
        ("notThere", vec!["history", "--ledger", "b", "--key", KEY, "--file", "nowhere.bin"]),
        ("aDirectory", vec!["history", "--ledger", "b", "--key", KEY, "--file", "a-directory"]),
        ("withContent", vec!["history", "--ledger", "b", "--key", KEY, "--file", "work.bin", "--content", &h32]),
        ("withMark", vec!["history", "--ledger", "b", "--key", KEY, "--file", "work.bin", "--mark", "x"]),
        ("withToolchain", vec!["history", "--ledger", "b", "--key", KEY, "--file", "work.bin", "--toolchain", &h32]),
    ] {
        let (code, out) = run(&w, &args);
        assert_eq!((code, out.is_empty()), (2, true), "{form}: misuse, not one byte on stdout");
    }
    let _ = std::fs::remove_dir_all(&w);
}
