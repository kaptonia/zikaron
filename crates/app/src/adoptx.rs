//! Adoption: recording anchors made outside this ledger. Each row is verified against the chain, and
//! co-signature verification cannot be bypassed.
//!
//! An anchor row must pass three checks: the transaction exists, it was sent by a declared sender, and its
//! calldata carries the promised content. The first two are answered by the transaction on chain
//! (`eth_getTransactionByHash`), the third by the anchoring crate's `calldata_carries`, which owns the offset
//! rule. Each failed check has its own name rather than one "this row fails".
//!
//! An external co-signature takes three steps: fix this entry's `prev`, produce the preimage binding adopter,
//! anchors and `prev`, and paste the signature back. Only a signature that verifies locally (the core's
//! `verify_signature`) may go into the entry; there is no "write first, check later" path. Because the
//! preimage includes `prev`, any new head voids the previous signature.
//!
//! A failed co-signature does not refuse the entry: the entry is still valid and those anchors are only
//! unproven. So co-signing and assembly are separate: assembly ignores the co-signature, which only decides
//! whether the body carries the `attestor` / `attestation` fields.

use crate::fault::{Fault, Known};
use crate::key::Address;
use zikaron::json::Value;

/// One anchor row (the four members of an `anchors` element).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorRow {
    pub chain_id: u64,
    pub tx: String,
    pub payload_kind: String,
    pub content: String,
}

/// Parses row-by-row input: four whitespace-separated fields per line. A malformed line is refused by name,
/// never skipped.
pub fn rows_of(text: &str) -> Result<Vec<AnchorRow>, Fault> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let f: Vec<&str> = t.split_whitespace().collect();
        if f.len() != 4 {
            return Err(Fault::known(
                Known::SettingsShape,
                crate::lang::filln(crate::lang::Key::Tail072, &[&(i + 1).to_string(), &(f.len()).to_string()]),
            ));
        }
        let chain_id: u64 = f[0].parse().map_err(|_| {
            Fault::known(Known::SettingsShape, crate::lang::filln(crate::lang::Key::Tail073, &[&(i + 1).to_string()]))
        })?;
        out.push(AnchorRow {
            chain_id,
            tx: f[1].to_string(),
            payload_kind: f[2].to_string(),
            content: f[3].to_string(),
        });
    }
    if out.is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::t(crate::lang::Key::Tail074).to_string()));
    }
    Ok(out)
}

/// Lays rows out as the `anchors` table (non-empty, four members per element). Validating the format is the
/// core's job.
pub fn anchors_value(rows: &[AnchorRow]) -> Value {
    Value::Arr(
        rows.iter()
            .map(|r| {
                Value::Obj(vec![
                    ("chainId".to_string(), Value::Int(r.chain_id)),
                    ("content".to_string(), Value::Str(r.content.clone())),
                    ("payloadKind".to_string(), Value::Str(r.payload_kind.clone())),
                    ("tx".to_string(), Value::Str(r.tx.clone())),
                ])
            })
            .collect(),
    )
}

/// One row's check result. Each of the four states has its own name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Proof {
    /// The chain has no such transaction.
    NoSuchTx,
    /// The transaction exists, but the sender is not one of those declared.
    WrongSender(String),
    /// The transaction exists and the sender matches, but the calldata lacks the promised 32 bytes (per the
    /// anchoring crate's offset rule).
    NoContent(String),
    /// All three checks pass.
    Proven(String),
    /// This ledger declares no senders, so the sender check was never made.
    ///
    /// This is not "passed": with no open home, or no lineage readable from the bytes, `senders` is empty, and
    /// skipping the check would mark as proven any transaction from anyone that carries those 32 bytes in its
    /// calldata. Missing evidence gets its own name.
    NoSenders,
}

impl Proof {
    pub fn as_str(&self) -> &'static str {
        match self {
            Proof::NoSuchTx => "no_such_tx",
            Proof::WrongSender(_) => "wrong_sender",
            Proof::NoContent(_) => "no_content",
            Proof::Proven(_) => "proven",
            Proof::NoSenders => "no_senders",
        }
    }

    pub fn ok(&self) -> bool {
        matches!(self, Proof::Proven(_))
    }
}

