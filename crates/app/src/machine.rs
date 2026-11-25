use crate::fault::{Fault, Known};
use zikaron::json::{self, Value};

pub const SHAPE: &str = "zikaron-desk/machine/1";
pub const FILE: &str = "machine.json";

pub const LOCK_CHOICES: [u64; 5] = [60, 300, 900, 1800, 3600];
pub const LOCK_DEFAULT: u64 = 900;
pub const LOCK_ON_DEFAULT: bool = true;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backed {
    pub at: u64,
    pub path: String,
    pub count: u64,
}

pub fn backup_behind(last: Option<&Backed>, now: Option<u64>) -> Option<u64> {
    Some(now?.saturating_sub(last?.count))
}

pub const APPEARANCES: [&str; 3] = [appearance::LIGHT, appearance::DARK, appearance::SYSTEM];

pub mod appearance {
    pub const LIGHT: &str = "light";
    pub const DARK: &str = "dark";
    pub const SYSTEM: &str = "system";
}

pub use zikaron_seal::machine::{CliAnchor, Lasts, LASTS_MAX, LASTS_MIN, NEVER};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliPath {
    Desktop,
    Alone,
    Closed,
}

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

#[derive(Clone, Debug, PartialEq)]
pub struct Machine {
    pub auto_lock: bool,
    pub auto_lock_secs: u64,
    pub backup: Option<Backed>,
    pub backup_failed: Option<u64>,
    pub network: Option<String>,
    pub appearance: Option<String>,
    pub lang: Option<crate::lang::Lang>,
    pub homes: Vec<String>,
    pub aside_at: Vec<(String, u64)>,
    pub proxy: Option<String>,
    pub cli_anchor: CliAnchor,
    pub cli_desktop: bool,
    pub cli_alone: Option<Lasts>,
    pub rest_keys: zikaron_seal::machine::RestKeys,
    extra: Vec<(String, Value)>,
    held: bool,
}

impl Machine {
    pub fn cli_path(&self) -> CliPath {
        match (self.cli_alone.is_some(), self.cli_desktop) {
            (true, _) => CliPath::Alone,
            (false, true) => CliPath::Desktop,
            (false, false) => CliPath::Closed,
        }
    }

    pub fn held(&self) -> bool {
        self.held
    }
}

impl Default for Machine {
    fn default() -> Self {
        Machine { auto_lock: LOCK_ON_DEFAULT, auto_lock_secs: LOCK_DEFAULT, backup: None, backup_failed: None, network: None, appearance: None, lang: None, homes: Vec::new(), aside_at: Vec::new(), proxy: None, cli_anchor: CliAnchor::Send, cli_desktop: true, cli_alone: None, rest_keys: zikaron_seal::machine::RestKeys::default(), extra: Vec::new(), held: false }
    }
}

pub fn path() -> Result<std::path::PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(FILE))
}

pub fn sealed_path() -> Result<std::path::PathBuf, Fault> {
    Ok(crate::home::machine_dir()?.join(crate::places::machine_file()))
}

fn read_plain(into: &mut Machine) -> Result<(), Fault> {
    let p = path()?;
    match std::fs::read(&p) {
        Ok(b) => parse_plain(&b, &p, into),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(crate::fault::classify(&e, &p.display().to_string())),
    }
}

fn read_sealed() -> Result<Machine, Fault> {
    let p = sealed_path()?;
    let mut m = match crate::local::read(&p, crate::local::Doc::MachineSealed)? {
        Some(b) => parse(&b, &p)?,
        None if crate::keybox::local_key().is_err() => return Ok(Machine::default()),
        None => Machine::default(),
    };
    m.lang = None;
    m.appearance = None;
    m.held = true;
    remember_route(&m);
    Ok(m)
}

pub fn read() -> Result<Machine, Fault> {
    crate::reads::touch();
    let mut m = read_sealed()?;
    read_plain(&mut m)?;
    follow(&mut m);
    Ok(m)
}

