#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::time::{Duration, Instant};

mod answer;
mod dial;
mod keep;
pub mod pac;
mod route;

pub use dial::{CONNECT_CAP, STAGGER};
pub use keep::{close_down, cuts, open_up, IDLE_KEEP, PER_PLACE};
pub use pac::{Auto, PacTrouble};
pub use route::{excepted, follow_the_system, follow_the_terminal, script_trouble_for, script_trouble_last, is_local, is_loopback, proxy_of, proxy_text_shown, read_choice, read_system_with, set_choice, way_for, way_shown, way_shown_under, way_under, Auth, Choice, Kind, NotAProxy, Proxy, Reading, Script, Secret, SystemProxies, Way, SYSTEM_FRESH, SYSTEM_READ_CAP};

pub const USER_AGENT: &str = "ZIKARON";

pub(crate) fn request_head(method: &str, target: &str, host: &str, lines: &str) -> String {
    format!("{method} {target} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: {USER_AGENT}\r\n{lines}\r\n")
}

pub const SLICE: Duration = Duration::from_millis(250);

pub mod env {
    pub const TIMEOUT_SECS: &str = "ZKA_TIMEOUT_SECS";
    pub const TIMEOUT_MS: &str = "ZKA_TIMEOUT_MS";
    pub const MAX_ANSWER_BYTES: &str = "ZKA_MAX_ANSWER_BYTES";
}

#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub deadline: std::time::Duration,
    pub max_answer: usize,
}

impl Limits {
    pub fn from_env() -> Limits {
        Limits::from_vars(|k| std::env::var(k).ok())
    }

