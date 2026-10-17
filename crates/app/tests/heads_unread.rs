//! The chain's head and time readers (`chainx::head_block`, `chainx::head_time`) when every node answers and no
//! answer reads: refused as the chain's shape (`CHAIN_SHAPE`) when some node answered something that is not
//! `null` and does not read, every node named; answers that are all `null` are the nodes' own word that they have
//! none, refused as unreachable. Against nodes started in this process (no network).

use app::chainx::{head_block, head_time, Endpoint};
use app::fault::Known;
use std::io::{Read, Write};

/// A node on a local port answering every question with this JSON-RPC result (raw JSON, `{id}` put in).
fn node(result: &'static str) -> String {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || {
                let mut s = s;
                let mut raw = Vec::new();
                let mut buf = [0u8; 4096];
                let req = loop {
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
                let id = req.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                let body = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{result}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    url
}

fn eps(urls: &[String]) -> Vec<Endpoint> {
    urls.iter().map(|u| Endpoint::at(31337, u.as_str())).collect()
}

#[test]
fn heads_that_do_not_read_are_the_chains_shape() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    // Text that is not a quantity, and a number that is not text.
    let f = head_block(&eps(&[node("\"0xzz\""), node("16")]), 31337).expect_err("no head reads");
    assert_eq!(f.which(), Some(Known::ChainShape), "{f:?}");
    // A quantity too wide for a height (over 64 bits) beside one that is not a quantity.
    let f = head_block(&eps(&[node("\"0x10000000000000000\""), node("\"x\"")]), 31337).expect_err("no head reads");
    assert_eq!(f.which(), Some(Known::ChainShape), "{f:?}");
    // One null beside one that is not a quantity: still the shape.
    let f = head_block(&eps(&[node("null"), node("\"x\"")]), 31337).expect_err("no head reads");
    assert_eq!(f.which(), Some(Known::ChainShape), "{f:?}");
    // Every node says null: their word that they have none.
    let f = head_block(&eps(&[node("null"), node("null")]), 31337).expect_err("no head reads");
    assert_eq!(f.which(), Some(Known::Unreachable), "{f:?}");
}

#[test]
fn latest_blocks_that_do_not_read_are_the_chains_shape() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    // A block without its time, and an answer that is not a block.
    let f = head_time(&eps(&[node("{\"number\":\"0x20\"}"), node("\"0x20\"")]), 31337).expect_err("no time reads");
    assert_eq!(f.which(), Some(Known::ChainShape), "{f:?}");
    let f = head_time(&eps(&[node("null"), node("null")]), 31337).expect_err("no time reads");
    assert_eq!(f.which(), Some(Known::Unreachable), "{f:?}");
}
