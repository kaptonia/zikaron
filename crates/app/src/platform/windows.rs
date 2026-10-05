//! Windows: the file dialog is `rfd` over the system's own dialog; the rest is the system's interfaces,
//! declared here: the lock is a byte-range lock (`LockFileEx`), the home directory and the application data
//! folder are known folders (`SHGetKnownFolderPath`), the time zone is `TZ` or the system's current zone rule
//! (`GetTimeZoneInformation`) written as a POSIX rule for the rule engine, the temporary directory is the
//! system's per-user one (`GetTempPath2W`, or `GetTempPathW` where the system has no `GetTempPath2W`).

use std::ffi::c_void;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};

type Handle = *mut c_void;

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

/// `FOLDERID_Profile`.
const FOLDER_PROFILE: Guid = Guid { data1: 0x5E6C_858F, data2: 0x0E22, data3: 0x4760, data4: [0x9A, 0xFE, 0xEA, 0x33, 0x17, 0xB6, 0x71, 0x73] };
/// `FOLDERID_LocalAppData`.
const FOLDER_LOCAL_APP_DATA: Guid = Guid { data1: 0xF1B3_2785, data2: 0x6FBA, data3: 0x4FCF, data4: [0x9D, 0x55, 0x7B, 0x8E, 0x7F, 0x15, 0x70, 0x91] };

#[repr(C)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: Handle,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

#[repr(C)]
struct TimeZoneInformation {
    bias: i32,
    standard_name: [u16; 32],
    standard_date: SystemTime,
    standard_bias: i32,
    daylight_name: [u16; 32],
    daylight_date: SystemTime,
    daylight_bias: i32,
}

const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x1;
const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x2;
const TIME_ZONE_ID_INVALID: u32 = 0xFFFF_FFFF;

#[link(name = "kernel32")]
extern "system" {
    fn LockFileEx(file: Handle, flags: u32, reserved: u32, low: u32, high: u32, overlapped: *mut Overlapped) -> i32;
    fn GetTimeZoneInformation(info: *mut TimeZoneInformation) -> u32;
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GetProcAddress(module: Handle, name: *const u8) -> *mut c_void;
    fn GetTempPathW(len: u32, buf: *mut u16) -> u32;
}

#[link(name = "user32")]
extern "system" {
    fn MessageBoxW(owner: Handle, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

#[link(name = "shell32")]
extern "system" {
    fn SHGetKnownFolderPath(id: *const Guid, flags: u32, token: Handle, path: *mut *mut u16) -> i32;
}

#[link(name = "ole32")]
extern "system" {
    fn CoTaskMemFree(p: *mut c_void);
}

/// `MB_OK | MB_ICONERROR | MB_SETFOREGROUND`.
const DIALOG_KIND: u32 = 0x0000_0010 | 0x0001_0000;

/// The folder this app's machine data lives in, under the system's local application data folder.
const APP_FOLDER: &str = "ZIKARON";

pub(super) fn choose_path(kind: super::Pick) -> Option<String> {
    let d = rfd::FileDialog::new();
    let got = match kind {
        super::Pick::Folder => d.pick_folder(),
        // The system dialog picks one kind per dialog: files here, and a directory still arrives by dropping it.
        super::Pick::File | super::Pick::FileOrFolder => d.pick_file(),
    };
    got.map(|p| p.display().to_string())
}

/// A window program here has no console: started from Explorer, standard error goes nowhere a person sees, and
/// the sentence is shown in the system's own dialog, which needs no window of this app and no graphics beyond
/// the system's. When standard error does go somewhere (a console, a pipe, a file: the caller attached it), the
/// line is read there and no dialog is shown, since a dialog waits for a click whoever started the program may
/// never give.
pub(super) fn say_without_window(line: &str, sentence: &str) {
    eprintln!("{line}");
    if !std::io::stderr().as_raw_handle().is_null() {
        return;
    }
    let wide = |s: &str| -> Vec<u16> { std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect() };
    let (text, caption) = (wide(sentence), wide("ZIKARON"));
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), DIALOG_KIND) };
}

pub(super) fn window_backend() -> super::Backend {
    super::Backend::Default
}

