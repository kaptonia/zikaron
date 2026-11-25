use std::io::Read;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

pub const ENTROPY_SOURCE: &str = "/dev/urandom";

pub fn fill_random(buf: &mut [u8]) -> std::io::Result<()> {
    std::fs::File::open(ENTROPY_SOURCE)?.read_exact(buf)
}

pub fn open(o: &crate::Options, path: &Path) -> std::io::Result<std::fs::File> {
    let mut s = std::fs::OpenOptions::new();
    s.read(o.read).write(o.write).create(o.create).create_new(o.create_new).truncate(o.truncate);
    if o.owner_only {
        s.mode(0o600);
    }
    s.open(path)
}

pub fn open_to_others(path: &Path, group_only: bool) -> std::io::Result<()> {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(if group_only { 0o640 } else { 0o644 }))
}

pub fn is_owner_only(path: &Path) -> std::io::Result<bool> {
    Ok(std::fs::metadata(path)?.permissions().mode() & 0o077 == 0)
}

pub fn sync_dir(path: &Path, _landed: &Path) -> std::io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

pub fn sync_file(path: &Path) -> std::io::Result<()> {
    std::fs::File::open(path)?.sync_all()
}

pub fn replace(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::rename(from, to)
}

fn c_path(p: &Path) -> std::io::Result<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    std::ffi::CString::new(p.as_os_str().as_bytes()).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "a path with a NUL byte"))
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn renamex_np(from: *const std::ffi::c_char, to: *const std::ffi::c_char, flags: u32) -> i32;
}

#[cfg(target_os = "macos")]
pub fn rename_new(from: &Path, to: &Path) -> std::io::Result<()> {
    const RENAME_EXCL: u32 = 0x4;
    let (f, t) = (c_path(from)?, c_path(to)?);
    // SAFETY: both paths are NUL-terminated and live for the call.
    if unsafe { renamex_np(f.as_ptr(), t.as_ptr(), RENAME_EXCL) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
unsafe extern "C" {
    fn renameat2(olddir: i32, from: *const std::ffi::c_char, newdir: i32, to: *const std::ffi::c_char, flags: u32) -> i32;
}

#[cfg(not(target_os = "macos"))]
pub fn rename_new(from: &Path, to: &Path) -> std::io::Result<()> {
    const AT_FDCWD: i32 = -100;
    const RENAME_NOREPLACE: u32 = 1;
    let (f, t) = (c_path(from)?, c_path(to)?);
    // SAFETY: both paths are NUL-terminated and live for the call.
    if unsafe { renameat2(AT_FDCWD, f.as_ptr(), AT_FDCWD, t.as_ptr(), RENAME_NOREPLACE) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

pub const HOME_VAR: &str = "HOME";

pub fn home_dir() -> Option<std::path::PathBuf> {
    None
}

pub fn app_data_dir(user_home: &Path) -> std::path::PathBuf {
    user_home.to_path_buf()
}

use std::os::unix::net::{UnixListener, UnixStream};

#[cfg(target_os = "macos")]
pub const DOOR_PATH_MAX: usize = 103;
#[cfg(not(target_os = "macos"))]
pub const DOOR_PATH_MAX: usize = 107;

pub fn door_place(machine: &Path, name: &str) -> std::path::PathBuf {
    machine.join(name)
}

fn too_long(place: &Path) -> bool {
    use std::os::unix::ffi::OsStrExt;
    place.as_os_str().as_bytes().len() > DOOR_PATH_MAX
}

fn not_open(why: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::NotFound, why.to_string())
}

pub struct DoorListener {
    l: UnixListener,
    wake: (UnixStream, UnixStream),
}
pub struct DoorStream(UnixStream, Due, Due);

type Due = std::cell::Cell<Option<std::time::Instant>>;

fn due(d: Option<std::time::Duration>) -> Due {
    std::cell::Cell::new(d.map(|d| std::time::Instant::now() + d))
}

fn left(until: &Due, late: &str) -> std::io::Result<Option<std::time::Duration>> {
    match until.get().map(|u| u.saturating_duration_since(std::time::Instant::now())) {
        Some(l) if l.is_zero() => Err(std::io::Error::new(std::io::ErrorKind::TimedOut, late.to_string())),
        l => Ok(l),
    }
}

const READ_LATE: &str = "nothing came within the deadline";
const WRITE_LATE: &str = "the other end took nothing within the deadline";

#[repr(C)]
struct PollFd {
    fd: i32,
    events: i16,
    revents: i16,
}

const POLLIN: i16 = 0x1;

#[cfg(any(target_os = "linux", target_os = "android"))]
type Nfds = std::ffi::c_ulong;
#[cfg(not(any(target_os = "linux", target_os = "android")))]
type Nfds = std::ffi::c_uint;

unsafe extern "C" {
    fn poll(fds: *mut PollFd, nfds: Nfds, timeout: i32) -> i32;
}

unsafe extern "C" {
    fn getuid() -> u32;
}

#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
unsafe extern "C" {
    fn getpeereid(fd: i32, uid: *mut u32, gid: *mut u32) -> i32;
}

#[cfg(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd"))]
fn peer_uid(s: &UnixStream) -> std::io::Result<u32> {
    use std::os::fd::AsRawFd;
    let (mut uid, mut gid) = (0u32, 0u32);
    // SAFETY: the descriptor is a connected socket this stream owns; both out-pointers live for the call.
    if unsafe { getpeereid(s.as_raw_fd(), &mut uid, &mut gid) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(uid)
}

#[cfg(not(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd")))]
unsafe extern "C" {
    fn getsockopt(fd: i32, level: i32, name: i32, value: *mut std::ffi::c_void, len: *mut u32) -> i32;
}

#[cfg(not(any(target_os = "macos", target_os = "freebsd", target_os = "openbsd", target_os = "netbsd")))]
fn peer_uid(s: &UnixStream) -> std::io::Result<u32> {
    use std::os::fd::AsRawFd;
    #[repr(C)]
    struct Cred {
        pid: i32,
        uid: u32,
        gid: u32,
    }
    const SOL_SOCKET: i32 = 1;
    const SO_PEERCRED: i32 = 17;
    let mut c = Cred { pid: 0, uid: u32::MAX, gid: 0 };
    let mut len = std::mem::size_of::<Cred>() as u32;
    // SAFETY: the descriptor is a connected socket this stream owns; the buffer is `Cred`-sized and lives.
    if unsafe { getsockopt(s.as_raw_fd(), SOL_SOCKET, SO_PEERCRED, (&mut c as *mut Cred).cast(), &mut len) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(c.uid)
}

fn same_user(s: &UnixStream) -> std::io::Result<()> {
    // SAFETY: `getuid` has no failure and no arguments.
    let me = unsafe { getuid() };
    match peer_uid(s)? {
        u if u == me => Ok(()),
        u => Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, format!("the other end is user {u}, not this user"))),
    }
}

pub fn door_listen(place: &Path) -> std::io::Result<DoorListener> {
    use std::os::unix::fs::FileTypeExt;
    if too_long(place) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("longer than a local socket's {DOOR_PATH_MAX} bytes")));
    }
    let l = match UnixListener::bind(place) {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            let socket = std::fs::symlink_metadata(place).map(|m| m.file_type().is_socket()).unwrap_or(false);
            if !socket {
                return Err(std::io::Error::new(std::io::ErrorKind::AddrInUse, "something other than a door is at this place"));
            }
            if UnixStream::connect(place).is_ok() {
                return Err(std::io::Error::new(std::io::ErrorKind::AddrInUse, "a door is open at this place"));
            }
            std::fs::remove_file(place)?;
            UnixListener::bind(place)?
        }
        Err(e) => return Err(e),
    };
    std::fs::set_permissions(place, std::fs::Permissions::from_mode(0o600))?;
    l.set_nonblocking(true)?;
    let wake = UnixStream::pair()?;
    wake.1.set_nonblocking(true)?;
    Ok(DoorListener { l, wake })
}

