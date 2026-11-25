use app::chainx::{self, Endpoint};
use app::fault::Known;

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
    assert_eq!(chainx::head_block(&eps(&[&high, &near]), 31337).expect("a head"), (0xf0, 2));
    assert_eq!(chainx::MAX_HEAD_LAG, 64);
    let f = chainx::head_block(&eps(&[&high, &far]), 31337).expect_err("a node behind");
    assert_eq!(f.which(), Some(Known::Disagree));
    assert!(f.tail().contains(&far) && f.tail().contains("128") && f.tail().contains("256"), "{}", f.tail());
    let gone = dead();
    assert_eq!(chainx::head_block(&eps(&[&gone, &high]), 31337).expect("a head"), (0x100, 1));
    assert_eq!(chainx::head_time(&eps(&[&near, &high]), 31337).expect("a time"), (0x64, 0x100, 2));
}

#[test]
fn a_silent_node_is_passed_over_named_and_the_reading_is_single_source() {
    quick();
    let (live, gone) = (rpc_node(node_at_256), dead());
    let (wei, r) = chainx::balance(&eps(&[&gone, &live]), 31337, &app::key::Address([0x11; 20])).expect("a balance");
    assert_eq!((wei, r.sources, r.single_source), (0x2a, 1, true));
    assert_eq!(r.unanswered.len(), 1);
    assert!(app::chainx::said_of(&r.unanswered[0]).contains(&gone), "the silent node is named: {:?}", r.unanswered);
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
    assert!(r.unanswered.iter().map(app::chainx::said_of).any(|u| u.contains(&other) && u.contains(&app::lang::filln(app::lang::Key::TailWrongChainNode, &[&other, "1", "31337"]))), "named: {:?}", r.unanswered);
    let (fees, left) = chainx::fees(&eps(&[&other, &good]), 31337);
    assert_ne!(fees, zikaron_anchor::send::Fees::fallback());
    assert!(left.len() == 1 && left[0].contains(&other) && left[0].contains(&app::lang::filln(app::lang::Key::TailWrongChainNode, &[&other, "1", "31337"])), "named: {left:?}");
    let other2 = rpc_node(node_of_chain_1);
    let f = chainx::balance(&eps(&[&other2]), 31337, &app::key::Address([0x11; 20])).expect_err("wrong chain");
    assert_eq!(f.which(), Some(Known::WrongChain));
    assert!(f.tail().contains(&other2), "{}", f.tail());
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
    let has2 = rpc_node(has_tx);
    let r = chainx::ask(&eps(&[&has, &has2]), "eth_getTransactionByHash", &q).expect("agreed");
    assert_eq!((r.sources, r.single_source), (2, false));
    assert!(r.value.member("hash").is_none());
    let lacks2 = rpc_node(lacks_tx);
    assert_eq!(chainx::ask(&eps(&[&lacks, &lacks2]), "eth_getTransactionByHash", &q).expect("none").value, zikaron::json::Value::Null);
}

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
        assert_eq!(f.which(), Some(Known::ChainShape), "{form}: {f:?}");
        assert!(f.tail().contains(&url), "{form}: the node named: {}", f.tail());
        let good = rpc_node(node_at_256);
        assert_eq!(chainx::head_block(&eps(&[&url, &good]), 31337).expect(form), (0x100, 1), "{form}: the one that reads");
    }
}

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
            Ok((_, r)) => assert!(r.unanswered.iter().map(app::chainx::said_of).all(|u| !u.contains(KEY)), "{form}: {:?}", r.unanswered),
            Err(f) => assert!(!f.tail().contains(KEY) && !f.evidence().contains(KEY), "{form}: {}", f.evidence()),
        }
        match chainx::head_block(&eps, 31337) {
            Ok(_) => {}
            Err(f) => assert!(!f.evidence().contains(KEY), "{form}: {}", f.evidence()),
        }
    }
}

#[test]
fn a_node_item_that_does_not_read_is_said_by_its_length() {
    const KEY: &str = "k3yInTheItem55";
    for item in [format!("31337=http://127.0.0.1:1/{KEY} x"), format!("http://127.0.0.1:1/{KEY}"), format!("chain=http://127.0.0.1:1/{KEY}")] {
        let said = app::chainx::typed(&item).expect_err("refused");
        assert!(!said.contains(KEY), "{item}: the key is said: {said}");
        assert!(said.contains(&zikaron_net::sayable(&item)), "{item}: said as a node is said: {said}");
    }
}

type Answer = std::sync::Arc<dyn Fn(&str, usize) -> Option<String> + Send + Sync>;
type Asked = std::sync::Arc<std::sync::Mutex<Vec<String>>>;
fn counted_node(answer: Answer) -> (String, Asked) {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let asked: Asked = Default::default();
    let log = asked.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (answer, log) = (answer.clone(), log.clone());
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
                let n = {
                    let mut g = log.lock().expect("log");
                    let n = g.iter().filter(|m| **m == method).count();
                    g.push(method.clone());
                    n
                };
                let Some(member) = answer(&method, n) else { return };
                let body = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",{member}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    (url, asked)
}

