//! The command-line door: while the desktop is unlocked and holds a home open for writing, a command naming
//! that home (`zikaron <verb> --home <home>`) is forwarded over a local channel (`zikaron_os::door`) and run by
//! this shell through the same [`crate::action::apply`] a click goes through: same lock gate, trace marks,
//! toast and redraw. There is one ledger and one state, so the window shows the result at once.
//!
//! - **Open or closed** is derived from the shell's state ([`Shell::door_sync`]): unlocked, a home open, and
//!   this instance its writer. Locking, closing or switching the home, or quitting closes the door, and a
//!   waiting request is answered [`Reply::Closed`]. A door that fails to open (path too long for the system,
//!   the system refusing) is recorded in [`Shell::door_shut`] and retried at the next drain; a command that
//!   arrives is told why by name. Each home has its own door (`zikaron_glue::door::place`); a home this
//!   instance does not write has none.
//! - **Access** is limited to the same user; the system checks this on both ends (`zikaron_os::door`).
//! - **The listener thread** is started like every thread in the app (`task::start_named`). It only reads a
//!   request, hands it to the shell and writes the reply back, one request at a time in arrival order.
//! - **The shell runs the request** in its own turn ([`Shell::door_turn`], at the end of every drain): the
//!   verb and flags become one [`Action`] by reversing `Action::verb` ([`turn_of`]). An action that needs the
//!   passcode is never run through the door ([`Reply::OnDesktop`]); one with a background half is answered
//!   when that half lands, never on a timer.
//! - **The answer** is the facts of the outcome ([`Reply`]); the command line renders them where it renders
//!   every answer.

use crate::action::{Action, Applied};
use crate::fault::{Fault, Known};
use crate::shell::Shell;
use crate::task::{Done, Kind, Outcome};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use zikaron_glue::door::{Reply, Request};

/// How long the listener waits for a connected client's request before dropping it, so a silent client
/// cannot hold up the next one.
pub const REQUEST_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// How long the listener waits for a client to take its answer, so one that does not read cannot hold the door.
pub const ANSWER_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// One verb accepted through the door: the flags it takes besides `--home`, and how a request becomes an
/// action. The verb name is not written here; it is the one `Action::verb` gives the row's action ([`verbs`],
/// [`turn_of`]), so the two can never disagree.
struct Row {
    flags: &'static [&'static str],
    make: fn(&Request) -> Action,
}

fn one(q: &Request, f: &str) -> String {
    q.one(f).unwrap_or_default().to_string()
}

/// The closed table of door verbs, in the command line's verb order. A verb not listed is not accepted through
/// the door (the command line refuses `--home` with it), and a flag not in its row is not accepted. Handing the
/// ledger to another key (`succeed`) is not listed: final and rare, it is done on the desktop by hand only.
/// `CLI-SCHEMA.md` §11 documents it (checked by the tests).
const ROWS: [Row; 11] = [
    Row { flags: &["statement"], make: |q| Action::Genesis { statement: one(q, "statement") } },
    Row { flags: &["file", "note"], make: |q| Action::RecordWork { note_md: one(q, "note"), files: vec![one(q, "file")], for_: None } },
    Row {
        flags: &["grantee", "work", "terms", crate::entryx::HISTORY, "window-from", "window-to", "scope", "upstream"],
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
            }),
            exclusive: false,
            terms_file: None,
        },
    },
    Row { flags: &["grant", "case"], make: |q| Action::Revoke { grant: one(q, "grant"), case: one(q, "case") } },
    // Adopting anchors: the command line passes the `--anchors` file's JSON unchanged; it is turned into the
    // adoption page's rows here ([`anchor_rows`]) and validated by the page's own parser.
    Row {
        flags: &["anchors", "attestor", "attestation"],
        make: |q| Action::AdoptAnchors { rows: anchor_rows(&one(q, "anchors")), attestor: one(q, "attestor"), attestation: one(q, "attestation") },
    },
    Row { flags: &[], make: |_| Action::AttestFor { text: String::new(), pin: crate::secret::Secret::default() } },
    Row { flags: &["subject", "note"], make: |q| Action::Annotate { subject: one(q, "subject"), note_md: one(q, "note") } },
    Row { flags: &["subject", "note"], make: |q| Action::Retract { subject: one(q, "subject"), note_md: one(q, "note") } },
    // Sending the queue: whether it is sent or left for the user depends on the setting ([`turn_of`]).
    Row { flags: &[], make: |_| Action::SendBatch { count: 0 } },
    // `--as`: the form's word as given (`folder`, `file`; none is one file), judged by the action layer. `--to` and
    // `--to-key`: the recipient a one-file package is sealed to (an address, its key found on this desktop's networks;
    // or a key), judged by the action layer too, as the export sheet's two fields are. The chosen records' originals
    // go in as the export page takes them unless `--originals none` (the word judged by the action layer too).
    Row {
        flags: &["entry", "note", "out", "as", "to", "to-key", "originals"],
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
            originals: one(q, "originals"),
        },
    },
    // Verifying a record package on this desktop's networks, as the verify page does (a package addressed to one of
    // its identities opened with it first); `--originals-to`: the originals then taken out there, as the page's
    // "take out originals" does, once the verification has landed ([`Shell::door_step`]).
    Row { flags: &["kit", "originals-to"], make: |q| Action::VerifyWork { path: one(q, "kit"), work: String::new(), then_to: one(q, "originals-to") } },
];

