//! The Windows implementation. No third-party crate: the Win32 functions it needs are declared here.
//!
//! - Entropy: `BCryptGenRandom` with the system-preferred RNG.
//! - Owner-only: the creating call (`CreateFileW`) passes a security descriptor with a protected DACL (nothing
//!   inherited) allowing only this process's user; a file reads as owner-only when every allow entry of its
//!   DACL names this user.
//! - Syncing: a directory is opened with backup semantics and flushed; where the OS cannot flush a directory,
//!   the file just named in it is flushed instead (NTFS journals in order, so the name reaches disk with it).
//!   Which works depends on the volume; both are tried, in that order.
//! - Replacing rename: `MoveFileExW` replacing an existing target, with write-through. Another handle that
//!   holds either file for a moment (a reader of the target, the system's scanner on a file just written) makes
//!   the move fail at once rather than wait; the move is tried again for a short while ([`MOVE_HELD_WAIT`]).

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::FromRawHandle;
use std::path::Path;

type Handle = *mut c_void;

const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
const GENERIC_READ: u32 = 0x8000_0000;
const GENERIC_WRITE: u32 = 0x4000_0000;
const FILE_SHARE_READ: u32 = 0x1;
const FILE_SHARE_WRITE: u32 = 0x2;
const FILE_SHARE_DELETE: u32 = 0x4;
const CREATE_NEW: u32 = 1;
const CREATE_ALWAYS: u32 = 2;
const OPEN_EXISTING: u32 = 3;
const OPEN_ALWAYS: u32 = 4;
const TRUNCATE_EXISTING: u32 = 5;
const FILE_ATTRIBUTE_NORMAL: u32 = 0x80;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x2;
const TOKEN_QUERY: u32 = 0x8;
/// `TOKEN_INFORMATION_CLASS::TokenUser`.
const TOKEN_USER: u32 = 1;
const SDDL_REVISION_1: u32 = 1;
/// `SE_OBJECT_TYPE::SE_FILE_OBJECT`.
const SE_FILE_OBJECT: u32 = 1;
const DACL_SECURITY_INFORMATION: u32 = 0x4;
const ACCESS_ALLOWED_ACE_TYPE: u8 = 0;
const ACCESS_DENIED_ACE_TYPE: u8 = 1;

#[repr(C)]
struct SecurityAttributes {
    length: u32,
    descriptor: *mut c_void,
    inherit: i32,
}

/// The fixed header of an access control list (`ACL`).
#[repr(C)]
struct Acl {
    revision: u8,
    sbz1: u8,
    size: u16,
    ace_count: u16,
    sbz2: u16,
}

#[link(name = "bcrypt")]
unsafe extern "system" {
    fn BCryptGenRandom(algorithm: Handle, buffer: *mut u8, count: u32, flags: u32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateFileW(name: *const u16, access: u32, share: u32, security: *mut SecurityAttributes, disposition: u32, flags: u32, template: Handle) -> Handle;
    fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    fn GetCurrentProcess() -> Handle;
    fn CloseHandle(h: Handle) -> i32;
    fn LocalFree(p: *mut c_void) -> *mut c_void;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateNamedPipeW(name: *const u16, open: u32, mode: u32, instances: u32, out: u32, inb: u32, timeout: u32, security: *mut SecurityAttributes) -> Handle;
    fn ConnectNamedPipe(pipe: Handle, overlapped: *mut c_void) -> i32;
    fn DisconnectNamedPipe(pipe: Handle) -> i32;
    fn FlushFileBuffers(h: Handle) -> i32;
    fn WaitNamedPipeW(name: *const u16, timeout: u32) -> i32;
    fn GetNamedPipeClientProcessId(pipe: Handle, pid: *mut u32) -> i32;
    fn GetNamedPipeServerProcessId(pipe: Handle, pid: *mut u32) -> i32;
    fn PeekNamedPipe(pipe: Handle, buf: *mut c_void, size: u32, read: *mut u32, avail: *mut u32, left: *mut u32) -> i32;
    fn ReadFile(h: Handle, buf: *mut u8, len: u32, read: *mut u32, overlapped: *mut c_void) -> i32;
    fn WriteFile(h: Handle, buf: *const u8, len: u32, written: *mut u32, overlapped: *mut c_void) -> i32;
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
}

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

#[link(name = "shell32")]
unsafe extern "system" {
    fn SHGetKnownFolderPath(id: *const Guid, flags: u32, token: Handle, path: *mut *mut u16) -> i32;
}

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoTaskMemFree(p: *mut c_void);
}

#[link(name = "advapi32")]
unsafe extern "system" {
    fn OpenProcessToken(process: Handle, access: u32, token: *mut Handle) -> i32;
    fn GetTokenInformation(token: Handle, class: u32, info: *mut c_void, len: u32, returned: *mut u32) -> i32;
    fn ConvertSidToStringSidW(sid: *mut c_void, out: *mut *mut u16) -> i32;
    fn ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl: *const u16, revision: u32, sd: *mut *mut c_void, size: *mut u32) -> i32;
    fn GetNamedSecurityInfoW(
        name: *const u16,
        object: u32,
        info: u32,
        owner: *mut *mut c_void,
        group: *mut *mut c_void,
        dacl: *mut *mut Acl,
        sacl: *mut *mut c_void,
        sd: *mut *mut c_void,
    ) -> u32;
    fn GetAce(acl: *const Acl, index: u32, ace: *mut *mut c_void) -> i32;
    fn EqualSid(a: *mut c_void, b: *mut c_void) -> i32;
}

pub const ENTROPY_SOURCE: &str = "BCryptGenRandom";

