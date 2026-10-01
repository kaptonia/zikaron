//! `Tasks::answered_since`: what a press asked of a kind of task has answered only when that kind lands after
//! the press (the gas page and the send sheet both judge by it).

use app::fault::{Fault, Known};
use app::task::{Kind, Tasks};

fn land(t: &mut Tasks, k: Kind) {
    for _ in 0..2000 {
        if t.drain_at(1.0).iter().any(|o| o.kind == k) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    panic!("the task never landed");
}

/// A press is answered by a landing after it, only: not by the one already there, not while in flight,
/// and not by a cleared "started" mark (the stamp is a count, not a time).
#[test]
fn a_press_is_answered_by_a_later_landing_only() {
    let mut t = Tasks::new();
    let _ = t.spawn(Kind::Chain, || Err(Fault::known(Known::Unreachable, String::new())));
    land(&mut t, Kind::Chain);
    // The press: its stamp, taken in a frame where the earlier answer is already there.
    let stamp = t.landings(Kind::Chain);
    assert!(!t.answered_since(Kind::Chain, stamp), "the answer already there is not the press's");
    let go = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let held = go.clone();
    let _ = t.spawn(Kind::Chain, move || {
        while !held.load(std::sync::atomic::Ordering::SeqCst) {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        Err(Fault::known(Known::Unreachable, String::new()))
    });
    assert!(!t.answered_since(Kind::Chain, stamp), "in flight is not answered");
    t.forget(Kind::Chain);
    assert!(!t.answered_since(Kind::Chain, stamp), "a cleared mark is not an answer");
    go.store(true, std::sync::atomic::Ordering::SeqCst);
    land(&mut t, Kind::Chain);
    assert!(t.answered_since(Kind::Chain, stamp), "the landing after the press answers it");
}

/// Every kind of task has its own progress slot: entering a stage and reading it back works for each one.
#[test]
fn every_task_kind_has_a_progress_slot() {
    for k in app::task::Kind::ALL {
        app::task::stage_at(k, 1);
        assert_eq!(app::task::stage(k).map(|s| s.at), Some(1), "{} has no progress slot", k.as_str());
    }
}
