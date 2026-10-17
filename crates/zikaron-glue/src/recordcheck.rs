//! Verifying a record package record by record: is each record there, is it anchored and when first, is its
//! original the one it records. One check, called by both the desktop's verification and the command line's
//! `kit-verify`; every fact it states comes from the core (the entry, the audit outcome), the kit crate (the bounds of
//! first anchoring) or the package's own manifest and files.

use std::path::Path;
use zikaron::audit::Outcome;
use zikaron::json::Value;
use zikaron::tokens::EntryType;

/// **Each entry's first-anchor block time**: the earliest block timestamp among the counted anchors that reach
/// it (the kit format's rule, the same as the depth reading's "first anchored"). Entries no anchor reaches are left
/// out.
pub fn first_anchored(outcome: &Outcome) -> Vec<(String, u64)> {
    let lines = zikaron_kit::reading::Lines::of(&outcome.ledger);
    let bounds = zikaron_kit::reading::bounds(outcome, &lines);
    bounds.iter().enumerate().filter_map(|(i, t)| t.map(|t| (lines.id(i).to_string(), t))).collect()
}

/// The earliest counted anchor that reaches a ledger entry, in full: the same rule as [`first_anchored`]'s
/// time, plus which anchor gave it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FirstAnchor {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_timestamp: u64,
    /// The anchoring transaction (hex32).
    pub tx: String,
    /// The hash it anchored (hex32): the entry it points to, at or after the one it reaches.
    pub hash: String,
    /// The registry contract whose log carried it (hex20), when the scan noted one.
    pub registry: Option<String>,
}

/// **Each entry's first anchor, in full**: per entry, among the counted anchors whose time is that entry's
/// bound and that reach it, the one with the smallest (chain id, block, transaction). Entries no anchor reaches are
/// left out.
pub fn first_anchors(outcome: &Outcome, fragment: &Value) -> Vec<(String, FirstAnchor)> {
    let lines = zikaron_kit::reading::Lines::of(&outcome.ledger);
    let bounds = zikaron_kit::reading::bounds(outcome, &lines);
    let int = |v: &Value, k: &str| match v.member(k) {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    };
    let text = |v: &Value, k: &str| match v.member(k) {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    };
    // The fragment's counted rows from a sender of this lineage (the set `counted` was trimmed from).
    let rows: Vec<FirstAnchor> = match fragment.member("anchors") {
        Some(Value::Arr(a)) => a
            .iter()
            .filter(|r| text(r, "verdict").as_deref() == Some(zikaron::tokens::Verdict::Counted.as_str()))
            .filter(|r| text(r, "sender").map(|s| outcome.lineage.iter().any(|l| l.eq_ignore_ascii_case(&s))).unwrap_or(false))
            .filter_map(|r| {
                Some(FirstAnchor {
                    chain_id: int(r, "chainId")?,
                    block_number: int(r, "blockNumber")?,
                    block_timestamp: int(r, "blockTimestamp")?,
                    tx: text(r, "tx")?,
                    hash: text(r, "hash")?,
                    registry: None,
                })
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    for (i, t) in bounds.iter().enumerate() {
        let Some(t) = *t else { continue };
        let first = outcome
            .counted
            .iter()
            .filter(|a| a.block_timestamp == t)
            .filter(|a| lines.position(&a.hash).map(|p| zikaron_kit::reading::reachable_at(&outcome.ledger, &lines, p, i)).unwrap_or(false))
            .flat_map(|a| rows.iter().filter(move |r| r.hash.eq_ignore_ascii_case(&a.hash) && r.block_timestamp == a.block_timestamp))
            .min_by(|x, y| (x.chain_id, x.block_number, &x.tx).cmp(&(y.chain_id, y.block_number, &y.tx)));
        if let Some(f) = first {
            out.push((lines.id(i).to_string(), f.clone()));
        }
    }
    out
}


/// The original file of a record in the kit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Original {
    /// The manifest's `contents` row matches, and the sha256 of the kit's file equals this record's
    /// `content`.
    Match,
    /// The row matches, but the file's bytes hash to another digest.
    Mismatch,
    /// The manifest has no `contents` row for this record, or the kit lacks the file in that row.
    Missing,
}

/// Whether this record is anchored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnChain {
    /// An anchor counted by this audit reaches it; `first_at` is the earliest block time among the anchors
    /// that reach it (kit law §8.2, read the same way as the depth reading's "first anchored":
    /// `zikaron_kit::reading::bounds`).
    Anchored { first_at: u64 },
    /// The chain was read and no counted anchor reaches it.
    NotAnchored,
    /// The chain was not read (no network configured, chain unreachable), or this pass left out a network that
    /// could hold its anchor ([`unread_where_missed`]): never shown as "not anchored".
    Unread,
}

/// One row of record bundle verification: one record (`history`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordRow {
    /// Entry id (record hash).
    pub id: String,
    /// This record's `content` (hex32, lowercase).
    pub content: String,
    pub original: Original,
    pub chain: OnChain,
    /// The first anchor reaching it, in full (the same reading as `chain`'s time); `None` when not anchored or
    /// not read.
    pub first: Option<FirstAnchor>,
}

