use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};
use zikaron_net::pac::{PacTrouble, RUN_LIMIT, SCRIPT_MAX};
use zikaron_net::{parse, post_json, read_system_with, set_choice, way_for, Ask, Choice, Fail, Kind, Limits, Proxy, Script, SystemProxies, Way, SYSTEM_READ_CAP};

fn turn() -> MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    static ROOT: OnceLock<bool> = OnceLock::new();
    let held = TURN.lock().unwrap_or_else(|e| e.into_inner());
    assert!(*ROOT.get_or_init(|| zikaron_net::drive_trust_root(include_bytes!("fixtures/tls-proxy/ca.crt.der"))), "the test root is added before any handshake");
    set_choice(Choice::System);
    held
}

static SCENE: Mutex<Option<SystemProxies>> = Mutex::new(None);

fn scene() -> Option<SystemProxies> {
    SCENE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn system_is(s: SystemProxies) {
    *SCENE.lock().unwrap_or_else(|e| e.into_inner()) = Some(s);
    read_system_with(scene);
}

fn pac_at(url: String) -> SystemProxies {
    SystemProxies { pac_url: Some(url), ..Default::default() }
}

fn limits() -> Limits {
    Limits { deadline: Duration::from_secs(10), max_answer: 0 }
}

fn px(kind: Kind, port: u16) -> Proxy {
    Proxy { kind, host: "127.0.0.1".into(), port, auth: None }
}

fn closed_port() -> u16 {
    TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port()
}

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

fn read_head(s: &mut impl Read) -> Option<String> {
    let mut head = Vec::new();
    let mut one = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if s.read(&mut one).ok()? == 0 {
            return None;
        }
        head.push(one[0]);
    }
    Some(String::from_utf8_lossy(&head).into_owned())
}

type Seen = Arc<Mutex<Vec<String>>>;

fn listen(serve: impl Fn(TcpStream, Seen) + Send + Sync + 'static) -> (u16, Seen) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    let (s2, serve) = (seen.clone(), Arc::new(serve));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (seen, serve) = (s2.clone(), serve.clone());
            std::thread::spawn(move || serve(s, seen));
        }
    });
    (port, seen)
}

fn served(status: u16, body: String) -> (u16, Seen) {
    listen(move |mut s, seen| {
        if let Some(h) = read_head(&mut s) {
            seen.lock().unwrap().push(h.lines().next().unwrap_or("").to_string());
            let _ = s.write_all(format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes());
            let _ = s.write_all(body.as_bytes());
        }
    })
}

fn script(find: &str) -> String {
    format!("function FindProxyForURL(url, host) {{ {find} }}")
}

fn node() -> (u16, Seen) {
    listen(|mut s, seen| {
        if let Some(h) = read_head(&mut s) {
            seen.lock().unwrap().push(h);
            let body = "{\"through\":true}";
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
        }
    })
}

fn tls_config(cert: &[u8], key: &[u8]) -> Arc<rustls::ServerConfig> {
    Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("versions")
            .with_no_client_auth()
            .with_single_cert(vec![rustls::pki_types::CertificateDer::from(cert.to_vec())], rustls::pki_types::PrivateKeyDer::Pkcs8(key.to_vec().into()))
            .expect("server config"),
    )
}

fn tls_node() -> u16 {
    let config = tls_config(include_bytes!("fixtures/tls-proxy/node.crt.der"), include_bytes!("fixtures/tls-proxy/node.key.der"));
    listen(move |s, _| {
        let Ok(c) = rustls::ServerConnection::new(config.clone()) else { return };
        let mut t = rustls::StreamOwned::new(c, s);
        if read_head(&mut t).is_some() {
            let body = "{\"tls\":true}";
            let _ = t.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            let _ = t.flush();
            t.conn.send_close_notify();
            let _ = t.flush();
        }
    })
    .0
}

