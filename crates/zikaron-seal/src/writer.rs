use crate::envelope::{SETTINGS_DIR, WRITER_LOCK};
use crate::trouble::{Code, Trouble};
use std::path::{Path, PathBuf};

pub const MARK_FILE: &str = "writer-mark";

pub const MACHINE_FILE: &str = "machine-mark";

pub fn is_mark(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

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
    match zikaron_glue::landing::land_bytes(&at, format!("{mark}\n").as_bytes()) {
        Ok(()) => Ok(mark),
        Err(_) => read()?.ok_or_else(|| Trouble::new(Code::MachineShape, at.display().to_string())),
    }
}

pub fn mark_path(home: &Path) -> PathBuf {
    home.join(SETTINGS_DIR).join(MARK_FILE)
}

pub fn lock_path(home: &Path) -> PathBuf {
    home.join(SETTINGS_DIR).join(WRITER_LOCK)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    None,
    Machine(String),
    Unread(Trouble),
}

pub fn mark_read(home: &Path) -> Mark {
    let at = mark_path(home);
    match std::fs::read(&at) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Mark::None,
        Err(e) => Mark::Unread(Trouble::io(&e, &at.display().to_string())),
        Ok(b) => match std::str::from_utf8(&b).ok().map(str::trim).filter(|s| is_mark(s)) {
            Some(m) => Mark::Machine(m.to_string()),
            None => Mark::Unread(Trouble::new(Code::SettingsShape, at.display().to_string())),
        },
    }
}

pub fn write_mark(home: &Path, machine_dir: &Path) -> Result<(), Trouble> {
    let me = this_machine(machine_dir)?;
    crate::land::put_at(&home.join(SETTINGS_DIR), MARK_FILE, format!("{me}\n").as_bytes())
}

#[derive(Debug)]
pub enum Took {
    Writer,
    Busy { holder: String },
    OtherMachine,
    Unread(Trouble),
}

pub struct Taken {
    pub held: zikaron_os::LockFile,
    pub took: Took,
}

pub fn take(home: &Path, machine_dir: &Path) -> Result<Taken, Trouble> {
    let path = lock_path(home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Trouble::io(&e, &parent.display().to_string()))?;
    }
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
