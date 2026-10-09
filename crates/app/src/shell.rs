//! Shell state, shared by the window and the test hooks. This layer does not know the zikaron/1 law: it
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

/// The bar at the top of every page.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Banner {
    /// Nothing shown.
    None,
    /// Broken chain: the whole desk is read-only; recover first.
    Broken,
    /// Cannot write (a reader, or the lock not yet taken), with who the writer is.
    ReadOnly(String),
    /// Read-only because the home's writer mark names another machine; the person may write from this one
    /// (`Action::TakeWriter`).
    OtherMachine,
    /// Read-only because this version cannot read the home's writer mark (`lock::Mark::Unread`: a read error,
    /// a wrong shape, a later version's format); the person may write from this machine (`Action::TakeWriter`),
    /// which replaces the mark with this machine's.
    MarkUnread,
    /// Already handed over by succession, with the new key.
    Handed(String),
}

impl Banner {
    pub fn as_str(&self) -> &'static str {
        match self {
            Banner::None => "none",
            Banner::Broken => "broken",
            Banner::ReadOnly(_) => "read_only",
            Banner::OtherMachine => "other_machine",
            Banner::MarkUnread => "mark_unread",
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
    /// Kept separate from `key()` so "page names follow the language" can fail on its own: a broken string
    /// table breaks every sentence, a break here only page names.
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

/// The key store as the shell holds it: the store's own state (`keybox::State`), or a store file that exists
/// but cannot be read (`keybox::state` returns `KEYBOX_SHAPE`, kept here for display). `Damaged` exists only
/// in the shell: a damaged file is never read as "no store yet", which would open the first-run wizard over the
/// keys it holds. A damaged store keeps the gate up and no key is ready.
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

    /// Whether the lock gate covers the window: per the store's own state, and always for a damaged store.
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

/// Build kind.
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
    /// The twelve words shown after the passcode check (memory only; each in the secret type, zeroed when hidden,
    /// on locking and on quitting).
    pub words: Option<Vec<crate::secret::Secret>>,
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
    /// Whether the vault still holds anything to lose (any recovery seal or key slot). Read with `vault`.
    /// Decides whether the lock screen offers "reset the vault" and whether `keybox::reset_empty` refuses.
    pub vault_recoverable: bool,
    /// The answer of a passcode task: key derivation runs in the background, and when its result arrives the
    /// frame-side half finishes and writes its answer here. The window and the test hooks each take it
    /// (taking clears it).
    pub vault_said: Option<crate::action::Applied>,
    /// The answer of an export whose exit gate passed in the background (`action::gate_landed`). Kept
    /// separate so passcode and export answers never overwrite each other. The window and the test hooks each
    /// take it (taking clears it).
    pub gate_said: Option<crate::action::Applied>,
    /// Answers of actions whose slow half runs in the background and answers when it lands (gas estimate,
    /// taking a content hash, recording files, moving the home: `action::landed`), by kind; separate from the
    /// passcode and exit-gate answers. The window and the test hooks each take theirs (taking clears it).
    pub said: std::collections::BTreeMap<crate::task::Kind, crate::action::Applied>,
    /// This machine's settings (`machine.json`). Read once at start; reread after each change.
    pub machine: crate::machine::Machine,
    /// Other results received while the test hooks waited for a passcode task, kept for the next receive.
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
    /// The last gas estimate: (anchors in this batch, gas). Sending checks it: a count mismatch blocks the
    /// batch, so what is shown is what is sent.
    pub gas: Option<(usize, u64)>,
    /// This batch's two fee fields: computed from the chain's base fee during estimation; the confirmation
    /// card and the balance check before sending read the same values.
    pub fees: Option<zikaron_anchor::send::Fees>,
    /// The nodes the fee reading left out because they serve another chain, each named (shown with the fees).
    pub fees_left: Vec<String>,
    /// The batch whose transaction the last receipt wait did not see included: its transactions, the latest as
    /// the chain holds it, and what may be done (a resend at which fees). Cleared once one is included, when a
    /// resend is taken (a new wait starts), and when the source changes.
    pub stuck: Option<crate::task::Stuck>,
    /// The two readings above belong to one chain, registry contract and node set; whenever those change
    /// (`commit_settings`, a source change) the readings are voided (`gas_void`) and this count advances.
    pub gas_epoch: u64,
    /// The count when the pending estimate started: it lands only if nothing it depends on has changed since
    /// (`gas_out_of_date`); otherwise it is dropped like a reading of an earlier source.
    pub gas_asked: Option<u64>,
    /// Backoff delays when the chain rate-limits: retry after each, then move to the next endpoint. Default
    /// `chainx::SEND_BACKOFF`; tests set zeros so they never wait on the wall clock.
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
    /// The last verdict of each held grant (disk cache, loaded when the home opens). Cards show it until
    /// re-checked.
    pub verdicts: Vec<(String, crate::lastread::Verdict)>,
    /// The read-only mark of a restored identity (`settings/unfetched.json` of this home, loaded when it
    /// opens). While present, the ledger-writing and anchoring actions are refused.
    pub unfetched: Option<crate::restorex::State>,
    /// Fetching found this home at odds with the fetched ledger; waiting for the person to confirm
    /// (`Action::FetchAside`) or decline. Its rows are this home's entries that would stay in the old data.
    pub fetch_conflict: Option<Conflict>,
    /// The old data on this machine (homes set aside after a conflict), read at opening.
    pub aside: Vec<OldData>,
    /// Old data open to read, and where to come back to.
    pub old_view: Option<OldView>,
    /// The old data the last fetch set aside (announced once after fetching, with a "view" link).
    pub last_aside: Option<std::path::PathBuf>,
    /// Digests of export attachments by path: content digest or error, computed in the background. Kept
    /// across homes, since a digest belongs to the file.
    pub vetted: std::collections::BTreeMap<String, Result<String, Fault>>,
    /// The kit index (machine directory `kits/index.json`); `None` when not read yet or unreadable (the
    /// trouble is recorded).
    pub kits_index: Option<Vec<crate::kitsindex::Row>>,
    /// The read-only network table (the machine directory's); `None` when not read yet or unreadable (the
    /// trouble is recorded). The paths that read it take it from disk when they run.
    pub read_nets: Option<Vec<crate::readnets::Net>>,
    /// The last reading of each read-only network (chain id, registry), from its "read the chain" key.
    pub net_reads: Vec<(u64, crate::key::Address, crate::widex::Reading)>,
    /// The main network's custom fields as last checked when saving (chain, registry, reading), shown beside
    /// them; written to settings only when accepted (`action::basis_read`).
    pub basis_read: Option<(u64, crate::key::Address, Option<crate::widex::Reading>)>,
    /// The root of this home's ledger (the export page lists only its kits; read with `reread_kits`, never in
    /// the frame).
    pub kits_root: Option<String>,
    /// Wall clock in seconds: both caches stamp times and judge staleness with it. The system clock in the
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
    /// Files in the vault directory with entry names that fail acceptance; empty when all pass.
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
    /// The primary identity settled by the first unlock after an upgrade (shown once, then taken).
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
    /// The command-line channel (`door`): open while unlocked with a writable home, bound to that home.
    pub door: Option<crate::door::Door>,
    /// The command-line request in progress, waiting for its slow half to land.
    pub door_doing: Option<crate::door::Doing>,
    /// Results of command-line requests, for the window to report as it reports a click's.
    pub door_told: Vec<crate::action::Applied>,
    /// Wakes the frame when a request arrives (set by the window; without it, the next drain picks it up).
    pub door_waker: Option<crate::door::Waker>,
    /// The home whose channel would not open, and why (retried at every drain; not listed among the troubles).
    pub door_shut: Option<(std::path::PathBuf, Fault)>,
    /// Quitting: the channel stays closed from now on.
    pub door_off: bool,
    /// How many requests this shell answered through the channel (counted where each reply is sent).
    pub door_answered: usize,
    /// "Enable command line": what is at its install path, as last read (`Action::ReadCliPath`, and after
    /// turning it on or off); `None` until read.
    pub cli_path: Option<zikaron_os::cli_path::State>,
}

impl Shell {
    /// Start the shell. Skin and fonts are applied by the control library's `skin::dress`; this takes its
    /// result, opens the trace channel, emits the first trace mark and prepares the background tasks. The
    /// window and the test hooks start the same way.
    pub fn boot(dressed: zikaron_ui::skin::Dressed) -> Shell {
        let found = dressed.found;
        let missing = dressed.missing;

        // Open the trace channel before any mark is emitted.
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
            fees_left: Vec::new(),
            stuck: None,
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
            basis_read: None,
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
            door: None,
            door_doing: None,
            door_told: Vec::new(),
            door_waker: None,
            door_shut: None,
            door_off: false,
            door_answered: 0,
            cli_path: None,
        }
    }

