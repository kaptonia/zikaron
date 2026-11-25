use crate::fault::{Fault, Known};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Hex,
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

pub fn expected(typed: &str) -> Result<([u8; 32], Channel), Fault> {
    crate::trace::mark(crate::feature::Feature::D4);
    let t = typed.trim();
    if t.is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail104).to_string())));
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

pub fn take(path: &Path) -> Result<Vec<u8>, Fault> {
    if path.as_os_str().is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail105).to_string())));
    }
    let md = std::fs::symlink_metadata(path)
        .map_err(|e| crate::fault::classify(&e, &path.display().to_string()))?;
    if !md.is_file() {
        return Err(Fault::known(
            Known::ContentShape,
            crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail106, &[&(path.display()).to_string()])),
        ));
    }
    std::fs::read(path).map_err(|e| crate::fault::classify(&e, &path.display().to_string()))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    pub path: String,
    pub typed: String,
    pub bytes: usize,
    pub got: [u8; 32],
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
