use std::io::{Read, Write};
use std::net::TcpListener;
use zikaron::json::Value;
use zikaron_send::node::{Endpoint, NodeAddr};
use zikaron_send::Code;

const CHAIN: u64 = 31337;
const BOOK: [u8; 20] = [0x3d; 20];
const FROM: [u8; 20] = [0x15; 20];

fn node(gas: u64) -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
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
                let req = zikaron::json::parse(body.as_bytes()).unwrap_or(Value::Null);
                let method = req.member("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
                let id = body.split("\"id\":").nth(1).and_then(|x| x.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                let result = match method.as_str() {
                    "eth_chainId" => format!("\"0x{CHAIN:x}\""),
                    "eth_blockNumber" => "\"0x40\"".into(),
                    "eth_getBlockByNumber" => "{\"baseFeePerGas\":\"0x3b9aca00\",\"number\":\"0x40\",\"timestamp\":\"0x64\"}".into(),
                    "eth_feeHistory" => "{\"oldestBlock\":\"0x2d\",\"reward\":[[\"0x5f5e100\"]]}".into(),
                    "eth_estimateGas" => format!("\"0x{gas:x}\""),
                    _ => "null".into(),
                };
                let body = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{result}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    url
}

fn eps(url: &str) -> Vec<Endpoint> {
    vec![Endpoint::at(CHAIN, NodeAddr::new(url))]
}

#[test]
fn a_book_call_or_a_gas_transfer_carries_its_estimate_past_the_anchor_ceiling() {
    let declare = zikaron_send::one::Call::book(CHAIN, BOOK, zikaron_anchor::book::Call::Declare, &[0x22; 20]);
    let (gas, fees) = zikaron_send::one::estimate(&eps(&node(247_544)), &FROM, &declare).expect("the book's declare is estimated, not refused");
    assert_eq!((gas, fees.gas_limit), (247_544, 371_316), "one and a half times the estimate, no ceiling");
    assert_eq!(fees.cap_wei(), fees.max_fee as u128 * 371_316, "what the balance check and the card read");
    let transfer = zikaron_send::one::Call::transfer(CHAIN, [0x22; 20], 1);
    let (gas, fees) = zikaron_send::one::estimate(&eps(&node(207_391)), &FROM, &transfer).expect("the gas transfer is estimated, not refused");
    assert_eq!((gas, fees.gas_limit), (207_391, 311_087));
    let over = zikaron_send::one::estimate(&eps(&node(u64::MAX)), &FROM, &declare).expect_err("past the field");
    assert_eq!(over.code, Code::GasRefused);
}

#[test]
fn an_anchor_estimated_past_the_ceiling_is_still_refused() {
    let data = zikaron_anchor::send::registry_calldata(&[[0x77; 32]]);
    let ask = |url: &str| zikaron_send::batch::GasAsk { chain: CHAIN, call: zikaron_anchor::send::estimate_call(&FROM, &[0x11; 20], &data), data: data.clone(), eps: eps(url) };
    let refused = zikaron_send::batch::estimate(&ask(&node(200_001))).expect_err("past the ceiling");
    assert_eq!(refused.code, Code::GasRefused);
    assert!(format!("{:?}", refused.why).contains("200001 > 200000"), "{:?}", refused.why);
    let taken = zikaron_send::batch::estimate(&ask(&node(200_000))).expect("at the ceiling");
    assert_eq!((taken.gas, taken.fees.gas_limit), (200_000, 200_000));
}
