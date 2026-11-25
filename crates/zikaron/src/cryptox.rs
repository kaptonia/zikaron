use k256::ecdsa::{RecoveryId, Signature, SigningKey, VerifyingKey};
use sha2::{Digest as _, Sha256};
use sha3::Keccak256;

pub const N: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
    0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b, 0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41,
];

pub const HALF_N: [u8; 32] = [
    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d, 0xdf, 0xe9, 0x2f, 0x46, 0x68, 0x1b, 0x20, 0xa0,
];

pub fn sha256(b: &[u8]) -> [u8; 32] {
    let out = Sha256::digest(b);
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

pub fn keccak256(b: &[u8]) -> [u8; 32] {
    let out = Keccak256::digest(b);
    let mut r = [0u8; 32];
    r.copy_from_slice(&out);
    r
}

pub fn in_range(x: &[u8; 32]) -> bool {
    x.iter().any(|&b| b != 0) && x[..] < N[..]
}

pub fn is_low_s(s: &[u8; 32]) -> bool {
    s[..] <= HALF_N[..]
}

fn address_of_key(vk: &VerifyingKey) -> [u8; 20] {
    let pt = vk.to_encoded_point(false);
    let mut key = [0u8; 65];
    key.copy_from_slice(pt.as_bytes());
    address_of_pubkey(&key)
}

pub fn address_of_pubkey(key: &[u8; 65]) -> [u8; 20] {
    let h = keccak256(&key[1..]);
    let mut a = [0u8; 20];
    a.copy_from_slice(&h[12..]);
    a
}

pub fn recover_pubkey(digest: &[u8; 32], r: &[u8; 32], s: &[u8; 32], recid: u8) -> Option<[u8; 65]> {
    let sig = Signature::from_scalars(*r, *s).ok()?;
    let rid = RecoveryId::from_byte(recid)?;
    let vk = VerifyingKey::recover_from_prehash(digest, &sig, rid).ok()?;
    vk.to_encoded_point(false).as_bytes().try_into().ok()
}

pub fn recover_address(digest: &[u8; 32], r: &[u8; 32], s: &[u8; 32], recid: u8) -> Option<[u8; 20]> {
    recover_pubkey(digest, r, s, recid).map(|k| address_of_pubkey(&k))
}

pub fn sign_digest(privkey: &[u8; 32], digest: &[u8; 32]) -> Option<([u8; 32], [u8; 32], u8)> {
    let sk = SigningKey::from_slice(privkey).ok()?;
    let (sig, rid): (Signature, RecoveryId) = sk.sign_prehash_recoverable(digest).ok()?;
    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&sig.r().to_bytes());
    s.copy_from_slice(&sig.s().to_bytes());
    Some((r, s, 27 + rid.to_byte()))
}

pub fn address_of_privkey(privkey: &[u8; 32]) -> Option<[u8; 20]> {
    let sk = SigningKey::from_slice(privkey).ok()?;
    Some(address_of_key(sk.verifying_key()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_recovered_key_is_the_signers_and_its_address_the_recovered_address() {
        let key = [0x2a; 32];
        let digest = keccak256(b"a transaction's signing payload");
        let (r, s, v) = sign_digest(&key, &digest).expect("signed");
        let recid = v - 27;
        let pubkey = recover_pubkey(&digest, &r, &s, recid).expect("recovered");
        assert_eq!(pubkey[0], 4);
        assert_eq!(Some(address_of_pubkey(&pubkey)), address_of_privkey(&key));
        assert_eq!(recover_address(&digest, &r, &s, recid), address_of_privkey(&key));
        let other = recover_pubkey(&keccak256(b"another payload"), &r, &s, recid);
        assert_ne!(other.map(|k| address_of_pubkey(&k)), address_of_privkey(&key));
    }
}
