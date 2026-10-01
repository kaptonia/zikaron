//! Single-file kits: a disclosure kit's enumeration (kit law §7.1) packed into one file.
//!
//! A grant file is this shape: the grant code is the smallest verifiable unit, and the grant file bundles
//! what makes checking convenient (entry bytes, terms documents, the grant code text, a publication address
//! pointer, optionally the issuer's ledger). The container signs nothing and judges nothing: unpacked, it is
//! an enumeration handed to the kit core's `verify_enumeration`, the same check as a directory kit.
//!
//! Shape:
//!
//! ```text
//! zikaron-kit-file/1\n
//! per item: <kit path>\n<decimal byte count>\n<bytes>\n
//! ```
//!
//! - `manifest.json` comes first, the rest in kit-path byte order, so a reader can check each item as it
//! arrives. The same enumeration always packs to the same bytes.
//! - The reader checks as it reads: magic, each path against kit law §7.2, strictly increasing order, caps on
//! item count and total bytes. Out of bounds, truncated, out of order or malformed stops by name; no partial
//! enumeration is returned.
//! - No third-party dependency: this page is the whole format.

use std::io::Read;

/// File header, defined once.
pub const MAGIC: &str = "zikaron-kit-file/1\n";
/// Grant file extension (without the dot), defined once.
pub const EXT: &str = "zkgrant";
/// Total byte cap (item headers included).
pub const MAX_TOTAL: u64 = 64 << 20;
/// Item count cap.
pub const MAX_ITEMS: usize = 4096;

/// Ways a container fails to unpack. Closed set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Bad {
    /// The file does not start with [`MAGIC`]: not a single-file kit.
    Magic,
    /// Cut off mid-read (an unfinished header, too few bytes).
    Truncated(String),
    /// An item path fails kit law §7.2, or the first item is not the manifest.
    Path(String),
    /// Order not strictly increasing (two items with the same path included).
    Order(String),
    /// Over a cap (total bytes or item count).
    Oversize(u64),
    /// The underlying stream cannot be read.
    Io(String),
}

impl Bad {
    pub fn code(&self) -> &'static str {
        match self {
            Bad::Magic => "E_FILE_MAGIC",
            Bad::Truncated(_) => "E_FILE_TRUNCATED",
            Bad::Path(_) => "E_FILE_PATH",
            Bad::Order(_) => "E_FILE_ORDER",
            Bad::Oversize(_) => "E_FILE_OVERSIZE",
            Bad::Io(_) => "E_IO",
        }
    }

    pub fn subject(&self) -> String {
        match self {
            Bad::Magic => MAGIC.trim().to_string(),
            Bad::Truncated(s) | Bad::Path(s) | Bad::Order(s) | Bad::Io(s) => s.clone(),
            Bad::Oversize(n) => n.to_string(),
        }
    }
}

/// Pack. The enumeration comes in the shape of [`crate::pack::enumeration`] (kit path to bytes); manifest
/// first, the rest in byte order.
pub fn encode(pairs: &[(String, Vec<u8>)]) -> Vec<u8> {
    crate::seam_v2();
    let mut items: Vec<&(String, Vec<u8>)> = pairs.iter().collect();
    items.sort_by(|a, b| order_key(&a.0).cmp(&order_key(&b.0)));
    let mut out = MAGIC.as_bytes().to_vec();
    for (p, bytes) in items {
        out.extend_from_slice(p.as_bytes());
        out.push(b'\n');
        out.extend_from_slice(bytes.len().to_string().as_bytes());
        out.push(b'\n');
        out.extend_from_slice(bytes);
        out.push(b'\n');
    }
    out
}

/// Order: the manifest first, the rest by kit-path byte order.
fn order_key(p: &str) -> (u8, &[u8]) {
    (if p == crate::names::MANIFEST { 0 } else { 1 }, p.as_bytes())
}

/// Whether these bytes are a single-file kit (by the header).
pub fn is_container(head: &[u8]) -> bool {
    head.starts_with(MAGIC.as_bytes())
}

/// A stream that counts every byte read and stops past [`MAX_TOTAL`].
struct Counted<'a, R: Read> {
    r: &'a mut R,
    total: u64,
}

impl<R: Read> Counted<'_, R> {
    fn count(&mut self, n: usize) -> Result<(), Bad> {
        self.total += n as u64;
        if self.total > MAX_TOTAL {
            return Err(Bad::Oversize(MAX_TOTAL));
        }
        Ok(())
    }

    /// Read exactly `n` bytes; fewer is truncation (the subject is the item being read).
    fn exact(&mut self, n: usize, what: &str) -> Result<Vec<u8>, Bad> {
        self.count(n)?;
        let mut buf = vec![0u8; n];
        let mut got = 0usize;
        while got < n {
            match self.r.read(&mut buf[got..]) {
                Ok(0) => return Err(Bad::Truncated(what.to_string())),
                Ok(k) => got += k,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                Err(e) => return Err(Bad::Io(e.to_string())),
            }
        }
        Ok(buf)
    }

    /// Read one header line (up to, not including, the newline). A stream ending at a line start gives `None`
    /// (only when `at_start`).
    fn line(&mut self, at_start: bool, what: &str) -> Result<Option<String>, Bad> {
        let mut acc: Vec<u8> = Vec::new();
        loop {
            let mut one = [0u8; 1];
            match self.r.read(&mut one) {
                Ok(0) if at_start && acc.is_empty() => return Ok(None),
                Ok(0) => return Err(Bad::Truncated(what.to_string())),
                Ok(_) => self.count(1)?,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(Bad::Io(e.to_string())),
            }
            if one[0] == b'\n' {
                break;
            }
            acc.push(one[0]);
            // Header lines are bounded: a kit path is at most 1024 bytes (§7.2), a byte count at most 20
            // digits.
            if acc.len() > 1024 {
                return Err(Bad::Path(String::from_utf8_lossy(&acc[..64]).into_owned()));
            }
        }
        String::from_utf8(acc).map(Some).map_err(|e| Bad::Path(String::from_utf8_lossy(e.as_bytes()).into_owned()))
    }
}

