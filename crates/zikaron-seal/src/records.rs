use zikaron::json::{self, Value};

pub const FORM: &str = "zikaron-desk/records/1";
pub const DIR: &str = "records";
pub const FILE: &str = "index.json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub root: String,
    pub content: String,
    pub id: String,
    pub seq: u64,
    pub name: String,
    pub path: String,
}

fn str_of(v: &Value, k: &str) -> Option<String> {
    match v.member(k) {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    }
}

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

pub fn withdrawn(items: &[Vec<u8>]) -> Vec<String> {
    zikaron_glue::retraction::read_items(items).1.deleted.into_keys().collect()
}

#[derive(Default)]
pub struct Took {
    pub original: Option<zikaron_glue::grantfile::Original>,
    pub place: Option<String>,
    pub taken: Vec<String>,
    pub left_out: Vec<String>,
}

pub fn take(rows: &[Row], items: &[Vec<u8>], work: &str) -> Took {
    use zikaron_glue::grantfile::Original;
    let w = work.trim().to_ascii_lowercase();
    let root = zikaron_glue::book::root_entry(items).map(|e| e.id_hex()).unwrap_or_default();
    let ids: Vec<String> = items
        .iter()
        .filter_map(|b| zikaron_glue::book::check(b).ok())
        .filter(|e| zikaron_glue::recordcheck::work_of(e.kind, &e.body).is_some_and(|x| x.eq_ignore_ascii_case(&w)))
        .map(|e| e.id_hex())
        .collect();
    let is_work = |d: [u8; 32]| zikaron::hexfmt::encode(&d).eq_ignore_ascii_case(&w);
    let mut out = Took::default();
    for row in originals_for(rows, &root, &ids) {
        let at = std::path::Path::new(&row.path);
        let name = at.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let got = match std::fs::symlink_metadata(at) {
            Ok(m) if m.is_file() => std::fs::read(at).ok().filter(|b| is_work(zikaron_glue::recording::content_of(b))).map(|b| Original::File(name.clone(), b)),
            Ok(m) if m.is_dir() => zikaron_glue::recording::dir_rows(at)
                .ok()
                .filter(|(r, skipped)| *skipped == 0 && zikaron_glue::recording::dir_contents(r).into_iter().any(|(d, _)| is_work(d)))
                .and_then(|_| {
                    let mut files = Vec::new();
                    zikaron_glue::pack::gather(at, "", &mut files).ok().map(|_| Original::Dir(name.clone(), files))
                }),
            _ => None,
        };
        match got {
            Some(o) => {
                out.taken.push(zikaron_glue::kitnames::kit_segment(&name).unwrap_or(name));
                out.place = Some(row.path.clone());
                out.original = Some(o);
                break;
            }
            None => out.left_out.push(row.path.clone()),
        }
    }
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
