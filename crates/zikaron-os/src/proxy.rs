//! System proxy settings, read only: macOS from its network configuration (as printed by `scutil --proxy`),
//! Windows from the user's Internet settings in the registry, Linux and other unix from the environment
//! (`https_proxy`, `http_proxy`, `all_proxy`, `no_proxy`, lower or upper case). The parsers are plain functions,
//! tested on every OS.

/// System proxy settings: the https and http proxies (both tunnelled with CONNECT), a SOCKS proxy, the
/// exception list, and whether automatic configuration (PAC script or discovery) is set (unsupported: treated
/// as no proxy, and the settings page says so).
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct Proxies {
    pub https: Option<(String, u16)>,
    pub http: Option<(String, u16)>,
    pub socks: Option<(String, u16)>,
    pub exceptions: Vec<String>,
    pub auto_config: bool,
}

/// Parse a proxy written as `host:port` or `scheme://host:port[/]`: scheme (lowercase, empty when none), host
/// and port. Credentials (`user@`, unsupported), a missing port or a path are rejected.
pub fn place_of(text: &str) -> Option<(String, String, u16)> {
    let t = text.trim();
    let (scheme, rest) = match t.split_once("://") {
        Some((s, r)) => (s.to_ascii_lowercase(), r),
        None => (String::new(), t),
    };
    let rest = rest.strip_suffix('/').unwrap_or(rest);
    if rest.is_empty() || rest.contains('@') || rest.contains('/') || rest.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let (host, port) = rest.rsplit_once(':')?;
    let port: u16 = port.parse().ok().filter(|p| *p > 0)?;
    if host.is_empty() || (host.contains(':') && !(host.starts_with('[') && host.ends_with(']'))) {
        return None;
    }
    Some((scheme, host.to_ascii_lowercase(), port))
}

fn is_socks(scheme: &str) -> bool {
    matches!(scheme, "socks" | "socks5" | "socks5h")
}

/// Settings from the environment (Linux and other unix): `https_proxy`, `http_proxy`, `all_proxy` and
/// `no_proxy`, lowercase taking precedence over uppercase. A SOCKS scheme sets the SOCKS proxy; any other the
/// CONNECT proxy for its kind; `all_proxy` fills whatever the other two left. `no_proxy` is a comma-separated
/// exception list.
pub fn from_env(var: impl Fn(&str) -> Option<String>) -> Proxies {
    let get = |name: &str| var(name).or_else(|| var(&name.to_ascii_uppercase())).filter(|v| !v.trim().is_empty());
    let mut p = Proxies::default();
    let put = |text: Option<String>, connect: &mut Option<(String, u16)>, socks: &mut Option<(String, u16)>| {
        if let Some((scheme, host, port)) = text.as_deref().and_then(place_of) {
            if is_socks(&scheme) {
                socks.get_or_insert((host, port));
            } else if scheme.is_empty() || scheme == "http" {
                connect.get_or_insert((host, port));
            }
        }
    };
    let (mut https, mut http, mut socks) = (None, None, None);
    put(get("https_proxy"), &mut https, &mut socks);
    put(get("http_proxy"), &mut http, &mut socks);
    if let Some((scheme, host, port)) = get("all_proxy").as_deref().and_then(place_of) {
        if is_socks(&scheme) {
            socks.get_or_insert((host, port));
        } else if scheme.is_empty() || scheme == "http" {
            https.get_or_insert((host.clone(), port));
            http.get_or_insert((host, port));
        }
    }
    p.https = https;
    p.http = http;
    p.socks = socks;
    p.exceptions = get("no_proxy").map(|v| v.split(',').map(|e| e.trim().to_string()).filter(|e| !e.is_empty()).collect()).unwrap_or_default();
    p
}

/// Settings from the Windows user Internet settings: `ProxyEnable`, `ProxyServer` (one `host:port` for every
/// kind, or `kind=host:port` entries separated by `;`), `ProxyOverride` (`;`-separated, possibly including
/// `<local>`) and whether `AutoConfigURL` is set.
pub fn from_windows(enabled: bool, server: &str, overrides: &str, auto_config_url: bool) -> Proxies {
    let mut p = Proxies { auto_config: auto_config_url, ..Default::default() };
    if !enabled {
        return p;
    }
    if server.contains('=') {
        for entry in server.split(';') {
            let Some((kind, place)) = entry.split_once('=') else { continue };
            let Some((scheme, host, port)) = place_of(place) else { continue };
            match kind.trim().to_ascii_lowercase().as_str() {
                "https" if !is_socks(&scheme) => p.https = Some((host, port)),
                "http" if !is_socks(&scheme) => p.http = Some((host, port)),
                "socks" => p.socks = Some((host, port)),
                _ => {}
            }
        }
    } else if let Some((scheme, host, port)) = place_of(server) {
        if is_socks(&scheme) {
            p.socks = Some((host, port));
        } else {
            p.https = Some((host.clone(), port));
            p.http = Some((host, port));
        }
    }
    p.exceptions = overrides.split(';').map(|e| e.trim().to_string()).filter(|e| !e.is_empty()).collect();
    p
}