fn http_proxy(to: u16, basic: Option<&'static str>) -> (u16, Seen) {
    listen(move |mut s, seen| {
        let Some(h) = read_head(&mut s) else { return };
        seen.lock().unwrap().push(h.clone());
        if let Some(b) = basic {
            if !h.lines().any(|l| l == format!("Proxy-Authorization: Basic {b}")) {
                let _ = s.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n");
                return;
            }
        }
        let _ = s.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
        if let Ok(up) = TcpStream::connect(("127.0.0.1", to)) {
            pipe(s, up);
        }
    })
}

fn https_proxy(to: u16, cert: &'static [u8], key: &'static [u8]) -> (u16, Seen) {
    let config = tls_config(cert, key);
    listen(move |s, seen| {
        let Ok(c) = rustls::ServerConnection::new(config.clone()) else { return };
        let mut t = rustls::StreamOwned::new(c, s);
        let Some(h) = read_head(&mut t) else { return };
        seen.lock().unwrap().push(h.lines().next().unwrap_or("").to_string());
        let _ = t.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
        let _ = t.flush();
        let Ok(mut up) = TcpStream::connect(("127.0.0.1", to)) else { return };
        let _ = t.sock.set_read_timeout(Some(Duration::from_millis(5)));
        let _ = up.set_read_timeout(Some(Duration::from_millis(5)));
        let mut buf = [0u8; 16 << 10];
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            match t.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if up.write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(_) => break,
            }
            match up.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    if t.write_all(&buf[..n]).and_then(|_| t.flush()).is_err() {
                        break;
                    }
                }
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(_) => break,
            }
        }
    })
}

fn socks5(to: u16, need: Option<(&'static str, &'static str)>) -> (u16, Seen) {
    listen(move |mut s, seen| {
        let mut g = [0u8; 2];
        if s.read_exact(&mut g).is_err() || g[0] != 5 {
            return;
        }
        let mut methods = vec![0u8; g[1] as usize];
        let _ = s.read_exact(&mut methods);
        let want = if need.is_some() { 2 } else { 0 };
        if !methods.contains(&want) {
            let _ = s.write_all(&[5, 0xFF]);
            return;
        }
        let _ = s.write_all(&[5, want]);
        seen.lock().unwrap().push(format!("method {want}"));
        if let Some((u, p)) = need {
            let got = (|| {
                let mut v = [0u8; 2];
                s.read_exact(&mut v).ok()?;
                let mut user = vec![0u8; v[1] as usize];
                s.read_exact(&mut user).ok()?;
                let mut pl = [0u8; 1];
                s.read_exact(&mut pl).ok()?;
                let mut pass = vec![0u8; pl[0] as usize];
                s.read_exact(&mut pass).ok()?;
                Some((String::from_utf8_lossy(&user).to_string(), String::from_utf8_lossy(&pass).to_string()))
            })();
            let Some((user, pass)) = got else {
                seen.lock().unwrap().push("no user".to_string());
                return;
            };
            seen.lock().unwrap().push(format!("user {user} pass {pass}"));
            let ok = user == u && pass == p;
            let _ = s.write_all(&[1, if ok { 0 } else { 1 }]);
            if !ok {
                return;
            }
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
            _ => String::new(),
        };
        let mut p = [0u8; 2];
        let _ = s.read_exact(&mut p);
        seen.lock().unwrap().push(format!("{name}:{}", u16::from_be_bytes(p)));
        let _ = s.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
        if let Ok(up) = TcpStream::connect(("127.0.0.1", to)) {
            pipe(s, up);
        }
    })
}

fn socks4_only(to: u16) -> (u16, Seen) {
    listen(move |mut s, seen| {
        let mut head = [0u8; 8];
        if s.read_exact(&mut head[..1]).is_err() || head[0] != 4 || s.read_exact(&mut head[1..]).is_err() {
            return;
        }
        let read_z = |s: &mut TcpStream| -> String {
            let mut out = Vec::new();
            let mut one = [0u8; 1];
            while s.read_exact(&mut one).is_ok() && one[0] != 0 {
                out.push(one[0]);
            }
            String::from_utf8_lossy(&out).to_string()
        };
        let id = read_z(&mut s);
        let addr = [head[4], head[5], head[6], head[7]];
        let name = if addr[..3] == [0, 0, 0] && addr[3] != 0 { read_z(&mut s) } else { std::net::Ipv4Addr::from(addr).to_string() };
        seen.lock().unwrap().push(format!("4 id={id} {name}:{}", u16::from_be_bytes([head[2], head[3]])));
        let _ = s.write_all(&[0, 90, 0, 0, 0, 0, 0, 0]);
        if let Ok(up) = TcpStream::connect(("127.0.0.1", to)) {
            pipe(s, up);
        }
    })
}

