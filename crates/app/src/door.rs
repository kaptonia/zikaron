use crate::action::{Action, Applied};
use crate::fault::{Fault, Known};
use crate::shell::Shell;
use crate::task::{Done, Kind, Outcome, Spawned};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use zikaron_glue::door::{Reply, Request};

pub const ORIGINALS_ALL: &str = "all";

pub const REQUEST_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

pub const ANSWER_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

struct Row {
    make: fn(&Request) -> Action,
}

fn one(q: &Request, f: &str) -> String {
    q.one(f).unwrap_or_default().to_string()
}

const ROWS: [Row; zikaron_glue::doorverbs::HOME.len()] = [
    Row { make: |q| Action::Genesis { statement: one(q, "statement") } },
    Row { make: |q| Action::RecordWork { note_md: one(q, "note"), files: vec![one(q, "file")], for_: None } },
    Row {
        make: |q| Action::DraftGrant {
            draft: Box::new(crate::grantx::Draft {
                grantee: one(q, "grantee"),
                work: one(q, "work"),
                terms: one(q, "terms"),
                history: one(q, crate::entryx::HISTORY),
                from: one(q, "window-from"),
                to: one(q, "window-to"),
                scope_md: one(q, "scope"),
                upstream: one(q, "upstream"),
                ledger: String::new(),
                supersedes: one(q, "supersedes"),
                ..Default::default()
            }),
            exclusive: false,
            terms_file: None,
        },
    },
    Row { make: |q| Action::Revoke { grant: one(q, "grant"), case: one(q, "case") } },
    Row { make: |q| Action::Annotate { subject: one(q, "subject"), note_md: one(q, "note") } },
    Row { make: |q| Action::Retract { subject: one(q, "subject"), note_md: one(q, "note") } },
    Row { make: |_| Action::SendBatch { count: 0 } },
    Row {
        make: |q| Action::ExportKit {
            from: String::new(),
            to: String::new(),
            ids: q.many("entry").join("\n"),
            attach: String::new(),
            note: one(q, "note"),
            out: one(q, "out"),
            form: one(q, "as"),
            recipient: one(q, "to"),
            recipient_key: one(q, "to-key"),
            originals: one(q, "originals"), companion: false
        },
    },
    Row { make: |q| Action::VerifyWork { path: one(q, "kit"), work: String::new(), then_to: one(q, "originals-to") } },
    Row {
        make: |q| Action::ExportGrantFile { id: one(q, "grant"), to: one(q, "out"), original: one(q, "originals"), seal_to: one(q, "to"), key: None },
    },
    Row { make: |q| Action::CleanExports { yes: one(q, "yes") } },
    Row { make: |_| Action::FaceReread },
    Row { make: |q| Action::FaceHome { chain: one(q, "chain").parse().unwrap_or(0) } },
    Row { make: |q| Action::FaceAccount { platform: one(q, "platform"), handle: one(q, "handle"), note: one(q, "note") } },
    Row {
        make: |q| Action::FaceKey { address: one(q, "address"), role: one(q, "role"), note: one(q, "note"), consent: one(q, "sig"), sign_here: false, pin: crate::secret::Secret::default() },
    },
    Row { make: |q| Action::FaceName { name: one(q, "name") } },
    Row { make: |q| Action::FaceRelease { subject: one(q, "subject") } },
    Row { make: |q| Action::FaceReroot { to: one(q, "to") } },
    Row { make: |q| Action::FaceOrigin { from: one(q, "from") } },
    Row { make: |q| Action::FaceJoined { root: one(q, "root"), to: one(q, "to") } },
];

pub fn verbs() -> Vec<(&'static str, Vec<&'static str>)> {
    zikaron_glue::doorverbs::HOME.iter().map(|(v, fl)| (*v, zikaron_glue::doorverbs::names(fl))).collect()
}

pub struct Asked {
    pub req: Request,
    answer: Sender<Reply>,
}

pub struct Door {
    home: PathBuf,
    closer: zikaron_os::door::Closer,
    asked: Receiver<Asked>,
    came: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    listener: Option<std::thread::JoinHandle<()>>,
    in_hand: InHand,
}

type InHand = std::sync::Arc<std::sync::Mutex<Option<Sender<Reply>>>>;

impl Drop for Door {
    fn drop(&mut self) {
        self.close(None);
    }
}

impl Door {
    fn close(&mut self, doing: Option<Doing>) -> usize {
        self.closer.close();
        let mut told = 0;
        if let Some(d) = doing {
            let _ = d.answer().send(Reply::Closed);
            told += 1;
        }
        let taken = std::mem::replace(&mut self.asked, std::sync::mpsc::channel().1);
        for a in taken.try_iter() {
            let _ = a.answer.send(Reply::Closed);
            told += 1;
        }
        drop(taken);
        if let Some(tx) = self.in_hand.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = tx.send(Reply::Closed);
        }
        if let Some(h) = self.listener.take() {
            let _ = h.join();
        }
        told
    }

    pub fn home(&self) -> &std::path::Path {
        &self.home
    }

    pub fn came(&self) -> usize {
        self.came.load(std::sync::atomic::Ordering::SeqCst)
    }
}

