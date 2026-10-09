//! Resolving a host and connecting to it, within the exchange's single deadline.
//!
//! Resolution waits no longer than the time left. Every resolved address is tried (Happy Eyeballs style): the
//! two address families alternate (the system's first address stays first), a new attempt starts every
//! [`STAGGER`] without cancelling earlier ones, and the first to connect wins. Connecting fails only when every
//! address has failed or the connect cap has passed; the error names every address tried. The address that last
//! connected for a host and port is tried first next time.

use crate::{keep, Clock, Fail, Target, SLICE};
use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Maximum time for connecting, within the time left, so a peer that never answers is not waited on for the
/// whole deadline.
pub const CONNECT_CAP: Duration = Duration::from_secs(10);

/// Delay before the next address is tried in parallel.
pub const STAGGER: Duration = Duration::from_millis(250);

/// For each host and port, the address that last connected.
static LAST: Mutex<BTreeMap<(String, u16), SocketAddr>> = Mutex::new(BTreeMap::new());

fn last() -> std::sync::MutexGuard<'static, BTreeMap<(String, u16), SocketAddr>> {
    LAST.lock().unwrap_or_else(|e| e.into_inner())
}

/// The addresses in try order: duplicates dropped, the two families alternating from the system's first, and
/// the address that last connected moved to the front.
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

/// Resolves one host name and port. Production uses the system resolver; it is a parameter so tests can stage
/// a resolver that never answers (as the race takes its [`Attempt`]).
pub(crate) type Resolver = fn(&str, u16) -> std::io::Result<Vec<SocketAddr>>;

/// The system resolver.
fn system_resolver(host: &str, port: u16) -> std::io::Result<Vec<SocketAddr>> {
    std::net::ToSocketAddrs::to_socket_addrs(&(host, port)).map(|a| a.collect::<Vec<_>>())
}

/// Resolve the host within the time left. An address literal needs no resolving.
fn resolve(t: &Target, clock: &Clock) -> Result<Vec<SocketAddr>, Fail> {
    resolve_with(t, clock, system_resolver)
}

/// [`resolve`] through the given resolver.
fn resolve_with(t: &Target, clock: &Clock, resolver: Resolver) -> Result<Vec<SocketAddr>, Fail> {
    let host = t.dial_host().to_string();
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, t.port)]);
    }
    let (tx, rx) = mpsc::channel();
    let (h, port) = (host.clone(), t.port);
    // The system resolver has no deadline of its own: run it on a thread and stop waiting at the exchange's
    // deadline (the thread ends when the resolver returns).
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

/// One connection attempt to one address with a timeout. Production uses `TcpStream::connect_timeout`; it is a
/// parameter so tests can stage an address that silently drops the attempt.
pub(crate) type Attempt = fn(&SocketAddr, Duration) -> std::io::Result<TcpStream>;

/// Connect to the host, trying every address (see the module docs) within [`CONNECT_CAP`] and the time left.
pub(crate) fn connect(t: &Target, clock: &Clock) -> Result<TcpStream, Fail> {
    let key = (t.dial_host().to_string(), t.port);
    let addrs = order(&resolve(t, clock)?, last().get(&key).copied());
    let (a, tcp) = race(&addrs, clock, TcpStream::connect_timeout)?;
    last().insert(key, a);
    Ok(tcp)
}

/// Race the addresses in order: a new attempt every [`STAGGER`] (immediately after a refusal), none
/// cancelled, the first to connect wins. All addresses failing or the connect cap passing is a connection
/// failure naming each address; the deadline passing is a deadline failure.
pub(crate) fn race(addrs: &[SocketAddr], clock: &Clock, attempt: Attempt) -> Result<(SocketAddr, TcpStream), Fail> {
    race_capped(addrs, clock, attempt, CONNECT_CAP)
}

