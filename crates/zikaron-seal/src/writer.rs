//! Who writes a home: one way to take its writer lock, for the desktop and the command line alike.
//!
//! Two things decide it. The kernel lock on the home's lock file (`envelope::WRITER_LOCK` in its settings room)
//! covers this machine: obtained means nobody else here writes it, and it is released when the process ends,
//! so no lock outlives its holder. The home's writer mark ([`MARK_FILE`]) covers other machines: a home on a
//! synced or shared disk names the machine that writes it, and a home whose mark names another machine, or a
//! mark this version cannot read, is no license to write here. This machine's own mark ([`MACHINE_FILE`]) lives
//! in the machine directory, outside every home, and is a random number that says nothing about the machine.
//!
//! [`take`] is the only place these are asked together; the desktop's `lock` and the command line's sealed
//! write both call it.

use crate::envelope::{SETTINGS_DIR, WRITER_LOCK};
use crate::trouble::{Code, Trouble};
use std::path::{Path, PathBuf};

/// The writer mark's file name, in a home's settings room: which machine writes this home.
pub const MARK_FILE: &str = "writer-mark";

/// This machine's mark's file name, in the machine directory.
pub const MACHINE_FILE: &str = "machine-mark";

/// A mark's shape: 32 lowercase hex digits.
pub fn is_mark(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// This machine's mark, created on first use. Created once: the write refuses an existing file and callers in
/// this process are serialized, so two first callers never make two marks; another process that made it first
/// is read back.
pub fn this_machine(machine_dir: &Path) -> Result<String, Trouble> {
    static MAKING: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _one = MAKING.lock().unwrap_or_else(|e| e.into_inner());
    let at = machine_dir.join(MACHINE_FILE);
    let read = || -> Result<Option<String>, Trouble> {
        match std::fs::read_to_string(&at) {
            Ok(s) if is_mark(s.trim()) => Ok(Some(s.trim().to_string())),
            Ok(_) => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Trouble::io(&e, &at.display().to_string())),
        }
    };
    if let Some(mark) = read()? {
        return Ok(mark);
    }
    let n = crate::trouble::random(16)?;
    let mark = zikaron::hexfmt::encode(&n).trim_start_matches("0x").to_ascii_lowercase();
    std::fs::create_dir_all(machine_dir).map_err(|e| Trouble::io(&e, &machine_dir.display().to_string()))?;
    // Not a secret, written like any file; `land_bytes` refuses if the file is already there.
    match zikaron_glue::landing::land_bytes(&at, format!("{mark}\n").as_bytes()) {
        Ok(()) => Ok(mark),
        Err(_) => read()?.ok_or_else(|| Trouble::new(Code::MachineShape, at.display().to_string())),
    }
}

/// Where a home's writer mark lives.
pub fn mark_path(home: &Path) -> PathBuf {
    home.join(SETTINGS_DIR).join(MARK_FILE)
}

/// Where a home's lock file lives.
pub fn lock_path(home: &Path) -> PathBuf {
    home.join(SETTINGS_DIR).join(WRITER_LOCK)
}

/// What a home's writer mark says. A missing mark is distinct from an unreadable one.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// No mark yet (never opened by a writer).
    None,
    /// The machine the mark names.
    Machine(String),
    /// A mark this version cannot read: a read error, or a file that is not one mark line (which is also how a
    /// later version's form appears).
    Unread(Trouble),
}

/// Read a home's writer mark.
pub fn mark_read(home: &Path) -> Mark {
    let at = mark_path(home);
    match std::fs::read(&at) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Mark::None,
        Err(e) => Mark::Unread(Trouble::io(&e, &at.display().to_string())),
        // Surrounding whitespace is ignored.
        Ok(b) => match std::str::from_utf8(&b).ok().map(str::trim).filter(|s| is_mark(s)) {
            Some(m) => Mark::Machine(m.to_string()),
            None => Mark::Unread(Trouble::new(Code::SettingsShape, at.display().to_string())),
        },
    }
}

/// Mark a home as written by this machine.
pub fn write_mark(home: &Path, machine_dir: &Path) -> Result<(), Trouble> {
    let me = this_machine(machine_dir)?;
    crate::land::put_at(&home.join(SETTINGS_DIR), MARK_FILE, format!("{me}\n").as_bytes())
}

