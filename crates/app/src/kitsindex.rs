//! Record bundle index.
//!
//! Bundles live in each identity's home (`<home>/kits/…`), and sibling apps need one place to find "which
//! bundles were exported on this machine, and where". Homes may not store absolute paths (any copy of a home
//! is equivalent), so the index lives in the machine directory: `<machine directory>/kits/index.json`. Only
//! this desk rewrites it, atomically (through `home::put_at`: temporary name then rename, 0600); it is
//! written empty at window start when absent, and rewritten together with the pointer when the home moves.
//!
//! ─── Shape ───
//!
//! `{"form":"zikaron.kits-index/1","kits":[{"contents":[hex32…],"created":seconds,"id":hex32,
//! "link":"https://…"?,"note_md":"…","path":"/…","root":hex32}…]}`, canonical key order. Readers recognize
//! the file by the `form` literal, not by an integer version.
//!
//! ─── `link` ───
//!
//! By default it is assembled from that home's publish base (`settings.publish`) plus the bundle's relative
//! path in the `kits` room; a hand-filled one overrides it, recorded in the same room's [`OVERRIDES`] (for
//! this desk only; readers do not read it). When the publish base changes, rows nobody filled by hand are
//! reassembled and hand-filled ones stay as they are. There is no second publishing concept.

use crate::fault::{classify, Fault, Known};
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};
use zikaron_glue::names::Field;

/// The two index cells named as in kit law (`contents`, `note_md`): the literals come from kit law's own name
/// table, and the shell writes no copy.
fn contents_key() -> &'static str {
    Field::Contents.as_str()
}
fn note_key() -> &'static str {
    Field::NoteMd.as_str()
}

/// The index's form literal. One name, one home.
pub const FORM: &str = "zikaron.kits-index/1";
/// The room in the machine directory that holds the index.
pub const DIR: &str = "kits";
/// Index file name.
pub const FILE: &str = "index.json";
/// Hand-filled `link`s are recorded here (for this desk only; readers do not read it).
pub const OVERRIDES: &str = "links.json";

/// One index row (seven cells).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The bundle's `kit_id`, lowercase with 0x.
    pub id: String,
    /// Export time, whole unix seconds.
    pub created: u64,
    /// This ledger's root (the genesis entry's id).
    pub root: String,
    /// The bundle directory's absolute path.
    pub path: String,
    /// Digests of the original files (the `contents` values already in the bundle's MANIFEST).
    pub contents: Vec<String>,
    pub note_md: String,
    pub link: Option<String>,
}

/// Where the index file is.
pub fn path_in(machine: &Path) -> PathBuf {
    machine.join(DIR).join(FILE)
}

fn str_of(v: &Value, k: &str) -> Option<String> {
    match v {
        Value::Obj(m) => m.iter().find(|(x, _)| x == k).and_then(|(_, v)| match v {
            Value::Str(s) => Some(s.clone()),
            _ => None,
        }),
        _ => None,
    }
}

fn member<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(x, _)| x == k).map(|(_, v)| v),
        _ => None,
    }
}

fn row_value(r: &Row) -> Value {
    let mut m = vec![
        (contents_key().to_string(), Value::Arr(r.contents.iter().map(|c| Value::Str(c.clone())).collect())),
        ("created".to_string(), Value::Int(r.created)),
        ("id".to_string(), Value::Str(r.id.clone())),
    ];
    if let Some(l) = &r.link {
        m.push(("link".to_string(), Value::Str(l.clone())));
    }
    m.push((note_key().to_string(), Value::Str(r.note_md.clone())));
    m.push(("path".to_string(), Value::Str(r.path.clone())));
    m.push(("root".to_string(), Value::Str(r.root.clone())));
    Value::Obj(m)
}

/// The index's canonical bytes. Writing and reading share one source.
pub fn bytes_of(rows: &[Row]) -> Vec<u8> {
    json::canon_bytes(&Value::Obj(vec![
        ("form".to_string(), Value::Str(FORM.to_string())),
        ("kits".to_string(), Value::Arr(rows.iter().map(row_value).collect())),
    ]))
}

/// Read the index. No file gives `None`; a `form` other than [`FORM`] or a row with missing cells is refused
/// by name (never quietly used as empty).
pub fn read(machine: &Path) -> Result<Option<Vec<Row>>, Fault> {
    let p = path_in(machine);
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(classify(&e, &p.display().to_string())),
    };
    let bad = || Fault::known(Known::SettingsShape, p.display().to_string());
    let v = json::parse(&bytes).map_err(|t| Fault::known(Known::SettingsShape, format!("{}: {t:?}", p.display())))?;
    if str_of(&v, "form").as_deref() != Some(FORM) {
        return Err(bad());
    }
    let Some(Value::Arr(kits)) = member(&v, "kits") else { return Err(bad()) };
    let mut out = Vec::new();
    for k in kits {
        let created = match member(k, "created") {
            Some(Value::Int(n)) => *n,
            _ => return Err(bad()),
        };
        let contents = match member(k, contents_key()) {
            Some(Value::Arr(a)) => a.iter().map(|x| if let Value::Str(s) = x { Some(s.clone()) } else { None }).collect::<Option<Vec<_>>>().ok_or_else(bad)?,
            _ => return Err(bad()),
        };
        let (Some(id), Some(root), Some(path), Some(note_md)) = (str_of(k, "id"), str_of(k, "root"), str_of(k, "path"), str_of(k, note_key())) else {
            return Err(bad());
        };
        out.push(Row { id, created, root, path, contents, note_md, link: str_of(k, "link") });
    }
    Ok(Some(out))
}

