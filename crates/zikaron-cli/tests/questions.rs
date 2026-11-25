use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use zikaron::json::Value;
use zikaron_anchor::rlp::Item;
use zikaron_send::node::{Endpoint, NodeAddr};

const CHAIN: u64 = 31337;
const REGISTRY: [u8; 20] = [0x11; 20];
const KEY: [u8; 32] = [0x59, 0xc6, 0x99, 0x5e, 0x99, 0x8f, 0x97, 0xa5, 0xa0, 0x04, 0x49, 0x66, 0xf0, 0x94, 0x53, 0x89, 0xdc, 0x9e, 0x86, 0xda, 0xe8, 0x8c, 0x7a, 0x84, 0x12, 0xf4, 0x60, 0x3b, 0x6b, 0x78, 0x69, 0x0d];
const BASE_FEE: u64 = 1_000_000_000;

fn sender() -> [u8; 20] {
    zikaron::cryptox::address_of_privkey(&KEY).expect("an address")
}

#[derive(Clone)]
struct Mined {
    hash: [u8; 32],
    raw: Vec<u8>,
    block: u64,
    index: u64,
    hashes: Vec<[u8; 32]>,
    first_log: u64,
}

#[derive(Default)]
struct Ledger {
    head: u64,
    mined: Vec<Mined>,
    replaced: Vec<u64>,
    forged_blocks: bool,
}

impl Ledger {
    fn mine(&mut self, block: u64, hashes: &[[u8; 32]]) {
        let nonce = self.mined.len() as u64;
        let fees = zikaron_anchor::send::Fees { max_fee: 2 * BASE_FEE, priority: BASE_FEE, gas_limit: 200_000, from_chain: true };
        let (raw, hash) = zikaron_anchor::send::sign_at(&KEY, CHAIN, zikaron_anchor::send::Form::Registry, Some(REGISTRY), hashes, None, fees, nonce).expect("signed");
        self.land(block, raw, hash, hashes.to_vec());
    }

    fn land(&mut self, block: u64, raw: Vec<u8>, hash: [u8; 32], hashes: Vec<[u8; 32]>) {
        let here: Vec<&Mined> = self.mined.iter().filter(|m| m.block == block).collect();
        let index = here.len() as u64;
        let first_log = here.iter().map(|m| m.hashes.len() as u64).sum();
        self.mined.push(Mined { hash, raw, block, index, hashes, first_log });
        self.head = self.head.max(block);
    }

    fn nonce(&self) -> u64 {
        self.mined.len() as u64
    }
}

fn qty(b: &[u8]) -> String {
    let t: Vec<u8> = b.iter().copied().skip_while(|x| *x == 0).collect();
    if t.is_empty() {
        return "0x0".into();
    }
    let h = zikaron::hexfmt::encode(&t);
    format!("0x{}", h.trim_start_matches("0x").trim_start_matches('0'))
}

fn hx(n: u64) -> String {
    format!("0x{n:x}")
}

fn word(b: &[u8]) -> String {
    let mut w = [0u8; 32];
    w[32 - b.len()..].copy_from_slice(b);
    zikaron::hexfmt::encode(&w)
}

fn block_hash(n: u64) -> String {
    hash_at(n, false)
}

fn hash_at(n: u64, replaced: bool) -> String {
    let mut b = if replaced { b"replaced".to_vec() } else { b"block".to_vec() };
    b.extend_from_slice(&n.to_be_bytes());
    zikaron::hexfmt::encode(&zikaron::cryptox::keccak256(&b))
}

impl Ledger {
    fn hash_of(&self, n: u64) -> String {
        hash_at(n, self.replaced.contains(&n))
    }
}

fn block_time(n: u64) -> u64 {
    1_700_000_000 + 12 * n
}

fn logs_in(m: &Mined, block: &str) -> Vec<String> {
    let t0 = zikaron::hexfmt::encode(&zikaron_anchor::scan::topic0());
    m.hashes
        .iter()
        .enumerate()
        .map(|(i, h)| {
            format!(
                "{{\"address\":\"{}\",\"blockHash\":\"{}\",\"blockNumber\":\"{}\",\"data\":\"0x\",\"logIndex\":\"{}\",\"topics\":[\"{t0}\",\"{}\",\"{}\"],\"transactionHash\":\"{}\",\"transactionIndex\":\"{}\"}}",
                zikaron::hexfmt::encode(&REGISTRY),
                block,
                hx(m.block),
                hx(m.first_log + i as u64),
                word(&sender()),
                zikaron::hexfmt::encode(h),
                zikaron::hexfmt::encode(&m.hash),
                hx(m.index)
            )
        })
        .collect()
}

fn tx_of(m: &Mined) -> String {
    let items = zikaron_anchor::rlp::decode_all(&m.raw[1..]).expect("rlp");
    let f: Vec<Vec<u8>> = items.list().expect("a list").iter().map(|i| i.bytes().map(<[u8]>::to_vec).unwrap_or_default()).collect();
    let _ = Item::Bytes(Vec::new());
    format!(
        "{{\"accessList\":[],\"blockHash\":\"{}\",\"blockNumber\":\"{}\",\"chainId\":\"{}\",\"from\":\"{}\",\"gas\":\"{}\",\"hash\":\"{}\",\"input\":\"{}\",\"maxFeePerGas\":\"{}\",\"maxPriorityFeePerGas\":\"{}\",\"nonce\":\"{}\",\"r\":\"{}\",\"s\":\"{}\",\"to\":\"{}\",\"transactionIndex\":\"{}\",\"type\":\"0x2\",\"v\":\"{}\",\"value\":\"{}\",\"yParity\":\"{}\"}}",
        block_hash(m.block),
        hx(m.block),
        qty(&f[0]),
        zikaron::hexfmt::encode(&sender()),
        qty(&f[4]),
        zikaron::hexfmt::encode(&m.hash),
        zikaron::hexfmt::encode(&f[7]),
        qty(&f[3]),
        qty(&f[2]),
        qty(&f[1]),
        qty(&f[10]),
        qty(&f[11]),
        zikaron::hexfmt::encode(&f[5]),
        hx(m.index),
        qty(&f[9]),
        qty(&f[6]),
        qty(&f[9]),
    )
}

