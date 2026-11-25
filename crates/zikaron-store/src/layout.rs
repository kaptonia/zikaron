use std::path::{Path, PathBuf};

pub const ENTRY_SUFFIX: &str = ".entry";
pub const NAME_HEX_LEN: usize = 64;
pub const TMP_PREFIX: &str = ".zks-tmp-";
pub const TMP_NONCE_LEN: usize = 16;
pub const SYSTEM_SIDE: (&str, &str) = (".DS_Store", "._");

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct EntryName(String);

impl EntryName {
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Entry,
    Tmp,
    System,
    Foreign,
}

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

pub fn tmp_file_name(nonce: &str) -> String {
    let mut s = String::with_capacity(TMP_PREFIX.len() + TMP_NONCE_LEN);
    s.push_str(TMP_PREFIX);
    s.push_str(nonce);
    s
}

pub fn tmp_shaped(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(TMP_PREFIX) else {
        return false;
    };
    rest.len() == TMP_NONCE_LEN
        && rest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn is_tmp_file(name: &str, is_regular_file: bool) -> bool {
    is_regular_file && tmp_shaped(name)
}

pub fn path_of(root: &Path, file: &str) -> Option<PathBuf> {
    if file.is_empty() || file == "." || file == ".." {
        return None;
    }
    if file.contains('/') || file.contains('\\') || file.contains('\0') {
        return None;
    }
    Some(root.join(file))
}
