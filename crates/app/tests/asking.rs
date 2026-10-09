//! Chain reads ask every node at once and judge the answers by one table: the head is the smallest, a node
//! too far behind is refused by name, a silent node is skipped and named (single source), a transaction some
//! nodes lack is refused naming them, and a node serving another chain is left out and named. Nodes are
//! local ports in this process; no network.

use app::chainx::{self, Endpoint};
use app::fault::Known;

/// A JSON-RPC node on a local port answering each request with `answer(method)` (`None`: close the
/// connection unanswered), one request per connection.
fn rpc_node(answer: fn(&str) -> Option<String>) -> String {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
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
                let method = body.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                let id = body.split("\"id\":").nth(1).and_then(|r| r.split(|c| c == ',' || c == '}').next()).unwrap_or("1").trim().to_string();
                let Some(result) = answer(&method) else { return };
                let body = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{result}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    url
}

fn dead() -> String {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    format!("http://{}", l.local_addr().expect("addr"))
}

fn eps(urls: &[&String]) -> Vec<Endpoint> {
    urls.iter().map(|u| Endpoint::parse(&format!("31337={u}")).expect("an endpoint")).collect()
}

fn quick() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
}

fn node_at_256(m: &str) -> Option<String> {
    Some(match m {
        "eth_chainId" => "\"0x7a69\"".into(),
        "eth_blockNumber" => "\"0x100\"".into(),
        "eth_getBalance" => "\"0x2a\"".into(),
        "eth_getBlockByNumber" => "{\"baseFeePerGas\":\"0x7\",\"number\":\"0x100\",\"timestamp\":\"0x64\"}".into(),
        "eth_feeHistory" => "{\"oldestBlock\":\"0xfb\",\"reward\":[[\"0x3\"],[\"0x3\"]]}".into(),
        _ => "null".into(),
    })
}
fn node_at_240(m: &str) -> Option<String> {
    Some(match m {
        "eth_blockNumber" => "\"0xf0\"".into(),
        "eth_getBlockByNumber" => "{\"baseFeePerGas\":\"0x7\",\"number\":\"0xf0\",\"timestamp\":\"0x60\"}".into(),
        other => return node_at_256(other),
    })
}
fn node_at_128(m: &str) -> Option<String> {
    Some(match m {
        "eth_blockNumber" => "\"0x80\"".into(),
        other => return node_at_256(other),
    })
}
fn node_of_chain_1(m: &str) -> Option<String> {
    Some(match m {
        "eth_chainId" => "\"0x1\"".into(),
        "eth_getBalance" => "\"0x99\"".into(),
        other => return node_at_256(other),
    })
}

#[test]
fn the_head_is_the_smallest_and_a_node_too_far_behind_is_named() {
    quick();
    let (high, near, far) = (rpc_node(node_at_256), rpc_node(node_at_240), rpc_node(node_at_128));
    // Sixteen apart: the smallest head, both nodes counted.
    assert_eq!(chainx::head_block(&eps(&[&high, &near]), 31337).expect("a head"), (0xf0, 2));
    // 128 apart, beyond `MAX_HEAD_LAG`: refused, naming the node behind, the lag and the highest head.
    assert_eq!(chainx::MAX_HEAD_LAG, 64);
    let f = chainx::head_block(&eps(&[&high, &far]), 31337).expect_err("a node behind");
    assert_eq!(f.which(), Some(Known::Disagree));
    assert!(f.tail().contains(&far) && f.tail().contains("128") && f.tail().contains("256"), "{}", f.tail());
    // A dead node and a live one: the live one's head, single source.
    let gone = dead();
    assert_eq!(chainx::head_block(&eps(&[&gone, &high]), 31337).expect("a head"), (0x100, 1));
    // The latest block is the highest by number, with its time.
    assert_eq!(chainx::head_time(&eps(&[&near, &high]), 31337).expect("a time"), (0x64, 0x100, 2));
}

#[test]
fn a_silent_node_is_passed_over_named_and_the_reading_is_single_source() {
    quick();
    let (live, gone) = (rpc_node(node_at_256), dead());
    let (wei, r) = chainx::balance(&eps(&[&gone, &live]), 31337, &app::key::Address([0x11; 20])).expect("a balance");
    assert_eq!((wei, r.sources, r.single_source), (0x2a, 1, true));
    assert_eq!(r.unanswered.len(), 1);
    assert!(r.unanswered[0].contains(&gone), "the silent node is named: {:?}", r.unanswered);
    // Both answering: agreed, nobody named.
    let live2 = rpc_node(node_at_256);
    let (_, r) = chainx::balance(&eps(&[&live, &live2]), 31337, &app::key::Address([0x11; 20])).expect("a balance");
    assert_eq!((r.sources, r.single_source, r.unanswered.len()), (2, false, 0));
}

