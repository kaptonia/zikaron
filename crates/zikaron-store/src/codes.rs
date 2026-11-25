#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Code {
    Conflict,
    TooLarge,
    Absent,
    Unreadable,
    NotAFile,
    BadName,
    NotADirectory,
    Unaccounted,
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

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Trouble {
    pub code: Code,
    pub names: Vec<String>,
    pub size: Option<usize>,
    pub cap: Option<usize>,
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
    pub fn io(what: &str, e: &std::io::Error) -> Self {
        let said = if what.is_empty() { e.to_string() } else { format!("{what}: {e}") };
        Trouble { code: Code::Io, names: Vec::new(), size: None, cap: None, said: Some(said) }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Why {
    ForeignName,
    InFlight,
    SystemSide,
    NotAFile,
    NonUtf8Name,
    TooLarge,
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
