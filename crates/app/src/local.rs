//! Local data: everything this app keeps on this machine that the chain does not have is sealed under the
//! local data key, and read and written only here.
//!
//! The local data key D is derived from the vault's master key M (`keybox::local_key`: HKDF-SHA256 under
//! `zikaron/local/v1`). Each file is sealed on its own with XChaCha20-Poly1305 and a fresh 24-byte random
//! nonce; the additional data is the file's kind and format version ([`Doc`]), so a file cannot be opened as
//! another kind, nor under an older version. D exists only while the vault is open: locked, every read and
//! write here is refused by name with `LOCKED` (the master key is wiped on lock, so D cannot be derived).
//!
//! Which files are sealed is the closed table [`Doc`]. Left plain, and never read through here: what must be
//! read before unlocking (the pointer and the chosen-home record, the vault file whose seals are already
//! ciphertext, lock files), exports people hand to others (record bundles, key files, grant files, badges,
//! mirrors, backups), and outside files read by the zero-permission faces.
//!
//! File shape: [`MAGIC`] · kind length (one byte) · kind · version (two bytes, big-endian) · nonce (24 bytes)
//! · ciphertext and tag. Everything before the nonce is the additional data.
//!
//! Writing seals first and lands through `home::put_at` (a temporary name, then an atomic rename, 0600), so a
//! cut never leaves half a file over a good one. Ledger entries go into the storage crate's ledger directory
//! as sealed bytes (the store keeps opaque bytes and knows nothing of this); [`Ledger`] opens them on the way
//! out.

use crate::fault::{classify, Fault, Known};
use crate::home::Home;
use crate::keybox::LocalKey;
use std::path::{Path, PathBuf};
use zikaron_store::{EntryName, LedgerDir, Pile, Skip, Stored, Survey};

/// The leading bytes of every sealed file (format version 1 of the envelope).
pub use zikaron_glue::sealed::MAGIC;
const NONCE: usize = 24;

/// The closed table of sealed kinds. Each has a tag and a format version, bound into the additional data.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Doc {
    /// Machine directory: the identity register.
    Registry,
    /// Machine directory: hand-filled kit links (`kits/links.json`).
    KitLinks,
    /// Machine directory: the local index of signed files (`records/index.json`).
    Records,
    /// Home `settings/`: the settings file.
    Settings,
    /// Home `settings/`: the anchor queue.
    Queue,
    /// Home `settings/`: the first-window checklist.
    FirstWindow,
    /// Home `settings/`: the last audit's anchored set.
    LastAudit,
    /// Home `settings/`: the restored identity's read-only mark.
    Unfetched,
    /// Home `grants-held/`: a held grant's entry bytes.
    Held,
    /// Home `grants-held/`: a held grant's last verdict.
    Verdict,
    /// Home `grants-held/files/`: a kept grant file.
    KeptGrant,
    /// Home `kits/terms/<digest>/`: a terms document received at signing.
    TermsDoc,
    /// Home `kits/terms/`: a grant's issuance record.
    TermsRecord,
    /// Home `ledger/`: one ledger entry.
    Entry,
    /// Machine directory: machine settings staged by a restore (the file itself stays plain, see
    /// `machine.rs`); only ever a staged file.
    Machine,
    /// Machine directory: a change's plan (what goes and where it was), staged with the change; only ever a
    /// staged file.
    Plan,
}

impl Doc {
    pub const ALL: [Doc; 16] = [
        Doc::Registry,
        Doc::KitLinks,
        Doc::Records,
        Doc::Settings,
        Doc::Queue,
        Doc::FirstWindow,
        Doc::LastAudit,
        Doc::Unfetched,
        Doc::Held,
        Doc::Verdict,
        Doc::KeptGrant,
        Doc::TermsDoc,
        Doc::TermsRecord,
        Doc::Entry,
        Doc::Machine,
        Doc::Plan,
    ];

    /// The kind bound into the additional data (and named in the account's closed table).
    pub fn tag(self) -> &'static str {
        match self {
            Doc::Registry => "registry",
            Doc::KitLinks => "kit-links",
            Doc::Records => "records",
            Doc::Settings => "settings",
            Doc::Queue => "queue",
            Doc::FirstWindow => "first-window",
            Doc::LastAudit => "last-audit",
            Doc::Unfetched => "unfetched",
            Doc::Held => "held",
            Doc::Verdict => "verdict",
            Doc::KeptGrant => "kept-grant",
            Doc::TermsDoc => "terms-doc",
            Doc::TermsRecord => "terms-record",
            Doc::Entry => "entry",
            Doc::Machine => "machine",
            Doc::Plan => "plan",
        }
    }

    /// The format version of the sealed plaintext (every kind starts at 1).
    pub fn version(self) -> u16 {
        1
    }

    pub fn from_tag(t: &str) -> Option<Doc> {
        Doc::ALL.into_iter().find(|d| d.tag() == t)
    }
}

fn head(doc: Doc) -> Vec<u8> {
    let tag = doc.tag().as_bytes();
    let mut h = Vec::with_capacity(MAGIC.len() + 3 + tag.len());
    h.extend_from_slice(MAGIC);
    h.push(tag.len() as u8);
    h.extend_from_slice(tag);
    h.extend_from_slice(&doc.version().to_be_bytes());
    h
}

fn nonce() -> Result<[u8; NONCE], Fault> {
    use std::io::Read;
    let mut h = std::fs::File::open(crate::key::ENTROPY).map_err(|e| classify(&e, crate::key::ENTROPY))?;
    let mut n = [0u8; NONCE];
    h.read_exact(&mut n).map_err(|e| classify(&e, crate::key::ENTROPY))?;
    Ok(n)
}

/// Whether bytes carry the sealed envelope (a plain file left by an older version does not).
pub fn is_sealed(bytes: &[u8]) -> bool {
    zikaron_glue::sealed::is_sealed(bytes)
}

/// Seal under a given key (the vault's, or a new master key's while staging a change).
pub fn seal_with(key: &LocalKey, doc: Doc, plain: &[u8]) -> Result<Vec<u8>, Fault> {
    let mut out = head(doc);
    let n = nonce()?;
    let ct = crate::cryptx::xchacha_seal(key.bytes(), &n, &out, plain)
        .ok_or_else(|| Fault::known(Known::LocalSeal, doc.tag().to_string()))?;
    out.extend_from_slice(&n);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Seal under this vault's key (refused with `LOCKED` while locked).
pub fn seal(doc: Doc, plain: &[u8]) -> Result<Vec<u8>, Fault> {
    seal_with(&crate::keybox::local_key()?, doc, plain)
}

/// Open under a given key. Not an envelope, another kind, another version, another key or one altered byte:
/// each refused with `LOCAL_SEAL` (the tail names the kind and `what`).
pub fn open_with(key: &LocalKey, doc: Doc, bytes: &[u8], what: &str) -> Result<Vec<u8>, Fault> {
    let h = head(doc);
    let bad = || Fault::known(Known::LocalSeal, format!("{} {what}", doc.tag()));
    if bytes.len() < h.len() + NONCE || bytes[..h.len()] != h[..] {
        return Err(bad());
    }
    let mut n = [0u8; NONCE];
    n.copy_from_slice(&bytes[h.len()..h.len() + NONCE]);
    crate::cryptx::xchacha_open(key.bytes(), &n, &h, &bytes[h.len() + NONCE..]).ok_or_else(bad)
}

/// Open under this vault's key.
pub fn open(doc: Doc, bytes: &[u8], what: &str) -> Result<Vec<u8>, Fault> {
    open_with(&crate::keybox::local_key()?, doc, bytes, what)
}

/// Read a sealed file: `None` when it is not there. Locked is `LOCKED`; a file that does not open is
/// `LOCAL_SEAL`.
pub fn read(path: &Path, doc: Doc) -> Result<Option<Vec<u8>>, Fault> {
    // A file that is not there holds nothing to protect: "none yet" answers the same locked or open (writing
    // one still needs the key).
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(classify(&e, &path.display().to_string())),
    };
    let key = crate::keybox::local_key()?;
    open_with(&key, doc, &bytes, &path.display().to_string()).map(Some)
}

/// Write a sealed file (replacing it): sealed, then landed through `home::put_at`.
pub fn put(dir: &Path, name: &str, doc: Doc, plain: &[u8]) -> Result<(), Fault> {
    let sealed = seal(doc, plain)?;
    crate::home::put_at(dir, name, &sealed)
}

/// Write a sealed file once: a file already there is refused by the glue crate's landing (write-once files:
/// terms documents, issuance records, held grants, kept grant files).
pub fn land(path: &Path, doc: Doc, plain: &[u8]) -> Result<(), Fault> {
    let sealed = seal(doc, plain)?;
    zikaron_glue::landing::land_bytes(path, &sealed).map_err(|t| Fault::landing(t.code(), t.subject()))
}

fn store(t: zikaron_store::Trouble) -> Fault {
    Fault::known(Known::Ledger, format!("{t:?}"))
}

/// A home's ledger: the storage crate's ledger directory holding sealed entries. Every way into a home's
/// ledger goes through here; the entry bytes handed out are the opened ones.
pub struct Ledger {
    dir: LedgerDir,
}

impl Ledger {
    /// Open a ledger directory (created when missing) whose entries are sealed.
    pub fn open_or_create(root: impl Into<PathBuf>) -> Result<Ledger, Fault> {
        Ok(Ledger { dir: LedgerDir::open_or_create(root).map_err(store)? })
    }

    /// Open an existing one (another seat's home, read-only use).
    pub fn open(root: impl Into<PathBuf>) -> Result<Ledger, Fault> {
        Ok(Ledger { dir: LedgerDir::open(root).map_err(store)? })
    }

    pub fn root(&self) -> &Path {
        self.dir.root()
    }

    /// The storage directory underneath (layout reports and sweeps, which do not read entry bytes).
    pub fn store(&self) -> &LedgerDir {
        &self.dir
    }

    /// Strict read: the audit pile, opened. Anything the store cannot account for, and any entry that does not
    /// open, refuses the whole read by name.
    pub fn pile(&self) -> Result<Pile, Fault> {
        let key = crate::keybox::local_key()?;
        let p = self.dir.pile().map_err(store)?;
        let mut items = Vec::with_capacity(p.items.len());
        for (i, b) in p.items.iter().enumerate() {
            items.push(open_with(&key, Doc::Entry, b, &format!("{} #{i}", self.root().display()))?);
        }
        Ok(Pile { items })
    }

