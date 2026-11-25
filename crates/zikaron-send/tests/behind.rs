use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use zikaron::json::Value;
use zikaron_anchor::rpc::{Endpoint, Trouble};
use zikaron_anchor::wire::W;
use zikaron_send::behind::{Audits, Job, Key, Prelude, State, Store};

type Kept = Arc<Mutex<Vec<(Key, String)>>>;

struct Shelf(Kept);

impl Store<String> for Shelf {
    fn load(&self, key: &Key) -> Option<String> {
        self.0.lock().unwrap().iter().find(|(k, _)| k == key).map(|(_, r)| r.clone())
    }
    fn save(&self, key: &Key, r: &String) {
        let mut s = self.0.lock().unwrap();
        s.retain(|(k, _)| k != key);
        s.push((*key, r.clone()));
    }
}

fn job(work: impl FnOnce() -> Result<String, String> + Send + 'static) -> Job<String, String> {
    Job { keeper: None, work: Box::new(work) }
}

fn key(n: u8) -> Key {
    Key::of(&[n], b"basis", b"pile")
}

#[test]
fn audits_asked_wait_in_one_queue_and_run_in_turn() {
    let audits: Audits<String, String> = Audits::new(None);
    let order = Arc::new(Mutex::new(Vec::new()));
    for n in [1u8, 2] {
        let o = order.clone();
        assert!(audits.ask(key(n), job(move || {
            o.lock().unwrap().push(n);
            Ok(format!("done {n}"))
        })));
    }
    assert!(!audits.ask(key(1), job(|| Ok("again".into()))), "an audit waiting is not asked twice");
    assert!(audits.waiting());
    assert_eq!(audits.run(), 2, "the two asks, in turn");
    assert_eq!(*order.lock().unwrap(), vec![1, 2]);
    assert_eq!(audits.state(&key(1)), State::Done("done 1".into()));
    assert_eq!(audits.state(&key(2)), State::Done("done 2".into()));
    assert!(!audits.waiting());
    assert!(audits.ask(key(1), job(|| Ok("done 1 again".into()))), "audit again, once done");
    audits.run();
    assert_eq!(audits.state(&key(1)), State::Done("done 1 again".into()));
}

#[test]
fn an_audit_is_not_audited_then_auditing_then_its_result() {
    let audits: Audits<String, String> = Audits::new(None);
    assert_eq!(audits.state(&key(7)), State::Unaudited);
    audits.ask(key(7), job(|| Err("chain not read".into())));
    assert_eq!(audits.state(&key(7)), State::Auditing, "asked, not done");
    audits.run();
    assert_eq!(audits.state(&key(7)), State::Failed("chain not read".into()));
    assert!(audits.ask(key(7), job(|| Ok("read".into()))), "a failed audit is asked again");
    assert_eq!(audits.state(&key(7)), State::Auditing);
    audits.run();
    assert_eq!(audits.state(&key(7)), State::Done("read".into()));
}

#[test]
fn a_result_is_kept_by_its_key_and_read_back_after_a_restart() {
    let kept: Kept = Arc::new(Mutex::new(Vec::new()));
    let first: Audits<String, String> = Audits::new(Some(Arc::new(Shelf(kept.clone()))));
    let k = Key::of(b"root", b"basis", b"pile one");
    first.ask(k, job(|| Ok("COMPLETE 312".into())));
    first.run();
    assert_eq!(kept.lock().unwrap().len(), 1, "given to the store as it ended");
    let again: Audits<String, String> = Audits::new(Some(Arc::new(Shelf(kept.clone()))));
    assert_eq!(again.state(&k), State::Unaudited, "memory alone: nothing read in a frame");
    assert!(again.recall(k));
    assert!(!again.recall(k), "one read back waiting is enough");
    again.run();
    assert_eq!(again.state(&k), State::Done("COMPLETE 312".into()));
    let newer = Key::of(b"root", b"basis", b"pile two");
    assert_ne!(newer, k);
    again.recall(newer);
    again.run();
    assert_eq!(again.state(&newer), State::Unaudited, "another pile, nothing kept for it");
    let bare: Audits<String, String> = Audits::new(None);
    bare.ask(k, job(|| Ok("in memory".into())));
    bare.run();
    assert_eq!(bare.state(&k), State::Done("in memory".into()));
}

