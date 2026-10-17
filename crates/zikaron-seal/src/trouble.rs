//! Why the sealed layer refused: a closed code and its evidence. No sentence for people lives here; each end
//! (the desktop's fault table, the command line's reasons) says a code in its own words.

/// The refusals of the sealed layer. Closed: each end maps every member, so a new one does not compile until it
/// is said.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Code {
    /// A file or directory that is not there.
    FileMissing,
    /// The system refused access.
    Denied,
    /// Any other system error (its message is the evidence).
    Unknown,
    /// The vault is locked: no master key in hand.
    Locked,
    /// All passcode tries are used: only recovery remains.
    LockedOut,
    /// No vault, or no passcode seal in it.
    KeyboxMissing,
    /// The vault file is not in the shape this version writes.
    KeyboxShape,
    /// A seal whose binding or account does not match.
    KeyboxSlot,
    /// The vault lock could not be taken.
    KeyboxLocked,
    /// The vault holds seals or slots, so it is not reset or kept as fresh.
    KeyboxNotEmpty,
    /// The vault records derivation parameters below this binary's floor.
    KdfBelowFloor,
    /// Derivation parameters scrypt cannot run.
    KeystoreParams,
    /// A key or entropy of the wrong shape.
    KeyMalformed,
    /// The vault already has a passcode.
    PinSet,
    /// A passcode that breaks the rules (the rule in the evidence).
    PinShape,
    /// A wrong passcode (the tries left in the evidence).
    PinWrong,
    /// A recovery secret that opens no seal.
    RecoveryNoMatch,
    /// A recovery secret of an identity that is not the primary.
    RecoveryNotPrimary,
    /// A sealed file that cannot be read (its reason first in the evidence, `envelope::Unread`).
    LocalSeal,
    /// The ledger directory refused (the store's trouble in the evidence).
    Ledger,
    /// Content of an entry kind that is not canonical.
    ContentShape,
    /// A settings-like file that is not canonical, or not in its shape.
    SettingsShape,
    /// The anchor queue file is not in its shape.
    QueueShape,
    /// The machine settings are not in their shape.
    MachineShape,
    /// The identity register is not in its shape.
    IdentitiesShape,
    /// A command-line credential that has expired (deleted on the way out).
    CredentialExpired,
    /// No command-line credential on this machine (the switch is off, or it was removed).
    CredentialMissing,
    /// A credential file not in its shape, or one that does not open.
    CredentialShape,
    /// The desktop holds this home's writer lock.
    HomeHeld,
    /// The home's writer mark names another machine, or does not read: no license to write it here.
    HomeOtherMachine,
    /// This ledger was handed off to another place (the handoff's id in the evidence) and not taken back: it is
    /// not written here.
    HandedOff,
}

/// One refusal: its code and its evidence (paths, members, numbers; never a sentence).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Trouble {
    pub code: Code,
    pub tail: String,
}

impl Trouble {
    pub fn new(code: Code, tail: impl Into<String>) -> Trouble {
        Trouble { code, tail: tail.into() }
    }

    /// A system error said with `subject` (a path, a source): missing and denied by their codes, any other as
    /// unknown with the system's message.
    pub fn io(e: &std::io::Error, subject: &str) -> Trouble {
        let tail = format!("{subject}: {e}");
        match e.kind() {
            std::io::ErrorKind::NotFound => Trouble::new(Code::FileMissing, tail),
            std::io::ErrorKind::PermissionDenied => Trouble::new(Code::Denied, tail),
            _ => Trouble::new(Code::Unknown, tail),
        }
    }
}

/// What every end's error says about itself, so the sealed layer can read back its own refusals from either
/// (`envelope::Unread::of`).
pub trait Coded {
    /// The sealed layer's code this error carries, when it carries one.
    fn sealed_code(&self) -> Option<Code>;
    /// Its evidence.
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

/// `n` bytes from the system entropy source (`zikaron_os::fill_random`), the failure said with the source.
pub fn random(n: usize) -> Result<Vec<u8>, Trouble> {
    let mut b = vec![0u8; n];
    fill_random(&mut b)?;
    Ok(b)
}

/// Fill `buf` from the system entropy source.
pub fn fill_random(buf: &mut [u8]) -> Result<(), Trouble> {
    zikaron_os::fill_random(buf).map_err(|e| Trouble::io(&e, zikaron_os::ENTROPY_SOURCE))
}
