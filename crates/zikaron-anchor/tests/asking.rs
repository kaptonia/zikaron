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
    assert_eq!(patience::class_of(&range_words()), Class::Other);
    assert_eq!(patience::class_of(&server()), Class::ServerError);
    assert_eq!(patience::class_of(&late()), Class::Timeout);
    assert_eq!(patience::class_of(&other()), Class::Other);
    assert_eq!(zikaron_anchor::said::RATE_BACKOFF, patience::RATE_LIMITED);
}

#[test]
fn each_class_is_asked_again_as_its_row_says_and_no_more() {
    zero_waits();
    let ok = Ok(w("\"0x1\""));
    let (mut n, asked) = node("a", vec![limited(), limited_words()], ok.clone());
    assert_eq!(patience::ask(&mut n, "eth_chainId", &Value::Arr(vec![])), ok);
    assert_eq!(asked.load(Ordering::SeqCst), 3);
    let (mut n, asked) = node("b", vec![limited(), limited(), limited()], ok.clone());
    assert_eq!(patience::ask(&mut n, "eth_chainId", &Value::Arr(vec![])), Err(limited()));
    assert_eq!(asked.load(Ordering::SeqCst), 3);
    let (mut n, asked) = node("c", vec![server(), server()], ok.clone());
    assert_eq!(patience::ask(&mut n, "eth_chainId", &Value::Arr(vec![])), Err(server()));
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    let (mut n, asked) = node("c2", vec![server()], ok.clone());
    assert_eq!(patience::ask(&mut n, "eth_chainId", &Value::Arr(vec![])), ok);
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    for t in [late(), other(), range_words()] {
        let (mut n, asked) = node("d", vec![t.clone()], ok.clone());
        assert_eq!(patience::ask(&mut n, "eth_chainId", &Value::Arr(vec![])), Err(t));
        assert_eq!(asked.load(Ordering::SeqCst), 1);
    }
    let (mut n, asked) = node("e", vec![server(), limited(), limited()], ok.clone());
    assert_eq!(patience::ask(&mut n, "eth_chainId", &Value::Arr(vec![])), ok);
    assert_eq!(asked.load(Ordering::SeqCst), 4);
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
    assert_eq!(judge::rule_of("eth_blockNumber", &no_params()), Rule::Reached);
    assert_eq!(judge::rule_of("eth_getTransactionCount", &no_params()), Rule::Most(None));
    assert_eq!(judge::rule_of("eth_estimateGas", &no_params()), Rule::Most(None));
    assert_eq!(judge::rule_of("eth_getBlockByNumber", &at("latest")), Rule::Most(Some("number")));
    assert_eq!(judge::rule_of("eth_getBlockByNumber", &at("0x40")), Rule::Facts(&judge::BLOCK_FACTS));
    assert_eq!(judge::rule_of("eth_feeHistory", &no_params()), Rule::Middle(judge::Statistic::PaidTip));
    assert_eq!(judge::rule_of("eth_getTransactionByHash", &no_params()), Rule::Facts(&judge::TX_FACTS));
    assert_eq!(judge::rule_of("eth_getTransactionReceipt", &no_params()), Rule::Facts(&judge::RECEIPT_FACTS));
    assert_eq!(judge::rule_of("eth_getBalance", &no_params()), Rule::Same);
    assert_eq!(judge::rule_of("eth_chainId", &no_params()), Rule::Same);
}

