//! Machine-level settings file.
//!
//! Settings exist per home (`settings.rs`), but some things belong to this machine and do not change with
//! identity or data directory: idle time before auto-lock, and which network was chosen. They live in the
//! machine directory (alongside `where.json` and the key vault); the file name and reading and writing live
//! only in this file (one name, one home); writing goes through `home::put_at`, so 0600 and atomic renaming
//! are answered in the same place as elsewhere.
//!
//! ─── Shape ───
//!
//! `{"appearance":"light","auto_lock":false,"auto_lock_secs":900,"backup":{"at":1790000000,"count":12,"path":"/…"},"lang":"en","network":"sepolia","shape":"zikaron-desk/machine/1"}`,
//! canonical key order. `auto_lock` is whether idle locking is on (absent means on, the factory setting);
//! `auto_lock_secs` how long idle before it locks. `backup` records the last whole-machine backup: when, where,
//! and how many ledger entries and held grants this machine had then (the settings page and the watch table
//! count what came after); absent means never backed up. `homes` lists the data folders this machine opened as its own. `lang` is the language last chosen on this machine
//! (`zh` or `en`), which the passcode gate speaks before unlocking. The file stays plain: the passcode gate
//! reads the appearance and the language before unlocking, and none of its cells is something the person wrote. `appearance` is how the window looks on this machine (`light`, `dark`, or `system`
//! to follow the operating system); absent means light.
//! `network` is the name the wizard's network step last chose (a row of `deploy::KNOWN`, or `deploy::CUSTOM`
//! "custom"); absent means not chosen yet (older files with only the auto-lock cell still open). It decides only
//! which choice the wizard's network step and the new-identity sheet select first: what network a home has is
//! decided by the identity that made it, or by the person in settings (`identity::Row::network`). Unknown
//! members are kept as they are, so changing one cell does not erase others (an older file's `custom`, the
//! network once filled in by hand for the whole machine, stays as it was and is read by nothing). No file means all defaults (a machine whose settings were
//! never changed). A wrong shape, or an auto-lock value outside the closed table, is refused by name as
//! `MACHINE_SHAPE`, and not one byte of the file changes; the shell locks by the default and hands the
//! refusal to the face (never silently "never lock").

use crate::fault::{Fault, Known};
use zikaron::json::{self, Value};

/// The file's shape.
pub const SHAPE: &str = "zikaron-desk/machine/1";
/// File name. One name, one home.
pub const FILE: &str = "machine.json";

/// Closed table of auto-lock times (seconds): 1, 5, 15, 30, 60 minutes. "Never" is the switch being off, not a
/// member of this table.
pub const LOCK_CHOICES: [u64; 5] = [60, 300, 900, 1800, 3600];
/// Default fifteen minutes.
pub const LOCK_DEFAULT: u64 = 900;
/// Factory setting of the auto-lock switch: on.
pub const LOCK_ON_DEFAULT: bool = true;

/// The last whole-machine backup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backed {
    /// When it was written (seconds).
    pub at: u64,
    /// Where it was written.
    pub path: String,
    /// How many ledger entries and held grants this machine had then.
    pub count: u64,
}

/// How many ledger entries and held grants came after the last whole-machine backup: the one count the setup
/// check, the wizard's backup point, the watch table and the settings page all read. `None` when never backed
/// up or when nothing has been measured yet (undecided, never "none behind").
pub fn backup_behind(last: Option<&Backed>, now: Option<u64>) -> Option<u64> {
    Some(now?.saturating_sub(last?.count))
}

/// The closed table of appearances.
pub const APPEARANCES: [&str; 3] = [appearance::LIGHT, appearance::DARK, appearance::SYSTEM];

/// The appearance words (the closed table above is made of them). One name, one home.
pub mod appearance {
    pub const LIGHT: &str = "light";
    pub const DARK: &str = "dark";
    pub const SYSTEM: &str = "system";
}

