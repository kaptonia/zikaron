use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::fs::OpenOptionsExt;
use std::os::windows::io::FromRawHandle;
use std::path::Path;

pub(crate) type Handle = *mut c_void;

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
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const STATUS_OBJECT_NAME_NOT_FOUND: i32 = 0xC000_0034_u32 as i32;
const STATUS_OBJECT_PATH_NOT_FOUND: i32 = 0xC000_003A_u32 as i32;
const STATUS_DELETE_PENDING: i32 = 0xC000_0056_u32 as i32;
const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
const BCRYPT_USE_SYSTEM_PREFERRED_RNG: u32 = 0x2;
const TOKEN_QUERY: u32 = 0x8;
const TOKEN_USER: u32 = 1;
const SDDL_REVISION_1: u32 = 1;
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

#[link(name = "ntdll")]
unsafe extern "system" {
    fn RtlGetLastNtStatus() -> i32;
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
    fn GetCurrentThread() -> Handle;
    fn DuplicateHandle(from_process: Handle, from: Handle, to_process: Handle, to: *mut Handle, access: u32, inherit: i32, options: u32) -> i32;
    fn CancelSynchronousIo(thread: Handle) -> i32;
}

#[repr(C)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

const FOLDER_PROFILE: Guid = Guid { data1: 0x5E6C_858F, data2: 0x0E22, data3: 0x4760, data4: [0x9A, 0xFE, 0xEA, 0x33, 0x17, 0xB6, 0x71, 0x73] };
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

struct Local(*mut c_void);

impl Drop for Local {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { LocalFree(self.0) };
        }
    }
}

struct User(Vec<u64>);

impl User {
    fn now() -> std::io::Result<User> {
        User::of(unsafe { GetCurrentProcess() })
    }

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

pub fn open_to_others(_path: &Path, _group_only: bool) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "opening a file to other accounts is not done on this system"))
}

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

pub fn sync_dir(path: &Path, landed: &Path) -> std::io::Result<()> {
    let dir = std::fs::OpenOptions::new().write(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS).open(path).and_then(|d| d.sync_all());
    match dir {
        Ok(()) => Ok(()),
        Err(e) if !landed.is_file() => Err(e),
        Err(d) => sync_file(landed).map_err(|f| std::io::Error::new(f.kind(), format!("the directory: {d}; then the file {}: {f}", landed.display()))),
    }
}

pub fn sync_file(path: &Path) -> std::io::Result<()> {
    std::fs::OpenOptions::new().write(true).open(path)?.sync_all()
}

pub fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    move_file(from, to, MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)
}

pub fn rename_new(from: &Path, to: &Path) -> std::io::Result<()> {
    move_file(from, to, MOVEFILE_WRITE_THROUGH)
}

pub const MOVE_HELD_WAIT: std::time::Duration = std::time::Duration::from_secs(1);

const MOVE_HELD_PAUSE: std::time::Duration = std::time::Duration::from_millis(10);
const MOVE_HELD_PAUSE_MAX: std::time::Duration = std::time::Duration::from_millis(100);

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

const APP_FOLDER: &str = "ZIKARON";

pub const HOME_VAR: &str = "USERPROFILE";

pub fn home_dir() -> Option<std::path::PathBuf> {
    known_folder(&FOLDER_PROFILE)
}

pub fn app_data_dir(user_home: &Path) -> std::path::PathBuf {
    if home_dir().is_some_and(|profile| crate::same_folder(&profile, user_home)) {
        if let Some(local) = known_folder(&FOLDER_LOCAL_APP_DATA) {
            return local.join(APP_FOLDER);
        }
    }
    user_home.join("AppData").join("Local").join(APP_FOLDER)
}

pub const DOOR_PATH_MAX: usize = 256;

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
const BUSY_WAIT_MS: u32 = 250;

pub struct DoorStream {
    h: Handle,
    server: bool,
    read_deadline: std::cell::Cell<Option<std::time::Instant>>,
    write_deadline: std::cell::Cell<Option<std::time::Instant>>,
}

unsafe impl Send for DoorStream {}

impl Drop for DoorStream {
    fn drop(&mut self) {
        unsafe {
            if self.server {
                let left = self.write_left();
                if left.is_none_or(|l| !l.is_zero()) {
                    within(left, || FlushFileBuffers(self.h));
                }
                DisconnectNamedPipe(self.h);
            }
            CloseHandle(self.h);
        }
    }
}

const DUPLICATE_SAME_ACCESS: u32 = 0x2;
const ERROR_OPERATION_ABORTED: i32 = 995;

struct ThreadHandle(Handle);
unsafe impl Send for ThreadHandle {}

