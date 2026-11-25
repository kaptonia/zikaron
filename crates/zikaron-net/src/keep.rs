use crate::{Fail, Scheme, Stream};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub const IDLE_KEEP: Duration = Duration::from_secs(15);

pub const PER_PLACE: usize = 4;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct Place {
    pub(crate) scheme: Scheme,
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) via: Option<crate::route::Proxy>,
}

pub(crate) struct Conn {
    pub(crate) stream: Stream,
    pub(crate) place: Place,
}

static CLOSING: AtomicBool = AtomicBool::new(false);
static NEXT: AtomicU64 = AtomicU64::new(1);
static FLYING: Mutex<BTreeMap<u64, std::net::TcpStream>> = Mutex::new(BTreeMap::new());
static IDLE: Mutex<BTreeMap<Place, Vec<(Instant, Conn)>>> = Mutex::new(BTreeMap::new());

fn flying() -> std::sync::MutexGuard<'static, BTreeMap<u64, std::net::TcpStream>> {
    FLYING.lock().unwrap_or_else(|e| e.into_inner())
}

fn idle() -> std::sync::MutexGuard<'static, BTreeMap<Place, Vec<(Instant, Conn)>>> {
    IDLE.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
pub(crate) fn turn() -> Option<std::sync::MutexGuard<'static, ()>> {
    static TURN: Mutex<()> = Mutex::new(());
    const CHILD: &str = "ZK_NET_ALONE";
    let name = std::thread::current().name().map(str::to_string);
    match name {
        Some(test) if test != "main" && std::env::var_os(CHILD).is_none() => {
            let me = std::env::current_exe().expect("this test binary");
            let mut c = std::process::Command::new(me);
            c.args([test.as_str(), "--exact", "--nocapture", "--test-threads=1"]).env(CHILD, &test);
            c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
            let out = zikaron_os::spawn(&mut c).and_then(|ch| ch.wait_with_output()).expect("the child runs");
            let said = String::from_utf8_lossy(&out.stdout);
            assert!(out.status.success() && said.contains("1 passed"), "{test}\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
            None
        }
        _ => Some(TURN.lock().unwrap_or_else(|e| e.into_inner())),
    }
}

#[cfg(test)]
pub(crate) fn raise_flag_only() {
    let _f = flying();
    CLOSING.store(true, Ordering::SeqCst);
}

pub(crate) struct Ticket(u64);

impl Drop for Ticket {
    fn drop(&mut self) {
        flying().remove(&self.0);
    }
}

pub(crate) struct Live {
    pub(crate) conn: Conn,
    _ticket: Ticket,
}

pub(crate) fn closing() -> bool {
    CLOSING.load(Ordering::SeqCst)
}

static CUTS: AtomicU64 = AtomicU64::new(0);

pub(crate) fn cut() -> Fail {
    CUTS.fetch_add(1, Ordering::SeqCst);
    Fail::Closed
}

pub fn cuts() -> u64 {
    CUTS.load(Ordering::SeqCst)
}

pub(crate) fn ticket(tcp: &std::net::TcpStream) -> Result<Ticket, Fail> {
    track(tcp)?.ok_or_else(cut)
}

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

pub(crate) fn live(conn: Conn, ticket: Ticket) -> Live {
    Live { conn, _ticket: ticket }
}

pub(crate) fn fly(conn: Conn) -> Result<Live, Fail> {
    let t = track(conn.stream.tcp())?.ok_or(Fail::Closed)?;
    Ok(live(conn, t))
}

pub(crate) fn take(place: &Place) -> Option<Live> {
    let found = {
        let mut idle = idle();
        let kept = idle.get_mut(place)?;
        kept.retain(|(since, _)| since.elapsed() < IDLE_KEEP);
        kept.pop().map(|(_, c)| c)
    };
    fly(found?).ok()
}

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

pub fn close_down() {
    {
        let mut f = flying();
        CLOSING.store(true, Ordering::SeqCst);
        for (_, s) in std::mem::take(&mut *f) {
            let _ = s.shutdown(std::net::Shutdown::Both);
        }
    }
    idle().clear();
    crate::route::wake();
}

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

    fn conn(l: &TcpListener, place: &Place) -> (Conn, u16) {
        let tcp = TcpStream::connect(l.local_addr().expect("addr")).expect("connect");
        let port = tcp.local_addr().expect("addr").port();
        (Conn { stream: Stream::Plain(crate::Sock::Tcp(tcp)), place: place.clone() }, port)
    }

    fn port_of(c: &Conn) -> u16 {
        c.stream.tcp().local_addr().expect("addr").port()
    }

    #[test]
    fn at_most_per_place_idle_connections_are_kept_the_newest() {
        let Some(_turn) = turn() else { return };
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

    #[test]
    fn an_idle_connection_older_than_idle_keep_is_not_taken() {
        let Some(_turn) = turn() else { return };
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

    #[test]
    fn a_kept_connection_refused_while_shutting_down_is_not_a_cut() {
        let Some(_turn) = turn() else { return };
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

    #[test]
    fn closing_down_clears_the_idle_connections() {
        let Some(_turn) = turn() else { return };
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
