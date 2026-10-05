//! Envelope (law §4), signature (law §5) and the seven body types (law §6). Decisions follow the thirteen
//! steps of §4.3 and report the first failing token.

use crate::cryptox;
use crate::hexfmt;
use crate::json::{self, Value};
use crate::tokens::{self as t, Domain, EntryType, SigToken, Token};
use crate::trace;

/// An entry that passes all thirteen steps of §4.3.
#[derive(Clone, Debug)]
pub struct Entry {
    pub bytes: Vec<u8>,
    pub id: [u8; 32],
    pub value: Value,
    /// The entryType bytes as they came (law §6.9: an unlisted type is still an entry).
    pub entry_type: String,
    /// The same value read through the §6 type table: the seven known types, everything else `Other`. Compare
    /// this, not the literal.
    pub kind: EntryType,
    pub author: String,
    pub seq: u64,
    /// `None` at seq 0 (law §4.3 step 8).
    pub prev: Option<String>,
    pub body: Value,
}

impl Entry {
    pub fn id_hex(&self) -> String {
        hexfmt::encode(&self.id)
    }
}

/// Law §2.1: identity is total; every byte string has an entry_id.
pub fn entry_id(b: &[u8]) -> [u8; 32] {
    trace::mark(trace::K1);
    cryptox::sha256(b)
}

/// Law §1: a token is a non-empty string of bytes in 0x21-0x7E.
fn is_token(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| (0x21..=0x7e).contains(&b))
}

fn is_token_val(v: &Value) -> bool {
    matches!(v, Value::Str(s) if is_token(s))
}

fn is_hex32_val(v: &Value) -> bool {
    matches!(v, Value::Str(s) if hexfmt::is_hex32(s))
}

fn is_hex20_val(v: &Value) -> bool {
    matches!(v, Value::Str(s) if hexfmt::is_hex20(s))
}

fn is_hex65_val(v: &Value) -> bool {
    matches!(v, Value::Str(s) if hexfmt::is_hex65(s))
}

fn is_int_val(v: &Value) -> bool {
    matches!(v, Value::Int(_))
}

/// Prose string shape: any string (the character set was already judged by §3.5 test 5).
fn is_prose_val(v: &Value) -> bool {
    matches!(v, Value::Str(_))
}

/// Law §5.1: canonical bytes of the six-member object without the top-level `sig`.
pub fn b6_bytes(v: &Value) -> Vec<u8> {
    let six = match v {
        Value::Obj(ms) => Value::Obj(ms.iter().filter(|(k, _)| k != "sig").cloned().collect()),
        other => other.clone(),
    };
    json::canon_bytes(&six)
}

/// Law §5.2: M = D || 0x0A || hex32(presig).
pub fn message(domain: &str, presig: &[u8; 32]) -> Vec<u8> {
    let mut m = Vec::new();
    m.extend_from_slice(domain.as_bytes());
    m.push(0x0a);
    m.extend_from_slice(hexfmt::encode(presig).as_bytes());
    m
}

/// Law §5.3: digest = keccak256(0x19 || "Ethereum Signed Message:\n" || decimal(len(M)) || M).
pub fn eip191_digest(m: &[u8]) -> [u8; 32] {
    let mut pre = Vec::new();
    pre.push(0x19u8);
    pre.extend_from_slice(b"Ethereum Signed Message:\n");
    pre.extend_from_slice(m.len().to_string().as_bytes());
    pre.extend_from_slice(m);
    cryptox::keccak256(&pre)
}

/// presig and digest under a domain, computed together.
pub fn presig_and_digest(preimage: &[u8], domain: &str) -> ([u8; 32], [u8; 32]) {
    trace::mark(trace::K1);
    let presig = cryptox::sha256(preimage);
    let digest = eip191_digest(&message(domain, &presig));
    (presig, digest)
}

