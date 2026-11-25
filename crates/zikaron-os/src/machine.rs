use std::path::{Path, PathBuf};

pub const APP_DIR: &str = ".zikaron-desk.d";

pub const POINTER: &str = ".zikaron-desk";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Pointer,
    Default,
}

#[derive(Debug)]
pub enum Unread {
    NoHome,
    PointerShape(PathBuf),
    Pointer(PathBuf, std::io::Error),
}

pub fn pointer_dir(user_home: &Path) -> PathBuf {
    crate::app_data_dir(user_home)
}

pub fn read_pointer(bytes: &[u8], folder: &Path) -> Option<PathBuf> {
    let s = std::str::from_utf8(bytes).ok()?;
    let line = s.strip_suffix('\n')?;
    if line.is_empty() || line.contains('\n') || line.contains('\r') {
        return None;
    }
    let p = Path::new(line);
    if p.is_absolute() {
        return Some(PathBuf::from(line));
    }
    let plain = p.components().all(|c| matches!(c, std::path::Component::Normal(_)));
    plain.then(|| folder.join(p))
}

pub fn pointer_bytes(machine: &Path, folder: &Path) -> Vec<u8> {
    let inside = machine.strip_prefix(folder).ok().filter(|r| r.components().next().is_some() && r.components().all(|c| matches!(c, std::path::Component::Normal(_))));
    match inside {
        Some(rel) => format!("{}\n", rel.display()).into_bytes(),
        None => format!("{}\n", machine.display()).into_bytes(),
    }
}

pub fn settle_pointer(user_home: &Path, machine: &Path) -> std::io::Result<bool> {
    let folder = pointer_dir(user_home);
    let ptr = folder.join(POINTER);
    let Ok(now) = std::fs::read(&ptr) else { return Ok(false) };
    if read_pointer(&now, &folder).as_deref() != Some(machine) {
        return Ok(false);
    }
    let want = pointer_bytes(machine, &folder);
    if want == now || read_pointer(&want, &folder).as_deref() != Some(machine) {
        return Ok(false);
    }
    let tmp = folder.join(format!("{POINTER}.settle-{}", std::process::id()));
    {
        use std::io::Write;
        let mut o = crate::Options::new();
        o.write(true).create(true).truncate(true);
        crate::owner_only(&mut o);
        let mut f = o.open(&tmp)?;
        f.write_all(&want)?;
        f.sync_all()?;
    }
    if let Err(e) = std::fs::rename(&tmp, &ptr) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(true)
}

