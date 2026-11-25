use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use zikaron::json::Value;
use zikaron_send::batch::{self, OnChain};
use zikaron_send::node::{Endpoint, NodeAddr};
use zikaron_send::{Code, Why};
use zikaron_seal::queue::{Block, Queue};

const CHAIN: u64 = 31337;
const OTHER: u64 = 1;

type Answer = Arc<dyn Fn(&str, &Value) -> String + Send + Sync>;

fn node(answer: Answer) -> (String, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let kept = asked.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (answer, kept) = (answer.clone(), kept.clone());
            std::thread::spawn(move || {
                let mut s = s;
                let mut raw = Vec::new();
                let mut buf = [0u8; 4096];
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
                let method = req.member("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
                let params = req.member("params").cloned().unwrap_or(Value::Null);
                kept.lock().expect("kept").push(method.clone());
                let id = body.split("\"id\":").nth(1).and_then(|x| x.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                let result = answer(&method, &params);
                let body = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{result}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    (url, asked)
}

fn chain_node(chain: u64, tip: u64) -> (String, Arc<Mutex<Vec<String>>>) {
    node(Arc::new(move |method, params| match method {
        "eth_chainId" => format!("\"0x{chain:x}\""),
        "eth_blockNumber" => "\"0x40\"".into(),
        "eth_getBlockByNumber" => match params.as_arr().and_then(|a| a.first()).and_then(|v| v.as_str()) {
            Some("0x30") => "{\"baseFeePerGas\":\"0x3b9aca00\",\"number\":\"0x30\",\"timestamp\":\"0x50\"}".into(),
            _ => "{\"baseFeePerGas\":\"0x3b9aca00\",\"number\":\"0x40\",\"timestamp\":\"0x64\"}".into(),
        },
        "eth_feeHistory" => format!("{{\"oldestBlock\":\"0x2d\",\"gasUsedRatio\":[0.5],\"reward\":[[\"0x{tip:x}\"],[\"0x{tip:x}\"],[\"0x{tip:x}\"]]}}"),
        "eth_estimateGas" => "\"0xc350\"".into(),
        "eth_getLogs" => "[]".into(),
        _ => "null".into(),
    }))
}

fn dead() -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    drop(l);
    url
}

fn eps(urls: &[&str]) -> Vec<Endpoint> {
    urls.iter().map(|u| Endpoint::at(CHAIN, NodeAddr::new(*u))).collect()
}

fn ask(e: Vec<Endpoint>) -> batch::GasAsk {
    let data = zikaron_anchor::send::registry_calldata(&[[0x5a; 32]]);
    let call = zikaron_anchor::send::estimate_call(&[0x15; 20], &[0x11; 20], &data);
    batch::GasAsk { chain: CHAIN, call, data, eps: e }
}

fn names(w: &Why, node: &str) -> bool {
    format!("{w:?}").contains(&zikaron_net::sayable(node))
}

fn quiet() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
}

#[test]
fn a_table_with_no_node_of_the_chain_is_refused_at_the_estimate_by_name() {
    quiet();
    let (other, other_asked) = chain_node(OTHER, 1);
    let (live, _) = chain_node(CHAIN, 1);
    let gone = dead();
    let none = batch::estimate(&ask(vec![Endpoint::at(OTHER, NodeAddr::new(&live))])).expect_err("no node of the chain");
    assert_eq!(none.code, Code::NoEndpoint, "{none:?}");
    assert!(format!("{:?}", none.why).contains(&CHAIN.to_string()), "names the chain: {none:?}");
    let alone = batch::estimate(&ask(eps(&[&gone]))).expect_err("a dead node alone");
    let beside = batch::estimate(&ask(eps(&[&gone, &other]))).expect_err("a dead node beside another chain's");
    assert_eq!(alone.code, beside.code, "two forms, one answer: {alone:?} / {beside:?}");
    assert!(names(&beside.why, &other), "the other chain's node is named: {beside:?}");
    assert!(names(&beside.why, &gone), "the dead node is named: {beside:?}");
    assert!(!other_asked.lock().expect("asked").iter().any(|m| m == "eth_estimateGas"), "the other chain's node is not asked the estimate");
    let (other2, _) = chain_node(OTHER, 1);
    let wrong = batch::estimate(&ask(eps(&[&other2]))).expect_err("another chain's alone");
    assert_eq!(wrong.code, Code::WrongChain, "{wrong:?}");
    assert!(names(&wrong.why, &other2), "{wrong:?}");
    let (other3, _) = chain_node(OTHER, 1);
    let e = batch::estimate(&ask(eps(&[&other3, &live]))).expect("the live node estimates");
    assert_eq!(e.gas, 50_000);
    assert!(e.fees_left.iter().any(|w| matches!(w, Why::WrongChainNode { serves: OTHER, chain: CHAIN, .. }) && names(w, &other3)), "{:?}", e.fees_left);
}

#[test]
fn a_book_call_is_estimated_through_the_same_choice() {
    quiet();
    let (other, _) = chain_node(OTHER, 1);
    let gone = dead();
    let call = zikaron_send::one::Call::transfer(CHAIN, [0x22; 20], 1);
    let alone = zikaron_send::one::estimate(&eps(&[&gone]), &[0x15; 20], &call).expect_err("a dead node alone");
    let beside = zikaron_send::one::estimate(&eps(&[&gone, &other]), &[0x15; 20], &call).expect_err("a dead node beside another chain's");
    assert_eq!(alone.code, beside.code, "{alone:?} / {beside:?}");
    assert!(names(&beside.why, &other), "{beside:?}");
    let (other2, _) = chain_node(OTHER, 1);
    assert_eq!(zikaron_send::one::estimate(&eps(&[&other2]), &[0x15; 20], &call).expect_err("another chain's").code, Code::WrongChain);
    assert_eq!(zikaron_send::one::estimate(&[], &[0x15; 20], &call).expect_err("none").code, Code::NoEndpoint);
}

#[test]
fn what_is_on_chain_is_the_reading_and_the_queues_own_receipts() {
    let id = |b: u8| format!("0x{}", format!("{b:02x}").repeat(32));
    let mut q = Queue { chain: CHAIN, ..Default::default() };
    q.blocks.push(Block { id: id(1), tx: id(9), chain: CHAIN, block: 0x30 });
    q.blocks.push(Block { id: id(2), tx: id(9), chain: 0, block: 0x31 });
    q.blocks.push(Block { id: id(3), tx: id(9), chain: OTHER, block: 0x99 });
    q.anchored.push((id(4), CHAIN));
    let on = OnChain::of(vec![(Some(CHAIN), id(5).to_ascii_uppercase()), (None, id(6))], &q);
    assert!(on.on(CHAIN, &id(1)) && on.on(CHAIN, &id(2).to_ascii_uppercase()) && on.on(CHAIN, &id(4)) && on.on(CHAIN, &id(5)) && on.on(CHAIN, &id(6)));
    assert!(!on.on(CHAIN, &id(3)) && on.on(OTHER, &id(3)) && on.anywhere(&id(3)), "a receipt on another chain is on that chain only");
    assert!(!on.on(OTHER, &id(5)) && on.on(OTHER, &id(6)), "a reading's chain is kept");
    assert!(!on.anywhere(&id(7)));
    assert_eq!(on.own_latest(CHAIN, &[id(1), id(2)]), Some(0x31));
    assert_eq!(on.own_latest(CHAIN, &[id(4)]), None, "recorded anchored without a block: no block to follow");
    assert_eq!(on.own_latest(CHAIN, &[id(7)]), None);
    assert!(!OnChain::of(Vec::new(), &Queue::default()).anywhere(&id(1)));
}

fn genesis() -> Vec<u8> {
    let key = [0x11u8; 32];
    let addr = zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(&key).expect("an address"));
    let sealed = zikaron_glue::book::seal::<()>(zikaron_glue::book::Law::Two, &addr, "genesis", 0, None, Value::Obj(vec![("statement_md".into(), Value::Str("开端".into()))]), |pre, domain| {
        let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
        let (r, s, v) = zikaron::cryptox::sign_digest(&key, &digest).ok_or(())?;
        let mut raw = r.to_vec();
        raw.extend_from_slice(&s);
        raw.push(v);
        Ok(zikaron::hexfmt::encode(&raw))
    });
    match sealed {
        zikaron_glue::book::Sealed::Done(e) => e.bytes,
        _ => panic!("sealed"),
    }
}

#[test]
fn the_fence_follows_the_queues_own_receipt_when_no_node_has_indexed_it() {
    quiet();
    let (live, _) = chain_node(CHAIN, 1);
    let pile = vec![genesis()];
    let first = zikaron::hexfmt::encode(&zikaron_glue::book::entry_id(&pile[0]));
    let b = batch::Batch { ids: vec![format!("0x{}", "ee".repeat(32))], hashes: vec![[0xee; 32]], chain: CHAIN, registry: [0x11; 20], urls: vec![NodeAddr::new(&live)], backoff: Vec::new(), basis: Vec::new(), carried: None };
    let nothing = batch::before_of(&b, &pile, &Queue { chain: CHAIN, ..Default::default() }, None, None).expect("read");
    assert!(nothing.is_empty(), "no receipt and no log: nothing to follow: {nothing:?}");
    let mut q = Queue { chain: CHAIN, ..Default::default() };
    q.blocks.push(Block { id: first.clone(), tx: first.clone(), chain: CHAIN, block: 0x30 });
    let before = batch::before_of(&b, &pile, &q, None, None).expect("read");
    assert_eq!(before, vec![(CHAIN, 0x50)], "the receipt's block time");
    assert_eq!(batch::fence_open(0x50, &before), Err(0x50), "the same time does not open");
    assert_eq!(batch::fence_open(0x51, &before), Ok(()));
}

#[test]
fn two_nodes_whose_fee_histories_differ_give_the_lower_number() {
    quiet();
    let (low, _) = chain_node(CHAIN, 30_000_000);
    let (high, _) = chain_node(CHAIN, 700_000_000);
    let r = zikaron_send::node::fee_reading(&eps(&[&high, &low]), CHAIN);
    assert!(r.fees.from_chain, "{r:?}");
    assert_eq!(r.fees.priority, 30_000_000, "the lower median of the two, not the ceiling: {r:?}");
    let (nulls, _) = node(Arc::new(|method, _| match method {
        "eth_chainId" => format!("\"0x{CHAIN:x}\""),
        "eth_blockNumber" => "\"0x40\"".into(),
        "eth_getBlockByNumber" => "{\"baseFeePerGas\":\"0x3b9aca00\",\"number\":\"0x40\",\"timestamp\":\"0x64\"}".into(),
        _ => "null".into(),
    }));
    let one = zikaron_send::node::fee_reading(&eps(&[&nulls, &high]), CHAIN);
    assert_eq!(one.fees.priority, zikaron_anchor::send::PRIORITY_FEE.min(700_000_000), "a node keeping no fee history gives no number: {one:?}");
}

fn landed_node() -> (String, Arc<Mutex<Vec<String>>>) {
    node(Arc::new(|method, _| match method {
        "eth_chainId" => format!("\"0x{CHAIN:x}\""),
        "eth_blockNumber" => "\"0x40\"".into(),
        "eth_getTransactionReceipt" => "{\"status\":\"0x1\",\"blockNumber\":\"0x30\"}".into(),
        _ => "null".into(),
    }))
}

fn beyond_chain_id(asked: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    asked.lock().expect("asked").iter().filter(|m| m.as_str() != "eth_chainId").cloned().collect()
}

#[test]
fn a_receipt_waited_on_beside_another_chains_node_is_read_landed_and_that_node_is_asked_nothing_past_its_chain() {
    quiet();
    let tx = format!("0x{}", "ab".repeat(32));
    let id = format!("0x{}", "cd".repeat(32));
    let backoff = [std::time::Duration::from_millis(50)];
    for first_other in [false, true] {
        let (live, _) = landed_node();
        let (other, other_asked) = chain_node(OTHER, 1);
        let urls: Vec<NodeAddr> = if first_other { vec![NodeAddr::new(&other), NodeAddr::new(&live)] } else { vec![NodeAddr::new(&live), NodeAddr::new(&other)] };
        let heard = batch::wait(urls, CHAIN, vec![tx.clone()], vec![id.clone()], None, std::time::Duration::from_secs(4), &backoff, None, None, None).expect("waited");
        assert!(matches!(heard.receipt, batch::Receipt::Included { status: 1, block: 0x30 }), "{first_other}: {:?}", heard.receipt);
        assert!(heard.confirmed, "{first_other}");
        assert_eq!(beyond_chain_id(&other_asked), Vec::<String>::new(), "{first_other}: the other chain's node");
    }
}

#[test]
fn a_receipt_waited_on_where_every_node_serves_another_chain_asks_them_nothing_past_their_chain_and_the_stuck_batch_says_so() {
    quiet();
    let tx = format!("0x{}", "ab".repeat(32));
    let id = format!("0x{}", "cd".repeat(32));
    let who = batch::Sender { from: [0x22; 20], registry: [0x11; 20] };
    let (one, one_asked) = chain_node(OTHER, 1);
    let (two, two_asked) = chain_node(OTHER, 1);
    let urls = vec![NodeAddr::new(&one), NodeAddr::new(&two)];
    let heard = batch::wait(urls, CHAIN, vec![tx], vec![id], None, std::time::Duration::from_secs(4), &[std::time::Duration::from_millis(50)], Some(who), Some(4), None).expect("heard, not refused");
    assert_eq!(heard.receipt, batch::Receipt::NotYet);
    assert!(!heard.confirmed);
    let stuck = heard.stuck.expect("a stuck batch");
    assert_eq!((stuck.offer, stuck.unpriced), (batch::Offer::PriceUnread, Some(batch::Unpriced::OtherChain)));
    assert_eq!(beyond_chain_id(&one_asked), Vec::<String>::new(), "the first node");
    assert_eq!(beyond_chain_id(&two_asked), Vec::<String>::new(), "the second node");
}