pub fn fill_random(buf: &mut [u8]) -> std::io::Result<()> {
    for chunk in buf.chunks_mut(u32::MAX as usize) {
        let status = unsafe { BCryptGenRandom(std::ptr::null_mut(), chunk.as_mut_ptr(), chunk.len() as u32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) };
        if status != 0 {
            return Err(std::io::Error::other(format!("{ENTROPY_SOURCE}: status {:#010x}", status as u32)));
        }
    }
    Ok(())
}

/// A NUL-terminated UTF-16 path for the wide Win32 calls. An absolute path long enough to hit the legacy
/// `MAX_PATH` limit uses the extended form (`\\?\`, or `\\?\UNC\` for a share), as the standard library does.
fn wide(path: &Path) -> Vec<u16> {
    let raw: Vec<u16> = path.as_os_str().encode_wide().collect();
    let slash = u16::from(b'/');
    let back = u16::from(b'\\');
    let starts = |p: &[u16], s: &str| p.len() >= s.len() && p.iter().zip(s.encode_utf16()).all(|(a, b)| *a == b);
    let mut out: Vec<u16> = if raw.len() < 248 || starts(&raw, r"\\?\") || !path.is_absolute() {
        raw
    } else {
        let flat: Vec<u16> = raw.iter().map(|c| if *c == slash { back } else { *c }).collect();
        if starts(&flat, r"\\") {
            r"\\?\UNC\".encode_utf16().chain(flat[2..].iter().copied()).collect()
        } else {
            r"\\?\".encode_utf16().chain(flat).collect()
        }
    };
    out.push(0);
    out
}

/// Memory allocated by the OS with `LocalAlloc`, freed on drop.
struct Local(*mut c_void);

impl Drop for Local {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { LocalFree(self.0) };
        }
    }
}

/// This process's user from its token: the buffer holding `TOKEN_USER`, whose first member points at the user's
/// SID inside the same buffer.
struct User(Vec<u64>);

impl User {
    fn now() -> std::io::Result<User> {
        User::of(unsafe { GetCurrentProcess() })
    }

    /// The user another process runs as (by the process id a pipe reports for its peer).
    fn of_pid(pid: u32) -> std::io::Result<User> {
        const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
        let p = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if p.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let u = User::of(p);
        unsafe { CloseHandle(p) };
        u
    }

    fn of(process: Handle) -> std::io::Result<User> {
        let mut token: Handle = std::ptr::null_mut();
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut len = 0u32;
        unsafe { GetTokenInformation(token, TOKEN_USER, std::ptr::null_mut(), 0, &mut len) };
        let mut buf = vec![0u64; (len as usize).div_ceil(8).max(1)];
        let ok = unsafe { GetTokenInformation(token, TOKEN_USER, buf.as_mut_ptr().cast(), (buf.len() * 8) as u32, &mut len) };
        let err = std::io::Error::last_os_error();
        unsafe { CloseHandle(token) };
        if ok == 0 {
            return Err(err);
        }
        Ok(User(buf))
    }

    fn sid(&self) -> *mut c_void {
        unsafe { *(self.0.as_ptr() as *const *mut c_void) }
    }

    /// A security descriptor for an owner-only file: a protected DACL (nothing inherited from the folder) with
    /// one entry granting this user all file access.
    fn owner_only_descriptor(&self) -> std::io::Result<Local> {
        let mut text: *mut u16 = std::ptr::null_mut();
        if unsafe { ConvertSidToStringSidW(self.sid(), &mut text) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let text = Local(text.cast());
        let sid: Vec<u16> = unsafe {
            let p = text.0 as *const u16;
            let n = (0..).take_while(|i| *p.add(*i) != 0).count();
            std::slice::from_raw_parts(p, n).to_vec()
        };
        let sddl: Vec<u16> = "D:P(A;;FA;;;".encode_utf16().chain(sid).chain(")".encode_utf16()).chain(std::iter::once(0)).collect();
        let mut sd: *mut c_void = std::ptr::null_mut();
        if unsafe { ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.as_ptr(), SDDL_REVISION_1, &mut sd, std::ptr::null_mut()) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Local(sd))
    }
}

pub fn open(o: &crate::Options, path: &Path) -> std::io::Result<std::fs::File> {
    if !o.owner_only {
        let mut s = std::fs::OpenOptions::new();
        s.read(o.read).write(o.write).create(o.create).create_new(o.create_new).truncate(o.truncate);
        return s.open(path);
    }
    // As in the standard library: create or truncate requires write access.
    if (o.create || o.create_new || o.truncate) && !o.write {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
    }
    let access = (if o.read { GENERIC_READ } else { 0 }) | (if o.write { GENERIC_WRITE } else { 0 });
    let disposition = match (o.create_new, o.create, o.truncate) {
        (true, _, _) => CREATE_NEW,
        (false, true, true) => CREATE_ALWAYS,
        (false, true, false) => OPEN_ALWAYS,
        (false, false, true) => TRUNCATE_EXISTING,
        (false, false, false) => OPEN_EXISTING,
    };
    let sd = User::now()?.owner_only_descriptor()?;
    let mut sa = SecurityAttributes { length: std::mem::size_of::<SecurityAttributes>() as u32, descriptor: sd.0, inherit: 0 };
    let name = wide(path);
    let share = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
    let h = unsafe { CreateFileW(name.as_ptr(), access, share, &mut sa, disposition, FILE_ATTRIBUTE_NORMAL, std::ptr::null_mut()) };
    if h == INVALID_HANDLE_VALUE {
        return Err(std::io::Error::last_os_error());
    }
    Ok(unsafe { std::fs::File::from_raw_handle(h) })
}

