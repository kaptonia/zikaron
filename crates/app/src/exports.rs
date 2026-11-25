use crate::fault::{Fault, Known};
use std::path::{Path, PathBuf};

pub use zikaron_seal::exports::{Item, Kind, Plan, Row, Why, FILE};

fn path_in(machine: &Path) -> PathBuf {
    machine.join(crate::kitsindex::DIR).join(FILE)
}

pub const YES: &str = "yes";

pub fn read(machine: &Path) -> Result<Vec<Row>, Fault> {
    let p = path_in(machine);
    let Some(bytes) = crate::local::read(&p, crate::local::Doc::Exports)? else { return Ok(Vec::new()) };
    zikaron_seal::exports::parse(&bytes).map_err(|w| Fault::known(Known::SettingsShape, format!("{}: {w}", p.display())))
}

fn write(machine: &Path, rows: &[Row]) -> Result<(), Fault> {
    crate::local::put(&machine.join(crate::kitsindex::DIR), FILE, crate::local::Doc::Exports, &zikaron_seal::exports::render(rows))
}

pub fn record_plain(path: &Path, kind: Kind) -> Result<(), Fault> {
    let machine = crate::home::machine_dir()?;
    let at = crate::home::plain_path(path.to_path_buf());
    let rows = zikaron_seal::exports::listed(&read(&machine)?, kind, &at, crate::lastread::now_secs()).ok_or_else(|| Fault::known(Known::FileMissing, path.display().to_string()))?;
    write(&machine, &rows)
}

fn index_bytes(machine: &Path) -> Result<Option<Vec<u8>>, Fault> {
    Ok(zikaron_seal::exports::index_at(machine)?)
}

pub fn plan(machine: &Path) -> Result<Plan, Fault> {
    let index = index_bytes(machine)?.map(|b| zikaron_seal::exports::index_rows(&b)).transpose()?.unwrap_or_default();
    Ok(zikaron_seal::exports::plan(&index, &read(machine)?))
}

pub fn clean(machine: &Path) -> Result<Plan, Fault> {
    let done = zikaron_seal::exports::remove(plan(machine)?);
    let gone = done.removed_paths();
    if let Some(b) = index_bytes(machine)? {
        if let Some(next) = zikaron_seal::exports::index_without(&b, &gone) {
            crate::home::put_at(&machine.join(crate::kitsindex::DIR), crate::kitsindex::FILE, &next)?;
        }
    }
    let rows: Vec<Row> = read(machine)?.into_iter().filter(|r| !gone.contains(&r.path)).collect();
    write(machine, &rows)?;
    Ok(done)
}
