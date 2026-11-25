#[cfg(unix)]
mod unix;

pub mod drops;
pub mod menubar;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Pick {
    File,
    Folder,
    FileOrFolder,
}

pub type Wait = Box<dyn FnOnce() -> Result<Option<String>, crate::fault::Fault> + Send>;

pub type Asker = fn(Pick) -> Result<Wait, crate::fault::Fault>;

pub fn ask_path(kind: Pick) -> Result<Wait, crate::fault::Fault> {
    imp::ask_path(kind)
}

pub fn dialog_brings_folders() -> bool {
    imp::dialog_brings_folders()
}

pub fn asks_file_or_folder() -> bool {
    imp::asks_file_or_folder()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backend {
    Default,
    X11,
}

pub fn window_backend() -> Backend {
    imp::window_backend()
}

pub fn backend_of(devices: Option<drops::Devices>, display: Option<&std::ffi::OsStr>) -> Backend {
    match (devices, display) {
        (Some(drops::Devices::NewestOnly), Some(d)) if !d.is_empty() => Backend::X11,
        _ => Backend::Default,
    }
}

pub fn lock_now(file: &std::fs::File) -> bool {
    zikaron_os::lock_now(file)
}

pub fn lock_wait(file: &std::fs::File) -> bool {
    zikaron_os::lock_wait(file)
}

pub fn home_dir() -> Option<std::path::PathBuf> {
    zikaron_os::user_home()
}

pub fn app_data_dir(user_home: &std::path::Path) -> std::path::PathBuf {
    zikaron_os::app_data_dir(user_home)
}

pub fn zone_rules() -> Option<Zone> {
    imp::zone_rules()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Zone {
    File(Vec<u8>),
    Rule(String),
    ByYear(Vec<(i64, String)>),
}

pub fn say_without_window(line: &str, sentence: &str) {
    imp::say_without_window(line, sentence)
}

pub fn user_temp_dir() -> Option<std::path::PathBuf> {
    imp::user_temp_dir()
}

pub fn menu_bar(words: &menubar::Words) {
    imp::menu_bar(words)
}

pub fn menu_takes_key(chars: &str) -> bool {
    imp::menu_takes_key(chars)
}

pub fn menu_bar_read() -> Vec<(String, Vec<(String, String, bool)>)> {
    imp::menu_bar_read()
}
