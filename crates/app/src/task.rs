//! Background tasks and the Outcome channel: work runs in the background, results come back to the UI thread
//! as messages, the UI frame never blocks, and each kind of task runs at most once at a time.
//!
//! 1. The frame never blocks: results arrive only through `drain`, which only calls `try_recv`. There is no
//! `recv()` or `recv_timeout` in this file, and `join` appears only at shutdown.
//! 2. Single flight: kinds in flight are recorded in `flying`, and `spawn` asks it before starting. There is
//! exactly one place that starts a thread, after that question.
//! 3. Reaping at exit: `shutdown` joins every handle and returns a count; the window's close and the test
//! hooks' `quit` both use it, so an orphaned writer is either reaped or counted.

use crate::fault::Fault;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::thread::JoinHandle;

/// Where a running task is: which of its stages, and how far through a counted stage. The task itself says
/// it (`stage`, `count`) as it passes each step; the window reads it for the words under a long key and the
/// bar in it. One slot per kind (single flight), each a packed atomic, so neither side ever waits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Stage {
    /// The stage index in the task's own list (0 before the first step is said).
    pub at: u8,
    /// Progress through a counted stage (`done` of `total`); zero total means not counted.
    pub done: u32,
    pub total: u32,
}

impl Stage {
    /// The fraction through a counted stage, when it is counted.
    pub fn frac(self) -> Option<f32> {
        (self.total > 0).then(|| (self.done.min(self.total) as f32) / self.total as f32)
    }

    fn pack(self) -> u64 {
        (u64::from(self.at) << 56) | (u64::from(self.done & 0x0FFF_FFFF) << 28) | u64::from(self.total & 0x0FFF_FFFF)
    }

    fn unpack(v: u64) -> Stage {
        Stage { at: (v >> 56) as u8, done: ((v >> 28) & 0x0FFF_FFFF) as u32, total: (v & 0x0FFF_FFFF) as u32 }
    }
}

/// One slot per kind, sized by the closed list of kinds so a new kind always has its slot; the top bit says
/// the slot is live.
static STAGES: [std::sync::atomic::AtomicU64; Kind::ALL.len()] = [const { std::sync::atomic::AtomicU64::new(0) }; Kind::ALL.len()];
const LIVE: u64 = 1 << 55;

/// Say that a task of kind `k` entered stage `at` (called from inside the task).
pub fn stage_at(k: Kind, at: u8) {
    STAGES[k as usize].store(Stage { at, done: 0, total: 0 }.pack() | LIVE, std::sync::atomic::Ordering::Relaxed);
}

/// Say how far a task is through its current stage (called from inside the task).
pub fn count(k: Kind, done: u64, total: u64) {
    let slot = &STAGES[k as usize];
    let cur = Stage::unpack(slot.load(std::sync::atomic::Ordering::Relaxed));
    let s = Stage { at: cur.at, done: done.min(0x0FFF_FFFF) as u32, total: total.min(0x0FFF_FFFF) as u32 };
    slot.store(s.pack() | LIVE, std::sync::atomic::Ordering::Relaxed);
}

/// Where a task of kind `k` is, while it runs and has said anything.
pub fn stage(k: Kind) -> Option<Stage> {
    let v = STAGES[k as usize].load(std::sync::atomic::Ordering::Relaxed);
    (v & LIVE != 0).then(|| Stage::unpack(v & !LIVE))
}

fn stage_clear(k: Kind) {
    STAGES[k as usize].store(0, std::sync::atomic::Ordering::Relaxed);
}

