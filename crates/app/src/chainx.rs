//! Chain reads. No chain decision is made here: the endpoint rule belongs to the anchoring crate's
//! `endpoints`, transport and JSON-RPC to its `rpc`, scanning and the three verdicts to its `scan`, and the
//! report to the core's `audit`.
//!
//! This layer arranges endpoints and turns call results into readings (balance, chain time).
//!
//! Load-bearing reads count only when several endpoints agree, and a single source is flagged: every reading
//! carries `sources` and `single_source`, so a balance from one endpoint looks different from one that two
//! endpoints agree on.

use crate::fault::{Fault, Known};
use crate::key::Address;
use zikaron::json::Value;
use zikaron_anchor::endpoints;
use zikaron_anchor::rpc;
use zikaron_anchor::wire;

/// A node's address as written, which may carry an API key in its path, query or user part. `Display`,
/// `Debug` and [`NodeAddr::said`] show only the safe form (`zikaron_net::sayable`: scheme, host and port). The
/// written form is readable only inside this crate ([`NodeAddr::for_transport`]), for the transport and this
/// machine's settings, so no message, log line or refusal built from a node can leak the key.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeAddr(String);

impl NodeAddr {
    /// A node's address, as written.
    pub fn new(written: impl Into<String>) -> NodeAddr {
        NodeAddr(written.into())
    }

    /// The address as it may be said: scheme, host and port only (`zikaron_net::sayable`).
    pub fn said(&self) -> String {
        zikaron_net::sayable(&self.0)
    }

    /// The key that identifies this node when facts about nodes are kept apart (`zikaron_net::place_key`). It
    /// includes the path, so it also stays inside this crate.
    pub(crate) fn place(&self) -> String {
        zikaron_net::place_key(&self.0)
    }

    /// The written form, for the transport (and this machine's settings) only.
    pub(crate) fn for_transport(&self) -> &str {
        &self.0
    }
}

impl From<String> for NodeAddr {
    fn from(s: String) -> NodeAddr {
        NodeAddr(s)
    }
}

impl From<&str> for NodeAddr {
    fn from(s: &str) -> NodeAddr {
        NodeAddr(s.to_string())
    }
}

impl std::fmt::Display for NodeAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.said())
    }
}

impl std::fmt::Debug for NodeAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("NodeAddr").field(&self.said()).finish()
    }
}

/// One endpoint: chain id and the node's address.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Endpoint {
    pub chain: u64,
    pub url: NodeAddr,
}

impl Endpoint {
    /// Parses the `<chain>=<url>` form with the same parser as the command line's `--endpoint`
    /// (`zikaron_anchor::rpc::endpoint_spec`).
    pub fn parse(spec: &str) -> Option<Endpoint> {
        let (chain, url) = zikaron_anchor::rpc::endpoint_spec(spec).ok()?;
        Some(Endpoint { chain, url: NodeAddr(url) })
    }

    /// An endpoint for a chain at a node.
    pub fn at(chain: u64, url: impl Into<NodeAddr>) -> Endpoint {
        Endpoint { chain, url: url.into() }
    }

    /// A node entry a person typed (node fields, the check page's nodes): the form is parsed by
    /// [`Endpoint::parse`] and the address by the transport's parser (`rpc::read_address`), the same two the
    /// command line's `--endpoint` uses, so both sides accept or refuse alike. An entry already on disk is read by
    /// form alone ([`Endpoint::parse`]): one whose address does not parse is kept and reported where it is used
    /// (`SETTINGS_SHAPE`), never silently dropped.
    pub fn typed(spec: &str) -> Result<Endpoint, String> {
        // An unparseable entry is described the way nodes are (`zikaron_net::sayable`: a non-address is
        // described by its length), never echoed, so a key typed into it cannot leak into messages or logs.
        let e = Endpoint::parse(spec).ok_or_else(|| crate::lang::filln(crate::lang::Key::Tail008, &[&zikaron_net::sayable(spec)]))?;
        zikaron_anchor::rpc::read_address(e.url.for_transport()).map_err(|_| address_said(e.url.for_transport()))?;
        Ok(e)
    }

    /// The written `<chain>=<url>` form, for this machine's settings and the person's own node editor.
    pub(crate) fn spec(&self) -> String {
        format!("{}={}", self.chain, self.url.for_transport())
    }
}

/// One reading, with how many endpoints it came from.
#[derive(Clone, Debug)]
pub struct Reading {
    pub value: Value,
    pub sources: usize,
    /// Single source: only one endpoint answered, which must be flagged.
    pub single_source: bool,
    /// The endpoints that contributed nothing to the reading, each described (which, and why), in table order.
    pub unanswered: Vec<String>,
}

/// Nodes listed in one line, each in its safe-to-display form (`zikaron_net::sayable`), in order.
fn sayable_all(urls: &[String]) -> String {
    urls.iter().map(|u| zikaron_net::sayable(u)).collect::<Vec<_>>().join(" ")
}

