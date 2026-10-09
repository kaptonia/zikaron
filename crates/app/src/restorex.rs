//! A restored identity is read-only until its full ledger is fetched.
//!
//! Restoring an identity from a secret (recovery words, private key, key file) brings back the key but not
//! the ledger: both homes are empty. Writing a genesis then would start a second ledger under the name of
//! someone who already has records on chain (a fork), and writing a record would append at a guessed tail.
//!
//! So when an identity is restored (it was not in the register before), each of its two homes gets the mark
//! [`FILE`] in `settings/`, even if the home already holds an old ledger. While the mark exists, every write
//! action (`action::Action::writes_ledger`) is refused at the entry to `apply`. Only the exit gate's tail check
//! (`exitgate::tail`) removes the mark: every digest this ledger's lineage and this seat's key anchored on
//! chain, with nodes agreeing, must be present in the ledger, however the ledger arrived (fetched from a
//! whole-machine backup with `Action::FetchLedger`, adopted in place, or never existed: an empty ledger and a
//! key with no anchors pass at once via `Action::CheckTail`). If the chain has anchors the ledger lacks, the
//! mark changes to "newer entries elsewhere" and the home stays read-only.
//!
//! The mark belongs to the home and travels with it: a copied home is still read-only and needs the same
//! tail check before writing.

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use zikaron::json::{self, Value};

/// The mark file name (in `settings/`).
pub const FILE: &str = "unfetched.json";

/// Which state the mark records.
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

    /// The fault returned when a write action is refused.
    pub fn fault(self) -> Fault {
        match self {
            State::Unfetched => Fault::known(Known::LedgerNotFetched, String::new()),
            State::NewerElsewhere { missing } => Fault::known(Known::NewerElsewhere, missing.to_string()),
        }
    }
}

/// The mark file's member names.
pub mod member {
    pub const STATE: &str = "state";
    pub const MISSING: &str = "missing";
}

/// The mark file's state words (spelled only here, for `State::as_str` and the reader).
pub mod word {
    pub const UNFETCHED: &str = "unfetched";
    pub const NEWER_ELSEWHERE: &str = "newer-elsewhere";
}

/// Place the mark when an identity is restored, even if the home already has a ledger: it may be an old
/// ledger from before the identity was deleted, and writing may have continued elsewhere. Writing opens only
/// after the tail check.
pub fn mark(home: &Home) -> Result<(), Fault> {
    write(home, State::Unfetched)
}

/// Write the mark in the given state (sealed, atomic, 0600, via `local::put`).
pub fn write(home: &Home, s: State) -> Result<(), Fault> {
    let mut m = vec![(member::STATE.to_string(), Value::Str(s.as_str().to_string()))];
    if let State::NewerElsewhere { missing } = s {
        m.insert(0, (member::MISSING.to_string(), Value::Int(missing as u64)));
    }
    crate::local::put(&home.dir(Slot::Settings), FILE, crate::local::Doc::Unfetched, &json::canon_bytes(&Value::Obj(m)))
}

/// Read the mark. `None` means writable. An unreadable mark is an error (the home stays read-only), never
/// treated as absent.
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

/// Remove the mark (after the tail check passes).
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

/// Tail check: compare a scan fragment (`anchors[].hash`) with this ledger's entry ids. Pure; no disk access.
/// The caller chooses which anchors to ask for; only the exit gate (`exitgate::tail`) moves a home's mark.
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

/// Land fetched entries in this home's ledger, skipping existing ones. All are staged first, then each is
/// moved into place, as in mirror restore. Returns how many landed.
pub fn land(home: &Home, items: &[Vec<u8>]) -> Result<usize, Fault> {
    let ledger = home.ledger()?;
    // What the ledger already holds. An unreadable ledger is an error, never taken as empty (that would land
    // fetched entries over ones it could not see).
    let mut have: Vec<[u8; 32]> = ledger.pile()?.items.iter().filter_map(|b| zikaron::entry::check(b).ok()).map(|e| e.id).collect();
    // Seal every entry first (sealing needs the vault open, so this fails before anything is staged).
    let nk = crate::names::key()?;
    let mut sealed: Vec<(zikaron_store::EntryName, Vec<u8>)> = Vec::new();
    for b in items {
        let e = zikaron::entry::check(b).map_err(|t| Fault::known(Known::EntryRefused, format!("{t:?}")))?;
        if have.contains(&e.id) {
            continue;
        }
        // The on-disk name is keyed by the names key (`names`), as for every entry.
        let name = crate::local::entry_name_under(&nk, &e.id_hex())?;
        // An entry repeated in the fetched set lands once.
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
