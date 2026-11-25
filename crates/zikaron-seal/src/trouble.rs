#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Code {
    FileMissing,
    Denied,
    NotAFolder,
    Unknown,
    Locked,
    LockedOut,
    KeyboxMissing,
    KeyboxShape,
    KeyboxSlot,
    KeyboxLocked,
    KeyboxNotEmpty,
    KdfBelowFloor,
    KeystoreParams,
    KeyMalformed,
    PinSet,
    PinShape,
    PinWrong,
    RecoveryNoMatch,
    RecoveryNotPrimary,
    LocalSeal,
    Ledger,
    ContentShape,
    SettingsShape,
    QueueShape,
    MachineShape,
    IdentitiesShape,
    CredentialExpired,
    CredentialMissing,
    CredentialShape,
    HomeHeld,
    HomeOtherMachine,
    HandedOff,
    CredentialPassMissing,
    CredentialPassUnknown,
    CredentialNote,
    CredentialFull,
    CredentialRowGone,
}

impl Code {
    pub fn one_slot(self) -> bool {
        matches!(self, Code::KeyboxSlot | Code::KeyMalformed)
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Trouble {
    pub code: Code,
    pub tail: String,
}

impl Trouble {
    pub fn new(code: Code, tail: impl Into<String>) -> Trouble {
        Trouble { code, tail: tail.into() }
    }

    pub fn io(e: &std::io::Error, subject: &str) -> Trouble {
        let tail = format!("{subject}: {e}");
        match e.kind() {
            std::io::ErrorKind::NotFound if file_on_the_way(std::path::Path::new(subject)) => Trouble::new(Code::NotAFolder, tail),
            std::io::ErrorKind::NotFound => Trouble::new(Code::FileMissing, tail),
            std::io::ErrorKind::PermissionDenied => Trouble::new(Code::Denied, tail),
            std::io::ErrorKind::NotADirectory => Trouble::new(Code::NotAFolder, tail),
            _ => Trouble::new(Code::Unknown, tail),
        }
    }
}

pub fn file_on_the_way(path: &std::path::Path) -> bool {
    path.is_absolute() && path.ancestors().skip(1).any(|a| std::fs::metadata(a).map(|m| m.is_file()).unwrap_or(false))
}

pub trait Coded {
    fn sealed_code(&self) -> Option<Code>;
    fn sealed_tail(&self) -> &str;
}

impl Coded for Trouble {
    fn sealed_code(&self) -> Option<Code> {
        Some(self.code)
    }

    fn sealed_tail(&self) -> &str {
        &self.tail
    }
}

pub fn random(n: usize) -> Result<Vec<u8>, Trouble> {
    let mut b = vec![0u8; n];
    fill_random(&mut b)?;
    Ok(b)
}

pub fn fill_random(buf: &mut [u8]) -> Result<(), Trouble> {
    zikaron_os::fill_random(buf).map_err(|e| Trouble::io(&e, zikaron_os::ENTROPY_SOURCE))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_path_below_a_file_is_not_a_folder() {
        let base = std::env::temp_dir().join(format!("zk-trouble-way-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&base);
        let home = base.join("home");
        std::fs::write(&home, b"a plain file").unwrap();
        let below = home.join("settings").join("home-label.json");
        let missing = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert_eq!(Trouble::io(&missing, &below.display().to_string()).code, Code::NotAFolder);
        assert_eq!(Trouble::io(&missing, &base.join("gone").join("x").display().to_string()).code, Code::FileMissing);
        assert_eq!(Trouble::io(&missing, "a relative subject").code, Code::FileMissing);
        let _ = std::fs::remove_dir_all(&base);
    }
}
