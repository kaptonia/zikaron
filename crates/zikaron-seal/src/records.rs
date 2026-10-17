//! The local index of signed files (`records/index.json` in the machine directory, sealed as `Doc::Records`):
//! which file was signed as which record. Its form is read and written here only; the desktop keeps it as it
//! signs, and the desktop's record package export and the command line's (with the credential) take originals
//! from it alike ([`originals_for`]).
//!
//! A row holds only what was fixed at signing (ledger, digest, entry id, `seq`, file name, path). Anchor state
//! is not stored, because it would go stale.

use zikaron::json::{self, Value};

/// Format identifier written into the index file.
pub const FORM: &str = "zikaron-desk/records/1";
/// The index's room in the machine directory, and its file there.
pub const DIR: &str = "records";
pub const FILE: &str = "index.json";

/// One row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// Which ledger (genesis id).
    pub root: String,
    /// `content` (hex32).
    pub content: String,
    /// Entry id.
    pub id: String,
    pub seq: u64,
    /// File name (last segment).
    pub name: String,
    /// The absolute path at signing time.
    pub path: String,
}

fn str_of(v: &Value, k: &str) -> Option<String> {
    match v.member(k) {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Read the index's plain bytes. Anything but this form is refused (its reason in words).
pub fn parse(plain: &[u8]) -> Result<Vec<Row>, String> {
    let v = json::parse(plain).map_err(|t| format!("{t:?}"))?;
    if str_of(&v, "form").as_deref() != Some(FORM) {
        return Err("form".to_string());
    }
    let Some(Value::Arr(rows)) = v.member("rows") else { return Err("rows".to_string()) };
    rows.iter()
        .map(|r| {
            Some(Row {
                root: str_of(r, "root")?,
                content: str_of(r, "content")?,
                id: str_of(r, "id")?,
                seq: match r.member("seq") {
                    Some(Value::Int(n)) => *n,
                    _ => return None,
                },
                name: str_of(r, "name")?,
                path: str_of(r, "path")?,
            })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "row".to_string())
}

/// The index's plain bytes for these rows (canonical JSON).
pub fn render(rows: &[Row]) -> Vec<u8> {
    let v = Value::Obj(vec![
        ("form".to_string(), Value::Str(FORM.to_string())),
        (
            "rows".to_string(),
            Value::Arr(
                rows.iter()
                    .map(|r| {
                        Value::Obj(vec![
                            ("content".to_string(), Value::Str(r.content.clone())),
                            ("id".to_string(), Value::Str(r.id.clone())),
                            ("name".to_string(), Value::Str(r.name.clone())),
                            ("path".to_string(), Value::Str(r.path.clone())),
                            ("root".to_string(), Value::Str(r.root.clone())),
                            ("seq".to_string(), Value::Int(r.seq)),
                        ])
                    })
                    .collect(),
            ),
        ),
    ]);
    json::canon_bytes(&v)
}

/// The rows of one ledger (`root`, its genesis id) for these entries (`ids`), by `seq` then path, each once: the
/// originals a record package of those entries takes from this machine.
pub fn originals_for(rows: &[Row], root: &str, ids: &[String]) -> Vec<Row> {
    let mut out: Vec<Row> = rows
        .iter()
        .filter(|r| r.root.eq_ignore_ascii_case(root) && ids.iter().any(|i| i.eq_ignore_ascii_case(&r.id)))
        .cloned()
        .collect();
    out.sort_by(|a, b| a.seq.cmp(&b.seq).then(a.path.cmp(&b.path)));
    out.dedup_by(|a, b| a.id == b.id && a.path == b.path);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_index_reads_back_what_it_wrote_and_refuses_another_form() {
        let row = |root: &str, id: &str, seq: u64, path: &str| Row { root: root.into(), content: "0xc".into(), id: id.into(), seq, name: "n".into(), path: path.into() };
        let rows = vec![row("0xR", "0xb", 2, "/b"), row("0xr", "0xA", 1, "/a"), row("0xother", "0xa", 0, "/o")];
        assert_eq!(parse(&render(&rows)), Ok(rows.clone()));
        assert!(parse(br#"{"form":"x","rows":[]}"#).is_err());
        assert!(parse(br#"{"form":"zikaron-desk/records/1","rows":[{"id":"0x1"}]}"#).is_err());
        let picked = originals_for(&rows, "0xr", &["0xa".into(), "0xB".into()]);
        assert_eq!(picked.iter().map(|r| r.path.as_str()).collect::<Vec<_>>(), ["/a", "/b"], "只这一本账、只所选条目、按序");
    }
}
