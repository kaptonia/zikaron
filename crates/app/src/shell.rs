//! Shell state, used by both the window and the test hooks. This layer does not know the zikaron/1 law: it
//! knows the build, fonts, channels and pages, plus identities and the archive.

use crate::fault::{Fault, Known};
use crate::feature::Feature;
use crate::home::Home;
use crate::key::Address;
use crate::lock::Lock;
use crate::auditx::Pen;
use crate::chainx::Endpoint;
use crate::probe::Report;
use crate::task::Done;
use crate::settings::Settings;
use crate::task::{Outcome, Tasks};
use crate::trace;
use zikaron_ui::fonts;

/// One self-audit reading. Every field comes back from the background task; nothing is recomputed in the
/// frame.
#[derive(Clone, Debug)]
pub struct AuditRead {
    /// The core's label, unchanged.
    pub label: String,
    pub complete: bool,
    /// Whether the chain broke. The app-wide write lock rests on this field.
    pub broken: bool,
    pub entries: usize,
    /// The core's report, unchanged; the screen lays out its fifteen items.
    pub report: zikaron::json::Value,
    /// Which endpoints did not answer this pass.
    pub unanswered: Vec<String>,
    pub asked: usize,
    pub single_source: bool,
    /// The fragment this pass used, unchanged.
    pub fragment: zikaron::json::Value,
    /// When it ran (the interface clock; only for scheduling, never for a decision).
    pub at: f64,
    /// Which ledger mark this report was computed for ([`Shell::book_mark`]).
    pub mark: u64,
}

/// The bar at the top of every page. Closed; nothing besides these.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Banner {
    /// Nothing shown.
    None,
    /// Broken chain: the whole desk is read-only; recover first.
    Broken,
    /// Cannot write (a reader, or the lock not yet taken), with who the writer is.
    ReadOnly(String),
    /// Already handed over by succession, with the new key.
    Handed(String),
}

impl Banner {
    pub fn as_str(&self) -> &'static str {
        match self {
            Banner::None => "none",
            Banner::Broken => "broken",
            Banner::ReadOnly(_) => "read_only",
            Banner::Handed(_) => "handed",
        }
    }
}

/// The pages. Only pages that exist are listed; a page not built is not in the navigation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    /// First run: what this machine still needs.
    FirstRun,
    /// Identity and keys.
    Identity,
    /// Archive and single writer.
    Archive,
    /// Mirror and chain.
    Mirror,
    /// Ledger view.
    Ledger,
    /// Disclosure kit output.
    Kit,
    /// Depth.
    Depth,
    /// Grant drafting.
    Grant,
    /// Grant ledger and double-sale guard.
    Grants,
    /// First-window checklist.
    FirstWindow,
    /// Revocation.
    Revoke,
    /// Adoption.
    Adopt,
    /// Succession.
    Succeed,
    /// Reading others' ledgers.
    Reader,
    /// Self-audit clock.
    Audit,
    /// Due diligence.
    Diligence,
    /// Record verification.
    Verifier,
    /// Checking a received record (a tab of the verify page).
    Delivery,
    /// Grant vault.
    Vault,
    /// Revocation sentinel.
    Sentinel,
    /// Multi-upstream ledger.
    Upstreams,
    /// Badges.
    Badge,
    /// Relicensing.
    Relicense,
    /// Grant check (shared, no identity needed).
    Check,
    /// Watch and notifications (shared).
    Watch,
    /// Anchoring desk.
    Anchoring,
    /// Anchor queue.
    Queue,
    /// Skeleton: this shell's own readings.
    Skeleton,
    /// About.
    About,
}

impl Page {
    /// Ordered by family: ledger and anchoring, grants, adoption and succession, disclosure and reading,
    /// machine and settings. Icons follow the family (see [`Page::icon`]): one icon per family, so a repeated
    /// icon marks pages of one family.
    pub const ALL: [Page; 29] = [
        // Ledger and anchoring.
        Page::Ledger,
        Page::Anchoring,
        Page::Queue,
        Page::Audit,
        // Grants.
        Page::Grant,
        Page::Grants,
        Page::Revoke,
        Page::FirstWindow,
        // Adoption and succession.
        Page::Adopt,
        Page::Succeed,
        // Disclosure and reading.
        Page::Kit,
        Page::Depth,
        Page::Reader,
        // Shared.
        Page::Check,
        Page::Watch,
        // Grantee.
        Page::Diligence,
        Page::Verifier,
        Page::Delivery,
        Page::Vault,
        Page::Upstreams,
        Page::Sentinel,
        Page::Relicense,
        Page::Badge,
        // Machine and settings.
        Page::FirstRun,
        Page::Identity,
        Page::Archive,
        Page::Mirror,
        Page::Skeleton,
        Page::About,
    ];

    /// The name this page shows now. Page names come only from here (rail, header, tests).
    ///
    /// It is separate from `key()` so that "page names follow the language" has its own point of failure:
    /// breaking the whole string table would break every sentence, while breaking this breaks only page
    /// names.
    pub fn title(self) -> &'static str {
        crate::lang::t(self.key())
    }

    /// The page name comes from the string table: no sentence is hard-coded in the interface.
    pub fn key(self) -> crate::lang::Key {
        match self {
            Page::FirstRun => crate::lang::Key::PageFirstRun,
            Page::Identity => crate::lang::Key::PageIdentity,
            Page::Archive => crate::lang::Key::PageArchive,
            Page::Mirror => crate::lang::Key::PageMirror,
            Page::Ledger => crate::lang::Key::PageLedger,
            Page::Kit => crate::lang::Key::PageKit,
            Page::Depth => crate::lang::Key::PageDepth,
            Page::Grant => crate::lang::Key::PageGrant,
            Page::Grants => crate::lang::Key::PageGrants,
            Page::FirstWindow => crate::lang::Key::PageFirstWindow,
            Page::Revoke => crate::lang::Key::PageRevoke,
            Page::Adopt => crate::lang::Key::PageAdopt,
            Page::Succeed => crate::lang::Key::PageSucceed,
            Page::Reader => crate::lang::Key::PageReader,
            Page::Audit => crate::lang::Key::PageAudit,
            Page::Diligence => crate::lang::Key::PageDiligence,
            Page::Verifier => crate::lang::Key::PageVerifier,
            Page::Delivery => crate::lang::Key::PageDelivery,
            Page::Vault => crate::lang::Key::PageVault,
            Page::Sentinel => crate::lang::Key::PageSentinel,
            Page::Upstreams => crate::lang::Key::PageUpstreams,
            Page::Badge => crate::lang::Key::PageBadge,
            Page::Relicense => crate::lang::Key::PageRelicense,
            Page::Check => crate::lang::Key::PageCheck,
            Page::Watch => crate::lang::Key::PageWatch,
            Page::Anchoring => crate::lang::Key::PageAnchoring,
            Page::Queue => crate::lang::Key::PageQueue,
            Page::Skeleton => crate::lang::Key::PageSkeleton,
            Page::About => crate::lang::Key::PageAbout,
        }
    }

    /// The page icon. One per family: pages of a family share a shape, so a repeat marks the family.
    pub fn icon(self) -> zikaron_ui::icons::Icon {
        use zikaron_ui::icons::Icon;
        match self {
            // Ledger and anchoring.
            Page::Ledger | Page::Anchoring | Page::Queue | Page::Audit => Icon::Ledger,
            // Grants.
            Page::Grant | Page::Grants | Page::Revoke | Page::FirstWindow => Icon::Grant,
            // Adoption and succession.
            Page::Adopt | Page::Succeed => Icon::Anchor,
            // Disclosure and reading.
            Page::Kit | Page::Depth | Page::Reader => Icon::Kit,
            // Grantee: verification and due diligence read bytes from the other party, like the disclosure
            // family.
            Page::Diligence | Page::Verifier | Page::Delivery => Icon::Kit,
            // Shared: checking reads the other party's payload, like the disclosure family.
            Page::Check => Icon::Kit,
            Page::Watch => Icon::Ledger,
            // Grantee credentials hold grants, like the grant family.
            Page::Vault
            | Page::Sentinel
            | Page::Upstreams
            | Page::Badge
            | Page::Relicense => Icon::Grant,
            // Machine and settings.
            Page::FirstRun
            | Page::Identity
            | Page::Archive
            | Page::Mirror
            | Page::Skeleton
            | Page::About => Icon::Gear,
        }
    }
}

/// Whether this pass actually got an answer from the chain. Only results that carry chain readings count;
/// those that turn unreachability into "unknown" (a review with an empty fragment, a check whose basis was
/// refused by name) do not.
fn chain_answered(d: &Done) -> bool {
    match d {
        Done::Chain { .. } | Done::Audited { .. } | Done::Sighting { .. } | Done::Book { .. } => true,
        Done::Anchored { confirmed, .. } => *confirmed,
        Done::Diligence(_) => true,
        Done::Verified(v) => v.review.is_ok(),
        Done::Reviewed { now, .. } => now.is_some(),
        Done::Checked(c) => c.basis.is_ok() || c.now_from == crate::checkx::NowFrom::Chain,
        _ => false,
    }
}

/// The key store as the shell holds it: the store's own reading (`keybox::State`, its closed four), or the
/// store's file there and unreadable (`keybox::state` refuses it by name, `KEYBOX_SHAPE`, kept here to be
/// said). The fifth member is the shell's alone: the store's table stays four, and a damaged file is never
/// read as "no store yet", which would open the first-run wizard over the keys the file holds. A damaged
/// store keeps the gate up and no key is ready.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Vault {
    Read(crate::keybox::State),
    Damaged(Fault),
}

impl Vault {
    /// The store's answer, as the shell holds it.
    pub fn of(read: Result<crate::keybox::State, Fault>) -> Vault {
        match read {
            Ok(s) => Vault::Read(s),
            Err(f) => Vault::Damaged(f),
        }
    }

    /// Whether the gate covers the window: the store's own table, and a damaged store.
    pub fn gate_up(&self) -> bool {
        match self {
            Vault::Read(s) => s.gate_up(),
            Vault::Damaged(_) => true,
        }
    }

    /// Whether a key can be had now: only from an open store.
    pub fn keys_ready(&self) -> bool {
        match self {
            Vault::Read(s) => s.keys_ready(),
            Vault::Damaged(_) => false,
        }
    }

    /// Whether this machine has no store yet (first run). A damaged store is a store.
    pub fn absent(&self) -> bool {
        matches!(self, Vault::Read(crate::keybox::State::Absent))
    }

    /// Whether the store reads as `s`.
    pub fn is(&self, s: crate::keybox::State) -> bool {
        matches!(self, Vault::Read(x) if *x == s)
    }

