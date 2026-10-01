//! Endpoints: a JSON-RPC question-and-answer channel, plus recording and replay.
//!
//! A recording is a scan's whole conversation with a node. On replay, a question the recording lacks is an
//! error, never a guess: filling in a "probably empty" answer would let a recording with a hole quietly
//! answer something else. So [`Replay`] returns [`Trouble::NotServed`] and the scan stops.
//!
//! Answers are matched by (method, params), not by order: a scan may ask the same question twice and the
//! order is up to the scan. The key is the method plus the canonical bytes of the params; two entries with
//! the same key and different answers make the recording self-contradictory.

use std::collections::BTreeMap;
use crate::wire::{self, Body, W};
use zikaron::json::{canon_bytes, Value};

/// Where an endpoint cannot go on.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Trouble {
    /// The recording lacks this question.
    NotServed(String),
    /// The node returned an error (it declined).
    Node(String),
    /// The transport broke (cannot connect, cannot read, the answer is not JSON).
    Transport(String),
    /// The recording contradicts itself: two answers to one question.
    Contradiction(String),
}

// Transport sentences, spelled once.
//
// Timeout, answer too long and answer not JSON are said by the real endpoints (`Http` here, `Https` in the
// app) and recognized by the chain client's closed "what the node said" type. Saying and recognizing go
// through the functions below only.

/// This call's total deadline passed (`url` did not finish within `secs` seconds).
pub fn late(url: &str, deadline: std::time::Duration) -> Trouble {
    // Whole seconds as seconds; a millisecond deadline (`ZKA_TIMEOUT_MS`) as fractional seconds. The tail
    // `is_late` recognizes stays the same.
    let said = if deadline.subsec_millis() == 0 { deadline.as_secs().to_string() } else { format!("{:.3}", deadline.as_secs_f64()) };
    Trouble::Transport(format!("{url} 在 {said} 秒里没把话说完"))
}

/// The answer passed its end (`max` bytes).
pub fn overlong(url: &str, max: usize) -> Trouble {
    Trouble::Transport(format!("{url} 的答越过了 {max} 字节的尽头"))
}

/// The answer is not JSON.
pub const NOT_JSON: &str = "答不是 JSON";

/// Whether a sentence says the total deadline passed.
pub fn is_late(said: &str) -> bool {
    said.ends_with("秒里没把话说完")
}

/// Whether a sentence says the answer passed its end.
pub fn is_overlong(said: &str) -> bool {
    said.ends_with("字节的尽头")
}

/// Whether a sentence says the answer is not JSON.
pub fn is_not_json(said: &str) -> bool {
    said == NOT_JSON
}

/// A question-and-answer channel.
pub trait Endpoint {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble>;
    /// A name for people (which endpoint a reading came from).
    fn name(&self) -> String;
}

fn key(method: &str, params: &Value) -> String {
    let mut k = String::from(method);
    k.push(' ');
    k.push_str(&String::from_utf8_lossy(&canon_bytes(params)));
    k
}

/// Recording keys use one rule: params inside the §3 value domain use canonical bytes (as the asking side
/// does); params outside it (which should not occur) use the transport spelling.
fn key_of(method: &str, params: &W) -> String {
    match wire::to_core(params) {
        Some(v) => key(method, &v),
        None => format!("{method} {}", wire::write(params)),
    }
}

/// Replay: one recording answers a whole scan offline.
pub struct Replay {
    who: String,
    answers: BTreeMap<String, Result<W, String>>,
    /// Keys actually asked in this run (recorded but unasked keys are slack, not errors).
    pub asked: Vec<String>,
}

impl Replay {
    /// Build a replay channel from a recording's exchange array.
    pub fn new(who: impl Into<String>, exchanges: &[W]) -> Result<Replay, Trouble> {
        let mut answers: BTreeMap<String, Result<W, String>> = BTreeMap::new();
        for e in exchanges {
            let m = e.member("method").and_then(|x| x.as_str()).unwrap_or("");
            let p = e.member("params").cloned().unwrap_or(W::of(Body::Arr(Vec::new())));
            let k = key_of(m, &p);
            let a = match e.member("error") {
                Some(err) => Err(wire::write(err)),
                None => Ok(e.member("result").cloned().unwrap_or(W::of(Body::Null))),
            };
            if let Some(prev) = answers.get(&k) {
                if prev != &a {
                    return Err(Trouble::Contradiction(k));
                }
            }
            answers.insert(k, a);
        }
        Ok(Replay { who: who.into(), answers, asked: Vec::new() })
    }
}