    pub fn from_vars(var: impl Fn(&str) -> Option<String>) -> Limits {
        let secs: u64 = var(env::TIMEOUT_SECS).and_then(|x| x.parse().ok()).unwrap_or(30);
        let deadline = match var(env::TIMEOUT_MS).and_then(|x| x.parse::<u64>().ok()) {
            Some(ms) => std::time::Duration::from_millis(ms),
            None => std::time::Duration::from_secs(secs),
        };
        let max: usize = var(env::MAX_ANSWER_BYTES).and_then(|x| x.parse().ok()).unwrap_or(64 << 20);
        Limits { deadline, max_answer: max }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Clock {
    limit: Duration,
    deadline: Option<Instant>,
}

impl Clock {
    fn start(limit: Duration) -> Clock {
        Clock { limit, deadline: (!limit.is_zero()).then(|| Instant::now() + limit) }
    }

    pub(crate) fn left(&self) -> Result<Option<Duration>, Fail> {
        match self.deadline {
            None => Ok(None),
            Some(d) => {
                let now = Instant::now();
                if now >= d {
                    Err(Fail::Late(self.limit))
                } else {
                    Ok(Some(d - now))
                }
            }
        }
    }

    pub(crate) fn late(&self) -> Fail {
        Fail::Late(self.limit)
    }

    fn system_wait(&self) -> Result<Duration, Fail> {
        let cap = route::SYSTEM_READ_CAP + pac::FETCH_LIMIT + pac::RUN_LIMIT;
        Ok(self.left()?.map(|l| l.min(cap)).unwrap_or(cap))
    }

    fn bound(&self, tcp: &std::net::TcpStream) -> Result<(), Fail> {
        let left = self.left()?;
        let read = Some(left.map_or(SLICE, |l| l.min(SLICE)));
        tcp.set_read_timeout(read).and_then(|_| tcp.set_write_timeout(left)).map_err(|e| if keep::closing() { keep::cut() } else { Fail::Stream(e.to_string()) })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Scheme {
    Http,
    Https,
}

impl Scheme {
    pub fn as_str(self) -> &'static str {
        match self {
            Scheme::Http => "http",
            Scheme::Https => "https",
        }
    }

    fn default_port(self) -> u16 {
        match self {
            Scheme::Http => 80,
            Scheme::Https => 443,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Target {
    pub scheme: Scheme,
    pub host: String,
    pub port: u16,
    pub path: String,
}

fn blank_or_control(s: &str) -> bool {
    s.chars().any(|c| c.is_control() || c.is_whitespace())
}

pub fn parse(url: &str) -> Option<Target> {
    read_address(url).ok()
}

pub fn sayable(url: &str) -> String {
    match parse(url) {
        Some(t) => t.sayable(),
        None => format!("({} bytes)", url.trim().len()),
    }
}

pub fn place_key(url: &str) -> String {
    parse(url).map(|t| t.place_id()).unwrap_or_else(|| url.trim().to_string())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotAnAddress {
    Scheme,
    Port,
    Host,
    Shape,
}

pub fn read_address(url: &str) -> Result<Target, NotAnAddress> {
    let (scheme, rest) = url.trim().split_once("://").ok_or(NotAnAddress::Scheme)?;
    let scheme = match scheme.to_ascii_lowercase().as_str() {
        "http" => Scheme::Http,
        "https" => Scheme::Https,
        _ => return Err(NotAnAddress::Scheme),
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.contains('@') || blank_or_control(authority) || blank_or_control(path) {
        return Err(NotAnAddress::Shape);
    }
    let port_of = |p: &str| p.parse::<u16>().map_err(|_| NotAnAddress::Port);
    let (host, port) = match authority.strip_prefix('[') {
        Some(inner) => {
            let (literal, after) = inner.split_once(']').ok_or(NotAnAddress::Host)?;
            literal.parse::<std::net::Ipv6Addr>().map_err(|_| NotAnAddress::Host)?;
            let port = match after {
                "" => scheme.default_port(),
                p => port_of(p.strip_prefix(':').ok_or(NotAnAddress::Host)?)?,
            };
            (&authority[..literal.len() + 2], port)
        }
        None => match authority.rsplit_once(':') {
            Some((h, p)) => (h, port_of(p)?),
            None => (authority, scheme.default_port()),
        },
    };
    if host.is_empty() {
        return Err(NotAnAddress::Host);
    }
    Ok(Target { scheme, host: host.to_ascii_lowercase(), port, path: path.to_string() })
}

impl Target {
    pub fn authority(&self) -> String {
        if self.port == self.scheme.default_port() {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    pub fn sayable(&self) -> String {
        format!("{}://{}", self.scheme.as_str(), self.authority())
    }

    pub fn url(&self) -> String {
        format!("{}://{}{}", self.scheme.as_str(), self.authority(), self.path)
    }

    pub fn place_id(&self) -> String {
        let host = match self.dial_host().parse::<std::net::Ipv6Addr>() {
            Ok(v6) if self.host.starts_with('[') => format!("[{v6}]"),
            _ => self.host.clone(),
        };
        format!("{}://{}:{}{}", self.scheme.as_str(), host, self.port, self.path)
    }

    pub fn dial_host(&self) -> &str {
        self.host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(&self.host)
    }

    fn place(&self, way: &Way) -> keep::Place {
        let via = match way {
            Way::Direct => None,
            Way::Through(p) => Some(p.clone()),
        };
        keep::Place { scheme: self.scheme, host: self.host.clone(), port: self.port, via }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Fail {
    Name(String),
    Connect(String),
    Handshake(String),
    Certificate(String),
    Late(std::time::Duration),
    Overlong(usize),
    Stream(String),
    Proxy(String),
    ProxyCredentials { given: bool, said: String },
    Closed,
}

static DRIVE_ROOTS: std::sync::OnceLock<Vec<Vec<u8>>> = std::sync::OnceLock::new();

pub fn drive_trust_root(der: &[u8]) -> bool {
    DRIVE_ROOTS.set(vec![der.to_vec()]).is_ok()
}

fn tls_config() -> Result<std::sync::Arc<rustls::ClientConfig>, Fail> {
    static CONFIG: std::sync::OnceLock<std::sync::Arc<rustls::ClientConfig>> = std::sync::OnceLock::new();
    if let Some(c) = CONFIG.get() {
        return Ok(c.clone());
    }
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    for der in DRIVE_ROOTS.get().map(|v| v.as_slice()).unwrap_or(&[]) {
        roots.add(rustls::pki_types::CertificateDer::from(der.clone())).map_err(|e| Fail::Handshake(e.to_string()))?;
    }
    let config = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|e| Fail::Handshake(e.to_string()))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(CONFIG.get_or_init(|| std::sync::Arc::new(config)).clone())
}

fn tls_fail(e: &std::io::Error) -> Fail {
    match e.get_ref().and_then(|x| x.downcast_ref::<rustls::Error>()) {
        Some(r) => certificate_fail(r).unwrap_or_else(|| Fail::Handshake(r.to_string())),
        None => Fail::Handshake(e.to_string()),
    }
}

fn certificate_fail(e: &rustls::Error) -> Option<Fail> {
    match e {
        rustls::Error::InvalidCertificate(c) => Some(Fail::Certificate(format!("{c:?}"))),
        rustls::Error::NoCertificatesPresented => Some(Fail::Certificate("NoCertificatesPresented".into())),
        rustls::Error::InvalidCertRevocationList(c) => Some(Fail::Certificate(format!("{c:?}"))),
        _ => None,
    }
}

pub(crate) enum Sock {
    Tcp(std::net::TcpStream),
    Tls(Box<Tls>),
}

impl Sock {
    pub(crate) fn tcp(&self) -> &std::net::TcpStream {
        match self {
            Sock::Tcp(s) => s,
            Sock::Tls(t) => t.sock.tcp(),
        }
    }
}

impl Read for Sock {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Sock::Tcp(s) => s.read(buf),
            Sock::Tls(t) => t.read(buf),
        }
    }
}

impl Write for Sock {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            Sock::Tcp(s) => s.write(buf),
            Sock::Tls(t) => t.send(buf).map(|_| buf.len()),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Sock::Tcp(s) => s.flush(),
            Sock::Tls(t) => t.sock.flush(),
        }
    }
}

pub(crate) struct Tls {
    conn: rustls::ClientConnection,
    sock: Sock,
}

impl Tls {
    fn step(&mut self) -> std::io::Result<usize> {
        if self.conn.wants_write() {
            return self.conn.write_tls(&mut self.sock);
        }
        let n = self.conn.read_tls(&mut self.sock)?;
        if let Err(e) = self.conn.process_new_packets() {
            let _ = self.conn.write_tls(&mut self.sock);
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e));
        }
        Ok(n)
    }

    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.conn.reader().read(buf) {
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
            other => return other,
        }
        self.step()?;
        self.conn.reader().read(buf)
    }

    fn send(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        let mut rest = bytes;
        while !rest.is_empty() || self.conn.wants_write() {
            let took = if rest.is_empty() { 0 } else { self.conn.writer().write(rest)? };
            rest = &rest[took..];
            if !self.conn.wants_write() {
                if took == 0 {
                    return Err(std::io::ErrorKind::WriteZero.into());
                }
                continue;
            }
            if self.conn.write_tls(&mut self.sock)? == 0 {
                return Err(std::io::ErrorKind::WriteZero.into());
            }
        }
        self.sock.flush()
    }
}

fn in_the_handshake(f: Fail) -> Fail {
    match f {
        Fail::Stream(why) => Fail::Handshake(why),
        other => other,
    }
}

fn tls_connect(host: &str, sock: Sock, clock: &Clock) -> Result<Tls, Fail> {
    let name = rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|e| Fail::Name(format!("{host}: {e}")))?;
    let conn = rustls::ClientConnection::new(tls_config()?, name).map_err(|e| Fail::Handshake(e.to_string()))?;
    let mut tls = Tls { conn, sock };
    while tls.conn.is_handshaking() || tls.conn.wants_write() {
        if keep::closing() {
            return Err(keep::cut());
        }
        clock.bound(tls.sock.tcp())?;
        let step = tls.step();
        if keep::closing() {
            return Err(keep::cut());
        }
        match step {
            Ok(0) => return Err(Fail::Handshake("the peer closed the connection during the handshake".to_string())),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => return Err(tls_fail(&e)),
        }
    }
    Ok(tls)
}

pub(crate) enum Stream {
    Plain(Sock),
    Tls(Box<Tls>),
}

impl Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Stream::Plain(s) => s.read(buf),
            Stream::Tls(s) => s.read(buf),
        }
    }

