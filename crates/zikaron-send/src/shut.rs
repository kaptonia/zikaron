//! The write gates a home keeps, judged from what is on disk, for every road that sends its anchors: the sender
//! asks them the same way for the desktop and for the command line with its credential, before anything is asked
//! of the chain (the desktop's `Shell::may_write_entries` keeps the same three from what it holds).
//!
//! - **The read-only mark** (`zikaron_seal::unfetched`): a restored identity's home not yet fetched, or one whose
//!   chain holds anchors its ledger lacks, sends nothing until its tail is checked.
//! - **A broken ledger**: its own entries give a hard finding (the core's offline audit, the label
//!   `BROKEN_CHAIN`); anchoring them would put the break on chain.
//! - **Handed off** (`zikaron_seal::pens`): a ledger this machine handed to another place is not written here
//!   until it comes back.

use crate::{Code, End, Trouble, Why};
use zikaron::json::Value;
use zikaron_seal::unfetched::State;

/// Ask the three for `end`'s home, in the desktop's order: the mark, the ledger, the pen.
pub fn check<E: End>(end: &E) -> Result<(), E::Fail> {
    match end.mark()? {
        Some(State::Unfetched) => return Err(Trouble::text(Code::NotFetched, "").into()),
        Some(State::NewerElsewhere { missing }) => return Err(Trouble::text(Code::NewerElsewhere, missing.to_string()).into()),
        None => {}
    }
    let pile = end.pile()?;
    let names = breaks(&pile);
    if !names.is_empty() {
        return Err(Trouble::new(Code::Broken, Why::Text(names.join(" "))).into());
    }
    let Some(root) = pile.iter().filter_map(|b| zikaron::entry::check(b).ok()).find(|e| e.seq == 0).map(|e| e.id) else { return Ok(()) };
    match zikaron_seal::pens::may_write(&end.machine()?, &root) {
        Err(t) if t.code == zikaron_seal::Code::HandedOff => Err(Trouble::text(Code::HandedOff, t.tail).into()),
        // The list itself not read: the sealed layer's own trouble, as the end words it, not one of the gates.
        Err(t) => Err(t.into()),
        Ok(()) => Ok(()),
    }
}

/// The hard findings the core's offline audit gives this pile (sorted, each name once): what makes its label
/// `BROKEN_CHAIN` (`zikaron::audit::label_of`). A pile with no single genesis is not audited here, and gives none.
pub fn breaks(pile: &[Vec<u8>]) -> Vec<String> {
    let mut roots: Vec<String> = pile.iter().filter_map(|b| zikaron::entry::check(b).ok()).filter(|e| e.seq == 0).map(|e| e.author.clone()).collect();
    roots.sort();
    roots.dedup();
    let [root] = roots.as_slice() else { return Vec::new() };
    let empty = zikaron_anchor::scan::Scanned { anchors: Vec::new(), evidence: Vec::new(), basis: empty_basis() };
    let hex: Vec<String> = pile.iter().map(|b| zikaron::hexfmt::encode(b)).collect();
    let Some(input) = zikaron_anchor::input::assemble(&zikaron_anchor::scan::fragment(&empty), root, &hex, &[]) else { return Vec::new() };
    let Some(out) = zikaron::audit::audit_full(&input) else { return Vec::new() };
    let mut names: Vec<String> = out.findings.iter().filter(|f| f.hard).map(|f| f.name.as_str().to_string()).collect();
    names.sort();
    names.dedup();
    names
}

/// The three members of a law §9.4 basis, every table empty.
fn empty_basis() -> Value {
    Value::Obj(vec![
        ("adoptionChains".to_string(), Value::Arr(Vec::new())),
        ("bareTx".to_string(), Value::Arr(Vec::new())),
        ("chains".to_string(), Value::Arr(Vec::new())),
    ])
}
