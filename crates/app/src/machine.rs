//! Machine-level settings.
//!
//! Most settings are per home (`settings.rs`), but some belong to the machine regardless of identity or data
//! directory: auto-lock, appearance, language, network choice, proxy, the command line's switches, backup
//! records. They live in the machine directory (next to `where.json` and the key vault) in two files:
//!
//! - `machine.json`, plain, holding only what the passcode screen reads before unlocking: `appearance` and
//!   `lang`. Nothing else is read from it; any other member (an earlier version's settings) is never read and
//!   goes at the next write.
//! - the sealed machine settings (`places::machine_file`, `local::Doc::MachineSealed`), holding the rest; read
//!   only once unlocked. While locked, a change to them is refused (`LOCKED`) and never written over them.
//!
//! ─── Shape ───
//!
//! The settings as one whole (a backup carries them so, and the sealed file is the whole without `appearance`
//! and `lang`):
//! `{"appearance":"light","auto_lock":false,"auto_lock_secs":900,"backup":{"at":1790000000,"count":12,"index":true,"path":"/…"},"lang":"en","network":"sepolia","shape":"zikaron-desk/machine/1"}`,
//! canonical key order.
//!
//! - `appearance`: `light`, `dark`, or `system` (follow the OS); absent means light.
//! - `auto_lock`: whether idle locking is on (absent means on); `auto_lock_secs`: idle time before locking.
//! - `backup`: the last whole-machine backup (when, where, and how many ledger entries and held grants existed
//!   then, so the settings page and watch table can count what came after); absent means never backed up.
//! - `homes`: data folders this machine has used as its own.
//! - `lang`: the last chosen language (`zh` or `en`), used by the passcode screen before unlocking.
//! - `cli_anchor`: how the CLI's `anchor` command is handled by the desktop app: `queue` shows the request and
//!   sends nothing; absent (or `send`, never written) sends as the send button does.
//! - `cli_desktop`: whether the command line may write through the desktop (its door); absent means on.
//! - `cli_alone`: whether the command line writes alone, with a credential lasting this many seconds (or
//!   `never` expiring); absent means off. When on, the desktop's door stays closed ([`Machine::cli_path`]).
//! - `network`: the network last chosen in the wizard (a `deploy::KNOWN` row or `deploy::CUSTOM`); absent means
//!   not chosen. It only decides which option is preselected; a home's network comes from its identity or
//!   settings (`identity::Row::network`).
//!
//! Unknown members of the sealed part are preserved, so changing one field never erases others. No file means
//! all defaults. A wrong shape, or a known member with an unknown value, fails the whole file with
//! `MACHINE_SHAPE` and leaves it untouched; the shell then locks with the default timeout and reports the error
//! (never silently "never lock").

use crate::fault::{Fault, Known};
use zikaron::json::{self, Value};

/// The file's shape.
pub const SHAPE: &str = "zikaron-desk/machine/1";
/// File name.
pub const FILE: &str = "machine.json";

/// Allowed auto-lock times in seconds: 1, 5, 15, 30, 60 minutes. "Never" is the switch being off, not an entry
/// here.
pub const LOCK_CHOICES: [u64; 5] = [60, 300, 900, 1800, 3600];
/// Default: fifteen minutes.
pub const LOCK_DEFAULT: u64 = 900;
/// Default for the auto-lock switch: on.
pub const LOCK_ON_DEFAULT: bool = true;

/// The last whole-machine backup.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backed {
    /// When it was written (seconds).
    pub at: u64,
    /// Where it was written.
    pub path: String,
    /// How many ledger entries and held grants this machine had then. Every backup records which entries and
    /// held grants it contains (the sealed index in the machine directory, `backup::index_path`), written as
    /// the record's `index` member, `true`; a record without it, or with any other value, is refused by name.
    /// Newer items are counted as those not in the index, regardless of deletions since (`backup::measured`);
    /// an index that is there but does not read is measured by plain count until the next backup.
    pub count: u64,
}

/// How many ledger entries and held grants were added since the last whole-machine backup, as shown by the
/// setup check, the wizard, the watch table and the settings page. `now` comes from `backup::measured`. `None`
/// when never backed up or not yet measured (unknown, never "zero behind").
pub fn backup_behind(last: Option<&Backed>, now: Option<u64>) -> Option<u64> {
    Some(now?.saturating_sub(last?.count))
}

/// The allowed appearances.
pub const APPEARANCES: [&str; 3] = [appearance::LIGHT, appearance::DARK, appearance::SYSTEM];

