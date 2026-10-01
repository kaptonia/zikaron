//! Exit codes, refusal tokens, output keys and law field names, each spelled once.
//!
//! Every free name this crate prints (keys, refusals, field names) is written once in this file; everything
//! else uses these `as_str()`. `CLI-SCHEMA.md` is the readable form of these four tables.
//!
//! The law's own bytes are not copied: `zikaron/1` comes from [`zikaron::tokens::SPEC`], the seven type names
//! from [`zikaron::tokens::EntryType::as_str`], signing domains from [`zikaron::tokens::Domain::as_str`], the
//! three kit specs and `KIT_OK` / `BADGE_OK` from [`zikaron_kit::tokens`].

/// Exit codes. Five, closed.
///
/// Two questions split them: was there an answer, and what was it.
///
/// | code | meaning | stdout |
/// |---|---|---|
/// | 0 | answered, affirmative (entry stands, GREEN, COMPLETE, KIT_OK, PAIRED) | one canonical JSON value |
/// | 1 | answered, negative (the law refused the bytes, FAIL, BROKEN_CHAIN, NO_LABEL) | one canonical JSON
/// value |
/// | 2 | misuse: malformed arguments, unreadable path, a flag the verb does not know | zero bytes |
/// | 3 | answered, neither: PARTIAL / GAPS / UNAVAILABLE, its own state, never merged into green | one
/// canonical JSON value |
/// | 4 | no answer: endpoint unreachable, readings disagree, scan declined. Law §9.4: a scan failure is the
/// absence of an answer, never an answer of absence | one canonical JSON value with `reason` |
///
/// Codes 3 and 4 carry the weight of the table. Folding PARTIAL into 0 would let a buyer take an unanchored
/// grant as a green light; folding an unreachable endpoint into 1 would make a network fault read as "this
/// chain is fake".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exit {
    Affirmed,
    Denied,
    Misuse,
    Partial,
    Unanswered,
}

impl Exit {
    pub fn code(self) -> u8 {
        match self {
            Exit::Affirmed => 0,
            Exit::Denied => 1,
            Exit::Misuse => 2,
            Exit::Partial => 3,
            Exit::Unanswered => 4,
        }
    }

    pub const ALL: [Exit; 5] = [
        Exit::Affirmed,
        Exit::Denied,
        Exit::Misuse,
        Exit::Partial,
        Exit::Unanswered,
    ];
}

/// This layer's own refusals. Tokens from the core are passed on unchanged and never renamed here; this table
/// only covers what happened on the shell side.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Reason {
    /// Malformed arguments (misuse, stderr only).
    Args,
    /// A file cannot be read (misuse, stderr only, subject named).
    Unreadable,
    /// The private key is not a scalar in [1, n−1].
    Key,
    /// The machine gives no randomness.
    Random,
    /// Storage refused (with the storage crate's own code).
    Ledger,
    /// The core refused the entry we built (with the core's token).
    Entry,
    /// This lineage has no entry yet: run `init` first.
    TipAbsent,
    /// The named entry is not in the pile (different from an empty lineage).
    EntryAbsent,
    /// More than one entry at the highest seq: the ledger forked, and this layer does not guess which to
    /// follow.
    TipForked,
    /// The anchoring crate declined the scan (with its code).
    Scan,
    /// Multi-endpoint readings disagree.
    EndpointsDisagree,
    /// Endpoint unreachable, cannot send, or cannot wait.
    Unreachable,
    /// The transaction was included with a status other than 1.
    TxStatus,
    /// Not included by the deadline.
    TxNotYet,
    /// The kit core refused the document we built (with its token).
    Doc,
    /// The kit core judged the kit invalid (with the kit law verdict).
    Kit,
    /// The kit core refused encoding or decoding a payload (with its token).
    Badge,
    /// The fragment lacks one of anchors / basis / evidence and cannot become an audit input.
    Fragment,
    /// `init`: this ledger already has a root (the core recognizes exactly one seq 0 entry). One ledger, one
    /// root.
    AlreadyRooted,
    /// `init`: the ledger is not empty and the core recognizes no root (unreadable stray files, entry-named
    /// bytes the core refuses, entries without a root). Genesis is the first entry; with something already
    /// there, a second root could not be ruled out, so nothing is written.
    NotEmpty,
    /// `retract`: the retraction convention's writing rule refused (with the convention's token,
    /// `zikaron_glue::retraction`). A malformed subject, one not on this ledger's lineage, not a work record,
    /// or already deleted each have a token; the ledger is unchanged.
    Retraction,
}

