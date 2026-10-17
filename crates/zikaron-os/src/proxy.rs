//! System proxy settings, read only, where each system keeps the settings in force:
//!
//! - macOS: the network configuration's proxies for the primary service, as `scutil --proxy` prints them
//!   (explicit proxies and the automatic configuration script).
//! - Windows: the current user's settings as the system answers them (`WinHttpGetIEProxyConfigForCurrentUser`:
//!   the configuration script, the proxy list and its bypass list); under the machine-wide policy
//!   `ProxySettingsPerUser = 0`, the machine's settings instead; the user's registry values when the call answers
//!   nothing.
//! - Linux and other unix: the desktop's settings where they are stored on disk (KDE's `kioslaverc`; GNOME,
//!   Cinnamon and MATE's GSettings, from the dconf database file or the keyfile backend; no program is run),
//!   else the environment (`https_proxy`, `http_proxy`, `all_proxy`, `no_proxy`, lower or upper case).
//!
//! Automatic discovery (WPAD: a script looked for by the name `wpad` on whatever network the machine is on) is
//! not read on any system: only a script at an address the person set is followed, so discovery switched on with
//! nothing else set reads as nothing set.
//!
//! Credentials written into a proxy address (`user:pass@`) are kept for the proxy that asks for them. The
//! parsers are plain functions, tested on every system.

/// One proxy as a system names it: its scheme as written (lower case, empty when none), host, port, and the
/// credentials given with it. The password is wiped when this is dropped and never printed.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Place {
    pub scheme: String,
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub pass: Option<String>,
}

impl std::fmt::Debug for Place {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Place({}://{}:{}{})", self.scheme, self.host, self.port, if self.pass.is_some() { " with a password" } else { "" })
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        if let Some(p) = self.pass.as_mut() {
            crate::wipe_string(p);
        }
    }
}

/// System proxy settings: the https, http and SOCKS proxies, the exception list and the automatic configuration
/// script's address.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Proxies {
    pub https: Option<Place>,
    pub http: Option<Place>,
    pub socks: Option<Place>,
    pub exceptions: Vec<String>,
    pub pac_url: Option<String>,
}