impl DoorListener {
    pub fn accept(&self) -> std::io::Result<DoorStream> {
        use std::os::fd::AsRawFd;
        loop {
            let mut fds = [PollFd { fd: self.l.as_raw_fd(), events: POLLIN, revents: 0 }, PollFd { fd: self.wake.0.as_raw_fd(), events: POLLIN, revents: 0 }];
            // SAFETY: two live descriptors this listener owns, in an array that outlives the call.
            if unsafe { poll(fds.as_mut_ptr(), 2, -1) } < 0 {
                let e = std::io::Error::last_os_error();
                if e.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                return Err(e);
            }
            if fds[1].revents != 0 {
                return Err(not_open("the door is closed"));
            }
            match self.l.accept() {
                Ok((s, _)) => {
                    s.set_nonblocking(false)?;
                    return Ok(DoorStream(s, due(None), due(None)));
                }
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted) => continue,
                Err(e) => return Err(e),
            }
        }
    }
}

pub fn door_close(place: &Path, listener: Option<&DoorListener>) {
    if let Some(l) = listener {
        let _ = std::io::Write::write(&mut &l.wake.1, &[1]);
    }
    door_remove(place);
}

fn door_remove(place: &Path) {
    use std::os::unix::fs::FileTypeExt;
    if std::fs::symlink_metadata(place).map(|m| m.file_type().is_socket()).unwrap_or(false) {
        let _ = std::fs::remove_file(place);
    }
}

pub fn door_connect(place: &Path) -> std::io::Result<DoorStream> {
    if too_long(place) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("longer than a local socket's {DOOR_PATH_MAX} bytes")));
    }
    let s = match UnixStream::connect(place) {
        Ok(s) => s,
        Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused) => return Err(not_open(&e.to_string())),
        Err(e) => return Err(e),
    };
    Ok(DoorStream(s, due(None), due(None)))
}

impl DoorStream {
    pub fn same_user(&self) -> std::io::Result<()> {
        same_user(&self.0)
    }

    pub fn set_read_deadline(&self, d: Option<std::time::Duration>) -> std::io::Result<()> {
        self.1.set(due(d).get());
        if d.is_none() {
            let _ = self.0.set_read_timeout(None);
        }
        Ok(())
    }

    pub fn set_write_deadline(&self, d: Option<std::time::Duration>) -> std::io::Result<()> {
        self.2.set(due(d).get());
        if d.is_none() {
            let _ = self.0.set_write_timeout(None);
        }
        Ok(())
    }
}

