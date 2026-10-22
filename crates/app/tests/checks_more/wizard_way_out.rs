//! The first-run wizard's way out is judged now: offered once a passcode and an identity are both on this
//! machine, the same on the first run as after a restart, never before; leaving lands on the seat with a key and
//! writes no ledger, and the wizard opens again at "Create the ledger". The ledger page with no ledger yet offers
//! "Create the ledger" (the sublicense page's key, named as the wizard's step), which opens the wizard at that step;
//! with a ledger it does not. Each test runs alone (its own machine directory).

use app::action::{apply, apply_settled, Action, Applied};
use zikaron_ui::egui;

const W: f32 = 1180.0;
const H: f32 = 760.0;

fn home_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("zk-way-out-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn the_way_out_appears_once_a_passcode_and_an_identity_are_here() {
    if super::alone_in(module_path!(), "the_way_out_appears_once_a_passcode_and_an_identity_are_here") {
        return;
    }
    // A machine never set up: no vault, no identity.
    super::vault_open();
    let m = app::home::machine_dir().expect("the machine directory");
    let _ = std::fs::remove_file(m.join(app::places::keybox_file()));
    app::keybox::lock();
    let ctx = egui::Context::default();
    let s = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let dir = home_dir("first-run");
    let (s, r) = app::window::probe_wizard_way_out(
        &ctx,
        s,
        |sh, phase| {
            if phase == 1 {
                assert!(matches!(apply_settled(sh, Action::SetPin { pin: "27618394".into(), again: "27618394".into() }), Applied::Unlocked));
            } else {
                super::seated(sh, &dir);
                assert!(matches!(apply(sh, Action::OpenHome { root: dir.display().to_string() }), Applied::Homed { .. }));
                apply(sh, Action::ChooseNetwork { name: app::deploy::DEFAULT.into() });
            }
        },
        W,
        H,
    );
    assert_eq!(r.offered, [false, false, true], "no way out before a passcode and an identity, one after: {r:?}");
    assert_eq!(r.exit_drawn, [false, false, true], "\"Exit setup\" drawn only then: {r:?}");
    assert!(r.left_by_click && !r.still_open && r.seat_has_key_after, "\"Exit setup\" clicked leaves, onto the seat with a key: {r:?}");
    let entries = s.home.as_ref().expect("a home").ledger().expect("the ledger").pile().expect("its entries").items.len();
    assert_eq!(entries, 0, "leaving writes no ledger");
    assert!(r.reopened && r.reopened_offered && r.reopened_step == "Genesis", "after a restart the wizard opens at Create the ledger, with its way out: {r:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_ledger_page_offers_to_set_up_a_ledger_only_without_one() {
    if super::alone_in(module_path!(), "the_ledger_page_offers_to_set_up_a_ledger_only_without_one") {
        return;
    }
    super::vault_open();
    let ctx = egui::Context::default();
    let mut s = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let dir = home_dir("no-ledger");
    super::seated(&mut s, &dir);
    assert!(matches!(apply(&mut s, Action::OpenHome { root: dir.display().to_string() }), Applied::Homed { .. }));
    // What opening the home started (the ledger and grants reads) lands first: the key is judged on a read ledger.
    landed(&mut s);
    let (mut s, drawn, step) = app::window::probe_ledger_set_up(&ctx, s, W, H);
    assert!(drawn, "the key is drawn on the ledger page with no ledger (in flight: {:?})", s.tasks.flying());
    assert_eq!(step, "Genesis", "pressing it opens the wizard at Create the ledger");
    let made = apply(&mut s, Action::Genesis { statement: "now one".into() });
    assert!(matches!(made, Applied::Genesised { .. }), "genesis: {made:?} (in flight: {:?})", s.tasks.flying());
    landed(&mut s);
    let (_, drawn, _) = app::window::probe_ledger_set_up(&ctx, s, W, H);
    assert!(!drawn, "with a ledger the key is not drawn");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Read the ledger and the grants and receive what is in flight until nothing is (bounded).
fn landed(s: &mut app::shell::Shell) {
    let _ = apply(s, Action::ReadLedger);
    let _ = apply(s, Action::ReadGrants);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    s.drain();
    while !s.tasks.flying().is_empty() && std::time::Instant::now() < until {
        s.drain();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    s.drain();
}