fn hashes_in(data: &[u8]) -> Vec<[u8; 32]> {
    if data.len() == 36 {
        return vec![data[4..36].try_into().expect("a word")];
    }
    let n = data.get(4 + 32 + 24..4 + 64).map(|b| u64::from_be_bytes(b.try_into().expect("eight"))).unwrap_or(0) as usize;
    (0..n).map(|i| data[4 + 64 + 32 * i..4 + 96 + 32 * i].try_into().expect("a word")).collect()
}

type Asked = Arc<Mutex<Vec<String>>>;

fn node(chain: Arc<Mutex<Ledger>>, limit: Option<u64>) -> (String, Asked) {
    node_batching(chain, limit, false)
}

fn node_batching(chain: Arc<Mutex<Ledger>>, limit: Option<u64>, batching: bool) -> (String, Asked) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let asked: Asked = Default::default();
    let log = asked.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (log, chain) = (log.clone(), chain.clone());
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
                let req = zikaron::json::parse(body.as_bytes()).unwrap_or(Value::Null);
                if let Value::Arr(qs) = &req {
                    let methods: Vec<String> = qs.iter().map(|q| q.member("method").and_then(|m| m.as_str()).unwrap_or("").to_string()).collect();
                    let mut kinds: Vec<&String> = Vec::new();
                    for m in &methods {
                        if !kinds.contains(&m) {
                            kinds.push(m);
                        }
                    }
                    let out = if !batching {
                        for m in &kinds {
                            log.lock().unwrap().push(format!("batch refused {m}"));
                        }
                        let e = r#"{"error":{"code":-32600,"message":"batch requests are not supported"},"id":null,"jsonrpc":"2.0"}"#;
                        format!("HTTP/1.1 400 Bad Request\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{e}", e.len())
                    } else {
                        for m in &kinds {
                            log.lock().unwrap().push(format!("batch {m} {}", methods.iter().filter(|x| x == m).count()));
                        }
                        let mut c = chain.lock().unwrap();
                        let rows: Vec<String> = qs
                            .iter()
                            .zip(&methods)
                            .map(|(q, method)| {
                                let id = q.member("id").and_then(Value::as_int).unwrap_or(0);
                                let params: Vec<Value> = match q.member("params") {
                                    Some(Value::Arr(p)) => p.clone(),
                                    _ => Vec::new(),
                                };
                                let p0 = params.first().and_then(|x| x.as_str()).unwrap_or("").to_string();
                                let answered = match method.as_str() {
                                    "eth_getProof" => Err("{\"code\":-32603,\"message\":\"distance to target block exceeds maximum proof window\"}".to_string()),
                                    "eth_getBlockByHash" if c.forged_blocks => Ok(format!("{{\"hash\":\"{p0}\",\"number\":\"0x64\"}}")),
                                    "eth_getBlockByHash" => Ok("null".to_string()),
                                    _ => answer(&mut c, method, &params, limit),
                                };
                                match answered {
                                    Ok(v) => format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{v}}}"),
                                    Err(e) => format!("{{\"error\":{e},\"id\":{id},\"jsonrpc\":\"2.0\"}}"),
                                }
                            })
                            .collect();
                        let b = format!("[{}]", rows.join(","));
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{b}", b.len())
                    };
                    let _ = s.write_all(out.as_bytes());
                    return;
                }
                let method = req.member("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
                let params: Vec<Value> = match req.member("params") {
                    Some(Value::Arr(p)) => p.clone(),
                    _ => Vec::new(),
                };
                let id = body.split("\"id\":").nth(1).and_then(|x| x.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                let num = |x: &str| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok();
                let named = match method.as_str() {
                    "eth_getLogs" => {
                        let f = params.first().cloned().unwrap_or(Value::Null);
                        let at = |k: &str| f.member(k).and_then(|v| v.as_str()).and_then(num).unwrap_or(0);
                        format!("eth_getLogs {}-{}", at("fromBlock"), at("toBlock"))
                    }
                    _ => method.clone(),
                };
                log.lock().unwrap().push(named);
                let mut c = chain.lock().unwrap();
                let result = answer(&mut c, &method, &params, limit);
                drop(c);
                let body = match result {
                    Ok(v) => format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{v}}}"),
                    Err(e) => format!("{{\"error\":{e},\"id\":{id},\"jsonrpc\":\"2.0\"}}"),
                };
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    (url, asked)
}