/// Converts the JSON array from a `--anchors` file into the adoption page's rows, one line per element:
/// chain id, transaction, payload kind and content, space-separated. Nothing is validated or reshaped here. An
/// element becomes a line only when it has exactly those four members, an integer chain id and non-empty
/// strings without white space, so the line reads back as the same four values. Any other element becomes
/// `-`, which the page's parser (`adoptx::rows_of`) refuses by its line number. Text that is not an array
/// becomes a single `-`; empty text gives no rows.
fn anchor_rows(text: &str) -> String {
    use zikaron::json::Value;
    const MEMBERS: [&str; 4] = ["chainId", "tx", "payloadKind", "content"];
    if text.trim().is_empty() {
        return String::new();
    }
    let Ok(Value::Arr(items)) = zikaron::json::parse(text.as_bytes()) else { return "-".to_string() };
    let word = |v: &Value, k: &str| match (k, v.member(k)?) {
        ("chainId", Value::Int(n)) => Some(n.to_string()),
        ("chainId", _) => None,
        (_, Value::Str(x)) if !x.is_empty() && !x.chars().any(char::is_whitespace) => Some(x.clone()),
        _ => None,
    };
    let line = |e: &Value| -> Option<String> {
        let Value::Obj(members) = e else { return None };
        if members.len() != MEMBERS.len() {
            return None;
        }
        MEMBERS.iter().map(|k| word(e, k)).collect::<Option<Vec<_>>>().map(|w| w.join(" "))
    };
    items.iter().map(|e| line(e).unwrap_or_else(|| "-".to_string())).collect::<Vec<_>>().join("\n")
}

/// A row's verb: the one its action names (`Action::verb`).
fn verb_of(r: &Row) -> &'static str {
    let empty = Request { verb: String::new(), home: String::new(), args: Vec::new() };
    (r.make)(&empty).verb().unwrap_or_default()
}

