//! Entry assembly (needed by the first-run genesis step). Not one byte of form or signature is invented here.
//!
//! Canonical bytes ← the core's `json::canon_bytes`; preimage ← the core's `entry::b6_bytes` (the six members
//! without `sig`, law §5.1); signature ← this crate's `sign::sign_entry` (the domain is fixed there, see the
//! `sign` file header); verdict ← the core's `entry::check`: every entry passes the thirteen steps before
//! landing, and a failure turns red with the law's token.
//!
//! This layer does one thing: lay the parameters out as a six-member object. A mistake is refused by the law
//! at once, so red here is the law's refusal token, not a sentence the shell made up.

use crate::fault::{Fault, Known};
use crate::key::Secret;
use zikaron::entry as k1;
use zikaron::json::{self, Value};

/// Names of the seven envelope members (the closed set of law §4.3 step 3). Spelled as in the core, the
/// camelCase one especially: the core's copy is a private constant and cannot be reached, so this is another
/// transcription, like the CLI's `codes::Field`; a mistake does not give a false green but the law's
/// immediate `E_ENVELOPE_MISSING` (writing `entry_type` in snake case is refused at once). These are JSON key
/// names, not the law's words, so the rule that the law's literals live only in the base does not cover them
/// (as in the CLI).
const SPEC: &str = "spec";
const ENTRY_TYPE: &str = "entryType";
const AUTHOR: &str = "author";
const SEQ: &str = "seq";
const PREV: &str = "prev";
const BODY: &str = "body";
const SIG: &str = "sig";

/// Names of members in the body. The same rule as the seven above: JSON key names, not the law's words, so
/// they are transcribed together here, and other modules call them instead of each writing its own (one name,
/// one home).
///
/// A name collision is recorded plainly: the kit crate's `Rule::NoteMd` is a rule name in kit law §7,
/// unrelated to this JSON key (the same form as `entries` in `mirror.rs`).
pub const NOTE_MD: &str = "note_md";

/// A grant's optional member "which history entry it points to" (law §6.3). It is a JSON key name, and
/// happens to share its form with an entry kind in law §6; the collision is recorded plainly, as with
/// `NOTE_MD`.
pub const HISTORY: &str = "history";

/// An entry signed and passed through the thirteen steps.
pub struct Sealed {
    pub bytes: Vec<u8>,
    pub id: String,
}

/// Build the envelope, sign, pass the thirteen steps. `prev` of `None` writes `null` (the genesis position of
/// law §4.3 step 8).
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

/// Genesis: `seq` is 0, `prev` is `null`, one line of prose in the body (law §6.1).
pub fn genesis(secret: &Secret, statement_md: &str) -> Result<Sealed, Fault> {
    seal(
        secret,
        // Entry kinds are the law's words (law §6), so they come from the core's closed type, not spelled
        // here.
        zikaron::tokens::EntryType::Genesis.as_str(),
        0,
        None,
        Value::Obj(vec![("statement_md".to_string(), Value::Str(statement_md.to_string()))]),
    )
}
