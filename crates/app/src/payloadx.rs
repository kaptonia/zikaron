//! One reading for payloads and document files. Vault import and check page intake both take bytes
//! from here: pasted text starting with `zikaron-grant:` goes to the kit crate to decode; anything else is
//! read as a path, and a file whose content (after removing ASCII whitespace) starts with that prefix is
//! decoded too; otherwise the whole file is returned as one entry's bytes (whether it passes the law is
//! decided by the taking exit).
//!
//! Two exits reading on their own (one stripping whitespace, one not) would accept a payload file with a
//! trailing newline in one and refuse it in the other; so the reading lives only here. Decoding belongs to
//! the kit crate's `badge::decode` (kit law §6); this layer decodes not one byte, and refusals are kit law
//! tokens with segment number and inner token.

use crate::fault::{Fault, Known};
use std::path::Path;

/// Which form was taken in. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Form {
    /// Pasted payload text (the same form as a scanned code).
    Payload,
    /// A file containing a payload.
    PayloadFile,
    /// A file containing an entry's bytes.
    EntryFile,
    /// A grant file (single-file container, `grantfilex`): opened and verified, with the chain taken from the
    /// grant code inside.
    GrantFile,
}

impl Form {
    pub fn as_str(self) -> &'static str {
        match self {
            Form::Payload => "payload",
            Form::PayloadFile => "payload_file",
            Form::EntryFile => "entry_file",
            Form::GrantFile => "grant_file",
        }
    }
}

/// The payload is decoded by the kit crate; refusals are kit law tokens, with segment number and inner token.
pub fn decode(payload: &[u8]) -> Result<Vec<Vec<u8>>, Fault> {
    match zikaron_kit::badge::decode(payload) {
        Ok(entries) => Ok(entries.into_iter().map(|e| e.bytes).collect()),
        Err(r) => Err(refused(r)),
    }
}

/// The kit core's refusal of a grant code, named (`PAYLOAD_REFUSED` with its token, segment and inner token).
pub fn refused(r: zikaron_kit::badge::DecodeReject) -> Fault {
    Fault::known(
        Known::PayloadRefused,
        format!(
            "{}{}{}",
            r.token.as_str(),
            r.index.map(|i| crate::lang::filln(crate::lang::Key::Tail087, &[&(i).to_string()])).unwrap_or_default(),
            r.inner.map(|t| format!(" {t:?}")).unwrap_or_default()
        ),
    )
}

/// Take. Returns (form, one or more byte strings): one per payload segment; an entry file is that whole file;
/// a grant file is the chain inside it. Empty is refused by name.
pub fn take(typed: &str) -> Result<(Form, Vec<Vec<u8>>), Fault> {
    let t = take_full(typed)?;
    Ok((t.form, t.hops))
}

/// One item taken in, with the opened bundle when it is a grant file.
pub struct Taken {
    pub form: Form,
    pub hops: Vec<Vec<u8>>,
    pub file: Option<(std::path::PathBuf, crate::grantfilex::Opened)>,
}

/// Take, in full. The only reading: pasted code, payload file, grant file, entry file.
pub fn take_full(typed: &str) -> Result<Taken, Fault> {
    let t = typed.trim();
    if t.is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::t(crate::lang::Key::Tail199).to_string()));
    }
    if t.starts_with(zikaron_kit::tokens::BADGE_PREFIX) {
        return Ok(Taken { form: Form::Payload, hops: decode(t.as_bytes())?, file: None });
    }
    let p = Path::new(t);
    if crate::grantfilex::is_grant_file(p) {
        let o = crate::grantfilex::open(p)?;
        return Ok(Taken { form: Form::GrantFile, hops: o.hops.clone(), file: Some((p.to_path_buf(), o)) });
    }
    let bytes = std::fs::read(p).map_err(|e| crate::fault::classify(&e, &p.display().to_string()))?;
    // A payload in a file: remove ASCII whitespace (trailing newline, indentation) before checking the prefix
    // and decoding; entry bytes are unchanged.
    let trimmed: Vec<u8> = bytes.iter().copied().filter(|c| !c.is_ascii_whitespace()).collect();
    if trimmed.starts_with(zikaron_kit::tokens::BADGE_PREFIX.as_bytes()) {
        return Ok(Taken { form: Form::PayloadFile, hops: decode(&trimmed)?, file: None });
    }
    Ok(Taken { form: Form::EntryFile, hops: vec![bytes], file: None })
}
