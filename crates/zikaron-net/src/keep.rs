//! Connection reuse between requests, and shutdown.
//!
//! An idle connection is kept per [`Place`] (scheme, host, port, proxy) for a short while and handed to the
//! next request to the same place; one that had an error, timed out, or whose peer asked to close is never
//! kept. Every connection in use is tracked, so shutting down (the app quitting) cuts them within a slice and
//! refuses any exchange that starts afterwards.

use crate::{Fail, Scheme, Stream};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How long an idle connection is kept for reuse. Shorter than the idle timeout public nodes and their
/// gateways commonly use, so a kept connection is seldom found closed.
pub const IDLE_KEEP: Duration = Duration::from_secs(15);

/// Maximum idle connections kept per place (enough for every kind of task that may query the same node at
/// once).
pub const PER_PLACE: usize = 4;

/// Where a connection goes, and through which proxy (`None`: direct); a connection is only reused for a
/// request with the same route.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Place {
    pub(crate) scheme: Scheme,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) via: Option<crate::route::Proxy>,
}

/// A connection with its place.
pub(crate) struct Conn {
    pub(crate) stream: Stream,
    pub(crate) place: Place,
}

/// Set while shutting down; read under `FLYING`'s lock so each connection is either cut or refused.
static CLOSING: AtomicBool = AtomicBool::new(false);
static NEXT: AtomicU64 = AtomicU64::new(1);
/// Every connection in use, by ticket: a second handle on its socket, so shutdown can cut it.
static FLYING: Mutex<BTreeMap<u64, std::net::TcpStream>> = Mutex::new(BTreeMap::new());
static IDLE: Mutex<BTreeMap<Place, Vec<(Instant, Conn)>>> = Mutex::new(BTreeMap::new());

fn flying() -> std::sync::MutexGuard<'static, BTreeMap<u64, std::net::TcpStream>> {
    FLYING.lock().unwrap_or_else(|e| e.into_inner())
}

fn idle() -> std::sync::MutexGuard<'static, BTreeMap<Place, Vec<(Instant, Conn)>>> {
    IDLE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Tests that shut down, and tests that would observe the flag meanwhile, run serially: the flag is
/// process-wide.
#[cfg(test)]
pub(crate) fn turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: Mutex<()> = Mutex::new(());
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

/// Raise the shutdown flag alone, touching no socket: shows a cut rests on the flag, not on the system waking
/// a blocked read. [`open_up`] lowers it.
#[cfg(test)]
pub(crate) fn raise_flag_only() {
    let _f = flying();
    CLOSING.store(true, Ordering::SeqCst);
}

/// A connection's entry in the in-use list; removed when dropped.
pub(crate) struct Ticket(u64);

impl Drop for Ticket {
    fn drop(&mut self) {
        flying().remove(&self.0);
    }
}

/// A connection in use, tracked until it is dropped or kept for reuse.
pub(crate) struct Live {
    pub(crate) conn: Conn,
    _ticket: Ticket,
}

/// Whether the transport is shutting down.
pub(crate) fn closing() -> bool {
    CLOSING.load(Ordering::SeqCst)
}

/// How many exchanges shutting down has cut while they were under way (resolving, connecting, in a handshake or
/// a proxy tunnel, waiting for or reading an answer) in this process.
static CUTS: AtomicU64 = AtomicU64::new(0);

/// An exchange under way is cut by the shutdown: counted ([`cuts`]) and said as closed. Every wait that finds
/// the flag raised ends through here; an exchange refused before it began is not a cut.
pub(crate) fn cut() -> Fail {
    CUTS.fetch_add(1, Ordering::SeqCst);
    Fail::Closed
}

/// How many exchanges under way shutting down has cut in this process: the trace a cut leaves, so whoever must
/// tell a cut from an exchange that ran out its deadline (or broke) reads a count, not a clock.
pub fn cuts() -> u64 {
    CUTS.load(Ordering::SeqCst)
}

/// Track a socket as in use from the moment it connects (handshake included); refused while shutting down. The
/// connection was being made, so a refusal here cuts an exchange under way ([`cut`]).
pub(crate) fn ticket(tcp: &std::net::TcpStream) -> Result<Ticket, Fail> {
    track(tcp)?.ok_or_else(cut)
}

/// Track a socket as in use: `None` while shutting down (the socket shut), counting nothing; whether that
/// refusal cut an exchange under way is the caller's to say.
fn track(tcp: &std::net::TcpStream) -> Result<Option<Ticket>, Fail> {
    let handle = tcp.try_clone().map_err(|e| Fail::Stream(e.to_string()))?;
    let mut f = flying();
    if closing() {
        let _ = handle.shutdown(std::net::Shutdown::Both);
        return Ok(None);
    }
    let ticket = NEXT.fetch_add(1, Ordering::SeqCst);
    f.insert(ticket, handle);
    Ok(Some(Ticket(ticket)))
}

/// A connection in use under the ticket its socket was tracked with.
pub(crate) fn live(conn: Conn, ticket: Ticket) -> Live {
    Live { conn, _ticket: ticket }
}

/// Track a kept connection as in use again; refused while shutting down. Nothing has been asked on it yet, so
/// the refusal is not a cut.
pub(crate) fn fly(conn: Conn) -> Result<Live, Fail> {
    let t = track(conn.stream.tcp())?.ok_or(Fail::Closed)?;
    Ok(live(conn, t))
}

/// The newest idle connection to this place still within [`IDLE_KEEP`], now tracked as in use.
pub(crate) fn take(place: &Place) -> Option<Live> {
    let found = {
        let mut idle = idle();
        let kept = idle.get_mut(place)?;
        kept.retain(|(since, _)| since.elapsed() < IDLE_KEEP);
        kept.pop().map(|(_, c)| c)
    };
    fly(found?).ok()
}

