use crate::fault::Fault;
use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::thread::JoinHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Stage {
    pub at: u8,
    pub done: u32,
    pub total: u32,
}

impl Stage {
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

static STAGES: [std::sync::atomic::AtomicU64; Kind::ALL.len()] = [const { std::sync::atomic::AtomicU64::new(0) }; Kind::ALL.len()];
const LIVE: u64 = 1 << 55;

pub fn stage_at(k: Kind, at: u8) {
    STAGES[k as usize].store(Stage { at, done: 0, total: 0 }.pack() | LIVE, std::sync::atomic::Ordering::Relaxed);
}

pub fn count(k: Kind, done: u64, total: u64) {
    let slot = &STAGES[k as usize];
    let cur = Stage::unpack(slot.load(std::sync::atomic::Ordering::Relaxed));
    let s = Stage { at: cur.at, done: done.min(0x0FFF_FFFF) as u32, total: total.min(0x0FFF_FFFF) as u32 };
    slot.store(s.pack() | LIVE, std::sync::atomic::Ordering::Relaxed);
}

pub fn stage(k: Kind) -> Option<Stage> {
    let v = STAGES[k as usize].load(std::sync::atomic::Ordering::Relaxed);
    (v & LIVE != 0).then(|| Stage::unpack(v & !LIVE))
}

static THREAD_FAILS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn pretend_thread_fails(on: bool) {
    THREAD_FAILS.store(on, std::sync::atomic::Ordering::SeqCst);
}

struct Part {
    kind: Kind,
    epoch: u64,
    nodes: u64,
    tx: Sender<Outcome>,
    wake: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
}

thread_local! {
    static PART: std::cell::RefCell<Option<Part>> = const { std::cell::RefCell::new(None) };
}

pub fn land_part(d: Done) -> bool {
    PART.with(|p| {
        let p = p.borrow();
        let Some(p) = p.as_ref() else { return false };
        let sent = p.tx.send(Outcome { kind: p.kind, serial: 0, result: Ok(d), epoch: p.epoch, stale: false, earlier: false, nodes: p.nodes, renoded: false, seen: None, part: true }).is_ok();
        if let Some(w) = &p.wake {
            w();
        }
        sent
    })
}

thread_local! {
    static TICKET: std::cell::RefCell<Option<(u64, std::sync::Arc<std::sync::atomic::AtomicU64>)>> = const { std::cell::RefCell::new(None) };
}

pub fn ticket_void() -> bool {
    TICKET.with(|t| t.borrow().as_ref().map(|(at, now)| *at != now.load(std::sync::atomic::Ordering::SeqCst)).unwrap_or(false))
}

pub fn void_ticket_fault(at: &std::path::Path) -> Fault {
    Fault::known(crate::fault::Known::HomeUnreachable, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::TailTicketVoid, &[&at.display().to_string()])))
}

fn start_thread(kind: Kind, work: impl FnOnce() + Send + 'static) -> std::io::Result<std::thread::JoinHandle<()>> {
    start_named(kind.as_str(), work)
}

pub(crate) fn start_named(name: &str, work: impl FnOnce() + Send + 'static) -> std::io::Result<std::thread::JoinHandle<()>> {
    if THREAD_FAILS.load(std::sync::atomic::Ordering::SeqCst) {
        return Err(std::io::Error::new(std::io::ErrorKind::OutOfMemory, "the system would not start a thread (pretended)"));
    }
    std::thread::Builder::new().name(format!("zikaron-{name}")).spawn(work)
}

fn stage_clear(k: Kind) {
    STAGES[k as usize].store(0, std::sync::atomic::Ordering::Relaxed);
}

pub use crate::kinds::Kind;

pub use zikaron_send::batch::{Offer, Stuck, Unpriced};

