use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use zikaron_net::{exchange, get, parse, post_json, Ask, Fail, Limits};

const SLACK: Duration = Duration::from_millis(1500);

fn limits(deadline_ms: u64, max_answer: usize) -> Limits {
    Limits { deadline: Duration::from_millis(deadline_ms), max_answer }
}

fn server(serve: impl FnOnce(TcpStream) + Send + 'static) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        if let Ok((s, _)) = l.accept() {
            serve(s);
        }
    });
    port
}

fn servers(serve: impl Fn(usize, TcpStream) + Send + Sync + 'static) -> (u16, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let count = Arc::new(AtomicUsize::new(0));
    let (c, serve) = (count.clone(), Arc::new(serve));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let n = c.fetch_add(1, Ordering::SeqCst);
            let serve = serve.clone();
            std::thread::spawn(move || serve(n, s));
        }
    });
    (port, count)
}

fn take_request(s: &mut TcpStream) {
    let mut buf = [0u8; 4096];
    let _ = s.read(&mut buf);
}

fn read_request(s: &mut TcpStream) -> Option<Vec<u8>> {
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&raw[..i]).to_ascii_lowercase();
            let len: usize = head.lines().find_map(|l| l.strip_prefix("content-length:").and_then(|n| n.trim().parse().ok())).unwrap_or(0);
            if raw.len() >= i + 4 + len {
                return Some(raw);
            }
        }
        match s.read(&mut buf) {
            Ok(0) | Err(_) => return None,
            Ok(n) => raw.extend_from_slice(&buf[..n]),
        }
    }
}

const REQUEST: &[u8] = b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n";

fn ok_answer(body: &str, close: bool) -> Vec<u8> {
    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n{}\r\n{body}", body.len(), if close { "Connection: close\r\n" } else { "" }).into_bytes()
}