fn ask(url: &str) -> Result<Vec<u8>, Fail> {
    post_json(&parse(url).expect("address"), b"{}", &limits(), Ask::Once).map(|a| a.body)
}

fn reading(url: &str) -> zikaron_net::Reading {
    way_for(&parse(url).expect("address"), SYSTEM_READ_CAP + Duration::from_secs(10))
}

#[test]
fn a_script_names_the_ways_and_the_next_is_tried_when_one_fails() {
    let _t = turn();
    let (n, _) = node();
    let (s5, socks_seen) = socks5(n, None);
    let gone = closed_port();
    let body = script(&format!("if (host == 'direct.invalid') return 'DIRECT'; if (url.indexOf('secret') >= 0) return 'PROXY 127.0.0.1:1'; return 'PROXY 127.0.0.1:{gone}; SOCKS5 127.0.0.1:{s5}; DIRECT';"));
    let (pac, fetched) = served(200, body);
    system_is(pac_at(format!("http://127.0.0.1:{pac}/proxy.pac")));
    assert_eq!(ask(&format!("http://node.invalid:{n}/v3/secret")).ok(), Some(b"{\"through\":true}".to_vec()));
    assert_eq!(socks_seen.lock().unwrap().last().cloned(), Some(format!("node.invalid:{n}")), "the name goes to the proxy");
    let r = reading(&format!("http://node.invalid:{n}/"));
    assert_eq!((r.ways(), r.script), (vec![Way::Through(px(Kind::Http, gone)), Way::Through(px(Kind::Socks5, s5)), Way::Direct], Script::Used));
    let d = reading("https://direct.invalid/");
    assert_eq!((d.way, d.script), (Way::Direct, Script::Used));
    assert_eq!(fetched.lock().unwrap().clone(), vec!["GET /proxy.pac HTTP/1.1".to_string()], "fetched once, directly");
}

#[test]
fn a_script_that_cannot_be_used_is_named_and_the_route_is_the_one_without_it() {
    let _t = turn();
    let explicit = px(Kind::Http, 7890);
    for (find, trouble) in [
        ("throw new Error('no');", PacTrouble::Failed),
        ("for (;;) {}", PacTrouble::Timeout),
        ("var a = []; for (;;) a.push(new Array(100000).join('x') + a.length);", PacTrouble::Memory),
        ("return 42;", PacTrouble::Answer),
        ("return 'BOGUS x:1';", PacTrouble::Answer),
    ] {
        let (pac, _) = served(200, script(find));
        system_is(SystemProxies { https: Some(explicit.clone()), ..pac_at(format!("http://127.0.0.1:{pac}/p.pac")) });
        let began = Instant::now();
        let r = reading("https://node.invalid/");
        assert!(began.elapsed() < RUN_LIMIT + Duration::from_secs(2), "{find}: {:?}", began.elapsed());
        assert_eq!((r.way, r.script), (Way::Through(explicit.clone()), Script::Trouble(trouble)), "{find}");
    }
    let (none, _) = served(200, "var x = 1;".into());
    let (missing, _) = served(404, String::new());
    let (huge, _) = served(200, "/".repeat(SCRIPT_MAX + 1));
    for (url, trouble) in [
        (format!("http://127.0.0.1:{none}/p.pac"), PacTrouble::NoFunction),
        (format!("http://127.0.0.1:{}/p.pac", closed_port()), PacTrouble::Unfetched),
        (format!("http://127.0.0.1:{missing}/p.pac"), PacTrouble::Unfetched),
        (format!("http://127.0.0.1:{huge}/p.pac"), PacTrouble::TooLarge),
        ("file:///etc/proxy.pac".to_string(), PacTrouble::Scheme),
    ] {
        system_is(pac_at(url.clone()));
        let r = reading("https://node.invalid/");
        assert_eq!((r.way, r.script), (Way::Direct, Script::Trouble(trouble)), "{url}");
    }
}

