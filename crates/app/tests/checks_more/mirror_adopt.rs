use app::action::{apply, Action, Applied};
use std::path::{Path, PathBuf};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mirror-0.1.2/author")
}

fn files_under(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).expect("a folder").flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push((p.strip_prefix(dir).expect("under it").to_path_buf(), std::fs::read(&p).expect("a file")));
            }
        }
    }
    out.sort();
    out
}

fn copy_of_fixture(to: &Path) {
    let _ = std::fs::remove_dir_all(to);
    for (rel, bytes) in files_under(&fixture()) {
        let at = to.join(rel);
        std::fs::create_dir_all(at.parent().expect("a parent")).expect("a folder");
        std::fs::write(&at, bytes).expect("written");
    }
}

fn fresh_home(tag: &str) -> (app::shell::Shell, PathBuf) {
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.reread_vault();
    let at = std::env::temp_dir().join(format!("zk-mirror-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    super::seated(&mut shell, &at);
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: at.display().to_string() }), Applied::Homed { .. }));
    (shell, at)
}

fn entries_in_home(shell: &app::shell::Shell) -> Vec<Vec<u8>> {
    let mut items = shell.home.as_ref().expect("a home").ledger().expect("the ledger").pile().expect("its entries").items;
    items.sort();
    items
}

#[test]
fn a_mirror_written_by_an_earlier_version_is_adopted_whole() {
    if super::alone_in(module_path!(), "a_mirror_written_by_an_earlier_version_is_adopted_whole") {
        return;
    }
    let from = fixture();
    let mut want: Vec<Vec<u8>> = app::mirror::verify(&from).expect("the fixture verifies").into_iter().map(|(_, b)| b).collect();
    want.sort();
    assert!(want.len() > 1, "the fixture holds a genesis and more");
    let before = files_under(&from);
    let (mut shell, at) = fresh_home("earlier");
    match apply(&mut shell, Action::Adopt { dir: from.display().to_string() }) {
        Applied::AdoptedInPlace { entries, linked, label } => {
            assert_eq!((entries, linked), (want.len(), want.len()), "every entry seen and linked");
            assert!(!label.is_empty(), "the self-audit ran");
        }
        other => panic!("adopted: {other:?}"),
    }
    assert_eq!(entries_in_home(&shell), want, "each entry in this home opens to the mirror's bytes");
    assert_eq!(files_under(&from), before, "the mirror untouched");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_mirror_with_one_entry_changed_is_refused_whole_and_nothing_is_written() {
    if super::alone_in(module_path!(), "a_mirror_with_one_entry_changed_is_refused_whole_and_nothing_is_written") {
        return;
    }
    let from = std::env::temp_dir().join(format!("zk-mirror-copy-{}", std::process::id()));
    copy_of_fixture(&from);
    let entries = from.join(zikaron_glue::mirror::ENTRIES);
    let one = std::fs::read_dir(&entries).expect("the entries").flatten().map(|e| e.path()).min().expect("an entry");
    let mut b = std::fs::read(&one).expect("an entry");
    *b.last_mut().expect("bytes") ^= 1;
    std::fs::write(&one, &b).expect("changed");
    let before = files_under(&from);
    let (mut shell, at) = fresh_home("changed");
    match apply(&mut shell, Action::Adopt { dir: from.display().to_string() }) {
        Applied::Trouble(f) => assert!(f.said().starts_with(app::fault::Known::MirrorEntry.as_str()), "refused by the mirror's own check: {}", f.said()),
        other => panic!("refused: {other:?}"),
    }
    assert!(entries_in_home(&shell).is_empty(), "not one entry written");
    assert_eq!(files_under(&from), before, "the mirror untouched");
    let _ = std::fs::remove_dir_all(&at);
    let _ = std::fs::remove_dir_all(&from);
}

#[test]
fn a_mirror_of_another_ledger_is_refused_by_a_home_that_has_its_own() {
    if super::alone_in(module_path!(), "a_mirror_of_another_ledger_is_refused_by_a_home_that_has_its_own") {
        return;
    }
    let (mut shell, at) = fresh_home("rooted");
    assert!(matches!(apply(&mut shell, Action::Genesis { statement: "my own".into() }), Applied::Genesised { .. }));
    let before = entries_in_home(&shell);
    match apply(&mut shell, Action::Adopt { dir: fixture().display().to_string() }) {
        Applied::Trouble(f) => {
            assert!(f.said().starts_with(app::fault::Known::AlreadyRooted.as_str()), "{}", f.said());
            assert_eq!(f.what_key(), Some(app::lang::Key::AdoptOtherRoot), "said as another ledger");
            assert_eq!(f.then_key(), Some(app::lang::Key::AdoptOtherRootNext), "with the way out");
        }
        other => panic!("refused: {other:?}"),
    }
    assert_eq!(entries_in_home(&shell), before, "not one entry written");
    let home = shell.home.as_ref().expect("a home");
    assert!(app::auditx::offline(home).expect("the check").complete, "the home's own ledger still checks whole");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn the_same_ledger_adopted_again_is_taken_as_before() {
    if super::alone_in(module_path!(), "the_same_ledger_adopted_again_is_taken_as_before") {
        return;
    }
    let (mut shell, at) = fresh_home("again");
    for _ in 0..2 {
        match apply(&mut shell, Action::Adopt { dir: fixture().display().to_string() }) {
            Applied::AdoptedInPlace { linked, label, .. } => assert_eq!((linked, label.as_str()), (4, app::auditx::complete())),
            other => panic!("adopted: {other:?}"),
        }
    }
    assert_eq!(entries_in_home(&shell).len(), 4, "each entry once");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_home_that_adopted_a_ledger_starts_no_other() {
    if super::alone_in(module_path!(), "a_home_that_adopted_a_ledger_starts_no_other") {
        return;
    }
    let (mut shell, at) = fresh_home("adopted-first");
    assert!(matches!(apply(&mut shell, Action::Adopt { dir: fixture().display().to_string() }), Applied::AdoptedInPlace { .. }));
    match apply(&mut shell, Action::Genesis { statement: "another".into() }) {
        Applied::Trouble(f) => assert!(f.said().starts_with(app::fault::Known::AlreadyRooted.as_str()), "{}", f.said()),
        other => panic!("refused: {other:?}"),
    }
    assert_eq!(entries_in_home(&shell).len(), 4);
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_second_instance_reading_a_home_adopts_nothing() {
    if super::alone_in(module_path!(), "a_second_instance_reading_a_home_adopts_nothing") {
        return;
    }
    let (_writer, at) = fresh_home("reader");
    let ctx = zikaron_ui::egui::Context::default();
    let mut second = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    second.reread_vault();
    let _ = apply(&mut second, Action::OpenHome { root: at.display().to_string() });
    assert!(second.lock.as_ref().is_some_and(|l| !l.mode().writable()), "the second instance reads the home");
    let got = apply(&mut second, Action::Adopt { dir: fixture().display().to_string() });
    assert!(matches!(&got, Applied::Trouble(f) if f.which() == Some(app::fault::Known::ReadOnly)), "refused as read-only: {got:?}");
    assert!(entries_in_home(&second).is_empty(), "nothing written");
    let _ = std::fs::remove_dir_all(&at);
}