#[test]
fn same_bytes_least_and_most_read_as_the_table_says() {
    let j = judge::judge("eth_getBalance", &no_params(), answers(&[("http://a", Ok("\"0x5\"")), ("http://b", Ok("\"0x5\""))])).unwrap();
    assert_eq!((j.value, j.sources.len(), j.single_source), (Value::Str("0x5".into()), 2, false));
    let j = judge::judge("eth_getBalance", &no_params(), answers(&[("http://a", Ok("\"0x5\"")), ("http://b", Ok("\"0x5\"")), ("http://c", Ok("\"0x6\""))])).unwrap();
    assert_eq!((j.value, j.sources.len(), j.differed), (Value::Str("0x5".into()), 2, Vec::<String>::new()));
    let j = judge::judge("eth_blockNumber", &no_params(), answers(&[("http://a", Ok("\"0x41\"")), ("http://b", Ok("\"0x40\"")), ("http://c", Ok("\"x\""))])).unwrap();
    assert_eq!(j.value, Value::Str("0x40".into()));
    assert!(j.missing.is_empty(), "two places' heads name nobody beside them (A10): {:?}", j.missing);
    let j = judge::judge("eth_getTransactionCount", &no_params(), answers(&[("http://a", Ok("\"0x3\"")), ("http://b", Ok("\"0x7\""))])).unwrap();
    assert_eq!(j.value, Value::Str("0x7".into()));
    let latest = Value::Arr(vec![Value::Str("latest".into()), Value::Bool(false)]);
    let j = judge::judge("eth_getBlockByNumber", &latest, answers(&[("http://a", Ok("{\"number\":\"0x9\",\"timestamp\":\"0x2\"}")), ("http://b", Ok("{\"number\":\"0xa\",\"timestamp\":\"0x3\"}"))])).unwrap();
    assert_eq!(j.value.member("number").and_then(|v| v.as_str()), Some("0xa"));
}

#[test]
fn named_facts_compare_only_those_and_a_missing_member_differs() {
    let pinned = Value::Arr(vec![Value::Str("0x40".into()), Value::Bool(false)]);
    let j = judge::judge(
        "eth_getBlockByNumber",
        &pinned,
        answers(&[("http://a", Ok("{\"baseFeePerGas\":\"0x7\",\"size\":\"0x1\"}")), ("http://b", Ok("{\"baseFeePerGas\":\"0x7\",\"size\":\"0x2\"}"))]),
    )
    .unwrap();
    assert_eq!(j.value.member("baseFeePerGas").and_then(|v| v.as_str()), Some("0x7"));
    assert!(j.value.member("size").is_none());
    let d = judge::judge("eth_getBlockByNumber", &pinned, answers(&[("http://a", Ok("{\"baseFeePerGas\":\"0x7\"}")), ("http://b", Ok("{\"size\":\"0x2\"}"))]));
    assert!(matches!(d, Err(NoReading::Differ(_))));
}

#[test]
fn a_silent_node_is_passed_over_and_named_and_the_reading_is_single_source() {
    let j = judge::judge("eth_getBalance", &no_params(), answers(&[("http://a", Err(late())), ("http://b", Ok("\"0x5\""))])).unwrap();
    assert!(j.single_source);
    assert_eq!(j.sources, vec!["http://b".to_string()]);
    assert_eq!(j.missing, vec![("http://a".to_string(), Missing::Trouble(late()))]);
    let none = judge::judge("eth_getBalance", &no_params(), answers(&[("http://a", Err(late())), ("http://b", Err(other()))]));
    match none {
        Err(NoReading::NoneAnswered(m)) => assert_eq!(m, vec![("http://a".to_string(), Missing::Trouble(late())), ("http://b".to_string(), Missing::Trouble(other()))]),
        x => panic!("{x:?}"),
    }
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
        let j = judge::judge(method, &q, answers(&[("http://a", Ok("null")), ("http://b", Ok("null"))])).unwrap();
        assert_eq!(j.value, Value::Null);
    }
    zero_waits();
    assert_eq!(judge::not_yet_pauses(), vec![Duration::ZERO; judge::NOT_YET_PAUSES.len()]);
}

#[test]
fn an_unmined_transaction_is_not_yet_as_null_is() {
    let q = Value::Arr(vec![Value::Str("0xab".into())]);
    let mined = "{\"blockNumber\":\"0x1\",\"from\":\"0x2\",\"input\":\"0x\"}";
    let unmined = "{\"blockNumber\":null,\"from\":\"0x2\",\"input\":\"0x\"}";
    match judge::judge("eth_getTransactionByHash", &q, answers(&[("http://a", Ok(mined)), ("http://b", Ok(unmined))])) {
        Err(NoReading::NotYet { has, not_yet }) => assert_eq!((has, not_yet), (vec!["http://a".to_string()], vec!["http://b".to_string()])),
        x => panic!("{x:?}"),
    }
    let j = judge::judge("eth_getTransactionByHash", &q, answers(&[("http://a", Ok(unmined)), ("http://b", Ok("null"))])).unwrap();
    assert_eq!(j.value, Value::Null);
    let j = judge::judge("eth_getTransactionByHash", &q, answers(&[("http://a", Ok(unmined)), ("http://b", Ok(unmined))])).unwrap();
    assert_eq!(j.value.member("blockNumber"), Some(&Value::Null));
}

