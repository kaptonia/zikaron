use super::*;
use std::sync::{Arc, Mutex};

const CHAIN: u64 = 1;
const REGISTRY: [u8; 20] = [0x11; 20];

const PUBLICNODE_WIDE: &str = r#"{"code":-32701,"message":"exceed maximum block range: 50000"}"#;
const PUBLICNODE_PRUNED: &str = r#"{"code":4444,"message":"pruned history unavailable"}"#;
const ETHPANDAOPS_WIDE: &str = r#"{"code":-32012,"message":"getLogs request exceeded max allowed range","data":{"details":{"maxAllowedRange":30000}}}"#;
const NARROW_WIDE: &str = r#"{"code":-32600,"message":"Under the Free tier plan, you can make eth_getLogs requests with up to a 10 block range."}"#;
const TOO_MANY: &str = r#"{"code":-32005,"message":"query returned more than 2 results"}"#;
const LIMITED: &str = r#"{"code":-32005,"message":"rate limit exceeded"}"#;

#[derive(Clone)]
struct Node {
    place: String,
    blocks: Vec<u64>,
    bound: Option<(u64, &'static str)>,
    pruned_below: Option<u64>,
    most_logs: Option<usize>,
    limited: bool,
    asked: Arc<Mutex<Vec<(u64, u64)>>>,
}

impl Node {
    fn new(place: &str) -> Node {
        Node { place: place.to_string(), blocks: vec![3, 40_000, 70_001, 99_999], bound: None, pruned_below: None, most_logs: None, limited: false, asked: Default::default() }
    }
    fn bounded(place: &str, bound: u64, said: &'static str) -> Node {
        Node { bound: Some((bound, said)), ..Node::new(place) }
    }
    fn ranges(&self) -> Vec<(u64, u64)> {
        self.asked.lock().unwrap().clone()
    }
}

impl Endpoint for Node {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        if method == "eth_getBlockByNumber" {
            let n = params.as_arr().and_then(|p| p.first()).and_then(Value::as_str).and_then(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok()).unwrap_or(0);
            return Ok(crate::wire::parse(format!(r#"{{"hash":"0x{n:064x}","number":"0x{n:x}"}}"#).as_bytes()).expect("json"));
        }
        assert_eq!(method, "eth_getLogs");
        let f = params.as_arr().and_then(|p| p.first()).cloned().unwrap_or(Value::Null);
        let n = |k: &str| f.member(k).and_then(Value::as_str).and_then(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok()).unwrap_or(0);
        let (from, to) = (n("fromBlock"), n("toBlock"));
        self.asked.lock().unwrap().push((from, to));
        if self.limited {
            return Err(Trouble::Node(LIMITED.into()));
        }
        if let Some((_, said)) = self.bound.filter(|(b, _)| to - from + 1 > *b) {
            return Err(Trouble::Node(said.into()));
        }
        if self.pruned_below.is_some_and(|p| from < p) {
            return Err(Trouble::Node(PUBLICNODE_PRUNED.into()));
        }
        let held: Vec<u64> = self.blocks.iter().copied().filter(|b| (from..=to).contains(b)).collect();
        if self.most_logs.is_some_and(|m| held.len() > m) {
            return Err(Trouble::Node(TOO_MANY.into()));
        }
        let logs: Vec<String> = held.iter().map(|b| format!(r#"{{"address":"{}","blockHash":"0x{b:064x}","blockNumber":"0x{b:x}","data":"0x","logIndex":"0x0","topics":[],"transactionHash":"0x{b:064x}"}}"#, hex20(&REGISTRY))).collect();
        Ok(crate::wire::parse(format!("[{}]", logs.join(",")).as_bytes()).expect("json"))
    }
    fn name(&self) -> String {
        self.place.clone()
    }
    fn place(&self) -> String {
        self.place.clone()
    }
}

fn filter() -> LogFilter {
    LogFilter { addresses: vec![REGISTRY], topics: Vec::new() }
}

fn read(nodes: &[&Node], from: u64, to: u64) -> AgreedLogs {
    let eps: Vec<Box<dyn Endpoint + Send>> = nodes.iter().map(|n| Box::new((*n).clone()) as Box<dyn Endpoint + Send>).collect();
    logs_agreed(eps, CHAIN, &filter(), from, to)
}

fn blocks(logs: &[W]) -> Vec<u64> {
    logs.iter().filter_map(crate::settled::block_of).collect()
}

fn forget(nodes: &[&Node]) {
    for n in nodes {
        crate::patience::forget_answered(&n.place);
    }
}

#[derive(Default)]
struct Held(Mutex<std::collections::BTreeMap<String, Vec<u8>>>);

impl crate::settled::Keep for Held {
    fn load(&self, part: &str) -> Option<Vec<u8>> {
        self.0.lock().unwrap().get(part).cloned()
    }
    fn save(&self, part: &str, plain: &[u8]) -> Result<(), crate::settled::Unkept> {
        self.0.lock().unwrap().insert(part.to_string(), plain.to_vec());
        Ok(())
    }
}

#[test]
fn the_widest_node_that_cannot_answer_the_head_of_the_range_gives_way_to_the_one_that_reads_it() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let _action = crate::action::open(None);
    let mut wide = Node::bounded("judge-a-publicnode", 50_000, PUBLICNODE_WIDE);
    wide.pruned_below = Some(200_000);
    let narrow = Node::bounded("judge-a-ethpandaops", 30_000, ETHPANDAOPS_WIDE);
    let got = read(&[&wide, &narrow], 0, 264_472);
    forget(&[&wide, &narrow]);
    assert_eq!(got.answered, 1, "the node that read the range: {:?}", got.refused);
    assert_eq!(blocks(&got.logs), [3, 40_000, 70_001, 99_999], "every log, from the node that read it");
    let answered: Vec<(u64, u64)> = narrow.ranges().into_iter().filter(|(f, t)| t - f < 30_000).collect();
    assert!(answered.first().is_some_and(|r| r.0 == 0) && answered.last().is_some_and(|r| r.1 == 264_472) && answered.windows(2).all(|w| w[1].0 == w[0].1 + 1), "it read the range in pieces it answers, end to end: {:?}", narrow.ranges());
    assert!(wide.ranges().len() as u64 <= crate::action::ASKS_DEFAULT, "the widest asked within the cap: {}", wide.ranges().len());
}

#[test]
fn a_node_ten_blocks_wide_gives_way_after_a_handful_of_questions_and_alone_the_read_fails_by_the_cap() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let _action = crate::action::open(None);
    let narrow = Node::bounded("ten-blocks", 10, NARROW_WIDE);
    let got = read(&[&narrow], 0, 300_000);
    forget(&[&narrow]);
    assert_eq!(got.answered, 0);
    let cap = crate::action::ASKS_DEFAULT;
    assert!(got.refused.iter().any(|(_, r)| r.over_asks() == Some(cap)), "refused by the cap: {:?}", got.refused);
    let most = u64::from(cap.ilog2()) + 2;
    assert!(narrow.ranges().len() as u64 <= most, "at most log2 of the cap past the whole question and its once again: {}", narrow.ranges().len());
}

#[test]
fn a_node_answering_the_whole_range_leaves_every_node_asked_the_whole_range_alone() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let _action = crate::action::open(None);
    let whole = Node::new("whole-answers");
    let bounded = Node::bounded("whole-bounded", 30_000, ETHPANDAOPS_WIDE);
    let got = read(&[&whole, &bounded], 0, 100_000);
    forget(&[&whole, &bounded]);
    assert_eq!(blocks(&got.logs), [3, 40_000, 70_001, 99_999]);
    assert_eq!(whole.ranges(), [(0, 100_000)], "asked the whole range once");
    assert!(bounded.ranges().iter().all(|r| *r == (0, 100_000)), "never a piece: {:?}", bounded.ranges());
    assert!(!crate::action::splits("whole-bounded"), "no leave to cut");
}

#[test]
fn what_a_cut_read_settles_is_kept_and_the_same_node_is_asked_only_the_new_blocks_next() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let keep: Arc<Held> = Arc::new(Held::default());
    let _action = crate::action::open(Some(keep.clone()));
    let mut wide = Node::bounded("kept-publicnode", 50_000, PUBLICNODE_WIDE);
    wide.pruned_below = Some(200_000);
    let narrow = Node::bounded("kept-ethpandaops", 30_000, ETHPANDAOPS_WIDE);
    let first = read(&[&wide, &narrow], 0, 100_000);
    assert_eq!(first.answered, 1, "{:?}", first.refused);
    assert!(!keep.0.lock().unwrap().is_empty(), "the read kept though the other node gave way");
    narrow.asked.lock().unwrap().clear();
    let second = read(&[&wide, &narrow], 0, 100_500);
    forget(&[&wide, &narrow]);
    assert_eq!(blocks(&second.logs), [3, 40_000, 70_001, 99_999]);
    let settled = 100_000 - crate::settled::MARGIN;
    let logs_asked: Vec<(u64, u64)> = narrow.ranges();
    assert!(!logs_asked.is_empty() && logs_asked.iter().all(|(f, _)| *f > settled), "only what came after it: {logs_asked:?}");
}

#[test]
fn a_piece_refused_later_is_halved_and_the_new_width_goes_on() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let _action = crate::action::open(None);
    let mut counted = Node::new("counted-results");
    counted.blocks = vec![10, 70_001, 90_001];
    counted.most_logs = Some(1);
    let got = read(&[&counted], 0, 100_000);
    forget(&[&counted]);
    assert_eq!(blocks(&got.logs), [10, 70_001, 90_001], "read whole: {:?}", got.refused);
    let pieces: Vec<(u64, u64)> = counted.ranges().into_iter().filter(|r| *r != (0, 100_000)).collect();
    assert_eq!(pieces, [(0, 50_000), (50_001, 100_000), (50_001, 75_000), (75_001, 100_000)], "halved, then the later piece refused for its results halved again and the new width read on");
}

