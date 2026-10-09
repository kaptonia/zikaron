//! Anchor reconciliation. No audit conclusion is reached here.
//!
//! The basis and fragment shapes belong to the anchoring crate (`scan::fragment`), assembling the audit input
//! to its `input::assemble`, and the fifteen-item report with its labels to the core's `audit`. This layer
//! only builds the pile from the ledger, finds the root, and passes the core's label through unchanged.
//!
//! Only an audit returning COMPLETE releases the pen: [`Verdict::complete`] is true only when the core's
//! label equals [`complete`]. Every other label is passed through unchanged and the pen stays held.

use crate::chainx::Endpoint;
use crate::fault::{Fault, Known};
use crate::home::Home;
use crate::key::Address;
use zikaron::json::Value;
use zikaron::tokens::Key;
use zikaron_anchor::input;
use zikaron_anchor::rpc;
use zikaron_anchor::scan::{self, Scanned};

/// The label that releases the pen, spelled by the core's closed type (labels are defined only in the core).
///
/// A local string constant would not break if the core changed a letter, and the shell would silently never
/// release the pen again.
pub fn complete() -> &'static str {
    zikaron::tokens::Label::Complete.as_str()
}

/// Whether this pass's chain reading is whole (decided only here): endpoints were asked (the offline path asks
/// none), every endpoint answered, and the core's label is readable and says the data is all here
/// (`COMPLETE` or `GAPS`). `GAPS` means some ledger entry is unanchored or missing, which concerns the ledger;
/// the reading itself is whole. `UNAVAILABLE`, `BROKEN_CHAIN` and an unreadable label are not whole.
/// Decisions based on "this entry is not in the last anchor set" (re-queueing the root) accept only a whole
/// reading: in an incomplete one, absent does not mean unanchored.
pub fn whole(label: &str, asked: usize, unanswered: &[String]) -> bool {
    use zikaron::tokens::Label;
    (label == Label::Complete.as_str() || label == Label::Gaps.as_str()) && asked > 0 && unanswered.is_empty()
}

/// One reconciliation's reading.
pub struct Verdict {
    /// The core's label, unchanged. An empty string when the label is unreadable (then `complete` is always
    /// false).
    pub label: String,
    /// The only condition for releasing the pen: the label is COMPLETE.
    pub complete: bool,
    /// The report itself, for the UI to expand.
    pub report: Value,
    /// How many ledger entries this pass reconciled.
    pub entries: usize,
    /// Which endpoints did not answer this pass. An empty list does not mean all answered: the offline path
    /// asks no endpoint at all.
    pub unanswered: Vec<String>,
    /// How many endpoints were asked (zero offline).
    pub asked: usize,
    /// Only one endpoint answered a load-bearing read; flagged explicitly.
    pub single_source: bool,
    /// The fragment this pass used, unchanged. Depth must be computed on the same basis, so the fragment
    /// travels with the reading; it cannot be derived from the report (which has no anchor or evidence set).
    pub fragment: Value,
    /// Whose ledger this pass reconciled: the author of the genesis entry in those bytes. The reading travels
    /// bound to its owner, so no consumer can show one ledger under another person's name (see
    /// `readerx::read`).
    pub root: String,
}

impl Verdict {
    /// The label's closed type. Read from the core's closed table, not by comparing strings in this layer.
    pub fn tag(&self) -> zikaron::tokens::Label {
        zikaron::tokens::Label::ALL
            .into_iter()
            .find(|l| l.as_str() == self.label)
            .unwrap_or(zikaron::tokens::Label::NoLabel)
    }

    /// Whether the chain is broken. The write lock depends on this field.
    pub fn broken(&self) -> bool {
        self.tag() == zikaron::tokens::Label::BrokenChain
    }
}

/// The report's sixteen members (fifteen items; the first has two keys). Keys come from the core's closed type:
/// if the core renames or removes one, this table stops compiling; if the core adds one, the self-check's
/// count catches it.
pub const ITEMS: [Key; 16] = [
    Key::Root,
    Key::Basis,
    Key::Entries,
    Key::Findings,
    Key::Missing,
    Key::Anchored,
    Key::Unanchored,
    Key::Excluded,
    Key::AdoptionUnproven,
    Key::UnknownType,
    Key::Malformed,
    Key::Unavailable,
    Key::Unproven,
    Key::Void,
    Key::Discarded,
    Key::LabelKey,
];

