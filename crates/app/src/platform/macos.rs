//! macOS: the file dialog is AppKit's NSOpenPanel through the Objective-C runtime (the system's own framework,
//! no third-party crate); the rest is shared with Linux (`unix.rs`); the temporary directory is the one the
//! system gives this user (`confstr(_CS_DARWIN_USER_TEMP_DIR)`), whatever `TMPDIR` says. Asked once, modally,
//! in the frame, starting no thread and no child process.

pub(super) use super::unix::{app_data_dir, home_dir, lock_now, lock_wait, say_without_window, zone_rules};

use std::ffi::c_void;

// ───────────────────────── Objective-C runtime (needed by the file dialog) ─────────────────────────

#[link(name = "Foundation", kind = "framework")]
extern "C" {}

extern "C" {
    fn objc_getClass(name: *const std::ffi::c_char) -> *mut c_void;
    fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
    fn objc_msgSend();
}

fn sel(name: &str) -> *mut c_void {
    let c = std::ffi::CString::new(name).unwrap_or_default();
    unsafe { sel_registerName(c.as_ptr()) }
}

// ───────────────────────── System file dialog (AppKit NSOpenPanel) ─────────────────────────

#[link(name = "AppKit", kind = "framework")]
extern "C" {}

/// NSModalResponseOK.
const MODAL_OK: isize = 1;

/// Pick a path: the system file dialog, allowing files, directories or both by `kind`, one at a time. Returns
/// the chosen path; `None` on cancel or when the dialog cannot open. Modal: while it is open this frame waits
/// for the person (the system's modal dialog covers the window anyway).
pub(super) fn choose_path(kind: super::Pick) -> Option<String> {
    let (files, folders) = match kind {
        super::Pick::File => (true, false),
        super::Pick::Folder => (false, true),
        super::Pick::FileOrFolder => (true, true),
    };
    unsafe {
        let cls = objc_getClass(c"NSOpenPanel".as_ptr());
        if cls.is_null() {
            return None;
        }
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ());
        let panel = send(cls, sel("openPanel"));
        if panel.is_null() {
            return None;
        }
        let set: unsafe extern "C" fn(*mut c_void, *mut c_void, bool) = std::mem::transmute(objc_msgSend as *const ());
        set(panel, sel("setCanChooseFiles:"), files);
        set(panel, sel("setCanChooseDirectories:"), folders);
        set(panel, sel("setAllowsMultipleSelection:"), false);
        let run: unsafe extern "C" fn(*mut c_void, *mut c_void) -> isize = std::mem::transmute(objc_msgSend as *const ());
        if run(panel, sel("runModal")) != MODAL_OK {
            return None;
        }
        let url = send(panel, sel("URL"));
        if url.is_null() {
            return None;
        }
        let path = send(url, sel("path"));
        if path.is_null() {
            return None;
        }
        let utf8: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const std::ffi::c_char = std::mem::transmute(objc_msgSend as *const ());
        let raw = utf8(path, sel("UTF8String"));
        if raw.is_null() {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(raw).to_string_lossy().to_string())
    }
}

extern "C" {
    fn confstr(name: i32, buf: *mut u8, len: usize) -> usize;
}

/// `_CS_DARWIN_USER_TEMP_DIR`.
const CS_DARWIN_USER_TEMP_DIR: i32 = 65537;

pub(super) fn window_backend() -> super::Backend {
    super::Backend::Default
}

pub(super) fn user_temp_dir() -> Option<std::path::PathBuf> {
    let mut buf = vec![0u8; 1024];
    let n = unsafe { confstr(CS_DARWIN_USER_TEMP_DIR, buf.as_mut_ptr(), buf.len()) };
    if n == 0 || n > buf.len() {
        return None;
    }
    buf.truncate(n - 1);
    String::from_utf8(buf).ok().map(std::path::PathBuf::from)
}
