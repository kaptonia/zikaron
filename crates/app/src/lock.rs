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
//! ─── A send's receipt ───
//!
//! A send holds the lock while it readies and sends its batch; its receipt wait holds no turn (`Tasks::spawn_turnless`),
//! so while the command line writes alone the lock is let go meanwhile and the command line may write or send the
//! rest of the queue. The wait settles what it heard under the lock again ([`settling`]): the shell's when it holds
//! it, or taken for that one write.
//!
//! ─── Another machine ───
//!
//! Taking the lock (the kernel lock and the writer mark together) is the sealed layer's, one implementation with
//! the command line's sealed write (`zikaron_seal::writer::take`); this module keeps the instance's mode, the
//! words for who holds it, and the turn the desktop takes per write while the command line writes alone
//! ([`Lock::release`], [`Lock::regain`]).
//!
//! A kernel lock only works on one machine: a data folder on a synced or shared disk opened on two machines
//! would get a writer on each, and the two ledgers would diverge. So each home keeps a writer mark
//! ([`MARK_FILE`]): one line naming the machine that writes it (this machine's mark, [`this_machine`], lives
//! in the machine directory, outside every data folder). Opening a home whose mark names another machine
//! makes this instance a reader ([`Mode::OtherMachine`]), until the user explicitly chooses "write from this
//! machine" ([`Lock::take_over`]), which rewrites the mark. There is no heartbeat or expiry, since both depend
//! on the wall clock and sync delays, which cannot be relied on.

use crate::fault::Fault;
use crate::home::{Home, Slot};
use std::path::PathBuf;
use zikaron_seal::writer::{self, Took};

/// The lock file's name.
pub const LOCK_FILE: &str = zikaron_seal::envelope::WRITER_LOCK;

/// The writer mark's file name (in a home's `settings/` directory): which machine writes this home.
pub const MARK_FILE: &str = writer::MARK_FILE;

/// The file name of this machine's mark (in the machine directory, outside every data folder).
pub const MACHINE_FILE: &str = writer::MACHINE_FILE;

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

/// The home's lock file as this instance holds it, and whether this instance writes the home under it (its mode is
/// the writer's: not a second instance reading, not one whose home another machine writes).
#[derive(Default)]
struct Held {
    file: Option<zikaron_os::LockFile>,
    writes: bool,
}

/// [`Held`] shared: the shell takes the kernel lock and lets it go between writes; a send's receipt wait, which holds
/// no turn, settles what it heard under it ([`settling`]). Whoever has it in use, the other waits for it.
struct Kept(std::sync::Arc<std::sync::Mutex<Held>>);

