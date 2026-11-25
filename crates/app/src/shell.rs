use crate::queue::Kept as _;
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

#[derive(Clone, Debug)]
pub struct AuditRead {
    pub label: String,
    pub complete: bool,
    pub broken: bool,
    pub entries: usize,
    pub report: zikaron::json::Value,
    pub unanswered: Vec<String>,
    pub unproven: Vec<crate::chainx::Why>,
    pub asked: usize,
    pub single_source: bool,
    pub fragment: zikaron::json::Value,
    pub at: f64,
    pub mark: u64,
    pub seen: Option<usize>,
    pub rows_gen: u64,
    pub nodes: Vec<crate::chainx::Endpoint>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CliAloneNow {
    Off,
    Keyless,
    Keyed(usize),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Banner {
    None,
    Broken,
    ReadOnly(String),
    OtherMachine,
    MarkUnread,
    Handed(String),
    HandedOff,
    Unfetched(crate::restorex::State),
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
            Banner::HandedOff => "handed_off",
            Banner::Unfetched(_) => "unfetched",
        }
    }
}

pub use crate::nav::Page;

fn chain_answered(d: &Done) -> bool {
    match d {
        Done::Chain { .. } | Done::Audited { .. } | Done::Book { .. } => true,
        Done::Anchored { confirmed, .. } => *confirmed,
        Done::Diligence(_) => true,
        Done::Verified(v) => v.review.is_ok(),
        Done::Reviewed { now, .. } => now.is_some(),
        Done::Checked(c) => c.basis.is_ok() || c.now_from == crate::checkx::NowFrom::Chain,
        _ => false,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Vault {
    Read(crate::keybox::State),
    Damaged(Fault),
}

impl Vault {
    pub fn of(read: Result<crate::keybox::State, Fault>) -> Vault {
        match read {
            Ok(s) => Vault::Read(s),
            Err(f) => Vault::Damaged(f),
        }
    }

    pub fn gate_up(&self) -> bool {
        match self {
            Vault::Read(s) => s.gate_up(),
            Vault::Damaged(_) => true,
        }
    }

    pub fn keys_ready(&self) -> bool {
        match self {
            Vault::Read(s) => s.keys_ready(),
            Vault::Damaged(_) => false,
        }
    }

    pub fn absent(&self) -> bool {
        matches!(self, Vault::Read(crate::keybox::State::Absent))
    }

    pub fn is(&self, s: crate::keybox::State) -> bool {
        matches!(self, Vault::Read(x) if *x == s)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Vault::Read(s) => s.as_str(),
            Vault::Damaged(_) => "damaged",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Conflict {
    pub root: std::path::PathBuf,
    pub offline: usize,
    pub fetched: usize,
    pub rows: Vec<(crate::ledgerx::Row, Option<u64>)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OldData {
    pub path: std::path::PathBuf,
    pub at: u64,
    pub entries: usize,
    pub queued: usize,
}

pub struct Hydrated {
    pub settings: Settings,
    pub endpoints: Vec<Endpoint>,
    pub seen_entries: usize,
    pub seen_queue: Option<[u8; 32]>,
    pub seen_seq: u64,
    pub queue: crate::queue::Queue,
    pub rooted: bool,
    pub remembered: Option<crate::lastread::Anchored>,
    pub verdicts: Vec<(String, crate::lastread::Verdict)>,
    pub kits_root: Option<String>,
    pub kits_index: Option<Vec<crate::kitsindex::Row>>,
    pub read_nets: Option<Vec<crate::readnets::Net>>,
    pub unfetched: Option<crate::restorex::State>,
    pub handed_off: Option<String>,
    pub pens: Option<[u8; 32]>,
    pub aside: Vec<OldData>,
    pub troubles: Vec<Fault>,
}

impl Hydrated {
    pub fn read(home: &crate::home::Home) -> Result<Hydrated, Fault> {
        let mut troubles = Vec::new();
        let settings = Settings::read(home)?;
        let endpoints = crate::chainx::nodes_of(&settings);
        let seen_seq = crate::task::next_seen();
        let seen_entries = entry_files(home);
        let seen_queue = queue_mark(home);
        let queue = match crate::queue::Queue::read(home) {
            Ok(q) => q,
            Err(f) => {
                troubles.push(f);
                crate::queue::Queue::default()
            }
        };
        let rooted = crate::ledgerx::rooted(home);
        let remembered = match crate::lastread::load_anchored(home) {
            Ok(a) => {
                let chain = settings.chain_id;
                a.map(|mut x| {
                    x.rows.retain(|(_, (c, _))| Some(*c) == chain);
                    x
                })
            }
            Err(f) => {
                troubles.push(f);
                None
            }
        };
        let (verdicts, bad) = crate::lastread::load_verdicts(home);
        troubles.extend(bad);
        let (kits_root, kits_index, read_nets, more) = kits_now(Some(home));
        troubles.extend(more);
        let unfetched = match crate::restorex::standing(home) {
            Ok(s) => s,
            Err(f) => {
                troubles.push(f);
                Some(crate::restorex::State::Unfetched)
            }
        };
        let pens = pens_mark();
        let handed_off = handed_off(home, &mut troubles);
        Ok(Hydrated { settings, endpoints, seen_entries, seen_queue, seen_seq, queue, rooted, remembered, verdicts, kits_root, kits_index, read_nets, unfetched, handed_off, pens, aside: aside_now(), troubles })
    }
}

fn aside_now() -> Vec<OldData> {
    let room = crate::home::machine_dir().map(|m| m.join(crate::local::ASIDE_HOMES)).ok();
    let machine = crate::machine::read().unwrap_or_default();
    machine
        .homes
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
        .collect()
}

fn kits_now(home: Option<&crate::home::Home>) -> (Option<String>, Option<Vec<crate::kitsindex::Row>>, Option<Vec<crate::readnets::Net>>, Vec<Fault>) {
    let mut troubles = Vec::new();
    let root = home.and_then(|h| crate::ledgerx::root_of(h).ok());
    let index = match crate::home::machine_dir().and_then(|m| crate::kitsindex::read(&m)) {
        Ok(rows) => Some(rows.unwrap_or_default()),
        Err(f) => {
            troubles.push(f);
            None
        }
    };
    let nets = match crate::action::read_nets_now() {
        Ok(n) => Some(n),
        Err(f) => {
            troubles.push(f);
            None
        }
    };
    (root, index, nets, troubles)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OldView {
    pub back: std::path::PathBuf,
    pub at: u64,
}

pub fn build_kind() -> &'static str {
    if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ArchiveRead {
    pub bytes: u64,
    pub items: usize,
    pub skipped: usize,
    pub mirror: crate::mirror::Mirrored,
    pub records: usize,
    pub backed: Option<(usize, usize)>,
    pub mark: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    Ledger,
    Home,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reread {
    Wrote,
    Noticed,
    WroteGrant,
    Issued,
    Files,
    Landed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stale {
    Rows,
    Grants,
}

impl crate::reads::Cause for Reread {
    type Reading = Stale;
    const ALL: &'static [Reread] = &[Reread::Wrote, Reread::Noticed, Reread::WroteGrant, Reread::Issued, Reread::Files, Reread::Landed];
    fn stales(self) -> &'static [Stale] {
        match self {
            Reread::Wrote | Reread::Files | Reread::Landed => &[Stale::Rows],
            Reread::WroteGrant => &[Stale::Grants, Stale::Rows],
            Reread::Issued | Reread::Noticed => &[Stale::Grants],
        }
    }
}

struct GatesShown {
    root: std::path::PathBuf,
    role: crate::roles::Role,
    moved: u64,
    said: Result<(), Fault>,
}

pub struct Shell {
    pub page: Page,
    pub tasks: Tasks,
    pub fonts: fonts::Found,
    pub font_missing: Vec<fonts::Role>,
    pub last: Option<Report>,
    pub archive: Option<ArchiveRead>,
    pub chain: Option<crate::task::Done>,
    pub reconciled: Option<(String, bool, usize)>,
    pub pen: Pen,
    pub endpoints: Vec<Endpoint>,
    pub documents: String,
    pub resume_blocked: Option<u64>,
    pub faults: Vec<Fault>,
    pub skinned: bool,
    pub anchor: Option<Address>,
    pub anchor_public: Option<[u8; 33]>,
    pub identities: Option<crate::identity::Registry>,
    pub new_words: Option<crate::identity::Fresh>,
    pub new_label: Option<String>,
    pub words: Option<Vec<crate::secret::Secret>>,
    pub home: Option<Home>,
    pub lock: Option<Lock>,
    pub settings: Settings,
    pub moved: u64,
    gates_shown: std::sync::Mutex<Option<GatesShown>>,
    pub rows: Option<(Vec<crate::ledgerx::Row>, usize)>,
    pub rows_gen: u64,
    pub rows_at: u64,
    pub faces_read: crate::reads::Cell<(u64, Option<std::path::PathBuf>), crate::action::LedgerFaces>,
    pub seat_chains: std::collections::BTreeMap<String, u64>,
    pub places_read: crate::reads::Cell<(u64, u64, Option<std::path::PathBuf>), std::collections::BTreeMap<String, zikaron_seal::queue::Place>>,
    pub rows_wanted: std::sync::atomic::AtomicBool,
    pub grants_gen: u64,
    pub grants_at: u64,
    pub ledger_ok: Option<bool>,
    pub ledger_handed_on: Option<usize>,
    pub vault: Vault,
    pub vault_recoverable: bool,
    pub vault_said: Option<crate::action::Applied>,
    pub gate_said: Option<crate::action::Applied>,
    pub said: std::collections::BTreeMap<crate::task::Kind, crate::action::Applied>,
    pub machine: crate::machine::Machine,
    held_back: Vec<Outcome>,
    pub audit: Option<AuditRead>,
    pub upgrade: Option<crate::upgradex::Stage>,
    pub upgrade_read: bool,
    pub rotation: Option<Vec<crate::rotatex::Chain>>,
    pub rotation_said: Option<String>,
    pub rot_left: Option<Vec<crate::rotatex::Left>>,
    pub rot_left_of: Option<crate::key::Address>,
    pub rot_vetted: Option<crate::key::Address>,
    pub rot_given: Option<crate::key::Address>,
    pub rot_written: Option<(String, String)>,
    pub rot_sending: Option<u64>,
    pub rot_home: Option<std::path::PathBuf>,
    pub rot_sweep_out: Option<(u64, u128)>,
    pub rot_sweep_held: Option<(u64, u128, String)>,
    pub rot_short: Option<crate::rotatex::Short>,
    pub rot_fresh: Vec<(u64, u128)>,
    pub rot_sweep_to: Option<crate::key::Address>,
    pub rot_reverted: Option<crate::rotatex::Reverted>,
    pub rot_looking: bool,
    pub rot_unread: Option<Fault>,
    pub upgrade_cost: Option<(crate::upgradex::Stage, usize, u128)>,
    pub office_trouble: Option<Fault>,
    pub book_mark: u64,
    pub audit_asked: Option<u64>,
    pub audit_lag_since: Option<(f64, std::collections::BTreeSet<String>)>,
    pub tail_asked: Option<u64>,
    pub tail_wanted: bool,
    pub tail_again: bool,
    pub fetch_checks_tail: bool,
    pub unwritten_asked: Vec<std::path::PathBuf>,
    pub queue: crate::queue::Queue,
    pub sent: Option<crate::task::Done>,
    pub repo_since: Option<(String, Option<usize>)>,
    pub content: Option<crate::anchorx::Content>,
    pub flow: crate::anchorx::Flow,
    pub grants: Option<Vec<crate::grantx::Row>>,
    pub kit: Option<crate::task::Done>,
    pub picked: Option<(usize, Vec<String>)>,
    pub clash: Vec<crate::grantx::Row>,
    pub clash_unsettled: usize,
    pub noticed_at: u64,
    pub handed: Option<String>,
    pub book: Option<crate::task::Done>,
    pub persona: crate::facex::State,
    pub face_books: Option<crate::facex::Books>,
    pub face_pair: Option<crate::facex::Pair>,
    pub reroot_fee: Option<crate::action::FeeReading>,
    pub face_joined: Option<crate::facex::Joined>,
    pub story: Option<(String, Option<crate::grantx::Row>, Vec<(String, Option<String>)>)>,
    pub backup_seen: Option<crate::identity::BackupSeen>,
    pub gas: Option<(usize, u64)>,
    pub gas_lane: Option<(u64, Vec<String>)>,
    pub door_followed: bool,
    pub fees: Option<zikaron_anchor::send::Fees>,
    pub fees_left: Vec<String>,
    pub stuck: Option<crate::task::Stuck>,
    pub gas_epoch: u64,
    pub gas_asked: Option<u64>,
    pub send_backoff: Vec<std::time::Duration>,
    pub receipt_backoff: Vec<std::time::Duration>,
    pub anchor_wait: std::time::Duration,
    pub resumed_at: f64,
    pub remembered: Option<crate::lastread::Anchored>,
    pub verdicts: Vec<(String, crate::lastread::Verdict)>,
    pub unfetched: Option<crate::restorex::State>,
    pub handed_off: Option<String>,
    pub seen_pens: Option<[u8; 32]>,
    pub fetch_conflict: Option<Conflict>,
    pub aside: Vec<OldData>,
    pub old_view: Option<OldView>,
    pub last_aside: Option<std::path::PathBuf>,
    pub vetted: std::collections::BTreeMap<String, Result<Vec<String>, Fault>>,
    pub peeks: Vec<(crate::peekx::Ask, crate::peekx::Got)>,
    pub keeps: Vec<Keep>,
    pub keep_inline: bool,
    pub kits_index: Option<Vec<crate::kitsindex::Row>>,
    pub read_nets: Option<Vec<crate::readnets::Net>>,
    pub net_reads: Vec<(u64, crate::key::Address, crate::widex::Reading)>,
    pub basis_read: Option<(u64, crate::key::Address, Option<crate::widex::Reading>)>,
    pub kits_root: Option<String>,
    pub clock: fn() -> u64,
    pub diligence: Option<crate::diligx::Read>,
    pub others_asked: Option<crate::action::OthersAsk>,
    pub others_whole: std::sync::Arc<std::sync::Mutex<Option<(crate::sendx::behind::Key, crate::diligx::Read)>>>,
    pub verified: Option<crate::verifyx::Verified>,
    pub cleaned: Option<(zikaron_seal::exports::Plan, bool)>,
    pub delivery: Option<crate::deliveryx::Checked>,
    pub cards: Option<(Vec<crate::vaultx::Card>, Option<u64>)>,
    pub rooted: bool,
    pub held: Option<Vec<crate::vaultx::Held>>,
    pub held_rejected: Vec<crate::verifyx::Rejected>,
    pub held_dirty: bool,
    pub vault_changed: bool,
    pub alarms: Vec<crate::sentinelx::Alarm>,
    pub rung: Vec<crate::sentinelx::Alarm>,
    pub attention: bool,
    pub badge: Option<crate::badgex::Made>,
    pub checked: Option<crate::checkx::Checked>,
    pub check_asked: Option<crate::action::CheckAsk>,
    pub audits: std::sync::Arc<crate::checkx::Audits>,
    pub published: Option<(String, crate::fetchx::Published)>,
    pub notices: Vec<crate::watchx::Notice>,
    pub fresh: Vec<crate::watchx::Notice>,
    pub status: Option<(crate::lang::Key, String)>,
    pub failed: std::collections::BTreeMap<crate::task::Kind, Fault>,
    pub chain_read_at: Option<f64>,
    pub chain_time: Option<u64>,
    pub primary: Option<(String, crate::keybox::PrimaryKind)>,
    pub rekeying: bool,
    pub swapping: bool,
    pub seat_wish: Option<crate::roles::Role>,
    pub identity_wish: Option<String>,
    pub lock_pending: bool,
    pub backup_made: Option<(String, crate::backup::Summary)>,
    pub backup_peek: Option<(String, crate::backup::Summary)>,
    pub items_now: Option<u64>,
    pub door: Option<crate::door::Door>,
    pub door_doing: Option<crate::door::Doing>,
    pub door_acting: bool,
    pub door_told: Vec<crate::action::Applied>,
    pub door_waker: Option<crate::door::Waker>,
    pub door_shut: Option<(std::path::PathBuf, Fault)>,
    pub door_too_long: bool,
    pub door_place: Option<(std::path::PathBuf, std::path::PathBuf)>,
    pub door_off: bool,
    pub door_answered: usize,
    pub cli_path: Option<zikaron_os::cli_path::State>,
    pub ai_config: Option<String>,
    pub credential: Option<zikaron_seal::credential::Term>,
    pub passes: Vec<zikaron_seal::credential::Row>,
    pub seen_entries: Option<usize>,
    pub seen_queue: Option<Option<[u8; 32]>>,
    pub followed_at: f64,
    pub seen_gen: u64,
    pub copies: crate::reads::Held,
    pub regain_stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub looks_dropped: u64,
    pub archive_mark: Option<u64>,
    pub landed: Option<(String, u64)>,
    pub followed: bool,
}

fn documents_folder() -> String {
    let home = crate::platform::home_dir();
    let docs = home.as_ref().map(|h| h.join("Documents")).filter(|d| d.is_dir());
    docs.or(home).map(|p| p.display().to_string()).unwrap_or_default()
}

impl Shell {
    pub fn boot(dressed: zikaron_ui::skin::Dressed) -> Shell {
        let found = dressed.found;
        let missing = dressed.missing;

        trace::open();
        crate::chainx::speak();
        let _ = crate::chainx::keys_file();
        let mut faults: Vec<Fault> = Vec::new();
        if let Some(f) = trace::trouble() {
            faults.push(f);
        }
        let machine = match crate::machine::read_face() {
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
            documents: documents_folder(),
            page: Default::default(),
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
            anchor_public: None,
            identities: None,
            new_words: None,
            new_label: None,
            words: None,
            home: None,
            lock: None,
            settings: Settings::default(),
            moved: 0,
            gates_shown: std::sync::Mutex::new(None),
            rows: None,
            rows_gen: 0,
            rows_at: 0,
            faces_read: crate::reads::Cell::new(),
            seat_chains: std::collections::BTreeMap::new(),
            places_read: crate::reads::Cell::new(),
            rows_wanted: std::sync::atomic::AtomicBool::new(false),
            grants_gen: 0,
            grants_at: 0,
            ledger_ok: None,
            ledger_handed_on: None,
            vault: Vault::of(crate::keybox::state()),
            vault_recoverable: crate::keybox::recoverable().unwrap_or(true),
            vault_said: None,
            said: std::collections::BTreeMap::new(),
            gate_said: None,
            machine,
            held_back: Vec::new(),
            audit: None,
            upgrade: None,
            upgrade_read: false,
            rotation: None,
            rotation_said: None,
            rot_left: None,
            rot_left_of: None,
            rot_vetted: None,
            rot_given: None,
            rot_written: None,
            rot_sending: None,
            rot_home: None,
            rot_sweep_out: None,
            rot_sweep_held: None,
            rot_short: None,
            rot_fresh: Vec::new(),
            rot_sweep_to: None,
            rot_reverted: None,
            rot_looking: false,
            rot_unread: None,
            upgrade_cost: None,
            office_trouble: None,
            book_mark: 0,
            audit_asked: None,
            audit_lag_since: None,
            tail_asked: None,
            tail_wanted: false,
            tail_again: false,
            fetch_checks_tail: false,
            queue: crate::queue::Queue::default(),
            sent: None,
            repo_since: None,
            content: None,
            flow: crate::anchorx::Flow::default(),
            gas: None,
            gas_lane: None,
            door_followed: false,
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
            kit: None,
            picked: None,
            clash: Vec::new(),
            clash_unsettled: 0,
            noticed_at: 0,
            handed: None,
            book: None,
            persona: crate::facex::State::Unread,
            face_books: None,
            face_pair: None,
            reroot_fee: None,
            face_joined: None,
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
            handed_off: None,
            seen_pens: None,
            fetch_conflict: None,
            aside: Vec::new(),
            old_view: None,
            last_aside: None,
            vetted: std::collections::BTreeMap::new(),
            peeks: Vec::new(),
            keeps: Vec::new(),
            keep_inline: false,
            unwritten_asked: Vec::new(),
            diligence: None,
            others_asked: None,
            others_whole: Default::default(),
            verified: None,
            cleaned: None,
            delivery: None,
            cards: None,
            held_rejected: Vec::new(),
            held_dirty: false,
            vault_changed: false,
            rooted: false,
            held: None,
            alarms: Vec::new(),
            rung: Vec::new(),
            attention: false,
            badge: None,
            checked: None,
            check_asked: None,
            audits: std::sync::Arc::new(crate::checkx::Audits::new(Some(std::sync::Arc::new(crate::checkx::AuditStore)))),
            published: None,
            notices: Vec::new(),
            fresh: Vec::new(),
            status: None,
            failed: std::collections::BTreeMap::new(),
            chain_read_at: None,
            chain_time: None,
            primary: primary_reading(),
            rekeying: false,
            swapping: false,
            seat_wish: None,
            identity_wish: None,
            lock_pending: false,
            backup_made: None,
            backup_peek: None,
            items_now: None,
            door: None,
            door_doing: None,
            door_acting: false,
            door_told: Vec::new(),
            door_waker: None,
            door_shut: None,
            door_too_long: false,
            door_place: None,
            door_off: false,
            door_answered: 0,
            cli_path: None,
            ai_config: None,
            credential: None,
            passes: Vec::new(),
            seen_entries: None,
            seen_queue: None,
            followed_at: f64::NEG_INFINITY,
            seen_gen: 0,
            copies: crate::reads::Held::default(),
            regain_stop: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            looks_dropped: 0,
            archive_mark: None,
            landed: None,
            followed: false,
        }
    }

    pub fn held_anchored_at(&self, id: &str) -> Option<u64> {
        let card = self.cards.as_ref().and_then(|(cs, _)| cs.iter().find(|c| c.id.eq_ignore_ascii_case(id)).and_then(|c| c.anchored_at));
        card.or_else(|| self.verdicts.iter().find(|(g, _)| g.eq_ignore_ascii_case(&crate::lastread::grant_form(id))).and_then(|(_, v)| v.anchored_at))
    }

    pub fn refresh_anchor(&mut self) -> Result<bool, Fault> {
        if !self.unlocked() {
            self.anchor = None;
            self.anchor_public = None;
            return Ok(false);
        }
        match crate::key::load(crate::register::account_now()?.as_deref())? {
            Some(s) => {
                self.anchor = s.address();
                self.anchor_public = s.public();
                Ok(true)
            }
            None => {
                self.anchor = None;
                self.anchor_public = None;
                Ok(false)
            }
        }
    }

    pub fn after_unlock(&mut self) {
        self.lock_pending = false;
        self.reread_vault();
        if let Some(o) = crate::local::take_opened() {
            self.faults.extend(o.troubles);
        }
        let opened_now = self.home.is_none() && self.unlocked();
        if self.unlocked() {
            match crate::machine::read() {
                Ok(m) => {
                    if m.backup != self.machine.backup {
                        self.items_now = None;
                    }
                    self.machine = m;
                }
                Err(f) => self.faults.push(f),
            }
            self.reread_credential();
        }
        if opened_now {
            let _ = crate::action::boot_home(self);
            if let Some(l) = self.settings.lang {
                crate::lang::set(l);
            }
            crate::when::set(crate::when::chosen(self.settings.zone));
        }
        if let Err(f) = self.refresh_anchor() {
            self.faults.push(f);
        }
        match crate::register::view() {
            Ok(v) => {
                let v = match crate::action::words_settle(&v) {
                    Ok(settled) => settled.unwrap_or(v),
                    Err(f) => {
                        self.faults.push(f);
                        v
                    }
                };
                self.seat_identities(Some(v))
            }
            Err(f) => self.faults.push(f),
        }
        if opened_now {
            let _ = crate::action::apply(self, crate::action::Action::CatchUp);
        }
    }

    pub fn reread_credential(&mut self) {
        use crate::keybox::Credential;
        match crate::keybox::credential_now(crate::lastread::now_secs()) {
            Ok(Credential::Live(st)) => {
                self.credential = Some(st.term);
                self.passes = st.rows;
            }
            Ok(Credential::Expired) => crate::action::cli_alone_ended(self, Known::CredentialExpired),
            Ok(Credential::Unformed) => crate::action::cli_alone_ended(self, Known::CredentialReclaimed),
            Ok(Credential::None) => {
                self.credential = None;
                self.passes.clear();
                if crate::keybox::take_credential_dropped() {
                    crate::action::alone_ended(self, Known::CliAloneDropped, true);
                } else if self.machine.cli_alone.is_some() {
                    crate::action::cli_alone_ended(self, Known::CredentialMissing);
                }
            }
            Err(f) => self.faults.push(f),
        }
    }

    pub fn cli_alone_now(&self) -> CliAloneNow {
        match (self.machine.cli_alone.is_some() && self.credential.is_some(), self.passes.len()) {
            (false, _) => CliAloneNow::Off,
            (true, 0) => CliAloneNow::Keyless,
            (true, n) => CliAloneNow::Keyed(n),
        }
    }

    pub fn take_turn(&mut self) -> Result<(), Fault> {
        if self.machine.cli_alone.is_none() {
            return Ok(());
        }
        let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) else { return Ok(()) };
        let home = crate::home::Home::bare(root.clone());
        let Some(lock) = self.lock.as_mut() else { return Ok(()) };
        if !lock.mode().writable() || lock.held() {
            return Ok(());
        }
        if !lock.regain(&home)? {
            return Err(match lock.mode() {
                crate::lock::Mode::Writer => Fault::known(Known::CliWriting, root.display().to_string()),
                _ => Fault::known(Known::ReadOnly, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail006, &[&lock.holder().to_string()]))),
            });
        }
        self.tasks.count_under(Some(root));
        if self.seen_entries != Some(entry_files(&home)) {
            self.reread_home_files(&home);
        } else if self.seen_queue != Some(queue_mark(&home)) {
            self.reread_queue(&home);
        }
        Ok(())
    }

    fn reread_home_files(&mut self, home: &crate::home::Home) {
        self.reread(Reread::Files);
        self.book_changed();
        self.count_here();
        self.tail_again |= self.unfetched.is_some();
        self.rooted = crate::ledgerx::rooted(home);
        self.reread_queue(home);
        match Settings::read(home) {
            Ok(s) => self.settings = s,
            Err(f) => self.faults.push(f),
        }
    }

    fn reread_queue(&mut self, home: &crate::home::Home) {
        self.copies.raise(crate::reads::Copies::Baseline, crate::task::next_seen());
        self.seen_queue = Some(queue_mark(home));
        self.seen_gen = self.seen_gen.wrapping_add(1);
        match crate::queue::Queue::read(home) {
            Ok(q) => self.queue = q,
            Err(f) => self.faults.push(f),
        }
        self.reread(Reread::Files);
    }

    pub fn turn_sync(&mut self) {
        let alone = self.machine.cli_alone.is_some();
        let writing = self.tasks.holds_turn();
        let root = self.home.as_ref().map(|h| h.root().to_path_buf());
        let writable = self.lock.as_ref().is_some_and(|l| l.mode().writable());
        let (Some(root), true) = (root, writable) else {
            self.tasks.count_under(None);
            return;
        };
        if alone {
            self.regain_stop.store(true, std::sync::atomic::Ordering::SeqCst);
            if let Some(lock) = self.lock.as_mut().filter(|l| l.held() && !writing) {
                lock.release();
            }
        } else if !self.lock.as_ref().is_some_and(|l| l.held()) && !self.tasks.in_flight(crate::task::Kind::Regain) {
            self.regain_stop.store(false, std::sync::atomic::Ordering::SeqCst);
            if let Some(hand) = self.lock.as_ref().map(crate::lock::Lock::hand) {
                let (stop, at) = (self.regain_stop.clone(), root.clone());
                let _ = crate::action::behind(self, crate::task::Kind::Regain, move || crate::lock::regain_behind(&hand, &at, &stop).map(|r| Done::Regained { root: at.clone(), took: r }));
            }
        }
        let held = self.lock.as_ref().is_some_and(|l| l.held());
        self.tasks.count_under(held.then_some(root));
    }

    pub fn credential_tick(&mut self) {
        if self.unlocked() && self.credential.is_some_and(|t| t.expired(crate::lastread::now_secs())) {
            crate::action::cli_alone_ended(self, Known::CredentialExpired);
        }
    }

    pub fn begin_lock(&mut self) {
        self.lock_pending = true;
        crate::keybox::begin_lock();
        self.reread_vault();
        self.anchor = None;
        self.anchor_public = None;
        self.words = None;
        self.new_words = None;
    }

    pub fn peeked(&self, path: &str) -> Option<&crate::backup::Summary> {
        self.backup_peek.as_ref().filter(|(p, _)| p.trim() == path.trim()).map(|(_, s)| s)
    }

    pub fn after_lock(&mut self) {
        self.reread_vault();
        self.backup_peek = None;
        self.anchor = None;
        self.anchor_public = None;
        self.words = None;
        self.new_words = None;
        self.unload();
    }

    pub fn unload(&mut self) {
        self.close_home();
        self.seat_identities(None);
        self.settings = Settings { role: self.settings.role, ..Settings::default() };
        self.endpoints = Vec::new();
        self.kits_index = None;
        self.kits_root = None;
        self.vetted.clear();
        self.peeks.clear();
        self.content = None;
        self.book = None;
        self.story = None;
        self.diligence = None;
        self.verified = None;
        self.delivery = None;
        self.checked = None;
        self.kit = None;
        self.picked = None;
        self.clash.clear();
        self.clash_unsettled = 0;
        self.badge = None;
        self.published = None;
        self.new_label = None;
        self.remembered = None;
        self.verdicts.clear();
        self.unfetched = None;
        self.handed_off = None;
        self.cards = None;
        self.held = None;
        self.held_rejected.clear();
        self.grants = None;
        self.rows = None;
        self.seat_chains.clear();
        self.audit = None;
        self.ledger_ok = None;
        self.ledger_handed_on = None;
        self.reconciled = None;
        self.handed = None;
    }

    pub fn reread_vault(&mut self) {
        let v = Vault::of(crate::keybox::state());
        if let Vault::Damaged(f) = &v {
            self.faults.push(f.clone());
        }
        self.vault = v;
        self.vault_recoverable = crate::keybox::recoverable().unwrap_or(true);
        self.primary = primary_reading();
    }

    pub fn seat_identities(&mut self, reg: Option<crate::identity::Registry>) {
        self.backup_seen = reg
            .as_ref()
            .and_then(|r| r.now())
            .map(|(row, _)| crate::identity::backup_seen(row));
        self.identities = reg;
        self.primary = primary_reading();
    }

    pub fn peek_landed(&self, ask: &crate::peekx::Ask) -> Option<&crate::peekx::Got> {
        self.peeks.iter().find(|(a, _)| a == ask).map(|(_, g)| g)
    }

    pub fn identity_listed(&self) -> bool {
        self.identities.as_ref().and_then(|r| r.now()).is_some()
    }

    pub fn seat_unseated(&self) -> bool {
        self.identities
            .as_ref()
            .and_then(|r| r.now())
            .map(|(row, _)| row.address(self.settings.role).is_none())
            .unwrap_or(false)
    }

    pub fn seal_rows(&mut self) {
        use crate::ledgerx::Lamp;
        let crate::facex::State::Read(b) = &self.persona else { return };
        if zikaron_glue::facegate::sealed(&b.table).is_ok() {
            return;
        }
        let seated = zikaron_glue::facegate::seated_ids(&b.table);
        let Some((rows, _)) = self.rows.as_mut() else { return };
        let mut held: Vec<String> = Vec::new();
        for r in rows.iter_mut().filter(|r| r.kind == zikaron_glue::book::Kind::Face && !seated.contains(&r.id.to_ascii_lowercase())) {
            if matches!(r.lamp, Lamp::Landed | Lamp::Queued | Lamp::Refused | Lamp::Reverted) {
                r.lamp = Lamp::Sealed;
                held.push(r.id.clone());
            }
        }
        let queued: Vec<String> = held.into_iter().filter(|id| self.queue.items.iter().any(|q| q.id.eq_ignore_ascii_case(id) && !q.step.in_flight())).collect();
        if queued.is_empty() || !self.lock.as_ref().is_some_and(|l| l.mode().writable()) {
            return;
        }
        if let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) {
            self.leave(Keep::Seal { root, ids: queued });
        }
    }

    pub fn close_home(&mut self) {
        self.source_changed(Source::Home);
        self.home = None;
        self.persona = crate::facex::State::Unread;
        self.face_books = None;
        self.face_pair = None;
        self.reroot_fee = None;
        self.lock = None;
        self.queue = crate::queue::Queue::default();
        self.rooted = false;
        self.door_sync();
    }

    pub fn speaks(&self) -> Option<crate::lang::Lang> {
        self.settings.lang.or(self.machine.lang)
    }

    pub fn sendable_now(&self) -> usize {
        self.next_batch(usize::MAX).len()
    }

    pub fn sendable_reading(&self) -> bool {
        self.next_lane(usize::MAX).is_none() && !self.queue.items.is_empty() && !self.placed_now() && !self.failed.contains_key(&crate::task::Kind::Ledger)
    }

    fn placed_now(&self) -> bool {
        let rows = self.rows.as_ref().map(|(r, _)| r.as_slice());
        let placed = crate::queue::place_in(rows, &self.seat_chains);
        self.queue.items.iter().all(|q| placed(&q.id).is_some()) || self.places_read.get(&(self.book_mark, self.rows_gen, self.home.as_ref().map(|h| h.root().to_path_buf()))).is_some()
    }

    pub fn sendable_here(&self) -> usize {
        self.next_batch_here(usize::MAX).len()
    }

    pub fn next_batch_here(&self, n: usize) -> Vec<String> {
        self.next_lane_here(n).map(|(_, ids)| ids).unwrap_or_default()
    }

    pub fn next_lane_here(&self, n: usize) -> Option<(u64, Vec<String>)> {
        if let Some(lane) = self.next_lane(n) {
            return Some(lane);
        }
        let key = (self.book_mark, self.rows_gen, self.home.as_ref().map(|h| h.root().to_path_buf()));
        self.places_read.get_or(key, || {
            let pile = self.home.as_ref().and_then(|h| h.ledger().ok()).and_then(|l| l.pile().ok()).map(|p| p.items).unwrap_or_default();
            zikaron_seal::queue::places_of(&pile)
        });
        self.next_lane(n)
    }

    pub fn next_batch(&self, n: usize) -> Vec<String> {
        self.next_lane(n).map(|(_, ids)| ids).unwrap_or_default()
    }

    pub fn next_lane(&self, n: usize) -> Option<(u64, Vec<String>)> {
        let rows = self.rows.as_ref().map(|(r, _)| r.as_slice());
        let placed = crate::queue::place_in(rows, &self.seat_chains);
        let anchoring = |(c, ids): (u64, Vec<String>)| (if c == 0 { self.settings.chain_id.unwrap_or(0) } else { c }, ids);
        if self.queue.items.iter().all(|q| placed(&q.id).is_some()) {
            return self.queue.next_batch(n, placed).map(anchoring);
        }
        let key = (self.book_mark, self.rows_gen, self.home.as_ref().map(|h| h.root().to_path_buf()));
        let Some(places) = self.places_read.get(&key) else {
            self.rows_wanted.store(true, std::sync::atomic::Ordering::Relaxed);
            return None;
        };
        self.queue.next_batch(n, |id| places.get(&id.to_ascii_lowercase()).copied()).map(anchoring)
    }

    pub fn carried_on(&self, id: &str) -> Option<Option<u64>> {
        let id = id.to_ascii_lowercase();
        if let Some((rows, _)) = self.rows.as_ref() {
            if let Some(r) = rows.iter().find(|r| r.id.eq_ignore_ascii_case(&id)) {
                return Some((r.kind == zikaron_glue::book::Kind::Face).then(|| self.seat_chains.get(&id).copied()).flatten());
            }
        }
        let key = (self.book_mark, self.rows_gen, self.home.as_ref().map(|h| h.root().to_path_buf()));
        self.places_read.get(&key).and_then(|p| p.get(&id).copied()).map(|p| p.carried_on)
    }

    pub fn basis_chains(&self) -> Vec<(u64, crate::key::Address, u64)> {
        let (Some(chain), Some(registry)) = (self.settings.chain_id, self.settings.registry) else { return Vec::new() };
        let mut out = vec![(chain, registry, self.settings.from_block)];
        for e in &self.settings.earlier {
            if !out.iter().any(|(c, _, _)| *c == e.chain_id) {
                out.push((e.chain_id, crate::key::Address(e.registry), e.from_block));
            }
        }
        out
    }

    pub fn rest_key_tail(&self, base: &str) -> Option<String> {
        self.machine.rest_keys.tail(&zikaron_anchor::rest::service_key(base)?)
    }

    pub fn rest_key_said(&self, base: &str) -> String {
        use crate::lang::{fill1, t, Key};
        match self.rest_key_tail(base) {
            Some(k) => fill1(Key::NetRestKeyTail, &k),
            None if zikaron_anchor::rest::service_key(base).is_some_and(|s| zikaron_anchor::rest::file_gives(&s)) => t(Key::NetRestKeyFromFile).to_string(),
            None => t(Key::NetRestNoKey).to_string(),
        }
    }

    pub fn lanes(&self) -> Vec<crate::chainx::Lane> {
        self.basis_chains()
            .into_iter()
            .map(|(chain, registry, from_block)| crate::chainx::Lane {
                chain,
                registry: registry.0,
                urls: self.endpoints.iter().filter(|e| e.chain == chain).map(|e| e.url.clone()).collect(),
                from_block,
            })
            .collect()
    }

    pub fn unlocked(&self) -> bool {
        self.vault.keys_ready()
    }

    pub fn may_save_settings(&self) -> Result<(), Fault> {
        if self.home.is_none() {
            return Err(Fault::known(Known::NoHome, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail211).to_string())));
        }
        if !self.settings_writable() {
            return Err(Fault::known(
                Known::ReadOnly,
                crate::lang::both_of(|| match self.lock.as_ref() {
                    Some(l) => crate::lang::filln(crate::lang::Key::Tail006, &[&(l.holder()).to_string()]),
                    None => crate::lang::t(crate::lang::Key::Tail212).to_string(),
                }),
            ));
        }
        Ok(())
    }

    pub fn save_settings(&self) -> Result<(), Fault> {
        self.may_save_settings()?;
        let h = self.home.as_ref().expect("上一句已经问过家在不在");
        self.settings.write(h)
    }

    pub fn commit_settings(&mut self, f: impl FnOnce(&mut Settings)) -> Result<(), Fault> {
        self.may_save_settings()?;
        let mut next = self.settings.clone();
        f(&mut next);
        let h = self.home.as_ref().expect("上一句已经问过家在不在");
        next.write(h)?;
        let moved = gas_source(&self.settings) != gas_source(&next);
        let nodes_moved = crate::chainx::nodes_of(&self.settings) != crate::chainx::nodes_of(&next);
        let chain_moved = crate::restorex::On::of(&self.settings) != crate::restorex::On::of(&next);
        self.settings = next;
        if chain_moved {
            self.retake_mark();
        }
        if moved {
            self.gas_void();
        }
        if nodes_moved {
            crate::chainx::forget_serving();
        }
        Ok(())
    }

    pub fn gas_void(&mut self) {
        self.gas = None;
        self.gas_lane = None;
        self.fees = None;
        self.fees_left.clear();
        self.gas_epoch += 1;
    }

    pub fn gas_out_of_date(&self) -> bool {
        self.gas_asked != Some(self.gas_epoch)
    }

    pub fn broken(&self) -> bool {
        self.reconciled
            .as_ref()
            .map(|(l, _, _)| l == zikaron::v2::tokens::Label::BrokenChain.as_str())
            .unwrap_or(false)
    }

    pub fn banner(&self) -> Banner {
        if let Some(to) = self.handed.as_ref() {
            return Banner::Handed(to.clone());
        }
        if self.broken() {
            return Banner::Broken;
        }
        if self.home.is_some() && self.handed_off.is_some() {
            return Banner::HandedOff;
        }
        if self.home.is_some() && self.lock.as_ref().map(|l| l.mode() == crate::lock::Mode::OtherMachine).unwrap_or(false) {
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
        if let (true, Some(s)) = (self.home.is_some(), self.unfetched.as_ref()) {
            return Banner::Unfetched(s.clone());
        }
        Banner::None
    }

    pub fn book_changed(&mut self) {
        self.book_mark = self.book_mark.wrapping_add(1);
    }

    pub fn count_here(&mut self) {
        let Some(h) = self.home.as_ref() else { return };
        let seen = crate::task::Seen::of(h.root());
        self.rooted = seen.entries > 0 && (self.rooted || crate::ledgerx::rooted(h));
        self.seen_entries = Some(seen.entries);
        self.seen_queue = Some(seen.queue);
        self.copies.raise(crate::reads::Copies::Baseline, seen.seq);
        self.seen_gen = self.seen_gen.wrapping_add(1);
    }

    pub fn leave(&mut self, k: Keep) {
        self.keeps.push(k);
    }

    pub fn keep_turn(&mut self) {
        if self.keep_inline || self.keeps.is_empty() || self.tasks.in_flight(crate::task::Kind::Keep) {
            return;
        }
        let all = std::mem::take(&mut self.keeps);
        let hand = self.lock.as_ref().map(crate::lock::Lock::hand);
        let writer = self.settings_writable();
        let _ = crate::action::behind(self, crate::task::Kind::Keep, move || Ok(Done::Kept(keep(all, hand, writer))));
    }

    pub fn kept(&mut self, all: &[Kept]) {
        use crate::reads::Copies;
        let here = |shell: &Shell, root: &std::path::Path| shell.home.as_ref().is_some_and(|h| h.root() == root);
        for k in all {
            match k {
                Kept::Machine(Ok(m), at) => {
                    if self.copies.takes(Copies::Machine, *at) {
                        self.machine = m.clone();
                    } else {
                        self.leave(Keep::Machine);
                    }
                }
                Kept::Machine(Err(f), _) | Kept::Wrote(Err(f)) | Kept::Settings { settings: Err(f), .. } => self.faults.push(f.clone()),
                Kept::Wrote(Ok(())) => {}
                Kept::Settings { root, settings, at } => {
                    if here(self, root) {
                        self.take_settings(root, settings, *at);
                    }
                }
                Kept::Kits { open, root, index, nets, troubles } => {
                    if self.home.as_ref().map(|h| h.root().to_path_buf()) == *open {
                        self.kits_root = root.clone();
                        self.kits_index = index.clone();
                        self.read_nets = nets.clone();
                    }
                    self.faults.extend(troubles.iter().cloned());
                }
                Kept::Register(Ok(v), at) => {
                    if self.copies.takes(Copies::Register, *at) {
                        self.seat_identities(Some(v.clone()));
                    } else {
                        self.leave(Keep::Register);
                    }
                }
                Kept::Register(Err(f), _) => self.faults.push(f.clone()),
                Kept::Queue { root, queue, at } => {
                    if here(self, root) {
                        self.take_queue(root, queue, *at);
                        self.reread(Reread::Landed);
                    }
                }
                Kept::Followed { root, queue: Some(Ok(q)), owed, at } => {
                    if here(self, root) && self.take_queue(root, &Ok(q.clone()), *at) {
                        if let Some((rows, _)) = self.rows.as_mut() {
                            for r in rows.iter_mut().filter(|r| owed.iter().any(|id| id == &r.id)) {
                                r.lamp = crate::ledgerx::lamp_of(&r.id, false, q);
                            }
                        }
                    }
                }
                Kept::Followed { queue: Some(Err(f)), .. } => self.faults.push(f.clone()),
                Kept::Followed { queue: None, .. } => {}
                Kept::Sealed { root, queue, held, at } => match queue {
                    Ok(q) if here(self, root) => {
                        self.take_queue(root, &Ok(q.clone()), *at);
                        self.faults.push(Fault::known(crate::fault::Known::FaceSealed, format!("held {held}")));
                    }
                    Ok(_) => {}
                    Err(f) => self.faults.push(f.clone()),
                },
                Kept::Tail { root, mark } => match mark {
                    Ok(s) if here(self, root) => {
                        self.unfetched = s.clone();
                        if s.is_none() && self.retake_mark() {
                            if let Some(h) = self.home.as_ref() {
                                if let Err(f) = crate::restorex::forget_on(h) {
                                    self.faults.push(f);
                                }
                            }
                        }
                        self.reread(Reread::Landed);
                    }
                    Ok(_) => {}
                    Err(f) => self.faults.push(f.clone()),
                },
            }
        }
    }

    pub fn follow_disk(&mut self, now: f64) {
        if now - self.followed_at < FOLLOW_EVERY || self.swapping || !self.unlocked() || self.tasks.in_flight(crate::task::Kind::Look) {
            return;
        }
        let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) else { return };
        self.followed_at = now;
        let base = Base { r#gen: self.seen_gen, entries: self.seen_entries, queue: self.seen_queue, pens: self.seen_pens, rooted: self.rooted };
        let _ = crate::action::behind(self, crate::task::Kind::Look, move || Ok(Done::Looked(Box::new(look(&root, base)))));
    }

    fn take_queue(&mut self, root: &std::path::Path, q: &Result<crate::queue::Queue, Fault>, at: u64) -> bool {
        match q {
            Ok(q) if self.copies.takes(crate::reads::Copies::Queue, at) => {
                self.queue = q.clone();
                true
            }
            Ok(_) => {
                self.leave(Keep::Queue { root: root.to_path_buf() });
                false
            }
            Err(e) => {
                self.faults.push(e.clone());
                false
            }
        }
    }

    fn take_settings(&mut self, root: &std::path::Path, st: &Result<Settings, Fault>, at: u64) {
        match st {
            Ok(st) if self.copies.takes(crate::reads::Copies::Settings, at) => self.settings = st.clone(),
            Ok(_) => self.leave(Keep::Settings { root: root.to_path_buf() }),
            Err(e) => self.faults.push(e.clone()),
        }
    }

    pub fn looked(&mut self, l: &Looked) -> bool {
        let here = self.home.as_ref().is_some_and(|h| h.root() == l.root.as_path());
        if l.base.r#gen != self.seen_gen || !here || !self.unlocked() || self.swapping {
            self.looks_dropped += 1;
            return false;
        }
        let mut moved = false;
        if let Some((handed, troubles)) = &l.handed {
            self.seen_pens = l.pens;
            self.handed_off = handed.clone();
            self.faults.extend(troubles.iter().cloned());
            moved = true;
        }
        if let (Some(seen), Some(f)) = (l.base.entries, l.files.as_ref()) {
            self.reread(Reread::Files);
            self.book_changed();
            let on_disk = match &f.mark {
                Ok((m, there)) => {
                    self.unfetched = m.clone();
                    *there
                }
                Err(e) => {
                    self.faults.push(e.clone());
                    self.unfetched = Some(crate::restorex::State::Unfetched);
                    true
                }
            };
            if on_disk && self.unfetched.is_none() {
                self.tail_wanted = true;
            }
            self.tail_again |= self.unfetched.is_some() || on_disk;
            self.rooted = f.rooted;
            self.take_queue(&l.root, &f.queue, l.queue_at);
            self.take_settings(&l.root, &f.settings, l.settings_at);
            self.followed |= l.now.entries > seen;
            moved = true;
        } else if let Some(q) = &l.queue {
            self.take_queue(&l.root, q, l.queue_at);
            self.reread(Reread::Files);
            moved = true;
        }
        self.seen_entries = Some(l.now.entries);
        self.seen_queue = Some(l.now.queue);
        self.copies.raise(crate::reads::Copies::Baseline, l.now.seq);
        moved
    }

    pub fn follow_wake(&self, now: f64) -> Option<f64> {
        let wanted = self.home.is_some() && self.unlocked() && !self.swapping && self.machine.cli_path() == crate::machine::CliPath::Alone;
        wanted.then(|| (self.followed_at + FOLLOW_EVERY - now).max(0.0))
    }

    pub fn take_measure_due(&mut self) -> bool {
        let due = self.home.is_some() && self.archive_mark != Some(self.book_mark) && !self.tasks.in_flight(crate::task::Kind::Archive) && !self.swapping;
        if due {
            self.archive_mark = Some(self.book_mark);
        }
        due
    }

    pub fn archive_now(&self) -> Option<&ArchiveRead> {
        self.archive.as_ref().filter(|a| a.mark == self.book_mark)
    }

    pub fn backup_carries(&self) -> Option<(usize, usize, usize)> {
        let ids = self.identities.as_ref().map(|r| r.rows.len()).unwrap_or(0);
        self.archive_now().and_then(|a| a.backed).map(|(items, records)| (ids, items, records))
    }

    fn queue_follows_ledger(&mut self) {
        let fresh = self.audit_in_step();
        let Some((rows, _)) = self.rows.as_ref() else { return };
        let on_chain = crate::own::on_chain(self, rows);
        let owed: Vec<(String, bool)> = if fresh { crate::own::owed(self, rows) } else { Vec::new() };
        let faces = owed.iter().any(|(id, _)| rows.iter().any(|r| r.id == *id && r.kind == zikaron_glue::book::Kind::Face));
        if on_chain.is_empty() && owed.is_empty() {
            return;
        }
        if !self.lock.as_ref().is_some_and(|l| l.mode().writable()) {
            return;
        }
        let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) else { return };
        let others: Vec<u64> = self.basis_chains().into_iter().skip(1).map(|(c, _, _)| c).collect();
        let at = (self.clock)();
        self.leave(Keep::Follow { root, on_chain, owed, faces, others, at });
    }

    fn published_ids(&self) -> Vec<String> {
        let mut out: Vec<String> = self.audit.as_ref().map(|a| crate::auditx::entry_ids(&a.report, zikaron::v2::tokens::Key::Anchored).into_iter().collect()).unwrap_or_default();
        out.extend(self.remembered.iter().flat_map(|r| r.rows.iter().map(|(id, _)| id.to_ascii_lowercase())));
        out
    }

    pub fn queue_uncovered_now(&self) -> bool {
        let Some(pile) = self.home.as_ref().and_then(|h| h.ledger().ok()).and_then(|l| l.pile().ok()) else { return false };
        !crate::queue::uncovered(&self.queue, &pile.items, &self.published_ids()).is_empty()
    }

    pub fn catch_up_from_report(&mut self) -> bool {
        use zikaron::v2::tokens::Key;
        if !self.audit_in_step() {
            return false;
        }
        let (Some(a), Some(root), Some(chain)) = (self.audit.as_ref(), self.home.as_ref().map(|h| h.root().to_path_buf()), self.settings.chain_id) else { return false };
        let unanchored: Vec<String> = crate::auditx::entry_ids(&a.report, Key::Unanchored).into_iter().collect();
        let mut entries: Vec<String> = crate::auditx::entry_ids(&a.report, Key::Anchored).into_iter().collect();
        entries.extend(unanchored.iter().cloned());
        let owed = crate::queue::Owed { entries, unanchored, complete: a.label == zikaron::v2::tokens::Label::Complete.as_str() };
        let others: Vec<u64> = self.basis_chains().into_iter().skip(1).map(|(c, _, _)| c).collect();
        let at = (self.clock)();
        let published = self.published_ids();
        self.leave(Keep::CatchUp { root, chain, others, at, published, owed });
        true
    }

    pub fn catching_up(&self) -> bool {
        self.keeps.iter().any(|k| matches!(k, Keep::CatchUp { .. })) || self.tasks.in_flight(crate::task::Kind::Keep)
    }

    pub fn chain_set(&self) -> bool {
        self.settings.chain_id.is_some() && self.settings.registry.is_some() && !self.endpoints.is_empty()
    }

    fn audit_possible(&self) -> bool {
        if self.home.is_none() || !self.rooted {
            return false;
        }
        if self.settings.chain_id.is_none() || self.settings.registry.is_none() || self.endpoints.is_empty() {
            return false;
        }
        !self.tasks.in_flight(crate::task::Kind::Audit)
    }

    pub fn audit_stale(&self) -> bool {
        self.audit_possible() && self.audit_asked != Some(self.book_mark)
    }

    pub fn take_tail_due(&mut self) -> bool {
        let due = self.tail_due();
        if due {
            self.tail_asked = Some(self.book_mark);
        }
        due
    }

    pub fn set_up(&self) -> Option<SetUp> {
        if self.settings.role != crate::roles::Role::Author {
            return None;
        }
        self.set_up_seat()
    }

    pub fn set_up_seat(&self) -> Option<SetUp> {
        if self.rooted {
            return None;
        }
        Some(if self.unfetched.is_some() { SetUp::Fetch } else { SetUp::Create })
    }

    pub fn take_unwritten_due(&mut self) -> bool {
        let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) else { return false };
        let due = !self.rooted
            && self.unfetched.is_none()
            && self.chain_set()
            && self.settings_writable()
            && !self.tasks.in_flight(crate::task::Kind::Fetch)
            && !self.unwritten_asked.iter().any(|r| crate::home::same_place(r, &root));
        if due {
            self.unwritten_asked.push(root);
        }
        due
    }

    pub fn retake_mark(&mut self) -> bool {
        if self.rooted || self.unfetched.is_some() || !self.settings_writable() {
            return false;
        }
        let Some(home) = self.home.as_ref() else { return false };
        if !crate::restorex::moved(home, crate::restorex::On::of(&self.settings)) {
            return false;
        }
        if let Err(f) = crate::restorex::mark(home) {
            self.faults.push(f);
        }
        self.unfetched = Some(crate::restorex::State::Unfetched);
        self.tail_asked = None;
        true
    }

    pub fn tail_due(&self) -> bool {
        self.home.is_some()
            && (self.unfetched.is_some() || self.tail_wanted)
            && self.settings.chain_id.is_some()
            && self.settings.registry.is_some()
            && !self.endpoints.is_empty()
            && !self.tasks.in_flight(crate::task::Kind::Fetch)
            && self.tail_asked != Some(self.book_mark)
    }

    pub fn audit_lags(&self, now: f64) -> bool {
        self.audit_possible() && self.audit_lag_at().is_some_and(|at| now >= at)
    }

    pub fn audit_lag_at(&self) -> Option<f64> {
        let a = self.audit.as_ref()?;
        if self.audit_behind().is_empty() {
            return None;
        }
        let tried = self.tasks.landed_at(crate::task::Kind::Audit).map_or(a.at, |t| t.max(a.at));
        let at = tried + AUDIT_LAG_AGAIN;
        match &self.audit_lag_since {
            Some((since, _)) if at > since + AUDIT_LAG_BOUND => None,
            _ => Some(at),
        }
    }

    pub fn audit_behind(&self) -> Vec<String> {
        crate::own::receipts_behind(self)
    }

    pub fn audit_lag_stopped(&self) -> Option<Vec<String>> {
        let behind = self.audit_behind();
        (!behind.is_empty() && self.audit_lag_since.is_some() && self.audit_lag_at().is_none()).then_some(behind)
    }

    fn note_audit_lag(&mut self, now: f64) {
        let behind: std::collections::BTreeSet<String> = self.audit_behind().into_iter().collect();
        if behind.is_empty() {
            self.audit_lag_since = None;
            return;
        }
        let fresh = match &self.audit_lag_since {
            Some((_, seen)) => !behind.is_subset(seen),
            None => true,
        };
        if fresh {
            self.audit_lag_since = Some((now, behind));
        }
    }

    pub fn audit_due(&self, now: f64, last_tick: f64) -> bool {
        let every = self.settings.audit_every;
        if every == 0 || self.home.is_none() {
            return false;
        }
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
        let last = self
            .tasks
            .landed_at(crate::task::Kind::Audit)
            .unwrap_or(last_tick);
        now - last >= every as f64
    }

    pub fn writable(&self) -> bool {
        if self.broken() || self.handed.is_some() {
            return false;
        }
        self.lock.as_ref().map(|l| l.mode().writable()).unwrap_or(false)
    }

    pub fn settings_writable(&self) -> bool {
        self.lock.as_ref().map(|l| l.mode().writable()).unwrap_or(false)
    }

    pub fn pen_here(&self) -> bool {
        self.writable() && self.handed_off.is_none() && self.office_trouble.is_none()
    }

    pub fn may_send(&self) -> bool {
        !self.broken() && self.lock.as_ref().map(|l| l.mode().writable()).unwrap_or(false)
    }

    pub fn may_hold_entries(&self) -> Result<(), Fault> {
        if let Some(f) = self.office_trouble.as_ref() {
            return Err(f.clone());
        }
        if let Some(to) = self.handed.as_ref() {
            return Err(Fault::known(Known::HandedOver, to.clone()));
        }
        let Some(l) = self.lock.as_ref() else {
            return Err(Fault::known(Known::NoHome, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail005).to_string())));
        };
        if !l.mode().writable() {
            return Err(Fault::known(Known::ReadOnly, crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail006, &[&(l.holder()).to_string()]))));
        }
        Ok(())
    }

    pub fn may_write_entries(&self) -> Result<(), Fault> {
        self.seat_gates()?;
        if let Some(home) = self.home.as_ref() {
            crate::sendx::write_gates(home.root())?;
        }
        self.writer_lock()?;
        if let Some(home) = self.home.as_ref() {
            crate::sendx::lift_met(home.root())?;
        }
        Ok(())
    }

    fn seat_gates(&self) -> Result<(), Fault> {
        let door = self.door_acting || self.door_doing.is_some();
        if let (Some(f), false) = (self.office_trouble.as_ref(), door) {
            return Err(f.clone());
        }
        if let (Some(to), false) = (self.handed.as_ref(), door) {
            return Err(Fault::known(Known::HandedOver, to.clone()));
        }
        Ok(())
    }

    pub fn may_write_entries_shown(&self) -> Result<(), Fault> {
        self.seat_gates()?;
        if let Some(home) = self.home.as_ref() {
            let role = self.settings.role;
            let mut held = self.gates_shown.lock().unwrap_or_else(|p| p.into_inner());
            if !held.as_ref().is_some_and(|g| g.moved == self.moved && g.role == role && g.root == home.root()) {
                *held = Some(GatesShown { root: home.root().to_path_buf(), role, moved: self.moved, said: crate::sendx::write_gates(home.root()) });
            }
            if let Some(Err(f)) = held.as_ref().map(|g| &g.said) {
                return Err(f.clone());
            }
        }
        self.writer_lock()
    }

    pub fn may_send_entries(&self) -> Result<(), Fault> {
        match self.may_write_entries() {
            Err(f) if f.which() == Some(Known::HandedOver) => {
                if let Some(home) = self.home.as_ref() {
                    crate::sendx::write_gates(home.root())?;
                }
                self.writer_lock()
            }
            asked => asked,
        }
    }

    pub fn may_move_funds(&self) -> Result<(), Fault> {
        self.writer_lock()
    }

    fn writer_lock(&self) -> Result<(), Fault> {
        let Some(l) = self.lock.as_ref() else {
            return Err(Fault::known(Known::NoHome, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::Tail005).to_string())));
        };
        if !l.mode().writable() {
            return Err(Fault::known(
                Known::ReadOnly,
                crate::lang::both_of(|| crate::lang::filln(crate::lang::Key::Tail006, &[&(l.holder()).to_string()])),
            ));
        }
        Ok(())
    }

    pub fn hydrate(&mut self, home: &crate::home::Home) -> Result<(), Fault> {
        let read = Hydrated::read(home)?;
        self.install(read);
        Ok(())
    }

    pub fn install(&mut self, h: Hydrated) {
        let Hydrated { settings, endpoints, seen_entries, seen_queue, seen_seq, queue, rooted, remembered, verdicts, kits_root, kits_index, read_nets, unfetched, handed_off, pens, aside, troubles } = h;
        self.settings = settings;
        self.endpoints = endpoints;
        self.seen_entries = Some(seen_entries);
        self.seen_queue = Some(seen_queue);
        self.copies.raise(crate::reads::Copies::Baseline, seen_seq);
        self.seen_gen = self.seen_gen.wrapping_add(1);
        self.queue = queue;
        self.rooted = rooted;
        self.remembered = remembered;
        self.verdicts = verdicts;
        self.kits_root = kits_root;
        self.kits_index = kits_index;
        self.read_nets = read_nets;
        self.unfetched = unfetched;
        self.handed_off = handed_off;
        self.seen_pens = pens;
        self.aside = aside;
        self.faults.extend(troubles);
    }

    pub fn reread_aside(&mut self) {
        self.aside = aside_now();
    }

    pub fn reread_kits(&mut self) {
        let (root, index, nets, troubles) = kits_now(self.home.as_ref());
        self.kits_root = root;
        self.kits_index = index;
        self.read_nets = nets;
        self.faults.extend(troubles);
    }

    pub fn reread_nets(&mut self) {
        match crate::action::read_nets_now() {
            Ok(n) => self.read_nets = Some(n),
            Err(f) => {
                self.read_nets = None;
                self.faults.push(f);
            }
        }
    }

    pub fn review_due(&self, now: f64, last_tick: f64) -> bool {
        if self.vault_changed && self.home.is_some() && !self.tasks.in_flight(crate::task::Kind::Review) {
            return true;
        }
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

    pub fn stale_rows(&mut self) {
        self.rows_gen += 1;
        self.tasks.forget(crate::task::Kind::Ledger);
    }

    pub fn stale_grants(&mut self) {
        self.grants_gen += 1;
        self.tasks.forget(crate::task::Kind::Grants);
    }

    pub fn rows_current(&self) -> bool {
        self.rows.is_some() && self.rows_at == self.rows_gen
    }

    pub fn grants_current(&self) -> bool {
        self.grants.is_some() && self.grants_at == self.grants_gen
    }

    pub fn reread(&mut self, why: Reread) {
        for s in crate::reads::Cause::stales(why) {
            match s {
                Stale::Rows => self.stale_rows(),
                Stale::Grants => self.stale_grants(),
            }
        }
    }

    fn judge_ledger(&mut self) {
        use zikaron::v2::tokens::Key as T;
        let Some(a) = self.audit.as_ref() else { return };
        let items = crate::auditx::items(&a.report);
        let n = |k: T| items.iter().find(|i| i.key == k.as_str()).and_then(|i| i.count).unwrap_or(0);
        let alone = n(T::Findings) + n(T::Missing) + n(T::Malformed) + n(T::Void) + n(T::Unproven);
        self.ledger_handed_on = handed_on(&a.report, a.broken, self.handed.as_deref());
        if a.broken || alone > 0 {
            self.ledger_ok = Some(false);
            return;
        }
        let unknown = n(T::UnknownType);
        if unknown == 0 {
            self.ledger_ok = Some(true);
            return;
        }
        let Some((rows, _)) = self.rows.as_ref().filter(|_| self.rows_at == a.rows_gen && a.mark == self.book_mark) else { return };
        self.ledger_ok = Some(crate::auditx::unknown_of(&a.report, rows).left == 0);
    }

    pub fn ledger_verdict(&self) -> Option<Verdict> {
        self.audit.as_ref()?;
        Some(match self.ledger_ok? {
            true => Verdict::Normal,
            false => self.ledger_handed_on.map_or(Verdict::Abnormal, Verdict::HandedOn),
        })
    }

    pub fn audit_in_step(&self) -> bool {
        let Some(a) = self.audit.as_ref() else { return false };
        if !crate::auditx::whole(&a.label, a.asked, &a.unanswered) {
            return false;
        }
        if a.mark == self.book_mark {
            return true;
        }
        let on = self.on_chain();
        self.written_since(a).is_some_and(|ids| ids.iter().all(|id| on.anywhere(id)))
    }

    fn written_since(&self, a: &AuditRead) -> Option<Vec<String>> {
        let (then, now) = (a.seen?, self.seen_entries?);
        let more = now.checked_sub(then)?;
        if more == 0 {
            return Some(Vec::new());
        }
        if !self.rows_current() {
            return None;
        }
        let rows = &self.rows.as_ref()?.0;
        (rows.len() >= more).then(|| rows[..more].iter().map(|r| r.id.clone()).collect())
    }

    pub fn on_chain(&self) -> crate::chainx::OnChain {
        let read: Vec<(Option<u64>, String)> = match self.audit.as_ref() {
            Some(a) => self.basis_chains().into_iter().flat_map(|(c, _, _)| crate::auditx::anchored_on(&a.report, c).into_iter().map(move |id| (Some(c), id))).collect(),
            None => Vec::new(),
        };
        crate::chainx::OnChain::of(read, &self.queue)
    }

    pub fn audit_report_in_step(&self) -> Option<&zikaron::json::Value> {
        self.audit.as_ref().filter(|_| self.audit_in_step()).map(|a| &a.report)
    }

    pub fn source_changed(&mut self, scope: Source) {
        self.tasks.new_epoch();
        let home = scope == Source::Home;
        let Shell {
            page: _,
            documents: _,
            moved: _,
            gates_shown: _,
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
            anchor_public: _,
            identities: _,
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
            handed_off,
            seen_pens,
            vetted: _,
            peeks: _,
            keeps: _,
            keep_inline: _,
            unwritten_asked: _,
            vault: _,
            vault_recoverable: _,
            primary: _,
            rekeying: _,
            swapping: _,
            seat_wish: _,
            identity_wish: _,
            lock_pending: _,
            backup_made: _,
            backup_peek: _,
            items_now: _,
            door: _,
            door_doing: _,
            door_acting: _,
            door_told: _,
            door_waker: _,
            door_shut: _,
            door_too_long: _,
            door_place: _,
            door_off: _,
            door_answered: _,
            cli_path: _,
            ai_config: _,
            credential: _,
            passes: _,
            seen_entries,
            seen_queue,
            followed_at: _,
            seen_gen: _,
            copies: _,
            regain_stop: _,
            looks_dropped: _,
            archive_mark,
            landed,
            followed,
            vault_said: _,
            gate_said: _,
            said: _,
            machine: _,
            held_back: _,
            home: _,
            lock: _,
            content: _,
            flow: _,
            book: _,
            persona,
            face_books,
            face_pair,
            reroot_fee,
            face_joined: _,
            diligence: _,
            others_asked: _,
            others_whole: _,
            verified: _,
            cleaned: _,
            delivery,
            checked: _,
            check_asked: _,
            audits: _,
            attention: _,
            settings: _,
            queue: _,
            rooted: _,
            rows,
            rows_gen,
            rows_at: _,
            faces_read: _,
            seat_chains: _,
            places_read: _,
            rows_wanted: _,
            grants_gen,
            grants_at: _,
            ledger_ok,
            ledger_handed_on,
            audit,
            upgrade,
            upgrade_read,
            rotation,
            rotation_said,
            rot_left,
            rot_left_of,
            rot_vetted,
            rot_given,
            rot_written,
            rot_sending,
            rot_home: _,
            rot_sweep_out,
            rot_sweep_held,
            rot_short,
            rot_fresh,
            rot_sweep_to,
            rot_reverted,
            rot_looking,
            rot_unread,
            upgrade_cost,
            office_trouble,
            audit_asked,
            audit_lag_since,
            tail_asked,
            tail_wanted,
            tail_again,
            sent,
            repo_since,
            grants,
            kit,
            picked,
            clash,
            clash_unsettled,
            noticed_at: _,
            story,
            gas,
            gas_lane,
            door_followed: _,
            fees,
            fees_left,
            stuck,
            gas_epoch,
            gas_asked: _,
            resumed_at: _,
            failed,
            archive,
            chain,
            reconciled,
            pen,
            handed,
            cards,
            held,
            held_rejected,
            held_dirty,
            vault_changed,
            alarms,
            rung,
            badge,
            published,
            notices,
            fresh,
            status,
            chain_read_at,
            chain_time,
            aside: _,
            old_view,
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
        *upgrade = None;
        *upgrade_read = false;
        *rotation = None;
        *rotation_said = None;
        *rot_left = None;
        *rot_left_of = None;
        *rot_vetted = None;
        *rot_given = None;
        *rot_written = None;
        *rot_sending = None;
        *rot_sweep_out = None;
        *rot_sweep_held = None;
        *rot_short = None;
        *rot_fresh = Vec::new();
        *rot_sweep_to = None;
        *rot_reverted = None;
        *rot_looking = false;
        *rot_unread = None;
        *upgrade_cost = None;
        *office_trouble = None;
        *ledger_ok = None;
        *ledger_handed_on = None;
        *audit_asked = None;
        *audit_lag_since = None;
        *tail_asked = None;
        *tail_wanted = false;
        *tail_again = false;
        *sent = None;
        *repo_since = None;
        *grants = None;
        *kit = None;
        *picked = None;
        clash.clear();
        *clash_unsettled = 0;
        *story = None;
        *gas = None;
        *gas_lane = None;
        *fees = None;
        fees_left.clear();
        *stuck = None;
        *gas_epoch += 1;
        failed.clear();
        *archive = None;
        *archive_mark = None;
        if home {
            *seen_entries = None;
            *seen_queue = None;
        }
        *landed = None;
        *followed = false;
        *persona = crate::facex::State::Unread;
        *face_books = None;
        *face_pair = None;
        *reroot_fee = None;
        if !home {
            return;
        }
        *chain = None;
        *reconciled = None;
        *pen = Pen::Granted;
        *handed = None;
        *remembered = None;
        verdicts.clear();
        *unfetched = None;
        *handed_off = None;
        *seen_pens = None;
        *delivery = None;
        *cards = None;
        *held = None;
        held_rejected.clear();
        *held_dirty = false;
        *vault_changed = false;
        alarms.clear();
        rung.clear();
        *badge = None;
        *published = None;
        notices.clear();
        fresh.clear();
        *status = None;
        *chain_read_at = None;
        *chain_time = None;
    }

    pub fn trouble(&mut self, f: Fault) -> crate::action::Applied {
        self.faults.push(f.clone());
        crate::action::Applied::Trouble(f)
    }

    fn fetch_landed(&mut self, root: &std::path::Path, tail: &crate::restorex::Tail, landed: Option<(usize, usize)>, on: Option<crate::restorex::On>) {
        let here = self.home.as_ref().map(|h| crate::home::same_place(h.root(), root)).unwrap_or(false);
        if here {
            self.tail_wanted = false;
            if let Some((_, entries)) = landed.filter(|(n, _)| *n > 0) {
                self.book_changed();
                self.rooted = entries > 0;
            }
            self.tail_asked = Some(self.book_mark);
        }
        let open = self.home.as_ref().map(|h| h.root().to_path_buf());
        self.leave(Keep::Tail { root: root.to_path_buf(), tail: tail.clone(), open, on });
        self.reread(Reread::Landed);
    }

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
        self.keep_inline = true;
        let mut got = self.drain_at(0.0);
        let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while self.tasks.in_flight(crate::task::Kind::Keep) && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(1));
            got.extend(self.drain_at(0.0));
        }
        self.keep_inline = false;
        self.keep_now();
        got
    }

    pub fn keep_now(&mut self) {
        if self.keeps.is_empty() {
            return;
        }
        let all = std::mem::take(&mut self.keeps);
        let hand = self.lock.as_ref().map(crate::lock::Lock::hand);
        let out = keep(all, hand, self.settings_writable());
        self.kept(&out);
        self.moved += 1;
    }

    pub fn drain_at(&mut self, now: f64) -> Vec<Outcome> {
        if let Some(f) = trace::take_full() {
            self.faults.push(f);
        }
        self.credential_tick();
        let noticed = crate::reads::last_write(crate::reads::Copies::Noticed);
        if noticed != self.noticed_at {
            self.noticed_at = noticed;
            self.reread(Reread::Noticed);
        }
        let mut got = self.tasks.drain_at(now);
        if got.iter().any(|o| !matches!(o.kind, crate::task::Kind::Look | crate::task::Kind::Regain)) {
            self.moved += 1;
        }
        let unkept = zikaron_anchor::settled::unkept();
        if !unkept.is_empty() {
            self.faults.push(crate::fault::Fault::known(crate::fault::Known::ReadsUnkept, unkept.join("; ")));
        }
        let audited = got.iter().any(|o| o.kind == crate::task::Kind::Audit);
        for o in got.iter_mut() {
            if let Some(seen) = o.seen.as_ref().filter(|s| self.copies.newer(crate::reads::Copies::Baseline, s.seq) && self.home.as_ref().is_some_and(|h| h.root() == s.root.as_path())) {
                self.copies.raise(crate::reads::Copies::Baseline, seen.seq);
                self.seen_entries = Some(seen.entries);
                self.seen_queue = Some(seen.queue);
                self.seen_gen = self.seen_gen.wrapping_add(1);
            }
            let short = |chain: Option<u64>, fresh: &[(u64, u128)]| match &o.result {
                Err(f) if f.which() == Some(crate::fault::Known::InsufficientFunds) => Some(crate::rotatex::Short::of(chain, crate::action::funds_of(f.tail()), fresh)),
                _ => None,
            };
            if o.kind == crate::task::Kind::Rotate {
                let sending = self.rot_sending.take();
                let sweep = self.rot_sweep_out.take();
                if let Some(other) = self.rot_home.take().filter(|h| self.home.as_ref().map(|x| x.root()) != Some(h.as_path())) {
                    match &o.result {
                        Ok(Done::Rotation { settled, unread, .. }) => {
                            self.keep_swept_in(other, settled);
                            if let Some(f) = unread {
                                self.faults.push(f.clone());
                            }
                        }
                        Ok(_) => {}
                        Err(f) => self.faults.push(f.clone()),
                    }
                    continue;
                }
                if let (Some((chain, value)), Err(f)) = (sweep, &o.result) {
                    if f.which() == Some(crate::fault::Known::NotIncluded) {
                        self.rot_sweep_held = Some((chain, value, f.tail().to_string()));
                    }
                }
                if sending.is_some() && sweep.is_none() {
                    self.rot_short = short(sending, &self.rot_fresh);
                }
                if sending.is_some() {
                    self.rot_reverted = match &o.result {
                        Ok(Done::Rotation { reverted, .. }) => reverted.clone(),
                        _ => None,
                    };
                }
                if std::mem::take(&mut self.rot_looking) {
                    self.rot_unread = o.result.as_ref().err().cloned();
                    if self.rot_unread.is_some() {
                        self.rotation = None;
                    }
                }
            }
            if o.kind == crate::task::Kind::Anchor {
                self.rot_short = short(None, &self.rot_fresh);
            }
            if o.renoded {
                continue;
            }
            if crate::landing::goes_back(o.kind, crate::landing::Back::Vault) {
                let got = o.result.clone().map(|d| match d {
                    Done::Vault(v) => v,
                    _ => crate::task::Vault::Opened,
                });
                let said = crate::action::vault_landed(self, got);
                self.vault_said = Some(said);
                continue;
            }
            if let (crate::task::Kind::Gate, Ok(Done::GatePassed { root, then, pass })) = (o.kind, &o.result) {
                if let Some(said) = crate::action::gate_landed(self, root.clone(), (**then).clone(), pass, o.stale) {
                    self.gate_said = Some(said);
                }
                continue;
            }
            if matches!(o.result, Ok(Done::Issued { .. }) | Ok(Done::Withdrew { .. })) {
                self.failed.remove(&o.kind);
            }
            if let (crate::task::Kind::Gate, Ok(Done::Issued { root, then, member })) = (o.kind, &o.result) {
                if let Some(said) = crate::action::issued(self, root.clone(), (**then).clone(), *member, o.stale) {
                    self.gate_said = Some(said);
                }
                continue;
            }
            if let (crate::task::Kind::Gate, Ok(Done::FaceCleared { root, then })) = (o.kind, &o.result) {
                self.failed.remove(&o.kind);
                if let Some(said) = crate::action::face_cleared(self, root.clone(), (**then).clone()) {
                    self.gate_said = Some(said);
                }
                continue;
            }
            if let (crate::task::Kind::Gate, Ok(Done::Withdrew { root, entry, grant, chain, noticed })) = (o.kind, &o.result) {
                if let Some(said) = crate::action::withdrew(self, root.clone(), entry.clone(), grant.clone(), *chain, noticed.clone()) {
                    self.gate_said = Some(said);
                }
                continue;
            }
            if crate::action::lands_said(o.kind) {
                if o.kind == crate::task::Kind::Migrate {
                    self.swapping = false;
                }
                let behind = o.kind == crate::task::Kind::Gas && self.gas_out_of_date();
                if behind && !o.stale {
                    let f = crate::fault::Fault::known(crate::fault::Known::GasNotShown, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::TailGasBehind).to_string()));
                    self.said.insert(o.kind, crate::action::Applied::Trouble(f));
                } else if o.stale {
                    if let Err(f) = &o.result {
                        self.faults.push(f.clone());
                    }
                } else {
                    match &o.result {
                        Ok(_) => {
                            self.failed.remove(&o.kind);
                        }
                        Err(f) => {
                            self.failed.insert(o.kind, f.clone());
                        }
                    }
                    let said = crate::action::landed(self, o.kind, o.result.clone());
                    self.said.insert(o.kind, said);
                }
                continue;
            }
            if o.stale {
                if let Err(f) = &o.result {
                    self.faults.push(f.clone());
                }
                continue;
            }
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
                    if o.kind == crate::task::Kind::Backup {
                        self.leave(Keep::Machine);
                    }
                    if o.kind == crate::task::Kind::Fetch && !self.fetch_checks_tail && !crate::local::is_cut(f) {
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
            if let Ok(d) = &o.result {
                if chain_answered(d) {
                    self.status = None;
                    self.chain_read_at = Some(now);
                }
            }
            match &o.result {
                Ok(Done::Vault(_)) => {}
                Ok(Done::Path(_)) => {}
                Ok(Done::Kept(all)) => self.kept(all),
                Ok(Done::Looked(l)) => {
                    if self.looked(l) {
                        self.moved += 1;
                    }
                }
                Ok(Done::Regained { root, took }) => {
                    if self.home.as_ref().is_some_and(|h| h.root() == root.as_path()) {
                        let home = crate::home::Home::bare(root.clone());
                        if let Some(l) = self.lock.as_mut() {
                            l.regained(took, &home);
                        }
                    }
                }
                Ok(Done::CliPath(s)) => self.cli_path = Some(s.clone()),
                Ok(Done::GatePassed { .. }) | Ok(Done::Issued { .. }) | Ok(Done::Withdrew { .. }) | Ok(Done::FaceCleared { .. }) => {}
                Ok(Done::Gas { .. }) | Ok(Done::Took { .. }) | Ok(Done::Hashed { .. }) | Ok(Done::Copied { .. }) | Ok(Done::Seated(_)) => {}
                Ok(Done::Extracted { .. }) => {}
                Ok(Done::Cleaned { plan, done }) => self.cleaned = Some((plan.clone(), *done)),
                Ok(Done::Check(r)) => self.last = Some(r.clone()),
                Ok(Done::Archive { bytes, items, skipped, mirror, records, machine_items, backed, mark }) => {
                    self.archive = Some(ArchiveRead {
                        bytes: *bytes,
                        items: *items,
                        skipped: *skipped,
                        mirror: mirror.clone(),
                        records: *records,
                        backed: *backed,
                        mark: *mark,
                    });
                    match machine_items {
                        Ok(n) => self.items_now = Some(*n),
                        Err(f) => {
                            self.items_now = None;
                            if !self.faults.iter().any(|x| x.said() == f.said()) {
                                self.faults.push(f.clone());
                            }
                        }
                    }
                }
                Ok(Done::ChainTime(t)) => self.note_chain_time(*t),
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
                    unproven,
                    asked,
                    single_source,
                    fragment,
                    recounted,
                }) => {
                    let at = match (&self.audit, recounted) {
                        (Some(before), true) => before.at,
                        _ => now,
                    };
                    self.audit = Some(AuditRead {
                        label: label.clone(),
                        complete: *complete,
                        broken: *broken,
                        entries: *entries,
                        report: report.clone(),
                        unanswered: unanswered.clone(),
                        unproven: unproven.clone(),
                        asked: *asked,
                        single_source: *single_source,
                        fragment: fragment.clone(),
                        at,
                        mark: self.audit_asked.unwrap_or(self.book_mark),
                        seen: if self.audit_asked.is_none_or(|m| m == self.book_mark) { self.seen_entries } else { None },
                        rows_gen: self.rows_gen,
                        nodes: self.endpoints.clone(),
                    });
                    if let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) {
                        let rows = crate::ledgerx::anchored_of(report);
                        let at = (self.clock)();
                        let whole = crate::auditx::whole(label, *asked, unanswered);
                        self.remembered = Some(crate::lastread::Anchored { rows: rows.clone(), at, whole });
                        self.leave(Keep::Anchored { root, rows, at, whole });
                    }
                    self.reconciled = Some((label.clone(), *complete, *entries));
                    self.pen = if *complete { Pen::Granted } else { Pen::Held };
                    let lacks = label == zikaron::v2::tokens::Label::Unproven.as_str() || !crate::auditx::rows_of(report, zikaron::v2::tokens::Key::Missing).is_empty();
                    if lacks {
                        self.tail_wanted = true;
                        self.tail_asked = None;
                    }
                    if *complete && !lacks && self.unfetched.is_some() {
                        self.tail_asked = None;
                        self.tail_again = true;
                    }
                    self.reread(Reread::Landed);
                    let r#gen = self.rows_gen;
                    if let Some(a) = self.audit.as_mut() {
                        a.rows_gen = r#gen;
                    }
                    self.judge_ledger();
                }
                Ok(Done::Ledger { rows, strays, handed, office, upgrade, upgrade_read, seats, faces, places, r#gen }) => {
                    if *r#gen == self.rows_gen {
                        self.rows = Some((rows.clone(), *strays));
                        self.seat_chains = seats.clone();
                        self.rows_at = *r#gen;
                        let root = self.home.as_ref().map(|h| h.root().to_path_buf());
                        self.faces_read.put((*r#gen, root.clone()), faces.clone());
                        self.places_read.put((self.book_mark, *r#gen, root), places.clone());
                        self.handed = handed.clone();
                        if crate::action::follow_office(self, office.as_ref()) {
                            self.handed = None;
                        }
                        self.upgrade = Some(upgrade.clone());
                        self.upgrade_read = *upgrade_read;
                        self.seal_rows();
                        self.queue_follows_ledger();
                        self.judge_ledger();
                    } else {
                        self.tasks.forget(crate::task::Kind::Ledger);
                    }
                }
                Ok(Done::UpgradeCost { stage, txs, wei }) => self.upgrade_cost = Some((stage.clone(), *txs, *wei)),
                Ok(Done::Rotation { chains, said, unread, left, settled, gone, fresh, .. }) => {
                    self.rotation = Some(chains.clone());
                    if !fresh.is_empty() {
                        self.rot_fresh = fresh.clone();
                    }
                    if *gone {
                        self.rot_sweep_held = None;
                    }
                    self.rotation_said = said.clone();
                    if let Some(f) = unread {
                        self.faults.push(f.clone());
                    }
                    if let Some(l) = left {
                        self.rot_left = Some(l.left.clone());
                        self.rot_left_of = Some(l.old);
                    }
                    self.keep_swept(settled);
                }
                Ok(Done::RotateVetted { to }) => self.rot_vetted = Some(*to),
                Ok(d @ Done::Book { .. }) => {
                    if let Done::Book { audit_key, .. } = d {
                        self.audits.recall(*audit_key);
                    }
                    self.book = Some(d.clone());
                    crate::action::behind_turn(self);
                }
                Ok(Done::Persona(state, pair)) => {
                    self.persona = state.clone();
                    self.face_pair = pair.clone();
                    self.seal_rows();
                }
                Ok(Done::RerootFee(f)) => self.reroot_fee = Some(f.clone()),
                Ok(Done::FaceJoined(j)) => self.face_joined = Some(j.as_ref().clone()),
                Ok(Done::FaceBooks(b)) => self.face_books = Some(b.clone()),
                Ok(Done::BasisRead { chain, registry, from_block, reading, said, after_nodes, asked }) => {
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
                Ok(Done::Submitted { .. }) | Ok(Done::Anchored { .. }) if o.earlier => {
                    if let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) {
                        self.leave(Keep::Queue { root });
                    }
                    self.stuck = None;
                    self.reread(Reread::Landed);
                }
                Ok(Done::Submitted { tx, chain, url, ids, gas, queue }) => {
                    self.queue = queue.clone();
                    if let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) {
                        self.leave(Keep::Queue { root });
                    }
                    self.gas = None;
                    self.gas_lane = None;
                    self.stuck = None;
                    self.reread(Reread::Landed);
                    if !self.lock_pending {
                        crate::action::wait_submitted(self, tx.clone(), *chain, url.clone(), ids.clone(), *gas);
                    }
                }
                Ok(d @ Done::Anchored { .. }) => {
                    if let Done::Anchored { queue, whole, .. } = d {
                        let root = self.home.as_ref().map(|h| h.root().to_path_buf());
                        match whole {
                            Some((Ok(q), at)) => {
                                if let Some(root) = root {
                                    self.take_queue(&root, &Ok(q.clone()), *at);
                                }
                            }
                            Some((Err(f), _)) => {
                                self.queue = crate::queue::Queue { chain: self.settings.chain_id.unwrap_or(0), items: queue.clone(), anchored: Vec::new(), blocks: Vec::new(), passed: Vec::new() };
                                self.faults.push(f.clone());
                            }
                            None => {
                                self.queue = crate::queue::Queue { chain: self.settings.chain_id.unwrap_or(0), items: queue.clone(), anchored: Vec::new(), blocks: Vec::new(), passed: Vec::new() };
                                if let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) {
                                    self.leave(Keep::Queue { root });
                                }
                            }
                        }
                    }
                    self.gas = None;
                    self.gas_lane = None;
                    if let Done::Anchored { stuck, confirmed, .. } = d {
                        self.stuck = if *confirmed { None } else { stuck.clone() };
                    }
                    if let Done::Anchored { voided: true, tx, .. } = d {
                        self.faults.push(crate::fault::Fault::known(crate::fault::Known::BatchVoided, tx.clone()));
                    }
                    self.sent = Some(d.clone());
                    self.reread(Reread::Landed);
                    self.book_changed();
                    if matches!(d, Done::Anchored { confirmed: true, .. }) {
                        crate::action::audit_after_send(self, d);
                    }
                }
                Ok(Done::Vetted(rows)) => {
                    for (p, d) in rows {
                        self.vetted.insert(p.clone(), d.clone());
                    }
                }
                Ok(Done::Peeked(got)) => {
                    let (ask, read) = (**got).clone();
                    self.peeks.retain(|(a, _)| a.which() != ask.which());
                    self.peeks.push((ask, read));
                }
                Ok(d @ Done::Kit { .. }) => {
                    if let Done::Kit { root, publish, path, kit_id, facts, .. } = d {
                        let open = self.home.as_ref().map(|h| h.root().to_path_buf());
                        self.leave(Keep::Kit { root: root.clone(), path: path.clone(), kit_id: kit_id.clone(), at: (self.clock)(), publish: publish.clone(), facts: facts.clone(), open });
                    }
                    self.kit = Some(d.clone())
                }
                Ok(Done::Keystore(crate::task::Keystore::BackedUp { id, path, .. })) => {
                    self.leave(Keep::BackedUp { id: id.clone(), path: path.clone() });
                }
                Ok(Done::BackupMade { path, summary }) => {
                    self.backup_made = Some((path.clone(), summary.clone()));
                    self.leave(Keep::Machine);
                }
                Ok(Done::BackupSeen { path, summary }) => self.backup_peek = Some((path.clone(), summary.clone())),
                Ok(Done::Grants { rows, r#gen }) => {
                    if *r#gen == self.grants_gen {
                        self.grants = Some(rows.clone());
                        self.grants_at = *r#gen;
                    } else {
                        self.tasks.forget(crate::task::Kind::Grants);
                    }
                }
                Ok(Done::Diligence(r)) => {
                    self.audits.recall(r.audit_key);
                    self.diligence = Some((**r).clone());
                    crate::action::behind_turn(self);
                }
                Ok(Done::Verified(v)) => self.verified = Some((**v).clone()),
                Ok(Done::Delivery(c)) => self.delivery = Some(c.clone()),
                Ok(Done::Reviewed { cards, now }) => {
                    if let Some(t) = now {
                        self.note_chain_time(*t);
                    }
                    let (fresh, keys) = crate::sentinelx::alarms(cards, &self.settings.alarmed, *now);
                    if !fresh.is_empty() {
                        self.rung.extend(fresh.iter().cloned());
                        for a in fresh.into_iter().rev() {
                            self.alarms.insert(0, a);
                        }
                        self.settings.alarmed.extend(keys.iter().cloned());
                        self.attention = true;
                        if let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) {
                            self.leave(Keep::Alarmed { root, keys });
                        }
                    }
                    if let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) {
                        let at = (self.clock)();
                        let mut rows = Vec::with_capacity(cards.len());
                        for c in cards.iter().filter(|c| c.told()) {
                            let v = crate::lastread::of_card(c, at, *now);
                            self.verdicts.retain(|(id, _)| !id.eq_ignore_ascii_case(&c.id));
                            self.verdicts.push((c.id.clone(), v.clone()));
                            rows.push((c.id.clone(), v));
                        }
                        self.leave(Keep::Verdicts { root, rows });
                    }
                    self.cards = Some((cards.clone(), *now));
                }
                Ok(Done::Held { held, rejected }) => {
                    self.held = Some(held.clone());
                    self.held_rejected = rejected.clone();
                }
                Ok(Done::Badge(b)) => self.badge = Some((**b).clone()),
                Ok(Done::Published { url, read }) => self.published = Some((url.clone(), read.clone())),
                Ok(Done::Fetched { root, tail, landed, entries, on, .. }) => {
                    self.fetch_conflict = None;
                    self.last_aside = None;
                    self.fetch_landed(root, tail, Some((*landed, *entries)), *on);
                }
                Ok(Done::TailChecked { checked, unwritten }) => {
                    for (root, tail, on) in checked {
                        match tail {
                            Ok(t @ crate::restorex::Tail::NewerElsewhere { .. }) => self.fetch_landed(root, t, None, *on),
                            Ok(_) | Err(_) if *unwritten => {}
                            Ok(tail) => self.fetch_landed(root, tail, None, *on),
                            Err(f) => self.faults.push(f.clone()),
                        }
                    }
                }
                Ok(Done::FetchConflict { root, offline, fetched, rows }) => {
                    self.fetch_conflict = Some(Conflict { root: root.clone(), offline: *offline, fetched: *fetched, rows: rows.clone() });
                }
                Ok(Done::FetchedAside { aside, fetched }) => {
                    self.fetch_conflict = None;
                    if let Done::Fetched { root, tail, landed, entries, on, .. } = &**fetched {
                        if self.home.as_ref().map(|h| crate::home::same_place(h.root(), root)).unwrap_or(false) {
                            if let Err(f) = crate::action::reopen_here(self, root) {
                                self.faults.push(f);
                            }
                        }
                        self.fetch_landed(root, tail, Some((*landed, *entries)), *on);
                    }
                    self.last_aside = Some(aside.clone());
                    self.reread_aside();
                }
                Ok(Done::Checked(c)) => {
                    if let (Some(t), crate::checkx::NowFrom::Chain) = (c.now, &c.now_from) {
                        self.note_chain_time(t);
                    }
                    self.audits.recall(c.audit_key);
                    self.checked = Some((**c).clone());
                    crate::action::behind_turn(self);
                }
                Ok(Done::Behind { .. }) => {
                    crate::action::behind_turn(self);
                }
                Ok(Done::Reconciled { label, complete, entries }) => {
                    self.reconciled = Some((label.clone(), *complete, *entries));
                    self.pen = if *complete { Pen::Granted } else { Pen::Held };
                }
                Err(_) if o.kind == crate::task::Kind::ChainTime => {}
                Err(f) => {
                    if crate::watchx::is_network(f) {
                        self.status = Some((crate::lang::Key::StatusNet, f.evidence()));
                    }
                    self.faults.push(f.clone())
                }
            }
        }
        if !got.is_empty() {
            self.sweep();
        }
        if self.lock_pending && !self.tasks.flying().into_iter().any(|k| k.writes_local()) {
            self.lock_pending = false;
            crate::keybox::lock();
            self.after_lock();
        }
        if self.held_dirty && !self.tasks.in_flight(crate::task::Kind::Held) {
            self.held_dirty = false;
            self.tasks.forget(crate::task::Kind::Held);
            let _ = crate::action::apply(self, crate::action::Action::ListHeld);
        }
        self.turn_sync();
        self.follow_disk(now);
        if audited {
            self.note_audit_lag(now);
        }
        if std::mem::take(&mut self.tail_again) && !self.rekeying && !self.swapping && self.take_tail_due() {
            let _ = crate::action::apply(self, crate::action::Action::CheckTail);
        }
        self.keep_turn();
        self.door_turn(&got);
        if !self.held_back.is_empty() {
            let mut all = std::mem::take(&mut self.held_back);
            all.extend(got);
            return all;
        }
        got
    }

    pub fn drain_hold(&mut self) {
        let got = self.drain_at(0.0);
        self.held_back = got;
    }

    pub(crate) fn keep_back(&mut self, got: Vec<Outcome>) {
        self.held_back = got;
    }

    pub fn idle_tick(&mut self, last: f64, now: f64) -> bool {
        if self.rekeying || self.lock_pending || !self.unlocked() || !crate::machine::idle_locks(&self.machine, last, now) {
            return false;
        }
        matches!(crate::action::apply(self, crate::action::Action::Lock), crate::action::Applied::LockedUp)
    }

    pub fn note_chain_time(&mut self, t: u64) {
        self.chain_time = Some(self.chain_time.map(|x| x.max(t)).unwrap_or(t));
    }

    pub fn chain_reading(&self) -> Result<Option<&Done>, &Fault> {
        match (crate::landing::of(crate::task::Kind::Chain).kept, self.failed.get(&crate::task::Kind::Chain)) {
            (crate::landing::Kept::Failed, Some(f)) => Err(f),
            _ => Ok(self.chain.as_ref()),
        }
    }

    pub fn chain_now(&self) -> Option<u64> {
        let vault = self.cards.as_ref().and_then(|(_, n)| *n);
        let ledger = self.audit.as_ref().and_then(|a| {
            crate::auditx::rows_of(&a.report, zikaron::v2::tokens::Key::Anchored)
                .iter()
                .filter_map(|r| match r {
                    zikaron::json::Value::Obj(m) => m
                        .iter()
                        .find(|(k, _)| k == zikaron::v2::tokens::Key::Anchors.as_str())
                        .and_then(|(_, x)| match x {
                            zikaron::json::Value::Arr(a) => a.first().cloned(),
                            _ => None,
                        }),
                    _ => None,
                })
                .filter_map(|one| match &one {
                    zikaron::json::Value::Obj(m) => m
                        .iter()
                        .find(|(k, _)| k == zikaron::v2::tokens::Key::BlockTimestamp.as_str())
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

    pub fn rot_short_said(&self) -> bool {
        self.rot_short.as_ref().is_some_and(|s| crate::rotatex::short_now(s, &self.rot_fresh))
    }

    pub fn watch_rows(&self) -> Vec<crate::watchx::Row> {
        match self.settings.role {
            crate::roles::Role::Author => crate::watchx::author(
                self.rows.as_ref().map(|(r, _)| r.as_slice()),
                self.queue.len(),
                self.grants.as_deref(),
                (self.machine.backup.as_ref(), self.items_now, self.machine.backup_failed),
                self.audit.as_ref().map(|a| (a.label.as_str(), &a.report, a.unanswered.as_slice())),
                self.ledger_verdict(),
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

    pub fn keep_swept(&mut self, settled: &[(String, u64, crate::rotatex::Settled)]) {
        let mut lines = Vec::new();
        for (id, chain, how) in settled {
            if crate::rotatex::settled_of(&self.settings.swept, id).iter().any(|(c, _)| c == chain) {
                continue;
            }
            let line = how.line(id, *chain);
            self.settings.swept.push(line.clone());
            lines.push(line);
            if self.rot_sweep_held.as_ref().is_some_and(|(c, _, _)| c == chain) {
                self.rot_sweep_held = None;
            }
            if self.rot_reverted.as_ref().is_some_and(|r| r.chain == *chain) {
                self.rot_reverted = None;
            }
        }
        if lines.is_empty() {
            return;
        }
        if let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) {
            self.leave(Keep::Swept { root, lines });
        }
    }

    fn keep_swept_in(&mut self, root: std::path::PathBuf, settled: &[(String, u64, crate::rotatex::Settled)]) {
        let lines: Vec<String> = settled.iter().map(|(id, chain, how)| how.line(id, *chain)).collect();
        if !lines.is_empty() {
            self.leave(Keep::Swept { root, lines });
        }
    }

    pub fn sweep(&mut self) {
        let rows = self.watch_rows();
        let (fresh, keys) = crate::watchx::fresh(crate::watchx::to_ring(rows), &self.settings.alarmed);
        if fresh.is_empty() {
            return;
        }
        self.settings.alarmed.extend(keys.iter().cloned());
        self.notices.extend(fresh.iter().cloned());
        self.fresh.extend(fresh);
        self.attention = true;
        if let Some(root) = self.home.as_ref().map(|h| h.root().to_path_buf()) {
            self.leave(Keep::Alarmed { root, keys });
        }
    }

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

fn primary_reading() -> Option<(String, crate::keybox::PrimaryKind)> {
    match crate::keybox::primary() {
        Ok(p) => p,
        Err(_) => crate::keybox::primary_kind().ok().flatten().map(|k| (String::new(), k)),
    }
}

fn gas_source(s: &Settings) -> (Option<u64>, Option<crate::key::Address>, std::collections::BTreeSet<&str>, std::collections::BTreeSet<&str>) {
    let rest = if s.rest_on { s.rest.iter().map(String::as_str).collect() } else { std::collections::BTreeSet::new() };
    (s.chain_id, s.registry, s.endpoints.iter().map(String::as_str).collect(), rest)
}

pub const FOLLOW_EVERY: f64 = 1.0;

pub const AUDIT_LAG_AGAIN: f64 = 15.0;

pub const AUDIT_LAG_BOUND: f64 = 600.0;

#[derive(Clone, Debug)]
pub enum Keep {
    Machine,
    Settings { root: std::path::PathBuf },
    Register,
    Alarmed { root: std::path::PathBuf, keys: Vec<String> },
    Swept { root: std::path::PathBuf, lines: Vec<String> },
    Verdicts { root: std::path::PathBuf, rows: Vec<(String, crate::lastread::Verdict)> },
    Anchored { root: std::path::PathBuf, rows: Vec<(String, (u64, String))>, at: u64, whole: bool },
    Kit { root: std::path::PathBuf, path: String, kit_id: String, at: u64, publish: Option<String>, facts: crate::kitsindex::RowFacts, open: Option<std::path::PathBuf> },
    BackedUp { id: Option<String>, path: String },
    Queue { root: std::path::PathBuf },
    Follow { root: std::path::PathBuf, on_chain: Vec<(String, u64)>, owed: Vec<(String, bool)>, faces: bool, others: Vec<u64>, at: u64 },
    CatchUp { root: std::path::PathBuf, chain: u64, others: Vec<u64>, at: u64, published: Vec<String>, owed: crate::queue::Owed },
    Seal { root: std::path::PathBuf, ids: Vec<String> },
    Tail { root: std::path::PathBuf, tail: crate::restorex::Tail, open: Option<std::path::PathBuf>, on: Option<crate::restorex::On> },
}

#[derive(Clone, Debug)]
pub enum Kept {
    Machine(Result<crate::machine::Machine, Fault>, u64),
    Settings { root: std::path::PathBuf, settings: Result<Settings, Fault>, at: u64 },
    Wrote(Result<(), Fault>),
    Kits { open: Option<std::path::PathBuf>, root: Option<String>, index: Option<Vec<crate::kitsindex::Row>>, nets: Option<Vec<crate::readnets::Net>>, troubles: Vec<Fault> },
    Register(Result<crate::identity::Registry, Fault>, u64),
    Queue { root: std::path::PathBuf, queue: Result<crate::queue::Queue, Fault>, at: u64 },
    Followed { root: std::path::PathBuf, queue: Option<Result<crate::queue::Queue, Fault>>, owed: Vec<String>, at: u64 },
    Sealed { root: std::path::PathBuf, queue: Result<crate::queue::Queue, Fault>, held: usize, at: u64 },
    Tail { root: std::path::PathBuf, mark: Result<Option<crate::restorex::State>, Fault> },
}

pub fn keep(all: Vec<Keep>, hand: Option<crate::lock::Hand>, writer: bool) -> Vec<Kept> {
    use crate::reads::{ticket, Copies};
    let under = |root: &std::path::Path, f: &mut dyn FnMut() -> Result<(), Fault>| -> Result<(), Fault> {
        if writer {
            crate::lock::settling(hand.as_ref(), root, f)?
        } else {
            f()
        }
    };
    let kits = |open: &Option<std::path::PathBuf>| {
        let home = open.as_ref().map(|r| crate::home::Home::bare(r.clone()));
        let (root, index, nets, troubles) = kits_now(home.as_ref());
        Kept::Kits { open: open.clone(), root, index, nets, troubles }
    };
    let mut out = Vec::new();
    for k in all {
        let mut one: Vec<Kept> = Vec::new();
        match k {
            Keep::Machine => {
                let at = ticket(Copies::Machine);
                one.push(Kept::Machine(crate::machine::read(), at));
            }
            Keep::Settings { root } => {
                let at = ticket(Copies::Settings);
                one.push(Kept::Settings { settings: Settings::read(&crate::home::Home::bare(root.clone())), root, at });
            }
            Keep::Register => {
                let at = ticket(Copies::Register);
                one.push(Kept::Register(crate::register::view(), at));
            }
            Keep::Alarmed { .. } | Keep::Swept { .. } if !writer => {}
            Keep::Alarmed { root, keys } => {
                let h = crate::home::Home::bare(root.clone());
                one.push(Kept::Wrote(under(&root, &mut || Settings::amend(&h, |s| {
                    for k in &keys {
                        if !s.alarmed.contains(k) {
                            s.alarmed.push(k.clone());
                        }
                    }
                }).map(|_| ()))));
            }
            Keep::Swept { root, lines } => {
                let h = crate::home::Home::bare(root.clone());
                one.push(Kept::Wrote(under(&root, &mut || Settings::amend(&h, |s| {
                    for l in &lines {
                        let mut w = l.split(' ');
                        let (id, chain) = (w.next().unwrap_or_default(), w.next().and_then(|c| c.parse::<u64>().ok()));
                        let settled = chain.is_some_and(|c| crate::rotatex::settled_of(&s.swept, id).iter().any(|(x, _)| *x == c));
                        if !settled && !s.swept.contains(l) {
                            s.swept.push(l.clone());
                        }
                    }
                }).map(|_| ()))));
            }
            Keep::Verdicts { root, rows } => {
                let h = crate::home::Home::bare(root.clone());
                for (id, v) in rows {
                    one.push(Kept::Wrote(under(&root, &mut || crate::lastread::save_verdict(&h, &id, &v))));
                }
            }
            Keep::Anchored { root, rows, at, whole } => {
                let h = crate::home::Home::bare(root.clone());
                one.push(Kept::Wrote(under(&root, &mut || crate::lastread::save_anchored(&h, &rows, at, whole))));
            }
            Keep::Kit { root, path, kit_id, at, publish, facts, open } => {
                let kit = std::fs::canonicalize(&path).unwrap_or_else(|_| std::path::PathBuf::from(&path));
                let r = crate::home::Home::open(&root).and_then(|h| crate::home::machine_dir().and_then(|m| crate::kitsindex::add(&m, &h, &kit, &kit_id, at, publish.as_deref(), facts.clone())));
                one.push(Kept::Wrote(r.map(|_| ())));
                one.push(kits(&open));
            }
            Keep::BackedUp { id, path } => {
                if let Some(id) = id {
                    if let Err(f) = crate::register::change(|reg| crate::identity::mark(reg, &id, false, true, Some(&path))) {
                        one.push(Kept::Wrote(Err(f)));
                    }
                }
                let at = ticket(Copies::Register);
                one.push(Kept::Register(crate::register::view(), at));
            }
            Keep::Queue { root } => {
                let at = ticket(Copies::Queue);
                one.push(Kept::Queue { queue: crate::queue::Queue::read(&crate::home::Home::bare(root.clone())), root, at });
            }
            Keep::Follow { root, on_chain, owed, faces, others, at } => {
                let h = crate::home::Home::bare(root.clone());
                let places = if faces { crate::home::Home::open(&root).ok().and_then(|h| h.ledger().ok()).and_then(|l| l.pile().ok()).map(|p| zikaron_seal::queue::places_of(&p.items)).unwrap_or_default() } else { Default::default() };
                let owed3: Vec<(String, bool, Option<u64>)> = owed.into_iter().map(|(id, alone)| {
                    let seated = places.get(&id.to_ascii_lowercase()).and_then(|p| p.carried_on);
                    (id, alone, seated)
                }).collect();
                let mut q = None;
                let mut read_at = 0;
                let took = under(&root, &mut || {
                    q = Some(crate::queue::follow(&h, &on_chain, &owed3, &others, at));
                    read_at = ticket(Copies::Queue);
                    Ok(())
                });
                let queue = took.ok().and(q);
                one.push(Kept::Followed { root, queue, owed: owed3.into_iter().map(|(id, _, _)| id).collect(), at: read_at });
            }
            Keep::CatchUp { root, chain, others, at, published, owed } => {
                let h = crate::home::Home::bare(root.clone());
                let mut q = None;
                let mut read_at = 0;
                let took = under(&root, &mut || {
                    q = Some(crate::queue::caught_up(&h, chain, &others, at, &published, owed.clone()));
                    read_at = ticket(Copies::Queue);
                    Ok(())
                });
                let queue = took.ok().and(q);
                one.push(Kept::Followed { root, queue, owed: Vec::new(), at: read_at });
            }
            Keep::Seal { root, ids } => {
                let h = crate::home::Home::bare(root.clone());
                let mut q = None;
                let mut read_at = 0;
                let took = under(&root, &mut || {
                    q = Some(crate::queue::amend(&h, |q| q.drop_ids(&ids)).map(|(_, q)| q));
                    read_at = ticket(Copies::Queue);
                    Ok(())
                });
                let queue = match (took, q) {
                    (Ok(()), Some(r)) => r,
                    (Err(f), _) => Err(f),
                    (Ok(()), None) => Err(Fault::unknown(String::new())),
                };
                one.push(Kept::Sealed { root, queue, held: ids.len(), at: read_at });
            }
            Keep::Tail { root, tail, open, on } => {
                let mark = crate::home::Home::open(&root).and_then(|h| match &tail {
                    crate::restorex::Tail::Pass { .. } => {
                        if crate::ledgerx::rooted(&h) {
                            crate::restorex::forget_on(&h)?;
                        } else if let Some(on) = on {
                            crate::restorex::passed_on(&h, on)?;
                        }
                        crate::restorex::clear(&h).map(|_| None)
                    }
                    crate::restorex::Tail::NewerElsewhere { lacks, .. } => {
                        let s = crate::restorex::State::lacking(lacks);
                        crate::restorex::write(&h, &s).map(|_| Some(s))
                    }
                });
                one.push(Kept::Tail { root, mark });
                one.push(kits(&open));
            }
        }
        if !one.is_empty() && !crate::task::land_part(Done::Kept(one.clone())) {
            out.extend(one);
        }
    }
    out
}

pub fn handed_on(report: &zikaron::json::Value, broken: bool, to: Option<&str>) -> Option<usize> {
    use zikaron::v2::tokens::Key as T;
    let to = to?;
    let items = crate::auditx::items(report);
    let n = |k: T| items.iter().find(|i| i.key == k.as_str()).and_then(|i| i.count).unwrap_or(0);
    if broken || n(T::Findings) + n(T::Malformed) + n(T::Void) + n(T::Unproven) > 0 {
        return None;
    }
    let missing = crate::auditx::rows_of(report, T::Missing);
    let by_successor = |row: &zikaron::json::Value| match row.member(T::Anchors.as_str()) {
        Some(zikaron::json::Value::Arr(a)) => !a.is_empty() && a.iter().all(|x| x.member(T::Sender.as_str()).and_then(zikaron::json::Value::as_str).is_some_and(|s| s.eq_ignore_ascii_case(to))),
        _ => false,
    };
    (!missing.is_empty() && missing.iter().all(by_successor)).then_some(missing.len())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Normal,
    HandedOn(usize),
    Abnormal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetUp {
    Create,
    Fetch,
}

#[derive(Clone, Copy, Debug)]
pub struct Base {
    pub r#gen: u64,
    pub entries: Option<usize>,
    pub queue: Option<Option<[u8; 32]>>,
    pub pens: Option<[u8; 32]>,
    pub rooted: bool,
}

#[derive(Clone, Debug)]
pub struct Looked {
    pub root: std::path::PathBuf,
    pub base: Base,
    pub now: crate::task::Seen,
    pub pens: Option<[u8; 32]>,
    pub handed: Option<(Option<String>, Vec<Fault>)>,
    pub files: Option<Files>,
    pub queue: Option<Result<crate::queue::Queue, Fault>>,
    pub queue_at: u64,
    pub settings_at: u64,
}

#[derive(Clone, Debug)]
pub struct Files {
    pub rooted: bool,
    pub queue: Result<crate::queue::Queue, Fault>,
    pub settings: Result<Settings, Fault>,
    pub mark: Result<(Option<crate::restorex::State>, bool), Fault>,
}

pub fn look(root: &std::path::Path, base: Base) -> Looked {
    let (queue_at, settings_at) = (crate::reads::ticket(crate::reads::Copies::Queue), crate::reads::ticket(crate::reads::Copies::Settings));
    let home = crate::home::Home::bare(root.to_path_buf());
    let now = crate::task::Seen::of(root);
    let pens = pens_mark();
    let handed = (pens != base.pens).then(|| {
        let mut troubles = Vec::new();
        let h = handed_off(&home, &mut troubles);
        (h, troubles)
    });
    let files = match base.entries {
        Some(seen) if seen != now.entries => Some(Files {
            rooted: now.entries > 0 && (base.rooted || crate::ledgerx::rooted(&home)),
            queue: crate::queue::Queue::read(&home),
            settings: Settings::read(&home),
            mark: crate::restorex::standing(&home).and_then(|s| Ok((s, crate::restorex::read(&home)?.is_some()))),
        }),
        _ => None,
    };
    let queue = (files.is_none() && base.entries.is_some() && base.queue.is_some_and(|was| was != now.queue)).then(|| crate::queue::Queue::read(&home));
    Looked { root: root.to_path_buf(), base, now, pens, handed, files, queue, queue_at, settings_at }
}

fn pens_mark() -> Option<[u8; 32]> {
    crate::reads::touch();
    let machine = crate::home::machine_dir().ok()?;
    std::fs::read(machine.join(zikaron_seal::pens::FILE)).ok().map(|b| zikaron::cryptox::sha256(&b))
}

fn handed_off(home: &crate::home::Home, troubles: &mut Vec<Fault>) -> Option<String> {
    let machine = crate::home::machine_dir().ok()?;
    if !machine.join(zikaron_seal::pens::FILE).exists() {
        return None;
    }
    let root = crate::ledgerx::root_of(home).ok()?;
    let root: [u8; 32] = zikaron::hexfmt::decode(&root).and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok())?;
    match zikaron_seal::pens::may_write(&machine, &root) {
        Ok(()) => None,
        Err(t) if t.code == zikaron_seal::Code::HandedOff => Some(t.tail),
        Err(t) => {
            troubles.push(t.into());
            None
        }
    }
}

fn queue_mark(home: &crate::home::Home) -> Option<[u8; 32]> {
    std::fs::read(home.dir(crate::home::Slot::Settings).join(crate::queue::FILE)).ok().map(|b| zikaron::cryptox::sha256(&b))
}

fn entry_files(home: &crate::home::Home) -> usize {
    let dir = home.dir(crate::home::Slot::Ledger);
    std::fs::read_dir(&dir)
        .map(|d| d.flatten().filter(|e| zikaron_store::layout::classify(&e.file_name().to_string_lossy()) == zikaron_store::Kind::Entry).count())
        .unwrap_or(0)
}
