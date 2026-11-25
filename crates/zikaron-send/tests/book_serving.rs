use std::io::{Read, Write};
use zikaron::json::Value;
use zikaron_anchor::book::Call;
use zikaron_send::book::{self, State};
use zikaron_send::node::{Endpoint, NodeAddr};
use zikaron_send::recipient::{book_current, Net};
use zikaron_send::Code;

const CHAIN: u64 = 11_155_111;
const OTHER: u64 = 31_337;
const ROOT: [u8; 20] = [0x2a; 20];

fn node(serves: u64, code: &'static str, word: String) -> NodeAddr {
    node_logging(serves, code, word, None)
}

fn node_logging(serves: u64, code: &'static str, word: String, log: Option<std::sync::Arc<std::sync::Mutex<Vec<String>>>>) -> NodeAddr {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = l.local_addr().expect("its address").port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let mut got: Vec<u8> = Vec::new();
            let mut b = [0u8; 8192];
            let body = loop {
                let Ok(n) = s.read(&mut b) else { break String::new() };
                if n == 0 {
                    break String::new();
                }
                got.extend_from_slice(&b[..n]);
                let text = String::from_utf8_lossy(&got).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let length = text[..end].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                    if got.len() >= end + 4 + length {
                        break text[end + 4..].to_string();
                    }
                }
            };
            let req = zikaron::json::parse(body.as_bytes()).unwrap_or(Value::Null);
            let id = req.member("id").map(|v| String::from_utf8_lossy(&zikaron::json::canon_bytes(v)).into_owned()).unwrap_or_else(|| "null".into());
            let method = req.member("method").and_then(|m| m.as_str()).unwrap_or("");
            if let Some(log) = &log {
                log.lock().unwrap_or_else(|e| e.into_inner()).push(method.to_string());
            }
            let result = match method {
                "eth_chainId" => format!("\"0x{serves:x}\""),
                "eth_blockNumber" => "\"0x20\"".into(),
                "eth_getCode" => format!("\"{code}\""),
                "eth_call" => format!("\"{word}\""),
                _ => "null".into(),
            };
            let reply = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{result}}}");
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
        }
    });
    NodeAddr::new(format!("http://127.0.0.1:{port}"))
}

fn word(a: &[u8; 20]) -> String {
    format!("0x{}{}", "0".repeat(24), zikaron::hexfmt::encode(a).trim_start_matches("0x"))
}

fn eps(nodes: &[&NodeAddr]) -> Vec<Endpoint> {
    nodes.iter().map(|u| Endpoint::at(CHAIN, (*u).clone())).collect()
}

#[test]
fn a_node_of_another_chain_is_left_out_of_the_books_reading() {
    let at = zikaron_anchor::book::at(CHAIN).expect("the book's row");
    let own = node(CHAIN, "0x", word(&[0x51; 20]));
    let other = node(OTHER, "0x6001", word(&[0x52; 20]));
    assert_eq!(book::state(&eps(&[&own]), CHAIN).ok(), Some(State::Wrong), "the chain's own node: no book code there");
    assert_eq!(book::state(&eps(&[&own, &other]), CHAIN).ok(), Some(State::Wrong), "the other chain's node left out, not a disagreement");
    assert_eq!(book::address(&eps(&[&own, &other]), CHAIN, &at, Call::Current, &ROOT).ok(), Some([0x51; 20]), "a view asks the chain's own node only");
    let alone = book::state(&eps(&[&other]), CHAIN).err().map(|t| t.code);
    assert_eq!(alone, Some(Code::WrongChain), "every node serving another chain: refused by name");
}

#[test]
fn the_book_in_office_is_read_past_another_chains_node() {
    let own = node(CHAIN, "0x", word(&ROOT));
    let other = node(OTHER, "0x6001", word(&[0x52; 20]));
    let net = |nodes: Vec<NodeAddr>| vec![Net { chain: CHAIN, registry: [0x11; 20], from_block: 0, nodes }];
    assert_eq!(book_current(&ROOT, &net(vec![own.clone(), other.clone()])), (None, vec![format!("{CHAIN} BOOK_CODE")]));
    assert_eq!(book_current(&ROOT, &net(vec![other])), (None, vec![format!("{CHAIN} BOOK WrongChain")]));
}

fn counted(serves: u64, code: &'static str, word: String) -> (NodeAddr, std::sync::Arc<std::sync::Mutex<Vec<String>>>) {
    let asked: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
    (node_logging(serves, code, word, Some(asked.clone())), asked)
}

#[test]
fn a_book_read_asks_the_head_and_the_code_once_per_chain_in_an_action() {
    let at = zikaron_anchor::book::at(CHAIN).expect("the book's row");
    let count = |asked: &std::sync::Arc<std::sync::Mutex<Vec<String>>>, m: &str| asked.lock().unwrap().iter().filter(|x| *x == m).count();
    let read = |e: &[Endpoint]| {
        for who in [[0x2a; 20], [0x2b; 20]] {
            let _ = book::state(e, CHAIN);
            let _ = book::address(e, CHAIN, &at, Call::Current, &who);
        }
    };
    let ((a, asked_a), (b, asked_b)) = (counted(CHAIN, "0x", word(&ROOT)), counted(CHAIN, "0x", word(&ROOT)));
    {
        let _action = zikaron_anchor::action::open(None);
        read(&eps(&[&a, &b]));
    }
    for (n, asked) in [("a", &asked_a), ("b", &asked_b)] {
        assert_eq!(count(asked, "eth_blockNumber"), 1, "{n}: the action's head, once");
        assert_eq!(count(asked, "eth_getCode"), 1, "{n}: the book's code, once for the chain");
        assert_eq!(count(asked, "eth_call"), 2, "{n}: each sender's view, once");
        asked.lock().unwrap().clear();
    }
    read(&eps(&[&a, &b]));
    for (n, asked) in [("a", &asked_a), ("b", &asked_b)] {
        assert_eq!((count(asked, "eth_blockNumber"), count(asked, "eth_getCode"), count(asked, "eth_call")), (4, 2, 2), "{n}: no action, each read asks as before");
    }
}