impl Kept {
    fn get(&self) -> std::sync::MutexGuard<'_, Held> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// This process's locks, by the lock file's place: the one place a receipt wait finds the lock the shell keeps for
/// that home now ([`settling`]), whether it is the lock the wait was started under or one taken again since (the
/// shell left the home and came back). Each is reached without being kept alive.
static KEPT: std::sync::Mutex<Vec<(PathBuf, std::sync::Weak<std::sync::Mutex<Held>>)>> = std::sync::Mutex::new(Vec::new());

/// The lock this process keeps for the lock file at `path` now, if any.
fn kept_for(path: &std::path::Path) -> Option<std::sync::Arc<std::sync::Mutex<Held>>> {
    let all = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    all.iter().rev().filter(|(p, _)| crate::home::same_place(p, path)).find_map(|(_, w)| w.upgrade())
}

/// A receipt wait's reach to the shell's lock ([`Lock::hand`]): it keeps no lock alive (a home closed meanwhile
/// lets its lock go as ever), and settles under the shell's while the shell keeps it.
#[derive(Clone)]
pub struct Hand(std::sync::Weak<std::sync::Mutex<Held>>);

/// How long settling a receipt waits for the command line to finish a write of its own (it holds the lock
/// for one write, or the first two stages of a send of its own), and how often it asks again meanwhile.
pub const SETTLE_WAIT: std::time::Duration = std::time::Duration::from_secs(60);
pub const SETTLE_STEP: std::time::Duration = std::time::Duration::from_millis(100);

/// Settle a send's receipt (its last stage, `zikaron_send::batch::settle`, run by `f`) under the home's writer
/// lock: the lock this process keeps for that home now (the hand's, or one the shell took again since it left the
/// home and came back, [`kept_for`]); with the kernel lock in that hand as the writer's, at once; in hand as no
/// writer's (another machine writes the home, or this instance reads it), refused by name as read-only, nothing
/// written; without it (let go between writes while the command line writes alone, or no instance holding one),
/// taken for this one write and let go after, asking again while the command line holds it, for [`SETTLE_WAIT`] at
/// most (then refused by name, `CLI_WRITING`: nothing is written, the batch stays in flight and the next wait settles
/// it). The lock is held throughout, so the shell neither lets it go nor takes it in between.
pub fn settling<T>(hand: Option<&Hand>, home: &std::path::Path, f: impl FnOnce() -> T) -> Result<T, Fault> {
    let until = std::time::Instant::now() + SETTLE_WAIT;
    loop {
        {
            let shell = hand.and_then(|h| h.0.upgrade()).or_else(|| kept_for(&writer::lock_path(home))).unwrap_or_default();
            let slot = shell.lock().unwrap_or_else(|e| e.into_inner());
            if slot.file.is_some() && slot.writes {
                return Ok(f());
            }
            if slot.file.is_some() {
                return Err(Fault::known(crate::fault::Known::ReadOnly, writer::mark_path(home).display().to_string()));
            }
            let t = writer::take(home, &crate::home::machine_dir()?)?;
            match t.took {
                Took::Writer => {
                    let out = f();
                    drop(t.held);
                    return Ok(out);
                }
                Took::Busy { .. } => {}
                Took::OtherMachine => return Err(Fault::known(crate::fault::Known::ReadOnly, writer::mark_path(home).display().to_string())),
                Took::Unread(tr) => return Err(Fault::from(tr)),
            }
        }
        if std::time::Instant::now() >= until {
            return Err(Fault::known(crate::fault::Known::CliWriting, home.display().to_string()));
        }
        std::thread::sleep(SETTLE_STEP);
    }
}

/// A lock on a home, released when this value is dropped or the process exits. A writer may let the kernel lock
/// go between writes while the command line writes alone ([`Lock::release`]) and take it again for the next
/// write ([`Lock::regain`]); its mode stays.
pub struct Lock {
    file: Kept,
    mode: Mode,
    path: PathBuf,
    holder: String,
    /// Why the home's writer mark could not be read ([`Mark::Unread`]), when that made this lock a reader.
    trouble: Option<Fault>,
}

impl Lock {
    /// A lock as taken, listed among this process's locks ([`kept_for`]).
    fn made(file: Option<zikaron_os::LockFile>, mode: Mode, path: PathBuf, holder: String, trouble: Option<Fault>) -> Lock {
        let held = std::sync::Arc::new(std::sync::Mutex::new(Held { file, writes: mode.writable() }));
        let mut all = KEPT.lock().unwrap_or_else(|e| e.into_inner());
        all.retain(|(_, w)| w.strong_count() > 0);
        all.push((path.clone(), std::sync::Arc::downgrade(&held)));
        drop(all);
        Lock { file: Kept(held), mode, path, holder, trouble }
    }

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

    /// Whether the kernel lock is in hand now.
    pub fn held(&self) -> bool {
        self.file.get().file.is_some()
    }

    /// Let the kernel lock go (a writer between writes while the command line writes alone); the mode stays.
    pub fn release(&mut self) {
        self.file.get().file = None;
    }

    /// The reach to this lock for a receipt wait to settle under ([`settling`]).
    pub fn hand(&self) -> Hand {
        Hand(std::sync::Arc::downgrade(&self.file.0))
    }

    /// Take the kernel lock again for a write: true when in hand (already, or now); false when another holder
    /// on this machine has it. A home whose mark now names another machine (or no longer reads) turns this lock
    /// into that reader, and is false.
    pub fn regain(&mut self, home: &Home) -> Result<bool, Fault> {
        let mut slot = self.file.get();
        if slot.file.is_some() {
            return Ok(true);
        }
        let t = writer::take(home.root(), &crate::home::machine_dir()?)?;
        match t.took {
            Took::Writer => {
                slot.file = Some(t.held);
                Ok(true)
            }
            Took::Busy { .. } => Ok(false),
            Took::OtherMachine => {
                slot.file = Some(t.held);
                slot.writes = false;
                self.mode = Mode::OtherMachine;
                self.holder = crate::lang::filln(crate::lang::Key::TailOtherMachine, &[&mark_path(home).display().to_string()]);
                Ok(false)
            }
            Took::Unread(tr) => {
                let f = Fault::from(tr);
                slot.file = Some(t.held);
                slot.writes = false;
                self.mode = Mode::OtherMachine;
                self.holder = f.evidence();
                self.trouble = Some(f);
                Ok(false)
            }
        }
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
        self.file.get().writes = true;
        self.holder = format!("pid {}", std::process::id());
        self.trouble = None;
        Ok(())
    }
}

/// This machine's mark, kept in the machine directory and created on first use (`zikaron_seal::writer`).
pub fn this_machine() -> Result<String, Fault> {
    Ok(writer::this_machine(&crate::home::machine_dir()?)?)
}

/// Where a home's writer mark lives.
pub fn mark_path(home: &Home) -> PathBuf {
    writer::mark_path(home.root())
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
    match writer::mark_read(home.root()) {
        writer::Mark::None => Mark::None,
        writer::Mark::Machine(m) => Mark::Machine(m),
        writer::Mark::Unread(t) => Mark::Unread(t.into()),
    }
}

/// The machine a home's writer mark names (`None`: no mark, or one this version cannot read).
pub fn mark_of(home: &Home) -> Option<String> {
    match mark_read(home) {
        Mark::Machine(m) => Some(m),
        _ => None,
    }
}

/// Mark a home as written by this machine (through the write gate, `home::put_at`).
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
    writer::lock_path(home.root())
}

