//! Refusal codes: a closed set, each with one string spelling ([`Code::as_str`]). Callers branch on these
//! codes, so each is spelled in one place only.

/// Why a call was refused. Closed, so callers can match every case.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Code {
    /// The name is already on disk with different bytes: refused, nothing written.
    Conflict,
    /// Over the entry size cap: refused on write, refused and disclosed on read.
    TooLarge,
    /// The named file does not exist.
    Absent,
    /// The file exists but cannot be read (permissions, bad block, removed mid-read).
    Unreadable,
    /// Not a regular file where one is required (directory, symlink, device).
    NotAFile,
    /// The name does not have the entry shape (64 lowercase hex) or contains a path separator.
    BadName,
    /// The given path is not a directory; an archive must be one.
    NotADirectory,
    /// A strict read met files this crate cannot account for; they are named.
    Unaccounted,
    /// A disk operation failed (create, link, fsync, remove).
    Io,
}

impl Code {
    pub fn as_str(self) -> &'static str {
        match self {
            Code::Conflict => "E_CONFLICT",
            Code::TooLarge => "E_TOO_LARGE",
            Code::Absent => "E_ABSENT",
            Code::Unreadable => "E_UNREADABLE",
            Code::NotAFile => "E_NOT_A_FILE",
            Code::BadName => "E_BAD_NAME",
            Code::NotADirectory => "E_NOT_A_DIRECTORY",
            Code::Unaccounted => "E_UNACCOUNTED",
            Code::Io => "E_IO",
        }
    }
}

/// A refusal: the code plus what a user needs to act on it (which files, how large). A cap refusal always
/// carries `size` and `cap`; an unaccounted refusal always carries `names`.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Trouble {
    pub code: Code,
    /// Named files in byte order; empty when none apply.
    pub names: Vec<String>,
    /// Measured size when over the cap.
    pub size: Option<usize>,
    /// The cap when over it.
    pub cap: Option<usize>,
    /// The OS message when a disk operation failed ([`Code::Io`]): operation, path and OS text (`zikaron_os`
    /// names the operation and path). `None` for every other code.
    pub said: Option<String>,
}

impl Trouble {
    pub fn of(code: Code) -> Self {
        Trouble { code, names: Vec::new(), size: None, cap: None, said: None }
    }
    pub fn named(code: Code, name: impl Into<String>) -> Self {
        Trouble { code, names: vec![name.into()], size: None, cap: None, said: None }
    }
    pub fn too_large(name: impl Into<String>, size: usize, cap: usize) -> Self {
        Trouble { code: Code::TooLarge, names: vec![name.into()], size: Some(size), cap: Some(cap), said: None }
    }
    /// A disk operation failed: [`Code::Io`] with the OS message. `what` names the operation and path when the
    /// error does not already (errors from `zikaron_os` do).
    pub fn io(what: &str, e: &std::io::Error) -> Self {
        let said = if what.is_empty() { e.to_string() } else { format!("{what}: {e}") };
        Trouble { code: Code::Io, names: Vec::new(), size: None, cap: None, said: Some(said) }
    }
}

/// Why a lenient read skipped something. Closed set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Why {
    /// Neither an entry name nor this crate's temporary-file name.
    ForeignName,
    /// This crate's temporary-file name: a write still in progress or interrupted.
    InFlight,
    /// A file the OS writes beside this crate's names (`layout::SYSTEM_SIDE`).
    SystemSide,
    /// Not a regular file.
    NotAFile,
    /// Not UTF-8; every name this crate writes is UTF-8, so this one is foreign.
    NonUtf8Name,
    /// Over the entry size cap.
    TooLarge,
    /// Unreadable.
    Unreadable,
}

impl Why {
    pub fn as_str(self) -> &'static str {
        match self {
            Why::ForeignName => "FOREIGN_NAME",
            Why::InFlight => "IN_FLIGHT",
            Why::SystemSide => "SYSTEM_SIDE",
            Why::NotAFile => "NOT_A_FILE",
            Why::NonUtf8Name => "NON_UTF8_NAME",
            Why::TooLarge => "TOO_LARGE",
            Why::Unreadable => "UNREADABLE",
        }
    }
}
