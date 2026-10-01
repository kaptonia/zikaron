//! Diligence desk. Type an author address, get four panels; a snapshot has zero evidentiary weight.
//!
//! ─── Each panel has its own owner; this layer decides nothing ───
//!
//! 1. Audit label: the anchoring crate scans anchors (endpoint rule), the core produces the report and label;
//! it takes the reader's reading (`readerx::read_scanned`), and this layer writes no separate reading
//! for "someone else's ledger".
//! 2. Record quantities: the kit crate's `reading::depth`, taken through the depth page's
//! `depthx::read`: the same implementation as the author's own proof, so both sides' readings match byte for
//! byte.
//! 3. Grant history and double-sale check: the history comes from the reader's table reading; the double-sale
//! check is here: an exclusive window I want to buy that overlaps any live grant turns red. Live = not
//! revoked by a revocation referencing it; overlap has one owner (`grantx::overlaps`, the grant ledger's rule), and
//! this layer writes no range rule of its own. Unlike the grant ledger's double-sale gate, which asks about the author's local
//! exclusive flag, that flag cannot be read on someone else's ledger, so this asks only "live" and
//! "overlapping", not the flag.
//! 4. Succession history: one row per succession in the pile, with whom it was handed to; the lineage is
//! computed by `auditx::senders_of` (the same algorithm as self-audit and the reader).
//!
//! ─── No indexer ───
//!
//! This layer neither discovers nor enumerates addresses; the address book (`settings.book`) is purely local
//! and never becomes a directory.
//!
//! ─── A snapshot has zero evidentiary weight ───
//!
//! The whole page can be saved as a snapshot, a note for oneself. The real evidence is the chain itself,
//! recomputable at any time: the first cell of the snapshot file says `evidenceWeight: none`, and it is
//! written through the glue crate's landing path (`landing::land_bytes`), refused when something is already
//! at the path, never overwriting.

use crate::fault::{Fault, Known};
use zikaron::json::Value;

/// One succession's row in the history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Succession {
    pub seq: u64,
    pub id: String,
    /// The key handed over (the entry's author).
    pub from: String,
    /// Who it was handed to.
    pub to: String,
    pub kind: String,
    pub effective: u64,
}

fn text<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).and_then(|(_, x)| match x {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        }),
        _ => None,
    }
}

fn int(v: &Value, k: &str) -> Option<u64> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).and_then(|(_, x)| match x {
            Value::Int(n) => Some(*n),
            _ => None,
        }),
        _ => None,
    }
}

/// Succession history. One row per succession in the pile, in ascending seq (ties by id). Whether an entry is
/// accepted is judged by the core's thirteen steps; this layer only lays out those that passed as rows.
pub fn successions(bytes: &[Vec<u8>]) -> Vec<Succession> {
    let mut out: Vec<Succession> = Vec::new();
    for b in bytes {
        let Ok(e) = zikaron::entry::check(b) else { continue };
        if e.kind != zikaron::tokens::EntryType::Succession {
            continue;
        }
        out.push(Succession {
            seq: e.seq,
            id: zikaron::hexfmt::encode(&e.id),
            from: e.author.clone(),
            to: text(&e.body, "to").unwrap_or("").to_string(),
            kind: text(&e.body, "kind").unwrap_or("").to_string(),
            effective: int(&e.body, "effective").unwrap_or(0),
        });
    }
    out.sort_by(|a, b| (a.seq, &a.id).cmp(&(b.seq, &b.id)));
    out
}

/// Double-sale check. An exclusive window I want to buy that overlaps any live grant (same record, not
/// revoked) turns red.
///
/// Without a window there is no reading (an empty table; the face says "no window, not checked"), never
/// colliding with "forever": that would paint every historical grant red, and red must be able to say which
/// window collided.
pub fn double_sale(
    rows: &[crate::grantx::Row],
    work: &str,
    window: Option<(u64, u64)>,
) -> Vec<crate::grantx::Row> {
    let Some(w) = window else { return Vec::new() };
    let want = work.trim().to_ascii_lowercase();
    if want.is_empty() {
        return Vec::new();
    }
    rows.iter()
        .filter(|r| !r.revoked)
        .filter(|r| r.work.to_ascii_lowercase() == want)
        .filter(|r| crate::grantx::overlaps(r.window, Some(w)))
        .cloned()
        .collect()
}

/// One diligence pass's reading: four panels. Every cell is what the background pass brought back; the frame
/// recomputes nothing.
#[derive(Clone, Debug)]
pub struct Read {
    /// The typed address (lowercase).
    pub who: String,
    pub anchors: usize,
    pub asked: usize,
    /// How many entries' bytes were obtained; zero means "anchors seen, bytes not seen", and panels two to
    /// four then have no reading.
    pub entries: usize,
    pub label: String,
    pub timeline: Vec<crate::ledgerx::Row>,
    pub grants: Vec<crate::grantx::Row>,
    /// Target record hash (may be empty); empty means panel two has no reading.
    pub work: String,
    /// The kit crate's depth reading, unchanged (`depthx::three` lays it out).
    pub depth: Option<Value>,
    /// The window I want to buy.
    pub window: Option<(u64, u64)>,
    /// What the double-sale check collided with.
    pub clash: Vec<crate::grantx::Row>,
    pub successions: Vec<Succession>,
    /// This ledger's lineage (the genesis key, plus every key handed over by successions).
    pub lineage: Vec<String>,
    /// The chain's current time (the smallest across endpoints); none when unavailable, and the badge then
    /// has no reading.
    pub now: Option<u64>,
    /// Block time of the latest anchor (`readerx::Book::latest`, same scan).
    pub latest: Option<u64>,
    /// Which level the ledger bytes came from (`supplyx::find_book`'s four levels, the same as the reader and
    /// the check page), and where; `None` when no level has them.
    pub from: Option<(crate::supplyx::Level, String)>,
    /// How many items the publish address level fetched per the manifest.
    pub files: Option<usize>,
    /// Which levels failed on the way, each named.
    pub misses: Vec<(crate::supplyx::Level, crate::fault::Fault)>,
}

