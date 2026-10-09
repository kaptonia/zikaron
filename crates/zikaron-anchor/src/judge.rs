//! The judging table: how the answers several nodes gave to one question become one reading. One table, by
//! method ([`rule_of`]); every question the app asks of several nodes is judged here.
//!
//! | rule | methods | reading |
//! |---|---|---|
//! | same bytes | every method not named below | the answers must be byte for byte the same |
//! | named facts | a pinned block (`eth_getBlockByNumber` at a number), the fee history, a transaction, a receipt | only the named members are compared; a node lacking one of them differs |
//! | least | the head (`eth_blockNumber`) | the smallest height (a block every node has reached) |
//! | most | the pending nonce (`eth_getTransactionCount`), the latest block (`eth_getBlockByNumber` at `latest`) | the largest |
//!
//! Every rule shares four rules of its own:
//! - a node that does not answer is passed over: the reading still comes, from the others, and says which did
//!   not answer (it is then single-source when fewer than two places are left);
//! - a node whose answer cannot be read by the rule (a head that is not a quantity) is passed over the same way;
//! - for a transaction or a receipt, a node saying "not yet" (`null`) while another says "here it is" is not a
//!   difference and does not let the one that has it speak alone: the caller asks again after the table's
//!   pauses ([`NOT_YET_PAUSES`]) and, still split, refuses naming the nodes that do not have it yet;
//! - answers that differ under the rule are a disagreement, never a majority or a first-come.

use crate::endpoints::{self, distinct_places, Disagreement};
use crate::rpc::Trouble;
use crate::wire::{self, W};
use std::time::Duration;
use zikaron::json::Value;

/// How the answers to one method are judged. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    /// Byte for byte the same.
    Same,
    /// Only these members compared (a node lacking one differs).
    Facts(&'static [&'static str]),
    /// The smallest quantity.
    Least,
    /// The largest: of the quantity, or of the named member of an object.
    Most(Option<&'static str>),
}

/// The facts a decision reads from a pinned block: its base fee.
pub const BLOCK_FACTS: [&str; 1] = ["baseFeePerGas"];
/// The facts a decision reads from the fee history: each block's paid priority fees.
pub const FEE_HISTORY_FACTS: [&str; 1] = ["reward"];
/// The facts a decision reads from a transaction: who sent it, what it carried, which block holds it.
pub const TX_FACTS: [&str; 3] = ["blockNumber", "from", "input"];
/// The facts a decision reads from a receipt: which block, and whether it succeeded.
pub const RECEIPT_FACTS: [&str; 2] = ["blockNumber", "status"];

/// The methods whose `null` means "not yet" (a transaction, a receipt), not "none".
pub const NOT_YET_METHODS: [&str; 2] = ["eth_getTransactionByHash", "eth_getTransactionReceipt"];

/// The pauses before asking again a question some nodes answered "not yet" and others did not; split still
/// after the last, it is refused naming the nodes that do not have it. They follow the patience table's waits
/// in a test (`patience::set_waits`).
pub const NOT_YET_PAUSES: [Duration; 2] = [Duration::from_millis(300), Duration::from_millis(900)];

/// The facts this question's decision reads, when its rule names them (`Rule::Facts`); `None` when the decision
/// reads the whole answer. Every reader of these questions cuts the answer to them before reading it into the
/// law's value domain ([`wire::project`]), the judging table here and the command line's one-node reading alike.
pub fn facts_of(method: &str, params: &Value) -> Option<&'static [&'static str]> {
    match rule_of(method, params) {
        Rule::Facts(facts) => Some(facts),
        _ => None,
    }
}

/// One answer read into the law's value domain as this question's decision reads it: cut to its named facts
/// first ([`facts_of`]), so a member no decision reads cannot make it unreadable; whole otherwise.
pub fn read_as_decided(method: &str, params: &Value, w: &W) -> Option<Value> {
    match facts_of(method, params) {
        Some(facts) => wire::to_core(&wire::project(w, facts)),
        None => wire::to_core(w),
    }
}

/// The one table: which rule judges this method (asked with these params).
pub fn rule_of(method: &str, params: &Value) -> Rule {
    let first = match params {
        Value::Arr(a) => a.first(),
        _ => None,
    };
    match method {
        "eth_blockNumber" => Rule::Least,
        "eth_getTransactionCount" => Rule::Most(None),
        "eth_getBlockByNumber" if matches!(first, Some(Value::Str(s)) if s == "latest") => Rule::Most(Some("number")),
        "eth_getBlockByNumber" => Rule::Facts(&BLOCK_FACTS),
        "eth_feeHistory" => Rule::Facts(&FEE_HISTORY_FACTS),
        "eth_getTransactionByHash" => Rule::Facts(&TX_FACTS),
        "eth_getTransactionReceipt" => Rule::Facts(&RECEIPT_FACTS),
        _ => Rule::Same,
    }
}