/// Widening a DACL is not supported here (tests that need it run on unix).
pub fn open_to_others(_path: &Path, _group_only: bool) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "opening a file to other accounts is not done on this system"))
}

/// Owner-only when the DACL allows nobody but this user: every allow entry names this user (deny entries do
/// not change that). A null DACL grants everyone access, so it is not owner-only; an entry of an unrecognized
/// type counts as granting someone else.
pub fn is_owner_only(path: &Path) -> std::io::Result<bool> {
    let name = wide(path);
    let mut dacl: *mut Acl = std::ptr::null_mut();
    let mut sd: *mut c_void = std::ptr::null_mut();
    let r = unsafe {
        GetNamedSecurityInfoW(
            name.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut dacl,
            std::ptr::null_mut(),
            &mut sd,
        )
    };
    if r != 0 {
        return Err(std::io::Error::from_raw_os_error(r as i32));
    }
    let _sd = Local(sd);
    if dacl.is_null() {
        return Ok(false);
    }
    let user = User::now()?;
    let count = unsafe { (*dacl).ace_count };
    for i in 0..u32::from(count) {
        let mut ace: *mut c_void = std::ptr::null_mut();
        if unsafe { GetAce(dacl, i, &mut ace) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        // `ACE_HEADER` (type, flags, size), then the access mask, then the identifier.
        let kind = unsafe { *(ace as *const u8) };
        match kind {
            ACCESS_DENIED_ACE_TYPE => continue,
            ACCESS_ALLOWED_ACE_TYPE => {
                let sid = unsafe { (ace as *mut u8).add(8) }.cast::<c_void>();
                if unsafe { EqualSid(sid, user.sid()) } == 0 {
                    return Ok(false);
                }
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// Flush the directory itself through a handle opened with backup semantics (the only way to open a
/// directory); if that fails, flush the file just named in it. With no such file, the directory's error is
/// returned.
pub fn sync_dir(path: &Path, landed: &Path) -> std::io::Result<()> {
    let dir = std::fs::OpenOptions::new().write(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS).open(path).and_then(|d| d.sync_all());
    match dir {
        Ok(()) => Ok(()),
        Err(e) if !landed.is_file() => Err(e),
        // If both fail, report both: the directory's error, then the file's (with the file's kind).
        Err(d) => sync_file(landed).map_err(|f| std::io::Error::new(f.kind(), format!("the directory: {d}; then the file {}: {f}", landed.display()))),
    }
}

/// Flushing needs a handle with write access.
pub fn sync_file(path: &Path) -> std::io::Result<()> {
    std::fs::OpenOptions::new().write(true).open(path)?.sync_all()
}

pub fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    move_file(from, to, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)
}

/// Without `MOVEFILE_REPLACE_EXISTING` the move refuses an existing target (`AlreadyExists`).
pub fn rename_new(from: &Path, to: &Path) -> std::io::Result<()> {
    move_file(from, to, MOVEFILE_WRITE_THROUGH)
}

/// How long a move keeps retrying while another handle briefly holds one of its files without sharing
/// deletion (a reader of the target, or a virus scanner on a fresh file). Windows then fails the move at once
/// with `ERROR_SHARING_VIOLATION` or `ERROR_ACCESS_DENIED`. Past this the OS error is returned unchanged.
pub const MOVE_HELD_WAIT: std::time::Duration = std::time::Duration::from_secs(1);

/// The pause before the first try again; each pause doubles, up to [`MOVE_HELD_PAUSE_MAX`].
const MOVE_HELD_PAUSE: std::time::Duration = std::time::Duration::from_millis(10);
const MOVE_HELD_PAUSE_MAX: std::time::Duration = std::time::Duration::from_millis(100);

/// `MoveFileExW` with `flags`, retried within [`MOVE_HELD_WAIT`] while it fails as a held file does; any other
/// failure is returned at once. A lasting refusal with the same codes (a read-only target, no folder rights)
/// costs the wait, then is returned unchanged.
fn move_file(from: &Path, to: &Path, flags: u32) -> std::io::Result<()> {
    let (f, t) = (wide(from), wide(to));
    let began = std::time::Instant::now();
    let mut pause = MOVE_HELD_PAUSE;
    loop {
        if unsafe { MoveFileExW(f.as_ptr(), t.as_ptr(), flags) } != 0 {
            return Ok(());
        }
        let e = std::io::Error::last_os_error();
        let held = matches!(e.raw_os_error(), Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION));
        if !held || began.elapsed() + pause > MOVE_HELD_WAIT {
            return Err(e);
        }
        std::thread::sleep(pause);
        pause = (pause * 2).min(MOVE_HELD_PAUSE_MAX);
    }
}

fn known_folder(id: &Guid) -> Option<std::path::PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    let mut p: *mut u16 = std::ptr::null_mut();
    let r = unsafe { SHGetKnownFolderPath(id, 0, std::ptr::null_mut(), &mut p) };
    // The caller must free the buffer whether or not the call succeeded.
    let out = if r == 0 && !p.is_null() {
        let wide = unsafe {
            let n = (0..).take_while(|i| *p.add(*i) != 0).count();
            std::slice::from_raw_parts(p, n).to_vec()
        };
        Some(std::path::PathBuf::from(std::ffi::OsString::from_wide(&wide))).filter(|x| !x.as_os_str().is_empty())
    } else {
        None
    };
    unsafe { CoTaskMemFree(p.cast()) };
    out
}

/// This app's folder in the local application data folder.
const APP_FOLDER: &str = "ZIKARON";

/// The variable that names the user's home (set by Windows from the profile at logon).
pub const HOME_VAR: &str = "USERPROFILE";

/// The user's home beyond [`HOME_VAR`]: the profile known folder.
pub fn home_dir() -> Option<std::path::PathBuf> {
    known_folder(&FOLDER_PROFILE)
}

/// This app's folder in `%LOCALAPPDATA%` (which does not roam) when `user_home` is the profile known folder,
/// however it is spelled ([`crate::same_folder`]). Any other home (e.g. a test stand-in) gets the same layout
/// under itself, so tests never touch the user's real folder.
pub fn app_data_dir(user_home: &Path) -> std::path::PathBuf {
    if home_dir().is_some_and(|profile| crate::same_folder(&profile, user_home)) {
        if let Some(local) = known_folder(&FOLDER_LOCAL_APP_DATA) {
            return local.join(APP_FOLDER);
        }
    }
    user_home.join("AppData").join("Local").join(APP_FOLDER)
}

// ───────────────────────── IPC endpoint: a named pipe ─────────────────────────

/// Pipe names carry the token, not the machine directory path, so no realistic path can exceed this limit.
pub const DOOR_PATH_MAX: usize = 256;

/// On Windows the endpoint is a named pipe named after `name`, not the machine directory's path.
pub fn door_place(_machine: &Path, name: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(format!(r"\\.\pipe\{name}"))
}

const ERROR_FILE_NOT_FOUND: i32 = 2;
const ERROR_ACCESS_DENIED: i32 = 5;
const ERROR_SHARING_VIOLATION: i32 = 32;
const ERROR_BROKEN_PIPE: i32 = 109;
const ERROR_PIPE_BUSY: i32 = 231;
const ERROR_NO_DATA: i32 = 232;
const ERROR_PIPE_CONNECTED: i32 = 535;
const PIPE_ACCESS_DUPLEX: u32 = 0x3;
const FILE_FLAG_FIRST_PIPE_INSTANCE: u32 = 0x0008_0000;
const PIPE_REJECT_REMOTE_CLIENTS: u32 = 0x8;
const PIPE_UNLIMITED_INSTANCES: u32 = 255;
const SECURITY_SQOS_PRESENT: u32 = 0x0010_0000;
const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;
const ERROR_SEM_TIMEOUT: i32 = 121;
/// How long one wait for a busy pipe lasts before the pipe is tried again.
const BUSY_WAIT_MS: u32 = 250;

/// One pipe end, closed on drop (a server end disconnects its client first).
pub struct DoorStream {
    h: Handle,
    server: bool,
    deadline: std::cell::Cell<Option<std::time::Duration>>,
}

unsafe impl Send for DoorStream {}

impl Drop for DoorStream {
    fn drop(&mut self) {
        unsafe {
            if self.server {
                FlushFileBuffers(self.h);
                DisconnectNamedPipe(self.h);
            }
            CloseHandle(self.h);
        }
    }
}

/// The listening side: the pipe instance the next client connects to.
///
/// A pipe has no path to remove: it exists while an instance does. Closing ([`door_close`]) closes the waiting
/// instance and marks it gone (`INVALID_HANDLE_VALUE`), and a woken accept creates no new one, so a connect then
/// gets `NotFound`, as on unix.
pub struct DoorListener {
    name: Vec<u16>,
    /// The instance the next client connects to, or `INVALID_HANDLE_VALUE` once closed. Held across
    /// `ConnectNamedPipe`, so a close waits for an accept in progress to finish its round.
    next: std::sync::Mutex<Handle>,
    /// Set by [`door_close`] before it wakes the waiting accept.
    closing: std::sync::atomic::AtomicBool,
}

unsafe impl Send for DoorListener {}
unsafe impl Sync for DoorListener {}

fn pipe_name(place: &Path) -> Vec<u16> {
    place.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

/// One pipe instance, reachable only by this user and not remotely; with `first`, fails if the pipe already
/// exists (someone else's, or a live endpoint: `AddrInUse`).
fn instance(name: &[u16], first: bool) -> std::io::Result<Handle> {
    let sd = User::now()?.owner_only_descriptor()?;
    let mut sa = SecurityAttributes { length: std::mem::size_of::<SecurityAttributes>() as u32, descriptor: sd.0, inherit: 0 };
    let open = PIPE_ACCESS_DUPLEX | if first { FILE_FLAG_FIRST_PIPE_INSTANCE } else { 0 };
    let h = unsafe { CreateNamedPipeW(name.as_ptr(), open, PIPE_REJECT_REMOTE_CLIENTS, PIPE_UNLIMITED_INSTANCES, 65536, 65536, 0, &mut sa) };
    if h == INVALID_HANDLE_VALUE {
        let e = std::io::Error::last_os_error();
        if e.raw_os_error() == Some(ERROR_ACCESS_DENIED) {
            return Err(std::io::Error::new(std::io::ErrorKind::AddrInUse, "a door is open at this place"));
        }
        return Err(e);
    }
    Ok(h)
}

/// Check that the peer runs as this process's user; anyone else is refused (`PermissionDenied`).
fn same_user(pid: std::io::Result<u32>) -> std::io::Result<()> {
    let them = User::of_pid(pid?)?;
    let me = User::now()?;
    if unsafe { EqualSid(them.sid(), me.sid()) } == 0 {
        return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, "the other end is another user"));
    }
    Ok(())
}

pub fn door_listen(place: &Path) -> std::io::Result<DoorListener> {
    let name = pipe_name(place);
    let first = instance(&name, true)?;
    Ok(DoorListener { name, next: std::sync::Mutex::new(first), closing: std::sync::atomic::AtomicBool::new(false) })
}

fn door_gone() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, "the door is closed")
}

