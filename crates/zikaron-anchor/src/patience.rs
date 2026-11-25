use crate::rpc::{self, Endpoint, Trouble};
use crate::wire::W;
use std::time::Duration;
use zikaron::json::Value;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    RateLimited,
    CutBeforeAnswer,
    ServerError,
    Timeout,
    Other,
}

pub const RATE_LIMITED: [Duration; 2] = [Duration::from_millis(300), Duration::from_millis(900)];
pub const SERVER_ERROR: [Duration; 1] = [Duration::from_millis(300)];

pub const TABLE: [(Class, &[Duration]); 5] = [
    (Class::RateLimited, &RATE_LIMITED),
    (Class::CutBeforeAnswer, &[]),
    (Class::ServerError, &SERVER_ERROR),
    (Class::Timeout, &[]),
    (Class::Other, &[]),
];

const RANGE_WORDS: [&str; 3] = ["range", "results", "blocks"];

pub fn names_range(said: &str) -> bool {
    let low = said.to_lowercase();
    RANGE_WORDS.iter().any(|w| low.contains(w))
}

pub fn log_blocks(method: &str, params: &Value) -> Option<(u64, u64)> {
    let Value::Arr(ps) = params else { return None };
    let at = |k: &str| ps.first().and_then(|f| f.member(k)).and_then(Value::as_str).and_then(|h| h.strip_prefix("0x")).and_then(|h| u64::from_str_radix(h, 16).ok());
    (method == "eth_getLogs").then_some(())?;
    Some((at("fromBlock")?, at("toBlock")?))
}

pub fn log_span(method: &str, params: &Value) -> Option<u64> {
    let Value::Arr(ps) = params else { return None };
    let at = |k: &str| ps.first().and_then(|f| f.member(k)).and_then(Value::as_str).and_then(|h| h.strip_prefix("0x")).and_then(|h| u64::from_str_radix(h, 16).ok());
    (method == "eth_getLogs").then_some(())?;
    let (from, to) = (at("fromBlock")?, at("toBlock")?);
    Some(to.saturating_sub(from).saturating_add(1))
}

pub fn log_to(method: &str, params: &Value) -> Option<u64> {
    let Value::Arr(ps) = params else { return None };
    (method == "eth_getLogs").then_some(())?;
    ps.first().and_then(|f| f.member("toBlock")).and_then(Value::as_str).and_then(|h| h.strip_prefix("0x")).and_then(|h| u64::from_str_radix(h, 16).ok())
}

pub fn block_asked(method: &str, params: &Value) -> Option<u64> {
    if method == "eth_getLogs" {
        return log_to(method, params);
    }
    let Value::Arr(ps) = params else { return None };
    (method == "eth_getBlockByNumber").then_some(())?;
    ps.first().and_then(Value::as_str).and_then(|h| h.strip_prefix("0x")).filter(|h| !h.is_empty() && h.len() <= 16).and_then(|h| u64::from_str_radix(h, 16).ok())
}

pub fn short_of(method: &str, params: &Value, head: u64) -> bool {
    block_asked(method, params).is_some_and(|to| head < to)
}

pub fn past_it(ep: &mut dyn Endpoint, method: &str, params: &Value) -> bool {
    block_asked(method, params).is_some() && head_of(ep).is_some_and(|h| !short_of(method, params, h))
}

fn head_of(ep: &mut dyn Endpoint) -> Option<u64> {
    let w = ask(ep, "eth_blockNumber", &Value::Arr(Vec::new())).ok()?;
    crate::wire::to_core(&w)?.as_str()?.strip_prefix("0x").and_then(|h| u64::from_str_radix(h, 16).ok())
}

static ANSWERED: std::sync::Mutex<std::collections::BTreeMap<String, u64>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

pub fn answered_width(place: &str) -> Option<u64> {
    if let Some(n) = ANSWERED.lock().unwrap_or_else(|e| e.into_inner()).get(place).copied() {
        return Some(n);
    }
    if !kept_on_record(place) {
        return None;
    }
    let notes = crate::settled::notes()?;
    notes.widths.get(&crate::settled::spell(place)).filter(|w| w.stands(clock())).map(|w| w.blocks)
}

pub fn keep_answered(place: &str, blocks: u64) {
    if crate::endpoints::stopped() || blocks == 0 {
        return;
    }
    let was = ANSWERED.lock().unwrap_or_else(|e| e.into_inner()).insert(place.to_string(), blocks);
    if was != Some(blocks) && kept_on_record(place) {
        let now = clock();
        crate::settled::keep_width(&crate::settled::spell(place), crate::settled::Width { blocks, at: now }, now);
    }
}

pub fn forget_answered(place: &str) {
    ANSWERED.lock().unwrap_or_else(|e| e.into_inner()).remove(place);
}

fn kept_on_record(place: &str) -> bool {
    !place.starts_with("rest://")
}

fn clock() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn class_of(t: &Trouble) -> Class {
    let range = match t {
        Trouble::Node(e) => names_range(e),
        _ => false,
    };
    if crate::said::says_rate_limited(t) && !range {
        return Class::RateLimited;
    }
    match t {
        Trouble::Transport(_) if rpc::status_of(t).is_some_and(|s| (500..600).contains(&s)) => Class::ServerError,
        Trouble::Transport(rpc::Broke::Late { .. }) => Class::Timeout,
        _ => Class::Other,
    }
}

static WAITS: std::sync::Mutex<Option<Duration>> = std::sync::Mutex::new(None);

pub fn set_waits(d: Option<Duration>) {
    *WAITS.lock().unwrap_or_else(|e| e.into_inner()) = d;
    zikaron_net::set_dial_again(d);
}

