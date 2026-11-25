use super::tokens::{self as t2, Domain, EntryType};
use crate::entry as v1;
use crate::hexfmt;
use crate::json::{self, Value};
use crate::tokens::Token;
use crate::trace;

#[derive(Clone, Debug)]
pub struct Entry {
    pub bytes: Vec<u8>,
    pub id: [u8; 32],
    pub id_hex: String,
    pub value: Value,
    pub entry_type: String,
    pub kind: EntryType,
    pub author: String,
    pub seq: u64,
    pub prev: Option<String>,
    pub body: Value,
}

fn is_token(v: &Value) -> bool {
    matches!(v, Value::Str(s) if !s.is_empty() && s.bytes().all(|b| (0x21..=0x7e).contains(&b)))
}

fn is_hex20(v: &Value) -> bool {
    matches!(v, Value::Str(s) if hexfmt::is_hex20(s))
}

fn is_hex32(v: &Value) -> bool {
    matches!(v, Value::Str(s) if hexfmt::is_hex32(s))
}

fn is_int(v: &Value) -> bool {
    matches!(v, Value::Int(_))
}

fn is_prose(v: &Value) -> bool {
    matches!(v, Value::Str(_))
}

type Test = fn(&Value) -> bool;

fn req(body: &Value, key: &'static str, test: Test) -> Result<(), &'static str> {
    match body.member(key) {
        Some(v) if test(v) => Ok(()),
        _ => Err(key),
    }
}

fn opt(body: &Value, key: &'static str, test: Test) -> Result<(), &'static str> {
    match body.member(key) {
        Some(v) if !test(v) => Err(key),
        _ => Ok(()),
    }
}

fn object_rows(
    v: &Value,
    key: &'static str,
    required: &[(&'static str, Test)],
    optional: &[(&'static str, Test)],
) -> Result<(), &'static str> {
    if !v.is_obj() {
        return Err(key);
    }
    for (k, test) in required {
        req(v, k, *test).map_err(|_| key)?;
    }
    for (k, test) in optional {
        opt(v, k, *test).map_err(|_| key)?;
    }
    Ok(())
}

pub fn body_fault(kind: EntryType, body: &Value) -> Option<&'static str> {
    body_check(kind, body).err()
}

fn body_check(kind: EntryType, body: &Value) -> Result<(), &'static str> {
    match kind {
        EntryType::Genesis => {
            req(body, "statement_md", is_prose)?;
            if let Some(c) = body.member("continues") {
                object_rows(c, "continues", &[("root", is_hex20), ("entry_id", is_hex32)], &[])?;
            }
            Ok(())
        }
        EntryType::History => {
            req(body, "content", is_hex32)?;
            match body.member("mode") {
                Some(m) => object_rows(m, "mode", &[("mark", is_token), ("toolchain", is_hex32)], &[])?,
                None => return Err("mode"),
            }
            opt(body, "note_md", is_prose)?;
            if let Some(f) = body.member("for") {
                object_rows(
                    f,
                    "for",
                    &[("app", is_token), ("identity", is_hex20), ("ref", is_token)],
                    &[("seat", is_token)],
                )?;
            }
            Ok(())
        }
        EntryType::Grant => {
            req(body, "grantee", is_hex20)?;
            req(body, "work", is_hex32)?;
            req(body, "terms", is_hex32)?;
            opt(body, "history", is_hex32)?;
            if let Some(w) = body.member("window") {
                object_rows(w, "window", &[("from", is_int), ("to", is_int)], &[])?;
                if w.member("from").and_then(Value::as_int) > w.member("to").and_then(Value::as_int) {
                    return Err("window");
                }
            }
            opt(body, "scope_md", is_prose)?;
            opt(body, "upstream", is_hex32)?;
            opt(body, "ledger", is_hex20)?;
            Ok(())
        }
        EntryType::Revocation => {
            req(body, "grant", is_hex32)?;
            opt(body, "case", is_hex32)?;
            Ok(())
        }
        EntryType::Succession => {
            req(body, "to", is_hex20)?;
            req(body, "kind", is_token)?;
            req(body, "effective", is_int)?;
            req(body, "statement_md", is_prose)?;
            Ok(())
        }
        EntryType::Migration => {
            req(body, "spec", is_token)?;
            req(body, "statement_md", is_prose)?;
            Ok(())
        }
        EntryType::Annotation => {
            opt(body, "subject", is_hex32)?;
            req(body, "note_md", is_prose)?;
            Ok(())
        }
        EntryType::Retraction => {
            req(body, "subject", is_hex32)?;
            opt(body, "note_md", is_prose)?;
            Ok(())
        }
        EntryType::Other => Ok(()),
    }
}

const ENVELOPE_KEYS: [&str; 7] = ["spec", "entryType", "author", "seq", "prev", "body", "sig"];

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

pub fn entry_digest(v: &Value) -> [u8; 32] {
    v1::presig_and_digest(&v1::b6_bytes(v), Domain::Entry.as_str()).1
}

pub fn check(b: &[u8]) -> Result<Entry, Token> {
    trace::mark(t2::K3);
    let v = json::accept(b)?;
    let members = match &v {
        Value::Obj(ms) => ms,
        _ => return Err(Token::Envelope),
    };
    envelope_keys_ok(members)?;
    match v.member("spec") {
        Some(Value::Str(s)) if s == t2::SPEC => {}
        _ => return Err(Token::Spec),
    }
    let entry_type = match v.member("entryType") {
        Some(x) if is_token(x) => x.as_str().unwrap_or_default().to_string(),
        _ => return Err(Token::EntryType),
    };
    let author = match v.member("author") {
        Some(Value::Str(s)) if hexfmt::is_hex20(s) => s.clone(),
        _ => return Err(Token::Author),
    };
    let seq = match v.member("seq") {
        Some(Value::Int(i)) => *i,
        _ => return Err(Token::Seq),
    };
    let prev = match v.member("prev") {
        Some(Value::Null) => None,
        Some(Value::Str(s)) if hexfmt::is_hex32(s) => Some(s.clone()),
        _ => return Err(Token::Prev),
    };
    if prev.is_none() != (seq == 0) {
        return Err(Token::PrevSeq);
    }
    let body = match v.member("body") {
        Some(x) if x.is_obj() => x.clone(),
        _ => return Err(Token::Body),
    };
    let sig = match v.member("sig") {
        Some(Value::Str(s)) if hexfmt::is_hex65(s) => s.clone(),
        _ => return Err(Token::SigForm),
    };
    let kind = EntryType::of(&entry_type);
    if (kind == EntryType::Genesis) != (seq == 0) {
        return Err(Token::GenesisPlace);
    }
    if body_check(kind, &body).is_err() {
        return Err(Token::BodyField);
    }
    v1::verify_signature(&sig, &entry_digest(&v), &author)?;
    let id = v1::entry_id(b);
    Ok(Entry {
        bytes: b.to_vec(),
        id,
        id_hex: hexfmt::encode(&id),
        value: v,
        entry_type,
        kind,
        author,
        seq,
        prev,
        body,
    })
}