    /// Its name: the store's own word, or `damaged`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Vault::Read(s) => s.as_str(),
            Vault::Damaged(_) => "damaged",
        }
    }
}

/// Fetching found this home at odds with the fetched ledger (see `Done::FetchConflict`).
#[derive(Clone, Debug)]
pub struct Conflict {
    pub root: std::path::PathBuf,
    pub offline: usize,
    pub fetched: usize,
    /// This home's entries the fetched ledger lacks, with when each was queued.
    pub rows: Vec<(crate::ledgerx::Row, Option<u64>)>,
}

/// Old data on this machine: a home set aside after a conflict.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OldData {
    pub path: std::path::PathBuf,
    /// When it was set aside (Unix seconds).
    pub at: u64,
    pub entries: usize,
    /// Its entries never anchored (still in its queue).
    pub queued: usize,
}

/// Old data open to read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OldView {
    /// The home open before it (where "back" returns).
    pub back: std::path::PathBuf,
    pub at: u64,
}

/// Build kind, as it is.
pub fn build_kind() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}


/// A home reading. Every field is computed by a background disk walk; nothing is recomputed in the frame.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ArchiveRead {
    pub bytes: u64,
    pub items: usize,
    pub skipped: usize,
    /// What the last written bundle looks like now.
    pub mirror: crate::mirror::Mirrored,
    /// Records (`history` entries) in this home's ledger.
    pub records: usize,
}

/// The scope of a source change (see `Shell::source_changed`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    /// The ledger in the same home was replaced (adoption in place, mirror restore).
    Ledger,
    /// Another home.
    Home,
}

pub struct Shell {
    pub page: Page,
    pub tasks: Tasks,
    pub fonts: fonts::Found,
    pub font_missing: Vec<fonts::Role>,
    pub last: Option<Report>,
    /// Home reading, computed in the background: walking the disk never happens in the frame.
    pub archive: Option<ArchiveRead>,
    /// Chain reading, asked in the background: network calls never happen in the frame.
    pub chain: Option<crate::task::Done>,
    /// The last reconciliation: the core's label unchanged.
    pub reconciled: Option<(String, bool, usize)>,
    /// Who holds the pen. Held after a restore until a reconciliation reports COMPLETE.
    pub pen: Pen,
    /// Configured chain endpoints.
    pub endpoints: Vec<Endpoint>,
    /// Set when resuming a submitted anchor finds no node for its chain (the queue page says so).
    pub resume_blocked: Option<u64>,
    pub faults: Vec<Fault>,
    pub skinned: bool,
    // Identity and keys.
    /// The anchor key's address. The private key is not here; it lives in the local key vault.
    pub anchor: Option<Address>,
    /// Identities on this machine (the registry, or the existing slot). Read by the action layer, never in
    /// the frame.
    pub identities: Option<crate::identity::Registry>,
    /// Words just generated and not yet confirmed (memory only).
    pub new_words: Option<crate::identity::Fresh>,
    /// The note for a new or imported identity (written to the registry when it lands).
    pub new_label: Option<String>,
    /// The twelve words shown after the passcode check (memory only; cleared when hidden).
    pub words: Option<Vec<String>>,
    // Archive and single writer.
    pub home: Option<Home>,
    pub lock: Option<Lock>,
    pub settings: Settings,
    // Ledger, anchoring and queue.
    /// The ledger table, read in the background: walking the ledger directory never happens in the frame.
    pub rows: Option<(Vec<crate::ledgerx::Row>, usize)>,
    /// Table generation: incremented on every invalidation; a read in flight returns with the generation it
    /// started with and is dropped on mismatch.
    pub rows_gen: u64,
    pub grants_gen: u64,
    /// The key vault state now. Read once at start and changed by each passcode action; the frame only reads
    /// this field, so the window never touches the disk for it.
    pub vault: Vault,
    /// Whether the vault still holds anything to lose (any recovery seal or key slot). Read with `vault`; the
    /// frame only reads it. Whether the lock screen offers "reset the vault" and whether
    /// `keybox::reset_empty` refuses both ask it.
    pub vault_recoverable: bool,
    /// The answer after a passcode task lands: key derivation runs in the background, and where results are
    /// received the frame half continues and writes its answer here; the window and the test driver each take
    /// it (taking clears it).
    pub vault_said: Option<crate::action::Applied>,
    /// The answer of an export whose exit gate passed in the background (`action::gate_landed`), written where
    /// the gate's result is received; its own place, so a passcode task's answer and an export's never take
    /// each other's place. The window and the test driver each take it (taking clears it).
    pub gate_said: Option<crate::action::Applied>,
    /// The answers of the actions that run their slow half in the background and answer where it lands (a gas
    /// estimate, taking a content, recording files, moving the home: `action::landed`), by kind; apart from the
    /// passcode and exit-gate answers. The window and the test driver each take theirs (taking clears it).
    pub said: std::collections::BTreeMap<crate::task::Kind, crate::action::Applied>,
    /// This machine's settings (`machine.json`). Read once at start; reread after each change.
    pub machine: crate::machine::Machine,
    /// Other results received while the test driver waited for a passcode task: recorded, kept for the next receive.
    held_back: Vec<Outcome>,
    /// The last self-audit.
    pub audit: Option<AuditRead>,
    /// The ledger's mark now. Incremented when an entry is written, when a batch of anchors lands, and when
    /// the ledger's source changes. A report describes the ledger at one mark; once the mark moves, the
    /// anchor lamps describe an older ledger.
    pub book_mark: u64,
    /// The mark the last self-audit started from (success or failure). `None` when this home has not run one.
    /// It prevents spinning: with endpoints down, a failed pass is not redialed every frame; it waits for the
    /// ledger to change or a period to pass.
    pub audit_asked: Option<u64>,
    /// The ledger mark the last tail check started from (success or failure; `action::check_tail`). `None`
    /// when this home has not run one, or its nodes or basis changed since.
    pub tail_asked: Option<u64>,
    /// Whether the one task of the fetch kind in flight is a tail check (it shares the fetch kind's single
    /// flight with fetching, so the two never run together); the window names it by what it does.
    pub fetch_checks_tail: bool,
    /// The anchor queue (read from disk when the home opens, written after every change).
    pub queue: crate::queue::Queue,
    /// The last anchoring reading.
    pub sent: Option<crate::task::Done>,
    /// The registered repository's passive indicator. Computed once per page open, no resident polling.
    pub repo_since: Option<(String, Option<usize>)>,
    /// The last content hash taken (from one of the three anchoring entry points).
    pub content: Option<crate::anchorx::Content>,
    /// The current color of each of sign, record, queue.
    pub flow: crate::anchorx::Flow,
    /// The grant table, read in the background.
    pub grants: Option<Vec<crate::grantx::Row>>,
    /// The last depth reading.
    pub depth: Option<(String, zikaron::json::Value)>,
    /// The last kit written.
    pub kit: Option<crate::task::Done>,
    /// How many entries the last pick chose and which were pulled in.
    pub picked: Option<(usize, Vec<String>)>,
    /// The first-window checklist.
    pub wizard: crate::wizard::Wizard,
    /// What the double-sale guard hit this pass (listed in a dialog).
    pub clash: Vec<crate::grantx::Row>,
    /// Who writes this ledger from now on. Present means this desk handed it over by succession, and the
    /// whole app is read-only.
    pub handed: Option<String>,
    /// Per-row results of checking adoption anchors.
    pub proofs: Option<Vec<crate::adoptx::Proof>>,
    /// The table of existing anchors to adopt: (which key was asked, empty for this key; the rows).
    pub key_anchors: Option<(String, Vec<crate::adoptx::KeyAnchor>)>,
    /// Attesting for someone: the claim text as read, and each anchor's block.
    pub claim: Option<(crate::adoptx::Claim, Vec<Option<u64>>)>,
    /// Attesting for someone: the signed (attestor, signature).
    pub attested: Option<(String, String)>,
    /// The succession scan of the new key: (address, anchors found, endpoints asked).
    pub sighting: Option<(String, usize, usize)>,
    /// The last ledger of someone else read.
    pub book: Option<crate::task::Done>,
    /// Three-part story: (the grant asked, its row, the revocations citing it).
    pub story: Option<(String, Option<crate::grantx::Row>, Vec<(String, Option<String>)>)>,
    /// Whether the current identity's backup file is on disk now (read by the action layer; the frame only
    /// reads this field). It changes with the identity table.
    pub backup_seen: Option<crate::identity::BackupSeen>,
    /// The last gas estimate: (anchors in this batch, gas). Sending asks for it: a count mismatch does not
    /// release the batch (shown before sent).
    pub gas: Option<(usize, u64)>,
    /// This batch's two fee fields: computed from the chain's base fee during estimation; the confirmation
    /// card and the balance check before sending read the same values.
    pub fees: Option<zikaron_anchor::send::Fees>,
    /// The two readings above are of one chain, registry contract and set of nodes: whenever those change
    /// (`commit_settings`, a change of source) the readings are void (`gas_void`), and this count moves on.
    pub gas_epoch: u64,
    /// The count when the estimate now out was started: it lands only if nothing it came from changed since
    /// (`gas_out_of_date`), else it is set aside like a reading of an earlier source.
    pub gas_asked: Option<u64>,
    /// Backoff deadlines when the chain rate-limits: ask again after each, then move to the next endpoint.
    /// Default `chainx::SEND_BACKOFF`; tests may change it (setting zeros so they never wait on the wall
    /// clock).
    pub send_backoff: Vec<std::time::Duration>,
    /// The pauses between rounds of asking for a receipt (`zikaron_anchor::send::RECEIPT_BACKOFF` in the
    /// product; test hooks inject their own so no wall clock is waited).
    pub receipt_backoff: Vec<std::time::Duration>,
    /// How long to wait for an anchor to be included ([`crate::action::ANCHOR_WAIT`] in the product; tests inject
    /// the deadline).
    pub anchor_wait: std::time::Duration,
    /// Interface time of the last resume of a submitted anchor (the frame asks again after a while).
    pub resumed_at: f64,
    // Cached readings, restore, old data, kits and grantee checks.
    /// The `anchored` set of the last audit (disk cache, loaded when the home opens). It only gives a
    /// "checked last time" lamp and never stands in for this pass's report.
    pub remembered: Option<crate::lastread::Anchored>,
    /// The last verdict of each held grant (disk cache, loaded when the home opens). Cards speak from it
    /// until reviewed.
    pub verdicts: Vec<(String, crate::lastread::Verdict)>,
    /// The read-only mark of a restored identity (`settings/unfetched.json` of this home, loaded when it
    /// opens). While present, the ledger-writing and anchoring actions are refused.
    pub unfetched: Option<crate::restorex::State>,
    /// Fetching found this home at odds with the fetched ledger: waiting for the person's yes
    /// (`Action::FetchAside`) or no. Its rows are this home's entries that would stay in the old data.
    pub fetch_conflict: Option<Conflict>,
    /// The old data on this machine (homes set aside after a conflict), read at opening.
    pub aside: Vec<OldData>,
    /// Old data open to read, and where to come back to.
    pub old_view: Option<OldView>,
    /// The old data the last fetch left (said once after fetching, with "view").
    pub last_aside: Option<std::path::PathBuf>,
    /// Digests of export attachments: path to content-form digest or refusal, computed in the background.
    /// Kept by path across homes (a digest belongs to the file).
    pub vetted: std::collections::BTreeMap<String, Result<String, Fault>>,
    /// The kit index (machine directory `kits/index.json`); `None` when not read yet or unreadable (the
    /// trouble is recorded).
    pub kits_index: Option<Vec<crate::kitsindex::Row>>,
    /// The read-only network table (the machine directory's); `None` when not read yet or unreadable (the
    /// trouble is recorded). The paths that read it take it from disk when they run.
    pub read_nets: Option<Vec<crate::readnets::Net>>,
    /// The last reading of each read-only network (chain id, registry), from its "read the chain" key.
    pub net_reads: Vec<(u64, crate::key::Address, crate::widex::Reading)>,
    /// The root of this home's ledger (the export page lists only its kits; read with `reread_kits`, never in
    /// the frame).
    pub kits_root: Option<String>,
    /// Wall clock (seconds): both caches stamp times and judge staleness by it. The system clock in the
    /// product ([`crate::lastread::now_secs`]); tests inject fixed values.
    pub clock: fn() -> u64,
    /// The last due-diligence panels, computed in the background.
    pub diligence: Option<crate::diligx::Read>,
    /// The last record verification.
    pub verified: Option<crate::verifyx::Verified>,
    /// The last delivery check.
    pub delivery: Option<crate::deliveryx::Checked>,
    // Grant vault, sentinel and notices.
    /// Vault cards (from the last review), with that pass's chain time.
    pub cards: Option<(Vec<crate::vaultx::Card>, Option<u64>)>,
    /// Whether the home has a genesis. Read once when the home opens, set after genesis, adoption or restore;
    /// the self-audit clock runs only on a home with a genesis (without a ledger there is nothing to ask the
    /// chain).
    pub rooted: bool,
    /// Which grants the vault holds (read in the background after opening and after imports).
    pub held: Option<Vec<crate::vaultx::Held>>,
    /// Files in the vault directory with entry names that fail acceptance (named); empty when all passed.
    pub held_rejected: Vec<crate::verifyx::Rejected>,
    /// The vault changed while a listing was in flight: list again after it lands.
    pub held_dirty: bool,
    /// Alarms the sentinel raised this session (newest first).
    pub alarms: Vec<crate::sentinelx::Alarm>,
    /// Alarms just raised and not yet shown (cleared when read).
    pub rung: Vec<crate::sentinelx::Alarm>,
    /// Whether to request the system's attention this frame (the sentinel just rang). Cleared when the window
    /// reads it.
    pub attention: bool,
    /// The last badge.
    pub badge: Option<crate::badgex::Made>,
    /// The last grant check.
    pub checked: Option<crate::checkx::Checked>,
    /// The last publication check: which address and the reading.
    pub published: Option<(String, crate::fetchx::Published)>,
    /// Notices raised this session; raised keys are stored in settings `alarmed` (the same record as the
    /// sentinel's).
    pub notices: Vec<crate::watchx::Notice>,
    /// Notices just raised and not yet shown (cleared when read).
    pub fresh: Vec<crate::watchx::Notice>,
    /// Status line: the latest sentence for it (network errors go here, not to notices).
    pub status: Option<(crate::lang::Key, String)>,
    /// The last failure of each kind of background task (cleared on success). A page reads its own kind's
    /// failure instead of guessing from the shared fault table.
    pub failed: std::collections::BTreeMap<crate::task::Kind, Fault>,
    /// When a real chain answer last arrived (interface clock). Recorded only when a pass actually read a
    /// block header, fragment or balance; a landed task that did not reach the chain does not count.
    pub chain_read_at: Option<f64>,
    /// Which adoption text `proofs` checked.
    pub proofs_rows: Option<String>,
    /// Chain time read back on a chain round trip (see `note_chain_time`).
    pub chain_time: Option<u64>,
    /// The primary identity the first unlock after upgrading settled (said once on screen, then taken).
    pub primary_settled: Option<String>,
    /// The primary identity and its kind (the vault header, readable while locked; read with `vault`, never in
    /// the frame). The lock screen offers words or a key file by it; the identity menus mark it.
    pub primary: Option<(String, crate::keybox::PrimaryKind)>,
    /// A master key change is in flight (making another identity primary, restoring from a backup): the screen
    /// says it is resealing and every other action waits (`action::apply`).
    pub rekeying: bool,
    /// A fetch and replace is swapping this home for a fresh one (`Action::FetchAside`), or a move is copying it
    /// (`Action::MigrateHome`): the home is frozen (`action::held_back`) until that task lands.
    pub swapping: bool,
    /// A lock asked for while a task that writes local data was running: the gate is up and the shell reads as
    /// locked; the master key is wiped and the home closed when those tasks have landed (`drain_at`).
    pub lock_pending: bool,
    /// The last whole-machine backup written this session (path and what it held).
    pub backup_made: Option<(String, crate::backup::Summary)>,
    /// The last backup opened with its password (the confirmation card reads it).
    pub backup_peek: Option<crate::backup::Summary>,
    /// Ledger entries and held grants on this machine now (measured in the background; the "after the last
    /// backup" count reads it).
    pub items_now: Option<u64>,
}