impl std::io::Read for DoorStream {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if let Some(l) = left(&self.1, READ_LATE)? {
            let _ = self.0.set_read_timeout(Some(l));
        }
        self.0.read(buf).map_err(|e| if e.kind() == std::io::ErrorKind::WouldBlock { std::io::Error::new(std::io::ErrorKind::TimedOut, READ_LATE) } else { e })
    }
}

impl std::io::Write for DoorStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(l) = left(&self.2, WRITE_LATE)? {
            let _ = self.0.set_write_timeout(Some(l));
        }
        std::io::Write::write(&mut self.0, buf).map_err(|e| if e.kind() == std::io::ErrorKind::WouldBlock { std::io::Error::new(std::io::ErrorKind::TimedOut, WRITE_LATE) } else { e })
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.0)
    }
}

#[cfg(target_os = "macos")]
pub fn cli_default_dir() -> Option<std::path::PathBuf> {
    Some(std::path::PathBuf::from("/usr/local/bin"))
}

#[cfg(not(target_os = "macos"))]
pub fn cli_default_dir() -> Option<std::path::PathBuf> {
    crate::user_home().map(|h| h.join(".local").join("bin"))
}

unsafe extern "C" {
    fn access(path: *const std::ffi::c_char, mode: i32) -> i32;
}

unsafe extern "C" {
    fn close(fd: i32) -> i32;
}

pub fn listed_id(file: &std::fs::File) -> i64 {
    use std::os::fd::AsRawFd;
    i64::from(file.as_raw_fd())
}

pub fn spawn(cmd: &mut std::process::Command) -> std::io::Result<std::process::Child> {
    use std::os::unix::process::CommandExt;
    // SAFETY: the closure runs in the child between fork and exec; it only reads atomics and calls `close`,
    // both async-signal-safe, and allocates nothing.
    unsafe {
        cmd.pre_exec(|| {
            for slot in &crate::LISTED {
                let fd = slot.load(std::sync::atomic::Ordering::SeqCst);
                if fd > 2 {
                    close(fd as i32);
                }
            }
            Ok(())
        });
    }
    cmd.spawn()
}

pub fn cli_unsupported(cli: &Path) -> Option<String> {
    const W_OK: i32 = 2;
    const EROFS: i32 = 30;
    if std::env::var_os("APPIMAGE").is_some_and(|v| !v.is_empty()) {
        return Some("an AppImage is mounted afresh at every start; a link to it would lead nowhere".into());
    }
    let dir = cli.parent()?;
    let c = c_path(dir).ok()?;
    // SAFETY: the path is NUL-terminated and lives for the call.
    if unsafe { access(c.as_ptr(), W_OK) } != 0 && std::io::Error::last_os_error().raw_os_error() == Some(EROFS) {
        return Some(format!("{} is on a read-only disk (a disk image): move the app to its folder of applications first", dir.display()));
    }
    None
}

pub fn cli_provided(cli: &Path, _moved: bool) -> bool {
    let Some(dir) = cli.parent() else { return false };
    let Some(path) = std::env::var_os("PATH") else { return false };
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let own = crate::cli_path::place_dir().map(|p| real(&p));
    std::env::split_paths(&path).any(|p| real(&p) == real(dir) && Some(real(&p)) != own)
}

pub fn cli_state(cli: &Path, _moved: bool) -> crate::cli_path::State {
    use crate::cli_path::{State, NAME};
    let Some(dir) = crate::cli_path::place_dir() else { return State::Unsupported("no place for the link".into()) };
    let link = dir.join(NAME);
    let meta = match std::fs::symlink_metadata(&link) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return State::Off,
        Err(e) => return State::Taken(format!("{}: {e}", link.display())),
    };
    if !meta.file_type().is_symlink() {
        return State::Taken(link.display().to_string());
    }
    let Ok(to) = std::fs::read_link(&link) else { return State::Taken(link.display().to_string()) };
    let end = if to.is_absolute() { to.clone() } else { dir.join(&to) };
    let real = |p: &Path| std::fs::canonicalize(p).ok();
    match real(&end) {
        Some(e) if Some(e.clone()) == real(cli) => State::On,
        Some(_) => State::Taken(to.display().to_string()),
        None if to.file_name().and_then(|n| n.to_str()) == Some(NAME) => State::Off,
        None => State::Taken(to.display().to_string()),
    }
}

fn denied(e: &std::io::Error) -> bool {
    matches!(e.kind(), std::io::ErrorKind::PermissionDenied) || e.raw_os_error() == Some(1)
}

pub fn cli_enable(cli: &Path, moved: bool) -> Result<(), crate::cli_path::Refused> {
    use crate::cli_path::{Refused, NAME};
    let dir = crate::cli_path::place_dir().ok_or_else(|| Refused::Unsupported("no place for the link".into()))?;
    let link = dir.join(NAME);
    let here = (|| {
        std::fs::create_dir_all(&dir)?;
        if std::fs::symlink_metadata(&link).is_ok_and(|m| m.file_type().is_symlink()) && !link.exists() {
            std::fs::remove_file(&link)?;
        }
        std::os::unix::fs::symlink(cli, &link)
    })();
    match here {
        Ok(()) => Ok(()),
        Err(e) if denied(&e) && !moved && cfg!(target_os = "macos") => as_admin(
            "do shell script \"/bin/mkdir -p \" & quoted form of (item 1 of argv) & \" && { [ ! -L \" & quoted form of (item 2 of argv) & \" ] || [ -e \" & quoted form of (item 2 of argv) & \" ] || /bin/rm \" & quoted form of (item 2 of argv) & \"; } && /bin/ln -s \" & quoted form of (item 3 of argv) & \" \" & quoted form of (item 2 of argv) with administrator privileges",
            &[&dir, &link, cli],
        ),
        Err(e) => Err(Refused::NotAllowed(format!("{}: {e}", link.display()))),
    }
}

