//! Record verifier. Drop in a disclosure kit or raw bytes for kit verification, anchor review and a depth
//! reading; every mismatch is listed individually.
//!
//! Each of the three has its own owner:
//!
//! 1. Kit verification belongs to the kit crate's `kitdir::verify_kit` (kit law §7.4: the manifest must
//!    match the bytes one by one, or the kit is refused). This layer reads no manifest and compares no hash;
//!    it shows the kit crate's verdict and subject unchanged.
//! 2. Anchor review belongs to the anchoring crate (anchor scan, endpoint rule) and the core (thirteen steps
//!    to accept entries, audit to produce the report). It takes the same assembly path as the self-audit
//!    (`auditx::ask_from`), and the lineage is computed from the bytes themselves.
//! 3. The depth reading belongs to the kit crate's `reading::depth`, via the depth page's `depthx::read`:
//!    the same implementation as the author's own proof, so readings match byte for byte.
//!
//! "Which file's hash does not match, which entry's signature fails" is one line each, with its subject; a
//! failed kit verification carries the kit crate's verdict. The mismatch list is built only in
//! [`mismatches`], and every reader uses that list.
//!
//! Raw bytes are entry files in a directory (names recognized by the store crate's `layout`) or a single
//! entry file. Files that cannot be read as entries are listed individually (`Rejected`), never skipped
//! silently: the other-ledger reader skipping them is its honest state ("bytes not seen"); the verifier's
//! honest state is showing the refusals.

use crate::fault::{Fault, Known};
use std::path::{Path, PathBuf};
use zikaron::json::Value;

/// What was dropped in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// A disclosure kit (a directory with `manifest.json`).
    Kit(PathBuf),
    /// Raw bytes: a directory or a file.
    Bytes(PathBuf),
    /// A single-file bundle (grant file, `grantfilex`).
    File(PathBuf),
    /// A publish address (`https://`, `fetchx`): fetched file by file from its manifest.
    Remote(crate::fetchx::Base),
}

impl Source {
    /// Where this came from (path or address), for the UI's "from …" line.
    pub fn said(&self) -> String {
        match self {
            Source::Kit(p) | Source::Bytes(p) | Source::File(p) => p.display().to_string(),
            Source::Remote(b) => b.as_str().to_string(),
        }
    }
}

/// Recognize a path. Anything unrecognized is an error, so a nonexistent path is never verified. An
/// `http(s)://` address is the publish-address form (https only, anything else refused); a file starting
/// with the single-file bundle magic is a grant file.
pub fn source_of(path: &str) -> Result<Source, Fault> {
    // Public functions emit their trace mark, so direct calls that bypass `apply` (tests, CLI) are traced
    // too.
    crate::trace::mark(crate::feature::Feature::D2);
    let p = Path::new(path.trim());
    if path.trim().is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::t(crate::lang::Key::Tail222).to_string()));
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
    } else if md.is_file() {
        Ok(Source::Bytes(p.to_path_buf()))
    } else {
        Err(Fault::known(Known::NotAdoptable, crate::lang::filln(crate::lang::Key::Tail027, &[&(p.display()).to_string()])).at_place(path.trim()))
    }
}

/// A file that could not be read as an entry, with the reason (`why`: the core's token unchanged, kept as
/// evidence for details and the log). The UI text comes from `say`, never from `why`: law refusals use the
/// token-to-text table (`fault::entry_token_say`); a refusal that is a fault uses that fault's own text; one
/// with neither says the file could not be read and that the reason is under details.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejected {
    pub file: String,
    pub why: String,
    pub say: Option<crate::lang::Key>,
}

impl Rejected {
    /// The law refused this file (the core's token).
    pub fn token(file: impl Into<String>, t: zikaron::tokens::Token) -> Rejected {
        Rejected { file: file.into(), why: format!("{t:?}"), say: Some(crate::fault::entry_token_say(t)) }
    }

    /// This file could not be read (or another refusal whose text is already plain).
    pub fn plain(file: impl Into<String>, why: impl Into<String>) -> Rejected {
        Rejected { file: file.into(), why: why.into(), say: None }
    }

