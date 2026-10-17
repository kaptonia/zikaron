//! A home's read-only mark: a restored identity's home takes no new entry and sends no anchor until its ledger is
//! fetched and its tail checked against the chain, and a home whose chain holds anchors its ledger lacks stays so
//! ("newer entries elsewhere"). The mark is a sealed file in the home's settings room ([`FILE`], sealed as
//! `Doc::Unfetched`); its shape is read and written here only, by the desktop (`restorex`) and by the command line
//! opening the home with its credential alike. Who places and clears it is the desktop's restore and exit gate,
//! and the sender's exit gate on either end.

use crate::envelope::{self as env, Doc, Expect, Ident, Owner, SETTINGS_DIR};
use crate::keybox::LocalKey;
use crate::names::NameKey;
use crate::trouble::{Code, Trouble};
use std::path::Path;
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
}

/// The mark file's member names.
pub mod member {
    pub const STATE: &str = "state";
    pub const MISSING: &str = "missing";
}

/// The mark file's state words (spelled only here).
pub mod word {
    pub const UNFETCHED: &str = "unfetched";
    pub const NEWER_ELSEWHERE: &str = "newer-elsewhere";
}

/// The mark's plain bytes (canonical JSON).
pub fn render(s: State) -> Vec<u8> {
    let mut m = vec![(member::STATE.to_string(), Value::Str(s.as_str().to_string()))];
    if let State::NewerElsewhere { missing } = s {
        m.insert(0, (member::MISSING.to_string(), Value::Int(missing as u64)));
    }
    json::canon_bytes(&Value::Obj(m))
}

/// The mark from its plain bytes; anything else is refused (`SettingsShape`, `what`): an unreadable mark keeps the
/// home read-only, never read as absent.
pub fn of_plain(bytes: &[u8], what: &str) -> Result<State, Trouble> {
    let bad = || Trouble::new(Code::SettingsShape, what.to_string());
    let v = json::parse(bytes).map_err(|_| bad())?;
    let Value::Obj(m) = &v else { return Err(bad()) };
    let state = m.iter().find(|(k, _)| k == member::STATE).and_then(|(_, v)| if let Value::Str(s) = v { Some(s.as_str()) } else { None });
    let missing = m.iter().find(|(k, _)| k == member::MISSING).and_then(|(_, v)| if let Value::Int(n) = v { Some(*n as usize) } else { None });
    match (state, missing) {
        (Some(word::UNFETCHED), _) => Ok(State::Unfetched),
        (Some(word::NEWER_ELSEWHERE), Some(n)) => Ok(State::NewerElsewhere { missing: n }),
        _ => Err(bad()),
    }
}

/// The mark's place in a home, as its seal names it.
pub fn rel() -> String {
    format!("{SETTINGS_DIR}/{FILE}")
}

/// Read the mark of the home at `home` (numbered `number`) with the keys sealed files are opened with: `None`, no
/// mark (writable).
pub fn read(home: &Path, local: &LocalKey, names: &NameKey, number: &str) -> Result<Option<State>, Trouble> {
    let rel = rel();
    let at = home.join(&rel);
    let raw = match std::fs::read(&at) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Trouble::io(&e, &at.display().to_string())),
    };
    let ex = Expect { owner: Some(Owner::Home(number.to_string())), doc: Doc::Unfetched, rel: Some(rel.clone()) };
    let plain = env::open_with(local, &ex, &raw, &rel, Some(names))?;
    of_plain(&plain, &at.display().to_string()).map(Some)
}

/// Write the mark of the home at `home` (numbered `number`) whole, sealed as the desktop seals it.
pub fn write(home: &Path, local: &LocalKey, number: &str, s: State) -> Result<(), Trouble> {
    let sealed = env::seal_checked(local, &Ident { owner: Owner::Home(number.to_string()), doc: Doc::Unfetched, logical: rel() }, &render(s))?;
    crate::land::put_at(&home.join(SETTINGS_DIR), FILE, &sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each state back as written; a state word not written here, a missing count or a value that is not an
    /// object is refused, never read as no mark.
    #[test]
    fn the_mark_reads_back_as_written() {
        for s in [State::Unfetched, State::NewerElsewhere { missing: 0 }, State::NewerElsewhere { missing: 13 }] {
            assert_eq!(of_plain(&render(s), "m"), Ok(s));
        }
        for bad in [r#"{"state":"later"}"#, r#"{"state":"newer-elsewhere"}"#, r#"{"missing":1}"#, "[]", "x"] {
            assert_eq!(of_plain(bad.as_bytes(), "m"), Err(Trouble::new(Code::SettingsShape, "m")), "{bad}");
        }
    }
}