pub fn cli_disable(_cli: &Path, moved: bool) -> Result<(), crate::cli_path::Refused> {
    use crate::cli_path::{Refused, NAME};
    let dir = crate::cli_path::place_dir().ok_or_else(|| Refused::Unsupported("no place for the link".into()))?;
    let link = dir.join(NAME);
    match std::fs::remove_file(&link) {
        Ok(()) => Ok(()),
        Err(e) if denied(&e) && !moved && cfg!(target_os = "macos") => {
            as_admin("do shell script \"[ -L \" & quoted form of (item 1 of argv) & \" ] && /bin/rm \" & quoted form of (item 1 of argv) with administrator privileges", &[&link])
        }
        Err(e) => Err(Refused::NotAllowed(format!("{}: {e}", link.display()))),
    }
}

fn as_admin(line: &str, paths: &[&Path]) -> Result<(), crate::cli_path::Refused> {
    use crate::cli_path::Refused;
    let mut c = std::process::Command::new("/usr/bin/osascript");
    c.args(["-e", "on run argv", "-e", line, "-e", "end run"]);
    c.args(paths);
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let out = crate::spawn(&mut c).and_then(|ch| ch.wait_with_output()).map_err(|e| Refused::NotAllowed(e.to_string()))?;
    if out.status.success() {
        return Ok(());
    }
    Err(admin_refusal(&String::from_utf8_lossy(&out.stderr)))
}

