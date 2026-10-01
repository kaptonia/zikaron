//! Proof kits (law §9.7): a self-contained proof of one anchor, captured and verified offline.
//!
//! A kit reduces trust in the chain to two 32-byte block hashes the recipient trusts on their own; everything
//! else is computed from the kit: the transaction is pinned to the header's transaction root, the receipt to
//! the receipt root at the same index, the sender's two account proofs to the two headers' state roots.
//! Verification asks no node.
//!
//! A missing account proof means that boundary was not asked, so the verdict is `UNPROVEN` (§9.3, §9.7); any
//! other missing part proves nothing. Any altered part is refused by name.

use crate::mpt::{self, Answer};
use crate::rlp;
use crate::rpc::{Endpoint, Trouble};
use crate::tx;
use crate::wire::W;
use zikaron::cryptox::keccak256;
use zikaron::hexfmt;
use zikaron::json::Value;
use zikaron::tokens::Verdict;

/// Why a kit is refused: each part has its own name.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Refuse {
    /// A missing part (by name).
    Missing(&'static str),
    /// The header hash is not the one it claims.
    Header,
    /// The previous header is not this block's parent.
    ParentHeader,
    /// The transaction does not pin to the header's transaction root.
    TxProof,
    /// The receipt does not pin to the header's receipt root.
    ReceiptProof,
    /// Transaction and receipt are not at the same index.
    IndexMismatch,
    /// The transaction cannot be read, or its signature does not name the kit's chain.
    Tx,
    /// The receipt says the transaction failed.
    Status,
    /// Registry form: the log is malformed, or its emitter, topic 1 or topic 2 differs from the kit's claim.
    Log,
    /// The anchored word is not at an aligned offset of the calldata the sender signed.
    Containment,
    /// An account proof does not pin to the state root.
    AccountProof,
    /// Bare form: the recipient is not the sender, calldata is not a positive multiple of 32, or the claimed
    /// hash is not in it.
    BareForm,
}

impl Refuse {
    pub fn code(&self) -> &'static str {
        match self {
            Refuse::Missing(_) => "E_KIT_MISSING",
            Refuse::Header => "E_KIT_HEADER",
            Refuse::ParentHeader => "E_KIT_PARENT",
            Refuse::TxProof => "E_KIT_TX_PROOF",
            Refuse::ReceiptProof => "E_KIT_RECEIPT_PROOF",
            Refuse::IndexMismatch => "E_KIT_INDEX",
            Refuse::Tx => "E_KIT_TX",
            Refuse::Status => "E_KIT_STATUS",
            Refuse::Log => "E_KIT_LOG",
            Refuse::Containment => "E_KIT_CONTAINMENT",
            Refuse::AccountProof => "E_KIT_ACCOUNT_PROOF",
            Refuse::BareForm => "E_KIT_BARE_FORM",
        }
    }
    pub fn detail(&self) -> String {
        match self {
            Refuse::Missing(p) => format!("缺 {p}"),
            _ => String::new(),
        }
    }
}

/// The anchor a kit proves.
///
/// Emitter, form and block hash go out with the verdict: §9.7 puts the emitter in the kit so the recipient
/// can run the §9.1 question "did I declare this address" against their own basis, and the whole kit depends on
/// the recipient trusting the block hash independently.
pub struct Proven {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_timestamp: u64,
    pub block_hash: [u8; 32],
    pub tx: [u8; 32],
    pub sender: [u8; 20],
    pub hash: [u8; 32],
    pub verdict: Verdict,
    /// Registry form: the address that emitted the log (the recipient checks it against their `registries`);
    /// none for the bare form.
    pub emitter: Option<[u8; 20]>,
    pub registry_form: bool,
}

fn need<'a>(v: &'a W, k: &'static str) -> Result<&'a W, Refuse> {
    v.member(k).ok_or(Refuse::Missing(k))
}

fn bytes_of(v: &W, k: &'static str) -> Result<Vec<u8>, Refuse> {
    hexfmt::decode(need(v, k)?.as_str().ok_or(Refuse::Missing(k))?).ok_or(Refuse::Missing(k))
}

fn proof_of(v: &W, k: &'static str) -> Result<Vec<Vec<u8>>, Refuse> {
    let arr = need(v, k)?.as_arr().ok_or(Refuse::Missing(k))?;
    arr.iter()
        .map(|x| x.as_str().and_then(hexfmt::decode).ok_or(Refuse::Missing(k)))
        .collect()
}

