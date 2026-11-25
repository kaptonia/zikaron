use super::*;
use crate::rpc::{Broke, Endpoint, Trouble};
use crate::wire::{Body, W};
use std::sync::{Arc, Mutex};
use zikaron::json::Value;

#[derive(Clone)]
struct Limiting {
    place: String,
    limits: Arc<Mutex<bool>>,
    asked: Arc<Mutex<usize>>,
}

impl Limiting {
    fn new(place: &str) -> Limiting {
        Limiting { place: place.to_string(), limits: Arc::new(Mutex::new(true)), asked: Default::default() }
    }
    fn asked(&self) -> usize {
        *self.asked.lock().unwrap()
    }
    fn lets_go(&self) {
        *self.limits.lock().unwrap() = false;
    }
}

const WORDS: &str = r#"{"code":-32005,"message":"rate limit exceeded"}"#;

impl Endpoint for Limiting {
    fn call(&mut self, _: &str, _: &Value) -> Result<W, Trouble> {
        *self.asked.lock().unwrap() += 1;
        if *self.limits.lock().unwrap() {
            Err(Trouble::Node(WORDS.into()))
        } else {
            Ok(W::of(Body::Str("0x1".into())))
        }
    }
    fn name(&self) -> String {
        self.place.clone()
    }
    fn place(&self) -> String {
        self.place.clone()
    }
}

fn opened(node: &Limiting) -> Box<dyn Endpoint + Send> {
    remembering(&node.place, Box::new(node.clone()))
}

const M: &str = "eth_getTransactionReceipt";

#[test]
fn a_method_limited_past_the_table_rests_and_its_probe_doubles_the_rest_up_to_thirty_seconds() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let node = Limiting::new("rest-node");
    let mut ep = opened(&node);
    let q = Value::Arr(Vec::new());
    assert!(crate::patience::ask(ep.as_mut(), M, &q).is_err());
    assert_eq!(node.asked(), 3, "the patience table: three asks");
    assert_eq!(resting("rest-node", M), Some(Trouble::Node(WORDS.into())), "it rests, in the node's words");
    assert_eq!(crate::patience::ask(ep.as_mut(), M, &q), Err(Trouble::Node(WORDS.into())), "answered at once");
    assert_eq!(node.asked(), 3, "nobody asked it while it rests");
    let mut lengths = Vec::new();
    for _ in 0..6 {
        let mut waited = 0;
        while resting("rest-node", M).is_some() {
            move_limit_clock(std::time::Duration::from_secs(1));
            waited += 1;
        }
        lengths.push(waited);
        let before = node.asked();
        assert!(crate::patience::ask(ep.as_mut(), M, &q).is_err());
        assert_eq!(node.asked(), before + 1, "the probe: one ask, its limit given back at once");
    }
    assert_eq!(lengths, [2, 4, 8, 16, 30, 30], "two seconds, doubling, at most thirty");
    while resting("rest-node", M).is_some() {
        move_limit_clock(std::time::Duration::from_secs(1));
    }
    node.lets_go();
    assert!(crate::patience::ask(ep.as_mut(), M, &q).is_ok(), "the probe answered");
    assert!(resting("rest-node", M).is_none(), "the rest forgotten");
    let before = node.asked();
    assert!(crate::patience::ask(ep.as_mut(), M, &q).is_ok());
    assert_eq!(node.asked(), before + 1, "asked as before");
    assert!(resting("rest-node", "eth_blockNumber").is_none());
}

#[test]
fn a_batch_sends_no_question_of_a_resting_method_and_the_probe_goes_alone() {
    #[derive(Clone, Default)]
    struct Batch {
        posts: Arc<Mutex<Vec<Vec<String>>>>,
    }
    impl Endpoint for Batch {
        fn call(&mut self, _: &str, _: &Value) -> Result<W, Trouble> {
            Ok(W::of(Body::Null))
        }
        fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, Trouble>> {
            self.posts.lock().unwrap().push(asks.iter().map(|(m, _)| m.to_string()).collect());
            Some(Ok(asks.iter().map(|_| Some(Ok(W::of(Body::Str("0x1".into()))))).collect()))
        }
        fn name(&self) -> String {
            "batch-rest".into()
        }
        fn place(&self) -> String {
            "batch-rest".into()
        }
    }
    let node = Batch::default();
    let mut ep = remembering("batch-rest", Box::new(node.clone()));
    rest("batch-rest", M, &Trouble::Transport(Broke::Status { node: "batch-rest".into(), code: 429 }));
    let asks: Vec<crate::rpc::Ask<'_>> = vec![(M, Value::Arr(vec![Value::Str("a".into())])), (M, Value::Arr(vec![Value::Str("b".into())])), (M, Value::Arr(vec![Value::Str("c".into())])), ("eth_getBlockByNumber", Value::Arr(Vec::new()))];
    let got = ep.call_batch(&asks).expect("a batch").expect("answered");
    assert_eq!(node.posts.lock().unwrap().last().cloned(), Some(vec!["eth_getBlockByNumber".to_string()]), "only the block sent");
    assert!(got[..3].iter().all(|a| matches!(a, Some(Err(t)) if crate::patience::class_of(t) == crate::patience::Class::RateLimited)), "the receipts answered the rest's words: {got:?}");
    move_limit_clock(REST_FIRST);
    let _ = ep.call_batch(&asks);
    assert_eq!(node.posts.lock().unwrap().last().cloned(), Some(vec![M.to_string(), "eth_getBlockByNumber".to_string()]), "the probe alone of the receipts");
    assert!(resting("batch-rest", M).is_none(), "the probe answered: the rest forgotten");
}

