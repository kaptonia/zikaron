pub(super) use super::unix::{say_without_window, zone_rules};

use std::ffi::c_void;

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

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {}

const MODAL_OK: isize = 1;

static OPEN: std::sync::Mutex<Option<(usize, std::sync::mpsc::Sender<Option<String>>)>> = std::sync::Mutex::new(None);

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

unsafe impl Sync for Block {}

unsafe extern "C" {
    static _NSConcreteGlobalBlock: c_void;
}

const BLOCK_IS_GLOBAL: i32 = 1 << 28;

static DESCRIPTOR: BlockDescriptor = BlockDescriptor { reserved: 0, size: std::mem::size_of::<Block>() };

static ANSWERED: Block = Block { isa: &raw const _NSConcreteGlobalBlock, flags: BLOCK_IS_GLOBAL, reserved: 0, invoke: answered, descriptor: &DESCRIPTOR };

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
        message(panel, "retain");
        let (tx, rx) = std::sync::mpsc::channel();
        if let Ok(mut g) = OPEN.lock() {
            *g = Some((panel as usize, tx));
        }
        let begin: unsafe extern "C" fn(*mut c_void, *mut c_void, *const Block) = std::mem::transmute(objc_msgSend as *const ());
        begin(panel, sel("beginWithCompletionHandler:"), &ANSWERED);
        rx
    };
    Ok(Box::new(move || Ok(rx.recv().unwrap_or(None))))
}

unsafe extern "C" {
    fn confstr(name: i32, buf: *mut u8, len: usize) -> usize;
}

const CS_DARWIN_USER_TEMP_DIR: i32 = 65537;

pub(super) fn dialog_brings_folders() -> bool {
    true
}

pub(super) fn asks_file_or_folder() -> bool {
    false
}

pub(super) fn window_backend() -> super::Backend {
    super::Backend::Default
}

pub(super) fn watch_drops(_window: super::drops::Window, _wake: super::drops::Wake) -> Option<super::drops::Watcher> {
    None
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

unsafe extern "C" {
    fn objc_allocateClassPair(superclass: *mut c_void, name: *const std::ffi::c_char, extra: usize) -> *mut c_void;
    fn objc_registerClassPair(cls: *mut c_void);
    fn class_addMethod(cls: *mut c_void, name: *mut c_void, imp: *const c_void, types: *const std::ffi::c_char) -> bool;
}

const OPTION: usize = 1 << 19;
const COMMAND: usize = 1 << 20;

struct Installed {
    items: Vec<(super::menubar::Act, usize)>,
    columns: Vec<(super::menubar::Column, usize, usize)>,
    words: super::menubar::Words,
}

static BAR: std::sync::Mutex<Option<Installed>> = std::sync::Mutex::new(None);

unsafe fn ns_string(s: &str) -> *mut c_void {
    let c = std::ffi::CString::new(s.replace('\0', "")).unwrap_or_default();
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *const std::ffi::c_char) -> *mut c_void = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(objc_getClass(c"NSString".as_ptr()), sel("stringWithUTF8String:"), c.as_ptr()) }
}

unsafe fn rust_string(ns: *mut c_void) -> String {
    if ns.is_null() {
        return String::new();
    }
    let utf8: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *const std::ffi::c_char = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    let raw = unsafe { utf8(ns, sel("UTF8String")) };
    if raw.is_null() { String::new() } else { unsafe { std::ffi::CStr::from_ptr(raw) }.to_string_lossy().to_string() }
}

unsafe fn send1(to: *mut c_void, name: &str, arg: *mut c_void) -> *mut c_void {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> *mut c_void = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(to, sel(name), arg) }
}

unsafe fn send_usize(to: *mut c_void, name: &str, arg: usize) {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { f(to, sel(name), arg) }
}

unsafe fn new_menu(title: &str) -> *mut c_void {
    let m = unsafe { message(objc_getClass(c"NSMenu".as_ptr()), "alloc") };
    unsafe { send1(m, "initWithTitle:", ns_string(title)) }
}

unsafe fn new_item(title: &str, action: &str, key: &str) -> *mut c_void {
    let f: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void, *mut c_void) -> *mut c_void = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    let it = unsafe { message(objc_getClass(c"NSMenuItem".as_ptr()), "alloc") };
    let action = if action.is_empty() { std::ptr::null_mut() } else { sel(action) };
    unsafe { f(it, sel("initWithTitle:action:keyEquivalent:"), ns_string(title), action, ns_string(key)) }
}