/// Laws §5.4 and §5.5: signature shape, range, low s, recovery, signer equals the given address. Tests run in
/// §5.4 order and fail with the matching §10 token. Public because kit law §3.1 signs its documents by §5.3
/// to §5.5 word for word.
pub fn verify_signature(sig_hex: &str, digest: &[u8; 32], expect_addr: &str) -> Result<(), SigToken> {
    trace::mark(trace::K1);
    // The signature is a law object: it guards its own hex65 (lowercase) shape.
    if !hexfmt::is_hex65(sig_hex) {
        return Err(SigToken::SigForm);
    }
    let raw = hexfmt::decode(sig_hex).ok_or(SigToken::SigForm)?;
    if raw.len() != 65 {
        return Err(SigToken::SigForm);
    }
    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&raw[0..32]);
    s.copy_from_slice(&raw[32..64]);
    let v = raw[64];
    if v != 27 && v != 28 {
        return Err(SigToken::SigV);
    }
    if !cryptox::in_range(&r) || !cryptox::in_range(&s) {
        return Err(SigToken::SigRange);
    }
    if !cryptox::is_low_s(&s) {
        return Err(SigToken::SigHighS);
    }
    let addr = cryptox::recover_address(digest, &r, &s, v - 27).ok_or(SigToken::SigRecover)?;
    if hexfmt::encode(&addr) != expect_addr {
        return Err(SigToken::SigSigner);
    }
    Ok(())
}

/// Law §6: judge the body by the type table; one token covers every body fault.
fn check_body(kind: EntryType, body: &Value) -> Result<(), Token> {
    body_check(kind, body).map_err(|_| Token::BodyField)
}

/// Which body member the type table refuses (law §6), by its path (`mode.mark`, `window.from`); `None` when
/// the body passes. The law answers every body fault with one token (`E_BODY_FIELD`); this names the member
/// for the people who wrote it, from the same table [`check`] judges by (step 12 asks this one function).
pub fn body_fault(kind: EntryType, body: &Value) -> Option<&'static str> {
    body_check(kind, body).err()
}

