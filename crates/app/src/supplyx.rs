//! Audit input resolved level by level. Shared by the grant check page and the vault re-check.
//!
//! Four of a grant's six checks need the issuer's ledger bytes (kit law §10.2: an empty audit input is
//! UNKNOWN). Where the bytes come from is resolved by source, level by level, in fixed order, and the face
//! names the level that supplied them:
//!
//! 1. This machine: when a seat's address in the local identity register is the issuer, read that seat's home
//! ledger (this does not count as an indexer);
//! 2. Vault: the ledger carried in the grant file kept at storing time, and the upstream ledger location the
//! person recorded for this grant;
//! 3. Record bundle: a bundle, directory or grant file the person points to, and the ledger carried by the
//! grant file being checked;
//! 4. Publish address: an `https://` address the person fills in, and the publish address pointer in the
//! grant file (fetched per its manifest, counting only when it passes kit verification).
//!
//! A level that cannot be read (directory unreadable, address unreachable, kit verification failed) does not
//! block later levels: that sentence is recorded by name in `misses` and the face says it; no material at any
//! level is "none". Zero permissions: read only, creating no directory and writing no home (other homes'
//! ledgers are read with `LedgerDir::open`, never created).

use crate::fault::Fault;
use std::path::PathBuf;

/// Which level the material came from. Closed, and the order is the priority.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Level {
    Local,
    Vault,
    Kit,
    Remote,
}

impl Level {
    pub const ALL: [Level; 4] = [Level::Local, Level::Vault, Level::Kit, Level::Remote];

    pub fn as_str(self) -> &'static str {
        match self {
            Level::Local => "local",
            Level::Vault => "vault",
            Level::Kit => "kit",
            Level::Remote => "remote",
        }
    }
}

/// The places available when resolving (gathered on the interface side and handed to the background; only
/// paths and addresses, no bytes).
#[derive(Clone, Debug, Default)]
pub struct Shelf {
    /// Local register: (seat address, that seat's home).
    pub homes: Vec<(String, PathBuf)>,
    /// Vault: the room of kept grant files (`grants-held/files`).
    pub kept: Option<PathBuf>,
    /// Vault: the upstream ledger location the person recorded for a grant (grant id, path or address).
    pub upstreams: Vec<(String, String)>,
    /// Places the person points to, row i for hop i (a path is the record bundle level, `https://` the
    /// publish address level).
    pub manual: Vec<Option<String>>,
    /// The entries carried by the grant file being checked (record bundle level) and its path.
    pub carried: Option<(String, Vec<Vec<u8>>)>,
    /// The publish address pointer in the grant file (publish address level).
    pub pointer: Option<String>,
}

/// The resolved material.
#[derive(Clone, Debug)]
pub struct Supply {
    pub level: Level,
    /// Where the material is (home path, grant file path, bundle path, address).
    pub place: String,
    /// The ledger containing this grant (picked by `auditx::ledger_of`).
    pub items: Vec<Vec<u8>>,
    /// Files at that place that could not be read as entries, each named (the face lists them line by line).
    pub rejected: Vec<crate::verifyx::Rejected>,
    /// Publish address level: how many items were fetched per the manifest (including it); other levels lack
    /// this cell.
    pub files: Option<usize>,
}

/// One hop's resolution reading: the material (when found), and which levels failed on the way (level, named
/// refusal).
#[derive(Clone, Debug, Default)]
pub struct Found {
    pub supply: Option<Supply>,
    pub misses: Vec<(Level, Fault)>,
}

/// Bytes at a place (path or address); unreadable is named (never silently empty).
fn read_at(where_: &str) -> Result<(Vec<Vec<u8>>, Vec<crate::verifyx::Rejected>), Fault> {
    crate::verifyx::bytes_named(where_)
}

/// A level that was read but gave nothing to take, while files there could not be read as entries: that place
/// is named with how many and the first reason (the refusal list is never dropped with the level).
fn unread(level: Level, place: &str, bad: &[crate::verifyx::Rejected]) -> Option<(Level, Fault)> {
    let first = bad.first()?;
    Some((level, Fault::known(crate::fault::Known::EntriesUnreadable, format!("{place} · {} · {} · {}", bad.len(), first.file, first.why))))
}

