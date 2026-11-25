use super::doc::{self, Accepted};
use super::tokens::{self as t2, GpToken, Key, KitToken, Rule};
use crate::kitdir::is_kit_path;
use crate::tokens::{self as t1, BadgeDecodeToken};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::tokens::Token;
use zikaron::trace;

pub type Enumeration = BTreeMap<String, Vec<u8>>;

fn obj(members: Vec<(Key, Value)>) -> Value {
    Value::Obj(members.into_iter().map(|(k, v)| (k.as_str().to_string(), v)).collect())
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

pub fn entry_path(id: &str) -> String {
    format!("entries/{}.zk1", id.strip_prefix("0x").unwrap_or(id))
}

pub fn file_path(path: &str) -> String {
    format!("files/{path}")
}

pub fn walk(dir: &Path) -> Result<Enumeration, String> {
    let mut out = Enumeration::new();
    walk_into(dir, "", &mut out)?;
    Ok(out)
}

fn here(rel: &str) -> String {
    if rel.is_empty() { ".".to_string() } else { rel.to_string() }
}

#[cfg(unix)]
fn name_bytes(s: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    s.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn name_bytes(s: &std::ffi::OsStr) -> Vec<u8> {
    s.as_encoded_bytes().to_vec()
}

fn walk_into(dir: &Path, rel: &str, out: &mut Enumeration) -> Result<(), String> {
    let listing = std::fs::read_dir(dir).map_err(|_| here(rel))?;
    let mut names: Vec<std::ffi::OsString> = Vec::new();
    for item in listing {
        names.push(item.map_err(|_| here(rel))?.file_name());
    }
    names.sort_by_key(|n| name_bytes(n));
    for name in &names {
        let Some(n) = name.to_str() else { return Err(here(rel)) };
        let r = if rel.is_empty() { n.to_string() } else { format!("{rel}/{n}") };
        let full = dir.join(name);
        let meta = std::fs::symlink_metadata(&full).map_err(|_| r.clone())?;
        if meta.file_type().is_symlink() {
            return Err(r);
        }
        if meta.is_file() {
            let bytes = std::fs::read(&full).map_err(|_| r.clone())?;
            out.insert(r, bytes);
        } else if meta.is_dir() {
            walk_into(&full, &r, out)?;
        } else {
            return Err(r);
        }
    }
    Ok(())
}

fn is_count(b: &[u8]) -> bool {
    !b.is_empty() && b.iter().all(u8::is_ascii_digit) && !(b.len() > 1 && b[0] == b'0')
}

pub fn parse_one_file(data: &[u8]) -> Result<Enumeration, String> {
    let dot = || ".".to_string();
    if !data.starts_with(t2::ONE_FILE_MAGIC) {
        return Err(dot());
    }
    let n = data.len();
    let mut i = t2::ONE_FILE_MAGIC.len();
    let mut items = Enumeration::new();
    let mut prev: Option<String> = None;
    let mut first = true;
    let lf = |from: usize| data[from..].iter().position(|c| *c == b'\n').map(|p| from + p);
    while i < n {
        let j = lf(i).ok_or_else(dot)?;
        let path = std::str::from_utf8(&data[i..j]).map_err(|_| dot())?.to_string();
        if !is_kit_path(&path) {
            return Err(dot());
        }
        let k = lf(j + 1).ok_or_else(|| path.clone())?;
        let count = &data[j + 1..k];
        if !is_count(count) {
            return Err(path);
        }
        let size: usize = std::str::from_utf8(count).ok().and_then(|c| c.parse().ok()).unwrap_or(usize::MAX);
        let start = k + 1;
        let end = start.checked_add(size).filter(|e| *e < n && data[*e] == b'\n').ok_or_else(|| path.clone())?;
        if first {
            if path != t2::MANIFEST {
                return Err(path);
            }
            first = false;
        } else {
            if items.contains_key(&path) || prev.as_ref().is_some_and(|p| path.as_bytes() <= p.as_bytes()) {
                return Err(path);
            }
            prev = Some(path.clone());
        }
        items.insert(path, data[start..end].to_vec());
        i = end + 1;
    }
    if first {
        return Err(dot());
    }
    Ok(items)
}

#[derive(Clone, Debug)]
pub struct FileRow {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    pub name_md: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ContentsRow {
    pub content: String,
    pub path: String,
    pub name_md: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub root: Option<String>,
    pub entries: Vec<String>,
    pub files: Vec<FileRow>,
    pub contents: Vec<ContentsRow>,
    pub kit_id: String,
}

fn is_hex20(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Str(x)) if hexfmt::is_hex20(x))
}

fn is_hex32(v: Option<&Value>) -> bool {
    matches!(v, Some(Value::Str(x)) if hexfmt::is_hex32(x))
}

fn has_keys(v: &Value, keys: &[&str]) -> bool {
    match v {
        Value::Obj(ms) => ms.len() == keys.len() && ms.iter().all(|(k, _)| keys.contains(&k.as_str())),
        _ => false,
    }
}

fn ascending<T: Ord>(xs: &[T]) -> bool {
    xs.windows(2).all(|w| w[0] <= w[1])
}

fn distinct<T: Ord + Clone>(xs: &[T]) -> bool {
    xs.iter().cloned().collect::<BTreeSet<T>>().len() == xs.len()
}

const REQUIRED: [&str; 6] = ["spec", "root", "entries", "files", "contents", "note_md"];
const OPTIONAL: [&str; 2] = ["anchoredOn", "anchors"];

pub fn check_manifest(b: &[u8]) -> Result<Manifest, Rule> {
    let v = json::accept(b).map_err(|_| Rule::Canonical)?;
    let Value::Obj(ms) = &v else { return Err(Rule::Members) };
    if !REQUIRED.iter().all(|k| ms.iter().any(|(mk, _)| mk == k))
        || !ms.iter().all(|(k, _)| REQUIRED.contains(&k.as_str()) || OPTIONAL.contains(&k.as_str()))
    {
        return Err(Rule::Members);
    }
    let get = |k: &str| v.member(k);

    if get("spec").and_then(Value::as_str) != Some(t2::SPEC_KIT) {
        return Err(Rule::Spec);
    }
    let root = match get("root") {
        Some(Value::Null) => None,
        Some(Value::Str(x)) if hexfmt::is_hex20(x) => Some(x.clone()),
        _ => return Err(Rule::Root),
    };

    let entries: Vec<String> = get("entries")
        .and_then(Value::as_arr)
        .and_then(|a| a.iter().map(|x| x.as_str().filter(|s| hexfmt::is_hex32(s)).map(str::to_string)).collect())
        .ok_or(Rule::Entries)?;
    if !distinct(&entries) || !ascending(&entries) {
        return Err(Rule::Entries);
    }

    let mut files: Vec<FileRow> = Vec::new();
    for r in get("files").and_then(Value::as_arr).ok_or(Rule::Files)? {
        if !has_keys(r, &["path", "sha256", "size"]) && !has_keys(r, &["path", "sha256", "size", "name_md"]) {
            return Err(Rule::Files);
        }
        let path = r.member("path").and_then(Value::as_str).filter(|p| is_kit_path(p)).ok_or(Rule::Files)?;
        if !is_hex32(r.member("sha256")) {
            return Err(Rule::Files);
        }
        let size = r.member("size").and_then(Value::as_int).ok_or(Rule::Files)?;
        let name_md = match r.member("name_md") {
            None => None,
            Some(Value::Str(n)) => Some(n.clone()),
            Some(_) => return Err(Rule::Files),
        };
        files.push(FileRow {
            path: path.to_string(),
            sha256: r.member("sha256").and_then(Value::as_str).unwrap_or_default().to_string(),
            size,
            name_md,
        });
    }
    let fpaths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
    if !distinct(&fpaths) {
        return Err(Rule::Files);
    }
    let under = |p: &str, q: &str| q.len() > p.len() && q.starts_with(p) && q.as_bytes()[p.len()] == b'/';
    if fpaths.iter().any(|p| fpaths.iter().any(|q| under(p, q))) {
        return Err(Rule::Files);
    }
    if !ascending(&fpaths) {
        return Err(Rule::Files);
    }
    let listed: HashMap<&str, &str> = files.iter().map(|f| (f.path.as_str(), f.sha256.as_str())).collect();

    let mut contents: Vec<ContentsRow> = Vec::new();
    for r in get("contents").and_then(Value::as_arr).ok_or(Rule::Contents)? {
        if (!has_keys(r, &["content", "path"]) && !has_keys(r, &["content", "path", "name_md"])) || !is_hex32(r.member("content")) {
            return Err(Rule::Contents);
        }
        let content = r.member("content").and_then(Value::as_str).unwrap_or_default();
        let p = r.member("path").and_then(Value::as_str).filter(|p| is_kit_path(p)).ok_or(Rule::Contents)?;
        let name_md = match r.member("name_md") {
            None => None,
            Some(Value::Str(n)) => Some(n.clone()),
            Some(_) => return Err(Rule::Contents),
        };
        match listed.get(p) {
            Some(h) if *h != content => return Err(Rule::Contents),
            Some(_) => {}
            None if !fpaths.iter().any(|q| under(p, q)) => return Err(Rule::Contents),
            None => {}
        }
        contents.push(ContentsRow { content: content.to_string(), path: p.to_string(), name_md });
    }
    let dir_origs: Vec<&str> = contents.iter().map(|c| c.path.as_str()).filter(|p| !listed.contains_key(p)).collect();
    if dir_origs.iter().any(|p| dir_origs.iter().any(|q| under(p, q))) {
        return Err(Rule::Contents);
    }
    for p in &dir_origs {
        let names = directory_names(&files, p);
        if !distinct(&names) {
            return Err(Rule::Contents);
        }
        if names.iter().any(|nm| nm.split('/').any(|sg| sg.is_empty() || sg == "." || sg == "..")) {
            return Err(Rule::Contents);
        }
        if names.iter().any(|nm| names.iter().any(|o| under(nm, o))) {
            return Err(Rule::Contents);
        }
    }
    let row_keys: Vec<(&[u8], &[u8])> = contents.iter().map(|c| (c.content.as_bytes(), c.path.as_bytes())).collect();
    if !distinct(&row_keys) || !ascending(&row_keys) {
        return Err(Rule::Contents);
    }

    if let Some(ao) = get("anchoredOn") {
        if !has_keys(ao, &["chainId", "registry", "fromBlock"])
            || ao.member("chainId").and_then(Value::as_int).is_none()
            || !is_hex20(ao.member("registry"))
            || ao.member("fromBlock").and_then(Value::as_int).is_none()
        {
            return Err(Rule::AnchoredOn);
        }
    }

    if let Some(an) = get("anchors") {
        let mut rows: Vec<(&[u8], u64, &[u8])> = Vec::new();
        for r in an.as_arr().ok_or(Rule::Anchors)? {
            if !has_keys(r, &["entry_id", "chainId", "tx"]) {
                return Err(Rule::Anchors);
            }
            let id = r.member("entry_id").and_then(Value::as_str).filter(|x| hexfmt::is_hex32(x)).ok_or(Rule::Anchors)?;
            if !entries.iter().any(|e| e == id) {
                return Err(Rule::Anchors);
            }
            let cid = r.member("chainId").and_then(Value::as_int).ok_or(Rule::Anchors)?;
            let tx = r.member("tx").and_then(Value::as_str).filter(|x| hexfmt::is_hex32(x)).ok_or(Rule::Anchors)?;
            rows.push((id.as_bytes(), cid, tx.as_bytes()));
        }
        if !distinct(&rows) || !ascending(&rows) {
            return Err(Rule::Anchors);
        }
    }

    if !matches!(get("note_md"), Some(Value::Str(_))) {
        return Err(Rule::NoteMd);
    }

    Ok(Manifest { root, entries, files, contents, kit_id: doc::doc_id(b) })
}

fn directory_names(files: &[FileRow], dir: &str) -> Vec<String> {
    let pre = format!("{dir}/");
    files
        .iter()
        .filter(|f| f.path.starts_with(&pre))
        .map(|f| f.name_md.clone().unwrap_or_else(|| f.path[pre.len()..].to_string()))
        .collect()
}

pub fn directory_manifest_digest(files: &[FileRow], dir: &str) -> String {
    let pre = format!("{dir}/");
    let mut rows: Vec<(String, &str)> = files
        .iter()
        .filter(|f| f.path.starts_with(&pre))
        .map(|f| (f.name_md.clone().unwrap_or_else(|| f.path[pre.len()..].to_string()), f.sha256.as_str()))
        .collect();
    rows.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    let doc = obj(vec![(
        Key::Files,
        Value::Arr(rows.into_iter().map(|(nm, h)| obj(vec![(Key::Path, Value::Str(nm)), (Key::Sha256, s(h))])).collect()),
    )]);
    doc::doc_id(&json::canon_bytes(&doc))
}

#[derive(Clone, Debug)]
pub enum KitVerdict {
    Ok {
        kit_id: String,
        entries: usize,
        files: usize,
        invalid: Vec<(String, Token)>,
        manifest: Manifest,
    },
    Fail {
        token: KitToken,
        subject: Option<String>,
    },
}

fn fail(token: KitToken, subject: &str) -> KitVerdict {
    KitVerdict::Fail { token, subject: Some(subject.to_string()) }
}

pub fn verify_kit(en: &Enumeration) -> KitVerdict {
    trace::mark(t2::K4);
    let Some(mb) = en.get(t2::MANIFEST) else {
        return KitVerdict::Fail { token: KitToken::ManifestAbsent, subject: None };
    };
    let man = match check_manifest(mb) {
        Ok(m) => m,
        Err(rule) => return fail(KitToken::Manifest, rule.as_str()),
    };
    for id in &man.entries {
        match en.get(&entry_path(id)) {
            Some(b) if doc::doc_id(b) == *id => {}
            _ => return fail(KitToken::EntryBytes, id),
        }
    }
    for f in &man.files {
        match en.get(&file_path(&f.path)) {
            Some(b) if doc::doc_id(b) == f.sha256 && b.len() as u64 == f.size => {}
            _ => return fail(KitToken::File, &f.path),
        }
    }
    let listed: BTreeSet<&str> = man.files.iter().map(|f| f.path.as_str()).collect();
    for c in &man.contents {
        if !listed.contains(c.path.as_str()) && directory_manifest_digest(&man.files, &c.path) != c.content {
            return fail(KitToken::Content, &c.path);
        }
    }
    let mut named: BTreeSet<String> = BTreeSet::new();
    named.insert(t2::MANIFEST.to_string());
    named.extend(man.entries.iter().map(|id| entry_path(id)));
    named.extend(man.files.iter().map(|f| file_path(&f.path)));
    if let Some(extra) = en.keys().find(|p| !named.contains(*p)) {
        return fail(KitToken::Extra, extra);
    }
    let invalid: Vec<(String, Token)> = man
        .entries
        .iter()
        .filter_map(|id| en.get(&entry_path(id)).and_then(|b| doc::accept(b).err()).map(|tok| (id.clone(), tok)))
        .collect();
    KitVerdict::Ok { kit_id: man.kit_id.clone(), entries: man.entries.len(), files: man.files.len(), invalid, manifest: man }
}

pub fn kit_verdict_value(v: &KitVerdict) -> Value {
    match v {
        KitVerdict::Ok { kit_id, entries, files, invalid, .. } => obj(vec![
            (Key::Verdict, s(t1::KIT_OK)),
            (Key::KitId, s(kit_id)),
            (Key::Counts, obj(vec![(Key::Entries, Value::Int(*entries as u64)), (Key::Files, Value::Int(*files as u64))])),
            (
                Key::InvalidEntries,
                Value::Arr(invalid.iter().map(|(id, tok)| obj(vec![(Key::EntryId, s(id)), (Key::Token, s(tok.as_str()))])).collect()),
            ),
        ]),
        KitVerdict::Fail { token, subject: None } => obj(vec![(Key::Verdict, s(token.as_str()))]),
        KitVerdict::Fail { token, subject: Some(x) } => obj(vec![(Key::Verdict, s(token.as_str())), (Key::Subject, s(x))]),
    }
}

pub fn verify_grant_package(en: &Enumeration) -> Value {
    trace::mark(t2::K4);
    let failure = |verdict: &str, subject: Value| obj(vec![(Key::Verdict, s(verdict)), (Key::Subject, subject)]);
    let man = match verify_kit(en) {
        KitVerdict::Ok { manifest, .. } => manifest,
        KitVerdict::Fail { token, subject } => return failure(token.as_str(), subject.map(Value::Str).unwrap_or(Value::Null)),
    };
    let Some(code) = en.get(t2::GRANT_CODE) else { return failure(GpToken::NoCode.as_str(), Value::Null) };
    let hops: Vec<Accepted> = match doc::badge_decode(code) {
        Ok(h) => h,
        Err(r) => {
            let per_segment = matches!(
                r.token,
                BadgeDecodeToken::B64 | BadgeDecodeToken::Entry | BadgeDecodeToken::Type | BadgeDecodeToken::Link
            );
            let subject = match r.index {
                Some(k) if per_segment => Value::Int(k as u64),
                _ => Value::Null,
            };
            return failure(r.token.as_str(), subject);
        }
    };
    let root = match &man.root {
        Some(r) => r.clone(),
        None => match hops[0].body().member("ledger") {
            Some(Value::Str(l)) if hexfmt::is_hex20(l) => l.clone(),
            _ => return failure(GpToken::Root.as_str(), Value::Null),
        },
    };
    let ids: Vec<String> = hops.iter().map(Accepted::id_hex).collect();
    if let Some(h) = ids.iter().find(|h| !en.contains_key(&entry_path(h))) {
        return failure(GpToken::Uncarried.as_str(), s(h));
    }
    let work = hops[hops.len() - 1].body().member("work");
    if let Some(c) = man.contents.iter().find(|c| Some(&Value::Str(c.content.clone())) != work) {
        return failure(GpToken::Content.as_str(), s(&c.path));
    }
    let terms_of: Vec<Option<&Value>> = hops.iter().map(|h| h.body().member("terms")).collect();
    let tdocs: Vec<(&str, Value)> = en
        .iter()
        .filter_map(|(p, b)| p.strip_prefix(t2::TERMS_PREFIX).map(|rest| (rest, s(&doc::doc_id(b)))))
        .collect();
    if let Some((p, _)) = tdocs.iter().find(|(_, d)| !terms_of.contains(&Some(d))) {
        return failure(GpToken::Terms.as_str(), s(p));
    }
    let per_hop: Vec<Value> = terms_of
        .iter()
        .map(|t| Value::Arr(tdocs.iter().filter(|(_, d)| Some(d) == *t).map(|(p, _)| s(p)).collect()))
        .collect();
    obj(vec![
        (Key::Verdict, s(t2::GP_OK)),
        (Key::KitId, s(&man.kit_id)),
        (Key::Root, s(&root)),
        (Key::Hops, Value::Arr(ids.iter().map(|h| s(h)).collect())),
        (
            Key::Contents,
            Value::Arr(man.contents.iter().map(|c| obj(vec![(Key::Content, s(&c.content)), (Key::Path, s(&c.path))])).collect()),
        ),
        (Key::Terms, Value::Arr(per_hop)),
    ])
}
