//! The single transport for nodes and remote files: address parsing, one HTTP/1.1 exchange over TCP (`http`)
//! or TLS (`https`), and the TLS client configuration.
//!
//! Request semantics (JSON-RPC, a file fetch) belong to the caller: this crate carries bytes within one
//! deadline and a response size cap, and names where it failed (address, name resolution, connection,
//! handshake, certificate, deadline, cap, stream, shutdown). The certificate chain and host name are always
//! verified; there is no way to skip them.
//!
//! Each exchange has one deadline, fixed when it starts: resolving, connecting, the handshake, writing and
//! every read get only the time left, and running out anywhere is a deadline failure. No wait for the peer (to
//! resolve, connect, answer or finish a handshake or tunnel) lasts longer than one [`SLICE`] before it looks
//! whether the transport is shutting down, so shutting down cuts an exchange in flight within a slice on every
//! system, without relying on the system waking a blocked read. A write the peer does not take is bounded by
//! the time left only, not sliced (a sliced write would break a request the peer is slow to take); shutting
//! down shuts its socket, and that is what ends it. Every resolved address is tried (`dial`). A connection
//! whose response ended cleanly and whose peer left it open is kept for the next request to the same place
//! (`keep`); an idempotent request ([`Ask::Read`]) is retried once on a new connection if the connection breaks
//! before any response byte arrives.

use std::io::{Read, Write};
use std::time::{Duration, Instant};

mod answer;
mod dial;
mod keep;
mod route;

pub use dial::{CONNECT_CAP, STAGGER};
pub use keep::{close_down, cuts, open_up, IDLE_KEEP, PER_PLACE};
pub use route::{excepted, is_loopback, proxy_of, read_choice, read_system_with, set_choice, way_for, way_shown, way_under, Choice, Kind, NotAProxy, Proxy, Reading, SystemProxies, Way, SYSTEM_FRESH, SYSTEM_READ_CAP};

// ───────────────────────── The request head ─────────────────────────

/// The `User-Agent` every request carries: the product name only. No version, OS or machine, so a node learns
/// nothing the request's shape does not already reveal, while gateways that reject requests without the header
/// accept these.
pub const USER_AGENT: &str = "ZIKARON";

/// Builds every request head: the request line, `Host`, `User-Agent`, the caller's own lines (each ending in
/// CRLF), then the blank line. Every request this crate sends (POST, GET, a proxy CONNECT) uses it, so none
/// goes out without `User-Agent`.
pub(crate) fn request_head(method: &str, target: &str, host: &str, lines: &str) -> String {
    format!("{method} {target} HTTP/1.1\r\nHost: {host}\r\nUser-Agent: {USER_AGENT}\r\n{lines}\r\n")
}

// ───────────────────────── Limits ─────────────────────────

/// The longest any wait in an exchange (resolving, connecting, a socket read) goes before it looks again
/// whether the transport is shutting down ([`close_down`]): the most a cut waits. The deadline is kept to the
/// instant regardless; a slice only bounds how long each wait sleeps.
pub const SLICE: Duration = Duration::from_millis(250);

/// Environment variables that override the limits; node queries, remote fetches and tests use only these
/// names.
pub mod env {
    pub const TIMEOUT_SECS: &str = "ZKA_TIMEOUT_SECS";
    pub const TIMEOUT_MS: &str = "ZKA_TIMEOUT_MS";
    pub const MAX_ANSWER_BYTES: &str = "ZKA_MAX_ANSWER_BYTES";
}

/// The two bounds of one exchange: total wall time and response bytes.
///
/// A per-syscall timeout cannot stop a peer that trickles one byte every two seconds forever (each byte resets
/// the read timeout), so the deadline counts from the start of the exchange and the response size is capped.
/// Both have defaults overridable from the environment; zero means no limit.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub deadline: std::time::Duration,
    pub max_answer: usize,
}

impl Limits {
    /// Default: 30 seconds, 64 MiB. `ZKA_TIMEOUT_MS` (milliseconds) overrides `ZKA_TIMEOUT_SECS`, for
    /// deadlines shorter than a second.
    pub fn from_env() -> Limits {
        Limits::from_vars(|k| std::env::var(k).ok())
    }

