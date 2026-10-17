//! Single-file kits: a disclosure kit's enumeration (kit law §7.1) in one file, this product's shape of the
//! one-file layout ([`crate::onefile`]).
//!
//! A record package (`.zkit`), a grant file (`.zkgrant`) and a ledger handoff file (`.zkhand`) are each a kit in
//! this shape: the grant code is the smallest verifiable unit and the grant file bundles what makes checking
//! convenient; a record package carries what the desktop or the command line chose; a handoff file carries a whole
//! ledger and the handoff's own note. The container signs and judges nothing: once read, the kit goes to the kit
//! core (`verify_enumeration` in memory, `verify_kit` on a folder), the same check as a kit folder.
//!
//! Shape:
//!
//! ```text
//! zikaron-kit-file/1\n
//! per item: <kit path>\n<decimal byte count>\n<bytes>\n
//! ```
//!
//! - `manifest.json` comes first, the rest in kit-path byte order, each path a kit path (kit law §7.2).
//! - Read into memory ([`decode`], [`decode_from`]: grant files, a kit fetched from a publish address) a file is
//! bounded by [`MAX_TOTAL`] bytes and [`MAX_ITEMS`] items. Written from a kit folder ([`pack_dir`]) and read
//! into one ([`unpack_to`]) it streams item by item and has no total bound: a record package is as large as what
//! it carries.

use crate::onefile::{self, Caps, Shape};
use std::io::Read;
use std::path::Path;

pub use crate::onefile::Bad;

/// File header.
pub const MAGIC: &str = "zikaron-kit-file/1\n";
/// Grant file extension (without the dot).
pub const EXT: &str = "zkgrant";
/// Record package extension (without the dot).
pub const EXT_KIT: &str = "zkit";
/// Ledger handoff file extension (without the dot).
pub const EXT_HAND: &str = "zkhand";
/// Total byte bound of a reader that gathers a file into memory (item headers included).
pub const MAX_TOTAL: u64 = 64 << 20;
/// Item count bound of a reader that gathers a file into memory.
pub const MAX_ITEMS: usize = 4096;

/// This product's shape: the magic, the manifest first, kit paths only.
pub const SHAPE: Shape = Shape { magic: MAGIC, first: Some(crate::names::MANIFEST), name_ok: zikaron_kit::kitdir::is_kit_path };

/// Lay out an enumeration held in memory (the shape of [`crate::pack::enumeration`]: kit path to bytes);
/// manifest first, the rest in byte order. The paths are not judged here: a verified enumeration comes in, and
/// every reader judges them again.
pub fn encode(pairs: &[(String, Vec<u8>)]) -> Vec<u8> {
    crate::seam_v2();
    onefile::lay(SHAPE, pairs)
}

/// Whether these bytes are a single-file kit (by the header).
pub fn is_container(head: &[u8]) -> bool {
    head.starts_with(MAGIC.as_bytes())
}

/// Whether the file at `path` is a single-file kit (by its first bytes; a folder or an unreadable path is not).
pub fn is_container_at(path: &Path) -> bool {
    let mut head = [0u8; 32];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let n = f.read(&mut head).unwrap_or(0);
    is_container(&head[..n])
}

/// The manifest of the single-file kit at `path`, read alone (the rest of the file is not read), within
/// [`MAX_TOTAL`]; `None` when the file is not a single-file kit or its first item does not read.
pub fn manifest_at(path: &Path) -> Option<Vec<u8>> {
    let f = std::fs::File::open(path).ok()?;
    match onefile::first(&mut std::io::BufReader::new(f), SHAPE, MAX_TOTAL) {
        Ok(Some((name, bytes))) if name == crate::names::MANIFEST => Some(bytes),
        _ => None,
    }
}

/// Read into memory from a stream, checking as it reads, within [`MAX_TOTAL`] and [`MAX_ITEMS`]. Returns only a
/// complete enumeration; the caller hands it to the kit core's `verify_enumeration`.
pub fn decode_from(r: &mut impl Read) -> Result<Vec<(String, Vec<u8>)>, Bad> {
    crate::seam_v2();
    onefile::decode_from(r, SHAPE, Caps { total: Some(MAX_TOTAL), items: Some(MAX_ITEMS) })
}

