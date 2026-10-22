//! Where an identity lands when it becomes current (`Row::seat_here_or_first`): the seat in use when the
//! identity occupies it, else its first occupied seat; one rule for importing (and creating), refilling and
//! switching.

use app::action::{apply, Action, Applied};
use app::roles::Role;

fn settled(shell: &mut app::shell::Shell, k: app::task::Kind) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while shell.tasks.in_flight(k) && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    shell.drain();
}

/// One import, landed; the id of the row it added.
fn import(shell: &mut app::shell::Shell, form: app::action::ImportForm, seat: Role, label: &str) -> String {
    let before: Vec<String> = rows();
    let started = apply(shell, Action::ImportIdentity { form, seat, label: label.into(), network: app::deploy::CUSTOM.into() });
    assert!(!matches!(started, Applied::Trouble(_)), "{label}: {started:?}");
    settled(shell, app::task::Kind::Vault);
    rows().into_iter().find(|id| !before.contains(id)).unwrap_or_else(|| panic!("{label}: no row added"))
}

fn rows() -> Vec<String> {
    app::register::read().ok().flatten().map(|r| r.rows.iter().map(|x| x.id.clone()).collect()).unwrap_or_default()
}

/// The seat the register says is current, and the shell's.
fn seat_now(shell: &app::shell::Shell) -> (Option<Role>, Role) {
    (app::register::read().ok().flatten().and_then(|r| r.current.map(|(_, s)| s)), shell.settings.role)
}

#[test]
fn an_identity_lands_on_the_seat_in_use_when_it_has_it() {
    if super::alone_in(module_path!(), "an_identity_lands_on_the_seat_in_use_when_it_has_it") {
        return;
    }
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let words = |w: &str| app::action::ImportForm::Words(w.to_string().into());
    let a = import(&mut shell, words("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"), Role::Author, "a");
    assert_eq!(seat_now(&shell), (Some(Role::Author), Role::Author), "the first identity: the author seat");
    answers!(app::action::apply_settled(&mut shell, Action::SwitchRole), Applied::Seated(Role::Grantee));
    // Imported while on the grantee seat, an identity with both seats stays on it.
    let b = import(&mut shell, words("legal winner thank year wave sausage worth useful legal winner thank yellow"), Role::Grantee, "b");
    assert_eq!(seat_now(&shell), (Some(Role::Grantee), Role::Grantee), "imported on the grantee seat: stays");
    // A key occupying the author seat only: the seat in use is not its, so its first seat.
    let key = app::action::ImportForm::PrivateKey { key: format!("0x{}", "11".repeat(32)).into(), keyfile: None };
    let c = import(&mut shell, key, Role::Author, "c");
    assert_eq!(seat_now(&shell), (Some(Role::Author), Role::Author), "an author-only key lands on the author seat");
    // Switching, on the author seat: to an identity with both seats, the author seat.
    answers!(app::action::apply_settled(&mut shell, Action::SwitchIdentity { id: a.clone() }), Applied::IdentitySwitched { .. });
    assert_eq!(seat_now(&shell), (Some(Role::Author), Role::Author));
    // On the grantee seat, switching to another identity with both seats stays on the grantee seat.
    answers!(app::action::apply_settled(&mut shell, Action::SwitchRole), Applied::Seated(Role::Grantee));
    answers!(app::action::apply_settled(&mut shell, Action::SwitchIdentity { id: b }), Applied::IdentitySwitched { .. });
    assert_eq!(seat_now(&shell), (Some(Role::Grantee), Role::Grantee), "switched on the grantee seat: stays");
    // On the grantee seat, switching to the author-only key: its first seat.
    answers!(app::action::apply_settled(&mut shell, Action::SwitchIdentity { id: c }), Applied::IdentitySwitched { .. });
    assert_eq!(seat_now(&shell), (Some(Role::Author), Role::Author), "the seat in use is not its: the author seat");
}
