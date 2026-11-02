//! Linux: the file dialog goes through the XDG desktop portal (`rfd`, portal backend only, no GTK); the rest is
//! shared with macOS (`unix.rs`); the temporary directory is `$XDG_RUNTIME_DIR`, else `/tmp`.

pub(super) use super::unix::{say_without_window, zone_rules};

/// Detects dialog failures. `rfd` returns "nothing chosen" for both a cancel and a failure, and reports
/// failures only through `log`, so this logger watches `rfd`'s records: a warning means it is moving to its
/// fallback (`zenity`), and an error after that (or with no fallback) means the dialog did not open. A cancel
/// logs nothing.
struct Watch;

static FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static WATCH: Watch = Watch;

impl log::Log for Watch {
    fn enabled(&self, m: &log::Metadata) -> bool {
        m.target().starts_with("rfd") && m.level() <= log::Level::Warn
    }
    fn log(&self, r: &log::Record) {
        if !self.enabled(r.metadata()) {
            return;
        }
        match r.level() {
            log::Level::Error => FAILED.store(true, std::sync::atomic::Ordering::SeqCst),
            _ => FAILED.store(false, std::sync::atomic::Ordering::SeqCst),
        }
    }
    fn flush(&self) {}
}

/// The dialog runs inside the wait, on the background task's thread (where the portal answers), so the window
/// keeps drawing while it is open.
pub(super) fn ask_path(kind: super::Pick) -> Result<super::Wait, crate::fault::Fault> {
    Ok(Box::new(move || {
        if log::set_logger(&WATCH).is_ok() {
            log::set_max_level(log::LevelFilter::Warn);
        }
        FAILED.store(false, std::sync::atomic::Ordering::SeqCst);
        let d = rfd::FileDialog::new();
        let got = match kind {
            super::Pick::Folder => d.pick_folder(),
            // The portal picks one kind per dialog: offer files; a directory can still be dropped.
            super::Pick::File | super::Pick::FileOrFolder => d.pick_file(),
        };
        if got.is_none() && FAILED.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(crate::fault::Fault::known(crate::fault::Known::DialogUnavailable, String::new()));
        }
        Ok(got.map(|p| p.display().to_string()))
    }))
}

/// Environment variable choosing the window system: `x11` or `wayland` (any case). Unset, empty or anything
/// else keeps the automatic choice.
const WINDOW_SYSTEM_ENV: &str = "ZIKARON_WINDOW_SYSTEM";

pub(super) fn window_backend() -> super::Backend {
    pick_backend(std::env::var_os(WINDOW_SYSTEM_ENV).as_deref(), std::env::var_os("DISPLAY").as_deref())
}

/// The window system for a `ZIKARON_WINDOW_SYSTEM` value and a `DISPLAY` value: the named one when the value
/// names one, else X11 when there is an X display, else the windowing layer's own pick.
fn pick_backend(choice: Option<&std::ffi::OsStr>, display: Option<&std::ffi::OsStr>) -> super::Backend {
    match choice.and_then(|c| c.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("x11") => return super::Backend::X11,
        Some("wayland") => return super::Backend::Wayland,
        _ => {}
    }
    match display {
        Some(d) if !d.is_empty() => super::Backend::X11,
        _ => super::Backend::Default,
    }
}

pub(super) fn user_temp_dir() -> Option<std::path::PathBuf> {
    match std::env::var_os("XDG_RUNTIME_DIR").filter(|v| !v.is_empty()) {
        Some(v) => Some(std::path::PathBuf::from(v)),
        None => Some(std::path::PathBuf::from("/tmp")),
    }
}

#[cfg(test)]
mod tests {
    use super::{pick_backend, super::Backend};
    use std::ffi::OsStr;

    #[test]
    fn the_window_system_switch_wins_over_the_x_display() {
        let x = Some(OsStr::new(":0"));
        assert_eq!(pick_backend(None, x), Backend::X11);
        assert_eq!(pick_backend(None, None), Backend::Default);
        assert_eq!(pick_backend(None, Some(OsStr::new(""))), Backend::Default);
        assert_eq!(pick_backend(Some(OsStr::new("wayland")), x), Backend::Wayland);
        assert_eq!(pick_backend(Some(OsStr::new("Wayland")), None), Backend::Wayland);
        assert_eq!(pick_backend(Some(OsStr::new("x11")), None), Backend::X11);
        assert_eq!(pick_backend(Some(OsStr::new("")), x), Backend::X11);
        assert_eq!(pick_backend(Some(OsStr::new("mir")), None), Backend::Default);
    }
}
