//! The ledger view: entries as a table in descending seq order.
//!
//! Validity is not decided here: whether bytes are an entry, its type, seq and prev all come from the core's
//! `entry::check`, and whether the chain is broken comes from the core's `audit` label, never from comparing
//! prev here. Summaries read the body members law §6 defines for each entry type and show "—" when
//! they are missing instead of a plausible default.
//!
//! Anchor lamp colors: green (anchored) only when the audit report's `anchored` item includes the entry; grey
//! (queued) from the local queue file; yellow for recorded but not anchored. Without an audit green never
//! appears, so "not checked" and "not on chain" look different.

use crate::fault::{Fault, Known};
use crate::home::Home;
use zikaron::json::Value;
use zikaron::tokens::EntryType;

/// Anchor status of an entry, shared by entry cards, record cards, the queue page and watch rows.
///
/// `Anchored` comes only from the audit report. The other states come from the local queue file
/// (`queue::Step` and its `blocks` and `anchored` records) and the ledger's retractions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lamp {
    /// Confirmed: the core's report says it is anchored.
    Anchored,
    /// Confirming: the receipt says it was included (recorded in the queue file); the audit has not counted it
    /// yet.
    Included,
    /// Broadcast and acknowledged by the node, waiting for the receipt.
    Submitted,
    /// To be anchored: queued, not sent.
    Queued,
    /// Reverted: included with a receipt status other than 1 (stays queued for resending).
    Reverted,
    /// Not sent: refused before broadcast (stays queued for resending).
    Refused,
    /// Not on chain: recorded, not queued, not anchored.
    Landed,
    /// Deleted, not anchored: the deleted entry was never published.
    Deleted,
    /// A retraction of an unpublished entry; it stays local too and is not anchored.
    LocalDeletion,
    /// Anchored according to the last audit's cached `anchored` set; the current audit has not arrived yet.
    /// Anything that requires anchoring still accepts only [`Lamp::Anchored`].
    Remembered,
    /// As above, with a stale cache (`lastread::STALE_SECS`): shown grey, not yellow.
    RememberedStale,
    /// Someone else's ledger only: no anchor was found on the chains read, but a network was skipped where it
    /// may be anchored. Never shown as "not on chain", and never used for this home's own ledger.
    ChainUnread,
}

impl Lamp {
    pub const ALL: [Lamp; 12] = [
        Lamp::Anchored,
        Lamp::Included,
        Lamp::Submitted,
        Lamp::Queued,
        Lamp::Reverted,
        Lamp::Refused,
        Lamp::Landed,
        Lamp::Deleted,
        Lamp::LocalDeletion,
        Lamp::Remembered,
        Lamp::RememberedStale,
        Lamp::ChainUnread,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Lamp::Anchored => "anchored",
            Lamp::Included => "included",
            Lamp::Submitted => "submitted",
            Lamp::Queued => "queued",
            Lamp::Reverted => "reverted",
            Lamp::Refused => "refused",
            Lamp::Landed => "landed",
            Lamp::Deleted => "deleted",
            Lamp::LocalDeletion => "local-deletion",
            Lamp::Remembered => "remembered",
            Lamp::RememberedStale => "remembered-stale",
            Lamp::ChainUnread => "chain-unread",
        }
    }

    /// Whether it is confirmed on chain: `Anchored`, `Remembered` or `RememberedStale`. `Included` (receipt
    /// seen, not yet audited) is not. Used by the status bar and the export page.
    pub fn confirmed(self) -> bool {
        matches!(self, Lamp::Anchored | Lamp::Remembered | Lamp::RememberedStale)
    }

    /// Whether it is in the anchoring queue (queued, submitted, reverted or refused).
    pub fn in_queue(self) -> bool {
        matches!(self, Lamp::Queued | Lamp::Submitted | Lamp::Reverted | Lamp::Refused)
    }

    /// Whether it stays local and is never anchored (either side of a local deletion). Density and watch do
    /// not count it as needing an anchor.
    pub fn local(self) -> bool {
        matches!(self, Lamp::Deleted | Lamp::LocalDeletion)
    }
}

