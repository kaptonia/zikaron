//! Family literals: the ZIKARON row of the family derivation path table.
//!
//! Path form `m/44'/60'/project'/group/index`. This desk's project number, group number, the two roles'
//! indices and the recovery word count live only in this constant table (one name, one home); the law
//! texts have nothing to do with it.

use crate::roles::Role;

/// BIP-44 purpose.
pub const PURPOSE: u32 = 44;
/// Ethereum coin type.
pub const COIN: u32 = 60;
/// ZIKARON's project number in the family table (account level).
pub const PROJECT: u32 = 0;
/// Group number within the project.
pub const GROUP: u32 = 0;
/// Recorder key index.
pub const AUTHOR_INDEX: u32 = 0;
/// User key index.
pub const GRANTEE_INDEX: u32 = 1;
/// Hardened bit.
pub const HARDENED: u32 = 0x8000_0000;
/// Recovery word count (English word list).
pub const WORDS: usize = 12;
/// Entropy length matching the word count (bytes).
pub const ENTROPY_BYTES: usize = 16;

/// This seat's index in the table.
pub fn index_of(role: Role) -> u32 {
    match role {
        Role::Author => AUTHOR_INDEX,
        Role::Grantee => GRANTEE_INDEX,
    }
}

/// This seat's derivation path (per-level indices, the first three hardened).
pub fn path(role: Role) -> [u32; 5] {
    [PURPOSE | HARDENED, COIN | HARDENED, PROJECT | HARDENED, GROUP, index_of(role)]
}

/// This seat's derivation path as text (the `m/44'/60'/0'/0/0` form).
pub fn path_text(role: Role) -> String {
    format!("m/{PURPOSE}'/{COIN}'/{PROJECT}'/{GROUP}/{}", index_of(role))
}