pub enum Doing {
    Record { answer: Sender<Reply>, serial: u64 },
    Kit { answer: Sender<Reply>, serial: u64 },
    Verify { answer: Sender<Reply>, serial: u64, then_to: String },
    Extract { answer: Sender<Reply>, serial: u64, verified: Box<Reply> },
    Estimate { answer: Sender<Reply>, count: usize, serial: u64 },
    Follow { answer: Sender<Reply>, req: Request, serial: Option<u64>, again: bool },
    Send { answer: Sender<Reply>, serial: u64 },
    Gate { answer: Sender<Reply>, serial: u64 },
    Clean { answer: Sender<Reply>, serial: u64 },
    Persona { answer: Sender<Reply>, serial: u64 },
    Joined { answer: Sender<Reply>, serial: u64 },
    Fresh { answer: Sender<Reply>, serial: u64, then: Box<Action> },
}

pub enum Turn {
    Act(Action),
    Send { count: usize },
    Await,
    Follow,
}

pub type Waker = std::sync::Arc<dyn Fn() + Send + Sync>;

pub fn turn_of(shell: &Shell, q: &Request) -> Result<Turn, Fault> {
    let Some(row) = zikaron_glue::doorverbs::HOME.iter().position(|(v, _)| *v == q.verb).map(|i| &ROWS[i]) else {
        return Err(Fault::known(Known::DoorUnread, q.verb.clone()));
    };
    let act = (row.make)(q);
    if let Action::SendBatch { .. } = act {
        if shell.machine.cli_anchor == zikaron_seal::machine::CliAnchor::Send && shell.queue.submitted().is_empty() && !shell.door_followed && shell.queue_uncovered_now() {
            return Ok(Turn::Follow);
        }
        let next = shell.sendable_here();
        let count = shell.queue.sendable();
        return Ok(match zikaron_seal::machine::turn(shell.machine.cli_anchor, next) {
            zikaron_seal::machine::Turn::Send => Turn::Send { count: next },
            zikaron_seal::machine::Turn::Nothing => Turn::Await,
            zikaron_seal::machine::Turn::Leave => Turn::Act(Action::SendAsked { count }),
        });
    }
    Ok(Turn::Act(act))
}

pub fn refused(f: &Fault) -> Reply {
    if let Some((names, tx, block)) = f.on_chain() {
        return Reply::OnChain { names: names.clone(), tx: tx.clone(), block: *block };
    }
    let tail = f.tail_in(crate::lang::Lang::En).to_string();
    if let Some(tx) = f.out() {
        return Reply::Unheard { tx: tx.to_string(), detail: tail };
    }
    if let Some(crate::fault::Owned::NotIncluded { entry, ledger }) = f.owned_by() {
        return Reply::Noticed { entry_id: entry.clone(), ledger: ledger.clone(), tx: tail, notice: crate::sendx::NOT_INCLUDED.to_string(), withdrawn: false };
    }
    let (code, network, (zh, en)) = match f.which() {
        Some(k) => {
            let (zh, en) = crate::lang::both(f.what_key().unwrap_or(k.what()));
            (k.as_str().to_string(), f.unanswered(), (with_evidence(zh, f.tail_in(crate::lang::Lang::Zh), true), with_evidence(en, f.tail_in(crate::lang::Lang::En), false)))
        }
        None => (f.class().as_str().to_string(), false, (f.said().to_string(), f.said().to_string())),
    };
    if let Some(r) = f.rule() {
        let (zh, en) = match f.which() {
            Some(k) => crate::lang::both(f.what_key().unwrap_or(k.what())),
            None => (f.said(), f.said()),
        };
        return Reply::RuleRefused { rule: r.rule, token: r.token.map(str::to_string), entry_id: r.entry_id.clone(), names: r.names.clone(), zh: zh.to_string(), en: en.to_string() };
    }
    let own = match f.owned_by() {
        Some(crate::fault::Owned::Reason(o)) => Some(*o),
        _ => None,
    };
    let tail = match (own, f.machine()) {
        (Some(zikaron_glue::door::Own::ChainUnread | zikaron_glue::door::Own::Face), Some(m)) => m.to_string(),
        _ => tail,
    };
    Reply::Refused { code, tail, network, zh, en, own }
}

