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
/// the door (the command line refuses `--home` with it), and a flag not in its row is not accepted.
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
    Row { flags: &["to", "kind", "effective", "statement"], make: |q| Action::Succeed { to: one(q, "to"), kind: one(q, "kind"), effective: one(q, "effective"), statement_md: one(q, "statement") } },
    Row { flags: &["subject", "note"], make: |q| Action::Annotate { subject: one(q, "subject"), note_md: one(q, "note") } },
    Row { flags: &["subject", "note"], make: |q| Action::Retract { subject: one(q, "subject"), note_md: one(q, "note") } },
    // Sending the queue: whether it is sent or left for the user depends on the setting ([`turn_of`]).
    Row { flags: &[], make: |_| Action::SendBatch { count: 0 } },
    Row { flags: &["entry", "note", "out"], make: |q| Action::ExportKit { from: String::new(), to: String::new(), ids: q.many("entry").join("\n"), attach: String::new(), note: one(q, "note"), out: one(q, "out") } },
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

/// The open door the shell holds: the home it serves, its closer, and the requests waiting for the shell's
/// turn.
pub struct Door {
    home: PathBuf,
    closer: zikaron_os::door::Closer,
    asked: Receiver<Asked>,
    /// How many requests were received and handed to the shell.
    came: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

/// Dropping a door (with its shell, in the window or a test) closes it: waiting clients are answered
/// [`Reply::Closed`] and the path is free for the next door.
impl Drop for Door {
    fn drop(&mut self) {
        self.closer.close();
        while let Ok(a) = self.asked.try_recv() {
            let _ = a.answer.send(Reply::Closed);
        }
    }
}

impl Door {
    /// The home this door is for.
    pub fn home(&self) -> &std::path::Path {
        &self.home
    }

    /// How many requests this door received and handed to the shell.
    pub fn came(&self) -> usize {
        self.came.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// What the request in progress waits for before it is answered (its background half).
pub enum Doing {
    /// A batch of records (`Kind::Record`), answered from what it says when it lands.
    Record { answer: Sender<Reply> },
    /// A record package (`Kind::Kit`).
    Kit { answer: Sender<Reply> },
    /// The estimate before sending (`Kind::Gas`), followed by the send, as the send key does.
    Estimate { answer: Sender<Reply>, count: usize },
    /// The sending and its receipt wait (`Kind::Anchor`), answered at the receipt.
    Send { answer: Sender<Reply> },
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
    // Send everything sendable in the queue, as the queue page's send key does. An empty queue is reported by
    // the action's own rule, whatever the setting says.
    if let Action::SendBatch { .. } = act {
        let count = shell.queue.sendable();
        return Ok(match (shell.machine.cli_anchor, count) {
            (crate::machine::CliAnchor::Send, 1..) => Turn::Send { count },
            _ => Turn::Act(Action::SendAsked { count }),
        });
    }
    Ok(Turn::Act(act))
}

/// A refusal as the door reports it: the code, the evidence, whether it is a network refusal (no answer
/// rather than a negative one, `Known::NETWORK`), and its message in both languages.
pub fn refused(f: &Fault) -> Reply {
    let (code, network, (zh, en)) = match f.which() {
        Some(k) => (k.as_str().to_string(), Known::NETWORK.contains(&k), crate::lang::both(f.what_key().unwrap_or(k.what()))),
        None => (f.class().as_str().to_string(), false, (f.said(), f.said())),
    };
    Reply::Refused { code, tail: f.tail().to_string(), network, zh: zh.to_string(), en: en.to_string() }
}

/// The listener thread: accept the next client, read its request (within [`REQUEST_WAIT`], at most
/// `zikaron_glue::door::CAP` bytes), hand it to the shell, wake the frame and write the shell's reply back.
/// An unreadable request gets a refusal and the door stays open.
fn wait_at(door: zikaron_os::door::Listener, asked: Sender<Asked>, wake: Option<Waker>, came: std::sync::Arc<std::sync::atomic::AtomicUsize>) {
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
                    if asked.send(Asked { req, answer: tx }).is_err() {
                        Reply::Closed
                    } else {
                        came.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        if let Some(w) = &wake {
                            w();
                        }
                        // The shell answers every request it took; one dropped unanswered (door closed) is
                        // answered `Closed`.
                        rx.recv().unwrap_or(Reply::Closed)
                    }
                }
                None => refused(&Fault::known(Known::DoorUnread, crate::lang::t(crate::lang::Key::TailDoorNotForm).to_string())),
            },
            Err(Unread::TooLarge(n)) => refused(&Fault::known(Known::DoorUnread, crate::lang::fill1(crate::lang::Key::TailDoorTooLarge, &n.to_string()))),
            // The client left (or sent nothing in time): nobody to answer.
            Err(Unread::Short) | Err(Unread::Io(_)) => continue,
        };
        let _ = put(&mut s, &reply.to_bytes());
    }
}