/// The locked range: one byte far beyond any data the file holds. Windows locks are mandatory, and the writer
/// lock file carries the holder's process number, which the settings page reads back while the lock is held.
const LOCK_OFFSET_HIGH: u32 = 0x4000_0000;

fn lock(file: &std::fs::File, flags: u32) -> bool {
    let mut at = Overlapped { internal: 0, internal_high: 0, offset: 0, offset_high: LOCK_OFFSET_HIGH, event: std::ptr::null_mut() };
    unsafe { LockFileEx(file.as_raw_handle() as Handle, flags, 0, 1, 0, &mut at) != 0 }
}

pub(super) fn lock_now(file: &std::fs::File) -> bool {
    lock(file, LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY)
}

pub(super) fn lock_wait(file: &std::fs::File) -> bool {
    lock(file, LOCKFILE_EXCLUSIVE_LOCK)
}

fn known_folder(id: &Guid) -> Option<PathBuf> {
    let mut p: *mut u16 = std::ptr::null_mut();
    let r = unsafe { SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut p) };
    // The buffer is the caller's to free whether or not the call succeeded.
    let out = if r == 0 && !p.is_null() {
        let wide = unsafe {
            let n = (0..).take_while(|i| *p.add(*i) != 0).count();
            std::slice::from_raw_parts(p, n).to_vec()
        };
        Some(PathBuf::from(std::ffi::OsString::from_wide(&wide))).filter(|x| !x.as_os_str().is_empty())
    } else {
        None
    };
    unsafe { CoTaskMemFree(p.cast()) };
    out
}

pub(super) fn home_dir() -> Option<PathBuf> {
    known_folder(&FOLDER_PROFILE)
}

/// The system's local application data folder (`%LOCALAPPDATA%`, which does not roam), this app's folder in
/// it. A stand-in home (the test hooks) keeps the same layout under itself, so a test never reaches the
/// person's own folder.
pub(super) fn app_data_dir(user_home: &Path) -> PathBuf {
    if home_dir().as_deref() == Some(user_home) {
        if let Some(local) = known_folder(&FOLDER_LOCAL_APP_DATA) {
            return local.join(APP_FOLDER);
        }
    }
    user_home.join("AppData").join("Local").join(APP_FOLDER)
}

/// `TZ` when set (a POSIX rule; Windows keeps no zone files); otherwise the zone the system is set to, its
/// offsets and daylight switches written as a POSIX rule ([`posix_rule`]). No zone database is carried: a zone
/// whose rules changed over the years has its older dates read by today's rule, as a POSIX `TZ` is.
pub(super) fn zone_rules() -> Option<super::Zone> {
    if let Ok(tz) = std::env::var("TZ") {
        let tz = tz.trim().trim_start_matches(':');
        if !tz.is_empty() {
            return Some(super::Zone::Rule(tz.to_string()));
        }
    }
    let mut info = TimeZoneInformation {
        bias: 0,
        standard_name: [0; 32],
        standard_date: SystemTime::default(),
        standard_bias: 0,
        daylight_name: [0; 32],
        daylight_date: SystemTime::default(),
        daylight_bias: 0,
    };
    if unsafe { GetTimeZoneInformation(&mut info) } == TIME_ZONE_ID_INVALID {
        return None;
    }
    posix_rule(&info).map(super::Zone::Rule)
}

/// The system's zone as a POSIX rule: the standard offset (minutes west of UTC, as the system keeps its bias),
/// and when the zone has daylight time, its offset and the two switches as `Mm.w.d/time` (week 5 is the last
/// week, as in the system's own form). The start is in standard local time and the end in daylight local time,
/// in both forms. A switch given as one year's date rather than a yearly rule has no POSIX form: `None`.
fn posix_rule(z: &TimeZoneInformation) -> Option<String> {
    let offset = |west: i32| {
        let (sign, m) = if west < 0 { ("-", -west) } else { ("", west) };
        if m % 60 == 0 {
            format!("{sign}{}", m / 60)
        } else {
            format!("{sign}{}:{:02}", m / 60, m % 60)
        }
    };
    let switch = |t: &SystemTime| -> Option<String> {
        let rule = t.year == 0 && (1..=12).contains(&t.month) && (1..=5).contains(&t.day) && t.day_of_week <= 6;
        rule.then(|| format!("M{}.{}.{}/{}:{:02}:{:02}", t.month, t.day, t.day_of_week, t.hour, t.minute, t.second))
    };
    let std = offset(z.bias + z.standard_bias);
    if z.daylight_date.month == 0 {
        return Some(format!("<STD>{std}"));
    }
    let dst = offset(z.bias + z.daylight_bias);
    Some(format!("<STD>{std}<DST>{dst},{},{}", switch(&z.daylight_date)?, switch(&z.standard_date)?))
}