pub fn face_reply(state: &crate::facex::State) -> Reply {
    let unread = |tail: String| {
        let (zh, en) = crate::lang::both(Known::FaceUnread.what());
        Reply::Refused { code: Known::FaceUnread.as_str().to_string(), tail, network: true, zh: zh.to_string(), en: en.to_string(), own: Some(zikaron_glue::door::Own::Face) }
    };
    match state {
        crate::facex::State::Read(b) => Reply::Face {
            table: String::from_utf8_lossy(&zikaron::json::canon_bytes(&b.table.value)).into_owned(),
            moment: b.moment,
            omitted: crate::chainx::face_omitted(&b.left),
        },
        crate::facex::State::Unread => unread("unread".to_string()),
        crate::facex::State::NotRead(m) => unread(crate::facex::missing_machine(m)),
    }
}

pub fn joined_reply(j: &crate::facex::Joined) -> Reply {
    let read = |p: &crate::facex::Pair| -> Result<zikaron_glue::door::ReadOf, Reply> {
        match &p.state {
            crate::facex::State::Read(b) => Ok(zikaron_glue::door::ReadOf { root: p.other.hex(), moment: b.moment, omitted: crate::chainx::face_omitted(&b.left) }),
            crate::facex::State::Unread => Err(face_reply(&p.state)),
            crate::facex::State::NotRead(m) => {
                let (zh, en) = crate::lang::both(Known::FaceUnread.what());
                Err(Reply::Refused { code: Known::FaceUnread.as_str().to_string(), tail: format!("root {} {}", p.other.hex(), crate::facex::missing_machine(m)), network: true, zh: zh.to_string(), en: en.to_string(), own: Some(zikaron_glue::door::Own::Face) })
            }
        }
    };
    match (read(&j.first), read(&j.second), j.joined()) {
        (Ok(a), Ok(b), Some(joined)) => Reply::Joined { joined, read: vec![a, b] },
        (Err(r), _, _) | (_, Err(r), _) => r,
        (Ok(_), Ok(_), None) => face_reply(&crate::facex::State::Unread),
    }
}

fn with_evidence(said: &str, evidence: &str, zh: bool) -> String {
    let evidence = evidence.trim();
    if evidence.is_empty() {
        return said.to_string();
    }
    match (zh, said.trim_end()) {
        (true, s) if s.ends_with('。') => format!("{s}{evidence}"),
        (true, s) => format!("{s}:{evidence}"),
        (false, s) if s.ends_with('.') => format!("{s} {evidence}"),
        (false, s) => format!("{s}: {evidence}"),
    }
}

