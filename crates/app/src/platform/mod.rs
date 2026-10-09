//! The platform interface: the eight things this app needs from the operating system.
//!
//! | Capability | Function | macOS | Linux | Windows |
//! |---|---|---|---|---|
//! | System file dialog | [`ask_path`] (returns at once; the answer is awaited off the UI thread) | AppKit `NSOpenPanel` (Objective-C runtime) | `rfd` over the XDG desktop portal (no GTK) | `rfd` over the system dialog |
//! | Single-writer lock | [`lock_now`], [`lock_wait`] | `flock` | `flock` | `LockFileEx` on one byte beyond the data |
//! | The user's home directory | [`home_dir`] | `$HOME` | `$HOME` | the profile known folder |
//! | The system time zone | [`zone_rules`] | `TZ`, else `/etc/localtime` | `TZ`, else `/etc/localtime` | `TZ`, else the system's zone written as a POSIX rule |
//! | Chinese font faces | `zikaron_ui::fonts::ROLES` (the widget library installs fonts) | system PingFang | Noto Sans SC embedded in the build (OFL) | system Microsoft YaHei UI, else as Linux |
//! | The per-user temporary directory | [`user_temp_dir`] | `confstr(_CS_DARWIN_USER_TEMP_DIR)` | `$XDG_RUNTIME_DIR`, else `/tmp` | `GetTempPath2W` (`GetTempPathW` before it) |
//! | Where this app keeps its machine data by default | [`app_data_dir`] | the home directory | the home directory | `%LOCALAPPDATA%\ZIKARON` |
//! | Telling the user why there is no window | [`say_without_window`] | standard error | standard error | standard error, and a system dialog when standard error goes nowhere |
//!
//! OS facilities that the core, store, glue and command line also need (entropy, owner-only files, syncing to
//! disk, replacing rename, non-replacing rename, system proxy settings) live in the `zikaron-os` crate; the
//! network transport reads the proxy settings itself.
//!
//! Other modules use these functions rather than calling the OS directly; this is the only app module with
//! `extern` blocks or platform crates. A port to another system adds one file beside `macos.rs`, `linux.rs`
//! and `windows.rs` with the same functions (and a row in the font table); nothing else changes. Building for
//! a system without such a file fails at compile time.

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

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as imp;

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
compile_error!("no platform implementation for this system: add one file beside platform/macos.rs, platform/linux.rs and platform/windows.rs");

/// What the system file dialog allows, matching the drop area it serves: file-only areas allow files; areas
/// that take files, directories and git repositories allow both.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    /// Only one file.
    File,
    /// Only one directory.
    Folder,
    /// A file or a directory (one at a time). Where the system dialog picks only one kind, it offers files and
    /// a directory can still be dropped.
    FileOrFolder,
}

/// Waits for the answer to an open file dialog. Run off the UI thread (in a background task), it returns the
/// chosen path, `None` on cancel, or an error when the dialog could not open (never reported as a cancel).
pub type Wait = Box<dyn FnOnce() -> Result<Option<String>, crate::fault::Fault> + Send>;

/// The function that asks the user for a path: normally [`ask_path`]; a windowless run substitutes its own.
pub type Asker = fn(Pick) -> Result<Wait, crate::fault::Fault>;

/// Ask the user for a path with the system file dialog without blocking the UI: returns at once with a
/// [`Wait`], and the window keeps drawing (and handling background results) while the dialog is open. On
/// macOS the panel opens here, on the UI thread, and answers into the wait; elsewhere the wait opens the
/// dialog on its own thread. The answer is delivered through the task channel (`task::Kind::Path`) to the
/// view that asked.
pub fn ask_path(kind: Pick) -> Result<Wait, crate::fault::Fault> {
    imp::ask_path(kind)
}

/// Which window system to open on. On Linux the window uses X11 when an X display exists (XWayland on a
/// Wayland desktop), because the windowing layer delivers dropped files on X11 but not on Wayland, and
/// dropping is how folders are added; without an X display it uses Wayland.
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
/// releases it when the process exits, so stale locks cannot arise.
pub fn lock_now(file: &std::fs::File) -> bool {
    imp::lock_now(file)
}

/// The same lock, blocking until obtained.
pub fn lock_wait(file: &std::fs::File) -> bool {
    imp::lock_wait(file)
}

/// The user's home directory; `None` when the system does not provide one. Implemented in `zikaron-os` so the
/// command line finds the machine directory the same way.
pub fn home_dir() -> Option<std::path::PathBuf> {
    zikaron_os::user_home()
}

/// Where this app keeps its machine data by default, given the user's home directory (or the test stand-in):
/// the home directory itself on macOS and Linux, the user's local application data folder on Windows. The
/// pointer to the machine directory is kept there too.
pub fn app_data_dir(user_home: &std::path::Path) -> std::path::PathBuf {
    zikaron_os::app_data_dir(user_home)
}

/// The system time zone rules: the bytes of a TZif zone file, or the `TZ` value as a POSIX rule string when it
/// names no readable file (validated later by the rule parser). `None` when neither `TZ` nor `/etc/localtime`
/// gives anything, or the file is not a zone file.
pub fn zone_rules() -> Option<Zone> {
    imp::zone_rules()
}

/// A time zone as provided by the system.
pub enum Zone {
    /// The bytes of a TZif zone file.
    File(Vec<u8>),
    /// A POSIX TZ rule (`CET-1CEST,M3.5.0,M10.5.0/3`).
    Rule(String),
}

/// Tell the user why the app exits without a window (misuse, or the window could not be created). `line` goes
/// to standard error (for a terminal or log); `sentence` is the same reason for a person, in the system
/// language, with what to do. Where the app has no terminal (Windows), `sentence` is also shown in a system
/// dialog when standard error goes nowhere. The caller sets the exit code.
pub fn say_without_window(line: &str, sentence: &str) {
    imp::say_without_window(line, sentence)
}

/// The system's own per-user temporary directory, ignoring `TMPDIR` (so a caller-set `TMPDIR` can be told
/// apart from the system default); `None` when the system does not provide one.
pub fn user_temp_dir() -> Option<std::path::PathBuf> {
    imp::user_temp_dir()
}