impl DoorListener {
    /// The next client; its user is not yet checked ([`DoorStream::same_user`]). A client that already hung up
    /// (`ERROR_NO_DATA`) is returned like any other, with a stream that reads as ended.
    pub fn accept(&self) -> std::io::Result<DoorStream> {
        let mut next = self.next.lock().unwrap_or_else(|e| e.into_inner());
        let h = *next;
        if h == INVALID_HANDLE_VALUE {
            return Err(door_gone());
        }
        let closing = || self.closing.load(std::sync::atomic::Ordering::SeqCst);
        if !closing() && unsafe { ConnectNamedPipe(h, std::ptr::null_mut()) } == 0 {
            let e = std::io::Error::last_os_error();
            if !matches!(e.raw_os_error(), Some(ERROR_PIPE_CONNECTED) | Some(ERROR_NO_DATA)) {
                return Err(e);
            }
        }
        // Woken by the close (or a client came at that moment): no next instance, and this one is closed now.
        if closing() {
            unsafe { CloseHandle(h) };
            *next = INVALID_HANDLE_VALUE;
            return Err(door_gone());
        }
        // The next client connects to a fresh instance; this one now belongs to the stream.
        *next = instance(&self.name, false)?;
        Ok(DoorStream { h, server: true, deadline: std::cell::Cell::new(None) })
    }
}