#[test]
fn not_yet_is_waited_for_at_least_a_block() {
    assert!(judge::NOT_YET_PAUSES.iter().sum::<Duration>() >= Duration::from_secs(12), "{:?}", judge::NOT_YET_PAUSES);
}

fn boxed(n: Node) -> (String, Box<dyn Endpoint + Send>) {
    (n.name.clone(), Box::new(n))
}

#[test]
fn one_question_goes_to_every_node_at_once_and_comes_back_in_table_order() {
    zero_waits();
    let ok = |s: &str| Ok(w(s));
    let mut nodes = Vec::new();
    for (i, v) in ["\"0x1\"", "\"0x2\"", "\"0x3\""].iter().enumerate() {
        let (mut n, _) = node(&format!("http://slow{i}"), vec![], ok(v));
        n.pause = Duration::from_millis(300);
        nodes.push(boxed(n));
    }
    let t = Instant::now();
    let got = zikaron_anchor::endpoints::all_in(endpoints::ask_each(nodes, "eth_blockNumber", &no_params(), zikaron_anchor::endpoints::every_answer));
    assert!(t.elapsed() < Duration::from_millis(800), "asked one after another: {:?}", t.elapsed());
    let names: Vec<&str> = got.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["http://slow0", "http://slow1", "http://slow2"]);
    assert_eq!(got.iter().map(|(_, a)| a.clone()).collect::<Vec<_>>(), vec![ok("\"0x1\""), ok("\"0x2\""), ok("\"0x3\"")]);
}

#[test]
fn the_forms_slow_silent_refusing_none_one_and_zero() {
    zero_waits();
    let ok = Ok(w("\"0x5\""));
    let (mut slow, _) = node("http://slow", vec![], ok.clone());
    slow.pause = Duration::from_millis(100);
    let (silent, _) = node("http://silent", vec![late()], ok.clone());
    let (refusing, refused) = node("http://refusing", vec![other()], ok.clone());
    let (good, _) = node("http://good", vec![], ok.clone());
    let got = zikaron_anchor::endpoints::all_in(endpoints::ask_each(vec![boxed(slow), boxed(silent), boxed(refusing), boxed(good)], "eth_getBalance", &no_params(), zikaron_anchor::endpoints::every_answer));
    assert_eq!(got.iter().map(|(_, a)| a.clone()).collect::<Vec<_>>(), vec![ok.clone(), Err(late()), Err(other()), ok.clone()]);
    assert_eq!(refused.load(Ordering::SeqCst), 1, "a refusal is never asked again");
    let j = judge::judge("eth_getBalance", &no_params(), got).unwrap();
    assert_eq!(j.sources, vec!["http://slow".to_string(), "http://good".to_string()]);
    assert!(j.missing.is_empty(), "two places alike name nobody beside them (A10): {:?}", j.missing);
    let (a, _) = node("http://a", vec![late()], ok.clone());
    let (b, _) = node("http://b", vec![other()], ok.clone());
    let got = zikaron_anchor::endpoints::all_in(endpoints::ask_each(vec![boxed(a), boxed(b)], "eth_getBalance", &no_params(), zikaron_anchor::endpoints::every_answer));
    assert!(matches!(judge::judge("eth_getBalance", &no_params(), got), Err(NoReading::NoneAnswered(m)) if m.len() == 2));
    let (one, asked) = node("http://one", vec![], ok.clone());
    let got = zikaron_anchor::endpoints::all_in(endpoints::ask_each(vec![boxed(one)], "eth_getBalance", &no_params(), zikaron_anchor::endpoints::every_answer));
    assert_eq!(asked.load(Ordering::SeqCst), 1);
    assert!(judge::judge("eth_getBalance", &no_params(), got).unwrap().single_source);
    assert!(endpoints::ask_each(Vec::new(), "eth_getBalance", &no_params(), zikaron_anchor::endpoints::every_answer).is_empty());
}