/// The plain wording of an RPC failure: the kind and the original message, without Debug quotes or variant
/// names. The fault table and the status line read the same `Fault`, so both say the same thing.
pub fn trouble_said(url: &str, t: &rpc::Trouble) -> String {
    // Name the node in its safe-to-display form (`zikaron_net::sayable`: no path, query or user info).
    let url = &zikaron_net::sayable(url);
    let (what, inner) = match t {
        rpc::Trouble::Node(e) => (crate::lang::t(crate::lang::Key::Tail089), e),
        // An answer refused at its HTTP status is the node's (or its gateway's) refusal, not a broken wire.
        rpc::Trouble::Transport(e) if rpc::status_of(e).is_some() => (crate::lang::t(crate::lang::Key::Tail089), e),
        rpc::Trouble::Transport(e) => (crate::lang::t(crate::lang::Key::Tail090), e),
        rpc::Trouble::NotServed(k) => (crate::lang::t(crate::lang::Key::Tail091), k),
        rpc::Trouble::Contradiction(k) => (crate::lang::t(crate::lang::Key::Tail092), k),
    };
    // A revert carries what the contract said, read from the error's data. Every place that reports a failure
    // goes through here, so none reports a revert without its reason.
    let reason = match t {
        rpc::Trouble::Node(e) if refusal_of(e) == Refusal::Reverted => zikaron_anchor::said::reverted(e).map(|r| format!(" \u{b7} {}", r.evidence())).unwrap_or_default(),
        _ => String::new(),
    };
    // The original message often already contains the URL; do not add it twice.
    if inner.starts_with(url) {
        format!("{what}:{inner}{reason}")
    } else {
        format!("{url}: {what}:{inner}{reason}")
    }
}

// What the node said.

/// What the node said. Closed.
///
/// Folding every send failure into `SEND_FAILED` with raw text would make insufficient balance, a used nonce,
/// rate limiting and a wrong chain all read "sent, but failed on chain", though often nothing was sent. This
/// closed table says why the node refused; pages read only it, and [`said_fault`] maps each member to plain
/// wording and next steps.
///
/// Two branches: the node answered and refused ([`Refusal`], from the JSON-RPC error's `code` plus message
/// markers), and transport failures ([`Wire`]: four TLS layers, timeout, answer too long, answer not JSON).
/// Anything unrecognized goes to its branch's "other" with the original words (including a coded node refusal
/// the table does not name: both are reported as the node's refusal).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Said {
    Node(Refusal),
    Wire(Wire),
}

/// The node answered and refused: the single closed table, shared with the command line (`zikaron_anchor::said`).
pub use zikaron_anchor::said::{refusal_of, Refusal};

/// Transport failures. Closed; anything unrecognized (cannot connect, disconnected) is [`Wire::Other`] with
/// the original words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Wire {
    Tls(Layer),
    Timeout,
    Oversize,
    NotJson,
    /// JSON, but not an answer to the question (no `result` and no `error`, both, an array, another `id`).
    Shape,
    Other(String),
}

/// Classifies an RPC failure as what the node said. The two recording cases (not served, contradictory) belong
/// to the tools and go to the transport branch's "other".
pub fn said(t: &rpc::Trouble) -> Said {
    // A node's error, or an answer refused at its HTTP status, is classified by the shared table.
    if let Some(r) = zikaron_anchor::said::refusal(t) {
        return Said::Node(r);
    }
    match t {
        rpc::Trouble::Node(e) => Said::Node(refusal_of(e)),
        rpc::Trouble::Transport(e) => Said::Wire(match Layer::of(e) {
            Some(l) => Wire::Tls(l),
            None if rpc::is_late(e) => Wire::Timeout,
            None if rpc::is_overlong(e) => Wire::Oversize,
            None if rpc::is_not_json(e) => Wire::NotJson,
            None if rpc::is_shapeless(e) => Wire::Shape,
            None => Wire::Other(e.clone()),
        }),
        rpc::Trouble::NotServed(k) | rpc::Trouble::Contradiction(k) => Said::Wire(Wire::Other(k.clone())),
    }
}

/// What sending does after an endpoint refuses. Decided only here (as with [`said_fault`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// Rate limited: wait through the backoff list and ask the same endpoint again.
    Retry,
    /// Network kind (other members of `Known::NETWORK`): move to the next endpoint.
    NextEndpoint,
    /// The node's substantive verdict (balance, nonce, price, revert): every endpoint would say the same, so
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
/// unknown means it did not (the refusal's wording alone decides nothing).
pub fn tx_known(urls: &[NodeAddr], chain: u64, hash: &[u8; 32]) -> bool {
    let q = Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(hash))]);
    urls.iter().any(|u| {
        let ep = Endpoint { chain, url: u.clone() };
        matches!(ask(std::slice::from_ref(&ep), "eth_getTransactionByHash", &q), Ok(r) if !matches!(r.value, Value::Null))
    })
}

