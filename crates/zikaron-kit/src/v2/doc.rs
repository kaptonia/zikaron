use super::tokens as t2;
use crate::b64;
use crate::tokens::{self as t1, BadgeDecodeToken, BadgeEncodeToken};
use zikaron::entry as v1;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::tokens::{EntryType as V1Type, Token};
use zikaron::trace;
use zikaron::v2::entry as v2;
use zikaron::v2::tokens::EntryType as V2Type;

#[derive(Clone, Debug)]
pub enum Accepted {
    V1(v1::Entry),
    V2(v2::Entry),
}

impl Accepted {
    pub fn id_hex(&self) -> String {
        match self {
            Accepted::V1(e) => e.id_hex(),
            Accepted::V2(e) => e.id_hex.clone(),
        }
    }

    pub fn body(&self) -> &Value {
        match self {
            Accepted::V1(e) => &e.body,
            Accepted::V2(e) => &e.body,
        }
    }

    pub fn is_grant(&self) -> bool {
        match self {
            Accepted::V1(e) => e.kind == V1Type::Grant,
            Accepted::V2(e) => e.kind == V2Type::Grant,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        match self {
            Accepted::V1(e) => &e.bytes,
            Accepted::V2(e) => &e.bytes,
        }
    }

    pub fn is_v1(&self) -> bool {
        matches!(self, Accepted::V1(_))
    }
}

const ENVELOPE_KEYS: [&str; 7] = ["spec", "entryType", "author", "seq", "prev", "body", "sig"];

pub fn accept(b: &[u8]) -> Result<Accepted, Token> {
    let v = json::accept(b)?;
    if let Value::Obj(ms) = &v {
        let seven = ENVELOPE_KEYS.iter().all(|k| ms.iter().any(|(mk, _)| mk == k))
            && ms.iter().all(|(mk, _)| ENVELOPE_KEYS.contains(&mk.as_str()));
        if seven && v.member("spec").and_then(Value::as_str) == Some(zikaron::tokens::SPEC) {
            return v1::check(b).map(Accepted::V1);
        }
    }
    v2::check(b).map(Accepted::V2)
}

pub fn doc_id(b: &[u8]) -> String {
    hexfmt::encode(&v1::entry_id(b))
}

pub fn byte_link(u: &Accepted, d: &Accepted) -> bool {
    match d.body().member("upstream") {
        Some(Value::Str(up)) if *up == u.id_hex() => d.body().member("work") == u.body().member("work"),
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EncodeReject {
    pub token: BadgeEncodeToken,
    pub index: Option<usize>,
}

pub fn badge_encode(entries: &[Vec<u8>]) -> Result<String, EncodeReject> {
    trace::mark(t2::K4);
    let mut segs: Vec<String> = Vec::with_capacity(entries.len());
    for (k, b) in entries.iter().enumerate() {
        match accept(b) {
            Err(_) => return Err(EncodeReject { token: BadgeEncodeToken::Entry, index: Some(k) }),
            Ok(e) if !e.is_grant() => return Err(EncodeReject { token: BadgeEncodeToken::Type, index: Some(k) }),
            Ok(_) => segs.push(b64::encode(b)),
        }
    }
    let payload = format!("{}{}", t1::BADGE_PREFIX, segs.join("."));
    if payload.len() > t1::BADGE_CAP {
        return Err(EncodeReject { token: BadgeEncodeToken::Cap, index: None });
    }
    Ok(payload)
}

#[derive(Clone, Debug, PartialEq)]
pub struct DecodeReject {
    pub token: BadgeDecodeToken,
    pub index: Option<usize>,
    pub inner: Option<Token>,
}

fn reject(token: BadgeDecodeToken, index: Option<usize>) -> DecodeReject {
    DecodeReject { token, index, inner: None }
}

pub fn badge_decode(p: &[u8]) -> Result<Vec<Accepted>, DecodeReject> {
    trace::mark(t2::K4);
    let prefix = t1::BADGE_PREFIX.as_bytes();
    if !p.starts_with(prefix) {
        return Err(reject(BadgeDecodeToken::Prefix, None));
    }
    if p.len() > t1::BADGE_CAP {
        return Err(reject(BadgeDecodeToken::Cap, None));
    }
    let mut grants: Vec<Accepted> = Vec::new();
    for (k, seg) in p[prefix.len()..].split(|c| *c == b'.').enumerate() {
        let raw = b64::decode(seg).ok_or(reject(BadgeDecodeToken::B64, Some(k)))?;
        let e = accept(&raw).map_err(|tok| DecodeReject { token: BadgeDecodeToken::Entry, index: Some(k), inner: Some(tok) })?;
        if !e.is_grant() {
            return Err(reject(BadgeDecodeToken::Type, Some(k)));
        }
        grants.push(e);
    }
    if grants[0].body().member("upstream").is_some() {
        return Err(reject(BadgeDecodeToken::Incomplete, None));
    }
    for k in 1..grants.len() {
        if !byte_link(&grants[k - 1], &grants[k]) {
            return Err(reject(BadgeDecodeToken::Link, Some(k)));
        }
    }
    Ok(grants)
}