/// Whether the writer lock held (at `held`, from [`writer_path`]) is this home's.
///
/// `flock` is per open file descriptor: a second descriptor to the same lock file in the same process counts
/// as another holder, so re-entering one's own home (switching roles, entering a home after import) would make
/// this process a reader. The lock is therefore taken once per process and reused when re-entering the same
/// home. Paths are compared after resolving aliases such as `/tmp` and `/private/tmp`.
pub fn holds_writer_at(held: Option<&std::path::Path>, home: &Home) -> bool {
    held.map(|p| crate::home::same_place(p, &path_of(home))).unwrap_or(false)
}

/// Where the held lock is, when it is a writer's: what opening a home (off the window's thread when switching seat)
/// takes with it to ask [`holds_writer_at`].
pub fn writer_path(held: Option<&Lock>) -> Option<&std::path::Path> {
    held.filter(|l| l.mode().writable()).map(|l| l.path())
}

/// Take the lock (`zikaron_seal::writer::take`): obtained means writer (or [`Mode::OtherMachine`] per the
/// writer mark), not obtained means reader. Either way a `Lock` is returned.
pub fn take(home: &Home) -> Result<Lock, Fault> {
    take_as(home, false)
}

/// [`take`], knowing whether the command line writes alone (`alone`). Then the command line may hold the kernel
/// lock for a write of its own just as this instance opens the home: that is its turn, not another instance, so
/// this one is still the writer, with the lock to take at its own next write ([`Lock::regain`]); the writer mark
/// decides as ever.
pub fn take_as(home: &Home, alone: bool) -> Result<Lock, Fault> {
    let path = path_of(home);
    let t = writer::take(home.root(), &crate::home::machine_dir()?)?;
    Ok(match t.took {
        Took::Writer => Lock::made(Some(t.held), Mode::Writer, path, format!("pid {}", std::process::id()), None),
        Took::OtherMachine => {
            let holder = crate::lang::filln(crate::lang::Key::TailOtherMachine, &[&mark_path(home).display().to_string()]);
            Lock::made(Some(t.held), Mode::OtherMachine, path, holder, None)
        }
        // An unreadable mark is no license to write: stay read-only, report why, and leave the mark alone (a
        // later version or another machine may be writing).
        Took::Unread(tr) => {
            let f = Fault::from(tr);
            Lock::made(Some(t.held), Mode::OtherMachine, path, f.evidence(), Some(f))
        }
        Took::Busy { .. } if alone => {
            let me = this_machine()?;
            match mark_read(home) {
                Mark::Machine(m) if m == me => Lock::made(None, Mode::Writer, path, format!("pid {}", std::process::id()), None),
                Mark::None => Lock::made(None, Mode::Writer, path, format!("pid {}", std::process::id()), None),
                Mark::Machine(_) => {
                    let holder = crate::lang::filln(crate::lang::Key::TailOtherMachine, &[&mark_path(home).display().to_string()]);
                    Lock::made(None, Mode::OtherMachine, path, holder, None)
                }
                Mark::Unread(f) => Lock::made(None, Mode::OtherMachine, path, f.evidence(), Some(f)),
            }
        }
        Took::Busy { holder } => {
            let holder = if holder.trim().is_empty() { crate::lang::t(crate::lang::Key::Tail184).to_string() } else { holder };
            Lock::made(Some(t.held), Mode::Reader, path, holder, None)
        }
    })
}
