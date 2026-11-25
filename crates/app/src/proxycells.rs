use crate::secret::Secret;
use zikaron_net::Kind;

pub const KINDS: [(Kind, &str); 4] = [(Kind::Http, "HTTP"), (Kind::Https, "HTTPS"), (Kind::Socks5, "SOCKS5"), (Kind::Socks4, "SOCKS4")];

pub const CREDENTIAL_MAX: usize = 255;

#[derive(Clone, Debug, PartialEq)]
pub struct Cells {
    pub kind: Kind,
    pub host: String,
    pub port: String,
    pub user: String,
    pub pass: Secret,
}

impl Default for Cells {
    fn default() -> Cells {
        Cells { kind: Kind::Http, host: String::new(), port: String::new(), user: String::new(), pass: Secret::default() }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bad {
    Host,
    Port,
    UserMissing,
    UserLong,
    PassLong,
}

pub fn takes_password(kind: Kind) -> bool {
    kind != Kind::Socks4
}

pub fn check(c: &Cells) -> Result<String, Vec<Bad>> {
    let mut bad = Vec::new();
    let host = c.host.trim();
    let host_reads = !host.is_empty() && !host.contains("://") && !host.contains('@') && !host.contains('/') && !host.chars().any(|ch| ch.is_whitespace() || ch.is_control());
    if !host_reads {
        bad.push(Bad::Host);
    }
    let port = c.port.trim();
    let port_n = port.parse::<u32>().ok().filter(|n| (1..=65_535).contains(n) && port.bytes().all(|b| b.is_ascii_digit()));
    if port_n.is_none() {
        bad.push(Bad::Port);
    }
    let user = c.user.trim();
    let pass = if takes_password(c.kind) { c.pass.expose() } else { "" };
    if user.is_empty() && !pass.is_empty() {
        bad.push(Bad::UserMissing);
    }
    if user.len() > CREDENTIAL_MAX {
        bad.push(Bad::UserLong);
    }
    if pass.len() > CREDENTIAL_MAX {
        bad.push(Bad::PassLong);
    }
    if !bad.is_empty() {
        return Err(bad);
    }
    let host = match host.parse::<std::net::Ipv6Addr>() {
        Ok(_) => format!("[{host}]"),
        Err(_) => host.to_string(),
    };
    let auth = (!user.is_empty()).then(|| zikaron_net::Auth { user: user.to_string(), pass: (!pass.is_empty()).then(|| zikaron_net::Secret::new(pass.to_string())) });
    let p = zikaron_net::Proxy { kind: c.kind, host, port: port_n.unwrap_or(0) as u16, auth };
    let written = p.written();
    match zikaron_net::proxy_of(&written) {
        Ok(_) => Ok(written),
        Err(zikaron_net::NotAProxy::Shape) => Err(vec![Bad::Host]),
        Err(zikaron_net::NotAProxy::Credentials) => Err(vec![Bad::UserMissing]),
    }
}

pub fn of_address(text: &str, kind: Kind) -> Option<Cells> {
    let t = text.trim();
    let p = if t.contains("://") { zikaron_net::proxy_of(t).ok()? } else { zikaron_net::proxy_of(&format!("{}{t}", scheme_of(kind))).ok()? };
    let host = p.host.trim_start_matches('[').trim_end_matches(']').to_string();
    let (user, pass) = match p.auth {
        Some(a) => (a.user, a.pass.map(|s| Secret::from(s.expose().to_string())).unwrap_or_default()),
        None => (String::new(), Secret::default()),
    };
    Some(Cells { kind: p.kind, host, port: p.port.to_string(), user, pass })
}

pub fn split_in(c: &Cells, pasted: Option<&str>, focused: bool) -> Option<Cells> {
    let whole_paste = pasted.is_some_and(|p| p.trim() == c.host.trim());
    if focused && !whole_paste {
        return None;
    }
    let split = of_address(&c.host, c.kind)?;
    let user = if split.user.is_empty() { c.user.clone() } else { split.user.clone() };
    let pass = if split.pass.is_empty() { c.pass.clone() } else { split.pass.clone() };
    Some(Cells { user, pass, ..split })
}

pub fn password_stays(saved: &zikaron_net::Proxy, typed: &zikaron_net::Proxy) -> bool {
    let same_proxy = (saved.kind, &saved.host, saved.port) == (typed.kind, &typed.host, typed.port);
    let same_user = match (&saved.auth, &typed.auth) {
        (Some(s), Some(t)) => s.user == t.user && s.pass.is_some() && t.pass.is_none(),
        _ => false,
    };
    takes_password(typed.kind) && same_proxy && same_user
}

pub fn of_saved(written: &str) -> Option<(Cells, bool)> {
    let p = zikaron_net::proxy_of(written).ok()?;
    let saved_pass = p.has_password();
    let host = p.host.trim_start_matches('[').trim_end_matches(']').to_string();
    let user = p.auth.map(|a| a.user).unwrap_or_default();
    Some((Cells { kind: p.kind, host, port: p.port.to_string(), user, pass: Secret::default() }, saved_pass))
}

fn scheme_of(kind: Kind) -> &'static str {
    match kind {
        Kind::Http => "http://",
        Kind::Https => "https://",
        Kind::Socks5 => "socks5://",
        Kind::Socks4 => "socks4://",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(kind: Kind, host: &str, port: &str, user: &str, pass: &str) -> Cells {
        Cells { kind, host: host.into(), port: port.into(), user: user.into(), pass: Secret::from(pass.to_string()) }
    }

    #[test]
    fn the_cells_are_judged_one_by_one() {
        for port in ["0", "65536", "", "8a", "-1", "+80", " "] {
            assert_eq!(check(&cells(Kind::Http, "127.0.0.1", port, "", "")), Err(vec![Bad::Port]), "{port:?}");
        }
        for port in ["1", "80", "65535"] {
            assert!(check(&cells(Kind::Http, "127.0.0.1", port, "", "")).is_ok(), "{port}");
        }
        for host in ["", "http://h", "u@h", "h/x", "a b"] {
            assert_eq!(check(&cells(Kind::Http, host, "80", "", "")), Err(vec![Bad::Host]), "{host:?}");
        }
        assert_eq!(check(&cells(Kind::Socks5, "", "0", "", "pw")), Err(vec![Bad::Host, Bad::Port, Bad::UserMissing]));
        assert_eq!(check(&cells(Kind::Socks5, "h", "1", &"u".repeat(256), &"p".repeat(256))), Err(vec![Bad::UserLong, Bad::PassLong]));
        assert!(check(&cells(Kind::Socks5, "h", "1", &"u".repeat(255), &"p".repeat(255))).is_ok());
    }

    #[test]
    fn the_cells_make_the_address_the_transport_reads() {
        let a = check(&cells(Kind::Http, "proxy.corp", "8080", "me@corp", "p@ss:w rd")).expect("reads");
        let p = zikaron_net::proxy_of(&a).expect("the transport reads it");
        assert_eq!((p.kind, p.host.as_str(), p.port), (Kind::Http, "proxy.corp", 8080));
        let auth = p.auth.expect("credentials");
        assert_eq!((auth.user.as_str(), auth.pass.as_ref().map(|s| s.expose())), ("me@corp", Some("p@ss:w rd")));
        let v6 = check(&cells(Kind::Socks5, "::1", "1080", "", "")).expect("reads");
        assert_eq!(zikaron_net::proxy_of(&v6).map(|p| p.port), Ok(1080));
        let s4 = check(&cells(Kind::Socks4, "h", "1080", "me", "ignored")).expect("reads");
        assert!(!zikaron_net::proxy_of(&s4).expect("reads").has_password());
        let kept = check(&cells(Kind::Https, "h", "443", "me", "")).expect("reads");
        assert_eq!(zikaron_net::proxy_of(&kept).map(|p| (p.has_password(), p.auth.map(|a| a.user))), Ok((false, Some("me".to_string()))));
    }

    #[test]
    fn a_pasted_address_is_split_into_the_cells() {
        let c = of_address("socks5://me:p%40ss@127.0.0.1:1080", Kind::Http).expect("split");
        assert_eq!((c.kind, c.host.as_str(), c.port.as_str(), c.user.as_str(), c.pass.expose()), (Kind::Socks5, "127.0.0.1", "1080", "me", "p@ss"));
        let c = of_address("proxy.corp:3128", Kind::Https).expect("split");
        assert_eq!((c.kind, c.host.as_str(), c.port.as_str()), (Kind::Https, "proxy.corp", "3128"));
        assert_eq!(of_address("proxy.corp", Kind::Http), None);
        assert_eq!(of_address("ftp://h:1", Kind::Http), None);
        let (c, saved) = of_saved("http://me:secret@h:8080").expect("saved");
        assert_eq!((c.user.as_str(), c.pass.expose(), saved), ("me", "", true));
    }

    #[test]
    fn the_host_cell_is_split_on_a_whole_paste_or_on_leaving() {
        let me = |host: &str| cells(Kind::Http, host, "", "me", "pw");
        let mut so_far = String::new();
        for ch in "127.0.0.1:7890".chars() {
            so_far.push(ch);
            assert_eq!(split_in(&me(&so_far), None, true), None, "{so_far}");
            assert_eq!(split_in(&me(&so_far), Some(&ch.to_string()), true), None, "{so_far}");
        }
        let left = split_in(&me("127.0.0.1:7890"), None, false).expect("split on leaving");
        assert_eq!((left.host.as_str(), left.port.as_str(), left.user.as_str(), left.pass.expose()), ("127.0.0.1", "7890", "me", "pw"));
        let pasted = split_in(&me("socks5://you:p%40ss@h:1080"), Some("socks5://you:p%40ss@h:1080"), true).expect("split on the paste");
        assert_eq!((pasted.kind, pasted.host.as_str(), pasted.user.as_str(), pasted.pass.expose()), (Kind::Socks5, "h", "you", "p@ss"));
        assert_eq!(split_in(&me("proxy.corp"), None, false), None);
    }

    #[test]
    fn a_saved_password_stays_only_with_its_own_proxy() {
        let p = |a: &str| zikaron_net::proxy_of(a).expect("reads");
        let saved = p("http://me:secret@h:8080");
        assert!(password_stays(&saved, &p("http://me@h:8080")));
        for other in ["http://me@h:8081", "http://me@other:8080", "https://me@h:8080", "socks5://me@h:8080", "http://you@h:8080", "http://me:new@h:8080", "http://h:8080"] {
            assert!(!password_stays(&saved, &p(other)), "{other}");
        }
        assert!(!password_stays(&p("socks4://me@h:1080"), &p("socks4://me@h:1080")));
    }
}
