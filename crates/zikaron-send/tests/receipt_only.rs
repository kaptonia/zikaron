use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use zikaron::json::Value;
use zikaron_anchor::rlp;
use zikaron_send::node::{Endpoint, NodeAddr};

const CHAIN: u64 = 11_155_111;
const NUMBER: u64 = 0x100;
const EMITTER: [u8; 20] = [0xee; 20];
const HASH: [u8; 32] = [0x5a; 32];

fn hex(b: &[u8]) -> String {
    zikaron::hexfmt::encode(b)
}

fn q(b: &[u8]) -> String {
    let s: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let t = s.trim_start_matches('0');
    format!("0x{}", if t.is_empty() { "0" } else { t })
}

fn qn(n: u64) -> String {
    format!("0x{n:x}")
}

fn w(json: &str) -> zikaron_anchor::wire::W {
    zikaron_anchor::wire::parse(json.as_bytes()).expect("json")
}

fn fees() -> zikaron_anchor::send::Fees {
    zikaron_anchor::send::Fees { max_fee: 2_000_000_000, priority: 1_000_000_000, gas_limit: 60_000, from_chain: true }
}

fn tx_object(raw: &[u8], block: &[u8; 32], index: u64) -> String {
    let item = rlp::decode_all(&raw[1..]).expect("rlp");
    let l = item.list().expect("list");
    let b = |i: usize| l[i].bytes().expect("bytes").to_vec();
    format!(
        r#"{{"blockHash":"{}","blockNumber":"{}","chainId":"{}","nonce":"{}","maxPriorityFeePerGas":"{}","maxFeePerGas":"{}","gas":"{}","to":"{}","value":"{}","input":"{}","accessList":[],"yParity":"{}","v":"{}","r":"{}","s":"{}","type":"0x2","hash":"{}","transactionIndex":"{}"}}"#,
        hex(block),
        qn(NUMBER),
        q(&b(0)),
        q(&b(1)),
        q(&b(2)),
        q(&b(3)),
        q(&b(4)),
        hex(&b(5)),
        q(&b(6)),
        hex(&b(7)),
        q(&b(9)),
        q(&b(9)),
        q(&b(10)),
        q(&b(11)),
        hex(&zikaron::cryptox::keccak256(raw)),
        qn(index)
    )
}

fn header_json(number: u64, parent: &[u8; 32], state: &[u8; 32], txs: &[u8; 32], receipts: &[u8; 32], time: u64) -> String {
    format!(
        r#""parentHash":"{}","sha3Uncles":"{}","miner":"{}","stateRoot":"{}","transactionsRoot":"{}","receiptsRoot":"{}","logsBloom":"{}","difficulty":"0x0","number":"{}","gasLimit":"0x1c9c380","gasUsed":"0xa410","timestamp":"{}","extraData":"0x","mixHash":"{}","nonce":"0x0000000000000000","baseFeePerGas":"0x7""#,
        hex(parent),
        hex(&zikaron::cryptox::keccak256(&rlp::list(&[]))),
        hex(&[0x99; 20]),
        hex(state),
        hex(txs),
        hex(receipts),
        hex(&[0u8; 256]),
        qn(number),
        qn(time),
        hex(&[0u8; 32])
    )
}

struct World {
    who: [u8; 20],
    tx: [u8; 32],
    block: [u8; 32],
    raws: Vec<Vec<u8>>,
    receipt: String,
    header_obj: String,
    parent_obj: String,
    state_proof: Vec<Vec<u8>>,
}

