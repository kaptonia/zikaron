//! Endpoints: a JSON-RPC question-and-answer channel, plus recording and replay.
//!
//! A recording is a scan's whole conversation with a node. On replay, a question the recording lacks is an
//! error, never a guess: filling in a "probably empty" answer would let a recording with a hole quietly
//! answer something else. So [`Replay`] returns [`Trouble::NotServed`] and the scan stops.
//!
//! Answers are matched by (method, params), not by order: a scan may ask the same question twice and the
//! order is up to the scan. The key is the method plus the canonical bytes of the params; two entries with
//! the same key and different answers make the recording self-contradictory.
//!
//! One closed rule widens a question: a log question naming senders (a second topic) that the recording
//! lacks is answered with what the recording holds for the same question without that topic, handed back as
//! recorded. The replay does not filter it; the scan judges every log it gets. Recordings made before log
//! questions named senders thus answer as they did. No other question is ever widened.

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
// Timeout, answer too long and answer not JSON are said by the real endpoints (`Http` here, the app's https
// adapter) and recognized by the chain client's closed "what the node said" type. Saying and recognizing go
// through the functions below only.

/// This call's total deadline passed (`url` did not finish within `secs` seconds).
pub fn late(url: &str, deadline: std::time::Duration) -> Trouble {
    let url = &zikaron_net::sayable(url);
    // Whole seconds as seconds; a millisecond deadline (`ZKA_TIMEOUT_MS`) as fractional seconds. The tail
    // `is_late` recognizes stays the same.
    let said = if deadline.subsec_millis() == 0 { deadline.as_secs().to_string() } else { format!("{:.3}", deadline.as_secs_f64()) };
    Trouble::Transport(format!("{url} 在 {said} 秒里没把话说完"))
}

