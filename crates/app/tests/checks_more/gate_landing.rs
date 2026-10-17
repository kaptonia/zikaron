//! The passcode gate in front of the five actions that ask for it again on an open vault only checks the
//! passcode: it opens nothing, and an action whose vault locked while its passcode was checked is not done. A
//! backup that fails anywhere inside its task is recorded on this machine.

use app::action::{apply, Action, Applied};

fn settled(shell: &mut app::shell::Shell, k: app::task::Kind) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while shell.tasks.in_flight(k) && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    shell.drain();
}

fn pin() -> app::secret::Secret {
    app::secret::Secret::from(String::from("27618394"))
}

/// A correct passcode checked on a locked vault answers yes and the vault stays locked; a wrong one counts one
/// try, as an opening does, and a correct one clears the count.
#[test]
fn checking_the_passcode_opens_nothing() {
    if super::alone_in(module_path!(), "checking_the_passcode_opens_nothing") {
        return;
    }
    super::vault_open();
    app::keybox::lock();
    assert!(app::keybox::check_pin("27618394").is_ok(), "the right passcode");
    assert!(matches!(app::keybox::state(), Ok(app::keybox::State::Locked { .. })), "still locked: {:?}", app::keybox::state());
    let before = app::keybox::tries_left().expect("tries");
    let wrong = app::keybox::check_pin("00000000").expect_err("a wrong passcode");
    assert_eq!(wrong.which(), Some(app::fault::Known::PinWrong));
    assert_eq!(app::keybox::tries_left().expect("tries"), before - 1, "one try spent");
    assert!(app::keybox::check_pin("27618394").is_ok());
    assert_eq!(app::keybox::tries_left().expect("tries"), before, "cleared by the right one");
    assert!(matches!(app::keybox::state(), Ok(app::keybox::State::Locked { .. })), "and still locked");
}

/// The words asked for, then the vault locked while the passcode is checked: the landing reads where the vault
/// stands, refuses as locked, shows nothing, and leaves the vault locked.
#[test]
fn a_passcode_action_whose_vault_locks_meanwhile_is_not_done() {
    if super::alone_in(module_path!(), "a_passcode_action_whose_vault_locks_meanwhile_is_not_done") {
        return;
    }
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    answers!(apply(&mut shell, Action::NewIdentity), Applied::FreshWords);
    let (list, picks) = shell.new_words.as_ref().map(|f| (f.words(), f.picks)).expect("fresh words");
    let answers: Vec<(usize, app::secret::Secret)> = picks.iter().map(|i| (*i, list[*i].clone())).collect();
    apply(&mut shell, Action::ConfirmIdentity { answers, label: "gate".into(), network: app::deploy::CUSTOM.into() });
    settled(&mut shell, app::task::Kind::Vault);
    answers!(apply(&mut shell, Action::RevealWords { pin: pin() }), Applied::Started(app::task::Kind::Vault));
    apply(&mut shell, Action::Lock);
    let faults = shell.faults.len();
    settled(&mut shell, app::task::Kind::Vault);
    assert!(shell.words.is_none(), "nothing shown");
    assert!(!shell.unlocked(), "the vault stays locked");
    assert!(matches!(app::keybox::state(), Ok(app::keybox::State::Locked { .. })), "on disk too: {:?}", app::keybox::state());
    assert_eq!(shell.faults.get(faults..).and_then(|f| f.last()).and_then(|f| f.which()), Some(app::fault::Known::Locked), "refused as locked");
    // Open again, ask again: shown, as before.
    apply(&mut shell, Action::Unlock { pin: pin() });
    settled(&mut shell, app::task::Kind::Vault);
    apply(&mut shell, Action::RevealWords { pin: pin() });
    settled(&mut shell, app::task::Kind::Vault);
    assert_eq!(shell.words.as_ref().map(|w| w.len()), Some(12), "shown on an open vault");
}

/// A backup whose folder cannot be made (a file stands where it would go): the task fails, and the failure is
/// recorded on this machine, as a failure to write or read the file back always was.
#[test]
fn a_backup_that_fails_anywhere_in_its_task_is_recorded() {
    if super::alone_in(module_path!(), "a_backup_that_fails_anywhere_in_its_task_is_recorded") {
        return;
    }
    super::vault_open();
    let base = std::env::temp_dir().join(format!("zk-backup-fails-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).expect("base");
    std::fs::write(base.join("a-file"), b"in the way").expect("a file");
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let _ = app::machine::update(|m| m.backup_failed = None);
    let password = || app::secret::Secret::from(String::from("a backup password long enough"));
    let dir = base.join("a-file").join("backups").display().to_string();
    apply(&mut shell, Action::ExportBackup { pin: pin(), password: password(), again: password(), dir });
    settled(&mut shell, app::task::Kind::Vault);
    settled(&mut shell, app::task::Kind::Backup);
    assert!(app::machine::read().expect("machine settings").backup_failed.is_some(), "recorded");
    let _ = app::machine::update(|m| m.backup_failed = None);
    let _ = std::fs::remove_dir_all(&base);
}
