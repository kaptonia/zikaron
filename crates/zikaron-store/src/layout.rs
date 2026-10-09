//! Layout: the only place this crate builds and recognizes file names.
//!
//! Every name in an archive falls in exactly one class: entry (bytes this crate wrote), temporary (a write in
//! progress or interrupted), system (a file the OS writes beside this crate's names, closed table
//! [`SYSTEM_SIDE`]) or foreign. So a strict read never skips anything silently: it skips system files by class
//! and refuses foreign ones.
//!
//! Entry names are 64 lowercase hex characters, the shape of an `entry_id` (law §4.2). Same name means
//! same entry, so refusing to overwrite is meaningful; lowercase only, because case-insensitive file systems
//! (APFS by default) treat names differing only in case as one file.
//!
//! The suffix and temporary-file shape are this crate's own constants, not spec constants.

use std::path::{Path, PathBuf};

/// Entry file suffix.
pub const ENTRY_SUFFIX: &str = ".entry";
/// Hex digits in an entry name (32 bytes, law §4.2).
pub const NAME_HEX_LEN: usize = 64;
/// Temporary-file prefix: hidden by the leading dot, carries the crate's name.
pub const TMP_PREFIX: &str = ".zks-tmp-";
/// Hex digits of the random part of a temporary-file name.
pub const TMP_NONCE_LEN: usize = 16;
/// Files the OS writes beside this crate's names (closed set): the Finder view file (`.DS_Store`), and an
/// AppleDouble sidecar (`._` followed by one of this crate's entry or temporary names), written on volumes
/// without extended attribute support. A sidecar of any other name is foreign.
pub const SYSTEM_SIDE: (&str, &str) = (".DS_Store", "._");

/// An entry name: 64 lowercase hex. No other shape can be built.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct EntryName(String);

impl EntryName {
    /// The only constructor; any other shape gives `None`.
    pub fn parse(s: &str) -> Option<EntryName> {
        if s.len() != NAME_HEX_LEN {
            return None;
        }
        if !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
            return None;
        }
        Some(EntryName(s.to_string()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The class of a name in an archive. Closed set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Entry,
    Tmp,
    /// A file the OS writes beside this crate's names ([`SYSTEM_SIDE`]).
    System,
    Foreign,
}

/// Classify a name. The strict read, lenient read and layout report all use this function.
pub fn classify(name: &str) -> Kind {
    if parse_entry_file(name).is_some() {
        return Kind::Entry;
    }
    if tmp_shaped(name) {
        return Kind::Tmp;
    }
    let (view, sidecar) = SYSTEM_SIDE;
    if name == view {
        return Kind::System;
    }
    if let Some(of) = name.strip_prefix(sidecar) {
        if parse_entry_file(of).is_some() || tmp_shaped(of) {
            return Kind::System;
        }
    }
    Kind::Foreign
}

pub fn entry_file_name(name: &EntryName) -> String {
    let mut s = String::with_capacity(NAME_HEX_LEN + ENTRY_SUFFIX.len());
    s.push_str(name.as_str());
    s.push_str(ENTRY_SUFFIX);
    s
}

pub fn parse_entry_file(file: &str) -> Option<EntryName> {
    let stem = file.strip_suffix(ENTRY_SUFFIX)?;
    EntryName::parse(stem)
}

/// Temporary-file name: prefix plus 16 lowercase hex. Every temporary file this crate writes has this shape,
/// and the sweep touches only files of this shape.
pub fn tmp_file_name(nonce: &str) -> String {
    let mut s = String::with_capacity(TMP_PREFIX.len() + TMP_NONCE_LEN);
    s.push_str(TMP_PREFIX);
    s.push_str(nonce);
    s
}

/// Whether a name has this crate's temporary shape (by name alone).
pub fn tmp_shaped(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(TMP_PREFIX) else {
        return false;
    };
    rest.len() == TMP_NONCE_LEN
        && rest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Whether the sweep may remove it: temporary shape and a regular file.
///
/// A directory with a temporary-looking name belongs to someone else, so the sweep leaves it alone.
pub fn is_tmp_file(name: &str, is_regular_file: bool) -> bool {
    is_regular_file && tmp_shaped(name)
}

/// Archive root plus a file name. The only place a path is joined.
///
/// A name with a path separator, `.` or `..` gives `None`: it would lead out of the archive.
pub fn path_of(root: &Path, file: &str) -> Option<PathBuf> {
    if file.is_empty() || file == "." || file == ".." {
        return None;
    }
    if file.contains('/') || file.contains('\\') || file.contains('\0') {
        return None;
    }
    Some(root.join(file))
}