    fn send(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            Stream::Plain(s) => s.write_all(bytes).and_then(|_| s.flush()),
            Stream::Tls(s) => s.send(bytes),
        }
    }

    pub(crate) fn tcp(&self) -> &std::net::TcpStream {
        match self {
            Stream::Plain(s) => s.tcp(),
            Stream::Tls(s) => s.sock.tcp(),
        }
    }

    fn fail(&self, e: std::io::Error) -> Fail {
        match e.get_ref().and_then(|x| x.downcast_ref::<rustls::Error>()) {
            Some(r) => certificate_fail(r).unwrap_or_else(|| Fail::Stream(format!("TLS: {r}"))),
            None => Fail::Stream(e.to_string()),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ask {
    Read,
    Once,
}

fn leg(t: &Target, way: &Way, clock: &Clock) -> Result<(Sock, keep::Ticket), Fail> {
    match way {
        Way::Direct => {
            let tcp = dial::connect(t, clock)?;
            let ticket = keep::ticket(&tcp)?;
            Ok((Sock::Tcp(tcp), ticket))
        }
        Way::Through(p) => route::tunnel(t, p, clock),
    }
}

fn open(t: &Target, ways: &[Way], clock: &Clock) -> Result<keep::Live, Fail> {
    if keep::closing() {
        return Err(Fail::Closed);
    }
    let mut failed: Vec<Fail> = Vec::new();
    let mut opened = None;
    for way in ways {
        match leg(t, way, clock) {
            Ok(l) => {
                opened = Some((way, l));
                break;
            }
            Err(f @ (Fail::Late(_) | Fail::Closed)) => return Err(f),
            Err(f) => {
                failed.push(f);
                if clock.left().is_err() {
                    break;
                }
            }
        }
    }
    let Some((way, (sock, ticket))) = opened else {
        return Err(match failed.len() {
            0 => Fail::Connect("no way to the node".into()),
            1 => failed.remove(0),
            _ if failed.iter().all(|f| matches!(f, Fail::ProxyCredentials { .. })) => Fail::ProxyCredentials {
                given: failed.iter().any(|f| matches!(f, Fail::ProxyCredentials { given: true, .. })),
                said: failed.iter().map(fail_said).collect::<Vec<_>>().join("; then "),
            },
            _ if failed.iter().all(|f| matches!(f, Fail::Proxy(_) | Fail::ProxyCredentials { .. })) => Fail::Proxy(failed.iter().map(fail_said).collect::<Vec<_>>().join("; then ")),
            _ => Fail::Connect(failed.iter().map(fail_said).collect::<Vec<_>>().join("; then ")),
        });
    };
    let stream = match t.scheme {
        Scheme::Http => Stream::Plain(sock),
        Scheme::Https => Stream::Tls(Box::new(tls_connect(t.dial_host(), sock, clock).map_err(in_the_handshake)?)),
    };
    Ok(keep::live(keep::Conn { stream, place: t.place(way) }, ticket))
}

fn fail_said(f: &Fail) -> String {
    match f {
        Fail::Name(x) | Fail::Connect(x) | Fail::Handshake(x) | Fail::Certificate(x) | Fail::Stream(x) | Fail::Proxy(x) => x.clone(),
        Fail::ProxyCredentials { said, .. } => said.clone(),
        Fail::Late(d) => format!("the deadline passed ({d:?})"),
        Fail::Overlong(n) => format!("an answer passed {n} bytes"),
        Fail::Closed => "closed".into(),
    }
}

enum Round {
    Broke(String),
    Failed(Fail),
}

fn breaks(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::*;
    matches!(e.kind(), ConnectionReset | ConnectionAborted | BrokenPipe | NotConnected | UnexpectedEof)
}

fn round(stream: &mut Stream, request: &[u8], limits: &Limits, clock: &Clock) -> Result<answer::Reader, Round> {
    let closed = || Round::Failed(keep::cut());
    let bound = |stream: &Stream, began: bool| match clock.bound(stream.tcp()) {
        Err(Fail::Stream(why)) if !began => Err(Round::Broke(why)),
        other => other.map_err(Round::Failed),
    };
    bound(stream, false)?;
    if let Err(e) = stream.send(request) {
        if keep::closing() {
            return Err(closed());
        }
        if breaks(&e) {
            return Err(Round::Broke(e.to_string()));
        }
        if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) {
            clock.left().map_err(Round::Failed)?;
        }
        return Err(Round::Failed(stream.fail(e)));
    }
    let mut reader = answer::Reader::new();
    let mut chunk = [0u8; 16 << 10];
    loop {
        bound(stream, !reader.raw().is_empty())?;
        let got = stream.read(&mut chunk);
        if keep::closing() {
            return Err(closed());
        }
        match got {
            Ok(n) if n > 0 => {
                if limits.max_answer > 0 && reader.raw().len() + n > limits.max_answer {
                    return Err(Round::Failed(Fail::Overlong(limits.max_answer)));
                }
                if reader.feed(&chunk[..n]).map_err(Round::Failed)? {
                    return Ok(reader);
                }
            }
            Ok(_) => return peer_closed(reader),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return peer_closed(reader),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) if breaks(&e) && reader.raw().is_empty() => return Err(Round::Broke(e.to_string())),
            Err(e) => return Err(Round::Failed(stream.fail(e))),
        }
        clock.left().map_err(Round::Failed)?;
    }
}

