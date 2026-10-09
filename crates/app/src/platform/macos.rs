//! macOS: the file dialog is AppKit's NSOpenPanel through the Objective-C runtime (no third-party crate); lock,
//! zone and error output are shared with Linux (`unix.rs`); the temporary directory is the per-user one from
//! `confstr(_CS_DARWIN_USER_TEMP_DIR)`, ignoring `TMPDIR`. The panel opens without blocking the run loop
//! (`beginWithCompletionHandler:`), so the window keeps drawing, and no child process is started.

pub(super) use super::unix::{lock_now, lock_wait, say_without_window, zone_rules};

use std::ffi::c_void;

// ───────────────────────── Objective-C runtime (needed by the file dialog) ─────────────────────────

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {}

unsafe extern "C" {
    fn objc_getClass(name: *const std::ffi::c_char) -> *mut c_void;
    fn sel_registerName(name: *const std::ffi::c_char) -> *mut c_void;
    fn objc_msgSend();
}

fn sel(name: &str) -> *mut c_void {
    let c = std::ffi::CString::new(name).unwrap_or_default();
    unsafe { sel_registerName(c.as_ptr()) }
}

// ───────────────────────── System file dialog (AppKit NSOpenPanel, not holding the run loop) ─────────────────────────

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

/// NSModalResponseOK.
const MODAL_OK: isize = 1;

/// The open panel, if any (the window never opens a second one meanwhile): the panel, retained until it
/// answers, and the channel for its answer.
static OPEN: std::sync::Mutex<Option<(usize, std::sync::mpsc::Sender<Option<String>>)>> = std::sync::Mutex::new(None);

/// The completion handler: a block with no captures (the answer goes through [`OPEN`]), laid out per the
/// Apple block ABI, so no third-party crate is needed.
#[repr(C)]
struct BlockDescriptor {
    reserved: usize,
    size: usize,
}

#[repr(C)]
struct Block {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: unsafe extern "C" fn(*const Block, isize),
    descriptor: *const BlockDescriptor,
}

// The block is a constant the system only reads.
unsafe impl Sync for Block {}

unsafe extern "C" {
    static _NSConcreteGlobalBlock: c_void;
}

/// `BLOCK_IS_GLOBAL`: a static block; copying returns the same block.
const BLOCK_IS_GLOBAL: i32 = 1 << 28;

static DESCRIPTOR: BlockDescriptor = BlockDescriptor { reserved: 0, size: std::mem::size_of::<Block>() };

static ANSWERED: Block = Block { isa: &raw const _NSConcreteGlobalBlock, flags: BLOCK_IS_GLOBAL, reserved: 0, invoke: answered, descriptor: &DESCRIPTOR };

/// Called on the run loop's thread when the panel closes: read the chosen path on OK, release the panel, and
/// send the answer to the wait.
unsafe extern "C" fn answered(_block: *const Block, response: isize) {
    let taken = OPEN.lock().ok().and_then(|mut g| g.take());
    if let Some((panel, tx)) = taken {
        let panel = panel as *mut c_void;
        let path = if response == MODAL_OK { unsafe { path_of(panel) } } else { None };
        unsafe { message(panel, "release") };
        let _ = tx.send(path);
    }
}

unsafe fn message(to: *mut c_void, name: &str) -> *mut c_void {
    let send: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { send(to, sel(name)) }
}

/// The chosen path of a panel that answered OK.
unsafe fn path_of(panel: *mut c_void) -> Option<String> {
    let url = unsafe { message(panel, "URL") };
    if url.is_null() {
        return None;
    }
    let path = unsafe { message(url, "path") };
    if path.is_null() {
        return None;
    }
    let utf8: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const std::ffi::c_char = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    let raw = unsafe { utf8(path, sel("UTF8String")) };
    if raw.is_null() {
        return None;
    }
    Some(unsafe { std::ffi::CStr::from_ptr(raw) }.to_string_lossy().to_string())
}

/// Open the system file dialog, allowing files, directories or both per `kind`, one item at a time. The panel
/// opens here, on the UI thread, and this returns at once (the run loop continues and the window keeps
/// drawing); the wait yields the chosen path, or `None` on cancel. A panel that cannot be created is an error.
pub(super) fn ask_path(kind: super::Pick) -> Result<super::Wait, crate::fault::Fault> {
    let (files, folders) = match kind {
        super::Pick::File => (true, false),
        super::Pick::Folder => (false, true),
        super::Pick::FileOrFolder => (true, true),
    };
    let unavailable = || crate::fault::Fault::known(crate::fault::Known::DialogUnavailable, String::new());
    let rx = unsafe {
        let cls = objc_getClass(c"NSOpenPanel".as_ptr());
        if cls.is_null() {
            return Err(unavailable());
        }
        let panel = message(cls, "openPanel");
        if panel.is_null() {
            return Err(unavailable());
        }
        let set: unsafe extern "C" fn(*mut c_void, *mut c_void, bool) = std::mem::transmute(objc_msgSend as *const ());
        set(panel, sel("setCanChooseFiles:"), files);
        set(panel, sel("setCanChooseDirectories:"), folders);
        set(panel, sel("setAllowsMultipleSelection:"), false);
        // Retained until it answers (the handler releases it).
        message(panel, "retain");
        let (tx, rx) = std::sync::mpsc::channel();
        if let Ok(mut g) = OPEN.lock() {
            *g = Some((panel as usize, tx));
        }
        let begin: unsafe extern "C" fn(*mut c_void, *mut c_void, *const Block) = std::mem::transmute(objc_msgSend as *const ());
        begin(panel, sel("beginWithCompletionHandler:"), &ANSWERED);
        rx
    };
    // A panel that closes without answering (e.g. the app quitting) counts as nothing chosen.
    Ok(Box::new(move || Ok(rx.recv().unwrap_or(None))))
}

unsafe extern "C" {
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
