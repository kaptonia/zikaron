//! Record packages in one file, `kit-verify`, and handing a ledger between two places, on the real binary. Each
//! place is its own user home (its own machine folder), as two machines are.

use std::path::{Path, PathBuf};
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
const KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let p = std::env::temp_dir().join(format!("zk-cli-handoff-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("a scratch place");
    Scratch(p)
}

/// One run: exit code, stdout, the first line of stderr. `home` is the user home (the place's machine).
fn run(dir: &Path, home: &Path, args: &[&str]) -> (i32, String, String) {
    let o = Command::new(BIN)
        .args(args)
        .current_dir(dir)
        .env(zikaron_os::HOME_VAR, home)
        .env("LANG", "en_US.UTF-8")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("LANGUAGE")
        .output()
        .expect("runs");
    let err = String::from_utf8_lossy(&o.stderr).lines().next().unwrap_or_default().to_string();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).into_owned(), err)
}

fn member(out: &str, k: &str) -> String {
    let v = zikaron::json::parse(out.as_bytes()).unwrap_or(zikaron::json::Value::Null);
    match v.member(k) {
        Some(zikaron::json::Value::Str(s)) => s.clone(),
        Some(zikaron::json::Value::Int(n)) => n.to_string(),
        Some(other) => format!("{other:?}"),
        None => String::new(),
    }
}

/// A ledger at `dir/name` with its root and one record.
fn a_ledger(dir: &Path, home: &Path, name: &str) {
    assert_eq!(run(dir, home, &["init", "--ledger", name, "--key", KEY, "--statement", "start"]).0, 0);
    std::fs::write(dir.join("w.txt"), b"a work").expect("a work");
    assert_eq!(run(dir, home, &["history", "--ledger", name, "--key", KEY, "--file", "w.txt"]).0, 0);
}

fn write(dir: &Path, home: &Path, name: &str, note: &str) -> (i32, String, String) {
    run(dir, home, &["annotate", "--ledger", name, "--key", KEY, "--note", note])
}

/// The pen goes with the ledger: a place that handed it off does not write; the other place takes only a follow-on
/// and then writes; the first takes back only the handoff after its own; handing off again makes the same handoff.
#[test]
fn a_ledger_goes_to_the_other_place_and_back_one_writer_at_a_time() {
    let w = scratch("pen");
    let (ha, hb) = (w.0.join("home-a"), w.0.join("home-b"));
    a_ledger(&w.0, &ha, "a");
    let (code, out, _) = run(&w.0, &ha, &["hand-over", "--ledger", "a", "--out", "h1.zkhand"]);
    assert_eq!(code, 0, "{out}");
    let h1 = member(&out, "handoff");
    assert_eq!(member(&out, "entries"), "2");
    assert!(zikaron_glue::container::is_container_at(&w.0.join("h1.zkhand")), "one file");
    // Handed: this place does not write; handing off again is the same handoff.
    let (code, out, err) = write(&w.0, &ha, "a", "after handing");
    assert_eq!((code, member(&out, "reason"), member(&out, "handoff")), (1, "E_HANDED_OFF".into(), h1.clone()), "{err}");
    let (code, out, _) = run(&w.0, &ha, &["hand-over", "--ledger", "a", "--out", "h1-again.zkhand"]);
    assert_eq!((code, member(&out, "handoff")), (0, h1.clone()), "the same handoff");
    // The other place (another machine) takes it: a folder not there yet is made, every entry written.
    let (code, out, _) = run(&w.0, &hb, &["take-over", "--ledger", "b", "--handoff", "h1.zkhand"]);
    assert_eq!((code, member(&out, "count"), member(&out, "handoff")), (0, "2".into(), h1.clone()), "{out}");
    assert_eq!(write(&w.0, &hb, "b", "on b").0, 0, "the pen is at b");
    let (code, out, _) = run(&w.0, &hb, &["hand-over", "--ledger", "b", "--out", "h2.zkhand"]);
    assert_eq!(code, 0, "{out}");
    let h2 = member(&out, "handoff");
    assert_ne!(h1, h2);
    // a takes back only the handoff made after its own; its own handoff fed back is refused.
    let (code, out, _) = run(&w.0, &ha, &["take-over", "--ledger", "a", "--handoff", "h1.zkhand"]);
    assert_eq!((code, member(&out, "reason"), member(&out, "token"), member(&out, "handoff")), (1, "E_HANDOFF".into(), "NOT_BACK".into(), h1.clone()));
    let before = std::fs::read_dir(w.0.join("a")).expect("a").count();
    let (code, out, _) = run(&w.0, &ha, &["take-over", "--ledger", "a", "--handoff", "h2.zkhand"]);
    assert_eq!((code, member(&out, "count")), (0, "1".into()), "{out}");
    assert_eq!(std::fs::read_dir(w.0.join("a")).expect("a").count(), before + 1, "the one entry a lacked");
    assert_eq!(write(&w.0, &ha, "a", "back at a").0, 0, "a writes again");
    // b handed it: b does not write, and its own handoff is not taken back by b.
    assert_eq!(member(&write(&w.0, &hb, "b", "b again").1, "reason"), "E_HANDED_OFF");
    assert_eq!(member(&run(&w.0, &hb, &["take-over", "--ledger", "b", "--handoff", "h2.zkhand"]).1, "token"), "NOT_BACK");
    // a, holding an entry h2 lacks, does not take h2 again: it would lose that entry.
    let (code, out, _) = run(&w.0, &ha, &["take-over", "--ledger", "a", "--handoff", "h2.zkhand"]);
    assert_eq!((code, member(&out, "token")), (1, "AHEAD".into()), "{out}");
    assert!(!member(&out, "entryId").is_empty(), "names the entry");
}

