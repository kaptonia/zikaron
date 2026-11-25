use crate::fault::{Fault, Known};
use std::path::{Path, PathBuf};

pub use zikaron_seal::records::{Row, DIR, FILE, FORM};

pub fn path_in(machine: &Path) -> PathBuf {
    machine.join(DIR).join(FILE)
}

pub fn read(machine: &Path) -> Result<Vec<Row>, Fault> {
    let p = path_in(machine);
    let Some(bytes) = crate::local::read(&p, crate::local::Doc::Records)? else { return Ok(Vec::new()) };
    zikaron_seal::records::parse(&bytes).map_err(|w| Fault::known(Known::SettingsShape, format!("{}: {w}", p.display())))
}

pub fn add(machine: &Path, row: Row) -> Result<(), Fault> {
    let mut rows = read(machine)?;
    rows.retain(|r| r.id != row.id);
    rows.push(row);
    crate::local::put(&machine.join(DIR), FILE, crate::local::Doc::Records, &zikaron_seal::records::render(&rows))
}

