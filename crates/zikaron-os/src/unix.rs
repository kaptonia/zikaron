//! The unix implementation (macOS, Linux).

use std::io::Read;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

pub const ENTROPY_SOURCE: &str = "/dev/urandom";

pub fn fill_random(buf: &mut [u8]) -> std::io::Result<()> {
    // Read exactly the requested length: the source never ends, so reading to end would hang.
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

/// A unix directory syncs itself; `landed` is not needed.
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

/// macOS: `renamex_np` with `RENAME_EXCL`.
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

/// Linux: `renameat2` with `RENAME_NOREPLACE`.
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

/// The variable that names the user's home.
pub const HOME_VAR: &str = "HOME";

/// The user's home beyond [`HOME_VAR`]: none (unix names the home only there).
pub fn home_dir() -> Option<std::path::PathBuf> {
    None
}

/// The home directory itself (the default machine directory is a dot-folder there).
pub fn app_data_dir(user_home: &Path) -> std::path::PathBuf {
    user_home.to_path_buf()
}

// ───────────────────────── IPC endpoint: a local socket file ─────────────────────────

use std::os::unix::net::{UnixListener, UnixStream};

/// Maximum local socket path length in bytes (the OS `sun_path` size minus the terminating NUL).
#[cfg(target_os = "macos")]
pub const DOOR_PATH_MAX: usize = 103;
#[cfg(not(target_os = "macos"))]
pub const DOOR_PATH_MAX: usize = 107;

/// On unix the endpoint is a socket file at this path.
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

pub struct DoorListener(UnixListener);
pub struct DoorStream(UnixStream);

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

/// Linux: the peer's credentials as the kernel recorded them when it connected (`SO_PEERCRED`).
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

/// Check that the peer runs as this process's user, per the kernel; anyone else is refused (`PermissionDenied`).
fn same_user(s: &UnixStream) -> std::io::Result<()> {
    // SAFETY: `getuid` has no failure and no arguments.
    let me = unsafe { getuid() };
    match peer_uid(s)? {
        u if u == me => Ok(()),
        u => Err(std::io::Error::new(std::io::ErrorKind::PermissionDenied, format!("the other end is user {u}, not this user"))),
    }
}

/// Open an endpoint at `place`. A socket file nobody answers was left by a dead process and is removed first.
/// One that answers is live (`AddrInUse`); anything else there is never removed (`AddrInUse`). The socket file
/// is owner-only.
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
    Ok(DoorListener(l))
}

impl DoorListener {
    /// The next client; its user is not yet checked ([`DoorStream::same_user`]).
    pub fn accept(&self) -> std::io::Result<DoorStream> {
        let (s, _) = self.0.accept()?;
        Ok(DoorStream(s))
    }
}

/// Close the endpoint: wake a waiting [`DoorListener::accept`], then remove the socket file, after which a
/// connect finds nothing (`NotFound`), whether or not the listening socket is still open.
pub fn door_close(place: &Path, _listener: Option<&DoorListener>) {
    door_knock(place);
    door_remove(place);
}

/// Connect and disconnect at once, to wake a [`DoorListener::accept`] that is waiting.
fn door_knock(place: &Path) {
    let _ = UnixStream::connect(place);
}

/// Remove the endpoint's socket file (only if it is a socket).
fn door_remove(place: &Path) {
    use std::os::unix::fs::FileTypeExt;
    if std::fs::symlink_metadata(place).map(|m| m.file_type().is_socket()).unwrap_or(false) {
        let _ = std::fs::remove_file(place);
    }
}

/// Connect to the endpoint at `place` without checking its user ([`DoorStream::same_user`]); none there, or a
/// stale one, is `NotFound`.
pub fn door_connect(place: &Path) -> std::io::Result<DoorStream> {
    if too_long(place) {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("longer than a local socket's {DOOR_PATH_MAX} bytes")));
    }
    let s = match UnixStream::connect(place) {
        Ok(s) => s,
        Err(e) if matches!(e.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused) => return Err(not_open(&e.to_string())),
        Err(e) => return Err(e),
    };
    Ok(DoorStream(s))
}

impl DoorStream {
    /// Whether the other end runs as this process's user, per the kernel; anyone else is refused
    /// (`PermissionDenied`).
    pub fn same_user(&self) -> std::io::Result<()> {
        same_user(&self.0)
    }

    pub fn set_read_deadline(&self, d: Option<std::time::Duration>) -> std::io::Result<()> {
        self.0.set_read_timeout(d)
    }
}

impl std::io::Read for DoorStream {
    /// A read past its deadline returns `TimedOut` (some unix systems report `WouldBlock`).
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf).map_err(|e| if e.kind() == std::io::ErrorKind::WouldBlock { std::io::Error::new(std::io::ErrorKind::TimedOut, e.to_string()) } else { e })
    }
}

impl std::io::Write for DoorStream {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        std::io::Write::write(&mut self.0, buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        std::io::Write::flush(&mut self.0)
    }
}

// ───────────────────────── "Enable command line": a link on the command path ─────────────────────────

