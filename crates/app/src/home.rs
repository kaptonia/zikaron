use crate::fault::{classify, Fault, Known};
use std::path::{Path, PathBuf};

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
            Slot::Ledger => zikaron_seal::envelope::LEDGER_DIR,
            Slot::Kits => zikaron_seal::envelope::KITS_DIR,
            Slot::GrantsHeld => zikaron_seal::envelope::GRANTS_HELD_DIR,
            Slot::Settings => zikaron_seal::envelope::SETTINGS_DIR,
        }
    }
}

pub const APP_DIR: &str = zikaron_os::machine::APP_DIR;
pub const POINTER: &str = zikaron_os::machine::POINTER;
pub const DEFAULT_HOME: &str = "default";
pub const PICKED: &str = "picked-home.json";
pub const HOME_ENV: &str = "ZIKARON_DESK_HOME";


pub const SIDE_FILES: [&str; 3] = [".DS_Store", "Thumbs.db", "desktop.ini"];
pub const SIDE_PREFIX: &str = "._";

pub fn is_side_file(name: &str) -> bool {
    SIDE_FILES.contains(&name) || name.starts_with(SIDE_PREFIX)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Place {
    Absent,
    Empty,
    Home,
    Other(Vec<String>),
}

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

pub fn may_open_at(root: &Path) -> Result<(), Fault> {
    match place_of(root)? {
        Place::Absent | Place::Empty | Place::Home => Ok(()),
        Place::Other(names) => {
            let shown: Vec<&str> = names.iter().take(3).map(String::as_str).collect();
            Err(Fault::known(Known::NotAHome, format!("{}: {}", root.display(), shown.join(" "))))
        }
    }
}

pub fn lay(root: &Path) -> Result<(), Fault> {
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
        let tail = crate::lang::both_of(|| {
            let what = if missing.is_empty() { crate::lang::t(crate::lang::Key::Tail159).to_string() } else { missing.join(" ") };
            if why.is_empty() {
                crate::lang::filln(crate::lang::Key::Tail016, &[&(root.display()).to_string(), &(what).to_string()])
            } else {
                crate::lang::filln(crate::lang::Key::Tail160, &[&(root.display()).to_string(), &(what).to_string(), &(why.join(" · ")).to_string()])
            }
        });
        return Err(Fault::known(Known::CannotLay, tail));
    }
    Ok(())
}

pub fn put(home: &Home, slot: Slot, name: &str, bytes: &[u8]) -> Result<(), Fault> {
    put_at(&home.dir(slot), name, bytes)
}

pub fn put_at(dir: &Path, name: &str, bytes: &[u8]) -> Result<(), Fault> {
    crate::reads::touch();
    if crate::task::ticket_void() {
        return Err(crate::task::void_ticket_fault(dir));
    }
    zikaron_os::put_at(dir, name, &put_tmp_name()?, bytes).map_err(unput)
}

fn unput(u: zikaron_os::Unput) -> Fault {
    classify(&u.error, &u.at.display().to_string())
}

fn put_tmp_name() -> Result<String, Fault> {
    Ok(zikaron_seal::land::tmp_name()?)
}

pub fn is_temp_name(name: &str) -> bool {
    zikaron_store::layout::tmp_shaped(name)
}

pub fn sweep_temps(root: &Path) -> usize {
    fn walk(dir: &Path, gone: &mut usize) {
        let Ok(listing) = std::fs::read_dir(dir) else { return };
        for item in listing.flatten() {
            let Ok(kind) = item.file_type() else { continue };
            if kind.is_dir() {
                walk(&item.path(), gone);
            } else if kind.is_file() && is_temp_name(&item.file_name().to_string_lossy()) && std::fs::remove_file(item.path()).is_ok() {
                *gone += 1;
            }
        }
    }
    let mut gone = 0;
    walk(root, &mut gone);
    gone
}