/// Kinds of background work. Closed; single flight is per kind.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Kind {
    /// Self-check: reads the disk.
    SelfCheck,
    /// Measure a home: walk the disk to count bytes, and ask the ledger once.
    Archive,
    /// Ask the chain once: balance and chain time. Network calls never happen in the frame.
    Chain,
    /// Reconcile once: assemble the audit input and have the core write the report.
    Reconcile,
    /// The self-audit clock: scan the chain, assemble the input, the core writes the report.
    Audit,
    /// Read the ledger table: walk the ledger directory and read bytes into rows. The frame never touches the
    /// disk.
    Ledger,
    /// Send a batch of anchors: estimate gas, send, wait for the receipt.
    Anchor,
    /// Depth reading: assemble the audit input, the kit core gives the three measures.
    Depth,
    /// Write a disclosure kit: walk the disk, lay out, self-verify, land.
    Kit,
    /// Read the grant table: walk the ledger and read grants into rows.
    Grants,
    /// Check adoption anchors row by row: ask the chain about those transactions.
    Adopt,
    /// Scan one address: where a new key in a succession came from.
    Sighting,
    /// Read someone else's ledger: scan anchors; with bytes, have the core write a report.
    Book,
    /// Due diligence: scan anchors, have the core give the label and three measures, read the grant and
    /// succession history.
    Diligence,
    /// Verify a record: kit verification, anchor review, depth reading.
    Verify,
    /// Delivery check: read the delivered bytes, compute sha256, compare with the terms hash.
    Delivery,
    /// Periodic vault review.
    Review,
    /// List the vault: walk grants-held and read grants into rows (the frame never touches the disk).
    Held,
    /// Check a payload or document: read the file, scan the chain, have the kit core run the six checks or
    /// the chain check.
    Check,
    /// Write a badge: cascade, encode, self-verify, draw, land.
    Badge,
    /// Encrypt a keystore (key backup): standard scrypt parameters, never in the frame.
    Keystore,
    /// Key derivation for passcode actions: unlock, set and change the passcode, both recoveries, reseal at
    /// the floor, and the unlock that gates the three passcode-protected actions (export key file, delete
    /// identity, show words). Never in the frame.
    Vault,
    /// Check publication: fetch each file of the local kit from the publication address and compare. Network
    /// calls never happen in the frame.
    Publish,
    /// Fetch a ledger and check its tail: fetch this identity's full ledger from four levels, land it in this
    /// home, scan the chain to check the tail.
    Fetch,
    /// Digest attachments first: files and directories dropped into the export page's attachment area are
    /// read and digested in the background, never in the frame.
    Vet,
    /// Write or open a whole-machine backup: scrypt on the backup password and the whole package, never in
    /// the frame.
    Backup,
    /// The exit gate for the exports that write in the frame (a grant file, a record bundle): read the chain
    /// in the background; when it passes, the export itself runs where the result lands.
    Gate,
}

impl Kind {
    /// Whether quitting waits for this kind to finish. Exhaustive: a new kind has to say.
    pub fn waited_at_quit(self) -> bool {
        match self {
            // What a cut would leave half written: a key file, a backup, an export folder, a badge, the key
            // store's own pass, a reconciliation's report.
            Kind::Keystore | Kind::Backup | Kind::Kit | Kind::Badge | Kind::Vault | Kind::Reconcile => true,
            // The network's (a node answers when it answers; each of these lands whole or not at all, and what
            // it leaves on disk is laid out to be picked up again), and the ones that only read.
            Kind::Chain
            | Kind::Audit
            | Kind::Anchor
            | Kind::Fetch
            | Kind::Review
            | Kind::Adopt
            | Kind::Sighting
            | Kind::Book
            | Kind::Diligence
            | Kind::Verify
            | Kind::Check
            | Kind::Publish
            | Kind::SelfCheck
            | Kind::Archive
            | Kind::Ledger
            | Kind::Depth
            | Kind::Grants
            | Kind::Delivery
            | Kind::Held
            | Kind::Vet
            | Kind::Gate => false,
        }
    }

    /// Whether this kind writes sealed local data, from its own thread or where it lands in the frame (the
    /// ledger, the queue, the last audit, review verdicts, the register's backup marks). A master key change
    /// and the completion of a lock wait for these; the others only read, or write exports. Exhaustive: a new
    /// kind has to say.
    pub fn writes_local(self) -> bool {
        match self {
            Kind::Fetch | Kind::Audit | Kind::Review | Kind::Anchor | Kind::Reconcile | Kind::Keystore => true,
            Kind::SelfCheck
            | Kind::Archive
            | Kind::Chain
            | Kind::Ledger
            | Kind::Depth
            | Kind::Kit
            | Kind::Grants
            | Kind::Adopt
            | Kind::Sighting
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
            | Kind::Backup
            | Kind::Gate => false,
        }
    }

