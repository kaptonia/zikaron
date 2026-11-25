use crate::landing;
use crate::names::{Field, Key, Slot, ENTRIES_DIR, ENTRY_SUFFIX, FILES_DIR, MANIFEST};
use crate::tidy::{self, Fate};
use std::path::Path;
use zikaron::json::{self, Value};
use zikaron_kit::v2::doc;
use zikaron_kit::v2::kit::{self as kit2, Enumeration, FileRow, KitVerdict};
use zikaron_kit::v2::tokens::SPEC_KIT;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Anchor {
    pub entry_id: String,
    pub chain_id: u64,
    pub tx: String,
}

#[derive(Default)]
pub struct Bundle {
    pub entries: Vec<Vec<u8>>,
    pub files: Vec<(String, Vec<u8>)>,
    pub contents: Vec<String>,
    pub root: Option<String>,
    pub note: String,
    pub names: Vec<(String, String)>,
    pub dir_names: Vec<(String, String)>,
    pub dir_members: Vec<(String, Vec<String>)>,
    pub one_work: bool,
    pub anchored_on: Option<crate::anchoredon::AnchoredOn>,
    pub anchors: Vec<Anchor>,
}

pub struct Landed {
    pub entries: usize,
    pub files: usize,
    pub kit_id: String,
    pub dropped: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trouble {
    Io(String, String),
    Occupied(String),
    BadPath(String),
    Duplicate(String),
    Refused(String, Option<String>),
}

impl From<landing::Trouble> for Trouble {
    fn from(t: landing::Trouble) -> Trouble {
        match t {
            landing::Trouble::Occupied(p) => Trouble::Occupied(p),
            landing::Trouble::Io(p, w) => Trouble::Io(p, w),
        }
    }
}

impl Trouble {
    pub fn code(&self) -> &'static str {
        match self {
            Trouble::Io(..) => "E_IO",
            Trouble::Occupied(_) => "E_OCCUPIED",
            Trouble::BadPath(_) => "E_BAD_PATH",
            Trouble::Duplicate(_) => "E_DUPLICATE_PATH",
            Trouble::Refused(_, _) => "E_KIT",
        }
    }

    pub fn said(&self) -> Option<&str> {
        match self {
            Trouble::Io(_, w) => Some(w.as_str()),
            _ => None,
        }
    }

    pub fn verdict(&self) -> Option<&str> {
        match self {
            Trouble::Refused(v, _) => Some(v.as_str()),
            _ => None,
        }
    }

    pub fn path(&self) -> Option<&str> {
        match self {
            Trouble::Io(p, _) | Trouble::Occupied(p) | Trouble::BadPath(p) | Trouble::Duplicate(p) => Some(p.as_str()),
            Trouble::Refused(_, _) => None,
        }
    }

    pub fn subject(&self) -> String {
        match self {
            Trouble::Io(p, _) | Trouble::Occupied(p) | Trouble::BadPath(p) | Trouble::Duplicate(p) => p.clone(),
            Trouble::Refused(verdict, subject) => match subject {
                Some(x) => format!("{verdict}:{x}"),
                None => verdict.clone(),
            },
        }
    }
}

fn digest(b: &[u8]) -> String {
    doc::doc_id(b)
}

fn row(members: Vec<(Field, Value)>) -> Value {
    Value::Obj(
        members
            .into_iter()
            .map(|(k, v)| (k.as_str().to_string(), v))
            .collect(),
    )
}

pub fn tidy(b: &mut Bundle) -> Result<Vec<String>, Trouble> {
    crate::seam_v2();
    let mut dropped: Vec<String> = Vec::new();
    for list in [&mut b.files] {
        let mut keep: Vec<(String, Vec<u8>)> = Vec::new();
        for (p, bytes) in list.drain(..) {
            match tidy::fate(&p) {
                Fate::Keep => keep.push((p, bytes)),
                Fate::Drop => dropped.push(p),
                Fate::Refuse => return Err(Trouble::BadPath(p)),
            }
        }
        *list = keep;
    }
    b.contents.retain(|p| !dropped.contains(p));
    dropped.sort();
    Ok(dropped)
}

