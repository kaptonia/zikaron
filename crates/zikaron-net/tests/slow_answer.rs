//! A node that takes its time: real nodes often answer after 300 ms to 2 s, longer than one read slice
//! (`SLICE`). An answer that comes several slices after the question, or arrives in two parts with several
//! slices between them, is read whole and byte for byte as sent, over plain HTTP, over TLS and through a proxy
//! tunnel; slicing the reads only bounds how long each wait sleeps.
//!
//! A peer that trickles, a byte at a time and never a slice apart, is cut at the deadline wherever it trickles:
//! a TLS answer's record, a TLS handshake flight, an HTTP CONNECT proxy's answer, a SOCKS5 proxy's reply. A
//! SOCKS5 tunnel cut while the answer comes through it is a named stream failure, at once. The deadline is
//! given directly (a few slices), so no test here waits on the wall clock past it.
//!
//! Its own test binary: it trusts a test root (`drive_trust_root`, once per process) and sets the proxy choice,
//! both process-wide, so its tests take turns.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};
use zikaron_net::{parse, post_json, set_choice, Ask, Choice, Fail, Kind, Limits, Proxy, SLICE};

/// One test at a time (the proxy choice is process-wide), the test root trusted before the first handshake.
fn turn() -> MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    static ROOT: OnceLock<bool> = OnceLock::new();
    let held = TURN.lock().unwrap_or_else(|e| e.into_inner());
    assert!(*ROOT.get_or_init(|| zikaron_net::drive_trust_root(include_bytes!("fixtures/tls/ca.crt.der"))), "the test root is added before any handshake");
    set_choice(Choice::Off);
    held
}

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

/// The TLS server configuration: a certificate for `localhost` under the test root.
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

/// A TLS node answering every connection slowly.
fn slow_tls_node(split: bool) -> u16 {
    let config = server_config();
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
    let _turn = turn();
    for split in [false, true] {
        let form = if split { "head, then the body several slices later" } else { "the whole answer several slices later" };
        assert_eq!(ask(&format!("http://127.0.0.1:{}/", slow_node(split))), body(), "plain HTTP, {form}");
        assert_eq!(ask(&format!("https://localhost:{}/", slow_tls_node(split))), body(), "TLS, {form}");
        set_choice(Choice::Manual(Proxy { kind: Kind::Http, host: "127.0.0.1".into(), port: connect_proxy(slow_node(split)), auth: None }));
        // A name that is not this machine, so the proxy is taken (this machine is always reached straight).
        assert_eq!(ask("http://node.invalid:8545/"), body(), "through a proxy tunnel, {form}");
        set_choice(Choice::Off);
    }
}

// ───────────────────────── Trickling peers ─────────────────────────

/// The deadline the trickle tests give: two slices.
fn short() -> Limits {
    Limits { deadline: SLICE * 2, max_answer: 0 }
}

/// The gap between two trickled bytes: well inside a slice, so no read ever waits a whole slice.
const DRIP: Duration = Duration::from_millis(10);

/// Write `bytes` one at a time, [`DRIP`] apart, until done or the client goes.
fn drip(s: &mut TcpStream, bytes: &[u8]) {
    for b in bytes {
        if s.write_all(&[*b]).and_then(|_| s.flush()).is_err() {
            return;
        }
        std::thread::sleep(DRIP);
    }
}

/// What TLS has queued to send, taken out as bytes.
fn queued(conn: &mut rustls::ServerConnection) -> Vec<u8> {
    let mut out = Vec::new();
    while conn.wants_write() {
        if conn.write_tls(&mut out).is_err() {
            break;
        }
    }
    out
}

/// A local listener serving every connection with `serve`.
fn serving(serve: impl Fn(TcpStream) + Send + Sync + 'static) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let serve = Arc::new(serve);
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let serve = serve.clone();
            std::thread::spawn(move || serve(s));
        }
    });
    port
}

/// A TLS node that trickles: with `in_handshake`, its own handshake flight after the client's hello; otherwise,
/// after a whole handshake and the request, its answer as one record. Either way it would take seconds.
fn trickling_tls_node(in_handshake: bool) -> u16 {
    let config = server_config();
    serving(move |mut s| {
        let Ok(mut conn) = rustls::ServerConnection::new(config.clone()) else { return };
        if in_handshake {
            while !conn.wants_write() {
                match conn.read_tls(&mut s) {
                    Ok(0) | Err(_) => return,
                    Ok(_) if conn.process_new_packets().is_err() => return,
                    Ok(_) => {}
                }
            }
            let flight = queued(&mut conn);
            drip(&mut s, &flight);
            return;
        }
        let mut tls = rustls::StreamOwned::new(conn, s);
        take_request(&mut tls);
        let rustls::StreamOwned { mut conn, mut sock } = tls;
        // The whole answer in one write, so it is one record: nothing of it decrypts until its last byte.
        let body = [b'x'; 300];
        let mut whole = format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).into_bytes();
        whole.extend_from_slice(&body);
        if conn.writer().write_all(&whole).is_err() {
            return;
        }
        let records = queued(&mut conn);
        drip(&mut sock, &records);
    })
}

