pub(super) fn say_without_window(line: &str, _sentence: &str) {
    eprintln!("{line}");
}

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
