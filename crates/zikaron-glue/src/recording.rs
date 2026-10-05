//! The record convention: how a `history` entry's `content` and `mode` are written for bytes (law §6.2 asks
//! for both and leaves their meaning to the writer). One home for the app's anchoring desk and the command
//! line's `history --file`: the same file gives the same two members either way.
//!
//! `mark` is always [`FAMILY`]; `toolchain` is the sha256 of that literal's UTF-8 bytes. The toolchain names the
//! convention (which bytes are hashed, by which algorithm), not the program that hashed them.

use zikaron::cryptox;

/// The family literal: every `content` written under it is the sha256 of some bytes (a file's, a manifest's,
/// a commit object's).
pub const FAMILY: &str = "bytes-sha256/1";

/// The mode's `toolchain`: the sha256 of [`FAMILY`]'s UTF-8 bytes.
pub fn toolchain() -> [u8; 32] {
    cryptox::sha256(FAMILY.as_bytes())
}

/// A file's `content` under this convention: the sha256 of its bytes.
pub fn content_of(bytes: &[u8]) -> [u8; 32] {
    cryptox::sha256(bytes)
}
