use crate::fault::{Fault, Known};
use crate::key::Secret;
use zikaron::json::Value;
use zikaron_glue::book::{self, Law};

pub const NOTE_MD: &str = "note_md";

pub const HISTORY: &str = "history";

pub struct Sealed {
    pub bytes: Vec<u8>,
    pub id: String,
    pub seq: u64,
}

pub fn seal(
    secret: &Secret,
    entry_type: &str,
    seq: u64,
    prev: Option<&str>,
    body: Value,
) -> Result<Sealed, Fault> {
    seal_under(Law::Two, secret, entry_type, seq, prev, body)
}

pub fn seal_under(law: Law, secret: &Secret, entry_type: &str, seq: u64, prev: Option<&str>, body: Value) -> Result<Sealed, Fault> {
    let author = secret
        .address()
        .ok_or_else(|| Fault::known(Known::KeyMalformed, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail013).to_string())))?
        .hex();
    match book::seal(law, &author, entry_type, seq, prev, body, |pre, _| crate::sign::sign_entry(secret, pre, law)) {
        book::Sealed::Done(e) => Ok(Sealed { id: e.id_hex(), bytes: e.bytes, seq }),
        book::Sealed::Refused(t) => Err(Fault::entry_refused(t)),
        book::Sealed::Signer(f) => Err(f),
    }
}

pub fn genesis(secret: &Secret, statement_md: &str) -> Result<Sealed, Fault> {
    seal(secret, book::Kind::Genesis.as_str(), 0, None, book::genesis_body(statement_md, None))
}