/// One report item laid out for display. `count` exists only for table items.
pub struct Item {
    pub key: &'static str,
    pub count: Option<usize>,
    /// A one-line reading for the non-table items (root, entries, label).
    pub said: String,
}

/// Lays the report out as sixteen items. A missing item gets no count rather than zero, so "the core did not
/// give this item" and "this item is an empty table" stay distinguishable.
pub fn items(report: &Value) -> Vec<Item> {
    ITEMS
        .into_iter()
        .map(|k| {
            let name = k.as_str();
            match member(report, name) {
                Some(Value::Arr(a)) => Item { key: name, count: Some(a.len()), said: String::new() },
                Some(Value::Str(s)) => Item { key: name, count: None, said: s.clone() },
                Some(Value::Int(n)) => Item { key: name, count: None, said: n.to_string() },
                Some(Value::Obj(m)) => Item { key: name, count: Some(m.len()), said: String::new() },
                Some(other) => Item { key: name, count: None, said: format!("{other:?}") },
                None => Item { key: name, count: None, said: String::new() },
            }
        })
        .collect()
}

/// One item's table from the report (used when listing MISSING / UNPROVEN / VOID / EXCLUDED row by row).
pub fn rows_of(report: &Value, k: Key) -> Vec<Value> {
    match member(report, k.as_str()) {
        Some(Value::Arr(a)) => a.clone(),
        _ => Vec::new(),
    }
}

fn member<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).map(|(_, x)| x),
        _ => None,
    }
}

/// An empty basis: exactly its three members, all three tables empty.
fn empty_basis() -> Value {
    Value::Obj(vec![
        ("adoptionChains".to_string(), Value::Arr(Vec::new())),
        ("bareTx".to_string(), Value::Arr(Vec::new())),
        ("chains".to_string(), Value::Arr(Vec::new())),
    ])
}

fn pile_hex(items: &[Vec<u8>]) -> Vec<String> {
    items.iter().map(|b| zikaron::hexfmt::encode(b)).collect()
}

/// Root: the author of the genesis entry in the pile. Deciding which entry is the genesis is the core's job;
/// the shell only counts how many there are.
pub fn root_of(items: &[Vec<u8>]) -> Result<String, Fault> {
    let mut found: Vec<String> = Vec::new();
    for b in items {
        if let Ok(e) = zikaron::entry::check(b) {
            if e.seq == 0 {
                found.push(e.author.clone());
            }
        }
    }
    found.sort();
    found.dedup();
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(Fault::known(Known::NoGenesis, "这本账里没有创世条目".to_string())),
        n => Err(Fault::known(Known::ForkedRoot, crate::lang::filln(crate::lang::Key::Tail079, &[&(n).to_string()]))),
    }
}

/// The ledger in a pile of bytes that contains this entry. A pile may mix several ledgers (a grant file
/// carries the whole chain and the issuer's ledger), and the audit input takes only one (`root_of` requires
/// exactly one genesis). Follow `prev` to the head; entries with the same head form one ledger. Returns the
/// ledger that contains `want`; if none does, the one whose lineage has `author` (the kit crate then reports
/// "not in the ledger"); with neither, `None`.
pub fn ledger_of(items: &[Vec<u8>], want: &str, author: &str) -> Option<Vec<Vec<u8>>> {
    let entries: Vec<zikaron::entry::Entry> = items.iter().filter_map(|b| zikaron::entry::check(b).ok()).collect();
    let by_id: std::collections::HashMap<String, &zikaron::entry::Entry> = entries.iter().map(|e| (e.id_hex(), e)).collect();
    let head_of = |e: &zikaron::entry::Entry| -> String {
        let mut at = e;
        for _ in 0..=entries.len() {
            match at.prev.as_ref().and_then(|p| by_id.get(p)) {
                Some(up) => at = up,
                None => break,
            }
        }
        at.id_hex()
    };
    let mut books: Vec<(String, Vec<Vec<u8>>)> = Vec::new();
    for e in &entries {
        let h = head_of(e);
        match books.iter_mut().find(|(k, _)| *k == h) {
            Some((_, v)) => v.push(e.bytes.clone()),
            None => books.push((h, vec![e.bytes.clone()])),
        }
    }
    let holds = |v: &Vec<Vec<u8>>| v.iter().any(|b| zikaron::hexfmt::encode(&zikaron::entry::entry_id(b)).eq_ignore_ascii_case(want));
    if let Some((_, v)) = books.iter().find(|(_, v)| holds(v)) {
        return Some(v.clone());
    }
    books.into_iter().find(|(_, v)| senders_of(v).iter().any(|a| a.eq_ignore_ascii_case(author))).map(|(_, v)| v)
}