/// Appearance values.
pub mod appearance {
    pub const LIGHT: &str = "light";
    pub const DARK: &str = "dark";
    pub const SYSTEM: &str = "system";
}

/// How the command line's sending is handled, and how long its credential lasts: read where the command line
/// reads them too (`zikaron_seal::machine`).
pub use zikaron_seal::machine::{CliAnchor, Lasts, LASTS_MAX, LASTS_MIN, NEVER};

/// Which way the command line writes now. Exactly one at any time: writing alone outranks the desktop's door,
/// so while it is on the door is closed whatever its own switch says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliPath {
    /// Through the desktop's door (the desktop switch on, writing alone off).
    Desktop,
    /// Alone, with the credential (writing alone on; the desktop switch is greyed out).
    Alone,
    /// Neither (both off).
    Closed,
}

/// Member names of the machine file (top level and the backup record).
pub mod member {
    pub const SHAPE: &str = "shape";
    pub const AUTO_LOCK: &str = "auto_lock";
    pub const AUTO_LOCK_SECS: &str = "auto_lock_secs";
    pub const BACKUP: &str = "backup";
    pub const NETWORK: &str = "network";
    pub const APPEARANCE: &str = "appearance";
    pub const LANG: &str = "lang";
    pub use zikaron_seal::machine::member::{CLI_ALONE, CLI_ANCHOR, PROXY};
    pub const CLI_DESKTOP: &str = "cli_desktop";
    pub const BACKUP_FAILED: &str = "backup_failed";
    pub const AT: &str = "at";
    pub const COUNT: &str = "count";
    pub const INDEX: &str = "index";
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
    /// When the last attempted whole-machine backup failed (could not be written, or did not read back), in
    /// Unix seconds; `None` when the last attempt succeeded or none was made. A successful backup clears it;
    /// the watch table and setup check show it as red.
    pub backup_failed: Option<u64>,
    /// The network the wizard last chose (row name, or `deploy::CUSTOM`); `None` means not chosen yet. Only
    /// decides the preselected option; no home takes its network from here.
    pub network: Option<String>,
    /// The chosen appearance, one of [`APPEARANCES`]; `None` means never chosen (light).
    pub appearance: Option<String>,
    /// The language last chosen on this machine, used by the passcode screen before unlocking (the home's own
    /// choice is sealed and read afterwards); `None` means never chosen.
    pub lang: Option<crate::lang::Lang>,
    /// Every data folder this machine has used as its own (not an identity's role folder), in first-use order.
    /// A master key change reseals them all and the whole-machine backup includes them all, so a folder used
    /// before is never left under a lost key.
    pub homes: Vec<String>,
    /// When each set-aside data folder among `homes` was set aside (path, Unix seconds), shown as a date.
    pub aside_at: Vec<(String, u64)>,
    /// The proxy choice as written: `system`, `none`, or a proxy address; `None` means never chosen (follow
    /// the system). An unreadable value is treated as `system` and written back unchanged until the user
    /// chooses again ([`proxy_choice`]).
    pub proxy: Option<String>,
    /// How the CLI's `anchor` command is handled; written only when not the default.
    pub cli_anchor: CliAnchor,
    /// Whether the command line may write through the desktop's door; written only when off.
    pub cli_desktop: bool,
    /// Whether the command line writes alone, and how long its credential lasts; `None` is off.
    pub cli_alone: Option<Lasts>,
    /// Unknown members, preserved unchanged.
    extra: Vec<(String, Value)>,
    /// Whether the sealed part was read (unlocked); a change to it is written only then.
    held: bool,
}

impl Machine {
    /// Which way the command line writes now ([`CliPath`]).
    pub fn cli_path(&self) -> CliPath {
        match (self.cli_alone.is_some(), self.cli_desktop) {
            (true, _) => CliPath::Alone,
            (false, true) => CliPath::Desktop,
            (false, false) => CliPath::Closed,
        }
    }

    /// Whether the sealed part of these settings was read (they are real, not the defaults shown while locked).
    pub fn held(&self) -> bool {
        self.held
    }
}

impl Default for Machine {
    fn default() -> Self {
        Machine { auto_lock: LOCK_ON_DEFAULT, auto_lock_secs: LOCK_DEFAULT, backup: None, backup_failed: None, network: None, appearance: None, lang: None, homes: Vec::new(), aside_at: Vec::new(), proxy: None, cli_anchor: CliAnchor::Send, cli_desktop: true, cli_alone: None, extra: Vec::new(), held: false }
    }
}

