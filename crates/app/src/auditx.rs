//! Anchor reconciliation. No audit conclusion is reached here.
//!
//! The shape of basis and fragment belongs to the anchoring crate (`scan::fragment`), the assembly of the
//! audit input to its `input::assemble`, and the fifteen-item report with its four labels to the core's
//! `audit`. This layer does two parameter jobs: build the pile from the ledger and find the root; then it
//! passes the label the core returns through unchanged.
//!
//! ─── Releasing the pen ───
//!
//! "Only an audit returning COMPLETE releases the pen" depends on [`Verdict::complete`], and that field only
//! checks whether the core's label is its own [`complete`]. The shell does not lean toward green: other
//! labels are passed through unchanged and the pen stays held.

use crate::chainx::Endpoint;
use crate::fault::{Fault, Known};
use crate::home::Home;
use crate::key::Address;
use zikaron::json::Value;
use zikaron::tokens::Key;
use zikaron_anchor::input;
use zikaron_anchor::rpc;
use zikaron_anchor::scan::{self, Scanned};

/// The label that releases the pen. Its spelling is taken from the core's closed type (the labels of law §8.7
/// are the law's words and live only in the base).
///
/// A local string constant would not turn red when the law changed a letter; the shell would silently never
/// release the pen again.
pub fn complete() -> &'static str {
    zikaron::tokens::Label::Complete.as_str()
}

/// Whether this pass's chain reading is whole (decided here only): endpoints were asked (the offline path
/// asks none), every endpoint asked answered, and the core's label is readable and says "the data is all
/// here" (`COMPLETE` or `GAPS`). `GAPS` means some ledger entry is unanchored or missing, which is the
/// ledger's matter; the reading itself is whole. `UNAVAILABLE`, `BROKEN_CHAIN` and an unreadable label are
/// not whole. The places that decide from "this entry is not in the last pass's anchor set" (re-queueing the
/// root) accept only a whole reading: in an incomplete one, "absent" does not mean unanchored.
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
    /// The report itself, for the face to expand.
    pub report: Value,
    /// How many ledger entries this pass reconciled.
    pub entries: usize,
    /// Which endpoints did not answer this pass. An empty list does not mean all answered: the offline path
    /// asks no endpoint at all.
    pub unanswered: Vec<String>,
    /// How many endpoints were asked (zero offline).
    pub asked: usize,
    /// Only one place answered a load-bearing read: flagged explicitly.
    pub single_source: bool,
    /// The fragment this pass used, unchanged. The depth reading must be computed on the same basis, so it
    /// travels with the reading; the face cannot derive it from the report (the report has no anchor set or
    /// evidence set).
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

/// The report's sixteen members (law §8.7 items 1 to 15; item 1 takes two). Keys come from the core's closed
/// type: if the core renames or removes one, this table stops compiling; if the core adds one and this table
/// does not, the self-check suite's count catches it.
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

/// One report item laid out for the face. `count` exists only for the table items.
pub struct Item {
    pub key: &'static str,
    pub count: Option<usize>,
    /// A one-line reading for the non-table items (root, entries, label).
    pub said: String,
}

/// Lay the report out as sixteen items. An unreadable item says so, never zero: "the core did not give this
/// item" and "this item is an empty table" differ on the face.
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

/// One item's table from the report (used when the face lists MISSING / UNPROVEN / VOID / EXCLUDED row by
/// row).
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

/// Law §9.4's basis with its three members, no more and no fewer, all three tables empty.
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
/// ledger that contains `want`; if none does, the one whose lineage has `author` (the kit crate then judges
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

/// Offline reconciliation: all three basis tables empty. Taken when the chain is unreachable, and its label
/// comes from the core, never guessed here: "chain unread" and "not on chain" are different, and the core's
/// labels tell them apart.
pub fn offline(home: &Home) -> Result<Verdict, Fault> {
    let ledger = home.ledger()?;
    let pile = ledger
        .pile()?;
    ask(&pile.items, &Scanned { anchors: Vec::new(), evidence: Vec::new(), basis: empty_basis() })
}

/// Offline reconciliation of a stack of entries (nothing written; used to reconcile a fetched ledger before
/// it lands).
pub fn offline_items(items: &[Vec<u8>]) -> Result<Verdict, Fault> {
    ask(items, &Scanned { anchors: Vec::new(), evidence: Vec::new(), basis: empty_basis() })
}

/// Build the audit input from a scan result and the ledger, and have the core produce the report.
pub fn ask(items: &[Vec<u8>], scanned: &Scanned) -> Result<Verdict, Fault> {
    ask_from(items, &scan::fragment(scanned), Vec::new(), 0, false)
}

