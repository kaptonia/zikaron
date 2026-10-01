//! Chain reads. No chain decision is made here: the endpoint rule belongs to the anchoring crate's
//! `endpoints`, transport and JSON-RPC to its `rpc`, scanning and the three verdicts to its `scan`, the
//! report to the core's `audit`.
//!
//! This layer arranges endpoints and turns a call's answers into a reading (balance, chain time).
//!
//! Load-bearing reads are green only when several endpoints agree, and a single source is flagged: every
//! reading carries `sources` and `single_source`, so a balance asked from one endpoint and one two endpoints
//! agree on look different on screen.

use crate::fault::{Fault, Known};
use crate::key::Address;
use zikaron::json::Value;
use zikaron_anchor::endpoints;
use zikaron_anchor::rpc;
use zikaron_anchor::wire;

/// One endpoint: chain id and url.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub chain: u64,
    pub url: String,
}

impl Endpoint {
    /// The `<chain>=<url>` spelling (the same as the command line's `--endpoint`).
    pub fn parse(spec: &str) -> Option<Endpoint> {
        let (c, url) = spec.split_once('=')?;
        let chain: u64 = c.trim().parse().ok()?;
        if url.trim().is_empty() {
            return None;
        }
        Some(Endpoint { chain, url: url.trim().to_string() })
    }

    pub fn spec(&self) -> String {
        format!("{}={}", self.chain, self.url)
    }
}

/// One reading, with how many places were asked.
#[derive(Clone, Debug)]
pub struct Reading {
    pub value: Value,
    pub sources: usize,
    /// Single source: only one place answered, which must be flagged.
    pub single_source: bool,
}

/// The plain words of an RPC trouble: which kind and the original sentence, without Debug quotes or variant
/// names. The fault table and the status line read the same `Fault`, so both say the same.
pub fn trouble_said(url: &str, t: &rpc::Trouble) -> String {
    let (what, inner) = match t {
        rpc::Trouble::Node(e) => (crate::lang::t(crate::lang::Key::Tail089), e),
        rpc::Trouble::Transport(e) => (crate::lang::t(crate::lang::Key::Tail090), e),
        rpc::Trouble::NotServed(k) => (crate::lang::t(crate::lang::Key::Tail091), k),
        rpc::Trouble::Contradiction(k) => (crate::lang::t(crate::lang::Key::Tail092), k),
    };
    // The original sentence often already contains the url; it is not added twice.
    if inner.starts_with(url) {
        format!("{what}:{inner}")
    } else {
        format!("{url}: {what}:{inner}")
    }
}

// What the node said.

/// What the node said. Closed.
///
/// Folding every sending failure into `SEND_FAILED` with the raw text on screen made insufficient balance, a
/// used nonce, rate limiting and a wrong chain all read "sent, but failed on chain", though often nothing was
/// sent. This closed table answers why the node refused; pages read only it, and [`said_fault`] dispatches
/// plain words and next steps per member.
///
/// Two branches: the node answered and refused ([`Refusal`], from the JSON-RPC error's `code` plus sentence
/// markers), and the transport families ([`Wire`]: four TLS layers, timeout, answer too long, answer not
/// JSON). Anything unrecognized goes to its branch's "other" with the original words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Said {
    Node(Refusal),
    Wire(Wire),
}

/// The node answered and refused. Closed; unrecognized is [`Refusal::Other`] with the original words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The balance does not cover fee cap times gas.
    Funds,
    /// The nonce is used.
    NonceUsed,
    /// The same transaction is already pending.
    Pending,
    /// Underpriced (below the base fee, or a resend without enough increase).
    Underpriced,
    /// Gas too low.
    GasTooLow,
    /// The contract refused (execution reverted).
    Reverted,
    /// Rate limited.
    RateLimited,
    /// This node does not provide the method.
    NoMethod,
    /// Credentials required (key, project id).
    Auth,
    /// Wrong chain id.
    WrongChain,
    /// Unrecognized: the node's own words.
    Other(String),
}

/// The transport families. Closed; unrecognized (cannot connect, disconnected) is [`Wire::Other`] with the
/// original words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Wire {
    Tls(Layer),
    Timeout,
    Oversize,
    NotJson,
    Other(String),
}