#[test]
fn a_node_serving_another_chain_is_left_out_of_balance_and_fees_and_named() {
    quick();
    let (good, other) = (rpc_node(node_at_256), rpc_node(node_of_chain_1));
    let (wei, r) = chainx::balance(&eps(&[&other, &good]), 31337, &app::key::Address([0x11; 20])).expect("a balance");
    assert_eq!(wei, 0x2a, "the other chain's balance never enters");
    // Named with the chain it serves.
    assert!(r.unanswered.iter().any(|u| u.contains(&other) && u.contains(&app::lang::filln(app::lang::Key::TailWrongChainNode, &[&other, "1", "31337"]))), "named: {:?}", r.unanswered);
    // Fees come from the node that serves the chain: its base fee, not the fallback.
    let (fees, left) = chainx::fees(&eps(&[&other, &good]), 31337);
    assert_ne!(fees, zikaron_anchor::send::Fees::fallback());
    // The node left out is named with them, as in the balance reading.
    assert!(left.len() == 1 && left[0].contains(&other) && left[0].contains(&app::lang::filln(app::lang::Key::TailWrongChainNode, &[&other, "1", "31337"])), "named: {left:?}");
    // Every node serving another chain: refused by name.
    let other2 = rpc_node(node_of_chain_1);
    let f = chainx::balance(&eps(&[&other2]), 31337, &app::key::Address([0x11; 20])).expect_err("wrong chain");
    assert_eq!(f.which(), Some(Known::WrongChain));
    assert!(f.tail().contains(&other2), "{}", f.tail());
    // Fees with every node on another chain: the fallback, every node named.
    let (fees, left) = chainx::fees(&eps(&[&other2]), 31337);
    assert_eq!(fees, zikaron_anchor::send::Fees::fallback());
    assert!(left.iter().any(|w| w.contains(&other2)), "named: {left:?}");
}

fn has_tx(m: &str) -> Option<String> {
    Some(match m {
        "eth_getTransactionByHash" => "{\"blockNumber\":\"0x5\",\"from\":\"0x1111111111111111111111111111111111111111\",\"input\":\"0x\",\"hash\":\"0xab\"}".into(),
        _ => "null".into(),
    })
}
fn lacks_tx(_m: &str) -> Option<String> {
    Some("null".into())
}

#[test]
fn a_transaction_one_node_has_and_another_not_yet_is_refused_naming_the_one_without() {
    quick();
    let (has, lacks) = (rpc_node(has_tx), rpc_node(lacks_tx));
    let q = zikaron::json::Value::Arr(vec![zikaron::json::Value::Str(format!("0x{}", "ab".repeat(32)))]);
    let f = chainx::ask(&eps(&[&has, &lacks]), "eth_getTransactionByHash", &q).expect_err("split");
    assert_eq!(f.which(), Some(Known::Disagree));
    assert!(f.tail().contains(&lacks), "the node without it is named: {}", f.tail());
    // Both have it: agreed on the shared facts (a member only one node adds, like `hash`, is not a difference).
    let has2 = rpc_node(has_tx);
    let r = chainx::ask(&eps(&[&has, &has2]), "eth_getTransactionByHash", &q).expect("agreed");
    assert_eq!((r.sources, r.single_source), (2, false));
    assert!(r.value.member("hash").is_none());
    // Neither has it: "not yet" is the reading.
    let lacks2 = rpc_node(lacks_tx);
    assert_eq!(chainx::ask(&eps(&[&lacks, &lacks2]), "eth_getTransactionByHash", &q).expect("none").value, zikaron::json::Value::Null);
}

// A block height reads only as a `0x` quantity that fits 64 bits. Zero and u64::MAX read; anything else is
// refused by name, never taken as zero, and a node whose height reads wins over one whose does not.

fn height_zero(m: &str) -> Option<String> {
    Some(match m {
        "eth_blockNumber" => "\"0x0\"".into(),
        other => return node_at_256(other),
    })
}
fn height_max(m: &str) -> Option<String> {
    Some(match m {
        "eth_blockNumber" => "\"0xffffffffffffffff\"".into(),
        other => return node_at_256(other),
    })
}
fn height_past_max(m: &str) -> Option<String> {
    Some(match m {
        "eth_blockNumber" => "\"0x10000000000000000\"".into(),
        other => return node_at_256(other),
    })
}
fn height_bare_prefix(m: &str) -> Option<String> {
    Some(match m {
        "eth_blockNumber" => "\"0x\"".into(),
        other => return node_at_256(other),
    })
}
fn height_word(m: &str) -> Option<String> {
    Some(match m {
        "eth_blockNumber" => "\"latest\"".into(),
        other => return node_at_256(other),
    })
}

