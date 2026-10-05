//! Errors in three parts: known ones are translated, unknown ones pass through, and the evidence tail is
//! always kept.
//!
//! 1. Known errors live in one closed table, `Known`, with plain words from `translate` only. An error
//! outside the table is never guessed into it: `classify` recognizes only what it knows, and everything else
//! goes to the unknown branch.
//! 2. Unknown errors pass through unchanged: the sentence on screen equals the one from below byte for byte.
//! 3. Evidence tail: both branches carry `tail`, and both constructors of `Fault` require it, so losing the
//! original return cannot be written.

use std::io::ErrorKind;

/// The closed table of known errors. It grows together with its plain words.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Known {
    /// The system lacks this font face.
    FontMissing,
    /// The file is not on disk.
    FileMissing,
    /// No permission.
    Denied,
    /// The local key vault does not hold this key.
    KeychainMissing,
    /// The system's entropy gave no usable key.
    Entropy,
    /// The key read back has the wrong shape.
    KeyMalformed,
    /// Could not sign.
    SignFailed,
    /// Keystore scrypt parameters are malformed.
    KeystoreParams,
    /// The keystore does not have the V3 shape.
    KeystoreShape,
    /// Wrong password (the MAC does not match).
    BadPassword,
    /// Trouble on the ledger directory side (the storage verdict carried in the evidence tail).
    Ledger,
    /// The environment has no home directory.
    NoHomeDir,
    /// The settings file has the wrong shape.
    SettingsShape,
    /// This string is not an address.
    AddressShape,
    /// The address in the keystore file does not match the decrypted key.
    AddressMismatch,
    /// Something is already at that path.
    Occupied,
    /// Copied files do not match byte for byte.
    CopyMismatch,
    /// Landing a file failed (with the storage or kit output verdict unchanged).
    Landing,
    /// This instance is a reader and does not write.
    ReadOnly,
    /// The mirror bundle has the wrong shape.
    MirrorShape,
    /// An entry in the mirror bundle fails re-verification.
    MirrorEntry,
    /// That place holds a backup of another ledger.
    MirrorOther,
    /// The key vault is locked.
    Locked,
    /// Too many failures; only recovery by words or key file remains.
    LockedOut,
    /// Wrong passcode (the tail carries tries left).
    PinWrong,
    /// The passcode breaks the rules (the tail says which).
    PinShape,
    /// A passcode is already set on this machine.
    PinSet,
    /// This machine has no key vault yet.
    KeyboxMissing,
    /// The key vault file has the wrong shape.
    KeyboxShape,
    /// A vault slot does not match the master key.
    KeyboxSlot,
    /// The vault still holds something (a slot or recovery seal), so it is not reset.
    KeyboxNotEmpty,
    /// The vault file's lock could not be taken (the kernel refused).
    KeyboxLocked,
    /// The key vault file records key-derivation parameters below this binary's floor.
    KdfBelowFloor,
    /// The machine settings file has the wrong shape (`machine.rs`).
    MachineShape,
    /// The balance does not cover fee cap times gas (the pre-send check, or the node at broadcast).
    InsufficientFunds,
    /// The node says the nonce is used.
    NonceUsed,
    /// The node says the same transaction is already pending.
    AlreadyPending,
    /// The node says the price is too low.
    Underpriced,
    /// The node says gas is too low.
    GasTooLow,
    /// The contract refused (execution reverted).
    ContractRefused,
    /// The node is rate-limiting.
    RateLimited,
    /// The node does not provide this method.
    MethodMissing,
    /// The node requires credentials.
    NodeAuth,
    /// Wrong chain id.
    WrongChain,
    /// The node refused and the reason is not recognized; its words are in the evidence tail.
    NodeRefused,
    /// One of the four TLS layers failed (the layer is in the evidence tail).
    NodeTls,
    /// This call's total deadline passed.
    NodeTimeout,
    /// The node's answer passed its end.
    AnswerTooLong,
    /// The node's answer is not JSON.
    AnswerNotJson,
    /// The current seat is empty: this identity's key is not on this seat (one key, one seat).
    SeatUnseated,
    /// This domain is not signed by the current seat (the seat × domain table).
    SeatDomain,
    /// The backup key file did not land on disk (read back after writing, it does not match).
    BackupNotLanded,
    /// The given words or key file do not open this machine's vault.
    RecoveryNoMatch,
    /// A location (backup, moving a home) is empty or relative.
    PathRelative,
    /// No endpoint at all.
    NoEndpoint,
    /// No endpoint reachable.
    Unreachable,
    /// Endpoints disagree (no green without agreement).
    Disagree,
    /// The chain returned the wrong shape.
    ChainShape,
    /// This ledger holds the pen: after a restore, one anchor reconciliation must report COMPLETE.
    PenHeld,
    /// A restored identity whose ledger is not fetched yet: writes are refused until the full ledger is
    /// fetched and its tail checked.
    LedgerNotFetched,
    /// The fetched ledger does not match this identity's anchors on chain: newer entries exist elsewhere.
    NewerElsewhere,
    /// The law refused this entry (token in the evidence tail).
    EntryRefused,
    /// This ledger has no genesis entry.
    NoGenesis,
    /// This ledger has more than one root.
    ForkedRoot,
    /// The audit input cannot be assembled.
    AuditInput,
    /// This directory cannot be adopted (entries unreadable or failing the core).
    NotAdoptable,
    /// This ledger already has a genesis entry.
    AlreadyRooted,
    /// The restore landed only partly; which entries landed is named in the evidence tail.
    RestorePartial,
    /// This home could not be created (nothing landed on disk).
    CannotLay,
    /// The folder chosen as the data folder is neither a data folder nor empty (named in the evidence tail).
    NotAHome,
    /// No home is open yet.
    NoHome,
    /// The key vault accepted the key but cannot return the same one.
    KeyNotStored,
    /// Git bytes cannot be read (a broken index, an undecodable object, an unappliable delta).
    GitShape,
    /// This is not a git repository.
    NotARepo,
    /// The repository has no such reference.
    RefMissing,
    /// The object store lacks this object (loose or packed).
    ObjectMissing,
    /// The audit reports BROKEN_CHAIN: writes are locked and the whole app is read-only.
    Broken,
    /// The content hash is malformed (law §6.2 requires hex32).
    ContentShape,
    /// The anchor queue file has the wrong shape.
    QueueShape,
    /// Nothing in the queue was chosen.
    QueueEmpty,
    /// No registry contract address configured yet.
    NoRegistry,
    /// No chain id configured yet.
    NoChainId,
    /// The node refused at gas estimation: sending would revert.
    GasRefused,
    /// Gas was not estimated for this batch, or was estimated for another count.
    GasNotShown,
    /// Sent, and the receipt says it did not succeed (law §9.1: status other than 1 is no anchor).
    SendFailed,
    /// No anchor audit has run yet; this field has no reading.
    NotAudited,
    /// The entry an annotation points to is not in this ledger (the subject of law §6.8).
    SubjectMissing,
    /// This directory holds no file: no manifest hash can be computed.
    DirEmpty,
    /// The anchoring crate says this scan did not read everything (its refusal carried in the evidence tail).
    ScanRefused,
    /// A required form field is empty.
    FieldMissing,
    /// A checklist step was skipped (steps cannot be skipped).
    StepSkipped,
    /// Kit output was refused by the kit output crate or the kit core; the refusal is in the evidence tail.
    KitRefused,
    /// The double-sale guard hit: same record, overlapping windows, and an existing grant with the local
    /// exclusive flag.
    Conflict,
    /// The cosignature failed verification (law §6.6; the core's token in the evidence tail).
    CosignRefused,
    /// A succession is anchored: this desk handed the ledger over and is read-only (law §7.3).
    HandedOver,
    /// Anchors seen, bytes not: this side cannot get that ledger's bytes.
    NoBytes,
    /// A background task crashed (thread panic). A crash still gets a sentence: otherwise that kind would
    /// stay "in flight" and the screen would keep saying it never ran.
    WorkerPanicked,
    /// A background run finished and its outcome never reached the frame (named, never waited on forever).
    OutcomeLost,
    /// These bytes are not the named person's ledger (the genesis author does not match).
    NotHisBook,
    /// Material was read at a level of the audit input, and it is not the ledger that holds this grant
    /// (another issuer's): not used, and said by name.
    NotThisLedger,
    /// The bytes pass the law but are not a grant (the vault holds grants only).
    NotAGrant,
    /// This grant is already in the vault.
    AlreadyHeld,
    /// A note was given for a grant that is not in the vault (its id in the evidence tail).
    GrantNotHeld,
    /// The folder chosen for "import grant folder" holds no file at all (only the system's side files, or nothing).
    GrantDirEmpty,
    /// This entry is already on chain (queueing it again would only pay gas again).
    AlreadyAnchored,
    /// The kit core does not accept this payload (the kit law §6 token in the evidence tail).
    PayloadRefused,
    /// The upstream chain does not reach a root: the tail names the grant found neither in the vault nor in
    /// this seat's ledger.
    ChainIncomplete,
    IdentityExists,
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
    /// Checking publication needs a local kit to compare with.
    PublishNoKit,
    /// Checked at signing: the kept copy is the one signed into the grant.
    TermsMismatch,
    /// Unpacked, it is an enumeration handed to the same kit verification.
    GrantFileKit,
    /// The single-file container has the wrong shape (magic, truncation, order, bounds).
    GrantFileBad,
    /// Fetched file by file from the manifest and handed to the same kit verification.
    RemoteKit,
    /// The remote answered a status other than 200.
    RemoteStatus,
    /// Redirects are allowed only to the same https origin.
    RemoteRedirect,
    /// A single file or the total passed the cap.
    RemoteTooLarge,
    /// The remote fetch's total deadline passed.
    RemoteTimeout,
    /// Host resolution, connection or handshake failed.
    RemoteUnreachable,
    /// The certificate chain or host name does not match; always verified, with no way to skip.
    RemoteCert,
    /// Remote fetch and publication addresses are https only.
    RemoteNotHttps,
    /// The attachment's digest is not among the chosen records' contents: a kit takes only the originals of
    /// the chosen records.
    NotAnOriginal,
    /// The trace file reached its byte cap; writing stops.
    TraceFull,
    /// The pasted claim text cannot be read (header, hex, three members, canonical form).
    ClaimShape,
    /// A sealed local file does not open with this machine's local data key (sealed under another master key, or altered).
    LocalSeal,
    /// Migrating plain local files: a sealed copy did not open back to the same bytes; migration stopped and no plain file was deleted.
    MigrateMismatch,
    /// The recovery secret belongs to a secondary identity: only the primary identity recovers the passcode.
    RecoveryNotPrimary,
    /// The primary identity cannot be deleted directly.
    PrimaryDelete,
    /// An imported-key identity must have exported its key file before it becomes primary.
    PrimaryNoKeyFile,
    /// The identity is already the primary identity.
    PrimaryAlready,
    /// The backup password does not open this backup (not counted as a passcode failure).
    BackupPassword,
    /// The file is not a ZIKARON Desk backup.
    BackupNotOurs,
    /// The backup's format version is newer than this app reads.
    BackupTooNew,
    /// The backup opened but its contents are not in the shape this version writes.
    BackupShape,
    /// Resealing under a new master key is in progress; no other action is taken meanwhile.
    Rekeying,
    /// The backup does not hold the identity asked for.
    BackupNoIdentity,
    /// A data folder the register names is not where it was (its place is gone, e.g. a disk not attached): a change that must reseal every local file refuses rather than leave that folder behind.
    HomeUnreachable,
    /// Another background task is still running: changing the master key or restoring waits for it to land, so nothing is written under the key being replaced.
    BusyForRekey,
    /// The system file dialog could not open on this machine (on Linux: the desktop portal and its zenity fallback both failed, as the dialog itself reports), said instead of reading as a cancel.
    DialogUnavailable,
    /// Local files sealed under a master key that is gone (the key store was reset or lost) were moved aside, byte for byte, before a new passcode made a new master key; said once.
    LocalSetAside,
    /// A place was read but none of the material wanted could be taken from it, and some files there could not be read as entries: named with the place, how many and the first one's reason, never dropped silently.
    EntriesUnreadable,
    /// A read-only network with the same chain and registry contract is already in the table (or is the main
    /// network): refused by name, the table unchanged.
    NetworkListed,
    /// The new place of a move is this home's root or lies under it (real paths, links resolved): refused before
    /// one byte is copied, the old home unwritten.
    InsideHome,
}

