use crate::onefile::{self, Caps, Shape};
use std::io::Read;
use std::path::Path;

pub use crate::onefile::Bad;

pub const MAGIC: &str = match std::str::from_utf8(zikaron_kit::v2::tokens::ONE_FILE_MAGIC) {
    Ok(m) => m,
    Err(_) => panic!("the one-file head of kit law §4.1a is ASCII"),
};
pub const EXT: &str = "zkgrant";
pub const EXT_KIT: &str = "zkit";
pub const EXT_HAND: &str = "zkhand";
pub const MAX_TOTAL: u64 = 64 << 20;
pub const MAX_ITEMS: usize = 4096;

pub const SHAPE: Shape = Shape { magic: MAGIC, first: Some(crate::names::MANIFEST), name_ok: zikaron_kit::kitdir::is_kit_path };

pub fn encode(pairs: &[(String, Vec<u8>)]) -> Vec<u8> {
    crate::seam_v2();
    onefile::lay(SHAPE, pairs)
}

pub fn is_container(head: &[u8]) -> bool {
    head.starts_with(MAGIC.as_bytes())
}

pub fn is_container_at(path: &Path) -> bool {
    let mut head = [0u8; 32];
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let n = f.read(&mut head).unwrap_or(0);
    is_container(&head[..n])
}

pub fn manifest_at(path: &Path) -> Option<Vec<u8>> {
    let f = std::fs::File::open(path).ok()?;
    match onefile::first(&mut std::io::BufReader::new(f), SHAPE, MAX_TOTAL) {
        Ok(Some((name, bytes))) if name == crate::names::MANIFEST => Some(bytes),
        _ => None,
    }
}

pub fn decode_from(r: &mut impl Read) -> Result<Vec<(String, Vec<u8>)>, Bad> {
    crate::seam_v2();
    onefile::decode_from(r, SHAPE, Caps { total: Some(MAX_TOTAL), items: Some(MAX_ITEMS) })
}

pub fn decode(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, Bad> {
    decode_from(&mut &bytes[..])
}

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

#[derive(Debug)]
pub enum PackTrouble {
    File(Bad),
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
        assert!(matches!(zikaron_kit::v2::kit::verify_kit(&back.iter().cloned().collect()), zikaron_kit::v2::kit::KitVerdict::Ok { .. }));
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
        for broken in ["+2", "+0", "2 ", "\t2", "02", "00", "0002", "099999999999999999999"] {
            assert!(matches!(decode(&with_header(broken)), Err(Bad::Truncated(_))), "{broken:?}");
        }
        let mut zero = MAGIC.as_bytes().to_vec();
        zero.extend_from_slice(b"manifest.json\n0\n\n");
        assert!(!matches!(decode(&zero), Err(Bad::Truncated(_))), "zero reads as zero bytes");
        let mut bad = MAGIC.as_bytes().to_vec();
        bad.extend_from_slice(b"manifest.json\n2\n{}\n../x\n1\nx\n");
        assert!(matches!(decode(&bad), Err(Bad::Path(_))));
        let mut flipped = a.clone();
        let at = flipped.windows(5).position(|w| w == b"hello").expect("有这一段");
        flipped[at] ^= 1;
        let back = decode(&flipped).ok().expect("拆得开");
        assert!(matches!(zikaron_kit::v2::kit::verify_kit(&back.iter().cloned().collect()), zikaron_kit::v2::kit::KitVerdict::Fail { .. }));
    }

    fn scratch(tag: &str) -> std::path::PathBuf {
        let mut n = [0u8; 8];
        zikaron_os::fill_random(&mut n).ok().expect("熵");
        let d = std::env::temp_dir().join(format!("zk-container-{tag}-{}", zikaron::hexfmt::encode(&n).trim_start_matches("0x")));
        std::fs::create_dir_all(&d).ok().expect("建不出");
        d
    }

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
        assert!(crate::book::kit_verdict(&w.join("back")).is_ok());
        assert_eq!(std::fs::read(w.join("back/files/big.bin")).ok(), Some(big));
        let before = std::fs::read(w.join("k.zkit")).ok();
        let again = crate::pack::export_as(&w.join("k.zkit"), crate::pack::Bundle { note: "x".into(), ..Default::default() }, crate::pack::Form::File);
        assert!(matches!(again, Err(crate::pack::Trouble::Occupied(_))));
        assert_eq!(std::fs::read(w.join("k.zkit")).ok(), before);
        assert!(matches!(unpack_to(&w.join("k.zkit"), &w.join("back")), Err(Bad::Io(_))), "读出处已有即拒");
        let raw = std::fs::read(w.join("k.zkit")).ok().expect("读");
        crate::landing::put(&w.join("cut.zkit"), &raw[..raw.len() / 2]).ok().expect("写");
        assert!(matches!(unpack_to(&w.join("cut.zkit"), &w.join("cut")), Err(Bad::Truncated(_))));
        assert!(!w.join("cut").exists());
        let _ = std::fs::remove_dir_all(&w);
    }

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
