//! Reading the ledger view: turn bytes into a table in descending seq order.
//!
//! No law decision is made here: whether bytes are an entry, which of the seven types, its seq and prev are
//! all answered by the core's `entry::check`; whether the chain is broken by the label of the core's `audit`,
//! never by comparing prev here. The summary reads the body members law §6.2 to §6.8 name, and says "unread"
//! when it cannot read them instead of filling a pleasant default.
//!
//! Anchor lamp colors each have a source: green (anchored) only from the audited report's `anchored` item,
//! when the entry's id is in the counted anchor set; grey (queued) from the local queue file; yellow for
//! recorded but not anchored. Before any audit, green never appears: "not asked" and "not on chain" look
//! different.

use crate::fault::{Fault, Known};
use crate::home::Home;
use zikaron::json::Value;
use zikaron::tokens::EntryType;

/// Anchor lamp. Closed (entry cards, record cards, the queue page and watch rows all read this one).
///
/// Green (anchored) comes only from the audited report; without an audit it never appears. The other members
/// come from the local queue file (`queue::Step` with its `blocks` and `anchored` books) and the ledger's
/// retraction reading.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lamp {
    /// Confirmed: the core's report says it is anchored.
    Anchored,
    /// Confirming: the receipt says it was included (recorded in the queue file); the report has not counted
    /// it yet.
    Included,
    /// Waiting to be anchored: broadcast and echoed, waiting for the receipt.
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
    /// Local deletion: it deletes an unpublished entry, and this retraction stays local too, not anchored.
    LocalDeletion,
    /// Checked last time: this pass's report has not arrived, and the last audit's `anchored` set (disk
    /// cache) has it. It is not "anchored": places that require anchored still accept only
    /// [`Lamp::Anchored`].
    Remembered,
    /// As above, with that set stale (`lastread::STALE_SECS`): the lamp turns grey, not yellow.
    RememberedStale,
}

impl Lamp {
    pub const ALL: [Lamp; 11] = [
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
        }
    }

    /// Whether it is confirmed on chain; the one classification. Counted by the report (`Anchored`) or
    /// recorded in the last audit's set (`Remembered`, `RememberedStale`) is confirmed; included by receipt
    /// but not yet counted (`Included`) is not, nor are queued, submitted, reverted, refused, unqueued or
    /// either deletion. The status bar and the export page's red lamps both read this.
    pub fn confirmed(self) -> bool {
        matches!(self, Lamp::Anchored | Lamp::Remembered | Lamp::RememberedStale)
    }

    /// Whether it is queued (to be anchored, waiting, reverted, not sent).
    pub fn in_queue(self) -> bool {
        matches!(self, Lamp::Queued | Lamp::Submitted | Lamp::Reverted | Lamp::Refused)
    }

    /// Whether it stays local and is not anchored (the local deletion pair). Density and watch do not count
    /// it as owing an anchor.
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
    /// Summary: what the body of this type contains, read out.
    pub summary: String,
    pub lamp: Lamp,
    /// When anchored, which transaction anchored it (chain id and tx hash).
    pub tx: Option<(u64, String)>,
    /// Canonical byte length.
    pub bytes: usize,
    /// The `content` of a history entry (hex32), unchanged.
    ///
    /// The summary shows its first twelve characters for the eyes; feeding the display string back as input
    /// fails `is_hex32`, so a page doing that would never compute. The source and the display string live
    /// apart, which is why this field exists.
    pub work: Option<String>,
    /// The summary's raw material, unchanged: the interface builds plain words from it; the summary string
    /// itself is for table output.
    pub facts: Facts,
    /// First-anchor block time (Unix seconds), from the chain fragment of the audit, others'-ledger
    /// read or verification this row came out of. `None` until a pass has read the chain.
    pub anchored_at: Option<u64>,
}

/// What a body shows the eye, taken unchanged from the members law §6 names; empty when absent.
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
    /// Whether a local-convention entry (`retractx`) has a body that fits the convention. Always false for
    /// other types; reading does not look at it.
    pub shape_ok: bool,
    /// The entry type as written (`entryType`; types outside the seven are recognizable only here).
    pub raw_type: String,
}

/// Take an entry's raw material.
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