fn answer(c: &mut Ledger, method: &str, params: &[Value], limit: Option<u64>) -> Result<String, String> {
    let p = |i: usize| params.get(i).and_then(|x| x.as_str()).unwrap_or("").to_string();
    let num = |x: &str| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok();
    let head = c.head;
    match method {
        "eth_chainId" => Ok(format!("\"{}\"", hx(CHAIN))),
        "eth_blockNumber" => Ok(format!("\"{}\"", hx(head))),
        "eth_getBlockByNumber" => {
            let n = if p(0) == "latest" { Some(head) } else { num(&p(0)) };
            Ok(match n.filter(|n| *n <= head) {
                Some(n) => format!(
                    "{{\"baseFeePerGas\":\"{}\",\"hash\":\"{}\",\"number\":\"{}\",\"parentHash\":\"{}\",\"timestamp\":\"{}\"}}",
                    hx(BASE_FEE),
                    c.hash_of(n),
                    hx(n),
                    c.hash_of(n.saturating_sub(1)),
                    hx(block_time(n))
                ),
                None => "null".into(),
            })
        }
        "eth_getLogs" => {
            let f = params.first().cloned().unwrap_or(Value::Null);
            let at = |k: &str| f.member(k).and_then(|v| v.as_str()).and_then(num).unwrap_or(0);
            let (from, to) = (at("fromBlock"), at("toBlock"));
            match limit {
                Some(n) if to - from + 1 > n => Err("{\"code\":-32012,\"message\":\"getLogs request exceeded max allowed range\"}".into()),
                _ => Ok(format!("[{}]", c.mined.iter().filter(|m| (from..=to).contains(&m.block)).flat_map(|m| logs_in(m, &c.hash_of(m.block))).collect::<Vec<_>>().join(","))),
            }
        }
        "eth_getTransactionByHash" => Ok(c.mined.iter().find(|m| zikaron::hexfmt::encode(&m.hash) == p(0)).map(tx_of).unwrap_or_else(|| "null".into())),
        "eth_getTransactionReceipt" => Ok(c
            .mined
            .iter()
            .find(|m| zikaron::hexfmt::encode(&m.hash) == p(0))
            .map(|m| format!("{{\"blockHash\":\"{}\",\"blockNumber\":\"{}\",\"logs\":[{}],\"status\":\"0x1\",\"transactionHash\":\"{}\",\"transactionIndex\":\"{}\"}}", c.hash_of(m.block), hx(m.block), logs_in(m, &c.hash_of(m.block)).join(","), zikaron::hexfmt::encode(&m.hash), hx(m.index)))
            .unwrap_or_else(|| "null".into())),
        "eth_getCode" => Ok("\"0x\"".into()),
        "eth_call" => Ok(format!("\"{}\"", word(&[]))),
        "eth_getBalance" => Ok("\"0x56bc75e2d63100000\"".into()),
        "eth_feeHistory" => Ok(format!("{{\"baseFeePerGas\":[\"{0}\",\"{0}\"],\"oldestBlock\":\"{1}\",\"reward\":[[\"{0}\"]]}}", hx(BASE_FEE), hx(head))),
        "eth_estimateGas" => Ok("\"0x7530\"".into()),
        "eth_getTransactionCount" => Ok(format!("\"{}\"", hx(c.nonce()))),
        "eth_sendRawTransaction" => {
            let bytes = zikaron::hexfmt::decode(&p(0)).unwrap_or_default();
            let h = zikaron::cryptox::keccak256(&bytes);
            let items = zikaron_anchor::rlp::decode_all(&bytes[1..]).expect("rlp");
            let data = items.list().and_then(|l| l.get(7)).and_then(Item::bytes).map(<[u8]>::to_vec).unwrap_or_default();
            let next = head + 1;
            c.land(next, bytes.clone(), h, hashes_in(&data));
            Ok(format!("\"{}\"", zikaron::hexfmt::encode(&h)))
        }
        _ => Err("{\"code\":-32601,\"message\":\"the method does not exist/is not available\"}".into()),
    }
}

fn node_holding(chain: Arc<Mutex<Ledger>>, hold: usize) -> (String, Asked) {
    let (url, asked) = node(chain, None);
    let front = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let at = format!("http://{}", front.local_addr().expect("addr"));
    let to = url.trim_start_matches("http://").to_string();
    let held = Arc::new(Mutex::new(hold));
    let log = asked.clone();
    std::thread::spawn(move || {
        for s in front.incoming().flatten() {
            let (to, held, log) = (to.clone(), held.clone(), log.clone());
            std::thread::spawn(move || {
                let mut s = s;
                let mut raw = Vec::new();
                let mut buf = [0u8; 8192];
                let (head_end, len) = loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => raw.extend_from_slice(&buf[..n]),
                    }
                    let text = String::from_utf8_lossy(&raw).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        let len: usize = text[..i].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|n| n.trim().parse().ok())).unwrap_or(0);
                        if raw.len() >= i + 4 + len {
                            break (i + 4, len);
                        }
                    }
                };
                let body = String::from_utf8_lossy(&raw[head_end..head_end + len]).to_string();
                if body.contains("eth_getTransactionReceipt") {
                    let mut h = held.lock().unwrap();
                    if *h > 0 {
                        *h -= 1;
                        drop(h);
                        log.lock().unwrap().push("eth_getTransactionReceipt".into());
                        let id = body.split("\"id\":").nth(1).and_then(|x| x.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                        let reply = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":null}}");
                        let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
                        return;
                    }
                }
                let Ok(mut back) = std::net::TcpStream::connect(&to) else { return };
                let _ = back.write_all(&raw[..head_end + len]);
                let mut answer = Vec::new();
                let _ = back.read_to_end(&mut answer);
                let _ = s.write_all(&answer);
            });
        }
    });
    (at, asked)
}

fn count(asked: &Asked, method: &str) -> usize {
    asked.lock().unwrap().iter().filter(|m| m.split(' ').next() == Some(method)).count()
}

