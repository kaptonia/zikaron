//! Entry assembly and signing. No byte shape or signature is invented here:
//!
//! - canonical bytes from the core's `json::canon_bytes` (member order, integer spelling, escapes);
//! - preimage from the core's `entry::b6_bytes` (the six members without `sig`, law §5.1);
//! - digest from the core's `entry::presig_and_digest` (law §5.2, §5.3), domain from `tokens::Domain`;
//! - signature from the core's `cryptox::sign_digest` (RFC 6979 low s, law §5.7);
//! - judgment from the core's `entry::check`: before landing, the entry must pass the thirteen steps, or the
//! verb fails with the law's token.
//!
//! This layer only arranges the arguments into a six-member object; a wrong arrangement is refused by the
//! law.

use crate::codes::Field;
use zikaron::entry as k1;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::tokens::{Domain, Token};

/// An entry signed and past the thirteen steps, with its canonical bytes.
pub struct Sealed {
    pub bytes: Vec<u8>,
    pub entry: k1::Entry,
}

/// Build an object from the field table (bodies and windows both).
pub fn shape(members: Vec<(Field, Value)>) -> Value {
    Value::Obj(
        members
            .into_iter()
            .map(|(k, v)| (k.as_str().to_string(), v))
            .collect(),
    )
}

/// The address of a private key (the §5.5 derivation, from the core).
pub fn address(key: &[u8; 32]) -> Option<String> {
    crate::seam();
    zikaron::cryptox::address_of_privkey(key).map(|a| hexfmt::encode(&a))
}

/// Build the envelope, sign, run the thirteen steps. `prev` of `None` writes `null` (the genesis position of
/// law §4.3 step 8).
pub fn seal(
    author: &str,
    entry_type: &str,
    seq: u64,
    prev: Option<&str>,
    body: Value,
    key: &[u8; 32],
) -> Result<Sealed, Token> {
    crate::seam();
    let six = shape(vec![
        (Field::Spec, Value::Str(zikaron::tokens::SPEC.to_string())),
        (Field::EntryType, Value::Str(entry_type.to_string())),
        (Field::Author, Value::Str(author.to_string())),
        (Field::Seq, Value::Int(seq)),
        (
            Field::Prev,
            match prev {
                Some(p) => Value::Str(p.to_string()),
                None => Value::Null,
            },
        ),
        (Field::Body, body),
    ]);
    let sig = sign(&k1::b6_bytes(&six), Domain::Entry.as_str(), key)?;
    let mut members = match six {
        Value::Obj(ms) => ms,
        // `shape` only builds objects, so this branch is unreachable; it continues over the value domain
        // because `unreachable!` would be a crash.
        other => vec![(Field::Body.as_str().to_string(), other)],
    };
    members.push((Field::Sig.as_str().to_string(), Value::Str(sig)));
    let bytes = json::canon_bytes(&Value::Obj(members));
    let entry = k1::check(&bytes)?;
    Ok(Sealed { bytes, entry })
}

/// Sign a preimage under a domain; returns the hex65 signature. The law's two domains come from
/// [`zikaron::tokens::Domain`], the kit's two from [`zikaron_kit::tokens::Domain`].
///
/// A key that cannot sign is reported as law §5's `E_SIG_RECOVER`: that token says the signature does not
/// recover a signer, which is what such a key produces. Out-of-range keys were already refused as misuse by
/// `args::key`.
pub fn sign(preimage: &[u8], domain: &str, key: &[u8; 32]) -> Result<String, Token> {
    crate::seam();
    let (_, digest) = k1::presig_and_digest(preimage, domain);
    let (r, s, v) = match zikaron::cryptox::sign_digest(key, &digest) {
        Some(x) => x,
        None => return Err(Token::SigRecover),
    };
    let mut raw = Vec::with_capacity(65);
    raw.extend_from_slice(&r);
    raw.extend_from_slice(&s);
    raw.push(v);
    Ok(hexfmt::encode(&raw))
}

/// The adoption cosignature of law §6.6: the preimage comes from the core's `adoption_preimage` (`prev` is
/// its third member), domain `Domain::Adoption`.
pub fn attestation(
    author: &str,
    anchors: &Value,
    prev: &str,
    key: &[u8; 32],
) -> Result<String, Token> {
    crate::seam();
    sign(
        &k1::adoption_preimage(author, anchors, prev),
        Domain::Adoption.as_str(),
        key,
    )
}