pub(super) fn user_temp_dir() -> Option<PathBuf> {
    type TempPath = unsafe extern "system" fn(u32, *mut u16) -> u32;
    let kernel: Vec<u16> = std::ffi::OsStr::new("kernel32.dll").encode_wide().chain(std::iter::once(0)).collect();
    let newer = unsafe {
        let m = GetModuleHandleW(kernel.as_ptr());
        if m.is_null() {
            std::ptr::null_mut()
        } else {
            GetProcAddress(m, c"GetTempPath2W".as_ptr().cast())
        }
    };
    let call: TempPath = if newer.is_null() { GetTempPathW } else { unsafe { std::mem::transmute::<*mut c_void, TempPath>(newer) } };
    let mut buf = vec![0u16; 261];
    let n = unsafe { call(buf.len() as u32, buf.as_mut_ptr()) } as usize;
    if n == 0 || n > buf.len() {
        return None;
    }
    buf.truncate(n);
    Some(PathBuf::from(std::ffi::OsString::from_wide(&buf)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(bias: i32, std: (u16, u16, u16, u16), dst: Option<((u16, u16, u16, u16), i32)>) -> TimeZoneInformation {
        let at = |(month, day, day_of_week, hour): (u16, u16, u16, u16)| SystemTime { year: 0, month, day_of_week, day, hour, ..SystemTime::default() };
        TimeZoneInformation {
            bias,
            standard_name: [0; 32],
            standard_date: if dst.is_some() { at(std) } else { SystemTime::default() },
            standard_bias: 0,
            daylight_name: [0; 32],
            daylight_date: dst.map(|(d, _)| at(d)).unwrap_or_default(),
            daylight_bias: dst.map(|(_, b)| b).unwrap_or(0),
        }
    }

    /// The system's form becomes the POSIX rule the rule engine reads, on both hemispheres, with half hours,
    /// without daylight time, and a one-year date has no rule.
    #[test]
    fn the_system_zone_becomes_a_posix_rule() {
        // Central Europe: UTC+1, daylight from the last Sunday of March 02:00 to the last Sunday of October 03:00.
        assert_eq!(posix_rule(&zone(-60, (10, 5, 0, 3), Some(((3, 5, 0, 2), -60)))).as_deref(), Some("<STD>-1<DST>-2,M3.5.0/2:00:00,M10.5.0/3:00:00"));
        // US Eastern: UTC-5, second Sunday of March to first Sunday of November.
        assert_eq!(posix_rule(&zone(300, (11, 1, 0, 2), Some(((3, 2, 0, 2), -60)))).as_deref(), Some("<STD>5<DST>4,M3.2.0/2:00:00,M11.1.0/2:00:00"));
        // India and China: no daylight time; half an hour kept.
        assert_eq!(posix_rule(&zone(-330, (0, 0, 0, 0), None)).as_deref(), Some("<STD>-5:30"));
        assert_eq!(posix_rule(&zone(-480, (0, 0, 0, 0), None)).as_deref(), Some("<STD>-8"));
        let mut once = zone(-60, (10, 5, 0, 3), Some(((3, 5, 0, 2), -60)));
        once.daylight_date.year = 2026;
        assert_eq!(posix_rule(&once), None);
    }

    /// The app data folder of a stand-in home stays under it.
    #[test]
    fn a_stand_in_home_keeps_the_app_folder_under_it() {
        let stand_in = Path::new(r"C:\stand-in\home");
        assert_eq!(app_data_dir(stand_in), stand_in.join("AppData").join("Local").join(APP_FOLDER));
    }
}