/// One table row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub seq: u64,
    pub kind: EntryType,
    /// Entry id (hex32).
    pub id: String,
    pub prev: Option<String>,
    pub author: String,
    /// One-line summary of the body.
    pub summary: String,
    pub lamp: Lamp,
    /// When anchored, which transaction anchored it (chain id and tx hash).
    pub tx: Option<(u64, String)>,
    /// Canonical byte length.
    pub bytes: usize,
    /// The full `content` of a history entry (hex32). The summary shows only a shortened form, which would
    /// fail `is_hex32` if used as input; code must use this field instead.
    pub work: Option<String>,
    /// The raw values behind the summary, from which the UI builds its text; `summary` is for table output.
    pub facts: Facts,
    /// Block time of the first anchor (Unix seconds), from the chain data of the audit, other-ledger read or
    /// verification that produced this row. `None` until the chain has been read.
    pub anchored_at: Option<u64>,
}

/// Display values taken unchanged from an entry body's members; empty when absent.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    /// The first line of `note_md` or `statement_md`.
    pub note: Option<String>,
    /// The mode mark of a history entry.
    pub mark: Option<String>,
    /// The grantee of a grant.
    pub grantee: Option<String>,
    /// The work a grant covers (its `work`, the content of the record granted).
    pub work: Option<String>,
    /// The window of a grant (from, to).
    pub window: Option<(u64, u64)>,
    /// The grant a revocation cites; the entry an annotation annotates.
    pub subject: Option<String>,
    /// The anchor row count of an adoption.
    pub count: Option<usize>,
    /// Whether an adoption is cosigned.
    pub cosigned: bool,
    /// The `to` of a succession.
    pub to: Option<String>,
    /// Whether a retraction entry (`retractx` convention) has a well-formed body. Always false for other types.
    pub shape_ok: bool,
    /// The `entryType` as written (the only way to identify types outside the seven known ones).
    pub raw_type: String,
}

/// Extract an entry body's display values.
pub fn facts(kind: EntryType, body: &Value) -> Facts {
    let note = |k: &str| text(body, k).map(|s| one_line(s, 48)).filter(|s| !s.is_empty());
    let mut f = Facts::default();
    match kind {
        EntryType::Genesis => f.note = note("statement_md"),
        EntryType::History => {
            f.note = note(crate::entryx::NOTE_MD);
            f.mark = member(body, "mode").and_then(|m| text(m, "mark").map(|x| x.to_string()));
        }
        EntryType::Grant => {
            f.grantee = text(body, "grantee").map(|s| s.to_string());
            f.work = text(body, "work").map(|s| s.to_string());
            f.window = member(body, "window").and_then(|w| Some((int(w, "from")?, int(w, "to")?)));
        }
        EntryType::Revocation => f.subject = text(body, "grant").map(|s| s.to_string()),
        EntryType::Adoption => {
            f.count = Some(match member(body, "anchors") {
                Some(Value::Arr(a)) => a.len(),
                _ => 0,
            });
            f.cosigned = member(body, "attestation").is_some();
        }
        EntryType::Succession => f.to = text(body, "to").map(|s| s.to_string()),
        EntryType::Annotation => {
            f.note = note(crate::entryx::NOTE_MD);
            f.subject = text(body, "subject").map(|s| s.to_string());
        }
        EntryType::Other => {}
    }
    f
}

/// As [`facts`], plus the retraction convention (`retractx`) for unlisted types. Used for both this ledger
/// and others' ledgers.
pub fn facts_of(e: &zikaron::entry::Entry) -> Facts {
    let mut f = facts(e.kind, &e.body);
    // Retraction fields come only from `zikaron_glue::retraction::Line::of`, shared with the command line, so
    // the two never disagree on a malformed retraction.
    let line = zikaron_glue::retraction::Line::of(e);
    f.raw_type = line.raw_type;
    if zikaron_glue::retraction::is_retraction(e.kind, &e.entry_type) {
        f.subject = line.subject;
        f.shape_ok = line.shape_ok;
        f.note = text(&e.body, crate::entryx::NOTE_MD).map(|s| one_line(s, 48)).filter(|s| !s.is_empty());
    }
    f
}

fn member<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).map(|(_, x)| x),
        _ => None,
    }
}

fn text<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    match member(v, k) {
        Some(Value::Str(s)) => Some(s.as_str()),
        _ => None,
    }
}

fn int(v: &Value, k: &str) -> Option<u64> {
    match member(v, k) {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    }
}

/// A short form of a hex string (the first twelve characters, `0x` included). For display only.
///
/// Cuts characters, not bytes: body strings can hold any code point, and slicing inside a multi-byte
/// character would panic and crash the background task.
pub fn short(h: &str) -> String {
    if h.chars().count() <= 12 {
        return h.to_string();
    }
    format!("{}…", h.chars().take(12).collect::<String>())
}

