//! One archive directory ("home") per identity. The product creates everything it needs (directories,
//! subdirectories, the settings file); nobody has to prepare anything.
//!
//! Four subdirectories: `ledger` (the storage crate's `LedgerDir`), `kits`, `grants-held`, `settings`. This
//! module defines no ledger layout: opening, reading and writing go through the storage crate (any directory
//! is an archive; any copy is equivalent).
//!
//! No absolute path is stored anywhere in this tree, so copying it elsewhere or to another machine reads
//! back byte for byte. Where this machine's home is lives outside the home, in [`PICKED`] in the machine
//! directory.
//!
//! The machine directory, one per machine, holds the key vault, identity registry, `machine.json` and the kit
//! index `kits/index.json`. It is found through the pointer `~/.zikaron-desk` (one line: the machine
//! directory's absolute path, trailing newline, mode 0600, written to a temporary name and renamed
//! atomically); without a pointer, the default is `~/.zikaron-desk.d/`. Older versions kept it in
//! `~/Library/Application Support/ZIKARON`, with the chosen home in its `where.json`; that layout is migrated
//! once by [`settle_machine`] (the old home pointer moves into [`PICKED`]), and once the new pointer is
//! written the old files are never read again. Companion apps read this pointer and index (read-only).
//!
//! Resolving and settling are separate: [`machine_dir`] only reads; the pointer, migration and empty index
//! are written only by [`settle_machine`], called once when the window starts (tests call it separately in a
//! temporary place).

use crate::fault::{classify, Fault, Known};
use std::path::{Path, PathBuf};

/// The four subdirectories. Closed; their names are written only here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Ledger,
    Kits,
    GrantsHeld,
    Settings,
}

impl Slot {
    pub const ALL: [Slot; 4] = [Slot::Ledger, Slot::Kits, Slot::GrantsHeld, Slot::Settings];

    pub fn as_str(self) -> &'static str {
        match self {
            Slot::Ledger => "ledger",
            Slot::Kits => "kits",
            Slot::GrantsHeld => "grants-held",
            Slot::Settings => "settings",
        }
    }
}

/// The default machine directory (under the user's home) and the pointer file name: spelled in `zikaron-os`,
/// where the one reading of the machine directory lives (the command line reads it too, to find the door).
pub const APP_DIR: &str = zikaron_os::machine::APP_DIR;
pub const POINTER: &str = zikaron_os::machine::POINTER;
/// Default home name when nobody chose a place (under the machine directory), defined once.
pub const DEFAULT_HOME: &str = "default";
/// The older machine directory (these three segments under the user's home) and its file recording the chosen
/// home. Read only during the one migration.
pub const LEGACY_DIR: [&str; 3] = zikaron_os::machine::LEGACY_DIR;
pub const LEGACY_POINTER: &str = "where.json";
/// The file in the machine directory recording the chosen home (`{"home": absolute path}`, the same shape as
/// the old `where.json`), defined once.
pub const PICKED: &str = "picked-home.json";
/// Environment variable naming a home explicitly (used by tools and tests to start a home in a temporary
/// place).
pub const HOME_ENV: &str = "ZIKARON_DESK_HOME";



/// Files the system leaves in folders on its own (folder view settings, thumbnail caches). A folder holding
/// only these is empty to a person, and a home holding them is still a home.
pub const SIDE_FILES: [&str; 3] = [".DS_Store", "Thumbs.db", "desktop.ini"];
/// The prefix of the companion files some systems write beside each file copied to a foreign disk.
pub const SIDE_PREFIX: &str = "._";

/// Whether a name in a folder is one of the system's side files (never a person's file).
pub fn is_side_file(name: &str) -> bool {
    SIDE_FILES.contains(&name) || name.starts_with(SIDE_PREFIX)
}

/// What a folder is, to the one question "may a home open here?": not there yet (the product makes it);
/// empty (nothing, or only the system's side files); already a home (every name in it is one of the home's
/// rooms or a side file, at least one room there; an older home missing a room is still a home); or something
/// else (a ledger folder written by the command line, a folder of documents), with the names that make it so.
/// A name of a room standing as a file is the home's own (a damaged room), not a stranger's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Absent,
    Empty,
    Home,
    Other(Vec<String>),
}

