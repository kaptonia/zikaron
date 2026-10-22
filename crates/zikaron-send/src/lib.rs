//! The sender: one for every road that puts a home's anchors on chain (the desktop's queue, the command line
//! through the desktop, the command line with its credential). It reads the chain (`node`, `agree`), lets a
//! batch out only through the exit gate (`gate`), finds the key behind an address for a record package addressed
//! to it (`recipient`), keeps the queue's steps under one lock (`keep`) and signs and
//! broadcasts a batch, waits for its receipt and judges a batch no node holds (`batch`), and asks a home's write
//! gates from what is on disk before anything goes (`shut`). The command line's own record of what its direct
//! `anchor` sent lives here too (`sent`).
//!
//! It holds no key and no sentence for people. The key is lent by each end through a signing port
//! ([`batch::Signer`]); a home's queue file and ledger are read and written by each end under its own seal
//! ([`End`]); a refusal is a closed code with its evidence ([`Trouble`]), and each end says it in its own words.

#![forbid(unsafe_code)]

pub mod agree;
pub mod batch;
pub mod fingerprint;
pub mod gate;
pub mod keep;
pub mod node;
pub mod recipient;
pub mod sent;
pub mod shut;

/// Why the sender refused: a closed code. Each end maps every member, so a new one does not compile until it is
/// said there.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Code {
    /// No node for the chain asked about (or none configured).
    NoEndpoint,
    /// A node address that does not read, or a setting of another shape.
    SettingsShape,
    /// No node answered.
    Unreachable,
    /// Nodes answered differently, or one is too far behind.
    Disagree,
    /// A node answered in a shape that is not an answer to the question.
    ChainShape,
    /// Every node serves another chain.
    WrongChain,
    /// The node's own refusals, one member each (the shared table, `zikaron_anchor::said`).
    InsufficientFunds,
    NonceUsed,
    AlreadyPending,
    Underpriced,
    GasTooLow,
    ContractRefused,
    RateLimited,
    MethodMissing,
    NodeAuth,
    NodeRefused,
    /// The transport's failures: one of the four connection layers, the deadline, the cap, not JSON.
    NodeTls,
    NodeTimeout,
    AnswerTooLong,
    AnswerNotJson,
    /// The leg to a proxy failed; the proxy asked for credentials none were given for, or refused them.
    ProxyUnreachable,
    ProxyCredentialsMissing,
    ProxyCredentialsRefused,
    /// The node refused to estimate the call (it would revert), or estimated above the ceiling.
    GasRefused,
    /// The key gives no address.
    KeyMalformed,
    /// A resend signed by another key than the batch's.
    AddressMismatch,
    /// A transaction hash or entry id that is not thirty-two bytes.
    ContentShape,
    /// No entry to send.
    QueueEmpty,
    /// What the scan read is not an audit input.
    AuditInput,
    /// The scan was refused (its refusals in the evidence).
    ScanRefused,
    /// No chain set for this home.
    NoChainId,
    /// No registry set for this home.
    NoRegistry,
    /// Every hash asked about the chain already holds from this key: nothing is signed.
    OnChain,
    /// The home's write gates (`shut`): its ledger's own entries break its chain; it was handed off to another
    /// place; it is under its read-only mark, not fetched or with newer entries elsewhere.
    Broken,
    HandedOff,
    NotFetched,
    NewerElsewhere,
}

impl Code {
    /// Every member, in declaration order.
    pub const ALL: [Code; 37] = [
        Code::NoEndpoint,
        Code::SettingsShape,
        Code::Unreachable,
        Code::Disagree,
        Code::ChainShape,
        Code::WrongChain,
        Code::InsufficientFunds,
        Code::NonceUsed,
        Code::AlreadyPending,
        Code::Underpriced,
        Code::GasTooLow,
        Code::ContractRefused,
        Code::RateLimited,
        Code::MethodMissing,
        Code::NodeAuth,
        Code::NodeRefused,
        Code::NodeTls,
        Code::NodeTimeout,
        Code::AnswerTooLong,
        Code::AnswerNotJson,
        Code::ProxyUnreachable,
        Code::ProxyCredentialsMissing,
        Code::ProxyCredentialsRefused,
        Code::GasRefused,
        Code::KeyMalformed,
        Code::AddressMismatch,
        Code::ContentShape,
        Code::QueueEmpty,
        Code::AuditInput,
        Code::ScanRefused,
        Code::NoChainId,
        Code::NoRegistry,
        Code::OnChain,
        Code::Broken,
        Code::HandedOff,
        Code::NotFetched,
        Code::NewerElsewhere,
    ];