fn take(asked: &Asked) -> Vec<String> {
    std::mem::take(&mut *asked.lock().unwrap())
}

const FACTS: [&str; 4] = ["eth_getTransactionByHash", "eth_getTransactionReceipt", "eth_getBlockByNumber", "eth_getCode"];

fn facts_asked(asked: &Asked) -> usize {
    FACTS.iter().map(|m| count(asked, m)).sum()
}

fn eps(urls: &[&String]) -> Vec<Endpoint> {
    urls.iter().map(|u| Endpoint::at(CHAIN, NodeAddr::new(u.as_str()))).collect()
}

fn basis(from: u64, to: u64) -> Vec<u8> {
    let senders = vec![zikaron::hexfmt::encode(&sender())];
    let registry = zikaron::hexfmt::encode(&REGISTRY);
    zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&zikaron_glue::basis::Window { chain: CHAIN, registries: std::slice::from_ref(&registry), from_block: from, to_block: to, senders: &senders }))
}

fn fifty() -> Arc<Mutex<Ledger>> {
    let mut l = Ledger::default();
    for t in 0..10u8 {
        let hashes: Vec<[u8; 32]> = (0..5u8).map(|i| [t * 5 + i + 1; 32]).collect();
        l.mine(100 + u64::from(t / 2), &hashes);
    }
    l.head = 1000;
    Arc::new(Mutex::new(l))
}

#[derive(Clone, Default)]
struct Room(Arc<Mutex<std::collections::BTreeMap<String, Vec<u8>>>>);

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

fn anchors_read(s: &zikaron_send::agree::Scan) -> usize {
    match s {
        zikaron_send::agree::Scan::Basis(a) => a.fragment.member("anchors").and_then(|x| x.as_arr()).map_or(0, |x| x.len()),
        zikaron_send::agree::Scan::NoLabel { .. } => panic!("a basis"),
    }
}

#[derive(Debug)]
struct Fail(String);

impl From<zikaron_send::Trouble> for Fail {
    fn from(t: zikaron_send::Trouble) -> Fail {
        Fail(format!("{} {:?}", t.code.as_str(), t.why))
    }
}

impl From<zikaron_seal::Trouble> for Fail {
    fn from(t: zikaron_seal::Trouble) -> Fail {
        Fail(format!("{:?} {}", t.code, t.tail))
    }
}

struct Home {
    queue: Mutex<zikaron_seal::queue::Queue>,
    machine: std::path::PathBuf,
    room: Room,
}

impl Home {
    fn with(ids: &[String]) -> Home {
        let mut q = zikaron_seal::queue::Queue { chain: CHAIN, ..Default::default() };
        for (i, id) in ids.iter().enumerate() {
            let _ = q.push(id, i as u64);
        }
        let machine = std::env::temp_dir().join(format!("zk-questions-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&machine);
        Home { queue: Mutex::new(q), machine, room: Room::default() }
    }
}

impl zikaron_send::End for Home {
    type Fail = Fail;
    fn code_of(f: &Fail) -> Option<String> {
        Some(f.0.clone())
    }
    fn read_queue(&self) -> Result<zikaron_seal::queue::Queue, Fail> {
        Ok(self.queue.lock().unwrap().clone())
    }
    fn write_queue(&self, q: &zikaron_seal::queue::Queue) -> Result<(), Fail> {
        *self.queue.lock().unwrap() = q.clone();
        Ok(())
    }
    fn pile(&self) -> Result<Vec<Vec<u8>>, Fail> {
        Ok(Vec::new())
    }
    fn behind(&self, lacks: &[String]) -> Fail {
        Fail(format!("behind {}", lacks.len()))
    }
    fn unread(&self, f: Fail) -> Fail {
        f
    }
    fn mark(&self) -> Result<Option<zikaron_seal::unfetched::State>, Fail> {
        Ok(None)
    }
    fn lift(&self) -> Result<(), Fail> {
        Ok(())
    }
    fn machine(&self) -> Result<std::path::PathBuf, Fail> {
        Ok(self.machine.clone())
    }
}

impl zikaron_send::Facts for Home {
    fn known(&self) -> zikaron_anchor::scan::Known {
        zikaron_send::Facts::known(&zikaron_send::checked::Record(self.room.clone()))
    }
    fn known_single(&self) -> zikaron_anchor::scan::Known {
        zikaron_send::Facts::known_single(&zikaron_send::checked::Record(self.room.clone()))
    }
    fn add(&self, runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], thin: &[u64]) {
        zikaron_send::Facts::add(&zikaron_send::checked::Record(self.room.clone()), runs, thin)
    }
}

struct Signs;

impl zikaron_send::batch::Signer for Signs {
    fn from(&self) -> Option<[u8; 20]> {
        Some(sender())
    }
    fn sign(&self, chain: u64, registry: Option<[u8; 20]>, hashes: &[[u8; 32]], input: Option<Vec<u8>>, fees: zikaron_anchor::send::Fees, nonce: u64) -> Result<(Vec<u8>, [u8; 32]), zikaron_anchor::rpc::Trouble> {
        zikaron_anchor::send::sign_at(&KEY, chain, zikaron_anchor::send::Form::Registry, registry, hashes, input, fees, nonce)
    }
}

fn send_queued(home: &Home, urls: &[&String]) -> zikaron_send::batch::Sent {
    queued(home, urls).expect("sent")
}

fn gate_ask(home: &Home, urls: &[&String]) -> zikaron_send::gate::Ask {
    zikaron_send::gate::Ask { root: home.machine.clone(), lanes: vec![zikaron_send::gate::Lane { chain: CHAIN, registry: REGISTRY, from_block: 0, eps: eps(urls) }], own: Some(zikaron::hexfmt::encode(&sender())) }
}

fn queued(home: &Home, urls: &[&String]) -> Result<zikaron_send::batch::Sent, Fail> {
    let ids: Vec<String> = home.queue.lock().unwrap().items.iter().map(|x| x.id.clone()).collect();
    let hashes = zikaron_seal::queue::Queue::hashes(&ids).expect("hashes");
    let data = zikaron_anchor::send::registry_calldata(&hashes);
    let call = zikaron_anchor::send::estimate_call(&sender(), &REGISTRY, &data);
    let est = zikaron_send::batch::estimate(&zikaron_send::batch::GasAsk { chain: CHAIN, call, data, eps: eps(urls) }).expect("an estimate");
    let ask = gate_ask(home, urls);
    let b = zikaron_send::batch::Batch { ids, hashes, chain: CHAIN, registry: REGISTRY, urls: urls.iter().map(|u| NodeAddr::new(u.as_str())).collect(), backoff: Vec::new(), basis: Vec::new(), carried: None };
    zikaron_send::batch::anchor(home, b, &Signs, Some(est.gas), est.fees, ask)
}

#[test]
fn a_queued_anchor_asks_each_node_the_head_once_and_the_lineage_window_once() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    zikaron_send::node::forget_serving();
    let chain = Arc::new(Mutex::new(Ledger { head: 1000, ..Ledger::default() }));
    let ((a, asked_a), (b, asked_b)) = (node(chain.clone(), None), node(chain.clone(), None));
    let home = Home::with(&[zikaron::hexfmt::encode(&[0xab; 32])]);
    let _action = zikaron_anchor::action::open(None);
    let sent = send_queued(&home, &[&a, &b]);
    assert!(!sent.earlier && chain.lock().unwrap().mined.len() == 1, "the batch went out");
    for (n, asked) in [("a", &asked_a), ("b", &asked_b)] {
        println!("SAID queued {n}: {:?}", asked.lock().unwrap());
        assert_eq!(count(asked, "eth_blockNumber"), 1, "{n}: the head, once in the action");
        assert_eq!(count(asked, "eth_chainId"), 1, "{n}: the chain id, once");
        assert_eq!(count(asked, "eth_getLogs"), 1, "{n}: the lineage's window, once for the duplicate check and the gate");
        assert_eq!(count(asked, "eth_getBlockByNumber"), 2, "{n}: the pinned block for the fees, and the latest for the confirmation card");
    }
    assert_eq!(count(&asked_a, "eth_sendRawTransaction") + count(&asked_b, "eth_sendRawTransaction"), 1);
}

