use zikaron::cryptox;

pub const FAMILY: &str = "bytes-sha256/1";

pub fn toolchain() -> [u8; 32] {
    cryptox::sha256(FAMILY.as_bytes())
}

pub fn content_of(bytes: &[u8]) -> [u8; 32] {
    cryptox::sha256(bytes)
}

pub type DirRow = (String, String);

pub const SEPARATOR: &str = "/";

const OLD_WINDOWS_SEPARATOR: &str = "\\";

pub fn dir_path(names: &[&str]) -> String {
    names.join(SEPARATOR)
}

pub fn dir_rows(root: &std::path::Path) -> Result<(Vec<DirRow>, usize), (std::io::Error, std::path::PathBuf)> {
    let mut rows = Vec::new();
    let mut skipped = 0usize;
    walk(root, &mut Vec::new(), &mut rows, &mut skipped)?;
    Ok((rows, skipped))
}

fn walk(at: &std::path::Path, names: &mut Vec<String>, out: &mut Vec<DirRow>, skipped: &mut usize) -> Result<(), (std::io::Error, std::path::PathBuf)> {
    let mut items: Vec<std::path::PathBuf> = std::fs::read_dir(at)
        .map_err(|e| (e, at.to_path_buf()))?
        .map(|e| e.map(|e| e.path()).map_err(|e| (e, at.to_path_buf())))
        .collect::<Result<_, _>>()?;
    items.sort();
    for p in items {
        let md = std::fs::symlink_metadata(&p).map_err(|e| (e, p.clone()))?;
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if md.is_dir() {
            names.push(name);
            let walked = walk(&p, names, out, skipped);
            names.pop();
            walked?;
        } else if md.is_file() {
            let b = std::fs::read(&p).map_err(|e| (e, p.clone()))?;
            let mut path: Vec<&str> = names.iter().map(|n| n.as_str()).collect();
            path.push(&name);
            out.push((dir_path(&path), zikaron::hexfmt::encode(&content_of(&b))));
        } else {
            *skipped += 1;
        }
    }
    Ok(())
}

mod member {
    pub const FILES: &str = "files";
    pub const PATH: &str = "path";
    pub const SHA256: &str = "sha256";
}

pub fn dir_manifest(rows: &[DirRow]) -> zikaron::json::Value {
    use zikaron::json::Value;
    let mut rows: Vec<&DirRow> = rows.iter().collect();
    rows.sort();
    let arr = rows
        .into_iter()
        .map(|(path, sha)| Value::Obj(vec![(member::PATH.to_string(), Value::Str(path.clone())), (member::SHA256.to_string(), Value::Str(sha.clone()))]))
        .collect();
    Value::Obj(vec![(member::FILES.to_string(), Value::Arr(arr))])
}

pub fn dir_content(rows: &[DirRow]) -> ([u8; 32], Vec<u8>) {
    let bytes = zikaron::json::canon_bytes(&dir_manifest(rows));
    (content_of(&bytes), bytes)
}

pub fn dir_contents(rows: &[DirRow]) -> Vec<([u8; 32], Vec<u8>)> {
    std::iter::once(dir_content(rows)).chain(old_dir_content(rows)).collect()
}

pub fn old_dir_content(rows: &[DirRow]) -> Option<([u8; 32], Vec<u8>)> {
    let old: Vec<DirRow> = rows.iter().map(|(p, sha)| (p.split(SEPARATOR).collect::<Vec<_>>().join(OLD_WINDOWS_SEPARATOR), sha.clone())).collect();
    let old = dir_content(&old);
    (old.0 != dir_content(rows).0).then_some(old)
}

pub fn manifest_rows(manifest: &zikaron::json::Value) -> Vec<DirRow> {
    use zikaron::json::Value;
    let Some(Value::Arr(files)) = manifest.member(member::FILES) else { return Vec::new() };
    files
        .iter()
        .filter_map(|f| match (f.member(member::PATH), f.member(member::SHA256)) {
            (Some(Value::Str(p)), Some(Value::Str(s))) => Some((p.clone(), s.clone())),
            _ => None,
        })
        .collect()
}

pub fn dir_holds(rows: &[DirRow], content: &str) -> bool {
    dir_contents(rows).iter().any(|(d, _)| zikaron::hexfmt::encode(d).eq_ignore_ascii_case(content.trim()))
}
