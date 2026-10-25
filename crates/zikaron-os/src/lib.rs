//! Platform services, one implementation per OS behind a single `cfg`:
//!
//! - [`random`]: bytes from the system entropy source; failure is reported, never replaced by a weaker source
//!   (a key from a degraded source looks like a key and protects nothing).
//! - [`owner_only`] / [`is_owner_only`]: create a file readable and writable by its owner only, and check
//!   whether a file is. Files are opened through [`Options`], so the permission is part of creation itself on
//!   every OS (on ACL systems it is passed in the creating call).
//! - [`sync_dir`] / [`sync_file`]: make a new directory entry or file contents durable across a power cut;
//!   failure is reported.
//! - [`replace`]: an atomic rename over the target, which is always either the old file or the new one.
//! - [`land_new`]: give a written file its name without ever replacing an existing one: a hard link, else a
//!   non-replacing rename ([`rename_new`]), else an exclusive create to claim the name and a rename over that
//!   claim; whichever the volume supports first.
//! - [`system_proxies`]: read-only system proxy settings ([`proxy`]: macOS network configuration, Windows as the
//!   system answers them, Linux desktop settings files, else the environment).
//! - [`spawn`] / [`LockFile`]: a kernel lock opened as a [`LockFile`] is listed while open, and every child
//!   closes the listed ones before it runs (a lock released while a child held a copy would stay held by it).
//! - [`user_home`] / [`app_data_dir`] / [`machine`]: the user's home, where the app keeps machine data by
//!   default, and where the machine directory is (shared by the app and the CLI).
//! - [`cli_path`]: "enable command line": put the bundled CLI on the shell's command path and remove it,
//!   requesting administrator rights only through the OS's own dialog.
//! - [`door`]: a local IPC endpoint for other processes of the same user (a socket file on unix, a named pipe on
//!   Windows), with the peer's user checked on both sides.
//! - [`lock_now`] / [`lock_wait`]: the single-writer lock, a kernel lock on an open descriptor (released when the
//!   process ends, so no lock is ever stale): the app and the command line take a home's lock and the key
//!   vault's lock through these.
//! - [`put_at`] / [`rename_over`] / [`open_owner_only`]: replace a file whole and durably: written owner-only under
//!   a temporary name, synced, renamed over its place in one step, the directory synced; a write that does not
//!   land takes its temporary file with it.
//! - [`backdrop`] / [`backdrop_available`] / [`backdrop_appearance`]: the system's own material under a window
//!   (macOS 26 glass, earlier macOS vibrancy, Windows 11 Mica); elsewhere the app paints its own tinted ground.
//! - [`wipe`] / [`wipe_string`] / [`wipe_string_tail`]: zero secret bytes in place with writes the compiler
//!   cannot drop. The one place outside the platform implementations that writes through a raw pointer: every
//!   other crate of the product forbids `unsafe` and wipes through these.
//!
//! The OS split lives only here: unix (macOS, Linux) in `unix.rs`, Windows in `windows.rs`.
//!
//! Every error names the operation and path before the OS message (`replacing rename /a → /b: Permission
//! denied`) and keeps the original `ErrorKind`, so callers matching `AlreadyExists`, `NotFound` or `Unsupported`
//! still work ([`named`]).

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as imp;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
use windows as imp;

pub mod cli_path;
pub mod door;
pub mod machine;
pub mod proxy;
pub use proxy::{from_env as proxies_from_env, from_windows as proxies_from_windows, place_of as proxy_place_of, Place as ProxyPlace, Proxies};

#[cfg(not(any(unix, windows)))]
compile_error!("zikaron-os has an implementation for unix and one for Windows; this system has neither");

use std::path::Path;

/// Wrap an OS error with `what` (the operation) and `subject` (the path or source) before the OS message,
/// keeping its kind. Every public function here reports errors through this.
fn named(what: &str, subject: impl std::fmt::Display, e: std::io::Error) -> std::io::Error {
    std::io::Error::new(e.kind(), format!("{what} {subject}: {e}"))
}

/// [`named`] for an operation on two paths (a rename, a link).
fn named_two(what: &str, from: &Path, to: &Path, e: std::io::Error) -> std::io::Error {
    named(what, format!("{} \u{2192} {}", from.display(), to.display()), e)
}

/// The entropy source's name, for error messages ("cannot read <source>").
pub const ENTROPY_SOURCE: &str = imp::ENTROPY_SOURCE;

/// `n` bytes from the system entropy source; an error explains why it could not be read.
pub fn random(n: usize) -> std::io::Result<Vec<u8>> {
    let mut b = vec![0u8; n];
    fill_random(&mut b)?;
    Ok(b)
}

/// Fill `buf` from the system entropy source.
pub fn fill_random(buf: &mut [u8]) -> std::io::Result<()> {
    entropy_pretended().and_then(|()| imp::fill_random(buf)).map_err(|e| named("entropy", ENTROPY_SOURCE, e))
}

static ENTROPY_PRETENDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Treat the entropy source as unreadable from now on in this process (`false`: normal). For testing the
/// [`fill_random`] error path; production never calls it.
pub fn pretend_entropy_fails(on: bool) {
    ENTROPY_PRETENDED.store(on, std::sync::atomic::Ordering::SeqCst);
}