/// The record (content digest, hex32) a history entry records, if any. Use this, not the summary.
pub fn work_of(kind: EntryType, body: &Value) -> Option<String> {
    if kind != EntryType::History {
        return None;
    }
    text(body, "content").map(|s| s.to_string())
}

/// A one-line summary from the body members of each entry type; "—" for missing members.
pub fn summarize(kind: EntryType, body: &Value) -> String {
    let missing = "—".to_string();
    match kind {
        // §6.1: the statement's first line.
        EntryType::Genesis => text(body, "statement_md")
            .map(|s| one_line(s, 60))
            .unwrap_or(missing),
        // §6.2: short content hash plus mode mark.
        EntryType::History => {
            let c = text(body, "content").map(short).unwrap_or_else(|| "—".into());
            let mark = member(body, "mode")
                .and_then(|m| text(m, "mark").map(|x| x.to_string()))
                .unwrap_or_else(|| "—".into());
            format!("{c} · {mark}")
        }
        // §6.3: short grantee address plus window.
        EntryType::Grant => {
            let g = text(body, "grantee").map(short).unwrap_or_else(|| "—".into());
            let w = match member(body, "window") {
                Some(w) => match (int(w, "from"), int(w, "to")) {
                    (Some(a), Some(b)) => format!("{a}–{b}"),
                    _ => "—".to_string(),
                },
                None => "∞".to_string(),
            };
            format!("{g} · {w}")
        }
        // §6.4: the grant cited.
        EntryType::Revocation => {
            let g = text(body, "grant").map(short).unwrap_or_else(|| "—".into());
            match text(body, "case") {
                Some(c) => format!("{g} · {}", short(c)),
                None => g,
            }
        }
        // §6.5: anchor row count.
        EntryType::Adoption => {
            let n = match member(body, "anchors") {
                Some(Value::Arr(a)) => a.len(),
                _ => 0,
            };
            let signed = member(body, "attestation").is_some();
            format!("{n} · {}", if signed { "cosigned" } else { "—" })
        }
        // §6.7: kind and successor.
        EntryType::Succession => {
            let k = text(body, "kind").unwrap_or("—");
            let to = text(body, "to").map(short).unwrap_or_else(|| "—".into());
            format!("{k} · {to}")
        }
        // §6.8: the entry annotated (or the note, for the whole ledger).
        EntryType::Annotation => match text(body, "subject") {
            Some(s) => short(s),
            None => text(body, crate::entryx::NOTE_MD).map(|s| one_line(s, 48)).unwrap_or(missing),
        },
        EntryType::Other => missing,
    }
}

fn one_line(s: &str, cap: usize) -> String {
    let one: String = s.lines().next().unwrap_or("").trim().to_string();
    if one.chars().count() <= cap {
        return one;
    }
    let cut: String = one.chars().take(cap).collect();
    format!("{cut}…")
}

/// The anchored set from the audit report's `anchored` item: entry id to (chain id, tx).
pub fn anchored_of(report: &Value) -> Vec<(String, (u64, String))> {
    let mut out = Vec::new();
    let Some(Value::Arr(rows)) = member(report, zikaron::tokens::Key::Anchored.as_str()) else {
        return out;
    };
    for r in rows {
        let Some(id) = text(r, zikaron::tokens::Key::EntryId.as_str()) else { continue };
        let Some(Value::Arr(anchors)) = member(r, zikaron::tokens::Key::Anchors.as_str()) else {
            continue;
        };
        let Some(first) = anchors.first() else { continue };
        let chain = int(first, zikaron::tokens::Key::ChainId.as_str()).unwrap_or(0);
        let tx = text(first, zikaron::tokens::Key::Tx.as_str()).unwrap_or("").to_string();
        out.push((id.to_string(), (chain, tx)));
    }
    out
}

/// Set each row's first-anchor block time from chain data (see [`crate::auditx::first_anchored`]); chain
/// data that does not audit leaves the rows unchanged.
pub fn stamp(rows: &mut [Row], items: &[Vec<u8>], fragment: &Value) {
    let Some(at) = crate::auditx::first_anchored(items, fragment) else { return };
    for r in rows.iter_mut() {
        r.anchored_at = at.iter().find(|(h, _)| h.eq_ignore_ascii_case(&r.id)).map(|(_, t)| *t);
    }
}

/// A ledger table.
pub struct Table {
    pub rows: Vec<Row>,
    /// How many files could not be read as entries (listed in `skipped` by the store's lenient read).
    pub strays: usize,
}

