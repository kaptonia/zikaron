//! Delivery check: hash the delivered file with SHA-256 and compare it with the record hash in the terms.
//! Green on a match; on a mismatch both hashes are shown side by side.
//!
//! The terms cell accepts either a 0x hash or arbitrary text: a hex32 value is taken as the hash, anything
//! else is hashed as text. One cell serves experts and newcomers alike, and the page says which reading was
//! used ([`Channel`]).
//!
//! The delivered bytes must be one file. The terms state "the sha256 of the bytes", and a directory has no
//! single byte string, so a directory is refused by name rather than picking a file or hashing a manifest.
//!
//! The digest is the core's `cryptox::sha256`; there is no second implementation here.

use crate::fault::{Fault, Known};
use std::path::Path;

/// How the terms cell was read. Closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// A 0x hash was entered.
    Hex,
    /// Text was entered; its sha256 is used.
    Text,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Channel::Hex => "hex",
            Channel::Text => "text",
        }
    }
}

/// The expected record hash: a hex32 value as is, anything else hashed as text; empty input is refused by name.
pub fn expected(typed: &str) -> Result<([u8; 32], Channel), Fault> {
    // Mark the trace here too, so direct calls that bypass `apply` (tests, the CLI) are traced.
    crate::trace::mark(crate::feature::Feature::D4);
    let t = typed.trim();
    if t.is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::t(crate::lang::Key::Tail104).to_string()));
    }
    if zikaron::hexfmt::is_hex32(t) {
        let raw = zikaron::hexfmt::decode(t)
            .ok_or_else(|| Fault::known(Known::ContentShape, t.to_string()))?;
        let mut h = [0u8; 32];
        h.copy_from_slice(&raw);
        return Ok((h, Channel::Hex));
    }
    Ok((zikaron::cryptox::sha256(typed.as_bytes()), Channel::Text))
}

/// Delivered bytes. One file; a directory or other shape is refused by name.
pub fn take(path: &Path) -> Result<Vec<u8>, Fault> {
    if path.as_os_str().is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::t(crate::lang::Key::Tail105).to_string()));
    }
    let md = std::fs::symlink_metadata(path)
        .map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    if !md.is_file() {
        return Err(Fault::known(
            Known::ContentShape,
            crate::lang::filln(crate::lang::Key::Tail106, &[&(path.display()).to_string()]),
        ));
    }
    std::fs::read(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))
}

/// The result of one delivery check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub path: String,
    /// The terms cell as entered (trimmed); together with `path` it identifies what was checked.
    pub typed: String,
    pub bytes: usize,
    /// The sha256 of the received bytes.
    pub got: [u8; 32],
    /// The hash the terms state.
    pub want: [u8; 32],
    pub channel: Channel,
}

impl Checked {
    pub fn matched(&self) -> bool {
        self.got == self.want
    }

    pub fn got_hex(&self) -> String {
        zikaron::hexfmt::encode(&self.got)
    }

    pub fn want_hex(&self) -> String {
        zikaron::hexfmt::encode(&self.want)
    }
}

/// Reads the file, hashes it and compares with the terms. Both hashes are returned so a mismatch can show
/// them side by side.
pub fn check(path: &Path, typed: &str) -> Result<Checked, Fault> {
    crate::trace::mark(crate::feature::Feature::D4);
    let (want, channel) = expected(typed)?;
    let bytes = take(path)?;
    Ok(Checked {
        path: path.display().to_string(),
        typed: typed.trim().to_string(),
        bytes: bytes.len(),
        got: zikaron::cryptox::sha256(&bytes),
        want,
        channel,
    })
}
