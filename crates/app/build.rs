//! The third-party notices the app carries, made now for the target being built by the one generator the
//! packages also use (`zikaron_pack::notices`), for the app's own dependency tree, and written into the
//! build's output directory, where `about::NOTICES` embeds it. The settings page "About" shows it.
//!
//! A build where the list cannot be made stops here with the reason: a binary without its notices is not built.
//!
//! For a Windows target only, the window binary also gets its version resource and icon: a compiled resource
//! file is written here (no resource compiler is run) and handed to the linker. Other targets get nothing more.

use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let workspace = manifest_dir.join("../..");
    println!("cargo:rerun-if-changed={}", workspace.join("Cargo.lock").display());
    println!("cargo:rerun-if-changed=build.rs");
    let target = std::env::var("TARGET").expect("TARGET");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let notices = zikaron_pack::notices::make(&cargo, &workspace.join("Cargo.toml"), &target, &["app"], &[])
        .unwrap_or_else(|e| panic!("third-party notices: {e}"));
    let dest = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("third-party-notices.txt");
    std::fs::write(&dest, notices).unwrap_or_else(|e| panic!("third-party notices: cannot write {}: {e}", dest.display()));
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let icons = workspace.join("packaging/icon/hicolor");
        let res = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("zikaron-desk.res");
        let version = std::env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
        std::fs::write(&res, resources(&icons, &version)).unwrap_or_else(|e| panic!("windows resources: cannot write {}: {e}", res.display()));
        println!("cargo:rustc-link-arg-bin=app={}", res.display());
    }
}

/// The sizes of the window icon, one image each (PNG, which the system reads inside an icon since Vista).
const ICON_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

/// `RT_ICON`, `RT_GROUP_ICON`, `RT_VERSION`.
const RT_ICON: u16 = 3;
const RT_GROUP_ICON: u16 = 14;
const RT_VERSION: u16 = 16;
/// English (United States), and the Unicode code page of the version strings.
const LANGUAGE: u16 = 0x0409;
const CODE_PAGE: u16 = 1200;

/// A compiled resource file (`.res`): an empty first entry, the icon images, the icon group, the version.
fn resources(icons: &Path, version: &str) -> Vec<u8> {
    let mut out = entry(0, 0, &[]);
    let mut group = Vec::new();
    group.extend_from_slice(&0u16.to_le_bytes());
    group.extend_from_slice(&1u16.to_le_bytes());
    group.extend_from_slice(&(ICON_SIZES.len() as u16).to_le_bytes());
    for (i, size) in ICON_SIZES.iter().enumerate() {
        let file = icons.join(format!("{size}.png"));
        println!("cargo:rerun-if-changed={}", file.display());
        let png = std::fs::read(&file).unwrap_or_else(|e| panic!("windows resources: cannot read {}: {e}", file.display()));
        let side = |at: usize| png.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
        assert!(png.starts_with(b"\x89PNG") && side(16) == Some(*size) && side(20) == Some(*size), "windows resources: {} is not a {size}x{size} PNG", file.display());
        let id = (i + 1) as u16;
        out.extend(entry(RT_ICON, id, &png));
        // `GRPICONDIRENTRY`: width and height (0 is 256), colours, reserved, planes, bits, bytes, id.
        let edge = if *size >= 256 { 0 } else { *size as u8 };
        group.extend_from_slice(&[edge, edge, 0, 0]);
        group.extend_from_slice(&1u16.to_le_bytes());
        group.extend_from_slice(&32u16.to_le_bytes());
        group.extend_from_slice(&(png.len() as u32).to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes());
    }
    out.extend(entry(RT_GROUP_ICON, 1, &group));
    out.extend(entry(RT_VERSION, 1, &version_info(version)));
    out
}

/// One resource: the header (sizes, numbered type and name, flags, language) and the data, padded to four bytes.
fn entry(kind: u16, id: u16, data: &[u8]) -> Vec<u8> {
    let mut h = Vec::new();
    h.extend_from_slice(&(data.len() as u32).to_le_bytes());
    h.extend_from_slice(&32u32.to_le_bytes());
    h.extend_from_slice(&[0xFF, 0xFF]);
    h.extend_from_slice(&kind.to_le_bytes());
    h.extend_from_slice(&[0xFF, 0xFF]);
    h.extend_from_slice(&id.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    let (flags, language) = if kind == 0 { (0u16, 0u16) } else { (0x1030u16, LANGUAGE) };
    h.extend_from_slice(&flags.to_le_bytes());
    h.extend_from_slice(&language.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(&0u32.to_le_bytes());
    h.extend_from_slice(data);
    pad(&mut h);
    h
}

fn pad(b: &mut Vec<u8>) {
    while b.len() % 4 != 0 {
        b.push(0);
    }
}

fn utf16z(s: &str) -> Vec<u8> {
    s.encode_utf16().chain(std::iter::once(0)).flat_map(|c| c.to_le_bytes()).collect()
}

/// One block of the version resource: length, value length, kind (1 text, 0 binary), key, value, children,
/// each part starting on four bytes.
fn block(key: &str, value: &[u8], value_len: u16, text: bool, children: &[Vec<u8>]) -> Vec<u8> {
    let mut b = vec![0, 0];
    b.extend_from_slice(&value_len.to_le_bytes());
    b.extend_from_slice(&u16::from(text).to_le_bytes());
    b.extend(utf16z(key));
    pad(&mut b);
    b.extend_from_slice(value);
    for c in children {
        pad(&mut b);
        b.extend_from_slice(c);
    }
    let len = b.len() as u16;
    b[..2].copy_from_slice(&len.to_le_bytes());
    b
}

/// `VS_VERSIONINFO`: the fixed part (version numbers, an application for 32-bit Windows) and the strings.
fn version_info(version: &str) -> Vec<u8> {
    let n: Vec<u16> = version.split('.').map(|x| x.parse().unwrap_or(0)).chain(std::iter::repeat(0)).take(4).collect();
    let (ms, ls) = ((u32::from(n[0]) << 16) | u32::from(n[1]), (u32::from(n[2]) << 16) | u32::from(n[3]));
    let mut fixed = Vec::new();
    for d in [0xFEEF_04BDu32, 0x0001_0000, ms, ls, ms, ls, 0x3F, 0, 0x0004_0004, 1, 0, 0, 0] {
        fixed.extend_from_slice(&d.to_le_bytes());
    }
    let strings: Vec<Vec<u8>> = [
        ("CompanyName", "Kaptonia"),
        ("FileDescription", "ZIKARON"),
        ("FileVersion", version),
        ("InternalName", "zikaron-desk"),
        ("LegalCopyright", "Copyright Kaptonia. MIT License."),
        ("OriginalFilename", "zikaron-desk.exe"),
        ("ProductName", "ZIKARON"),
        ("ProductVersion", version),
    ]
    .iter()
    .map(|(k, v)| block(k, &utf16z(v), (v.encode_utf16().count() + 1) as u16, true, &[]))
    .collect();
    let table = block(&format!("{LANGUAGE:04X}{CODE_PAGE:04X}"), &[], 0, true, &strings);
    let string_info = block("StringFileInfo", &[], 0, true, &[table]);
    let mut translation = LANGUAGE.to_le_bytes().to_vec();
    translation.extend_from_slice(&CODE_PAGE.to_le_bytes());
    let var = block("Translation", &translation, 4, false, &[]);
    let var_info = block("VarFileInfo", &[], 0, true, &[var]);
    block("VS_VERSION_INFO", &fixed, fixed.len() as u16, false, &[string_info, var_info])
}
