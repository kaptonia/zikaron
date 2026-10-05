//! Badge packer. Choose a grant from the vault, cascade the upstream chain to the root automatically,
//! and badge export produces the payload and QR code.
//!
//! ─── The chain starts at the root ───
//!
//! The kit crate's reading (kit law §6): upstream on the first segment is `E_BADGE_INCOMPLETE`. So this layer
//! follows `upstream` up from the chosen grant until one with no upstream, which is the root; if a hop is in
//! neither the vault nor this seat's ledger, it says by name that the root cannot be reached
//! (`CHAIN_INCOMPLETE`, with the missing id), never pretending.
//!
//! ─── Encode, verify, draw ───
//!
//! Encoding belongs to the kit crate's `badge::encode` (the same cap of 2953); after encoding, the kit
//! crate's `badge::decode` verifies once, and only BADGE_OK counts as a badge; `qr` produces the QR matrix,
//! drawn as SVG and written to disk; writing goes only through the glue crate's landing. The customer's check
//! entry is the shared check page and CLI; this layer has no other.

use crate::fault::{Fault, Known};
use crate::home::Home;
use std::path::{Path, PathBuf};
use zikaron::entry::Entry;

fn upstream_of(e: &Entry) -> Option<String> {
    match &e.body {
        zikaron::json::Value::Obj(m) => m.iter().find(|(k, _)| k == "upstream").and_then(|(_, v)| match v {
            zikaron::json::Value::Str(s) => Some(s.clone()),
            _ => None,
        }),
        _ => None,
    }
}

/// The candidate pool: every vault item, plus every grant in this seat's ledger (grants a relicensor issued
/// live in their own ledger).
pub fn pool(home: &Home) -> Result<Vec<Vec<u8>>, Fault> {
    let mut out: Vec<Vec<u8>> = crate::vaultx::held(home)?.into_iter().map(|h| h.bytes).collect();
    if let Ok(l) = home.ledger() {
        if let Ok(s) = l.survey() {
            for b in s.items {
                if let Ok(e) = zikaron::entry::check(&b) {
                    if e.kind == zikaron::tokens::EntryType::Grant {
                        out.push(b);
                    }
                }
            }
        }
    }
    Ok(out)
}

/// Cascade to the root. From `id`, follow upstream; return the byte chain from the root. Refused by name when
/// the root cannot be reached.
pub fn chain_for(pool: &[Vec<u8>], id: &str) -> Result<Vec<Vec<u8>>, Fault> {
    let find = |want: &str| -> Option<Entry> {
        pool.iter()
            .filter_map(|b| zikaron::entry::check(b).ok())
            .find(|e| e.id_hex().eq_ignore_ascii_case(want))
    };
    let mut chain: Vec<Vec<u8>> = Vec::new();
    let mut at = id.trim().to_string();
    for _ in 0..64 {
        let Some(e) = find(&at) else {
            return Err(Fault::known(Known::ChainIncomplete, at));
        };
        if e.kind != zikaron::tokens::EntryType::Grant {
            return Err(Fault::known(Known::NotAGrant, e.kind.as_str().to_string()));
        }
        let up = upstream_of(&e);
        chain.push(e.bytes.clone());
        match up {
            Some(u) => at = u,
            None => break,
        }
    }
    // Still carrying upstream after sixty-four hops: the root cannot be reached, said by name (truncating
    // silently would fail later, at encoding).
    if chain.last().and_then(|b| zikaron::entry::check(b).ok()).map(|e| upstream_of(&e).is_some()).unwrap_or(false) {
        return Err(Fault::known(Known::ChainIncomplete, at));
    }
    chain.reverse();
    Ok(chain)
}

/// Grant text: encode a grant (with its upstreams, from the root down) as the text the counterpart pastes
/// into the check page. Encoding belongs to the kit crate (`zikaron_kit::badge::encode`); this layer
/// assembles not one byte. The one encoding: the copied code, the badge and the grant file all go through it,
/// and a refusal is the kit crate's token by name.
pub fn payload_text(chain: &[Vec<u8>]) -> Result<String, Fault> {
    zikaron_kit::badge::encode(chain).map_err(|r| {
        Fault::known(Known::PayloadRefused, format!("{}{}", r.token.as_str(), r.index.map(|i| crate::lang::filln(crate::lang::Key::Tail087, &[&(i).to_string()])).unwrap_or_default()))
    })
}

/// A grant's whole code: its chain cascaded to the root ([`chain_for`]), then encoded ([`payload_text`]). A
/// relicensed grant's code carries every hop above it, as its badge and grant file do.
pub fn code_for(pool: &[Vec<u8>], id: &str) -> Result<String, Fault> {
    payload_text(&chain_for(pool, id)?)
}

/// The kit crate's reading of a payload as values: whether it decoded, the token it answered (`BADGE_OK` or
/// its refusal token), how many items it carried, and at which item a refusal points.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub ok: bool,
    pub token: String,
    pub count: Option<usize>,
    pub index: Option<usize>,
}

/// The kit crate's two colors, as values.
pub fn verify_result(payload: &[u8]) -> Checked {
    match zikaron_kit::badge::decode(payload) {
        Ok(v) => Checked { ok: true, token: zikaron_kit::tokens::BADGE_OK.to_string(), count: Some(v.len()), index: None },
        Err(r) => Checked { ok: false, token: r.token.as_str().to_string(), count: None, index: r.index },
    }
}

