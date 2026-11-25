use std::io::{Read, Write};
use zikaron::json::Value;
use zikaron_send::face::{input, Lane, Left};
use zikaron_send::node::{Endpoint, NodeAddr};

const BOOK_CODE: &str = include_str!("data/SuccessionBook.runtime.hex");
const HEAD: u64 = 12_200_000;
const SPACING: u64 = 12;

fn node(chain: u64, head_time: u64, below: bool) -> NodeAddr {
    let book = zikaron_anchor::book::at(chain).expect("the chain's book row");
    let code = BOOK_CODE.trim().to_string();
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
            let reply = |req: &Value| -> String {
                let id = req.member("id").map(|v| String::from_utf8_lossy(&zikaron::json::canon_bytes(v)).into_owned()).unwrap_or_else(|| "null".into());
                let method = req.member("method").and_then(|m| m.as_str()).unwrap_or("");
                let params: Vec<Value> = req.member("params").and_then(Value::as_arr).cloned().unwrap_or_default();
                let block = |n: u64| format!("{{\"hash\":\"0x{:064x}\",\"number\":\"0x{n:x}\",\"parentHash\":\"0x{:064x}\",\"timestamp\":\"0x{:x}\"}}", n + 1, n, head_time - (HEAD - n) * SPACING);
                let result = match method {
                    "eth_chainId" => format!("\"0x{chain:x}\""),
                    "eth_blockNumber" => format!("\"0x{HEAD:x}\""),
                    "eth_getBlockByNumber" => match params.first().and_then(Value::as_str) {
                        Some("latest") => block(HEAD),
                        Some(n) => match u64::from_str_radix(n.trim_start_matches("0x"), 16) {
                            Ok(n) if n == HEAD || (below && n < HEAD) => block(n),
                            _ => "null".into(),
                        },
                        None => "null".into(),
                    },
                    "eth_getCode" => {
                        let at = params.first().and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
                        if at == zikaron::hexfmt::encode(&book) {
                            format!("\"{code}\"")
                        } else {
                            "\"0x\"".into()
                        }
                    }
                    "eth_getLogs" => "[]".into(),
                    _ => "null".into(),
                };
                format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{result}}}")
            };
            let out = match &req {
                Value::Arr(xs) => format!("[{}]", xs.iter().map(reply).collect::<Vec<_>>().join(",")),
                one => reply(one),
            };
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{out}", out.len()).as_bytes());
        }
    });
    NodeAddr::new(format!("http://127.0.0.1:{port}"))
}

#[test]
fn a_chain_left_out_while_its_end_is_found_does_not_hold_the_moment() {
    let (early, late) = (11_155_111u64, 1u64);
    let (t_early, t_late) = (1_800_000_000u64, 1_800_003_600u64);
    let eps = vec![Endpoint::at(early, node(early, t_early, false)), Endpoint::at(late, node(late, t_late, true))];
    let lanes = [Lane { chain: early, registry: [0x22; 20], from_block: HEAD - 100 }, Lane { chain: late, registry: [0x22; 20], from_block: HEAD - 100 }];
    let built = input(&eps, &[0x2a; 20], &lanes, &[], &zikaron_send::Bare).expect("the chain left gives a face input");
    assert!(matches!(built.left.as_slice(), [(c, Left::Unread(_))] if *c == early), "the chain whose end is not found is left out by name: {:?}", built.left);
    assert_eq!(built.moment, t_late - 1, "the moment is the chain left's, one second before its head, not the left-out chain's");
    assert_eq!(built.ends, vec![(late, HEAD - 1)], "the chain left ends at its last block at or before its own moment");
}
