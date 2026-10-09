//! Anchoring: one path for each of the two forms (§9.1).
//!
//! - Registry form: a call to a contract that emits `Anchored(address indexed, bytes32 indexed)`; each
//! anchored 32-byte word must sit at an aligned offset of the calldata the sender signs (plain ABI encoding
//! puts every argument at a multiple of 32).
//! - Bare form: a transaction to oneself whose calldata is exactly `k × 32` bytes, each word one anchor.
//!
//! The anchor key never holds funds beyond fees, never delegates and never has code (law §5.7, §9.3). This
//! module signs bytes in the chain's form; where the key comes from and whether to send are decided above.

use crate::rlp;
use crate::rpc::{Endpoint, Trouble};
use crate::tx;
use zikaron::cryptox;
use zikaron::hexfmt;
use zikaron::json::Value;

/// The two anchoring forms.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    /// Call `anchor` / `anchorMany` on a registry contract.
    Registry,
    /// Send to oneself; calldata is the words.
    Bare,
}

/// The highest priority fee an anchor pays, and the one it pays when the chain's priority fees cannot be read
/// (wei per gas; 1 gwei).
pub const PRIORITY_FEE: u64 = 1_000_000_000;
/// Fee cap when the chain's base fee cannot be read (wei per gas; 3 gwei). A fallback only: the cap is
/// normally computed from the base fee.
pub const MAX_FEE: u64 = 3_000_000_000;
/// The most gas an anchor ever carries: the ceiling of [`limit_for`], and the limit a transaction carries when
/// no estimate stands behind it (the command line's `anchor`, the fallback pair). An estimate above it is
/// refused before anything is shown or sent.
pub const GAS_LIMIT: u64 = 200_000;

/// The gas limit a transaction carries for the node's estimate: one and a half times the estimate, rounded
/// up, held at [`GAS_LIMIT`]. The one place this number is worked out: the balance check before sending, the
/// confirmation card and the signed transaction all read it through [`Fees::gas_limit`].
pub fn limit_for(estimate: u64) -> u64 {
    estimate.saturating_mul(3).div_ceil(2).min(GAS_LIMIT)
}
/// Why an estimate gives no gas figure to send with. Closed; each caller says each member in its own terms.
#[derive(Clone, Debug, PartialEq)]
pub enum NoGas<E> {
    /// The transport broke, or the node refused in the network kind (the caller's judgement): it says nothing
    /// about the call.
    Network(E),
    /// The node answered the estimate and refused: sending would revert.
    Refused(E),
    /// The answer is not text.
    NotText(Value),
    /// The text is not a `0x` quantity of at most 128 bits.
    Unreadable(String),
    /// Above [`GAS_LIMIT`]: on chain it would run out of gas with the fee still paid.
    OverCap(u128),
}

/// The call an estimate asks the node about: the very transaction's `from`, `to` and `data`, so the estimate
/// is of the bytes that are sent.
pub fn estimate_call(from: &[u8; 20], to: &[u8; 20], data: &[u8]) -> Value {
    Value::Obj(vec![
        ("data".into(), Value::Str(hexfmt::encode(data))),
        ("from".into(), Value::Str(hexfmt::encode(from))),
        ("to".into(), Value::Str(hexfmt::encode(to))),
    ])
}

/// One gas estimate, the one rule the app and the command line both take; each brings its own way of asking
/// (the app its endpoints' agreement, the command line its one node). The estimate is asked at one pinned
/// block, `head` (the app's smallest head every endpoint has reached): asked at `latest`, two endpoints a block
/// apart estimate over two different states and disagree over a call neither refuses. A failure the caller
/// judges of the network kind says nothing about the call; any other refusal means the call would revert. The
/// answer is read as a quantity of at most 128 bits, and one above [`GAS_LIMIT`] is refused by the node's own
/// number before anything is shown or sent.
pub fn estimate_gas<E>(
    head: impl FnOnce() -> Result<u64, E>,
    ask: impl FnOnce(&Value) -> Result<Value, E>,
    call: Value,
    is_network: impl Fn(&E) -> bool,
) -> Result<u64, NoGas<E>> {
    crate::seam();
    let v = head()
        .and_then(|height| ask(&Value::Arr(vec![call, Value::Str(format!("0x{height:x}"))])))
        .map_err(|e| if is_network(&e) { NoGas::Network(e) } else { NoGas::Refused(e) })?;
    let hex = match &v {
        Value::Str(s) => s.clone(),
        other => return Err(NoGas::NotText(other.clone())),
    };
    let n = quantity_128(&hex).ok_or_else(|| NoGas::Unreadable(hex.clone()))?;
    if n > GAS_LIMIT as u128 {
        return Err(NoGas::OverCap(n));
    }
    Ok(n as u64)
}

/// A `0x` quantity that fits in 128 bits (hex digits in either case after a lowercase `0x`).
fn quantity_128(x: &str) -> Option<u128> {
    let body = x.strip_prefix("0x")?;
    if body.is_empty() || body.len() > 32 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u128::from_str_radix(body, 16).ok()
}

/// How much both fee fields of a replacement must rise over the transaction it replaces, in percent: the
/// replacement rule of the nodes' transaction pools (go-ethereum's `txpool.pricebump`, ten by default).
pub const REPLACE_BUMP_PERCENT: u64 = 10;

/// How many blocks the priority fee is read over, ending at the pinned block (`eth_feeHistory`).
pub const TIP_BLOCKS: u64 = 20;
/// Which percentile of each block's paid priority fees is asked (the median).
pub const TIP_PERCENTILE: u64 = 50;

