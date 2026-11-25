#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Known {
    FontMissing,
    FileMissing,
    Denied,
    NotAFolder,
    KeychainMissing,
    Entropy,
    KeyMalformed,
    SignFailed,
    KeystoreParams,
    KeystoreShape,
    BadPassword,
    Ledger,
    NoHomeDir,
    SettingsShape,
    AddressShape,
    AddressMismatch,
    Occupied,
    CopyMismatch,
    Landing,
    ReadOnly,
    MirrorShape,
    MirrorEntry,
    MirrorOther,
    Locked,
    LockedOut,
    PinWrong,
    PinShape,
    PinSet,
    KeyboxMissing,
    KeyboxShape,
    KeyboxSlot,
    KeyboxNotEmpty,
    KeyboxLocked,
    KdfBelowFloor,
    MachineShape,
    InsufficientFunds,
    NonceUsed,
    AlreadyPending,
    Underpriced,
    GasTooLow,
    ContractRefused,
    RateLimited,
    MethodMissing,
    NodeAuth,
    ServiceUnserved,
    ServiceKeyMissing,
    WrongChain,
    NodeRefused,
    NodeTls,
    ProxyUnreachable,
    ProxyCredentialsMissing,
    ProxyCredentialsRefused,
    NodeTimeout,
    AnswerTooLong,
    AnswerNotJson,
    SeatUnseated,
    SeatDomain,
    BackupNotLanded,
    RecoveryNoMatch,
    PathRelative,
    NoEndpoint,
    Unreachable,
    Disagree,
    ChainShape,
    LedgerNotFetched,
    NewerElsewhere,
    EntryRefused,
    NoGenesis,
    ForkedRoot,
    AuditInput,
    NotAdoptable,
    AlreadyRooted,
    RestorePartial,
    CannotLay,
    NotAHome,
    NoHome,
    KeyNotStored,
    GitShape,
    NotARepo,
    RefMissing,
    ObjectMissing,
    Broken,
    WouldBreak,
    ContentShape,
    QueueShape,
    QueueEmpty,
    NoRegistry,
    NoChainId,
    GasRefused,
    GasNotShown,
    SendFailed,
    NotAudited,
    CleanWord,
    SeatUnhomed,
    AuditBehind,
    SubjectMissing,
    DirEmpty,
    ScanRefused,
    FieldMissing,
    KitRefused,
    Conflict,
    HandedOver,
    NoBytes,
    WorkerPanicked,
    OutcomeLost,
    NotHisBook,
    BookNotWhole,
    NotThisLedger,
    NotAGrant,
    AlreadyHeld,
    GrantNotHeld,
    GrantDirEmpty,
    AlreadyAnchored,
    PayloadRefused,
    ChainIncomplete,
    IdentityExists,
    IdentityHereAs,
    IdentityNameTaken,
    PlaceOneLine,
    NoIdentity,
    PhraseWords,
    PhraseInvalid,
    PhraseConfirm,
    NoWords,
    DeleteUnbacked,
    PasswordsDiffer,
    PasswordShort,
    PasswordLong,
    IdentitiesShape,
    PublishNoKit,
    TermsMismatch,
    GrantFileKit,
    GrantFileBad,
    KitFileBad,
    KitFormWord,
    KitOriginalsWord,
    ExtractKit,
    ExtractPath,
    ExtractNothing,
    ExtractClash,
    ExtractName,
    ExtractNotFolder,
    ExtractExists,
    ExtractLink,
    ExtractChanged,
    ExtractLanding,
    AddressedNotRecipient,
    AddressedBroken,
    AddressedHead,
    AddressedElsewhere,
    AddressedKeyShape,
    AddressedBoth,
    AddressedFolder,
    AddressNotAnchored,
    AddressChainUnread,
    AddressKeyUnproven,
    RemoteKit,
    RemoteStatus,
    RemoteRedirect,
    RemoteTooLarge,
    RemoteTimeout,
    RemoteUnreachable,
    RemoteCert,
    RemoteNotHttps,
    NotAnOriginal,
    TraceFull,
    LocalSeal,
    RecoveryNotPrimary,
    PrimaryDelete,
    PrimaryNoKeyFile,
    PrimaryAlready,
    BackupPassword,
    BackupNotOurs,
    BackupTooNew,
    BackupShape,
    Rekeying,
    BackupNoIdentity,
    HomeUnreachable,
    BusyForRekey,
    DialogUnavailable,
    LocalSetAside,
    EntriesUnreadable,
    NetworkListed,
    InsideHome,
    DoorShut,
    DoorUnread,
    InFlight,
    CliPathTaken,
    CliPathCancelled,
    CliPathNotAllowed,
    CliPathUnsupported,
    BatchVoided,
    CredentialExpired,
    CredentialMissing,
    CredentialShape,
    HomeHeld,
    MachineHeld,
    CliAloneOn,
    CliAloneDropped,
    HomeOtherMachine,
    CliWriting,
    ReadsUnkept,
    HandedOff,
    OnChain,
    TooEarly,
    UpgradeHanded,
    NotMoved,
    NotATarget,
    NotIncluded,
    BookCode,
    RotateOwed,
    RotateBookless,
    OfficeNotInWords,
    ChainInBasis,
    NoNoticeVenue,
    NoticeContract,
    RootNotInBook,
    NotAcceded,
    OfficeElsewhere,
    OfficeIsContract,
    RootKeyAbsent,
    FaceUnread,
    FaceNoHome,
    FaceRotating,
    FaceUnwritable,
    ConsentRefused,
    FaceRow,
    FaceHomeChain,
    RegistryCode,
    RotateUnseated,
    RotateUnentered,
    BookClosed,
    CredentialPassMissing,
    CredentialPassUnknown,
    CredentialNote,
    CredentialFull,
    CredentialRowGone,
    CredentialReclaimed,
    FaceSealed,
}

impl Known {
    pub fn of_str(code: &str) -> Option<Known> {
        Known::ALL.iter().copied().find(|k| k.as_str() == code)
    }

    pub const NETWORK: [Known; 19] = [
        Known::Unreachable,
        Known::NoEndpoint,
        Known::Disagree,
        Known::ScanRefused,
        Known::ChainShape,
        Known::NodeTls,
        Known::ProxyUnreachable,
        Known::ProxyCredentialsMissing,
        Known::ProxyCredentialsRefused,
        Known::NodeTimeout,
        Known::AnswerTooLong,
        Known::AnswerNotJson,
        Known::RateLimited,
        Known::MethodMissing,
        Known::NodeAuth,
        Known::ServiceUnserved,
        Known::ServiceKeyMissing,
        Known::WrongChain,
        Known::NodeRefused,
    ];

    pub fn unanswered(self) -> bool {
        Known::NETWORK.contains(&self) || self.addressed().is_some_and(zikaron_seal::addressed::Why::unanswered)
    }

    pub fn addressed(self) -> Option<zikaron_seal::addressed::Why> {
        use zikaron_seal::addressed::Why;
        Some(match self {
            Known::AddressedNotRecipient => Why::NotRecipient,
            Known::AddressedBroken => Why::Broken,
            Known::AddressedHead => Why::Head,
            Known::AddressedElsewhere => Why::Elsewhere,
            Known::AddressedKeyShape => Why::KeyShape,
            Known::AddressedBoth => Why::Both,
            Known::AddressedFolder => Why::Folder,
            Known::AddressNotAnchored => Why::NotAnchored,
            Known::AddressChainUnread => Why::ChainUnread,
            Known::AddressKeyUnproven => Why::Unproven,
            _ => return None,
        })
    }

    pub fn of_addressed(why: zikaron_seal::addressed::Why) -> Known {
        use zikaron_seal::addressed::Why;
        match why {
            Why::NotRecipient => Known::AddressedNotRecipient,
            Why::Broken => Known::AddressedBroken,
            Why::Head => Known::AddressedHead,
            Why::Elsewhere => Known::AddressedElsewhere,
            Why::KeyShape => Known::AddressedKeyShape,
            Why::Both => Known::AddressedBoth,
            Why::Folder => Known::AddressedFolder,
            Why::NotAnchored => Known::AddressNotAnchored,
            Why::ChainUnread => Known::AddressChainUnread,
            Why::Unproven => Known::AddressKeyUnproven,
        }
    }