/// The type table itself: the first member that fails, by path.
fn body_check(kind: EntryType, body: &Value) -> Result<(), &'static str> {
    let get = |k: &str| body.member(k);
    match kind {
        EntryType::Genesis => {
            match get("statement_md") {
                Some(v) if is_prose_val(v) => {}
                _ => return Err("statement_md"),
            }
            Ok(())
        }
        EntryType::History => {
            match get("content") {
                Some(v) if is_hex32_val(v) => {}
                _ => return Err("content"),
            }
            match get("mode") {
                Some(m) if m.is_obj() => {
                    match m.member("mark") {
                        Some(v) if is_token_val(v) => {}
                        _ => return Err("mode.mark"),
                    }
                    match m.member("toolchain") {
                        Some(v) if is_hex32_val(v) => {}
                        _ => return Err("mode.toolchain"),
                    }
                }
                _ => return Err("mode"),
            }
            if let Some(v) = get("note_md") {
                if !is_prose_val(v) {
                    return Err("note_md");
                }
            }
            Ok(())
        }
        EntryType::Grant => {
            match get("grantee") {
                Some(v) if is_hex20_val(v) => {}
                _ => return Err("grantee"),
            }
            match get("work") {
                Some(v) if is_hex32_val(v) => {}
                _ => return Err("work"),
            }
            match get("terms") {
                Some(v) if is_hex32_val(v) => {}
                _ => return Err("terms"),
            }
            if let Some(v) = get("history") {
                if !is_hex32_val(v) {
                    return Err("history");
                }
            }
            if let Some(w) = get("window") {
                if !w.is_obj() {
                    return Err("window");
                }
                let from = match w.member("from") {
                    Some(v) if is_int_val(v) => v.as_int().unwrap_or(0),
                    _ => return Err("window.from"),
                };
                let to = match w.member("to") {
                    Some(v) if is_int_val(v) => v.as_int().unwrap_or(0),
                    _ => return Err("window.to"),
                };
                if from > to {
                    return Err("window");
                }
            }
            if let Some(v) = get("scope_md") {
                if !is_prose_val(v) {
                    return Err("scope_md");
                }
            }
            Ok(())
        }
        EntryType::Revocation => {
            match get("grant") {
                Some(v) if is_hex32_val(v) => {}
                _ => return Err("grant"),
            }
            if let Some(v) = get("case") {
                if !is_hex32_val(v) {
                    return Err("case");
                }
            }
            Ok(())
        }
        EntryType::Adoption => {
            match get("anchors") {
                Some(Value::Arr(items)) if !items.is_empty() => {
                    for el in items {
                        if !el.is_obj() {
                            return Err("anchors");
                        }
                        match el.member("chainId") {
                            Some(v) if is_int_val(v) => {}
                            _ => return Err("anchors.chainId"),
                        }
                        match el.member("tx") {
                            Some(v) if is_hex32_val(v) => {}
                            _ => return Err("anchors.tx"),
                        }
                        match el.member("payloadKind") {
                            Some(v) if is_token_val(v) => {}
                            _ => return Err("anchors.payloadKind"),
                        }
                        match el.member("content") {
                            Some(v) if is_hex32_val(v) => {}
                            _ => return Err("anchors.content"),
                        }
                    }
                }
                _ => return Err("anchors"),
            }
            let attestor = get("attestor");
            let attestation = get("attestation");
            match (attestor, attestation) {
                (None, None) => {}
                (Some(a), Some(g)) => {
                    if !is_hex20_val(a) {
                        return Err("attestor");
                    }
                    if !is_hex65_val(g) {
                        return Err("attestation");
                    }
                }
                (None, Some(_)) => return Err("attestor"),
                (Some(_), None) => return Err("attestation"),
            }
            Ok(())
        }
        EntryType::Succession => {
            match get("to") {
                Some(v) if is_hex20_val(v) => {}
                _ => return Err("to"),
            }
            match get("kind") {
                Some(v) if is_token_val(v) => {}
                _ => return Err("kind"),
            }
            match get("effective") {
                Some(v) if is_int_val(v) => {}
                _ => return Err("effective"),
            }
            match get("statement_md") {
                Some(v) if is_prose_val(v) => {}
                _ => return Err("statement_md"),
            }
            Ok(())
        }
        EntryType::Annotation => {
            if let Some(v) = get("subject") {
                if !is_hex32_val(v) {
                    return Err("subject");
                }
            }
            match get("note_md") {
                Some(v) if is_prose_val(v) => {}
                _ => return Err("note_md"),
            }
            Ok(())
        }
        // Law §6.9: an unlisted type only needs an object body (§4.3 step 9).
        EntryType::Other => Ok(()),
    }
}

const ENVELOPE_KEYS: [&str; 7] = ["spec", "entryType", "author", "seq", "prev", "body", "sig"];

/// Law §4.3 step 3: none of the seven keys missing (E_ENVELOPE_MISSING), no eighth key (E_ENVELOPE_CLOSED);
/// both before the signature. Member count alone decides nothing: seven members can miss one key and carry an
/// extra.
fn envelope_keys_ok(members: &[(String, Value)]) -> Result<(), Token> {
    for k in ENVELOPE_KEYS {
        if !members.iter().any(|(mk, _)| mk == k) {
            return Err(Token::EnvelopeMissing);
        }
    }
    if members.iter().any(|(mk, _)| !ENVELOPE_KEYS.contains(&mk.as_str())) {
        return Err(Token::EnvelopeClosed);
    }
    Ok(())
}

