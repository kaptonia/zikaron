//! The identity register on disk: the one file in the machine directory that lists this machine's
//! identities with their seats and homes, sealed like all local data. This module reads and writes it; the
//! key store (`identity`) owns what the rows mean and never touches local data itself.
//!
//! Identity changes are read, applied and written here: `identity`'s add, rename, switch, delete and mark are
//! pure transformations of the table that [`change`] hands them.

use crate::fault::{Fault, Known};
use crate::identity::{Registry, Row};
use crate::roles::Role;
use std::path::PathBuf;

/// Where the register is.
pub fn path() -> Result<PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(crate::places::registry_file()))
}

/// Read the register. A missing file is not an error (a machine with no identity actions yet).
pub fn read() -> Result<Option<Registry>, Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are marked too.
    crate::trace::mark(crate::feature::Feature::H3);
    // Sealed local data (`local::Doc::Registry`): refused with `LOCKED` while locked.
    let p = path()?;
    match crate::local::read(&p, crate::local::Doc::Registry)? {
        Some(b) => Registry::parse(&b)
            .map(Some)
            .map_err(|why| Fault::known(Known::IdentitiesShape, format!("{}: {why}", p.display()))),
        None => Ok(None),
    }
}

/// Write the register (sealed, in the machine directory).
pub fn write(reg: &Registry) -> Result<(), Fault> {
    crate::trace::mark(crate::feature::Feature::H3);
    let machine = crate::home::machine_dir()?;
    crate::local::put(&machine, &crate::places::registry_file(), crate::local::Doc::Registry, &reg.to_bytes())
}

/// The identities on this machine (`identity::view` over the stored register). Without a register, the
/// account-base key appears as the only row.
pub fn view(seat: Role) -> Result<Registry, Fault> {
    crate::identity::view(read()?, seat)
}

/// Which registry row is current (`None` without a registry; never the account-base key).
///
/// Used by the signing-key ownership check, seat switching, opening a home and mirror export. They need a row
/// the person registered on this machine; treating the account-base key as current would let machines without
/// a registry pass those checks (and opening a home would then rewrite the machine pointer).
pub fn now_row_listed() -> Result<Option<(Row, Role)>, Fault> {
    Ok(read()?.and_then(|r| r.now().map(|(row, s)| (row.clone(), s))))
}

/// Which slot signs now (`identity::account_now` over [`view`]).
pub fn account_now() -> Result<Option<String>, Fault> {
    Ok(crate::identity::account_now(&view(Role::Author)?))
}

/// Apply one change to the register as stored on disk. Returns `None` and writes nothing when this machine
/// has no register; otherwise `apply` transforms it and it is written if it changed. Used at boot and when
/// following a moved home, neither of which should record the account-base key's row.
pub fn change_listed<T>(apply: impl FnOnce(&mut Registry) -> Result<T, Fault>) -> Result<Option<T>, Fault> {
    let Some(mut reg) = read()? else { return Ok(None) };
    let before = reg.clone();
    let out = apply(&mut reg)?;
    if reg != before {
        write(&reg)?;
    }
    Ok(Some(out))
}

/// One identity change: read the table ([`view`]), let `apply` transform it (`identity`'s add, rename,
/// switch, delete, mark), and write it if it changed or if this machine had no register yet (the first
/// identity action records the account-base key's row). A refused change writes nothing.
pub fn change<T>(seat: Role, apply: impl FnOnce(&mut Registry) -> Result<T, Fault>) -> Result<T, Fault> {
    let listed = read()?;
    let had = listed.is_some();
    let mut reg = crate::identity::view(listed, seat)?;
    let before = reg.clone();
    let out = apply(&mut reg)?;
    if !had || reg != before {
        write(&reg)?;
    }
    Ok(out)
}