    pub const ALL: [Known; 239] = [
        Known::FontMissing,
        Known::FileMissing,
        Known::Denied,
        Known::NotAFolder,
        Known::KeychainMissing,
        Known::Entropy,
        Known::KeyMalformed,
        Known::SignFailed,
        Known::KeystoreParams,
        Known::KeystoreShape,
        Known::BadPassword,
        Known::Ledger,
        Known::NoHomeDir,
        Known::SettingsShape,
        Known::AddressShape,
        Known::AddressMismatch,
        Known::Occupied,
        Known::CopyMismatch,
        Known::Landing,
        Known::ReadOnly,
        Known::MirrorShape,
        Known::MirrorEntry,
        Known::MirrorOther,
        Known::Locked,
        Known::LockedOut,
        Known::PinWrong,
        Known::PinShape,
        Known::PinSet,
        Known::KeyboxMissing,
        Known::KeyboxShape,
        Known::KeyboxSlot,
        Known::KeyboxNotEmpty,
        Known::KeyboxLocked,
        Known::KdfBelowFloor,
        Known::MachineShape,
        Known::InsufficientFunds,
        Known::NonceUsed,
        Known::AlreadyPending,
        Known::Underpriced,
        Known::GasTooLow,
        Known::ContractRefused,
        Known::RateLimited,
        Known::MethodMissing,
        Known::NodeAuth,
        Known::ServiceUnserved,
        Known::ServiceKeyMissing,
        Known::WrongChain,
        Known::NodeRefused,
        Known::NodeTls,
        Known::ProxyUnreachable,
        Known::ProxyCredentialsMissing,
        Known::ProxyCredentialsRefused,
        Known::NodeTimeout,
        Known::AnswerTooLong,
        Known::AnswerNotJson,
        Known::SeatUnseated,
        Known::SeatDomain,
        Known::BackupNotLanded,
        Known::RecoveryNoMatch,
        Known::PathRelative,
        Known::NoEndpoint,
        Known::Unreachable,
        Known::Disagree,
        Known::ChainShape,
        Known::LedgerNotFetched,
        Known::NewerElsewhere,
        Known::EntryRefused,
        Known::NoGenesis,
        Known::ForkedRoot,
        Known::AuditInput,
        Known::NotAdoptable,
        Known::AlreadyRooted,
        Known::RestorePartial,
        Known::CannotLay,
        Known::NotAHome,
        Known::NoHome,
        Known::KeyNotStored,
        Known::GitShape,
        Known::NotARepo,
        Known::RefMissing,
        Known::ObjectMissing,
        Known::Broken,
        Known::WouldBreak,
        Known::ContentShape,
        Known::QueueShape,
        Known::QueueEmpty,
        Known::NoRegistry,
        Known::NoChainId,
        Known::GasRefused,
        Known::GasNotShown,
        Known::SendFailed,
        Known::NotAudited,
        Known::CleanWord,
        Known::SeatUnhomed,
        Known::AuditBehind,
        Known::SubjectMissing,
        Known::DirEmpty,
        Known::ScanRefused,
        Known::FieldMissing,
        Known::KitRefused,
        Known::Conflict,
        Known::HandedOver,
        Known::NoBytes,
        Known::WorkerPanicked,
        Known::OutcomeLost,
        Known::NotHisBook,
        Known::BookNotWhole,
        Known::NotThisLedger,
        Known::NotAGrant,
        Known::AlreadyHeld,
        Known::GrantNotHeld,
        Known::GrantDirEmpty,
        Known::AlreadyAnchored,
        Known::PayloadRefused,
        Known::ChainIncomplete,
        Known::IdentityExists,
        Known::IdentityHereAs,
        Known::IdentityNameTaken,
        Known::PlaceOneLine,
        Known::NoIdentity,
        Known::PhraseWords,
        Known::PhraseInvalid,
        Known::PhraseConfirm,
        Known::NoWords,
        Known::DeleteUnbacked,
        Known::PasswordsDiffer,
        Known::PasswordShort,
        Known::PasswordLong,
        Known::IdentitiesShape,
        Known::PublishNoKit,
        Known::TermsMismatch,
        Known::GrantFileKit,
        Known::GrantFileBad,
        Known::KitFileBad,
        Known::KitFormWord,
        Known::KitOriginalsWord,
        Known::ExtractKit,
        Known::ExtractPath,
        Known::ExtractNothing,
        Known::ExtractClash,
        Known::ExtractName,
        Known::ExtractNotFolder,
        Known::ExtractExists,
        Known::ExtractLink,
        Known::ExtractChanged,
        Known::ExtractLanding,
        Known::AddressedNotRecipient,
        Known::AddressedBroken,
        Known::AddressedHead,
        Known::AddressedElsewhere,
        Known::AddressedKeyShape,
        Known::AddressedBoth,
        Known::AddressedFolder,
        Known::AddressNotAnchored,
        Known::AddressChainUnread,
        Known::AddressKeyUnproven,
        Known::RemoteKit,
        Known::RemoteStatus,
        Known::RemoteRedirect,
        Known::RemoteTooLarge,
        Known::RemoteTimeout,
        Known::RemoteUnreachable,
        Known::RemoteCert,
        Known::RemoteNotHttps,
        Known::NotAnOriginal,
        Known::TraceFull,
        Known::LocalSeal,
        Known::RecoveryNotPrimary,
        Known::PrimaryDelete,
        Known::PrimaryNoKeyFile,
        Known::PrimaryAlready,
        Known::BackupPassword,
        Known::BackupNotOurs,
        Known::BackupTooNew,
        Known::BackupShape,
        Known::Rekeying,
        Known::BackupNoIdentity,
        Known::HomeUnreachable,
        Known::BusyForRekey,
        Known::DialogUnavailable,
        Known::LocalSetAside,
        Known::EntriesUnreadable,
        Known::NetworkListed,
        Known::InsideHome,
        Known::DoorShut,
        Known::DoorUnread,
        Known::InFlight,
        Known::CliPathTaken,
        Known::CliPathCancelled,
        Known::CliPathNotAllowed,
        Known::CliPathUnsupported,
        Known::BatchVoided,
        Known::CredentialExpired,
        Known::CredentialMissing,
        Known::CredentialShape,
        Known::HomeHeld,
        Known::MachineHeld,
        Known::CliAloneOn,
        Known::CliAloneDropped,
        Known::HomeOtherMachine,
        Known::CliWriting,
        Known::ReadsUnkept,
        Known::HandedOff,
        Known::OnChain,
        Known::TooEarly,
        Known::UpgradeHanded,
        Known::NotMoved,
        Known::NotATarget,
        Known::NotIncluded,
        Known::BookCode,
        Known::RotateOwed,
        Known::RotateBookless,
        Known::OfficeNotInWords,
        Known::ChainInBasis,
        Known::NoNoticeVenue,
        Known::NoticeContract,
        Known::RootNotInBook,
        Known::NotAcceded,
        Known::OfficeElsewhere,
        Known::OfficeIsContract,
        Known::RootKeyAbsent,
        Known::FaceUnread,
        Known::FaceNoHome,
        Known::FaceRotating,
        Known::FaceUnwritable,
        Known::ConsentRefused,
        Known::FaceRow,
        Known::FaceHomeChain,
        Known::RegistryCode,
        Known::RotateUnseated,
        Known::RotateUnentered,
        Known::BookClosed,
        Known::CredentialPassMissing,
        Known::CredentialPassUnknown,
        Known::CredentialNote,
        Known::CredentialFull,
        Known::CredentialRowGone,
        Known::CredentialReclaimed,
        Known::FaceSealed,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Known::FontMissing => "FONT_MISSING",
            Known::FileMissing => "FILE_MISSING",
            Known::Denied => "DENIED",
            Known::NotAFolder => "NOT_A_FOLDER",
            Known::KeychainMissing => "KEYCHAIN_MISSING",
            Known::Entropy => "ENTROPY",
            Known::KeyMalformed => "KEY_MALFORMED",
            Known::SignFailed => "SIGN_FAILED",
            Known::KeystoreParams => "KEYSTORE_PARAMS",
            Known::KeystoreShape => "KEYSTORE_SHAPE",
            Known::BadPassword => "BAD_PASSWORD",
            Known::Ledger => "LEDGER",
            Known::NoHomeDir => "NO_HOME_DIR",
            Known::SettingsShape => "SETTINGS_SHAPE",
            Known::AddressShape => "ADDRESS_SHAPE",
            Known::AddressMismatch => "ADDRESS_MISMATCH",
            Known::Occupied => "OCCUPIED",
            Known::CopyMismatch => "COPY_MISMATCH",
            Known::Landing => "LANDING",
            Known::ReadOnly => "READ_ONLY",
            Known::MirrorShape => "MIRROR_SHAPE",
            Known::MirrorEntry => "MIRROR_ENTRY",
            Known::MirrorOther => "MIRROR_OTHER",
            Known::Locked => "LOCKED",
            Known::LockedOut => "LOCKED_OUT",
            Known::PinWrong => "PIN_WRONG",
            Known::PinShape => "PIN_SHAPE",
            Known::PinSet => "PIN_SET",
            Known::KeyboxMissing => "KEYBOX_MISSING",
            Known::KeyboxShape => "KEYBOX_SHAPE",
            Known::KeyboxSlot => "KEYBOX_SLOT",
            Known::KeyboxNotEmpty => "KEYBOX_NOT_EMPTY",
            Known::KeyboxLocked => "KEYBOX_LOCKED",
            Known::KdfBelowFloor => "KDF_BELOW_FLOOR",
            Known::MachineShape => "MACHINE_SHAPE",
            Known::InsufficientFunds => "INSUFFICIENT_FUNDS",
            Known::NonceUsed => "NONCE_USED",
            Known::AlreadyPending => "ALREADY_PENDING",
            Known::Underpriced => "UNDERPRICED",
            Known::GasTooLow => "GAS_TOO_LOW",
            Known::ContractRefused => "CONTRACT_REFUSED",
            Known::RateLimited => "RATE_LIMITED",
            Known::MethodMissing => "METHOD_MISSING",
            Known::NodeAuth => "NODE_AUTH",
            Known::ServiceUnserved => "SERVICE_UNSERVED",
            Known::ServiceKeyMissing => "SERVICE_KEY_MISSING",
            Known::WrongChain => "WRONG_CHAIN",
            Known::NodeRefused => "NODE_REFUSED",
            Known::NodeTls => "NODE_TLS",
            Known::ProxyUnreachable => "PROXY_UNREACHABLE",
            Known::ProxyCredentialsMissing => "PROXY_CREDENTIALS_MISSING",
            Known::ProxyCredentialsRefused => "PROXY_CREDENTIALS_REFUSED",
            Known::NodeTimeout => "NODE_TIMEOUT",
            Known::AnswerTooLong => "ANSWER_TOO_LONG",
            Known::AnswerNotJson => "ANSWER_NOT_JSON",
            Known::SeatUnseated => "SEAT_UNSEATED",
            Known::SeatDomain => "SEAT_DOMAIN",
            Known::BackupNotLanded => "BACKUP_NOT_LANDED",
            Known::RecoveryNoMatch => "RECOVERY_NO_MATCH",
            Known::PathRelative => "PATH_RELATIVE",
            Known::NoEndpoint => "NO_ENDPOINT",
            Known::Unreachable => "UNREACHABLE",
            Known::Disagree => "DISAGREE",
            Known::ChainShape => "CHAIN_SHAPE",
            Known::LedgerNotFetched => "LEDGER_NOT_FETCHED",
            Known::NewerElsewhere => "NEWER_ELSEWHERE",
            Known::EntryRefused => "ENTRY_REFUSED",
            Known::NoGenesis => "NO_GENESIS",
            Known::ForkedRoot => "FORKED_ROOT",
            Known::AuditInput => "AUDIT_INPUT",
            Known::NotAdoptable => "NOT_ADOPTABLE",
            Known::AlreadyRooted => "ALREADY_ROOTED",
            Known::RestorePartial => "RESTORE_PARTIAL",
            Known::CannotLay => "CANNOT_LAY",
            Known::NotAHome => "NOT_A_HOME",
            Known::NoHome => "NO_HOME",
            Known::KeyNotStored => "KEY_NOT_STORED",
            Known::GitShape => "GIT_SHAPE",
            Known::NotARepo => "NOT_A_REPO",
            Known::RefMissing => "REF_MISSING",
            Known::ObjectMissing => "OBJECT_MISSING",
            Known::Broken => "BROKEN",
            Known::WouldBreak => "WOULD_BREAK",
            Known::ContentShape => "CONTENT_SHAPE",
            Known::QueueShape => "QUEUE_SHAPE",
            Known::QueueEmpty => "QUEUE_EMPTY",
            Known::NoRegistry => "NO_REGISTRY",
            Known::NoChainId => "NO_CHAIN_ID",
            Known::GasRefused => "GAS_REFUSED",
            Known::GasNotShown => "GAS_NOT_ESTIMATED",
            Known::SendFailed => "SEND_FAILED",
            Known::NotAudited => "NOT_AUDITED",
            Known::CleanWord => "CLEAN_WORD",
            Known::SeatUnhomed => "SEAT_UNHOMED",
            Known::AuditBehind => "AUDIT_BEHIND",
            Known::SubjectMissing => "SUBJECT_MISSING",
            Known::DirEmpty => "DIR_EMPTY",
            Known::ScanRefused => "SCAN_REFUSED",
            Known::FieldMissing => "FIELD_MISSING",
            Known::KitRefused => "KIT_REFUSED",
            Known::Conflict => "CONFLICT",
            Known::HandedOver => "HANDED_OVER",
            Known::NoBytes => "NO_BYTES",
            Known::WorkerPanicked => "WORKER_PANICKED",
            Known::OutcomeLost => "OUTCOME_LOST",
            Known::NotHisBook => "NOT_HIS_BOOK",
            Known::BookNotWhole => "BOOK_NOT_WHOLE",
            Known::NotThisLedger => "NOT_THIS_LEDGER",
            Known::NotAGrant => "ONLY_GRANTS_HELD",
            Known::AlreadyHeld => "ALREADY_HELD",
            Known::GrantNotHeld => "GRANT_NOT_HELD",
            Known::GrantDirEmpty => "GRANT_DIR_EMPTY",
            Known::AlreadyAnchored => "ALREADY_ANCHORED",
            Known::PayloadRefused => "PAYLOAD_REFUSED",
            Known::ChainIncomplete => "CHAIN_UNREACHED",
            Known::IdentityExists => "IDENTITY_EXISTS",
            Known::IdentityHereAs => "IDENTITY_HERE_AS",
            Known::IdentityNameTaken => "IDENTITY_NAME_TAKEN",
            Known::PlaceOneLine => "PLACE_ONE_LINE",
            Known::NoIdentity => "NO_IDENTITY",
            Known::PhraseWords => "PHRASE_WORDS",
            Known::PhraseInvalid => "PHRASE_INVALID",
            Known::PhraseConfirm => "PHRASE_CONFIRM",
            Known::NoWords => "NO_WORDS",
            Known::DeleteUnbacked => "DELETE_UNBACKED",
            Known::PasswordsDiffer => "PASSWORDS_DIFFER",
            Known::PasswordShort => "PASSWORD_SHORT",
            Known::PasswordLong => "PASSWORD_LONG",
            Known::IdentitiesShape => "IDENTITIES_SHAPE",
            Known::PublishNoKit => "PUBLISH_NO_KIT",
            Known::TermsMismatch => "TERMS_MISMATCH",
            Known::GrantFileKit => "GRANT_FILE_KIT",
            Known::GrantFileBad => "GRANT_FILE",
            Known::KitFileBad => "KIT_FILE",
            Known::KitFormWord => "KIT_FORM",
            Known::KitOriginalsWord => "KIT_ORIGINALS",
            Known::ExtractKit => "EXTRACT_KIT",
            Known::ExtractPath => "EXTRACT_PATH",
            Known::ExtractNothing => "EXTRACT_NOTHING",
            Known::ExtractClash => "EXTRACT_CLASH",
            Known::ExtractName => "EXTRACT_NAME",
            Known::ExtractNotFolder => "EXTRACT_NOT_A_FOLDER",
            Known::ExtractExists => "EXTRACT_EXISTS",
            Known::ExtractLink => "EXTRACT_LINK",
            Known::ExtractChanged => "EXTRACT_CHANGED",
            Known::ExtractLanding => "EXTRACT_LANDING",
            Known::AddressedNotRecipient
            | Known::AddressedBroken
            | Known::AddressedHead
            | Known::AddressedElsewhere
            | Known::AddressedKeyShape
            | Known::AddressedBoth
            | Known::AddressedFolder
            | Known::AddressNotAnchored
            | Known::AddressChainUnread
            | Known::AddressKeyUnproven => self.addressed().map(zikaron_seal::addressed::Why::code).unwrap_or_default(),
            Known::RemoteKit => "REMOTE_KIT",
            Known::RemoteStatus => "REMOTE_STATUS",
            Known::RemoteRedirect => "REMOTE_REDIRECT",
            Known::RemoteTooLarge => "REMOTE_TOO_LARGE",
            Known::RemoteTimeout => "REMOTE_TIMEOUT",
            Known::RemoteUnreachable => "REMOTE_UNREACHABLE",
            Known::RemoteCert => "REMOTE_CERT",
            Known::RemoteNotHttps => "REMOTE_NOT_HTTPS",
            Known::NotAnOriginal => "NOT_AN_ORIGINAL",
            Known::TraceFull => "TRACE_FULL",
            Known::LocalSeal => "LOCAL_SEAL",
            Known::RecoveryNotPrimary => "RECOVERY_NOT_PRIMARY",
            Known::PrimaryDelete => "PRIMARY_DELETE",
            Known::PrimaryNoKeyFile => "PRIMARY_NO_KEYFILE",
            Known::PrimaryAlready => "PRIMARY_ALREADY",
            Known::BackupPassword => "BACKUP_PASSWORD",
            Known::BackupNotOurs => "BACKUP_NOT_OURS",
            Known::BackupTooNew => "BACKUP_TOO_NEW",
            Known::BackupShape => "BACKUP_SHAPE",
            Known::Rekeying => "REKEYING",
            Known::BackupNoIdentity => "BACKUP_NO_IDENTITY",
            Known::HomeUnreachable => "HOME_UNREACHABLE",
            Known::BusyForRekey => "BUSY_FOR_REKEY",
            Known::DialogUnavailable => "DIALOG_UNAVAILABLE",
            Known::LocalSetAside => "LOCAL_SET_ASIDE",
            Known::EntriesUnreadable => "ENTRIES_UNREADABLE",
            Known::NetworkListed => "NETWORK_LISTED",
            Known::InsideHome => "INSIDE_HOME",
            Known::DoorShut => "DOOR_SHUT",
            Known::DoorUnread => "DOOR_UNREAD",
            Known::InFlight => "IN_FLIGHT",
            Known::CliPathTaken => "CLI_PATH_TAKEN",
            Known::CliPathCancelled => "CLI_PATH_CANCELLED",
            Known::CliPathNotAllowed => "CLI_PATH_NOT_ALLOWED",
            Known::CliPathUnsupported => "CLI_PATH_UNSUPPORTED",
            Known::BatchVoided => "BATCH_VOIDED",
            Known::CredentialExpired => "CREDENTIAL_EXPIRED",
            Known::CredentialMissing => "CREDENTIAL_MISSING",
            Known::CredentialShape => "CREDENTIAL_SHAPE",
            Known::HomeHeld => "HOME_HELD",
            Known::MachineHeld => "MACHINE_HELD",
            Known::CliAloneOn => "CLI_ALONE_ON",
            Known::CliAloneDropped => "CLI_ALONE_DROPPED",
            Known::HomeOtherMachine => "HOME_OTHER_MACHINE",
            Known::CliWriting => "CLI_WRITING",
            Known::ReadsUnkept => "READS_UNKEPT",
            Known::HandedOff => "HANDED_OFF",
            Known::OnChain => "ON_CHAIN",
            Known::TooEarly => "TOO_EARLY",
            Known::UpgradeHanded => "UPGRADE_HANDED",
            Known::NotMoved => "NOT_MOVED",
            Known::NotATarget => "NOT_A_TARGET",
            Known::ChainInBasis => "CHAIN_IN_BASIS",
            Known::OfficeNotInWords => "OFFICE_NOT_IN_WORDS",
            Known::RotateOwed => "ROTATE_OWED",
            Known::RotateBookless => "ROTATE_BOOKLESS",
            Known::BookCode => "BOOK_CODE",
            Known::NotIncluded => "NOT_INCLUDED",
            Known::NoNoticeVenue => "NO_REVOCATION_CONTRACT",
            Known::NoticeContract => "REVOCATION_CODE",
            Known::RootNotInBook => "ROOT_NOT_IN_BOOK",
            Known::NotAcceded => "NOT_ACCEDED",
            Known::OfficeElsewhere => "BOOK_OFFICE_OTHER",
            Known::OfficeIsContract => "OFFICE_IS_CONTRACT",
            Known::RootKeyAbsent => "ROOT_KEY_ABSENT",
            Known::FaceUnread => "FACE_UNREAD",
            Known::FaceNoHome => "FACE_NO_HOME",
            Known::FaceRotating => "FACE_ROTATING",
            Known::FaceUnwritable => "FACE_UNWRITABLE",
            Known::ConsentRefused => "CONSENT_REFUSED",
            Known::FaceRow => "FACE_ROW",
            Known::FaceHomeChain => "FACE_HOME_CHAIN",
            Known::RegistryCode => "REGISTRY_CODE",
            Known::RotateUnseated => "ROTATE_UNSEATED",
            Known::RotateUnentered => "ROTATE_UNENTERED",
            Known::BookClosed => "BOOK_CLOSED",
            Known::CredentialPassMissing => "CREDENTIAL_PASS_MISSING",
            Known::CredentialPassUnknown => "CREDENTIAL_PASS_UNKNOWN",
            Known::CredentialNote => "CREDENTIAL_NOTE",
            Known::CredentialFull => "CREDENTIAL_FULL",
            Known::CredentialRowGone => "CREDENTIAL_ROW_GONE",
            Known::CredentialReclaimed => "CREDENTIAL_RECLAIMED",
            Known::FaceSealed => "FACE_SEALED",
        }
    }
}

