//! A restored identity is read-only until the full ledger is fetched.
//!
//! Restoring an identity onto this machine from a secret (recovery words, private key, key file) brings the
//! key back but not the ledger: both homes are empty. Writing a genesis then would start another ledger under
//! the name of someone who already has records on chain (a fork); writing a record would append at a place
//! merely "believed to be the tail". Restoring only the key would leave the homes writable as usual.
//!
//! So at the moment of restoring (the identity was not in the register before), each of the two homes gets a
//! mark [`FILE`] (in `settings/`; even when the home already has an old ledger). While the mark is present,
//! the closed table of write actions (`action::Action::writes_ledger`) is refused by name at the entry to
//! `apply`; only after fetching the full ledger (`Action::FetchLedger`) and checking its tail against this
//! identity's anchors on chain (every digest this identity anchored is in the fetched ledger) is the mark
//! removed and writing opened. If the chain has anchors this ledger lacks, "there are newer entries
//! elsewhere": the mark changes to that form, still read-only.
//!
//! The mark is a fact of this home and travels with it (copies are equivalent: a copied home is still
//! read-only and must have its tail checked the same way before writing).

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use zikaron::json::{self, Value};

/// The mark file name (in `settings/`). One name, one home.
pub const FILE: &str = "unfetched.json";

/// Which form the mark states. Closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    /// The ledger has not been fetched.
    Unfetched,
    /// Fetched, but the chain has anchors this ledger lacks (`missing` of them).
    NewerElsewhere { missing: usize },
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Unfetched => word::UNFETCHED,
            State::NewerElsewhere { .. } => word::NEWER_ELSEWHERE,
        }
    }

    /// The refusal when a write action is refused.
    pub fn fault(self) -> Fault {
        match self {
            State::Unfetched => Fault::known(Known::LedgerNotFetched, String::new()),
            State::NewerElsewhere { missing } => Fault::known(Known::NewerElsewhere, missing.to_string()),
        }
    }
}

/// The mark file's member names. One name, one home.
pub mod member {
    pub const STATE: &str = "state";
    pub const MISSING: &str = "missing";
}

/// The mark file's state words (`State::as_str` and the reader spell them only here).
pub mod word {
    pub const UNFETCHED: &str = "unfetched";
    pub const NEWER_ELSEWHERE: &str = "newer-elsewhere";
}

/// Place the mark (at the moment of restoring). Even when the home already has a ledger: it may be an old
/// ledger from before the identity was deleted, and writing may have continued elsewhere; writing opens only
/// after the tail is checked.
pub fn mark(home: &Home) -> Result<(), Fault> {
    write(home, State::Unfetched)
}

/// Place a form of mark (sealed, atomic, 0600, through `local::put`).
pub fn write(home: &Home, s: State) -> Result<(), Fault> {
    let mut m = vec![(member::STATE.to_string(), Value::Str(s.as_str().to_string()))];
    if let State::NewerElsewhere { missing } = s {
        m.insert(0, (member::MISSING.to_string(), Value::Int(missing as u64)));
    }
    crate::local::put(&home.dir(Slot::Settings), FILE, crate::local::Doc::Unfetched, &json::canon_bytes(&Value::Obj(m)))
}

/// Read the mark. None means writable; unreadable is refused by name (treated as read-only and still refusing
/// writes, never as absent).
pub fn read(home: &Home) -> Result<Option<State>, Fault> {
    let p = home.dir(Slot::Settings).join(FILE);
    let Some(bytes) = crate::local::read(&p, crate::local::Doc::Unfetched)? else { return Ok(None) };
    let bad = || Fault::known(Known::SettingsShape, p.display().to_string());
    let v = json::parse(&bytes).map_err(|_| bad())?;
    let Value::Obj(m) = &v else { return Err(bad()) };
    let state = m.iter().find(|(k, _)| k == member::STATE).and_then(|(_, v)| if let Value::Str(s) = v { Some(s.as_str()) } else { None });
    let missing = m.iter().find(|(k, _)| k == member::MISSING).and_then(|(_, v)| if let Value::Int(n) = v { Some(*n as usize) } else { None });
    match (state, missing) {
        (Some(word::UNFETCHED), _) => Ok(Some(State::Unfetched)),
        (Some(word::NEWER_ELSEWHERE), Some(n)) => Ok(Some(State::NewerElsewhere { missing: n })),
        _ => Err(bad()),
    }
}