fn peer_closed(mut reader: answer::Reader) -> Result<answer::Reader, Round> {
    if reader.raw().is_empty() {
        return Err(Round::Broke("the connection closed before the answer began".into()));
    }
    reader.closed().map_err(Round::Failed)?;
    Ok(reader)
}

pub const DIAL_AGAIN_AFTER: Duration = Duration::from_millis(300);

static DIAL_AGAIN: std::sync::Mutex<Option<Duration>> = std::sync::Mutex::new(None);

pub fn set_dial_again(d: Option<Duration>) {
    *DIAL_AGAIN.lock().unwrap_or_else(|e| e.into_inner()) = d;
}

fn dial(t: &Target, ways: &[Way], clock: &Clock) -> Result<keep::Live, Fail> {
    match open(t, ways, clock) {
        Err(f @ (Fail::Connect(_) | Fail::Handshake(_))) => {
            let beat = DIAL_AGAIN.lock().unwrap_or_else(|e| e.into_inner()).unwrap_or(DIAL_AGAIN_AFTER);
            if clock.left()?.is_some_and(|left| left <= beat) {
                return Err(f);
            }
            if !beat.is_zero() {
                std::thread::sleep(beat);
            }
            open(t, ways, clock)
        }
        r => r,
    }
}

fn carry(t: &Target, request: &[u8], limits: &Limits, how: Ask) -> Result<answer::Reader, Fail> {
    let clock = Clock::start(limits.deadline);
    let ways = route::way_for(t, clock.system_wait()?).ways();
    let mut again = false;
    loop {
        let mut live = match how {
            Ask::Read if again => dial(t, &ways, &clock)?,
            Ask::Read => match ways.iter().find_map(|w| keep::take(&t.place(w))) {
                Some(l) => l,
                None => dial(t, &ways, &clock)?,
            },
            Ask::Once => open(t, &ways, &clock)?,
        };
        route::note_reached(t);
        match round(&mut live.conn.stream, request, limits, &clock) {
            Ok(reader) => {
                if how == Ask::Read && reader.keeps() {
                    keep::put(live);
                }
                return Ok(reader);
            }
            Err(Round::Broke(_)) if how == Ask::Read && !again && !keep::closing() => again = true,
            Err(Round::Broke(why)) => return Err(Fail::Stream(why)),
            Err(Round::Failed(f)) => return Err(f),
        }
    }
}

