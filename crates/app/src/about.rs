pub const NOTICES: &str = include_str!(concat!(env!("OUT_DIR"), "/third-party-notices.txt"));

pub fn notice_lines() -> &'static [&'static str] {
    static LINES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    LINES.get_or_init(|| NOTICES.lines().collect())
}

pub fn count() -> usize {
    NOTICES.lines().take_while(|l| !l.trim().is_empty()).count()
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn source_epoch() -> Option<u64> {
    epoch_of(option_env!("ZIKARON_SOURCE_EPOCH"))
}

fn epoch_of(raw: Option<&str>) -> Option<u64> {
    raw.map(str::trim).filter(|x| !x.is_empty()).and_then(|x| x.parse().ok())
}

pub fn updated(epoch: Option<u64>) -> Option<String> {
    epoch.map(|e| crate::lang::fill1(crate::lang::Key::AboutUpdated, &crate::when::day(e)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facts {
    pub icon: &'static [u8],
    pub icon_side: u32,
    pub name: String,
    pub version: String,
    pub updated: Option<String>,
}

pub const ICON: &[u8] = include_bytes!("../assets/icon-256.rgba");

impl Facts {
    pub fn now() -> Facts {
        Facts { icon: ICON, icon_side: 256, name: crate::lang::t(crate::lang::Key::AppName).to_string(), version: format!("{} {VERSION}", crate::lang::t(crate::lang::Key::Version)), updated: updated(source_epoch()) }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_about_facts_come_from_their_one_place() {
        assert_eq!(super::VERSION, env!("CARGO_PKG_VERSION"));
        let page = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/window/settings.rs")).expect("settings.rs");
        assert!(page.contains("crate::about::VERSION") && !page.contains("env!(\"CARGO_PKG_VERSION\")"), "the settings page reads the one version");
        assert_eq!(super::epoch_of(Some("1763424000")), Some(1_763_424_000));
        assert_eq!(super::epoch_of(Some("")), None);
        assert_eq!(super::epoch_of(Some("x")), None);
        assert_eq!(super::epoch_of(None), None);
        crate::when::set(crate::when::Zone::Utc);
        let line = super::updated(Some(1_763_424_000)).expect("a line");
        assert!(line.contains("2025-11-18"), "{line}");
        assert_eq!(super::updated(None), None, "no moment, no line");
        assert_eq!(super::ICON.len(), 256 * 256 * 4, "the window's icon");
    }

    #[test]
    fn the_notices_name_the_crates_the_app_links() {
        let head: Vec<&str> = super::NOTICES.lines().take(super::count()).collect();
        assert!(head.iter().any(|l| l.starts_with("k256 ")), "k256 is listed");
        assert!(head.iter().any(|l| l.starts_with("rustls ")), "rustls is listed");
        assert!(head.iter().all(|l| l.split(" \u{b7} ").nth(1).map(|x| !x.trim().is_empty()).unwrap_or(false)), "every line carries a licence");
        assert!(super::NOTICES.contains("Apache License"), "licence texts follow");
    }

    #[test]
    fn the_notices_are_split_once() {
        let a = super::notice_lines();
        let b = super::notice_lines();
        assert!(std::ptr::eq(a, b), "the same table, not split again");
        assert_eq!(a, super::NOTICES.lines().collect::<Vec<_>>().as_slice());
    }
}
