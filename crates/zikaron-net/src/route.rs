//! Proxy routing for new connections: direct to the node, or through a proxy (HTTP CONNECT, HTTPS CONNECT,
//! SOCKS5, SOCKS4), and the tunnel itself.
//!
//! The route is chosen per new connection from one of three settings: follow the system (platform proxy
//! settings, read when the connection opens, so a system change applies to the next connection without a
//! restart), none, or one manually entered proxy. Loopback hosts always go direct; under a manual proxy,
//! private, link-local and `.local` hosts do too. Following the system, a configuration script at the address it
//! names comes first ([`crate::pac`]), then the explicit proxies; a script names a list of ways, tried in order
//! until one opens. Automatic discovery (WPAD) is not followed.
//!
//! Through a proxy, the proxy resolves the host name (CONNECT names it, SOCKS5 sends the domain form, SOCKS4
//! the 4a domain tail); the leg to the proxy is dialed like any connection (every address, one deadline); to an
//! HTTPS proxy it is TLS, verified against the proxy's name; TLS stays end to end with the node, verified
//! against the node's name. A proxy that asks for credentials gets the ones written with it (HTTP Basic,
//! SOCKS5 user name and password, the SOCKS4 user id); the password is wiped once written. A proxy that asks for
//! credentials when none are written, or refuses the ones written, is its own failure
//! ([`crate::Fail::ProxyCredentials`]); any other unreachable or refusing proxy is a connection failure naming the
//! proxy and its answer.

use crate::{dial, keep, Clock, Fail, Scheme, Sock, Target};
use std::io::{Read, Write};
use std::net::IpAddr;
use std::sync::{Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

/// The proxy kinds. Closed set.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Kind {
    /// HTTP, tunnelled with CONNECT.
    Http,
    /// HTTPS: TLS to the proxy (its certificate verified), then CONNECT inside it.
    Https,
    /// SOCKS version 5.
    Socks5,
    /// SOCKS version 4; a host name is sent in the 4a form, for the proxy to resolve.
    Socks4,
}

impl Kind {
    fn scheme(self) -> &'static str {
        match self {
            Kind::Http => "http",
            Kind::Https => "https",
            Kind::Socks5 => "socks5",
            Kind::Socks4 => "socks4",
        }
    }
}

/// A proxy password: never printed, wiped when dropped.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: String) -> Secret {
        Secret(s)
    }

    /// The password, to be written to the proxy that asks for it.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("(a password)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        zikaron_os::wipe_string(&mut self.0);
    }
}

/// Credentials for a proxy: a user name and, except for SOCKS4, a password.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Auth {
    pub user: String,
    pub pass: Option<Secret>,
}

/// One proxy: its kind, host, port and the credentials written with it.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Proxy {
    pub kind: Kind,
    pub host: String,
    pub port: u16,
    pub auth: Option<Auth>,
}

impl Proxy {
    /// The proxy's canonical spelling, without credentials: `http://host:port`, `https://…`, `socks5://…` or
    /// `socks4://…`.
    pub fn spelled(&self) -> String {
        format!("{}://{}:{}", self.kind.scheme(), self.host, self.port)
    }

    /// The spelling with the user name when there is one (`socks5://user@host:port`), never the password.
    pub fn shown(&self) -> String {
        match &self.auth {
            Some(a) => format!("{}://{}@{}:{}", self.kind.scheme(), a.user, self.host, self.port),
            None => self.spelled(),
        }
    }

    /// The spelling that keeps the credentials (percent-encoded), for the sealed settings that store it; never
    /// shown.
    pub fn written(&self) -> String {
        let enc = |s: &str| -> String {
            s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
        };
        match &self.auth {
            Some(a) => match &a.pass {
                Some(p) => format!("{}://{}:{}@{}:{}", self.kind.scheme(), enc(&a.user), enc(p.expose()), self.host, self.port),
                None => format!("{}://{}@{}:{}", self.kind.scheme(), enc(&a.user), self.host, self.port),
            },
            None => self.spelled(),
        }
    }

    /// Whether a password is written with it.
    pub fn has_password(&self) -> bool {
        self.auth.as_ref().is_some_and(|a| a.pass.is_some())
    }

    fn target(&self) -> Target {
        Target { scheme: Scheme::Http, host: self.host.clone(), port: self.port, path: "/".into() }
    }
}

/// Why a manually entered proxy address is rejected. Closed set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotAProxy {
    /// Not `scheme://host:port` with a proxy scheme (another scheme, no port, a path, an empty host).
    Shape,
    /// Credentials that do not read: an empty user name, one or a password longer than 255 bytes, or a
    /// password for SOCKS4 (which carries a user id only).
    Credentials,
}

/// `%XX` decoded; a malformed escape stays as written.
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

/// Credentials as written before `@` (`user` or `user:pass`, percent-encoded), for a proxy of `kind`.
fn auth_of(written: &str, kind: Kind) -> Result<Auth, NotAProxy> {
    let (user, pass) = match written.split_once(':') {
        Some((u, p)) => (unpercent(u), Some(unpercent(p))),
        None => (unpercent(written), None),
    };
    if user.is_empty() || user.len() > 255 || pass.as_ref().is_some_and(|p| p.len() > 255) || (kind == Kind::Socks4 && pass.is_some()) {
        return Err(NotAProxy::Credentials);
    }
    Ok(Auth { user, pass: pass.map(Secret) })
}

/// Parse a manually entered proxy address: `scheme://[user[:pass]@]host:port[/]` with the scheme `http`,
/// `https`, `socks5` (or `socks5h`), `socks4` (or `socks4a`); scheme and host case-insensitive, port required;
/// a bracketed IPv6 literal host is accepted; the credentials percent-decoded. Anything else is refused by name.
pub fn proxy_of(typed: &str) -> Result<Proxy, NotAProxy> {
    let (scheme, rest) = typed.trim().split_once("://").ok_or(NotAProxy::Shape)?;
    let kind = match scheme.to_ascii_lowercase().as_str() {
        "http" => Kind::Http,
        "https" => Kind::Https,
        "socks5" | "socks5h" => Kind::Socks5,
        "socks4" | "socks4a" => Kind::Socks4,
        _ => return Err(NotAProxy::Shape),
    };
    let authority = rest.strip_suffix('/').unwrap_or(rest);
    let (creds, authority) = match authority.rsplit_once('@') {
        Some((c, a)) => (Some(c), a),
        None => (None, authority),
    };
    if authority.is_empty() || authority.contains('/') || authority.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(NotAProxy::Shape);
    }
    // The address parser handles host and port; the port must be explicit.
    let t = crate::parse(&format!("http://{authority}/")).ok_or(NotAProxy::Shape)?;
    let (_, port) = authority.rsplit_once(':').ok_or(NotAProxy::Shape)?;
    if port.is_empty() || port.ends_with(']') || t.port == 0 {
        return Err(NotAProxy::Shape);
    }
    if creds.is_some_and(|c| c.chars().any(|c| c.is_control() || c.is_whitespace())) {
        return Err(NotAProxy::Credentials);
    }
    let auth = creds.map(|c| auth_of(c, kind)).transpose()?;
    Ok(Proxy { kind, host: t.host, port: t.port, auth })
}

/// A typed proxy address as it may be shown: its password left out (`http://user@host:port`), whether or not
/// the rest reads; and whether a password was written. Any text before the last `@` after `scheme://` is
/// credentials.
pub fn proxy_text_shown(typed: &str) -> (String, bool) {
    let t = typed.trim();
    let (head, rest) = match t.split_once("://") {
        Some((s, r)) => (format!("{s}://"), r),
        None => (String::new(), t),
    };
    match rest.rsplit_once('@') {
        Some((creds, after)) => match creds.split_once(':') {
            Some((user, _)) => (format!("{head}{user}@{after}"), true),
            None => (t.to_string(), false),
        },
        None => (t.to_string(), false),
    }
}