impl Drop for DoorListener {
    fn drop(&mut self) {
        let h = *self.next.lock().unwrap_or_else(|e| e.into_inner());
        if h != INVALID_HANDLE_VALUE {
            unsafe { CloseHandle(h) };
        }
    }
}

fn open_pipe(name: &[u16]) -> std::io::Result<Handle> {
    loop {
        let h = unsafe {
            CreateFileW(name.as_ptr(), GENERIC_READ | GENERIC_WRITE, 0, std::ptr::null_mut(), OPEN_EXISTING, SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION, std::ptr::null_mut())
        };
        if h != INVALID_HANDLE_VALUE {
            return Ok(h);
        }
        let e = std::io::Error::last_os_error();
        match e.raw_os_error() {
            Some(ERROR_FILE_NOT_FOUND) => return Err(std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string())),
            Some(ERROR_ACCESS_DENIED) => return Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, e.to_string())),
            // All instances busy: wait in short slices and retry, so an endpoint closed meanwhile is `NotFound`.
            Some(ERROR_PIPE_BUSY) => {
                if unsafe { WaitNamedPipeW(name.as_ptr(), BUSY_WAIT_MS) } == 0 {
                    let e = std::io::Error::last_os_error();
                    if !matches!(e.raw_os_error(), Some(ERROR_SEM_TIMEOUT) | Some(ERROR_FILE_NOT_FOUND)) {
                        return Err(e);
                    }
                }
            }
            _ => return Err(e),
        }
    }
}

/// Close the endpoint (see [`DoorListener`]): mark it closing, wake a waiting accept by connecting once (without
/// waiting on a busy pipe, which has no waiter), then close the instance under the accept's lock. A dropped
/// listener has already closed its instance.
pub fn door_close(_place: &Path, listener: Option<&DoorListener>) {
    let Some(l) = listener else { return };
    l.closing.store(true, std::sync::atomic::Ordering::SeqCst);
    let h = unsafe {
        CreateFileW(l.name.as_ptr(), GENERIC_READ | GENERIC_WRITE, 0, std::ptr::null_mut(), OPEN_EXISTING, SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION, std::ptr::null_mut())
    };
    if h != INVALID_HANDLE_VALUE {
        unsafe { CloseHandle(h) };
    }
    let mut next = l.next.lock().unwrap_or_else(|e| e.into_inner());
    if *next != INVALID_HANDLE_VALUE {
        unsafe { CloseHandle(*next) };
        *next = INVALID_HANDLE_VALUE;
    }
}

/// Connect to the endpoint at `place` without checking its user ([`DoorStream::same_user`]).
pub fn door_connect(place: &Path) -> std::io::Result<DoorStream> {
    let h = open_pipe(&pipe_name(place))?;
    Ok(DoorStream { h, server: false, deadline: std::cell::Cell::new(None) })
}

impl DoorStream {
    /// Whether the other end runs as this process's user (the server asks for the client's process, the client
    /// for the server's); anyone else is refused (`PermissionDenied`).
    pub fn same_user(&self) -> std::io::Result<()> {
        let mut pid = 0u32;
        let asked = unsafe { if self.server { GetNamedPipeClientProcessId(self.h, &mut pid) } else { GetNamedPipeServerProcessId(self.h, &mut pid) } };
        same_user(if asked == 0 { Err(std::io::Error::last_os_error()) } else { Ok(pid) })
    }

    pub fn set_read_deadline(&self, d: Option<std::time::Duration>) -> std::io::Result<()> {
        self.deadline.set(d);
        Ok(())
    }
}

impl std::io::Read for DoorStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // A synchronous pipe read blocks forever; with a deadline, wait until bytes are available first.
        if let Some(d) = self.deadline.get() {
            let until = std::time::Instant::now() + d;
            loop {
                let mut avail = 0u32;
                if unsafe { PeekNamedPipe(self.h, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut avail, std::ptr::null_mut()) } == 0 {
                    let e = std::io::Error::last_os_error();
                    if e.raw_os_error() == Some(ERROR_BROKEN_PIPE) {
                        return Ok(0);
                    }
                    return Err(e);
                }
                if avail > 0 {
                    break;
                }
                if std::time::Instant::now() >= until {
                    return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "nothing came within the deadline"));
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        let mut got = 0u32;
        let len = buf.len().min(u32::MAX as usize) as u32;
        if unsafe { ReadFile(self.h, buf.as_mut_ptr(), len, &mut got, std::ptr::null_mut()) } == 0 {
            let e = std::io::Error::last_os_error();
            if e.raw_os_error() == Some(ERROR_BROKEN_PIPE) {
                return Ok(0);
            }
            return Err(e);
        }
        Ok(got as usize)
    }
}

