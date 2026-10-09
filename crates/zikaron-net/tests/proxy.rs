//! Proxy routing against in-process HTTP CONNECT and SOCKS5 stubs (no network; a stub resolves the node's name
//! by piping to a local listener). The proxy setting is process-wide, so tests run serially (`TURN`).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zikaron_net::{parse, post_json, read_system_with, set_choice, Ask, Choice, Fail, Kind, Limits, Proxy, SystemProxies};

/// Serializes tests: the proxy setting is process-wide.
static TURN: Mutex<()> = Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

fn limits() -> Limits {
    Limits { deadline: Duration::from_secs(5), max_answer: 0 }
}

/// Copy both ways between two streams until either side closes.
fn pipe(a: TcpStream, b: TcpStream) {
    let (mut a2, mut b2) = (a.try_clone().expect("clone"), b.try_clone().expect("clone"));
    let (mut a, mut b) = (a, b);
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut a2, &mut b2);
        let _ = b2.shutdown(std::net::Shutdown::Write);
    });
    let _ = std::io::copy(&mut b, &mut a);
    let _ = a.shutdown(std::net::Shutdown::Write);
}

/// A node on a local port answering one JSON body to every request.
fn node() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || {
                let mut s = s;
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let body = "{\"through\":true}";
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    port
}

/// A TLS server with a self-signed certificate (no root trusts it).
fn untrusted_tls() -> u16 {
    let cert = rustls::pki_types::CertificateDer::from(include_bytes!("fixtures/self-signed-cert.der").to_vec());
    let key = rustls::pki_types::PrivateKeyDer::Pkcs8(include_bytes!("fixtures/self-signed-key.pk8.der").to_vec().into());
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
                let mut s = s;
                if let Ok(mut c) = rustls::ServerConnection::new(config) {
                    for _ in 0..20 {
                        if c.complete_io(&mut s).is_err() || !c.is_handshaking() {
                            break;
                        }
                    }
                }
            });
        }
    });
    port
}

/// An HTTP CONNECT proxy that routes every target to local port `to`, or refuses with `refuse`; records the
/// request lines it saw.
fn connect_proxy(to: u16, refuse: Option<&'static str>) -> (u16, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let seen = s2.clone();
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
                seen.lock().expect("seen").push(String::from_utf8_lossy(&head).lines().next().unwrap_or("").to_string());
                if let Some(line) = refuse {
                    let _ = s.write_all(format!("{line}\r\nContent-Length: 0\r\n\r\n").as_bytes());
                    return;
                }
                let _ = s.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
                if let Ok(up) = TcpStream::connect(("127.0.0.1", to)) {
                    pipe(s, up);
                }
            });
        }
    });
    (port, seen)
}

/// A SOCKS5 proxy (no authentication) that routes every target to `to`, or refuses with reply code `refuse`;
/// records the host names requested.
fn socks_proxy(to: u16, refuse: Option<u8>) -> (u16, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let seen = s2.clone();
            std::thread::spawn(move || {
                let mut s = s;
                let mut greet = [0u8; 3];
                if s.read_exact(&mut greet).is_err() || s.write_all(&[5, 0]).is_err() {
                    return;
                }
                let mut head = [0u8; 4];
                if s.read_exact(&mut head).is_err() {
                    return;
                }
                let name = match head[3] {
                    3 => {
                        let mut n = [0u8; 1];
                        let _ = s.read_exact(&mut n);
                        let mut b = vec![0u8; n[0] as usize];
                        let _ = s.read_exact(&mut b);
                        String::from_utf8_lossy(&b).to_string()
                    }
                    1 => {
                        let mut b = [0u8; 4];
                        let _ = s.read_exact(&mut b);
                        std::net::Ipv4Addr::from(b).to_string()
                    }
                    _ => String::new(),
                };
                let mut p = [0u8; 2];
                let _ = s.read_exact(&mut p);
                seen.lock().expect("seen").push(format!("{name}:{}", u16::from_be_bytes(p)));
                let code = refuse.unwrap_or(0);
                let _ = s.write_all(&[5, code, 0, 1, 0, 0, 0, 0, 0, 0]);
                if code != 0 {
                    return;
                }
                if let Ok(up) = TcpStream::connect(("127.0.0.1", to)) {
                    pipe(s, up);
                }
            });
        }
    });
    (port, seen)
}