/// Markers in a JSON-RPC error sentence (compared in lowercase), one member per line; order decides: the
/// first match answers.
const MARKS: [(&[&str], fn() -> Refusal); 10] = [
    (&["insufficient funds", "insufficient balance"], || Refusal::Funds),
    (&["nonce too low", "nonce has already been used", "nonce is too low", "already been used"], || Refusal::NonceUsed),
    (&["already known", "known transaction", "already imported", "already exists", "alreadyknown"], || Refusal::Pending),
    (&["underpriced", "fee cap less than block base fee", "max fee per gas less than block base fee", "less than block base fee", "fee too low"], || Refusal::Underpriced),
    (&["intrinsic gas too low", "gas too low", "gas limit too low"], || Refusal::GasTooLow),
    (&["execution reverted", "reverted"], || Refusal::Reverted),
    (&["rate limit", "too many requests", "limit exceeded", "request limit", "capacity exceeded", "throttled"], || Refusal::RateLimited),
    (&["method not found", "does not exist/is not available", "not supported", "unsupported method", "method not available"], || Refusal::NoMethod),
    (&["unauthorized", "authentication", "api key", "project id", "forbidden", "access denied", "invalid key"], || Refusal::Auth),
    (&["chain id", "chainid", "wrong chain", "invalid chain"], || Refusal::WrongChain),
];

/// Read a node's error object as a member. Sentence markers first (codes differ between providers, and
/// sentences are more precise), then codes: -32601 method not provided, -32005 rate limited, 3 contract
/// refused. Anything else is "other" with the original words.
pub fn refusal_of(err: &str) -> Refusal {
    let w = wire::parse(err.as_bytes());
    let message = w.as_ref().and_then(|x| x.member("message")).and_then(|m| m.as_str()).unwrap_or(err).to_string();
    let code = w.as_ref().and_then(|x| x.member("code")).and_then(|c| match &c.body {
        wire::Body::Num(n) => n.parse::<i64>().ok(),
        _ => None,
    });
    let low = message.to_lowercase();
    for (marks, which) in MARKS.iter() {
        if marks.iter().any(|m| low.contains(m)) {
            return which();
        }
    }
    match code {
        Some(-32601) => Refusal::NoMethod,
        Some(-32005) => Refusal::RateLimited,
        Some(3) => Refusal::Reverted,
        _ => Refusal::Other(err.to_string()),
    }
}

/// Read an RPC trouble as "what the node said". The two recording cases (not recorded, contradictory) belong
/// to the tools and go to the transport branch's "other".
pub fn said(t: &rpc::Trouble) -> Said {
    match t {
        rpc::Trouble::Node(e) => Said::Node(refusal_of(e)),
        rpc::Trouble::Transport(e) => Said::Wire(match Layer::of(e) {
            Some(l) => Wire::Tls(l),
            None if rpc::is_late(e) => Wire::Timeout,
            None if rpc::is_overlong(e) => Wire::Oversize,
            None if rpc::is_not_json(e) => Wire::NotJson,
            None => Wire::Other(e.clone()),
        }),
        rpc::Trouble::NotServed(k) | rpc::Trouble::Contradiction(k) => Said::Wire(Wire::Other(k.clone())),
    }
}

/// What sending does next after an endpoint refuses. Dispatched only here (as with [`said_fault`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// Rate limited: wait through the backoff list and ask the same place again.
    Retry,
    /// Network kind (other members of `Known::NETWORK`): move to the next endpoint.
    NextEndpoint,
    /// The node's substantive verdict (balance, nonce, price, revert): every place would say the same, so
    /// refuse by name at once.
    Stop,
}

/// A refusal at submission may mean "already taken": "already pending" and "nonce used". Whether it really
/// was taken is decided by [`tx_known`] asking by hash; this only matches the form.
pub fn already_taken(f: &Fault) -> bool {
    matches!(f.which(), Some(Known::AlreadyPending) | Some(Known::NonceUsed))
}