/// Where the plain file is.
pub fn path() -> Result<std::path::PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(FILE))
}

/// Where the sealed file is.
pub fn sealed_path() -> Result<std::path::PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(crate::places::machine_file()))
}

/// The plain part: the language and appearance (no file means neither chosen).
fn read_plain(into: &mut Machine) -> Result<(), Fault> {
    let p = path()?;
    match std::fs::read(&p) {
        Ok(b) => parse_plain(&b, &p, into),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(crate::fault::classify(&e, &p.display().to_string())),
    }
}

/// The sealed part (no file means defaults); `LOCKED` while locked.
fn read_sealed() -> Result<Machine, Fault> {
    let p = sealed_path()?;
    let mut m = match crate::local::read(&p, crate::local::Doc::MachineSealed)? {
        Some(b) => parse(&b, &p)?,
        // No file yet: defaults, held only with the key in hand (a first run before its passcode has none).
        None if crate::keybox::local_key().is_err() => return Ok(Machine::default()),
        None => Machine::default(),
    };
    // Only the plain file says these.
    m.lang = None;
    m.appearance = None;
    m.held = true;
    remember_route(&m);
    Ok(m)
}

/// Read the settings, both parts. No file means defaults; a wrong shape is an error, and so is the vault being
/// locked (the sealed part is not in hand).
pub fn read() -> Result<Machine, Fault> {
    let mut m = read_sealed()?;
    read_plain(&mut m)?;
    Ok(m)
}

/// Read what can be read now: the plain part always, the sealed part when unlocked (otherwise its defaults, and
/// [`Machine::held`] false). For the screens shown before unlocking (language, appearance) and the start.
pub fn read_face() -> Result<Machine, Fault> {
    let mut m = match read_sealed() {
        Ok(m) => m,
        // The key is not in hand (locked, or no vault yet): the sealed part waits for it.
        Err(_) if crate::keybox::local_key().is_err() => Machine::default(),
        Err(f) => return Err(f),
    };
    read_plain(&mut m)?;
    Ok(m)
}

/// Write the plain part given as its file's bytes (a restore's staged plain settings), validated like the file.
pub fn write_plain_bytes(bytes: &[u8]) -> Result<(), Fault> {
    let p = path()?;
    let mut m = Machine::default();
    parse_plain(bytes, &p, &mut m)?;
    let _turn = turn();
    crate::home::put_at(&crate::home::machine_dir()?, FILE, &plain_bytes(&m)?)
}

/// Process-wide lock held by every write and every read-modify-write ([`update`]). Without it a background
/// change (a backup record, a home set aside) and a UI change (language, proxy) could both read before either
/// writes, and the later write would undo the earlier one. Across processes, the home's single-writer lock
/// applies.
fn turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

/// Read, modify and write the settings under the lock, returning what was written. All field changes go
/// through here. Unreadable settings are never overwritten (the read fails first). While locked only the plain
/// part can change: a change to the sealed part is refused (`LOCKED`) and nothing is written.
pub fn update(change: impl FnOnce(&mut Machine)) -> Result<Machine, Fault> {
    let _turn = turn();
    let mut m = read_face()?;
    let before = sealed_bytes(&m)?;
    change(&mut m);
    if !m.held && sealed_bytes(&m)? != before {
        return Err(Fault::known(Known::Locked, crate::places::machine_file()));
    }
    write_now(&m)?;
    Ok(m)
}

/// The plain file: only `appearance` and `lang` are read; every other member is not, and goes at the next
/// write (an earlier version kept every setting here).
fn parse_plain(bytes: &[u8], p: &std::path::Path, into: &mut Machine) -> Result<(), Fault> {
    let v = json::parse(bytes).map_err(|t| Fault::known(Known::MachineShape, format!("{}: {t:?}", p.display())))?;
    let Value::Obj(members) = v else {
        return Err(Fault::known(Known::MachineShape, p.display().to_string()));
    };
    for (k, val) in members {
        match (k.as_str(), &val) {
            (member::APPEARANCE, Value::Str(a)) if APPEARANCES.contains(&a.as_str()) => into.appearance = Some(a.clone()),
            (member::APPEARANCE, _) => return Err(Fault::known(Known::MachineShape, member::APPEARANCE.to_string())),
            (member::LANG, Value::Str(l)) if crate::lang::Lang::ALL.iter().any(|x| x.as_str() == l) => {
                into.lang = crate::lang::Lang::ALL.into_iter().find(|x| x.as_str() == l);
            }
            (member::LANG, _) => return Err(Fault::known(Known::MachineShape, member::LANG.to_string())),
            _ => {}
        }
    }
    Ok(())
}