/// Keep a connection whose response was complete and whose peer left it open; beyond [`PER_PLACE`] the oldest
/// is dropped. Nothing is kept while shutting down.
pub(crate) fn put(live: Live) {
    let Live { conn, _ticket } = live;
    drop(_ticket);
    if closing() {
        return;
    }
    let mut idle = idle();
    let kept = idle.entry(conn.place.clone()).or_default();
    kept.retain(|(since, _)| since.elapsed() < IDLE_KEEP);
    kept.push((Instant::now(), conn));
    while kept.len() > PER_PLACE {
        kept.remove(0);
    }
}

/// Shut down: every exchange in flight is cut within one [`crate::SLICE`] and fails with a named error, idle
/// connections are dropped, and any exchange started afterwards fails before opening anything. The app calls
/// this when it quits, before waiting for its tasks; the CLI never does.
///
/// The cut rests on the flag: every wait in an exchange looks at it after a slice at most. Shutting the sockets
/// in flight only wakes a blocked read sooner where the system does so; nothing depends on it.
pub fn close_down() {
    {
        let mut f = flying();
        CLOSING.store(true, Ordering::SeqCst);
        for (_, s) in std::mem::take(&mut *f) {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }
    idle().clear();
    // A connection waiting for the system's proxy settings stops waiting.
    crate::route::wake();
}

/// Reopen after [`close_down`], so a new app session can use the transport.
pub fn open_up() {
    let _f = flying();
    CLOSING.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{TcpListener, TcpStream};

    fn place(host: &str) -> Place {
        Place { scheme: Scheme::Http, host: host.into(), port: 8545, via: None }
    }

    /// A new connection to a local listener, for `place`; its local port identifies it.
    fn conn(l: &TcpListener, place: &Place) -> (Conn, u16) {
        let tcp = TcpStream::connect(l.local_addr().expect("addr")).expect("connect");
        let port = tcp.local_addr().expect("addr").port();
        (Conn { stream: Stream::Plain(tcp), place: place.clone() }, port)
    }

    fn port_of(c: &Conn) -> u16 {
        c.stream.tcp().local_addr().expect("addr").port()
    }

    /// At most `PER_PLACE` idle connections per place: the oldest is dropped, and the newest is handed out
    /// first.
    #[test]
    fn at_most_per_place_idle_connections_are_kept_the_newest() {
        let _turn = turn();
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        let at = place("per-place.test");
        let mut ports = Vec::new();
        for _ in 0..PER_PLACE + 2 {
            let (c, p) = conn(&l, &at);
            ports.push(p);
            put(fly(c).expect("listed"));
        }
        let kept: Vec<u16> = idle().get(&at).map(|v| v.iter().map(|(_, c)| port_of(c)).collect()).unwrap_or_default();
        assert_eq!(PER_PLACE, 4);
        assert_eq!(kept, ports[2..].to_vec(), "the four newest kept, oldest first");
        assert_eq!(take(&at).map(|l| port_of(&l.conn)), ports.last().copied(), "the newest is handed out first");
    }

    /// An idle connection older than `IDLE_KEEP` is dropped, not reused; a slightly younger one is reused. The
    /// timestamp is backdated in the idle list itself.
    #[test]
    fn an_idle_connection_older_than_idle_keep_is_not_taken() {
        let _turn = turn();
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        let at = place("idle-keep.test");
        let (old, _) = conn(&l, &at);
        let since = Instant::now().checked_sub(IDLE_KEEP + Duration::from_millis(100)).expect("an instant that long ago");
        idle().entry(at.clone()).or_default().push((since, old));
        assert!(take(&at).is_none(), "older than IDLE_KEEP: not taken");
        assert!(idle().get(&at).is_some_and(|v| v.is_empty()), "and let go");
        let (within, p) = conn(&l, &at);
        let since = Instant::now().checked_sub(IDLE_KEEP - Duration::from_secs(1)).expect("an instant that long ago");
        idle().entry(at.clone()).or_default().push((since, within));
        assert_eq!(take(&at).map(|l| port_of(&l.conn)), Some(p), "inside IDLE_KEEP: taken");
    }

    /// While shutting down, taking a kept connection is refused and counts no cut (nothing was asked on it);
    /// tracking a connection just made is refused as the cut of the exchange making it, counted once.
    #[test]
    fn a_kept_connection_refused_while_shutting_down_is_not_a_cut() {
        let _turn = turn();
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        let at = place("refused-kept.test");
        let (kept, _) = conn(&l, &at);
        let (fresh, _) = conn(&l, &at);
        raise_flag_only();
        idle().entry(at.clone()).or_default().push((Instant::now(), kept));
        let before = cuts();
        let taken = take(&at);
        let after_take = cuts();
        let tracked = ticket(fresh.stream.tcp());
        let after_ticket = cuts();
        idle().clear();
        open_up();
        assert!(taken.is_none(), "refused while shutting down");
        assert_eq!(after_take, before, "taking a kept connection is no cut");
        assert!(matches!(tracked, Err(Fail::Closed)), "tracking a new one is refused as closed");
        assert_eq!(after_ticket, before + 1, "and that exchange is cut, once");
    }

    /// Shutdown drops every idle connection: one kept before shutdown is not handed out after reopening.
    #[test]
    fn closing_down_clears_the_idle_connections() {
        let _turn = turn();
        let l = TcpListener::bind("127.0.0.1:0").expect("bind");
        let at = place("close-down.test");
        let (c, _) = conn(&l, &at);
        put(fly(c).expect("listed"));
        assert_eq!(idle().get(&at).map(|v| v.len()), Some(1), "kept before closing down");
        close_down();
        open_up();
        assert!(take(&at).is_none(), "not handed out after opening again");
    }
}
