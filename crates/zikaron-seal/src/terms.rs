use crate::envelope::KITS_DIR;
use crate::names::{Logical, NameKey};
use zikaron::json::Value;

pub const RECORD_PREFIX: &str = "grant-";
pub const RECORD_SUFFIX: &str = ".json";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub grant: String,
    pub terms: String,
    pub exclusive: bool,
    pub doc: Option<String>,
    pub name: Option<String>,
}

impl Record {
    pub fn doc_name(&self) -> Option<&str> {
        self.doc.as_deref().and_then(|d| d.rsplit('/').next())
    }

    pub fn shown_name(&self) -> Option<&str> {
        self.doc.as_ref()?;
        self.name.as_deref().or_else(|| self.doc_name())
    }
}

pub fn record_rel(nk: &NameKey, grant: &str) -> String {
    format!("{KITS_DIR}/{}/{RECORD_PREFIX}{}{RECORD_SUFFIX}", zikaron_glue::grantfile::TERMS_ROOM, nk.name(Logical::TermsRecord(grant)))
}

pub fn doc_rel_on_disk(nk: &NameKey, terms: &str) -> String {
    format!("{KITS_DIR}/{}/{}/{}", zikaron_glue::grantfile::TERMS_ROOM, nk.name(Logical::TermsDir(terms)), nk.name(Logical::TermsDoc(terms)))
}

pub fn record_bytes(r: &Record) -> Vec<u8> {
    let mut m: Vec<(String, Value)> = vec![
        ("exclusive".to_string(), Value::Bool(r.exclusive)),
        ("grant".to_string(), Value::Str(r.grant.clone())),
        ("terms".to_string(), Value::Str(r.terms.clone())),
    ];
    if let Some(d) = &r.doc {
        m.push(("doc".to_string(), Value::Str(d.clone())));
    }
    if let Some(n) = &r.name {
        m.push(("name".to_string(), Value::Str(n.clone())));
    }
    m.sort_by(|a, b| a.0.cmp(&b.0));
    zikaron::json::canon_bytes(&Value::Obj(m))
}