/// Read what a folder is (see [`Place`]). "Change data folder" asks this before laying out rooms there: a home
/// chosen by a person never lays its rooms beside someone else's files.
pub fn place_of(root: &Path) -> Result<Place, Fault> {
    if !root.exists() {
        return Ok(Place::Absent);
    }
    if !root.is_dir() {
        return Ok(Place::Other(vec![root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()]));
    }
    let mut rooms = 0usize;
    let mut other: Vec<String> = Vec::new();
    for e in std::fs::read_dir(root).map_err(|e| classify(&e, &root.display().to_string()))? {
        let e = e.map_err(|e| classify(&e, &root.display().to_string()))?;
        let name = e.file_name().to_string_lossy().to_string();
        if is_side_file(&name) {
            continue;
        }
        // Anything at a room's name belongs to the home: a file there is a damaged room (reported as "cannot
        // create" when the rooms are laid out), not a stranger's file.
        if Slot::ALL.iter().any(|s| s.as_str() == name) {
            if e.path().is_dir() {
                rooms += 1;
            }
        } else {
            other.push(name);
        }
    }
    other.sort();
    Ok(match (rooms, other.is_empty()) {
        (_, false) => Place::Other(other),
        (0, true) => Place::Empty,
        (_, true) => Place::Home,
    })
}

/// A home may open here: refused by name ([`Known::NotAHome`], the first names that make it so) when the
/// folder is neither a home nor empty.
pub fn may_open_at(root: &Path) -> Result<(), Fault> {
    match place_of(root)? {
        Place::Absent | Place::Empty | Place::Home => Ok(()),
        Place::Other(names) => {
            let shown: Vec<&str> = names.iter().take(3).map(String::as_str).collect();
            Err(Fault::known(Known::NotAHome, format!("{}: {}", root.display(), shown.join(" "))))
        }
    }
}

/// Lays out a home: the root and four subdirectories.
///
/// Verified on disk afterwards: success means the rooms exist as the file system reports them, and any
/// failure is refused by name as "cannot create directory".
pub fn lay(root: &Path) -> Result<(), Fault> {
    // The create calls do not decide the outcome: `classify` recognizes only "missing" and "denied", so the
    // common case (the name taken by a file) would pass a raw system message through. Creation errors are
    // kept as the evidence tail, and the verdict is whether the rooms exist on disk now.
    let mut why: Vec<String> = Vec::new();
    if let Err(e) = std::fs::create_dir_all(root) {
        why.push(classify(&e, &root.display().to_string()).tail().to_string());
    }
    for s in Slot::ALL {
        let d = root.join(s.as_str());
        if let Err(e) = std::fs::create_dir_all(&d) {
            why.push(classify(&e, &d.display().to_string()).tail().to_string());
        }
    }
    let missing: Vec<&str> = Slot::ALL
        .iter()
        .filter(|s| !root.join(s.as_str()).is_dir())
        .map(|s| s.as_str())
        .collect();
    if !root.is_dir() || !missing.is_empty() {
        let what = if missing.is_empty() { crate::lang::t(crate::lang::Key::Tail159).to_string() } else { missing.join(" ") };
        let tail = if why.is_empty() {
            crate::lang::filln(crate::lang::Key::Tail016, &[&(root.display()).to_string(), &(what).to_string()])
        } else {
            crate::lang::filln(crate::lang::Key::Tail160, &[&(root.display()).to_string(), &(what).to_string(), &(why.join(" · ")).to_string()])
        };
        return Err(Fault::known(Known::CannotLay, tail));
    }
    Ok(())
}

/// Writes a small file into a room of the home. Every home file is written here.
///
/// Written beside, then renamed: a temporary name in the same room, `sync_all`, then rename over. Rename is
/// atomic on one file system, so a truncated file never appears. With this one write path, a power loss
/// mid-write cannot leave a half-written queue, checklist or settings file that silently loses data.
pub fn put(home: &Home, slot: Slot, name: &str, bytes: &[u8]) -> Result<(), Fault> {
    put_at(&home.dir(slot), name, bytes)
}

/// The same write into a given directory (machine-level files outside the home, such as the identity
/// registry). The only write path; [`put`] goes through it.
pub fn put_at(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), Fault> {
    // The writing task's ticket (`task::ticket_void`): a worker whose home is no longer the one it started
    // for writes nothing and ends with the refusal.
    if crate::task::ticket_void() {
        return Err(crate::task::void_ticket_fault(dir));
    }
    use std::io::Write;
    let dir = dir.to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| classify(&e, &dir.display().to_string()))?;
    // The temporary name must be unique: two paths in one process may write the same file at once (queueing
    // in the frame, dequeuing in the background), and a shared temporary name would mix their bytes.
    let tmp = dir.join(put_tmp_name(name));
    let p = dir.join(name);
    {
        let mut f = open_owner_only(&tmp)?;
        f.write_all(bytes).map_err(|e| classify(&e, &tmp.display().to_string()))?;
        f.sync_all().map_err(|e| classify(&e, &tmp.display().to_string()))?;
    }
    replace_lasting(&tmp, &p)
}

