//! Ledger mirror layout, shared by the app (which writes and restores mirrors) and the CLI (which reads them).
//! A bundle is a directory with a manifest ([`MANIFEST`]) and an entries folder ([`ENTRIES`]) whose files are
//! named by entry id. This module names the layout and reads the manifest; whether a file is a valid entry
//! matching its name is for the core to decide.

use zikaron::json::{self, Value};

/// Manifest file name.
pub const MANIFEST: &str = "mirror.json";
/// The entries folder inside a bundle.
pub const ENTRIES: &str = "entries";
/// Bundle kind name. A product format, not a spec domain, so it has no `zikaron.` prefix.
pub const KIND: &str = "desk-mirror";
/// The manifest's shape version.
pub const VERSION: u64 = 1;

/// One manifest row: a name (an entry id, or a path for the vault folder), its digest and size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
}

/// A parsed manifest, member by member (absent members read as empty).
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
    /// An unreadable bundle file (its path).
    Unreadable(String),
    /// The manifest is not JSON (the parser's token).
    NotJson(String),
    /// The manifest is not this kind or shape version (`kind/version` as written).
    NotThisKind(String),
    /// A row's name is not an entry name (64 lowercase hex digits), e.g. a path, `..`, an absolute name, a
    /// separator or empty (the name as written). Nothing outside the entries folder is ever read.
    NotAnEntryName(String),
}

/// The entries a bundle lists, read from its entries folder in manifest order. Only valid entry names are read;
/// validating the bytes is for the core.
pub fn entries(dir: &std::path::Path) -> Result<Vec<Vec<u8>>, ReadTrouble> {
    crate::seam_v2();
    let m = dir.join(MANIFEST);
    let bytes = std::fs::read(&m).map_err(|_| ReadTrouble::Unreadable(m.display().to_string()))?;
    let s = sheet(&bytes).map_err(|t| ReadTrouble::NotJson(format!("{t:?}")))?;
    if s.kind != KIND || s.version != VERSION {
        return Err(ReadTrouble::NotThisKind(format!("{}/{}", s.kind, s.version)));
    }
    let room = dir.join(ENTRIES);
    // Check every name before reading any file; a row naming anything but an entry is refused by name.
    if let Some(r) = s.rows.iter().find(|r| zikaron_store::layout::EntryName::parse(&r.name).is_none()) {
        return Err(ReadTrouble::NotAnEntryName(r.name.clone()));
    }
    let mut items = Vec::new();
    for r in &s.rows {
        let p = room.join(&r.name);
        items.push(std::fs::read(&p).map_err(|_| ReadTrouble::Unreadable(p.display().to_string()))?);
    }
    Ok(items)
}