/// `%XX` decoded (credentials in a proxy address may be percent-encoded); a malformed escape stays as written.
fn unpercent(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = ((b[i + 1] as char).to_digit(16), (b[i + 2] as char).to_digit(16)) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Parse a proxy written as `host:port` or `scheme://[user[:pass]@]host:port[/]`. A missing port, a path, an
/// empty user, or a space or control character is rejected.
pub fn place_of(text: &str) -> Option<Place> {
    let t = text.trim();
    let (scheme, rest) = match t.split_once("://") {
        Some((s, r)) => (s.to_ascii_lowercase(), r),
        None => (String::new(), t),
    };
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    let (creds, rest) = match rest.rsplit_once('@') {
        Some((c, r)) => (Some(c), r),
        None => (None, rest),
    };
    if rest.is_empty() || rest.contains('/') || rest.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let (host, port) = rest.rsplit_once(':')?;
    let port: u16 = port.parse().ok().filter(|p| *p > 0)?;
    if host.is_empty() || (host.contains(':') && !(host.starts_with('[') && host.ends_with(']'))) {
        return None;
    }
    let (user, pass) = match creds.map(|c| c.split_once(':').unwrap_or((c, ""))) {
        None => (None, None),
        Some((u, p)) => (Some(unpercent(u)), creds.is_some_and(|c| c.contains(':')).then(|| unpercent(p))),
    };
    if user.as_deref().is_some_and(str::is_empty) {
        return None;
    }
    Some(Place { scheme, host: host.to_ascii_lowercase(), port, user, pass })
}

fn is_socks(scheme: &str) -> bool {
    matches!(scheme, "socks" | "socks5" | "socks5h" | "socks4" | "socks4a")
}

fn is_connect(scheme: &str) -> bool {
    matches!(scheme, "" | "http" | "https")
}

/// Settings from the environment: `https_proxy`, `http_proxy`, `all_proxy` and `no_proxy`, lowercase taking
/// precedence over uppercase. A SOCKS scheme sets the SOCKS proxy; `http`, `https` or none the proxy for its
/// kind; `all_proxy` fills whatever the other two left. `no_proxy` is a comma-separated exception list.
pub fn from_env(var: impl Fn(&str) -> Option<String>) -> Proxies {
    let get = |name: &str| var(name).or_else(|| var(&name.to_ascii_uppercase())).filter(|v| !v.trim().is_empty());
    let (mut https, mut http, mut socks): (Option<Place>, Option<Place>, Option<Place>) = (None, None, None);
    for (name, slot) in [("https_proxy", &mut https), ("http_proxy", &mut http)] {
        if let Some(p) = get(name).as_deref().and_then(place_of) {
            if is_socks(&p.scheme) {
                socks.get_or_insert(p);
            } else if is_connect(&p.scheme) {
                slot.get_or_insert(p);
            }
        }
    }
    if let Some(p) = get("all_proxy").as_deref().and_then(place_of) {
        if is_socks(&p.scheme) {
            socks.get_or_insert(p);
        } else if is_connect(&p.scheme) {
            https.get_or_insert(p.clone());
            http.get_or_insert(p);
        }
    }
    Proxies {
        https,
        http,
        socks,
        exceptions: get("no_proxy").map(|v| v.split(',').map(|e| e.trim().to_string()).filter(|e| !e.is_empty()).collect()).unwrap_or_default(),
        ..Default::default()
    }
}

/// A Windows proxy list (`ProxyServer`, or the list the system answers): one `host:port` (with or without
/// `http://`) for every kind; or `kind=host:port` entries separated by `;` (`http=`, `https=`, `socks=`;
/// `socks=` names a SOCKS5 proxy). Returns the https, http and SOCKS proxies.
pub fn from_windows_server(server: &str) -> (Option<Place>, Option<Place>, Option<Place>) {
    let (mut https, mut http, mut socks) = (None, None, None);
    if server.contains('=') {
        for entry in server.split(';') {
            let Some((kind, place)) = entry.split_once('=') else { continue };
            let Some(mut p) = place_of(place) else { continue };
            match kind.trim().to_ascii_lowercase().as_str() {
                "https" if is_connect(&p.scheme) => https = Some(p),
                "http" if is_connect(&p.scheme) => http = Some(p),
                "socks" => {
                    if !is_socks(&p.scheme) {
                        p.scheme = "socks5".into();
                    }
                    socks = Some(p);
                }
                _ => {}
            }
        }
    } else if let Some(p) = server.split(';').map(str::trim).find(|s| !s.is_empty()).and_then(place_of) {
        if is_socks(&p.scheme) {
            socks = Some(p);
        } else if is_connect(&p.scheme) {
            https = Some(p.clone());
            http = Some(p);
        }
    }
    (https, http, socks)
}

/// Settings from Windows: whether a proxy list is in force and the list (`server`), the bypass list
/// (`;`-separated, possibly including `<local>`) and the configuration script's address (empty when none).
pub fn from_windows(enabled: bool, server: &str, overrides: &str, auto_config_url: &str) -> Proxies {
    let pac = auto_config_url.trim();
    let mut p = Proxies { pac_url: (!pac.is_empty()).then(|| pac.to_string()), ..Default::default() };
    if !enabled {
        return p;
    }
    (p.https, p.http, p.socks) = from_windows_server(server);
    p.exceptions = overrides.split(';').map(|e| e.trim().to_string()).filter(|e| !e.is_empty()).collect();
    p
}

/// Parse macOS `scutil --proxy` output (`Key : value` lines, `Key : <array> {` blocks of `N : value`): the
/// https, http and SOCKS proxies when their `…Enable` is 1 with a host and port, `ExceptionsList`,
/// `ExcludeSimpleHostnames` (as `<local>`) and the configuration script (`ProxyAutoConfigEnable` with
/// `ProxyAutoConfigURLString`).
pub fn from_scutil(text: &str) -> Proxies {
    let mut values: Vec<(String, String)> = Vec::new();
    let mut exceptions: Vec<String> = Vec::new();
    let mut in_array: Option<String> = None;
    for line in text.lines() {
        let l = line.trim();
        if l == "}" {
            in_array = None;
            continue;
        }
        let Some((k, v)) = l.split_once(" : ") else { continue };
        let (k, v) = (k.trim(), v.trim());
        match &in_array {
            Some(name) => {
                if name == "ExceptionsList" {
                    exceptions.push(v.to_string());
                }
            }
            None if v.starts_with("<array>") => in_array = Some(k.to_string()),
            None => values.push((k.to_string(), v.to_string())),
        }
    }
    let get = |k: &str| values.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    let on = |k: &str| get(k).map(|v| v == "1").unwrap_or(false);
    let place = |enable: &str, host: &str, port: &str, scheme: &str| -> Option<Place> {
        if !on(enable) {
            return None;
        }
        let h = get(host)?.trim().to_ascii_lowercase();
        let p: u16 = get(port)?.parse().ok().filter(|p| *p > 0)?;
        (!h.is_empty()).then(|| Place { scheme: scheme.into(), host: h, port: p, user: None, pass: None })
    };
    if on("ExcludeSimpleHostnames") {
        exceptions.push("<local>".into());
    }
    Proxies {
        https: place("HTTPSEnable", "HTTPSProxy", "HTTPSPort", "http"),
        http: place("HTTPEnable", "HTTPProxy", "HTTPPort", "http"),
        socks: place("SOCKSEnable", "SOCKSProxy", "SOCKSPort", "socks5"),
        exceptions,
        pac_url: on("ProxyAutoConfigEnable").then(|| get("ProxyAutoConfigURLString").unwrap_or("").trim().to_string()).filter(|u| !u.is_empty()),
    }
}

// ───────────────────────── Linux desktops ─────────────────────────

/// One KDE proxy value: `scheme://host:port`, `scheme://host port`, `host:port` or `host port`.
fn kde_place(v: &str) -> Option<Place> {
    let v = v.trim();
    match v.rsplit_once(' ') {
        Some((h, port)) if !h.trim().is_empty() && port.trim().parse::<u16>().is_ok() => place_of(&format!("{}:{}", h.trim(), port.trim())),
        _ => place_of(v),
    }
}

/// Settings from KDE's `kioslaverc` (group `[Proxy Settings]`): `ProxyType` 0 none, 1 manual (`httpProxy`,
/// `httpsProxy`, `socksProxy`, `NoProxyFor`), 2 the configuration script (`Proxy Config Script`), 3 automatic
/// discovery (not followed: read as nothing set). The SOCKS value is a SOCKS5 proxy whatever scheme it is
/// written with (KDE writes `http://IP:PORT` there). `None` when the file names no type, type 4 (the
/// environment), or another.
pub fn from_kioslaverc(text: &str) -> Option<Proxies> {
    let mut group = String::new();
    let mut vals: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') && l.ends_with(']') {
            group = l[1..l.len() - 1].to_string();
        } else if group == "Proxy Settings" {
            if let Some((k, v)) = l.split_once('=') {
                vals.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
    }
    let get = |k: &str| vals.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    let mut p = Proxies::default();
    match get("ProxyType").and_then(|t| t.parse::<u8>().ok())? {
        0 | 3 => {}
        1 => {
            p.http = get("httpProxy").as_deref().and_then(kde_place);
            p.https = get("httpsProxy").as_deref().and_then(kde_place);
            p.socks = get("socksProxy").as_deref().and_then(kde_place).map(|mut s| {
                s.scheme = "socks5".into();
                s
            });
            p.exceptions = get("NoProxyFor").map(|v| v.split(',').map(|e| e.trim().to_string()).filter(|e| !e.is_empty()).collect()).unwrap_or_default();
        }
        2 => p.pac_url = get("Proxy Config Script").filter(|u| !u.trim().is_empty()),
        _ => return None,
    }
    Some(p)
}

