//! Asking several nodes: the patience table (`patience`), the judging table (`judge`) and one question asked of
//! every node at once (`endpoints::ask_each`). In-process nodes only; no network.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use zikaron::json::Value;
use zikaron_anchor::endpoints;
use zikaron_anchor::judge::{self, Missing, NoReading, Rule};
use zikaron_anchor::patience::{self, Class};
use zikaron_anchor::rpc::{self, Endpoint, Trouble};
use zikaron_anchor::wire::{self, W};

fn w(s: &str) -> W {
    wire::parse(s.as_bytes()).expect("a wire value")
}

/// A node that gives the troubles in `first`, one per ask, then `then`; every ask counted.
struct Node {
    name: String,
    first: Vec<Trouble>,
    then: Result<W, Trouble>,
    pause: Duration,
    asked: Arc<AtomicUsize>,
}

fn node(name: &str, first: Vec<Trouble>, then: Result<W, Trouble>) -> (Node, Arc<AtomicUsize>) {
    let asked = Arc::new(AtomicUsize::new(0));
    (Node { name: name.into(), first, then, pause: Duration::ZERO, asked: asked.clone() }, asked)
}

impl Endpoint for Node {
    fn call(&mut self, _method: &str, _params: &Value) -> Result<W, Trouble> {
        let n = self.asked.fetch_add(1, Ordering::SeqCst);
        if !self.pause.is_zero() {
            std::thread::sleep(self.pause);
        }
        match self.first.get(n) {
            Some(t) => Err(t.clone()),
            None => self.then.clone(),
        }
    }
    fn name(&self) -> String {
        self.name.clone()
    }
    /// No address: the place is the name.
    fn place(&self) -> String {
        self.name()
    }
}

fn limited() -> Trouble {
    rpc::status("http://n", 429)
}
fn limited_words() -> Trouble {
    Trouble::Node("{\"code\":-32005,\"message\":\"rate limit exceeded\"}".into())
}
fn range_words() -> Trouble {
    Trouble::Node("{\"code\":-32005,\"message\":\"block range limit exceeded\"}".into())
}
fn server() -> Trouble {
    rpc::status("http://n", 502)
}
fn late() -> Trouble {
    rpc::late("http://n", Duration::from_secs(1))
}
fn other() -> Trouble {
    Trouble::Node("{\"code\":-32601,\"message\":\"the method does not exist\"}".into())
}

fn zero_waits() {
    patience::set_waits(Some(Duration::ZERO));
}

#[test]
fn the_patience_table_has_one_row_per_class_and_reads_troubles_by_class() {
    assert_eq!(patience::TABLE.len(), 5);
    assert_eq!(patience::RATE_LIMITED, [Duration::from_millis(300), Duration::from_millis(900)]);
    assert_eq!(patience::SERVER_ERROR, [Duration::from_millis(300)]);
    assert_eq!(patience::class_of(&limited()), Class::RateLimited);
    assert_eq!(patience::class_of(&limited_words()), Class::RateLimited);
    // A range refusal that also reads as a limit being exceeded is about the range: never waited out.
    assert_eq!(patience::class_of(&range_words()), Class::Other);
    assert_eq!(patience::class_of(&server()), Class::ServerError);
    assert_eq!(patience::class_of(&late()), Class::Timeout);
    assert_eq!(patience::class_of(&other()), Class::Other);
    // The rate-limit pauses the send path reads are this table's row.
    assert_eq!(zikaron_anchor::said::RATE_BACKOFF, patience::RATE_LIMITED);
}