    pub const ALL: [Kind; 27] = [
        Kind::SelfCheck,
        Kind::Archive,
        Kind::Chain,
        Kind::Reconcile,
        Kind::Audit,
        Kind::Ledger,
        Kind::Anchor,
        Kind::Depth,
        Kind::Kit,
        Kind::Grants,
        Kind::Adopt,
        Kind::Sighting,
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
        Kind::Backup,
        Kind::Gate,
    ];

    /// Whether this kind's readings follow the ledger's source. Closed: a new kind must be answered here.
    /// Those that follow (reading this home's ledger, settings, vault) are void when the source changes and
    /// may start again for the new source; those that do not (anchoring, kit and file output, reading outside
    /// files) stay single-flight across epochs and deliver normally.
    pub fn follows_source(self) -> bool {
        match self {
            Kind::Archive
            | Kind::Chain
            | Kind::Reconcile
            | Kind::Audit
            | Kind::Ledger
            | Kind::Depth
            | Kind::Grants
            | Kind::Adopt
            | Kind::Sighting
            | Kind::Review
            | Kind::Held
            | Kind::Gate => true,
            Kind::SelfCheck | Kind::Anchor | Kind::Kit | Kind::Book | Kind::Diligence | Kind::Verify | Kind::Delivery | Kind::Badge | Kind::Check | Kind::Keystore | Kind::Vault | Kind::Publish | Kind::Fetch | Kind::Vet | Kind::Backup => false,
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
            Kind::Depth => "depth",
            Kind::Kit => "kit",
            Kind::Grants => "grants",
            Kind::Adopt => "adopt",
            Kind::Sighting => "sighting",
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
            Kind::Backup => "backup",
            Kind::Gate => "gate",
        }
    }
}