impl Known {
    pub fn what(self) -> crate::lang::Key {
        use crate::lang::Key as L;
        match self {
            Known::FontMissing => L::FaultWhatFontMissing,
            Known::FileMissing => L::FaultWhatFileMissing,
            Known::Denied => L::FaultWhatDenied,
            Known::NotAFolder => L::FaultWhatNotAFolder,
            Known::KeychainMissing => L::FaultWhatKeychainMissing,
            Known::Entropy => L::FaultWhatEntropy,
            Known::KeyMalformed => L::FaultWhatKeyMalformed,
            Known::SignFailed => L::FaultWhatSignFailed,
            Known::KeystoreParams => L::FaultWhatKeystoreParams,
            Known::KeystoreShape => L::FaultWhatKeystoreShape,
            Known::BadPassword => L::FaultWhatBadPassword,
            Known::Ledger => L::FaultWhatLedger,
            Known::NoHomeDir => L::FaultWhatNoHomeDir,
            Known::SettingsShape => L::FaultWhatSettingsShape,
            Known::AddressShape => L::FaultWhatAddressShape,
            Known::AddressMismatch => L::FaultWhatAddressMismatch,
            Known::Occupied => L::FaultWhatOccupied,
            Known::CopyMismatch => L::FaultWhatCopyMismatch,
            Known::Landing => L::FaultWhatLanding,
            Known::ReadOnly => L::FaultWhatReadOnly,
            Known::MirrorShape => L::FaultWhatMirrorShape,
            Known::MirrorEntry => L::FaultWhatMirrorEntry,
            Known::MirrorOther => L::FaultWhatMirrorOther,
            Known::Locked => L::FaultWhatLocked,
            Known::LockedOut => L::FaultWhatLockedOut,
            Known::PinWrong => L::FaultWhatPinWrong,
            Known::PinShape => L::FaultWhatPinShape,
            Known::PinSet => L::FaultWhatPinSet,
            Known::KeyboxMissing => L::FaultWhatKeyboxMissing,
            Known::KeyboxShape => L::FaultWhatKeyboxShape,
            Known::KeyboxSlot => L::FaultWhatKeyboxSlot,
            Known::KeyboxNotEmpty => L::FaultWhatKeyboxNotEmpty,
            Known::KeyboxLocked => L::FaultWhatKeyboxLocked,
            Known::KdfBelowFloor => L::FaultWhatKdfBelowFloor,
            Known::MachineShape => L::FaultWhatMachineShape,
            Known::InsufficientFunds => L::FaultWhatInsufficientFunds,
            Known::NonceUsed => L::FaultWhatNonceUsed,
            Known::AlreadyPending => L::FaultWhatAlreadyPending,
            Known::Underpriced => L::FaultWhatUnderpriced,
            Known::GasTooLow => L::FaultWhatGasTooLow,
            Known::ContractRefused => L::FaultWhatContractRefused,
            Known::RateLimited => L::FaultWhatRateLimited,
            Known::MethodMissing => L::FaultWhatMethodMissing,
            Known::NodeAuth => L::FaultWhatNodeAuth,
            Known::ServiceUnserved => L::FaultWhatServiceUnserved,
            Known::ServiceKeyMissing => L::FaultWhatServiceKeyMissing,
            Known::WrongChain => L::FaultWhatWrongChain,
            Known::NodeRefused => L::FaultWhatNodeRefused,
            Known::NodeTls => L::FaultWhatNodeTls,
            Known::ProxyUnreachable => L::FaultWhatProxyUnreachable,
            Known::ProxyCredentialsMissing => L::FaultWhatProxyCredentialsMissing,
            Known::ProxyCredentialsRefused => L::FaultWhatProxyCredentialsRefused,
            Known::NodeTimeout => L::FaultWhatNodeTimeout,
            Known::AnswerTooLong => L::FaultWhatAnswerTooLong,
            Known::AnswerNotJson => L::FaultWhatAnswerNotJson,
            Known::SeatUnseated => L::FaultWhatSeatUnseated,
            Known::SeatDomain => L::FaultWhatSeatDomain,
            Known::BackupNotLanded => L::FaultWhatBackupNotLanded,
            Known::RecoveryNoMatch => L::FaultWhatRecoveryNoMatch,
            Known::PathRelative => L::FaultWhatPathRelative,
            Known::NoEndpoint => L::FaultWhatNoEndpoint,
            Known::Unreachable => L::FaultWhatUnreachable,
            Known::Disagree => L::FaultWhatDisagree,
            Known::ChainShape => L::FaultWhatChainShape,
            Known::LedgerNotFetched => L::FaultWhatLedgerNotFetched,
            Known::NewerElsewhere => L::FaultWhatNewerElsewhere,
            Known::EntryRefused => L::FaultWhatEntryRefused,
            Known::NoGenesis => L::FaultWhatNoGenesis,
            Known::ForkedRoot => L::FaultWhatForkedRoot,
            Known::AuditInput => L::FaultWhatAuditInput,
            Known::NotAdoptable => L::FaultWhatNotAdoptable,
            Known::AlreadyRooted => L::FaultWhatAlreadyRooted,
            Known::RestorePartial => L::FaultWhatRestorePartial,
            Known::CannotLay => L::FaultWhatCannotLay,
            Known::NotAHome => L::FaultWhatNotAHome,
            Known::NoHome => L::FaultWhatNoHome,
            Known::KeyNotStored => L::FaultWhatKeyNotStored,
            Known::GitShape => L::FaultWhatGitShape,
            Known::NotARepo => L::FaultWhatNotARepo,
            Known::RefMissing => L::FaultWhatRefMissing,
            Known::ObjectMissing => L::FaultWhatObjectMissing,
            Known::Broken => L::FaultWhatBroken,
            Known::WouldBreak => L::FaultWhatWouldBreak,
            Known::ContentShape => L::FaultWhatContentShape,
            Known::QueueShape => L::FaultWhatQueueShape,
            Known::QueueEmpty => L::FaultWhatQueueEmpty,
            Known::NoRegistry => L::FaultWhatNoRegistry,
            Known::NoChainId => L::FaultWhatNoChainId,
            Known::GasRefused => L::FaultWhatGasRefused,
            Known::GasNotShown => L::FaultWhatGasNotShown,
            Known::SendFailed => L::FaultWhatSendFailed,
            Known::NotAudited => L::FaultWhatNotAudited,
            Known::CleanWord => L::FaultWhatCleanWord,
            Known::SeatUnhomed => L::FaultWhatSeatUnhomed,
            Known::AuditBehind => L::FaultWhatAuditBehind,
            Known::SubjectMissing => L::FaultWhatSubjectMissing,
            Known::DirEmpty => L::FaultWhatDirEmpty,
            Known::ScanRefused => L::FaultWhatScanRefused,
            Known::FieldMissing => L::FaultWhatFieldMissing,
            Known::KitRefused => L::FaultWhatKitRefused,
            Known::Conflict => L::FaultWhatConflict,
            Known::HandedOver => L::FaultWhatHandedOver,
            Known::NoBytes => L::FaultWhatNoBytes,
            Known::WorkerPanicked => L::FaultWhatWorkerPanicked,
            Known::OutcomeLost => L::FaultWhatOutcomeLost,
            Known::NotHisBook => L::FaultWhatNotHisBook,
            Known::BookNotWhole => L::FaultWhatBookNotWhole,
            Known::NotThisLedger => L::FaultWhatNotThisLedger,
            Known::NotAGrant => L::FaultWhatNotAGrant,
            Known::AlreadyHeld => L::FaultWhatAlreadyHeld,
            Known::GrantNotHeld => L::FaultWhatGrantNotHeld,
            Known::GrantDirEmpty => L::FaultWhatGrantDirEmpty,
            Known::AlreadyAnchored => L::FaultWhatAlreadyAnchored,
            Known::PayloadRefused => L::FaultWhatPayloadRefused,
            Known::ChainIncomplete => L::FaultWhatChainIncomplete,
            Known::IdentityExists => L::FaultWhatIdentityExists,
            Known::IdentityHereAs => L::FaultWhatIdentityHereAs,
            Known::IdentityNameTaken => L::FaultWhatIdentityNameTaken,
            Known::PlaceOneLine => L::FaultWhatPlaceOneLine,
            Known::NoIdentity => L::FaultWhatNoIdentity,
            Known::PhraseWords => L::FaultWhatPhraseWords,
            Known::PhraseInvalid => L::FaultWhatPhraseInvalid,
            Known::PhraseConfirm => L::FaultWhatPhraseConfirm,
            Known::NoWords => L::FaultWhatNoWords,
            Known::DeleteUnbacked => L::FaultWhatDeleteUnbacked,
            Known::PasswordsDiffer => L::FaultWhatPasswordsDiffer,
            Known::PasswordShort => L::FaultWhatPasswordShort,
            Known::PasswordLong => L::FaultWhatPasswordLong,
            Known::IdentitiesShape => L::FaultWhatIdentitiesShape,
            Known::PublishNoKit => L::FaultWhatPublishNoKit,
            Known::TermsMismatch => L::FaultWhatTermsMismatch,
            Known::GrantFileKit => L::FaultWhatGrantFileKit,
            Known::GrantFileBad => L::FaultWhatGrantFileBad,
            Known::KitFileBad => L::FaultWhatKitFileBad,
            Known::KitFormWord => L::FaultWhatKitFormWord,
            Known::KitOriginalsWord => L::FaultWhatKitOriginalsWord,
            Known::ExtractKit => L::FaultWhatExtractKit,
            Known::ExtractPath => L::FaultWhatExtractPath,
            Known::ExtractNothing => L::FaultWhatExtractNothing,
            Known::ExtractClash => L::FaultWhatExtractClash,
            Known::ExtractName => L::FaultWhatExtractName,
            Known::ExtractNotFolder => L::FaultWhatExtractNotFolder,
            Known::ExtractExists => L::FaultWhatExtractExists,
            Known::ExtractLink => L::FaultWhatExtractLink,
            Known::ExtractChanged => L::FaultWhatExtractChanged,
            Known::ExtractLanding => L::FaultWhatExtractLanding,
            Known::AddressedNotRecipient => L::FaultWhatAddressedNotRecipient,
            Known::AddressedBroken => L::FaultWhatAddressedBroken,
            Known::AddressedHead => L::FaultWhatAddressedHead,
            Known::AddressedElsewhere => L::FaultWhatAddressedElsewhere,
            Known::AddressedKeyShape => L::FaultWhatAddressedKeyShape,
            Known::AddressedBoth => L::FaultWhatAddressedBoth,
            Known::AddressedFolder => L::FaultWhatAddressedFolder,
            Known::AddressNotAnchored => L::FaultWhatAddressNotAnchored,
            Known::AddressChainUnread => L::FaultWhatAddressChainUnread,
            Known::AddressKeyUnproven => L::FaultWhatAddressKeyUnproven,
            Known::RemoteKit => L::FaultWhatRemoteKit,
            Known::RemoteStatus => L::FaultWhatRemoteStatus,
            Known::RemoteRedirect => L::FaultWhatRemoteRedirect,
            Known::RemoteTooLarge => L::FaultWhatRemoteTooLarge,
            Known::RemoteTimeout => L::FaultWhatRemoteTimeout,
            Known::RemoteUnreachable => L::FaultWhatRemoteUnreachable,
            Known::RemoteCert => L::FaultWhatRemoteCert,
            Known::RemoteNotHttps => L::FaultWhatRemoteNotHttps,
            Known::NotAnOriginal => L::FaultWhatNotAnOriginal,
            Known::TraceFull => L::FaultWhatTraceFull,
            Known::LocalSeal => L::FaultWhatLocalSeal,
            Known::RecoveryNotPrimary => L::FaultWhatRecoveryNotPrimary,
            Known::PrimaryDelete => L::FaultWhatPrimaryDelete,
            Known::PrimaryNoKeyFile => L::FaultWhatPrimaryNoKeyFile,
            Known::PrimaryAlready => L::FaultWhatPrimaryAlready,
            Known::BackupPassword => L::FaultWhatBackupPassword,
            Known::BackupNotOurs => L::FaultWhatBackupNotOurs,
            Known::BackupTooNew => L::FaultWhatBackupTooNew,
            Known::BackupShape => L::FaultWhatBackupShape,
            Known::Rekeying => L::FaultWhatRekeying,
            Known::BackupNoIdentity => L::FaultWhatBackupNoIdentity,
            Known::HomeUnreachable => L::FaultWhatHomeUnreachable,
            Known::BusyForRekey => L::FaultWhatBusyForRekey,
            Known::DialogUnavailable => L::FaultWhatDialogUnavailable,
            Known::LocalSetAside => L::FaultWhatLocalSetAside,
            Known::EntriesUnreadable => L::FaultWhatEntriesUnreadable,
            Known::NetworkListed => L::FaultWhatNetworkListed,
            Known::InsideHome => L::FaultWhatInsideHome,
            Known::DoorShut => L::FaultWhatDoorShut,
            Known::DoorUnread => L::FaultWhatDoorUnread,
            Known::InFlight => L::FaultWhatInFlight,
            Known::CliPathTaken => L::FaultWhatCliPathTaken,
            Known::CliPathCancelled => L::FaultWhatCliPathCancelled,
            Known::CliPathNotAllowed => L::FaultWhatCliPathNotAllowed,
            Known::CliPathUnsupported => L::FaultWhatCliPathUnsupported,
            Known::BatchVoided => L::FaultWhatBatchVoided,
            Known::CredentialExpired => L::FaultWhatCredentialExpired,
            Known::CredentialMissing => L::FaultWhatCredentialMissing,
            Known::CredentialShape => L::FaultWhatCredentialShape,
            Known::HomeHeld => L::FaultWhatHomeHeld,
            Known::MachineHeld => L::FaultWhatMachineHeld,
            Known::CliAloneOn => L::SetCliDesktopGrey,
            Known::CliAloneDropped => L::FaultWhatCliAloneDropped,
            Known::HomeOtherMachine => L::FaultWhatHomeOtherMachine,
            Known::CliWriting => L::FaultWhatCliWriting,
            Known::ReadsUnkept => L::FaultWhatReadsUnkept,
            Known::HandedOff => L::FaultWhatHandedOff,
            Known::OnChain => L::FaultWhatOnChain,
            Known::TooEarly => L::FaultWhatTooEarly,
            Known::UpgradeHanded => L::FaultWhatUpgradeHanded,
            Known::NotMoved => L::FaultWhatNotMoved,
            Known::NotATarget => L::FaultWhatNotATarget,
            Known::ChainInBasis => L::FaultWhatChainInBasis,
            Known::OfficeNotInWords => L::FaultWhatOfficeNotInWords,
            Known::RotateOwed => L::FaultWhatRotateOwed,
            Known::RotateBookless => L::FaultWhatRotateBookless,
            Known::BookCode => L::FaultWhatBookCode,
            Known::NotIncluded => L::FaultWhatNotIncluded,
            Known::NoNoticeVenue => L::FaultWhatNoNoticeVenue,
            Known::NoticeContract => L::FaultWhatNoticeContract,
            Known::RootNotInBook => L::FaultWhatRootNotInBook,
            Known::NotAcceded => L::FaultWhatNotAcceded,
            Known::OfficeElsewhere => L::FaultWhatOfficeElsewhere,
            Known::OfficeIsContract => L::FaultWhatOfficeIsContract,
            Known::RootKeyAbsent => L::FaultWhatRootKeyAbsent,
            Known::FaceUnread => L::FaultWhatFaceUnread,
            Known::FaceNoHome => L::FaultWhatFaceNoHome,
            Known::FaceRotating => L::FaultWhatFaceRotating,
            Known::FaceUnwritable => L::FaultWhatFaceUnwritable,
            Known::ConsentRefused => L::FaultWhatConsentRefused,
            Known::FaceRow => L::FaultWhatFaceRow,
            Known::FaceHomeChain => L::FaultWhatFaceHomeChain,
            Known::RegistryCode => L::FaultWhatRegistryCode,
            Known::RotateUnseated => L::FaultWhatRotateUnseated,
            Known::RotateUnentered => L::FaultWhatRotateUnentered,
            Known::BookClosed => L::FaultWhatBookClosed,
            Known::CredentialPassMissing => L::FaultWhatCredentialPassMissing,
            Known::CredentialPassUnknown => L::FaultWhatCredentialPassUnknown,
            Known::CredentialNote => L::FaultWhatCredentialNote,
            Known::CredentialFull => L::FaultWhatCredentialFull,
            Known::CredentialRowGone => L::FaultWhatCredentialRowGone,
            Known::CredentialReclaimed => L::FaultWhatCredentialReclaimed,
            Known::FaceSealed => L::FaultWhatFaceSealed,
        }
    }

