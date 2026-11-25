use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use zikaron_net::{parse, post_json, set_choice, Ask, Choice, Fail, Limits};

fn trusted() {
    static ROOT: OnceLock<bool> = OnceLock::new();
    assert!(*ROOT.get_or_init(|| zikaron_net::drive_trust_root(include_bytes!("fixtures/tls/ca.crt.der"))), "the test root is added before any handshake");
    set_choice(Choice::Off);
    zikaron_net::set_dial_again(Some(Duration::from_millis(100)));
}

fn server_config() -> Arc<rustls::ServerConfig> {
    let cert = rustls::pki_types::CertificateDer::from(include_bytes!("fixtures/tls/localhost.crt.der").to_vec());
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(include_bytes!("fixtures/tls/localhost.key.der").to_vec().into());
    Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("versions")
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .expect("server config"),
    )
}

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

const BODY: &[u8] = br#"{"jsonrpc":"2.0","id":1,"result":"0x1"}"#;

fn flaky_tls(fail: usize) -> (u16, Arc<AtomicUsize>) {
    let config = server_config();
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let seen = Arc::new(AtomicUsize::new(0));
    let count = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let n = count.fetch_add(1, Ordering::SeqCst);
            if n < fail {
                drop(s);
                continue;
            }
            let config = config.clone();
            std::thread::spawn(move || {
                let Ok(conn) = rustls::ServerConnection::new(config) else { return };
                let mut tls = rustls::StreamOwned::new(conn, s);
                take_request(&mut tls);
                let _ = tls.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", BODY.len()).as_bytes());
                let _ = tls.write_all(BODY);
                tls.conn.send_close_notify();
                let _ = tls.flush();
            });
        }
    });
    (port, seen)
}

fn ask(port: u16, deadline: Duration) -> Result<Vec<u8>, Fail> {
    let t = parse(&format!("https://localhost:{port}/")).expect("a target");
    post_json(&t, b"{}", &Limits { deadline, max_answer: 0 }, Ask::Read).map(|a| a.body)
}

#[test]
fn a_handshake_that_fails_once_is_dialled_once_more_and_answered() {
    trusted();
    let (port, seen) = flaky_tls(1);
    assert_eq!(ask(port, Duration::from_secs(10)).expect("answered on the second dial"), BODY);
    assert_eq!(seen.load(Ordering::SeqCst), 2, "two connections: the failed one and the one that answered");
    let (port, seen) = flaky_tls(usize::MAX);
    assert!(matches!(ask(port, Duration::from_secs(10)), Err(Fail::Handshake(_))), "still failing: the handshake's failure");
    assert_eq!(seen.load(Ordering::SeqCst), 2, "dialled twice, no more");
    let (port, seen) = flaky_tls(1);
    let t = parse(&format!("https://localhost:{port}/")).expect("a target");
    assert!(matches!(post_json(&t, b"{}", &Limits { deadline: Duration::from_secs(10), max_answer: 0 }, Ask::Once), Err(Fail::Handshake(_))), "a broadcast is not dialled again");
    assert_eq!(seen.load(Ordering::SeqCst), 1);
}

#[test]
fn a_connect_refused_once_is_dialled_once_more_and_a_deadline_spent_is_not() {
    trusted();
    let port = TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port();
    let later = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(30));
        let l = TcpListener::bind(("127.0.0.1", port)).expect("the port again");
        if let Some(Ok(mut s)) = l.incoming().next() {
            take_request(&mut s);
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", BODY.len()).as_bytes());
            let _ = s.write_all(BODY);
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("a target");
    let got = post_json(&t, b"{}", &Limits { deadline: Duration::from_secs(10), max_answer: 0 }, Ask::Read);
    assert_eq!(got.expect("answered on the second dial").body, BODY);
    let _ = later.join();
    if !cfg!(windows) {
        let dead = TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port();
        let t = parse(&format!("http://127.0.0.1:{dead}/")).expect("a target");
        let started = Instant::now();
        let got = post_json(&t, b"{}", &Limits { deadline: Duration::from_millis(20), max_answer: 0 }, Ask::Read);
        assert!(matches!(got, Err(Fail::Connect(_))), "named as the connect it was, not as the deadline: {:?}", got.err());
        assert!(started.elapsed() < Duration::from_millis(100), "no beat waited when the deadline leaves none past it");
    }
}