/// The machine file's member names (top level and the backup record). One name, one home: the reader, the
/// writer and anything reading the file's raw members spell them only here.
pub mod member {
    pub const SHAPE: &str = "shape";
    pub const AUTO_LOCK: &str = "auto_lock";
    pub const AUTO_LOCK_SECS: &str = "auto_lock_secs";
    pub const BACKUP: &str = "backup";
    pub const NETWORK: &str = "network";
    pub const APPEARANCE: &str = "appearance";
    pub const LANG: &str = "lang";
    pub const AT: &str = "at";
    pub const COUNT: &str = "count";
    pub const PATH: &str = "path";
}

/// This machine's settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Machine {
    /// Whether idle locking is on.
    pub auto_lock: bool,
    /// Idle seconds before auto-lock; always in [`LOCK_CHOICES`].
    pub auto_lock_secs: u64,
    /// The last whole-machine backup; `None` means never.
    pub backup: Option<Backed>,
    /// The network the wizard last chose (row name, or `deploy::CUSTOM`); `None` means not chosen yet. Only
    /// which choice is selected first: no home takes its network from here.
    pub network: Option<String>,
    /// The chosen appearance, one of [`APPEARANCES`]; `None` means never chosen (light).
    pub appearance: Option<String>,
    /// The language last chosen on this machine, so the passcode gate speaks it before unlocking (the home's
    /// own choice is sealed and read after); `None` means never chosen.
    pub lang: Option<crate::lang::Lang>,
    /// Every data folder this machine has opened as its own (not an identity's seat folder), in the order
    /// first opened; a master key change reseals them all and the whole-machine backup carries them all, so
    /// one opened before and left is never left under a key that is gone.
    pub homes: Vec<String>,
    /// When each old data place among `homes` was set aside (its path, Unix seconds): said as its date.
    pub aside_at: Vec<(String, u64)>,
    /// Unknown members, kept as they are (reading and writing never touch them).
    extra: Vec<(String, Value)>,
}

impl Default for Machine {
    fn default() -> Self {
        Machine { auto_lock: LOCK_ON_DEFAULT, auto_lock_secs: LOCK_DEFAULT, backup: None, network: None, appearance: None, lang: None, homes: Vec::new(), aside_at: Vec::new(), extra: Vec::new() }
    }
}

/// Where the file is.
pub fn path() -> Result<std::path::PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(FILE))
}

/// Read. No file means defaults; a wrong shape is refused by name.
pub fn read() -> Result<Machine, Fault> {
    let p = path()?;
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Machine::default()),
        Err(e) => return Err(crate::fault::classify(&e, &p.display().to_string())),
    };
    parse(&bytes, &p)
}

/// Write settings carried in whole (a restore's staged machine settings): read with the same checks as the
/// file, then written the one way.
pub fn write_bytes(bytes: &[u8]) -> Result<(), Fault> {
    let m = parse(bytes, &path()?)?;
    write(&m)
}