/// Renames `from` over `to` in one step, then syncs the directory (`zikaron_os::sync_dir`) so the new name
/// survives a power cut; otherwise the directory could still name the old file and the last write of a
/// setting, queue or pointer would come back as the one before. A failed rename is reported with the target,
/// a failed sync with the directory (the file is in place, but the write is not taken as done). The app's
/// only rename-and-sync: [`put_at`] and [`rename_over`] both end here.
fn replace_lasting(from: &Path, to: &Path) -> Result<(), Fault> {
    zikaron_os::replace(from, to).map_err(|e| classify(&e, &to.display().to_string()))?;
    let dir = to.parent().unwrap_or_else(|| Path::new("."));
    zikaron_os::sync_dir(dir, to).map_err(|e| classify(&e, &dir.display().to_string()))
}

/// The temporary name [`put_at`] writes beside its target: `.{name}.{pid}.{nanoseconds}.tmp`.
fn put_tmp_name(name: &str) -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!(".{name}.{}.{nanos}{PUT_TMP_SUFFIX}", std::process::id())
}

const PUT_TMP_SUFFIX: &str = ".tmp";

/// Whether a name is one [`put_tmp_name`] writes, by the name alone.
fn is_put_tmp(name: &str) -> bool {
    let Some(rest) = name.strip_prefix('.').and_then(|r| r.strip_suffix(PUT_TMP_SUFFIX)) else { return false };
    let mut parts = rest.rsplitn(3, '.');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(nanos), Some(pid), Some(base)) => {
            !base.is_empty() && !nanos.is_empty() && nanos.bytes().all(|b| b.is_ascii_digit()) && !pid.is_empty() && pid.bytes().all(|b| b.is_ascii_digit())
        }
        _ => false,
    }
}

/// Whether a file name is one of this product's temporary names, by the name alone: what [`put_at`], the glue
/// crate's landing (`landing::is_beside_name`) and the store crate's archive writes (`layout::tmp_shaped`)
/// put beside a target before renaming it over. A leftover one is from a write cut short. A file staged for a
/// later commit (`keybox::NEXT`) is not temporary: it is settled at the next start.
pub fn is_temp_name(name: &str) -> bool {
    is_put_tmp(name) || zikaron_glue::landing::is_beside_name(name) || zikaron_store::layout::tmp_shaped(name)
}

/// Moves a file written beside its place (a staged `.zk-next` or a sealed copy, itself written by [`put_at`])
/// over that place in one rename. The only rename of a written file outside `put_at`: the vault change, the
/// settling of staged files and the plain-file migration all go through it.
pub fn rename_over(from: &Path, to: &Path) -> Result<(), Fault> {
    replace_lasting(from, to)
}

/// Creates a new file readable and writable by its owner only.
///
/// Used for this machine's private files: the key vault (key ciphertext and recovery seals), the identity
/// registry, the queue, the checklist, settings. A plain create follows the system's defaults, often
/// readable by other accounts; the files hold no plaintext, but this is one more layer. The owner-only mode is
/// set at creation (`zikaron_os::owner_only`), so there is no moment when others may read the file, and
/// replacing keeps it. `create_new` refuses a name collision instead of truncating someone else's file (the
/// temporary name already carries pid and nanoseconds).
fn open_owner_only(tmp: &Path) -> Result<std::fs::File, Fault> {
    let mut o = zikaron_os::Options::new();
    o.write(true).create_new(true);
    zikaron_os::owner_only(&mut o);
    o.open(tmp).map_err(|e| classify(&e, &tmp.display().to_string()))
}

/// An archive directory.
pub struct Home {
    root: PathBuf,
}

impl Home {
    /// Opens a home, creating it and its four subdirectories if missing.
    pub fn open_or_create(root: impl Into<PathBuf>) -> Result<Home, Fault> {
        let root = root.into();
        lay(&root)?;
        Ok(Home { root })
    }

    /// Opens an existing home. Missing subdirectories are reported, never silently added.
    pub fn open(root: impl Into<PathBuf>) -> Result<Home, Fault> {
        let root = root.into();
        if !root.is_dir() {
            return Err(Fault::known(Known::FileMissing, root.display().to_string()));
        }
        Ok(Home { root })
    }