fn follow(m: &mut Machine) {
    let Ok(dir) = crate::home::machine_dir() else { return };
    let here = |h: &str| crate::home::here_under(&dir, std::path::Path::new(h)).display().to_string();
    for h in m.homes.iter_mut() {
        *h = here(h);
    }
    for (h, _) in m.aside_at.iter_mut() {
        *h = here(h);
    }
}

pub fn read_face() -> Result<Machine, Fault> {
    let mut m = match read_sealed() {
        Ok(m) => m,
        Err(_) if crate::keybox::local_key().is_err() => Machine::default(),
        Err(f) => return Err(f),
    };
    read_plain(&mut m)?;
    follow(&mut m);
    Ok(m)
}

pub fn write_plain_bytes(bytes: &[u8]) -> Result<(), Fault> {
    let p = path()?;
    let mut m = Machine::default();
    parse_plain(bytes, &p, &mut m)?;
    let _turn = turn();
    crate::home::put_at(&crate::home::machine_dir()?, FILE, &plain_bytes(&m)?)
}

fn turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

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
            _ if zikaron_seal::machine::take(&mut cli, &k, &val)? => {}
            _ => out.extra.push((k, val)),
        }
    }
    if !shape_ok {
        return Err(Fault::known(Known::MachineShape, member::SHAPE.to_string()));
    }
    let zikaron_seal::machine::Cli { proxy, cli_anchor, cli_alone, rest_keys } = cli;
    Ok(Machine { proxy, cli_anchor, cli_alone, rest_keys, ..out })
}

pub fn write(m: &Machine) -> Result<(), Fault> {
    let _turn = turn();
    write_now(m)
}

fn write_now(m: &Machine) -> Result<(), Fault> {
    let sealed = sealed_bytes(m)?;
    let plain = plain_bytes(m)?;
    let dir = crate::home::machine_dir()?;
    if m.held {
        crate::local::put(&dir, &crate::places::machine_file(), crate::local::Doc::MachineSealed, &sealed)?;
        remember_route(m);
    }
    crate::home::put_at(&dir, FILE, &plain)?;
    crate::reads::wrote(crate::reads::Copies::Machine);
    Ok(())
}

pub fn from_bytes(bytes: &[u8]) -> Result<Machine, Fault> {
    parse(bytes, &path()?)
}

pub fn remember_home(at: &std::path::Path) -> Result<(), Fault> {
    let _turn = turn();
    let mut m = read()?;
    if m.homes.iter().any(|h| crate::home::same_place(std::path::Path::new(h), at)) {
        return Ok(());
    }
    m.homes.push(at.display().to_string());
    write_now(&m)
}

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
    if !m.rest_keys.0.is_empty() {
        members.push((zikaron_seal::machine::member::REST_KEYS.into(), m.rest_keys.value()));
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
    let written: Vec<String> = members.iter().map(|(k, _)| k.clone()).collect();
    members.extend(m.extra.iter().filter(|(k, _)| !written.contains(k)).cloned());
    members.sort_by(|a, b| a.0.cmp(&b.0));
    let bytes = json::canon_bytes(&Value::Obj(members));
    if let Err(t) = json::parse(&bytes) {
        return Err(Fault::known(Known::MachineShape, format!("{t:?}")));
    }
    Ok(bytes)
}

pub use zikaron_seal::machine::proxy;

pub fn proxy_choice(m: &Machine) -> zikaron_net::Choice {
    crate::chainx::route_of(m.proxy.as_deref())
}

static LAST_ROUTE: std::sync::Mutex<Option<zikaron_net::Choice>> = std::sync::Mutex::new(None);

fn remember_route(m: &Machine) {
    *LAST_ROUTE.lock().unwrap_or_else(|e| e.into_inner()) = Some(without_password(proxy_choice(m)));
}

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

pub fn proxy_now() -> zikaron_net::Choice {
    match read_face() {
        Ok(m) if m.held => proxy_choice(&m),
        Ok(_) => LAST_ROUTE.lock().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or(zikaron_net::Choice::System),
        Err(_) => zikaron_net::Choice::System,
    }
}

