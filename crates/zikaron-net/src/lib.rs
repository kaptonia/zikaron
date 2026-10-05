//! The one transport for nodes and remote files: an address is read in one place, one HTTP/1.1 exchange is
//! made over TCP (`http`) or TLS (`https`), and the TLS client configuration is built in one place.
//!
//! What a request means (JSON-RPC, a file fetch) is the caller's: this crate carries bytes there and back,
//! within a total deadline and an answer cap, and says by name where it failed (the address, the name, the
//! connection, the handshake, the certificate, the deadline, the cap, the stream). The certificate chain and
//! host name are always verified; there is no way to skip them.

use std::io::{Read, Write};

// ───────────────────────── Limits ─────────────────────────

/// The environment names that override the limits. One name, one home: node queries, remote fetches and
/// tests read or set them only through here.
pub mod env {
    pub const TIMEOUT_SECS: &str = "ZKA_TIMEOUT_SECS";
    pub const TIMEOUT_MS: &str = "ZKA_TIMEOUT_MS";
    pub const MAX_ANSWER_BYTES: &str = "ZKA_MAX_ANSWER_BYTES";
}

/// The two bounds of one exchange: total wall time and answer bytes.
///
/// A per-syscall timeout cannot stop a peer that sends one byte every two seconds forever: each byte resets
/// the read timeout. So the deadline counts from the start of the exchange and the answer has an end. Both
/// are product constants, overridable from the environment; zero means none.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub deadline: std::time::Duration,
    pub max_answer: usize,
}

impl Limits {
    /// Default: 30 seconds, 64 MiB. `ZKA_TIMEOUT_MS` (milliseconds) overrides `ZKA_TIMEOUT_SECS`, for
    /// deadlines shorter than a second.
    pub fn from_env() -> Limits {
        let secs: u64 = std::env::var(env::TIMEOUT_SECS).ok().and_then(|x| x.parse().ok()).unwrap_or(30);
        let deadline = match std::env::var(env::TIMEOUT_MS).ok().and_then(|x| x.parse::<u64>().ok()) {
            Some(ms) => std::time::Duration::from_millis(ms),
            None => std::time::Duration::from_secs(secs),
        };
        let max: usize = std::env::var(env::MAX_ANSWER_BYTES).ok().and_then(|x| x.parse().ok()).unwrap_or(64 << 20);
        Limits { deadline, max_answer: max }
    }
}

// ───────────────────────── Addresses ─────────────────────────

/// The two schemes. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
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

/// An address read: scheme, host, port, path.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Target {
    pub scheme: Scheme,
    pub host: String,
    pub port: u16,
    pub path: String,
}

/// Read an address `scheme://host[:port][/path]`. The scheme and the host are compared without regard to
/// case (RFC 3986 §3.1, §3.2.2: `HTTPS://Node.Example` is `https://node.example`), so they are lowercased
/// here; the path is kept as written. Any other scheme, an empty host, user information or a port that is not
/// a number gives `None`. Every place that asks "is this an address, and which" asks this one function.
pub fn parse(url: &str) -> Option<Target> {
    let (scheme, rest) = url.trim().split_once("://")?;
    let scheme = match scheme.to_ascii_lowercase().as_str() {
        "http" => Scheme::Http,
        "https" => Scheme::Https,
        _ => return None,
    };
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.contains('@') {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h, p.parse().ok()?),
        None => (authority, scheme.default_port()),
    };
    if host.is_empty() {
        return None;
    }
    Some(Target { scheme, host: host.to_ascii_lowercase(), port, path: path.to_string() })
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

    /// The normalized address (lowercase scheme and host, default port left out).
    pub fn url(&self) -> String {
        format!("{}://{}{}", self.scheme.as_str(), self.authority(), self.path)
    }
}

// ───────────────────────── Failures ─────────────────────────

/// Where an exchange failed. Closed; the certificate layer is kept apart from the reachability layers.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Fail {
    /// The host name did not resolve, or is not a verifiable server name.
    Name(String),
    /// TCP could not connect.
    Connect(String),
    /// Other TLS handshake failures (protocol mismatch, the peer disconnected, timeout).
    Handshake(String),
    /// The server certificate failed verification (no chain to a root, expired, wrong host name,
    /// self-signed, none presented).
    Certificate(String),
    /// The exchange's total deadline passed.
    Late(std::time::Duration),
    /// The answer passed its cap (bytes).
    Overlong(usize),
    /// The stream broke, or the answer is not an HTTP answer.
    Stream(String),
}

// ───────────────────────── TLS ─────────────────────────

/// Extra roots for tests only. A test driver adds a test root certificate here when it starts a local https
/// stub in its own process; the product never calls it. It can be added only once, before the first
/// handshake (the configuration is built once); adding it loosens no check: the certificate chain and host
/// name are still always verified.
static DRIVE_ROOTS: std::sync::OnceLock<Vec<Vec<u8>>> = std::sync::OnceLock::new();

/// A test driver adds a test root certificate (DER). Returns whether it was added (`false` when the
/// configuration is already built or one was already added).
pub fn drive_trust_root(der: &[u8]) -> bool {
    DRIVE_ROOTS.set(vec![der.to_vec()]).is_ok()
}