impl Shell {
    /// The home whose door should be open now: unlocked, a home open, and this instance its writer.
    fn door_wanted(&self) -> Option<PathBuf> {
        let writer = self.lock.as_ref().map(|l| l.mode() == crate::lock::Mode::Writer).unwrap_or(false);
        if self.door_off || !self.unlocked() || !writer {
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
        if let Some(d) = self.door.take() {
            d.closer.close();
            // Requests received but not taken are answered `Closed`.
            while let Ok(a) = d.asked.try_recv() {
                self.door_send(&a.answer, Reply::Closed);
            }
            if let Some(doing) = self.door_doing.take() {
                self.door_send(doing.answer(), Reply::Closed);
            }
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
        if let Err(e) = crate::task::start_named("door", move || wait_at(listener, tx, wake, counted)) {
            closer.close();
            return Err(Fault::known(Known::DoorShut, e.to_string()));
        }
        Ok(Door { home: home.to_path_buf(), closer, asked: rx, came })
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
                match got {
                    Applied::Started(Kind::Gas) => self.door_doing = Some(Doing::Estimate { answer, count }),
                    other => self.door_answer(answer, other),
                }
            }
            Turn::Act(act) => {
                // The passcode is asked only on the desktop, per the action layer's own table.
                if act.pin_asked().is_some() {
                    self.door_send(&answer, Reply::OnDesktop);
                    return;
                }
                let got = crate::action::apply(self, act);
                match got {
                    Applied::Started(Kind::Record) => self.door_doing = Some(Doing::Record { answer }),
                    Applied::Started(Kind::Kit) => self.door_doing = Some(Doing::Kit { answer }),
                    other => self.door_answer(answer, other),
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
            | Applied::Adopted { id, .. }
            | Applied::Succeeded { id, .. } => wrote(id),
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

    /// Advances the request in progress on what landed; `None` once it is answered.
    fn door_step(&mut self, doing: Doing, landed: &[Outcome]) -> Option<Doing> {
        let gone = |shell: &Shell, k: Kind| !shell.tasks.in_flight(k) && !shell.tasks.flying().contains(&k);
        let lost = |k: Kind| refused(&Fault::known(Known::OutcomeLost, k.as_str().to_string()));
        match doing {
            Doing::Record { answer } => match self.said.remove(&Kind::Record) {
                Some(got) => {
                    self.door_answer(answer, got);
                    None
                }
                None if gone(self, Kind::Record) => {
                    self.door_send(&answer, lost(Kind::Record));
                    None
                }
                None => Some(Doing::Record { answer }),
            },
            Doing::Kit { answer } => match landed.iter().find(|o| o.kind == Kind::Kit) {
                Some(o) => {
                    let reply = match &o.result {
                        Ok(Done::Kit { path, kit_id, entries, files, dropped, .. }) => {
                            Reply::Kit { kit_id: kit_id.clone(), path: path.clone(), entries: *entries as u64, files: *files as u64, dropped: dropped.clone() }
                        }
                        Ok(_) => lost(Kind::Kit),
                        Err(f) => refused(f),
                    };
                    self.door_send(&answer, reply);
                    None
                }
                None if gone(self, Kind::Kit) => {
                    self.door_send(&answer, lost(Kind::Kit));
                    None
                }
                None => Some(Doing::Kit { answer }),
            },
            Doing::Estimate { answer, count } => match self.said.remove(&Kind::Gas) {
                // The estimate is in: send the batch it is for, as the send key does next.
                Some(Applied::Gas { .. }) => match crate::action::apply(self, Action::SendBatch { count }) {
                    Applied::Started(Kind::Anchor) => Some(Doing::Send { answer }),
                    other => {
                        self.door_answer(answer, other);
                        None
                    }
                },
                Some(other) => {
                    self.door_answer(answer, other);
                    None
                }
                None if gone(self, Kind::Gas) => {
                    self.door_send(&answer, lost(Kind::Gas));
                    None
                }
                None => Some(Doing::Estimate { answer, count }),
            },
            Doing::Send { answer } => {
                for o in landed.iter().filter(|o| o.kind == Kind::Anchor) {
                    match &o.result {
                        // Broadcast: the receipt wait continues in the shell; keep waiting.
                        Ok(Done::Submitted { .. }) => {}
                        Ok(Done::Anchored { tx, state, voided, .. }) => {
                            let reply = match crate::action::receipt_of(state) {
                                // Void (no node holds it and its nonce was used by another transaction):
                                // reported as void with its entries back in the queue, never as "not yet".
                                _ if *voided => Reply::Voided { tx: tx.clone() },
                                Some((1, block)) => Reply::Anchored { tx: tx.clone(), block },
                                Some((status, _)) => Reply::Reverted { tx: tx.clone(), status },
                                None if crate::action::receipt_unheard(state) => Reply::Unheard { tx: tx.clone(), detail: state.clone() },
                                None => Reply::NotYet { tx: tx.clone(), waited: self.anchor_wait.as_secs() },
                            };
                            self.door_send(&answer, reply);
                            return None;
                        }
                        Ok(_) => {}
                        Err(f) => {
                            self.door_send(&answer, refused(f));
                            return None;
                        }
                    }
                }
                if gone(self, Kind::Anchor) {
                    self.door_send(&answer, lost(Kind::Anchor));
                    return None;
                }
                Some(Doing::Send { answer })
            }
        }
    }
}

impl Doing {
    fn answer(&self) -> &Sender<Reply> {
        match self {
            Doing::Record { answer } | Doing::Kit { answer } | Doing::Estimate { answer, .. } | Doing::Send { answer } => answer,
        }
    }
}