#[test]
fn each_class_is_asked_again_as_its_row_says_and_no_more() {
    zero_waits();
    let ok = Ok(w("\"0x1\""));
    // Rate limited twice, then answered: three asks, the answer.
    let (mut n, asked) = node("a", vec![limited(), limited_words()], ok.clone());
    assert_eq!(patience::ask(&mut n, "eth_chainId", &Value::Arr(vec![])), ok);
    assert_eq!(asked.load(Ordering::SeqCst), 3);
    // Still limited after the two pauses: the trouble is passed on after three asks.
    let (mut n, asked) = node("b", vec![limited(), limited(), limited()], ok.clone());
    assert_eq!(patience::ask(&mut n, "eth_chainId", &Value::Arr(vec![])), Err(limited()));
    assert_eq!(asked.load(Ordering::SeqCst), 3);
    // A server error: asked once more.
    let (mut n, asked) = node("c", vec![server(), server()], ok.clone());
    assert_eq!(patience::ask(&mut n, "eth_chainId", &Value::Arr(vec![])), Err(server()));
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    let (mut n, asked) = node("c2", vec![server()], ok.clone());
    assert_eq!(patience::ask(&mut n, "eth_chainId", &Value::Arr(vec![])), ok);
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    // A timeout and any other refusal: never asked again.
    for t in [late(), other(), range_words()] {
        let (mut n, asked) = node("d", vec![t.clone()], ok.clone());
        assert_eq!(patience::ask(&mut n, "eth_chainId", &Value::Arr(vec![])), Err(t));
        assert_eq!(asked.load(Ordering::SeqCst), 1);
    }
    // Each class counted apart: a server error, then two limits, then the answer.
    let (mut n, asked) = node("e", vec![server(), limited(), limited()], ok.clone());
    assert_eq!(patience::ask(&mut n, "eth_chainId", &Value::Arr(vec![])), ok);
    assert_eq!(asked.load(Ordering::SeqCst), 4);
    // The broadcast is never in the table: asked once, whatever comes back.
    let (mut n, asked) = node("f", vec![limited()], ok.clone());
    assert_eq!(patience::ask(&mut n, rpc::BROADCAST, &Value::Arr(vec![])), Err(limited()));
    assert_eq!(asked.load(Ordering::SeqCst), 1);
}

fn answers(list: &[(&str, Result<&str, Trouble>)]) -> Vec<(String, Result<W, Trouble>)> {
    list.iter().map(|(n, a)| (n.to_string(), a.clone().map(w))).collect()
}

fn no_params() -> Value {
    Value::Arr(vec![])
}

#[test]
fn the_judging_table_names_one_rule_per_method() {
    let at = |s: &str| Value::Arr(vec![Value::Str(s.into()), Value::Bool(false)]);
    assert_eq!(judge::rule_of("eth_blockNumber", &no_params()), Rule::Least);
    assert_eq!(judge::rule_of("eth_getTransactionCount", &no_params()), Rule::Most(None));
    assert_eq!(judge::rule_of("eth_getBlockByNumber", &at("latest")), Rule::Most(Some("number")));
    assert_eq!(judge::rule_of("eth_getBlockByNumber", &at("0x40")), Rule::Facts(&judge::BLOCK_FACTS));
    assert_eq!(judge::rule_of("eth_feeHistory", &no_params()), Rule::Facts(&judge::FEE_HISTORY_FACTS));
    assert_eq!(judge::rule_of("eth_getTransactionByHash", &no_params()), Rule::Facts(&judge::TX_FACTS));
    assert_eq!(judge::rule_of("eth_getTransactionReceipt", &no_params()), Rule::Facts(&judge::RECEIPT_FACTS));
    // Every method not named is byte for byte.
    assert_eq!(judge::rule_of("eth_getBalance", &no_params()), Rule::Same);
    assert_eq!(judge::rule_of("eth_chainId", &no_params()), Rule::Same);
}