/// Whether any node knows this transaction (asked by hash; the first non-empty answer). "Already pending",
/// "nonce used" and "submitted but no answer anywhere" are decided only by this: known means it went out,
/// unknown means it did not (the refusal's words alone decide nothing).
pub fn tx_known(urls: &[String], chain: u64, hash: &[u8; 32]) -> bool {
    let q = Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(hash))]);
    urls.iter().any(|u| {
        let ep = Endpoint { chain, url: u.clone() };
        matches!(ask(std::slice::from_ref(&ep), "eth_getTransactionByHash", &q), Ok(r) if !matches!(r.value, Value::Null))
    })
}

/// The first answer from the chain (for the balance check and reading call data back): ask endpoint by
/// endpoint and use the first answer. Load-bearing green still uses [`ask`]'s agreement rule; these two are a
/// pre-send check and a screen reading, and a node a block behind should not block sending.
pub fn ask_first(eps: &[Endpoint], method: &str, params: &Value) -> Result<Value, Fault> {
    let mut last: Option<Fault> = None;
    for ep in eps {
        match ask(std::slice::from_ref(ep), method, params) {
            Ok(r) => return Ok(r.value),
            Err(f) => last = Some(f),
        }
    }
    Err(last.unwrap_or_else(|| Fault::known(Known::NoEndpoint, String::new())))
}

/// Balance from the first endpoint that answers (see [`ask_first`]).
pub fn balance_first(eps: &[Endpoint], who: &Address) -> Result<u128, Fault> {
    let params = Value::Arr(vec![Value::Str(who.hex()), Value::Str("latest".into())]);
    let v = ask_first(eps, "eth_getBalance", &params)?;
    let hex = match &v {
        Value::Str(s) => s.clone(),
        other => return Err(Fault::known(Known::ChainShape, crate::lang::filln(crate::lang::Key::Tail099, &[&format!("{:?}", other)]))),
    };
    wei(&hex).ok_or_else(|| Fault::known(Known::ChainShape, crate::lang::filln(crate::lang::Key::Tail100, &[&(hex).to_string()])))
}

/// Decide the next step from the refusal.
pub fn next_after(f: &Fault) -> Next {
    match f.which() {
        Some(Known::RateLimited) => Next::Retry,
        Some(k) if Known::NETWORK.contains(&k) => Next::NextEndpoint,
        _ => Next::Stop,
    }
}

/// Make a named refusal from what the node said; dispatched only here. The evidence tail is the original
/// words (`trouble_said`, with the endpoint), so even "unrecognized" shows what the node actually said.
pub fn said_fault(url: &str, t: &rpc::Trouble) -> Fault {
    let tail = trouble_said(url, t);
    let k = match said(t) {
        Said::Node(r) => match r {
            Refusal::Funds => Known::InsufficientFunds,
            Refusal::NonceUsed => Known::NonceUsed,
            Refusal::Pending => Known::AlreadyPending,
            Refusal::Underpriced => Known::Underpriced,
            Refusal::GasTooLow => Known::GasTooLow,
            Refusal::Reverted => Known::ContractRefused,
            Refusal::RateLimited => Known::RateLimited,
            Refusal::NoMethod => Known::MethodMissing,
            Refusal::Auth => Known::NodeAuth,
            Refusal::WrongChain => Known::WrongChain,
            Refusal::Other(_) => Known::NodeRefused,
        },
        Said::Wire(w) => match w {
            Wire::Tls(_) => Known::NodeTls,
            Wire::Timeout => Known::NodeTimeout,
            Wire::Oversize => Known::AnswerTooLong,
            Wire::NotJson => Known::AnswerNotJson,
            Wire::Other(_) => Known::Unreachable,
        },
    };
    Fault::known(k, tail)
}

/// The only place an endpoint is opened from a url. `http://` uses the anchoring crate's [`rpc::Http`]
/// unchanged, `https://` uses [`Https`], anything else gives `None` and the caller refuses by name. Chain
/// queries, block height, chain time, scans, reconciliation and anchoring all call it.
pub fn endpoint_at(url: &str) -> Option<Box<dyn rpc::Endpoint>> {
    if url.starts_with("http://") {
        return rpc::Http::new(url).map(|h| Box::new(h) as Box<dyn rpc::Endpoint>);
    }
    if url.starts_with("https://") {
        return Https::new(url).map(|h| Box::new(h) as Box<dyn rpc::Endpoint>);
    }
    None
}