/// What does not read as a handoff, and a handoff whose ledger is not one line, are refused by name; nothing is
/// written.
#[test]
fn a_file_that_is_no_handoff_or_not_one_line_is_refused_by_name() {
    let w = scratch("refused");
    let h = w.0.join("home");
    a_ledger(&w.0, &h, "a");
    std::fs::write(w.0.join("plain.txt"), b"not a handoff").expect("a file");
    let (code, out, _) = run(&w.0, &h, &["take-over", "--ledger", "a", "--handoff", "plain.txt"]);
    assert_eq!((code, member(&out, "reason"), member(&out, "token")), (1, "E_HANDOFF".into(), "E_FILE_MAGIC".into()));
    // A record package in one file, but no handoff note in it.
    let (code, _, _) = run(&w.0, &h, &["kit-export", "--ledger", "a", "--out", "kit.zkit", "--note", ""]);
    assert_eq!(code, 0);
    let (code, out, _) = run(&w.0, &h, &["take-over", "--ledger", "a", "--handoff", "kit.zkit"]);
    assert_eq!((code, member(&out, "token")), (1, "E_HANDOFF_NOTE".into()));
    // Two entries at the same position: written into two folders by hand, laid into one handoff file.
    std::fs::create_dir_all(w.0.join("y")).expect("y");
    let pile = |d: &str| -> Vec<Vec<u8>> {
        let mut v: Vec<Vec<u8>> = std::fs::read_dir(w.0.join(d)).expect("dir").filter_map(|e| e.ok()).map(|e| std::fs::read(e.path()).expect("entry")).collect();
        v.sort();
        v
    };
    let genesis = pile("a").into_iter().find(|b| zikaron::entry::check(b).map(|e| e.seq == 0).unwrap_or(false)).expect("genesis");
    let gid = zikaron::hexfmt::encode(&zikaron::entry::entry_id(&genesis));
    std::fs::write(w.0.join("y").join(format!("{}.entry", gid.trim_start_matches("0x"))), &genesis).expect("genesis copied");
    assert_eq!(run(&w.0, &h, &["annotate", "--ledger", "y", "--key", KEY, "--note", "other one"]).0, 0);
    let mut both = pile("a");
    both.extend(pile("y"));
    both.sort();
    both.dedup();
    let note = zikaron_seal::handover::Note { id: zikaron_seal::handover::HandId([9; 16]), after: None };
    let bundle = zikaron_glue::pack::Bundle { entries: both, files: vec![(zikaron_glue::names::HANDOFF_NOTE.into(), note.bytes())], note: "fork".into(), ..Default::default() };
    zikaron_glue::pack::export_as(&w.0.join("fork.zkhand"), bundle, zikaron_glue::pack::Form::File).ok().expect("laid");
    let before = std::fs::read_dir(w.0.join("a")).expect("a").count();
    let (code, out, _) = run(&w.0, &h, &["take-over", "--ledger", "a", "--handoff", "fork.zkhand"]);
    assert_eq!((code, member(&out, "token")), (1, "NOT_ONE_LINE".into()), "{out}");
    assert_eq!(std::fs::read_dir(w.0.join("a")).expect("a").count(), before, "nothing written");
    // A folder holding the fork is not handed off either.
    std::fs::create_dir_all(w.0.join("z")).expect("z");
    for d in ["a", "y"] {
        for e in std::fs::read_dir(w.0.join(d)).expect("dir").filter_map(|e| e.ok()) {
            let _ = std::fs::copy(e.path(), w.0.join("z").join(e.file_name()));
        }
    }
    let (code, out, _) = run(&w.0, &h, &["hand-over", "--ledger", "z", "--out", "z.zkhand"]);
    assert_eq!((code, member(&out, "reason")), (1, "E_TIP_FORKED".into()), "{out}");
    assert!(!w.0.join("z.zkhand").exists());
}