/// Remove the mark (when the tail is checked).
pub fn clear(home: &Home) -> Result<(), Fault> {
    let p = home.dir(Slot::Settings).join(FILE);
    match std::fs::remove_file(&p) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(crate::fault::classify(&e, &p.display().to_string())),
    }
}

/// The tail check's answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tail {
    /// Every digest this identity anchored on chain is in this ledger (`anchors` of them; zero means the
    /// chain has no anchors from this identity).
    Pass { anchors: usize },
    /// The chain has anchors this ledger lacks.
    NewerElsewhere { missing: usize, anchors: usize },
}

/// Tail check: the scan fragment (`anchors[].hash`, only those sent by this identity) against this ledger's
/// entry ids. Pure; touches no disk.
pub fn tail_check(pile: &[Vec<u8>], fragment: &Value) -> Tail {
    let ids: Vec<String> = pile.iter().filter_map(|b| zikaron::entry::check(b).ok()).map(|e| e.id_hex().trim_start_matches("0x").to_ascii_lowercase()).collect();
    let hashes: Vec<String> = match fragment {
        Value::Obj(m) => match m.iter().find(|(k, _)| k == "anchors").map(|(_, v)| v) {
            Some(Value::Arr(a)) => a
                .iter()
                .filter_map(|x| match x {
                    Value::Obj(r) => r.iter().find(|(k, _)| k == "hash").and_then(|(_, v)| if let Value::Str(s) = v { Some(s.trim_start_matches("0x").to_ascii_lowercase()) } else { None }),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    let missing = hashes.iter().filter(|h| !ids.contains(h)).count();
    if missing == 0 {
        Tail::Pass { anchors: hashes.len() }
    } else {
        Tail::NewerElsewhere { missing, anchors: hashes.len() }
    }
}

/// Land the fetched entries in this home's ledger (skipping existing ones; stage all in a temporary place
/// first, then move each into place, as mirror restore does). Returns how many landed.
pub fn land(home: &Home, items: &[Vec<u8>]) -> Result<usize, Fault> {
    let ledger = home.ledger()?;
    // What this ledger already holds; a ledger that cannot be read is refused by name, never taken as empty
    // (that would land the fetched entries over ones it could not see).
    let mut have: Vec<[u8; 32]> = ledger.pile()?.items.iter().filter_map(|b| zikaron::entry::check(b).ok()).map(|e| e.id).collect();
    // Every entry is sealed first (sealing needs the vault open; refused before anything is staged).
    let nk = crate::names::key()?;
    let mut sealed: Vec<(zikaron_store::EntryName, Vec<u8>)> = Vec::new();
    for b in items {
        let e = zikaron::entry::check(b).map_err(|t| Fault::known(Known::EntryRefused, format!("{t:?}")))?;
        if have.contains(&e.id) {
            continue;
        }
        // Its name on disk is keyed by the names key (`names`), as every entry's is.
        let name = crate::local::entry_name_under(&nk, &e.id_hex())?;
        // An entry appearing twice in the fetched stack lands once.
        have.push(e.id);
        sealed.push((name, ledger.seal_entry(b)?));
    }
    let mut staged = Vec::new();
    for (name, bytes) in &sealed {
        match ledger.store().stage(name, bytes) {
            Ok(s) => staged.push(s),
            Err(t) => {
                for s in staged {
                    s.abandon();
                }
                return Err(Fault::known(Known::Ledger, format!("{t:?}")));
            }
        }
    }
    let mut n = 0;
    let mut rest = staged.into_iter();
    while let Some(s) = rest.next() {
        if let Err(t) = s.link().and_then(|l| l.seal()) {
            for left in rest {
                left.abandon();
            }
            return Err(Fault::known(Known::RestorePartial, format!("{t:?}")));
        }
        n += 1;
    }
    Ok(n)
}
