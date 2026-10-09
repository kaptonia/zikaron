//! Closed tables of the law: refusal tokens (§10), finding names (§8.3), labels and item names (§8.7),
//! verdicts (§9.3), domains (§5.6), entry types (§6).
//!
//! Each table is a closed type with one byte outlet (`as_str`). A new member means changing the enum and
//! `as_str`, and the compiler holds every match to it; a list of `pub const` strings would let a new literal
//! slip onto the wire.
//!
//! [`EntryType`] is open by law (§6.9: the type table may grow), so it carries `Other`.

/// The 26 refusal tokens of law §10. Decision order follows §3.5 and §4.3; each token belongs to one decision
/// point.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Token {
    Utf8,
    Json,
    Number,
    Depth,
    DupKey,
    KeyCharset,
    ValueCharset,
    NotCanonical,
    Envelope,
    EnvelopeMissing,
    EnvelopeClosed,
    Spec,
    EntryType,
    Author,
    Seq,
    Prev,
    PrevSeq,
    Body,
    SigForm,
    GenesisPlace,
    BodyField,
    SigV,
    SigRange,
    SigHighS,
    SigRecover,
    SigSigner,
}

impl Token {
    pub fn as_str(self) -> &'static str {
        match self {
            Token::Utf8 => "E_UTF8",
            Token::Json => "E_JSON",
            Token::Number => "E_NUMBER",
            Token::Depth => "E_DEPTH",
            Token::DupKey => "E_DUP_KEY",
            Token::KeyCharset => "E_KEY_CHARSET",
            Token::ValueCharset => "E_VALUE_CHARSET",
            Token::NotCanonical => "E_NOT_CANONICAL",
            Token::Envelope => "E_ENVELOPE",
            Token::EnvelopeMissing => "E_ENVELOPE_MISSING",
            Token::EnvelopeClosed => "E_ENVELOPE_CLOSED",
            Token::Spec => "E_SPEC",
            Token::EntryType => "E_ENTRYTYPE",
            Token::Author => "E_AUTHOR",
            Token::Seq => "E_SEQ",
            Token::Prev => "E_PREV",
            Token::PrevSeq => "E_PREV_SEQ",
            Token::Body => "E_BODY",
            Token::SigForm => "E_SIG_FORM",
            Token::GenesisPlace => "E_GENESIS_PLACE",
            Token::BodyField => "E_BODY_FIELD",
            Token::SigV => "E_SIG_V",
            Token::SigRange => "E_SIG_RANGE",
            Token::SigHighS => "E_SIG_HIGH_S",
            Token::SigRecover => "E_SIG_RECOVER",
            Token::SigSigner => "E_SIG_SIGNER",
        }
    }

    /// The whole table.
    pub const ALL: [Token; 26] = [
        Token::Utf8,
        Token::Json,
        Token::Number,
        Token::Depth,
        Token::DupKey,
        Token::KeyCharset,
        Token::ValueCharset,
        Token::NotCanonical,
        Token::Envelope,
        Token::EnvelopeMissing,
        Token::EnvelopeClosed,
        Token::Spec,
        Token::EntryType,
        Token::Author,
        Token::Seq,
        Token::Prev,
        Token::PrevSeq,
        Token::Body,
        Token::SigForm,
        Token::GenesisPlace,
        Token::BodyField,
        Token::SigV,
        Token::SigRange,
        Token::SigHighS,
        Token::SigRecover,
        Token::SigSigner,
    ];
}

/// The eight tokens of law §3.5 (the faults of tests 1 to 6); the only ones the `json` reader returns. A
/// subtable of [`Token`]: `From` lifts each to the §10 token of the same bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CanonToken {
    Utf8,
    Json,
    Number,
    Depth,
    DupKey,
    KeyCharset,
    ValueCharset,
    NotCanonical,
}

impl CanonToken {
    pub fn as_str(self) -> &'static str {
        Token::from(self).as_str()
    }

    pub const ALL: [CanonToken; 8] = [
        CanonToken::Utf8,
        CanonToken::Json,
        CanonToken::Number,
        CanonToken::Depth,
        CanonToken::DupKey,
        CanonToken::KeyCharset,
        CanonToken::ValueCharset,
        CanonToken::NotCanonical,
    ];
}