/// Decides the next step from the refusal.
pub fn next_after(f: &Fault) -> Next {
    match f.which() {
        Some(Known::RateLimited) => Next::Retry,
        Some(k) if Known::NETWORK.contains(&k) => Next::NextEndpoint,
        _ => Next::Stop,
    }
}

/// Why a node address does not parse, in plain words: its scheme, its port, or the rest of its form (from
/// `zikaron_net::read_address`). Every path that opens a node uses this. Empty when it parses.
pub fn address_said(url: &str) -> String {
    use crate::lang::{filln, Key};
    // Named the way every message names a node (`zikaron_net::sayable`): an unparseable address is described
    // by its length, never echoed, so a key typed into it (user info, path, query) never reaches a message or log.
    let said = zikaron_net::sayable(url);
    match zikaron_net::read_address(url) {
        Ok(_) => String::new(),
        Err(zikaron_net::NotAnAddress::Scheme) => filln(Key::Tail093, &[&said]),
        Err(zikaron_net::NotAnAddress::Port) => filln(Key::TailNodePort, &[&said]),
        Err(_) => filln(Key::TailNodeAddress, &[&said]),
    }
}

/// The refusal when no node address parses: an address shape error (`SETTINGS_SHAPE`) with each reason
/// ([`address_said`]); never "unreachable", which would send the person to check the network.
fn no_address_reads(said: &[String]) -> Fault {
    Fault::known(Known::SettingsShape, said.join(" · "))
}

/// What the send card says about where its fee figures came from: the fallback line when the chain gave no
/// base fee (or no fees were read), nothing when the cap was computed from the chain's own base fee. Reads
/// `Fees::from_chain`, never the numbers (the computed pair can equal the fallback pair).
pub fn fee_source_line(fees: Option<&zikaron_anchor::send::Fees>) -> Option<crate::lang::Key> {
    match fees {
        Some(f) if f.from_chain => None,
        _ => Some(crate::lang::Key::U3FeeFallback),
    }
}

/// Turns what the node said into a named refusal; decided only here. The evidence tail is the original
/// message (`trouble_said`, with the endpoint), so even an unrecognized refusal shows what the node said.
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
            Refusal::Coded(_) | Refusal::Other(_) => Known::NodeRefused,
        },
        Said::Wire(w) => match w {
            Wire::Tls(_) => Known::NodeTls,
            Wire::Timeout => Known::NodeTimeout,
            Wire::Oversize => Known::AnswerTooLong,
            Wire::NotJson => Known::AnswerNotJson,
            Wire::Shape => Known::ChainShape,
            Wire::Other(_) => Known::Unreachable,
        },
    };
    Fault::known(k, tail)
}

/// The only place an endpoint is opened from a URL. The address is parsed by the transport's parser
/// (`zikaron_net::parse`: case-insensitive scheme and host, RFC 3986): `http` uses the anchoring crate's
/// [`rpc::Http`], `https` uses [`Https`] (the same transport, with failures named by layer); anything else
/// gives `None` and the caller refuses by name. Chain queries, block height, chain time, scans,
/// reconciliation and anchoring all use it.
pub fn endpoint_at(url: &str) -> Option<Box<dyn rpc::Endpoint + Send>> {
    match zikaron_net::parse(url)?.scheme {
        zikaron_net::Scheme::Http => rpc::Http::new(url).map(|h| Box::new(h) as Box<dyn rpc::Endpoint + Send>),
        zikaron_net::Scheme::Https => Https::new(url).map(|h| Box::new(h) as Box<dyn rpc::Endpoint + Send>),
    }
}

/// Which layer of an https connection failed. Closed; the certificate layer is kept separate from the
/// reachability layers.
///
/// The code leads the failure message (`TLS_CERT · <plain words>`): people read the second half, tools the
/// first.
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

    /// Which layer a failure message reports (from its leading code; `None` otherwise).
    pub fn of(said: &str) -> Option<Layer> {
        Layer::ALL.into_iter().find(|l| said.starts_with(l.code()))
    }
}

/// An https endpoint over the single transport (`zikaron_net`: TLS configuration, certificate chain and host
/// name always verified, deadline and response cap from [`rpc::Limits`]), with failures named by layer (name
/// resolution, connection, TLS handshake, certificate) in this app's words. Node queries (POST) and remote
/// fetches (GET) both use it.
pub struct Https {
    url: String,
    target: zikaron_net::Target,
    next_id: u64,
    limits: rpc::Limits,
}