impl Replay {
    /// The recorded answer to a question. Not recorded is an error, never a guess: the §9.4 completeness rule
    /// says an incomplete read gives no report.
    pub fn answer(&mut self, k: &str) -> Result<W, Trouble> {
        match self.answers.get(k) {
            None => Err(Trouble::NotServed(k.to_string())),
            Some(Ok(v)) => Ok(v.clone()),
            Some(Err(e)) => Err(Trouble::Node(e.clone())),
        }
    }
}

impl Endpoint for Replay {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        let k = key(method, params);
        self.asked.push(k.clone());
        self.answer(&k)
    }
    fn name(&self) -> String {
        self.who.clone()
    }
}

/// The environment names that override the limits below. One name, one home: the anchoring crate, the app's
/// fetches and tests read or set them only through here.
pub mod env {
    pub const TIMEOUT_SECS: &str = "ZKA_TIMEOUT_SECS";
    pub const TIMEOUT_MS: &str = "ZKA_TIMEOUT_MS";
    pub const MAX_ANSWER_BYTES: &str = "ZKA_MAX_ANSWER_BYTES";
}

/// The two bounds of one call: total wall time and answer bytes.
///
/// A per-syscall timeout cannot stop an endpoint that sends one byte every two seconds forever: each byte
/// resets the read timeout. So the deadline counts from the start of the call and the answer has an end. Both
/// are product constants, overridable from the environment; zero means none.
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

/// A real endpoint: our own HTTP/1.1 client (no third-party chain crates).
///
/// Plain HTTP only. TLS would need a third-party crate or cryptography outside the core's `cryptox`; the app
/// carries its own HTTPS endpoint for that.
pub struct Http {
    url: String,
    host: String,
    port: u16,
    path: String,
    next_id: u64,
    limits: Limits,
}

impl Http {
    /// `http://host:port/path`; any other spelling (https included) gives `None`.
    pub fn new(url: &str) -> Option<Http> {
        let rest = url.strip_prefix("http://")?;
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), p.parse().ok()?),
            None => (authority.to_string(), 80u16),
        };
        Some(Http { url: url.to_string(), host, port, path: path.to_string(), next_id: 1, limits: Limits::from_env() })
    }
}

impl Http {
    /// Replace the bounds (when the caller sets a deadline).
    pub fn with_limits(mut self, limits: Limits) -> Http {
        self.limits = limits;
        self
    }
}

impl Endpoint for Http {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        use std::io::{Read, Write};
        let began = std::time::Instant::now();
        let id = self.next_id;
        self.next_id += 1;
        let body = canon_bytes(&Value::Obj(vec![
            ("id".into(), Value::Int(id)),
            ("jsonrpc".into(), Value::Str("2.0".into())),
            ("method".into(), Value::Str(method.into())),
            ("params".into(), params.clone()),
        ]));
        let head = format!(
            "POST {} HTTP/1.1\r\nHost: {}:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.path,
            self.host,
            self.port,
            body.len()
        );
        // A silent endpoint must not hang a scan: connect, read and write each have a bound, and the call has
        // a total deadline.
        let limit = self.limits.deadline;
        let mut sock = match limit.is_zero() {
            true => std::net::TcpStream::connect((self.host.as_str(), self.port))
                .map_err(|e| Trouble::Transport(format!("{}: {e}", self.url)))?,
            false => {
                let addrs: Vec<std::net::SocketAddr> = std::net::ToSocketAddrs::to_socket_addrs(&(self.host.as_str(), self.port))
                    .map_err(|e| Trouble::Transport(format!("{}: {e}", self.url)))?
                    .collect();
                let first = addrs.first().ok_or_else(|| Trouble::Transport(format!("{}: 解不出地址", self.url)))?;
                let sock = std::net::TcpStream::connect_timeout(first, limit)
                    .map_err(|e| Trouble::Transport(format!("{}: {e}", self.url)))?;
                let d = Some(limit);
                sock.set_read_timeout(d).and_then(|_| sock.set_write_timeout(d))
                    .map_err(|e| Trouble::Transport(format!("{}: {e}", self.url)))?;
                sock
            }
        };
        sock.write_all(head.as_bytes()).and_then(|_| sock.write_all(&body)).map_err(|e| Trouble::Transport(e.to_string()))?;
        // Read in chunks; after each, check the total deadline and the answer size. `read_to_end` checks
        // neither: it only knows the last read did not time out, and a trickling endpoint never times out.
        let mut raw = Vec::new();
        let mut chunk = [0u8; 16 << 10];
        loop {
            match sock.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    raw.extend_from_slice(&chunk[..n]);
                    if self.limits.max_answer > 0 && raw.len() > self.limits.max_answer {
                        return Err(overlong(&self.url, self.limits.max_answer));
                    }
                }
                Err(e) if matches!(e.kind(), std::io::ErrorKind::Interrupted) => continue,
                Err(e) => {
                    // Read timeouts come here too: retry until the total deadline, then return by name.
                    if !matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) {
                        return Err(Trouble::Transport(e.to_string()));
                    }
                }
            }
            if !limit.is_zero() && began.elapsed() >= limit {
                return Err(late(&self.url, limit));
            }
        }
        let split = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or_else(|| Trouble::Transport("答里没有头身分界".into()))?;
        let payload = body_of(&raw[..split], &raw[split + 4..])?;
        let v = wire::parse(&payload).ok_or(Trouble::Transport(NOT_JSON.into()))?;
        if let Some(err) = v.member("error") {
            return Err(Trouble::Node(wire::write(err)));
        }
        Ok(v.member("result").cloned().unwrap_or(W::of(Body::Null)))
    }
    fn name(&self) -> String {
        self.url.clone()
    }
}