fn world() -> World {
    let k1 = [0x11u8; 32];
    let k2 = [0x22u8; 32];
    let who = zikaron::cryptox::address_of_privkey(&k1).expect("an address");
    let mut calldata = vec![0xee, 0xcd, 0xf9, 0x27];
    calldata.extend_from_slice(&HASH);
    let (raw1, h1) = zikaron_anchor::send::sign_plain(&k1, CHAIN, EMITTER, 0, &calldata, fees(), 0).expect("signed");
    let (raw2, _) = zikaron_anchor::send::sign_plain(&k2, CHAIN, [0x33; 20], 5, &[], fees(), 7).expect("signed");
    let raws = vec![raw2, raw1];
    let mut topic1 = [0u8; 32];
    topic1[12..].copy_from_slice(&who);
    let receipt = |cum: &str, logs: &str| format!(r#"{{"type":"0x2","status":"0x1","cumulativeGasUsed":"{cum}","logsBloom":"{}","logs":[{logs}]}}"#, hex(&[0u8; 256]));
    let log = format!(r#"{{"address":"{}","topics":["{}","{}","{}"],"data":"0x"}}"#, hex(&EMITTER), hex(&zikaron_anchor::scan::topic0()), hex(&topic1), hex(&HASH));
    let receipts = [receipt("0x5208", ""), receipt("0xa410", &log)];
    let receipt_bytes: Vec<Vec<u8>> = receipts.iter().map(|r| zikaron_anchor::proofs::receipt_bytes(&w(r)).expect("a receipt")).collect();
    let empty_root = zikaron_anchor::trie::root_of(&[]);
    let account = |nonce: u64, code: &[u8]| rlp::list(&[rlp::quantity(nonce), rlp::quantity(1_000_000), rlp::bytes(&empty_root), rlp::bytes(&zikaron::cryptox::keccak256(code))]);
    let pairs = vec![(zikaron::cryptox::keccak256(&who).to_vec(), account(1, &[])), (zikaron::cryptox::keccak256(&[0x44u8; 20]).to_vec(), account(0, &[0x60, 0x00]))];
    let (state, state_proof) = zikaron_anchor::trie::keyed_root_and_proof(&pairs, &zikaron::cryptox::keccak256(&who)).expect("a state");
    let parent_fields = header_json(NUMBER - 1, &[0x01; 32], &state, &empty_root, &empty_root, 0x5ff0);
    let parent = zikaron_anchor::ahead::header_rlp(&w(&format!("{{{parent_fields}}}"))).expect("a header");
    let parent_hash = zikaron::cryptox::keccak256(&parent);
    let fields = header_json(NUMBER, &parent_hash, &state, &zikaron_anchor::trie::root_of(&raws), &zikaron_anchor::trie::root_of(&receipt_bytes), 0x6000);
    let header = zikaron_anchor::ahead::header_rlp(&w(&format!("{{{fields}}}"))).expect("a header");
    let block = zikaron::cryptox::keccak256(&header);
    World {
        who,
        tx: h1,
        block,
        raws,
        receipt: receipts[1].clone(),
        header_obj: fields,
        parent_obj: format!(r#"{{{parent_fields},"hash":"{}","transactions":[]}}"#, hex(&parent_hash)),
        state_proof,
    }
}

impl World {
    fn block_obj(&self, full: bool) -> String {
        let txs: Vec<String> = self.raws.iter().enumerate().map(|(i, r)| if full { tx_object(r, &self.block, i as u64) } else { format!("\"{}\"", hex(&zikaron::cryptox::keccak256(r))) }).collect();
        format!(r#"{{{},"hash":"{}","transactions":[{}]}}"#, self.header_obj, hex(&self.block), txs.join(","))
    }

    fn log(&self) -> String {
        let mut topic1 = [0u8; 32];
        topic1[12..].copy_from_slice(&self.who);
        format!(
            r#"{{"address":"{}","topics":["{}","{}","{}"],"data":"0x","blockNumber":"{}","blockHash":"{}","transactionHash":"{}","transactionIndex":"0x1","logIndex":"0x0","removed":false}}"#,
            hex(&EMITTER),
            hex(&zikaron_anchor::scan::topic0()),
            hex(&topic1),
            hex(&HASH),
            qn(NUMBER),
            hex(&self.block),
            hex(&self.tx)
        )
    }

    fn answer(&self, method: &str, ps: &[Value], limit_receipts: bool) -> Result<String, String> {
        let first = ps.first().and_then(Value::as_str).unwrap_or_default().to_string();
        Ok(match method {
            "eth_chainId" => format!("\"{}\"", qn(CHAIN)),
            "eth_blockNumber" => "\"0x1000\"".into(),
            "eth_getLogs" => format!("[{}]", self.log()),
            "eth_getTransactionReceipt" if limit_receipts => return Err(r#"{"code":-32005,"message":"rate limit exceeded"}"#.into()),
            "eth_getTransactionReceipt" if first == hex(&self.tx) => {
                let r = self.receipt.trim_end_matches('}');
                format!(r#"{r},"blockHash":"{}","blockNumber":"{}","transactionHash":"{}","transactionIndex":"0x1"}}"#, hex(&self.block), qn(NUMBER), hex(&self.tx))
            }
            "eth_getTransactionByHash" if first == hex(&self.tx) => tx_object(&self.raws[1], &self.block, 1),
            "eth_getBlockByHash" if first == hex(&self.block) => self.block_obj(ps.get(1) == Some(&Value::Bool(true))),
            "eth_getBlockByNumber" if first == qn(NUMBER) => self.block_obj(false),
            "eth_getBlockByNumber" if first == qn(NUMBER - 1) => self.parent_obj.clone(),
            "eth_getCode" => "\"0x\"".into(),
            "eth_getProof" => {
                let nodes: Vec<String> = self.state_proof.iter().map(|n| format!("\"{}\"", hex(n))).collect();
                format!(r#"{{"accountProof":[{}]}}"#, nodes.join(","))
            }
            _ => "null".into(),
        })
    }
}

type Asked = Arc<Mutex<BTreeMap<String, usize>>>;

fn node(world: Arc<World>, limit: Arc<AtomicBool>) -> (String, Asked) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let asked: Asked = Default::default();
    let log = asked.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (log, world, limit) = (log.clone(), world.clone(), limit.clone());
            std::thread::spawn(move || {
                let mut s = s;
                let mut raw = Vec::new();
                let mut buf = [0u8; 8192];
                let body = loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => raw.extend_from_slice(&buf[..n]),
                    }
                    let text = String::from_utf8_lossy(&raw).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        let len: usize = text[..i].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|n| n.trim().parse().ok())).unwrap_or(0);
                        if raw.len() >= i + 4 + len {
                            break text[i + 4..].to_string();
                        }
                    }
                };
                let one = |req: &Value| -> String {
                    let id = req.member("id").map(|i| String::from_utf8_lossy(&zikaron::json::canon_bytes(i)).to_string()).unwrap_or_else(|| "1".into());
                    let method = req.member("method").and_then(Value::as_str).unwrap_or_default().to_string();
                    let ps = req.member("params").and_then(Value::as_arr).cloned().unwrap_or_default();
                    *log.lock().unwrap().entry(method.clone()).or_default() += 1;
                    match world.answer(&method, &ps, limit.load(Ordering::SeqCst)) {
                        Ok(r) => format!(r#"{{"jsonrpc":"2.0","id":{id},"result":{r}}}"#),
                        Err(e) => format!(r#"{{"jsonrpc":"2.0","id":{id},"error":{e}}}"#),
                    }
                };
                let text = match zikaron::json::parse(body.as_bytes()).unwrap_or(Value::Null) {
                    Value::Arr(all) => format!("[{}]", all.iter().map(one).collect::<Vec<_>>().join(",")),
                    req => one(&req),
                };
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len());
            });
        }
    });
    (url, asked)
}