#[test]
fn a_cap_of_one_cuts_nothing_and_the_refusal_names_the_cap() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let _action = crate::action::open(None);
    crate::action::ask_at_most(1);
    let (a, b) = (Node::bounded("cap-one-a", 30_000, ETHPANDAOPS_WIDE), Node::bounded("cap-one-b", 50_000, PUBLICNODE_WIDE));
    let got = read(&[&a, &b], 0, 100_000);
    forget(&[&a, &b]);
    assert_eq!(got.answered, 0);
    assert!(a.ranges().iter().chain(b.ranges().iter()).all(|r| *r == (0, 100_000)), "no piece asked");
    assert_eq!(got.refused.iter().filter_map(|(_, r)| r.over_asks()).collect::<Vec<_>>(), [1, 1], "each gave way by the cap: {:?}", got.refused);
    let details: Vec<String> = got.refused.iter().map(|(_, r)| r.detail()).collect();
    assert!(details.iter().any(|d| d.contains(ETHPANDAOPS_WIDE)) && details.iter().any(|d| d.contains(PUBLICNODE_WIDE)), "each gave way with the words it said last: {details:?}");
}

#[test]
fn a_node_rate_limited_to_the_end_gives_way_and_what_the_other_read_is_used_and_kept() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let keep: Arc<Held> = Arc::new(Held::default());
    let _action = crate::action::open(Some(keep.clone()));
    let mut limited = Node::new("limited-to-the-end");
    limited.limited = true;
    let narrow = Node::bounded("limited-other", 30_000, ETHPANDAOPS_WIDE);
    let got = read(&[&limited, &narrow], 0, 100_000);
    forget(&[&limited, &narrow]);
    assert_eq!(blocks(&got.logs), [3, 40_000, 70_001, 99_999], "{:?}", got.refused);
    assert!(limited.ranges().iter().all(|r| *r == (0, 100_000)), "the rate limit is waited out, never cut: {:?}", limited.ranges());
    assert!(!keep.0.lock().unwrap().is_empty(), "kept");
}