pub fn rest_keys_now() -> Vec<(String, String)> {
    match read_face() {
        Ok(m) if m.held => m.rest_keys.0,
        _ => Vec::new(),
    }
}

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

fn network_ok(n: &str) -> bool {
    crate::deploy::is_choice(n)
}

pub fn chosen(m: &Machine) -> Option<&'static crate::deploy::Deployment> {
    m.network.as_deref().and_then(crate::deploy::named)
}

pub fn pick(m: &Machine) -> String {
    m.network.clone().unwrap_or_else(|| crate::deploy::DEFAULT.to_string())
}

pub fn idle_due(last: f64, now: f64, secs: u64) -> bool {
    now - last >= secs as f64
}

pub fn idle_locks(m: &Machine, last: f64, now: f64) -> bool {
    m.auto_lock && idle_due(last, now, m.auto_lock_secs)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn rest_keys_are_sealed_settings_carried_whole_and_never_said() {
        let p = std::path::Path::new("machine.sealed");
        let keys = zikaron_seal::machine::RestKeys(vec![("api.etherscan.io".into(), "ETHERSCAN-KEY-1234".into()), ("eth.blockscout.com".into(), "proapi_abcd".into())]);
        let m = Machine { rest_keys: keys.clone(), lang: Some(crate::lang::Lang::En), ..Machine::default() };
        let sealed = sealed_bytes(&m).expect("writes");
        assert_eq!(parse(&sealed, p).expect("reads back").rest_keys, keys);
        let whole = to_bytes(&m).expect("the whole, as a backup carries it");
        assert_eq!(parse(&whole, p).expect("a restore reads it").rest_keys, keys, "a whole-machine backup keeps them");
        assert!(!String::from_utf8(plain_bytes(&m).expect("writes")).expect("utf-8").contains("KEY"), "never in the plain part");
        assert!(!String::from_utf8(sealed_bytes(&Machine::default()).expect("writes")).expect("utf-8").contains("rest_keys"), "none is not written");
        assert!(!format!("{m:?}").contains("ETHERSCAN-KEY") && !format!("{m:?}").contains("proapi_abcd"), "never said");
        assert_eq!(keys.tail("API.Etherscan.io").as_deref(), Some("1234"), "the last four, by service in any case");
        assert_eq!(keys.tail("api.other.example"), None);
        for bad in ["\"rest_keys\":[],", "\"rest_keys\":{\"a\":1},", "\"rest_keys\":{\"\":\"k\"},"] {
            let text = format!("{{{bad}\"shape\":\"{SHAPE}\"}}");
            assert!(matches!(parse(text.as_bytes(), p).err().and_then(|f| f.which()), Some(Known::MachineShape)), "{bad}");
        }
    }

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

    #[test]
    fn the_plain_file_reads_the_language_and_appearance_only() {
        let p = std::path::Path::new("machine.json");
        let mut m = Machine::default();
        parse_plain(format!("{{\"appearance\":\"dark\",\"auto_lock_secs\":300,\"lang\":\"en\",\"shape\":\"{SHAPE}\"}}").as_bytes(), p, &mut m).expect("reads");
        assert_eq!((m.appearance.as_deref(), m.lang, m.auto_lock_secs), (Some(appearance::DARK), Some(crate::lang::Lang::En), LOCK_DEFAULT));
        assert!(parse_plain(b"{\"lang\":\"fr\"}", p, &mut Machine::default()).is_err(), "a language this version does not know");
        assert!(plain_bytes(&Machine { appearance: Some("sepia".into()), ..Machine::default() }).is_err(), "an appearance not in the table is not written");
    }

    #[test]
    fn idle_is_due_at_the_mark_and_not_before() {
        assert!(!idle_due(100.0, 159.9, 60));
        assert!(idle_due(100.0, 160.0, 60));
    }
}