fn member<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).and_then(|(_, x)| match x {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        }),
        _ => None,
    }
}

/// Checks one row. The chain answers about the transaction and the anchoring crate about the offset rule; this
/// layer decides nothing itself.
///
/// `senders` are the declared senders (this ledger's lineage); if empty, the sender check is skipped and the
/// result says "no senders declared", not "passed".
pub fn verify_row(
    eps: &[crate::chainx::Endpoint],
    row: &AnchorRow,
    senders: &[String],
) -> Result<Proof, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::W10);
    let mine: Vec<crate::chainx::Endpoint> =
        eps.iter().filter(|e| e.chain == row.chain_id).cloned().collect();
    if mine.is_empty() {
        return Err(Fault::known(
            Known::NoEndpoint,
            crate::lang::filln(crate::lang::Key::Tail031, &[&(row.chain_id).to_string()]),
        ));
    }
    let params = Value::Arr(vec![Value::Str(row.tx.clone())]);
    // Agreement is required only over the transaction facts (`chainx::TX_FACTS`): endpoints may add members
    // of their own, but a differing sender or calldata still counts as disagreement.
    let r = crate::chainx::ask(&mine, "eth_getTransactionByHash", &params)?;
    if matches!(r.value, Value::Null) {
        return Ok(Proof::NoSuchTx);
    }
    let Some(from) = member(&r.value, "from") else {
        return Err(Fault::known(Known::ChainShape, crate::lang::t(crate::lang::Key::Tail075).to_string()));
    };
    let from = from.to_ascii_lowercase();
    if !senders.is_empty() && !senders.iter().any(|s| s.eq_ignore_ascii_case(&from)) {
        return Ok(Proof::WrongSender(from));
    }
    let Some(input) = member(&r.value, "input") else {
        return Err(Fault::known(Known::ChainShape, crate::lang::t(crate::lang::Key::Tail076).to_string()));
    };
    let calldata = zikaron::hexfmt::decode(input)
        .ok_or_else(|| Fault::known(Known::ChainShape, crate::lang::t(crate::lang::Key::Tail077).to_string()))?;
    let want = zikaron::hexfmt::decode(&row.content)
        .ok_or_else(|| Fault::known(Known::ContentShape, row.content.clone()))?;
    if want.len() != 32 {
        return Err(Fault::known(Known::ContentShape, row.content.clone()));
    }
    let mut word = [0u8; 32];
    word.copy_from_slice(&want);
    // The offset rule lives in the anchoring crate; no copy here.
    if !zikaron_anchor::scan::calldata_carries(&calldata, &word) {
        return Ok(Proof::NoContent(from));
    }
    // The missing-evidence verdict comes last, so answerable checks are answered first: a transaction without
    // the promised content should fail the content check, not be reported as "no senders declared".
    if senders.is_empty() {
        return Ok(Proof::NoSenders);
    }
    Ok(Proof::Proven(from))
}

/// Co-signing steps one and two: the preimage (adopter, anchors, `prev`) given to the counterpart to sign.
///
/// The preimage includes `prev`, so once this entry's head changes the preimage changes: "a new head requires
/// a new signature" follows from the bytes, not from a check.
pub fn preimage(author: &Address, rows: &[AnchorRow], prev: &str) -> Vec<u8> {
    zikaron::entry::adoption_preimage(&author.hex(), &anchors_value(rows), prev)
}

/// Co-signing step three: local verification. Passes only when the recovered address equals `attestor`.
///
/// Judged by the core's `verify_signature` and the adoption domain's digest; this layer computes nothing
/// itself.
pub fn cosigned(
    author: &Address,
    rows: &[AnchorRow],
    prev: &str,
    attestor: &str,
    attestation: &str,
) -> Result<(), Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::W10);
    let (_, digest) = zikaron::entry::presig_and_digest(
        &preimage(author, rows, prev),
        zikaron::tokens::Domain::Adoption.as_str(),
    );
    zikaron::entry::verify_signature(attestation.trim(), &digest, attestor.trim())
        .map_err(|t| Fault::known(Known::CosignRefused, format!("{t:?}")))
}