#[test]
fn a_scan_asks_each_transaction_and_block_once_however_many_anchors_they_carry() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let chain = fifty();
    let ((a, asked_a), (b, asked_b)) = (node(chain.clone(), None), node(chain.clone(), None));
    let room = Room::default();
    let facts = zikaron_send::checked::Record(room.clone());
    {
        let _action = zikaron_anchor::action::open(None);
        let s = zikaron_send::agree::scan(&eps(&[&a, &b]), &basis(0, 1000), &facts).expect("the scan");
        assert_eq!(anchors_read(&s), 50);
    }
    for (n, asked) in [("a", &asked_a), ("b", &asked_b)] {
        let per = |m: &str| count(asked, m);
        println!("SAID scan cold {n}: chainId {} logs {} tx {} receipt {} block {} code {}", per("eth_chainId"), per("eth_getLogs"), per("eth_getTransactionByHash"), per("eth_getTransactionReceipt"), per("eth_getBlockByNumber"), per("eth_getCode"));
        assert_eq!((per("eth_chainId"), per("eth_getLogs")), (1, 1), "{n}");
        assert_eq!(per("eth_getTransactionReceipt"), 10, "{n}: each receipt once, not once per anchor");
        assert_eq!(per("eth_getBlockByNumber"), 5, "{n}: each block once");
        assert_eq!(per("eth_getCode"), 6, "{n}: the code at each block once (blocks 99 to 104)");
    }
    assert_eq!(count(&asked_a, "eth_getTransactionByHash") + count(&asked_b, "eth_getTransactionByHash"), 10, "each transaction of one node only: it proves itself");
    take(&asked_a);
    take(&asked_b);
    let _action = zikaron_anchor::action::open(None);
    let s = zikaron_send::agree::scan(&eps(&[&a, &b]), &basis(0, 1000), &facts).expect("the scan");
    assert_eq!(anchors_read(&s), 50);
    for (n, asked) in [("a", &asked_a), ("b", &asked_b)] {
        println!("SAID scan warm {n}: facts {}", facts_asked(asked));
        assert_eq!(facts_asked(asked), 0, "{n}: the facts kept, no anchor asked about");
    }
}

