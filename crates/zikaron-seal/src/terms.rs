//! A grant's issuance record and its terms document, kept in a home's kits room (sealed as `Doc::TermsRecord`,
//! `Doc::TermsDoc`): the record's form and where both lie are named here, so the desktop that writes them and
//! the command line that reads them from a home (a record package exported with the credential carries the
//! terms of its grants, as the desktop's does) read them alike.

use crate::envelope::KITS_DIR;
use crate::names::{Logical, NameKey};
use zikaron::json::Value;

/// A terms record's file name is `<prefix><keyed name><suffix>`.
pub const RECORD_PREFIX: &str = "grant-";
pub const RECORD_SUFFIX: &str = ".json";

/// A grant's issuance record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub grant: String,
    pub terms: String,
    pub exclusive: bool,
    /// The document's relative path in `kits` (also its path under a record bundle's `files/`); `None` when
    /// no document was given.
    pub doc: Option<String>,
    /// The document's file name as the person picked it (before transliteration). `None` when no document
    /// was given, and for records written by older versions without this field.
    pub name: Option<String>,
}

impl Record {
    /// The name inside the kit (the last path segment of `doc`, transliterated).
    pub fn doc_name(&self) -> Option<&str> {
        self.doc.as_deref().and_then(|d| d.rsplit('/').next())
    }

    /// The name shown for the attached document: the original file name when recorded, otherwise the name
    /// inside the kit (older records without the original name). Decided only here.
    pub fn shown_name(&self) -> Option<&str> {
        self.doc.as_ref()?;
        self.name.as_deref().or_else(|| self.doc_name())
    }
}

/// Where a grant's issuance record lies under its home, its name keyed by the names key.
pub fn record_rel(nk: &NameKey, grant: &str) -> String {
    format!("{KITS_DIR}/{}/{RECORD_PREFIX}{}{RECORD_SUFFIX}", zikaron_glue::grantfile::TERMS_ROOM, nk.name(Logical::TermsRecord(grant)))
}

/// Where a terms document lies under its home: one file per digest, both names keyed by the names key.
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

