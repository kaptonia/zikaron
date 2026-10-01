//! Single writer: with two instances on one machine, the second is read-only.
//!
//! ─── Stale locks do not exist ───
//!
//! Using a "lock file" as the lock is the asymptote: after a process is killed (SIGKILL, power loss, crash)
//! the file remains, and another judgment is needed on "is this lock stale", guessing by pid, by timestamp or
//! by heartbeat. Each can be wrong: pids get reused by other processes, timestamps do not survive clock
//! rollback, and heartbeats need state of their own. When such a judgment guesses wrong, the result is two
//! writers open at once, exactly what it was meant to prevent.
//!
//! The design hands the lock's lifetime to the kernel: `flock` is an advisory lock on an open file
//! descriptor, and when the process goes away the kernel releases it at once, SIGKILL and power loss
//! included. So "stale lock" has nowhere to exist: getting it means nobody holds it, failing means somebody
//! does, the product has not one line that deletes lock files, and nobody needs to delete them by hand.
//!
//! The two lines in the file (pid and start time) are only words for the face ("who holds it") and never take
//! part in the decision. The decision is `flock`'s.

use crate::fault::{classify, Fault};
use crate::home::{Home, Slot};
use std::path::PathBuf;


/// The lock file's name. One name, one home.
pub const LOCK_FILE: &str = "writer.lock";

/// This instance's identity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Writer: this home is written by it.
    Writer,
    /// Reader: another instance holds it; this one is read-only.
    Reader,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Writer => "writer",
            Mode::Reader => "reader",
        }
    }

    pub fn writable(self) -> bool {
        matches!(self, Mode::Writer)
    }
}

/// A lock in hand. While it lives, the lock holds; when it goes (or the process goes), the lock is released.
pub struct Lock {
    _file: std::fs::File,
    mode: Mode,
    path: PathBuf,
    holder: String,
}

impl Lock {
    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// The face's "who holds it". Only words; the decision does not look at it.
    pub fn holder(&self) -> &str {
        &self.holder
    }
}

/// Ask the kernel for an exclusive lock on this descriptor, without waiting. True when obtained.
///
/// The whole single-writer rule rests on this line: a kernel lock is released when the process goes, so stale
/// locks cannot arise.
pub fn grab(file: &std::fs::File) -> bool {
    crate::platform::lock_now(file)
}

/// The same primitive, waiting until it is obtained (used by the key vault file's lock).
///
/// The two lifetimes have separate uses: the home's writer lock does not wait (failing means another instance
/// holds it and this one becomes a reader, which must be told to the person at once); the vault file's lock
/// waits (a passcode attempt should queue, not be judged "wrong passcode" because another process is changing
/// the vault). The kernel still releases the lock when the process goes, so waiting cannot produce a stale
/// lock.
pub fn grab_waiting(file: &std::fs::File) -> bool {
    crate::platform::lock_wait(file)
}

/// Where a home's lock file lives. One name, one home: taking the lock and "is this lock in hand this home's"
/// both ask it.
pub fn path_of(home: &Home) -> PathBuf {
    home.dir(Slot::Settings).join(LOCK_FILE)
}

/// Whether the lock in hand is this home's writer lock.
///
/// `flock` is tied to open file descriptors: opening a second descriptor to the same lock file in the same
/// process and taking it would be treated by the kernel as another holder, so re-entering one's own open home
/// (switching seats back and forth, entering a home after import) would judge itself a reader. The lock is
/// held once per process, and re-entering the same home reuses the lock in hand. Both paths are resolved
/// first (aliases such as `/tmp` and `/private/tmp` count as the same place).
pub fn holds_writer(held: Option<&Lock>, home: &Home) -> bool {
    let Some(l) = held else { return false };
    if !l.mode().writable() {
        return false;
    }
    crate::home::same_place(l.path(), &path_of(home))
}

/// Take the lock. Obtained means writer, not obtained means reader. Both paths return a `Lock`; there is no
/// third.
pub fn take(home: &Home) -> Result<Lock, Fault> {
    let path = path_of(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| classify(&e, &parent.display().to_string()))?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| classify(&e, &path.display().to_string()))?;
    let got = grab(&file);
    if got {
        let me = format!("pid {}", std::process::id());
        // Written only so the face has something to say; failing to write does not change the decision.
        use std::io::{Seek, Write};
        let mut h = &file;
        let _ = h.set_len(0);
        let _ = h.rewind();
        let _ = h.write_all(me.as_bytes());
        let _ = h.flush();
        Ok(Lock { _file: file, mode: Mode::Writer, path, holder: me })
    } else {
        let holder = std::fs::read_to_string(&path).unwrap_or_default();
        let holder = if holder.trim().is_empty() { crate::lang::t(crate::lang::Key::Tail184).to_string() } else { holder };
        Ok(Lock { _file: file, mode: Mode::Reader, path, holder })
    }
}
