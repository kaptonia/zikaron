use crate::route::{proxy_of, Kind, SystemProxies, Way};
use crate::{keep, Target};
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const MEMORY_LIMIT: usize = 8 << 20;
pub const STACK_LIMIT: usize = 512 << 10;
pub const RUN_LIMIT: Duration = Duration::from_secs(2);
pub const DNS_LIMIT: Duration = Duration::from_secs(1);
pub const FETCH_LIMIT: Duration = Duration::from_secs(5);
pub const SCRIPT_MAX: usize = 1 << 20;
pub const SCRIPT_FRESH: Duration = Duration::from_secs(300);
pub const ANSWER_FRESH: Duration = Duration::from_secs(60);
pub const PAC_FUNCTIONS: [&str; 21] = [
    "isPlainHostName",
    "dnsDomainIs",
    "localHostOrDomainIs",
    "isResolvable",
    "isInNet",
    "dnsResolve",
    "myIpAddress",
    "dnsDomainLevels",
    "shExpMatch",
    "weekdayRange",
    "dateRange",
    "timeRange",
    "isResolvableEx",
    "isInNetEx",
    "dnsResolveEx",
    "myIpAddressEx",
    "sortIpAddressList",
    "getClientVersion",
    "FindProxyForURL",
    "FindProxyForURLEx",
    "alert",
];

pub const LANGUAGE_GLOBALS: [&str; 34] = [
    "Object",
    "Function",
    "Array",
    "Number",
    "Boolean",
    "String",
    "Symbol",
    "Math",
    "JSON",
    "Date",
    "RegExp",
    "Error",
    "EvalError",
    "RangeError",
    "ReferenceError",
    "SyntaxError",
    "TypeError",
    "URIError",
    "InternalError",
    "AggregateError",
    "parseInt",
    "parseFloat",
    "isNaN",
    "isFinite",
    "decodeURI",
    "decodeURIComponent",
    "encodeURI",
    "encodeURIComponent",
    "escape",
    "unescape",
    "NaN",
    "Infinity",
    "undefined",
    "globalThis",
];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum PacTrouble {
    Unfetched,
    Scheme,
    RemoteHttp,
    TooLarge,
    Failed,
    Timeout,
    Memory,
    NoFunction,
    Answer,
}

impl PacTrouble {
    pub const ALL: [PacTrouble; 9] = [
        PacTrouble::Unfetched,
        PacTrouble::Scheme,
        PacTrouble::RemoteHttp,
        PacTrouble::TooLarge,
        PacTrouble::Failed,
        PacTrouble::Timeout,
        PacTrouble::Memory,
        PacTrouble::NoFunction,
        PacTrouble::Answer,
    ];

    pub fn code(self) -> &'static str {
        match self {
            PacTrouble::Unfetched => "PAC_UNFETCHED",
            PacTrouble::Scheme => "PAC_SCHEME",
            PacTrouble::RemoteHttp => "PAC_REMOTE_HTTP",
            PacTrouble::TooLarge => "PAC_TOO_LARGE",
            PacTrouble::Failed => "PAC_FAILED",
            PacTrouble::Timeout => "PAC_TIMEOUT",
            PacTrouble::Memory => "PAC_MEMORY",
            PacTrouble::NoFunction => "PAC_NO_FUNCTION",
            PacTrouble::Answer => "PAC_ANSWER",
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Auto {
    None,
    Ways(Vec<Way>),
    Pending,
    Trouble(PacTrouble),
}

pub fn ways_of(answer: &str) -> Result<Vec<Way>, PacTrouble> {
    let mut out: Vec<Way> = Vec::new();
    for entry in answer.split(';') {
        let mut words = entry.split_whitespace();
        let Some(word) = words.next() else { continue };
        let place = words.next();
        if words.next().is_some() {
            continue;
        }
        let (kinds, port): (&[Kind], u16) = match word.to_ascii_uppercase().as_str() {
            "DIRECT" if place.is_none() => {
                if !out.contains(&Way::Direct) {
                    out.push(Way::Direct);
                }
                continue;
            }
            "PROXY" | "HTTP" => (&[Kind::Http], 80),
            "HTTPS" => (&[Kind::Https], 443),
            "SOCKS" => (&[Kind::Socks5, Kind::Socks4], 1080),
            "SOCKS5" => (&[Kind::Socks5], 1080),
            "SOCKS4" => (&[Kind::Socks4], 1080),
            _ => continue,
        };
        let Some(place) = place else { continue };
        for kind in kinds {
            let scheme = match kind {
                Kind::Http => "http",
                Kind::Https => "https",
                Kind::Socks5 => "socks5",
                Kind::Socks4 => "socks4",
            };
            let host_part = place.rsplit_once('@').map_or(place, |(_, h)| h);
            let has_port = host_part.rsplit_once(':').is_some_and(|(h, p)| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()) && (!h.contains(':') || h.ends_with(']')));
            let spelled = if has_port { format!("{scheme}://{place}") } else { format!("{scheme}://{place}:{port}") };
            let spelled = match (kind, spelled.split_once("://").and_then(|(_, r)| r.rsplit_once('@'))) {
                (Kind::Socks4, Some((creds, rest))) if creds.contains(':') => format!("{scheme}://{}@{rest}", creds.split(':').next().unwrap_or("")),
                _ => spelled,
            };
            if let Ok(p) = proxy_of(&spelled) {
                let w = Way::Through(p);
                if !out.contains(&w) {
                    out.push(w);
                }
            }
        }
    }
    if out.is_empty() {
        return Err(PacTrouble::Answer);
    }
    Ok(out)
}

#[derive(Clone, Copy)]
pub(crate) struct Host {
    pub resolve: fn(&str, Duration) -> Vec<IpAddr>,
    pub my_ips: fn() -> Vec<IpAddr>,
    pub stop: fn() -> bool,
}

fn my_ips() -> Vec<IpAddr> {
    let mut out = Vec::new();
    for (bind, to) in [("0.0.0.0:0", "8.8.8.8:80"), ("[::]:0", "[2001:4860:4860::8888]:80")] {
        if let Ok(s) = std::net::UdpSocket::bind(bind) {
            if s.connect(to).is_ok() {
                if let Ok(a) = s.local_addr() {
                    if !a.ip().is_unspecified() {
                        out.push(a.ip());
                    }
                }
            }
        }
    }
    if out.is_empty() {
        out.push(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST));
    }
    out
}