#[test]
fn same_bytes_least_and_most_read_as_the_table_says() {
    // Byte for byte: alike agrees, unlike differs (never a majority).
    let j = judge::judge("eth_getBalance", &no_params(), answers(&[("http://a", Ok("\"0x5\"")), ("http://b", Ok("\"0x5\""))])).unwrap();
    assert_eq!((j.value, j.sources.len(), j.single_source), (Value::Str("0x5".into()), 2, false));
    let d = judge::judge("eth_getBalance", &no_params(), answers(&[("http://a", Ok("\"0x5\"")), ("http://b", Ok("\"0x5\"")), ("http://c", Ok("\"0x6\""))]));
    assert!(matches!(d, Err(NoReading::Differ(_))));
    // Least: the smallest head; a head that is not a quantity is passed over by its text.
    let j = judge::judge("eth_blockNumber", &no_params(), answers(&[("http://a", Ok("\"0x41\"")), ("http://b", Ok("\"0x40\"")), ("http://c", Ok("\"x\""))])).unwrap();
    assert_eq!(j.value, Value::Str("0x40".into()));
    assert_eq!(j.missing, vec![("http://c".to_string(), Missing::Unreadable("\"x\"".into()))]);
    // Most: the largest pending nonce.
    let j = judge::judge("eth_getTransactionCount", &no_params(), answers(&[("http://a", Ok("\"0x3\"")), ("http://b", Ok("\"0x7\""))])).unwrap();
    assert_eq!(j.value, Value::Str("0x7".into()));
    // Most by number: the latest block.
    let latest = Value::Arr(vec![Value::Str("latest".into()), Value::Bool(false)]);
    let j = judge::judge("eth_getBlockByNumber", &latest, answers(&[("http://a", Ok("{\"number\":\"0x9\",\"timestamp\":\"0x2\"}")), ("http://b", Ok("{\"number\":\"0xa\",\"timestamp\":\"0x3\"}"))])).unwrap();
    assert_eq!(j.value.member("number").and_then(|v| v.as_str()), Some("0xa"));
}

#[test]
fn named_facts_compare_only_those_and_a_missing_member_differs() {
    let pinned = Value::Arr(vec![Value::Str("0x40".into()), Value::Bool(false)]);
    // Members of their own do not differ; the fact alone is the reading.
    let j = judge::judge(
        "eth_getBlockByNumber",
        &pinned,
        answers(&[("http://a", Ok("{\"baseFeePerGas\":\"0x7\",\"size\":\"0x1\"}")), ("http://b", Ok("{\"baseFeePerGas\":\"0x7\",\"size\":\"0x2\"}"))]),
    )
    .unwrap();
    assert_eq!(j.value.member("baseFeePerGas").and_then(|v| v.as_str()), Some("0x7"));
    assert!(j.value.member("size").is_none());
    // A node lacking a named member differs.
    let d = judge::judge("eth_getBlockByNumber", &pinned, answers(&[("http://a", Ok("{\"baseFeePerGas\":\"0x7\"}")), ("http://b", Ok("{\"size\":\"0x2\"}"))]));
    assert!(matches!(d, Err(NoReading::Differ(_))));
}

#[test]
fn a_silent_node_is_passed_over_and_named_and_the_reading_is_single_source() {
    let j = judge::judge("eth_getBalance", &no_params(), answers(&[("http://a", Err(late())), ("http://b", Ok("\"0x5\""))])).unwrap();
    assert!(j.single_source);
    assert_eq!(j.sources, vec!["http://b".to_string()]);
    assert_eq!(j.missing, vec![("http://a".to_string(), Missing::Trouble(late()))]);
    // None answering: every node and why, in order.
    let none = judge::judge("eth_getBalance", &no_params(), answers(&[("http://a", Err(late())), ("http://b", Err(other()))]));
    match none {
        Err(NoReading::NoneAnswered(m)) => assert_eq!(m, vec![("http://a".to_string(), Missing::Trouble(late())), ("http://b".to_string(), Missing::Trouble(other()))]),
        x => panic!("{x:?}"),
    }
    // Zero nodes: none answered, nobody named.
    assert!(matches!(judge::judge("eth_getBalance", &no_params(), Vec::new()), Err(NoReading::NoneAnswered(m)) if m.is_empty()));
}