impl Reason {
    pub fn as_str(self) -> &'static str {
        match self {
            Reason::Args => "E_ARGS",
            Reason::Unreadable => "E_UNREADABLE",
            Reason::Key => "E_KEY",
            Reason::Random => "E_RANDOM",
            Reason::Ledger => "E_LEDGER",
            Reason::Entry => "E_ENTRY",
            Reason::TipAbsent => "E_TIP_ABSENT",
            Reason::EntryAbsent => "E_ENTRY_ABSENT",
            Reason::TipForked => "E_TIP_FORKED",
            Reason::Scan => "E_SCAN",
            Reason::EndpointsDisagree => "E_ENDPOINTS_DISAGREE",
            Reason::Unreachable => "E_UNREACHABLE",
            Reason::TxStatus => "E_TX_STATUS",
            Reason::TxNotYet => "E_TX_NOT_YET",
            Reason::Doc => "E_DOC",
            Reason::Kit => "E_KIT",
            Reason::Badge => "E_BADGE",
            Reason::Fragment => "E_FRAGMENT",
            Reason::AlreadyRooted => "E_ALREADY_ROOTED",
            Reason::NotEmpty => "E_NOT_EMPTY",
            Reason::Retraction => "E_RETRACTION",
        }
    }

    pub const ALL: [Reason; 21] = [
        Reason::Args,
        Reason::Unreadable,
        Reason::Key,
        Reason::Random,
        Reason::Ledger,
        Reason::Entry,
        Reason::TipAbsent,
        Reason::EntryAbsent,
        Reason::TipForked,
        Reason::Scan,
        Reason::EndpointsDisagree,
        Reason::Unreachable,
        Reason::TxStatus,
        Reason::TxNotYet,
        Reason::Doc,
        Reason::Kit,
        Reason::Badge,
        Reason::Fragment,
        Reason::AlreadyRooted,
        Reason::NotEmpty,
        Reason::Retraction,
    ];
}

/// Output object keys. Closed: every member name this crate prints is one of these.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Ok,
    Reason,
    Detail,
    Dropped,
    Token,
    Index,
    State,
    EntryId,
    EntryType,
    Seq,
    Prev,
    Author,
    Address,
    Privkey,
    Ledger,
    Entries,
    Files,
    Proofs,
    KitId,
    Path,
    Fragment,
    Sources,
    SingleSource,
    SingleSourceChains,
    Tx,
    BlockNumber,
    Attestor,
    Attestation,
    DocId,
    Doc,
    Payload,
    Count,
    Written,
    Names,
    Value,
}

impl Key {
    pub fn as_str(self) -> &'static str {
        match self {
            Key::Ok => "ok",
            Key::Reason => "reason",
            Key::Detail => "detail",
            Key::Dropped => "dropped",
            Key::Token => "token",
            Key::Index => "index",
            Key::State => "state",
            Key::EntryId => "entryId",
            Key::EntryType => "entryType",
            Key::Seq => "seq",
            Key::Prev => "prev",
            Key::Author => "author",
            Key::Address => "address",
            Key::Privkey => "privkey",
            Key::Ledger => "ledger",
            Key::Entries => "entries",
            Key::Files => "files",
            Key::Proofs => "proofs",
            Key::KitId => "kitId",
            Key::Path => "path",
            Key::Fragment => "fragment",
            Key::Sources => "sources",
            Key::SingleSource => "singleSource",
            Key::SingleSourceChains => "singleSourceChains",
            Key::Tx => "tx",
            Key::BlockNumber => "blockNumber",
            Key::Attestor => "attestor",
            Key::Attestation => "attestation",
            Key::DocId => "docId",
            Key::Doc => "doc",
            Key::Payload => "payload",
            Key::Count => "count",
            Key::Written => "written",
            Key::Names => "names",
            Key::Value => "value",
        }
    }
    /// The whole set; `CLI-SCHEMA.md` section 7 lists it cell by cell (checked by the tests).
    pub const ALL: [Key; 35] = [
        Key::Ok,
        Key::Reason,
        Key::Detail,
        Key::Dropped,
        Key::Token,
        Key::Index,
        Key::State,
        Key::EntryId,
        Key::EntryType,
        Key::Seq,
        Key::Prev,
        Key::Author,
        Key::Address,
        Key::Privkey,
        Key::Ledger,
        Key::Entries,
        Key::Files,
        Key::Proofs,
        Key::KitId,
        Key::Path,
        Key::Fragment,
        Key::Sources,
        Key::SingleSource,
        Key::SingleSourceChains,
        Key::Tx,
        Key::BlockNumber,
        Key::Attestor,
        Key::Attestation,
        Key::DocId,
        Key::Doc,
        Key::Payload,
        Key::Count,
        Key::Written,
        Key::Names,
        Key::Value,
    ];

}