impl Https {
    /// An `https` address (case-insensitive scheme and host); anything else gives `None`.
    pub fn new(url: &str) -> Option<Https> {
        let target = zikaron_net::parse(url).filter(|t| t.scheme == zikaron_net::Scheme::Https)?;
        Some(Https { url: url.trim().to_string(), target, next_id: 1, limits: rpc::Limits::from_env() })
    }

    fn fail(&self, layer: Layer, detail: &str) -> rpc::Trouble {
        rpc::Trouble::Transport(format!("{} · {}", layer.code(), crate::lang::filln(layer.key(), &[&zikaron_net::sayable(&self.url), detail])))
    }

    /// A transport failure in this app's words: the four connection layers by layer, the deadline and the cap
    /// in the anchoring crate's recognized messages.
    fn said(&self, f: zikaron_net::Fail) -> rpc::Trouble {
        match f {
            zikaron_net::Fail::Name(x) => self.fail(Layer::Name, &x),
            zikaron_net::Fail::Connect(x) => self.fail(Layer::Connect, &x),
            zikaron_net::Fail::Handshake(x) => self.fail(Layer::Handshake, &x),
            zikaron_net::Fail::Certificate(x) => self.fail(Layer::Certificate, &x),
            other => rpc::transport_said(&self.url, &other),
        }
    }

    /// Fetches one resource (GET): status, `Location` (if present) and body, within `limits`. Whether to follow
    /// redirects is the caller's decision (remote fetch allows only same-origin https); this makes one request.
    pub fn get(&self, limits: &rpc::Limits) -> Result<Got, rpc::Trouble> {
        let a = zikaron_net::get(&self.target, limits).map_err(|f| self.said(f))?;
        Ok(Got { status: a.status, location: a.location, body: a.body })
    }

    /// The address as the transport's parser normalizes it (lowercase scheme and host, default port omitted).
    pub fn address(&self) -> String {
        self.target.url()
    }

    /// The address's host, port and path (used when comparing origins and building the next file's address).
    pub fn parts(&self) -> (&str, u16, &str) {
        (&self.target.host, self.target.port, &self.target.path)
    }
}

/// The result of one GET.
pub struct Got {
    pub status: u16,
    pub location: Option<String>,
    pub body: Vec<u8>,
}

impl Https {
    /// One request through the anchoring crate's node query (`rpc::ask_node`), with transport failures named by
    /// layer.
    fn ask(&mut self, method: &str, params: &Value, limits: rpc::Limits) -> Result<wire::W, rpc::Trouble> {
        let id = self.next_id;
        self.next_id += 1;
        rpc::ask_node(&self.url, &self.target, &limits, id, method, params, &|f| self.said(f))
    }
}

impl rpc::Endpoint for Https {
    fn call(&mut self, method: &str, params: &Value) -> Result<wire::W, rpc::Trouble> {
        self.ask(method, params, self.limits)
    }
    fn call_within(&mut self, method: &str, params: &Value, within: std::time::Duration) -> Result<wire::W, rpc::Trouble> {
        self.ask(method, params, rpc::within(self.limits, within))
    }
    fn name(&self) -> String {
        zikaron_net::sayable(&self.url)
    }
    fn place(&self) -> String {
        zikaron_net::place_key(&self.url)
    }
}

/// The refusal when no endpoint answered: the first failure that actually reached a node is classified by
/// [`said_fault`] (refusals, TLS and timeouts each named); if none reached a node (malformed address,
/// unparseable answer), `UNREACHABLE`. The evidence tail is every endpoint's original message joined by ` · `,
/// unchanged.
fn none_answered(first: Option<Fault>, refused: &[String]) -> Fault {
    let k = first.and_then(|f| f.which()).unwrap_or(Known::Unreachable);
    Fault::known(k, refused.join(" · "))
}

/// Default backoff when sending is rate-limited: retry the same endpoint after each of these delays, then move
/// on. The scan uses the same backoff (`zikaron_anchor::said`).
pub const SEND_BACKOFF: [std::time::Duration; 2] = zikaron_anchor::said::RATE_BACKOFF;

pub fn ask(eps: &[Endpoint], method: &str, params: &Value) -> Result<Reading, Fault> {
    ask_with(eps, method, params).map_err(|(f, _)| f)
}

/// [`ask`] for a request whose refusal may concern the call itself (gas estimation): the refusal comes with
/// whether the shared table reads it that way (`zikaron_anchor::said::refuses_the_call`), judged on the same
/// failure the refusal was built from. A refusal not caused by a node (no endpoint, disagreeing endpoints,
/// unparseable addresses) is not about the call.
pub fn ask_call(eps: &[Endpoint], method: &str, params: &Value) -> Result<Reading, (Fault, bool)> {
    ask_with(eps, method, params)
}

/// The transaction facts a decision reads (`zikaron_anchor::judge::TX_FACTS`).
pub const TX_FACTS: [&str; 3] = zikaron_anchor::judge::TX_FACTS;

