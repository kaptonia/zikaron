//! The exit gate: nothing a ledger holds leaves the machine until the chain has been read, now.
//!
//! Writing stays offline: entries are written without asking the chain. Forks are kept off the chain by
//! guarding the ways facts leave the machine, not the ways a key arrives (those stay open: words on a new
//! machine, an old backup restored, a lost mark). The exits form a closed set ([`Exit`]); each one calls
//! [`pass`] as its last step before the effect. [`pass`] reads the anchors of this ledger's lineage from the
//! chain: the set of anchors is fetched afresh every time, while facts about an anchor that several nodes already
//! confirmed alike come from the end's checked-facts record ([`Facts`]); anchors not in it are asked about in
//! full. This home's ledger is checked entry by entry, and every anchor must have an entry here that passes. An
//! anchor without one is refused, and the home gets the end's read-only mark that leads to fetching
//! ([`End::behind`]); a chain that cannot be read is refused with a network code (`NoEndpoint` or `Disagree`).

use crate::node::Endpoint;
use crate::{Code, End, Facts, Trouble};
use std::path::PathBuf;

/// The ways facts of a ledger leave the machine. Closed: every action that lets facts out is classified as one,
/// so a new one is named here before it can pass.
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

/// What the gate needs, taken where the action starts: chain settings and endpoints, the home, and the
/// identity's own address (a ledger with no entries yet still has a key whose anchors count).
#[derive(Clone, Debug)]
pub struct Ask {
    pub root: PathBuf,
    pub eps: Vec<Endpoint>,
    pub chain: u64,
    pub registry: [u8; 20],
    pub from_block: u64,
    pub own: Option<String>,
}

/// What the gate needs from a home's settings (no chain read here). A missing setting is refused by name: an
/// exit cannot pass without reading the chain. Only that chain's nodes are kept.
pub fn ask(root: PathBuf, chain: Option<u64>, registry: Option<[u8; 20]>, from_block: u64, eps: &[Endpoint], own: Option<String>) -> Result<Ask, Trouble> {
    let chain = chain.ok_or_else(|| Trouble::text(Code::NoChainId, ""))?;
    let registry = registry.ok_or_else(|| Trouble::text(Code::NoRegistry, ""))?;
    let eps: Vec<Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
    if eps.is_empty() {
        return Err(Trouble::text(Code::NoEndpoint, chain.to_string()));
    }
    Ok(Ask { root, eps, chain, registry, from_block, own })
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
/// last step.
pub fn read<E: End + Facts>(ask: &Ask, end: &E) -> Result<Reading, E::Fail> {
    let pile = end.pile()?;
    let mut verified: Vec<String> = pile.iter().filter_map(|b| zikaron::entry::check(b).ok()).map(|e| bare(&e.id_hex())).collect();
    verified.sort();
    verified.dedup();
    // The keys that wrote in this ledger, plus the identity's own. A successor named only by a handover keeps
    // writing elsewhere, and its anchors there are not this home's to hold.
    let mut senders: Vec<String> = pile.iter().filter_map(|b| zikaron::entry::check(b).ok()).map(|e| e.author).collect();
    if let Some(a) = &ask.own {
        senders.push(a.clone());
    }
    let mut senders: Vec<String> = senders.iter().map(|s| format!("0x{}", bare(s))).collect();
    senders.sort();
    senders.dedup();
    let (head, _) = crate::node::head_block(&ask.eps, ask.chain)?;
    let registry = zikaron::hexfmt::encode(&ask.registry);
    let window = zikaron_glue::basis::Window { chain: ask.chain, registry: &registry, from_block: ask.from_block, to_block: head.max(ask.from_block), senders: &senders };
    let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&window));
    let fragment = match crate::agree::scan(&ask.eps, &basis, end)? {
        crate::agree::Scan::Basis(a) => a.fragment,
        crate::agree::Scan::NoLabel { .. } => return Err(Trouble::text(Code::AuditInput, "").into()),
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

/// A home's tail against the chain, judged by the gate's own [`read`] (same lineage, same node agreement):
/// `Ok((missing, anchors))`. Every check that clears a home's read-only mark or writes "newer entries elsewhere"
/// uses this, so no narrower check can clear a mark the gate placed.
pub fn tail<E: End + Facts>(ask: &Ask, end: &E) -> Result<(usize, usize), E::Fail> {
    let r = read(ask, end)?;
    Ok((r.missing.len(), r.anchored.len()))
}

/// Proof that the gate let an exit through: the reading it made, for the home it read. Only [`pass`]
/// constructs one, and every effect that lets facts leave the machine requires one (a batch's broadcast,
/// `batch::cast`; each end's exports), so none of them can run without the gate.
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
/// mark (the way to fetching what is missing, [`End::behind`]). The mark is an early warning, not the safeguard:
/// the next exit reads the chain again. On success returns the [`Pass`] the exits require.
pub fn pass<E: End + Facts>(ask: &Ask, end: &E) -> Result<Pass, E::Fail> {
    // Chain not read (no node, none reached, nodes disagree): the exit is refused as such and the home is not
    // marked.
    let r = read(ask, end).map_err(|f| end.unread(f))?;
    if !r.missing.is_empty() {
        // If the mark cannot be written, that failure is the refusal (reported now, with its cause), never a
        // refusal pointing to a fetch path that was not set up.
        return Err(end.behind(r.missing.len()));
    }
    Ok(Pass { reading: r, root: ask.root.clone() })
}

