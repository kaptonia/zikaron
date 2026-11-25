pub const NAMES_FILE: &str = "zikaron-names.json";

pub const DIGEST_HEX: usize = 8;

pub fn kit_segment(orig: &str) -> Option<String> {
    if zikaron_kit::kitdir::is_kit_path(orig) && !orig.contains('/') {
        return Some(orig.to_string());
    }
    let squash = |x: &str| -> String {
        let mut out = String::new();
        let mut dash = false;
        for c in x.chars().flat_map(|c| c.to_lowercase()) {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.' {
                out.push(c);
                dash = false;
            } else if !dash {
                out.push('-');
                dash = true;
            }
        }
        out.trim_matches(|c| c == '-' || c == '.').to_string()
    };
    let (stem, ext) = match orig.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() && !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric()) => (s, Some(e.to_ascii_lowercase())),
        _ => (orig, None),
    };
    let digest = zikaron::hexfmt::encode(&zikaron::cryptox::sha256(orig.as_bytes()));
    let tag = &digest.trim_start_matches("0x")[..DIGEST_HEX];
    let body = squash(stem);
    let name = match (body.is_empty(), ext) {
        (true, Some(e)) => format!("{tag}.{e}"),
        (true, None) => tag.to_string(),
        (false, Some(e)) => format!("{body}-{tag}.{e}"),
        (false, None) => format!("{body}-{tag}"),
    };
    if zikaron_kit::kitdir::is_kit_path(&name) { Some(name) } else { None }
}

pub fn kit_rel(orig: &str) -> Option<String> {
    let segs: Option<Vec<String>> = orig.split('/').map(kit_segment).collect();
    let rel = segs?.join("/");
    if zikaron_kit::kitdir::is_kit_path(&rel) { Some(rel) } else { None }
}

pub mod names_member {
    pub const NAMES: &str = "names";
    pub const FROM: &str = "from";
    pub const TO: &str = "to";
}

pub fn names_read(bytes: &[u8]) -> Vec<(String, String)> {
    use zikaron::json::Value;
    let Ok(v) = zikaron::json::parse(bytes) else { return Vec::new() };
    let Some(Value::Arr(rows)) = v.member(names_member::NAMES) else { return Vec::new() };
    rows.iter()
        .filter_map(|r| match (r.member(names_member::FROM), r.member(names_member::TO)) {
            (Some(Value::Str(from)), Some(Value::Str(to))) => Some((from.clone(), to.clone())),
            _ => None,
        })
        .collect()
}

pub fn names_table(names: &[(String, String)]) -> Vec<u8> {
    use zikaron::json::Value;
    let mut rows: Vec<&(String, String)> = names.iter().collect();
    rows.sort_by(|a, b| a.1.cmp(&b.1));
    let arr = rows
        .into_iter()
        .map(|(from, to)| Value::Obj(vec![(names_member::FROM.to_string(), Value::Str(from.clone())), (names_member::TO.to_string(), Value::Str(to.clone()))]))
        .collect();
    zikaron::json::canon_bytes(&Value::Obj(vec![(names_member::NAMES.to_string(), Value::Arr(arr))]))
}

pub fn put_original(name: &str, bytes: Vec<u8>, into: &mut crate::pack::Bundle) -> Result<String, String> {
    let inside = if name.contains('/') { None } else { kit_rel(name) }.ok_or_else(|| name.to_string())?;
    if inside != name {
        into.names.push((inside.clone(), name.to_string()));
    }
    into.files.push((inside.clone(), bytes));
    into.contents.push(inside.clone());
    Ok(inside)
}

pub fn put_directory(dir_name: &str, files: Vec<(String, Vec<u8>)>, into: &mut crate::pack::Bundle) -> Result<String, String> {
    let top = kit_segment(dir_name).ok_or_else(|| dir_name.to_string())?;
    let mut laid: Vec<(String, String, Vec<u8>)> = Vec::new();
    for (rel, bytes) in files {
        let inside = kit_rel(&rel).ok_or_else(|| rel.clone())?;
        let path = format!("{top}/{inside}");
        if !zikaron_kit::kitdir::is_kit_path(&path) {
            return Err(rel);
        }
        laid.push((rel, path, bytes));
    }
    let mut members = Vec::with_capacity(laid.len());
    for (rel, path, bytes) in laid {
        if path[top.len() + 1..] != rel {
            into.names.push((path.clone(), rel));
        }
        members.push(path.clone());
        into.files.push((path, bytes));
    }
    into.dir_members.push((top.clone(), members));
    into.dir_names.push((top.clone(), dir_name.to_string()));
    into.contents.push(top.clone());
    Ok(top)
}