/// The block fact the fee cap reads (`zikaron_anchor::judge::BLOCK_FACTS`).
pub const FEE_FACTS: [&str; 1] = zikaron_anchor::judge::BLOCK_FACTS;

/// The fee-history fact the priority fee reads (`zikaron_anchor::judge::FEE_HISTORY_FACTS`).
pub const TIP_FACTS: [&str; 1] = zikaron_anchor::judge::FEE_HISTORY_FACTS;

/// Asks every endpoint at once (`zikaron_anchor::endpoints::ask_each`, each with its timeout table), each
/// endpoint once however many times it is listed (`zikaron_net::place_key`). Returns the answers in table
/// order, plus messages for endpoints whose address does not parse (never asked).
fn ask_all(eps: &[Endpoint], method: &str, params: &Value) -> (Vec<(String, Result<wire::W, rpc::Trouble>)>, Vec<String>) {
    let mut seen: Vec<String> = Vec::new();
    let mut nodes: Vec<(String, Box<dyn rpc::Endpoint + Send>)> = Vec::new();
    let mut unread: Vec<String> = Vec::new();
    for e in eps {
        let place = e.url.place();
        if seen.contains(&place) {
            continue;
        }
        seen.push(place);
        match endpoint_at(e.url.for_transport()) {
            Some(h) => nodes.push((e.url.for_transport().to_string(), h)),
            None => unread.push(address_said(e.url.for_transport())),
        }
    }
    (endpoints::ask_each(nodes, method, params), unread)
}

/// The message for an endpoint the judging table skipped: its failure, or that its answer could not be read.
fn missing_said(url: &str, m: &zikaron_anchor::judge::Missing) -> String {
    match m {
        zikaron_anchor::judge::Missing::Trouble(t) => trouble_said(url, t),
        zikaron_anchor::judge::Missing::Unreadable(_) => crate::lang::filln(crate::lang::Key::Tail094, &[&zikaron_net::sayable(url)]),
    }
}

/// Asks every endpoint at once and judges the answers with the shared table (`zikaron_anchor::judge`): an
/// endpoint that does not answer is skipped and named in the reading; differing answers are a disagreement; a
/// transaction some endpoints have and others not yet is asked again after the table's pauses and, if still
/// split, refused naming the endpoints that lack it. A refusal comes with whether it is about the call
/// ([`ask_call`]).
fn ask_with(eps: &[Endpoint], method: &str, params: &Value) -> Result<Reading, (Fault, bool)> {
    use zikaron_anchor::judge::{self, NoReading};
    if eps.is_empty() {
        return Err((Fault::known(Known::NoEndpoint, method.to_string()), false));
    }
    let pauses = judge::not_yet_pauses();
    let mut round = 0usize;
    loop {
        let (answers, unread) = ask_all(eps, method, params);
        let asked = answers.len() + unread.len();
        match judge::judge(method, params, answers) {
            Ok(r) => {
                let unanswered: Vec<String> = unread.iter().cloned().chain(r.missing.iter().map(|(u, m)| missing_said(u, m))).collect();
                // Single source comes from the endpoint rule (via the judging table), not from counting here.
                return Ok(Reading { value: r.value, sources: r.sources.len(), single_source: r.single_source, unanswered });
            }
            Err(NoReading::NotYet { has, not_yet }) => match pauses.get(round) {
                Some(p) => {
                    round += 1;
                    if !p.is_zero() {
                        std::thread::sleep(*p);
                    }
                }
                None => return Err((Fault::known(Known::Disagree, crate::lang::filln(crate::lang::Key::TailNotYetAt, &[&sayable_all(&has), &sayable_all(&not_yet)])), false)),
            },
            Err(NoReading::Differ(d)) => {
                return Err((
                    Fault::known(Known::Disagree, crate::lang::filln(crate::lang::Key::Tail086, &[&(d.sources.len()).to_string(), &sayable_all(&d.sources)])),
                    false,
                ));
            }
            Err(NoReading::NoneAnswered(missing)) => {
                // The refusal kind and whether it is about the call are both read from the first failure.
                let first = missing.iter().find_map(|(u, m)| match m {
                    judge::Missing::Trouble(t) => Some((u, t)),
                    _ => None,
                });
                let call = first.is_some_and(|(_, t)| zikaron_anchor::said::refuses_the_call(t));
                let refused: Vec<String> = unread.iter().cloned().chain(missing.iter().map(|(u, m)| missing_said(u, m))).collect();
                if unread.len() == asked {
                    return Err((no_address_reads(&refused), false));
                }
                return Err((none_answered(first.map(|(u, t)| said_fault(u, t)), &refused), call));
            }
        }
    }
}