#[test]
fn not_yet_against_has_is_neither_a_difference_nor_a_reading() {
    let q = Value::Arr(vec![Value::Str("0xab".into())]);
    for method in judge::NOT_YET_METHODS {
        let split = judge::judge(method, &q, answers(&[("http://a", Ok("{\"blockNumber\":\"0x1\",\"from\":\"0x2\",\"input\":\"0x\",\"status\":\"0x1\"}")), ("http://b", Ok("null"))]));
        match split {
            Err(NoReading::NotYet { has, not_yet }) => assert_eq!((has, not_yet), (vec!["http://a".to_string()], vec!["http://b".to_string()])),
            x => panic!("{method}: {x:?}"),
        }
        // Every node "not yet": that is the reading.
        let j = judge::judge(method, &q, answers(&[("http://a", Ok("null")), ("http://b", Ok("null"))])).unwrap();
        assert_eq!(j.value, Value::Null);
    }
    // The pauses before asking again follow the patience table's test waits.
    zero_waits();
    assert_eq!(judge::not_yet_pauses(), vec![Duration::ZERO; judge::NOT_YET_PAUSES.len()]);
}

fn boxed(n: Node) -> (String, Box<dyn Endpoint + Send>) {
    (n.name.clone(), Box::new(n))
}

#[test]
fn one_question_goes_to_every_node_at_once_and_comes_back_in_table_order() {
    zero_waits();
    let ok = |s: &str| Ok(w(s));
    // Three slow nodes: together they take about one node's time, not three.
    let mut nodes = Vec::new();
    for (i, v) in ["\"0x1\"", "\"0x2\"", "\"0x3\""].iter().enumerate() {
        let (mut n, _) = node(&format!("http://slow{i}"), vec![], ok(v));
        n.pause = Duration::from_millis(300);
        nodes.push(boxed(n));
    }
    let t = Instant::now();
    let got = endpoints::ask_each(nodes, "eth_blockNumber", &no_params());
    assert!(t.elapsed() < Duration::from_millis(800), "asked one after another: {:?}", t.elapsed());
    let names: Vec<&str> = got.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["http://slow0", "http://slow1", "http://slow2"]);
    assert_eq!(got.iter().map(|(_, a)| a.clone()).collect::<Vec<_>>(), vec![ok("\"0x1\""), ok("\"0x2\""), ok("\"0x3\"")]);
}

#[test]
fn the_forms_slow_silent_refusing_none_one_and_zero() {
    zero_waits();
    let ok = Ok(w("\"0x5\""));
    // One slow, one silent (its deadline passed), one refusing, one answering: the answers come in order,
    // the slow one's included.
    let (mut slow, _) = node("http://slow", vec![], ok.clone());
    slow.pause = Duration::from_millis(100);
    let (silent, _) = node("http://silent", vec![late()], ok.clone());
    let (refusing, refused) = node("http://refusing", vec![other()], ok.clone());
    let (good, _) = node("http://good", vec![], ok.clone());
    let got = endpoints::ask_each(vec![boxed(slow), boxed(silent), boxed(refusing), boxed(good)], "eth_getBalance", &no_params());
    assert_eq!(got.iter().map(|(_, a)| a.clone()).collect::<Vec<_>>(), vec![ok.clone(), Err(late()), Err(other()), ok.clone()]);
    assert_eq!(refused.load(Ordering::SeqCst), 1, "a refusal is never asked again");
    let j = judge::judge("eth_getBalance", &no_params(), got).unwrap();
    assert_eq!(j.sources, vec!["http://slow".to_string(), "http://good".to_string()]);
    assert_eq!(j.missing.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["http://silent", "http://refusing"]);
    // None answering.
    let (a, _) = node("http://a", vec![late()], ok.clone());
    let (b, _) = node("http://b", vec![other()], ok.clone());
    let got = endpoints::ask_each(vec![boxed(a), boxed(b)], "eth_getBalance", &no_params());
    assert!(matches!(judge::judge("eth_getBalance", &no_params(), got), Err(NoReading::NoneAnswered(m)) if m.len() == 2));
    // Only one node: asked on this thread, single source.
    let (one, asked) = node("http://one", vec![], ok.clone());
    let got = endpoints::ask_each(vec![boxed(one)], "eth_getBalance", &no_params());
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    assert!(judge::judge("eth_getBalance", &no_params(), got).unwrap().single_source);
    // Zero nodes: nothing asked, nothing answered.
    assert!(endpoints::ask_each(Vec::new(), "eth_getBalance", &no_params()).is_empty());
}

