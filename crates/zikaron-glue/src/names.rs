pub const ENTRIES_DIR: &str = "entries";
pub const FILES_DIR: &str = "files";
pub const PROOFS_DIR: &str = "proofs";
pub const MANIFEST: &str = "manifest.json";
pub const ENTRY_SUFFIX: &str = ".zk1";
pub const HANDOFF_NOTE: &str = "zikaron-handoff.txt";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    Spec,
    Root,
    Entries,
    Files,
    Contents,
    Proofs,
    NoteMd,
    AnchoredOn,
    Anchors,
    Path,
    Sha256,
    Size,
    Content,
    Tx,
    NameMd,
    ChainId,
    Registry,
    FromBlock,
    EntryId,
    Work,
    Grant,
}

impl Field {
    pub fn as_str(self) -> &'static str {
        match self {
            Field::Spec => "spec",
            Field::Root => "root",
            Field::Entries => "entries",
            Field::Files => "files",
            Field::Contents => "contents",
            Field::Proofs => "proofs",
            Field::NoteMd => "note_md",
            Field::AnchoredOn => "anchoredOn",
            Field::Anchors => "anchors",
            Field::NameMd => "name_md",
            Field::ChainId => "chainId",
            Field::Registry => "registry",
            Field::FromBlock => "fromBlock",
            Field::EntryId => "entry_id",
            Field::Path => "path",
            Field::Sha256 => "sha256",
            Field::Size => "size",
            Field::Content => "content",
            Field::Tx => "tx",
            Field::Work => "work",
            Field::Grant => "grant",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    Verify,
}

impl Slot {
    pub fn path(self) -> &'static str {
        match self {
            Slot::Verify => "verify.md",
        }
    }

    pub const ALL: [Slot; 1] = [Slot::Verify];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Ok,
    Reason,
    Detail,
    State,
    Path,
    Entries,
    Files,
    Proofs,
    KitId,
    Dropped,
}

impl Key {
    pub fn as_str(self) -> &'static str {
        match self {
            Key::Ok => "ok",
            Key::Reason => "reason",
            Key::Detail => "detail",
            Key::State => "state",
            Key::Path => "path",
            Key::Entries => "entries",
            Key::Files => "files",
            Key::Proofs => "proofs",
            Key::KitId => "kitId",
            Key::Dropped => "dropped",
        }
    }
}