#[test]
fn a_width_a_node_answered_is_where_its_next_read_begins() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let _action = crate::action::open(None);
    let narrow = Node::bounded("begins-at-its-width", 30_000, ETHPANDAOPS_WIDE);
    let first = read(&[&narrow], 0, 100_000);
    assert_eq!(blocks(&first.logs), [3, 40_000, 70_001, 99_999]);
    let kept = crate::patience::answered_width("begins-at-its-width").expect("the width it answered");
    assert!(kept <= 30_000);
    narrow.asked.lock().unwrap().clear();
    let again = read(&[&narrow], 0, 100_000);
    forget(&[&narrow]);
    assert_eq!(blocks(&again.logs), [3, 40_000, 70_001, 99_999]);
    assert!(narrow.ranges().iter().all(|(f, t)| t - f < kept), "not the whole range first: {:?}", narrow.ranges());
}

fn subject(n: u8) -> [u8; 32] {
    let mut s = [0u8; 32];
    s[31] = n;
    s
}

fn with_subjects(n: u8) -> LogFilter {
    LogFilter { addresses: vec![REGISTRY], topics: vec![Topic::One(topic0()), Topic::AnyOf((0..n).map(subject).collect())] }
}

#[test]
fn a_filter_of_many_subjects_is_read_group_by_group_and_a_full_groups_key_never_moves() {
    let thirty_two = with_subjects(32);
    assert_eq!(subject_groups(&thirty_two), vec![thirty_two.clone()], "a group's worth: the filter itself");
    assert_eq!(filter_key(&subject_groups(&thirty_two)[0]), filter_key(&thirty_two), "keyed as today");
    let g33 = subject_groups(&with_subjects(33));
    let g34 = subject_groups(&with_subjects(34));
    assert_eq!(g33.len(), 2);
    assert!(matches!(&g33[1].topics[1], Topic::AnyOf(xs) if xs.len() == 1));
    assert_eq!(filter_key(&g33[0]), filter_key(&g34[0]), "a full group's key unmoved");
    assert_eq!(filter_key(&g33[0]), filter_key(&thirty_two), "the first group is the 32 alone");
    let mut twice = with_subjects(32);
    if let Topic::AnyOf(xs) = &mut twice.topics[1] {
        xs.push(subject(0));
    }
    assert_eq!(subject_groups(&twice).len(), 1, "a subject given twice counted once");
}

