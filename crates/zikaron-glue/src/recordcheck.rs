use std::path::Path;
use zikaron::audit::Outcome;
use zikaron::json::Value;

pub fn first_anchored(outcome: &Outcome) -> Vec<(String, u64)> {
    let lines = zikaron_kit::reading::Lines::of(&outcome.ledger);
    let bounds = zikaron_kit::reading::bounds(outcome, &lines);
    bounds.iter().enumerate().filter_map(|(i, t)| t.map(|t| (lines.id(i).to_string(), t))).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FirstAnchor {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_timestamp: u64,
    pub tx: String,
    pub hash: String,
    pub registry: Option<String>,
}

pub fn first_anchors(outcome: &Outcome, fragment: &Value) -> Vec<(String, FirstAnchor)> {
    let lines = zikaron_kit::reading::Lines::of(&outcome.ledger);
    let bounds = zikaron_kit::reading::bounds(outcome, &lines);
    let int = |v: &Value, k: &str| match v.member(k) {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    };
    let text = |v: &Value, k: &str| match v.member(k) {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    };
    let rows: Vec<FirstAnchor> = match fragment.member("anchors") {
        Some(Value::Arr(a)) => a
            .iter()
            .filter(|r| text(r, "verdict").as_deref() == Some(zikaron::tokens::Verdict::Counted.as_str()))
            .filter(|r| text(r, "sender").map(|s| outcome.lineage.iter().any(|l| l.eq_ignore_ascii_case(&s))).unwrap_or(false))
            .filter_map(|r| {
                Some(FirstAnchor {
                    chain_id: int(r, "chainId")?,
                    block_number: int(r, "blockNumber")?,
                    block_timestamp: int(r, "blockTimestamp")?,
                    tx: text(r, "tx")?,
                    hash: text(r, "hash")?,
                    registry: None,
                })
            })
            .collect(),
        _ => Vec::new(),
    };
    let mut out = Vec::new();
    for (i, t) in bounds.iter().enumerate() {
        let Some(t) = *t else { continue };
        let first = outcome
            .counted
            .iter()
            .filter(|a| a.block_timestamp == t)
            .filter(|a| lines.position(&a.hash).map(|p| zikaron_kit::reading::reachable_at(&outcome.ledger, &lines, p, i)).unwrap_or(false))
            .flat_map(|a| rows.iter().filter(move |r| r.hash.eq_ignore_ascii_case(&a.hash) && r.block_timestamp == a.block_timestamp))
            .min_by(|x, y| (x.chain_id, x.block_number, &x.tx).cmp(&(y.chain_id, y.block_number, &y.tx)));
        if let Some(f) = first {
            out.push((lines.id(i).to_string(), f.clone()));
        }
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Original {
    Match,
    Mismatch,
    Missing,
}

impl Original {
    pub fn word(self) -> &'static str {
        match self {
            Original::Match => "match",
            Original::Mismatch => "mismatch",
            Original::Missing => "missing",
        }
    }

    pub fn of_word(w: &str) -> Option<Original> {
        [Original::Match, Original::Mismatch, Original::Missing].into_iter().find(|o| o.word() == w)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnChain {
    Anchored { first_at: u64 },
    NotAnchored,
    Unread,
}

impl OnChain {
    pub fn word(self) -> &'static str {
        match self {
            OnChain::Anchored { .. } => "anchored",
            OnChain::NotAnchored => "notAnchored",
            OnChain::Unread => "unread",
        }
    }

    pub fn of_word(w: &str, first_at: Option<u64>) -> Option<OnChain> {
        match (w, first_at) {
            ("anchored", Some(t)) => Some(OnChain::Anchored { first_at: t }),
            ("notAnchored", None) => Some(OnChain::NotAnchored),
            ("unread", None) => Some(OnChain::Unread),
            _ => None,
        }
    }

    pub fn first_at(self) -> Option<u64> {
        match self {
            OnChain::Anchored { first_at } => Some(first_at),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordRow {
    pub id: String,
    pub content: String,
    pub original: Original,
    pub chain: OnChain,
    pub first: Option<FirstAnchor>,
    pub deleted: Option<Withdrawn>,
    pub notes: Vec<NoteOn>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Withdrawn {
    pub by: String,
    pub seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteOn {
    pub id: String,
    pub seq: u64,
    pub note: String,
}

pub fn contents_of(manifest: &[u8]) -> Vec<(String, String)> {
    contents_named(manifest).into_iter().map(|(c, p, _)| (c, p)).collect()
}

pub fn contents_named(manifest: &[u8]) -> Vec<(String, String, Option<String>)> {
    use crate::names::Field;
    let Ok(v) = zikaron::json::parse(manifest) else { return Vec::new() };
    let Some(Value::Arr(rows)) = v.member(Field::Contents.as_str()) else { return Vec::new() };
    rows.iter()
        .filter_map(|r| {
            let content = match r.member(Field::Content.as_str()) {
                Some(Value::Str(s)) => s.to_ascii_lowercase(),
                _ => return None,
            };
            let path = match r.member(Field::Path.as_str()) {
                Some(Value::Str(s)) => s.clone(),
                _ => return None,
            };
            let name = match r.member(Field::NameMd.as_str()) {
                Some(Value::Str(s)) => Some(s.clone()),
                _ => None,
            };
            Some((content, path, name))
        })
        .collect()
}

pub fn originals_in_dir(dir: &Path) -> Vec<(String, Option<Vec<u8>>)> {
    crate::extract::originals_in_dir(dir).into_iter().map(|(o, b)| (o.content, b)).collect()
}

pub fn originals_in_pairs(pairs: &[(String, Vec<u8>)]) -> Vec<(String, Option<Vec<u8>>)> {
    crate::extract::originals_in_pairs(pairs).into_iter().map(|(o, b)| (o.content, b)).collect()
}

pub fn records(
    bytes: &[Vec<u8>],
    originals: &[(String, Option<Vec<u8>>)],
    read: Option<&std::collections::BTreeMap<String, FirstAnchor>>,
) -> Vec<RecordRow> {
    crate::seam_v2();
    let (entries, reading) = crate::retraction::read_items(bytes);
    let deleted = |id: &str| -> Option<Withdrawn> {
        let (_, (_, by)) = reading.deleted.iter().find(|(h, _)| h.eq_ignore_ascii_case(id))?;
        let seq = entries.iter().find(|e| e.id_hex().eq_ignore_ascii_case(by))?.seq;
        Some(Withdrawn { by: by.clone(), seq })
    };
    let text = |body: &Value, k: &str| match body {
        Value::Obj(m) => m.iter().find(|(x, _)| x == k).and_then(|(_, v)| v.as_str()).map(str::to_string),
        _ => None,
    };
    let notes = |id: &str| -> Vec<NoteOn> {
        entries
            .iter()
            .filter(|e| e.kind == crate::book::Kind::Annotation && text(&e.body, "subject").is_some_and(|s| s.eq_ignore_ascii_case(id)))
            .map(|e| NoteOn { id: e.id_hex(), seq: e.seq, note: text(&e.body, crate::retraction::NOTE_MD).unwrap_or_default() })
            .collect()
    };
    bytes
        .iter()
        .filter_map(|b| crate::book::check(b).ok())
        .filter_map(|e| work_of(e.kind, &e.body).map(|c| (e.id_hex(), c.to_ascii_lowercase())))
        .map(|(id, content)| {
            let original = original_of(&content, originals);
            let found = read.map(|m| m.iter().find(|(h, _)| h.eq_ignore_ascii_case(&id)).map(|(_, f)| f.clone()));
            let chain = match &found {
                None => OnChain::Unread,
                Some(Some(f)) => OnChain::Anchored { first_at: f.block_timestamp },
                Some(None) => OnChain::NotAnchored,
            };
            let first = found.flatten();
            let (deleted, notes) = (deleted(&id), notes(&id));
            RecordRow { id, content, original, chain, first, deleted, notes }
        })
        .collect()
}

pub fn original_of(content: &str, originals: &[(String, Option<Vec<u8>>)]) -> Original {
    let mine: Vec<&Option<Vec<u8>>> = originals.iter().filter(|(c, _)| *c == content).map(|(_, b)| b).collect();
    if mine.iter().any(|b| b.as_ref().map(|x| zikaron::hexfmt::encode(&crate::recording::content_of(x)).eq_ignore_ascii_case(content)).unwrap_or(false)) {
        Original::Match
    } else if mine.iter().any(|b| b.is_some()) {
        Original::Mismatch
    } else {
        Original::Missing
    }
}

pub fn work_of(kind: crate::book::Kind, body: &Value) -> Option<String> {
    if kind != crate::book::Kind::History {
        return None;
    }
    match body.member("content") {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    }
}