    /// Refused by a fault: `why` is its evidence, the text is the fault's own.
    pub fn of_fault(file: impl Into<String>, f: &Fault) -> Rejected {
        Rejected { file: file.into(), why: format!("{} · {}", f.said(), f.tail()), say: f.what_key() }
    }

    /// The system could not read this file or folder: `why` is the error kind, the text the classified fault's.
    pub fn io(file: impl Into<String>, e: &std::io::Error) -> Rejected {
        let file = file.into();
        let say = crate::fault::classify(e, &file).what_key();
        Rejected { file, why: e.kind().to_string(), say }
    }

    /// The UI text: from the table, never the evidence.
    pub fn human(&self) -> String {
        crate::lang::t(self.say.unwrap_or(crate::lang::Key::CheckRejectedUnsaid)).to_string()
    }
}

/// Read raw bytes. A directory is walked (only names matching the store crate's entry names count); a file
/// is that one file. Each goes through the core's thirteen steps: passes go to `entries`, failures to
/// `rejected`. Both empty is an error: nothing to verify.
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
                    // An unreadable directory must be reported: skipping it would mean judging green on a subset.
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
        // A sealed file is this machine's own local data: opened under the local data key (while unlocked)
        // and judged by its plain bytes; one that does not open is reported with the reason, never skipped.
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
        match zikaron::entry::check(&bytes) {
            Ok(_) => good.push(bytes),
            Err(t) => bad.push(Rejected::token(f.display().to_string(), t)),
        }
    }
    if good.is_empty() && bad.is_empty() {
        return Err(Fault::known(Known::NoBytes, p.display().to_string()));
    }
    Ok((good, bad))
}

/// Entries in a disclosure kit, following the kit's own manifest (the `entries` table, one id per row), each
/// at `entries/<id without 0x>.zk1` (kit law §7.3's file naming). Manifest keys, directory names and suffixes
/// come from the glue crate's `names` (shared with the kit writer); no literal is spelled here. Each goes
/// through the core's thirteen steps: passes go to `entries`, failures to `rejected`; rows the manifest lists
/// but the disk lacks are rejected too.
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
            Ok(bytes) => match zikaron::entry::check(&bytes) {
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

/// Entries in a file list that passed kit verification: each item under `entries/` goes through the core's
/// thirteen steps; passes are returned, failures listed.
pub fn entries_of_pairs(pairs: &[(String, Vec<u8>)]) -> (Vec<Vec<u8>>, Vec<Rejected>) {
    let dir = format!("{}/", zikaron_glue::names::ENTRIES_DIR);
    let mut good: Vec<Vec<u8>> = Vec::new();
    let mut bad: Vec<Rejected> = Vec::new();
    for (p, b) in pairs.iter().filter(|(p, _)| p.starts_with(&dir)) {
        match zikaron::entry::check(b) {
            Ok(_) => good.push(b.clone()),
            Err(t) => bad.push(Rejected::token(p.clone(), t)),
        }
    }
    (good, bad)
}

/// Kit verification for a file list (single-file bundles and fetched kits); same verdict as
/// [`verify_kit_at`].
pub fn kit_facts_of(pairs: &[(String, Vec<u8>)]) -> KitFacts {
    facts(zikaron_kit::kitdir::verify_enumeration(pairs))
}

/// Bytes at a path. An empty string means "not given" (`Ok` with no entries); given but unreadable is an
/// error, never silently empty (`unwrap_or_default` would read "unreadable" as "no bytes").
pub fn bytes_or_none(typed: &str) -> Result<Vec<Vec<u8>>, Fault> {
    Ok(bytes_named(typed)?.0)
}

/// As [`bytes_or_none`], also returning the files that could not be read as entries.
pub fn bytes_named(typed: &str) -> Result<(Vec<Vec<u8>>, Vec<Rejected>), Fault> {
    if typed.trim().is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    // Any error reading a named place includes that place as a value.
    source_of(typed).and_then(|s| bytes_at(&s)).map_err(|f| f.at_place(typed.trim()))
}

/// Bytes at a path: a kit is read by its manifest, raw bytes by the store crate's names. The diligence desk
/// and the verifier share this.
pub fn bytes_at(source: &Source) -> Result<(Vec<Vec<u8>>, Vec<Rejected>), Fault> {
    match source {
        Source::Kit(d) => kit_entries(d),
        Source::File(p) => Ok((crate::grantfilex::open(p)?.ledger, Vec::new())),
        Source::Remote(b) => Ok(entries_of_pairs(&crate::fetchx::fetch_kit(b)?.pairs)),
        Source::Bytes(p) => entries_of(p),
    }
}

/// Kit verification result: the kit crate's verdict unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KitFacts {
    pub ok: bool,
    pub entries: usize,
    pub files: usize,
    pub proofs: usize,
    pub kit_id: String,
    /// On failure, the kit crate's verdict (token) and subject; empty on success.
    pub verdict: String,
    pub subject: String,
    /// On success, the entries the manifest lists that fail the law (path, token).
    pub invalid: Vec<(String, String)>,
}

