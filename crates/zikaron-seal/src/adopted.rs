use crate::trouble::{Code, Trouble};
use std::path::Path;

pub const SALT_FILE: &str = "adopted-salt";

pub const LIST_FILE: &str = "adopted-list";

fn hex32(s: &str) -> Option<[u8; 32]> {
    if s.len() != 64 || !s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return None;
    }
    zikaron::hexfmt::decode(&format!("0x{s}"))?.try_into().ok()
}

fn line(v: &[u8; 32]) -> String {
    zikaron::hexfmt::encode(v).trim_start_matches("0x").to_string()
}

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

pub fn salt(machine_dir: &Path) -> Result<Option<[u8; 32]>, Trouble> {
    let at = machine_dir.join(SALT_FILE);
    match lines(&at)? {
        None => Ok(None),
        Some(v) if v.len() == 1 => Ok(Some(v[0])),
        Some(_) => Err(Trouble::new(Code::SettingsShape, at.display().to_string())),
    }
}

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

pub fn list(machine_dir: &Path) -> Result<Vec<[u8; 32]>, Trouble> {
    Ok(lines(&machine_dir.join(LIST_FILE))?.unwrap_or_default())
}

pub fn blind(salt: &[u8; 32], root: &[u8; 32]) -> [u8; 32] {
    let mut m = Vec::with_capacity(64);
    m.extend_from_slice(salt);
    m.extend_from_slice(root);
    zikaron::cryptox::sha256(&m)
}

pub fn contains(machine_dir: &Path, root: &[u8; 32]) -> Result<bool, Trouble> {
    let Some(salt) = salt(machine_dir)? else { return Ok(false) };
    let want = blind(&salt, root);
    Ok(list(machine_dir)?.contains(&want))
}

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