/// As above, with the fragment given by the caller (the online path has already converged the endpoints).
pub fn ask_from(
    items: &[Vec<u8>],
    fragment: &Value,
    unanswered: Vec<String>,
    asked: usize,
    single_source: bool,
) -> Result<Verdict, Fault> {
    let root = root_of(items)?;
    let assembled = input::assemble(fragment, &root, &pile_hex(items), &[])
        .ok_or_else(|| Fault::known(Known::AuditInput, crate::lang::t(crate::lang::Key::Tail080).to_string()))?;
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

/// An audit input, for the places that take it to the kit crate.
///
/// The depth reading and the six checks both take an audit input or audit outcome, and assembling the input
/// has one owner ([`zikaron_anchor::input::assemble`]); this layer does not assemble a separate one for each.
pub fn input_of(items: &[Vec<u8>], fragment: &Value) -> Result<Value, Fault> {
    let root = root_of(items)?;
    input::assemble(fragment, &root, &pile_hex(items), &[])
        .ok_or_else(|| Fault::known(Known::AuditInput, crate::lang::t(crate::lang::Key::Tail080).to_string()))
}

/// The same input handed to the core for the audit outcome (the report plus ledger, findings, anchor set and
/// lineage).
///
/// The kit crate's depth reading and chain check need exactly this. The shell recomputes nothing; this layer
/// only passes the input along.
pub fn outcome_of(items: &[Vec<u8>], fragment: &Value) -> Result<zikaron::audit::Outcome, Fault> {
    let input = input_of(items, fragment)?;
    zikaron::audit::audit_full(&input)
        .ok_or_else(|| Fault::known(Known::AuditInput, crate::lang::t(crate::lang::Key::Tail081).to_string()))
}

/// **Each entry's first-anchor block time**: the earliest block timestamp among the counted
/// anchors that reach it (kit law §8.2, the same reading as the depth "first anchored").
/// `None` when the fragment does not audit; entries no anchor reaches are left out.
pub fn first_anchored(items: &[Vec<u8>], fragment: &Value) -> Option<Vec<(String, u64)>> {
    let outcome = outcome_of(items, fragment).ok()?;
    let lines = zikaron_kit::reading::Lines::of(&outcome.ledger);
    let bounds = zikaron_kit::reading::bounds(&outcome, &lines);
    Some(bounds.iter().enumerate().filter_map(|(i, t)| t.map(|t| (lines.id(i).to_string(), t))).collect())
}

/// A fragment with an empty basis (the offline path needs it).
pub fn empty_fragment() -> Value {
    scan::fragment(&Scanned { anchors: Vec::new(), evidence: Vec::new(), basis: empty_basis() })
}

// ───────────────────────── Basis and the online pass ─────────────────────────

/// The cells of a basis declaration (law §9.4). Built by the face: the person fills in chain id, endpoints
/// and scan window on the settings page; the senders are computed from this ledger itself (see
/// [`senders_of`]).
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
/// This is a superset: law §8.1 discards anchor records outside the whole set's lineage and lists them in
/// DISCARDED, so declaring wider only scans more, never counts more. Sorting and deduplicating is required by
/// §9.4 (one scan, one assembly).
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

/// Lay out a basis (law §9.4's three tables, with member order set by the canonical byte rules).
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

/// Which chains had only one place answer this pass (this decides the single-source flag).
///
/// It counts endpoints that answered, not endpoints configured: with two configured and the second down, the
/// reading still has one source, and counting configured ones would show "several sources agree", saying
/// "both places say so" for "unknown". Each chain is computed from the urls that actually answered for it.
fn thin_from(answered: &[(u64, String)], chains: &[u64]) -> Vec<u64> {
    chains
        .iter()
        .copied()
        .filter(|c| {
            let mut urls: Vec<&String> =
                answered.iter().filter(|(x, _)| x == c).map(|(_, u)| u).collect();
            urls.sort();
            urls.dedup();
            urls.len() < 2
        })
        .collect()
}

fn nth_for(eps: &[Endpoint], chain: u64, k: usize) -> Option<String> {
    let mine: Vec<&String> = eps.iter().filter(|e| e.chain == chain).map(|e| &e.url).collect();
    mine.get(k.min(mine.len().saturating_sub(1))).map(|u| (*u).to_string())
}

/// Scan once, no audit. The succession desk asks "how many anchors has this address sent", not for a report.
///
/// Returns one scan's reading. The scan is still the anchoring crate's `scan::run`, and the basis is still
/// accepted by the core.
pub struct Scanned1 {
    /// How many anchor records this pass found.
    pub anchors: usize,
    /// How many endpoints were asked.
    pub asked: usize,
    /// This pass's fragment, unchanged. The report built from it gives the anchor lights a real source;
    /// without it, every row read back could only say "recorded, not anchored", a claim never verified.
    pub fragment: zikaron::json::Value,
}

pub fn scan_once(eps: &[Endpoint], g: &Ground) -> Result<Scanned1, Fault> {
    let basis_bytes = zikaron::json::canon_bytes(&basis_of(g));
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
    match scan::run(&basis_bytes, &[], &mut handed) {
        Ok(Ok(s)) => Ok(Scanned1 { anchors: s.anchors.len(), asked, fragment: scan::fragment(&s) }),
        Ok(Err(_)) => Err(Fault::known(Known::AuditInput, crate::lang::t(crate::lang::Key::Tail083).to_string())),
        Err(r) => Err(Fault::scan_refused(Fault::scan_tail(&r), std::slice::from_ref(&r))),
    }
}

/// A scan converged by the endpoint rule. Each round scans every chain at its k-th endpoint, and rounds are
/// compared by the endpoint rule (`agree_over`); the single-source flag comes from the endpoints that
/// actually answered. No home, no ledger: this touches only the chain, so the check page (zero permissions)
/// and the self-audit clock share it; each assembles its own ledger half.
pub struct Agreed {
    /// The converged fragment, unchanged.
    pub fragment: Value,
    pub unanswered: Vec<String>,
    /// How many endpoints were asked.
    pub asked: usize,
    /// The single-source flag: given by the endpoint rule; this layer does not count.
    pub single_source: bool,
}

/// The two answers of a scan: a basis, or the law check saying this is not a basis (that no-label result is
/// passed through unchanged).
pub enum Scan {
    Basis(Agreed),
    NoLabel { report: Value, unanswered: Vec<String>, asked: usize },
}

pub fn scan_agreed(eps: &[Endpoint], g: &Ground) -> Result<Scan, Fault> {
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, crate::lang::t(crate::lang::Key::Tail084).to_string()));
    }
    let basis_bytes = zikaron::json::canon_bytes(&basis_of(g));
    let mut chains: Vec<u64> = eps.iter().map(|e| e.chain).collect();
    chains.sort_unstable();
    chains.dedup();

    let mut runs: Vec<(String, Value)> = Vec::new();
    let mut unanswered: Vec<String> = Vec::new();
    let mut refused: Vec<zikaron_anchor::scan::Refusal> = Vec::new();
    // The places that actually answered this pass (chain id and url); the single-source flag counts them.
    let mut answered: Vec<(u64, String)> = Vec::new();
    let mut asked = 0usize;
    for k in 0..rounds(eps) {
        let mut https: Vec<(u64, Box<dyn rpc::Endpoint>)> = Vec::new();
        let mut used: Vec<(u64, String)> = Vec::new();
        for c in &chains {
            let Some(url) = nth_for(eps, *c, k) else { continue };
            match crate::chainx::endpoint_at(&url) {
                Some(h) => {
                    used.push((*c, url));
                    https.push((*c, h));
                }
                None => unanswered.push(crate::lang::filln(crate::lang::Key::Tail085, &[&(c).to_string(), &(url).to_string()])),
            }
        }
        if https.is_empty() {
            continue;
        }
        let names: Vec<String> = https.iter().map(|(c, _)| format!("{c}#{k}")).collect();
        asked += https.len();
        let mut handed: Vec<(u64, &mut dyn rpc::Endpoint)> = https
            .iter_mut()
            .map(|(c, h)| (*c, &mut **h as &mut dyn rpc::Endpoint))
            .collect();
        match scan::run(&basis_bytes, &[], &mut handed) {
            // The law check says this is not a basis: the no-label result is passed through unchanged, never
            // wrapped as a successful scan.
            Ok(Err(no_label)) => return Ok(Scan::NoLabel { report: no_label, unanswered, asked }),
            Ok(Ok(s)) => {
                // This round actually answered, so these urls count as sources (the single-source flag
                // counts them).
                answered.extend(used.iter().cloned());
                runs.push((names.join(","), scan::fragment(&s)))
            }
            Err(r) => {
                unanswered.push(Fault::scan_tail(&r));
                refused.push(r);
            }
        }
    }
    if runs.is_empty() {
        return Err(Fault::scan_refused(unanswered.join(" · "), &refused));
    }
    let thin = thin_from(&answered, &chains);
    let reading = zikaron_anchor::endpoints::agree_over(runs, thin).map_err(|d| {
        Fault::known(
            Known::Disagree,
            crate::lang::filln(crate::lang::Key::Tail086, &[&(d.sources.len()).to_string(), &(d.sources.join(" ")).to_string()]),
        )
    })?;
    Ok(Scan::Basis(Agreed {
        fragment: reading.fragment,
        unanswered,
        asked,
        single_source: reading.single_source,
    }))
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
