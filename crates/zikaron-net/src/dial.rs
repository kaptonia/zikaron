use crate::{keep, Clock, Fail, Target, SLICE};
use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const CONNECT_CAP: Duration = Duration::from_secs(10);

pub const STAGGER: Duration = Duration::from_millis(250);

static LAST: Mutex<BTreeMap<(String, u16), SocketAddr>> = Mutex::new(BTreeMap::new());

fn last() -> std::sync::MutexGuard<'static, BTreeMap<(String, u16), SocketAddr>> {
    LAST.lock().unwrap_or_else(|e| e.into_inner())
}

pub(crate) fn order(addrs: &[SocketAddr], last: Option<SocketAddr>) -> Vec<SocketAddr> {
    let mut seen: Vec<SocketAddr> = Vec::new();
    for a in addrs {
        if !seen.contains(a) {
            seen.push(*a);
        }
    }
    let Some(first) = seen.first().copied() else { return seen };
    let (same, other): (Vec<SocketAddr>, Vec<SocketAddr>) = seen.into_iter().partition(|a| a.is_ipv6() == first.is_ipv6());
    let mut out = Vec::with_capacity(same.len() + other.len());
    let (mut a, mut b) = (same.into_iter(), other.into_iter());
    loop {
        match (a.next(), b.next()) {
            (None, None) => break,
            (x, y) => out.extend(x.into_iter().chain(y)),
        }
    }
    if let Some(l) = last {
        if let Some(i) = out.iter().position(|a| *a == l) {
            let l = out.remove(i);
            out.insert(0, l);
        }
    }
    out
}

pub(crate) type Resolver = fn(&str, u16) -> std::io::Result<Vec<SocketAddr>>;

fn system_resolver(host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
    std::net::ToSocketAddrs::to_socket_addrs(&(host, port)).map(|a| a.collect::<Vec<_>>())
}

fn resolve(t: &Target, clock: &Clock) -> Result<Vec<SocketAddr>, Fail> {
    resolve_with(t, clock, system_resolver)
}

fn resolve_with(t: &Target, clock: &Clock, resolver: Resolver) -> Result<Vec<SocketAddr>, Fail> {
    let host = t.dial_host().to_string();
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, t.port)]);
    }
    let (tx, rx) = mpsc::channel();
    let (h, port) = (host.clone(), t.port);
    std::thread::spawn(move || {
        let _ = tx.send(resolver(h.as_str(), port));
    });
    loop {
        if keep::closing() {
            return Err(keep::cut());
        }
        let wait = match clock.left()? {
            Some(left) => left.min(SLICE),
            None => SLICE,
        };
        match rx.recv_timeout(wait) {
            Ok(Ok(addrs)) if !addrs.is_empty() => return Ok(addrs),
            Ok(Ok(_)) => return Err(Fail::Name(host)),
            Ok(Err(e)) => return Err(Fail::Name(format!("{host}: {e}"))),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Err(Fail::Name(host)),
        }
    }
}

pub(crate) fn lookup(host: &str, limit: Duration) -> Vec<IpAddr> {
    if limit.is_zero() {
        return Vec::new();
    }
    let t = Target { scheme: crate::Scheme::Http, host: host.trim().to_ascii_lowercase(), port: 0, path: "/".into() };
    resolve(&t, &Clock::start(limit)).map(|v| v.into_iter().map(|a| a.ip()).collect()).unwrap_or_default()
}

pub(crate) type Attempt = fn(&SocketAddr, Duration) -> std::io::Result<TcpStream>;

pub(crate) fn connect(t: &Target, clock: &Clock) -> Result<TcpStream, Fail> {
    let key = (t.dial_host().to_string(), t.port);
    let addrs = order(&resolve(t, clock)?, last().get(&key).copied());
    let (a, tcp) = race(&addrs, clock, TcpStream::connect_timeout)?;
    last().insert(key, a);
    Ok(tcp)
}

pub(crate) fn race(addrs: &[SocketAddr], clock: &Clock, attempt: Attempt) -> Result<(SocketAddr, TcpStream), Fail> {
    race_capped(addrs, clock, attempt, CONNECT_CAP)
}