#[derive(Clone, Default)]
struct Room(Arc<Mutex<BTreeMap<String, Vec<u8>>>>);

impl zikaron_send::checked::Room for Room {
    type Fail = ();
    fn read(&self, file: &str) -> Option<Vec<u8>> {
        self.0.lock().unwrap().get(file).cloned()
    }
    fn put(&self, file: &str, plain: &[u8]) -> Result<(), ()> {
        self.0.lock().unwrap().insert(file.to_string(), plain.to_vec());
        Ok(())
    }
}

const ANCHOR: [&str; 6] = ["eth_getTransactionReceipt", "eth_getTransactionByHash", "eth_getBlockByHash", "eth_getBlockByNumber", "eth_getCode", "eth_getProof"];

fn took(asked: &Asked) -> BTreeMap<String, usize> {
    std::mem::take(&mut *asked.lock().unwrap()).into_iter().filter(|(m, _)| ANCHOR.contains(&m.as_str())).collect()
}

#[test]
fn an_anchor_one_node_read_proved_but_for_its_receipt_is_asked_its_receipt_alone_of_each_node() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let wd = Arc::new(world());
    let (limit_a, limit_b) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicBool::new(true)));
    let ((a, asked_a), (b, asked_b)) = (node(wd.clone(), limit_a), node(wd.clone(), limit_b.clone()));
    let eps: Vec<Endpoint> = [&a, &b].iter().map(|u| Endpoint::at(CHAIN, NodeAddr::new(u.as_str()))).collect();
    let emitter = hex(&EMITTER);
    let senders = vec![hex(&wd.who)];
    let window = zikaron_glue::basis::Window { chain: CHAIN, registries: std::slice::from_ref(&emitter), from_block: 0, to_block: 512, senders: &senders };
    let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&window));
    let room = Room::default();
    let facts = zikaron_send::checked::Record(room.clone());
    let record = || String::from_utf8_lossy(&room.0.lock().unwrap().get(zikaron_send::checked::FILE).cloned().unwrap_or_default()).to_string();
    let mut passes = Vec::new();
    for pass in ["bLimitsReceipts", "bothAnswer", "again"] {
        if pass == "bothAnswer" {
            limit_b.store(false, Ordering::SeqCst);
            zikaron_anchor::endpoints::move_limit_clock_all(zikaron_anchor::endpoints::REST_FIRST);
        }
        let _action = zikaron_anchor::action::open(None);
        let read = match zikaron_send::agree::scan(&eps, &basis, &facts).expect("the scan") {
            zikaron_send::agree::Scan::Basis(r) => r,
            zikaron_send::agree::Scan::NoLabel { .. } => panic!("a basis"),
        };
        let anchors = read.fragment.member("anchors").and_then(Value::as_arr).map_or(0, |v| v.len());
        let (ta, tb) = (took(&asked_a), took(&asked_b));
        println!("SAID {pass}: anchors {anchors} single {} a {ta:?} b {tb:?} record {}", read.single_source, record());
        passes.push((pass, anchors, read.single_source, ta, tb, record()));
    }
    let receipt_only = |m: &BTreeMap<String, usize>| m.len() == 1 && m.get("eth_getTransactionReceipt") == Some(&1);
    let (_, anchors, single, _, _, rec) = &passes[0];
    assert_eq!((*anchors, *single), (1, true), "the first read is a's alone");
    assert!(rec.contains(r#""proved":true"#) && rec.contains(r#""source":"single""#), "kept single-source, marked proved: {rec}");
    let (_, anchors, single, ta, tb, rec) = &passes[1];
    assert_eq!((*anchors, *single), (1, false), "the next read stands on both");
    assert!(receipt_only(ta) && receipt_only(tb), "each node asked the anchor's receipt alone: a {ta:?} b {tb:?}");
    assert!(rec.contains(r#""source":"agreed""#) && !rec.contains(r#""proved""#), "agreed now: {rec}");
    let (_, anchors, _, ta, tb, _) = &passes[2];
    assert_eq!(*anchors, 1);
    assert!(ta.is_empty() && tb.is_empty(), "nothing of the anchor asked: a {ta:?} b {tb:?}");
}

#[test]
fn a_scan_tests_only_the_logs_its_choice_names_and_its_rounds_agree_on_the_rest() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let wd = Arc::new(world());
    let ((a, asked_a), (b, asked_b)) = (node(wd.clone(), Arc::new(AtomicBool::new(false))), node(wd.clone(), Arc::new(AtomicBool::new(false))));
    let eps: Vec<Endpoint> = [&a, &b].iter().map(|u| Endpoint::at(CHAIN, NodeAddr::new(u.as_str()))).collect();
    let emitter = hex(&EMITTER);
    let senders = vec![hex(&wd.who)];
    let window = zikaron_glue::basis::Window { chain: CHAIN, registries: std::slice::from_ref(&emitter), from_block: 0, to_block: 512, senders: &senders };
    let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&window));
    let read = |pick: Option<zikaron_anchor::scan::Pick>| {
        let _action = zikaron_anchor::action::open(None);
        let got = match pick {
            Some(p) => zikaron_send::agree::scan_picked(&eps, &basis, &zikaron_send::Bare, &p),
            None => zikaron_send::agree::scan(&eps, &basis, &zikaron_send::Bare),
        };
        match got.expect("the scan") {
            zikaron_send::agree::Scan::Basis(r) => (r, took(&asked_a), took(&asked_b)),
            zikaron_send::agree::Scan::NoLabel { .. } => panic!("a basis"),
        }
    };
    let (none, ta, tb) = read(Some(zikaron_anchor::scan::Pick::only(|_| false)));
    assert!(ta.is_empty() && tb.is_empty(), "nothing of the anchor asked: a {ta:?} b {tb:?}");
    assert_eq!(none.fragment.member("anchors").and_then(Value::as_arr).map(Vec::len), Some(0), "no record");
    let left = zikaron_anchor::scan::candidates_in(&none.fragment);
    assert_eq!(left.len(), 1, "the anchor's log a candidate");
    assert_eq!((left[0].hash, left[0].sender, left[0].block_number, left[0].tx), (HASH, wd.who, NUMBER, wd.tx));
    assert!(!none.single_source, "both rounds read the candidate alike");
    let one: Vec<Endpoint> = eps[..1].to_vec();
    let alone = |pick: Option<zikaron_anchor::scan::Pick>| {
        let _action = zikaron_anchor::action::open(None);
        let got = match pick {
            Some(p) => zikaron_send::agree::scan_picked(&one, &basis, &zikaron_send::Bare, &p),
            None => zikaron_send::agree::scan(&one, &basis, &zikaron_send::Bare),
        };
        match got.expect("the scan") {
            zikaron_send::agree::Scan::Basis(r) => (r, took(&asked_a)),
            zikaron_send::agree::Scan::NoLabel { .. } => panic!("a basis"),
        }
    };
    let (plain, pa) = alone(None);
    let (every, ea) = alone(Some(zikaron_anchor::scan::Pick::only(|_| true)));
    let (all, aa) = alone(Some(zikaron_anchor::scan::Pick::All));
    let _ = took(&asked_b);
    assert!(!pa.is_empty(), "the plain scan asks of the anchor");
    assert_eq!(ea, pa, "choosing every log asks what the plain scan asks");
    assert_eq!(aa, pa);
    let bytes = |v: &Value| zikaron::json::canon_bytes(v);
    assert_eq!(bytes(&every.fragment), bytes(&plain.fragment), "the same reading");
    assert_eq!(bytes(&all.fragment), bytes(&plain.fragment));
    assert!(plain.fragment.member("candidates").is_none(), "no candidates member where every log was tested");
}