/// A GSettings value as stored on disk, of the types the proxy keys use.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GValue {
    Str(String),
    Int(i64),
    Bool(bool),
    Strs(Vec<String>),
}

/// Settings from GSettings (`org.gnome.system.proxy`, also read by Cinnamon and MATE), keyed by dconf path
/// (`/system/proxy/mode`, `/system/proxy/http/host` …): mode `none`; `manual` (each kind's host and port, the
/// http proxy's credentials when `use-authentication` is on, `use-same-proxy`, `ignore-hosts`); `auto` (the
/// script's address; none means automatic discovery, not followed: read as nothing set). `None` when no mode is
/// stored (never set: the environment is read instead).
pub fn from_gsettings(get: impl Fn(&str) -> Option<GValue>) -> Option<Proxies> {
    let s = |k: &str| match get(k) {
        Some(GValue::Str(v)) => Some(v),
        _ => None,
    };
    let i = |k: &str| match get(k) {
        Some(GValue::Int(v)) => Some(v),
        _ => None,
    };
    let b = |k: &str| matches!(get(k), Some(GValue::Bool(true)));
    let mut p = Proxies::default();
    match s("/system/proxy/mode")?.as_str() {
        "manual" => {
            let place = |kind: &str, scheme: &str| -> Option<Place> {
                let host = s(&format!("/system/proxy/{kind}/host")).filter(|h| !h.trim().is_empty())?;
                let port = u16::try_from(i(&format!("/system/proxy/{kind}/port"))?).ok().filter(|p| *p > 0)?;
                Some(Place { scheme: scheme.into(), host: host.trim().to_ascii_lowercase(), port, user: None, pass: None })
            };
            p.http = place("http", "http").map(|mut h| {
                if b("/system/proxy/http/use-authentication") {
                    h.user = s("/system/proxy/http/authentication-user").filter(|u| !u.is_empty());
                    h.pass = h.user.as_ref().and_then(|_| s("/system/proxy/http/authentication-password"));
                }
                h
            });
            p.https = place("https", "http");
            if p.https.is_none() && b("/system/proxy/use-same-proxy") {
                p.https = p.http.clone();
            }
            p.socks = place("socks", "socks5");
            if let Some(GValue::Strs(v)) = get("/system/proxy/ignore-hosts") {
                p.exceptions = v;
            }
        }
        "auto" => p.pac_url = s("/system/proxy/autoconfig-url").filter(|u| !u.trim().is_empty()),
        _ => {}
    }
    Some(p)
}

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at.checked_add(4)?)?.try_into().ok()?))
}

/// One serialized GVariant of a proxy key's type (`s`, `b`, `i`, `as`), little-endian; `None` for any other
/// type or a malformed value.
fn gvariant(data: &[u8], ty: &str) -> Option<GValue> {
    let text = |s: &[u8]| String::from_utf8(s.strip_suffix(&[0])?.to_vec()).ok();
    match ty {
        "s" => text(data).map(GValue::Str),
        "b" => Some(GValue::Bool(*data.first()? != 0)),
        "i" => Some(GValue::Int(i32::from_le_bytes(data.get(..4)?.try_into().ok()?) as i64)),
        "as" => {
            let n = data.len();
            if n == 0 {
                return Some(GValue::Strs(Vec::new()));
            }
            // The width of the framing offsets follows the container's size.
            let w = if n <= 0xff { 1 } else if n <= 0xffff { 2 } else { 4 };
            let off = |at: usize| -> Option<usize> {
                let b = data.get(at..at + w)?;
                Some(b.iter().rev().fold(0usize, |acc, x| (acc << 8) | *x as usize))
            };
            let table = off(n.checked_sub(w)?)?;
            if table > n || (n - table) % w != 0 {
                return None;
            }
            let mut out = Vec::new();
            let mut start = 0;
            for k in 0..(n - table) / w {
                let end = off(table + k * w)?;
                out.push(text(data.get(start..end)?)?);
                start = end;
            }
            Some(GValue::Strs(out))
        }
        _ => None,
    }
}