fn within<T>(deadline: Option<std::time::Duration>, op: impl FnOnce() -> T) -> Option<T> {
    let Some(d) = deadline else { return Some(op()) };
    let mut me: Handle = std::ptr::null_mut();
    let here = unsafe { GetCurrentProcess() };
    if unsafe { DuplicateHandle(here, GetCurrentThread(), here, &mut me, 0, 0, DUPLICATE_SAME_ACCESS) } == 0 {
        return None;
    }
    let watched = ThreadHandle(me);
    let (done, wait) = std::sync::mpsc::channel::<()>();
    let watch = std::thread::spawn(move || {
        let t = watched;
        let mut wait_for = d;
        while wait.recv_timeout(wait_for) == Err(std::sync::mpsc::RecvTimeoutError::Timeout) {
            unsafe { CancelSynchronousIo(t.0) };
            wait_for = std::time::Duration::from_millis(10);
        }
    });
    let out = op();
    drop(done);
    let _ = watch.join();
    unsafe { CloseHandle(me) };
    Some(out)
}

pub struct DoorListener {
    name: Vec<u16>,
    next: std::sync::Mutex<Handle>,
    closing: std::sync::atomic::AtomicBool,
}

unsafe impl Send for DoorListener {}
unsafe impl Sync for DoorListener {}

fn pipe_name(place: &Path) -> Vec<u16> {
    place.as_os_str().encode_wide().chain(std::iter::once(0)).collect()
}

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
        if closing() {
            unsafe { CloseHandle(h) };
            *next = INVALID_HANDLE_VALUE;
            return Err(door_gone());
        }
        *next = instance(&self.name, false)?;
        Ok(DoorStream { h, server: true, read_deadline: std::cell::Cell::new(None), write_deadline: std::cell::Cell::new(None) })
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

pub fn door_connect(place: &Path) -> std::io::Result<DoorStream> {
    let h = open_pipe(&pipe_name(place))?;
    Ok(DoorStream { h, server: false, read_deadline: std::cell::Cell::new(None), write_deadline: std::cell::Cell::new(None) })
}

impl DoorStream {
    pub fn same_user(&self) -> std::io::Result<()> {
        let mut pid = 0u32;
        let asked = unsafe { if self.server { GetNamedPipeClientProcessId(self.h, &mut pid) } else { GetNamedPipeServerProcessId(self.h, &mut pid) } };
        same_user(if asked == 0 { Err(std::io::Error::last_os_error()) } else { Ok(pid) })
    }

    pub fn set_read_deadline(&self, d: Option<std::time::Duration>) -> std::io::Result<()> {
        self.read_deadline.set(d.map(|d| std::time::Instant::now() + d));
        Ok(())
    }

    pub fn set_write_deadline(&self, d: Option<std::time::Duration>) -> std::io::Result<()> {
        self.write_deadline.set(d.map(|d| std::time::Instant::now() + d));
        Ok(())
    }

    fn write_left(&self) -> Option<std::time::Duration> {
        self.write_deadline.get().map(|u| u.saturating_duration_since(std::time::Instant::now()))
    }
}

impl std::io::Read for DoorStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(until) = self.read_deadline.get() {
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
        let late = || std::io::Error::new(std::io::ErrorKind::TimedOut, "the other end took nothing within the deadline");
        let left = self.write_left();
        if left.is_some_and(|l| l.is_zero()) {
            return Err(late());
        }
        let wrote = within(left, || if unsafe { WriteFile(self.h, buf.as_ptr(), len, &mut put, std::ptr::null_mut()) } == 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) });
        match wrote {
            None => Err(std::io::Error::other("the write could not be bounded")),
            Some(Err(e)) if e.raw_os_error() == Some(ERROR_OPERATION_ABORTED) => Err(late()),
            Some(Err(e)) => Err(e),
            Some(Ok(())) => Ok(put as usize),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn listed_id(file: &std::fs::File) -> i64 {
    use std::os::windows::io::AsRawHandle;
    file.as_raw_handle() as isize as i64
}

pub fn spawn(cmd: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    cmd.spawn()
}

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
    fn SystemParametersInfoW(action: u32, uparam: u32, pvparam: *mut c_void, winini: u32) -> i32;
}

const SPI_GETCLIENTAREAANIMATION: u32 = 0x1042;

pub fn reduce_motion() -> bool {
    let mut on: i32 = 1;
    // SAFETY: the call writes one BOOL into `on`.
    let ok = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, &mut on as *mut i32 as *mut c_void, 0) };
    ok != 0 && on == 0
}

pub(crate) const HKEY_CURRENT_USER: Handle = 0x8000_0001usize as Handle;
pub(crate) const HKEY_LOCAL_MACHINE: Handle = 0x8000_0002usize as Handle;
const KEY_READ: u32 = 0x2_0019;
const KEY_WRITE: u32 = 0x2_0006;
const REG_EXPAND_SZ: u32 = 2;
const ERROR_FILE_NOT_FOUND_REG: i32 = 2;
const PATH_MAX_CHARS: usize = 32_767;

fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn cli_default_dir() -> Option<std::path::PathBuf> {
    None
}

pub fn cli_unsupported(_cli: &Path) -> Option<String> {
    None
}

pub fn cli_provided(cli: &Path, moved: bool) -> bool {
    let Some(dir) = cli.parent() else { return false };
    machine_path(moved).is_ok_and(|p| p.split(';').any(|e| same_dir(e, dir)))
}

fn user_path(moved: bool) -> std::io::Result<String> {
    if moved {
        return stand_in(crate::cli_path::PATH_STAND_IN);
    }
    reg_path(HKEY_CURRENT_USER, "Environment")
}

fn machine_path(moved: bool) -> std::io::Result<String> {
    if moved {
        return stand_in(crate::cli_path::MACHINE_PATH_STAND_IN);
    }
    reg_path(HKEY_LOCAL_MACHINE, r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment")
}

fn stand_in(name: &str) -> std::io::Result<String> {
    let f = crate::cli_path::place_dir().unwrap_or_default().join(name);
    match std::fs::read_to_string(&f) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    }
}

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

fn reg_raw(root: Handle, sub: &str, name: &str) -> Option<Vec<u8>> {
    let mut key: Handle = std::ptr::null_mut();
    // SAFETY: opened for reading only and closed below; the first query asks the size, the second reads into a
    // buffer of that size.
    unsafe {
        if RegOpenKeyExW(root, w(sub).as_ptr(), 0, KEY_READ, &mut key) != 0 {
            return None;
        }
        let n = w(name);
        let mut len = 0u32;
        let mut out = None;
        if RegQueryValueExW(key, n.as_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), &mut len) == 0 {
            let mut buf = vec![0u8; len as usize];
            if RegQueryValueExW(key, n.as_ptr(), std::ptr::null_mut(), std::ptr::null_mut(), buf.as_mut_ptr(), &mut len) == 0 {
                buf.truncate(len as usize);
                out = Some(buf);
            }
        }
        RegCloseKey(key);
        out
    }
}

pub(crate) fn reg_dword(root: Handle, sub: &str, name: &str) -> Option<u32> {
    reg_raw(root, sub, name).and_then(|b| Some(u32::from_le_bytes(b.get(..4)?.try_into().ok()?)))
}

pub(crate) fn reg_text(root: Handle, sub: &str, name: &str) -> Option<String> {
    let b = reg_raw(root, sub, name)?;
    let units: Vec<u16> = b.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).take_while(|u| *u != 0).collect();
    Some(String::from_utf16_lossy(&units))
}

#[repr(C)]
struct IeProxyConfigRaw {
    _auto_detect: i32,
    auto_config_url: *mut u16,
    proxy: *mut u16,
    proxy_bypass: *mut u16,
}

#[link(name = "winhttp")]
unsafe extern "system" {
    fn WinHttpGetIEProxyConfigForCurrentUser(config: *mut IeProxyConfigRaw) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalFree(p: *mut c_void) -> *mut c_void;
}

pub(crate) struct IeProxy {
    pub auto_config_url: Option<String>,
    pub proxy: Option<String>,
    pub bypass: Option<String>,
}

pub(crate) fn ie_proxy_config() -> Option<IeProxy> {
    let mut raw = IeProxyConfigRaw { _auto_detect: 0, auto_config_url: std::ptr::null_mut(), proxy: std::ptr::null_mut(), proxy_bypass: std::ptr::null_mut() };
    // SAFETY: the call fills the structure; each non-null string is zero-terminated, read once and freed once.
    unsafe {
        if WinHttpGetIEProxyConfigForCurrentUser(&mut raw) == 0 {
            return None;
        }
        let take = |p: *mut u16| -> Option<String> {
            if p.is_null() {
                return None;
            }
            let mut n = 0;
            while *p.add(n) != 0 {
                n += 1;
            }
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
            GlobalFree(p.cast());
            Some(s).filter(|s| !s.trim().is_empty())
        };
        Some(IeProxy { auto_config_url: take(raw.auto_config_url), proxy: take(raw.proxy), bypass: take(raw.proxy_bypass) })
    }
}

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

fn same_dir(entry: &str, dir: &Path) -> bool {
    let norm = |s: &str| s.trim().trim_end_matches(['\\', '/']).to_lowercase();
    norm(entry) == norm(&dir.display().to_string())
}

