//! The proxy route while the vault is locked: the sealed settings are not in hand, so a connection opened then
//! goes the route last read in this process, kept without its password; it never falls back to the system's
//! proxy unsaid. Runs alone in its own process ([`super::alone_in`]): it locks the vault.

use super::vault_open;
use app::action::{apply, Action, Applied};
use zikaron_net::{Choice, Kind};

/// Off stays off and a typed proxy stays that proxy while locked (its password not kept); unlocked again, the
/// password is read back with the rest; a choice saved just before locking is the one kept.
#[test]
fn a_locked_vault_keeps_the_route_last_read_without_its_password() {
    if super::alone_in(module_path!(), "a_locked_vault_keeps_the_route_last_read_without_its_password") {
        return;
    }
    vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    for (typed, locked) in [("none", Choice::Off), ("socks5://u:pw@127.0.0.1:1080", Choice::Manual(zikaron_net::proxy_of("socks5://u@127.0.0.1:1080").expect("typed")))] {
        assert!(matches!(apply(&mut shell, Action::SetProxy { choice: typed.into() }), Applied::ProxySet(_)), "{typed}");
        let open = app::machine::proxy_now();
        app::keybox::lock();
        assert_eq!(app::machine::proxy_now(), locked, "{typed}: locked");
        app::keybox::unlock("27618394").expect("opened again");
        assert_eq!(app::machine::proxy_now(), open, "{typed}: unlocked");
    }
    match app::machine::proxy_now() {
        Choice::Manual(p) => assert_eq!((p.kind, p.has_password()), (Kind::Socks5, true)),
        other => panic!("{other:?}"),
    }
    // A choice saved just before locking, no connection opened in between, is the route a locked connection goes.
    assert!(matches!(apply(&mut shell, Action::SetProxy { choice: "none".into() }), Applied::ProxySet(_)));
    app::keybox::lock();
    assert_eq!(app::machine::proxy_now(), Choice::Off, "saved just before locking");
    app::keybox::unlock("27618394").expect("opened again");
}