const MACHINE: Host = Host { resolve: crate::dial::lookup, my_ips, stop: keep::closing };

const LIBRARY: &str = r#"
(function (g) {
  var MONTHS = ['JAN','FEB','MAR','APR','MAY','JUN','JUL','AUG','SEP','OCT','NOV','DEC'];
  var DAYS = ['SUN','MON','TUE','WED','THU','FRI','SAT'];
  function v4(s) {
    var m = /^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/.exec(String(s));
    if (!m) return null;
    var n = 0;
    for (var i = 1; i <= 4; i++) { var o = Number(m[i]); if (o > 255) return null; n = n * 256 + o; }
    return n;
  }
  function gmtLast(args) {
    var a = Array.prototype.slice.call(args);
    var gmt = a.length > 0 && String(a[a.length - 1]).toUpperCase() === 'GMT';
    if (gmt) a.pop();
    return { a: a, now: new Date(), gmt: gmt };
  }
  // The lookups answer `null` when a name does not resolve.
  var rawResolve = g.dnsResolve;
  g.dnsResolve = function (host) { var r = rawResolve(host); return r === undefined ? null : r; };
  g.isPlainHostName = function (host) { return String(host).indexOf('.') < 0; };
  g.dnsDomainIs = function (host, domain) {
    host = String(host).toLowerCase(); domain = String(domain).toLowerCase();
    return host.length >= domain.length && host.substring(host.length - domain.length) === domain;
  };
  g.localHostOrDomainIs = function (host, hostdom) {
    host = String(host).toLowerCase(); hostdom = String(hostdom).toLowerCase();
    return host === hostdom || (host.indexOf('.') < 0 && hostdom.indexOf(host + '.') === 0);
  };
  g.isResolvable = function (host) { return g.dnsResolve(host) !== null; };
  g.isResolvableEx = function (host) { var r = g.dnsResolveEx(host); return r !== null && r !== ''; };
  g.dnsDomainLevels = function (host) { return String(host).split('.').length - 1; };
  g.isInNet = function (host, pattern, mask) {
    var ip = v4(host) === null ? g.dnsResolve(host) : String(host);
    if (ip === null) return false;
    var a = v4(ip), p = v4(pattern), m = v4(mask);
    if (a === null || p === null || m === null) return false;
    var hi = function (x) { return Math.floor(x / 65536); }, lo = function (x) { return x % 65536; };
    return (hi(a) & hi(m)) === (hi(p) & hi(m)) && (lo(a) & lo(m)) === (lo(p) & lo(m));
  };
  g.shExpMatch = function (str, shexp) {
    var re = String(shexp).replace(/[.+^${}()|[\]\\\/]/g, '\\$&').replace(/\*/g, '.*').replace(/\?/g, '.');
    return new RegExp('^' + re + '$').test(String(str));
  };
  g.weekdayRange = function () {
    var x = gmtLast(arguments);
    if (x.a.length < 1 || x.a.length > 2) return false;
    var w1 = DAYS.indexOf(String(x.a[0]).toUpperCase());
    var w2 = x.a.length > 1 ? DAYS.indexOf(String(x.a[1]).toUpperCase()) : w1;
    if (w1 < 0 || w2 < 0) return false;
    var d = x.gmt ? x.now.getUTCDay() : x.now.getDay();
    return w1 <= w2 ? (w1 <= d && d <= w2) : (d >= w1 || d <= w2);
  };
  g.dateRange = function () {
    var x = gmtLast(arguments), a = x.a;
    if (a.length < 1 || a.length > 6) return false;
    var parts = [];
    for (var i = 0; i < a.length; i++) {
      var mi = MONTHS.indexOf(String(a[i]).toUpperCase());
      if (mi >= 0) { parts.push(['m', mi]); continue; }
      var n = Number(a[i]);
      if (isNaN(n) || n !== Math.floor(n) || n < 1) return false;
      parts.push(n > 31 ? ['y', n] : ['d', n]);
    }
    var now = { d: x.gmt ? x.now.getUTCDate() : x.now.getDate(), m: x.gmt ? x.now.getUTCMonth() : x.now.getMonth(), y: x.gmt ? x.now.getUTCFullYear() : x.now.getFullYear() };
    function key(ps) {
      var k = { y: null, m: null, d: null };
      for (var j = 0; j < ps.length; j++) { if (k[ps[j][0]] !== null) return null; k[ps[j][0]] = ps[j][1]; }
      if (k.y !== null && k.d !== null && k.m === null) return null;
      return k;
    }
    function num(k, src) { return (k.y !== null ? src.y : 0) * 10000 + (k.m !== null ? src.m : 0) * 100 + (k.d !== null ? src.d : 0); }
    if (parts.length === 1) { var p = parts[0]; return now[p[0]] === p[1]; }
    if (parts.length % 2) return false;
    var lo = key(parts.slice(0, parts.length / 2)), hi = key(parts.slice(parts.length / 2));
    if (lo === null || hi === null || (lo.y === null) !== (hi.y === null) || (lo.m === null) !== (hi.m === null) || (lo.d === null) !== (hi.d === null)) return false;
    var l = num(lo, lo), h = num(hi, hi), n = num(lo, now);
    return l <= h ? (l <= n && n <= h) : (n >= l || n <= h);
  };
  g.timeRange = function () {
    var x = gmtLast(arguments), a = x.a;
    for (var i = 0; i < a.length; i++) { a[i] = Number(a[i]); if (isNaN(a[i])) return false; }
    var h = x.gmt ? x.now.getUTCHours() : x.now.getHours(), mi = x.gmt ? x.now.getUTCMinutes() : x.now.getMinutes(), s = x.gmt ? x.now.getUTCSeconds() : x.now.getSeconds();
    var l, hi, n;
    if (a.length === 1) return h === a[0];
    if (a.length === 2) { l = a[0] * 3600; hi = a[1] * 3600; n = h * 3600 + mi * 60 + s; return l <= hi ? (l <= n && n < hi) : (n >= l || n < hi); }
    if (a.length === 4) { l = a[0] * 3600 + a[1] * 60; hi = a[2] * 3600 + a[3] * 60; n = h * 3600 + mi * 60 + s; return l <= hi ? (l <= n && n < hi) : (n >= l || n < hi); }
    if (a.length === 6) { l = a[0] * 3600 + a[1] * 60 + a[2]; hi = a[3] * 3600 + a[4] * 60 + a[5]; n = h * 3600 + mi * 60 + s; return l <= hi ? (l <= n && n <= hi) : (n >= l || n <= hi); }
    return false;
  };
  g.sortIpAddressList = function (list) {
    var items = String(list).split(';').map(function (s) { return s.trim(); }).filter(function (s) { return s !== ''; });
    if (items.length === 0) return false;
    var keyed = [];
    for (var i = 0; i < items.length; i++) {
      var k = g.isInNetEx(items[i], '::/0') ? '0' : (g.isInNetEx(items[i], '0.0.0.0/0') ? '1' : null);
      if (k === null) return false;
      keyed.push([k, items[i]]);
    }
    keyed.sort(function (p, q) { return p[0] < q[0] ? -1 : p[0] > q[0] ? 1 : 0; });
    return keyed.map(function (p) { return p[1]; }).join(';');
  };
  g.getClientVersion = function () { return '1.0'; };
  g.alert = function () {};
})(globalThis);
"#;