impl std::io::Write for DoorStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut put = 0u32;
        let len = buf.len().min(u32::MAX as usize) as u32;
        if unsafe { WriteFile(self.h, buf.as_ptr(), len, &mut put, std::ptr::null_mut()) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(put as usize)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// ───────────────────────── Children and lock descriptors ─────────────────────────

/// A lock file's handle, as listed (`crate::LISTED`).
pub fn listed_id(file: &std::fs::File) -> i64 {
    use std::os::windows::io::AsRawHandle;
    file.as_raw_handle() as isize as i64
}

/// Start the child. No child holds a copy of a lock: `open` here creates handles with `bInheritHandle` false (the
/// standard library's files are not inheritable either), and `CreateProcess` passes only inheritable handles; the
/// listed handles need nothing done.
pub fn spawn(cmd: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    cmd.spawn()
}

// ───────────────────────── "Enable command line": the user's `Path` ─────────────────────────

#[link(name = "advapi32")]
unsafe extern "system" {
    fn RegOpenKeyExW(key: Handle, sub: *const u16, options: u32, access: u32, out: *mut Handle) -> i32;
    fn RegQueryValueExW(key: Handle, name: *const u16, reserved: *mut u32, kind: *mut u32, data: *mut u8, len: *mut u32) -> i32;
    fn RegSetValueExW(key: Handle, name: *const u16, reserved: u32, kind: u32, data: *const u8, len: u32) -> i32;
    fn RegCloseKey(key: Handle) -> i32;
}

#[link(name = "user32")]
unsafe extern "system" {
    fn SendMessageTimeoutW(window: Handle, msg: u32, wparam: usize, lparam: isize, flags: u32, timeout: u32, result: *mut usize) -> isize;
}

const HKEY_CURRENT_USER: Handle = 0x8000_0001usize as Handle;
const HKEY_LOCAL_MACHINE: Handle = 0x8000_0002usize as Handle;
const KEY_READ: u32 = 0x2_0019;
const KEY_WRITE: u32 = 0x2_0006;
const REG_EXPAND_SZ: u32 = 2;
const ERROR_FILE_NOT_FOUND_REG: i32 = 2;
/// Maximum length of an environment variable value, in characters.
const PATH_MAX_CHARS: usize = 32_767;

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// On Windows the switch edits the user's `Path` rather than placing a link in a folder.
pub fn cli_default_dir() -> Option<std::path::PathBuf> {
    None
}

pub fn cli_unsupported(_cli: &Path) -> Option<String> {
    None
}

/// The CLI's folder is on the machine-wide `Path` (put there by an installer). Only the machine `Path` is read:
/// the process `PATH` also inherits the switch's own user-`Path` entry, which would then read as "provided" and
/// could never be turned off.
pub fn cli_provided(cli: &Path, moved: bool) -> bool {
    let Some(dir) = cli.parent() else { return false };
    machine_path(moved).is_ok_and(|p| p.split(';').any(|e| same_dir(e, dir)))
}

/// The user's `Path` as stored (the stand-in file when redirected): where the switch writes.
fn user_path(moved: bool) -> std::io::Result<String> {
    if moved {
        return stand_in(crate::cli_path::PATH_STAND_IN);
    }
    reg_path(HKEY_CURRENT_USER, "Environment")
}

/// The machine-wide `Path` as stored (the stand-in file when redirected): written by installers, never by the
/// switch.
fn machine_path(moved: bool) -> std::io::Result<String> {
    if moved {
        return stand_in(crate::cli_path::MACHINE_PATH_STAND_IN);
    }
    reg_path(HKEY_LOCAL_MACHINE, r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment")
}

/// A stand-in file under the redirected folder; a missing file reads as empty.
fn stand_in(name: &str) -> std::io::Result<String> {
    let f = crate::cli_path::place_dir().unwrap_or_default().join(name);
    match std::fs::read_to_string(&f) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    }
}

/// The `Path` value of one registry environment key (missing reads as empty).
fn reg_path(root: Handle, sub: &str) -> std::io::Result<String> {
    let mut key: Handle = std::ptr::null_mut();
    let r = unsafe { RegOpenKeyExW(root, w(sub).as_ptr(), 0, KEY_READ, &mut key) };
    if r != 0 {
        return Err(std::io::Error::from_raw_os_error(r));
    }
    let name = w("Path");
    let mut len = 0u32;
    let r = unsafe { RegQueryValueExW(key, name.as_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), &mut len) };
    if r == ERROR_FILE_NOT_FOUND_REG {
        unsafe { RegCloseKey(key) };
        return Ok(String::new());
    }
    let mut buf = vec![0u16; (len as usize).div_ceil(2) + 1];
    let mut got = (buf.len() * 2) as u32;
    let r = unsafe { RegQueryValueExW(key, name.as_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), buf.as_mut_ptr().cast(), &mut got) };
    unsafe { RegCloseKey(key) };
    if r != 0 {
        return Err(std::io::Error::from_raw_os_error(r));
    }
    let n = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    Ok(String::from_utf16_lossy(&buf[..n]))
}

