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

/// Priority fee of an anchor (wei per gas).
pub const PRIORITY_FEE: u64 = 1_000_000_000;
/// Fee cap when the chain's base fee cannot be read (wei per gas; 3 gwei). A fallback only: the cap is
/// normally computed from the base fee.
pub const MAX_FEE: u64 = 3_000_000_000;
/// Gas limit of an anchor.
pub const GAS_LIMIT: u64 = 200_000;

/// This transaction's two fee fields, fee cap and priority fee, in wei per gas.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fees {
    pub max_fee: u64,
    pub priority: u64,
}

impl Fees {
    /// The fallback when the base fee cannot be read.
    pub fn fallback() -> Fees {
        Fees { max_fee: MAX_FEE, priority: PRIORITY_FEE }
    }

    /// Computed from the chain's base fee: cap = base fee × 2 + priority fee (the base fee grows at most one
    /// eighth per block, so double covers several blocks).
    pub fn from_base(base_fee: u64) -> Fees {
        Fees { max_fee: base_fee.saturating_mul(2).saturating_add(PRIORITY_FEE), priority: PRIORITY_FEE }
    }

    /// The most this transaction can spend in wei: fee cap times gas limit. The balance check before sending
    /// and the confirmation card read it; it is what the node checks at broadcast (`insufficient funds for
    /// gas * price + value`).
    pub fn cap_wei(self) -> u128 {
        self.max_fee as u128 * GAS_LIMIT as u128
    }
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
/// This joins [`broadcast`] and [`confirm`] (fees at the fallback). Callers that persist "submitted" between
/// the two stages use them directly, so after a restart they wait for the same transaction without resending.
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
    let hash = broadcast(ep, key, chain_id, form, registry, hashes, calldata_override, Fees::fallback())?;
    let confirm = confirm(ep, &hash, wait);
    Ok(Sent { tx: hash, form, confirm })
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
    let (to, data) = match form {
        Form::Registry => {
            let r = registry.ok_or(Trouble::Transport("登记形制要一个合约地址".into()))?;
            (r, calldata_override.unwrap_or_else(|| registry_calldata(hashes)))
        }
        Form::Bare => (from, calldata_override.unwrap_or_else(|| bare_calldata(hashes))),
    };
    let nonce = {
        let v = ep.call(
            "eth_getTransactionCount",
            // `pending`: while an earlier batch is still in the pool, the new one follows it and does not
            // reuse its nonce (reusing it would replace the earlier one or be refused as underpriced).
            &Value::Arr(vec![Value::Str(hexfmt::encode(&from)), Value::Str("pending".into())]),
        )?;
        tx::hex_qty(&v).ok_or(Trouble::Transport("nonce 读不出".into()))?
    };
    let unsigned = Unsigned {
        chain_id,
        nonce,
        max_priority_fee: fees.priority,
        max_fee: fees.max_fee,
        gas: GAS_LIMIT,
        to,
        value: 0,
        data,
    };
    unsigned.sign(key).ok_or(Trouble::Transport("签不出来".into()))
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
    confirm_each(&mut [ep], hash, wait)
}

/// Ask each endpoint for the same receipt. Each round asks them in turn; the first receipt wins. When every
/// endpoint fails in a round it is "out of sight" (with the last sentence); when one says "not yet" the wait
/// goes on until the deadline, then "not yet". Asking only the endpoint that took the broadcast would wait
/// forever once that endpoint went down.
pub fn confirm_each(eps: &mut [&mut dyn Endpoint], hash: &[u8; 32], wait: std::time::Duration) -> Confirm {
    let deadline = std::time::Instant::now() + wait;
    let q = Value::Arr(vec![Value::Str(hexfmt::encode(hash))]);
    loop {
        let mut last: Option<String> = None;
        let mut answered = false;
        for ep in eps.iter_mut() {
            match ep.call("eth_getTransactionReceipt", &q) {
                Ok(r) if !r.is_null() => match (tx::qty_u64(&r, "status"), tx::qty_u64(&r, "blockNumber")) {
                    (Some(status), Some(block_number)) => return Confirm::Included { status, block_number },
                    // This endpoint's receipt is incomplete: note the sentence and ask the next one.
                    _ => last = Some("收据里没有状态或块号".into()),
                },
                Ok(_) => answered = true,
                Err(e) => last = Some(format!("{e:?}")),
            }
        }
        if !answered {
            return Confirm::Unreachable(last.unwrap_or_else(|| "没有可问的端点".into()));
        }
        if std::time::Instant::now() >= deadline {
            return Confirm::NotYet;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}
