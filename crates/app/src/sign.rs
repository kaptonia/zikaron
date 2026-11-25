#[path = "key.rs"]
pub mod key;

use crate::fault::{Fault, Known};
use crate::key::Secret;
use zikaron::cryptox;
use zikaron::entry as k1;
use zikaron::hexfmt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Law {
    Entry(zikaron_glue::book::Law),
    Consent,
}

impl Law {
    pub fn as_str(self) -> &'static str {
        match self {
            Law::Entry(l) => l.domain(),
            Law::Consent => zikaron_glue::face::DOMAIN,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Face {
    Entry,
    Consent,
}

impl Face {
    pub const ALL: [Face; 2] = [Face::Entry, Face::Consent];

    pub fn as_str(self) -> &'static str {
        match self {
            Face::Entry => "entry",
            Face::Consent => "consent",
        }
    }

    pub fn law(self) -> Law {
        match self {
            Face::Entry => Law::Entry(zikaron_glue::book::Law::Two),
            Face::Consent => Law::Consent,
        }
    }

    pub fn domain(self) -> &'static str {
        self.law().as_str()
    }

    pub fn parse(short: &str) -> Option<Face> {
        Face::ALL.into_iter().find(|f| f.as_str() == short)
    }
}

fn seal(s: &Secret, preimage: &[u8], domain: Law) -> Result<String, Fault> {
    let digest = digest_under(preimage, domain);
    let (r, sv, v) = s
        .with_sign_key(|k| cryptox::sign_digest(k, &digest))
        .ok_or_else(|| Fault::known(Known::SignFailed, format!("domain {}", domain.as_str())))?;
    let mut out = Vec::with_capacity(65);
    out.extend_from_slice(&r);
    out.extend_from_slice(&sv);
    out.push(v);
    Ok(hexfmt::encode(&out))
}

fn digest_under(preimage: &[u8], domain: Law) -> [u8; 32] {
    k1::presig_and_digest(preimage, domain.as_str()).1
}

pub fn digest_as(preimage: &[u8], face: Face) -> [u8; 32] {
    digest_under(preimage, face.law())
}

pub fn sign_entry(s: &Secret, preimage: &[u8], law: zikaron_glue::book::Law) -> Result<String, Fault> {
    seal(s, preimage, Law::Entry(law))
}

impl zikaron_send::batch::Signer for Secret {
    fn from(&self) -> Option<[u8; 20]> {
        self.address().map(|a| a.0)
    }

    fn sign(&self, chain: u64, registry: Option<[u8; 20]>, hashes: &[[u8; 32]], input: Option<Vec<u8>>, fees: zikaron_anchor::send::Fees, nonce: u64) -> Result<(Vec<u8>, [u8; 32]), zikaron_anchor::rpc::Trouble> {
        self.with_tx_key(|k| zikaron_anchor::send::sign_at(k, chain, zikaron_anchor::send::Form::Registry, registry, hashes, input, fees, nonce))
    }
}

impl zikaron_send::one::PlainSigner for Secret {
    fn from(&self) -> Option<[u8; 20]> {
        self.address().map(|a| a.0)
    }

    fn sign_plain(&self, chain: u64, to: [u8; 20], value: u128, data: &[u8], fees: zikaron_anchor::send::Fees, nonce: u64) -> Result<(Vec<u8>, [u8; 32]), zikaron_anchor::rpc::Trouble> {
        self.with_plain_key(|k| zikaron_anchor::send::sign_plain(k, chain, to, value, data, fees, nonce))
    }

    fn seal_at(&self, dir: &std::path::Path) -> Option<zikaron_send::sent::Seal> {
        Some(self.with_plain_key(|k| zikaron_send::sent::Seal::of(k, dir)))
    }
}

pub fn sign_as(s: &Secret, preimage: &[u8], face: Face) -> Result<String, Fault> {
    match face {
        Face::Entry => sign_entry(s, preimage, zikaron_glue::book::Law::Two),
        Face::Consent => Err(Fault::known(Known::SeatDomain, Face::Consent.as_str().to_string())),
    }
}

pub fn sign_consent(s: &Secret, root: &[u8; 20], role: &str) -> Result<zikaron_glue::face::Consent, Fault> {
    let address = s.address().ok_or_else(|| Fault::known(Known::KeyMalformed, String::new()))?.0;
    if !zikaron_glue::face::names::is_label(role) {
        return Err(Fault::known(Known::FaceUnwritable, format!("role {role}")));
    }
    let (r, a) = (hexfmt::encode(root), hexfmt::encode(&address));
    let sig = s
        .with_sign_key(|k| zikaron_glue::face::consent::sign(k, &r, &a, role))
        .ok_or_else(|| Fault::known(Known::SignFailed, format!("domain {}", Law::Consent.as_str())))?;
    Ok(zikaron_glue::face::Consent { root: *root, address, role: role.to_string(), sig })
}

pub fn domains() -> Vec<&'static str> {
    Face::ALL.iter().map(|f| f.domain()).collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Use {
    Sign(Face),
    Anchor,
}

impl Use {
    pub fn all() -> Vec<Use> {
        Face::ALL.iter().map(|f| Use::Sign(*f)).chain([Use::Anchor]).collect()
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Use::Sign(f) => f.as_str(),
            Use::Anchor => "anchor",
        }
    }
}

pub fn seat_may(seat: crate::roles::Role, u: Use) -> bool {
    use crate::roles::Role;
    match (seat, u) {
        (Role::Author, Use::Sign(Face::Entry)) | (Role::Grantee, Use::Sign(Face::Entry)) => true,
        (Role::Author, Use::Sign(Face::Consent)) | (Role::Grantee, Use::Sign(Face::Consent)) => true,
        (Role::Author, Use::Anchor) | (Role::Grantee, Use::Anchor) => true,
    }
}

pub fn seat_for(u: Use) -> Option<crate::roles::Role> {
    let allowed: Vec<crate::roles::Role> = crate::roles::Role::ALL.into_iter().filter(|r| seat_may(*r, u)).collect();
    match allowed.as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}
