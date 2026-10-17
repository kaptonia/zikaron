//! The command line's two switches and its credential: turning writing alone on issues an
//! owner-only credential holding this vault's master key, the desktop switch does not flip while it is on, off
//! removes the credential, and a credential that expired or holds another master key is removed with writing
//! alone turned off, said by name. Each runs alone (its own machine directory).

use app::action::{apply, apply_settled, Action, Applied};
use app::machine::Lasts;

const PIN: &str = "27618394";

fn shell() -> app::shell::Shell {
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx))
}

fn credential() -> std::path::PathBuf {
    app::home::machine_dir().expect("the machine directory").join(app::places::credential_file())
}

fn codes(shell: &app::shell::Shell) -> Vec<String> {
    shell.faults.iter().map(|f| f.said().split([' ', ':']).next().unwrap_or_default().to_string()).collect()
}

#[test]
fn writing_alone_issues_an_owner_only_credential_of_this_vault_and_off_removes_it() {
    if super::alone_in(module_path!(), "writing_alone_issues_an_owner_only_credential_of_this_vault_and_off_removes_it") {
        return;
    }
    let mut shell = shell();
    let before = app::keybox::tries_left().expect("tries");
    match apply_settled(&mut shell, Action::SetCliAlone { lasts: Lasts::Secs(3_600), pin: PIN.into() }) {
        Applied::CliAloneSet(Some(Some(_))) => {}
        other => panic!("writing alone should be on: {other:?}"),
    }
    let p = credential();
    assert!(zikaron_os::is_owner_only(&p).expect("the credential is there"), "owner-only");
    let v = zikaron::json::parse(&std::fs::read(&p).expect("read")).expect("json");
    let zikaron::json::Value::Obj(m) = v else { panic!("an object") };
    assert_eq!(m.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), zikaron_seal::credential::member::ALL, "the closed table");
    assert!(matches!(app::keybox::credential_now(app::lastread::now_secs()), Ok(app::keybox::Credential::Live(_))), "it opens to this vault's master key");
    assert_eq!(app::machine::read().expect("machine").cli_alone, Some(Lasts::Secs(3_600)), "the switch is sealed");
    let plain = std::fs::read_to_string(app::machine::path().expect("path")).expect("plain");
    assert!(!plain.contains("cli_"), "the plain file does not carry it: {plain}");
    assert_eq!(shell.machine.cli_path(), app::machine::CliPath::Alone);
    match apply(&mut shell, Action::SetCliDesktop { on: false }) {
        Applied::Trouble(f) => assert!(f.said().starts_with("CLI_ALONE_ON"), "{}", f.said()),
        other => panic!("the desktop switch should not flip: {other:?}"),
    }
    assert!(shell.machine.cli_desktop, "unchanged");
    match apply(&mut shell, Action::CliAloneOff) {
        Applied::CliAloneSet(None) => {}
        other => panic!("writing alone should be off: {other:?}"),
    }
    assert!(!p.exists(), "the credential is gone");
    assert_eq!(app::machine::read().expect("machine").cli_alone, None);
    match apply(&mut shell, Action::SetCliDesktop { on: false }) {
        Applied::CliDesktopSet(false) => {}
        other => panic!("now it flips: {other:?}"),
    }
    assert_eq!(shell.machine.cli_path(), app::machine::CliPath::Closed);
    assert_eq!(app::keybox::tries_left().expect("tries"), before, "the right passcode counts nothing");
}

#[test]
fn an_expired_credential_is_removed_and_writing_alone_goes_off_said_by_name() {
    if super::alone_in(module_path!(), "an_expired_credential_is_removed_and_writing_alone_goes_off_said_by_name") {
        return;
    }
    let mut shell = shell();
    assert!(matches!(apply_settled(&mut shell, Action::SetCliAlone { lasts: Lasts::Secs(60), pin: PIN.into() }), Applied::CliAloneSet(Some(_))));
    // Its time dialled back: issued and expired before now.
    app::keybox::issue_credential(zikaron_seal::credential::Term { issued: 10, expires: Some(70) }).expect("issued");
    shell.credential = Some(zikaron_seal::credential::Term { issued: 10, expires: Some(70) });
    let _ = shell.drain_at(0.0);
    assert!(!credential().exists(), "removed");
    assert_eq!(app::machine::read().expect("machine").cli_alone, None, "the switch went off");
    assert!(codes(&shell).iter().any(|c| c == "CREDENTIAL_EXPIRED"), "said: {:?}", codes(&shell));
}

#[test]
fn a_credential_of_another_master_key_is_dropped_and_a_bad_one_counts_no_passcode_try() {
    if super::alone_in(module_path!(), "a_credential_of_another_master_key_is_dropped_and_a_bad_one_counts_no_passcode_try") {
        return;
    }
    let mut shell = shell();
    assert!(matches!(apply_settled(&mut shell, Action::SetCliAlone { lasts: Lasts::Never, pin: PIN.into() }), Applied::CliAloneSet(Some(None))));
    let before = app::keybox::tries_left().expect("tries");
    let dir = app::home::machine_dir().expect("dir");
    // A credential that does not open: refused by name, no passcode try counted.
    std::fs::write(credential(), b"{}").expect("laid");
    assert!(app::keybox::credential_now(app::lastread::now_secs()).is_err());
    assert_eq!(app::keybox::tries_left().expect("tries"), before, "a credential failure is no passcode try");
    // One holding another master key (issued before a new master key took effect): dropped at the next read.
    zikaron_seal::credential::issue(&dir, app::places::key_account(), &[9u8; 32], zikaron_seal::credential::Term { issued: 1, expires: None }).expect("issued");
    shell.reread_credential();
    assert!(!credential().exists(), "removed");
    assert_eq!(app::machine::read().expect("machine").cli_alone, None, "the switch went off");
    assert!(codes(&shell).iter().any(|c| c == "CLI_ALONE_DROPPED"), "said: {:?}", codes(&shell));
}