impl Shell {
    /// Start the shell. The clothes (skin and fonts) are put on by the control library's `skin::dress`; this
    /// takes its reading, opens the channel, drops the first trace mark and prepares the background tasks. The
    /// window and the test driver start the same way.
    pub fn boot(dressed: zikaron_ui::skin::Dressed) -> Shell {
        let found = dressed.found;
        let missing = dressed.missing;

        // Open the channel before any trace mark is dropped.
        trace::open();
        let mut faults: Vec<Fault> = Vec::new();
        if let Some(f) = trace::trouble() {
            faults.push(f);
        }
        // Unreadable machine settings fall back to the default lock (fifteen minutes), with the refusal on
        // screen; never silently "never lock".
        let machine = match crate::machine::read() {
            Ok(m) => m,
            Err(f) => {
                faults.push(f);
                crate::machine::Machine::default()
            }
        };
        trace::mark(Feature::H0);
        for r in &missing {
            faults.push(Fault::known(Known::FontMissing, format!("role {}", r.as_str())));
        }
        trace::mark(Feature::H1);

        Shell {
            page: Page::FirstRun,
            resume_blocked: None,
            tasks: Tasks::new(),
            fonts: found,
            font_missing: missing,
            last: None,
            archive: None,
            chain: None,
            reconciled: None,
            pen: Pen::Granted,
            endpoints: Vec::new(),
            faults,
            skinned: true,
            anchor: None,
            identities: None,
            new_words: None,
            new_label: None,
            words: None,
            home: None,
            lock: None,
            settings: Settings::default(),
            rows: None,
            rows_gen: 0,
            grants_gen: 0,
            vault: Vault::of(crate::keybox::state()),
            vault_recoverable: crate::keybox::recoverable().unwrap_or(true),
            vault_said: None,
            said: std::collections::BTreeMap::new(),
            gate_said: None,
            machine,
            held_back: Vec::new(),
            audit: None,
            book_mark: 0,
            audit_asked: None,
            tail_asked: None,
            fetch_checks_tail: false,
            queue: crate::queue::Queue::default(),
            sent: None,
            repo_since: None,
            content: None,
            flow: crate::anchorx::Flow::default(),
            gas: None,
            send_backoff: crate::chainx::SEND_BACKOFF.to_vec(),
            receipt_backoff: zikaron_anchor::send::RECEIPT_BACKOFF.to_vec(),
            anchor_wait: crate::action::ANCHOR_WAIT,
            fees: None,
            gas_epoch: 0,
            gas_asked: None,
            resumed_at: 0.0,
            grants: None,
            depth: None,
            kit: None,
            picked: None,
            wizard: crate::wizard::Wizard::default(),
            clash: Vec::new(),
            handed: None,
            proofs: None,
            key_anchors: None,
            claim: None,
            attested: None,
            sighting: None,
            book: None,
            story: None,
            backup_seen: None,
            remembered: None,
            verdicts: Vec::new(),
            clock: crate::lastread::now_secs,
            kits_index: None,
            read_nets: None,
            net_reads: Vec::new(),
            kits_root: None,
            unfetched: None,
            fetch_conflict: None,
            aside: Vec::new(),
            old_view: None,
            last_aside: None,
            vetted: std::collections::BTreeMap::new(),
            diligence: None,
            verified: None,
            delivery: None,
            cards: None,
            held_rejected: Vec::new(),
            held_dirty: false,
            rooted: false,
            held: None,
            alarms: Vec::new(),
            rung: Vec::new(),
            attention: false,
            badge: None,
            checked: None,
            published: None,
            notices: Vec::new(),
            fresh: Vec::new(),
            status: None,
            failed: std::collections::BTreeMap::new(),
            chain_read_at: None,
            proofs_rows: None,
            chain_time: None,
            primary_settled: None,
            primary: primary_reading(),
            rekeying: false,
            swapping: false,
            lock_pending: false,
            backup_made: None,
            backup_peek: None,
            items_now: None,
        }
    }

    /// The block time a held grant was anchored at in its issuer's ledger: this run's re-check first, else the
    /// cached verdict of the last one (so a date range still holds after a restart). `None` while neither has
    /// read it. The one reading the vault's date range filters by.
    pub fn held_anchored_at(&self, id: &str) -> Option<u64> {
        // A card of this run that has no time yet does not hide the cached one.
        let card = self.cards.as_ref().and_then(|(cs, _)| cs.iter().find(|c| c.id.eq_ignore_ascii_case(id)).and_then(|c| c.anchored_at));
        card.or_else(|| self.verdicts.iter().find(|(g, _)| g.eq_ignore_ascii_case(&crate::lastread::grant_form(id))).and_then(|(_, v)| v.anchored_at))
    }

    /// Whether the anchor key is in the vault, recording its address for the screen. Asked now, not
    /// remembered.
    pub fn refresh_anchor(&mut self) -> Result<bool, Fault> {
        // A closed vault reads as "no key available now", not an error. This sits on every path into a home
        // (open, switch seat, switch identity); as an error, those paths would fail after writing the
        // registry and settings, and the disk would change while the screen said it failed. While locked the
        // screen says "no signing key yet", which is true.
        if !self.unlocked() {
            self.anchor = None;
            return Ok(false);
        }
        match crate::key::load(crate::register::account_now()?.as_deref())? {
            Some(s) => {
                self.anchor = s.address();
                Ok(true)
            }
            None => {
                self.anchor = None;
                Ok(false)
            }
        }
    }

