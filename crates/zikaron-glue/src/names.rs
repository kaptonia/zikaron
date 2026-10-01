//! Every free name this crate spells (kit directories and file names, manifest members, entry body members,
//! answer keys) is written once, here.
//!
//! The law's own bytes are not copied: `zikaron.kit/1` comes from [`zikaron_kit::tokens::SPEC_KIT`], `KIT_OK`
//! and the three verdicts from the kit core's constants, entry types from [`zikaron::tokens::EntryType`].

/// The three places in a kit and the manifest file name (kit law §7.1 to §7.4).
pub const ENTRIES_DIR: &str = "entries";
pub const FILES_DIR: &str = "files";
pub const PROOFS_DIR: &str = "proofs";
pub const MANIFEST: &str = "manifest.json";
/// Suffix of entry files in a kit (`entries/<64 hex>.zk1`, kit law §7.4).
pub const ENTRY_SUFFIX: &str = ".zk1";

/// Member names of the manifest (kit law §7.3) and of entry bodies (law §6). Closed set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    // The seven manifest members.
    Spec,
    Root,
    Entries,
    Files,
    Contents,
    Proofs,
    NoteMd,
    // Manifest rows.
    Path,
    Sha256,
    Size,
    Content,
    Tx,
    // Entry body members the selector reads (law §6.2, §6.3, §6.4).
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

/// Kit paths of the items this layer adds itself (the verification note). Closed set.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    /// The verification note: how to check this kit (every kit carries it, so it is a fixed member of
    /// `files/`).
    Verify,
}

impl Slot {
    /// Kit path (the part under `files/`). Segments use only `a-z0-9._-` (kit law §7.2).
    pub fn path(self) -> &'static str {
        match self {
            Slot::Verify => "verify.md",
        }
    }

    pub const ALL: [Slot; 1] = [Slot::Verify];
}

/// Answer keys. Closed set.
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
