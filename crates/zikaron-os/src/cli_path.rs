//! "Enable command line": put the CLI that ships beside the app on the shell's command path so typing `zikaron`
//! runs it, and remove it again. One implementation per OS: creating and removing the single item this switch
//! places, requesting administrator rights when needed (only through the OS's own dialog, so the app never sees a
//! password), and reading the current state.
//!
//! - **macOS, Linux**: a symlink named `zikaron` in a folder on the command path (macOS `/usr/local/bin`, created
//!   if missing; Linux `~/.local/bin`) pointing at the CLI beside the app. A CLI already on the path where it
//!   ships (a Linux package installs it in a system folder) is "provided by the package"; one running from inside
//!   a disk image or AppImage would leave a dangling link once that is gone, so it is refused by name.
//! - **Windows**: the CLI's folder is added to this user's `Path` (no administrator; newly opened terminals see
//!   it).
//!
//! State is re-read every time ([`state`]): a link pointing at this app's CLI is on; nothing there is off; a link
//! this switch made to an app that has since moved (dangling) is off and may be recreated; anything else there
//! (another install's copy, a link to another program, a plain file) belongs to someone else, is named, and is
//! never overwritten or removed. Turning off removes only what this switch made.
//!
//! Tests and benchmarks redirect the location to their own folder with [`ENV_DIR`], so system folders are never
//! touched and administrator rights are never requested.

use std::path::{Path, PathBuf};

/// The command's name on the path.
pub const NAME: &str = "zikaron";

/// Redirects where this switch writes (tests, benchmarks): on unix the folder the link goes in, on Windows a
/// folder holding a stand-in for the user's `Path` (a file named [`PATH_STAND_IN`]).
pub const ENV_DIR: &str = "ZIKARON_CLI_LINK_DIR";

/// On Windows under [`ENV_DIR`]: the file standing in for the user's `Path`.
pub const PATH_STAND_IN: &str = "user-path";

/// On Windows under [`ENV_DIR`]: the file standing in for this machine's `Path` (what an installer writes).
pub const MACHINE_PATH_STAND_IN: &str = "machine-path";

/// The current state. Closed set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// This app's CLI is on the path through this switch.
    On,
    /// Nothing from this switch is there (a dangling link it made counts as nothing).
    Off,
    /// Something else is there: what it is, as reported by the OS (a path, a link target).
    Taken(String),
    /// The CLI is already on the path where it ships (installed by a system package): nothing to switch.
    Provided,
    /// The way the app is running cannot be put on the path, with the reason.
    Unsupported(String),
}

/// Why enabling or disabling failed. Closed set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Something else is there (never overwritten or removed).
    Taken(String),
    /// The user cancelled the OS administrator dialog.
    Cancelled,
    /// The OS refused (no permission, a wrong password in its dialog, an unwritable location), with its message.
    NotAllowed(String),
    /// The way the app is running cannot be put on the path (see [`State::Unsupported`]).
    Unsupported(String),
    /// The user's `Path` would exceed the OS length limit (Windows).
    TooLong(usize),
}

/// The CLI that ships beside this program (same folder, with this OS's executable suffix).
pub fn beside_this_program() -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?;
    Some(me.parent()?.join(format!("{NAME}{}", std::env::consts::EXE_SUFFIX)))
}

/// Where this switch writes: [`ENV_DIR`] when set, else the OS default.
pub fn place_dir() -> Option<PathBuf> {
    match std::env::var_os(ENV_DIR) {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => crate::imp::cli_default_dir(),
    }
}

/// Whether the location is redirected for a test or benchmark (administrator rights are then never requested).
fn moved() -> bool {
    std::env::var_os(ENV_DIR).is_some_and(|d| !d.is_empty())
}

/// The current state for the CLI at `cli` (the one beside the app).
pub fn state(cli: &Path) -> State {
    if let Some(why) = unsupported(cli) {
        return State::Unsupported(why);
    }
    if provided(cli) {
        return State::Provided;
    }
    crate::imp::cli_state(cli, moved())
}

/// Put the CLI at `cli` on the path. If something else is there, refuse by name and leave it untouched.
pub fn enable(cli: &Path) -> Result<State, Refused> {
    match state(cli) {
        State::On | State::Provided => Ok(state(cli)),
        State::Unsupported(why) => Err(Refused::Unsupported(why)),
        State::Taken(what) => Err(Refused::Taken(what)),
        State::Off => {
            crate::imp::cli_enable(cli, moved())?;
            Ok(state(cli))
        }
    }
}

/// Take the CLI at `cli` off the path, removing only what this switch made.
pub fn disable(cli: &Path) -> Result<State, Refused> {
    match state(cli) {
        State::Off | State::Provided => Ok(state(cli)),
        State::Unsupported(why) => Err(Refused::Unsupported(why)),
        State::Taken(what) => Err(Refused::Taken(what)),
        State::On => {
            crate::imp::cli_disable(cli, moved())?;
            Ok(state(cli))
        }
    }
}

/// The CLI is missing, or runs from a location that will disappear (a disk image, an AppImage).
fn unsupported(cli: &Path) -> Option<String> {
    if !cli.is_file() {
        return Some(format!("no command line at {}", cli.display()));
    }
    crate::imp::cli_unsupported(cli)
}

/// The CLI's folder is already on the command path by other means (a package installed it). Each OS answers
/// via [`crate::imp`]'s `cli_provided`; what this switch writes is never read back as "provided", or an enabled
/// switch could never be disabled.
fn provided(cli: &Path) -> bool {
    crate::imp::cli_provided(cli, moved())
}