/// How far, in blocks, the lowest node may lag the highest before the head is refused as "a node is behind":
/// 64. Healthy nodes of one chain answer within a few blocks of each other; a node 64 behind has stopped
/// following (on the slowest shipped chain about 13 minutes, on the fastest about 15 seconds, still many times
/// a healthy gap). Within this, the lowest head is used: a block every node has reached.
pub const MAX_HEAD_LAG: u64 = 64;

/// Which block this pass reads. Each endpoint reports its own height, all asked at once; the lowest is used
/// (the judging table's head rule: `zikaron_anchor::judge::Rule::Least`).
///
/// Different heights are not a disagreement (some nodes are a block or two behind), so the lowest is one every
/// endpoint has reached, and readings at that block still require agreement across endpoints. Heights further
/// apart than [`MAX_HEAD_LAG`] are refused by name: which node is behind, by how much, and where the highest
/// is (a stalled node would make every recent record read "not on chain yet").
///
/// Pinning the height gives "which moment does this credential describe" an externally checkable answer,
/// whereas a `latest` number goes stale within seconds (see `grantx::Held::block`).
///
/// A height is parsed one way ([`height_of`]): a `0x` quantity that fits 64 bits, zero and the maximum
/// included. A node whose answer does not parse (not text, not a quantity, over 64 bits) is skipped and named;
/// the lowest parsed height is the head, and if none parses the head is refused by name with every node's
/// message, never defaulting to zero.
pub fn head_block(eps: &[Endpoint], chain: u64) -> Result<(u64, usize), Fault> {
    use zikaron_anchor::judge::{self, NoReading};
    // Ask only this chain's endpoints (as in `head_time`): mixing chains would take another chain's height.
    let eps: Vec<Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::filln(crate::lang::Key::Tail095, &[&(chain).to_string()])));
    }
    let empty = Value::Arr(Vec::new());
    let (answers, unread) = ask_all(&eps, "eth_blockNumber", &empty);
    let asked = answers.len() + unread.len();
    // Every parsed height, by node, for the head and the gap.
    let heights: Vec<(String, u64)> = answers.iter().filter_map(|(u, a)| a.as_ref().ok().and_then(height_of).map(|n| (u.clone(), n))).collect();
    // Quantities the judging table accepts that cannot be a height (over 64 bits), by node: skipped and named.
    let wide: Vec<String> = answers
        .iter()
        .filter_map(|(u, a)| match a {
            Ok(w) if height_of(w).is_none() && wire::to_core(w).is_some_and(|v| v.as_str().and_then(wei).is_some()) => {
                Some(crate::lang::filln(crate::lang::Key::Tail096, &[&zikaron_net::sayable(u), wire::write(w).trim_matches('"')]))
            }
            _ => None,
        })
        .collect();
    let said_height = |u: &str, m: &judge::Missing| match m {
        // A height given as text that is not a quantity is reported with the text; anything else is unreadable.
        judge::Missing::Unreadable(text) if text.starts_with('"') => crate::lang::filln(crate::lang::Key::Tail096, &[&zikaron_net::sayable(u), text.trim_matches('"')]),
        other => missing_said(u, other),
    };
    match judge::judge("eth_blockNumber", &empty, answers) {
        Ok(j) => {
            // The judging table read a quantity too wide for a height (over 64 bits): skipped here and named; if none
            // is left, refuse rather than use zero.
            let Some((low_at, low)) = heights.iter().min_by_key(|(_, h)| *h) else {
                let first = j.missing.iter().find_map(|(u, m)| match m {
                    judge::Missing::Trouble(t) => Some(said_fault(u, t)),
                    _ => None,
                });
                let refused: Vec<String> = unread.iter().cloned().chain(wide.iter().cloned()).chain(j.missing.iter().map(|(u, m)| said_height(u, m))).collect();
                return Err(none_answered(first, &refused));
            };
            if let Some((_, high)) = heights.iter().max_by_key(|(_, h)| *h) {
                if high - low > MAX_HEAD_LAG {
                    return Err(Fault::known(Known::Disagree, crate::lang::filln(crate::lang::Key::TailNodeBehind, &[&zikaron_net::sayable(low_at), &(high - low).to_string(), &high.to_string()])));
                }
            }
            Ok((*low, heights.len()))
        }
        Err(NoReading::NoneAnswered(missing)) => {
            let first = missing.iter().find_map(|(u, m)| match m {
                judge::Missing::Trouble(t) => Some(said_fault(u, t)),
                _ => None,
            });
            let refused: Vec<String> = unread.iter().cloned().chain(missing.iter().map(|(u, m)| said_height(u, m))).collect();
            if unread.len() == asked {
                return Err(no_address_reads(&refused));
            }
            Err(none_answered(first, &refused))
        }
        Err(_) => Err(Fault::known(Known::Unreachable, String::new())),
    }
}

/// One node's height as [`head_block`] parses it: a `0x` quantity that fits 64 bits (`None` otherwise).
fn height_of(w: &wire::W) -> Option<u64> {
    wire::to_core(w).and_then(|v| v.as_str().and_then(wei)).and_then(|n| u64::try_from(n).ok())
}

