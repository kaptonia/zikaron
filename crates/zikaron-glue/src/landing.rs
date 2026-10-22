//! The single way files are written to a caller-named path.
//!
//! A file or tree is written as a temporary sibling in full, fsynced, moved into place, and the temporary is
//! cleared. If something already exists at the path, the write is refused by name and nothing is overwritten.
//!
//! `fs::write` on a caller's path would silently replace what is there, and an interrupted write (disk full,
//! killed, power loss) would leave a truncated file that looks valid. Documents and kits go to another party;
//! a truncated acknowledgement is worse than none.
//!
//! No-overwrite is enforced by the kernel: a single file is placed with `hard_link`, which fails if the target
//! exists, whereas `exists()` then rename leaves a race window. Directories can only move by `rename`, so tree
//! landings still check first.
//!
//! In this crate and in `zikaron-cli`, `std::fs::write` appears only in this file; both test suites scan for it.

use std::io::Write;
use std::path::{Path, PathBuf};

/// The two ways a landing fails.
#[derive(Debug)]
pub enum Trouble {
    /// Something is already at that path.
    Occupied(String),
    /// A disk operation failed: the subject, and the system's message (operation, path, OS text).
    Io(String, String),
}

impl Trouble {
    pub fn code(&self) -> &'static str {
        match self {
            Trouble::Occupied(_) => "E_OCCUPIED",
            Trouble::Io(..) => "E_IO",
        }
    }

    pub fn subject(&self) -> &str {
        match self {
            Trouble::Occupied(p) | Trouble::Io(p, _) => p.as_str(),
        }
    }

    /// The system's message for a failed disk operation (`None` for an occupied path).
    pub fn said(&self) -> Option<&str> {
        match self {
            Trouble::Io(_, w) => Some(w.as_str()),
            Trouble::Occupied(_) => None,
        }
    }
}

fn say(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

/// A failed disk operation on `subject`: `what` was done on `at` and failed with `e`.
fn failed(subject: &Path, what: &str, at: &Path, e: &std::io::Error) -> Trouble {
    Trouble::Io(say(subject), format!("{what} {}: {e}", at.display()))
}

/// The temporary sibling of `out`, named in the product's single temporary-file form
/// (`zikaron_store::layout::tmp_file_name`: a fixed prefix and sixteen random hex digits), so any reader of a
/// directory recognises a leftover one with `zikaron_store::layout::tmp_shaped`, whichever write left it.
fn beside(out: &Path) -> Result<PathBuf, Trouble> {
    let parent = zikaron_os::parent_dir(out);
    let nonce = nonce().map_err(|e| failed(Path::new(zikaron_os::ENTROPY_SOURCE), "read", Path::new(zikaron_os::ENTROPY_SOURCE), &e))?;
    Ok(parent.join(zikaron_store::layout::tmp_file_name(&nonce)))
}

/// Sixteen random hex digits from the system entropy source. If it cannot be read the landing fails, naming
/// the source; no weaker name is substituted.
fn nonce() -> std::io::Result<String> {
    let mut b = [0u8; 8];
    zikaron_os::fill_random(&mut b)?;
    Ok(b.iter().map(|x| format!("{x:02x}")).collect())
}

/// A temporary staging path beside the target (tree landings lay themselves out, so they need the name).
pub fn staging_beside(out: &Path) -> Result<PathBuf, Trouble> {
    beside(out)
}

/// Write one file. An existing path is refused atomically (`hard_link`); a failed write leaves no truncated
/// file.
pub fn land_bytes(out: &Path, bytes: &[u8]) -> Result<(), Trouble> {
    land_bytes_for(Readers::Anyone, out, bytes)
}

/// Who may read the written file. Closed set.
///
/// Otherwise permissions follow the process umask (commonly 0644). Documents for another party are meant to be
/// read, but a key file is not: other local accounts could copy its ciphertext and brute-force the password
/// offline. So each caller states who may read the file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Readers {
    /// Files for another party (grant documents, manifests and acknowledgements, badges, kits, mirrors): the
    /// environment's default.
    Anyone,
    /// Owner only (the key file): 0600 from the moment the temporary file is created, so it is never briefly
    /// 0644 (the move is `hard_link`, and permissions belong to the inode).
    Owner,
}

/// [`land_bytes`] with explicit readers.
pub fn land_bytes_for(readers: Readers, out: &Path, bytes: &[u8]) -> Result<(), Trouble> {
    if out.exists() {
        return Err(Trouble::Occupied(say(out)));
    }
    let tmp = beside(out)?;
    let _ = std::fs::remove_file(&tmp);
    // Write in full and fsync: after the move the bytes must be on disk, not only in the page cache.
    let wrote = (|| -> std::io::Result<()> {
        let mut f = create_for(readers, &tmp)?;
        f.write_all(bytes)?;
        f.sync_all()
    })();
    if let Err(e) = wrote {
        let _ = std::fs::remove_file(&tmp);
        return Err(failed(out, "write", &tmp, &e));
    }
    // Move into place without ever replacing (`zikaron_os::land_new`: a hard link, else a non-replacing rename,
    // else an exclusive claim of the name then a rename over it). Any existing file is refused, even an empty
    // one: a landing interrupted between claim and rename may not be ours to remove.
    let linked = zikaron_os::land_new(&tmp, out);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(_) => sync_parent(out),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(Trouble::Occupied(say(out))),
        Err(e) => Err(failed(out, "land", out, &e)),
    }
}