fn file_rows(b: &Bundle) -> Vec<FileRow> {
    let mut rows: Vec<FileRow> = b
        .files
        .iter()
        .map(|(p, x)| FileRow {
            path: p.clone(),
            sha256: digest(x),
            size: x.len() as u64,
            name_md: b.names.iter().find(|(k, _)| k == p).map(|(_, n)| n.clone()).filter(|n| n != p),
        })
        .collect();
    rows.sort_by(|x, y| x.path.as_bytes().cmp(y.path.as_bytes()));
    rows
}

fn last_segment(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or(p)
}

fn directories_stand(b: &Bundle) -> Result<(), Trouble> {
    for (top, members) in &b.dir_members {
        let prefix = format!("{top}/");
        let under = b.files.iter().filter(|(p, _)| p.starts_with(&prefix)).count();
        if under != members.len() || b.dir_members.iter().filter(|(t, _)| t == top).count() > 1 {
            return Err(Trouble::Duplicate(top.clone()));
        }
    }
    Ok(())
}

pub fn manifest(b: &Bundle) -> Value {
    crate::seam_v2();
    let mut ids: Vec<String> = b.entries.iter().map(|x| digest(x)).collect();
    ids.sort_by(|x, y| x.as_bytes().cmp(y.as_bytes()));
    ids.dedup();

    let files = file_rows(b);
    let dir_name = |p: &str| b.dir_names.iter().find(|(k, n)| k == p && n != last_segment(p)).map(|(_, n)| n.clone());
    let mut contents: Vec<(String, String, Option<String>)> = b
        .contents
        .iter()
        .filter_map(|p| match files.iter().find(|f| f.path == *p) {
            Some(f) => Some((f.sha256.clone(), p.clone(), None)),
            None if files.iter().any(|f| f.path.starts_with(&format!("{p}/"))) => Some((kit2::directory_manifest_digest(&files, p), p.clone(), dir_name(p))),
            None => None,
        })
        .collect();
    let recorded: Vec<String> = b
        .entries
        .iter()
        .filter(|_| !b.one_work)
        .filter_map(|x| crate::book::check(x).ok())
        .filter_map(|e| crate::recordcheck::work_of(e.kind, &e.body).map(|c| c.to_ascii_lowercase()))
        .collect();
    let dirs: Vec<String> = contents.iter().filter(|(_, p, _)| !files.iter().any(|f| f.path == *p)).map(|(_, p, _)| format!("{p}/")).collect();
    for f in files.iter().filter(|f| dirs.iter().any(|d| f.path.starts_with(d.as_str())) && recorded.contains(&f.sha256.to_ascii_lowercase())) {
        contents.push((f.sha256.clone(), f.path.clone(), None));
    }
    contents.sort_by(|x, y| (x.0.as_bytes(), x.1.as_bytes()).cmp(&(y.0.as_bytes(), y.1.as_bytes())));
    contents.dedup_by(|x, y| x.0 == y.0 && x.1 == y.1);

    let mut anchors: Vec<Anchor> = b.anchors.iter().filter(|a| ids.contains(&a.entry_id)).cloned().collect();
    anchors.sort_by(|x, y| (x.entry_id.as_bytes(), x.chain_id, x.tx.as_bytes()).cmp(&(y.entry_id.as_bytes(), y.chain_id, y.tx.as_bytes())));
    anchors.dedup();

    let mut m: Vec<(String, Value)> = vec![
        (Field::Spec.as_str().to_string(), Value::Str(SPEC_KIT.to_string())),
        (
            Field::Root.as_str().to_string(),
            match &b.root {
                Some(r) => Value::Str(r.clone()),
                None => Value::Null,
            },
        ),
        (Field::Entries.as_str().to_string(), Value::Arr(ids.into_iter().map(Value::Str).collect())),
        (
            Field::Files.as_str().to_string(),
            Value::Arr(
                files
                    .into_iter()
                    .map(|f| {
                        let mut cells = vec![(Field::Path, Value::Str(f.path)), (Field::Sha256, Value::Str(f.sha256)), (Field::Size, Value::Int(f.size))];
                        if let Some(n) = f.name_md {
                            cells.push((Field::NameMd, Value::Str(n)));
                        }
                        row(cells)
                    })
                    .collect(),
            ),
        ),
        (
            Field::Contents.as_str().to_string(),
            Value::Arr(
                contents
                    .into_iter()
                    .map(|(c, p, n)| {
                        let mut cells = vec![(Field::Content, Value::Str(c)), (Field::Path, Value::Str(p))];
                        if let Some(n) = n {
                            cells.push((Field::NameMd, Value::Str(n)));
                        }
                        row(cells)
                    })
                    .collect(),
            ),
        ),
        (Field::NoteMd.as_str().to_string(), Value::Str(b.note.clone())),
    ];
    if let Some(a) = &b.anchored_on {
        m.push((
            Field::AnchoredOn.as_str().to_string(),
            row(vec![(Field::ChainId, Value::Int(a.chain_id)), (Field::Registry, Value::Str(a.registry.clone())), (Field::FromBlock, Value::Int(a.from_block))]),
        ));
    }
    if !anchors.is_empty() {
        m.push((
            Field::Anchors.as_str().to_string(),
            Value::Arr(
                anchors
                    .into_iter()
                    .map(|a| row(vec![(Field::EntryId, Value::Str(a.entry_id)), (Field::ChainId, Value::Int(a.chain_id)), (Field::Tx, Value::Str(a.tx))]))
                    .collect(),
            ),
        ));
    }
    Value::Obj(m)
}