pub(crate) fn admin_refusal(stderr: &str) -> crate::cli_path::Refused {
    let said = stderr.trim().to_string();
    if said.contains("(-128)") {
        return crate::cli_path::Refused::Cancelled;
    }
    crate::cli_path::Refused::NotAllowed(said)
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn a_folder_reached_by_a_link_is_the_same_folder() {
        let d = std::env::temp_dir().join(format!("zikaron-os-link-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let other = d.join("other");
        std::fs::create_dir_all(&other).expect("other");
        let link = d.join("link");
        std::os::unix::fs::symlink(&other, &link).expect("a link");
        assert!(crate::same_folder(&other, &link) && crate::same_folder(&link, &d.join("link").join("..").join("other")), "a link to it");
        assert!(!crate::same_folder(&d, &link), "a link to another folder");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_administrator_dialog_is_read_by_what_it_said() {
        use crate::cli_path::Refused;
        assert_eq!(super::admin_refusal("execution error: User canceled. (-128)\n"), Refused::Cancelled);
        assert_eq!(super::admin_refusal("execution error: The administrator user name or password was incorrect. (-60007)"), Refused::NotAllowed("execution error: The administrator user name or password was incorrect. (-60007)".into()));
    }

    #[test]
    fn a_file_others_may_read_is_not_owner_only() {
        let p = std::env::temp_dir().join(format!("zikaron-os-plain-{}", std::process::id()));
        std::fs::write(&p, b"x").expect("plain");
        for (mode, owner) in [(0o644, false), (0o604, false), (0o620, false), (0o600, true), (0o400, true)] {
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).expect("mode");
            assert_eq!(super::is_owner_only(&p).expect("read"), owner, "{mode:o}");
        }
        let _ = std::fs::remove_file(&p);
    }
}

#[cfg(test)]
mod cli_path_tests {
    use crate::cli_path::*;
    use std::path::{Path, PathBuf};
    use std::os::unix::fs::symlink;

    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct Bench {
        root: PathBuf,
        dir: PathBuf,
        cli: PathBuf,
    }

    fn bench(tag: &str) -> (Bench, std::sync::MutexGuard<'static, ()>) {
        let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
        let root = std::env::temp_dir().join(format!("zikaron-os-cli-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let app = root.join("App.app").join("Contents").join("MacOS");
        std::fs::create_dir_all(&app).expect("app");
        let cli = app.join(NAME);
        std::fs::write(&cli, b"#!").expect("cli");
        let dir = root.join("bin");
        // SAFETY: tests that set this variable run serially (`TURN`); nothing else in this crate reads it.
        unsafe { std::env::set_var(ENV_DIR, &dir) };
        (Bench { root, dir, cli }, turn)
    }

    impl Drop for Bench {
        fn drop(&mut self) {
            unsafe { std::env::remove_var(ENV_DIR) };
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn on_and_off_make_and_remove_the_one_link() {
        let (b, _t) = bench("onoff");
        assert_eq!(state(&b.cli), State::Off);
        assert_eq!(enable(&b.cli), Ok(State::On));
        let link = b.dir.join(NAME);
        assert_eq!(std::fs::read_link(&link).expect("a link"), b.cli);
        assert_eq!(enable(&b.cli), Ok(State::On), "on again: nothing more");
        assert_eq!(disable(&b.cli), Ok(State::Off));
        assert!(std::fs::symlink_metadata(&link).is_err() && b.dir.is_dir(), "the link gone, the folder kept");
        assert_eq!(disable(&b.cli), Ok(State::Off), "off again: nothing more");
    }

    #[test]
    fn what_another_install_put_there_is_never_touched() {
        let (b, _t) = bench("taken");
        std::fs::create_dir_all(&b.dir).expect("dir");
        let link = b.dir.join(NAME);
        let other = b.root.join("Other.app").join("Contents").join("MacOS");
        std::fs::create_dir_all(&other).expect("other");
        std::fs::write(other.join(NAME), b"other").expect("other cli");
        for (what, lay) in [
            ("a plain file", Box::new(|l: &Path| std::fs::write(l, b"pkg").expect("file")) as Box<dyn Fn(&Path)>),
            ("a link to another program", Box::new(|l: &Path| symlink("/bin/ls", l).expect("link"))),
            ("a link to another app's command line", Box::new(move |l: &Path| symlink(other.join(NAME), l).expect("link"))),
        ] {
            let _ = std::fs::remove_file(&link);
            lay(&link);
            let before = (std::fs::symlink_metadata(&link).expect("there").file_type(), std::fs::read_link(&link).ok(), std::fs::read(&link).ok());
            assert!(matches!(state(&b.cli), State::Taken(_)), "{what}");
            assert!(matches!(enable(&b.cli), Err(Refused::Taken(_))), "{what}: on refused");
            assert!(matches!(disable(&b.cli), Err(Refused::Taken(_))), "{what}: off refused");
            let after = (std::fs::symlink_metadata(&link).expect("still there").file_type(), std::fs::read_link(&link).ok(), std::fs::read(&link).ok());
            assert_eq!(before, after, "{what}: not one byte changed");
        }
    }

    #[test]
    fn a_link_to_a_moved_app_is_off_and_made_again() {
        let (b, _t) = bench("dangling");
        std::fs::create_dir_all(&b.dir).expect("dir");
        let link = b.dir.join(NAME);
        symlink(b.root.join("Gone.app").join("Contents").join("MacOS").join(NAME), &link).expect("dangling");
        assert_eq!(state(&b.cli), State::Off);
        assert_eq!(enable(&b.cli), Ok(State::On));
        assert_eq!(std::fs::read_link(&link).expect("link"), b.cli);
        std::fs::remove_file(&link).expect("rm");
        symlink(b.root.join("nowhere").join("tool"), &link).expect("dangling other");
        assert!(matches!(state(&b.cli), State::Taken(_)), "a link to nowhere of another kind");
    }

    #[test]
    fn a_place_not_writable_and_no_command_line_are_named() {
        use std::os::unix::fs::PermissionsExt;
        let (b, _t) = bench("denied");
        std::fs::create_dir_all(&b.dir).expect("dir");
        std::fs::set_permissions(&b.dir, std::fs::Permissions::from_mode(0o500)).expect("read only");
        let r = enable(&b.cli);
        std::fs::set_permissions(&b.dir, std::fs::Permissions::from_mode(0o700)).expect("back");
        assert!(matches!(r, Err(Refused::NotAllowed(_))), "{r:?}");
        assert!(std::fs::symlink_metadata(b.dir.join(NAME)).is_err());
        let gone = b.root.join("none").join(NAME);
        assert!(matches!(state(&gone), State::Unsupported(_)));
        assert!(matches!(enable(&gone), Err(Refused::Unsupported(_))));
    }

    #[test]
    fn an_appimage_is_not_supported() {
        let (b, _t) = bench("appimage");
        unsafe { std::env::set_var("APPIMAGE", "/tmp/Z.AppImage") };
        let s = (state(&b.cli), enable(&b.cli));
        unsafe { std::env::remove_var("APPIMAGE") };
        assert!(matches!(s.0, State::Unsupported(_)) && matches!(s.1, Err(Refused::Unsupported(_))), "{s:?}");
        assert!(std::fs::symlink_metadata(b.dir.join(NAME)).is_err());
    }

    #[test]
    fn a_command_line_on_the_path_where_it_ships_is_provided() {
        let (b, _t) = bench("provided");
        let was = std::env::var_os("PATH");
        let dir = b.cli.parent().expect("dir").to_path_buf();
        let path = std::env::join_paths(std::iter::once(dir).chain(was.iter().flat_map(std::env::split_paths))).expect("path");
        unsafe { std::env::set_var("PATH", &path) };
        let s = (state(&b.cli), enable(&b.cli), disable(&b.cli));
        match was {
            Some(p) => unsafe { std::env::set_var("PATH", p) },
            None => unsafe { std::env::remove_var("PATH") },
        }
        assert_eq!(s, (State::Provided, Ok(State::Provided), Ok(State::Provided)));
        assert!(std::fs::symlink_metadata(b.dir.join(NAME)).is_err(), "nothing made");
    }
}

unsafe extern "C" {
    fn flock(fd: i32, op: i32) -> i32;
}

const LOCK_EX: i32 = 2;
const LOCK_NB: i32 = 4;

pub fn going(path: &std::path::Path) -> bool {
    matches!(std::fs::symlink_metadata(path), Err(e) if e.kind() == std::io::ErrorKind::NotFound)
}

pub fn lock_now(file: &std::fs::File) -> bool {
    use std::os::unix::io::AsRawFd;
    // SAFETY: `flock` reads only the descriptor, which `file` keeps open for the call.
    unsafe { flock(file.as_raw_fd(), LOCK_EX | LOCK_NB) == 0 }
}

pub fn lock_wait(file: &std::fs::File) -> bool {
    use std::os::unix::io::AsRawFd;
    // SAFETY: as above.
    unsafe { flock(file.as_raw_fd(), LOCK_EX) == 0 }
}

#[cfg(target_os = "macos")]
mod appkit {
    use std::ffi::{c_char, c_void, CString};

    #[link(name = "AppKit", kind = "framework")]
    unsafe extern "C" {}

    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
        fn objc_allocateClassPair(superclass: *mut c_void, name: *const c_char, extra: usize) -> *mut c_void;
        fn objc_registerClassPair(class: *mut c_void);
        fn class_addMethod(class: *mut c_void, name: *mut c_void, imp: *const c_void, types: *const c_char) -> i8;
    }

    pub type Id = *mut c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Rect {
        pub x: f64,
        pub y: f64,
        pub w: f64,
        pub h: f64,
    }

    pub fn sel(name: &str) -> Id {
        let c = CString::new(name).unwrap_or_default();
        unsafe { sel_registerName(c.as_ptr()) }
    }

    pub fn class(name: &str) -> Id {
        let c = CString::new(name).unwrap_or_default();
        unsafe { objc_getClass(c.as_ptr()) }
    }

    pub unsafe fn send(obj: Id, s: &str) -> Id {
        let f: unsafe extern "C" fn(Id, Id) -> Id = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(obj, sel(s)) }
    }

    pub unsafe fn send_id(obj: Id, s: &str, arg: Id) -> Id {
        let f: unsafe extern "C" fn(Id, Id, Id) -> Id = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(obj, sel(s), arg) }
    }

    pub unsafe fn send_int(obj: Id, s: &str, n: isize) {
        let f: unsafe extern "C" fn(Id, Id, isize) = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(obj, sel(s), n) }
    }

    pub unsafe fn send_bool(obj: Id, s: &str, b: bool) {
        let f: unsafe extern "C" fn(Id, Id, bool) = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(obj, sel(s), b) }
    }

    pub unsafe fn send_bool_ret(obj: Id, s: &str) -> bool {
        let f: unsafe extern "C" fn(Id, Id) -> i8 = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(obj, sel(s)) != 0 }
    }

    pub unsafe fn send_rect(obj: Id, s: &str) -> Rect {
        let f: unsafe extern "C" fn(Id, Id) -> Rect = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(obj, sel(s)) }
    }

    pub unsafe fn init_frame(obj: Id, r: Rect) -> Id {
        let f: unsafe extern "C" fn(Id, Id, Rect) -> Id = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(obj, sel("initWithFrame:"), r) }
    }

    pub unsafe fn add_subview(parent: Id, view: Id, place: isize, other: Id) {
        let f: unsafe extern "C" fn(Id, Id, Id, isize, Id) = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(parent, sel("addSubview:positioned:relativeTo:"), view, place, other) }
    }

    pub unsafe fn make_class(name: &str, superclass: &str, methods: &[(&str, unsafe extern "C" fn(Id, Id, Id))]) -> Option<Id> {
        let c = CString::new(name).ok()?;
        let cls = unsafe { objc_allocateClassPair(class(superclass), c.as_ptr(), 0) };
        if cls.is_null() {
            return None;
        }
        let types = CString::new("v@:@").ok()?;
        for (s, imp) in methods {
            unsafe { class_addMethod(cls, sel(s), *imp as *const c_void, types.as_ptr()) };
        }
        unsafe { objc_registerClassPair(cls) };
        Some(cls)
    }

    pub unsafe fn observe(center: Id, observer: Id, s: &str, name: Id, object: Id) {
        let f: unsafe extern "C" fn(Id, Id, Id, Id, Id, Id) = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(center, sel("addObserver:selector:name:object:"), observer, sel(s), name, object) }
    }

    pub unsafe fn send_uint_ret(obj: Id, s: &str) -> usize {
        let f: unsafe extern "C" fn(Id, Id) -> usize = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(obj, sel(s)) }
    }

    pub unsafe fn string(s: &str) -> Id {
        let c = CString::new(s).unwrap_or_default();
        let f: unsafe extern "C" fn(Id, Id, *const c_char) -> Id = unsafe { std::mem::transmute(objc_msgSend as *const ()) };
        unsafe { f(class("NSString"), sel("stringWithUTF8String:"), c.as_ptr()) }
    }
}