/// The plain file's bytes: `appearance` and `lang` when chosen, nothing else. An appearance not in
/// [`APPEARANCES`] is an error and nothing is written.
pub fn plain_bytes(m: &Machine) -> Result<Vec<u8>, Fault> {
    if let Some(a) = m.appearance.as_ref().filter(|a| !APPEARANCES.contains(&a.as_str())) {
        return Err(Fault::known(Known::MachineShape, a.clone()));
    }
    let mut members: Vec<(String, Value)> = Vec::new();
    if let Some(a) = &m.appearance {
        members.push((member::APPEARANCE.into(), Value::Str(a.clone())));
    }
    if let Some(l) = m.lang {
        members.push((member::LANG.into(), Value::Str(l.as_str().to_string())));
    }
    Ok(json::canon_bytes(&Value::Obj(members)))
}

/// The sealed file's bytes: the whole ([`to_bytes`]) without `appearance` and `lang`.
pub fn sealed_bytes(m: &Machine) -> Result<Vec<u8>, Fault> {
    to_bytes(&Machine { appearance: None, lang: None, ..m.clone() })
}

fn parse(bytes: &[u8], p: &std::path::Path) -> Result<Machine, Fault> {
    let v = json::parse(bytes).map_err(|t| Fault::known(Known::MachineShape, format!("{}: {t:?}", p.display())))?;
    let Value::Obj(members) = v else {
        return Err(Fault::known(Known::MachineShape, p.display().to_string()));
    };
    let mut out = Machine::default();
    let mut cli = zikaron_seal::machine::Cli::default();
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
                let indexed = matches!(b.member(member::INDEX), Some(Value::Bool(true)));
                match (int(member::AT), int(member::COUNT), path, indexed) {
                    (Some(at), Some(count), Some(path), true) => out.backup = Some(Backed { at, path, count }),
                    _ => return Err(Fault::known(Known::MachineShape, member::BACKUP.to_string())),
                }
            }
            (member::BACKUP_FAILED, Value::Int(at)) => out.backup_failed = Some(*at),
            (member::BACKUP_FAILED, _) => return Err(Fault::known(Known::MachineShape, member::BACKUP_FAILED.to_string())),
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
            (member::CLI_DESKTOP, Value::Bool(b)) => out.cli_desktop = *b,
            (member::CLI_DESKTOP, _) => return Err(Fault::known(Known::MachineShape, member::CLI_DESKTOP.to_string())),
            // The proxy, the handling of the command line's sending and writing alone: read by the sealed layer,
            // where the command line reads them with its credential.
            _ if zikaron_seal::machine::take(&mut cli, &k, &val)? => {}
            _ => out.extra.push((k, val)),
        }
    }
    if !shape_ok {
        return Err(Fault::known(Known::MachineShape, member::SHAPE.to_string()));
    }
    let zikaron_seal::machine::Cli { proxy, cli_anchor, cli_alone } = cli;
    Ok(Machine { proxy, cli_anchor, cli_alone, ..out })
}

/// Write the settings. An invalid value (e.g. an auto-lock time not in [`LOCK_CHOICES`]) is an error and the
/// files are untouched.
pub fn write(m: &Machine) -> Result<(), Fault> {
    let _turn = turn();
    write_now(m)
}

/// [`write`], with the lock already held: the sealed part first (when held), then the plain part.
fn write_now(m: &Machine) -> Result<(), Fault> {
    let sealed = sealed_bytes(m)?;
    let plain = plain_bytes(m)?;
    let dir = crate::home::machine_dir()?;
    if m.held {
        crate::local::put(&dir, &crate::places::machine_file(), crate::local::Doc::MachineSealed, &sealed)?;
        remember_route(m);
    }
    crate::home::put_at(&dir, FILE, &plain)
}

/// Parse settings from their whole bytes with the file's checks (a restore validates the backup's copy before
/// staging).
pub fn from_bytes(bytes: &[u8]) -> Result<Machine, Fault> {
    parse(bytes, &path()?)
}

/// Record a data folder this machine uses as its own (called when the home pointer is written), once.
pub fn remember_home(at: &std::path::Path) -> Result<(), Fault> {
    let _turn = turn();
    let mut m = read()?;
    if m.homes.iter().any(|h| crate::home::same_place(std::path::Path::new(h), at)) {
        return Ok(());
    }
    m.homes.push(at.display().to_string());
    write_now(&m)
}

