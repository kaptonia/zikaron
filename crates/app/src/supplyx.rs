use crate::fault::Fault;
use std::path::PathBuf;

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

#[derive(Clone, Debug, Default)]
pub struct Shelf {
    pub homes: Vec<(String, PathBuf)>,
    pub kept: Option<PathBuf>,
    pub upstreams: Vec<(String, String)>,
    pub manual: Vec<Option<String>>,
    pub carried: Option<(String, Vec<Vec<u8>>)>,
    pub pointer: Option<String>,
    pub reads: Vec<crate::readnets::Net>,
}

#[derive(Clone, Debug)]
pub struct Supply {
    pub level: Level,
    pub place: String,
    pub items: Vec<Vec<u8>>,
    pub rejected: Vec<crate::verifyx::Rejected>,
    pub files: Option<usize>,
    pub kept: bool,
}

pub fn rank(ledger: &[Vec<u8>], grant: &str) -> (bool, usize) {
    let revokes = ledger.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).any(|e| {
        e.kind == zikaron_glue::book::Kind::Revocation && crate::ledgerx::facts_of(&e).subject.is_some_and(|g| g.eq_ignore_ascii_case(grant.trim()))
    });
    (revokes, ledger.len())
}

fn kept_paths(dir: &std::path::Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(l) => l.filter_map(|x| x.ok().map(|x| x.path())).filter(|p| p.is_file()).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e),
    };
    paths.sort();
    Ok(paths)
}

fn newest_kept(paths: Vec<PathBuf>, want: &str, author: &str, misses: &mut Vec<(Level, Fault)>) -> Option<Supply> {
    let mut best: Option<Supply> = None;
    for p in paths {
        match crate::local::read(&p, crate::local::Doc::KeptGrant) {
            Ok(Some(bytes)) => {
                if let Ok(o) = crate::grantfilex::open_bytes(&bytes) {
                    if o.carries_ledger() {
                        match crate::auditx::ledger_of(&o.ledger, want, author) {
                            Some(items) if best.as_ref().is_none_or(|b| rank(&items, want) > rank(&b.items, want)) => {
                                best = Some(Supply { level: Level::Vault, place: p.display().to_string(), items, rejected: Vec::new(), files: None, kept: true })
                            }
                            Some(_) | None => {}
                        }
                    }
                }
            }
            Ok(None) => {}
            Err(f) => misses.push((Level::Vault, f)),
        }
    }
    best
}

