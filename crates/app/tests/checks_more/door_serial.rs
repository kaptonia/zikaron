//! The command-line channel answers a request from the outcome of the task that request started, by its
//! serial, never by its kind: another task of the same kind landing meanwhile does not answer it, and each of
//! its own landings has a named reply (a broadcast whose receipt wait did not start is "not yet included" with
//! its transaction; a landing for a home no longer open closes the channel; an estimate made stale is refused
//! by name). Tested by handing the shell's channel turn (`door_turn`) the landings it would drain, with no node
//! and no clock.

use app::door::Doing;
use app::task::{Done, Kind, Outcome};
use std::sync::mpsc::{channel, Receiver};
use zikaron_glue::door::Reply;

fn shell() -> app::shell::Shell {
    let ctx = zikaron_ui::egui::Context::default();
    super::vault_open();
    app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx))
}

fn landed(kind: Kind, serial: u64, result: Result<Done, app::fault::Fault>) -> Outcome {
    Outcome { kind, serial, result, epoch: 0, stale: false, earlier: false }
}

fn submitted(tx: &str) -> Done {
    Done::Submitted { tx: tx.into(), chain: 31337, url: app::chainx::NodeAddr::new("http://127.0.0.1:1"), ids: Vec::new(), gas: None, queue: Default::default() }
}

/// The reply sent so far, if any.
fn replied(rx: &Receiver<Reply>) -> Option<Reply> {
    rx.try_recv().ok()
}

#[test]
fn a_send_is_answered_by_its_own_landing_and_a_broadcast_never_reads_as_lost() {
    let mut s = shell();
    let (tx, rx) = channel();
    s.door_doing = Some(Doing::Send { answer: tx, serial: 7 });
    // Another task of the kind lands: not this request's.
    s.door_turn(&[landed(Kind::Anchor, 6, Ok(submitted("0xother")))]);
    assert!(replied(&rx).is_none() && s.door_doing.is_some(), "another serial's landing does not answer it");
    // Nothing of its own landed, and nothing of the kind is in flight: it keeps waiting (no loss guessed).
    s.door_turn(&[]);
    assert!(replied(&rx).is_none() && s.door_doing.is_some(), "no landing, no answer");
    // Its broadcast lands and no receipt wait started (the vault locking meanwhile): not yet, with its tx.
    s.door_turn(&[landed(Kind::Anchor, 7, Ok(submitted("0xmine")))]);
    assert_eq!(replied(&rx), Some(Reply::NotYet { tx: "0xmine".into(), waited: 0 }));
    assert!(s.door_doing.is_none(), "answered once");
}

#[test]
fn each_landing_of_a_request_has_its_own_reply() {
    let fault = || app::fault::Fault::known(app::fault::Known::Unreachable, "node".to_string());
    // A send refused by name.
    let mut s = shell();
    let (tx, rx) = channel();
    s.door_doing = Some(Doing::Send { answer: tx, serial: 3 });
    s.door_turn(&[landed(Kind::Anchor, 3, Err(fault()))]);
    assert!(matches!(replied(&rx), Some(Reply::Refused { code, network: true, .. }) if code == "UNREACHABLE"));
    // A record batch whose answer is not there (it landed for a home no longer open): the channel closing.
    let (tx, rx) = channel();
    s.door_doing = Some(Doing::Record { answer: tx, serial: 4 });
    s.door_turn(&[landed(Kind::Record, 9, Err(fault()))]);
    assert!(replied(&rx).is_none(), "not its serial");
    s.door_turn(&[landed(Kind::Record, 4, Err(fault()))]);
    assert_eq!(replied(&rx), Some(Reply::Closed));
    // A package refused by name.
    let (tx, rx) = channel();
    s.door_doing = Some(Doing::Kit { answer: tx, serial: 5 });
    s.door_turn(&[landed(Kind::Kit, 5, Err(fault()))]);
    assert!(matches!(replied(&rx), Some(Reply::Refused { code, .. }) if code == "UNREACHABLE"));
    // An estimate answered as stale (`said` holds the shell's answer for it): refused by that name, not lost.
    let (tx, rx) = channel();
    s.door_doing = Some(Doing::Estimate { answer: tx, count: 1, serial: 8 });
    let stale = app::fault::Fault::known(app::fault::Known::GasNotShown, app::lang::t(app::lang::Key::TailGasBehind).to_string());
    s.said.insert(Kind::Gas, app::action::Applied::Trouble(stale));
    s.door_turn(&[landed(Kind::Gas, 8, Ok(Done::Gas { count: 1, gas: 1, calldata: String::new(), fees: zikaron_anchor::send::Fees::fallback(), fees_left: Vec::new(), head_time: None }))]);
    assert!(matches!(replied(&rx), Some(Reply::Refused { code, .. }) if code == "GAS_NOT_ESTIMATED"));
}
