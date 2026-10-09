//! Entry assembly (used by the first-run genesis step). No byte of form or signature is invented here.
//!
//! Canonical bytes come from the core's `json::canon_bytes`; the signing preimage from the core's
//! `entry::b6_bytes` (the six members without `sig`, law §5.1); the signature from this crate's
//! `sign::sign_entry` (the domain is fixed there); the verdict from the core's `entry::check`. Every entry
//! passes the core's thirteen-step entry check before it is written, and a failure is reported with the
//! spec's rejection token.
//!
//! This module only lays the parameters out as a six-member object. A mistake is refused at once by the
//! core, so an error here is the spec's rejection token, not a message made up by the shell.

use crate::fault::{Fault, Known};
use crate::key::Secret;
use zikaron::entry as k1;
use zikaron::json::{self, Value};

/// Names of the seven envelope members (the closed set of law §4.3 step 3), spelled exactly as in the core
/// (note the camelCase `entryType`). The core's copy is private, so this is a transcription, like the CLI's
/// `codes::Field`. A misspelling cannot give a false pass: the core rejects it at once with
/// `E_ENVELOPE_MISSING`. These are JSON key names, not spec tokens, so they may be defined outside the core.
const SPEC: &str = "spec";
const ENTRY_TYPE: &str = "entryType";
const AUTHOR: &str = "author";
const SEQ: &str = "seq";
const PREV: &str = "prev";
const BODY: &str = "body";
const SIG: &str = "sig";

/// Name of a body member. Like the envelope names above, body member names are JSON keys, defined here once
/// so other modules use them instead of spelling their own.
///
/// Not to be confused with the kit crate's `Rule::NoteMd`, a rule name in kit law §7 unrelated to this key.
pub const NOTE_MD: &str = "note_md";

/// A grant's optional member naming the history entry it points to (law §6.3). A JSON key name that happens
/// to share its spelling with an entry kind (law §6).
pub const HISTORY: &str = "history";

/// An entry signed and passed through the core's entry check.
pub struct Sealed {
    pub bytes: Vec<u8>,
    pub id: String,
}

/// Builds the envelope, signs it and runs the core's entry check. A `prev` of `None` writes `null` (the
/// genesis position, law §4.3 step 8).
pub fn seal(
    secret: &Secret,
    entry_type: &str,
    seq: u64,
    prev: Option<&str>,
    body: Value,
) -> Result<Sealed, Fault> {
    let author = secret
        .address()
        .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::t(crate::lang::Key::Tail013).to_string()))?
        .hex();
    let six = Value::Obj(vec![
        (SPEC.to_string(), Value::Str(zikaron::tokens::SPEC.to_string())),
        (ENTRY_TYPE.to_string(), Value::Str(entry_type.to_string())),
        (AUTHOR.to_string(), Value::Str(author)),
        (SEQ.to_string(), Value::Int(seq)),
        (
            PREV.to_string(),
            match prev {
                Some(p) => Value::Str(p.to_string()),
                None => Value::Null,
            },
        ),
        (BODY.to_string(), body),
    ]);
    let sig = crate::sign::sign_entry(secret, &k1::b6_bytes(&six))?;
    let mut members = match six {
        Value::Obj(ms) => ms,
        other => vec![(BODY.to_string(), other)],
    };
    members.push((SIG.to_string(), Value::Str(sig)));
    let bytes = json::canon_bytes(&Value::Obj(members));
    k1::check(&bytes).map_err(Fault::entry_refused)?;
    let id = zikaron::hexfmt::encode(&k1::entry_id(&bytes));
    Ok(Sealed { bytes, id })
}

/// Genesis: `seq` 0, `prev` `null`, one line of prose in the body (law §6.1).
pub fn genesis(secret: &Secret, statement_md: &str) -> Result<Sealed, Fault> {
    seal(
        secret,
        // Entry kinds are spec tokens (law §6), so they come from the core's closed type, not spelled here.
        zikaron::tokens::EntryType::Genesis.as_str(),
        0,
        None,
        Value::Obj(vec![("statement_md".to_string(), Value::Str(statement_md.to_string()))]),
    )
}
