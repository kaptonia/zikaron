//! Ledger directory access without any direct disk write. Open, append, read the pile and read by name all go
//! through the storage crate's `LedgerDir`; entry names through its `EntryName::parse` (64 lowercase hex, no
//! other shape can be built).
//!
//! The two lineage questions (who is the root, which entry to follow) are not judged here either: an offline
//! audit input (empty anchors, evidence and basis) goes to the core's `audit_full`, and its lineage and
//! ledger come back. The shell only picks the highest seq in what the law returns; when that is not unique (a
//! fork) it says so and does not guess.

use crate::codes::{Field, Reason};
use zikaron::audit;
use zikaron::entry::Entry;
use zikaron::hexfmt;
use zikaron::json::Value;
use zikaron_store::codes::Trouble;
use zikaron_store::layout::EntryName;
use zikaron_store::ledger::{LedgerDir, Stored};
use zikaron_anchor::input;
use zikaron_anchor::scan::{self, Scanned};

/// Open an existing ledger directory.
pub fn open(root: &str) -> Result<LedgerDir, Trouble> {
    crate::seam();
    LedgerDir::open(root)
}

/// Open a ledger directory, creating it when missing (`init` takes this path).
pub fn open_or_create(root: &str) -> Result<LedgerDir, Trouble> {
    crate::seam();
    LedgerDir::open_or_create(root)
}

/// Append an entry. The file name is the 64-hex entry id (without `0x`), through the storage crate's only
/// constructor.
pub fn append(dir: &LedgerDir, entry: &Entry, bytes: &[u8]) -> Result<Stored, Trouble> {
    crate::seam();
    let hex = entry.id_hex();
    let name = match EntryName::parse(hex.trim_start_matches("0x")) {
        Some(n) => n,
        // An id is a 32-byte sha256 whose hex is always 64 lowercase digits, so this branch is unreachable;
        // if it were reached, the storage crate's naming rule would decide, reported with its own code.
        None => return Err(Trouble::named(zikaron_store::codes::Code::BadName, hex)),
    };
    dir.append(&name, bytes)
}

/// The strict read's bytes are the audit pile.
pub fn pile(dir: &LedgerDir) -> Result<Vec<Vec<u8>>, Trouble> {
    crate::seam();
    let items = dir.pile()?.items;
    refuse_sealed(&dir.root().to_string_lossy(), &items);
    Ok(items)
}

/// The sentence said when refusing a sealed ledger, in the language of the lines for people
/// ([`crate::out::lang`]). One name, one home.
pub fn sealed_said() -> &'static str {
    crate::out::Said::Sealed.text()
}

/// A ledger ZIKARON Desk keeps is sealed local data, and the command line holds no passcode: it reads such a
/// ledger as the app does while locked, refused by the existing unreadable reason, naming "locked".
/// `subject` is the ledger read (its path), named on the first stderr line.
pub fn refuse_sealed(subject: &str, items: &[Vec<u8>]) {
    if items.iter().any(|b| zikaron_glue::sealed::is_sealed(b)) {
        crate::out::misuse(crate::codes::Reason::Unreadable, crate::out::typed(subject), crate::out::Said::Sealed);
    }
}

/// Transport spelling of the pile: `0x` plus an even number of lowercase hex, one per item (HARNESS `audit`
/// input).
pub fn pile_hex(items: &[Vec<u8>]) -> Vec<String> {
    items.iter().map(|b| hexfmt::encode(b)).collect()
}

/// An offline audit input: the pile is real, the three chain fields are empty.
///
/// The fragment comes from the anchoring crate's `scan::fragment` (empty anchors and evidence) and the input
/// from its `input::assemble`. The shell only supplies a basis with all three tables empty.
pub fn offline_input(root: &str, items: &[Vec<u8>]) -> Option<Value> {
    crate::seam();
    let empty = Scanned {
        anchors: Vec::new(),
        evidence: Vec::new(),
        basis: empty_basis(),
    };
    input::assemble(&scan::fragment(&empty), root, &pile_hex(items), &[])
}

/// The three members of a law §9.4 basis, no more, no fewer, all tables empty.
pub fn empty_basis() -> Value {
    Value::Obj(vec![
        (Field::AdoptionChains.as_str().to_string(), Value::Arr(Vec::new())),
        (Field::BareTx.as_str().to_string(), Value::Arr(Vec::new())),
        (Field::Chains.as_str().to_string(), Value::Arr(Vec::new())),
    ])
}

/// The root: the author of the genesis entry in the pile. The core decides which entry is genesis (law §4.2
/// makes seq 0 and type genesis equivalent); the shell only counts.
pub fn root_of(items: &[Vec<u8>]) -> Result<String, Reason> {
    crate::seam();
    let mut authors: Vec<String> = Vec::new();
    for b in items {
        if let Ok(e) = zikaron::entry::check(b) {
            if e.seq == 0 {
                authors.push(e.author);
            }
        }
    }
    authors.sort();
    authors.dedup();
    match authors.len() {
        1 => Ok(authors.remove(0)),
        0 => Err(Reason::TipAbsent),
        _ => Err(Reason::TipForked),
    }
}

/// Lineage entries: an offline audit input goes to the core's `audit_full` and its ledger comes back. [`tip`]
/// and the retraction writing rule ask this same question.
pub fn entries(root: &str, items: &[Vec<u8>]) -> Result<Vec<Entry>, Reason> {
    crate::seam();
    let input = offline_input(root, items).ok_or(Reason::TipAbsent)?;
    let outcome = audit::audit_full(&input).ok_or(Reason::TipAbsent)?;
    Ok(outcome.ledger)
}