fn manual(kind: Kind, port: u16) -> Choice {
    Choice::Manual(Proxy { kind, host: "127.0.0.1".into(), port })
}

fn nothing() -> Option<SystemProxies> {
    Some(SystemProxies::default())
}

static SYSTEM_SOCKS: std::sync::atomic::AtomicU16 = std::sync::atomic::AtomicU16::new(0);

fn system_with_socks() -> Option<SystemProxies> {
    Some(SystemProxies { socks: Some(("127.0.0.1".into(), SYSTEM_SOCKS.load(std::sync::atomic::Ordering::SeqCst))), ..Default::default() })
}

#[test]
fn a_connection_goes_the_way_the_choice_says() {
    let _turn = turn();
    read_system_with(nothing);
    let node = node();
    let ask = |url: &str| post_json(&parse(url).expect("address"), b"{}", &limits(), Ask::Read);

    // CONNECT: the host name goes to the proxy (never resolved locally: `.invalid` would not resolve).
    let (p, seen) = connect_proxy(node, None);
    set_choice(manual(Kind::Http, p));
    assert_eq!(ask(&format!("http://node.invalid:{node}/")).map(|a| a.body).ok(), Some(b"{\"through\":true}".to_vec()));
    assert_eq!(seen.lock().expect("seen").first().cloned(), Some(format!("CONNECT node.invalid:{node} HTTP/1.1")));

    // TLS is end to end with the node: an untrusted certificate behind the proxy is a certificate failure.
    let tls = untrusted_tls();
    let (p, _) = connect_proxy(tls, None);
    set_choice(manual(Kind::Http, p));
    assert!(matches!(ask(&format!("https://node.invalid:{tls}/")), Err(Fail::Certificate(_))));

    // SOCKS5: the domain address type, for the proxy to resolve.
    let (p, seen) = socks_proxy(node, None);
    set_choice(manual(Kind::Socks5, p));
    assert_eq!(ask(&format!("http://node.invalid:{node}/")).map(|a| a.status).ok(), Some(200));
    assert_eq!(seen.lock().expect("seen").first().cloned(), Some(format!("node.invalid:{node}")));

    // A refusing proxy or an absent one: a connection failure naming the proxy and its response.
    let (p, _) = connect_proxy(node, Some("HTTP/1.1 403 Forbidden"));
    set_choice(manual(Kind::Http, p));
    match ask(&format!("http://node.invalid:{node}/")) {
        Err(Fail::Connect(said)) => assert!(said.contains(&format!("via proxy http://127.0.0.1:{p}")) && said.contains("403"), "{said}"),
        other => panic!("{:?}", other.err()),
    }
    let (p, _) = socks_proxy(node, Some(5));
    set_choice(manual(Kind::Socks5, p));
    match ask(&format!("http://node.invalid:{node}/")) {
        Err(Fail::Connect(said)) => assert!(said.contains("socks5://127.0.0.1") && said.contains("code 5"), "{said}"),
        other => panic!("{:?}", other.err()),
    }
    let closed = TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port();
    set_choice(manual(Kind::Http, closed));
    match ask(&format!("http://node.invalid:{node}/")) {
        Err(Fail::Connect(said)) => assert!(said.contains(&format!("via proxy http://127.0.0.1:{closed}")), "{said}"),
        other => panic!("{:?}", other.err()),
    }

    // Loopback goes direct whatever the setting (the proxy here does not even exist).
    assert_eq!(ask(&format!("http://127.0.0.1:{node}/")).map(|a| a.status).ok(), Some(200));
    assert_eq!(ask(&format!("http://localhost:{node}/")).map(|a| a.status).ok(), Some(200));

    // No proxy: the name is resolved locally, and `.invalid` does not resolve.
    set_choice(Choice::Off);
    assert!(matches!(ask(&format!("http://node.invalid:{node}/")), Err(Fail::Name(_))));

    // Following the system: the platform settings, read when the connection opens.
    let (p, seen) = socks_proxy(node, None);
    SYSTEM_SOCKS.store(p, std::sync::atomic::Ordering::SeqCst);
    read_system_with(system_with_socks);
    set_choice(Choice::System);
    assert_eq!(ask(&format!("http://node.invalid:{node}/")).map(|a| a.status).ok(), Some(200));
    assert_eq!(seen.lock().expect("seen").len(), 1);
    read_system_with(nothing);
    assert!(matches!(ask(&format!("http://node.invalid:{node}/")), Err(Fail::Name(_))), "the system gives none: straight");
}

