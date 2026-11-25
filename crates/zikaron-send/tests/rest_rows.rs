use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use zikaron_send::node::{self, Endpoint, NodeAddr};

fn rpc_node() -> String {
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
                let id = body.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                let result = match method.as_str() {
                    "eth_chainId" => "\"0x7a69\"",
                    "eth_blockNumber" => "\"0x100\"",
                    "eth_getBalance" => "\"0x5\"",
                    _ => "null",
                };
                let body = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{result}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    url
}

fn closing_service() -> (u16, Arc<AtomicUsize>) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let n = Arc::new(AtomicUsize::new(0));
    let seen = n.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            seen.fetch_add(1, Ordering::SeqCst);
            drop(s);
        }
    });
    (port, n)
}

#[test]
fn a_rest_row_that_gave_nothing_is_asked_nothing_more_in_the_read() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let good = rpc_node();
    let (port, connections) = closing_service();
    let base = format!("https://127.0.0.1:{port}/api");
    zikaron_anchor::rest::hold_keys(vec![(format!("127.0.0.1:{port}"), "K".into())]);
    let eps = vec![Endpoint::at(31337, NodeAddr::new(good)), Endpoint::at(31337, NodeAddr::rest(&base, 31337))];
    let (wei, r) = node::balance(&eps, 31337, &[0x11; 20]).expect("a balance from the JSON-RPC node");
    assert_eq!(wei, 5);
    assert!(r.single_source && r.sources == 1, "{:?}", r.unanswered);
    let named = r.unanswered.iter().filter(|w| format!("{w:?}").contains(&format!("127.0.0.1:{port}"))).count();
    assert_eq!(named, 1, "{:?}", r.unanswered);
    assert!((1..=2).contains(&connections.load(Ordering::SeqCst)), "{}", connections.load(Ordering::SeqCst));
    let before = connections.load(Ordering::SeqCst);
    let _ = node::head_block(&eps, 31337).expect("the head");
    assert!(connections.load(Ordering::SeqCst) > before, "a new read asks every node again");
}
