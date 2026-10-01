//! The one way the shell writes files to a caller's path.
//!
//! A file or a tree lands on the path the caller names only here: write a temporary sibling in full, fsync,
//! move into place, clear the temporary. If something is already at that path, the write is refused by name
//! and nothing is overwritten.
//!
//! `fs::write` on a caller's path would silently replace what is there, and a write cut off (disk full,
//! killed, power loss) would leave a truncated file that looks like a good one. Documents and kits go to
//! another party; a truncated acknowledgement is worse than none.
//!
//! Not overwriting is enforced by the kernel: a single file lands by `hard_link`, which fails when the target
//! exists. Checking `exists()` and then renaming leaves a window in which another process can create the
//! name. A tree has no such call (directories move only by `rename`), so that path still checks first.
//!
//! In this crate and in `zikaron-cli`, `std::fs::write` appears only in this file; the test suites of both
//! scan for it.

use std::io::Write;
use std::path::{Path, PathBuf};

/// The two ways a landing fails.
#[derive(Debug)]
pub enum Trouble {
    /// Something is already at that path.
    Occupied(String),
    /// A disk operation failed; the subject is named.
    Io(String),
}

impl Trouble {
    pub fn code(&self) -> &'static str {
        match self {
            Trouble::Occupied(_) => "E_OCCUPIED",
            Trouble::Io(_) => "E_IO",
        }
    }

    pub fn subject(&self) -> &str {
        match self {
            Trouble::Occupied(p) | Trouble::Io(p) => p.as_str(),
        }
    }
}

fn say(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// The temporary sibling shares the target's parent: a move is atomic only on one file system.
///
/// The name carries the process id and a random part read from the OS, so two processes landing on one path
/// in the same second do not collide.
fn beside(out: &Path, tag: &str) -> PathBuf {
    let parent = out.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
    let base = out
        .file_name()
        .map(|x| x.to_string_lossy().into_owned())
        .unwrap_or_else(|| String::from("landing"));
    parent.join(format!(".{base}.{tag}-{}-{}", std::process::id(), nonce()))
}

/// Sixteen random hex digits. Without entropy, falls back to the process id and a counter: the name only
/// needs to be unique and nothing depends on it.
fn nonce() -> String {
    use std::io::Read;
    let mut b = [0u8; 8];
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        if f.read_exact(&mut b).is_ok() {
            return b.iter().map(|x| format!("{x:02x}")).collect();
        }
    }
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    format!("{:016x}", N.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

/// A temporary place beside the target (tree landings lay themselves out, so they need the name).
pub fn staging_beside(out: &Path) -> PathBuf {
    beside(out, "staging")
}

/// Land one file. An existing name is refused (the atomicity of `hard_link`); a failed write leaves no
/// truncated file.
pub fn land_bytes(out: &Path, bytes: &[u8]) -> Result<(), Trouble> {
    land_bytes_for(Readers::Anyone, out, bytes)
}

/// Who may read what lands. Closed set.
///
/// Without this, every file's permissions follow the process umask (commonly 0644). Documents for another
/// party are meant to be read, but a key file is not: other accounts on the same machine could copy its
/// ciphertext and brute-force the password offline. So each caller names who may read the file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Readers {
    /// What goes to another party (grant documents, manifests and acknowledgements, badges, kits, mirrors):
    /// the environment's default.
    Anyone,
    /// Owner only (the key file): 0600 from the moment the temporary file is created, so there is no window
    /// where it is 0644 first (the move is `hard_link`, and permissions belong to the same inode).
    Owner,
}

/// The same landing, with who may read the file.
pub fn land_bytes_for(readers: Readers, out: &Path, bytes: &[u8]) -> Result<(), Trouble> {
    if out.exists() {
        return Err(Trouble::Occupied(say(out)));
    }
    let tmp = beside(out, "landing");
    let _ = std::fs::remove_file(&tmp);
    // Write in full and fsync: after the move the bytes must be on disk, not only in the page cache.
    let wrote = (|| -> std::io::Result<()> {
        let mut f = create_for(readers, &tmp)?;
        f.write_all(bytes)?;
        f.sync_all()
    })();
    if wrote.is_err() {
        let _ = std::fs::remove_file(&tmp);
        return Err(Trouble::Io(say(out)));
    }
    // Move into place: fails if the target exists, so nothing is overwritten.
    let linked = std::fs::hard_link(&tmp, out);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(()) => {
            sync_parent(out);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(Trouble::Occupied(say(out))),
        Err(_) => Err(Trouble::Io(say(out))),
    }
}

/// Create the temporary file for `readers`. Permissions are set here only.
fn create_for(readers: Readers, tmp: &Path) -> std::io::Result<std::fs::File> {
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    if readers == Readers::Owner {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    o.open(tmp)
}

/// Move an already laid-out temporary tree into place. An existing target is refused; on failure the caller
/// clears the temporary tree.
pub fn land_tree(out: &Path, staged: &Path) -> Result<(), Trouble> {
    if out.exists() {
        return Err(Trouble::Occupied(say(out)));
    }
    match std::fs::rename(staged, out) {
        Ok(()) => {
            sync_parent(out);
            Ok(())
        }
        Err(_) => Err(Trouble::Io(say(out))),
    }
}

/// Put something into a temporary place we just created.
///
/// Different from [`land_bytes`]: this lays out our own staging area, where nothing of anyone else can be
/// overwritten and nobody can take anything yet.
pub fn put(path: &Path, bytes: &[u8]) -> Result<(), Trouble> {
    std::fs::write(path, bytes).map_err(|_| Trouble::Io(say(path)))
}

/// Create a directory of our own.
pub fn mkdir(path: &Path) -> Result<(), Trouble> {
    std::fs::create_dir_all(path).map_err(|_| Trouble::Io(say(path)))
}

/// fsync the parent after the move so the name is on disk too. Failure does not change the outcome (it only
/// affects durability).
fn sync_parent(out: &Path) {
    if let Some(parent) = out.parent() {
        if let Ok(d) = std::fs::File::open(parent) {
            let _ = d.sync_all();
        }
    }
}