fn parse(bytes: &[u8], p: &std::path::Path) -> Result<Machine, Fault> {
    let v = json::parse(bytes).map_err(|t| Fault::known(Known::MachineShape, format!("{}: {t:?}", p.display())))?;
    let Value::Obj(members) = v else {
        return Err(Fault::known(Known::MachineShape, p.display().to_string()));
    };
    let mut out = Machine::default();
    let mut shape_ok = false;
    for (k, val) in members {
        match (k.as_str(), &val) {
            (member::SHAPE, Value::Str(s)) if s == SHAPE => shape_ok = true,
            (member::SHAPE, _) => return Err(Fault::known(Known::MachineShape, member::SHAPE.to_string())),
            (member::AUTO_LOCK, Value::Bool(b)) => out.auto_lock = *b,
            (member::AUTO_LOCK, _) => return Err(Fault::known(Known::MachineShape, member::AUTO_LOCK.to_string())),
            (member::BACKUP, b) => {
                let int = |k: &str| match b.member(k) {
                    Some(Value::Int(n)) => Some(*n),
                    _ => None,
                };
                let path = match b.member(member::PATH) {
                    Some(Value::Str(x)) => Some(x.clone()),
                    _ => None,
                };
                match (int(member::AT), int(member::COUNT), path) {
                    (Some(at), Some(count), Some(path)) => out.backup = Some(Backed { at, path, count }),
                    _ => return Err(Fault::known(Known::MachineShape, member::BACKUP.to_string())),
                }
            }
            (member::AUTO_LOCK_SECS, Value::Int(n)) if LOCK_CHOICES.contains(n) => out.auto_lock_secs = *n,
            (member::AUTO_LOCK_SECS, _) => return Err(Fault::known(Known::MachineShape, member::AUTO_LOCK_SECS.to_string())),
            (member::NETWORK, Value::Str(n)) if network_ok(n) => out.network = Some(n.clone()),
            (member::NETWORK, _) => return Err(Fault::known(Known::MachineShape, member::NETWORK.to_string())),
            (member::APPEARANCE, Value::Str(a)) if APPEARANCES.contains(&a.as_str()) => out.appearance = Some(a.clone()),
            (member::APPEARANCE, _) => return Err(Fault::known(Known::MachineShape, member::APPEARANCE.to_string())),
            (member::LANG, Value::Str(l)) if crate::lang::Lang::ALL.iter().any(|x| x.as_str() == l) => {
                out.lang = crate::lang::Lang::ALL.into_iter().find(|x| x.as_str() == l);
            }
            (member::LANG, _) => return Err(Fault::known(Known::MachineShape, member::LANG.to_string())),
            ("homes", Value::Arr(a)) => {
                for x in a {
                    match x {
                        Value::Str(h) if !h.is_empty() => out.homes.push(h.clone()),
                        _ => return Err(Fault::known(Known::MachineShape, "homes".to_string())),
                    }
                }
            }
            ("homes", _) => return Err(Fault::known(Known::MachineShape, "homes".to_string())),
            ("aside", Value::Arr(a)) => {
                for x in a {
                    match (x.member(member::PATH), x.member(member::AT)) {
                        (Some(Value::Str(p)), Some(Value::Int(at))) if !p.is_empty() => out.aside_at.push((p.clone(), *at)),
                        _ => return Err(Fault::known(Known::MachineShape, "aside".to_string())),
                    }
                }
            }
            ("aside", _) => return Err(Fault::known(Known::MachineShape, "aside".to_string())),
            _ => out.extra.push((k, val)),
        }
    }
    if !shape_ok {
        return Err(Fault::known(Known::MachineShape, member::SHAPE.to_string()));
    }
    Ok(out)
}

/// Write. An auto-lock value outside the closed table is refused by name, and the file is untouched.
pub fn write(m: &Machine) -> Result<(), Fault> {
    let bytes = to_bytes(m)?;
    crate::home::put_at(&crate::home::machine_dir()?, FILE, &bytes)
}

/// Read settings from bytes with the file's checks (a restore checks the backup's before staging anything).
pub fn from_bytes(bytes: &[u8]) -> Result<Machine, Fault> {
    parse(bytes, &path()?)
}

/// Record a data folder this machine opened as its own (the pointer's write asks it), once.
pub fn remember_home(at: &std::path::Path) -> Result<(), Fault> {
    let mut m = read()?;
    if m.homes.iter().any(|h| crate::home::same_place(std::path::Path::new(h), at)) {
        return Ok(());
    }
    m.homes.push(at.display().to_string());
    write(&m)
}