#[test]
fn a_node_refusing_batches_is_asked_one_by_one_and_the_reading_stands_on_both() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let chain = fifty();
    let ((a, asked_a), (b, asked_b)) = (node_batching(chain.clone(), None, true), node(chain.clone(), None));
    let room = Room::default();
    let facts = zikaron_send::checked::Record(room.clone());
    let _action = zikaron_anchor::action::open(None);
    let s = zikaron_send::agree::scan(&eps(&[&a, &b]), &basis(0, 1000), &facts).expect("the scan");
    assert_eq!(anchors_read(&s), 50);
    let zikaron_send::agree::Scan::Basis(read) = &s else { panic!("a basis") };
    assert!(!read.single_source, "the reading stands on both nodes");
    let all = |asked: &Asked| asked.lock().unwrap().clone();
    println!("SAID batches a: {:?}", all(&asked_a).iter().filter(|m| m.starts_with("batch")).collect::<Vec<_>>());
    println!("SAID batches b: {:?}", all(&asked_b).iter().filter(|m| m.starts_with("batch")).collect::<Vec<_>>());
    assert!(all(&asked_a).contains(&"batch eth_getTransactionReceipt 10".to_string()), "a's receipts in one batch");
    assert_eq!(count(&asked_a, "eth_getTransactionReceipt"), 0, "and none one by one");
    assert_eq!(count(&asked_b, "eth_getTransactionReceipt"), 10, "b's receipts one by one");
    assert_eq!(all(&asked_b).iter().filter(|m| *m == "batch refused eth_getTransactionReceipt").count(), 1, "b refused the batch once and was not asked again");
    let batched = |asked: &Asked, method: &str| all(asked).iter().filter_map(|m| m.strip_prefix("batch ")?.strip_prefix(method)?.strip_prefix(' ')?.parse::<usize>().ok()).sum::<usize>();
    assert_eq!(batched(&asked_a, "eth_getProof"), 4, "a: its share's proofs asked in batch (and refused), b's share not dealt to it");
    assert_eq!(facts_asked(&asked_a), 0, "a: no fact asked one by one");
    assert_eq!(batched(&asked_a, "eth_getCode"), 6, "a: the code asked in batch, once each");
    assert_eq!(batched(&asked_a, "eth_getBlockByNumber"), 5, "a: each block's time once with the transactions; no block before, its proofs refused (#606)");
    assert_eq!(count(&asked_b, "eth_getBlockByNumber"), 5, "b: blocks asked as before");
    assert_eq!(count(&asked_b, "eth_getCode"), 6, "b: the code asked as before");
}

#[test]
fn a_node_that_does_not_prove_is_named_apart_and_the_reading_is_whole() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let chain = fifty();
    chain.lock().unwrap().forged_blocks = true;
    let ((a, _), (b, _)) = (node_batching(chain.clone(), None, true), node(chain.clone(), None));
    let facts = zikaron_send::checked::Record(Room::default());
    let _action = zikaron_anchor::action::open(None);
    let s = zikaron_send::agree::scan(&eps(&[&a, &b]), &basis(0, 1000), &facts).expect("the scan");
    assert_eq!(anchors_read(&s), 50);
    let zikaron_send::agree::Scan::Basis(read) = &s else { panic!("a basis") };
    assert!(read.unanswered.is_empty(), "the reading is whole: {:?}", read.unanswered);
    assert!(!read.single_source, "on both nodes");
    assert_eq!(read.unproven.len(), 1, "the forging node named once: {:?}", read.unproven);
    let again = zikaron_send::agree::scan(&eps(&[&a, &b]), &basis(0, 1000), &facts).expect("the scan again in the action");
    let zikaron_send::agree::Scan::Basis(again) = &again else { panic!("a basis") };
    assert_eq!(again.unproven.len(), 1, "once in the action: {:?}", again.unproven);
}

#[test]
fn a_verify_of_fifty_anchors_with_facts_kept_asks_nothing_per_anchor() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let chain = fifty();
    let ((a, asked_a), (b, asked_b)) = (node(chain.clone(), None), node(chain.clone(), None));
    let facts = zikaron_send::checked::Record(Room::default());
    {
        let _action = zikaron_anchor::action::open(None);
        let s = zikaron_send::agree::scan(&eps(&[&a]), &basis(0, 1000), &facts).expect("the verify's reading");
        assert_eq!(anchors_read(&s), 50);
    }
    println!("SAID verify cold: facts {}", facts_asked(&asked_a));
    assert_eq!(facts_asked(&asked_a), 31, "each transaction, receipt and block once, the code at each block once");
    take(&asked_a);
    {
        let _action = zikaron_anchor::action::open(None);
        let s = zikaron_send::agree::scan(&eps(&[&a]), &basis(0, 1000), &facts).expect("the verify's reading again");
        assert_eq!(anchors_read(&s), 50);
    }
    println!("SAID verify warm: {:?}", asked_a.lock().unwrap());
    assert_eq!(facts_asked(&asked_a), 0, "one node's facts kept: the same reading asks nothing about any anchor");
    take(&asked_a);
    let _action = zikaron_anchor::action::open(None);
    let s = zikaron_send::agree::scan(&eps(&[&a, &b]), &basis(0, 1000), &facts).expect("an agreed reading");
    assert_eq!(anchors_read(&s), 50);
    assert!(facts_asked(&asked_b) >= 21, "an agreed reading takes no single-source fact: asked again ({})", facts_asked(&asked_b));
}