/// Offline reconciliation: all three basis tables empty. Used when the chain is unreachable; the label comes
/// from the core, never guessed here, since "chain not read" and "not on chain" differ and the core's labels
/// tell them apart.
pub fn offline(home: &Home) -> Result<Verdict, Fault> {
    let ledger = home.ledger()?;
    let pile = ledger
        .pile()?;
    ask(&pile.items, &Scanned { anchors: Vec::new(), evidence: Vec::new(), basis: empty_basis() })
}

/// Offline reconciliation of a set of entries (nothing written; used to reconcile a fetched ledger before it
/// lands).
pub fn offline_items(items: &[Vec<u8>]) -> Result<Verdict, Fault> {
    ask(items, &Scanned { anchors: Vec::new(), evidence: Vec::new(), basis: empty_basis() })
}

/// Builds the audit input from a scan result and the ledger, and has the core produce the report.
pub fn ask(items: &[Vec<u8>], scanned: &Scanned) -> Result<Verdict, Fault> {
    ask_from(items, &scan::fragment(scanned), Vec::new(), 0, false)
}

/// Like [`ask`], with the fragment given by the caller (the online path has already reconciled the endpoints).
pub fn ask_from(
    items: &[Vec<u8>],
    fragment: &Value,
    unanswered: Vec<String>,
    asked: usize,
    single_source: bool,
) -> Result<Verdict, Fault> {
    let root = root_of(items)?;
    let assembled = input_of(items, fragment)?;
    let report = zikaron::audit::audit(&assembled);
    let label = match member(&report, Key::LabelKey.as_str()) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    };
    Ok(Verdict {
        complete: label == complete(),
        label,
        report,
        entries: items.len(),
        root,
        unanswered,
        asked,
        single_source,
        fragment: fragment.clone(),
    })
}

/// An audit input, for the places that pass it to the kit crate.
///
/// The depth reading and the six checks both take an audit input or outcome, and only
/// [`zikaron_anchor::input::assemble`] assembles one; this layer never builds its own.
///
/// The input is validated as the core reads it: one whose canonical bytes the core's reader refuses (for
/// example a number a node returned past the canonical integer ceiling) is not an audit input, so it is refused
/// by name and nothing (no label, no first-anchor time) is derived from it that the bytes would not give.
pub fn input_of(items: &[Vec<u8>], fragment: &Value) -> Result<Value, Fault> {
    let root = root_of(items)?;
    let input = input::assemble(fragment, &root, &pile_hex(items), &[])
        .ok_or_else(|| Fault::known(Known::AuditInput, crate::lang::t(crate::lang::Key::Tail080).to_string()))?;
    if let Err(t) = zikaron::json::parse(&zikaron::json::canon_bytes(&input)) {
        return Err(Fault::known(Known::AuditInput, format!("{} · {t:?}", crate::lang::t(crate::lang::Key::Tail081))));
    }
    Ok(input)
}

/// The same input handed to the core for the audit outcome (the report plus ledger, findings, anchor set and
/// lineage).
///
/// The kit crate's depth reading and chain check need exactly this. Nothing is recomputed here; the input is
/// only passed along.
pub fn outcome_of(items: &[Vec<u8>], fragment: &Value) -> Result<zikaron::audit::Outcome, Fault> {
    let input = input_of(items, fragment)?;
    zikaron::audit::audit_full(&input)
        .ok_or_else(|| Fault::known(Known::AuditInput, crate::lang::t(crate::lang::Key::Tail081).to_string()))
}