    /// [`Limits::from_env`] over any variable source; a value that is not a whole number counts as absent.
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

/// One exchange's deadline, fixed when it starts.
#[derive(Clone, Copy)]
pub(crate) struct Clock {
    limit: Duration,
    deadline: Option<Instant>,
}

impl Clock {
    fn start(limit: Duration) -> Clock {
        Clock { limit, deadline: (!limit.is_zero()).then(|| Instant::now() + limit) }
    }

    /// The time left (`None`: no deadline); past the deadline, a deadline failure.
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

    /// This exchange's deadline failure.
    pub(crate) fn late(&self) -> Fail {
        Fail::Late(self.limit)
    }

    /// How long this exchange may wait for the system proxy settings: [`route::SYSTEM_READ_CAP`], never past
    /// the deadline.
    fn system_wait(&self) -> Result<Duration, Fail> {
        Ok(self.left()?.map(|l| l.min(route::SYSTEM_READ_CAP)).unwrap_or(route::SYSTEM_READ_CAP))
    }

    /// Limit the socket's writes to the time left and each read to one [`SLICE`] within it, so every read loop
    /// looks at shutdown after a slice at most (with no deadline, too) and still runs out exactly at the
    /// deadline.
    ///
    /// On some systems a socket closed by shutdown refuses its timeouts: a refusal found with the flag
    /// raised is the cut ([`keep::cut`]), never a stream failure.
    fn bound(&self, tcp: &std::net::TcpStream) -> Result<(), Fail> {
        let left = self.left()?;
        let read = Some(left.map_or(SLICE, |l| l.min(SLICE)));
        tcp.set_read_timeout(read).and_then(|_| tcp.set_write_timeout(left)).map_err(|e| if keep::closing() { keep::cut() } else { Fail::Stream(e.to_string()) })
    }
}

// ───────────────────────── Addresses ─────────────────────────

/// The two supported schemes. Closed set.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Scheme {
    Http,
    Https,
}

impl Scheme {
    /// The scheme as written in a normalized address.
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

/// A parsed address: scheme, host, port, path. An IPv6 literal host keeps its brackets (`[::1]`), as sent in
/// `Host`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Target {
    pub scheme: Scheme,
    pub host: String,
    pub port: u16,
    pub path: String,
}

/// Whether text contains a control character or whitespace (never allowed in a host or a request-line path).
fn blank_or_control(s: &str) -> bool {
    s.chars().any(|c| c.is_control() || c.is_whitespace())
}

/// Parse an address `scheme://host[:port][/path]`. Scheme and host are case-insensitive (RFC 3986 §3.1,
/// §3.2.2: `HTTPS://Node.Example` is `https://node.example`), so they are lowercased; the path is kept as
/// written. A bracketed IPv6 literal (`[::1]`, `[::1]:8545`) is the whole host. Any other scheme, an empty
/// host, userinfo, a non-numeric port, a bracket that is not an IPv6 address, or a control character or
/// whitespace in the host or path gives `None`. This is the only address parser.
pub fn parse(url: &str) -> Option<Target> {
    read_address(url).ok()
}

/// An address safe to display: scheme, host and port only ([`Target::sayable`]), never the path, query or
/// userinfo, because a node's API key often lives there (`/v3/<key>`, `?apikey=…`) and every message can reach
/// a log. Text that does not parse is reported by its length only. The transport, the anchoring crate and the
/// app name nodes only through this.
pub fn sayable(url: &str) -> String {
    match parse(url) {
        Some(t) => t.sayable(),
        None => format!("({} bytes)", url.trim().len()),
    }
}

/// [`Target::place_id`] of an address; unparsable text is its own key (trimmed), so two unparsable spellings
/// are the same place only when the text is identical.
pub fn place_key(url: &str) -> String {
    parse(url).map(|t| t.place_id()).unwrap_or_else(|| url.trim().to_string())
}

/// Why an address does not parse; tells the user what to fix. Closed set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotAnAddress {
    /// No `scheme://`, or a scheme other than `http` and `https`.
    Scheme,
    /// The port is empty or not a number within 16 bits.
    Port,
    /// The host is empty, or bracketed but not an IPv6 address (or not closed).
    Host,
    /// Userinfo, or a control character or whitespace in the host or path.
    Shape,
}

