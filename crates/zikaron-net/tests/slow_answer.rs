//! A node that takes its time: real nodes often answer after 300 ms to 2 s, longer than one read slice
//! (`SLICE`). An answer that comes several slices after the question, or arrives in two parts with several
//! slices between them, is read whole and byte for byte as sent, over plain HTTP, over TLS and through a proxy
//! tunnel; slicing the reads only bounds how long each wait sleeps. Its own test binary: it trusts a test root
//! (`drive_trust_root`, once per process) and sets the proxy choice, both process-wide.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::time::Duration;
use zikaron_net::{parse, post_json, set_choice, Ask, Choice, Kind, Limits, Proxy, SLICE};

/// How long the node waits before answering: three slices and a little more.
fn pause() -> Duration {
    SLICE * 3 + Duration::from_millis(50)
}

/// A deadline far beyond the pause, so only reading can fail.
fn limits() -> Limits {
    Limits { deadline: Duration::from_secs(20), max_answer: 0 }
}

/// The answer the node gives: a body long enough to span several reads.
fn body() -> Vec<u8> {
    (0..40_000u32).map(|i| b"0123456789abcdef"[(i % 16) as usize]).collect()
}

/// The node's whole answer, head and body.
fn answer() -> (Vec<u8>, Vec<u8>) {
    let b = body();
    (format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", b.len()).into_bytes(), b)
}

/// Read one request head (the requests here have small bodies sent with it).
fn take_request(s: &mut impl Read) {
    let mut seen = Vec::new();
    let mut buf = [0u8; 4096];
    while !seen.windows(4).any(|w| w == b"\r\n\r\n") {
        match s.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => seen.extend_from_slice(&buf[..n]),
        }
    }
}

/// Answer slowly on `s`: wait, then the head; when `split`, wait again before the body.
fn answer_slowly(s: &mut impl Write, split: bool) {
    let (head, body) = answer();
    std::thread::sleep(pause());
    let _ = s.write_all(&head);
    let _ = s.flush();
    if split {
        std::thread::sleep(pause());
    }
    let _ = s.write_all(&body);
    let _ = s.flush();
}

/// A plain node answering every connection slowly.
fn slow_node(split: bool) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || {
                let mut s = s;
                take_request(&mut s);
                answer_slowly(&mut s, split);
            });
        }
    });
    port
}

/// A TLS node (certificate for `localhost` under the test root) answering every connection slowly.
fn slow_tls_node(split: bool) -> u16 {
    let cert = rustls::pki_types::CertificateDer::from(include_bytes!("fixtures/tls/localhost.crt.der").to_vec());
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(include_bytes!("fixtures/tls/localhost.key.der").to_vec().into());
    let config = Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("versions")
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .expect("server config"),
    );
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let config = config.clone();
            std::thread::spawn(move || {
                let Ok(conn) = rustls::ServerConnection::new(config) else { return };
                let mut tls = rustls::StreamOwned::new(conn, s);
                take_request(&mut tls);
                answer_slowly(&mut tls, split);
                tls.conn.send_close_notify();
                let _ = tls.flush();
            });
        }
    });
    port
}

/// An HTTP CONNECT proxy that pipes every tunnel to local port `to`.
fn connect_proxy(to: u16) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || {
                let mut s = s;
                let mut head = Vec::new();
                let mut one = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    if s.read(&mut one).unwrap_or(0) == 0 {
                        return;
                    }
                    head.push(one[0]);
                }
                let _ = s.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
                let Ok(up) = TcpStream::connect(("127.0.0.1", to)) else { return };
                let (mut s2, mut up2) = (s.try_clone().expect("clone"), up.try_clone().expect("clone"));
                std::thread::spawn(move || {
                    let _ = std::io::copy(&mut s2, &mut up2);
                });
                let mut up = up;
                let _ = std::io::copy(&mut up, &mut s);
                let _ = s.shutdown(std::net::Shutdown::Write);
            });
        }
    });
    port
}

/// Ask `url` once and return the answer's body.
fn ask(url: &str) -> Vec<u8> {
    let t = parse(url).expect("address");
    match post_json(&t, b"{}", &limits(), Ask::Once) {
        Ok(a) => {
            assert_eq!(a.status, 200, "{url}");
            a.body
        }
        Err(f) => panic!("{url}: a slow answer within the deadline is read, not {f:?}"),
    }
}

#[test]
fn an_answer_slower_than_a_slice_is_read_whole() {
    assert!(zikaron_net::drive_trust_root(include_bytes!("fixtures/tls/ca.crt.der")), "the test root is added before any handshake");
    set_choice(Choice::Off);
    for split in [false, true] {
        let form = if split { "head, then the body several slices later" } else { "the whole answer several slices later" };
        assert_eq!(ask(&format!("http://127.0.0.1:{}/", slow_node(split))), body(), "plain HTTP, {form}");
        assert_eq!(ask(&format!("https://localhost:{}/", slow_tls_node(split))), body(), "TLS, {form}");
        set_choice(Choice::Manual(Proxy { kind: Kind::Http, host: "127.0.0.1".into(), port: connect_proxy(slow_node(split)) }));
        // A name that is not this machine, so the proxy is taken (this machine is always reached straight).
        assert_eq!(ask("http://node.invalid:8545/"), body(), "through a proxy tunnel, {form}");
        set_choice(Choice::Off);
    }
}
