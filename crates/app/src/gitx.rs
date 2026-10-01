//! Read a git repository (the anchoring desk's third input kind). No child process is started.
//!
//! ─── Why not call `git` ───
//!
//! The shipped build may not start any child process (checked by the self-check suite:
//! `the_shipped_app_starts_no_child_process_at_all`), and `git cat-file` is one. So this layer reads the
//! object store on disk itself: loose objects in `objects/xx/yyyy…`, packed ones in `objects/pack/*.idx` and
//! `*.pack`, with the compression layer through [`crate::zlibx`].
//!
//! ─── Recomputable byte for byte ───
//!
//! "The bytes of the HEAD commit object" has a byte-exact definition here: the bytes `git cat-file commit
//! <id>` prints, that is, the object without its `commit <len>\0` header. The anchored content is their
//! sha256 (computed by the core's `cryptox::sha256`; this layer invents no digest). So anyone can
//! independently recompute the same string with `git cat-file commit HEAD | shasum -a 256`.
//!
//! ─── Unreadable says unreadable ───
//!
//! Every point where reading cannot continue returns a named [`Fault`]: not a repository, HEAD unresolved,
//! object missing, decompression failed, each with its own name. Nothing is guessed: a guessed hash would be
//! anchored on chain, and what is on chain cannot be changed back.

use crate::fault::{classify, Fault, Known};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The largest size one object may inflate to. Commit objects are far smaller; this stops corrupt files and
/// compression bombs.
pub const OBJECT_MAX: usize = 64 * 1024 * 1024;

/// The most ancestors counted. When the count cannot finish it says so by name, instead of giving a truncated
/// number as the reading.
pub const ANCESTOR_MAX: usize = 500_000;

/// The deepest delta chain. Carried all the way (see `Repo::object_at`); no path can reset it to zero.
pub const DELTA_MAX: usize = 64;

/// git's four object kinds. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Commit,
    Tree,
    Blob,
    Tag,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Commit => "commit",
            Kind::Tree => "tree",
            Kind::Blob => "blob",
            Kind::Tag => "tag",
        }
    }

    fn of(name: &str) -> Option<Kind> {
        match name {
            "commit" => Some(Kind::Commit),
            "tree" => Some(Kind::Tree),
            "blob" => Some(Kind::Blob),
            "tag" => Some(Kind::Tag),
            _ => None,
        }
    }

    fn of_code(n: u8) -> Option<Kind> {
        match n {
            1 => Some(Kind::Commit),
            2 => Some(Kind::Tree),
            3 => Some(Kind::Blob),
            4 => Some(Kind::Tag),
            _ => None,
        }
    }
}

/// An object name (sha1, twenty bytes).
pub type Oid = [u8; 20];

/// An object name as forty bare hex digits (git's spelling: no `0x`). The spelling still comes from the
/// core's `hexfmt`; this only drops the two characters of law §1.
fn oid_hex(o: &Oid) -> String {
    let s = zikaron::hexfmt::encode(o);
    s.strip_prefix("0x").unwrap_or(&s).to_string()
}

fn oid_of_hex(s: &str) -> Option<Oid> {
    let t = s.trim();
    let bare = t.strip_prefix("0x").unwrap_or(t);
    if bare.len() != 40 || !bare.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // The core's `decode` takes the law §1 form (with `0x`); git's ref files hold the bare forty digits, so
    // the prefix is added here before handing it over, and hex spelling still has one owner.
    let b = zikaron::hexfmt::decode(&format!("0x{}", bare.to_ascii_lowercase()))?;
    let mut o = [0u8; 20];
    o.copy_from_slice(&b);
    Some(o)
}

fn bad(what: &str) -> Fault {
    Fault::known(Known::GitShape, what.to_string())
}

// ───────────────────────── Pack files ─────────────────────────