fn u64_of(v: &W, k: &'static str) -> Result<u64, Refuse> {
    need(v, k)?.as_u64().ok_or(Refuse::Missing(k))
}

/// Header field positions (the fixed order of the RLP list).
struct Header {
    parent_hash: [u8; 32],
    state_root: [u8; 32],
    tx_root: [u8; 32],
    receipt_root: [u8; 32],
    number: u64,
    timestamp: u64,
    hash: [u8; 32],
}

fn read_header(raw: &[u8]) -> Option<Header> {
    let items = rlp::decode_all(raw)?.list()?.to_vec();
    if items.len() < 15 {
        return None;
    }
    let fixed = |i: usize| -> Option<[u8; 32]> {
        let b = items.get(i)?.bytes()?;
        if b.len() != 32 {
            return None;
        }
        let mut h = [0u8; 32];
        h.copy_from_slice(b);
        Some(h)
    };
    Some(Header {
        parent_hash: fixed(0)?,
        state_root: fixed(3)?,
        tx_root: fixed(4)?,
        receipt_root: fixed(5)?,
        number: items.get(8)?.u64()?,
        timestamp: items.get(11)?.u64()?,
        hash: keccak256(raw),
    })
}

/// What an account proof says at a boundary: with code, without code, or not asked.
fn codeless_at(state_root: &[u8; 32], who: &[u8; 20], proof: &[Vec<u8>]) -> Result<bool, Refuse> {
    let key = keccak256(who);
    match mpt::verify(state_root, &key, proof) {
        None => Err(Refuse::AccountProof),
        // The address is not in the state trie: it has no code (§9.3 says so).
        Some(Answer::Absent) => Ok(true),
        Some(Answer::Value(account)) => {
            let items = rlp::decode_all(&account).and_then(|x| x.list().map(<[rlp::Item]>::to_vec)).ok_or(Refuse::AccountProof)?;
            let code_hash = items.get(3).and_then(|x| x.bytes()).ok_or(Refuse::AccountProof)?;
            Ok(code_hash == keccak256(&[]))
        }
    }
}