/// A stub proxy that runs `script` on every connection it accepts.
fn scripted(script: fn(TcpStream)) -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || script(s));
        }
    });
    port
}

/// Read a CONNECT request's head (to its blank line).
fn read_head(s: &mut TcpStream) -> bool {
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

/// Read a SOCKS5 greeting (version, count, methods).
fn read_greeting(s: &mut TcpStream) -> bool {
    let mut g = [0u8; 3];
    s.read_exact(&mut g).is_ok()
}

/// Read a SOCKS5 request with a domain-name address.
fn read_socks_request(s: &mut TcpStream) -> bool {
    let mut head = [0u8; 5];
    if s.read_exact(&mut head).is_err() {
        return false;
    }
    let mut rest = vec![0u8; head[4] as usize + 2];
    s.read_exact(&mut rest).is_ok()
}

fn hold(s: TcpStream) {
    std::thread::sleep(Duration::from_secs(10));
    drop(s);
}

/// The result of a request through a proxy of `kind` on `port`, under a deadline of `ms`.
fn through(kind: Kind, port: u16, ms: u64) -> Result<u16, Fail> {
    set_choice(manual(kind, port));
    let t = parse("http://node.invalid:8545/").expect("address");
    post_json(&t, b"{}", &Limits { deadline: Duration::from_millis(ms), max_answer: 0 }, Ask::Once).map(|a| a.status)
}

fn connect_said(r: Result<u16, Fail>) -> String {
    match r {
        Err(Fail::Connect(s)) => s,
        other => panic!("not the connection's failure: {other:?}"),
    }
}

/// Every way the proxy side of the tunnel can fail is reported distinctly: silence (the deadline), a partial
/// response, an unreadable response, a credentials demand; connection failures name the proxy.
#[test]
fn each_failure_on_the_proxys_side_is_named() {
    let _turn = turn();
    read_system_with(nothing);
    // Silent after accepting: the exchange deadline applies, not a hang.
    let p = scripted(|mut s| {
        let _ = read_head(&mut s);
        hold(s)
    });
    assert!(matches!(through(Kind::Http, p, 400), Err(Fail::Late(_))), "CONNECT silent");
    let p = scripted(hold);
    assert!(matches!(through(Kind::Socks5, p, 400), Err(Fail::Late(_))), "SOCKS5 silent before its greeting answer");
    let p = scripted(|mut s| {
        if read_greeting(&mut s) && s.write_all(&[5, 0]).is_ok() {
            hold(s)
        }
    });
    assert!(matches!(through(Kind::Socks5, p, 400), Err(Fail::Late(_))), "SOCKS5 silent before its reply");
    // A partial response, then closed.
    let p = scripted(|mut s| {
        if read_head(&mut s) {
            let _ = s.write_all(b"HTTP/1.1 200 Conn");
        }
    });
    let said = connect_said(through(Kind::Http, p, 3000));
    assert!(said.contains(&format!("via proxy http://127.0.0.1:{p}")) && said.contains("closed the connection"), "{said}");
    let p = scripted(|mut s| {
        if read_greeting(&mut s) && s.write_all(&[5, 0]).is_ok() && read_socks_request(&mut s) {
            let _ = s.write_all(&[5, 0, 0]);
        }
    });
    assert!(connect_said(through(Kind::Socks5, p, 3000)).contains("closed the connection"), "SOCKS5 reply cut");
    // Unreadable responses.
    let p = scripted(|mut s| {
        if read_head(&mut s) {
            let _ = s.write_all(b"garbage\r\n\r\n");
        }
    });
    assert!(connect_said(through(Kind::Http, p, 3000)).contains("answered garbage"));
    let p = scripted(|mut s| {
        if read_head(&mut s) {
            let _ = s.write_all(b"FOO 200 OK\r\n\r\n");
        }
    });
    assert!(connect_said(through(Kind::Http, p, 3000)).contains("answered FOO 200 OK"), "a 200 that is not an HTTP status line is not a tunnel");
    let p = scripted(|mut s| {
        if read_head(&mut s) {
            let _ = s.write_all(b"HTTP/1.1 200 OK\r\nX: ");
            let _ = s.write_all(&[b'x'; 20 << 10]);
            hold(s)
        }
    });
    assert!(connect_said(through(Kind::Http, p, 3000)).contains("no end of head"));
    for (greeting, said) in [([4u8, 0u8], "greeting answer does not read"), ([5, 2], "picked method 2")] {
        static ANSWER: Mutex<[u8; 2]> = Mutex::new([0, 0]);
        *ANSWER.lock().expect("answer") = greeting;
        let p = scripted(|mut s| {
            if read_greeting(&mut s) {
                let a = *ANSWER.lock().expect("answer");
                let _ = s.write_all(&a);
            }
        });
        assert!(connect_said(through(Kind::Socks5, p, 3000)).contains(said), "{said}");
    }
    for (reply, form) in [([4u8, 0u8, 0u8, 1u8], "version"), ([5, 0, 0, 9], "address type")] {
        static REPLY: Mutex<[u8; 4]> = Mutex::new([0; 4]);
        *REPLY.lock().expect("reply") = reply;
        let p = scripted(|mut s| {
            if read_greeting(&mut s) && s.write_all(&[5, 0]).is_ok() && read_socks_request(&mut s) {
                let r = *REPLY.lock().expect("reply");
                let _ = s.write_all(&r);
                hold(s)
            }
        });
        assert!(connect_said(through(Kind::Socks5, p, 3000)).contains("reply does not read"), "{form}");
    }
    // Requires credentials: unsupported, and reported as such.
    let p = scripted(|mut s| {
        if read_head(&mut s) {
            let _ = s.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nProxy-Authenticate: Basic\r\nContent-Length: 0\r\n\r\n");
        }
    });
    let said = connect_said(through(Kind::Http, p, 3000));
    assert!(said.contains("407") && said.contains("needs credentials"), "{said}");
    let p = scripted(|mut s| {
        if read_greeting(&mut s) {
            let _ = s.write_all(&[5, 0xFF]);
        }
    });
    assert!(connect_said(through(Kind::Socks5, p, 3000)).contains("needs authentication"));
}

/// A node that keeps connections open (HTTP/1.1, a length, no close), counting accepted connections.
fn keeping_node() -> (u16, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let conns = Arc::new(AtomicUsize::new(0));
    let c = conns.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            c.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                let mut s = s;
                loop {
                    if !read_head(&mut s) {
                        return;
                    }
                    let mut body = [0u8; 2];
                    if s.read_exact(&mut body).is_err() {
                        return;
                    }
                    let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}");
                }
            });
        }
    });
    (port, conns)
}

