pub use zikaron_seal::names::{is_home_name, Logical, NameKey};

pub fn key() -> Result<NameKey, crate::fault::Fault> {
    crate::keybox::name_key()
}
