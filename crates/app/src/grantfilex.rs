//! Grant file: a small single-file container that bundles everything needed to check a grant.
//!
//! A grant code is to a grant file as a magnet link is to a torrent file. The grant code is still the
//! smallest verifiable unit; the grant file itself is not signed, and everything in it is tied to the grant
//! code and the chain by hashes:
//!
//! - `entries/`: the entry bytes of each hop of the grant chain, optionally with the issuer's ledger (so the
//!   grantee's six checks have material at once);
//! - `files/zikaron-grant.txt`: the grant code text (the same text as "copy grant code", the kit crate's
//!   `badge::encode`);
//! - `files/terms/<digest>/<kit name>`: the terms document itself (the one kept at signing, `termsx`);
//! - `files/publish.txt`: the publish address pointer (a hint only, may be absent).
//!
//! The shape is the glue crate's single-file bundle (`zikaron_glue::container`, kit law §7.1's
//! enumeration); opened, it is an enumeration handed to the same kit verification (`verify_enumeration`).
//! The grant chain is taken from the grant code inside, and each hop's bytes must also be in the manifest's
//! entry table (the two must agree, otherwise it is refused by name), so the container cannot hold a chain
//! different from its grant code.

use crate::fault::{Fault, Known};
use crate::home::{Home, Slot};
use std::path::{Path, PathBuf};

/// File names of the grant code and the publish pointer inside the container (under `files/`), shared with
/// the command line (`zikaron_glue::grantfile`).
pub use zikaron_glue::grantfile::{CODE_FILE, PUBLISH_FILE};
/// Vault subdirectory holding kept grant files (under `grants-held/`; mirroring carries the whole vault).
pub const KEPT: &str = "files";

/// A grant file's name stem: `grant-<first ten hex digits of the id>` (the extension is added separately).
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

/// Opens a grant file's bytes with the reader shared with the command line (`zikaron_glue::grantfile::open`).
/// A wrong container shape (magic, order, caps) is `GRANT_FILE`; failed kit verification, a missing grant
/// code, or a chain hop missing from the entry table is `GRANT_FILE_KIT`; a grant code the kit core refuses
/// is `PAYLOAD_REFUSED`. Returned only when all pass.
pub fn open_bytes(bytes: &[u8]) -> Result<Opened, Fault> {
    use zikaron_glue::grantfile::Refused;
    crate::trace::mark(crate::feature::Feature::D6);
    let o = zikaron_glue::grantfile::open(bytes).map_err(|r| match r {
        Refused::Shape(b) => Fault::known(Known::GrantFileBad, format!("{}:{}", b.code(), b.subject())),
        Refused::Kit(verdict, subject) => {
            Fault::known(Known::GrantFileKit, format!("{}{}", verdict.as_str(), subject.map(|s| format!(":{s}")).unwrap_or_default()))
        }
        Refused::NoCode(at) => Fault::known(Known::GrantFileKit, format!("E_GRANT_FILE_CODE:{at}")),
        Refused::Code(r) => crate::payloadx::refused(r),
        Refused::Uncarried(id) => Fault::known(Known::GrantFileKit, format!("E_GRANT_FILE_CHAIN:{}", zikaron::hexfmt::encode(&id))),
    })?;
    Ok(Opened { hops: o.hops, ledger: o.ledger, terms: o.terms, publish: o.publish, kit_id: zikaron::hexfmt::encode(&o.kit_id), files: o.files })
}

/// Opens a grant file. The size is checked first: a file over the single-file bundle cap is refused without
/// reading it in.
pub fn open(path: &Path) -> Result<Opened, Fault> {
    let md = std::fs::metadata(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    if md.len() > zikaron_glue::container::MAX_TOTAL {
        return Err(Fault::known(Known::GrantFileBad, format!("E_FILE_OVERSIZE:{}", path.display())));
    }
    let bytes = std::fs::read(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    open_bytes(&bytes)
}

/// Whether this file is a grant file, judged by its first bytes. Unreadable means no (the caller that reads
/// it reports its own error).
pub fn is_grant_file(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0u8; 32];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let n = f.read(&mut head).unwrap_or(0);
    zikaron_glue::container::is_container(&head[..n])
}

/// Builds a grant file's bytes. `chain` runs from the root; `ledger` is the accompanying issuer ledger (may
/// be empty); terms documents and publish address may each be absent. The result goes through the glue
/// crate's in-memory self-verification (`pack::enumerate`, returned only on KIT_OK).
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
    let (pairs, landed) = zikaron_glue::pack::enumerate(b).map_err(|t| Fault::of_landing(t))?;
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

/// Exports a grant file. The chain is followed to the root through the vault and this seat's ledger
/// (`badgex::chain_for`); when the grant is in this seat's ledger, that ledger (the issuer's) goes along;
/// each hop with an issuance record and a kept terms document here carries the document; the publish address
/// comes from settings. The file is written into the chosen folder (named by `home::choose`, numbered if the
/// name exists) only through the glue crate's landing path. `_pass` is the exit gate's
/// [`crate::exitgate::Pass`]: this effect cannot be reached without the gate.
pub fn export(_pass: &crate::exitgate::Pass, home: &Home, id: &str, publish: Option<&str>, folder: &Path) -> Result<Exported, Fault> {
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
    zikaron_glue::landing::land_bytes(&chosen.at, &bytes).map_err(|t| Fault::of_landing(t))?;
    Ok(Exported { path: chosen.at.clone(), chosen, hops: chain.len(), terms: terms.len(), ledger: !ledger.is_empty(), files })
}

/// Path of the copy kept in the vault (named by bundle id, kept once).
pub fn kept_path(home: &Home, kit_id: &str) -> Result<PathBuf, Fault> {
    // Named through the names key (`names`), so the file name does not reveal the bundle.
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