/// What a task returns. Closed: the background returns only these, and the screen renders by kind.
#[derive(Clone, Debug)]
pub enum Done {
    /// The exit gate passed for this export, read for the home at `root`; the export itself runs where this
    /// lands.
    GatePassed { root: Option<std::path::PathBuf>, then: Box<crate::action::Action> },
    /// A whole-machine backup was written and read back.
    BackupMade { path: String, summary: crate::backup::Summary },
    /// A backup was opened with its password (nothing here changed).
    BackupSeen { summary: crate::backup::Summary },
    /// Attachment digests: each path as given, with its content-form hex32 or a named refusal.
    Vetted(Vec<(String, Result<String, crate::fault::Fault>)>),
    Check(crate::probe::Report),
    /// A home reading: bytes used, ledger entries, stray files.
    Archive {
        bytes: u64,
        items: usize,
        skipped: usize,
        /// What the last written bundle looks like now (four states).
        mirror: crate::mirror::Mirrored,
        /// Records (`history` entries) in this home's ledger.
        records: usize,
        /// Ledger entries and held grants on the whole machine (the "after the last backup" count).
        machine_items: Option<u64>,
    },
    /// Chain readings. `None` means "not read", which the screen keeps apart from "read as zero".
    Chain {
        gas_wei: Option<u128>,
        sources: usize,
        single_source: bool,
        /// Chain time fetched along the way (the latest block's time); `None` when unavailable, which does
        /// not block the balance.
        head_time: Option<u64>,
    },
    /// One reconciliation: the core's label unchanged, and how many entries it covered.
    Reconciled { label: String, complete: bool, entries: usize },
    /// One self-audit: the report brought back unchanged; the screen lays out its fifteen items.
    Audited {
        label: String,
        complete: bool,
        broken: bool,
        entries: usize,
        /// The core's report, unchanged.
        report: zikaron::json::Value,
        /// Which endpoints did not answer this pass.
        unanswered: Vec<String>,
        asked: usize,
        single_source: bool,
        /// The fragment this pass used, unchanged (depth readings must use the same basis).
        fragment: zikaron::json::Value,
    },
    /// A ledger table, with who writes this ledger from now on: once a succession is in the ledger, this desk
    /// has handed it over.
    Ledger {
        rows: Vec<crate::ledgerx::Row>,
        strays: usize,
        handed: Option<String>,
        /// The table generation when this pass started (`Shell::rows_gen`); if it changed by the time the
        /// result lands, the reading came from an older source and is not accepted.
        gen: u64,
    },
    /// One depth reading (the kit core's three measures).
    Depth { work: String, value: zikaron::json::Value },
    /// A kit was written. `root` and `publish` are the home and publication base when the task started (the
    /// index row records them, not the home open when it lands). Attachments from the interface that did not
    /// go in (paths as given): `left_out` were no longer originals of the chosen records when the kit was
    /// written; `unreadable` could not be read then (missing, no permission, symlink, device). Each is
    /// reported separately.
    Kit { root: std::path::PathBuf, publish: Option<String>, path: String, kit_id: String, entries: usize, files: usize, pulled: Vec<String>, dropped: Vec<String>, left_out: Vec<String>, unreadable: Vec<(String, crate::fault::Fault)> },
    /// A grant table.
    Grants { gen: u64, rows: Vec<crate::grantx::Row> },
    /// Checked adoption rows, with the text they were checked against (the results count only for that text).
    Adopt { proofs: Vec<crate::adoptx::Proof>, rows: String },
    /// The anchors a key sent, each with the answers to three questions (`address` empty means this key).
    KeyAnchors { address: String, rows: Vec<crate::adoptx::KeyAnchor> },
    /// A claim as read, with each anchor's block (empty when not asked).
    Claim { claim: crate::adoptx::Claim, blocks: Vec<Option<u64>> },
    /// The scan of a new key.
    Sighting { to: String, anchors: usize, asked: usize },
    /// Someone else's ledger.
    Book {
        who: String,
        anchors: usize,
        asked: usize,
        entries: usize,
        label: String,
        timeline: Vec<crate::ledgerx::Row>,
        grants: Vec<crate::grantx::Row>,
        /// Block time of the latest anchor (`readerx::Book::latest`).
        latest: Option<u64>,
        /// Which level the ledger bytes came from (`supplyx::find_book`, four levels) and where; `None` when
        /// no level had them.
        from: Option<(crate::supplyx::Level, String)>,
        /// How many files the publication level fetched by the manifest; other levels have none.
        files: Option<usize>,
        /// Levels that could not be read along the way, each named.
        misses: Vec<(crate::supplyx::Level, crate::fault::Fault)>,
    },
    /// Broadcast, echo matched: those entries are recorded as submitted in the queue file; the shell starts
    /// the receipt wait when it receives this (`action::confirm_batch`).
    Submitted {
        tx: String,
        chain: u64,
        url: String,
        ids: Vec<String>,
        gas: Option<u64>,
        queue: crate::queue::Queue,
    },
    /// A batch of anchors was sent. The transaction hash is always present: the bytes were broadcast and must
    /// be traceable.
    Anchored {
        tx: String,
        chain: u64,
        /// Whether the transaction succeeded (law §9.1: a status other than 1 is no anchor in either form).
        confirmed: bool,
        /// What the receipt says, unchanged.
        state: String,
        /// How many were sent in this batch and dequeued.
        sent: usize,
        dropped: usize,
        /// The queue table on disk after dequeuing, unchanged. The shell's copy has no other source (see
        /// `queue::settle`).
        queue: Vec<crate::queue::Queued>,
        /// Gas estimated by the node before sending.
        gas: Option<u64>,
        /// The call data of the transaction actually sent, read back from the chain by its hash (never
        /// rebuilt here). Empty when it cannot be read, and the screen then says "not read".
        calldata: String,
    },
    /// One due-diligence pass: all four panels.
    Diligence(Box<crate::diligx::Read>),
    /// One record verification.
    Verified(Box<crate::verifyx::Verified>),
    /// One delivery check.
    Delivery(crate::deliveryx::Checked),
    /// A six-check review of the whole vault (periodic).
    Reviewed { cards: Vec<crate::vaultx::Card>, now: Option<u64> },
    /// What the vault holds.
    Held { held: Vec<crate::vaultx::Held>, rejected: Vec<crate::verifyx::Rejected> },
    /// A badge.
    Badge(Box<crate::badgex::Made>),
    /// The publication check reading: which address, how many files, which are missing, which differ.
    Published { url: String, read: crate::fetchx::Published },
    /// One grant check.
    Checked(Box<crate::checkx::Checked>),
    /// Fetched a ledger and checked its tail: entries landed, entries in this home now, the tail check's
    /// answer, which level the bytes came from.
    Fetched { root: std::path::PathBuf, landed: usize, entries: usize, tail: crate::restorex::Tail, from: Option<crate::supplyx::Level> },
    /// Fetching found the fetched ledger and this home's at odds; nothing landed. `offline`: this home's
    /// entries the fetched ledger lacks (they would stay where this home is set aside).
    FetchConflict { root: std::path::PathBuf, offline: usize, fetched: usize, rows: Vec<(crate::ledgerx::Row, Option<u64>)> },
    /// This seat's home was set aside (`aside`) and the fetched ledger landed in a fresh one (`fetched`, a
    /// `Fetched`).
    FetchedAside { aside: std::path::PathBuf, fetched: Box<Done> },
    /// A keystore was encrypted (the key backup landed).
    Keystore(Keystore),
    /// Key derivation for a passcode action finished: the shell continues with the frame half when it
    /// receives this.
    Vault(Vault),
}

