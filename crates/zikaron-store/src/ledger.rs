//! A ledger directory: atomic append, refusal to overwrite, strict and lenient reads, size cap, sweep, layout
//! report.
//!
//! An append has three stages, [`LedgerDir::stage`] then [`Staged::link`] then [`Linked::seal`]: bytes go to
//! a temporary file and are fsynced; they land on the entry name without replacing anything
//! (`zikaron_os::land_new`: a hard link, else a non-replacing rename, else an exclusive claim of the name
//! renamed over); then the directory is fsynced and any temporary name left by the link is removed. A cut at
//! any point leaves a valid archive:
//!
//! | cut | on disk | strict read |
//! |---|---|---|
//! | before write | nothing | unchanged |
//! | during write | a temporary file | unchanged; the sweep can clear it |
//! | between a claim and its rename | an empty file under the entry's name, and the temporary file | unchanged (the empty file is a write in flight); the next landing of that entry takes it over |
//! | after write | the entry and (after a link) a leftover temporary file | includes the entry; the sweep can clear it |
//!
//! [`LedgerDir::append`] runs the three stages in one call.
//!
//! Landing never uses a plain `rename`, which silently replaces a file of the same name. An existing name is
//! `AlreadyExists`; the existing bytes are read back: same bytes is idempotent, different bytes is refused with
//! nothing changed.
//!
//! The strict read ([`LedgerDir::pile`]) yields the audit pile. A pile missing one entry looks like a
//! complete one downstream and audits differently, so anything the crate cannot account for refuses the whole
//! read and is named. The lenient read ([`LedgerDir::survey`]) accepts any directory and returns every
//! skipped item with its reason.

use crate::codes::{Code, Trouble, Why};
use crate::layout::{self, EntryName, Kind};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Size cap of one entry.
///
/// 16 MiB: the largest entry the core accepts (`HARNESS.md`: a candidate accepts every input of sixteen
/// mebibytes or less). A lower archive cap would refuse a valid entry.
pub const ENTRY_MAX: usize = 16 * 1024 * 1024;

/// A ledger directory. Any directory is a valid archive: opening it reads nothing, writes no marker, changes
/// nothing.
#[derive(Debug)]
pub struct LedgerDir {
    root: PathBuf,
    cap: usize,
}

/// State of the archive after an append. The two states look the same on disk (that is idempotence); the
/// caller learns whether this write created the entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stored {
    Written,
    AlreadyThere,
}

/// What the strict read yields: the audit pile, entry bytes in entry-name byte order.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pile {
    pub items: Vec<Vec<u8>>,
}

/// One item a lenient read skipped.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Skip {
    pub name: String,
    pub why: Why,
}

/// Lenient read result: the pile read plus every skipped item and its reason.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Survey {
    pub items: Vec<Vec<u8>>,
    pub skipped: Vec<Skip>,
}

/// Sweep result: how many were removed, and which temporary-looking non-files were left.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sweep {
    pub swept: usize,
    pub kept: Vec<String>,
}

/// Layout report: how many items of each class the directory holds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Layout {
    pub entries: usize,
    pub tmp: usize,
    pub dirs: usize,
    pub foreign: usize,
}

impl LedgerDir {
    /// Open a directory that exists. Not a directory is refused; its contents are not inspected.
    pub fn open(root: impl Into<PathBuf>) -> Result<LedgerDir, Trouble> {
        crate::trace::mark(crate::trace::A1);
        let root = root.into();
        match fs::metadata(&root) {
            Ok(m) if m.is_dir() => Ok(LedgerDir { root, cap: ENTRY_MAX }),
            Ok(_) => Err(Trouble::of(Code::NotADirectory)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Trouble::of(Code::Absent)),
            Err(_) => Err(Trouble::of(Code::Unreadable)),
        }
    }