    pub fn next(self) -> Option<crate::lang::Key> {
        use crate::lang::Key as L;
        match self {
            Known::FontMissing => Some(L::FaultNextFontMissing),
            Known::FileMissing => Some(L::FaultNextFileMissing),
            Known::Denied => Some(L::FaultNextDenied),
            Known::NotAFolder => Some(L::FaultNextNotAFolder),
            Known::KeychainMissing => Some(L::FaultNextKeychainMissing),
            Known::Entropy => Some(L::FaultNextEntropy),
            Known::KeyMalformed => Some(L::FaultNextKeyMalformed),
            Known::SignFailed => Some(L::FaultNextSignFailed),
            Known::KeystoreParams => Some(L::FaultNextKeystoreParams),
            Known::KeystoreShape => Some(L::FaultNextKeystoreShape),
            Known::BadPassword => Some(L::FaultNextBadPassword),
            Known::Ledger => Some(L::FaultNextLedger),
            Known::NoHomeDir => Some(L::FaultNextNoHomeDir),
            Known::SettingsShape => Some(L::FaultNextSettingsShape),
            Known::AddressShape => Some(L::FaultNextAddressShape),
            Known::AddressMismatch => Some(L::FaultNextAddressMismatch),
            Known::Occupied => Some(L::FaultNextOccupied),
            Known::CopyMismatch => Some(L::FaultNextCopyMismatch),
            Known::Landing => Some(L::FaultNextLanding),
            Known::ReadOnly => Some(L::FaultNextReadOnly),
            Known::MirrorShape => Some(L::FaultNextMirrorShape),
            Known::MirrorEntry => Some(L::FaultNextMirrorEntry),
            Known::MirrorOther => Some(L::FaultNextMirrorOther),
            Known::Locked => Some(L::FaultNextLocked),
            Known::LockedOut => Some(L::FaultNextLockedOut),
            Known::PinWrong => Some(L::FaultNextPinWrong),
            Known::PinShape => Some(L::FaultNextPinShape),
            Known::PinSet => Some(L::FaultNextPinSet),
            Known::KeyboxMissing => Some(L::FaultNextKeyboxMissing),
            Known::KeyboxShape => Some(L::FaultNextKeyboxShape),
            Known::KeyboxSlot => Some(L::FaultNextKeyboxSlot),
            Known::KeyboxNotEmpty => Some(L::FaultNextKeyboxNotEmpty),
            Known::KeyboxLocked => Some(L::FaultNextKeyboxLocked),
            Known::KdfBelowFloor => Some(L::FaultNextKdfBelowFloor),
            Known::MachineShape => Some(L::FaultNextMachineShape),
            Known::InsufficientFunds => Some(L::FaultNextInsufficientFunds),
            Known::NonceUsed => Some(L::FaultNextNonceUsed),
            Known::AlreadyPending => Some(L::FaultNextAlreadyPending),
            Known::Underpriced => Some(L::FaultNextUnderpriced),
            Known::GasTooLow => Some(L::FaultNextGasTooLow),
            Known::ContractRefused => Some(L::FaultNextContractRefused),
            Known::RateLimited => Some(L::FaultNextRateLimited),
            Known::MethodMissing => Some(L::FaultNextMethodMissing),
            Known::NodeAuth => Some(L::FaultNextNodeAuth),
            Known::ServiceUnserved => Some(L::FaultNextServiceUnserved),
            Known::ServiceKeyMissing => Some(L::FaultNextServiceKeyMissing),
            Known::WrongChain => Some(L::FaultNextWrongChain),
            Known::NodeRefused => Some(L::FaultNextNodeRefused),
            Known::NodeTls => Some(L::FaultNextNodeTls),
            Known::ProxyUnreachable => Some(L::FaultNextProxyUnreachable),
            Known::ProxyCredentialsMissing => Some(L::FaultNextProxyCredentialsMissing),
            Known::ProxyCredentialsRefused => Some(L::FaultNextProxyCredentialsRefused),
            Known::NodeTimeout => Some(L::FaultNextNodeTimeout),
            Known::AnswerTooLong => Some(L::FaultNextAnswerTooLong),
            Known::AnswerNotJson => Some(L::FaultNextAnswerNotJson),
            Known::SeatUnseated => Some(L::FaultNextSeatUnseated),
            Known::SeatDomain => Some(L::FaultNextSeatDomain),
            Known::BackupNotLanded => Some(L::FaultNextBackupNotLanded),
            Known::RecoveryNoMatch => Some(L::FaultNextRecoveryNoMatch),
            Known::PathRelative => Some(L::FaultNextPathRelative),
            Known::NoEndpoint => Some(L::FaultNextNoEndpoint),
            Known::Unreachable => Some(L::FaultNextUnreachable),
            Known::Disagree => Some(L::FaultNextDisagree),
            Known::ChainShape => Some(L::FaultNextChainShape),
            Known::LedgerNotFetched => Some(L::FaultNextLedgerNotFetched),
            Known::NewerElsewhere => Some(L::FaultNextNewerElsewhere),
            Known::EntryRefused => Some(L::FaultNextEntryRefused),
            Known::NoGenesis => Some(L::FaultNextNoGenesis),
            Known::ForkedRoot => Some(L::FaultNextForkedRoot),
            Known::AuditInput => Some(L::FaultNextAuditInput),
            Known::NotAdoptable => Some(L::FaultNextNotAdoptable),
            Known::AlreadyRooted => Some(L::FaultNextAlreadyRooted),
            Known::RestorePartial => Some(L::FaultNextRestorePartial),
            Known::CannotLay => Some(L::FaultNextCannotLay),
            Known::NotAHome => Some(L::FaultNextNotAHome),
            Known::NoHome => Some(L::FaultNextNoHome),
            Known::KeyNotStored => Some(L::FaultNextKeyNotStored),
            Known::GitShape => Some(L::FaultNextGitShape),
            Known::NotARepo => Some(L::FaultNextNotARepo),
            Known::RefMissing => Some(L::FaultNextRefMissing),
            Known::ObjectMissing => Some(L::FaultNextObjectMissing),
            Known::Broken => Some(L::FaultNextBroken),
            Known::WouldBreak => Some(L::FaultNextWouldBreak),
            Known::ContentShape => Some(L::FaultNextContentShape),
            Known::QueueShape => Some(L::FaultNextQueueShape),
            Known::QueueEmpty => Some(L::FaultNextQueueEmpty),
            Known::NoRegistry => Some(L::FaultNextNoRegistry),
            Known::NoChainId => Some(L::FaultNextNoChainId),
            Known::GasRefused => Some(L::FaultNextGasRefused),
            Known::GasNotShown => Some(L::FaultNextGasNotShown),
            Known::SendFailed => Some(L::FaultNextSendFailed),
            Known::NotAudited => Some(L::FaultNextNotAudited),
            Known::CleanWord => Some(L::FaultNextCleanWord),
            Known::SeatUnhomed => Some(L::FaultNextSeatUnhomed),
            Known::AuditBehind => Some(L::FaultNextAuditBehind),
            Known::SubjectMissing => Some(L::FaultNextSubjectMissing),
            Known::DirEmpty => Some(L::FaultNextDirEmpty),
            Known::ScanRefused => Some(L::FaultNextScanRefused),
            Known::FieldMissing => Some(L::FaultNextFieldMissing),
            Known::KitRefused => Some(L::FaultNextKitRefused),
            Known::Conflict => Some(L::FaultNextConflict),
            Known::HandedOver => Some(L::FaultNextHandedOver),
            Known::NoBytes => Some(L::FaultNextNoBytes),
            Known::WorkerPanicked => Some(L::FaultNextWorkerPanicked),
            Known::OutcomeLost => Some(L::FaultNextOutcomeLost),
            Known::NotHisBook => Some(L::FaultNextNotHisBook),
            Known::BookNotWhole => Some(L::FaultNextBookNotWhole),
            Known::NotThisLedger => Some(L::FaultNextNotThisLedger),
            Known::NotAGrant => Some(L::FaultNextNotAGrant),
            Known::AlreadyHeld => Some(L::FaultNextAlreadyHeld),
            Known::GrantNotHeld => Some(L::FaultNextGrantNotHeld),
            Known::GrantDirEmpty => Some(L::FaultNextGrantDirEmpty),
            Known::AlreadyAnchored => Some(L::FaultNextAlreadyAnchored),
            Known::PayloadRefused => Some(L::FaultNextPayloadRefused),
            Known::ChainIncomplete => Some(L::FaultNextChainIncomplete),
            Known::IdentityExists => Some(L::FaultNextIdentityExists),
            Known::IdentityHereAs => Some(L::FaultNextIdentityHereAs),
            Known::IdentityNameTaken => Some(L::FaultNextIdentityNameTaken),
            Known::PlaceOneLine => Some(L::FaultNextPlaceOneLine),
            Known::NoIdentity => Some(L::FaultNextNoIdentity),
            Known::PhraseWords => Some(L::FaultNextPhraseWords),
            Known::PhraseInvalid => Some(L::FaultNextPhraseInvalid),
            Known::PhraseConfirm => Some(L::FaultNextPhraseConfirm),
            Known::NoWords => Some(L::FaultNextNoWords),
            Known::DeleteUnbacked => Some(L::FaultNextDeleteUnbacked),
            Known::PasswordsDiffer => Some(L::FaultNextPasswordsDiffer),
            Known::PasswordShort => Some(L::FaultNextPasswordShort),
            Known::PasswordLong => Some(L::FaultNextPasswordLong),
            Known::IdentitiesShape => Some(L::FaultNextIdentitiesShape),
            Known::PublishNoKit => Some(L::FaultNextPublishNoKit),
            Known::TermsMismatch => Some(L::FaultNextTermsMismatch),
            Known::GrantFileKit => Some(L::FaultNextGrantFileKit),
            Known::GrantFileBad => Some(L::FaultNextGrantFileBad),
            Known::KitFileBad => Some(L::FaultNextKitFileBad),
            Known::KitFormWord => Some(L::FaultNextKitFormWord),
            Known::KitOriginalsWord => Some(L::FaultNextKitOriginalsWord),
            Known::ExtractKit | Known::ExtractPath | Known::ExtractNothing | Known::ExtractClash | Known::ExtractName | Known::ExtractNotFolder | Known::ExtractExists | Known::ExtractLink | Known::ExtractChanged | Known::ExtractLanding => None,
            Known::AddressedNotRecipient | Known::AddressedBroken | Known::AddressedHead | Known::AddressedElsewhere | Known::AddressedKeyShape | Known::AddressedBoth | Known::AddressedFolder | Known::AddressNotAnchored | Known::AddressChainUnread | Known::AddressKeyUnproven => None,
            Known::RemoteKit => Some(L::FaultNextRemoteKit),
            Known::RemoteStatus => Some(L::FaultNextRemoteStatus),
            Known::RemoteRedirect => Some(L::FaultNextRemoteRedirect),
            Known::RemoteTooLarge => Some(L::FaultNextRemoteTooLarge),
            Known::RemoteTimeout => Some(L::FaultNextRemoteTimeout),
            Known::RemoteUnreachable => Some(L::FaultNextRemoteUnreachable),
            Known::RemoteCert => Some(L::FaultNextRemoteCert),
            Known::RemoteNotHttps => Some(L::FaultNextRemoteNotHttps),
            Known::NotAnOriginal => Some(L::FaultNextNotAnOriginal),
            Known::TraceFull => Some(L::FaultNextTraceFull),
            Known::LocalSeal => Some(L::FaultNextLocalSeal),
            Known::RecoveryNotPrimary => Some(L::FaultNextRecoveryNotPrimary),
            Known::PrimaryDelete => Some(L::FaultNextPrimaryDelete),
            Known::PrimaryNoKeyFile => Some(L::FaultNextPrimaryNoKeyFile),
            Known::PrimaryAlready => Some(L::FaultNextPrimaryAlready),
            Known::BackupPassword => Some(L::FaultNextBackupPassword),
            Known::BackupNotOurs => Some(L::FaultNextBackupNotOurs),
            Known::BackupTooNew => Some(L::FaultNextBackupTooNew),
            Known::BackupShape => Some(L::FaultNextBackupShape),
            Known::Rekeying => Some(L::FaultNextRekeying),
            Known::BackupNoIdentity => Some(L::FaultNextBackupNoIdentity),
            Known::HomeUnreachable => Some(L::FaultNextHomeUnreachable),
            Known::BusyForRekey => Some(L::FaultNextBusyForRekey),
            Known::DialogUnavailable => Some(L::FaultNextDialogUnavailable),
            Known::LocalSetAside => Some(L::FaultNextLocalSetAside),
            Known::EntriesUnreadable => Some(L::FaultNextEntriesUnreadable),
            Known::NetworkListed => Some(L::FaultNextNetworkListed),
            Known::InsideHome => Some(L::FaultNextInsideHome),
            Known::DoorShut => Some(L::FaultNextDoorShut),
            Known::DoorUnread => Some(L::FaultNextDoorUnread),
            Known::InFlight => Some(L::FaultNextInFlight),
            Known::CliPathTaken => Some(L::FaultNextCliPathTaken),
            Known::CliPathCancelled => Some(L::FaultNextCliPathCancelled),
            Known::CliPathNotAllowed => Some(L::FaultNextCliPathNotAllowed),
            Known::CliPathUnsupported => Some(L::FaultNextCliPathUnsupported),
            Known::BatchVoided => Some(L::FaultNextBatchVoided),
            Known::CredentialExpired => Some(L::FaultNextCredentialExpired),
            Known::CredentialMissing => Some(L::FaultNextCredentialMissing),
            Known::CredentialShape => Some(L::FaultNextCredentialShape),
            Known::HomeHeld => Some(L::FaultNextHomeHeld),
            Known::MachineHeld => Some(L::FaultNextMachineHeld),
            Known::CliAloneOn => Some(L::FaultNextCliAloneOn),
            Known::CliAloneDropped => None,
            Known::HomeOtherMachine => Some(L::FaultNextHomeOtherMachine),
            Known::CliWriting => Some(L::FaultNextCliWriting),
            Known::ReadsUnkept => None,
            Known::HandedOff => Some(L::FaultNextHandedOff),
            Known::OnChain => Some(L::FaultNextOnChain),
            Known::TooEarly => Some(L::FaultNextTooEarly),
            Known::UpgradeHanded => Some(L::FaultNextUpgradeHanded),
            Known::NotMoved => Some(L::FaultNextNotMoved),
            Known::NotATarget => Some(L::FaultNextNotATarget),
            Known::ChainInBasis => None,
            Known::OfficeNotInWords => Some(L::FaultNextOfficeNotInWords),
            Known::RotateOwed => None,
            Known::RotateBookless => None,
            Known::BookCode => None,
            Known::NotIncluded => Some(L::FaultNextNotIncluded),
            Known::NoNoticeVenue => None,
            Known::NoticeContract => None,
            Known::RootNotInBook => Some(L::FaultNextRootNotInBook),
            Known::NotAcceded => Some(L::FaultNextNotAcceded),
            Known::OfficeElsewhere => None,
            Known::OfficeIsContract => None,
            Known::RootKeyAbsent => None,
            Known::FaceUnread => Some(L::FaultNextFaceUnread),
            Known::FaceNoHome => Some(L::FaultNextFaceNoHome),
            Known::FaceRotating => Some(L::FaultNextFaceRotating),
            Known::FaceUnwritable => None,
            Known::ConsentRefused => None,
            Known::FaceRow => None,
            Known::FaceHomeChain => None,
            Known::RegistryCode => None,
            Known::RotateUnseated => Some(L::FaultNextRotateUnseated),
            Known::RotateUnentered => Some(L::FaultNextRotateUnentered),
            Known::BookClosed => Some(L::FaultNextBookClosed),
            Known::CredentialPassMissing => None,
            Known::CredentialPassUnknown => None,
            Known::CredentialNote => None,
            Known::CredentialFull => Some(L::FaultNextCredentialFull),
            Known::CredentialRowGone => None,
            Known::CredentialReclaimed => Some(L::FaultNextCredentialReclaimed),
            Known::FaceSealed => Some(L::FaultNextFaceSealed),
        }
    }
}

