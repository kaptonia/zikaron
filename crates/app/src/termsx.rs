use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use std::path::{Path, PathBuf};

pub use zikaron_glue::grantfile::TERMS_ROOM as ROOM;

pub use zikaron_seal::terms::{record_bytes, record_of, Record, RECORD_PREFIX, RECORD_SUFFIX};

pub fn doc_rel(terms: &str, name: &str) -> Option<String> {
    let seg = crate::kitx::kit_segment(name)?;
    Some(format!("{ROOM}/{}/{seg}", terms.trim().trim_start_matches("0x").to_ascii_lowercase()))
}

pub fn record_path(home: &Home, grant: &str) -> Result<PathBuf, Fault> {
    Ok(home.root().join(zikaron_seal::terms::record_rel(&crate::names::key()?, grant)))
}

pub fn doc_path(home: &Home, terms: &str) -> Result<PathBuf, Fault> {
    Ok(home.root().join(zikaron_seal::terms::doc_rel_on_disk(&crate::names::key()?, terms)))
}

pub fn keep(home: &Home, grant: &str, terms: &str, exclusive: bool, doc: Option<&Path>) -> Result<Record, Fault> {
    let terms = terms.trim().to_ascii_lowercase();
    let mut rel: Option<String> = None;
    let mut picked: Option<String> = None;
    if let Some(p) = doc {
        let bytes = std::fs::read(p).map_err(|e| crate::fault::classify(&e, &p.display().to_string()))?;
        let got = zikaron::hexfmt::encode(&zikaron_kit::doc::doc_id(&bytes));
        if got != terms {
            return Err(Fault::known(Known::TermsMismatch, format!("{got} ≠ {terms}")));
        }
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let r = doc_rel(&terms, &name).ok_or_else(|| Fault::of_landing(zikaron_glue::pack::Trouble::BadPath(name.to_string())))?;
        let at = doc_path(home, &terms)?;
        if !at.exists() {
            if let Some(d) = at.parent() {
                std::fs::create_dir_all(d).map_err(|e| crate::fault::classify(&e, &d.display().to_string()))?;
            }
            crate::local::land(&at, crate::local::Doc::TermsDoc, &bytes)?;
        }
        rel = Some(r);
        picked = Some(name).filter(|n| !n.is_empty());
    }
    let rec = Record { grant: grant.trim().to_ascii_lowercase(), terms, exclusive, doc: rel, name: picked };
    let at = record_path(home, &rec.grant)?;
    if let Some(d) = at.parent() {
        std::fs::create_dir_all(d).map_err(|e| crate::fault::classify(&e, &d.display().to_string()))?;
    }
    crate::local::land(&at, crate::local::Doc::TermsRecord, &record_bytes(&rec))?;
    Ok(rec)
}

pub fn records(home: &Home) -> Result<Vec<Record>, crate::fault::Fault> {
    let dir = home.dir(Slot::Kits).join(ROOM);
    let listing = match std::fs::read_dir(&dir) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(crate::fault::classify(&e, &dir.display().to_string())),
    };
    let mut out: Vec<Record> = Vec::new();
    for p in listing.filter_map(|e| e.ok().map(|e| e.path())) {
        if !(p.is_file() && p.extension().map(|x| x == "json").unwrap_or(false)) {
            continue;
        }
        if let Some(b) = crate::local::read(&p, crate::local::Doc::TermsRecord)? {
            if let Some(r) = record_of(&b) {
                out.push(r);
            }
        }
    }
    out.sort_by(|a, b| a.grant.cmp(&b.grant));
    Ok(out)
}

pub fn record(home: &Home, grant: &str) -> Result<Option<Record>, crate::fault::Fault> {
    Ok(crate::local::read(&record_path(home, grant)?, crate::local::Doc::TermsRecord)?.and_then(|b| record_of(&b)))
}

pub fn doc_bytes(home: &Home, r: &Record) -> Result<Option<(String, Vec<u8>)>, crate::fault::Fault> {
    let Some(rel) = r.doc.clone() else { return Ok(None) };
    Ok(crate::local::read(&doc_path(home, &r.terms)?, crate::local::Doc::TermsDoc)?.map(|b| (rel, b)))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exclusive {
    No,
    Signed,
}

pub fn exclusive_of(records: &[Record], grant: &str) -> Exclusive {
    match records.iter().find(|r| r.grant.eq_ignore_ascii_case(grant)) {
        Some(r) if r.exclusive => Exclusive::Signed,
        _ => Exclusive::No,
    }
}
