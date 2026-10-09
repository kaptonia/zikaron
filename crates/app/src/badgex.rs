//! Badges. Choose a grant from the vault; its upstream chain is followed to the root automatically, and badge
//! export produces the payload and QR code.
//!
//! The kit crate rejects a payload whose first hop still has an upstream (`E_BADGE_INCOMPLETE`), so this
//! layer follows `upstream` from the chosen grant until one has none, which is the root. If a hop is in
//! neither the vault nor this seat's ledger, it reports by name that the root cannot be reached
//! (`CHAIN_INCOMPLETE`, with the missing id).
//!
//! Encoding belongs to the kit crate's `badge::encode` (capped at 2953); after encoding, `badge::decode`
//! verifies once, and only BADGE_OK counts as a badge. `qr` produces the QR matrix, drawn as SVG and written
//! to disk only through the glue crate's landing. Recipients check badges on the shared check page or with
//! the CLI.

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

/// Follows upstreams from `id` to the root and returns the byte chain, root first. Refused by name when `id`
/// is not shaped like an entry id (nothing is looked up), when the root cannot be reached (a hop not in the
/// pool), and when the chain exceeds 64 hops (the code's own limit; the payload refusal names the 65th hop).
pub fn chain_for(pool: &[Vec<u8>], id: &str) -> Result<Vec<Vec<u8>>, Fault> {
    // Check the id's shape first (`0x` plus 64 lowercase hex digits; surrounding whitespace allowed): an empty
    // or differently written id is refused as a shape error, never looked up and reported unreachable (the
    // copied code, the badge and the grant file all start here).
    if !zikaron::hexfmt::is_hex32(id.trim()) {
        return Err(Fault::known(Known::ContentShape, id.to_string()));
    }
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
    // Still pointing upstream after 64 hops: past the hop limit, reported as such (the chain may well reach its
    // root; truncating silently would only fail later, at encoding).
    if chain.last().and_then(|b| zikaron::entry::check(b).ok()).map(|e| upstream_of(&e).is_some()).unwrap_or(false) {
        return Err(Fault::known(Known::PayloadRefused, crate::lang::filln(crate::lang::Key::TailPastHops, &[&at])));
    }
    chain.reverse();
    Ok(chain)
}

/// Grant text: encodes a grant (with its upstreams, root first) as the text the counterpart pastes into the
/// check page. Encoding belongs to the kit crate (`zikaron_kit::badge::encode`); this layer builds no bytes
/// itself. The copied code, the badge and the grant file all use this one encoding, and a refusal carries the
/// kit crate's token.
pub fn payload_text(chain: &[Vec<u8>]) -> Result<String, Fault> {
    zikaron_kit::badge::encode(chain).map_err(|r| {
        Fault::known(Known::PayloadRefused, format!("{}{}", r.token.as_str(), r.index.map(|i| crate::lang::filln(crate::lang::Key::Tail087, &[&(i).to_string()])).unwrap_or_default()))
    })
}

/// A grant's full code: its chain to the root ([`chain_for`]), encoded ([`payload_text`]). A relicensed
/// grant's code carries every hop above it, as its badge and grant file do.
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

/// The kit crate's pass/fail verdict on a payload, as values.
pub fn verify_result(payload: &[u8]) -> Checked {
    match zikaron_kit::badge::decode(payload) {
        Ok(v) => Checked { ok: true, token: zikaron_kit::tokens::BADGE_OK.to_string(), count: Some(v.len()), index: None },
        Err(r) => Checked { ok: false, token: r.token.as_str().to_string(), count: None, index: r.index },
    }
}

/// The kit crate's pass/fail verdict as text: BADGE_OK, or its refusal token (from [`verify_result`]).
pub fn verify(payload: &[u8]) -> (bool, String) {
    let c = verify_result(payload);
    if c.ok {
        (true, format!("{} {}", c.token, c.count.unwrap_or(0)))
    } else {
        (false, format!("{}{}", c.token, c.index.map(|i| crate::lang::filln(crate::lang::Key::Tail087, &[&(i).to_string()])).unwrap_or_default()))
    }
}

/// The chain check, delegated to the kit crate. Each hop's audit input comes from this vault's last re-check
/// (the same item's card); a hop in this seat's ledger without a card has no input (the kit crate treats it as
/// undecided). Returns the kit crate's result object unchanged (verdict, token, failing point); this layer
/// judges no hop. [`chain_verdict`] summarizes it in one line.
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

/// The chain check in one line: GREEN / PARTIAL, or FAIL with the kit crate's failure point (chain token and
/// hop index), from [`chain_result`].
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

/// An exported badge.
#[derive(Clone, Debug)]
pub struct Made {
    /// Which grant the badge was made for (shown only in that item's detail).
    pub grant: String,
    /// The chain check line, filled by the export pass from the vault's cards; empty means not done.
    pub chain: String,
    /// The chain time of the re-check that supplied the per-hop audit inputs; `None` without cards.
    pub input_at: Option<u64>,
    /// The QR matrix (computed once at export and drawn by the UI, never re-encoded per frame).
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

/// Exports a badge: encode (kit crate), self-verify (kit crate), draw (qr), write (glue crate).
/// `_pass` is the exit gate's [`crate::exitgate::Pass`]: this can only be reached through the gate.
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
    let code = crate::qr::make(payload.as_bytes()).map_err(|e| {
        let why = match e {
            crate::qr::NotMade::TooLong => crate::lang::Key::Tail088,
            crate::qr::NotMade::SelfCheck => crate::lang::Key::Tail237,
        };
        Fault::known(Known::PayloadRefused, crate::lang::t(why).to_string())
    })?;
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