impl Known {
    /// The network kind's members, listed only here. The status line sorts by it: members go to the status
    /// line, not notices. It covers the transport families `chainx::said_fault` dispatches and the node
    /// refusals a read can meet; a member added to that dispatch but not here would be missed by the status
    /// line.
    pub const NETWORK: [Known; 14] = [
        Known::Unreachable,
        Known::NoEndpoint,
        Known::Disagree,
        Known::ScanRefused,
        Known::ChainShape,
        Known::NodeTls,
        Known::NodeTimeout,
        Known::AnswerTooLong,
        Known::AnswerNotJson,
        Known::RateLimited,
        Known::MethodMissing,
        Known::NodeAuth,
        Known::WrongChain,
        Known::NodeRefused,
    ];

    pub const ALL: [Known; 152] = [
        Known::FontMissing,
        Known::FileMissing,
        Known::Denied,
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
        Known::WrongChain,
        Known::NodeRefused,
        Known::NodeTls,
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
        Known::PenHeld,
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
        Known::ContentShape,
        Known::QueueShape,
        Known::QueueEmpty,
        Known::NoRegistry,
        Known::NoChainId,
        Known::GasRefused,
        Known::GasNotShown,
        Known::SendFailed,
        Known::NotAudited,
        Known::SubjectMissing,
        Known::DirEmpty,
        Known::ScanRefused,
        Known::FieldMissing,
        Known::StepSkipped,
        Known::KitRefused,
        Known::Conflict,
        Known::CosignRefused,
        Known::HandedOver,
        Known::NoBytes,
        Known::WorkerPanicked,
        Known::OutcomeLost,
        Known::NotHisBook,
        Known::NotThisLedger,
        Known::NotAGrant,
        Known::AlreadyHeld,
        Known::GrantNotHeld,
        Known::GrantDirEmpty,
        Known::AlreadyAnchored,
        Known::PayloadRefused,
        Known::ChainIncomplete,
        Known::IdentityExists,
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
        Known::ClaimShape,
        Known::LocalSeal,
        Known::MigrateMismatch,
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
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Known::FontMissing => "FONT_MISSING",
            Known::FileMissing => "FILE_MISSING",
            Known::Denied => "DENIED",
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
            Known::WrongChain => "WRONG_CHAIN",
            Known::NodeRefused => "NODE_REFUSED",
            Known::NodeTls => "NODE_TLS",
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
            Known::PenHeld => "PEN_HELD",
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
            Known::ContentShape => "CONTENT_SHAPE",
            Known::QueueShape => "QUEUE_SHAPE",
            Known::QueueEmpty => "QUEUE_EMPTY",
            Known::NoRegistry => "NO_REGISTRY",
            Known::NoChainId => "NO_CHAIN_ID",
            Known::GasRefused => "GAS_REFUSED",
            Known::GasNotShown => "GAS_NOT_ESTIMATED",
            Known::SendFailed => "SEND_FAILED",
            Known::NotAudited => "NOT_AUDITED",
            Known::SubjectMissing => "SUBJECT_MISSING",
            Known::DirEmpty => "DIR_EMPTY",
            Known::ScanRefused => "SCAN_REFUSED",
            Known::FieldMissing => "FIELD_MISSING",
            Known::StepSkipped => "STEP_SKIPPED",
            Known::KitRefused => "KIT_REFUSED",
            Known::Conflict => "CONFLICT",
            Known::CosignRefused => "COSIGN_REFUSED",
            Known::HandedOver => "HANDED_OVER",
            Known::NoBytes => "NO_BYTES",
            Known::WorkerPanicked => "WORKER_PANICKED",
            Known::OutcomeLost => "OUTCOME_LOST",
            Known::NotHisBook => "NOT_HIS_BOOK",
            Known::NotThisLedger => "NOT_THIS_LEDGER",
            Known::NotAGrant => "ONLY_GRANTS_HELD",
            Known::AlreadyHeld => "ALREADY_HELD",
            Known::GrantNotHeld => "GRANT_NOT_HELD",
            Known::GrantDirEmpty => "GRANT_DIR_EMPTY",
            Known::AlreadyAnchored => "ALREADY_ANCHORED",
            Known::PayloadRefused => "PAYLOAD_REFUSED",
            Known::ChainIncomplete => "CHAIN_UNREACHED",
            Known::IdentityExists => "IDENTITY_EXISTS",
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
            Known::ClaimShape => "CLAIM_SHAPE",
            Known::LocalSeal => "LOCAL_SEAL",
            Known::MigrateMismatch => "MIGRATE_MISMATCH",
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
        }
    }
}