/// The answer passed its end (`max` bytes).
pub fn overlong(url: &str, max: usize) -> Trouble {
    let url = &zikaron_net::sayable(url);
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

/// The words between the address and the status in [`status`].
const AT_STATUS: &str = " 答 HTTP ";

/// The answer was not the node's word, and came with this HTTP status (a gateway's page, a bare refusal).
pub fn status(url: &str, code: u16) -> Trouble {
    let url = &zikaron_net::sayable(url);
    Trouble::Transport(format!("{url}{AT_STATUS}{code}"))
}

/// The HTTP status a sentence says the answer came with ([`status`]); `None` for any other sentence.
pub fn status_of(said: &str) -> Option<u16> {
    let (_, code) = said.rsplit_once(AT_STATUS)?;
    code.parse().ok().filter(|_| code.bytes().all(|b| b.is_ascii_digit()))
}

/// The answer is JSON, but not a JSON-RPC answer to this question (no `result` and no `error`, both, an
/// array, another question's `id`).
pub fn shapeless(url: &str) -> Trouble {
    let url = &zikaron_net::sayable(url);
    Trouble::Transport(format!("{url} 的答不是这一问成形的应答"))
}

/// Whether a sentence says the answer was JSON without the shape of an answer ([`shapeless`]).
pub fn is_shapeless(said: &str) -> bool {
    said.ends_with("的答不是这一问成形的应答")
}

/// The question a broadcast asks: carried once, on a new connection, never asked again. Named here, where
/// the answer is read, and nowhere else.
pub const BROADCAST: &str = "eth_sendRawTransaction";

/// Whether an answer is the node's word to the question numbered `id`, and if so what it says: a JSON
/// object carrying `result` with `error` absent or null and that `id` (a null `result` is a valid empty
/// answer), or a non-null `error` with `result` absent and that `id`, a null `id` or none (JSON-RPC lets a
/// node that could not read the request's id answer its error so: a gateway's rate limit often does). Both,
/// neither, an array, a result under another or no `id`, an error under another `id`: `None`.
fn node_word(v: &W, id: u64) -> Option<Result<W, Trouble>> {
    if !matches!(v.body, Body::Obj(_)) {
        return None;
    }
    let ours = v.member("id").and_then(|i| i.as_u64()) == Some(id);
    let unread = v.member("id").is_none_or(|i| i.is_null());
    let error = v.member("error").filter(|e| !e.is_null());
    match (v.member("result"), error) {
        (Some(r), None) if ours => Some(Ok(r.clone())),
        (None, Some(e)) if ours || unread => Some(Err(Trouble::Node(wire::write(e)))),
        _ => None,
    }
}

/// The one reading of a node's answer, for every endpoint. The node's word ([`node_word`]) is read whatever
/// the HTTP status (nodes often send a well-formed error with a 4xx or 5xx). Anything else is read by the
/// status: 429, 401, 403 and every other status outside 2xx are named with their status ([`status`]); a 2xx
/// answer that is not JSON is [`NOT_JSON`]; one that is JSON without the shape of an answer is
/// [`shapeless`]. Nothing else ever reads as an answer: a gateway's JSON page is not "the result is empty".
pub fn read_answer(url: &str, id: u64, got: &zikaron_net::Answer) -> Result<W, Trouble> {
    let v = wire::parse(&got.body);
    if let Some(said) = v.as_ref().and_then(|v| node_word(v, id)) {
        return said;
    }
    match (got.status, v) {
        (s, _) if !(200..300).contains(&s) => Err(status(url, s)),
        (_, None) => Err(Trouble::Transport(NOT_JSON.into())),
        (_, Some(_)) => Err(shapeless(url)),
    }
}

/// Ask a node one question over the one transport and read its answer ([`read_answer`]): the one place the
/// command line's endpoint ([`Http`]) and the app's https endpoint both ask through. A broadcast
/// ([`BROADCAST`]) is carried once on a new connection; every other question is a read. `said` puts a
/// transport failure in the caller's words.
pub fn ask_node(
    url: &str,
    target: &zikaron_net::Target,
    limits: &Limits,
    id: u64,
    method: &str,
    params: &Value,
    said: &dyn Fn(zikaron_net::Fail) -> Trouble,
) -> Result<W, Trouble> {
    let body = canon_bytes(&Value::Obj(vec![
        ("id".into(), Value::Int(id)),
        ("jsonrpc".into(), Value::Str("2.0".into())),
        ("method".into(), Value::Str(method.into())),
        ("params".into(), params.clone()),
    ]));
    let how = if method == BROADCAST { zikaron_net::Ask::Once } else { zikaron_net::Ask::Read };
    let got = zikaron_net::post_json(target, &body, limits, how).map_err(said)?;
    read_answer(url, id, &got)
}

/// `limits` with a deadline no longer than `within` (none stays none only when `within` is zero).
pub fn within(limits: Limits, within: std::time::Duration) -> Limits {
    let deadline = match (limits.deadline.is_zero(), within.is_zero()) {
        (_, true) => limits.deadline,
        (true, false) => within,
        (false, false) => limits.deadline.min(within),
    };
    Limits { deadline, ..limits }
}

/// A question-and-answer channel.
pub trait Endpoint {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble>;
    /// [`Endpoint::call`] given no longer than `within` (a wait with a deadline of its own, as a receipt
    /// wait has). Channels without a clock (a recording) ask as `call` does.
    fn call_within(&mut self, method: &str, params: &Value, within: std::time::Duration) -> Result<W, Trouble> {
        let _ = within;
        self.call(method, params)
    }
    /// A name for people (which endpoint a reading came from).
    fn name(&self) -> String;
    /// The key that keeps facts about different endpoints apart (e.g. a per-node cache). Unlike the name
    /// ([`zikaron_net::sayable`]: scheme, host and port), it includes the path ([`zikaron_net::place_key`]), so
    /// two endpoints on one host stay distinct. There is no default, so a wrapper cannot silently merge places by
    /// name; a channel without an address returns its name.
    fn place(&self) -> String;
}

fn key(method: &str, params: &Value) -> String {
    let mut k = String::from(method);
    k.push(' ');
    k.push_str(&String::from_utf8_lossy(&canon_bytes(params)));
    k
}

/// Recording keys use one rule: params inside the zikaron-v1 §3 value domain use canonical bytes (as the asking side
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
            // Read as a live answer is read: `result` with `error` absent or null, or a non-null `error`
            // without `result`. An exchange holding neither, or both, holds no answer: the question is not
            // served, never replayed as an empty result.
            let a = match (e.member("result"), e.member("error").filter(|x| !x.is_null())) {
                (Some(r), None) => Ok(r.clone()),
                (None, Some(err)) => Err(wire::write(err)),
                _ => continue,
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
    /// The recorded answer to a question. Not recorded is an error, never a guess: by zikaron-v1 §9.4 an
    /// incomplete read gives no report.
    pub fn answer(&mut self, k: &str) -> Result<W, Trouble> {
        match self.answers.get(k) {
            None => Err(Trouble::NotServed(k.to_string())),
            Some(Ok(v)) => Ok(v.clone()),
            Some(Err(e)) => Err(Trouble::Node(e.clone())),
        }
    }
}

/// The one wider question a replay may answer in place of a log question naming senders: the same question
/// with only its first topic (`None` for every other question).
fn without_senders(method: &str, params: &Value) -> Option<Value> {
    if method != "eth_getLogs" {
        return None;
    }
    let Value::Arr(ps) = params else { return None };
    let [Value::Obj(filter)] = ps.as_slice() else { return None };
    let at = filter.iter().position(|(k, _)| k == "topics")?;
    let Value::Arr(topics) = &filter[at].1 else { return None };
    if topics.len() != 2 {
        return None;
    }
    let mut wider = filter.clone();
    wider[at].1 = Value::Arr(vec![topics[0].clone()]);
    Some(Value::Arr(vec![Value::Obj(wider)]))
}

impl Endpoint for Replay {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        let k = key(method, params);
        self.asked.push(k.clone());
        if !self.answers.contains_key(&k) {
            if let Some(wider) = without_senders(method, params) {
                let w = key(method, &wider);
                if self.answers.contains_key(&w) {
                    return self.answer(&w);
                }
            }
        }
        self.answer(&k)
    }
    fn name(&self) -> String {
        self.who.clone()
    }
    /// No address: the place is the name.
    fn place(&self) -> String {
        self.name()
    }
}

