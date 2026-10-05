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

/// The only place an endpoint is opened from a url. The address is read by the one transport's reader
/// (`zikaron_net::parse`: the scheme and host in any case, RFC 3986): `http` uses the anchoring crate's
/// [`rpc::Http`], `https` uses [`Https`] (the same transport, with its failures named by layer); anything else
/// gives `None` and the caller refuses by name. Chain queries, block height, chain time, scans, reconciliation
/// and anchoring all call it.
pub fn endpoint_at(url: &str) -> Option<Box<dyn rpc::Endpoint>> {
    match zikaron_net::parse(url)?.scheme {
        zikaron_net::Scheme::Http => rpc::Http::new(url).map(|h| Box::new(h) as Box<dyn rpc::Endpoint>),
        zikaron_net::Scheme::Https => Https::new(url).map(|h| Box::new(h) as Box<dyn rpc::Endpoint>),
    }
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

/// An https endpoint: the one transport (`zikaron_net`: TLS configuration, certificate chain and host name
/// always verified, deadline and answer cap from [`rpc::Limits`]), with its failures said by layer (name
/// resolution, connection, TLS handshake, certificate) in this app's words. Node queries (POST) and remote
/// fetches (GET) both use it.
pub struct Https {
    url: String,
    target: zikaron_net::Target,
    next_id: u64,
    limits: rpc::Limits,
}

impl Https {
    /// An `https` address (the scheme and host in any case); any other gives `None`.
    pub fn new(url: &str) -> Option<Https> {
        let target = zikaron_net::parse(url).filter(|t| t.scheme == zikaron_net::Scheme::Https)?;
        Some(Https { url: url.trim().to_string(), target, next_id: 1, limits: rpc::Limits::from_env() })
    }

    fn fail(&self, layer: Layer, detail: &str) -> rpc::Trouble {
        rpc::Trouble::Transport(format!("{} · {}", layer.code(), crate::lang::filln(layer.key(), &[&self.url, detail])))
    }

    /// A transport failure in this app's words: the four connection layers by layer, the deadline and the cap
    /// in the anchoring crate's recognized sentences.
    fn said(&self, f: zikaron_net::Fail) -> rpc::Trouble {
        match f {
            zikaron_net::Fail::Name(x) => self.fail(Layer::Name, &x),
            zikaron_net::Fail::Connect(x) => self.fail(Layer::Connect, &x),
            zikaron_net::Fail::Handshake(x) => self.fail(Layer::Handshake, &x),
            zikaron_net::Fail::Certificate(x) => self.fail(Layer::Certificate, &x),
            other => rpc::transport_said(&self.url, &other),
        }
    }

    /// Fetch one resource (GET): status, `Location` (when present) and body, within `limits`. Whether to
    /// follow redirects is the caller's decision (remote fetch allows only same-origin https); this asks one
    /// place at a time.
    pub fn get(&self, limits: &rpc::Limits) -> Result<Got, rpc::Trouble> {
        let a = zikaron_net::get(&self.target, limits).map_err(|f| self.said(f))?;
        Ok(Got { status: a.status, location: a.location, body: a.body })
    }

    /// The address as the transport's reader writes it (lowercase scheme and host, default port left out).
    pub fn address(&self) -> String {
        self.target.url()
    }

    /// The three parts of the address: host, port, path (read when comparing origins and building the next
    /// file's address).
    pub fn parts(&self) -> (&str, u16, &str) {
        (&self.target.host, self.target.port, &self.target.path)
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
        let got = zikaron_net::post_json(&self.target, &body, &self.limits).map_err(|f| self.said(f))?;
        let v = wire::parse(&got.body).ok_or(rpc::Trouble::Transport(rpc::NOT_JSON.into()))?;
        if let Some(err) = v.member("error") {
            return Err(rpc::Trouble::Node(wire::write(err)));
        }
        Ok(v.member("result").cloned().unwrap_or(wire::W::of(wire::Body::Null)))
    }
    fn name(&self) -> String {
        self.url.clone()
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
    ask_with(eps, method, params, None)
}

/// The transaction facts a decision reads: who sent it, what it carried, which block holds it. Endpoints add
/// members of their own (`blockTimestamp`, signature padding), so agreement is asked over these alone.
pub const TX_FACTS: [&str; 3] = ["blockNumber", "from", "input"];

/// The block fact the fee cap reads.
pub const FEE_FACTS: [&str; 1] = ["baseFeePerGas"];

/// The fee-history fact the priority fee reads (each block's median paid priority fee).
pub const TIP_FACTS: [&str; 1] = ["reward"];

/// [`ask`], with agreement over the named facts only (`zikaron_anchor::endpoints::project`): what endpoints
/// add of their own no longer disagrees, a fact that differs still does. The reading's value is the
/// projection.
pub fn ask_facts(eps: &[Endpoint], method: &str, params: &Value, facts: &[&str]) -> Result<Reading, Fault> {
    ask_with(eps, method, params, Some(facts))
}

fn ask_with(eps: &[Endpoint], method: &str, params: &Value, facts: Option<&[&str]>) -> Result<Reading, Fault> {
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
                Some(v) => runs.push((e.url.clone(), match facts {
                    Some(f) => endpoints::project(&v, f),
                    None => v,
                })),
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

/// An address's balance (wei) on `chain`. A load-bearing read: several endpoints must agree.
///
/// Only that chain's endpoints are asked (another chain's balance is another number), at one pinned block:
/// `latest` moves between two asks, and two endpoints a block apart would disagree over a balance neither
/// disputes. The smallest head is one every endpoint has reached (`head_block`).
pub fn balance(eps: &[Endpoint], chain: u64, who: &Address) -> Result<(u128, Reading), Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are marked too.
    crate::trace::mark(crate::feature::Feature::H4);
    let eps: Vec<Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
    let (height, _) = head_block(&eps, chain)?;
    let params = Value::Arr(vec![Value::Str(who.hex()), Value::Str(format!("0x{height:x}"))]);
    let r = ask(&eps, "eth_getBalance", &params)?;
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

/// This transaction's two fee fields, from what this chain's endpoints agree on at one pinned block: that
/// block's `baseFeePerGas`, and the priority fees paid over the blocks up to it (`eth_feeHistory`); the rule is
/// `zikaron_anchor::send::Fees::of`, the one the command line uses. Without a base fee the whole pair is the
/// fallback; without priority fees (unanswered, `null`, refused, endpoints that differ, another shape) the
/// priority fee is its ceiling. Sending is not blocked either way; the balance check and the confirmation card
/// read this same value, and the screen says which was used.
pub fn fees(eps: &[Endpoint], chain: u64) -> zikaron_anchor::send::Fees {
    use zikaron_anchor::send::{tip_of, tip_params, Fees};
    // Only this chain's endpoints are asked: another chain's fees are another number.
    let eps: Vec<Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
    // Ask for one pinned block, not `latest`: two endpoints a block apart answer `latest` with different
    // blocks, the readings disagree, and the fallback would stand in for a base fee every endpoint knows.
    // The smallest head is one every endpoint has reached (`head_block`).
    let Ok((height, _)) = head_block(&eps, chain) else {
        return Fees::fallback();
    };
    let params = Value::Arr(vec![Value::Str(format!("0x{height:x}")), Value::Bool(false)]);
    // Agreement over the base fee alone: endpoints answer the same block with members of their own, and a
    // whole-block comparison would put the fallback in place of a base fee every endpoint gave alike.
    let base = ask_facts(&eps, "eth_getBlockByNumber", &params, &FEE_FACTS).ok().and_then(|r| zikaron_anchor::send::base_fee_of(&r.value));
    // The priority fees paid up to the same block, agreed over the facts the rule reads.
    let tip = ask_facts(&eps, "eth_feeHistory", &tip_params(height), &TIP_FACTS).ok().and_then(|r| tip_of(&r.value));
    Fees::of(base, tip)
}
