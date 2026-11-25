use crate::fault::{classify, Fault, Known};
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};
use zikaron_glue::names::Field;

fn contents_key() -> &'static str {
    Field::Contents.as_str()
}
fn note_key() -> &'static str {
    Field::NoteMd.as_str()
}

pub const FORM: &str = "zikaron.kits-index/1";
pub const DIR: &str = zikaron_seal::exports::DIR;
pub const FILE: &str = zikaron_seal::exports::INDEX;
pub const OVERRIDES: &str = "links.json";

pub const EXPORTS: [Export; 6] = [
    Export { name: "kitsIndex", place: Some(&[DIR, FILE]), room: false },
    Export { name: "kitVerifications", place: Some(&[DIR, crate::verifiedx::DIR]), room: true },
    Export { name: "kit", place: None, room: false },
    Export { name: "mirror", place: None, room: false },
    Export { name: "machineBackup", place: None, room: false },
    Export { name: "grantFile", place: None, room: false },
];

#[derive(Clone, Copy, Debug)]
pub struct Export {
    pub name: &'static str,
    pub place: Option<&'static [&'static str]>,
    pub room: bool,
}

pub use zikaron_glue::anchoredon::{note_with, split_note, AnchoredOn};

fn anchored_value(a: &AnchoredOn) -> Value {
    Value::Obj(vec![
        ("chainId".to_string(), Value::Int(a.chain_id)),
        ("fromBlock".to_string(), Value::Int(a.from_block)),
        ("registry".to_string(), Value::Str(a.registry.clone())),
    ])
}

fn anchored_of(v: &Value) -> Option<AnchoredOn> {
    let int = |k: &str| match member(v, k) {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    };
    let a = AnchoredOn { chain_id: int("chainId")?, from_block: int("fromBlock")?, registry: str_of(v, "registry")? };
    (AnchoredOn::of_line(&a.line()).as_ref() == Some(&a)).then_some(a)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub created: u64,
    pub root: String,
    pub path: String,
    pub contents: Vec<String>,
    pub note_md: String,
    pub link: Option<String>,
    pub anchored_on: Option<AnchoredOn>,
    pub one_time: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowFacts {
    pub contents: Vec<String>,
    pub note_md: String,
    pub anchored_on: Option<AnchoredOn>,
    pub one_time: Option<String>,
}

impl RowFacts {
    pub fn of_kit(kit: &Path) -> Result<RowFacts, Fault> {
        let (contents, note_md, anchored_on) = manifest_row_facts(kit)?;
        Ok(RowFacts { contents, note_md, anchored_on, one_time: None })
    }
}

const ONE_TIME: &str = "oneTimeKey";

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
    let mut m = Vec::new();
    if let Some(a) = &r.anchored_on {
        m.push(("anchoredOn".to_string(), anchored_value(a)));
    }
    m.extend([
        (contents_key().to_string(), Value::Arr(r.contents.iter().map(|c| Value::Str(c.clone())).collect())),
        ("created".to_string(), Value::Int(r.created)),
        ("id".to_string(), Value::Str(r.id.clone())),
    ]);
    if let Some(l) = &r.link {
        m.push(("link".to_string(), Value::Str(l.clone())));
    }
    m.push((note_key().to_string(), Value::Str(r.note_md.clone())));
    if let Some(k) = &r.one_time {
        m.push((ONE_TIME.to_string(), Value::Str(k.clone())));
    }
    m.push(("path".to_string(), Value::Str(r.path.clone())));
    m.push(("root".to_string(), Value::Str(r.root.clone())));
    Value::Obj(m)
}

pub fn bytes_of(rows: &[Row]) -> Vec<u8> {
    json::canon_bytes(&Value::Obj(vec![
        ("form".to_string(), Value::Str(FORM.to_string())),
        ("kits".to_string(), Value::Arr(rows.iter().map(row_value).collect())),
    ]))
}

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
        let anchored_on = match member(k, "anchoredOn") {
            None => None,
            Some(a) => Some(anchored_of(a).ok_or_else(bad)?),
        };
        out.push(Row { id, created, root, path, contents, note_md, link: str_of(k, "link"), anchored_on, one_time: str_of(k, ONE_TIME) });
    }
    Ok(Some(out))
}

pub fn write(machine: &Path, rows: &[Row]) -> Result<(), Fault> {
    crate::home::put_at(&machine.join(DIR), FILE, &bytes_of(rows))
}

pub fn ensure(machine: &Path) -> Result<bool, Fault> {
    if path_in(machine).exists() {
        return Ok(false);
    }
    write(machine, &[])?;
    Ok(true)
}

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

fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).map(crate::home::plain_path).unwrap_or_else(|_| p.to_path_buf())
}

pub fn rel_of(kits_room: &Path, kit: &Path) -> String {
    let (room, kit) = (canon(kits_room), canon(kit));
    match kit.strip_prefix(&room) {
        Ok(r) if !r.as_os_str().is_empty() => r.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect::<Vec<_>>().join("/"),
        _ => kit.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
    }
}