    /// A home not yet on disk: only its path, without asking whether it exists. Used where the screen asks
    /// whether the mirror location exists before a home is open.
    pub fn bare(root: PathBuf) -> Home {
        Home { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn dir(&self, s: Slot) -> PathBuf {
        self.root.join(s.as_str())
    }

    /// Which subdirectories are missing, read from disk now.
    pub fn missing(&self) -> Vec<&'static str> {
        Slot::ALL
            .iter()
            .filter(|s| !self.dir(**s).is_dir())
            .map(|s| s.as_str())
            .collect()
    }

    /// The ledger: the storage crate's `LedgerDir` holding sealed entries, opened on the way out
    /// (`local::Ledger`); this layer defines no layout of its own.
    pub fn ledger(&self) -> Result<crate::local::Ledger, Fault> {
        crate::local::ledger_of(self)
    }

    /// Bytes this home uses now, by walking the disk (not estimated).
    pub fn usage(&self) -> Result<u64, Fault> {
        walk_size(&self.root)
    }
}

fn walk_size(p: &Path) -> Result<u64, Fault> {
    let md = std::fs::symlink_metadata(p).map_err(|e| classify(&e, &p.display().to_string()))?;
    if md.is_file() {
        return Ok(md.len());
    }
    if !md.is_dir() {
        return Ok(0);
    }
    let mut n = 0;
    for e in std::fs::read_dir(p).map_err(|e| classify(&e, &p.display().to_string()))? {
        let e = e.map_err(|e| classify(&e, &p.display().to_string()))?;
        n += walk_size(&e.path())?;
    }
    Ok(n)
}

/// Where an identity's seat home lives. A new identity's two seat homes sit beside this machine's current
/// home: `<home's parent>/<keyed name>/<seat>`. The name derives from the identity, so deleting and
/// reimporting an identity finds the same two homes (deleting an identity does not delete ledger
/// directories). This path is built only here.
pub fn identity_home(identity: &str, seat: crate::roles::Role) -> Result<PathBuf, Fault> {
    identity_home_under(&crate::names::key()?, identity, seat)
}

/// The same place under a given names key (a master key change names the homes it lays down anew).
pub fn identity_home_under(nk: &crate::names::NameKey, identity: &str, seat: crate::roles::Role) -> Result<PathBuf, Fault> {
    let here = where_is()?;
    let base = match here.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => machine_dir()?,
    };
    Ok(base.join(home_dir_name(nk, identity)).join(seat.as_str()))
}

/// An identity's home directory name under a names key. The name is keyed (`names`), so while locked it
/// reveals nothing about the identity. Tests read it from here rather than computing it.
pub fn home_dir_name(nk: &crate::names::NameKey, identity: &str) -> String {
    nk.name(crate::names::Logical::Home(identity))
}

/// The user's home directory (the pointer, default machine directory and old location derive from it). A
/// stand-in set through `places` wins; with the machine directory set and no stand-in (usual in tests) it is
/// `None` and the pointer is neither read nor written. Otherwise the system's home directory
/// (`platform::home_dir`).
pub fn user_home() -> Result<Option<PathBuf>, Fault> {
    let p = crate::places::get();
    if let Some(u) = p.user_home.clone() {
        return Ok(Some(u));
    }
    if p.machine_dir.is_some() {
        return Ok(None);
    }
    crate::platform::home_dir()
        .map(Some)
        .ok_or_else(|| Fault::known(Known::NoHomeDir, crate::lang::t(crate::lang::Key::Tail161).to_string()))
}

/// Where the pointer file is (`None`: this run does not touch the pointer, see [`user_home`]).
pub fn pointer_path() -> Result<Option<PathBuf>, Fault> {
    Ok(user_home()?.map(|u| pointer_dir(&u).join(POINTER)))
}

/// The folder the pointer file sits in, given the user's home directory: where the system keeps this app's
/// machine data by default (`platform::app_data_dir`; on macOS and Linux the home directory itself), beside the
/// default machine directory and outside it.
pub fn pointer_dir(user_home: &Path) -> PathBuf {
    crate::platform::app_data_dir(user_home)
}

/// Where the older machine directory is (as above).
pub fn legacy_dir() -> Result<Option<PathBuf>, Fault> {
    Ok(user_home()?.map(|u| LEGACY_DIR.iter().fold(u, |p, s| p.join(s))))
}

/// Which level the machine directory was resolved from. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    /// Set through `places` (only tests set it).
    Placed,
    /// The pointer `~/.zikaron-desk`.
    Pointer,
    /// No pointer, and the older machine directory is on disk: a machine not yet migrated (the migration is
    /// written by [`settle_machine`]).
    Legacy,
    /// The default `~/.zikaron-desk.d/`.
    Default,
}

impl Layer {
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Placed => "placed",
            Layer::Pointer => "pointer",
            Layer::Legacy => "legacy",
            Layer::Default => "default",
        }
    }
}

/// Where the machine directory is and which level it came from. Reads only. Order: set through `places`,
/// then the pointer, then the older machine directory (not migrated), then the default. A pointer that is not
/// one absolute path line is refused with `MACHINE_SHAPE`, never silently replaced by the default (which
/// would leave the key vault and registry behind).
pub fn machine_at() -> Result<(PathBuf, Layer), Fault> {
    if let Some(p) = crate::places::get().machine_dir.clone() {
        return Ok((p, Layer::Placed));
    }
    let Some(u) = user_home()? else {
        return Err(Fault::known(Known::NoHomeDir, crate::lang::t(crate::lang::Key::Tail161).to_string()));
    };
    // The remaining levels are read in `zikaron-os`, shared with the command line.
    use zikaron_os::machine::{self, Unread};
    match machine::of(&u) {
        Ok((p, machine::Layer::Pointer)) => Ok((p, Layer::Pointer)),
        Ok((p, machine::Layer::Legacy)) => Ok((p, Layer::Legacy)),
        Ok((p, machine::Layer::Default)) => Ok((p, Layer::Default)),
        Err(Unread::PointerShape(ptr)) => Err(Fault::known(Known::MachineShape, ptr.display().to_string())),
        Err(Unread::Pointer(ptr, e)) => Err(classify(&e, &ptr.display().to_string())),
        Err(Unread::NoHome) => Err(Fault::known(Known::NoHomeDir, crate::lang::t(crate::lang::Key::Tail161).to_string())),
    }
}