/// [`parse`] with the reason for failure (checked in order: scheme, shape, host, port).
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
    /// The authority as sent in `Host`: the port is left out when it is the scheme's default.
    pub fn authority(&self) -> String {
        if self.port == self.scheme.default_port() {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    /// The displayable address ([`sayable`]): scheme, host and port, the default port omitted.
    pub fn sayable(&self) -> String {
        format!("{}://{}", self.scheme.as_str(), self.authority())
    }

    /// The normalized address (lowercase scheme and host, default port left out).
    pub fn url(&self) -> String {
        format!("{}://{}{}", self.scheme.as_str(), self.authority(), self.path)
    }

    /// A canonical identity string for this address: scheme, host (an IPv6 literal in canonical form:
    /// `[0:0:0:0:0:0:0:1]` is `[::1]`), port (always written, even the default) and path as written. Case,
    /// an explicit default port and IPv6 spelling do not make a different place; a different path, port,
    /// scheme or host name does.
    pub fn place_id(&self) -> String {
        let host = match self.dial_host().parse::<std::net::Ipv6Addr>() {
            Ok(v6) if self.host.starts_with('[') => format!("[{v6}]"),
            _ => self.host.clone(),
        };
        format!("{}://{}:{}{}", self.scheme.as_str(), host, self.port, self.path)
    }

    /// The host for connecting and certificate verification: an IPv6 literal without brackets.
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

// ───────────────────────── Failures ─────────────────────────

/// Where an exchange failed. Closed set; certificate failures are kept separate from reachability failures.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Fail {
    /// The host name did not resolve, or is not a verifiable server name.
    Name(String),
    /// TCP could not connect (every address tried is named).
    Connect(String),
    /// Other TLS handshake failures (protocol mismatch, the peer disconnected).
    Handshake(String),
    /// The server certificate failed verification (no chain to a root, expired, wrong host name,
    /// self-signed, none presented).
    Certificate(String),
    /// The exchange's deadline passed, at any step.
    Late(std::time::Duration),
    /// The response exceeded its size cap (bytes).
    Overlong(usize),
    /// The stream broke, or the response is not valid HTTP.
    Stream(String),
    /// The transport is shutting down (the app is quitting): the exchange was cut or never began.
    Closed,
}

// ───────────────────────── TLS ─────────────────────────

/// Extra trust roots, for tests only: a test harness adds a test root certificate when it starts a local
/// https stub in its own process; production never does. It can be added once, before the first handshake
/// (the configuration is built once), and loosens no check: chain and host name are still verified.
static DRIVE_ROOTS: std::sync::OnceLock<Vec<Vec<u8>>> = std::sync::OnceLock::new();

/// Add a test root certificate (DER), for test harnesses. Returns whether it was added (`false` if the
/// configuration is already built or one was already added).
pub fn drive_trust_root(der: &[u8]) -> bool {
    DRIVE_ROOTS.set(vec![der.to_vec()]).is_ok()
}

/// The TLS client configuration, built once: `ring` cryptography, safe default protocol versions, the
/// `webpki-roots` root table, no client certificate.
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

/// Classify the TLS error inside an I/O error: certificate errors go to `Certificate`, everything else to
/// `Handshake`.
///
/// Certificate errors include more than `InvalidCertificate`: no certificate presented
/// (`NoCertificatesPresented`) and an unverifiable revocation list (`InvalidCertRevocationList`) both mean the
/// peer's identity did not verify. Reported as `Handshake`, users would mistake them for network trouble.
fn tls_fail(e: &std::io::Error) -> Fail {
    match e.get_ref().and_then(|x| x.downcast_ref::<rustls::Error>()) {
        Some(rustls::Error::InvalidCertificate(c)) => Fail::Certificate(format!("{c:?}")),
        Some(rustls::Error::NoCertificatesPresented) => Fail::Certificate("NoCertificatesPresented".into()),
        Some(rustls::Error::InvalidCertRevocationList(c)) => Fail::Certificate(format!("{c:?}")),
        Some(other) => Fail::Handshake(other.to_string()),
        None => Fail::Handshake(e.to_string()),
    }
}

/// Perform the TLS handshake over a connected TCP stream, completing it before the stream is used so a bad
/// certificate is reported at once. Each round gets only the time left; running out is a deadline failure,
/// not a handshake failure.
fn tls_connect(
    host: &str,
    mut tcp: std::net::TcpStream,
    clock: &Clock,
) -> Result<rustls::StreamOwned<rustls::ClientConnection, std::net::TcpStream>, Fail> {
    let name = rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|e| Fail::Name(format!("{host}: {e}")))?;
    let mut conn = rustls::ClientConnection::new(tls_config()?, name).map_err(|e| Fail::Handshake(e.to_string()))?;
    while conn.is_handshaking() {
        if keep::closing() {
            return Err(keep::cut());
        }
        clock.bound(&tcp)?;
        let round = conn.complete_io(&mut tcp);
        // Cut by shutdown during the handshake: reported as closed, not as a handshake failure.
        if keep::closing() {
            return Err(keep::cut());
        }
        match round {
            Ok((0, 0)) if conn.is_handshaking() => return Err(Fail::Handshake("the peer closed the connection during the handshake".to_string())),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            // A slice passed: back to the loop head, where shutdown and the deadline are checked.
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => return Err(tls_fail(&e)),
        }
    }
    Ok(rustls::StreamOwned::new(conn, tcp))
}