/// The two sentences on screen (what happened, what next), from the string table in the interface language,
/// one pair per member. `translate` gives the evidence words used by tests and raw errors (format stable), not
/// shown at the first level.
impl Known {
    pub fn what(self) -> crate::lang::Key {
        use crate::lang::Key as L;
        match self {
            Known::FontMissing => L::FaultWhatFontMissing,
            Known::FileMissing => L::FaultWhatFileMissing,
            Known::Denied => L::FaultWhatDenied,
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
            Known::WrongChain => L::FaultWhatWrongChain,
            Known::NodeRefused => L::FaultWhatNodeRefused,
            Known::NodeTls => L::FaultWhatNodeTls,
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
            Known::PenHeld => L::FaultWhatPenHeld,
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
            Known::ContentShape => L::FaultWhatContentShape,
            Known::QueueShape => L::FaultWhatQueueShape,
            Known::QueueEmpty => L::FaultWhatQueueEmpty,
            Known::NoRegistry => L::FaultWhatNoRegistry,
            Known::NoChainId => L::FaultWhatNoChainId,
            Known::GasRefused => L::FaultWhatGasRefused,
            Known::GasNotShown => L::FaultWhatGasNotShown,
            Known::SendFailed => L::FaultWhatSendFailed,
            Known::NotAudited => L::FaultWhatNotAudited,
            Known::SubjectMissing => L::FaultWhatSubjectMissing,
            Known::DirEmpty => L::FaultWhatDirEmpty,
            Known::ScanRefused => L::FaultWhatScanRefused,
            Known::FieldMissing => L::FaultWhatFieldMissing,
            Known::StepSkipped => L::FaultWhatStepSkipped,
            Known::KitRefused => L::FaultWhatKitRefused,
            Known::Conflict => L::FaultWhatConflict,
            Known::CosignRefused => L::FaultWhatCosignRefused,
            Known::HandedOver => L::FaultWhatHandedOver,
            Known::NoBytes => L::FaultWhatNoBytes,
            Known::WorkerPanicked => L::FaultWhatWorkerPanicked,
            Known::OutcomeLost => L::FaultWhatOutcomeLost,
            Known::NotHisBook => L::FaultWhatNotHisBook,
            Known::NotThisLedger => L::FaultWhatNotThisLedger,
            Known::NotAGrant => L::FaultWhatNotAGrant,
            Known::AlreadyHeld => L::FaultWhatAlreadyHeld,
            Known::GrantNotHeld => L::FaultWhatGrantNotHeld,
            Known::GrantDirEmpty => L::FaultWhatGrantDirEmpty,
            Known::AlreadyAnchored => L::FaultWhatAlreadyAnchored,
            Known::PayloadRefused => L::FaultWhatPayloadRefused,
            Known::ChainIncomplete => L::FaultWhatChainIncomplete,
            Known::IdentityExists => L::FaultWhatIdentityExists,
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
            Known::ClaimShape => L::FaultWhatClaimShape,
            Known::LocalSeal => L::FaultWhatLocalSeal,
            Known::MigrateMismatch => L::FaultWhatMigrateMismatch,
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
        }
    }

