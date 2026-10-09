//! Font lookup for the five roles, per platform:
//!
//! - macOS: Latin and monospace faces are embedded; the Chinese roles come from the system's PingFang SC
//!   (body face 3, medium face 7, strong Semibold face 11, all in one `PingFang.ttc` read once per process).
//!   Face 0 of `PingFang.ttc` is the Hong Kong glyph set and mainland readers need the simplified faces, so
//!   face numbers are fixed in the table. macOS keeps PingFang in AssetsV2 content-addressed directories
//!   (`.../<40 hex>.asset/AssetData/`) that differ per machine, so lookup walks them by file name instead of
//!   hard-coding a path. System faces are never embedded.
//! - Windows: the same embedded faces; the Chinese roles are Microsoft YaHei UI (body and medium, in
//!   `msyh.ttc`) and Microsoft YaHei UI Bold (strong, in `msyhbd.ttc`, a real bold) from the system font
//!   folder, taken by full name ([`NAMED`], [`face_named`]) rather than by a face number the file's layout
//!   could move. The embedded Noto Sans SC is the fallback.
//! - Linux and any other system: all five faces embedded; the three Chinese roles share Noto Sans SC
//!   Regular, so no Chinese font needs to be installed.
//!
//! Medium and strong text have their own faces because egui has one weight per face.
//!
//! SF and SF Mono may not be redistributed, so the Latin and monospace faces are embedded from SIL OFL 1.1
//! fonts, whose licence allows embedding and commercial use with the licence text included:
//!
//! - `JetBrainsMono-Regular.ttf` (JetBrains Mono 2.304, official release; licence
//!   `fonts/OFL-JetBrainsMono.txt`);
//! - `Inter-Regular.ttf` (Inter 4.1, official release; licence `fonts/OFL-Inter.txt`);
//! - Noto Sans SC Regular (the notofonts `noto-cjk` subset release; licence `fonts/OFL-NotoSansSC.txt`), in
//!   every build except macOS.
//!
//! The licence texts are in the repository and shown on the about page.
//!
//! This is the Chinese-font part of the platform interface (see the app's `platform` module). File names,
//! face numbers, search directories and embedded bytes live only in `ROLES`, `FALLBACK`, `ROOTS` and
//! `embedded`, chosen by build target; a port to another system adds its rows here and nothing else.
//!
//! Missing faces are not silent: `install` returns them for the caller to show on screen, since boxes for
//! missing glyphs would be a silent failure. Embedded faces are never missing.

use std::path::{Path, PathBuf};

/// The five roles; this is all the control library knows about fonts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    /// Latin body text.
    Latin,
    /// Chinese body text.
    Cjk,
    /// Monospace.
    Mono,
    /// Medium text (the words on keys).
    Medium,
    /// Strong text (page, card and group titles).
    Strong,
}

impl Role {
    pub const ALL: [Role; 5] = [Role::Latin, Role::Cjk, Role::Mono, Role::Medium, Role::Strong];

    pub fn as_str(self) -> &'static str {
        match self {
            Role::Latin => "latin",
            Role::Cjk => "cjk",
            Role::Mono => "mono",
            Role::Medium => "medium",
            Role::Strong => "strong",
        }
    }
}

/// Where a face comes from. Closed, two members.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    /// Travels with the binary (bytes in this file).
    Embedded,
    /// Found by file name in the system font directories.
    System,
}

impl Place {
    pub fn as_str(self) -> &'static str {
        match self {
            Place::Embedded => "embedded",
            Place::System => "system",
        }
    }
}

/// The two embedded faces, monospace and Latin body. Their bytes live only in these two lines.
const JETBRAINS_MONO: &[u8] = include_bytes!("../fonts/JetBrainsMono-Regular.ttf");
const INTER: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");

/// The licence of the embedded faces (reported on screen), defined once.
pub const OFL: &str = "SIL OFL 1.1";

/// Role, file name, face number and place of each first choice. Face numbers matter (PingFang faces 3, 7
/// and 11).
#[cfg(target_os = "macos")]
pub const ROLES: [(Role, &str, u32, Place); 5] = [
    (Role::Latin, "Inter-Regular.ttf", 0, Place::Embedded),
    (Role::Cjk, "PingFang.ttc", 3, Place::System),
    (Role::Mono, "JetBrainsMono-Regular.ttf", 0, Place::Embedded),
    (Role::Medium, "PingFang.ttc", 7, Place::System),
    (Role::Strong, "PingFang.ttc", 11, Place::System),
];