/// Kept connections are keyed by route: after switching to another proxy, a connection opened through the first
/// is not reused; after switching back, it is.
#[test]
fn a_kept_connection_is_kept_under_its_way_out() {
    let _turn = turn();
    read_system_with(nothing);
    let (node, conns) = keeping_node();
    let (a, a_seen) = connect_proxy(node, None);
    let (b, b_seen) = connect_proxy(node, None);
    let ask = || {
        let t = parse(&format!("http://node.invalid:{node}/")).expect("address");
        post_json(&t, b"{}", &limits(), Ask::Read).map(|x| x.status).ok()
    };
    set_choice(manual(Kind::Http, a));
    assert_eq!((ask(), ask()), (Some(200), Some(200)));
    assert_eq!((a_seen.lock().expect("a").len(), conns.load(Ordering::SeqCst)), (1, 1), "through the first: one tunnel, kept");
    set_choice(manual(Kind::Http, b));
    assert_eq!(ask(), Some(200));
    assert_eq!((b_seen.lock().expect("b").len(), conns.load(Ordering::SeqCst)), (1, 2), "another way: a new tunnel, the kept one not taken");
    set_choice(manual(Kind::Http, a));
    assert_eq!(ask(), Some(200));
    assert_eq!((a_seen.lock().expect("a").len(), conns.load(Ordering::SeqCst)), (1, 2), "back on the first: its kept connection");
}