fn in_net_ex(ip: &str, prefix: &str) -> bool {
    let (Ok(a), Some((net, bits))) = (ip.trim().parse::<IpAddr>(), prefix.trim().split_once('/')) else { return false };
    let (Ok(n), Ok(bits)) = (net.parse::<IpAddr>(), bits.parse::<u32>()) else { return false };
    let mask = |w: u32| if bits == 0 { 0u128 } else { u128::MAX << (w - bits) };
    match (a, n) {
        (IpAddr::V4(a), IpAddr::V4(n)) if bits <= 32 => (u32::from(a) as u128 & mask(32)) == (u32::from(n) as u128 & mask(32)),
        (IpAddr::V6(a), IpAddr::V6(n)) if bits <= 128 => (u128::from(a) & mask(128)) == (u128::from(n) & mask(128)),
        _ => false,
    }
}

fn run_in_engine(script: &str, url: &str, host: &str, until: Instant, machine: Host) -> Result<String, PacTrouble> {
    use rquickjs::context::intrinsic;
    use rquickjs::{Coerced, Context, Function, Runtime, Value};
    use std::sync::atomic::{AtomicBool, Ordering};
    let rt = Runtime::new().map_err(|_| PacTrouble::Memory)?;
    rt.set_memory_limit(MEMORY_LIMIT);
    rt.set_max_stack_size(STACK_LIMIT);
    let fired = Arc::new(AtomicBool::new(false));
    let seen = fired.clone();
    let halt = machine.stop;
    rt.set_interrupt_handler(Some(Box::new(move || {
        let stop = Instant::now() >= until || halt();
        if stop {
            seen.store(true, Ordering::SeqCst);
        }
        stop
    })));
    let ctx = Context::builder().with::<intrinsic::Date>().with::<intrinsic::Eval>().with::<intrinsic::RegExpCompiler>().with::<intrinsic::RegExp>().with::<intrinsic::Json>().build(&rt).map_err(|_| PacTrouble::Memory)?;
    let left = move || until.saturating_duration_since(Instant::now()).min(DNS_LIMIT);
    let first_v4 = |v: Vec<IpAddr>| v.into_iter().find(IpAddr::is_ipv4).map(|a| a.to_string());
    let joined = |v: Vec<IpAddr>| v.into_iter().map(|a| a.to_string()).collect::<Vec<_>>().join(";");
    let outcome: Result<String, PacTrouble> = ctx.with(|ctx| {
        let failed = |ctx: &rquickjs::Ctx<'_>| -> PacTrouble {
            let thrown = ctx.catch();
            let said = thrown.as_exception().and_then(|e| e.message()).unwrap_or_default();
            if fired.load(Ordering::SeqCst) {
                PacTrouble::Timeout
            } else if said.contains("out of memory") || said.contains("Maximum call stack size exceeded") {
                PacTrouble::Memory
            } else {
                PacTrouble::Failed
            }
        };
        let g = ctx.globals();
        let setup = || -> rquickjs::Result<()> {
            let resolve = machine.resolve;
            g.set("dnsResolve", Function::new(ctx.clone(), move |h: Coerced<String>| first_v4(resolve(&h.0, left())))?.with_name("dnsResolve")?)?;
            g.set("dnsResolveEx", Function::new(ctx.clone(), move |h: Coerced<String>| joined(resolve(&h.0, left())))?.with_name("dnsResolveEx")?)?;
            let mine = machine.my_ips;
            g.set("myIpAddress", Function::new(ctx.clone(), move || first_v4(mine()).unwrap_or_else(|| "127.0.0.1".into()))?.with_name("myIpAddress")?)?;
            g.set("myIpAddressEx", Function::new(ctx.clone(), move || joined(mine()))?.with_name("myIpAddressEx")?)?;
            g.set("isInNetEx", Function::new(ctx.clone(), |ip: Coerced<String>, prefix: Coerced<String>| in_net_ex(&ip.0, &prefix.0))?.with_name("isInNetEx")?)?;
            ctx.eval::<(), _>(LIBRARY)?;
            let names: Vec<String> = ctx.eval("Object.getOwnPropertyNames(globalThis)")?;
            for n in names {
                if !LANGUAGE_GLOBALS.contains(&n.as_str()) && !PAC_FUNCTIONS.contains(&n.as_str()) {
                    g.remove(n.as_str())?;
                }
            }
            Ok(())
        };
        setup().map_err(|_| failed(&ctx))?;
        ctx.eval::<(), _>(script).map_err(|_| failed(&ctx))?;
        let find = ["FindProxyForURLEx", "FindProxyForURL"].iter().find_map(|n| g.get::<_, Value>(*n).ok().and_then(|v| v.into_function())).ok_or(PacTrouble::NoFunction)?;
        let said: Value = find.call((url, host)).map_err(|_| failed(&ctx))?;
        said.as_string().and_then(|s| s.to_string().ok()).ok_or(PacTrouble::Answer)
    });
    outcome
}

