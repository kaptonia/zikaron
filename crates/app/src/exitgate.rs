//! The exit gate: nothing this ledger holds leaves the machine until the chain has been read, now.
//!
//! Writing stays offline: entries are written without asking the chain. Forks are kept off the chain by
//! guarding the ways facts leave the machine, not the ways a key arrives (those stay open: words on a new
//! machine, an old backup restored, a lost mark). The exits form a closed set ([`Exit`], classified for every
//! action by `Action::exit`); each one calls [`pass`] as its last step before the effect. [`pass`] reads the
//! anchors of this ledger's lineage from the chain: the set of anchors is fetched afresh every time, while
//! facts about an anchor that several nodes already confirmed alike (same block hash, same log) come from
//! this machine's checked-facts cache (`checkedx`); anchors not in the cache are asked about in full. This
//! home's ledger is checked entry by entry, and every anchor must have an entry here that passes. An anchor
//! without one is refused as `NEWER_ELSEWHERE` and the home gets the read-only mark that leads to fetching;
//! a chain that cannot be read is refused with a network code (`NO_ENDPOINT` or `DISAGREE`).

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

/// What the gate needs, taken where the action starts (the frame): chain settings and endpoints, this home,
/// and the identity's own address (a ledger with no entries yet still has a key whose anchors count).
#[derive(Clone, Debug)]
pub struct Ask {
    pub root: PathBuf,
    pub eps: Vec<crate::chainx::Endpoint>,
    pub chain: u64,
    pub registry: crate::key::Address,
    pub from_block: u64,
    pub own: Option<String>,
}

/// Takes what the gate needs from the shell (no chain read here). A missing setting is refused by name: an
/// exit cannot pass without reading the chain.
pub fn ask_of(shell: &crate::shell::Shell) -> Result<Ask, Fault> {
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf()).ok_or_else(|| Fault::known(Known::NoHome, String::new()))?;
    ask_from(root, &shell.settings, &shell.endpoints, shell.anchor.map(|a| a.hex()))
}

/// The same for a home other than the open one: its own settings supply the chain settings and endpoints
/// (it may be on another network than the open home); `own` is its identity's address.
pub fn ask_for_home(home: &crate::home::Home, own: Option<String>) -> Result<Ask, Fault> {
    let s = crate::settings::Settings::read(home)?;
    let eps: Vec<crate::chainx::Endpoint> = s.endpoints.iter().filter_map(|x| crate::chainx::Endpoint::parse(x)).collect();
    ask_from(home.root().to_path_buf(), &s, &eps, own)
}

fn ask_from(root: PathBuf, s: &crate::settings::Settings, eps: &[crate::chainx::Endpoint], own: Option<String>) -> Result<Ask, Fault> {
    let chain = s.chain_id.ok_or_else(|| Fault::known(Known::NoChainId, String::new()))?;
    let registry = s.registry.ok_or_else(|| Fault::known(Known::NoRegistry, String::new()))?;
    let eps: Vec<crate::chainx::Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
    if eps.is_empty() {
        return Err(Fault::known(Known::NoEndpoint, chain.to_string()));
    }
    Ok(Ask { root, eps, chain, registry, from_block: s.from_block, own })
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

/// Reads the chain now and judges. Every anchor of this ledger's lineage (every key that wrote an entry in
/// it, predecessors included, plus the identity's own address) must have an entry in this home's ledger that
/// passes checking (signature, and id equal to its content hash). Blocking network work: called on the exit's
/// last step (in a background pass, or in the frame for exits that run there).
pub fn read(ask: &Ask) -> Result<Reading, Fault> {
    let home = crate::home::Home::open(&ask.root)?;
    let pile = home.ledger()?.pile()?;
    let mut verified: Vec<String> = pile.items.iter().filter_map(|b| zikaron::entry::check(b).ok()).map(|e| bare(&e.id_hex())).collect();
    verified.sort();
    verified.dedup();
    // The keys that wrote in this ledger, plus the identity's own. A successor named only by a handover keeps
    // writing elsewhere, and its anchors there are not this home's to hold.
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

/// A home's tail against the chain, judged by the gate's own [`read`] (same lineage, same node agreement).
/// Every check that clears a home's read-only mark or writes "newer entries elsewhere" (tail check,
/// fetching) uses this, so no narrower check can clear a mark the gate placed.
pub fn tail(ask: &Ask) -> Result<crate::restorex::Tail, Fault> {
    let r = read(ask)?;
    Ok(if r.missing.is_empty() {
        crate::restorex::Tail::Pass { anchors: r.anchored.len() }
    } else {
        crate::restorex::Tail::NewerElsewhere { missing: r.missing.len(), anchors: r.anchored.len() }
    })
}

/// Proof that the gate let an exit through: the reading it made, for the home it read. Only [`pass`]
/// constructs one, and the five effects that let facts leave the machine (`sign::anchor_send`,
/// `kitx::export`, `badgex::export`, `mirror::export`, `grantfilex::export`) each require one, so none of
/// them can run without the gate.
#[derive(Clone, Debug)]
pub struct Pass {
    reading: Reading,
    root: PathBuf,
}

impl Pass {
    /// What the gate read.
    pub fn reading(&self) -> &Reading {
        &self.reading
    }

    /// The home the gate read.
    pub fn root(&self) -> &std::path::Path {
        &self.root
    }
}

/// The gate itself: [`read`], then refuse by name if any anchor is missing here, placing the home's read-only
/// mark (the way to fetching what is missing). The mark is an early warning, not the safeguard: the next exit
/// reads the chain again. On success returns the [`Pass`] the exits require.
pub fn pass(ask: &Ask) -> Result<Pass, Fault> {
    // Chain not read (no node, none reached, nodes disagree): the exit is refused as such and the home is not
    // marked.
    let r = read(ask).map_err(|f| f.worded(None, Some(crate::lang::Key::ExitRetryLater)))?;
    if !r.missing.is_empty() {
        let state = crate::restorex::State::NewerElsewhere { missing: r.missing.len() };
        // If the mark cannot be written, that failure is the refusal (reported now, with its cause), never a
        // refusal pointing to a fetch path that was not set up.
        crate::home::Home::open(&ask.root).and_then(|home| crate::restorex::write(&home, state))?;
        return Err(state.fault().worded(Some(crate::lang::Key::ExitBehind), Some(crate::lang::Key::ExitBehindSay)));
    }
    Ok(Pass { reading: r, root: ask.root.clone() })
}
