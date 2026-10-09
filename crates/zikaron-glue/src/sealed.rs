//! Magic prefix of ZIKARON Desk's sealed local data: every local file the app keeps (ledger entries included)
//! starts with one of these lines, then its envelope (the app's `local` module seals and opens it; only the app
//! holds the key). Shared with the CLI so it can tell a sealed file from a plain one without knowing more.
//!
//! Both envelope versions are read; only the second is written. The first ([`MAGIC`]) binds a file's kind and
//! format version; the second ([`MAGIC_V2`]) also binds which file it is (its owner and location; see
//! `app::local`).

/// Prefix of a first-version sealed file (read for compatibility, never written).
pub const MAGIC: &[u8] = b"zikaron-local/1\n";

/// Prefix of a second-version sealed file (the version written).
pub const MAGIC_V2: &[u8] = b"zikaron-local/2\n";

/// Whether bytes are a sealed local file (either envelope).
pub fn is_sealed(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC) || bytes.starts_with(MAGIC_V2)
}