#[test]
fn a_step_before_the_result_runs_first_and_its_error_is_the_audits() {
    let order = Arc::new(Mutex::new(Vec::new()));
    let o = order.clone();
    let audits: Audits<String, String> = Audits::new(None).with_prelude(Prelude {
        state: "structure",
        step: Box::new(move |k: &Key| {
            o.lock().unwrap().push("step");
            if *k == key(9) {
                return Err("structure broken".into());
            }
            Ok(())
        }),
    });
    let o2 = order.clone();
    audits.ask(key(1), job(move || {
        o2.lock().unwrap().push("audit");
        Ok("done".into())
    }));
    let ran = Arc::new(AtomicUsize::new(0));
    let r = ran.clone();
    audits.ask(key(9), job(move || {
        r.fetch_add(1, Ordering::SeqCst);
        Ok("never".into())
    }));
    audits.run();
    assert_eq!(*order.lock().unwrap(), vec!["step", "audit", "step"], "the step before each audit's work");
    assert_eq!(audits.state(&key(1)), State::Done("done".into()));
    assert_eq!(audits.state(&key(9)), State::Failed("structure broken".into()));
    assert_eq!(ran.load(Ordering::SeqCst), 0, "no work over a step that failed");
}

#[test]
fn the_steps_state_is_read_while_it_runs() {
    let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let g = gate.clone();
    let audits: Arc<Audits<String, String>> = Arc::new(Audits::new(None).with_prelude(Prelude {
        state: "structure",
        step: Box::new(move |_| {
            let (m, cv) = &*g;
            let mut go = m.lock().unwrap();
            while !*go {
                go = cv.wait(go).unwrap();
            }
            Ok(())
        }),
    }));
    audits.ask(key(4), job(|| Ok("after the step".into())));
    let runner = audits.clone();
    let t = std::thread::spawn(move || runner.run());
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while audits.state(&key(4)) != State::Before("structure") && std::time::Instant::now() < until {
        std::thread::yield_now();
    }
    assert_eq!(audits.state(&key(4)), State::Before("structure"), "the step's name while it runs");
    {
        let (m, cv) = &*gate;
        *m.lock().unwrap() = true;
        cv.notify_all();
    }
    t.join().unwrap();
    assert_eq!(audits.state(&key(4)), State::Done("after the step".into()));
}

struct Counting {
    asked: Arc<AtomicUsize>,
}

impl Endpoint for Counting {
    fn call(&mut self, _method: &str, _params: &Value) -> Result<W, Trouble> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        Ok(zikaron_anchor::wire::parse(b"\"0x10\"").unwrap())
    }
    fn name(&self) -> String {
        "http://counting.test".into()
    }
    fn place(&self) -> String {
        "counting.test".into()
    }
}

fn opened(asked: &Arc<AtomicUsize>, at: &str) -> Box<dyn Endpoint + Send> {
    zikaron_anchor::endpoints::remembering(at, Box::new(Counting { asked: asked.clone() }))
}

#[test]
fn an_audit_gives_way_to_a_read_in_front() {
    let front_asked = Arc::new(AtomicUsize::new(0));
    let behind_asked = Arc::new(AtomicUsize::new(0));
    let hold = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let started = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
    let (h, s, fa) = (hold.clone(), started.clone(), front_asked.clone());
    let front = std::thread::spawn(move || {
        let _action = zikaron_anchor::action::open(None);
        let mut ep = opened(&fa, "http://front.test");
        ep.call("eth_blockNumber", &Value::Arr(Vec::new())).unwrap();
        {
            let (m, cv) = &*s;
            *m.lock().unwrap() = true;
            cv.notify_all();
        }
        let (m, cv) = &*h;
        let mut go = m.lock().unwrap();
        while !*go {
            go = cv.wait(go).unwrap();
        }
    });
    {
        let (m, cv) = &*started;
        let mut go = m.lock().unwrap();
        while !*go {
            go = cv.wait(go).unwrap();
        }
    }
    assert!(zikaron_anchor::action::in_front() >= 1, "the read in front is counted from its first question");
    let audits: Arc<Audits<String, String>> = Arc::new(Audits::new(None));
    let ba = behind_asked.clone();
    audits.ask(key(5), job(move || {
        assert!(zikaron_anchor::action::is_behind(), "the audit runs as an action behind");
        let mut ep = opened(&ba, "http://behind.test");
        ep.call("eth_blockNumber", &Value::Arr(vec![Value::Str("again".into())])).map_err(|t| format!("{t:?}"))?;
        Ok("landed".into())
    }));
    let runner = audits.clone();
    let t = std::thread::spawn(move || runner.run());
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while zikaron_anchor::action::giving_way() == 0 && std::time::Instant::now() < until {
        std::thread::yield_now();
    }
    assert_eq!(zikaron_anchor::action::giving_way(), 1, "the audit's question gives way");
    assert_eq!(behind_asked.load(Ordering::SeqCst), 0, "the audit waits while a read in front is under way");
    assert_eq!(audits.state(&key(5)), State::Auditing);
    {
        let (m, cv) = &*hold;
        *m.lock().unwrap() = true;
        cv.notify_all();
    }
    front.join().unwrap();
    t.join().unwrap();
    assert_eq!(behind_asked.load(Ordering::SeqCst), 1, "once the read in front ends, the audit goes on");
    assert_eq!(audits.state(&key(5)), State::Done("landed".into()));
    assert_eq!(front_asked.load(Ordering::SeqCst), 1);
}
