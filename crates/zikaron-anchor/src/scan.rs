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
    /// Ask with patience (`patience::ask`: a rate limit and a server error are waited out by the one table).
    fn ask(&mut self, method: &str, params: Vec<Value>) -> Result<W, Refusal> {
        crate::patience::ask(self.ep, method, &Value::Arr(params)).map_err(|t| self.refused(method, t))
    }

    /// [`Chain::ask`], also telling whether the node still refused for its rate (after the patience table's
    /// pauses) rather than for what was asked ([`rate_limited`]).
    fn ask_paced(&mut self, method: &str, params: Vec<Value>) -> Result<W, (Refusal, bool)> {
        crate::patience::ask(self.ep, method, &Value::Arr(params)).map_err(|t| {
            let limited = rate_limited(&t);
            (self.refused(method, t), limited)
        })
    }

    /// A trouble as the scan's refusal.
    fn refused(&self, method: &str, t: Trouble) -> Refusal {
        match t {
            Trouble::Node(e) => Refusal::Unanswered { chain: self.id, what: format!("{method} 被拒:{e}") },
            Trouble::NotServed(k) => Refusal::Unanswered { chain: self.id, what: format!("录制里没有这一问:{k}") },
            Trouble::Transport(e) => Refusal::Unanswered { chain: self.id, what: format!("{method} 传输坏了:{e}") },
            Trouble::Contradiction(k) => Refusal::Malformed(format!("录制自相矛盾:{k}")),
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
    /// code ([`code_at`]).
    fn code_at(&mut self, who: &[u8; 20], block: u64) -> Result<Option<bool>, Refusal> {
        code_at(self.ep, self.id, who, block)
    }
}

/// Whether `who` has code at `block` on one node of chain `chain` (`eth_getCode`): `Ok(Some(true))` it has,
/// `Ok(Some(false))` it has none, `Ok(None)` the node declined (this boundary could not be asked: §9.3
/// UNPROVEN). Asked with patience as every other question of the scan (`patience::ask`): a rate limit is waited
/// out by the one table before the node's word is taken as a decline; a node still limited after the table's
/// pauses declined. A broken transport, a question a recording lacks, or an answer that is not bytes is no
/// answer at all.
pub fn code_at(ep: &mut dyn Endpoint, chain: u64, who: &[u8; 20], block: u64) -> Result<Option<bool>, Refusal> {
    let params = vec![Value::Str(hex20(who)), Value::Str(hex_quantity(block))];
    match crate::patience::ask(ep, "eth_getCode", &Value::Arr(params)) {
        Ok(v) => {
            let b = v.as_str().and_then(hexfmt::decode).ok_or(Refusal::Malformed("eth_getCode 的答不是字节串".into()))?;
            Ok(Some(!b.is_empty()))
        }
        Err(Trouble::Node(_)) => Ok(None),
        Err(Trouble::NotServed(k)) => Err(Refusal::Unanswered { chain, what: format!("录制里没有这一问:{k}") }),
        Err(Trouble::Transport(e)) => Err(Refusal::Unanswered { chain, what: format!("eth_getCode 传输坏了:{e}") }),
        Err(Trouble::Contradiction(k)) => Err(Refusal::Malformed(format!("录制自相矛盾:{k}"))),
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
/// (block numbers, error codes, result counts). `only` is Arbitrum's official node: `query spans 268435456
/// blocks (1 to 268435456), but only 10000000 are allowed for this request`.
const LIMIT_WORDS: [&str; 9] = ["maximum", "max", "limited to", "limit", "up to", "at most", "more than", "range", "only"];

/// Whether a refusal says the node is limiting its rate, and not that the range was too wide: the patience
/// table's class (`patience::class_of`: a 429, or a rate-limit marker in words naming no range). A rate
/// refusal carries no range limit, so no number is read from it.
fn rate_limited(t: &Trouble) -> bool {
    crate::patience::class_of(t) == crate::patience::Class::RateLimited
}

/// Set the pauses a scan waits before asking a rate-limited log question again: now every pause of the
/// patience table (`patience::set_waits`; `Some` of anything takes every pause as its first entry, zero in
/// a test; `None`: the table's own).
pub fn set_rate_backoff(pauses: Option<Vec<std::time::Duration>>) {
    crate::patience::set_waits(pauses.map(|p| p.first().copied().unwrap_or_default()));
}

/// The widest log range each node took after its range was cut, by node, for the life of this process: the
/// next window asked of that node starts at it, instead of being refused down to it again (a node that takes
/// 10,000 blocks is asked 10,000 at a time from then on). Only ever narrowed.
static SPANS: std::sync::Mutex<std::collections::BTreeMap<String, u64>> = std::sync::Mutex::new(std::collections::BTreeMap::new());

/// The range this node is known to take ([`SPANS`]), if it was ever cut.
pub fn known_span(node: &str) -> Option<u64> {
    SPANS.lock().unwrap_or_else(|e| e.into_inner()).get(node).copied()
}

/// Remember that this node took `span` blocks after its range was cut (the narrower of what is known).
fn remember_span(node: &str, span: u64) {
    let mut g = SPANS.lock().unwrap_or_else(|e| e.into_inner());
    let at = g.entry(node.to_string()).or_insert(span);
    *at = (*at).min(span);
}

/// Forget every node's known range (for tests, or a run that starts over).
pub fn forget_spans() {
    SPANS.lock().unwrap_or_else(|e| e.into_inner()).clear();
}

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
///
/// A node limiting its rate ([`rate_limited`]) is not asked again at once and its range is not shrunk: the
/// patience table asked it again after each of its pauses already (`patience::ask`), so a refusal still
/// limited is passed on. Shrinking would only send more, smaller questions to a node that asks for fewer.
///
/// A node whose range was cut before in this process ([`known_span`]) is asked at that range from the start;
/// a range cut in this window is remembered once a question at it is answered.
fn logs_over_window(chain: &mut Chain, w: &Window, t0: &[u8; 32]) -> Result<Vec<W>, Refusal> {
    let mut logs: Vec<W> = Vec::new();
    if w.to_block < w.from_block {
        return Ok(logs);
    }
    // Saturating: a window from block 0 to the last there is spans one more block than a u64 counts.
    let whole = (w.to_block - w.from_block).saturating_add(1);
    // Kept by place, not by the name said: two nodes on one host (`…/eth/<key>`, `…/polygon/<key>`) are two.
    let node = chain.ep.place();
    let mut span = known_span(&node).map_or(whole, |k| k.clamp(MIN_SPAN, whole));
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
        match chain.ask_paced("eth_getLogs", vec![log_filter(w, at, end, t0)]) {
            // Still limited after the patience table's pauses: passed on as it is.
            Err((e, true)) => return Err(e),
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
                if shrinks > 0 {
                    remember_span(&node, span);
                }
                // The last block there is was read: `at` cannot move past it.
                if end == u64::MAX {
                    break;
                }
            }
            Err((e, false)) => {
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

#[cfg(test)]
mod window_tests {
    use super::*;

    /// A node answering log questions by a rule over `(from, to, how many asked before)`; every question's
    /// range is kept in order.
    struct Logs<F: FnMut(u64, u64, usize) -> Result<W, Trouble>> {
        rule: F,
        asked: Vec<(u64, u64)>,
        methods: Vec<String>,
        /// A name of its own: a node's known range is kept by place for the life of the process.
        name: String,
        /// Its place, when it is not its name (two nodes said alike, one host, paths apart).
        place: Option<String>,
    }

    impl<F: FnMut(u64, u64, usize) -> Result<W, Trouble>> Endpoint for Logs<F> {
        fn call(&mut self, method: &str, params: &Value) -> Result<W, Trouble> {
            self.methods.push(method.to_string());
            let at = |k: &str| -> u64 {
                match params {
                    Value::Arr(ps) => match ps.first() {
                        Some(Value::Obj(f)) => f.iter().find(|(n, _)| n == k).and_then(|(_, v)| v.as_str()).and_then(|h| u64::from_str_radix(h.trim_start_matches("0x"), 16).ok()).unwrap_or(0),
                        _ => 0,
                    },
                    _ => 0,
                }
            };
            let (from, to) = (at("fromBlock"), at("toBlock"));
            let n = self.asked.len();
            self.asked.push((from, to));
            (self.rule)(from, to, n)
        }
        fn name(&self) -> String {
            self.name.clone()
        }
        fn place(&self) -> String {
            self.place.clone().unwrap_or_else(|| self.name.clone())
        }
    }

    fn logs<F: FnMut(u64, u64, usize) -> Result<W, Trouble>>(rule: F) -> Logs<F> {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let name = format!("logs-{}", N.fetch_add(1, std::sync::atomic::Ordering::SeqCst));
        Logs { rule, asked: Vec::new(), methods: Vec::new(), name, place: None }
    }

    fn window(from: u64, to: u64) -> Window {
        Window { chain_id: 1, from_block: from, to_block: to, registries: vec![[0x11; 20]], senders: vec![[0x22; 20]] }
    }

    fn empty() -> Result<W, Trouble> {
        Ok(W::of(crate::wire::Body::Arr(Vec::new())))
    }

    fn node_says(code: i64, message: &str) -> Trouble {
        Trouble::Node(format!("{{\"code\":{code},\"message\":\"{message}\"}}"))
    }

    fn scan_window<F: FnMut(u64, u64, usize) -> Result<W, Trouble>>(node: &mut Logs<F>, w: &Window) -> Result<Vec<W>, Refusal> {
        let mut chain = Chain { id: w.chain_id, ep: node };
        logs_over_window(&mut chain, w, &topic0())
    }

    fn no_pauses() {
        set_rate_backoff(Some(vec![std::time::Duration::ZERO; crate::said::RATE_BACKOFF.len()]));
    }

    /// A refusal asked again at once answers on the third try: three questions, the range whole.
    #[test]
    fn a_refused_range_is_asked_again_before_it_is_split() {
        let mut node = logs(|_, _, n| if n < 2 { Err(node_says(-32000, "busy")) } else { empty() });
        assert!(scan_window(&mut node, &window(1, 1000)).is_ok());
        assert_eq!(node.asked, vec![(1, 1000); 3]);
    }

    /// Refused three times, the range shrinks to the limit the node states, else by half; one block still refused
    /// passes the node's words on; the shrinks are bounded.
    #[test]
    fn a_range_shrinks_to_the_stated_limit_or_by_half() {
        let mut node = logs(|f, t, _| if t - f + 1 > 50 { Err(node_says(-32000, "maximum block range: 50")) } else { empty() });
        assert!(scan_window(&mut node, &window(1, 100)).is_ok());
        assert_eq!(&node.asked[3..], &[(1, 50), (51, 100)]);
        let mut node = logs(|f, t, _| if t - f + 1 > 25 { Err(node_says(-32000, "try a smaller range")) } else { empty() });
        assert!(scan_window(&mut node, &window(1, 100)).is_ok());
        assert_eq!(node.asked[3], (1, 50), "no number: half");
        assert_eq!(node.asked[6], (1, 25));
        let mut node = logs(|_, _, _| Err(node_says(-32000, "always no")));
        match scan_window(&mut node, &window(1, 1)) {
            Err(Refusal::Unanswered { what, .. }) => assert!(what.contains("always no"), "{what}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(node.asked.len(), TRIES);
        let mut node = logs(|_, _, _| Err(node_says(-32000, "always no")));
        assert!(scan_window(&mut node, &window(1, 1 << 20)).is_err());
        assert_eq!(node.asked.len(), TRIES * (MAX_SHRINKS + 1), "the shrinks are bounded");
    }

    /// A node's range cut once is remembered for the life of the process, by node: the next window at that node
    /// starts at the cut range (no three refusals and a shrink again); another node starts whole.
    #[test]
    fn a_nodes_cut_range_is_remembered_for_the_next_window() {
        let rule = |f: u64, t: u64, _| if t - f + 1 > 50 { Err(node_says(-32000, "maximum block range: 50")) } else { empty() };
        let mut node = logs(rule);
        assert!(scan_window(&mut node, &window(1, 100)).is_ok());
        assert_eq!(known_span(&node.name), Some(50));
        node.asked.clear();
        assert!(scan_window(&mut node, &window(101, 200)).is_ok());
        assert_eq!(node.asked, vec![(101, 150), (151, 200)], "asked at the remembered range from the start");
        // A window narrower than the remembered range is asked whole.
        node.asked.clear();
        assert!(scan_window(&mut node, &window(201, 210)).is_ok());
        assert_eq!(node.asked, vec![(201, 210)]);
        // Another node is not this one.
        let mut other = logs(|_, _, _| empty());
        assert!(scan_window(&mut other, &window(1, 100)).is_ok());
        assert_eq!((other.asked.clone(), known_span(&other.name)), (vec![(1, 100)], None));
    }

    /// A node's range is kept by its place, not by its display name: two nodes with the same name (one host,
    /// `…/eth/<key>` and `…/polygon/<key>`), the first cut to 50, the second still asked its whole window.
    #[test]
    fn two_nodes_said_alike_keep_their_ranges_apart() {
        let mut eth = logs(|f: u64, t: u64, _| if t - f + 1 > 50 { Err(node_says(-32000, "maximum block range: 50")) } else { empty() });
        let mut polygon = logs(|_, _, _| empty());
        polygon.name = eth.name.clone();
        eth.place = Some(format!("{}/eth", eth.name));
        polygon.place = Some(format!("{}/polygon", polygon.name));
        assert!(scan_window(&mut eth, &window(1, 100)).is_ok());
        assert_eq!(known_span(&eth.place()), Some(50));
        assert!(scan_window(&mut polygon, &window(1, 100)).is_ok());
        assert_eq!(polygon.asked, vec![(1, 100)], "the other place is asked whole");
        assert_eq!(known_span(&polygon.place()), None);
    }

    /// A remembered range only ever narrows: remembered at 50, then cut to 25, it is 25; a wider range taken later
    /// (100) leaves it at 25.
    #[test]
    fn a_nodes_remembered_range_keeps_the_narrowest() {
        let node = logs(|_, _, _| empty()).name;
        remember_span(&node, 50);
        assert_eq!(known_span(&node), Some(50));
        remember_span(&node, 25);
        assert_eq!(known_span(&node), Some(25), "narrowed");
        remember_span(&node, 100);
        assert_eq!(known_span(&node), Some(25), "never widened");
    }

    /// Past the ask bound the window is refused as unfinished, never as zero logs; an answer that is not a log list
    /// is refused.
    #[test]
    fn an_unfinished_window_is_refused_never_empty() {
        let mut node = logs(|f, t, _| if t > f { Err(node_says(-32000, "limited to 1 block")) } else { empty() });
        match scan_window(&mut node, &window(1, 5000)) {
            Err(Refusal::Unanswered { what, .. }) => assert!(what.contains(&MAX_ASKS.to_string()), "{what}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(node.asked.len(), MAX_ASKS);
        let mut node = logs(|_, _, _| Ok(W::of(crate::wire::Body::Null)));
        assert!(matches!(scan_window(&mut node, &window(1, 10)), Err(Refusal::Unanswered { .. })));
    }

    /// The window's extreme edges: from block 0 to the last block there is counts without overflow and
    /// ends after the last block.
    #[test]
    fn a_window_to_the_last_block_ends() {
        let mut node = logs(|_, _, _| empty());
        assert!(scan_window(&mut node, &window(0, u64::MAX)).is_ok());
        // One block more than a u64 counts: the span saturates, so the last block is a question of its own.
        assert_eq!(node.asked, vec![(0, u64::MAX - 1), (u64::MAX, u64::MAX)]);
        let mut node = logs(|_, _, _| empty());
        assert!(scan_window(&mut node, &window(u64::MAX, u64::MAX)).is_ok());
        assert_eq!(node.asked.len(), 1);
    }

    /// A node limiting its rate is asked the same range again after each pause and never has its range shrunk;
    /// past the pauses its refusal is passed on. A number in a rate-limit sentence is not read as a range limit; a
    /// 429 is a rate limit; a refusal naming a range is a range refusal even with the rate-limit code.
    #[test]
    fn a_rate_limited_node_is_waited_out_and_its_range_kept() {
        no_pauses();
        let pauses = crate::said::RATE_BACKOFF.len();
        let mut node = logs(|_, _, _| Err(node_says(-32005, "rate limit: max 25 requests per second")));
        assert!(scan_window(&mut node, &window(1, 1000)).is_err());
        assert_eq!(node.asked, vec![(1, 1000); 1 + pauses], "the same range, once and once after each pause");
        let mut node = logs(|_, _, n| if n == 0 { Err(node_says(-32005, "daily request limit exceeded")) } else { empty() });
        assert!(scan_window(&mut node, &window(1, 1000)).is_ok());
        assert_eq!(node.asked, vec![(1, 1000); 2]);
        let mut node = logs(|_, _, _| Err(crate::rpc::status("https://n.example", 429)));
        assert!(scan_window(&mut node, &window(1, 1000)).is_err());
        assert_eq!(node.asked, vec![(1, 1000); 1 + pauses]);
        // "more than 10000 results" with -32005 names results: a range refusal, split as one.
        let mut node = logs(|f, t, _| if t - f + 1 > 500 { Err(node_says(-32005, "query returned more than 10000 results")) } else { empty() });
        assert!(scan_window(&mut node, &window(1, 1000)).is_ok());
        assert_eq!(node.asked[TRIES], (1, 500));
        // The rate-limit code without rate-limit words is not waited out: asked again at once, then split.
        let mut node = logs(|_, _, _| Err(node_says(-32005, "query timeout exceeded")));
        assert!(scan_window(&mut node, &window(1, 1 << 10)).is_err());
        assert_eq!(node.asked.len(), TRIES * (MAX_SHRINKS + 1));
        let mut node = logs(|f, t, _| if t > f { Err(node_says(-32005, "maximum 1 block per query")) } else { empty() });
        assert!(scan_window(&mut node, &window(1, 3)).is_ok());
        assert_eq!(node.asked[TRIES], (1, 1));
        // A limit exceeded on the range is the range's.
        let mut node = logs(|f, t, _| if t - f + 1 > 500 { Err(node_says(-32000, "block range limit exceeded")) } else { empty() });
        assert!(scan_window(&mut node, &window(1, 1000)).is_ok());
        assert_eq!(node.asked[TRIES], (1, 500));
    }

    /// Arbitrum's official node and Sepolia's ethpandaops node, their refusals as they were returned
    /// (2026-10-05): each is read at its own limit, so the next question already fits.
    #[test]
    fn two_public_nodes_state_their_range_in_their_own_words() {
        let arbitrum = "{\"code\":-32602,\"message\":\"query spans 268435456 blocks (1 to 268435456), but only 10000000 are allowed for this request; narrow the block range\"}";
        let mut node = logs(|f, t, _| if t - f + 1 > 10_000_000 { Err(Trouble::Node(arbitrum.into())) } else { empty() });
        assert!(scan_window(&mut node, &window(1, 30_000_000)).is_ok());
        assert_eq!(&node.asked[TRIES..], &[(1, 10_000_000), (10_000_001, 20_000_000), (20_000_001, 30_000_000)]);
        let arbitrum_wide = "{\"code\":-32602,\"message\":\"query spans 16777216 blocks (1 to 16777216), but only 30000 are allowed for this request; narrow the block range, or add an address filter\"}";
        assert_eq!(said_limit(&format!("eth_getLogs 被拒:{arbitrum_wide}"), 16_777_216), Some(30_000));
        let sepolia = "{\"data\":{\"code\":\"ErrGetLogsExceededMaxAllowedRange\",\"message\":\"getLogs request exceeded max allowed range\",\"details\":{\"requestRange\":1048576,\"maxAllowedRange\":30000}},\"code\":-32012,\"message\":\"getLogs request exceeded max allowed range\"}";
        let mut node = logs(|f, t, _| if t - f + 1 > 30_000 { Err(Trouble::Node(sepolia.into())) } else { empty() });
        assert!(scan_window(&mut node, &window(1, 60_000)).is_ok());
        assert_eq!(&node.asked[TRIES..], &[(1, 30_000), (30_001, 60_000)]);
    }

    /// A transaction the node says it does not have (`null`, a lawful empty answer) drops the log from the anchor
    /// set without a failure and without a further question; a gateway's page never arrives here as `null` (the
    /// reading of answers refuses it first).
    #[test]
    fn a_transaction_the_node_does_not_have_is_no_anchor() {
        let t0 = topic0();
        let mut padded = [0u8; 32];
        padded[12..].copy_from_slice(&[0x22; 20]);
        let log = crate::wire::parse(
            format!(
                "{{\"address\":\"{}\",\"blockNumber\":\"0x5\",\"data\":\"0x\",\"logIndex\":\"0x0\",\"topics\":[\"{}\",\"{}\",\"0x{}\"],\"transactionHash\":\"0x{}\"}}",
                hex20(&[0x11; 20]),
                hexfmt::encode(&t0),
                hexfmt::encode(&padded),
                "33".repeat(32),
                "44".repeat(32)
            )
            .as_bytes(),
        )
        .expect("a log");
        let mut node = logs(|_, _, _| Ok(W::of(crate::wire::Body::Null)));
        let mut chain = Chain { id: 1, ep: &mut node };
        let got = registry_record(&mut chain, &window(1, 10), &log, &t0, &Known::default(), &mut Vec::new());
        assert!(matches!(got, Ok(None)));
        assert_eq!(node.methods, vec!["eth_getTransactionByHash".to_string()]);
    }
}
