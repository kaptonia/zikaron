//! Single writer: when two instances open the same home on one machine, the second is read-only.
//!
//! ─── No stale locks ───
//!
//! A lock that is just a file outlives a killed process (SIGKILL, power loss, crash), and deciding whether it
//! is stale means guessing by pid, timestamp or heartbeat. Each can be wrong (pids are reused, clocks roll
//! back, heartbeats need their own state), and a wrong guess yields two writers at once.
//!
//! Instead the lock's lifetime belongs to the kernel: `flock` is an advisory lock on an open file descriptor,
//! released as soon as the process ends, SIGKILL and power loss included. Getting it means nobody holds it;
//! failing means somebody does. The app never deletes lock files, and nobody needs to by hand.
//!
//! The holder text in the file is only for display ("who holds it") and never affects the decision.
//!
//! ─── Another machine ───
//!
//! A kernel lock only works on one machine: a data folder on a synced or shared disk opened on two machines
//! would get a writer on each, and the two ledgers would diverge. So each home keeps a writer mark
//! ([`MARK_FILE`]): one line naming the machine that writes it (this machine's mark, [`this_machine`], lives
//! in the machine directory, outside every data folder). Opening a home whose mark names another machine
//! makes this instance a reader ([`Mode::OtherMachine`]), until the user explicitly chooses "write from this
//! machine" ([`Lock::take_over`]), which rewrites the mark. There is no heartbeat or expiry, since both depend
//! on the wall clock and sync delays, which cannot be relied on.

use crate::fault::{classify, Fault};
use crate::home::{Home, Slot};
use std::path::PathBuf;


/// The lock file's name.
pub const LOCK_FILE: &str = "writer.lock";

/// The writer mark's file name (in a home's `settings/` directory): which machine writes this home.
pub const MARK_FILE: &str = "writer-mark";

/// The file name of this machine's mark (in the machine directory, outside every data folder).
pub const MACHINE_FILE: &str = "machine-mark";

/// This instance's access mode for a home.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Writer: this instance writes the home.
    Writer,
    /// Reader: another instance on this machine holds the lock.
    Reader,
    /// Reader: the home's writer mark names another machine, or cannot be read by this version (a read error,
    /// a wrong shape, or a later version's format: [`Mark::Unread`], reported via [`Lock::holder`] and
    /// [`Lock::mark_trouble`]; the mark is never rewritten). This instance holds the kernel lock, and writes
    /// only after the user takes the home over ([`Lock::take_over`]).
    OtherMachine,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Writer => "writer",
            Mode::Reader => "reader",
            Mode::OtherMachine => "other-machine",
        }
    }

    pub fn writable(self) -> bool {
        matches!(self, Mode::Writer)
    }
}

/// A held lock, released when this value is dropped or the process exits.
pub struct Lock {
    _file: Held,
    mode: Mode,
    path: PathBuf,
    holder: String,
    /// Why the home's writer mark could not be read ([`Mark::Unread`]), when that made this lock a reader.
    trouble: Option<Fault>,
}

impl Lock {
    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// "Who holds it", for display only; never used in the decision.
    pub fn holder(&self) -> &str {
        &self.holder
    }

    /// Why the home's writer mark could not be read, when this lock is a reader for that ([`Mark::Unread`]).
    pub fn mark_trouble(&self) -> Option<&Fault> {
        self.trouble.as_ref()
    }

    /// The user's "write from this machine": rewrite the home's writer mark to this machine and become a writer.
    /// Only applies to [`Mode::OtherMachine`]; a reader blocked by another local instance cannot take over,
    /// since the kernel lock decides that.
    pub fn take_over(&mut self, home: &Home) -> Result<(), Fault> {
        if self.mode != Mode::OtherMachine {
            return Ok(());
        }
        write_mark(home)?;
        self.mode = Mode::Writer;
        self.holder = format!("pid {}", std::process::id());
        self.trouble = None;
        Ok(())
    }
}

/// This machine's mark, kept in the machine directory and created on first use (a random number that reveals
/// nothing about the machine). Created once: the write refuses an existing file, and callers in this process
/// are serialized, so two first callers never create two marks.
pub fn this_machine() -> Result<String, Fault> {
    static MAKING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = MAKING.lock().unwrap_or_else(|e| e.into_inner());
    let m = crate::home::machine_dir()?;
    let at = m.join(MACHINE_FILE);
    let read = || -> Result<Option<String>, Fault> {
        match std::fs::read_to_string(&at) {
            Ok(s) if is_mark(s.trim()) => Ok(Some(s.trim().to_string())),
            Ok(_) => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(classify(&e, &at.display().to_string())),
        }
    };
    if let Some(mark) = read()? {
        return Ok(mark);
    }
    let mut n = [0u8; 16];
    crate::key::fill_random(&mut n)?;
    let mark = zikaron::hexfmt::encode(&n).trim_start_matches("0x").to_ascii_lowercase();
    std::fs::create_dir_all(&m).map_err(|e| classify(&e, &m.display().to_string()))?;
    // Not a secret, written like any file; `land_bytes` refuses if the file already exists.
    match zikaron_glue::landing::land_bytes(&at, format!("{mark}\n").as_bytes()) {
        Ok(()) => Ok(mark),
        // Another process created it first: use its mark.
        Err(_) => read()?.ok_or_else(|| Fault::known(crate::fault::Known::MachineShape, at.display().to_string())),
    }
}

/// A mark's shape: 32 lowercase hex digits.
fn is_mark(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Where a home's writer mark lives.
pub fn mark_path(home: &Home) -> PathBuf {
    home.dir(Slot::Settings).join(MARK_FILE)
}

/// What a home's writer mark says. A missing mark is distinct from an unreadable one.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// No mark yet (a home from an older version, or never opened by a writer).
    None,
    /// The machine the mark names.
    Machine(String),
    /// A mark this version cannot read: a read error, or a file that is not a single mark line (which is also
    /// how a later version's format appears).
    Unread(Fault),
}