    /// Lenient read: every entry that opens, and each skip with its reason (an entry that does not open is
    /// listed as unreadable).
    pub fn survey(&self) -> Result<Survey, Fault> {
        let key = crate::keybox::local_key()?;
        let s = self.dir.survey().map_err(store)?;
        let mut out = Survey { items: Vec::with_capacity(s.items.len()), skipped: s.skipped };
        for (i, b) in s.items.iter().enumerate() {
            match open_with(&key, Doc::Entry, b, "") {
                Ok(p) => out.items.push(p),
                Err(_) => zikaron_store::ledger::note_skip(&mut out.skipped, &format!("sealed #{i}"), zikaron_store::Why::Unreadable),
            }
        }
        out.skipped.sort_by(|a: &Skip, b: &Skip| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Read one entry by its logical file name (`<entry id>.entry`), opened. The name on disk is keyed
    /// (`names`); callers speak entry ids only.
    pub fn read_named(&self, file: &str) -> Result<Vec<u8>, Fault> {
        let key = crate::keybox::local_key()?;
        let disk = disk_entry_file(file)?;
        let b = self.dir.read_named(&disk).map_err(store)?;
        open_with(&key, Doc::Entry, &b, file)
    }

    /// Seal one entry under this vault's key (for staging several before landing any).
    pub fn seal_entry(&self, plain: &[u8]) -> Result<Vec<u8>, Fault> {
        seal(Doc::Entry, plain)
    }

    /// Append one entry. The store refuses to overwrite and keeps opaque bytes, and every seal takes a fresh
    /// nonce, so sameness is judged here on the opened bytes: the same entry again is idempotent, a different
    /// one under the same name is refused with nothing changed.
    pub fn append(&self, name: &EntryName, plain: &[u8]) -> Result<Stored, Fault> {
        // `name` is the entry's id; the file on disk is named by the names key (`names`).
        let name = &disk_entry_name(name.as_str())?;
        let file = zikaron_store::layout::entry_file_name(name);
        match self.dir.read_named(&file) {
            Ok(have) => {
                let key = crate::keybox::local_key()?;
                let opened = open_with(&key, Doc::Entry, &have, &file)?;
                if zikaron_store::ledger::identical(&opened, plain) {
                    return Ok(Stored::AlreadyThere);
                }
                return Err(store(zikaron_store::Trouble::named(zikaron_store::Code::Conflict, &file)));
            }
            Err(t) if t.code == zikaron_store::Code::Absent => {}
            Err(t) => return Err(store(t)),
        }
        let sealed = self.seal_entry(plain)?;
        self.dir.append(name, &sealed).map_err(store)
    }
}

/// The name on disk of an entry, from its id (under the vault's names key now).
fn disk_entry_name(id: &str) -> Result<EntryName, Fault> {
    let nk = crate::names::key()?;
    entry_name_under(&nk, id)
}

/// The name on disk of an entry under a given names key.
pub fn entry_name_under(nk: &crate::names::NameKey, id: &str) -> Result<EntryName, Fault> {
    EntryName::parse(&nk.name(crate::names::Logical::Entry(id))).ok_or_else(|| Fault::known(Known::Ledger, id.to_string()))
}

/// Where one entry lies in a ledger directory on disk (its name keyed by the names key; vault open).
pub fn entry_path(ledger: &Path, id: &str) -> Result<PathBuf, Fault> {
    Ok(ledger.join(zikaron_store::layout::entry_file_name(&disk_entry_name(id)?)))
}

/// The disk file name of a logical entry file name (`<id>.entry`).
fn disk_entry_file(file: &str) -> Result<String, Fault> {
    let id = zikaron_store::layout::parse_entry_file(file).ok_or_else(|| Fault::known(Known::Ledger, file.to_string()))?;
    Ok(zikaron_store::layout::entry_file_name(&disk_entry_name(id.as_str())?))
}

/// A home's ledger (the one way `Home` hands it out).
pub fn ledger_of(home: &Home) -> Result<Ledger, Fault> {
    Ledger::open_or_create(home.dir(crate::home::Slot::Ledger))
}

// ───────────────────────── Where local data lies ─────────────────────────
//
// One walker lists every sealed file on this machine: the machine directory's sealed files, and every home
// (each identity's seat homes from the register, plus the home this machine resolves to without one). Plain
// migration, resealing under a new master key, settling staged files and the whole-machine backup all walk
// the same list, so none of them can miss a file another one sees.

/// Whose a home is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Whose {
    /// The machine directory itself.
    Machine,
    /// An identity's seat home.
    Seat { id: String, seat: crate::roles::Role },
    /// A home with no identity (the chosen or default home of a machine without a register row for it).
    Loose,
    /// A data folder this machine opened as its own before and no longer points at (`machine::Machine::homes`);
    /// numbered in the order remembered.
    Kept { n: usize },
    /// A home a deleted identity left on disk (`identity::Registry::left`).
    Left { id: String, seat: crate::roles::Role },
}

/// One sealed (or not yet sealed) file.
#[derive(Clone, Debug)]
pub struct Found {
    pub at: PathBuf,
    /// Its path inside its home or the machine directory, `/`-separated.
    pub rel: String,
    pub doc: Doc,
    pub whose: Whose,
}

/// Which kind a file in the machine directory is, by its path there (`None`: not sealed local data).
pub fn machine_doc(rel: &str) -> Option<Doc> {
    if rel == crate::places::registry_file() {
        return Some(Doc::Registry);
    }
    if rel == format!("{}/{}", crate::kitsindex::DIR, crate::kitsindex::OVERRIDES) {
        return Some(Doc::KitLinks);
    }
    if rel == format!("{}/{}", crate::recordsx::DIR, crate::recordsx::FILE) {
        return Some(Doc::Records);
    }
    None
}

/// Which kind a file in a home is, by its path there (`None`: not sealed local data, e.g. record bundles
/// exported into `kits/`, lock files, temporary files).
pub fn home_doc(rel: &str) -> Option<Doc> {
    use crate::home::Slot;
    let parts: Vec<&str> = rel.split('/').collect();
    let settings = Slot::Settings.as_str();
    match parts.as_slice() {
        [s, f] if *s == settings && *f == crate::settings::FILE => Some(Doc::Settings),
        [s, f] if *s == settings && *f == crate::queue::FILE => Some(Doc::Queue),
        [s, f] if *s == settings && *f == crate::wizard::FILE => Some(Doc::FirstWindow),
        [s, f] if *s == settings && *f == crate::lastread::ANCHORED_FILE => Some(Doc::LastAudit),
        [s, f] if *s == settings && *f == crate::restorex::FILE => Some(Doc::Unfetched),
        [l, f] if *l == Slot::Ledger.as_str() && zikaron_store::layout::parse_entry_file(f).is_some() => Some(Doc::Entry),
        [g, f] if *g == Slot::GrantsHeld.as_str() && crate::lastread::is_cache(f) => Some(Doc::Verdict),
        [g, rest @ ..] if *g == Slot::GrantsHeld.as_str() => crate::vaultx::held_doc(&rest.join("/")),
        [k, t, f] if *k == Slot::Kits.as_str() && *t == crate::termsx::ROOM && f.starts_with("grant-") && f.ends_with(".json") => Some(Doc::TermsRecord),
        [k, t, d, _] if *k == Slot::Kits.as_str() && *t == crate::termsx::ROOM && d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit()) => Some(Doc::TermsDoc),
        _ => None,
    }
}

fn walk(root: &Path, rel: &str, out: &mut Vec<(PathBuf, String)>) {
    let at = if rel.is_empty() { root.to_path_buf() } else { root.join(rel) };
    let Ok(listing) = std::fs::read_dir(&at) else { return };
    for e in listing.filter_map(|e| e.ok()) {
        let name = e.file_name().to_string_lossy().to_string();
        let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        match std::fs::symlink_metadata(e.path()) {
            Ok(m) if m.is_dir() => walk(root, &r, out),
            Ok(m) if m.is_file() => out.push((e.path(), r)),
            _ => {}
        }
    }
}

/// The same walk, refusing by name a directory that cannot be listed (a change that reseals every file may not
/// pass over one it cannot see).
fn walk_strict(root: &Path, rel: &str, out: &mut Vec<(PathBuf, String)>) -> Result<(), Fault> {
    let at = if rel.is_empty() { root.to_path_buf() } else { root.join(rel) };
    let listing = std::fs::read_dir(&at).map_err(|e| classify(&e, &at.display().to_string()))?;
    for e in listing {
        let e = e.map_err(|x| classify(&x, &at.display().to_string()))?;
        let name = e.file_name().to_string_lossy().to_string();
        let r = if rel.is_empty() { name } else { format!("{rel}/{name}") };
        match std::fs::symlink_metadata(e.path()) {
            Ok(m) if m.is_dir() => walk_strict(root, &r, out)?,
            Ok(m) if m.is_file() => out.push((e.path(), r)),
            Ok(_) => {}
            Err(x) => return Err(classify(&x, &e.path().display().to_string())),
        }
    }
    Ok(())
}

/// Every home on this machine: each identity's seat homes (from the register, which must be readable: sealed
/// under this vault's key), then the home this machine resolves to when it is not one of them. A home not on
/// disk is left out (a seat never opened has none yet).
pub fn homes() -> Result<Vec<(Whose, PathBuf)>, Fault> {
    homes_of(crate::register::read()?, false)
}

/// This machine's homes as its plain records and the layout say, without reading the sealed register: the
/// home the pointer names, the folders the machine settings remember, and beside each of them (and in the
/// machine directory) every seat home laid out by the rule `identity_home` follows (`<place>/<identity>/<seat>`).
/// A whole-machine restore from the lock card cannot read the register (locked, and the key it is sealed
/// under is the one being replaced), so this is how it still finds every former home.
pub fn homes_on_record() -> Vec<PathBuf> {
    let mut places: Vec<PathBuf> = Vec::new();
    if let Ok(h) = crate::home::where_is() {
        places.push(h);
    }
    if let Ok(m) = crate::machine::read() {
        places.extend(m.homes.iter().map(PathBuf::from));
    }
    let mut bases: Vec<PathBuf> = places.iter().filter_map(|p| p.parent().map(Path::to_path_buf)).collect();
    if let Ok(m) = crate::home::machine_dir() {
        bases.push(m);
    }
    let is_id = |n: &str| n.len() == 40 && n.chars().all(|c| c.is_ascii_hexdigit());
    let mut out = places;
    for b in bases {
        let Ok(list) = std::fs::read_dir(&b) else { continue };
        for e in list.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !is_id(&name) || !e.path().is_dir() {
                continue;
            }
            for seat in crate::roles::Role::ALL {
                let h = e.path().join(seat.as_str());
                if h.is_dir() {
                    out.push(h);
                }
            }
        }
    }
    let mut uniq: Vec<PathBuf> = Vec::new();
    for p in out {
        if !uniq.iter().any(|x| crate::home::same_place(x, &p)) {
            uniq.push(p);
        }
    }
    uniq
}

/// The register as it lies: sealed, or plain when an older version left it (read only by the passes that run
/// before the plain files are sealed: settling and migration).
fn registry_any() -> Result<Option<crate::identity::Registry>, Fault> {
    let at = crate::register::path()?;
    match std::fs::read(&at) {
        Ok(b) if !is_sealed(&b) => Ok(Some(
            crate::identity::Registry::parse(&b).map_err(|why| Fault::known(Known::IdentitiesShape, format!("{}: {why}", at.display())))?,
        )),
        _ => crate::register::read(),
    }
}

