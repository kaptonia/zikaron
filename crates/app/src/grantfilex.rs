//! Grant file: a small single-file container, the bundle that makes checking easy.
//!
//! A grant code is to a grant file as a magnet link is to a torrent file. The grant code is still the
//! smallest verifiable unit; the grant file is not signed itself, and everything in it is tied to the grant
//! code and the chain by hashes:
//!
//! - `entries/`: the entry bytes of each hop of the grant chain, optionally with the issuer's ledger (when
//! included, the user's six checks have material at once);
//! - `files/zikaron-grant.txt`: the grant code text (the same text as "copy grant code", the kit crate's
//! `badge::encode`);
//! - `files/terms/<digest>/<kit name>`: the terms document itself (the one kept at signing, `termsx`);
//! - `files/publish.txt`: the publish address pointer (a hint only, may be absent).
//!
//! The shape is the glue crate's single-file bundle (`zikaron_glue::container`, kit law §7.1's enumeration);
//! opened, it is an enumeration handed to the same kit verification (`verify_enumeration`). The grant chain
//! is taken from the grant code inside, and each hop's bytes must be in the manifest's entry table (both
//! places must say the same bytes, otherwise refused by name), so the container cannot hold a chain different
//! from the grant code.

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use std::path::{Path, PathBuf};

/// The grant code text file in the container (under `files/`). One name, one home.
pub const CODE_FILE: &str = "zikaron-grant.txt";
/// The publish address pointer file (under `files/`). One name, one home.
pub const PUBLISH_FILE: &str = "publish.txt";
/// The vault room keeping grant files (under `grants-held/`; mirroring carries the whole vault directory).
pub const KEPT: &str = "files";

/// A grant file's name: `grant-<first ten digits of the id>` (extension given separately). One name, one
/// home.
pub fn stem(id: &str) -> String {
    let head: String = id.trim().trim_start_matches("0x").chars().take(10).collect();
    format!("grant-{head}")
}

/// An opened grant file.
#[derive(Clone, Debug)]
pub struct Opened {
    /// The grant chain, from the root (the segment order in the grant code).
    pub hops: Vec<Vec<u8>>,
    /// Every entry in the container that passes the core (each chain hop and the issuer's ledger).
    pub ledger: Vec<Vec<u8>>,
    /// Terms documents: (in-bundle path, bytes).
    pub terms: Vec<(String, Vec<u8>)>,
    /// Publish address pointer (when present).
    pub publish: Option<String>,
    /// The bundle id (sha256 of the manifest).
    pub kit_id: String,
    /// How many items in the bundle (including the manifest).
    pub files: usize,
}

impl Opened {
    /// Whether it carries ledger entries besides the grant chain's hops.
    pub fn carries_ledger(&self) -> bool {
        self.ledger.len() > self.hops.len()
    }
}