pub fn exchange(t: &Target, request: &[u8], limits: &Limits) -> Result<Vec<u8>, Fail> {
    let clock = Clock::start(limits.deadline);
    let ways = route::way_for(t, clock.system_wait()?).ways();
    let mut live = open(t, &ways, &clock)?;
    route::note_reached(t);
    match round(&mut live.conn.stream, request, limits, &clock) {
        Ok(reader) => Ok(reader.raw().to_vec()),
        Err(Round::Broke(why)) => Err(Fail::Stream(why)),
        Err(Round::Failed(f)) => Err(f),
    }
}

pub struct Answer {
    pub status: u16,
    pub location: Option<String>,
    pub body: Vec<u8>,
}

fn answer_of(reader: answer::Reader) -> Result<Answer, Fail> {
    let (head, body) = reader.answer().ok_or_else(|| Fail::Stream("the answer is not whole".into()))?;
    Ok(Answer { status: head.status, location: head.location, body })
}

fn closing_line(how: Ask) -> &'static str {
    match how {
        Ask::Read => "",
        Ask::Once => "Connection: close\r\n",
    }
}

pub fn post_json(t: &Target, body: &[u8], limits: &Limits, how: Ask) -> Result<Answer, Fail> {
    post(t, "application/json", body, limits, how)
}