/// `strict`: a home the register names whose place is gone (its parent is not there either: a disk not
/// attached, a folder moved away) is refused by name instead of left out; a home whose place is there but was
/// never made is still left out. Two seats sharing one home (an older row) count it once.
fn homes_of(reg: Option<crate::identity::Registry>, strict: bool) -> Result<Vec<(Whose, PathBuf)>, Fault> {
    let mut out: Vec<(Whose, PathBuf)> = Vec::new();
    if let Some(reg) = &reg {
        for row in &reg.rows {
            for seat in crate::roles::Role::ALL {
                if let Some(h) = row.home(seat) {
                    if out.iter().any(|(_, x)| crate::home::same_place(x, &h)) {
                        continue;
                    }
                    if h.is_dir() {
                        out.push((Whose::Seat { id: row.id.clone(), seat }, h));
                    } else if strict && !h.parent().map(Path::is_dir).unwrap_or(false) {
                        return Err(Fault::known(Known::HomeUnreachable, h.display().to_string()));
                    }
                }
            }
        }
        // The homes deleted identities left (listed, not a seat's): taken when on disk, never refused.
        for (id, seat, home) in &reg.left {
            let h = PathBuf::from(home);
            if h.is_dir() && !out.iter().any(|(_, x)| crate::home::same_place(x, &h)) {
                out.push((Whose::Left { id: id.clone(), seat: *seat }, h));
            }
        }
    }
    if let Ok(h) = crate::home::where_is() {
        if h.is_dir() && !out.iter().any(|(_, x)| crate::home::same_place(x, &h)) {
            out.push((Whose::Loose, h));
        }
    }
    // The folders this machine opened before: gone with their place still there means the person removed
    // them; gone with their place gone too is refused by name when strict (as for a seat's home).
    let remembered = match crate::machine::read() {
        Ok(m) => m.homes,
        Err(f) if strict => return Err(f),
        Err(_) => Vec::new(),
    };
    for (n, h) in remembered.iter().map(PathBuf::from).enumerate() {
        if out.iter().any(|(_, x)| crate::home::same_place(x, &h)) {
            continue;
        }
        if h.is_dir() {
            out.push((Whose::Kept { n }, h));
        } else if strict && !h.parent().map(Path::is_dir).unwrap_or(false) {
            return Err(Fault::known(Known::HomeUnreachable, h.display().to_string()));
        }
    }
    Ok(out)
}

/// The machine directory's sealed files.
pub fn machine_files() -> Result<Vec<Found>, Fault> {
    let m = crate::home::machine_dir()?;
    let mut all = Vec::new();
    walk(&m, "", &mut all);
    let mut out: Vec<Found> = all
        .into_iter()
        .filter_map(|(at, rel)| machine_doc(&rel).map(|doc| Found { at, rel, doc, whose: Whose::Machine }))
        .collect();
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    Ok(out)
}