#[test]
fn a_question_in_flight_ends_with_its_slowest_node_and_leaves_nothing_running() {
    zero_waits();
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
        fn place(&self) -> String {
            self.name()
        }
    }
    let nodes: Vec<(String, Box<dyn Endpoint + Send>)> = (0..4).map(|i| (format!("http://t{i}"), Box::new(Tracked(alive.clone())) as Box<dyn Endpoint + Send>)).collect();
    let got = zikaron_anchor::endpoints::all_in(endpoints::ask_each(nodes, "eth_blockNumber", &no_params(), zikaron_anchor::endpoints::every_answer));
    assert_eq!(got.len(), 4);
    assert_eq!(alive.load(Ordering::SeqCst), 0, "a node was still being asked after the question returned");
}

#[test]
fn work_on_every_item_at_once_keeps_order_and_survives_a_worker_that_panics() {
    let got = endpoints::until_enough(vec![3u64, 1, 2], |n| { std::thread::sleep(Duration::from_millis(n * 20)); n * 10 }, || 0, endpoints::every);
    assert_eq!(got, vec![Some(30), Some(10), Some(20)]);
    let got = endpoints::until_enough(vec![1u64, 2, 3], |n| if n == 2 { panic!("a worker fell") } else { n }, || 99, endpoints::every);
    assert_eq!(got, vec![Some(1), Some(99), Some(3)]);
}

#[test]
fn enough_hands_back_at_once_and_the_one_on_its_way_finds_the_stop_set() {
    let late_saw_stop = Arc::new(AtomicUsize::new(0));
    let seen = late_saw_stop.clone();
    let t = Instant::now();
    let got = endpoints::until_enough(
        vec![0u64, 1, 2000],
        move |ms| {
            std::thread::sleep(Duration::from_millis(ms));
            if endpoints::stopped() {
                seen.fetch_add(1, Ordering::SeqCst);
            }
            ms
        },
        || u64::MAX,
        |slots: &[Option<u64>]| slots.iter().flatten().count() >= 2,
    );
    assert!(t.elapsed() < Duration::from_millis(1000), "waited for the held one: {:?}", t.elapsed());
    assert_eq!(got, vec![Some(0), Some(1), None]);
    assert!(!endpoints::stopped(), "the caller's thread is under no stop");
    std::thread::sleep(Duration::from_millis(2300));
    assert_eq!(late_saw_stop.load(Ordering::SeqCst), 1, "the held worker found its asking stopped");
}

#[test]
fn a_stop_reaches_the_askings_inside() {
    let inner_stopped = Arc::new(AtomicUsize::new(0));
    let seen = inner_stopped.clone();
    let _ = endpoints::until_enough(
        vec![0u64, 600],
        move |ms| {
            let seen = seen.clone();
            let _ = endpoints::until_enough(
                vec![ms, ms],
                move |ms| {
                    std::thread::sleep(Duration::from_millis(ms));
                    std::thread::sleep(Duration::from_millis(100));
                    if endpoints::stopped() {
                        seen.fetch_add(1, Ordering::SeqCst);
                    }
                },
                || (),
                endpoints::every,
            );
        },
        || (),
        |slots: &[Option<()>]| slots.iter().flatten().count() >= 1,
    );
    std::thread::sleep(Duration::from_millis(1100));
    assert_eq!(inner_stopped.load(Ordering::SeqCst), 2, "the inner workers of the outer one still on its way saw the stop");
}

#[test]
fn the_reading_crates_start_threads_in_one_place() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut found: Vec<(String, &str)> = Vec::new();
    for krate in ["zikaron-anchor", "zikaron-send"] {
        let mut stack = vec![root.join(krate).join("src")];
        while let Some(dir) = stack.pop() {
            for e in std::fs::read_dir(&dir).expect("the source").flatten() {
                let path = e.path();
                if path.is_dir() {
                    stack.push(path);
                    continue;
                }
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
                if !name.ends_with(".rs") || name == "tests.rs" || name.ends_with("_tests.rs") {
                    continue;
                }
                let text = std::fs::read_to_string(&path).expect("a source file");
                let shipped = text.split("#[cfg(test)]").next().unwrap_or_default();
                let code: String = shipped.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
                for needle in ["thread::spawn", "thread::Builder", "thread::scope", "spawn_scoped"] {
                    for _ in code.matches(needle) {
                        found.push((format!("{krate}/{name}"), needle));
                    }
                }
            }
        }
    }
    found.sort();
    let want: Vec<(String, &str)> = vec![
        ("zikaron-anchor/ahead.rs".to_string(), "spawn_scoped"),
        ("zikaron-anchor/ahead.rs".to_string(), "thread::Builder"),
        ("zikaron-anchor/ahead.rs".to_string(), "thread::scope"),
        ("zikaron-anchor/endpoints.rs".to_string(), "thread::Builder"),
    ];
    assert_eq!(found, want, "a thread started outside the one place");
}