/// The system proxy settings in the transport's form: the https proxy, the http proxy, a SOCKS proxy, the
/// exception list and the configuration script's address.
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct SystemProxies {
    pub https: Option<Proxy>,
    pub http: Option<Proxy>,
    pub socks: Option<Proxy>,
    pub exceptions: Vec<String>,
    pub pac_url: Option<String>,
}

impl SystemProxies {
    /// Whether the system names automatic settings (a configuration script).
    pub fn automatic(&self) -> bool {
        self.pac_url.is_some()
    }
}

/// The three proxy settings. Closed set.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Choice {
    /// Follow the system's proxy settings (the default).
    System,
    /// No proxy: every connection goes direct.
    Off,
    /// This one proxy for every node (except this machine and the local networks).
    Manual(Proxy),
}

/// Where the setting comes from: fixed (the default "follow the system", or the CLI's `--proxy`), or read
/// whenever a connection opens (the app's machine settings, so a saved change applies to the next connection).
#[derive(Clone, Copy)]
enum Source {
    Fixed,
    Read(fn() -> Choice),
}

struct Routes {
    source: Source,
    fixed: Choice,
}

/// This process's proxy setting. A process that sets nothing follows the system settings, read through the
/// platform ([`SYSTEM`]).
static ROUTES: RwLock<Routes> = RwLock::new(Routes { source: Source::Fixed, fixed: Choice::System });

/// Fix the setting for this process (the CLI's `--proxy`).
pub fn set_choice(choice: Choice) {
    let mut r = ROUTES.write().unwrap_or_else(|e| e.into_inner());
    *r = Routes { source: Source::Fixed, fixed: choice };
}

/// Read the setting anew for each new connection through `choice` (the app's machine settings).
pub fn read_choice(choice: fn() -> Choice) {
    let mut r = ROUTES.write().unwrap_or_else(|e| e.into_inner());
    *r = Routes { source: Source::Read(choice), fixed: Choice::System };
}

// ───────────────────────── The system's settings ─────────────────────────

/// How long a read of the system settings stays fresh; every new connection checks, so a system change is
/// picked up within this time.
pub const SYSTEM_FRESH: Duration = Duration::from_secs(1);

/// The longest a new connection waits for the system settings (never past its exchange's deadline). If the
/// read takes longer, that connection goes as if the system gave no proxy and reports it
/// ([`Reading::system_unread`]); the result is still stored for the next connection.
pub const SYSTEM_READ_CAP: Duration = Duration::from_secs(2);

/// The platform's system proxy settings (`zikaron_os::system_proxies`) in the transport's form (`None`: they
/// could not be read).

/// One proxy the platform names, in the transport's form. In the https and http slots an `https` scheme is an
/// HTTPS proxy and any other a CONNECT proxy; in the SOCKS slot `socks4`/`socks4a` is SOCKS4 and any other
/// SOCKS5. Credentials written with it are kept; credentials that do not read drop the proxy.
fn system_proxy(p: &zikaron_os::ProxyPlace, socks: bool) -> Option<Proxy> {
    let kind = match (socks, p.scheme.as_str()) {
        (true, "socks4" | "socks4a") => Kind::Socks4,
        (true, _) => Kind::Socks5,
        (false, "https") => Kind::Https,
        (false, _) => Kind::Http,
    };
    let auth = match &p.user {
        None => None,
        Some(u) if u.is_empty() || u.len() > 255 => return None,
        Some(u) => Some(Auth { user: u.clone(), pass: if kind == Kind::Socks4 { None } else { p.pass.clone().map(Secret) } }),
    };
    Some(Proxy { kind, host: proxy_host(&p.host), port: p.port, auth })
}

impl SystemProxies {
    /// The platform's reading in the transport's form.
    pub fn from_platform(p: &zikaron_os::Proxies) -> SystemProxies {
        SystemProxies {
            https: p.https.as_ref().and_then(|x| system_proxy(x, false)),
            http: p.http.as_ref().and_then(|x| system_proxy(x, false)),
            socks: p.socks.as_ref().and_then(|x| system_proxy(x, true)),
            exceptions: p.exceptions.clone(),
            pac_url: p.pac_url.clone(),
        }
    }
}

/// The platform's system proxy settings (`zikaron_os::system_proxies`) in the transport's form (`None`: they
/// could not be read).
fn platform() -> Option<SystemProxies> {
    zikaron_os::system_proxies().map(|p| SystemProxies::from_platform(&p))
}

/// The system settings as last read, and the read in flight. One read at a time: a new connection that finds
/// no fresh result starts one (or joins the one in flight) and waits only up to its own bound; the lock is
/// never held while the system is being read.
struct System {
    reader: fn() -> Option<SystemProxies>,
    /// Which reader the reads belong to (a result from a replaced reader is discarded).
    epoch: u64,
    last: Option<(Instant, SystemProxies)>,
    /// How many reads have completed and been stored.
    landed: u64,
    reading: bool,
}

static SYSTEM: Mutex<System> = Mutex::new(System { reader: platform, epoch: 0, last: None, landed: 0, reading: false });
static SYSTEM_LANDED: Condvar = Condvar::new();

/// Follow the system the way a command line does from now on: the terminal's proxy variables when any is set,
/// else the system's settings (`zikaron_os::proxy::for_the_terminal`, the same on every system). The desktop
/// follows the system's settings alone.
pub fn follow_the_terminal() {
    read_system_with(terminal);
}

/// Follow the system the way the desktop does from now on: the system's proxy settings alone, never a terminal's
/// proxy variables (undoes [`follow_the_terminal`]).
pub fn follow_the_system() {
    read_system_with(platform);
}

fn terminal() -> Option<SystemProxies> {
    zikaron_os::proxy::for_the_terminal().map(|p| SystemProxies::from_platform(&p))
}

/// Read the system settings through `read` from now on instead of the platform (for tests and test harnesses;
/// `None`: the settings could not be read).
pub fn read_system_with(read: fn() -> Option<SystemProxies>) {
    let mut s = SYSTEM.lock().unwrap_or_else(|e| e.into_inner());
    *s = System { reader: read, epoch: s.epoch + 1, last: None, landed: s.landed, reading: false };
}

/// Wake every connection waiting for the system settings (on shutdown).
pub(crate) fn wake() {
    SYSTEM_LANDED.notify_all();
}

/// The result of waiting for the system settings.
enum Sys {
    Read(SystemProxies),
    /// Not read within the bound (or the read failed).
    Unread,
}

/// The system settings for a new connection: a read at most [`SYSTEM_FRESH`] old, else a new read waited for at
/// most `wait` (`stale_ok`: any previous read will do while a fresh one runs in the background; for display
/// only, never for connecting).
fn system_now(wait: Duration, stale_ok: bool) -> Sys {
    let mut s = SYSTEM.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((at, p)) = &s.last {
        if at.elapsed() < SYSTEM_FRESH {
            return Sys::Read(p.clone());
        }
    }
    let before = s.landed;
    if !s.reading {
        s.reading = true;
        let (read, epoch) = (s.reader, s.epoch);
        let started = std::thread::Builder::new().name("system-proxies".into()).spawn(move || {
            let got = std::panic::catch_unwind(read).ok().flatten();
            let mut s = SYSTEM.lock().unwrap_or_else(|e| e.into_inner());
            if s.epoch == epoch {
                s.reading = false;
                if let Some(p) = got {
                    s.last = Some((Instant::now(), p));
                    s.landed += 1;
                }
            }
            SYSTEM_LANDED.notify_all();
        });
        if started.is_err() {
            s.reading = false;
            return Sys::Unread;
        }
    }
    if stale_ok {
        return match &s.last {
            Some((_, p)) => Sys::Read(p.clone()),
            None => Sys::Unread,
        };
    }
    let until = Instant::now() + wait;
    while s.landed == before && !keep::closing() {
        let now = Instant::now();
        if now >= until || !s.reading {
            break;
        }
        s = SYSTEM_LANDED.wait_timeout(s, until - now).unwrap_or_else(|e| e.into_inner()).0;
    }
    match (&s.last, s.landed != before) {
        (Some((_, p)), true) => Sys::Read(p.clone()),
        _ => Sys::Unread,
    }
}


