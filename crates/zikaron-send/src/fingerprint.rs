//! The fingerprint gate for one read-only network, one implementation for the desktop and the command line: a
//! read-only network's registry is an address a person typed, often taken from what a package's author said, and a
//! contract there that is not the pinned build could emit "any sender anchored any hash". So before such a network
//! is read, each of its nodes is asked its chain, then the code at the registry, whose keccak must be the pinned
//! build's (`zikaron_anchor::deploy::CODE_HASH`). A node that does not answer, answers for another chain, or answers
//! no code at all (it has not reached the block the registry was deployed in, or nothing is deployed there) does not
//! count; one that gives other code closes the network at once. The network is read on the nodes that counted and no
//! other ([`gate_counted`]). It is asked every time; nothing is kept.
//!
//! The main network asks none of this: the gate admits read-only networks to reading and changes nothing the law
//! reads.

use crate::node;
use zikaron::json::Value;
use zikaron_anchor::rpc;

/// How a read-only network's gate went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Every node that answered gave the pinned code, and more than one answered.
    Agreed(usize),
    /// One node answered, with the pinned code.
    Single,
    /// No node counted, and not every node answered the empty code: what each node did, in node order.
    Down(Vec<Miss>),
    /// A node answered with code that is not the pinned build; or no node counted and every node answered the empty
    /// code at the registry (nothing there is the pinned build, on every node asked).
    Fingerprint,
}

impl Gate {
    /// Whether the network may be read.
    pub fn admits(&self) -> bool {
        matches!(self, Gate::Agreed(_) | Gate::Single)
    }

    /// The gate's code as written to files and said in evidence.
    pub fn code(&self) -> &'static str {
        match self {
            Gate::Agreed(_) => "agreed",
            Gate::Single => "single",
            Gate::Down(_) => "down",
            Gate::Fingerprint => "fingerprint",
        }
    }
}

/// One node that did not count, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Miss {
    /// The node as written.
    pub url: String,
    pub why: Why,
}

/// Why a node did not count. Closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Why {
    /// It is not a node address.
    NotAddress,
    /// A question went unanswered or was refused.
    Trouble(rpc::Trouble),
    /// It serves another chain (what it said).
    OtherChain(String),
    /// It answered no code at the registry (the empty code, or an answer that is not code).
    NoCode,
}

/// The gate for the network on `chain` whose registry is `registry`, asked of `nodes`, against `pinned` (`0x` and
/// sixty-four lowercase hex digits): the product passes its pin; tests pass the hash of a code of their own.
pub fn gate(chain: u64, registry: &[u8; 20], nodes: &[String], pinned: &str) -> Gate {
    gate_counted(chain, registry, nodes, pinned).0
}

/// [`gate`], also returning the nodes that counted (as given, in node order): the network is read on these and on no
/// other, so a node that did not count (one behind the registry's deployment among them) is never asked for it.
pub fn gate_counted(chain: u64, registry: &[u8; 20], nodes: &[String], pinned: &str) -> (Gate, Vec<String>) {
    let mut counted: Vec<String> = Vec::new();
    // Nodes that answered the empty code (`0x`): behind the deployment, or nothing deployed there.
    let mut empty = 0usize;
    let mut misses: Vec<Miss> = Vec::new();
    let miss = |url: &str, why: Why| Miss { url: url.to_string(), why };
    for url in nodes {
        let Some(mut ep) = node::endpoint_at(url) else {
            misses.push(miss(url, Why::NotAddress));
            continue;
        };
        // Both questions use patience (`zikaron_anchor::patience::ask`, as every read does): a node that
        // rate-limits once is waited out, not counted as down.
        let served = match zikaron_anchor::patience::ask(ep.as_mut(), "eth_chainId", &Value::Arr(Vec::new())) {
            Ok(s) => s,
            Err(t) => {
                misses.push(miss(url, Why::Trouble(t)));
                continue;
            }
        };
        if !zikaron_anchor::scan::endpoint_serves(&served, chain) {
            misses.push(miss(url, Why::OtherChain(served.as_str().unwrap_or("?").to_string())));
            continue;
        }
        let params = Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(registry)), Value::Str("latest".to_string())]);
        let code = match zikaron_anchor::patience::ask(ep.as_mut(), "eth_getCode", &params) {
            Ok(c) => c,
            Err(t) => {
                misses.push(miss(url, Why::Trouble(t)));
                continue;
            }
        };
        let Some(bytes) = code.as_str().and_then(zikaron::hexfmt::decode) else {
            misses.push(miss(url, Why::NoCode));
            continue;
        };
        if bytes.is_empty() {
            empty += 1;
            misses.push(miss(url, Why::NoCode));
            continue;
        }
        if zikaron::hexfmt::encode(&zikaron::cryptox::keccak256(&bytes)) != pinned {
            return (Gate::Fingerprint, Vec::new());
        }
        counted.push(url.clone());
    }
    let gate = match counted.len() {
        0 if empty > 0 && empty == misses.len() => Gate::Fingerprint,
        0 => Gate::Down(misses),
        1 => Gate::Single,
        n => Gate::Agreed(n),
    };
    (gate, counted)
}
