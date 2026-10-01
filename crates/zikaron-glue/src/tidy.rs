//! Cleaning before output: platform junk is dropped and counted; anything else malformed is refused by name.
//!
//! A file whose name fails kit law §7.2 is not renamed: two different names could map to the same legal one,
//! and the dropped one would vanish unnoticed (the manifest would show one row). Renaming trades a visible
//! refusal for a silent loss. So there are two outcomes:
//!
//! - Platform junk (a closed list, plus the AppleDouble `._` prefix): dropped, and the number dropped is
//! reported. The file system made these, not the author.
//! - Everything else malformed: refused with its path; the author decides what it should be called.

use zikaron_kit::kitdir;

/// Platform junk, a closed list. An entry needs a reason, and the only one is that the file system made it.
pub const JUNK: [&str; 6] = [
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
    ".Spotlight-V100",
    ".Trashes",
    ".localized",
];

/// AppleDouble companion prefix (`._<name>`), written by macOS on non-HFS volumes.
pub const APPLE_DOUBLE: &str = "._";

/// The last segment of a path (kit paths are joined by `/`).
fn last_segment(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Whether a path is platform junk, by its last segment: junk can sit at any depth.
pub fn is_junk(path: &str) -> bool {
    let name = last_segment(path);
    JUNK.contains(&name) || name.starts_with(APPLE_DOUBLE)
}

/// What happens to a path.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fate {
    /// A kit path valid under kit law §7.2: kept.
    Keep,
    /// Platform junk: dropped and counted.
    Drop,
    /// Anything else malformed: refused, subject this path.
    Refuse,
}

/// Decide a path's fate. Whether it is a valid kit path is answered by the kit core's `is_kit_path`.
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