/// Windows: the Chinese roles from the system (Microsoft YaHei UI; the face found by its full name, see
/// [`NAMED`]; the number here is where it is expected).
#[cfg(target_os = "windows")]
pub const ROLES: [(Role, &str, u32, Place); 5] = [
    (Role::Latin, "Inter-Regular.ttf", 0, Place::Embedded),
    (Role::Cjk, "msyh.ttc", 1, Place::System),
    (Role::Mono, "JetBrainsMono-Regular.ttf", 0, Place::Embedded),
    (Role::Medium, "msyh.ttc", 1, Place::System),
    (Role::Strong, "msyhbd.ttc", 1, Place::System),
];

/// Every other system (Linux, and the neutral fallback for any other): every face embedded; the three
/// Chinese roles share Noto Sans SC Regular.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const ROLES: [(Role, &str, u32, Place); 5] = [
    (Role::Latin, "Inter-Regular.ttf", 0, Place::Embedded),
    (Role::Cjk, "NotoSansSC-Regular.otf", 0, Place::Embedded),
    (Role::Mono, "JetBrainsMono-Regular.ttf", 0, Place::Embedded),
    (Role::Medium, "NotoSansSC-Regular.otf", 0, Place::Embedded),
    (Role::Strong, "NotoSansSC-Regular.otf", 0, Place::Embedded),
];

/// The Chinese face embedded in every build but macOS's.
#[cfg(not(target_os = "macos"))]
const NOTO_SANS_SC: &[u8] = include_bytes!("../fonts/NotoSansSC-Regular.otf");

/// The bytes of an embedded face; none for other roles.
pub fn embedded(file: &str) -> Option<&'static [u8]> {
    match file {
        "JetBrainsMono-Regular.ttf" => Some(JETBRAINS_MONO),
        "Inter-Regular.ttf" => Some(INTER),
        #[cfg(not(target_os = "macos"))]
        "NotoSansSC-Regular.otf" => Some(NOTO_SANS_SC),
        _ => None,
    }
}

/// The fallback when the first choice is missing (at most one per role), with its place as in `ROLES`. The
/// embedded faces have none (they cannot be missing); a system face falls back to another Chinese face
/// (missing glyphs as boxes would be a silent failure): on macOS another system face, on Windows the embedded
/// Noto Sans SC.
#[cfg(target_os = "macos")]
pub const FALLBACK: [(Role, &str, u32, Place); 3] = [
    (Role::Cjk, "STHeiti Light.ttc", 0, Place::System),
    (Role::Medium, "STHeiti Medium.ttc", 0, Place::System),
    (Role::Strong, "STHeiti Medium.ttc", 0, Place::System),
];

/// Windows: the embedded Noto Sans SC when Microsoft YaHei UI is not on the machine.
#[cfg(target_os = "windows")]
pub const FALLBACK: [(Role, &str, u32, Place); 3] = [
    (Role::Cjk, "NotoSansSC-Regular.otf", 0, Place::Embedded),
    (Role::Medium, "NotoSansSC-Regular.otf", 0, Place::Embedded),
    (Role::Strong, "NotoSansSC-Regular.otf", 0, Place::Embedded),
];

/// Every other system: nothing to fall back to (every face is embedded).
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const FALLBACK: [(Role, &str, u32, Place); 0] = [];

/// System faces taken by full name (the `name` table's full name, record 4) rather than by number: the first
/// face of the file whose full name is one of these. A file without such a face counts as missing (the
/// fallback is taken).
#[cfg(target_os = "windows")]
pub const NAMED: [(Role, &[&str]); 3] = [
    (Role::Cjk, &["Microsoft YaHei UI", "Microsoft YaHei UI Regular"]),
    (Role::Medium, &["Microsoft YaHei UI", "Microsoft YaHei UI Regular"]),
    (Role::Strong, &["Microsoft YaHei UI Bold"]),
];

/// Every other system: system faces are taken by the number in `ROLES`.
#[cfg(not(target_os = "windows"))]
pub const NAMED: [(Role, &[&str]); 0] = [];

/// egui name of the strong family, defined once.
pub const STRONG_FAMILY: &str = "strong";

/// The strong family.
pub fn strong() -> egui::FontFamily {
    egui::FontFamily::Name(STRONG_FAMILY.into())
}

/// egui name of the medium family, defined once.
pub const MEDIUM_FAMILY: &str = "medium";

/// The medium family.
pub fn medium() -> egui::FontFamily {
    egui::FontFamily::Name(MEDIUM_FAMILY.into())
}

