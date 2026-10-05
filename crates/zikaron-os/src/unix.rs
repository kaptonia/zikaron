//! The unix implementation (macOS, Linux).

use std::io::Read;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

pub const ENTROPY_SOURCE: &str = "/dev/urandom";

pub fn fill_random(buf: &mut [u8]) -> std::io::Result<()> {
    // Read exactly the bytes asked for: the source never ends, so reading to its end would hang.
    std::fs::File::open(ENTROPY_SOURCE)?.read_exact(buf)
}

pub fn open(o: &crate::Options, path: &Path) -> std::io::Result<std::fs::File> {
    let mut s = std::fs::OpenOptions::new();
    s.read(o.read).write(o.write).create(o.create).create_new(o.create_new).truncate(o.truncate);
    if o.owner_only {
        s.mode(0o600);
    }
    s.open(path)
}

pub fn is_owner_only(path: &Path) -> std::io::Result<bool> {
    Ok(std::fs::metadata(path)?.permissions().mode() & 0o077 == 0)
}

/// A unix directory syncs itself; `landed` is not needed.
pub fn sync_dir(path: &Path, _landed: &Path) -> std::io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

pub fn sync_file(path: &Path) -> std::io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

pub fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    /// Read back: a file others may read or write is not owner-only, whatever the umask made it.
    #[test]
    fn a_file_others_may_read_is_not_owner_only() {
        let p = std::env::temp_dir().join(format!("zikaron-os-plain-{}", std::process::id()));
        std::fs::write(&p, b"x").expect("plain");
        for (mode, owner) in [(0o644, false), (0o604, false), (0o620, false), (0o600, true), (0o400, true)] {
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).expect("mode");
            assert_eq!(super::is_owner_only(&p).expect("read"), owner, "{mode:o}");
        }
        let _ = std::fs::remove_file(&p);
    }
}
