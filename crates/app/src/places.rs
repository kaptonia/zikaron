//! Where this run keeps its keychain entries and machine directory.
//!
//! Two locations are machine plumbing rather than user features: the anchor key's keychain account name, and
//! the directory holding the machine pointer. Tests exercise the full path on the same machine that may hold
//! the user's real key and pointer, and must never touch those.
//!
//! ─── Why not environment variables ───
//!
//! An environment variable could silently swap the key the user signs with. Instead the shipped build has no
//! way to change these: this module holds a set of locations that can be set only once, and only tests set
//! it; the window binary never calls `set` (enforced by the self-check suite), so it always uses
//! `Places::product()`.
//!
//! The home location (`ZIKARON_DESK_HOME`) is not part of this: choosing the data directory is a user feature,
//! with the real path shown in the UI.

use std::path::PathBuf;
use std::sync::OnceLock;

/// The keychain account base the app uses (also the slot name of the original anchor key).
pub const ACCOUNT: &str = "anchor";

/// This run's locations.
#[derive(Clone, Debug)]
pub struct Places {
    /// The anchor key's account name in the keychain.
    pub key_account: String,
    /// The machine directory (`None`: resolved by [`crate::home::machine_dir`]'s usual lookup).
    pub machine_dir: Option<PathBuf>,
    /// A stand-in for the user's home directory, from which the pointer `~/.zikaron-desk`, the default machine
    /// directory and the old location are built. `None` with no machine directory set means `$HOME` (the
    /// shipped build); with a machine directory set but this unset, the pointer files are neither read nor
    /// written, so a test never touches the real `$HOME`.
    pub user_home: Option<PathBuf>,
}

impl Places {
    /// The shipped build's locations.
    pub fn product() -> Places {
        Places { key_account: ACCOUNT.to_string(), machine_dir: None, user_home: None }
    }
}

static PLACES: OnceLock<Places> = OnceLock::new();

/// Set the locations (tests only). Succeeds only once; later calls return false and never replace them.
pub fn set(p: Places) -> bool {
    PLACES.set(p).is_ok()
}

/// Provisional locations: a test driver can set a test account here at startup, used whenever no explicit
/// [`set`] was made (an explicit `set` overrides it). The shipped build never sets it.
static PROVISIONAL: OnceLock<Places> = OnceLock::new();

/// Set provisional locations (only once; an explicit `set` overrides them).
pub fn set_provisional(p: Places) -> bool {
    PROVISIONAL.set(p).is_ok()
}

/// This run's locations: the explicitly set ones, else the provisional ones, else `Places::product()`.
pub fn get() -> &'static Places {
    if let Some(p) = PLACES.get() {
        return p;
    }
    if let Some(p) = PROVISIONAL.get() {
        return p;
    }
    PLACES.get_or_init(Places::product)
}

/// This run's account base. In the shipped build it is [`ACCOUNT`] (the slot of the original random anchor
/// key, never moved, deleted or renamed); a test driver sets its own prefix, which every derived slot name
/// carries, so test cleanup can find them.
pub fn key_account() -> &'static str {
    &get().key_account
}

// ───────────────────────── One key, one slot: three forms of slot name ─────────────────────────
//
// Identity keys and seeds live only in the keychain, one key per slot, with the address in the account name.
// Slot names take exactly three forms:
//
// 1. Original slot: the account base itself (holds the original random anchor key; when adopted as an
// "existing identity", both roles use it);
// 2. Key slot: `<base>-<address>` (one per derived role key and per imported private key);
// 3. Seed slot: `<base>-seed-<identity>` (the 16 bytes of recovery-word entropy; the identity is named by the
// author key's address).
//
// In the shipped build the base is always `anchor`; a test driver's base carries its own test prefix.

/// A key's slot name.
pub fn key_slot(address: &crate::key::Address) -> String {
    format!("{}-{}", key_account(), address.hex())
}

/// An identity's seed slot name.
pub fn seed_slot(identity: &str) -> String {
    format!("{}-seed-{}", key_account(), identity)
}

/// The prefix of every derived slot name (used for sweeping and counting; the base slot itself is counted
/// separately).
pub fn family_prefix() -> String {
    format!("{}-", key_account())
}

/// The key vault's file name (in the machine directory; every key in it is encrypted). Named by account
/// base, so a test vault and the real one never collide.
pub fn keybox_file() -> String {
    keybox_file_for(key_account())
}

/// The key vault's file name under a given account base.
pub fn keybox_file_for(account: &str) -> String {
    format!("keys-{account}.json")
}

/// The account base a key vault file belongs to, read back from its name (`None`: not a key vault file).
pub fn keybox_file_account(name: &str) -> Option<&str> {
    name.strip_prefix("keys-")?.strip_suffix(".json")
}

/// The key vault's lock file name (next to the vault file).
///
/// A separate file rather than a lock on the vault itself: the vault is replaced on every change (written
/// aside, then renamed), so two processes could each lock a different, stale inode and exclude nothing. This
/// file exists only to be locked; its content is never read.
pub fn keybox_lock_file() -> String {
    format!("keys-{}.lock", key_account())
}

/// The identity register's file name (in the machine directory; no key material). Named by account base, so
/// a test register and the real one never collide.
pub fn registry_file() -> String {
    registry_file_for(key_account())
}

/// The identity register's file name under a given account base.
pub fn registry_file_for(account: &str) -> String {
    format!("identities-{account}.json")
}

/// The account base an identity register file belongs to, read back from its name (`None`: not one).
pub fn registry_file_account(name: &str) -> Option<&str> {
    name.strip_prefix("identities-")?.strip_suffix(".json")
}