/// An `.idx` / `.pack` pair. The index is read wholly into memory (only tens of bytes per object); the pack
/// file is sliced as needed.
struct Pack {
    pack: PathBuf,
    /// Object name → offset in the pack file.
    at: HashMap<Oid, u64>,
    /// The pack file's bytes are read once and kept. Rereading the whole file for every object would, when
    /// counting ancestors (tens of thousands of objects), read a pack of hundreds of megabytes tens of
    /// thousands of times; that work runs on a background thread, and the person would see a cell forever in
    /// transit.
    bytes: std::cell::RefCell<Option<std::rc::Rc<Vec<u8>>>>,
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at + 4)?;
    Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn be64(b: &[u8], at: usize) -> Option<u64> {
    let s = b.get(at..at + 8)?;
    Some(u64::from_be_bytes([s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]]))
}

impl Pack {
    /// This pack file's bytes. Read once, then handed out by reference.
    fn raw(&self) -> Result<std::rc::Rc<Vec<u8>>, Fault> {
        if let Some(b) = self.bytes.borrow().as_ref() {
            return Ok(b.clone());
        }
        let b = std::rc::Rc::new(
            std::fs::read(&self.pack).map_err(|e| classify(&e, &self.pack.display().to_string()))?,
        );
        *self.bytes.borrow_mut() = Some(b.clone());
        Ok(b)
    }

    /// Read a v2 index (`\377tOc` plus version 2). Other versions are refused by name.
    fn open(idx: &Path, pack: &Path) -> Result<Pack, Fault> {
        let b = std::fs::read(idx).map_err(|e| classify(&e, &idx.display().to_string()))?;
        if b.get(..4) != Some(&[0xff, b't', b'O', b'c']) || be32(&b, 4) != Some(2) {
            return Err(bad(&crate::lang::filln(crate::lang::Key::Tail116, &[&(idx.display()).to_string()])));
        }
        let n = be32(&b, 8 + 255 * 4).ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail117)))? as usize;
        let names: usize = 8 + 256 * 4;
        // Check the count against the file length before using it to allocate. The number is written in the
        // file, and a corrupt index can say four billion: `with_capacity` would ask for tens of GB at once,
        // the system would kill the process, and the kill leaves no sentence at this layer. This closes the
        // "foreign number used directly as a capacity" form.
        let sized = |a: usize, w: usize| -> Result<usize, Fault> {
            n.checked_mul(w)
                .and_then(|x| a.checked_add(x))
                .ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail118)))
        };
        let crcs = sized(names, 20)?;
        let offs = sized(crcs, 4)?;
        let big = sized(offs, 4)?;
        if big > b.len() {
            return Err(bad(&crate::lang::filln(crate::lang::Key::Tail119, &[&(idx.display()).to_string(), &(n).to_string()])));
        }
        let mut at = HashMap::with_capacity(n);
        for i in 0..n {
            let s = b
                .get(names + i * 20..names + i * 20 + 20)
                .ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail120)))?;
            let mut o = [0u8; 20];
            o.copy_from_slice(s);
            let raw = be32(&b, offs + i * 4).ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail121)))?;
            let off = if raw & 0x8000_0000 == 0 {
                raw as u64
            } else {
                let j = (raw & 0x7fff_ffff) as usize;
                be64(&b, big + j * 8).ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail122)))?
            };
            at.insert(o, off);
        }
        Ok(Pack { pack: pack.to_path_buf(), at, bytes: std::cell::RefCell::new(None) })
    }
}