/// Kit verification, delegated to the kit crate; no hash is compared here.
pub fn verify_kit_at(dir: &Path) -> KitFacts {
    // Public functions emit their trace mark, so direct calls that bypass `apply` (tests, CLI) are traced
    // too.
    crate::trace::mark(crate::feature::Feature::D2);
    facts(zikaron_kit::kitdir::verify_kit(dir))
}

fn facts(v: zikaron_kit::kitdir::KitVerdict) -> KitFacts {
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

/// The original file of a record in the kit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Original {
    /// The manifest's `contents` row matches, and the sha256 of the kit's file equals this record's
    /// `content`.
    Match,
    /// The row matches, but the file's bytes hash to another digest.
    Mismatch,
    /// The manifest has no `contents` row for this record, or the kit lacks the file in that row.
    Missing,
}

/// Whether this record is anchored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnChain {
    /// An anchor counted by this audit reaches it; `first_at` is the earliest block time among the anchors
    /// that reach it (kit law §8.2, read the same way as the depth reading's "first anchored":
    /// `zikaron_kit::reading::bounds`).
    Anchored { first_at: u64 },
    /// The chain was read and no counted anchor reaches it.
    NotAnchored,
    /// The chain was not read (no network configured, chain unreachable), or this pass left out a network that
    /// could hold its anchor ([`unread_where_missed`]): never shown as "not anchored".
    Unread,
}

/// One row of record bundle verification: one record (`history`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordRow {
    /// Entry id (record hash).
    pub id: String,
    /// This record's `content` (hex32, lowercase).
    pub content: String,
    /// The record's name (the note of the entry that anchored it), when it has one.
    pub name: Option<String>,
    pub original: Original,
    pub chain: OnChain,
    /// The first anchor reaching it, in full (the same reading as `chain`'s time); `None` when not anchored or
    /// not read.
    pub first: Option<crate::auditx::FirstAnchor>,
}

/// Kit law §7.3's `contents` rows: (content, path), read from the manifest unchanged (empty when the
/// manifest is unreadable).
pub fn contents_of(manifest: &[u8]) -> Vec<(String, String)> {
    // Manifest member names come from the kit writer's name table (`zikaron_glue::names::Field`); no literal
    // is written here.
    use zikaron_glue::names::Field;
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
            Some((content, path))
        })
        .collect()
}

/// The file each `contents` row points to in a kit directory: (content, bytes; `None` when the kit lacks it).
pub fn originals_in_dir(dir: &Path) -> Vec<(String, Option<Vec<u8>>)> {
    let manifest = std::fs::read(dir.join(zikaron_glue::names::MANIFEST)).unwrap_or_default();
    contents_of(&manifest)
        .into_iter()
        .map(|(content, path)| {
            let bytes = std::fs::read(dir.join(&path)).or_else(|_| std::fs::read(dir.join(zikaron_glue::names::FILES_DIR).join(&path))).ok();
            (content, bytes)
        })
        .collect()
}