/// Which ledger is wanted.
#[derive(Clone, Copy, Debug)]
pub enum Want<'a> {
    /// The one containing this hop (a grant entry) (check page, vault re-check).
    Hop(&'a [u8]),
    /// This issuer's ledger (the reader's "view others' ledger"): only an address in hand, no hop.
    Author(&'a str),
}

/// Resolve a ledger (reader): the same four levels in the same order as the check page, accepting only a
/// ledger that contains entries signed by this author.
pub fn find_book(shelf: &Shelf, author: &str) -> Found {
    find(shelf, Want::Author(author), 0)
}

/// Resolve one hop. Order per [`Level::ALL`]; stops at the first level with material.
pub fn find(shelf: &Shelf, want: Want, index: usize) -> Found {
    let (want, author) = match want {
        Want::Hop(hop) => {
            let Ok(e) = zikaron::entry::check(hop) else { return Found::default() };
            (e.id_hex(), e.author.clone())
        }
        // No hop: the id is empty, and `auditx::ledger_of` matches by issuer only.
        Want::Author(a) => (String::new(), a.to_string()),
    };
    let mut found = Found::default();
    let take = |level: Level, place: String, items: Vec<Vec<u8>>, rejected: Vec<crate::verifyx::Rejected>| -> Option<Supply> {
        crate::auditx::ledger_of(&items, &want, &author).map(|items| Supply { level, place, items, rejected, files: None })
    };
    // Material read at a level that is not the ledger holding this grant (another issuer's) is not used, and the
    // level says so by name: a level that read something never falls through in silence.
    let not_this = |level: Level, place: &str| (level, crate::fault::Fault::known(crate::fault::Known::NotThisLedger, place.to_string()));
    // 1 · This machine: which register seat's address is the issuer; read that seat's home (opened read-only,
    // no directory created).
    for (addr, home) in &shelf.homes {
        if !addr.eq_ignore_ascii_case(&author) {
            continue;
        }
        let dir = home.join(crate::home::Slot::Ledger.as_str());
        // Another seat's home on this machine holds sealed entries (`local::Ledger`).
        match crate::local::Ledger::open(&dir).and_then(|l| l.pile()) {
            Ok(p) => {
                let read_some = !p.items.is_empty();
                if let Some(s) = take(Level::Local, home.display().to_string(), p.items, Vec::new()) {
                    return Found { supply: Some(s), misses: found.misses };
                }
                if read_some {
                    found.misses.push(not_this(Level::Local, &home.display().to_string()));
                }
            }
            Err(f) => found.misses.push((Level::Local, f)),
        }
    }
    // 2 · Vault: kept grant files, and the upstream locations the person recorded.
    if let Some(dir) = &shelf.kept {
        // No room yet means nothing kept; a room or a kept file that cannot be read is named, not skipped.
        let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
            Ok(l) => l.filter_map(|x| x.ok().map(|x| x.path())).filter(|p| p.is_file()).collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => {
                found.misses.push((Level::Vault, crate::fault::classify(&e, &dir.display().to_string())));
                Vec::new()
            }
        };
        paths.sort();
        for p in paths {
            // Same rule as the vault's own list (`grantfilex::kept`): a kept file that cannot be opened is named;
            // one that opens but is not a grant file of this vault is left out.
            match crate::local::read(&p, crate::local::Doc::KeptGrant) {
                Ok(Some(bytes)) => {
                    if let Ok(o) = crate::grantfilex::open_bytes(&bytes) {
                        if o.carries_ledger() {
                            if let Some(s) = take(Level::Vault, p.display().to_string(), o.ledger, Vec::new()) {
                                return Found { supply: Some(s), misses: found.misses };
                            }
                            found.misses.push(not_this(Level::Vault, &p.display().to_string()));
                        }
                    }
                }
                Ok(None) => {}
                Err(f) => found.misses.push((Level::Vault, f)),
            }
        }
    }
    if let Some((_, at)) = shelf.upstreams.iter().find(|(g, _)| g.eq_ignore_ascii_case(&want)) {
        // A stored relative path (older files) is refused by name, never resolved against the current
        // directory (`home::landing` requires absolute).
        match crate::home::landing(at).and_then(|_| read_at(at)) {
            Ok((items, bad)) => {
                let named = unread(Level::Vault, at, &bad);
                let read_some = !items.is_empty();
                if let Some(s) = take(Level::Vault, at.clone(), items, bad) {
                    return Found { supply: Some(s), misses: found.misses };
                }
                if read_some {
                    found.misses.push(not_this(Level::Vault, at));
                }
                found.misses.extend(named);
            }
            Err(f) => found.misses.push((Level::Vault, f)),
        }
    }
    // 3 · Record bundle: paths the person points to (those that are not addresses), and the entries carried
    // by the grant file being checked.
    let manual = shelf.manual.get(index).cloned().flatten().filter(|m| !m.trim().is_empty());
    if let Some(m) = manual.as_ref().filter(|m| !crate::fetchx::is_address(m)) {
        match read_at(m) {
            Ok((items, bad)) => {
                let named = unread(Level::Kit, m, &bad);
                let read_some = !items.is_empty();
                if let Some(s) = take(Level::Kit, m.clone(), items, bad) {
                    return Found { supply: Some(s), misses: found.misses };
                }
                if read_some {
                    found.misses.push(not_this(Level::Kit, m));
                }
                found.misses.extend(named);
            }
            Err(f) => found.misses.push((Level::Kit, f)),
        }
    }
    if let Some((at, items)) = &shelf.carried {
        if let Some(s) = take(Level::Kit, at.clone(), items.clone(), Vec::new()) {
            return Found { supply: Some(s), misses: found.misses };
        }
        if !items.is_empty() {
            found.misses.push(not_this(Level::Kit, at));
        }
    }
    // 4 · Publish address: the address the person fills in, and the pointer in the grant file.
    let remote: Vec<String> = manual.into_iter().filter(|m| crate::fetchx::is_address(m)).chain(shelf.pointer.clone()).collect();
    for url in remote {
        match crate::fetchx::base_of(&url).and_then(|b| crate::fetchx::fetch_kit(&b)) {
            Ok(got) => {
                let (items, bad) = crate::verifyx::entries_of_pairs(&got.pairs);
                let named = unread(Level::Remote, &got.url, &bad);
                let read_some = !items.is_empty();
                if let Some(mut s) = take(Level::Remote, got.url.clone(), items, bad) {
                    s.files = Some(got.files);
                    return Found { supply: Some(s), misses: found.misses };
                }
                if read_some {
                    found.misses.push(not_this(Level::Remote, &got.url));
                }
                found.misses.extend(named);
            }
            Err(f) => found.misses.push((Level::Remote, f)),
        }
    }
    found
}
