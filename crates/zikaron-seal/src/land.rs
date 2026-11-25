use crate::trouble::Trouble;
use std::path::Path;

pub fn tmp_name() -> Result<String, Trouble> {
    let mut b = [0u8; 8];
    crate::trouble::fill_random(&mut b)?;
    Ok(zikaron_store::layout::tmp_file_name(&zikaron::hexfmt::encode(&b)[2..]))
}

pub fn put_at(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), Trouble> {
    zikaron_os::put_at(dir, name, &tmp_name()?, bytes).map_err(unput)
}

pub fn unput(u: zikaron_os::Unput) -> Trouble {
    Trouble::io(&u.error, &u.at.display().to_string())
}