/// Law §4.3: the thirteen steps; what passes is an entry, nothing else is.
pub fn check(b: &[u8]) -> Result<Entry, Token> {
    trace::mark(trace::K1);
    // Step 1: §3.5 acceptance (its tokens).
    let v = json::accept(b)?;

    // Step 2: the root is an object.
    let members = match &v {
        Value::Obj(ms) => ms,
        _ => return Err(Token::Envelope),
    };

    // Step 3: exactly the seven keys.
    envelope_keys_ok(members)?;

    // Each step judges shape with its own token only; an absent member is that step's token. Step 4: spec
    // bytes equal `zikaron/1`.
    match v.member("spec") {
        Some(Value::Str(s)) if s == t::SPEC => {}
        _ => return Err(Token::Spec),
    }

    // Step 5: entryType is a token.
    let entry_type = match v.member("entryType") {
        Some(Value::Str(s)) if is_token(s) => s.clone(),
        _ => return Err(Token::EntryType),
    };

    // Step 6: author is hex20.
    let author = match v.member("author") {
        Some(Value::Str(s)) if hexfmt::is_hex20(s) => s.clone(),
        _ => return Err(Token::Author),
    };

    // Step 7: seq is an int.
    let seq = match v.member("seq") {
        Some(Value::Int(i)) => *i,
        _ => return Err(Token::Seq),
    };

    // Step 8: prev is null or hex32, null exactly when seq is 0.
    let prev = match v.member("prev") {
        Some(Value::Null) => None,
        Some(Value::Str(s)) if hexfmt::is_hex32(s) => Some(s.clone()),
        _ => return Err(Token::Prev),
    };
    if prev.is_none() != (seq == 0) {
        return Err(Token::PrevSeq);
    }

    // Step 9: body is an object.
    let body = match v.member("body") {
        Some(b) if b.is_obj() => b.clone(),
        _ => return Err(Token::Body),
    };

    // Step 10: sig is hex65.
    let sig = match v.member("sig") {
        Some(Value::Str(s)) if hexfmt::is_hex65(s) => s.clone(),
        _ => return Err(Token::SigForm),
    };

    let kind = EntryType::of(&entry_type);

    // Step 11: genesis position (law §4.2).
    if (kind == EntryType::Genesis) != (seq == 0) {
        return Err(Token::GenesisPlace);
    }

    // Step 12: the body table (law §6).
    check_body(kind, &body)?;

    // Step 13: the signature (law §5).
    let (_, digest) = presig_and_digest(&b6_bytes(&v), Domain::Entry.as_str());
    verify_signature(&sig, &digest, &author)?;

    Ok(Entry {
        bytes: b.to_vec(),
        id: entry_id(b),
        value: v,
        entry_type,
        kind,
        author,
        seq,
        prev,
        body,
    })
}

/// Law §6.6: canonical bytes of the cosignature preimage C = {"adopter", "anchors", "prev"}.
pub fn adoption_preimage(author: &str, anchors: &Value, prev: &str) -> Vec<u8> {
    let c = Value::Obj(vec![
        ("adopter".to_string(), Value::Str(author.to_string())),
        ("anchors".to_string(), anchors.clone()),
        ("prev".to_string(), Value::Str(prev.to_string())),
    ]);
    json::canon_bytes(&c)
}

/// Law §6.6: the attestor address when the cosignature passes; `None` when absent or failing. A failed
/// cosignature does not void the entry (§6.6); it only leaves the §9.5 element unproven.
pub fn attestation_attestor(e: &Entry) -> Option<String> {
    let attestor = e.body.member("attestor")?.as_str()?.to_string();
    let attestation = e.body.member("attestation")?.as_str()?.to_string();
    let anchors = e.body.member("anchors")?;
    let prev = e.prev.as_ref()?;
    let (_, digest) = presig_and_digest(
        &adoption_preimage(&e.author, anchors, prev),
        Domain::Adoption.as_str(),
    );
    match verify_signature(&attestation, &digest, &attestor) {
        Ok(()) => Some(attestor),
        Err(_) => None,
    }
}