pub fn translate(k: Known) -> &'static str {
    match k {
        Known::FontMissing => "系统里找不到这一面字体,界面会改用别的面顶上",
        Known::FileMissing => "这一档不在盘上",
        Known::Denied => "没有权限读写这一处",
        Known::NotAFolder => "路上那一处不是文件夹",
        Known::KeychainMissing => "本机密钥库里还没有这一枚",
        Known::Entropy => "系统熵这一趟取不出可用的钥",
        Known::KeyMalformed => "取回来的那一枚钥形不对,不硬凑",
        Known::SignFailed => "这一枚签不出来",
        Known::KeystoreParams => "这一份 keystore 的 scrypt 参数不成形",
        Known::KeystoreShape => "这一份 keystore 的形不合 V3",
        Known::BadPassword => "密码不对:这一份 keystore 的校验位对不上",
        Known::Ledger => "账本目录这一侧没通过,底下的判词在证据尾里",
        Known::NoHomeDir => "环境里没有 HOME,不知道该把家放在哪",
        Known::SettingsShape => "设置档的形不对",
        Known::AddressShape => "贴回来的这一串不是一个地址",
        Known::AddressMismatch => "档里写的地址与解出来的钥对不上",
        Known::Occupied => "那条路上已经有东西,不盖",
        Known::CopyMismatch => "拷过去之后逐档核字节,有对不上的",
        Known::Landing => "落档那一步没成,底下的判词在证据尾里",
        Known::ReadOnly => "这一处家已经有一个写者开着,本实例只读",
        Known::MirrorShape => "这一束镜像的形不对",
        Known::MirrorEntry => "这一束镜像里有一条过不了重验,整束不收",
        Known::MirrorOther => "那一处放着的是另一本账的备份",
        Known::Locked => "密钥库锁着,要用钥的动作先解锁",
        Known::LockedOut => "口令连续错满,只剩助记词或密钥文件那一路",
        Known::PinWrong => "口令不对",
        Known::PinShape => "口令不合规矩",
        Known::PinSet => "这台机器上已经设过口令",
        Known::KeyboxMissing => "这台机器上还没有密钥库",
        Known::KeyboxShape => "密钥库档的形不对",
        Known::KeyboxSlot => "库里那一槽与主钥对不上号",
        Known::KeyboxNotEmpty => "库里还有东西,重置没有做",
        Known::KeyboxLocked => "密钥库那一把锁取不到",
        Known::KdfBelowFloor => "密钥库档自述的加密参数低于本机的下界,不照它开也不照它封",
        Known::MachineShape => "机器级设置档的形不对",
        Known::InsufficientFunds => "签名地址的余额不够付这一笔的费用上限",
        Known::NonceUsed => "节点说这一笔的交易序号已被用掉",
        Known::AlreadyPending => "节点说同一笔已在交易池里",
        Known::Underpriced => "节点说这一笔的出价过低",
        Known::GasTooLow => "节点说这一笔给的 gas 太少",
        Known::ContractRefused => "登记合约拒了这一笔(执行回滚)",
        Known::RateLimited => "节点限流",
        Known::MethodMissing => "这一处节点不提供这一式",
        Known::NodeAuth => "节点要凭据",
        Known::ServiceUnserved => "REST 服务不答这一问",
        Known::ServiceKeyMissing => "本机没有这一 REST 服务的钥匙",
        Known::WrongChain => "节点的链号与所选网络不符",
        Known::NodeRefused => "节点拒了,原话在证据尾里",
        Known::NodeTls => "与节点的安全连接没建成(哪一层在证据尾里)",
        Known::ProxyUnreachable => "经代理没连上(代理与它的原因在证据尾里)",
        Known::ProxyCredentialsMissing => "代理要用户名与密码,没有给(代理与它的原话在证据尾里)",
        Known::ProxyCredentialsRefused => "代理不收所给的用户名与密码(代理与它的原话在证据尾里)",
        Known::NodeTimeout => "节点在时限里没把话说完",
        Known::AnswerTooLong => "节点的答越过了字节上限",
        Known::AnswerNotJson => "节点的答不是 JSON",
        Known::SeatUnseated => "当前身份没有钥",
        Known::SeatDomain => "这一域不归当前身份签",
        Known::BackupNotLanded => "那一份密钥文件没有真落到盘上",
        Known::RecoveryNoMatch => "交进来的这一份开不回这台机器上的库",
        Known::PathRelative => "落处留空或写的是相对路径",
        Known::NoEndpoint => "还没有配任何链端点",
        Known::Unreachable => "配的端点一处也够不着",
        Known::Disagree => "端点之间对不上,承重读取不出绿",
        Known::ChainShape => "链上回来的东西形不对",
        Known::LedgerNotFetched => "导入的身份,账本还没与链上核过尾:这一席的账本对过这把钥的锚、无缺才开写",
        Known::NewerElsewhere => "链上有本世系的锚而本机没有它们的字节:别处有更新的条目,或同一位置另签过一条",
        Known::EntryRefused => "法拒了这一枚条目,拒因的 token 在证据尾里",
        Known::NoGenesis => "这本账里还没有创建账本的那一条",
        Known::ForkedRoot => "这本账里不止一个根",
        Known::AuditInput => "拼不出审计输入",
        Known::NotAdoptable => "这一处目录里有读不成或过不了核的条目,整处不收",
        Known::AlreadyRooted => "这本账已经有创建账本的那一条;一本账一个根",
        Known::RestorePartial => "这一束只落了一半,落了哪几条在证据尾里",
        Known::CannotLay => "这一处家建不出来:盘上没有落下东西",
        Known::NotAHome => "这一处既不是数据目录也不是空文件夹,不开、也不在里面建",
        Known::NoHome => "还没有开着的家",
        Known::KeyNotStored => "本机密钥库说收下了,取回来的却不是同一枚",
        Known::GitShape => "git 那一侧的字节读不成形",
        Known::NotARepo => "这一处不是一个 git 仓",
        Known::RefMissing => "这个引用在仓里找不到",
        Known::ObjectMissing => "对象库里没有这一枚",
        Known::Broken => "这本账断链了:账本转只读,先走恢复",
        Known::WouldBreak => "写这一条会让账本断链,一个字节都没写",
        Known::ContentShape => "内容哈希要三十二个字节",
        Known::QueueShape => "待锚队列那一份档的形不对",
        Known::QueueEmpty => "队列里没有挑中的条目",
        Known::NoRegistry => "还没有配喇叭合约的地址",
        Known::NoChainId => "还没有配这一处端点的链号",
        Known::GasRefused => "节点估不出这一笔的气:它会回滚",
        Known::GasNotShown => "这一批没按这个枚数估过 Gas 费",
        Known::SendFailed => "发出去了,而链上那一笔不是成功",
        Known::NotAudited => "还没有跑过一次对锚审计",
        Known::CleanWord => "清理只认 yes",
        Known::SeatUnhomed => "这一席没有账本而换新助记词会改它的地址",
        Known::AuditBehind => "上次自审之后账本又写入了,或有链没读全",
        Known::SubjectMissing => "注记指的那一条不在这本账里",
        Known::DirEmpty => "这一处目录里一个档也没有",
        Known::ScanRefused => "链上这一趟没扫全(A2 的拒因在证据尾里)",
        Known::FieldMissing => "这一格是必填的,还空着",
        Known::KitRefused => "这一包出不去(V2 的拒因在证据尾里)",
        Known::Conflict => "同一枚记录上已经有一件带独占旗的授权,窗口还相交",
        Known::HandedOver => "这本账已经承继给新钥了,这一台转只读",
        Known::NoBytes => "只见锚,未见字节",
        Known::WorkerPanicked => "后台那一趟自己塌了",
        Known::OutcomeLost => "后台那一趟做完了,结果没有回到壳",
        Known::NotHisBook => "这一叠字节不是所填那一位的账本",
        Known::BookNotWhole => "这份记录内容不是从开端起的整本账",
        Known::NotThisLedger => "这里的账不是这一条授权签发者的账,不作料",
        Known::NotAGrant => "这一枚过了法,而它不是一枚授权",
        Known::AlreadyHeld => "这一枚授权已在库里",
        Known::GrantNotHeld => "这一份授权不在保管库里,备注无处可记",
        Known::GrantDirEmpty => "这一处授权文件夹里一份档也没有",
        Known::AlreadyAnchored => "这一条已经上链,不必再排进待上链",
        Known::PayloadRefused => "这段载荷 K2 不收(它的 token 在证据尾)",
        Known::ChainIncomplete => "上游链级不到根,缺的那一枚在证据尾",
        Known::IdentityExists => "这个身份已经在这台机器上了",
        Known::IdentityHereAs => "这个身份已在本机,名字是证据尾里那一个,没有改名",
        Known::IdentityNameTaken => "本机另一个身份已用这个名字,什么也没写",
        Known::PlaceOneLine => "指处给了多行;这一页只读一处",
        Known::NoIdentity => "登记表里没有这个身份",
        Known::PhraseWords => "助记词的词数不对",
        Known::PhraseInvalid => "助记词里有词不在英文词表里,或校验位对不上",
        Known::PhraseConfirm => "抄录确认的词对不上",
        Known::NoWords => "这个身份没有助记词",
        Known::DeleteUnbacked => "这个身份没有备份过,也没有写过移交",
        Known::PasswordsDiffer => "两次输入的密码不一样",
        Known::PasswordShort => "密码太短",
        Known::PasswordLong => "密码太长,口令格收不下",
        Known::IdentitiesShape => "身份登记表的形不对",
        Known::PublishNoKit => "这处家还没有出过记录包",
        Known::TermsMismatch => "条款文件与所填摘要对不上",
        Known::GrantFileKit => "授权文件没过包验",
        Known::GrantFileBad => "授权文件拆不开",
        Known::KitFileBad => "单文件记录包拆不开",
        Known::KitFormWord => "记录包的形只认 file 或 folder",
        Known::KitOriginalsWord => "记录包的原件只认 none",
        Known::ExtractKit => "取出时记录包没过包验或拆不开",
        Known::ExtractPath => "记录包里有不合包内路径形制的路径",
        Known::ExtractNothing => "包里没有与记录对得上的原件",
        Known::ExtractClash => "两份原件要取成同一个名字",
        Known::ExtractName => "包里的名字在这台电脑上建不成",
        Known::ExtractNotFolder => "所选的不是文件夹",
        Known::ExtractExists => "所选文件夹里已有同名者",
        Known::ExtractLink => "所选文件夹里那个名字是链接",
        Known::ExtractChanged => "取出的原件与记录对不上",
        Known::ExtractLanding => "取出时写盘失败,已撤回",
        Known::AddressedNotRecipient => "没有一个本机身份开得了这份加密记录包",
        Known::AddressedBroken => "加密记录包有一段过不了认证",
        Known::AddressedHead => "加密记录包的头读不成",
        Known::AddressedElsewhere => "这是加密记录包",
        Known::AddressedKeyShape => "所填公钥不是曲线上的点",
        Known::AddressedBoth => "接收方地址与公钥都填了",
        Known::AddressedFolder => "加密只用于单文件记录包",
        Known::AddressNotAnchored => "各网络都读完了,没有这个地址自己签的锚",
        Known::AddressChainUnread => "有网络没读完,没找到这个地址的公钥",
        Known::AddressKeyUnproven => "有节点给的交易证不出自身,没找到这个地址的公钥",
        Known::RemoteKit => "取回的记录包没过包验",
        Known::RemoteStatus => "发布地址上没有这一份",
        Known::RemoteRedirect => "发布地址转向别处",
        Known::RemoteTooLarge => "取回的内容越过上限",
        Known::RemoteTimeout => "发布地址在时限里没答完",
        Known::RemoteUnreachable => "连不上发布地址",
        Known::RemoteCert => "发布地址的证书没过校验",
        Known::RemoteNotHttps => "地址不是 https",
        Known::NotAnOriginal => "这份文件不是所选记录的原件",
        Known::TraceFull => "痕迹档到了上限,此后停写",
        Known::LocalSeal => "本机数据里有一份档解不开",
        Known::RecoveryNotPrimary => "这是次要身份的助记词或密钥文件",
        Known::PrimaryDelete => "主身份不能直接删除",
        Known::PrimaryNoKeyFile => "这个身份还没导出过密钥文件",
        Known::PrimaryAlready => "它已经是主身份",
        Known::BackupPassword => "备份密码错误",
        Known::BackupNotOurs => "不是 ZIKARON 备份",
        Known::BackupTooNew => "备份来自更新的版本",
        Known::BackupShape => "备份内容不成形",
        Known::Rekeying => "正在重封或替换这一处数据,暂不收其他动作",
        Known::BackupNoIdentity => "备份里没有这个身份",
        Known::HomeUnreachable => "有一处数据目录此刻不在原处(外置盘没接上?)",
        Known::BusyForRekey => "还有后台活在跑,换主钥、恢复与取回并替换要等它落地",
        Known::DialogUnavailable => "系统的选档框在这台机器上开不了",
        Known::LocalSetAside => "旧的本机数据封在已找不回的钥下,开不了,已原样挪到一旁",
        Known::EntriesUnreadable => "这一处读到了,可里面有档读不成条目,要的那一本没取到",
        Known::NetworkListed => "这条网络已在只读网络里",
        Known::InsideHome => "搬家的新处在这处数据目录里头",
        Known::DoorShut => "命令行的门在桌面这一侧开不了",
        Known::DoorUnread => "门收到的请求读不出,没有照做",
        Known::InFlight => "同一类后台活正在跑,这一次不开始",
        Known::CliPathTaken => "命令行那一处已被别的安装占着,不覆盖",
        Known::CliPathCancelled => "系统的管理员框取消了,命令行那一处没动",
        Known::CliPathNotAllowed => "系统不许动命令行那一处",
        Known::CliPathUnsupported => "这样开的应用接不进终端的命令路径",
        Known::BatchVoided => "这一批交易作废了:没有一笔入块、没有节点还持有、nonce 已被别的交易用掉;条目已回到排队",
        Known::CredentialExpired => "命令行凭据已过期,已删去",
        Known::CredentialMissing => "这台机器上没有命令行凭据",
        Known::CredentialShape => "命令行凭据读不出,或开不了",
        Known::HomeHeld => "桌面正持着这一处数据的写者锁",
        Known::MachineHeld => "别的程序正握着本机数据目录里的一把锁",
        Known::CliAloneOn => "纯命令行写入开着,经桌面写入不可拨",
        Known::CliAloneDropped => "主钥换了,命令行凭据失效,纯命令行写入已关",
        Known::HomeOtherMachine => "另一台机器是这一处数据的写者",
        Known::CliWriting => "命令行正在写入这本账,这一步没有做",
        Known::ReadsUnkept => "这台机器没留下这一趟读链所定的部分,下一趟照旧从头读",
        Known::HandedOff => "这本账已交接到另一处,收回之前不在这里写",
        Known::OnChain => "这一批里有锚链上已有:一笔也不签不发",
        Known::TooEarly => "链头时刻未晚于上一批的块时刻:一笔也不签不发",
        Known::UpgradeHanded => "在任钥曾交出这本账:不能用它升级",
        Known::NotMoved => "这本账尚未升级到 zikaron/2,升级之前不写",
        Known::NotATarget => "收编(旧法)条目只读,不作目标",
        Known::ChainInBasis => "这条链已在基底里",
        Known::OfficeNotInWords => "这本账的现行钥不在这组助记词里,一字未写",
        Known::RotateOwed => "在任钥还有条目没上链,不换钥",
        Known::RotateBookless => "基底有链读不到换钥簿(没有簿行或码不对),不换钥",
        Known::BookCode => "换钥簿那一址的码不是家族的簿:不读不写",
        Known::NotIncluded => "这一笔已发出,等候时限内还没入块",
        Known::NoNoticeVenue => "该链无撤销合约:不签带撤销成员的授权、不发告示",
        Known::NoticeContract => "撤销合约之址的码不是它的钉:不签不发",
        Known::RootNotInBook => "这一席的根钥不在换钥簿里:入簿之前不签带撤销成员的授权",
        Known::NotAcceded => "换钥未完成接任:接任之前不签不撤",
        Known::OfficeElsewhere => "换钥簿上的现任钥不是账本的现任钥:不签不撤",
        Known::OfficeIsContract => "现任钥是合约地址:发不出告示,不签不撤",
        Known::RootKeyAbsent => "根钥不在本机:不能入簿",
        Known::FaceUnread => "面表没读成:不写面条目",
        Known::FaceNoHome => "没有世系链:不写关联、解除与显示名",
        Known::FaceRotating => "面表的在任钥不是这一席的在任钥:不写面条目",
        Known::FaceUnwritable => "面条目写不成其形:一字未写",
        Known::ConsentRefused => "同意不能用:不写",
        Known::FaceRow => "面表里这一行不容这一步",
        Known::FaceHomeChain => "这条链不能立为世系链:不写",
        Known::RegistryCode => "登记合约的码不是钉住的那一份:不带原文发送",
        Known::RotateUnseated => "有面条目未落座或面表没读:不换钥",
        Known::RotateUnentered => "有链的换钥簿没把在任钥记在根名下:不换钥",
        Known::BookClosed => "这条链的换钥簿入不了",
        Known::CredentialPassMissing => "命令行没有带程序钥",
        Known::CredentialPassUnknown => "所带的程序钥开不了凭据",
        Known::CredentialNote => "程序钥的备注不合:要一行、1 至 40 个字",
        Known::CredentialFull => "程序钥已到上限",
        Known::CredentialRowGone => "没有这一把程序钥(已删)",
        Known::CredentialReclaimed => "命令行凭据不成形(或是旧版的),已删去,纯命令行写入已关",
        Known::FaceSealed => "已换根:这个根不再写面条目",
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    class: Class,
    said: String,
    tail: String,
    say: Option<crate::lang::Key>,
    then: Option<crate::lang::Key>,
    landing: Option<zikaron_glue::pack::Trouble>,
    place: Option<String>,
    status: Option<u16>,
    script: Option<zikaron_net::PacTrouble>,
    on_chain: Option<(Vec<String>, String, u64)>,
    out: Option<String>,
    tails: Option<Box<crate::lang::Words>>,
    each: Vec<crate::lang::Words>,
    nodes: Vec<zikaron_send::Why>,
    ruled: Option<Box<Ruled>>,
    owned: Option<Box<Owned>>,
    machine: Option<String>,
    no_answer: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Owned {
    Reason(zikaron_glue::door::Own),
    NotIncluded { entry: String, ledger: String },
}

pub fn rule_words(rule: zikaron_glue::door::Rule, token: &str) -> Option<(crate::lang::Key, crate::lang::Key)> {
    use crate::lang::Key as L;
    use zikaron_glue::door::Rule;
    use zikaron_glue::retraction::{self as r, revocation as v};
    match rule {
        Rule::Revocation if token == v::Invalid::NotInLedger.token() => Some((L::RevokeNotInLedger, L::RevokeNext)),
        Rule::Revocation if token == v::Invalid::NotAGrant.token() => Some((L::RevokeNotAGrant, L::RevokeNext)),
        Rule::Retraction if token == r::Invalid::NotInLedger.token() => Some((L::RetractNotInLedger, L::RetractNext)),
        Rule::WouldBreak if token == zikaron_glue::book::Sender::Outside.as_str() => Some((L::WriteKeyNotCounted, L::WriteKeyNext)),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ruled {
    pub rule: zikaron_glue::door::Rule,
    pub token: Option<&'static str>,
    pub entry_id: Option<String>,
    pub names: Option<Vec<String>>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Class {
    Known,
    Unknown,
}

impl Class {
    pub fn as_str(self) -> &'static str {
        match self {
            Class::Known => "known",
            Class::Unknown => "unknown",
        }
    }
}

impl Fault {
    pub fn known(k: Known, tail: impl Into<crate::lang::Words>) -> Fault {
        let w: crate::lang::Words = tail.into();
        let tail = w.spoken().to_string();
        let tails = (w.zh != w.en).then(|| Box::new(w));
        Fault {
            class: Class::Known,
            said: format!("{}:{}", k.as_str(), translate(k)),
            tail,
            tails,
            each: Vec::new(),
            nodes: Vec::new(),
            ruled: None,
            owned: None,
            say: None,
            then: None,
            landing: None,
            place: None,
            status: None,
            script: None,
            on_chain: None,
            out: None,
            machine: None,
            no_answer: false,
        }
    }

    pub fn landing(code: &str, subject: impl AsRef<str>) -> Fault {
        use crate::lang::Key as L;
        let (say, then) = match code {
            "E_IO" => (Some(L::LandIoSay), Some(L::LandIoNext)),
            "E_OCCUPIED" => (Some(L::LandOccupiedSay), Some(L::LandOccupiedNext)),
            "E_BAD_PATH" => (Some(L::LandBadPathSay), Some(L::LandBadPathNext)),
            "E_DUPLICATE_PATH" => (Some(L::LandDuplicateSay), Some(L::LandDuplicateNext)),
            "E_KIT" => (Some(L::LandVerifySay), Some(L::LandVerifyNext)),
            _ => (None, None),
        };
        Fault { say, then, ..Fault::known(Known::Landing, format!("{code}: {}", subject.as_ref())) }
    }

    pub fn of_landing(t: impl Into<zikaron_glue::pack::Trouble>) -> Fault {
        let t = t.into();
        Fault { landing: Some(t.clone()), ..Fault::landing(t.code(), t.subject()) }
    }

    pub fn at_place(mut self, place: impl Into<String>) -> Fault {
        if self.place.is_none() {
            self.place = Some(place.into());
        }
        self
    }

    pub fn without_script(mut self, x: zikaron_net::PacTrouble) -> Fault {
        self.script = Some(x);
        self
    }

    pub fn retold(mut self, tail: impl Into<crate::lang::Words>) -> Fault {
        let w: crate::lang::Words = tail.into();
        self.tail = w.spoken().to_string();
        self.tails = (w.zh != w.en).then(|| Box::new(w));
        self
    }

    pub fn with_status(mut self, status: u16) -> Fault {
        self.status = Some(status);
        self
    }

    pub fn status(&self) -> Option<u16> {
        self.status
    }

    pub fn on_chain(&self) -> Option<&(Vec<String>, String, u64)> {
        self.on_chain.as_ref()
    }

    pub fn out(&self) -> Option<&str> {
        self.out.as_deref()
    }

    pub fn sent_out(self, tx: String) -> Fault {
        Fault { out: Some(tx), ..self }
    }

    pub fn place(&self) -> Option<&str> {
        self.place.as_deref()
    }

    pub fn landing_trouble(&self) -> Option<&zikaron_glue::pack::Trouble> {
        self.landing.as_ref()
    }

    pub fn entry_refused(t: zikaron::tokens::Token) -> Fault {
        Fault { say: Some(entry_token_say(t)), ..Fault::known(Known::EntryRefused, format!("{t:?}")) }
    }

    pub fn mirror_entry(name: &str, t: zikaron::tokens::Token) -> Fault {
        Fault { say: Some(entry_token_say(t)), ..Fault::known(Known::MirrorEntry, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail193, &[name, &format!("{t:?}")]))) }
    }

    pub fn scan_refused(tail: impl Into<crate::lang::Words>, rs: &[zikaron_anchor::scan::Refusal]) -> Fault {
        let over = rs.iter().find(|r| r.over_asks().is_some());
        Fault { say: over.or(rs.first()).map(scan_refusal_say), then: over.map(|_| crate::lang::Key::FaultNextOverAsks), ..Fault::known(Known::ScanRefused, tail) }
    }

    pub fn scan_tail(r: &zikaron_anchor::scan::Refusal) -> String {
        format!("{}:{}", r.code(), r.detail())
    }

    pub fn unknown(tail: impl Into<String>) -> Fault {
        let tail = tail.into();
        Fault { class: Class::Unknown, said: tail.clone(), tail, say: None, then: None, landing: None, place: None, status: None, script: None, on_chain: None, out: None, tails: None, each: Vec::new(), nodes: Vec::new(), ruled: None, owned: None, machine: None, no_answer: false }
    }

    pub fn class(&self) -> Class {
        self.class
    }

    pub fn said(&self) -> &str {
        &self.said
    }

    pub fn no_answer(self) -> Fault {
        Fault { no_answer: true, ..self }
    }

    pub fn unanswered(&self) -> bool {
        self.no_answer || self.which().is_some_and(Known::unanswered)
    }

    pub fn worded(self, say: Option<crate::lang::Key>, then: Option<crate::lang::Key>) -> Fault {
        Fault { say: say.or(self.say), then: then.or(self.then), ..self }
    }

    pub fn then_key(&self) -> Option<crate::lang::Key> {
        self.then
    }

    pub fn what_key(&self) -> Option<crate::lang::Key> {
        match (self.which(), self.say) {
            (Some(_), Some(said)) => Some(said),
            (Some(k), None) => Some(k.what()),
            (None, _) => None,
        }
    }

    pub fn human(&self) -> &str {
        match (self.which(), self.say) {
            (Some(_), Some(said)) => crate::lang::t(said),
            (Some(k), None) => crate::lang::t(k.what()),
            (None, _) => &self.said,
        }
    }

    pub fn next(&self) -> String {
        use crate::lang::{fill1, t, Key};
        if let Some(x) = self.script {
            return match x {
                zikaron_net::PacTrouble::RemoteHttp => t(Key::PacNextRemoteHttp).to_string(),
                zikaron_net::PacTrouble::Unfetched => t(Key::PacNextUnfetched).to_string(),
                other => fill1(Key::PacNextFailed, other.code()),
            };
        }
        match self.which() {
            Some(_) if self.then.is_some() => self.then.map(|then| fill1(then, &self.tail)).unwrap_or_default(),
            Some(k) => k.next().map(t).unwrap_or("").to_string(),
            None => t(Key::FaultNextUnknown).to_string(),
        }
    }

    pub fn raw(&self) -> String {
        match self.which() {
            Some(k) if self.tail.is_empty() => k.as_str().to_string(),
            Some(k) => format!("{} · {}", k.as_str(), self.tail),
            None => self.tail.clone(),
        }
    }

    pub fn shown(&self) -> String {
        if !self.nodes.is_empty() {
            return crate::chainx::plain_of(&self.nodes).join("\n");
        }
        self.tail.split(" · ").map(str::trim).filter(|p| !p.is_empty() && !code_like(p) && !p.contains("{\"")).collect::<Vec<_>>().join(" · ")
    }

    pub fn which(&self) -> Option<Known> {
        if self.class != Class::Known {
            return None;
        }
        Known::of_str(self.said.split_once(':').map(|(c, _)| c).unwrap_or(&self.said))
    }

    pub fn evidence(&self) -> String {
        format!("{} · {}", self.said, self.tail)
    }

    pub fn tail(&self) -> &str {
        &self.tail
    }

    pub fn nodes(&self) -> &[zikaron_send::Why] {
        &self.nodes
    }

    pub fn each(&self) -> Vec<&str> {
        self.each.iter().map(|w| w.spoken()).collect()
    }

    pub fn ruled(self, rule: zikaron_glue::door::Rule, token: Option<&'static str>, entry_id: Option<String>, names: Option<Vec<String>>) -> Fault {
        let words = token.and_then(|t| rule_words(rule, t));
        Fault { say: self.say.or(words.map(|w| w.0)), then: self.then.or(words.map(|w| w.1)), ruled: Some(Box::new(Ruled { rule, token, entry_id, names })), ..self }
    }

    pub fn rule(&self) -> Option<&Ruled> {
        self.ruled.as_deref()
    }

    pub fn owned(self, o: Owned) -> Fault {
        Fault { owned: Some(Box::new(o)), ..self }
    }

    pub fn owned_by(&self) -> Option<&Owned> {
        self.owned.as_deref()
    }

    pub fn in_machine_words(self, m: String) -> Fault {
        Fault { machine: Some(m), ..self }
    }

    pub fn machine(&self) -> Option<&str> {
        self.machine.as_deref()
    }

    pub fn gate_owned(self) -> Fault {
        use zikaron_glue::door::Own;
        match self.which() {
            Some(Known::LedgerNotFetched | Known::NewerElsewhere | Known::Broken) => self.owned(Owned::Reason(Own::ReadOnly)),
            Some(Known::HandedOff) => self.owned(Owned::Reason(Own::HandedOff)),
            Some(Known::NoChainId | Known::NoRegistry | Known::Disagree) => self.owned(Owned::Reason(Own::ChainUnread)),
            Some(k) if Known::NETWORK.contains(&k) => self.owned(Owned::Reason(Own::ChainUnread)),
            _ => self,
        }
    }

    pub fn tail_in(&self, l: crate::lang::Lang) -> &str {
        match &self.tails {
            Some(w) => w.of(l),
            None => &self.tail,
        }
    }

    pub fn passthrough(&self) -> bool {
        self.class == Class::Unknown && self.said == self.tail
    }
}

pub fn entry_token_say(t: zikaron::tokens::Token) -> crate::lang::Key {
    use crate::lang::Key as L;
    use zikaron::tokens::Token as T;
    match t {
        T::Utf8 => L::TokEntryUtf8,
        T::Json => L::TokEntryJson,
        T::Number => L::TokEntryNumber,
        T::Depth => L::TokEntryDepth,
        T::DupKey => L::TokEntryDupKey,
        T::KeyCharset => L::TokEntryKeyCharset,
        T::ValueCharset => L::TokEntryValueCharset,
        T::NotCanonical => L::TokEntryNotCanonical,
        T::Envelope => L::TokEntryEnvelope,
        T::EnvelopeMissing => L::TokEntryEnvelopeMissing,
        T::EnvelopeClosed => L::TokEntryEnvelopeClosed,
        T::Spec => L::TokEntrySpec,
        T::EntryType => L::TokEntryEntryType,
        T::Author => L::TokEntryAuthor,
        T::Seq => L::TokEntrySeq,
        T::Prev => L::TokEntryPrev,
        T::PrevSeq => L::TokEntryPrevSeq,
        T::Body => L::TokEntryBody,
        T::SigForm => L::TokEntrySigForm,
        T::GenesisPlace => L::TokEntryGenesisPlace,
        T::BodyField => L::TokEntryBodyField,
        T::SigV => L::TokEntrySigV,
        T::SigRange => L::TokEntrySigRange,
        T::SigHighS => L::TokEntrySigHighS,
        T::SigRecover => L::TokEntrySigRecover,
        T::SigSigner => L::TokEntrySigSigner,
    }
}

pub fn scan_refusal_say(r: &zikaron_anchor::scan::Refusal) -> crate::lang::Key {
    use crate::lang::Key as L;
    use zikaron_anchor::rpc::Broke;
    use zikaron_anchor::scan::{Asked, Refusal as R};
    match r {
        R::ChainIdMismatch { .. } => L::TokScanChainId,
        R::NoEndpoint(_) => L::TokScanNoEndpoint,
        R::Unanswered { asked: Asked::Broke { broke: Broke::NoKey { .. }, .. }, .. } => L::TokScanNoKey,
        R::Unanswered { asked: Asked::Broke { broke: Broke::Unserved { .. }, .. }, .. } => L::TokScanUnserved,
        R::Unanswered { asked: Asked::Declined { width: Some(_), .. }, .. } => L::TokScanTooWide,
        R::Unanswered { asked: Asked::OverAsks { .. }, .. } => L::TokScanOverAsks,
        R::Unanswered { .. } => L::TokScanUnanswered,
        R::TxNotItself(_) => L::TokScanTxNotItself,
        R::Malformed(_) => L::TokScanMalformed,
    }
}

pub fn whole_within_ceiling(typed: &str, not_a_number: crate::lang::Key) -> Result<u64, Fault> {
    let t = typed.trim();
    let digits = t.strip_prefix('+').unwrap_or(t);
    let past = || Fault::known(Known::SettingsShape, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::TailPastIntCeiling, &[t])));
    match t.parse::<u64>() {
        Ok(n) if n <= zikaron::json::MAX_INT => Ok(n),
        Ok(_) => Err(past()),
        Err(_) if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) => Err(past()),
        Err(_) => Err(Fault::known(Known::SettingsShape, crate::lang::both_of(|| crate::lang::filln(not_a_number, &[&format!("{t:?}")])))),
    }
}

