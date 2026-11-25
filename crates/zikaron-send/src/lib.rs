#![forbid(unsafe_code)]

pub mod agree;
pub mod batch;
pub mod behind;
pub mod book;
pub mod checked;
pub mod face;
pub mod fingerprint;
pub mod gate;
pub mod issue;
pub mod keep;
pub mod node;
pub mod notices;
pub mod one;
pub mod proofs;
pub mod recipient;
pub mod sent;
pub mod shut;
pub mod withdraw;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Code {
    NoEndpoint,
    SettingsShape,
    Unreachable,
    Disagree,
    ChainShape,
    WrongChain,
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
    NodeRefused,
    NodeTls,
    NodeTimeout,
    AnswerTooLong,
    AnswerNotJson,
    ProxyUnreachable,
    ProxyCredentialsMissing,
    ProxyCredentialsRefused,
    GasRefused,
    KeyMalformed,
    AddressMismatch,
    ContentShape,
    QueueEmpty,
    AuditInput,
    ScanRefused,
    NoChainId,
    NoRegistry,
    OnChain,
    TooEarly,
    Broken,
    HandedOff,
    NotFetched,
    NewerElsewhere,
    NotIncluded,
    NoNoticeVenue,
    NoticeContract,
    BookCode,
    RootNotInBook,
    NotAcceded,
    OfficeElsewhere,
    OfficeIsContract,
    RegistryCode,
}

impl Code {
    pub fn another_chain(self) -> bool {
        matches!(self, Code::WrongChain)
    }

    pub const ALL: [Code; 49] = [
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
        Code::ServiceUnserved,
        Code::ServiceKeyMissing,
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
        Code::TooEarly,
        Code::Broken,
        Code::HandedOff,
        Code::NotFetched,
        Code::NewerElsewhere,
        Code::NotIncluded,
        Code::NoNoticeVenue,
        Code::NoticeContract,
        Code::BookCode,
        Code::RootNotInBook,
        Code::NotAcceded,
        Code::OfficeElsewhere,
        Code::OfficeIsContract,
        Code::RegistryCode,
    ];

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
            Code::ServiceUnserved => "SERVICE_UNSERVED",
            Code::ServiceKeyMissing => "SERVICE_KEY_MISSING",
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
            Code::TooEarly => "TOO_EARLY",
            Code::Broken => "BROKEN",
            Code::HandedOff => "HANDED_OFF",
            Code::NotFetched => "LEDGER_NOT_FETCHED",
            Code::NewerElsewhere => "NEWER_ELSEWHERE",
            Code::NotIncluded => "NOT_INCLUDED",
            Code::NoNoticeVenue => "NO_REVOCATION_CONTRACT",
            Code::NoticeContract => "REVOCATION_CODE",
            Code::BookCode => "BOOK_CODE",
            Code::RootNotInBook => "ROOT_NOT_IN_BOOK",
            Code::NotAcceded => "NOT_ACCEDED",
            Code::OfficeElsewhere => "BOOK_OFFICE_OTHER",
            Code::OfficeIsContract => "OFFICE_IS_CONTRACT",
            Code::RegistryCode => "REGISTRY_CODE",
        }
    }

    pub const NETWORK: [Code; 19] = [
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
        Code::ServiceUnserved,
        Code::ServiceKeyMissing,
        Code::WrongChain,
        Code::NodeRefused,
    ];
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Why {
    Text(String),
    Each(Vec<Why>),
    Node { node: String, trouble: zikaron_anchor::rpc::Trouble },
    Unreadable { node: String },
    Differs { node: String },
    Scheme { node: String },
    Port { node: String },
    NotAddress { node: String },
    NotSpec { node: String },
    AtChain { chain: u64, why: Box<Why> },
    NoHeadNode { chain: u64 },
    NoTimeNode { chain: u64 },
    NotHeight { node: String, text: String },
    Behind { node: String, lag: u64, high: u64 },
    NoTime { node: String },
    NotYetAt { has: String, not_yet: String },
    Differ { count: usize, sources: String },
    NoHistory { nodes: String },
    WrongChainNode { node: String, serves: u64, chain: u64 },
    BalanceNotText(String),
    BalanceNotQty(String),
    GasNotText(String),
    GasNotQty(String),
    NoAddress,
    Funds { need: u128, have: u128 },
    OnChain { names: Vec<String>, tx: String, block: u64 },
    TooEarly { chain: u64, head_time: u64, after: u64 },
    NoNodes,
    NonceUnread { node: String },
    Scan { unanswered: Vec<Why>, refused: Vec<zikaron_anchor::scan::Refusal> },
}

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

    pub fn text(code: Code, text: impl Into<String>) -> Trouble {
        Trouble::new(code, Why::Text(text.into()))
    }

    pub fn retold(mut self, why: Why) -> Trouble {
        self.why = why;
        self
    }

    pub fn network(&self) -> bool {
        Code::NETWORK.contains(&self.code)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Note {
    Stage(u8),
    Balance,
}

pub trait End {
    type Fail: From<Trouble> + From<zikaron_seal::Trouble>;
    fn code_of(f: &Self::Fail) -> Option<String>;
    fn read_queue(&self) -> Result<zikaron_seal::queue::Queue, Self::Fail>;
    fn write_queue(&self, q: &zikaron_seal::queue::Queue) -> Result<(), Self::Fail>;
    fn pile(&self) -> Result<Vec<Vec<u8>>, Self::Fail>;
    fn behind(&self, lacks: &[String]) -> Self::Fail;
    fn unread(&self, f: Self::Fail) -> Self::Fail;
    fn mark(&self) -> Result<Option<zikaron_seal::unfetched::State>, Self::Fail>;
    fn lift(&self) -> Result<(), Self::Fail>;
    fn machine(&self) -> Result<std::path::PathBuf, Self::Fail>;
    fn tail(&self) -> Result<(), Self::Fail> {
        Ok(())
    }
    fn note(&self, _n: Note) {}
}

pub trait Facts {
    fn known(&self) -> zikaron_anchor::scan::Known;
    fn known_single(&self) -> zikaron_anchor::scan::Known {
        self.known()
    }
    fn add(&self, runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], thin: &[u64]);
}