pub fn waited(d: Duration) -> Duration {
    WAITS.lock().unwrap_or_else(|e| e.into_inner()).unwrap_or(d)
}

pub fn pauses(c: Class) -> Vec<Duration> {
    let row = TABLE.iter().find(|(k, _)| *k == c).map(|(_, p)| p.to_vec()).unwrap_or_default();
    match *WAITS.lock().unwrap_or_else(|e| e.into_inner()) {
        Some(d) => vec![d; row.len()],
        None => row,
    }
}

pub fn ask(ep: &mut dyn Endpoint, method: &str, params: &Value) -> Result<W, Trouble> {
    if method == rpc::BROADCAST || !ep.asks_a_node() {
        return ep.call(method, params);
    }
    if let Some(said) = crate::endpoints::lacking(&ep.place(), method) {
        return Err(Trouble::Node(said));
    }
    let mut taken: Vec<(Class, usize)> = Vec::new();
    loop {
        if crate::endpoints::stopped() {
            return Err(Trouble::Transport(rpc::Broke::Short));
        }
        let t = match ep.call(method, params) {
            Ok(w) => return Ok(w),
            Err(t) => t,
        };
        let c = class_of(&t);
        if c == Class::RateLimited && crate::endpoints::resting(&ep.place(), method).is_some() {
            return Err(t);
        }
        let n = taken.iter().find(|(k, _)| *k == c).map(|(_, n)| *n).unwrap_or(0);
        let waited = c == Class::RateLimited && ep.waits_out_limits(method);
        let Some(pause) = pauses(c).get(n).copied().filter(|_| !waited) else {
            if c == Class::RateLimited {
                crate::endpoints::rest(&ep.place(), method, &t);
            }
            return Err(t);
        };
        if !pause.is_zero() {
            std::thread::sleep(pause);
        }
        match taken.iter_mut().find(|(k, _)| *k == c) {
            Some((_, m)) => *m += 1,
            None => taken.push((c, 1)),
        }
    }
}

pub const AGAIN_AFTER: Duration = RATE_LIMITED[0];

pub fn answers_the_question(t: &Trouble) -> bool {
    use crate::said::Refusal;
    matches!(crate::said::refusal(t), Some(Refusal::Reverted | Refusal::Funds | Refusal::NonceUsed | Refusal::Pending | Refusal::Underpriced | Refusal::GasTooLow))
}

pub fn lacks_the_method(method: &str, t: &Trouble) -> bool {
    crate::said::refusal(t) == Some(crate::said::Refusal::NoMethod) && !(method == "eth_getLogs" && matches!(t, Trouble::Node(e) if names_range(e)))
}

pub fn refuses_the_question(t: &Trouble) -> bool {
    let Trouble::Node(e) = t else { return false };
    let code = crate::wire::parse(e.as_bytes()).and_then(|w| {
        w.member("code").and_then(|c| match &c.body {
            crate::wire::Body::Num(n) => n.parse::<i64>().ok(),
            _ => None,
        })
    });
    names_range(e) || code == Some(-32602)
}

pub fn stands_aside(t: &Trouble) -> bool {
    match t {
        Trouble::Transport(rpc::Broke::Unserved { .. } | rpc::Broke::NoKey { .. }) => true,
        Trouble::Node(_) => refuses_the_question(t) || (crate::said::refusal(t) == Some(crate::said::Refusal::NoMethod) && !crate::said::says_rate_limited(t)),
        _ => false,
    }
}

pub fn asked_again(method: &str, t: &Trouble) -> bool {
    matches!(t, Trouble::Node(_)) && class_of(t) == Class::Other && !answers_the_question(t) && !lacks_the_method(method, t)
}

pub fn ask_again_once(ep: &mut dyn Endpoint, method: &str, params: &Value) -> Result<W, Trouble> {
    match ask(ep, method, params) {
        Err(t) if asked_again(method, &t) && method != rpc::BROADCAST && ep.asks_a_node() => {
            let pause = waited(AGAIN_AFTER);
            if !pause.is_zero() {
                std::thread::sleep(pause);
            }
            ask(ep, method, params)
        }
        got => got,
    }
}

#[cfg(test)]
mod block_tests {
    use super::*;

    fn logs(from: u64, to: u64) -> Value {
        Value::Arr(vec![Value::Obj(vec![("fromBlock".into(), Value::Str(format!("0x{from:x}"))), ("toBlock".into(), Value::Str(format!("0x{to:x}")))])])
    }

    #[test]
    fn the_block_a_question_asks_is_its_numbered_block_or_its_last_log_block() {
        let block = |b: &str| Value::Arr(vec![Value::Str(b.into()), Value::Bool(false)]);
        assert_eq!(block_asked("eth_getBlockByNumber", &block("0x10")), Some(16));
        assert_eq!(block_asked("eth_getLogs", &logs(0, 1_000)), Some(1_000));
        for (m, p) in [("eth_getBlockByNumber", block("latest")), ("eth_getBlockByNumber", block("0x")), ("eth_getBlockByNumber", block("0x10000000000000000")), ("eth_getBlockByHash", block("0x10")), ("eth_blockNumber", Value::Arr(Vec::new()))] {
            assert_eq!(block_asked(m, &p), None, "{m} {p:?}");
        }
        assert!(short_of("eth_getBlockByNumber", &block("0x10"), 15) && !short_of("eth_getBlockByNumber", &block("0x10"), 16) && !short_of("eth_getBlockByNumber", &block("latest"), 0));
    }
}