/// The machine directory (where the key vault, registry, `machine.json` and kit index live). See
/// [`machine_at`].
pub fn machine_dir() -> Result<PathBuf, Fault> {
    machine_at().map(|(p, _)| p)
}

/// Where this machine's home is: named by the environment, then the choice recorded in the machine directory
/// ([`PICKED`]), then the old pointer on a machine not yet migrated, then the default home under the machine
/// directory. An unreadable record is refused by name; no path is ever guessed.
pub fn where_is() -> Result<PathBuf, Fault> {
    if let Some(p) = std::env::var_os(HOME_ENV) {
        let p = PathBuf::from(p);
        if !p.as_os_str().is_empty() {
            return Ok(p);
        }
    }
    let (machine, layer) = machine_at()?;
    // An unreadable recorded choice is refused by name: silently opening the default home would make the
    // ledger look lost, and the next home change would write the wrong path back.
    let picked = machine.join(PICKED);
    match std::fs::read(&picked) {
        Ok(b) => return read_pointer(&b).ok_or_else(|| Fault::known(Known::MachineShape, picked.display().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(classify(&e, &picked.display().to_string())),
    }
    if layer == Layer::Legacy {
        if let Some(p) = legacy_pick(&machine)? {
            return Ok(p);
        }
    }
    Ok(machine.join(DEFAULT_HOME))
}

/// The home the old pointer records on a machine not yet migrated. `None` without an old pointer; an
/// unreadable one is refused by name (falling back to the default would make the ledger look lost and make
/// the migration record "none" permanently).
fn legacy_pick(machine: &Path) -> Result<Option<PathBuf>, Fault> {
    let p = machine.join(LEGACY_POINTER);
    match std::fs::read(&p) {
        Ok(b) => read_pointer(&b).map(Some).ok_or_else(|| Fault::known(Known::MachineShape, p.display().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(classify(&e, &p.display().to_string())),
    }
}

/// One pointer line: the machine directory's absolute path followed by exactly one newline (parsed in
/// `zikaron-os`).
fn read_machine_pointer(bytes: &[u8]) -> Option<PathBuf> {
    zikaron_os::machine::read_pointer(bytes)
}

/// The pointer file bytes (one absolute path line with a newline), matching what the reader expects.
pub fn machine_pointer_bytes(machine: &Path) -> Vec<u8> {
    format!("{}\n", machine.display()).into_bytes()
}

/// Writes the pointer through [`put_at`] (temporary name, atomic rename, 0600). Not rewritten when the bytes
/// are already this place. Returns whether it wrote; with no user-home stand-in in tests it writes nothing.
pub fn write_machine_pointer(machine: &Path) -> Result<bool, Fault> {
    let Some(u) = user_home()? else { return Ok(false) };
    let want = machine_pointer_bytes(machine);
    // Check with the reader before writing: a path that does not round-trip through these bytes (a line break
    // in it, or bytes a text line cannot carry) is refused by name, never written as a pointer that a later
    // start could not read or that leads elsewhere.
    if read_machine_pointer(&want).as_deref() != Some(machine) {
        return Err(Fault::known(Known::MachineShape, machine.display().to_string()));
    }
    let dir = pointer_dir(&u);
    if std::fs::read(dir.join(POINTER)).ok().as_deref() == Some(want.as_slice()) {
        return Ok(false);
    }
    put_at(&dir, POINTER, &want)?;
    Ok(true)
}

/// What was settled when the window started (read by tests).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settled {
    pub machine: PathBuf,
    /// Which level it resolved from before this run.
    pub layer: Layer,
    /// Whether this run wrote the pointer.
    pub pointer_written: bool,
    /// The home the old pointer recorded, moved into [`PICKED`] this run (only during migration).
    pub migrated_home: Option<PathBuf>,
    /// Whether this run wrote an empty index.
    pub index_written: bool,
}

/// Settles the machine directory once when the window starts (tests call it separately in a temporary
/// place):
///
/// 1. With no pointer and the older machine directory on disk, that stays the machine directory (key vault
///    and registry stay where they are), and the home recorded in the old `where.json` moves into
///    [`PICKED`] (a one-time migration);
/// 2. the machine directory is created and the pointer written to it (not rewritten when unchanged);
/// 3. an empty kit index is written when missing.
///
/// Once the pointer exists the old locations ([`LEGACY_DIR`], [`LEGACY_POINTER`]) are never read again:
/// [`machine_at`] sees the pointer first.
pub fn settle_machine() -> Result<Settled, Fault> {
    let (machine, layer) = machine_at()?;
    let before = pointer_path()?.and_then(|p| std::fs::read(p).ok());
    let mut migrated_home = None;
    if layer == Layer::Legacy {
        if let Some(p) = legacy_pick(&machine)? {
            if !machine.join(PICKED).exists() {
                // The home is recorded only through `write_pointer` (which also writes the machine pointer).
                write_pointer(&p)?;
                migrated_home = Some(p);
            }
        }
    }
    std::fs::create_dir_all(&machine).map_err(|e| classify(&e, &machine.display().to_string()))?;
    if layer != Layer::Placed {
        write_machine_pointer(&machine)?;
    }
    let pointer_written = pointer_path()?.and_then(|p| std::fs::read(p).ok()) != before;
    let index_written = crate::kitsindex::ensure(&machine)?;
    Ok(Settled { machine, layer, pointer_written, migrated_home, index_written })
}

/// Whether this home was named by the environment. Such a home was not chosen in the product: the pointer
/// follows only the user's choices, and a home the environment named once must not replace this machine's
/// default home.
///
/// When both exist on disk, paths are compared canonically (`/tmp` and `/private/tmp` are the same place);
/// otherwise as given.
pub fn named_by_env(root: &Path) -> bool {
    let Some(p) = std::env::var_os(HOME_ENV) else { return false };
    let p = PathBuf::from(p);
    if p.as_os_str().is_empty() {
        return false;
    }
    same_place(&p, root)
}

/// A canonical path as people and other programs read it. On Windows canonicalizing gives the extended form
/// (`\\?\C:\x`, `\\?\UNC\host\share\x`); that prefix is removed (`C:\x`, `\\host\share\x`). Any other path,
/// including every unix one, is returned unchanged. Paths that are stored or shown pass through here; paths
/// only compared with other canonical paths need not.
pub fn plain_path(p: PathBuf) -> PathBuf {
    let Some(t) = p.to_str() else { return p };
    if let Some(rest) = t.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match t.strip_prefix(r"\\?\") {
        // Only a drive path is given back plainly; other extended forms (a volume by its identifier) have no
        // plain spelling.
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') && rest.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) => PathBuf::from(rest),
        _ => p,
    }
}

/// Whether two paths are the same place on disk. When both exist, compared by canonical path (`/tmp` and
/// `/private/tmp` are the same place); otherwise as given. Lock reuse, signing key ownership and
/// environment-named homes all ask it.
pub fn same_place(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

// Choosing where output lands is decided in one place: [`choose`]. It also says why the result differs from
// the picked place ([`Why`], three closed forms), so an unusable pick never leads to a silent subfolder. The
// chosen path is valid only while it does not exist.

/// What kind of thing is landing. Closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Moving a home: the user picks a folder. If empty, move into it; otherwise open a new name inside it
    /// (from [`HOME_STEM`]).
    Home,
    /// Writing a bundle (kit, badge): a named file is used as is; a named folder gets a name from `stem`
    /// inside it, numbered when taken.
    Bundle { stem: String },
    /// Writing a file: `<stem>.<ext>` inside the chosen folder, numbered when taken.
    File { stem: String, ext: String },
}

/// The folder name opened inside a non-empty folder when moving a home.
pub const HOME_STEM: &str = "ZIKARON";

/// Why output lands where it does. Three closed forms (the page words each one).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Why {
    /// The chosen place fits (an empty folder, a name not yet taken, or a named file).
    AsPicked,
    /// The chosen folder already has content, so a new name was opened inside it.
    FolderNotEmpty,
    /// The name was taken, so a number was added.
    NameTaken,
}

impl Why {
    pub fn as_str(self) -> &'static str {
        match self {
            Why::AsPicked => "as-picked",
            Why::FolderNotEmpty => "folder-not-empty",
            Why::NameTaken => "name-taken",
        }
    }

    /// The sentence on screen (`None` for the form that needs no explanation: landing where chosen).
    pub fn say(self) -> Option<crate::lang::Key> {
        match self {
            Why::AsPicked => None,
            Why::FolderNotEmpty => Some(crate::lang::Key::LandingFolderNotEmpty),
            Why::NameTaken => Some(crate::lang::Key::LandingNameTaken),
        }
    }

    /// The same, said before the write: where it will land once the key is pressed.
    pub fn say_ahead(self) -> Option<crate::lang::Key> {
        match self {
            Why::AsPicked => None,
            Why::FolderNotEmpty => Some(crate::lang::Key::LandingAheadFolderNotEmpty),
            Why::NameTaken => Some(crate::lang::Key::LandingAheadNameTaken),
        }
    }
}

/// The answer to where it lands, and why that is not the chosen place.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chosen {
    pub at: PathBuf,
    pub why: Why,
}