fn target() -> *mut c_void {
    static TARGET: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *TARGET.get_or_init(|| unsafe {
        let cls = objc_allocateClassPair(objc_getClass(c"NSObject".as_ptr()), c"ZikaronMenuTarget".as_ptr(), 0);
        if cls.is_null() {
            return 0;
        }
        class_addMethod(cls, sel("zikaronAbout:"), about_pressed as *const c_void, c"v@:@".as_ptr());
        class_addMethod(cls, sel("zikaronSettings:"), settings_pressed as *const c_void, c"v@:@".as_ptr());
        #[cfg(target_arch = "aarch64")]
        class_addMethod(cls, sel("validateMenuItem:"), validate as *const c_void, c"B@:@".as_ptr());
        #[cfg(not(target_arch = "aarch64"))]
        class_addMethod(cls, sel("validateMenuItem:"), validate as *const c_void, c"c@:@".as_ptr());
        class_addMethod(cls, sel("menuNeedsUpdate:"), prune as *const c_void, c"v@:@".as_ptr());
        objc_registerClassPair(cls);
        let made = message(message(cls, "alloc"), "init");
        made as usize
    }) as *mut c_void
}

unsafe extern "C" fn prune(_this: *mut c_void, _cmd: *mut c_void, menu: *mut c_void) {
    unsafe { prune_menu(menu) }
}

unsafe fn prune_menu(menu: *mut c_void) {
    let count: unsafe extern "C" fn(*mut c_void, *mut c_void) -> isize = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    let at: unsafe extern "C" fn(*mut c_void, *mut c_void, isize) -> *mut c_void = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    let tag: unsafe extern "C" fn(*mut c_void, *mut c_void) -> isize = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    let remove: unsafe extern "C" fn(*mut c_void, *mut c_void, isize) = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    let n = unsafe { count(menu, sel("numberOfItems")) };
    for i in (0..n).rev() {
        let it = unsafe { at(menu, sel("itemAtIndex:"), i) };
        if !it.is_null() && unsafe { tag(it, sel("tag")) } != OURS {
            unsafe { remove(menu, sel("removeItemAtIndex:"), i) };
        }
    }
}

const OURS: isize = 0x5A1C_0B35;

unsafe fn system_additions_off() {
    let defaults = unsafe { message(objc_getClass(c"NSUserDefaults".as_ptr()), "standardUserDefaults") };
    if !defaults.is_null() {
        let set: unsafe extern "C" fn(*mut c_void, *mut c_void, bool, *mut c_void) = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        for (key, on) in [("NSDisabledDictationMenuItem", true), ("NSDisabledCharacterPaletteMenuItem", true), ("NSFullScreenMenuItemEverywhere", false)] {
            unsafe { set(defaults, sel("setBool:forKey:"), on, ns_string(key)) };
        }
    }
    let tabbing: unsafe extern "C" fn(*mut c_void, *mut c_void, bool) = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    unsafe { tabbing(objc_getClass(c"NSWindow".as_ptr()), sel("setAllowsAutomaticWindowTabbing:"), false) };
}

unsafe extern "C" fn about_pressed(_this: *mut c_void, _cmd: *mut c_void, _sender: *mut c_void) {
    super::menubar::ask(super::menubar::Act::About);
}

unsafe extern "C" fn settings_pressed(_this: *mut c_void, _cmd: *mut c_void, _sender: *mut c_void) {
    super::menubar::ask(super::menubar::Act::Settings);
}

#[cfg(target_arch = "aarch64")]
unsafe extern "C" fn validate(_this: *mut c_void, _cmd: *mut c_void, item: *mut c_void) -> bool {
    unsafe { validated(item) }
}

#[cfg(not(target_arch = "aarch64"))]
unsafe extern "C" fn validate(_this: *mut c_void, _cmd: *mut c_void, item: *mut c_void) -> i8 {
    i8::from(unsafe { validated(item) })
}

unsafe fn validated(item: *mut c_void) -> bool {
    let action = unsafe { message(item, "action") };
    if action == sel(super::menubar::Act::Settings.selector()) {
        super::menubar::enabled(super::menubar::Act::Settings)
    } else {
        super::menubar::enabled(super::menubar::Act::About)
    }
}