/// Take an entry's raw material, including the local reading convention (`retractx`) among unlisted types.
/// One source for this ledger and others' ledgers.
pub fn facts_of(e: &zikaron::entry::Entry) -> Facts {
    let mut f = facts(e.kind, &e.body);
    // The retraction fields come only from the convention table: `zikaron_glue::retraction::Line::of` is
    // shared by the command line and this table. Filling `subject` and `shape_ok`
    // separately here would let the two readers disagree on a malformed retraction.
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

/// A short form of a hex string (the first twelve characters, `0x` included). For the eyes only; nothing
/// decides by it.
///
/// It cuts characters, not bytes: strings from the summary come from the body, which can hold any code point,
/// and a byte cut inside a multi-byte character panics (the whole background task would crash).
pub fn short(h: &str) -> String {
    if h.chars().count() <= 12 {
        return h.to_string();
    }
    format!("{}…", h.chars().take(12).collect::<String>())
}

/// Which record this entry records (hex32), if any. Decisions take this, not the summary field.
pub fn work_of(kind: EntryType, body: &Value) -> Option<String> {
    if kind != EntryType::History {
        return None;
    }
    text(body, "content").map(|s| s.to_string())
}

/// One summary sentence. It reads the body members the law names; when it cannot read them, it says so.
pub fn summarize(kind: EntryType, body: &Value) -> String {
    let missing = "—".to_string();
    match kind {
        // §6.1: a prose sentence.
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
        // §6.7: kind and to.
        EntryType::Succession => {
            let k = text(body, "kind").unwrap_or("—");
            let to = text(body, "to").map(short).unwrap_or_else(|| "—".into());
            format!("{k} · {to}")
        }
        // §6.8: the entry annotated (or the whole ledger).
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

/// The anchored set, from the `anchored` item of the audited report: id to (chain id, tx).
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

/// **Stamp the first-anchor block time** on each row from a chain fragment (see
/// [`crate::auditx::first_anchored`]); a fragment that does not audit leaves the rows as they are.
pub fn stamp(rows: &mut [Row], items: &[Vec<u8>], fragment: &Value) {
    let Some(at) = crate::auditx::first_anchored(items, fragment) else { return };
    for r in rows.iter_mut() {
        r.anchored_at = at.iter().find(|(h, _)| h.eq_ignore_ascii_case(&r.id)).map(|(_, t)| *t);
    }
}

/// One table reading.
pub struct Table {
    pub rows: Vec<Row>,
    /// How many stray files could not be read as entries (the storage crate's lenient read lists them in
    /// `skipped`).
    pub strays: usize,
}

/// Read a table. `report` is the last audit's report (`None` without one: then no green lamp); `queued` are
/// the ids in the local queue (all read as queued; for the full states use [`table_with`]).
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

/// Read a table, lamps computed from the queue file. Without the last pass's cache (readings that do not
/// need it use this).
pub fn table_with(home: &Home, report: Option<&Value>, queue: &crate::queue::Queue) -> Result<Table, Fault> {
    table_remembering(home, report, queue, None, 0)
}

/// As above, with the last pass's set. The one source: the shell reads the table here. In this report means
/// anchored; missing from it but in the last pass's set (disk cache) means checked last time; neither, from
/// the queue file.
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
        // Bytes that are not entries stay off the table: they are strays, counted in `strays`.
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
        // Included but not counted yet, submitted and reverted entries take their transaction from the queue
        // file (so an entry in flight shows its tx hash).
        let tx = tx.or_else(|| queue.block_of(&id).map(|b| (b.chain, b.tx.clone()))).or_else(|| match queue.step_of(&id) {
            Some(crate::queue::Step::Submitted { tx, chain }) | Some(crate::queue::Step::Reverted { tx, chain }) => Some((*chain, tx.clone())),
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
    // The local deletion pair: the deleted entry was never published, so it and its retraction stay local.
    // The rule lives in `queue::Queue::published`; reading lives in `retractx` (the same rows the deletion
    // path reads). Entries remembered as anchored by the last pass (cache) count as published
    // too, so they do not read as local-only while the report is pending.
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
    // Descending seq; entries with the same seq ordered by id, so the order repeats every pass.
    rows.sort_by(|a, b| (b.seq, &a.id).cmp(&(a.seq, &b.id)));
    Ok(Table { rows, strays: survey.skipped.len() })
}

/// One entry's lamp: green when the report says anchored; otherwise from the queue file (included, which
/// step); neither means not on chain.
pub fn lamp_of(id: &str, in_report: bool, queue: &crate::queue::Queue) -> Lamp {
    if in_report {
        return Lamp::Anchored;
    }
    if queue.block_of(id).is_some() || queue.anchored_here(id) {
        return Lamp::Included;
    }
    match queue.step_of(id) {
        Some(crate::queue::Step::Queued) => Lamp::Queued,
        Some(crate::queue::Step::Submitted { .. }) => Lamp::Submitted,
        Some(crate::queue::Step::Reverted { .. }) => Lamp::Reverted,
        Some(crate::queue::Step::Refused { .. }) => Lamp::Refused,
        None => Lamp::Landed,
    }
}

/// One entry's details: canonical bytes, id, prev, anchor transaction. Bytes are read from disk now, never
/// kept in the table.
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

/// Read one entry's details. The file name comes from the storage crate's own name constructor.
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

/// This ledger's head now: the highest seq and its entry id. The next entry follows it. `None` for an empty
/// ledger (then genesis is what to write).
pub fn head(home: &Home) -> Result<Option<(u64, String)>, Fault> {
    let ledger = home.ledger()?;
    let survey = ledger
        .survey()?;
    // No unreadable item may exist. New entries follow the ledger's last entry, and an unreadable item might
    // be that entry; treating it as absent would write a second entry at the same seq (EQUIVOCATION, law
    // §8.4, and a ledger has no way to delete). So this fails closed.
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

/// Whether the root is confirmed on chain: the genesis entry's (`seq` 0) lamp, read now; the ledger status
/// bar reads only this. Confirmation is decided by [`Lamp::confirmed`] (the export page reads the same).
/// `None` without a genesis in the table (the status bar says nothing).
pub fn root_anchored(rows: &[Row]) -> Option<bool> {
    rows.iter().find(|r| r.seq == 0 && r.kind == EntryType::Genesis).map(|r| r.lamp.confirmed())
}

/// This ledger's root: the id of the genesis entry (`seq` 0). The kit index's `root` field reads it. A ledger
/// without genesis is refused by name.
pub fn root_of(home: &Home) -> Result<String, Fault> {
    let pile = home.ledger()?.pile()?;
    pile.items
        .iter()
        .filter_map(|b| zikaron::entry::check(b).ok())
        .find(|e| e.seq == 0)
        .map(|e| e.id_hex())
        .ok_or_else(|| Fault::known(Known::Ledger, home.root().display().to_string()))
}