/// Read into memory from bytes.
pub fn decode(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, Bad> {
    decode_from(&mut &bytes[..])
}

/// Lay a kit folder out as one file at `out`, streaming each file of the folder into it: the manifest first, the
/// rest in kit-path byte order (the folder's whole tree, every name a kit path; a link or a name that is not one is
/// refused before anything lands). The file lands the one way files land (`landing::land_stream`): an existing
/// `out` is refused and never overwritten, and a write that fails leaves nothing there.
pub fn pack_dir(dir: &Path, out: &Path) -> Result<(), PackTrouble> {
    crate::seam_v2();
    let mut names: Vec<String> = Vec::new();
    tree(dir, "", &mut names)?;
    names.sort_by(|a, b| (a != crate::names::MANIFEST, a.as_bytes()).cmp(&(b != crate::names::MANIFEST, b.as_bytes())));
    let landed = crate::landing::land_stream(crate::landing::Readers::Anyone, out, |f| {
        let mut w = onefile::Writer::start(std::io::BufWriter::new(f), SHAPE)?;
        for name in &names {
            let path = dir.join(name);
            let mut src = std::fs::File::open(&path).map_err(|e| Bad::Io(format!("open {}: {e}", path.display())))?;
            let len = src.metadata().map_err(|e| Bad::Io(format!("read metadata {}: {e}", path.display())))?.len();
            w.put(name, len, &mut src)?;
        }
        let mut buf = w.finish();
        std::io::Write::flush(&mut buf).map_err(|e| Bad::Io(e.to_string()))?;
        Ok(())
    });
    landed.map_err(|s| match s {
        crate::landing::Streamed::Fill(b) => PackTrouble::File(b),
        crate::landing::Streamed::Land(t) => PackTrouble::Land(t),
    })
}

/// Why a kit folder did not lay out as one file, or one file did not read out into a folder.
#[derive(Debug)]
pub enum PackTrouble {
    /// The layout or the read refused (its code and subject).
    File(Bad),
    /// Landing refused or failed (something already at the path, a disk operation).
    Land(crate::landing::Trouble),
}

impl From<Bad> for PackTrouble {
    fn from(b: Bad) -> PackTrouble {
        PackTrouble::File(b)
    }
}

impl From<crate::landing::Trouble> for PackTrouble {
    fn from(t: crate::landing::Trouble) -> PackTrouble {
        PackTrouble::Land(t)
    }
}

impl PackTrouble {
    pub fn code(&self) -> &'static str {
        match self {
            PackTrouble::File(b) => b.code(),
            PackTrouble::Land(t) => t.code(),
        }
    }

    pub fn subject(&self) -> String {
        match self {
            PackTrouble::File(b) => b.subject(),
            PackTrouble::Land(t) => t.subject().to_string(),
        }
    }
}

/// The files under `dir` as kit paths (`prefix` joined), every directory walked; a link, a name that is not text,
/// or anything that is neither a file nor a directory is refused by its path.
fn tree(dir: &Path, prefix: &str, out: &mut Vec<String>) -> Result<(), Bad> {
    let read = |e: std::io::Error| Bad::Io(format!("read directory {}: {e}", dir.display()));
    for item in std::fs::read_dir(dir).map_err(read)? {
        let e = item.map_err(read)?;
        let Some(name) = e.file_name().to_str().map(str::to_string) else {
            return Err(Bad::Path(e.path().display().to_string()));
        };
        let rel = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        let meta = std::fs::symlink_metadata(e.path()).map_err(|x| Bad::Io(format!("read metadata {}: {x}", e.path().display())))?;
        if meta.is_dir() {
            tree(&e.path(), &rel, out)?;
        } else if meta.is_file() {
            out.push(rel);
        } else {
            return Err(Bad::Path(rel));
        }
    }
    Ok(())
}

