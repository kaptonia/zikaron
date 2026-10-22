//! How names go into a kit: a name segment and a path made kit-legal (kit law §7.2), and the table of items
//! renamed on the way in (`files/zikaron-names.json`). The desktop's record package and the command line's both
//! name by these.

/// The table mapping original names to kit names, stored in the kit (under `files/`) and listed in the
/// manifest.
pub const NAMES_FILE: &str = "zikaron-names.json";

/// How many hex digits of the original name's digest are appended when transliterating.
pub const DIGEST_HEX: usize = 8;

/// Convert one name segment to a valid kit name segment, as defined by the kit crate's `is_kit_path`
/// (kit law §7.2: lowercase letters, digits, `.` `_` `-`, not starting with `-`, at most 255 bytes).
/// A valid segment is kept as is. Otherwise: lowercase, replace each run of other characters with one `-`,
/// append the first 8 hex digits of the sha256 of the original name (UTF-8) for uniqueness, and keep the
/// extension when valid. `None` when the result is still invalid (too long, for example); the UI then marks
/// it red and disables export.
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

/// Transliterate a relative path segment by segment. The whole path must still pass `is_kit_path`, which
/// also enforces the 1024-byte total length.
pub fn kit_rel(orig: &str) -> Option<String> {
    let segs: Option<Vec<String>> = orig.split('/').map(kit_segment).collect();
    let rel = segs?.join("/");
    if zikaron_kit::kitdir::is_kit_path(&rel) { Some(rel) } else { None }
}

/// Member names of the name table.
pub mod names_member {
    pub const NAMES: &str = "names";
    pub const FROM: &str = "from";
    pub const TO: &str = "to";
}

/// The table read back ([`names_table`]'s shape): (original name, kit name) per row. A table that does not read,
/// and any row not of the shape, gives nothing: an item it does not name keeps its kit name.
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

/// The table of original names to kit names: canonical JSON `{"names":[{"from":original,"to":kit name},…]}`,
/// sorted by kit name.
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


/// Put one original file into a kit under its kit name ([`kit_rel`] of `name`), as a work content of the kit
/// (kit law §7.3); a renamed item is added to `names` (original name, kit name). A name that cannot be made
/// kit-legal is refused (the name), and nothing is put. Whether the bytes are an original of a chosen record is
/// the caller's to have judged first (their digest is that record's `content`).
pub fn put_original(name: &str, bytes: Vec<u8>, into: &mut crate::pack::Bundle, names: &mut Vec<(String, String)>) -> Result<String, String> {
    let inside = kit_rel(name).ok_or_else(|| name.to_string())?;
    if inside != name {
        names.push((name.to_string(), inside.clone()));
    }
    into.files.push((inside.clone(), bytes));
    into.contents.push(inside.clone());
    Ok(inside)
}