/// Why a node gives nothing to the reading.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Missing {
    /// It did not answer (its trouble, as it came).
    Trouble(Trouble),
    /// It answered something the rule cannot read (the answer's text).
    Unreadable(String),
}

/// One reading judged from several nodes' answers.
#[derive(Clone, Debug)]
pub struct Judged {
    /// The reading (for a named-facts rule, the facts alone).
    pub value: Value,
    /// The nodes the reading came from, in the table's order.
    pub sources: Vec<String>,
    /// The nodes passed over and why, in the table's order.
    pub missing: Vec<(String, Missing)>,
    /// Fewer than two distinct places gave the reading.
    pub single_source: bool,
}

/// Why no reading came. Closed.
#[derive(Clone, Debug)]
pub enum NoReading {
    /// No node gave anything the rule could read (each node and why, in order).
    NoneAnswered(Vec<(String, Missing)>),
    /// The answers differ under the rule.
    Differ(Disagreement),
    /// Some nodes have it and some say not yet (`has`, `not_yet`, each in order).
    NotYet { has: Vec<String>, not_yet: Vec<String> },
}

/// One node's answer, with its name.
type Named = (String, Value);

fn quantity(v: &Value) -> Option<u128> {
    let s = v.as_str()?.strip_prefix("0x")?;
    if s.is_empty() || s.len() > 32 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u128::from_str_radix(s, 16).ok()
}

/// Judge the answers several nodes gave to one question (each with its node's name, in the table's order).
pub fn judge(method: &str, params: &Value, answers: Vec<(String, Result<W, Trouble>)>) -> Result<Judged, NoReading> {
    let rule = rule_of(method, params);
    let mut read: Vec<(String, Value)> = Vec::new();
    let mut missing: Vec<(String, Missing)> = Vec::new();
    for (name, got) in answers {
        match got {
            Err(t) => missing.push((name, Missing::Trouble(t))),
            Ok(w) => match read_as_decided(method, params, &w) {
                Some(v) => read.push((name, v)),
                None => missing.push((name, Missing::Unreadable(wire::write(&w)))),
            },
        }
    }
    // A number rule reads a number: an answer that is not one is passed over, said by its text.
    if matches!(rule, Rule::Least | Rule::Most(_)) {
        let key = |v: &Value| -> Option<u128> {
            match rule {
                Rule::Most(Some(member)) => v.member(member).and_then(quantity),
                _ => quantity(v),
            }
        };
        let (good, bad): (Vec<Named>, Vec<Named>) = read.into_iter().partition(|(_, v)| key(v).is_some());
        missing.extend(bad.into_iter().map(|(n, v)| (n, Missing::Unreadable(String::from_utf8_lossy(&zikaron::json::canon_bytes(&v)).into_owned()))));
        let chosen = match rule {
            Rule::Least => good.iter().min_by_key(|(_, v)| key(v)),
            _ => good.iter().max_by_key(|(_, v)| key(v)),
        };
        let Some((_, value)) = chosen else { return Err(NoReading::NoneAnswered(missing)) };
        let sources: Vec<String> = good.iter().map(|(n, _)| n.clone()).collect();
        let single_source = distinct_places(sources.iter().map(String::as_str)) < 2;
        return Ok(Judged { value: value.clone(), sources, missing, single_source });
    }
    if read.is_empty() {
        return Err(NoReading::NoneAnswered(missing));
    }
    if NOT_YET_METHODS.contains(&method) {
        let (has, not_yet): (Vec<&Named>, Vec<&Named>) = read.iter().partition(|(_, v)| !matches!(v, Value::Null));
        if !has.is_empty() && !not_yet.is_empty() {
            return Err(NoReading::NotYet { has: has.iter().map(|(n, _)| n.clone()).collect(), not_yet: not_yet.iter().map(|(n, _)| n.clone()).collect() });
        }
    }
    let runs: Vec<(String, Value)> = match rule {
        Rule::Facts(facts) => read.into_iter().map(|(n, v)| (n, endpoints::project(&v, facts))).collect(),
        _ => read,
    };
    match endpoints::agree(runs) {
        Ok(r) => Ok(Judged { value: r.fragment, single_source: r.single_source, sources: r.sources, missing }),
        Err(d) => Err(NoReading::Differ(d)),
    }
}

/// The pauses before asking a "not yet" question again, as this process takes them (`patience::set_waits`).
pub fn not_yet_pauses() -> Vec<Duration> {
    NOT_YET_PAUSES.iter().map(|d| crate::patience::waited(*d)).collect()
}
