//! Reading grant codes and files typed or dropped by the user. Vault import and the check page both use this:
//! text that is a grant code ([`code_in`]) is decoded by the kit crate; anything else is a path, and a file
//! whose content is a grant code by the same reading is decoded too; otherwise the whole file is returned as one
//! entry's bytes (validated by the caller).
//!
//! Keeping one reader means a code reads the same whether typed or in a file, with or without surrounding or
//! embedded white space. Decoding is done only by the kit crate's `badge::decode` (`zikaron.kit/1` §6); errors
//! carry its tokens with segment number and inner token.

use crate::fault::{Fault, Known};
use std::path::Path;

/// The form of the input.
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

/// Decode a payload with the kit crate; errors carry its token, segment number and inner token.
pub fn decode(payload: &[u8]) -> Result<Vec<Vec<u8>>, Fault> {
    match zikaron_kit::badge::decode(payload) {
        Ok(entries) => Ok(entries.into_iter().map(|e| e.bytes).collect()),
        Err(r) => Err(refused(r)),
    }
}

/// The kit crate's rejection of a grant code as a `PAYLOAD_REFUSED` error (token, segment and inner token).
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

/// Read the input. Returns the form and one or more byte strings: one per payload segment, the whole file for
/// an entry file, or the chain inside a grant file. Empty input is an error.
pub fn take(typed: &str) -> Result<(Form, Vec<Vec<u8>>), Fault> {
    let t = take_full(typed)?;
    Ok((t.form, t.hops))
}

/// One input read, with the opened container when it is a grant file.
pub struct Taken {
    pub form: Form,
    pub hops: Vec<Vec<u8>>,
    pub file: Option<(std::path::PathBuf, crate::grantfilex::Opened)>,
}

/// Whether bytes look like a grant code: they start with its prefix, in any letter case. A code with a
/// wrongly cased prefix is still treated as a code, so the kit crate reports `E_BADGE_PREFIX` instead of it
/// being read as a path or an entry.
fn written_as_code(b: &[u8]) -> bool {
    let p = zikaron_kit::tokens::BADGE_PREFIX.as_bytes();
    b.len() >= p.len() && b[..p.len()].eq_ignore_ascii_case(p)
}

/// The grant code in `b`, if any. All white space (including no-break spaces that messengers and mail clients
/// add) and zero-width marks ([`unseen`]) are dropped, anywhere in the text, and the rest must look like a code
/// ([`written_as_code`]). A real code (base64 segments joined by dots) contains none of these characters, so
/// dropping them never changes it. Used for typed text and files alike; non-UTF-8 bytes hold no code.
fn code_in(b: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(b).ok()?;
    let code: Vec<u8> = text.chars().filter(|c| !c.is_whitespace() && !unseen(*c)).collect::<String>().into_bytes();
    written_as_code(&code).then_some(code)
}

/// Invisible characters that are not white space but often get pasted along: zero-width space, non-joiner and
/// joiner, word joiner, soft hyphen, and the byte-order mark.
fn unseen(c: char) -> bool {
    matches!(c, '\u{200B}'..='\u{200D}' | '\u{2060}' | '\u{00AD}' | '\u{FEFF}')
}

/// Read the input in full: pasted code, payload file, grant file, or entry file.
pub fn take_full(typed: &str) -> Result<Taken, Fault> {
    let t = typed.trim();
    if t.is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::t(crate::lang::Key::Tail199).to_string()));
    }
    if let Some(code) = code_in(t.as_bytes()) {
        return Ok(Taken { form: Form::Payload, hops: decode(&code)?, file: None });
    }
    let p = Path::new(t);
    if crate::grantfilex::is_grant_file(p) {
        let o = crate::grantfilex::open(p)?;
        return Ok(Taken { form: Form::GrantFile, hops: o.hops.clone(), file: Some((p.to_path_buf(), o)) });
    }
    let bytes = std::fs::read(p).map_err(|e| crate::fault::classify(&e, &p.display().to_string()))?;
    // A payload in a file is read as typed text is; entry bytes are kept unchanged.
    if let Some(code) = code_in(&bytes) {
        return Ok(Taken { form: Form::PayloadFile, hops: decode(&code)?, file: None });
    }
    Ok(Taken { form: Form::EntryFile, hops: vec![bytes], file: None })
}