/// The one check on a location given by the user: empty or not absolute is refused with `PATH_RELATIVE`
/// before any write.
///
/// A relative path follows the process's current directory: homes, kits, backups, snapshots, badges, key
/// files and grant files would land wherever the program happens to run from, and a recorded pointer would
/// lead elsewhere next time. Every action that takes a location asks this, as does the window when enabling
/// its primary buttons. It touches no disk, so it is cheap enough for every frame.
pub fn landing(given: &str) -> Result<PathBuf, Fault> {
    let t = given.trim();
    let p = Path::new(t);
    if t.is_empty() || !p.is_absolute() {
        return Err(Fault::known(Known::PathRelative, given.to_string()));
    }
    Ok(p.to_path_buf())
}

/// The one place a path stored in settings becomes absolute. The window's picker already gives absolute
/// paths; relative ones (from tests or the command line) are resolved against the current directory now,
/// without touching the disk or requiring the place to exist. A stored relative path would point elsewhere
/// when started from another directory while the settings bytes stayed the same. An empty string is refused
/// with `PATH_RELATIVE`; "empty means clear" is the caller's decision.
pub fn kept(given: &str) -> Result<PathBuf, Fault> {
    let t = given.trim();
    if t.is_empty() {
        return Err(Fault::known(Known::PathRelative, given.to_string()));
    }
    std::path::absolute(t).map_err(|e| classify(&e, t))
}

