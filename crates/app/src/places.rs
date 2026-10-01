//! Where this run lives.
//!
//! Two things are "where on this machine" rather than product features: the anchor key's account name in the
//! keychain, and the directory holding the machine pointer. Tests need to exercise the whole path on the
//! same machine, and that machine may hold the real key and the real pointer; a test must never break the
//! real ones.
//!
//! ─── Why not environment variables ───
//!
//! One environment variable could quietly swap the key the user signs with, and "nobody set that variable"
//! is only a hope. Instead the shipped build has no path to change it. This file provides a table of places
//! that may be set only once, and only tests set it; the window binary never calls `set` (checked by the
//! self-check suite), so it always runs `Places::product()`.
//!
//! The home location (`ZIKARON_DESK_HOME`) is not in this set: choosing the directory is a product feature,
//! with the real path shown on the face and chosen by the person.

use std::path::PathBuf;
use std::sync::OnceLock;

/// The account base the product uses (also the slot name of the original anchor key). One name, one home.
pub const ACCOUNT: &str = "anchor";

/// This run's places.
#[derive(Clone, Debug)]
pub struct Places {
    /// The anchor key's account name in the keychain.
    pub key_account: String,
    /// Where the machine directory is (`None` means resolved now by the three levels of
    /// [`crate::home::machine_dir`]).
    pub machine_dir: Option<PathBuf>,
    /// A stand-in for the user's home directory: the pointer `~/.zikaron-desk`, the default machine directory
    /// and the old location are all built from it. `None` with no machine directory set means `$HOME` (the
    /// shipped build); with a machine directory set but this unset, the pointer family is neither read nor
    /// written anywhere (the real `$HOME` is never touched when a test sets places).
    pub user_home: Option<PathBuf>,
}

impl Places {
    /// What the shipped build runs.
    pub fn product() -> Places {
        Places { key_account: ACCOUNT.to_string(), machine_dir: None, user_home: None }
    }
}

static PLACES: OnceLock<Places> = OnceLock::new();

/// Set places once. Succeeds only once, and only tests call it. Returns whether this call set them; later
/// calls always return false, never silently replacing them.
pub fn set(p: Places) -> bool {
    PLACES.set(p).is_ok()
}

/// This run's places. When never set, the product's. A provisional set: a test driver can set a test account
/// here as soon as it starts, and every path without explicitly set places lands on it; the explicit `set` is
/// still allowed only once and overrides it. The shipped build never calls this, so it is always `product()`.
static PROVISIONAL: OnceLock<Places> = OnceLock::new();

/// Set provisional places (only once; an explicit `set` overrides it).
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

/// This run's account base. In the shipped build it is [`ACCOUNT`] (the slot of the existing random anchor
/// key, never moved, deleted or renamed); a test driver sets its own prefixed family, and every slot name
/// derived from it carries the same prefix, so test cleanup reaches them.
pub fn key_account() -> &'static str {
    &get().key_account
}

// ───────────────────────── One key, one slot: three forms of slot name ─────────────────────────
//
// Identity keys and seeds live only in the keychain, one key per slot, with the address in the account name.
// Slot names are assembled only in these three places (one name, one home):
//
// 1. Existing slot: the account base itself (where the original random anchor key lives; when adopted as an
// "existing identity", both seats still use it);
// 2. Key slot: `<base>-<address>` (one slot for each derived seat key and each imported private key);
// 3. Seed slot: `<base>-seed-<identity>` (the sixteen bytes of recovery-word entropy; the identity is named
// by the recorder key's address).
//
// In the shipped build the base is always `anchor`, so there is no path to change the prefix; a test
// driver's base always carries its own test prefix.

/// A key's slot name.
pub fn key_slot(address: &crate::key::Address) -> String {
    format!("{}-{}", key_account(), address.hex())
}

/// An identity's seed slot name.
pub fn seed_slot(identity: &str) -> String {
    format!("{}-seed-{}", key_account(), identity)
}

/// Every slot name of this family starts with this (sweeping and counting recognize it; the base itself
/// counts separately).
pub fn family_prefix() -> String {
    format!("{}-", key_account())
}

/// The key vault's file name (in the machine directory; every key in the vault is ciphertext, not one in
/// plaintext). Named by account base: a test vault and the shipped one never see each other, and test
/// cleanup sweeps by directory.
pub fn keybox_file() -> String {
    keybox_file_for(key_account())
}

/// The key vault's file name under a given account base (the one place its spelling is written).
pub fn keybox_file_for(account: &str) -> String {
    format!("keys-{account}.json")
}

/// The account base a key vault file belongs to, read back from its name (`None`: not a key vault file).
pub fn keybox_file_account(name: &str) -> Option<&str> {
    name.strip_prefix("keys-")?.strip_suffix(".json")
}

/// The key vault's file lock name (in the same directory as the vault file).
///
/// A separate file, not a lock on the vault file itself: the vault file is rewritten wholesale on every
/// change (written aside, then renamed), and after a rename that inode is no longer the file on disk, so two
/// processes each locking their own old inode would amount to no lock. This file is only a lock seat; not one
/// byte of its content is read.
pub fn keybox_lock_file() -> String {
    format!("keys-{}.lock", key_account())
}

/// The identity register's file name (in the machine directory; zero key material). Named by account base:
/// a test register and the shipped one never see each other.
pub fn registry_file() -> String {
    registry_file_for(key_account())
}

/// The identity register's file name under a given account base (the one place its spelling is written).
pub fn registry_file_for(account: &str) -> String {
    format!("identities-{account}.json")
}

/// The account base an identity register file belongs to, read back from its name (`None`: not one).
pub fn registry_file_account(name: &str) -> Option<&str> {
    name.strip_prefix("identities-")?.strip_suffix(".json")
}