#[cfg(target_os = "macos")]
pub fn backdrop_available() -> bool {
    true
}

#[cfg(target_os = "macos")]
fn set_appearance(window: appkit::Id, dark: bool, follows: bool) {
    use appkit::*;
    unsafe {
        let Some(name) = appearance_name(dark, follows) else {
            send_id(window, "setAppearance:", std::ptr::null_mut());
            return;
        };
        let look = send_id(class("NSAppearance"), "appearanceNamed:", string(name));
        if !look.is_null() {
            send_id(window, "setAppearance:", look);
        }
    }
}

#[cfg(any(target_os = "macos", test))]
fn appearance_name(dark: bool, follows: bool) -> Option<&'static str> {
    if follows {
        None
    } else if dark {
        Some("NSAppearanceNameDarkAqua")
    } else {
        Some("NSAppearanceNameAqua")
    }
}

#[cfg(test)]
mod appearance_tests {
    use super::appearance_name;

    #[test]
    fn a_window_following_the_system_is_given_no_appearance_and_a_chosen_one_is_pinned() {
        assert_eq!(appearance_name(true, true), None);
        assert_eq!(appearance_name(false, true), None);
        assert_eq!(appearance_name(true, false), Some("NSAppearanceNameDarkAqua"));
        assert_eq!(appearance_name(false, false), Some("NSAppearanceNameAqua"));
    }
}

