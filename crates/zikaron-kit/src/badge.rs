//! Grant payloads: encoding (kit law §6.1), decoding (§6.2) and the byte link (§6.3). Entries are accepted
//! through the public API of `zikaron`.

use crate::b64;
use crate::tokens::{self as t, BadgeDecodeToken, BadgeEncodeToken};
use zikaron::tokens::{EntryType, Token};
use zikaron::entry::{self, Entry};
use zikaron::hexfmt;
use zikaron::json::Value;
use zikaron::trace;

/// An encoding refusal (kit law §6.1): one of three tokens, two of them with a segment index.
#[derive(Clone, Debug, PartialEq)]
pub struct EncodeReject {
    pub token: BadgeEncodeToken,
    pub index: Option<usize>,
}

/// A decoding refusal (kit law §6.2): one of seven tokens; per-segment ones carry an index, E_BADGE_ENTRY
/// also carries the parent law's inner token.
#[derive(Clone, Debug, PartialEq)]
pub struct DecodeReject {
    pub token: BadgeDecodeToken,
    pub index: Option<usize>,
    pub inner: Option<Token>,
}

fn rej(token: BadgeDecodeToken) -> DecodeReject {
    DecodeReject {
        token,
        index: None,
        inner: None,
    }
}

fn rej_at(token: BadgeDecodeToken, index: usize) -> DecodeReject {
    DecodeReject {
        token,
        index: Some(index),
        inner: None,
    }
}

fn rej_entry(index: usize, inner: Token) -> DecodeReject {
    DecodeReject {
        token: BadgeDecodeToken::Entry,
        index: Some(index),
        inner: Some(inner),
    }
}

/// Kit law §6.3: the byte link between upstream `u` and downstream `d`. `d.body.upstream` is a string whose
/// bytes equal `hex32(entry_id(u))`, and both have the same `body.work`.
pub fn byte_link(u: &Entry, d: &Entry) -> bool {
    let up = match d.body.member("upstream") {
        Some(Value::Str(s)) => s.clone(),
        _ => return false,
    };
    if up != hexfmt::encode(&u.id) {
        return false;
    }
    match (u.body.member("work"), d.body.member("work")) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// Kit law §6.1: encode accepted grant entries, in order, into one payload. Each segment is tested for
/// acceptance then type (the in-segment order of §6.2 step 3); the cap is tested last.
pub fn encode(entries: &[Vec<u8>]) -> Result<String, EncodeReject> {
    trace::mark(t::K2);
    // §6.1 is defined for one or more segments; with zero, the bytes built by the rule are the prefix alone
    // (the harness always passes at least one path), and §6.2 refuses it at segment 0 with E_BADGE_B64.
    let mut segments: Vec<String> = Vec::with_capacity(entries.len());
    for (k, bytes) in entries.iter().enumerate() {
        let e = match entry::check(bytes) {
            Ok(e) => e,
            Err(_) => {
                return Err(EncodeReject {
                    token: BadgeEncodeToken::Entry,
                    index: Some(k),
                })
            }
        };
        if e.kind != EntryType::Grant {
            return Err(EncodeReject {
                token: BadgeEncodeToken::Type,
                index: Some(k),
            });
        }
        segments.push(b64::encode(bytes));
    }
    let payload = format!("{}{}", t::BADGE_PREFIX, segments.join("."));
    if payload.len() > t::BADGE_CAP {
        return Err(EncodeReject {
            token: BadgeEncodeToken::Cap,
            index: None,
        });
    }
    Ok(payload)
}

/// Kit law §6.2: decoding is total and judged in the written order; a token result renders nothing.
pub fn decode(payload: &[u8]) -> Result<Vec<Entry>, DecodeReject> {
    trace::mark(t::K2);
    let prefix = t::BADGE_PREFIX.as_bytes();
    if payload.len() < prefix.len() || &payload[..prefix.len()] != prefix {
        return Err(rej(BadgeDecodeToken::Prefix));
    }
    if payload.len() > t::BADGE_CAP {
        return Err(rej(BadgeDecodeToken::Cap));
    }
    let rest = &payload[prefix.len()..];
    // n dots give n + 1 segments; leading, trailing and doubled dots each give an empty segment.
    let segments: Vec<&[u8]> = rest.split(|c| *c == b'.').collect();

    let mut grants: Vec<Entry> = Vec::with_capacity(segments.len());
    for (k, seg) in segments.iter().enumerate() {
        let raw = match b64::decode(seg) {
            Some(x) => x,
            None => return Err(rej_at(BadgeDecodeToken::B64, k)),
        };
        let e = match entry::check(&raw) {
            Ok(e) => e,
            Err(tok) => return Err(rej_entry(k, tok)),
        };
        if e.kind != EntryType::Grant {
            return Err(rej_at(BadgeDecodeToken::Type, k));
        }
        grants.push(e);
    }
    // A segment-0 grant whose body has an `upstream` key (any value) means the payload does not start at the
    // grant that declares no upstream.
    if grants[0].body.member("upstream").is_some() {
        return Err(rej(BadgeDecodeToken::Incomplete));
    }
    for k in 1..grants.len() {
        if !byte_link(&grants[k - 1], &grants[k]) {
            return Err(rej_at(BadgeDecodeToken::Link, k));
        }
    }
    Ok(grants)
}

/// The first hop of a grant chain that is not among `carried`, byte for byte (`None` when every hop is). A
/// bundle that carries a grant code and the entries beside it must carry the same bytes in both places: a
/// code naming a hop the bundle does not hold would be checked against entries that are not its own.
pub fn uncarried<'a>(hops: &'a [Vec<u8>], carried: &[Vec<u8>]) -> Option<&'a [u8]> {
    trace::mark(t::K2);
    hops.iter().find(|h| !carried.iter().any(|b| b == *h)).map(|h| h.as_slice())
}

