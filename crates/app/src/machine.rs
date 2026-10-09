//! Machine-level settings file.
//!
//! Most settings are per home (`settings.rs`), but some belong to the machine regardless of identity or data
//! directory: auto-lock, appearance, language, network choice, proxy, backup records. They live in
//! `machine.json` in the machine directory (next to `where.json` and the key vault), written through
//! `home::put_at` (mode 0600, atomic rename). The file stays plain because the passcode screen reads the
//! appearance and language before unlocking, and nothing in it is user content.
//!
//! ─── Shape ───
//!
//! `{"appearance":"light","auto_lock":false,"auto_lock_secs":900,"backup":{"at":1790000000,"count":12,"path":"/…"},"lang":"en","network":"sepolia","shape":"zikaron-desk/machine/1"}`,
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
//! - `network`: the network last chosen in the wizard (a `deploy::KNOWN` row or `deploy::CUSTOM`); absent means
//!   not chosen (older files with only the auto-lock fields still open). It only decides which option is
//!   preselected; a home's network comes from its identity or settings (`identity::Row::network`).
//!
//! Unknown members are preserved, so changing one field never erases others (e.g. the obsolete `custom`
//! member from older versions is kept but unused). No file means all defaults. A wrong shape, or a known
//! member with an unknown value, fails the whole file with `MACHINE_SHAPE` and leaves it untouched; the shell
//! then locks with the default timeout and reports the error (never silently "never lock").

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
    /// How many ledger entries and held grants this machine had then.
    pub count: u64,
    /// Whether this backup recorded which entries and held grants it contains (the sealed index in the machine
    /// directory, `backup::index_path`). If so, newer items are counted as those not in the index, regardless
    /// of deletions since (`backup::measured`). Records from older versions (no `index` member), or with an
    /// unreadable index, are measured by plain count until the next backup.
    pub indexed: bool,
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

/// How the desktop app handles the CLI's `anchor` command: send it as the send button would, or leave the
/// entries queued for the user to send from the desktop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CliAnchor {
    /// Send at once (the default, also when the member is absent).
    #[default]
    Send,
    /// Send nothing: show the request and leave the entries queued for the user.
    Queue,
}

impl CliAnchor {
    pub const ALL: [CliAnchor; 2] = [CliAnchor::Send, CliAnchor::Queue];

    /// The value written to the file.
    pub fn as_str(self) -> &'static str {
        match self {
            CliAnchor::Send => "send",
            CliAnchor::Queue => "queue",
        }
    }
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
    pub const PROXY: &str = "proxy";
    pub const CLI_ANCHOR: &str = "cli_anchor";
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
    /// Unknown members, preserved unchanged.
    extra: Vec<(String, Value)>,
}

impl Default for Machine {
    fn default() -> Self {
        Machine { auto_lock: LOCK_ON_DEFAULT, auto_lock_secs: LOCK_DEFAULT, backup: None, backup_failed: None, network: None, appearance: None, lang: None, homes: Vec::new(), aside_at: Vec::new(), proxy: None, cli_anchor: CliAnchor::Send, extra: Vec::new() }
    }
}

/// Where the file is.
pub fn path() -> Result<std::path::PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(FILE))
}

/// Read the settings. No file means defaults; a wrong shape is an error.
pub fn read() -> Result<Machine, Fault> {
    let p = path()?;
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Machine::default()),
        Err(e) => return Err(crate::fault::classify(&e, &p.display().to_string())),
    };
    parse(&bytes, &p)
}

/// Write settings given as whole-file bytes (a restore's staged machine settings), validated like the file.
pub fn write_bytes(bytes: &[u8]) -> Result<(), Fault> {
    let m = parse(bytes, &path()?)?;
    let _turn = turn();
    write_now(&m)
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
/// through here. Unreadable settings are never overwritten (the read fails first).
pub fn update(change: impl FnOnce(&mut Machine)) -> Result<Machine, Fault> {
    let _turn = turn();
    let mut m = read()?;
    change(&mut m);
    write_now(&m)?;
    Ok(m)
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
                let indexed = match b.member(member::INDEX) {
                    None => Some(false),
                    Some(Value::Bool(x)) => Some(*x),
                    Some(_) => None,
                };
                match (int(member::AT), int(member::COUNT), path, indexed) {
                    (Some(at), Some(count), Some(path), Some(indexed)) => out.backup = Some(Backed { at, path, count, indexed }),
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
            (member::PROXY, Value::Str(p)) => out.proxy = Some(p.clone()),
            (member::CLI_ANCHOR, Value::Str(w)) if CliAnchor::ALL.iter().any(|x| x.as_str() == w) => {
                out.cli_anchor = CliAnchor::ALL.into_iter().find(|x| x.as_str() == w).unwrap_or_default();
            }
            (member::CLI_ANCHOR, _) => return Err(Fault::known(Known::MachineShape, member::CLI_ANCHOR.to_string())),
            _ => out.extra.push((k, val)),
        }
    }
    if !shape_ok {
        return Err(Fault::known(Known::MachineShape, member::SHAPE.to_string()));
    }
    Ok(out)
}

