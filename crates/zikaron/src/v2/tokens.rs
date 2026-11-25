pub const K3: &str = "K3";

pub const SPEC: &str = "zikaron/2";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Domain {
    Entry,
}

impl Domain {
    pub fn as_str(self) -> &'static str {
        match self {
            Domain::Entry => "zikaron/2",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntryType {
    Genesis,
    History,
    Grant,
    Revocation,
    Succession,
    Migration,
    Annotation,
    Retraction,
    Other,
}

impl EntryType {
    pub fn of(x: &str) -> EntryType {
        match x {
            "genesis" => EntryType::Genesis,
            "history" => EntryType::History,
            "grant" => EntryType::Grant,
            "revocation" => EntryType::Revocation,
            "succession" => EntryType::Succession,
            "migration" => EntryType::Migration,
            "annotation" => EntryType::Annotation,
            "retraction" => EntryType::Retraction,
            _ => EntryType::Other,
        }
    }
}

pub const CLOSING_KIND: &str = "migration";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Label {
    BrokenChain,
    Unproven,
    Gaps,
    Complete,
}

impl Label {
    pub fn as_str(self) -> &'static str {
        match self {
            Label::BrokenChain => "BROKEN_CHAIN",
            Label::Unproven => "UNPROVEN",
            Label::Gaps => "GAPS",
            Label::Complete => "COMPLETE",
        }
    }

    pub fn parse(x: &str) -> Option<Label> {
        Label::ALL.into_iter().find(|l| l.as_str() == x)
    }

    pub fn of_v1(x: &str) -> Option<Label> {
        if x == crate::tokens::Label::Unavailable.as_str() {
            return Some(Label::Unproven);
        }
        Label::parse(x)
    }

    pub const ALL: [Label; 4] = [Label::BrokenChain, Label::Unproven, Label::Gaps, Label::Complete];
}

pub const NO_LABEL: &str = "NO_LABEL";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Root,
    Basis,
    Entries,
    Findings,
    Missing,
    Anchored,
    Unanchored,
    Excluded,
    Retired,
    UnknownType,
    Malformed,
    Unproven,
    Void,
    Discarded,
    Continues,
    LabelKey,
    Ok,
    Reason,
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
    BlockTimestamp,
    Author,
    Sender,
    EntryTypeKey,
    TokenKey,
    VerdictKey,
    Cut,
    Report,
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
            Key::Retired => "retired",
            Key::UnknownType => "unknown_type",
            Key::Malformed => "malformed",
            Key::Unproven => "unproven",
            Key::Void => "void",
            Key::Discarded => "discarded",
            Key::Continues => "continues",
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
            Key::BlockTimestamp => "blockTimestamp",
            Key::Author => "author",
            Key::Sender => "sender",
            Key::EntryTypeKey => "entryType",
            Key::TokenKey => "token",
            Key::VerdictKey => "verdict",
            Key::Cut => "cut",
            Key::Report => "report",
        }
    }
}