/// Read a home's writer mark.
pub fn mark_read(home: &Home) -> Mark {
    let at = mark_path(home);
    match std::fs::read(&at) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Mark::None,
        Err(e) => Mark::Unread(classify(&e, &at.display().to_string())),
        // Surrounding whitespace is ignored.
        Ok(b) => match std::str::from_utf8(&b).ok().map(str::trim).filter(|s| is_mark(s)) {
            Some(m) => Mark::Machine(m.to_string()),
            None => Mark::Unread(Fault::known(crate::fault::Known::SettingsShape, at.display().to_string())),
        },
    }
}

/// The machine a home's writer mark names (`None`: no mark, or one this version cannot read).
pub fn mark_of(home: &Home) -> Option<String> {
    match mark_read(home) {
        Mark::Machine(m) => Some(m),
        _ => None,
    }
}

/// Mark a home as written by this machine.
pub fn write_mark(home: &Home) -> Result<(), Fault> {
    let me = this_machine()?;
    crate::home::put_at(&home.dir(Slot::Settings), MARK_FILE, format!("{me}\n").as_bytes())
}

/// Ask the kernel for an exclusive lock on this descriptor, without waiting. True when obtained.
///
/// The single-writer rule rests on this: a kernel lock is released when the process ends, so stale locks
/// cannot arise.
pub fn grab(file: &std::fs::File) -> bool {
    crate::platform::lock_now(file)
}

/// The same primitive, blocking until obtained (used for the key vault file's lock).
///
/// The home's writer lock does not wait: failing means another instance holds it, and this one must become a
/// reader and tell the user at once. The vault file's lock waits, so a passcode attempt queues instead of
/// failing as "wrong passcode" while another process changes the vault.
pub fn grab_waiting(file: &std::fs::File) -> bool {
    crate::platform::lock_wait(file)
}

/// Where a home's lock file lives. Used both to take the lock and to check whether a held lock is this home's.
pub fn path_of(home: &Home) -> PathBuf {
    home.dir(Slot::Settings).join(LOCK_FILE)
}

/// Whether the held lock is this home's writer lock.
///
/// `flock` is per open file descriptor: a second descriptor to the same lock file in the same process counts
/// as another holder, so re-entering one's own home (switching roles, entering a home after import) would make
/// this process a reader. The lock is therefore taken once per process and reused when re-entering the same
/// home. Paths are compared after resolving aliases such as `/tmp` and `/private/tmp`.
pub fn holds_writer(held: Option<&Lock>, home: &Home) -> bool {
    let Some(l) = held else { return false };
    if !l.mode().writable() {
        return false;
    }
    crate::home::same_place(l.path(), &path_of(home))
}

/// The lock file's descriptor, registered with the OS layer while open (`zikaron_os::LockFile`). A child process
/// spawned meanwhile closes its inherited copy before it runs, so releasing the lock here frees it at once and a
/// later attempt on this machine is never turned into a reader by a child's copy. Every close, including after a
/// failed grab, goes through that type.
struct Held(zikaron_os::LockFile);

/// Take the lock: obtained means writer (or [`Mode::OtherMachine`] per the writer mark), not obtained means
/// reader. Either way a `Lock` is returned.
pub fn take(home: &Home) -> Result<Lock, Fault> {
    let path = path_of(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| classify(&e, &parent.display().to_string()))?;
    }
    // Owner-only from creation, like the key vault's lock file (`keybox`): on a multi-user machine another
    // account could otherwise read the holder's pid and, since locking needs only an open handle (even
    // read-only), hold the lock forever and turn every instance here into a reader. An existing file is
    // opened without truncation, so the holder text stays until a writer takes it.
    let mut o = zikaron_os::Options::new();
    o.create(true).read(true).write(true);
    zikaron_os::owner_only(&mut o);
    let held = Held(zikaron_os::LockFile::open(&o, &path).map_err(|e| classify(&e, &path.display().to_string()))?);
    let got = grab(held.0.file());
    // The kernel lock covers this machine; the writer mark says whether another machine writes this home.
    if got {
        let me = this_machine()?;
        match mark_read(home) {
            Mark::Machine(other) if other != me => {
                let holder = crate::lang::filln(crate::lang::Key::TailOtherMachine, &[&mark_path(home).display().to_string()]);
                return Ok(Lock { _file: held, mode: Mode::OtherMachine, path, holder, trouble: None });
            }
            Mark::Machine(_) => {}
            Mark::None => write_mark(home)?,
            // An unreadable mark is no license to write: stay read-only, report why, and leave the mark alone
            // (a later version or another machine may be writing).
            Mark::Unread(f) => {
                let holder = f.evidence();
                return Ok(Lock { _file: held, mode: Mode::OtherMachine, path, holder, trouble: Some(f) });
            }
        }
    }
    if got {
        let me = format!("pid {}", std::process::id());
        // Holder text for display only; a failed write does not affect the lock.
        use std::io::{Seek, Write};
        let mut h = held.0.file();
        let _ = h.set_len(0);
        let _ = h.rewind();
        let _ = h.write_all(me.as_bytes());
        let _ = h.flush();
        Ok(Lock { _file: held, mode: Mode::Writer, path, holder: me, trouble: None })
    } else {
        let holder = std::fs::read_to_string(&path).unwrap_or_default();
        let holder = if holder.trim().is_empty() { crate::lang::t(crate::lang::Key::Tail184).to_string() } else { holder };
        Ok(Lock { _file: held, mode: Mode::Reader, path, holder, trouble: None })
    }
}