impl From<CanonToken> for Token {
    fn from(c: CanonToken) -> Token {
        match c {
            CanonToken::Utf8 => Token::Utf8,
            CanonToken::Json => Token::Json,
            CanonToken::Number => Token::Number,
            CanonToken::Depth => Token::Depth,
            CanonToken::DupKey => Token::DupKey,
            CanonToken::KeyCharset => Token::KeyCharset,
            CanonToken::ValueCharset => Token::ValueCharset,
            CanonToken::NotCanonical => Token::NotCanonical,
        }
    }
}

/// The six signature tokens of laws §5.4 and §5.5 (shape, v, range, low s, recovery, signer); the only ones
/// `verify_signature` returns, borrowed word for word by kit law §3.1. `From` lifts them to §10 tokens.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SigToken {
    SigForm,
    SigV,
    SigRange,
    SigHighS,
    SigRecover,
    SigSigner,
}

impl SigToken {
    pub fn as_str(self) -> &'static str {
        Token::from(self).as_str()
    }

    pub const ALL: [SigToken; 6] = [
        SigToken::SigForm,
        SigToken::SigV,
        SigToken::SigRange,
        SigToken::SigHighS,
        SigToken::SigRecover,
        SigToken::SigSigner,
    ];
}

impl From<SigToken> for Token {
    fn from(t: SigToken) -> Token {
        match t {
            SigToken::SigForm => Token::SigForm,
            SigToken::SigV => Token::SigV,
            SigToken::SigRange => Token::SigRange,
            SigToken::SigHighS => Token::SigHighS,
            SigToken::SigRecover => Token::SigRecover,
            SigToken::SigSigner => Token::SigSigner,
        }
    }
}

/// The five finding names of law §8.3.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FindingName {
    SeqGap,
    PrevMismatch,
    AuthorityMismatch,
    RootMismatch,
    Equivocation,
}

impl FindingName {
    pub fn as_str(self) -> &'static str {
        match self {
            FindingName::SeqGap => "SEQ_GAP",
            FindingName::PrevMismatch => "PREV_MISMATCH",
            FindingName::AuthorityMismatch => "AUTHORITY_MISMATCH",
            FindingName::RootMismatch => "ROOT_MISMATCH",
            FindingName::Equivocation => "EQUIVOCATION",
        }
    }

    pub const ALL: [FindingName; 5] = [
        FindingName::SeqGap,
        FindingName::PrevMismatch,
        FindingName::AuthorityMismatch,
        FindingName::RootMismatch,
        FindingName::Equivocation,
    ];
}

/// The four labels of law §8.7 item 15, plus `NoLabel`: the outcome when the input is not a §9.4 audit input.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Label {
    BrokenChain,
    Unavailable,
    Gaps,
    Complete,
    NoLabel,
}

impl Label {
    pub fn as_str(self) -> &'static str {
        match self {
            Label::BrokenChain => "BROKEN_CHAIN",
            Label::Unavailable => "UNAVAILABLE",
            Label::Gaps => "GAPS",
            Label::Complete => "COMPLETE",
            Label::NoLabel => "NO_LABEL",
        }
    }

    pub const ALL: [Label; 5] = [
        Label::BrokenChain,
        Label::Unavailable,
        Label::Gaps,
        Label::Complete,
        Label::NoLabel,
    ];
}

/// The three anchor verdicts of law §9.3. [`Verdict::parse`] reads input bytes; other bytes are no verdict.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Verdict {
    Counted,
    Unproven,
    Void,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Counted => "counted",
            Verdict::Unproven => "UNPROVEN",
            Verdict::Void => "VOID",
        }
    }

    pub fn parse(x: &str) -> Option<Verdict> {
        Verdict::ALL.into_iter().find(|v| v.as_str() == x)
    }

    pub const ALL: [Verdict; 3] = [Verdict::Counted, Verdict::Unproven, Verdict::Void];
}

