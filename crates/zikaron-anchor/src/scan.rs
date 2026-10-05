//! Scan: read the part of the chain a basis declares into §9.2 records, by the §9.4 completeness rule.
//!
//! This layer draws no audit conclusion. Whether a basis is a `zikaron/1` basis is answered by the core
//! ([`ask_core_about_basis`]) through an empty-pile audit input; no second basis check is written here.
//!
//! Completeness means an incomplete read gives no report. An unreadable block range, a `bareTx` that cannot
//! be fetched, a chain that does not answer are each a scan failure, never an empty result. Failures are a
//! named [`Refusal`]: an anchor set missing records looks complete downstream and gets a different label.
//!
//! Query order follows decision order: wherever the answer is already known, stop without asking. A malformed
//! log (three topics, empty data) stops without a node query. On replay, asking one question the recording
//! lacks is an error, so query order is part of what is tested.

use crate::rpc::{Endpoint, Trouble};
use crate::tx::{self, Tx};
use zikaron::cryptox;
use zikaron::hexfmt;
use crate::wire::W;
use zikaron::json::Value;
use zikaron::tokens::Verdict;

/// The §9.1 topic 0 preimage. The event is declared `Anchored(address indexed sender, bytes32 indexed hash)`;
/// the topic 0 preimage omits `indexed`.
pub const ANCHORED_SIGNATURE: &str = "Anchored(address,bytes32)";

/// Topic 0, 32 bytes.
pub fn topic0() -> [u8; 32] {
    cryptox::keccak256(ANCHORED_SIGNATURE.as_bytes())
}

/// Why a scan cannot go on. Every case means the scan did not read everything, which differs from reading
/// everything and finding no records.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The endpoint's chain id is not the one the basis declares (check identity first, then trust answers).
    ChainIdMismatch { declared: u64, served: String },
    /// The basis names a chain with no endpoint at hand.
    NoEndpoint(u64),
    /// The chain did not answer: declined, unreachable, or not in the recording.
    Unanswered { chain: u64, what: String },
    /// The transaction the endpoint gave does not re-encode to the one it claims.
    TxNotItself(String),
    /// What the endpoint gave is missing parts or has the wrong shape.
    Malformed(String),
}

impl Refusal {
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::ChainIdMismatch { .. } => "E_CHAIN_ID",
            Refusal::NoEndpoint(_) => "E_NO_ENDPOINT",
            Refusal::Unanswered { .. } => "E_UNANSWERED",
            Refusal::TxNotItself(_) => "E_TX_NOT_ITSELF",
            Refusal::Malformed(_) => "E_MALFORMED",
        }
    }
    pub fn detail(&self) -> String {
        match self {
            Refusal::ChainIdMismatch { declared, served } => format!("chain {declared} 的端点自称 {served}"),
            Refusal::NoEndpoint(c) => format!("chain {c} 没有端点"),
            Refusal::Unanswered { chain, what } => format!("chain {chain}: {what}"),
            Refusal::TxNotItself(h) => format!("tx {h}"),
            Refusal::Malformed(w) => w.clone(),
        }
    }
}

/// Whether the endpoint's chain id is the one the basis declares.
///
/// Another chain's answer is the answer to another question, so only byte equality continues.
pub fn endpoint_serves(served: &W, declared: u64) -> bool {
    tx::hex_qty(served) == Some(declared)
}

/// One `chains` window.
#[derive(Clone, Debug)]
pub struct Window {
    pub chain_id: u64,
    pub from_block: u64,
    pub to_block: u64,
    pub registries: Vec<[u8; 20]>,
    pub senders: Vec<[u8; 20]>,
}

/// The basis as read. The core owns the shape check; this only reads bytes that already passed into a
/// structure.
#[derive(Clone, Debug)]
pub struct Basis {
    pub windows: Vec<Window>,
    pub bare: Vec<(u64, [u8; 32])>,
    pub adoption_chains: Vec<(u64, u64)>,
    pub value: Value,
}

/// A §9.2 anchor record.
#[derive(Clone, Debug)]
pub struct AnchorRec {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_timestamp: u64,
    pub tx: [u8; 32],
    pub sender: [u8; 20],
    pub hash: [u8; 32],
    pub verdict: Verdict,
}

/// A §9.2 adoption evidence record.
#[derive(Clone, Debug)]
pub struct EvidenceRec {
    pub chain_id: u64,
    pub tx: [u8; 32],
    pub sender: [u8; 20],
    pub calldata: Vec<u8>,
}

/// What one scan produces.
pub struct Scanned {
    pub anchors: Vec<AnchorRec>,
    pub evidence: Vec<EvidenceRec>,
    pub basis: Value,
}