/// Unpack from a stream, checking as it reads. Returns only a complete enumeration; the caller hands it to
/// the kit core's `verify_enumeration`.
pub fn decode_from(r: &mut impl Read) -> Result<Vec<(String, Vec<u8>)>, Bad> {
    crate::seam_v2();
    let mut c = Counted { r, total: 0 };
    let magic = c.exact(MAGIC.len(), "magic").map_err(|b| match b {
        Bad::Truncated(_) => Bad::Magic,
        other => other,
    })?;
    if magic != MAGIC.as_bytes() {
        return Err(Bad::Magic);
    }
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    while let Some(path) = c.line(true, "")? {
        if out.is_empty() && path != crate::names::MANIFEST {
            return Err(Bad::Path(path));
        }
        if !zikaron_kit::kitdir::is_kit_path(&path) {
            return Err(Bad::Path(path));
        }
        if let Some((last, _)) = out.last() {
            if order_key(last) >= order_key(&path) {
                return Err(Bad::Order(path));
            }
        }
        if out.len() >= MAX_ITEMS {
            return Err(Bad::Oversize(MAX_ITEMS as u64));
        }
        let size = c.line(false, &path)?.unwrap_or_default();
        let n: u64 = size.parse().map_err(|_| Bad::Truncated(path.clone()))?;
        if n > MAX_TOTAL {
            return Err(Bad::Oversize(MAX_TOTAL));
        }
        let bytes = c.exact(n as usize, &path)?;
        if c.exact(1, &path)? != b"\n" {
            return Err(Bad::Truncated(path));
        }
        out.push((path, bytes));
    }
    Ok(out)
}

/// Unpack from bytes in memory.
pub fn decode(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, Bad> {
    decode_from(&mut &bytes[..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kit() -> Vec<(String, Vec<u8>)> {
        let b = crate::pack::Bundle {
            files: vec![("a.txt".into(), b"hello".to_vec()), ("terms/x.pdf".into(), vec![0u8, 1, 2, 10])],
            note: "n".into(),
            ..Default::default()
        };
        // The verdict literal comes from the core's constant.
        crate::pack::enumerate(b).ok().expect(zikaron_kit::tokens::KIT_OK).0
    }

    #[test]
    fn round_trip_is_deterministic_and_verifies() {
        let pairs = kit();
        let a = encode(&pairs);
        let mut shuffled = pairs.clone();
        shuffled.reverse();
        assert_eq!(a, encode(&shuffled), "同一份枚举装出同一份字节");
        let back = decode(&a).ok().expect("拆得开");
        assert!(matches!(zikaron_kit::kitdir::verify_enumeration(&back), zikaron_kit::kitdir::KitVerdict::Ok { .. }));
        assert_eq!(back[0].0, crate::names::MANIFEST, "清单在先");
    }

    #[test]
    fn each_bad_form_is_named() {
        let a = encode(&kit());
        assert_eq!(decode(b"PK\x03\x04"), Err(Bad::Magic));
        assert!(matches!(decode(&a[..a.len() - 3]), Err(Bad::Truncated(_))));
        let mut swapped = MAGIC.as_bytes().to_vec();
        swapped.extend_from_slice(b"files/a.txt\n1\nx\n");
        assert!(matches!(decode(&swapped), Err(Bad::Path(_))), "头一件须是清单");
        let mut dup = MAGIC.as_bytes().to_vec();
        dup.extend_from_slice(b"manifest.json\n2\n{}\nfiles/b\n1\nx\nfiles/a\n1\nx\n");
        assert!(matches!(decode(&dup), Err(Bad::Order(_))));
        let mut big = MAGIC.as_bytes().to_vec();
        big.extend_from_slice(format!("manifest.json\n{}\n", MAX_TOTAL + 1).as_bytes());
        assert_eq!(decode(&big), Err(Bad::Oversize(MAX_TOTAL)));
        let mut bad = MAGIC.as_bytes().to_vec();
        bad.extend_from_slice(b"manifest.json\n2\n{}\n../x\n1\nx\n");
        assert!(matches!(decode(&bad), Err(Bad::Path(_))));
        // One byte flipped inside: it unpacks, and kit verification refuses it by name.
        let mut flipped = a.clone();
        let at = flipped.windows(5).position(|w| w == b"hello").expect("有这一段");
        flipped[at] ^= 1;
        let back = decode(&flipped).ok().expect("拆得开");
        assert!(matches!(zikaron_kit::kitdir::verify_enumeration(&back), zikaron_kit::kitdir::KitVerdict::Fail { .. }));
    }
}