/// As [`originals_in_dir`], from a file list (a grant file, or a kit fetched from a publish address).
pub fn originals_in_pairs(pairs: &[(String, Vec<u8>)]) -> Vec<(String, Option<Vec<u8>>)> {
    let manifest = pairs.iter().find(|(p, _)| p == zikaron_glue::names::MANIFEST).map(|(_, b)| b.clone()).unwrap_or_default();
    let files_prefixed = |path: &str| format!("{}/{}", zikaron_glue::names::FILES_DIR, path);
    contents_of(&manifest)
        .into_iter()
        .map(|(content, path)| {
            let bytes = pairs.iter().find(|(p, _)| *p == path || *p == files_prefixed(&path)).map(|(_, b)| b.clone());
            (content, bytes)
        })
        .collect()
}

/// Verify a record bundle record by record: one row per record (`history`) in the bundle. Original: the
/// `contents` row for this record's `content`, with the kit's file hashed with sha256 and compared. Anchored
/// and first-anchor block time: on this audit's fragment (`None` when the chain was not read), the core
/// produces the audit result and the kit crate computes each record's bounds (`reading::bounds`, as for
/// depth), once for all records. Kit format, kit law and the audit report format are unchanged.
pub fn records(bytes: &[Vec<u8>], originals: &[(String, Option<Vec<u8>>)], fragment: Option<&Value>) -> Vec<RecordRow> {
    crate::trace::mark(crate::feature::Feature::D2);
    let times = fragment.and_then(|f| crate::auditx::first_anchored(bytes, f));
    let firsts = fragment.and_then(|f| crate::auditx::first_anchors(bytes, f)).unwrap_or_default();
    bytes
        .iter()
        .filter_map(|b| zikaron::entry::check(b).ok())
        .filter_map(|e| crate::ledgerx::work_of(e.kind, &e.body).map(|c| (e.id_hex(), c.to_ascii_lowercase(), crate::ledgerx::facts_of(&e).note)))
        .map(|(id, content, name)| {
            let mine: Vec<&Option<Vec<u8>>> = originals.iter().filter(|(c, _)| *c == content).map(|(_, b)| b).collect();
            let original = if mine.iter().any(|b| b.as_ref().map(|x| zikaron::hexfmt::encode(&crate::anchorx::file_digest(x)).eq_ignore_ascii_case(&content)).unwrap_or(false)) {
                Original::Match
            } else if mine.iter().any(|b| b.is_some()) {
                Original::Mismatch
            } else {
                Original::Missing
            };
            let chain = match &times {
                None => OnChain::Unread,
                Some(at) => match at.iter().find(|(h, _)| *h == id).map(|(_, t)| *t) {
                    Some(t) => OnChain::Anchored { first_at: t },
                    None => OnChain::NotAnchored,
                },
            };
            let first = match chain {
                OnChain::Anchored { .. } => firsts.iter().find(|(h, _)| *h == id).map(|(_, f)| f.clone()),
                _ => None,
            };
            RecordRow { id, content, name, original, chain, first }
        })
        .collect()
}

/// Anchor review result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnchorReview {
    pub label: String,
    pub anchors: usize,
    pub asked: usize,
    /// The ids the report lists as UNANCHORED, when this pass read every network it was to read.
    pub unanchored: Vec<String>,
    pub anchored: usize,
    /// The ids the report lists as UNANCHORED when this pass left a network out ([`unread_where_missed`]): their
    /// anchor may be on the chain not read, so they are not called unanchored.
    pub unread: Vec<String>,
}

/// A pass that left a network out (unreachable, fingerprint mismatch) cannot say a record it did not reach is
/// not anchored: its anchor may be on the network not read. So with any network missed, every record row read
/// "not anchored" reads "chain not read", and the review's unanchored ids move to `unread`. A pass that read
/// every network it was to read (always so with no read-only network) is left exactly as it is.
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