pub(crate) fn run(script: Arc<String>, url: &str, host: &str, limit: Duration, machine: Host) -> Result<String, PacTrouble> {
    let until = Instant::now() + limit;
    let (tx, rx) = std::sync::mpsc::channel();
    let (url, host) = (url.to_string(), host.to_string());
    let started = std::thread::Builder::new().name("pac".into()).stack_size(4 << 20).spawn(move || {
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_in_engine(&script, &url, &host, until, machine))).unwrap_or(Err(PacTrouble::Failed));
        let _ = tx.send(r);
    });
    if started.is_err() {
        return Err(PacTrouble::Memory);
    }
    rx.recv_timeout(limit + crate::SLICE).unwrap_or(Err(PacTrouble::Timeout))
}

#[derive(Clone)]
enum Fetched {
    Script(Arc<String>),
    Trouble(PacTrouble),
}

pub fn script_source(t: &Target) -> Result<(), PacTrouble> {
    let host = t.host.trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    let this_machine = host == "localhost"
        || match host.parse::<IpAddr>() {
            Ok(IpAddr::V4(a)) => a.octets()[0] == 127,
            Ok(IpAddr::V6(a)) => a == std::net::Ipv6Addr::LOCALHOST,
            Err(_) => false,
        };
    match t.scheme {
        crate::Scheme::Https => Ok(()),
        crate::Scheme::Http if this_machine => Ok(()),
        crate::Scheme::Http => Err(PacTrouble::RemoteHttp),
    }
}

fn script_address(url: &str) -> Result<(), PacTrouble> {
    let lower = url.trim().to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err(PacTrouble::Scheme);
    }
    script_source(&crate::read_address(url.trim()).map_err(|_| PacTrouble::Scheme)?)
}

fn fetch(url: &str, limit: Duration) -> Fetched {
    let Ok(mut t) = crate::read_address(url) else { return Fetched::Trouble(PacTrouble::Scheme) };
    if let Err(x) = script_source(&t) {
        return Fetched::Trouble(x);
    }
    let until = Instant::now() + limit.min(FETCH_LIMIT);
    for _ in 0..=REDIRECTS {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        let limits = crate::Limits { deadline: left, max_answer: SCRIPT_MAX + (64 << 10) };
        match crate::get_direct(&t, &limits) {
            Ok(a) if a.status == 200 && a.body.len() > SCRIPT_MAX => return Fetched::Trouble(PacTrouble::TooLarge),
            Ok(a) if a.status == 200 => return Fetched::Script(Arc::new(String::from_utf8_lossy(&a.body).into_owned())),
            Ok(a) if matches!(a.status, 301 | 302 | 303 | 307 | 308) => match a.location.as_deref().and_then(|l| redirected(&t, l)) {
                Some(next) => match script_source(&next) {
                    Ok(()) => t = next,
                    Err(x) => return Fetched::Trouble(x),
                },
                None => return Fetched::Trouble(PacTrouble::Unfetched),
            },
            Ok(_) => return Fetched::Trouble(PacTrouble::Unfetched),
            Err(crate::Fail::Overlong(_)) => return Fetched::Trouble(PacTrouble::TooLarge),
            Err(_) => return Fetched::Trouble(PacTrouble::Unfetched),
        }
    }
    Fetched::Trouble(PacTrouble::Unfetched)
}

pub const REDIRECTS: usize = 5;

fn redirected(from: &Target, location: &str) -> Option<Target> {
    crate::read_address(&resolve_reference(from, location.trim())?).ok()
}

fn resolve_reference(base: &Target, reference: &str) -> Option<String> {
    let reference = reference.split_once('#').map_or(reference, |(r, _)| r);
    let (base_path, base_query) = match base.path.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (base.path.as_str(), None),
    };
    let scheme = reference.split_once(':').map(|(s, _)| s).filter(|s| {
        let mut c = s.chars();
        c.next().is_some_and(|f| f.is_ascii_alphabetic()) && c.all(|x| x.is_ascii_alphanumeric() || "+-.".contains(x))
    });
    let split = |rest: &str| -> (String, Option<String>) {
        match rest.split_once('?') {
            Some((p, q)) => (p.to_string(), Some(q.to_string())),
            None => (rest.to_string(), None),
        }
    };
    let (scheme, authority, path, query) = if let Some(sc) = scheme {
        let rest = &reference[sc.len() + 1..];
        let rest = rest.strip_prefix("//")?;
        let cut = rest.find(['/', '?']).unwrap_or(rest.len());
        let (p, q) = split(&rest[cut..]);
        (sc.to_ascii_lowercase(), rest[..cut].to_string(), remove_dot_segments(&p), q)
    } else if let Some(rest) = reference.strip_prefix("//") {
        let cut = rest.find(['/', '?']).unwrap_or(rest.len());
        let (p, q) = split(&rest[cut..]);
        (base.scheme.as_str().to_string(), rest[..cut].to_string(), remove_dot_segments(&p), q)
    } else {
        let (p, q) = split(reference);
        let (path, query) = if p.is_empty() {
            (base_path.to_string(), q.or(base_query.map(str::to_string)))
        } else if p.starts_with('/') {
            (remove_dot_segments(&p), q)
        } else {
            let merged = match base_path.rfind('/') {
                Some(i) => format!("{}{p}", &base_path[..=i]),
                None => format!("/{p}"),
            };
            (remove_dot_segments(&merged), q)
        };
        (base.scheme.as_str().to_string(), base.authority(), path, query)
    };
    let path = if path.is_empty() { "/".to_string() } else { path };
    Some(format!("{scheme}://{authority}{path}{}", query.map(|q| format!("?{q}")).unwrap_or_default()))
}