/// Decides where output lands; the only place that does.
///
/// All three kinds follow one rule: a named file is used; a named folder gets a name inside it, and "it
/// already has content" and "the name is taken" are each reported. The result is valid only while that path
/// does not exist.
pub fn choose(kind: &Kind, picked: &Path) -> Chosen {
    let chosen = match kind {
        // Moving a home: an empty folder is used; otherwise a new name inside it.
        Kind::Home => {
            let empty = std::fs::read_dir(picked).map(|mut d| d.next().is_none()).unwrap_or(false);
            if empty {
                Chosen { at: picked.to_path_buf(), why: Why::AsPicked }
            } else {
                numbered(picked, HOME_STEM, None, Why::FolderNotEmpty)
            }
        }
        // A bundle: a named file is used as is (not a directory).
        Kind::Bundle { stem } => {
            if !picked.is_dir() {
                Chosen { at: picked.to_path_buf(), why: Why::AsPicked }
            } else {
                numbered(picked, stem, None, Why::AsPicked)
            }
        }
        Kind::File { stem, ext } => numbered(picked, stem, Some(ext), Why::AsPicked),
    };
    chosen
}

impl Chosen {
    /// The second check: whether the path is free now.
    ///
    /// New names from [`numbered`] are free by construction, while the picked place is expected to exist (an
    /// empty folder or a named file). So this answers rather than asserts, for callers that re-check (only
    /// tests; the product just takes the path).
    pub fn free(&self) -> bool {
        !self.at.exists()
    }

    /// Whether this is a new name (only when the chosen place could not be used).
    pub fn is_new_name(&self) -> bool {
        self.why != Why::AsPicked
    }
}

/// Makes a landing name inside a folder, numbered when taken.
///
/// `first` is the reason used when the first name is free: the two writers give `AsPicked`, while moving a
/// home already knows the folder is not empty and gives `FolderNotEmpty`. A taken first name gives
/// `NameTaken`.
fn numbered(dir: &Path, stem: &str, ext: Option<&str>, first: Why) -> Chosen {
    let mut n = 1u32;
    loop {
        let p = dir.join(numbered_name(stem, ext, n));
        if !p.exists() {
            return Chosen { at: p, why: if n == 1 { first } else { Why::NameTaken } };
        }
        n += 1;
    }
}

/// The `n`th landing name for a stem: the stem itself first, then `stem-2`, `stem-3`… (the extension after).
fn numbered_name(stem: &str, ext: Option<&str>, n: u32) -> String {
    match (n, ext) {
        (1, None) => stem.to_string(),
        (1, Some(e)) => format!("{stem}.{e}"),
        (k, None) => format!("{stem}-{k}"),
        (k, Some(e)) => format!("{stem}-{k}.{e}"),
    }
}

