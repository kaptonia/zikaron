use std::io::{Read, Write};
use zikaron::json::Value;
use zikaron_anchor::book::Call;
use zikaron_send::book;
use zikaron_send::node::{Endpoint, NodeAddr};
use zikaron_send::Code;

const BOOK_CODE: &str = include_str!("data/SuccessionBook.runtime.hex");
const CHAIN: u64 = 11_155_111;
const R: [u8; 20] = [0x2a; 20];
const K: [u8; 20] = [0x3b; 20];
const OLD: [u8; 20] = [0x4c; 20];

fn node(root: [u8; 20], current: [u8; 20], code: Option<&'static str>) -> NodeAddr {
    let at = zikaron_anchor::book::at(CHAIN).expect("the book's row");
    let code = code.unwrap_or(BOOK_CODE).trim().to_string();
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
            let params: Vec<Value> = req.member("params").and_then(Value::as_arr).cloned().unwrap_or_default();
            let word = |a: &[u8; 20]| format!("\"0x{}{}\"", "0".repeat(24), zikaron::hexfmt::encode(a).trim_start_matches("0x"));
            let result = match method {
                "eth_chainId" => format!("\"0x{CHAIN:x}\""),
                "eth_blockNumber" => "\"0x20\"".into(),
                "eth_getCode" if params.first().and_then(Value::as_str).is_some_and(|a| a.eq_ignore_ascii_case(&zikaron::hexfmt::encode(&at))) => format!("\"{code}\""),
                "eth_getCode" => "\"0x\"".into(),
                "eth_call" => {
                    let data = params.first().and_then(|c| c.member("data")).and_then(Value::as_str).unwrap_or("").to_string();
                    if data.starts_with(&zikaron::hexfmt::encode(&Call::Root.selector())) {
                        word(&root)
                    } else {
                        word(&current)
                    }
                }
                _ => "null".into(),
            };
            let reply = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{result}}}");
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
        }
    });
    NodeAddr::new(format!("http://127.0.0.1:{port}"))
}

fn seated(n: NodeAddr) -> Result<(), Code> {
    book::seated(&[Endpoint::at(CHAIN, n)], CHAIN, &R, &K, &[R, OLD]).map_err(|t| t.code)
}

#[test]
fn the_book_holds_the_key_in_office_under_the_root_both_views_read() {
    assert_eq!(seated(node(R, K, None)), Ok(()));
    assert_eq!(seated(node(R, R, None)), Err(Code::RootNotInBook), "never entered");
    assert_eq!(seated(node(R, OLD, None)), Err(Code::NotAcceded), "a key the seat moved on from");
    assert_eq!(seated(node([0x5d; 20], K, None)), Err(Code::OfficeElsewhere), "the root under another root: current(R) does not count");
    assert_eq!(seated(node(R, [0x6e; 20], None)), Err(Code::OfficeElsewhere), "another key");
    assert_eq!(seated(node(R, K, Some("0x6001"))), Err(Code::BookCode));
    let h = book::held(&[Endpoint::at(CHAIN, node([0x5d; 20], K, None))], CHAIN, &zikaron_anchor::book::at(CHAIN).unwrap(), &R).expect("read");
    assert_eq!((h.key_under(&R), h.holds(&R, &K)), (None, false));
}