fn result_256(m: &str) -> Option<String> {
    node_at_256(m).map(|r| format!("\"result\":{r}"))
}

fn times(asked: &Asked, method: &str) -> usize {
    asked.lock().expect("log").iter().filter(|m| *m == method).count()
}

const BUSY: &str = "\"error\":{\"code\":-32603,\"message\":\"service temporarily unavailable\"}";

#[test]
fn a_node_that_gave_nothing_is_asked_nothing_more_in_the_read() {
    quick();
    chainx::forget_serving();
    let forms: [(&str, Answer, &[(&str, usize)]); 3] = [
        ("the head refused", std::sync::Arc::new(|m: &str, _| if m == "eth_blockNumber" { Some(BUSY.into()) } else { result_256(m) }), &[("eth_chainId", 1), ("eth_blockNumber", 2), ("eth_getBalance", 0)]),
        ("the head no number", std::sync::Arc::new(|m: &str, _| if m == "eth_blockNumber" { Some("\"result\":\"x\"".into()) } else { result_256(m) }), &[("eth_chainId", 1), ("eth_blockNumber", 1), ("eth_getBalance", 0)]),
        ("the way broken", std::sync::Arc::new(|_: &str, _| None), &[("eth_blockNumber", 0), ("eth_getBalance", 0)]),
    ];
    for (form, answer, want) in forms {
        let (bad, asked) = counted_node(answer);
        let good = rpc_node(node_at_256);
        let (wei, r) = chainx::balance(&eps(&[&bad, &good]), 31337, &app::key::Address([0x11; 20])).expect(form);
        assert_eq!((wei, r.sources, r.single_source), (0x2a, 1, true), "{form}");
        for (method, n) in want {
            assert_eq!(times(&asked, method), *n, "{form}: {method}");
        }
        let before = times(&asked, "eth_blockNumber");
        let _ = chainx::head_block(&eps(&[&bad, &good]), 31337);
        assert!(times(&asked, "eth_blockNumber") > before, "{form}: a new read asks again");
    }
}

#[test]
fn a_node_saying_not_yet_is_asked_again_in_the_read() {
    quick();
    let tx = "{\"blockNumber\":\"0x5\",\"from\":\"0x11\",\"input\":\"0x\"}";
    let (late, asked) = counted_node(std::sync::Arc::new(move |m: &str, n| match m {
        "eth_getTransactionByHash" if n == 0 => Some("\"result\":null".into()),
        "eth_getTransactionByHash" => Some(format!("\"result\":{tx}")),
        other => result_256(other),
    }));
    let (has, _) = counted_node(std::sync::Arc::new(move |m: &str, _| match m {
        "eth_getTransactionByHash" => Some(format!("\"result\":{tx}")),
        other => result_256(other),
    }));
    let q = zikaron::json::Value::Arr(vec![zikaron::json::Value::Str(format!("0x{}", "ab".repeat(32)))]);
    let r = chainx::ask(&eps(&[&late, &has]), "eth_getTransactionByHash", &q).expect("both have it");
    assert_eq!((r.sources, r.single_source, times(&asked, "eth_getTransactionByHash")), (2, false, 2));
}

#[test]
fn a_node_passed_over_is_named_once_in_the_read() {
    quick();
    chainx::forget_serving();
    let (gone, asked) = counted_node(std::sync::Arc::new(|_: &str, _| None));
    let good = rpc_node(node_at_256);
    let (_, r) = chainx::balance(&eps(&[&gone, &good]), 31337, &app::key::Address([0x11; 20])).expect("a balance");
    assert_eq!(r.unanswered.len(), 1, "{:?}", r.unanswered);
    assert!(app::chainx::said_of(&r.unanswered[0]).contains(&gone), "{:?}", r.unanswered);
    let n = asked.lock().expect("log").len();
    {
        let _read = chainx::OneRead::open();
        let (_, r) = chainx::balance(&eps(&[&gone, &good]), 31337, &app::key::Address([0x11; 20])).expect("a balance");
        assert_eq!(r.unanswered.iter().map(app::chainx::said_of).filter(|u| u.contains(&gone)).count(), 1, "{:?}", r.unanswered);
        assert!(chainx::head_time(&eps(&[&gone, &good]), 31337).is_ok());
    }
    let asked_in_read = asked.lock().expect("log")[n..].to_vec();
    assert!(asked_in_read.iter().all(|m| m == "eth_chainId"), "only its first question: {asked_in_read:?}");
}

