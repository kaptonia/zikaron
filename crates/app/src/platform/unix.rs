//! What macOS and Linux do the same way: `$HOME`, and the zone from `TZ` or `/etc/localtime` (the kernel lock is
//! `zikaron-os`'s).

/// On Unix the reason goes to standard error only.
pub(super) fn say_without_window(line: &str, _sentence: &str) {
    eprintln!("{line}");
}

/// `TZ` names a zone file, a path or a POSIX rule; unset means `/etc/localtime`. A file counts only if it has
/// the TZif magic; a `TZ` naming no readable file is returned as a rule for the rule parser (the POSIX reading
/// of `TZ`).
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
