//! Entropy, from one source.
//!
//! With no third-party crates, randomness comes from the system's entropy source, read for exactly 32 bytes
//! through the operating-system crate (`zikaron_os::fill_random`).
//!
//! When it cannot be read, that is said: a private key made from a degraded source looks like a key and
//! protects nothing, so this module has no second source.

/// Thirty-two random bytes from the system's entropy source (`zikaron_os`, which knows each system's);
/// `None` when unavailable (the caller reports `E_RANDOM`).
pub fn bytes32() -> Option<[u8; 32]> {
    crate::seam();
    let mut buf = [0u8; 32];
    zikaron_os::fill_random(&mut buf).ok()?;
    Some(buf)
}