/// Ask the core whether these bytes are a `zikaron/1` basis.
///
/// With an empty pile, anchor set and evidence set, the basis is the only thing left that can be wrong, so no
/// label from the core means no. No basis check is written here.
pub fn ask_core_about_basis(basis_bytes: &[u8]) -> Option<Value> {
    let mut doc = Vec::new();
    doc.extend_from_slice(b"{\"anchors\":[],\"basis\":");
    doc.extend_from_slice(basis_bytes);
    doc.extend_from_slice(
        b",\"evidence\":[],\"pile\":[],\"root\":\"0x0000000000000000000000000000000000000000\",\"unavailable\":[]}",
    );
    let v = zikaron::json::parse_tests_1_3(&doc).ok()?;
    zikaron::audit::audit_full(&v)?;
    Some(v.member("basis")?.clone())
}

fn addr_of(v: &Value) -> Option<[u8; 20]> {
    let b = hexfmt::decode(v.as_str()?)?;
    if b.len() != 20 {
        return None;
    }
    let mut a = [0u8; 20];
    a.copy_from_slice(&b);
    Some(a)
}

fn h32_core(v: &Value) -> Option<[u8; 32]> {
    let b = hexfmt::decode(v.as_str()?)?;
    if b.len() != 32 {
        return None;
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&b);
    Some(a)
}

fn h32(v: &W) -> Option<[u8; 32]> {
    let b = hexfmt::decode(v.as_str()?)?;
    if b.len() != 32 {
        return None;
    }
    let mut a = [0u8; 32];
    a.copy_from_slice(&b);
    Some(a)
}

/// Read a basis the core accepted into a structure.
pub fn read_basis(basis: &Value) -> Option<Basis> {
    let mut windows = Vec::new();
    for c in basis.member("chains")?.as_arr()? {
        windows.push(Window {
            chain_id: c.member("chainId")?.as_int()?,
            from_block: c.member("fromBlock")?.as_int()?,
            to_block: c.member("toBlock")?.as_int()?,
            registries: c.member("registries")?.as_arr()?.iter().map(addr_of).collect::<Option<_>>()?,
            senders: c.member("senders")?.as_arr()?.iter().map(addr_of).collect::<Option<_>>()?,
        });
    }
    let mut bare = Vec::new();
    for b in basis.member("bareTx")?.as_arr()? {
        bare.push((b.member("chainId")?.as_int()?, h32_core(b.member("tx")?)?));
    }
    let mut adoption_chains = Vec::new();
    for a in basis.member("adoptionChains")?.as_arr()? {
        adoption_chains.push((a.member("chainId")?.as_int()?, a.member("throughBlock")?.as_int()?));
    }
    Some(Basis { windows, bare, adoption_chains, value: basis.clone() })
}

/// Hex quantity spelling the nodes accept (no leading zeros, zero as `0x0`). The only place it is spelled.
pub fn hex_quantity(x: u64) -> String {
    format!("0x{x:x}")
}

fn hex20(a: &[u8; 20]) -> String {
    hexfmt::encode(a)
}

/// The §9.1 containment test: the 32-byte word appears in calldata at an offset aligned to 32 or 4 + 32k.
pub fn calldata_carries(calldata: &[u8], word: &[u8; 32]) -> bool {
    let mut base = 0usize;
    while base <= 4 {
        let mut off = base;
        while off + 32 <= calldata.len() {
            if &calldata[off..off + 32] == word {
                return true;
            }
            off += 32;
        }
        base += 4;
    }
    false
}

struct Chain<'a> {
    id: u64,
    ep: &'a mut dyn Endpoint,
}

impl<'a> Chain<'a> {
    fn ask(&mut self, method: &str, params: Vec<Value>) -> Result<W, Refusal> {
        match self.ep.call(method, &Value::Arr(params)) {
            Ok(v) => Ok(v),
            Err(Trouble::Node(e)) => Err(Refusal::Unanswered { chain: self.id, what: format!("{method} 被拒:{e}") }),
            Err(Trouble::NotServed(k)) => Err(Refusal::Unanswered { chain: self.id, what: format!("录制里没有这一问:{k}") }),
            Err(Trouble::Transport(e)) => Err(Refusal::Unanswered { chain: self.id, what: format!("{method} 传输坏了:{e}") }),
            Err(Trouble::Contradiction(k)) => Err(Refusal::Malformed(format!("录制自相矛盾:{k}"))),
        }
    }

    /// A declined answer and "no such transaction on chain" differ: the first is a scan failure, the second a
    /// definite no.
    fn tx_by_hash(&mut self, h: &[u8; 32]) -> Result<Option<Tx>, Refusal> {
        let v = self.ask("eth_getTransactionByHash", vec![Value::Str(hexfmt::encode(h))])?;
        if v.is_null() {
            return Ok(None);
        }
        match tx::read(&v) {
            Ok(t) => Ok(Some(t)),
            Err(tx::Bad::HashMismatch) => Err(Refusal::TxNotItself(hexfmt::encode(h))),
            Err(tx::Bad::Shape(w)) => Err(Refusal::Malformed(format!("交易缺 {w}:{}", hexfmt::encode(h)))),
        }
    }

