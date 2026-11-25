use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub const URI_LIST: &str = "text/uri-list";

pub type Wake = Arc<dyn Fn() + Send + Sync>;

#[derive(Debug, Default)]
pub struct Drops {
    over: bool,
    dropped: Vec<PathBuf>,
}

impl Drops {
    pub fn entered(&mut self, ours: bool, offers_files: bool) {
        self.over = ours && offers_files;
    }

    pub fn left(&mut self) {
        self.over = false;
    }

    pub fn delivered(&mut self, list: &[u8]) {
        self.over = false;
        self.dropped.extend(paths_of_uri_list(list));
    }

    pub fn hovering(&self) -> bool {
        self.over
    }

    pub fn take(&mut self) -> Vec<PathBuf> {
        std::mem::take(&mut self.dropped)
    }
}

pub type Stop = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub struct Watcher {
    pub(super) shared: Arc<Mutex<Drops>>,
    stop: Option<Stop>,
}

impl Watcher {
    pub fn over(shared: Arc<Mutex<Drops>>) -> Watcher {
        Watcher { shared, stop: None }
    }

    pub fn stopped_by(shared: Arc<Mutex<Drops>>, stop: Stop) -> Watcher {
        Watcher { shared, stop: Some(stop) }
    }

    pub fn stop(&self) {
        if let Some(s) = self.stop.as_ref() {
            s();
        }
    }

    pub fn hovering(&self) -> bool {
        self.shared.lock().map(|d| d.hovering()).unwrap_or(false)
    }

    pub fn take(&self) -> Vec<PathBuf> {
        self.shared.lock().map(|mut d| d.take()).unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Window {
    Wayland { display: *mut std::ffi::c_void, surface: *mut std::ffi::c_void },
    Other,
}

pub fn watch(window: Window, wake: Wake) -> Option<Watcher> {
    super::imp::watch_drops(window, wake)
}

pub fn taken_here(window: Window) -> bool {
    matches!(window, Window::Wayland { display, surface } if !display.is_null() && !surface.is_null())
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Devices {
    Every,
    NewestOnly,
}

const NEWEST_ONLY_MARK: &str = "gtk_shell1";

pub fn devices_of_globals<'a>(interfaces: impl IntoIterator<Item = &'a str>) -> Devices {
    if interfaces.into_iter().any(|i| i == NEWEST_ONLY_MARK) {
        Devices::NewestOnly
    } else {
        Devices::Every
    }
}

pub fn binds_beside_clipboard(devices: Devices) -> bool {
    devices == Devices::Every
}

pub fn paths_of_uri_list(list: &[u8]) -> Vec<PathBuf> {
    let text = String::from_utf8_lossy(list);
    text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).trim()).filter(|l| !l.is_empty() && !l.starts_with('#')).filter_map(path_of_file_uri).collect()
}

fn path_of_file_uri(uri: &str) -> Option<PathBuf> {
    let (scheme, rest) = uri.split_once(':')?;
    if !scheme.eq_ignore_ascii_case("file") {
        return None;
    }
    let path = match rest.strip_prefix("//") {
        Some(after) => {
            let (host, path) = after.split_at(after.find('/')?);
            if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
                return None;
            }
            path
        }
        None => rest,
    };
    if !path.starts_with('/') {
        return None;
    }
    let path = path.split(['?', '#']).next().unwrap_or(path);
    let bytes = unescape(path)?;
    path_of_bytes(bytes)
}

fn unescape(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = b.get(i + 1..i + 3)?;
            let v = u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()?;
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    Some(out)
}

fn path_of_bytes(bytes: Vec<u8>) -> Option<PathBuf> {
    String::from_utf8(bytes).ok().filter(|s| !s.contains('\0')).map(PathBuf::from)
}