/// The file's bytes for these settings, after the same checks the write makes.
pub fn to_bytes(m: &Machine) -> Result<Vec<u8>, Fault> {
    if !LOCK_CHOICES.contains(&m.auto_lock_secs) {
        return Err(Fault::known(Known::MachineShape, m.auto_lock_secs.to_string()));
    }
    if let Some(n) = &m.network {
        if !network_ok(n) {
            return Err(Fault::known(Known::MachineShape, n.clone()));
        }
    }
    if let Some(a) = &m.appearance {
        if !APPEARANCES.contains(&a.as_str()) {
            return Err(Fault::known(Known::MachineShape, a.clone()));
        }
    }
    let mut members = m.extra.clone();
    if let Some(a) = &m.appearance {
        members.push((member::APPEARANCE.into(), Value::Str(a.clone())));
    }
    if !m.auto_lock {
        members.push((member::AUTO_LOCK.into(), Value::Bool(false)));
    }
    members.push((member::AUTO_LOCK_SECS.into(), Value::Int(m.auto_lock_secs)));
    if let Some(l) = m.lang {
        members.push((member::LANG.into(), Value::Str(l.as_str().to_string())));
    }
    if let Some(b) = &m.backup {
        members.push((
            member::BACKUP.into(),
            Value::Obj(vec![
                (member::AT.into(), Value::Int(b.at)),
                (member::COUNT.into(), Value::Int(b.count)),
                (member::PATH.into(), Value::Str(b.path.clone())),
            ]),
        ));
    }
    if let Some(n) = &m.network {
        members.push((member::NETWORK.into(), Value::Str(n.clone())));
    }
    if !m.homes.is_empty() {
        members.push(("homes".into(), Value::Arr(m.homes.iter().map(|h| Value::Str(h.clone())).collect())));
    }
    if !m.aside_at.is_empty() {
        members.push((
            "aside".into(),
            Value::Arr(m.aside_at.iter().map(|(p, at)| Value::Obj(vec![(member::AT.into(), Value::Int(*at)), (member::PATH.into(), Value::Str(p.clone()))])).collect()),
        ));
    }
    members.push((member::SHAPE.into(), Value::Str(SHAPE.to_string())));
    members.sort_by(|a, b| a.0.cmp(&b.0));
    let bytes = json::canon_bytes(&Value::Obj(members));
    // Judged by the reader that reads it back (a number past the canonical integer ceiling, for one, would
    // write and then never read): refused by name, and the file is untouched.
    if let Err(t) = json::parse(&bytes) {
        return Err(Fault::known(Known::MachineShape, format!("{t:?}")));
    }
    Ok(bytes)
}

/// Names the network cell accepts: a table row, or "custom".
fn network_ok(n: &str) -> bool {
    crate::deploy::is_choice(n)
}

/// The chosen row (only when a table row was chosen; not chosen or "custom" gives `None`).
pub fn chosen(m: &Machine) -> Option<&'static crate::deploy::Deployment> {
    m.network.as_deref().and_then(crate::deploy::named)
}

/// The choice selected first in the wizard's network step and the new-identity sheet: what this machine last
/// chose, or the table's default row when it never chose.
pub fn pick(m: &Machine) -> String {
    m.network.clone().unwrap_or_else(|| crate::deploy::DEFAULT.to_string())
}

/// Whether idle time is up. A pure function: the last human input at `last`, now at `now` (same clock,
/// seconds); idle for `secs` means due. The window and tests both ask it; "now" is passed in by the caller
/// (injected by tests, the UI clock in the window).
pub fn idle_due(last: f64, now: f64, secs: u64) -> bool {
    now - last >= secs as f64
}

/// Whether this machine locks itself now: the switch is on and idle time is up. The core's one decision; the
/// window only hands it the last input moment (`Shell::idle_tick`).
pub fn idle_locks(m: &Machine, last: f64, now: f64) -> bool {
    m.auto_lock && idle_due(last, now, m.auto_lock_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_is_due_at_the_mark_and_not_before() {
        assert!(!idle_due(100.0, 159.9, 60));
        assert!(idle_due(100.0, 160.0, 60));
    }
}