/// The values of a dconf database file (the GVariant database, little-endian, in which dconf keeps a user's
/// settings): each value's full key (its parents' names before its own) and the value, for the types the proxy
/// keys use. `None` when the file is not one (a big-endian or later form included).
pub fn gsettings_from_gvdb(b: &[u8]) -> Option<Vec<(String, GValue)>> {
    if b.get(..8)? != b"GVariant" {
        return None;
    }
    let (start, end) = (u32_at(b, 16)? as usize, u32_at(b, 20)? as usize);
    if end > b.len() || start.checked_add(8)? > end {
        return None;
    }
    let n_bloom = (u32_at(b, start)? & ((1 << 27) - 1)) as usize;
    let n_buckets = u32_at(b, start + 4)? as usize;
    let items_at = start.checked_add(8)?.checked_add(n_bloom.checked_mul(4)?)?.checked_add(n_buckets.checked_mul(4)?)?;
    if items_at > end || (end - items_at) % 24 != 0 {
        return None;
    }
    let count = (end - items_at) / 24;
    let item = |k: usize| -> Option<(u32, String, u8, usize, usize)> {
        let at = items_at + 24 * k;
        let key_start = u32_at(b, at + 8)? as usize;
        let key_size = u16::from_le_bytes(b.get(at + 12..at + 14)?.try_into().ok()?) as usize;
        let key = String::from_utf8(b.get(key_start..key_start + key_size)?.to_vec()).ok()?;
        Some((u32_at(b, at + 4)?, key, *b.get(at + 14)?, u32_at(b, at + 16)? as usize, u32_at(b, at + 20)? as usize))
    };
    let mut out = Vec::new();
    for k in 0..count {
        let (mut parent, mut full, ty, vs, ve) = item(k)?;
        if ty != b'v' {
            continue;
        }
        let mut hops = 0;
        while parent != u32::MAX {
            hops += 1;
            if hops > count {
                return None;
            }
            let (pp, pk, ..) = item(parent as usize)?;
            full = format!("{pk}{full}");
            parent = pp;
        }
        // A variant: the value's bytes, a zero, its type.
        let v = b.get(vs..ve)?;
        let nul = v.iter().rposition(|c| *c == 0)?;
        let ty = std::str::from_utf8(&v[nul + 1..]).ok()?;
        if let Some(val) = gvariant(&v[..nul], ty) {
            out.push((full, val));
        }
    }
    Some(out)
}

/// One GVariant text value as the keyfile backend writes it: `'text'`, a whole number, `true`/`false`, or
/// `['a', 'b']`.
fn gvariant_text(v: &str) -> Option<GValue> {
    let v = v.trim();
    let quoted = |s: &str| -> Option<String> {
        let s = s.trim();
        let q = s.chars().next().filter(|c| *c == '\'' || *c == '"')?;
        let inner = s.strip_prefix(q)?.strip_suffix(q)?;
        Some(inner.replace(&format!("\\{q}"), &q.to_string()))
    };
    if let Some(s) = quoted(v) {
        return Some(GValue::Str(s));
    }
    if let Some(list) = v.strip_prefix("@as ").unwrap_or(v).strip_prefix('[').and_then(|r| r.strip_suffix(']')) {
        return list.split(',').map(str::trim).filter(|s| !s.is_empty()).map(quoted).collect::<Option<Vec<_>>>().map(GValue::Strs);
    }
    match v {
        "true" => Some(GValue::Bool(true)),
        "false" => Some(GValue::Bool(false)),
        _ => v.strip_prefix("int32 ").unwrap_or(v).parse().ok().map(GValue::Int),
    }
}

/// The values of the GSettings keyfile backend (`[system/proxy/http]` groups of `key=value` lines), keyed by
/// dconf path.
pub fn gsettings_from_keyfile(text: &str) -> Vec<(String, GValue)> {
    let mut group = String::new();
    let mut out = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') && l.ends_with(']') {
            group = l[1..l.len() - 1].trim_matches('/').to_string();
        } else if !l.starts_with('#') && !group.is_empty() {
            if let Some((k, v)) = l.split_once('=') {
                if let Some(val) = gvariant_text(v) {
                    out.push((format!("/{group}/{}", k.trim()), val));
                }
            }
        }
    }
    out
}

/// The Linux desktop's settings from the files under the user's configuration folder (`config`), for the
/// desktop `XDG_CURRENT_DESKTOP` names: KDE's `kioslaverc`; any other, GSettings from the dconf database
/// `dconf/user`, else the keyfile backend `glib-2.0/settings/keyfile`. `None` when nothing is set there (the
/// environment is read instead).
pub fn from_desktop_files(desktop: &str, config: &std::path::Path) -> Option<Proxies> {
    if desktop.split(':').any(|d| d.eq_ignore_ascii_case("KDE")) {
        return std::fs::read_to_string(config.join("kioslaverc")).ok().as_deref().and_then(from_kioslaverc);
    }
    let values = match std::fs::read(config.join("dconf").join("user")).ok().as_deref().and_then(gsettings_from_gvdb) {
        Some(v) => v,
        None => gsettings_from_keyfile(&std::fs::read_to_string(config.join("glib-2.0").join("settings").join("keyfile")).unwrap_or_default()),
    };
    from_gsettings(|k| values.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()))
}

/// The current system proxy settings (`None` if they could not be read). Each call reads afresh; caching and
/// timeouts are the transport's concern.
pub fn system() -> Option<Proxies> {
    imp::read()
}

/// The proxy variables a terminal sets (`https_proxy`, `http_proxy`, `all_proxy`, lower or upper case; `no_proxy`
/// alone sets no proxy).
const TERMINAL_VARS: [&str; 3] = ["https_proxy", "http_proxy", "all_proxy"];

