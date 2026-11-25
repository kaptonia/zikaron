pub fn keybox_file_for(account: &str) -> String {
    format!("keys-{account}.json")
}

pub fn keybox_file_account(name: &str) -> Option<&str> {
    name.strip_prefix("keys-")?.strip_suffix(".json")
}

pub fn keybox_lock_file_for(account: &str) -> String {
    format!("keys-{account}.lock")
}

pub fn registry_file_for(account: &str) -> String {
    format!("identities-{account}.json")
}

pub fn registry_file_account(name: &str) -> Option<&str> {
    name.strip_prefix("identities-")?.strip_suffix(".json")
}

pub fn key_slot_for(account: &str, address: &str) -> String {
    format!("{account}-{address}")
}

pub const CREDENTIAL_HEAD: &str = "cli-credential-";

pub fn credential_file_for(account: &str) -> String {
    format!("{CREDENTIAL_HEAD}{account}.json")
}

pub fn credential_file_account(name: &str) -> Option<&str> {
    name.strip_prefix(CREDENTIAL_HEAD)?.strip_suffix(".json")
}

pub fn machine_file_for(account: &str) -> String {
    format!("machine-{account}.sealed")
}