pub fn record_of(bytes: &[u8]) -> Option<Record> {
    let v = zikaron::json::parse(bytes).ok()?;
    let s = |k: &str| match v.member(k) {
        Some(Value::Str(x)) => Some(x.clone()),
        _ => None,
    };
    Some(Record {
        grant: s("grant")?,
        terms: s("terms")?,
        exclusive: matches!(v.member("exclusive"), Some(Value::Bool(true))),
        doc: s("doc").filter(|d| zikaron_kit::kitdir::is_kit_path(d)),
        name: s("name").filter(|n| !n.is_empty() && !n.contains('/')),
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
    pub id: String,
    pub work: String,
    pub window: Option<(u64, u64)>,
    pub revoked: bool,
    pub exclusive: bool,
}

pub fn window_of(body: &Value) -> Option<(u64, u64)> {
    let w = body.member("window")?;
    match (w.member("from"), w.member("to")) {
        (Some(Value::Int(a)), Some(Value::Int(b))) => Some((*a, *b)),
        _ => None,
    }
}

pub fn overlaps(a: Option<(u64, u64)>, b: Option<(u64, u64)>) -> bool {
    match (a, b) {
        (Some((a0, a1)), Some((b0, b1))) => a0 <= b1 && b0 <= a1,
        _ => true,
    }
}

pub fn held_of(entries: &[zikaron_glue::book::Entry], records: &[Record]) -> Vec<Held> {
    use zikaron_glue::book::Kind;
    let text = |e: &zikaron_glue::book::Entry, k: &str| e.body.member(k).and_then(Value::as_str).map(str::to_string);
    let revoked: Vec<String> = entries.iter().filter(|e| e.kind == Kind::Revocation).filter_map(|e| text(e, "grant")).collect();
    entries
        .iter()
        .filter(|e| e.kind == Kind::Grant)
        .map(|e| {
            let id = e.id_hex();
            Held {
                work: text(e, "work").unwrap_or_default(),
                window: window_of(&e.body),
                revoked: revoked.iter().any(|r| r.eq_ignore_ascii_case(&id)),
                exclusive: records.iter().any(|r| r.grant.eq_ignore_ascii_case(&id) && r.exclusive),
                id,
            }
        })
        .collect()
}

pub fn clashes(held: &[Held], work: &str, window: Option<(u64, u64)>) -> Vec<String> {
    let w = work.trim();
    held.iter().filter(|h| !h.revoked && h.exclusive && h.work.eq_ignore_ascii_case(w) && overlaps(h.window, window)).map(|h| h.id.clone()).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Withheld {
    Deleted,
    Clash(Vec<String>),
    Undecided(Vec<String>),
}

impl Withheld {
    pub fn token(&self) -> &'static str {
        match self {
            Withheld::Deleted => "E_GRANT_WORK_DELETED",
            Withheld::Clash(_) => "E_GRANT_EXCLUSIVE_CLASH",
            Withheld::Undecided(_) => "E_GRANT_WITHDRAWAL_UNREAD",
        }
    }
}

pub fn may_grant(pile: &[Vec<u8>], records: &[Record], work: &str, window: Option<(u64, u64)>, supersedes: Option<&str>) -> Result<(), Withheld> {
    let Ok(read) = zikaron_glue::book::read(pile, &zikaron_glue::book::empty_fragment()) else { return Ok(()) };
    let w = work.trim();
    let histories: Vec<String> = read.entries.iter().filter(|e| zikaron_glue::recordcheck::work_of(e.kind, &e.body).is_some_and(|x| x.eq_ignore_ascii_case(w))).map(|e| e.id_hex()).collect();
    if zikaron_glue::retraction::work_deleted(&zikaron_glue::retraction::lines_of(&read.entries), &histories) {
        return Err(Withheld::Deleted);
    }
    let stands_in = |id: &str| supersedes.is_some_and(|x| x.trim().eq_ignore_ascii_case(id));
    match clashes(&held_of(&read.entries, records), w, window).into_iter().filter(|id| !stands_in(id)).collect::<Vec<_>>() {
        c if c.is_empty() => Ok(()),
        c => Err(Withheld::Clash(c)),
    }
}

pub fn package_parts<E>(
    chain: &[Vec<u8>],
    ledger: &[Vec<u8>],
    mine: &[Vec<u8>],
    upstream: &[Vec<Vec<u8>>],
    code: String,
    mut terms_of: impl FnMut(&str) -> Result<Option<(String, Vec<u8>)>, E>,
    ground: &crate::ground::Ground,
    last: Option<&crate::lastaudit::Anchored>,
) -> Result<zikaron_glue::grantfile::Parts, E> {
    let mut terms = Vec::new();
    for h in chain {
        if let Some(d) = terms_of(&zikaron::hexfmt::encode(&zikaron_glue::book::entry_id(h)))? {
            terms.push(d);
        }
    }
    Ok(zikaron_glue::grantfile::Parts {
        root: zikaron_glue::grantfile::first_root(chain, mine),
        code,
        hops: chain.to_vec(),
        ledger: ledger.to_vec(),
        terms,
        publish: ground.publish.clone(),
        anchored_on: match (ground.chain_id, ground.registry) {
            (Some(chain_id), Some(r)) => Some(zikaron_glue::anchoredon::AnchoredOn { chain_id, from_block: ground.from_block, registry: zikaron::hexfmt::encode(&r) }),
            _ => None,
        },
        anchors: last.map(|a| a.rows.iter().map(|(entry_id, (chain_id, tx))| zikaron_glue::pack::Anchor { entry_id: entry_id.clone(), chain_id: *chain_id, tx: tx.clone() }).collect()).unwrap_or_default(),
        upstream: upstream.to_vec(),
        ..Default::default()
    })
}