fn race_capped(addrs: &[SocketAddr], clock: &Clock, attempt: Attempt, cap: Duration) -> Result<(SocketAddr, TcpStream), Fail> {
    let began = Instant::now();
    let left = clock.left()?;
    let cap_at = match left {
        Some(left) => began + left.min(cap),
        None => began + cap,
    };
    let bound_is_deadline = left.is_some_and(|l| l < cap);
    let mut timed_out = false;
    let (tx, rx) = mpsc::channel::<(SocketAddr, std::io::Result<TcpStream>)>();
    let mut started = 0usize;
    let mut next_at = began;
    let mut failed: Vec<String> = Vec::new();
    loop {
        if keep::closing() {
            return Err(keep::cut());
        }
        let now = Instant::now();
        if started < addrs.len() && now >= next_at && now < cap_at {
            let (a, tx, within) = (addrs[started], tx.clone(), cap_at - now);
            std::thread::spawn(move || {
                let _ = tx.send((a, attempt(&a, within)));
            });
            started += 1;
            next_at = now + STAGGER;
        }
        let until = if started < addrs.len() { next_at.min(cap_at) } else { cap_at };
        let wait = until.saturating_duration_since(Instant::now()).min(SLICE).max(Duration::from_millis(1));
        match rx.recv_timeout(wait) {
            Ok((a, Ok(tcp))) => return Ok((a, tcp)),
            Ok((a, Err(e))) => {
                timed_out |= matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock);
                failed.push(format!("{a}: {e}"));
                next_at = Instant::now();
                if failed.len() == addrs.len() {
                    if timed_out && bound_is_deadline {
                        return Err(clock.late());
                    }
                    return Err(Fail::Connect(failed.join(" · ")));
                }
            }
            Err(_) => {
                if Instant::now() >= cap_at {
                    clock.left()?;
                    let tried: Vec<String> = addrs[..started].iter().map(|a| a.to_string()).collect();
                    failed.push(format!("{} did not connect within {} ms", tried.join(" "), cap.as_millis()));
                    return Err(Fail::Connect(failed.join(" · ")));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(s: &str) -> SocketAddr {
        s.parse().expect("an address")
    }

    #[test]
    fn a_first_address_that_swallows_the_attempt_does_not_hold_up_the_second() {
        let Some(_turn) = keep::turn() else { return };
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let live = l.local_addr().expect("addr");
        let swallowing = a("127.0.0.2:9");
        fn attempt(to: &SocketAddr, within: Duration) -> std::io::Result<TcpStream> {
            if to.ip() == "127.0.0.2".parse::<IpAddr>().expect("ip") {
                std::thread::sleep(within);
                return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "swallowed"));
            }
            TcpStream::connect_timeout(to, within)
        }
        let clock = Clock::start(Duration::from_secs(30));
        let began = Instant::now();
        let (used, _tcp) = race(&[swallowing, live], &clock, attempt).expect("the second address connects");
        let took = began.elapsed();
        assert_eq!(used, live);
        assert!(took >= STAGGER && took < STAGGER + Duration::from_millis(750), "took {took:?}");
        assert!(took < CONNECT_CAP / 4);
        fn swallow(_: &SocketAddr, within: Duration) -> std::io::Result<TcpStream> {
            std::thread::sleep(within.min(Duration::from_millis(1500)));
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "swallowed"))
        }
        let short = Clock::start(Duration::from_millis(600));
        match race(&[swallowing, a("127.0.0.3:9")], &short, swallow) {
            Err(Fail::Late(d)) => assert_eq!(d, Duration::from_millis(600)),
            other => panic!("{:?}", other.map(|x| x.0)),
        }
        fn early(_: &SocketAddr, within: Duration) -> std::io::Result<TcpStream> {
            std::thread::sleep(within.saturating_sub(Duration::from_millis(100)));
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "swallowed"))
        }
        let short = Clock::start(Duration::from_millis(600));
        assert!(matches!(race(&[swallowing, a("127.0.0.3:9")], &short, early), Err(Fail::Late(_))));
        fn refuse(_: &SocketAddr, _: Duration) -> std::io::Result<TcpStream> {
            Err(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused"))
        }
        let short = Clock::start(Duration::from_millis(600));
        assert!(matches!(race(&[swallowing, a("127.0.0.3:9")], &short, refuse), Err(Fail::Connect(s)) if s.contains("127.0.0.2:9") && s.contains("127.0.0.3:9")));
    }

    #[test]
    fn the_deadline_running_out_while_resolving_is_late() {
        let Some(_turn) = keep::turn() else { return };
        fn slow(_: &str, _: u16) -> std::io::Result<Vec<SocketAddr>> {
            std::thread::sleep(Duration::from_secs(2));
            Ok(vec!["127.0.0.1:1".parse().expect("an address")])
        }
        let t = crate::parse("http://node.example:8545/").expect("an address");
        let clock = Clock::start(Duration::from_millis(300));
        let began = Instant::now();
        assert_eq!(resolve_with(&t, &clock, slow).err(), Some(Fail::Late(Duration::from_millis(300))));
        let took = began.elapsed();
        assert!(took >= Duration::from_millis(300) && took < Duration::from_millis(300 + 1500), "took {took:?}");
    }

    #[test]
    fn closing_down_while_resolving_is_closed() {
        let Some(_turn) = keep::turn() else { return };
        fn slow(_: &str, _: u16) -> std::io::Result<Vec<SocketAddr>> {
            std::thread::sleep(Duration::from_secs(2));
            Ok(vec!["127.0.0.1:1".parse().expect("an address")])
        }
        let t = crate::parse("http://node.example:8545/").expect("an address");
        let clock = Clock::start(Duration::from_secs(30));
        let asking = std::thread::spawn(move || {
            let began = Instant::now();
            (resolve_with(&t, &clock, slow).err(), began.elapsed())
        });
        std::thread::sleep(Duration::from_millis(100));
        let cut_before = keep::cuts();
        keep::close_down();
        let (said, took) = asking.join().expect("the resolving thread");
        let cut_after = keep::cuts();
        keep::open_up();
        assert_eq!(said, Some(Fail::Closed));
        assert_eq!(cut_after, cut_before + 1, "the cut leaves its trace, once");
        assert!(took < Duration::from_secs(1), "at once, not at the resolver's pace: {took:?}");
    }

    #[test]
    fn the_connect_cap_running_out_names_every_address_tried() {
        let Some(_turn) = keep::turn() else { return };
        fn swallow(_: &SocketAddr, _: Duration) -> std::io::Result<TcpStream> {
            std::thread::sleep(Duration::from_secs(2));
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "swallowed"))
        }
        let cap = STAGGER * 3;
        let clock = Clock::start(Duration::from_secs(5));
        let began = Instant::now();
        match race_capped(&[a("127.0.0.2:9"), a("127.0.0.3:9")], &clock, swallow, cap) {
            Err(Fail::Connect(s)) => assert_eq!(s, format!("127.0.0.2:9 127.0.0.3:9 did not connect within {} ms", cap.as_millis())),
            other => panic!("{:?}", other.map(|x| x.0)),
        }
        let took = began.elapsed();
        assert!(took >= cap && took < cap + Duration::from_millis(1500), "took {took:?}");
    }

    #[test]
    fn closing_down_while_connecting_is_closed() {
        let Some(_turn) = keep::turn() else { return };
        fn swallow(_: &SocketAddr, _: Duration) -> std::io::Result<TcpStream> {
            std::thread::sleep(Duration::from_secs(5));
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "swallowed"))
        }
        let clock = Clock::start(Duration::from_secs(30));
        let asking = std::thread::spawn(move || {
            let began = Instant::now();
            (race(&[a("127.0.0.2:9"), a("127.0.0.3:9")], &clock, swallow).err().map(|f| f == Fail::Closed), began.elapsed())
        });
        std::thread::sleep(STAGGER + Duration::from_millis(50));
        let cut_before = keep::cuts();
        let raised = Instant::now();
        keep::raise_flag_only();
        let (closed, _) = asking.join().expect("the connecting thread");
        let took = raised.elapsed();
        let cut_after = keep::cuts();
        keep::open_up();
        assert_eq!(closed, Some(true));
        assert_eq!(cut_after, cut_before + 1, "the cut leaves its trace, once");
        assert!(took < SLICE * 4, "within a slice or so, not at the cap: {took:?}");
    }

    #[test]
    fn the_families_take_turns_and_the_last_good_address_leads() {
        let given = [a("[::1]:1"), a("[::2]:1"), a("127.0.0.1:1"), a("[::1]:1"), a("127.0.0.2:1")];
        assert_eq!(order(&given, None), vec![a("[::1]:1"), a("127.0.0.1:1"), a("[::2]:1"), a("127.0.0.2:1")]);
        assert_eq!(order(&given, Some(a("127.0.0.2:1")))[0], a("127.0.0.2:1"));
        assert_eq!(order(&given, Some(a("10.0.0.1:1")))[0], a("[::1]:1"), "an address not resolved now is not tried");
        let v4 = [a("127.0.0.1:1"), a("127.0.0.2:1"), a("[::1]:1")];
        assert_eq!(order(&v4, None), vec![a("127.0.0.1:1"), a("[::1]:1"), a("127.0.0.2:1")]);
        assert!(order(&[], None).is_empty());
    }
}