/// Write the user's `Path` and broadcast the environment change (newly opened terminals pick it up).
fn set_user_path(text: &str, moved: bool) -> std::io::Result<()> {
    if moved {
        let d = crate::cli_path::place_dir().unwrap_or_default();
        std::fs::create_dir_all(&d)?;
        return std::fs::write(d.join(crate::cli_path::PATH_STAND_IN), text);
    }
    let mut key: Handle = std::ptr::null_mut();
    let r = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, w("Environment").as_ptr(), 0, KEY_WRITE, &mut key) };
    if r != 0 {
        return Err(std::io::Error::from_raw_os_error(r));
    }
    let data = w(text);
    let r = unsafe { RegSetValueExW(key, w("Path").as_ptr(), 0, REG_EXPAND_SZ, data.as_ptr().cast(), (data.len() * 2) as u32) };
    unsafe { RegCloseKey(key) };
    if r != 0 {
        return Err(std::io::Error::from_raw_os_error(r));
    }
    const HWND_BROADCAST: Handle = 0xffff as Handle;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SMTO_ABORTIFHUNG: u32 = 0x2;
    let env = w("Environment");
    let mut out = 0usize;
    unsafe { SendMessageTimeoutW(HWND_BROADCAST, WM_SETTINGCHANGE, 0, env.as_ptr() as isize, SMTO_ABORTIFHUNG, 1000, &mut out) };
    Ok(())
}

/// Whether a `Path` entry is the same folder, ignoring case and a trailing separator.
fn same_dir(entry: &str, dir: &Path) -> bool {
    let norm = |s: &str| s.trim().trim_end_matches(['\\', '/']).to_lowercase();
    norm(entry) == norm(&dir.display().to_string())
}

pub fn cli_state(cli: &Path, moved: bool) -> crate::cli_path::State {
    use crate::cli_path::State;
    let Some(dir) = cli.parent() else { return State::Unsupported("no folder".into()) };
    match user_path(moved) {
        Ok(p) if p.split(';').any(|e| same_dir(e, dir)) => State::On,
        // Another install's CLI earlier on the user's path would shadow the same name.
        Ok(p) => match p.split(';').filter(|e| !e.trim().is_empty()).find(|e| Path::new(e.trim()).join(format!("{}.exe", crate::cli_path::NAME)).is_file()) {
            Some(other) => State::Taken(other.trim().to_string()),
            None => State::Off,
        },
        Err(e) => State::Taken(format!("Path: {e}")),
    }
}

pub fn cli_enable(cli: &Path, moved: bool) -> Result<(), crate::cli_path::Refused> {
    use crate::cli_path::Refused;
    let dir = cli.parent().ok_or_else(|| Refused::Unsupported("no folder".into()))?;
    let now = user_path(moved).map_err(|e| Refused::NotAllowed(e.to_string()))?;
    let entry = dir.display().to_string();
    let next = if now.trim().is_empty() { entry } else { format!("{};{entry}", now.trim_end_matches(';')) };
    if next.encode_utf16().count() > PATH_MAX_CHARS {
        return Err(Refused::TooLong(next.encode_utf16().count()));
    }
    set_user_path(&next, moved).map_err(|e| Refused::NotAllowed(e.to_string()))
}

