//! A ledger directory: atomic append, refusal to overwrite, strict and lenient reads, size cap, sweep, layout
//! report.
//!
//! An append has three stages, [`LedgerDir::stage`] then [`Staged::link`] then [`Linked::seal`]: bytes go to
//! a temporary file and are fsynced, a hard link lands them on the entry name, then the directory is fsynced
//! and the temporary file removed. A cut at any point leaves a valid archive:
//!
//! | cut | on disk | strict read |
//! |---|---|---|
//! | before write | nothing | unchanged |
//! | during write | a temporary file | unchanged; the sweep can clear it |
//! | after write | the entry and a leftover temporary file | includes the entry; the sweep can clear it |
//!
//! [`LedgerDir::append`] runs the three stages in one call.
//!
//! Landing uses `link`, because `rename` silently replaces a file of the same name. With `link`, an existing
//! name returns `EEXIST`; the existing bytes are read back: same bytes is idempotent, different bytes is
//! refused with nothing changed.
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
/// 16 MiB: the largest entry the law core accepts (`HARNESS.md`: a candidate accepts every input of sixteen
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
            fs::create_dir_all(&root).map_err(|_| Trouble::of(Code::Io))?;
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
        for round in 0..8u32 {
            let tmp_name = layout::tmp_file_name(&nonce(round));
            let Some(tmp) = layout::path_of(&self.root, &tmp_name) else {
                return Err(Trouble::of(Code::BadName));
            };
            match fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
                Ok(mut f) => {
                    if f.write_all(bytes).is_err() || f.sync_all().is_err() {
                        let _ = fs::remove_file(&tmp);
                        return Err(Trouble::of(Code::Io));
                    }
                    return Ok(Staged { dir: self, tmp, target, bytes });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    last = Trouble::of(Code::Io);
                    continue;
                }
                Err(_) => return Err(Trouble::of(Code::Io)),
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
                    Kind::Entry if it.is_file => named.push((name, it.path)),
                    // An entry-shaped name that is not a regular file was not landed by this crate.
                    Kind::Entry => unaccounted.push(name),
                    Kind::Tmp if it.is_file => {}
                    Kind::Tmp => unaccounted.push(name),
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
                Kind::Entry if it.is_file => named.push((name, it.path)),
                Kind::Entry => note_skip(&mut out.skipped, &name, Why::NotAFile),
                // A temporary-shaped name that is not a regular file is someone else's.
                Kind::Tmp if it.is_file => note_skip(&mut out.skipped, &name, Why::InFlight),
                Kind::Tmp => note_skip(&mut out.skipped, &name, Why::NotAFile),
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
    stored: Stored,
}

impl<'a> Staged<'a> {
    /// Stage two: land by hard link. An existing name is left alone and settled by [`settle_existing`].
    pub fn link(self) -> Result<Linked<'a>, Trouble> {
        match fs::hard_link(&self.tmp, &self.target) {
            Ok(()) => Ok(Linked { dir: self.dir, tmp: self.tmp, stored: Stored::Written }),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let stored = self.dir.settle_existing(&self.target, self.bytes);
                // Idempotent or refused, this write's temporary file does not stay: it never landed.
                let _ = fs::remove_file(&self.tmp);
                Ok(Linked { dir: self.dir, tmp: self.tmp, stored: stored? })
            }
            Err(_) => {
                let _ = fs::remove_file(&self.tmp);
                Err(Trouble::of(Code::Io))
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
    /// temporary name is the entry's own file under its other name (the link), so a system that syncs the file
    /// in place of the directory is handed it.
    pub fn seal(self) -> Result<Stored, Trouble> {
        zikaron_os::sync_dir(self.dir.root(), &self.tmp).map_err(|_| Trouble::of(Code::Io))?;
        let _ = fs::remove_file(&self.tmp);
        Ok(self.stored)
    }

    pub fn stored(&self) -> Stored {
        self.stored
    }
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
    Err(Trouble { code: Code::Unaccounted, names: std::mem::take(unaccounted), size: None, cap: None })
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