/// The kit crate's two colors: BADGE_OK, or its refusal token (said from [`verify_result`]).
pub fn verify(payload: &[u8]) -> (bool, String) {
    let c = verify_result(payload);
    if c.ok {
        (true, format!("{} {}", c.token, c.count.unwrap_or(0)))
    } else {
        (false, format!("{}{}", c.token, c.index.map(|i| crate::lang::filln(crate::lang::Key::Tail087, &[&(i).to_string()])).unwrap_or_default()))
    }
}

/// Chain check (kit law §10.5), handed to the kit crate. Each hop's audit input comes from this vault's last
/// re-check (the card of the same item); a hop in this seat's ledger without a card has no input (the kit
/// crate reads it as undecided). Answers the kit crate's §10.5 result object as it is (verdict, token,
/// failing point); this layer judges no hop. [`chain_verdict`] says it in one sentence.
pub fn chain_result(chain: &[Vec<u8>], cards: &[crate::vaultx::Card], now: Option<u64>) -> zikaron::json::Value {
    crate::trace::mark(crate::feature::Feature::D9);
    let hops: Vec<zikaron_kit::check::Hop> = chain
        .iter()
        .map(|g| {
            let id = zikaron::entry::check(g).map(|e| e.id_hex()).unwrap_or_default();
            let input = cards.iter().find(|c| c.id == id).and_then(|c| c.input.clone());
            zikaron_kit::check::Hop { grant: g, input }
        })
        .collect();
    zikaron_kit::check::chain_check(&hops, now)
}

/// The chain check in one sentence: GREEN / PARTIAL, or FAIL with the kit crate's failure point (chain token
/// and hop index), read from [`chain_result`].
pub fn chain_verdict(chain: &[Vec<u8>], cards: &[crate::vaultx::Card], now: Option<u64>) -> String {
    use zikaron::json::Value;
    let v = chain_result(chain, cards, now);
    let get = |k: &str| match &v {
        Value::Obj(m) => m.iter().find(|(x, _)| x == k).map(|(_, y)| y.clone()),
        _ => None,
    };
    let verdict = match get("verdict") {
        Some(Value::Str(s)) => s,
        _ => String::new(),
    };
    let token = match get("token") {
        Some(Value::Str(s)) => s,
        _ => String::new(),
    };
    let failing = match get("failing") {
        Some(Value::Obj(m)) => {
            let kind = m.iter().find(|(k, _)| k == "kind").and_then(|(_, x)| match x { Value::Str(s) => Some(s.clone()), _ => None }).unwrap_or_default();
            let index = m.iter().find(|(k, _)| k == "index").and_then(|(_, x)| match x { Value::Int(n) => Some(*n), _ => None }).unwrap_or(0);
            format!("{kind} {index}")
        }
        _ => String::new(),
    };
    if verdict == zikaron_kit::tokens::CheckVerdict::Fail.as_str() {
        let mut s = verdict;
        if !token.is_empty() {
            s.push_str(" · ");
            s.push_str(&token);
        }
        if !failing.is_empty() {
            s.push_str(" · ");
            s.push_str(&failing);
        }
        s
    } else {
        verdict
    }
}

/// A badge's reading.
#[derive(Clone, Debug)]
pub struct Made {
    /// Which grant the badge was made for (the face shows it only in that item's detail).
    pub grant: String,
    /// The chain check sentence (kit law §10.5), filled by the export pass from the vault's cards; empty
    /// means not done.
    pub chain: String,
    /// Which re-check pass supplied the chain check's per-hop audit inputs (that pass's chain time); none
    /// without cards.
    pub input_at: Option<u64>,
    /// The QR matrix (computed once at export, drawn by the face, never re-encoded per frame).
    pub modules: Vec<Vec<bool>>,
    pub hops: usize,
    pub payload: String,
    pub bytes: usize,
    pub verdict: String,
    pub version: usize,
    pub size: usize,
    pub txt: PathBuf,
    pub svg: PathBuf,
}

pub const TXT: &str = "badge.txt";
pub const SVG: &str = "badge.svg";

/// Make one. Encode (kit crate), self-verify (kit crate), draw (qr), write (glue crate).
/// `_pass` is the exit gate's [`crate::exitgate::Pass`]: there is no way to this effect but through the gate.
pub fn export(_pass: &crate::exitgate::Pass, chain: &[Vec<u8>], out: &Path) -> Result<Made, Fault> {
    crate::trace::mark(crate::feature::Feature::D9);
    if out.as_os_str().is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::t(crate::lang::Key::Tail070).to_string()));
    }
    let payload = payload_text(chain)?;
    let (ok, verdict) = verify(payload.as_bytes());
    if !ok {
        return Err(Fault::known(Known::PayloadRefused, verdict));
    }
    let code = crate::qr::encode(payload.as_bytes())
        .ok_or_else(|| Fault::known(Known::PayloadRefused, crate::lang::t(crate::lang::Key::Tail088).to_string()))?;
    std::fs::create_dir_all(out).map_err(|e| crate::fault::classify(&e, &out.display().to_string()))?;
    let txt = out.join(TXT);
    let svg = out.join(SVG);
    for (p, b) in [(&txt, payload.as_bytes().to_vec()), (&svg, crate::qr::svg(&code).into_bytes())] {
        zikaron_glue::landing::land_bytes(p, &b).map_err(|t| {
            Fault::of_landing(t)
        })?;
    }
    Ok(Made {
        grant: String::new(),
        chain: String::new(),
        input_at: None,
        modules: code.modules.clone(),
        hops: chain.len(),
        bytes: payload.len(),
        payload,
        verdict,
        version: code.version,
        size: code.size,
        txt,
        svg,
    })
}