    /// Whether the receipt says the transaction succeeded. A missing receipt is not failure: the transaction
    /// is in a block and the node cannot give its receipt, so the endpoint contradicts itself, and §9.4 calls
    /// that a scan failure.
    fn status_one(&mut self, h: &[u8; 32]) -> Result<bool, Refusal> {
        let v = self.ask("eth_getTransactionReceipt", vec![Value::Str(hexfmt::encode(h))])?;
        if v.is_null() {
            return Err(Refusal::Unanswered {
                chain: self.id,
                what: format!("交易 {} 已入块而取不回它的收据", hexfmt::encode(h)),
            });
        }
        // A receipt without `status` does not have status 1 (§9.1); the status byte is read, never the state
        // root.
        Ok(tx::qty_u64(&v, "status") == Some(1))
    }

    fn block_timestamp(&mut self, n: u64) -> Result<u64, Refusal> {
        let v = self.ask("eth_getBlockByNumber", vec![Value::Str(hex_quantity(n)), Value::Bool(false)])?;
        tx::qty_u64(&v, "timestamp").ok_or(Refusal::Malformed(format!("区块 {n} 没有 timestamp")))
    }

    /// The two §9.3 boundaries: unanswered is UNPROVEN, answered with code is VOID.
    fn verdict_at(&mut self, who: &[u8; 20], block: u64) -> Result<Verdict, Refusal> {
        let mut coded = false;
        if block > 0 {
            match self.code_at(who, block - 1)? {
                None => return Ok(Verdict::Unproven),
                Some(c) => coded |= c,
            }
        }
        match self.code_at(who, block)? {
            None => return Ok(Verdict::Unproven),
            Some(c) => coded |= c,
        }
        Ok(if coded { Verdict::Void } else { Verdict::Counted })
    }

    /// `Ok(None)`: this boundary could not be asked (the node declined); `Ok(Some(true))`: the address has
    /// code.
    fn code_at(&mut self, who: &[u8; 20], block: u64) -> Result<Option<bool>, Refusal> {
        let params = vec![Value::Str(hex20(who)), Value::Str(hex_quantity(block))];
        match self.ep.call("eth_getCode", &Value::Arr(params)) {
            Ok(v) => {
                let b = v.as_str().and_then(hexfmt::decode).ok_or(Refusal::Malformed("eth_getCode 的答不是字节串".into()))?;
                Ok(Some(!b.is_empty()))
            }
            Err(Trouble::Node(_)) => Ok(None),
            Err(Trouble::NotServed(k)) => Err(Refusal::Unanswered { chain: self.id, what: format!("录制里没有这一问:{k}") }),
            Err(Trouble::Transport(e)) => Err(Refusal::Unanswered { chain: self.id, what: format!("eth_getCode 传输坏了:{e}") }),
            Err(Trouble::Contradiction(k)) => Err(Refusal::Malformed(format!("录制自相矛盾:{k}"))),
        }
    }
}

/// One scan. `endpoints` gives a channel per chain id; a chain the basis names without a channel fails the
/// scan.
pub fn run(
    basis_bytes: &[u8],
    adoptions: &[(u64, [u8; 32])],
    endpoints: &mut Vec<(u64, &mut dyn Endpoint)>,
) -> Result<Result<Scanned, Value>, Refusal> {
    crate::seam();
    noting(basis_bytes, adoptions, endpoints, &Known::default()).map(|r| r.map(|(s, _, _)| s))
}

/// The §9.4 deduplication key of a record: `(chainId, blockNumber, tx, hash)`.
pub type RecordKey = (u64, u64, [u8; 32], [u8; 32]);

/// What one log on chain is known by, for the facts read about it: the chain, the hash of its block (a block
/// that was replaced is another block, and its facts are not these), the transaction, and the log's index in
/// that block.
pub type FactKey = (u64, [u8; 32], [u8; 32], u64);

/// What a registry-form record took questions to learn, once the log was judged an anchor: the block's time
/// and the sender's verdict at that block (the transaction, its receipt and its calldata having passed), with
/// the log as it was when they passed (its block number, the sender and hash it names, the registry that
/// emitted it). A log read later under the same key is spared the questions only when it still says exactly
/// this; one that says anything else is asked about as new, so what the questions bound to the transaction
/// cannot be carried over to other words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fact {
    pub block_number: u64,
    pub sender: [u8; 20],
    pub hash: [u8; 32],
    pub emitter: [u8; 20],
    pub block_timestamp: u64,
    pub verdict: Verdict,
}

/// Facts a caller already holds, checked before (its own record of readings several endpoints agreed on).
/// The logs are still asked for whole, every time; a log found here under the same key and saying the same
/// is not asked about again, and any other (a new anchor, the same transaction in another block, a log that
/// says something else under a key held here) is asked about as always. Empty: every log is asked about.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Known(pub std::collections::BTreeMap<FactKey, Fact>);

/// Facts this scan read afresh, each under its key: only logs judged anchors with a decided verdict (an
/// unproven verdict is no fact), and only logs that carry their block hash and index.
pub type Sightings = Vec<(FactKey, Fact)>;

