//! The only module of this crate that uses third-party cryptography (closed list: RustCrypto's `scrypt`, `aes`,
//! `ctr`, `hmac`, `sha2`, `chacha20poly1305` in its XChaCha form, `hkdf`, `k256` for its curve arithmetic and
//! Diffie-Hellman only). Keccak and SHA-256 hashes come from the
//! core (`zikaron::cryptox`). Phrases and private-key derivation (BIP-39, secp256k1) are not here: they stay
//! with the desktop.
//!
//! 1. [`scrypt`] and [`aes128_ctr`]: the two primitives of keystore V3, which the key vault's seals use.
//! 2. [`hkdf_sha256`] and [`hmac_sha256`]: the local data key and the names key derived from the master key, the
//!    keyed place half and the names on disk.
//! 3. [`xchacha_seal`] and [`xchacha_open`]: the one authenticated cipher of every sealed file (24-byte nonce,
//!    additional data bound in).
//! 4. [`point`], [`public_of`], [`fresh_secret`], [`agree`] and [`hkdf_sha256_salted`]: the key agreement of a
//!    record package addressed to one key (`addressed`). Agreeing never signs anything.

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

/// HKDF-SHA256 (RFC 5869) with a salt: `ikm` extracted under `salt`, expanded under `info` into `out`. False only
/// when `out` is longer than HKDF allows.
pub fn hkdf_sha256_salted(salt: &[u8], ikm: &[u8], info: &[u8], out: &mut [u8]) -> bool {
    hkdf::Hkdf::<sha2::Sha256>::new(Some(salt), ikm).expand(info, out).is_ok()
}

/// A compressed secp256k1 point's length.
pub const POINT: usize = 33;

/// A public key as given, judged a point on the curve: 33 bytes compressed (`02`/`03` first) or 65 bytes
/// uncompressed (`04` first), not the point at infinity. Returned compressed; anything else is `None`.
pub fn point(bytes: &[u8]) -> Option<[u8; POINT]> {
    let tag_ok = matches!((bytes.len(), bytes.first()), (33, Some(0x02 | 0x03)) | (65, Some(0x04)));
    if !tag_ok {
        return None;
    }
    let pk = k256::PublicKey::from_sec1_bytes(bytes).ok()?;
    compressed(&pk)
}

fn compressed(pk: &k256::PublicKey) -> Option<[u8; POINT]> {
    use k256::elliptic_curve::sec1::ToEncodedPoint;
    pk.to_encoded_point(true).as_bytes().try_into().ok()
}

/// The compressed public key of a private key (`None` when the bytes are not a private key of the curve).
pub fn public_of(secret: &[u8; 32]) -> Option<[u8; POINT]> {
    let sk = k256::SecretKey::from_slice(secret).ok()?;
    compressed(&sk.public_key())
}

/// The uncompressed public key (65 bytes, `04` first) of a compressed one that is a point.
pub fn uncompressed(point: &[u8; POINT]) -> Option<[u8; 65]> {
    use k256::elliptic_curve::sec1::ToEncodedPoint;
    let pk = k256::PublicKey::from_sec1_bytes(point).ok()?;
    pk.to_encoded_point(false).as_bytes().try_into().ok()
}

/// A fresh private key from the system entropy source (drawn again in the vanishing case it is not one).
pub fn fresh_secret() -> Result<[u8; 32], crate::Trouble> {
    loop {
        let mut b = [0u8; 32];
        crate::trouble::fill_random(&mut b)?;
        if k256::SecretKey::from_slice(&b).is_ok() {
            return Ok(b);
        }
        zikaron_os::wipe(&mut b);
    }
}

/// Diffie-Hellman on secp256k1: the x coordinate of `secret` times `point` (a compressed point that passed
/// [`point`]). `None` when either is not of the curve.
pub fn agree(secret: &[u8; 32], point: &[u8; POINT]) -> Option<[u8; 32]> {
    let sk = k256::SecretKey::from_slice(secret).ok()?;
    let pk = k256::PublicKey::from_sec1_bytes(point).ok()?;
    let shared = k256::ecdh::diffie_hellman(sk.to_nonzero_scalar(), pk.as_affine());
    let mut x = [0u8; 32];
    x.copy_from_slice(shared.raw_secret_bytes());
    Some(x)
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
