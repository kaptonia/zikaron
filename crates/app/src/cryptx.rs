//! The only app module allowed to use third-party cryptography.
//!
//! Allowed crates (closed list): RustCrypto's `scrypt`, `aes`, `ctr`, `hmac`, `pbkdf2`, `sha2`, `k256`,
//! `chacha20poly1305` (XChaCha variant) and `hkdf`, plus `bip39` (English word list and checksum only).
//! No other module imports them; a test enforces this for `bip39`. TLS lives in `zikaron_net`, and the app
//! reaches https nodes and remote files only through it (`chainx::Https`).
//!
//! Provided here:
//!
//! 1. [`scrypt`] and [`aes128_ctr`]: the two primitives of keystore V3 (Web3 Secret Storage).
//! 2. [`phrase_of`] and [`entropy_of`]: 16 bytes of entropy to 12 words and back (BIP-39).
//! 3. [`derive`]: a private key from entropy (BIP-39 seed plus BIP-32 derivation). The seed never leaves
//!    this module: it exists only within one `derive` call and is zeroed on the way out.
//! 4. [`hkdf_sha256`], [`xchacha_seal`] and [`xchacha_open`]: the local data key derived from the master key,
//!    and the one authenticated cipher that seals local data and the backup body (24-byte nonce, additional
//!    data bound in).
//!
//! BIP-32 is computed here from `hmac` (HMAC-SHA512) and `k256` scalar arithmetic, without the `bip32` crate:
//! master key `I = HMAC-SHA512("Bitcoin seed", seed)`; a hardened child's input is `0x00 ‖ k ‖ ser32(i)`, a
//! normal child's is `serP(point(k)) ‖ ser32(i)`; `k_i = parse256(I_L) + k mod n`. An `I_L` outside the order
//! or a zero child returns `None` instead of skipping to the next index: the family's path table has fixed
//! indices, and skipping would change the key. The seed is `PBKDF2-HMAC-SHA512(phrase, "mnemonic", 2048)`
//! with an always-empty passphrase (no 25th word).

use hmac::{Hmac, Mac};
use sha2::Sha512;

use crate::family::ENTROPY_BYTES;

type HmacSha512 = Hmac<Sha512>;

/// Zeroes a byte range with volatile writes, so the compiler cannot drop them as dead stores.
fn wipe(b: &mut [u8]) {
    for x in b.iter_mut() {
        unsafe { std::ptr::write_volatile(x, 0) };
    }
}