    /// After the vault opens: this seat's key is available; the address and identity table are reread, with
    /// failures in the trouble panel (the unlock itself still counts as done).
    pub fn after_unlock(&mut self) {
        // Unlocked before a pending lock completed: the lock is off (the key never left).
        self.lock_pending = false;
        self.reread_vault();
        // What the passes right after opening found (`local::after_open`, run in the unlock's own task).
        if let Some(o) = crate::local::take_opened() {
            self.faults.extend(o.troubles);
            if o.primary.is_some() {
                self.primary_settled = o.primary;
            }
        }
        // Local data opens only now: with the home closed (it is closed while locked), land on the seat and
        // open its home the same way the window starts.
        let opened_now = self.home.is_none() && self.unlocked();
        if opened_now {
            let _ = crate::action::boot_home(self);
            if let Some(l) = self.settings.lang {
                crate::lang::set(l);
            }
            crate::when::set(self.settings.zone.unwrap_or(crate::when::Zone::Utc));
        }
        if let Err(f) = self.refresh_anchor() {
            self.faults.push(f);
        }
        match crate::register::view(self.settings.role) {
            Ok(v) => self.seat_identities(Some(v)),
            Err(f) => self.faults.push(f),
        }
        // The background work stopped while locked: done once now (`Action::CatchUp`).
        if opened_now {
            let _ = crate::action::apply(self, crate::action::Action::CatchUp);
        }
    }

    /// After the vault locks: the key is unavailable, so key-related fields are cleared (address, shown
    /// words, new words not yet built). Key-using actions are refused first by the action layer's table; this
    /// only clears readings.
    /// The first half of a lock asked for while local data is being written (`Action::Lock`): on screen and in
    /// every gate the vault reads locked, key-related readings are cleared; the home stays open for the tasks
    /// under way to land into.
    pub fn begin_lock(&mut self) {
        self.lock_pending = true;
        // The vault itself answers locked from here (one source: every reread sees it); the key stays in
        // memory until the writes in flight land.
        crate::keybox::begin_lock();
        self.reread_vault();
        self.anchor = None;
        self.words = None;
        self.new_words = None;
    }

    pub fn after_lock(&mut self) {
        self.reread_vault();
        self.anchor = None;
        self.words = None;
        self.new_words = None;
        self.unload();
    }

    /// Locked means no local data in hand: the home closes (its writer lock is released) and every copy read
    /// from local data is dropped; unlocking opens the home again and reads it anew (`after_unlock`).
    pub fn unload(&mut self) {
        self.close_home();
        self.seat_identities(None);
        self.settings = Settings { role: self.settings.role, ..Settings::default() };
        self.endpoints = Vec::new();
        self.wizard = crate::wizard::Wizard::default();
        self.kits_index = None;
        self.kits_root = None;
        self.vetted.clear();
        self.content = None;
        self.book = None;
        self.story = None;
        self.diligence = None;
        self.verified = None;
        self.delivery = None;
        self.checked = None;
        self.depth = None;
        self.kit = None;
        self.picked = None;
        self.clash.clear();
        self.proofs = None;
        self.proofs_rows = None;
        self.key_anchors = None;
        self.claim = None;
        self.attested = None;
        self.sighting = None;
        self.badge = None;
        self.published = None;
        self.new_label = None;
        self.remembered = None;
        self.verdicts.clear();
        self.unfetched = None;
        self.cards = None;
        self.held = None;
        self.held_rejected.clear();
        self.grants = None;
        self.rows = None;
        self.audit = None;
        self.reconciled = None;
        self.handed = None;
    }

    /// Reread the vault state (after passcode actions and after opening a home; never in the frame).
    pub fn reread_vault(&mut self) {
        let v = Vault::of(crate::keybox::state());
        if let Vault::Damaged(f) = &v {
            self.faults.push(f.clone());
        }
        self.vault = v;
        // Unreadable counts as "still holds something": the delete key is withheld whenever the reading is
        // uncertain.
        self.vault_recoverable = crate::keybox::recoverable().unwrap_or(true);
        self.primary = primary_reading();
    }

    /// Place the identity table together with its disk reading.
    ///
    /// Whether the current identity's backup file is on disk is a disk read, so it changes in the same step
    /// as the table: every placement goes through here and the frame only reads the shell field.
    ///
    /// Taking `Option` includes "the registry cannot be read": both fields become empty, and nothing else in
    /// the shell writes `identities`.
    pub fn seat_identities(&mut self, reg: Option<crate::identity::Registry>) {
        self.backup_seen = reg
            .as_ref()
            .and_then(|r| r.now())
            .map(|(row, _)| crate::identity::backup_seen(row));
        self.identities = reg;
    }

    /// Whether this seat is empty now (an existing key holds only one seat): the current identity has no
    /// address on this seat.
    ///
    /// One decision: the identity card's sentence and three keys, "domains this seat can sign", and the data
    /// card's home and key buttons all ask it. Without an identity it answers `false` (that state has its own
    /// words).
    pub fn seat_unseated(&self) -> bool {
        self.identities
            .as_ref()
            .and_then(|r| r.now())
            .map(|(row, _)| row.address(self.settings.role).is_none())
            .unwrap_or(false)
    }

    /// This seat has no home. An empty seat has no home, and the shell reflects it: the home and its lock are
    /// released, readings that follow the home are invalidated (`source_changed`), queue and ledger head
    /// cleared, so every page speaks from "no home". Otherwise the other seat's home and its ledger would
    /// show as this seat's.
    ///
    /// Releasing the lock releases the writer role of that home; returning to the held seat, `open_home_at`
    /// takes the lock again as writer.
    pub fn close_home(&mut self) {
        self.source_changed(Source::Home);
        self.home = None;
        self.lock = None;
        self.queue = crate::queue::Queue::default();
        self.rooted = false;
    }

    /// Whether the vault is open. The lock screen and the action layer's table both ask it.
    /// The language to speak now: the home's own choice once it can be read, otherwise the last one chosen on
    /// this machine (the passcode gate, before unlocking); `None` means neither was ever chosen.
    pub fn speaks(&self) -> Option<crate::lang::Lang> {
        self.settings.lang.or(self.machine.lang)
    }

    pub fn unlocked(&self) -> bool {
        self.vault.keys_ready()
    }

    /// Whether settings can be saved now. Saving asks it, and so does anything that needs to know it can be
    /// recorded before doing the work (see `action::export_mirror`), so a mirror is never written to disk
    /// only to fail at recording it.
    pub fn may_save_settings(&self) -> Result<(), Fault> {
        if self.home.is_none() {
            return Err(Fault::known(Known::NoHome, crate::lang::t(crate::lang::Key::Tail211).to_string()));
        }
        if !self.writable() {
            return Err(Fault::known(
                Known::ReadOnly,
                match self.lock.as_ref() {
                    Some(l) => crate::lang::filln(crate::lang::Key::Tail006, &[&(l.holder()).to_string()]),
                    None => crate::lang::t(crate::lang::Key::Tail212).to_string(),
                },
            ));
        }
        Ok(())
    }

    /// Save settings. Without an open home, say so by name; never drop silently.
    pub fn save_settings(&self) -> Result<(), Fault> {
        self.may_save_settings()?;
        let h = self.home.as_ref().expect("上一句已经问过家在不在");
        self.settings.write(h)
    }

    /// Change settings: write to disk first, and only a successful write counts. The change applies to a copy
    /// that replaces the shell's only after writing; on failure the shell is unchanged. Changing memory first
    /// would show a new seat or node that disappears after restart when the write failed (a read-only second
    /// instance, a full disk).
    pub fn commit_settings(&mut self, f: impl FnOnce(&mut Settings)) -> Result<(), Fault> {
        self.may_save_settings()?;
        let mut next = self.settings.clone();
        f(&mut next);
        let h = self.home.as_ref().expect("上一句已经问过家在不在");
        next.write(h)?;
        // A gas estimate and its fees are readings of one chain, registry contract and set of nodes: saved
        // settings that change any of them leave those readings speaking of another place, so they go,
        // whichever key saved (nodes, the chain cells, a network chosen or cleared).
        let moved = gas_source(&self.settings) != gas_source(&next);
        self.settings = next;
        if moved {
            self.gas_void();
        }
        Ok(())
    }

    /// The gas estimate and its fees are void: cleared, and an estimate still out lands as out of date.
    pub fn gas_void(&mut self) {
        self.gas = None;
        self.fees = None;
        self.gas_epoch += 1;
    }

    /// Whether a gas estimate landing now was started before what it came from changed.
    pub fn gas_out_of_date(&self) -> bool {
        self.gas_asked != Some(self.gas_epoch)
    }

    /// Whether this ledger's chain broke. Reads the label the last reconciliation returned (from the core),
    /// never comparing prev itself. Before any reconciliation, "unknown" does not lock.
    ///
    /// The label has three sources (offline reconciliation, the self-audit clock, the self-audit after
    /// adoption) and all land in the same field (`reconciled`), so the lock does not depend on the path.
    pub fn broken(&self) -> bool {
        self.reconciled
            .as_ref()
            .map(|(l, _, _)| l == zikaron::tokens::Label::BrokenChain.as_str())
            .unwrap_or(false)
    }

    /// What the bar at the top of each page says. Closed: the window only turns it into a sentence and a
    /// color, and tests read which bar is up.
    pub fn banner(&self) -> Banner {
        if let Some(to) = self.handed.as_ref() {
            return Banner::Handed(to.clone());
        }
        if self.broken() {
            return Banner::Broken;
        }
        if self.home.is_some() && !self.writable() {
            return Banner::ReadOnly(match self.lock.as_ref() {
                Some(l) => l.holder().to_string(),
                None => String::new(),
            });
        }
        Banner::None
    }

    /// The ledger moved one step. Writing an entry, a batch of anchors landing, and a source change all come
    /// here: once the mark changes, the last report describes an older ledger and `audit_stale` becomes true.
    pub fn book_changed(&mut self) {
        self.book_mark = self.book_mark.wrapping_add(1);
    }

    /// Whether a self-audit can start: home open, a root, basis and endpoints configured, none of its kind in
    /// flight. The periodic and stale questions both build on it.
    fn audit_possible(&self) -> bool {
        if self.home.is_none() || !self.rooted {
            return false;
        }
        if self.settings.chain_id.is_none() || self.settings.registry.is_none() || self.endpoints.is_empty() {
            return false;
        }
        !self.tasks.in_flight(crate::task::Kind::Audit)
    }