/// Which layer of an https connection failed. Closed; the certificate layer is kept apart from the
/// reachability layers (failures are named by layer).
///
/// The code leads the trouble sentence (`TLS_CERT · <plain words>`): people read the second half, tools the
/// first, both from one place.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    /// The host name did not resolve.
    Name,
    /// TCP could not connect.
    Connect,
    /// Other TLS handshake failures (protocol mismatch, the peer disconnected, timeout).
    Handshake,
    /// The server certificate failed verification (no chain to a root, expired, wrong host name,
    /// self-signed).
    Certificate,
}

impl Layer {
    /// Closed: tools tell the layers apart by these codes.
    pub const ALL: [Layer; 4] = [Layer::Name, Layer::Connect, Layer::Handshake, Layer::Certificate];

    pub fn code(self) -> &'static str {
        match self {
            Layer::Name => "TLS_NAME",
            Layer::Connect => "TLS_CONNECT",
            Layer::Handshake => "TLS_HANDSHAKE",
            Layer::Certificate => "TLS_CERT",
        }
    }

    fn key(self) -> crate::lang::Key {
        match self {
            Layer::Name => crate::lang::Key::Tail232,
            Layer::Connect => crate::lang::Key::Tail233,
            Layer::Handshake => crate::lang::Key::Tail234,
            Layer::Certificate => crate::lang::Key::Tail235,
        }
    }

    /// Which layer a trouble sentence reports (read from its leading code; `None` for anything else).
    pub fn of(said: &str) -> Option<Layer> {
        Layer::ALL.into_iter().find(|l| said.starts_with(l.code()))
    }
}

/// An https endpoint. The HTTP/1.1 exchange matches [`rpc::Http`] (one connection per request, `Connection:
/// close`, canonical request body) over a TLS stream from [`crate::cryptx::tls_connect`]. The certificate
/// chain and host name are always verified; there is no way to skip them. Timeouts and the answer cap use the
/// same [`rpc::Limits`] (`ZKA_TIMEOUT_SECS`, `ZKA_MAX_ANSWER_BYTES`), so a silent node cannot hang a scan.
/// Connection failures are [`rpc::Trouble::Transport`] naming the layer (name resolution, connection, TLS
/// handshake, certificate).
pub struct Https {
    url: String,
    host: String,
    port: u16,
    path: String,
    next_id: u64,
    limits: rpc::Limits,
}

impl Https {
    /// `https://host[:port]/path`; any other spelling gives `None`.
    pub fn new(url: &str) -> Option<Https> {
        let rest = url.strip_prefix("https://")?;
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), p.parse().ok()?),
            None => (authority.to_string(), 443u16),
        };
        if host.is_empty() {
            return None;
        }
        Some(Https { url: url.to_string(), host, port, path: path.to_string(), next_id: 1, limits: rpc::Limits::from_env() })
    }

    fn fail(&self, layer: Layer, detail: &str) -> rpc::Trouble {
        rpc::Trouble::Transport(format!("{} · {}", layer.code(), crate::lang::filln(layer.key(), &[&self.url, detail])))
    }
}