fn entropy_pretended() -> std::io::Result<()> {
    if ENTROPY_PRETENDED.load(std::sync::atomic::Ordering::SeqCst) {
        // A kind other than `Other`, so a wrapper that dropped the OS error kind would be caught.
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "cannot be read here (pretended)"));
    }
    Ok(())
}

/// File open options: the modes this workspace uses (named as in `std::fs::OpenOptions`), plus whether a created
/// file is owner-only ([`owner_only`]). Opening goes through the OS implementation, so owner-only is applied in
/// the creating call, never afterwards.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub read: bool,
    pub write: bool,
    /// Create the file if missing (an existing one is opened).
    pub create: bool,
    /// Create the file; an existing one is refused (`AlreadyExists`).
    pub create_new: bool,
    /// Truncate an existing file to zero bytes.
    pub truncate: bool,
    /// A created file is readable and writable by its owner only.
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

    /// Open (or create) the file at `path` with these options.
    pub fn open(&self, path: &Path) -> std::io::Result<std::fs::File> {
        imp::open(self, path).map_err(|e| named("open", path.display(), e))
    }
}

/// Make files created with these options readable and writable by their owner only, from the moment they exist
/// (applied in the creating call). An existing file keeps its permissions.
pub fn owner_only(o: &mut Options) -> &mut Options {
    o.owner_only = true;
    o
}

/// Whether the file at `path` is readable and writable by its owner only.
pub fn is_owner_only(path: &Path) -> std::io::Result<bool> {
    imp::is_owner_only(path).map_err(|e| named("owner-only check", path.display(), e))
}

/// Make the entries of the directory at `path` durable after `landed` (a path in it) was created, linked or
/// renamed there. On an OS that cannot sync a directory, `landed` itself is synced instead, which persists its
/// name.
pub fn sync_dir(path: &Path, landed: &Path) -> std::io::Result<()> {
    if sync_pretended_at(path) {
        return Err(named("directory sync", path.display(), std::io::Error::other("cannot be synced here (pretended)")));
    }
    imp::sync_dir(path, landed).map_err(|e| named("directory sync", path.display(), e))
}

static SYNC_PRETENDED: std::sync::Mutex<Option<std::path::PathBuf>> = std::sync::Mutex::new(None);

/// Make syncing exactly the directory `dir` fail from now on in this process (`None`: none). For testing
/// non-durable writes; production never calls it. Scoped to one directory so concurrent tests are unaffected.
pub fn pretend_sync_fails_at(dir: Option<std::path::PathBuf>) {
    *SYNC_PRETENDED.lock().unwrap_or_else(|e| e.into_inner()) = dir;
}

fn sync_pretended_at(path: &Path) -> bool {
    SYNC_PRETENDED.lock().map(|g| g.as_deref() == Some(path)).unwrap_or(false)
}

/// Make the file at `path` readable by other accounts (only its group when `group_only`). For testing "a file
/// other accounts can read"; production never calls it. Unsupported on ACL-based systems.
pub fn open_to_others(path: &Path, group_only: bool) -> std::io::Result<()> {
    imp::open_to_others(path, group_only).map_err(|e| named("open to others", path.display(), e))
}

/// Make the contents of the file at `path` durable (for a file written and closed elsewhere).
pub fn sync_file(path: &Path) -> std::io::Result<()> {
    imp::sync_file(path).map_err(|e| named("file sync", path.display(), e))
}

/// Atomically rename `from` over `to`, replacing an existing `to`.
pub fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    imp::replace(from, to).map_err(|e| named_two("replacing rename", from, to, e))
}

/// The three methods [`land_new`] uses; tests can mark each as unsupported ([`pretend_unsupported`]).
pub mod way {
    /// A hard link.
    pub const LINK: u8 = 1;
    /// The non-replacing rename.
    pub const RENAME_NEW: u8 = 2;
    /// The exclusive create that claims the name.
    pub const CLAIM: u8 = 4;
}

static PRETENDED: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Treat the methods in `ways` (a union of [`way`] bits) as unsupported from now on in this process (`0`:
/// none). For testing each step of [`land_new`]; production never calls it.
pub fn pretend_unsupported(ways: u8) {
    PRETENDED.store(ways, std::sync::atomic::Ordering::SeqCst);
}

fn pretended(w: u8, what: &str) -> std::io::Result<()> {
    if PRETENDED.load(std::sync::atomic::Ordering::SeqCst) & w != 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::Unsupported, format!("{what} is not supported here (pretended)")));
    }
    Ok(())
}

/// Which method [`land_new`] used.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Landed {
    /// A hard link: the file has both names now (the caller removes the temporary one).
    Linked,
    /// Moved: the file has only the new name.
    Moved,
}

