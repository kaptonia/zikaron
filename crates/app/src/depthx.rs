//! Depth page. None of the three depth quantities is computed here.
//!
//! The depth reading belongs to [`zikaron_kit::reading::depth`] (kit law §9.2), which the grantee's verifier
//! also uses, so both sides read depth with the same code and get byte-identical results. This module only
//! recognizes the record hash cell and fetches the audit outcome.
//!
//! No report file is written (kit law §9): self-reported depth carries no evidentiary weight. The honest
//! deliverable is always the disclosure kit, from which the buyer computes depth themselves. So this module
//! has no path to disk; the reading lives only on the page.

use crate::fault::{Fault, Known};
use zikaron::json::Value;
use zikaron_kit::reading;

/// Parses a record hash cell. Anything but a hex32 value is refused by name, so depth is never asked for an
/// empty string.
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

/// Record hashes that appear in this ledger (for choosing directly from entries).
///
/// Reads each history entry's `content`, the same field the kit reads (kit law §9.2); sorted and
/// deduplicated, so repeated passes over one ledger give the same list.
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

/// The three quantities laid out for the page. Every field is taken from the kit crate's reading; nothing is
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
    /// The kit crate's result, unchanged (for anything the page shows beyond these fields).
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

/// One depth cell as a page says it: a value, "none", or "chain not read".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Said<T> {
    Is(T),
    Nothing,
    ChainUnread,
}

/// The three depth cells as a page shows them. `unread` means a chain was not read this pass (none read, or
/// a network skipped). Then a cell that reads "none" or falls short may have its anchor on the unread chain,
/// so it shows "chain not read"; a value found on the chains that were read shows as is. Without `unread`, or
/// for a work this ledger does not hold (no chain could change that), every cell is as the kit read it. Every
/// page uses this one rule.
pub fn said(t: &Three, unread: bool) -> (Said<u64>, Said<u64>, Said<(u64, u64)>) {
    let unread = unread && t.found;
    let first = match t.earliest {
        Some(at) => Said::Is(at),
        None if unread => Said::ChainUnread,
        None => Said::Nothing,
    };
    let deepest = if unread && t.earliest.is_none() { Said::ChainUnread } else { Said::Is(t.deepest) };
    let continuity = if unread && t.anchored < t.span { Said::ChainUnread } else { Said::Is((t.anchored, t.span)) };
    (first, deepest, continuity)
}

/// Reads depth once: the audit outcome from the core (`auditx`), the three quantities from the kit crate. No
/// number is computed here.
pub fn read(items: &[Vec<u8>], fragment: &Value, work: &str) -> Result<Three, Fault> {
    let outcome = crate::auditx::outcome_of(items, fragment)?;
    Ok(three(reading::depth(Some(&outcome), work)))
}