pub fn classify(e: &std::io::Error, subject: &str) -> Fault {
    Fault::from(zikaron_seal::Trouble::io(e, subject))
}

pub fn known_of_sealed(c: zikaron_seal::Code) -> Option<Known> {
    use zikaron_seal::Code as C;
    Some(match c {
        C::FileMissing => Known::FileMissing,
        C::Denied => Known::Denied,
        C::NotAFolder => Known::NotAFolder,
        C::Unknown => return None,
        C::Locked => Known::Locked,
        C::LockedOut => Known::LockedOut,
        C::KeyboxMissing => Known::KeyboxMissing,
        C::KeyboxShape => Known::KeyboxShape,
        C::KeyboxSlot => Known::KeyboxSlot,
        C::KeyboxLocked => Known::KeyboxLocked,
        C::KeyboxNotEmpty => Known::KeyboxNotEmpty,
        C::KdfBelowFloor => Known::KdfBelowFloor,
        C::KeystoreParams => Known::KeystoreParams,
        C::KeyMalformed => Known::KeyMalformed,
        C::PinSet => Known::PinSet,
        C::PinShape => Known::PinShape,
        C::PinWrong => Known::PinWrong,
        C::RecoveryNoMatch => Known::RecoveryNoMatch,
        C::RecoveryNotPrimary => Known::RecoveryNotPrimary,
        C::LocalSeal => Known::LocalSeal,
        C::Ledger => Known::Ledger,
        C::ContentShape => Known::ContentShape,
        C::SettingsShape => Known::SettingsShape,
        C::QueueShape => Known::QueueShape,
        C::MachineShape => Known::MachineShape,
        C::IdentitiesShape => Known::IdentitiesShape,
        C::CredentialExpired => Known::CredentialExpired,
        C::CredentialMissing => Known::CredentialMissing,
        C::CredentialShape => Known::CredentialShape,
        C::HomeHeld => Known::HomeHeld,
        C::HomeOtherMachine => Known::HomeOtherMachine,
        C::HandedOff => Known::HandedOff,
        C::CredentialPassMissing => Known::CredentialPassMissing,
        C::CredentialPassUnknown => Known::CredentialPassUnknown,
        C::CredentialNote => Known::CredentialNote,
        C::CredentialFull => Known::CredentialFull,
        C::CredentialRowGone => Known::CredentialRowGone,
    })
}