pub struct Bare;

impl Facts for Bare {
    fn known(&self) -> zikaron_anchor::scan::Known {
        zikaron_anchor::scan::Known::default()
    }
    fn add(&self, _runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], _thin: &[u64]) {}
}

pub fn node_said(t: &zikaron_anchor::rpc::Trouble) -> Option<&str> {
    match t {
        zikaron_anchor::rpc::Trouble::Node(e) => Some(e.as_str()),
        _ => None,
    }
}

pub fn said(w: &Why) -> String {
    match w {
        Why::Text(t) => t.clone(),
        Why::Each(all) => all.iter().map(said).collect::<Vec<_>>().join("; "),
        Why::Node { node, trouble } => format!("{node}: {trouble:?}"),
        Why::Unreadable { node } => format!("{node}: answer unreadable"),
        Why::Differs { node } => format!("{node}: answered otherwise, not counted"),
        Why::Scheme { node } => format!("{node}: not http or https"),
        Why::Port { node } => format!("{node}: port not 0-65535"),
        Why::NotAddress { node } => format!("{node}: not an address"),
        Why::NotSpec { node } => format!("{node}: not <chain>=<url>"),
        Why::AtChain { chain, why } => format!("{chain}={}", said(why)),
        Why::NoHeadNode { chain } => format!("{chain}: no node for the head"),
        Why::NoTimeNode { chain } => format!("{chain}: no node for the time"),
        Why::NotHeight { node, text } => format!("{node}: height {text}"),
        Why::Behind { node, lag, high } => format!("{node}: {lag} blocks behind {high}"),
        Why::NoTime { node } => format!("{node}: block without time or number"),
        Why::NotYetAt { has, not_yet } => format!("at {has}; not yet at {not_yet}"),
        Why::Differ { count, sources } => format!("{count} differ: {sources}"),
        Why::NoHistory { nodes } => format!("no records at {nodes}"),
        Why::WrongChainNode { node, serves, chain } => format!("{node}: chain {serves}, not {chain}"),
        Why::BalanceNotText(x) | Why::BalanceNotQty(x) => format!("eth_getBalance: {x}"),
        Why::GasNotText(x) | Why::GasNotQty(x) => format!("eth_estimateGas: {x}"),
        Why::NoAddress => "no address".to_string(),
        Why::Funds { need, have } => format!("need {need} wei, have {have} wei"),
        Why::OnChain { names, tx, block } => format!("{} {tx} {block}", names.join(" ")),
        Why::TooEarly { chain, head_time, after } => format!("chainId={chain} headTime={head_time} after={after}"),
        Why::NoNodes => "no nodes".to_string(),
        Why::NonceUnread { node } => format!("{node}: nonce unreadable"),
        Why::Scan { unanswered, refused } => {
            let mut parts: Vec<String> = unanswered.iter().map(said).collect();
            parts.extend(refused.iter().map(crate::agree::scan_tail));
            parts.join("; ")
        }
    }
}
