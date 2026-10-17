//! The plain ledgers this machine's desktop adopted, kept blind: a random salt and a list of
//! `sha256(salt ‖ root id)`, one line per adopted ledger (the root id is the ledger's genesis entry id). The list
//! alone says nothing; only someone holding that ledger can find its line. The ledger's own folder is never
//! touched, so moving or renaming it changes nothing (the root id does not follow the path).
//!
//! The desktop adds a line when it adopts a ledger; the command line checks the list before it writes a plain
//! ledger and refuses one that is on it. Neither the passcode nor the credential is needed to check. Both
//! files live in the machine directory ([`SALT_FILE`], [`LIST_FILE`]) and are read and written here only.
//!
//! Shapes, closed: the salt file is 64 lowercase hex digits and a newline; the list is zero or more lines, each
//! 64 lowercase hex digits and a newline, no line twice. Anything else is refused by name, never read past.

use crate::trouble::{Code, Trouble};
use std::path::Path;

/// The salt's file name, in the machine directory.
pub const SALT_FILE: &str = "adopted-salt";

/// The list's file name, in the machine directory.
pub const LIST_FILE: &str = "adopted-list";

/// One line's value: 64 lowercase hex digits.
fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    zikaron::hexfmt::decode(&format!("0x{s}"))?.try_into().ok()
}

fn line(v: &[u8; 32]) -> String {
    zikaron::hexfmt::encode(v).trim_start_matches("0x").to_string()
}

/// Read a file of hex lines in its closed shape (`None`: not there).
fn lines(at: &Path) -> Result<Option<Vec<[u8; 32]>>, Trouble> {
    let text = match std::fs::read(at) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Trouble::io(&e, &at.display().to_string())),
    };
    let shape = || Trouble::new(Code::SettingsShape, at.display().to_string());
    let text = std::str::from_utf8(&text).map_err(|_| shape())?;
    if !text.is_empty() && !text.ends_with('\n') {
        return Err(shape());
    }
    let mut out: Vec<[u8; 32]> = Vec::new();
    for l in text.lines() {
        let v = hex32(l).ok_or_else(shape)?;
        if out.contains(&v) {
            return Err(shape());
        }
        out.push(v);
    }
    Ok(Some(out))
}

/// This machine's salt (`None`: nothing adopted yet). A salt file not of one line is refused by name.
pub fn salt(machine_dir: &Path) -> Result<Option<[u8; 32]>, Trouble> {
    let at = machine_dir.join(SALT_FILE);
    match lines(&at)? {
        None => Ok(None),
        Some(v) if v.len() == 1 => Ok(Some(v[0])),
        Some(_) => Err(Trouble::new(Code::SettingsShape, at.display().to_string())),
    }
}

/// This machine's salt, made on first use (32 random bytes). The machine's other blind lists (the pens of handed
/// ledgers, `pens`) are kept under it too.
pub fn salt_made(machine_dir: &Path) -> Result<[u8; 32], Trouble> {
    match salt(machine_dir)? {
        Some(s) => Ok(s),
        None => {
            let s: [u8; 32] = crate::trouble::random(32)?.try_into().map_err(|_| Trouble::new(Code::Unknown, SALT_FILE))?;
            crate::land::put_at(machine_dir, SALT_FILE, format!("{}\n", line(&s)).as_bytes())?;
            Ok(s)
        }
    }
}

/// The adopted list's lines (empty when there is none).
pub fn list(machine_dir: &Path) -> Result<Vec<[u8; 32]>, Trouble> {
    Ok(lines(&machine_dir.join(LIST_FILE))?.unwrap_or_default())
}

/// The line a ledger with root id `root` has under `salt`: `sha256(salt ‖ root)`.
pub fn blind(salt: &[u8; 32], root: &[u8; 32]) -> [u8; 32] {
    let mut m = Vec::with_capacity(64);
    m.extend_from_slice(salt);
    m.extend_from_slice(root);
    zikaron::cryptox::sha256(&m)
}

/// Whether the ledger with root id `root` was adopted on this machine.
pub fn contains(machine_dir: &Path, root: &[u8; 32]) -> Result<bool, Trouble> {
    let Some(salt) = salt(machine_dir)? else { return Ok(false) };
    let want = blind(&salt, root);
    Ok(list(machine_dir)?.contains(&want))
}

/// Record that the ledger with root id `root` was adopted: the salt is made on first use (32 random bytes), and
/// the ledger's line is added once. True when the line is new.
pub fn mark(machine_dir: &Path, root: &[u8; 32]) -> Result<bool, Trouble> {
    let salt = salt_made(machine_dir)?;
    let mut all = list(machine_dir)?;
    let want = blind(&salt, root);
    if all.contains(&want) {
        return Ok(false);
    }
    all.push(want);
    let text: String = all.iter().map(|v| format!("{}\n", line(v))).collect();
    crate::land::put_at(machine_dir, LIST_FILE, text.as_bytes())?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("zk-seal-adopted-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("dir");
        d
    }

    #[test]
    fn the_list_holds_one_blind_line_per_ledger_and_never_the_root() {
        let d = dir("lines");
        let root = [7u8; 32];
        assert!(!contains(&d, &root).expect("nothing yet"));
        assert!(mark(&d, &root).expect("marked"));
        assert!(!mark(&d, &root).expect("once"), "a ledger has one line");
        assert!(contains(&d, &root).expect("found"));
        assert!(!contains(&d, &[8u8; 32]).expect("another ledger"));
        let text = std::fs::read_to_string(d.join(LIST_FILE)).expect("the list");
        assert_eq!(text.lines().count(), 1);
        assert!(!text.contains(&line(&root)), "the root id is not written");
        let salt = std::fs::read_to_string(d.join(SALT_FILE)).expect("the salt");
        assert_eq!((salt.len(), salt.ends_with('\n')), (65, true), "one line of 64 hex digits");
    }

    #[test]
    fn a_list_or_salt_out_of_its_shape_is_refused_by_name() {
        let d = dir("shape");
        mark(&d, &[1u8; 32]).expect("marked");
        for bad in ["not hex\n", &format!("{}", "ab".repeat(32)), &format!("{0}\n{0}\n", "cd".repeat(32)), &format!("{}\n", "AB".repeat(32))] {
            std::fs::write(d.join(LIST_FILE), bad).expect("laid");
            assert_eq!(contains(&d, &[1u8; 32]).err().map(|t| t.code), Some(Code::SettingsShape), "{bad:?}");
        }
        std::fs::write(d.join(LIST_FILE), b"").expect("empty list");
        assert!(!contains(&d, &[1u8; 32]).expect("an empty list reads"));
        std::fs::write(d.join(SALT_FILE), format!("{0}\n{0}\n", "ef".repeat(32))).expect("two salts");
        assert_eq!(contains(&d, &[1u8; 32]).err().map(|t| t.code), Some(Code::SettingsShape));
    }
}