/// This transaction's two fee fields, fee cap and priority fee, in wei per gas, and where they came from, with
/// the gas limit it carries.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fees {
    pub max_fee: u64,
    pub priority: u64,
    /// The gas limit the transaction carries: [`GAS_LIMIT`] until an estimate is known
    /// ([`Fees::with_estimate`]).
    pub gas_limit: u64,
    /// The cap was computed from a base fee the chain gave ([`Fees::of`]); `false` for the fallback pair. What
    /// reads "fallback or not" reads this, never the numbers: a base fee of exactly 1 gwei with the priority fee
    /// at its ceiling makes the computed pair equal to the fallback pair.
    pub from_chain: bool,
}

impl Fees {
    /// The fallback when the base fee cannot be read.
    pub fn fallback() -> Fees {
        Fees { max_fee: MAX_FEE, priority: PRIORITY_FEE, gas_limit: GAS_LIMIT, from_chain: false }
    }

    /// The one fee rule, from what the chain says: the priority fee is `tip` (the median of paid priority
    /// fees, [`tip_of`]) held at most [`PRIORITY_FEE`], and [`PRIORITY_FEE`] when it could not be read; the cap
    /// is base fee × 2 + priority fee (the base fee grows at most one eighth per block, so double covers
    /// several blocks). Without a base fee the whole pair is the fallback. The app and the command line both
    /// come here, each with the facts it read.
    pub fn of(base_fee: Option<u64>, tip: Option<u64>) -> Fees {
        let Some(base) = base_fee else { return Fees::fallback() };
        let priority = tip.map(|t| t.min(PRIORITY_FEE)).unwrap_or(PRIORITY_FEE);
        Fees { max_fee: base.saturating_mul(2).saturating_add(priority), priority, gas_limit: GAS_LIMIT, from_chain: true }
    }

    /// The fees a replacement of a transaction sent with `old` carries (same nonce, same gas limit): the one fee
    /// rule's pair read now (`now`, [`Fees::of`]), each field at least [`REPLACE_BUMP_PERCENT`] above the old
    /// one (a node takes a replacement only when both fields rise by that much), and the priority fee never
    /// above the cap. Where it came from is where `now` came from.
    pub fn replacing(old: Fees, now: Fees) -> Fees {
        let up = |x: u64| x.saturating_mul(100 + REPLACE_BUMP_PERCENT).div_ceil(100).max(x.saturating_add(1));
        let max_fee = now.max_fee.max(up(old.max_fee));
        let priority = now.priority.max(up(old.priority)).min(max_fee);
        Fees { max_fee, priority, gas_limit: old.gas_limit, from_chain: now.from_chain }
    }

    /// These fees for a transaction the node estimated at `estimate` gas: the gas limit becomes
    /// [`limit_for`]`(estimate)`; the fee fields stay.
    pub fn with_estimate(self, estimate: u64) -> Fees {
        Fees { gas_limit: limit_for(estimate), ..self }
    }

    /// The most this transaction can spend in wei: fee cap times the gas limit it carries. The balance check
    /// before sending and the confirmation card read it; it is what the node checks at broadcast (`insufficient
    /// funds for gas * price + value`).
    pub fn cap_wei(self) -> u128 {
        self.max_fee as u128 * self.gas_limit as u128
    }
}

/// The parameters of the priority-fee question at the pinned block `block`: [`TIP_BLOCKS`] blocks ending
/// there, each block's [`TIP_PERCENTILE`] percentile of paid priority fees.
pub fn tip_params(block: u64) -> Value {
    Value::Arr(vec![
        Value::Str(format!("0x{TIP_BLOCKS:x}")),
        Value::Str(format!("0x{block:x}")),
        Value::Arr(vec![Value::Int(TIP_PERCENTILE)]),
    ])
}

/// The priority fee a fee history says (its `reward` member: one list per block, holding that block's median
/// paid priority fee): the median of those block medians (the lower one of the middle two for an even
/// count), over the blocks whose median is not zero. A block that paid no priority fee at all (an empty
/// block, or one of transactions paying none) says nothing about what a transaction must pay to get in, and
/// on a chain of many such blocks counting them would make the fee zero and leave a transaction waiting. When
/// every block's median is zero, the fee is zero, as the chain says. `None` when it cannot be read: no answer
/// (`null`), no `reward`, no block, or any entry not a `0x` quantity.
pub fn tip_of(history: &Value) -> Option<u64> {
    let Some(Value::Arr(blocks)) = history.member("reward") else { return None };
    let mut tips: Vec<u64> = Vec::with_capacity(blocks.len());
    for b in blocks {
        let Value::Arr(cells) = b else { return None };
        let [Value::Str(one)] = cells.as_slice() else { return None };
        tips.push(quantity(one)?);
    }
    if tips.is_empty() {
        return None;
    }
    let paid: Vec<u64> = tips.iter().copied().filter(|t| *t != 0).collect();
    let mut over = if paid.is_empty() { tips } else { paid };
    over.sort_unstable();
    Some(over[(over.len() - 1) / 2])
}

/// The base fee a block says (its `baseFeePerGas`), read one way for every caller (the app over several nodes,
/// the command line over its one): a `0x` quantity of at most 32 hex digits, leading zeros allowed, whose value
/// fits 64 bits. Anything else (missing, not a string, not hex, longer, larger) is unread, and the caller's
/// rule for an unread base fee applies.
pub fn base_fee_of(block: &Value) -> Option<u64> {
    let body = block.member("baseFeePerGas")?.as_str()?.strip_prefix("0x")?;
    if body.is_empty() || body.len() > 32 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u128::from_str_radix(body, 16).ok().and_then(|n| u64::try_from(n).ok())
}

