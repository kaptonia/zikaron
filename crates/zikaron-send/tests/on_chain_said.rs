use std::io::{Read, Write};
use zikaron_send::node::{self, Endpoint, NodeAddr};
use zikaron_send::Code;

fn plain_text_port() -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = l.local_addr().expect("its address").port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let mut b = [0u8; 4096];
            let _ = s.read(&mut b);
            let _ = s.write_all(b"HTTP/1.1 400 Bad Request\r\ncontent-type: text/plain\r\ncontent-length: 11\r\nconnection: close\r\n\r\nBad Request");
        }
    });
    port
}

fn refusing_node(error: &'static str) -> u16 {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = l.local_addr().expect("its address").port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let mut got: Vec<u8> = Vec::new();
            let mut b = [0u8; 4096];
            while let Ok(n) = s.read(&mut b) {
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&b[..n]);
                let text = String::from_utf8_lossy(&got).to_string();
                if let Some(head_end) = text.find("\r\n\r\n") {
                    let length = text[..head_end].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                    if got.len() >= head_end + 4 + length {
                        break;
                    }
                }
            }
            let body = format!("{{\"error\":{error},\"id\":null,\"jsonrpc\":\"2.0\"}}");
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len()).as_bytes());
        }
    });
    port
}

fn ask(url: &NodeAddr) -> zikaron_send::Trouble {
    zikaron_send::batch::on_chain(std::slice::from_ref(url), 1, &[0x11; 20], &[0x22; 20], &[[0xaa; 32]], 0, &[]).expect_err("no node can say")
}

#[test]
fn the_on_chain_question_names_a_handshake_as_the_handshake() {
    let url = NodeAddr::new(format!("https://127.0.0.1:{}", plain_text_port()));
    let t = ask(&url);
    assert_eq!(t.code, Code::NodeTls, "{t:?}");
    let head = node::head_block(&[Endpoint::at(1, url)], 1).expect_err("no head either");
    assert_eq!(t.code, head.code, "the same name every read of the chain gives it");
}

#[test]
fn a_node_that_refuses_the_question_cannot_say() {
    let error = "{\"code\":-32000,\"message\":\"logs unavailable\"}";
    let url = NodeAddr::new(format!("http://127.0.0.1:{}", refusing_node(error)));
    let t = ask(&url);
    assert_eq!(t.code, Code::Unreachable, "reached, and it could not say: no answer, nothing sent on a guess: {t:?}");
    assert!(Code::NETWORK.contains(&t.code), "still no answer, nothing sent on a guess: {t:?}");
}