/// Give the file at `tmp` the name `target` without ever replacing an existing file, using the first method the
/// volume supports:
/// 1. a hard link (the file then has both names);
/// 2. a non-replacing rename ([`rename_new`]);
/// 3. an exclusive create of `target` (an empty file claiming the name), then `tmp` renamed over that claim.
///
/// An existing `target` at any step is `AlreadyExists` and nothing moves. An unsupported method falls through
/// to the next; if none is supported the error lists all three (with the last one's kind). Between claim and
/// rename in method 3, `target` is an empty file; an interruption leaves it, and callers must treat an empty
/// file as an unfinished landing (entries are never empty). If the rename fails after the claim, the claim is
/// removed while still empty.
pub fn land_new(tmp: &Path, target: &Path) -> std::io::Result<Landed> {
    let not_supported = |e: &std::io::Error| e.kind() != std::io::ErrorKind::AlreadyExists;
    let link = pretended(way::LINK, "a hard link").and_then(|()| std::fs::hard_link(tmp, target)).map_err(|e| named_two("hard link", tmp, target, e));
    let link = match link {
        Ok(()) => return Ok(Landed::Linked),
        Err(e) if !not_supported(&e) => return Err(e),
        Err(e) => e,
    };
    let moved = match pretended(way::RENAME_NEW, "a rename that never replaces").map_err(|e| named_two("rename that never replaces", tmp, target, e)).and_then(|()| rename_new(tmp, target)) {
        Ok(()) => return Ok(Landed::Moved),
        Err(e) if !not_supported(&e) => return Err(e),
        Err(e) => e,
    };
    // Method 3. If it also fails, the error lists all three methods' errors (with the last one's kind).
    let claim = pretended(way::CLAIM, "an exclusive create").map_err(|e| named("exclusive create", target.display(), e)).and_then(|()| Options::new().write(true).create_new(true).open(target).map(drop));
    if let Err(e) = claim {
        if !not_supported(&e) {
            return Err(e);
        }
        return Err(std::io::Error::new(e.kind(), format!("none of the three ways gave {} its name: {link}; {moved}; {e}", target.display())));
    }
    if let Err(e) = replace(tmp, target) {
        if std::fs::symlink_metadata(target).map(|m| m.is_file() && m.len() == 0).unwrap_or(false) {
            let _ = std::fs::remove_file(target);
        }
        return Err(e);
    }
    Ok(Landed::Moved)
}

/// Atomically rename `from` to `to` only if `to` does not exist; otherwise `AlreadyExists` and both stay as
/// they were. A volume that cannot do this returns `Unsupported` or the OS's own error.
pub fn rename_new(from: &Path, to: &Path) -> std::io::Result<()> {
    imp::rename_new(from, to).map_err(|e| named_two("rename that never replaces", from, to, e))
}

/// Serializes starting children against opening and closing kernel locks. A starting child holds a copy of
/// every open descriptor until it execs, and a `flock` released while such a copy lives stays held by it, so
/// this process could later find its own home locked and open it read-only. A [`LockFile`] is therefore listed
/// while open, and every child started through [`spawn`] closes the listed descriptors before it runs. Locks
/// open and close under the write side and children start under the read side, so no listed lock changes
/// while a child is starting.
static CHILDREN: std::sync::RwLock<()> = std::sync::RwLock::new(());

/// How many kernel locks this process may hold open at once ([`LockFile`]); one more is refused by name. The
/// app holds one per open home.
pub const LOCKS_MAX: usize = 64;

/// The descriptors of the kernel locks this process holds open, a free slot `-1`. A starting child reads it
/// between fork and exec, where only atomics and `close` may be used: no allocation, no lock.
pub(crate) static LISTED: [std::sync::atomic::AtomicI64; LOCKS_MAX] = [const { std::sync::atomic::AtomicI64::new(-1) }; LOCKS_MAX];

/// Start a child process (see [`CHILDREN`]); the only way this process and its tests start one. On unix the
/// child closes every listed lock descriptor before exec, and this returns only once the child has exec'd (or
/// failed to). On Windows no child holds a copy at all: [`Options::open`] creates handles that are not
/// inheritable (as the standard library's files are), and a child is given only inheritable ones.
pub fn spawn(cmd: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    let _turn = CHILDREN.read().unwrap_or_else(|e| e.into_inner());
    imp::spawn(cmd)
}

/// A kernel lock's file, open while this value lives and listed so no child holds a copy of it ([`spawn`]);
/// once dropped, the next locker finds it free. More than [`LOCKS_MAX`] at once is refused and opens nothing.
pub struct LockFile {
    file: std::mem::ManuallyDrop<std::fs::File>,
    slot: usize,
}

impl LockFile {
    /// Open the lock file at `path` with these options and list it.
    pub fn open(o: &Options, path: &Path) -> std::io::Result<LockFile> {
        let _turn = CHILDREN.write().unwrap_or_else(|e| e.into_inner());
        let Some(slot) = LISTED.iter().position(|s| s.load(std::sync::atomic::Ordering::SeqCst) < 0) else {
            return Err(named("kernel lock", path.display(), std::io::Error::other(format!("more than {LOCKS_MAX} held at once"))));
        };
        let file = o.open(path)?;
        LISTED[slot].store(imp::listed_id(&file), std::sync::atomic::Ordering::SeqCst);
        Ok(LockFile { file: std::mem::ManuallyDrop::new(file), slot })
    }

