//! **The operating system's four**, one implementation per system behind one `cfg`:
//!
//! - [`random`]: bytes from the system's entropy source; when it cannot be read that is said, never replaced by
//!   a weaker source (a key made from a degraded source looks like a key and protects nothing).
//! - [`owner_only`] / [`is_owner_only`]: a file being created is readable and writable by its owner only, and
//!   whether a file on disk is so, read back. Files are opened through [`Options`], so the rule is part of the
//!   creation itself on every system (a system whose rule is an access list gives it in the creating call).
//! - [`sync_dir`] / [`sync_file`]: what was written to a directory (a new name in it) or to a file lasts
//!   through a power cut; failing to make it so is said.
//! - [`replace`]: a rename that replaces its target in one step, so the target is the old file or the new one,
//!   never neither.
//!
//! Which system answers is decided here only: unix (macOS, Linux) in `unix.rs`, Windows in `windows.rs`.

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as imp;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

#[cfg(not(any(unix, windows)))]
compile_error!("zikaron-os has an implementation for unix and one for Windows; this system has neither");

use std::path::Path;

/// The entropy source's name, for the words of a refusal ("cannot read <source>").
pub const ENTROPY_SOURCE: &str = imp::ENTROPY_SOURCE;

/// `n` bytes from the system's entropy source; an error names why it could not be read.
pub fn random(n: usize) -> std::io::Result<Vec<u8>> {
    let mut b = vec![0u8; n];
    imp::fill_random(&mut b)?;
    Ok(b)
}

/// Fill `buf` from the system's entropy source.
pub fn fill_random(buf: &mut [u8]) -> std::io::Result<()> {
    imp::fill_random(buf)
}

/// How a file is opened: the ways this workspace opens one (as `std::fs::OpenOptions` names them), and whether
/// a file it creates is its owner's only ([`owner_only`]). Opening goes through the system's implementation, so
/// the owner-only rule is given in the call that creates the file, never set after.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub read: bool,
    pub write: bool,
    /// Create the file when it is not there (an existing one is opened).
    pub create: bool,
    /// Create the file; an existing one is refused (`AlreadyExists`).
    pub create_new: bool,
    /// Cut an existing file to zero bytes.
    pub truncate: bool,
    /// A file this creates is readable and writable by its owner only.
    pub owner_only: bool,
}

impl Options {
    pub fn new() -> Options {
        Options::default()
    }

    pub fn read(&mut self, on: bool) -> &mut Options {
        self.read = on;
        self
    }

    pub fn write(&mut self, on: bool) -> &mut Options {
        self.write = on;
        self
    }

    pub fn create(&mut self, on: bool) -> &mut Options {
        self.create = on;
        self
    }

    pub fn create_new(&mut self, on: bool) -> &mut Options {
        self.create_new = on;
        self
    }

    pub fn truncate(&mut self, on: bool) -> &mut Options {
        self.truncate = on;
        self
    }

    /// Open (or create) the file at `path` as these options say.
    pub fn open(&self, path: &Path) -> std::io::Result<std::fs::File> {
        imp::open(self, path)
    }
}

/// Make the file these options create readable and writable by its owner only (from the moment it exists:
/// given in the creating call, not set after). An existing file it opens keeps the access it has.
pub fn owner_only(o: &mut Options) -> &mut Options {
    o.owner_only = true;
    o
}

/// Whether the file at `path` is readable and writable by its owner only.
pub fn is_owner_only(path: &Path) -> std::io::Result<bool> {
    imp::is_owner_only(path)
}

/// Make the names in the directory at `path` last through a power cut, after `landed` (a path in it) was
/// created, linked or renamed there. A system that cannot sync a directory makes `landed` itself last instead,
/// which carries its name with it.
pub fn sync_dir(path: &Path, landed: &Path) -> std::io::Result<()> {
    imp::sync_dir(path, landed)
}

/// Make the bytes of the file at `path` last through a power cut (for a file written and closed elsewhere).
pub fn sync_file(path: &Path) -> std::io::Result<()> {
    imp::sync_file(path)
}

/// Rename `from` over `to` in one step: an existing `to` is replaced.
pub fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    imp::replace(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("zikaron-os-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("scratch");
        d
    }

    #[test]
    fn entropy_is_read_and_differs() {
        let (a, b) = (random(32).expect("entropy"), random(32).expect("entropy"));
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }

    #[test]
    fn an_owner_only_file_reads_back_as_one() {
        let d = scratch("owner");
        let p = d.join("secret");
        let mut o = Options::new();
        o.write(true).create_new(true);
        owner_only(&mut o).open(&p).expect("created");
        assert!(is_owner_only(&p).expect("read"));
        assert_eq!(o.open(&p).map_err(|e| e.kind()).err(), Some(std::io::ErrorKind::AlreadyExists), "create_new refuses an existing file");
        // Opened again to keep and to cut: the bytes and the owner-only rule as the options say.
        std::fs::write(&p, b"kept").expect("write");
        Options::new().read(true).write(true).create(true).open(&p).expect("kept");
        assert_eq!(std::fs::read(&p).expect("read"), b"kept");
        Options::new().write(true).create(true).truncate(true).open(&p).expect("cut");
        assert_eq!(std::fs::read(&p).expect("read"), b"");
        assert!(is_owner_only(&p).expect("read"));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn replace_takes_the_place_whether_or_not_the_target_is_there() {
        let d = scratch("replace");
        let (a, b, t) = (d.join("a"), d.join("b"), d.join("t"));
        std::fs::write(&a, b"first").expect("a");
        replace(&a, &t).expect("into an empty place");
        assert_eq!(std::fs::read(&t).expect("t"), b"first");
        std::fs::write(&b, b"second").expect("b");
        replace(&b, &t).expect("over an existing target");
        assert_eq!(std::fs::read(&t).expect("t"), b"second");
        assert!(!a.exists() && !b.exists());
        sync_dir(&d, &t).expect("the directory syncs");
        sync_file(&t).expect("the file syncs");
        let nowhere = d.join("nowhere");
        assert!(sync_dir(&nowhere, &nowhere.join("t")).is_err(), "a directory that is not there is said");
        let _ = std::fs::remove_dir_all(&d);
    }
}
