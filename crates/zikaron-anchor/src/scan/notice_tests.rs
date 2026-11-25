use super::*;
use crate::settled::Keep;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use zikaron::json::canon_bytes;

const CHAIN: u64 = 1;
const FAILS_NOW: &str = "{\"code\":-32005,\"message\":\"query refused\"}";
const TOO_WIDE: &str = "{\"code\":-32012,\"message\":\"ErrGetLogsExceededMaxAllowedRange\",\"data\":\"maxAllowedRange 30000 requestRange 376868\"}";
const CONTRACT: [u8; 20] = [0x50; 20];
const BOOK: [u8; 20] = [0x60; 20];

fn venue() -> crate::revoke::Venue {
    crate::revoke::Venue { chain: CHAIN, contract: CONTRACT, from_block: 10, book: BOOK, book_from_block: 5 }
}

#[derive(Clone)]
struct Log {
    block: u64,
    tx_index: u64,
    address: [u8; 20],
    topics: Vec<[u8; 32]>,
    tx: [u8; 32],
}

#[derive(Default)]
struct World {
    logs: Vec<Log>,
    txs: BTreeMap<[u8; 32], (Vec<u8>, u64)>,
    version: BTreeMap<u64, u8>,
    code_declined: bool,
    status_zero: Option<[u8; 32]>,
}

impl World {
    fn hash_of(&self, block: u64) -> [u8; 32] {
        let mut b = block.to_be_bytes().to_vec();
        b.push(*self.version.get(&block).unwrap_or(&0));
        cryptox::keccak256(&b)
    }

    fn log_json(&self, l: &Log, index: u64) -> String {
        let topics: Vec<String> = l.topics.iter().map(|t| format!("\"{}\"", hexfmt::encode(t))).collect();
        format!(
            r#"{{"address":"{}","blockHash":"{}","blockNumber":"0x{:x}","data":"0x","logIndex":"0x{:x}","topics":[{}],"transactionHash":"{}","transactionIndex":"0x{:x}"}}"#,
            hex20(&l.address),
            hexfmt::encode(&self.hash_of(l.block)),
            l.block,
            index,
            topics.join(","),
            hexfmt::encode(&l.tx),
            l.tx_index
        )
    }

    fn accede(&mut self, block: u64, holder: u8) {
        let mut tx = [0u8; 32];
        tx[..8].copy_from_slice(&block.to_be_bytes());
        tx[8] = holder;
        tx[31] = 0xa5;
        let t0 = cryptox::keccak256(crate::revoke::ACCEDED_SIGNATURE.as_bytes());
        self.logs.push(Log { block, tx_index: 0, address: BOOK, topics: vec![t0, padded(&[holder; 20]), padded(&[holder.wrapping_add(1); 20]), padded(&[0x77; 20])], tx });
    }

    fn notice(&mut self, block: u64, k: u8, nonce: u64, grant: [u8; 32]) {
        let key = [k; 32];
        let sender = cryptox::address_of_privkey(&key).expect("a key");
        let data = crate::revoke::calldata(&[0x44; 32], &grant);
        let (raw, hash) = crate::send::sign_plain(&key, CHAIN, CONTRACT, 0, &data, crate::send::Fees::fallback(), nonce).expect("signed");
        let t0 = cryptox::keccak256(crate::revoke::REVOKED_SIGNATURE.as_bytes());
        self.logs.push(Log { block, tx_index: 1, address: CONTRACT, topics: vec![t0, padded(&sender), grant], tx: hash });
        self.txs.insert(hash, (raw, block));
    }