// ───────────────────────── The way ─────────────────────────

/// One route a new connection to a target can take.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Way {
    Direct,
    Through(Proxy),
}

/// What the system's automatic settings did for a route. Closed set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Script {
    /// None in force: no script named, or the setting is not to follow the system.
    None,
    /// The configuration script named the ways.
    Used,
    /// For display: the system names automatic settings whose answer for this node is not known yet (it is
    /// asked when a connection opens).
    Pending,
    /// The system names a script that could not be used (why): the route is the one without it.
    Trouble(crate::pac::PacTrouble),
}

/// A route decision for display and logging: the way a new connection goes, the ways tried after it when it
/// cannot be opened, and why it goes as it does when the reason is not the setting itself.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Reading {
    pub way: Way,
    /// The ways tried, in order, when `way` cannot be opened (a configuration script's list).
    pub then: Vec<Way>,
    pub choice: Choice,
    /// The node is this machine: always reached directly.
    pub loopback: bool,
    /// Under a manual proxy, the node is on a local network (private, link-local or `.local`): reached
    /// directly.
    pub local: bool,
    /// The system was followed but its settings were not read in time (or could not be read): this connection
    /// went as if there were no proxy.
    pub system_unread: bool,
    /// What the system's automatic settings did.
    pub script: Script,
}

impl Reading {
    /// Every way, in the order tried.
    pub fn ways(&self) -> Vec<Way> {
        std::iter::once(self.way.clone()).chain(self.then.iter().cloned()).collect()
    }
}

/// A host normalized for comparison: no brackets, lower case, without a single trailing dot.
fn host_form(host: &str) -> String {
    let h = host.trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    match h.strip_suffix('.') {
        Some(b) if !b.is_empty() && !b.ends_with('.') => b.to_string(),
        _ => h,
    }
}

/// A host's IP when it is an address literal; an IPv4-mapped IPv6 address yields its IPv4 address.
fn address_of(host: &str) -> Option<IpAddr> {
    match host.parse::<IpAddr>().ok()? {
        IpAddr::V6(v6) => Some(v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(IpAddr::V6(v6))),
        v4 => Some(v4),
    }
}

/// Whether a host is this machine: `localhost` and its subdomains (with or without the trailing dot), a
/// loopback address (`127.0.0.0/8`, `::1`, and their IPv4-mapped forms) or the unspecified address
/// (`0.0.0.0`, `::`). Always reached directly.
pub fn is_loopback(host: &str) -> bool {
    let h = host_form(host);
    if h == "localhost" || h.ends_with(".localhost") {
        return true;
    }
    match address_of(&h) {
        Some(ip) => ip.is_loopback() || ip.is_unspecified(),
        None => false,
    }
}

/// An address range: `a.b.c.d/n` or `v6/n`, plus macOS's short form with octets omitted (`169.254/16`,
/// `10/8`: missing octets are zero). A prefix wider than the address does not parse.
fn range_of(text: &str) -> Option<(IpAddr, u32)> {
    let (addr, bits) = text.split_once('/')?;
    let bits: u32 = bits.parse().ok()?;
    let ip = match addr.parse::<IpAddr>() {
        Ok(ip) => ip,
        Err(_) => {
            let parts: Vec<&str> = addr.split('.').collect();
            if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit())) {
                return None;
            }
            let mut octets = [0u8; 4];
            for (i, p) in parts.iter().enumerate() {
                octets[i] = p.parse().ok()?;
            }
            IpAddr::V4(octets.into())
        }
    };
    let width = if ip.is_ipv4() { 32 } else { 128 };
    (bits <= width).then_some((ip, bits))
}

fn in_range(ip: IpAddr, (net, bits): (IpAddr, u32)) -> bool {
    let mask = |width: u32| if bits == 0 { 0u128 } else { u128::MAX << (width - bits) };
    match (ip, net) {
        (IpAddr::V4(a), IpAddr::V4(n)) => (u32::from(a) as u128 & mask(32)) == (u32::from(n) as u128 & mask(32)),
        (IpAddr::V6(a), IpAddr::V6(n)) => (u128::from(a) & mask(128)) == (u128::from(n) & mask(128)),
        _ => false,
    }
}

/// Whether `pattern` (with `*` standing for any run of characters, dots included) covers all of `text`.
fn glob(pattern: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut pi, mut ti, mut star, mut mark) = (0usize, 0usize, None::<usize>, 0usize);
    while ti < t.len() {
        if pi < p.len() && p[pi] != '*' && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

/// An entry's port, if any: `name:port`, `a.b.c.d:port`, `[v6]:port`. A bare IPv6 address or a range has
/// none. `Err` when the port does not parse (the entry is skipped).
fn split_port(entry: &str) -> Result<(&str, Option<u16>), ()> {
    if let Some(rest) = entry.strip_prefix('[') {
        let (addr, after) = rest.split_once(']').ok_or(())?;
        return match after {
            "" => Ok((addr, None)),
            p => p.strip_prefix(':').and_then(|p| p.parse().ok()).map(|p| (addr, Some(p))).ok_or(()),
        };
    }
    if entry.contains('/') || entry.matches(':').count() != 1 {
        return Ok((entry, None));
    }
    let (body, port) = entry.rsplit_once(':').ok_or(())?;
    port.parse().map(|p| (body, Some(p))).map_err(|_| ())
}

/// Whether one entry of the system's exception list covers a target. Entry forms:
/// - `*`: every host.
/// - `<local>`: a plain name without a dot (never an address).
/// - `name`: that name and its subdomains; `*.name` and `.name`: the same (subdomains and the bare name).
/// - a pattern with `*` elsewhere (Windows's `10.*`, `192.168.*`, `*.corp.*`): matched over the whole host as
///   written (an IPv4-mapped address as its IPv4 form), `*` standing for any run of characters.
/// - an address (`192.168.1.5`, `::1`, `[::1]`): that address (IPv4-mapped forms are the IPv4 address).
/// - a range (`10.0.0.0/8`, `fe80::/10`, macOS's `169.254/16`): an address host inside it; a name is never
///   resolved to be compared.
/// - with a port (`name:8080`, `[::1]:8545`): as above, and only for that port.
/// - with a scheme (`http://…`, `https://…`): as above, and only for that scheme.
///
/// Skipped: an empty entry, one with whitespace or a control character, another scheme, a path, an unparsable
/// port or range, `<-loopback>` and any other `<…>` (loopback always goes direct anyway). A name entry never
/// covers an address host, nor an address entry a name.
fn entry_covers(raw: &str, t: &Target, host: &str, ip: Option<IpAddr>) -> bool {
    let mut e = raw.trim().to_ascii_lowercase();
    if e.is_empty() || e.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return false;
    }
    if let Some((scheme, rest)) = e.clone().split_once("://") {
        let wanted = match scheme {
            "http" => Scheme::Http,
            "https" => Scheme::Https,
            _ => return false,
        };
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        if wanted != t.scheme || rest.contains('/') {
            return false;
        }
        e = rest.to_string();
    }
    if e == "*" {
        return true;
    }
    if e == "<local>" {
        return ip.is_none() && !host.contains('.');
    }
    if e.starts_with('<') {
        return false;
    }
    let Ok((body, port)) = split_port(&e) else { return false };
    if port.is_some_and(|p| p != t.port) {
        return false;
    }
    if body.contains('/') {
        return match (ip, range_of(body)) {
            (Some(ip), Some(r)) => in_range(ip, r),
            _ => false,
        };
    }
    let suffix = |name: &str| ip.is_none() && !name.is_empty() && (host == name || host.ends_with(&format!(".{name}")));
    if let Some(name) = body.strip_prefix("*.") {
        if !name.contains('*') {
            return suffix(name.trim_end_matches('.'));
        }
    }
    if body.contains('*') {
        let shown = ip.map(|a| a.to_string()).unwrap_or_else(|| host.to_string());
        return glob(body, &shown);
    }
    if let Some(name) = body.strip_prefix('.') {
        return suffix(name.trim_end_matches('.'));
    }
    if let Some(entry_ip) = address_of(body.trim_start_matches('[').trim_end_matches(']')) {
        return ip == Some(entry_ip);
    }
    suffix(body.trim_end_matches('.'))
}

