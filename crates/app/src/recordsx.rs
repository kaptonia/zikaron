//! The local signing index and "verify a file".
//!
//! A ledger `history` entry records only the content digest (`content`), not the file name or location.
//! To answer "was this file signed, which entry is it, is it anchored", this machine keeps its own index at
//! `<machine directory>/records/index.json` (homes must not store absolute paths; other apps do not read it).
//! A row holds only what was fixed at signing (ledger, digest, entry id, `seq`, file name, path). Anchor state
//! is not stored, because it would go stale; it is taken from the current audit report.
//!
//! "Verify a file" does not rely on this index: it hashes the file now and searches the ledger for a `history`
//! entry with the same `content`. The index only adds which file was signed.

use crate::fault::{Fault, Known};
use std::path::{Path, PathBuf};

/// The index's form, its place and its row: the sealed layer's (`zikaron_seal::records`), where the command line
/// reads it too.
pub use zikaron_seal::records::{Row, DIR, FILE, FORM};

pub fn path_in(machine: &Path) -> PathBuf {
    machine.join(DIR).join(FILE)
}

/// Read the index. A missing file means no rows; a malformed file is refused.
pub fn read(machine: &Path) -> Result<Vec<Row>, Fault> {
    let p = path_in(machine);
    let Some(bytes) = crate::local::read(&p, crate::local::Doc::Records)? else { return Ok(Vec::new()) };
    zikaron_seal::records::parse(&bytes).map_err(|w| Fault::known(Known::SettingsShape, format!("{}: {w}", p.display())))
}

/// Add a row (replacing an existing row with the same entry id), with an atomic rewrite.
pub fn add(machine: &Path, row: Row) -> Result<(), Fault> {
    let mut rows = read(machine)?;
    rows.retain(|r| r.id != row.id);
    rows.push(row);
    crate::local::put(&machine.join(DIR), FILE, crate::local::Doc::Records, &zikaron_seal::records::render(&rows))
}

