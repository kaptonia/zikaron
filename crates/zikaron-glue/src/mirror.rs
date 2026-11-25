use zikaron::json::{self, Value};

pub const MANIFEST: &str = "mirror.json";
pub const ENTRIES: &str = "entries";
pub const KIND: &str = "desk-mirror";
pub const VERSION: u64 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
}

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

pub fn is_bundle(dir: &std::path::Path) -> bool {
    dir.join(MANIFEST).is_file()
}

#[derive(Debug)]
pub enum ReadTrouble {
    Unreadable(String),
    NotJson(String),
    NotThisKind(String),
    NotAnEntryName(String),
}

pub fn entries(dir: &std::path::Path) -> Result<Vec<Vec<u8>>, ReadTrouble> {
    crate::seam_v2();
    let m = dir.join(MANIFEST);
    let bytes = std::fs::read(&m).map_err(|_| ReadTrouble::Unreadable(m.display().to_string()))?;
    let s = sheet(&bytes).map_err(|t| ReadTrouble::NotJson(format!("{t:?}")))?;
    if s.kind != KIND || s.version != VERSION {
        return Err(ReadTrouble::NotThisKind(format!("{}/{}", s.kind, s.version)));
    }
    let room = dir.join(ENTRIES);
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


