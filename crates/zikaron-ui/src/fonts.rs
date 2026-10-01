//! Font lookup: on macOS two faces embedded and the Chinese ones read from system paths; on Linux all five
//! embedded. This is the Chinese-font part of the
//! platform interface (see the app's `platform` module): the system faces' file names, face numbers and
//! search directories are one table per system (`ROLES`, `FALLBACK`, `ROOTS`), chosen by the build target;
//! a port to another system adds its rows here and nothing else.
//!
//! On Linux the three Chinese roles are embedded too: Noto Sans SC Regular (SIL OFL 1.1, the notofonts
//! `noto-cjk` subset release; licence `fonts/OFL-NotoSansSC.txt`), one weight for body, medium and strong, so
//! a Linux machine needs no Chinese font installed. Its bytes enter only the Linux build.
//!
//! Five faces: Latin body text embeds Inter (SIL OFL 1.1); Chinese body text is PingFang SC, face 3 (from the
//! system); monospace embeds JetBrains Mono (SIL OFL 1.1); medium text is PingFang SC Medium, face 7, and
//! strong text PingFang SC Semibold, face 11 (both from the system). Medium and strong text have their own
//! faces because egui has one weight per face. The three system faces share one file, read once per process.
//! Face 0 of `PingFang.ttc` is the Hong Kong glyph set; mainland readers need the simplified faces, so face
//! numbers live in the closed table.
//!
//! Embedded faces: SF and SF Mono may not be redistributed and PingFang ships with the system, so the Latin
//! and monospace faces are embedded from two SIL OFL 1.1 fonts, whose licence allows embedding and commercial
//! use with the licence text included:
//!
//! - `JetBrainsMono-Regular.ttf` (JetBrains Mono 2.304, official release; licence
//! `fonts/OFL-JetBrainsMono.txt`);
//! - `Inter-Regular.ttf` (Inter 4.1, official release; licence `fonts/OFL-Inter.txt`).
//!
//! The licence texts are in the repository and reported on the about page. On macOS the system faces
//! (Chinese and strong) are never embedded.
//!
//! Font file names, face numbers, search directories and the embedded faces' bytes live only in `ROLES`,
//! `ROOTS` and `embedded`.
//!
//! macOS moved PingFang into AssetsV2 content-addressed directories (`.../<40 hex>.asset/AssetData/`) that
//! differ per machine, so lookup walks the directories by file name instead of hard-coding a path.
//!
//! Missing faces are not silent: `install` returns them to the caller to show on screen. Rendering boxes for
//! missing glyphs would be a silent failure. The embedded faces are never missing.

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

/// Role, file name, face number and place: the first choice. Face numbers are part of the decision (PingFang
/// faces 3, 7 and 11).
#[cfg(not(target_os = "linux"))]
pub const ROLES: [(Role, &str, u32, Place); 5] = [
    (Role::Latin, "Inter-Regular.ttf", 0, Place::Embedded),
    (Role::Cjk, "PingFang.ttc", 3, Place::System),
    (Role::Mono, "JetBrainsMono-Regular.ttf", 0, Place::Embedded),
    (Role::Medium, "PingFang.ttc", 7, Place::System),
    (Role::Strong, "PingFang.ttc", 11, Place::System),
];

/// Linux: every face embedded; the three Chinese roles share Noto Sans SC Regular.
#[cfg(target_os = "linux")]
pub const ROLES: [(Role, &str, u32, Place); 5] = [
    (Role::Latin, "Inter-Regular.ttf", 0, Place::Embedded),
    (Role::Cjk, "NotoSansSC-Regular.otf", 0, Place::Embedded),
    (Role::Mono, "JetBrainsMono-Regular.ttf", 0, Place::Embedded),
    (Role::Medium, "NotoSansSC-Regular.otf", 0, Place::Embedded),
    (Role::Strong, "NotoSansSC-Regular.otf", 0, Place::Embedded),
];

/// The Chinese face embedded in the Linux build only.
#[cfg(target_os = "linux")]
const NOTO_SANS_SC: &[u8] = include_bytes!("../fonts/NotoSansSC-Regular.otf");

/// The bytes of an embedded face; none for other roles.
pub fn embedded(file: &str) -> Option<&'static [u8]> {
    match file {
        "JetBrainsMono-Regular.ttf" => Some(JETBRAINS_MONO),
        "Inter-Regular.ttf" => Some(INTER),
        #[cfg(target_os = "linux")]
        "NotoSansSC-Regular.otf" => Some(NOTO_SANS_SC),
        _ => None,
    }
}

/// The fallback when the first choice is missing (at most one per role). The embedded faces have none (they
/// cannot be missing); Chinese and strong fall back to another system Chinese face (missing glyphs as boxes
/// would be a silent failure).
#[cfg(not(target_os = "linux"))]
pub const FALLBACK: [(Role, &str, u32); 3] = [
    (Role::Cjk, "STHeiti Light.ttc", 0),
    (Role::Medium, "STHeiti Medium.ttc", 0),
    (Role::Strong, "STHeiti Medium.ttc", 0),
];

/// Linux: nothing to fall back to (every face is embedded).
#[cfg(target_os = "linux")]
pub const FALLBACK: [(Role, &str, u32); 0] = [];

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
#[cfg(not(target_os = "linux"))]
pub const ROOTS: [&str; 2] = ["/System/Library/Fonts", "/System/Library/Fonts/Supplemental"];
#[cfg(not(target_os = "linux"))]
pub const ASSETS_V2: &str = "/System/Library/AssetsV2";

/// Linux: no system directory is read (every face is embedded).
#[cfg(target_os = "linux")]
pub const ROOTS: [&str; 0] = [];

/// Where one face was taken from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub role: Role,
    /// File name (the decided one, without path). Same on every machine.
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
/// then in AssetsV2 content-addressed directories, then once via [`FALLBACK`]. Only these two faces are
/// embedded.
pub fn find() -> Found {
    let mut out = Found::default();
    for (role, file, index, place) in ROLES {
        if place == Place::Embedded {
            let bytes = embedded(file).map(|b| b.len() as u64).unwrap_or(0);
            out.faces.push(Face { role, file, index, place, path: None, bytes });
            continue;
        }
        let first = look(file).map(|path| (file, index, path));
        let got = first.or_else(|| {
            FALLBACK
                .iter()
                .filter(|(r, _, _)| *r == role)
                .find_map(|(_, f, i)| look(f).map(|path| (*f, *i, path)))
        });
        match got {
            Some((file, index, path)) => {
                let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                out.faces.push(Face { role, file, index, place, path: Some(path), bytes });
            }
            None => out.missing.push(role),
        }
    }
    out
}

/// Look up by file name. Fixed locations are read directly; AssetsV2 is one content-addressed directory plus
/// one AssetData level.
fn look(file: &str) -> Option<PathBuf> {
    for root in ROOTS {
        let p = Path::new(root).join(file);
        if p.is_file() {
            return Some(p);
        }
    }
    assets_v2(file)
}

/// macOS keeps PingFang in AssetsV2 content-addressed directories.
#[cfg(not(target_os = "linux"))]
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

/// Linux has no content-addressed font store.
#[cfg(target_os = "linux")]
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