/// The file bytes for these settings, after validation.
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
    let mut members: Vec<(String, Value)> = Vec::new();
    if let Some(a) = &m.appearance {
        members.push((member::APPEARANCE.into(), Value::Str(a.clone())));
    }
    if !m.auto_lock {
        members.push((member::AUTO_LOCK.into(), Value::Bool(false)));
    }
    if m.cli_anchor != CliAnchor::Send {
        members.push((member::CLI_ANCHOR.into(), Value::Str(m.cli_anchor.as_str().to_string())));
    }
    if !m.cli_desktop {
        members.push((member::CLI_DESKTOP.into(), Value::Bool(false)));
    }
    match m.cli_alone {
        Some(l) if !l.ok() => return Err(Fault::known(Known::MachineShape, member::CLI_ALONE.to_string())),
        Some(Lasts::Secs(n)) => members.push((member::CLI_ALONE.into(), Value::Int(n))),
        Some(Lasts::Never) => members.push((member::CLI_ALONE.into(), Value::Str(NEVER.to_string()))),
        None => {}
    }
    members.push((member::AUTO_LOCK_SECS.into(), Value::Int(m.auto_lock_secs)));
    if let Some(l) = m.lang {
        members.push((member::LANG.into(), Value::Str(l.as_str().to_string())));
    }
    if let Some(b) = &m.backup {
        members.push((
            member::BACKUP.into(),
            Value::Obj(
                [
                    vec![(member::AT.into(), Value::Int(b.at)), (member::COUNT.into(), Value::Int(b.count))],
                    vec![(member::INDEX.into(), Value::Bool(true))],
                    vec![(member::PATH.into(), Value::Str(b.path.clone()))],
                ]
                .concat(),
            ),
        ));
    }
    if let Some(at) = m.backup_failed {
        members.push((member::BACKUP_FAILED.into(), Value::Int(at)));
    }
    if let Some(n) = &m.network {
        members.push((member::NETWORK.into(), Value::Str(n.clone())));
    }
    if let Some(p) = &m.proxy {
        members.push((member::PROXY.into(), Value::Str(p.clone())));
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
    // Unknown members are written back unchanged, except under a name this version writes: e.g. an
    // unreadable non-text proxy value gives way to the one the user has since chosen. One member per name.
    let written: Vec<String> = members.iter().map(|(k, _)| k.clone()).collect();
    members.extend(m.extra.iter().filter(|(k, _)| !written.contains(k)).cloned());
    members.sort_by(|a, b| a.0.cmp(&b.0));
    let bytes = json::canon_bytes(&Value::Obj(members));
    // Validate with the reader (an integer above the canonical limit, for example, would write but never read
    // back); on failure the file is untouched.
    if let Err(t) = json::parse(&bytes) {
        return Err(Fault::known(Known::MachineShape, format!("{t:?}")));
    }
    Ok(bytes)
}

/// Proxy setting values in the machine settings.
pub use zikaron_seal::machine::proxy;

/// The effective proxy choice, by the one reading of the member (the sender's `node::route_of`, through
/// `chainx`; the command line's with its credential too): unset or `system` follows the system, `none` disables
/// the proxy, a valid address is used as the proxy. An unreadable value follows the system (and stays in the file
/// as written).
pub fn proxy_choice(m: &Machine) -> zikaron_net::Choice {
    crate::chainx::route_of(m.proxy.as_deref())
}

/// The route last read from or written to the sealed settings in this process, kept without the password: what
/// a connection opened while the vault is locked goes by.
static LAST_ROUTE: std::sync::Mutex<Option<zikaron_net::Choice>> = std::sync::Mutex::new(None);

/// Keep the route of settings read or written with the key in hand (every sealed read and every change goes
/// through here, so a choice saved just before locking is the one a locked connection goes by).
fn remember_route(m: &Machine) {
    *LAST_ROUTE.lock().unwrap_or_else(|e| e.into_inner()) = Some(without_password(proxy_choice(m)));
}

/// A choice as it may be kept while locked: a typed proxy without its password.
fn without_password(c: zikaron_net::Choice) -> zikaron_net::Choice {
    match c {
        zikaron_net::Choice::Manual(mut p) => {
            if let Some(a) = p.auth.as_mut() {
                a.pass = None;
            }
            zikaron_net::Choice::Manual(p)
        }
        other => other,
    }
}

