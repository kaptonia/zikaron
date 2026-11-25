use crate::fault::{Fault, Known};
use std::path::{Path, PathBuf};
use zikaron::json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Kit(PathBuf),
    Bytes(PathBuf),
    File(PathBuf),
    Addressed(PathBuf),
    Remote(crate::fetchx::Base),
}

impl Source {
    pub fn said(&self) -> String {
        match self {
            Source::Kit(p) | Source::Bytes(p) | Source::File(p) | Source::Addressed(p) => p.display().to_string(),
            Source::Remote(b) => b.as_str().to_string(),
        }
    }
}

pub fn source_of(path: &str) -> Result<Source, Fault> {
    crate::trace::mark(crate::feature::Feature::D2);
    let p = Path::new(path.trim());
    if path.trim().is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail222).to_string())));
    }
    if crate::fetchx::is_address(path) {
        return Ok(Source::Remote(crate::fetchx::base_of(path)?));
    }
    let md = std::fs::symlink_metadata(p).map_err(|e| crate::fault::classify(&e, &p.display().to_string()).at_place(path.trim()))?;
    if md.is_dir() {
        if p.join(zikaron_glue::names::MANIFEST).is_file() {
            Ok(Source::Kit(p.to_path_buf()))
        } else {
            Ok(Source::Bytes(p.to_path_buf()))
        }
    } else if md.is_file() && crate::grantfilex::is_grant_file(p) {
        Ok(Source::File(p.to_path_buf()))
    } else if md.is_file() && zikaron_glue::addressed::is_addressed_at(p) {
        Ok(Source::Addressed(p.to_path_buf()))
    } else if md.is_file() {
        Ok(Source::Bytes(p.to_path_buf()))
    } else {
        Err(Fault::known(Known::NotAdoptable, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail027, &[&(p.display()).to_string()]))).at_place(path.trim()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejected {
    pub file: String,
    pub why: String,
    pub say: Option<crate::lang::Key>,
}

impl Rejected {
    pub fn token(file: impl Into<String>, t: zikaron::tokens::Token) -> Rejected {
        Rejected { file: file.into(), why: format!("{t:?}"), say: Some(crate::fault::entry_token_say(t)) }
    }

    pub fn plain(file: impl Into<String>, why: impl Into<String>) -> Rejected {
        Rejected { file: file.into(), why: why.into(), say: None }
    }

    pub fn of_fault(file: impl Into<String>, f: &Fault) -> Rejected {
        Rejected { file: file.into(), why: format!("{} · {}", f.said(), f.tail()), say: f.what_key() }
    }

    pub fn io(file: impl Into<String>, e: &std::io::Error) -> Rejected {
        let file = file.into();
        let say = crate::fault::classify(e, &file).what_key();
        Rejected { file, why: e.kind().to_string(), say }
    }

    pub fn human(&self) -> String {
        crate::lang::t(self.say.unwrap_or(crate::lang::Key::CheckRejectedUnsaid)).to_string()
    }
}

pub fn entries_of(p: &Path) -> Result<(Vec<Vec<u8>>, Vec<Rejected>), Fault> {
    let mut good: Vec<Vec<u8>> = Vec::new();
    let mut bad: Vec<Rejected> = Vec::new();
    let md = std::fs::symlink_metadata(p).map_err(|e| crate::fault::classify(&e, &p.display().to_string()))?;
    let mut files: Vec<PathBuf> = Vec::new();
    if md.is_file() {
        files.push(p.to_path_buf());
    } else {
        let mut stack = vec![p.to_path_buf()];
        while let Some(at) = stack.pop() {
            let listing = match std::fs::read_dir(&at) {
                Ok(l) => l,
                Err(e) => {
                    bad.push(Rejected::io(at.display().to_string(), &e));
                    continue;
                }
            };
            let mut names: Vec<PathBuf> = listing.filter_map(|e| e.ok().map(|e| e.path())).collect();
            names.sort();
            for q in names {
                let m = match std::fs::symlink_metadata(&q) {
                    Ok(m) => m,
                    Err(e) => {
                        bad.push(Rejected::io(q.display().to_string(), &e));
                        continue;
                    }
                };
                if m.is_dir() {
                    stack.push(q);
                    continue;
                }
                if !m.is_file() {
                    continue;
                }
                let name = q.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                if zikaron_store::layout::parse_entry_file(&name).is_some() {
                    files.push(q);
                }
            }
        }
    }
    for f in files {
        let bytes = std::fs::read(&f).map_err(|e| crate::fault::classify(&e, &f.display().to_string()))?;
        let bytes = if crate::local::is_sealed(&bytes) {
            let name = f.file_name().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
            match crate::keybox::local_key().and_then(|k| crate::local::open_with(&k, &crate::local::expect_entry(f.parent().unwrap_or(&f))?, &bytes, &name)) {
                Ok(plain) => plain,
                Err(e) => {
                    bad.push(Rejected::of_fault(f.display().to_string(), &e));
                    continue;
                }
            }
        } else {
            bytes
        };
        match zikaron_glue::book::check(&bytes) {
            Ok(_) => good.push(bytes),
            Err(t) => bad.push(Rejected::token(f.display().to_string(), t)),
        }
    }
    if good.is_empty() && bad.is_empty() {
        return Err(Fault::known(Known::NoBytes, p.display().to_string()));
    }
    Ok((good, bad))
}

pub fn kit_entries(dir: &Path) -> Result<(Vec<Vec<u8>>, Vec<Rejected>), Fault> {
    use zikaron_glue::names::{Field, ENTRIES_DIR, ENTRY_SUFFIX, MANIFEST};
    let m = dir.join(MANIFEST);
    let raw = std::fs::read(&m).map_err(|e| crate::fault::classify(&e, &m.display().to_string()))?;
    let v = zikaron::json::parse(&raw)
        .map_err(|t| Fault::known(Known::KitRefused, format!("{}:{t:?}", m.display())))?;
    let ids: Vec<String> = match &v {
        Value::Obj(ms) => ms
            .iter()
            .find(|(k, _)| k == Field::Entries.as_str())
            .and_then(|(_, x)| match x {
                Value::Arr(a) => Some(a.iter().filter_map(|i| match i {
                    Value::Str(s) => Some(s.clone()),
                    _ => None,
                }).collect()),
                _ => None,
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let mut good: Vec<Vec<u8>> = Vec::new();
    let mut bad: Vec<Rejected> = Vec::new();
    for id in ids {
        let f = dir.join(ENTRIES_DIR).join(format!("{}{ENTRY_SUFFIX}", id.trim_start_matches("0x")));
        match std::fs::read(&f) {
            Ok(bytes) => match zikaron_glue::book::check(&bytes) {
                Ok(_) => good.push(bytes),
                Err(t) => bad.push(Rejected::token(f.display().to_string(), t)),
            },
            Err(e) => bad.push(Rejected::io(f.display().to_string(), &e)),
        }
    }
    if good.is_empty() && bad.is_empty() {
        return Err(Fault::known(Known::NoBytes, dir.display().to_string()));
    }
    Ok((good, bad))
}

pub fn entries_of_pairs(pairs: &[(String, Vec<u8>)]) -> (Vec<Vec<u8>>, Vec<Rejected>) {
    let dir = format!("{}/", zikaron_glue::names::ENTRIES_DIR);
    let mut good: Vec<Vec<u8>> = Vec::new();
    let mut bad: Vec<Rejected> = Vec::new();
    for (p, b) in pairs.iter().filter(|(p, _)| p.starts_with(&dir)) {
        match zikaron_glue::book::check(b) {
            Ok(_) => good.push(b.clone()),
            Err(t) => bad.push(Rejected::token(p.clone(), t)),
        }
    }
    (good, bad)
}

pub fn kit_facts_of(pairs: &[(String, Vec<u8>)]) -> KitFacts {
    let en: zikaron_kit::v2::kit::Enumeration = pairs.iter().cloned().collect();
    facts(zikaron_kit::v2::dispatch::read_kit(&en))
}

pub fn bytes_or_none(typed: &str) -> Result<Vec<Vec<u8>>, Fault> {
    Ok(bytes_named(typed)?.0)
}

pub fn bytes_named(typed: &str) -> Result<(Vec<Vec<u8>>, Vec<Rejected>), Fault> {
    if typed.trim().is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    source_of(typed).and_then(|s| bytes_at(&s)).map_err(|f| f.at_place(typed.trim()))
}

pub fn bytes_at(source: &Source) -> Result<(Vec<Vec<u8>>, Vec<Rejected>), Fault> {
    match source {
        Source::Kit(d) => kit_entries(d),
        Source::File(p) => match zikaron_glue::read::unpack(p) {
            Ok(out) if !out.dir.join(zikaron_glue::names::FILES_DIR).join(zikaron_glue::grantfile::CODE_FILE).is_file() => kit_entries(&out.dir),
            _ => Ok((crate::grantfilex::open(p)?.ledger, Vec::new())),
        },
        Source::Remote(b) => Ok(entries_of_pairs(&crate::fetchx::fetch_kit(b)?.pairs)),
        Source::Bytes(p) => entries_of(p),
        Source::Addressed(p) => Err(Fault::known(Known::AddressedElsewhere, p.display().to_string())),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KitFacts {
    pub ok: bool,
    pub entries: usize,
    pub files: usize,
    pub proofs: usize,
    pub kit_id: String,
    pub verdict: String,
    pub subject: String,
    pub invalid: Vec<(String, String)>,
}

pub fn verify_kit_at(dir: &Path) -> KitFacts {
    crate::trace::mark(crate::feature::Feature::D2);
    facts(zikaron_kit::v2::dispatch::read_kit_dir(dir))
}

fn facts(v: zikaron_kit::v2::dispatch::KitReading) -> KitFacts {
    use zikaron_kit::v2::dispatch::KitReading;
    use zikaron_kit::v2::kit::KitVerdict as V2;
    match v {
        KitReading::Kit1(v) => facts_v1(v),
        KitReading::Kit2(V2::Ok { kit_id, entries, files, invalid, .. }) => KitFacts {
            ok: true,
            entries,
            files,
            proofs: 0,
            kit_id,
            verdict: String::new(),
            subject: String::new(),
            invalid: invalid.into_iter().map(|(p, t)| (p, format!("{t:?}"))).collect(),
        },
        KitReading::Kit2(V2::Fail { token, subject }) => KitFacts {
            ok: false,
            entries: 0,
            files: 0,
            proofs: 0,
            kit_id: String::new(),
            verdict: token.as_str().to_string(),
            subject: subject.unwrap_or_default(),
            invalid: Vec::new(),
        },
    }
}

fn facts_v1(v: zikaron_kit::kitdir::KitVerdict) -> KitFacts {
    match v {
        zikaron_kit::kitdir::KitVerdict::Ok { entries, files, proofs, invalid, kit_id } => KitFacts {
            ok: true,
            entries,
            files,
            proofs,
            kit_id: zikaron::hexfmt::encode(&kit_id),
            verdict: String::new(),
            subject: String::new(),
            invalid: invalid.into_iter().map(|(p, t)| (p, format!("{t:?}"))).collect(),
        },
        zikaron_kit::kitdir::KitVerdict::Fail { verdict, subject } => KitFacts {
            ok: false,
            entries: 0,
            files: 0,
            proofs: 0,
            kit_id: String::new(),
            verdict: verdict.as_str().to_string(),
            subject: subject.unwrap_or_default(),
            invalid: Vec::new(),
        },
    }
}

pub use zikaron_glue::recordcheck::{contents_of, originals_in_dir, originals_in_pairs, OnChain, Original};

pub fn originals_of(held: &[(zikaron_glue::extract::Original, Option<Vec<u8>>)]) -> Vec<(String, Option<Vec<u8>>)> {
    held.iter().map(|(o, b)| (o.content.clone(), b.clone())).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordRow {
    pub id: String,
    pub content: String,
    pub name: Option<String>,
    pub original: Original,
    pub chain: OnChain,
    pub first: Option<crate::auditx::FirstAnchor>,
    pub deleted: Option<zikaron_glue::recordcheck::Withdrawn>,
    pub notes: Vec<zikaron_glue::recordcheck::NoteOn>,
}

pub fn records(bytes: &[Vec<u8>], originals: &[(String, Option<Vec<u8>>)], fragment: Option<&Value>) -> Vec<RecordRow> {
    crate::trace::mark(crate::feature::Feature::D2);
    let first = fragment.and_then(|f| crate::auditx::read(bytes, f).ok()).map(|r| r.first);
    let read = first.as_ref();
    let names: Vec<(String, Option<String>)> =
        bytes.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).map(|e| (e.id_hex(), crate::ledgerx::facts_of(&e).record_name())).collect();
    zikaron_glue::recordcheck::records(bytes, originals, read)
        .into_iter()
        .map(|r| RecordRow {
            name: names.iter().find(|(id, _)| *id == r.id).and_then(|(_, n)| n.clone()),
            id: r.id,
            content: r.content,
            original: r.original,
            chain: r.chain,
            first: r.first,
            deleted: r.deleted,
            notes: r.notes,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorReview {
    pub anchors: usize,
    pub asked: usize,
    pub unanchored: Vec<String>,
    pub anchored: usize,
    pub unread: Vec<String>,
}

pub fn own_pick(bytes: &[Vec<u8>]) -> zikaron_anchor::scan::Pick {
    let ids: std::collections::BTreeSet<[u8; 32]> = bytes.iter().map(|b| zikaron::cryptox::sha256(b)).collect();
    zikaron_anchor::scan::Pick::only(move |h| ids.contains(h))
}

pub fn own_anchors(bytes: &[Vec<u8>], fragment: &Value, anchors: usize, asked: usize) -> AnchorReview {
    let (ledger, first): (Vec<String>, Vec<String>) = match crate::auditx::read(bytes, fragment) {
        Ok(l) => (l.entries.iter().map(|e| e.id_hex().to_ascii_lowercase()).collect(), l.first.iter().map(|(id, _)| id.to_ascii_lowercase()).collect()),
        Err(_) => (Vec::new(), Vec::new()),
    };
    let unanchored: Vec<String> = ledger.iter().filter(|id| !first.contains(id)).cloned().collect();
    AnchorReview { anchors, asked, anchored: ledger.len() - unanchored.len(), unanchored, unread: Vec::new() }
}

pub fn unread_where_missed(missed: &[crate::widex::Missed], review: &mut Result<AnchorReview, String>, records: &mut [RecordRow]) {
    if missed.is_empty() {
        return;
    }
    if let Ok(r) = review.as_mut() {
        r.unread.append(&mut r.unanchored);
    }
    for row in records.iter_mut().filter(|row| row.chain == OnChain::NotAnchored) {
        row.chain = OnChain::Unread;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GrantRead {
    pub package: Result<(), (String, Option<String>)>,
    pub verdict: Option<String>,
    pub failed: Vec<String>,
    pub delivers: bool,
    pub behind: bool,
    pub lights: Vec<Vec<(String, String)>>,
    pub notes: crate::noticex::Notes,
    pub undecided: bool,
}

pub fn grant_read(pairs: &[(String, Vec<u8>)], path: &std::path::Path, check: Option<&dyn Fn(crate::checkx::Hops) -> crate::checkx::Checked>) -> Option<GrantRead> {
    use zikaron_glue::grantfile::Refused;
    if !zikaron_glue::grantfile::is_package(pairs) {
        return None;
    }
    let o = match zikaron_glue::grantfile::open_pairs(pairs) {
        Ok(o) => o,
        Err(r) => {
            let (t, sub) = match r {
                Refused::Kit(t, s) | Refused::Package(t, s) => (t, s),
                Refused::NoCode(at) => ("E_GRANT_FILE_CODE".to_string(), Some(at)),
                Refused::Uncarried(id) => ("E_GRANT_FILE_CHAIN".to_string(), Some(id)),
                Refused::Code(d) => (d.token.as_str().to_string(), d.index.map(|i| i.to_string())),
                Refused::Shape(b) => (b.code().to_string(), Some(b.subject())),
            };
            return Some(GrantRead { package: Err((t, sub)), verdict: None, failed: Vec::new(), delivers: false, behind: false, lights: Vec::new(), notes: Default::default(), undecided: false });
        }
    };
    let delivers = !o.contents.is_empty();
    let Some(check) = check else {
        let empty = crate::auditx::empty_fragment();
        let mut behind = false;
        let hops: Vec<(Vec<u8>, Option<Value>, Option<Value>)> = o
            .hops
            .iter()
            .zip(zikaron_glue::grantfile::hops_judged(&o, &empty, &|_| None))
            .map(|(h, j)| {
                behind |= j.late;
                (h.clone(), j.input, None)
            })
            .collect();
        let checked = zikaron_glue::book::chain_check(&hops, None);
        let (verdict, failed, lights) = read_checked(&checked);
        return Some(GrantRead { package: Ok(()), verdict, failed, delivers, behind, lights, notes: Default::default(), undecided: false });
    };
    let opened = crate::grantfilex::Opened { hops: o.hops.clone(), ledger: o.ledger, terms: o.terms, publish: o.publish, root: o.root, contents: o.contents, kit_id: o.kit_id, files: o.files };
    let x = check(crate::checkx::Hops { form: crate::checkx::Form::GrantFile, hops: o.hops, file: Some((path.to_path_buf(), opened)) });
    let (_, failed, lights) = read_checked(&x.judged.value);
    let said_behind = crate::lang::t(crate::lang::Key::LedgerBehindBook);
    let behind = x.judged.hops.iter().any(|h| matches!(&h.input, crate::checkx::InputFrom::Refused(w) if w == said_behind));
    Some(GrantRead { package: Ok(()), verdict: Some(x.judged.verdict.clone()), failed, delivers, behind, lights, notes: x.notes.clone(), undecided: !x.undecided.is_empty() })
}

fn read_checked(value: &Value) -> (Option<String>, Vec<String>, Vec<Vec<(String, String)>>) {
    let verdict = value.member("verdict").and_then(Value::as_str).map(str::to_string);
    let mut failed: Vec<String> = Vec::new();
    match value.member("hops") {
        Some(Value::Arr(hs)) => {
            for h in hs {
                if let Some(Value::Arr(f)) = h.member("failed") {
                    failed.extend(f.iter().filter_map(Value::as_str).map(str::to_string));
                }
            }
        }
        _ => {
            if let Some(Value::Arr(f)) = value.member("failed") {
                failed.extend(f.iter().filter_map(Value::as_str).map(str::to_string));
            }
        }
    }
    if let Some(t) = value.member("token").and_then(Value::as_str) {
        failed.push(t.to_string());
    }
    failed.dedup();
    let lights = match value.member("hops") {
        Some(Value::Arr(hs)) => hs.iter().map(crate::checkx::lights).collect(),
        _ => vec![crate::checkx::lights(value)],
    };
    (verdict, failed, lights)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasisRead {
    pub chain: u64,
    pub registries: Vec<String>,
    pub from_block: u64,
    pub to_block: u64,
}

pub fn basis_read(fragment: &Value) -> Vec<BasisRead> {
    let mut out: Vec<BasisRead> = match fragment.member("basis").and_then(|b| b.member("chains")) {
        Some(Value::Arr(a)) => a
            .iter()
            .filter_map(|w| {
                let int = |k: &str| match w.member(k) {
                    Some(Value::Int(n)) => Some(*n),
                    _ => None,
                };
                let registries = match w.member("registries") {
                    Some(Value::Arr(r)) => r.iter().filter_map(Value::as_str).map(str::to_string).collect(),
                    _ => Vec::new(),
                };
                Some(BasisRead { chain: int("chainId")?, registries, from_block: int("fromBlock")?, to_block: int("toBlock")? })
            })
            .collect(),
        _ => Vec::new(),
    };
    out.sort_by_key(|b| b.chain);
    out
}

#[derive(Clone, Debug)]
pub struct Verified {
    pub path: String,
    pub source: Source,
    pub kit: Option<KitFacts>,
    pub entries: usize,
    pub rejected: Vec<Rejected>,
    pub review: Result<AnchorReview, String>,
    pub work: String,
    pub work_found: Option<bool>,
    pub mismatches: Vec<String>,
    pub records: Vec<RecordRow>,
    pub takes: Option<Result<zikaron_glue::extract::Plan, zikaron_glue::extract::Refused>>,
    pub not_added: Option<crate::kitsindex::AnchoredOn>,
    pub missed: Vec<crate::widex::Missed>,
    pub unanswered: Vec<String>,
    pub read: Vec<u64>,
    pub basis: Vec<BasisRead>,
    pub grant: Option<GrantRead>,
    pub filed: Option<Result<String, String>>,
}

pub fn chains_of(fragment: &Value) -> Vec<u64> {
    let mut out: Vec<u64> = match fragment.member("basis").and_then(|b| b.member("chains")) {
        Some(Value::Arr(a)) => a.iter().filter_map(|w| match w.member("chainId") {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        }).collect(),
        _ => Vec::new(),
    };
    out.sort_unstable();
    out.dedup();
    out
}

pub fn manifest_in(pairs: &[(String, Vec<u8>)]) -> Option<Vec<u8>> {
    pairs.iter().find(|(p, _)| p == zikaron_glue::names::MANIFEST).map(|(_, b)| b.clone())
}

pub fn stated_on(manifest: &[u8]) -> Option<crate::kitsindex::AnchoredOn> {
    zikaron_glue::anchoredon::of_manifest(manifest).1
}

pub fn work_of(typed: &str) -> Result<String, Fault> {
    let t = typed.trim();
    if t.is_empty() {
        return Err(Fault::known(Known::ContentShape, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail107).to_string())));
    }
    if !zikaron::hexfmt::is_hex32(t) {
        return Err(Fault::known(Known::ContentShape, t.to_string()));
    }
    Ok(t.to_string())
}

pub fn work_present(bytes: &[Vec<u8>], work: &str) -> bool {
    bytes
        .iter()
        .filter_map(|b| zikaron_glue::book::check(b).ok())
        .filter_map(|e| zikaron_glue::recordcheck::work_of(e.kind, &e.body))
        .any(|c| c.eq_ignore_ascii_case(work))
}

pub fn mismatches(
    kit: Option<&KitFacts>,
    rejected: &[Rejected],
    review: &Result<AnchorReview, String>,
    work_found: Option<bool>,
) -> Vec<String> {
    crate::trace::mark(crate::feature::Feature::D2);
    mismatch_list(kit, rejected, review, work_found).iter().map(Mismatch::machine).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mismatch<'a> {
    Kit { verdict: &'a str, subject: &'a str },
    Invalid { path: &'a str, token: &'a str },
    Rejected(&'a Rejected),
    Unanchored(&'a str),
    WorkAbsent,
}

impl Mismatch<'_> {
    pub fn machine(&self) -> String {
        match self {
            Mismatch::Kit { verdict, subject } => format!("kit {verdict} {subject}"),
            Mismatch::Invalid { path, token } => format!("invalid {path} {token}"),
            Mismatch::Rejected(r) => format!("rejected {} {}", r.file, r.why),
            Mismatch::Unanchored(id) => format!("unanchored {id}"),
            Mismatch::WorkAbsent => "work absent".to_string(),
        }
    }
}

pub fn mismatch_list<'a>(kit: Option<&'a KitFacts>, rejected: &'a [Rejected], review: &'a Result<AnchorReview, String>, work_found: Option<bool>) -> Vec<Mismatch<'a>> {
    let mut out = Vec::new();
    if let Some(k) = kit {
        if !k.ok {
            out.push(Mismatch::Kit { verdict: &k.verdict, subject: &k.subject });
        }
        for (p, t) in &k.invalid {
            out.push(Mismatch::Invalid { path: p, token: t });
        }
    }
    out.extend(rejected.iter().map(Mismatch::Rejected));
    if let Ok(r) = review {
        out.extend(r.unanchored.iter().map(|id| Mismatch::Unanchored(id)));
    }
    if work_found == Some(false) {
        out.push(Mismatch::WorkAbsent);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review(unanchored: &[&str], unread: &[&str]) -> Result<AnchorReview, String> {
        Ok(AnchorReview { anchors: 1, asked: 1, unanchored: unanchored.iter().map(|x| x.to_string()).collect(), anchored: 1, unread: unread.iter().map(|x| x.to_string()).collect() })
    }

    #[test]
    fn the_chain_not_read_is_no_mismatch_and_a_real_one_still_is() {
        let good = KitFacts { ok: true, entries: 2, files: 1, proofs: 0, kit_id: "k".into(), verdict: String::new(), subject: String::new(), invalid: Vec::new() };
        assert!(mismatches(Some(&good), &[], &Err("NO_ENDPOINT".into()), None).is_empty(), "no node answered");
        assert!(mismatches(Some(&good), &[], &review(&[], &["0x01"]), None).is_empty(), "a record's chain not read");
        let bad = KitFacts { ok: false, verdict: "E_FILE".into(), subject: "files/a.txt".into(), ..good.clone() };
        assert_eq!(mismatches(Some(&bad), &[], &Err("NO_ENDPOINT".into()), None), vec!["kit E_FILE files/a.txt".to_string()], "a damaged package, the chain not read: the damage alone");
        assert_eq!(mismatches(Some(&good), &[], &review(&["0x02"], &["0x01"]), None), vec!["unanchored 0x02".to_string()], "unanchored stays a mismatch, not read does not");
    }
}