    pub fn next(self) -> crate::lang::Key {
        use crate::lang::Key as L;
        match self {
            Known::FontMissing => L::FaultNextFontMissing,
            Known::FileMissing => L::FaultNextFileMissing,
            Known::Denied => L::FaultNextDenied,
            Known::KeychainMissing => L::FaultNextKeychainMissing,
            Known::Entropy => L::FaultNextEntropy,
            Known::KeyMalformed => L::FaultNextKeyMalformed,
            Known::SignFailed => L::FaultNextSignFailed,
            Known::KeystoreParams => L::FaultNextKeystoreParams,
            Known::KeystoreShape => L::FaultNextKeystoreShape,
            Known::BadPassword => L::FaultNextBadPassword,
            Known::Ledger => L::FaultNextLedger,
            Known::NoHomeDir => L::FaultNextNoHomeDir,
            Known::SettingsShape => L::FaultNextSettingsShape,
            Known::AddressShape => L::FaultNextAddressShape,
            Known::AddressMismatch => L::FaultNextAddressMismatch,
            Known::Occupied => L::FaultNextOccupied,
            Known::CopyMismatch => L::FaultNextCopyMismatch,
            Known::Landing => L::FaultNextLanding,
            Known::ReadOnly => L::FaultNextReadOnly,
            Known::MirrorShape => L::FaultNextMirrorShape,
            Known::MirrorEntry => L::FaultNextMirrorEntry,
            Known::MirrorOther => L::FaultNextMirrorOther,
            Known::Locked => L::FaultNextLocked,
            Known::LockedOut => L::FaultNextLockedOut,
            Known::PinWrong => L::FaultNextPinWrong,
            Known::PinShape => L::FaultNextPinShape,
            Known::PinSet => L::FaultNextPinSet,
            Known::KeyboxMissing => L::FaultNextKeyboxMissing,
            Known::KeyboxShape => L::FaultNextKeyboxShape,
            Known::KeyboxSlot => L::FaultNextKeyboxSlot,
            Known::KeyboxNotEmpty => L::FaultNextKeyboxNotEmpty,
            Known::KeyboxLocked => L::FaultNextKeyboxLocked,
            Known::KdfBelowFloor => L::FaultNextKdfBelowFloor,
            Known::MachineShape => L::FaultNextMachineShape,
            Known::InsufficientFunds => L::FaultNextInsufficientFunds,
            Known::NonceUsed => L::FaultNextNonceUsed,
            Known::AlreadyPending => L::FaultNextAlreadyPending,
            Known::Underpriced => L::FaultNextUnderpriced,
            Known::GasTooLow => L::FaultNextGasTooLow,
            Known::ContractRefused => L::FaultNextContractRefused,
            Known::RateLimited => L::FaultNextRateLimited,
            Known::MethodMissing => L::FaultNextMethodMissing,
            Known::NodeAuth => L::FaultNextNodeAuth,
            Known::WrongChain => L::FaultNextWrongChain,
            Known::NodeRefused => L::FaultNextNodeRefused,
            Known::NodeTls => L::FaultNextNodeTls,
            Known::NodeTimeout => L::FaultNextNodeTimeout,
            Known::AnswerTooLong => L::FaultNextAnswerTooLong,
            Known::AnswerNotJson => L::FaultNextAnswerNotJson,
            Known::SeatUnseated => L::FaultNextSeatUnseated,
            Known::SeatDomain => L::FaultNextSeatDomain,
            Known::BackupNotLanded => L::FaultNextBackupNotLanded,
            Known::RecoveryNoMatch => L::FaultNextRecoveryNoMatch,
            Known::PathRelative => L::FaultNextPathRelative,
            Known::NoEndpoint => L::FaultNextNoEndpoint,
            Known::Unreachable => L::FaultNextUnreachable,
            Known::Disagree => L::FaultNextDisagree,
            Known::ChainShape => L::FaultNextChainShape,
            Known::PenHeld => L::FaultNextPenHeld,
            Known::LedgerNotFetched => L::FaultNextLedgerNotFetched,
            Known::NewerElsewhere => L::FaultNextNewerElsewhere,
            Known::EntryRefused => L::FaultNextEntryRefused,
            Known::NoGenesis => L::FaultNextNoGenesis,
            Known::ForkedRoot => L::FaultNextForkedRoot,
            Known::AuditInput => L::FaultNextAuditInput,
            Known::NotAdoptable => L::FaultNextNotAdoptable,
            Known::AlreadyRooted => L::FaultNextAlreadyRooted,
            Known::RestorePartial => L::FaultNextRestorePartial,
            Known::CannotLay => L::FaultNextCannotLay,
            Known::NotAHome => L::FaultNextNotAHome,
            Known::NoHome => L::FaultNextNoHome,
            Known::KeyNotStored => L::FaultNextKeyNotStored,
            Known::GitShape => L::FaultNextGitShape,
            Known::NotARepo => L::FaultNextNotARepo,
            Known::RefMissing => L::FaultNextRefMissing,
            Known::ObjectMissing => L::FaultNextObjectMissing,
            Known::Broken => L::FaultNextBroken,
            Known::ContentShape => L::FaultNextContentShape,
            Known::QueueShape => L::FaultNextQueueShape,
            Known::QueueEmpty => L::FaultNextQueueEmpty,
            Known::NoRegistry => L::FaultNextNoRegistry,
            Known::NoChainId => L::FaultNextNoChainId,
            Known::GasRefused => L::FaultNextGasRefused,
            Known::GasNotShown => L::FaultNextGasNotShown,
            Known::SendFailed => L::FaultNextSendFailed,
            Known::NotAudited => L::FaultNextNotAudited,
            Known::SubjectMissing => L::FaultNextSubjectMissing,
            Known::DirEmpty => L::FaultNextDirEmpty,
            Known::ScanRefused => L::FaultNextScanRefused,
            Known::FieldMissing => L::FaultNextFieldMissing,
            Known::StepSkipped => L::FaultNextStepSkipped,
            Known::KitRefused => L::FaultNextKitRefused,
            Known::Conflict => L::FaultNextConflict,
            Known::CosignRefused => L::FaultNextCosignRefused,
            Known::HandedOver => L::FaultNextHandedOver,
            Known::NoBytes => L::FaultNextNoBytes,
            Known::WorkerPanicked => L::FaultNextWorkerPanicked,
            Known::OutcomeLost => L::FaultNextOutcomeLost,
            Known::NotHisBook => L::FaultNextNotHisBook,
            Known::NotThisLedger => L::FaultNextNotThisLedger,
            Known::NotAGrant => L::FaultNextNotAGrant,
            Known::AlreadyHeld => L::FaultNextAlreadyHeld,
            Known::GrantNotHeld => L::FaultNextGrantNotHeld,
            Known::GrantDirEmpty => L::FaultNextGrantDirEmpty,
            Known::AlreadyAnchored => L::FaultNextAlreadyAnchored,
            Known::PayloadRefused => L::FaultNextPayloadRefused,
            Known::ChainIncomplete => L::FaultNextChainIncomplete,
            Known::IdentityExists => L::FaultNextIdentityExists,
            Known::NoIdentity => L::FaultNextNoIdentity,
            Known::PhraseWords => L::FaultNextPhraseWords,
            Known::PhraseInvalid => L::FaultNextPhraseInvalid,
            Known::PhraseConfirm => L::FaultNextPhraseConfirm,
            Known::NoWords => L::FaultNextNoWords,
            Known::DeleteUnbacked => L::FaultNextDeleteUnbacked,
            Known::PasswordsDiffer => L::FaultNextPasswordsDiffer,
            Known::PasswordShort => L::FaultNextPasswordShort,
            Known::PasswordLong => L::FaultNextPasswordLong,
            Known::IdentitiesShape => L::FaultNextIdentitiesShape,
            Known::PublishNoKit => L::FaultNextPublishNoKit,
            Known::TermsMismatch => L::FaultNextTermsMismatch,
            Known::GrantFileKit => L::FaultNextGrantFileKit,
            Known::GrantFileBad => L::FaultNextGrantFileBad,
            Known::RemoteKit => L::FaultNextRemoteKit,
            Known::RemoteStatus => L::FaultNextRemoteStatus,
            Known::RemoteRedirect => L::FaultNextRemoteRedirect,
            Known::RemoteTooLarge => L::FaultNextRemoteTooLarge,
            Known::RemoteTimeout => L::FaultNextRemoteTimeout,
            Known::RemoteUnreachable => L::FaultNextRemoteUnreachable,
            Known::RemoteCert => L::FaultNextRemoteCert,
            Known::RemoteNotHttps => L::FaultNextRemoteNotHttps,
            Known::NotAnOriginal => L::FaultNextNotAnOriginal,
            Known::TraceFull => L::FaultNextTraceFull,
            Known::ClaimShape => L::FaultNextClaimShape,
            Known::LocalSeal => L::FaultNextLocalSeal,
            Known::MigrateMismatch => L::FaultNextMigrateMismatch,
            Known::RecoveryNotPrimary => L::FaultNextRecoveryNotPrimary,
            Known::PrimaryDelete => L::FaultNextPrimaryDelete,
            Known::PrimaryNoKeyFile => L::FaultNextPrimaryNoKeyFile,
            Known::PrimaryAlready => L::FaultNextPrimaryAlready,
            Known::BackupPassword => L::FaultNextBackupPassword,
            Known::BackupNotOurs => L::FaultNextBackupNotOurs,
            Known::BackupTooNew => L::FaultNextBackupTooNew,
            Known::BackupShape => L::FaultNextBackupShape,
            Known::Rekeying => L::FaultNextRekeying,
            Known::BackupNoIdentity => L::FaultNextBackupNoIdentity,
            Known::HomeUnreachable => L::FaultNextHomeUnreachable,
            Known::BusyForRekey => L::FaultNextBusyForRekey,
            Known::DialogUnavailable => L::FaultNextDialogUnavailable,
            Known::LocalSetAside => L::FaultNextLocalSetAside,
            Known::EntriesUnreadable => L::FaultNextEntriesUnreadable,
            Known::NetworkListed => L::FaultNextNetworkListed,
            Known::InsideHome => L::FaultNextInsideHome,
        }
    }
}