pub fn cli_state(cli: &Path, moved: bool) -> crate::cli_path::State {
    use crate::cli_path::State;
    let Some(dir) = cli.parent() else { return State::Unsupported("no folder".into()) };
    match user_path(moved) {
        Ok(p) if p.split(';').any(|e| same_dir(e, dir)) => State::On,
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

    #[test]
    fn a_stand_in_home_keeps_the_app_folder_under_it() {
        let stand_in = Path::new(r"C:\stand-in\home");
        assert_eq!(app_data_dir(stand_in), stand_in.join("AppData").join("Local").join(APP_FOLDER));
    }

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

#[repr(C)]
struct Overlapped {
    internal: usize,
    internal_high: usize,
    offset: u32,
    offset_high: u32,
    event: Handle,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LockFileEx(file: Handle, flags: u32, reserved: u32, low: u32, high: u32, overlapped: *mut Overlapped) -> i32;
}

const LOCKFILE_FAIL_IMMEDIATELY: u32 = 0x1;
const LOCKFILE_EXCLUSIVE_LOCK: u32 = 0x2;

const LOCK_OFFSET_HIGH: u32 = 0x4000_0000;

fn lock_with(file: &std::fs::File, flags: u32) -> bool {
    use std::os::windows::io::AsRawHandle;
    let mut at = Overlapped { internal: 0, internal_high: 0, offset: 0, offset_high: LOCK_OFFSET_HIGH, event: std::ptr::null_mut() };
    // SAFETY: the handle stays open for the call (`file` holds it) and `at` outlives it.
    unsafe { LockFileEx(file.as_raw_handle() as Handle, flags, 0, 1, 0, &mut at) != 0 }
}

pub fn going(path: &Path) -> bool {
    let name = wide(path);
    let share = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
    // SAFETY: `name` is a NUL-terminated wide string that outlives the call; no security descriptor or template.
    let h = unsafe { CreateFileW(name.as_ptr(), 0, share, std::ptr::null_mut(), OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, std::ptr::null_mut()) };
    if h != INVALID_HANDLE_VALUE {
        // SAFETY: `h` is the handle just opened, closed once.
        unsafe { CloseHandle(h) };
        return false;
    }
    // SAFETY: reads this thread's last status; nothing has been called since the failed open.
    matches!(unsafe { RtlGetLastNtStatus() }, STATUS_OBJECT_NAME_NOT_FOUND | STATUS_OBJECT_PATH_NOT_FOUND | STATUS_DELETE_PENDING)
}

pub fn lock_now(file: &std::fs::File) -> bool {
    lock_with(file, LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY)
}

pub fn lock_wait(file: &std::fs::File) -> bool {
    lock_with(file, LOCKFILE_EXCLUSIVE_LOCK)
}

#[repr(C)]
struct Margins {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

#[repr(C)]
struct OsVersion {
    size: u32,
    major: u32,
    minor: u32,
    build: u32,
    platform: u32,
    service_pack: [u16; 128],
}

#[link(name = "dwmapi")]
unsafe extern "system" {
    fn DwmSetWindowAttribute(hwnd: isize, attribute: u32, value: *const c_void, size: u32) -> i32;
    fn DwmExtendFrameIntoClientArea(hwnd: isize, margins: *const Margins) -> i32;
}

#[link(name = "ntdll")]
unsafe extern "system" {
    fn RtlGetVersion(info: *mut OsVersion) -> i32;
}

const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
const DWMWA_SYSTEMBACKDROP_TYPE: u32 = 38;
const DWMSBT_MAINWINDOW: i32 = 2;
const MICA_BUILD: u32 = 22621;

fn build() -> u32 {
    let mut v = OsVersion { size: std::mem::size_of::<OsVersion>() as u32, major: 0, minor: 0, build: 0, platform: 0, service_pack: [0; 128] };
    if unsafe { RtlGetVersion(&mut v) } == 0 { v.build } else { 0 }
}

pub fn backdrop_available() -> bool {
    build() >= MICA_BUILD
}

fn set_dark(hwnd: isize, dark: bool) {
    let on: i32 = dark as i32;
    unsafe { DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &on as *const i32 as *const c_void, 4) };
}

pub fn backdrop(w: crate::Window, dark: bool, _follows: bool) -> crate::Backdrop {
    let crate::Window::Win32(hwnd) = w else { return crate::Backdrop::Tint };
    if hwnd == 0 || !backdrop_available() {
        return crate::Backdrop::Tint;
    }
    set_dark(hwnd, dark);
    let whole = Margins { left: -1, right: -1, top: -1, bottom: -1 };
    let kind = DWMSBT_MAINWINDOW;
    let extended = unsafe { DwmExtendFrameIntoClientArea(hwnd, &whole) };
    let set = unsafe { DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, &kind as *const i32 as *const c_void, 4) };
    if extended >= 0 && set >= 0 { crate::Backdrop::Mica } else { crate::Backdrop::Tint }
}

pub fn backdrop_appearance(w: crate::Window, dark: bool, _follows: bool) {
    if let crate::Window::Win32(hwnd) = w {
        if hwnd != 0 {
            set_dark(hwnd, dark);
        }
    }
}
