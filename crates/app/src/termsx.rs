//! Keeping terms documents and exclusivity records. Written once at signing, read-only afterwards. Both are
//! sealed local data (`local::Doc::TermsDoc`, `local::Doc::TermsRecord`); record bundles and grant files carry
//! the document opened.
//!
//! ─── Two things, one place ───
//!
//! 1. The terms document itself: the file received by the issuing form's cell, with its digest (law §6.3's
//! `terms` is its sha256). It lands in this home at `kits/terms/<digest>/<kit name>`, with the kit name
//! transliterated (`kitx::kit_segment`, the only place). Directories are by digest: one document signed into
//! several grants lands once, and two different documents with the same name do not collide. Record bundles
//! and grant files carry it along the same path into `files/terms/…`, so a verifier shown the document can
//! recompute the digest and read exclusivity themselves (exactly the path law §6.3 points to).
//! 2. The issuance record: one `kits/terms/grant-<id>.json` per grant, recording exclusivity, the terms
//! digest, where the document lies and the document's original file name at the moment of signing. Written only once, at signing: landing
//! refuses when present, the product has no second writer and no action to change it (changing exclusivity
//! means revoking and issuing again). It is local bookkeeping, not in the grant bytes (law §6.3's grant
//! fields have no exclusivity cell); the face marks it "local record only".
//!
//! The older exclusive list in the settings file (`settings.exclusive`) is demoted to read-only historical
//! bookkeeping: read, never written again, with the face marking "no terms document". The double-sale gate
//! reads both (`grantx::table`).

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use std::path::{Path, PathBuf};
use zikaron::json::Value;

/// The name of this room inside the `kits` room, the same room a grant file carries its terms in: named once, in
/// the grant file reading (`zikaron_glue::grantfile`).
pub use zikaron_glue::grantfile::TERMS_ROOM as ROOM;

/// A grant's issuance record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub grant: String,
    pub terms: String,
    pub exclusive: bool,
    /// The document's relative path in the `kits` room (also its path under a record bundle's `files/`);
    /// `None` when no document was received.
    pub doc: Option<String>,
    /// The document's file name as the person picked it (before transliteration). `None` when no document
    /// was received, and for records written before this cell existed.
    pub name: Option<String>,
}

impl Record {
    /// The name inside the kit (the last path segment of `doc`, transliterated).
    pub fn doc_name(&self) -> Option<&str> {
        self.doc.as_deref().and_then(|d| d.rsplit('/').next())
    }

    /// The name the face shows for the attached document: the original file name when recorded, otherwise
    /// the name inside the kit (records signed before the original name was kept). Decided only here.
    pub fn shown_name(&self) -> Option<&str> {
        self.doc.as_ref()?;
        self.name.as_deref().or_else(|| self.doc_name())
    }
}

/// The document's relative path, assembled only here: `terms/<digest without 0x>/<kit name>`. `None` when it
/// cannot be transliterated.
pub fn doc_rel(terms: &str, name: &str) -> Option<String> {
    let seg = crate::kitx::kit_segment(name)?;
    Some(format!("{ROOM}/{}/{seg}", terms.trim().trim_start_matches("0x").to_ascii_lowercase()))
}

/// A terms record's file name is `<prefix><keyed name><suffix>`. One name, one home.
pub const RECORD_PREFIX: &str = "grant-";
pub const RECORD_SUFFIX: &str = ".json";

/// Where a grant's issuance record lives. One name, one home; keyed by the names key (`names`).
pub fn record_path(home: &Home, grant: &str) -> Result<PathBuf, Fault> {
    let name = crate::names::key()?.name(crate::names::Logical::TermsRecord(grant));
    Ok(home.dir(Slot::Kits).join(ROOM).join(format!("{RECORD_PREFIX}{name}{RECORD_SUFFIX}")))
}

/// Where a terms document lies on disk: one file per digest (the same digest is the same bytes), both names
/// keyed by the names key. Records and exports speak the document's logical path (`doc_rel`); only this
/// finds it on disk.
pub fn doc_path(home: &Home, terms: &str) -> Result<PathBuf, Fault> {
    let nk = crate::names::key()?;
    Ok(home
        .dir(Slot::Kits)
        .join(ROOM)
        .join(nk.name(crate::names::Logical::TermsDir(terms)))
        .join(nk.name(crate::names::Logical::TermsDoc(terms))))
}

fn record_bytes(r: &Record) -> Vec<u8> {
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

fn record_of(bytes: &[u8]) -> Option<Record> {
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

/// Written once at signing. A received document is first checked that its digest equals the one signed into
/// the grant (otherwise `TERMS_MISMATCH`, and nothing is written), then the document is written (not again
/// when the same one exists) and the issuance record (refused when present: written once).
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

/// Read all issuance records. No room means empty; a file that opens but is not a record of this form is
/// skipped; one that cannot be opened (locked, sealed under another key, unreadable) is refused by name, never
/// read as "no record" (that would drop the exclusivity it carries).
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

/// A grant's issuance record (`None` when absent: signed before this feature, or not signed in this home); one
/// that cannot be opened is refused by name.
pub fn record(home: &Home, grant: &str) -> Result<Option<Record>, crate::fault::Fault> {
    Ok(crate::local::read(&record_path(home, grant)?, crate::local::Doc::TermsRecord)?.and_then(|b| record_of(&b)))
}

/// The document bytes a record points to (relative path, bytes); `None` without a document or when the file is
/// gone; one that cannot be opened is refused by name (a kit never leaves it out silently).
pub fn doc_bytes(home: &Home, r: &Record) -> Result<Option<(String, Vec<u8>)>, crate::fault::Fault> {
    let Some(rel) = r.doc.clone() else { return Ok(None) };
    Ok(crate::local::read(&doc_path(home, &r.terms)?, crate::local::Doc::TermsDoc)?.map(|b| (rel, b)))
}

/// Where exclusivity comes from. Closed: the record at signing, or the older list in the settings file (no
/// terms document).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exclusive {
    No,
    Signed,
    Legacy,
}

/// Whether a grant is exclusive, read in one place. The issuance record first; only without a record is the
/// old list consulted.
pub fn exclusive_of(records: &[Record], legacy: &[String], grant: &str) -> Exclusive {
    match records.iter().find(|r| r.grant.eq_ignore_ascii_case(grant)) {
        Some(r) if r.exclusive => Exclusive::Signed,
        Some(_) => Exclusive::No,
        None if legacy.iter().any(|x| x.eq_ignore_ascii_case(grant)) => Exclusive::Legacy,
        None => Exclusive::No,
    }
}