/// Plain words of known errors. The only source.
pub fn translate(k: Known) -> &'static str {
    match k {
        Known::FontMissing => "系统里找不到这一面字体,界面会改用别的面顶上",
        Known::FileMissing => "这一档不在盘上",
        Known::Denied => "没有权限读写这一处",
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
        Known::WrongChain => "节点的链号与所选网络不符",
        Known::NodeRefused => "节点拒了,原话在证据尾里",
        Known::NodeTls => "与节点的安全连接没建成(哪一层在证据尾里)",
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
        Known::PenHeld => "这本账还握着笔:恢复之后要等一次对锚对账报 COMPLETE",
        Known::LedgerNotFetched => "导入的身份,账本还没与链上核过尾:这一席的账本对过这把钥的锚、无缺才开写",
        Known::NewerElsewhere => "取回的账本对不上链上这一位的锚:别处有更新的条目",
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
        Known::Broken => "这本账断链了:全 app 转只读,先走恢复",
        Known::ContentShape => "内容哈希要三十二个字节",
        Known::QueueShape => "待锚队列那一份档的形不对",
        Known::QueueEmpty => "队列里没有挑中的条目",
        Known::NoRegistry => "还没有配喇叭合约的地址",
        Known::NoChainId => "还没有配这一处端点的链号",
        Known::GasRefused => "节点估不出这一笔的气:它会回滚",
        Known::GasNotShown => "这一批没按这个枚数估过 Gas 费",
        Known::SendFailed => "发出去了,而链上那一笔不是成功",
        Known::NotAudited => "还没有跑过一次对锚审计",
        Known::SubjectMissing => "注记指的那一条不在这本账里",
        Known::DirEmpty => "这一处目录里一个档也没有",
        Known::ScanRefused => "链上这一趟没扫全(A2 的拒因在证据尾里)",
        Known::FieldMissing => "这一格是必填的,还空着",
        Known::StepSkipped => "清单不许跳步:先做前一步",
        Known::KitRefused => "这一包出不去(V2 的拒因在证据尾里)",
        Known::Conflict => "同一枚记录上已经有一件带独占旗的授权,窗口还相交",
        Known::CosignRefused => "连署验签没过(K1 的 token 在证据尾里)",
        Known::HandedOver => "这本账已经承继给新钥了,这一台转只读",
        Known::NoBytes => "只见锚,未见字节",
        Known::WorkerPanicked => "后台那一趟自己塌了",
        Known::OutcomeLost => "后台那一趟做完了,结果没有回到壳",
        Known::NotHisBook => "这一叠字节不是所填那一位的账本",
        Known::NotThisLedger => "这里的账不是这一条授权签发者的账,不作料",
        Known::NotAGrant => "这一枚过了法,而它不是一枚授权",
        Known::AlreadyHeld => "这一枚授权已在库里",
        Known::GrantNotHeld => "这一份授权不在保管库里,备注无处可记",
        Known::GrantDirEmpty => "这一处授权文件夹里一份档也没有",
        Known::AlreadyAnchored => "这一条已经上链,不必再排进待上链",
        Known::PayloadRefused => "这段载荷 K2 不收(它的 token 在证据尾)",
        Known::ChainIncomplete => "上游链级不到根,缺的那一枚在证据尾",
        Known::IdentityExists => "这个身份已经在这台机器上了",
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
        Known::ClaimShape => "认领的字读不成",
        Known::LocalSeal => "本机数据里有一份档解不开",
        Known::MigrateMismatch => "旧版明文档转封后比对不一致,迁移已停",
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
    }
}

