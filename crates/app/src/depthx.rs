//! Depth page. None of the three quantities is computed here.
//!
//! ─── One implementation, byte-identical readings on both sides ───
//!
//! The owner of the depth reading is the kit crate's [`zikaron_kit::reading::depth`] (kit law §9.2), and the
//! grantee seat's verifier reads it too. So "byte-identical on both sides" is not two copies compared, but
//! only one.
//!
//! This layer does two parameter jobs: recognize a record hash cell, and fetch the audit outcome.
//!
//! ─── No external report file (as kit law §9) ───
//!
//! Self-reported depth has zero evidentiary weight; the honest deliverable is always the disclosure kit,
//! and the buyer computes for themselves. So this file has no path to disk: no `fs::write`, no `land_bytes`;
//! the reading lives only on the face.

use crate::fault::{Fault, Known};
use zikaron::json::Value;
use zikaron_kit::reading;

/// A record hash cell. Unrecognized is refused by name, never asking depth with an empty string.
pub fn work_of(typed: &str) -> Result<String, Fault> {
    let t = typed.trim();
    if t.is_empty() {
        return Err(Fault::known(Known::ContentShape, crate::lang::t(crate::lang::Key::Tail107).to_string()));
    }
    if !zikaron::hexfmt::is_hex32(t) {
        return Err(Fault::known(Known::ContentShape, t.to_string()));
    }
    Ok(t.to_string())
}

/// Record hashes that appear in this ledger (the path choosing directly from entries).
///
/// Reads history's `content` (the same predicate as kit law §9.2); byte order, deduplicated, so two passes
/// over one ledger give the same list.
pub fn works_in(items: &[Vec<u8>]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in items {
        let Ok(e) = zikaron::entry::check(b) else { continue };
        if e.kind != zikaron::tokens::EntryType::History {
            continue;
        }
        if let Value::Obj(m) = &e.body {
            if let Some((_, Value::Str(c))) = m.iter().find(|(k, _)| k == "content") {
                out.push(c.clone());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The three quantities laid out for the face. Every cell is taken from the kit crate's reading; nothing is
/// recomputed.
pub struct Three {
    /// Whether the input is well formed (as the kit crate says).
    pub valid: bool,
    /// This pass's audit label (the reading carries it: which records the depth is relative to).
    pub label: String,
    /// Whether this ledger has this record.
    pub found: bool,
    /// Earliest: the upper bound of the first anchor's time (none means no reading, not zero).
    pub earliest: Option<u64>,
    /// Deepest: the number of anchored entries.
    pub deepest: u64,
    /// Most continuous: anchored count and span.
    pub anchored: u64,
    pub span: u64,
    /// The kit crate's result, unchanged (read when the face lays out things beyond the eleven cells).
    pub value: Value,
}

fn member<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).map(|(_, x)| x),
        _ => None,
    }
}

fn int(v: &Value, k: &str) -> Option<u64> {
    match member(v, k) {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    }
}

fn flag(v: &Value, k: &str) -> bool {
    matches!(member(v, k), Some(Value::Bool(true)))
}

/// Read the kit crate's reading as three quantities. Unreadable cells say so, never zero.
pub fn three(v: Value) -> Three {
    let cont = member(&v, "continuity").cloned().unwrap_or(Value::Null);
    Three {
        valid: flag(&v, "valid"),
        label: match member(&v, "label") {
            Some(Value::Str(s)) => s.clone(),
            _ => String::new(),
        },
        found: flag(&v, "found"),
        earliest: int(&v, "earliest"),
        deepest: int(&v, "deepest").unwrap_or(0),
        anchored: int(&cont, "anchored").unwrap_or(0),
        span: int(&cont, "span").unwrap_or(0),
        value: v,
    }
}

/// Read depth once. The audit outcome comes from the core (the input assembly has one owner), the three
/// quantities from the kit crate; this layer computes not one number.
pub fn read(items: &[Vec<u8>], fragment: &Value, work: &str) -> Result<Three, Fault> {
    let outcome = crate::auditx::outcome_of(items, fragment)?;
    Ok(three(reading::depth(Some(&outcome), work)))
}
