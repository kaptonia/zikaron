//! The names of the machine directory's files that belong to one account base (the shipped build has one; test
//! benches use their own, so a test vault never meets the real one). Each end builds them here, and reads an
//! account base back from a name here.

/// The key vault's file name.
pub fn keybox_file_for(account: &str) -> String {
    format!("keys-{account}.json")
}

/// The account base a key vault file belongs to (`None`: not a key vault file).
pub fn keybox_file_account(name: &str) -> Option<&str> {
    name.strip_prefix("keys-")?.strip_suffix(".json")
}

/// The key vault's lock file name (next to the vault file; only ever locked, never read).
pub fn keybox_lock_file_for(account: &str) -> String {
    format!("keys-{account}.lock")
}

/// The identity register's file name.
pub fn registry_file_for(account: &str) -> String {
    format!("identities-{account}.json")
}

/// The account base an identity register file belongs to (`None`: not one).
pub fn registry_file_account(name: &str) -> Option<&str> {
    name.strip_prefix("identities-")?.strip_suffix(".json")
}

/// An identity key's slot name: the account base and the key's address (`0x` and forty lowercase hex).
pub fn key_slot_for(account: &str, address: &str) -> String {
    format!("{account}-{address}")
}

/// The command line's credential file name (`credential`).
pub fn credential_file_for(account: &str) -> String {
    format!("cli-credential-{account}.json")
}

/// The account base a credential file belongs to (`None`: not one). The command line finds the account it
/// writes for this way: the credential file is the one thing it is given.
pub fn credential_file_account(name: &str) -> Option<&str> {
    name.strip_prefix("cli-credential-")?.strip_suffix(".json")
}

/// The sealed machine settings' file name (`machine`).
pub fn machine_file_for(account: &str) -> String {
    format!("machine-{account}.sealed")
}