    /// Open a directory, creating it and its parents if missing.
    pub fn open_or_create(root: impl Into<PathBuf>) -> Result<LedgerDir, Trouble> {
        let root = root.into();
        if fs::metadata(&root).is_err() {
            fs::create_dir_all(&root).map_err(|e| Trouble::io(&format!("create directory {}", root.display()), &e))?;
        }
        LedgerDir::open(root)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Append in one call: the three stages in a row.
    pub fn append(&self, name: &EntryName, bytes: &[u8]) -> Result<Stored, Trouble> {
        self.stage(name, bytes)?.link()?.seal()
    }

    /// Stage one: bytes into a temporary file, fsynced. Until the link lands, no part of the entry is in the
    /// archive.
    pub fn stage<'a>(&'a self, name: &EntryName, bytes: &'a [u8]) -> Result<Staged<'a>, Trouble> {
        crate::trace::mark(crate::trace::A1);
        if !within_cap(bytes.len(), self.cap) {
            return Err(Trouble::too_large(layout::entry_file_name(name), bytes.len(), self.cap));
        }
        let target = layout::path_of(&self.root, &layout::entry_file_name(name))
            .ok_or_else(|| Trouble::of(Code::BadName))?;
        // The temporary file is opened with `create_new`; on a nonce collision another nonce is tried, so a
        // temporary file always belongs to exactly one write.
        let mut last = Trouble::of(Code::Io);
        let said_of = |what: &str, tmp: &Path, e: &std::io::Error| Trouble::io(&format!("{what} {}", tmp.display()), e);
        for round in 0..8u32 {
            let tmp_name = layout::tmp_file_name(&nonce(round));
            let Some(tmp) = layout::path_of(&self.root, &tmp_name) else {
                return Err(Trouble::of(Code::BadName));
            };
            match fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
                Ok(mut f) => {
                    if let Err(e) = f.write_all(bytes).map_err(|e| said_of("write", &tmp, &e)).and_then(|()| f.sync_all().map_err(|e| said_of("file sync", &tmp, &e))) {
                        let _ = fs::remove_file(&tmp);
                        return Err(e);
                    }
                    return Ok(Staged { dir: self, tmp, target, bytes });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    last = said_of("create", &tmp, &e);
                    continue;
                }
                Err(e) => return Err(said_of("create", &tmp, &e)),
            }
        }
        Err(last)
    }

    /// Strict read: the archive's bytes, which are the audit pile.
    ///
    /// Anything that cannot be accounted for refuses the whole read and is named.
    pub fn pile(&self) -> Result<Pile, Trouble> {
        crate::trace::mark(crate::trace::A1);
        let mut named: Vec<(String, PathBuf)> = Vec::new();
        let mut unaccounted: Vec<String> = Vec::new();
        for it in self.list()? {
            match it.name {
                None => unaccounted.push(it.shown),
                Some(name) => match layout::classify(&name) {
                    // A landing cut between its claim and its rename: a write in flight, not an entry.
                    Kind::Entry if it.is_file && it.len == 0 => {}
                    Kind::Entry if it.is_file => named.push((name, it.path)),
                    // An entry-shaped name that is not a regular file was not landed by this crate.
                    Kind::Entry => unaccounted.push(name),
                    Kind::Tmp if it.is_file => {}
                    Kind::Tmp => unaccounted.push(name),
                    // The system's file beside this crate's names is accounted for: it is the system's.
                    Kind::System if it.is_file => {}
                    Kind::System => unaccounted.push(name),
                    Kind::Foreign => unaccounted.push(name),
                },
            }
        }
        account_for(&mut unaccounted)?;
        named.sort_by(|a, b| a.0.cmp(&b.0));
        let mut items = Vec::with_capacity(named.len());
        for (name, path) in named {
            items.push(self.read_capped(&path, &name)?);
        }
        Ok(Pile { items })
    }