pub fn rename_over(from: &Path, to: &Path) -> Result<(), Fault> {
    zikaron_os::rename_over(from, to).map_err(unput)
}

pub struct Home {
    root: PathBuf,
}

impl Home {
    pub fn open_or_create(root: impl Into<PathBuf>) -> Result<Home, Fault> {
        let root = root.into();
        lay(&root)?;
        Ok(Home { root })
    }

    pub fn open(root: impl Into<PathBuf>) -> Result<Home, Fault> {
        crate::reads::touch();
        let root = root.into();
        if !root.is_dir() {
            return Err(Fault::known(Known::FileMissing, root.display().to_string()));
        }
        Ok(Home { root })
    }

    pub fn bare(root: PathBuf) -> Home {
        Home { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn dir(&self, s: Slot) -> PathBuf {
        self.root.join(s.as_str())
    }

    pub fn missing(&self) -> Vec<&'static str> {
        Slot::ALL
            .iter()
            .filter(|s| !self.dir(**s).is_dir())
            .map(|s| s.as_str())
            .collect()
    }

    pub fn ledger(&self) -> Result<crate::local::Ledger, Fault> {
        crate::reads::touch();
        crate::local::ledger_of(self)
    }

    pub fn usage(&self) -> Result<u64, Fault> {
        walk_size(&self.root)
    }
}

fn walk_size(p: &Path) -> Result<u64, Fault> {
    size_of(p, false)
}

fn size_of(p: &Path, listed: bool) -> Result<u64, Fault> {
    let gone = |e: &std::io::Error| listed && (e.kind() == std::io::ErrorKind::NotFound || zikaron_os::going(p));
    let md = match std::fs::symlink_metadata(p) {
        Ok(m) => m,
        Err(e) if gone(&e) => return Ok(0),
        Err(e) => return Err(classify(&e, &p.display().to_string())),
    };
    if md.is_file() {
        return Ok(md.len());
    }
    if !md.is_dir() {
        return Ok(0);
    }
    let listing = match std::fs::read_dir(p) {
        Ok(l) => l,
        Err(e) if gone(&e) => return Ok(0),
        Err(e) => return Err(classify(&e, &p.display().to_string())),
    };
    let mut n = 0;
    for e in listing {
        let e = match e {
            Ok(e) => e,
            Err(e) if gone(&e) => return Ok(n),
            Err(e) => return Err(classify(&e, &p.display().to_string())),
        };
        n += size_of(&e.path(), true)?;
    }
    Ok(n)
}

pub fn identity_home(identity: &str, seat: crate::roles::Role) -> Result<PathBuf, Fault> {
    identity_home_under(&crate::names::key()?, identity, seat)
}

pub fn identity_home_under(nk: &crate::names::NameKey, identity: &str, seat: crate::roles::Role) -> Result<PathBuf, Fault> {
    let here = where_is()?;
    let base = match here.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => machine_dir()?,
    };
    Ok(base.join(home_dir_name(nk, identity)).join(seat.as_str()))
}

pub fn home_dir_name(nk: &crate::names::NameKey, identity: &str) -> String {
    nk.name(crate::names::Logical::Home(identity))
}

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
        .ok_or_else(|| Fault::known(Known::NoHomeDir, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail161).to_string())))
}

pub fn pointer_path() -> Result<Option<PathBuf>, Fault> {
    Ok(user_home()?.map(|u| pointer_dir(&u).join(POINTER)))
}

pub fn pointer_dir(user_home: &Path) -> PathBuf {
    crate::platform::app_data_dir(user_home)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Placed,
    Pointer,
    Default,
}

impl Layer {
    pub fn as_str(self) -> &'static str {
        match self {
            Layer::Placed => "placed",
            Layer::Pointer => "pointer",
            Layer::Default => "default",
        }
    }
}