/// The environment names and the two bounds of one call live in the transport (`zikaron_net`); they are
/// named here too, so the callers of this crate keep one path.
pub use zikaron_net::{env, Limits};

/// The way out (straight, or through a proxy) is the transport's; the command line chooses it through here,
/// so the callers of this crate keep one path.
pub use zikaron_net::{proxy_of, read_address, set_choice, Choice, NotAProxy, NotAnAddress, SystemProxies};

/// Why an endpoint spelling does not read. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotAnEndpoint {
    /// No `=` between the chain id and the address.
    NoEquals,
    /// The chain id is not a whole number that fits 64 bits.
    ChainNotInt,
    /// Nothing after the `=` (white space alone included).
    NoAddress,
    /// White space inside the value (around the `=`, inside the address): one item holds none, so a spelling
    /// cut at white space (the app's cell) and one taken whole (the command line) never read it two ways.
    InnerSpace,
}

/// The one reading of an endpoint's spelling, `<chain id>=<address>`, for the app's cells and the command
/// line's `--endpoint` alike: split at the first `=` (an address may hold more); white space at either end of
/// the value is not part of it, and white space anywhere else (around the `=`, inside the address) is
/// [`NotAnEndpoint::InnerSpace`]; the chain id is a decimal whole number within 64 bits (leading zeros and a
/// leading `+` read as the same number); the address must not be empty. Read left to right: a chain id that
/// does not read is said before white space after it. The address itself is the transport's to read
/// (`zikaron_net::parse`), where it is used.
pub fn endpoint_spec(spec: &str) -> Result<(u64, String), NotAnEndpoint> {
    let (c, address) = spec.split_once('=').ok_or(NotAnEndpoint::NoEquals)?;
    let chain: u64 = c.trim().parse().map_err(|_| NotAnEndpoint::ChainNotInt)?;
    let address = address.trim();
    if address.is_empty() {
        return Err(NotAnEndpoint::NoAddress);
    }
    if spec.trim().chars().any(char::is_whitespace) {
        return Err(NotAnEndpoint::InnerSpace);
    }
    Ok((chain, address.to_string()))
}