/// The shapes a passcode task returns. Closed: where results are received, each continues (see
/// `action::vault_landed`).
#[derive(Clone, Debug)]
pub enum Vault {
    /// Opened (unlock, set passcode, reseal at the floor).
    Opened,
    /// The passcode changed.
    Changed,
    /// Recovered.
    Recovered,
    /// The gating unlock passed: continue with this action (its passcode field already wiped).
    Gate(Box<crate::action::Action>),
    /// A new or imported identity landed in the registry and the vault (derivation and recovery sealing
    /// happen in the background). `restored`: it was already in the registry (a missing slot was restored);
    /// `fresh`: built new (the words in hand are wiped on landing).
    Identity { row: crate::identity::Row, restored: bool, fresh: bool },
    /// Another identity became primary (a new master key took effect).
    PrimarySet { id: String },
    /// Restored from a whole-machine backup (a new master key took effect).
    Restored { summary: crate::backup::Summary },
}

/// The shapes a keystore task returns.
#[derive(Clone, Debug)]
pub enum Keystore {
    /// This seat's key was backed up to this file; `id` is the registry identity (`None` without a registry),
    /// and on landing a "backed up" mark is recorded.
    BackedUp { path: String, address: crate::key::Address, id: Option<String>, seat: crate::roles::Role },
}

/// The result of a task. The only shape that returns to the UI thread.
pub struct Outcome {
    pub kind: Kind,
    pub result: Result<Done, Fault>,
    /// The epoch of the ledger's source when this task started (see `Tasks::new_epoch`).
    pub epoch: u64,
    /// This kind follows the source, and the source changed after it started: what it read does not belong to
    /// the current home (decided when received).
    pub stale: bool,
}

/// Why a task was not started.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spawned {
    /// Started.
    Started,
    /// A task of this kind is in flight; not started.
    InFlight,
}

impl Spawned {
    pub fn as_str(self) -> &'static str {
        match self {
            Spawned::Started => "started",
            Spawned::InFlight => "in_flight",
        }
    }
}

/// The reaping count.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Reaped {
    pub workers: usize,
    pub joined: usize,
    pub left: usize,
    /// The kinds in flight when quitting (one per thread still running), those waited for, and those left to
    /// end with the process (`Kind::waited_at_quit`).
    pub flying: Vec<Kind>,
    pub waited: Vec<Kind>,
    pub not_waited: Vec<Kind>,
}