/// The verbs accepted through the door and the flags each takes besides `--home`, in the command line's verb
/// order.
pub fn verbs() -> Vec<(&'static str, &'static [&'static str])> {
    ROWS.iter().map(|r| (verb_of(r), r.flags)).collect()
}

/// One received request and the channel its reply goes to.
pub struct Asked {
    pub req: Request,
    answer: Sender<Reply>,
}

/// The open door the shell holds: the home it serves, its closer, the requests waiting for the shell's turn,
/// and the listener thread that writes each reply back.
pub struct Door {
    home: PathBuf,
    closer: zikaron_os::door::Closer,
    asked: Receiver<Asked>,
    /// How many requests were received and handed to the shell.
    came: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    listener: Option<std::thread::JoinHandle<()>>,
    /// The answer the listener waits for now, whoever holds the request (the close answers it too).
    in_hand: InHand,
}

type InHand = std::sync::Arc<std::sync::Mutex<Option<Sender<Reply>>>>;

/// Dropping a door closes it as [`Door::close`] does.
impl Drop for Door {
    fn drop(&mut self) {
        self.close(None);
    }
}

impl Door {
    /// The one way a door closes: nobody new is taken; every request it took (`doing`: the one in progress) is
    /// answered [`Reply::Closed`]; and the listener has written its last reply back and ended before this returns.
    /// Each place it can wait is bounded: the close wakes its accept itself, a request is read within
    /// [`REQUEST_WAIT`], an answer taken within [`ANSWER_WAIT`]. Returns how many were answered here.
    fn close(&mut self, doing: Option<Doing>) -> usize {
        self.closer.close();
        let mut told = 0;
        if let Some(d) = doing {
            let _ = d.answer().send(Reply::Closed);
            told += 1;
        }
        // A request handed over after this finds no taker; the listener answers it `Closed` itself.
        let taken = std::mem::replace(&mut self.asked, std::sync::mpsc::channel().1);
        for a in taken.try_iter() {
            let _ = a.answer.send(Reply::Closed);
            told += 1;
        }
        drop(taken);
        // The listener's request in hand, held anywhere (in a turn under way, say): answered here too, so the join
        // never waits on an answer this thread holds. The first answer is the one it writes.
        if let Some(tx) = self.in_hand.lock().unwrap_or_else(|e| e.into_inner()).take() {
            let _ = tx.send(Reply::Closed);
        }
        if let Some(h) = self.listener.take() {
            let _ = h.join();
        }
        told
    }

    /// The home this door is for.
    pub fn home(&self) -> &std::path::Path {
        &self.home
    }

    /// How many requests this door received and handed to the shell.
    pub fn came(&self) -> usize {
        self.came.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// What the request in progress waits for before it is answered (its background half): the task this request
/// started, by its serial (`Tasks::serial_of`), never by its kind; its own outcome answers it, whatever else of
/// the kind lands meanwhile.
pub enum Doing {
    /// A batch of records (`Kind::Record`), answered from what it says when it lands.
    Record { answer: Sender<Reply>, serial: u64 },
    /// A record package (`Kind::Kit`).
    Kit { answer: Sender<Reply>, serial: u64 },
    /// A record package's verification (`Kind::Verify`); `then_to`, when not empty, the folder its originals are
    /// taken out into once it lands (`Kind::Extract`, the page's own action).
    Verify { answer: Sender<Reply>, serial: u64, then_to: String },
    /// Taking the originals of a verified package out (`Kind::Extract`), answered with the verification's facts
    /// (`verified`) and what was taken.
    Extract { answer: Sender<Reply>, serial: u64, verified: Box<Reply> },
    /// The estimate before sending (`Kind::Gas`), followed by the send, as the send key does.
    Estimate { answer: Sender<Reply>, count: usize, serial: u64 },
    /// The sending, then the receipt wait it starts (`Kind::Anchor`): `serial` is the one awaited now. Answered
    /// at the receipt.
    Send { answer: Sender<Reply>, serial: u64 },
}

/// What a request turns into in this shell.
pub enum Turn {
    /// One action, done now.
    Act(Action),
    /// Sending what the queue holds, as the queue page's send key does: the estimate first, then the batch.
    Send { count: usize },
}

/// Wakes the frame when a request arrives (in the window: a repaint, even when minimized or covered); the
/// shell runs the request in its next drain.
pub type Waker = std::sync::Arc<dyn Fn() + Send + Sync>;

/// The reverse of `Action::verb`: the shell action a request names. A verb not accepted through the door is
/// refused by name (the command line never sends one).
pub fn turn_of(shell: &Shell, q: &Request) -> Result<Turn, Fault> {
    let Some(row) = ROWS.iter().find(|r| verb_of(r) == q.verb) else {
        return Err(Fault::known(Known::DoorUnread, q.verb.clone()));
    };
    let act = (row.make)(q);
    // Send everything sendable in the queue, as the queue page's send key does, by the switch's one judgement
    // (`zikaron_seal::machine::turn`, the credential road's too). Left queued, and an empty queue, are reported
    // by the action's own rule.
    if let Action::SendBatch { .. } = act {
        let count = shell.queue.sendable();
        return Ok(match zikaron_seal::machine::turn(shell.machine.cli_anchor, count) {
            zikaron_seal::machine::Turn::Send => Turn::Send { count },
            zikaron_seal::machine::Turn::Leave | zikaron_seal::machine::Turn::Nothing => Turn::Act(Action::SendAsked { count }),
        });
    }
    Ok(Turn::Act(act))
}

/// A refusal as the door reports it: the code, the evidence, whether it is the absence of an answer rather than a
/// negative one (`Known::unanswered`: the network kind, and a chain not read through for sealing), and its message
/// in both languages. The sender's "already on
/// chain" goes as its own reply, its evidence as values (the hashes, the earliest transaction and block), so
/// both of the command line's roads answer it alike.
pub fn refused(f: &Fault) -> Reply {
    if let Some((names, tx, block)) = f.on_chain() {
        return Reply::OnChain { names: names.clone(), tx: tx.clone(), block: *block };
    }
    // A send whose batch is out though the broadcast failed is no refusal: no node answered for its transaction.
    if let Some(tx) = f.out() {
        return Reply::Unheard { tx: tx.to_string(), detail: f.tail().to_string() };
    }
    let (code, network, (zh, en)) = match f.which() {
        Some(k) => (k.as_str().to_string(), k.unanswered(), crate::lang::both(f.what_key().unwrap_or(k.what()))),
        None => (f.class().as_str().to_string(), false, (f.said(), f.said())),
    };
    Reply::Refused { code, tail: f.tail().to_string(), network, zh: zh.to_string(), en: en.to_string() }
}

/// The listener thread: accept the next client, read its request (within [`REQUEST_WAIT`], at most
/// `zikaron_glue::door::CAP` bytes), hand it to the shell, wake the frame and write the shell's reply back (taken
/// within [`ANSWER_WAIT`]).
/// An unreadable request gets a refusal and the door stays open.
fn wait_at(door: zikaron_os::door::Listener, asked: Sender<Asked>, wake: Option<Waker>, came: std::sync::Arc<std::sync::atomic::AtomicUsize>, in_hand: InHand) {
    use zikaron_glue::door::{put, take, Unread};
    loop {
        let mut s = match door.accept() {
            Ok(Some(s)) => s,
            Ok(None) => return,
            // Another user, or a failed accept: keep listening.
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
                        // The shell answers every request it took; one dropped unanswered (door closed) is
                        // answered `Closed`.
                        rx.recv().unwrap_or(Reply::Closed)
                    };
                    *in_hand.lock().unwrap_or_else(|e| e.into_inner()) = None;
                    reply
                }
                None => refused(&Fault::known(Known::DoorUnread, crate::lang::t(crate::lang::Key::TailDoorNotForm).to_string())),
            },
            Err(Unread::TooLarge(n)) => refused(&Fault::known(Known::DoorUnread, crate::lang::fill1(crate::lang::Key::TailDoorTooLarge, &n.to_string()))),
            // The client left (or sent nothing in time): nobody to answer.
            Err(Unread::Short) | Err(Unread::Io(_)) => continue,
        };
        // The answer's own time to be taken starts once it is ready, however long the work took.
        let _ = s.set_write_deadline(Some(ANSWER_WAIT));
        let _ = put(&mut s, &reply.to_bytes());
    }
}