    fn answer(&self, method: &str, params: &Value) -> Result<W, Trouble> {
        let p = |i: usize| match params {
            Value::Arr(ps) => ps.get(i).cloned(),
            _ => None,
        };
        let qty = |v: Option<&Value>| v.and_then(Value::as_str).and_then(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok());
        let h32_of = |v: Option<Value>| v.as_ref().and_then(Value::as_str).and_then(hexfmt::decode).and_then(|b| <[u8; 32]>::try_from(b).ok());
        let text = match method {
            "eth_chainId" => "\"0x1\"".to_string(),
            "eth_getLogs" => {
                let f = p(0).unwrap_or(Value::Null);
                let (from, to) = (qty(f.member("fromBlock")).unwrap_or(0), qty(f.member("toBlock")).unwrap_or(0));
                let addrs: Vec<String> = f.member("address").and_then(Value::as_arr).map(|a| a.iter().filter_map(Value::as_str).map(str::to_ascii_lowercase).collect()).unwrap_or_default();
                let t0 = f.member("topics").and_then(Value::as_arr).and_then(|t| t.first()).and_then(Value::as_str).unwrap_or_default().to_ascii_lowercase();
                let mut by_block: BTreeMap<u64, u64> = BTreeMap::new();
                let mut out = Vec::new();
                let mut logs = self.logs.clone();
                logs.sort_by_key(|l| (l.block, l.tx_index));
                for l in &logs {
                    let index = by_block.entry(l.block).or_insert(0);
                    let i = *index;
                    *index += 1;
                    if (from..=to).contains(&l.block) && addrs.contains(&hex20(&l.address)) && hexfmt::encode(&l.topics[0]) == t0 {
                        out.push(self.log_json(l, i));
                    }
                }
                format!("[{}]", out.join(","))
            }
            "eth_getTransactionReceipt" => {
                let h = h32_of(p(0)).unwrap_or([0; 32]);
                match self.logs.iter().find(|l| l.tx == h) {
                    Some(l) => {
                        let status = if self.status_zero == Some(h) { "0x0" } else { "0x1" };
                        format!(r#"{{"blockHash":"{}","blockNumber":"0x{:x}","status":"{status}","transactionHash":"{}"}}"#, hexfmt::encode(&self.hash_of(l.block)), l.block, hexfmt::encode(&h))
                    }
                    None => "null".into(),
                }
            }
            "eth_getTransactionByHash" => {
                let h = h32_of(p(0)).unwrap_or([0; 32]);
                match self.txs.get(&h) {
                    Some((raw, block)) => tx_json(raw, &h, *block, &self.hash_of(*block)),
                    None => "null".into(),
                }
            }
            "eth_getCode" if self.code_declined => return Err(Trouble::Node("{\"code\":-32000,\"message\":\"missing trie node\"}".into())),
            "eth_getCode" => "\"0x\"".into(),
            "eth_getBlockByNumber" => {
                let n = qty(p(0).as_ref()).unwrap_or(0);
                format!(r#"{{"hash":"{}","number":"0x{n:x}"}}"#, hexfmt::encode(&self.hash_of(n)))
            }
            _ => "null".into(),
        };
        Ok(crate::wire::parse(text.as_bytes()).expect("json"))
    }
}

fn tx_json(raw: &[u8], hash: &[u8; 32], block: u64, block_hash: &[u8; 32]) -> String {
    let item = crate::rlp::decode_all(&raw[1..]).expect("rlp");
    let l = item.list().expect("a list");
    let q = |i: usize| {
        let b = l[i].bytes().unwrap_or(&[]);
        let h = hexfmt::encode(b);
        let digits = h.trim_start_matches("0x").trim_start_matches('0');
        format!("0x{}", if digits.is_empty() { "0" } else { digits })
    };
    let d = |i: usize| hexfmt::encode(l[i].bytes().unwrap_or(&[]));
    format!(
        r#"{{"accessList":[],"blockHash":"{}","blockNumber":"0x{block:x}","chainId":"{}","gas":"{}","hash":"{}","input":"{}","maxFeePerGas":"{}","maxPriorityFeePerGas":"{}","nonce":"{}","r":"{}","s":"{}","to":"{}","type":"0x2","value":"{}","yParity":"{}"}}"#,
        hexfmt::encode(block_hash),
        q(0),
        q(4),
        hexfmt::encode(hash),
        d(7),
        q(3),
        q(2),
        q(1),
        q(10),
        q(11),
        d(5),
        q(6),
        q(9)
    )
}

#[derive(Default)]
struct Tally {
    questions: Vec<String>,
    posts: usize,
    log_from: Vec<u64>,
    alone: Vec<String>,
}

impl Tally {
    fn of(&self, method: &str) -> usize {
        self.questions.iter().filter(|m| *m == method).count()
    }
}

struct Node {
    world: Arc<Mutex<World>>,
    tally: Arc<Mutex<Tally>>,
    batches: bool,
    refuses_logs: Arc<Mutex<bool>>,
    words: Arc<Mutex<&'static str>>,
    place: String,
}

impl Node {
    fn note(&self, method: &str, params: &Value) {
        let mut t = self.tally.lock().unwrap();
        t.questions.push(method.to_string());
        if method == "eth_getLogs" {
            let from = params.as_arr().and_then(|p| p.first()).and_then(|f| f.member("fromBlock")).and_then(Value::as_str).and_then(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok());
            t.log_from.push(from.unwrap_or(u64::MAX));
        }
    }
}

impl Endpoint for Node {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        {
            let mut t = self.tally.lock().unwrap();
            t.posts += 1;
            t.alone.push(method.to_string());
        }
        self.note(method, params);
        if method == "eth_getLogs" && *self.refuses_logs.lock().unwrap() {
            return Err(Trouble::Node((*self.words.lock().unwrap()).into()));
        }
        self.world.lock().unwrap().answer(method, params)
    }
    fn call_batch(&mut self, asks: &[crate::rpc::Ask<'_>]) -> Option<Result<crate::rpc::Batched, Trouble>> {
        if !self.batches {
            return None;
        }
        self.tally.lock().unwrap().posts += 1;
        let w = self.world.lock().unwrap();
        Some(Ok(asks
            .iter()
            .map(|(method, p)| {
                self.note(method, p);
                if *method == "eth_getLogs" && *self.refuses_logs.lock().unwrap() {
                    return Some(Err(Trouble::Node((*self.words.lock().unwrap()).into())));
                }
                Some(w.answer(method, p))
            })
            .collect()))
    }
    fn name(&self) -> String {
        self.place.clone()
    }
    fn place(&self) -> String {
        self.place.clone()
    }
}

struct Held(Mutex<std::collections::BTreeMap<String, Vec<u8>>>);

impl Keep for Held {
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

struct Table {
    world: Arc<Mutex<World>>,
    tallies: Vec<Arc<Mutex<Tally>>>,
    refuses: Vec<Arc<Mutex<bool>>>,
    words: Vec<Arc<Mutex<&'static str>>>,
    places: Vec<String>,
    batches: Vec<bool>,
}

impl Table {
    fn new(world: World, batches: &[bool]) -> Table {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let places = batches.iter().map(|_| format!("http://notice-node-{}.test/", N.fetch_add(1, std::sync::atomic::Ordering::SeqCst))).collect();
        Table {
            world: Arc::new(Mutex::new(world)),
            tallies: batches.iter().map(|_| Arc::new(Mutex::new(Tally::default()))).collect(),
            refuses: batches.iter().map(|_| Arc::new(Mutex::new(false))).collect(),
            words: batches.iter().map(|_| Arc::new(Mutex::new(FAILS_NOW))).collect(),
            places,
            batches: batches.to_vec(),
        }
    }

    fn read(&self, which: &[usize], to: u64) -> NoticesRead {
        for t in &self.tallies {
            *t.lock().unwrap() = Tally::default();
        }
        let eps: Vec<Box<dyn Endpoint + Send>> = which
            .iter()
            .map(|&i| crate::endpoints::remembering(&self.places[i], Box::new(Node { world: self.world.clone(), tally: self.tallies[i].clone(), batches: self.batches[i], refuses_logs: self.refuses[i].clone(), words: self.words[i].clone(), place: self.places[i].clone() })))
            .collect();
        notices_agreed(eps, &venue(), to)
    }

    fn tally(&self, i: usize) -> std::sync::MutexGuard<'_, Tally> {
        self.tallies[i].lock().unwrap()
    }
}

fn input_of(r: &NoticesRead) -> Value {
    assert!(r.refused.is_empty(), "every node answered: {:?}", r.refused);
    let first = r.read.first().expect("a reading").1.clone();
    assert!(r.read.iter().all(|(_, n)| canon_bytes(n) == canon_bytes(&first)), "the nodes read alike");
    first
}

fn whole(t: &Table, which: &[usize], to: u64) -> Vec<u8> {
    let _a = crate::action::open(None);
    canon_bytes(&input_of(&t.read(which, to)))
}

fn same(kept: &[u8], whole: &[u8], what: &str) {
    if kept != whole {
        let (k, w) = (zikaron::json::parse(kept).expect("json"), zikaron::json::parse(whole).expect("json"));
        for m in ["accessions", REVOCATIONS] {
            let (a, b) = (k.member(m).and_then(Value::as_arr).cloned().unwrap_or_default(), w.member(m).and_then(Value::as_arr).cloned().unwrap_or_default());
            let at = a.iter().zip(&b).position(|(x, y)| x != y);
            assert!(a.len() == b.len() && at.is_none(), "{what}: {m} kept {} whole {}, part at {at:?}: {:?} / {:?}", a.len(), b.len(), at.map(|i| String::from_utf8_lossy(&canon_bytes(&a[i])).into_owned()), at.map(|i| String::from_utf8_lossy(&canon_bytes(&b[i])).into_owned()));
        }
        panic!("{what}: the bases part");
    }
}

fn count(n: &Value, member: &str) -> usize {
    n.member(member).and_then(Value::as_arr).map_or(0, Vec::len)
}

fn chain_of(records: u64) -> World {
    let mut w = World::default();
    w.notice(20, 0x31, 0, [0xe1; 32]);
    w.notice(30, 0x32, 0, [0xe2; 32]);
    for i in 0..records {
        w.accede(40 + i * 3, (i % 200) as u8 + 1);
    }
    w
}

fn quiet() {
    crate::patience::set_waits(Some(std::time::Duration::ZERO));
}

#[test]
fn a_homes_second_read_asks_a_constant_and_new_records_add_their_own() {
    quiet();
    let mut seconds = Vec::new();
    for records in [8u64, 300] {
        let t = Table::new(chain_of(records), &[true, false]);
        let keep = Arc::new(Held(Mutex::new(Default::default())));
        let head = 40 + records * 3 + 200;
        {
            let _a = crate::action::open(Some(keep.clone()));
            let n = input_of(&t.read(&[0, 1], head));
            assert_eq!((count(&n, "accessions"), count(&n, REVOCATIONS)), (records as usize, 2));
            let (a, b) = (t.tally(0), t.tally(1));
            assert_eq!(a.of("eth_getTransactionReceipt"), records as usize + 2, "first read: a receipt per record");
            assert!(a.posts < a.questions.len(), "the batching node: fewer posts than questions");
            assert_eq!(b.posts, b.questions.len(), "the node taking no batch: one post a question");
        }
        let second = {
            let _a = crate::action::open(Some(keep.clone()));
            let n = t.read(&[0, 1], head);
            same(&canon_bytes(&input_of(&n)), &whole(&t, &[0, 1], head), "kept and whole read alike");
            let _a2 = crate::action::open(Some(keep.clone()));
            let _ = t.read(&[0, 1], head);
            let a = t.tally(0);
            assert_eq!(a.of("eth_getTransactionReceipt") + a.of("eth_getTransactionByHash") + a.of("eth_getCode"), 0, "nothing asked about a kept record");
            a.questions.len()
        };
        seconds.push(second);
        let mut last_head = head;
        for k in [5u64, 40] {
            let base = last_head + 1;
            for i in 0..k {
                t.world.lock().unwrap().accede(base + i, 0xc0);
            }
            let head = base + k + 200;
            {
                let _a = crate::action::open(Some(keep.clone()));
                let _ = t.read(&[0, 1], head);
            }
            let _a = crate::action::open(Some(keep.clone()));
            let n = t.read(&[0, 1], head + 1);
            let _ = input_of(&n);
            let a = t.tally(0);
            assert_eq!(a.questions.len(), second, "{records} records: read again, the same few questions");
            drop(a);
            let more = base + k + 300;
            for i in 0..k {
                t.world.lock().unwrap().accede(more + i, 0xd0);
            }
            let _a = crate::action::open(Some(keep.clone()));
            let n = t.read(&[0, 1], more + k + 200);
            let (receipts, questions) = {
                let a = t.tally(0);
                (a.of("eth_getTransactionReceipt"), a.questions.len())
            };
            assert_eq!(receipts, k as usize, "{k} new records: {k} receipts");
            assert_eq!(questions, second + k as usize, "{k} new records: their receipts and nothing else more");
            same(&canon_bytes(&input_of(&n)), &whole(&t, &[0, 1], more + k + 200), "kept and whole read alike, new records after");
            last_head = more + k + 200;
        }
    }
    assert_eq!(seconds[0], seconds[1], "8 records or 300: the same questions read again");
    assert!(seconds[0] <= 5, "the chain id, a block hash and a log question a form: {}", seconds[0]);
}

#[test]
fn the_two_forms_log_questions_go_to_a_node_in_one_post() {
    quiet();
    let t = Table::new(chain_of(8), &[true]);
    let keep = Arc::new(Held(Mutex::new(Default::default())));
    let head = 40 + 8 * 3 + 200;
    {
        let _a = crate::action::open(Some(keep.clone()));
        let _ = input_of(&t.read(&[0], head));
        let a = t.tally(0);
        assert_eq!(a.of("eth_getLogs"), 2);
        assert!(!a.alone.iter().any(|m| m == "eth_getLogs"), "the two forms' logs asked together: {:?}", a.alone);
    }
    let _a = crate::action::open(Some(keep));
    let n = t.read(&[0], head + 50);
    {
        let a = t.tally(0);
        assert_eq!((a.of("eth_getLogs"), a.of("eth_getBlockByNumber")), (2, 2), "each form's check and tail");
        assert_eq!(a.alone, vec!["eth_chainId".to_string()], "the chain id alone, the four in one post");
        assert_eq!(a.posts, 2);
    }
    same(&canon_bytes(&input_of(&n)), &whole(&t, &[0], head + 50), "kept and whole read alike");
}

#[test]
fn kept_and_whole_reads_are_one_through_new_blocks_and_a_replaced_tip() {
    quiet();
    let t = Table::new(chain_of(40), &[true, true]);
    let keep = Arc::new(Held(Mutex::new(Default::default())));
    let head = 400;
    {
        let mut w = t.world.lock().unwrap();
        w.accede(380, 0x91);
    }
    {
        let _a = crate::action::open(Some(keep.clone()));
        let _ = input_of(&t.read(&[0, 1], head));
    }
    {
        let mut w = t.world.lock().unwrap();
        w.logs.retain(|l| l.block != 380);
        w.version.insert(380, 1);
        w.accede(380, 0x92);
        w.accede(420, 0x93);
    }
    let kept = {
        let _a = crate::action::open(Some(keep.clone()));
        canon_bytes(&input_of(&t.read(&[0, 1], 450)))
    };
    same(&kept, &whole(&t, &[0, 1], 450), "the replaced tip read as it is now");
    assert!(String::from_utf8_lossy(&kept).contains(&hex20(&[0x92; 20])) && !String::from_utf8_lossy(&kept).contains(&hex20(&[0x91; 20])));
    let last = 380;
    {
        let mut w = t.world.lock().unwrap();
        w.logs.retain(|l| l.block != last);
        w.version.insert(last, 2);
        w.accede(last, 0x94);
    }
    let kept = {
        let _a = crate::action::open(Some(keep.clone()));
        let n = canon_bytes(&input_of(&t.read(&[0, 1], 460)));
        assert!(t.tally(0).log_from.iter().any(|f| *f < last), "read again from before the replaced block: {:?}", t.tally(0).log_from);
        n
    };
    same(&kept, &whole(&t, &[0, 1], 460), "the replaced block read as it is now");
}

#[test]
fn a_second_read_in_one_action_asks_only_after_the_first() {
    quiet();
    let t = Table::new(chain_of(60), &[true, true]);
    let head = 1000;
    let _a = crate::action::open(None);
    let _ = input_of(&t.read(&[0, 1], head));
    t.world.lock().unwrap().notice(1003, 0x33, 0, [0xe3; 32]);
    let n = input_of(&t.read(&[0, 1], 1005));
    assert_eq!(count(&n, REVOCATIONS), 3);
    let (a, b) = (t.tally(0), t.tally(1));
    for x in [&a, &b] {
        assert_eq!(x.of("eth_getBlockByNumber") + x.of("eth_chainId"), 0, "what this action read stands");
        assert_eq!(x.of("eth_getLogs"), 2, "one log question a form");
        assert!(x.log_from.iter().all(|f| *f == head - crate::settled::MARGIN + 1), "from past the settled margin: {:?}", x.log_from);
        assert_eq!((x.of("eth_getTransactionReceipt"), x.of("eth_getCode")), (1, 2), "the new notice's receipt and code, each node's own");
        assert!(x.of("eth_getTransactionByHash") <= 1, "the new notice's transaction, at most once a node");
    }
    assert_eq!(a.of("eth_getTransactionByHash") + b.of("eth_getTransactionByHash"), 1, "the new notice's transaction, of one node only");
    drop((a, b));
    drop(_a);
    assert_eq!(canon_bytes(&n), whole(&t, &[0, 1], 1005));
    let _a = crate::action::open(None);
    let _ = t.read(&[0, 1], 1005);
    assert!(t.tally(0).log_from.iter().all(|f| *f <= 10), "a new action with no keeper: whole");
}

#[test]
fn one_nodes_reading_serves_only_itself_and_a_split_or_silent_read_keeps_nothing() {
    quiet();
    let t = Table::new(chain_of(30), &[true, true]);
    let keep = Arc::new(Held(Mutex::new(Default::default())));
    {
        let _a = crate::action::open(Some(keep.clone()));
        let _ = input_of(&t.read(&[0], 500));
    }
    {
        let _a = crate::action::open(Some(keep.clone()));
        let _ = input_of(&t.read(&[0, 1], 500));
        assert!(t.tally(0).log_from.iter().all(|f| *f <= 10), "A and B: every block, A's own reading serving only A");
    }
    {
        let _a = crate::action::open(Some(keep.clone()));
        let _ = input_of(&t.read(&[0], 500));
        assert!(t.tally(0).log_from.iter().all(|f| *f > 400), "A alone again: from its kept part");
    }
    let t = Table::new(chain_of(30), &[true, true]);
    let keep = Arc::new(Held(Mutex::new(Default::default())));
    *t.refuses[1].lock().unwrap() = true;
    {
        let _a = crate::action::open(Some(keep.clone()));
        let r = t.read(&[0, 1], 500);
        assert_eq!((r.read.len(), r.refused.len()), (1, 1));
    }
    *t.refuses[1].lock().unwrap() = false;
    {
        let _a = crate::action::open(Some(keep.clone()));
        let _ = input_of(&t.read(&[0, 1], 500));
        assert!(t.tally(0).log_from.iter().all(|f| *f <= 10), "nothing was kept: whole");
    }
    let t = Table::new(chain_of(30), &[true, true]);
    let keep = Arc::new(Held(Mutex::new(Default::default())));
    let split = t.world.lock().unwrap().logs.iter().find(|l| l.address == BOOK).map(|l| l.tx).unwrap();
    {
        let _a = crate::action::open(Some(keep.clone()));
        let mut eps: Vec<Box<dyn Endpoint + Send>> = Vec::new();
        let mut other = chain_of(30);
        other.status_zero = Some(split);
        let other = Arc::new(Mutex::new(other));
        eps.push(crate::endpoints::remembering(&t.places[0], Box::new(Node { world: t.world.clone(), tally: t.tallies[0].clone(), batches: true, refuses_logs: t.refuses[0].clone(), words: t.words[0].clone(), place: t.places[0].clone() })));
        eps.push(crate::endpoints::remembering(&t.places[1], Box::new(Node { world: other, tally: t.tallies[1].clone(), batches: true, refuses_logs: t.refuses[1].clone(), words: t.words[1].clone(), place: t.places[1].clone() })));
        let r = notices_agreed(eps, &venue(), 500);
        assert_ne!(canon_bytes(&r.read[0].1), canon_bytes(&r.read[1].1), "read apart");
    }
    {
        let _a = crate::action::open(Some(keep.clone()));
        let _ = input_of(&t.read(&[0, 1], 500));
        assert!(t.tally(0).log_from.iter().all(|f| *f <= 10), "read apart: nothing kept, whole");
    }
}

fn marks(keep: &Held) -> Vec<bool> {
    let b = crate::settled::read_whole(keep);
    b.entries.values().map(|e| e.single_source).collect()
}

#[test]
fn a_node_that_refuses_the_range_holds_nothing_back_and_one_left_reads_single_source() {
    quiet();
    let t = Table::new(chain_of(30), &[true, true, true]);
    let keep = Arc::new(Held(Mutex::new(Default::default())));
    *t.refuses[2].lock().unwrap() = true;
    *t.words[2].lock().unwrap() = TOO_WIDE;
    {
        let _a = crate::action::open(Some(keep.clone()));
        let r = t.read(&[0, 1, 2], 500);
        assert_eq!((r.read.len(), r.refused.len()), (2, 0));
    }
    assert!(!marks(&keep).is_empty() && marks(&keep).iter().all(|single| !single), "kept as agreed: {:?}", marks(&keep));
    *t.refuses[2].lock().unwrap() = false;
    {
        let _a = crate::action::open(Some(keep.clone()));
        let r = t.read(&[0, 1, 2], 500);
        assert_eq!((r.read.len(), r.refused.len()), (2, 0));
        for i in 0..3 {
            let n = t.tally(i);
            assert!(n.log_from.len() <= 2 && n.log_from.iter().all(|f| *f > 400), "node {i}: only after the kept part: {:?}", n.log_from);
            assert!(n.questions.len() <= 5 && n.of("eth_getTransactionReceipt") == 0, "node {i}: a few questions: {:?}", n.questions);
        }
    }

    let t = Table::new(chain_of(30), &[true, true]);
    let keep = Arc::new(Held(Mutex::new(Default::default())));
    *t.refuses[1].lock().unwrap() = true;
    *t.words[1].lock().unwrap() = TOO_WIDE;
    {
        let _a = crate::action::open(Some(keep.clone()));
        let r = t.read(&[0, 1], 500);
        assert_eq!((r.read.len(), r.refused.len()), (1, 1));
    }
    assert!(!marks(&keep).is_empty() && marks(&keep).iter().all(|single| *single), "kept single-source: {:?}", marks(&keep));
    *t.refuses[1].lock().unwrap() = false;
    {
        let _a = crate::action::open(Some(keep.clone()));
        let _ = input_of(&t.read(&[0, 1], 500));
        for i in 0..2 {
            assert!(t.tally(i).log_from.iter().all(|f| *f > 400), "node {i}: served from the single-source part: {:?}", t.tally(i).log_from);
        }
    }
    assert!(marks(&keep).iter().all(|single| *single), "carried on, the weaker mark stays: {:?}", marks(&keep));

    let t = Table::new(chain_of(30), &[true, true, true]);
    let keep = Arc::new(Held(Mutex::new(Default::default())));
    *t.refuses[2].lock().unwrap() = true;
    *t.words[2].lock().unwrap() = TOO_WIDE;
    let split = t.world.lock().unwrap().logs.iter().find(|l| l.address == BOOK).map(|l| l.tx).unwrap();
    {
        let _a = crate::action::open(Some(keep.clone()));
        let mut other = chain_of(30);
        other.status_zero = Some(split);
        let other = Arc::new(Mutex::new(other));
        let mut eps: Vec<Box<dyn Endpoint + Send>> = Vec::new();
        eps.push(crate::endpoints::remembering(&t.places[0], Box::new(Node { world: t.world.clone(), tally: t.tallies[0].clone(), batches: true, refuses_logs: t.refuses[0].clone(), words: t.words[0].clone(), place: t.places[0].clone() })));
        eps.push(crate::endpoints::remembering(&t.places[1], Box::new(Node { world: other, tally: t.tallies[1].clone(), batches: true, refuses_logs: t.refuses[1].clone(), words: t.words[1].clone(), place: t.places[1].clone() })));
        eps.push(crate::endpoints::remembering(&t.places[2], Box::new(Node { world: t.world.clone(), tally: t.tallies[2].clone(), batches: true, refuses_logs: t.refuses[2].clone(), words: t.words[2].clone(), place: t.places[2].clone() })));
        let r = notices_agreed(eps, &venue(), 500);
        assert_eq!((r.read.len(), r.refused.len()), (2, 1));
        assert_ne!(canon_bytes(&r.read[0].1), canon_bytes(&r.read[1].1), "read apart");
    }
    assert!(crate::settled::read_whole(&*keep).entries.is_empty(), "nothing kept");

    let t = Table::new(chain_of(30), &[true, true, true]);
    let keep = Arc::new(Held(Mutex::new(Default::default())));
    *t.refuses[2].lock().unwrap() = true;
    {
        let _a = crate::action::open(Some(keep.clone()));
        let r = t.read(&[0, 1, 2], 500);
        assert_eq!((r.read.len(), r.refused.len()), (2, 0));
    }
    assert!(!crate::settled::read_whole(&*keep).entries.is_empty(), "two alike keep what they read whatever the third does");
    let t = Table::new(chain_of(30), &[true, true]);
    let keep = Arc::new(Held(Mutex::new(Default::default())));
    *t.refuses[1].lock().unwrap() = true;
    {
        let _a = crate::action::open(Some(keep.clone()));
        let r = t.read(&[0, 1], 500);
        assert!(!r.refused[0].1.stands_aside());
    }
    assert!(crate::settled::read_whole(&*keep).entries.is_empty(), "a failure of the moment keeps it all back");
}

#[test]
fn an_undecided_notice_is_read_again_next_time() {
    quiet();
    let mut w = chain_of(10);
    w.code_declined = true;
    let t = Table::new(w, &[true, true]);
    let keep = Arc::new(Held(Mutex::new(Default::default())));
    {
        let _a = crate::action::open(Some(keep.clone()));
        let n = input_of(&t.read(&[0, 1], 500));
        assert!(String::from_utf8_lossy(&canon_bytes(&n)).contains("UNPROVEN"));
    }
    t.world.lock().unwrap().code_declined = false;
    let _a = crate::action::open(Some(keep.clone()));
    let n = input_of(&t.read(&[0, 1], 500));
    assert!(!String::from_utf8_lossy(&canon_bytes(&n)).contains("UNPROVEN"), "decided this time");
    assert_eq!(t.tally(0).log_from, vec![20, 500 - crate::settled::MARGIN + 1], "the notice form from the undecided notice, the book's from past its margin");
}

struct InTheClear(Mutex<std::collections::BTreeMap<String, Vec<u8>>>);

impl Keep for InTheClear {
    fn load(&self, part: &str) -> Option<Vec<u8>> {
        self.0.lock().unwrap().get(part).cloned()
    }
    fn save(&self, part: &str, plain: &[u8]) -> Result<(), crate::settled::Unkept> {
        self.0.lock().unwrap().insert(part.to_string(), plain.to_vec());
        Ok(())
    }
    fn public(&self) -> bool {
        true
    }
}

#[test]
fn a_record_in_the_clear_names_no_node_and_serves_alike() {
    quiet();
    let t = Table::new(chain_of(20), &[true, true]);
    let keep = Arc::new(InTheClear(Mutex::new(Default::default())));
    {
        let _a = crate::action::open(Some(keep.clone()));
        let _ = input_of(&t.read(&[0, 1], 400));
    }
    let text: String = keep.0.lock().unwrap().values().map(|b| String::from_utf8(b.clone()).expect("text")).collect();
    assert!(!text.is_empty(), "kept");
    assert!(t.places.iter().all(|p| !text.contains(p.as_str()) && !text.contains(p.trim_start_matches("http://").trim_end_matches('/'))), "no node's address in the clear");
    assert!(text.contains("sha256:"), "each node by its digest");
    let _a = crate::action::open(Some(keep.clone()));
    let n = t.read(&[0, 1], 401);
    let asked = t.tally(0).questions.len();
    assert!(asked <= 5, "served from the record in the clear: {asked} questions");
    assert_eq!(canon_bytes(&input_of(&n)), whole(&t, &[0, 1], 401));
}