/// **Each entry's first-anchor block time**: the earliest block timestamp among the counted anchors that reach
/// it (the kit format's rule, the same as the depth reading's "first anchored"). `None` when the fragment does
/// not audit; entries no anchor reaches are left out.
pub fn first_anchored(items: &[Vec<u8>], fragment: &Value) -> Option<Vec<(String, u64)>> {
    let outcome = outcome_of(items, fragment).ok()?;
    let lines = zikaron_kit::reading::Lines::of(&outcome.ledger);
    let bounds = zikaron_kit::reading::bounds(&outcome, &lines);
    Some(bounds.iter().enumerate().filter_map(|(i, t)| t.map(|t| (lines.id(i).to_string(), t))).collect())
}

/// The earliest counted anchor that reaches a ledger entry, in full: the same rule as [`first_anchored`]'s
/// time, plus which anchor gave it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FirstAnchor {
    pub chain_id: u64,
    pub block_number: u64,
    pub block_timestamp: u64,
    /// The anchoring transaction (hex32).
    pub tx: String,
    /// The hash it anchored (hex32): the entry it points to, at or after the one it reaches.
    pub hash: String,
    /// The registry contract whose log carried it (hex20), when the scan noted one.
    pub registry: Option<String>,
}

/// **Each entry's first anchor, in full**: per entry, among the counted anchors whose time is that entry's
/// bound and that reach it, the one with the smallest (chain id, block, transaction). `None` when the fragment
/// does not audit; entries no anchor reaches are left out.
pub fn first_anchors(items: &[Vec<u8>], fragment: &Value) -> Option<Vec<(String, FirstAnchor)>> {
    let outcome = outcome_of(items, fragment).ok()?;
    let lines = zikaron_kit::reading::Lines::of(&outcome.ledger);
    let bounds = zikaron_kit::reading::bounds(&outcome, &lines);
    let int = |v: &Value, k: &str| match v.member(k) {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    };
    let text = |v: &Value, k: &str| match v.member(k) {
        Some(Value::Str(s)) => Some(s.clone()),
        _ => None,
    };
    // The fragment's counted rows from a sender of this lineage (the set `counted` was trimmed from).
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
    Some(out)
}

/// Names the registry of each first anchor from a scan's side reading (if several, the lowest address).
pub fn name_registry(first: &mut FirstAnchor, emitters: &zikaron_anchor::scan::Emitters) {
    let h32 = |s: &str| -> Option<[u8; 32]> { zikaron::hexfmt::decode(s)?.try_into().ok() };
    let (Some(tx), Some(hash)) = (h32(&first.tx), h32(&first.hash)) else { return };
    if let Some(set) = emitters.get(&(first.chain_id, first.block_number, tx, hash)) {
        first.registry = set.iter().next().map(|a| zikaron::hexfmt::encode(a));
    }
}

/// A fragment with an empty basis (the offline path needs it).
pub fn empty_fragment() -> Value {
    scan::fragment(&Scanned { anchors: Vec::new(), evidence: Vec::new(), basis: empty_basis() })
}

// ───────────────────────── Basis and the online pass ─────────────────────────

/// The fields of a basis declaration. Filled from the settings page (chain id, endpoints, scan window); the
/// senders are computed from this ledger itself (see [`senders_of`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ground {
    pub chain: u64,
    pub registry: Address,
    pub from_block: u64,
    pub to_block: u64,
    /// Sender addresses scanned (hex20, sorted and deduplicated).
    pub senders: Vec<String>,
}