/// The current proxy choice, read when a new connection opens. Unlocked: the sealed settings as they are now
/// (an unreadable value, or settings that do not read, follow the system, as the default does). Locked, with
/// the sealed part not in hand: the route last read in this process, without its password (a proxy that asks
/// for one is then refused by name until unlocking); the system's settings when none was read yet (nothing
/// connects before the first unlock).
pub fn proxy_now() -> zikaron_net::Choice {
    match read_face() {
        Ok(m) if m.held => proxy_choice(&m),
        Ok(_) => LAST_ROUTE.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or(zikaron_net::Choice::System),
        Err(_) => zikaron_net::Choice::System,
    }
}

/// How new connections go now, as one line: through which proxy (its user name if any, never its password), or
/// direct with the reason when it is not the setting itself (node on this machine, on a local network under a
/// custom proxy, system settings not read yet); the system's script deciding when its answer is not known yet;
/// and, when the system's script could not be used, its trouble by name (a script on another machine over plain
/// `http` also says what to do: an `https` address, or a custom proxy).
pub fn proxy_said(r: &zikaron_net::Reading) -> String {
    use crate::lang::{filln, t, Key};
    use zikaron_net::{Script, Way};
    let base = match &r.way {
        _ if r.script == Script::Pending => t(Key::ProxyNowScriptPending).to_string(),
        Way::Through(p) => filln(Key::ProxyNowVia, &[&p.shown()]),
        Way::Direct if r.loopback => t(Key::ProxyNowLoopback).to_string(),
        Way::Direct if r.local => t(Key::ProxyNowLocal).to_string(),
        Way::Direct if r.system_unread => t(Key::ProxyNowUnread).to_string(),
        Way::Direct => t(Key::ProxyNowDirect).to_string(),
    };
    match r.script {
        Script::Trouble(x @ zikaron_net::PacTrouble::RemoteHttp) => filln(Key::ProxyScriptRemoteHttp, &[&base, x.code()]),
        Script::Trouble(x) => filln(Key::ProxyScriptTrouble, &[&base, x.code()]),
        _ => base,
    }
}

/// Values the `network` member accepts: a table row, or "custom".
fn network_ok(n: &str) -> bool {
    crate::deploy::is_choice(n)
}

/// The chosen deployment row (`None` when not chosen or "custom").
pub fn chosen(m: &Machine) -> Option<&'static crate::deploy::Deployment> {
    m.network.as_deref().and_then(crate::deploy::named)
}

/// The preselected network in the wizard and the new-identity sheet: the last choice, or the default row.
pub fn pick(m: &Machine) -> String {
    m.network.clone().unwrap_or_else(|| crate::deploy::DEFAULT.to_string())
}

/// Whether the idle time is up: last user input at `last`, current time `now` (same clock, seconds), limit
/// `secs`. The caller supplies `now` (the UI clock in the window, fixed values in tests).
pub fn idle_due(last: f64, now: f64, secs: u64) -> bool {
    now - last >= secs as f64
}