/// The key of a log as it came back: its chain, block hash, transaction and index; `None` when the log does
/// not carry them.
fn fact_key(chain: u64, log: &W, tx: &[u8; 32]) -> Option<FactKey> {
    let block = log.member("blockHash").and_then(h32)?;
    let index = log.member("logIndex").and_then(tx::hex_qty)?;
    Some((chain, block, *tx, index))
}

/// Which registry contracts emitted the logs each registry-form record was read from: a side reading, never
/// part of the record or of the scan's canonical bytes. One key may have several (one transaction calling two
/// registries of a window); bare-form records have none.
pub type Emitters = std::collections::BTreeMap<RecordKey, std::collections::BTreeSet<[u8; 20]>>;

/// The key a record is deduplicated by.
pub fn record_key(a: &AnchorRec) -> RecordKey {
    (a.chain_id, a.block_number, a.tx, a.hash)
}

/// [`run`], noting beside it which registry each registry-form record came from. The address is the one on
/// the log already read: no question is asked for it, so the query order is [`run`]'s.
pub fn run_noting(
    basis_bytes: &[u8],
    adoptions: &[(u64, [u8; 32])],
    endpoints: &mut Vec<(u64, &mut dyn Endpoint)>,
) -> Result<Result<(Scanned, Emitters), Value>, Refusal> {
    crate::seam();
    noting(basis_bytes, adoptions, endpoints, &Known::default()).map(|r| r.map(|(s, e, _)| (s, e)))
}

/// [`run_noting`] with facts already checked ([`Known`]), handing back as well the facts this scan read
/// afresh ([`Sightings`]). With an empty table it asks exactly what [`run`] asks.
pub fn run_knowing(
    basis_bytes: &[u8],
    adoptions: &[(u64, [u8; 32])],
    endpoints: &mut Vec<(u64, &mut dyn Endpoint)>,
    known: &Known,
) -> Result<Result<(Scanned, Emitters, Sightings), Value>, Refusal> {
    crate::seam();
    noting(basis_bytes, adoptions, endpoints, known)
}

/// The scan every public entry runs (each marks the trace once, then comes here).
fn noting(
    basis_bytes: &[u8],
    adoptions: &[(u64, [u8; 32])],
    endpoints: &mut Vec<(u64, &mut dyn Endpoint)>,
    known: &Known,
) -> Result<Result<(Scanned, Emitters, Sightings), Value>, Refusal> {
    let Some(basis_value) = ask_core_about_basis(basis_bytes) else {
        // The core says this is not a zikaron/1 basis; its no-label value is the only byte shape.
        return Ok(Err(zikaron::audit::no_label()));
    };
    let basis = read_basis(&basis_value).ok_or(Refusal::Malformed("基底过了 K1 而读不成结构".into()))?;

    let mut chains: Vec<u64> = basis.windows.iter().map(|w| w.chain_id).collect();
    chains.extend(basis.bare.iter().map(|b| b.0));
    chains.extend(basis.adoption_chains.iter().map(|a| a.0));
    chains.sort_unstable();
    chains.dedup();

    let mut anchors: Vec<AnchorRec> = Vec::new();
    let mut evidence: Vec<EvidenceRec> = Vec::new();
    let mut emitters = Emitters::new();
    let mut seen = Sightings::new();
    let t0 = topic0();

    for id in chains {
        let idx = endpoints
            .iter()
            .position(|(c, _)| *c == id)
            .ok_or(Refusal::NoEndpoint(id))?;
        let ep: &mut dyn Endpoint = &mut *endpoints[idx].1;
        let mut chain = Chain { id, ep };

        // Check the endpoint's chain id first; on mismatch stop and trust nothing after.
        let served = chain.ask("eth_chainId", vec![])?;
        if !endpoint_serves(&served, id) {
            return Err(Refusal::ChainIdMismatch { declared: id, served: crate::wire::write(&served) });
        }

        // Registry form.
        for w in basis.windows.iter().filter(|w| w.chain_id == id) {
            // No registry, or no sender this window admits: no log could become a record, so none is asked.
            if w.registries.is_empty() || w.senders.is_empty() {
                continue;
            }
            let logs = logs_over_window(&mut chain, w, &t0)?;
            for log in &logs {
                let Some(rec) = registry_record(&mut chain, w, log, &t0, known, &mut seen)? else { continue };
                if let Some(at) = emitter_of(log) {
                    emitters.entry(record_key(&rec)).or_default().insert(at);
                }
                anchors.push(rec);
            }
        }

        // Bare form.
        for (_, h) in basis.bare.iter().filter(|b| b.0 == id) {
            bare_records(&mut chain, h, &mut anchors)?;
        }

        // Adoption evidence.
        if let Some((_, through)) = basis.adoption_chains.iter().find(|a| a.0 == id) {
            let mut seen: Vec<[u8; 32]> = Vec::new();
            for (_, h) in adoptions.iter().filter(|e| e.0 == id) {
                if seen.contains(h) {
                    continue;
                }
                seen.push(*h);
                if let Some(rec) = evidence_record(&mut chain, h, *through)? {
                    evidence.push(rec);
                }
            }
        }
    }

    // The §9.4 deduplication key and both tables' order; sorting goes through the core's `ordered` only.
    let anchors = zikaron::audit::ordered(anchors, |a, b| {
        (a.chain_id, a.block_number, a.tx, a.hash).cmp(&(b.chain_id, b.block_number, b.tx, b.hash))
    });
    let deduped = dedupe(anchors);
    let evidence = zikaron::audit::ordered(evidence, |a, b| (a.chain_id, a.tx).cmp(&(b.chain_id, b.tx)));

    Ok(Ok((Scanned { anchors: deduped, evidence, basis: basis_value }, emitters, seen)))
}

