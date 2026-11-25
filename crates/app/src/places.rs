use std::path::PathBuf;
use std::sync::OnceLock;

pub const ACCOUNT: &str = "anchor";

#[derive(Clone, Debug)]
pub struct Places {
    pub key_account: String,
    pub machine_dir: Option<PathBuf>,
    pub user_home: Option<PathBuf>,
}

impl Places {
    pub fn product() -> Places {
        Places { key_account: ACCOUNT.to_string(), machine_dir: None, user_home: None }
    }
}

static PLACES: OnceLock<Places> = OnceLock::new();

pub fn set(p: Places) -> bool {
    PLACES.set(p).is_ok()
}

static PROVISIONAL: OnceLock<Places> = OnceLock::new();

pub fn set_provisional(p: Places) -> bool {
    PROVISIONAL.set(p).is_ok()
}

pub fn get() -> &'static Places {
    if let Some(p) = PLACES.get() {
        return p;
    }
    if let Some(p) = PROVISIONAL.get() {
        return p;
    }
    PLACES.get_or_init(Places::product)
}

pub fn key_account() -> &'static str {
    &get().key_account
}

pub fn key_slot(address: &crate::key::Address) -> String {
    zikaron_seal::places::key_slot_for(key_account(), &address.hex())
}

pub fn slot_address(name: &str) -> Option<crate::key::Address> {
    let rest = name.strip_prefix(key_account())?.strip_prefix('-')?;
    crate::key::Address::parse(rest).filter(|a| a.hex() == rest)
}

pub fn seed_slot(identity: &str) -> String {
    format!("{}-seed-{}", key_account(), identity)
}

pub fn seed_next_slot(identity: &str) -> String {
    format!("{}-seed-next-{}", key_account(), identity)
}

pub fn seed_reseal_slot(identity: &str) -> String {
    format!("{}-seed-reseal-{}", key_account(), identity)
}

pub fn family_prefix() -> String {
    format!("{}-", key_account())
}

pub fn keybox_file() -> String {
    keybox_file_for(key_account())
}

pub fn keybox_file_for(account: &str) -> String {
    zikaron_seal::places::keybox_file_for(account)
}

pub fn keybox_file_account(name: &str) -> Option<&str> {
    zikaron_seal::places::keybox_file_account(name)
}

pub fn keybox_lock_file() -> String {
    zikaron_seal::places::keybox_lock_file_for(key_account())
}

pub fn registry_file() -> String {
    registry_file_for(key_account())
}

pub fn registry_file_for(account: &str) -> String {
    zikaron_seal::places::registry_file_for(account)
}

pub fn registry_file_account(name: &str) -> Option<&str> {
    zikaron_seal::places::registry_file_account(name)
}

pub fn machine_file() -> String {
    zikaron_seal::places::machine_file_for(key_account())
}

pub fn credential_file() -> String {
    zikaron_seal::places::credential_file_for(key_account())
}