/// Chain time: the latest block's time and number. Only the basis chain's endpoints are asked, all at once
/// (mixing chains would take another chain's time); the latest block wins (the judging table's rule: highest
/// number). Load-bearing reads are conservative, and for deadlines and countdowns the conservative moment is
/// the later one: a closed window must never still show open. If nobody answers, it is refused by name. Every
/// `now` used for deadlines and countdowns comes from here; no wall clock enters a decision in this crate.
pub fn head_time(eps: &[Endpoint], chain: u64) -> Result<(u64, u64, usize), Fault> {
    let eps: Vec<Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::filln(crate::lang::Key::Tail097, &[&(chain).to_string()])));
    }
    let params = Value::Arr(vec![Value::Str("latest".into()), Value::Bool(false)]);
    let (answers, unread) = ask_all(&eps, "eth_getBlockByNumber", &params);
    let asked = answers.len() + unread.len();
    let mut seen: Vec<(u64, u64)> = Vec::new();
    let mut refused: Vec<String> = unread.clone();
    let mut first: Option<Fault> = None;
    for (url, got) in answers {
        match got {
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
                        _ => refused.push(crate::lang::filln(crate::lang::Key::Tail098, &[&zikaron_net::sayable(&url)])),
                    }
                }
                _ => refused.push(crate::lang::filln(crate::lang::Key::Tail094, &[&zikaron_net::sayable(&url)])),
            },
            Err(t) => {
                first.get_or_insert_with(|| said_fault(&url, &t));
                refused.push(trouble_said(&url, &t));
            }
        }
    }
    // The latest block: the highest number (the judging table's rule), with its time.
    match seen.iter().max_by_key(|(t, n)| (*n, *t)) {
        Some((t, n)) => Ok((*t, *n, seen.len())),
        None if unread.len() == asked => Err(no_address_reads(&refused)),
        None => Err(none_answered(first, &refused)),
    }
}

/// The chain each node reported serving, for the life of this process, keyed by endpoint
/// (`zikaron_net::place_key`): asked once per node, before the first balance or fee reading it takes part in.
static SERVES: std::sync::Mutex<std::collections::BTreeMap<String, u64>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// This chain's endpoints that actually serve it, each asked its chain id once per process (all at once). A
/// node that reports another chain is left out and named in the returned messages; a node that does not answer
/// is kept (not known to be wrong; its own reading will show). Every node left out is a [`Known::WrongChain`]
/// refusal naming it.
pub fn serving(eps: &[Endpoint], chain: u64) -> Result<(Vec<Endpoint>, Vec<String>), Fault> {
    let mine: Vec<Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
    let unknown: Vec<Endpoint> = {
        let known = SERVES.lock().unwrap_or_else(|e| e.into_inner());
        mine.iter().filter(|e| !known.contains_key(&e.url.place())).cloned().collect()
    };
    if !unknown.is_empty() {
        let (answers, _) = ask_all(&unknown, "eth_chainId", &Value::Arr(Vec::new()));
        let mut known = SERVES.lock().unwrap_or_else(|e| e.into_inner());
        for (url, got) in answers {
            if let Some(id) = got.ok().as_ref().and_then(wire::to_core).and_then(|v| v.as_str().and_then(wei)).filter(|n| *n <= u128::from(u64::MAX)) {
                known.insert(zikaron_net::place_key(&url), id as u64);
            }
        }
    }
    let known = SERVES.lock().unwrap_or_else(|e| e.into_inner());
    let mut kept = Vec::new();
    let mut left: Vec<String> = Vec::new();
    for e in mine {
        match known.get(&e.url.place()) {
            Some(id) if *id != chain => left.push(crate::lang::filln(crate::lang::Key::TailWrongChainNode, &[&e.url.said(), &id.to_string(), &chain.to_string()])),
            _ => kept.push(e),
        }
    }
    if kept.is_empty() && !left.is_empty() {
        return Err(Fault::known(Known::WrongChain, left.join(" · ")));
    }
    Ok((kept, left))
}

/// A transaction in full as one node of this chain holds it (`eth_getTransactionByHash`, not trimmed to the
/// judging table's facts): the first serving node that knows it answers. Used where one node's own answer is
/// what matters: a resend re-signs exactly the transaction a node holds, never an agreement of several.
pub fn tx_as_held(urls: &[NodeAddr], chain: u64, tx: &str) -> Option<Value> {
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint { chain, url: u.clone() }).collect();
    let (eps, _) = serving(&eps, chain).ok()?;
    let q = Value::Arr(vec![Value::Str(tx.to_string())]);
    eps.iter().find_map(|e| {
        let mut ep = endpoint_at(e.url.for_transport())?;
        let got = ep.call("eth_getTransactionByHash", &q).ok()?;
        wire::to_core(&got).filter(|v| matches!(v, Value::Obj(_)))
    })
}