/// A variable-length number in a pack file (seven low bits per group, high bit continues).
fn varint(b: &[u8], at: &mut usize) -> Option<u64> {
    let mut v: u64 = 0;
    let mut shift = 0;
    loop {
        let x = *b.get(*at)?;
        *at += 1;
        v |= ((x & 0x7f) as u64) << shift;
        if x & 0x80 == 0 {
            return Some(v);
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
}

/// A variable-length number in a delta (different from the one above: high part first, each group adds one).
fn ofs_varint(b: &[u8], at: &mut usize) -> Option<u64> {
    let mut x = *b.get(*at)?;
    *at += 1;
    let mut v = (x & 0x7f) as u64;
    while x & 0x80 != 0 {
        x = *b.get(*at)?;
        *at += 1;
        v = (v + 1) << 7 | (x & 0x7f) as u64;
    }
    Some(v)
}

/// A little-endian fixed-width number in a delta (bytes picked by a bitmap).
fn packed_size(b: &[u8], at: &mut usize, mask: u8, bytes: usize, default: usize) -> Option<usize> {
    let mut v: usize = 0;
    for i in 0..bytes {
        if mask & (1 << i) != 0 {
            v |= (*b.get(*at)? as usize) << (8 * i);
            *at += 1;
        }
    }
    Some(if v == 0 { default } else { v })
}

/// Apply a delta (git's delta format).
fn undelta(base: &[u8], delta: &[u8]) -> Option<Vec<u8>> {
    let mut at = 0usize;
    let want_base = varint(delta, &mut at)? as usize;
    if want_base != base.len() {
        return None;
    }
    let want_out = varint(delta, &mut at)? as usize;
    if want_out > OBJECT_MAX {
        return None;
    }
    let mut out: Vec<u8> = Vec::with_capacity(want_out);
    while at < delta.len() {
        let op = delta[at];
        at += 1;
        if op & 0x80 != 0 {
            let off = packed_size(delta, &mut at, op & 0x0f, 4, 0)?;
            let len = packed_size(delta, &mut at, (op >> 4) & 0x07, 3, 0x10000)?;
            let end = off.checked_add(len)?;
            out.extend_from_slice(base.get(off..end)?);
        } else if op != 0 {
            let n = op as usize;
            out.extend_from_slice(delta.get(at..at + n)?);
            at += n;
        } else {
            // Reserved zero opcode: git never writes it, and this layer does not guess its meaning.
            return None;
        }
    }
    if out.len() != want_out {
        return None;
    }
    Some(out)
}

// ───────────────────────── A repository ─────────────────────────

/// An opened git repository.
pub struct Repo {
    /// The `.git` location (for a bare repository, its root).
    git: PathBuf,
    packs: Vec<Pack>,
}

/// A commit, read as the three things this layer needs.
pub struct Commit {
    /// The bytes `git cat-file commit <id>` prints, byte for byte.
    pub bytes: Vec<u8>,
    pub parents: Vec<Oid>,
    /// Commit subject: the first line of the message.
    pub subject: String,
}

impl Repo {
    /// Recognize an object name from forty hex digits. `None` when unrecognized.
    pub fn oid(hex: &str) -> Option<Oid> {
        oid_of_hex(hex)
    }

    /// Open a repository. Given a work tree root (containing `.git`) or a bare repository.
    pub fn open(dir: &Path) -> Result<Repo, Fault> {
        if dir.as_os_str().is_empty() {
            return Err(bad(crate::lang::t(crate::lang::Key::Tail123)));
        }
        let dot = dir.join(".git");
        let git = if dot.is_dir() {
            dot
        } else if dot.is_file() {
            // Work trees and submodules: `.git` is a file saying `gitdir: …`.
            let t = std::fs::read_to_string(&dot).map_err(|e| classify(&e, &dot.display().to_string()))?;
            let p = t
                .lines()
                .find_map(|l| l.trim().strip_prefix("gitdir:"))
                .map(|x| x.trim().to_string())
                .ok_or_else(|| bad(&crate::lang::filln(crate::lang::Key::Tail124, &[&(dot.display()).to_string()])))?;
            let p = PathBuf::from(&p);
            if p.is_absolute() { p } else { dir.join(p) }
        } else if dir.join("objects").is_dir() && dir.join("HEAD").is_file() {
            dir.to_path_buf()
        } else {
            return Err(Fault::known(
                Known::NotARepo,
                dir.display().to_string(),
            ));
        };
        if !git.join("objects").is_dir() {
            return Err(Fault::known(Known::NotARepo, git.display().to_string()));
        }
        let mut packs = Vec::new();
        let pdir = git.join("objects").join("pack");
        if pdir.is_dir() {
            let mut idxs: Vec<PathBuf> = std::fs::read_dir(&pdir)
                .map_err(|e| classify(&e, &pdir.display().to_string()))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().map(|x| x == "idx").unwrap_or(false))
                .collect();
            idxs.sort();
            for idx in idxs {
                let pack = idx.with_extension("pack");
                if pack.is_file() {
                    packs.push(Pack::open(&idx, &pack)?);
                }
            }
        }
        Ok(Repo { git, packs })
    }

    pub fn git_dir(&self) -> &Path {
        &self.git
    }

    /// The commit HEAD points to. Symbolic refs are resolved step by step; failure is named.
    pub fn head(&self) -> Result<Oid, Fault> {
        let p = self.git.join("HEAD");
        let t = std::fs::read_to_string(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
        let t = t.trim().to_string();
        let mut name = match t.strip_prefix("ref:") {
            Some(r) => r.trim().to_string(),
            None => {
                return oid_of_hex(&t)
                    .ok_or_else(|| bad(&crate::lang::filln(crate::lang::Key::Tail125, &[&(p.display()).to_string()])))
            }
        };
        // The ref chain is followed at most this many steps: a cycle has nowhere to go.
        for _ in 0..8 {
            match self.resolve_ref(&name)? {
                Ref::Oid(o) => return Ok(o),
                Ref::Sym(next) => name = next,
            }
        }
        Err(bad(crate::lang::t(crate::lang::Key::Tail126)))
    }

    fn resolve_ref(&self, name: &str) -> Result<Ref, Fault> {
        // Names may not contain `..`: a ref name is a relative path inside the repository, and going outside
        // is a bad name.
        if name.is_empty() || name.split('/').any(|s| s == ".." || s.is_empty()) {
            return Err(bad(&crate::lang::filln(crate::lang::Key::Tail127, &[&format!("{:?}", name)])));
        }
        let direct = self.git.join(name);
        if direct.is_file() {
            let t = std::fs::read_to_string(&direct)
                .map_err(|e| classify(&e, &direct.display().to_string()))?;
            let t = t.trim().to_string();
            if let Some(r) = t.strip_prefix("ref:") {
                return Ok(Ref::Sym(r.trim().to_string()));
            }
            return oid_of_hex(&t)
                .map(Ref::Oid)
                .ok_or_else(|| bad(&crate::lang::filln(crate::lang::Key::Tail125, &[&(direct.display()).to_string()])));
        }
        let packed = self.git.join("packed-refs");
        if packed.is_file() {
            let t = std::fs::read_to_string(&packed)
                .map_err(|e| classify(&e, &packed.display().to_string()))?;
            for line in t.lines() {
                let line = line.trim();
                if line.starts_with('#') || line.starts_with('^') {
                    continue;
                }
                let Some((h, n)) = line.split_once(' ') else { continue };
                if n.trim() == name {
                    return oid_of_hex(h)
                        .map(Ref::Oid)
                        .ok_or_else(|| bad(&crate::lang::filln(crate::lang::Key::Tail128, &[&(name).to_string()])));
                }
            }
        }
        Err(Fault::known(Known::RefMissing, name.to_string()))
    }

    /// An object's kind and content. The content excludes the `<kind> <len>\0` header.
    pub fn object(&self, id: &Oid) -> Result<(Kind, Vec<u8>), Fault> {
        self.object_at(id, 0)
    }

    /// As above, with the delta depth carried all the way.
    ///
    /// If offset deltas (type 6) carried the depth while ref deltas (type 7) went back through `object` and
    /// restarted at zero, a pack where A refers to B by name and B to A would recurse forever and overflow
    /// the stack, killing the whole process without even a named refusal. This closes the "some path resets
    /// the depth counter" form.
    fn object_at(&self, id: &Oid, depth: usize) -> Result<(Kind, Vec<u8>), Fault> {
        if depth > DELTA_MAX {
            return Err(bad(crate::lang::t(crate::lang::Key::Tail129)));
        }
        if let Some(x) = self.loose(id)? {
            return Ok(x);
        }
        for p in &self.packs {
            if let Some(off) = p.at.get(id) {
                let raw = p.raw()?;
                return self.from_pack(&raw, *off, depth);
            }
        }
        Err(Fault::known(Known::ObjectMissing, oid_hex(id)))
    }

    fn loose(&self, id: &Oid) -> Result<Option<(Kind, Vec<u8>)>, Fault> {
        let h = oid_hex(id);
        let p = self.git.join("objects").join(&h[..2]).join(&h[2..]);
        if !p.is_file() {
            return Ok(None);
        }
        let z = std::fs::read(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
        let all = crate::zlibx::inflate_zlib(&z, OBJECT_MAX)
            .ok_or_else(|| bad(&crate::lang::filln(crate::lang::Key::Tail130, &[&(p.display()).to_string()])))?;
        let nul = all
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| bad(&crate::lang::filln(crate::lang::Key::Tail131, &[&(p.display()).to_string()])))?;
        let head = std::str::from_utf8(&all[..nul])
            .map_err(|_| bad(&crate::lang::filln(crate::lang::Key::Tail132, &[&(p.display()).to_string()])))?;
        let (name, len) = head
            .split_once(' ')
            .ok_or_else(|| bad(&crate::lang::filln(crate::lang::Key::Tail133, &[&(p.display()).to_string()])))?;
        let kind = Kind::of(name).ok_or_else(|| bad(&crate::lang::filln(crate::lang::Key::Tail134, &[&(p.display()).to_string(), &(name).to_string()])))?;
        let want: usize = len
            .parse()
            .map_err(|_| bad(&crate::lang::filln(crate::lang::Key::Tail135, &[&(p.display()).to_string()])))?;
        let body = all[nul + 1..].to_vec();
        if body.len() != want {
            return Err(bad(&crate::lang::filln(crate::lang::Key::Tail136, &[&(p.display()).to_string(), &(want).to_string(), &(body.len()).to_string()])));
        }
        Ok(Some((kind, body)))
    }

    /// Take one object from a pack file. Deltas nest at most this deep.
    fn from_pack(&self, raw: &[u8], off: u64, depth: usize) -> Result<(Kind, Vec<u8>), Fault> {
        if depth > DELTA_MAX {
            return Err(bad(crate::lang::t(crate::lang::Key::Tail137)));
        }
        let mut at = off as usize;
        let first = *raw.get(at).ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail138)))?;
        at += 1;
        let code = (first >> 4) & 0x07;
        let mut size = (first & 0x0f) as usize;
        let mut shift = 4;
        let mut b = first;
        while b & 0x80 != 0 {
            b = *raw.get(at).ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail139)))?;
            at += 1;
            size |= ((b & 0x7f) as usize) << shift;
            shift += 7;
            if shift > 60 {
                return Err(bad(crate::lang::t(crate::lang::Key::Tail140)));
            }
        }
        if size > OBJECT_MAX {
            return Err(bad(crate::lang::t(crate::lang::Key::Tail141)));
        }
        match code {
            1..=4 => {
                let kind = Kind::of_code(code).ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail142)))?;
                let body = crate::zlibx::inflate_zlib(&raw[at..], OBJECT_MAX)
                    .ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail143)))?;
                if body.len() != size {
                    return Err(bad(crate::lang::t(crate::lang::Key::Tail144)));
                }
                Ok((kind, body))
            }
            6 => {
                let mut p = at;
                let back = ofs_varint(raw, &mut p).ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail145)))?;
                let base_off = off.checked_sub(back).ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail146)))?;
                let (kind, base) = self.from_pack(raw, base_off, depth + 1)?;
                let delta = crate::zlibx::inflate_zlib(&raw[p..], OBJECT_MAX)
                    .ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail147)))?;
                let out = undelta(&base, &delta).ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail148)))?;
                Ok((kind, out))
            }
            7 => {
                let s = raw.get(at..at + 20).ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail149)))?;
                let mut base_id = [0u8; 20];
                base_id.copy_from_slice(s);
                let (kind, base) = self.object_at(&base_id, depth + 1)?;
                let delta = crate::zlibx::inflate_zlib(&raw[at + 20..], OBJECT_MAX)
                    .ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail147)))?;
                let out = undelta(&base, &delta).ok_or_else(|| bad(crate::lang::t(crate::lang::Key::Tail148)))?;
                Ok((kind, out))
            }
            _ => Err(bad(&crate::lang::filln(crate::lang::Key::Tail150, &[&(code).to_string()]))),
        }
    }

    /// A commit, read as three things. Anything other than a commit is refused by name.
    pub fn commit(&self, id: &Oid) -> Result<Commit, Fault> {
        let (kind, bytes) = self.object(id)?;
        if kind != Kind::Commit {
            return Err(bad(&crate::lang::filln(crate::lang::Key::Tail151, &[&(oid_hex(id)).to_string(), &(kind.as_str()).to_string()])));
        }
        let text = String::from_utf8_lossy(&bytes).to_string();
        let mut parents = Vec::new();
        for line in text.lines() {
            if line.is_empty() {
                break;
            }
            if let Some(h) = line.strip_prefix("parent ") {
                if let Some(o) = oid_of_hex(h) {
                    parents.push(o);
                }
            }
        }
        // The message follows the first blank line; the subject is the message's first line.
        let subject = text
            .split_once("\n\n")
            .map(|(_, m)| m.lines().next().unwrap_or("").to_string())
            .unwrap_or_default();
        Ok(Commit { bytes, parents, subject })
    }

    /// How many commits are reachable from this one (including itself, the same as `git rev-list --count`).
    /// When the count cannot finish it says so by name, instead of giving a truncated number.
    pub fn ancestors(&self, head: &Oid) -> Result<usize, Fault> {
        Ok(self.ancestor_set(head)?.len())
    }

    /// The reachable set itself. A count cannot answer "is it on this lineage", so places asking that take
    /// the set (see `anchorx::since`).
    pub fn ancestor_set(&self, head: &Oid) -> Result<std::collections::HashSet<Oid>, Fault> {
        let mut seen: std::collections::HashSet<Oid> = std::collections::HashSet::new();
        let mut stack = vec![*head];
        while let Some(id) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            if seen.len() > ANCESTOR_MAX {
                return Err(bad(&crate::lang::filln(crate::lang::Key::Tail152, &[&(ANCESTOR_MAX).to_string()])));
            }
            let c = self.commit(&id)?;
            for p in c.parents {
                if !seen.contains(&p) {
                    stack.push(p);
                }
            }
        }
        Ok(seen)
    }
}

enum Ref {
    Oid(Oid),
    Sym(String),
}

/// A repository's reading now: HEAD, its object bytes, subject, ancestor count.
pub struct Head {
    pub commit: String,
    /// The sha256 of the bytes of `git cat-file commit HEAD` (computed by the core). This is the anchored
    /// content.
    pub content: [u8; 32],
    pub subject: String,
    pub ancestors: usize,
    pub bytes: usize,
}

/// Read a repository's HEAD. The only owner of the anchoring desk's git-repository input.
pub fn head_of(dir: &Path) -> Result<Head, Fault> {
    let repo = Repo::open(dir)?;
    let id = repo.head()?;
    let c = repo.commit(&id)?;
    Ok(Head {
        commit: oid_hex(&id),
        content: zikaron::cryptox::sha256(&c.bytes),
        subject: c.subject.clone(),
        ancestors: repo.ancestors(&id)?,
        bytes: c.bytes.len(),
    })
}