#[test]
fn limited_in_an_action_is_a_limit_noted_since_it_opened() {
    let words = Trouble::Node(WORDS.into());
    note_limited("action-node", M, &words);
    move_limit_clock(std::time::Duration::from_millis(1));
    let _action = crate::action::open(None);
    assert!(!limited_in_action("action-node", M), "noted before it opened");
    move_limit_clock(std::time::Duration::from_millis(1));
    note_limited("action-node", M, &words);
    assert!(limited_in_action("action-node", M), "noted in it");
    assert!(!limited_in_action("action-node", "eth_getProof"), "by method");
    drop(_action);
    assert!(!limited_in_action("action-node", M), "outside an action, none");
}

struct StopsAfter {
    node: Limiting,
    mark: Arc<std::sync::atomic::AtomicBool>,
}

impl Endpoint for StopsAfter {
    fn call(&mut self, m: &str, p: &Value) -> Result<W, Trouble> {
        let got = self.node.call(m, p);
        self.mark.store(true, std::sync::atomic::Ordering::SeqCst);
        got
    }
    fn name(&self) -> String {
        self.node.place.clone()
    }
    fn place(&self) -> String {
        self.node.place.clone()
    }
}

fn read_stopped_short(node: &Limiting) -> Result<W, Trouble> {
    let mark = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut ep = remembering(&node.place, Box::new(StopsAfter { node: node.clone(), mark: mark.clone() }));
    under(vec![mark], || crate::patience::ask(ep.as_mut(), M, &Value::Arr(Vec::new())))
}

#[test]
fn a_node_that_keeps_limiting_rests_though_every_read_stops_short() {
    let node = Limiting::new("short-node");
    forget_limits_of("short-node");
    for _ in 0..3 {
        move_limit_clock(std::time::Duration::from_secs(1));
        assert!(read_stopped_short(&node).is_err());
    }
    assert_eq!(node.asked(), 3, "three reads, three limits in a row");
    assert!(resting("short-node", M).is_some(), "the method rests from the node");
    for _ in 0..3 {
        assert!(read_stopped_short(&node).is_err());
    }
    assert_eq!(node.asked(), 3, "nobody asked it while it rests");
    assert!(resting("short-node", M).is_some(), "the method rests from the node");
    while resting("short-node", M).is_some() {
        move_limit_clock(std::time::Duration::from_secs(1));
    }
    assert!(read_stopped_short(&node).is_err());
    assert_eq!(node.asked(), 4, "the rest over: one probe");
    assert!(resting("short-node", M).is_some(), "still limited: it rests again");
}

#[test]
fn limits_in_a_row_rest_at_the_count_and_span_of_the_table() {
    use crate::patience::RATE_LIMITED;
    use std::time::Duration;
    assert!(!limits_rest(3, Duration::ZERO, &RATE_LIMITED), "three side by side");
    assert!(!limits_rest(3, Duration::from_millis(1199), &RATE_LIMITED), "short of the pauses together");
    assert!(limits_rest(3, Duration::from_millis(1200), &RATE_LIMITED), "the moment the table is spent");
    assert!(limits_rest(5, Duration::from_secs(3), &RATE_LIMITED));
    assert!(!limits_rest(2, Duration::from_secs(60), &RATE_LIMITED), "two are not the table's three");
    assert!(limits_rest(3, Duration::ZERO, &[Duration::ZERO; 2]), "pauses set to zero");
    assert!(!limits_rest(2, Duration::ZERO, &[Duration::ZERO; 2]));
}

#[test]
fn an_answer_breaks_the_row_and_a_time_out_does_not() {
    let place = "row-node";
    forget_limits_of(place);
    let limit: Result<W, Trouble> = Err(Trouble::Node(WORDS.into()));
    let hear = |got: &Result<W, Trouble>| {
        move_limit_clock(std::time::Duration::from_secs(1));
        heard(place, M, got, false);
    };
    hear(&limit);
    hear(&limit);
    hear(&Ok(W::of(Body::Str("0x1".into()))));
    hear(&limit);
    hear(&limit);
    assert!(resting(place, M).is_none(), "the answer began the row again");
    hear(&limit);
    assert!(resting(place, M).is_some(), "three in a row since the answer");
    forget_limits_of(place);
    hear(&limit);
    hear(&Err(Trouble::Transport(Broke::Late { node: place.into(), deadline: std::time::Duration::from_secs(5) })));
    hear(&limit);
    assert!(resting(place, M).is_none());
    hear(&limit);
    assert!(resting(place, M).is_some(), "the time-out left the row as it was");
}

#[test]
fn a_batchs_limits_are_left_to_the_batch_table() {
    #[derive(Clone)]
    struct Limits;
    impl Endpoint for Limits {
        fn call(&mut self, _: &str, _: &Value) -> Result<W, Trouble> {
            Err(Trouble::Node(WORDS.into()))
        }
        fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, Trouble>> {
            Some(Ok(asks.iter().map(|_| Some(Err(Trouble::Node(WORDS.into())))).collect()))
        }
        fn name(&self) -> String {
            "batch-row".into()
        }
        fn place(&self) -> String {
            "batch-row".into()
        }
    }
    forget_limits_of("batch-row");
    let mut ep = remembering("batch-row", Box::new(Limits));
    let asks: Vec<crate::rpc::Ask<'_>> = ["a", "b", "c"].iter().map(|x| (M, Value::Arr(vec![Value::Str((*x).into())]))).collect();
    for _ in 0..5 {
        move_limit_clock(std::time::Duration::from_secs(1));
        let _ = ep.call_batch(&asks);
    }
    assert!(resting("batch-row", M).is_none(), "the batches rest nothing");
    move_limit_clock(std::time::Duration::from_secs(1));
    assert!(ep.call(M, &Value::Arr(Vec::new())).is_err());
    assert!(resting("batch-row", M).is_none(), "the single question's limit is the first in a row");
}
