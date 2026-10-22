//! Chain reads, wired: every reading, every node opened and every refusal comes from the sender
//! (`zikaron_send::node`, one with the command line); here only this desktop's words for its codes and evidence
//! ([`said_of`], [`said_fault`], [`trouble_said`], [`address_said`]), the connection layers' words ([`speak`]),
//! the fee card's source line and this crate's trace mark on a balance read.
//!
//! Load-bearing reads count only when several endpoints agree, and a single source is flagged: every reading
//! carries `sources` and `single_source`, so a balance from one endpoint looks different from one that two
//! endpoints agree on.

use crate::fault::Fault;
use crate::key::Address;
use zikaron::json::Value;
use zikaron_anchor::rpc;
use zikaron_send::Why;

pub use zikaron_send::node::{forget_serving, refusal_of, route_of, said, wei, Endpoint, Got, Https, Layer, NodeAddr, Refusal, Said, Wire, FEE_FACTS, MAX_HEAD_LAG, SEND_BACKOFF, TIP_FACTS, TX_FACTS};

/// This desktop's words for a connection layer's failure (the node in its safe form, the system's detail),
/// handed to the sender once ([`zikaron_send::node::say_layers`]). Every way into the sender's nodes from this
/// crate says them first (this module's readings, the gate and the queue's wiring, `exitgate::Desk`).
pub fn speak() {
    zikaron_send::node::say_layers(|l, node, detail| crate::lang::filln(layer_key(l), &[node, detail]));
}

fn layer_key(l: Layer) -> crate::lang::Key {
    match l {
        Layer::Name => crate::lang::Key::Tail232,
        Layer::Connect => crate::lang::Key::Tail233,
        Layer::Handshake => crate::lang::Key::Tail234,
        Layer::Certificate => crate::lang::Key::Tail235,
    }
}