/// The fees one node says (the command line's `anchor`, which has one node): its head, that block's base fee
/// and the priority fees paid up to it, then [`Fees::of`]. Whatever it does not answer, or answers in another
/// shape, counts as unread.
pub fn read_fees(ep: &mut dyn Endpoint) -> Fees {
    crate::seam();
    // Each answer read as its question's decision reads it (`judge::read_as_decided`): a fee history's
    // fractional `gasUsedRatio` never makes its `reward` unreadable.
    let mut ask = |method: &str, params: &Value| crate::patience::ask(ep, method, params).ok().and_then(|w| crate::judge::read_as_decided(method, params, &w));
    let Some(head) = ask("eth_blockNumber", &Value::Arr(vec![])).as_ref().and_then(|v| v.as_str().and_then(quantity)) else {
        return Fees::fallback();
    };
    let block = Value::Arr(vec![Value::Str(format!("0x{head:x}")), Value::Bool(false)]);
    let base = ask("eth_getBlockByNumber", &block).and_then(|b| base_fee_of(&b));
    let tip = ask("eth_feeHistory", &tip_params(head)).and_then(|h| tip_of(&h));
    Fees::of(base, tip)
}

/// A `0x` quantity that fits in 64 bits.
fn quantity(x: &str) -> Option<u64> {
    let body = x.strip_prefix("0x")?;
    if body.is_empty() || body.len() > 16 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(body, 16).ok()
}

/// The cap of the fallback fees.
pub fn fee_cap_wei() -> u128 {
    Fees::fallback().cap_wei()
}

/// Selector of `anchor(bytes32)`.
pub fn selector_anchor() -> [u8; 4] {
    selector("anchor(bytes32)")
}

/// Selector of `anchorMany(bytes32[])`.
pub fn selector_anchor_many() -> [u8; 4] {
    selector("anchorMany(bytes32[])")
}

fn selector(sig: &str) -> [u8; 4] {
    let h = cryptox::keccak256(sig.as_bytes());
    [h[0], h[1], h[2], h[3]]
}

/// Registry-form calldata: `anchor` for one hash, `anchorMany` for several.
pub fn registry_calldata(hashes: &[[u8; 32]]) -> Vec<u8> {
    if hashes.len() == 1 {
        let mut out = selector_anchor().to_vec();
        out.extend_from_slice(&hashes[0]);
        return out;
    }
    let mut out = selector_anchor_many().to_vec();
    let mut head = [0u8; 32];
    head[24..].copy_from_slice(&32u64.to_be_bytes());  // Array offset: after one head word.
    out.extend_from_slice(&head);
    let mut len = [0u8; 32];
    len[24..].copy_from_slice(&(hashes.len() as u64).to_be_bytes());
    out.extend_from_slice(&len);
    for h in hashes {
        out.extend_from_slice(h);
    }
    out
}

/// Bare-form calldata: the words back to back.
pub fn bare_calldata(hashes: &[[u8; 32]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(hashes.len() * 32);
    for h in hashes {
        out.extend_from_slice(h);
    }
    out
}

/// An EIP-1559 transaction to be signed.
pub struct Unsigned {
    pub chain_id: u64,
    pub nonce: u64,
    pub max_priority_fee: u64,
    pub max_fee: u64,
    pub gas: u64,
    pub to: [u8; 20],
    pub value: u64,
    pub data: Vec<u8>,
}

impl Unsigned {
    fn fields(&self) -> Vec<Vec<u8>> {
        vec![
            rlp::quantity(self.chain_id),
            rlp::quantity(self.nonce),
            rlp::quantity(self.max_priority_fee),
            rlp::quantity(self.max_fee),
            rlp::quantity(self.gas),
            rlp::bytes(&self.to),
            rlp::quantity(self.value),
            rlp::bytes(&self.data),
            rlp::list(&[]),
        ]
    }

    /// Signing payload: `0x02 || rlp([...])`.
    pub fn payload(&self) -> Vec<u8> {
        let mut p = vec![2u8];
        p.extend_from_slice(&rlp::list(&self.fields()));
        p
    }

    /// Sign the whole transaction; returns the raw bytes and the hash.
    pub fn sign(&self, key: &[u8; 32]) -> Option<(Vec<u8>, [u8; 32])> {
        let digest = cryptox::keccak256(&self.payload());
        let (r, s, v) = cryptox::sign_digest(key, &digest)?;
        // `sign_digest` returns v as 27/28 (the EIP-191 convention); typed transactions need yParity.
        let y = v.checked_sub(27)?;
        let mut full = self.fields();
        full.push(rlp::scalar(&[y]));
        full.push(rlp::scalar(&r));
        full.push(rlp::scalar(&s));
        let mut raw = vec![2u8];
        raw.extend_from_slice(&rlp::list(&full));
        let hash = cryptox::keccak256(&raw);
        Some((raw, hash))
    }
}

/// Where a broadcast transaction ended up. Three named states: "included with status N", "not included yet"
/// and "lost sight of it" differ, and folding them into an `Option` would print a slow inclusion on a real
/// chain as a failure.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Confirm {
    /// The receipt arrived.
    Included { status: u64, block_number: u64 },
    /// Not included by the deadline. Normal on a real chain, not a verdict.
    NotYet,
    /// The endpoint went silent while waiting. The bytes were already broadcast, so all that can be said is
    /// that it is out of sight.
    Unreachable(String),
    /// Some nodes give the receipt and others still say "not yet", after the judging table's re-asks
    /// (`judge::NOT_YET_PAUSES`): the receipt is not taken on the word of some nodes alone. Each list names its
    /// nodes in order.
    Split { has: Vec<String>, not_yet: Vec<String> },
}

/// The result of one anchoring. The transaction hash is always present: the bytes were broadcast and must be
/// traceable.
pub struct Sent {
    pub tx: [u8; 32],
    pub form: Form,
    pub confirm: Confirm,
}

impl Sent {
    /// Whether this transaction counts as anchored. Decided here, not in the shell: §9.1 says a transaction
    /// with status other than 1 is no anchor in either form.
    pub fn anchored(&self) -> bool {
        matches!(self.confirm, Confirm::Included { status: 1, .. })
    }
}