/// Whether the app should auto-lock now: the switch is on and the idle time is up. The window only supplies
/// the last input time (`Shell::idle_tick`).
pub fn idle_locks(m: &Machine, last: f64, now: f64) -> bool {
    m.auto_lock && idle_due(last, now, m.auto_lock_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Unknown members and the proxy value round-trip; the backup `index` member reads and writes, and a record
    /// without it, or one that says it has no index, is refused by name; an unreadable proxy value follows the
    /// system.
    #[test]
    fn unknown_members_and_the_proxy_cell_are_carried() {
        let p = std::path::Path::new("machine.json");
        assert!(parse(format!("{{\"backup\":{{\"at\":5,\"count\":2,\"path\":\"/b\"}},\"shape\":\"{SHAPE}\"}}").as_bytes(), p).is_err(), "a backup record without its index member");
        assert!(parse(format!("{{\"backup\":{{\"at\":5,\"count\":2,\"index\":false,\"path\":\"/b\"}},\"shape\":\"{SHAPE}\"}}").as_bytes(), p).is_err(), "a backup record without an index");
        let text = format!("{{\"backup\":{{\"at\":5,\"count\":2,\"index\":true,\"path\":\"/b\"}},\"future\":{{\"x\":1}},\"proxy\":\"pac://later\",\"shape\":\"{SHAPE}\"}}");
        let m = parse(text.as_bytes(), p).expect("reads");
        assert_eq!(m.backup.as_ref().map(|b| b.count), Some(2));
        assert_eq!(proxy_choice(&m), zikaron_net::Choice::System);
        let back = String::from_utf8(to_bytes(&m).expect("writes")).expect("utf-8");
        assert!(back.contains("\"future\":{\"x\":1}") && back.contains("\"proxy\":\"pac://later\""), "{back}");
        let m2 = Machine { backup: Some(Backed { at: 5, path: "/b".into(), count: 3 }), proxy: Some(proxy::NONE.into()), ..m };
        let again = parse(&to_bytes(&m2).expect("writes"), p).expect("reads back");
        assert_eq!((again.backup.clone(), proxy_choice(&again)), (m2.backup.clone(), zikaron_net::Choice::Off));
        let manual = Machine { proxy: Some("socks5://127.0.0.1:1080".into()), ..Machine::default() };
        assert!(matches!(proxy_choice(&manual), zikaron_net::Choice::Manual(_)));
        assert!(parse(format!("{{\"backup\":{{\"at\":5,\"count\":2,\"index\":1,\"path\":\"/b\"}},\"shape\":\"{SHAPE}\"}}").as_bytes(), p).is_err(), "the index member is a truth value");
    }

    /// A known member with an unknown value fails the whole file with `MACHINE_SHAPE`, so an older version
    /// reading a newer version's file reports the error and uses defaults, leaving the file untouched.
    #[test]
    fn a_known_member_with_a_value_this_version_does_not_know_refuses_the_whole_file() {
        let p = std::path::Path::new("machine.json");
        for (name, value) in [
            (member::NETWORK, "\"a-network-of-later\""),
            (member::AUTO_LOCK_SECS, "7"),
            (member::APPEARANCE, "\"sepia\""),
            (member::LANG, "\"fr\""),
        ] {
            let text = format!("{{\"{name}\":{value},\"shape\":\"{SHAPE}\"}}");
            let refused = parse(text.as_bytes(), p).err();
            assert!(matches!(refused.as_ref().and_then(|f| f.which()), Some(Known::MachineShape)), "{name}: {:?}", refused.map(|f| f.said().to_string()));
        }
    }

    /// `cli_anchor`: absent and `send` mean send (and `send` is not written back, so an unchanged file keeps its
    /// bytes); `queue` round-trips; any other value fails the whole file.
    #[test]
    fn the_command_line_anchoring_cell_reads_each_form() {
        let p = std::path::Path::new("machine.json");
        let read = |cell: &str| parse(format!("{{{cell}\"shape\":\"{SHAPE}\"}}").as_bytes(), p);
        assert_eq!(read("").expect("absent").cli_anchor, CliAnchor::Send);
        let sent = read("\"cli_anchor\":\"send\",").expect("send");
        assert_eq!(sent.cli_anchor, CliAnchor::Send);
        assert!(!String::from_utf8(to_bytes(&sent).expect("writes")).expect("utf-8").contains(member::CLI_ANCHOR));
        let queued = read("\"cli_anchor\":\"queue\",").expect("queue");
        assert_eq!(queued.cli_anchor, CliAnchor::Queue);
        assert_eq!(parse(&to_bytes(&queued).expect("writes"), p).expect("reads back").cli_anchor, CliAnchor::Queue);
        for bad in ["\"cli_anchor\":\"later\",", "\"cli_anchor\":\"Queue\",", "\"cli_anchor\":1,", "\"cli_anchor\":null,"] {
            assert!(matches!(read(bad).err().and_then(|f| f.which()), Some(Known::MachineShape)), "{bad}");
        }
    }

    /// A non-text proxy value (from a later version) follows the system and is written back once, unchanged;
    /// a new choice replaces it, never leaving two `proxy` members.
    #[test]
    fn a_proxy_cell_that_is_not_text_is_carried_until_chosen_again() {
        let p = std::path::Path::new("machine.json");
        let text = format!("{{\"proxy\":{{\"mode\":\"pac\",\"url\":\"x\"}},\"shape\":\"{SHAPE}\"}}");
        let m = parse(text.as_bytes(), p).expect("reads");
        assert_eq!((m.proxy.clone(), proxy_choice(&m)), (None, zikaron_net::Choice::System));
        let back = String::from_utf8(to_bytes(&m).expect("writes")).expect("utf-8");
        assert_eq!(back.matches("\"proxy\"").count(), 1, "{back}");
        assert!(back.contains("\"proxy\":{\"mode\":\"pac\",\"url\":\"x\"}"), "{back}");
        let chosen = Machine { proxy: Some(proxy::NONE.into()), ..m };
        let again = String::from_utf8(to_bytes(&chosen).expect("writes")).expect("utf-8");
        assert_eq!(again.matches("\"proxy\"").count(), 1, "{again}");
        assert!(again.contains("\"proxy\":\"none\""), "{again}");
        assert_eq!(proxy_choice(&parse(again.as_bytes(), p).expect("reads back")), zikaron_net::Choice::Off);
    }

    /// The four states of the command line's two switches, one test each ("the higher outranks the lower"):
    /// both off, no path; through the desktop alone, the door; writing alone on, alone whatever the desktop
    /// switch says (it is greyed and the door stays closed).
    fn path_of(desktop: bool, alone: Option<Lasts>) -> CliPath {
        Machine { cli_desktop: desktop, cli_alone: alone, ..Machine::default() }.cli_path()
    }

    #[test]
    fn both_switches_off_leave_the_command_line_no_way_to_write() {
        assert_eq!(path_of(false, None), CliPath::Closed);
    }

    #[test]
    fn through_the_desktop_on_and_alone_off_is_the_door() {
        assert_eq!(path_of(true, None), CliPath::Desktop);
        assert_eq!(Machine::default().cli_path(), CliPath::Desktop, "the default is the door, as before the switches");
    }

    #[test]
    fn alone_on_outranks_the_desktop_switch_on() {
        assert_eq!(path_of(true, Some(Lasts::Secs(3_600))), CliPath::Alone);
    }

    #[test]
    fn alone_on_with_the_desktop_switch_off_is_alone_too() {
        assert_eq!(path_of(false, Some(Lasts::Never)), CliPath::Alone);
    }

    /// The switches are sealed members: written only when not the default, read back as written; a validity out
    /// of range, or any other value, fails the whole file. The plain file never carries them.
    #[test]
    fn the_command_line_switches_round_trip_in_the_sealed_part_only() {
        let p = std::path::Path::new("machine.sealed");
        let m = Machine { cli_desktop: false, cli_alone: Some(Lasts::Secs(86_400)), lang: Some(crate::lang::Lang::En), ..Machine::default() };
        let sealed = sealed_bytes(&m).expect("writes");
        let back = parse(&sealed, p).expect("reads back");
        assert_eq!((back.cli_desktop, back.cli_alone), (false, Some(Lasts::Secs(86_400))));
        let plain = String::from_utf8(plain_bytes(&m).expect("writes")).expect("utf-8");
        assert_eq!(plain, "{\"lang\":\"en\"}", "only the language and appearance stay plain");
        let never = Machine { cli_alone: Some(Lasts::Never), ..Machine::default() };
        assert_eq!(parse(&sealed_bytes(&never).expect("writes"), p).expect("reads").cli_alone, Some(Lasts::Never));
        assert!(!String::from_utf8(sealed_bytes(&Machine::default()).expect("writes")).expect("utf-8").contains("cli_"), "defaults are not written");
        for bad in ["\"cli_alone\":59,", "\"cli_alone\":31536001,", "\"cli_alone\":\"later\",", "\"cli_desktop\":1,"] {
            let text = format!("{{{bad}\"shape\":\"{SHAPE}\"}}");
            assert!(matches!(parse(text.as_bytes(), p).err().and_then(|f| f.which()), Some(Known::MachineShape)), "{bad}");
        }
        assert!(to_bytes(&Machine { cli_alone: Some(Lasts::Secs(1)), ..Machine::default() }).is_err(), "out of range is not written");
    }

    /// The plain file reads its two members and nothing else: an earlier version's members are not read.
    #[test]
    fn the_plain_file_reads_the_language_and_appearance_only() {
        let p = std::path::Path::new("machine.json");
        let mut m = Machine::default();
        parse_plain(format!("{{\"appearance\":\"dark\",\"auto_lock_secs\":300,\"lang\":\"en\",\"shape\":\"{SHAPE}\"}}").as_bytes(), p, &mut m).expect("reads");
        assert_eq!((m.appearance.as_deref(), m.lang, m.auto_lock_secs), (Some(appearance::DARK), Some(crate::lang::Lang::En), LOCK_DEFAULT));
        assert!(parse_plain(b"{\"lang\":\"fr\"}", p, &mut Machine::default()).is_err(), "a language this version does not know");
        assert!(plain_bytes(&Machine { appearance: Some("sepia".into()), ..Machine::default() }).is_err(), "an appearance not in the table is not written");
    }

    /// Idle locking is due exactly at the limit, not before.
    #[test]
    fn idle_is_due_at_the_mark_and_not_before() {
        assert!(!idle_due(100.0, 159.9, 60));
        assert!(idle_due(100.0, 160.0, 60));
    }
}
