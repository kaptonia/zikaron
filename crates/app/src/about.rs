//! The third-party notices the app carries, made at build time from `Cargo.lock` for the target built
//! (`build.rs`) and embedded here; the settings page "About" shows them.

/// Every third-party crate linked on this target (name, version, licence expression, one per line), then each
/// distinct licence text once under the crates that ship it.
pub const NOTICES: &str = include_str!(concat!(env!("OUT_DIR"), "/third-party-notices.txt"));

/// How many third-party crates the notices list (the lines before the first licence text).
pub fn count() -> usize {
    NOTICES.lines().take_while(|l| !l.trim().is_empty()).count()
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_notices_name_the_crates_the_app_links() {
        // The TLS stack and the curve are linked on every target; each listed line has a licence.
        let head: Vec<&str> = super::NOTICES.lines().take(super::count()).collect();
        assert!(head.iter().any(|l| l.starts_with("k256 ")), "k256 is listed");
        assert!(head.iter().any(|l| l.starts_with("rustls ")), "rustls is listed");
        assert!(head.iter().all(|l| l.split(" \u{b7} ").nth(1).map(|x| !x.trim().is_empty()).unwrap_or(false)), "every line carries a licence");
        assert!(super::NOTICES.contains("Apache License"), "licence texts follow");
    }
}