/// Send one or more anchors, wait for inclusion and bring back the receipt status.
///
/// The node's hash echo only shows it took the bytes. A call that reverts for lack of gas and a transaction
/// priced below the base fee that never gets included echo exactly like a success, and §9.1 says a status
/// other than 1 is no anchor. So the receipt is awaited.
///
/// The caller sets the wait: six seconds is shorter than a block on any real chain. Not included by then is
/// "not yet"; an endpoint lost after broadcast is "out of sight"; neither is a failure verdict.
///
/// This joins [`read_fees`], [`broadcast`] and [`confirm`], all on the one node given. Callers that persist
/// "submitted" between the two stages use them directly, so after a restart they wait for the same transaction
/// without resending.
pub fn anchor(
    ep: &mut dyn Endpoint,
    key: &[u8; 32],
    chain_id: u64,
    form: Form,
    registry: Option<[u8; 20]>,
    hashes: &[[u8; 32]],
    calldata_override: Option<Vec<u8>>,
    wait: std::time::Duration,
) -> Result<Sent, Trouble> {
    crate::seam();
    let fees = read_fees(ep);
    let hash = broadcast(ep, key, chain_id, form, registry, hashes, calldata_override, fees)?;
    let confirm = confirm(ep, &hash, wait);
    Ok(Sent { tx: hash, form, confirm })
}

/// Why [`anchor_estimated`] sent nothing, or did not get its answer.
#[derive(Debug)]
pub enum NotSent {
    /// The estimate gave no gas figure ([`estimate_gas`]): nothing was broadcast.
    Gas(NoGas<Trouble>),
    /// The nonce could not be read, or signing failed: nothing was broadcast.
    Send(Trouble),
    /// The signed transaction's hash could not be landed where the caller keeps it (the caller's words):
    /// nothing was broadcast, so a rerun never meets a transaction it has no record of.
    Land(String),
    /// The broadcast itself failed (the node refused it, its echo was not the hash signed, or the transport
    /// broke): the hash was landed before it, and the bytes may be in a pool, so the hash goes with the trouble.
    Broadcast([u8; 32], Trouble),
}

/// [`anchor`] with the estimate before the send (the command line's `anchor`, which has one node): its fees,
/// then one estimate of this very transaction at the head that node gives ([`estimate_gas`], the rule the app
/// takes), the limit it carries following that estimate ([`limit_for`]), then broadcast and wait. An estimate
/// the node refuses, or one above [`GAS_LIMIT`], sends nothing. Of what the node says, a refusal about the
/// call itself (the one table's `said::refuses_the_call`: a member about the call, or the node's JSON-RPC
/// error with a numeric `code` the table does not name) is the call's refusal; a rate limit, a missing
/// method, credentials, a wrong chain, an error without a numeric `code`, a page at an HTTP status, a
/// transport failure, a recording that lacks the question or contradicts itself is the network's.
/// Returns what was sent with the limit it carried.
#[allow(clippy::too_many_arguments)]
pub fn anchor_estimated(
    ep: &mut dyn Endpoint,
    key: &[u8; 32],
    chain_id: u64,
    form: Form,
    registry: Option<[u8; 20]>,
    hashes: &[[u8; 32]],
    calldata_override: Option<Vec<u8>>,
    wait: std::time::Duration,
) -> Result<(Sent, u64), NotSent> {
    anchor_landed(ep, key, chain_id, form, registry, hashes, calldata_override, wait, None, &mut |_, _| Ok(()))
}

/// [`anchor_estimated`] with the two things a sender that keeps its own record of what it sent needs (the
/// command line's direct `anchor`): the nonce to sign at (`None`: the node's pending one; `Some`: the nonce of
/// transactions sent earlier that no node holds and whose nonce is unused, [`Earlier::Unused`], so at most one
/// of them can ever be included), and `land`, called with the nonce and the hash once the transaction is
/// signed and before it is broadcast: a hash it cannot land sends nothing ([`NotSent::Land`]), so no
/// transaction ever goes out that a rerun would not ask about first. A broadcast that fails after it answers
/// with that hash ([`NotSent::Broadcast`]).
#[allow(clippy::too_many_arguments)]
pub fn anchor_landed(
    ep: &mut dyn Endpoint,
    key: &[u8; 32],
    chain_id: u64,
    form: Form,
    registry: Option<[u8; 20]>,
    hashes: &[[u8; 32]],
    calldata_override: Option<Vec<u8>>,
    wait: std::time::Duration,
    nonce: Option<u64>,
    land: &mut dyn FnMut(u64, &[u8; 32]) -> Result<(), String>,
) -> Result<(Sent, u64), NotSent> {
    crate::seam();
    let fees = read_fees(ep);
    let from = cryptox::address_of_privkey(key).ok_or(NotSent::Send(Trouble::Transport("私钥不在曲线的范围里".into())))?;
    let (to, data) = target(form, registry, hashes, calldata_override.clone(), from).map_err(NotSent::Send)?;
    let call = estimate_call(&from, &to, &data);
    let gas = {
        let mut ask = |method: &str, params: &Value| -> Result<Value, Trouble> {
            let w = crate::patience::ask(ep, method, params)?;
            crate::judge::read_as_decided(method, params, &w).ok_or_else(|| Trouble::Transport(format!("{method} 的答读不成")))
        };
        // The head is not the call: a node that will not give it says nothing about the call either.
        let head = match ask("eth_blockNumber", &Value::Arr(vec![])) {
            Ok(v) => v.as_str().and_then(quantity).ok_or_else(|| Trouble::Transport("链头读不出".into())),
            Err(Trouble::Node(m)) => Err(Trouble::Transport(m)),
            Err(t) => Err(t),
        };
        // Read by the one judgement the app takes too (`said::refuses_the_call`): only a refusal about the
        // call itself says the call would revert; a rate limit, a missing method, credentials, a wrong chain
        // or words that are not the node's coded refusal say nothing about it, as a broken transport does not.
        estimate_gas(|| head, |params| ask("eth_estimateGas", params), call, |t| !crate::said::refuses_the_call(t))
    }
    .map_err(NotSent::Gas)?;
    let fees = fees.with_estimate(gas);
    let nonce = match nonce {
        Some(n) => n,
        None => {
            let v = crate::patience::ask(ep, "eth_getTransactionCount", &nonce_params(&from)).map_err(NotSent::Send)?;
            tx::hex_qty(&v).ok_or(NotSent::Send(Trouble::Transport("nonce 读不出".into())))?
        }
    };
    let (raw, hash) = sign_at(key, chain_id, form, registry, hashes, calldata_override, fees, nonce).map_err(NotSent::Send)?;
    land(nonce, &hash).map_err(NotSent::Land)?;
    submit(ep, &raw, &hash).map_err(|t| NotSent::Broadcast(hash, t))?;
    let confirm = confirm(ep, &hash, wait);
    Ok((Sent { tx: hash, form, confirm }, fees.gas_limit))
}