    /// The block time a held grant was anchored at in its issuer's ledger: this run's re-check first, else the
    /// cached verdict (so a date range filter still works after a restart). `None` while neither has read it.
    /// The vault's date range filter uses only this.
    pub fn held_anchored_at(&self, id: &str) -> Option<u64> {
        // A card of this run that has no time yet does not hide the cached one.
        let card = self.cards.as_ref().and_then(|(cs, _)| cs.iter().find(|c| c.id.eq_ignore_ascii_case(id)).and_then(|c| c.anchored_at));
        card.or_else(|| self.verdicts.iter().find(|(g, _)| g.eq_ignore_ascii_case(&crate::lastread::grant_form(id))).and_then(|(_, v)| v.anchored_at))
    }

    /// Whether the anchor key is in the vault, recording its address for display. Checked each time, not
    /// cached.
    pub fn refresh_anchor(&mut self) -> Result<bool, Fault> {
        // A closed vault reads as "no key available now", not an error. This runs on every path into a home
        // (open, switch seat, switch identity); as an error, those paths would fail after writing the registry
        // and settings, leaving the disk changed while the screen reported failure. While locked the screen
        // says "no signing key yet", which is true.
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
        // Local data opens only now: the home is closed while locked, so land on the seat and open its home as
        // the window does at start.
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
        // Background work stopped while locked: catch up once now (`Action::CatchUp`).
        if opened_now {
            let _ = crate::action::apply(self, crate::action::Action::CatchUp);
        }
    }