/// The contract that emitted a log (its `address`), as twenty bytes.
fn emitter_of(log: &W) -> Option<[u8; 20]> {
    let b = log.member("address").and_then(|a| a.as_str()).and_then(hexfmt::decode)?;
    b.try_into().ok()
}

/// An address as a log's indexed topic carries it: twelve zero bytes, then the twenty.
fn padded(a: &[u8; 20]) -> [u8; 32] {
    let mut out = [0u8; 32];
    out[12..].copy_from_slice(a);
    out
}

/// The `eth_getLogs` query, built in one place for the whole range and for every sub-range: the window's
/// registries, the range, topic 0, and as topic 1 the window's senders in one "any of" list (each left-padded
/// to 32 bytes as the log carries it). §9.4 reads only logs whose sender the window lists, so asking for those
/// alone reads the same records; the node's filtering is never trusted for that, every log that comes back is
/// still judged here (`registry_record`). One question per window range, never one per sender.
fn log_filter(w: &Window, from: u64, to: u64, t0: &[u8; 32]) -> Value {
    let senders: Vec<Value> = w.senders.iter().map(|a| Value::Str(hexfmt::encode(&padded(a)))).collect();
    Value::Obj(vec![
        ("address".into(), Value::Arr(w.registries.iter().map(|r| Value::Str(hex20(r))).collect())),
        ("fromBlock".into(), Value::Str(hex_quantity(from))),
        ("toBlock".into(), Value::Str(hex_quantity(to))),
        ("topics".into(), Value::Arr(vec![Value::Str(hexfmt::encode(t0)), Value::Arr(senders)])),
    ])
}

/// After splitting, each sub-range spans at least one block; a refusal at one block is passed on as the node
/// said it.
const MIN_SPAN: u64 = 1;
/// How many times to shrink. Each shrink costs the refused query, so this bounds the splitting path: if the
/// node refuses for another reason (unknown method, auth), at most eight extra queries pass its sentence on.
const MAX_SHRINKS: usize = 8;

/// How many times one log question is asked before its range is split: a node that declines now often
/// answers the same question a moment later (a busy public node), and splitting a range that was refused only
/// for that would cost many questions. Asked again at once, never after waiting.
const TRIES: usize = 3;

/// Queries per window at most (asking again counts). Splitting exists to finish a window, not to shatter it: a misread limit
/// (reading `up to a 2K block range` as 2) would turn a two-million-block window into a million queries. At
/// this bound the scan returns a named refusal saying what to do (move the start block forward, or use a node
/// that accepts wider ranges).
///
/// 4096: at a 2000-block limit this covers over eight million blocks; a wider window on such a node takes
/// hours, and the person should know that up front.
const MAX_ASKS: usize = 4096;

/// Words that introduce a limit. The number must follow the word: a node's sentence carries many numbers
/// (block numbers, error codes, result counts).
const LIMIT_WORDS: [&str; 8] = ["maximum", "max", "limited to", "limit", "up to", "at most", "more than", "range"];

/// The limit a node reports in its refusal, if any. `range: 50000`, `limited to 1000 blocks`, `up to a 2K
/// block range` and `more than 10000 results` each carry one.
///
/// Only the first number after a limit word counts (`2k` and `10K` in thousands), and it must fall within
/// `1..span`. Taking the first number in the sentence would read 1000000 from `requested from block 1000000
/// to 2000000, maximum is 5000` and 2 from `up to a 2K block range`.
fn said_limit(said: &str, span: u64) -> Option<u64> {
    let low = said.to_lowercase();
    let mut best: Option<(usize, u64)> = None;
    for w in LIMIT_WORDS {
        let mut from = 0usize;
        while let Some(i) = low[from..].find(w) {
            let at = from + i + w.len();
            if let Some((n, _)) = number_after(&low[at..]) {
                if n >= MIN_SPAN && n < span && best.map(|(p, _)| at < p).unwrap_or(true) {
                    best = Some((at, n));
                }
            }
            from = at;
        }
    }
    best.map(|(_, n)| n)
}