/// The senders this ledger declares: the author of every entry in the pile, plus every succession's `to`.
///
/// This is a superset: the audit discards anchor records outside the whole set's lineage and lists them as
/// DISCARDED, so declaring wider only scans more, never counts more. Sorting and deduplicating is required
/// (one scan, one assembly).
pub fn senders_of(items: &[Vec<u8>]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for b in items {
        let Ok(e) = zikaron::entry::check(b) else { continue };
        out.push(e.author.clone());
        if e.kind == zikaron::tokens::EntryType::Succession {
            if let Value::Obj(m) = &e.body {
                if let Some((_, Value::Str(to))) = m.iter().find(|(k, _)| k == "to") {
                    out.push(to.clone());
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Lays out a basis (three tables, member order set by the canonical byte rules).
pub fn basis_of(g: &Ground) -> Value {
    let window = Value::Obj(vec![
        ("chainId".to_string(), Value::Int(g.chain)),
        ("fromBlock".to_string(), Value::Int(g.from_block)),
        (
            "registries".to_string(),
            Value::Arr(vec![Value::Str(g.registry.hex())]),
        ),
        (
            "senders".to_string(),
            Value::Arr(g.senders.iter().map(|s| Value::Str(s.clone())).collect()),
        ),
        ("toBlock".to_string(), Value::Int(g.to_block)),
    ]);
    Value::Obj(vec![
        ("adoptionChains".to_string(), Value::Arr(Vec::new())),
        ("bareTx".to_string(), Value::Arr(Vec::new())),
        ("chains".to_string(), Value::Arr(vec![window])),
    ])
}

/// The largest number of endpoints on any chain: how many rounds a scan runs.
fn rounds(eps: &[Endpoint]) -> usize {
    let mut n = 1;
    for e in eps {
        n = n.max(eps.iter().filter(|x| x.chain == e.chain).count());
    }
    n
}

/// Which chains had only one endpoint answer this pass (this sets the single-source flag).
///
/// It counts endpoints that answered, not endpoints configured: with two configured and the second down, the
/// reading still has one source, and counting configured ones would wrongly claim that several sources agree.
/// Each chain is computed from the URLs that actually answered for it.
fn thin_from(answered: &[(u64, String)], chains: &[u64]) -> Vec<u64> {
    zikaron_anchor::endpoints::thin_chains(answered, chains)
}

fn nth_for(eps: &[Endpoint], chain: u64, k: usize) -> Option<String> {
    let mine: Vec<&str> = eps.iter().filter(|e| e.chain == chain).map(|e| e.url.for_transport()).collect();
    mine.get(k.min(mine.len().saturating_sub(1))).map(|u| (*u).to_string())
}

/// Whether a scan uses this machine's record of already-checked chain facts (`checkedx`). Every scan does,
/// except the grant check page's: that page has no privileges (no key, no disk, no sending), so it reads and
/// writes no local file and asks about every log.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facts {
    /// Read the record; an agreed scan adds what its endpoints read alike.
    Local,
    /// Neither read nor write it.
    Bare,
}

impl Facts {
    fn known(self) -> scan::Known {
        match self {
            Facts::Local => crate::checkedx::read(),
            Facts::Bare => scan::Known::default(),
        }
    }
}

/// Scans once without an audit. The succession desk asks how many anchors an address has sent, not for a
/// report.
///
/// Returns one scan's reading. The scan is still the anchoring crate's `scan::run`, and the basis is still
/// validated by the core.
pub struct Scanned1 {
    /// How many anchor records this pass found.
    pub anchors: usize,
    /// How many endpoints were asked.
    pub asked: usize,
    /// This pass's fragment, unchanged. The report built from it gives the anchor status lights a real source;
    /// without it every row read back could only say "recorded, not anchored", which was never verified.
    pub fragment: zikaron::json::Value,
}

pub fn scan_once(eps: &[Endpoint], g: &Ground) -> Result<Scanned1, Fault> {
    scan_once_noting(eps, g).map(|(s, _)| s)
}

/// Like [`scan_once`], also noting which registry each record came from (the scan's side reading).
pub fn scan_once_noting(eps: &[Endpoint], g: &Ground) -> Result<(Scanned1, scan::Emitters), Fault> {
    scan_first(eps, &zikaron::json::canon_bytes(&basis_of(g)))
}

/// One scan of the given basis bytes at each chain's first endpoint: the body of [`scan_once`], shared with
/// the read side across networks (`widex`), whose windows may name several registries.
pub fn scan_first(eps: &[Endpoint], basis_bytes: &[u8]) -> Result<(Scanned1, scan::Emitters), Fault> {
    let mut chains: Vec<u64> = eps.iter().map(|e| e.chain).collect();
    chains.sort_unstable();
    chains.dedup();
    let mut https: Vec<(u64, Box<dyn rpc::Endpoint>)> = Vec::new();
    for c in &chains {
        let Some(url) = nth_for(eps, *c, 0) else { continue };
        if let Some(h) = crate::chainx::endpoint_at(&url) {
            https.push((*c, h));
        }
    }
    if https.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::t(crate::lang::Key::Tail082).to_string()));
    }
    let asked = https.len();
    let mut handed: Vec<(u64, &mut dyn rpc::Endpoint)> = https
        .iter_mut()
        .map(|(c, h)| (*c, &mut **h as &mut dyn rpc::Endpoint))
        .collect();
    // One endpoint per chain: already-checked facts save questions, but nothing this single source reads is
    // added to the record.
    match scan::run_knowing(basis_bytes, &[], &mut handed, &Facts::Local.known()) {
        Ok(Ok((s, emitters, _))) => Ok((Scanned1 { anchors: s.anchors.len(), asked, fragment: scan::fragment(&s) }, emitters)),
        Ok(Err(_)) => Err(Fault::known(Known::AuditInput, crate::lang::t(crate::lang::Key::Tail083).to_string())),
        Err(r) => Err(Fault::scan_refused(Fault::scan_tail(&r), std::slice::from_ref(&r))),
    }
}

/// A scan reconciled by the endpoint rule. Each round scans every chain at its k-th endpoint, and rounds are
/// compared by the endpoint rule (`agree_over`); the single-source flag comes from the endpoints that
/// actually answered. No home, no ledger: this touches only the chain, so the check page (no privileges) and
/// the periodic self-audit share it, each assembling its own ledger half.
pub struct Agreed {
    /// The reconciled fragment, unchanged.
    pub fragment: Value,
    pub unanswered: Vec<String>,
    /// How many endpoints were asked.
    pub asked: usize,
    /// The single-source flag, given by the endpoint rule; this layer does not count.
    pub single_source: bool,
}

/// The two outcomes of a scan: a basis, or the core's validation saying this is not a basis (that no-label
/// result is passed through unchanged).
pub enum Scan {
    Basis(Agreed),
    NoLabel { report: Value, unanswered: Vec<String>, asked: usize },
}

pub fn scan_agreed(eps: &[Endpoint], g: &Ground) -> Result<Scan, Fault> {
    scan_agreed_with(eps, g, Facts::Local)
}

/// [`scan_agreed`], choosing whether this machine's record of checked facts is used ([`Facts`]).
pub fn scan_agreed_with(eps: &[Endpoint], g: &Ground, facts: Facts) -> Result<Scan, Fault> {
    scan_agreed_bytes_with(eps, &zikaron::json::canon_bytes(&basis_of(g)), facts)
}

/// [`scan_agreed`] over the given basis bytes: the read side across networks (`widex`) asks each chain's
/// window this way, which may name several registries.
pub fn scan_agreed_bytes(eps: &[Endpoint], basis_bytes: &[u8]) -> Result<Scan, Fault> {
    scan_agreed_bytes_with(eps, basis_bytes, Facts::Local)
}

/// [`scan_agreed_bytes`], choosing whether this machine's record of checked facts is used ([`Facts`]). With
/// [`Facts::Local`], once the endpoints agree, the facts every run read identically on a chain where more than
/// one endpoint answered are added to the record (`checkedx::agreed`); failing to add them only costs extra
/// questions next time, never this scan's answer.
pub fn scan_agreed_bytes_with(eps: &[Endpoint], basis_bytes: &[u8], facts: Facts) -> Result<Scan, Fault> {
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::t(crate::lang::Key::Tail084).to_string()));
    }
    let r = match rounds_over(eps, basis_bytes, &facts.known()) {
        Ok(r) => r,
        Err(no_label) => return Ok(no_label),
    };
    let fresh = crate::checkedx::agreed(&r.sightings, &thin_from(&r.answered, &r.chains));
    let (reading, unanswered, asked) = r.converge()?;
    if facts == Facts::Local {
        let _ = crate::checkedx::add(&fresh);
    }
    Ok(Scan::Basis(Agreed {
        fragment: reading.fragment,
        unanswered,
        asked,
        single_source: reading.single_source,
    }))
}

