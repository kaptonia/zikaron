//! Local IPC endpoint (the "door"): a channel one process keeps open for other processes of the same user on
//! this machine. On unix it is a socket file in the machine directory, accessible to its owner only; on Windows
//! a named pipe whose ACL allows only this user and rejects remote clients. Once a peer connects, the OS is
//! asked which user the other end runs as, and another user is refused on both sides (the listener refuses a
//! foreign client; the client refuses an endpoint owned by another user). There is no token: anyone able to
//! read one could already reach the endpoint.
//!
//! This module decides where an endpoint lives ([`place`]); the protocol over it is the caller's.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

/// Prefix of every endpoint name (the token follows).
pub const NAME_HEAD: &str = "zikaron-door-";

/// The location of the endpoint named by `token`, for machine directory `machine`.
pub fn place(machine: &Path, token: &str) -> PathBuf {
    crate::imp::door_place(machine, &format!("{NAME_HEAD}{token}"))
}

/// Maximum endpoint path length in bytes on this OS (unix: a local socket path).
pub const PLACE_MAX: usize = crate::imp::DOOR_PATH_MAX;

/// Whether `place` is too long for an endpoint on this OS; such a path is refused (`InvalidInput`), never
/// truncated.
pub fn too_long(place: &Path) -> bool {
    place.as_os_str().len() > PLACE_MAX
}

static STRANGER: AtomicBool = AtomicBool::new(false);

/// Treat every peer as another user from now on in this process (`false`: ask the OS). For tests of both
/// refusals; production never calls it. It is asked before the OS on both sides, so a pretended stranger is
/// refused the same way on every system, whatever the other end does meanwhile.
pub fn pretend_stranger(on: bool) {
    STRANGER.store(on, Ordering::SeqCst);
}

fn stranger() -> std::io::Result<()> {
    if STRANGER.load(Ordering::SeqCst) {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "the other end is another user (pretended)"));
    }
    Ok(())
}

/// An open endpoint; [`Listener::accept`] waits for the next client.
pub struct Listener {
    door: Arc<crate::imp::DoorListener>,
    shut: Arc<AtomicBool>,
}

/// Closes an endpoint from a thread other than the one waiting on it.
#[derive(Clone)]
pub struct Closer {
    place: PathBuf,
    shut: Arc<AtomicBool>,
    /// The listening side while it lives, so closing ends it on the spot rather than when its thread lets go.
    door: Weak<crate::imp::DoorListener>,
}

/// One connection, on either side: a bidirectional byte stream.
pub struct Stream(crate::imp::DoorStream);

/// Open the endpoint at `place`. A stale endpoint left by a dead process (a socket file nobody answers) is
/// removed first; a live endpoint, or anything else at the path, is `AddrInUse` and left untouched; a path too
/// long for this OS is `InvalidInput`.
pub fn listen(place: &Path) -> std::io::Result<(Listener, Closer)> {
    if too_long(place) {
        return Err(crate::named("door", place.display(), std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("longer than {PLACE_MAX} bytes"))));
    }
    let door = Arc::new(crate::imp::door_listen(place).map_err(|e| crate::named("door", place.display(), e))?);
    let shut = Arc::new(AtomicBool::new(false));
    Ok((Listener { door: door.clone(), shut: shut.clone() }, Closer { place: place.to_path_buf(), shut, door: Arc::downgrade(&door) }))
}

impl Listener {
    /// The next client, or `None` once the endpoint is closed ([`Closer::close`]). A client running as another
    /// user is refused (`PermissionDenied`) and the endpoint stays open. Taking the connection and judging who
    /// is at the other end are two steps; the test hook ([`pretend_stranger`]) is asked between them, before
    /// the OS, so a client that hangs up meanwhile does not change the answer.
    pub fn accept(&self) -> std::io::Result<Option<Stream>> {
        // Closed already: the wake-up the close sent may have been taken by an earlier `accept`, so this one is
        // not left waiting for a client that can no longer come.
        if self.shut.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let got = self.door.accept();
        if self.shut.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let s = got?;
        stranger()?;
        s.same_user()?;
        Ok(Some(Stream(s)))
    }
}

