//! The platform interface: the six things this app needs from the operating system, each in one place.
//!
//! | Capability | Function | macOS | Linux |
//! |---|---|---|---|
//! | System file dialog | [`choose_path`] | AppKit `NSOpenPanel` (Objective-C runtime) | `rfd` over the XDG desktop portal (no GTK) |
//! | Single-writer lock | [`lock_now`], [`lock_wait`] | `flock` | `flock` |
//! | The user's home directory | [`home_dir`] | `$HOME` | `$HOME` |
//! | The system time zone | [`zone_rules`] | `TZ`, else `/etc/localtime` | `TZ`, else `/etc/localtime` |
//! | Chinese font faces | `zikaron_ui::fonts::ROLES` (the widget library installs fonts) | system PingFang | Noto Sans SC embedded in the build (OFL) |
//! | The per-user temporary directory | [`user_temp_dir`] | `confstr(_CS_DARWIN_USER_TEMP_DIR)` | `$XDG_RUNTIME_DIR`, else `/tmp` |
//!
//! Every other module calls these functions and never the operating system for these things; this is the only
//! module with `extern "C"` and the only one that names a platform crate. A port to another system implements
//! one file beside `macos.rs` and `linux.rs` with the same six functions (and adds its row to the font table);
//! nothing else changes. Building for a system without such a file stops at compile time.

#[cfg(unix)]
mod unix;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as imp;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
compile_error!("no platform implementation for this system: add one file beside platform/macos.rs and platform/linux.rs");

/// What the system file dialog allows. Set by what the drop area takes: places taking only files allow only
/// files; places taking files, directories and git repositories allow both.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    /// Only one file.
    File,
    /// Only one directory.
    Folder,
    /// A file or a directory (one at a time; a system whose dialog picks one kind at a time offers files, and a
    /// directory still arrives by dropping it).
    FileOrFolder,
}

/// Pick a path with the system file dialog, one at a time. Returns the chosen path; `None` on cancel. When the
/// dialog cannot open at all, `None` too, and the reason is kept for the frame to say ([`take_trouble`]), so a
/// dialog that never opened is never read as a cancel. Modal: while it is open this frame waits for the person.
pub fn choose_path(kind: Pick) -> Option<String> {
    imp::choose_path(kind)
}

static TROUBLE: std::sync::Mutex<Option<crate::fault::Fault>> = std::sync::Mutex::new(None);

/// Keep why the file dialog could not open (an implementation calls it).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn say(f: crate::fault::Fault) {
    if let Ok(mut g) = TROUBLE.lock() {
        *g = Some(f);
    }
}

/// Take why the last file dialog could not open (the frame asks once per frame and says it).
pub fn take_trouble() -> Option<crate::fault::Fault> {
    TROUBLE.lock().ok().and_then(|mut g| g.take())
}

/// Which window system the window opens on. On Linux the window opens on X11 when an X display is there
/// (XWayland on a Wayland desktop): the windowing layer delivers dropped files on X11 and not on Wayland, and
/// dropping is how a folder comes in; without an X display it opens on Wayland.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backend {
    /// Whatever the windowing layer picks (macOS; Linux without an X display).
    Default,
    /// X11.
    X11,
}

pub fn window_backend() -> Backend {
    imp::window_backend()
}

/// Ask the kernel for an exclusive lock on this open file without waiting; true when obtained. The kernel
/// releases it when the process goes, so a stale lock cannot arise.
pub fn lock_now(file: &std::fs::File) -> bool {
    imp::lock_now(file)
}

/// The same lock, waiting until it is obtained.
pub fn lock_wait(file: &std::fs::File) -> bool {
    imp::lock_wait(file)
}

/// The user's home directory; `None` when the system does not say.
pub fn home_dir() -> Option<std::path::PathBuf> {
    imp::home_dir()
}

/// The system time zone's rules as the system keeps them: the bytes of a zone file (TZif), or the `TZ` value as a
/// POSIX rule string when it names no readable file (the rule parser judges it). `None` when neither `TZ` nor
/// `/etc/localtime` gives anything, or the file given is not a zone file.
pub fn zone_rules() -> Option<Zone> {
    imp::zone_rules()
}

/// How the system gives its zone.
pub enum Zone {
    /// The bytes of a TZif zone file.
    File(Vec<u8>),
    /// A POSIX TZ rule (`CET-1CEST,M3.5.0,M10.5.0/3`).
    Rule(String),
}

/// The system's own per-user temporary directory, whatever `TMPDIR` says (so a caller that set `TMPDIR` can be
/// told apart from the system default); `None` when the system does not say.
pub fn user_temp_dir() -> Option<std::path::PathBuf> {
    imp::user_temp_dir()
}
