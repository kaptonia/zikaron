//! Windows: the file dialog is `rfd` over the system dialog; everything else uses Win32 APIs declared here.
//! The lock is a byte-range lock (`LockFileEx`); the home and application data folders are known folders
//! (read in `zikaron-os`, shared with the command line); the time zone is `TZ` or the current system zone
//! (`GetTimeZoneInformation`) converted to a POSIX rule; the temporary directory is the per-user one
//! (`GetTempPath2W`, or `GetTempPathW` on systems without it).

use std::ffi::c_void;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::AsRawHandle;
use std::path::{Path, PathBuf};

type Handle = *mut c_void;

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
unsafe extern "system" {
    fn LockFileEx(file: Handle, flags: u32, reserved: u32, low: u32, high: u32, overlapped: *mut Overlapped) -> i32;
    fn GetTimeZoneInformation(info: *mut TimeZoneInformation) -> u32;
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GetProcAddress(module: Handle, name: *const u8) -> *mut c_void;
    fn GetTempPathW(len: u32, buf: *mut u16) -> u32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(owner: Handle, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

/// `MB_OK | MB_ICONERROR | MB_SETFOREGROUND`.
const DIALOG_KIND: u32 = 0x0000_0010 | 0x0001_0000;

/// The dialog runs inside the wait, on the background task's thread (with no owner window), so the window
/// keeps drawing while it is open.
pub(super) fn ask_path(kind: super::Pick) -> Result<super::Wait, crate::fault::Fault> {
    Ok(Box::new(move || {
        let d = rfd::FileDialog::new();
        let got = match kind {
            super::Pick::Folder => d.pick_folder(),
            // The system dialog picks one kind per dialog: offer files; a directory can still be dropped.
            super::Pick::File | super::Pick::FileOrFolder => d.pick_file(),
        };
        Ok(got.map(|p| p.display().to_string()))
    }))
}

/// A GUI program has no console: started from Explorer, standard error goes nowhere visible, so the sentence is
/// shown in a system message box (which needs no app window). When standard error is attached (a console,
/// pipe or file), the line goes there and no dialog is shown, since a dialog would wait for a click that may
/// never come.
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

/// The locked range: one byte far beyond any data in the file. Windows locks are mandatory, and the writer
/// lock file holds the holder's pid, which the settings page reads while the lock is held.
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

/// `TZ` when set (a POSIX rule; Windows has no zone files); otherwise the system's zone, with its offsets and
/// daylight transitions written as a POSIX rule ([`posix_rule`]). No zone database is bundled, so historical
/// dates use today's rule, as with a POSIX `TZ`.
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

/// The system zone as a POSIX rule: the standard offset (minutes west of UTC, like the system's bias), and for
/// zones with daylight time, its offset and both transitions as `Mm.w.d/time` (week 5 means the last week, in
/// both forms). The start is in standard local time and the end in daylight local time, in both forms. A
/// transition given as a specific year's date rather than a yearly rule has no POSIX form: `None`.
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

    /// System zones convert to POSIX rules (both hemispheres, half-hour offsets, no daylight time); a
    /// single-year transition gives `None`.
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
}