/// What becomes of transactions sent at one nonce that no node holds and none of which has a receipt (dropped
/// from every pool, or their nonce used by another transaction). Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unheld {
    /// Their nonce is used on chain and, asked once more, none has a receipt: void, never to be included.
    Void,
    /// Their nonce is unused: they may be sent again at that very nonce (at most one of them can be included).
    Unused,
    /// Not judged: the account's nonce on chain was not read, or a receipt was heard on asking again.
    Wait,
}

/// The one rule for transactions no node holds, judged by the sending account's nonce on chain (`latest`,
/// what blocks have used) against theirs (`mine`): past it, every one of them is asked for its receipt once more
/// (`none_included`: a node past the nonce has the block, and the receipt if it was one of these) and they are
/// void when none has one; not past it, unused. The app's queue (a batch no node holds) and the command line's
/// direct `anchor` (a rerun, [`earlier`]) both judge by this, each with its own way of asking.
pub fn unheld(on_chain: Option<u64>, mine: u64, none_included: impl FnOnce() -> bool) -> Unheld {
    match on_chain {
        None => Unheld::Wait,
        Some(n) if n > mine => match none_included() {
            true => Unheld::Void,
            false => Unheld::Wait,
        },
        Some(_) => Unheld::Unused,
    }
}

/// Where transactions sent earlier for the same anchoring stand, asked of one node by their hashes before
/// anything new is signed (the command line's direct `anchor`, on a rerun). Closed.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Earlier {
    /// One of them has its receipt (the first that does, in the order sent): included, with its status and block.
    Included { tx: [u8; 32], status: u64, block_number: u64 },
    /// The node holds one of them and none has a receipt: in flight. Nothing new is signed.
    Held { tx: [u8; 32] },
    /// None is held or included, and their nonce is used on chain ([`Unheld::Void`]): void.
    Void,
    /// None is held or included, and their nonce is unused ([`Unheld::Unused`]): sendable again at it.
    Unused { nonce: u64 },
    /// The node did not answer one of the questions, answered one in another shape, or a receipt was heard
    /// on asking again: nothing can be said, and nothing new is signed (`tx`, the last sent; the words).
    Unread { tx: [u8; 32], said: String },
}

/// Ask one node where the transactions sent earlier stand (`sent`: each one's nonce and hash, in the order
/// sent; not empty): each one's receipt, then whether the node holds each, then the account's nonce on chain
/// judged by [`unheld`] against the largest nonce they were sent at. Every question goes through the patience
/// table. A receipt is read by its status and block, a held transaction as an object carrying its hash, the
/// nonce as a quantity; `null` is "no"; anything else is [`Earlier::Unread`].
pub fn earlier(ep: &mut dyn Endpoint, from: &[u8; 20], sent: &[(u64, [u8; 32])]) -> Earlier {
    crate::seam();
    let last = sent.last().map(|(_, h)| *h).unwrap_or([0u8; 32]);
    let unread = |said: String| Earlier::Unread { tx: last, said };
    let by_hash = |h: &[u8; 32]| Value::Arr(vec![Value::Str(hexfmt::encode(h))]);
    // One receipt: a receipt (its status and block), not yet (`None`), or unread (the words).
    let receipt = |ep: &mut dyn Endpoint, h: &[u8; 32]| -> Result<Option<(u64, u64)>, String> {
        let w = crate::patience::ask(ep, "eth_getTransactionReceipt", &by_hash(h)).map_err(|t| format!("eth_getTransactionReceipt {t:?}"))?;
        if w.is_null() {
            return Ok(None);
        }
        match (tx::qty_u64(&w, "status"), tx::qty_u64(&w, "blockNumber")) {
            (Some(s), Some(b)) => Ok(Some((s, b))),
            _ => Err("收据里没有状态或块号".into()),
        }
    };
    for (_, h) in sent {
        match receipt(ep, h) {
            Ok(Some((status, block_number))) => return Earlier::Included { tx: *h, status, block_number },
            Ok(None) => {}
            Err(said) => return unread(said),
        }
    }
    for (_, h) in sent {
        match crate::patience::ask(ep, "eth_getTransactionByHash", &by_hash(h)) {
            Ok(w) if w.is_null() => {}
            Ok(w) if w.member("hash").is_some() => return Earlier::Held { tx: *h },
            Ok(w) => return unread(format!("eth_getTransactionByHash {}", crate::wire::write(&w))),
            Err(t) => return unread(format!("eth_getTransactionByHash {t:?}")),
        }
    }
    let latest = Value::Arr(vec![Value::Str(hexfmt::encode(from)), Value::Str("latest".into())]);
    let on_chain = match crate::patience::ask(ep, "eth_getTransactionCount", &latest) {
        Ok(w) => match tx::hex_qty(&w) {
            Some(n) => n,
            None => return unread(format!("eth_getTransactionCount {}", crate::wire::write(&w))),
        },
        Err(t) => return unread(format!("eth_getTransactionCount {t:?}")),
    };
    let mine = sent.iter().map(|(n, _)| *n).max().unwrap_or(0);
    let mut heard: Option<Earlier> = None;
    let judged = unheld(Some(on_chain), mine, || {
        for (_, h) in sent {
            match receipt(ep, h) {
                Ok(None) => {}
                Ok(Some((status, block_number))) => {
                    heard = Some(Earlier::Included { tx: *h, status, block_number });
                    return false;
                }
                Err(said) => {
                    heard = Some(unread(said));
                    return false;
                }
            }
        }
        true
    });
    match judged {
        Unheld::Void => Earlier::Void,
        Unheld::Unused => Earlier::Unused { nonce: mine },
        Unheld::Wait => heard.unwrap_or_else(|| unread("eth_getTransactionCount".into())),
    }
}

