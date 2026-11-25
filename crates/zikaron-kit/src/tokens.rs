pub use zikaron::tokens::{CanonToken, SigToken, Token};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DocToken {
    Canon(CanonToken),
    Doc,
    DocMissing,
    DocClosed,
    Spec,
    FpmAuthor,
    FpmWork,
    FpmGrant,
    FpmRows,
    FpmRow,
    FpmDupRecipient,
    FpmDupVariant,
    FpmRowOrder,
    FpmNote,
    AckRecipient,
    AckFpm,
    AckVariant,
    AckNote,
    Sig(SigToken),
}

impl DocToken {
    pub fn as_str(self) -> &'static str {
        match self {
            DocToken::Canon(t) => t.as_str(),
            DocToken::Doc => "E_DOC",
            DocToken::DocMissing => "E_DOC_MISSING",
            DocToken::DocClosed => "E_DOC_CLOSED",
            DocToken::Spec => "E_SPEC",
            DocToken::FpmAuthor => "E_FPM_AUTHOR",
            DocToken::FpmWork => "E_FPM_WORK",
            DocToken::FpmGrant => "E_FPM_GRANT",
            DocToken::FpmRows => "E_FPM_ROWS",
            DocToken::FpmRow => "E_FPM_ROW",
            DocToken::FpmDupRecipient => "E_FPM_DUP_RECIPIENT",
            DocToken::FpmDupVariant => "E_FPM_DUP_VARIANT",
            DocToken::FpmRowOrder => "E_FPM_ROW_ORDER",
            DocToken::FpmNote => "E_FPM_NOTE",
            DocToken::AckRecipient => "E_ACK_RECIPIENT",
            DocToken::AckFpm => "E_ACK_FPM",
            DocToken::AckVariant => "E_ACK_VARIANT",
            DocToken::AckNote => "E_ACK_NOTE",
            DocToken::Sig(t) => t.as_str(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BadgeEncodeToken {
    Entry,
    Type,
    Cap,
}

impl BadgeEncodeToken {
    pub fn as_str(self) -> &'static str {
        match self {
            BadgeEncodeToken::Entry => "E_BADGE_ENTRY",
            BadgeEncodeToken::Type => "E_BADGE_TYPE",
            BadgeEncodeToken::Cap => "E_BADGE_CAP",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BadgeDecodeToken {
    Prefix,
    Cap,
    B64,
    Entry,
    Type,
    Incomplete,
    Link,
}

impl BadgeDecodeToken {
    pub const ALL: [BadgeDecodeToken; 7] = [
        BadgeDecodeToken::Prefix,
        BadgeDecodeToken::Cap,
        BadgeDecodeToken::B64,
        BadgeDecodeToken::Entry,
        BadgeDecodeToken::Type,
        BadgeDecodeToken::Incomplete,
        BadgeDecodeToken::Link,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            BadgeDecodeToken::Prefix => "E_BADGE_PREFIX",
            BadgeDecodeToken::Cap => "E_BADGE_CAP",
            BadgeDecodeToken::B64 => "E_BADGE_B64",
            BadgeDecodeToken::Entry => "E_BADGE_ENTRY",
            BadgeDecodeToken::Type => "E_BADGE_TYPE",
            BadgeDecodeToken::Incomplete => "E_BADGE_INCOMPLETE",
            BadgeDecodeToken::Link => "E_BADGE_LINK",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KitFailToken {
    Unreadable,
    ManifestAbsent,
    Manifest,
    EntryBytes,
    File,
    ProofBytes,
    Extra,
}

impl KitFailToken {
    pub fn as_str(self) -> &'static str {
        match self {
            KitFailToken::Unreadable => "E_KIT_UNREADABLE",
            KitFailToken::ManifestAbsent => "E_KIT_MANIFEST_ABSENT",
            KitFailToken::Manifest => "E_KIT_MANIFEST",
            KitFailToken::EntryBytes => "E_KIT_ENTRY_BYTES",
            KitFailToken::File => "E_KIT_FILE",
            KitFailToken::ProofBytes => "E_KIT_PROOF_BYTES",
            KitFailToken::Extra => "E_KIT_EXTRA",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PairVerdict {
    Paired,
    FpmInvalid,
    AckInvalid,
    AckFpmMismatch,
    AckNoRow,
    AckVariantMismatch,
}

impl PairVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            PairVerdict::Paired => "PAIRED",
            PairVerdict::FpmInvalid => "FPM_INVALID",
            PairVerdict::AckInvalid => "ACK_INVALID",
            PairVerdict::AckFpmMismatch => "ACK_FPM_MISMATCH",
            PairVerdict::AckNoRow => "ACK_NO_ROW",
            PairVerdict::AckVariantMismatch => "ACK_VARIANT_MISMATCH",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Attribution {
    Attributed,
    NotAttributed,
}

impl Attribution {
    pub fn as_str(self) -> &'static str {
        match self {
            Attribution::Attributed => "ATTRIBUTED",
            Attribution::NotAttributed => "NOT_ATTRIBUTED",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum State {
    Pass,
    Fail,
    Unknown,
}

impl State {
    pub fn as_str(self) -> &'static str {
        match self {
            State::Pass => "PASS",
            State::Fail => "FAIL",
            State::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Check {
    BadSig,
    BrokenLedger,
    NotInLedger,
    Unanchored,
    Expired,
    Revoked,
}

impl Check {
    pub fn as_str(self) -> &'static str {
        match self {
            Check::BadSig => "BAD_SIG",
            Check::BrokenLedger => "BROKEN_LEDGER",
            Check::NotInLedger => "NOT_IN_LEDGER",
            Check::Unanchored => "UNANCHORED",
            Check::Expired => "EXPIRED",
            Check::Revoked => "REVOKED",
        }
    }

    pub const ALL: [Check; 6] = [
        Check::BadSig,
        Check::BrokenLedger,
        Check::NotInLedger,
        Check::Unanchored,
        Check::Expired,
        Check::Revoked,
    ];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reason {
    NotAGrant,
    Parent(Token),
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::NotAGrant => "NOT_A_GRANT",
            Reason::Parent(t) => t.as_str(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CheckVerdict {
    Green,
    Partial,
    Fail,
}

impl CheckVerdict {
    pub fn as_str(self) -> &'static str {
        match self {
            CheckVerdict::Green => "GREEN",
            CheckVerdict::Partial => "PARTIAL",
            CheckVerdict::Fail => "FAIL",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChainToken {
    Incomplete,
    Link,
    Empty,
}

impl ChainToken {
    pub fn as_str(self) -> &'static str {
        match self {
            ChainToken::Incomplete => "CHAIN_INCOMPLETE",
            ChainToken::Link => "CHAIN_LINK",
            ChainToken::Empty => "CHAIN_EMPTY",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FailKind {
    Empty,
    Incomplete,
    Link,
    Hop,
}

impl FailKind {
    pub fn as_str(self) -> &'static str {
        match self {
            FailKind::Empty => "empty",
            FailKind::Incomplete => "incomplete",
            FailKind::Link => "link",
            FailKind::Hop => "hop",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rule {
    Canonical,
    Members,
    Spec,
    Root,
    Entries,
    Files,
    Contents,
    Proofs,
    NoteMd,
}

impl Rule {
    pub fn as_str(self) -> &'static str {
        match self {
            Rule::Canonical => "canonical",
            Rule::Members => "members",
            Rule::Spec => "spec",
            Rule::Root => "root",
            Rule::Entries => "entries",
            Rule::Files => "files",
            Rule::Contents => "contents",
            Rule::Proofs => "proofs",
            Rule::NoteMd => "note_md",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Ok,
    Token,
    Index,
    Inner,
    Verdict,
    DocId,
    Recipient,
    Variant,
    Attributed,
    Payload,
    Grants,
    Counts,
    Entries,
    Files,
    Proofs,
    InvalidEntries,
    EntryId,
    KitId,
    Subject,
    Valid,
    Label,
    Found,
    Earliest,
    Deepest,
    Continuity,
    Anchored,
    Span,
    Basis,
    Checks,
    N,
    State,
    Reason,
    Failed,
    Hops,
    Links,
    Failing,
    Kind,
    Digest,
    Presig,
    Sig,
    Signer,
}

impl Key {
    pub fn as_str(self) -> &'static str {
        match self {
            Key::Ok => "ok",
            Key::Token => "token",
            Key::Index => "index",
            Key::Inner => "inner",
            Key::Verdict => "verdict",
            Key::DocId => "doc_id",
            Key::Recipient => "recipient",
            Key::Variant => "variant",
            Key::Attributed => "attributed",
            Key::Payload => "payload",
            Key::Grants => "grants",
            Key::Counts => "counts",
            Key::Entries => "entries",
            Key::Files => "files",
            Key::Proofs => "proofs",
            Key::InvalidEntries => "invalid_entries",
            Key::EntryId => "entry_id",
            Key::KitId => "kit_id",
            Key::Subject => "subject",
            Key::Valid => "valid",
            Key::Label => "label",
            Key::Found => "found",
            Key::Earliest => "earliest",
            Key::Deepest => "deepest",
            Key::Continuity => "continuity",
            Key::Anchored => "anchored",
            Key::Span => "span",
            Key::Basis => "basis",
            Key::Checks => "checks",
            Key::N => "n",
            Key::State => "state",
            Key::Reason => "reason",
            Key::Failed => "failed",
            Key::Hops => "hops",
            Key::Links => "links",
            Key::Failing => "failing",
            Key::Kind => "kind",
            Key::Digest => "digest",
            Key::Presig => "presig",
            Key::Sig => "sig",
            Key::Signer => "signer",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Domain {
    Fpm,
    Ack,
}

impl Domain {
    pub fn as_str(self) -> &'static str {
        match self {
            Domain::Fpm => "zikaron.fpm/1",
            Domain::Ack => "zikaron.ack/1",
        }
    }

    pub fn parse(x: &str) -> Option<Domain> {
        [Domain::Fpm, Domain::Ack].into_iter().find(|d| d.as_str() == x)
    }
}

pub const SPEC_FPM: &str = "zikaron.fpm/1";
pub const SPEC_ACK: &str = "zikaron.ack/1";
pub const SPEC_KIT: &str = "zikaron.kit/1";

pub const BADGE_OK: &str = "BADGE_OK";
pub const KIT_OK: &str = "KIT_OK";

pub const BADGE_PREFIX: &str = "zikaron-grant:";
pub const BADGE_CAP: usize = 2953;

pub const K2: &str = "K2";