/// [`race`] with the given connect cap (production uses [`CONNECT_CAP`]; tests use a shorter one).
fn race_capped(addrs: &[SocketAddr], clock: &Clock, attempt: Attempt, cap: Duration) -> Result<(SocketAddr, TcpStream), Fail> {
    let began = Instant::now();
    let left = clock.left()?;
    let cap_at = match left {
        Some(left) => began + left.min(cap),
        None => began + cap,
    };
    // When the deadline is nearer than the cap, a timed-out attempt is a deadline failure, however early
    // before it the timeout arrived.
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
                // A refused address does not delay the next one.
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
                    // Past the exchange's deadline the exchange is late, whatever stage it was in.
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

    /// The first address silently drops the attempt (as when a VPN blocks IPv6 by dropping), the second
    /// listens: the second is reached about one stagger in, well within the connect cap, and the dropped
    /// attempt is left to time out on its own.
    #[test]
    fn a_first_address_that_swallows_the_attempt_does_not_hold_up_the_second() {
        let _turn = keep::turn();
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let live = l.local_addr().expect("addr");
        let swallowing = a("127.0.0.2:9");
        fn attempt(to: &SocketAddr, within: Duration) -> std::io::Result<TcpStream> {
            if to.ip() == "127.0.0.2".parse::<IpAddr>().expect("ip") {
                // Dropped: nothing comes back until this attempt's own timeout.
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
        // Both dropped under a short deadline: the deadline runs out while connecting and is reported.
        fn swallow(_: &SocketAddr, within: Duration) -> std::io::Result<TcpStream> {
            std::thread::sleep(within.min(Duration::from_millis(1500)));
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "swallowed"))
        }
        let short = Clock::start(Duration::from_millis(600));
        match race(&[swallowing, a("127.0.0.3:9")], &short, swallow) {
            Err(Fail::Late(d)) => assert_eq!(d, Duration::from_millis(600)),
            other => panic!("{:?}", other.map(|x| x.0)),
        }
        // Timed out just before the deadline (each attempt's timeout is the deadline): still a deadline
        // failure, not a connection failure.
        fn early(_: &SocketAddr, within: Duration) -> std::io::Result<TcpStream> {
            std::thread::sleep(within.saturating_sub(Duration::from_millis(100)));
            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "swallowed"))
        }
        let short = Clock::start(Duration::from_millis(600));
        assert!(matches!(race(&[swallowing, a("127.0.0.3:9")], &short, early), Err(Fail::Late(_))));
        // Refused outright under a short deadline: a connection failure naming each address.
        fn refuse(_: &SocketAddr, _: Duration) -> std::io::Result<TcpStream> {
            Err(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused"))
        }
        let short = Clock::start(Duration::from_millis(600));
        assert!(matches!(race(&[swallowing, a("127.0.0.3:9")], &short, refuse), Err(Fail::Connect(s)) if s.contains("127.0.0.2:9") && s.contains("127.0.0.3:9")));
    }

    /// A resolver that answers in 2 s under a 300 ms deadline: a deadline failure (`Late`) at the deadline,
    /// not at the resolver's pace.
    #[test]
    fn the_deadline_running_out_while_resolving_is_late() {
        let _turn = keep::turn();
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

    /// Closing down while resolving fails at once as closed, not at the deadline, and counts as one cut.
    #[test]
    fn closing_down_while_resolving_is_closed() {
        let _turn = keep::turn();
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

    /// The connect cap running out before the deadline (two dropping addresses, a cap of three staggers under a
    /// 5 s deadline, so the second address is tried a whole stagger or more before the cap even on a slow
    /// machine): a connection failure at the cap, saying no address connected within it and naming each one.
    #[test]
    fn the_connect_cap_running_out_names_every_address_tried() {
        let _turn = keep::turn();
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

    /// Closing down while every address is still connecting fails at once as closed, not at the cap, and counts
    /// as one cut.
    #[test]
    fn closing_down_while_connecting_is_closed() {
        let _turn = keep::turn();
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

    /// The two families alternate from the system's first, duplicates are dropped, and the address that last
    /// connected is tried first.
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