/// Sign and broadcast one transaction; returns its hash when the echo matches. A mismatched echo, a node
/// refusal and a transport failure each return as they are. Signing and submitting live apart ([`sign_for`]
/// and [`submit`]), so one signed transaction can go to several endpoints with its nonce fixed.
#[allow(clippy::too_many_arguments)]
pub fn broadcast(
    ep: &mut dyn Endpoint,
    key: &[u8; 32],
    chain_id: u64,
    form: Form,
    registry: Option<[u8; 20]>,
    hashes: &[[u8; 32]],
    calldata_override: Option<Vec<u8>>,
    fees: Fees,
) -> Result<[u8; 32], Trouble> {
    crate::seam();
    let (raw, hash) = sign_for(ep, key, chain_id, form, registry, hashes, calldata_override, fees)?;
    submit(ep, &raw, &hash)?;
    Ok(hash)
}

/// Sign this transaction with the nonce read from `ep`; returns the raw bytes and hash. Nothing is sent.
#[allow(clippy::too_many_arguments)]
pub fn sign_for(
    ep: &mut dyn Endpoint,
    key: &[u8; 32],
    chain_id: u64,
    form: Form,
    registry: Option<[u8; 20]>,
    hashes: &[[u8; 32]],
    calldata_override: Option<Vec<u8>>,
    fees: Fees,
) -> Result<(Vec<u8>, [u8; 32]), Trouble> {
    crate::seam();
    let from = cryptox::address_of_privkey(key).ok_or(Trouble::Transport("私钥不在曲线的范围里".into()))?;
    let v = crate::patience::ask(ep, "eth_getTransactionCount", &nonce_params(&from))?;
    let nonce = tx::hex_qty(&v).ok_or(Trouble::Transport("nonce 读不出".into()))?;
    sign_at(key, chain_id, form, registry, hashes, calldata_override, fees, nonce)
}

/// The nonce question for `from`: `pending`, so while an earlier batch is still in the pool the new one follows
/// it and does not reuse its nonce (reusing it would replace the earlier one or be refused as underpriced).
pub fn nonce_params(from: &[u8; 20]) -> Value {
    Value::Arr(vec![Value::Str(hexfmt::encode(from)), Value::Str("pending".into())])
}

/// Sign this transaction with the nonce given (asked of several nodes by the caller and judged by the one
/// table: the largest pending nonce, `judge::rule_of`); returns the raw bytes and hash. Nothing is sent.
#[allow(clippy::too_many_arguments)]
pub fn sign_at(
    key: &[u8; 32],
    chain_id: u64,
    form: Form,
    registry: Option<[u8; 20]>,
    hashes: &[[u8; 32]],
    calldata_override: Option<Vec<u8>>,
    fees: Fees,
    nonce: u64,
) -> Result<(Vec<u8>, [u8; 32]), Trouble> {
    let from = cryptox::address_of_privkey(key).ok_or(Trouble::Transport("私钥不在曲线的范围里".into()))?;
    let (to, data) = target(form, registry, hashes, calldata_override, from)?;
    let unsigned = Unsigned {
        chain_id,
        nonce,
        max_priority_fee: fees.priority,
        max_fee: fees.max_fee,
        gas: fees.gas_limit,
        to,
        value: 0,
        data,
    };
    unsigned.sign(key).ok_or(Trouble::Transport("签不出来".into()))
}

/// Where this transaction goes and what it carries: a registry's `anchor`/`anchorMany`, or to oneself with the
/// words. The one place signing, estimating and the command line's record of what it sent ([`earlier`]) take
/// them from, so the estimate is of the sent bytes and a rerun is known by them.
pub fn target(form: Form, registry: Option<[u8; 20]>, hashes: &[[u8; 32]], calldata_override: Option<Vec<u8>>, from: [u8; 20]) -> Result<([u8; 20], Vec<u8>), Trouble> {
    Ok(match form {
        Form::Registry => {
            let r = registry.ok_or(Trouble::Transport("登记形制要一个合约地址".into()))?;
            (r, calldata_override.unwrap_or_else(|| registry_calldata(hashes)))
        }
        Form::Bare => (from, calldata_override.unwrap_or_else(|| bare_calldata(hashes))),
    })
}

/// Hand the signed transaction to one node; done when the echo matches. The same bytes sent to several nodes
/// are one transaction (same hash).
pub fn submit(ep: &mut dyn Endpoint, raw: &[u8], hash: &[u8; 32]) -> Result<(), Trouble> {
    crate::seam();
    let sent = ep.call("eth_sendRawTransaction", &Value::Arr(vec![Value::Str(hexfmt::encode(raw))]))?;
    let echoed = sent.as_str().and_then(hexfmt::decode).unwrap_or_default();
    if echoed != hash {
        return Err(Trouble::Transport("节点回的交易哈希不是我们签的那一笔".into()));
    }
    Ok(())
}

