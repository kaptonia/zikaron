//! The only module of this crate that uses third-party cryptography (closed list: RustCrypto's `scrypt`, `aes`,
//! `ctr`, `hmac`, `sha2`, `chacha20poly1305` in its XChaCha form, `hkdf`). Keccak and SHA-256 hashes come from the
//! core (`zikaron::cryptox`). Phrases and private-key derivation (BIP-39, secp256k1) are not here: they stay
//! with the desktop.
//!
//! 1. [`scrypt`] and [`aes128_ctr`]: the two primitives of keystore V3, which the key vault's seals use.
//! 2. [`hkdf_sha256`] and [`hmac_sha256`]: the local data key and the names key derived from the master key, the
//!    keyed place half and the names on disk.
//! 3. [`xchacha_seal`] and [`xchacha_open`]: the one authenticated cipher of every sealed file (24-byte nonce,
//!    additional data bound in).

thread_local! {
    /// How many password derivations this thread has run (here and in the desktop's own PBKDF2). Observation
    /// only: tests read it to confirm a refusal came before the expensive work.
    static DERIVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The derivations this thread has run so far.
pub fn derivations() -> usize {
    DERIVATIONS.with(|d| d.get())
}

/// Count one derivation on this thread (every password derivation of the product calls it).
pub fn derived_once() {
    DERIVATIONS.with(|d| d.set(d.get() + 1));
}

/// scrypt (RFC 7914). `n` must be a power of two; invalid parameters return false, never panic.
pub fn scrypt(password: &[u8], salt: &[u8], n: usize, r: usize, p: usize, out: &mut [u8]) -> bool {
    if n < 2 || n & (n - 1) != 0 || r == 0 || p == 0 || r > u32::MAX as usize || p > u32::MAX as usize {
        return false;
    }
    derived_once();
    let Ok(params) = scrypt::Params::new(n.trailing_zeros() as u8, r as u32, p as u32, out.len()) else {
        return false;
    };
    scrypt::scrypt(password, salt, &params, out).is_ok()
}

/// AES-128-CTR with the whole 16-byte block as a big-endian counter, as keystore V3 implementations use it.
/// Encrypts or decrypts in place.
pub fn aes128_ctr(key: &[u8; 16], iv: &[u8; 16], buf: &mut [u8]) {
    use ctr::cipher::{KeyIvInit, StreamCipher};
    let mut c = ctr::Ctr128BE::<aes::Aes128>::new(key.into(), iv.into());
    c.apply_keystream(buf);
}

/// HKDF-SHA256 (RFC 5869) with no salt: `ikm` expanded under `info` into `out`. False only when `out` is longer
/// than HKDF allows (never for the 32 bytes asked here).
pub fn hkdf_sha256(ikm: &[u8], info: &[u8], out: &mut [u8]) -> bool {
    hkdf::Hkdf::<sha2::Sha256>::new(None, ikm).expand(info, out).is_ok()
}

/// HMAC-SHA256 of `msg` under `key`.
pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    use hmac::Mac;
    let mut m = <hmac::Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(key).expect("HMAC takes a key of any length");
    m.update(msg);
    m.finalize().into_bytes().into()
}

/// XChaCha20-Poly1305 sealing: `plain` under `key` and a 24-byte `nonce`, with `aad` bound in. The result is the
/// ciphertext followed by the 16-byte tag.
pub fn xchacha_seal(key: &[u8; 32], nonce: &[u8; 24], aad: &[u8], plain: &[u8]) -> Option<Vec<u8>> {
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    let c = chacha20poly1305::XChaCha20Poly1305::new(key.into());
    c.encrypt(nonce.into(), Payload { msg: plain, aad }).ok()
}

/// XChaCha20-Poly1305 opening. `None` when the key, the nonce, the additional data or one byte of the
/// ciphertext differs: nothing is decrypted without the tag passing first.
pub fn xchacha_open(key: &[u8; 32], nonce: &[u8; 24], aad: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    let c = chacha20poly1305::XChaCha20Poly1305::new(key.into());
    c.decrypt(nonce.into(), Payload { msg: sealed, aad }).ok()
}
