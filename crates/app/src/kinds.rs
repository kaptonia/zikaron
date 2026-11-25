#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Kind {
    SelfCheck,
    Archive,
    Chain,
    ChainTime,
    Reconcile,
    Audit,
    Ledger,
    Anchor,
    Kit,
    Grants,
    Book,
    Diligence,
    Verify,
    Delivery,
    Review,
    Held,
    Check,
    Badge,
    Keystore,
    Vault,
    Publish,
    Fetch,
    Vet,
    Peek,
    Backup,
    Gate,
    ReadNet,
    Basis,
    Gas,
    Take,
    Record,
    Migrate,
    Path,
    CliPath,
    Seat,
    Extract,
    CleanList,
    Clean,
    Rotate,
    Persona,
    FaceBooks,
    RerootFee,
    FaceJoined,
    Behind,
    Look,
    Regain,
    Keep,
}

impl Kind {
    pub fn waited_at_quit(self) -> bool {
        match self {
            Kind::Keystore | Kind::Backup | Kind::Kit | Kind::Badge | Kind::Vault | Kind::Reconcile | Kind::Migrate | Kind::Seat | Kind::Extract | Kind::Clean | Kind::Keep => true,
            Kind::Chain
            | Kind::ChainTime
            | Kind::Audit
            | Kind::Anchor
            | Kind::Fetch
            | Kind::Review
            | Kind::Book
            | Kind::Diligence
            | Kind::Verify
            | Kind::Check
            | Kind::Publish
            | Kind::SelfCheck
            | Kind::Archive
            | Kind::Ledger
            | Kind::Grants
            | Kind::Delivery
            | Kind::Held
            | Kind::Vet
            | Kind::Peek
            | Kind::Gate
            | Kind::ReadNet
            | Kind::Basis
            | Kind::Gas
            | Kind::Take
            | Kind::Record
            | Kind::Path
            | Kind::CliPath
            | Kind::CleanList
            | Kind::Rotate
            | Kind::RerootFee
            | Kind::Persona
            | Kind::FaceJoined
            | Kind::FaceBooks
            | Kind::Behind
            | Kind::Look
            | Kind::Regain => false,
        }
    }

    pub fn writes_local(self) -> bool {
        match self {
            Kind::Fetch | Kind::Audit | Kind::Review | Kind::Anchor | Kind::Reconcile | Kind::Keystore | Kind::Record | Kind::Migrate | Kind::Seat | Kind::Clean | Kind::Keep => true,
            Kind::SelfCheck
            | Kind::Archive
            | Kind::Chain
            | Kind::Ledger
            | Kind::Kit
            | Kind::Grants
            | Kind::Book
            | Kind::Diligence
            | Kind::Verify
            | Kind::Delivery
            | Kind::Held
            | Kind::Check
            | Kind::Badge
            | Kind::Vault
            | Kind::Publish
            | Kind::Vet
            | Kind::Peek
            | Kind::Backup
            | Kind::Gate
            | Kind::ReadNet
            | Kind::Basis
            | Kind::Gas
            | Kind::Take
            | Kind::Extract
            | Kind::Path
            | Kind::CliPath
            | Kind::ChainTime
            | Kind::CleanList
            | Kind::Rotate
            | Kind::RerootFee
            | Kind::Persona
            | Kind::FaceJoined
            | Kind::FaceBooks
            | Kind::Behind
            | Kind::Look
            | Kind::Regain => false,
        }
    }

    pub fn keeps_reads(self) -> bool {
        match self {
            Kind::Check => false,
            Kind::SelfCheck
            | Kind::Archive
            | Kind::Chain
            | Kind::Reconcile
            | Kind::Audit
            | Kind::Ledger
            | Kind::Anchor
            | Kind::Kit
            | Kind::Grants
            | Kind::Book
            | Kind::Diligence
            | Kind::Verify
            | Kind::Delivery
            | Kind::Review
            | Kind::Held
            | Kind::Badge
            | Kind::Keystore
            | Kind::Vault
            | Kind::Publish
            | Kind::Fetch
            | Kind::Vet
            | Kind::Peek
            | Kind::Backup
            | Kind::Gate
            | Kind::ReadNet
            | Kind::Basis
            | Kind::Gas
            | Kind::Take
            | Kind::Record
            | Kind::Migrate
            | Kind::Path
            | Kind::CliPath
            | Kind::Seat
            | Kind::Extract
            | Kind::ChainTime
            | Kind::Rotate
            | Kind::RerootFee
            | Kind::CleanList
            | Kind::Clean
            | Kind::Persona
            | Kind::FaceJoined
            | Kind::FaceBooks
            | Kind::Behind
            | Kind::Look
            | Kind::Regain
            | Kind::Keep => true,
        }
    }