impl Https {
    /// The one transport for an exchange: connect TCP, handshake TLS (certificate and host name always
    /// verified), write the request, read the whole answer in chunks, within `limits`'s deadline and cap.
    /// Node queries (POST) and remote fetches (GET) both use it, so TLS, timeouts and overflow behave the
    /// same.
    fn exchange(&self, request: &[u8], limits: &rpc::Limits) -> Result<Vec<u8>, rpc::Trouble> {
        use std::io::{Read, Write};
        let began = std::time::Instant::now();
        let limit = limits.deadline;
        let deadline = (!limit.is_zero()).then(|| began + limit);
        let addrs: Vec<std::net::SocketAddr> = std::net::ToSocketAddrs::to_socket_addrs(&(self.host.as_str(), self.port))
            .map_err(|e| self.fail(Layer::Name, &e.to_string()))?
            .collect();
        let first = addrs.first().ok_or_else(|| self.fail(Layer::Name, &self.host))?;
        let tcp = match limit.is_zero() {
            true => std::net::TcpStream::connect(first),
            false => std::net::TcpStream::connect_timeout(first, limit),
        }
        .map_err(|e| self.fail(Layer::Connect, &e.to_string()))?;
        if !limit.is_zero() {
            let d = Some(limit);
            tcp.set_read_timeout(d).and_then(|_| tcp.set_write_timeout(d)).map_err(|e| self.fail(Layer::Connect, &e.to_string()))?;
        }
        let mut tls = crate::cryptx::tls_connect(&self.host, tcp, deadline).map_err(|t| match t {
            crate::cryptx::TlsTrouble::Name(x) => self.fail(Layer::Name, &x),
            crate::cryptx::TlsTrouble::Certificate(x) => self.fail(Layer::Certificate, &x),
            crate::cryptx::TlsTrouble::Handshake(x) => self.fail(Layer::Handshake, &x),
        })?;
        let io = |this: &Https, e: std::io::Error| match crate::cryptx::tls_said(&e) {
            Some(crate::cryptx::TlsTrouble::Certificate(x)) => this.fail(Layer::Certificate, &x),
            _ => rpc::Trouble::Transport(format!("{}: {e}", this.url)),
        };
        tls.write_all(request).and_then(|_| tls.flush()).map_err(|e| io(self, e))?;
        // Read in chunks; after each, check the total deadline, the answer size, and whether the answer is
        // complete.
        let mut raw = Vec::new();
        let mut chunk = [0u8; 16 << 10];
        loop {
            match tls.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    raw.extend_from_slice(&chunk[..n]);
                    if limits.max_answer > 0 && raw.len() > limits.max_answer {
                        return Err(rpc::overlong(&self.url, limits.max_answer));
                    }
                    if answer_complete(&raw) {
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                // A peer that disconnects without a close alert: a complete answer is accepted; an incomplete
                // one is named.
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(e) => return Err(io(self, e)),
            }
            if !limit.is_zero() && began.elapsed() >= limit {
                return Err(rpc::late(&self.url, limit));
            }
        }
        Ok(raw)
    }

    fn authority(&self) -> String {
        if self.port == 443 { self.host.clone() } else { format!("{}:{}", self.host, self.port) }
    }

    /// Fetch one resource (GET): status, `Location` (when present) and body, within `limits`. Whether to
    /// follow redirects is the caller's decision (remote fetch allows only same-origin https); this asks one
    /// place at a time.
    pub fn get(&self, limits: &rpc::Limits) -> Result<Got, rpc::Trouble> {
        let head = format!("GET {} HTTP/1.1\r\nHost: {}\r\nAccept: */*\r\nConnection: close\r\n\r\n", self.path, self.authority());
        let raw = self.exchange(head.as_bytes(), limits)?;
        let split = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or_else(|| rpc::Trouble::Transport("答里没有头身分界".into()))?;
        let text = String::from_utf8_lossy(&raw[..split]).to_string();
        let status: u16 = text
            .lines()
            .next()
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|c| c.parse().ok())
            .ok_or_else(|| rpc::Trouble::Transport("答的状态行读不出".into()))?;
        let location = text.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim().eq_ignore_ascii_case("location").then(|| v.trim().to_string())
        });
        let body = http_body(&raw[..split], &raw[split + 4..])?;
        Ok(Got { status, location, body })
    }

    /// The three parts of the address: host, port, path (read when comparing origins and building the next
    /// file's address).
    pub fn parts(&self) -> (&str, u16, &str) {
        (&self.host, self.port, &self.path)
    }
}

/// The answer of one GET.
pub struct Got {
    pub status: u16,
    pub location: Option<String>,
    pub body: Vec<u8>,
}

impl rpc::Endpoint for Https {
    fn call(&mut self, method: &str, params: &Value) -> Result<wire::W, rpc::Trouble> {
        let id = self.next_id;
        self.next_id += 1;
        let body = zikaron::json::canon_bytes(&Value::Obj(vec![
            ("id".into(), Value::Int(id)),
            ("jsonrpc".into(), Value::Str("2.0".into())),
            ("method".into(), Value::Str(method.into())),
            ("params".into(), params.clone()),
        ]));
        let head = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.path,
            self.authority(),
            body.len()
        );
        let mut request = head.into_bytes();
        request.extend_from_slice(&body);
        let raw = self.exchange(&request, &self.limits)?;
        let split = raw
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .ok_or_else(|| rpc::Trouble::Transport("答里没有头身分界".into()))?;
        let payload = http_body(&raw[..split], &raw[split + 4..])?;
        let v = wire::parse(&payload).ok_or(rpc::Trouble::Transport(rpc::NOT_JSON.into()))?;
        if let Some(err) = v.member("error") {
            return Err(rpc::Trouble::Node(wire::write(err)));
        }
        Ok(v.member("result").cloned().unwrap_or(wire::W::of(wire::Body::Null)))
    }
    fn name(&self) -> String {
        self.url.clone()
    }
}