/// The settings a command line follows, as programs in a terminal take them on every system: the terminal's
/// proxy variables when any is set ([`from_env`]); else the system's settings ([`system`]).
pub fn for_the_terminal() -> Option<Proxies> {
    terminal_or(|k| std::env::var(k).ok(), system)
}

/// [`for_the_terminal`] over a given environment and system reading.
pub fn terminal_or(var: impl Fn(&str) -> Option<String>, otherwise: impl FnOnce() -> Option<Proxies>) -> Option<Proxies> {
    let set = TERMINAL_VARS.iter().any(|k| var(k).or_else(|| var(&k.to_ascii_uppercase())).is_some_and(|v| !v.trim().is_empty()));
    if set {
        Some(from_env(var))
    } else {
        otherwise()
    }
}

/// Maximum run time for the child process that reads system settings (macOS); after it the child is killed
/// and the settings count as unread.
pub const CHILD_CAP: std::time::Duration = std::time::Duration::from_secs(5);

#[cfg(target_os = "macos")]
mod imp {
    //! macOS: proxies from the network configuration as printed by `scutil --proxy`, parsed by
    //! [`super::from_scutil`].
    use std::io::Read;
    use std::process::{Command, Stdio};

    pub fn read() -> Option<super::Proxies> {
        let mut child = crate::spawn(Command::new("/usr/sbin/scutil").arg("--proxy").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null())).ok()?;
        // Read the output while waiting, so long output cannot block the child on a full pipe.
        let mut out = child.stdout.take()?;
        let text = std::thread::spawn(move || {
            let mut b = Vec::new();
            out.read_to_end(&mut b).map(|_| b)
        });
        let until = std::time::Instant::now() + super::CHILD_CAP;
        let status = loop {
            match child.try_wait() {
                Ok(Some(st)) => break Some(st),
                Ok(None) if std::time::Instant::now() < until => std::thread::sleep(std::time::Duration::from_millis(5)),
                _ => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
            }
        };
        let bytes = text.join().ok()?.ok()?;
        status.filter(|s| s.success()).map(|_| super::from_scutil(&String::from_utf8_lossy(&bytes)))
    }
}

#[cfg(windows)]
mod imp {
    //! Windows: under the machine-wide policy the machine's registry values; otherwise the user's settings as
    //! the system answers them, the user's registry values when it answers nothing. The calls are the crate's
    //! one declaration (`windows.rs`).
    use super::Proxies;
    use crate::windows::{ie_proxy_config, reg_dword, reg_text, Handle, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};

    const PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
    const POLICY: &str = r"Software\Policies\Microsoft\Windows\CurrentVersion\Internet Settings";

    /// The registry's form under one root.
    fn registry(root: Handle) -> Proxies {
        let text = |name: &str| reg_text(root, PATH, name).unwrap_or_default();
        super::from_windows(reg_dword(root, PATH, "ProxyEnable").unwrap_or(0) != 0, &text("ProxyServer"), &text("ProxyOverride"), &text("AutoConfigURL"))
    }

