use crate::ANCHOR_SELECTOR;
use zikaron::cryptox;
use zikaron::v2::entry::{self, Entry};

pub fn carried(c: &[u8]) -> Option<Entry> {
    if c.len() <= 36 || c[..4] != ANCHOR_SELECTOR {
        return None;
    }
    let tail = &c[36..];
    if cryptox::sha256(tail)[..] != c[4..36] {
        return None;
    }
    entry::check(tail).ok()
}