struct Receipts(&'static str, &'static str);

impl Endpoint for Receipts {
    fn call(&mut self, _m: &str, _p: &Value) -> Result<W, Trouble> {
        Ok(w(self.1))
    }
    fn name(&self) -> String {
        self.0.into()
    }
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
    let (mut a, mut b) = (Receipts("http://has", receipt), Receipts("http://has2", receipt));
    let mut eps: Vec<&mut dyn Endpoint> = vec![&mut a, &mut b];
    assert!(matches!(confirm_each(&mut eps, &[0xab; 32], Duration::ZERO, &[]), Confirm::Included { status: 1, block_number: 5 }));
}

struct Behind {
    late: usize,
    asked: usize,
}

impl Endpoint for Behind {
    fn call(&mut self, _m: &str, _p: &Value) -> Result<W, Trouble> {
        self.asked += 1;
        Ok(w(if self.asked > self.late { "{\"status\":\"0x1\",\"blockNumber\":\"0x5\",\"logs\":[]}" } else { "null" }))
    }
    fn name(&self) -> String {
        "http://behind".into()
    }
    fn place(&self) -> String {
        self.name()
    }
}

#[test]
fn within_a_wait_a_split_is_asked_again_until_the_deadline() {
    use zikaron_anchor::send::{confirm_each, Confirm};
    zero_waits();
    let receipt = "{\"status\":\"0x1\",\"blockNumber\":\"0x5\",\"logs\":[]}";
    let pause = [Duration::from_millis(10)];
    let (mut a, mut b) = (Receipts("http://has", receipt), Behind { late: 5, asked: 0 });
    let mut eps: Vec<&mut dyn Endpoint> = vec![&mut a, &mut b];
    let began = std::time::Instant::now();
    assert!(matches!(confirm_each(&mut eps, &[0xab; 32], Duration::from_secs(5), &pause), Confirm::Included { status: 1, block_number: 5 }));
    assert!(began.elapsed() < Duration::from_secs(5), "included as soon as the node behind had it");
    let (mut a, mut b) = (Receipts("http://has", receipt), Receipts("http://not-yet", "null"));
    let mut eps: Vec<&mut dyn Endpoint> = vec![&mut a, &mut b];
    let wait = Duration::from_millis(300);
    let began = std::time::Instant::now();
    match confirm_each(&mut eps, &[0xab; 32], wait, &pause) {
        Confirm::Split { has, not_yet } => assert_eq!((has, not_yet), (vec!["http://has".to_string()], vec!["http://not-yet".to_string()])),
        x => panic!("{x:?}"),
    }
    assert!(began.elapsed() >= wait, "a split is named at the deadline, not before: {:?}", began.elapsed());
}

fn logs_over(from: u64, to: u64) -> Value {
    Value::Arr(vec![zikaron::json::parse(format!("{{\"fromBlock\":\"0x{from:x}\",\"toBlock\":\"0x{to:x}\"}}").as_bytes()).expect("a filter")])
}

#[test]
fn a_range_a_node_refused_is_asked_once_more_and_its_words_are_kept_for_nothing_after() {
    zero_waits();
    let said = Trouble::Node("{\"code\":-32012,\"message\":\"getLogs request exceeded max allowed range\"}".into());
    let (mut n, asked) = node("bounded-twice", vec![said.clone(), said.clone(), said.clone(), said.clone()], Ok(w("[]")));
    assert_eq!(patience::ask_again_once(&mut n, "eth_getLogs", &logs_over(0, 1000)), Err(said.clone()), "refused, asked once more, refused");
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    assert_eq!(patience::ask_again_once(&mut n, "eth_getLogs", &logs_over(5, 2000)), Err(said.clone()), "as wide or wider: asked again, its words not replayed");
    assert_eq!(asked.load(Ordering::SeqCst), 4);
    assert_eq!(patience::ask_again_once(&mut n, "eth_getLogs", &logs_over(0, 999)), Ok(w("[]")), "narrower: asked");
    assert_eq!(asked.load(Ordering::SeqCst), 5);
    let (mut once, asked) = node("bounded-once", vec![said.clone()], Ok(w("[]")));
    assert_eq!(patience::ask_again_once(&mut once, "eth_getLogs", &logs_over(0, 1000)), Ok(w("[]")));
    assert_eq!(patience::ask_again_once(&mut once, "eth_getLogs", &logs_over(0, 1000)), Ok(w("[]")));
    assert_eq!(asked.load(Ordering::SeqCst), 3, "answered on its one more ask: the range asked again");
}