/// Atomic rewrite (temporary name then rename, 0600, through `home::put_at`).
pub fn write(machine: &Path, rows: &[Row]) -> Result<(), Fault> {
    crate::home::put_at(&machine.join(DIR), FILE, &bytes_of(rows))
}

/// At window start: write an empty index when there is none. Returns whether it wrote.
pub fn ensure(machine: &Path) -> Result<bool, Fault> {
    if path_in(machine).exists() {
        return Ok(false);
    }
    write(machine, &[])?;
    Ok(true)
}

/// Hand-filled `link`s (bundle directory absolute path → address; the same bundle exported twice is two rows,
/// each with its own source). No file means empty.
pub fn overrides(machine: &Path) -> Result<Vec<(String, String)>, Fault> {
    let p = machine.join(DIR).join(OVERRIDES);
    let Some(bytes) = crate::local::read(&p, crate::local::Doc::KitLinks)? else { return Ok(Vec::new()) };
    match json::parse(&bytes).map_err(|t| Fault::known(Known::SettingsShape, format!("{}: {t:?}", p.display())))? {
        Value::Obj(m) => m
            .into_iter()
            .map(|(k, v)| if let Value::Str(s) = v { Some((k, s)) } else { None })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| Fault::known(Known::SettingsShape, p.display().to_string())),
        _ => Err(Fault::known(Known::SettingsShape, p.display().to_string())),
    }
}

fn write_overrides(machine: &Path, o: &[(String, String)]) -> Result<(), Fault> {
    let v = Value::Obj(o.iter().map(|(k, s)| (k.clone(), Value::Str(s.clone()))).collect());
    crate::local::put(&machine.join(DIR), OVERRIDES, crate::local::Doc::KitLinks, &json::canon_bytes(&v))
}

/// On disk, the canonicalized path; otherwise unchanged (`/tmp` and `/private/tmp` are the same place).
fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// The bundle's relative path in the `kits` room (for assembling `link`): relative to the room when inside
/// it, otherwise the bundle directory's name.
pub fn rel_of(kits_room: &Path, kit: &Path) -> String {
    let (room, kit) = (canon(kits_room), canon(kit));
    match kit.strip_prefix(&room) {
        Ok(r) if !r.as_os_str().is_empty() => r.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/"),
        _ => kit.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
    }
}

/// The default `link`: the publish base (recognized by `fetchx::base_of`, ending in `/`) plus the path
/// within, plus `/`. No base configured means none.
pub fn default_link(publish: Option<&str>, rel: &str) -> Option<String> {
    let base = crate::fetchx::base_of(publish?).ok()?;
    if rel.is_empty() {
        return None;
    }
    // The path is percent-encoded segment by segment (spaces, non-ASCII and reserved characters never go into
    // an address raw).
    let enc: Vec<String> = rel.split('/').map(pct).collect();
    Some(format!("{}{}/", base.as_str(), enc.join("/")))
}