/// A node entry a person typed, read as the sender reads it (`Endpoint::typed`), its refusal in words.
pub fn typed(spec: &str) -> Result<Endpoint, String> {
    Endpoint::typed(spec).map_err(|w| said_of(&w))
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

fn reading_of(r: zikaron_send::node::Reading) -> Reading {
    Reading { value: r.value, sources: r.sources, single_source: r.single_source, unanswered: r.unanswered.iter().map(said_of).collect() }
}

/// The sender's evidence in this desktop's words, one place for every member: nodes in their safe form with what
/// they said, and the sentences of this crate's table.
pub fn said_of(w: &Why) -> String {
    use crate::lang::{filln, t, Key};
    match w {
        Why::Text(s) => s.clone(),
        Why::Each(all) => all.iter().map(said_of).collect::<Vec<_>>().join(" · "),
        Why::Node { node, trouble } => node_words(node, trouble),
        Why::Unreadable { node } => filln(Key::Tail094, &[node]),
        Why::Scheme { node } => filln(Key::Tail093, &[node]),
        Why::Port { node } => filln(Key::TailNodePort, &[node]),
        Why::NotAddress { node } => filln(Key::TailNodeAddress, &[node]),
        Why::NotSpec { node } => filln(Key::Tail008, &[node]),
        Why::AtChain { chain, why } => format!("{chain}={}", said_of(why)),
        Why::NoHeadNode { chain } => filln(Key::Tail095, &[&chain.to_string()]),
        Why::NoTimeNode { chain } => filln(Key::Tail097, &[&chain.to_string()]),
        Why::NotHeight { node, text } => filln(Key::Tail096, &[node, text]),
        Why::Behind { node, lag, high } => filln(Key::TailNodeBehind, &[node, &lag.to_string(), &high.to_string()]),
        Why::NoTime { node } => filln(Key::Tail098, &[node]),
        Why::NotYetAt { has, not_yet } => filln(Key::TailNotYetAt, &[has, not_yet]),
        Why::Differ { count, sources } => filln(Key::Tail086, &[&count.to_string(), sources]),
        Why::NoHistory { nodes } => filln(Key::TailNoHistory, &[nodes]),
        Why::WrongChainNode { node, serves, chain } => filln(Key::TailWrongChainNode, &[node, &serves.to_string(), &chain.to_string()]),
        Why::BalanceNotText(s) => filln(Key::Tail099, &[s]),
        Why::BalanceNotQty(s) => filln(Key::Tail100, &[s]),
        Why::GasNotText(s) => filln(Key::Tail033, &[s]),
        Why::GasNotQty(s) => filln(Key::Tail034, &[s]),
        Why::NoAddress => t(Key::Tail013).to_string(),
        Why::Funds { need, have } => crate::action::funds_tail(*need, *have),
        Why::OnChain { names, tx, block } => format!("{} · {tx} · {block}", names.join(" ")),
        Why::NoNodes => t(Key::Tail084).to_string(),
        Why::NonceUnread { node } => filln(Key::TailNonceUnread, &[node]),
        Why::Scan { unanswered, .. } => unanswered.iter().map(said_of).collect::<Vec<_>>().join(" · "),
    }
}

/// The plain wording of an RPC failure: the kind and the original message, without Debug quotes or variant
/// names. The fault table and the status line read the same `Fault`, so both say the same thing.
pub fn trouble_said(url: &str, t: &rpc::Trouble) -> String {
    // Name the node in its safe-to-display form (`zikaron_net::sayable`: no path, query or user info).
    node_words(&zikaron_net::sayable(url), t)
}

fn node_words(url: &str, t: &rpc::Trouble) -> String {
    // The kind in this end's words; what follows is the evidence as it came: the node's own words, or the
    // transport's machine form (`rpc::Broke::evidence`).
    let (what, inner) = match t {
        rpc::Trouble::Node(e) => (crate::lang::t(crate::lang::Key::Tail089), e.clone()),
        // An answer refused at its HTTP status is the node's (or its gateway's) refusal, not a broken wire.
        rpc::Trouble::Transport(b @ rpc::Broke::Status { .. }) => (crate::lang::t(crate::lang::Key::Tail089), b.evidence()),
        rpc::Trouble::Transport(b) => (crate::lang::t(crate::lang::Key::Tail090), b.evidence()),
        rpc::Trouble::NotServed(k) => (crate::lang::t(crate::lang::Key::Tail091), k.clone()),
        rpc::Trouble::Contradiction(k) => (crate::lang::t(crate::lang::Key::Tail092), k.clone()),
    };
    // A revert carries what the contract said, read from the error's data. Every place that reports a failure
    // goes through here, so none reports a revert without its reason.
    let reason = match t {
        rpc::Trouble::Node(e) if refusal_of(e) == Refusal::Reverted => zikaron_anchor::said::reverted(e).map(|r| format!(" \u{b7} {}", r.evidence())).unwrap_or_default(),
        _ => String::new(),
    };
    // The evidence often already names the node; do not name it twice.
    if inner.contains(url) {
        format!("{what}:{inner}{reason}")
    } else {
        format!("{url}: {what}:{inner}{reason}")
    }
}

/// Turns what the node said into a named refusal (the sender's one table, `node::said_trouble`), said here.
pub fn said_fault(url: &str, t: &rpc::Trouble) -> Fault {
    zikaron_send::node::said_trouble(url, t).into()
}

/// Why a node address does not parse, in plain words: its scheme, its port, or the rest of its form. Empty when
/// it parses.
pub fn address_said(url: &str) -> String {
    said_of(&zikaron_send::node::address_said(url))
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

/// The only place an endpoint is opened from a URL (the sender's, `node::endpoint_at`).
pub fn endpoint_at(url: &str) -> Option<Box<dyn rpc::Endpoint + Send>> {
    speak();
    zikaron_send::node::endpoint_at(url)
}

/// An https address for a remote fetch, over the same TLS client nodes are asked through (`node::Https`).
pub fn https(url: &str) -> Option<Https> {
    speak();
    Https::new(url)
}

pub fn ask(eps: &[Endpoint], method: &str, params: &Value) -> Result<Reading, Fault> {
    speak();
    zikaron_send::node::ask(eps, method, params).map(reading_of).map_err(Fault::from)
}

/// [`ask`] for a request whose refusal may concern the call itself (gas estimation): the refusal comes with
/// whether the shared table reads it that way (`zikaron_send::node::ask_call`).
pub fn ask_call(eps: &[Endpoint], method: &str, params: &Value) -> Result<Reading, (Fault, bool)> {
    speak();
    zikaron_send::node::ask_call(eps, method, params).map(reading_of).map_err(|(t, call)| (Fault::from(t), call))
}

/// Whether any node knows this transaction, asked by hash (`node::tx_known`).
pub fn tx_known(urls: &[NodeAddr], chain: u64, hash: &[u8; 32]) -> bool {
    speak();
    zikaron_send::node::tx_known(urls, chain, hash)
}

/// A transaction in full as one node of this chain holds it (`node::tx_as_held`).
pub fn tx_as_held(urls: &[NodeAddr], chain: u64, tx: &str) -> Option<Value> {
    speak();
    zikaron_send::node::tx_as_held(urls, chain, tx)
}

/// Which block this pass reads: the lowest head every endpoint has reached (`node::head_block`).
pub fn head_block(eps: &[Endpoint], chain: u64) -> Result<(u64, usize), Fault> {
    speak();
    zikaron_send::node::head_block(eps, chain).map_err(Fault::from)
}

/// Chain time: the latest block's time and number (`node::head_time`).
pub fn head_time(eps: &[Endpoint], chain: u64) -> Result<(u64, u64, usize), Fault> {
    speak();
    zikaron_send::node::head_time(eps, chain).map_err(Fault::from)
}

/// This chain's endpoints that actually serve it, and the nodes left out, each named (`node::serving`).
pub fn serving(eps: &[Endpoint], chain: u64) -> Result<(Vec<Endpoint>, Vec<String>), Fault> {
    speak();
    zikaron_send::node::serving(eps, chain).map(|(kept, left)| (kept, left.iter().map(said_of).collect())).map_err(Fault::from)
}

/// The key behind an address on these networks (`zikaron_send::recipient::key_of`: found from a transaction the
/// address signed, not anchored on any of them, or not read through), this desktop's words for the connection layers
/// said first.
pub fn recipient_key(nets: &[RecipientNet], author: &[u8; 20]) -> zikaron_send::recipient::Found {
    speak();
    zikaron_send::recipient::key_of(nets, author)
}

/// One network a recipient's key is looked for on.
pub use zikaron_send::recipient::{Found as RecipientFound, Net as RecipientNet};

/// The key behind an address for sealing a package to it, the sender's one judgement for both ends
/// (`zikaron_send::recipient::key_for`), this desktop's words for the connection layers said first.
pub fn key_for_sealing(address: &[u8; 20], nets: Vec<RecipientNet>, readonly: &[RecipientNet], unread: Vec<String>, pinned: &str) -> zikaron_send::recipient::KeyFor {
    speak();
    zikaron_send::recipient::key_for(address, nets, readonly, unread, pinned)
}

/// A read-only network's fingerprint gate, the sender's one gate for both ends (`zikaron_send::fingerprint`), this
/// desktop's words for the connection layers said first.
pub fn fingerprint_gate(chain: u64, registry: &[u8; 20], nodes: &[String], pinned: &str) -> FingerprintGate {
    fingerprint_gate_counted(chain, registry, nodes, pinned).0
}

/// [`fingerprint_gate`], also returning the nodes that counted (`zikaron_send::fingerprint::gate_counted`): the network
/// is read on those and no other.
pub fn fingerprint_gate_counted(chain: u64, registry: &[u8; 20], nodes: &[String], pinned: &str) -> (FingerprintGate, Vec<String>) {
    speak();
    zikaron_send::fingerprint::gate_counted(chain, registry, nodes, pinned)
}

/// How a read-only network's gate went, and why a node did not count (the sender's).
pub use zikaron_send::fingerprint::{Gate as FingerprintGate, Why as FingerprintMiss};

/// An address's balance (wei) on `chain`, several endpoints agreeing (`node::balance`).
pub fn balance(eps: &[Endpoint], chain: u64, who: &Address) -> Result<(u128, Reading), Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::H4);
    speak();
    zikaron_send::node::balance(eps, chain, &who.0).map(|(n, r)| (n, reading_of(r))).map_err(Fault::from)
}

/// This transaction's two fee fields, and the nodes left out because they serve another chain, each named
/// (`node::fees`).
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

/// [`fees`], also returning the base fee read (`node::fee_reading`).
pub fn fee_reading(eps: &[Endpoint], chain: u64) -> FeeReading {
    speak();
    let r = zikaron_send::node::fee_reading(eps, chain);
    FeeReading { base: r.base, fees: r.fees, left: r.left.iter().map(said_of).collect() }
}