/// Writes `bytes` (owner only) as a new file in `dir` under the first free numbered name (`stem.ext`, then
/// `stem-2.ext`…) and returns its path. The write never replaces an existing entry (`zikaron_glue::landing`: a
/// hard link or a non-replacing rename); a name found taken at that moment, by anything, moves on to the next
/// number. Any other failure is refused by name.
pub fn land_numbered(dir: &Path, stem: &str, ext: &str, bytes: &[u8]) -> Result<PathBuf, Fault> {
    if crate::task::ticket_void() {
        return Err(crate::task::void_ticket_fault(dir));
    }
    std::fs::create_dir_all(dir).map_err(|e| classify(&e, &dir.display().to_string()))?;
    let mut n = 1u32;
    loop {
        let p = dir.join(numbered_name(stem, Some(ext), n));
        // `symlink_metadata`: a link at the name (even a broken one) holds it too.
        if std::fs::symlink_metadata(&p).is_err() {
            match zikaron_glue::landing::land_bytes_for(zikaron_glue::landing::Readers::Owner, &p, bytes) {
                Ok(()) => return Ok(p),
                Err(zikaron_glue::landing::Trouble::Occupied(_)) => {}
                Err(t) => return Err(Fault::of_landing(t)),
            }
        }
        n += 1;
    }
}

/// Records the chosen home ([`PICKED`] in the machine directory): the only record of the home's absolute
/// path, kept outside the home so copying the home does not carry it. Written by atomic rename, 0600. The
/// machine pointer is written at the same time (not rewritten when unchanged), so companion apps always find
/// the current machine directory.
pub fn write_pointer(home: &Path) -> Result<(), Fault> {
    let machine = machine_dir()?;
    // Every folder this machine has pointed at is remembered, so a later master key change or backup still
    // reaches it after the pointer moves on: the one being left (the default, or one chosen by an older
    // version that never passed through here) and the new one. Remembered before the pointer moves, so a
    // failure leaves the pointer where it was.
    if let Ok(was) = where_is() {
        if was.is_dir() {
            crate::machine::remember_home(&was)?;
        }
    }
    crate::machine::remember_home(home)?;
    // Canonical value `{"home": path}`; read by `read_pointer`.
    let doc = zikaron::json::Value::Obj(vec![(
        POINTER_MEMBER.to_string(),
        zikaron::json::Value::Str(home.display().to_string()),
    )]);
    let bytes = zikaron::json::canon_bytes(&doc);
    // Check with the reader before writing, as for the machine pointer: a path that does not round-trip
    // through these bytes is refused by name.
    if read_pointer(&bytes).as_deref() != Some(home) {
        return Err(Fault::known(Known::MachineShape, home.display().to_string()));
    }
    put_at(&machine, PICKED, &bytes)?;
    if machine_at()?.1 != Layer::Placed {
        write_machine_pointer(&machine)?;
    }
    Ok(())
}

/// The pointer's one member (`{"home": path}`).
pub const POINTER_MEMBER: &str = "home";

/// Reads the `{"home": path}` shape (used by [`PICKED`] and the old `where.json`; the old file is read only
/// during migration and on machines not yet migrated).
pub fn read_pointer(bytes: &[u8]) -> Option<PathBuf> {
    let v = zikaron::json::parse(bytes).ok()?;
    match v {
        zikaron::json::Value::Obj(m) => m.iter().find(|(k, _)| k == POINTER_MEMBER).and_then(|(_, x)| match x {
            zikaron::json::Value::Str(s) if !s.is_empty() => Some(PathBuf::from(s)),
            _ => None,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_temporary_name_this_product_writes_is_read_as_one() {
        let put = super::put_tmp_name("settings.json");
        let landing = zikaron_glue::landing::staging_beside(&std::env::temp_dir().join(std::path::Path::new("kit"))).expect("a temporary name").file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let store = zikaron_store::layout::tmp_file_name("0123456789abcdef");
        for n in [&put, &landing, &store] {
            assert!(super::is_temp_name(n), "{n} is ours");
        }
        for n in ["settings.json", ".DS_Store", ".settings.json.12.tmp.bak", "a.tmp", ".kit.staging-x-0123456789abcdef", ".zks-tmp-xyz"] {
            assert!(!super::is_temp_name(n), "{n} is not ours");
        }
    }

    /// Removes the extended prefix Windows canonicalization adds (shares back to their plain form); plain,
    /// unix and extended paths with no plain spelling come back unchanged.
    #[test]
    fn a_canonical_path_is_shown_plainly() {
        let plain = |s: &str| super::plain_path(std::path::PathBuf::from(s)).to_string_lossy().into_owned();
        assert_eq!(plain(r"\\?\C:\Users\a\kit"), r"C:\Users\a\kit");
        assert_eq!(plain(r"\\?\UNC\host\share\kit"), r"\\host\share\kit");
        assert_eq!(plain(r"C:\Users\a\kit"), r"C:\Users\a\kit");
        assert_eq!(plain(r"\\host\share\kit"), r"\\host\share\kit");
        assert_eq!(plain("/private/tmp/kit"), "/private/tmp/kit");
        assert_eq!(plain(r"\\?\Volume{0b1c}\kit"), r"\\?\Volume{0b1c}\kit");
    }
}