pub struct Tasks {
    tx: Sender<Outcome>,
    rx: Receiver<Outcome>,
    /// Every kind in flight, with the epoch it started in.
    flying: std::collections::BTreeMap<Kind, u64>,
    /// The epoch of the ledger's source. A source change (another home, adoption in place, mirror restore)
    /// moves to a new epoch: tasks from the previous epoch are void when they land, their "started" marks are
    /// cleared, and every reading starts again for the new source. One counter replaces clearing at every
    /// call site.
    epoch: u64,
    /// When each kind last landed, success or failure.
    ///
    /// Inferring "has it run" from results (no table yet means never ran) made a failing ledger read start
    /// again every frame: unbounded threads, an unbounded fault table, full CPU and nothing on screen. The
    /// self-audit clock was the same: a failed pass did not advance the clock, so a down endpoint was
    /// redialed forever. What is recorded is "started", not "succeeded", which removes that class.
    landed: std::collections::BTreeMap<Kind, f64>,
    /// How many times each kind has landed in this shell (success or failure; never cleared, never
    /// decreasing): the stamp a press takes to tell a later answer from one already there.
    landings: std::collections::BTreeMap<Kind, u64>,
    /// Each started task's serial (counting up) and, per kind in flight, the serial of the one in flight: which
    /// worker is "the one in flight" is told by its serial, never guessed from an earlier worker of the kind.
    serial: u64,
    flying_serial: std::collections::BTreeMap<Kind, u64>,
    hands: Vec<(Kind, u64, JoinHandle<()>)>,
}

impl Default for Tasks {
    fn default() -> Self {
        Tasks::new()
    }
}

impl Tasks {
    pub fn new() -> Tasks {
        let (tx, rx) = std::sync::mpsc::channel();
        Tasks {
            tx,
            rx,
            flying: std::collections::BTreeMap::new(),
            epoch: 0,
            landed: std::collections::BTreeMap::new(),
            landings: std::collections::BTreeMap::new(),
            serial: 0,
            flying_serial: std::collections::BTreeMap::new(),
            hands: Vec::new(),
        }
    }

    /// Whether this kind is in flight. The source of single flight.
    pub fn in_flight(&self, k: Kind) -> bool {
        match self.flying.get(&k) {
            Some(e) => !k.follows_source() || *e == self.epoch,
            None => false,
        }
    }

    pub fn flying(&self) -> Vec<Kind> {
        self.flying.keys().copied().collect()
    }

    /// Move to a new epoch. Kinds that follow the source: tasks in flight from the old epoch run to
    /// completion but deliver only their trouble, and their "started" marks are cleared. Kinds that do not
    /// (anchoring, kit output) stay single-flight and deliver normally.
    pub fn new_epoch(&mut self) {
        self.epoch += 1;
        self.landed.retain(|k, _| !k.follows_source());
    }

    /// Whether this kind has started (success or failure). In flight counts as started.
    pub fn attempted(&self, k: Kind) -> bool {
        self.in_flight(k) || self.landed.contains_key(&k)
    }

    /// Clear this kind's "started" mark. Used only when a reading is deliberately invalidated (see
    /// `Shell::stale_rows`), so that kind starts again; otherwise the page would never read a second time.
    pub fn forget(&mut self, k: Kind) {
        self.landed.remove(&k);
    }

    /// When this kind last landed (success or failure); `None` when it never started.
    pub fn landed_at(&self, k: Kind) -> Option<f64> {
        self.landed.get(&k).copied()
    }

    /// Whether the worker of this kind now in flight has finished. A worker sends its outcome before it ends,
    /// so asked before a drain: when it is true and the kind is still in flight after that drain, the outcome
    /// never reached this side (lost), and whoever waits for it stops by name instead of waiting on the clock.
    pub fn finished_in_flight(&self, k: Kind) -> bool {
        let Some(serial) = self.flying_serial.get(&k) else { return false };
        self.in_flight(k) && self.hands.iter().any(|(x, s, h)| *x == k && s == serial && h.is_finished())
    }

    /// How many times this kind has landed (the stamp for [`Tasks::answered_since`]).
    pub fn landings(&self, k: Kind) -> u64 {
        self.landings.get(&k).copied().unwrap_or(0)
    }

    /// Whether what was asked of this kind at a press has answered: it has landed since the press (more
    /// landings than the stamp taken then) and none of it is in flight. Judged by the landing itself, never
    /// by the clock or by "not in flight" alone (a press's own frame has not started the task yet).
    pub fn answered_since(&self, k: Kind, stamp: u64) -> bool {
        self.landings(k) > stamp && !self.in_flight(k)
    }

