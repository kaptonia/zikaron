//! The ledger mirror's layout, read in one place by the app (which writes and restores mirrors) and the
//! command line (which reads them): a bundle is a directory with a manifest ([`MANIFEST`]) and an entries room
//! ([`ENTRIES`]) whose files are named by entry id. This module names the layout and reads the manifest; it
//! decides nothing: whether an entry is an entry, and whether its id is its name, is the core's.

use zikaron::json::{self, Value};

/// Manifest file name.
pub const MANIFEST: &str = "mirror.json";
/// The entries room inside a bundle.
pub const ENTRIES: &str = "entries";
/// Bundle kind name. A product name, not a law domain, so it has no `zikaron.` prefix.
pub const KIND: &str = "desk-mirror";
/// The manifest's shape version.
pub const VERSION: u64 = 1;

/// One row of a manifest: a name (an entry id, or a path for the vault room), its digest and size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
}

/// A manifest as written: what it says, member by member (absent members read as empty).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sheet {
    pub kind: String,
    pub version: u64,
    pub root: String,
    pub owner: String,
    pub rows: Vec<Row>,
    pub held: Vec<Row>,
}

fn field<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).map(|(_, x)| x),
        _ => None,
    }
}

fn text(v: &Value, k: &str) -> String {
    match field(v, k) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

fn int(v: &Value, k: &str) -> u64 {
    match field(v, k) {
        Some(Value::Int(n)) => *n,
        _ => 0,
    }
}

fn rows(v: &Value, list: &str, name: &str) -> Vec<Row> {
    match field(v, list) {
        Some(Value::Arr(a)) => a.iter().map(|r| Row { name: text(r, name), sha256: text(r, "sha256"), bytes: int(r, "bytes") }).collect(),
        _ => Vec::new(),
    }
}

/// Read a manifest's bytes. Bytes that are not JSON give the parser's token.
pub fn sheet(bytes: &[u8]) -> Result<Sheet, zikaron::tokens::Token> {
    crate::seam_v2();
    let v = json::parse(bytes)?;
    Ok(Sheet {
        kind: text(&v, "kind"),
        version: int(&v, "version"),
        root: text(&v, "root"),
        owner: text(&v, "owner"),
        rows: rows(&v, "entries", "name"),
        held: rows(&v, "held", "path"),
    })
}

/// Whether a directory is a mirror bundle (its manifest is there).
pub fn is_bundle(dir: &std::path::Path) -> bool {
    dir.join(MANIFEST).is_file()
}

/// Why a bundle's entries could not be read.
#[derive(Debug)]
pub enum ReadTrouble {
    /// A file of the bundle that does not read (its path).
    Unreadable(String),
    /// The manifest is not JSON (the parser's token, spelled).
    NotJson(String),
    /// The manifest is not this kind or shape version (`kind/version` as written).
    NotThisKind(String),
}

/// The entries a bundle lists, read from its entries room in the manifest's order. Whether each is an entry is
/// the core's; this reads bytes by the layout's names.
pub fn entries(dir: &std::path::Path) -> Result<Vec<Vec<u8>>, ReadTrouble> {
    crate::seam_v2();
    let m = dir.join(MANIFEST);
    let bytes = std::fs::read(&m).map_err(|_| ReadTrouble::Unreadable(m.display().to_string()))?;
    let s = sheet(&bytes).map_err(|t| ReadTrouble::NotJson(format!("{t:?}")))?;
    if s.kind != KIND || s.version != VERSION {
        return Err(ReadTrouble::NotThisKind(format!("{}/{}", s.kind, s.version)));
    }
    let room = dir.join(ENTRIES);
    let mut items = Vec::new();
    for r in &s.rows {
        let p = room.join(&r.name);
        items.push(std::fs::read(&p).map_err(|_| ReadTrouble::Unreadable(p.display().to_string()))?);
    }
    Ok(items)
}

