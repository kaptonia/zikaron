use std::path::{Path, PathBuf};

pub const NAME: &str = "zikaron";

pub const ENV_DIR: &str = "ZIKARON_CLI_LINK_DIR";

pub const PATH_STAND_IN: &str = "user-path";

pub const MACHINE_PATH_STAND_IN: &str = "machine-path";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    On,
    Off,
    Taken(String),
    Provided,
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    Taken(String),
    Cancelled,
    NotAllowed(String),
    Unsupported(String),
    TooLong(usize),
}

pub fn beside_this_program() -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?;
    Some(me.parent()?.join(format!("{NAME}{}", std::env::consts::EXE_SUFFIX)))
}

pub fn place_dir() -> Option<PathBuf> {
    match std::env::var_os(ENV_DIR) {
        Some(d) if !d.is_empty() => Some(PathBuf::from(d)),
        _ => crate::imp::cli_default_dir(),
    }
}

fn moved() -> bool {
    std::env::var_os(ENV_DIR).is_some_and(|d| !d.is_empty())
}

pub fn state(cli: &Path) -> State {
    if let Some(why) = unsupported(cli) {
        return State::Unsupported(why);
    }
    if provided(cli) {
        return State::Provided;
    }
    crate::imp::cli_state(cli, moved())
}

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

fn unsupported(cli: &Path) -> Option<String> {
    if !cli.is_file() {
        return Some(format!("no command line at {}", cli.display()));
    }
    crate::imp::cli_unsupported(cli)
}

fn provided(cli: &Path) -> bool {
    crate::imp::cli_provided(cli, moved())
}