#[test]
fn a_script_address_that_redirects_is_followed() {
    let _t = turn();
    let (pac, _) = served(200, script("return 'SOCKS5 127.0.0.1:1081';"));
    let to = format!("http://127.0.0.1:{pac}/real.pac");
    let absolute = listen(move |mut s, _| {
        if read_head(&mut s).is_some() {
            let _ = s.write_all(format!("HTTP/1.1 302 Found\r\nLocation: {to}\r\nContent-Length: 0\r\n\r\n").as_bytes());
        }
    })
    .0;
    system_is(pac_at(format!("http://127.0.0.1:{absolute}/moved.pac")));
    let r = reading("https://node.invalid/");
    assert_eq!((r.way, r.script), (Way::Through(px(Kind::Socks5, 1081)), Script::Used));
    let relative = listen(move |mut s, _| {
        let Some(h) = read_head(&mut s) else { return };
        if h.starts_with("GET /there.pac ") {
            let body = script("return 'SOCKS5 127.0.0.1:1082';");
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes());
        } else {
            let _ = s.write_all(b"HTTP/1.1 301 Moved\r\nLocation: /there.pac\r\nContent-Length: 0\r\n\r\n");
        }
    })
    .0;
    system_is(pac_at(format!("http://127.0.0.1:{relative}/here.pac")));
    assert_eq!(reading("https://node.invalid/").way, Way::Through(px(Kind::Socks5, 1082)));
}

#[test]
fn credentials_longer_than_socks5_carries_are_refused_by_name() {
    let _t = turn();
    let (n, _) = node();
    let (s5, seen) = socks5(n, Some(("u", "pw")));
    let long = zikaron_net::Auth { user: "u".into(), pass: Some(zikaron_net::Secret::new("p".repeat(300))) };
    set_choice(Choice::Manual(Proxy { kind: Kind::Socks5, host: "127.0.0.1".into(), port: s5, auth: Some(long) }));
    match ask(&format!("http://node.invalid:{n}/")) {
        Err(Fail::Proxy(why)) => assert!(why.contains("longer than SOCKS5 carries"), "{why}"),
        other => panic!("{other:?}"),
    }
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !seen.lock().unwrap().iter().any(|x| x.starts_with("user ") || x == "no user") && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let seen = seen.lock().unwrap().clone();
    assert!(seen.iter().any(|x| x == "no user"), "the proxy saw the exchange end: {seen:?}");
    assert!(seen.iter().all(|x| !x.starts_with("user ")), "nothing cut was sent: {seen:?}");
}