    /// Whether this home's report is stale.
    ///
    /// Two cases: this home has not run one yet (after a key or seat change, restore, adoption or reopening),
    /// or it has and the ledger changed since (entries written, anchors landed). Neither waits for the
    /// period; the next frame audits.
    ///
    /// The periodic question is unchanged (`audit_due`): the period governs repeated review, this governs
    /// whether the report at hand is stale. The started mark records success or failure (`audit_asked`), so
    /// with endpoints down this does not redial every frame.
    pub fn audit_stale(&self) -> bool {
        // Even with a period of zero: "does not run by itself" is about repetition, and the anchor lamps come
        // from the report, so a report about an older ledger would make them lie.
        self.audit_possible() && self.audit_asked != Some(self.book_mark)
    }

    /// [`Shell::tail_due`], and when it is due the asking is recorded at once for this ledger state, before
    /// anything is asked: whatever answers (a refusal before the check starts included) does not make it due
    /// again on the next frame; a ledger that moves, new nodes or basis, or another home do.
    pub fn take_tail_due(&mut self) -> bool {
        let due = self.tail_due();
        if due {
            self.tail_asked = Some(self.book_mark);
        }
        due
    }

    /// Whether this identity's tail is due to be checked against the chain (`Action::CheckTail`): the open
    /// home holds the not-fetched mark (either form), basis and nodes are set, no fetch is in flight, and this
    /// ledger as it is now has not been checked since its nodes or basis last changed (`tail_asked`). A pure
    /// decision without disk or network; the window's clock and the places that make it due ask it.
    pub fn tail_due(&self) -> bool {
        self.home.is_some()
            && self.unfetched.is_some()
            && self.settings.chain_id.is_some()
            && self.settings.registry.is_some()
            && !self.endpoints.is_empty()
            && !self.tasks.in_flight(crate::task::Kind::Fetch)
            && self.tail_asked != Some(self.book_mark)
    }

    /// Whether the self-audit clock is due. A pure decision without disk or network: a zero period never
    /// runs; missing basis or endpoints do not start; in flight does not start; less than a period since the
    /// last does not start.
    pub fn audit_due(&self, now: f64, last_tick: f64) -> bool {
        let every = self.settings.audit_every;
        if every == 0 || self.home.is_none() {
            return false;
        }
        // A home without a genesis does not run the clock. Grantee homes often have no ledger; asking the
        // chain would only put NO_GENESIS in the trouble panel. Whether the home has a genesis is read when
        // it opens and set at genesis (`rooted`).
        if !self.rooted {
            return false;
        }
        if self.settings.chain_id.is_none()
            || self.settings.registry.is_none()
            || self.endpoints.is_empty()
        {
            return false;
        }
        if self.tasks.in_flight(crate::task::Kind::Audit) {
            return false;
        }
        // When the last pass started is recorded by `Tasks`, success or failure. Reading `self.audit.at`,
        // written only on success, would let failed passes leave the clock unmoved and redial every frame.
        let last = self
            .tasks
            .landed_at(crate::task::Kind::Audit)
            .unwrap_or(last_tick);
        now - last >= every as f64
    }

    /// Whether this instance may write: the lock, plus the broken-chain gate.
    ///
    /// After a broken chain the whole app is read-only (appending would deepen the damage). This one
    /// predicate carries it, and every write (settings, entries, moving, bundles) already asks it.
    pub fn writable(&self) -> bool {
        if self.broken() || self.handed.is_some() {
            return false;
        }
        self.lock.as_ref().map(|l| l.mode().writable()).unwrap_or(false)
    }

    /// Whether this ledger can take new entries now: both the lock and the pen are needed.
    pub fn may_write_entries(&self) -> Result<(), Fault> {
        // Two different refusals: "no home open" and "another writer has this home" are different things;
        // sharing `READ_ONLY` would point to a second instance that does not exist. Succession first: once
        // handed over, the writing side should hear "it belongs to the new key" (law §7.3: the new key writes
        // this ledger from then on).
        if let Some(to) = self.handed.as_ref() {
            return Err(Fault::known(Known::HandedOver, to.clone()));
        }
        // Broken chain before the lock: after a break the lock is still held, and the person should see that
        // the ledger is broken, not who holds it.
        if self.broken() {
            return Err(Fault::known(
                Known::Broken,
                match self.audit.as_ref() {
                    Some(a) => crate::lang::filln(crate::lang::Key::Tail213, &[&(a.label).to_string()]),
                    None => String::new(),
                },
            ));
        }
        let Some(l) = self.lock.as_ref() else {
            return Err(Fault::known(Known::NoHome, crate::lang::t(crate::lang::Key::Tail005).to_string()));
        };
        if !l.mode().writable() {
            return Err(Fault::known(
                Known::ReadOnly,
                crate::lang::filln(crate::lang::Key::Tail006, &[&(l.holder()).to_string()]),
            ));
        }
        if !self.pen.writable() {
            return Err(Fault::known(
                Known::PenHeld,
                match &self.reconciled {
                    Some((label, _, _)) => crate::lang::filln(crate::lang::Key::Tail214, &[&(label).to_string()]),
                    None => crate::lang::t(crate::lang::Key::Tail215).to_string(),
                },
            ));
        }
        Ok(())
    }

    /// The only way the shell's copies of disk state are loaded. Opening a home comes here; nothing else
    /// reads them.
    ///
    /// Endpoints, the anchor queue and the first-window checklist live on disk with a copy in the shell for
    /// the frame. Loading happens only here, and a malformed file is named at once instead of being read as
    /// empty.
    pub fn hydrate(&mut self, home: &crate::home::Home) -> Result<(), Fault> {
        // Unreadable settings mean the home cannot open: endpoints, basis and registrations live there, and
        // going on with empty ones would make the screen say something else entirely.
        self.settings = Settings::read(home)?;
        self.endpoints = self
            .settings
            .endpoints
            .iter()
            .filter_map(|spec| Endpoint::parse(spec))
            .collect();
        // The two local bookkeeping files do not block opening when malformed, but they are visible. Reading
        // them as empty would silently lose queued entries; refusing to open would block the broken-chain
        // recovery path, which needs the home open. So a named trouble goes to the screen and an empty one is
        // used.
        match crate::queue::Queue::read(home) {
            Ok(q) => self.queue = q,
            Err(f) => {
                self.queue = crate::queue::Queue::default();
                self.faults.push(f);
            }
        }
        match crate::wizard::Wizard::read(home) {
            Ok(w) => self.wizard = w,
            Err(f) => {
                self.wizard = crate::wizard::Wizard::default();
                self.faults.push(f);
            }
        }
        // Whether the home has a genesis: one read of the ledger head (opening already walks the disk).
        self.rooted = crate::ledgerx::head(home).map(|h| h.is_some()).unwrap_or(false);
        // Load what the last pass knew first: the audit set and the grant verdicts speak at start and on
        // return while the background audits again (`audit_stale` stays true for this home). A malformed
        // cache is named and does not block opening.
        match crate::lastread::load_anchored(home) {
            // Keep only rows anchored on this home's current chain (old rows from another network do not
            // claim "checked last time").
            Ok(a) => {
                let chain = self.settings.chain_id;
                self.remembered = a.map(|mut x| {
                    x.rows.retain(|(_, (c, _))| Some(*c) == chain);
                    x
                })
            }
            Err(f) => {
                self.remembered = None;
                self.faults.push(f);
            }
        }
        let (verdicts, bad) = crate::lastread::load_verdicts(home);
        self.verdicts = verdicts;
        self.faults.extend(bad);
        self.reread_kits();
        // The read-only mark of a restored identity. Unreadable counts as "not fetched yet" and still refuses
        // writes (never as absent); the refusal goes to the trouble panel.
        self.unfetched = match crate::restorex::read(home) {
            Ok(s) => s,
            Err(f) => {
                self.faults.push(f);
                Some(crate::restorex::State::Unfetched)
            }
        };
        self.reread_aside();
        Ok(())
    }

    /// The old data on this machine: each home set aside after a conflict, when, how many entries it holds and
    /// how many of them were never anchored (its queue). Read where the home is loaded, never in the frame.
    pub fn reread_aside(&mut self) {
        let room = crate::home::machine_dir().map(|m| m.join(crate::local::ASIDE_HOMES)).ok();
        let machine = crate::machine::read().unwrap_or_default();
        let homes = machine.homes.clone();
        self.aside = homes
            .iter()
            .map(std::path::PathBuf::from)
            .filter(|p| room.as_ref().map(|r| p.starts_with(r)).unwrap_or(false) && p.is_dir())
            .map(|p| {
                let h = crate::home::Home::bare(p.clone());
                let at = machine.aside_at.iter().find(|(x, _)| std::path::Path::new(x) == p).map(|(_, at)| *at).unwrap_or(0);
                let entries = h.ledger().and_then(|l| l.survey()).map(|s| s.items.len()).unwrap_or(0);
                let queued = crate::queue::Queue::read(&h).map(|q| q.items.len()).unwrap_or(0);
                OldData { path: p, at, entries, queued }
            })
            .collect();
    }

    /// Reread the kit index (the machine directory's). Unreadable goes to the trouble panel by name and the
    /// screen says it was not read.
    pub fn reread_kits(&mut self) {
        self.kits_root = self.home.as_ref().and_then(|h| crate::ledgerx::root_of(h).ok());
        match crate::home::machine_dir().and_then(|m| crate::kitsindex::read(&m)) {
            Ok(rows) => self.kits_index = Some(rows.unwrap_or_default()),
            Err(f) => {
                self.kits_index = None;
                self.faults.push(f);
            }
        }
        self.reread_nets();
    }

    /// Reread the read-only network table (the machine directory's). Unreadable goes to the trouble panel by
    /// name.
    pub fn reread_nets(&mut self) {
        match crate::action::read_nets_now() {
            Ok(n) => self.read_nets = Some(n),
            Err(f) => {
                self.read_nets = None;
                self.faults.push(f);
            }
        }
    }

    /// Whether the periodic review should start (vault card checks follow the basis): a zero period does not;
    /// missing endpoints or basis do not; in flight does not; less than a period since the last does not. A
    /// pure decision, no disk.
    pub fn review_due(&self, now: f64, last_tick: f64) -> bool {
        let every = self.settings.review_every;
        if every == 0 || self.home.is_none() || self.endpoints.is_empty() {
            return false;
        }
        if self.settings.chain_id.is_none() || self.settings.registry.is_none() {
            return false;
        }
        if self.tasks.in_flight(crate::task::Kind::Review) {
            return false;
        }
        let last = self.tasks.landed_at(crate::task::Kind::Review).unwrap_or(last_tick);
        now - last >= every as f64
    }