/// Remove only the entry this switch added (the CLI's folder); every other entry stays as written.
pub fn cli_disable(cli: &Path, moved: bool) -> Result<(), crate::cli_path::Refused> {
    use crate::cli_path::Refused;
    let dir = cli.parent().ok_or_else(|| Refused::Unsupported("no folder".into()))?;
    let now = user_path(moved).map_err(|e| Refused::NotAllowed(e.to_string()))?;
    let kept: Vec<&str> = now.split(';').filter(|e| !same_dir(e, dir)).collect();
    set_user_path(&kept.join(";"), moved).map_err(|e| Refused::NotAllowed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch folder with `from` holding "new" and, when `target`, `to` holding "old".
    fn move_scratch(tag: &str, target: bool) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let d = std::env::temp_dir().join(format!("zikaron-os-move-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("scratch");
        let (from, to) = (d.join("from"), d.join("to"));
        std::fs::write(&from, b"new").expect("from");
        if target {
            std::fs::write(&to, b"old").expect("to");
        }
        (d, from, to)
    }

    /// Open `at` sharing reading only (not deletion), and close it after `hold`; `None` keeps it until the
    /// returned handle is dropped.
    fn hold(at: &Path, hold: Option<std::time::Duration>) -> (Option<std::fs::File>, Option<std::thread::JoinHandle<()>>) {
        let f = std::fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(at).expect("held");
        match hold {
            Some(d) => (None, Some(std::thread::spawn(move || {
                std::thread::sleep(d);
                drop(f);
            }))),
            None => (Some(f), None),
        }
    }

    /// A move whose source or target is briefly held succeeds once released, for both move kinds.
    #[test]
    fn a_move_waits_out_a_file_held_for_a_moment() {
        let moment = Some(MOVE_HELD_WAIT / 5);
        let (d, from, to) = move_scratch("target-held", true);
        let (_, gone) = hold(&to, moment);
        replace(&from, &to).expect("made once the target is let go");
        let _ = gone.map(|h| h.join());
        assert_eq!(std::fs::read(&to).expect("to"), b"new");
        assert!(!from.exists());
        let _ = std::fs::remove_dir_all(&d);
        let (d, from, to) = move_scratch("source-held", true);
        let (_, gone) = hold(&from, moment);
        replace(&from, &to).expect("made once the source is let go");
        let _ = gone.map(|h| h.join());
        assert_eq!(std::fs::read(&to).expect("to"), b"new");
        let _ = std::fs::remove_dir_all(&d);
        let (d, from, to) = move_scratch("new-source-held", false);
        let (_, gone) = hold(&from, moment);
        rename_new(&from, &to).expect("a move that never replaces, once the source is let go");
        let _ = gone.map(|h| h.join());
        assert_eq!(std::fs::read(&to).expect("to"), b"new");
        let _ = std::fs::remove_dir_all(&d);
        let (d, from, to) = move_scratch("no-target", false);
        replace(&from, &to).expect("no target: made");
        assert_eq!(std::fs::read(&to).expect("to"), b"new");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Held past the wait, the move returns the OS error and leaves both files; a missing source fails at once;
    /// a non-replacing move still refuses an existing target.
    #[test]
    fn a_move_held_past_the_wait_is_refused_as_before() {
        let (d, from, to) = move_scratch("held-long", true);
        let (held, _) = hold(&to, None);
        let e = replace(&from, &to).expect_err("held throughout");
        assert!(matches!(e.raw_os_error(), Some(ERROR_ACCESS_DENIED | ERROR_SHARING_VIOLATION)), "{e:?}");
        drop(held);
        assert_eq!(std::fs::read(&to).expect("to"), b"old", "the target as it was");
        assert_eq!(std::fs::read(&from).expect("from"), b"new", "the source as it was");
        let missing = d.join("not-there");
        assert_eq!(replace(&missing, &to).expect_err("no source").raw_os_error(), Some(ERROR_FILE_NOT_FOUND));
        assert_eq!(rename_new(&from, &to).expect_err("a target there").kind(), std::io::ErrorKind::AlreadyExists);
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The app data folder of a stand-in home stays under that home.
    #[test]
    fn a_stand_in_home_keeps_the_app_folder_under_it() {
        let stand_in = Path::new(r"C:\stand-in\home");
        assert_eq!(app_data_dir(stand_in), stand_in.join("AppData").join("Local").join(APP_FOLDER));
    }

    /// The profile spelled differently (case, trailing separator) still maps to `%LOCALAPPDATA%`, where existing
    /// users' data is.
    #[test]
    fn the_profile_spelled_otherwise_is_still_the_profile() {
        let (Some(profile), Some(local)) = (home_dir(), known_folder(&FOLDER_LOCAL_APP_DATA)) else { return };
        let real = local.join(APP_FOLDER);
        let upper = std::path::PathBuf::from(profile.as_os_str().to_string_lossy().to_uppercase());
        let trailing = std::path::PathBuf::from(format!("{}\\", profile.display()));
        for spelled in [profile.clone(), upper, trailing] {
            assert_eq!(app_data_dir(&spelled), real, "{}", spelled.display());
        }
    }

    /// A file created in the temp folder without owner-only inherits the folder's DACL (SYSTEM and
    /// Administrators besides this user), so it is not owner-only; one created with it is.
    #[test]
    fn the_access_list_reads_back() {
        let d = std::env::temp_dir().join(format!("zikaron-os-acl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("scratch");
        let plain = d.join("plain");
        std::fs::write(&plain, b"x").expect("plain");
        assert!(!is_owner_only(&plain).expect("read"));
        let mine = d.join("mine");
        let mut o = crate::Options::new();
        o.write(true).create_new(true);
        crate::owner_only(&mut o).open(&mine).expect("created");
        assert!(is_owner_only(&mine).expect("read"));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The switch's own entry reads as on (and can be turned off) even when the process `PATH` carries it; an
    /// installer's machine `Path` entry reads as provided. Uses stand-in files, not the registry.
    #[test]
    fn the_switchs_own_entry_is_never_read_as_provided() {
        use crate::cli_path::{disable, enable, state, State, ENV_DIR, MACHINE_PATH_STAND_IN};
        let d = std::env::temp_dir().join(format!("zikaron-os-cli-own-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (place, app) = (d.join("place"), d.join("app"));
        std::fs::create_dir_all(&place).expect("place");
        std::fs::create_dir_all(&app).expect("app");
        let cli = app.join(format!("{}.exe", crate::cli_path::NAME));
        std::fs::write(&cli, b"MZ").expect("a command line");
        let path_before = std::env::var_os("PATH");
        unsafe { std::env::set_var(ENV_DIR, &place) };
        assert_eq!(enable(&cli), Ok(State::On));
        unsafe { std::env::set_var("PATH", std::env::join_paths([app.clone()]).expect("a path")) };
        assert_eq!(state(&cli), State::On, "its own entry on the process's path is still the switch's");
        assert_eq!(disable(&cli), Ok(State::Off), "and it turns off");
        std::fs::write(place.join(MACHINE_PATH_STAND_IN), app.display().to_string()).expect("an installer's entry");
        assert_eq!(state(&cli), State::Provided);
        match path_before {
            Some(p) => unsafe { std::env::set_var("PATH", p) },
            None => unsafe { std::env::remove_var("PATH") },
        }
        unsafe { std::env::remove_var(ENV_DIR) };
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A long absolute path takes the extended form; a short or relative one is left as given.
    #[test]
    fn long_paths_take_the_extended_form() {
        let text = |p: &Path| String::from_utf16(&wide(p)[..wide(p).len() - 1]).expect("utf-16");
        let long = format!(r"C:\{}", "a".repeat(300));
        assert!(text(Path::new(&long)).starts_with(r"\\?\C:\"));
        let share = format!(r"\\host\share\{}", "b".repeat(300));
        assert!(text(Path::new(&share)).starts_with(r"\\?\UNC\host\share\"));
        assert_eq!(text(Path::new(r"C:\short")), r"C:\short");
        assert_eq!(text(Path::new("relative")), "relative");
    }
}
