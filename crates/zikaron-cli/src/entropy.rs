//! Entropy, from one source.
//!
//! With no third-party crates, randomness comes from the system: `/dev/urandom` on Unix-like systems, read
//! for exactly 32 bytes (the source never ends, so reading to end would hang).
//!
//! When it cannot be read, that is said: a private key made from a degraded source looks like a key and
//! protects nothing, so this module has no second source.

use std::io::Read;

/// Path of the system entropy source, written once.
const WELL: &str = "/dev/urandom";

/// Thirty-two random bytes; `None` when unavailable (the caller reports `E_RANDOM`).
pub fn bytes32() -> Option<[u8; 32]> {
    crate::seam();
    let mut f = std::fs::File::open(WELL).ok()?;
    let mut buf = [0u8; 32];
    f.read_exact(&mut buf).ok()?;
    Some(buf)
}
