use crate::endpoints::{self, distinct_places, Disagreement};
use crate::rpc::Trouble;
use crate::wire::{self, W};
use std::time::Duration;
use zikaron::json::Value;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    Same,
    Facts(&'static [&'static str]),
    Reached,
    Most(Option<&'static str>),
    Middle(Statistic),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Statistic {
    PaidTip,
}

impl Statistic {
    pub fn facts(self) -> &'static [&'static str] {
        match self {
            Statistic::PaidTip => &FEE_HISTORY_FACTS,
        }
    }

    pub fn of(self, v: &Value) -> Option<u128> {
        match self {
            Statistic::PaidTip => crate::send::tip_of(v).map(u128::from),
        }
    }
}

pub const BLOCK_FACTS: [&str; 1] = ["baseFeePerGas"];
pub const FEE_HISTORY_FACTS: [&str; 1] = ["reward"];
pub const TX_FACTS: [&str; 3] = ["blockNumber", "from", "input"];
pub const RECEIPT_FACTS: [&str; 2] = ["blockNumber", "status"];

pub const NOT_YET_METHODS: [&str; 2] = ["eth_getTransactionByHash", "eth_getTransactionReceipt"];

pub const NOT_YET_PAUSES: [Duration; 4] = [Duration::from_millis(500), Duration::from_millis(1500), Duration::from_secs(4), Duration::from_secs(7)];

fn not_yet(method: &str, v: &Value) -> bool {
    matches!(v, Value::Null) || (NOT_YET_METHODS.contains(&method) && matches!(v.member("blockNumber"), Some(Value::Null)))
}

pub fn reached<T: AsRef<str>, H: Ord + Copy>(heights: &[(T, H)]) -> Option<(usize, H)> {
    let all = distinct_places(heights.iter().map(|(n, _)| n.as_ref()));
    let need = all.min(2);
    let mut order: Vec<usize> = (0..heights.len()).collect();
    order.sort_by(|a, b| heights[*b].1.cmp(&heights[*a].1));
    let mut seen: Vec<&str> = Vec::new();
    for i in order {
        seen.push(heights[i].0.as_ref());
        if need > 0 && distinct_places(seen.iter().copied()) >= need {
            return Some((i, heights[i].1));
        }
    }
    None
}

pub fn facts_of(method: &str, params: &Value) -> Option<&'static [&'static str]> {
    match rule_of(method, params) {
        Rule::Facts(facts) => Some(facts),
        Rule::Middle(s) => Some(s.facts()),
        _ => None,
    }
}

pub fn read_as_decided(method: &str, params: &Value, w: &W) -> Option<Value> {
    match facts_of(method, params) {
        Some(facts) => wire::to_core(&wire::project(w, facts)),
        None => wire::to_core(w),
    }
}

