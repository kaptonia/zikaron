//! The two kit documents (fingerprint manifest and acknowledgement): assemble, sign, and hand to the kit core
//! for judgment.
//!
//! Signing goes through the core's `presig_and_digest` (kit law §3.1 signs documents by parent law §5.3 to
//! §5.5 word for word); domains come from `zikaron_kit::tokens::Domain`, specs from `SPEC_FPM` / `SPEC_ACK`.
//! Before landing or printing, each document must pass the kit core's `check_fpm` / `check_ack`: row shape,
//! distinct recipients and variants, row order, and the signer being the member the document names are all
//! judged there. So a refusal from these verbs is a kit law token.

use crate::codes::Field;
use crate::entry;
use zikaron::json::{self, Value};
use zikaron::tokens::Token;
use zikaron_kit::doc::{self, Ack, Fpm, Reject};
use zikaron_kit::tokens::{Domain, SPEC_ACK, SPEC_FPM};

/// A document signed and accepted by the kit core.
pub struct Signed<T> {
    pub bytes: Vec<u8>,
    pub read: T,
}

/// Could not sign, or the kit core refused: each failure is named.
pub enum Bad {
    Sign(Token),
    Refused(Reject),
}

fn shape(members: Vec<(Field, Value)>) -> Value {
    entry::shape(members)
}

/// Kit law §4: a fingerprint manifest. `rows` comes from the caller in the law's shape (the array goes in as
/// is and the kit core judges it).
pub fn fpm(
    author: &str,
    work: &str,
    grant: Option<&str>,
    rows: Value,
    note: &str,
    key: &[u8; 32],
) -> Result<Signed<Fpm>, Bad> {
    crate::seam();
    let six = shape(vec![
        (Field::Spec, Value::Str(SPEC_FPM.to_string())),
        (Field::Author, Value::Str(author.to_string())),
        (Field::Work, Value::Str(work.to_string())),
        (
            Field::Grant,
            match grant {
                Some(g) => Value::Str(g.to_string()),
                None => Value::Null,
            },
        ),
        (Field::Rows, rows),
        (Field::NoteMd, Value::Str(note.to_string())),
    ]);
    let bytes = seal(six, Domain::Fpm, key)?;
    match doc::check_fpm(&bytes) {
        Ok(read) => Ok(Signed { bytes, read }),
        Err(r) => Err(Bad::Refused(r)),
    }
}

/// Kit law §5: an acknowledgement. The signer is `recipient` (the kit core's signature check matches it).
pub fn ack(
    recipient: &str,
    fpm_id: &str,
    variant: &str,
    note: &str,
    key: &[u8; 32],
) -> Result<Signed<Ack>, Bad> {
    crate::seam();
    let five = shape(vec![
        (Field::Spec, Value::Str(SPEC_ACK.to_string())),
        (Field::Recipient, Value::Str(recipient.to_string())),
        (Field::Fpm, Value::Str(fpm_id.to_string())),
        (Field::Variant, Value::Str(variant.to_string())),
        (Field::NoteMd, Value::Str(note.to_string())),
    ]);
    let bytes = seal(five, Domain::Ack, key)?;
    match doc::check_ack(&bytes) {
        Ok(read) => Ok(Signed { bytes, read }),
        Err(r) => Err(Bad::Refused(r)),
    }
}

/// The object without `sig` is the preimage (kit law §3.1, the same function as parent law §5.1); sign, then
/// add `sig` back.
fn seal(body: Value, domain: Domain, key: &[u8; 32]) -> Result<Vec<u8>, Bad> {
    let sig = entry::sign(&zikaron::entry::b6_bytes(&body), domain.as_str(), key)
        .map_err(Bad::Sign)?;
    let mut members = match body {
        Value::Obj(ms) => ms,
        other => vec![(Field::Body.as_str().to_string(), other)],
    };
    members.push((Field::Sig.as_str().to_string(), Value::Str(sig)));
    Ok(json::canon_bytes(&Value::Obj(members)))
}