/// Write the settings. An invalid value (e.g. an auto-lock time not in [`LOCK_CHOICES`]) is an error and the
/// file is untouched.
pub fn write(m: &Machine) -> Result<(), Fault> {
    let _turn = turn();
    write_now(m)
}

/// [`write`], with the lock already held.
fn write_now(m: &Machine) -> Result<(), Fault> {
    let bytes = to_bytes(m)?;
    crate::home::put_at(&crate::home::machine_dir()?, FILE, &bytes)
}

/// Parse settings from bytes with the file's checks (a restore validates the backup's copy before staging).
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
                    if b.indexed { vec![(member::INDEX.into(), Value::Bool(true))] } else { Vec::new() },
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
pub mod proxy {
    pub const SYSTEM: &str = "system";
    pub const NONE: &str = "none";
}

/// The effective proxy choice: unset or `system` follows the system, `none` disables the proxy, a valid
/// address is used as the proxy. An unreadable value follows the system (and stays in the file as written).
pub fn proxy_choice(m: &Machine) -> zikaron_net::Choice {
    match m.proxy.as_deref().map(str::trim) {
        None | Some(proxy::SYSTEM) => zikaron_net::Choice::System,
        Some(proxy::NONE) => zikaron_net::Choice::Off,
        Some(p) => zikaron_net::proxy_of(p).map(zikaron_net::Choice::Manual).unwrap_or(zikaron_net::Choice::System),
    }
}

/// The current proxy choice (read when opening a new connection; unreadable settings follow the system, as
/// the default does).
pub fn proxy_now() -> zikaron_net::Choice {
    read().map(|m| proxy_choice(&m)).unwrap_or(zikaron_net::Choice::System)
}

/// A user-facing line for the settings page describing how new connections go: through which proxy, or
/// direct, with the reason when it is not the user's choice (node on this machine, system uses a PAC script,
/// system settings not read yet).
pub fn proxy_said(r: &zikaron_net::Reading) -> String {
    use crate::lang::{filln, t, Key};
    match &r.way {
        zikaron_net::Way::Through(p) => filln(Key::ProxyNowVia, &[&p.spelled()]),
        zikaron_net::Way::Direct if r.loopback => t(Key::ProxyNowLoopback).to_string(),
        zikaron_net::Way::Direct if r.system_unread => t(Key::ProxyNowUnread).to_string(),
        zikaron_net::Way::Direct if r.auto_config_ignored => t(Key::ProxyNowAutoConfig).to_string(),
        zikaron_net::Way::Direct => t(Key::ProxyNowDirect).to_string(),
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

    /// Unknown members and the proxy value round-trip; the backup `index` member reads and writes, and older
    /// records without it still read; an unreadable proxy value follows the system.
    #[test]
    fn unknown_members_and_the_proxy_cell_are_carried() {
        let p = std::path::Path::new("machine.json");
        let text = format!("{{\"backup\":{{\"at\":5,\"count\":2,\"path\":\"/b\"}},\"future\":{{\"x\":1}},\"proxy\":\"pac://later\",\"shape\":\"{SHAPE}\"}}");
        let m = parse(text.as_bytes(), p).expect("reads");
        assert_eq!(m.backup.as_ref().map(|b| b.indexed), Some(false));
        assert_eq!(proxy_choice(&m), zikaron_net::Choice::System);
        let back = String::from_utf8(to_bytes(&m).expect("writes")).expect("utf-8");
        assert!(back.contains("\"future\":{\"x\":1}") && back.contains("\"proxy\":\"pac://later\""), "{back}");
        let m2 = Machine { backup: Some(Backed { at: 5, path: "/b".into(), count: 2, indexed: true }), proxy: Some(proxy::NONE.into()), ..m };
        let again = parse(&to_bytes(&m2).expect("writes"), p).expect("reads back");
        assert_eq!((again.backup.as_ref().map(|b| b.indexed), proxy_choice(&again)), (Some(true), zikaron_net::Choice::Off));
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

    /// Idle locking is due exactly at the limit, not before.
    #[test]
    fn idle_is_due_at_the_mark_and_not_before() {
        assert!(!idle_due(100.0, 159.9, 60));
        assert!(idle_due(100.0, 160.0, 60));
    }
}
