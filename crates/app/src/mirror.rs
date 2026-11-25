use crate::fault::{classify, Fault, Known};
use crate::home::Home;
use zikaron::entry as k1;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron_store::EntryName;

pub use zikaron_glue::mirror::{ENTRIES, KIND, MANIFEST, VERSION};
pub const STEM: &str = "ZIKARON-backup";
pub const HELD: &str = "held";

pub struct Made {
    pub root: std::path::PathBuf,
    pub entries: usize,
    pub bytes: u64,
    pub added: usize,
    pub topped_up: bool,
}

pub struct Row {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
}

pub struct Sheet {
    pub kind: String,
    pub version: u64,
    pub root: String,
    pub rows: Vec<Row>,
    pub held: Vec<Row>,
    pub owner: String,
}

fn sha_hex(b: &[u8]) -> String {
    hexfmt::encode(&zikaron::cryptox::sha256(b)).trim_start_matches("0x").to_string()
}

pub fn bundle_in(folder: &std::path::Path, holder: &str, seat: crate::roles::Role) -> Result<std::path::PathBuf, Fault> {
    crate::home::landing(&folder.to_string_lossy())?;
    let who = holder.trim().trim_start_matches("0x").to_ascii_lowercase();
    if who.is_empty() {
        return Err(Fault::known(Known::KeychainMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::TailBackupNoHolder).to_string())));
    }
    Ok(folder.join(STEM).join(who).join(seat.as_str()))
}