/// One home's sealed files.
pub fn home_files(root: &Path, whose: &Whose) -> Vec<Found> {
    let mut all = Vec::new();
    walk(root, "", &mut all);
    let mut out: Vec<Found> = all
        .into_iter()
        .filter_map(|(at, rel)| home_doc(&rel).map(|doc| Found { at, rel, doc, whose: whose.clone() }))
        .collect();
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

/// Every sealed file on this machine (machine directory first, then each home). Strict: a home whose place is
/// gone, or a directory that cannot be listed, is refused by name, never passed over (resealing under a new
/// master key and the whole-machine backup walk this list, and a file they miss would be lost with the old key).
pub fn all_files() -> Result<Vec<Found>, Fault> {
    let m = crate::home::machine_dir()?;
    let mut mine = Vec::new();
    walk_strict(&m, "", &mut mine)?;
    let mut out: Vec<Found> = mine
        .into_iter()
        .filter_map(|(at, rel)| machine_doc(&rel).map(|doc| Found { at, rel, doc, whose: Whose::Machine }))
        .collect();
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    for (w, h) in homes_of(crate::register::read()?, true)? {
        let mut all = Vec::new();
        walk_strict(&h, "", &mut all)?;
        let mut here: Vec<Found> = all
            .into_iter()
            .filter_map(|(at, rel)| home_doc(&rel).map(|doc| Found { at, rel, doc, whose: w.clone() }))
            .collect();
        here.sort_by(|a, b| a.rel.cmp(&b.rel));
        out.extend(here);
    }
    Ok(out)
}

// ───────────────────────── Names on disk: logical and keyed ─────────────────────────
//
// A file's logical path says what it holds by what it stands for (`ledger/<entry id>.entry`,
// `grants-held/<grant id>.entry`, `kits/terms/<digest>/<digest>`...): backups carry it, and it is what a change
// of names starts from. Its path on disk keys those names by the names key (`names`). The two are translated
// here only.

fn hex_id(b: &[u8]) -> String {
    zikaron::hexfmt::encode(b).trim_start_matches("0x").to_ascii_lowercase()
}

/// A file's logical path, from its path on disk and its opened bytes. Kinds with fixed names keep theirs.
pub fn logical_rel(doc: Doc, rel: &str, plain: &[u8]) -> Result<String, Fault> {
    use crate::home::Slot;
    let shape = || Fault::known(Known::LocalSeal, format!("{} {rel}", doc.tag()));
    let text_of = |k: &str| -> Option<String> {
        match zikaron::json::parse(plain).ok()?.member(k) {
            Some(zikaron::json::Value::Str(x)) => Some(x.trim().trim_start_matches("0x").to_ascii_lowercase()),
            _ => None,
        }
    };
    Ok(match doc {
        Doc::Entry => format!("{}/{}{}", Slot::Ledger.as_str(), hex_id(&zikaron::entry::entry_id(plain)), zikaron_store::layout::ENTRY_SUFFIX),
        Doc::Held => format!("{}/{}{}", Slot::GrantsHeld.as_str(), hex_id(&zikaron::entry::entry_id(plain)), zikaron_store::layout::ENTRY_SUFFIX),
        Doc::Verdict => {
            // The grant is named inside (older caches: their name was the grant id).
            let id = match text_of("grant") {
                Some(g) => g,
                None => rel.rsplit('/').next().and_then(|n| n.strip_suffix(crate::lastread::VERDICT_SUFFIX)).map(str::to_string).ok_or_else(shape)?,
            };
            format!("{}/{id}{}", Slot::GrantsHeld.as_str(), crate::lastread::VERDICT_SUFFIX)
        }
        Doc::KeptGrant => {
            let kit = crate::grantfilex::open_bytes(plain)?.kit_id;
            format!("{}/{}/{}.{}", Slot::GrantsHeld.as_str(), crate::grantfilex::KEPT, kit.trim().trim_start_matches("0x").to_ascii_lowercase(), zikaron_glue::container::EXT)
        }
        Doc::TermsDoc => {
            let d = hex_id(&zikaron_kit::doc::doc_id(plain));
            format!("{}/{}/{d}/{d}", Slot::Kits.as_str(), crate::termsx::ROOM)
        }
        Doc::TermsRecord => format!("{}/{}/grant-{}.json", Slot::Kits.as_str(), crate::termsx::ROOM, text_of("grant").ok_or_else(shape)?),
        _ => rel.to_string(),
    })
}

/// A logical path's place on disk under a names key.
pub fn disk_rel(doc: Doc, logical: &str, nk: &crate::names::NameKey) -> Result<String, Fault> {
    use crate::home::Slot;
    use crate::names::Logical as L;
    let parts: Vec<&str> = logical.split('/').collect();
    let bad = || Fault::known(Known::LocalSeal, format!("{} {logical}", doc.tag()));
    let stem = |f: &str, suffix: &str| -> Result<String, Fault> { f.strip_suffix(suffix).map(str::to_string).ok_or_else(bad) };
    Ok(match (doc, parts.as_slice()) {
        (Doc::Entry, [_, f]) => format!("{}/{}{}", Slot::Ledger.as_str(), nk.name(L::Entry(&stem(f, zikaron_store::layout::ENTRY_SUFFIX)?)), zikaron_store::layout::ENTRY_SUFFIX),
        (Doc::Held, [_, f]) => format!("{}/{}{}", Slot::GrantsHeld.as_str(), nk.name(L::Held(&stem(f, zikaron_store::layout::ENTRY_SUFFIX)?)), zikaron_store::layout::ENTRY_SUFFIX),
        (Doc::Verdict, [_, f]) => format!("{}/{}{}", Slot::GrantsHeld.as_str(), nk.name(L::Held(&stem(f, crate::lastread::VERDICT_SUFFIX)?)), crate::lastread::VERDICT_SUFFIX),
        (Doc::KeptGrant, [_, _, f]) => {
            let ext = format!(".{}", zikaron_glue::container::EXT);
            format!("{}/{}/{}{ext}", Slot::GrantsHeld.as_str(), crate::grantfilex::KEPT, nk.name(L::Kept(&stem(f, &ext)?)))
        }
        (Doc::TermsDoc, [_, _, d, _]) => format!("{}/{}/{}/{}", Slot::Kits.as_str(), crate::termsx::ROOM, nk.name(L::TermsDir(d)), nk.name(L::TermsDoc(d))),
        (Doc::TermsRecord, [_, _, f]) => {
            let g = f.strip_prefix("grant-").and_then(|x| x.strip_suffix(".json")).ok_or_else(bad)?;
            format!("{}/{}/grant-{}.json", Slot::Kits.as_str(), crate::termsx::ROOM, nk.name(L::TermsRecord(g)))
        }
        (Doc::Entry | Doc::Held | Doc::Verdict | Doc::KeptGrant | Doc::TermsDoc | Doc::TermsRecord, _) => return Err(bad()),
        _ => logical.to_string(),
    })
}

/// A seat home's place under a names key: the same base, the identity's keyed name, the seat. A home laid out
/// otherwise (not `<base>/<40 hex>/<seat>`) keeps its place.
pub fn home_under(nk: &crate::names::NameKey, id: &str, seat: crate::roles::Role, at: &Path) -> PathBuf {
    let seat_ok = at.file_name().map(|n| n == seat.as_str()).unwrap_or(false);
    let named = at.parent().and_then(|p| p.file_name()).map(|n| crate::names::is_home_name(&n.to_string_lossy())).unwrap_or(false);
    match (seat_ok && named, at.parent().and_then(Path::parent)) {
        (true, Some(base)) => base.join(crate::home::home_dir_name(nk, id)).join(seat.as_str()),
        _ => at.to_path_buf(),
    }
}

/// What a change of names staged: where each file will be, and what it leaves.
#[derive(Default, Debug)]
pub struct Restaged {
    /// Staged files (their places after landing).
    pub to: Vec<PathBuf>,
    /// Places that go once the change takes effect (a file now at another name).
    pub gone: Vec<PathBuf>,
    /// Directories this change made (removed if it never takes effect).
    pub made: Vec<PathBuf>,
}

/// Stage every local file of this machine for a change of key or of names: opened under `old`, sealed under
/// `new`, placed at its name under `nk` (a seat home's directory renamed too, and the register's homes
/// rewritten to match), each beside its new place as `<file>.zk-next`. Nothing takes the place of anything
/// yet; the plan (what goes, what was made) is staged with it, sealed under `new`, for the landing pass
/// (`settle_pending`). The machine settings are not touched.
pub fn restage(old: &LocalKey, new: &LocalKey, nk: &crate::names::NameKey) -> Result<Restaged, Fault> {
    let files = all_files()?;
    let reg = crate::register::read()?;
    // Each seat home (and each home a deleted identity left) at its new place.
    let mut homes: Vec<(PathBuf, PathBuf)> = Vec::new();
    if let Some(r) = &reg {
        for row in &r.rows {
            for seat in crate::roles::Role::ALL {
                if let Some(h) = row.home(seat) {
                    homes.push((h.clone(), home_under(nk, &row.id, seat, &h)));
                }
            }
        }
        for (id, seat, h) in &r.left {
            let h = PathBuf::from(h);
            homes.push((h.clone(), home_under(nk, id, *seat, &h)));
        }
    }
    let moved_home = |p: &str| -> String {
        homes.iter().find(|(a, _)| crate::home::same_place(a, Path::new(p))).map(|(_, b)| b.display().to_string()).unwrap_or_else(|| p.to_string())
    };
    let mut out = Restaged::default();
    // Each home that moves is laid out at its new place first (its rooms, as a home has them), planned
    // before it is made.
    for (a, b) in &homes {
        if a != b && a.is_dir() && !b.exists() {
            let top = identity_dir_of(b).filter(|d| !d.exists()).unwrap_or_else(|| b.clone());
            if !out.made.contains(&top) {
                out.made.push(top);
                stage_plan(PLAN_MADE, &out.made)?;
            }
            crate::home::lay(b)?;
        }
    }
    for f in files {
        let raw = std::fs::read(&f.at).map_err(|e| classify(&e, &f.at.display().to_string()))?;
        let mut plain = open_with(old, f.doc, &raw, &f.rel)?;
        if f.doc == Doc::Registry {
            // The register names the homes: rewritten to their new places.
            let mut r = crate::identity::Registry::parse(&plain).map_err(|w| Fault::known(Known::IdentitiesShape, w))?;
            for row in r.rows.iter_mut() {
                row.author_home = if row.author_home.is_empty() { String::new() } else { moved_home(&row.author_home) };
                row.grantee_home = if row.grantee_home.is_empty() { String::new() } else { moved_home(&row.grantee_home) };
            }
            for (_, _, h) in r.left.iter_mut() {
                *h = moved_home(h);
            }
            plain = r.to_bytes();
        }
        if f.doc == Doc::Verdict {
            // An older cache is named by its grant only: the grant goes inside before its name is keyed.
            let lr = logical_rel(Doc::Verdict, &f.rel, &plain)?;
            let id = lr.rsplit('/').next().and_then(|n| n.strip_suffix(crate::lastread::VERDICT_SUFFIX)).unwrap_or_default().to_string();
            plain = crate::lastread::with_grant(&plain, &id)?;
        }
        let root = root_of(&f);
        let new_root = homes.iter().find(|(a, _)| crate::home::same_place(a, &root)).map(|(_, b)| b.clone()).unwrap_or_else(|| root.clone());
        let to = new_root.join(disk_rel(f.doc, &logical_rel(f.doc, &f.rel, &plain)?, nk)?);
        if let Some(d) = to.parent() {
            if !d.exists() {
                // The topmost directory this change makes (removed whole if the change never takes effect).
                let mut top = d.to_path_buf();
                while let Some(up) = top.parent() {
                    if up.exists() {
                        break;
                    }
                    top = up.to_path_buf();
                }
                if !out.made.contains(&top) {
                    // Planned before it is made: a cut from here on removes it at the next unlock.
                    out.made.push(top);
                    stage_plan(PLAN_MADE, &out.made)?;
                }
                std::fs::create_dir_all(d).map_err(|e| classify(&e, &d.display().to_string()))?;
            }
        }
        let put = stage_next(&to, new, f.doc, &plain);
        zikaron_ui::secret::wipe(&mut plain);
        put?;
        if to != f.at {
            out.gone.push(f.at.clone());
        }
        out.to.push(to);
    }
    stage_plan(PLAN_MADE, &out.made)?;
    stage_plan_sealed(PLAN_GONE, new, &out.gone.iter().map(|p| p.display().to_string()).collect::<Vec<_>>())?;
    let moved: Vec<String> = homes.iter().filter(|(a, b)| a != b && a.is_dir()).map(|(a, b)| format!("{}\t{}", a.display(), b.display())).collect();
    stage_plan_sealed(PLAN_MOVED, new, &moved)?;
    Ok(out)
}

/// Where a found file's home (or the machine directory) is: its path without its relative path.
fn root_of(f: &Found) -> PathBuf {
    let mut root = f.at.clone();
    for _ in f.rel.split('/') {
        root.pop();
    }
    root
}

/// Whether some local file is not at its name under the vault's names key now: an older disk (names that say
/// ids) or one cut half way through a change of names.
fn names_out_of_date(nk: &crate::names::NameKey) -> Result<bool, Fault> {
    let key = crate::keybox::local_key()?;
    let reg = crate::register::read()?;
    if let Some(r) = &reg {
        for row in &r.rows {
            for seat in crate::roles::Role::ALL {
                if let Some(h) = row.home(seat) {
                    if home_under(nk, &row.id, seat, &h) != h {
                        return Ok(true);
                    }
                }
            }
        }
        for (id, seat, h) in &r.left {
            let h = PathBuf::from(h);
            if home_under(nk, id, *seat, &h) != h {
                return Ok(true);
            }
        }
    }
    for f in all_files()? {
        if matches!(f.doc, Doc::Entry | Doc::Held | Doc::Verdict | Doc::KeptGrant | Doc::TermsDoc | Doc::TermsRecord) {
            let raw = std::fs::read(&f.at).map_err(|e| classify(&e, &f.at.display().to_string()))?;
            let mut plain = open_with(&key, f.doc, &raw, &f.rel)?;
            let want = disk_rel(f.doc, &logical_rel(f.doc, &f.rel, &plain)?, nk)?;
            let old_verdict = f.doc == Doc::Verdict && !plain.windows(7).any(|w| w == b"\"grant\"");
            zikaron_ui::secret::wipe(&mut plain);
            if want != f.rel || old_verdict {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// The first opening after upgrading (and after a change of names cut half way): every local file whose name
/// still says an identity, an entry or a grant is moved to its keyed name, all through the same pass as a
/// change of master key (staged, then landed; the old names go only once every new one is staged), so a cut
/// at any moment is picked up again at the next opening and no file is lost. Answers how many moved.
pub fn migrate_names() -> Result<usize, Fault> {
    let nk = crate::names::key()?;
    // What an older version left outside the register (homes of identities it deleted) and outside the homes'
    // files (its set-aside trees, under the paths they had) is brought in first.
    if let Err(f) = flatten_old_aside() {
        note_trouble(f);
    }
    // Strays named by an id (an empty seat, files this key cannot open) are cleared first, so what is left of
    // them is a seat home `adopt_left_homes` can list and the renaming below renames.
    match crate::keybox::local_key().and_then(|k| clear_strays(&k, true)) {
        Ok((0, _)) => {}
        Ok((n, at)) => note_trouble(Fault::known(Known::LocalSetAside, format!("{n} · {}", at.map(|p| p.display().to_string()).unwrap_or_default()))),
        Err(f) => note_trouble(f),
    }
    if let Err(f) = adopt_left_homes() {
        note_trouble(f);
    }
    if !names_out_of_date(&nk)? {
        return Ok(0);
    }
    let key = crate::keybox::local_key()?;
    let m = crate::home::machine_dir()?;
    stage_plan(PLAN_OPEN, &[])?;
    let staged = match restage(&key, &key, &nk) {
        Ok(s) => s,
        Err(f) => {
            // Never took effect: the next unlock (or this one, now) drops what was staged.
            let _ = settle_pending();
            return Err(f);
        }
    };
    cut_point(Cut::BeforeCommit)?;
    let n = staged.gone.len();
    std::fs::remove_file(plan_path(&m, PLAN_OPEN)).map_err(|e| classify(&e, PLAN_OPEN))?;
    cut_point(Cut::AfterCommit)?;
    let s = settle_pending()?;
    if s.held > 0 {
        return Err(Fault::known(Known::MigrateMismatch, format!("{} held", s.held)));
    }
    Ok(n)
}

// ───────────────────────── Migrating plain files ─────────────────────────

static MIGRATE_FAULT: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

/// Make the sealed copy of one file read back different during migration, once (only the test hooks call it,
/// to show a mismatch stops migration with every plain file kept; the window never does).
pub fn set_migrate_fault(at: PathBuf) -> bool {
    MIGRATE_FAULT.set(at).is_ok()
}

/// The suffix of a sealed copy written beside a plain file during migration. One name, one home.
pub const SEALED_BESIDE: &str = ".zk-sealed";

/// What a migration pass did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Migrated {
    /// How many plain files were sealed.
    pub sealed: usize,
}

fn beside(at: &Path, suffix: &str) -> PathBuf {
    let mut s = at.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Seal a list of plain files, all or none: each gets a sealed copy beside it, read back from disk and
/// opened, and compared byte for byte with the plain file; only when every one matches is each copy renamed
/// over its plain file. The first mismatch stops the pass: every copy written is removed, not one plain file
/// is deleted, and the refusal names that file (`MIGRATE_MISMATCH`).
fn seal_plain(list: &[Found]) -> Result<usize, Fault> {
    let key = crate::keybox::local_key()?;
    let mut done: Vec<(PathBuf, PathBuf)> = Vec::new();
    let undo = |done: &[(PathBuf, PathBuf)]| {
        for (_, copy) in done {
            let _ = std::fs::remove_file(copy);
        }
    };
    for f in list {
        let plain = std::fs::read(&f.at).map_err(|e| classify(&e, &f.at.display().to_string()))?;
        if is_sealed(&plain) {
            continue;
        }
        let copy = beside(&f.at, SEALED_BESIDE);
        let step = (|| -> Result<(), Fault> {
            let sealed = seal_with(&key, f.doc, &plain)?;
            // A sealed entry is longer than its plain bytes by the envelope; one that would pass the ledger's
            // cap no more is refused by name here, not written to make the whole ledger unreadable.
            if f.doc == Doc::Entry && sealed.len() > zikaron_store::ENTRY_MAX {
                return Err(Fault::known(Known::Ledger, format!("{}: {} > {}", f.at.display(), sealed.len(), zikaron_store::ENTRY_MAX)));
            }
            let dir = copy.parent().map(Path::to_path_buf).unwrap_or_default();
            let name = copy.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            crate::home::put_at(&dir, &name, &sealed)?;
            let mut back = std::fs::read(&copy).map_err(|e| classify(&e, &copy.display().to_string()))?;
            // The test hooks' injected read-back fault (never set in the shipped app).
            if MIGRATE_FAULT.get().map(|p| p == &f.at).unwrap_or(false) {
                if let Some(b) = back.last_mut() {
                    *b ^= 1;
                }
            }
            // Reading the copy back is the check itself: a copy that does not open is as much a mismatch as one
            // that opens to other bytes.
            let opened = open_with(&key, f.doc, &back, &f.rel).map_err(|_| Fault::known(Known::MigrateMismatch, f.at.display().to_string()))?;
            if opened != plain {
                return Err(Fault::known(Known::MigrateMismatch, f.at.display().to_string()));
            }
            Ok(())
        })();
        if let Err(e) = step {
            let _ = std::fs::remove_file(&copy);
            undo(&done);
            // Said with its own reason (too large, unreadable, a mismatch), naming the file: never all folded
            // into "did not match", which would send the person after the wrong cause.
            let named = match e.which() {
                Some(Known::MigrateMismatch) | None => e,
                Some(k) => Fault::known(k, format!("{} · {}", f.at.display(), e.tail())),
            };
            return Err(named);
        }
        done.push((f.at.clone(), copy));
    }
    for (at, copy) in &done {
        crate::home::rename_over(copy, at)?;
    }
    Ok(done.len())
}

/// The first unlock after upgrading: every plain local file on this machine is sealed in one pass, all or
/// none (the register, still plain before the pass, is read as it is to find the homes). Files already sealed
/// are left as they are, so a second pass seals nothing. Leftover sealed copies of a cut pass are removed
/// first (their plain files were never deleted).
pub fn migrate_plain() -> Result<Migrated, Fault> {
    let _ = crate::keybox::local_key()?;
    let m = crate::home::machine_dir()?;
    drop_leftovers(&m);
    let mut list = machine_files()?;
    for (w, h) in homes_of(registry_any()?, false)? {
        drop_leftovers(&h);
        list.extend(home_files(&h, &w));
    }
    Ok(Migrated { sealed: seal_plain(&list)? })
}

fn drop_leftovers(root: &Path) {
    let mut all = Vec::new();
    walk(root, "", &mut all);
    for (at, rel) in all {
        if rel.ends_with(SEALED_BESIDE) {
            let _ = std::fs::remove_file(at);
        }
    }
}

// ───────────────────────── Files under a master key that is gone ─────────────────────────

/// The room, under each place, where files sealed under a master key that is gone are moved. One name, one home.
pub const SET_ASIDE: &str = "set-aside";

/// Before a new passcode makes a new master key on a machine whose key store is gone (reset, or its file lost):
/// every sealed file still under the machine directory, the home this machine resolves to and the folders it
/// opened before can never open again, and one left in place would stop its home from opening. Each is moved,
/// byte for byte, into `set-aside` (numbered when taken) under its own place, keeping its path there. Answers
/// how many and the first place they went (`None` when there were none).
pub fn set_aside_sealed() -> Result<(usize, Option<PathBuf>), Fault> {
    let mut roots: Vec<PathBuf> = vec![crate::home::machine_dir()?];
    if let Ok(h) = crate::home::where_is() {
        roots.push(h);
    }
    if let Ok(m) = crate::machine::read() {
        roots.extend(m.homes.iter().map(PathBuf::from));
    }
    set_aside_in(roots, None)
}

/// After a master key change has taken effect (a whole-machine restore): every sealed file under these places
/// that does not open under the new key is sealed under the key that was replaced and can never open again.
/// Each is moved aside the same way, never deleted (the places a restore leaves behind: this machine's former
/// homes, the files of the home it keeps that the backup did not have, machine files the backup did not have).
pub fn set_aside_unopenable(roots: Vec<PathBuf>, key: &LocalKey) -> Result<(usize, Option<PathBuf>), Fault> {
    set_aside_in(roots, Some(key))
}

/// Whether sealed bytes open under this key, as any kind.
fn opens_under(key: &LocalKey, bytes: &[u8]) -> bool {
    Doc::ALL.iter().any(|d| open_with(key, *d, bytes, "").is_ok())
}

fn set_aside_in(roots: Vec<PathBuf>, keep: Option<&LocalKey>) -> Result<(usize, Option<PathBuf>), Fault> {
    let m = crate::home::machine_dir()?;
    // The homes in use now (after a change took effect, the register sealed under the new key names them):
    // never taken away, whatever they hold.
    let live: Vec<PathBuf> = match keep {
        Some(_) => homes_of(crate::register::read().ok().flatten(), false).map(|h| h.into_iter().map(|(_, p)| p).collect()).unwrap_or_default(),
        None => Vec::new(),
    };
    let mut seen: Vec<PathBuf> = Vec::new();
    let mut sealed: Vec<PathBuf> = Vec::new();
    let mut seats: Vec<PathBuf> = Vec::new();
    for root in roots {
        if !root.is_dir() || seen.iter().any(|x| crate::home::same_place(x, &root)) {
            continue;
        }
        seen.push(root.clone());
        let mut all = Vec::new();
        walk(&root, "", &mut all);
        let here: Vec<PathBuf> = all
            .into_iter()
            // What an earlier pass set aside stays where it is; a staged file belongs to a change in flight.
            .filter(|(_, rel)| !rel.split('/').any(|c| c.starts_with(SET_ASIDE)) && !is_staged(&rel))
            .filter(|(at, _)| match std::fs::read(at) {
                Ok(b) => is_sealed(&b) && keep.map(|k| !opens_under(k, &b)).unwrap_or(true),
                Err(_) => false,
            })
            .map(|(at, _)| at)
            .collect();
        for at in here {
            if !sealed.iter().any(|x| x == &at) {
                sealed.push(at);
            }
        }
        if identity_dir_of(&root).is_some() && !live.iter().any(|h| crate::home::same_place(h, &root)) {
            seats.push(root);
        }
    }
    // One place in the machine directory, the files numbered flat: a name they had (an identity's home, an
    // entry, a grant, under the key being replaced) is not kept. Made only when something goes there.
    let mut to: Option<PathBuf> = None;
    let mut n = 0usize;
    let mut put = |at: &Path, to: &mut Option<PathBuf>| -> Result<(), Fault> {
        if to.is_none() {
            let mut p = m.join(SET_ASIDE);
            let mut i = 2u32;
            while p.exists() {
                p = m.join(format!("{SET_ASIDE}-{i}"));
                i += 1;
            }
            std::fs::create_dir_all(&p).map_err(|e| classify(&e, &p.display().to_string()))?;
            *to = Some(p);
        }
        n += 1;
        let dest = to.as_ref().map(|p| p.join(format!("{n}"))).unwrap_or_default();
        move_across(at, &dest)
    };
    for at in &sealed {
        put(at, &mut to)?;
    }
    // A former seat home goes whole, and its identity directory with it: whatever it still holds that is not
    // sealed (its lock seat, record bundles exported into it) is set aside with the rest, numbered on (kept,
    // never deleted, never under the home's name). A home still holding sealed files the key in effect opens
    // is not a former one and stays as it is.
    for root in &seats {
        let mut rest = Vec::new();
        walk(root, "", &mut rest);
        if rest.iter().any(|(at, _)| std::fs::read(at).map(|b| is_sealed(&b)).unwrap_or(false)) {
            continue;
        }
        for (at, rel) in rest {
            if !is_staged(&rel) {
                put(&at, &mut to)?;
            }
        }
        prune_empty_under(root);
        if std::fs::remove_dir(root).is_ok() {
            if let Some(d) = root.parent() {
                let _ = std::fs::remove_dir(d);
            }
        }
    }
    Ok((sealed.len(), to))
}

// ───────────────────────── Staged files of a master key change ─────────────────────────

/// A plan's file in the machine directory: the one place a plan's name is spelled (`<name>` beside a staged
/// suffix). Staging, reading and removing a plan, and the test hooks, all go through it.
pub fn plan_path(machine: &Path, name: &str) -> PathBuf {
    staged_path(&machine.join(name))
}

/// Where a file's staged copy lies: beside it, under the staged suffix (`keybox::NEXT`). With
/// [`staged_name`], the only places that suffix is put on a name.
pub fn staged_path(at: &Path) -> PathBuf {
    beside(at, crate::keybox::NEXT)
}

/// A file name's staged copy's name (the name under the staged suffix).
pub fn staged_name(name: &str) -> String {
    format!("{name}{}", crate::keybox::NEXT)
}

/// Whether a path (or a relative path) names a staged copy.
pub fn is_staged(p: &str) -> bool {
    p.ends_with(crate::keybox::NEXT)
}

/// A staged copy's path without its staged suffix (`None` when it is not one).
pub fn unstaged(p: &str) -> Option<&str> {
    p.strip_suffix(crate::keybox::NEXT)
}

/// Write a plan's file (see [`plan_path`]).
fn put_plan(machine: &Path, name: &str, bytes: &[u8]) -> Result<(), Fault> {
    let at = plan_path(machine, name);
    let file = at.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    crate::home::put_at(machine, &file, bytes)
}

/// The staged content that means "this file goes" (a whole-machine restore removes machine files the backup
/// does not have).
pub const GONE: &[u8] = b"zikaron-local/gone";

/// Where a master key change stops on purpose (a test hook's cut, to show a cut leaves one whole state).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cut {
    /// Everything staged, the vault not yet renamed: the change never took effect.
    BeforeCommit,
    /// The vault renamed, the staged files not yet settled: the change took effect; the next unlock settles.
    AfterCommit,
}

static CUT: std::sync::OnceLock<Cut> = std::sync::OnceLock::new();

/// Set a cut once. Only the test hooks call it (as with `places::set`); the window never does, so the shipped
/// app never stops midway on purpose.
pub fn set_cut(c: Cut) -> bool {
    CUT.set(c).is_ok()
}

/// Stop here when a test hook set this cut: answered as an error without cleaning up, exactly what a power cut
/// at this point would leave.
pub fn cut_point(c: Cut) -> Result<(), Fault> {
    if CUT.get() == Some(&c) {
        return Err(Fault::known(Known::RestorePartial, format!("cut {c:?}")));
    }
    Ok(())
}

/// Whether a failure is a test hook's cut (`cut_point`): what a power cut leaves, which the running process
/// must not tidy up (a real cut never gets the chance).
pub fn is_cut(f: &Fault) -> bool {
    CUT.get().is_some() && f.which() == Some(Known::RestorePartial) && f.tail().starts_with("cut ")
}

/// Remove every staged file on this machine (a change that failed before taking effect).
pub fn drop_staged_everywhere() -> Result<(), Fault> {
    drop_staged_machine()?;
    for (_, h) in homes()? {
        let mut all = Vec::new();
        walk(&h, "", &mut all);
        for (at, rel) in all {
            if is_staged(&rel) {
                let _ = std::fs::remove_file(at);
            }
        }
    }
    Ok(())
}

/// Undo a change that failed before taking effect: the directories it made go, and every staged file.
pub fn drop_change() -> Result<(), Fault> {
    let m = crate::home::machine_dir()?;
    for d in read_plan(&m, PLAN_MADE) {
        if d.is_dir() {
            let _ = std::fs::remove_dir_all(&d);
        }
    }
    drop_staged_everywhere()
}

/// Remove the machine directory's staged files (a change that failed before taking effect).
pub fn drop_staged_machine() -> Result<(), Fault> {
    let m = crate::home::machine_dir()?;
    let mut all = Vec::new();
    walk(&m, "", &mut all);
    for (at, rel) in all {
        if is_staged(&rel) {
            let _ = std::fs::remove_file(at);
        }
    }
    Ok(())
}

/// Stage one file sealed under a new key: `<file>.zk-next` beside it (see `keybox::NEXT`).
pub fn stage_next(at: &Path, key: &LocalKey, doc: Doc, plain: &[u8]) -> Result<(), Fault> {
    let sealed = seal_with(key, doc, plain)?;
    let next = staged_path(at);
    let dir = next.parent().map(Path::to_path_buf).unwrap_or_default();
    let name = next.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    crate::home::put_at(&dir, &name, &sealed)
}

/// A change's own plan, staged beside its files in the machine directory (plain, one path per line) and gone
/// with them: the homes a restore made in fresh places, removed when the change never took effect; and the
/// places whose files the new key cannot open, moved aside once it took effect. Whether the change finishes in
/// its own run or at the next unlock after a cut, the same pass (`settle_pending`) carries the plan out.
pub const PLAN_MADE: &str = "change-made-homes";
pub const PLAN_ASIDE: &str = "change-set-aside";
/// A change of names: the places that go once it takes effect (a file now at its new name), and each home
/// that moved (`<old>\t<new>`: what else it holds follows it, and the old place goes). Both are sealed (they
/// name what the disk names) under the key the change leads to.
pub const PLAN_GONE: &str = "change-gone";
pub const PLAN_MOVED: &str = "change-moved-homes";
/// A change of names under the same key (a migration) has no vault to rename: this mark, down while it
/// stages, is the change not having taken effect; removing it is the moment it does.
pub const PLAN_OPEN: &str = "change-open";

/// Stage one part of a change's plan (replacing what was staged under that name).
pub fn stage_plan(name: &str, paths: &[PathBuf]) -> Result<(), Fault> {
    let lines: String = paths.iter().map(|p| format!("{}\n", p.display())).collect();
    put_plan(&crate::home::machine_dir()?, name, lines.as_bytes())
}

/// Stage one part of a change's plan sealed under `key` (the key the change leads to).
pub fn stage_plan_sealed(name: &str, key: &LocalKey, lines: &[String]) -> Result<(), Fault> {
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let sealed = seal_with(key, Doc::Plan, text.as_bytes())?;
    put_plan(&crate::home::machine_dir()?, name, &sealed)
}

/// A sealed plan's lines (empty when there is none or it does not open under `key`: a change that never took
/// effect).
fn read_plan_sealed(machine: &Path, name: &str, key: &LocalKey) -> Vec<String> {
    let at = plan_path(&machine, name);
    let Ok(raw) = std::fs::read(&at) else { return Vec::new() };
    match open_with(key, Doc::Plan, &raw, name) {
        Ok(mut plain) => {
            let out = String::from_utf8_lossy(&plain).lines().filter(|l| !l.is_empty()).map(str::to_string).collect();
            zikaron_ui::secret::wipe(&mut plain);
            out
        }
        Err(_) => Vec::new(),
    }
}

fn plan_there(machine: &Path, name: &str) -> bool {
    plan_path(&machine, name).exists()
}

/// Remove empty directories upward from `from` (itself included), stopping below `stop` or at the first
/// directory that holds anything.
fn prune_up(from: &Path, stop: &Path) {
    let mut at = from.to_path_buf();
    while at.starts_with(stop) && at != stop {
        if std::fs::remove_dir(&at).is_err() {
            break;
        }
        match at.parent() {
            Some(p) => at = p.to_path_buf(),
            None => break,
        }
    }
}

/// Remove every empty directory under `root` (itself kept).
fn prune_empty_under(root: &Path) {
    let Ok(list) = std::fs::read_dir(root) else { return };
    for e in list.flatten() {
        let p = e.path();
        if std::fs::symlink_metadata(&p).map(|m| m.is_dir()).unwrap_or(false) {
            prune_empty_under(&p);
            let _ = std::fs::remove_dir(&p);
        }
    }
}

/// A seat home's identity directory (`<base>/<40 hex>/<seat>` → `<base>/<40 hex>`), or `None` for a home laid
/// out otherwise.
fn identity_dir_of(home: &Path) -> Option<PathBuf> {
    let seat = home.file_name()?.to_string_lossy().to_string();
    let parent = home.parent()?;
    let named = crate::names::is_home_name(&parent.file_name()?.to_string_lossy());
    (named && crate::roles::Role::ALL.iter().any(|r| r.as_str() == seat)).then(|| parent.to_path_buf())
}

/// A home that moved under a change of names: whatever it still holds (what is not sealed local data, e.g.
/// record bundles exported into it) follows it to its new place unless that place already has it; then the old
/// place goes, and its identity directory with it when that is left empty.
fn carry_home(old: &Path, new: &Path) -> usize {
    let mut failed = 0usize;
    let mut rest = Vec::new();
    walk(old, "", &mut rest);
    let mut clash: Vec<PathBuf> = Vec::new();
    for (at, rel) in rest {
        if is_staged(&rel) {
            continue;
        }
        let to = new.join(&rel);
        if to.exists() {
            // Its new place already has one: this one is kept aside, never left under the old name.
            clash.push(at);
            continue;
        }
        let moved = to.parent().map(|d| std::fs::create_dir_all(d).is_ok()).unwrap_or(true) && move_across(&at, &to).is_ok();
        if !moved {
            failed += 1;
        }
    }
    if !clash.is_empty() && aside_flat(&clash).is_err() {
        failed += clash.len();
    }
    prune_empty_under(old);
    match std::fs::remove_dir(old) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => failed += 1,
    }
    if let Some(d) = identity_dir_of(old) {
        let _ = std::fs::remove_dir(&d);
    }
    failed
}

/// Move one file to a place that may be on another volume (a home the person picked on another disk, set
/// aside into the machine directory): a rename where it can be one; across volumes, copied, flushed to disk,
/// then the source removed (the copy is whole before the source goes, so a cut leaves one or both).
fn move_across(from: &Path, to: &Path) -> Result<(), Fault> {
    let n = MOVES.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    if let Some(MoveFault::FailOnce(k)) = MOVE_FAULT.get() {
        if *k == n {
            return Err(classify(&std::io::Error::other("injected"), &to.display().to_string()));
        }
    }
    let renamed = if matches!(MOVE_FAULT.get(), Some(MoveFault::CrossDevice)) {
        Err(std::io::Error::from(std::io::ErrorKind::CrossesDevices))
    } else {
        std::fs::rename(from, to)
    };
    match renamed {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            let say = |e: std::io::Error| classify(&e, &to.display().to_string());
            let mut src = std::fs::File::open(from).map_err(say)?;
            let mut dst = std::fs::OpenOptions::new().write(true).create_new(true).open(to).map_err(say)?;
            std::io::copy(&mut src, &mut dst).map_err(say)?;
            dst.sync_all().map_err(say)?;
            std::fs::remove_file(from).map_err(|e| classify(&e, &from.display().to_string()))
        }
        Err(e) => Err(classify(&e, &to.display().to_string())),
    }
}

/// A fault the test hooks inject into moving files (as with `set_cut`): every move answers as across
/// volumes, or the n-th move (counted from 1 in this process) fails once. Only the test hooks set it; the
/// window never does, so the shipped app never fails a move on purpose.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MoveFault {
    CrossDevice,
    FailOnce(usize),
}

static MOVE_FAULT: std::sync::OnceLock<MoveFault> = std::sync::OnceLock::new();
static MOVES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Set the injected move fault once (test hooks only).
pub fn set_move_fault(f: MoveFault) -> bool {
    MOVE_FAULT.set(f).is_ok()
}

/// How many moves this process has made so far (tests pick which one fails).
pub fn moves_so_far() -> usize {
    MOVES.load(std::sync::atomic::Ordering::SeqCst)
}

/// Move these files into a fresh place in the machine directory's `set-aside`, numbered flat (kept, never
/// deleted, never under a name they had). Answers the place.
fn aside_flat(files: &[PathBuf]) -> Result<PathBuf, Fault> {
    let m = crate::home::machine_dir()?;
    let mut to = m.join(SET_ASIDE);
    let mut i = 2u32;
    while to.exists() {
        to = m.join(format!("{SET_ASIDE}-{i}"));
        i += 1;
    }
    std::fs::create_dir_all(&to).map_err(|e| classify(&e, &to.display().to_string()))?;
    for (n, at) in files.iter().enumerate() {
        if let Err(f) = move_across(at, &to.join(format!("{}", n + 1))) {
            // Nothing half-made is left behind: a place that received nothing goes again.
            let _ = std::fs::remove_dir(&to);
            return Err(f);
        }
    }
    Ok(to)
}

/// Where whole homes set aside go (in the machine directory, numbered; the name says nothing of whose).
pub const ASIDE_HOMES: &str = "aside-homes";

/// A change that swaps a home for a fresh one (`<home>\t<old data place>`), staged in the machine directory
/// while it runs (see `swap_aside`).
pub const PLAN_SWAP: &str = "change-swap-home";

/// Where the fresh home that will take a home's place is prepared: beside it, named after it.
pub fn staged_home(root: &Path) -> PathBuf {
    staged_path(root)
}

/// Prepare the fresh home that will take the place of the home at `root` (see `action::fetch_aside`): staged
/// beside it, with the rooms that are not the ledger carried over as they are (its settings file, the held
/// grants, the record bundles; sealed under the same key), and its read-only mark placed: writing opens only
/// once the fetched ledger's tail is checked. The home at `root` is not touched.
pub fn stage_fresh_home(root: &Path) -> Result<crate::home::Home, Fault> {
    let staged = staged_home(root);
    if staged.exists() {
        std::fs::remove_dir_all(&staged).map_err(|e| classify(&e, &staged.display().to_string()))?;
    }
    let home = crate::home::Home::open_or_create(&staged)?;
    carry_settings(root, &home)?;
    for slot in [crate::home::Slot::GrantsHeld, crate::home::Slot::Kits] {
        copy_tree(&root.join(slot.as_str()), &home.dir(slot))?;
    }
    crate::restorex::write(&home, crate::restorex::State::Unfetched)?;
    Ok(home)
}

/// Copy every file under `from` to the same place under `to` (flushed to disk).
fn copy_tree(from: &Path, to: &Path) -> Result<(), Fault> {
    let mut files = Vec::new();
    walk(from, "", &mut files);
    for (at, rel) in files {
        let dest = to.join(&rel);
        if let Some(d) = dest.parent() {
            std::fs::create_dir_all(d).map_err(|e| classify(&e, &d.display().to_string()))?;
        }
        let say = |e: std::io::Error| classify(&e, &dest.display().to_string());
        std::fs::copy(&at, &dest).map_err(say)?;
        std::fs::File::open(&dest).and_then(|f| f.sync_all()).map_err(say)?;
    }
    Ok(())
}

/// Swap the home at `root` for the fresh one staged beside it: the home is set aside whole as old data, in
/// the machine directory's `aside-homes/<n>` (named by number, never by whose), listed among this machine's
/// data folders (resealed and renamed with every home, carried by a backup, opened to read), and the staged
/// home takes its place. The plan is written first and removed last; a cut anywhere between is finished at
/// the next unlock (`settle_swap`): forward once the old data is whole in its place, back otherwise. Answers
/// where the old data went.
pub fn swap_aside(root: &Path) -> Result<PathBuf, Fault> {
    let m = crate::home::machine_dir()?;
    let room = m.join(ASIDE_HOMES);
    std::fs::create_dir_all(&room).map_err(|e| classify(&e, &room.display().to_string()))?;
    let mut n = 1u32;
    while room.join(format!("{n}")).exists() || room.join(format!("{n}{PARTIAL}")).exists() {
        n += 1;
    }
    let to = room.join(format!("{n}"));
    put_plan(&m, PLAN_SWAP, format!("{}\t{}\n", root.display(), to.display()).as_bytes())?;
    let mut machine = crate::machine::read()?;
    machine.homes.push(to.display().to_string());
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    machine.aside_at.push((to.display().to_string(), now));
    crate::machine::write(&machine)?;
    cut_point(Cut::BeforeCommit)?;
    move_dir(root, &to)?;
    cut_point(Cut::AfterCommit)?;
    crate::home::rename_over(&staged_home(root), root)?;
    let _ = std::fs::remove_file(plan_path(&m, PLAN_SWAP));
    Ok(to)
}

/// A place being copied across volumes is filled under this suffix and named only once whole.
const PARTIAL: &str = ".partial";

/// Move a whole directory: renamed where it can be; across volumes copied whole into `<to>.partial`, then
/// named `to`, and only then the original removed (a cut leaves the original whole, or both whole).
fn move_dir(from: &Path, to: &Path) -> Result<(), Fault> {
    let injected = matches!(MOVE_FAULT.get(), Some(MoveFault::CrossDevice));
    match if injected { Err(std::io::Error::from(std::io::ErrorKind::CrossesDevices)) } else { std::fs::rename(from, to) } {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
            let partial = beside(to, PARTIAL);
            if partial.exists() {
                std::fs::remove_dir_all(&partial).map_err(|e| classify(&e, &partial.display().to_string()))?;
            }
            copy_tree(from, &partial)?;
            crate::home::rename_over(&partial, to)?;
            std::fs::remove_dir_all(from).map_err(|e| classify(&e, &from.display().to_string()))
        }
        Err(e) => Err(classify(&e, &to.display().to_string())),
    }
}

