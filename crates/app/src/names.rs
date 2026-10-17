//! On-disk names for anything that could be linked to on-chain data: the sealed layer's (`zikaron_seal::names`),
//! with the vault's current names key taken here.

pub use zikaron_seal::names::{is_home_name, Logical, NameKey};

/// The vault's current names key (`LOCKED` while locked).
pub fn key() -> Result<NameKey, crate::fault::Fault> {
    crate::keybox::name_key()
}
