use crate::fault::Fault;
use crate::home::{Home, Slot};
use std::path::PathBuf;
use zikaron_seal::writer::{self, Took};

pub const LOCK_FILE: &str = zikaron_seal::envelope::WRITER_LOCK;

pub const MARK_FILE: &str = writer::MARK_FILE;

pub const MACHINE_FILE: &str = writer::MACHINE_FILE;

pub fn files_under(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    crate::local::walk(dir, "", &mut found);
    found.into_iter().filter(|(at, _)| at.file_name().is_some_and(|n| n == LOCK_FILE)).map(|(at, _)| at).collect()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Writer,
    Reader,
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

#[derive(Default)]
struct Held {
    file: Option<zikaron_os::LockFile>,
    writes: bool,
}

struct Kept(std::sync::Arc<std::sync::Mutex<Held>>);

impl Kept {
    fn get(&self) -> std::sync::MutexGuard<'_, Held> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

static KEPT: std::sync::Mutex<Vec<(PathBuf, std::sync::Weak<std::sync::Mutex<Held>>)>> = std::sync::Mutex::new(Vec::new());

fn kept_for(path: &std::path::Path) -> Option<std::sync::Arc<std::sync::Mutex<Held>>> {
    let all = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    all.iter().rev().filter(|(p, _)| crate::home::same_place(p, path)).find_map(|(_, w)| w.upgrade())
}

fn locks(held: &std::sync::Arc<std::sync::Mutex<Held>>, path: &std::path::Path) -> bool {
    let all = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    all.iter().any(|(p, w)| crate::home::same_place(p, path) && std::ptr::eq(w.as_ptr(), std::sync::Arc::as_ptr(held)))
}

#[derive(Clone)]
pub struct Hand(std::sync::Weak<std::sync::Mutex<Held>>);

pub const SETTLE_WAIT: std::time::Duration = std::time::Duration::from_secs(60);
pub const SETTLE_STEP: std::time::Duration = std::time::Duration::from_millis(100);

pub fn settling<T>(hand: Option<&Hand>, home: &std::path::Path, f: impl FnOnce() -> T) -> Result<T, Fault> {
    crate::reads::touch();
    let until = std::time::Instant::now() + SETTLE_WAIT;
    let path = writer::lock_path(home);
    loop {
        {
            let shell = hand.and_then(|h| h.0.upgrade()).filter(|h| locks(h, &path)).or_else(|| kept_for(&path)).unwrap_or_default();
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

#[derive(Clone, Debug)]
pub enum Regained {
    Took,
    Stopped,
    OtherMachine,
    Unread(Fault),
}

pub fn regain_behind(hand: &Hand, home: &std::path::Path, stop: &std::sync::atomic::AtomicBool) -> Result<Regained, Fault> {
    crate::reads::touch();
    loop {
        if stop.load(std::sync::atomic::Ordering::SeqCst) {
            return Ok(Regained::Stopped);
        }
        let Some(shared) = hand.0.upgrade() else { return Ok(Regained::Stopped) };
        {
            let mut slot = shared.lock().unwrap_or_else(|e| e.into_inner());
            if slot.file.is_some() {
                return Ok(Regained::Took);
            }
            let t = writer::take(home, &crate::home::machine_dir()?)?;
            match t.took {
                Took::Writer => {
                    slot.file = Some(t.held);
                    return Ok(Regained::Took);
                }
                Took::Busy { .. } => {}
                Took::OtherMachine => {
                    slot.file = Some(t.held);
                    slot.writes = false;
                    return Ok(Regained::OtherMachine);
                }
                Took::Unread(tr) => {
                    slot.file = Some(t.held);
                    slot.writes = false;
                    return Ok(Regained::Unread(Fault::from(tr)));
                }
            }
        }
        drop(shared);
        std::thread::sleep(SETTLE_STEP);
    }
}

pub struct Lock {
    file: Kept,
    mode: Mode,
    path: PathBuf,
    holder: String,
    trouble: Option<Fault>,
}

impl Lock {
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

    pub fn holder(&self) -> &str {
        &self.holder
    }

    pub fn mark_trouble(&self) -> Option<&Fault> {
        self.trouble.as_ref()
    }

    pub fn held(&self) -> bool {
        self.file.get().file.is_some()
    }

    pub fn release(&mut self) {
        self.file.get().file = None;
    }

    pub fn hand(&self) -> Hand {
        Hand(std::sync::Arc::downgrade(&self.file.0))
    }

    pub fn regain(&mut self, home: &Home) -> Result<bool, Fault> {
        crate::reads::touch();
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

    pub fn regained(&mut self, r: &Regained, home: &Home) {
        match r {
            Regained::Took | Regained::Stopped => {}
            Regained::OtherMachine => {
                self.mode = Mode::OtherMachine;
                self.holder = crate::lang::filln(crate::lang::Key::TailOtherMachine, &[&mark_path(home).display().to_string()]);
            }
            Regained::Unread(f) => {
                self.mode = Mode::OtherMachine;
                self.holder = f.evidence();
                self.trouble = Some(f.clone());
            }
        }
    }

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

pub fn this_machine() -> Result<String, Fault> {
    Ok(writer::this_machine(&crate::home::machine_dir()?)?)
}

pub fn mark_path(home: &Home) -> PathBuf {
    writer::mark_path(home.root())
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    None,
    Machine(String),
    Unread(Fault),
}

pub fn mark_read(home: &Home) -> Mark {
    match writer::mark_read(home.root()) {
        writer::Mark::None => Mark::None,
        writer::Mark::Machine(m) => Mark::Machine(m),
        writer::Mark::Unread(t) => Mark::Unread(t.into()),
    }
}

pub fn mark_of(home: &Home) -> Option<String> {
    match mark_read(home) {
        Mark::Machine(m) => Some(m),
        _ => None,
    }
}

pub fn write_mark(home: &Home) -> Result<(), Fault> {
    let me = this_machine()?;
    crate::home::put_at(&home.dir(Slot::Settings), MARK_FILE, format!("{me}\n").as_bytes())
}

pub fn grab(file: &std::fs::File) -> bool {
    crate::platform::lock_now(file)
}

pub fn grab_waiting(file: &std::fs::File) -> bool {
    crate::platform::lock_wait(file)
}

pub fn path_of(home: &Home) -> PathBuf {
    writer::lock_path(home.root())
}

pub fn holds_writer_at(held: Option<&std::path::Path>, home: &Home) -> bool {
    held.map(|p| crate::home::same_place(p, &path_of(home))).unwrap_or(false)
}

pub fn writer_path(held: Option<&Lock>) -> Option<&std::path::Path> {
    held.filter(|l| l.mode().writable()).map(|l| l.path())
}

pub fn take(home: &Home) -> Result<Lock, Fault> {
    take_as(home, false)
}

pub fn take_as(home: &Home, alone: bool) -> Result<Lock, Fault> {
    let path = path_of(home);
    let t = writer::take(home.root(), &crate::home::machine_dir()?)?;
    Ok(match t.took {
        Took::Writer => Lock::made(Some(t.held), Mode::Writer, path, format!("pid {}", std::process::id()), None),
        Took::OtherMachine => {
            let holder = crate::lang::filln(crate::lang::Key::TailOtherMachine, &[&mark_path(home).display().to_string()]);
            Lock::made(Some(t.held), Mode::OtherMachine, path, holder, None)
        }
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