/// The first number within 24 characters of the start of a string, with thousands grouped by commas read as
/// one number (`2,000`, `1,000,000`: a lead of one to three digits, then groups of exactly three) and `k`/`m`
/// suffixes in thousands or millions; returns the number and where it ends. A comma not followed by exactly
/// three digits ends the number (`10,20` is 10).
fn number_after(rest: &str) -> Option<(u64, usize)> {
    let b = rest.as_bytes();
    let mut i = 0usize;
    while i < b.len() && i < 24 && !b[i].is_ascii_digit() {
        i += 1;
    }
    if i >= b.len() || i >= 24 || !b[i].is_ascii_digit() {
        return None;
    }
    let start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut digits = rest[start..i].to_string();
    if digits.len() <= 3 {
        let group = |at: usize| b.get(at) == Some(&b',') && (1..=3).all(|k| b.get(at + k).is_some_and(u8::is_ascii_digit)) && !b.get(at + 4).is_some_and(u8::is_ascii_digit);
        while group(i) {
            digits.push_str(&rest[i + 1..i + 4]);
            i += 4;
        }
    }
    let mut n: u64 = digits.parse().ok()?;
    if i < b.len() {
        match b[i] {
            b'k' => {
                n = n.saturating_mul(1000);
                i += 1;
            }
            b'm' => {
                n = n.saturating_mul(1_000_000);
                i += 1;
            }
            _ => {}
        }
    }
    Some((n, i))
}

/// Logs of one window, fetched in ranges the node accepts.
///
/// Ask for the whole range first: when the node accepts, the query is byte-identical to a single-range scan
/// (so recordings still replay). A refused question is asked again as it was, up to [`TRIES`] times in all;
/// refused that many times, the span shrinks and the next question starts from the same block, using the
/// node's reported limit or halving, and that question too has [`TRIES`] tries. Asking again counts toward
/// [`MAX_ASKS`], never toward [`MAX_SHRINKS`]; if one block is still refused, pass the node's sentence on (a
/// refusal is not "zero logs").
fn logs_over_window(chain: &mut Chain, w: &Window, t0: &[u8; 32]) -> Result<Vec<W>, Refusal> {
    let mut logs: Vec<W> = Vec::new();
    if w.to_block < w.from_block {
        return Ok(logs);
    }
    let whole = w.to_block - w.from_block + 1;
    let mut span = whole;
    let mut at = w.from_block;
    let mut shrinks = 0usize;
    let mut asks = 0usize;
    // How many times the question now due was refused.
    let mut refused = 0usize;
    while at <= w.to_block {
        let end = at.saturating_add(span - 1).min(w.to_block);
        if asks >= MAX_ASKS {
            // Unfinished is said as unfinished: a refusal goes out, never "zero logs", which downstream would
            // read as "no anchors in this range".
            return Err(Refusal::Unanswered {
                chain: chain.id,
                what: format!(
                    "这一扇窗按节点收得下的跨度({span} 块)问不完:{whole} 块要过 {MAX_ASKS} 问以上;\
把起始区块往前挪,或换一处收得下更宽窗的节点"
                ),
            });
        }
        asks += 1;
        match chain.ask("eth_getLogs", vec![log_filter(w, at, end, t0)]) {
            Ok(answered) => {
                // An answer that is not a log list was not answered: reading it as zero logs would turn an
                // unusable answer into a complete empty anchor set.
                let Some(mut got) = answered.as_arr().map(<[W]>::to_vec) else {
                    return Err(Refusal::Unanswered {
                        chain: chain.id,
                        what: "eth_getLogs 回的不是一张日志表".into(),
                    });
                };
                logs.append(&mut got);
                at = end.saturating_add(1);
                refused = 0;
            }
            Err(e) => {
                refused += 1;
                if refused < TRIES {
                    continue;
                }
                refused = 0;
                if span <= MIN_SPAN || shrinks >= MAX_SHRINKS {
                    return Err(e);
                }
                let said = match &e {
                    Refusal::Unanswered { what, .. } => what.clone(),
                    _ => String::new(),
                };
                span = said_limit(&said, span).unwrap_or(span / 2).max(MIN_SPAN);
                shrinks += 1;
            }
        }
    }
    Ok(logs)
}

/// A log read as the anchor it claims to be, or as not an anchor.
#[derive(Clone, Debug)]
pub struct LogFacts {
    pub claimed: [u8; 20],
    pub hash: [u8; 32],
    pub tx: [u8; 32],
    pub block_number: u64,
}

/// The §9.1 log shape: exactly three topics and no data bytes; topic 0 is the hash of the signature, topic 1
/// is a 20-byte address left-padded with twelve zero bytes.
///
/// One topic more or less, or one data byte, is not an anchor whatever the topics hold. No node query is
/// spent on a malformed log.
pub fn anchored_log(log: &W, t0: &[u8; 32]) -> Option<LogFacts> {
    let topics = log.member("topics")?.as_arr()?;
    if topics.len() != 3 {
        return None;
    }
    // `data` must be present, a string and empty; a log without it is incomplete, not an anchor.
    if log.member("data")?.as_str()? != "0x" {
        return None;
    }
    let (a, b, c) = (h32(&topics[0])?, h32(&topics[1])?, h32(&topics[2])?);
    if &a != t0 || b[..12].iter().any(|x| *x != 0) {
        return None;
    }
    let mut claimed = [0u8; 20];
    claimed.copy_from_slice(&b[12..]);
    Some(LogFacts {
        claimed,
        hash: c,
        tx: log.member("transactionHash").and_then(h32)?,
        block_number: log.member("blockNumber").and_then(tx::hex_qty)?,
    })
}