const RECEIPT: &str = "{\"blockNumber\":\"0x10\",\"status\":\"0x1\"}";
const TX: &str = "{\"blockNumber\":\"0x10\",\"from\":\"0x2\",\"input\":\"0x\"}";
const BLOCK: &str = "{\"baseFeePerGas\":\"0x7\"}";

fn pinned() -> Value {
    Value::Arr(vec![Value::Str("0x10".into()), Value::Bool(false)])
}

fn by_hash() -> Value {
    Value::Arr(vec![Value::Str("0xab".into())])
}

#[test]
fn two_alike_stand_and_a_silent_third_is_only_missing() {
    let j = judge::judge("eth_getBalance", &no_params(), answers(&[("http://a", Ok("\"0x5\"")), ("http://b", Ok("\"0x5\"")), ("http://c", Err(late()))])).unwrap();
    assert_eq!((j.value, j.sources.len(), j.single_source), (Value::Str("0x5".into()), 2, false));
    assert!(j.missing.is_empty(), "{:?}", j.missing);
    assert!(j.differed.is_empty());
}

#[test]
fn a_third_nodes_null_is_no_answer_for_a_block_a_transaction_and_a_receipt() {
    for (method, params, given) in [("eth_getBlockByNumber", pinned(), BLOCK), ("eth_getTransactionByHash", by_hash(), TX), ("eth_getTransactionReceipt", by_hash(), RECEIPT)] {
        let j = judge::judge(method, &params, answers(&[("http://a", Ok(given)), ("http://b", Ok(given)), ("http://c", Ok("null"))])).unwrap_or_else(|e| panic!("{method}: {e:?}"));
        assert_eq!((j.sources.len(), j.single_source, j.differed.len()), (2, false, 0), "{method}");
        assert!(j.missing.is_empty(), "{method}: a null is no trouble");
    }
}

#[test]
fn the_head_is_the_highest_two_places_reached() {
    let head = |list: &[(&str, &str)]| judge::judge("eth_blockNumber", &no_params(), answers(&list.iter().map(|(n, h)| (*n, Ok(*h))).collect::<Vec<_>>())).unwrap().value;
    assert_eq!(head(&[("http://a", "\"0x41\""), ("http://b", "\"0x41\""), ("http://c", "\"0x40\"")]), Value::Str("0x41".into()));
    assert_eq!(head(&[("http://a", "\"0x42\""), ("http://b", "\"0x41\""), ("http://c", "\"0x40\"")]), Value::Str("0x41".into()));
    assert_eq!(head(&[("http://a", "\"0x42\""), ("http://b", "\"0x41\"")]), Value::Str("0x41".into()));
    assert_eq!(head(&[("http://a", "\"0x42\"")]), Value::Str("0x42".into()));
    assert_eq!(judge::reached(&[("http://a", 9u64), ("http://a", 9), ("http://b", 5)]), Some((2, 5)));
}

#[test]
fn a_third_real_answer_beside_two_alike_is_not_counted_and_named() {
    let other = "{\"blockNumber\":\"0x11\",\"status\":\"0x1\"}";
    let j = judge::judge("eth_getTransactionReceipt", &by_hash(), answers(&[("http://a", Ok(RECEIPT)), ("http://b", Ok(other)), ("http://c", Ok(RECEIPT))])).unwrap();
    assert_eq!(j.sources, vec!["http://a".to_string(), "http://c".to_string()]);
    assert!(j.differed.is_empty(), "{:?}", j.differed);
    assert!(j.missing.is_empty());
}