pub fn companion_v1(b: &Bundle) -> Option<Bundle> {
    crate::seam_v2();
    let entries: Vec<Vec<u8>> = b.entries.iter().filter(|x| crate::book::check(x).is_ok_and(|e| e.law == crate::book::Law::One)).cloned().collect();
    if entries.is_empty() {
        return None;
    }
    let recorded: Vec<String> = entries.iter().filter_map(|x| crate::book::check(x).ok()).filter_map(|e| crate::recordcheck::work_of(e.kind, &e.body).map(|c| c.to_ascii_lowercase())).collect();
    let rows = file_rows(b);
    let kept: Vec<String> = b
        .contents
        .iter()
        .filter(|p| match rows.iter().find(|f| f.path == **p) {
            Some(f) => recorded.contains(&f.sha256.to_ascii_lowercase()),
            None => {
                let pre = format!("{p}/");
                let dir_rows: Vec<crate::recording::DirRow> = rows.iter().filter(|f| f.path.starts_with(&pre)).map(|f| (f.name_md.clone().unwrap_or_else(|| f.path[pre.len()..].to_string()), f.sha256.clone())).collect();
                crate::recording::dir_contents(&dir_rows).iter().any(|(d, _)| recorded.contains(&zikaron::hexfmt::encode(d)))
            }
        })
        .cloned()
        .collect();
    let under = |p: &str| kept.iter().any(|c| c == p || p.starts_with(&format!("{c}/")));
    let mut files: Vec<(String, Vec<u8>)> = b.files.iter().filter(|(p, _)| under(p)).cloned().collect();
    let names: Vec<(String, String)> = b
        .names
        .iter()
        .filter(|(path, _)| under(path))
        .map(|(path, name)| {
            let from = match path.split_once('/') {
                Some((top, _)) if b.contents.iter().any(|c| c == top) => format!("{top}/{name}"),
                _ => name.clone(),
            };
            (from, path.clone())
        })
        .collect();
    if !names.is_empty() {
        files.push((crate::kitnames::NAMES_FILE.to_string(), crate::kitnames::names_table(&names)));
    }
    let contents = b.files.iter().map(|(p, _)| p.clone()).filter(|p| under(p)).collect();
    Some(Bundle { entries, files, contents, root: None, note: b.note.clone(), ..Default::default() })
}