#[test]
fn a_height_that_does_not_read_as_64_bits_is_named_never_zero() {
    quick();
    let zero = rpc_node(height_zero);
    assert_eq!(chainx::head_block(&eps(&[&zero]), 31337).expect("zero reads"), (0, 1), "zero is a height");
    let max = rpc_node(height_max);
    assert_eq!(chainx::head_block(&eps(&[&max]), 31337).expect("the largest reads"), (u64::MAX, 1));
    for (form, node) in [("pastMax", height_past_max as fn(&str) -> Option<String>), ("barePrefix", height_bare_prefix), ("word", height_word)] {
        let url = rpc_node(node);
        let f = chainx::head_block(&eps(&[&url]), 31337).expect_err(form);
        assert_eq!(f.which(), Some(Known::Unreachable), "{form}: {f:?}");
        assert!(f.tail().contains(&url), "{form}: the node named: {}", f.tail());
        // Beside a node whose height reads, that node's height is the head.
        let good = rpc_node(node_at_256);
        assert_eq!(chainx::head_block(&eps(&[&url, &good]), 31337).expect(form), (0x100, 1), "{form}: the one that reads");
    }
}

// The balance before sending is read at the smallest head and compared across nodes; the funds check passes
// exactly enough and refuses one wei less with both numbers.

fn node_at_255(m: &str) -> Option<String> {
    Some(match m {
        "eth_blockNumber" => "\"0xff\"".into(),
        other => return node_at_256(other),
    })
}
fn node_richer(m: &str) -> Option<String> {
    Some(match m {
        "eth_getBalance" => "\"0x2b\"".into(),
        other => return node_at_256(other),
    })
}

#[test]
fn the_balance_before_sending_is_pinned_and_compared() {
    quick();
    chainx::forget_serving();
    let who = app::key::Address([0x11; 20]);
    let (a, behind) = (rpc_node(node_at_256), rpc_node(node_at_255));
    let (n, r) = chainx::balance(&eps(&[&a, &behind]), 31337, &who).expect("a node one block behind agrees at the pinned block");
    assert_eq!((n, r.sources, r.single_source), (0x2a, 2, false));
    let richer = rpc_node(node_richer);
    let f = chainx::balance(&eps(&[&a, &richer]), 31337, &who).expect_err("two answers that differ");
    assert_eq!(f.which(), Some(Known::Disagree), "{f:?}");
    let gone = dead();
    let (n, r) = chainx::balance(&eps(&[&gone, &a]), 31337, &who).expect("one node alone");
    assert_eq!((n, r.sources, r.single_source), (0x2a, 1, true));
    assert!(app::action::funds_check(42, 42).is_ok(), "exactly enough");
    let short = app::action::funds_check(42, 41).expect_err("one wei short");
    assert_eq!((short.which(), app::action::funds_of(short.tail())), (Some(Known::InsufficientFunds), Some((42, 41))));
}

// A key in a node URL's path or query never appears in a reading, a refusal or log evidence: nodes are named
// by scheme, host and port only.

fn refuses_balance(m: &str) -> Option<String> {
    Some(match m {
        "eth_getBalance" => "HTTP 503 Service Unavailable".into(),
        other => return node_at_256(other),
    })
}

#[test]
fn a_key_in_a_node_address_is_in_no_sentence() {
    quick();
    chainx::forget_serving();
    let who = app::key::Address([0x11; 20]);
    const KEY: &str = "S3CRETk3y";
    let gone = dead();
    let live = rpc_node(node_at_256);
    let refusing = rpc_node(refuses_balance);
    for (form, eps) in [
        ("deadInPath", eps(&[&format!("{gone}/v3/{KEY}")])),
        ("deadInQuery", eps(&[&format!("{gone}/rpc?apikey={KEY}")])),
        ("aliveBesideDead", eps(&[&format!("{gone}/v3/{KEY}"), &format!("{live}/v3/{KEY}")])),
        ("refusingInPath", eps(&[&format!("{refusing}/v3/{KEY}")])),
    ] {
        match chainx::balance(&eps, 31337, &who) {
            Ok((_, r)) => assert!(r.unanswered.iter().all(|u| !u.contains(KEY)), "{form}: {:?}", r.unanswered),
            Err(f) => assert!(!f.tail().contains(KEY) && !f.evidence().contains(KEY), "{form}: {}", f.evidence()),
        }
        match chainx::head_block(&eps, 31337) {
            Ok(_) => {}
            Err(f) => assert!(!f.evidence().contains(KEY), "{form}: {}", f.evidence()),
        }
    }
}

/// A malformed node item (`<chain id>=<url>`) is refused without echoing it: it is named as nodes are named
/// (`zikaron_net::sayable`: by length, or scheme, host and port), so a typed key reaches no sentence or log.
#[test]
fn a_node_item_that_does_not_read_is_said_by_its_length() {
    const KEY: &str = "k3yInTheItem55";
    for item in [format!("31337=http://127.0.0.1:1/{KEY} x"), format!("http://127.0.0.1:1/{KEY}"), format!("chain=http://127.0.0.1:1/{KEY}")] {
        let said = app::chainx::Endpoint::typed(&item).expect_err("refused");
        assert!(!said.contains(KEY), "{item}: the key is said: {said}");
        assert!(said.contains(&zikaron_net::sayable(&item)), "{item}: said as a node is said: {said}");
    }
}