pub fn of(user_home: &Path) -> Result<(PathBuf, Layer), Unread> {
    let folder = pointer_dir(user_home);
    let ptr = folder.join(POINTER);
    let read_pointer = |bytes: &[u8]| self::read_pointer(bytes, &folder);
    match std::fs::read(&ptr) {
        Ok(bytes) => return read_pointer(&bytes).map(|p| (p, Layer::Pointer)).ok_or(Unread::PointerShape(ptr)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(Unread::Pointer(ptr, e)),
    }
    Ok((crate::app_data_dir(user_home).join(APP_DIR), Layer::Default))
}

pub fn here() -> Result<(PathBuf, Layer), Unread> {
    of(&crate::user_home().ok_or(Unread::NoHome)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("zikaron-os-machine-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("scratch");
        d
    }

    #[test]
    fn a_pointer_is_settled_to_its_one_form_and_never_laid() {
        let u = scratch("settle");
        let folder = pointer_dir(&u);
        std::fs::create_dir_all(&folder).expect("folder");
        let inside = folder.join("machine-here");
        assert!(!settle_pointer(&u, &inside).expect("none"), "no pointer: none laid");
        assert!(!folder.join(POINTER).exists());
        std::fs::write(folder.join(POINTER), format!("{}\n", inside.display())).expect("absolute");
        assert!(settle_pointer(&u, &inside).expect("rewritten"), "absolute inside: rewritten");
        assert_eq!(std::fs::read(folder.join(POINTER)).expect("read"), pointer_bytes(&inside, &folder));
        assert_eq!(read_pointer(&std::fs::read(folder.join(POINTER)).expect("read"), &folder).as_deref(), Some(inside.as_path()));
        assert!(!settle_pointer(&u, &inside).expect("same"), "already in its form: not written again");
        let outside = scratch("settle-out");
        std::fs::write(folder.join(POINTER), format!("{}\n", outside.display())).expect("outside");
        assert!(!settle_pointer(&u, &outside).expect("outside"), "outside the folder: left absolute");
        assert!(!settle_pointer(&u, &inside).expect("another"), "names another place: left");
        assert_eq!(std::fs::read(folder.join(POINTER)).expect("read"), format!("{}\n", outside.display()).into_bytes());
    }

    #[test]
    fn the_machine_directory_is_read_level_by_level() {
        let u = scratch("levels");
        assert_eq!(of(&u).expect("default").1, Layer::Default);
        assert_eq!(of(&u).expect("default").0, crate::app_data_dir(&u).join(APP_DIR));
        let earlier = ["Library", "Application Support", "ZIKARON"].iter().fold(u.clone(), |p, s| p.join(s));
        std::fs::create_dir_all(&earlier).expect("an earlier version's folder");
        assert_eq!(of(&u).expect("default"), (crate::app_data_dir(&u).join(APP_DIR), Layer::Default), "not read");
        let there = u.join("elsewhere");
        std::fs::create_dir_all(pointer_dir(&u)).expect("pointer dir");
        std::fs::write(pointer_dir(&u).join(POINTER), format!("{}\n", there.display())).expect("pointer");
        assert_eq!(of(&u).expect("pointer"), (there, Layer::Pointer));
        for bad in [&b"../relative\n"[..], b"./here\n", b"a/../b\n", b"/no/newline", b"/two\n/lines\n", b"\n", b"\xff\n", b"rel\r\n", b""] {
            std::fs::write(pointer_dir(&u).join(POINTER), bad).expect("bad pointer");
            assert!(matches!(of(&u), Err(Unread::PointerShape(_))), "{bad:?}");
        }
        let _ = std::fs::remove_dir_all(&u);
    }

    #[test]
    fn a_machine_directory_inside_the_folder_is_kept_relative_and_follows_a_copied_home() {
        let u = scratch("relative");
        let folder = pointer_dir(&u);
        std::fs::create_dir_all(&folder).expect("pointer dir");
        let inside = folder.join("Data").join("machine");
        let bytes = pointer_bytes(&inside, &folder);
        assert_eq!(bytes, format!("{}\n", Path::new("Data").join("machine").display()).into_bytes(), "written relative");
        assert_eq!(read_pointer(&bytes, &folder), Some(inside.clone()), "read back as written");
        std::fs::write(folder.join(POINTER), &bytes).expect("pointer");
        assert_eq!(of(&u).expect("relative").0, inside);
        let v = scratch("relative-copy");
        let vfolder = pointer_dir(&v);
        std::fs::create_dir_all(&vfolder).expect("copy's pointer dir");
        std::fs::write(vfolder.join(POINTER), &bytes).expect("copied pointer");
        assert_eq!(of(&v).expect("copy").0, vfolder.join("Data").join("machine"), "the copy reads its own");
        let outside = std::env::temp_dir().join(format!("zikaron-os-outside-{}", std::process::id()));
        let abs = pointer_bytes(&outside, &folder);
        assert_eq!(abs, format!("{}\n", outside.display()).into_bytes(), "written as given");
        std::fs::write(vfolder.join(POINTER), &abs).expect("outside pointer");
        assert_eq!(of(&v).expect("outside").0, outside, "read as written in the copy too");
        std::fs::write(folder.join(POINTER), format!("{}\n", inside.display())).expect("old pointer");
        assert_eq!(of(&u).expect("old form").0, inside);
        assert_eq!(pointer_bytes(&folder, &folder), format!("{}\n", folder.display()).into_bytes());
        let _ = std::fs::remove_dir_all(&u);
        let _ = std::fs::remove_dir_all(&v);
    }
}