fn manifest_v1(b: &Bundle) -> Value {
    let mut ids: Vec<String> = b.entries.iter().map(|x| digest(x)).collect();
    ids.sort_by(|x, y| x.as_bytes().cmp(y.as_bytes()));
    ids.dedup();
    let rows = file_rows(&Bundle { names: Vec::new(), files: b.files.clone(), ..Default::default() });
    let mut contents: Vec<(String, String)> = b.contents.iter().filter_map(|p| rows.iter().find(|f| f.path == *p).map(|f| (f.sha256.clone(), p.clone()))).collect();
    contents.sort_by(|x, y| (x.0.as_bytes(), x.1.as_bytes()).cmp(&(y.0.as_bytes(), y.1.as_bytes())));
    contents.dedup();
    Value::Obj(vec![
        (Field::Entries.as_str().to_string(), Value::Arr(ids.into_iter().map(Value::Str).collect())),
        (
            Field::Files.as_str().to_string(),
            Value::Arr(rows.into_iter().map(|f| row(vec![(Field::Path, Value::Str(f.path)), (Field::Sha256, Value::Str(f.sha256)), (Field::Size, Value::Int(f.size))])).collect()),
        ),
        (
            Field::Contents.as_str().to_string(),
            Value::Arr(contents.into_iter().map(|(c, p)| row(vec![(Field::Content, Value::Str(c)), (Field::Path, Value::Str(p))])).collect()),
        ),
        (Field::NoteMd.as_str().to_string(), Value::Str(b.note.clone())),
        (Field::Proofs.as_str().to_string(), Value::Arr(Vec::new())),
        (Field::Root.as_str().to_string(), Value::Null),
        (Field::Spec.as_str().to_string(), Value::Str(zikaron_kit::tokens::SPEC_KIT.to_string())),
    ])
}

pub fn companion_bytes(b: &Bundle) -> Result<Vec<u8>, Trouble> {
    crate::seam_v2();
    let mut pairs: Vec<(String, Vec<u8>)> = Vec::new();
    for bytes in &b.entries {
        pairs.push((format!("{ENTRIES_DIR}/{}{}", digest(bytes).trim_start_matches("0x"), ENTRY_SUFFIX), bytes.clone()));
    }
    for (p, bytes) in &b.files {
        pairs.push((format!("{FILES_DIR}/{p}"), bytes.clone()));
    }
    pairs.push((MANIFEST.to_string(), json::canon_bytes(&manifest_v1(b))));
    pairs.sort_by(|x, y| x.0.as_bytes().cmp(y.0.as_bytes()));
    pairs.dedup_by(|x, y| x.0 == y.0);
    match zikaron_kit::kitdir::verify_enumeration(&pairs) {
        zikaron_kit::kitdir::KitVerdict::Ok { .. } => Ok(crate::container::encode(&pairs)),
        zikaron_kit::kitdir::KitVerdict::Fail { verdict, subject } => Err(Trouble::Refused(verdict.as_str().to_string(), subject)),
    }
}

pub fn verification_note(b: &Bundle) -> Vec<u8> {
    crate::seam_v2();
    let (entries, files) = (b.entries.len(), b.files.len() + 1);
    let mut t = String::new();
    t.push_str("# 怎么核这一包\n\n");
    t.push_str("1. 逐档算 sha256,对 `manifest.json` 里 `files` 各行的 `sha256`;\n");
    t.push_str("2. `entries/` 下每一档的 sha256 即它的档名(去 `.zk1`),也即 `entries` 表里那一行;\n");
    t.push_str("3. 每一枚条目按它的 `spec` 所名的法(zikaron/1 或 zikaron/2)判;\n");
    t.push_str("4. 清单外的档一件也不该有。\n\n");
    t.push_str(&format!("本束:条目 {entries} 枚 · 档 {files} 件。\n"));
    t.push_str("\n出包时已照 zikaron.kit/2 §4.4 自验,KIT_OK 才落盘。\n");
    t.push_str("\n# How to verify this kit\n\n");
    t.push_str("1. Compute each file's sha256 and compare it with the `sha256` of its row under `files` in `manifest.json`;\n");
    t.push_str("2. Each file under `entries/` is named by its own sha256 (less `.zk1`), which is also its row in the `entries` table;\n");
    t.push_str("3. Judge each entry by the law its `spec` names (zikaron/1 or zikaron/2);\n");
    t.push_str("4. There should be no file outside the manifest.\n\n");
    t.push_str(&format!("This kit: {entries} entries · {files} files.\n"));
    t.push_str("\nBefore it was written, the kit verified itself under zikaron.kit/2 §4.4, and it was written only on KIT_OK.\n");
    t.into_bytes()
}

