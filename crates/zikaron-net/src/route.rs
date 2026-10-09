//! Proxy routing for new connections: direct to the node, or through a proxy (HTTP CONNECT or SOCKS5), and
//! the tunnel itself.
//!
//! The route is chosen per new connection from one of three settings: follow the system (platform proxy
//! settings, read when the connection opens, so a system change applies to the next connection without a
//! restart), none, or one manually entered proxy. Loopback hosts always go direct. Through a proxy, the proxy
//! resolves the host name (CONNECT names it, SOCKS5 sends the domain form); the leg to the proxy is dialed like
//! any connection (every address, one deadline); TLS stays end to end with the node, verified against the
//! node's name. An unreachable or refusing proxy is a connection failure naming the proxy and its answer; no
//! extra failure kind is added.

use crate::{dial, keep, Clock, Fail, Scheme, Target};
use std::io::{Read, Write};
use std::net::{IpAddr, TcpStream};
use std::sync::{Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

/// The two proxy kinds. Closed set.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Kind {
    /// HTTP, tunnelled with CONNECT.
    Http,
    /// SOCKS version 5, no authentication.
    Socks5,
}

/// One proxy: its kind, host and port.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Proxy {
    pub kind: Kind,
    pub host: String,
    pub port: u16,
}

impl Proxy {
    /// The proxy's canonical spelling: `http://host:port` or `socks5://host:port`.
    pub fn spelled(&self) -> String {
        let scheme = match self.kind {
            Kind::Http => "http",
            Kind::Socks5 => "socks5",
        };
        format!("{scheme}://{}:{}", self.host, self.port)
    }

    fn target(&self) -> Target {
        Target { scheme: Scheme::Http, host: self.host.clone(), port: self.port, path: "/".into() }
    }
}

/// Why a manually entered proxy address is rejected. Closed set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NotAProxy {
    /// Not `http://host:port` or `socks5://host:port` (another scheme, no port, a path, an empty host).
    Shape,
    /// It carries credentials (`user@`); proxies that need them are not supported.
    Credentials,
}

/// Parse a manually entered proxy address: `http://host:port` (CONNECT) or `socks5://host:port`, scheme and
/// host case-insensitive, port required; a bracketed IPv6 literal host is accepted. Anything else is refused
/// by name, including an address carrying credentials.
pub fn proxy_of(typed: &str) -> Result<Proxy, NotAProxy> {
    let (scheme, rest) = typed.trim().split_once("://").ok_or(NotAProxy::Shape)?;
    let kind = match scheme.to_ascii_lowercase().as_str() {
        "http" => Kind::Http,
        "socks5" | "socks5h" => Kind::Socks5,
        _ => return Err(NotAProxy::Shape),
    };
    let authority = rest.strip_suffix('/').unwrap_or(rest);
    if authority.contains('@') {
        return Err(NotAProxy::Credentials);
    }
    if authority.is_empty() || authority.contains('/') || authority.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(NotAProxy::Shape);
    }
    // The address parser handles host and port; the port must be explicit.
    let t = crate::parse(&format!("http://{authority}/")).ok_or(NotAProxy::Shape)?;
    let (_, port) = authority.rsplit_once(':').ok_or(NotAProxy::Shape)?;
    if port.is_empty() || port.ends_with(']') || t.port == 0 {
        return Err(NotAProxy::Shape);
    }
    Ok(Proxy { kind, host: t.host, port: t.port })
}

/// The system proxy settings as read from the platform: the https proxy, the http proxy, a SOCKS proxy, the
/// exception list, and whether an automatic configuration script (PAC) is set (unsupported: treated as no
/// proxy).
#[derive(Clone, Default, PartialEq, Eq, Debug)]
pub struct SystemProxies {
    pub https: Option<(String, u16)>,
    pub http: Option<(String, u16)>,
    pub socks: Option<(String, u16)>,
    pub exceptions: Vec<String>,
    pub auto_config: bool,
}

/// The three proxy settings. Closed set.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Choice {
    /// Follow the system's proxy settings (the default).
    System,
    /// No proxy: every connection goes direct.
    Off,
    /// This one proxy for every node.
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
fn platform() -> Option<SystemProxies> {
    let p = zikaron_os::system_proxies()?;
    Some(SystemProxies { https: p.https, http: p.http, socks: p.socks, exceptions: p.exceptions, auto_config: p.auto_config })
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

/// The route a new connection to a target takes.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Way {
    Direct,
    Through(Proxy),
}