    /// This table is stale. Clearing the reading and allowing a new read are the same action: `Tasks` records
    /// "started" (it is not reset by failure, which is how it prevents spinning), so clearing only the
    /// reading would never trigger another read. Every place that sets `rows` to `None` goes through here
    /// (`grants` likewise).
    pub fn stale_rows(&mut self) {
        self.rows = None;
        self.rows_gen += 1;
        self.tasks.forget(crate::task::Kind::Ledger);
    }

    pub fn stale_grants(&mut self) {
        self.grants = None;
        self.grants_gen += 1;
        self.tasks.forget(crate::task::Kind::Grants);
    }

    /// The ledger's source changed. Two scopes: `Source::Ledger`, the ledger in the same home was replaced
    /// (adoption in place, mirror restore), invalidating everything read from it; `Source::Home`, another
    /// home, also invalidating settings, vault, chain readings, pen and alarms. Background tasks that follow
    /// the source move to a new epoch, and results of the old epoch deliver only their trouble
    /// (`Tasks::new_epoch`).
    ///
    /// The destructuring has no `..`: each new shell field must be assigned here to a scope. The pen and
    /// alarms clear only on a home change: a failed adoption or restore in the same home keeps the
    /// broken-chain bar and read-only state.
    pub fn source_changed(&mut self, scope: Source) {
        self.tasks.new_epoch();
        let home = scope == Source::Home;
        let Shell {
            // Following the machine, the person and this session, not the ledger's source:
            page: _,
            // Names the fetch kind's flight, which a source change does not stop (its result is set aside).
            fetch_checks_tail: _,
            resume_blocked: _,
            tasks: _,
            fonts: _,
            font_missing: _,
            last: _,
            endpoints: _,
            faults: _,
            skinned: _,
            anchor: _,
            identities: _,
            // The backup reading is read from `identities`: it changes with the identity table
            // (`seat_identities`) and not with the ledger.
            backup_seen: _,
            new_words: _,
            new_label: _,
            words: _,
            book_mark: _,
            send_backoff: _,
            receipt_backoff: _,
            anchor_wait: _,
            remembered,
            verdicts,
            clock: _,
            kits_index: _,
            read_nets: _,
            net_reads: _,
            kits_root: _,
            unfetched,
            vetted: _,
            vault: _,
            vault_recoverable: _,
            primary_settled: _,
            primary: _,
            rekeying: _,
            swapping: _,
            lock_pending: _,
            backup_made: _,
            backup_peek: _,
            items_now: _,
            // The passcode answer and machine settings follow the machine; results held while the test driver waits
            // follow the session.
            vault_said: _,
            gate_said: _,
            // Those kinds follow the source: one started for the earlier source lands stale and writes nothing here.
            said: _,
            machine: _,
            held_back: _,
            home: _,
            lock: _,
            content: _,
            flow: _,
            book: _,
            diligence: _,
            verified: _,
            delivery,
            checked: _,
            attention: _,
            // Reread from the new source by `hydrate` (opening a home); adoption and restore do not replace
            // the settings file:
            settings: _,
            queue: _,
            wizard: _,
            rooted: _,
            // Read from the ledger (cleared in both scopes):
            rows,
            rows_gen,
            grants_gen,
            audit,
            audit_asked,
            tail_asked,
            sent,
            repo_since,
            grants,
            depth,
            kit,
            picked,
            clash,
            proofs,
            proofs_rows,
            key_anchors,
            claim,
            attested,
            sighting,
            story,
            gas,
            fees,
            gas_epoch,
            gas_asked: _,
            resumed_at: _,
            failed,
            archive,
            // Following this home (cleared only on a home change):
            chain,
            reconciled,
            pen,
            handed,
            cards,
            held,
            held_rejected,
            held_dirty,
            alarms,
            rung,
            badge,
            published,
            notices,
            fresh,
            status,
            chain_read_at,
            chain_time,
            // The old data on this machine and the one being read follow the machine, not the home.
            aside: _,
            // Opening old data and coming back set this after their change of home (`view_old`, `leave_old`);
            // any other change of home leaves it.
            old_view,
            // What fetching left belongs to that home.
            fetch_conflict,
            last_aside,
        } = self;
        *fetch_conflict = None;
        *last_aside = None;
        *old_view = None;
        *rows = None;
        *rows_gen += 1;
        *grants_gen += 1;
        *audit = None;
        // A source change means this home has no report yet: the next frame audits (without waiting for the
        // period).
        *audit_asked = None;
        *tail_asked = None;
        *sent = None;
        *repo_since = None;
        *grants = None;
        *depth = None;
        *kit = None;
        *picked = None;
        clash.clear();
        *proofs = None;
        *proofs_rows = None;
        *key_anchors = None;
        *claim = None;
        *attested = None;
        *sighting = None;
        *story = None;
        *gas = None;
        *fees = None;
        *gas_epoch += 1;
        failed.clear();
        // The usage reading includes the entry count: invalidated when the ledger changes; the caller
        // measures again.
        *archive = None;
        if !home {
            return;
        }
        *chain = None;
        *reconciled = None;
        *pen = Pen::Granted;
        *handed = None;
        // The two caches belong to that home: put away on a home change, and `hydrate` reads the new home's.
        *remembered = None;
        verdicts.clear();
        *unfetched = None;
        // The last delivery conclusion belongs to that home's grant: put away on a home change.
        *delivery = None;
        *cards = None;
        *held = None;
        held_rejected.clear();
        *held_dirty = false;
        alarms.clear();
        rung.clear();
        *badge = None;
        // The publication check was against a kit of this home: invalidated when the source changes.
        *published = None;
        notices.clear();
        fresh.clear();
        *status = None;
        *chain_read_at = None;
        *chain_time = None;
    }

    /// Record a trouble and hand it back to the screen. There is no silent branch.
    pub fn trouble(&mut self, f: Fault) -> crate::action::Applied {
        self.faults.push(f.clone());
        crate::action::Applied::Trouble(f)
    }

    /// Receive results and record them.
    /// A fetched ledger landed in the home at `root` (fetching, or fetching after setting a home aside): the
    /// mark follows the tail check, on the home the fetch started in (even if another is open now).
    fn fetch_landed(&mut self, root: &std::path::Path, tail: &crate::restorex::Tail) {
        let r = crate::home::Home::open(root).and_then(|h| match tail {
            crate::restorex::Tail::Pass { .. } => crate::restorex::clear(&h).map(|_| None),
            crate::restorex::Tail::NewerElsewhere { missing, .. } => {
                let s = crate::restorex::State::NewerElsewhere { missing: *missing };
                crate::restorex::write(&h, s).map(|_| Some(s))
            }
        });
        let here = self.home.as_ref().map(|h| crate::home::same_place(h.root(), root)).unwrap_or(false);
        match r {
            Ok(s) if here => {
                self.unfetched = s;
                // The ledger grew (from empty): record the root and one ledger step (the self-audit follows
                // the new ledger); the table is invalidated.
                if let Some(h) = self.home.as_ref() {
                    self.rooted = crate::ledgerx::head(h).map(|x| x.is_some()).unwrap_or(false);
                }
                self.book_changed();
                // The answer that just landed is about this ledger as it now is: the tail is not due again until
                // the ledger, its nodes or basis, or the home change (an answer of "newer entries elsewhere" is
                // not asked again every frame).
                self.tail_asked = Some(self.book_mark);
                // With a root, the export page lists kits of this ledger.
                self.reread_kits();
            }
            Ok(_) => {}
            Err(f) => self.faults.push(f),
        }
        self.stale_rows();
    }

    /// An exit refused because the chain holds anchors this home lacks: the gate left this home's read-only
    /// mark on disk (`exitgate::pass`); the shell takes it now, so writing waits for fetching at once.
    pub fn gate_refused(&mut self, f: &crate::fault::Fault) {
        if f.which() == Some(crate::fault::Known::NewerElsewhere) {
            if let Some(home) = self.home.as_ref() {
                if let Ok(s) = crate::restorex::read(home) {
                    self.unfetched = s;
                }
            }
        }
    }

    pub fn drain(&mut self) -> Vec<Outcome> {
        self.drain_at(0.0)
    }