/// Where to look. The first two are fixed locations; AssetsV2 content-addressed directories are walked level
/// by level.
#[cfg(target_os = "macos")]
pub const ROOTS: [&str; 2] = ["/System/Library/Fonts", "/System/Library/Fonts/Supplemental"];
#[cfg(target_os = "macos")]
pub const ASSETS_V2: &str = "/System/Library/AssetsV2";

/// Every system but macOS: no fixed directory (Windows reads its font folder from where the system says it
/// is, see [`roots`]).
#[cfg(not(target_os = "macos"))]
pub const ROOTS: [&str; 0] = [];

/// The directories looked in, in order.
#[cfg(not(target_os = "windows"))]
fn roots() -> Vec<PathBuf> {
    ROOTS.iter().map(PathBuf::from).collect()
}

/// Windows: the system's font folder, `%WINDIR%\Fonts` (`SystemRoot` when `WINDIR` is not set).
#[cfg(target_os = "windows")]
fn roots() -> Vec<PathBuf> {
    let win = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    vec![win.join("Fonts")]
}

/// Where one face was taken from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub role: Role,
    /// File name without path; the same on every machine.
    pub file: &'static str,
    /// Face number.
    pub index: u32,
    /// Embedded or from the system.
    pub place: Place,
    /// Full path of a system face on this machine. It may differ per machine, so it only goes into on-screen
    /// diagnostics, never into a comparison; embedded faces have none.
    pub path: Option<PathBuf>,
    /// Size of the face in bytes (embedded: byte count; system: file length; unreadable: zero).
    pub bytes: u64,
}

impl Face {
    /// The face's licence ([`OFL`] for the embedded faces; system faces are not distributed by this crate).
    pub fn licence(&self) -> Option<&'static str> {
        (self.place == Place::Embedded).then_some(OFL)
    }
}

/// One lookup: the faces found and the roles not found.
#[derive(Clone, Debug, Default)]
pub struct Found {
    pub faces: Vec<Face>,
    pub missing: Vec<Role>,
}

impl Found {
    pub fn none() -> Found {
        Found { faces: Vec::new(), missing: Role::ALL.to_vec() }
    }

    pub fn face(&self, r: Role) -> Option<&Face> {
        self.faces.iter().find(|f| f.role == r)
    }
}

/// Lookup. Embedded faces are always present; system faces are looked up by file name in the fixed locations,
/// then in AssetsV2 content-addressed directories (a face named in [`NAMED`] must also be found by its full
/// name in that file), then once via [`FALLBACK`], which may be an embedded face.
pub fn find() -> Found {
    let mut out = Found::default();
    for (role, file, index, place) in ROLES {
        if place == Place::Embedded {
            out.faces.push(embedded_face(role, file, index));
            continue;
        }
        let first = look(file).and_then(|path| {
            let index = match NAMED.iter().find(|(r, _)| *r == role) {
                Some((_, names)) => face_named(system_bytes(&path)?, names)?,
                None => index,
            };
            let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            Some(Face { role, file, index, place, path: Some(path), bytes })
        });
        let got = first.or_else(|| {
            FALLBACK.iter().filter(|(r, _, _, _)| *r == role).find_map(|(_, f, i, p)| {
                if *p == Place::Embedded {
                    return Some(embedded_face(role, f, *i));
                }
                look(f).map(|path| {
                    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    Face { role, file: f, index: *i, place: Place::System, path: Some(path), bytes }
                })
            })
        });
        match got {
            Some(face) => out.faces.push(face),
            None => out.missing.push(role),
        }
    }
    out
}

fn embedded_face(role: Role, file: &'static str, index: u32) -> Face {
    let bytes = embedded(file).map(|b| b.len() as u64).unwrap_or(0);
    Face { role, file, index, place: Place::Embedded, path: None, bytes }
}