/// Whether an HTTP answer is complete: the header/body boundary arrived, and the body has its
/// `Content-Length` or the final chunk arrived. With neither, read until the peer closes.
fn answer_complete(raw: &[u8]) -> bool {
    let Some(split) = raw.windows(4).position(|w| w == b"\r\n\r\n") else { return false };
    let head = String::from_utf8_lossy(&raw[..split]).to_ascii_lowercase();
    let body = &raw[split + 4..];
    if head.contains("transfer-encoding: chunked") {
        return http_body(&raw[..split], body).is_ok();
    }
    head.lines()
        .find_map(|l| l.strip_prefix("content-length:").and_then(|n| n.trim().parse::<usize>().ok()))
        .map(|n| body.len() >= n)
        .unwrap_or(false)
}

/// The HTTP body: chunked transfer is unchunked, `Content-Length` bodies are taken by length, anything else
/// as received.
fn http_body(head: &[u8], rest: &[u8]) -> Result<Vec<u8>, rpc::Trouble> {
    let h = String::from_utf8_lossy(head).to_ascii_lowercase();
    if !h.contains("transfer-encoding: chunked") {
        let len = h.lines().find_map(|l| l.strip_prefix("content-length:").and_then(|n| n.trim().parse::<usize>().ok()));
        return match len {
            Some(n) => rest.get(..n).map(|b| b.to_vec()).ok_or_else(|| rpc::Trouble::Transport("答的身短于它报的长度".into())),
            None => Ok(rest.to_vec()),
        };
    }
    let mut out = Vec::new();
    let mut i = 0usize;
    loop {
        let tail = rest.get(i..).ok_or_else(|| rpc::Trouble::Transport("分块答被截断".into()))?;
        let eol = tail.windows(2).position(|w| w == b"\r\n").ok_or_else(|| rpc::Trouble::Transport("分块的长度行没收尾".into()))?;
        let size = String::from_utf8_lossy(&tail[..eol]);
        let size = size.split(';').next().unwrap_or("").trim().to_string();
        let n = usize::from_str_radix(&size, 16).map_err(|_| rpc::Trouble::Transport("分块的长度读不出".into()))?;
        i += eol + 2;
        if n == 0 {
            return Ok(out);
        }
        let chunk = rest.get(i..i + n).ok_or_else(|| rpc::Trouble::Transport("分块短了".into()))?;
        out.extend_from_slice(chunk);
        if rest.get(i + n..i + n + 2) != Some(b"\r\n") {
            return Err(rpc::Trouble::Transport("分块答被截断".into()));
        }
        i += n + 2;
    }
}

/// The code when nobody answered: the first trouble that actually reached a node is dispatched by
/// [`said_fault`] (refusals, TLS and timeouts each named); when none reached a node (malformed address,
/// unparseable answer), `UNREACHABLE`. The evidence tail is every place's original words joined by ` · `,
/// unchanged.
fn none_answered(first: Option<Fault>, refused: &[String]) -> Fault {
    let k = first.and_then(|f| f.which()).unwrap_or(Known::Unreachable);
    Fault::known(k, refused.join(" · "))
}

/// Default backoff when sending is rate-limited: ask the same place again after each of these (not whole
/// seconds), then move on.
pub const SEND_BACKOFF: [std::time::Duration; 2] = [std::time::Duration::from_millis(300), std::time::Duration::from_millis(900)];

