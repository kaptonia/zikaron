pub const K4: &str = "K4";

pub const SPEC_KIT: &str = "zikaron.kit/2";

pub const ONE_FILE_MAGIC: &[u8] = b"zikaron-kit-file/1\n";

pub const MANIFEST: &str = "manifest.json";

pub const GRANT_CODE: &str = "files/zikaron-grant.txt";

pub const TERMS_PREFIX: &str = "files/terms/";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KitToken {
    Unreadable,
    ManifestAbsent,
    Manifest,
    EntryBytes,
    File,
    Content,
    Extra,
}

impl KitToken {
    pub fn as_str(self) -> &'static str {
        match self {
            KitToken::Unreadable => "E_KIT_UNREADABLE",
            KitToken::ManifestAbsent => "E_KIT_MANIFEST_ABSENT",
            KitToken::Manifest => "E_KIT_MANIFEST",
            KitToken::EntryBytes => "E_KIT_ENTRY_BYTES",
            KitToken::File => "E_KIT_FILE",
            KitToken::Content => "E_KIT_CONTENT",
            KitToken::Extra => "E_KIT_EXTRA",
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
    AnchoredOn,
    Anchors,
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
            Rule::AnchoredOn => "anchoredOn",
            Rule::Anchors => "anchors",
            Rule::NoteMd => "note_md",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GpToken {
    NoCode,
    Root,
    Uncarried,
    Content,
    Terms,
}

impl GpToken {
    pub fn as_str(self) -> &'static str {
        match self {
            GpToken::NoCode => "E_GP_NO_CODE",
            GpToken::Root => "E_GP_ROOT",
            GpToken::Uncarried => "E_GP_UNCARRIED",
            GpToken::Content => "E_GP_CONTENT",
            GpToken::Terms => "E_GP_TERMS",
        }
    }
}

pub const GP_OK: &str = "GP_OK";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Ok,
    Token,
    Index,
    Inner,
    Payload,
    Grants,
    Verdict,
    Subject,
    KitId,
    Counts,
    Entries,
    Files,
    InvalidEntries,
    EntryId,
    Contents,
    Hops,
    Root,
    Terms,
    Content,
    Path,
    Sha256,
    Size,
    NameMd,
    Spec,
    AnchoredOn,
    Anchors,
    NoteMd,
    ChainId,
    Registry,
    FromBlock,
    Tx,
    Basis,
    Checks,
    Failed,
    N,
    State,
    Reason,
    Links,
    Failing,
    Kind,
    Notices,
    Revocations,
    Accessions,
    ToBlock,
    RevocationContract,
    Book,
    BookFromBlock,
    BlockNumber,
    TxIndex,
    LogIndex,
    Sender,
    Grant,
    Holder,
    To,
    Revocation,
    Chain,
    At,
}

impl Key {
    pub fn as_str(self) -> &'static str {
        match self {
            Key::Ok => "ok",
            Key::Token => "token",
            Key::Index => "index",
            Key::Inner => "inner",
            Key::Payload => "payload",
            Key::Grants => "grants",
            Key::Verdict => "verdict",
            Key::Subject => "subject",
            Key::KitId => "kit_id",
            Key::Counts => "counts",
            Key::Entries => "entries",
            Key::Files => "files",
            Key::InvalidEntries => "invalid_entries",
            Key::EntryId => "entry_id",
            Key::Contents => "contents",
            Key::Hops => "hops",
            Key::Root => "root",
            Key::Terms => "terms",
            Key::Content => "content",
            Key::Path => "path",
            Key::Sha256 => "sha256",
            Key::Size => "size",
            Key::NameMd => "name_md",
            Key::Spec => "spec",
            Key::AnchoredOn => "anchoredOn",
            Key::Anchors => "anchors",
            Key::NoteMd => "note_md",
            Key::ChainId => "chainId",
            Key::Registry => "registry",
            Key::FromBlock => "fromBlock",
            Key::Tx => "tx",
            Key::Basis => "basis",
            Key::Checks => "checks",
            Key::Failed => "failed",
            Key::N => "n",
            Key::State => "state",
            Key::Reason => "reason",
            Key::Links => "links",
            Key::Failing => "failing",
            Key::Kind => "kind",
            Key::Notices => "notices",
            Key::Revocations => "revocations",
            Key::Accessions => "accessions",
            Key::ToBlock => "toBlock",
            Key::RevocationContract => "revocationContract",
            Key::Book => "book",
            Key::BookFromBlock => "bookFromBlock",
            Key::BlockNumber => "blockNumber",
            Key::TxIndex => "txIndex",
            Key::LogIndex => "logIndex",
            Key::Sender => "sender",
            Key::Grant => "grant",
            Key::Holder => "holder",
            Key::To => "to",
            Key::Revocation => "revocation",
            Key::Chain => "chain",
            Key::At => "at",
        }
    }
}