impl Closer {
    /// Close the endpoint: a waiting `accept` wakes and returns `None` (as does every `accept` after), and once
    /// this returns nobody new is taken: a connect finds nothing there (`NotFound`), at once where the endpoint
    /// is a path, and where it is a pipe once a conversation accepted earlier is over (that one instance keeps
    /// the name busy meanwhile). Connections accepted earlier remain the caller's to finish. Closing twice does
    /// nothing more.
    pub fn close(&self) {
        if !self.shut.swap(true, Ordering::SeqCst) {
            crate::imp::door_close(&self.place, self.door.upgrade().as_deref());
        }
    }

    /// Whether [`Closer::close`] was called.
    pub fn closed(&self) -> bool {
        self.shut.load(Ordering::SeqCst)
    }
}

/// Connect to the endpoint at `place`: none there (or a stale one) is `NotFound`; one owned by another user is
/// `PermissionDenied`; a path too long for this OS is `InvalidInput`.
pub fn connect(place: &Path) -> std::io::Result<Stream> {
    if too_long(place) {
        return Err(crate::named("door", place.display(), std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("longer than {PLACE_MAX} bytes"))));
    }
    let s = crate::imp::door_connect(place).map_err(|e| crate::named("door", place.display(), e))?;
    stranger().map_err(|e| crate::named("door", place.display(), e))?;
    s.same_user().map_err(|e| crate::named("door", place.display(), e))?;
    Ok(Stream(s))
}

impl Stream {
    /// How long a read waits before returning `TimedOut` (`None`: forever).
    pub fn set_read_deadline(&self, d: Option<std::time::Duration>) -> std::io::Result<()> {
        self.0.set_read_deadline(d)
    }
}

