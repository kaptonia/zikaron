//! The exit gate, wired: nothing this ledger holds leaves the machine until the chain has been read, now. The
//! gate is the sender's (`zikaron_send::gate`, one with the command line): the closed set of exits, the reading,
//! the judgement and the [`Pass`] every exit's effect requires. What stays here is where its question comes from
//! (this home's settings and nodes, or another home's), and the home it reads ([`crate::sendx::Desk`]: this
//! machine's seal, its words for the gate's refusals, the read-only mark).

use crate::fault::Fault;
use std::path::PathBuf;

pub use zikaron_send::gate::{Ask, Exit, Pass, Reading};

/// Takes what the gate needs from the shell (no chain read here). A missing setting is refused by name: an
/// exit cannot pass without reading the chain.
pub fn ask_of(shell: &crate::shell::Shell) -> Result<Ask, Fault> {
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf()).ok_or_else(|| Fault::known(crate::fault::Known::NoHome, String::new()))?;
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
    Ok(zikaron_send::gate::ask(root, s.chain_id, s.registry.map(|r| r.0), s.from_block, eps, own)?)
}

/// Reads the chain now and judges (`zikaron_send::gate::read`), for the home the question names.
pub fn read(ask: &Ask) -> Result<Reading, Fault> {
    zikaron_send::gate::read(ask, &crate::sendx::Desk::open(&ask.root)?)
}

/// A home's tail against the chain, judged by the gate's own reading (same lineage, same node agreement).
/// Every check that clears a home's read-only mark or writes "newer entries elsewhere" (tail check,
/// fetching) uses this, so no narrower check can clear a mark the gate placed.
pub fn tail(ask: &Ask) -> Result<crate::restorex::Tail, Fault> {
    let (missing, anchors) = zikaron_send::gate::tail(ask, &crate::sendx::Desk::open(&ask.root)?)?;
    Ok(if missing == 0 { crate::restorex::Tail::Pass { anchors } } else { crate::restorex::Tail::NewerElsewhere { missing, anchors } })
}

/// The gate itself (`zikaron_send::gate::pass`): read, then refuse by name if any anchor is missing here, placing
/// the home's read-only mark. On success returns the [`Pass`] the exits require.
pub fn pass(ask: &Ask) -> Result<Pass, Fault> {
    // A home that cannot be opened is a chain not read for this exit, worded as the gate words one.
    let desk = crate::sendx::Desk::open(&ask.root).map_err(|f| f.worded(None, Some(crate::lang::Key::ExitRetryLater)))?;
    zikaron_send::gate::pass(ask, &desk)
}
