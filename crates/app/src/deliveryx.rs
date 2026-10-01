//! Delivery verification. Drop in the delivered bytes; sha256 against the record hash in the terms,
//! green or red; on red, the two hashes side by side.
//!
//! ─── Two channels for the hash input ───
//!
//! The terms cell takes both a 0x hash and arbitrary text: hex32 is taken as a hash; anything else is text,
//! and this desk computes its sha256. Experts and laypeople use the same cell, and the face says which
//! channel was used (`Channel`).
//!
//! ─── Delivered bytes are one file ───
//!
//! The record hash in the terms is "the sha256 of the bytes"; a directory has no single "bytes", so dropping
//! a directory is refused by name, never picking a file for the person or assembling a manifest hash (that is
//! the author seat anchoring desk's reading, not what the terms say).
//!
//! The digest is the core's `cryptox::sha256`; this layer writes no second copy.

use crate::fault::{Fault, Known};
use std::path::Path;

/// The channel the terms cell took. Closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// A 0x hash was entered.
    Hex,
    /// Text was entered; this desk computes its sha256.
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

/// The terms record hash. hex32 unchanged; anything else computed as text; empty is refused by name.
pub fn expected(typed: &str) -> Result<([u8; 32], Channel), Fault> {
    // Public functions of a component emit its trace mark, so direct calls that bypass `apply` (tests, CLI)
    // are traced too.
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

/// One check's reading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub path: String,
    /// What the person entered in the terms cell, unchanged (half of the subject; the other half is `path`).
    pub typed: String,
    pub bytes: usize,
    /// The sha256 of the received bytes.
    pub got: [u8; 32],
    /// The one the terms state.
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

/// Check. Read the file, compute sha256, compare with the terms; both are returned, and on red the face shows
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