    /// As above, with the arrival time given by the caller (the interface clock). The self-audit clock
    /// schedules by it, and this layer does not ask the system time.
    pub fn drain_at(&mut self, now: f64) -> Vec<Outcome> {
        // The trace file reached its cap: say so once when writing stops.
        if let Some(f) = trace::take_full() {
            self.faults.push(f);
        }
        let got = self.tasks.drain_at(now);
        for o in &got {
            // Passcode tasks: the frame half continues here and its answer goes to `vault_said`; its refusal
            // is recorded there (`Shell::trouble`), not again below.
            if o.kind == crate::task::Kind::Vault {
                let got = o.result.clone().map(|d| match d {
                    Done::Vault(v) => v,
                    _ => crate::task::Vault::Opened,
                });
                let said = crate::action::vault_landed(self, got);
                self.vault_said = Some(said);
                continue;
            }
            // An export's exit gate passed: the export runs now and its answer goes to `gate_said` (its own
            // place, apart from a passcode task's). A refused gate takes the common path below (recorded, and the home's read-only
            // mark taken). A pass holds only for the home and the source it started on (`o.stale` says whether the
            // source moved since): `gate_landed` judges it before anything runs.
            if let (crate::task::Kind::Gate, Ok(Done::GatePassed { root, then, pass })) = (o.kind, &o.result) {
                if let Some(said) = crate::action::gate_landed(self, root.clone(), (**then).clone(), pass, o.stale) {
                    self.gate_said = Some(said);
                }
                continue;
            }
            // The actions whose slow half ran in the background: their frame half runs here (`action::landed`)
            // and the answer goes to `said`, not through the common path below. One started for an earlier source
            // lands nothing: it was for that source. A move ends the home's freeze whatever it came to.
            if crate::action::lands_said(o.kind) {
                if o.kind == crate::task::Kind::Migrate {
                    self.swapping = false;
                }
                // An estimate started before its chain, registry or nodes changed is of the place left behind.
                let behind = o.kind == crate::task::Kind::Gas && self.gas_out_of_date();
                if o.stale || behind {
                    if let Err(f) = &o.result {
                        self.faults.push(f.clone());
                    }
                } else {
                    let said = crate::action::landed(self, o.kind, o.result.clone());
                    self.said.insert(o.kind, said);
                }
                continue;
            }
            // Results from an earlier source deliver only their trouble: the trouble is recorded (a task the
            // person started that failed must show), the reading does not enter the shell because it
            // describes the previous source.
            if o.stale {
                if let Err(f) = &o.result {
                    self.faults.push(f.clone());
                }
                continue;
            }
            // The swap's fetch landed (it is the only fetch that can be in flight while swapping): the home thaws.
            if o.kind == crate::task::Kind::Fetch {
                self.swapping = false;
            }
            match &o.result {
                Ok(_) => {
                    self.failed.remove(&o.kind);
                }
                Err(f) => {
                    self.failed.insert(o.kind, f.clone());
                    self.gate_refused(f);
                    // A fetch that failed after swapping this home (settled forward in its own run): the old
                    // data list grew, and the home in this place is another one now, opened again as it is.
                    // Any other failure touched nothing here.
                    if o.kind == crate::task::Kind::Fetch && !crate::local::is_cut(f) {
                        let before = self.aside.len();
                        self.reread_aside();
                        if self.aside.len() != before {
                            if let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) {
                                if let Err(e) = crate::action::reopen_here(self, &root) {
                                    self.faults.push(e);
                                }
                            }
                        }
                    }
                }
            }
            // The status line clears when the network is back. The evidence is a pass that actually got a
            // chain answer (header, fragment, balance), not merely `Ok`: reviews and checks that cannot reach
            // the chain also return `Ok` (turning unreachability into "unknown"), which does not mean the
            // network is back.
            if let Ok(d) = &o.result {
                if chain_answered(d) {
                    self.status = None;
                    self.chain_read_at = Some(now);
                }
            }
            match &o.result {
                // Passcode tasks were taken above (`vault_landed`) and never reach here.
                Ok(Done::Vault(_)) => {}
                // Taken above (`gate_landed`) and never reaches here.
                Ok(Done::GatePassed { .. }) => {}
                // Taken above (`action::landed`) and never reach here.
                Ok(Done::Gas { .. }) | Ok(Done::Took { .. }) | Ok(Done::Hashed { .. }) | Ok(Done::Copied { .. }) => {}
                Ok(Done::Check(r)) => self.last = Some(r.clone()),
                Ok(Done::Archive { bytes, items, skipped, mirror, records, machine_items }) => {
                    self.archive = Some(ArchiveRead {
                        bytes: *bytes,
                        items: *items,
                        skipped: *skipped,
                        mirror: mirror.clone(),
                        records: *records,
                    });
                    if machine_items.is_some() {
                        self.items_now = *machine_items;
                    }
                }
                Ok(d @ Done::Chain { .. }) => {
                    if let Done::Chain { head_time: Some(t), .. } = d {
                        self.note_chain_time(*t);
                    }
                    self.chain = Some(d.clone())
                }
                Ok(Done::Audited {
                    label,
                    complete,
                    broken,
                    entries,
                    report,
                    unanswered,
                    asked,
                    single_source,
                    fragment,
                }) => {
                    self.audit = Some(AuditRead {
                        label: label.clone(),
                        complete: *complete,
                        broken: *broken,
                        entries: *entries,
                        report: report.clone(),
                        unanswered: unanswered.clone(),
                        asked: *asked,
                        single_source: *single_source,
                        fragment: fragment.clone(),
                        at: now,
                        mark: self.audit_asked.unwrap_or(self.book_mark),
                    });
                    // Save this pass's `anchored` set: the next start says "checked last time" from it.
                    if let Some(h) = self.home.as_ref() {
                        let rows = crate::ledgerx::anchored_of(report);
                        let at = (self.clock)();
                        let whole = crate::auditx::whole(label, *asked, unanswered);
                        match crate::lastread::save_anchored(h, &rows, at, whole) {
                            Ok(()) => self.remembered = Some(crate::lastread::Anchored { rows, at, whole }),
                            Err(f) => self.faults.push(f),
                        }
                    }
                    // Self-audit and reconciliation give the same label: the pen follows it (as for
                    // `Reconciled`).
                    self.reconciled = Some((label.clone(), *complete, *entries));
                    self.pen = if *complete { Pen::Granted } else { Pen::Held };
                    // The anchor lamps' source changed, so the table is stale. Green comes only from the
                    // report's `anchored` item; a new report with an unrefreshed table would show the
                    // previous report's lamps, a silent staleness.
                    self.stale_rows();
                }
                // A read started from a source that has since been invalidated (a new report, new entries, a
                // queue change) is not accepted: it describes the state before. Leave it empty, clear
                // "started", and the next frame reads again.
                Ok(Done::Ledger { rows, strays, handed, gen }) => {
                    if *gen == self.rows_gen {
                        self.rows = Some((rows.clone(), *strays));
                        self.handed = handed.clone();
                    } else {
                        self.tasks.forget(crate::task::Kind::Ledger);
                    }
                }
                Ok(Done::Adopt { proofs, rows }) => {
                    self.proofs = Some(proofs.clone());
                    self.proofs_rows = Some(rows.clone());
                }
                Ok(Done::KeyAnchors { address, rows }) => self.key_anchors = Some((address.clone(), rows.clone())),
                Ok(Done::Claim { claim, blocks }) => self.claim = Some((claim.clone(), blocks.clone())),
                Ok(Done::Sighting { to, anchors, asked }) => {
                    self.sighting = Some((to.clone(), *anchors, *asked))
                }
                Ok(d @ Done::Book { .. }) => self.book = Some(d.clone()),
                // A reading speaks only for the row it asked: when the row was changed or removed while it was
                // in flight, the reading is dropped (the row shows no mark, as after any change).
                Ok(Done::NetRead { chain_id, registry, nodes, reading }) => {
                    let same = self.read_nets.as_ref().map(|t| t.iter().any(|n| n.is(*chain_id, registry) && n.nodes == *nodes)).unwrap_or(false);
                    if same {
                        self.net_reads.retain(|(c, r, _)| !(c == chain_id && r == registry));
                        self.net_reads.push((*chain_id, *registry, *reading));
                    }
                }
                // Broadcast landed: the queue file records those entries as submitted; the shell's copy
                // follows the disk, the table is invalidated (lamps now "waiting to be anchored"), and the
                // receipt wait starts.
                Ok(Done::Submitted { tx, chain, url, ids, gas, queue }) => {
                    self.queue = queue.clone();
                    self.gas = None;
                    self.stale_rows();
                    // Receipt watching is background work that stops while locked; `CatchUp` resumes it.
                    if !self.lock_pending {
                        crate::action::wait_submitted(self, tx.clone(), *chain, url.clone(), ids.clone(), *gas);
                    }
                }
                Ok(d @ Done::Anchored { .. }) => {
                    // Reload the queue table from disk. Dequeuing happens on a background thread, and a stale
                    // shell copy would send the same batch again (paying gas twice). It is read from the
                    // current home's disk: if the home changed since anchoring started, the table sent back
                    // belongs to the old home.
                    if let Done::Anchored { queue, .. } = d {
                        match self.home.as_ref().map(crate::queue::Queue::read) {
                            Some(Ok(q)) => self.queue = q,
                            Some(Err(f)) => {
                                // The disk copy cannot be read: show the entries the background returned for
                                // now. The "anchored before" field's source is the file on disk, so it stays
                                // empty until the file reads again; a guessed value never stands in for it.
                                self.queue = crate::queue::Queue { items: queue.clone(), anchored: Vec::new(), blocks: Vec::new() };
                                self.faults.push(f);
                            }
                            None => self.queue = crate::queue::Queue { items: queue.clone(), anchored: Vec::new(), blocks: Vec::new() },
                        }
                    }
                    // The gas estimate is void: it was for this batch, which is gone; keeping it would let
                    // the next "check gas before sending" gate pass on an old reading.
                    self.gas = None;
                    self.sent = Some(d.clone());
                    // The queued color's source (the queue table) changed, so the table is invalidated.
                    self.stale_rows();
                    // A batch of anchors landed, so the ledger no longer matches the report.
                    self.book_changed();
                    // Confirmed on chain: the chain is read again at once by the self-audit (a fork the send
                    // did not see shows there, and the pen follows its label).
                    if matches!(d, Done::Anchored { confirmed: true, .. }) {
                        crate::action::audit_after_send(self);
                    }
                }
                Ok(Done::Vetted(rows)) => {
                    for (p, d) in rows {
                        self.vetted.insert(p.clone(), d.clone());
                    }
                }
                Ok(Done::Depth { work, value }) => {
                    self.depth = Some((work.clone(), value.clone()))
                }
                Ok(d @ Done::Kit { .. }) => {
                    // A landed kit adds a row to the index: kit id, time, this ledger's root, the kit's
                    // absolute path, the manifest's original digests and notes, and `link` (kept if set by
                    // hand, else built from this home's publication base).
                    if let Done::Kit { root, publish, path, kit_id, .. } = d {
                        let at = std::fs::canonicalize(path).unwrap_or_else(|_| std::path::PathBuf::from(path));
                        let now = (self.clock)();
                        let r = crate::home::Home::open(root).and_then(|h| crate::home::machine_dir().and_then(|m| crate::kitsindex::add(&m, &h, &at, kit_id, now, publish.as_deref())));
                        if let Err(f) = r {
                            self.faults.push(f);
                        }
                    }
                    self.reread_kits();
                    self.kit = Some(d.clone())
                }
                Ok(Done::Keystore(crate::task::Keystore::BackedUp { id, seat, path, .. })) => {
                    // The location is recorded with the flag: the flag says it was done, the location says
                    // where, so whether the backup is really on disk can be asked (`identity::backup_seen`).
                    if let Some(id) = id {
                        if let Err(f) = crate::register::change(*seat, |reg| crate::identity::mark(reg, id, false, true, Some(path))) {
                            self.faults.push(f);
                        }
                    }
                    match crate::register::view(*seat) {
                        Ok(v) => self.seat_identities(Some(v)),
                        Err(f) => self.faults.push(f),
                    }
                }
                Ok(Done::BackupMade { path, summary }) => {
                    self.backup_made = Some((path.clone(), summary.clone()));
                    // The last backup is recorded in the machine settings: read them again.
                    match crate::machine::read() {
                        Ok(m) => self.machine = m,
                        Err(f) => self.faults.push(f),
                    }
                }
                Ok(Done::BackupSeen { summary }) => self.backup_peek = Some(summary.clone()),
                Ok(Done::Grants { rows, gen }) => {
                    if *gen == self.grants_gen {
                        self.grants = Some(rows.clone());
                    } else {
                        self.tasks.forget(crate::task::Kind::Grants);
                    }
                }
                Ok(Done::Diligence(r)) => self.diligence = Some((**r).clone()),
                Ok(Done::Verified(v)) => self.verified = Some((**v).clone()),
                Ok(Done::Delivery(c)) => self.delivery = Some(c.clone()),
                Ok(Done::Reviewed { cards, now }) => {
                    if let Some(t) = now {
                        self.note_chain_time(*t);
                    }
                    // The sentinel reads this pass's cards: revocation and change of owner each ring once,
                    // and rung keys are saved in settings.
                    let (fresh, keys) = crate::sentinelx::alarms(cards, &self.settings.alarmed, *now);
                    if !fresh.is_empty() {
                        self.rung.extend(fresh.iter().cloned());
                        for a in fresh.into_iter().rev() {
                            self.alarms.insert(0, a);
                        }
                        self.settings.alarmed.extend(keys);
                        self.attention = true;
                        if let Some(h) = self.home.as_ref() {
                            if let Err(f) = self.settings.write(h) {
                                self.faults.push(f);
                            }
                        }
                    }
                    // Save each grant's verdict and check time (`grants-held/<id>.verdict.json`); the next
                    // start speaks from them first.
                    if let Some(h) = self.home.as_ref() {
                        let at = (self.clock)();
                        for c in cards.iter() {
                            let checks = crate::vaultx::states(&c.checks);
                            let v = crate::lastread::Verdict { verdict: c.verdict.clone(), checks, at, upstream_label: c.upstream_label.clone(), chain_now: *now, anchored_at: c.anchored_at };
                            match crate::lastread::save_verdict(h, &c.id, &v) {
                                Ok(()) => {
                                    self.verdicts.retain(|(id, _)| !id.eq_ignore_ascii_case(&c.id));
                                    self.verdicts.push((c.id.clone(), v));
                                }
                                Err(f) => self.faults.push(f),
                            }
                        }
                    }
                    self.cards = Some((cards.clone(), *now));
                }
                Ok(Done::Held { held, rejected }) => {
                    self.held = Some(held.clone());
                    self.held_rejected = rejected.clone();
                }
                Ok(Done::Badge(b)) => self.badge = Some((**b).clone()),
                Ok(Done::Published { url, read }) => self.published = Some((url.clone(), read.clone())),
                // A fetched ledger landed: a checked tail removes the mark and allows writing; an anchor on
                // chain missing from this copy becomes "newer entries elsewhere".
                Ok(Done::Fetched { root, tail, .. }) => {
                    self.fetch_conflict = None;
                    self.last_aside = None;
                    self.fetch_landed(root, tail);
                }
                // The tail of each marked seat home was checked: each follows its answer as a fetch does; a home
                // whose chain could not be read keeps its mark and says why, without holding the others back.
                Ok(Done::TailChecked { checked }) => {
                    for (root, tail) in checked {
                        match tail {
                            Ok(tail) => self.fetch_landed(root, tail),
                            Err(f) => self.faults.push(f.clone()),
                        }
                    }
                }
                Ok(Done::FetchConflict { root, offline, fetched, rows }) => {
                    self.fetch_conflict = Some(Conflict { root: root.clone(), offline: *offline, fetched: *fetched, rows: rows.clone() });
                }
                Ok(Done::FetchedAside { aside, fetched }) => {
                    self.fetch_conflict = None;
                    if let Done::Fetched { root, tail, .. } = &**fetched {
                        // The home in this place is a fresh one now: opened again as it is, then the fetch
                        // lands as any other (the mark, the ledger step).
                        if self.home.as_ref().map(|h| crate::home::same_place(h.root(), root)).unwrap_or(false) {
                            if let Err(f) = crate::action::reopen_here(self, root) {
                                self.faults.push(f);
                            }
                        }
                        self.fetch_landed(root, tail);
                    }
                    self.last_aside = Some(aside.clone());
                    self.reread_aside();
                }
                Ok(Done::Checked(c)) => {
                    if let (Some(t), crate::checkx::NowFrom::Chain) = (c.now, &c.now_from) {
                        self.note_chain_time(t);
                    }
                    self.checked = Some((**c).clone())
                }
                Ok(Done::Reconciled { label, complete, entries }) => {
                    self.reconciled = Some((label.clone(), *complete, *entries));
                    // The only rule for the pen: the core's label is COMPLETE. The shell does not lean
                    // toward green, and the reverse holds too: when the core says incomplete, the pen is
                    // withdrawn; "complete last time" is not this time's answer.
                    self.pen = if *complete { Pen::Granted } else { Pen::Held };
                }
                Err(f) => {
                    // Network errors go to the status line, not notices: only the status sentence changes;
                    // the fault table still records them.
                    if crate::watchx::is_network(f) {
                        self.status = Some((crate::lang::Key::StatusNet, f.evidence()));
                    }
                    self.faults.push(f.clone())
                }
            }
        }
        // The watch table is recomputed from readings; what should ring rings once.
        if !got.is_empty() {
            self.sweep();
        }
        // A lock waiting for local writes completes once they have all landed.
        if self.lock_pending && !self.tasks.flying().into_iter().any(|k| k.writes_local()) {
            self.lock_pending = false;
            crate::keybox::lock();
            self.after_lock();
        }
        // The vault changed while a listing was in flight: list again now that it landed.
        if self.held_dirty && !self.tasks.in_flight(crate::task::Kind::Held) {
            self.held_dirty = false;
            self.tasks.forget(crate::task::Kind::Held);
            let _ = crate::action::apply(self, crate::action::Action::ListHeld);
        }
        if !self.held_back.is_empty() {
            let mut all = std::mem::take(&mut self.held_back);
            all.extend(got);
            return all;
        }
        got
    }

    /// Receive once and hold (used while the test driver waits for a passcode task): recorded as usual, delivered at
    /// the next `drain_at`.
    pub fn drain_hold(&mut self) {
        let got = self.drain_at(0.0);
        self.held_back = got;
    }

    /// Idle lock: the last human input was at `last`, now is `now` (same clock, seconds); at this machine's
    /// configured time, take the lock path (the same as pressing lock; the master key is wiped byte by byte).
    /// Returns whether it locked. The window asks every frame; tests inject both times.
    pub fn idle_tick(&mut self, last: f64, now: f64) -> bool {
        // Resealing refuses every other action and a pending lock is already locking: nothing to ask then.
        if self.rekeying || self.lock_pending || !self.unlocked() || !crate::machine::idle_locks(&self.machine, last, now) {
            return false;
        }
        matches!(crate::action::apply(self, crate::action::Action::Lock), crate::action::Applied::LockedUp)
    }

    /// Record chain time read back on a chain round trip. Whoever reads the chain writes it (chain read, gas
    /// estimate, review, check); it only moves forward.
    pub fn note_chain_time(&mut self, t: u64) {
        self.chain_time = Some(self.chain_time.map(|x| x.max(t)).unwrap_or(t));
    }

    pub fn chain_now(&self) -> Option<u64> {
        // Chain time has one reading: the latest of the time recorded on chain round trips (`chain_time`),
        // the block times of this ledger's anchors, and the chain time the last vault review brought back.
        let vault = self.cards.as_ref().and_then(|(_, n)| *n);
        let ledger = self.audit.as_ref().and_then(|a| {
            crate::auditx::rows_of(&a.report, zikaron::tokens::Key::Anchored)
                .iter()
                .filter_map(|r| match r {
                    zikaron::json::Value::Obj(m) => m
                        .iter()
                        .find(|(k, _)| k == zikaron::tokens::Key::Anchors.as_str())
                        .and_then(|(_, x)| match x {
                            zikaron::json::Value::Arr(a) => a.first().cloned(),
                            _ => None,
                        }),
                    _ => None,
                })
                .filter_map(|one| match &one {
                    zikaron::json::Value::Obj(m) => m
                        .iter()
                        .find(|(k, _)| k == zikaron::tokens::Key::BlockTimestamp.as_str())
                        .and_then(|(_, x)| match x {
                            zikaron::json::Value::Int(n) => Some(*n),
                            _ => None,
                        }),
                    _ => None,
                })
                .max()
        });
        [ledger, vault, self.chain_time].into_iter().flatten().max()
    }

    /// This seat's watch table. Pure readings, no disk. The seat comes from the role.
    pub fn watch_rows(&self) -> Vec<crate::watchx::Row> {
        match self.settings.role {
            crate::roles::Role::Author => crate::watchx::author(
                self.rows.as_ref().map(|(r, _)| r.as_slice()),
                self.queue.len(),
                self.grants.as_deref(),
                (self.machine.backup.as_ref(), self.items_now),
                self.audit.as_ref().map(|a| (a.label.as_str(), &a.report, a.unanswered.as_slice())),
                self.chain_now(),
            ),
            crate::roles::Role::Grantee => crate::watchx::grantee(
                self.cards.as_ref().map(|(c, _)| c.as_slice()),
                &self.alarms,
                self.cards.as_ref().and_then(|(_, n)| *n),
                (self.machine.backup.as_ref(), self.items_now),
            ),
        }
    }

    /// One watch sweep: compute the table, pick what should ring, drop what already rang, record, hand to the
    /// screen. Rung keys share a record with the sentinel (`settings.alarmed`); a failed save is named in the
    /// fault table (only with a home).
    pub fn sweep(&mut self) {
        let rows = self.watch_rows();
        let (fresh, keys) = crate::watchx::fresh(crate::watchx::notices(&rows), &self.settings.alarmed);
        if fresh.is_empty() {
            return;
        }
        self.settings.alarmed.extend(keys);
        self.notices.extend(fresh.iter().cloned());
        self.fresh.extend(fresh);
        self.attention = true;
        if let Some(h) = self.home.as_ref() {
            if self.writable() {
                if let Err(f) = self.settings.write(h) {
                    self.faults.push(f);
                }
            }
        }
    }

    /// The four status line readings. This layer writes no sentence: the screen builds it from the string
    /// table, so the status line follows the language.
    pub fn note_parts(&self) -> (&'static str, crate::lang::Key, usize, usize, usize) {
        (
            build_kind(),
            match self.lock.as_ref() {
                None => crate::lang::Key::NoteNoHome,
                Some(l) => {
                    if l.mode().writable() {
                        crate::lang::Key::NoteWriter
                    } else {
                        crate::lang::Key::NoteReader
                    }
                }
            },
            trace::dropped(),
            self.fonts.faces.len(),
            fonts::Role::ALL.len(),
        )
    }
}

/// The primary identity as the shell holds it: id and kind while open; locked, the store keeps the id sealed
/// and only the kind is read (the lock screen offers words or a key file by it; the id reads empty).
fn primary_reading() -> Option<(String, crate::keybox::PrimaryKind)> {
    match crate::keybox::primary() {
        Ok(p) => p,
        Err(_) => crate::keybox::primary_kind().ok().flatten().map(|k| (String::new(), k)),
    }
}

/// What a gas estimate and its fees are read from: this home's chain, registry contract and nodes (as a set).
fn gas_source(s: &Settings) -> (Option<u64>, Option<crate::key::Address>, std::collections::BTreeSet<&str>) {
    (s.chain_id, s.registry, s.endpoints.iter().map(String::as_str).collect())
}