/// Whether the system's exception list covers a target (each entry interpreted by [`entry_covers`]).
pub fn excepted(t: &Target, exceptions: &[String]) -> bool {
    let host = host_form(&t.host);
    let ip = address_of(&host);
    exceptions.iter().any(|e| entry_covers(e, t, &host, ip))
}

/// A system-provided proxy host in canonical form: an IPv6 address in brackets (`[::1]`, never `::1`), names
/// in lower case.
fn proxy_host(host: &str) -> String {
    let h = host.trim().to_ascii_lowercase();
    match h.parse::<std::net::Ipv6Addr>() {
        Ok(_) => format!("[{h}]"),
        Err(_) => h,
    }
}

/// Whether a host is on a local network, reached directly under a manual proxy: a private IPv4 address
/// (RFC 1918: `10/8`, `172.16/12`, `192.168/16`), a link-local address (`169.254/16`, `fe80::/10`), their
/// IPv4-mapped forms, or a name under `.local` (with or without the trailing dot). A name is never resolved to
/// be judged.
pub fn is_local(host: &str) -> bool {
    let h = host_form(host);
    match address_of(&h) {
        Some(IpAddr::V4(a)) => a.is_private() || a.is_link_local(),
        Some(IpAddr::V6(a)) => (a.segments()[0] & 0xffc0) == 0xfe80,
        None => h.ends_with(".local"),
    }
}

/// The system's explicit proxies for a target: an excepted host goes direct; https uses the https proxy, else
/// SOCKS; http uses the http proxy, else SOCKS.
fn explicit(t: &Target, s: &SystemProxies) -> Way {
    if excepted(t, &s.exceptions) {
        return Way::Direct;
    }
    let pick = match t.scheme {
        Scheme::Https => s.https.as_ref().or(s.socks.as_ref()),
        Scheme::Http => s.http.as_ref().or(s.socks.as_ref()),
    };
    pick.cloned().map_or(Way::Direct, Way::Through)
}

/// The route for a target under a setting, the system settings and what their automatic settings give (pure;
/// [`way_for`] uses the current ones; `system` is `None` when not read in time). Loopback always goes direct;
/// under a manual proxy so does a local network ([`is_local`]); following the system, a script's ways come
/// first, else (none, or one that could not be used) the explicit proxies and their exceptions.
pub fn way_under(t: &Target, choice: &Choice, system: Option<&SystemProxies>, auto: &crate::pac::Auto) -> Reading {
    use crate::pac::Auto;
    let reading = |way: Way| Reading { way, then: Vec::new(), choice: choice.clone(), loopback: false, local: false, system_unread: false, script: Script::None };
    if is_loopback(&t.host) {
        return Reading { loopback: true, ..reading(Way::Direct) };
    }
    match choice {
        Choice::Off => reading(Way::Direct),
        Choice::Manual(_) if is_local(&t.host) => Reading { local: true, ..reading(Way::Direct) },
        Choice::Manual(p) => reading(Way::Through(p.clone())),
        Choice::System => {
            let Some(s) = system else { return Reading { system_unread: true, ..reading(Way::Direct) } };
            let script = match auto {
                Auto::Ways(ways) if !ways.is_empty() => return Reading { then: ways[1..].to_vec(), script: Script::Used, ..reading(ways[0].clone()) },
                Auto::Ways(_) | Auto::None => Script::None,
                Auto::Pending => Script::Pending,
                Auto::Trouble(x) => Script::Trouble(*x),
            };
            Reading { script, ..reading(explicit(t, s)) }
        }
    }
}

/// The current setting.
fn choice_now() -> Choice {
    let (source, fixed) = {
        let r = ROUTES.read().unwrap_or_else(|e| e.into_inner());
        (r.source, r.fixed.clone())
    };
    match source {
        Source::Fixed => fixed,
        Source::Read(f) => f(),
    }
}

/// The route a new connection to this target takes now: the current setting and, when following the system,
/// the system settings (waited for at most [`SYSTEM_READ_CAP`]) and their automatic settings (the script
/// fetched and asked), all within `wait`.
pub fn way_for(t: &Target, wait: Duration) -> Reading {
    let until = Instant::now() + wait;
    let choice = choice_now();
    let sys = match &choice {
        Choice::System if !is_loopback(&t.host) => match system_now(wait.min(SYSTEM_READ_CAP), false) {
            Sys::Read(p) => Some(p),
            Sys::Unread => None,
        },
        _ => None,
    };
    let auto = match &sys {
        Some(s) if s.automatic() => crate::pac::auto_for(t, s, until.saturating_duration_since(Instant::now())),
        _ => crate::pac::Auto::None,
    };
    let r = way_under(t, &choice, sys.as_ref(), &auto);
    note_script(t, &r);
    r
}

/// What the system's configuration script did for the latest connections, by the node's place (scheme, host,
/// port): its trouble when a connection following the system went without it and did not reach the node; nothing
/// when it was used, none is named, the system is not followed, or the connection was made ([`note_reached`]).
/// The latest connection's comes first.
static SCRIPT_SEEN: Mutex<Vec<(String, Option<crate::pac::PacTrouble>)>> = Mutex::new(Vec::new());

fn place_of(t: &Target) -> String {
    format!("{}://{}:{}", t.scheme.as_str(), t.host, t.port)
}

fn note_script(t: &Target, r: &Reading) {
    let trouble = match (&r.choice, r.script) {
        (Choice::System, Script::Trouble(x)) => Some(x),
        _ => None,
    };
    let place = place_of(t);
    let mut seen = SCRIPT_SEEN.lock().unwrap_or_else(|e| e.into_inner());
    seen.retain(|(p, _)| *p != place);
    seen.insert(0, (place, trouble));
    seen.truncate(64);
}

/// A connection to this target was made: whatever its script did, there is nothing to say about it.
pub(crate) fn note_reached(t: &Target) {
    let place = place_of(t);
    let mut seen = SCRIPT_SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(e) = seen.iter_mut().find(|(p, _)| *p == place) {
        e.1 = None;
    }
}

/// Why the latest connection to `url` went without the configuration script, when it then could not be made
/// (`None` when the script was used, none is named, the system is not followed, the connection was made, or none
/// was tried).
pub fn script_trouble_for(url: &str) -> Option<crate::pac::PacTrouble> {
    let t = crate::parse(url)?;
    let place = place_of(&t);
    SCRIPT_SEEN.lock().unwrap_or_else(|e| e.into_inner()).iter().find(|(p, _)| *p == place).and_then(|(_, x)| *x)
}

/// The same for the latest connection of this process, to any node (a command line makes its few connections and
/// says how the last one failed).
pub fn script_trouble_last() -> Option<crate::pac::PacTrouble> {
    SCRIPT_SEEN.lock().unwrap_or_else(|e| e.into_inner()).first().and_then(|(_, x)| *x)
}

/// How a new connection to this target would go, for display: never waits, fetches or runs a script (the last
/// system read and the answers already known will do); `system_unread` when none has been read yet, and
/// [`Script::Pending`] when the script's answer for this node is not known yet.
pub fn way_shown(t: &Target) -> Reading {
    way_shown_under(t, &choice_now())
}