/// Read a table. `report` is the last audit report (`None`: no entry shows as anchored); `queued` are the ids
/// in the local queue, all shown as queued (use [`table_with`] for full queue states).
pub fn table(home: &Home, report: Option<&Value>, queued: &[String]) -> Result<Table, Fault> {
    let q = crate::queue::Queue {
        items: queued
            .iter()
            .map(|id| crate::queue::Queued { id: id.clone(), at: 0, step: crate::queue::Step::Queued })
            .collect(),
        ..crate::queue::Queue::default()
    };
    table_with(home, report, &q)
}

/// Read a table with lamps from the queue file, without the cached audit set.
pub fn table_with(home: &Home, report: Option<&Value>, queue: &crate::queue::Queue) -> Result<Table, Fault> {
    table_remembering(home, report, queue, None, 0)
}

/// As above, with the cached audit set; the shell reads the table through this. In the report means
/// `Anchored`; only in the cached set means `Remembered`; otherwise the lamp comes from the queue file.
pub fn table_remembering(
    home: &Home,
    report: Option<&Value>,
    queue: &crate::queue::Queue,
    remembered: Option<&crate::lastread::Anchored>,
    now: u64,
) -> Result<Table, Fault> {
    let stale = remembered.map(|r| crate::lastread::stale(r.at, now)).unwrap_or(false);
    let ledger = home.ledger()?;
    let survey = ledger
        .survey()?;
    let anchored = report.map(anchored_of).unwrap_or_default();
    let mut rows: Vec<Row> = Vec::new();
    for b in &survey.items {
        // Non-entries stay off the table; they are counted in `strays`.
        let Ok(e) = zikaron::entry::check(b) else { continue };
        let id = zikaron::hexfmt::encode(&e.id);
        let tx = anchored.iter().find(|(h, _)| *h == id).map(|(_, t)| t.clone());
        let recalled = if tx.is_none() { remembered.and_then(|r| r.rows.iter().find(|(h, _)| h.eq_ignore_ascii_case(&id)).map(|(_, t)| t.clone())) } else { None };
        let lamp = match (tx.is_some(), recalled.is_some()) {
            (false, true) if stale => Lamp::RememberedStale,
            (false, true) => Lamp::Remembered,
            _ => lamp_of(&id, tx.is_some(), queue),
        };
        let tx = tx.or(recalled);
        // In-flight entries (included, submitted, reverted) take their transaction from the queue file.
        let tx = tx.or_else(|| queue.block_of(&id).map(|b| (b.chain, b.tx.clone()))).or_else(|| match queue.step_of(&id) {
            Some(crate::queue::Step::Submitted { tx, chain, .. }) | Some(crate::queue::Step::Reverted { tx, chain }) => Some((*chain, tx.clone())),
            // Resent: the latest transaction (the one with the current fees).
            Some(crate::queue::Step::Resent { txs, chain, .. }) => txs.last().map(|t| (*chain, t.clone())),
            _ => None,
        });
        rows.push(Row {
            seq: e.seq,
            kind: e.kind,
            summary: summarize(e.kind, &e.body),
            work: work_of(e.kind, &e.body),
            facts: facts_of(&e),
            id,
            prev: e.prev.clone(),
            author: e.author.clone(),
            lamp,
            tx,
            bytes: b.len(),
            anchored_at: None,
        });
    }
    // Local deletion: when the deleted entry was never published, it and its retraction stay local. The rule
    // is `queue::Queue::published`; pairs come from `retractx`. Entries in the cached audit set count as
    // published, so they do not show as local-only while the report is pending.
    let report_ids: Vec<String> = anchored
        .iter()
        .map(|(h, _)| h.clone())
        .chain(remembered.map(|r| r.rows.iter().map(|(h, _)| h.clone()).collect::<Vec<_>>()).unwrap_or_default())
        .collect();
    let pairs = crate::retractx::pairs(&rows);
    for (retraction, subject) in pairs {
        if queue.published(&subject, &report_ids) {
            continue;
        }
        for r in rows.iter_mut() {
            if r.id == subject && !matches!(r.lamp, Lamp::Anchored | Lamp::Remembered | Lamp::RememberedStale | Lamp::Included | Lamp::Submitted) {
                r.lamp = Lamp::Deleted;
            }
            if r.id == retraction && matches!(r.lamp, Lamp::Landed) {
                r.lamp = Lamp::LocalDeletion;
            }
        }
    }
    // Descending seq, ties by id, so the order is stable.
    rows.sort_by(|a, b| (b.seq, &a.id).cmp(&(a.seq, &b.id)));
    Ok(Table { rows, strays: survey.skipped.len() })
}