/// What taking the writer lock found.
#[derive(Debug)]
pub enum Took {
    /// The kernel lock is held and the mark is this machine's (written now if there was none): this holder
    /// writes the home.
    Writer,
    /// Another holder on this machine has the kernel lock (`holder`: what it wrote in the lock file, for display
    /// only).
    Busy { holder: String },
    /// The kernel lock is held, but the home's mark names another machine (left as it is).
    OtherMachine,
    /// The kernel lock is held, but the mark does not read (left as it is; the trouble says why).
    Unread(Trouble),
}

/// The lock file's descriptor and what taking it found. The kernel lock (when obtained) is released when this is
/// dropped or the process ends.
pub struct Taken {
    pub held: zikaron_os::LockFile,
    pub took: Took,
}

/// Take a home's writer lock now, never waiting: the kernel lock first, then the writer mark.
pub fn take(home: &Path, machine_dir: &Path) -> Result<Taken, Trouble> {
    let path = lock_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Trouble::io(&e, &parent.display().to_string()))?;
    }
    // Owner-only from creation: on a machine with several accounts another one could otherwise hold the lock
    // forever with a read-only handle. An existing file is opened without truncation, so the holder text stays
    // until a writer takes it.
    let mut o = zikaron_os::Options::new();
    o.create(true).read(true).write(true);
    zikaron_os::owner_only(&mut o);
    let held = zikaron_os::LockFile::open(&o, &path).map_err(|e| Trouble::io(&e, &path.display().to_string()))?;
    if !zikaron_os::lock_now(held.file()) {
        let holder = std::fs::read_to_string(&path).unwrap_or_default();
        return Ok(Taken { held, took: Took::Busy { holder } });
    }
    let me = this_machine(machine_dir)?;
    match mark_read(home) {
        Mark::Machine(other) if other != me => return Ok(Taken { held, took: Took::OtherMachine }),
        Mark::Machine(_) => {}
        Mark::None => write_mark(home, machine_dir)?,
        Mark::Unread(t) => return Ok(Taken { held, took: Took::Unread(t) }),
    }
    // Holder text for display only; a failed write does not affect the lock.
    use std::io::{Seek, Write};
    let mut h = held.file();
    let _ = h.set_len(0);
    let _ = h.rewind();
    let _ = h.write_all(format!("pid {}", std::process::id()).as_bytes());
    let _ = h.flush();
    Ok(Taken { held, took: Took::Writer })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dirs(name: &str) -> (PathBuf, PathBuf) {
        let d = std::env::temp_dir().join(format!("zk-seal-writer-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("home").join(SETTINGS_DIR)).expect("home");
        std::fs::create_dir_all(d.join("machine")).expect("machine");
        (d.join("home"), d.join("machine"))
    }

    #[test]
    fn one_holder_writes_and_a_second_is_told_it_is_busy() {
        let (home, machine) = dirs("busy");
        let first = take(&home, &machine).expect("taken");
        assert!(matches!(first.took, Took::Writer));
        assert_eq!(mark_read(&home), Mark::Machine(this_machine(&machine).expect("mark")), "the mark is written on first take");
        let second = take(&home, &machine).expect("asked");
        assert!(matches!(second.took, Took::Busy { ref holder } if holder.starts_with("pid ")), "{:?}", second.took);
        drop(second);
        drop(first);
        assert!(matches!(take(&home, &machine).expect("taken again").took, Took::Writer), "released when dropped");
    }

    #[test]
    fn another_machines_mark_or_one_that_does_not_read_is_no_license() {
        let (home, machine) = dirs("other");
        std::fs::write(mark_path(&home), format!("{}\n", "ab".repeat(16))).expect("laid");
        assert!(matches!(take(&home, &machine).expect("asked").took, Took::OtherMachine));
        assert_eq!(std::fs::read_to_string(mark_path(&home)).expect("mark"), format!("{}\n", "ab".repeat(16)), "left as it is");
        std::fs::write(mark_path(&home), b"not a mark").expect("laid");
        assert!(matches!(take(&home, &machine).expect("asked").took, Took::Unread(ref t) if t.code == Code::SettingsShape));
    }
}