/// A shell dropped with its door open (a test, a probe) closes it with the request in progress, so the listener
/// is not left waiting for that request's answer.
impl Drop for Shell {
    fn drop(&mut self) {
        if let Some(mut d) = self.door.take() {
            d.close(self.door_doing.take());
        }
    }
}

impl Shell {
    /// The home whose door should be open now: unlocked, a home open, this instance its writer, and the
    /// command line's path through the desktop (`machine::CliPath::Desktop`: "writes through the desktop" on,
    /// "writes alone" off).
    fn door_wanted(&self) -> Option<PathBuf> {
        let writer = self.lock.as_ref().map(|l| l.mode() == crate::lock::Mode::Writer).unwrap_or(false);
        let through = self.machine.cli_path() == crate::machine::CliPath::Desktop;
        if self.door_off || !self.unlocked() || !writer || !through {
            return None;
        }
        self.home.as_ref().map(|h| h.root().to_path_buf())
    }

    /// Opens or closes the door to match the shell's state: open exactly while [`Shell::door_wanted`] names a
    /// home, and for that home. Called at every drain and wherever the home closes, so no lock, close, switch
    /// or quit leaves it open. A door that fails to open is retried at the next drain and reported once per
    /// home.
    pub fn door_sync(&mut self) {
        let want = self.door_wanted();
        let have = self.door.as_ref().map(|d| d.home.clone());
        if want == have {
            if want.is_none() {
                self.door_shut = None;
            }
            return;
        }
        if let Some(mut d) = self.door.take() {
            self.door_answered += d.close(self.door_doing.take());
        }
        let Some(home) = want else {
            self.door_shut = None;
            return;
        };
        match self.door_open(&home) {
            Ok(d) => {
                self.door = Some(d);
                self.door_shut = None;
            }
            // Kept as shell state rather than shown as an error: nobody pressed anything, and a command for this
            // home is told by name on its side (`E_DESKTOP`, `PATH_TOO_LONG` or `NOT_OPEN`).
            Err(f) => self.door_shut = Some((home, f)),
        }
    }