#[test]
fn a_send_judges_at_the_actions_lowest_head_and_a_countdown_shows_the_newest_highest() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let low = Arc::new(Mutex::new(Ledger { head: 1000, mined: Vec::new(), ..Ledger::default() }));
    let high = Arc::new(Mutex::new(Ledger { head: 1002, mined: Vec::new(), ..Ledger::default() }));
    let ((a, asked_a), (b, asked_b)) = (node(low.clone(), None), node(high.clone(), None));
    let e = eps(&[&a, &b]);
    let _action = zikaron_anchor::action::open(None);
    assert_eq!(zikaron_send::node::head_block(&e, CHAIN).expect("a head").0, 1000, "the action's head: the lowest");
    let (t, n, _) = zikaron_send::node::head_time(&e, CHAIN).expect("a time");
    assert_eq!((t, n), (block_time(1002), 1002), "the time shown: the highest latest block");
    low.lock().unwrap().head = 1005;
    high.lock().unwrap().head = 1007;
    assert_eq!(zikaron_send::node::head_block(&e, CHAIN).expect("a head").0, 1000, "the action's head holds for its judgements");
    let (t, n, _) = zikaron_send::node::head_time(&e, CHAIN).expect("a time");
    assert_eq!((t, n), (block_time(1007), 1007), "the time shown moves with the chain, never held");
    for (name, asked) in [("a", &asked_a), ("b", &asked_b)] {
        assert_eq!(count(asked, "eth_blockNumber"), 1, "{name}: the action's head, asked once");
        assert_eq!(count(asked, "eth_getBlockByNumber"), 2, "{name}: the latest block, asked each time it is shown");
    }
}

#[test]
fn an_anchor_the_chain_holds_is_refused_and_leaves_by_the_window_the_send_already_asked() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    zikaron_send::node::forget_serving();
    let hash = [0xcd; 32];
    let id = zikaron::hexfmt::encode(&hash);
    let bare = id.trim_start_matches("0x").to_string();
    let holding = || {
        let mut l = Ledger::default();
        l.mine(100, &[hash]);
        l.head = 1000;
        Arc::new(Mutex::new(l))
    };
    let held = holding();
    let ((a, asked_a), (b, asked_b)) = (node(held.clone(), None), node(held.clone(), None));
    let home = Home::with(std::slice::from_ref(&id));
    {
        let _action = zikaron_anchor::action::open(None);
        let refused = queued(&home, &[&a, &b]).err().map(|f| f.0).unwrap_or_default();
        assert!(refused.starts_with("ON_CHAIN"), "refused as on chain: {refused}");
        for (n, asked) in [("a", &asked_a), ("b", &asked_b)] {
            assert_eq!(count(asked, "eth_getLogs"), 1, "{n}: the duplicate check asked the lineage's window");
            take(asked);
        }
        let r = zikaron_send::gate::read(&gate_ask(&home, &[&a, &b]), &home).expect("an agreed reading");
        assert!(r.on_chain.iter().any(|(c, h)| *c == CHAIN && h.trim_start_matches("0x") == bare), "the gate's reading holds the entry: it leaves the queue");
        for (n, asked) in [("a", &asked_a), ("b", &asked_b)] {
            println!("SAID on chain, gate after {n}: {:?}", asked.lock().unwrap());
            assert_eq!((count(asked, "eth_getLogs"), count(asked, "eth_blockNumber"), count(asked, "eth_chainId")), (0, 0, 0), "{n}: the window, the head and the chain id not asked again");
        }
    }
    assert_eq!(count(&asked_a, "eth_sendRawTransaction") + count(&asked_b, "eth_sendRawTransaction"), 0, "nothing sent");
    zikaron_send::node::forget_serving();
    let behind = Arc::new(Mutex::new(Ledger { head: 1000, mined: Vec::new(), ..Ledger::default() }));
    let ((c, asked_c), (d, asked_d)) = (node(holding(), None), node(behind, None));
    let _action = zikaron_anchor::action::open(None);
    let refused = queued(&home, &[&c, &d]).err().map(|f| f.0).unwrap_or_default();
    assert!(refused.starts_with("ON_CHAIN"), "one node holding it: still refused as on chain, nothing signed: {refused}");
    let read = zikaron_send::gate::read(&gate_ask(&home, &[&c, &d]), &home).err().map(|f| f.0).unwrap_or_default();
    assert!(read.starts_with("DISAGREE"), "the nodes disagree: the gate's reading is no reading, so the entry stays: {read}");
    assert_eq!(count(&asked_c, "eth_sendRawTransaction") + count(&asked_d, "eth_sendRawTransaction"), 0, "nothing sent");
}

fn keeper(room: &Room) -> std::sync::Arc<dyn zikaron_anchor::settled::Keep> {
    std::sync::Arc::new(zikaron_send::checked::Logs(room.clone()))
}

#[test]
fn a_scan_served_from_what_was_settled_checks_the_last_anchors_block_once() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let chain = fifty();
    let ((a, asked_a), (b, asked_b)) = (node(chain.clone(), None), node(chain.clone(), None));
    let room = Room::default();
    let facts = zikaron_send::checked::Record(room.clone());
    let scan = || {
        let _action = zikaron_anchor::action::open(Some(keeper(&room)));
        let s = zikaron_send::agree::scan(&eps(&[&a, &b]), &basis(0, 1000), &facts).expect("the scan");
        assert_eq!(anchors_read(&s), 50);
    };
    scan();
    take(&asked_a);
    take(&asked_b);
    scan();
    let tail = format!("eth_getLogs {}-1000", 1000 - zikaron_anchor::settled::MARGIN + 1);
    for (n, asked) in [("a", &asked_a), ("b", &asked_b)] {
        let got = take(asked);
        println!("SAID settled warm {n}: {got:?}");
        assert_eq!(got, vec!["eth_chainId".to_string(), "eth_getBlockByNumber".to_string(), tail.clone()], "{n}: the chain id, the last anchor's block checked once, the tail");
    }
    chain.lock().unwrap().replaced.push(104);
    scan();
    for (n, asked) in [("a", &asked_a), ("b", &asked_b)] {
        let got = take(asked);
        println!("SAID settled replaced {n}: {got:?}");
        assert_eq!(got.iter().filter(|m| m.starts_with("eth_getLogs")).cloned().collect::<Vec<_>>(), vec!["eth_getLogs 104-1000".to_string()], "{n}: read again from the block replaced");
        assert_eq!(got.iter().filter(|m| *m == "eth_getTransactionReceipt").count(), 2, "{n}: the replaced block's two transactions asked about afresh");
    }
}