/// Parse macOS `scutil --proxy` output (`Key : value` lines, `Key : <array> {` blocks of `N : value`): the
/// https, http and SOCKS proxies when their `…Enable` is 1 with a host and port, `ExceptionsList`,
/// `ExcludeSimpleHostnames` (as `<local>`), and automatic configuration (script or discovery).
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
    let place = |enable: &str, host: &str, port: &str| -> Option<(String, u16)> {
        if !on(enable) {
            return None;
        }
        let h = get(host)?.trim().to_ascii_lowercase();
        let p: u16 = get(port)?.parse().ok().filter(|p| *p > 0)?;
        (!h.is_empty()).then_some((h, p))
    };
    if on("ExcludeSimpleHostnames") {
        exceptions.push("<local>".into());
    }
    Proxies {
        https: place("HTTPSEnable", "HTTPSProxy", "HTTPSPort"),
        http: place("HTTPEnable", "HTTPProxy", "HTTPPort"),
        socks: place("SOCKSEnable", "SOCKSProxy", "SOCKSPort"),
        exceptions,
        auto_config: on("ProxyAutoConfigEnable") || on("ProxyAutoDiscoveryEnable"),
    }
}

/// The current system proxy settings (`None` if they could not be read). Each call reads afresh; caching and
/// timeouts are the transport's concern.
pub fn system() -> Option<Proxies> {
    imp::read()
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
    //! Windows: the user's Internet settings in the registry (read only).
    use super::Proxies;

    const HKEY_CURRENT_USER: isize = 0x8000_0001u32 as i32 as isize;
    const KEY_READ: u32 = 0x20019;
    const PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegOpenKeyExW(key: isize, sub: *const u16, options: u32, access: u32, out: *mut isize) -> i32;
        fn RegQueryValueExW(key: isize, name: *const u16, reserved: *mut u32, kind: *mut u32, data: *mut u8, len: *mut u32) -> i32;
        fn RegCloseKey(key: isize) -> i32;
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// One value's bytes (`None` when absent).
    fn raw(key: isize, name: &str) -> Option<Vec<u8>> {
        let n = wide(name);
        let mut len: u32 = 0;
        // SAFETY: the first call asks the size; the second reads into a buffer of that size.
        unsafe {
            if RegQueryValueExW(key, n.as_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), &mut len) != 0 {
                return None;
            }
            let mut buf = vec![0u8; len as usize];
            if RegQueryValueExW(key, n.as_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), buf.as_mut_ptr(), &mut len) != 0 {
                return None;
            }
            buf.truncate(len as usize);
            Some(buf)
        }
    }

    fn text(key: isize, name: &str) -> String {
        let b = raw(key, name).unwrap_or_default();
        let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
        String::from_utf16_lossy(&units)
    }

    pub fn read() -> Option<Proxies> {
        let mut key: isize = 0;
        let p = wide(PATH);
        // SAFETY: opened for reading only, closed below.
        if unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, p.as_ptr(), 0, KEY_READ, &mut key) } != 0 {
            return None;
        }
        let enabled = raw(key, "ProxyEnable").map(|b| b.first().copied().unwrap_or(0) != 0).unwrap_or(false);
        let out = super::from_windows(enabled, &text(key, "ProxyServer"), &text(key, "ProxyOverride"), !text(key, "AutoConfigURL").trim().is_empty());
        // SAFETY: the key opened above is closed once.
        unsafe { RegCloseKey(key) };
        Some(out)
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod imp {
    //! Linux and other unix: the environment.
    pub fn read() -> Option<super::Proxies> {
        Some(super::from_env(|k| std::env::var(k).ok()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |k| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
    }

    /// A proxy address parses with or without a scheme; credentials, a missing port or a path are rejected.
    #[test]
    fn a_proxy_place_is_read_or_left() {
        assert_eq!(place_of("127.0.0.1:7890"), Some((String::new(), "127.0.0.1".into(), 7890)));
        assert_eq!(place_of("HTTP://Proxy.Example:8080/"), Some(("http".into(), "proxy.example".into(), 8080)));
        assert_eq!(place_of("socks5h://[::1]:1080"), Some(("socks5h".into(), "[::1]".into(), 1080)));
        for bad in ["http://u:p@h:1", "h", "h:", "h:0", "h:65536", "http://h:1/path", "::1:80", " "] {
            assert_eq!(place_of(bad), None, "{bad}");
        }
    }

    /// Environment: lowercase before uppercase, SOCKS schemes to SOCKS, `all_proxy` fills the rest, `no_proxy`
    /// split on commas; an entry with credentials is skipped.
    #[test]
    fn the_environment_is_read() {
        let p = from_env(env(&[("https_proxy", "http://a:1"), ("HTTPS_PROXY", "http://b:2"), ("HTTP_PROXY", "c:3"), ("no_proxy", "localhost, .corp.example,,")]));
        assert_eq!((p.https, p.http, p.socks), (Some(("a".into(), 1)), Some(("c".into(), 3)), None));
        assert_eq!(p.exceptions, vec!["localhost".to_string(), ".corp.example".to_string()]);
        let p = from_env(env(&[("all_proxy", "socks5://s:1080")]));
        assert_eq!((p.https, p.http, p.socks), (None, None, Some(("s".into(), 1080))));
        let p = from_env(env(&[("ALL_PROXY", "http://all:8"), ("http_proxy", "http://h:9")]));
        assert_eq!((p.https, p.http), (Some(("all".into(), 8)), Some(("h".into(), 9))));
        let p = from_env(env(&[("https_proxy", "socks5://s:1"), ("http_proxy", "http://u:p@h:2")]));
        assert_eq!((p.https, p.http, p.socks), (None, None, Some(("s".into(), 1))));
        assert_eq!(from_env(env(&[])), Proxies::default());
    }

    /// Windows: one proxy for every kind, or per-kind entries; the override list with `<local>`; an automatic
    /// configuration URL; disabled means none.
    #[test]
    fn the_windows_settings_are_read() {
        let p = from_windows(true, "127.0.0.1:7890", "*.local;<local>;10.*", false);
        assert_eq!((p.https.clone(), p.http.clone(), p.socks.clone()), (Some(("127.0.0.1".into(), 7890)), Some(("127.0.0.1".into(), 7890)), None));
        assert_eq!(p.exceptions, vec!["*.local".to_string(), "<local>".to_string(), "10.*".to_string()]);
        let p = from_windows(true, "http=h:1;https=s:2;socks=k:3;ftp=f:4", "", false);
        assert_eq!((p.https, p.http, p.socks), (Some(("s".into(), 2)), Some(("h".into(), 1)), Some(("k".into(), 3))));
        assert_eq!(from_windows(false, "h:1", "", false), Proxies::default());
        assert!(from_windows(false, "", "", true).auto_config);
    }

    /// macOS: a system with a proxy switched on, one switched off, exceptions, simple host names excluded,
    /// an automatic configuration script; a system with nothing set.
    #[test]
    fn the_macos_configuration_is_read() {
        let on = "<dictionary> {\n  ExceptionsList : <array> {\n    0 : *.local\n    1 : 169.254/16\n  }\n  ExcludeSimpleHostnames : 1\n  HTTPEnable : 1\n  HTTPPort : 7890\n  HTTPProxy : 127.0.0.1\n  HTTPSEnable : 1\n  HTTPSPort : 7890\n  HTTPSProxy : 127.0.0.1\n  SOCKSEnable : 0\n  SOCKSPort : 7891\n  SOCKSProxy : 127.0.0.1\n}\n";
        let p = from_scutil(on);
        assert_eq!((p.https.clone(), p.http.clone(), p.socks.clone()), (Some(("127.0.0.1".into(), 7890)), Some(("127.0.0.1".into(), 7890)), None));
        assert_eq!(p.exceptions, vec!["*.local".to_string(), "169.254/16".to_string(), "<local>".to_string()]);
        assert!(!p.auto_config);
        let pac = from_scutil("<dictionary> {\n  ProxyAutoConfigEnable : 1\n  ProxyAutoConfigURLString : http://wpad/x.pac\n}\n");
        assert!(pac.auto_config && pac.https.is_none());
        let nothing = from_scutil("<dictionary> {\n  ExceptionsList : <array> {\n    0 : *.local\n  }\n  FTPPassive : 1\n}\n");
        assert_eq!((nothing.https, nothing.http, nothing.socks, nothing.auto_config), (None, None, None, false));
    }

    /// Reading the real system settings completes on this OS (whatever they are).
    #[test]
    fn the_system_is_read_without_writing() {
        // This machine's settings, whatever they are, read within the child's time cap.
        let started = std::time::Instant::now();
        let _ = system();
        assert!(started.elapsed() < CHILD_CAP + std::time::Duration::from_secs(1));
    }
}