    fn door_open(&self, home: &std::path::Path) -> Result<Door, Fault> {
        let machine = crate::home::machine_dir()?;
        let place = zikaron_glue::door::place(&machine, home);
        let (listener, closer) = zikaron_os::door::listen(&place).map_err(|e| Fault::known(Known::DoorShut, e.to_string()))?;
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
                return Err(Fault::known(Known::DoorShut, e.to_string()));
            }
        };
        Ok(Door { home: home.to_path_buf(), closer, asked: rx, came, listener: Some(listener), in_hand })
    }

    /// The door's turn, at the end of every drain (`landed`: what this drain received): the request in
    /// progress advances on what landed, then waiting requests run one at a time, each answered before the
    /// next is taken.
    pub fn door_turn(&mut self, landed: &[Outcome]) {
        // Let what landed answer the request in progress first (it may have landed in the drain that locked).
        if let Some(doing) = self.door_doing.take() {
            self.door_doing = self.door_step(doing, landed);
        }
        self.door_sync();
        while self.door_doing.is_none() {
            let Some(a) = self.door.as_ref().and_then(|d| d.asked.try_recv().ok()) else { return };
            self.door_do(a);
        }
    }

    /// Runs one request now through `apply`; answered at once, or left waiting for its background result.
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
                let got = crate::action::apply(self, Action::EstimateGas { count });
                match (got, self.tasks.serial_of(Kind::Gas)) {
                    (Applied::Started(Kind::Gas), Some(serial)) => self.door_doing = Some(Doing::Estimate { answer, count, serial }),
                    (other, _) => self.door_answer(answer, other),
                }
            }
            Turn::Act(act) => {
                // The passcode is asked only on the desktop, per the action layer's own table.
                if act.pin_asked().is_some() {
                    self.door_send(&answer, Reply::OnDesktop);
                    return;
                }
                let then_to = match &act {
                    Action::VerifyWork { then_to, .. } => then_to.trim().to_string(),
                    _ => String::new(),
                };
                let got = crate::action::apply(self, act);
                match (got, self.tasks.serial_of(Kind::Record), self.tasks.serial_of(Kind::Kit), self.tasks.serial_of(Kind::Verify)) {
                    (Applied::Started(Kind::Record), Some(serial), _, _) => self.door_doing = Some(Doing::Record { answer, serial }),
                    (Applied::Started(Kind::Kit), _, Some(serial), _) => self.door_doing = Some(Doing::Kit { answer, serial }),
                    (Applied::Started(Kind::Verify), _, _, Some(serial)) => self.door_doing = Some(Doing::Verify { answer, serial, then_to }),
                    (other, _, _, _) => self.door_answer(answer, other),
                }
            }
        }
    }

    /// Sends a reply and counts it (`Shell::door_answered`), so tests can tell a request was answered.
    fn door_send(&mut self, answer: &Sender<Reply>, r: Reply) {
        self.door_answered += 1;
        let _ = answer.send(r);
    }

    /// Answers a request from its action's result and hands the same result to the window to show (the
    /// toast a click would give).
    fn door_answer(&mut self, answer: Sender<Reply>, got: Applied) {
        let reply = self.door_reply(&got);
        // A refusal the action layer recorded is shown by the window from there; anything else as for a click.
        if !matches!(got, Applied::Trouble(_)) {
            self.door_told.push(got);
        }
        self.door_send(&answer, reply);
    }

    /// The reply facts for an action's result.
    fn door_reply(&self, got: &Applied) -> Reply {
        let wrote = |id: &str| self.door_wrote(id);
        match got {
            Applied::Genesised { id, .. }
            | Applied::Annotated(id)
            | Applied::Retracted { id, .. }
            | Applied::Recorded { id, .. }
            | Applied::Granted { id, .. }
            | Applied::Revoked { id, .. }
            | Applied::Adopted { id, .. } => wrote(id),
            Applied::RecordedBatch { ids, stopped, .. } => match (ids.first(), stopped) {
                (Some(id), _) => wrote(id),
                (None, Some((_, _, f))) => refused(f),
                (None, None) => refused(&Fault::known(Known::OutcomeLost, crate::lang::t(crate::lang::Key::TailDoorNothingCame).to_string())),
            },
            Applied::SendAsked { count } => Reply::Queued { count: *count as u64 },
            Applied::Trouble(f) => refused(f),
            Applied::Refused(k) => refused(&Fault::known(Known::InFlight, k.as_str().to_string())),
            other => refused(&Fault::known(Known::OutcomeLost, format!("{other:?}"))),
        }
    }

    /// Reply for an entry this shell wrote: its id, the ledger folder, and its sequence number from the
    /// ledger head (the entry just written is the head).
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

    /// Advances the request in progress on what landed; `None` once it is answered. Only the outcome of the task
    /// this request started counts (by serial), and each of its landings has its own reply: a send broadcast
    /// whose receipt wait does not start (the vault locking meanwhile) is "not yet included" with its
    /// transaction; a task whose home left before it landed is answered as the door closing.
    fn door_step(&mut self, doing: Doing, landed: &[Outcome]) -> Option<Doing> {
        let mine = |serial: u64| landed.iter().find(|o| o.serial == serial);
        let unexpected = |k: Kind| refused(&Fault::known(Known::OutcomeLost, k.as_str().to_string()));
        match doing {
            Doing::Record { answer, serial } => {
                if mine(serial).is_none() {
                    return Some(Doing::Record { answer, serial });
                }
                // Its answer is in `said` (`action::landed`); none when it landed for a home no longer open.
                match self.said.remove(&Kind::Record) {
                    Some(got) => self.door_answer(answer, got),
                    None => self.door_send(&answer, Reply::Closed),
                }
                None
            }
            Doing::Kit { answer, serial } => {
                let Some(o) = mine(serial) else { return Some(Doing::Kit { answer, serial }) };
                let reply = match &o.result {
                    Ok(Done::Kit { path, kit_id, entries, files, dropped, originals, left_out, unreadable, .. }) => Reply::Kit {
                        kit_id: kit_id.clone(),
                        path: path.clone(),
                        entries: *entries as u64,
                        files: *files as u64,
                        dropped: dropped.clone(),
                        originals: originals.clone(),
                        // What did not go in, as the command line's own export names it: no longer the bytes
                        // recorded, or not there to read.
                        left_out: left_out.iter().cloned().chain(unreadable.iter().map(|(p, _)| p.clone())).collect(),
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
                // Verified and asked to take the originals out: the page's own action, the package read again and
                // judged there; answered when it lands.
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
                    (Ok(Done::Extracted { landed, .. }), Reply::Verified { kit_id, entries, records, .. }) => Reply::Verified {
                        kit_id,
                        entries,
                        records,
                        taken: Some(zikaron_glue::door::Taken { path: landed.to.display().to_string(), names: landed.names.clone() }),
                    },
                    (Ok(_), _) => unexpected(Kind::Extract),
                    (Err(f), _) => refused(f),
                };
                self.door_send(&answer, reply);
                None
            }
            Doing::Estimate { answer, count, serial } => {
                if mine(serial).is_none() {
                    return Some(Doing::Estimate { answer, count, serial });
                }
                match self.said.remove(&Kind::Gas) {
                    // The estimate is in: send the batch it is for, as the send key does next.
                    Some(Applied::Gas { .. }) => match (crate::action::apply(self, Action::SendBatch { count }), self.tasks.serial_of(Kind::Anchor)) {
                        (Applied::Started(Kind::Anchor), Some(serial)) => return Some(Doing::Send { answer, serial }),
                        (other, _) => self.door_answer(answer, other),
                    },
                    Some(other) => self.door_answer(answer, other),
                    None => self.door_send(&answer, Reply::Closed),
                }
                None
            }
            Doing::Send { answer, serial } => {
                let Some(o) = mine(serial) else { return Some(Doing::Send { answer, serial }) };
                let reply = match &o.result {
                    // Broadcast: the receipt wait it started is awaited next. None started (the vault is locking,
                    // or the home left): broadcast and not yet waited for.
                    Ok(Done::Submitted { tx, .. }) => match self.tasks.serial_of(Kind::Anchor) {
                        Some(next) if next != serial => return Some(Doing::Send { answer, serial: next }),
                        _ => Reply::NotYet { tx: tx.clone(), waited: 0 },
                    },
                    Ok(Done::Anchored { tx, state, voided, waited, .. }) => match crate::action::receipt_of(state) {
                        // Void (no node holds it and its nonce was used by another transaction): reported as
                        // void with its entries back in the queue, never as "not yet".
                        _ if *voided => Reply::Voided { tx: tx.clone() },
                        Some((1, block)) => Reply::Anchored { tx: tx.clone(), block },
                        Some((status, _)) => Reply::Reverted { tx: tx.clone(), status },
                        None if crate::action::receipt_unheard(state) => Reply::Unheard { tx: tx.clone(), detail: state.clone() },
                        // How long the receipt wait ran, as measured: never the wait it was given.
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

/// The door's facts of a verification as it landed: the kit law's refusal (a package that does not pass, or a place
/// that is no record package), or the package's id, its entries and each record's facts as the page judged them
/// (`zikaron_glue::recordcheck`, the one judge).
fn verified_reply(v: &crate::verifyx::Verified) -> Reply {
    match &v.kit {
        Some(k) if k.ok => Reply::Verified {
            kit_id: k.kit_id.clone(),
            entries: k.entries as u64,
            records: v.records.iter().map(|r| zikaron_glue::door::Record { entry_id: r.id.clone(), content: r.content.clone(), original: r.original, chain: r.chain }).collect(),
            taken: None,
        },
        Some(k) => Reply::KitRefused { verdict: k.verdict.clone(), subject: Some(k.subject.clone()).filter(|x| !x.is_empty()) },
        None => Reply::KitRefused { verdict: zikaron_kit::tokens::KitFailToken::ManifestAbsent.as_str().to_string(), subject: None },
    }
}

impl Doing {
    fn answer(&self) -> &Sender<Reply> {
        match self {
            Doing::Record { answer, .. }
            | Doing::Kit { answer, .. }
            | Doing::Verify { answer, .. }
            | Doing::Extract { answer, .. }
            | Doing::Estimate { answer, .. }
            | Doing::Send { answer, .. } => answer,
        }
    }
}