/// Open a grant file's bytes. A wrong single-file bundle shape (magic, order, caps) is `GRANT_FILE`; failed
/// kit verification, a missing grant code, or a chain hop missing from the entry table is `GRANT_FILE_KIT`.
/// Returned only when all pass.
pub fn open_bytes(bytes: &[u8]) -> Result<Opened, Fault> {
    crate::trace::mark(crate::feature::Feature::D6);
    let pairs = zikaron_glue::container::decode(bytes).map_err(|b| Fault::known(Known::GrantFileBad, format!("{}:{}", b.code(), b.subject())))?;
    let kit_id = match zikaron_kit::kitdir::verify_enumeration(&pairs) {
        zikaron_kit::kitdir::KitVerdict::Ok { kit_id, .. } => zikaron::hexfmt::encode(&kit_id),
        zikaron_kit::kitdir::KitVerdict::Fail { verdict, subject } => {
            return Err(Fault::known(Known::GrantFileKit, format!("{}{}", verdict.as_str(), subject.map(|s| format!(":{s}")).unwrap_or_default())))
        }
    };
    let under = |dir: &str, rel: &str| format!("{dir}/{rel}");
    let files_dir = zikaron_glue::names::FILES_DIR;
    let code = pairs
        .iter()
        .find(|(p, _)| *p == under(files_dir, CODE_FILE))
        .map(|(_, b)| b.clone())
        .ok_or_else(|| Fault::known(Known::GrantFileKit, format!("E_GRANT_FILE_CODE:{}", under(files_dir, CODE_FILE))))?;
    let trimmed: Vec<u8> = code.iter().copied().filter(|c| !c.is_ascii_whitespace()).collect();
    let hops = crate::payloadx::decode(&trimmed)?;
    let entries_dir = format!("{}/", zikaron_glue::names::ENTRIES_DIR);
    let ledger: Vec<Vec<u8>> = pairs
        .iter()
        .filter(|(p, _)| p.starts_with(&entries_dir))
        .filter(|(_, b)| zikaron::entry::check(b).is_ok())
        .map(|(_, b)| b.clone())
        .collect();
    for h in &hops {
        if !ledger.iter().any(|b| b == h) {
            let id = zikaron::hexfmt::encode(&zikaron::entry::entry_id(h));
            return Err(Fault::known(Known::GrantFileKit, format!("E_GRANT_FILE_CHAIN:{id}")));
        }
    }
    let terms_dir = format!("{files_dir}/{}/", crate::termsx::ROOM);
    let terms: Vec<(String, Vec<u8>)> = pairs
        .iter()
        .filter(|(p, _)| p.starts_with(&terms_dir))
        .map(|(p, b)| (p[files_dir.len() + 1..].to_string(), b.clone()))
        .collect();
    let publish = pairs
        .iter()
        .find(|(p, _)| *p == under(files_dir, PUBLISH_FILE))
        .map(|(_, b)| String::from_utf8_lossy(b).trim().to_string())
        .filter(|s| !s.is_empty());
    Ok(Opened { hops, ledger, terms, publish, kit_id, files: pairs.len() })
}