/// Verify a kit offline. No node is asked.
pub fn verify(kit: &W) -> Result<Proven, Refuse> {
    crate::seam();
    let chain_id = u64_of(kit, "chainId")?;
    let claimed = bytes_of(kit, "hash")?;
    if claimed.len() != 32 {
        return Err(Refuse::Missing("hash"));
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(&claimed);
    let form = need(kit, "form")?.as_str().ok_or(Refuse::Missing("form"))?.to_string();

    // 1. The header, and the block hash the recipient trusts independently.
    let header_raw = bytes_of(kit, "header")?;
    let header = read_header(&header_raw).ok_or(Refuse::Header)?;
    let block_hash = bytes_of(kit, "blockHash")?;
    if block_hash != header.hash {
        return Err(Refuse::Header);
    }

    // 2. The transaction and its position under the transaction root.
    let index = u64_of(kit, "txIndex")?;
    let key = rlp::quantity(index);
    let tx_raw = bytes_of(kit, "tx")?;
    match mpt::verify(&header.tx_root, &key, &proof_of(kit, "txProof")?) {
        Some(Answer::Value(v)) if v == tx_raw => {}
        _ => return Err(Refuse::TxProof),
    }
    let t = tx::from_raw(&tx_raw).ok_or(Refuse::Tx)?;
    if t.chain_id != Some(chain_id) {
        return Err(Refuse::Tx);
    }
    let sender = t.sender.ok_or(Refuse::Tx)?;

    // 3. The receipt must be at the transaction's index, so the status read is this transaction's.
    let receipt_raw = bytes_of(kit, "receipt")?;
    match mpt::verify(&header.receipt_root, &key, &proof_of(kit, "receiptProof")?) {
        Some(Answer::Value(v)) if v == receipt_raw => {}
        _ => return Err(Refuse::ReceiptProof),
    }
    let (status, logs) = read_receipt(&receipt_raw).ok_or(Refuse::ReceiptProof)?;
    if status != 1 {
        return Err(Refuse::Status);
    }

    // 4. The checks of each form.
    match form.as_str() {
        "registry" => {
            let emitter = bytes_of(kit, "emitter")?;
            let mut found = false;
            for (addr, topics, data) in &logs {
                if addr != &emitter || topics.len() != 3 || !data.is_empty() {
                    continue;
                }
                // Topics on chain are always 32 bytes, and this receipt comes from the kit: check length
                // before taking bytes.
                if topics.iter().any(|t| t.len() != 32) {
                    continue;
                }
                if topics[0] != crate::scan::topic0().to_vec() {
                    continue;
                }
                if topics[1][..12].iter().any(|x| *x != 0) || topics[1][12..] != sender[..] {
                    continue;
                }
                if topics[2] != hash.to_vec() {
                    continue;
                }
                found = true;
            }
            if !found {
                return Err(Refuse::Log);
            }
            if t.to.is_none() || !crate::scan::calldata_carries(&t.input, &hash) {
                return Err(Refuse::Containment);
            }
        }
        _ => {
            if t.to != Some(sender) || t.input.is_empty() || t.input.len() % 32 != 0 {
                return Err(Refuse::BareForm);
            }
            if !t.input.chunks(32).any(|w| w == hash) {
                return Err(Refuse::BareForm);
            }
        }
    }

    // 5. The two §9.3 boundaries. A missing account proof means that boundary was not asked and the verdict
    // is `UNPROVEN`; any other missing part (the previous header included) proves nothing.
    let anchor_proof = match kit.member("accountProofAnchor") {
        None => None,
        Some(_) => Some(proof_of(kit, "accountProofAnchor")?),
    };
    let here = match &anchor_proof {
        None => None,
        Some(p) => Some(codeless_at(&header.state_root, &sender, p)?),
    };
    let verdict = if header.number == 0 {
        // Before block 0 is the pre-genesis state with no addresses, so that boundary needs no query.
        match here {
            None => Verdict::Unproven,
            Some(true) => Verdict::Counted,
            Some(false) => Verdict::Void,
        }
    } else {
        // The previous header is required except at block 0.
        let parent_raw = bytes_of(kit, "parentHeader")?;
        let parent = read_header(&parent_raw).ok_or(Refuse::ParentHeader)?;
        if parent.hash != header.parent_hash {
            return Err(Refuse::ParentHeader);
        }
        let before = match kit.member("accountProofParent") {
            None => None,
            Some(_) => Some(codeless_at(&parent.state_root, &sender, &proof_of(kit, "accountProofParent")?)?),
        };
        // The §9.3 order: not asked outranks has code. VOID needs both boundaries asked and one with code;
        // one unasked boundary makes it UNPROVEN even if the other shows code. A kit missing an account proof
        // proves UNPROVEN, not VOID.
        match (here, before) {
            (Some(a), Some(b)) => {
                if a && b {
                    Verdict::Counted
                } else {
                    Verdict::Void
                }
            }
            _ => Verdict::Unproven,
        }
    };

    let emitter = match form.as_str() {
        "registry" => {
            let e = bytes_of(kit, "emitter")?;
            let mut a = [0u8; 20];
            if e.len() != 20 {
                return Err(Refuse::Missing("emitter"));
            }
            a.copy_from_slice(&e);
            Some(a)
        }
        _ => None,
    };
    Ok(Proven {
        chain_id,
        block_number: header.number,
        block_timestamp: header.timestamp,
        block_hash: header.hash,
        tx: t.hash,
        sender,
        hash,
        verdict,
        emitter,
        registry_form: form == "registry",
    })
}

/// Receipt bytes to (status, logs).
fn read_receipt(raw: &[u8]) -> Option<(u64, Vec<(Vec<u8>, Vec<Vec<u8>>, Vec<u8>)>)> {
    let body = if raw.first().copied()? >= 0xc0 { raw } else { &raw[1..] };
    let items = rlp::decode_all(body)?.list()?.to_vec();
    if items.len() != 4 {
        return None;
    }
    let status = items[0].u64()?;
    let mut logs = Vec::new();
    for l in items[3].list()? {
        let parts = l.list()?;
        let addr = parts.first()?.bytes()?.to_vec();
        let topics: Vec<Vec<u8>> = parts.get(1)?.list()?.iter().map(|t| t.bytes().map(<[u8]>::to_vec)).collect::<Option<_>>()?;
        let data = parts.get(2)?.bytes()?.to_vec();
        logs.push((addr, topics, data));
    }
    Some((status, logs))
}

/// Capture a kit: ask the node for all transactions and receipts of the block, build both tries, take this
/// transaction's paths.
///
/// The built roots must match the header's roots; otherwise the block at hand is not the one it claims and
/// capture stops.
pub fn capture(
    ep: &mut dyn Endpoint,
    chain_id: u64,
    tx_hash: &[u8; 32],
    claimed: &[u8; 32],
    emitter: Option<[u8; 20]>,
) -> Result<Value, Trouble> {
    crate::seam();
    let txv = ep.call("eth_getTransactionByHash", &Value::Arr(vec![Value::Str(hexfmt::encode(tx_hash))]))?;
    let t = tx::read(&txv).map_err(|e| Trouble::Transport(format!("交易读不成:{e:?}")))?;
    let bn = t.block_number.ok_or(Trouble::Transport("这一笔还没入块".into()))?;
    let block = ep.call(
        "eth_getBlockByNumber",
        &Value::Arr(vec![Value::Str(crate::scan::hex_quantity(bn)), Value::Bool(true)]),
    )?;
    let header_raw = header_rlp(&block).ok_or(Trouble::Transport("区块头重编不出来".into()))?;
    let header = read_header(&header_raw).ok_or(Trouble::Transport("重编出来的头读不回".into()))?;
    let block_hash = block.member("hash").and_then(|x| x.as_str()).and_then(hexfmt::decode).unwrap_or_default();
    if header.hash.to_vec() != block_hash {
        return Err(Trouble::Transport("重编出来的头不是节点给的那一枚哈希".into()));
    }

    // All transactions and receipts of the block, by index.
    let txs = block.member("transactions").and_then(|x| x.as_arr()).ok_or(Trouble::Transport("区块没有交易表".into()))?;
    let mut raws = Vec::new();
    let mut receipts = Vec::new();
    let mut index = None;
    for (i, one) in txs.iter().enumerate() {
        let parsed = tx::read(one).map_err(|e| Trouble::Transport(format!("块内交易读不成:{e:?}")))?;
        if &parsed.hash == tx_hash {
            index = Some(i as u64);
        }
        raws.push(parsed.raw);
        let r = ep.call("eth_getTransactionReceipt", &Value::Arr(vec![Value::Str(hexfmt::encode(&parsed.hash))]))?;
        receipts.push(receipt_rlp(&r).ok_or(Trouble::Transport("收据重编不出来".into()))?);
    }
    let index = index.ok_or(Trouble::Transport("这一笔不在它自称的那一块里".into()))?;
    let tx_trie = mpt::Trie::indexed(&raws);
    if tx_trie.root() != header.tx_root {
        return Err(Trouble::Transport("自建的交易根与头里的对不上".into()));
    }
    let receipt_trie = mpt::Trie::indexed(&receipts);
    if receipt_trie.root() != header.receipt_root {
        return Err(Trouble::Transport("自建的收据根与头里的对不上".into()));
    }
    let key = rlp::quantity(index);
    let sender = t.sender.ok_or(Trouble::Transport("这一笔在这套语法里没有发送者".into()))?;

    let account_here = account_proof(ep, &sender, bn)?;
    let mut fields = vec![
        ("accountProofAnchor".to_string(), Value::Arr(account_here.iter().map(|n| Value::Str(hexfmt::encode(n))).collect())),
        ("blockHash".to_string(), Value::Str(hexfmt::encode(&header.hash))),
        ("chainId".to_string(), Value::Int(chain_id)),
        ("form".to_string(), Value::Str(if emitter.is_some() { "registry".into() } else { "bare".into() })),
        ("hash".to_string(), Value::Str(hexfmt::encode(claimed))),
        ("header".to_string(), Value::Str(hexfmt::encode(&header_raw))),
        ("receipt".to_string(), Value::Str(hexfmt::encode(&receipts[index as usize]))),
        ("receiptProof".to_string(), Value::Arr(receipt_trie.proof(&key).iter().map(|n| Value::Str(hexfmt::encode(n))).collect())),
        ("tx".to_string(), Value::Str(hexfmt::encode(&raws[index as usize]))),
        ("txIndex".to_string(), Value::Int(index)),
        ("txProof".to_string(), Value::Arr(tx_trie.proof(&key).iter().map(|n| Value::Str(hexfmt::encode(n))).collect())),
    ];
    if let Some(e) = emitter {
        fields.push(("emitter".to_string(), Value::Str(hexfmt::encode(&e))));
    }
    if bn > 0 {
        let parent_block = ep.call(
            "eth_getBlockByNumber",
            &Value::Arr(vec![Value::Str(crate::scan::hex_quantity(bn - 1)), Value::Bool(false)]),
        )?;
        let parent_raw = header_rlp(&parent_block).ok_or(Trouble::Transport("上一块的头重编不出来".into()))?;
        fields.push(("parentHeader".to_string(), Value::Str(hexfmt::encode(&parent_raw))));
        let before = account_proof(ep, &sender, bn - 1)?;
        fields.push((
            "accountProofParent".to_string(),
            Value::Arr(before.iter().map(|n| Value::Str(hexfmt::encode(n))).collect()),
        ));
    }
    fields.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(Value::Obj(fields))
}

fn account_proof(ep: &mut dyn Endpoint, who: &[u8; 20], block: u64) -> Result<Vec<Vec<u8>>, Trouble> {
    let v = ep.call(
        "eth_getProof",
        &Value::Arr(vec![
            Value::Str(hexfmt::encode(who)),
            Value::Arr(vec![]),
            Value::Str(crate::scan::hex_quantity(block)),
        ]),
    )?;
    let arr = v.member("accountProof").and_then(|x| x.as_arr()).ok_or(Trouble::Transport("eth_getProof 没给账户证明".into()))?;
    arr.iter()
        .map(|n| n.as_str().and_then(hexfmt::decode).ok_or(Trouble::Transport("账户证明里有不是字节串的东西".into())))
        .collect()
}

/// The header re-encoded as RLP: nodes do not serve raw headers, and a re-encoding that hashes to the block
/// hash is the header.
fn header_rlp(block: &W) -> Option<Vec<u8>> {
    let hexf = |k: &str| -> Option<Vec<u8>> { block.member(k)?.as_str().and_then(hexfmt::decode) };
    let qty = |k: &str| -> Option<Vec<u8>> { tx::qty(block, k) };
    let mut fields = vec![
        rlp::bytes(&hexf("parentHash")?),
        rlp::bytes(&hexf("sha3Uncles")?),
        rlp::bytes(&hexf("miner")?),
        rlp::bytes(&hexf("stateRoot")?),
        rlp::bytes(&hexf("transactionsRoot")?),
        rlp::bytes(&hexf("receiptsRoot")?),
        rlp::bytes(&hexf("logsBloom")?),
        rlp::scalar(&qty("difficulty").unwrap_or_default()),
        rlp::scalar(&qty("number")?),
        rlp::scalar(&qty("gasLimit")?),
        rlp::scalar(&qty("gasUsed")?),
        rlp::scalar(&qty("timestamp")?),
        rlp::bytes(&hexf("extraData")?),
        rlp::bytes(&hexf("mixHash")?),
        rlp::bytes(&hexf("nonce")?),
    ];
    // Fork-added trailing fields: included when present, in the order of the upstream specification.
    for k in ["baseFeePerGas"] {
        if let Some(v) = qty(k) {
            fields.push(rlp::scalar(&v));
        }
    }
    for k in ["withdrawalsRoot"] {
        if let Some(v) = hexf(k) {
            fields.push(rlp::bytes(&v));
        }
    }
    for k in ["blobGasUsed", "excessBlobGas"] {
        if let Some(v) = qty(k) {
            fields.push(rlp::scalar(&v));
        }
    }
    for k in ["parentBeaconBlockRoot", "requestsHash"] {
        if let Some(v) = hexf(k) {
            fields.push(rlp::bytes(&v));
        }
    }
    Some(rlp::list(&fields))
}

/// The receipt re-encoded as RLP: `type || rlp([status, cumulativeGasUsed, logsBloom, logs])`.
fn receipt_rlp(r: &W) -> Option<Vec<u8>> {
    let status = tx::qty_u64(r, "status")?;
    let cumulative = tx::qty(r, "cumulativeGasUsed")?;
    let bloom = r.member("logsBloom")?.as_str().and_then(hexfmt::decode)?;
    let mut logs = Vec::new();
    for l in r.member("logs")?.as_arr()? {
        let addr = l.member("address")?.as_str().and_then(hexfmt::decode)?;
        let topics: Vec<Vec<u8>> = l
            .member("topics")?
            .as_arr()?
            .iter()
            .map(|t| t.as_str().and_then(hexfmt::decode).map(|b| rlp::bytes(&b)))
            .collect::<Option<_>>()?;
        let data = l.member("data")?.as_str().and_then(hexfmt::decode)?;
        logs.push(rlp::list(&[rlp::bytes(&addr), rlp::list(&topics), rlp::bytes(&data)]));
    }
    let body = rlp::list(&[
        rlp::scalar(&status.to_be_bytes()),
        rlp::scalar(&cumulative),
        rlp::bytes(&bloom),
        rlp::list(&logs),
    ]);
    let ty = tx::qty_u64(r, "type").unwrap_or(0);
    if ty == 0 {
        return Some(body);
    }
    let mut out = vec![ty as u8];
    out.extend_from_slice(&body);
    Some(out)
}