#[derive(Clone, Debug)]
pub enum Done {
    Rotation { chains: Vec<crate::rotatex::Chain>, said: Option<String>, unread: Option<crate::fault::Fault>, left: Option<crate::rotatex::LeftRead>, settled: Vec<(String, u64, crate::rotatex::Settled)>, gone: bool, reverted: Option<crate::rotatex::Reverted>, fresh: Vec<(u64, u128)>, withdrawn: bool },
    RotateVetted { to: crate::key::Address },
    UpgradeCost { stage: crate::upgradex::Stage, txs: usize, wei: u128 },
    Path(Option<String>),
    Looked(Box<crate::shell::Looked>),
    Regained { root: std::path::PathBuf, took: crate::lock::Regained },
    Kept(Vec<crate::shell::Kept>),
    CliPath(zikaron_os::cli_path::State),
    Gas { count: usize, gas: u64, calldata: String, fees: zikaron_anchor::send::Fees, fees_left: Vec<String>, head_time: Option<u64>, lane: (u64, Vec<String>) },
    Took { source: crate::anchorx::Source, content: crate::anchorx::Content },
    Hashed { note_md: String, for_: Option<crate::anchorx::For>, files: Vec<(String, Result<crate::anchorx::Content, crate::fault::Fault>)> },
    Copied { old: std::path::PathBuf, root: std::path::PathBuf },
    Seated(Seated),
    Extracted { landed: zikaron_glue::extract::Landed, left: Vec<zikaron_glue::extract::Left> },
    Cleaned { plan: zikaron_seal::exports::Plan, done: bool },
    GatePassed { root: Option<std::path::PathBuf>, then: Box<crate::action::Action>, pass: Box<crate::exitgate::Pass> },
    Issued { root: Option<std::path::PathBuf>, then: Box<crate::action::Action>, member: zikaron_send::issue::Member },
    Withdrew { root: Option<std::path::PathBuf>, entry: String, grant: String, chain: u64, noticed: zikaron_send::withdraw::Noticed },
    BackupMade { path: String, summary: crate::backup::Summary },
    BackupSeen { path: String, summary: crate::backup::Summary },
    Vetted(Vec<(String, Result<Vec<String>, crate::fault::Fault>)>),
    Peeked(Box<(crate::peekx::Ask, crate::peekx::Got)>),
    Check(crate::probe::Report),
    Archive {
        bytes: u64,
        items: usize,
        skipped: usize,
        mirror: crate::mirror::Mirrored,
        records: usize,
        machine_items: Result<u64, crate::fault::Fault>,
        backed: Option<(usize, usize)>,
        mark: u64,
    },
    ChainTime(u64),
    Chain {
        gas_wei: Option<u128>,
        sources: usize,
        single_source: bool,
        unanswered: Vec<zikaron_send::Why>,
        differed: Vec<zikaron_send::Why>,
        head_time: Option<u64>,
    },
    Reconciled { label: String, complete: bool, entries: usize },
    Audited {
        label: String,
        complete: bool,
        broken: bool,
        entries: usize,
        report: zikaron::json::Value,
        unanswered: Vec<String>,
        unproven: Vec<zikaron_send::Why>,
        asked: usize,
        single_source: bool,
        fragment: zikaron::json::Value,
        recounted: bool,
    },
    Ledger {
        rows: Vec<crate::ledgerx::Row>,
        strays: usize,
        handed: Option<String>,
        office: Option<(String, u64, bool)>,
        upgrade: crate::upgradex::Stage,
        upgrade_read: bool,
        seats: std::collections::BTreeMap<String, u64>,
        faces: crate::action::LedgerFaces,
        places: std::collections::BTreeMap<String, zikaron_seal::queue::Place>,
        r#gen: u64,
    },
    Kit { root: std::path::PathBuf, publish: Option<String>, path: String, kit_id: String, entries: usize, files: usize, pulled: Vec<String>, dropped: Vec<String>, originals: Vec<String>, left_out: Vec<String>, unreadable: Vec<(String, crate::fault::Fault)>, withdrawn: Vec<String>, facts: crate::kitsindex::RowFacts },
    Grants { r#gen: u64, rows: Vec<crate::grantx::Row> },
    Book {
        who: String,
        anchors: usize,
        asked: usize,
        entries: usize,
        label: String,
        timeline: Vec<crate::ledgerx::Row>,
        grants: Vec<crate::grantx::Row>,
        latest: Option<u64>,
        from: Option<(crate::supplyx::Level, String)>,
        files: Option<usize>,
        misses: Vec<(crate::supplyx::Level, crate::fault::Fault)>,
        missed: Vec<crate::widex::Missed>,
        pile: Vec<Vec<u8>>,
        audit_key: crate::sendx::behind::Key,
        face: crate::facex::State,
        moves: Vec<crate::facex::Hop>,
    },
    Persona(crate::facex::State, Option<crate::facex::Pair>),
    FaceBooks(crate::facex::Books),
    RerootFee(crate::action::FeeReading),
    FaceJoined(Box<crate::facex::Joined>),
    NetRead { chain_id: u64, registry: crate::key::Address, nodes: Vec<String>, reading: crate::widex::Reading },
    BasisRead { chain: u64, registry: crate::key::Address, from_block: u64, reading: crate::widex::Reading, said: Option<crate::fault::Fault>, after_nodes: bool, asked: crate::action::BasisStamp },
    Submitted {
        tx: String,
        chain: u64,
        url: crate::chainx::NodeAddr,
        ids: Vec<String>,
        gas: Option<u64>,
        queue: crate::queue::Queue,
    },
    Anchored {
        tx: String,
        chain: u64,
        confirmed: bool,
        state: String,
        sent: usize,
        dropped: usize,
        queue: Vec<crate::queue::Queued>,
        gas: Option<u64>,
        calldata: String,
        stuck: Option<Stuck>,
        voided: bool,
        waited: u64,
        kept: Option<zikaron_send::batch::Kept>,
        whole: Option<(Result<crate::queue::Queue, crate::fault::Fault>, u64)>,
    },
    Diligence(Box<crate::diligx::Read>),
    Verified(Box<crate::verifyx::Verified>),
    Delivery(crate::deliveryx::Checked),
    Reviewed { cards: Vec<crate::vaultx::Card>, now: Option<u64> },
    Held { held: Vec<crate::vaultx::Held>, rejected: Vec<crate::verifyx::Rejected> },
    Badge(Box<crate::badgex::Made>),
    Published { url: String, read: crate::fetchx::Published },
    Checked(Box<crate::checkx::Checked>),
    Behind { taken: usize },
    Fetched { root: std::path::PathBuf, landed: usize, entries: usize, tail: crate::restorex::Tail, from: Option<crate::supplyx::Level>, on: Option<crate::restorex::On> },
    TailChecked { checked: Vec<(std::path::PathBuf, Result<crate::restorex::Tail, crate::fault::Fault>, Option<crate::restorex::On>)>, unwritten: bool },
    FetchConflict { root: std::path::PathBuf, offline: usize, fetched: usize, rows: Vec<(crate::ledgerx::Row, Option<u64>)> },
    FetchedAside { aside: std::path::PathBuf, fetched: Box<Done> },
    Keystore(Keystore),
    Vault(Vault),
    FaceCleared { root: Option<std::path::PathBuf>, then: Box<crate::action::Action> },
}

#[derive(Clone, Debug)]
pub enum Vault {
    Opened,
    Changed,
    Recovered,
    Gate(Box<crate::action::Action>),
    Identity { row: crate::identity::Row, restored: bool, fresh: bool, was: Option<(String, crate::roles::Role)> },
    PrimarySet { id: String },
    Restored { summary: crate::backup::Summary },
    RerootMade { id: String },
}

#[derive(Clone, Debug)]
pub enum Keystore {
    BackedUp { path: String, address: crate::key::Address, id: Option<String>, seat: crate::roles::Role },
}

pub struct Outcome {
    pub kind: Kind,
    pub serial: u64,
    pub result: Result<Done, Fault>,
    pub epoch: u64,
    pub stale: bool,
    pub earlier: bool,
    pub nodes: u64,
    pub renoded: bool,
    pub seen: Option<Seen>,
    pub part: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seen {
    pub root: std::path::PathBuf,
    pub entries: usize,
    pub queue: Option<[u8; 32]>,
    pub seq: u64,
}

pub fn next_seen() -> u64 {
    crate::reads::ticket(crate::reads::Copies::Baseline)
}

impl Seen {
    pub fn of(root: &std::path::Path) -> Seen {
        crate::reads::touch();
        let seq = next_seen();
        let home = crate::home::Home::bare(root.to_path_buf());
        let dir = home.dir(crate::home::Slot::Ledger);
        let entries = std::fs::read_dir(&dir)
            .map(|d| d.flatten().filter(|e| zikaron_store::layout::classify(&e.file_name().to_string_lossy()) == zikaron_store::Kind::Entry).count())
            .unwrap_or(0);
        let queue = std::fs::read(home.dir(crate::home::Slot::Settings).join(crate::queue::FILE)).ok().map(|b| zikaron::cryptox::sha256(&b));
        Seen { root: root.to_path_buf(), entries, queue, seq }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spawned {
    Started,
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

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Reaped {
    pub workers: usize,
    pub joined: usize,
    pub left: usize,
    pub flying: Vec<Kind>,
    pub waited: Vec<Kind>,
    pub not_waited: Vec<Kind>,
}

pub struct Tasks {
    tx: Sender<Outcome>,
    rx: Receiver<Outcome>,
    flying: std::collections::BTreeMap<Kind, u64>,
    epoch: u64,
    epoch_now: std::sync::Arc<std::sync::atomic::AtomicU64>,
    nodes: u64,
    flying_nodes: std::collections::BTreeMap<Kind, u64>,
    landed: std::collections::BTreeMap<Kind, f64>,
    landings: std::collections::BTreeMap<Kind, u64>,
    serial: u64,
    flying_serial: std::collections::BTreeMap<Kind, u64>,
    hands: Vec<(Kind, u64, JoinHandle<()>)>,
    carried: Option<zikaron_anchor::action::Action>,
    turnless: std::collections::BTreeSet<Kind>,
    count_root: Option<std::path::PathBuf>,
    waker: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    started: std::collections::BTreeMap<Kind, u64>,
}

impl Default for Tasks {
    fn default() -> Self {
        Tasks::new()
    }
}

impl Tasks {
    pub fn new() -> Tasks {
        zikaron_net::open_up();
        zikaron_net::read_choice(crate::machine::proxy_now);
        let (tx, rx) = std::sync::mpsc::channel();
        Tasks {
            tx,
            rx,
            flying: std::collections::BTreeMap::new(),
            epoch: 0,
            epoch_now: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            nodes: 0,
            flying_nodes: std::collections::BTreeMap::new(),
            landed: std::collections::BTreeMap::new(),
            landings: std::collections::BTreeMap::new(),
            serial: 0,
            flying_serial: std::collections::BTreeMap::new(),
            hands: Vec::new(),
            carried: None,
            turnless: std::collections::BTreeSet::new(),
            count_root: None,
            waker: None,
            started: std::collections::BTreeMap::new(),
        }
    }

    pub fn starts(&self, k: Kind) -> u64 {
        self.started.get(&k).copied().unwrap_or(0)
    }

    pub fn wake_with(&mut self, waker: Option<std::sync::Arc<dyn Fn() + Send + Sync>>) {
        self.waker = waker;
    }

    pub fn polled(&self) -> bool {
        self.flying.keys().any(|k| !k.wakes_when_done())
    }

    pub fn count_under(&mut self, root: Option<std::path::PathBuf>) {
        self.count_root = root;
    }

    pub fn carry(&mut self, action: Option<zikaron_anchor::action::Action>) {
        self.carried = action;
    }

    pub fn in_flight(&self, k: Kind) -> bool {
        match self.flying.get(&k) {
            Some(e) => (!k.follows_source() || *e == self.epoch) && (!k.reads_nodes() || self.flying_nodes.get(&k) == Some(&self.nodes)),
            None => false,
        }
    }

    pub fn started_over(&self, k: Kind) -> Option<u64> {
        self.flying.get(&k).and(self.flying_nodes.get(&k)).copied()
    }

    pub fn flying(&self) -> Vec<Kind> {
        self.flying.keys().copied().collect()
    }

    pub fn holds_turn(&self) -> bool {
        self.flying.keys().any(|k| k.writes_local() && !self.turnless.contains(k))
    }

    pub fn spawn_turnless(&mut self, kind: Kind, work: impl FnOnce() -> Result<Done, Fault> + Send + 'static) -> Spawned {
        self.start(kind, work, true)
    }

    pub fn nodes_moved(&mut self) {
        self.nodes += 1;
        self.landed.retain(|k, _| !k.reads_nodes());
    }

    pub fn new_epoch(&mut self) {
        self.epoch += 1;
        self.epoch_now.store(self.epoch, std::sync::atomic::Ordering::SeqCst);
        self.landed.retain(|k, _| !k.follows_source());
    }

    pub fn attempted(&self, k: Kind) -> bool {
        self.in_flight(k) || self.landed.contains_key(&k)
    }

    pub fn forget(&mut self, k: Kind) {
        self.landed.remove(&k);
    }

    pub fn landed_at(&self, k: Kind) -> Option<f64> {
        self.landed.get(&k).copied()
    }

    pub fn finished_in_flight(&self, k: Kind) -> bool {
        let Some(serial) = self.flying_serial.get(&k) else { return false };
        self.in_flight(k) && self.hands.iter().any(|(x, s, h)| *x == k && s == serial && h.is_finished())
    }

    pub fn serial_of(&self, k: Kind) -> Option<u64> {
        self.flying_serial.get(&k).copied().filter(|_| self.in_flight(k))
    }

    pub fn landings(&self, k: Kind) -> u64 {
        self.landings.get(&k).copied().unwrap_or(0)
    }

    pub fn answered_since(&self, k: Kind, stamp: u64) -> bool {
        self.landings(k) > stamp && !self.in_flight(k)
    }

    pub fn spawn(
        &mut self,
        kind: Kind,
        work: impl FnOnce() -> Result<Done, Fault> + Send + 'static,
    ) -> Spawned {
        self.start(kind, work, false)
    }

    fn start(&mut self, kind: Kind, work: impl FnOnce() -> Result<Done, Fault> + Send + 'static, turnless: bool) -> Spawned {
        if self.in_flight(kind) {
            return Spawned::InFlight;
        }
        self.turnless.remove(&kind);
        if turnless {
            self.turnless.insert(kind);
        }
        self.flying.insert(kind, self.epoch);
        let nodes = self.nodes;
        if kind.reads_nodes() {
            self.flying_nodes.insert(kind, nodes);
        }
        self.serial += 1;
        *self.started.entry(kind).or_insert(0) += 1;
        let serial = self.serial;
        self.flying_serial.insert(kind, serial);
        stage_clear(kind);
        let epoch = self.epoch;
        let tx = self.tx.clone();
        let failed_tx = self.tx.clone();
        let ticket = kind.follows_source().then(|| (epoch, self.epoch_now.clone()));
        let keeper = kind.keeps_reads().then(Kind::keeper).flatten();
        let judged = kind.keeps_notices().then(Kind::keeper).flatten();
        let carried = self.carried.clone().filter(|_| kind.carries());
        let count_at = self.count_root.clone().filter(|_| kind.writes_local() && !self.turnless.contains(&kind));
        let wake = self.waker.clone().filter(|_| kind.wakes_when_done());
        let part = Part { kind, epoch, nodes, tx: self.tx.clone(), wake: self.waker.clone() };
        let started = start_thread(kind, move || {
            TICKET.with(|t| *t.borrow_mut() = ticket);
            PART.with(|p| *p.borrow_mut() = Some(part));
            let result = {
                let _action = match carried {
                    Some(a) => zikaron_anchor::action::enter(a),
                    None => zikaron_anchor::action::open(keeper),
                };
                if let Some(k) = judged {
                    zikaron_anchor::action::keep_judged_in(k);
                }
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(work)) {
                    Ok(r) => r,
                    Err(p) => Err(Fault::known(
                        crate::fault::Known::WorkerPanicked,
                        crate::lang::both_of(|| format!("{} · {}", crate::lang::filln(crate::lang::Key::Tail219, &[&(kind.as_str()).to_string()]), panic_said(p.as_ref()))),
                    )),
                }
            };
            stage_clear(kind);
            let seen = count_at.as_deref().map(Seen::of);
            let _ = tx.send(Outcome { kind, serial, result, epoch, stale: false, earlier: false, nodes, renoded: false, seen, part: false });
            if let Some(w) = wake {
                w();
            }
        });
        match started {
            Ok(h) => self.hands.push((kind, serial, h)),
            Err(e) => {
                stage_clear(kind);
                let f = Fault::unknown(crate::lang::filln(crate::lang::Key::Tail236, &[&crate::lang::filln(crate::lang::Key::Tail219, &[&kind.as_str().to_string()]), &e.to_string()]));
                let _ = failed_tx.send(Outcome { kind, serial, result: Err(f), epoch, stale: false, earlier: false, nodes, renoded: false, seen: None, part: false });
            }
        }
        Spawned::Started
    }

    pub fn drain(&mut self) -> Vec<Outcome> {
        self.drain_at(0.0)
    }

    pub fn drain_at(&mut self, now: f64) -> Vec<Outcome> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(mut o) if o.part => {
                    o.earlier = o.epoch != self.epoch;
                    o.renoded = o.kind.reads_nodes() && o.nodes != self.nodes;
                    o.stale = (o.kind.follows_source() && o.earlier) || o.renoded;
                    out.push(o);
                }
                Ok(mut o) => {
                    let this = !o.kind.reads_nodes() || self.flying_nodes.get(&o.kind) == Some(&o.nodes);
                    if self.flying.get(&o.kind) == Some(&o.epoch) && this {
                        self.flying.remove(&o.kind);
                        self.flying_nodes.remove(&o.kind);
                        self.turnless.remove(&o.kind);
                    }
                    o.earlier = o.epoch != self.epoch;
                    o.renoded = o.kind.reads_nodes() && o.nodes != self.nodes;
                    o.stale = (o.kind.follows_source() && o.earlier) || o.renoded;
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

    pub fn workers(&self) -> usize {
        self.hands.len()
    }

    pub fn shutdown(&mut self) -> Reaped {
        zikaron_net::close_down();
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
                not_waited.push(k);
            }
        }
        self.flying.clear();
        self.flying_nodes.clear();
        Reaped { workers, joined, left: workers - joined, flying, waited, not_waited }
    }
}

fn panic_said(p: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = p.downcast_ref::<&str>() {
        return s.to_string();
    }
    if let Some(s) = p.downcast_ref::<String>() {
        return s.clone();
    }
    crate::lang::t(crate::lang::Key::Tail219Opaque).to_string()
}

#[derive(Clone)]
pub struct Seated(pub std::sync::Arc<std::sync::Mutex<Option<crate::action::Seating>>>);

impl Seated {
    pub fn new(s: crate::action::Seating) -> Seated {
        Seated(std::sync::Arc::new(std::sync::Mutex::new(Some(s))))
    }

    pub fn take(&self) -> Option<crate::action::Seating> {
        self.0.lock().ok().and_then(|mut s| s.take())
    }
}

impl std::fmt::Debug for Seated {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Seated")
    }
}