/// The number of the first face in a font file (a collection or a single font) whose full name (the `name`
/// table's record 4, Windows platform, Unicode) is one of `names`; none when no face has it or the file does
/// not read as a font.
pub fn face_named(bytes: &[u8], names: &[&str]) -> Option<u32> {
    let u16_at = |at: usize| bytes.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let u32_at = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let starts: Vec<usize> = if bytes.get(0..4) == Some(b"ttcf") {
        let n = u32_at(8)? as usize;
        (0..n).map(|i| u32_at(12 + 4 * i).map(|o| o as usize)).collect::<Option<_>>()?
    } else {
        vec![0]
    };
    for (i, start) in starts.into_iter().enumerate() {
        let tables = u16_at(start + 4)? as usize;
        let Some(name) = (0..tables).find_map(|k| {
            let rec = start + 12 + 16 * k;
            (bytes.get(rec..rec + 4) == Some(b"name")).then(|| u32_at(rec + 8)).flatten()
        }) else {
            continue;
        };
        let name = name as usize;
        let (count, strings) = (u16_at(name + 2)? as usize, u16_at(name + 4)? as usize);
        for r in 0..count {
            let rec = name + 6 + 12 * r;
            let (platform, encoding, id) = (u16_at(rec)?, u16_at(rec + 2)?, u16_at(rec + 6)?);
            if platform != 3 || encoding != 1 || id != 4 {
                continue;
            }
            let (len, off) = (u16_at(rec + 8)? as usize, u16_at(rec + 10)? as usize);
            let Some(raw) = bytes.get(name + strings + off..name + strings + off + len) else { continue };
            let units: Vec<u16> = raw.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
            if names.iter().any(|n| String::from_utf16(&units).map(|s| s == *n).unwrap_or(false)) {
                return u32::try_from(i).ok();
            }
        }
    }
    None
}

/// Look up by file name. Fixed locations are read directly; AssetsV2 is one content-addressed directory plus
/// one AssetData level.
fn look(file: &str) -> Option<PathBuf> {
    for root in roots() {
        let p = root.join(file);
        if p.is_file() {
            return Some(p);
        }
    }
    assets_v2(file)
}

/// macOS keeps PingFang in AssetsV2 content-addressed directories.
#[cfg(target_os = "macos")]
fn assets_v2(file: &str) -> Option<PathBuf> {
    let assets = Path::new(ASSETS_V2);
    let Ok(kinds) = std::fs::read_dir(assets) else {
        return None;
    };
    for kind in kinds.flatten() {
        let Ok(shots) = std::fs::read_dir(kind.path()) else {
            continue;
        };
        for shot in shots.flatten() {
            let p = shot.path().join("AssetData").join(file);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Only macOS has a content-addressed font store.
#[cfg(not(target_os = "macos"))]
fn assets_v2(_file: &str) -> Option<PathBuf> {
    None
}

/// Install the found faces into egui. Returns the roles not installed so the caller can say what is missing.
///
/// An unreadable file counts as missing: for the person it is the same thing.
pub fn install(ctx: &egui::Context, found: &Found) -> Vec<Role> {
    let mut defs = egui::FontDefinitions::empty();
    let mut missing = found.missing.clone();
    let mut proportional: Vec<String> = Vec::new();
    let mut monospace: Vec<String> = Vec::new();
    let mut heavy: Vec<String> = Vec::new();
    let mut middle: Vec<String> = Vec::new();
    for face in &found.faces {
        // Embedded bytes are in this file; system faces are read once per file, and unreadable counts as
        // missing.
        let bytes = match face.place {
            Place::Embedded => embedded(face.file),
            Place::System => face.path.as_deref().and_then(system_bytes),
        };
        let Some(bytes) = bytes else {
            missing.push(face.role);
            continue;
        };
        let mut data = egui::FontData::from_static(bytes);
        data.index = face.index;
        let name = face.role.as_str().to_string();
        defs.font_data.insert(name.clone(), std::sync::Arc::new(data));
        match face.role {
            Role::Latin => proportional.insert(0, name),
            Role::Cjk => proportional.push(name),
            Role::Mono => monospace.push(name),
            Role::Medium => middle.push(name),
            Role::Strong => heavy.push(name),
        }
    }
    // Chinese in monospace and Latin in medium and strong borrow from the body faces: a glyph drawn as a box
    // would be a silent failure.
    monospace.extend(proportional.iter().cloned());
    heavy.extend(proportional.iter().cloned());
    middle.extend(proportional.iter().cloned());
    defs.families.insert(egui::FontFamily::Proportional, proportional);
    defs.families.insert(egui::FontFamily::Monospace, monospace);
    defs.families.insert(strong(), heavy);
    defs.families.insert(medium(), middle);
    ctx.set_fonts(defs);
    missing
}

/// A system font file's bytes, read once per process and kept for its life: the three PingFang faces are
/// one file, so they share one copy however many times fonts are installed.
fn system_bytes(path: &Path) -> Option<&'static [u8]> {
    static READ: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<PathBuf, &'static [u8]>>> = std::sync::OnceLock::new();
    let mut held = READ.get_or_init(Default::default).lock().ok()?;
    if let Some(b) = held.get(path) {
        return Some(b);
    }
    let bytes: &'static [u8] = Box::leak(std::fs::read(path).ok()?.into_boxed_slice());
    held.insert(path.to_path_buf(), bytes);
    Some(bytes)
}