/// The shell's own words: the flag values it recognizes. Closed, one spelling each for `--form` and the two
/// `badge` sides.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Word {
    Registry,
    Bare,
}

impl Word {
    pub fn as_str(self) -> &'static str {
        match self {
            Word::Registry => "registry",
            Word::Bare => "bare",
        }
    }
}

/// Law field names: the seven envelope members (law §4.1), the body members of the seven types (law §6), kit
/// document members (kit law §4, §5) and kit manifest members (kit law §7.3).
///
/// These names belong to the law and would ideally be exported by the core and the kit core, which today
/// spell them only inside their `check_*` functions. Until they export constants, the shell keeps them in
/// this one closed table, so switching to re-exports later touches only this file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Field {
    // Envelope (law §4.1).
    Spec,
    EntryType,
    Author,
    Seq,
    Prev,
    Body,
    Sig,
    // Bodies (law §6.1 to §6.8).
    StatementMd,
    Content,
    Mode,
    Mark,
    Toolchain,
    NoteMd,
    Grantee,
    Work,
    Terms,
    History,
    Window,
    From,
    To,
    ScopeMd,
    Grant,
    Case,
    Anchors,
    ChainId,
    Tx,
    PayloadKind,
    Attestor,
    Attestation,
    Kind,
    Effective,
    Subject,
    // The member the kit reading adds to a grant body (parent law §6.10 treats it as data).
    Upstream,
    // The three tables of a law §9.4 basis (the offline lineage reading uses a basis with all three empty).
    AdoptionChains,
    BareTx,
    Chains,
    // Kit documents (kit law §4, §5).
    Rows,
    Recipient,
    Variant,
    Fpm,
    // Kit manifest (kit law §7.3).
    Entries,
    Files,
    Contents,
    Proofs,
    Root,
    Sha256,
    Size,
    Path,
}

impl Field {
    pub fn as_str(self) -> &'static str {
        match self {
            Field::Spec => "spec",
            Field::EntryType => "entryType",
            Field::Author => "author",
            Field::Seq => "seq",
            Field::Prev => "prev",
            Field::Body => "body",
            Field::Sig => "sig",
            Field::StatementMd => "statement_md",
            Field::Content => "content",
            Field::Mode => "mode",
            Field::Mark => "mark",
            Field::Toolchain => "toolchain",
            Field::NoteMd => "note_md",
            Field::Grantee => "grantee",
            Field::Work => "work",
            Field::Terms => "terms",
            Field::History => "history",
            Field::Window => "window",
            Field::From => "from",
            Field::To => "to",
            Field::ScopeMd => "scope_md",
            Field::Grant => "grant",
            Field::Case => "case",
            Field::Anchors => "anchors",
            Field::ChainId => "chainId",
            Field::Tx => "tx",
            Field::PayloadKind => "payloadKind",
            Field::Attestor => "attestor",
            Field::Attestation => "attestation",
            Field::Kind => "kind",
            Field::Effective => "effective",
            Field::Subject => "subject",
            Field::Upstream => "upstream",
            Field::AdoptionChains => "adoptionChains",
            Field::BareTx => "bareTx",
            Field::Chains => "chains",
            Field::Rows => "rows",
            Field::Recipient => "recipient",
            Field::Variant => "variant",
            Field::Fpm => "fpm",
            Field::Entries => "entries",
            Field::Files => "files",
            Field::Contents => "contents",
            Field::Proofs => "proofs",
            Field::Root => "root",
            Field::Sha256 => "sha256",
            Field::Size => "size",
            Field::Path => "path",
        }
    }
}