fn remove_dot_segments(path: &str) -> String {
    let mut input = path.to_string();
    let mut output = String::new();
    while !input.is_empty() {
        if let Some(r) = input.strip_prefix("../") {
            input = r.to_string();
        } else if let Some(r) = input.strip_prefix("./") {
            input = r.to_string();
        } else if input.starts_with("/./") {
            input = input[2..].to_string();
        } else if input == "/." {
            input = "/".to_string();
        } else if input.starts_with("/../") || input == "/.." {
            input = if input == "/.." { "/".to_string() } else { input[3..].to_string() };
            match output.rfind('/') {
                Some(i) => output.truncate(i),
                None => output.clear(),
            }
        } else if input == "." || input == ".." {
            input.clear();
        } else {
            let start = usize::from(input.starts_with('/'));
            let end = input[start..].find('/').map_or(input.len(), |i| i + start);
            output.push_str(&input[..end]);
            input = input[end..].to_string();
        }
    }
    output
}

struct Kept {
    scripts: Vec<(String, Instant, Fetched)>,
    answers: Vec<((u64, String), Instant, Result<Vec<Way>, PacTrouble>)>,
    fetching: Vec<String>,
    asking: Vec<(u64, String)>,
}

static KEPT: Mutex<Kept> = Mutex::new(Kept { scripts: Vec::new(), answers: Vec::new(), fetching: Vec::new(), asking: Vec::new() });
static LANDED: std::sync::Condvar = std::sync::Condvar::new();

fn kept() -> std::sync::MutexGuard<'static, Kept> {
    KEPT.lock().unwrap_or_else(|e| e.into_inner())
}

pub fn forget() {
    let mut k = kept();
    k.scripts.clear();
    k.answers.clear();
}

fn kept_script(url: &str) -> Option<Fetched> {
    kept().scripts.iter().find(|(u, at, _)| u == url && at.elapsed() < SCRIPT_FRESH).map(|(_, _, f)| f.clone())
}

fn fresh_script(k: &Kept, url: &str) -> Option<Fetched> {
    k.scripts.iter().find(|(u, at, _)| u == url && at.elapsed() < SCRIPT_FRESH).map(|(_, _, f)| f.clone())
}

fn fresh_answer(k: &Kept, key: &(u64, String)) -> Option<Result<Vec<Way>, PacTrouble>> {
    k.answers.iter().find(|(x, at, _)| x == key && at.elapsed() < ANSWER_FRESH).map(|(_, _, a)| a.clone())
}

fn wait_kept<T>(until: Instant, found: impl Fn(&Kept) -> Option<T>, start: impl FnOnce(&mut Kept) -> bool) -> Option<T> {
    let mut k = kept();
    let mut start = Some(start);
    loop {
        if let Some(t) = found(&k) {
            return Some(t);
        }
        if let Some(s) = start.take() {
            if !s(&mut k) {
                return None;
            }
        }
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() || keep::closing() {
            return None;
        }
        k = LANDED.wait_timeout(k, left.min(crate::SLICE)).unwrap_or_else(|e| e.into_inner()).0;
    }
}

fn script_at(url: &str, limit: Duration) -> Fetched {
    let until = Instant::now() + limit;
    let owned = url.to_string();
    let got = wait_kept(
        until,
        |k| fresh_script(k, url),
        |k| {
            if k.fetching.contains(&owned) {
                return true;
            }
            k.fetching.push(owned.clone());
            let u = owned.clone();
            let started = std::thread::Builder::new().name("pac-fetch".into()).spawn(move || {
                let f = fetch(&u, FETCH_LIMIT);
                let mut k = kept();
                k.fetching.retain(|x| *x != u);
                k.scripts.retain(|(x, _, _)| *x != u);
                k.scripts.push((u, Instant::now(), f));
                LANDED.notify_all();
            });
            if started.is_err() {
                k.fetching.retain(|x| *x != owned);
                return false;
            }
            true
        },
    );
    got.unwrap_or(Fetched::Trouble(PacTrouble::Unfetched))
}

