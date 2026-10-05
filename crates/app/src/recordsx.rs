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

