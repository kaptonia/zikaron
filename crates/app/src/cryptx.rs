use hmac::{Hmac, Mac};
use sha2::Sha512;

use crate::family::ENTROPY_BYTES;

type HmacSha512 = Hmac<Sha512>;

use zikaron_os::wipe;

pub use zikaron_seal::crypto::{aes128_ctr, derivations, hkdf_sha256, hmac_sha256, scrypt, xchacha_open, xchacha_seal};

pub fn pbkdf2_sha256(password: &[u8], salt: &[u8], c: u32, out: &mut [u8]) -> bool {
    if c == 0 {
        return false;
    }
    zikaron_seal::crypto::derived_once();
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password, salt, c, out);
    true
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PhraseTrouble {
    WordCount(usize),
    NotAPhrase,
}

pub fn phrase_of(entropy: &[u8; ENTROPY_BYTES]) -> String {
    match bip39::Mnemonic::from_entropy_in(bip39::Language::English, entropy) {
        Ok(m) => m.to_string(),
        Err(_) => String::new(),
    }
}

pub fn strangers(words: &[crate::secret::Secret; crate::family::WORDS]) -> [bool; crate::family::WORDS] {
    let list = bip39::Language::English.word_list();
    let mut out = [false; crate::family::WORDS];
    for (i, w) in words.iter().enumerate() {
        let w = w.expose().trim();
        out[i] = !w.is_empty() && !list.iter().any(|x| x.eq_ignore_ascii_case(w));
    }
    out
}

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

pub fn derive(entropy: &[u8; ENTROPY_BYTES], path: &[u32]) -> Option<[u8; 32]> {
    use k256::elliptic_curve::sec1::ToEncodedPoint;
    let mut phrase = phrase_of(entropy);
    if phrase.is_empty() {
        return None;
    }
    let mut seed = [0u8; 64];
    pbkdf2::pbkdf2_hmac::<Sha512>(phrase.as_bytes(), b"mnemonic", 2048, &mut seed);
    zikaron_os::wipe_string(&mut phrase);
    let out = walk(&seed, path, |k| {
        k256::SecretKey::from_slice(k).ok().map(|sk| sk.public_key().to_encoded_point(true).as_bytes().to_vec())
    });
    wipe(&mut seed);
    out
}

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

    #[test]
    fn bip32_vector_one() {
        use k256::elliptic_curve::sec1::ToEncodedPoint;
        let seed: Vec<u8> = (0u8..16).collect();
        let pt = |k: &[u8; 32]| k256::SecretKey::from_slice(k).ok().map(|sk| sk.public_key().to_encoded_point(true).as_bytes().to_vec());
        let h = crate::family::HARDENED;
        assert_eq!(hex(&walk(&seed, &[h], pt).unwrap()), "edb2e14f9ee77d26dd93b4ecede8d16ed408ce149b6cd80b0715a2d911a0afea");
        assert_eq!(hex(&walk(&seed, &[h, 1], pt).unwrap()), "3c6cb8d0f6a264c91ea8b5030fadaa8e538b020f0a387421a12de9319dc93368");
    }

    #[test]
    fn scrypt_rfc_vector() {
        let mut dk = [0u8; 64];
        assert!(scrypt(b"password", b"NaCl", 1024, 8, 16, &mut dk));
        assert_eq!(hex(&dk[..8]), "fdbabe1c9d347200");
        assert!(!scrypt(b"x", b"y", 1000, 8, 1, &mut dk[..32]));
    }

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