    /// First half of a lock requested while local data is being written (`Action::Lock`): the vault reads as
    /// locked on screen and in every gate and key-related readings are cleared, but the home stays open so
    /// in-flight tasks can land.
    pub fn begin_lock(&mut self) {
        self.lock_pending = true;
        // From here the vault itself answers locked (one source, so every reread sees it); the key stays in
        // memory until in-flight writes land.
        crate::keybox::begin_lock();
        self.reread_vault();
        self.anchor = None;
        self.words = None;
        self.new_words = None;
    }

    /// After the vault locks: the key is unavailable, so key-related readings are cleared (address, shown
    /// words, new words not yet confirmed). Key-using actions are refused earlier by the action layer's table;
    /// this only clears readings.
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
        // Unreadable counts as "still holds something": the reset option is withheld whenever the reading is
        // uncertain.
        self.vault_recoverable = crate::keybox::recoverable().unwrap_or(true);
        self.primary = primary_reading();
    }

    /// Set the identity table together with its disk reading.
    ///
    /// Whether the current identity's backup file exists is a disk read, so it is updated in the same step as
    /// the table; every update goes through here and the frame only reads the shell field.
    ///
    /// `None` covers "the registry cannot be read": both fields become empty. Nothing else in the shell writes
    /// `identities`.
    pub fn seat_identities(&mut self, reg: Option<crate::identity::Registry>) {
        self.backup_seen = reg
            .as_ref()
            .and_then(|r| r.now())
            .map(|(row, _)| crate::identity::backup_seen(row));
        self.identities = reg;
    }

    /// Whether this seat is empty (an existing key holds only one seat): the current identity has no address
    /// on this seat.
    ///
    /// The single source for the identity card's text and three buttons, "domains this seat can sign", and the
    /// data card's home and key buttons. Without an identity it returns `false` (that state has its own text).
    pub fn seat_unseated(&self) -> bool {
        self.identities
            .as_ref()
            .and_then(|r| r.now())
            .map(|(row, _)| row.address(self.settings.role).is_none())
            .unwrap_or(false)
    }

    /// This seat has no home. An empty seat has no home, and the shell reflects it: the home and its lock are
    /// released, readings tied to the home are invalidated (`source_changed`), and queue and ledger head are
    /// cleared, so every page shows "no home". Otherwise the other seat's home and ledger would show as this
    /// seat's.
    ///
    /// Releasing the lock gives up the writer role for that home; returning to that seat, `open_home_at` takes
    /// the lock again as writer.
    pub fn close_home(&mut self) {
        self.source_changed(Source::Home);
        self.home = None;
        self.lock = None;
        self.queue = crate::queue::Queue::default();
        self.rooted = false;
        // No home, no command-line channel.
        self.door_sync();
    }

    /// The interface language now: the home's own choice once readable, else the last choice on this machine
    /// (used at the passcode gate before unlocking); `None` means neither was ever chosen.
    pub fn speaks(&self) -> Option<crate::lang::Lang> {
        self.settings.lang.or(self.machine.lang)
    }

    /// Whether the vault is open. The lock screen and the action layer's table both ask this.
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

    /// Save settings. Without an open home this returns an error; it never drops the change silently.
    pub fn save_settings(&self) -> Result<(), Fault> {
        self.may_save_settings()?;
        let h = self.home.as_ref().expect("上一句已经问过家在不在");
        self.settings.write(h)
    }

    /// Change settings: write to disk first; only a successful write counts. The change applies to a copy that
    /// replaces the shell's settings only after writing, so on failure the shell is unchanged. Changing memory
    /// first would show a new seat or node that disappears after restart when the write failed (a read-only
    /// second instance, a full disk).
    pub fn commit_settings(&mut self, f: impl FnOnce(&mut Settings)) -> Result<(), Fault> {
        self.may_save_settings()?;
        let mut next = self.settings.clone();
        f(&mut next);
        let h = self.home.as_ref().expect("上一句已经问过家在不在");
        next.write(h)?;
        // A gas estimate and its fees belong to one chain, registry contract and node set; saved settings that
        // change any of them void those readings, whichever key was saved (nodes, chain fields, a network
        // chosen or cleared).
        let moved = gas_source(&self.settings) != gas_source(&next);
        let nodes_moved = self.settings.endpoints != next.endpoints;
        self.settings = next;
        if moved {
            self.gas_void();
        }
        // Which chain each node serves is asked once per process and cached (`chainx::serving`); saved nodes
        // are asked again, so a node restarted on another chain is not judged by what it served before.
        if nodes_moved {
            crate::chainx::forget_serving();
        }
        Ok(())
    }

    /// Void the gas estimate and its fees: clear them, so an estimate still in flight lands as out of date.
    pub fn gas_void(&mut self) {
        self.gas = None;
        self.fees = None;
        self.fees_left.clear();
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

    /// What the bar at the top of each page says. The window only turns it into text and a color; tests read
    /// which bar is up.
    pub fn banner(&self) -> Banner {
        if let Some(to) = self.handed.as_ref() {
            return Banner::Handed(to.clone());
        }
        if self.broken() {
            return Banner::Broken;
        }
        if self.home.is_some() && self.lock.as_ref().map(|l| l.mode() == crate::lock::Mode::OtherMachine).unwrap_or(false) {
            // Which of the two: an unreadable mark says so, never claims that another machine writes.
            return match self.lock.as_ref().and_then(|l| l.mark_trouble()) {
                Some(_) => Banner::MarkUnread,
                None => Banner::OtherMachine,
            };
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
    /// or the ledger changed since (entries written, anchors landed). Neither waits for the period; the next
    /// frame audits.
    ///
    /// The period (`audit_due`) governs repeated checks; this governs whether the report at hand is stale. The
    /// start mark is recorded on success or failure (`audit_asked`), so with endpoints down this does not
    /// redial every frame.
    pub fn audit_stale(&self) -> bool {
        // Even with a period of zero: "does not run by itself" is about repetition, and the anchor lamps come
        // from the report, so a report about an older ledger would make them lie.
        self.audit_possible() && self.audit_asked != Some(self.book_mark)
    }

    /// [`Shell::tail_due`], recording the attempt for this ledger state before anything is asked: whatever the
    /// answer (including a refusal before the check starts), it is not due again on the next frame. A ledger
    /// change, new nodes or basis, or another home make it due again.
    pub fn take_tail_due(&mut self) -> bool {
        let due = self.tail_due();
        if due {
            self.tail_asked = Some(self.book_mark);
        }
        due
    }

    /// Whether this identity's tail is due to be checked against the chain (`Action::CheckTail`): the open
    /// home holds the not-fetched mark (either state), basis and nodes are set, no fetch is in flight, and the
    /// ledger as it is now has not been checked since its nodes or basis last changed (`tail_asked`). A pure
    /// decision without disk or network, used by the window's clock and the places that make it due.
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
        // A home without a genesis does not run the clock: grantee homes often have no ledger, and asking the
        // chain would only add NO_GENESIS to the trouble panel. `rooted` is read when the home opens and set at
        // genesis.
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

    /// Whether this instance may write: the lock, plus the broken-chain and handed-over gates.
    ///
    /// After a broken chain the whole app is read-only (appending would deepen the damage). Every write
    /// (settings, entries, moving, bundles) asks this one predicate.
    pub fn writable(&self) -> bool {
        if self.broken() || self.handed.is_some() {
            return false;
        }
        self.lock.as_ref().map(|l| l.mode().writable()).unwrap_or(false)
    }

    /// Whether this ledger can take new entries now: both the lock and the pen are needed.
    pub fn may_write_entries(&self) -> Result<(), Fault> {
        // Distinct refusals: "no home open" and "another writer has this home" differ, and sharing `READ_ONLY`
        // would point at a second instance that does not exist. Succession comes first: once handed over, the
        // writer should hear that the ledger belongs to the new key (law §7.3: the new key writes this ledger
        // from then on).
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

    /// The only place the shell's copies of disk state are loaded; opening a home comes here.
    ///
    /// Endpoints, the anchor queue and the first-window checklist live on disk with a copy in the shell for
    /// the frame. A malformed file is reported at once instead of being read as empty.
    pub fn hydrate(&mut self, home: &crate::home::Home) -> Result<(), Fault> {
        // Unreadable settings mean the home cannot open: endpoints, basis and registrations live there, and
        // continuing with empty ones would make the screen show something else entirely.
        self.settings = Settings::read(home)?;
        self.endpoints = self
            .settings
            .endpoints
            .iter()
            .filter_map(|spec| Endpoint::parse(spec))
            .collect();
        // The two local bookkeeping files do not block opening when malformed, but the problem is shown.
        // Reading them as empty silently would lose queued entries; refusing to open would block the
        // broken-chain recovery path, which needs the home open. So the fault goes to the screen and an empty
        // value is used.
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
        // Load what the last pass knew first: the audit set and grant verdicts show at start while the
        // background audits again (`audit_stale` stays true for this home). A malformed cache is reported and
        // does not block opening.
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
        // writes (never treated as absent); the fault goes to the trouble panel.
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

    /// Reread the kit index (in the machine directory). Read errors go to the trouble panel and the screen
    /// says it was not read.
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

    /// Reread the read-only network table (in the machine directory). Read errors go to the trouble panel.
    pub fn reread_nets(&mut self) {
        match crate::action::read_nets_now() {
            Ok(n) => self.read_nets = Some(n),
            Err(f) => {
                self.read_nets = None;
                self.faults.push(f);
            }
        }
    }

    /// Whether the periodic vault review should start (card checks need the basis): not with a zero period,
    /// missing endpoints or basis, one in flight, or less than a period since the last. A pure decision, no
    /// disk.
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

    /// Mark this table stale. Clearing the reading and allowing a new read must happen together: `Tasks`
    /// records "started" (not reset by failure, which prevents spinning), so clearing only the reading would
    /// never trigger another read. Every place that sets `rows` to `None` goes through here (`grants`
    /// likewise).
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

    /// The ledger's source changed. `Source::Ledger`: the ledger in the same home was replaced (adoption in
    /// place, mirror restore), invalidating everything read from it. `Source::Home`: another home, also
    /// invalidating settings, vault, chain readings, pen and alarms. Background tasks tied to the source move
    /// to a new epoch, and results from the old epoch deliver only their faults (`Tasks::new_epoch`).
    ///
    /// The destructuring has no `..`, so every new shell field must be assigned a scope here. The pen and
    /// alarms clear only on a home change: a failed adoption or restore in the same home keeps the
    /// broken-chain bar and read-only state.
    pub fn source_changed(&mut self, scope: Source) {
        self.tasks.new_epoch();
        let home = scope == Source::Home;
        let Shell {
            // Tied to the machine, the person and this session, not to the ledger's source:
            page: _,
            // Names the fetch kind's in-flight task, which a source change does not stop (its result is set aside).
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
            basis_read: _,
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
            // The command-line channel follows the home by its own rule (`door_sync`), not by a source change.
            door: _,
            door_doing: _,
            door_told: _,
            door_waker: _,
            door_shut: _,
            door_off: _,
            door_answered: _,
            // What is at the command-line install path belongs to the machine, not to a ledger's source.
            cli_path: _,
            // The passcode answer and machine settings follow the machine; results held while the test hooks
            // wait follow the session.
            vault_said: _,
            gate_said: _,
            // These kinds follow the source: one started for the earlier source lands stale and writes nothing here.
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
            fees_left,
            stuck,
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
        fees_left.clear();
        *stuck = None;
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
        // The two caches belong to that home: cleared on a home change, and `hydrate` reads the new home's.
        *remembered = None;
        verdicts.clear();
        *unfetched = None;
        // The last delivery conclusion belongs to that home's grant: cleared on a home change.
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

    /// Record a fault and return it to the screen. There is no silent path.
    pub fn trouble(&mut self, f: Fault) -> crate::action::Applied {
        self.faults.push(f.clone());
        crate::action::Applied::Trouble(f)
    }

    /// A fetched ledger landed in the home at `root` (a fetch, or a fetch after setting a home aside): update
    /// the mark from the tail check, on the home the fetch started in (even if another is open now).
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
                // The ledger grew (from empty): record the root and advance the ledger mark (the self-audit
                // follows the new ledger); the table is invalidated.
                if let Some(h) = self.home.as_ref() {
                    self.rooted = crate::ledgerx::head(h).map(|x| x.is_some()).unwrap_or(false);
                }
                self.book_changed();
                // The answer is about the ledger as it now is, so the tail is not due again until the ledger,
                // its nodes or basis, or the home change (a "newer entries elsewhere" answer is not re-asked
                // every frame).
                self.tail_asked = Some(self.book_mark);
                // With a root, the export page lists kits of this ledger.
                self.reread_kits();
            }
            Ok(_) => {}
            Err(f) => self.faults.push(f),
        }
        self.stale_rows();
    }

    /// An export refused because the chain holds anchors this home lacks: the gate left the read-only mark on
    /// disk (`exitgate::pass`); the shell picks it up now so writing waits for a fetch immediately.
    pub fn gate_refused(&mut self, f: &crate::fault::Fault) {
        if f.which() == Some(crate::fault::Known::NewerElsewhere) {
            if let Some(home) = self.home.as_ref() {
                if let Ok(s) = crate::restorex::read(home) {
                    self.unfetched = s;
                }
            }
        }
    }

    /// Receive results and record them.
    pub fn drain(&mut self) -> Vec<Outcome> {
        self.drain_at(0.0)
    }

    /// As [`Shell::drain`], with the arrival time from the caller (the interface clock). The self-audit clock
    /// schedules by it; this layer never asks the system time.
    pub fn drain_at(&mut self, now: f64) -> Vec<Outcome> {
        // The trace file reached its cap: say so once when writing stops.
        if let Some(f) = trace::take_full() {
            self.faults.push(f);
        }
        let mut got = self.tasks.drain_at(now);
        for o in got.iter_mut() {
            // Passcode tasks: the frame-side half runs here and its answer goes to `vault_said`; a refusal is
            // recorded there (`Shell::trouble`), not again below.
            if crate::landing::goes_back(o.kind, crate::landing::Back::Vault) {
                let got = o.result.clone().map(|d| match d {
                    Done::Vault(v) => v,
                    _ => crate::task::Vault::Opened,
                });
                let said = crate::action::vault_landed(self, got);
                self.vault_said = Some(said);
                continue;
            }
            // An export's exit gate passed: the export runs now and its answer goes to `gate_said`, separate
            // from passcode answers. A refused gate takes the common path below (recorded, and the read-only
            // mark picked up). A pass holds only for the home and source it started on (`o.stale` says whether
            // the source moved); `gate_landed` checks that before anything runs.
            if let (crate::task::Kind::Gate, Ok(Done::GatePassed { root, then, pass })) = (o.kind, &o.result) {
                if let Some(said) = crate::action::gate_landed(self, root.clone(), (**then).clone(), pass, o.stale) {
                    self.gate_said = Some(said);
                }
                continue;
            }
            // Actions whose slow half ran in the background: their frame-side half runs here (`action::landed`)
            // and the answer goes to `said`, bypassing the common path below. One started for an earlier source
            // lands nothing. A move ends the home freeze whatever its outcome.
            if crate::action::lands_said(o.kind) {
                if o.kind == crate::task::Kind::Migrate {
                    self.swapping = false;
                }
                // An estimate started before its chain, registry or nodes changed describes the old setup.
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
            // Results from an earlier source deliver only their fault: a failed task the person started must
            // show, but the reading describes the previous source and is not kept.
            if o.stale {
                if let Err(f) = &o.result {
                    self.faults.push(f.clone());
                }
                continue;
            }
            // The swap's fetch landed (the only fetch possible while swapping): the home is unfrozen.
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
                    // A failed backup is recorded in the machine settings (`backup_failed`, the lamp's red):
                    // reread them, as after a successful backup.
                    if o.kind == crate::task::Kind::Backup {
                        match crate::machine::read() {
                            Ok(m) => self.machine = m,
                            Err(e) => self.faults.push(e),
                        }
                    }
                    // A fetch that failed after swapping this home (completed forward in its own run): the
                    // old-data list grew and a different home is now at this path, so reopen it. Any other
                    // failure changed nothing here.
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
                // A file dialog's answer goes back to the requester (the window's path mailbox).
                Ok(Done::Path(_)) => {}
                // What is at the command-line install path now, read after turning it on or off.
                Ok(Done::CliPath(s)) => self.cli_path = Some(s.clone()),
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
                    // Not counted: the count since the last backup is unknown (never an old number), and the
                    // reason is shown.
                    match machine_items {
                        Ok(n) => self.items_now = Some(*n),
                        Err(f) => {
                            self.items_now = None;
                            // Reported once while it persists: the measure runs after every ledger change, and
                            // the same fault is not news each time.
                            if !self.faults.iter().any(|x| x.said() == f.said()) {
                                self.faults.push(f.clone());
                            }
                        }
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
                // queue change) describes the earlier state and is dropped: leave it empty, clear "started",
                // and the next frame reads again.
                Ok(Done::Ledger { rows, strays, handed, r#gen }) => {
                    if *r#gen == self.rows_gen {
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
                Ok(Done::BasisRead { chain, registry, from_block, reading, said, after_nodes, asked }) => {
                    // The field or its chain's nodes changed since it was asked: the reading is void and lands
                    // as from an earlier source (reported nowhere); the check runs again for the current values.
                    if !crate::action::basis_read(self, *chain, *registry, *from_block, *reading, said.clone(), *after_nodes, *asked) {
                        o.stale = true;
                    }
                }
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
                // An anchoring started before the source changed continued for its own home (its queue file
                // records it); here the current home's queue is reread from disk, and no wait, offer or result
                // from that other home is taken (the periodic resume follows this disk).
                Ok(Done::Submitted { .. }) | Ok(Done::Anchored { .. }) if o.earlier => {
                    if let Some(Ok(q)) = self.home.as_ref().map(crate::queue::Queue::read) {
                        self.queue = q;
                    }
                    self.stuck = None;
                    self.stale_rows();
                }
                Ok(Done::Submitted { tx, chain, url, ids, gas, queue }) => {
                    self.queue = queue.clone();
                    self.gas = None;
                    // A new transaction is out (a send, or a resend): its own wait decides what is offered next.
                    self.stuck = None;
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
                                // now. The "anchored before" field comes only from the file on disk, so it
                                // stays empty until the file reads again; it is never guessed.
                                self.queue = crate::queue::Queue { items: queue.clone(), anchored: Vec::new(), blocks: Vec::new() };
                                self.faults.push(f);
                            }
                            None => self.queue = crate::queue::Queue { items: queue.clone(), anchored: Vec::new(), blocks: Vec::new() },
                        }
                    }
                    // The gas estimate is void: it was for this batch, which is gone; keeping it would let
                    // the next "check gas before sending" gate pass on an old reading.
                    self.gas = None;
                    // Not included by the end of this wait: record what may be done now; if included, nothing.
                    if let Done::Anchored { stuck, confirmed, .. } = d {
                        self.stuck = if *confirmed { None } else { stuck.clone() };
                    }
                    // Voided (held by no node, its nonce used by another transaction): the entries are back in
                    // the queue; reported once.
                    if let Done::Anchored { voided: true, tx, .. } = d {
                        self.faults.push(crate::fault::Fault::known(crate::fault::Known::BatchVoided, tx.clone()));
                    }
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
                Ok(Done::Grants { rows, r#gen }) => {
                    if *r#gen == self.grants_gen {
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
                    // The sentinel reads this pass's cards: a revocation and a change of hands each alert once,
                    // and alerted keys are saved in settings.
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
                    // start shows them first.
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
                // The tail of each marked seat home was checked: each is handled like a fetch; a home whose
                // chain could not be read keeps its mark and reports why, without holding up the others.
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
                        // The home at this path is a fresh one now: reopen it, then the fetch lands as any
                        // other (mark, ledger step).
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
                    // The only rule for the pen: the core's label is COMPLETE. The shell does not lean toward
                    // green, and the reverse holds: when the core says incomplete the pen is withdrawn;
                    // "complete last time" is not this time's answer.
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
        // The watch table is recomputed from readings; each notice alerts once.
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
        // The command-line channel: the request in progress advances with what landed, and newly arrived
        // requests are handled in this turn.
        self.door_turn(&got);
        if !self.held_back.is_empty() {
            let mut all = std::mem::take(&mut self.held_back);
            all.extend(got);
            return all;
        }
        got
    }

    /// Receive once and hold (used while the test hooks wait for a passcode task): recorded as usual,
    /// delivered at the next `drain_at`.
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

    /// The chain reading as it stands (the only accessor): `Err` with the fault when the last read failed
    /// (the chain kind keeps no reading over a failure, `landing::Kept::Failed`), else the last reading.
    pub fn chain_reading(&self) -> Result<Option<&Done>, &Fault> {
        match (crate::landing::of(crate::task::Kind::Chain).kept, self.failed.get(&crate::task::Kind::Chain)) {
            (crate::landing::Kept::Failed, Some(f)) => Err(f),
            _ => Ok(self.chain.as_ref()),
        }
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
                (self.machine.backup.as_ref(), self.items_now, self.machine.backup_failed),
                self.audit.as_ref().map(|a| (a.label.as_str(), &a.report, a.unanswered.as_slice())),
                self.chain_now(),
            ),
            crate::roles::Role::Grantee => crate::watchx::grantee(
                self.cards.as_ref().map(|(c, _)| c.as_slice()),
                &self.alarms,
                self.cards.as_ref().and_then(|(_, n)| *n),
                (self.machine.backup.as_ref(), self.items_now, self.machine.backup_failed),
            ),
        }
    }

    /// One watch sweep: compute the table, pick what should alert, drop what already alerted, record, and
    /// hand to the screen. Alerted keys share a record with the sentinel (`settings.alarmed`); a failed save
    /// goes to the fault table (only with a home).
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

    /// The status line's parts. This layer writes no text: the screen builds it from the string table, so the
    /// status line follows the language.
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

/// The primary identity as the shell holds it: id and kind while unlocked. While locked, the store keeps the
/// id sealed and only the kind is read (the lock screen offers words or a key file by it; the id reads empty).
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