pub fn rule_of(method: &str, params: &Value) -> Rule {
    let first = match params {
        Value::Arr(a) => a.first(),
        _ => None,
    };
    match method {
        "eth_blockNumber" => Rule::Reached,
        "eth_getTransactionCount" => Rule::Most(None),
        "eth_estimateGas" => Rule::Most(None),
        "eth_getBlockByNumber" if matches!(first, Some(Value::Str(s)) if s == "latest") => Rule::Most(Some("number")),
        "eth_getBlockByNumber" => Rule::Facts(&BLOCK_FACTS),
        "eth_feeHistory" => Rule::Middle(Statistic::PaidTip),
        "eth_getTransactionByHash" => Rule::Facts(&TX_FACTS),
        "eth_getTransactionReceipt" => Rule::Facts(&RECEIPT_FACTS),
        _ => Rule::Same,
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Missing {
    Trouble(Trouble),
    Unreadable(String),
}

impl From<Trouble> for Missing {
    fn from(t: Trouble) -> Missing {
        Missing::Trouble(t)
    }
}

enum Read {
    Value(Value),
    Shapeless(String),
    NotNumber(String),
}

fn read_by_rule(method: &str, params: &Value, w: &W) -> Read {
    let rule = rule_of(method, params);
    let Some(v) = read_as_decided(method, params, w) else { return Read::Shapeless(wire::write(w)) };
    if matches!(rule, Rule::Reached | Rule::Most(_) | Rule::Middle(_)) && number_of(rule, &v).is_none() {
        return Read::NotNumber(String::from_utf8_lossy(&zikaron::json::canon_bytes(&v)).into_owned());
    }
    Read::Value(v)
}

fn number_of(rule: Rule, v: &Value) -> Option<u128> {
    match rule {
        Rule::Most(Some(member)) => v.member(member).and_then(quantity),
        Rule::Middle(s) => s.of(v),
        _ => quantity(v),
    }
}

pub fn unreadable(method: &str, params: &Value, w: &W) -> Option<Missing> {
    match read_by_rule(method, params, w) {
        Read::Value(_) => None,
        Read::NotNumber(_) if matches!(rule_of(method, params), Rule::Middle(_)) => None,
        Read::Shapeless(text) | Read::NotNumber(text) => Some(Missing::Unreadable(text)),
    }
}

#[derive(Clone, Debug)]
pub struct Judged {
    pub value: Value,
    pub sources: Vec<String>,
    pub missing: Vec<(String, Missing)>,
    pub single_source: bool,
    pub differed: Vec<String>,
}

#[derive(Clone, Debug)]
pub enum NoReading {
    NoneAnswered(Vec<(String, Missing)>),
    Differ(Disagreement),
    NotYet { has: Vec<String>, not_yet: Vec<String> },
}

type Named = (String, Value);

fn quantity(v: &Value) -> Option<u128> {
    let s = v.as_str()?.strip_prefix("0x")?;
    if s.is_empty() || s.len() > 32 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u128::from_str_radix(s, 16).ok()
}

pub fn judge(method: &str, params: &Value, answers: Vec<(String, Result<W, Trouble>)>) -> Result<Judged, NoReading> {
    judge_read(method, params, answers.into_iter().map(|(n, got)| (n, got.map_err(Missing::from))).collect())
}

pub fn judge_read(method: &str, params: &Value, answers: Vec<(String, Result<W, Missing>)>) -> Result<Judged, NoReading> {
    judged(method, params, answers, true)
}

pub fn judge_settled(method: &str, params: &Value, answers: Vec<(String, Result<W, Missing>)>) -> Result<Judged, NoReading> {
    judged(method, params, answers, false)
}

fn judged(method: &str, params: &Value, answers: Vec<(String, Result<W, Missing>)>, waiting: bool) -> Result<Judged, NoReading> {
    let rule = rule_of(method, params);
    let mut read: Vec<(String, Value)> = Vec::new();
    let mut missing: Vec<(String, Missing)> = Vec::new();
    let mut not_numbers: Vec<(String, Missing)> = Vec::new();
    for (name, got) in answers {
        match got {
            Err(m) => missing.push((name, m)),
            Ok(w) => match read_by_rule(method, params, &w) {
                Read::Value(v) => read.push((name, v)),
                Read::Shapeless(text) => missing.push((name, Missing::Unreadable(text))),
                Read::NotNumber(text) => not_numbers.push((name, Missing::Unreadable(text))),
            },
        }
    }
    missing.extend(not_numbers);
    if matches!(rule, Rule::Reached | Rule::Most(_) | Rule::Middle(_)) {
        let good = read;
        let key = |v: &Value| number_of(rule, v);
        let chosen = match rule {
            Rule::Reached => {
                let heights: Vec<(&str, u128)> = good.iter().map(|(n, v)| (n.as_str(), key(v).unwrap_or(0))).collect();
                reached(&heights).and_then(|(i, _)| good.get(i))
            }
            Rule::Middle(_) => {
                let mut order: Vec<&Named> = good.iter().collect();
                order.sort_by_key(|(_, v)| key(v));
                order.get(order.len().saturating_sub(1) / 2).copied()
            }
            _ => good.iter().max_by_key(|(_, v)| key(v)),
        };
        let Some((_, value)) = chosen else { return Err(NoReading::NoneAnswered(missing)) };
        let sources: Vec<String> = good.iter().map(|(n, _)| n.clone()).collect();
        let single_source = distinct_places(sources.iter().map(String::as_str)) < 2;
        return Ok(stood(Judged { value: value.clone(), sources, missing, single_source, differed: Vec::new() }));
    }
    if read.is_empty() {
        return Err(NoReading::NoneAnswered(missing));
    }
    let (later, has): (Vec<Named>, Vec<Named>) = read.into_iter().partition(|(_, v)| not_yet(method, v));
    if has.is_empty() {
        if later.iter().any(|(_, v)| matches!(v, Value::Null)) && later.iter().any(|(_, v)| !matches!(v, Value::Null)) {
            let sources: Vec<String> = later.iter().map(|(n, _)| n.clone()).collect();
            let single_source = distinct_places(sources.iter().map(String::as_str)) < 2;
            return Ok(stood(Judged { value: Value::Null, sources, missing, single_source, differed: Vec::new() }));
        }
        let names: Vec<String> = later.iter().map(|(n, _)| n.clone()).collect();
        let runs: Vec<(String, Value)> = match rule {
            Rule::Facts(facts) => later.into_iter().map(|(n, v)| (n, endpoints::project(&v, facts))).collect(),
            _ => later,
        };
        return match endpoints::agree(runs) {
            Ok(r) => {
                let differed = endpoints::left_out(&names, &r);
                Ok(stood(Judged { value: r.fragment, single_source: r.single_source, sources: r.sources, missing, differed }))
            }
            Err(d) => Err(NoReading::Differ(d)),
        };
    }
    let mut runs: Vec<(String, Value)> = match rule {
        Rule::Facts(facts) => has.into_iter().map(|(n, v)| (n, endpoints::project(&v, facts))).collect(),
        _ => has,
    };
    if method == "eth_getLogs" {
        let behind: Vec<String> = runs
            .iter()
            .filter(|(n, v)| crate::rest::of_lane(n).is_some() && runs.iter().any(|(m, w)| m != n && matches!((v, w), (Value::Arr(a), Value::Arr(b)) if endpoints::only_behind(a, b, log_block))))
            .map(|(n, _)| n.clone())
            .collect();
        if !behind.is_empty() && behind.len() < runs.len() {
            runs.retain(|(n, _)| !behind.contains(n));
            missing.extend(behind.into_iter().map(|n| (n.clone(), Missing::Trouble(Trouble::Transport(crate::rpc::Broke::LogsBehind { node: zikaron_net::sayable(&n) })))));
        }
    }
    let names: Vec<String> = runs.iter().map(|(n, _)| n.clone()).collect();
    let waits = |sources: &[String]| (waiting || NOT_YET_METHODS.contains(&method)) && !later.is_empty() && distinct_places(sources.iter().map(String::as_str)) < 2;
    match endpoints::agree(runs) {
        Ok(r) if waits(&r.sources) => Err(NoReading::NotYet { has: r.sources, not_yet: later.into_iter().map(|(n, _)| n).collect() }),
        Ok(r) => {
            let differed = endpoints::left_out(&names, &r);
            Ok(stood(Judged { value: r.fragment, single_source: r.single_source, sources: r.sources, missing, differed }))
        }
        Err(_) if NOT_YET_METHODS.contains(&method) && !later.is_empty() => Err(NoReading::NotYet { has: names, not_yet: later.into_iter().map(|(n, _)| n).collect() }),
        Err(d) => Err(NoReading::Differ(d)),
    }
}

fn stood(j: Judged) -> Judged {
    if j.single_source {
        return j;
    }
    Judged { missing: Vec::new(), differed: Vec::new(), ..j }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Enough {
    Alike,
    Reached,
    Conservative,
    Void,
    Proves,
}

pub fn enough_of(method: &str, params: &Value) -> Enough {
    match rule_of(method, params) {
        Rule::Same | Rule::Facts(_) => Enough::Alike,
        Rule::Reached => Enough::Reached,
        Rule::Most(_) | Rule::Middle(_) => Enough::Conservative,
    }
}

pub fn enough(method: &str, params: &Value, arrived: &[(String, Result<W, Missing>)], floor: Option<u64>) -> bool {
    let rule = rule_of(method, params);
    let read: Vec<(&str, Value)> = arrived
        .iter()
        .filter_map(|(n, a)| match a {
            Ok(w) => match read_by_rule(method, params, w) {
                Read::Value(v) => Some((n.as_str(), v)),
                _ => None,
            },
            Err(_) => None,
        })
        .collect();
    match enough_of(method, params) {
        Enough::Reached | Enough::Conservative => {
            let numbered = read.iter().filter(|(_, v)| number_of(rule, v).is_some_and(|x| floor.is_none_or(|f| x >= u128::from(f)))).map(|(n, _)| *n);
            distinct_places(numbered) >= 2
        }
        _ => {
            let facts: Vec<(&str, Vec<u8>)> = read
                .iter()
                .filter(|(_, v)| !not_yet(method, v))
                .map(|(n, v)| (*n, zikaron::json::canon_bytes(&match rule { Rule::Facts(f) => endpoints::project(v, f), _ => v.clone() })))
                .collect();
            two_places_alike(&facts)
        }
    }
}

pub fn two_places_alike<N: AsRef<str>>(answers: &[(N, Vec<u8>)]) -> bool {
    alike_among(answers).is_some()
}

pub fn alike_among<N: AsRef<str>>(answers: &[(N, Vec<u8>)]) -> Option<usize> {
    answers.iter().position(|(_, b)| distinct_places(answers.iter().filter(|(_, x)| x == b).map(|(n, _)| n.as_ref())) >= 2)
}

pub fn not_yet_pauses() -> Vec<Duration> {
    NOT_YET_PAUSES.iter().map(|d| crate::patience::waited(*d)).collect()
}

fn log_block(log: &Value) -> Option<u64> {
    log.member("blockNumber").and_then(Value::as_str).and_then(|x| u64::from_str_radix(x.trim_start_matches("0x"), 16).ok())
}