/// Whether this window admits the sender (§9.4: `senders` bounds only the registry-form scan).
pub fn window_admits(w: &Window, claimed: &[u8; 20]) -> bool {
    w.senders.contains(claimed)
}

fn registry_record(
    chain: &mut Chain,
    w: &Window,
    log: &W,
    t0: &[u8; 32],
    known: &Known,
    seen: &mut Sightings,
) -> Result<Option<AnchorRec>, Refusal> {
    let Some(f) = anchored_log(log, t0) else { return Ok(None) };
    let (claimed, hash, txh, bn) = (f.claimed, f.hash, f.tx, f.block_number);
    if bn < w.from_block || bn > w.to_block {
        return Ok(None);
    }
    // The §9.1 half "the emitter is a registry this window declares" is checked locally, not left to the
    // node's filter, as every other endpoint claim (chain id, transaction hash, sender) is rechecked here.
    let Some(addr) = log.member("address").and_then(|a| a.as_str()).and_then(hexfmt::decode) else {
        return Ok(None);
    };
    if addr.len() != 20 || !w.registries.iter().any(|r| r[..] == addr[..]) {
        return Ok(None);
    }
    let mut emitter = [0u8; 20];
    emitter.copy_from_slice(&addr);
    // Checked before under this very key (same block hash, same transaction, same log) and the log still says
    // what it said then: its transaction, receipt and calldata passed and the time and verdict are known, so
    // nothing is asked. A log that says anything else under that key is asked about as new. The window's own
    // sender list is still applied here, every time.
    let key = fact_key(chain.id, log, &txh);
    let said_then = |f: &&Fact| f.block_number == bn && f.sender == claimed && f.hash == hash && f.emitter == emitter;
    if let Some(f) = key.and_then(|k| known.0.get(&k)).filter(said_then) {
        if !window_admits(w, &claimed) {
            return Ok(None);
        }
        return Ok(Some(AnchorRec { chain_id: chain.id, block_number: bn, block_timestamp: f.block_timestamp, tx: txh, sender: claimed, hash, verdict: f.verdict }));
    }

    let Some(t) = chain.tx_by_hash(&txh)? else { return Ok(None) };
    if !chain.status_one(&txh)? {
        return Ok(None);
    }
    // `senders` bounds the registry-form scan: a sender the window does not list yields no record from it.
    if !window_admits(w, &claimed) {
        return Ok(None);
    }
    if !calldata_carries(&t.input, &hash) {
        return Ok(None);
    }
    // These three are decided from bytes at hand, so they come before the two queries: the recovered sender
    // must equal topic 1, the signature must name this chain, and a contract creation carries init code with
    // no calldata to test containment in. Asking the node after the law has already ruled out an anchor would
    // be one question too many, and on replay that is an error.
    if t.sender != Some(claimed) || t.chain_id != Some(chain.id) || t.to.is_none() {
        return Ok(None);
    }
    let block_timestamp = chain.block_timestamp(bn)?;
    let verdict = chain.verdict_at(&claimed, bn)?;
    if let (Some(k), false) = (key, verdict == Verdict::Unproven) {
        seen.push((k, Fact { block_number: bn, sender: claimed, hash, emitter, block_timestamp, verdict }));
    }
    Ok(Some(AnchorRec {
        chain_id: chain.id,
        block_number: bn,
        block_timestamp,
        tx: txh,
        sender: claimed,
        hash,
        verdict,
    }))
}

/// Read a transaction named by a `bareTx` into zero or more records.
fn bare_records(chain: &mut Chain, h: &[u8; 32], out: &mut Vec<AnchorRec>) -> Result<(), Refusal> {
    let Some(t) = chain.tx_by_hash(h)? else { return Ok(()) };
    let Some(bn) = t.block_number else { return Ok(()) };
    // The recipient is the sender itself; a contract creation has no recipient.
    let Some(to) = t.to else { return Ok(()) };
    if let Some(s) = t.sender {
        if s != to {
            return Ok(());
        }
    }
    if !chain.status_one(h)? {
        return Ok(());
    }
    if t.input.is_empty() || t.input.len() % 32 != 0 {
        return Ok(());
    }
    let block_timestamp = chain.block_timestamp(bn)?;
    let verdict = chain.verdict_at(&to, bn)?;
    if t.sender != Some(to) || t.chain_id != Some(chain.id) {
        return Ok(());
    }
    for word in t.input.chunks(32) {
        let mut hash = [0u8; 32];
        hash.copy_from_slice(word);
        out.push(AnchorRec {
            chain_id: chain.id,
            block_number: bn,
            block_timestamp,
            tx: *h,
            sender: to,
            hash,
            verdict,
        });
    }
    Ok(())
}