/// Read one file out into the folder `dir` (made here, and not there before), streaming each item into its file:
/// every check of the layout as it reads, no total bound. On any refusal the folder is removed and the refusal
/// returned; what comes back is a kit folder for the kit core's `verify_kit`, never judged here.
pub fn unpack_to(file: &Path, dir: &Path) -> Result<usize, Bad> {
    crate::seam_v2();
    let f = std::fs::File::open(file).map_err(|e| Bad::Io(format!("open {}: {e}", file.display())))?;
    if dir.exists() {
        return Err(Bad::Io(format!("create directory {}: already there", dir.display())));
    }
    let land = |t: crate::landing::Trouble| Bad::Io(t.said().unwrap_or(t.subject()).to_string());
    crate::landing::mkdir(dir).map_err(land)?;
    let read = onefile::each(&mut std::io::BufReader::new(f), SHAPE, Caps::default(), |name, _, body| {
        let at = dir.join(name);
        if let Some(p) = at.parent() {
            crate::landing::mkdir(p).map_err(land)?;
        }
        crate::landing::put_from(&at, body).map_err(land)
    });
    if read.is_err() {
        let _ = std::fs::remove_dir_all(dir);
    }
    read
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
        // The verdict literal comes from the kit core's constant.
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
        assert_eq!(decode(b"PK\x03\x04"), Err(Bad::Magic("zikaron-kit-file/1".into())));
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
        // A length header of digits beyond 64 bits is oversize (like one past the total cap); a non-number or
        // empty header is broken.
        let with_header = |h: &str| {
            let mut v = MAGIC.as_bytes().to_vec();
            v.extend_from_slice(format!("manifest.json\n{h}\n").as_bytes());
            v
        };
        assert_eq!(decode(&with_header("99999999999999999999")), Err(Bad::Oversize(MAX_TOTAL)));
        assert_eq!(decode(&with_header("18446744073709551616")), Err(Bad::Oversize(MAX_TOTAL)));
        assert_eq!(decode(&with_header("18446744073709551615")), Err(Bad::Oversize(MAX_TOTAL)));
        for broken in ["-1", "x", "", "1.5", " 2"] {
            assert!(matches!(decode(&with_header(broken)), Err(Bad::Truncated(_))), "{broken:?}");
        }
        // Only the encoder's spelling reads: a sign, outer whitespace or a leading zero (even with the same
        // value) is a broken header; `0` and canonical digits read.
        for broken in ["+2", "+0", "2 ", "\t2", "02", "00", "0002", "099999999999999999999"] {
            assert!(matches!(decode(&with_header(broken)), Err(Bad::Truncated(_))), "{broken:?}");
        }
        let mut zero = MAGIC.as_bytes().to_vec();
        zero.extend_from_slice(b"manifest.json\n0\n\n");
        assert!(!matches!(decode(&zero), Err(Bad::Truncated(_))), "zero reads as zero bytes");
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

    fn scratch(tag: &str) -> std::path::PathBuf {
        let mut n = [0u8; 8];
        zikaron_os::fill_random(&mut n).ok().expect("熵");
        let d = std::env::temp_dir().join(format!("zk-container-{tag}-{}", zikaron::hexfmt::encode(&n).trim_start_matches("0x")));
        std::fs::create_dir_all(&d).ok().expect("建不出");
        d
    }

    /// A kit folder laid out as one file and read back into a folder is the same kit, byte for byte, whatever its
    /// size: a file past the in-memory bound streams through both ways. An existing target is refused and left as
    /// it was; a file that does not read leaves no folder behind.
    #[test]
    fn a_kit_folder_streams_into_one_file_and_back_without_a_total_bound() {
        let w = scratch("stream");
        let big = vec![0x5au8; (MAX_TOTAL as usize) + 4096];
        let b = crate::pack::Bundle { files: vec![("big.bin".into(), big.clone()), ("a.txt".into(), b"a".to_vec())], note: "n".into(), ..Default::default() };
        let landed = crate::pack::export_as(&w.join("k.zkit"), b, crate::pack::Form::File).ok().expect("导得出");
        assert!(std::fs::metadata(w.join("k.zkit")).map(|m| m.len()).unwrap_or(0) > MAX_TOTAL, "单档过内存界");
        assert!(is_container_at(&w.join("k.zkit")));
        let left: Vec<String> = std::fs::read_dir(&w).ok().expect("列").filter_map(|e| e.ok()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(left, vec!["k.zkit".to_string()], "排版那一份文件夹清掉了");
        assert_eq!(unpack_to(&w.join("k.zkit"), &w.join("back")), Ok(landed.files + 1), "清单与各件逐件读出");
        assert!(matches!(zikaron_kit::kitdir::verify_kit(&w.join("back")), zikaron_kit::kitdir::KitVerdict::Ok { .. }));
        assert_eq!(std::fs::read(w.join("back/files/big.bin")).ok(), Some(big));
        // An existing target: refused, untouched.
        let before = std::fs::read(w.join("k.zkit")).ok();
        let again = crate::pack::export_as(&w.join("k.zkit"), crate::pack::Bundle { note: "x".into(), ..Default::default() }, crate::pack::Form::File);
        assert!(matches!(again, Err(crate::pack::Trouble::Occupied(_))));
        assert_eq!(std::fs::read(w.join("k.zkit")).ok(), before);
        assert!(matches!(unpack_to(&w.join("k.zkit"), &w.join("back")), Err(Bad::Io(_))), "读出处已有即拒");
        // A cut file: refused, the folder it began is gone.
        let raw = std::fs::read(w.join("k.zkit")).ok().expect("读");
        crate::landing::put(&w.join("cut.zkit"), &raw[..raw.len() / 2]).ok().expect("写");
        assert!(matches!(unpack_to(&w.join("cut.zkit"), &w.join("cut")), Err(Bad::Truncated(_))));
        assert!(!w.join("cut").exists());
        let _ = std::fs::remove_dir_all(&w);
    }

    /// A folder holding a name that is no kit path does not lay out as one file, and nothing lands.
    #[test]
    fn a_folder_with_a_bad_name_does_not_lay_out() {
        let w = scratch("refuse");
        std::fs::create_dir_all(w.join("k")).ok().expect("建");
        crate::landing::put(&w.join("k").join(crate::names::MANIFEST), b"{}").ok().expect("写");
        crate::landing::put(&w.join("k/BAD"), b"x").ok().expect("写");
        assert!(matches!(pack_dir(&w.join("k"), &w.join("o1")), Err(PackTrouble::File(Bad::Path(_)))));
        assert!(!w.join("o1").exists());
        let _ = std::fs::remove_dir_all(&w);
    }
}