    /// Start a task. With one of its kind in flight, it is not started, and says so: no queueing, no silent
    /// drop.
    pub fn spawn(
        &mut self,
        kind: Kind,
        work: impl FnOnce() -> Result<Done, Fault> + Send + 'static,
    ) -> Spawned {
        if self.in_flight(kind) {
            return Spawned::InFlight;
        }
        self.flying.insert(kind, self.epoch);
        self.serial += 1;
        let serial = self.serial;
        self.flying_serial.insert(kind, serial);
        stage_clear(kind);
        let epoch = self.epoch;
        let tx = self.tx.clone();
        let h = std::thread::spawn(move || {
            // A crash still sends a message. When a worker panicked, its sender went with the thread and the
            // kind stayed in `flying` forever: every later request answered "in flight", the screen said "not
            // run yet", and the fault table was empty. The panic is caught here and becomes a named `Fault`,
            // so every exit sends.
            let result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)) {
                Ok(r) => r,
                // Bring back what the panic said; without it the screen would only say it crashed.
                Err(p) => Err(Fault::known(
                    crate::fault::Known::WorkerPanicked,
                    format!("{} · {}", crate::lang::filln(crate::lang::Key::Tail219, &[&(kind.as_str()).to_string()]), panic_said(p.as_ref())),
                )),
            };
            stage_clear(kind);
            // Sending fails only when the UI side is gone, and then nobody wants the result.
            let _ = tx.send(Outcome { kind, result, epoch, stale: false });
        });
        self.hands.push((kind, serial, h));
        Spawned::Started
    }

    /// Receive. Only `try_recv`: take what is there this frame and move on, never wait.
    pub fn drain(&mut self) -> Vec<Outcome> {
        self.drain_at(0.0)
    }

    /// As above, with the landing time given by the caller (the interface clock).
    pub fn drain_at(&mut self, now: f64) -> Vec<Outcome> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(mut o) => {
                    if self.flying.get(&o.kind) == Some(&o.epoch) {
                        self.flying.remove(&o.kind);
                    }
                    o.stale = o.kind.follows_source() && o.epoch != self.epoch;
                    // Success or failure is recorded (see the `landed` field). Results from a previous epoch
                    // do not count as started in this one.
                    if !o.stale {
                        self.landed.insert(o.kind, now);
                        *self.landings.entry(o.kind).or_insert(0) += 1;
                    }
                    out.push(o);
                }
                Err(TryRecvError::Empty) | Err(TryRecvError::Disconnected) => break,
            }
        }
        self.hands.retain(|(_, _, h)| !h.is_finished());
        out
    }

    /// How many handles are not yet reaped.
    pub fn workers(&self) -> usize {
        self.hands.len()
    }

    /// Reap at shutdown. Only the threads a cut would leave half written are joined (`Kind::waited_at_quit`);
    /// the others (the network's, which can take as long as a node does, and the ones that only read) are left
    /// to end with the process, so quitting never waits on the network. The window's `on_exit` and the test
    /// hooks' `quit` both come here.
    pub fn shutdown(&mut self) -> Reaped {
        let hands: Vec<(Kind, JoinHandle<()>)> = std::mem::take(&mut self.hands).into_iter().filter(|(_, _, h)| !h.is_finished()).map(|(k, _, h)| (k, h)).collect();
        let workers = hands.len();
        let flying: Vec<Kind> = hands.iter().map(|(k, _)| *k).collect();
        let (mut joined, mut waited, mut not_waited) = (0, Vec::new(), Vec::new());
        for (k, h) in hands {
            if k.waited_at_quit() {
                waited.push(k);
                if h.join().is_ok() {
                    joined += 1;
                }
            } else {
                // Left running: it ends with the process (dropping the handle detaches it).
                not_waited.push(k);
            }
        }
        self.flying.clear();
        Reaped { workers, joined, left: workers - joined, flying, waited, not_waited }
    }
}

/// What a panic said. `panic!` carries `&str` or `String`; any other payload has no readable text and is
/// reported as unrecognized.
fn panic_said(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        return s.to_string();
    }
    if let Some(s) = p.downcast_ref::<String>() {
        return s.clone();
    }
    crate::lang::t(crate::lang::Key::Tail219Opaque).to_string()
}
