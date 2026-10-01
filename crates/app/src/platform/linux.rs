//! Linux: the file dialog goes through the XDG desktop portal (`rfd`, portal backend only, no GTK); the rest is
//! shared with macOS (`unix.rs`); the temporary directory is `$XDG_RUNTIME_DIR`, else `/tmp`.

pub(super) use super::unix::{home_dir, lock_now, lock_wait, zone_rules};

/// Whether the dialog failed is told by the dialog itself: `rfd` answers "nothing chosen" alike for a
/// cancel and a failure, and says failures only through `log`. So this interface watches `rfd`'s own records:
/// a warning marks it moving to its fallback (`zenity`), an error after that (or with no fallback) marks the
/// dialog as not opened. A cancel logs nothing.
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

pub(super) fn choose_path(kind: super::Pick) -> Option<String> {
    if log::set_logger(&WATCH).is_ok() {
        log::set_max_level(log::LevelFilter::Warn);
    }
    FAILED.store(false, std::sync::atomic::Ordering::SeqCst);
    let d = rfd::FileDialog::new();
    let got = match kind {
        super::Pick::Folder => d.pick_folder(),
        // The portal picks one kind per dialog: files here, and a directory still arrives by dropping it.
        super::Pick::File | super::Pick::FileOrFolder => d.pick_file(),
    };
    if got.is_none() && FAILED.load(std::sync::atomic::Ordering::SeqCst) {
        super::say(crate::fault::Fault::known(crate::fault::Known::DialogUnavailable, String::new()));
    }
    got.map(|p| p.display().to_string())
}

pub(super) fn window_backend() -> super::Backend {
    match std::env::var_os("DISPLAY") {
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