/// [`way_shown`] under a given setting (a settings page shows the one in hand, without reading it again).
pub fn way_shown_under(t: &Target, choice: &Choice) -> Reading {
    let choice = choice.clone();
    let sys = match &choice {
        Choice::System if !is_loopback(&t.host) => match system_now(Duration::ZERO, true) {
            Sys::Read(p) => Some(p),
            Sys::Unread => None,
        },
        _ => None,
    };
    let auto = match &sys {
        Some(s) if s.automatic() => crate::pac::auto_known(t, s),
        _ => crate::pac::Auto::None,
    };
    way_under(t, &choice, sys.as_ref(), &auto)
}

/// A failure on the proxy leg, reported as the proxy's ([`Fail::Proxy`], the proxy named): the node was never
/// reached, a proxy that does not answer within the deadline included. Only shutdown passes through unchanged.
fn through(p: &Proxy, f: Fail) -> Fail {
    match f {
        Fail::Closed => f,
        Fail::Late(d) => Fail::Proxy(format!("via proxy {}: no answer within {} ms", p.spelled(), d.as_millis())),
        Fail::Name(x) | Fail::Connect(x) | Fail::Handshake(x) | Fail::Certificate(x) | Fail::Stream(x) | Fail::Proxy(x) => {
            Fail::Proxy(format!("via proxy {}: {x}", p.spelled()))
        }
        Fail::Overlong(n) => Fail::Proxy(format!("via proxy {}: its answer passed {n} bytes", p.spelled())),
        Fail::ProxyCredentials { given, said } => Fail::ProxyCredentials { given, said: format!("via proxy {}: {said}", p.spelled()) },
    }
}

/// Maximum proxy response head size (a CONNECT response is a few lines).
const MAX_PROXY_HEAD: usize = 16 << 10;

/// Open a tunnel to the target through a proxy: connect to the proxy (every address, one deadline), track the
/// socket for shutdown, TLS to an HTTPS proxy (its certificate verified against its name), then send CONNECT
/// or the SOCKS request. The tunnel is a byte stream to the target's port; TLS for https targets is the
/// caller's job.
pub(crate) fn tunnel(t: &Target, p: &Proxy, clock: &Clock) -> Result<(Sock, keep::Ticket), Fail> {
    let tcp = dial::connect(&p.target(), clock).map_err(|f| through(p, f))?;
    let ticket = keep::ticket(&tcp)?;
    let opened = open_tunnel(Sock::Tcp(tcp), t, p, clock);
    // Shut down meanwhile: closed, counted once (a wait inside that found the flag already counted its cut).
    if keep::closing() {
        return Err(match opened {
            Err(Fail::Closed) => Fail::Closed,
            _ => keep::cut(),
        });
    }
    let sock = opened.map_err(|f| through(p, f))?;
    Ok((sock, ticket))
}

/// The proxy leg after connecting: TLS for an HTTPS proxy, then the tunnel request of its kind.
fn open_tunnel(sock: Sock, t: &Target, p: &Proxy, clock: &Clock) -> Result<Sock, Fail> {
    let mut sock = match p.kind {
        Kind::Https => Sock::Tls(Box::new(crate::tls_connect(p.target().dial_host(), sock, clock)?)),
        _ => sock,
    };
    match p.kind {
        Kind::Http | Kind::Https => connect_tunnel(&mut sock, t, p.auth.as_ref(), clock)?,
        Kind::Socks5 => socks_tunnel(&mut sock, t, p.auth.as_ref(), clock)?,
        Kind::Socks4 => socks4_tunnel(&mut sock, t, p.auth.as_ref(), clock)?,
    }
    Ok(sock)
}

fn io(e: std::io::Error, clock: &Clock) -> Fail {
    if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) {
        if let Err(late) = clock.left() {
            return late;
        }
    }
    Fail::Stream(e.to_string())
}

/// Read exactly `buf.len()` bytes within the time left; shutting down cuts it within a slice.
fn read_exact(sock: &mut Sock, buf: &mut [u8], clock: &Clock) -> Result<(), Fail> {
    let mut at = 0;
    while at < buf.len() {
        clock.bound(sock.tcp())?;
        let got = sock.read(&mut buf[at..]);
        if keep::closing() {
            return Err(keep::cut());
        }
        match got {
            Ok(0) => return Err(Fail::Stream("the proxy closed the connection".into())),
            Ok(n) => at += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                clock.left()?;
            }
            Err(e) => return Err(io(e, clock)),
        }
    }
    Ok(())
}

/// Write it all within the time left, then wipe it (it may hold a password).
fn send_wiped(sock: &mut Sock, mut bytes: Vec<u8>, clock: &Clock) -> Result<(), Fail> {
    let sent = clock.bound(sock.tcp()).and_then(|_| sock.write_all(&bytes).and_then(|_| sock.flush()).map_err(|e| io(e, clock)));
    zikaron_os::wipe(&mut bytes);
    sent
}

/// Standard base64 (RFC 4648 §4, padded), into a buffer the caller wipes.
fn base64(input: &[u8]) -> Vec<u8> {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(input.len().div_ceil(3) * 4);
    for c in input.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(A[(n >> 18) as usize & 63]);
        out.push(A[(n >> 12) as usize & 63]);
        out.push(if c.len() > 1 { A[(n >> 6) as usize & 63] } else { b'=' });
        out.push(if c.len() > 2 { A[n as usize & 63] } else { b'=' });
    }
    out
}

/// HTTP CONNECT: send `CONNECT host:port` (with `Proxy-Authorization: Basic …` when credentials are written
/// with the proxy) and read the response head; a 2xx opens the tunnel, anything else is the proxy's refusal
/// with its status line.
fn connect_tunnel(sock: &mut Sock, t: &Target, auth: Option<&Auth>, clock: &Clock) -> Result<(), Fail> {
    let place = format!("{}:{}", t.host, t.port);
    let mut lines = Vec::new();
    if let Some(a) = auth {
        let mut pair = format!("{}:{}", a.user, a.pass.as_ref().map(Secret::expose).unwrap_or(""));
        let mut coded = base64(pair.as_bytes());
        zikaron_os::wipe_string(&mut pair);
        lines.extend_from_slice(b"Proxy-Authorization: Basic ");
        lines.extend_from_slice(&coded);
        lines.extend_from_slice(b"\r\n");
        zikaron_os::wipe(&mut coded);
    }
    let mut ask = crate::request_head("CONNECT", &place, &place, "").into_bytes();
    // The caller's lines go before the blank line that ends the head.
    let end = ask.len() - 2;
    ask.splice(end..end, lines.iter().copied());
    zikaron_os::wipe(&mut lines);
    send_wiped(sock, ask, clock)?;
    // Read byte by byte to the end of the head, so no tunnel data is consumed.
    let mut head: Vec<u8> = Vec::new();
    let mut one = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() > MAX_PROXY_HEAD {
            return Err(Fail::Stream("the proxy's answer has no end of head".into()));
        }
        read_exact(sock, &mut one, clock)?;
        head.push(one[0]);
    }
    let text = String::from_utf8_lossy(&head);
    let line = text.lines().next().unwrap_or("").trim().to_string();
    // An HTTP status line (`HTTP/1.x NNN …`); anything else is an unreadable response.
    let status: Option<u16> = match line.split_whitespace().collect::<Vec<_>>()[..] {
        [v, code, ..] if v.starts_with("HTTP/1.") && code.len() == 3 => code.parse().ok(),
        _ => None,
    };
    match status {
        Some(s) if (200..300).contains(&s) => Ok(()),
        Some(407) if auth.is_none() => Err(Fail::ProxyCredentials { given: false, said: format!("CONNECT {place} answered {line} (it asks for credentials, and none are written with this proxy)") }),
        Some(407) => Err(Fail::ProxyCredentials { given: true, said: format!("CONNECT {place} answered {line} (the credentials written with this proxy were refused)") }),
        _ => Err(Fail::Stream(format!("CONNECT {place} answered {line}"))),
    }
}