#[test]
fn a_question_in_flight_ends_with_its_slowest_node_and_leaves_nothing_running() {
    zero_waits();
    // `ask_each` returns only when every node's answer (or trouble) is in: no thread outlives the question,
    // so a quit that waits for the task in flight waits for no more than the slowest node's deadline.
    let alive = Arc::new(AtomicUsize::new(0));
    struct Tracked(Arc<AtomicUsize>);
    impl Endpoint for Tracked {
        fn call(&mut self, _m: &str, _p: &Value) -> Result<W, Trouble> {
            self.0.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(50));
            self.0.fetch_sub(1, Ordering::SeqCst);
            Err(rpc::late("http://t", Duration::from_millis(50)))
        }
        fn name(&self) -> String {
            "t".into()
        }
        /// No address: the place is the name.
        fn place(&self) -> String {
            self.name()
        }
    }
    let nodes: Vec<(String, Box<dyn Endpoint + Send>)> = (0..4).map(|i| (format!("http://t{i}"), Box::new(Tracked(alive.clone())) as Box<dyn Endpoint + Send>)).collect();
    let got = endpoints::ask_each(nodes, "eth_blockNumber", &no_params());
    assert_eq!(got.len(), 4);
    assert_eq!(alive.load(Ordering::SeqCst), 0, "a node was still being asked after the question returned");
}

#[test]
fn work_on_every_item_at_once_keeps_order_and_survives_a_worker_that_panics() {
    let got = endpoints::each(vec![3u64, 1, 2], |n| { std::thread::sleep(Duration::from_millis(n * 20)); n * 10 }, || 0);
    assert_eq!(got, vec![30, 10, 20]);
    let got = endpoints::each(vec![1u64, 2, 3], |n| if n == 2 { panic!("a worker fell") } else { n }, || 99);
    assert_eq!(got, vec![1, 99, 3]);
}

/// A node answering every receipt question with `receipt`.
struct Receipts(&'static str, &'static str);

impl Endpoint for Receipts {
    fn call(&mut self, _m: &str, _p: &Value) -> Result<W, Trouble> {
        Ok(w(self.1))
    }
    fn name(&self) -> String {
        self.0.into()
    }
    /// No address: the place is the name.
    fn place(&self) -> String {
        self.name()
    }
}

#[test]
fn a_receipt_one_node_has_and_another_not_yet_is_split_naming_both_sides() {
    use zikaron_anchor::send::{confirm_each, Confirm};
    zero_waits();
    let receipt = "{\"status\":\"0x1\",\"blockNumber\":\"0x5\",\"logs\":[]}";
    let (mut a, mut b) = (Receipts("http://has", receipt), Receipts("http://not-yet", "null"));
    let mut eps: Vec<&mut dyn Endpoint> = vec![&mut a, &mut b];
    match confirm_each(&mut eps, &[0xab; 32], Duration::ZERO, &[]) {
        Confirm::Split { has, not_yet } => assert_eq!((has, not_yet), (vec!["http://has".to_string()], vec!["http://not-yet".to_string()])),
        x => panic!("{x:?}"),
    }
    // Both have it, alike: included.
    let (mut a, mut b) = (Receipts("http://has", receipt), Receipts("http://has2", receipt));
    let mut eps: Vec<&mut dyn Endpoint> = vec![&mut a, &mut b];
    assert!(matches!(confirm_each(&mut eps, &[0xab; 32], Duration::ZERO, &[]), Confirm::Included { status: 1, block_number: 5 }));
}