/// Percent-encode one path segment (RFC 3986 unreserved characters unchanged, everything else `%XX` per
/// byte).
fn pct(seg: &str) -> String {
    seg.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// Read the bundle's own manifest for the digests in the `contents` column (existing values, not recomputed)
/// and `note_md`.
pub fn manifest_facts(kit: &Path) -> Result<(Vec<String>, String), Fault> {
    let p = kit.join(zikaron_glue::names::MANIFEST);
    let bytes = std::fs::read(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
    let v = json::parse(&bytes).map_err(|t| Fault::known(Known::NotAdoptable, format!("{}: {t:?}", p.display())))?;
    let contents = match member(&v, contents_key()) {
        Some(Value::Arr(a)) => a.iter().filter_map(|r| str_of(r, Field::Content.as_str())).collect(),
        _ => Vec::new(),
    };
    Ok((contents, str_of(&v, note_key()).unwrap_or_default()))
}

/// Add a row when an export lands. A row is identified by the bundle directory's path (the same bundle
/// exported to two places is two rows; exporting again to the same place replaces that row). `link` follows
/// that place's override, otherwise the default.
pub fn add(machine: &Path, home: &crate::home::Home, kit: &Path, id: &str, created: u64, publish: Option<&str>) -> Result<Row, Fault> {
    let (contents, note_md) = manifest_facts(kit)?;
    let root = crate::ledgerx::root_of(home)?;
    let mut rows = read(machine)?.unwrap_or_default();
    let o = overrides(machine)?;
    let path = kit.display().to_string();
    let link = o.iter().find(|(k, _)| *k == path).map(|(_, l)| l.clone()).or_else(|| default_link(publish, &rel_of(&home.dir(crate::home::Slot::Kits), kit)));
    let row = Row { id: id.to_string(), created, root, path, contents, note_md, link };
    rows.retain(|r| r.path != row.path);
    rows.push(row.clone());
    write(machine, &rows)?;
    Ok(row)
}

/// Hand-fill `link`: non-empty overrides and is recorded; empty removes the override and returns to the
/// default (assembled from that home's publish base).
pub fn set_link(machine: &Path, path: &str, link: &str, root: &str, kits_room: &Path, publish: Option<&str>) -> Result<Option<String>, Fault> {
    let mut rows = read(machine)?.unwrap_or_default();
    // Only rows exported from this ledger (`root`) change: returning to the default needs that home's publish
    // base, and other homes' rows are changed in their own homes.
    let Some(at) = rows.iter().position(|r| r.path == path && r.root.eq_ignore_ascii_case(root)) else {
        return Err(Fault::known(Known::SubjectMissing, path.to_string()));
    };
    let mut o = overrides(machine)?;
    o.retain(|(k, _)| k != path);
    let t = link.trim();
    let now = if t.is_empty() {
        default_link(publish, &rel_of(kits_room, Path::new(&rows[at].path)))
    } else {
        let u = crate::fetchx::base_of(t)?.as_str().to_string();
        o.push((path.to_string(), u.clone()));
        Some(u)
    };
    rows[at].link = now.clone();
    write_overrides(machine, &o)?;
    write(machine, &rows)?;
    Ok(now)
}

/// The publish base changed: among rows exported from this ledger (`root`), those nobody filled by hand get
/// `link` reassembled on the new base (whether or not the bundle is in the `kits` room, assembled the same
/// way as the default at add time). Returns how many rows changed.
pub fn relink(machine: &Path, root: &str, kits_room: &Path, publish: Option<&str>) -> Result<usize, Fault> {
    let Some(mut rows) = read(machine)? else { return Ok(0) };
    let o = overrides(machine)?;
    let mut n = 0;
    for r in rows.iter_mut() {
        if o.iter().any(|(k, _)| *k == r.path) || !r.root.eq_ignore_ascii_case(root) {
            continue;
        }
        let l = default_link(publish, &rel_of(kits_room, Path::new(&r.path)));
        if l != r.link {
            r.link = l;
            n += 1;
        }
    }
    if n > 0 {
        write(machine, &rows)?;
    }
    Ok(n)
}

/// Delete the local copy: the row is removed; the bundle directory is deleted entirely only when the bundle
/// there is exactly the one this row names (the bundle id computed from the manifest equals the row's `id`;
/// the ledger is untouched). When that place is already gone or holds another bundle (deleted in Finder, a
/// new one exported under the same name), only the row is removed and no other bundle is touched. Returns
/// (the removed row, whether the directory was deleted).
pub fn drop_copy(machine: &Path, path: &str) -> Result<(Row, bool), Fault> {
    let mut rows = read(machine)?.unwrap_or_default();
    let Some(at) = rows.iter().position(|r| r.path == path) else {
        return Err(Fault::known(Known::SubjectMissing, path.to_string()));
    };
    let row = rows[at].clone();
    let dir = PathBuf::from(&row.path);
    let manifest = std::fs::read(dir.join(zikaron_glue::names::MANIFEST)).ok();
    let same = manifest.map(|b| zikaron::hexfmt::encode(&zikaron::entry::entry_id(&b)).eq_ignore_ascii_case(&row.id)).unwrap_or(false);
    if same {
        std::fs::remove_dir_all(&dir).map_err(|e| classify(&e, &row.path))?;
    }
    rows.remove(at);
    let mut o = overrides(machine)?;
    if o.iter().any(|(k, _)| *k == row.path) {
        o.retain(|(k, _)| *k != row.path);
        write_overrides(machine, &o)?;
    }
    write(machine, &rows)?;
    Ok((row, same))
}

/// When the home moves: rows whose bundle path is under the old home get the new home as prefix (together
/// with the pointer). Returns how many rows changed.
pub fn rebase(machine: &Path, from: &Path, to: &Path) -> Result<usize, Fault> {
    let Some(mut rows) = read(machine)? else { return Ok(0) };
    let (from_c, to) = (canon(from), canon(to));
    let mut n = 0;
    let mut moved: Vec<(String, String)> = Vec::new();
    for r in rows.iter_mut() {
        let p = Path::new(&r.path);
        if let Ok(rest) = p.strip_prefix(&from_c).or_else(|_| p.strip_prefix(from)) {
            let new = to.join(rest).display().to_string();
            moved.push((r.path.clone(), new.clone()));
            r.path = new;
            n += 1;
        }
    }
    if n > 0 {
        // Hand-filled overrides are recorded by bundle path: when the path changes, that cell follows.
        let mut o = overrides(machine)?;
        let mut touched = false;
        for (k, _) in o.iter_mut() {
            if let Some((_, new)) = moved.iter().find(|(old, _)| old == k) {
                *k = new.clone();
                touched = true;
            }
        }
        if touched {
            write_overrides(machine, &o)?;
        }
        write(machine, &rows)?;
    }
    Ok(n)
}