    /// The code as one word in capitals, spelled as the desktop's closed table spells the refusal it becomes (each
    /// end answers this word: the desktop's fault and the command line's `token` alike).
    pub fn as_str(self) -> &'static str {
        match self {
            Code::NoEndpoint => "NO_ENDPOINT",
            Code::SettingsShape => "SETTINGS_SHAPE",
            Code::Unreachable => "UNREACHABLE",
            Code::Disagree => "DISAGREE",
            Code::ChainShape => "CHAIN_SHAPE",
            Code::WrongChain => "WRONG_CHAIN",
            Code::InsufficientFunds => "INSUFFICIENT_FUNDS",
            Code::NonceUsed => "NONCE_USED",
            Code::AlreadyPending => "ALREADY_PENDING",
            Code::Underpriced => "UNDERPRICED",
            Code::GasTooLow => "GAS_TOO_LOW",
            Code::ContractRefused => "CONTRACT_REFUSED",
            Code::RateLimited => "RATE_LIMITED",
            Code::MethodMissing => "METHOD_MISSING",
            Code::NodeAuth => "NODE_AUTH",
            Code::NodeRefused => "NODE_REFUSED",
            Code::NodeTls => "NODE_TLS",
            Code::NodeTimeout => "NODE_TIMEOUT",
            Code::AnswerTooLong => "ANSWER_TOO_LONG",
            Code::AnswerNotJson => "ANSWER_NOT_JSON",
            Code::ProxyUnreachable => "PROXY_UNREACHABLE",
            Code::ProxyCredentialsMissing => "PROXY_CREDENTIALS_MISSING",
            Code::ProxyCredentialsRefused => "PROXY_CREDENTIALS_REFUSED",
            Code::GasRefused => "GAS_REFUSED",
            Code::KeyMalformed => "KEY_MALFORMED",
            Code::AddressMismatch => "ADDRESS_MISMATCH",
            Code::ContentShape => "CONTENT_SHAPE",
            Code::QueueEmpty => "QUEUE_EMPTY",
            Code::AuditInput => "AUDIT_INPUT",
            Code::ScanRefused => "SCAN_REFUSED",
            Code::NoChainId => "NO_CHAIN_ID",
            Code::NoRegistry => "NO_REGISTRY",
            Code::OnChain => "ON_CHAIN",
            Code::Broken => "BROKEN",
            Code::HandedOff => "HANDED_OFF",
            Code::NotFetched => "LEDGER_NOT_FETCHED",
            Code::NewerElsewhere => "NEWER_ELSEWHERE",
        }
    }

    /// The network family: refusals that are the absence of an answer, not a verdict every node would give alike
    /// (a broadcast moves to the next node on these, `node::next_after`).
    pub const NETWORK: [Code; 17] = [
        Code::Unreachable,
        Code::NoEndpoint,
        Code::Disagree,
        Code::ScanRefused,
        Code::ChainShape,
        Code::NodeTls,
        Code::ProxyUnreachable,
        Code::ProxyCredentialsMissing,
        Code::ProxyCredentialsRefused,
        Code::NodeTimeout,
        Code::AnswerTooLong,
        Code::AnswerNotJson,
        Code::RateLimited,
        Code::MethodMissing,
        Code::NodeAuth,
        Code::WrongChain,
        Code::NodeRefused,
    ];
}

/// A refusal's evidence: what was asked and what came back (nodes in their safe-to-say form, numbers, the nodes'
/// own words). No sentence lives here; each end lays each member out in its own words.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Why {
    /// Evidence as it is: a method, a hash, a number, a path, a system's words.
    Text(String),
    /// Several pieces, said one after another.
    Each(Vec<Why>),
    /// A node's failure: the node (safe to say) and what it said.
    Node { node: String, trouble: zikaron_anchor::rpc::Trouble },
    /// A node whose answer did not read.
    Unreadable { node: String },
    /// A node address whose scheme is not `http` or `https`.
    Scheme { node: String },
    /// A node address whose port is not a number from 0 to 65535.
    Port { node: String },
    /// A node address that does not read otherwise.
    NotAddress { node: String },
    /// A node spec (`<chain>=<url>`) that does not read as one.
    NotSpec { node: String },
    /// Another piece, about one chain (`<chain>=…`).
    AtChain { chain: u64, why: Box<Why> },
    /// No node for this chain's head.
    NoHeadNode { chain: u64 },
    /// No node for this chain's time.
    NoTimeNode { chain: u64 },
    /// A node's height that is not a quantity of 64 bits.
    NotHeight { node: String, text: String },
    /// A node behind the highest by more than the lag allowed.
    Behind { node: String, lag: u64, high: u64 },
    /// A block answer without its time or number.
    NoTime { node: String },
    /// Some nodes hold it and others not yet.
    NotYetAt { has: String, not_yet: String },
    /// How many sources answered differently, and which.
    Differ { count: usize, sources: String },
    /// Nodes that answered with no records where another answered with some.
    NoHistory { nodes: String },
    /// A node serving another chain, left out.
    WrongChainNode { node: String, serves: u64, chain: u64 },
    /// A balance that is not text, or not a quantity.
    BalanceNotText(String),
    BalanceNotQty(String),
    /// A gas estimate that is not text, or not a quantity.
    GasNotText(String),
    GasNotQty(String),
    /// The key gives no address.
    NoAddress,
    /// The balance gate: what the batch may spend and what the account holds, in wei.
    Funds { need: u128, have: u128 },
    /// The hashes the chain already holds from this key, and the first one's transaction and block.
    OnChain { names: Vec<String>, tx: String, block: u64 },
    /// No node configured to scan with.
    NoNodes,
    /// A node whose answer to the nonce question did not read as a quantity.
    NonceUnread { node: String },
    /// A scan's refusal: who did not answer and why, and the scan's own refusals.
    Scan { unanswered: Vec<Why>, refused: Vec<zikaron_anchor::scan::Refusal> },
}