/// `kit-export` writes one file unless a folder is asked for; the readers take both; `kit-verify` answers each
/// record (its original, its anchoring) and refuses what the kit law refuses.
#[test]
fn a_record_package_is_one_file_unless_a_folder_is_asked_and_both_read_back() {
    let w = scratch("kit");
    let h = w.0.join("home");
    a_ledger(&w.0, &h, "a");
    let (code, out, _) = run(&w.0, &h, &["kit-export", "--ledger", "a", "--out", "k.zkit", "--note", "", "--file", "w.txt=w.txt"]);
    assert_eq!(code, 0, "{out}");
    assert!(w.0.join("k.zkit").is_file() && zikaron_glue::container::is_container_at(&w.0.join("k.zkit")), "one file");
    let (code, out, _) = run(&w.0, &h, &["kit-export", "--ledger", "a", "--out", "k-dir", "--note", "", "--as", "folder"]);
    assert_eq!(code, 0, "{out}");
    assert!(w.0.join("k-dir").join(zikaron_glue::names::MANIFEST).is_file(), "a folder");
    let (code, _, err) = run(&w.0, &h, &["kit-export", "--ledger", "a", "--out", "k2", "--note", "", "--as", "box"]);
    assert_eq!((code, err.as_str()), (2, "E_ARGS --as"));
    // The readers take the file: the audit, an entry shown from it.
    let (code, out, _) = run(&w.0, &h, &["audit", "--ledger", "k.zkit"]);
    assert!(code == 0 || code == 3, "{code} {out}");
    // `kit-verify`: the record's original is the one recorded; no chain read, so not anchored is never said.
    let (code, out, _) = run(&w.0, &h, &["kit-verify", "--kit", "k.zkit"]);
    assert_eq!((code, member(&out, "reason")), (3, "E_RECORDS".into()), "{out}");
    assert!(out.contains("\"original\":\"match\"") && out.contains("\"anchored\":\"unread\""), "{out}");
    let (code, out, _) = run(&w.0, &h, &["kit-verify", "--kit", "k-dir"]);
    assert_eq!(code, 3);
    assert!(out.contains("\"original\":\"missing\""), "the folder carries no original: {out}");
    // A file cut short, and a folder that is no kit: refused by name.
    let raw = std::fs::read(w.0.join("k.zkit")).expect("read");
    std::fs::write(w.0.join("cut.zkit"), &raw[..raw.len() - 4]).expect("cut");
    let (code, out, _) = run(&w.0, &h, &["kit-verify", "--kit", "cut.zkit"]);
    assert_eq!((code, member(&out, "reason"), member(&out, "state")), (1, "E_KIT".into(), "E_FILE_TRUNCATED".into()), "{out}");
    let (code, out, _) = run(&w.0, &h, &["kit-verify", "--kit", "a"]);
    assert_eq!((code, member(&out, "reason")), (1, "E_KIT".into()), "{out}");
}

/// `scan --ledger` takes a home's ledger only: a plain folder has no settings to declare a basis from; `--basis`
/// and `--ledger` together, or neither, is misuse.
#[test]
fn scan_takes_its_basis_from_a_home_or_a_file_never_both() {
    let w = scratch("scan");
    let h = w.0.join("home");
    a_ledger(&w.0, &h, "a");
    let (code, out, err) = run(&w.0, &h, &["scan", "--endpoint", "1=http://127.0.0.1:9", "--ledger", "a"]);
    assert_eq!((code, out.as_str(), err.as_str()), (2, "", "E_ARGS --ledger"));
    std::fs::write(w.0.join("b.json"), b"{}").expect("basis");
    let (code, _, err) = run(&w.0, &h, &["scan", "--endpoint", "1=http://127.0.0.1:9", "--ledger", "a", "--basis", "b.json"]);
    assert_eq!((code, err.as_str()), (2, "E_ARGS --basis --ledger"));
    let (code, _, err) = run(&w.0, &h, &["scan", "--endpoint", "1=http://127.0.0.1:9"]);
    assert_eq!((code, err.as_str()), (2, "E_ARGS --basis --ledger"));
}

/// `scan --ledger <a home's ledger>` with neither `--endpoint` nor `--fixture` takes the home's own nodes, opening the
/// home with the credential as a send does: without the credential it is refused by the credential's reason, never
/// misuse. A plain folder with neither is misuse, and so is `--fixture` beside `--ledger`.
#[test]
fn a_scan_of_a_home_with_neither_takes_its_nodes_through_the_credential() {
    let w = scratch("scan-home");
    let h = w.0.join("user");
    std::fs::create_dir_all(&h).expect("user home");
    a_ledger(&w.0, &h, "a");
    let (code, out, err) = run(&w.0, &h, &["scan", "--ledger", "a"]);
    assert_eq!((code, out.as_str(), err.as_str()), (2, "", "E_ARGS --fixture --endpoint"), "a plain folder: misuse");
    let home = w.0.join("kept");
    let label = zikaron_seal::envelope::label_path(&home);
    std::fs::create_dir_all(label.parent().expect("settings")).expect("settings");
    std::fs::write(&label, b"{}").expect("a label: a home the desktop keeps");
    let ledger = home.join("ledger").display().to_string();
    let (code, out, err) = run(&w.0, &h, &["scan", "--ledger", &ledger]);
    assert_eq!((code, member(&out, "reason")), (1, "E_CREDENTIAL_MISSING".into()), "{out} {err}");
    std::fs::write(w.0.join("rec.json"), b"{}").expect("a recording");
    let (code, _, err) = run(&w.0, &h, &["scan", "--fixture", "rec.json", "--ledger", &ledger]);
    assert_eq!((code, err.as_str()), (2, "E_ARGS --ledger"));
}