#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Full {
    Windowed,
    Entering,
    Whole,
    Leaving,
}

#[cfg(any(target_os = "macos", test))]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Notice {
    WillEnter,
    DidEnter,
    FailedToEnter,
    WillExit,
    DidExit,
    FailedToExit,
}

#[cfg(any(target_os = "macos", test))]
impl Notice {
    const ALL: [(Notice, &'static str, &'static str); 6] = [
        (Notice::WillEnter, "NSWindowWillEnterFullScreenNotification", "zikaronWillEnter:"),
        (Notice::DidEnter, "NSWindowDidEnterFullScreenNotification", "zikaronDidEnter:"),
        (Notice::FailedToEnter, "NSWindowDidFailToEnterFullScreenNotification", "zikaronFailedToEnter:"),
        (Notice::WillExit, "NSWindowWillExitFullScreenNotification", "zikaronWillExit:"),
        (Notice::DidExit, "NSWindowDidExitFullScreenNotification", "zikaronDidExit:"),
        (Notice::FailedToExit, "NSWindowDidFailToExitFullScreenNotification", "zikaronFailedToExit:"),
    ];

    fn then(self) -> Full {
        match self {
            Notice::WillEnter => Full::Entering,
            Notice::DidEnter | Notice::FailedToExit => Full::Whole,
            Notice::WillExit => Full::Leaving,
            Notice::DidExit | Notice::FailedToEnter => Full::Windowed,
        }
    }
}

#[cfg(any(target_os = "macos", test))]
fn toolbar_shows(f: Full) -> bool {
    f == Full::Windowed
}