pub fn kept_copy(dir: &std::path::Path, want: &str, author: &str, misses: &mut Vec<(Level, Fault)>) -> Option<Supply> {
    match kept_paths(dir) {
        Ok(paths) => newest_kept(paths, want, author, misses),
        Err(e) => {
            misses.push((Level::Vault, crate::fault::classify(&e, &dir.display().to_string())));
            None
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Found {
    pub supply: Option<Supply>,
    pub misses: Vec<(Level, Fault)>,
}

fn read_at(where_: &str) -> Result<(Vec<Vec<u8>>, Vec<crate::verifyx::Rejected>), Fault> {
    crate::verifyx::bytes_named(where_)
}

fn unread(level: Level, place: &str, bad: &[crate::verifyx::Rejected]) -> Option<(Level, Fault)> {
    let first = bad.first()?;
    Some((level, Fault::known(crate::fault::Known::EntriesUnreadable, format!("{place} · {} · {} · {}", bad.len(), first.file, first.why))))
}

#[derive(Clone, Copy, Debug)]
pub enum Want<'a> {
    Hop(&'a [u8]),
    Author(&'a str),
}

#[derive(Clone, Copy)]
enum WantOf {
    Hop,
    Author,
}

pub fn find_book(shelf: &Shelf, author: &str) -> Found {
    find(shelf, Want::Author(author), 0)
}

fn offer_beside(s: &Supply) {
    zikaron_glue::anchorproofs::offer_beside(std::path::Path::new(&s.place));
}

pub(crate) fn offer_fetched(url: &str) {
    let url = url.to_string();
    zikaron_anchor::ahead::offer_by_hash(move |hash| match crate::fetchx::get_one(&zikaron_glue::anchorproofs::link_of(&url, hash)) {
        Ok(crate::fetchx::One::Bytes(b)) => zikaron_glue::anchorproofs::of_hash(&b, hash, None),
        _ => None,
    });
}

pub fn find(shelf: &Shelf, want: Want, index: usize) -> Found {
    let want_of = match want {
        Want::Hop(_) => WantOf::Hop,
        Want::Author(_) => WantOf::Author,
    };
    let (want, author) = match want {
        Want::Hop(hop) => {
            let Ok(e) = zikaron_glue::book::check(hop) else { return Found::default() };
            (e.id_hex(), e.author.clone())
        }
        Want::Author(a) => (String::new(), a.to_string()),
    };
    let mut found = Found::default();
    let take = |level: Level, place: String, items: Vec<Vec<u8>>, rejected: Vec<crate::verifyx::Rejected>| -> Option<Supply> {
        crate::auditx::ledger_of(&items, &want, &author).map(|items| Supply { level, place, items, rejected, files: None, kept: false })
    };
    let not_this = |level: Level, place: &str| {
        let why = match want_of {
            WantOf::Hop => crate::fault::Known::NotThisLedger,
            WantOf::Author => crate::fault::Known::NotHisBook,
        };
        (level, crate::fault::Fault::known(why, place.to_string()))
    };
    for (addr, home) in &shelf.homes {
        if !addr.eq_ignore_ascii_case(&author) {
            continue;
        }
        let dir = home.join(crate::home::Slot::Ledger.as_str());
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
    let mut held: Option<Supply> = None;
    let newer = |held: &Option<Supply>, s: Supply| -> Supply {
        match held {
            Some(h) if rank(&h.items, &want) >= rank(&s.items, &want) => h.clone(),
            _ => s,
        }
    };
    if let Some(dir) = &shelf.kept {
        let paths = match kept_paths(dir) {
            Ok(p) => p,
            Err(e) => {
                found.misses.push((Level::Vault, crate::fault::classify(&e, &dir.display().to_string())));
                Vec::new()
            }
        };
        if let Some(s) = newest_kept(paths, &want, &author, &mut found.misses) {
            let carried_newer = shelf.carried.as_ref().and_then(|(_, items)| crate::auditx::ledger_of(items, &want, &author)).is_some_and(|c| rank(&c, &want) > rank(&s.items, &want));
            if !carried_newer {
                held = Some(s);
            }
        }
    }
    if let Some((_, at)) = shelf.upstreams.iter().find(|(g, _)| g.eq_ignore_ascii_case(&want)) {
        match crate::home::landing(at).and_then(|_| read_at(at)) {
            Ok((items, bad)) => {
                let named = unread(Level::Vault, at, &bad);
                let read_some = !items.is_empty();
                if let Some(s) = take(Level::Vault, at.clone(), items, bad) {
                    offer_beside(&s);
                    return Found { supply: Some(newer(&held, s)), misses: found.misses };
                }
                if read_some {
                    found.misses.push(not_this(Level::Vault, at));
                }
                found.misses.extend(named);
            }
            Err(f) => found.misses.push((Level::Vault, f)),
        }
    }
    let manual = shelf.manual.get(index).cloned().flatten().filter(|m| !m.trim().is_empty());
    if let Some(m) = manual.as_ref().filter(|m| !crate::fetchx::is_address(m)) {
        match read_at(m) {
            Ok((items, bad)) => {
                let named = unread(Level::Kit, m, &bad);
                let read_some = !items.is_empty();
                if let Some(s) = take(Level::Kit, m.clone(), items, bad) {
                    offer_beside(&s);
                    return Found { supply: Some(newer(&held, s)), misses: found.misses };
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
            return Found { supply: Some(newer(&held, s)), misses: found.misses };
        }
        if !items.is_empty() {
            found.misses.push(not_this(Level::Kit, at));
        }
    }
    if let Some(h) = held {
        return Found { supply: Some(h), misses: found.misses };
    }
    let remote: Vec<String> = manual.into_iter().filter(|m| crate::fetchx::is_address(m)).chain(shelf.pointer.clone()).collect();
    for url in remote {
        match crate::fetchx::base_of(&url).and_then(|b| crate::fetchx::fetch_kit(&b)) {
            Ok(got) => {
                let (items, bad) = crate::verifyx::entries_of_pairs(&got.pairs);
                let named = unread(Level::Remote, &got.url, &bad);
                let read_some = !items.is_empty();
                if let Some(mut s) = take(Level::Remote, got.url.clone(), items, bad) {
                    s.files = Some(got.files);
                    offer_fetched(&got.url);
                    return Found { supply: Some(s), misses: found.misses };
                }
                if read_some {
                    found.misses.push(not_this(Level::Remote, &got.url));
                }
                found.misses.extend(named);
            }
            Err(f) => found.misses.push((Level::Remote, f.at_place(url.trim()))),
        }
    }
    found
}