#[test]
fn the_questions_outside_a_read_still_ask_a_node_it_passed_over() {
    quick();
    chainx::forget_serving();
    let (flaky, asked) = counted_node(std::sync::Arc::new(|m: &str, _| match m {
        "eth_blockNumber" => Some(BUSY.into()),
        "eth_getTransactionByHash" => Some("\"result\":{\"blockNumber\":\"0x5\",\"from\":\"0x11\",\"input\":\"0x\"}".into()),
        "eth_getTransactionReceipt" => Some("\"result\":{\"blockNumber\":\"0x5\",\"status\":\"0x1\"}".into()),
        "eth_getLogs" => Some("\"result\":[]".into()),
        other => result_256(other),
    }));
    let good = rpc_node(node_at_256);
    let _read = chainx::OneRead::open();
    assert!(chainx::head_block(&eps(&[&flaky, &good]), 31337).is_ok());
    assert_eq!(times(&asked, "eth_blockNumber"), 2);
    let nodes: Vec<(String, Box<dyn zikaron_anchor::rpc::Endpoint + Send>)> = vec![(flaky.clone(), chainx::endpoint_at(&flaky).expect("opens"))];
    let _ = zikaron_anchor::endpoints::ask_each(nodes, "eth_blockNumber", &zikaron::json::Value::Arr(Vec::new()), zikaron_anchor::endpoints::every_answer);
    assert_eq!(times(&asked, "eth_blockNumber"), 3);
    assert!(chainx::tx_as_held(&[app::chainx::NodeAddr::new(flaky.clone())], 31337, &format!("0x{}", "ab".repeat(32))).held().is_some());
    assert_eq!(times(&asked, "eth_getTransactionByHash"), 1);
    let mut ep = chainx::endpoint_at(&flaky).expect("opens");
    let mut eps_r: Vec<&mut dyn zikaron_anchor::rpc::Endpoint> = vec![ep.as_mut() as &mut dyn zikaron_anchor::rpc::Endpoint];
    assert!(matches!(zikaron_anchor::send::confirm_each(&mut eps_r, &[0xab; 32], std::time::Duration::ZERO, &[]), zikaron_anchor::send::Confirm::Included { .. }));
    assert_eq!(times(&asked, "eth_getTransactionReceipt"), 1);
    let mut ep = chainx::endpoint_at(&flaky).expect("opens");
    let filter = zikaron_anchor::scan::LogFilter { addresses: vec![[0x11; 20]], topics: Vec::new() };
    assert!(zikaron_anchor::scan::logs_over_window(ep.as_mut(), 31337, &filter, 1, 10).is_ok());
    assert_eq!(times(&asked, "eth_getLogs"), 1);
}

#[test]
fn an_answer_the_reading_cannot_take_passes_the_node_over_in_the_read() {
    quick();
    let wide: Answer = std::sync::Arc::new(|m: &str, _| if m == "eth_blockNumber" { Some("\"result\":\"0x10000000000000000\"".into()) } else { result_256(m) });
    let timeless: Answer = std::sync::Arc::new(|m: &str, _| if m == "eth_getBlockByNumber" { Some("\"result\":{\"number\":\"0x100\"}".into()) } else { result_256(m) });
    for (form, answer, first) in [("a head past 64 bits", wide, "eth_blockNumber"), ("a block with no time", timeless, "eth_getBlockByNumber")] {
        let (odd, asked) = counted_node(answer);
        let good = rpc_node(node_at_256);
        let _read = chainx::OneRead::open();
        let first_ok = if first == "eth_blockNumber" { chainx::head_block(&eps(&[&odd, &good]), 31337).is_ok() } else { chainx::head_time(&eps(&[&odd, &good]), 31337).is_ok() };
        assert!(first_ok, "{form}: the other node's reading");
        let n = asked.lock().expect("log").len();
        let q = zikaron::json::Value::Arr(vec![zikaron::json::Value::Str(format!("0x{}", "11".repeat(20))), zikaron::json::Value::Str("0x100".into())]);
        let r = chainx::ask(&eps(&[&odd, &good]), "eth_getBalance", &q).expect(form);
        assert_eq!(asked.lock().expect("log").len(), n, "{form}: asked nothing more in the read");
        assert_eq!((r.sources, r.unanswered.iter().map(app::chainx::said_of).filter(|u| u.contains(&odd)).count()), (1, 1), "{form}: {:?}", r.unanswered);
    }
}

#[test]
fn a_rest_row_with_no_key_here_is_asked_nothing_and_named_once_in_each_reading() {
    quick();
    let good = rpc_node(node_at_256);
    let mut nodes = eps(&[&good]);
    nodes.push(Endpoint::at(31337, app::chainx::NodeAddr::rest("https://api.etherscan.io/v2/api", 31337)));
    let _read = chainx::OneRead::open();
    let head = chainx::ask(&nodes, "eth_blockNumber", &zikaron::json::Value::Arr(Vec::new())).expect("the head from the other node");
    let q = zikaron::json::Value::Arr(vec![zikaron::json::Value::Str(format!("0x{}", "11".repeat(20))), zikaron::json::Value::Str("0x100".into())]);
    let code = chainx::ask(&nodes, "eth_getCode", &q).expect("a second question of the read");
    for r in [head, code] {
        assert_eq!(r.sources, 1, "{:?}", r.unanswered);
        assert!(r.single_source, "one place answered");
        let named: Vec<String> = r.unanswered.iter().map(app::chainx::said_of).filter(|u| u.contains("NO_KEY_HERE")).collect();
        assert_eq!(named.len(), 1, "{:?}", r.unanswered);
        assert!(named[0].contains("https://api.etherscan.io") && !named[0].contains("/v2/api") && !named[0].contains("#rest"), "said by its service alone: {}", named[0]);
    }
}