/// The HTTP body: chunked transfer is unchunked, anything else taken by length.
fn body_of(head: &[u8], rest: &[u8]) -> Result<Vec<u8>, Trouble> {
    let h = String::from_utf8_lossy(head).to_lowercase();
    if !h.contains("transfer-encoding: chunked") {
        return Ok(rest.to_vec());
    }
    let mut out = Vec::new();
    let mut i = 0usize;
    loop {
        // Each step checks bounds first: a truncated chunked answer must not crash the process.
        let tail = rest.get(i..).ok_or_else(|| Trouble::Transport("分块答被截断".into()))?;
        let eol = tail
            .windows(2)
            .position(|w| w == b"\r\n")
            .ok_or_else(|| Trouble::Transport("分块的长度行没收尾".into()))?;
        let n = usize::from_str_radix(String::from_utf8_lossy(&tail[..eol]).trim(), 16)
            .map_err(|_| Trouble::Transport("分块的长度读不出".into()))?;
        i += eol + 2;
        if n == 0 {
            return Ok(out);
        }
        let chunk = rest.get(i..i + n).ok_or_else(|| Trouble::Transport("分块短了".into()))?;
        out.extend_from_slice(chunk);
        // The CRLF after a chunk must be there; without it the answer is broken.
        if rest.get(i + n..i + n + 2) != Some(b"\r\n") {
            return Err(Trouble::Transport("分块答被截断".into()));
        }
        i += n + 2;
    }
}

/// Record every question and answer on a real channel, in the fixture shape.
pub struct Recorder {
    inner: Box<dyn Endpoint>,
    pub exchanges: Vec<W>,
}

impl Recorder {
    pub fn new(inner: Box<dyn Endpoint>) -> Recorder {
        Recorder { inner, exchanges: Vec::new() }
    }
}

impl Endpoint for Recorder {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        let out = self.inner.call(method, params);
        let mut rec = vec![
            ("method".to_string(), W::of(Body::Str(method.to_string()))),
            ("params".to_string(), wire::from_core(params)),
        ];
        match &out {
            Ok(v) => rec.push(("result".to_string(), v.clone())),
            Err(Trouble::Node(e)) => rec.push((
                "error".to_string(),
                wire::parse(e.as_bytes()).unwrap_or(W::of(Body::Str(e.clone()))),
            )),
            Err(_) => {}
        }
        // Transport failures are not recorded: they are not what the node said.
        if !matches!(out, Err(Trouble::Transport(_)) | Err(Trouble::NotServed(_)) | Err(Trouble::Contradiction(_))) {
            self.exchanges.push(W::of(Body::Obj(rec)));
        }
        out
    }
    fn name(&self) -> String {
        self.inner.name()
    }
}