/// Keys of the fifteen-item report of law §8.7, with the finding-row and item-row keys. The report's byte
/// shape grows only from here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    // The sixteen members (law §8.7 items 1 to 15; item 1 takes two).
    Root,
    Basis,
    Entries,
    Findings,
    Missing,
    Anchored,
    Unanchored,
    Excluded,
    AdoptionUnproven,
    UnknownType,
    Malformed,
    Unavailable,
    Unproven,
    Void,
    Discarded,
    LabelKey,
    // The two no-label members (§9.4).
    Ok,
    Reason,
    // Finding rows (§8.3) and item-row keys.
    Name,
    Position,
    EntryId,
    Hard,
    Expected,
    Actual,
    SeqKey,
    Certain,
    A,
    B,
    Hash,
    Anchors,
    ChainId,
    Tx,
    BlockNumber,
    Author,
    Index,
    Sender,
    BlockTimestamp,
    EntryTypeKey,
    TokenKey,
    VerdictKey,
}

impl Key {
    pub fn as_str(self) -> &'static str {
        match self {
            Key::Root => "root",
            Key::Basis => "basis",
            Key::Entries => "entries",
            Key::Findings => "findings",
            Key::Missing => "missing",
            Key::Anchored => "anchored",
            Key::Unanchored => "unanchored",
            Key::Excluded => "excluded",
            Key::AdoptionUnproven => "adoption_unproven",
            Key::UnknownType => "unknown_type",
            Key::Malformed => "malformed",
            Key::Unavailable => "unavailable",
            Key::Unproven => "unproven",
            Key::Void => "void",
            Key::Discarded => "discarded",
            Key::LabelKey => "label",
            Key::Ok => "ok",
            Key::Reason => "reason",
            Key::Name => "name",
            Key::Position => "position",
            Key::EntryId => "entry_id",
            Key::Hard => "hard",
            Key::Expected => "expected",
            Key::Actual => "actual",
            Key::SeqKey => "seq",
            Key::Certain => "certain",
            Key::A => "a",
            Key::B => "b",
            Key::Hash => "hash",
            Key::Anchors => "anchors",
            Key::ChainId => "chainId",
            Key::Tx => "tx",
            Key::BlockNumber => "blockNumber",
            Key::Author => "author",
            Key::Index => "index",
            Key::Sender => "sender",
            Key::BlockTimestamp => "blockTimestamp",
            Key::EntryTypeKey => "entryType",
            Key::TokenKey => "token",
            Key::VerdictKey => "verdict",
        }
    }
}

/// The two signing domains of law §5.6.
///
/// On the harness surface a domain is a free string from the command line (HARNESS `sign`), so the signing
/// API in [`crate::entry`] still takes `&str`; the law's own two call sites take their bytes from here.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Domain {
    Entry,
    Adoption,
}

impl Domain {
    pub fn as_str(self) -> &'static str {
        match self {
            Domain::Entry => "zikaron/1",
            Domain::Adoption => "zikaron/1-adoption",
        }
    }
}

/// The spec literal of law §4.1.
pub const SPEC: &str = "zikaron/1";

/// The seven types of law §6.1 to §6.8, plus the unlisted member of §6.9. Open by law: `Other` is the rule
/// that an unlisted type only needs an object body.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryType {
    Genesis,
    History,
    Grant,
    Revocation,
    Adoption,
    Succession,
    Annotation,
    Other,
}

impl EntryType {
    pub fn as_str(self) -> &'static str {
        match self {
            EntryType::Genesis => "genesis",
            EntryType::History => "history",
            EntryType::Grant => "grant",
            EntryType::Revocation => "revocation",
            EntryType::Adoption => "adoption",
            EntryType::Succession => "succession",
            EntryType::Annotation => "annotation",
            EntryType::Other => "",
        }
    }

    /// Known types map to themselves, anything else to `Other` (law §6.9).
    pub fn of(x: &str) -> EntryType {
        match x {
            "genesis" => EntryType::Genesis,
            "history" => EntryType::History,
            "grant" => EntryType::Grant,
            "revocation" => EntryType::Revocation,
            "adoption" => EntryType::Adoption,
            "succession" => EntryType::Succession,
            "annotation" => EntryType::Annotation,
            _ => EntryType::Other,
        }
    }
}