fn prepare(b: &mut Bundle) -> Result<Vec<String>, Trouble> {
    directories_stand(b)?;
    let dropped = tidy(b)?;
    b.files.push((Slot::Verify.path().to_string(), verification_note(b)));
    duplicates(b)?;
    Ok(dropped)
}

pub fn enumeration(b: &Bundle) -> Vec<(String, Vec<u8>)> {
    crate::seam_v2();
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    for bytes in &b.entries {
        let id = digest(bytes);
        out.push((format!("{ENTRIES_DIR}/{}{}", id.trim_start_matches("0x"), ENTRY_SUFFIX), bytes.clone()));
    }
    for (p, bytes) in &b.files {
        out.push((format!("{FILES_DIR}/{p}"), bytes.clone()));
    }
    out.push((MANIFEST.to_string(), json::canon_bytes(&manifest(b))));
    out.sort_by(|x, y| x.0.as_bytes().cmp(y.0.as_bytes()));
    out.dedup_by(|x, y| x.0 == y.0);
    out
}

pub fn enumerate(mut b: Bundle) -> Result<(Vec<(String, Vec<u8>)>, Landed), Trouble> {
    crate::seam_v2();
    let dropped = prepare(&mut b)?;
    let pairs = enumeration(&b);
    match verify_pairs(&pairs) {
        Ok((entries, files, kit_id)) => Ok((pairs, Landed { entries, files, kit_id, dropped })),
        Err(t) => Err(t),
    }
}

