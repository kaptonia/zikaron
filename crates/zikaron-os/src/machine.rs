//! Locating the machine directory, shared by the app (which writes there) and the CLI (which finds the app's IPC
//! endpoint there, `crate::door`).
//!
//! Lookup order: the pointer file in the app's machine-data folder (one absolute path line), then the legacy
//! machine directory on machines not yet migrated, then the default. The app may override all of these (its
//! test hooks); that layer is the app's own and never read here.

use std::path::{Path, PathBuf};

/// The default machine directory's name, inside the app's machine-data folder.
pub const APP_DIR: &str = ".zikaron-desk.d";

/// The pointer file's name, beside the default machine directory and outside it.
pub const POINTER: &str = ".zikaron-desk";

/// The legacy machine directory written by older versions, under the user's home directory.
pub const LEGACY_DIR: [&str; 3] = ["Library", "Application Support", "ZIKARON"];

/// Which source the machine directory came from. Closed set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    /// The pointer file.
    Pointer,
    /// No pointer, and the legacy machine directory exists.
    Legacy,
    /// Neither: the default.
    Default,
}

/// Why the machine directory could not be read. Closed.
#[derive(Debug)]
pub enum Unread {
    /// The OS does not report the user's home directory.
    NoHome,
    /// The pointer file exists but is not one absolute path line (never silently falls back to the default,
    /// which would leave the key vault and the registry behind).
    PointerShape(PathBuf),
    /// The pointer file exists but cannot be read.
    Pointer(PathBuf, std::io::Error),
}

/// The folder the pointer sits in, given the user's home directory.
pub fn pointer_dir(user_home: &Path) -> PathBuf {
    crate::app_data_dir(user_home)
}

/// Parse a pointer: an absolute path followed by exactly one newline; anything else is rejected.
pub fn read_pointer(bytes: &[u8]) -> Option<PathBuf> {
    let s = std::str::from_utf8(bytes).ok()?;
    let line = s.strip_suffix('\n')?;
    if line.is_empty() || line.contains('\n') || line.contains('\r') || !Path::new(line).is_absolute() {
        return None;
    }
    Some(PathBuf::from(line))
}

/// The machine directory for this user home, and its source. Read-only.
pub fn of(user_home: &Path) -> Result<(PathBuf, Layer), Unread> {
    let ptr = pointer_dir(user_home).join(POINTER);
    match std::fs::read(&ptr) {
        Ok(bytes) => return read_pointer(&bytes).map(|p| (p, Layer::Pointer)).ok_or(Unread::PointerShape(ptr)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Unread::Pointer(ptr, e)),
    }
    let legacy = LEGACY_DIR.iter().fold(user_home.to_path_buf(), |p, s| p.join(s));
    if legacy.is_dir() {
        return Ok((legacy, Layer::Legacy));
    }
    Ok((crate::app_data_dir(user_home).join(APP_DIR), Layer::Default))
}

/// The machine directory of this process's user (home from [`crate::user_home`]).
pub fn here() -> Result<(PathBuf, Layer), Unread> {
    of(&crate::user_home().ok_or(Unread::NoHome)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("zikaron-os-machine-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("scratch");
        d
    }

    /// Each source in turn: the default when nothing exists, the legacy directory when present, the pointer over
    /// both; a malformed pointer is refused by name, never replaced by the default.
    #[test]
    fn the_machine_directory_is_read_level_by_level() {
        let u = scratch("levels");
        assert_eq!(of(&u).expect("default").1, Layer::Default);
        assert_eq!(of(&u).expect("default").0, crate::app_data_dir(&u).join(APP_DIR));
        let legacy = LEGACY_DIR.iter().fold(u.clone(), |p, s| p.join(s));
        std::fs::create_dir_all(&legacy).expect("legacy");
        assert_eq!(of(&u).expect("legacy"), (legacy.clone(), Layer::Legacy));
        let there = u.join("elsewhere");
        std::fs::create_dir_all(pointer_dir(&u)).expect("pointer dir");
        std::fs::write(pointer_dir(&u).join(POINTER), format!("{}\n", there.display())).expect("pointer");
        assert_eq!(of(&u).expect("pointer"), (there, Layer::Pointer));
        for bad in [&b"relative\n"[..], b"/no/newline", b"/two\n/lines\n", b"\n", b"\xff\n"] {
            std::fs::write(pointer_dir(&u).join(POINTER), bad).expect("bad pointer");
            assert!(matches!(of(&u), Err(Unread::PointerShape(_))), "{bad:?}");
        }
        let _ = std::fs::remove_dir_all(&u);
    }
}