/// The client configuration, the one place it is built and built once: `ring` cryptography, safe default
/// protocol versions, the `webpki-roots` root table, no client certificate.
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

/// Read the TLS error inside an IO error: the certificate family goes to `Certificate`, everything else to
/// `Handshake`.
///
/// The certificate family is more than `InvalidCertificate`: a peer presenting no certificate
/// (`NoCertificatesPresented`) and an unverifiable revocation list (`InvalidCertRevocationList`) both mean
/// "the other side has no identity that verified". Filed under `Handshake`, the person would read it as an
/// unstable network, pointing the wrong way.
fn tls_fail(e: &std::io::Error) -> Fail {
    match e.get_ref().and_then(|x| x.downcast_ref::<rustls::Error>()) {
        Some(rustls::Error::InvalidCertificate(c)) => Fail::Certificate(format!("{c:?}")),
        Some(rustls::Error::NoCertificatesPresented) => Fail::Certificate("NoCertificatesPresented".into()),
        Some(rustls::Error::InvalidCertRevocationList(c)) => Fail::Certificate(format!("{c:?}")),
        Some(other) => Fail::Handshake(other.to_string()),
        None => Fail::Handshake(e.to_string()),
    }
}

/// Handshake TLS over a connected TCP stream. The handshake completes here before the stream is used: a
/// failed certificate is named at once. `deadline` is the exchange's total deadline, checked every round.
fn tls_connect(
    host: &str,
    mut tcp: std::net::TcpStream,
    deadline: Option<std::time::Instant>,
) -> Result<rustls::StreamOwned<rustls::ClientConnection, std::net::TcpStream>, Fail> {
    let name = rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|e| Fail::Name(format!("{host}: {e}")))?;
    let mut conn = rustls::ClientConnection::new(tls_config()?, name).map_err(|e| Fail::Handshake(e.to_string()))?;
    while conn.is_handshaking() {
        if let Some(d) = deadline {
            if std::time::Instant::now() >= d {
                return Err(Fail::Handshake("handshake did not finish within the deadline".to_string()));
            }
        }
        match conn.complete_io(&mut tcp) {
            Ok((0, 0)) if conn.is_handshaking() => return Err(Fail::Handshake("the peer closed the connection during the handshake".to_string())),
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            // The TCP read or write timeout expired: back to the loop head, where the total deadline is asked.
            Err(e) if deadline.is_some() && matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => return Err(tls_fail(&e)),
        }
    }
    Ok(rustls::StreamOwned::new(conn, tcp))
}

// ───────────────────────── The exchange ─────────────────────────

/// One stream: plain TCP or TLS over it.
enum Stream {
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

    /// A failure on the stream: on a TLS stream the peer's certificate alert may arrive on the first read
    /// after the handshake, and it is said as the certificate layer.
    fn fail(&self, e: std::io::Error) -> Fail {
        match self {
            Stream::Tls(_) if e.get_ref().and_then(|x| x.downcast_ref::<rustls::Error>()).is_some() => tls_fail(&e),
            _ => Fail::Stream(e.to_string()),
        }
    }
}

/// The one exchange: resolve, connect TCP, handshake TLS for `https` (certificate and host name always
/// verified), write `request`, read the whole answer in chunks, within `limits`'s deadline and cap. Node
/// queries (POST) and remote fetches (GET) both come here, so the scheme, TLS, timeouts and the cap behave
/// the same for every caller.
pub fn exchange(t: &Target, request: &[u8], limits: &Limits) -> Result<Vec<u8>, Fail> {
    let began = std::time::Instant::now();
    let limit = limits.deadline;
    let deadline = (!limit.is_zero()).then(|| began + limit);
    let addrs: Vec<std::net::SocketAddr> = std::net::ToSocketAddrs::to_socket_addrs(&(t.host.as_str(), t.port))
        .map_err(|e| Fail::Name(format!("{}: {e}", t.host)))?
        .collect();
    let first = addrs.first().ok_or_else(|| Fail::Name(t.host.clone()))?;
    let tcp = match limit.is_zero() {
        true => std::net::TcpStream::connect(first),
        false => std::net::TcpStream::connect_timeout(first, limit),
    }
    .map_err(|e| Fail::Connect(e.to_string()))?;
    if !limit.is_zero() {
        let d = Some(limit);
        tcp.set_read_timeout(d).and_then(|_| tcp.set_write_timeout(d)).map_err(|e| Fail::Connect(e.to_string()))?;
    }
    let mut stream = match t.scheme {
        Scheme::Http => Stream::Plain(tcp),
        Scheme::Https => Stream::Tls(Box::new(tls_connect(&t.host, tcp, deadline)?)),
    };
    if let Err(e) = stream.send(request) {
        return Err(stream.fail(e));
    }
    // Read in chunks; after each, check the total deadline, the answer size and whether the answer is
    // complete. `read_to_end` checks none of them: a trickling peer never times out.
    let mut raw = Vec::new();
    let mut chunk = [0u8; 16 << 10];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                raw.extend_from_slice(&chunk[..n]);
                if limits.max_answer > 0 && raw.len() > limits.max_answer {
                    return Err(Fail::Overlong(limits.max_answer));
                }
                if complete(&raw) {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            // A peer that disconnects without a close alert: what arrived is judged as it is.
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            // Read timeouts come here: retry until the total deadline, then fail by name.
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(e) => return Err(stream.fail(e)),
        }
        if !limit.is_zero() && began.elapsed() >= limit {
            return Err(Fail::Late(limit));
        }
    }
    Ok(raw)
}

