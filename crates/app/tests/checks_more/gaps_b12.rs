//! The whole-machine backup's index is sealed, and an endpoint item holds no inner white space on either side.
//!
//! The index names entries and held grants by their ids, so it lives in the machine directory sealed under the
//! machine's key (`local`, kind `backup-index`), never in the clear. Forms, one test each: a backup writes it
//! sealed and the count behind reads it; no index; an index that does not read (not sealed, truncated, altered)
//! falls back to the older reading and is never overwritten; another backup's index; a plain index left by an
//! older version is never read, never sealed in place, and is removed at the next backup (if its place is taken
//! by something that cannot be removed, that backup is refused by name); while locked, the index is not
//! guessed; the kind moves with the machine's files and is resealed under a new master key. Places are set
//! before any vault or shell use (`vault_open`); each test runs alone in its own process
//! ([`super::alone_in`]).

use super::vault_open;
use app::action::{apply, apply_settled, Action, Applied};
use app::local::Doc;
use std::path::{Path, PathBuf};

const PW: &str = "zikaron-backup-probe";

/// A shell on a fresh base, its anchoring key made.
fn bench(name: &str) -> (app::shell::Shell, PathBuf) {
    vault_open();
    let base = std::env::temp_dir().join(format!("zk-b12-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    answers!(apply(&mut shell, Action::MakeAnchorKey), Applied::AnchorKey(_));
    (shell, base)
}

/// A home under `base` with one entry of its own (the first entry is recorded on the spot).
fn home(shell: &mut app::shell::Shell, base: &Path, name: &str) -> PathBuf {
    let dir = base.join(name);
    answers!(apply(shell, Action::OpenHome { root: dir.display().to_string() }), Applied::Homed { .. });
    let made = apply(shell, Action::Genesis { statement: format!("{name} genesis") });
    assert!(matches!(made, Applied::Genesised { .. }), "{made:?}");
    dir
}

/// What the count behind reads now, by the one reading every reader takes.
fn behind() -> Option<u64> {
    let m = app::machine::read().expect("the machine settings");
    app::machine::backup_behind(m.backup.as_ref(), app::backup::measured(m.backup.as_ref()).ok())
}

fn last() -> app::machine::Backed {
    app::machine::read().expect("the machine settings").backup.expect("a backup recorded")
}

/// Two homes, one entry each; a backup; the second home's folder deleted and a third home made: by the index
/// one is behind (3 against 2), by the older reading none (2 against 2), so the reading taken is visible.
fn two_readings_apart(name: &str) -> (app::shell::Shell, PathBuf) {
    let (mut shell, base) = bench(name);
    home(&mut shell, &base, "a");
    let b = home(&mut shell, &base, "b");
    app::backup::export(&base.join("out"), PW, 1_800_000_000).expect("a backup");
    std::fs::remove_dir_all(&b).expect("the second folder deleted");
    home(&mut shell, &base, "c");
    assert_eq!(behind(), Some(1), "by the index");
    assert_eq!(app::backup::count_now().ok(), Some(2));
    (shell, base)
}

/// The ids of every ledger entry on this machine (their logical names say them).
fn entry_ids() -> Vec<String> {
    let key = app::keybox::local_key().expect("the local key");
    app::local::all_files()
        .expect("the machine's files")
        .into_iter()
        .filter(|f| f.doc == Doc::Entry)
        .map(|f| {
            let raw = std::fs::read(&f.at).expect("an entry");
            let plain = app::local::open_with(&key, &app::local::expect_found(&f).expect("expect"), &raw, &f.rel).expect("opens");
            let logical = app::local::logical_rel(f.doc, &f.rel, &plain).expect("logical");
            logical.rsplit('/').next().unwrap_or_default().trim_end_matches(".entry").to_string()
        })
        .collect()
}

#[test]
fn a_backup_seals_its_index_and_says_no_id_in_the_clear() {
    if super::alone_in(module_path!(), "a_backup_seals_its_index_and_says_no_id_in_the_clear") {
        return;
    }
    let (mut shell, base) = bench("sealed");
    home(&mut shell, &base, "a");
    app::backup::export(&base.join("out"), PW, 1_800_000_000).expect("a backup");
    let at = app::backup::index_path().expect("the index's place");
    let raw = std::fs::read(&at).expect("the index is there");
    assert!(app::local::is_sealed(&raw), "sealed");
    let ids = entry_ids();
    assert_eq!(ids.len(), 1);
    for id in &ids {
        assert_eq!(id.len(), 64, "{id}");
        assert!(!raw.windows(id.len()).any(|w| w == id.as_bytes()), "an entry id in the clear");
    }
    let plain = String::from_utf8(app::local::read(&at, Doc::BackupIndex).expect("opens").expect("there")).expect("text");
    assert!(ids.iter().all(|id| plain.contains(id.as_str())), "the index names what the backup holds: {plain}");
    assert!(!app::home::machine_dir().expect("machine").join(app::backup::PLAIN_INDEX_FILE).exists(), "no plain index");
    // The kind moves with the machine's files (what a key change reseals).
    let found: Vec<_> = app::local::all_files().expect("files").into_iter().filter(|f| f.doc == Doc::BackupIndex).collect();
    assert_eq!(found.len(), 1);
    assert!(matches!(found[0].whose, app::local::Whose::Machine) && found[0].rel == app::backup::index_rel());
    assert_eq!(behind(), Some(0), "right after the backup");
    home(&mut shell, &base, "b");
    assert_eq!(behind(), Some(1), "one made after it");
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn no_index_is_the_older_reading() {
    if super::alone_in(module_path!(), "no_index_is_the_older_reading") {
        return;
    }
    let (shell, base) = two_readings_apart("none");
    std::fs::remove_file(app::backup::index_path().expect("place")).expect("the index removed");
    assert_eq!(behind(), Some(0), "the older reading");
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// Not sealed, truncated, a byte altered: each falls back to the older reading, and a backup over it is refused
/// by name (`LOCAL_SEAL`, with the way out), the file untouched; once moved aside, the next backup rewrites it.
#[test]
fn an_index_that_does_not_read_is_the_older_reading_and_never_written_over() {
    if super::alone_in(module_path!(), "an_index_that_does_not_read_is_the_older_reading_and_never_written_over") {
        return;
    }
    let (shell, base) = two_readings_apart("unread");
    let at = app::backup::index_path().expect("place");
    let good = std::fs::read(&at).expect("the index");
    let mut altered = good.clone();
    if let Some(b) = altered.last_mut() {
        *b ^= 1;
    }
    for (form, bytes, why) in [
        ("not sealed", b"{\"at\":1800000000}".to_vec(), app::local::Unread::NotSealed),
        ("cut short", good[..good.len().min(20)].to_vec(), app::local::Unread::Truncated),
        ("altered", altered, app::local::Unread::Unopenable),
    ] {
        std::fs::write(&at, &bytes).expect("laid");
        let read = app::local::read(&at, Doc::BackupIndex).expect_err(form);
        assert_eq!(app::local::Unread::of(&read), Some(why), "{form}: {read:?}");
        assert_eq!(behind(), Some(0), "{form}: the older reading");
        let refused = app::backup::export(&base.join("out2"), PW, 1_800_000_100).expect_err(form);
        assert_eq!(app::local::Unread::of(&refused), Some(why), "{form}: {refused:?}");
        assert_eq!(std::fs::read(&at).expect("still there"), bytes, "{form}: never written over");
        assert_eq!(last().at, 1_800_000_000, "{form}: the record is the first backup's still");
    }
    std::fs::remove_file(&at).expect("moved aside");
    app::backup::export(&base.join("out3"), PW, 1_800_000_200).expect("the next backup");
    assert!(app::local::read(&at, Doc::BackupIndex).expect("reads").is_some());
    assert_eq!(behind(), Some(0));
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn another_backups_index_is_the_older_reading() {
    if super::alone_in(module_path!(), "another_backups_index_is_the_older_reading") {
        return;
    }
    let (shell, base) = two_readings_apart("other");
    let at = app::backup::index_path().expect("place");
    let text = String::from_utf8(app::local::read(&at, Doc::BackupIndex).expect("opens").expect("there")).expect("text");
    let other = text.replacen("\"at\":1800000000", "\"at\":1800000100", 1);
    assert_ne!(other, text);
    app::local::put(at.parent().expect("room"), app::backup::INDEX_FILE, Doc::BackupIndex, other.as_bytes()).expect("sealed");
    assert_eq!(behind(), Some(0), "another backup's index: the older reading");
    app::local::put(at.parent().expect("room"), app::backup::INDEX_FILE, Doc::BackupIndex, text.as_bytes()).expect("put back");
    assert_eq!(behind(), Some(1), "this backup's again");
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// An older version's plain index (beside the machine settings, the same items in the clear) is never read,
/// never sealed in place by the first unlock's pass, and is gone once the next backup's sealed index is written.
#[test]
fn an_older_plain_index_is_never_read_and_goes_at_the_next_backup() {
    if super::alone_in(module_path!(), "an_older_plain_index_is_never_read_and_goes_at_the_next_backup") {
        return;
    }
    let (shell, base) = two_readings_apart("plain");
    let at = app::backup::index_path().expect("place");
    let text = app::local::read(&at, Doc::BackupIndex).expect("opens").expect("there");
    std::fs::remove_file(&at).expect("the sealed index removed");
    let plain_at = app::home::machine_dir().expect("machine").join(app::backup::PLAIN_INDEX_FILE);
    std::fs::write(&plain_at, &text).expect("a plain index as older versions wrote it");
    assert_eq!(behind(), Some(0), "the plain index is not read");
    assert_eq!(app::local::migrate_plain().map(|m| m.sealed).ok(), Some(0), "nothing sealed in place");
    assert_eq!(std::fs::read(&plain_at).expect("still there"), text, "untouched");
    assert!(!at.exists(), "no sealed index made of it");
    app::backup::export(&base.join("out2"), PW, 1_800_000_100).expect("the next backup");
    assert!(!plain_at.exists(), "the plain index goes");
    assert!(app::local::is_sealed(&std::fs::read(&at).expect("the sealed index")));
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// While locked, the index cannot be opened: the count is refused as locked, never guessed from the older
/// reading; unlocked again, it reads.
#[test]
fn locked_an_index_is_not_guessed() {
    if super::alone_in(module_path!(), "locked_an_index_is_not_guessed") {
        return;
    }
    let (shell, base) = two_readings_apart("locked");
    let record = last();
    app::keybox::lock();
    let said = app::backup::measured(Some(&record)).expect_err("locked");
    assert_eq!(said.which(), Some(app::fault::Known::Locked), "{said:?}");
    app::keybox::unlock("27618394").expect("opened again");
    assert_eq!(behind(), Some(1));
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// A new master key (another identity made primary): the index is resealed with every local file under the new
/// key, at the same place, and reads to the same count.
#[test]
fn a_new_master_key_seals_the_index_again() {
    if super::alone_in(module_path!(), "a_new_master_key_seals_the_index_again") {
        return;
    }
    let (mut shell, base) = bench("rekey");
    let mut ids = Vec::new();
    for label in ["b12-a", "b12-b"] {
        answers!(apply(&mut shell, Action::NewIdentity), Applied::FreshWords);
        let (list, picks) = shell.new_words.as_ref().map(|f| (f.words(), f.picks)).expect("fresh words");
        let answers: Vec<(usize, app::secret::Secret)> = picks.iter().map(|i| (*i, list[*i].clone().into())).collect();
        let network = shell.machine.network.clone().unwrap_or_else(|| app::deploy::CUSTOM.to_string());
        let before: Vec<String> = app::register::read().ok().flatten().map(|r| r.rows.iter().map(|x| x.id.clone()).collect()).unwrap_or_default();
        let _ = apply_settled(&mut shell, Action::ConfirmIdentity { answers, label: label.into(), network });
        let after: Vec<String> = app::register::read().ok().flatten().map(|r| r.rows.iter().map(|x| x.id.clone()).collect()).unwrap_or_default();
        ids.push(after.into_iter().find(|x| !before.contains(x)).expect("an identity made"));
    }
    // The identity's own home (the one open now): its first entry, a backup, then one recorded after it.
    answers!(apply(&mut shell, Action::Genesis { statement: "b12 rekey".into() }), Applied::Genesised { .. });
    app::backup::export(&base.join("out"), PW, 1_800_000_000).expect("a backup");
    std::fs::create_dir_all(&base).expect("base");
    let file = base.join("work.txt");
    std::fs::write(&file, b"b12 work").expect("a file");
    let _ = apply_settled(&mut shell, Action::RecordWork { note_md: "b12".into(), files: vec![file.display().to_string()], for_: None });
    assert_eq!(behind(), Some(1));
    let at = app::backup::index_path().expect("place");
    let before = std::fs::read(&at).expect("the index");
    let old_key = app::keybox::local_key().expect("the key");
    app::rekey::set_primary(&ids[1], "27618394").expect("the second identity made primary");
    let new_key = app::keybox::local_key().expect("the key");
    assert_ne!(old_key.bytes(), new_key.bytes(), "a new master key");
    let after = std::fs::read(&at).expect("the index at its place");
    assert_ne!(before, after, "sealed again");
    assert!(app::local::read(&at, Doc::BackupIndex).expect("opens under the new key").is_some());
    assert_eq!(behind(), Some(1), "the same count");
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

// ───────────────────── the app's cell and the command line read one item alike ─────────────────────

/// The app's cell splits at white space and reads each piece with the shared reader; the command line reads a
/// value whole with it. A spelling with inner white space is refused by the shared reader, so the app never
/// takes it as the single item the command line would see: `1= https://…` is two pieces there, the first with
/// no address, refused. White space at either end is accepted by both.
#[test]
fn the_app_cell_and_the_command_line_read_an_item_alike() {
    if super::alone_in(module_path!(), "the_app_cell_and_the_command_line_read_an_item_alike") {
        return;
    }
    let (mut shell, base) = bench("cell");
    home(&mut shell, &base, "a");
    let url = "https://node.example/v3";
    let cell = |shell: &mut app::shell::Shell, spec: &str| -> Option<Vec<app::chainx::Endpoint>> {
        match apply(shell, Action::SetEndpoints { specs: spec.to_string() }) {
            Applied::Trouble(f) => {
                assert_eq!(f.which(), Some(app::fault::Known::SettingsShape), "{spec:?}: {f:?}");
                None
            }
            _ => Some(shell.endpoints.clone()),
        }
    };
    let one = |spec: &str| zikaron_anchor::rpc::endpoint_spec(spec).ok().map(|(c, u)| vec![format!("{c}={u}")]);
    // Taken alike: white space at either end only.
    for spec in [format!("1={url}"), format!("  1={url}\t"), format!("\n1={url} ")] {
        assert_eq!(cell(&mut shell, &spec), Some(vec![app::chainx::Endpoint::at(1, url)]), "{spec:?}");
        assert_eq!(one(&spec), Some(vec![format!("1={url}")]), "{spec:?}");
    }
    // Refused alike: white space around the `=`, inside the address, a tab, a line end.
    for spec in [format!("1= {url}"), format!("1 ={url}"), format!("1 = {url}"), format!("1={url} x"), format!("1=\t{url}"), format!("1=\n{url}")] {
        assert_eq!(zikaron_anchor::rpc::endpoint_spec(&spec), Err(zikaron_anchor::rpc::NotAnEndpoint::InnerSpace), "{spec:?}");
        assert_eq!(cell(&mut shell, &spec), None, "{spec:?}");
    }
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// The plain index's name taken by something that cannot be removed (a folder with a file in it): the next
/// backup is refused naming that place, and the record stays the earlier backup's; once moved aside, the next
/// backup goes through and the record is its own. The refusal's code is the system's answer to removing a
/// folder (macOS: not permitted; Linux: is a directory), so only the place is checked.
#[test]
fn a_plain_index_place_that_cannot_be_cleared_is_said_by_name() {
    if super::alone_in(module_path!(), "a_plain_index_place_that_cannot_be_cleared_is_said_by_name") {
        return;
    }
    let (mut shell, base) = bench("plain-stuck");
    home(&mut shell, &base, "a");
    app::backup::export(&base.join("out"), PW, 1_800_000_000).expect("a backup");
    let plain_at = app::home::machine_dir().expect("machine").join(app::backup::PLAIN_INDEX_FILE);
    std::fs::create_dir_all(&plain_at).expect("a folder under the plain index's name");
    std::fs::write(plain_at.join("inside"), b"x").expect("a file in it");
    let said = app::backup::export(&base.join("out2"), PW, 1_800_000_100).expect_err("refused");
    assert!(said.tail().contains(app::backup::PLAIN_INDEX_FILE), "{said:?}");
    assert_eq!(last().at, 1_800_000_000, "the record is the earlier backup's");
    std::fs::remove_dir_all(&plain_at).expect("moved aside");
    app::backup::export(&base.join("out3"), PW, 1_800_000_200).expect("the next backup");
    assert_eq!(last().at, 1_800_000_200);
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// A restore whose set-aside pass is interrupted (one move fails): the restored index is staged with the other
/// machine files, so it replaces this machine's index in the same landing; no unreadable-index notice is shown,
/// and after the next unlock finishes the pass the index reads as the restored backup's.
#[test]
fn a_restore_cut_in_its_set_aside_lands_the_index_with_the_rest() {
    if super::alone_in(module_path!(), "a_restore_cut_in_its_set_aside_lands_the_index_with_the_rest") {
        return;
    }
    let (mut shell, base) = bench("restore-cut");
    home(&mut shell, &base, "a");
    home(&mut shell, &base, "b");
    let made = app::backup::export(&base.join("out"), PW, 1_800_000_000).expect("a backup");
    drop(shell);
    assert!(app::local::set_move_fault(app::local::MoveFault::FailOnce(app::local::moves_so_far() + 1)));
    let _ = app::local::take_opened();
    app::backup::restore(&made.path, PW, app::backup::From::Settings("27618394")).expect("restored");
    let said: Vec<String> = app::local::take_opened().map(|o| o.troubles.iter().map(|f| f.said().to_string()).collect()).unwrap_or_default();
    assert!(!said.iter().any(|s| s.starts_with("LOCAL_SEAL")), "{said:?}");
    let ctx = zikaron_ui::egui::Context::default();
    let mut again = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let _ = apply(&mut again, Action::Lock);
    let unlocked = apply_settled(&mut again, Action::Unlock { pin: "27618394".into() });
    assert!(matches!(unlocked, Applied::Unlocked), "{unlocked:?}");
    let at = app::backup::index_path().expect("place");
    let text = String::from_utf8(app::local::read(&at, Doc::BackupIndex).expect("opens").expect("there")).expect("text");
    assert!(text.contains("\"at\":1800000000"), "{text}");
    assert_eq!(behind(), Some(0));
    drop(again);
    let _ = std::fs::remove_dir_all(&base);
}