// ───────────────────────── The exchange ─────────────────────────

/// One stream: plain TCP or TLS over it.
pub(crate) enum Stream {
    Plain(std::net::TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, std::net::TcpStream>>),
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
            Stream::Tls(s) => s.write_all(bytes).and_then(|_| s.flush()),
        }
    }

    /// The TCP socket under the stream.
    pub(crate) fn tcp(&self) -> &std::net::TcpStream {
        match self {
            Stream::Plain(s) => s,
            Stream::Tls(s) => &s.sock,
        }
    }

    /// A stream failure: on TLS, the peer's certificate alert may arrive on the first read after the
    /// handshake, and is reported as a certificate failure.
    fn fail(&self, e: std::io::Error) -> Fail {
        match self {
            Stream::Tls(_) if e.get_ref().and_then(|x| x.downcast_ref::<rustls::Error>()).is_some() => tls_fail(&e),
            _ => Fail::Stream(e.to_string()),
        }
    }
}

/// How a request may be sent. Closed set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ask {
    /// An idempotent request (a chain read, a file fetch): it may reuse a kept connection, and is retried once
    /// on a new connection if the connection breaks before any response byte arrives.
    Read,
    /// A request sent exactly once (a broadcast): always on a new connection, closed afterwards, never
    /// retried.
    Once,
}

/// Open a new connection the given way: direct (resolve, try every address) or through a proxy tunnel; then,
/// for `https`, the TLS handshake with the target (certificate and host name always verified end to end), all
/// within the time left.
fn open(t: &Target, way: &Way, clock: &Clock) -> Result<keep::Live, Fail> {
    if keep::closing() {
        return Err(Fail::Closed);
    }
    // Tracked before the handshake, so shutdown cuts a handshake in flight like any exchange.
    let (tcp, ticket) = match way {
        Way::Direct => {
            let tcp = dial::connect(t, clock)?;
            let ticket = keep::ticket(&tcp)?;
            (tcp, ticket)
        }
        Way::Through(p) => route::tunnel(t, p, clock)?,
    };
    let stream = match t.scheme {
        Scheme::Http => Stream::Plain(tcp),
        Scheme::Https => Stream::Tls(Box::new(tls_connect(t.dial_host(), tcp, clock)?)),
    };
    Ok(keep::live(keep::Conn { stream, place: t.place(way) }, ticket))
}

/// How one round on one connection ended without a response.
enum Round {
    /// The connection broke before any response byte arrived.
    Broke(String),
    Failed(Fail),
}

/// Whether an I/O error means the connection broke (as opposed to a TLS alert or a timeout).
fn breaks(e: &std::io::Error) -> bool {
    use std::io::ErrorKind::*;
    matches!(e.kind(), ConnectionReset | ConnectionAborted | BrokenPipe | NotConnected | UnexpectedEof)
}

