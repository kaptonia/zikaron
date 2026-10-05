//! One archive directory per identity. The product creates everything it needs: directories, subdirectories
//! and the settings file; nobody has to prepare anything.
//!
//! Four subdirectories: `ledger` (the ledger, which is the storage crate's `LedgerDir`), `kits`,
//! `grants-held`, `settings`. This file invents no ledger layout: opening, reading and writing it go through
//! the storage crate (any directory is an archive; any copy is equivalent).
//!
//! No absolute path is written anywhere in this tree: the settings file does not record where it is, nor does
//! the ledger. So copying the whole tree elsewhere or to another machine reads back byte for byte, because
//! nothing stores a path. Where this machine's home is lives outside the home: [`PICKED`] in the machine
//! directory, which stays with the machine.
//!
//! The machine directory: one per machine (key vault, identity registry, `machine.json`, kit index
//! `kits/index.json`). It is found through the pointer `~/.zikaron-desk` (one line, the machine directory's
//! absolute path, trailing newline, 0600, written to a temporary name and renamed atomically); without a
//! pointer, the default `~/.zikaron-desk.d/`. Older machines kept it in `~/Library/Application
//! Support/ZIKARON` with the chosen home in its `where.json`: that is migrated once ([`settle_machine`]: old
//! without new, read the old and write the new, moving the old pointer's home into [`PICKED`]); once the
//! pointer is written the old two are never read again. Companion apps read this pointer and index
//! (read-only).
//!
//! Resolving (reading) and settling (writing) are separate: [`machine_dir`] only reads; the pointer,
//! migration and empty index are written only by [`settle_machine`], called once when the shipped window
//! starts (the test hooks call it separately in a temporary place).

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

/// The default machine directory (under the user's home) and the pointer file name, written only here.
pub const APP_DIR: &str = ".zikaron-desk.d";
pub const POINTER: &str = ".zikaron-desk";
/// Default home name when nobody chose a place (under the machine directory), defined once.
pub const DEFAULT_HOME: &str = "default";
/// The older machine directory (these three segments under the user's home) and its file recording the chosen
/// home. Read only during the one migration.
pub const LEGACY_DIR: [&str; 3] = ["Library", "Application Support", "ZIKARON"];
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
        // A room's name is the home's whatever stands there: a file in a room's place is a damaged room,
        // which laying out the rooms names ("cannot create"), not a stranger's file.
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