/// What the rounds of one agreed scan returned, before they are reconciled.
struct Rounds {
    runs: Vec<(String, Value)>,
    /// Which endpoint each run asked on each chain (chain id and url), in run order.
    places: Vec<Vec<(u64, String)>>,
    unanswered: Vec<String>,
    refused: Vec<zikaron_anchor::scan::Refusal>,
    /// The endpoints that actually answered (chain id and url); the single-source flag counts them.
    answered: Vec<(u64, String)>,
    asked: usize,
    chains: Vec<u64>,
    /// For each run that answered, the chains it read and the facts it read afresh (`checkedx::agreed`).
    sightings: Vec<(Vec<u64>, scan::Sightings)>,
}

impl Rounds {
    /// Reconciles by the endpoint rule: no round answering is the scan's refusal; rounds that differ are a
    /// disagreement.
    fn converge(self) -> Result<(zikaron_anchor::endpoints::Reading, Vec<String>, usize), Fault> {
        if self.runs.is_empty() {
            return Err(Fault::scan_refused(self.unanswered.join(" · "), &self.refused));
        }
        let thin = thin_from(&self.answered, &self.chains);
        let empty = answered_empty(&self.runs, &self.places);
        let reading = zikaron_anchor::endpoints::agree_over(self.runs, thin).map_err(|d| {
            // Still a disagreement (no majority, no first-come wins); if one endpoint returned nothing where
            // another returned records, the message names it: it lacks history back to the start block (a node
            // that keeps only recent logs answers older windows empty).
            let said = if empty.is_empty() {
                crate::lang::filln(crate::lang::Key::Tail086, &[&(d.sources.len()).to_string(), &(d.sources.join(" ")).to_string()])
            } else {
                crate::lang::filln(crate::lang::Key::TailNoHistory, &[&empty.join(" ")])
            };
            Fault::known(Known::Disagree, said)
        })?;
        Ok((reading, self.unanswered, self.asked))
    }
}

