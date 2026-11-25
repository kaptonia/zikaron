use super::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

const CHAIN: u64 = 1;

fn no_pauses() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Batches {
    Takes,
    Refuses,
    LimitedOnce,
    DropsFirst,
}

#[derive(Clone)]
struct Node {
    place: String,
    logs: BTreeMap<[u8; 20], Vec<u64>>,
    batches: Batches,
    refuses_logs: bool,
    odd: bool,
    tally: Arc<Mutex<Tally>>,
}

#[derive(Default)]
struct Tally {
    posts: usize,
    alone: Vec<String>,
    batched: Vec<String>,
    limited: bool,
}

fn hash_of(b: u64) -> String {
    format!("0x{b:064x}")
}

impl Node {
    fn answer(&self, method: &str, params: &Value) -> Result<W, Trouble> {
        let first = params.as_arr().and_then(|p| p.first());
        let n = |v: Option<&Value>| v.and_then(Value::as_str).and_then(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok());
        let text = match method {
            "eth_chainId" => "\"0x1\"".to_string(),
            "eth_getBlockByNumber" => {
                let b = n(first).unwrap_or(0);
                format!(r#"{{"hash":"{}","number":"0x{b:x}","timestamp":"0x1"}}"#, hash_of(b))
            }
            "eth_getLogs" if self.refuses_logs => return Err(Trouble::Node("{\"code\":-32000,\"message\":\"down\"}".into())),
            "eth_getLogs" => {
                let f = first.cloned().unwrap_or(Value::Null);
                let (from, to) = (n(f.member("fromBlock")).unwrap_or(0), n(f.member("toBlock")).unwrap_or(0));
                let asked: Vec<String> = f.member("address").and_then(Value::as_arr).map(|a| a.iter().filter_map(Value::as_str).map(str::to_ascii_lowercase).collect()).unwrap_or_default();
                let mut out = Vec::new();
                for (a, blocks) in &self.logs {
                    if !asked.contains(&hex20(a)) {
                        continue;
                    }
                    for b in blocks.iter().filter(|b| (from..=to).contains(*b)) {
                        let shown = if self.odd { b + 1 } else { *b };
                        out.push(format!(r#"{{"address":"{}","blockHash":"{}","blockNumber":"0x{shown:x}","data":"0x","logIndex":"0x0","topics":[],"transactionHash":"0x{b:064x}","transactionIndex":"0x0"}}"#, hex20(a), hash_of(shown)));
                    }
                }
                format!("[{}]", out.join(","))
            }
            _ => "null".into(),
        };
        Ok(crate::wire::parse(text.as_bytes()).expect("json"))
    }
}

impl Endpoint for Node {
    fn another(&self) -> Option<Box<dyn Endpoint + Send>> {
        Some(Box::new(self.clone()))
    }
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        {
            let mut t = self.tally.lock().unwrap();
            t.posts += 1;
            t.alone.push(method.to_string());
        }
        self.answer(method, params)
    }
    fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, Trouble>> {
        let mut t = self.tally.lock().unwrap();
        t.posts += 1;
        match self.batches {
            Batches::Refuses => return Some(Err(Trouble::Node("{\"code\":-32600,\"message\":\"batch requests are not supported\"}".into()))),
            Batches::LimitedOnce if !t.limited => {
                t.limited = true;
                return Some(Err(Trouble::Transport(crate::rpc::Broke::Status { node: self.place.clone(), code: 429 })));
            }
            _ => {}
        }
        t.batched.extend(asks.iter().map(|(m, _)| m.to_string()));
        drop(t);
        Some(Ok(asks.iter().enumerate().map(|(i, (m, p))| if i == 0 && self.batches == Batches::DropsFirst { None } else { Some(self.answer(m, p)) }).collect()))
    }
    fn name(&self) -> String {
        self.place.clone()
    }
    fn place(&self) -> String {
        self.place.clone()
    }
}

struct Shelf(Mutex<BTreeMap<String, Vec<u8>>>);

impl crate::settled::Keep for Shelf {
    fn load(&self, part: &str) -> Option<Vec<u8>> {
        self.0.lock().unwrap().get(part).cloned()
    }
    fn save(&self, part: &str, plain: &[u8]) -> Result<(), crate::settled::Unkept> {
        self.0.lock().unwrap().insert(part.to_string(), plain.to_vec());
        Ok(())
    }
    fn parts(&self) -> Vec<String> {
        self.0.lock().unwrap().keys().cloned().collect()
    }
}

fn filters(n: usize) -> Vec<LogFilter> {
    (0..n).map(|i| LogFilter { addresses: vec![emitter(i)], topics: Vec::new() }).collect()
}

fn emitter(i: usize) -> [u8; 20] {
    let mut a = [0x10u8; 20];
    a[..8].copy_from_slice(&(i as u64 + 1).to_be_bytes());
    a
}

fn node(place: &str, n: usize, batches: Batches, tally: &Arc<Mutex<Tally>>) -> Box<dyn Endpoint + Send> {
    let logs = (0..n).map(|i| (emitter(i), vec![5 + i as u64, 1050])).collect();
    crate::endpoints::remembering(place, Box::new(Node { place: place.to_string(), logs, batches, refuses_logs: false, odd: false, tally: tally.clone() }))
}

fn read_all(eps: &mut [Box<dyn Endpoint + Send>], fs: &[LogFilter], to: u64, ahead: bool) -> Vec<(usize, bool, usize, Vec<u8>)> {
    let places: Vec<String> = eps.iter().map(|e| e.place()).collect();
    let list = crate::settled::nodes_of(places.iter().map(String::as_str));
    if ahead {
        let reads: Vec<(&LogFilter, u64, u64)> = fs.iter().map(|f| (f, 0, to)).collect();
        let mut refs: Vec<&mut (dyn Endpoint + Send)> = eps.iter_mut().map(|e| e.as_mut()).collect();
        logs_ahead(&mut refs, CHAIN, &reads, Some(&list));
    }
    fs.iter()
        .map(|f| {
            let fresh: Vec<Box<dyn Endpoint + Send>> = eps.iter().map(|e| e.another().expect("the same node anew")).collect();
            let got = logs_agreed(fresh, CHAIN, f, 0, to);
            let bytes = got.logs.iter().map(|l| crate::wire::write(l)).collect::<Vec<_>>().join(",").into_bytes();
            (got.answered, got.agreed, got.refused.len(), bytes)
        })
        .collect()
}

#[allow(clippy::type_complexity)]
fn two_actions(n: usize, ahead: bool) -> ([usize; 2], [usize; 2], Vec<Vec<(usize, bool, usize, Vec<u8>)>>, BTreeMap<String, Vec<u8>>) {
    no_pauses();
    let shelf = Arc::new(Shelf(Mutex::new(BTreeMap::new())));
    let keep: Arc<dyn crate::settled::Keep> = shelf.clone();
    let fs = filters(n);
    let places = [format!("http://ahead-a-{n}.test/"), format!("http://ahead-b-{n}.test/")];
    let mut first = [0usize; 2];
    let mut second = [0usize; 2];
    let mut reads = Vec::new();
    for (round, to) in [(0usize, 1000u64), (1, 1100)] {
        let tallies = [Arc::new(Mutex::new(Tally::default())), Arc::new(Mutex::new(Tally::default()))];
        {
            let _action = crate::action::open(Some(keep.clone()));
            let mut eps: Vec<Box<dyn Endpoint + Send>> = (0..2).map(|i| node(&places[i], n, Batches::Takes, &tallies[i])).collect();
            reads.push(read_all(&mut eps, &fs, to, ahead));
        }
        let posts = [tallies[0].lock().unwrap().posts, tallies[1].lock().unwrap().posts];
        if round == 0 {
            first = posts;
        } else {
            second = posts;
        }
    }
    let parts = shelf.0.lock().unwrap().iter().filter(|(k, _)| k.as_str() != crate::settled::NOTES).map(|(k, v)| (k.clone(), v.clone())).collect();
    (first, second, reads, parts)
}

#[test]
fn a_group_of_filters_asks_each_node_in_batches_and_reads_and_keeps_as_one_by_one() {
    for n in [3usize, 25] {
        let (first, second, reads, parts) = two_actions(n, true);
        let (first_alone, second_alone, reads_alone, parts_alone) = two_actions(n, false);
        let up = |x: usize| x.div_ceil(crate::ahead::BATCH_MAX);
        assert_eq!(first, [up(n); 2], "nothing kept: one question a filter, in batches ({n} filters)");
        assert_eq!(second, [up(2 * n); 2], "every filter kept: its check and its tail, in batches ({n} filters)");
        assert_eq!(first_alone, [n; 2], "read one by one: a post a filter");
        assert_eq!(second_alone, [n; 2], "read one by one on kept entries: check and tail together, a post a filter");
        assert_eq!(reads, reads_alone, "the same logs, the same agreement");
        assert!(reads.iter().flatten().all(|(answered, agreed, refused, _)| *answered == 2 && *agreed && *refused == 0));
        assert!(!parts.is_empty());
        assert_eq!(parts, parts_alone, "the same keys, the same entries kept");
    }
}

fn one_action(tag: &str, n: usize, batches: Batches, b_logs: (bool, bool)) -> (Tally, Tally, Vec<(usize, bool, usize, Vec<u8>)>) {
    no_pauses();
    let fs = filters(n);
    let (ta, tb) = (Arc::new(Mutex::new(Tally::default())), Arc::new(Mutex::new(Tally::default())));
    let pa = format!("http://ahead-{tag}-a.test/");
    let pb = format!("http://ahead-{tag}-b.test/");
    let logs = |n: usize| (0..n).map(|i| (emitter(i), vec![5 + i as u64, 1050])).collect::<BTreeMap<_, _>>();
    let reads = {
        let _action = crate::action::open(None);
        let mut eps: Vec<Box<dyn Endpoint + Send>> = vec![
            node(&pa, n, batches, &ta),
            crate::endpoints::remembering(&pb, Box::new(Node { place: pb.clone(), logs: logs(n), batches: Batches::Takes, refuses_logs: b_logs.0, odd: b_logs.1, tally: tb.clone() })),
        ];
        read_all(&mut eps, &fs, 1100, true)
    };
    let take = |t: &Arc<Mutex<Tally>>| std::mem::take(&mut *t.lock().unwrap());
    (take(&ta), take(&tb), reads)
}

#[test]
fn a_node_that_refuses_batches_is_asked_one_question_at_a_time_and_remembered() {
    let (a, b, reads) = one_action("refuses", 3, Batches::Refuses, (false, false));
    assert_eq!(a.posts, 1 + 3, "the batch refused, then each filter alone: {:?}", a.alone);
    assert_eq!(a.alone.iter().filter(|m| *m == "eth_getLogs").count(), 3);
    assert_eq!(b.posts, 1, "the other node: one batch");
    assert!(reads.iter().all(|(answered, agreed, _, _)| *answered == 2 && *agreed));
    assert!(crate::ahead::unbatched().iter().any(|(p, m)| p == "http://ahead-refuses-a.test/" && m == crate::ahead::WHOLE), "remembered as taking no batch");
    let (again, _, _) = one_action("refuses", 3, Batches::Takes, (false, false));
    assert_eq!((again.posts, again.batched.len()), (3, 0), "never asked in batch again: {:?}", again.alone);
}

#[test]
fn a_rate_limited_batch_is_waited_out_by_the_patience_table() {
    let (a, _, reads) = one_action("limited", 3, Batches::LimitedOnce, (false, false));
    assert_eq!(a.posts, 2, "limited, then taken: {:?}", a.alone);
    assert!(a.alone.is_empty(), "nothing asked alone");
    assert!(reads.iter().all(|(_, agreed, _, _)| *agreed));
}

#[test]
fn a_question_the_batch_did_not_bring_is_asked_by_its_read() {
    let (a, _, reads) = one_action("drops", 3, Batches::DropsFirst, (false, false));
    assert_eq!(a.posts, 2, "the batch, then the one it did not bring: {:?}", a.alone);
    assert_eq!(a.alone, vec!["eth_getLogs".to_string()]);
    assert!(reads.iter().all(|(_, agreed, _, _)| *agreed));
}

#[test]
fn agreement_single_source_and_disagreement_are_each_reads_own() {
    let (_, _, alike) = one_action("alike", 3, Batches::Takes, (false, false));
    assert!(alike.iter().all(|(answered, agreed, refused, _)| (*answered, *agreed, *refused) == (2, true, 0)));
    let (_, b, single) = one_action("single", 3, Batches::Takes, (true, false));
    assert!(single.iter().all(|(answered, agreed, refused, _)| (*answered, *agreed, *refused) == (1, false, 1)), "one answered, the other named");
    assert!(b.posts >= 1);
    let (_, _, differ) = one_action("differ", 3, Batches::Takes, (false, true));
    assert!(differ.iter().all(|(answered, agreed, refused, _)| (*answered, *agreed, *refused) == (2, false, 0)), "two answers, not alike");
}

#[test]
fn an_entry_kept_with_no_log_asks_its_tail() {
    no_pauses();
    let keep: Arc<dyn crate::settled::Keep> = Arc::new(Shelf(Mutex::new(BTreeMap::new())));
    let tally = Arc::new(Mutex::new(Tally::default()));
    let place = "http://ahead-empty.test/";
    let f = LogFilter { addresses: vec![[0x99; 20]], topics: Vec::new() };
    {
        let _action = crate::action::open(Some(keep.clone()));
        let mut ep = node(place, 0, Batches::Takes, &tally);
        assert!(logs_over_window(ep.as_mut(), CHAIN, &f, 0, 1000).expect("read").is_empty());
    }
    let _next = crate::action::open(Some(keep));
    let ep = node(place, 0, Batches::Takes, &tally);
    let asks = log_asks(ep.as_ref(), CHAIN, &f, 0, 1100, None);
    assert_eq!(asks, vec![("eth_getLogs", logs_question(&f, 1000 - crate::settled::MARGIN + 1, 1100))], "the tail alone");
}

#[test]
fn a_scans_windows_on_one_chain_are_asked_ahead_together() {
    no_pauses();
    let tally = Arc::new(Mutex::new(Tally::default()));
    let basis = br#"{"chains":[{"chainId":1,"fromBlock":0,"registries":["0x1111111111111111111111111111111111111111"],"senders":["0x2222222222222222222222222222222222222222"],"toBlock":50},{"chainId":1,"fromBlock":51,"registries":["0x1111111111111111111111111111111111111111"],"senders":["0x3333333333333333333333333333333333333333"],"toBlock":100}]}"#;
    let _action = crate::action::open(None);
    let mut ep = node("http://ahead-scan.test/", 0, Batches::Takes, &tally);
    let mut eps: Vec<(u64, &mut dyn Endpoint)> = vec![(CHAIN, ep.as_mut())];
    let got = run(basis, &mut eps).expect("scanned");
    assert!(got.is_ok(), "a basis");
    let t = tally.lock().unwrap();
    assert_eq!(t.alone, vec!["eth_chainId".to_string()], "nothing else asked alone");
    assert_eq!((t.posts, t.batched.iter().filter(|m| *m == "eth_getLogs").count()), (2, 2), "both windows in one post");
}