/// Lay out a home: the root and four subdirectories, so the product creates what it needs.
///
/// Checked on disk afterwards: returning no error does not mean anything landed. Success means the rooms
/// exist, as the file system answers, and failure to create them is refused by name as "cannot create
/// directory".
pub fn lay(root: &Path) -> Result<(), Fault> {
    // The create calls do not decide the outcome. With a `?` after each `create_dir_all`, `classify`
    // recognizes only "missing" and "denied"; the most common case (the name taken by a file) would fall to
    // the unknown branch and pass a system sentence through instead of "kits is missing". So the disk read is
    // the only judge: creation errors are kept as the evidence tail, and the verdict is whether the rooms
    // exist now. Every cause of "cannot create" leaves by the same named refusal.
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

/// Write a small file into a room of the home. Every home file is written here.
///
/// Written beside, then renamed: a temporary name in the same room, `sync_all`, then rename over. Rename is
/// atomic on one file system, so a truncated file cannot appear on disk.
///
/// With separate `fs::write` calls for queue, checklist and settings, a power loss mid-write left half a file
/// whose parse error was swallowed upstream, silently losing queued entries. There is no second way to write
/// here, so half-written files cannot occur.
pub fn put(home: &Home, slot: Slot, name: &str, bytes: &[u8]) -> Result<(), Fault> {
    put_at(&home.dir(slot), name, bytes)
}

/// The same write into a named directory (machine-level files outside the home, such as the identity
/// registry). The only way to write; `put` goes through it.
pub fn put_at(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), Fault> {
    use std::io::Write;
    let dir = dir.to_path_buf();
    std::fs::create_dir_all(&dir).map_err(|e| classify(&e, &dir.display().to_string()))?;
    // The temporary name must be unique: two paths in one process may write the same file at once (queueing
    // in the frame, dequeuing in the background), and a shared temporary name would mix their bytes before
    // each renamed over the target.
    let tmp = dir.join(put_tmp_name(name));
    let p = dir.join(name);
    {
        let mut f = open_owner_only(&tmp)?;
        f.write_all(bytes).map_err(|e| classify(&e, &tmp.display().to_string()))?;
        f.sync_all().map_err(|e| classify(&e, &tmp.display().to_string()))?;
    }
    zikaron_os::replace(&tmp, &p).map_err(|e| classify(&e, &p.display().to_string()))
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

/// Whether a file name is one of this product's temporary names, by the name alone: what this app's one write
/// ([`put_at`]), the glue crate's landing (`landing::is_beside_name`) and the store crate's archive writes
/// (`layout::tmp_shaped`) put beside a target before renaming it over. A leftover one is what a write cut short
/// left. A file staged for a later commit (`keybox::NEXT`) is not a temporary: it is settled at the next start.
pub fn is_temp_name(name: &str) -> bool {
    is_put_tmp(name) || zikaron_glue::landing::is_beside_name(name) || zikaron_store::layout::tmp_shaped(name)
}

/// Move a file written beside its place (a staged `.zk-next` or a sealed copy, itself written by [`put_at`])
/// over that place, in one rename. The only rename of a written file outside `put_at`: the vault change, the
/// settling of staged files and the plain-file migration all take effect through it.
pub fn rename_over(from: &Path, to: &Path) -> Result<(), Fault> {
    zikaron_os::replace(from, to).map_err(|e| classify(&e, &to.display().to_string()))
}

/// Create a new file readable and writable by its owner only.
///
/// This handles private things of this machine: the key vault (every key's ciphertext and the recovery
/// seals), the identity registry, the queue, the checklist, settings. A plain create follows the system's
/// defaults, commonly readable by other accounts on the same machine. The files hold no plain text; this is
/// one more layer. The owner-only rule is set on the creation itself (`zikaron_os::owner_only`), so the
/// temporary file is owner-only from the moment it exists, and replacing keeps it: there is no moment when
/// others may read it. `create_new`: the temporary name already carries process and nanoseconds, and a
/// collision is refused by name instead of truncating someone else's file.
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
    /// Open, creating it if missing, with the four subdirectories laid out.
    pub fn open_or_create(root: impl Into<PathBuf>) -> Result<Home, Fault> {
        let root = root.into();
        lay(&root)?;
        Ok(Home { root })
    }

    /// Open an existing home. Missing subdirectories are named, never silently added.
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
/// home: `<home's parent>/<40 hex of the author address>/<seat>`. The name comes from the address, so
/// deleting and reimporting an identity finds the same two homes (deleting an identity does not delete ledger
/// directories). This path is built only here.
pub fn identity_home(identity: &str, seat: crate::roles::Role) -> Result<PathBuf, Fault> {
    identity_home_under(&crate::names::key()?, identity, seat)
}

/// The same place under a given names key (a change of master key names the homes it lays down anew).
pub fn identity_home_under(nk: &crate::names::NameKey, identity: &str, seat: crate::roles::Role) -> Result<PathBuf, Fault> {
    let here = where_is()?;
    let base = match here.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => machine_dir()?,
    };
    Ok(base.join(home_dir_name(nk, identity)).join(seat.as_str()))
}

/// An identity's home directory name under a names key. The directory name is keyed (`names`): locked, it
/// says nothing about the identity. One name, one home (tests read it from here too, never working it
/// out themselves).
pub fn home_dir_name(nk: &crate::names::NameKey, identity: &str) -> String {
    nk.name(crate::names::Logical::Home(identity))
}

/// The user's home directory (pointer, default machine directory and the old location are built from it). A
/// stand-in set through `places` is used when present; with the machine directory set and no stand-in (the
/// usual case under the test hooks) it is `None` and the pointer family is neither read nor written.
/// Otherwise the system's home directory (`platform::home_dir`).
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
    /// Set through `places` (only the test hooks set it).
    Placed,
    /// The pointer `~/.zikaron-desk`.
    Pointer,
    /// No pointer, and the older machine directory is on disk: a machine not yet migrated (migration is
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

/// Where the machine directory is and which level it came from. Reads only. Order: set through `places`, then
/// the pointer, then the older machine directory (not migrated), then the default. A pointer present but not
/// one absolute path line is refused with `MACHINE_SHAPE` (never silently the default, which would leave the
/// key vault and registry behind).
pub fn machine_at() -> Result<(PathBuf, Layer), Fault> {
    if let Some(p) = crate::places::get().machine_dir.clone() {
        return Ok((p, Layer::Placed));
    }
    let Some(u) = user_home()? else {
        return Err(Fault::known(Known::NoHomeDir, crate::lang::t(crate::lang::Key::Tail161).to_string()));
    };
    let ptr = pointer_dir(&u).join(POINTER);
    match std::fs::read(&ptr) {
        Ok(bytes) => {
            return read_machine_pointer(&bytes)
                .map(|p| (p, Layer::Pointer))
                .ok_or_else(|| Fault::known(Known::MachineShape, ptr.display().to_string()));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(classify(&e, &ptr.display().to_string())),
    }
    let legacy = LEGACY_DIR.iter().fold(u.clone(), |p, s| p.join(s));
    if legacy.is_dir() {
        return Ok((legacy, Layer::Legacy));
    }
    Ok((crate::platform::app_data_dir(&u).join(APP_DIR), Layer::Default))
}

/// The machine directory (where the key vault, registry, `machine.json` and kit index live). See
/// [`machine_at`].
pub fn machine_dir() -> Result<PathBuf, Fault> {
    machine_at().map(|(p, _)| p)
}

/// Where this machine's home is: named by the environment, then the chosen place recorded in the machine
/// directory ([`PICKED`]), then the old pointer on a machine not yet migrated, then the default home under
/// the machine directory. When none resolves, it says so by name and never guesses a path.
pub fn where_is() -> Result<PathBuf, Fault> {
    if let Some(p) = std::env::var_os(HOME_ENV) {
        let p = PathBuf::from(p);
        if !p.as_os_str().is_empty() {
            return Ok(p);
        }
    }
    let (machine, layer) = machine_at()?;
    // A recorded choice that cannot be read is refused by name (opening the default home silently would make
    // the ledger look lost, and the next home change would write the wrong path back).
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

/// The home the old pointer records on a machine not yet migrated. `None` without an old pointer; present but
/// unreadable is refused by name (falling back to the default would make the ledger look lost and make the
/// migration record "none" permanently).
fn legacy_pick(machine: &Path) -> Result<Option<PathBuf>, Fault> {
    let p = machine.join(LEGACY_POINTER);
    match std::fs::read(&p) {
        Ok(b) => read_pointer(&b).map(Some).ok_or_else(|| Fault::known(Known::MachineShape, p.display().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(classify(&e, &p.display().to_string())),
    }
}

/// One pointer line: the machine directory's absolute path followed by exactly one newline. Nothing else
/// reads.
fn read_machine_pointer(bytes: &[u8]) -> Option<PathBuf> {
    let s = std::str::from_utf8(bytes).ok()?;
    let line = s.strip_suffix('\n')?;
    if line.is_empty() || line.contains('\n') || line.contains('\r') || !Path::new(line).is_absolute() {
        return None;
    }
    Some(PathBuf::from(line))
}

/// The pointer file bytes (one absolute path line with a newline). Written and read from the same definition.
pub fn machine_pointer_bytes(machine: &Path) -> Vec<u8> {
    format!("{}\n", machine.display()).into_bytes()
}

/// Write the pointer: a temporary name then an atomic rename, 0600 (through [`put_at`]). Not rewritten when
/// the bytes are already this place. Returns whether it wrote; `None` (no user-home stand-in under the test
/// hooks) writes nothing.
pub fn write_machine_pointer(machine: &Path) -> Result<bool, Fault> {
    let Some(u) = user_home()? else { return Ok(false) };
    let want = machine_pointer_bytes(machine);
    // Judged by the one reader before it lands: a place whose path does not come back from those bytes (a line
    // break in it, or bytes a text line cannot carry) is refused by name, never written as a pointer no later
    // start could read or one that leads somewhere else.
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

/// Settled once when the window starts (the shipped app calls it only then; the test hooks call it separately
/// in a temporary place):
///
/// 1. With no pointer and the older machine directory on disk: that is the machine directory (key vault and
/// registry stay where they are), and the home recorded in the old `where.json` moves into [`PICKED`] (one
/// migration);
/// 2. the machine directory is created and the pointer written to it (not rewritten when already so);
/// 3. an empty kit index is written when missing.
///
/// Once the pointer exists the older two ([`LEGACY_DIR`] as a level, [`LEGACY_POINTER`]) are never read
/// again: [`machine_at`] sees the pointer first.
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

/// Whether this home was named by the environment. Then it was not chosen in the product: the pointer follows
/// only the person's choices, and a home the environment named once must not replace this machine's default
/// home.
///
/// When both exist on disk, compared by canonical path (`/tmp` and `/private/tmp` are the same place);
/// otherwise as given.
pub fn named_by_env(root: &Path) -> bool {
    let Some(p) = std::env::var_os(HOME_ENV) else { return false };
    let p = PathBuf::from(p);
    if p.as_os_str().is_empty() {
        return false;
    }
    same_place(&p, root)
}

/// A canonical path as people and other programs read it. Canonicalizing on Windows gives the extended form
/// (`\\?\C:\x`, `\\?\UNC\host\share\x`); that prefix is taken off (`C:\x`, `\\host\share\x`). Any other
/// path, including every unix one, comes back as it was. Paths that are stored or shown pass through here;
/// paths only compared with other canonical paths do not need to.
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

// Choosing where things land, decided in one place.
//
// [`choose`] is the only place that decides a landing, and it answers together with why it is not the chosen
// place ([`Why`], three closed forms), so a folder that cannot be used as picked never leads to a silent
// subfolder. The chosen path must also hold when asked again at once (as `open_home_at` does with
// `missing()`): it counts only while the path does not exist.

/// What kind of thing is landing. Closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Moving a home: the person picks a folder. Empty, move into it; not empty, open a new name inside it
    /// (the name from [`HOME_STEM`]).
    Home,
    /// Writing a bundle (kit, badge): a named file is used as is; a named folder gets a name from `stem`
    /// inside it, numbered when taken.
    Bundle { stem: String },
    /// Writing a file: `<stem>.<ext>` inside the chosen folder, numbered when taken.
    File { stem: String, ext: String },
}

/// The name opened for a move, defined once.
pub const HOME_STEM: &str = "ZIKARON";

/// Why it lands here. Three closed forms (the screen speaks by form).
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

/// The one check on a location a person gives: empty, relative, or anything not absolute is refused with
/// `PATH_RELATIVE` before any write.
///
/// A relative path follows the process's current directory: homes, kits, backups, snapshots, badges, key
/// files and grant files written by it would go wherever the program happens to stand, and a recorded pointer
/// would lead elsewhere next time. Every action-layer entry that takes a location, and whether the window's
/// primary keys are enabled, ask this. It touches no disk, so the window can ask every frame.
pub fn landing(given: &str) -> Result<PathBuf, Fault> {
    let t = given.trim();
    let p = Path::new(t);
    if t.is_empty() || !p.is_absolute() {
        return Err(Fault::known(Known::PathRelative, given.to_string()));
    }
    Ok(p.to_path_buf())
}

/// The one place a path stored in settings and read later becomes absolute. The window's picker already gives
/// absolute paths; relative ones (from tests or the command line) are made absolute against the current
/// directory now, without touching the disk or requiring the place to exist. A stored relative path would
/// read somewhere else (or nothing) when started from another directory, while the settings bytes stay the
/// same. An empty string is not a path and is refused with `PATH_RELATIVE`; "empty means clear" is decided by
/// the caller before asking.
pub fn kept(given: &str) -> Result<PathBuf, Fault> {
    let t = given.trim();
    if t.is_empty() {
        return Err(Fault::known(Known::PathRelative, given.to_string()));
    }
    std::path::absolute(t).map_err(|e| classify(&e, t))
}

/// Landing is decided only here.
///
/// All three kinds follow one rule: a named file is used; a named folder gets a name inside it, and "it
/// already has content" and "the name is taken" are each reported by name. The choice is asked again at once:
/// it counts only while that path does not exist (`!exists`).
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
    /// New names from [`numbered`] are free by construction, while the chosen place is expected to exist (an
    /// empty folder or a named file). So this does not assert; it answers, for callers that re-ask (only the
    /// tests; the product only takes the path).
    pub fn free(&self) -> bool {
        !self.at.exists()
    }

    /// Whether this is a new name (only when the chosen place could not be used).
    pub fn is_new_name(&self) -> bool {
        self.why != Why::AsPicked
    }
}

/// Make a landing name inside a folder, numbered when taken.
///
/// `first` is the reason when the first name is free: the two writers give `AsPicked` (the chosen place
/// fits), while moving a home arrives knowing the folder is not empty and gives `FolderNotEmpty`. A taken
/// first name gives `NameTaken`.
fn numbered(dir: &Path, stem: &str, ext: Option<&str>, first: Why) -> Chosen {
    let mut n = 1u32;
    loop {
        let name = match (n, ext) {
            (1, None) => stem.to_string(),
            (1, Some(e)) => format!("{stem}.{e}"),
            (k, None) => format!("{stem}-{k}"),
            (k, Some(e)) => format!("{stem}-{k}.{e}"),
        };
        let p = dir.join(name);
        if !p.exists() {
            return Chosen { at: p, why: if n == 1 { first } else { Why::NameTaken } };
        }
        n += 1;
    }
}

/// Record the chosen home ([`PICKED`] in the machine directory): the only place that records the home's
/// absolute path, and it lives outside the home, so copying the home does not carry it. Replacing it is
/// intended (changing home means changing it); atomic rename, 0600. The machine pointer is written at the
/// same moment (not rewritten when unchanged), so companion apps always find this moment's machine directory.
pub fn write_pointer(home: &Path) -> Result<(), Fault> {
    let machine = machine_dir()?;
    // Every folder this machine points at is remembered, so a later master key change or backup reaches it
    // after the pointer has moved on: the one it leaves (the default folder, or one chosen by an older
    // version, never passed through here) and the one it goes to. Remembered before the pointer moves, so a
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
    // Judged by its reader before it lands, as the machine pointer is: a path that does not come back from
    // these bytes is refused by name.
    if read_pointer(&bytes).as_deref() != Some(home) {
        return Err(Fault::known(Known::MachineShape, home.display().to_string()));
    }
    put_at(&machine, PICKED, &bytes)?;
    if machine_at()?.1 != Layer::Placed {
        write_machine_pointer(&machine)?;
    }
    Ok(())
}

/// The pointer's one member (`{"home": path}`). One name, one home.
pub const POINTER_MEMBER: &str = "home";

/// The `{"home": path}` shape (the same as [`PICKED`] and the old `where.json`). The old one is read only
/// during migration and on machines not yet migrated.
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

    /// The extended prefix canonicalizing gives on Windows comes off, a share's back to its plain form; a
    /// plain path, a unix path and an extended form with no plain spelling come back as they were.
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