#[cfg(target_os = "macos")]
unsafe fn on_notice(note: appkit::Id, n: Notice) {
    use appkit::*;
    unsafe {
        let window = send(note, "object");
        if window.is_null() {
            return;
        }
        let bar = send(window, "toolbar");
        if !bar.is_null() {
            send_bool(bar, "setVisible:", toolbar_shows(n.then()));
        }
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" fn will_enter(_: appkit::Id, _: appkit::Id, note: appkit::Id) {
    unsafe { on_notice(note, Notice::WillEnter) }
}
#[cfg(target_os = "macos")]
unsafe extern "C" fn did_enter(_: appkit::Id, _: appkit::Id, note: appkit::Id) {
    unsafe { on_notice(note, Notice::DidEnter) }
}
#[cfg(target_os = "macos")]
unsafe extern "C" fn failed_to_enter(_: appkit::Id, _: appkit::Id, note: appkit::Id) {
    unsafe { on_notice(note, Notice::FailedToEnter) }
}
#[cfg(target_os = "macos")]
unsafe extern "C" fn will_exit(_: appkit::Id, _: appkit::Id, note: appkit::Id) {
    unsafe { on_notice(note, Notice::WillExit) }
}
#[cfg(target_os = "macos")]
unsafe extern "C" fn did_exit(_: appkit::Id, _: appkit::Id, note: appkit::Id) {
    unsafe { on_notice(note, Notice::DidExit) }
}
#[cfg(target_os = "macos")]
unsafe extern "C" fn failed_to_exit(_: appkit::Id, _: appkit::Id, note: appkit::Id) {
    unsafe { on_notice(note, Notice::FailedToExit) }
}

#[cfg(target_os = "macos")]
fn imp_of(n: Notice) -> unsafe extern "C" fn(appkit::Id, appkit::Id, appkit::Id) {
    match n {
        Notice::WillEnter => will_enter,
        Notice::DidEnter => did_enter,
        Notice::FailedToEnter => failed_to_enter,
        Notice::WillExit => will_exit,
        Notice::DidExit => did_exit,
        Notice::FailedToExit => failed_to_exit,
    }
}

#[cfg(target_os = "macos")]
unsafe fn watch_full_screen(window: appkit::Id) {
    use appkit::*;
    static CLASS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    let cls = *CLASS.get_or_init(|| {
        let methods: Vec<(&str, unsafe extern "C" fn(Id, Id, Id))> = Notice::ALL.iter().map(|(n, _, m)| (*m, imp_of(*n))).collect();
        unsafe { make_class("ZikaronFullScreenWatch", "NSObject", &methods) }.unwrap_or_else(|| class("ZikaronFullScreenWatch")) as usize
    }) as Id;
    if cls.is_null() {
        return;
    }
    unsafe {
        let watcher = send(send(cls, "alloc"), "init");
        if watcher.is_null() {
            return;
        }
        let center = send(class("NSNotificationCenter"), "defaultCenter");
        for (_, name, method) in Notice::ALL {
            observe(center, watcher, method, string(name), window);
        }
        let full = send_uint_ret(window, "styleMask") & (1 << 14) != 0;
        let bar = send(window, "toolbar");
        if !bar.is_null() {
            send_bool(bar, "setVisible:", toolbar_shows(if full { Full::Whole } else { Full::Windowed }));
        }
    }
}

#[cfg(test)]
mod toolbar_tests {
    use super::{toolbar_shows, Full, Notice};

    #[test]
    fn the_toolbar_shows_on_the_desktop_alone() {
        let shown: Vec<(Notice, bool)> = Notice::ALL.iter().map(|(n, _, _)| (*n, toolbar_shows(n.then()))).collect();
        assert_eq!(
            shown,
            vec![
                (Notice::WillEnter, false),
                (Notice::DidEnter, false),
                (Notice::FailedToEnter, true),
                (Notice::WillExit, false),
                (Notice::DidExit, true),
                (Notice::FailedToExit, false),
            ]
        );
        assert!(toolbar_shows(Full::Windowed));
        assert!(![Full::Entering, Full::Whole, Full::Leaving].into_iter().any(toolbar_shows));
    }

    #[test]
    fn each_notice_is_named_once_with_a_method_of_its_own() {
        let names: std::collections::BTreeSet<&str> = Notice::ALL.iter().map(|(_, n, _)| *n).collect();
        let methods: std::collections::BTreeSet<&str> = Notice::ALL.iter().map(|(_, _, m)| *m).collect();
        assert_eq!((names.len(), methods.len()), (6, 6));
        assert!(names.iter().all(|n| n.starts_with("NSWindow") && n.ends_with("FullScreenNotification")));
        assert!(methods.iter().all(|m| m.ends_with(':')));
    }
}

#[cfg(target_os = "macos")]
pub fn backdrop(w: crate::Window, dark: bool, follows: bool) -> crate::Backdrop {
    use appkit::*;
    let crate::Window::AppKit(view) = w else { return crate::Backdrop::Tint };
    if view.is_null() {
        return crate::Backdrop::Tint;
    }
    unsafe {
        let window = send(view, "window");
        if window.is_null() {
            return crate::Backdrop::Tint;
        }
        let content = send(window, "contentView");
        let frame_view = if content.is_null() { std::ptr::null_mut() } else { send(content, "superview") };
        if frame_view.is_null() {
            return crate::Backdrop::Tint;
        }
        set_appearance(window, dark, follows);
        let glass = class("NSGlassEffectView");
        let (cls, kind) = if glass.is_null() { (class("NSVisualEffectView"), crate::Backdrop::Vibrancy) } else { (glass, crate::Backdrop::Glass) };
        if cls.is_null() {
            return crate::Backdrop::Tint;
        }
        let bounds = send_rect(frame_view, "bounds");
        let effect = init_frame(send(cls, "alloc"), bounds);
        if effect.is_null() {
            return crate::Backdrop::Tint;
        }
        send_int(effect, "setAutoresizingMask:", 2 | 16);
        if kind == crate::Backdrop::Vibrancy {
            send_int(effect, "setMaterial:", 21);
            send_int(effect, "setBlendingMode:", 0);
            send_int(effect, "setState:", 1);
        }
        send_bool(window, "setOpaque:", false);
        send_id(window, "setBackgroundColor:", send(class("NSColor"), "clearColor"));
        if kind == crate::Backdrop::Glass {
            let bar = send_id(send(class("NSToolbar"), "alloc"), "initWithIdentifier:", string("zikaron-window"));
            if !bar.is_null() {
                send_id(window, "setToolbar:", bar);
                send_int(window, "setToolbarStyle:", 3);
                watch_full_screen(window);
            }
        }
        add_subview(frame_view, effect, -1, content);
        kind
    }
}

#[cfg(target_os = "macos")]
pub fn reduce_motion() -> bool {
    use appkit::*;
    unsafe {
        let ws = send(class("NSWorkspace"), "sharedWorkspace");
        if ws.is_null() {
            return false;
        }
        send_bool_ret(ws, "accessibilityDisplayShouldReduceMotion")
    }
}

#[cfg(not(target_os = "macos"))]
pub fn reduce_motion() -> bool {
    static ONCE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ONCE.get_or_init(|| {
        std::process::Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "enable-animations"])
            .output()
            .map(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "false")
            .unwrap_or(false)
    })
}

#[cfg(target_os = "macos")]
pub fn backdrop_appearance(w: crate::Window, dark: bool, follows: bool) {
    use appkit::*;
    let crate::Window::AppKit(view) = w else { return };
    if view.is_null() {
        return;
    }
    unsafe {
        let window = send(view, "window");
        if !window.is_null() {
            set_appearance(window, dark, follows);
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn backdrop_available() -> bool {
    false
}

#[cfg(not(target_os = "macos"))]
pub fn backdrop(_w: crate::Window, _dark: bool, _follows: bool) -> crate::Backdrop {
    crate::Backdrop::Tint
}

#[cfg(not(target_os = "macos"))]
pub fn backdrop_appearance(_w: crate::Window, _dark: bool, _follows: bool) {}