#[test]
fn credentials_go_to_the_proxy_that_asks_and_nowhere_else() {
    let _t = turn();
    let (n, node_seen) = node();
    let (hp, heads) = http_proxy(n, Some("dTpwIHc="));
    set_choice(Choice::Manual(zikaron_net::proxy_of(&format!("http://u:p%20w@127.0.0.1:{hp}")).expect("typed")));
    assert_eq!(ask(&format!("http://node.invalid:{n}/")).ok(), Some(b"{\"through\":true}".to_vec()));
    assert!(heads.lock().unwrap().last().is_some_and(|h| h.contains("Proxy-Authorization: Basic dTpwIHc=\r\n")));
    assert!(node_seen.lock().unwrap().iter().all(|h| !h.contains("Proxy-Authorization")), "the node never sees it");
    set_choice(Choice::Manual(px(Kind::Http, hp)));
    match ask(&format!("http://node.invalid:{n}/")) {
        Err(Fail::ProxyCredentials { given: false, said: why }) => assert!(why.contains("407") && why.contains("asks for credentials"), "{why}"),
        other => panic!("{other:?}"),
    }
    let (s5, socks_seen) = socks5(n, Some(("u", "pw")));
    set_choice(Choice::Manual(zikaron_net::proxy_of(&format!("socks5://u:pw@127.0.0.1:{s5}")).expect("typed")));
    assert_eq!(ask(&format!("http://node.invalid:{n}/")).ok(), Some(b"{\"through\":true}".to_vec()));
    assert_eq!(socks_seen.lock().unwrap()[..3].to_vec(), vec!["method 2".to_string(), "user u pass pw".to_string(), format!("node.invalid:{n}")]);
    set_choice(Choice::Manual(zikaron_net::proxy_of(&format!("socks5://u:bad@127.0.0.1:{s5}")).expect("typed")));
    match ask(&format!("http://node.invalid:{n}/")) {
        Err(Fail::ProxyCredentials { given: true, said: why }) => assert!(why.contains("refused the user name and password"), "{why}"),
        other => panic!("{other:?}"),
    }
    set_choice(Choice::Manual(px(Kind::Socks5, s5)));
    match ask(&format!("http://node.invalid:{n}/")) {
        Err(Fail::ProxyCredentials { given: false, said: why }) => assert!(why.contains("asks for authentication"), "{why}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_https_proxy_is_its_own_tls_leg_and_the_node_keeps_its_own() {
    let _t = turn();
    let tn = tls_node();
    let (hp, seen) = https_proxy(tn, include_bytes!("fixtures/tls-proxy/proxy.crt.der"), include_bytes!("fixtures/tls-proxy/proxy.key.der"));
    set_choice(Choice::Manual(zikaron_net::proxy_of(&format!("https://localhost:{hp}")).expect("typed")));
    assert_eq!(ask(&format!("https://node.invalid:{tn}/")).ok(), Some(b"{\"tls\":true}".to_vec()));
    assert_eq!(seen.lock().unwrap().last().cloned(), Some(format!("CONNECT node.invalid:{tn} HTTP/1.1")));
    let (bad, _) = https_proxy(tn, include_bytes!("fixtures/self-signed-cert.der"), include_bytes!("fixtures/self-signed-key.pk8.der"));
    set_choice(Choice::Manual(zikaron_net::proxy_of(&format!("https://localhost:{bad}")).expect("typed")));
    match ask(&format!("https://node.invalid:{tn}/")) {
        Err(Fail::Proxy(why)) => assert!(why.contains(&format!("via proxy https://localhost:{bad}")), "{why}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn socks_tries_five_then_four_a_with_the_name_at_the_proxy() {
    let _t = turn();
    let (n, _) = node();
    let (s4, seen) = socks4_only(n);
    let (pac, _) = served(200, script(&format!("return 'SOCKS 127.0.0.1:{s4}';")));
    system_is(pac_at(format!("http://127.0.0.1:{pac}/socks.pac")));
    assert_eq!(ask(&format!("http://node.invalid:{n}/")).ok(), Some(b"{\"through\":true}".to_vec()));
    assert_eq!(seen.lock().unwrap().last().cloned(), Some(format!("4 id= node.invalid:{n}")));
    assert_eq!(reading(&format!("http://node.invalid:{n}/")).ways(), vec![Way::Through(px(Kind::Socks5, s4)), Way::Through(px(Kind::Socks4, s4))]);
    set_choice(Choice::Manual(zikaron_net::proxy_of(&format!("socks4a://me@127.0.0.1:{s4}")).expect("typed")));
    assert_eq!(ask(&format!("http://node.invalid:{n}/")).ok(), Some(b"{\"through\":true}".to_vec()));
    assert_eq!(seen.lock().unwrap().last().cloned(), Some(format!("4 id=me node.invalid:{n}")));
}

#[test]
fn a_manual_proxy_leaves_local_networks_straight() {
    let _t = turn();
    let p = px(Kind::Http, 3128);
    set_choice(Choice::Manual(p.clone()));
    for url in ["https://192.168.1.10:8545/", "https://10.0.0.2/", "https://169.254.0.9/", "https://[fe80::2]/", "https://box.local/"] {
        let r = reading(url);
        assert_eq!((r.way, r.local), (Way::Direct, true), "{url}");
    }
    assert!(reading("http://127.0.0.1:8545/").loopback);
    assert_eq!(reading("https://node.example/").way, Way::Through(p));
}