thread_local! {
    /// How many password derivations ([`scrypt`], [`pbkdf2_sha256`]) this thread has run. Observation only:
    /// tests read it to confirm a refusal came before the expensive work. Per thread, so work on other
    /// threads is not counted.
    static DERIVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// The derivations this thread has run so far ([`DERIVATIONS`]).
pub fn derivations() -> usize {
    DERIVATIONS.with(|d| d.get())
}

fn derived_once() {
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

/// PBKDF2 with HMAC-SHA256 (RFC 8018), `c` rounds, into `out`. A zero round count returns false, never panics.
pub fn pbkdf2_sha256(password: &[u8], salt: &[u8], c: u32, out: &mut [u8]) -> bool {
    if c == 0 {
        return false;
    }
    derived_once();
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password, salt, c, out);
    true
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

/// The two ways a phrase cannot be recognized.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PhraseTrouble {
    /// The word count is not the number the family table sets.
    WordCount(usize),
    /// A word is not in the English word list, or the checksum does not match.
    NotAPhrase,
}

/// 16 bytes of entropy read as 12 English words (separated by single spaces).
pub fn phrase_of(entropy: &[u8; ENTROPY_BYTES]) -> String {
    match bip39::Mnemonic::from_entropy_in(bip39::Language::English, entropy) {
        Ok(m) => m.to_string(),
        // Unreachable: 16 bytes is a valid BIP-39 length. Even so, it never yields a false phrase.
        Err(_) => String::new(),
    }
}

/// Which of the twelve cells are not words in the English list; empty cells (not filled yet) are not flagged.
///
/// The UI marks those cells red, so the user sees at once which word is wrong. The checksum is not checked
/// here; it needs all twelve words.
pub fn strangers(words: &[crate::secret::Secret; crate::family::WORDS]) -> [bool; crate::family::WORDS] {
    let list = bip39::Language::English.word_list();
    let mut out = [false; crate::family::WORDS];
    for (i, w) in words.iter().enumerate() {
        // Compared case-insensitively instead of lowercased, so no unzeroed copy of the word is made.
        let w = w.expose().trim();
        out[i] = !w.is_empty() && !list.iter().any(|x| x.eq_ignore_ascii_case(w));
    }
    out
}

/// Read a pasted phrase back into 16 bytes of entropy. Case and extra whitespace are ignored; word count,
/// word list and checksum are each named.
pub fn entropy_of(words: &str) -> Result<[u8; ENTROPY_BYTES], PhraseTrouble> {
    let norm: Vec<String> = words.split_whitespace().map(|w| w.to_ascii_lowercase()).collect();
    if norm.len() != crate::family::WORDS {
        return Err(PhraseTrouble::WordCount(norm.len()));
    }
    let m = bip39::Mnemonic::parse_in_normalized(bip39::Language::English, &norm.join(" "))
        .map_err(|_| PhraseTrouble::NotAPhrase)?;
    let (mut arr, n) = m.to_entropy_array();
    if n != ENTROPY_BYTES {
        wipe(&mut arr);
        return Err(PhraseTrouble::WordCount(norm.len()));
    }
    let mut e = [0u8; ENTROPY_BYTES];
    e.copy_from_slice(&arr[..ENTROPY_BYTES]);
    wipe(&mut arr);
    Ok(e)
}

fn scalar(k: &[u8; 32]) -> Option<k256::Scalar> {
    use k256::elliptic_curve::ff::PrimeField;
    Option::<k256::Scalar>::from(k256::Scalar::from_repr((*k).into()))
}

/// Derive a private key from entropy. `path` is the per-level index with hardened bits (see `family::path`).
/// Returns thirty-two bytes; the caller hands them to `key::Secret::take` at once.
pub fn derive(entropy: &[u8; ENTROPY_BYTES], path: &[u32]) -> Option<[u8; 32]> {
    use k256::elliptic_curve::sec1::ToEncodedPoint;
    let mut phrase = phrase_of(entropy);
    if phrase.is_empty() {
        return None;
    }
    let mut seed = [0u8; 64];
    pbkdf2::pbkdf2_hmac::<Sha512>(phrase.as_bytes(), b"mnemonic", 2048, &mut seed);
    // This copy of the phrase is zeroed once used.
    unsafe { wipe(phrase.as_bytes_mut()) };
    let out = walk(&seed, path, |k| {
        k256::SecretKey::from_slice(k).ok().map(|sk| sk.public_key().to_encoded_point(true).as_bytes().to_vec())
    });
    wipe(&mut seed);
    out
}

/// The BIP-32 part: master key plus child keys level by level. Kept separate so the specification's test
/// vectors can feed a seed directly.
fn walk(seed: &[u8], path: &[u32], point: impl Fn(&[u8; 32]) -> Option<Vec<u8>>) -> Option<[u8; 32]> {
    let mut mac = HmacSha512::new_from_slice(b"Bitcoin seed").ok()?;
    mac.update(seed);
    let mut i: [u8; 64] = mac.finalize().into_bytes().into();
    let mut k = [0u8; 32];
    let mut c = [0u8; 32];
    k.copy_from_slice(&i[..32]);
    c.copy_from_slice(&i[32..]);
    wipe(&mut i);
    let ok = scalar(&k).map(|s| !bool::from(s.is_zero())).unwrap_or(false);
    if !ok {
        wipe(&mut k);
        return None;
    }
    for idx in path {
        let mut mac = HmacSha512::new_from_slice(&c).ok()?;
        if idx & crate::family::HARDENED != 0 {
            mac.update(&[0u8]);
            mac.update(&k);
        } else {
            mac.update(&point(&k)?);
        }
        mac.update(&idx.to_be_bytes());
        let mut i: [u8; 64] = mac.finalize().into_bytes().into();
        let mut il = [0u8; 32];
        il.copy_from_slice(&i[..32]);
        let child = match (scalar(&il), scalar(&k)) {
            (Some(l), Some(par)) => l + par,
            _ => {
                wipe(&mut il);
                wipe(&mut i);
                wipe(&mut k);
                return None;
            }
        };
        wipe(&mut il);
        if bool::from(child.is_zero()) {
            wipe(&mut i);
            wipe(&mut k);
            return None;
        }
        k = child.to_bytes().into();
        c.copy_from_slice(&i[32..]);
        wipe(&mut i);
    }
    wipe(&mut c);
    Some(k)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// BIP-32 test vector 1 (seed 000102…0f): private keys of m/0H and m/0H/1.
    #[test]
    fn bip32_vector_one() {
        use k256::elliptic_curve::sec1::ToEncodedPoint;
        let seed: Vec<u8> = (0u8..16).collect();
        let pt = |k: &[u8; 32]| k256::SecretKey::from_slice(k).ok().map(|sk| sk.public_key().to_encoded_point(true).as_bytes().to_vec());
        let h = crate::family::HARDENED;
        assert_eq!(hex(&walk(&seed, &[h], pt).unwrap()), "edb2e14f9ee77d26dd93b4ecede8d16ed408ce149b6cd80b0715a2d911a0afea");
        assert_eq!(hex(&walk(&seed, &[h, 1], pt).unwrap()), "3c6cb8d0f6a264c91ea8b5030fadaa8e538b020f0a387421a12de9319dc93368");
    }

    /// The first eight bytes of RFC 7914 §12's third vector (password / NaCl / 1024 / 8 / 16).
    #[test]
    fn scrypt_rfc_vector() {
        let mut dk = [0u8; 64];
        assert!(scrypt(b"password", b"NaCl", 1024, 8, 16, &mut dk));
        assert_eq!(hex(&dk[..8]), "fdbabe1c9d347200");
        assert!(!scrypt(b"x", b"y", 1000, 8, 1, &mut dk[..32]));
    }

    /// RFC 5869 test case 3 (SHA-256, zero-length salt and info) for HKDF; for XChaCha20-Poly1305 the
    /// draft-irtf-cfrg-xchacha appendix A.3.1 vector's tag, and one flipped byte of aad refused.
    #[test]
    fn hkdf_and_xchacha_vectors() {
        let mut okm = [0u8; 42];
        assert!(hkdf_sha256(&[0x0b; 22], b"", &mut okm));
        assert_eq!(hex(&okm[..16]), "8da4e775a563c18f715f802a063c5a31");
        let key: [u8; 32] = core::array::from_fn(|i| 0x80 + i as u8);
        let nonce: [u8; 24] = core::array::from_fn(|i| 0x40 + i as u8);
        let aad = [0x50, 0x51, 0x52, 0x53, 0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xc5, 0xc6, 0xc7];
        let msg = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
        let ct = xchacha_seal(&key, &nonce, &aad, msg).unwrap();
        assert_eq!(hex(&ct[ct.len() - 16..]), "c0875924c1c7987947deafd8780acf49");
        assert_eq!(xchacha_open(&key, &nonce, &aad, &ct).unwrap(), msg.to_vec());
        let mut bad = aad;
        bad[0] ^= 1;
        assert!(xchacha_open(&key, &nonce, &bad, &ct).is_none());
    }

    /// All-zero entropy reads as the well-known phrase and back to all zeros; a wrong word count and a wrong
    /// checksum are each named.
    #[test]
    fn phrase_round_trip() {
        let p = phrase_of(&[0u8; ENTROPY_BYTES]);
        assert_eq!(p, "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about");
        assert_eq!(entropy_of(&format!("  {}  ", p.to_uppercase())).unwrap(), [0u8; ENTROPY_BYTES]);
        assert_eq!(entropy_of("abandon about"), Err(PhraseTrouble::WordCount(2)));
        let bad = p.replace("about", "abandon");
        assert_eq!(entropy_of(&bad), Err(PhraseTrouble::NotAPhrase));
    }
}