/// Wait for the receipt until the caller's deadline. No `?` anywhere: the transaction is already out, so this
/// only says "included with status N", "not yet" or "out of sight".
pub fn confirm(ep: &mut dyn Endpoint, hash: &[u8; 32], wait: std::time::Duration) -> Confirm {
    confirm_each(&mut [ep], hash, wait, &RECEIPT_BACKOFF)
}

/// The pauses between rounds of asking for a receipt: doubling from the first, held at the last.
pub const RECEIPT_BACKOFF: [std::time::Duration; 4] = [
    std::time::Duration::from_millis(250),
    std::time::Duration::from_millis(500),
    std::time::Duration::from_millis(1000),
    std::time::Duration::from_millis(2000),
];

/// Ask each endpoint for the same receipt, round after round until the caller's deadline. A receipt counts when
/// every endpoint that answered gives it, alike (block and status, `judge::RECEIPT_FACTS`); an endpoint that
/// does not answer is passed over. When some give it and others say "not yet", it is asked again after each
/// of the judging table's pauses (`judge::not_yet_pauses`), and still split after the last it is
/// [`Confirm::Split`], naming both sides: one node's word alone does not anchor anything. A round in which
/// every endpoint fails (rate limited, down for a moment) is not an answer: the transaction is already out, and
/// a node that refuses one round may give the receipt the next, so the rounds go on, paused by `backoff` (the
/// product's is [`RECEIPT_BACKOFF`]; a test gives its own so no wall clock is waited; an empty list does not
/// pause). At the deadline it is "not yet" when any endpoint ever answered (it did not know the receipt), and
/// "out of sight" (with the last sentence) when none ever did. Asking only the endpoint that took the broadcast
/// would wait forever once that endpoint went down.
///
/// The wait is one deadline over every question: each question is given only the wait left
/// ([`Endpoint::call_within`]), and once it is spent no further endpoint is asked, so a node that does not
/// answer cannot stretch the wait by a whole deadline of its own. A wait of zero asks one round, each
/// endpoint within its own deadline.
pub fn confirm_each(eps: &mut [&mut dyn Endpoint], hash: &[u8; 32], wait: std::time::Duration, backoff: &[std::time::Duration]) -> Confirm {
    if eps.is_empty() {
        return Confirm::Unreachable("没有可问的端点".into());
    }
    let deadline = std::time::Instant::now() + wait;
    let q = Value::Arr(vec![Value::Str(hexfmt::encode(hash))]);
    let mut last: Option<String> = None;
    let mut ever_answered = false;
    let mut round = 0usize;
    let split_pauses = crate::judge::not_yet_pauses();
    let mut split_rounds = 0usize;
    loop {
        // This round's receipts (by node) and the nodes that said "not yet".
        let mut has: Vec<(String, (u64, u64))> = Vec::new();
        let mut not_yet: Vec<String> = Vec::new();
        // A round the deadline cut before every node was asked decides nothing: a receipt is never taken on the
        // word of the nodes asked before the cut.
        let mut cut = false;
        for ep in eps.iter_mut() {
            let asked = if wait.is_zero() {
                ep.call("eth_getTransactionReceipt", &q)
            } else {
                let left = deadline.saturating_duration_since(std::time::Instant::now());
                if left.is_zero() {
                    cut = true;
                    break;
                }
                ep.call_within("eth_getTransactionReceipt", &q, left)
            };
            match asked {
                Ok(r) if !r.is_null() => match (tx::qty_u64(&r, "status"), tx::qty_u64(&r, "blockNumber")) {
                    (Some(status), Some(block_number)) => {
                        ever_answered = true;
                        has.push((ep.name(), (status, block_number)));
                    }
                    // This endpoint's receipt is incomplete: it answered (so the deadline says "not yet", not "out of
                    // sight"); note the sentence and ask the next one.
                    _ => {
                        ever_answered = true;
                        last = Some("收据里没有状态或块号".into());
                    }
                },
                Ok(_) => {
                    ever_answered = true;
                    not_yet.push(ep.name());
                }
                Err(e) => last = Some(format!("{e:?}")),
            }
        }
        if let Some((_, first)) = has.first().filter(|_| !cut) {
            let alike = has.iter().all(|(_, r)| r == first);
            let (status, block_number) = *first;
            match (alike, not_yet.is_empty()) {
                (true, true) => return Confirm::Included { status, block_number },
                // Split: asked again after the judging table's pauses, then named.
                (true, false) => match split_pauses.get(split_rounds) {
                    Some(pause) => {
                        split_rounds += 1;
                        let left = deadline.saturating_duration_since(std::time::Instant::now());
                        if !pause.is_zero() {
                            std::thread::sleep((*pause).min(left));
                        }
                        continue;
                    }
                    None => return Confirm::Split { has: has.into_iter().map(|(n, _)| n).collect(), not_yet },
                },
                // Receipts that differ: no reading this round.
                (false, _) => {}
            }
        }
        let now = std::time::Instant::now();
        if now >= deadline {
            return match ever_answered {
                true => Confirm::NotYet,
                false => Confirm::Unreachable(last.unwrap_or_else(|| "没有可问的端点".into())),
            };
        }
        if let Some(pause) = backoff.get(round.min(backoff.len().saturating_sub(1))) {
            std::thread::sleep((*pause).min(deadline - now));
        }
        round += 1;
    }
}

#[cfg(test)]
mod fee_tests {
    use super::*;