pub fn post(t: &Target, content_type: &str, body: &[u8], limits: &Limits, how: Ask) -> Result<Answer, Fail> {
    let head = request_head(
        "POST",
        &t.path,
        &t.authority(),
        &format!("Content-Type: {content_type}\r\nContent-Length: {}\r\n{}", body.len(), closing_line(how)),
    );
    let mut request = head.into_bytes();
    request.extend_from_slice(body);
    answer_of(carry(t, &request, limits, how)?)
}

pub fn get(t: &Target, limits: &Limits) -> Result<Answer, Fail> {
    let head = request_head("GET", &t.path, &t.authority(), &format!("Accept: */*\r\n{}", closing_line(Ask::Read)));
    answer_of(carry(t, head.as_bytes(), limits, Ask::Read)?)
}

pub(crate) fn get_direct(t: &Target, limits: &Limits) -> Result<Answer, Fail> {
    let clock = Clock::start(limits.deadline);
    let head = request_head("GET", &t.path, &t.authority(), &format!("Accept: */*\r\n{}", closing_line(Ask::Once)));
    let mut live = open(t, &[Way::Direct], &clock)?;
    match round(&mut live.conn.stream, head.as_bytes(), limits, &clock) {
        Ok(reader) => answer_of(reader),
        Err(Round::Broke(why)) => Err(Fail::Stream(why)),
        Err(Round::Failed(f)) => Err(f),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn silent_peer(then: &'static [u8]) -> (u16, Arc<AtomicUsize>) {
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = l.local_addr().expect("addr").port();
        let heard = Arc::new(AtomicUsize::new(0));
        let h = heard.clone();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let h = h.clone();
                std::thread::spawn(move || {
                    let mut s = s;
                    let mut buf = [0u8; 4096];
                    let _ = s.read(&mut buf);
                    let _ = s.write_all(then);
                    h.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_secs(30));
                });
            }
        });
        (port, heard)
    }

    fn cut_by_the_flag_alone(heard: &Arc<AtomicUsize>, ask: impl FnOnce() -> Option<Fail> + Send + 'static) -> (Option<Fail>, Duration) {
        let asking = std::thread::spawn(ask);
        while heard.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(50));
        let cut_before = keep::cuts();
        let raised = Instant::now();
        keep::raise_flag_only();
        let said = asking.join().expect("the asking thread");
        let took = raised.elapsed();
        keep::open_up();
        assert_eq!(keep::cuts(), cut_before + 1, "the cut leaves its trace");
        (said, took)
    }

    #[test]
    fn a_timeout_refused_while_shutting_down_is_the_cut() {
        let Some(_turn) = keep::turn() else { return };
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let tcp = std::net::TcpStream::connect(l.local_addr().expect("addr")).expect("connect");
        let _ = tcp.shutdown(std::net::Shutdown::Both);
        let clock = Clock::start(Duration::from_secs(20));
        keep::raise_flag_only();
        let before = keep::cuts();
        let got = clock.bound(&tcp);
        let after = keep::cuts();
        keep::open_up();
        match got {
            Ok(()) => assert_eq!(after, before, "taken: nothing cut here"),
            Err(f) => {
                assert!(matches!(f, Fail::Closed), "refused while shutting down: closed, not {f:?}");
                assert_eq!(after, before + 1, "the cut leaves its trace");
            }
        }
    }

    #[test]
    fn a_cut_rests_on_the_flag_and_lands_within_a_slice() {
        let Some(_turn) = keep::turn() else { return };
        let limits = Limits { deadline: Duration::from_secs(20), max_answer: 0 };
        let none = Limits { deadline: Duration::ZERO, max_answer: 0 };
        for (form, then, limits) in [
            ("asked, nothing back", &b""[..], limits),
            ("half an answer in", &b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nab"[..], limits),
            ("no deadline at all", &b""[..], none),
        ] {
            let (port, heard) = silent_peer(then);
            let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
            let (said, took) = cut_by_the_flag_alone(&heard, move || post_json(&t, b"{}", &limits, Ask::Once).err());
            assert_eq!(said, Some(Fail::Closed), "{form}");
            assert!(took < SLICE * 4, "{form}: cut within a slice or so, not at the deadline: {took:?}");
        }
        let (port, heard) = silent_peer(b"");
        let t = parse(&format!("https://127.0.0.1:{port}/")).expect("address");
        let (said, took) = cut_by_the_flag_alone(&heard, move || open(&t, &[Way::Direct], &Clock::start(limits.deadline)).err());
        assert_eq!(said, Some(Fail::Closed), "a handshake in flight");
        assert!(took < SLICE * 4, "a handshake in flight: {took:?}");
        let (port, heard) = silent_peer(b"");
        let p = Proxy { kind: Kind::Http, host: "127.0.0.1".into(), port, auth: None };
        let t = parse("http://node.invalid:8545/").expect("address");
        let (said, took) = cut_by_the_flag_alone(&heard, move || route::tunnel(&t, &p, &Clock::start(limits.deadline)).err());
        assert_eq!(said, Some(Fail::Closed), "a tunnel in flight");
        assert!(took < SLICE * 4, "a tunnel in flight: {took:?}");
    }

    #[test]
    fn the_deadline_is_kept_to_the_instant_not_to_a_slice() {
        let Some(_turn) = keep::turn() else { return };
        let (port, heard) = silent_peer(b"");
        let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
        let limits = Limits { deadline: Duration::from_millis(60), max_answer: 0 };
        let began = Instant::now();
        assert_eq!(post_json(&t, b"{}", &limits, Ask::Once).err(), Some(Fail::Late(Duration::from_millis(60))));
        let took = began.elapsed();
        assert!(heard.load(Ordering::SeqCst) == 1, "the request went out");
        assert!(took >= Duration::from_millis(60) && took < SLICE, "late at the deadline: {took:?}");
    }

    #[test]
    fn a_cut_and_the_deadline_in_one_slice_end_as_either() {
        let Some(_turn) = keep::turn() else { return };
        let (port, heard) = silent_peer(b"");
        let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
        let limits = Limits { deadline: Duration::from_millis(300), max_answer: 0 };
        let began = Instant::now();
        let asking = std::thread::spawn(move || post_json(&t, b"{}", &limits, Ask::Once).err());
        while heard.load(Ordering::SeqCst) == 0 {
            std::thread::sleep(Duration::from_millis(5));
        }
        std::thread::sleep(Duration::from_millis(300).saturating_sub(began.elapsed()).saturating_sub(Duration::from_millis(20)));
        keep::raise_flag_only();
        let said = asking.join().expect("the asking thread");
        let took = began.elapsed();
        keep::open_up();
        assert!(matches!(said, Some(Fail::Closed) | Some(Fail::Late(_))), "{said:?}");
        assert!(took < Duration::from_millis(300) + SLICE, "{took:?}");
    }

    #[test]
    fn a_sentence_names_a_node_by_scheme_host_and_port_only() {
        for (url, said) in [
            ("https://mainnet.example/v3/S3CRETk3y", "https://mainnet.example"),
            ("https://node.example:8443/rpc?apikey=S3CRETk3y", "https://node.example:8443"),
            ("HTTP://Node.Example:80/x#S3CRETk3y", "http://node.example"),
            ("http://127.0.0.1:8545", "http://127.0.0.1:8545"),
            ("http://[::1]:8545/S3CRETk3y", "http://[::1]:8545"),
        ] {
            assert_eq!(sayable(url), said, "{url}");
        }
        for not_an_address in ["https://user:S3CRETk3y@node.example/", "node.example/S3CRETk3y", "wss://node.example/S3CRETk3y"] {
            let s = sayable(not_an_address);
            assert!(!s.contains("S3CRETk3y") && !s.contains("node.example"), "{not_an_address}: {s}");
            assert_eq!(s, format!("({} bytes)", not_an_address.len()));
        }
    }

    #[test]
    fn the_scheme_and_host_are_read_without_case() {
        let t = parse("HTTPS://Node.Example:8443/Rpc").expect("an address");
        assert_eq!(t.scheme, Scheme::Https);
        assert_eq!(t.host, "node.example");
        assert_eq!(t.port, 8443);
        assert_eq!(t.path, "/Rpc");
        assert_eq!(parse("Http://127.0.0.1:8545").map(|t| (t.scheme, t.port, t.path)), Some((Scheme::Http, 8545, "/".into())));
        assert_eq!(parse("https://a.example").map(|t| t.url()), Some("https://a.example/".into()));
        for bad in ["ftp://a.example/", "https://", "https://:443/", "https://u@127.0.0.1/", "https://a.example:x/", "a.example"] {
            assert!(parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn an_ipv6_literal_is_a_host_whole() {
        let t = parse("http://[::1]:8545/rpc").expect("an address");
        assert_eq!((t.host.as_str(), t.port, t.path.as_str(), t.dial_host()), ("[::1]", 8545, "/rpc", "::1"));
        assert_eq!(t.authority(), "[::1]:8545");
        let t = parse("https://[2001:DB8::1]").expect("an address");
        assert_eq!((t.host.as_str(), t.port, t.url()), ("[2001:db8::1]", 443, "https://[2001:db8::1]/".to_string()));
        for bad in ["http://[::1", "http://[::1]x/", "http://[::1]:/", "http://[not-v6]:80/", "http://[127.0.0.1]/", "http://[fe80::1%en0]/"] {
            assert!(parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn the_address_forms_read_before_are_read_the_same() {
        assert!(parse("https://a.example:/").is_none());
        assert_eq!(parse("  https://a.example/x  ").map(|t| t.url()), Some("https://a.example/x".into()));
        assert_eq!(parse("https://a.example:+8443/").map(|t| t.port), Some(8443));
        assert!(parse("https://a.example,https://b.example").is_none());
        assert_eq!(parse("HTTPS://a.example/").map(|t| t.scheme), Some(Scheme::Https));
        assert_eq!(parse("https://a.example?x=1").map(|t| t.host), Some("a.example?x=1".into()));
        assert_eq!(parse("http://::1:8545").map(|t| (t.host, t.port)), Some(("::1".into(), 8545)));
        assert_eq!(parse("https://a.example:0/").map(|t| t.port), Some(0));
        assert_eq!(parse("https://a.example:65535/").map(|t| t.port), Some(65535));
        assert!(parse("https://a.example:65536/").is_none());
        for bad in ["https://a.example/x\r\nHost: b", "https://a.ex ample/", "https://a.example/a b", "https://a.example/a\tb", "https://a\u{0}.example/", "https://a.example/\u{7f}"] {
            assert!(parse(bad).is_none(), "{bad:?}");
        }
    }

    #[test]
    fn the_limits_read_their_three_names() {
        let read = |pairs: &[(&str, &str)]| {
            let owned: Vec<(String, String)> = pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
            Limits::from_vars(move |k| owned.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()))
        };
        let d = read(&[]);
        assert_eq!((d.deadline, d.max_answer), (Duration::from_secs(30), 64 << 20));
        assert_eq!(read(&[(env::TIMEOUT_SECS, "7")]).deadline, Duration::from_secs(7));
        assert_eq!(read(&[(env::TIMEOUT_SECS, "7"), (env::TIMEOUT_MS, "250")]).deadline, Duration::from_millis(250));
        assert_eq!(read(&[(env::TIMEOUT_SECS, "7"), (env::TIMEOUT_MS, "x")]).deadline, Duration::from_secs(7));
        assert_eq!(read(&[(env::TIMEOUT_SECS, "0")]).deadline, Duration::ZERO);
        assert_eq!(read(&[(env::MAX_ANSWER_BYTES, "0")]).max_answer, 0);
        for bad in ["-1", "", "1e3", " 5"] {
            assert_eq!(read(&[(env::TIMEOUT_SECS, bad)]).deadline, Duration::from_secs(30), "{bad:?}");
            assert_eq!(read(&[(env::MAX_ANSWER_BYTES, bad)]).max_answer, 64 << 20, "{bad:?}");
        }
    }

    #[test]
    fn an_address_that_does_not_read_says_why() {
        use super::{read_address, NotAnAddress};
        for (url, why) in [
            ("https://h:65536/", NotAnAddress::Port),
            ("https://h:/", NotAnAddress::Port),
            ("https://h:-1/", NotAnAddress::Port),
            ("https://h:x/", NotAnAddress::Port),
            ("https://[::1]:70000/", NotAnAddress::Port),
            ("wss://h/", NotAnAddress::Scheme),
            ("h:443", NotAnAddress::Scheme),
            ("https:///x", NotAnAddress::Host),
            ("https://[nope]/", NotAnAddress::Host),
            ("https://u@h/", NotAnAddress::Shape),
            ("https://h x/", NotAnAddress::Shape),
        ] {
            assert_eq!(read_address(url).err(), Some(why), "{url}");
        }
        assert_eq!(read_address("https://h:65535/").map(|t| t.port), Ok(65535));
        assert_eq!(read_address("https://h:0/").map(|t| t.port), Ok(0));
    }

}
