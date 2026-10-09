//! Documents: signatures (kit law §3), fingerprint manifests (§4), acknowledgements (§5), pairing (§5.3) and
//! attribution (§5.4). Canonical form and signatures go through the public API of `zikaron`.

use crate::tokens::{self as t, Domain, DocToken, PairVerdict};
use zikaron::tokens::{CanonToken, SigToken};
use zikaron::entry;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::trace;

/// A refusal: one token, plus the row index `E_FPM_ROW` carries.
#[derive(Clone, Debug, PartialEq)]
pub struct Reject {
    pub token: DocToken,
    pub index: Option<usize>,
}

fn rej(token: DocToken) -> Reject {
    Reject { token, index: None }
}

fn rej_at(token: DocToken, index: usize) -> Reject {
    Reject {
        token,
        index: Some(index),
    }
}

/// A law §3 token passed through `Canon`, bytes unchanged.
fn canon(token: CanonToken) -> Reject {
    rej(DocToken::Canon(token))
}

/// A law §5 token passed through `Sig` (borrowed word for word by kit law §3.1).
fn sig(token: SigToken) -> Reject {
    rej(DocToken::Sig(token))
}

#[derive(Clone, Debug)]
pub struct Fpm {
    pub doc_id: [u8; 32],
    pub author: String,
    pub work: String,
    pub grant: Option<String>,
    /// (recipient, variant), in input order.
    pub rows: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
pub struct Ack {
    pub doc_id: [u8; 32],
    pub recipient: String,
    pub fpm: String,
    pub variant: String,
}

/// Kit law §1: `doc_id(b) = sha256(b)`; identity is total here too.
pub fn doc_id(b: &[u8]) -> [u8; 32] {
    entry::entry_id(b)
}

/// Kit law §3.1: the canonical bytes without the top-level `sig` are the preimage B.
fn preimage(v: &Value) -> Vec<u8> {
    entry::b6_bytes(v)
}

/// Kit law §3.1: document signatures follow law §5.3 to §5.5 word for word; the signer must equal the
/// member the document names.
fn verify_doc_signature(v: &Value, sig: &str, domain: &str, signer: &str) -> Result<(), Reject> {
    let (_, digest) = entry::presig_and_digest(&preimage(v), domain);
    entry::verify_signature(sig, &digest, signer).map_err(self::sig)
}

fn as_hex20(v: Option<&Value>) -> Option<String> {
    let x = v?.as_str()?;
    if hexfmt::is_hex20(x) {
        Some(x.to_string())
    } else {
        None
    }
}

fn as_hex32(v: Option<&Value>) -> Option<String> {
    let x = v?.as_str()?;
    if hexfmt::is_hex32(x) {
        Some(x.to_string())
    } else {
        None
    }
}

fn member_set_ok(members: &[(String, Value)], keys: &[&str]) -> Result<(), Reject> {
    for k in keys {
        if !members.iter().any(|(mk, _)| mk == k) {
            return Err(rej(DocToken::DocMissing));
        }
    }
    if members.iter().any(|(mk, _)| !keys.contains(&mk.as_str())) {
        return Err(rej(DocToken::DocClosed));
    }
    Ok(())
}

const FPM_KEYS: [&str; 7] = ["spec", "author", "work", "grant", "rows", "note_md", "sig"];
const ACK_KEYS: [&str; 6] = ["spec", "recipient", "fpm", "variant", "note_md", "sig"];

/// Kit law §4.1 and §4.2: per-row shape (array order; E_FPM_ROW carries the index), distinct recipients,
/// distinct variants, rows in recipient byte order.
fn check_rows(rows_val: &[Value]) -> Result<Vec<(String, String)>, Reject> {
    let mut rows: Vec<(String, String)> = Vec::with_capacity(rows_val.len());
    for (i, row) in rows_val.iter().enumerate() {
        let ms = match row {
            Value::Obj(ms) => ms,
            _ => return Err(rej_at(DocToken::FpmRow, i)),
        };
        if ms.len() != 2
            || !ms.iter().any(|(k, _)| k == "recipient")
            || !ms.iter().any(|(k, _)| k == "variant")
        {
            return Err(rej_at(DocToken::FpmRow, i));
        }
        let recipient = match as_hex20(row.member("recipient")) {
            Some(x) => x,
            None => return Err(rej_at(DocToken::FpmRow, i)),
        };
        let variant = match as_hex32(row.member("variant")) {
            Some(x) => x,
            None => return Err(rej_at(DocToken::FpmRow, i)),
        };
        rows.push((recipient, variant));
    }
    // Find duplicates by sorting and comparing neighbours (pairwise comparison would be quadratic).
    let mut recipients: Vec<&str> = rows.iter().map(|(r, _)| r.as_str()).collect();
    recipients.sort_unstable();
    for i in 1..recipients.len() {
        if recipients[i - 1] == recipients[i] {
            return Err(rej(DocToken::FpmDupRecipient));
        }
    }
    let mut variants: Vec<&str> = rows.iter().map(|(_, v)| v.as_str()).collect();
    variants.sort_unstable();
    for i in 1..variants.len() {
        if variants[i - 1] == variants[i] {
            return Err(rej(DocToken::FpmDupVariant));
        }
    }
    for i in 1..rows.len() {
        if rows[i - 1].0.as_bytes() > rows[i].0.as_bytes() {
            return Err(rej(DocToken::FpmRowOrder));
        }
    }
    Ok(rows)
}

/// Kit law §4.2: decision order of a fingerprint manifest.
pub fn check_fpm(b: &[u8]) -> Result<Fpm, Reject> {
    trace::mark(t::K2);
    let v = json::accept(b).map_err(canon)?;
    let members = match &v {
        Value::Obj(ms) => ms,
        _ => return Err(rej(DocToken::Doc)),
    };
    member_set_ok(members, &FPM_KEYS)?;

    match v.member("spec") {
        Some(Value::Str(s)) if s == t::SPEC_FPM => {}
        _ => return Err(rej(DocToken::Spec)),
    }
    let author = as_hex20(v.member("author")).ok_or_else(|| rej(DocToken::FpmAuthor))?;
    let work = as_hex32(v.member("work")).ok_or_else(|| rej(DocToken::FpmWork))?;
    let grant = match v.member("grant") {
        Some(Value::Null) => None,
        Some(Value::Str(s)) if hexfmt::is_hex32(s) => Some(s.clone()),
        _ => return Err(rej(DocToken::FpmGrant)),
    };

    let rows_val = match v.member("rows") {
        Some(Value::Arr(a)) if !a.is_empty() => a,
        _ => return Err(rej(DocToken::FpmRows)),
    };
    let rows = check_rows(rows_val)?;

    match v.member("note_md") {
        Some(Value::Str(_)) => {}
        _ => return Err(rej(DocToken::FpmNote)),
    }
    let sig = match v.member("sig") {
        Some(Value::Str(s)) if hexfmt::is_hex65(s) => s.clone(),
        _ => return Err(rej(DocToken::Sig(SigToken::SigForm))),
    };
    verify_doc_signature(&v, &sig, Domain::Fpm.as_str(), &author)?;

    Ok(Fpm {
        doc_id: doc_id(b),
        author,
        work,
        grant,
        rows,
    })
}

/// Kit law §5.2: decision order of an acknowledgement.
pub fn check_ack(b: &[u8]) -> Result<Ack, Reject> {
    trace::mark(t::K2);
    let v = json::accept(b).map_err(canon)?;
    let members = match &v {
        Value::Obj(ms) => ms,
        _ => return Err(rej(DocToken::Doc)),
    };
    member_set_ok(members, &ACK_KEYS)?;

    match v.member("spec") {
        Some(Value::Str(s)) if s == t::SPEC_ACK => {}
        _ => return Err(rej(DocToken::Spec)),
    }
    let recipient = as_hex20(v.member("recipient")).ok_or_else(|| rej(DocToken::AckRecipient))?;
    let fpm = as_hex32(v.member("fpm")).ok_or_else(|| rej(DocToken::AckFpm))?;
    let variant = as_hex32(v.member("variant")).ok_or_else(|| rej(DocToken::AckVariant))?;
    match v.member("note_md") {
        Some(Value::Str(_)) => {}
        _ => return Err(rej(DocToken::AckNote)),
    }
    let sig = match v.member("sig") {
        Some(Value::Str(s)) if hexfmt::is_hex65(s) => s.clone(),
        _ => return Err(rej(DocToken::Sig(SigToken::SigForm))),
    };
    verify_doc_signature(&v, &sig, Domain::Ack.as_str(), &recipient)?;

    Ok(Ack {
        doc_id: doc_id(b),
        recipient,
        fpm,
        variant,
    })
}

/// The six pairing verdicts of kit law §5.3.
#[derive(Clone, Debug, PartialEq)]
pub enum Pairing {
    Paired { recipient: String, variant: String },
    FpmInvalid(Reject),
    AckInvalid(Reject),
    AckFpmMismatch,
    AckNoRow,
    AckVariantMismatch,
}

impl Pairing {
    pub fn verdict(&self) -> PairVerdict {
        match self {
            Pairing::Paired { .. } => PairVerdict::Paired,
            Pairing::FpmInvalid(_) => PairVerdict::FpmInvalid,
            Pairing::AckInvalid(_) => PairVerdict::AckInvalid,
            Pairing::AckFpmMismatch => PairVerdict::AckFpmMismatch,
            Pairing::AckNoRow => PairVerdict::AckNoRow,
            Pairing::AckVariantMismatch => PairVerdict::AckVariantMismatch,
        }
    }
}

/// Kit law §5.3: pairing, in the written order.
pub fn pair(m: &[u8], a: &[u8]) -> Pairing {
    trace::mark(t::K2);
    let fpm = match check_fpm(m) {
        Ok(x) => x,
        Err(e) => return Pairing::FpmInvalid(e),
    };
    let ack = match check_ack(a) {
        Ok(x) => x,
        Err(e) => return Pairing::AckInvalid(e),
    };
    if ack.fpm != hexfmt::encode(&fpm.doc_id) {
        return Pairing::AckFpmMismatch;
    }
    let row = match fpm.rows.iter().find(|(r, _)| r == &ack.recipient) {
        Some(r) => r,
        None => return Pairing::AckNoRow,
    };
    if row.1 != ack.variant {
        return Pairing::AckVariantMismatch;
    }
    Pairing::Paired {
        recipient: ack.recipient.clone(),
        variant: ack.variant.clone(),
    }
}

/// Kit law §5.4: attribution. Without a pairing the pairing verdict is reported; with one, whether the sha256
/// of the bytes equals the acknowledged variant.
pub fn attribute(m: &[u8], a: &[u8], x: &[u8]) -> (Pairing, Option<bool>) {
    trace::mark(t::K2);
    let p = pair(m, a);
    match &p {
        Pairing::Paired { variant, .. } => {
            let hit = &hexfmt::encode(&doc_id(x)) == variant;
            (p, Some(hit))
        }
        _ => (p, None),
    }
}