impl std::io::Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl std::io::Write for Stream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// `pretend_stranger` is process-wide, so these tests run serially.
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn scratch(tag: &str) -> PathBuf {
        // Kept short: local socket paths have a small length limit.
        let d = std::env::temp_dir().join(format!("zkd-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("scratch");
        d
    }

    /// Bytes flow both ways; closing wakes the waiter and removes the path; a closed endpoint cannot be reached.
    #[test]
    fn a_door_carries_bytes_both_ways_and_closes() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("both");
        let at = place(&d, "t1");
        let (l, closer) = listen(&at).expect("open");
        let server = std::thread::spawn(move || {
            let mut s = l.accept().expect("accept").expect("open still");
            let mut b = [0u8; 4];
            s.read_exact(&mut b).expect("read");
            s.write_all(&[b[0] + 1, b[1] + 1, b[2] + 1, b[3] + 1]).expect("write");
            drop(s);
            // Closed while waiting: woken, and returns `None`.
            l.accept().expect("woken")
        });
        let mut c = connect(&at).expect("in");
        c.write_all(&[1, 2, 3, 4]).expect("write");
        let mut back = [0u8; 4];
        c.read_exact(&mut back).expect("read");
        assert_eq!(back, [2, 3, 4, 5]);
        std::thread::sleep(std::time::Duration::from_millis(50));
        closer.close();
        assert!(server.join().expect("server").is_none(), "a closed door says so");
        assert_eq!(connect(&at).err().map(|e| e.kind()), Some(std::io::ErrorKind::NotFound), "nobody at a closed door");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A stale endpoint from a dead process is replaced when a new one opens; a live one is not.
    #[cfg(unix)]
    #[test]
    fn a_door_left_behind_is_replaced_and_an_open_one_is_not() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("left");
        let at = place(&d, "t2");
        // Stale: opened and never closed by a process that stopped (the path exists, nobody answers).
        let (left, _never_closed) = listen(&at).expect("left");
        drop(left);
        assert!(at.exists());
        let (_l, closer) = listen(&at).expect("a door left behind is replaced");
        assert_eq!(listen(&at).err().map(|e| e.kind()), Some(std::io::ErrorKind::AddrInUse), "an open door is not taken");
        closer.close();
        // Anything other than an endpoint at the path is never removed.
        std::fs::write(&at, b"not a door").expect("file");
        assert_eq!(listen(&at).err().map(|e| e.kind()), Some(std::io::ErrorKind::AddrInUse));
        assert_eq!(std::fs::read(&at).expect("kept"), b"not a door");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The socket file is accessible to its owner only.
    #[cfg(unix)]
    #[test]
    fn the_door_is_its_owners_only() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("own");
        let at = place(&d, "t3");
        let (_l, closer) = listen(&at).expect("open");
        assert!(crate::is_owner_only(&at).expect("read"));
        closer.close();
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A path too long for this OS is refused by name for both listen and connect, never truncated.
    #[test]
    fn a_place_too_long_is_refused_by_name() {
        let long = std::env::temp_dir().join("d".repeat(PLACE_MAX + 1));
        assert!(too_long(&long));
        assert_eq!(listen(&long).err().map(|e| e.kind()), Some(std::io::ErrorKind::InvalidInput));
        assert_eq!(connect(&long).err().map(|e| e.kind()), Some(std::io::ErrorKind::InvalidInput));
        assert!(!long.exists());
    }

    /// Another user is refused on both sides: the client is told, and the endpoint stays open.
    #[test]
    fn another_user_is_refused_both_ways() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("other");
        let at = place(&d, "t4");
        let (l, closer) = listen(&at).expect("open");
        let (tx, rx) = std::sync::mpsc::channel();
        let server = std::thread::spawn(move || {
            let _ = tx.send(l.accept().map(|s| s.is_some()).map_err(|e| e.kind()));
            l.accept().map(|s| s.is_some()).map_err(|e| e.kind())
        });
        pretend_stranger(true);
        let refused = connect(&at).err().map(|e| e.kind());
        let first = rx.recv().expect("first");
        pretend_stranger(false);
        assert_eq!(refused, Some(std::io::ErrorKind::PermissionDenied), "the one who came in refused a door kept by a stranger");
        assert_eq!(first, Err(std::io::ErrorKind::PermissionDenied), "the door refused the stranger");
        closer.close();
        assert_eq!(server.join().expect("server"), Ok(false), "and was still open after it");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Once `close` returns, a connect finds nothing and the waiter returns `None`. Repeated because a race
    /// would show only some of the time.
    #[test]
    fn once_closed_nobody_reaches_the_door() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("gone");
        let at = place(&d, "t6");
        for round in 0..20 {
            let (l, closer) = listen(&at).expect("open");
            let server = std::thread::spawn(move || l.accept().map(|s| s.is_some()).map_err(|e| e.kind()));
            if round % 2 == 0 {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            closer.close();
            assert_eq!(connect(&at).err().map(|e| e.kind()), Some(std::io::ErrorKind::NotFound), "round {round}: nobody at a closed door");
            assert_eq!(server.join().expect("server"), Ok(false), "round {round}: the waiter is told the door closed");
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A client already in the doorway (connected, not yet taken) when the door closes is never served: no
    /// byte reaches it, and the door gives `None` rather than taking it.
    #[test]
    fn a_client_in_the_doorway_when_it_closes_is_not_served() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("doorway");
        let at = place(&d, "t7");
        let (l, closer) = listen(&at).expect("open");
        let mut c = connect(&at).expect("in the doorway");
        closer.close();
        assert!(l.accept().expect("closed").is_none(), "not taken after the close");
        drop(l);
        // macOS refuses a read deadline on a socket its peer has already reset: that is the no-byte answer too.
        if c.set_read_deadline(Some(std::time::Duration::from_millis(500))).is_ok() {
            let mut b = [0u8; 1];
            assert!(matches!(c.read(&mut b), Ok(0) | Err(_)), "no byte for a client the closed door never took");
        }
        assert_eq!(connect(&at).err().map(|e| e.kind()), Some(std::io::ErrorKind::NotFound));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A connection accepted before the close still carries bytes both ways; a new client is never taken and
    /// gets `NotFound` (on Windows only after the open pipe instance is done).
    #[test]
    fn a_conversation_under_way_finishes_after_the_door_closes() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("talk");
        let at = place(&d, "t8");
        let (l, closer) = listen(&at).expect("open");
        let server = std::thread::spawn(move || l.accept().expect("accept").expect("open still"));
        let mut c = connect(&at).expect("in");
        let mut s = server.join().expect("server");
        closer.close();
        let late_at = at.clone();
        let late = std::thread::spawn(move || connect(&late_at).err().map(|e| e.kind()));
        c.write_all(&[7]).expect("write");
        let mut b = [0u8; 1];
        s.read_exact(&mut b).expect("read");
        s.write_all(&[b[0] + 1]).expect("answer");
        c.read_exact(&mut b).expect("read the answer");
        assert_eq!(b, [8]);
        drop(s);
        drop(c);
        assert_eq!(late.join().expect("the newcomer"), Some(std::io::ErrorKind::NotFound), "never taken; told the door is gone");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Closing twice is harmless: no hang, and every later `accept` returns `None`.
    #[test]
    fn closing_twice_does_nothing_more() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("twice");
        let at = place(&d, "t9");
        let (l, closer) = listen(&at).expect("open");
        let other = closer.clone();
        closer.close();
        other.close();
        assert!(closer.closed() && other.closed());
        assert!(l.accept().expect("closed").is_none());
        assert!(l.accept().expect("closed").is_none(), "asked again after the close: `None`, never a wait");
        assert_eq!(connect(&at).err().map(|e| e.kind()), Some(std::io::ErrorKind::NotFound));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A closed endpoint can be reopened at the same place at once, even before the old listener is dropped.
    #[test]
    fn a_closed_door_opens_again_at_the_same_place() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("again");
        let at = place(&d, "t10");
        let (old, closer) = listen(&at).expect("open");
        closer.close();
        let (l, again) = listen(&at).expect("open again at once");
        let server = std::thread::spawn(move || {
            let mut s = l.accept().expect("accept").expect("the new door");
            let mut b = [0u8; 1];
            s.read_exact(&mut b).expect("read");
            b[0]
        });
        connect(&at).expect("reaches the new door").write_all(&[5]).expect("write");
        assert_eq!(server.join().expect("server"), 5);
        assert!(old.accept().expect("closed").is_none(), "the old one stays closed");
        again.close();
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A client that hangs up before the user check is not reported as another user; with `pretend_stranger`
    /// on, it is refused regardless, since the hook is asked before the OS.
    #[test]
    fn a_client_gone_before_it_is_judged() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("gone-early");
        let at = place(&d, "t11");
        let (l, closer) = listen(&at).expect("open");
        drop(crate::imp::door_connect(&at).expect("in and out"));
        let first = l.accept().map(|s| s.is_some()).map_err(|e| e.kind());
        assert_ne!(first, Err(std::io::ErrorKind::PermissionDenied), "a client of this user that left is not a stranger");
        pretend_stranger(true);
        drop(crate::imp::door_connect(&at).expect("in and out"));
        let pretended = l.accept().map(|s| s.is_some()).map_err(|e| e.kind());
        pretend_stranger(false);
        assert_eq!(pretended, Err(std::io::ErrorKind::PermissionDenied), "the hook is asked before the system");
        let server = std::thread::spawn(move || l.accept().map(|s| s.is_some()).map_err(|e| e.kind()));
        let _c = connect(&at).expect("the next one comes in");
        assert_eq!(server.join().expect("server"), Ok(true), "and is taken");
        closer.close();
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A read with a deadline returns `TimedOut` when nothing arrives.
    #[test]
    fn a_read_waits_no_longer_than_its_deadline() {
        let _turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let d = scratch("dl");
        let at = place(&d, "t5");
        let (l, closer) = listen(&at).expect("open");
        let server = std::thread::spawn(move || {
            let mut s = l.accept().expect("accept").expect("open");
            s.set_read_deadline(Some(std::time::Duration::from_millis(100))).expect("deadline");
            let mut b = [0u8; 1];
            s.read(&mut b).map_err(|e| e.kind())
        });
        let _c = connect(&at).expect("in");
        assert_eq!(server.join().expect("server").err(), Some(std::io::ErrorKind::TimedOut));
        closer.close();
        let _ = std::fs::remove_dir_all(&d);
    }
}
