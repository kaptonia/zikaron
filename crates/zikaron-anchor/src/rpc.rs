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
}

/// The environment names and the two bounds of one call live in the transport (`zikaron_net`); they are
/// named here too, so the callers of this crate keep one path.
pub use zikaron_net::{env, Limits};

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
    match f {
        zikaron_net::Fail::Late(d) => late(url, *d),
        zikaron_net::Fail::Overlong(max) => overlong(url, *max),
        zikaron_net::Fail::Name(x)
        | zikaron_net::Fail::Connect(x)
        | zikaron_net::Fail::Handshake(x)
        | zikaron_net::Fail::Certificate(x)
        | zikaron_net::Fail::Stream(x) => Trouble::Transport(format!("{url}: {x}")),
    }
}

impl Endpoint for Http {
    fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
        let id = self.next_id;
        self.next_id += 1;
        let body = canon_bytes(&Value::Obj(vec![
            ("id".into(), Value::Int(id)),
            ("jsonrpc".into(), Value::Str("2.0".into())),
            ("method".into(), Value::Str(method.into())),
            ("params".into(), params.clone()),
        ]));
        let got = zikaron_net::post_json(&self.target, &body, &self.limits).map_err(|f| transport_said(&self.url, &f))?;
        let v = wire::parse(&got.body).ok_or(Trouble::Transport(NOT_JSON.into()))?;
        if let Some(err) = v.member("error") {
            return Err(Trouble::Node(wire::write(err)));
        }
        Ok(v.member("result").cloned().unwrap_or(W::of(Body::Null)))
    }
    fn name(&self) -> String {
        self.url.clone()
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