fn code_like(p: &str) -> bool {
    p.len() >= 2 && p.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') && p.chars().any(|c| c.is_ascii_uppercase())
}

impl From<zikaron_seal::Trouble> for Fault {
    fn from(t: zikaron_seal::Trouble) -> Fault {
        match known_of_sealed(t.code) {
            Some(k) => Fault::known(k, t.tail),
            None => Fault::unknown(t.tail),
        }
    }
}

pub fn refusal_words(code: &str) -> String {
    Known::of_str(code).map(|k| crate::lang::t(k.what()).to_string()).unwrap_or_else(|| code.to_string())
}

pub fn known_of_sent(c: zikaron_send::Code) -> Known {
    use zikaron_send::Code as C;
    match c {
        C::NoEndpoint => Known::NoEndpoint,
        C::SettingsShape => Known::SettingsShape,
        C::Unreachable => Known::Unreachable,
        C::Disagree => Known::Disagree,
        C::ChainShape => Known::ChainShape,
        C::WrongChain => Known::WrongChain,
        C::InsufficientFunds => Known::InsufficientFunds,
        C::NonceUsed => Known::NonceUsed,
        C::AlreadyPending => Known::AlreadyPending,
        C::Underpriced => Known::Underpriced,
        C::GasTooLow => Known::GasTooLow,
        C::ContractRefused => Known::ContractRefused,
        C::RateLimited => Known::RateLimited,
        C::MethodMissing => Known::MethodMissing,
        C::NodeAuth => Known::NodeAuth,
        C::ServiceUnserved => Known::ServiceUnserved,
        C::ServiceKeyMissing => Known::ServiceKeyMissing,
        C::NodeRefused => Known::NodeRefused,
        C::NodeTls => Known::NodeTls,
        C::NodeTimeout => Known::NodeTimeout,
        C::AnswerTooLong => Known::AnswerTooLong,
        C::AnswerNotJson => Known::AnswerNotJson,
        C::ProxyUnreachable => Known::ProxyUnreachable,
        C::ProxyCredentialsMissing => Known::ProxyCredentialsMissing,
        C::ProxyCredentialsRefused => Known::ProxyCredentialsRefused,
        C::GasRefused => Known::GasRefused,
        C::KeyMalformed => Known::KeyMalformed,
        C::AddressMismatch => Known::AddressMismatch,
        C::ContentShape => Known::ContentShape,
        C::QueueEmpty => Known::QueueEmpty,
        C::AuditInput => Known::AuditInput,
        C::ScanRefused => Known::ScanRefused,
        C::NoChainId => Known::NoChainId,
        C::NoRegistry => Known::NoRegistry,
        C::OnChain => Known::OnChain,
        C::TooEarly => Known::TooEarly,
        C::Broken => Known::Broken,
        C::HandedOff => Known::HandedOff,
        C::NotFetched => Known::LedgerNotFetched,
        C::NewerElsewhere => Known::NewerElsewhere,
        C::NotIncluded => Known::NotIncluded,
        C::NoNoticeVenue => Known::NoNoticeVenue,
        C::NoticeContract => Known::NoticeContract,
        C::RootNotInBook => Known::RootNotInBook,
        C::NotAcceded => Known::NotAcceded,
        C::OfficeElsewhere => Known::OfficeElsewhere,
        C::OfficeIsContract => Known::OfficeIsContract,
        C::BookCode => Known::BookCode,
        C::RegistryCode => Known::RegistryCode,
    }
}