/// Finish a swap a cut left half done (on unlock, before any home is read): with the old data whole in its
/// place, the fresh home is put in the home's (forward); otherwise the staged home and any partial copy go and
/// the old data is unlisted (back: the home stays as it was). Staged fresh homes no plan names go too.
pub fn settle_swap() -> Result<(), Fault> {
    let m = crate::home::machine_dir()?;
    let plan = plan_path(&m, PLAN_SWAP);
    if let Ok(text) = std::fs::read_to_string(&plan) {
        if let Some((root, to)) = text.trim().split_once('\t') {
            let (root, to) = (PathBuf::from(root), PathBuf::from(to));
            let staged = staged_home(&root);
            if staged.is_dir() {
                if to.is_dir() {
                    if root.exists() {
                        std::fs::remove_dir_all(&root).map_err(|e| classify(&e, &root.display().to_string()))?;
                    }
                    crate::home::rename_over(&staged, &root)?;
                } else {
                    let _ = std::fs::remove_dir_all(beside(&to, PARTIAL));
                    let _ = std::fs::remove_dir_all(&staged);
                    let mut machine = crate::machine::read()?;
                    machine.homes.retain(|h| Path::new(h) != to);
                    machine.aside_at.retain(|(h, _)| Path::new(h) != to);
                    crate::machine::write(&machine)?;
                }
            }
        }
        let _ = std::fs::remove_file(&plan);
    }
    // A fresh home staged by a pass that stopped before its plan: never swapped in. (A register that cannot be
    // read now, still sealed under a key being replaced, is looked at again at the next unlock.)
    if let Ok(Some(reg)) = registry_any() {
        for row in &reg.rows {
            for seat in crate::roles::Role::ALL {
                if let Some(h) = row.home(seat) {
                    let staged = staged_home(&h);
                    if staged.is_dir() && h.is_dir() {
                        let _ = std::fs::remove_dir_all(&staged);
                    }
                }
            }
        }
    }
    Ok(())
}