/// What a SOCKS5 reply code means (RFC 1928 §6).
fn socks_said(code: u8) -> &'static str {
    match code {
        1 => "general failure",
        2 => "not allowed by its rules",
        3 => "network unreachable",
        4 => "host unreachable",
        5 => "connection refused",
        6 => "time to live expired",
        7 => "command not supported",
        8 => "address type not supported",
        _ => "an unassigned code",
    }
}

/// SOCKS5: the greeting (no authentication, and user name and password when credentials are written with the
/// proxy), the RFC 1929 exchange when the proxy picks it, then CONNECT to the host by name (an address literal
/// in its own address type), then read the full reply; anything but success is the proxy's refusal with its
/// code.
fn socks_tunnel(sock: &mut Sock, t: &Target, auth: Option<&Auth>, clock: &Clock) -> Result<(), Fail> {
    clock.bound(sock.tcp())?;
    let greeting: &[u8] = if auth.is_some() { &[5, 2, 0, 2] } else { &[5, 1, 0] };
    sock.write_all(greeting).map_err(|e| io(e, clock))?;
    let mut pick = [0u8; 2];
    read_exact(sock, &mut pick, clock)?;
    match (pick, auth) {
        ([5, 0], _) => {}
        ([5, 2], Some(a)) => {
            let pass = a.pass.as_ref().map(Secret::expose).unwrap_or("");
            // Each field carries its length in one byte (RFC 1929): longer credentials, wherever they came from,
            // are refused by name here rather than sent cut.
            let (Ok(ulen), Ok(plen)) = (u8::try_from(a.user.len()), u8::try_from(pass.len())) else {
                return Err(Fail::Stream("the user name or password written with this proxy is longer than SOCKS5 carries (255 bytes)".into()));
            };
            let mut ask = Vec::with_capacity(3 + a.user.len() + pass.len());
            ask.push(1);
            ask.push(ulen);
            ask.extend_from_slice(a.user.as_bytes());
            ask.push(plen);
            ask.extend_from_slice(pass.as_bytes());
            send_wiped(sock, ask, clock)?;
            let mut said = [0u8; 2];
            read_exact(sock, &mut said, clock)?;
            if said[1] != 0 {
                return Err(Fail::ProxyCredentials { given: true, said: format!("SOCKS5 refused the user name and password written with it (status {})", said[1]) });
            }
        }
        ([5, 0xFF], None) => return Err(Fail::ProxyCredentials { given: false, said: "SOCKS5 asks for authentication, and no credentials are written with this proxy".into() }),
        ([5, 0xFF], Some(_)) => return Err(Fail::Stream("SOCKS5 accepts neither no authentication nor a user name and password".into())),
        ([5, m], _) => return Err(Fail::Stream(format!("SOCKS5 picked method {m}, which was not offered"))),
        _ => return Err(Fail::Stream("the SOCKS5 greeting answer does not read".into())),
    }
    let host = t.dial_host();
    let mut ask = vec![5u8, 1, 0];
    match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(a)) => {
            ask.push(1);
            ask.extend_from_slice(&a.octets());
        }
        Ok(std::net::IpAddr::V6(a)) => {
            ask.push(4);
            ask.extend_from_slice(&a.octets());
        }
        Err(_) => {
            let name = host.as_bytes();
            if name.len() > 255 {
                return Err(Fail::Name(format!("{host}: longer than SOCKS5 carries")));
            }
            ask.push(3);
            ask.push(name.len() as u8);
            ask.extend_from_slice(name);
        }
    }
    ask.extend_from_slice(&t.port.to_be_bytes());
    clock.bound(sock.tcp())?;
    sock.write_all(&ask).map_err(|e| io(e, clock))?;
    let mut reply = [0u8; 4];
    read_exact(sock, &mut reply, clock)?;
    if reply[0] != 5 {
        return Err(Fail::Stream("the SOCKS5 reply does not read".into()));
    }
    if reply[1] != 0 {
        return Err(Fail::Stream(format!("SOCKS5 refused {}:{} (code {}: {})", host, t.port, reply[1], socks_said(reply[1]))));
    }
    let rest = match reply[3] {
        1 => 4 + 2,
        4 => 16 + 2,
        3 => {
            let mut n = [0u8; 1];
            read_exact(sock, &mut n, clock)?;
            n[0] as usize + 2
        }
        _ => return Err(Fail::Stream("the SOCKS5 reply does not read".into())),
    };
    let mut bound = vec![0u8; rest];
    read_exact(sock, &mut bound, clock)
}

