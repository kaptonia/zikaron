//! The local index and "verify a file".
//!
//! A `history` in the ledger records only the content's digest (`content`), not which file or where; the
//! recorder asks "was this file signed, which entry is it, is it anchored". File names and paths are this
//! machine's matter, and homes may not store absolute paths, so this index lives in the machine directory:
//! `<machine directory>/records/index.json` (this desk's own; sibling apps do not read it). A row records
//! only what was fixed at signing (which ledger, digest, entry id, `seq`, file name, path); anchor state is
//! not stored but asked of this pass's audit report (stored anchor state would go stale, a silent error).
//!
//! "Verify a file" does not decide by this index: it computes the file's digest now and reads the ledger now
//! for a `history` with the same `content`; the index only adds "which file was signed back then".

use crate::fault::{Fault, Known};
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};

/// Form literal. One name, one home.
pub const FORM: &str = "zikaron-desk/records/1";
pub const DIR: &str = "records";
pub const FILE: &str = "index.json";

/// One row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Which ledger (genesis id).
    pub root: String,
    /// `content` (hex32).
    pub content: String,
    /// Entry id.
    pub id: String,
    pub seq: u64,
    /// File name (last segment).
    pub name: String,
    /// The absolute path at signing time.
    pub path: String,
}

pub fn path_in(machine: &Path) -> PathBuf {
    machine.join(DIR).join(FILE)
}

fn str_of(v: &Value, k: &str) -> Option<String> {
    match v {
        Value::Obj(m) => m.iter().find(|(x, _)| x == k).and_then(|(_, v)| if let Value::Str(s) = v { Some(s.clone()) } else { None }),
        _ => None,
    }
}

fn int_of(v: &Value, k: &str) -> Option<u64> {
    match v {
        Value::Obj(m) => m.iter().find(|(x, _)| x == k).and_then(|(_, v)| if let Value::Int(n) = v { Some(*n) } else { None }),
        _ => None,
    }
}

