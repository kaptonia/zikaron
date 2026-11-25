use std::io::{Read, Write};
use zikaron::json::Value;
use zikaron_send::node::{Endpoint, NodeAddr};

type Log = std::sync::Arc<std::sync::Mutex<Vec<String>>>;

fn node(serves: u64, code: &'static str, word: &'static str) -> (NodeAddr, Log) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = l.local_addr().expect("its address").port();
    let log: Log = Default::default();
    let kept = log.clone();
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
            kept.lock().unwrap_or_else(|e| e.into_inner()).push(body.clone());
            let req = zikaron::json::parse(body.as_bytes()).unwrap_or(Value::Null);
            let id = req.member("id").map(|v| String::from_utf8_lossy(&zikaron::json::canon_bytes(v)).into_owned()).unwrap_or_else(|| "null".into());
            let result = match req.member("method").and_then(|m| m.as_str()).unwrap_or("") {
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
    (NodeAddr::new(format!("http://127.0.0.1:{port}")), log)
}

fn tx() -> zikaron_anchor::send::Unsigned {
    zikaron_anchor::send::Unsigned { chain_id: 10, nonce: u64::MAX, max_priority_fee: 1, max_fee: 3, gas: 31_500, to: [0x22; 20], value: 5_000, data: Vec::new() }
}

const FEE: &str = "0x000000000000000000000000000000000000000000000000000000000000007b";

#[test]
fn a_chain_that_charges_for_data_is_asked_its_charge_for_the_very_transfer() {
    let (url, log) = node(10, "0x6080", FEE);
    assert_eq!(zikaron_send::one::data_fee(&[Endpoint::at(10, url)], &tx()).expect("read"), 123);
    let asked = log.lock().expect("log").clone();
    let call = asked.iter().find(|r| r.contains("eth_call")).expect("the oracle asked");
    let bytes = tx().payload();
    let mut want = zikaron::cryptox::keccak256(b"getL1Fee(bytes)")[..4].to_vec();
    want.extend_from_slice(&[[0u8; 31].as_slice(), &[32u8]].concat());
    want.extend_from_slice(&[[0u8; 31].as_slice(), &[bytes.len() as u8]].concat());
    want.extend_from_slice(&bytes);
    want.extend(std::iter::repeat_n(0u8, (32 - bytes.len() % 32) % 32));
    assert!(call.contains(&zikaron::hexfmt::encode(&want)), "the very transaction's bytes, ABI-encoded: {call}");
    assert!(call.to_ascii_lowercase().contains("0x420000000000000000000000000000000000000f"), "at the oracle");
}

#[test]
fn a_chain_with_no_oracle_charges_gas_alone_and_is_asked_nothing_more() {
    let (url, log) = node(1, "0x", FEE);
    assert_eq!(zikaron_send::one::data_fee(&[Endpoint::at(1, url)], &tx()).expect("read"), 0);
    assert!(!log.lock().expect("log").iter().any(|r| r.contains("eth_call")), "nothing more asked");
}

#[test]
fn an_answer_no_balance_can_be_is_no_answer() {
    for word in ["0x7b", "0x0100000000000000000000000000000000000000000000000000000000000000"] {
        let (url, _) = node(10, "0x6080", word);
        assert!(zikaron_send::one::data_fee(&[Endpoint::at(10, url)], &tx()).is_err(), "{word}");
    }
}
