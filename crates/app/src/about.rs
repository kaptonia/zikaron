//! Third-party notices, generated at build time from `Cargo.lock` for the target (`build.rs`) and embedded
//! here. The "About" settings page shows them.

/// Every third-party crate linked on this target (name, version, licence expression, one per line), then each
/// distinct licence text once under the crates that ship it.
pub const NOTICES: &str = include_str!(concat!(env!("OUT_DIR"), "/third-party-notices.txt"));

/// The notice lines, split once per process: the about page renders only the visible lines each frame, and
/// re-splitting the whole text every frame is too slow.
pub fn notice_lines() -> &'static [&'static str] {
    static LINES: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    LINES.get_or_init(|| NOTICES.lines().collect())
}

/// How many third-party crates the notices list (the lines before the first licence text).
pub fn count() -> usize {
    NOTICES.lines().take_while(|l| !l.trim().is_empty()).count()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_notices_name_the_crates_the_app_links() {
        // The TLS stack and the curve are linked on every target; every listed crate has a licence.
        let head: Vec<&str> = super::NOTICES.lines().take(super::count()).collect();
        assert!(head.iter().any(|l| l.starts_with("k256 ")), "k256 is listed");
        assert!(head.iter().any(|l| l.starts_with("rustls ")), "rustls is listed");
        assert!(head.iter().all(|l| l.split(" \u{b7} ").nth(1).map(|x| !x.trim().is_empty()).unwrap_or(false)), "every line carries a licence");
        assert!(super::NOTICES.contains("Apache License"), "licence texts follow");
    }

    /// The notices are split into lines once per process.
    #[test]
    fn the_notices_are_split_once() {
        let a = super::notice_lines();
        let b = super::notice_lines();
        assert!(std::ptr::eq(a, b), "the same table, not split again");
        assert_eq!(a, super::NOTICES.lines().collect::<Vec<_>>().as_slice());
    }
}
