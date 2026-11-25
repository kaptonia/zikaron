use zikaron_kit::kitdir;

pub const JUNK: [&str; 6] = [
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
    ".Spotlight-V100",
    ".Trashes",
    ".localized",
];

pub const APPLE_DOUBLE: &str = "._";

fn last_segment(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

pub fn is_junk(path: &str) -> bool {
    let name = last_segment(path);
    JUNK.contains(&name) || name.starts_with(APPLE_DOUBLE)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fate {
    Keep,
    Drop,
    Refuse,
}

pub fn fate(path: &str) -> Fate {
    crate::seam_v2();
    if is_junk(path) {
        return Fate::Drop;
    }
    if kitdir::is_kit_path(path) {
        Fate::Keep
    } else {
        Fate::Refuse
    }
}