    /// The open file, to lock and to read.
    pub fn file(&self) -> &std::fs::File {
        &self.file
    }
}

impl Drop for LockFile {
    fn drop(&mut self) {
        let _turn = CHILDREN.write().unwrap_or_else(|e| e.into_inner());
        LISTED[self.slot].store(-1, std::sync::atomic::Ordering::SeqCst);
        // SAFETY: dropped once, here, and never used again.
        unsafe { std::mem::ManuallyDrop::drop(&mut self.file) };
    }
}

/// The environment variable the system sets for the user's home (unix `HOME`, Windows `USERPROFILE`). The home
/// is read from it first, and a parent process can pass a home to a child through it.
pub const HOME_VAR: &str = imp::HOME_VAR;

/// The user's home directory: [`HOME_VAR`] when set and not empty, otherwise the OS's answer (Windows: the
/// profile known folder; unix: none); `None` if neither. The path is returned even if it does not exist or is
/// not writable; whoever writes there reports the failure.
pub fn user_home() -> Option<std::path::PathBuf> {
    home_from(std::env::var_os(HOME_VAR), imp::home_dir)
}

/// [`user_home`]'s rule over a given value of [`HOME_VAR`] and the OS's own answer.
fn home_from(var: Option<std::ffi::OsString>, otherwise: impl FnOnce() -> Option<std::path::PathBuf>) -> Option<std::path::PathBuf> {
    var.filter(|h| !h.is_empty()).map(std::path::PathBuf::from).or_else(otherwise)
}

/// Whether two paths name the same folder: as written, or once both are resolved by the system (case, short
/// names, links, a trailing separator and `.` steps do not make another folder). A path that does not resolve
/// (not there, not readable) is the same only as itself as written.
pub fn same_folder(a: &Path, b: &Path) -> bool {
    a == b || matches!((std::fs::canonicalize(a), std::fs::canonicalize(b)), (Ok(x), Ok(y)) if x == y)
}

/// Default location of the app's machine data, given the user's home directory (or a stand-in): macOS and Linux
/// under the home directory; Windows in the user's local application data folder. The pointer to the machine
/// directory lives there too ([`machine`]).
pub fn app_data_dir(user_home: &Path) -> std::path::PathBuf {
    imp::app_data_dir(user_home)
}

/// The current system proxy settings (read only; `None` if they could not be read).
pub fn system_proxies() -> Option<Proxies> {
    proxy::system()
}


// ───────────────────────── Wiping secrets ─────────────────────────