/// One round: write the request and read one whole response within the time left and the size cap (which
/// counts every byte read, head included).
fn round(stream: &mut Stream, request: &[u8], limits: &Limits, clock: &Clock) -> Result<answer::Reader, Round> {
    let closed = || Round::Failed(keep::cut());
    // A socket that rejects its timeout is no longer connected (macOS does this, under various error kinds,
    // for a kept connection the peer has since reset): before any response byte, treat it as broken.
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
            // A peer that disconnects without a TLS close alert: judge what arrived as is.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return peer_closed(reader),
            // A slice passed with nothing read: shutdown was checked above, the deadline is below.
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) if breaks(&e) && reader.raw().is_empty() => return Err(Round::Broke(e.to_string())),
            Err(e) => return Err(Round::Failed(stream.fail(e))),
        }
        // A peer trickling bytes is also cut at the deadline.
        clock.left().map_err(Round::Failed)?;
    }
}

/// The peer closed: before any byte, the connection broke; otherwise judge the response as it arrived.
fn peer_closed(mut reader: answer::Reader) -> Result<answer::Reader, Round> {
    if reader.raw().is_empty() {
        return Err(Round::Broke("the connection closed before the answer began".into()));
    }
    reader.closed().map_err(Round::Failed)?;
    Ok(reader)
}

/// Send one request to the target and read its whole response, per `how` (see [`Ask`]), within one deadline.
fn carry(t: &Target, request: &[u8], limits: &Limits, how: Ask) -> Result<answer::Reader, Fail> {
    let clock = Clock::start(limits.deadline);
    // The route is chosen once per exchange, from the current setting and the system settings read now
    // (within the deadline).
    let way = route::way_for(t, clock.system_wait()?).way;
    let mut again = false;
    loop {
        let mut live = match how {
            // A retry always uses a new connection, never another kept one.
            Ask::Read if again => open(t, &way, &clock)?,
            Ask::Read => match keep::take(&t.place(&way)) {
                Some(l) => l,
                None => open(t, &way, &clock)?,
            },
            Ask::Once => open(t, &way, &clock)?,
        };
        match round(&mut live.conn.stream, request, limits, &clock) {
            Ok(reader) => {
                if how == Ask::Read && reader.keeps() {
                    keep::put(live);
                }
                return Ok(reader);
            }
            // Broken before the response began: an idempotent request is retried once on a new connection,
            // within the same deadline.
            Err(Round::Broke(_)) if how == Ask::Read && !again && !keep::closing() => again = true,
            Err(Round::Broke(why)) => return Err(Fail::Stream(why)),
            Err(Round::Failed(f)) => return Err(f),
        }
    }
}

/// A raw exchange: a new connection, `request` written as given, and the whole response returned as bytes,
/// within `limits`; the connection is not kept and nothing is retried.
pub fn exchange(t: &Target, request: &[u8], limits: &Limits) -> Result<Vec<u8>, Fail> {
    let clock = Clock::start(limits.deadline);
    let way = route::way_for(t, clock.system_wait()?).way;
    let mut live = open(t, &way, &clock)?;
    match round(&mut live.conn.stream, request, limits, &clock) {
        Ok(reader) => Ok(reader.raw().to_vec()),
        Err(Round::Broke(why)) => Err(Fail::Stream(why)),
        Err(Round::Failed(f)) => Err(f),
    }
}

/// One HTTP response: status, `Location` (when present) and body.
pub struct Answer {
    pub status: u16,
    pub location: Option<String>,
    pub body: Vec<u8>,
}

fn answer_of(reader: answer::Reader) -> Result<Answer, Fail> {
    let (head, body) = reader.answer().ok_or_else(|| Fail::Stream("the answer is not whole".into()))?;
    Ok(Answer { status: head.status, location: head.location, body })
}

/// The header line asking to close the connection after the response (for [`Ask::Once`]).
fn closing_line(how: Ask) -> &'static str {
    match how {
        Ask::Read => "",
        Ask::Once => "Connection: close\r\n",
    }
}

/// POST `body` as JSON to the target, sent per `how`; returns the whole response.
pub fn post_json(t: &Target, body: &[u8], limits: &Limits, how: Ask) -> Result<Answer, Fail> {
    let head = request_head(
        "POST",
        &t.path,
        &t.authority(),
        &format!("Content-Type: application/json\r\nContent-Length: {}\r\n{}", body.len(), closing_line(how)),
    );
    let mut request = head.into_bytes();
    request.extend_from_slice(body);
    answer_of(carry(t, &request, limits, how)?)
}