const CODE_ONLY: &str = r#"{"code":-32601,"message":"Request contains invalid block params"}"#;

struct Says(std::sync::Arc<dyn Fn(&str, &Value) -> Result<W, Trouble> + Send + Sync>, String);

impl Endpoint for Says {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        (self.0)(method, params)
    }
    fn name(&self) -> String {
        self.1.clone()
    }
    fn place(&self) -> String {
        self.1.clone()
    }
}

fn ask_through(place: &str, method: &str, params: &Value, rule: impl Fn(&str, &Value) -> Result<W, Trouble> + Send + Sync + 'static) -> Result<W, Trouble> {
    let mut ep = crate::endpoints::remembering(place, Box::new(Says(std::sync::Arc::new(rule), place.to_string())));
    crate::patience::ask_again_once(&mut *ep, method, params)
}

#[test]
fn a_lack_stands_where_one_block_is_refused_by_the_code_or_the_words_say_so() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let _action = crate::action::open(None);
    let one_block = ask_through("code-one-block", "eth_getLogs", &logs_question(&filter(), 7, 7), |_, _| Err(Trouble::Node(CODE_ONLY.into())));
    assert!(one_block.is_err());
    assert!(crate::endpoints::lacking("code-one-block", "eth_getLogs").is_some(), "one block refused by the code alone: a lack");
    let words = r#"{"code":-32601,"message":"the method eth_getLogs does not exist/is not available"}"#;
    let wide = ask_through("words-wide", "eth_getLogs", &logs_question(&filter(), 0, 100_000), move |_, _| Err(Trouble::Node(words.into())));
    assert!(wide.is_err());
    assert!(crate::endpoints::lacking("words-wide", "eth_getLogs").is_some(), "words saying so: a lack, however wide the range");
    let other = ask_through("code-balance", "eth_getBalance", &Value::Arr(vec![Value::Str("0x00".into()), Value::Str("latest".into())]), |_, _| Err(Trouble::Node(CODE_ONLY.into())));
    assert!(other.is_err());
    assert!(crate::endpoints::lacking("code-balance", "eth_getBalance").is_some(), "another method by the code alone: a lack");
    let wide_code = ask_through("code-wide", "eth_getLogs", &logs_question(&filter(), 0, 100_000), |_, _| Err(Trouble::Node(CODE_ONLY.into())));
    assert!(wide_code.is_err());
    assert!(crate::endpoints::lacking("code-wide", "eth_getLogs").is_none(), "a range of blocks refused by the code alone is no lack");
}

#[test]
fn another_methods_refusal_naming_a_range_in_the_words_of_a_missing_method_is_a_lack() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let _action = crate::action::open(None);
    let words = r#"{"code":-32000,"message":"ranges over 10000 blocks are not supported on freetier"}"#;
    let got = ask_through("other-range-words", "eth_getBlockReceipts", &Value::Arr(vec![Value::Str("latest".into())]), move |_, _| Err(Trouble::Node(words.into())));
    assert!(got.is_err());
    assert!(crate::endpoints::lacking("other-range-words", "eth_getBlockReceipts").is_some(), "another method: a lack");
    assert!(crate::patience::lacks_the_method("eth_getBlockReceipts", &Trouble::Node(words.into())));
    assert!(!crate::patience::lacks_the_method("eth_getLogs", &Trouble::Node(words.into())), "a log question's: no lack");
}

#[test]
fn words_saying_the_method_is_not_supported_naming_no_range_are_a_lack() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
    let _action = crate::action::open(None);
    let words = r#"{"code":-32000,"message":"the method eth_getLogs is not supported"}"#;
    let wide = ask_through("words-not-supported", "eth_getLogs", &logs_question(&filter(), 0, 100_000), move |_, _| Err(Trouble::Node(words.into())));
    assert!(wide.is_err());
    assert!(crate::endpoints::lacking("words-not-supported", "eth_getLogs").is_some(), "no range named: a lack");
}