pub fn machine_at() -> Result<(PathBuf, Layer), Fault> {
    if let Some(p) = crate::places::get().machine_dir.clone() {
        return Ok((p, Layer::Placed));
    }
    if let Some(kept) = SETTLED.lock().unwrap_or_else(|e| e.into_inner()).clone() {
        return Ok(kept);
    }
    machine_fresh()
}

static SETTLED: std::sync::Mutex<Option<(PathBuf, Layer)>> = std::sync::Mutex::new(None);

pub fn forget_machine() {
    *SETTLED.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

fn keep_machine(machine: &Path) {
    *SETTLED.lock().unwrap_or_else(|e| e.into_inner()) = Some((machine.to_path_buf(), Layer::Pointer));
}

fn machine_fresh() -> Result<(PathBuf, Layer), Fault> {
    crate::reads::touch();
    if let Some(p) = crate::places::get().machine_dir.clone() {
        return Ok((p, Layer::Placed));
    }
    let Some(u) = user_home()? else {
        return Err(Fault::known(Known::NoHomeDir, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail161).to_string())));
    };
    use zikaron_os::machine::{self, Unread};
    match machine::of(&u) {
        Ok((p, machine::Layer::Pointer)) => Ok((p, Layer::Pointer)),
        Ok((p, machine::Layer::Default)) => Ok((p, Layer::Default)),
        Err(Unread::PointerShape(ptr)) => Err(Fault::known(Known::MachineShape, ptr.display().to_string())),
        Err(Unread::Pointer(ptr, e)) => Err(classify(&e, &ptr.display().to_string())),
        Err(Unread::NoHome) => Err(Fault::known(Known::NoHomeDir, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail161).to_string()))),
    }
}

pub fn machine_dir() -> Result<PathBuf, Fault> {
    machine_at().map(|(p, _)| p)
}

pub fn here(recorded: &Path) -> PathBuf {
    match machine_dir() {
        Ok(m) => here_under(&m, recorded),
        Err(_) => recorded.to_path_buf(),
    }
}

pub fn here_under(m: &Path, recorded: &Path) -> PathBuf {
    if recorded.starts_with(m) {
        return recorded.to_path_buf();
    }
    let parts: Vec<std::path::Component> = recorded.components().collect();
    for tail_len in [2usize, 1] {
        if parts.len() <= tail_len {
            continue;
        }
        let (head, tail) = parts.split_at(parts.len() - tail_len);
        let (was, rel): (PathBuf, PathBuf) = (head.iter().collect(), tail.iter().collect());
        let was_machine = !was.exists() || was.join(crate::places::keybox_file()).is_file();
        if was_machine && !same_place(&was, m) && m.join(&rel).is_dir() {
            note_moved(&was, m);
            return m.join(rel);
        }
    }
    recorded.to_path_buf()
}

static MOVED: std::sync::Mutex<Option<(PathBuf, PathBuf)>> = std::sync::Mutex::new(None);

fn note_moved(was: &Path, now: &Path) {
    *MOVED.lock().unwrap_or_else(|e| e.into_inner()) = Some((was.to_path_buf(), now.to_path_buf()));
}

pub const MOVED_SAID: &str = "moved-said";

pub fn moved_unsaid() -> Option<PathBuf> {
    let (was, now) = MOVED.lock().unwrap_or_else(|e| e.into_inner()).take()?;
    say_once(&was, &now).then_some(was)
}

fn say_once(was: &Path, now: &Path) -> bool {
    let mark = zikaron::hexfmt::encode(&zikaron::cryptox::sha256(was.display().to_string().as_bytes()));
    let said = std::fs::read_to_string(now.join(MOVED_SAID)).unwrap_or_default();
    if said.lines().any(|l| l.trim() == mark) {
        return false;
    }
    let _ = put_at(now, MOVED_SAID, format!("{said}{mark}\n").as_bytes());
    true
}