/// Kit law §7.3's `contents` rows: (content, path), read from the manifest unchanged (empty when the
/// manifest is unreadable).
pub fn contents_of(manifest: &[u8]) -> Vec<(String, String)> {
    // Manifest member names come from the kit writer's name table (`crate::names::Field`); no literal
    // is written here.
    use crate::names::Field;
    let Ok(v) = zikaron::json::parse(manifest) else { return Vec::new() };
    let Some(Value::Arr(rows)) = v.member(Field::Contents.as_str()) else { return Vec::new() };
    rows.iter()
        .filter_map(|r| {
            let content = match r.member(Field::Content.as_str()) {
                Some(Value::Str(s)) => s.to_ascii_lowercase(),
                _ => return None,
            };
            let path = match r.member(Field::Path.as_str()) {
                Some(Value::Str(s)) => s.clone(),
                _ => return None,
            };
            Some((content, path))
        })
        .collect()
}

/// The file each `contents` row points to in a kit directory: (content, bytes; `None` when the kit lacks it).
pub fn originals_in_dir(dir: &Path) -> Vec<(String, Option<Vec<u8>>)> {
    let manifest = std::fs::read(dir.join(crate::names::MANIFEST)).unwrap_or_default();
    contents_of(&manifest)
        .into_iter()
        .map(|(content, path)| {
            let bytes = std::fs::read(dir.join(&path)).or_else(|_| std::fs::read(dir.join(crate::names::FILES_DIR).join(&path))).ok();
            (content, bytes)
        })
        .collect()
}

/// As [`originals_in_dir`], from a file list (a grant file, or a kit fetched from a publish address).
pub fn originals_in_pairs(pairs: &[(String, Vec<u8>)]) -> Vec<(String, Option<Vec<u8>>)> {
    let manifest = pairs.iter().find(|(p, _)| p == crate::names::MANIFEST).map(|(_, b)| b.clone()).unwrap_or_default();
    let files_prefixed = |path: &str| format!("{}/{}", crate::names::FILES_DIR, path);
    contents_of(&manifest)
        .into_iter()
        .map(|(content, path)| {
            let bytes = pairs.iter().find(|(p, _)| *p == path || *p == files_prefixed(&path)).map(|(_, b)| b.clone());
            (content, bytes)
        })
        .collect()
}

/// Verify a record package record by record: one row per record (`history`) in it. Original: the `contents` row
/// for this record's `content`, with the package's file hashed with sha256 and compared. Anchored and first-anchor
/// block time: on the audit outcome of the package's entries with a fragment and that fragment (`None` when the
/// chain was not read), each record's bounds as the kit crate computes them (`reading::bounds`, as for depth),
/// once for all records. The desktop's verification and the command line's `kit-verify` both judge by this one
/// function.
pub fn records(bytes: &[Vec<u8>], originals: &[(String, Option<Vec<u8>>)], read: Option<(&Outcome, &Value)>) -> Vec<RecordRow> {
    crate::seam_v2();
    let times = read.map(|(o, _)| first_anchored(o));
    let firsts = read.map(|(o, f)| first_anchors(o, f)).unwrap_or_default();
    bytes
        .iter()
        .filter_map(|b| zikaron::entry::check(b).ok())
        .filter_map(|e| work_of(e.kind, &e.body).map(|c| (e.id_hex(), c.to_ascii_lowercase())))
        .map(|(id, content)| {
            let mine: Vec<&Option<Vec<u8>>> = originals.iter().filter(|(c, _)| *c == content).map(|(_, b)| b).collect();
            let original = if mine.iter().any(|b| b.as_ref().map(|x| zikaron::hexfmt::encode(&crate::recording::content_of(x)).eq_ignore_ascii_case(&content)).unwrap_or(false)) {
                Original::Match
            } else if mine.iter().any(|b| b.is_some()) {
                Original::Mismatch
            } else {
                Original::Missing
            };
            let chain = match &times {
                None => OnChain::Unread,
                Some(at) => match at.iter().find(|(h, _)| *h == id).map(|(_, t)| *t) {
                    Some(t) => OnChain::Anchored { first_at: t },
                    None => OnChain::NotAnchored,
                },
            };
            let first = match chain {
                OnChain::Anchored { .. } => firsts.iter().find(|(h, _)| *h == id).map(|(_, f)| f.clone()),
                _ => None,
            };
            RecordRow { id, content, original, chain, first }
        })
        .collect()
}

/// A record's work: the `content` member of a `history` entry's body (law §6.2); no other entry type has one.
pub fn work_of(kind: EntryType, body: &Value) -> Option<String> {
    if kind != EntryType::History {
        return None;
    }
    match body.member("content") {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    }
}