/// One refusal: its code, its evidence, and, for a connection made without the system's proxy script because the
/// script could not be used, that script's trouble.
#[derive(Clone, PartialEq, Debug)]
pub struct Trouble {
    pub code: Code,
    pub why: Why,
    pub script: Option<zikaron_net::PacTrouble>,
}

impl Trouble {
    pub fn new(code: Code, why: Why) -> Trouble {
        Trouble { code, why, script: None }
    }

    /// With evidence as it is.
    pub fn text(code: Code, text: impl Into<String>) -> Trouble {
        Trouble::new(code, Why::Text(text.into()))
    }

    /// The same refusal with other evidence (every node's words for one attempt): its code and script stay.
    pub fn retold(mut self, why: Why) -> Trouble {
        self.why = why;
        self
    }

    /// Whether this is the network's absence of an answer rather than a node's verdict.
    pub fn network(&self) -> bool {
        Code::NETWORK.contains(&self.code)
    }
}

/// Where a running send is, for the end that shows it: the stage it entered (0 signing, 1 broadcasting), and a
/// balance read.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Note {
    Stage(u8),
    Balance,
}

/// What the sender needs from the end it runs for: the home's queue file read and written under that end's seal,
/// the home's ledger, how the end words the exit gate's two refusals, and where a send is. Each end implements it
/// once (the desktop for its open home, the command line for a home opened with its credential).
pub trait End {
    /// The end's own refusal: the sender's are turned into it (`From<Trouble>`), the sealed layer's that the sender
    /// meets on the end's own files are the end's to word (`From<zikaron_seal::Trouble>`), the end's own pass
    /// through.
    type Fail: From<Trouble> + From<zikaron_seal::Trouble>;
    /// The code a refusal carries as the queue records it on a batch refused before broadcast (`Step::Refused`).
    fn code_of(f: &Self::Fail) -> Option<String>;
    /// The queue as the file holds it (no file: empty).
    fn read_queue(&self) -> Result<zikaron_seal::queue::Queue, Self::Fail>;
    /// Write the queue whole.
    fn write_queue(&self, q: &zikaron_seal::queue::Queue) -> Result<(), Self::Fail>;
    /// The home's ledger entries, as stored.
    fn pile(&self) -> Result<Vec<Vec<u8>>, Self::Fail>;
    /// The exit gate found `missing` anchors of this ledger's lineage with no entry here: place the home's
    /// read-only mark and say the refusal (a mark that cannot be written is the refusal instead).
    fn behind(&self, missing: usize) -> Self::Fail;
    /// The exit gate could not read the chain: the refusal as the end words it.
    fn unread(&self, f: Self::Fail) -> Self::Fail;
    /// The home's read-only mark as its sealed file holds it (`zikaron_seal::unfetched`; none: `None`).
    fn mark(&self) -> Result<Option<zikaron_seal::unfetched::State>, Self::Fail>;
    /// This user's machine folder (the list of ledgers handed off from here lives there, `zikaron_seal::pens`).
    fn machine(&self) -> Result<std::path::PathBuf, Self::Fail>;
    /// Where a send is (the end may show it or not).
    fn note(&self, _n: Note) {}
}

/// This machine's record of chain facts already checked by several nodes alike (`zikaron_anchor::scan::Known`):
/// a scan asks less about what it holds, and hands it what each run read once the runs agree (each run's chains
/// and facts, and the chains only one endpoint answered), for it to keep what it judges agreed. Each end keeps its
/// own, or none.
pub trait Facts {
    fn known(&self) -> zikaron_anchor::scan::Known;
    fn add(&self, runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], thin: &[u64]);
}

/// No record: every fact is asked about, nothing is kept.
pub struct Bare;

impl Facts for Bare {
    fn known(&self) -> zikaron_anchor::scan::Known {
        zikaron_anchor::scan::Known::default()
    }
    fn add(&self, _runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], _thin: &[u64]) {}
}