/// One verification's result. Every field comes from the background pass.
#[derive(Clone, Debug)]
pub struct Verified {
    pub path: String,
    pub source: Source,
    /// When a kit was dropped in, its kit verification reading.
    pub kit: Option<KitFacts>,
    pub entries: usize,
    pub rejected: Vec<Rejected>,
    /// Anchor review: the result on success, the error otherwise (chain unreachable, basis not configured).
    pub review: Result<AnchorReview, String>,
    pub work: String,
    /// The kit crate's depth reading, unchanged; `None` when no record was given.
    pub depth: Option<Value>,
    /// The mismatch list.
    pub mismatches: Vec<String>,
    /// Per record: one row per record when a record bundle was dropped in (directory, grant file, publish
    /// address); empty otherwise.
    pub records: Vec<RecordRow>,
    /// The network the kit says it is anchored on, when that network is neither the main network nor a
    /// read-only one: then no chain was read, and the page says which network to add.
    pub not_added: Option<crate::kitsindex::AnchoredOn>,
    /// Networks this pass could not read (unreachable, fingerprint mismatch), each named.
    pub missed: Vec<crate::widex::Missed>,
    /// The chains this pass read (its basis), by ascending chain id; empty when no chain was read.
    pub read: Vec<u64>,
    /// The result file: where it was written, or why not (`None`: not written because the network is not
    /// added or there is no kit).
    pub filed: Option<Result<String, String>>,
}

/// The chains a fragment's basis names, by ascending chain id.
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

/// The manifest bytes of a kit given as a file list.
pub fn manifest_in(pairs: &[(String, Vec<u8>)]) -> Option<Vec<u8>> {
    pairs.iter().find(|(p, _)| p == zikaron_glue::names::MANIFEST).map(|(_, b)| b.clone())
}

/// Where a kit's manifest says it is anchored: the fixed last non-empty line of its `note_md` (`kitsindex::split_note`).
/// The author's statement, a pointer only: no verdict reads it.
pub fn stated_on(manifest: &[u8]) -> Option<crate::kitsindex::AnchoredOn> {
    let v = zikaron::json::parse(manifest).ok()?;
    let note = match v.member(zikaron_glue::names::Field::NoteMd.as_str()) {
        Some(Value::Str(s)) => s.clone(),
        _ => return None,
    };
    crate::kitsindex::split_note(&note).1
}

/// Depth reading, via the depth page (the same implementation).
pub fn depth_of(bytes: &[Vec<u8>], fragment: &Value, work: &str) -> Result<Value, Fault> {
    // Public functions emit their trace mark, so direct calls that bypass `apply` (tests, CLI) are traced
    // too.
    crate::trace::mark(crate::feature::Feature::D2);
    Ok(crate::depthx::read(bytes, fragment, work)?.value)
}

/// Every mismatch: a failed kit verification (verdict and subject), manifest entries that fail the law, files
/// whose signature fails, incomplete labels, unanchored entries, entries not reached on a pass that left a
/// network out, records not found. One line each, with its subject; an empty list means no mismatches.
pub fn mismatches(
    kit: Option<&KitFacts>,
    rejected: &[Rejected],
    review: &Result<AnchorReview, String>,
    depth: Option<&Value>,
) -> Vec<String> {
    // Public functions emit their trace mark, so direct calls that bypass `apply` (tests, CLI) are traced
    // too.
    crate::trace::mark(crate::feature::Feature::D2);
    let mut out: Vec<String> = Vec::new();
    if let Some(k) = kit {
        if !k.ok {
            out.push(format!("kit {} {}", k.verdict, k.subject));
        }
        for (p, t) in &k.invalid {
            out.push(format!("invalid {p} {t}"));
        }
    }
    for r in rejected {
        out.push(format!("rejected {} {}", r.file, r.why));
    }
    match review {
        Ok(r) => {
            // A record bundle carries only the chosen entries plus the ledger's spine, so gaps in sequence
            // numbers (GAPS) are normal for a bundle and are not a mismatch; other labels (broken chain,
            // empty) are still listed. The whole-ledger bytes path still requires COMPLETE.
            let partial_kit = kit.is_some() && r.label == zikaron::tokens::Label::Gaps.as_str();
            if r.label != zikaron::tokens::Label::Complete.as_str() && !partial_kit {
                out.push(format!("label {}", r.label));
            }
            for id in &r.unanchored {
                out.push(format!("unanchored {id}"));
            }
            for id in &r.unread {
                out.push(format!("unread {id}"));
            }
        }
        Err(said) => out.push(format!("review {said}")),
    }
    if let Some(d) = depth {
        let t = crate::depthx::three(d.clone());
        if !t.valid {
            out.push("depth invalid".to_string());
        } else if !t.found {
            out.push("work absent".to_string());
        }
    }
    out
}
