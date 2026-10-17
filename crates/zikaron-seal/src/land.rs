//! Writing a file whole: the product's one temporary shape (`zikaron_store::layout::tmp_file_name`, sixteen hex
//! digits from the system entropy source) and the platform piece's atomic replacement (`zikaron_os::put_at`).

use crate::trouble::Trouble;
use std::path::Path;

/// A fresh temporary name of the product's one shape. Two writers of one file at once get two names; a
/// collision is refused by the owner-only create, never mixed.
pub fn tmp_name() -> Result<String, Trouble> {
    let mut b = [0u8; 8];
    crate::trouble::fill_random(&mut b)?;
    Ok(zikaron_store::layout::tmp_file_name(&zikaron::hexfmt::encode(&b)[2..]))
}

/// Replace `name` in `dir` whole and durably, owner-only (`zikaron_os::put_at`): the old file or the new one,
/// never half of either. A failure is said with the path the platform piece names.
pub fn put_at(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), Trouble> {
    zikaron_os::put_at(dir, name, &tmp_name()?, bytes).map_err(unput)
}

/// A write that did not land, said with its path.
pub fn unput(u: zikaron_os::Unput) -> Trouble {
    Trouble::io(&u.error, &u.at.display().to_string())
}