/// Read. No file means empty; a wrong shape is refused by name.
pub fn read(machine: &Path) -> Result<Vec<Row>, Fault> {
    let p = path_in(machine);
    let Some(bytes) = crate::local::read(&p, crate::local::Doc::Records)? else { return Ok(Vec::new()) };
    let bad = || Fault::known(Known::SettingsShape, p.display().to_string());
    let v = json::parse(&bytes).map_err(|t| Fault::known(Known::SettingsShape, format!("{}: {t:?}", p.display())))?;
    if str_of(&v, "form").as_deref() != Some(FORM) {
        return Err(bad());
    }
    let rows = match &v {
        Value::Obj(m) => m.iter().find(|(k, _)| k == "rows").map(|(_, v)| v),
        _ => None,
    };
    let Some(Value::Arr(rows)) = rows else { return Err(bad()) };
    rows.iter()
        .map(|r| {
            Some(Row {
                root: str_of(r, "root")?,
                content: str_of(r, "content")?,
                id: str_of(r, "id")?,
                seq: int_of(r, "seq")?,
                name: str_of(r, "name")?,
                path: str_of(r, "path")?,
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(bad)
}

/// Add a row (replacing an existing row with the same entry id), with an atomic rewrite.
pub fn add(machine: &Path, row: Row) -> Result<(), Fault> {
    let mut rows = read(machine)?;
    rows.retain(|r| r.id != row.id);
    rows.push(row);
    let v = Value::Obj(vec![
        ("form".to_string(), Value::Str(FORM.to_string())),
        (
            "rows".to_string(),
            Value::Arr(
                rows.iter()
                    .map(|r| {
                        Value::Obj(vec![
                            ("content".to_string(), Value::Str(r.content.clone())),
                            ("id".to_string(), Value::Str(r.id.clone())),
                            ("name".to_string(), Value::Str(r.name.clone())),
                            ("path".to_string(), Value::Str(r.path.clone())),
                            ("root".to_string(), Value::Str(r.root.clone())),
                            ("seq".to_string(), Value::Int(r.seq)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]);
    crate::local::put(&machine.join(DIR), FILE, crate::local::Doc::Records, &json::canon_bytes(&v))
}

/// The answer of "verify a file".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    /// The digest computed from the file now (hex32).
    pub content: String,
    /// The ledger entry with the same `content`: `seq`, entry id; `None` when absent ("this ledger has no
    /// such digest").
    pub found: Option<(u64, String)>,
    /// Where that entry is anchored (chain id, transaction, block time; the block time is the anchor of the
    /// same transaction in this pass's scan fragment). `None` when this pass's audit report lacks it.
    pub anchored: Option<(u64, String, Option<u64>)>,
    /// The file recorded in the local index back then (name, path); `None` when never recorded.
    pub signed_as: Option<(String, String)>,
    /// Whether the anchor comes from this pass's report or from the last pass's cache (then with the cache's
    /// time).
    pub remembered_at: Option<u64>,
    /// The answered entry was deleted by a delete entry in this ledger.
    pub deleted: bool,
}

/// Answer by computing now and reading the ledger now. `report` and `fragment` are this pass's audit (without
/// them, nothing is said about anchors).
pub fn verify(
    home: &crate::home::Home,
    machine: &Path,
    file: &Path,
    report: Option<&Value>,
    fragment: Option<&Value>,
    remembered: Option<&crate::lastread::Anchored>,
) -> Result<Verdict, Fault> {
    let c = crate::anchorx::of_file(file)?;
    let content = c.hex();
    let pile = home.ledger()?.pile()?;
    let entries: Vec<zikaron::entry::Entry> = pile.items.iter().filter_map(|b| zikaron::entry::check(b).ok()).collect();
    // The same content signed several times: sorted by seq, the anchored one first (per this pass's report,
    // otherwise the last pass's cache); with none anchored, the earliest.
    let mut matches: Vec<(u64, String)> = entries
        .iter()
        .filter(|e| e.kind == zikaron::tokens::EntryType::History && str_of(&e.body, "content").map(|x| x.eq_ignore_ascii_case(&content)).unwrap_or(false))
        .map(|e| (e.seq, e.id_hex()))
        .collect();
    matches.sort();
    let in_report = |id: &str| report.and_then(|r| crate::ledgerx::anchored_of(r).into_iter().find(|(x, _)| x.eq_ignore_ascii_case(id)).map(|(_, t)| t));
    let in_cache = |id: &str| remembered.and_then(|r| r.rows.iter().find(|(x, _)| x.eq_ignore_ascii_case(id)).map(|(_, t)| t.clone()));
    let pick = matches
        .iter()
        .find(|(_, id)| in_report(id).is_some())
        .or_else(|| matches.iter().find(|(_, id)| in_cache(id).is_some()))
        .or_else(|| matches.first())
        .cloned();
    let found = pick.clone();
    let (anchored, remembered_at) = match &pick {
        Some((_, id)) => match (in_report(id), in_cache(id)) {
            (Some((chain, tx)), _) => {
                let at = fragment.and_then(|f| block_time_of(f, &tx));
                (Some((chain, tx, at)), None)
            }
            (None, Some((chain, tx))) => (Some((chain, tx, None)), remembered.map(|r| r.at)),
            (None, None) => (None, None),
        },
        None => (None, None),
    };
    // Whether deleted: a delete entry in this ledger names it (this desk's reading convention; `retractx`
    // reads `subject` the same way).
    let deleted = pick
        .as_ref()
        .map(|(_, id)| {
            entries.iter().any(|e| {
                zikaron_glue::retraction::is_retraction(e.kind, &e.entry_type)
                    && str_of(&e.body, zikaron_glue::retraction::SUBJECT).map(|s| s.eq_ignore_ascii_case(id)).unwrap_or(false)
            })
        })
        .unwrap_or(false);
    let signed_as = match &found {
        Some((_, id)) => read(machine)?.into_iter().find(|r| r.id.eq_ignore_ascii_case(id)).map(|r| (r.name, r.path)),
        None => None,
    };
    Ok(Verdict { content, found, anchored, signed_as, remembered_at, deleted })
}

/// The block time of the anchor for the same transaction in the fragment.
fn block_time_of(fragment: &Value, tx: &str) -> Option<u64> {
    let Value::Obj(m) = fragment else { return None };
    let Some((_, Value::Arr(anchors))) = m.iter().find(|(k, _)| k == "anchors") else { return None };
    let want = tx.trim_start_matches("0x");
    anchors
        .iter()
        .find(|a| str_of(a, "tx").map(|t| t.trim_start_matches("0x").eq_ignore_ascii_case(want)).unwrap_or(false))
        .and_then(|a| int_of(a, "blockTimestamp"))
}