/// A real endpoint: JSON-RPC over the one transport (`zikaron_net`), `http` or `https`.
///
/// The address is read by `zikaron_net::parse` (scheme and host without regard to case), the exchange and the
/// TLS configuration are the transport's; this side builds the JSON-RPC request and reads its answer.
pub struct Http {
    url: String,
    target: zikaron_net::Target,
    next_id: u64,
    limits: Limits,
}

impl Http {
    /// `http://host[:port]/path` or `https://…`, the scheme in any case; any other spelling gives `None`.
    pub fn new(url: &str) -> Option<Http> {
        let target = zikaron_net::parse(url)?;
        Some(Http { url: url.trim().to_string(), target, next_id: 1, limits: Limits::from_env() })
    }

    /// Replace the bounds (when the caller sets a deadline).
    pub fn with_limits(mut self, limits: Limits) -> Http {
        self.limits = limits;
        self
    }
}

/// A transport failure as this crate says it: the deadline and the cap in their recognized sentences
/// ([`late`], [`overlong`]), every other layer with the endpoint and the layer's own words.
pub fn transport_said(url: &str, f: &zikaron_net::Fail) -> Trouble {
    let url = &zikaron_net::sayable(url);
    match f {
        zikaron_net::Fail::Late(d) => late(url, *d),
        zikaron_net::Fail::Overlong(max) => overlong(url, *max),
        zikaron_net::Fail::Name(x)
        | zikaron_net::Fail::Connect(x)
        | zikaron_net::Fail::Handshake(x)
        | zikaron_net::Fail::Certificate(x)
        | zikaron_net::Fail::Stream(x) => Trouble::Transport(format!("{url}: {x}")),
        zikaron_net::Fail::Closed => Trouble::Transport(format!("{url}: 这一问随收场停下")),
    }
}

impl Http {
    fn ask(&mut self, method: &str, params: &Value, limits: Limits) -> Result<W, Trouble> {
        let id = self.next_id;
        self.next_id += 1;
        let url = self.url.clone();
        ask_node(&self.url, &self.target, &limits, id, method, params, &|f| transport_said(&url, &f))
    }
}

impl Endpoint for Http {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        self.ask(method, params, self.limits)
    }
    fn call_within(&mut self, method: &str, params: &Value, within: std::time::Duration) -> Result<W, Trouble> {
        self.ask(method, params, self::within(self.limits, within))
    }
    fn name(&self) -> String {
        zikaron_net::sayable(&self.url)
    }
    fn place(&self) -> String {
        zikaron_net::place_key(&self.url)
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

impl Recorder {
    fn record(&mut self, method: &str, params: &Value, out: Result<W, Trouble>) -> Result<W, Trouble> {
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
}

impl Endpoint for Recorder {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        let out = self.inner.call(method, params);
        self.record(method, params, out)
    }
    fn call_within(&mut self, method: &str, params: &Value, within: std::time::Duration) -> Result<W, Trouble> {
        let out = self.inner.call_within(method, params, within);
        self.record(method, params, out)
    }
    fn name(&self) -> String {
        self.inner.name()
    }
    fn place(&self) -> String {
        self.inner.place()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two endpoints on one host with different paths share a name but are two places; a recording's place is
    /// its name.
    #[test]
    fn two_nodes_said_alike_are_two_places() {
        let eth = Http::new("https://node.example/eth/k1").expect("an address");
        let polygon = Http::new("https://node.example/polygon/k2").expect("an address");
        assert_eq!(eth.name(), polygon.name(), "said alike: scheme, host and port only");
        assert_ne!(eth.place(), polygon.place(), "two places");
        assert_eq!(Http::new("HTTPS://Node.Example:443/eth/k1").expect("an address").place(), eth.place(), "one place however it is written");
        let r = Replay::new("a recording", &[]).expect("an empty recording");
        assert_eq!(r.place(), r.name());
    }
}
