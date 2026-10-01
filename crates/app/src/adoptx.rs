//! Adoption desk. Each row is verified against basis evidence, and co-signature verification cannot be
//! bypassed.
//!
//! ─── The chain answers three questions; this layer does not guess ───
//!
//! An anchor row must pass three questions (law §9.5): does the transaction exist, who sent it, does the
//! calldata carry the promised content. The first two are answered by the transaction on chain
//! (`eth_getTransactionByHash`), the third by the anchoring crate's `calldata_carries` (§9.5's offset rule
//! lives there; this layer writes no copy). Each question has its own name: one sentence "this row fails"
//! covering three things is a silent failure.
//!
//! ─── Co-signature verification cannot be bypassed ───
//!
//! An external key co-signature takes three steps: fix this entry's `prev`, produce the message binding
//! adopter, anchors and prev (law §6.6's three-member preimage), paste the signature back, and only when it
//! verifies locally may the entry be assembled. Verification rests on the core's `verify_signature`; this
//! layer does not weaken it and offers no "write first, check later" path.
//!
//! A new head requires a new signature: the preimage includes `prev`, so once the ledger grows the previous
//! signature is void at once. That is not a check in this layer; the preimage itself changed.
//!
//! ─── A failed co-signature does not refuse the entry (law §6.6) ───
//!
//! When the co-signature fails, the entry is still a legal entry; those elements are only unproven. So this
//! layer keeps co-signing and assembly apart: assembly does not ask about the co-signature, and the
//! co-signature decides only whether the body carries the `attestor` / `attestation` cells.

use crate::fault::{Fault, Known};
use crate::key::Address;
use zikaron::json::Value;

/// One anchor row of the table (the four members of an `anchors` element, law §6.5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorRow {
    pub chain_id: u64,
    pub tx: String,
    pub payload_kind: String,
    pub content: String,
}

/// Row-by-row entry: four cells per line, separated by whitespace. Unrecognized is refused by name, never
/// quietly skipping the line.
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

/// Lay out as law §6.5's `anchors` (a non-empty table, four members per element). Judging the format belongs
/// to the core's thirteen steps.
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
    /// The transaction exists and the sender matches, but the calldata lacks the promised thirty-two bytes
    /// (§9.5's offset rule).
    NoContent(String),
    /// All three questions pass.
    Proven(String),
    /// This ledger declares no senders, so the sender question was never asked.
    ///
    /// It is not "passed": with no open home, or no lineage readable from those bytes, `senders` is empty,
    /// and skipping the question on an empty table would go straight to `Proven`, painting green any
    /// transaction from anyone that happens to carry those thirty-two bytes in its calldata. Missing evidence
    /// needs its own name and may not borrow green.
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

/// Check one row. The chain answers about the transaction, the anchoring crate about the offset rule; this
/// layer decides nothing.
///
/// `senders` are the declared senders (this ledger's lineage); an empty table means the sender question is
/// not asked, and the face then says plainly "no senders declared", not "passed".
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
    // §9.5's offset rule lives in the anchoring crate; this layer writes no copy.
    if !zikaron_anchor::scan::calldata_carries(&calldata, &word) {
        return Ok(Proof::NoContent(from));
    }
    // The missing-evidence verdict comes last. Questions that can be answered are answered first: a
    // transaction that carries no promised content should be red on the content question, not turn the whole
    // row gray because this ledger declares no senders. Only here does an empty table become its own verdict:
    // one of the three questions was never asked, so this row is not "passed".
    if senders.is_empty() {
        return Ok(Proof::NoSenders);
    }
    Ok(Proof::Proven(from))
}

/// Steps one and two of co-signing: the preimage (law §6.6's three members), the part given to the
/// counterpart to sign.
///
/// The preimage includes `prev`, so once this entry's head changes the preimage changes: "a new head requires
/// a new signature" is not a check but a different preimage.
pub fn preimage(author: &Address, rows: &[AnchorRow], prev: &str) -> Vec<u8> {
    zikaron::entry::adoption_preimage(&author.hex(), &anchors_value(rows), prev)
}

/// Step three of co-signing: local verification. It passes only when the recovered address equals `attestor`
/// (law §6.6).
///
/// Judged by the core's `verify_signature` and that domain's digest; this layer computes not one byte itself.
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

/// The body of `adoption` (law §6.5). `attestor` and `attestation` are both present or both absent; when the
/// co-signature fails neither is carried, and the entry is still a legal entry (law §6.6: a failed
/// co-signature does not refuse the entry).
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

/// The head of the text the counterpart signs, followed by the preimage's hex (law §6.6's three members). One
/// name, one home.
pub const CLAIM_PREFIX: &str = "zikaron/1-adoption:";

/// The text sent to the counterpart: `zikaron/1-adoption:` plus the preimage's hex. The preimage includes
/// `prev`, so this text changes as soon as the ledger grows (a new head requires a new signature).
pub fn claim_text(author: &Address, rows: &[AnchorRow], prev: &str) -> String {
    format!("{CLAIM_PREFIX}{}", zikaron::hexfmt::encode(&preimage(author, rows, prev)))
}

/// What the text pasted back by the counterpart reads as: who is claiming, which anchors, their ledger head;
/// `preimage` is the bytes to sign.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claim {
    pub adopter: String,
    pub rows: Vec<AnchorRow>,
    pub prev: String,
    pub preimage: Vec<u8>,
}

/// Read a claim text. A wrong head, bad hex, unreadable JSON, a missing member, or a preimage recomputed from
/// the three members that differs from the given bytes (not canonical form) are all refused by name as
/// `CLAIM_SHAPE`, never guessed.
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
    let again = zikaron::entry::adoption_preimage(&adopter, &anchors_value(&rows), &prev);
    if again != bytes {
        return Err(bad("canon"));
    }
    Ok(Claim { adopter, rows, prev, preimage: bytes })
}

/// A table row's state, a closed table of five. Decided only here: in the ledger means "already in the
/// ledger" (the chain is not asked again); otherwise per the three questions (law §9.5). The no-senders form
/// (`NoSenders`) reads as a sender mismatch: the sender question was not asked, so it does not pass.
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

/// One anchor a key sent on chain (the material of a table row). `proof` is the three-question answer per row
/// (empty for rows already in the ledger, which are not asked).
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

/// A table row as a line of entry text (the same shape as [`rows_of`]: chain id, transaction, payload form,
/// content).
pub fn line_of(row: &AnchorRow) -> String {
    format!("{} {} {} {}", row.chain_id, row.tx, row.payload_kind, row.content)
}

/// Anchors a key sent, from a scan fragment (`fragment`'s `anchors`, in the anchoring crate's shape): those
/// whose sender is `sender`, each marked whether it is in the ledger (`ledger_ids`). The payload form follows
/// the registry contract's (`registry`).
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

/// Which block an anchor's transaction landed in (asks the chain; empty when unanswered, never zero).
pub fn block_of(eps: &[crate::chainx::Endpoint], row: &AnchorRow) -> Option<u64> {
    let mine: Vec<crate::chainx::Endpoint> = eps.iter().filter(|e| e.chain == row.chain_id).cloned().collect();
    if mine.is_empty() {
        return None;
    }
    let r = crate::chainx::ask(&mine, "eth_getTransactionByHash", &Value::Arr(vec![Value::Str(row.tx.clone())])).ok()?;
    let b = member(&r.value, "blockNumber")?;
    u64::from_str_radix(b.trim_start_matches("0x"), 16).ok()
}