/// SOCKS4: CONNECT with the user id written with the proxy (empty when none); a host name goes in the 4a form
/// (address `0.0.0.1`, the name after the user id) for the proxy to resolve; an IPv4 literal as itself; an
/// IPv6 literal cannot be carried. The 8-byte reply's code 90 opens the tunnel.
fn socks4_tunnel(sock: &mut Sock, t: &Target, auth: Option<&Auth>, clock: &Clock) -> Result<(), Fail> {
    let host = t.dial_host();
    let mut ask = vec![4u8, 1];
    ask.extend_from_slice(&t.port.to_be_bytes());
    let name = match host.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V4(a)) => {
            ask.extend_from_slice(&a.octets());
            None
        }
        Ok(std::net::IpAddr::V6(_)) => return Err(Fail::Name(format!("{host}: SOCKS4 carries no IPv6 address"))),
        Err(_) => {
            ask.extend_from_slice(&[0, 0, 0, 1]);
            Some(host)
        }
    };
    ask.extend_from_slice(auth.map(|a| a.user.as_bytes()).unwrap_or(b""));
    ask.push(0);
    if let Some(n) = name {
        ask.extend_from_slice(n.as_bytes());
        ask.push(0);
    }
    clock.bound(sock.tcp())?;
    sock.write_all(&ask).map_err(|e| io(e, clock))?;
    let mut reply = [0u8; 8];
    read_exact(sock, &mut reply, clock)?;
    match reply[1] {
        90 => Ok(()),
        91 => Err(Fail::Stream(format!("SOCKS4 refused {}:{} (code 91: rejected or failed)", host, t.port))),
        92 | 93 => Err(Fail::ProxyCredentials {
            given: auth.is_some_and(|a| !a.user.is_empty()),
            said: format!("SOCKS4 refused {}:{} (code {}: the user id was not confirmed)", host, t.port, reply[1]),
        }),
        c => Err(Fail::Stream(format!("SOCKS4 refused {}:{} (code {c})", host, t.port))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(url: &str) -> Target {
        crate::parse(url).expect("an address")
    }

    fn px(kind: Kind, host: &str, port: u16) -> Proxy {
        Proxy { kind, host: host.into(), port, auth: None }
    }

    /// A manual proxy is `scheme://[user[:pass]@]host:port` with an HTTP, HTTPS, SOCKS5 or SOCKS4 scheme; other
    /// schemes, a missing port, a path and an empty host are refused by name, and so are credentials that do
    /// not read. The password is never spelled, shown or printed.
    #[test]
    fn a_typed_proxy_is_read_or_refused_by_name() {
        assert_eq!(proxy_of("http://127.0.0.1:7890"), Ok(px(Kind::Http, "127.0.0.1", 7890)));
        assert_eq!(proxy_of(" SOCKS5://Proxy.Example:1080/ ").map(|p| p.spelled()), Ok("socks5://proxy.example:1080".into()));
        assert_eq!(proxy_of("socks5://[::1]:1080").map(|p| p.host), Ok("[::1]".into()));
        assert_eq!(proxy_of("https://proxy.example:443").map(|p| p.kind), Ok(Kind::Https));
        assert_eq!(proxy_of("socks4a://h:1").map(|p| p.kind), Ok(Kind::Socks4));
        let c = proxy_of("http://us%40er:p%3Aw@proxy.example:8080").expect("credentials");
        assert_eq!((c.auth.as_ref().map(|a| a.user.as_str()), c.auth.as_ref().and_then(|a| a.pass.as_ref()).map(Secret::expose)), (Some("us@er"), Some("p:w")));
        assert_eq!((c.spelled(), c.shown(), c.has_password()), ("http://proxy.example:8080".into(), "http://us@er@proxy.example:8080".into(), true));
        assert!(!format!("{c:?}").contains("p:w"), "never printed");
        assert_eq!(proxy_of("socks4://id@h:1").map(|p| p.shown()), Ok("socks4://id@h:1".into()));
        for bad in ["http://@h:1", "http://:pw@h:1", "socks4://id:pw@h:1", &format!("socks5://{}:p@h:1", "u".repeat(256)), "http://u s:p@h:1"] {
            assert_eq!(proxy_of(bad), Err(NotAProxy::Credentials), "{bad}");
        }
        for bad in ["ftp://proxy.example:21", "proxy.example:8080", "http://proxy.example", "http://proxy.example:", "http://:8080", "http://proxy.example:8080/x", "http://proxy.example:0", "http://proxy.example:65536", "http://[::1]", "http://u:p@"] {
            assert_eq!(proxy_of(bad), Err(NotAProxy::Shape), "{bad}");
        }
        assert_eq!(proxy_text_shown("http://u:secret@h:1"), ("http://u@h:1".into(), true));
        assert_eq!(proxy_text_shown("not a proxy u:secret@x"), ("not a proxy u@x".into(), true));
        assert_eq!(proxy_text_shown("socks5://h:1"), ("socks5://h:1".into(), false));
    }

    fn none() -> crate::pac::Auto {
        crate::pac::Auto::None
    }

    /// Loopback always goes direct; following the system uses its https, http and SOCKS proxies by target
    /// scheme and honours its exceptions; a manual proxy is not used for local networks.
    #[test]
    fn the_way_follows_the_choice_and_the_system() {
        let manual = Choice::Manual(px(Kind::Socks5, "p", 1));
        let sys = SystemProxies { https: Some(px(Kind::Http, "hp", 8443)), http: Some(px(Kind::Http, "p", 8080)), socks: Some(px(Kind::Socks5, "s", 1080)), exceptions: vec!["*.corp.example".into(), "<local>".into(), "10.0.0.0/8".into()], ..Default::default() };
        for host in ["http://localhost:8545", "http://127.0.0.9:1", "http://[::1]:1", "https://a.localhost/"] {
            for c in [Choice::System, Choice::Off, manual.clone()] {
                assert_eq!(way_under(&t(host), &c, Some(&sys), &none()).way, Way::Direct, "{host} {c:?}");
            }
        }
        assert_eq!(way_under(&t("https://node.example/"), &Choice::Off, Some(&sys), &none()).way, Way::Direct);
        assert_eq!(way_under(&t("https://node.example/"), &manual, Some(&sys), &none()).way, Way::Through(px(Kind::Socks5, "p", 1)));
        let through = |url: &str, s: &SystemProxies| match way_under(&t(url), &Choice::System, Some(s), &none()).way {
            Way::Through(p) => p.spelled(),
            Way::Direct => "direct".into(),
        };
        assert_eq!(through("https://node.example/", &sys), "http://hp:8443");
        assert_eq!(through("http://node.example/", &sys), "http://p:8080");
        assert_eq!(through("https://a.corp.example/", &sys), "direct");
        assert_eq!(through("https://corp.example/", &sys), "direct");
        assert_eq!(through("https://intranet/", &sys), "direct");
        assert_eq!(through("https://10.1.2.3/", &sys), "direct", "a range entry is read");
        let socks_only = SystemProxies { socks: Some(px(Kind::Socks5, "s", 1080)), ..Default::default() };
        assert_eq!(through("https://node.example/", &socks_only), "socks5://s:1080");
        assert_eq!(through("http://node.example/", &socks_only), "socks5://s:1080");
        let r = way_under(&t("https://node.example/"), &Choice::System, None, &none());
        assert_eq!((r.way, r.system_unread), (Way::Direct, true));
    }

    /// Following the system, a script's ways come first and the rest are tried after; a script that could not
    /// be used leaves the explicit proxies, and says why; an answer not known yet is said as such.
    #[test]
    fn a_scripts_ways_come_first_and_its_trouble_leaves_the_explicit_proxies() {
        use crate::pac::{Auto, PacTrouble};
        let sys = SystemProxies { https: Some(px(Kind::Http, "hp", 8443)), pac_url: Some("http://127.0.0.1:1/pac".into()), ..Default::default() };
        let ways = vec![Way::Through(px(Kind::Socks5, "s", 1)), Way::Direct];
        let r = way_under(&t("https://node.example/"), &Choice::System, Some(&sys), &Auto::Ways(ways.clone()));
        assert_eq!((r.ways(), r.script), (ways, Script::Used));
        let r = way_under(&t("https://node.example/"), &Choice::System, Some(&sys), &Auto::Trouble(PacTrouble::Timeout));
        assert_eq!((r.way, r.script), (Way::Through(px(Kind::Http, "hp", 8443)), Script::Trouble(PacTrouble::Timeout)));
        let bare = SystemProxies { pac_url: Some("http://127.0.0.1:1/pac".into()), ..Default::default() };
        let r = way_under(&t("https://node.example/"), &Choice::System, Some(&bare), &Auto::Trouble(PacTrouble::Unfetched));
        assert_eq!((r.way, r.script), (Way::Direct, Script::Trouble(PacTrouble::Unfetched)));
        assert_eq!(way_under(&t("https://node.example/"), &Choice::System, Some(&bare), &Auto::Pending).script, Script::Pending);
        let r = way_under(&t("http://localhost:8545/"), &Choice::System, Some(&bare), &Auto::Ways(vec![Way::Through(px(Kind::Http, "x", 1))]));
        assert_eq!((r.way, r.loopback), (Way::Direct, true), "loopback stays direct whatever the script says");
    }

    /// Under a manual proxy, private, link-local and `.local` hosts go direct and say so; public ones go
    /// through it; a name is never resolved to be judged.
    #[test]
    fn a_local_network_goes_straight_under_a_manual_proxy() {
        let manual = Choice::Manual(px(Kind::Http, "p", 1));
        for host in ["https://10.0.0.5/", "https://172.16.1.1/", "https://172.31.255.255/", "https://192.168.1.7:8545/", "https://169.254.1.1/", "https://[fe80::1]/", "https://[::ffff:192.168.0.1]/", "https://printer.local/", "https://Printer.LOCAL./"] {
            let r = way_under(&t(host), &manual, None, &none());
            assert_eq!((r.way, r.local), (Way::Direct, true), "{host}");
        }
        for host in ["https://8.8.8.8/", "https://172.32.0.1/", "https://192.169.0.1/", "https://[2001:db8::1]/", "https://node.example/", "https://local.example/", "https://intranet/"] {
            let r = way_under(&t(host), &manual, None, &none());
            assert_eq!((r.way, r.local), (Way::Through(px(Kind::Http, "p", 1)), false), "{host}");
        }
    }

    fn covers(url: &str, entry: &str) -> bool {
        excepted(&t(url), &[entry.to_string()])
    }

    /// `*`: every host, names and addresses.
    #[test]
    fn an_exception_star_covers_every_host() {
        assert!(covers("https://node.example/", "*") && covers("https://10.1.2.3/", "*") && covers("https://[2001:db8::1]/", "*"));
    }

    /// `<local>`: a plain name without a dot; never a dotted name, never an address.
    #[test]
    fn an_exception_local_covers_plain_names_only() {
        assert!(covers("https://intranet/", "<local>"));
        assert!(!covers("https://node.example/", "<local>") && !covers("https://10.1.2.3/", "<local>") && !covers("https://[::2]/", "<local>"));
    }

    /// A name: itself and its subdomains, ignoring case and a trailing dot; not a longer name with the same
    /// suffix; never an address.
    #[test]
    fn an_exception_name_covers_itself_and_its_subdomains() {
        assert!(covers("https://corp.example/", "corp.example") && covers("https://a.b.corp.example/", "Corp.Example."));
        assert!(covers("https://corp.example./", "corp.example"));
        assert!(!covers("https://notcorp.example/", "corp.example") && !covers("https://corp.example.org/", "corp.example"));
        assert!(!covers("https://192.168.1.5/", "1.5"), "a name entry never covers an address by its tail");
    }

    /// `*.name` and `.name`: the subdomains and the bare name.
    #[test]
    fn an_exception_suffix_covers_the_subdomains_and_the_bare_name() {
        for e in ["*.corp.example", ".corp.example"] {
            assert!(covers("https://a.corp.example/", e) && covers("https://corp.example/", e), "{e}");
            assert!(!covers("https://xcorp.example/", e) && !covers("https://10.0.0.1/", e), "{e}");
        }
    }

    /// A pattern with `*` elsewhere (including Windows address wildcards): matched over the whole host.
    #[test]
    fn an_exception_wildcard_pattern_covers_by_the_whole_host() {
        assert!(covers("https://10.1.2.3/", "10.*") && covers("https://192.168.0.7/", "192.168.*"));
        assert!(!covers("https://110.1.2.3/", "10.*") && !covers("https://192.169.0.7/", "192.168.*"));
        assert!(covers("https://a.corp.internal/", "*.corp.*") && !covers("https://corp.internal/", "*.corp.*"));
        assert!(covers("https://[::ffff:10.1.2.3]/", "10.*"), "an IPv4-mapped host is its IPv4 form");
    }

    /// An address: that address (IPv6 with or without brackets, IPv4-mapped forms as IPv4); never a name.
    #[test]
    fn an_exception_address_covers_that_address() {
        assert!(covers("https://192.168.1.5/", "192.168.1.5") && !covers("https://192.168.1.50/", "192.168.1.5"));
        assert!(covers("https://[2001:db8::1]/", "2001:db8::1") && covers("https://[2001:db8::1]/", "[2001:DB8:0::1]"));
        assert!(covers("https://[::ffff:192.168.1.5]/", "192.168.1.5"));
        assert!(!covers("https://node.example/", "192.168.1.5"));
    }

    /// A range: an address inside it; macOS's short form with octets left off; a name is not resolved.
    #[test]
    fn an_exception_range_covers_the_addresses_inside_it() {
        assert!(covers("https://10.200.0.1/", "10.0.0.0/8") && !covers("https://11.0.0.1/", "10.0.0.0/8"));
        assert!(covers("https://169.254.3.4/", "169.254/16") && covers("https://10.9.9.9/", "10/8") && covers("https://192.168.1.77/", "192.168.1/24"));
        assert!(!covers("https://169.255.0.1/", "169.254/16"));
        assert!(covers("https://[fe80::1]/", "fe80::/10") && !covers("https://[2001:db8::1]/", "fe80::/10"));
        assert!(covers("https://1.2.3.4/", "0.0.0.0/0") && covers("https://1.2.3.4/", "1.2.3.4/32"));
        assert!(!covers("https://node.example/", "10.0.0.0/8"), "a name is never resolved to be compared");
        assert!(!covers("https://10.0.0.1/", "10.0.0.0/8x") && !covers("https://10.0.0.1/", "10.0.0.0/33") && !covers("https://10.0.0.1/", "10.0.0.0/") && !covers("https://10.0.0.1/", "1.2.3.4.5/8"), "a range that does not read is passed over");
    }

    /// With a port: as without it, and only for that port; an unparsable port is skipped.
    #[test]
    fn an_exception_with_a_port_covers_that_port_only() {
        assert!(covers("https://node.example:8545/", "node.example:8545") && !covers("https://node.example/", "node.example:8545"));
        assert!(covers("http://10.0.0.1:8545/", "10.0.0.1:8545") && covers("https://[::2]:8545/", "[::2]:8545") && !covers("https://[::2]:8546/", "[::2]:8545"));
        assert!(!covers("https://node.example/", "node.example:x") && !covers("https://node.example/", "node.example:"));
    }

    /// With a scheme: as without it, and only for that scheme; another scheme or a path is skipped.
    #[test]
    fn an_exception_with_a_scheme_covers_that_scheme_only() {
        assert!(covers("http://node.example/", "http://node.example") && !covers("https://node.example/", "http://node.example"));
        assert!(covers("https://a.node.example/", "https://*.node.example/"));
        assert!(!covers("https://node.example/", "ftp://node.example") && !covers("https://node.example/", "https://node.example/x"));
    }

    /// Skipped: empty, whitespace, control characters, `<-loopback>` and other `<…>` entries.
    #[test]
    fn an_exception_that_does_not_read_is_passed_over() {
        for e in ["", "   ", "a b", "node\t.example", "<-loopback>", "<anything>"] {
            assert!(!covers("https://node.example/", e) && !covers("https://intranet/", e), "{e:?}");
        }
        assert!(excepted(&t("https://intranet/"), &["".into(), "<local>".into()]), "one entry passed over, the next still read");
    }

    /// This machine in every form goes direct: `localhost` in any case with or without the trailing dot, its
    /// subdomains, loopback addresses and their IPv4-mapped forms, the unspecified address. Names merely
    /// containing the word, and other addresses, do not.
    #[test]
    fn this_machine_in_every_form_is_loopback() {
        for h in ["localhost", "LOCALHOST", "localhost.", "a.localhost", "a.localhost.", "127.0.0.1", "127.9.9.9", "[::1]", "::1", "[::ffff:127.0.0.1]", "0.0.0.0", "[::]"] {
            assert!(is_loopback(h), "{h}");
        }
        for h in ["localhost.example", "mylocalhost", "localhost..", "128.0.0.1", "[::2]", "[::ffff:10.0.0.1]", "node.example"] {
            assert!(!is_loopback(h), "{h}");
        }
        let any = Choice::Manual(px(Kind::Http, "p", 1));
        let r = way_under(&t("http://localhost.:8545/"), &any, None, &none());
        assert_eq!((r.way, r.loopback), (Way::Direct, true));
    }

    /// A system proxy given as a bare IPv6 address is bracketed when spelled and unbracketed when dialed; its
    /// kind follows its slot and scheme, and its credentials are kept.
    #[test]
    fn a_system_proxy_takes_its_kind_and_credentials_and_brackets() {
        let place = |scheme: &str, host: &str, user: Option<&str>, pass: Option<&str>| zikaron_os::ProxyPlace { scheme: scheme.into(), host: host.into(), port: 7890, user: user.map(Into::into), pass: pass.map(Into::into) };
        let p = system_proxy(&place("", "::1", None, None), false).expect("read");
        assert_eq!((p.spelled(), p.target().dial_host().to_string()), ("http://[::1]:7890".into(), "::1".into()));
        assert_eq!(system_proxy(&place("https", "h", None, None), false).map(|p| p.kind), Some(Kind::Https));
        assert_eq!(system_proxy(&place("http", "h", None, None), true).map(|p| p.kind), Some(Kind::Socks5));
        assert_eq!(system_proxy(&place("socks4a", "h", None, None), true).map(|p| p.kind), Some(Kind::Socks4));
        let a = system_proxy(&place("http", "h", Some("u"), Some("pw")), false).expect("read");
        assert_eq!((a.shown(), a.has_password()), ("http://u@h:7890".into(), true));
        assert_eq!(system_proxy(&place("socks4", "h", Some("u"), Some("pw")), true).map(|p| p.has_password()), Some(false));
    }
}
