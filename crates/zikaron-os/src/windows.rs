//! The Windows implementation. No third-party crate: the system's interfaces it needs are declared here.
//!
//! - Entropy: `BCryptGenRandom` with the system's preferred generator.
//! - Owner-only: the creating call (`CreateFileW`) carries a security descriptor whose access list is
//!   protected (nothing inherited) and allows this process's user alone; read back, a file is owner-only when
//!   every allowing entry of its access list names this user.
//! - Syncing: a directory is opened with backup semantics and flushed; where the system does not flush a
//!   directory, the file that just took its name in it is flushed instead (NTFS logs in order, so the name goes
//!   to disk with it). Which of the two a given volume takes is the system's to say; both are here, in that
//!   order.
//! - Replacing rename: `MoveFileExW` replacing an existing target and writing through.

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

/// The fixed head of an access list (`ACL`).
#[repr(C)]
struct Acl {
    revision: u8,
    sbz1: u8,
    size: u16,
    ace_count: u16,
    sbz2: u16,
}

#[link(name = "bcrypt")]
extern "system" {
    fn BCryptGenRandom(algorithm: Handle, buffer: *mut u8, count: u32, flags: u32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn CreateFileW(name: *const u16, access: u32, share: u32, security: *mut SecurityAttributes, disposition: u32, flags: u32, template: Handle) -> Handle;
    fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
    fn GetCurrentProcess() -> Handle;
    fn CloseHandle(h: Handle) -> i32;
    fn LocalFree(p: *mut c_void) -> *mut c_void;
}

#[link(name = "advapi32")]
extern "system" {
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

/// A path for the wide-character calls, ending in a zero. An absolute path long enough to meet the old length
/// limit is given in the extended form (`\\?\`, or `\\?\UNC\` for a share), as the standard library does.
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

/// Memory the system handed out with `LocalAlloc`, given back on drop.
struct Local(*mut c_void);

impl Drop for Local {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { LocalFree(self.0) };
        }
    }
}

/// This process's user, as the token says it: the buffer holding `TOKEN_USER`, whose first member points at the
/// user's security identifier inside the same buffer.
struct User(Vec<u64>);

impl User {
    fn now() -> std::io::Result<User> {
        let mut token: Handle = std::ptr::null_mut();
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
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

    /// The security descriptor of a file only this user may reach: a protected access list (nothing inherited
    /// from the folder) with one entry allowing this user all file access.
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
    // The standard library's rule: creating or cutting needs write access.
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

/// Owner-only when the file's access list allows nobody but this user: every allowing entry names this user
/// (denying entries take nothing away from that). No access list at all lets everyone in, so it is not; an entry
/// of a kind not read here counts as letting someone else in.
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

/// First the directory itself, flushed through a handle opened with backup semantics (the one way to open a
/// directory); when that cannot be done, the file that just took its name in it. With no such file there
/// (nothing new landed), the directory's own failure is said.
pub fn sync_dir(path: &Path, landed: &Path) -> std::io::Result<()> {
    let dir = std::fs::OpenOptions::new().write(true).custom_flags(FILE_FLAG_BACKUP_SEMANTICS).open(path).and_then(|d| d.sync_all());
    match dir {
        Ok(()) => Ok(()),
        Err(e) if !landed.is_file() => Err(e),
        Err(_) => sync_file(landed),
    }
}

/// Flushing needs a handle with write access.
pub fn sync_file(path: &Path) -> std::io::Result<()> {
    std::fs::OpenOptions::new().write(true).open(path)?.sync_all()
}

pub fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    let (f, t) = (wide(from), wide(to));
    if unsafe { MoveFileExW(f.as_ptr(), t.as_ptr(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read back: a file made without the rule in the temporary folder inherits that folder's access list
    /// (the system and administrators besides this user), so it is not owner-only; one made with it is.
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