pub fn ask(eps: &[Endpoint], method: &str, params: &Value) -> Result<Reading, Fault> {
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, method.to_string()));
    }
    let mut runs: Vec<(String, Value)> = Vec::new();
    let mut refused: Vec<String> = Vec::new();
    let mut first: Option<Fault> = None;
    for e in eps {
        let Some(mut http) = endpoint_at(&e.url) else {
            refused.push(crate::lang::filln(crate::lang::Key::Tail093, &[&(e.url).to_string()]));
            continue;
        };
        match http.call(method, params) {
            Ok(w) => match wire::to_core(&w) {
                Some(v) => runs.push((e.url.clone(), v)),
                None => refused.push(crate::lang::filln(crate::lang::Key::Tail094, &[&(e.url).to_string()])),
            },
            Err(t) => {
                first.get_or_insert_with(|| said_fault(&e.url, &t));
                refused.push(trouble_said(&e.url, &t));
            }
        }
    }
    if runs.is_empty() {
        return Err(none_answered(first, &refused));
    }
    let sources = runs.len();
    match endpoints::agree(runs) {
        Ok(r) => Ok(Reading {
            value: r.fragment,
            sources,
            // Single source comes from the endpoint rule, not from counting here.
            single_source: r.single_source,
        }),
        Err(d) => Err(Fault::known(
            Known::Disagree,
            crate::lang::filln(crate::lang::Key::Tail086, &[&(d.sources.len()).to_string(), &(d.sources.join(" ")).to_string()]),
        )),
    }
}

/// Which block this pass reads. Each endpoint reports its own height; the smallest is used.
///
/// Different heights are not a disagreement (some are a block or two behind), so this does not use the
/// endpoint rule's agreement; the smallest is one every endpoint has reached, so readings asked at that block
/// still require agreement across endpoints.
///
/// Pinning the height gives "which moment does this credential describe" an externally checkable answer: a
/// number from `latest` no longer matches a second later (see `grantx::Held::block`).
pub fn head_block(eps: &[Endpoint], chain: u64) -> Result<(u64, usize), Fault> {
    // Ask only this chain's endpoints (as in `head_time`): mixing chains would take another chain's height.
    let eps: Vec<&Endpoint> = eps.iter().filter(|e| e.chain == chain).collect();
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::filln(crate::lang::Key::Tail095, &[&(chain).to_string()])));
    }
    let mut heights: Vec<u64> = Vec::new();
    let mut refused: Vec<String> = Vec::new();
    let mut first: Option<Fault> = None;
    for e in eps {
        let Some(mut http) = endpoint_at(&e.url) else {
            refused.push(crate::lang::filln(crate::lang::Key::Tail093, &[&(e.url).to_string()]));
            continue;
        };
        match http.call("eth_blockNumber", &Value::Arr(Vec::new())) {
            Ok(w) => match wire::to_core(&w) {
                Some(Value::Str(h)) => match wei(&h) {
                    Some(n) if n <= u128::from(u64::MAX) => heights.push(n as u64),
                    _ => refused.push(crate::lang::filln(crate::lang::Key::Tail096, &[&(e.url).to_string(), &(h).to_string()])),
                },
                _ => refused.push(crate::lang::filln(crate::lang::Key::Tail094, &[&(e.url).to_string()])),
            },
            Err(t) => {
                first.get_or_insert_with(|| said_fault(&e.url, &t));
                refused.push(trouble_said(&e.url, &t));
            }
        }
    }
    match heights.iter().min() {
        Some(n) => Ok((*n, heights.len())),
        None => Err(none_answered(first, &refused)),
    }
}