/// Open a grant file. Check the size first (over the single-file bundle cap is refused without reading it
/// in).
pub fn open(path: &Path) -> Result<Opened, Fault> {
    let md = std::fs::metadata(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    if md.len() > zikaron_glue::container::MAX_TOTAL {
        return Err(Fault::known(Known::GrantFileBad, format!("E_FILE_OVERSIZE:{}", path.display())));
    }
    let bytes = std::fs::read(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    open_bytes(&bytes)
}

/// Whether this file is a grant file (by its start). Unreadable means not (the reading exit names its own
/// error).
pub fn is_grant_file(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 32];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let n = f.read(&mut head).unwrap_or(0);
    zikaron_glue::container::is_container(&head[..n])
}

/// Build a grant file's bytes. The chain from the root; `ledger` is the accompanying issuer ledger (may be
/// empty); terms documents and publish address may each be absent. Once built it goes to the glue crate's
/// in-memory self-verification (`pack::enumerate`, returned only on KIT_OK).
pub fn build(chain: &[Vec<u8>], ledger: &[Vec<u8>], terms: &[(String, Vec<u8>)], publish: Option<&str>) -> Result<(Vec<u8>, usize), Fault> {
    let code = crate::badgex::payload_text(chain)?;
    let mut entries: Vec<Vec<u8>> = chain.to_vec();
    entries.extend(ledger.iter().cloned());
    let mut files: Vec<(String, Vec<u8>)> = vec![(CODE_FILE.to_string(), code.into_bytes())];
    for (rel, b) in terms {
        if !files.iter().any(|(p, _)| p == rel) {
            files.push((rel.clone(), b.clone()));
        }
    }
    if let Some(u) = publish.map(str::trim).filter(|u| !u.is_empty()) {
        files.push((PUBLISH_FILE.to_string(), u.as_bytes().to_vec()));
    }
    let b = zikaron_glue::pack::Bundle { entries, files, note: "grant-file".to_string(), ..Default::default() };
    let (pairs, landed) = zikaron_glue::pack::enumerate(b).map_err(|t| Fault::landing(t.code(), t.subject()))?;
    Ok((zikaron_glue::container::encode(&pairs), landed.files))
}

/// One export's reading.
#[derive(Clone, Debug)]
pub struct Exported {
    pub path: PathBuf,
    pub chosen: crate::home::Chosen,
    pub hops: usize,
    pub terms: usize,
    pub ledger: bool,
    pub files: usize,
}

/// Export a grant file. The chain is cascaded to the root from the vault and this seat's ledger
/// (`badgex::chain_for`); when this grant is in this seat's ledger, that ledger goes along (the issuer's
/// ledger); each hop with an issuance record in this home and a kept document carries the document; the
/// publish address comes from the settings cell. It lands in the folder the person chose (named by
/// `home::choose`, numbered when the name exists), written only through the glue crate's landing.
pub fn export(home: &Home, id: &str, publish: Option<&str>, folder: &Path) -> Result<Exported, Fault> {
    let pool = crate::badgex::pool(home)?;
    let chain = crate::badgex::chain_for(&pool, id)?;
    let mine: Vec<Vec<u8>> = home.ledger()?.survey()?.items;
    let ledger: Vec<Vec<u8>> = if chain.iter().any(|h| mine.contains(h)) { mine } else { Vec::new() };
    let mut terms: Vec<(String, Vec<u8>)> = Vec::new();
    for h in &chain {
        let hid = zikaron::hexfmt::encode(&zikaron::entry::entry_id(h));
        if let Some(r) = crate::termsx::record(home, &hid)? {
            if let Some(d) = crate::termsx::doc_bytes(home, &r)? {
                terms.push(d);
            }
        }
    }
    let (bytes, files) = build(&chain, &ledger, &terms, publish)?;
    let chosen = crate::home::choose(&crate::home::Kind::File { stem: stem(id), ext: zikaron_glue::container::EXT.to_string() }, folder);
    if let Some(d) = chosen.at.parent() {
        std::fs::create_dir_all(d).map_err(|e| crate::fault::classify(&e, &d.display().to_string()))?;
    }
    zikaron_glue::landing::land_bytes(&chosen.at, &bytes).map_err(|t| Fault::landing(t.code(), t.subject()))?;
    Ok(Exported { path: chosen.at.clone(), chosen, hops: chain.len(), terms: terms.len(), ledger: !ledger.is_empty(), files })
}

/// The path of the copy kept in the vault (named by bundle id, kept once). One name, one home.
pub fn kept_path(home: &Home, kit_id: &str) -> Result<PathBuf, Fault> {
    // Keyed by the names key (`names`): locked, the name does not say the bundle.
    let name = crate::names::key()?.name(crate::names::Logical::Kept(kit_id));
    Ok(home.dir(Slot::GrantsHeld).join(KEPT).join(format!("{name}.{}", zikaron_glue::container::EXT)))
}

/// Keep a copy when storing. When a grant file carries the issuer's ledger, the vault keeps it, and checks
/// and re-checks take material from here (the "vault" level). Not written again when already present.
pub fn keep(home: &Home, bytes: &[u8], kit_id: &str) -> Result<PathBuf, Fault> {
    let at = kept_path(home, kit_id)?;
    if at.exists() {
        return Ok(at);
    }
    if let Some(d) = at.parent() {
        std::fs::create_dir_all(d).map_err(|e| crate::fault::classify(&e, &d.display().to_string()))?;
    }
    // Sealed (`local::Doc::KeptGrant`): the copy kept in the vault is local data.
    crate::local::land(&at, crate::local::Doc::KeptGrant, bytes)?;
    Ok(at)
}


/// Every grant file kept in the vault that passes kit verification (one that fails it is not a grant file of
/// this vault and is left out); a file that cannot be opened (locked, sealed under another key) is refused by
/// name, never left out silently.
pub fn kept(home: &Home) -> Result<Vec<(PathBuf, Opened)>, Fault> {
    let dir = home.dir(Slot::GrantsHeld).join(KEPT);
    let listing = match std::fs::read_dir(&dir) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(crate::fault::classify(&e, &dir.display().to_string())),
    };
    let mut paths: Vec<PathBuf> = listing.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_file()).collect();
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        if let Some(bytes) = crate::local::read(&p, crate::local::Doc::KeptGrant)? {
            if let Ok(o) = open_bytes(&bytes) {
                out.push((p, o));
            }
        }
    }
    Ok(out)
}