/// A fresh home in the place of one set aside keeps the settings it had (nodes, basis, publish address),
/// sealed as they were (its queue belongs to the entries set aside and stays with them).
pub fn carry_settings(from_root: &Path, into: &crate::home::Home) -> Result<(), Fault> {
    let from = from_root.join(crate::home::Slot::Settings.as_str()).join(crate::settings::FILE);
    match read(&from, Doc::Settings)? {
        Some(plain) => put(&into.dir(crate::home::Slot::Settings), crate::settings::FILE, Doc::Settings, &plain),
        None => Ok(()),
    }
}

/// A set-aside place already in the flat form: no directory in it, and every file named by its number (dot
/// files the system leaves in a folder someone looked at aside).
fn aside_is_flat(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|d| {
            d.flatten().all(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                !e.path().is_dir() && (name.starts_with('.') || name.bytes().all(|b| b.is_ascii_digit()))
            })
        })
        .unwrap_or(true)
}

/// Older versions set files aside inside each place, under the paths they had there (an entry's or a grant's
/// id in its name): every such tree, in the machine directory and in every home, is flattened into the
/// machine directory's `set-aside` (numbered), and goes. Idempotent; a cut leaves every file in one of the
/// two places.
fn flatten_old_aside() -> Result<usize, Fault> {
    let m = crate::home::machine_dir()?;
    let mut roots = vec![m.clone()];
    roots.extend(homes_of(crate::register::read()?, false)?.into_iter().map(|(_, h)| h));
    let mut n = 0usize;
    for root in roots {
        let Ok(list) = std::fs::read_dir(&root) else { continue };
        for e in list.flatten() {
            let p = e.path();
            if !e.file_name().to_string_lossy().starts_with(SET_ASIDE) || !p.is_dir() {
                continue;
            }
            if root == m && aside_is_flat(&p) {
                continue;
            }
            let mut files = Vec::new();
            walk(&p, "", &mut files);
            let files: Vec<PathBuf> = files.into_iter().map(|(at, _)| at).collect();
            if !files.is_empty() {
                aside_flat(&files)?;
                n += files.len();
            }
            prune_empty_under(&p);
            let _ = std::fs::remove_dir(&p);
        }
    }
    Ok(n)
}

