//! The mark of ZIKARON Desk's sealed local data: every local file the app keeps (ledger entries included)
//! starts with this line, then its kind, version, nonce and ciphertext (the app's `local` module seals and
//! opens; only the app holds the key). One name, one home: the app and the command line both read it here, so
//! the command line can tell a sealed file from a plain one without knowing anything else about it.

/// The first bytes of every sealed local file.
pub const MAGIC: &[u8] = b"zikaron-local/1\n";

/// Whether bytes are a sealed local file.
pub fn is_sealed(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}
