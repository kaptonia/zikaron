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
    Outcome { kind, serial, result, epoch: 0, stale: false, earlier: false, nodes: 0, renoded: false, seen: None, part: false }
}

fn submitted(tx: &str) -> Done {
    Done::Submitted { tx: tx.into(), chain: 31337, url: app::chainx::NodeAddr::new("http://127.0.0.1:1"), ids: Vec::new(), gas: None, queue: Default::default() }
}

fn replied(rx: &Receiver<Reply>) -> Option<Reply> {
    rx.try_recv().ok()
}

#[test]
fn a_send_is_answered_by_its_own_landing_and_a_broadcast_never_reads_as_lost() {
    let mut s = shell();
    let (tx, rx) = channel();
    s.door_doing = Some(Doing::Send { answer: tx, serial: 7 });
    s.door_turn(&[landed(Kind::Anchor, 6, Ok(submitted("0xother")))]);
    assert!(replied(&rx).is_none() && s.door_doing.is_some(), "another serial's landing does not answer it");
    s.door_turn(&[]);
    assert!(replied(&rx).is_none() && s.door_doing.is_some(), "no landing, no answer");
    s.door_turn(&[landed(Kind::Anchor, 7, Ok(submitted("0xmine")))]);
    assert_eq!(replied(&rx), Some(Reply::NotYet { tx: "0xmine".into(), waited: 0 }));
    assert!(s.door_doing.is_none(), "answered once");
}

#[test]
fn each_landing_of_a_request_has_its_own_reply() {
    let fault = || app::fault::Fault::known(app::fault::Known::Unreachable, "node".to_string());
    let mut s = shell();
    let (tx, rx) = channel();
    s.door_doing = Some(Doing::Send { answer: tx, serial: 3 });
    s.door_turn(&[landed(Kind::Anchor, 3, Err(fault()))]);
    assert!(matches!(replied(&rx), Some(Reply::Refused { code, network: true, .. }) if code == "UNREACHABLE"));
    let (tx, rx) = channel();
    s.door_doing = Some(Doing::Record { answer: tx, serial: 4 });
    s.door_turn(&[landed(Kind::Record, 9, Err(fault()))]);
    assert!(replied(&rx).is_none(), "not its serial");
    s.door_turn(&[landed(Kind::Record, 4, Err(fault()))]);
    assert_eq!(replied(&rx), Some(Reply::Closed));
    let (tx, rx) = channel();
    s.door_doing = Some(Doing::Kit { answer: tx, serial: 5 });
    s.door_turn(&[landed(Kind::Kit, 5, Err(fault()))]);
    assert!(matches!(replied(&rx), Some(Reply::Refused { code, .. }) if code == "UNREACHABLE"));
    let (tx, rx) = channel();
    s.door_doing = Some(Doing::Estimate { answer: tx, count: 1, serial: 8 });
    let stale = app::fault::Fault::known(app::fault::Known::GasNotShown, app::lang::t(app::lang::Key::TailGasBehind).to_string());
    s.said.insert(Kind::Gas, app::action::Applied::Trouble(stale));
    s.door_turn(&[landed(Kind::Gas, 8, Ok(Done::Gas { count: 1, gas: 1, calldata: String::new(), fees: zikaron_anchor::send::Fees::fallback(), fees_left: Vec::new(), head_time: None, lane: (0, Vec::new()) }))]);
    assert!(matches!(replied(&rx), Some(Reply::Refused { code, .. }) if code == "GAS_NOT_ESTIMATED"));
}