fn wait_at(door: zikaron_os::door::Listener, asked: Sender<Asked>, wake: Option<Waker>, came: std::sync::Arc<std::sync::atomic::AtomicUsize>, in_hand: InHand) {
    use zikaron_glue::door::{put, take, Unread};
    loop {
        let mut s = match door.accept() {
            Ok(Some(s)) => s,
            Ok(None) => return,
            Err(_) => continue,
        };
        let _ = s.set_read_deadline(Some(REQUEST_WAIT));
        let reply = match take(&mut s) {
            Ok(bytes) => match Request::of_bytes(&bytes) {
                Some(req) => {
                    let (tx, rx) = std::sync::mpsc::channel();
                    *in_hand.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx.clone());
                    let reply = if asked.send(Asked { req, answer: tx }).is_err() {
                        Reply::Closed
                    } else {
                        came.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if let Some(w) = &wake {
                            w();
                        }
                        rx.recv().unwrap_or(Reply::Closed)
                    };
                    *in_hand.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    reply
                }
                None => refused(&Fault::known(Known::DoorUnread, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::TailDoorNotForm).to_string()))),
            },
            Err(Unread::TooLarge(n)) => refused(&Fault::known(Known::DoorUnread, crate::lang::both_of(|| crate::lang::fill1(crate::lang::Key::TailDoorTooLarge, &n.to_string())))),
            Err(Unread::Short) | Err(Unread::Io(_)) => continue,
        };
        let _ = s.set_write_deadline(Some(ANSWER_WAIT));
        let _ = put(&mut s, &reply.to_bytes());
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        if let Some(mut d) = self.door.take() {
            d.close(self.door_doing.take());
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DoorWhy {
    TooLong,
    Refused(String),
}

impl Shell {
    pub fn door_why(&self) -> Option<DoorWhy> {
        let (home, f) = self.door_shut.as_ref()?;
        if self.door_wanted().as_ref() != Some(home) || self.door.is_some() {
            return None;
        }
        Some(if self.door_too_long { DoorWhy::TooLong } else { DoorWhy::Refused(f.tail().to_string()) })
    }

    fn door_wanted(&self) -> Option<PathBuf> {
        let writer = self.lock.as_ref().map(|l| l.mode() == crate::lock::Mode::Writer).unwrap_or(false);
        let through = self.machine.cli_path() == crate::machine::CliPath::Desktop;
        if self.door_off || !self.unlocked() || !writer || !through {
            return None;
        }
        self.home.as_ref().map(|h| h.root().to_path_buf())
    }

    pub fn door_sync(&mut self) {
        let want = self.door_wanted();
        let have = self.door.as_ref().map(|d| d.home.clone());
        if want == have {
            if want.is_none() {
                self.door_shut = None;
                self.door_place = None;
            }
            return;
        }
        if let Some(mut d) = self.door.take() {
            self.door_answered += d.close(self.door_doing.take());
            self.tasks.carry(None);
        }
        let Some(home) = want else {
            self.door_shut = None;
            self.door_place = None;
            return;
        };
        match self.door_open(&home) {
            Ok(d) => {
                self.door = Some(d);
                self.door_shut = None;
            }
            Err((f, long)) => {
                self.door_shut = Some((home, f));
                self.door_too_long = long;
            }
        }
    }

    fn door_place_for(&mut self, home: &std::path::Path) -> Result<std::path::PathBuf, Fault> {
        if let Some((h, place)) = &self.door_place {
            if h == home {
                return Ok(place.clone());
            }
        }
        let machine = crate::home::machine_dir()?;
        let place = zikaron_glue::door::place(&machine, home);
        self.door_place = Some((home.to_path_buf(), place.clone()));
        Ok(place)
    }

    fn door_open(&mut self, home: &std::path::Path) -> Result<Door, (Fault, bool)> {
        let place = self.door_place_for(home).map_err(|f| (f, false))?;
        let (listener, closer) = zikaron_os::door::listen(&place).map_err(|e| (Fault::known(Known::DoorShut, e.to_string()), zikaron_os::door::too_long(&place)))?;
        let (tx, rx) = std::sync::mpsc::channel();
        let wake = self.door_waker.clone();
        let came = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = came.clone();
        let in_hand: InHand = std::sync::Arc::new(std::sync::Mutex::new(None));
        let held = in_hand.clone();
        let listener = match crate::task::start_named("door", move || wait_at(listener, tx, wake, counted, held)) {
            Ok(h) => h,
            Err(e) => {
                closer.close();
                return Err((Fault::known(Known::DoorShut, e.to_string()), false));
            }
        };
        Ok(Door { home: home.to_path_buf(), closer, asked: rx, came, listener: Some(listener), in_hand })
    }

    pub fn door_turn(&mut self, landed: &[Outcome]) {
        if let Some(doing) = self.door_doing.take() {
            self.door_doing = self.door_step(doing, landed);
        }
        self.door_sync();
        while self.door_doing.is_none() {
            self.tasks.carry(None);
            let Some(a) = self.door.as_ref().and_then(|d| d.asked.try_recv().ok()) else { return };
            self.door_do(a);
        }
    }

    fn door_do(&mut self, a: Asked) {
        let Asked { req, answer } = a;
        let turn = match turn_of(self, &req) {
            Ok(t) => t,
            Err(f) => {
                self.door_send(&answer, refused(&f));
                return;
            }
        };
        match turn {
            Turn::Send { count } => {
                self.tasks.carry(Some(zikaron_anchor::action::fresh(Some(crate::checkedx::settled_keeper()))));
                let got = crate::action::apply(self, Action::EstimateGas { count });
                match (got, self.tasks.serial_of(Kind::Gas)) {
                    (Applied::Started(Kind::Gas), Some(serial)) => self.door_doing = Some(Doing::Estimate { answer, count, serial }),
                    (other, _) => self.door_answer(answer, other),
                }
            }
            Turn::Follow => {
                let started = crate::action::apply(self, Action::Audit);
                match (started, self.tasks.serial_of(Kind::Audit)) {
                    (Applied::Started(Kind::Audit) | Applied::Refused(Kind::Audit), Some(serial)) => self.door_doing = Some(Doing::Follow { answer, req, serial: Some(serial), again: false }),
                    _ => {
                        self.door_followed = true;
                        self.door_do(Asked { req, answer });
                        self.door_followed = false;
                    }
                }
            }
            Turn::Await => {
                if let (true, Some(serial)) = (self.tasks.in_flight(Kind::Anchor), self.tasks.serial_of(Kind::Anchor)) {
                    self.door_doing = Some(Doing::Send { answer, serial });
                    return;
                }
                match (crate::action::await_out(self), self.tasks.serial_of(Kind::Anchor)) {
                    (Ok(Spawned::Started), Some(serial)) => self.door_doing = Some(Doing::Send { answer, serial }),
                    (Ok(_), _) => self.door_send(&answer, refused(&Fault::known(Known::InFlight, Kind::Anchor.as_str().to_string()))),
                    (Err(f), _) => self.door_send(&answer, refused(&f)),
                }
            }
            Turn::Act(act) => {
                if act.pin_asked().is_some() {
                    self.door_send(&answer, Reply::OnDesktop);
                    return;
                }
                if matches!(act, Action::FaceHome { .. } | Action::FaceAccount { .. } | Action::FaceKey { .. } | Action::FaceName { .. } | Action::FaceRelease { .. } | Action::FaceReroot { .. } | Action::FaceOrigin { .. }) {
                    if let (Applied::Started(Kind::Persona), Some(serial)) = (crate::action::apply(self, Action::FaceReread), self.tasks.serial_of(Kind::Persona)) {
                        self.door_doing = Some(Doing::Fresh { answer, serial, then: Box::new(act) });
                        return;
                    }
                }
                self.door_act(answer, act);
            }
        }
    }

    fn door_act(&mut self, answer: Sender<Reply>, act: Action) {
        let then_to = match &act {
            Action::VerifyWork { then_to, .. } => then_to.trim().to_string(),
            _ => String::new(),
        };
        self.door_acting = true;
        let got = crate::action::apply(self, act);
        self.door_acting = false;
        let serial_of = |k: Kind| self.tasks.serial_of(k);
        match got {
            Applied::Started(Kind::Record) if serial_of(Kind::Record).is_some() => self.door_doing = serial_of(Kind::Record).map(|serial| Doing::Record { answer, serial }),
            Applied::Started(Kind::Kit) if serial_of(Kind::Kit).is_some() => self.door_doing = serial_of(Kind::Kit).map(|serial| Doing::Kit { answer, serial }),
            Applied::Started(Kind::Verify) if serial_of(Kind::Verify).is_some() => self.door_doing = serial_of(Kind::Verify).map(|serial| Doing::Verify { answer, serial, then_to }),
            Applied::Started(Kind::Gate) if serial_of(Kind::Gate).is_some() => self.door_doing = serial_of(Kind::Gate).map(|serial| Doing::Gate { answer, serial }),
            Applied::Started(k @ (Kind::CleanList | Kind::Clean)) if serial_of(k).is_some() => self.door_doing = serial_of(k).map(|serial| Doing::Clean { answer, serial }),
            Applied::Started(Kind::Persona) if serial_of(Kind::Persona).is_some() => self.door_doing = serial_of(Kind::Persona).map(|serial| Doing::Persona { answer, serial }),
            Applied::Started(Kind::FaceJoined) if serial_of(Kind::FaceJoined).is_some() => self.door_doing = serial_of(Kind::FaceJoined).map(|serial| Doing::Joined { answer, serial }),
            other => self.door_answer(answer, other),
        }
    }

    fn door_send(&mut self, answer: &Sender<Reply>, r: Reply) {
        self.door_answered += 1;
        let _ = answer.send(r);
    }

    fn door_answer(&mut self, answer: Sender<Reply>, got: Applied) {
        let reply = self.door_reply(&got);
        if !matches!(got, Applied::Trouble(_)) {
            self.door_told.push(got);
        }
        self.door_send(&answer, reply);
    }

    fn door_reply(&self, got: &Applied) -> Reply {
        let wrote = |id: &str| self.door_wrote(id);
        match got {
            Applied::Genesised { id, .. }
            | Applied::Annotated(id)
            | Applied::Retracted { id, .. }
            | Applied::Recorded { id, .. }
            | Applied::Granted { id, .. }
            | Applied::Revoked { id, notice: None, .. } => wrote(id),
            Applied::Revoked { id, notice: Some(n), .. } => match self.home.as_ref() {
                Some(home) => Reply::Noticed { entry_id: id.clone(), ledger: home.dir(crate::home::Slot::Ledger).display().to_string(), tx: n.tx.clone(), notice: n.back.clone(), withdrawn: n.withdrawn },
                None => refused(&Fault::known(Known::NoHome, String::new())),
            },
            Applied::RecordedBatch { ids, stopped, .. } => match (ids.first(), stopped) {
                (Some(id), _) => wrote(id),
                (None, Some((_, _, f))) => refused(f),
                (None, None) => refused(&Fault::known(Known::OutcomeLost, crate::lang::both_of(|| crate::lang::t(crate::lang::Key::TailDoorNothingCame).to_string()))),
            },
            Applied::SendAsked { count } => Reply::Queued { count: *count as u64 },
            Applied::Queued { id, .. } => wrote(id),
            Applied::GrantFileExported { path, files, kit_id, taken, left_out, sealed, .. } => Reply::GrantPackage { path: path.clone(), kit_id: kit_id.clone(), files: *files as u64, originals: taken.clone(), left_out: left_out.clone(), sealed: *sealed },
            Applied::Trouble(f) => refused(f),
            Applied::Refused(k) => refused(&Fault::known(Known::InFlight, k.as_str().to_string())),
            other => refused(&Fault::known(Known::OutcomeLost, format!("{other:?}"))),
        }
    }

    fn door_wrote(&self, id: &str) -> Reply {
        let Some(home) = self.home.as_ref() else {
            return refused(&Fault::known(Known::NoHome, String::new()));
        };
        let ledger = home.dir(crate::home::Slot::Ledger).display().to_string();
        match crate::ledgerx::head(home) {
            Ok(Some((seq, head))) if head.eq_ignore_ascii_case(id) => Reply::Wrote { entry_id: id.to_string(), ledger, seq },
            Ok(_) => refused(&Fault::known(Known::OutcomeLost, id.to_string())),
            Err(f) => refused(&f),
        }
    }

    fn door_step(&mut self, doing: Doing, landed: &[Outcome]) -> Option<Doing> {
        let mine = |serial: u64| landed.iter().find(|o| o.serial == serial);
        let unexpected = |k: Kind| refused(&Fault::known(Known::OutcomeLost, k.as_str().to_string()));
        match doing {
            Doing::Record { answer, serial } => {
                if mine(serial).is_none() {
                    return Some(Doing::Record { answer, serial });
                }
                match self.said.remove(&Kind::Record) {
                    Some(got) => self.door_answer(answer, got),
                    None => self.door_send(&answer, Reply::Closed),
                }
                None
            }
            Doing::Kit { answer, serial } => {
                let Some(o) = mine(serial) else { return Some(Doing::Kit { answer, serial }) };
                let reply = match &o.result {
                    Ok(Done::Kit { path, kit_id, entries, files, dropped, originals, left_out, unreadable, withdrawn, .. }) => Reply::Kit {
                        kit_id: kit_id.clone(),
                        path: path.clone(),
                        entries: *entries as u64,
                        files: *files as u64,
                        dropped: dropped.clone(),
                        originals: originals.clone(),
                        left_out: left_out.iter().cloned().chain(unreadable.iter().map(|(p, _)| p.clone())).chain(withdrawn.iter().cloned()).collect(),
                    },
                    Ok(_) => unexpected(Kind::Kit),
                    Err(f) => refused(f),
                };
                self.door_send(&answer, reply);
                None
            }
            Doing::Verify { answer, serial, then_to } => {
                let Some(o) = mine(serial) else { return Some(Doing::Verify { answer, serial, then_to }) };
                let reply = match &o.result {
                    Ok(Done::Verified(v)) => verified_reply(v),
                    Ok(_) => unexpected(Kind::Verify),
                    Err(f) => refused(f),
                };
                if let (Reply::Verified { .. }, false, Ok(Done::Verified(v))) = (&reply, then_to.is_empty(), &o.result) {
                    let take = Action::ExtractOriginals { kit: v.path.clone(), to: then_to };
                    return match (crate::action::apply(self, take), self.tasks.serial_of(Kind::Extract)) {
                        (Applied::Started(Kind::Extract), Some(serial)) => Some(Doing::Extract { answer, serial, verified: Box::new(reply) }),
                        (other, _) => {
                            self.door_answer(answer, other);
                            None
                        }
                    };
                }
                self.door_send(&answer, reply);
                None
            }
            Doing::Extract { answer, serial, verified } => {
                let Some(o) = mine(serial) else { return Some(Doing::Extract { answer, serial, verified }) };
                let reply = match (&o.result, *verified) {
                    (Ok(Done::Extracted { landed, .. }), Reply::Verified { kit_id, entries, records, unanswered, .. }) => Reply::Verified {
                        kit_id,
                        entries,
                        records,
                        taken: Some(zikaron_glue::door::Taken { path: landed.to.display().to_string(), names: landed.names.clone() }),
                        unanswered,
                    },
                    (Ok(_), _) => unexpected(Kind::Extract),
                    (Err(f), _) => refused(f),
                };
                self.door_send(&answer, reply);
                None
            }
            Doing::Follow { answer, req, serial: Some(serial), again } => {
                let Some(o) = mine(serial) else { return Some(Doing::Follow { answer, req, serial: Some(serial), again }) };
                if o.result.is_ok() && self.catch_up_from_report() {
                    return Some(Doing::Follow { answer, req, serial: None, again });
                }
                if o.result.is_ok() && !again && !self.audit_in_step() {
                    if let (Applied::Started(Kind::Audit), Some(next)) = (crate::action::apply(self, Action::Audit), self.tasks.serial_of(Kind::Audit)) {
                        return Some(Doing::Follow { answer, req, serial: Some(next), again: true });
                    }
                }
                self.door_followed = true;
                self.door_do(Asked { req, answer });
                self.door_followed = false;
                self.door_doing.take()
            }
            Doing::Follow { answer, req, serial: None, again } => {
                if self.catching_up() {
                    return Some(Doing::Follow { answer, req, serial: None, again });
                }
                self.door_followed = true;
                self.door_do(Asked { req, answer });
                self.door_followed = false;
                self.door_doing.take()
            }
            Doing::Estimate { answer, count, serial } => {
                if mine(serial).is_none() {
                    return Some(Doing::Estimate { answer, count, serial });
                }
                match self.said.remove(&Kind::Gas) {
                    Some(Applied::Gas { .. }) => match (crate::action::apply(self, Action::SendBatch { count }), self.tasks.serial_of(Kind::Anchor)) {
                        (Applied::Started(Kind::Anchor), Some(serial)) => return Some(Doing::Send { answer, serial }),
                        (other, _) => self.door_answer(answer, other),
                    },
                    Some(other) => self.door_answer(answer, other),
                    None => self.door_send(&answer, Reply::Closed),
                }
                None
            }
            Doing::Gate { answer, serial } => {
                let Some(o) = mine(serial) else { return Some(Doing::Gate { answer, serial }) };
                match (&o.result, self.gate_said.take()) {
                    (Err(f), _) => self.door_send(&answer, refused(f)),
                    (Ok(_), Some(Applied::Started(Kind::Gate))) => {
                        if let Some(next) = self.tasks.serial_of(Kind::Gate) {
                            return Some(Doing::Gate { answer, serial: next });
                        }
                        self.door_send(&answer, Reply::Closed)
                    }
                    (Ok(_), Some(got)) => self.door_answer(answer, got),
                    (Ok(_), None) => self.door_send(&answer, Reply::Closed),
                }
                None
            }
            Doing::Persona { answer, serial } => {
                let Some(o) = mine(serial) else { return Some(Doing::Persona { answer, serial }) };
                let reply = match &o.result {
                    Ok(_) => face_reply(&self.persona),
                    Err(f) => refused(f),
                };
                self.door_send(&answer, reply);
                None
            }
            Doing::Fresh { answer, serial, then } => {
                if mine(serial).is_none() {
                    return Some(Doing::Fresh { answer, serial, then });
                }
                self.door_act(answer, *then);
                self.door_doing.take()
            }
            Doing::Joined { answer, serial } => {
                let Some(o) = mine(serial) else { return Some(Doing::Joined { answer, serial }) };
                let reply = match &o.result {
                    Ok(Done::FaceJoined(j)) => joined_reply(j),
                    Ok(_) => unexpected(Kind::FaceJoined),
                    Err(f) => refused(f),
                };
                self.door_send(&answer, reply);
                None
            }
            Doing::Clean { answer, serial } => {
                let Some(o) = mine(serial) else { return Some(Doing::Clean { answer, serial }) };
                let reply = match &o.result {
                    Ok(Done::Cleaned { plan, done }) => cleaned_reply(plan, *done),
                    Ok(_) => unexpected(Kind::Clean),
                    Err(f) => refused(f),
                };
                self.door_send(&answer, reply);
                None
            }
            Doing::Send { answer, serial } => {
                let Some(o) = mine(serial) else { return Some(Doing::Send { answer, serial }) };
                let reply = match &o.result {
                    Ok(Done::Submitted { tx, .. }) => match self.tasks.serial_of(Kind::Anchor) {
                        Some(next) if next != serial => return Some(Doing::Send { answer, serial: next }),
                        _ => Reply::NotYet { tx: tx.clone(), waited: 0 },
                    },
                    Ok(Done::Anchored { tx, state, voided, waited, .. }) => match crate::action::receipt_of(state) {
                        _ if *voided => Reply::Voided { tx: tx.clone() },
                        Some((1, block)) => Reply::Anchored { tx: tx.clone(), block },
                        Some((status, _)) => Reply::Reverted { tx: tx.clone(), status },
                        None if crate::action::receipt_unheard(state) => Reply::Unheard { tx: tx.clone(), detail: state.clone() },
                        None => Reply::NotYet { tx: tx.clone(), waited: *waited },
                    },
                    Ok(_) => unexpected(Kind::Anchor),
                    Err(f) => refused(f),
                };
                self.door_send(&answer, reply);
                None
            }
        }
    }
}

fn verified_reply(v: &crate::verifyx::Verified) -> Reply {
    match &v.kit {
        Some(k) if k.ok => Reply::Verified {
            kit_id: k.kit_id.clone(),
            entries: k.entries as u64,
            records: v.records.iter().map(|r| zikaron_glue::door::Record { entry_id: r.id.clone(), content: r.content.clone(), original: r.original, chain: r.chain, deleted: r.deleted.as_ref().map(|d| d.by.clone()) }).collect(),
            taken: None,
            unanswered: v.unanswered.clone(),
        },
        Some(k) => Reply::KitRefused { verdict: k.verdict.clone(), subject: Some(k.subject.clone()).filter(|x| !x.is_empty()) },
        None => Reply::KitRefused { verdict: zikaron_kit::tokens::KitFailToken::ManifestAbsent.as_str().to_string(), subject: None },
    }
}

pub fn cleaned_reply(plan: &zikaron_seal::exports::Plan, done: bool) -> Reply {
    let row = |i: &zikaron_seal::exports::Item, why: Option<&zikaron_seal::exports::Why>| zikaron_glue::door::Cleared { kind: i.kind.word().to_string(), path: i.path.clone(), link: i.link.clone(), why: why.map(|w| w.word().to_string()) };
    Reply::Cleaned { done, removed: plan.remove.iter().map(|i| row(i, None)).collect(), kept: plan.keep.iter().map(|(i, w)| row(i, Some(w))).collect() }
}

impl Doing {
    fn answer(&self) -> &Sender<Reply> {
        match self {
            Doing::Record { answer, .. }
            | Doing::Kit { answer, .. }
            | Doing::Verify { answer, .. }
            | Doing::Extract { answer, .. }
            | Doing::Estimate { answer, .. }
            | Doing::Follow { answer, .. }
            | Doing::Gate { answer, .. }
            | Doing::Clean { answer, .. }
            | Doing::Persona { answer, .. }
            | Doing::Joined { answer, .. }
            | Doing::Fresh { answer, .. }
            | Doing::Send { answer, .. } => answer,
        }
    }
}

#[cfg(test)]
mod own_reasons {
    use super::*;
    use crate::fault::Owned;
    use zikaron_glue::door::Own;

    #[test]
    fn every_desktop_code_has_one_row_in_the_door_table() {
        let mut want: Vec<&str> = Known::ALL.iter().map(|k| k.as_str()).collect();
        let mut got: Vec<&str> = zikaron_glue::door::DESK.iter().map(|(c, _, _)| *c).collect();
        want.sort_unstable();
        got.sort_unstable();
        let n = got.len();
        got.dedup();
        assert_eq!(n, got.len(), "a code twice in the table");
        assert_eq!(got, want, "the table is the desktop's closed table of codes");
        for k in Known::ALL {
            match refused(&Fault::known(k, "t")) {
                Reply::Refused { code, .. } => assert!(zikaron_glue::door::desk_reason(&code).is_some(), "{k:?}"),
                _ => {}
            }
        }
    }

    #[test]
    fn what_refused_goes_through_the_door_by_the_command_lines_own_reason() {
        let own = |f: &Fault| match refused(f) {
            Reply::Refused { own, .. } => own,
            other => panic!("a refusal: {other:?}"),
        };
        let issue = Fault::known(Known::BookCode, "11155111").owned(Owned::Reason(Own::IssueRefused));
        assert!(matches!(refused(&issue), Reply::Refused { ref code, network: false, own: Some(Own::IssueRefused), .. } if code == "BOOK_CODE"));
        let unreached = Fault::known(Known::Unreachable, "node").owned(Owned::Reason(Own::SendRefused));
        assert!(matches!(refused(&unreached), Reply::Refused { network: true, own: Some(Own::SendRefused), .. }));
        for (k, o) in [(Known::LedgerNotFetched, Some(Own::ReadOnly)), (Known::NewerElsewhere, Some(Own::ReadOnly)), (Known::Broken, Some(Own::ReadOnly)), (Known::HandedOff, Some(Own::HandedOff)), (Known::Unreachable, Some(Own::ChainUnread)), (Known::Disagree, Some(Own::ChainUnread)), (Known::NoChainId, Some(Own::ChainUnread)), (Known::NoRegistry, Some(Own::ChainUnread)), (Known::ReadOnly, None)] {
            assert_eq!(own(&Fault::known(k, "t").gate_owned()), o, "{k:?}");
        }
        match refused(&Fault::from(crate::chainx::disagree_trouble("2 differ")).gate_owned()) {
            Reply::Refused { code, tail, own: Some(Own::ChainUnread), .. } => assert_eq!((code.as_str(), tail.as_str()), ("DISAGREE", "2 differ")),
            other => panic!("the chain not read: {other:?}"),
        }
        assert_eq!(own(&Fault::known(Known::HandedOver, "0x1")), None, "another cause: the desktop's own");
        assert_eq!(Known::NotMoved.as_str(), zikaron_glue::door::TIP_NOT_MOVED);
        let late = Fault::known(Known::NotIncluded, "0xt").owned(Owned::NotIncluded { entry: "0xe".into(), ledger: "/l".into() });
        assert_eq!(refused(&late), Reply::Noticed { entry_id: "0xe".into(), ledger: "/l".into(), tx: "0xt".into(), notice: crate::sendx::NOT_INCLUDED.into(), withdrawn: false });
    }
}

#[cfg(test)]
mod rows {
    use super::*;

    #[test]
    fn each_row_of_the_door_makes_the_verb_the_one_table_names_there() {
        let empty = Request { verb: String::new(), home: String::new(), args: Vec::new() };
        let made: Vec<&str> = ROWS.iter().map(|r| (r.make)(&empty).verb().unwrap_or_default()).collect();
        let named: Vec<&str> = zikaron_glue::doorverbs::HOME.iter().map(|(v, _)| *v).collect();
        assert_eq!(made, named);
    }
}