pub(super) fn menu_bar(words: &super::menubar::Words) {
    use super::menubar::{Entry, BAR as TABLE};
    let mut held = BAR.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(b) = held.as_mut() {
        if b.words == *words {
            return;
        }
        unsafe {
            for (act, it) in &b.items {
                send1(*it as *mut c_void, "setTitle:", ns_string(words.item(*act)));
            }
            for (c, top, menu) in &b.columns {
                send1(*top as *mut c_void, "setTitle:", ns_string(words.column(*c)));
                send1(*menu as *mut c_void, "setTitle:", ns_string(words.column(*c)));
            }
        }
        b.words = words.clone();
        return;
    }
    unsafe {
        let app = message(objc_getClass(c"NSApplication".as_ptr()), "sharedApplication");
        if app.is_null() {
            return;
        }
        system_additions_off();
        let bar = new_menu("");
        let mut items = Vec::new();
        let mut columns = Vec::new();
        for (column, entries) in TABLE {
            let title = words.column(column);
            let top = new_item(title, "", "");
            let menu = new_menu(title);
            for e in entries {
                match *e {
                    Entry::Sep => {
                        let s = message(objc_getClass(c"NSMenuItem".as_ptr()), "separatorItem");
                        send_usize(s, "setTag:", OURS as usize);
                        send1(menu, "addItem:", s);
                    }
                    Entry::Item { act, key, option } => {
                        let it = new_item(words.item(act), act.selector(), key);
                        send_usize(it, "setTag:", OURS as usize);
                        if option {
                            send_usize(it, "setKeyEquivalentModifierMask:", COMMAND | OPTION);
                        }
                        if act.own() {
                            send1(it, "setTarget:", target());
                        }
                        if act == super::menubar::Act::Services {
                            let services = new_menu(words.item(act));
                            send1(it, "setSubmenu:", services);
                            send1(app, "setServicesMenu:", services);
                        }
                        send1(menu, "addItem:", it);
                        items.push((act, it as usize));
                    }
                }
            }
            send1(menu, "setDelegate:", target());
            send1(top, "setSubmenu:", menu);
            send1(bar, "addItem:", top);
            columns.push((column, top as usize, menu as usize));
        }
        send1(app, "setMainMenu:", bar);
        *held = Some(Installed { items, columns, words: words.clone() });
    }
}

pub(super) fn menu_takes_key(chars: &str) -> bool {
    unsafe {
        let app = message(objc_getClass(c"NSApplication".as_ptr()), "sharedApplication");
        let bar = if app.is_null() { std::ptr::null_mut() } else { message(app, "mainMenu") };
        if bar.is_null() {
            return false;
        }
        #[repr(C)]
        struct Point {
            x: f64,
            y: f64,
        }
        type Make = unsafe extern "C" fn(*mut c_void, *mut c_void, usize, Point, usize, f64, isize, *mut c_void, *mut c_void, *mut c_void, bool, u16) -> *mut c_void;
        let make: Make = std::mem::transmute(objc_msgSend as *const ());
        let event = make(objc_getClass(c"NSEvent".as_ptr()), sel("keyEventWithType:location:modifierFlags:timestamp:windowNumber:context:characters:charactersIgnoringModifiers:isARepeat:keyCode:"), 10, Point { x: 0.0, y: 0.0 }, COMMAND, 0.0, 0, std::ptr::null_mut(), ns_string(chars), ns_string(chars), false, 43);
        if event.is_null() {
            return false;
        }
        #[cfg(target_arch = "aarch64")]
        let took: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> bool = std::mem::transmute(objc_msgSend as *const ());
        #[cfg(target_arch = "aarch64")]
        return took(bar, sel("performKeyEquivalent:"), event);
        #[cfg(not(target_arch = "aarch64"))]
        {
            let took: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void) -> i8 = std::mem::transmute(objc_msgSend as *const ());
            took(bar, sel("performKeyEquivalent:"), event) != 0
        }
    }
}

pub(super) fn menu_bar_read() -> Vec<(String, Vec<(String, String, bool)>)> {
    let mut out = Vec::new();
    unsafe {
        let app = message(objc_getClass(c"NSApplication".as_ptr()), "sharedApplication");
        let bar = if app.is_null() { std::ptr::null_mut() } else { message(app, "mainMenu") };
        if bar.is_null() {
            return out;
        }
        let count: unsafe extern "C" fn(*mut c_void, *mut c_void) -> isize = std::mem::transmute(objc_msgSend as *const ());
        let at: unsafe extern "C" fn(*mut c_void, *mut c_void, isize) -> *mut c_void = std::mem::transmute(objc_msgSend as *const ());
        #[cfg(target_arch = "aarch64")]
        let yes: unsafe extern "C" fn(*mut c_void, *mut c_void) -> bool = std::mem::transmute(objc_msgSend as *const ());
        #[cfg(not(target_arch = "aarch64"))]
        let yes = {
            let f: unsafe extern "C" fn(*mut c_void, *mut c_void) -> i8 = std::mem::transmute(objc_msgSend as *const ());
            move |a, b| f(a, b) != 0
        };
        for i in 0..count(bar, sel("numberOfItems")) {
            let top = at(bar, sel("itemAtIndex:"), i);
            let menu = message(top, "submenu");
            if menu.is_null() {
                continue;
            }
            message(menu, "update");
            let delegate = message(menu, "delegate");
            if !delegate.is_null() {
                send1(delegate, "menuNeedsUpdate:", menu);
            }
            let mut rows = Vec::new();
            for j in 0..count(menu, sel("numberOfItems")) {
                let it = at(menu, sel("itemAtIndex:"), j);
                if yes(it, sel("isSeparatorItem")) {
                    continue;
                }
                rows.push((rust_string(message(it, "title")), rust_string(message(it, "keyEquivalent")), yes(it, sel("isEnabled"))));
            }
            out.push((rust_string(message(menu, "title")), rows));
        }
    }
    out
}
