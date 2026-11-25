use crate::codes::{Code, Trouble, Why};
use crate::layout::{self, EntryName, Kind};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const ENTRY_MAX: usize = 16 * 1024 * 1024;

#[derive(Debug)]
pub struct LedgerDir {
    root: PathBuf,
    cap: usize,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stored {
    Written,
    AlreadyThere,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Pile {
    pub items: Vec<Vec<u8>>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Skip {
    pub name: String,
    pub why: Why,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Survey {
    pub items: Vec<Vec<u8>>,
    pub skipped: Vec<Skip>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Sweep {
    pub swept: usize,
    pub kept: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Layout {
    pub entries: usize,
    pub tmp: usize,
    pub dirs: usize,
    pub foreign: usize,
}

impl LedgerDir {
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

    pub fn append(&self, name: &EntryName, bytes: &[u8]) -> Result<Stored, Trouble> {
        self.stage(name, bytes)?.link()?.seal()
    }

    pub fn stage<'a>(&'a self, name: &EntryName, bytes: &'a [u8]) -> Result<Staged<'a>, Trouble> {
        crate::trace::mark(crate::trace::A1);
        if !within_cap(bytes.len(), self.cap) {
            return Err(Trouble::too_large(layout::entry_file_name(name), bytes.len(), self.cap));
        }
        let target = layout::path_of(&self.root, &layout::entry_file_name(name))
            .ok_or_else(|| Trouble::of(Code::BadName))?;
        let mut last = Trouble::of(Code::Io);
        let said_of = |what: &str, tmp: &Path, e: &std::io::Error| Trouble::io(&format!("{what} {}", tmp.display()), e);
        for round in 0..8u32 {
            let tmp_name = layout::tmp_file_name(&nonce(round));
            let Some(tmp) = layout::path_of(&self.root, &tmp_name) else {
                return Err(Trouble::of(Code::BadName));
            };
            let mut o = zikaron_os::Options::new();
            o.write(true).create_new(true);
            zikaron_os::owner_only(&mut o);
            match o.open(&tmp) {
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

    pub fn pile(&self) -> Result<Pile, Trouble> {
        crate::trace::mark(crate::trace::A1);
        let mut named: Vec<(String, PathBuf)> = Vec::new();
        let mut unaccounted: Vec<String> = Vec::new();
        for it in self.list()? {
            match it.name {
                None => unaccounted.push(it.shown),
                Some(name) => match layout::classify(&name) {
                    Kind::Entry if it.is_file && it.len == 0 => {}
                    Kind::Entry if it.is_file => named.push((name, it.path)),
                    Kind::Entry => unaccounted.push(name),
                    Kind::Tmp if it.is_file => {}
                    Kind::Tmp => unaccounted.push(name),
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
        if !within_cap(bytes.len(), self.cap) {
            return Err(Trouble::too_large(name, bytes.len(), self.cap));
        }
        Ok(bytes)
    }

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

pub struct Staged<'a> {
    dir: &'a LedgerDir,
    tmp: PathBuf,
    target: PathBuf,
    bytes: &'a [u8],
}

pub struct Linked<'a> {
    dir: &'a LedgerDir,
    tmp: PathBuf,
    landed: PathBuf,
    stored: Stored,
}

impl<'a> Staged<'a> {
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
                let _ = fs::remove_file(&self.tmp);
                Ok(Linked { dir: self.dir, landed: self.tmp.clone(), tmp: self.tmp, stored: stored? })
            }
            Err(e) => {
                let _ = fs::remove_file(&self.tmp);
                Err(Trouble::io("", &e))
            }
        }
    }

    pub fn abandon(self) {
        let _ = fs::remove_file(&self.tmp);
    }

    pub fn tmp_path(&self) -> &Path {
        &self.tmp
    }
}

impl<'a> Linked<'a> {
    pub fn seal(self) -> Result<Stored, Trouble> {
        zikaron_os::sync_dir(self.dir.root(), &self.landed).map_err(|e| Trouble::io("", &e))?;
        if self.landed == self.tmp {
            let _ = fs::remove_file(&self.tmp);
        }
        Ok(self.stored)
    }

    pub fn stored(&self) -> Stored {
        self.stored
    }
}

pub fn unfinished(path: &Path) -> bool {
    fs::symlink_metadata(path).map(|m| m.is_file() && m.len() == 0).unwrap_or(false)
}

impl LedgerDir {
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

pub fn identical(existing: &[u8], incoming: &[u8]) -> bool {
    existing == incoming
}

pub fn within_cap(len: usize, cap: usize) -> bool {
    len <= cap
}

pub fn account_for(unaccounted: &mut Vec<String>) -> Result<(), Trouble> {
    if unaccounted.is_empty() {
        return Ok(());
    }
    unaccounted.sort();
    unaccounted.dedup();
    Err(Trouble { code: Code::Unaccounted, names: std::mem::take(unaccounted), size: None, cap: None, said: None })
}

pub fn note_skip(out: &mut Vec<Skip>, name: &str, why: Why) {
    out.push(Skip { name: name.to_string(), why });
}

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