pub fn folder_of(bundle: &std::path::Path) -> std::path::PathBuf {
    let up = |p: &std::path::Path| p.parent().map(|x| x.to_path_buf()).unwrap_or_default();
    up(&up(&up(bundle)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Belongs {
    Empty,
    Ours { entries: usize },
    Other { root: String },
    Junk { why: String },
}

pub fn belongs(bundle: &std::path::Path, our_root: &str) -> Belongs {
    if !bundle.exists() {
        return Belongs::Empty;
    }
    let empty = std::fs::read_dir(bundle).map(|mut d| d.next().is_none()).unwrap_or(false);
    if empty {
        return Belongs::Empty;
    }
    let sheet = match inspect(bundle) {
        Ok(s) => s,
        Err(f) => return Belongs::Junk { why: f.said().to_string() },
    };
    if let Some(why) = sheet.shape_trouble() {
        return Belongs::Junk { why };
    }
    match sheet.root.as_str() {
        "" if our_root.is_empty() => match our_owner() {
            Some(now) if sheet.owner.eq_ignore_ascii_case(&now) => Belongs::Ours { entries: sheet.rows.len() },
            _ => Belongs::Other { root: format!("身份 {}", sheet.owner) },
        },
        root if root.eq_ignore_ascii_case(our_root) => Belongs::Ours { entries: sheet.rows.len() },
        root => Belongs::Other { root: root.to_string() },
    }
}

fn our_owner() -> Option<String> {
    crate::register::now_row_listed().ok().flatten().map(|(row, _)| row.id)
}

fn our_root(home: &Home) -> Result<String, Fault> {
    let survey = home
        .ledger()?
        .survey()?;
    if survey.items.is_empty() {
        return Ok(String::new());
    }
    crate::auditx::root_of(&survey.items)
}

pub fn export(_pass: &crate::exitgate::Pass, home: &Home, out: &std::path::Path, _now: u64) -> Result<Made, Fault> {
    crate::trace::mark(crate::feature::Feature::H4);
    crate::home::landing(&out.to_string_lossy())?;
    let root = our_root(home)?;
    let onto_old = match belongs(out, &root) {
        Belongs::Empty => false,
        Belongs::Ours { .. } => true,
        Belongs::Other { root } => return Err(Fault::known(Known::MirrorOther, root)),
        Belongs::Junk { why } => return Err(Fault::known(Known::Occupied, why)),
    };
    let ledger = home.ledger()?;
    let survey = ledger
        .survey()?;
    let dir = out.join(ENTRIES);
    std::fs::create_dir_all(&dir).map_err(|e| classify(&e, &dir.display().to_string()))?;
    let mut rows: Vec<Value> = Vec::new();
    let mut total = 0u64;
    let mut added = 0usize;
    for bytes in &survey.items {
        let name = hexfmt::encode(&k1::entry_id(bytes));
        let name = name.trim_start_matches("0x").to_string();
        total += bytes.len() as u64;
        let sha = sha_hex(bytes);
        let at = dir.join(&name);
        let have = std::fs::read(&at).map(|b| sha_hex(&b) == sha).unwrap_or(false);
        if !have {
            if at.exists() {
                std::fs::remove_file(&at).map_err(|e| classify(&e, &at.display().to_string()))?;
            }
            zikaron_glue::landing::land_bytes(&at, bytes)
                .map_err(|t| Fault::of_landing(t))?;
            added += 1;
        }
        rows.push(Value::Obj(vec![
            ("bytes".into(), Value::Int(bytes.len() as u64)),
            ("name".into(), Value::Str(name)),
            ("sha256".into(), Value::Str(sha)),
        ]));
    }
    let mut held_rows: Vec<Value> = Vec::new();
    for (rel, bytes) in crate::vaultx::held_rows(home)? {
        let at = out.join(HELD).join(&rel);
        if let Some(d) = at.parent() {
            std::fs::create_dir_all(d).map_err(|e| classify(&e, &d.display().to_string()))?;
        }
        total += bytes.len() as u64;
        let sha = sha_hex(&bytes);
        let same = std::fs::read(&at).map(|b| sha_hex(&b) == sha).unwrap_or(false);
        if !same {
            if at.exists() {
                std::fs::remove_file(&at).map_err(|e| classify(&e, &at.display().to_string()))?;
            }
            zikaron_glue::landing::land_bytes(&at, &bytes)
                .map_err(|t| Fault::of_landing(t))?;
        }
        held_rows.push(Value::Obj(vec![
            ("bytes".into(), Value::Int(bytes.len() as u64)),
            ("path".into(), Value::Str(rel)),
            ("sha256".into(), Value::Str(sha)),
        ]));
    }
    let owner = if root.is_empty() { our_owner().ok_or_else(|| Fault::known(Known::NoIdentity, String::new()))? } else { String::new() };
    let mut sheet_fields = vec![
        ("entries".into(), Value::Arr(rows)),
        ("held".into(), Value::Arr(held_rows)),
        ("kind".into(), Value::Str(KIND.to_string())),
        ("root".into(), Value::Str(root)),
        ("version".into(), Value::Int(VERSION)),
    ];
    if !owner.is_empty() {
        sheet_fields.push(("owner".into(), Value::Str(owner)));
        sheet_fields.sort_by(|a: &(String, Value), b: &(String, Value)| a.0.cmp(&b.0));
    }
    let sheet = Value::Obj(sheet_fields);
    let manifest = out.join(MANIFEST);
    if manifest.exists() {
        std::fs::remove_file(&manifest).map_err(|e| classify(&e, &manifest.display().to_string()))?;
    }
    zikaron_glue::landing::land_bytes(&manifest, &json::canon_bytes(&sheet))
        .map_err(|t| Fault::of_landing(t))?;
    Ok(Made {
        root: out.to_path_buf(),
        entries: survey.items.len(),
        bytes: total,
        added,
        topped_up: onto_old,
    })
}

pub fn inspect(bundle: &std::path::Path) -> Result<Sheet, Fault> {
    let p = bundle.join(MANIFEST);
    let bytes = std::fs::read(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
    if let Ok(v) = zikaron::json::parse(&bytes) {
        let has = |k: &str| matches!(&v, zikaron::json::Value::Obj(m) if m.iter().any(|(n, _)| n == k));
        let rootless = matches!(v.member("root"), Some(zikaron::json::Value::Str(r)) if r.is_empty());
        for k in ["entries", "held", "kind", "root", "version"].into_iter().chain(rootless.then_some("owner")) {
            if !has(k) {
                return Err(Fault::known(Known::MirrorShape, format!("{MANIFEST}: {k}")));
            }
        }
    }
    let s = zikaron_glue::mirror::sheet(&bytes)
        .map_err(|t| Fault::known(Known::MirrorShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail185, &[&(MANIFEST).to_string(), &format!("{:?}", t)]))))?;
    let row = |r: zikaron_glue::mirror::Row| Row { name: r.name, sha256: r.sha256, bytes: r.bytes };
    Ok(Sheet {
        kind: s.kind,
        version: s.version,
        root: s.root,
        rows: s.rows.into_iter().map(row).collect(),
        held: s.held.into_iter().map(row).collect(),
        owner: s.owner,
    })
}

impl Sheet {
    pub fn shape_trouble(&self) -> Option<String> {
        if self.kind != KIND {
            return Some(crate::lang::filln(crate::lang::Key::Tail186, &[&format!("{:?}", self.kind), &(KIND).to_string()]));
        }
        if self.version != VERSION {
            return Some(crate::lang::filln(crate::lang::Key::Tail187, &[&(self.version).to_string(), &(VERSION).to_string()]));
        }
        for r in &self.rows {
            if EntryName::parse(&r.name).is_none() {
                return Some(crate::lang::filln(crate::lang::Key::Tail188, &[&format!("{:?}", r.name)]));
            }
            if r.sha256.len() != 64 {
                return Some(crate::lang::filln(crate::lang::Key::Tail189, &[&(r.name).to_string()]));
            }
        }
        for r in &self.held {
            if r.name.is_empty()
                || r.name.starts_with('/')
                || r.name.split('/').any(|seg| seg.is_empty() || seg == "." || seg == "..")
            {
                return Some(crate::lang::filln(crate::lang::Key::Tail190, &[&format!("{:?}", r.name)]));
            }
            if r.sha256.len() != 64 {
                return Some(crate::lang::filln(crate::lang::Key::Tail189, &[&(r.name).to_string()]));
            }
        }
        None
    }
}

pub fn verify(bundle: &std::path::Path) -> Result<Vec<(EntryName, Vec<u8>)>, Fault> {
    let sheet = inspect(bundle)?;
    if sheet.shape_trouble().is_some() {
        return Err(Fault::known(Known::MirrorShape, crate::lang::both_of(|| sheet.shape_trouble().unwrap_or_default())));
    }
    let dir = bundle.join(ENTRIES);
    let mut out = Vec::new();
    for r in &sheet.rows {
        let p = dir.join(&r.name);
        let bytes = std::fs::read(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
        if bytes.len() as u64 != r.bytes {
            return Err(Fault::known(
                Known::MirrorEntry,
                crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail191, &[&(r.name).to_string(), &(bytes.len()).to_string(), &(r.bytes).to_string()])),
            ));
        }
        let got = sha_hex(&bytes);
        if got != r.sha256 {
            return Err(Fault::known(Known::MirrorEntry, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail192, &[&(r.name).to_string()]))));
        }
        if let Err(token) = zikaron_glue::book::check(&bytes) {
            return Err(Fault::mirror_entry(&r.name, token));
        }
        let id = hexfmt::encode(&k1::entry_id(&bytes));
        let id = id.trim_start_matches("0x");
        if id != r.name {
            return Err(Fault::known(
                Known::MirrorEntry,
                crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail194, &[&(r.name).to_string(), &(id).to_string()])),
            ));
        }
        let name = EntryName::parse(&r.name)
            .ok_or_else(|| Fault::known(Known::MirrorShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail188, &[&(r.name).to_string()]))))?;
        out.push((name, bytes));
    }
    Ok(out)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mirrored {
    Never,
    Gone { path: String },
    Bad { path: String, why: String },
    At { path: String, at: u64, entries: usize },
}

pub fn status(rec: Option<&crate::settings::MirrorRecord>) -> Mirrored {
    let Some(r) = rec else {
        return Mirrored::Never;
    };
    let at = std::path::Path::new(&r.path);
    if !at.is_dir() {
        return Mirrored::Gone { path: r.path.clone() };
    }
    match verify(at) {
        Ok(rows) => Mirrored::At { path: r.path.clone(), at: r.at, entries: rows.len() },
        Err(f) => Mirrored::Bad { path: r.path.clone(), why: f.said().to_string() },
    }
}
