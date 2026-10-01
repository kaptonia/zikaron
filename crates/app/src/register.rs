//! The identity register on disk: the one file in the machine directory that lists this machine's
//! identities, their seats and homes, sealed like every other piece of local data. Reading and writing it is
//! the machine directory's business (the archive); what the rows mean, and every key they name, is the key store's,
//! which never touches local data itself.

use crate::fault::{Fault, Known};
use crate::identity::Registry;
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