pub fn where_is() -> Result<PathBuf, Fault> {
    if let Some(p) = std::env::var_os(HOME_ENV) {
        let p = PathBuf::from(p);
        if !p.as_os_str().is_empty() {
            return Ok(p);
        }
    }
    let machine = machine_dir()?;
    let picked = machine.join(PICKED);
    match std::fs::read(&picked) {
        Ok(b) => return read_pointer(&b).map(|h| here_under(&machine, &h)).ok_or_else(|| Fault::known(Known::MachineShape, picked.display().to_string())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(classify(&e, &picked.display().to_string())),
    }
    Ok(machine.join(DEFAULT_HOME))
}

fn read_machine_pointer(bytes: &[u8], folder: &Path) -> Option<PathBuf> {
    zikaron_os::machine::read_pointer(bytes, folder)
}

pub fn machine_pointer_bytes(machine: &Path) -> Vec<u8> {
    match user_home() {
        Ok(Some(u)) => zikaron_os::machine::pointer_bytes(machine, &pointer_dir(&u)),
        _ => format!("{}\n", machine.display()).into_bytes(),
    }
}

pub fn write_machine_pointer(machine: &Path) -> Result<bool, Fault> {
    let Some(u) = user_home()? else { return Ok(false) };
    let dir = pointer_dir(&u);
    let want = zikaron_os::machine::pointer_bytes(machine, &dir);
    if read_machine_pointer(&want, &dir).as_deref() != Some(machine) {
        return Err(Fault::known(Known::MachineShape, machine.display().to_string()));
    }
    if std::fs::read(dir.join(POINTER)).ok().as_deref() == Some(want.as_slice()) {
        keep_machine(machine);
        return Ok(false);
    }
    put_at(&dir, POINTER, &want)?;
    keep_machine(machine);
    Ok(true)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settled {
    pub machine: PathBuf,
    pub layer: Layer,
    pub pointer_written: bool,
    pub index_written: bool,
}

pub fn settle_machine() -> Result<Settled, Fault> {
    let (machine, layer) = machine_fresh()?;
    let before = pointer_path()?.and_then(|p| std::fs::read(p).ok());
    std::fs::create_dir_all(&machine).map_err(|e| classify(&e, &machine.display().to_string()))?;
    if layer != Layer::Placed {
        write_machine_pointer(&machine)?;
    }
    let pointer_written = pointer_path()?.and_then(|p| std::fs::read(p).ok()) != before;
    let index_written = crate::kitsindex::ensure(&machine)?;
    Ok(Settled { machine, layer, pointer_written, index_written })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsidePlan {
    pub machine: PathBuf,
    pub kept: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Aside {
    pub from: PathBuf,
    pub to: PathBuf,
}

pub fn aside_plan() -> Result<AsidePlan, Fault> {
    let machine = machine_dir()?;
    let inside = |p: &Path| {
        let (a, m) = (std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()), std::fs::canonicalize(&machine).unwrap_or_else(|_| machine.clone()));
        a.starts_with(&m)
    };
    let kept = where_is().ok().filter(|h| !named_by_env(h) && !inside(h));
    Ok(AsidePlan { machine, kept })
}

pub fn aside_name(machine: &Path, day: &str) -> PathBuf {
    let name = machine.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let dir = machine.parent().map(Path::to_path_buf).unwrap_or_default();
    let mut n = 1u32;
    loop {
        let p = dir.join(numbered_name(&format!("{name}.aside-{day}"), None, n));
        if std::fs::symlink_metadata(&p).is_err() {
            return p;
        }
        n += 1;
    }
}

pub fn set_machine_aside(day: &str) -> Result<Aside, Fault> {
    let machine = machine_dir()?;
    let to = aside_name(&machine, day);
    let mut locks = crate::lock::files_under(&machine);
    locks.push(machine.join(crate::places::keybox_lock_file()));
    zikaron_os::set_aside(&machine, &to, &locks).map_err(|n| match n {
        zikaron_os::NotAside::Held(p) => Fault::known(Known::MachineHeld, p.display().to_string()),
        zikaron_os::NotAside::Io(p, e) => classify(&e, &p.display().to_string()),
    })?;
    Ok(Aside { from: machine, to })
}

pub fn named_by_env(root: &Path) -> bool {
    let Some(p) = std::env::var_os(HOME_ENV) else { return false };
    let p = PathBuf::from(p);
    if p.as_os_str().is_empty() {
        return false;
    }
    same_place(&p, root)
}

pub fn plain_path(p: PathBuf) -> PathBuf {
    let Some(t) = p.to_str() else { return p };
    if let Some(rest) = t.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    match t.strip_prefix(r"\\?\") {
        Some(rest) if rest.as_bytes().get(1) == Some(&b':') && rest.as_bytes().first().is_some_and(u8::is_ascii_alphabetic) => PathBuf::from(rest),
        _ => p,
    }
}

pub fn same_place(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Home,
    Bundle { stem: String },
    File { stem: String, ext: String },
}

pub const HOME_STEM: &str = "ZIKARON";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Why {
    AsPicked,
    FolderNotEmpty,
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

    pub fn say(self) -> Option<crate::lang::Key> {
        match self {
            Why::AsPicked => None,
            Why::FolderNotEmpty => Some(crate::lang::Key::LandingFolderNotEmpty),
            Why::NameTaken => Some(crate::lang::Key::LandingNameTaken),
        }
    }

    pub fn say_ahead(self) -> Option<crate::lang::Key> {
        match self {
            Why::AsPicked => None,
            Why::FolderNotEmpty => Some(crate::lang::Key::LandingAheadFolderNotEmpty),
            Why::NameTaken => Some(crate::lang::Key::LandingAheadNameTaken),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chosen {
    pub at: PathBuf,
    pub why: Why,
}

pub fn landing(given: &str) -> Result<PathBuf, Fault> {
    let t = given.trim();
    let p = Path::new(t);
    if t.is_empty() || !p.is_absolute() {
        return Err(Fault::known(Known::PathRelative, given.to_string()));
    }
    Ok(p.to_path_buf())
}

pub fn kept(given: &str) -> Result<PathBuf, Fault> {
    let t = given.trim();
    if t.is_empty() {
        return Err(Fault::known(Known::PathRelative, given.to_string()));
    }
    std::path::absolute(t).map_err(|e| classify(&e, t))
}

pub fn choose(kind: &Kind, picked: &Path) -> Chosen {
    let chosen = match kind {
        Kind::Home => {
            let empty = std::fs::read_dir(picked).map(|mut d| d.next().is_none()).unwrap_or(false);
            if empty {
                Chosen { at: picked.to_path_buf(), why: Why::AsPicked }
            } else {
                numbered(picked, HOME_STEM, None, Why::FolderNotEmpty)
            }
        }
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
    pub fn free(&self) -> bool {
        !self.at.exists()
    }

    pub fn is_new_name(&self) -> bool {
        self.why != Why::AsPicked
    }
}

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

fn numbered_name(stem: &str, ext: Option<&str>, n: u32) -> String {
    match (n, ext) {
        (1, None) => stem.to_string(),
        (1, Some(e)) => format!("{stem}.{e}"),
        (k, None) => format!("{stem}-{k}"),
        (k, Some(e)) => format!("{stem}-{k}.{e}"),
    }
}

pub fn land_numbered(dir: &Path, stem: &str, ext: &str, bytes: &[u8]) -> Result<PathBuf, Fault> {
    if crate::task::ticket_void() {
        return Err(crate::task::void_ticket_fault(dir));
    }
    std::fs::create_dir_all(dir).map_err(|e| classify(&e, &dir.display().to_string()))?;
    let mut n = 1u32;
    loop {
        let p = dir.join(numbered_name(stem, Some(ext), n));
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

pub fn write_pointer(home: &Path) -> Result<(), Fault> {
    let machine = machine_dir()?;
    if let Ok(was) = where_is() {
        if was.is_dir() {
            crate::machine::remember_home(&was)?;
        }
    }
    crate::machine::remember_home(home)?;
    let doc = zikaron::json::Value::Obj(vec![(
        POINTER_MEMBER.to_string(),
        zikaron::json::Value::Str(home.display().to_string()),
    )]);
    let bytes = zikaron::json::canon_bytes(&doc);
    if read_pointer(&bytes).as_deref() != Some(home) {
        return Err(Fault::known(Known::MachineShape, home.display().to_string()));
    }
    put_at(&machine, PICKED, &bytes)?;
    if machine_at()?.1 != Layer::Placed {
        write_machine_pointer(&machine)?;
    }
    Ok(())
}

pub const POINTER_MEMBER: &str = "home";

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
    fn a_move_is_said_once_per_place_it_came_from() {
        let d = std::env::temp_dir().join(format!("zikaron-moved-said-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("dir");
        let (was, other) = (std::path::Path::new("/old/place/.zikaron-desk.d"), std::path::Path::new("/another/.zikaron-desk.d"));
        assert!(super::say_once(was, &d), "said the first time");
        assert!(!super::say_once(was, &d), "not again");
        assert!(super::say_once(other, &d), "another place: said");
        let record = std::fs::read_to_string(d.join(super::MOVED_SAID)).expect("recorded");
        assert!(!record.contains("old"), "the place itself is not written");
        std::fs::remove_file(d.join(super::MOVED_SAID)).expect("removed");
        assert!(super::say_once(was, &d), "the record gone: said once more");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_recorded_home_follows_the_machine_directory_it_lay_in() {
        let at = |base: &std::path::Path, parts: &[&str]| parts.iter().fold(base.to_path_buf(), |p, x| p.join(x));
        let t = std::env::temp_dir().join(format!("zikaron-here-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let (was, now, chosen, gone) = (at(&t, &["was"]), at(&t, &["now"]), at(&t, &["documents"]), at(&t, &["gone"]));
        let seat = ["k1", "author"];
        for d in [at(&was, &seat), at(&now, &seat), at(&now, &[super::DEFAULT_HOME]), at(&chosen, &seat)] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(at(&was, &[&crate::places::keybox_file()]), b"vault").unwrap();
        let here = |p: std::path::PathBuf| super::here_under(&now, &p);
        assert_eq!(here(at(&was, &seat)), at(&now, &seat), "copied from a machine directory");
        assert_eq!(here(at(&was, &[super::DEFAULT_HOME])), at(&now, &[super::DEFAULT_HOME]), "the default home with it");
        assert_eq!(here(at(&gone, &seat)), at(&now, &seat), "moved from one no longer there");
        assert_eq!(here(at(&chosen, &seat)), at(&chosen, &seat), "a home chosen elsewhere stays where it was chosen");
        assert_eq!(here(at(&now, &seat)), at(&now, &seat), "already inside the opened one");
        let other = ["k2", "author"];
        assert_eq!(here(at(&was, &other)), at(&was, &other), "a home the opened directory does not hold is read as recorded");
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn every_temporary_name_this_product_writes_is_read_as_one() {
        let put = super::put_tmp_name().expect("a temporary name");
        let landing = zikaron_glue::landing::staging_beside(&std::env::temp_dir().join(std::path::Path::new("kit"))).expect("a temporary name").file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let store = zikaron_store::layout::tmp_file_name("0123456789abcdef");
        for n in [&put, &landing, &store] {
            assert!(super::is_temp_name(n), "{n} is ours");
        }
        for n in ["settings.json", ".DS_Store", ".settings.json.12.34.tmp", "a.tmp", ".kit.staging-1-0123456789abcdef", ".zks-tmp-xyz"] {
            assert!(!super::is_temp_name(n), "{n} is not ours");
        }
    }

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
