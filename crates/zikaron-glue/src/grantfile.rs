//! Reading a grant file: one reading for the app and the command line.
//!
//! A grant file is a single-file bundle ([`crate::container`]) holding a disclosure kit whose `files/` carry
//! the grant code (`files/zikaron-grant.txt`), the terms documents (`files/terms/…`) and a publish pointer
//! (`files/publish.txt`), and whose `entries/` carry each hop of the grant chain, with the issuer's ledger
//! when included. This module lays the reading out and decides nothing: the bundle shape is this crate's own
//! format; whether the kit stands is the kit core's (`kitdir::verify_enumeration`); what the grant code
//! says is the kit core's (`badge::decode`); whether every hop it names is carried is the kit core's
//! (`badge::uncarried`). Each answer is passed on unchanged.

/// The grant code text file in the bundle (under `files/`). One name, one home.
pub const CODE_FILE: &str = "zikaron-grant.txt";
/// The publish address pointer file (under `files/`). One name, one home.
pub const PUBLISH_FILE: &str = "publish.txt";
/// The terms room (under `files/`, and in a home's kits room). One name, one home.
pub const TERMS_ROOM: &str = "terms";

/// An opened grant file.
#[derive(Clone, Debug)]
pub struct Opened {
    /// The grant chain, from the root (the segment order in the grant code).
    pub hops: Vec<Vec<u8>>,
    /// Every entry in the bundle that passes the core (each chain hop and the issuer's ledger).
    pub ledger: Vec<Vec<u8>>,
    /// Terms documents: (path under `files/`, bytes).
    pub terms: Vec<(String, Vec<u8>)>,
    /// Publish address pointer (when present).
    pub publish: Option<String>,
    /// The bundle id (sha256 of the manifest).
    pub kit_id: [u8; 32],
    /// How many items in the bundle (including the manifest).
    pub files: usize,
}

/// Why a grant file did not open: whose answer it was, unchanged.
#[derive(Debug)]
pub enum Refused {
    /// The single-file bundle shape (this crate's own format): its code and subject.
    Shape(crate::container::Bad),
    /// The kit core's verdict on the kit inside, and its subject.
    Kit(zikaron_kit::tokens::KitFailToken, Option<String>),
    /// No grant code file in the bundle (its path).
    NoCode(String),
    /// The kit core refused the grant code.
    Code(zikaron_kit::badge::DecodeReject),
    /// The kit core found a hop of the grant code that the bundle does not carry (its entry id).
    Uncarried([u8; 32]),
}

/// Open a grant file's bytes.
pub fn open(bytes: &[u8]) -> Result<Opened, Refused> {
    crate::seam_v2();
    let pairs = crate::container::decode(bytes).map_err(Refused::Shape)?;
    let kit_id = match zikaron_kit::kitdir::verify_enumeration(&pairs) {
        zikaron_kit::kitdir::KitVerdict::Ok { kit_id, .. } => kit_id,
        zikaron_kit::kitdir::KitVerdict::Fail { verdict, subject } => return Err(Refused::Kit(verdict, subject)),
    };
    let files_dir = crate::names::FILES_DIR;
    let under = |rel: &str| format!("{files_dir}/{rel}");
    let code = pairs
        .iter()
        .find(|(p, _)| *p == under(CODE_FILE))
        .map(|(_, b)| b.clone())
        .ok_or_else(|| Refused::NoCode(under(CODE_FILE)))?;
    let trimmed: Vec<u8> = code.iter().copied().filter(|c| !c.is_ascii_whitespace()).collect();
    let hops: Vec<Vec<u8>> = zikaron_kit::badge::decode(&trimmed).map_err(Refused::Code)?.into_iter().map(|e| e.bytes).collect();
    let entries_dir = format!("{}/", crate::names::ENTRIES_DIR);
    let ledger: Vec<Vec<u8>> = pairs
        .iter()
        .filter(|(p, _)| p.starts_with(&entries_dir))
        .filter(|(_, b)| zikaron::entry::check(b).is_ok())
        .map(|(_, b)| b.clone())
        .collect();
    if let Some(h) = zikaron_kit::badge::uncarried(&hops, &ledger) {
        return Err(Refused::Uncarried(zikaron::entry::entry_id(h)));
    }
    let terms_dir = format!("{files_dir}/{TERMS_ROOM}/");
    let terms: Vec<(String, Vec<u8>)> = pairs
        .iter()
        .filter(|(p, _)| p.starts_with(&terms_dir))
        .map(|(p, b)| (p[files_dir.len() + 1..].to_string(), b.clone()))
        .collect();
    let publish = pairs
        .iter()
        .find(|(p, _)| *p == under(PUBLISH_FILE))
        .map(|(_, b)| String::from_utf8_lossy(b).trim().to_string())
        .filter(|s| !s.is_empty());
    Ok(Opened { hops, ledger, terms, publish, kit_id, files: pairs.len() })
}
