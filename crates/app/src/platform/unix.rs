//! What macOS and Linux do the same way: the kernel lock, `$HOME`, and the zone from `TZ` or `/etc/localtime`.

extern "C" {
    fn flock(fd: i32, op: i32) -> i32;
}

const LOCK_EX: i32 = 2;
const LOCK_NB: i32 = 4;

pub(super) fn lock_now(file: &std::fs::File) -> bool {
    use std::os::unix::io::AsRawFd;
    unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) == 0 }
}

pub(super) fn lock_wait(file: &std::fs::File) -> bool {
    use std::os::unix::io::AsRawFd;
    unsafe { flock(file.as_raw_fd(), LOCK_EX) == 0 }
}

pub(super) fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").filter(|h| !h.is_empty()).map(std::path::PathBuf::from)
}

/// `TZ` names a zone file, a path or a POSIX rule; unset means `/etc/localtime`. A file counts only when it is a
/// zone file (TZif magic); a `TZ` that names no readable file is handed over as a rule for the rule parser to
/// judge (the POSIX reading of `TZ`).
pub(super) fn zone_rules() -> Option<super::Zone> {
    let tzif = |b: Vec<u8>| if b.starts_with(b"TZif") { Some(super::Zone::File(b)) } else { None };
    match std::env::var("TZ") {
        Ok(tz) if !tz.trim().is_empty() => {
            let name = tz.trim().trim_start_matches(':');
            let path = if name.starts_with('/') { std::path::PathBuf::from(name) } else { std::path::Path::new("/usr/share/zoneinfo").join(name) };
            match std::fs::read(&path) {
                Ok(b) => tzif(b),
                Err(_) => Some(super::Zone::Rule(name.to_string())),
            }
        }
        _ => std::fs::read("/etc/localtime").ok().and_then(tzif),
    }
}
