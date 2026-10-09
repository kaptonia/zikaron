//! The system refusing a thread does not take the window down. The task's kind lands at the next drain like any
//! other outcome, with the failure named (which task, the system's words); nothing is left in flight, and the
//! next start, once threads are available again, runs. A separate test binary, because the simulated failure
//! affects the whole process.

use app::fault::{Class, Fault, Known};
use app::task::{pretend_thread_fails, Kind, Tasks};

#[test]
fn a_thread_the_system_will_not_start_is_said_and_nothing_is_left_in_flight() {
    let mut t = Tasks::new();
    pretend_thread_fails(true);
    let ran = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = ran.clone();
    let spawned = t.spawn(Kind::Chain, move || {
        seen.store(true, std::sync::atomic::Ordering::SeqCst);
        Err(Fault::known(Known::Unreachable, String::new()))
    });
    pretend_thread_fails(false);
    assert_eq!(spawned.as_str(), "started", "the press is taken; its outcome comes through the drain");
    let out = t.drain_at(1.0);
    assert_eq!(out.len(), 1, "it lands at once");
    let f = out[0].result.as_ref().err().expect("a failure");
    assert_eq!((out[0].kind, f.class()), (Kind::Chain, Class::Unknown));
    assert!(f.tail().contains(Kind::Chain.as_str()) && f.tail().contains("would not start a thread"), "named: which task, the system's words: {}", f.tail());
    assert!(!ran.load(std::sync::atomic::Ordering::SeqCst), "the work never ran");
    assert!(!t.in_flight(Kind::Chain), "nothing left in flight");
    // Threads to be had again: the next start runs.
    let _ = t.spawn(Kind::Chain, || Err(Fault::known(Known::Unreachable, String::new())));
    let mut landed = false;
    for _ in 0..2000 {
        if t.drain_at(2.0).iter().any(|o| o.kind == Kind::Chain && o.result.as_ref().err().and_then(|f| f.which()) == Some(Known::Unreachable)) {
            landed = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(landed, "the next start runs");
}