/// Where the link goes: macOS `/usr/local/bin` (created if missing, with administrator rights when needed);
/// Linux the user's own `~/.local/bin`.
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

// ───────────────────────── Children and lock descriptors ─────────────────────────

unsafe extern "C" {
    fn close(fd: i32) -> i32;
}

/// A lock file's descriptor, as listed (`crate::LISTED`).
pub fn listed_id(file: &std::fs::File) -> i64 {
    use std::os::fd::AsRawFd;
    i64::from(file.as_raw_fd())
}

/// Start the child; between fork and exec it closes every listed lock descriptor, so it never holds a copy
/// when its program runs. Descriptors 0-2 are skipped: by then they are the child's own standard streams. With
/// a `pre_exec` closure the standard library waits for the exec, so this returns only after the close.
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

/// A CLI running from a location that will disappear: a read-only volume (an app opened inside its disk
/// image), or an AppImage (mounted afresh at every start).
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

/// The CLI's folder is on this process's command path via a folder other than the switch's link folder (a
/// package installed it). The link folder itself is on the path by design, so being there counts as the
/// switch's, not a package's.
pub fn cli_provided(cli: &Path, _moved: bool) -> bool {
    let Some(dir) = cli.parent() else { return false };
    let Some(path) = std::env::var_os("PATH") else { return false };
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let own = crate::cli_path::place_dir().map(|p| real(&p));
    std::env::split_paths(&path).any(|p| real(&p) == real(dir) && Some(real(&p)) != own)
}

/// The state of the link location, for the CLI at `cli`.
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
        // A dangling link whose target has the CLI's name: the app has moved; the switch may recreate it.
        None if to.file_name().and_then(|n| n.to_str()) == Some(NAME) => State::Off,
        None => State::Taken(to.display().to_string()),
    }
}

fn denied(e: &std::io::Error) -> bool {
    matches!(e.kind(), std::io::ErrorKind::PermissionDenied) || e.raw_os_error() == Some(1)
}

/// Create the link (and its folder if missing), replacing a dangling link to a moved app. If macOS refuses in
/// its system folder, retry through the OS administrator dialog.
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

/// Remove the link this switch made (the caller checked it is this app's). If macOS refuses in its system
/// folder, retry through the OS administrator dialog, removing only a link.
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

/// Run one command as administrator through the OS's own dialog (the password is typed there, never seen
/// here). Paths are passed as arguments, quoted by the script, never spliced into the command. Cancellation
/// (`-128`) is reported as such; any other failure with the OS message.
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

/// Interpret the script's stderr when the command did not run: cancelled at the dialog (`-128`), or anything
/// else with its own message (three wrong passwords, not an administrator, the command itself failing).
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

    /// A folder reached by a link is the same folder (`same_folder`); a link to another folder is not.
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

    /// The dialog's two failure modes map to distinct refusals.
    #[test]
    fn the_administrator_dialog_is_read_by_what_it_said() {
        use crate::cli_path::Refused;
        assert_eq!(super::admin_refusal("execution error: User canceled. (-128)\n"), Refused::Cancelled);
        assert_eq!(super::admin_refusal("execution error: The administrator user name or password was incorrect. (-60007)"), Refused::NotAllowed("execution error: The administrator user name or password was incorrect. (-60007)".into()));
    }

    /// A file others can read or write is not owner-only, whatever the umask.
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

// Unix tests of `crate::cli_path`, kept here beside the unix interfaces they use.
#[cfg(test)]
mod cli_path_tests {
    use crate::cli_path::*;
    use std::path::{Path, PathBuf};
    use std::os::unix::fs::symlink;

    /// The redirected location is process-wide (an environment variable), so these tests run serially.
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

    /// Off with nothing there (folder not yet created); enabling creates the folder and the link to this CLI;
    /// disabling removes the link and keeps the folder; state is re-read each time.
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

    /// Anything else there is named and never overwritten or removed: another install's file, a link to another
    /// program, a link to another app's existing CLI.
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

    /// A dangling link this switch made (the app has moved) reads as off and is recreated; a dangling link whose
    /// target has another name belongs to someone else.
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

    /// An unwritable location is reported with the OS message and nothing changes; a missing CLI beside the app
    /// is reported as unsupported.
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

    /// An AppImage is mounted afresh at every start: unsupported, named, nothing created.
    #[test]
    fn an_appimage_is_not_supported() {
        let (b, _t) = bench("appimage");
        unsafe { std::env::set_var("APPIMAGE", "/tmp/Z.AppImage") };
        let s = (state(&b.cli), enable(&b.cli));
        unsafe { std::env::remove_var("APPIMAGE") };
        assert!(matches!(s.0, State::Unsupported(_)) && matches!(s.1, Err(Refused::Unsupported(_))), "{s:?}");
        assert!(std::fs::symlink_metadata(b.dir.join(NAME)).is_err());
    }

    /// A CLI whose folder is already on the command path (installed by a package) is "provided": nothing to
    /// switch either way.
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