pub fn default_link(publish: Option<&str>, rel: &str) -> Option<String> {
    let base = crate::fetchx::base_of(publish?).ok()?;
    if rel.is_empty() {
        return None;
    }
    let enc: Vec<String> = rel.split('/').map(pct).collect();
    Some(format!("{}{}/", base.as_str(), enc.join("/")))
}

fn default_for(publish: Option<&str>, kits_room: &Path, kit: &Path) -> Option<String> {
    if kit.is_file() { None } else { default_link(publish, &rel_of(kits_room, kit)) }
}

fn pct(seg: &str) -> String {
    seg.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

pub fn manifest_facts(kit: &Path) -> Result<(Vec<String>, String), Fault> {
    manifest_row_facts(kit).map(|(c, n, _)| (c, n))
}

pub fn manifest_row_facts(kit: &Path) -> Result<(Vec<String>, String, Option<AnchoredOn>), Fault> {
    let p = kit.join(zikaron_glue::names::MANIFEST);
    let bytes = manifest_of(kit)?;
    let v = json::parse(&bytes).map_err(|t| Fault::known(Known::NotAdoptable, format!("{}: {t:?}", p.display())))?;
    let contents = match member(&v, contents_key()) {
        Some(Value::Arr(a)) => a.iter().filter_map(|r| str_of(r, Field::Content.as_str())).collect(),
        _ => Vec::new(),
    };
    let (note, at) = zikaron_glue::anchoredon::of_manifest(&bytes);
    Ok((contents, note, at))
}

fn manifest_of(kit: &Path) -> Result<Vec<u8>, Fault> {
    if kit.is_file() {
        return zikaron_glue::container::manifest_at(kit).ok_or_else(|| Fault::known(Known::KitFileBad, kit.display().to_string()));
    }
    let p = kit.join(zikaron_glue::names::MANIFEST);
    std::fs::read(&p).map_err(|e| classify(&e, &p.display().to_string()))
}

pub fn add(machine: &Path, home: &crate::home::Home, kit: &Path, id: &str, created: u64, publish: Option<&str>, facts: RowFacts) -> Result<Row, Fault> {
    let RowFacts { contents, note_md, anchored_on, one_time } = facts;
    let root = crate::ledgerx::root_of(home)?;
    let mut rows = read(machine)?.unwrap_or_default();
    let o = overrides(machine)?;
    let path = crate::home::plain_path(kit.to_path_buf()).display().to_string();
    let link = o.iter().find(|(k, _)| *k == path).map(|(_, l)| l.clone()).or_else(|| default_for(publish, &home.dir(crate::home::Slot::Kits), kit));
    let row = Row { id: id.to_string(), created, root, path, contents, note_md, link, anchored_on, one_time };
    rows.retain(|r| r.path != row.path);
    rows.push(row.clone());
    write(machine, &rows)?;
    Ok(row)
}

pub fn set_link(machine: &Path, path: &str, link: &str, root: &str, kits_room: &Path, publish: Option<&str>) -> Result<Option<String>, Fault> {
    let mut rows = read(machine)?.unwrap_or_default();
    let Some(at) = rows.iter().position(|r| r.path == path && r.root.eq_ignore_ascii_case(root)) else {
        return Err(Fault::known(Known::SubjectMissing, path.to_string()));
    };
    let mut o = overrides(machine)?;
    o.retain(|(k, _)| k != path);
    let t = link.trim();
    let now = if t.is_empty() {
        default_for(publish, kits_room, Path::new(&rows[at].path))
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

pub fn relink(machine: &Path, root: &str, kits_room: &Path, publish: Option<&str>) -> Result<usize, Fault> {
    let Some(mut rows) = read(machine)? else { return Ok(0) };
    let o = overrides(machine)?;
    let mut n = 0;
    for r in rows.iter_mut() {
        if o.iter().any(|(k, _)| *k == r.path) || !r.root.eq_ignore_ascii_case(root) {
            continue;
        }
        let l = default_for(publish, kits_room, Path::new(&r.path));
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

pub fn drop_copy(machine: &Path, path: &str) -> Result<(Row, bool), Fault> {
    let mut rows = read(machine)?.unwrap_or_default();
    let Some(at) = rows.iter().position(|r| r.path == path) else {
        return Err(Fault::known(Known::SubjectMissing, path.to_string()));
    };
    let row = rows[at].clone();
    let dir = PathBuf::from(&row.path);
    let same = match (&row.one_time, crate::addressedx::one_time_of(&dir)) {
        (Some(k), Some(head)) => zikaron::hexfmt::encode(&head).eq_ignore_ascii_case(k),
        (Some(_), None) => false,
        (None, _) => manifest_of(&dir).ok().map(|b| zikaron::hexfmt::encode(&zikaron::entry::entry_id(&b)).eq_ignore_ascii_case(&row.id)).unwrap_or(false),
    };
    if same && dir.is_file() {
        std::fs::remove_file(&dir).map_err(|e| classify(&e, &row.path))?;
    } else if same {
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