#[test]
fn two_against_two_and_two_unlike_in_a_table_of_two_differ() {
    let four = answers(&[("http://a", Ok("\"0x5\"")), ("http://b", Ok("\"0x6\"")), ("http://c", Ok("\"0x5\"")), ("http://d", Ok("\"0x6\""))]);
    assert!(matches!(judge::judge("eth_getBalance", &no_params(), four), Err(NoReading::Differ(_))));
    let two = answers(&[("http://a", Ok("\"0x5\"")), ("http://b", Ok("\"0x6\""))]);
    assert!(matches!(judge::judge("eth_getBalance", &no_params(), two), Err(NoReading::Differ(_))));
    let rows = answers(&[("http://a", Ok("\"0x5\"")), ("http://a", Ok("\"0x5\"")), ("http://b", Ok("\"0x6\""))]);
    assert!(matches!(judge::judge("eth_getBalance", &no_params(), rows), Err(NoReading::Differ(_))));
}

#[test]
fn one_place_giving_beside_a_null_waits_for_every_question_and_settles_as_before() {
    let one_has = |given: &str| answers(&[("http://a", Ok(given)), ("http://b", Ok("null")), ("http://c", Err(late()))]);
    let split = |r: Result<judge::Judged, NoReading>| match r {
        Err(NoReading::NotYet { has, not_yet }) => (has, not_yet),
        x => panic!("{x:?}"),
    };
    let named = (vec!["http://a".to_string()], vec!["http://b".to_string()]);
    for (method, params, given) in [("eth_getBlockByNumber", pinned(), BLOCK), ("eth_getBlockByHash", by_hash(), BLOCK), ("eth_getTransactionByHash", by_hash(), TX), ("eth_getTransactionReceipt", by_hash(), RECEIPT)] {
        assert_eq!(split(judge::judge(method, &params, one_has(given))), named, "{method}");
    }
    for (method, params) in [("eth_getBlockByNumber", pinned()), ("eth_getBlockByHash", by_hash())] {
        let settled = |a: Vec<(String, Result<W, Trouble>)>| judge::judge_settled(method, &params, a.into_iter().map(|(n, g)| (n, g.map_err(judge::Missing::from))).collect());
        let j = settled(one_has(BLOCK)).unwrap_or_else(|e| panic!("{method}: {e:?}"));
        assert_eq!((j.sources, j.single_source, j.value), (vec!["http://a".to_string()], true, zikaron::json::parse(BLOCK.as_bytes()).unwrap()), "{method}");
        let j = settled(answers(&[("http://a", Ok(BLOCK)), ("http://b", Ok("null"))])).unwrap();
        assert_eq!((j.sources.len(), j.single_source), (1, true), "{method}: a table of two");
    }
    for (method, given) in [("eth_getTransactionByHash", TX), ("eth_getTransactionReceipt", RECEIPT)] {
        let a: Vec<(String, Result<W, judge::Missing>)> = one_has(given).into_iter().map(|(n, g)| (n, g.map_err(judge::Missing::from))).collect();
        assert_eq!(split(judge::judge_settled(method, &by_hash(), a)), named, "{method}: still split after the waits, refused");
    }
}

#[test]
fn every_null_two_alike_two_apart_and_one_beside_failures_are_read_at_once() {
    let all_null = judge::judge("eth_getBlockByNumber", &pinned(), answers(&[("http://a", Ok("null")), ("http://b", Ok("null"))])).unwrap();
    assert_eq!(all_null.value, Value::Null);
    let two = judge::judge("eth_getBlockByNumber", &pinned(), answers(&[("http://a", Ok(BLOCK)), ("http://b", Ok(BLOCK)), ("http://c", Ok("null"))])).unwrap();
    assert_eq!((two.sources.len(), two.single_source), (2, false));
    let apart = judge::judge("eth_getBlockByNumber", &pinned(), answers(&[("http://a", Ok(BLOCK)), ("http://b", Ok("{\"baseFeePerGas\":\"0x9\"}")), ("http://c", Ok("null"))]));
    assert!(matches!(apart, Err(NoReading::Differ(_))), "{apart:?}");
    let alone = judge::judge("eth_getBlockByNumber", &pinned(), answers(&[("http://a", Ok(BLOCK)), ("http://b", Err(late())), ("http://c", Err(other()))])).unwrap();
    assert_eq!((alone.sources.len(), alone.single_source), (1, true));
    let one = judge::judge("eth_getBlockByNumber", &pinned(), answers(&[("http://a", Ok(BLOCK))])).unwrap();
    assert!(one.single_source);
}