    fn history(tips: &[u64]) -> Value {
        Value::Obj(vec![(
            "reward".into(),
            Value::Arr(tips.iter().map(|t| Value::Arr(vec![Value::Str(format!("0x{t:x}"))])).collect()),
        )])
    }

    /// The priority fee is the median of the block medians (the lower middle for an even count), held at the
    /// ceiling; anything unread is the ceiling; the cap is base fee × 2 + priority fee; no base fee is the
    /// fallback pair.
    #[test]
    fn the_fee_rule_reads_the_chain_and_holds_its_ceiling() {
        assert_eq!(tip_of(&history(&[5, 1, 4, 2, 3])), Some(3));
        assert_eq!(tip_of(&history(&[4, 1, 3, 2])), Some(2));
        assert_eq!(tip_of(&history(&[])), None);
        assert_eq!(tip_of(&Value::Null), None);
        assert_eq!(tip_of(&Value::Obj(vec![("reward".into(), Value::Arr(vec![Value::Arr(vec![Value::Str("0xzz".into())])]))])), None);
        assert_eq!(Fees::of(Some(10), Some(3)), Fees { max_fee: 23, priority: 3, gas_limit: GAS_LIMIT, from_chain: true });
        assert_eq!(Fees::of(Some(10), Some(PRIORITY_FEE * 3)).priority, PRIORITY_FEE);
        assert_eq!(Fees::of(Some(10), None).priority, PRIORITY_FEE);
        assert_eq!(Fees::of(Some(0), Some(0)), Fees { max_fee: 0, priority: 0, gas_limit: GAS_LIMIT, from_chain: true });
        assert_eq!(Fees::of(Some(u64::MAX), Some(1)).max_fee, u64::MAX);
        assert_eq!(Fees::of(None, Some(1)), Fees::fallback());
        assert!(!Fees::fallback().from_chain);
        // A base fee of 1 gwei with the priority fee at its ceiling: the numbers of the fallback, from the chain.
        let even = Fees::of(Some(PRIORITY_FEE), Some(PRIORITY_FEE * 5));
        assert_eq!((even.max_fee, even.priority), (MAX_FEE, PRIORITY_FEE));
        assert!(even.from_chain);
    }

    /// Blocks that paid no priority fee are not counted, one line per form: zeros among paid blocks (left out, odd
    /// and even counts left), one paid block among zeros (its fee), every block zero (zero, as the chain says), one
    /// block zero, no block, a zero next to an unreadable entry (unread wins).
    #[test]
    fn blocks_that_paid_no_priority_fee_are_not_counted() {
        assert_eq!(tip_of(&history(&[0, 5, 0, 1, 3])), Some(3), "three paid blocks: their median");
        assert_eq!(tip_of(&history(&[0, 4, 0, 2])), Some(2), "two paid blocks: the lower middle");
        assert_eq!(tip_of(&history(&[0, 0, 0, 7, 0])), Some(7), "one paid block among empty ones");
        assert_eq!(tip_of(&history(&[0, 0, 0])), Some(0), "every block empty: zero");
        assert_eq!(tip_of(&history(&[0])), Some(0));
        assert_eq!(tip_of(&history(&[])), None);
        let half_read = Value::Obj(vec![("reward".into(), Value::Arr(vec![Value::Arr(vec![Value::Str("0x0".into())]), Value::Arr(vec![Value::Str("0xzz".into())])]))]);
        assert_eq!(tip_of(&half_read), None, "an unreadable entry is unread, zeros or not");
        assert_eq!(Fees::of(Some(10), tip_of(&history(&[0, 0, 0]))).priority, 0, "all zero: no priority fee");
    }

    /// The gas limit for an estimate: one and a half times it, rounded up, held at the ceiling; the fees carry
    /// it and the cap is worked from it.
    #[test]
    fn the_gas_limit_follows_the_estimate() {
        assert_eq!(limit_for(0), 0);
        assert_eq!(limit_for(1), 2);
        assert_eq!(limit_for(21_000), 31_500);
        assert_eq!(limit_for(25_001), 37_502);
        assert_eq!(limit_for(133_333), 200_000);
        assert_eq!(limit_for(133_334), 200_000);
        assert_eq!(limit_for(GAS_LIMIT), GAS_LIMIT);
        assert_eq!(limit_for(u64::MAX), GAS_LIMIT);
        let f = Fees::of(Some(10), Some(3)).with_estimate(30_000);
        assert_eq!((f.max_fee, f.priority, f.gas_limit), (23, 3, 45_000));
        assert_eq!(f.cap_wei(), 23 * 45_000);
        assert_eq!(Fees::fallback().gas_limit, GAS_LIMIT);
        assert_eq!(fee_cap_wei(), MAX_FEE as u128 * GAS_LIMIT as u128);
    }

    /// One reading of a block's base fee for both ends: leading zeros read, more than 64 bits unread.
    #[test]
    fn a_base_fee_is_read_one_way() {
        let b = |x: &str| Value::Obj(vec![("baseFeePerGas".into(), Value::Str(x.into()))]);
        assert_eq!(base_fee_of(&b("0x3b9aca00")), Some(1_000_000_000));
        assert_eq!(base_fee_of(&b("0x00000000000000000000000000000001")), Some(1));
        assert_eq!(base_fee_of(&b("0xffffffffffffffff")), Some(u64::MAX));
        assert_eq!(base_fee_of(&b("0x8000000000000000")), Some(1 << 63));
        for bad in ["0x10000000000000000", "0x", "1", "0xzz", "0x000000000000000000000000000000001"] {
            assert_eq!(base_fee_of(&b(bad)), None, "{bad}");
        }
        assert_eq!(base_fee_of(&Value::Obj(Vec::new())), None);
        assert_eq!(Fees::of(base_fee_of(&b("0x8000000000000000")), Some(1)).max_fee, u64::MAX, "doubled saturates");
    }
}