/// The body of `adoption`. `attestor` and `attestation` are both present or both absent; when the co-signature
/// fails neither is carried, and the entry is still valid.
pub fn adoption_body(rows: &[AnchorRow], cosign: Option<(&str, &str)>) -> Value {
    let mut body = vec![("anchors".to_string(), anchors_value(rows))];
    if let Some((attestor, attestation)) = cosign {
        body.push(("attestation".to_string(), Value::Str(attestation.trim().to_string())));
        body.push(("attestor".to_string(), Value::Str(attestor.trim().to_string())));
    }
    body.sort_by(|a, b| a.0.cmp(&b.0));
    Value::Obj(body)
}

// ───────────────────────── The import-existing-anchors table ─────────────────────────

/// The prefix of the text the counterpart signs, followed by the preimage's hex.
pub const CLAIM_PREFIX: &str = "zikaron/1-adoption:";

/// The text sent to the counterpart: `zikaron/1-adoption:` plus the preimage's hex. The preimage includes
/// `prev`, so this text changes as soon as the ledger grows (a new head requires a new signature).
pub fn claim_text(author: &Address, rows: &[AnchorRow], prev: &str) -> String {
    format!("{CLAIM_PREFIX}{}", zikaron::hexfmt::encode(&preimage(author, rows, prev)))
}

/// A parsed claim text: who is claiming, which anchors, their ledger head; `preimage` is the bytes to sign.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim {
    pub adopter: String,
    pub rows: Vec<AnchorRow>,
    pub prev: String,
    pub preimage: Vec<u8>,
}

/// Parses a claim text. A wrong prefix, bad hex, unreadable JSON, a missing member, or a preimage that differs
/// when recomputed from its three members (not canonical form) are all refused as `CLAIM_SHAPE`, never guessed.
pub fn claim_of(text: &str) -> Result<Claim, Fault> {
    let bad = |why: &str| Fault::known(Known::ClaimShape, why.to_string());
    let t = text.trim();
    let hex = t.strip_prefix(CLAIM_PREFIX).ok_or_else(|| bad("prefix"))?;
    let bytes = zikaron::hexfmt::decode(hex).ok_or_else(|| bad("hex"))?;
    let v = zikaron::json::parse(&bytes).map_err(|_| bad("json"))?;
    let adopter = match v.member("adopter") {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(bad("adopter")),
    };
    let prev = match v.member("prev") {
        Some(Value::Str(s)) => s.clone(),
        _ => return Err(bad("prev")),
    };
    let Some(Value::Arr(list)) = v.member("anchors") else { return Err(bad("anchors")) };
    let mut rows = Vec::with_capacity(list.len());
    for a in list {
        let s = |k: &str| match a.member(k) {
            Some(Value::Str(x)) => Some(x.clone()),
            _ => None,
        };
        let chain_id = match a.member("chainId") {
            Some(Value::Int(n)) => *n,
            _ => return Err(bad("chainId")),
        };
        let (Some(tx), Some(payload_kind), Some(content)) = (s("tx"), s("payloadKind"), s("content")) else { return Err(bad("anchor")) };
        rows.push(AnchorRow { chain_id, tx, payload_kind, content });
    }
    if rows.is_empty() {
        return Err(bad("anchors"));
    }
    // Every member the claimer's adoption entry will carry must use the canonical spelling (a 20-byte address,
    // 32-byte head and anchor ids, lowercase with `0x`): a signature over any other spelling never verifies on
    // the claimer's side, so such a text is refused before anything is signed.
    if !zikaron::hexfmt::is_hex20(&adopter) {
        return Err(bad("adopter"));
    }
    if !zikaron::hexfmt::is_hex32(&prev) {
        return Err(bad("prev"));
    }
    if rows.iter().any(|r| !zikaron::hexfmt::is_hex32(&r.tx) || !zikaron::hexfmt::is_hex32(&r.content)) {
        return Err(bad("anchor"));
    }
    let again = zikaron::entry::adoption_preimage(&adopter, &anchors_value(&rows), &prev);
    if again != bytes {
        return Err(bad("canon"));
    }
    Ok(Claim { adopter, rows, prev, preimage: bytes })
}