/// One entry's lamp: `Anchored` when in the report; otherwise from the queue file; `Landed` (not on chain)
/// when neither.
pub fn lamp_of(id: &str, in_report: bool, queue: &crate::queue::Queue) -> Lamp {
    if in_report {
        return Lamp::Anchored;
    }
    if queue.block_of(id).is_some() || queue.anchored_here(id) {
        return Lamp::Included;
    }
    match queue.step_of(id) {
        Some(crate::queue::Step::Queued) => Lamp::Queued,
        // Resent is still waiting for a receipt, like submitted.
        Some(crate::queue::Step::Submitted { .. }) | Some(crate::queue::Step::Resent { .. }) => Lamp::Submitted,
        Some(crate::queue::Step::Reverted { .. }) => Lamp::Reverted,
        Some(crate::queue::Step::Refused { .. }) => Lamp::Refused,
        None => Lamp::Landed,
    }
}

/// One entry's details. The bytes are read from disk on demand, never kept in the table.
#[derive(Clone, Debug)]
pub struct Detail {
    pub id: String,
    pub bytes: Vec<u8>,
    pub prev: Option<String>,
    pub seq: u64,
    pub author: String,
    pub kind: EntryType,
    pub body: Value,
}

/// Read one entry's details. The file name comes from the store crate's name constructor.
pub fn detail(home: &Home, id: &str) -> Result<Detail, Fault> {
    let ledger = home.ledger()?;
    let bare = id.trim().trim_start_matches("0x");
    let name = zikaron_store::EntryName::parse(bare)
        .ok_or_else(|| Fault::known(Known::ContentShape, id.to_string()))?;
    let file = zikaron_store::layout::entry_file_name(&name);
    let bytes = ledger
        .read_named(&file)?;
    let e = zikaron::entry::check(&bytes)
        .map_err(Fault::entry_refused)?;
    Ok(Detail {
        id: zikaron::hexfmt::encode(&e.id),
        prev: e.prev.clone(),
        seq: e.seq,
        author: e.author.clone(),
        kind: e.kind,
        body: e.body.clone(),
        bytes,
    })
}

/// The ledger's current head: the highest seq and its entry id. `None` for an empty ledger (genesis comes
/// next).
pub fn head(home: &Home) -> Result<Option<(u64, String)>, Fault> {
    let ledger = home.ledger()?;
    let survey = ledger
        .survey()?;
    // Fail closed on any unreadable item: it might be the last entry, and ignoring it would write a second
    // entry at the same seq (EQUIVOCATION, law §8.4, which a ledger cannot undo). An empty entry file
    // is reported as such (`local::EMPTY`, as the strict read does).
    let empty = crate::local::empty_in(&survey.skipped);
    if !empty.is_empty() {
        return Err(crate::local::empty_fault(&empty));
    }
    if !survey.skipped.is_empty() {
        return Err(Fault::known(
            Known::Ledger,
            crate::lang::filln(crate::lang::Key::Tail182, &[&(survey.skipped.len()).to_string()]),
        ));
    }
    let mut best: Option<(u64, String)> = None;
    for b in &survey.items {
        let Ok(e) = zikaron::entry::check(b) else {
            return Err(Fault::known(Known::Ledger, crate::lang::t(crate::lang::Key::Tail183).to_string()));
        };
        let id = zikaron::hexfmt::encode(&e.id);
        best = match best {
            None => Some((e.seq, id)),
            Some((s, ref h)) if (e.seq, &id) > (s, h) => Some((e.seq, id)),
            other => other,
        };
    }
    Ok(best)
}

/// Whether the genesis entry (`seq` 0) is confirmed on chain ([`Lamp::confirmed`]), for the ledger status
/// bar. `None` without a genesis in the table.
pub fn root_anchored(rows: &[Row]) -> Option<bool> {
    rows.iter().find(|r| r.seq == 0 && r.kind == EntryType::Genesis).map(|r| r.lamp.confirmed())
}

/// This ledger's root: the id of the genesis entry (`seq` 0), used as the kit index's `root`. A ledger
/// without genesis is an error.
pub fn root_of(home: &Home) -> Result<String, Fault> {
    let pile = home.ledger()?.pile()?;
    pile.items
        .iter()
        .filter_map(|b| zikaron::entry::check(b).ok())
        .find(|e| e.seq == 0)
        .map(|e| e.id_hex())
        .ok_or_else(|| Fault::known(Known::Ledger, home.root().display().to_string()))
}