    pub const ALL: [Kind; 47] = [
        Kind::SelfCheck,
        Kind::Archive,
        Kind::Chain,
        Kind::Reconcile,
        Kind::Audit,
        Kind::Ledger,
        Kind::Anchor,
        Kind::Kit,
        Kind::Grants,
        Kind::Book,
        Kind::Diligence,
        Kind::Verify,
        Kind::Delivery,
        Kind::Review,
        Kind::Held,
        Kind::Badge,
        Kind::Check,
        Kind::Keystore,
        Kind::Vault,
        Kind::Publish,
        Kind::Fetch,
        Kind::Vet,
        Kind::Peek,
        Kind::Backup,
        Kind::Gate,
        Kind::ReadNet,
        Kind::Basis,
        Kind::Gas,
        Kind::Take,
        Kind::Record,
        Kind::Migrate,
        Kind::Path,
        Kind::CliPath,
        Kind::Seat,
        Kind::Extract,
        Kind::ChainTime,
        Kind::Rotate,
        Kind::CleanList,
        Kind::Clean,
        Kind::Persona,
        Kind::FaceBooks,
        Kind::RerootFee,
        Kind::FaceJoined,
        Kind::Behind,
        Kind::Look,
        Kind::Regain,
        Kind::Keep,
    ];

    pub fn reads_nodes(self) -> bool {
        matches!(self, Kind::Chain | Kind::Audit | Kind::ChainTime)
    }

    pub fn follows_source(self) -> bool {
        match self {
            Kind::Archive
            | Kind::Chain
            | Kind::Reconcile
            | Kind::Audit
            | Kind::Ledger
            | Kind::Grants
            | Kind::Review
            | Kind::Held
            | Kind::Gate
            | Kind::Basis
            | Kind::Gas
            | Kind::Take
            | Kind::Record
            | Kind::Migrate
            | Kind::Persona
            | Kind::FaceJoined
            | Kind::FaceBooks
            | Kind::Look => true,
            Kind::Regain | Kind::Keep => false,
            Kind::SelfCheck | Kind::Anchor | Kind::Kit | Kind::Book | Kind::Diligence | Kind::Verify | Kind::Delivery | Kind::Badge | Kind::Check | Kind::Keystore | Kind::Vault | Kind::Publish | Kind::Fetch | Kind::Vet | Kind::Peek | Kind::Backup | Kind::ReadNet | Kind::Path | Kind::CliPath | Kind::Seat | Kind::Extract | Kind::ChainTime | Kind::Rotate | Kind::RerootFee | Kind::CleanList | Kind::Clean | Kind::Behind => false,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Kind::SelfCheck => "selfcheck",
            Kind::Archive => "archive",
            Kind::Chain => "chain",
            Kind::Reconcile => "reconcile",
            Kind::Audit => "audit",
            Kind::Ledger => "ledger",
            Kind::Anchor => "anchor",
            Kind::Kit => "kit",
            Kind::Grants => "grants",
            Kind::Book => "book",
            Kind::Diligence => "diligence",
            Kind::Verify => "verify",
            Kind::Delivery => "delivery",
            Kind::Review => "review",
            Kind::Held => "held",
            Kind::Badge => "badge",
            Kind::Check => "check",
            Kind::Keystore => "keystore",
            Kind::Vault => "vault",
            Kind::Publish => "publish",
            Kind::Fetch => "fetch",
            Kind::Vet => "vet",
            Kind::Peek => "peek",
            Kind::Backup => "backup",
            Kind::Gate => "gate",
            Kind::ReadNet => "readnet",
            Kind::Basis => "basis",
            Kind::Gas => "gas",
            Kind::Take => "take",
            Kind::Record => "record",
            Kind::Migrate => "migrate",
            Kind::Path => "path",
            Kind::CliPath => "clipath",
            Kind::Seat => "seat",
            Kind::Extract => "extract",
            Kind::ChainTime => "chainTime",
            Kind::Rotate => "rotate",
            Kind::CleanList => "cleanList",
            Kind::Clean => "clean",
            Kind::Persona => "persona",
            Kind::FaceBooks => "face-books",
            Kind::RerootFee => "reroot-fee",
            Kind::FaceJoined => "face-joined",
            Kind::Behind => "behind",
            Kind::Look => "look",
            Kind::Regain => "regain",
            Kind::Keep => "keep",
        }
    }
}

impl Kind {
    pub fn keeps_notices(self) -> bool {
        self == Kind::Check
    }

    pub fn keeper() -> Option<std::sync::Arc<dyn zikaron_anchor::settled::Keep>> {
        Some(crate::checkedx::settled_keeper())
    }

    pub fn wakes_when_done(self) -> bool {
        matches!(self, Kind::Look | Kind::Regain)
    }

    pub fn carries(self) -> bool {
        matches!(self, Kind::Gas | Kind::Anchor)
    }
}