/// Forgets every node's chain (for tests, or when what a node serves changes).
pub fn forget_serving() {
    SERVES.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

/// An address's balance (wei) on `chain`. A load-bearing read: several endpoints must agree.
///
/// Only that chain's endpoints are asked (another chain's balance is a different number), at one pinned block:
/// `latest` moves between requests, and two endpoints a block apart would disagree over a balance neither
/// disputes. The lowest head is one every endpoint has reached (`head_block`).
pub fn balance(eps: &[Endpoint], chain: u64, who: &Address) -> Result<(u128, Reading), Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::H4);
    let eps: Vec<Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
    // Only nodes that serve this chain are read (each asked its chain once per process); one serving another
    // chain is left out and named in the reading.
    let (eps, left) = serving(&eps, chain)?;
    let (height, _) = head_block(&eps, chain)?;
    let params = Value::Arr(vec![Value::Str(who.hex()), Value::Str(format!("0x{height:x}"))]);
    let mut r = ask(&eps, "eth_getBalance", &params)?;
    r.unanswered.splice(0..0, left);
    let hex = match &r.value {
        Value::Str(s) => s.clone(),
        other => return Err(Fault::known(Known::ChainShape, crate::lang::filln(crate::lang::Key::Tail099, &[&format!("{:?}", other)]))),
    };
    let n = wei(&hex).ok_or_else(|| Fault::known(Known::ChainShape, crate::lang::filln(crate::lang::Key::Tail100, &[&(hex).to_string()])))?;
    Ok((n, r))
}

/// Parses a `0x` quantity string.
pub fn wei(hex: &str) -> Option<u128> {
    let bare = hex.strip_prefix("0x")?;
    if bare.is_empty() || bare.len() > 32 || !bare.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u128::from_str_radix(bare, 16).ok()
}

/// This transaction's two fee fields (EIP-1559), from what this chain's endpoints agree on at one pinned
/// block: that block's `baseFeePerGas`, and the priority fees paid over the blocks up to it (`eth_feeHistory`);
/// the rule is `zikaron_anchor::send::Fees::of`, shared with the command line. Without a base fee the whole
/// pair is the fallback; without priority fees (unanswered, `null`, refused, differing, other shape) the
/// priority fee is its ceiling. Sending is not blocked either way; the balance check and confirmation card use
/// this same value, and the screen says which was used.
///
/// Also returns the nodes left out because they serve another chain, each named (as a balance reading does),
/// so the screen can say the fees were not read from them.
pub fn fees(eps: &[Endpoint], chain: u64) -> (zikaron_anchor::send::Fees, Vec<String>) {
    let r = fee_reading(eps, chain);
    (r.fees, r.left)
}

/// One fee reading ([`fees`]) with the base fee it was computed from (`None` if none was read).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeReading {
    pub base: Option<u64>,
    pub fees: zikaron_anchor::send::Fees,
    pub left: Vec<String>,
}

/// [`fees`], also returning the base fee read (the current price a stuck transaction's cap is compared
/// against).
pub fn fee_reading(eps: &[Endpoint], chain: u64) -> FeeReading {
    use zikaron_anchor::send::{tip_of, tip_params, Fees};
    let unread = |left: Vec<String>| FeeReading { base: None, fees: Fees::fallback(), left };
    // Only this chain's endpoints are asked: another chain's fees are a different number.
    // Only nodes that serve this chain (each asked its chain once per process); if none is left, the fallback,
    // with every node named.
    let (eps, left) = match serving(eps, chain) {
        Ok(x) => x,
        Err(f) => return unread(vec![f.tail().to_string()]),
    };
    // Ask about one pinned block, not `latest`: two endpoints a block apart would answer `latest` with
    // different blocks, the readings would disagree, and the fallback would replace a base fee every endpoint
    // knows. The lowest head is one every endpoint has reached (`head_block`).
    let Ok((height, _)) = head_block(&eps, chain) else {
        return unread(left);
    };
    let params = Value::Arr(vec![Value::Str(format!("0x{height:x}")), Value::Bool(false)]);
    // Require agreement on the base fee only (the judging table's rule for a pinned block): endpoints add
    // members of their own, and comparing whole blocks would let the fallback replace a base fee every endpoint
    // gave identically.
    let base = ask(&eps, "eth_getBlockByNumber", &params).ok().and_then(|r| zikaron_anchor::send::base_fee_of(&r.value));
    // The priority fees paid up to the same block, agreed over the facts the rule reads (the judging table's).
    let tip = ask(&eps, "eth_feeHistory", &tip_params(height)).ok().and_then(|r| tip_of(&r.value));
    FeeReading { base, fees: Fees::of(base, tip), left }
}