/// Read a transaction named by an adoption element into zero or one evidence record.
fn evidence_record(chain: &mut Chain, h: &[u8; 32], through: u64) -> Result<Option<EvidenceRec>, Refusal> {
    let Some(t) = chain.tx_by_hash(h)? else { return Ok(None) };
    let Some(bn) = t.block_number else { return Ok(None) };
    if bn > through {
        return Ok(None);
    }
    let (Some(sender), Some(cid)) = (t.sender, t.chain_id) else { return Ok(None) };
    if cid != chain.id {
        return Ok(None);
    }
    // §9.2: a creation's record carries empty calldata; its init code proves no element.
    let calldata = if t.to.is_none() { Vec::new() } else { t.input };
    Ok(Some(EvidenceRec { chain_id: chain.id, tx: *h, sender, calldata }))
}

/// The §9.4 deduplication key: at most one record per `(chainId, blockNumber, tx, hash)`.
///
/// The same hash twice in one `anchorMany` is two logs on chain and one record here; one record and two
/// records are different anchor sets for the audit. Input must be sorted by that key.
pub fn dedupe(anchors: Vec<AnchorRec>) -> Vec<AnchorRec> {
    let mut out: Vec<AnchorRec> = Vec::new();
    for a in anchors {
        let key = (a.chain_id, a.block_number, a.tx, a.hash);
        if out.last().map(|p| (p.chain_id, p.block_number, p.tx, p.hash)) == Some(key) {
            continue;
        }
        out.push(a);
    }
    out
}

/// The scan output as canonical bytes: `{"anchors":[...],"basis":{...},"evidence":[...]}`.
pub fn fragment(s: &Scanned) -> Value {
    crate::seam();
    let anchors = s
        .anchors
        .iter()
        .map(|a| {
            Value::Obj(vec![
                ("blockNumber".into(), Value::Int(a.block_number)),
                ("blockTimestamp".into(), Value::Int(a.block_timestamp)),
                ("chainId".into(), Value::Int(a.chain_id)),
                ("hash".into(), Value::Str(hexfmt::encode(&a.hash))),
                ("sender".into(), Value::Str(hex20(&a.sender))),
                ("tx".into(), Value::Str(hexfmt::encode(&a.tx))),
                ("verdict".into(), Value::Str(a.verdict.as_str().into())),
            ])
        })
        .collect();
    let evidence = s
        .evidence
        .iter()
        .map(|e| {
            Value::Obj(vec![
                ("calldata".into(), Value::Str(hexfmt::encode(&e.calldata))),
                ("chainId".into(), Value::Int(e.chain_id)),
                ("sender".into(), Value::Str(hex20(&e.sender))),
                ("tx".into(), Value::Str(hexfmt::encode(&e.tx))),
            ])
        })
        .collect();
    Value::Obj(vec![
        ("anchors".into(), Value::Arr(anchors)),
        ("basis".into(), s.basis.clone()),
        ("evidence".into(), Value::Arr(evidence)),
    ])
}

#[cfg(test)]
mod said_limit_tests {
    use super::{number_after, said_limit};

    /// The limit a node reports is read from the right number. These are sentences real nodes returned (a
    /// public Sepolia node, Alchemy, geth); each yields its stated limit and no other number.
    #[test]
    fn a_node_that_states_its_block_range_limit_is_read_at_that_number() {
        // A public Sepolia node (the sentence also carries `"code":-32701`).
        let real = "节点拒绝请求:{\"code\":-32701,\"message\":\"exceed maximum block range: 50000\"}";
        assert_eq!(said_limit(real, 60_000), Some(50_000), "取的该是 50000,不是句里那个错误码");
        // Alchemy: `2K` in thousands.
        assert_eq!(said_limit("You can make eth_getLogs requests with up to a 2K block range", 2_600_000), Some(2_000));
        // geth: block numbers first, the limit after.
        assert_eq!(
            said_limit("requested from block 1000000 to 2000000, maximum is 5000", 2_600_000),
            Some(5_000),
            "取的该是 maximum 后面那一枚,不是起始块号"
        );
        // No limit in the sentence: halving takes over.
        assert_eq!(said_limit("query timeout exceeded", 50_000), None);
        // The official Base node: thousands grouped by a comma.
        assert_eq!(said_limit("eth_getLogs is limited to a 2,000 range", 2_600_000), Some(2_000), "2,000 是两千,不是 2");
        // BlockPI: the limit, then a link carrying no number.
        assert_eq!(
            said_limit("eth_getLogs is limited to 5000 block range. Please check the parameter requirements at  https://docs.blockpi.io/documentations/api-reference", 2_600_000),
            Some(5_000)
        );
        // Groups of three after a lead of one to three digits; anything else ends the number.
        assert_eq!(number_after(" 1,000,000 blocks"), Some((1_000_000, 10)));
        assert_eq!(number_after(" 10,20 blocks"), Some((10, 3)));
        assert_eq!(number_after(" 2,0000"), Some((2, 2)));
        assert_eq!(number_after(" 5000,000"), Some((5000, 5)));
        assert_eq!(number_after(" 2,000k"), Some((2_000_000, 7)));
    }
}