    /// Lenient read: any directory, everything readable is returned, every skip is listed with its reason.
    ///
    /// A directory that cannot be listed is refused as in `pile`; it does not read as empty.
    pub fn survey(&self) -> Result<Survey, Trouble> {
        crate::trace::mark(crate::trace::A1);
        let mut out = Survey { items: Vec::new(), skipped: Vec::new() };
        let listing = self.list()?;
        let mut named: Vec<(String, PathBuf)> = Vec::new();
        for it in listing {
            let Some(name) = it.name else {
                note_skip(&mut out.skipped, &it.shown, Why::NonUtf8Name);
                continue;
            };
            match layout::classify(&name) {
                Kind::Entry if it.is_file && it.len == 0 => note_skip(&mut out.skipped, &name, Why::InFlight),
                Kind::Entry if it.is_file => named.push((name, it.path)),
                Kind::Entry => note_skip(&mut out.skipped, &name, Why::NotAFile),
                // A temporary-shaped name that is not a regular file is someone else's.
                Kind::Tmp if it.is_file => note_skip(&mut out.skipped, &name, Why::InFlight),
                Kind::Tmp => note_skip(&mut out.skipped, &name, Why::NotAFile),
                Kind::System if it.is_file => note_skip(&mut out.skipped, &name, Why::SystemSide),
                Kind::System => note_skip(&mut out.skipped, &name, Why::NotAFile),
                Kind::Foreign => {
                    let why = if it.is_file { Why::ForeignName } else { Why::NotAFile };
                    note_skip(&mut out.skipped, &name, why);
                }
            }
        }
        named.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, path) in named {
            match self.read_capped(&path, &name) {
                Ok(b) => out.items.push(b),
                Err(t) if t.code == Code::TooLarge => note_skip(&mut out.skipped, &name, Why::TooLarge),
                Err(_) => note_skip(&mut out.skipped, &name, Why::Unreadable),
            }
        }
        out.skipped.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Read a named file as is, whatever its shape. Recovery and forensics read foreign layouts this way.
    pub fn read_named(&self, file: &str) -> Result<Vec<u8>, Trouble> {
        crate::trace::mark(crate::trace::A1);
        let path = layout::path_of(&self.root, file).ok_or_else(|| Trouble::of(Code::BadName))?;
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Trouble::of(Code::Absent)),
            Err(_) => Err(Trouble::named(Code::Unreadable, file)),
            Ok(m) if !m.is_file() => Err(Trouble::named(Code::NotAFile, file)),
            Ok(_) => self.read_capped(&path, file),
        }
    }

    /// Sweep: remove only files of this crate's exact temporary shape.
    ///
    /// Temporary-looking items that are not regular files (typically directories) are left and named.
    pub fn sweep(&self) -> Result<Sweep, Trouble> {
        crate::trace::mark(crate::trace::A1);
        let mut out = Sweep { swept: 0, kept: Vec::new() };
        let listing = self.list()?;
        for it in listing {
            let Some(name) = it.name else { continue };
            if layout::is_tmp_file(&name, it.is_file) {
                if fs::remove_file(&it.path).is_ok() {
                    out.swept += 1;
                } else {
                    out.kept.push(name);
                }
            } else if layout::tmp_shaped(&name) {
                out.kept.push(name);
            }
        }
        out.kept.sort();
        Ok(out)
    }

    /// Layout report: count per class (directories counted apart).
    pub fn layout(&self) -> Result<Layout, Trouble> {
        crate::trace::mark(crate::trace::A1);
        let mut r = Layout { entries: 0, tmp: 0, dirs: 0, foreign: 0 };
        let listing = self.list()?;
        for it in listing {
            if it.is_dir {
                r.dirs += 1;
                continue;
            }
            match it.name.as_deref().map(layout::classify) {
                Some(Kind::Entry) if it.is_file && it.len == 0 => r.tmp += 1,
                Some(Kind::Entry) if it.is_file => r.entries += 1,
                Some(Kind::Tmp) if it.is_file => r.tmp += 1,
                _ => r.foreign += 1,
            }
        }
        Ok(r)
    }

    /// Read a file; over the cap it is refused with measured size and cap. Never truncated: a truncated entry
    /// is a different entry.
    fn read_capped(&self, path: &Path, name: &str) -> Result<Vec<u8>, Trouble> {
        let meta = fs::symlink_metadata(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Trouble::of(Code::Absent),
            _ => Trouble::named(Code::Unreadable, name),
        })?;
        let size = meta.len() as usize;
        if !within_cap(size, self.cap) {
            return Err(Trouble::too_large(name, size, self.cap));
        }
        let bytes = fs::read(path).map_err(|_| Trouble::named(Code::Unreadable, name))?;
        // The file may grow while it is read; check the cap again on the bytes read.
        if !within_cap(bytes.len(), self.cap) {
            return Err(Trouble::too_large(name, bytes.len(), self.cap));
        }
        Ok(bytes)
    }

    /// List the directory once for all three readers. A non-UTF-8 name is kept (`name: None`); each reader
    /// decides to refuse or disclose it, and none may drop it.
    fn list(&self) -> Result<Vec<Item>, Trouble> {
        let rd = fs::read_dir(&self.root).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Trouble::of(Code::Absent),
            _ => Trouble::of(Code::Unreadable),
        })?;
        let mut out = Vec::new();
        for e in rd {
            let e = e.map_err(|_| Trouble::of(Code::Unreadable))?;
            let raw = e.file_name();
            let name = raw.to_str().map(str::to_string);
            let shown = raw.to_string_lossy().into_owned();
            // `symlink_metadata`: a symlink counts as itself. Following it would make a copied archive differ
            // from the original.
            let meta = fs::symlink_metadata(e.path()).map_err(|_| Trouble::of(Code::Unreadable))?;
            out.push(Item {
                name,
                shown,
                path: e.path(),
                is_file: meta.is_file(),
                is_dir: meta.is_dir(),
                len: meta.len(),
            });
        }
        Ok(out)
    }
}

