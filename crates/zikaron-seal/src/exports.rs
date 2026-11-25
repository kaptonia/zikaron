use std::path::Path;
use zikaron::json::Value;

pub const DIR: &str = "kits";
pub const FILE: &str = "exports.json";
pub const INDEX: &str = "index.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Kit,
    GrantPackage,
    Mirror,
    Proofs,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Kit, Kind::GrantPackage, Kind::Mirror];

    pub fn word(self) -> &'static str {
        match self {
            Kind::Kit => "kit",
            Kind::GrantPackage => "grant",
            Kind::Mirror => "mirror",
            Kind::Proofs => "proofs",
        }
    }

    pub fn of(word: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.word() == word)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub kind: Kind,
    pub path: String,
    pub digest: String,
    pub at: u64,
}

pub fn parse(plain: &[u8]) -> Result<Vec<Row>, String> {
    let v = zikaron::json::parse(plain).map_err(|t| format!("{t:?}"))?;
    let Some(Value::Arr(rows)) = v.member("exports") else { return Err("exports".to_string()) };
    rows.iter()
        .map(|r| {
            let text = |k: &str| r.member(k).and_then(Value::as_str).map(str::to_string).ok_or_else(|| k.to_string());
            let kind = Kind::of(&text("kind")?).ok_or_else(|| "kind".to_string())?;
            let at = match r.member("at") {
                Some(Value::Int(n)) => *n,
                _ => return Err("at".to_string()),
            };
            Ok(Row { kind, path: text("path")?, digest: text("digest")?, at })
        })
        .collect()
}

pub fn render(rows: &[Row]) -> Vec<u8> {
    let arr = rows
        .iter()
        .map(|r| {
            Value::Obj(vec![
                ("at".to_string(), Value::Int(r.at)),
                ("digest".to_string(), Value::Str(r.digest.clone())),
                ("kind".to_string(), Value::Str(r.kind.word().to_string())),
                ("path".to_string(), Value::Str(r.path.clone())),
            ])
        })
        .collect();
    zikaron::json::canon_bytes(&Value::Obj(vec![("exports".to_string(), Value::Arr(arr))]))
}

pub fn digest_of(path: &Path) -> Option<String> {
    let md = std::fs::symlink_metadata(path).ok()?;
    if md.is_file() {
        return std::fs::read(path).ok().map(|b| zikaron::hexfmt::encode(&zikaron::cryptox::sha256(&b)));
    }
    if !md.is_dir() {
        return None;
    }
    let (rows, skipped) = zikaron_glue::recording::dir_rows(path).ok()?;
    if skipped > 0 {
        return None;
    }
    Some(zikaron::hexfmt::encode(&zikaron_glue::recording::dir_content(&rows).0))
}

pub fn listed(rows: &[Row], kind: Kind, path: &Path, at: u64) -> Option<Vec<Row>> {
    let digest = digest_of(path)?;
    let place = path.display().to_string();
    let mut out: Vec<Row> = rows.iter().filter(|r| r.path != place).cloned().collect();
    out.push(Row { kind, path: place, digest, at });
    Some(out)
}

pub fn index_at(machine: &Path) -> Result<Option<Vec<u8>>, crate::Trouble> {
    let p = machine.join(DIR).join(INDEX);
    match std::fs::read(&p) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(crate::Trouble::io(&e, &p.display().to_string())),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub id: String,
    pub path: String,
    pub link: Option<String>,
    pub sealed: bool,
}

pub fn index_rows(index: &[u8]) -> Result<Vec<Listed>, crate::Trouble> {
    let shape = |what: &str| crate::Trouble::new(crate::trouble::Code::SettingsShape, format!("{DIR}/{INDEX}: {what}"));
    let v = zikaron::json::parse(index).map_err(|t| shape(&format!("{t:?}")))?;
    let Some(Value::Arr(rows)) = v.member("kits") else { return Err(shape("kits")) };
    Ok(rows
        .iter()
        .filter_map(|r| {
            let text = |k: &str| r.member(k).and_then(Value::as_str).map(str::to_string);
            Some(Listed { id: text("id")?, path: text("path")?, link: text("link"), sealed: r.member("oneTimeKey").is_some() })
        })
        .collect())
}