#[test]
fn an_answer_past_the_cap_is_refused_at_the_cap() {
    let port = server(|mut s| {
        take_request(&mut s);
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5000\r\n\r\n");
        let _ = s.write_all(&[b'x'; 2000]);
        std::thread::sleep(Duration::from_secs(2));
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    assert_eq!(exchange(&t, REQUEST, &limits(10_000, 1000)).err(), Some(Fail::Overlong(1000)));
}

#[test]
fn the_cap_counts_the_head_with_the_body() {
    let whole = ok_answer("0123456789", true);
    let cap = whole.len();
    for (max, refused) in [(cap, false), (cap - 1, true)] {
        let w = whole.clone();
        let port = server(move |mut s| {
            take_request(&mut s);
            let _ = s.write_all(&w);
        });
        let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
        let got = exchange(&t, REQUEST, &limits(5000, max));
        assert_eq!(got.err() == Some(Fail::Overlong(max)), refused, "cap {max} for {cap} bytes");
    }
}

#[test]
fn a_trickling_peer_is_cut_at_the_total_deadline() {
    let port = server(|mut s| {
        take_request(&mut s);
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100000\r\n\r\n");
        for _ in 0..200 {
            if s.write_all(b"x").is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    let began = Instant::now();
    assert_eq!(exchange(&t, REQUEST, &limits(600, 0)).err(), Some(Fail::Late(Duration::from_millis(600))));
    let took = began.elapsed();
    assert!(took >= Duration::from_millis(600) && took < Duration::from_millis(600) + SLACK, "took {took:?}");
}

#[test]
fn a_silent_peer_is_cut_at_the_total_deadline() {
    let port = server(|mut s| {
        take_request(&mut s);
        std::thread::sleep(Duration::from_secs(4));
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    let began = Instant::now();
    assert_eq!(exchange(&t, REQUEST, &limits(600, 0)).err(), Some(Fail::Late(Duration::from_millis(600))));
    let took = began.elapsed();
    assert!(took >= Duration::from_millis(600) && took < Duration::from_millis(600) + SLACK, "took {took:?}");
}

#[test]
fn a_handshake_that_never_ends_is_the_deadline() {
    let port = server(|mut s| {
        take_request(&mut s);
        std::thread::sleep(Duration::from_secs(4));
    });
    let t = parse(&format!("https://127.0.0.1:{port}/")).expect("address");
    let began = Instant::now();
    assert_eq!(exchange(&t, REQUEST, &limits(700, 0)).err(), Some(Fail::Late(Duration::from_millis(700))));
    let took = began.elapsed();
    assert!(took >= Duration::from_millis(700) && took < Duration::from_millis(700) + SLACK, "took {took:?}");
}

#[test]
fn writing_and_reading_share_one_deadline() {
    let port = server(|mut s| {
        std::thread::sleep(Duration::from_millis(400));
        take_request(&mut s);
        std::thread::sleep(Duration::from_millis(400));
        let _ = s.write_all(&ok_answer("late", true));
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    let began = Instant::now();
    assert_eq!(exchange(&t, REQUEST, &limits(600, 0)).err(), Some(Fail::Late(Duration::from_millis(600))));
    assert!(began.elapsed() < Duration::from_millis(600) + SLACK);
}

#[test]
fn a_refused_connection_is_named_as_the_connection() {
    let port = TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port();
    let at: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().expect("an address");
    match TcpStream::connect_timeout(&at, Duration::from_millis(500)) {
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {}
        other => {
            eprintln!("skipped: this machine does not refuse a closed local port within 500 ms ({:?})", other.map(|_| "connected").map_err(|e| e.kind()));
            return;
        }
    }
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    match exchange(&t, REQUEST, &limits(2000, 0)) {
        Err(Fail::Connect(said)) => assert!(said.contains(&format!("127.0.0.1:{port}")), "{said}"),
        other => panic!("a refused connection is a connection failure, not {:?}", other.err()),
    }
}

#[test]
fn a_name_that_does_not_resolve_is_named_as_the_name() {
    zikaron_net::set_choice(zikaron_net::Choice::Off);
    let t = parse("http://no-such-node.invalid/").expect("address");
    assert!(matches!(exchange(&t, REQUEST, &limits(5000, 0)), Err(Fail::Name(_))));
}

fn self_signed_server() -> u16 {
    let cert = rustls::pki_types::CertificateDer::from(include_bytes!("fixtures/self-signed-cert.der").to_vec());
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(include_bytes!("fixtures/self-signed-key.pk8.der").to_vec().into());
    let config = rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .expect("versions")
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .expect("server config");
    let config = std::sync::Arc::new(config);
    server(move |mut s| {
        if let Ok(mut conn) = rustls::ServerConnection::new(config) {
            for _ in 0..20 {
                if conn.complete_io(&mut s).is_err() || !conn.is_handshaking() {
                    break;
                }
            }
        }
    })
}

#[test]
fn an_untrusted_certificate_is_named_as_the_certificate() {
    let port = self_signed_server();
    let t = parse(&format!("https://127.0.0.1:{port}/")).expect("address");
    match exchange(&t, REQUEST, &limits(5000, 0)) {
        Err(Fail::Certificate(_)) => {}
        other => panic!("an untrusted certificate is a certificate failure, not {other:?}"),
    }
    let port = self_signed_server();
    let t = parse(&format!("https://127.0.0.1:{port}/")).expect("address");
    match post_json(&t, b"{}", &limits(5000, 0), Ask::Read) {
        Err(Fail::Certificate(_)) => {}
        other => panic!("a read with an untrusted certificate stays a certificate failure, not {:?}", other.err()),
    }
}

#[test]
fn a_whole_answer_comes_back_as_sent() {
    let port = server(|mut s| {
        take_request(&mut s);
        let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 24\r\n\r\n{\"jsonrpc\":\"2.0\",\"id\":1}");
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    let a = post_json(&t, b"{}", &limits(5000, 0), Ask::Read).expect("an answer");
    assert_eq!(a.status, 200);
    assert_eq!(a.body, b"{\"jsonrpc\":\"2.0\",\"id\":1}".to_vec());
}

fn v6_loopback() -> Option<TcpListener> {
    TcpListener::bind("[::1]:0").ok()
}

fn localhost_is_dual() -> bool {
    let addrs: Vec<std::net::SocketAddr> = std::net::ToSocketAddrs::to_socket_addrs(&("localhost", 1)).map(|a| a.collect()).unwrap_or_default();
    addrs.iter().any(|a| a.is_ipv4()) && addrs.iter().any(|a| a.is_ipv6())
}

#[test]
fn every_resolved_address_is_tried() {
    if v6_loopback().is_none() || !localhost_is_dual() {
        eprintln!("skipped: no IPv6 loopback, or `localhost` does not name both loopbacks, on this machine");
        return;
    }
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = l.accept() {
            take_request(&mut s);
            let _ = s.write_all(&ok_answer("v4", true));
        }
    });
    let t = parse(&format!("http://localhost:{port}/")).expect("address");
    assert_eq!(exchange(&t, REQUEST, &limits(5000, 0)).map(|r| r.ends_with(b"v4")), Ok(true));
    let l = TcpListener::bind("[::1]:0").expect("bind v6");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = l.accept() {
            take_request(&mut s);
            let _ = s.write_all(&ok_answer("v6", true));
        }
    });
    let t = parse(&format!("http://localhost:{port}/")).expect("address");
    assert_eq!(exchange(&t, REQUEST, &limits(5000, 0)).map(|r| r.ends_with(b"v6")), Ok(true));
    let port = TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port();
    match exchange(&parse(&format!("http://localhost:{port}/")).expect("address"), REQUEST, &limits(5000, 0)) {
        Err(Fail::Connect(said)) => {
            assert!(said.contains(&format!("127.0.0.1:{port}")) && said.contains(&format!("[::1]:{port}")), "{said}")
        }
        other => panic!("{:?}", other.err()),
    }
}

#[test]
fn an_ipv6_literal_is_reached() {
    let Some(l) = v6_loopback() else {
        eprintln!("skipped: no IPv6 loopback on this machine");
        return;
    };
    let port = l.local_addr().expect("addr").port();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let Ok((mut s, _)) = l.accept() {
            let _ = tx.send(read_request(&mut s).unwrap_or_default());
            let _ = s.write_all(&ok_answer("{}", true));
        }
    });
    let t = parse(&format!("http://[::1]:{port}/rpc")).expect("address");
    assert_eq!(post_json(&t, b"{}", &limits(5000, 0), Ask::Read).map(|a| a.status).ok(), Some(200));
    let request = String::from_utf8_lossy(&rx.recv().expect("asked")).to_string();
    assert!(request.starts_with("POST /rpc HTTP/1.1\r\n") && request.contains(&format!("\r\nHost: [::1]:{port}\r\n")), "{request}");
}

#[test]
fn a_connection_left_open_carries_the_next_question() {
    let (port, conns) = servers(|_, mut s| {
        while read_request(&mut s).is_some() {
            if s.write_all(&ok_answer("kept", false)).is_err() {
                return;
            }
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    for _ in 0..3 {
        assert_eq!(post_json(&t, b"{}", &limits(5000, 0), Ask::Read).map(|a| a.body).ok(), Some(b"kept".to_vec()));
    }
    assert_eq!(conns.load(Ordering::SeqCst), 1, "three reads, one connection");
    let (port, conns) = servers(|_, mut s| {
        if read_request(&mut s).is_some() {
            let _ = s.write_all(&ok_answer("closed", true));
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    for _ in 0..3 {
        assert_eq!(post_json(&t, b"{}", &limits(5000, 0), Ask::Read).map(|a| a.body).ok(), Some(b"closed".to_vec()));
    }
    assert_eq!(conns.load(Ordering::SeqCst), 3, "a peer that says close is let go each time");
}

#[test]
fn a_question_asked_once_opens_its_own_connection() {
    let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
    let tx = std::sync::Mutex::new(tx);
    let (port, conns) = servers(move |_, mut s| {
        while let Some(r) = read_request(&mut s) {
            let _ = tx.lock().map(|t| t.send(r));
            if s.write_all(&ok_answer("ok", false)).is_err() {
                return;
            }
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    assert!(post_json(&t, b"{}", &limits(5000, 0), Ask::Read).is_ok());
    assert!(post_json(&t, b"{}", &limits(5000, 0), Ask::Once).is_ok());
    assert_eq!(conns.load(Ordering::SeqCst), 2, "the broadcast did not take the kept connection");
    let _read = rx.recv().expect("read asked");
    let once = String::from_utf8_lossy(&rx.recv().expect("once asked")).to_ascii_lowercase();
    assert!(once.contains("\r\nconnection: close\r\n"), "{once}");
    assert!(post_json(&t, b"{}", &limits(5000, 0), Ask::Read).is_ok());
    assert_eq!(conns.load(Ordering::SeqCst), 2);
}

#[test]
fn a_kept_connection_closed_by_the_peer_is_replaced() {
    let (port, conns) = servers(|n, mut s| {
        if read_request(&mut s).is_some() {
            let _ = s.write_all(&ok_answer(&format!("conn {n}"), false));
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    assert_eq!(post_json(&t, b"{}", &limits(5000, 0), Ask::Read).map(|a| a.body).ok(), Some(b"conn 0".to_vec()));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(post_json(&t, b"{}", &limits(5000, 0), Ask::Read).map(|a| a.body).ok(), Some(b"conn 1".to_vec()));
    assert_eq!(conns.load(Ordering::SeqCst), 2);
}

#[test]
fn a_kept_connection_the_peer_reset_is_replaced() {
    let (port, conns) = servers(|n, mut s| {
        let mut head = [0u8; 16];
        if s.read_exact(&mut head).is_ok() {
            let _ = s.write_all(&ok_answer(&format!("conn {n}"), false));
            std::thread::sleep(Duration::from_millis(50));
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    assert_eq!(post_json(&t, b"{}", &limits(5000, 0), Ask::Read).map(|a| a.body).ok(), Some(b"conn 0".to_vec()));
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(post_json(&t, b"{}", &limits(5000, 0), Ask::Read).map(|a| a.body).ok(), Some(b"conn 1".to_vec()));
    assert_eq!(conns.load(Ordering::SeqCst), 2);
}

#[test]
fn a_read_broken_before_the_answer_is_asked_once_more() {
    let asked = Arc::new(AtomicUsize::new(0));
    let a = asked.clone();
    let (port, _) = servers(move |n, mut s| {
        if read_request(&mut s).is_some() {
            a.fetch_add(1, Ordering::SeqCst);
            if n > 0 {
                let _ = s.write_all(&ok_answer("second", true));
            }
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    assert_eq!(post_json(&t, b"{}", &limits(5000, 0), Ask::Read).map(|a| a.body).ok(), Some(b"second".to_vec()));
    assert_eq!(asked.load(Ordering::SeqCst), 2);

    let asked = Arc::new(AtomicUsize::new(0));
    let a = asked.clone();
    let (port, _) = servers(move |_, mut s| {
        if read_request(&mut s).is_some() {
            a.fetch_add(1, Ordering::SeqCst);
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    assert!(matches!(post_json(&t, b"{}", &limits(5000, 0), Ask::Read), Err(Fail::Stream(_))));
    assert_eq!(asked.load(Ordering::SeqCst), 2, "asked once more, not more");
    assert!(matches!(post_json(&t, b"{}", &limits(5000, 0), Ask::Once), Err(Fail::Stream(_))));
    assert_eq!(asked.load(Ordering::SeqCst), 3, "a question asked once is asked once");
}

#[test]
fn a_read_cut_after_the_answer_began_is_not_asked_again() {
    const PART: &[u8] = b"HTTP/1.1 200 OK\r\nContent-Length: 50\r\n\r\nabc";
    let (port, conns) = servers(|_, mut s| {
        if read_request(&mut s).is_some() {
            let _ = s.write_all(PART);
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    assert_eq!(post_json(&t, b"{}", &limits(5000, 0), Ask::Read).err(), Some(Fail::Stream("the body is shorter than its length".into())));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(conns.load(Ordering::SeqCst), 1, "closed mid-answer: asked once");

    let (port, conns) = servers(|_, mut s| {
        let mut head = [0u8; 16];
        if s.read_exact(&mut head).is_ok() {
            let _ = s.write_all(PART);
            std::thread::sleep(Duration::from_millis(1000));
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    match post_json(&t, b"{}", &limits(5000, 0), Ask::Read) {
        Err(Fail::Stream(_)) => {}
        other => panic!("a reset mid-answer is a stream failure, not {:?}", other.map(|a| a.body)),
    }
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(conns.load(Ordering::SeqCst), 1, "reset mid-answer: asked once");
}

#[test]
fn a_late_read_is_not_asked_again() {
    let asked = Arc::new(AtomicUsize::new(0));
    let a = asked.clone();
    let (port, _) = servers(move |_, mut s| {
        if read_request(&mut s).is_some() {
            a.fetch_add(1, Ordering::SeqCst);
            std::thread::sleep(Duration::from_secs(2));
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    assert_eq!(post_json(&t, b"{}", &limits(400, 0), Ask::Read).err(), Some(Fail::Late(Duration::from_millis(400))));
    assert_eq!(asked.load(Ordering::SeqCst), 1);
}

#[test]
fn an_interim_answer_is_passed_over() {
    let port = server(|mut s| {
        take_request(&mut s);
        let _ = s.write_all(b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 200 OK\r\nTRANSFER-ENCODING:chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n");
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    let a = post_json(&t, b"{}", &limits(5000, 0), Ask::Read).expect("an answer");
    assert_eq!((a.status, a.body), (200, b"{}".to_vec()));
}

#[test]
fn a_get_hands_up_the_status_and_where_it_points() {
    let port = server(|mut s| {
        take_request(&mut s);
        let _ = s.write_all(b"HTTP/1.1 301 Moved\r\nlocation:  /next \r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    });
    let a = get(&parse(&format!("http://127.0.0.1:{port}/k")).expect("address"), &limits(5000, 0)).expect("an answer");
    assert_eq!((a.status, a.location.as_deref()), (301, Some("/next")));
    let port = server(|mut s| {
        take_request(&mut s);
        let _ = s.write_all(b"HTTP/1.1 200\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    });
    let a = get(&parse(&format!("http://127.0.0.1:{port}/")).expect("address"), &limits(5000, 0)).expect("an answer");
    assert_eq!((a.status, a.body), (200, b"ok".to_vec()));
    let port = server(|mut s| {
        take_request(&mut s);
        let _ = s.write_all(b"HTTP/1.1 abc\r\n\r\n");
    });
    assert!(matches!(get(&parse(&format!("http://127.0.0.1:{port}/")).expect("address"), &limits(5000, 0)), Err(Fail::Stream(_))));
}
