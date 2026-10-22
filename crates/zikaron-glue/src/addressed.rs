//! A record package addressed to one key: the format's constants and recognizing its head. The package itself
//! (key agreement, segments, opening) is the sealed layer's (`zikaron_seal::addressed`); this crate holds no
//! cryptography, only what every reader needs to tell an addressed package from a plain one and say so by name.
//!
//! Layout (one file, the record package's `.zkit` extension kept):
//!
//! ```text
//! MAGIC · VERSION (1 byte) · one-time public key (33 bytes, compressed)    -- the head, HEAD bytes
//! segment 0 · segment 1 · …                                                  -- each SEGMENT plaintext bytes
//!                                                                              and a TAG, the last one shorter
//! ```
//!
//! The head names no recipient: the one-time key is fresh for each package and says nothing of whom it is for.
//! The whole head is bound into every segment, so one changed head byte opens nothing. Inside is a plain record
//! package (`crate::container`), byte for byte the one that was sealed.

use std::io::Read;
use std::path::Path;

/// The format mark: the first bytes of an addressed record package.
pub const MAGIC: &[u8] = b"zikaron-kit-addressed\n";
/// The format version written and read.
pub const VERSION: u8 = 1;
/// The one-time public key's length in the head (a compressed curve point).
pub const POINT: usize = 33;
/// The head's length: the mark, the version, the one-time public key.
pub const HEAD: usize = MAGIC.len() + 1 + POINT;
/// Plaintext bytes in each segment but the last (64 KiB).
pub const SEGMENT: usize = 64 << 10;
/// The authentication tag after each segment's ciphertext.
pub const TAG: usize = 16;
/// The key derivation's domain (the information string of the key schedule).
pub const INFO: &str = "zikaron-kit-addressed/1";
/// The default file name of an addressed package: nothing in it comes from what the package holds.
pub const DEFAULT_STEM: &str = "kit";

/// Whether these bytes start an addressed record package (by the mark alone).
pub fn is_addressed(head: &[u8]) -> bool {
    head.starts_with(MAGIC)
}

/// Whether the file at `path` is an addressed record package (by its first bytes; a folder or an unreadable path
/// is not).
pub fn is_addressed_at(path: &Path) -> bool {
    let mut head = [0u8; MAGIC.len()];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let mut got = 0;
    while got < head.len() {
        match f.read(&mut head[got..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => got += n,
        }
    }
    is_addressed(&head[..got])
}

/// A head read apart: its version and its one-time public key as written (whether the key is a curve point is
/// the sealed layer's to judge).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    pub version: u8,
    pub point: [u8; POINT],
}

/// Why a head does not read. Closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BadHead {
    /// The bytes do not start with [`MAGIC`].
    NotAddressed,
    /// The file ends inside the head.
    Short,
    /// A version this build does not read (the version in hand).
    Version(u8),
}

/// Read the head off the first [`HEAD`] bytes.
pub fn head(bytes: &[u8]) -> Result<Head, BadHead> {
    if !is_addressed(bytes) {
        return Err(if MAGIC.starts_with(bytes) && bytes.len() < MAGIC.len() { BadHead::Short } else { BadHead::NotAddressed });
    }
    if bytes.len() < HEAD {
        return Err(BadHead::Short);
    }
    let version = bytes[MAGIC.len()];
    if version != VERSION {
        return Err(BadHead::Version(version));
    }
    let mut point = [0u8; POINT];
    point.copy_from_slice(&bytes[MAGIC.len() + 1..HEAD]);
    Ok(Head { version, point })
}

/// Lay a head out.
pub fn lay_head(point: &[u8; POINT]) -> [u8; HEAD] {
    let mut h = [0u8; HEAD];
    h[..MAGIC.len()].copy_from_slice(MAGIC);
    h[MAGIC.len()] = VERSION;
    h[MAGIC.len() + 1..].copy_from_slice(point);
    h
}

/// The default file name of an addressed package (`kit.zkit`).
pub fn default_name() -> String {
    format!("{DEFAULT_STEM}.{}", crate::container::EXT_KIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_head_lays_out_and_reads_back() {
        let p = [7u8; POINT];
        let h = lay_head(&p);
        assert_eq!(head(&h), Ok(Head { version: VERSION, point: p }));
        assert!(is_addressed(&h));
        assert!(!crate::container::is_container(&h));
    }

    #[test]
    fn a_head_that_is_short_or_of_another_version_is_named() {
        let h = lay_head(&[1u8; POINT]);
        assert_eq!(head(&h[..HEAD - 1]), Err(BadHead::Short));
        assert_eq!(head(&h[..4]), Err(BadHead::Short));
        let mut v = h;
        v[MAGIC.len()] = 2;
        assert_eq!(head(&v), Err(BadHead::Version(2)));
        assert_eq!(head(crate::container::MAGIC.as_bytes()), Err(BadHead::NotAddressed));
    }

    #[test]
    fn the_default_name_takes_nothing_from_the_package() {
        assert_eq!(default_name(), "kit.zkit");
    }
}