fn direct(home: &std::path::Path, url: &str, hash: [u8; 32]) -> (i32, String) {
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_zikaron"))
        .args(["anchor", "--endpoint", &format!("{CHAIN}={url}"), "--key", &zikaron::hexfmt::encode(&KEY), "--form", "registry", "--registry", &zikaron::hexfmt::encode(&REGISTRY), "--hash", &zikaron::hexfmt::encode(&hash), "--wait-secs", "0"])
        .env(zikaron_os::HOME_VAR, home)
        .output()
        .expect("zikaron runs");
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).to_string())
}

#[test]
fn a_direct_anchor_asks_the_head_once_and_the_next_run_only_the_logs_after_what_it_settled() {
    let chain = Arc::new(Mutex::new(Ledger { head: 1000, ..Ledger::default() }));
    let (url, asked) = node(chain.clone(), None);
    let home = std::env::temp_dir().join(format!("zk-questions-direct-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).expect("a home");
    let (code, out) = direct(&home, &url, [0x61; 32]);
    assert_eq!(code, 0, "{out}");
    let first = take(&asked);
    println!("SAID direct first: {first:?}");
    assert_eq!(first.iter().filter(|m| *m == "eth_blockNumber").count(), 1, "the head once in the run");
    assert_eq!(first.iter().filter(|m| m.starts_with("eth_getLogs")).cloned().collect::<Vec<_>>(), vec!["eth_getLogs 0-1000".to_string()]);
    chain.lock().unwrap().head = 1006;
    let (code, out) = direct(&home, &url, [0x62; 32]);
    assert_eq!(code, 0, "{out}");
    let second = take(&asked);
    println!("SAID direct second: {second:?}");
    assert_eq!(second.iter().filter(|m| *m == "eth_blockNumber").count(), 1, "the head once in the run");
    assert_eq!(second.iter().filter(|m| m.starts_with("eth_getLogs")).cloned().collect::<Vec<_>>(), vec![format!("eth_getLogs {}-1006", 1000 - zikaron_anchor::settled::MARGIN + 1)], "only after what the first run settled");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn a_queued_anchors_receipt_is_polled_at_one_node_and_its_facts_kept_for_the_next_gate() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    zikaron_send::node::forget_serving();
    let chain = Arc::new(Mutex::new(Ledger { head: 1000, ..Ledger::default() }));
    let (a, asked_a) = node_holding(chain.clone(), 3);
    let (b, asked_b) = node(chain.clone(), None);
    let home = Home::with(&[zikaron::hexfmt::encode(&[0xcd; 32])]);
    {
        let _action = zikaron_anchor::action::open(None);
        let sent = send_queued(&home, &[&a, &b]);
        let urls: Vec<NodeAddr> = [&sent.url].into_iter().cloned().chain([&a, &b].iter().map(|u| NodeAddr::new(u.as_str())).filter(|u| *u != sent.url)).collect();
        assert_eq!(urls[0], NodeAddr::new(a.as_str()), "the first node took the broadcast");
        take(&asked_a);
        take(&asked_b);
        let sender = zikaron_send::batch::Sender { from: sender(), registry: REGISTRY };
        let mut heard = zikaron_send::batch::wait(urls, CHAIN, vec![sent.tx.clone()], sent.ids.clone(), sent.gas, std::time::Duration::from_secs(20), &[], Some(sender), None, None).expect("heard");
        assert!(heard.confirmed, "{:?}", heard.receipt);
        zikaron_send::batch::landed_facts(&home, &mut heard);
        let landed = zikaron_send::batch::settle(&home, heard).expect("settled");
        assert_eq!(landed.calldata, zikaron::hexfmt::encode(&zikaron_anchor::send::registry_calldata(&[[0xcd; 32]])), "the bytes signed");
    }
    let receipts = |x: &Asked| count(x, "eth_getTransactionReceipt");
    println!("SAID receipt a {} b {}; settle a {:?} b {:?}", receipts(&asked_a), receipts(&asked_b), asked_a.lock().unwrap(), asked_b.lock().unwrap());
    assert_eq!((receipts(&asked_a), receipts(&asked_b)), (4, 1), "polled at the node that took it, the other asked once");
    take(&asked_a);
    take(&asked_b);
    let _action = zikaron_anchor::action::open(None);
    let ask = zikaron_send::gate::Ask { root: home.machine.clone(), lanes: vec![zikaron_send::gate::Lane { chain: CHAIN, registry: REGISTRY, from_block: 0, eps: eps(&[&a, &b]) }], own: Some(zikaron::hexfmt::encode(&sender())) };
    let r = zikaron_send::gate::read(&ask, &home).expect("the gate's reading");
    assert_eq!(r.anchored.len(), 1, "{r:?}");
    for (n, asked) in [("a", &asked_a), ("b", &asked_b)] {
        println!("SAID next gate {n}: {:?}", asked.lock().unwrap());
        assert_eq!(count(asked, "eth_getTransactionByHash") + count(asked, "eth_getTransactionReceipt") + count(asked, "eth_getCode"), 0, "{n}: the batch's facts kept at settling: nothing asked about its anchor");
    }
}