/// A route decision for display and logging: how a new connection goes now, and why it goes direct when the
/// reason is not the setting itself.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Reading {
    pub way: Way,
    pub choice: Choice,
    /// The node is this machine: always reached directly.
    pub loopback: bool,
    /// The system was followed and uses an automatic configuration script (unsupported).
    pub auto_config_ignored: bool,
    /// The system was followed but its settings were not read in time (or could not be read): this connection
    /// went as if there were no proxy.
    pub system_unread: bool,
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

/// The route for a target under a setting and the system settings (pure; [`way_for`] uses the current ones;
/// `system` is `None` when not read in time). Loopback always goes direct; an excepted host goes direct when
/// following the system; https uses the system's https proxy, else SOCKS; http uses its http proxy, else SOCKS.
pub fn way_under(t: &Target, choice: &Choice, system: Option<&SystemProxies>) -> Reading {
    let reading = |way: Way| Reading { way, choice: choice.clone(), loopback: false, auto_config_ignored: false, system_unread: false };
    if is_loopback(&t.host) {
        return Reading { loopback: true, ..reading(Way::Direct) };
    }
    match choice {
        Choice::Off => reading(Way::Direct),
        Choice::Manual(p) => reading(Way::Through(p.clone())),
        Choice::System => {
            let Some(s) = system else { return Reading { system_unread: true, ..reading(Way::Direct) } };
            let explicit = s.https.is_some() || s.http.is_some() || s.socks.is_some();
            if !explicit {
                return Reading { auto_config_ignored: s.auto_config, ..reading(Way::Direct) };
            }
            if excepted(t, &s.exceptions) {
                return reading(Way::Direct);
            }
            let pick = match t.scheme {
                Scheme::Https => s.https.as_ref().map(|p| (Kind::Http, p)).or(s.socks.as_ref().map(|p| (Kind::Socks5, p))),
                Scheme::Http => s.http.as_ref().map(|p| (Kind::Http, p)).or(s.socks.as_ref().map(|p| (Kind::Socks5, p))),
            };
            match pick {
                Some((kind, (host, port))) => reading(Way::Through(Proxy { kind, host: proxy_host(host), port: *port })),
                None => reading(Way::Direct),
            }
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
/// the system settings, waited for at most `wait`.
pub fn way_for(t: &Target, wait: Duration) -> Reading {
    let choice = choice_now();
    let sys = match &choice {
        Choice::System if !is_loopback(&t.host) => match system_now(wait, false) {
            Sys::Read(p) => Some(p),
            Sys::Unread => None,
        },
        _ => None,
    };
    way_under(t, &choice, sys.as_ref())
}

/// How a new connection to this target would go, for display: never waits (the last system read will do while
/// a fresh one runs); `system_unread` when none has been read yet.
pub fn way_shown(t: &Target) -> Reading {
    let choice = choice_now();
    let sys = match &choice {
        Choice::System if !is_loopback(&t.host) => match system_now(Duration::ZERO, true) {
            Sys::Read(p) => Some(p),
            Sys::Unread => None,
        },
        _ => None,
    };
    way_under(t, &choice, sys.as_ref())
}

/// A failure on the proxy leg, reported as a connection failure naming the proxy. Deadline and shutdown
/// failures pass through unchanged.
fn through(p: &Proxy, f: Fail) -> Fail {
    match f {
        Fail::Late(_) | Fail::Closed => f,
        Fail::Name(x) | Fail::Connect(x) | Fail::Handshake(x) | Fail::Certificate(x) | Fail::Stream(x) => {
            Fail::Connect(format!("via proxy {}: {x}", p.spelled()))
        }
        Fail::Overlong(n) => Fail::Connect(format!("via proxy {}: its answer passed {n} bytes", p.spelled())),
    }
}

/// Maximum proxy response head size (a CONNECT response is a few lines).
const MAX_PROXY_HEAD: usize = 16 << 10;

/// Open a tunnel to the target through a proxy: connect to the proxy (every address, one deadline), track the
/// socket for shutdown, then send CONNECT or the SOCKS5 request. The tunnel is a plain TCP stream to the
/// target's port; TLS for https targets is the caller's job.
pub(crate) fn tunnel(t: &Target, p: &Proxy, clock: &Clock) -> Result<(TcpStream, keep::Ticket), Fail> {
    let mut tcp = dial::connect(&p.target(), clock).map_err(|f| through(p, f))?;
    let ticket = keep::ticket(&tcp)?;
    let opened = match p.kind {
        Kind::Http => connect_tunnel(&mut tcp, t, clock),
        Kind::Socks5 => socks_tunnel(&mut tcp, t, clock),
    };
    // Shut down meanwhile: closed, counted once (a wait inside that found the flag already counted its cut).
    if keep::closing() {
        return Err(match opened {
            Err(Fail::Closed) => Fail::Closed,
            _ => keep::cut(),
        });
    }
    opened.map_err(|f| through(p, f))?;
    Ok((tcp, ticket))
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
fn read_exact(tcp: &mut TcpStream, buf: &mut [u8], clock: &Clock) -> Result<(), Fail> {
    let mut at = 0;
    while at < buf.len() {
        clock.bound(tcp)?;
        let got = tcp.read(&mut buf[at..]);
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

/// HTTP CONNECT: send `CONNECT host:port` and read the response head; a 2xx opens the tunnel, anything else is
/// the proxy's refusal with its status line.
fn connect_tunnel(tcp: &mut TcpStream, t: &Target, clock: &Clock) -> Result<(), Fail> {
    let place = format!("{}:{}", t.host, t.port);
    clock.bound(tcp)?;
    tcp.write_all(crate::request_head("CONNECT", &place, &place, "").as_bytes()).map_err(|e| io(e, clock))?;
    // Read byte by byte to the end of the head, so no tunnel data is consumed.
    let mut head: Vec<u8> = Vec::new();
    let mut one = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() > MAX_PROXY_HEAD {
            return Err(Fail::Stream("the proxy's answer has no end of head".into()));
        }
        read_exact(tcp, &mut one, clock)?;
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
        Some(407) => Err(Fail::Stream(format!("CONNECT {place} answered {line} (a proxy that needs credentials is not supported)"))),
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

/// SOCKS5 without authentication: the greeting, then CONNECT to the host by name (an address literal in its
/// own address type), then read the full reply; anything but success is the proxy's refusal with its code.
fn socks_tunnel(tcp: &mut TcpStream, t: &Target, clock: &Clock) -> Result<(), Fail> {
    clock.bound(tcp)?;
    tcp.write_all(&[5, 1, 0]).map_err(|e| io(e, clock))?;
    let mut pick = [0u8; 2];
    read_exact(tcp, &mut pick, clock)?;
    match pick {
        [5, 0] => {}
        [5, 0xFF] => return Err(Fail::Stream("SOCKS5 needs authentication (not supported)".into())),
        [5, m] => return Err(Fail::Stream(format!("SOCKS5 picked method {m}, which was not offered"))),
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
    clock.bound(tcp)?;
    tcp.write_all(&ask).map_err(|e| io(e, clock))?;
    let mut reply = [0u8; 4];
    read_exact(tcp, &mut reply, clock)?;
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
            read_exact(tcp, &mut n, clock)?;
            n[0] as usize + 2
        }
        _ => return Err(Fail::Stream("the SOCKS5 reply does not read".into())),
    };
    let mut bound = vec![0u8; rest];
    read_exact(tcp, &mut bound, clock)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(url: &str) -> Target {
        crate::parse(url).expect("an address")
    }

    /// A manual proxy is `http://host:port` or `socks5://host:port`; credentials, other schemes, a missing
    /// port, a path and an empty host are refused by name.
    #[test]
    fn a_typed_proxy_is_read_or_refused_by_name() {
        assert_eq!(proxy_of("http://127.0.0.1:7890"), Ok(Proxy { kind: Kind::Http, host: "127.0.0.1".into(), port: 7890 }));
        assert_eq!(proxy_of(" SOCKS5://Proxy.Example:1080/ ").map(|p| p.spelled()), Ok("socks5://proxy.example:1080".into()));
        assert_eq!(proxy_of("socks5://[::1]:1080").map(|p| p.host), Ok("[::1]".into()));
        assert_eq!(proxy_of("http://user:pw@proxy.example:8080"), Err(NotAProxy::Credentials));
        assert_eq!(proxy_of("socks5://u@h:1"), Err(NotAProxy::Credentials));
        for bad in ["https://proxy.example:443", "proxy.example:8080", "http://proxy.example", "http://proxy.example:", "http://:8080", "http://proxy.example:8080/x", "http://proxy.example:0", "http://proxy.example:65536", "socks4://h:1", "http://[::1]"] {
            assert_eq!(proxy_of(bad), Err(NotAProxy::Shape), "{bad}");
        }
    }

    /// Loopback always goes direct; following the system uses its https, http and SOCKS proxies by target
    /// scheme, honours its exceptions, and uses no proxy when it gives none or only a PAC script.
    #[test]
    fn the_way_follows_the_choice_and_the_system() {
        let manual = Choice::Manual(Proxy { kind: Kind::Socks5, host: "p".into(), port: 1 });
        let sys = SystemProxies { https: Some(("hp".into(), 8443)), http: Some(("p".into(), 8080)), socks: Some(("s".into(), 1080)), exceptions: vec!["*.corp.example".into(), "<local>".into(), "10.0.0.0/8".into()], auto_config: false };
        for host in ["http://localhost:8545", "http://127.0.0.9:1", "http://[::1]:1", "https://a.localhost/"] {
            for c in [Choice::System, Choice::Off, manual.clone()] {
                assert_eq!(way_under(&t(host), &c, Some(&sys)).way, Way::Direct, "{host} {c:?}");
            }
        }
        assert_eq!(way_under(&t("https://node.example/"), &Choice::Off, Some(&sys)).way, Way::Direct);
        assert_eq!(way_under(&t("https://node.example/"), &manual, Some(&sys)).way, Way::Through(Proxy { kind: Kind::Socks5, host: "p".into(), port: 1 }));
        let through = |url: &str, s: &SystemProxies| match way_under(&t(url), &Choice::System, Some(s)).way {
            Way::Through(p) => p.spelled(),
            Way::Direct => "direct".into(),
        };
        assert_eq!(through("https://node.example/", &sys), "http://hp:8443");
        assert_eq!(through("http://node.example/", &sys), "http://p:8080");
        assert_eq!(through("https://a.corp.example/", &sys), "direct");
        assert_eq!(through("https://corp.example/", &sys), "direct");
        assert_eq!(through("https://intranet/", &sys), "direct");
        assert_eq!(through("https://10.1.2.3/", &sys), "direct", "a range entry is read");
        let socks_only = SystemProxies { socks: Some(("s".into(), 1080)), ..Default::default() };
        assert_eq!(through("https://node.example/", &socks_only), "socks5://s:1080");
        assert_eq!(through("http://node.example/", &socks_only), "socks5://s:1080");
        let pac = SystemProxies { auto_config: true, ..Default::default() };
        let r = way_under(&t("https://node.example/"), &Choice::System, Some(&pac));
        assert_eq!((r.way, r.auto_config_ignored), (Way::Direct, true));
        assert_eq!(way_under(&t("https://node.example/"), &Choice::System, None).way, Way::Direct);
        let r = way_under(&t("https://node.example/"), &Choice::System, None);
        assert_eq!((r.way, r.system_unread), (Way::Direct, true));
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
        let any = Choice::Manual(Proxy { kind: Kind::Http, host: "p".into(), port: 1 });
        let r = way_under(&t("http://localhost.:8545/"), &any, None);
        assert_eq!((r.way, r.loopback), (Way::Direct, true));
    }

    /// A system proxy given as a bare IPv6 address is bracketed when spelled and unbracketed when dialed.
    #[test]
    fn a_system_proxy_on_an_ipv6_address_is_bracketed() {
        let sys = SystemProxies { https: Some(("::1".into(), 7890)), http: Some(("FE80::2".into(), 8080)), ..Default::default() };
        let via = |url: &str| match way_under(&t(url), &Choice::System, Some(&sys)).way {
            Way::Through(p) => (p.spelled(), p.target().dial_host().to_string()),
            Way::Direct => (String::new(), String::new()),
        };
        assert_eq!(via("https://node.example/"), ("http://[::1]:7890".into(), "::1".into()));
        assert_eq!(via("http://node.example/"), ("http://[fe80::2]:8080".into(), "fe80::2".into()));
    }
}