/// Ask `url` once under the short deadline; the result and how long it took.
fn ask_short(url: &str) -> (Result<u16, Fail>, Duration) {
    let t = parse(url).expect("address");
    let began = Instant::now();
    let got = post_json(&t, b"{}", &short(), Ask::Once).map(|a| a.status);
    (got, began.elapsed())
}

/// Ended by the deadline, and soon after it: within a slice, with room for a loaded machine; the peer's
/// trickle would have run many seconds longer.
fn late_at_the_deadline(form: &str, (got, took): (Result<u16, Fail>, Duration)) {
    assert_eq!(got, Err(Fail::Late(short().deadline)), "{form}: ended by the deadline");
    assert!(took < short().deadline + SLICE + Duration::from_secs(1), "{form}: at the deadline, not when the peer stops ({took:?})");
}

/// A TLS answer trickled a byte at a time: each socket read is one step, so the deadline cuts it mid-record.
#[test]
fn a_tls_answer_trickled_is_cut_at_the_deadline() {
    let _turn = turn();
    late_at_the_deadline("TLS answer trickled", ask_short(&format!("https://localhost:{}/", trickling_tls_node(false))));
}

/// A TLS handshake flight trickled a byte at a time: the handshake takes one socket step per turn, so the
/// deadline cuts it mid-flight, as a deadline (not a handshake failure).
#[test]
fn a_tls_handshake_trickled_is_cut_at_the_deadline() {
    let _turn = turn();
    late_at_the_deadline("TLS handshake trickled", ask_short(&format!("https://localhost:{}/", trickling_tls_node(true))));
}

/// Read a CONNECT request's head (to its blank line), byte by byte.
fn read_connect(s: &mut TcpStream) -> bool {
    let mut head = Vec::new();
    let mut one = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if s.read(&mut one).unwrap_or(0) == 0 {
            return false;
        }
        head.push(one[0]);
    }
    true
}

/// Read a SOCKS5 greeting (version, count, one method).
fn read_greeting(s: &mut TcpStream) -> bool {
    let mut g = [0u8; 3];
    s.read_exact(&mut g).is_ok()
}

/// Read a SOCKS5 CONNECT request by name.
fn read_request(s: &mut TcpStream) -> bool {
    let mut head = [0u8; 5];
    if s.read_exact(&mut head).is_err() {
        return false;
    }
    let mut rest = vec![0u8; head[4] as usize + 2];
    s.read_exact(&mut rest).is_ok()
}

/// A proxy that is slow to answer, trickling its answer a byte at a time: an HTTP CONNECT proxy's answer head,
/// a SOCKS5 proxy's reply (its longest answer: a bound name of 255 bytes). Each is cut at the deadline, as a
/// deadline. (A proxy silent before it answers is `proxy.rs`'s.)
#[test]
fn a_proxy_trickling_its_answer_is_cut_at_the_deadline() {
    let _turn = turn();
    let connect = serving(|mut s| {
        if read_connect(&mut s) {
            let mut head = b"HTTP/1.1 200 Connection established\r\nX-Pad: ".to_vec();
            head.extend_from_slice(&[b'x'; 4096]);
            head.extend_from_slice(b"\r\n\r\n");
            drip(&mut s, &head);
        }
    });
    let reply = serving(|mut s| {
        if read_greeting(&mut s) && s.write_all(&[5, 0]).is_ok() && read_request(&mut s) {
            let mut r = vec![5u8, 0, 0, 3, 255];
            r.extend_from_slice(&[b'p'; 255]);
            r.extend_from_slice(&[0, 80]);
            drip(&mut s, &r);
        }
    });
    for (form, kind, port) in [("CONNECT answer trickled", Kind::Http, connect), ("SOCKS5 reply trickled", Kind::Socks5, reply)] {
        set_choice(Choice::Manual(Proxy { kind, host: "127.0.0.1".into(), port, auth: None }));
        // A name that is not this machine, so the proxy is taken. The deadline passes on the proxy's leg: the
        // proxy's failure, named as such.
        let (got, took) = ask_short("http://node.invalid:8545/");
        assert!(matches!(&got, Err(Fail::Proxy(s)) if s.contains("no answer within")), "{form}: ended by the deadline on the proxy's leg: {got:?}");
        assert!(took < short().deadline + SLICE + Duration::from_secs(1), "{form}: at the deadline, not when the proxy stops ({took:?})");
    }
    set_choice(Choice::Off);
}