/// The lineage tip: the `seq` and `prev` of the next entry.
///
/// Ledger and lineage come from the core's `audit_full`; the shell takes the highest seq. More than one at
/// that level means the lineage forked; this layer cannot choose and reports `E_TIP_FORKED`.
pub fn tip(root: &str, items: &[Vec<u8>]) -> Result<(u64, String), Reason> {
    crate::seam();
    let ledger = entries(root, items)?;
    let mut top: Option<u64> = None;
    for e in &ledger {
        top = Some(match top {
            Some(x) if x >= e.seq => x,
            _ => e.seq,
        });
    }
    let top = match top {
        Some(x) => x,
        None => return Err(Reason::TipAbsent),
    };
    let mut ids: Vec<String> = ledger
        .iter()
        .filter(|e| e.seq == top)
        .map(|e| e.id_hex())
        .collect();
    ids.sort();
    ids.dedup();
    match ids.len() {
        1 => Ok((top + 1, ids.remove(0))),
        _ => Err(Reason::TipForked),
    }
}

/// What writing `candidate` would newly break, as the core judges it: one offline audit of this pile and one
/// of this pile with the candidate, under the same root, and the chain findings only the second has (law
/// §8.3's five, hard or not, each as its name). Empty means the candidate adds no finding the pile did not
/// already have.
///
/// Every chain rule is the core's (authority after a succession, sequence gaps, broken links, equivocation,
/// the root): this layer asks once and compares, so one gate covers the whole class, `--seq`/`--prev` given by
/// hand included. Not only the hard ones: a sequence gap is a soft finding, and once the walk is uncertain
/// after it a key that no longer holds the seat is only softly out of place, so a gate on hard findings alone
/// lets the old key write after any gap. A write by the key that holds the seat, at the tip, adds none. The
/// root is the one the writer names (`--root`, the same one its tip was asked under) when given; otherwise the
/// pile's own, and a pile that would hold two roots, whether the candidate gives it the second or it had two
/// already, cannot be audited under one and is `E_TIP_FORKED` (the gate refuses what it cannot judge); a
/// ledger with no root at all has nothing to audit and is left to the core's own check of the entry.
pub fn new_findings(items: &[Vec<u8>], candidate: &[u8], named: Option<&str>) -> Result<Vec<String>, Reason> {
    crate::seam();
    let mut all = items.to_vec();
    all.push(candidate.to_vec());
    let root = match (named, root_of(&all)) {
        (Some(r), _) => r.to_string(),
        (None, Ok(r)) => r,
        (None, Err(Reason::TipForked)) => return Err(Reason::TipForked),
        (None, Err(_)) => return Ok(Vec::new()),
    };
    let found = |pile: &[Vec<u8>]| -> Vec<(String, String)> {
        offline_input(&root, pile)
            .and_then(|i| audit::audit_full(&i))
            .map(|o| o.findings.iter().map(|f| (f.name.as_str().to_string(), f.entry_id.clone())).collect())
            .unwrap_or_default()
    };
    let before = found(items);
    let mut names: Vec<String> = found(&all).into_iter().filter(|f| !before.contains(f)).map(|(n, _)| n).collect();
    names.sort();
    names.dedup();
    Ok(names)
}

/// Why a read-only verb could not read what `--ledger` names.
pub enum ReadRefused {
    /// A ledger directory the storage crate refused (its own code).
    Ledger(Trouble),
    /// A record package the kit core judged invalid (its verdict and subject).
    Kit(zikaron_kit::tokens::KitFailToken, Option<String>),
    /// A mirror bundle whose manifest is not this kind or shape (what it says, as `kind/version`), or does
    /// not read.
    Mirror(String),
    /// A file of the bundle or package that does not read (its path).
    Unreadable(String),
}

/// What a read-only verb reads at `path` (`audit`, and through it `check-grant`, `chain-check`, `depth`;
/// `show --entry`): a mirror bundle by its manifest, read by the names the app writes it with
/// (`zikaron_glue::mirror`: each listed entry from the entries room); a record package (a disclosure kit)
/// once the kit core verifies it (`kitdir::verify_kit`), the entries in its entries room; anything else as a
/// ledger directory, strictly. Whether each entry is an entry is the core's call, in the audit. Writing verbs
/// and `init` never come here: they read ledger directories only.
pub fn read_any(path: &str) -> Result<Vec<Vec<u8>>, ReadRefused> {
    crate::seam();
    let dir = std::path::Path::new(path);
    if zikaron_glue::mirror::is_bundle(dir) {
        return zikaron_glue::mirror::entries(dir).map_err(|t| match t {
            zikaron_glue::mirror::ReadTrouble::Unreadable(p) => ReadRefused::Unreadable(p),
            zikaron_glue::mirror::ReadTrouble::NotJson(x) | zikaron_glue::mirror::ReadTrouble::NotThisKind(x) | zikaron_glue::mirror::ReadTrouble::NotAnEntryName(x) => ReadRefused::Mirror(x),
        });
    }
    if zikaron_glue::read::is_kit(dir) {
        if let zikaron_kit::kitdir::KitVerdict::Fail { verdict, subject } = zikaron_kit::kitdir::verify_kit(dir) {
            return Err(ReadRefused::Kit(verdict, subject));
        }
        return zikaron_glue::read::kit_entries(dir).map_err(ReadRefused::Unreadable);
    }
    let d = open(path).map_err(ReadRefused::Ledger)?;
    pile(&d).map_err(ReadRefused::Ledger)
}