fn digest(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

fn asked_url(t: &Target) -> String {
    format!("{}/", t.sayable())
}

fn kept_answer(key: &(u64, String)) -> Option<Result<Vec<Way>, PacTrouble>> {
    fresh_answer(&kept(), key)
}

fn answer(script: &Arc<String>, t: &Target, limit: Duration, machine: Host) -> Result<Vec<Way>, PacTrouble> {
    let key = (digest(script), asked_url(t));
    let until = Instant::now() + limit;
    let host = t.dial_host().to_string();
    let got = wait_kept(
        until,
        |k| fresh_answer(k, &key),
        |k| {
            if k.asking.contains(&key) {
                return true;
            }
            k.asking.push(key.clone());
            let (key2, script2) = (key.clone(), script.clone());
            let started = std::thread::Builder::new().name("pac-ask".into()).spawn(move || {
                let a = run(script2, &key2.1, &host, RUN_LIMIT, machine).and_then(|s| ways_of(&s));
                let mut k = kept();
                k.asking.retain(|x| *x != key2);
                k.answers.retain(|(x, at, _)| *x != key2 && at.elapsed() < ANSWER_FRESH);
                if k.answers.len() >= 256 {
                    drop(k.answers.remove(0));
                }
                k.answers.push((key2, Instant::now(), a));
                LANDED.notify_all();
            });
            if started.is_err() {
                k.asking.retain(|x| *x != key);
                return false;
            }
            true
        },
    );
    got.unwrap_or(Err(PacTrouble::Timeout))
}

fn auto_of(a: Result<Vec<Way>, PacTrouble>) -> Auto {
    match a {
        Ok(w) => Auto::Ways(w),
        Err(t) => Auto::Trouble(t),
    }
}

pub(crate) fn auto_for(t: &Target, s: &SystemProxies, limit: Duration) -> Auto {
    auto_with(t, s, limit, MACHINE)
}

pub(crate) fn auto_with(t: &Target, s: &SystemProxies, limit: Duration, machine: Host) -> Auto {
    let until = Instant::now() + limit;
    let left = || until.saturating_duration_since(Instant::now());
    let Some(url) = s.pac_url.as_deref() else { return Auto::None };
    if let Err(x) = script_address(url) {
        return Auto::Trouble(x);
    }
    match script_at(url.trim(), left()) {
        Fetched::Script(sc) => auto_of(answer(&sc, t, left(), machine)),
        Fetched::Trouble(x) => Auto::Trouble(x),
    }
}

pub(crate) fn auto_known(t: &Target, s: &SystemProxies) -> Auto {
    let Some(url) = s.pac_url.as_deref() else { return Auto::None };
    if let Err(x) = script_address(url) {
        return Auto::Trouble(x);
    }
    match kept_script(url.trim()) {
        None => Auto::Pending,
        Some(Fetched::Script(sc)) => kept_answer(&(digest(&sc), asked_url(t))).map(auto_of).unwrap_or(Auto::Pending),
        Some(Fetched::Trouble(x)) => Auto::Trouble(x),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(host: &str, _: Duration) -> Vec<IpAddr> {
        match host {
            "node.example" => vec!["93.184.216.34".parse().unwrap(), "2606:2800:220:1::1".parse().unwrap()],
            "intranet.corp" => vec!["10.1.2.3".parse().unwrap()],
            _ => Vec::new(),
        }
    }

    fn slow_names(_: &str, limit: Duration) -> Vec<IpAddr> {
        std::thread::sleep(limit);
        Vec::new()
    }

    fn mine() -> Vec<IpAddr> {
        vec!["192.168.1.20".parse().unwrap(), "fe80::1".parse().unwrap()]
    }

    fn never() -> bool {
        false
    }

    const STAGED: Host = Host { resolve: names, my_ips: mine, stop: never };

    fn ask(script: &str, url: &str, host: &str) -> Result<String, PacTrouble> {
        run(Arc::new(script.to_string()), url, host, RUN_LIMIT, STAGED)
    }

    fn says(body: &str) -> String {
        ask(&format!("function FindProxyForURL(url, host) {{ return String({body}); }}"), "https://node.example/", "node.example").expect("runs")
    }

    #[test]
    fn an_answer_reads_into_ways_in_order() {
        let w = ways_of("SOCKS5 127.0.0.1:1080; PROXY p.example; HTTPS h.example:8443;SOCKS s:9; DIRECT; QUIC q:1; PROXY ;direct").expect("reads");
        let spelled: Vec<String> = w.iter().map(|w| match w { Way::Direct => "direct".into(), Way::Through(p) => p.spelled() }).collect();
        assert_eq!(spelled, ["socks5://127.0.0.1:1080", "http://p.example:80", "https://h.example:8443", "socks5://s:9", "socks4://s:9", "direct"]);
        let c = ways_of("PROXY u:pw@p:3128").expect("credentials");
        assert!(matches!(&c[0], Way::Through(p) if p.has_password() && p.shown() == "http://u@p:3128"));
        let s4 = ways_of("SOCKS u:pw@s:1").expect("socks");
        assert!(matches!(&s4[1], Way::Through(p) if p.kind == Kind::Socks4 && !p.has_password() && p.shown() == "socks4://u@s:1"));
        for bad in ["", "  ;  ", "BOGUS x:1", "PROXY a b", "PROXY h:99999"] {
            assert_eq!(ways_of(bad), Err(PacTrouble::Answer), "{bad:?}");
        }
    }

    #[test]
    fn the_standard_functions_answer() {
        assert_eq!(says("isPlainHostName('intranet') + ',' + isPlainHostName('a.b')"), "true,false");
        assert_eq!(says("dnsDomainIs('www.Corp.example', '.corp.example') + ',' + dnsDomainIs('corp.example', '.corp.example')"), "true,false");
        assert_eq!(says("localHostOrDomainIs('www', 'www.corp.example') + ',' + localHostOrDomainIs('www.corp.example', 'www.corp.example') + ',' + localHostOrDomainIs('www.other', 'www.corp.example')"), "true,true,false");
        assert_eq!(says("isResolvable('node.example') + ',' + isResolvable('nowhere.example')"), "true,false");
        assert_eq!(says("isInNet('intranet.corp', '10.0.0.0', '255.0.0.0') + ',' + isInNet('192.168.1.5', '192.168.0.0', '255.255.0.0') + ',' + isInNet('11.0.0.1', '10.0.0.0', '255.0.0.0')"), "true,true,false");
        assert_eq!(says("dnsResolve('node.example') + ',' + dnsResolve('nowhere.example')"), "93.184.216.34,null");
        assert_eq!(says("myIpAddress()"), "192.168.1.20");
        assert_eq!(says("dnsDomainLevels('www') + ',' + dnsDomainLevels('a.b.c')"), "0,2");
        assert_eq!(says("shExpMatch('http://a.corp.example/x', '*.corp.example/*') + ',' + shExpMatch('abc', 'a?c') + ',' + shExpMatch('a.c', 'a?d') + ',' + shExpMatch('a+b', 'a+b')"), "true,true,false,true");
        assert_eq!(says("weekdayRange('SUN', 'SAT') + ',' + weekdayRange('SUN', 'SAT', 'GMT') + ',' + weekdayRange('XYZ')"), "true,true,false");
        assert_eq!(says("dateRange(1, 31) + ',' + dateRange('JAN', 'DEC') + ',' + dateRange(1995, 2999) + ',' + dateRange(1, 'JAN', 31, 'DEC') + ',' + dateRange(1990) + ',' + dateRange(1, 1990, 2, 1991)"), "true,true,true,true,false,false");
        assert_eq!(says("timeRange(0, 24) + ',' + timeRange(0, 0, 0, 23, 59, 59) + ',' + timeRange(0, 0, 24, 0, 'GMT')"), "true,true,true");
        assert_eq!(says("isResolvableEx('node.example') + ',' + isResolvableEx('nowhere.example')"), "true,false");
        assert_eq!(says("isInNetEx('2606:2800:220:1::1', '2606:2800::/32') + ',' + isInNetEx('10.1.2.3', '10.0.0.0/8') + ',' + isInNetEx('10.1.2.3', '11.0.0.0/8')"), "true,true,false");
        assert_eq!(says("dnsResolveEx('node.example')"), "93.184.216.34;2606:2800:220:1::1");
        assert_eq!(says("myIpAddressEx()"), "192.168.1.20;fe80::1");
        assert_eq!(says("sortIpAddressList('10.0.0.1;fe80::1;1.2.3.4') + ',' + sortIpAddressList('x')"), "fe80::1;10.0.0.1;1.2.3.4,false");
        assert_eq!(says("getClientVersion()"), "1.0");
    }

    #[test]
    fn the_sandbox_holds_the_closed_sets_and_nothing_else() {
        let names = says("Object.getOwnPropertyNames(globalThis).sort().join(',')");
        for n in names.split(',') {
            assert!(LANGUAGE_GLOBALS.contains(&n) || PAC_FUNCTIONS.contains(&n), "{n} is in the sandbox");
        }
        for gone in ["require", "setTimeout", "setInterval", "queueMicrotask", "fetch", "XMLHttpRequest", "os", "std", "Promise", "WebAssembly", "print", "console"] {
            assert_eq!(says(&format!("typeof {gone}")), "undefined", "{gone}");
        }
        assert_eq!(ask("import x from 'os'; function FindProxyForURL() { return 'DIRECT'; }", "https://n/", "n"), Err(PacTrouble::Failed));
    }

    #[test]
    fn a_script_that_cannot_be_used_is_named_within_the_limit() {
        let started = Instant::now();
        assert_eq!(ask("function FindProxyForURL() { throw new Error('no'); }", "https://n/", "n"), Err(PacTrouble::Failed));
        assert_eq!(ask("function FindProxyForURL( {", "https://n/", "n"), Err(PacTrouble::Failed));
        assert_eq!(ask("var x = 1;", "https://n/", "n"), Err(PacTrouble::NoFunction));
        assert_eq!(ask("function FindProxyForURL() { return 42; }", "https://n/", "n"), Err(PacTrouble::Answer));
        assert_eq!(ask("function FindProxyForURL() { var a = []; for (;;) a.push(new Array(100000).join('x') + a.length); }", "https://n/", "n"), Err(PacTrouble::Memory));
        assert_eq!(ask("function f(n) { return f(n + 1) + 1; } function FindProxyForURL() { return f(0); }", "https://n/", "n"), Err(PacTrouble::Memory));
        assert!(started.elapsed() < RUN_LIMIT, "the quick refusals take no time to speak of");
        let looping = Instant::now();
        assert_eq!(ask("function FindProxyForURL() { for (;;) {} }", "https://n/", "n"), Err(PacTrouble::Timeout));
        assert_eq!(ask("function FindProxyForURL() { try { for (;;) {} } catch (e) { return 'DIRECT'; } }", "https://n/", "n"), Err(PacTrouble::Timeout), "the limit cannot be caught");
        let took = looping.elapsed();
        assert!(took >= RUN_LIMIT * 2 && took < RUN_LIMIT * 2 + Duration::from_secs(1), "{took:?}");
    }

    #[test]
    fn lookups_share_the_runs_limit() {
        let started = Instant::now();
        let r = run(Arc::new("function FindProxyForURL(u, h) { for (var i = 0; i < 100; i++) dnsResolve('x' + i); return 'DIRECT'; }".into()), "https://n/", "n", RUN_LIMIT, Host { resolve: slow_names, my_ips: mine, stop: never });
        assert!(matches!(r, Ok(_) | Err(PacTrouble::Timeout)), "{r:?}");
        assert!(started.elapsed() < RUN_LIMIT + Duration::from_secs(1), "{:?}", started.elapsed());
    }

    #[test]
    fn the_ex_function_comes_first_and_the_path_is_never_asked() {
        assert_eq!(ask("function FindProxyForURLEx(u, h) { return 'PROXY ex:1'; } function FindProxyForURL() { return 'DIRECT'; }", "https://n/", "n").as_deref(), Ok("PROXY ex:1"));
        let t = crate::parse("https://node.example:8545/v3/secret?apikey=x").expect("address");
        assert_eq!(asked_url(&t), "https://node.example:8545/");
    }

    #[test]
    fn a_fetch_or_run_is_judged_in_its_own_time_and_kept_once_it_lands() {
        let landed = |f: &dyn Fn() -> bool| {
            let until = Instant::now() + FETCH_LIMIT + RUN_LIMIT + Duration::from_secs(1);
            while !f() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(10));
            }
            f()
        };
        let gone = std::net::TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port();
        let url = format!("http://127.0.0.1:{gone}/own-time.pac");
        assert!(matches!(script_at(&url, Duration::ZERO), Fetched::Trouble(PacTrouble::Unfetched)));
        assert!(landed(&|| kept_script(&url).is_some()), "the fetch lands and is kept");
        let script = Arc::new("function FindProxyForURL(u, h) { return isResolvable('node.example') ? 'PROXY p:1' : 'DIRECT'; }".to_string());
        let t = crate::parse("https://own-time.example/").expect("address");
        let key = (digest(&script), asked_url(&t));
        assert_eq!(answer(&script, &t, Duration::ZERO, STAGED), Err(PacTrouble::Timeout), "no time left: not this once");
        assert!(landed(&|| kept_answer(&key).is_some()), "the run lands and is kept");
        assert_eq!(answer(&script, &t, Duration::ZERO, STAGED).map(|w| w.len()), Ok(1), "its own answer, with no time needed");
    }

    #[test]
    fn a_redirect_resolves_as_rfc_3986_resolves_a_reference() {
        let base = crate::parse("http://a/b/c/d;p?q").expect("base");
        let cases = [
            ("g", "http://a/b/c/g"),
            ("./g", "http://a/b/c/g"),
            ("g/", "http://a/b/c/g/"),
            ("/g", "http://a/g"),
            ("//g", "http://g/"),
            ("?y", "http://a/b/c/d;p?y"),
            ("g?y", "http://a/b/c/g?y"),
            ("#s", "http://a/b/c/d;p?q"),
            ("g#s", "http://a/b/c/g"),
            ("g?y#s", "http://a/b/c/g?y"),
            (";x", "http://a/b/c/;x"),
            ("g;x", "http://a/b/c/g;x"),
            ("", "http://a/b/c/d;p?q"),
            (".", "http://a/b/c/"),
            ("./", "http://a/b/c/"),
            ("..", "http://a/b/"),
            ("../", "http://a/b/"),
            ("../g", "http://a/b/g"),
            ("../..", "http://a/"),
            ("../../", "http://a/"),
            ("../../g", "http://a/g"),
            ("../../../g", "http://a/g"),
            ("../../../../g", "http://a/g"),
            ("/./g", "http://a/g"),
            ("/../g", "http://a/g"),
            ("g.", "http://a/b/c/g."),
            (".g", "http://a/b/c/.g"),
            ("g..", "http://a/b/c/g.."),
            ("..g", "http://a/b/c/..g"),
            ("./../g", "http://a/b/g"),
            ("./g/.", "http://a/b/c/g/"),
            ("g/./h", "http://a/b/c/g/h"),
            ("g/../h", "http://a/b/c/h"),
            ("g;x=1/./y", "http://a/b/c/g;x=1/y"),
            ("g;x=1/../y", "http://a/b/c/y"),
            ("https://other.example:8443/x/../p.pac", "https://other.example:8443/p.pac"),
        ];
        for (reference, want) in cases {
            assert_eq!(redirected(&base, reference).map(|t| t.url()), Some(want.to_string()), "{reference:?}");
        }
        for not_followed in ["g:h", "ftp://a/x.pac", "file:///etc/proxy.pac"] {
            assert!(redirected(&base, not_followed).is_none(), "{not_followed}");
        }
    }

    #[test]
    fn in_net_ex_reads_prefixes() {
        assert!(in_net_ex("10.1.2.3", "10.0.0.0/8") && in_net_ex("fe80::1", "fe80::/10") && in_net_ex("1.2.3.4", "0.0.0.0/0"));
        assert!(!in_net_ex("10.1.2.3", "fe80::/10") && !in_net_ex("x", "10.0.0.0/8") && !in_net_ex("10.1.2.3", "10.0.0.0/33") && !in_net_ex("10.1.2.3", "10.0.0.0"));
    }

    #[test]
    fn a_file_script_is_not_read_and_each_trouble_has_its_name() {
        let t = crate::parse("https://node.example/").expect("address");
        let s = SystemProxies { pac_url: Some("file:///etc/proxy.pac".into()), ..Default::default() };
        assert_eq!(auto_with(&t, &s, Duration::from_secs(1), STAGED), Auto::Trouble(PacTrouble::Scheme));
        let codes: Vec<&str> = PacTrouble::ALL.iter().map(|t| t.code()).collect();
        assert_eq!(codes, ["PAC_UNFETCHED", "PAC_SCHEME", "PAC_REMOTE_HTTP", "PAC_TOO_LARGE", "PAC_FAILED", "PAC_TIMEOUT", "PAC_MEMORY", "PAC_NO_FUNCTION", "PAC_ANSWER"]);
    }

    #[test]
    fn a_script_comes_from_this_machine_or_over_https() {
        let src = |u: &str| script_source(&crate::read_address(u).expect("address"));
        for ok in ["http://127.0.0.1:1/p.pac", "http://127.9.9.9/p.pac", "http://[::1]:1/p.pac", "http://localhost:1/p.pac", "http://LOCALHOST/p.pac", "https://pac.example/p.pac", "https://10.0.0.1/p.pac"] {
            assert_eq!(src(ok), Ok(()), "{ok}");
        }
        for no in ["http://pac.example/p.pac", "http://10.0.0.1/p.pac", "http://192.168.1.1/p.pac", "http://localhost.example/p.pac", "http://sub.localhost/p.pac", "http://0.0.0.0/p.pac", "http://[::ffff:127.0.0.1]/p.pac"] {
            assert_eq!(src(no), Err(PacTrouble::RemoteHttp), "{no}");
        }
        let t = crate::parse("https://node.example/").expect("address");
        let s = SystemProxies { pac_url: Some("http://pac.example/proxy.pac".into()), ..Default::default() };
        assert_eq!(auto_with(&t, &s, Duration::from_secs(1), STAGED), Auto::Trouble(PacTrouble::RemoteHttp));
        assert_eq!(auto_known(&t, &s), Auto::Trouble(PacTrouble::RemoteHttp));
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = l.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            use std::io::{Read, Write};
            if let Ok((mut c, _)) = l.accept() {
                let mut buf = [0u8; 1024];
                let _ = c.read(&mut buf);
                let _ = c.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://192.0.2.10/p.pac\r\nContent-Length: 0\r\n\r\n");
            }
        });
        assert!(matches!(fetch(&format!("http://127.0.0.1:{port}/p.pac"), FETCH_LIMIT), Fetched::Trouble(PacTrouble::RemoteHttp)));
    }
}