/// A SOCKS5 tunnel cut while the answer comes through it: the answer's head and part of its body arrive, then
/// the proxy closes both sides. The exchange ends at once with a stream failure (the answer is not whole),
/// never a success with half a body, never a wait for the deadline; a request sent once is not sent again.
#[test]
fn a_socks_tunnel_cut_mid_answer_is_a_stream_failure() {
    let _turn = turn();
    let node = serving(|mut s| {
        take_request(&mut s);
        let (head, body) = answer();
        let _ = s.write_all(&head);
        let _ = s.write_all(&body[..body.len() / 2]);
        let _ = s.flush();
        std::thread::sleep(Duration::from_secs(5));
    });
    let asked = Arc::new(Mutex::new(0usize));
    let counted = asked.clone();
    let proxy = serving(move |mut s| {
        if !(read_greeting(&mut s) && s.write_all(&[5, 0]).is_ok() && read_request(&mut s)) {
            return;
        }
        *counted.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        let Ok(mut up) = TcpStream::connect(("127.0.0.1", node)) else { return };
        if s.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 80]).is_err() {
            return;
        }
        // The request through, then the answer's first part back, then the tunnel cut.
        let mut buf = [0u8; 4096];
        let Ok(n) = s.read(&mut buf) else { return };
        if up.write_all(&buf[..n]).is_err() {
            return;
        }
        let (head, body) = answer();
        let mut part = vec![0u8; head.len() + body.len() / 2];
        if up.read_exact(&mut part).is_ok() {
            let _ = s.write_all(&part);
            let _ = s.flush();
        }
        let _ = s.shutdown(std::net::Shutdown::Both);
        let _ = up.shutdown(std::net::Shutdown::Both);
    });
    set_choice(Choice::Manual(Proxy { kind: Kind::Socks5, host: "127.0.0.1".into(), port: proxy, auth: None }));
    let t = parse("http://node.invalid:8545/").expect("address");
    let began = Instant::now();
    let got = post_json(&t, b"{}", &limits(), Ask::Once).map(|a| a.status);
    let took = began.elapsed();
    set_choice(Choice::Off);
    assert!(matches!(got, Err(Fail::Stream(_))), "a tunnel cut mid-answer is the stream's failure: {got:?}");
    assert!(took < limits().deadline / 2, "at once, not at the deadline ({took:?})");
    assert_eq!(*asked.lock().unwrap_or_else(|e| e.into_inner()), 1, "sent once, not again");
}

/// A peer that takes no writes: the connection (and, over TLS, the handshake) is made, then the peer reads
/// nothing, so a request larger than the socket buffers cannot be written whole. The write is bounded by the
/// time left, as every step is: the exchange ends at the deadline, as a deadline, over TLS and plain alike.
#[test]
fn a_peer_that_takes_no_writes_is_cut_at_the_deadline() {
    let _turn = turn();
    let config = server_config();
    let tls = serving(move |s| {
        let Ok(conn) = rustls::ServerConnection::new(config.clone()) else { return };
        let mut tls = rustls::StreamOwned::new(conn, s);
        while tls.conn.is_handshaking() {
            if tls.conn.complete_io(&mut tls.sock).is_err() {
                return;
            }
        }
        // Handshake done; read nothing more, hold the connection until the client goes.
        std::thread::sleep(Duration::from_secs(10));
    });
    let plain = serving(|s| {
        std::thread::sleep(Duration::from_secs(10));
        drop(s);
    });
    // Far larger than any socket buffer on a loopback connection.
    let body = vec![b'x'; 64 << 20];
    for (form, url) in [("TLS", format!("https://localhost:{tls}/")), ("plain", format!("http://127.0.0.1:{plain}/"))] {
        let t = parse(&url).expect("address");
        let began = Instant::now();
        let got = post_json(&t, &body, &short(), Ask::Once).map(|a| a.status);
        late_at_the_deadline(&format!("{form}: a peer that takes no writes"), (got, began.elapsed()));
    }
}
