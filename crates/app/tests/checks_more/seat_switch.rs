//! Switching seat runs off the window's thread (`Kind::Seat`): the press moves the seat control at once and
//! leaves the shell where it stands until the switch lands; presses while it runs turn the wish round and the last
//! one stands; a switch that fails says so and leaves the shell, the register and the control on the seat it was.
//! Victim: whoever switches between the two seats often, the window stopping a frame or two at each press.

use app::action::{apply, apply_settled, Action, Applied};
use app::roles::Role;
use app::task::Kind;

fn drain_until_still(shell: &mut app::shell::Shell) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while shell.tasks.in_flight(Kind::Seat) && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    shell.drain();
}

fn register_seat() -> Option<Role> {
    app::register::read().ok().flatten().and_then(|r| r.current.map(|(_, s)| s))
}

#[test]
fn a_seat_switch_runs_in_the_background_and_the_last_press_stands() {
    if super::alone_in(module_path!(), "a_seat_switch_runs_in_the_background_and_the_last_press_stands") {
        return;
    }
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let words = app::action::ImportForm::Words("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string().into());
    let started = apply(&mut shell, Action::ImportIdentity { form: words, seat: Role::Author, label: "a".into(), network: app::deploy::CUSTOM.into() });
    assert!(!matches!(started, Applied::Trouble(_)), "{started:?}");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while shell.tasks.in_flight(Kind::Vault) && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    shell.drain();
    assert_eq!((register_seat(), shell.settings.role), (Some(Role::Author), Role::Author));
    let home_before = shell.home.as_ref().map(|h| h.root().to_path_buf());
    // The press: the control shows the grantee seat at once; the shell has not moved.
    answers!(apply(&mut shell, Action::SwitchRole), Applied::Started(Kind::Seat));
    assert_eq!((shell.seat_wish, shell.settings.role), (Some(Role::Grantee), Role::Author), "the control moves, the shell stays until it lands");
    assert_eq!(shell.home.as_ref().map(|h| h.root().to_path_buf()), home_before, "no home opened on the window's thread");
    drain_until_still(&mut shell);
    assert_eq!((register_seat(), shell.settings.role, shell.seat_wish), (Some(Role::Grantee), Role::Grantee, None), "landed on the grantee seat");
    assert_ne!(shell.home.as_ref().map(|h| h.root().to_path_buf()), home_before, "in the other seat's home");
    // Two presses before the first lands: back to the author seat, the last press standing.
    answers!(apply(&mut shell, Action::SwitchRole), Applied::Started(Kind::Seat));
    answers!(apply(&mut shell, Action::SwitchRole), Applied::Started(Kind::Seat));
    assert_eq!(shell.seat_wish, Some(Role::Grantee), "the second press turned the wish round");
    drain_until_still(&mut shell);
    assert_eq!((register_seat(), shell.settings.role, shell.seat_wish), (Some(Role::Grantee), Role::Grantee, None), "the last press stands");
    // A switch that fails: the other seat's home is not a home any more. Said, and nothing moved.
    let author_home = app::register::read().ok().flatten().and_then(|r| r.now().and_then(|(row, _)| row.home(Role::Author))).expect("the author seat's home");
    let aside = author_home.with_extension("aside");
    std::fs::rename(&author_home, &aside).expect("moved");
    std::fs::write(&author_home, b"not a home").expect("a file");
    answers!(apply_settled(&mut shell, Action::SwitchRole), Applied::Trouble(_));
    assert_eq!((register_seat(), shell.settings.role, shell.seat_wish), (Some(Role::Grantee), Role::Grantee, None), "the register put back, the control on the seat it was");
    std::fs::remove_file(&author_home).expect("gone");
    std::fs::rename(&aside, &author_home).expect("back");
}

/// The identity the register says is current, and its seat.
fn register_now() -> Option<(String, Role)> {
    app::register::read().ok().flatten().and_then(|r| r.current.clone())
}

/// One identity imported by its words, landed; its id.
fn import_words(shell: &mut app::shell::Shell, words: &str, label: &str) -> String {
    let before: Vec<String> = app::register::read().ok().flatten().map(|r| r.rows.iter().map(|x| x.id.clone()).collect()).unwrap_or_default();
    let form = app::action::ImportForm::Words(words.to_string().into());
    let started = apply(shell, Action::ImportIdentity { form, seat: Role::Author, label: label.into(), network: app::deploy::CUSTOM.into() });
    assert!(!matches!(started, Applied::Trouble(_)), "{label}: {started:?}");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while shell.tasks.in_flight(Kind::Vault) && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    shell.drain();
    app::register::read().ok().flatten().and_then(|r| r.rows.iter().map(|x| x.id.clone()).find(|id| !before.contains(id))).unwrap_or_else(|| panic!("{label}: no row added"))
}

/// Switching identity (the settings' identity switch) runs off the window's thread too: the press
/// opens no home and leaves the shell, the register and the identity where they stand until the switch lands;
/// identities asked for while it runs are wished and the last one stands; a switch that fails says so and leaves
/// the register on the identity and seat it named before, and the shell where it was.
/// Victim: whoever switches among several identities, the window stopping a frame at each press.
#[test]
fn an_identity_switch_runs_in_the_background_and_the_last_asked_stands() {
    if super::alone_in(module_path!(), "an_identity_switch_runs_in_the_background_and_the_last_asked_stands") {
        return;
    }
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let a = import_words(&mut shell, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about", "a");
    let b = import_words(&mut shell, "legal winner thank year wave sausage worth useful legal winner thank yellow", "b");
    let c = import_words(&mut shell, "letter advice cage absurd amount doctor acoustic avoid letter advice cage above", "c");
    assert_eq!(register_now(), Some((c.clone(), Role::Author)), "the last imported is current");
    let home_before = shell.home.as_ref().map(|h| h.root().to_path_buf());
    // The press: nothing opened on the window's thread; the register and the shell stay until it lands.
    answers!(apply(&mut shell, Action::SwitchIdentity { id: a.clone() }), Applied::Started(Kind::Seat));
    assert_eq!(shell.home.as_ref().map(|h| h.root().to_path_buf()), home_before, "no home opened on the window's thread");
    drain_until_still(&mut shell);
    assert_eq!(register_now(), Some((a.clone(), Role::Author)), "landed on the identity asked for");
    assert_ne!(shell.home.as_ref().map(|h| h.root().to_path_buf()), home_before, "in its home");
    // Two identities asked for before the first lands: the last one stands.
    answers!(apply(&mut shell, Action::SwitchIdentity { id: b.clone() }), Applied::Started(Kind::Seat));
    answers!(apply(&mut shell, Action::SwitchIdentity { id: c.clone() }), Applied::Started(Kind::Seat));
    assert_eq!(shell.identity_wish.as_deref(), Some(c.as_str()), "the second asked is wished");
    drain_until_still(&mut shell);
    assert_eq!((register_now(), shell.identity_wish.clone()), (Some((c.clone(), Role::Author)), None), "the last asked stands");
    // Settled, the switch answers as before.
    answers!(apply_settled(&mut shell, Action::SwitchIdentity { id: b.clone() }), Applied::IdentitySwitched { .. });
    assert_eq!(register_now(), Some((b.clone(), Role::Author)));
    // A switch that fails: the identity's home is not a home any more. Said; the register named back, the shell stays.
    let a_home = app::register::read().ok().flatten().and_then(|r| r.find(&a).and_then(|row| row.home(Role::Author))).expect("a's home");
    let aside = a_home.with_extension("aside");
    std::fs::rename(&a_home, &aside).expect("moved");
    std::fs::write(&a_home, b"not a home").expect("a file");
    let home_b = shell.home.as_ref().map(|h| h.root().to_path_buf());
    answers!(apply_settled(&mut shell, Action::SwitchIdentity { id: a.clone() }), Applied::Trouble(_));
    assert_eq!((register_now(), shell.home.as_ref().map(|h| h.root().to_path_buf())), (Some((b.clone(), Role::Author)), home_b), "the register put back on the identity it named, the shell where it was");
    std::fs::remove_file(&a_home).expect("gone");
    std::fs::rename(&aside, &a_home).expect("back");
}
