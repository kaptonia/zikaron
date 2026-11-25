use std::ffi::c_void;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::AsRawHandle;
use std::path::PathBuf;

type Handle = *mut c_void;

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
#[derive(Clone, Copy)]
struct TimeZoneInformation {
    bias: i32,
    standard_name: [u16; 32],
    standard_date: SystemTime,
    standard_bias: i32,
    daylight_name: [u16; 32],
    daylight_date: SystemTime,
    daylight_bias: i32,
}

impl TimeZoneInformation {
    fn zero() -> TimeZoneInformation {
        TimeZoneInformation {
            bias: 0,
            standard_name: [0; 32],
            standard_date: SystemTime::default(),
            standard_bias: 0,
            daylight_name: [0; 32],
            daylight_date: SystemTime::default(),
            daylight_bias: 0,
        }
    }
}

#[repr(C)]
struct DynamicTimeZoneInformation {
    zone: TimeZoneInformation,
    key_name: [u16; 128],
    dynamic_daylight_time_disabled: u8,
}

const TIME_ZONE_ID_INVALID: u32 = 0xFFFF_FFFF;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetTimeZoneInformation(info: *mut TimeZoneInformation) -> u32;
    fn GetDynamicTimeZoneInformation(info: *mut DynamicTimeZoneInformation) -> u32;
    fn GetTimeZoneInformationForYear(year: u16, zone: *const DynamicTimeZoneInformation, info: *mut TimeZoneInformation) -> i32;
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GetProcAddress(module: Handle, name: *const u8) -> *mut c_void;
    fn GetTempPathW(len: u32, buf: *mut u16) -> u32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(owner: Handle, text: *const u16, caption: *const u16, kind: u32) -> i32;
}

const DIALOG_KIND: u32 = 0x0000_0010 | 0x0001_0000;

pub(super) fn ask_path(kind: super::Pick) -> Result<super::Wait, crate::fault::Fault> {
    Ok(Box::new(move || {
        let d = rfd::FileDialog::new();
        let got = match kind {
            super::Pick::Folder => d.pick_folder(),
            super::Pick::File | super::Pick::FileOrFolder => d.pick_file(),
        };
        Ok(got.map(|p| p.display().to_string()))
    }))
}

pub(super) fn say_without_window(line: &str, sentence: &str) {
    eprintln!("{line}");
    if !std::io::stderr().as_raw_handle().is_null() {
        return;
    }
    let wide = |s: &str| -> Vec<u16> { std::ffi::OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect() };
    let (text, caption) = (wide(sentence), wide("ZIKARON"));
    unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), DIALOG_KIND) };
}

pub(super) fn dialog_brings_folders() -> bool {
    false
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

const RULE_YEARS: std::ops::RangeInclusive<u16> = 1970..=2100;

pub(super) fn zone_rules() -> Option<super::Zone> {
    if let Ok(tz) = std::env::var("TZ") {
        let tz = tz.trim().trim_start_matches(':');
        if !tz.is_empty() {
            return Some(super::Zone::Rule(tz.to_string()));
        }
    }
    let mut dynamic = DynamicTimeZoneInformation { zone: TimeZoneInformation::zero(), key_name: [0; 128], dynamic_daylight_time_disabled: 0 };
    if unsafe { GetDynamicTimeZoneInformation(&mut dynamic) } == TIME_ZONE_ID_INVALID {
        return None;
    }
    if dynamic.dynamic_daylight_time_disabled != 0 {
        return today_rule();
    }
    let mut years: Vec<(i64, String)> = Vec::new();
    for year in RULE_YEARS {
        let mut info = TimeZoneInformation::zero();
        if unsafe { GetTimeZoneInformationForYear(year, &dynamic, &mut info) } == 0 {
            return today_rule();
        }
        let Some(rule) = posix_rule(&info) else { return today_rule() };
        if years.last().map(|(_, r)| r) != Some(&rule) {
            years.push((i64::from(year), rule));
        }
    }
    Some(super::Zone::ByYear(years))
}

fn today_rule() -> Option<super::Zone> {
    let mut info = TimeZoneInformation::zero();
    if unsafe { GetTimeZoneInformation(&mut info) } == TIME_ZONE_ID_INVALID {
        return None;
    }
    posix_rule(&info).map(super::Zone::Rule)
}

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

    #[test]
    fn the_system_zone_becomes_a_posix_rule() {
        assert_eq!(posix_rule(&zone(-60, (10, 5, 0, 3), Some(((3, 5, 0, 2), -60)))).as_deref(), Some("<STD>-1<DST>-2,M3.5.0/2:00:00,M10.5.0/3:00:00"));
        assert_eq!(posix_rule(&zone(300, (11, 1, 0, 2), Some(((3, 2, 0, 2), -60)))).as_deref(), Some("<STD>5<DST>4,M3.2.0/2:00:00,M11.1.0/2:00:00"));
        assert_eq!(posix_rule(&zone(-330, (0, 0, 0, 0), None)).as_deref(), Some("<STD>-5:30"));
        assert_eq!(posix_rule(&zone(-480, (0, 0, 0, 0), None)).as_deref(), Some("<STD>-8"));
        let mut once = zone(-60, (10, 5, 0, 3), Some(((3, 5, 0, 2), -60)));
        once.daylight_date.year = 2026;
        assert_eq!(posix_rule(&once), None);
    }
}

pub(super) fn menu_bar(_words: &super::menubar::Words) {}

pub(super) fn menu_takes_key(_chars: &str) -> bool {
    false
}

pub(super) fn menu_bar_read() -> Vec<(String, Vec<(String, String, bool)>)> {
    Vec::new()
}