    pub fn read() -> Option<Proxies> {
        if reg_dword(HKEY_LOCAL_MACHINE, POLICY, "ProxySettingsPerUser") == Some(0) {
            return Some(registry(HKEY_LOCAL_MACHINE));
        }
        Some(match ie_proxy_config() {
            Some(c) => super::from_windows(c.proxy.is_some(), c.proxy.as_deref().unwrap_or(""), c.bypass.as_deref().unwrap_or(""), c.auto_config_url.as_deref().unwrap_or("")),
            None => registry(HKEY_CURRENT_USER),
        })
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    //! Linux and other unix: the desktop's settings from its files, else the environment.
    pub fn read() -> Option<super::Proxies> {
        let config = std::env::var_os("XDG_CONFIG_HOME").filter(|v| !v.is_empty()).map(std::path::PathBuf::from).or_else(|| crate::user_home().map(|h| h.join(".config")));
        let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
        config.as_deref().and_then(|c| super::from_desktop_files(&desktop, c)).or_else(|| Some(super::from_env(|k| std::env::var(k).ok())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
    }

    fn hp(p: &Option<Place>) -> Option<(String, String, u16)> {
        p.as_ref().map(|p| (p.scheme.clone(), p.host.clone(), p.port))
    }

    fn sp(scheme: &str, host: &str, port: u16) -> Option<(String, String, u16)> {
        Some((scheme.into(), host.into(), port))
    }

    fn lookup(values: Vec<(String, GValue)>) -> impl Fn(&str) -> Option<GValue> {
        move |k| values.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone())
    }

    /// A proxy address parses with or without a scheme and with credentials (percent-decoded); a missing port, a
    /// path, a space or an empty user is rejected; the password is never printed.
    #[test]
    fn a_proxy_place_is_read_or_left() {
        assert_eq!(hp(&place_of("127.0.0.1:7890")), sp("", "127.0.0.1", 7890));
        assert_eq!(hp(&place_of("HTTP://Proxy.Example:8080/")), sp("http", "proxy.example", 8080));
        assert_eq!(hp(&place_of("socks5h://[::1]:1080")), sp("socks5h", "[::1]", 1080));
        let c = place_of("http://us%40er:p%3Ass@h:1").expect("with credentials");
        assert_eq!((c.user.as_deref(), c.pass.as_deref(), c.host.as_str()), (Some("us@er"), Some("p:ss"), "h"));
        assert!(!format!("{c:?}").contains("p:ss"));
        assert_eq!(place_of("u@h:1").and_then(|p| p.user.clone()), Some("u".into()));
        for bad in ["h", "h:", "h:0", "h:65536", "http://h:1/path", "::1:80", " ", "http://@h:1", "http://:p@h:1"] {
            assert!(place_of(bad).is_none(), "{bad}");
        }
    }

    /// Environment: lowercase before uppercase, SOCKS schemes to SOCKS, `all_proxy` fills the rest, credentials
    /// and an `https://` proxy kept, `no_proxy` split on commas.
    #[test]
    fn the_environment_is_read() {
        let p = from_env(env(&[("https_proxy", "http://a:1"), ("HTTPS_PROXY", "http://b:2"), ("HTTP_PROXY", "c:3"), ("no_proxy", "localhost, .corp.example,,")]));
        assert_eq!((hp(&p.https), hp(&p.http), hp(&p.socks)), (sp("http", "a", 1), sp("", "c", 3), None));
        assert_eq!(p.exceptions, vec!["localhost".to_string(), ".corp.example".to_string()]);
        let p = from_env(env(&[("all_proxy", "socks5://s:1080")]));
        assert_eq!((hp(&p.https), hp(&p.socks)), (None, sp("socks5", "s", 1080)));
        let p = from_env(env(&[("ALL_PROXY", "http://all:8"), ("http_proxy", "http://h:9")]));
        assert_eq!((hp(&p.https), hp(&p.http)), (sp("http", "all", 8), sp("http", "h", 9)));
        let p = from_env(env(&[("http_proxy", "http://u:pw@h:2"), ("https_proxy", "https://tls.example:443")]));
        assert_eq!(p.http.as_ref().and_then(|h| h.pass.clone()), Some("pw".into()));
        assert_eq!(hp(&p.https), sp("https", "tls.example", 443));
        assert_eq!(from_env(env(&[])), Proxies::default());
    }

    /// The command line's reading: any of the terminal's three proxy variables, in either case, set and not blank,
    /// is followed as the environment says; `no_proxy` alone, or blank values, leave the system's settings.
    #[test]
    fn the_terminal_comes_before_the_system_for_the_command_line() {
        let system = || Some(Proxies { pac_url: Some("http://127.0.0.1:1/pac".into()), ..Default::default() });
        let env = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        for vars in [&[("https_proxy", "http://127.0.0.1:7890")][..], &[("HTTP_PROXY", "http://127.0.0.1:7890")], &[("all_proxy", "socks5://127.0.0.1:7891")]] {
            let p = terminal_or(env(vars), system).expect("read");
            assert_eq!(p.pac_url, None, "{vars:?}: the terminal's, not the system's");
            assert!(p.https.is_some() || p.http.is_some() || p.socks.is_some(), "{vars:?}");
        }
        for vars in [&[][..], &[("no_proxy", "localhost")], &[("https_proxy", "  ")]] {
            assert_eq!(terminal_or(env(vars), system).and_then(|p| p.pac_url), Some("http://127.0.0.1:1/pac".into()), "{vars:?}: the system's");
        }
    }

    /// Windows: the list's forms (one place with or without `http://`, per-kind entries, `socks=` alone as
    /// SOCKS5), the bypass list and the script address; disabled keeps only the script address.
    #[test]
    fn the_windows_forms_are_read() {
        let p = from_windows(true, "127.0.0.1:7890", "*.local;<local>;10.*", "");
        assert_eq!((hp(&p.https), hp(&p.http), hp(&p.socks)), (sp("", "127.0.0.1", 7890), sp("", "127.0.0.1", 7890), None));
        assert_eq!(p.exceptions, vec!["*.local".to_string(), "<local>".to_string(), "10.*".to_string()]);
        assert_eq!(hp(&from_windows(true, "http://127.0.0.1:2080", "", "").https), sp("http", "127.0.0.1", 2080));
        let p = from_windows(true, "http=h:1;https=s:2;socks=k:3;ftp=f:4", "", "");
        assert_eq!((hp(&p.https), hp(&p.http), hp(&p.socks)), (sp("", "s", 2), sp("", "h", 1), sp("socks5", "k", 3)));
        let p = from_windows(true, "socks=127.0.0.1:10808", "", "");
        assert_eq!((hp(&p.https), hp(&p.http), hp(&p.socks)), (None, None, sp("socks5", "127.0.0.1", 10808)));
        assert_eq!(from_windows(false, "h:1", "", ""), Proxies::default());
        let p = from_windows(false, "", "", " http://127.0.0.1:10811/pac ");
        assert_eq!(p.pac_url.as_deref(), Some("http://127.0.0.1:10811/pac"));
    }

    /// macOS: explicit proxies (one switched off), exceptions, simple host names excluded, the script address
    /// (automatic discovery beside it not read); discovery alone reads as nothing set; nothing set.
    #[test]
    fn the_macos_configuration_is_read() {
        let on = "<dictionary> {\n  ExceptionsList : <array> {\n    0 : *.local\n    1 : 169.254/16\n  }\n  ExcludeSimpleHostnames : 1\n  HTTPEnable : 1\n  HTTPPort : 7890\n  HTTPProxy : 127.0.0.1\n  HTTPSEnable : 1\n  HTTPSPort : 7890\n  HTTPSProxy : 127.0.0.1\n  SOCKSEnable : 0\n  SOCKSPort : 7891\n  SOCKSProxy : 127.0.0.1\n}\n";
        let p = from_scutil(on);
        assert_eq!((hp(&p.https), hp(&p.http), hp(&p.socks)), (sp("http", "127.0.0.1", 7890), sp("http", "127.0.0.1", 7890), None));
        assert_eq!(p.exceptions, vec!["*.local".to_string(), "169.254/16".to_string(), "<local>".to_string()]);
        assert_eq!(p.pac_url.as_deref(), None);
        let pac = from_scutil("<dictionary> {\n  ProxyAutoConfigEnable : 1\n  ProxyAutoConfigURLString : http://localhost:1089/proxy.pac\n  ProxyAutoDiscoveryEnable : 1\n}\n");
        assert_eq!((pac.pac_url.as_deref(), pac.https.is_none()), (Some("http://localhost:1089/proxy.pac"), true));
        assert_eq!(from_scutil("<dictionary> {\n  ProxyAutoDiscoveryEnable : 1\n}\n"), Proxies::default());
        let off = from_scutil("<dictionary> {\n  ProxyAutoConfigEnable : 0\n  ProxyAutoConfigURLString : http://localhost:1089/proxy.pac\n}\n");
        assert_eq!(off.pac_url, None);
        let nothing = from_scutil("<dictionary> {\n  ExceptionsList : <array> {\n    0 : *.local\n  }\n  FTPPassive : 1\n}\n");
        assert_eq!((nothing.https, nothing.socks, nothing.pac_url), (None, None, None));
    }

    /// KDE: manual with each value form (the SOCKS value is SOCKS5 whatever its scheme), the script, discovery
    /// (read as nothing set), none; the environment type and a file without a type are left to the environment.
    #[test]
    fn kde_settings_are_read() {
        let p = from_kioslaverc("[General]\nx=1\n[Proxy Settings]\nProxyType=1\nhttpProxy=http://127.0.0.1 7890\nhttpsProxy=http://127.0.0.1:7890\nsocksProxy=http://127.0.0.1:10808\nNoProxyFor=localhost,127.0.0.1\n").expect("manual");
        assert_eq!((hp(&p.http), hp(&p.https), hp(&p.socks)), (sp("http", "127.0.0.1", 7890), sp("http", "127.0.0.1", 7890), sp("socks5", "127.0.0.1", 10808)));
        assert_eq!(p.exceptions, vec!["localhost".to_string(), "127.0.0.1".to_string()]);
        assert_eq!(from_kioslaverc("[Proxy Settings]\nProxyType=1\nsocksProxy=socks://127.0.0.1 1089\n").and_then(|q| hp(&q.socks)), sp("socks5", "127.0.0.1", 1089));
        assert_eq!(from_kioslaverc("[Proxy Settings]\nProxyType=2\nProxy Config Script=http://127.0.0.1:1/pac\n").and_then(|p| p.pac_url.clone()), Some("http://127.0.0.1:1/pac".into()));
        assert_eq!(from_kioslaverc("[Proxy Settings]\nProxyType=3\n"), Some(Proxies::default()));
        assert_eq!(from_kioslaverc("[Proxy Settings]\nProxyType=0\nhttpProxy=http://h:1\n"), Some(Proxies::default()));
        assert_eq!(from_kioslaverc("[Proxy Settings]\nProxyType=4\n"), None);
        assert_eq!(from_kioslaverc("[Other]\nProxyType=1\n"), None);
    }

    /// A dconf database in the shape dconf writes: each path segment an item naming its parent, values as
    /// variants (no hash buckets: this reader walks the items).
    fn gvdb(pairs: &[(&str, GValue)]) -> Vec<u8> {
        let ser = |v: &GValue| -> (Vec<u8>, &'static str) {
            match v {
                GValue::Str(s) => ([s.as_bytes(), &[0]].concat(), "s"),
                GValue::Int(i) => ((*i as i32).to_le_bytes().to_vec(), "i"),
                GValue::Bool(b) => (vec![*b as u8], "b"),
                GValue::Strs(v) => {
                    let mut d = Vec::new();
                    let mut ends = Vec::new();
                    for s in v {
                        d.extend_from_slice(s.as_bytes());
                        d.push(0);
                        ends.push(d.len() as u8);
                    }
                    assert!(d.len() + ends.len() <= 0xff);
                    d.extend(ends);
                    (d, "as")
                }
            }
        };
        let mut items: Vec<(u32, String, u8, Vec<u8>)> = Vec::new();
        let mut dirs: Vec<(String, u32)> = Vec::new();
        for (k, v) in pairs {
            let (mut parent, mut acc) = (u32::MAX, String::new());
            let parts: Vec<&str> = k.split_inclusive('/').collect();
            for (i, part) in parts.iter().enumerate() {
                acc.push_str(part);
                if i + 1 == parts.len() {
                    let (data, ty) = ser(v);
                    items.push((parent, part.to_string(), b'v', [data, vec![0], ty.as_bytes().to_vec()].concat()));
                } else if let Some((_, at)) = dirs.iter().find(|(d, _)| *d == acc) {
                    parent = *at;
                } else {
                    items.push((parent, part.to_string(), b'L', Vec::new()));
                    parent = (items.len() - 1) as u32;
                    dirs.push((acc.clone(), parent));
                }
            }
        }
        let root = 24usize;
        let items_at = root + 8 + 4;
        let root_end = items_at + 24 * items.len();
        let mut out = vec![0u8; root_end];
        out[..8].copy_from_slice(b"GVariant");
        out[16..20].copy_from_slice(&(root as u32).to_le_bytes());
        out[20..24].copy_from_slice(&(root_end as u32).to_le_bytes());
        out[root + 4..root + 8].copy_from_slice(&1u32.to_le_bytes());
        for (k, (parent, key, ty, value)) in items.iter().enumerate() {
            let key_at = out.len();
            out.extend_from_slice(key.as_bytes());
            let (vs, ve) = (out.len(), out.len() + value.len());
            out.extend_from_slice(value);
            let at = items_at + 24 * k;
            out[at + 4..at + 8].copy_from_slice(&parent.to_le_bytes());
            out[at + 8..at + 12].copy_from_slice(&(key_at as u32).to_le_bytes());
            out[at + 12..at + 14].copy_from_slice(&(key.len() as u16).to_le_bytes());
            out[at + 14] = *ty;
            out[at + 16..at + 20].copy_from_slice(&(vs as u32).to_le_bytes());
            out[at + 20..at + 24].copy_from_slice(&(ve as u32).to_le_bytes());
        }
        out
    }

    /// GNOME: the dconf database read back key by key; manual with credentials and the same proxy for https;
    /// the script; discovery (`auto` without a script address, read as nothing set); nothing stored; the keyfile
    /// backend.
    #[test]
    fn gnome_settings_are_read_from_the_dconf_database_and_the_keyfile() {
        let db = gvdb(&[
            ("/system/proxy/mode", GValue::Str("manual".into())),
            ("/system/proxy/use-same-proxy", GValue::Bool(true)),
            ("/system/proxy/http/host", GValue::Str("127.0.0.1".into())),
            ("/system/proxy/http/port", GValue::Int(7890)),
            ("/system/proxy/http/use-authentication", GValue::Bool(true)),
            ("/system/proxy/http/authentication-user", GValue::Str("u".into())),
            ("/system/proxy/http/authentication-password", GValue::Str("pw".into())),
            ("/system/proxy/socks/host", GValue::Str("127.0.0.1".into())),
            ("/system/proxy/socks/port", GValue::Int(7891)),
            ("/system/proxy/ignore-hosts", GValue::Strs(vec!["localhost".into(), "10.0.0.0/8".into()])),
        ]);
        let p = from_gsettings(lookup(gsettings_from_gvdb(&db).expect("a database"))).expect("set");
        assert_eq!((hp(&p.http), hp(&p.https), hp(&p.socks)), (sp("http", "127.0.0.1", 7890), sp("http", "127.0.0.1", 7890), sp("socks5", "127.0.0.1", 7891)));
        assert_eq!((p.http.as_ref().and_then(|h| h.user.clone()), p.http.as_ref().and_then(|h| h.pass.clone())), (Some("u".into()), Some("pw".into())));
        assert_eq!(p.exceptions, vec!["localhost".to_string(), "10.0.0.0/8".to_string()]);
        let auto = gvdb(&[("/system/proxy/mode", GValue::Str("auto".into())), ("/system/proxy/autoconfig-url", GValue::Str("http://127.0.0.1:1/pac".into()))]);
        assert_eq!(from_gsettings(lookup(gsettings_from_gvdb(&auto).expect("db"))).and_then(|p| p.pac_url.clone()), Some("http://127.0.0.1:1/pac".into()));
        let wpad = gvdb(&[("/system/proxy/mode", GValue::Str("auto".into()))]);
        assert_eq!(from_gsettings(lookup(gsettings_from_gvdb(&wpad).expect("db"))), Some(Proxies::default()));
        let unset = gvdb(&[("/org/other/key", GValue::Str("x".into()))]);
        assert_eq!(from_gsettings(lookup(gsettings_from_gvdb(&unset).expect("db"))), None);
        assert!(gsettings_from_gvdb(b"not a database").is_none());
        let mut cut = db.clone();
        cut.truncate(40);
        assert!(gsettings_from_gvdb(&cut).is_none());
        let kf = gsettings_from_keyfile("[system/proxy]\nmode='manual'\nignore-hosts=['localhost', '*.local']\n\n[system/proxy/https]\nhost='10.0.0.1'\nport=3128\n");
        let p = from_gsettings(lookup(kf)).expect("keyfile");
        assert_eq!((hp(&p.https), p.exceptions.len()), (sp("http", "10.0.0.1", 3128), 2));
    }

    /// The files under a configuration folder: KDE's for KDE, GSettings otherwise, none when nothing is there.
    #[test]
    fn the_desktop_files_are_read_by_desktop() {
        let d = std::env::temp_dir().join(format!("zk-os-desktop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("glib-2.0/settings")).expect("dir");
        std::fs::write(d.join("kioslaverc"), "[Proxy Settings]\nProxyType=1\nhttpProxy=http://k:1\n").expect("kde");
        std::fs::write(d.join("glib-2.0/settings/keyfile"), "[system/proxy]\nmode='auto'\nautoconfig-url='http://g/pac'\n").expect("gnome");
        assert_eq!(from_desktop_files("KDE", &d).and_then(|p| hp(&p.http)), sp("http", "k", 1));
        assert_eq!(from_desktop_files("ubuntu:GNOME", &d).and_then(|p| p.pac_url.clone()), Some("http://g/pac".into()));
        let _ = std::fs::remove_dir_all(&d);
        assert_eq!(from_desktop_files("GNOME", &d), None);
    }

    /// Reading the real system settings completes on this system (whatever they are).
    #[test]
    fn the_system_is_read_without_writing() {
        let started = std::time::Instant::now();
        let _ = system();
        assert!(started.elapsed() < CHILD_CAP + std::time::Duration::from_secs(1));
    }
}