impl Read {
    pub fn only_anchors(&self) -> bool {
        self.entries == 0
    }

    /// Whether the double-sale check is red. No window is not green: `window` is then empty, and the face has
    /// a separate sentence.
    pub fn double_sold(&self) -> bool {
        !self.clash.is_empty()
    }
}

/// Assemble the four panels. A read ledger (panel one and the first half of three), this pass's scan
/// fragment, the stack of bytes, the target record and the window I want to buy go in; the quantities go to
/// the kit crate (through the depth page), succession history and lineage are read now, and the double-sale comparison is
/// made now.
pub fn assemble(
    book: crate::readerx::Book,
    fragment: &Value,
    bytes: &[Vec<u8>],
    work: &str,
    window: Option<(u64, u64)>,
) -> Result<Read, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::D1);
    let work = work.trim().to_string();
    if !work.is_empty() && !zikaron::hexfmt::is_hex32(&work) {
        return Err(Fault::known(Known::ContentShape, work));
    }
    // Quantities: a reading only with bytes and a record given; produced by the kit crate, and this layer
    // computes not one number.
    let depth = if bytes.is_empty() || work.is_empty() {
        None
    } else {
        Some(crate::depthx::read(bytes, fragment, &work)?.value)
    };
    let clash = double_sale(&book.grants, &work, window);
    Ok(Read {
        latest: book.latest,
        who: book.who,
        anchors: book.anchors,
        asked: book.asked,
        entries: book.entries,
        label: book.label,
        timeline: book.timeline,
        grants: book.grants,
        work,
        depth,
        window,
        clash,
        successions: successions(bytes),
        lineage: crate::auditx::senders_of(bytes),
        now: None,
        from: None,
        files: None,
        misses: Vec::new(),
    })
}

/// The snapshot file's first cell's member name. One name, one home.
pub const EVIDENCE_WEIGHT: &str = "evidenceWeight";

/// The literal of the snapshot file's first cell: zero evidentiary weight.
pub const EVIDENCE_WEIGHT_NONE: &str = "none";

/// Write the four panels as a snapshot (canonical JSON).
pub fn snapshot_value(r: &Read) -> Value {
    let s = |x: &str| Value::Str(x.to_string());
    let grants = r
        .grants
        .iter()
        .map(|g| {
            Value::Obj(vec![
                ("grantee".into(), s(&g.grantee)),
                ("id".into(), s(&g.id)),
                ("revoked".into(), Value::Bool(g.revoked)),
                ("seq".into(), Value::Int(g.seq)),
                ("terms".into(), s(&g.terms)),
                (
                    "window".into(),
                    match g.window {
                        Some((a, b)) => Value::Obj(vec![("from".into(), Value::Int(a)), ("to".into(), Value::Int(b))]),
                        None => Value::Null,
                    },
                ),
                ("work".into(), s(&g.work)),
            ])
        })
        .collect();
    let succ = r
        .successions
        .iter()
        .map(|x| {
            Value::Obj(vec![
                ("effective".into(), Value::Int(x.effective)),
                ("from".into(), s(&x.from)),
                ("id".into(), s(&x.id)),
                ("kind".into(), s(&x.kind)),
                ("seq".into(), Value::Int(x.seq)),
                ("to".into(), s(&x.to)),
            ])
        })
        .collect();
    Value::Obj(vec![
        ("anchors".into(), Value::Int(r.anchors as u64)),
        ("asked".into(), Value::Int(r.asked as u64)),
        ("clash".into(), Value::Arr(r.clash.iter().map(|c| s(&c.id)).collect())),
        ("depth".into(), r.depth.clone().unwrap_or(Value::Null)),
        ("entriesSeen".into(), Value::Int(r.entries as u64)),
        (EVIDENCE_WEIGHT.into(), s(EVIDENCE_WEIGHT_NONE)),
        ("grants".into(), Value::Arr(grants)),
        ("label".into(), s(&r.label)),
        ("lineage".into(), Value::Arr(r.lineage.iter().map(|x| s(x)).collect())),
        ("recomputeFrom".into(), s("chain")),
        ("successions".into(), Value::Arr(succ)),
        ("who".into(), s(&r.who)),
        (
            "window".into(),
            match r.window {
                Some((a, b)) => Value::Obj(vec![("from".into(), Value::Int(a)), ("to".into(), Value::Int(b))]),
                None => Value::Null,
            },
        ),
        ("work".into(), s(&r.work)),
    ])
}

/// Save a snapshot. Written through the glue crate's landing path: refused when something is already at the
/// path, never overwriting. Returns the byte count written.
pub fn snapshot(r: &Read, to: &std::path::Path) -> Result<usize, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
    crate::trace::mark(crate::feature::Feature::D1);
    if to.as_os_str().is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::t(crate::lang::Key::Tail108).to_string()));
    }
    let bytes = zikaron::json::canon_bytes(&snapshot_value(r));
    zikaron_glue::landing::land_bytes(to, &bytes).map_err(|t| {
        Fault::landing(t.code(), t.subject())
    })?;
    Ok(bytes.len())
}