/// The places homes lie in: the machine directory, the place of the home this machine resolves to and of the
/// folders it opened before, and the place of every home the register names.
fn home_places() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Ok(m) = crate::home::machine_dir() {
        out.push(m);
    }
    if let Some(p) = crate::home::where_is().ok().and_then(|h| h.parent().map(Path::to_path_buf)) {
        out.push(p);
    }
    if let Ok(m) = crate::machine::read() {
        out.extend(m.homes.iter().filter_map(|h| Path::new(h).parent().map(Path::to_path_buf)));
    }
    for d in known_identity_dirs() {
        if let Some(p) = d.parent() {
            out.push(p.to_path_buf());
        }
    }
    let mut uniq: Vec<PathBuf> = Vec::new();
    for p in out {
        if !uniq.iter().any(|x| crate::home::same_place(x, &p)) {
            uniq.push(p);
        }
    }
    uniq
}

/// The identity directory of every home the register names (its identities' seats, on disk or only named, and
/// the homes deleted identities left), read as the register lies.
fn known_identity_dirs() -> Vec<PathBuf> {
    let Ok(Some(reg)) = registry_any() else { return Vec::new() };
    let mut homes: Vec<PathBuf> = reg.rows.iter().flat_map(|r| crate::roles::Role::ALL.iter().filter_map(|s| r.home(*s)).collect::<Vec<_>>()).collect();
    homes.extend(reg.left.iter().map(|(_, _, h)| PathBuf::from(h)));
    homes.iter().filter_map(|h| identity_dir_of(h)).collect()
}

/// **Nothing is left named by an id, whatever left it.** Under every place homes lie in, a
/// directory laid out as an identity's homes (a home's name, a seat's directory in it) that is no home the
/// register knows is a stray: an older version's, one a cut left, one a person copied in. Its files that do not
/// open under this vault's key (and its plain ones) are set aside byte for byte, numbered flat in the machine
/// directory; its empty directories go. With `adopt`, a seat whose files all open under the key stays for
/// `adopt_left_homes` to list and the change of names to rename; without it (after a change took effect,
/// when every home to be kept is on the register already) whatever is still there is set aside too. Staged
/// files of a change in flight are left alone. The rule is the end state, not a list of the ways a stray comes
/// about. Answers how many files were set aside, and where.
pub fn clear_strays(key: &LocalKey, adopt: bool) -> Result<(usize, Option<PathBuf>), Fault> {
    let known = known_identity_dirs();
    let seat_named = |p: &Path| crate::roles::Role::ALL.iter().any(|r| p.join(r.as_str()).is_dir());
    let mut aside: Vec<PathBuf> = Vec::new();
    let mut strays: Vec<PathBuf> = Vec::new();
    for place in home_places() {
        let Ok(list) = std::fs::read_dir(&place) else { continue };
        for e in list.flatten() {
            let d = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            if !d.is_dir() || !crate::names::is_home_name(&name) || !seat_named(&d) || known.iter().any(|k| crate::home::same_place(k, &d)) {
                continue;
            }
            if strays.iter().any(|x| crate::home::same_place(x, &d)) {
                continue;
            }
            let mut files = Vec::new();
            walk(&d, "", &mut files);
            let opens = |at: &Path| std::fs::read(at).map(|b| is_sealed(&b) && opens_under(key, &b)).unwrap_or(false);
            // Each file by what it is: in a seat's directory (`<seat>/<rel in the home>`), a local file that opens
            // is kept for adoption, and so is what a home holds besides its local files (its lock seat, bundles
            // exported into it) when that seat keeps at least one local file; everything else goes aside.
            let in_seat = |rel: &str| -> Option<(String, String)> {
                let (seat, rest) = rel.split_once('/')?;
                crate::roles::Role::ALL.iter().any(|r| r.as_str() == seat).then(|| (seat.to_string(), rest.to_string()))
            };
            let seats_kept: Vec<String> = files
                .iter()
                .filter_map(|(at, rel)| in_seat(rel).filter(|(_, rest)| home_doc(rest).is_some() && opens(at)).map(|(seat, _)| seat))
                .collect();
            for (at, rel) in files {
                if is_staged(&rel) {
                    continue;
                }
                let keep = adopt
                    && match in_seat(&rel) {
                        Some((seat, rest)) => seats_kept.contains(&seat) && (home_doc(&rest).is_none() || opens(&at)),
                        None => false,
                    };
                if !keep {
                    aside.push(at);
                }
            }
            strays.push(d);
        }
    }
    let to = if aside.is_empty() { None } else { Some(aside_flat(&aside)?) };
    for d in &strays {
        prune_empty_under(d);
        let _ = std::fs::remove_dir(d);
    }
    Ok((aside.len(), to))
}

/// Seat homes an older version left on disk for identities it deleted (it kept the homes and recorded
/// nothing): laid out as `<place>/<identity id>/<seat>` beside the known homes, neither the register's nor
/// listed as left. Each whose sealed files all open under this vault's key is listed as left, so it is renamed
/// with the rest and found again when the identity is imported again. Answers how many were listed.
fn adopt_left_homes() -> Result<usize, Fault> {
    let Some(mut reg) = crate::register::read()? else { return Ok(0) };
    let key = crate::keybox::local_key()?;
    let mut known: Vec<PathBuf> = reg.rows.iter().flat_map(|r| crate::roles::Role::ALL.iter().filter_map(|s| r.home(*s)).collect::<Vec<_>>()).collect();
    known.extend(reg.left.iter().map(|(_, _, h)| PathBuf::from(h)));
    let mut added = 0usize;
    for h in homes_on_record() {
        let Some(d) = identity_dir_of(&h) else { continue };
        let Some(seat) = h.file_name().and_then(|n| crate::roles::Role::ALL.into_iter().find(|r| r.as_str() == n.to_string_lossy())) else { continue };
        if known.iter().any(|k| crate::home::same_place(k, &h)) {
            continue;
        }
        // Its local files (the kinds a home holds): at least one, every one sealed and opening under this
        // vault's key. A home of plain files (deleted before its files were sealed) or of another vault's is
        // not this machine's to rename.
        let mut files = Vec::new();
        walk(&h, "", &mut files);
        let local: Vec<&(PathBuf, String)> = files.iter().filter(|(_, rel)| home_doc(rel).is_some()).collect();
        let all_open = !local.is_empty()
            && local.iter().all(|(at, _)| match std::fs::read(at) {
                Ok(b) => is_sealed(&b) && opens_under(&key, &b),
                Err(_) => false,
            });
        if !all_open {
            continue;
        }
        let id = format!("0x{}", d.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default());
        reg.left.push((id, seat, h.display().to_string()));
        known.push(h);
        added += 1;
    }
    if added > 0 {
        crate::register::write(&reg)?;
    }
    Ok(added)
}