#[test]
fn one_answer_alone_is_a_single_source_reading() {
    let j = judge::judge("eth_getBalance", &no_params(), answers(&[("http://a", Ok("\"0x5\""))])).unwrap();
    assert!(j.single_source);
    let j = judge::judge("eth_getBalance", &no_params(), answers(&[("http://a", Ok("\"0x5\"")), ("http://b", Err(late())), ("http://c", Err(late()))])).unwrap();
    assert_eq!((j.sources.len(), j.single_source), (1, true));
}

#[test]
fn two_rounds_alike_make_the_scan_and_the_third_is_left_out() {
    let v = |n: u64| Value::Obj(vec![("anchors".into(), Value::Arr(vec![Value::Int(n)]))]);
    let runs = vec![("1#0".to_string(), v(1)), ("1#1".to_string(), v(2)), ("1#2".to_string(), v(1))];
    let places = vec![vec!["http://a".to_string()], vec!["http://b".to_string()], vec!["http://c".to_string()]];
    let names: Vec<String> = runs.iter().map(|(n, _)| n.clone()).collect();
    let r = endpoints::agree_among(runs, &places, Vec::new()).expect("two alike");
    assert_eq!(r.fragment, v(1));
    assert_eq!(endpoints::left_out(&names, &r), vec!["1#1".to_string()]);
    assert!(!r.single_source);
    let one = endpoints::agree_among(vec![("1#0".to_string(), v(1))], &places[..1], vec![1]).expect("one round");
    assert!(one.single_source);
}

#[test]
fn enough_is_two_places_by_the_kind_of_read() {
    let got = |list: &[(&str, Result<&str, Trouble>)]| -> Vec<(String, Result<W, Missing>)> { list.iter().map(|(n, a)| (n.to_string(), a.clone().map(w).map_err(Missing::Trouble))).collect() };
    let bal = no_params();
    assert!(judge::enough("eth_getBalance", &bal, &got(&[("http://a", Ok("\"0x5\"")), ("http://b", Ok("\"0x5\""))]), None));
    assert!(!judge::enough("eth_getBalance", &bal, &got(&[("http://a", Ok("\"0x5\"")), ("http://b", Ok("\"0x6\""))]), None), "two that differ");
    assert!(!judge::enough("eth_getBalance", &bal, &got(&[("http://a", Ok("\"0x5\"")), ("http://a", Ok("\"0x5\""))]), None), "one place twice");
    assert!(!judge::enough("eth_getBalance", &bal, &got(&[("http://a", Ok("\"0x5\"")), ("http://b", Err(late()))]), None), "a trouble is no answer");
    assert!(!judge::enough("eth_getTransactionReceipt", &by_hash(), &got(&[("http://a", Ok("null")), ("http://b", Ok("null"))]), None), "two `null`s are not enough");
    assert!(judge::enough("eth_getTransactionReceipt", &by_hash(), &got(&[("http://a", Ok(RECEIPT)), ("http://c", Ok("null")), ("http://b", Ok(RECEIPT))]), None));
    let heads = got(&[("http://a", Ok("\"0x41\"")), ("http://b", Ok("\"0x3f\""))]);
    assert!(judge::enough("eth_blockNumber", &no_params(), &heads, None));
    assert!(!judge::enough("eth_blockNumber", &no_params(), &heads, Some(0x40)), "a head below the floor is not yet");
    assert!(judge::enough("eth_blockNumber", &no_params(), &heads, Some(0x3f)));
    assert!(judge::enough("eth_getTransactionCount", &no_params(), &got(&[("http://a", Ok("\"0x5\"")), ("http://b", Ok("\"0x7\""))]), None));
    assert!(!judge::enough("eth_getTransactionCount", &no_params(), &got(&[("http://a", Ok("\"0x5\"")), ("http://b", Ok("\"x\""))]), None), "an answer that is no number");
    assert_eq!(judge::enough_of("eth_feeHistory", &no_params()), judge::Enough::Conservative);
    assert_eq!(judge::enough_of("eth_getBlockByNumber", &Value::Arr(vec![Value::Str("0x40".into()), Value::Bool(false)])), judge::Enough::Alike);
}