impl From<zikaron_send::Trouble> for Fault {
    fn from(t: zikaron_send::Trouble) -> Fault {
        let f = match (&t.code, &t.why) {
            (zikaron_send::Code::ScanRefused, zikaron_send::Why::Scan { refused, .. }) => Fault::scan_refused(crate::lang::both_of(|| crate::chainx::said_of(&t.why)), refused),
            (zikaron_send::Code::OnChain, zikaron_send::Why::OnChain { names, tx, block }) => {
                Fault { on_chain: Some((names.clone(), tx.clone(), *block)), ..Fault::known(Known::OnChain, crate::lang::both_of(|| crate::chainx::said_of(&t.why))) }
            }
            _ => Fault::known(known_of_sent(t.code), crate::lang::both_of(|| crate::chainx::said_of(&t.why))),
        };
        let f = Fault { each: nodes_said(&t.why).iter().map(|w| crate::lang::both_of(|| crate::chainx::said_of(w))).collect(), nodes: nodes_said(&t.why).to_vec(), machine: Some(zikaron_send::said(&t.why)), ..f };
        let f = if f.which() == Some(Known::Unreachable) && reached_and_refused(nodes_said(&t.why)) { f.worded(Some(crate::lang::Key::FaultWhatUnanswered), Some(crate::lang::Key::FaultNextUnanswered)) } else { f };
        match t.script {
            Some(x) => f.without_script(x),
            None => f,
        }
    }
}

pub fn reached_and_refused(nodes: &[zikaron_send::Why]) -> bool {
    nodes.iter().any(|w| matches!(w, zikaron_send::Why::Node { trouble: zikaron_anchor::rpc::Trouble::Node(_), .. }))
}

fn nodes_said(w: &zikaron_send::Why) -> &[zikaron_send::Why] {
    match w {
        zikaron_send::Why::Each(xs) => xs,
        zikaron_send::Why::AtChain { why, .. } => nodes_said(why),
        zikaron_send::Why::Node { trouble: zikaron_anchor::rpc::Trouble::Node(_), .. } | zikaron_send::Why::Scan { .. } => std::slice::from_ref(w),
        _ => &[],
    }
}

impl zikaron_seal::Coded for Fault {
    fn sealed_code(&self) -> Option<zikaron_seal::Code> {
        use zikaron_seal::Code as C;
        let k = self.which()?;
        [
            C::FileMissing, C::Denied, C::NotAFolder, C::Locked, C::LockedOut, C::KeyboxMissing, C::KeyboxShape, C::KeyboxSlot, C::KeyboxLocked,
            C::KeyboxNotEmpty, C::KdfBelowFloor, C::KeystoreParams, C::KeyMalformed, C::PinSet, C::PinShape, C::PinWrong,
            C::RecoveryNoMatch, C::RecoveryNotPrimary, C::LocalSeal, C::Ledger, C::ContentShape, C::SettingsShape, C::QueueShape,
            C::MachineShape, C::IdentitiesShape, C::CredentialExpired, C::CredentialMissing, C::CredentialShape, C::HomeHeld, C::HomeOtherMachine,
            C::HandedOff, C::CredentialPassMissing, C::CredentialPassUnknown, C::CredentialNote, C::CredentialFull, C::CredentialRowGone,
        ]
        .into_iter()
        .find(|c| known_of_sealed(*c) == Some(k))
    }

    fn sealed_tail(&self) -> &str {
        self.tail()
    }
}

#[cfg(test)]
mod unanswered_tests {
    use super::*;

    #[test]
    fn a_node_reached_that_refused_is_never_said_unreachable() {
        use zikaron_anchor::rpc::{Broke, Trouble as R};
        use zikaron_send::{Code, Trouble, Why};
        let closed = |n: &str| Why::Node { node: n.into(), trouble: R::Transport(Broke::Closed { node: n.into() }) };
        let refused = |n: &str| Why::Node { node: n.into(), trouble: R::Node("{\"code\":-32000,\"message\":\"logs unavailable\"}".into()) };
        let none: Fault = Trouble::new(Code::Unreachable, Why::Each(vec![closed("http://a"), closed("http://b")])).into();
        let one: Fault = Trouble::new(Code::Unreachable, Why::Each(vec![closed("http://a"), refused("http://b")])).into();
        let only: Fault = Trouble::new(Code::Unreachable, Why::Each(vec![refused("http://b")])).into();
        for f in [&none, &one, &only] {
            assert_eq!(f.which(), Some(Known::Unreachable), "the name stays: {f:?}");
        }
        assert_eq!((none.what_key(), none.next()), (Some(crate::lang::Key::FaultWhatUnreachable), crate::lang::t(crate::lang::Key::FaultNextUnreachable).to_string()));
        for f in [&one, &only] {
            assert_eq!((f.what_key(), f.next()), (Some(crate::lang::Key::FaultWhatUnanswered), crate::lang::t(crate::lang::Key::FaultNextUnanswered).to_string()), "{f:?}");
        }
        assert_eq!(one.nodes().len(), 2, "each node in the details, as before");
        assert!(!reached_and_refused(&[closed("http://a")]) && reached_and_refused(&[closed("http://a"), refused("http://b")]));
        let named: Fault = Trouble::new(Code::NodeRefused, Why::Each(vec![refused("http://b")])).into();
        assert_eq!(named.what_key(), Some(crate::lang::Key::FaultWhatNodeRefused));
    }
}

#[cfg(test)]
mod shown_tests {
    use super::*;

    #[test]
    fn every_refusal_whose_next_step_points_to_the_nodes_words_shows_them() {
        use zikaron_anchor::rpc::Trouble as R;
        use zikaron_send::{Code, Trouble, Why};
        let words = "{\"code\":-32000,\"message\":\"account service is closed for maintenance\"}";
        let node = "http://127.0.0.1:5";
        let refused = || Why::Node { node: node.into(), trouble: R::Node(words.into()) };
        let pointing: Vec<Known> = Known::ALL.iter().copied().filter(|k| k.next() == Some(crate::lang::Key::FaultNextNodeRefused)).collect();
        assert!(!pointing.is_empty());
        for k in pointing {
            let codes: Vec<Code> = Code::ALL.iter().copied().filter(|c| known_of_sent(*c) == k).collect();
            assert!(!codes.is_empty(), "{k:?} comes from a node's answer");
            for c in codes {
                for why in [refused(), Why::Each(vec![refused()]), Why::AtChain { chain: 31337, why: Box::new(refused()) }] {
                    let f: Fault = Trouble::new(c, why.clone()).into();
                    assert!(f.shown().contains(words), "{c:?} {why:?}: {:?}", f.shown());
                }
            }
        }
        let nonce: Fault = zikaron_send::node::said_trouble(node, &R::Node(words.into())).into();
        assert_eq!(nonce.next(), crate::lang::t(crate::lang::Key::FaultNextNodeRefused), "the refused question for the nonce");
        assert!(nonce.shown().contains(words), "{:?}", nonce.shown());
    }

    #[test]
    fn a_node_giving_way_by_the_cap_shows_why_and_what_it_said_last() {
        use zikaron_anchor::scan::{Asked, Refusal};
        use zikaron_send::{Code, Trouble, Why};
        let words = "{\"code\":-32000,\"message\":\"query exceeds max block range 2\"}";
        let r = Refusal::Unanswered { chain: 31337, asked: Asked::OverAsks { cap: 7, said: Some(words.into()) } };
        let line = format!("http://127.0.0.1:5 {}", zikaron_anchor::send::NotSaid::Logs(r.clone()).evidence());
        let f: Fault = Trouble::new(Code::ScanRefused, Why::Scan { unanswered: vec![Why::Text(line)], refused: vec![r.clone()] }).into();
        assert!(f.shown().contains("OVER_ASKS 7") && f.shown().contains(words), "{:?}", f.shown());
        assert_eq!(f.next(), crate::lang::t(crate::lang::Key::FaultNextOverAsks));
        let alone: Fault = Trouble::new(Code::ScanRefused, Why::Scan { unanswered: Vec::new(), refused: vec![r] }).into();
        assert!(alone.shown().contains("OVER_ASKS 7") && alone.shown().contains(words), "{:?}", alone.shown());
    }

    #[test]
    fn the_details_shown_carry_values_never_codes() {
        let missing = Fault::known(Known::FileMissing, "/p/a.txt");
        assert_eq!(missing.shown(), "/p/a.txt");
        assert!(missing.raw().starts_with(Known::FileMissing.as_str()), "the code stays in raw");
        assert_eq!(Fault::known(Known::FileMissing, "NOT_RECIPIENT").shown(), "", "a token alone shows nothing");
        assert_eq!(Fault::unknown(r#"NO_BYTES · /p/b · {"code":-32012,"message":"x"}"#).shown(), "/p/b");
    }
}

#[cfg(test)]
mod next_tests {
    use super::*;

    fn sources(at: &std::path::Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(at).expect("the source reads").flatten() {
            let p = e.path();
            if p.is_dir() {
                sources(&p, out);
            } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                let text = std::fs::read_to_string(&p).expect("a source file reads");
                out.push(text.split("#[cfg(test)]").next().unwrap_or_default().to_string());
            }
        }
    }

    fn args_of(call: &str) -> Vec<String> {
        let (mut depth, mut cur, mut out) = (0usize, String::new(), Vec::new());
        for ch in call.chars() {
            match ch {
                '(' | '[' | '{' => {
                    depth += 1;
                    cur.push(ch);
                }
                ')' | ']' | '}' if depth == 0 => break,
                ')' | ']' | '}' => {
                    depth -= 1;
                    cur.push(ch);
                }
                ',' if depth == 0 => out.push(std::mem::take(&mut cur)),
                _ => cur.push(ch),
            }
        }
        out.push(cur);
        out.into_iter().map(|a| a.trim().to_string()).collect()
    }

    fn key_named(arg: &str) -> Option<String> {
        let inner = arg.strip_prefix("Some(")?.strip_suffix(')')?;
        Some(inner.rsplit("::").next()?.trim().to_string())
    }

    fn then_keys() -> Vec<String> {
        let mut files = Vec::new();
        sources(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut files);
        let mut names = Vec::new();
        for f in &files {
            for (i, _) in f.match_indices(".worded(") {
                if let Some(name) = args_of(&f[i + ".worded(".len()..]).get(1).and_then(|a| key_named(a)) {
                    names.push(name);
                }
            }
        }
        let fault = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/fault.rs")).expect("fault.rs");
        let table = fault.split("pub fn rule_words(").nth(1).and_then(|b| b.split("\n}\n").next()).unwrap_or_default();
        for (i, _) in table.match_indices("Some((") {
            if let Some(second) = args_of(&table[i + "Some((".len()..]).get(1) {
                names.push(second.rsplit("::").next().unwrap_or_default().to_string());
            }
        }
        names.sort();
        names.dedup();
        names
    }

    #[test]
    fn every_next_step_handed_as_then_is_said_whole_with_the_faults_own_evidence() {
        let names = then_keys();
        assert!(names.iter().any(|n| n == "ExitBehindSay"), "the reading finds the then keys: {names:?}");
        for name in &names {
            let key = crate::lang::Key::ALL.iter().copied().find(|k| format!("{k:?}") == *name).unwrap_or_else(|| panic!("{name} is handed as a then and is no key"));
            let said = Fault::known(Known::Broken, "7").worded(None, Some(key)).next();
            let slot = said.match_indices('{').any(|(i, _)| said[i + 1..].split('}').next().is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit())));
            assert!(!slot && !said.is_empty(), "{name}: {said}");
        }
        let behind = Fault::known(Known::Broken, "7").worded(Some(crate::lang::Key::ExitBehind), Some(crate::lang::Key::ExitBehindSay));
        assert_eq!(behind.next(), crate::lang::fill1(crate::lang::Key::ExitBehindSay, "7"));
    }
}