fn read_plan(machine: &Path, name: &str) -> Vec<PathBuf> {
    std::fs::read_to_string(plan_path(&machine, name))
        .map(|t| t.lines().filter(|l| !l.is_empty()).map(PathBuf::from).collect())
        .unwrap_or_default()
}

/// What settling found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settled {
    /// Staged files that opened under the vault's key and took their place (the change had taken effect).
    pub forward: usize,
    /// Staged files that did not (the change never took effect) and were removed.
    pub back: usize,
    /// Staged files of the change that took effect but could not take their place now: said, and kept where
    /// they lie for the next unlock (the only copy under the new key).
    pub held: usize,
}

fn settle_in(root: &Path, doc_of: &dyn Fn(&str) -> Option<Doc>, key: &LocalKey, back_all: bool, out: &mut Settled, kept: &mut Vec<PathBuf>) -> Result<(), Fault> {
    let mut all = Vec::new();
    walk(root, "", &mut all);
    for (at, rel) in all {
        let Some(base) = unstaged(&rel) else { continue };
        // Not a kind of this pass (a home can sit inside the machine directory): its own pass settles it.
        let Some(doc) = doc_of(base) else { continue };
        let target = beside_strip(&at);
        let opened = if back_all { None } else { std::fs::read(&at).ok().and_then(|b| open_with(key, doc, &b, base).ok()).map(|p| (doc, p)) };
        match opened {
            Some((Doc::Machine, plain)) => {
                // Machine settings are kept plain: the staged copy proves it belongs to the change that took
                // effect, and its content is written plain.
                match crate::machine::write_bytes(&plain) {
                    Ok(()) => {
                        let _ = std::fs::remove_file(&at);
                        out.forward += 1;
                    }
                    // Said, and the pass goes on: one file that cannot take its place does not hold back the
                    // others. It stays staged (the sweep below spares it) and is tried again at the next unlock.
                    Err(f) => {
                        note_trouble(f);
                        kept.push(at.clone());
                        out.held += 1;
                    }
                }
            }
            Some((_, plain)) if plain == GONE => {
                // Staged as gone: the change removes this file.
                let _ = std::fs::remove_file(&target);
                let _ = std::fs::remove_file(&at);
                out.forward += 1;
            }
            Some(_) => match crate::home::rename_over(&at, &target) {
                Ok(()) => out.forward += 1,
                Err(f) => {
                    note_trouble(f);
                    kept.push(at.clone());
                    out.held += 1;
                }
            },
            None => {
                let _ = std::fs::remove_file(&at);
                out.back += 1;
            }
        }
    }
    Ok(())
}

fn beside_strip(at: &Path) -> PathBuf {
    let s = at.as_os_str().to_string_lossy().to_string();
    PathBuf::from(unstaged(&s).unwrap_or(&s))
}

/// The directory pruning stops at for a place that went: the home (or machine directory) it lay in.
fn homes_stop(at: &Path, homes: &[(Whose, PathBuf)], machine: &Path) -> PathBuf {
    homes.iter().map(|(_, h)| h).find(|h| at.starts_with(h)).cloned().unwrap_or_else(|| {
        // A home that moved is no longer among the homes: its seat directory, or the machine directory.
        at.ancestors().find(|a| identity_dir_of(a).is_some()).map(Path::to_path_buf).unwrap_or_else(|| machine.to_path_buf())
    })
}

/// On unlock, before anything local is read. A change stages its new vault first and renames it last, so a
/// staged vault still on disk means the change never took effect: it is removed and so is every staged file.
/// With none, the change took effect: every staged local file that opens under the vault's key takes its place
/// and every one that does not is removed. Machine directory first (the register may be staged), then every
/// home the register names (read as it lies: this runs before plain files of an older version are sealed).
pub fn settle_pending() -> Result<Settled, Fault> {
    let key = crate::keybox::local_key()?;
    // A swap a cut left half done is finished first; its own trouble is said, never holding back the landing
    // below (a change of master key may have its register still staged).
    if let Err(f) = settle_swap() {
        note_trouble(f);
    }
    let m = crate::home::machine_dir()?;
    // A migration's mark still down: it never took effect either (`PLAN_OPEN`).
    let never = crate::keybox::drop_staged()? | plan_there(&m, PLAN_OPEN);
    let mut out = Settled::default();
    let machine_doc_or = |rel: &str| -> Option<Doc> {
        if rel == crate::machine::FILE {
            Some(Doc::Machine)
        } else {
            machine_doc(rel)
        }
    };
    let mut kept: Vec<PathBuf> = Vec::new();
    let made = read_plan(&m, PLAN_MADE);
    let aside = read_plan(&m, PLAN_ASIDE);
    let gone = if never { Vec::new() } else { read_plan_sealed(&m, PLAN_GONE, &key) };
    let moved = if never { Vec::new() } else { read_plan_sealed(&m, PLAN_MOVED, &key) };
    if never {
        // The change never took effect: the homes it made in fresh places are nobody's (nothing refers to
        // them), and they go with its other staged files.
        for d in &made {
            if d.is_dir() {
                let _ = std::fs::remove_dir_all(d);
            }
        }
    }
    settle_in(&m, &machine_doc_or, &key, never, &mut out, &mut kept)?;
    let homes = homes_of(registry_any()?, false)?;
    for (_, h) in &homes {
        settle_in(h, &home_doc, &key, never, &mut out, &mut kept)?;
    }
    if !never && (!gone.is_empty() || !moved.is_empty()) {
        // A change of names took effect: its old places go (a file held above keeps its old one until the
        // next unlock lands it: the plan stays staged with it), then each moved home's rest follows it.
        let mut failed = 0usize;
        if out.held == 0 {
            for g in &gone {
                let at = PathBuf::from(g);
                match std::fs::remove_file(&at) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(_) => failed += 1,
                }
                if let Some(d) = at.parent() {
                    let stop = homes_stop(&at, &homes, &m);
                    prune_up(d, &stop);
                }
            }
            for line in &moved {
                if let Some((a, b)) = line.split_once('\t') {
                    failed += carry_home(Path::new(a), Path::new(b));
                }
            }
        }
        if out.held > 0 || failed > 0 {
            // Not finished: said, and the plan stays staged so the next unlock carries it out again (every
            // step of it is idempotent).
            if failed > 0 {
                note_trouble(Fault::known(Known::RestorePartial, format!("names · {failed}")));
            }
            for name in [PLAN_GONE, PLAN_MOVED] {
                kept.push(plan_path(&m, name));
            }
        }
    }
    let changed = !gone.is_empty() || !moved.is_empty() || !aside.is_empty();
    if !never && !aside.is_empty() {
        // It took effect: what the listed places hold that the new key cannot open is moved aside, said once.
        match set_aside_unopenable(aside, &key) {
            Ok((0, _)) => {}
            Ok((n, at)) => note_trouble(Fault::known(Known::LocalSetAside, format!("{n} · {}", at.map(|p| p.display().to_string()).unwrap_or_default()))),
            Err(f) => {
                // Said, and the plan stays staged: the next unlock sets aside what is still left (idempotent).
                note_trouble(f);
                kept.push(plan_path(&m, PLAN_ASIDE));
            }
        }
    }
    // A change that took effect and landed whole (a change of names, of master key, a restore): the end state
    // holds no directory named by an id that is no home on the register, whatever left it (`clear_strays`).
    if !never && changed && kept.is_empty() {
        match clear_strays(&key, false) {
            Ok((0, _)) => {}
            Ok((n, at)) => note_trouble(Fault::known(Known::LocalSetAside, format!("{n} · {}", at.map(|p| p.display().to_string()).unwrap_or_default()))),
            Err(f) => note_trouble(f),
        }
    }
    // Whatever staged file no pass claimed belongs to nothing that took effect (a held one was claimed).
    let mut left = Vec::new();
    walk(&m, "", &mut left);
    for (_, h) in &homes {
        walk(h, "", &mut left);
    }
    for (at, rel) in left {
        if is_staged(&rel) && !kept.contains(&at) && std::fs::remove_file(&at).is_ok() {
            out.back += 1;
        }
    }
    Ok(out)
}

// ───────────────────────── Right after the vault opens ─────────────────────────

/// What the passes after opening left to say (taken once by the shell where the unlock lands).
static OPENED: std::sync::Mutex<Option<Opened>> = std::sync::Mutex::new(None);

/// What the passes after opening found.
#[derive(Clone, Debug, Default)]
pub struct Opened {
    pub settled: Settled,
    pub migrated: usize,
    /// Local files moved to their keyed names (an older disk's first opening).
    pub renamed: usize,
    /// The primary identity this pass settled on an older vault (said once on screen).
    pub primary: Option<String>,
    pub troubles: Vec<Fault>,
}

/// Run right after the vault opens (unlock, reseal, recovery), in the same background task: settle staged
/// files of a master key change, seal plain files left by an older version, settle the primary identity of
/// an older vault. Each is refused by name into the answer; none undoes the opening.
pub fn after_open() {
    let mut o = Opened::default();
    match settle_pending() {
        Ok(s) => o.settled = s,
        Err(f) => o.troubles.push(f),
    }
    match migrate_plain() {
        Ok(m) => o.migrated = m.sealed,
        Err(f) => o.troubles.push(f),
    }
    match crate::identity::settle_primary() {
        Ok(p) => o.primary = p,
        Err(f) => o.troubles.push(f),
    }
    // The key store's own names: an older shape is upgraded in place (after the primary is settled), so its
    // file names nobody (`keybox::upgrade_names`).
    if let Err(f) = crate::keybox::upgrade_names() {
        o.troubles.push(f);
    }
    // Then the local files' names: an older disk's names that say identities, entries and grants are keyed
    // (`migrate_names`; a cut half way is picked up here at the next opening).
    match migrate_names() {
        Ok(n) => o.renamed = n,
        Err(f) => o.troubles.push(f),
    }
    // Merged, not replaced: what the passes noted while they ran (`note_trouble`) is said too.
    if let Ok(mut g) = OPENED.lock() {
        let was = g.get_or_insert_with(Opened::default);
        was.settled = o.settled;
        was.migrated = o.migrated;
        was.renamed = o.renamed;
        was.primary = o.primary;
        was.troubles.extend(o.troubles);
    }
}

/// Record a trouble for the shell to say where the next opening lands (a change that took effect but could not
/// settle every staged file now: the next unlock settles them).
pub fn note_trouble(f: Fault) {
    if let Ok(mut g) = OPENED.lock() {
        g.get_or_insert_with(Opened::default).troubles.push(f);
    }
}

/// Take what the passes after opening found (empty after taking).
pub fn take_opened() -> Option<Opened> {
    OPENED.lock().ok().and_then(|mut g| g.take())
}