pub fn export(out: &Path, mut b: Bundle) -> Result<Landed, Trouble> {
    crate::seam_v2();
    if out.exists() {
        return Err(Trouble::Occupied(out.to_string_lossy().into_owned()));
    }
    let dropped = prepare(&mut b)?;

    let staging = landing::staging_beside(out)?;
    let _ = std::fs::remove_dir_all(&staging);
    let landed = lay_and_verify(&staging, &b);
    match landed {
        Ok((entries, files, kit_id)) => {
            if let Err(t) = landing::land_tree(out, &staging) {
                let _ = std::fs::remove_dir_all(&staging);
                return Err(Trouble::from(t));
            }
            Ok(Landed {
                entries,
                files,
                kit_id,
                dropped,
            })
        }
        Err(t) => {
            let _ = std::fs::remove_dir_all(&staging);
            Err(t)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    File,
    Folder,
}

impl Form {
    pub fn word(self) -> &'static str {
        match self {
            Form::File => "file",
            Form::Folder => "folder",
        }
    }

    pub fn of(word: &str) -> Option<Form> {
        [Form::File, Form::Folder].into_iter().find(|f| f.word() == word)
    }
}

pub fn export_as(out: &Path, b: Bundle, form: Form) -> Result<Landed, Trouble> {
    crate::seam_v2();
    if form == Form::Folder {
        return export(out, b);
    }
    if out.exists() {
        return Err(Trouble::Occupied(out.to_string_lossy().into_owned()));
    }
    let laid = landing::staging_beside(out)?;
    let landed = export(&laid, b);
    let packed = match landed {
        Ok(l) => crate::container::pack_dir(&laid, out).map(|_| l).map_err(|t| match t {
            crate::container::PackTrouble::Land(t) => Trouble::from(t),
            crate::container::PackTrouble::File(b) => Trouble::Io(out.to_string_lossy().into_owned(), format!("{}: {}", b.code(), b.subject())),
        }),
        Err(t) => Err(t),
    };
    let _ = std::fs::remove_dir_all(&laid);
    packed
}

fn duplicates(b: &Bundle) -> Result<(), Trouble> {
    for list in [b.files.iter().map(|(p, _)| p.clone()).collect::<Vec<_>>()] {
        let mut seen = list.clone();
        seen.sort();
        for i in 1..seen.len() {
            if seen[i - 1] == seen[i] {
                return Err(Trouble::Duplicate(seen[i].clone()));
            }
        }
    }
    Ok(())
}

fn lay_and_verify(staging: &Path, b: &Bundle) -> Result<(usize, usize, String), Trouble> {
    mkdir(staging)?;
    for (rel, bytes) in enumeration(b) {
        put_under(staging, &rel, bytes.as_slice())?;
    }
    match kit2::walk(staging) {
        Ok(en) => verdict_of(kit2::verify_kit(&en)),
        Err(subject) => Err(Trouble::Refused(zikaron_kit::v2::tokens::KitToken::Unreadable.as_str().to_string(), Some(subject))),
    }
}

fn verify_pairs(pairs: &[(String, Vec<u8>)]) -> Result<(usize, usize, String), Trouble> {
    let en: Enumeration = pairs.iter().cloned().collect();
    verdict_of(kit2::verify_kit(&en))
}

fn verdict_of(v: KitVerdict) -> Result<(usize, usize, String), Trouble> {
    match v {
        KitVerdict::Ok { entries, files, kit_id, .. } => Ok((entries, files, kit_id)),
        KitVerdict::Fail { token, subject } => Err(Trouble::Refused(token.as_str().to_string(), subject)),
    }
}

pub fn answer(l: &Landed, out: &str) -> Value {
    Value::Obj(vec![
        (Key::Dropped.as_str().to_string(), Value::Arr(l.dropped.iter().map(|x| Value::Str(x.clone())).collect())),
        (Key::Entries.as_str().to_string(), Value::Int(l.entries as u64)),
        (Key::Files.as_str().to_string(), Value::Int(l.files as u64)),
        (Key::KitId.as_str().to_string(), Value::Str(l.kit_id.clone())),
        (Key::Ok.as_str().to_string(), Value::Bool(true)),
        (Key::Path.as_str().to_string(), Value::Str(out.to_string())),
        (Key::State.as_str().to_string(), Value::Str(zikaron_kit::tokens::KIT_OK.to_string())),
    ])
}

fn put_under(root: &Path, rel: &str, bytes: &[u8]) -> Result<(), Trouble> {
    let full = root.join(rel);
    if let Some(p) = full.parent() {
        mkdir(p)?;
    }
    write(&full, bytes)
}

fn mkdir(p: &Path) -> Result<(), Trouble> {
    landing::mkdir(p).map_err(Trouble::from)
}

fn write(p: &Path, bytes: &[u8]) -> Result<(), Trouble> {
    landing::put(p, bytes).map_err(Trouble::from)
}

pub fn gather(root: &Path, prefix: &str, out: &mut Vec<(String, Vec<u8>)>) -> Result<(), Trouble> {
    crate::seam_v2();
    let read_dir = |e: std::io::Error| Trouble::Io(root.to_string_lossy().into_owned(), format!("read directory {}: {e}", root.display()));
    let listing = std::fs::read_dir(root).map_err(read_dir)?;
    let mut names: Vec<std::ffi::OsString> = Vec::new();
    for item in listing {
        let e = item.map_err(read_dir)?;
        names.push(e.file_name());
    }
    names.sort_by(|a, b| a.as_encoded_bytes().cmp(b.as_encoded_bytes()));
    for name in names {
        let Some(name_str) = name.to_str() else {
            return Err(Trouble::BadPath(root.join(&name).to_string_lossy().into_owned()));
        };
        let rel = if prefix.is_empty() {
            name_str.to_string()
        } else {
            format!("{prefix}/{name_str}")
        };
        let path = root.join(&name);
        let meta = std::fs::symlink_metadata(&path).map_err(|e| Trouble::Io(rel.clone(), format!("read metadata {}: {e}", path.display())))?;
        if meta.file_type().is_symlink() {
            return Err(Trouble::BadPath(rel));
        }
        if meta.is_dir() {
            gather(&path, &rel, out)?;
        } else if meta.is_file() {
            let bytes = std::fs::read(&path).map_err(|e| Trouble::Io(rel.clone(), format!("read {}: {e}", path.display())))?;
            out.push((rel, bytes));
        } else {
            return Err(Trouble::BadPath(rel));
        }
    }
    Ok(())
}