/// Chain time: the latest block's time and number. Only the basis chain's endpoints are asked (mixing chains
/// would take another chain's time); each is asked once and the latest time is taken (load-bearing reads are
/// conservative, and for deadlines and countdowns the conservative moment is the later one: a closed window
/// must not still show open). When nobody answers, it is refused by name. Every `now` used for deadlines and
/// countdowns comes from here; no wall clock enters a decision in this crate.
pub fn head_time(eps: &[Endpoint], chain: u64) -> Result<(u64, u64, usize), Fault> {
    let eps: Vec<&Endpoint> = eps.iter().filter(|e| e.chain == chain).collect();
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::filln(crate::lang::Key::Tail097, &[&(chain).to_string()])));
    }
    let mut seen: Vec<(u64, u64)> = Vec::new();
    let mut refused: Vec<String> = Vec::new();
    let mut first: Option<Fault> = None;
    for e in eps {
        let Some(mut http) = endpoint_at(&e.url) else {
            refused.push(crate::lang::filln(crate::lang::Key::Tail093, &[&(e.url).to_string()]));
            continue;
        };
        let params = Value::Arr(vec![Value::Str("latest".into()), Value::Bool(false)]);
        match http.call("eth_getBlockByNumber", &params) {
            Ok(w) => match wire::to_core(&w) {
                Some(Value::Obj(m)) => {
                    let take = |k: &str| -> Option<u64> {
                        m.iter().find(|(n, _)| n == k).and_then(|(_, v)| match v {
                            Value::Str(h) => wei(h).filter(|n| *n <= u128::from(u64::MAX)).map(|n| n as u64),
                            _ => None,
                        })
                    };
                    match (take("timestamp"), take("number")) {
                        (Some(t), Some(n)) => seen.push((t, n)),
                        _ => refused.push(crate::lang::filln(crate::lang::Key::Tail098, &[&(e.url).to_string()])),
                    }
                }
                _ => refused.push(crate::lang::filln(crate::lang::Key::Tail094, &[&(e.url).to_string()])),
            },
            Err(t) => {
                first.get_or_insert_with(|| said_fault(&e.url, &t));
                refused.push(trouble_said(&e.url, &t));
            }
        }
    }
    match seen.iter().max() {
        Some((t, n)) => Ok((*t, *n, seen.len())),
        None => Err(none_answered(first, &refused)),
    }
}

/// An address's balance (wei). A load-bearing read: several endpoints must agree.
pub fn balance(eps: &[Endpoint], who: &Address) -> Result<(u128, Reading), Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are marked too.
    crate::trace::mark(crate::feature::Feature::H4);
    let params = Value::Arr(vec![Value::Str(who.hex()), Value::Str("latest".into())]);
    let r = ask(eps, "eth_getBalance", &params)?;
    let hex = match &r.value {
        Value::Str(s) => s.clone(),
        other => return Err(Fault::known(Known::ChainShape, crate::lang::filln(crate::lang::Key::Tail099, &[&format!("{:?}", other)]))),
    };
    let n = wei(&hex).ok_or_else(|| Fault::known(Known::ChainShape, crate::lang::filln(crate::lang::Key::Tail100, &[&(hex).to_string()])))?;
    Ok((n, r))
}

/// A `0x` quantity string to a number.
pub fn wei(hex: &str) -> Option<u128> {
    let bare = hex.strip_prefix("0x")?;
    if bare.is_empty() || bare.len() > 32 || !bare.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u128::from_str_radix(bare, 16).ok()
}

/// This transaction's two fee fields, computed from the chain's base fee: ask the latest block's
/// `baseFeePerGas`; cap = base fee × 2 + priority fee. When unavailable (no answer, no such field), the
/// fallback is used and sending is not blocked; the balance check and the confirmation card read this same
/// value, and the screen says which was used.
pub fn fees(eps: &[Endpoint], chain: u64) -> zikaron_anchor::send::Fees {
    let eps: Vec<Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
    // Ask for one pinned block, not `latest`: two endpoints a block apart answer `latest` with different
    // blocks, the readings disagree, and the fallback would stand in for a base fee every endpoint knows.
    // The smallest head is one every endpoint has reached (`head_block`).
    let Ok((height, _)) = head_block(&eps, chain) else {
        return zikaron_anchor::send::Fees::fallback();
    };
    let params = Value::Arr(vec![Value::Str(format!("0x{height:x}")), Value::Bool(false)]);
    match ask(&eps, "eth_getBlockByNumber", &params) {
        Ok(r) => match &r.value {
            Value::Obj(m) => m
                .iter()
                .find(|(k, _)| k == "baseFeePerGas")
                .and_then(|(_, v)| match v {
                    Value::Str(h) => wei(h).filter(|n| *n <= u128::from(u64::MAX)).map(|n| n as u64),
                    _ => None,
                })
                .map(zikaron_anchor::send::Fees::from_base)
                .unwrap_or_else(zikaron_anchor::send::Fees::fallback),
            _ => zikaron_anchor::send::Fees::fallback(),
        },
        Err(_) => zikaron_anchor::send::Fees::fallback(),
    }
}
