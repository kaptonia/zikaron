use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Latin,
    Cjk,
    Mono,
    Medium,
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Place {
    Embedded,
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

const JETBRAINS_MONO: &[u8] = include_bytes!("../fonts/JetBrainsMono-Regular.ttf");
const JETBRAINS_MONO_BOLD: &[u8] = include_bytes!("../fonts/JetBrainsMono-Bold.ttf");
const INTER: &[u8] = include_bytes!("../fonts/Inter-Regular.ttf");
const INTER_MEDIUM: &[u8] = include_bytes!("../fonts/Inter-Medium.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../fonts/Inter-SemiBold.ttf");

pub const LATIN_WEIGHTS: [(Role, &str); 2] = [(Role::Medium, "Inter-Medium.ttf"), (Role::Strong, "Inter-SemiBold.ttf")];

pub const OFL: &str = "SIL OFL 1.1";

#[cfg(target_os = "macos")]
pub const ROLES: [(Role, &str, u32, Place); 5] = [
    (Role::Latin, "Inter-Regular.ttf", 0, Place::Embedded),
    (Role::Cjk, "PingFang.ttc", 3, Place::System),
    (Role::Mono, "JetBrainsMono-Regular.ttf", 0, Place::Embedded),
    (Role::Medium, "PingFang.ttc", 7, Place::System),
    (Role::Strong, "PingFang.ttc", 11, Place::System),
];

#[cfg(target_os = "windows")]
pub const ROLES: [(Role, &str, u32, Place); 5] = [
    (Role::Latin, "Inter-Regular.ttf", 0, Place::Embedded),
    (Role::Cjk, "msyh.ttc", 1, Place::System),
    (Role::Mono, "JetBrainsMono-Regular.ttf", 0, Place::Embedded),
    (Role::Medium, "msyh.ttc", 1, Place::System),
    (Role::Strong, "msyhbd.ttc", 1, Place::System),
];

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const ROLES: [(Role, &str, u32, Place); 5] = [
    (Role::Latin, "Inter-Regular.ttf", 0, Place::Embedded),
    (Role::Cjk, "NotoSansSC-Regular.otf", 0, Place::Embedded),
    (Role::Mono, "JetBrainsMono-Regular.ttf", 0, Place::Embedded),
    (Role::Medium, "NotoSansSC-Regular.otf", 0, Place::Embedded),
    (Role::Strong, "NotoSansSC-Regular.otf", 0, Place::Embedded),
];

#[cfg(not(target_os = "macos"))]
const NOTO_SANS_SC: &[u8] = include_bytes!("../fonts/NotoSansSC-Regular.otf");

pub fn embedded(file: &str) -> Option<&'static [u8]> {
    match file {
        "JetBrainsMono-Regular.ttf" => Some(JETBRAINS_MONO),
        "JetBrainsMono-Bold.ttf" => Some(JETBRAINS_MONO_BOLD),
        "Inter-Regular.ttf" => Some(INTER),
        "Inter-Medium.ttf" => Some(INTER_MEDIUM),
        "Inter-SemiBold.ttf" => Some(INTER_SEMIBOLD),
        #[cfg(not(target_os = "macos"))]
        "NotoSansSC-Regular.otf" => Some(NOTO_SANS_SC),
        _ => None,
    }
}

#[cfg(target_os = "macos")]
pub const FALLBACK: [(Role, &str, u32, Place); 3] = [
    (Role::Cjk, "STHeiti Light.ttc", 0, Place::System),
    (Role::Medium, "STHeiti Medium.ttc", 0, Place::System),
    (Role::Strong, "STHeiti Medium.ttc", 0, Place::System),
];

#[cfg(target_os = "windows")]
pub const FALLBACK: [(Role, &str, u32, Place); 3] = [
    (Role::Cjk, "NotoSansSC-Regular.otf", 0, Place::Embedded),
    (Role::Medium, "NotoSansSC-Regular.otf", 0, Place::Embedded),
    (Role::Strong, "NotoSansSC-Regular.otf", 0, Place::Embedded),
];

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub const FALLBACK: [(Role, &str, u32, Place); 0] = [];