/// Why a streamed landing did not land: what filled the file refused, or the landing itself.
#[derive(Debug)]
pub enum Streamed<E> {
    Fill(E),
    Land(Trouble),
}

/// Write one file by streaming into it: `fill` writes the temporary sibling (whatever size), which is then fsynced
/// and moved into place as [`land_bytes_for`] moves it. An existing path is refused before `fill` runs and again,
/// atomically, at the move; `fill` refusing, or any step failing, leaves nothing at `out`.
pub fn land_stream<E>(readers: Readers, out: &Path, fill: impl FnOnce(&mut std::fs::File) -> Result<(), E>) -> Result<(), Streamed<E>> {
    if out.exists() {
        return Err(Streamed::Land(Trouble::Occupied(say(out))));
    }
    let tmp = beside(out).map_err(Streamed::Land)?;
    let _ = std::fs::remove_file(&tmp);
    let mut f = match create_for(readers, &tmp) {
        Ok(f) => f,
        Err(e) => return Err(Streamed::Land(failed(out, "write", &tmp, &e))),
    };
    if let Err(x) = fill(&mut f) {
        drop(f);
        let _ = std::fs::remove_file(&tmp);
        return Err(Streamed::Fill(x));
    }
    if let Err(e) = f.sync_all() {
        drop(f);
        let _ = std::fs::remove_file(&tmp);
        return Err(Streamed::Land(failed(out, "write", &tmp, &e)));
    }
    drop(f);
    let linked = zikaron_os::land_new(&tmp, out);
    let _ = std::fs::remove_file(&tmp);
    match linked {
        Ok(_) => sync_parent(out).map_err(Streamed::Land),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(Streamed::Land(Trouble::Occupied(say(out)))),
        Err(e) => Err(Streamed::Land(failed(out, "land", out, &e))),
    }
}

/// Create the temporary file for `readers`. Permissions are set here only.
fn create_for(readers: Readers, tmp: &Path) -> std::io::Result<std::fs::File> {
    let mut o = zikaron_os::Options::new();
    o.write(true).create(true).truncate(true);
    if readers == Readers::Owner {
        zikaron_os::owner_only(&mut o);
    }
    o.open(tmp)
}

/// Move a laid-out temporary tree into place. An existing target is refused; on failure the caller clears the
/// temporary tree.
pub fn land_tree(out: &Path, staged: &Path) -> Result<(), Trouble> {
    if out.exists() {
        return Err(Trouble::Occupied(say(out)));
    }
    match std::fs::rename(staged, out) {
        Ok(()) => sync_parent(out),
        Err(e) => Err(failed(out, "rename", staged, &e)),
    }
}

/// Write a file in full inside a staging area of our own, streamed from `body`, and fsync it: a name already
/// there (two names one case-folding file system takes as one) is refused, never written over.
pub fn put_new_from(path: &Path, body: &mut dyn std::io::Read) -> Result<(), Trouble> {
    let mut o = zikaron_os::Options::new();
    o.write(true).create_new(true);
    let mut f = o.open(path).map_err(|e| if e.kind() == std::io::ErrorKind::AlreadyExists { Trouble::Occupied(say(path)) } else { failed(path, "write", path, &e) })?;
    std::io::copy(body, &mut f).map_err(|e| failed(path, "write", path, &e))?;
    f.sync_all().map_err(|e| failed(path, "write", path, &e))
}

/// Take back what an all-or-nothing landing placed a moment ago (a file, or a tree when `tree`), when a later step of
/// the same landing failed. Only what this process just placed is ever passed here.
pub fn take_back(at: &Path, tree: bool) {
    let _ = if tree { std::fs::remove_dir_all(at) } else { std::fs::remove_file(at) };
}

/// Move a file laid out in full in a staging area of our own into place, never replacing (`zikaron_os::land_new`,
/// as [`land_bytes_for`] moves its temporary); the staged name is cleared either way. An existing path is refused.
pub fn place_file(staged: &Path, out: &Path) -> Result<(), Trouble> {
    let linked = zikaron_os::land_new(staged, out);
    let _ = std::fs::remove_file(staged);
    match linked {
        Ok(_) => sync_parent(out),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Err(Trouble::Occupied(say(out))),
        Err(e) => Err(failed(out, "land", out, &e)),
    }
}

/// Write a file inside a staging area we just created. Unlike [`land_bytes`], nothing of anyone else's can be
/// overwritten there and nobody reads it yet.
pub fn put(path: &Path, bytes: &[u8]) -> Result<(), Trouble> {
    std::fs::write(path, bytes).map_err(|e| failed(path, "write", path, &e))
}

/// [`put`], streamed from `body`: a file of any size inside a staging area we just created.
pub fn put_from(path: &Path, body: &mut dyn std::io::Read) -> Result<(), Trouble> {
    let mut f = std::fs::File::create(path).map_err(|e| failed(path, "write", path, &e))?;
    std::io::copy(body, &mut f).map(|_| ()).map_err(|e| failed(path, "write", path, &e))
}

/// Create a directory of our own.
pub fn mkdir(path: &Path) -> Result<(), Trouble> {
    std::fs::create_dir_all(path).map_err(|e| failed(path, "create directory", path, &e))
}

/// Sync the parent after the move so the name is durable too. Failure is reported (`E_IO` naming the parent)
/// rather than silently claiming success: the bytes are in place, but may not survive a power cut.
fn sync_parent(out: &Path) -> Result<(), Trouble> {
    let parent = zikaron_os::parent_dir(out);
    zikaron_os::sync_dir(parent, out).map_err(|e| failed(parent, "directory sync", parent, &e))
}