struct Item {
    name: Option<String>,
    shown: String,
    path: PathBuf,
    is_file: bool,
    is_dir: bool,
    len: u64,
}

/// Between stage one and the link.
pub struct Staged<'a> {
    dir: &'a LedgerDir,
    tmp: PathBuf,
    target: PathBuf,
    bytes: &'a [u8],
}

/// Between the link and the seal.
pub struct Linked<'a> {
    dir: &'a LedgerDir,
    tmp: PathBuf,
    /// The path handed to systems that sync the file instead of the directory: the temporary name after a hard
    /// link (the same file under another name), or the entry's own name after a non-replacing rename.
    landed: PathBuf,
    stored: Stored,
}

impl<'a> Staged<'a> {
    /// Stage two: land without ever replacing (`zikaron_os::land_new`: a hard link, else a non-replacing rename,
    /// else an exclusive claim of the name renamed over). An existing entry is left alone and settled by
    /// [`settle_existing`]. An empty file under the name is a landing cut between its claim and its rename (an
    /// entry is never empty, and its name derives from its content), and this landing takes it over. A volume
    /// that supports none of the three returns an error.
    pub fn link(self) -> Result<Linked<'a>, Trouble> {
        let landed = match zikaron_os::land_new(&self.tmp, &self.target) {
            Ok(zikaron_os::Landed::Linked) => Ok(self.tmp.clone()),
            Ok(zikaron_os::Landed::Moved) => Ok(self.target.clone()),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && unfinished(&self.target) => {
                zikaron_os::replace(&self.tmp, &self.target).map(|()| self.target.clone())
            }
            Err(e) => Err(e),
        };
        match landed {
            Ok(landed) => Ok(Linked { dir: self.dir, tmp: self.tmp, landed, stored: Stored::Written }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let stored = self.dir.settle_existing(&self.target, self.bytes);
                // Idempotent or refused, this write's temporary file does not stay: it never landed.
                let _ = fs::remove_file(&self.tmp);
                Ok(Linked { dir: self.dir, landed: self.tmp.clone(), tmp: self.tmp, stored: stored? })
            }
            Err(e) => {
                let _ = fs::remove_file(&self.tmp);
                Err(Trouble::io("", &e))
            }
        }
    }

    /// Abandon this write: the temporary file is removed, the archive is unchanged.
    pub fn abandon(self) {
        let _ = fs::remove_file(&self.tmp);
    }

    pub fn tmp_path(&self) -> &Path {
        &self.tmp
    }
}

