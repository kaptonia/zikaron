//! The exit gate: nothing this ledger holds leaves the machine until the chain has been read, now.
//!
//! Writing stays offline: entries land without asking the chain. What guards against a fork reaching the
//! chain is the way facts leave the machine, not the ways a key comes in (those are open: words on a new
//! machine, an old backup restored, a mark lost). The actions that let facts leave are a closed table
//! ([`Exit`], answered for every action by `Action::exit`); each of them, as its last step before the effect,
//! asks [`pass`]: this ledger's lineage's anchors are read from the chain now (no cache, no earlier reading),
//! this home's ledger is checked entry by entry, and every anchor must have an entry here that passes. An
//! anchor without one is refused as `NEWER_ELSEWHERE`, and this home gets the read-only mark that leads to
//! fetching; a chain that cannot be read is refused by the network codes, `NO_ENDPOINT` or `DISAGREE`.

use crate::fault::{Fault, Known};
use std::path::PathBuf;

/// The ways facts of this ledger leave the machine. Closed: every action is classified by `Action::exit`
/// (an exhaustive match), so a new action does not compile until it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exit {
    /// A batch of anchors sent on chain.
    Send,
    /// A record bundle exported.
    Kit,
    /// A grant file exported.
    GrantFile,
    /// A ledger mirror exported.
    Mirror,
    /// A badge exported.
    Badge,
}

impl Exit {
    pub const ALL: [Exit; 5] = [Exit::Send, Exit::Kit, Exit::GrantFile, Exit::Mirror, Exit::Badge];
}

/// What the gate needs, taken where the action starts (the frame): the chain cells and endpoints, this
/// home, and this seat's own address (a ledger with no entries yet still has one key whose anchors count).
#[derive(Clone, Debug)]
pub struct Ask {
    pub root: PathBuf,
    pub eps: Vec<crate::chainx::Endpoint>,
    pub chain: u64,
    pub registry: crate::key::Address,
    pub from_block: u64,
    pub own: Option<String>,
}

/// Take what the gate needs from the shell (no chain read here). Refused by name when a cell is missing:
/// an exit cannot pass without reading the chain.
pub fn ask_of(shell: &crate::shell::Shell) -> Result<Ask, Fault> {
    let chain = shell.settings.chain_id.ok_or_else(|| Fault::known(Known::NoChainId, String::new()))?;
    let registry = shell.settings.registry.ok_or_else(|| Fault::known(Known::NoRegistry, String::new()))?;
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf()).ok_or_else(|| Fault::known(Known::NoHome, String::new()))?;
    let eps: Vec<crate::chainx::Endpoint> = shell.endpoints.iter().filter(|e| e.chain == chain).cloned().collect();
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, chain.to_string()));
    }
    Ok(Ask { root, eps, chain, registry, from_block: shell.settings.from_block, own: shell.anchor.map(|a| a.hex()) })
}

/// What the gate read (kept for tests and for saying why).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reading {
    /// The lineage whose anchors were read (sorted).
    pub senders: Vec<String>,
    /// The anchored entry ids the chain answered (bare lower hex, sorted).
    pub anchored: Vec<String>,
    /// The entry ids of this home's ledger that passed checking (bare lower hex, sorted).
    pub verified: Vec<String>,
    /// Anchors with no checked entry here.
    pub missing: Vec<String>,
}

fn bare(s: &str) -> String {
    s.trim().trim_start_matches("0x").to_ascii_lowercase()
}

/// Read the chain now and judge. Every anchor of this ledger's lineage (every key that wrote an entry in it,
/// predecessors included, plus this seat's own address) must have, in this
/// home's ledger, an entry that passes checking (signature, and id equal to its content's hash). Blocking
/// network work: called on the exit's own last step (a background pass, or the frame for the exits that run
/// there).
pub fn read(ask: &Ask) -> Result<Reading, Fault> {
    let home = crate::home::Home::open(&ask.root)?;
    let pile = home.ledger()?.pile()?;
    let mut verified: Vec<String> = pile.items.iter().filter_map(|b| zikaron::entry::check(b).ok()).map(|e| bare(&e.id_hex())).collect();
    verified.sort();
    verified.dedup();
    // The keys that wrote in this ledger, and this seat's own: a successor it only names (a handover) keeps
    // writing elsewhere, and what it anchors there is not this home's to hold.
    let mut senders: Vec<String> = pile.items.iter().filter_map(|b| zikaron::entry::check(b).ok()).map(|e| e.author).collect();
    if let Some(a) = &ask.own {
        senders.push(a.clone());
    }
    let mut senders: Vec<String> = senders.iter().map(|s| format!("0x{}", bare(s))).collect();
    senders.sort();
    senders.dedup();
    let (head, _) = crate::chainx::head_block(&ask.eps, ask.chain)?;
    let g = crate::auditx::Ground { chain: ask.chain, registry: ask.registry, from_block: ask.from_block, to_block: head.max(ask.from_block), senders: senders.clone() };
    let fragment = match crate::auditx::scan_agreed(&ask.eps, &g)? {
        crate::auditx::Scan::Basis(a) => a.fragment,
        crate::auditx::Scan::NoLabel { .. } => return Err(Fault::known(Known::AuditInput, String::new())),
    };
    let mut anchored: Vec<String> = match fragment.member("anchors") {
        Some(zikaron::json::Value::Arr(a)) => a
            .iter()
            .filter_map(|x| match x.member("hash") {
                Some(zikaron::json::Value::Str(h)) => Some(bare(h)),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    anchored.sort();
    anchored.dedup();
    let missing: Vec<String> = anchored.iter().filter(|h| verified.binary_search(h).is_err()).cloned().collect();
    Ok(Reading { senders, anchored, verified, missing })
}

/// The gate itself: [`read`], then refuse by name when anything anchored is missing here, placing this home's
/// read-only mark (the way to fetching what is missing). The mark is an early warning, not what holds: the
/// next exit reads the chain again.
pub fn pass(ask: &Ask) -> Result<Reading, Fault> {
    // Not read (no node, none reached, nodes that disagree): the exit is not done, said as such; this home is
    // not marked.
    let r = read(ask).map_err(|f| f.worded(None, Some(crate::lang::Key::ExitRetryLater)))?;
    if !r.missing.is_empty() {
        let state = crate::restorex::State::NewerElsewhere { missing: r.missing.len() };
        // The mark that leads to fetching: when it cannot be written, that failure is the refusal (said now,
        // with its cause), never a refusal that points at a way the disk did not open.
        crate::home::Home::open(&ask.root).and_then(|home| crate::restorex::write(&home, state))?;
        return Err(state.fault().worded(Some(crate::lang::Key::ExitBehind), Some(crate::lang::Key::ExitBehindSay)));
    }
    Ok(r)
}