pub fn index_without(index: &[u8], removed: &[String]) -> Option<Vec<u8>> {
    let Value::Obj(mut members) = zikaron::json::parse(index).ok()? else { return None };
    for (k, v) in members.iter_mut() {
        if k == "kits" {
            if let Value::Arr(rows) = v {
                rows.retain(|r| !r.member("path").and_then(Value::as_str).is_some_and(|p| removed.iter().any(|x| x == p)));
            }
        }
    }
    Some(zikaron::json::canon_bytes(&Value::Obj(members)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub kind: Kind,
    pub path: String,
    pub link: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Why {
    Changed,
    Missing,
    Unremoved(String),
    Foreign,
}

impl Why {
    pub fn word(&self) -> &'static str {
        match self {
            Why::Changed => "changed",
            Why::Foreign => "foreign",
            Why::Missing => "missing",
            Why::Unremoved(_) => "unremoved",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    pub remove: Vec<Item>,
    pub keep: Vec<(Item, Why)>,
}

fn kit_id_at(path: &Path) -> Option<String> {
    if zikaron_glue::container::is_container_at(path) {
        let out = zikaron_glue::read::unpack(path).ok()?;
        return zikaron_glue::book::kit_verdict(&out.dir).ok();
    }
    if path.is_dir() {
        return zikaron_glue::book::kit_verdict(path).ok();
    }
    None
}

pub fn plan(index: &[Listed], list: &[Row]) -> Plan {
    let mut p = Plan::default();
    for r in index.iter().filter(|r| !r.sealed) {
        let item = Item { kind: Kind::Kit, path: r.path.clone(), link: r.link.clone() };
        let at = Path::new(&r.path);
        let kept = if std::fs::symlink_metadata(at).is_err() {
            Some(Why::Missing)
        } else if kit_id_at(at).is_some_and(|id| id.eq_ignore_ascii_case(&r.id)) {
            None
        } else {
            Some(Why::Changed)
        };
        match &kept {
            None => p.remove.push(item),
            Some(why) => p.keep.push((item, why.clone())),
        }
        with_proofs(&mut p, at, kept);
    }
    for r in list {
        let item = Item { kind: r.kind, path: r.path.clone(), link: None };
        let at = Path::new(&r.path);
        if std::fs::symlink_metadata(at).is_err() {
            p.keep.push((item, Why::Missing));
        } else if digest_of(at).is_some_and(|d| d.eq_ignore_ascii_case(&r.digest)) {
            p.remove.push(item);
        } else {
            p.keep.push((item, Why::Changed));
        }
    }
    p
}

fn with_proofs(p: &mut Plan, package: &Path, kept: Option<Why>) {
    let beside = zikaron_glue::anchorproofs::beside(package);
    if std::fs::symlink_metadata(&beside).is_err() {
        return;
    }
    let folder = Item { kind: Kind::Proofs, path: beside.display().to_string(), link: None };
    match kept {
        Some(Why::Missing) | None if only_proofs(&beside) => p.remove.push(folder),
        Some(Why::Missing) | None => p.keep.push((folder, Why::Foreign)),
        Some(why) => p.keep.push((folder, why)),
    }
}

fn only_proofs(folder: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(folder) else { return false };
    entries.into_iter().all(|e| {
        let Ok(e) = e else { return false };
        let name = e.file_name().to_string_lossy().into_owned();
        let named = name.strip_suffix(".json").and_then(zikaron::hexfmt::decode).and_then(|b| <[u8; 32]>::try_from(b).ok()).is_some_and(|h| zikaron_glue::anchorproofs::file_of(&h) == name);
        named && e.file_type().is_ok_and(|t| t.is_file())
    })
}

pub fn remove(p: Plan) -> Plan {
    let mut done = Plan { remove: Vec::new(), keep: p.keep };
    for item in p.remove {
        let at = Path::new(&item.path);
        let gone = if at.is_dir() { std::fs::remove_dir_all(at) } else { std::fs::remove_file(at) };
        match gone {
            Ok(()) => done.remove.push(item),
            Err(e) => done.keep.push((item, Why::Unremoved(e.to_string()))),
        }
    }
    done
}

impl Plan {
    pub fn removed_paths(&self) -> Vec<String> {
        self.remove.iter().map(|i| i.path.clone()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_proofs_beside_a_package_go_with_it() {
        let dir = std::env::temp_dir().join(format!("zk-clean-proofs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let package = dir.join("kit-a.zkit");
        std::fs::create_dir_all(&package).expect("package");
        let beside = zikaron_glue::anchorproofs::beside(&package);
        std::fs::create_dir_all(&beside).expect("folder");
        std::fs::write(beside.join(zikaron_glue::anchorproofs::file_of(&[0x11; 32])), b"{}").expect("proof");
        let plan_of = |kept: Option<Why>| {
            let mut p = Plan::default();
            with_proofs(&mut p, &package, kept);
            p
        };
        let words = |p: &Plan| (p.remove.iter().map(|i| i.kind.word()).collect::<Vec<_>>(), p.keep.iter().map(|(i, w)| (i.kind.word(), w.word())).collect::<Vec<_>>());
        assert_eq!(words(&plan_of(None)), (vec!["proofs"], vec![]), "removed with its package");
        assert_eq!(words(&plan_of(Some(Why::Changed))), (vec![], vec![("proofs", "changed")]), "kept with its package");
        assert_eq!(words(&plan_of(Some(Why::Missing))), (vec!["proofs"], vec![]), "the package moved away: the folder left is removed");
        std::fs::write(beside.join("notes.txt"), b"mine").expect("someone's file");
        assert_eq!(words(&plan_of(None)), (vec![], vec![("proofs", "foreign")]), "a file not a proof: the folder stays");
        assert_eq!(words(&plan_of(Some(Why::Missing))), (vec![], vec![("proofs", "foreign")]), "the package moved away, the folder holding another file: it stays");
        let _ = std::fs::remove_file(beside.join("notes.txt"));
        let mut p = Plan { remove: vec![Item { kind: Kind::Kit, path: package.display().to_string(), link: None }], keep: Vec::new() };
        with_proofs(&mut p, &package, None);
        let done = remove(p);
        assert!(!package.exists() && !beside.exists(), "carried out: the package and its folder gone");
        assert_eq!(done.remove.len(), 2);
        let mut none = Plan::default();
        with_proofs(&mut none, &dir.join("kit-b.zkit"), None);
        assert!(none.remove.is_empty() && none.keep.is_empty(), "no folder, nothing listed");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_index_that_does_not_read_is_refused_by_name() {
        let good = br#"{"kits":[{"id":"0x01","path":"/p/a.zkit"},{"id":"0x02","path":"/p/b.zkit","link":"https://x/b","oneTimeKey":"k"}]}"#;
        let rows = index_rows(good).expect("reads");
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[1].path.as_str(), rows[1].link.as_deref(), rows[1].sealed), ("/p/b.zkit", Some("https://x/b"), true));
        for (bad, what) in [(&b"not a list"[..], "not JSON"), (&br#"{"exports":[]}"#[..], "no kits list")] {
            let t = index_rows(bad).err().unwrap_or_else(|| panic!("{what}: read as rows"));
            assert_eq!(t.code, crate::trouble::Code::SettingsShape, "{what}");
            assert!(t.tail.starts_with(&format!("{DIR}/{INDEX}: ")), "{what}: {}", t.tail);
        }
    }
}