#[cfg(target_os = "windows")]
pub const NAMED: [(Role, &[&str]); 3] = [
    (Role::Cjk, &["Microsoft YaHei UI", "Microsoft YaHei UI Regular"]),
    (Role::Medium, &["Microsoft YaHei UI", "Microsoft YaHei UI Regular"]),
    (Role::Strong, &["Microsoft YaHei UI Bold"]),
];

#[cfg(not(target_os = "windows"))]
pub const NAMED: [(Role, &[&str]); 0] = [];

pub const STRONG_FAMILY: &str = "strong";

pub fn strong() -> egui::FontFamily {
    egui::FontFamily::Name(STRONG_FAMILY.into())
}

pub const MONO_BOLD_FAMILY: &str = "mono-bold";

pub const MONO_BOLD_FILE: &str = "JetBrainsMono-Bold.ttf";

pub fn mono_bold() -> egui::FontFamily {
    egui::FontFamily::Name(MONO_BOLD_FAMILY.into())
}

pub fn about_name() -> egui::FontId {
    egui::FontId::new(crate::tokens::Type::Page.size(), mono_bold())
}

pub const MEDIUM_FAMILY: &str = "medium";

pub fn medium() -> egui::FontFamily {
    egui::FontFamily::Name(MEDIUM_FAMILY.into())
}

#[cfg(target_os = "macos")]
pub const ROOTS: [&str; 2] = ["/System/Library/Fonts", "/System/Library/Fonts/Supplemental"];
#[cfg(target_os = "macos")]
pub const ASSETS_V2: &str = "/System/Library/AssetsV2";

#[cfg(not(target_os = "macos"))]
pub const ROOTS: [&str; 0] = [];

#[cfg(not(target_os = "windows"))]
fn roots() -> Vec<PathBuf> {
    ROOTS.iter().map(PathBuf::from).collect()
}

#[cfg(target_os = "windows")]
fn roots() -> Vec<PathBuf> {
    let win = std::env::var_os("WINDIR").or_else(|| std::env::var_os("SystemRoot")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("C:\\Windows"));
    vec![win.join("Fonts")]
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Face {
    pub role: Role,
    pub file: &'static str,
    pub index: u32,
    pub place: Place,
    pub path: Option<PathBuf>,
    pub bytes: u64,
}

impl Face {
    pub fn licence(&self) -> Option<&'static str> {
        (self.place == Place::Embedded).then_some(OFL)
    }
}

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

fn look(file: &str) -> Option<PathBuf> {
    for root in roots() {
        let p = root.join(file);
        if p.is_file() {
            return Some(p);
        }
    }
    assets_v2(file)
}

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

#[cfg(not(target_os = "macos"))]
fn assets_v2(_file: &str) -> Option<PathBuf> {
    None
}

pub fn install(ctx: &egui::Context, found: &Found) -> Vec<Role> {
    let mut defs = egui::FontDefinitions::empty();
    let mut missing = found.missing.clone();
    let mut proportional: Vec<String> = Vec::new();
    let mut monospace: Vec<String> = Vec::new();
    let mut heavy: Vec<String> = Vec::new();
    let mut middle: Vec<String> = Vec::new();
    for face in &found.faces {
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
    for (role, file) in LATIN_WEIGHTS {
        let Some(bytes) = embedded(file) else { continue };
        let name = format!("{}-latin", role.as_str());
        defs.font_data.insert(name.clone(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
        match role {
            Role::Medium => middle.insert(0, name),
            _ => heavy.insert(0, name),
        }
    }
    monospace.extend(proportional.iter().cloned());
    heavy.extend(proportional.iter().cloned());
    middle.extend(proportional.iter().cloned());
    if let Some(bytes) = embedded(MONO_BOLD_FILE) {
        defs.font_data.insert(MONO_BOLD_FAMILY.to_string(), std::sync::Arc::new(egui::FontData::from_static(bytes)));
        let mut bold = vec![MONO_BOLD_FAMILY.to_string()];
        bold.extend(proportional.iter().cloned());
        defs.families.insert(mono_bold(), bold);
    }
    defs.families.insert(egui::FontFamily::Proportional, proportional);
    defs.families.insert(egui::FontFamily::Monospace, monospace);
    defs.families.insert(strong(), heavy);
    defs.families.insert(medium(), middle);
    ctx.set_fonts(defs);
    missing
}

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