/// A table row's state (a closed set of five), decided only here. A row in the ledger is "already in the
/// ledger" (the chain is not asked again); otherwise it follows the three checks. `NoSenders` reads as a
/// sender mismatch: the sender check was not made, so it does not pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    Passed,
    NoSuchTx,
    WrongSender,
    WrongDigest,
    InLedger,
}

pub fn row_state(in_ledger: bool, proof: Option<&Proof>) -> Option<RowState> {
    if in_ledger {
        return Some(RowState::InLedger);
    }
    proof.map(|p| match p {
        Proof::Proven(_) => RowState::Passed,
        Proof::NoSuchTx => RowState::NoSuchTx,
        Proof::WrongSender(_) | Proof::NoSenders => RowState::WrongSender,
        Proof::NoContent(_) => RowState::WrongDigest,
    })
}

/// One anchor a key sent on chain (one table row). `proof` is the row's three-check result (empty for rows
/// already in the ledger, which are not checked).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyAnchor {
    pub row: AnchorRow,
    pub block: u64,
    pub time: u64,
    pub in_ledger: bool,
    pub proof: Option<Proof>,
}

impl KeyAnchor {
    pub fn state(&self) -> Option<RowState> {
        row_state(self.in_ledger, self.proof.as_ref())
    }
}

/// A table row as a line of input text (the shape [`rows_of`] parses: chain id, transaction, payload kind,
/// content).
pub fn line_of(row: &AnchorRow) -> String {
    format!("{} {} {} {}", row.chain_id, row.tx, row.payload_kind, row.content)
}

/// The anchors `sender` sent, from a scan fragment (`fragment`'s `anchors`, in the anchoring crate's shape),
/// each marked whether it is in the ledger (`ledger_ids`). The payload kind is the registry contract's
/// (`registry`).
pub fn anchors_in(fragment: &Value, sender: &str, ledger_ids: &[String]) -> Vec<KeyAnchor> {
    let Some(Value::Arr(list)) = fragment.member("anchors") else { return Vec::new() };
    list.iter()
        .filter_map(|a| {
            let s = |k: &str| match a.member(k) {
                Some(Value::Str(x)) => Some(x.clone()),
                _ => None,
            };
            let n = |k: &str| match a.member(k) {
                Some(Value::Int(x)) => Some(*x),
                _ => None,
            };
            let from = s("sender")?;
            if !from.eq_ignore_ascii_case(sender) {
                return None;
            }
            let hash = s("hash")?;
            let in_ledger = ledger_ids.iter().any(|x| x.eq_ignore_ascii_case(&hash));
            Some(KeyAnchor {
                row: AnchorRow { chain_id: n("chainId")?, tx: s("tx")?, payload_kind: "registry".to_string(), content: hash },
                block: n("blockNumber").unwrap_or(0),
                time: n("blockTimestamp").unwrap_or(0),
                in_ledger,
                proof: None,
            })
        })
        .collect()
}

/// The block an anchor's transaction landed in (asks the chain; `None` when unanswered, never zero).
pub fn block_of(eps: &[crate::chainx::Endpoint], row: &AnchorRow) -> Option<u64> {
    let mine: Vec<crate::chainx::Endpoint> = eps.iter().filter(|e| e.chain == row.chain_id).cloned().collect();
    if mine.is_empty() {
        return None;
    }
    let r = crate::chainx::ask(&mine, "eth_getTransactionByHash", &Value::Arr(vec![Value::Str(row.tx.clone())])).ok()?;
    let b = member(&r.value, "blockNumber")?;
    u64::from_str_radix(b.trim_start_matches("0x"), 16).ok()
}
