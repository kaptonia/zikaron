use std::io::Read;
use std::path::Path;

pub const MAGIC: &[u8] = b"zikaron-kit-addressed\n";
pub const VERSION: u8 = 1;
pub const POINT: usize = 33;
pub const HEAD: usize = MAGIC.len() + 1 + POINT;
pub const SEGMENT: usize = 64 << 10;
pub const TAG: usize = 16;
pub const INFO: &str = "zikaron-kit-addressed/1";
pub const DEFAULT_STEM: &str = "kit";

pub fn is_addressed(head: &[u8]) -> bool {
    head.starts_with(MAGIC)
}

pub fn is_addressed_at(path: &Path) -> bool {
    let mut head = [0u8; MAGIC.len()];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let mut got = 0;
    while got < head.len() {
        match f.read(&mut head[got..]) {
            Ok(0) | Err(_) => break,
            Ok(n) => got += n,
        }
    }
    is_addressed(&head[..got])
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    pub version: u8,
    pub point: [u8; POINT],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BadHead {
    NotAddressed,
    Short,
    Version(u8),
}

pub fn head(bytes: &[u8]) -> Result<Head, BadHead> {
    if !is_addressed(bytes) {
        return Err(if MAGIC.starts_with(bytes) && bytes.len() < MAGIC.len() { BadHead::Short } else { BadHead::NotAddressed });
    }
    if bytes.len() < HEAD {
        return Err(BadHead::Short);
    }
    let version = bytes[MAGIC.len()];
    if version != VERSION {
        return Err(BadHead::Version(version));
    }
    let mut point = [0u8; POINT];
    point.copy_from_slice(&bytes[MAGIC.len() + 1..HEAD]);
    Ok(Head { version, point })
}

pub fn lay_head(point: &[u8; POINT]) -> [u8; HEAD] {
    let mut h = [0u8; HEAD];
    h[..MAGIC.len()].copy_from_slice(MAGIC);
    h[MAGIC.len()] = VERSION;
    h[MAGIC.len() + 1..].copy_from_slice(point);
    h
}

pub fn default_name() -> String {
    format!("{DEFAULT_STEM}.{}", crate::container::EXT_KIT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_head_lays_out_and_reads_back() {
        let p = [7u8; POINT];
        let h = lay_head(&p);
        assert_eq!(head(&h), Ok(Head { version: VERSION, point: p }));
        assert!(is_addressed(&h));
        assert!(!crate::container::is_container(&h));
    }

    #[test]
    fn a_head_that_is_short_or_of_another_version_is_named() {
        let h = lay_head(&[1u8; POINT]);
        assert_eq!(head(&h[..HEAD - 1]), Err(BadHead::Short));
        assert_eq!(head(&h[..4]), Err(BadHead::Short));
        let mut v = h;
        v[MAGIC.len()] = 2;
        assert_eq!(head(&v), Err(BadHead::Version(2)));
        assert_eq!(head(crate::container::MAGIC.as_bytes()), Err(BadHead::NotAddressed));
    }

    #[test]
    fn the_default_name_takes_nothing_from_the_package() {
        assert_eq!(default_name(), "kit.zkit");
    }
}