/// The endpoints that answered a chain with no records (no anchor, no evidence) while another endpoint answered
/// the same chain with some: each by its URL, once, in run order.
fn answered_empty(runs: &[(String, Value)], places: &[Vec<(u64, String)>]) -> Vec<String> {
    let count = |v: &Value, chain: u64| -> usize {
        ["anchors", "evidence"]
            .iter()
            .filter_map(|k| match v.member(k) {
                Some(Value::Arr(a)) => Some(a.iter().filter(|x| matches!(x.member("chainId"), Some(Value::Int(c)) if *c == chain)).count()),
                _ => None,
            })
            .sum()
    };
    let mut out: Vec<String> = Vec::new();
    for (i, (_, v)) in runs.iter().enumerate() {
        for (chain, url) in places.get(i).map(|p| p.as_slice()).unwrap_or(&[]) {
            let others = runs.iter().enumerate().any(|(j, (_, w))| j != i && places.get(j).map(|p| p.iter().any(|(c, _)| c == chain)).unwrap_or(false) && count(w, *chain) > 0);
            if count(v, *chain) == 0 && others && !out.contains(url) {
                out.push(url.clone());
            }
        }
    }
    out
}

/// Each round scans every chain at its k-th endpoint. If the core's validation says this is not a basis, the
/// no-label scan comes back unchanged.
fn rounds_over(eps: &[Endpoint], basis_bytes: &[u8], known: &scan::Known) -> Result<Rounds, Scan> {
    let mut chains: Vec<u64> = eps.iter().map(|e| e.chain).collect();
    chains.sort_unstable();
    chains.dedup();

    let mut runs: Vec<(String, Value)> = Vec::new();
    let mut places: Vec<Vec<(u64, String)>> = Vec::new();
    let mut unanswered: Vec<String> = Vec::new();
    let mut refused: Vec<zikaron_anchor::scan::Refusal> = Vec::new();
    // The endpoints that actually answered this pass (chain id and url); the single-source flag counts them.
    let mut answered: Vec<(u64, String)> = Vec::new();
    let mut asked = 0usize;
    let mut sightings: Vec<(Vec<u64>, scan::Sightings)> = Vec::new();
    // Open every round's nodes in order; the rounds then run concurrently (`zikaron_anchor::endpoints::each`)
    // and results are taken in round order, so the scan's result never depends on which answered first.
    type Round = (Vec<String>, Vec<(u64, String)>, Vec<(u64, Box<dyn rpc::Endpoint + Send>)>);
    let mut jobs: Vec<Round> = Vec::new();
    for k in 0..rounds(eps) {
        let mut said: Vec<String> = Vec::new();
        let mut https: Vec<(u64, Box<dyn rpc::Endpoint + Send>)> = Vec::new();
        let mut used: Vec<(u64, String)> = Vec::new();
        for c in &chains {
            let Some(url) = nth_for(eps, *c, k) else { continue };
            match crate::chainx::endpoint_at(&url) {
                Some(h) => {
                    used.push((*c, url));
                    https.push((*c, h));
                }
                None => said.push(format!("{c}={}", crate::chainx::address_said(&url))),
            }
        }
        jobs.push((said, used, https));
    }
    let lost_chain = chains.first().copied().unwrap_or(0);
    let done = zikaron_anchor::endpoints::each(
        jobs,
        |(said, used, mut https)| {
            if https.is_empty() {
                return (said, used, None);
            }
            let mut handed: Vec<(u64, &mut dyn rpc::Endpoint)> = https
                .iter_mut()
                .map(|(c, h)| (*c, &mut **h as &mut dyn rpc::Endpoint))
                .collect();
            // Every round reads the same record: one round's new facts never spare another round a question.
            let got = scan::run_knowing(basis_bytes, &[], &mut handed, known);
            (said, used, Some(got))
        },
        || (Vec::new(), Vec::new(), Some(Err(zikaron_anchor::scan::Refusal::Unanswered { chain: lost_chain, what: "the scan stopped short".into() }))),
    );
    for (k, (said, used, got)) in done.into_iter().enumerate() {
        unanswered.extend(said);
        let Some(got) = got else { continue };
        let names: Vec<String> = used.iter().map(|(c, _)| format!("{c}#{k}")).collect();
        asked += used.len();
        match got {
            // The core's validation says this is not a basis: pass the no-label result through unchanged,
            // never wrapped as a successful scan.
            Ok(Err(no_label)) => return Err(Scan::NoLabel { report: no_label, unanswered, asked }),
            Ok(Ok((s, _, seen))) => {
                // This round actually answered, so its URLs count as sources for the single-source flag.
                answered.extend(used.iter().cloned());
                sightings.push((used.iter().map(|(c, _)| *c).collect(), seen));
                places.push(used.clone());
                runs.push((names.join(","), scan::fragment(&s)))
            }
            Err(r) => {
                unanswered.push(Fault::scan_tail(&r));
                refused.push(r);
            }
        }
    }
    Ok(Rounds { runs, places, unanswered, refused, answered, asked, chains, sightings })
}

pub fn online(home: &Home, eps: &[Endpoint], g: &Ground) -> Result<Verdict, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, the
    // CLI) are traced too.
    crate::trace::mark(crate::feature::Feature::W2);
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::t(crate::lang::Key::Tail021).to_string()));
    }
    let ledger = home.ledger()?;
    let pile = ledger
        .pile()?;
    match scan_agreed(eps, g)? {
        Scan::NoLabel { report, unanswered, asked } => Ok(Verdict {
            label: String::new(),
            complete: false,
            report,
            entries: pile.items.len(),
            unanswered,
            asked,
            single_source: true,
            fragment: empty_fragment(),
            root: root_of(&pile.items).unwrap_or_default(),
        }),
        Scan::Basis(a) => ask_from(&pile.items, &a.fragment, a.unanswered, a.asked, a.single_source),
    }
}

/// Who holds the pen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pen {
    /// Writing is allowed.
    Granted,
    /// Held: after a restore, waiting for a reconciliation to report COMPLETE.
    Held,
}

impl Pen {
    pub fn as_str(self) -> &'static str {
        match self {
            Pen::Granted => "granted",
            Pen::Held => "held",
        }
    }

    pub fn writable(self) -> bool {
        matches!(self, Pen::Granted)
    }
}