/// One HTTP answer: status, `Location` (when present) and body.
pub struct Answer {
    pub status: u16,
    pub location: Option<String>,
    pub body: Vec<u8>,
}

/// POST `body` as JSON to the target; the whole answer read.
pub fn post_json(t: &Target, body: &[u8], limits: &Limits) -> Result<Answer, Fail> {
    let head = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        t.path,
        t.authority(),
        body.len()
    );
    let mut request = head.into_bytes();
    request.extend_from_slice(body);
    answer_of(&exchange(t, &request, limits)?)
}

/// GET the target; the whole answer read. Whether to follow a redirect is the caller's decision.
pub fn get(t: &Target, limits: &Limits) -> Result<Answer, Fail> {
    let head = format!("GET {} HTTP/1.1\r\nHost: {}\r\nAccept: */*\r\nConnection: close\r\n\r\n", t.path, t.authority());
    answer_of(&exchange(t, head.as_bytes(), limits)?)
}

/// Split an HTTP answer into status, `Location` and body.
fn answer_of(raw: &[u8]) -> Result<Answer, Fail> {
    let split = raw
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .ok_or_else(|| Fail::Stream("the answer has no end of head".into()))?;
    let text = String::from_utf8_lossy(&raw[..split]).to_string();
    let status: u16 = text
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .ok_or_else(|| Fail::Stream("the answer's status line does not read".into()))?;
    let location = text.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case("location").then(|| v.trim().to_string())
    });
    let body = body_of(&raw[..split], &raw[split + 4..])?;
    Ok(Answer { status, location, body })
}

/// Whether an HTTP answer is complete: the head/body boundary arrived, and the body has its
/// `Content-Length` or the final chunk arrived. With neither, read until the peer closes.
fn complete(raw: &[u8]) -> bool {
    let Some(split) = raw.windows(4).position(|w| w == b"\r\n\r\n") else { return false };
    let head = String::from_utf8_lossy(&raw[..split]).to_ascii_lowercase();
    let body = &raw[split + 4..];
    if head.contains("transfer-encoding: chunked") {
        return body_of(&raw[..split], body).is_ok();
    }
    head.lines()
        .find_map(|l| l.strip_prefix("content-length:").and_then(|n| n.trim().parse::<usize>().ok()))
        .map(|n| body.len() >= n)
        .unwrap_or(false)
}

/// The HTTP body: chunked transfer is unchunked, a `Content-Length` body is taken by its length, anything
/// else as received. Each step checks its bounds first: a truncated answer is named, never a crash.
fn body_of(head: &[u8], rest: &[u8]) -> Result<Vec<u8>, Fail> {
    let h = String::from_utf8_lossy(head).to_ascii_lowercase();
    if !h.contains("transfer-encoding: chunked") {
        let len = h.lines().find_map(|l| l.strip_prefix("content-length:").and_then(|n| n.trim().parse::<usize>().ok()));
        return match len {
            Some(n) => rest.get(..n).map(|b| b.to_vec()).ok_or_else(|| Fail::Stream("the body is shorter than its length".into())),
            None => Ok(rest.to_vec()),
        };
    }
    let cut = || Fail::Stream("the chunked answer is cut short".into());
    let mut out = Vec::new();
    let mut i = 0usize;
    loop {
        let tail = rest.get(i..).ok_or_else(cut)?;
        let eol = tail.windows(2).position(|w| w == b"\r\n").ok_or_else(cut)?;
        let size = String::from_utf8_lossy(&tail[..eol]);
        let size = size.split(';').next().unwrap_or("").trim().to_string();
        let n = usize::from_str_radix(&size, 16).map_err(|_| Fail::Stream("a chunk length does not read".into()))?;
        i += eol + 2;
        if n == 0 {
            return Ok(out);
        }
        let chunk = rest.get(i..i + n).ok_or_else(cut)?;
        out.extend_from_slice(chunk);
        if rest.get(i + n..i + n + 2) != Some(b"\r\n") {
            return Err(cut());
        }
        i += n + 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn a_cut_answer_is_named() {
        assert!(body_of(b"transfer-encoding: chunked", b"5\r\nab").is_err());
        assert!(body_of(b"content-length: 9", b"abc").is_err());
        assert_eq!(body_of(b"content-length: 3", b"abcdef").ok(), Some(b"abc".to_vec()));
        assert_eq!(body_of(b"transfer-encoding: chunked", b"3\r\nabc\r\n0\r\n\r\n").ok(), Some(b"abc".to_vec()));
    }
}