/// Zero `b` in place. Volatile writes and a fence, so the compiler cannot drop them as dead stores before the
/// memory is freed or reused.
pub fn wipe(b: &mut [u8]) {
    for x in b.iter_mut() {
        // SAFETY: `x` is a valid, aligned, exclusive reference to one byte.
        unsafe { std::ptr::write_volatile(x, 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

/// Zero a string's whole allocation (its text and the capacity past it, where removed characters were) and
/// leave it empty.
pub fn wipe_string(s: &mut String) {
    // SAFETY: only zeros are written, within the allocation; zero bytes are valid UTF-8, and the length is then
    // set to zero.
    let v = unsafe { s.as_mut_vec() };
    let (p, cap) = (v.as_mut_ptr(), v.capacity());
    for i in 0..cap {
        unsafe { std::ptr::write_volatile(p.add(i), 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
    v.clear();
}

/// After a string shrank from `old` bytes to its length now, zero the bytes it let go of (`[len, old)`, within
/// capacity, so nothing moves); its text stays.
pub fn wipe_string_tail(s: &mut String, old: usize) {
    let new = s.len();
    // SAFETY: only zeros are written, only within capacity and past the length, and the length stays.
    let v = unsafe { s.as_mut_vec() };
    let p = v.as_mut_ptr();
    for i in new..old.min(v.capacity()) {
        unsafe { std::ptr::write_volatile(p.add(i), 0) };
    }
    std::sync::atomic::compiler_fence(std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
mod tests {

    /// An entry is going when it is not there: a file or a folder that is there is not going, the same names once
    /// removed are. (A Windows entry being deleted, its last handle still open, is the third form; the walks of a
    /// live folder meet it in the app's tests.)
    #[test]
    fn an_entry_is_going_when_it_is_not_there() {
        let dir = std::env::temp_dir().join(format!("zk-os-going-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a folder");
        let file = dir.join("file");
        std::fs::write(&file, b"x").expect("a file");
        assert!(!going(&file), "a file that is there");
        assert!(!going(&dir), "a folder that is there");
        std::fs::remove_file(&file).expect("removed");
        assert!(going(&file), "a file removed");
        std::fs::remove_dir_all(&dir).expect("removed");
        assert!(going(&dir), "a folder removed");
        assert!(going(&dir.join("never")), "under a folder that is not there");
    }

    /// The three wipes leave zeros where the secret was: a slice whole; a string's whole allocation, past its
    /// length too, and the string empty; a string's let-go tail, its text kept.
    #[test]
    fn a_wipe_leaves_zeros_where_the_secret_was() {
        let mut b = *b"secret bytes";
        wipe(&mut b);
        assert_eq!(b, [0u8; 12]);
        let mut s = String::with_capacity(32);
        s.push_str("passphrase words here");
        let (p, cap) = (s.as_ptr(), s.capacity());
        wipe_string(&mut s);
        assert!(s.is_empty());
        assert_eq!(s.as_ptr(), p, "the allocation stays where it was");
        // SAFETY: reading the string's own allocation, which it still owns.
        assert!((0..cap).all(|i| unsafe { *p.add(i) } == 0), "the whole allocation is zero");
        let mut t = String::from("keep this, drop that");
        let old = t.len();
        t.truncate(9);
        wipe_string_tail(&mut t, old);
        assert_eq!(t, "keep this");
        let q = t.as_ptr();
        // SAFETY: as above, within the capacity the string still owns.
        assert!((9..old).all(|i| unsafe { *q.add(i) } == 0), "the tail let go is zero");
    }

    use super::*;

    fn scratch(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("zikaron-os-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("scratch");
        d
    }

    /// The home comes from [`HOME_VAR`] when set and not empty, else from the OS, else `None`.
    #[test]
    fn the_home_is_the_systems_own_variable_first() {
        let os = || Some(std::path::PathBuf::from("from-the-os"));
        let none = || None;
        assert_eq!(home_from(Some("/a/home".into()), os), Some("/a/home".into()), "set: the variable");
        assert_eq!(home_from(Some("/not/there/yet".into()), none), Some("/not/there/yet".into()), "set to a folder not there: as given");
        assert_eq!(home_from(None, os), Some("from-the-os".into()), "unset: the OS");
        assert_eq!(home_from(Some("".into()), os), Some("from-the-os".into()), "empty: the OS");
        assert_eq!(home_from(None, none), None, "neither: none");
        assert_eq!(home_from(Some("".into()), none), None);
    }

    /// A folder matches itself however it is spelled; another folder, or a missing path spelled otherwise, does not.
    #[test]
    fn a_folder_is_the_same_however_it_is_spelled() {
        let d = scratch("same");
        let other = d.join("other");
        std::fs::create_dir_all(&other).expect("other");
        let trailing = std::path::PathBuf::from(format!("{}{}", d.display(), std::path::MAIN_SEPARATOR));
        assert!(same_folder(&d, &d) && same_folder(&d, &trailing) && same_folder(&d, &d.join(".")) && same_folder(&d, &other.join("..")));
        assert!(!same_folder(&d, &other), "another folder");
        let gone = d.join("gone");
        assert!(same_folder(&gone, &gone), "not there: itself as written");
        assert!(!same_folder(&gone, &gone.join("x").join("..")), "not there, spelled otherwise: not resolved, not the same");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The lock list, and a child, are process-wide: these tests run one at a time.
    static LOCKS_TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn listed() -> Vec<i64> {
        LISTED.iter().map(|s| s.load(std::sync::atomic::Ordering::SeqCst)).filter(|d| *d >= 0).collect()
    }

    fn lock_at(p: &Path) -> LockFile {
        LockFile::open(Options::new().read(true).write(true).create(true), p).expect("a lock file")
    }

    /// Each lock is listed while open and unlisted when dropped; dropping one frees only its own slot.
    #[test]
    fn a_lock_is_listed_while_it_is_open() {
        let _t = LOCKS_TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("listed");
        let before = listed();
        let one = lock_at(&d.join("one"));
        let id = imp::listed_id(one.file());
        assert_eq!(listed().iter().filter(|x| **x == id).count(), 1, "one: listed once");
        let many: Vec<LockFile> = (0..5).map(|i| lock_at(&d.join(format!("l{i}")))).collect();
        let ids: Vec<i64> = many.iter().map(|l| imp::listed_id(l.file())).collect();
        assert!(ids.iter().all(|i| listed().contains(i)), "several: each listed");
        drop(one);
        assert!(!listed().contains(&id) || ids.contains(&id), "let go: unlisted");
        drop(many);
        assert_eq!(listed(), before, "all let go: the list as it was");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// One lock more than [`LOCKS_MAX`] at once is refused by name and opens nothing; the others stay listed.
    #[test]
    fn one_lock_more_than_the_list_holds_is_refused_by_name() {
        let _t = LOCKS_TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("full");
        let free = LOCKS_MAX - listed().len();
        let held: Vec<LockFile> = (0..free).map(|i| lock_at(&d.join(format!("l{i}")))).collect();
        let over = d.join("over");
        let e = LockFile::open(Options::new().read(true).write(true).create(true), &over).err().expect("refused");
        assert!(e.to_string().starts_with("kernel lock") && e.to_string().contains(&over.display().to_string()), "{e}");
        assert!(!over.exists(), "nothing opened");
        assert_eq!(listed().len(), LOCKS_MAX);
        drop(held);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Dropping a lock while a child is starting waits until the start is done.
    #[test]
    fn a_lock_let_go_while_a_child_starts_waits_for_the_start() {
        let _t = LOCKS_TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("waits");
        let l = lock_at(&d.join("lock"));
        let starting = CHILDREN.read().unwrap_or_else(|e| e.into_inner());
        let gone = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let g = gone.clone();
        let letting = std::thread::spawn(move || {
            drop(l);
            g.store(true, std::sync::atomic::Ordering::SeqCst);
        });
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(!gone.load(std::sync::atomic::Ordering::SeqCst), "held off while the child starts");
        drop(starting);
        letting.join().expect("let go");
        assert!(gone.load(std::sync::atomic::Ordering::SeqCst), "let go once it has started");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[cfg(unix)]
    unsafe extern "C" {
        fn fcntl(fd: i32, cmd: i32, ...) -> i32;
    }

    /// Whether a child started through [`spawn`] has descriptor `fd` open when its program runs.
    #[cfg(unix)]
    fn child_holds(fd: i64) -> bool {
        let out = spawn(std::process::Command::new("/bin/sh").args(["-c", &format!("test -e /dev/fd/{fd} && echo held || echo free")]).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()))
            .and_then(|c| c.wait_with_output())
            .expect("the child runs");
        String::from_utf8_lossy(&out.stdout).trim() == "held"
    }

    /// Made inheritable across exec (close-on-exec cleared), so only [`spawn`] closing it keeps it from a child.
    #[cfg(unix)]
    fn inheritable(f: &std::fs::File) -> i64 {
        use std::os::fd::AsRawFd;
        const F_SETFD: i32 = 2;
        // SAFETY: `fd` is open for the call; clearing the flags only drops close-on-exec.
        assert_eq!(unsafe { fcntl(f.as_raw_fd(), F_SETFD, 0) }, 0);
        i64::from(f.as_raw_fd())
    }

    /// A child never runs holding a listed lock's descriptor, even an inheritable one; unlisted descriptors are
    /// left alone.
    #[cfg(unix)]
    #[test]
    fn a_child_never_runs_holding_a_listed_lock() {
        let _t = LOCKS_TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("child");
        let plain = std::fs::File::create(d.join("plain")).expect("a plain file");
        let plain_fd = inheritable(&plain);
        assert!(child_holds(plain_fd), "a descriptor that is not a lock reaches the child as it would");
        let one = lock_at(&d.join("one"));
        let one_fd = inheritable(one.file());
        assert!(!child_holds(one_fd), "one lock: closed before the child runs");
        let more: Vec<LockFile> = (0..3).map(|i| lock_at(&d.join(format!("m{i}")))).collect();
        let fds: Vec<i64> = more.iter().map(|l| inheritable(l.file())).collect();
        for fd in fds.iter().chain([&one_fd]) {
            assert!(!child_holds(*fd), "several locks: {fd} closed before the child runs");
        }
        drop(more);
        drop(one);
        assert!(child_holds(plain_fd), "none listed: the child starts and is handed the rest as it would");
        let _ = std::fs::remove_dir_all(&d);
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
        // Reopened with and without truncate: contents and owner-only as the options say.
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
        // Never replacing: a new name succeeds, an existing one is refused and both are unchanged.
        let (n, m) = (d.join("n"), d.join("m"));
        std::fs::write(&n, b"new").expect("n");
        rename_new(&n, &m).expect("into an empty place");
        assert_eq!(std::fs::read(&m).expect("m"), b"new");
        std::fs::write(&n, b"other").expect("n again");
        assert_eq!(rename_new(&n, &m).map_err(|e| e.kind()).err(), Some(std::io::ErrorKind::AlreadyExists));
        assert_eq!((std::fs::read(&m).expect("m"), std::fs::read(&n).expect("n")), (b"new".to_vec(), b"other".to_vec()));
        sync_file(&t).expect("the file syncs");
        let nowhere = d.join("nowhere");
        assert!(sync_dir(&nowhere, &nowhere.join("t")).is_err(), "a directory that is not there is said");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Every error names the operation and path before the OS message and keeps the OS error kind: open, the
    /// owner-only check, directory and file sync, replacing and non-replacing rename (both paths).
    #[test]
    fn every_error_names_its_interface_and_path_and_keeps_its_kind() {
        let d = scratch("named");
        let (gone, there) = (d.join("gone"), d.join("there"));
        std::fs::write(&there, b"x").expect("there");
        let nf = std::io::ErrorKind::NotFound;
        let cases: Vec<(&str, std::io::Error, Vec<String>, std::io::ErrorKind)> = vec![
            ("open", Options::new().read(true).open(&gone).err().expect("open"), vec![gone.display().to_string()], nf),
            ("owner-only check", is_owner_only(&gone).err().expect("check"), vec![gone.display().to_string()], nf),
            ("directory sync", sync_dir(&gone, &gone.join("t")).err().expect("dir sync"), vec![gone.display().to_string()], nf),
            ("file sync", sync_file(&gone).err().expect("file sync"), vec![gone.display().to_string()], nf),
            ("replacing rename", replace(&gone, &there).err().expect("replace"), vec![gone.display().to_string(), there.display().to_string()], nf),
            ("rename that never replaces", rename_new(&gone, &d.join("new")).err().expect("rename new"), vec![gone.display().to_string()], nf),
        ];
        for (what, e, paths, kind) in cases {
            let said = e.to_string();
            assert!(said.starts_with(what), "{what}: named first: {said}");
            for p in paths {
                assert!(said.contains(&p), "{what}: the path {p}: {said}");
            }
            assert_eq!(e.kind(), kind, "{what}: the system's kind kept");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Each [`land_new`] method lands the bytes and never replaces an existing file; with none supported it
    /// errors and moves nothing; a failed rename after the claim removes the claim. One test, since the
    /// setting is process-wide.
    #[test]
    fn a_name_is_given_by_the_first_way_the_volume_supports() {
        let d = scratch("land");
        for (ways, landed) in [(0, Landed::Linked), (way::LINK, Landed::Moved), (way::LINK | way::RENAME_NEW, Landed::Moved)] {
            pretend_unsupported(ways);
            let (tmp, target) = (d.join(format!("tmp-{ways}")), d.join(format!("t-{ways}")));
            std::fs::write(&tmp, b"bytes").expect("tmp");
            assert_eq!(land_new(&tmp, &target).expect("lands"), landed, "{ways}");
            assert_eq!(std::fs::read(&target).expect("target"), b"bytes");
            assert_eq!(tmp.exists(), landed == Landed::Linked, "{ways}: the temporary name stays only after a link");
            // Already there, with bytes or empty (an interrupted claim): refused, both unchanged.
            for there in [b"other".to_vec(), Vec::new()] {
                let tmp = d.join(format!("again-{ways}"));
                std::fs::write(&tmp, b"new").expect("tmp");
                std::fs::write(&target, &there).expect("there");
                assert_eq!(land_new(&tmp, &target).map_err(|e| e.kind()).err(), Some(std::io::ErrorKind::AlreadyExists), "{ways}");
                assert_eq!((std::fs::read(&target).expect("t"), std::fs::read(&tmp).expect("tmp")), (there, b"new".to_vec()));
            }
        }
        // No method supported: an error, nothing at the target, the temporary file left to its writer.
        pretend_unsupported(way::LINK | way::RENAME_NEW | way::CLAIM);
        let (tmp, target) = (d.join("tmp-none"), d.join("t-none"));
        std::fs::write(&tmp, b"bytes").expect("tmp");
        let none = land_new(&tmp, &target).err().expect("no way supported");
        assert_eq!(none.kind(), std::io::ErrorKind::Unsupported);
        let said = none.to_string();
        for way in ["hard link", "rename that never replaces", "exclusive create"] {
            assert!(said.contains(way), "each way said in its own words: {said}");
        }
        assert!(said.contains(&target.display().to_string()), "{said}");
        assert!(!target.exists() && tmp.exists());
        // Claim made, then the rename fails (the temporary file is gone): the claim is removed too.
        pretend_unsupported(way::LINK | way::RENAME_NEW);
        let (gone, target) = (d.join("tmp-gone"), d.join("t-gone"));
        assert!(land_new(&gone, &target).is_err());
        assert!(!target.exists(), "an empty claim is not left behind");
        pretend_unsupported(0);
        let _ = std::fs::remove_dir_all(&d);
    }
}

// ───────────────────────── An entry on its way out ─────────────────────────

/// Whether an entry a folder listed is on its way out at this moment, judged by the entry's own state, not by the
/// error a look at it met: it is not there any more, or (Windows) it is being deleted, its last handle not yet
/// closed, so the system opens it as "delete pending" and answers access denied. A walk of a live folder takes
/// such an entry as not there; an entry that is there and will not open is not going, and its trouble stands.
pub fn going(path: &Path) -> bool {
    imp::going(path)
}

// ───────────────────────── The single-writer lock ─────────────────────────

/// Ask the kernel for an exclusive lock on this descriptor, without waiting. True when obtained. The lock is
/// released when the descriptor closes or the process ends, so no lock is ever stale.
pub fn lock_now(file: &std::fs::File) -> bool {
    imp::lock_now(file)
}

/// The same lock, blocking until obtained.
pub fn lock_wait(file: &std::fs::File) -> bool {
    imp::lock_wait(file)
}

// ───────────────────────── A directory set aside whole ─────────────────────────

/// Why a directory was not set aside.
#[derive(Debug)]
pub enum NotAside {
    /// Another process holds this lock file.
    Held(std::path::PathBuf),
    /// The system refused, with the path it names.
    Io(std::path::PathBuf, std::io::Error),
}

/// Rename the directory `from` to the new name `to` beside it, not a byte inside touched. Each of `locks` (missing
/// ones are nobody's) is taken first without waiting; one held elsewhere refuses with nothing done. They are held
/// across the rename, except on Windows, where a file open in the directory makes the rename itself refuse.
pub fn set_aside(from: &Path, to: &Path, locks: &[std::path::PathBuf]) -> Result<(), NotAside> {
    if std::fs::symlink_metadata(to).is_ok() {
        return Err(NotAside::Io(to.to_path_buf(), std::io::Error::from(std::io::ErrorKind::AlreadyExists)));
    }
    let mut held = Vec::new();
    for l in locks {
        let f = match std::fs::File::open(l) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(NotAside::Io(l.clone(), e)),
        };
        if !lock_now(&f) {
            return Err(NotAside::Held(l.clone()));
        }
        held.push(f);
    }
    if cfg!(windows) {
        held.clear();
    }
    let moved = std::fs::rename(from, to).map_err(|e| NotAside::Io(from.to_path_buf(), e));
    drop(held);
    moved
}

// ───────────────────────── Replacing a file whole ─────────────────────────

/// A write that did not land: the system's error, and the path it is said with (the file being written; its
/// directory for creating or syncing that directory). The temporary name is the writer's own business and is
/// never the path said.
#[derive(Debug)]
pub struct Unput {
    pub error: std::io::Error,
    pub at: std::path::PathBuf,
}

/// Creates a new file readable and writable by its owner only (refusing an existing name: `create_new`). The
/// permission is part of the creating call ([`owner_only`]), so there is no moment others may read it.
pub fn open_owner_only(path: &Path) -> std::io::Result<std::fs::File> {
    let mut o = Options::new();
    o.write(true).create_new(true);
    owner_only(&mut o);
    o.open(path)
}

/// Replaces the file `name` in `dir` whole: the bytes go to `tmp` (a name of the caller's one temporary shape)
/// created owner-only, are synced, and the file is renamed over `name` in one step and the directory synced
/// ([`rename_over`]), so the place holds either the old file or the new one, never half of either. A write that
/// does not land removes its temporary file. `dir` is created when missing.
pub fn put_at(dir: &Path, name: &str, tmp: &str, bytes: &[u8]) -> Result<(), Unput> {
    use std::io::Write;
    std::fs::create_dir_all(dir).map_err(|error| Unput { error, at: dir.to_path_buf() })?;
    let tmp = dir.join(tmp);
    let p = dir.join(name);
    let wrote = (|| -> Result<(), Unput> {
        let at = || p.clone();
        let mut f = open_owner_only(&tmp).map_err(|error| Unput { error, at: at() })?;
        f.write_all(bytes).map_err(|error| Unput { error, at: at() })?;
        f.sync_all().map_err(|error| Unput { error, at: at() })?;
        drop(f);
        rename_over(&tmp, &p)
    })();
    if wrote.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    wrote
}

/// Renames `from` over `to` in one step, then syncs the directory `to` is in ([`parent_dir`]) so the new name
/// survives a power cut. A failed rename is said with the target, a failed sync with the directory (the file is
/// in place, but the write is not taken as done).
pub fn rename_over(from: &Path, to: &Path) -> Result<(), Unput> {
    replace(from, to).map_err(|error| Unput { error, at: to.to_path_buf() })?;
    let dir = parent_dir(to);
    sync_dir(dir, to).map_err(|error| Unput { error, at: dir.to_path_buf() })
}

/// The directory `path` is in, the one place that says it: its parent; the working directory `.` when the path
/// names no directory (a bare file name, whose parent is the empty path, or the empty path itself); a root is
/// its own. Every write that syncs the directory it landed in takes the directory from here.
pub fn parent_dir(path: &Path) -> &Path {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        None if path.has_root() => path,
        _ => Path::new("."),
    }
}

// ───────────────────────── The window's backdrop ─────────────────────────

/// What the system draws under a window's own surfaces.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backdrop {
    /// macOS 26 and later: the system's glass (`NSGlassEffectView`).
    Glass,
    /// Earlier macOS: the system's vibrancy (`NSVisualEffectView`).
    Vibrancy,
    /// Windows 11: Mica.
    Mica,
    /// No system material: the app paints a tinted ground of its own.
    Tint,
}

/// A native window as the windowing layer hands it: macOS its content view (`NSView`), Windows its `HWND`.
#[derive(Clone, Copy, Debug)]
pub enum Window {
    AppKit(*mut std::ffi::c_void),
    Win32(isize),
    Other,
}

/// Whether this system can put its own material under a window (macOS; Windows 11 22H2 and later). Asked
/// before the window opens: a window that shows the material is made transparent.
pub fn backdrop_available() -> bool {
    imp::backdrop_available()
}

/// Put the system's material under `w` in the dark or light appearance; asked once per window, when it opens.
/// `follows` says the app follows the system's appearance: the window is then left to the system's (macOS), so
/// the system still tells the app when it changes; otherwise the window takes the app's. Answers what was put
/// (`Tint`: nothing, the app paints its own ground).
pub fn backdrop(w: Window, dark: bool, follows: bool) -> Backdrop {
    imp::backdrop(w, dark, follows)
}

/// Follow the app's light or dark appearance, and whether it follows the system's, on a window's material.
pub fn backdrop_appearance(w: Window, dark: bool, follows: bool) {
    imp::backdrop_appearance(w, dark, follows)
}

/// Whether the system asks apps to reduce motion (macOS: Accessibility › Display › Reduce motion; Windows:
/// "Animation effects" off; Linux: GNOME's animations off). The window asks every few seconds.
pub fn reduce_motion() -> bool {
    imp::reduce_motion()
}