impl<'a> Linked<'a> {
    /// Stage three: fsync the directory so the link survives a power cut, then remove the temporary file. The
    /// temporary name is the entry's own file under another name, so a system that syncs the file instead of the
    /// directory still gets the entry synced.
    pub fn seal(self) -> Result<Stored, Trouble> {
        zikaron_os::sync_dir(self.dir.root(), &self.landed).map_err(|e| Trouble::io("", &e))?;
        // After a hard link the temporary name goes; after the rename there is none left to remove.
        if self.landed == self.tmp {
            let _ = fs::remove_file(&self.tmp);
        }
        Ok(self.stored)
    }

    pub fn stored(&self) -> Stored {
        self.stored
    }
}

/// Whether the file at an entry's name is a landing cut between its claim and its rename: a regular file with
/// no bytes (an entry is never empty). Every reader passes over it as a write in flight; the next landing of
/// that entry takes it over.
pub fn unfinished(path: &Path) -> bool {
    fs::symlink_metadata(path).map(|m| m.is_file() && m.len() == 0).unwrap_or(false)
}

impl LedgerDir {
    /// The name exists: read back its bytes; same bytes is idempotent, different bytes is refused.
    ///
    /// On refusal the existing entry is left untouched.
    fn settle_existing(&self, target: &Path, bytes: &[u8]) -> Result<Stored, Trouble> {
        let name = target
            .file_name()
            .and_then(|x| x.to_str())
            .unwrap_or_default()
            .to_string();
        let existing = self.read_capped(target, &name)?;
        if identical(&existing, bytes) {
            Ok(Stored::AlreadyThere)
        } else {
            Err(Trouble::named(Code::Conflict, name))
        }
    }
}

/// Whether two byte strings are the same. It decides idempotence: `true` makes a second append a no-op.
pub fn identical(existing: &[u8], incoming: &[u8]) -> bool {
    existing == incoming
}

/// Whether a length is within the cap. Write and read ask this same function.
pub fn within_cap(len: usize, cap: usize) -> bool {
    len <= cap
}

/// Anything unaccounted refuses the whole read and is named in byte order.
///
/// This is what keeps a pile from ever being silently short.
pub fn account_for(unaccounted: &mut Vec<String>) -> Result<(), Trouble> {
    if unaccounted.is_empty() {
        return Ok(());
    }
    unaccounted.sort();
    unaccounted.dedup();
    Err(Trouble { code: Code::Unaccounted, names: std::mem::take(unaccounted), size: None, cap: None, said: None })
}

/// Record one skip with its reason. Every lenient-read skip goes through here.
pub fn note_skip(out: &mut Vec<Skip>, name: &str, why: Why) {
    out.push(Skip { name: name.to_string(), why });
}

/// The 16 hex digits of a temporary-file name: process id, a counter and a monotonic clock mixed.
///
/// Nothing depends on uniqueness: `create_new` catches a collision and another round is tried.
fn nonce(round: u32) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let mut x = 0xcbf29ce484222325u64;
    for v in [
        std::process::id() as u64,
        SEQ.fetch_add(1, Ordering::Relaxed),
        t,
        round as u64,
    ] {
        x ^= v;
        x = x.wrapping_mul(0x100000001b3);
    }
    let mut s = String::with_capacity(layout::TMP_NONCE_LEN);
    for i in 0..layout::TMP_NONCE_LEN {
        let nib = (x >> (60 - 4 * i)) & 0xf;
        s.push(char::from_digit(nib as u32, 16).unwrap_or('0'));
    }
    s
}