/// One error. Two branches, each with an evidence tail.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fault {
    class: Class,
    said: String,
    tail: String,
    /// For faults that carry a law token (`ENTRY_REFUSED`, `SCAN_REFUSED`), the plain words looked up by that
    /// token; when present, the screen's half sentence says this. Code, tail and evidence words are
    /// unaffected.
    say: Option<crate::lang::Key>,
    /// Likewise, the next-step sentence (`None` uses the closed table's).
    then: Option<crate::lang::Key>,
    /// For a landing that failed, which of the landing troubles it was, as the glue handed it (the code and
    /// subject in the tail are made from the same value).
    landing: Option<zikaron_glue::pack::Trouble>,
    /// For a place a person named that could not be read, that place as given (the tail says it in words;
    /// this is the same place as a value).
    place: Option<String>,
    /// For a remote answer refused by its status, that status number (the tail says it in words).
    status: Option<u16>,
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
    /// Known: plain words from the closed table, with the evidence tail.
    pub fn known(k: Known, tail: impl Into<String>) -> Fault {
        Fault {
            class: Class::Known,
            said: format!("{}:{}", k.as_str(), translate(k)),
            tail: tail.into(),
            say: None,
            then: None,
            landing: None,
            place: None,
            status: None,
        }
    }

    /// Landing failures in five forms, each said separately: read/write error, place taken, kit path not
    /// allowed, duplicate kit path, kit verification failed. The code stays `LANDING`; the tail is only the
    /// refusal code and its subject (the offending name or the target path). Unrecognized codes use the
    /// closed table's two sentences.
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

    /// A landing that failed, from the glue's own trouble: the same code, tail and sentences as
    /// [`Fault::landing`], with the trouble itself kept beside them ([`Fault::landing_trouble`]).
    pub fn of_landing(t: impl Into<zikaron_glue::pack::Trouble>) -> Fault {
        let t = t.into();
        Fault { landing: Some(t.clone()), ..Fault::landing(t.code(), t.subject()) }
    }

    /// The same fault, saying which place a person named could not be read (kept when one is already said:
    /// the innermost place wins).
    pub fn at_place(mut self, place: impl Into<String>) -> Fault {
        if self.place.is_none() {
            self.place = Some(place.into());
        }
        self
    }

    /// The same fault, carrying the remote status number it was refused for.
    pub fn with_status(mut self, status: u16) -> Fault {
        self.status = Some(status);
        self
    }

    /// The remote status number this fault was refused for (`None` for every other fault).
    pub fn status(&self) -> Option<u16> {
        self.status
    }

    /// The place a person named that could not be read (`None` for every other fault).
    pub fn place(&self) -> Option<&str> {
        self.place.as_deref()
    }

    /// Which landing trouble this fault was made from (`None` for every other fault).
    pub fn landing_trouble(&self) -> Option<&zikaron_glue::pack::Trouble> {
        self.landing.as_ref()
    }

    /// The law refused an entry: the tail is the token's name, the screen's sentence comes from
    /// [`entry_token_say`].
    pub fn entry_refused(t: zikaron::tokens::Token) -> Fault {
        Fault { say: Some(entry_token_say(t)), ..Fault::known(Known::EntryRefused, format!("{t:?}")) }
    }

    /// An entry in a mirror bundle was refused by the law: the code stays `MIRROR_ENTRY` (the whole bundle is
    /// refused), the tail carries the file name and token; the screen's sentence comes from
    /// [`entry_token_say`].
    pub fn mirror_entry(name: &str, t: zikaron::tokens::Token) -> Fault {
        Fault { say: Some(entry_token_say(t)), ..Fault::known(Known::MirrorEntry, crate::lang::filln(crate::lang::Key::Tail193, &[name, &format!("{t:?}")])) }
    }

    /// A chain scan did not read everything: the tail is as the caller gave it; the screen's sentence comes
    /// from [`scan_refusal_say`] by the first refusal (with none, for example a malformed node address, the
    /// closed table's sentence).
    pub fn scan_refused(tail: impl Into<String>, rs: &[zikaron_anchor::scan::Refusal]) -> Fault {
        Fault { say: rs.first().map(scan_refusal_say), ..Fault::known(Known::ScanRefused, tail) }
    }

    /// How a scan refusal is written in the evidence tail (`code:detail`). One answer.
    pub fn scan_tail(r: &zikaron_anchor::scan::Refusal) -> String {
        format!("{}:{}", r.code(), r.detail())
    }

    /// Unknown: passed through unchanged. The sentence on screen is the one from below, nothing added or
    /// changed.
    pub fn unknown(tail: impl Into<String>) -> Fault {
        let tail = tail.into();
        Fault { class: Class::Unknown, said: tail.clone(), tail, say: None, then: None, landing: None, place: None, status: None }
    }

    pub fn class(&self) -> Class {
        self.class
    }

    /// The sentence said on screen.
    pub fn said(&self) -> &str {
        &self.said
    }

    /// The half sentence of plain words shown at the first level: the known branch without the table code
    /// (code and tail fold into the raw error); the unknown branch has no plain words and stays as is.
    /// The same fault said in other words on screen (the first sentence, the next-step sentence); code,
    /// tail and evidence unchanged. `None` keeps the table's.
    pub fn worded(self, say: Option<crate::lang::Key>, then: Option<crate::lang::Key>) -> Fault {
        Fault { say: say.or(self.say), then: then.or(self.then), ..self }
    }

    /// The next-step sentence this fault was given (`worded`), if any.
    pub fn then_key(&self) -> Option<crate::lang::Key> {
        self.then
    }

    pub fn human(&self) -> &str {
        match (self.which(), self.say) {
            (Some(_), Some(said)) => crate::lang::t(said),
            (Some(k), None) => crate::lang::t(k.what()),
            (None, _) => &self.said,
        }
    }

    /// What to do next (the second sentence). The unknown branch has no table sentence and points to the raw
    /// error.
    pub fn next(&self) -> &'static str {
        match self.which() {
            Some(_) if self.then.is_some() => crate::lang::t(self.then.unwrap_or(crate::lang::Key::FaultNextUnknown)),
            Some(k) => crate::lang::t(k.next()),
            None => crate::lang::t(crate::lang::Key::FaultNextUnknown),
        }
    }

    /// The raw error field: table code plus evidence tail (known); the unknown branch is the sentence from
    /// below.
    pub fn raw(&self) -> String {
        match self.which() {
            Some(k) if self.tail.is_empty() => k.as_str().to_string(),
            Some(k) => format!("{} · {}", k.as_str(), self.tail),
            None => self.tail.clone(),
        }
    }

    /// Which member of the closed table a known fault is (recognized from the code; codes come from
    /// `Known::as_str`).
    pub fn which(&self) -> Option<Known> {
        if self.class != Class::Known {
            return None;
        }
        let code = self.said.split_once(':').map(|(c, _)| c).unwrap_or(&self.said);
        Known::ALL.iter().copied().find(|k| k.as_str() == code)
    }

    /// Evidence words: `said · tail`. Tests read this form (the first word up to `:` is the code), and so do
    /// readings built from faults (status line, check input source, mirror and vault refusals); the format is
    /// stable. The two screen sentences and the raw error use `human`,
    /// `next` and `raw`.
    pub fn evidence(&self) -> String {
        format!("{} · {}", self.said, self.tail)
    }

    /// Evidence tail: the original return from below, kept in both branches.
    pub fn tail(&self) -> &str {
        &self.tail
    }

    /// The unknown branch passes through byte for byte, computed.
    pub fn passthrough(&self) -> bool {
        self.class == Class::Unknown && self.said == self.tail
    }
}

/// Law entry refusal tokens to plain words (the 26 members of law §10). Exhaustive over the closed type: a
/// new token does not compile until added here.
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

/// Scan refusals to plain words (five members). Exhaustive, as above.
pub fn scan_refusal_say(r: &zikaron_anchor::scan::Refusal) -> crate::lang::Key {
    use crate::lang::Key as L;
    use zikaron_anchor::scan::Refusal as R;
    match r {
        R::ChainIdMismatch { .. } => L::TokScanChainId,
        R::NoEndpoint(_) => L::TokScanNoEndpoint,
        R::Unanswered { .. } => L::TokScanUnanswered,
        R::TxNotItself(_) => L::TokScanTxNotItself,
        R::Malformed(_) => L::TokScanMalformed,
    }
}

/// Classify. Only the table's members are recognized; everything else goes to the unknown branch, never
/// guessed.
pub fn classify(e: &std::io::Error, subject: &str) -> Fault {
    let tail = format!("{subject}: {e}");
    match e.kind() {
        ErrorKind::NotFound => Fault::known(Known::FileMissing, tail),
        ErrorKind::PermissionDenied => Fault::known(Known::Denied, tail),
        _ => Fault::unknown(tail),
    }
}