/// GET the target (idempotent); returns the whole response. Following redirects is up to the caller.
pub fn get(t: &Target, limits: &Limits) -> Result<Answer, Fail> {
    let head = request_head("GET", &t.path, &t.authority(), &format!("Accept: */*\r\n{}", closing_line(Ask::Read)));
    answer_of(carry(t, head.as_bytes(), limits, Ask::Read)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A local peer that takes every connection, reads once (the request, or a handshake's first flight), writes
    /// `then` and stays silent; counts the connections whose first read arrived.
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

    /// Run `ask` on its own thread; once the peer has heard it, raise the shutdown flag alone (no socket shut,
    /// so no system wakes the blocked read) and return what the exchange said and how long the cut took.
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

    /// A socket closed by shutdown may refuse its timeouts (some systems do): with the flag raised that is
    /// the cut, counted, never a stream failure.
    #[test]
    fn a_timeout_refused_while_shutting_down_is_the_cut() {
        let _turn = keep::turn();
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

    /// The cut rests on the flag, not on the system waking a blocked read: with only the flag raised, an
    /// exchange waiting after its request, one with half an answer in, one with no deadline at all, a handshake
    /// and a proxy tunnel each end as closed within a few slices, far inside their 20 s deadline.
    #[test]
    fn a_cut_rests_on_the_flag_and_lands_within_a_slice() {
        let _turn = keep::turn();
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
        let (said, took) = cut_by_the_flag_alone(&heard, move || open(&t, &Way::Direct, &Clock::start(limits.deadline)).err());
        assert_eq!(said, Some(Fail::Closed), "a handshake in flight");
        assert!(took < SLICE * 4, "a handshake in flight: {took:?}");
        let (port, heard) = silent_peer(b"");
        let p = Proxy { kind: Kind::Http, host: "127.0.0.1".into(), port };
        let t = parse("http://node.invalid:8545/").expect("address");
        let (said, took) = cut_by_the_flag_alone(&heard, move || route::tunnel(&t, &p, &Clock::start(limits.deadline)).err());
        assert_eq!(said, Some(Fail::Closed), "a tunnel in flight");
        assert!(took < SLICE * 4, "a tunnel in flight: {took:?}");
    }

    /// A deadline shorter than a slice runs out at the deadline, not at the slice: reads are sliced, the
    /// deadline is not rounded up to them.
    #[test]
    fn the_deadline_is_kept_to_the_instant_not_to_a_slice() {
        let _turn = keep::turn();
        let (port, heard) = silent_peer(b"");
        let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
        let limits = Limits { deadline: Duration::from_millis(60), max_answer: 0 };
        let began = Instant::now();
        assert_eq!(post_json(&t, b"{}", &limits, Ask::Once).err(), Some(Fail::Late(Duration::from_millis(60))));
        let took = began.elapsed();
        assert!(heard.load(Ordering::SeqCst) == 1, "the request went out");
        assert!(took >= Duration::from_millis(60) && took < SLICE, "late at the deadline: {took:?}");
    }

    /// A cut and the deadline in the same slice: the exchange ends as one of the two, named, within that slice.
    #[test]
    fn a_cut_and_the_deadline_in_one_slice_end_as_either() {
        let _turn = keep::turn();
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

    /// A displayed address is scheme, host and port only: a key in the path, query or fragment never appears;
    /// the default port is omitted as in `Host`; an IPv6 literal keeps its brackets; text that does not parse
    /// (including userinfo) is reported by length only.
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

    /// A bracketed IPv6 literal is the whole host, with or without a port; it keeps its brackets in `Host` and
    /// drops them for connecting. A bracket that is not an IPv6 address does not parse.
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

    /// Assorted forms: empty port refused, outer whitespace trimmed, a `+` on the port accepted, a comma-joined
    /// list refused, an upper-case scheme accepted, a query without a path kept in the host; a control
    /// character or whitespace in the host or path is refused.
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

    /// Limit variables: milliseconds override seconds, zero means none, a non-integer counts as absent.
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

    /// An unparsable address reports why: an empty or out-of-range port is `Port`; scheme, host and shape each
    /// have their own reason. Ports 0 and 65535 parse.
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
