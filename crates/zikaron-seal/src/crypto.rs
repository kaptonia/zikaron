thread_local! {
    static DERIVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub fn derivations() -> usize {
    DERIVATIONS.with(|d| d.get())
}

pub fn derived_once() {
    DERIVATIONS.with(|d| d.set(d.get() + 1));
}

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

pub fn aes128_ctr(key: &[u8; 16], iv: &[u8; 16], buf: &mut [u8]) {
    use ctr::cipher::{KeyIvInit, StreamCipher};
    let mut c = ctr::Ctr128BE::<aes::Aes128>::new(key.into(), iv.into());
    c.apply_keystream(buf);
}

pub fn hkdf_sha256(ikm: &[u8], info: &[u8], out: &mut [u8]) -> bool {
    hkdf::Hkdf::<sha2::Sha256>::new(None, ikm).expand(info, out).is_ok()
}

pub fn hkdf_sha256_salted(salt: &[u8], ikm: &[u8], info: &[u8], out: &mut [u8]) -> bool {
    hkdf::Hkdf::<sha2::Sha256>::new(Some(salt), ikm).expand(info, out).is_ok()
}

pub const POINT: usize = 33;

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

pub fn public_of(secret: &[u8; 32]) -> Option<[u8; POINT]> {
    let sk = k256::SecretKey::from_slice(secret).ok()?;
    compressed(&sk.public_key())
}

pub fn uncompressed(point: &[u8; POINT]) -> Option<[u8; 65]> {
    use k256::elliptic_curve::sec1::ToEncodedPoint;
    let pk = k256::PublicKey::from_sec1_bytes(point).ok()?;
    pk.to_encoded_point(false).as_bytes().try_into().ok()
}

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

pub fn agree(secret: &[u8; 32], point: &[u8; POINT]) -> Option<[u8; 32]> {
    let sk = k256::SecretKey::from_slice(secret).ok()?;
    let pk = k256::PublicKey::from_sec1_bytes(point).ok()?;
    let shared = k256::ecdh::diffie_hellman(sk.to_nonzero_scalar(), pk.as_affine());
    let mut x = [0u8; 32];
    x.copy_from_slice(shared.raw_secret_bytes());
    Some(x)
}

pub fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    use hmac::Mac;
    let mut m = <hmac::Hmac<sha2::Sha256> as hmac::Mac>::new_from_slice(key).expect("HMAC takes a key of any length");
    m.update(msg);
    m.finalize().into_bytes().into()
}

pub fn xchacha_seal(key: &[u8; 32], nonce: &[u8; 24], aad: &[u8], plain: &[u8]) -> Option<Vec<u8>> {
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    let c = chacha20poly1305::XChaCha20Poly1305::new(key.into());
    c.encrypt(nonce.into(), Payload { msg: plain, aad }).ok()
}

pub fn xchacha_open(key: &[u8; 32], nonce: &[u8; 24], aad: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    use chacha20poly1305::aead::{Aead, KeyInit, Payload};
    let c = chacha20poly1305::XChaCha20Poly1305::new(key.into());
    c.decrypt(nonce.into(), Payload { msg: sealed, aad }).ok()
}
