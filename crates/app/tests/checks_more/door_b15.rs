//! The command line's door, desktop side. A shell is booted, unlocked and made the writer of a home; a
//! stand-in for the command line in this process connects at the door's place, sends a request and reads the
//! reply (`zikaron_glue::door`), while the test drains the shell as the window's frame does. Each case of the
//! door's boundary table is one test (the command line's half is `zikaron-cli/tests/desk.rs`). Each test runs
//! alone in its own process (places are per process); the machine folder path is short because a local
//! socket's path must be.

use app::action::{apply, Action, Applied};
use app::shell::Shell;
use std::path::{Path, PathBuf};
use zikaron_glue::door::{Reply, Request};

const PIN: &str = "27618394";

struct Bench {
    root: PathBuf,
    machine: PathBuf,
    home: PathBuf,
}

impl Bench {
    /// Places for this process (a short machine folder), the vault open, and a shell writing a home with its first
    /// entry; the door opens at the first drain.
    fn new(tag: &str) -> (Bench, Shell) {
        // This process's places and vault (`vault_open`: a short machine folder of its own).
        super::vault_open();
        let base = if std::env::temp_dir().as_os_str().len() <= 40 { std::env::temp_dir() } else { PathBuf::from("/tmp") };
        let root = base.join(format!("zkb15-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = root.join("h");
        let machine = app::home::machine_dir().expect("machine");
        std::fs::create_dir_all(&machine).expect("machine");
        let machine = std::fs::canonicalize(&machine).expect("machine");
        let mut shell = boot();
        let home = home_with_genesis(&mut shell, &home);
        shell.drain();
        (Bench { root, machine, home }, shell)
    }

    fn place(&self) -> PathBuf {
        zikaron_glue::door::place(&self.machine, &self.home)
    }

    fn ask(&self, verb: &str, args: &[(&str, &str)]) -> Request {
        Request { verb: verb.into(), home: self.home.display().to_string(), args: args.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn boot() -> Shell {
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.reread_vault();
    shell
}

/// A home with its first entry, signed by the identity's anchor key (made once).
fn home_with_genesis(shell: &mut Shell, at: &Path) -> PathBuf {
    let opened = apply(shell, Action::OpenHome { root: at.display().to_string() });
    assert!(matches!(opened, Applied::Homed { .. }), "{opened:?}");
    if shell.anchor.is_none() {
        apply(shell, Action::MakeAnchorKey);
    }
    let made = apply(shell, Action::Genesis { statement: "一本账的开端".into() });
    assert!(matches!(made, Applied::Genesised { .. }), "{made:?}");
    std::fs::canonicalize(at).expect("home")
}

/// Send `bytes` at `place` from another thread (the command line's side); returns the reply's bytes, if any.
fn send(place: &Path, bytes: Vec<u8>) -> std::thread::JoinHandle<Option<Vec<u8>>> {
    let place = place.to_path_buf();
    std::thread::spawn(move || {
        let mut s = zikaron_os::door::connect(&place).ok()?;
        zikaron_glue::door::put(&mut s, &bytes).ok()?;
        zikaron_glue::door::take(&mut s).ok()
    })
}

/// Drain the shell as the frame does until the command line's side is done (bounded, so a test never hangs).
fn served(shell: &mut Shell, h: std::thread::JoinHandle<Option<Vec<u8>>>) -> Option<Reply> {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !h.is_finished() && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    h.join().ok().flatten().and_then(|b| Reply::of_bytes(&b))
}

/// The trace marks dropped since `from` (a count of all marks so far), from the action's own first mark on;
/// earlier marks are the shell's background work on other threads.
fn own_marks(from: usize) -> Vec<&'static str> {
    let all: Vec<&str> = app::trace::ring().into_iter().skip(from.saturating_sub(app::trace::dropped())).collect();
    let first = all.iter().position(|m| *m == app::feature::Feature::W1.id()).unwrap_or(all.len());
    all[first..].to_vec()
}

/// Drain until no background work is running (what opening a home started has landed), bounded.
fn settle(shell: &mut Shell) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    shell.drain();
    while !shell.tasks.flying().is_empty() && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn head(shell: &Shell) -> (u64, String) {
    app::ledgerx::head(shell.home.as_ref().expect("a home")).expect("head").expect("an entry")
}

/// An entry's opened bytes, by its id.
fn entry_bytes(shell: &Shell, id: &str) -> Vec<u8> {
    let pile = shell.home.as_ref().expect("home").ledger().expect("ledger").pile().expect("pile");
    pile.items.into_iter().find(|b| zikaron::hexfmt::encode(&zikaron::entry::entry_id(b)).eq_ignore_ascii_case(id)).expect("the entry")
}

/// The door is open exactly while the shell is unlocked and writes a home, and belongs to that home: it goes
/// on lock, returns on unlock, moves with the open home and shuts on quit. A stale door left by a stopped
/// desktop does not block a new one.
#[test]
fn the_door_follows_the_lock_and_the_home() {
    if super::alone_in(module_path!(), "the_door_follows_the_lock_and_the_home") {
        return;
    }
    let (b, mut shell) = Bench::new("follows");
    let at = b.place();
    assert!(zikaron_os::door::connect(&at).is_ok(), "unlocked and writing: the door is open");
    answers!(apply(&mut shell, Action::Lock), Applied::LockedUp);
    assert!(shell.door.is_none());
    assert_eq!(zikaron_os::door::connect(&at).err().map(|e| e.kind()), Some(std::io::ErrorKind::NotFound), "locked: nobody at the door");
    answers!(app::action::apply_settled(&mut shell, Action::Unlock { pin: PIN.into() }), Applied::Unlocked);
    shell.drain();
    assert!(zikaron_os::door::connect(&at).is_ok(), "unlocked again: the door is back");
    // Another home: the door belongs to that home.
    let other = home_with_genesis(&mut shell, &b.root.join("h2"));
    shell.drain();
    assert_eq!(zikaron_os::door::connect(&at).err().map(|e| e.kind()), Some(std::io::ErrorKind::NotFound), "the first home's door is shut");
    assert!(zikaron_os::door::connect(&zikaron_glue::door::place(&b.machine, &other)).is_ok(), "the open home has the door");
    // Quitting shuts it, whatever else holds.
    apply(&mut shell, Action::Quit);
    shell.drain();
    assert!(shell.door.is_none() && zikaron_os::door::connect(&zikaron_glue::door::place(&b.machine, &other)).is_err(), "quit: shut");
    // A door left behind at a home's place (opened by a stopped desktop and never closed) gives way when the
    // door opens.
    {
        let (left, _never_closed) = zikaron_os::door::listen(&b.place()).expect("left behind");
        drop(left);
        let mut again = boot();
        let _ = apply(&mut again, Action::OpenHome { root: b.home.display().to_string() });
        again.drain();
        assert!(again.door.is_some() && zikaron_os::door::connect(&b.place()).is_ok(), "a door left behind gave way");
    }
}

/// A second instance that only reads the home keeps no door; the writer's is the home's.
#[test]
fn a_reader_keeps_no_door() {
    if super::alone_in(module_path!(), "a_reader_keeps_no_door") {
        return;
    }
    let (b, shell) = Bench::new("reader");
    let mut second = boot();
    let opened = apply(&mut second, Action::OpenHome { root: b.home.display().to_string() });
    assert!(matches!(opened, Applied::Homed { mode: app::lock::Mode::Reader, .. }), "{opened:?}");
    second.drain();
    assert!(second.door.is_none(), "a reader opens no door");
    assert!(shell.door.is_some());
}

/// A request goes through the action layer and answers with its entry (id, ledger folder, head sequence); the
/// window reports the same result, and the same action through the door or on the desktop writes the same
/// bytes and trace marks.
#[test]
fn a_request_is_done_by_the_action_layer_as_a_click_is() {
    if super::alone_in(module_path!(), "a_request_is_done_by_the_action_layer_as_a_click_is") {
        return;
    }
    let (b, mut shell) = Bench::new("same");
    let (_, genesis) = head(&shell);
    // Pressed on the desktop, in a second home of the same identity (same key, same first entry).
    let first = b.home.clone();
    let twin = home_with_genesis(&mut shell, &b.root.join("twin"));
    settle(&mut shell);
    let marks = app::trace::ring().len() + app::trace::dropped();
    let pressed = apply(&mut shell, Action::Annotate { subject: genesis.clone(), note_md: "n".into() });
    let pressed_marks = own_marks(marks);
    let Applied::Annotated(pressed_id) = pressed else { panic!("{pressed:?}") };
    let pressed_bytes = entry_bytes(&shell, &pressed_id);
    assert_ne!(twin, first);
    // Through the door, in the first home.
    let _ = apply(&mut shell, Action::OpenHome { root: first.display().to_string() });
    settle(&mut shell);
    shell.door_told.clear();
    let marks = app::trace::ring().len() + app::trace::dropped();
    let h = send(&b.place(), b.ask("annotate", &[("subject", &genesis), ("note", "n")]).to_bytes());
    let reply = served(&mut shell, h).expect("answered");
    let door_marks = own_marks(marks);
    let (seq, id) = head(&shell);
    let ledger = shell.home.as_ref().unwrap().dir(app::home::Slot::Ledger).display().to_string();
    assert_eq!(reply, Reply::Wrote { entry_id: id.clone(), ledger, seq });
    assert_eq!(seq, 1);
    assert_eq!(entry_bytes(&shell, &id), pressed_bytes, "the same bytes, pressed or through the door");
    assert!(!pressed_marks.is_empty(), "the action dropped its marks");
    assert_eq!(door_marks, pressed_marks, "the same trace marks");
    assert!(matches!(shell.door_told.as_slice(), [Applied::Annotated(x)] if *x == id), "the window tells it as a click's: {:?}", shell.door_told);
}

/// Each entry-writing verb through the door writes its entry: `init` on a rooted home is refused by name;
/// `history`, `grant`, `revoke`, `succeed` and `retract` each land and answer their entry (an action layer
/// refusal answers its code).
#[test]
fn each_writing_verb_lands_its_entry() {
    if super::alone_in(module_path!(), "each_writing_verb_lands_its_entry") {
        return;
    }
    let (b, mut shell) = Bench::new("each");
    let ask = |shell: &mut Shell, q: Request| served(shell, send(&b.place(), q.to_bytes())).expect("answered");
    let refused_code = |r: &Reply| match r {
        Reply::Refused { code, .. } => code.clone(),
        other => format!("{other:?}"),
    };
    assert_eq!(refused_code(&ask(&mut shell, b.ask("init", &[("statement", "again")]))), app::fault::Known::AlreadyRooted.as_str());
    let file = b.root.join("work.txt");
    std::fs::write(&file, b"the work").expect("file");
    let r = ask(&mut shell, b.ask("history", &[("file", &file.display().to_string()), ("note", "a work")]));
    let Reply::Wrote { entry_id: work_entry, seq, .. } = r.clone() else { panic!("{r:?}") };
    assert_eq!(seq, 1);
    let body = zikaron::json::parse(&entry_bytes(&shell, &work_entry)).expect("json");
    let content = body.member("body").and_then(|x| x.member("content")).and_then(|x| x.as_str()).unwrap_or_default().to_string();
    assert_eq!(content, zikaron::hexfmt::encode(&zikaron_glue::recording::content_of(b"the work")), "recorded by the convention");
    let work = content;
    let me = shell.anchor.map(|a| a.hex()).unwrap_or_default();
    let grantee = format!("0x{}", "ab".repeat(20));
    let r = ask(&mut shell, b.ask("grant", &[("grantee", &grantee), ("work", &work), ("terms", &format!("0x{}", "cd".repeat(32))), ("window-from", "1"), ("window-to", "9"), ("scope", "s")]));
    let Reply::Wrote { entry_id: grant, seq, .. } = r.clone() else { panic!("grant: {r:?}") };
    assert_eq!(seq, 2);
    let r = ask(&mut shell, b.ask("revoke", &[("grant", &grant), ("case", &format!("0x{}", "ef".repeat(32)))]));
    assert!(matches!(r, Reply::Wrote { seq: 3, .. }), "revoke: {r:?}");
    let r = ask(&mut shell, b.ask("retract", &[("subject", &work_entry), ("note", "gone")]));
    assert!(matches!(r, Reply::Wrote { seq: 4, .. }), "retract: {r:?}");
    let r = ask(&mut shell, b.ask("succeed", &[("to", &format!("0x{}", "12".repeat(20))), ("kind", "handover"), ("effective", "5"), ("statement", "s")]));
    assert!(matches!(r, Reply::Wrote { seq: 5, .. }) || matches!(&r, Reply::Refused { .. }), "succeed: {r:?}");
    assert!(!me.is_empty());
    // A refusal answers the action layer's code: a malformed subject.
    let r = ask(&mut shell, b.ask("retract", &[("subject", "not-an-id")]));
    assert!(matches!(&r, Reply::Refused { network: false, .. }), "{r:?}");
}

/// An action that asks for the passcode is never done through the door: answered "on the desktop", nothing
/// written, judged by the action layer's own table (`Action::pin_asked`).
#[test]
fn a_passcode_action_is_left_to_the_desktop() {
    if super::alone_in(module_path!(), "a_passcode_action_is_left_to_the_desktop") {
        return;
    }
    let (b, mut shell) = Bench::new("pin");
    let before = head(&shell);
    let r = served(&mut shell, send(&b.place(), b.ask("attest", &[]).to_bytes()));
    assert_eq!(r, Some(Reply::OnDesktop));
    assert_eq!(head(&shell), before, "nothing written");
    let attest = Action::AttestFor { text: String::new(), pin: app::secret::Secret::default() };
    assert!(attest.pin_asked().is_some() && attest.verb() == Some("attest"));
}

/// Unreadable requests are answered as such and the door stays open: an unsupported verb, a malformed frame,
/// an oversize frame; a client that leaves or stays silent past the wait gets no answer and blocks nobody.
#[test]
fn what_does_not_read_is_answered_and_the_door_stays_open() {
    if super::alone_in(module_path!(), "what_does_not_read_is_answered_and_the_door_stays_open") {
        return;
    }
    let (b, mut shell) = Bench::new("unread");
    let code = |r: Option<Reply>| match r {
        Some(Reply::Refused { code, .. }) => code,
        other => format!("{other:?}"),
    };
    let unread = app::fault::Known::DoorUnread.as_str();
    assert_eq!(code(served(&mut shell, send(&b.place(), b.ask("scan", &[]).to_bytes()))), unread, "a verb not done through the door");
    assert_eq!(code(served(&mut shell, send(&b.place(), b"{\"form\":\"other\"}".to_vec()))), unread, "not the door's form");
    // Oversize: the length prefix exceeds the cap, and nothing more is read.
    let place = b.place();
    let over = std::thread::spawn(move || {
        use std::io::Write;
        let mut s = zikaron_os::door::connect(&place).ok()?;
        s.write_all(&((zikaron_glue::door::CAP + 1) as u32).to_be_bytes()).ok()?;
        zikaron_glue::door::take(&mut s).ok()
    });
    assert_eq!(code(served(&mut shell, over)), unread, "over the size");
    // One that connects and leaves at once, then one that says nothing (dropped after the wait); the next is served.
    drop(zikaron_os::door::connect(&b.place()).expect("in"));
    let silent = zikaron_os::door::connect(&b.place()).expect("in");
    let started = std::time::Instant::now();
    let r = served(&mut shell, send(&b.place(), b.ask("annotate", &[("note", "after")]).to_bytes()));
    assert!(matches!(r, Some(Reply::Wrote { .. })), "{r:?}");
    assert!(started.elapsed() >= app::door::REQUEST_WAIT - std::time::Duration::from_millis(500), "the one who said nothing was waited for, no longer");
    drop(silent);
}

/// Requests are handled one at a time, in arrival order: two at once both land, one after the other.
#[test]
fn two_at_once_are_done_in_turn() {
    if super::alone_in(module_path!(), "two_at_once_are_done_in_turn") {
        return;
    }
    let (b, mut shell) = Bench::new("two");
    let one = send(&b.place(), b.ask("annotate", &[("note", "one")]).to_bytes());
    let two = send(&b.place(), b.ask("annotate", &[("note", "two")]).to_bytes());
    let mut seqs = Vec::new();
    for h in [one, two] {
        match served(&mut shell, h) {
            Some(Reply::Wrote { seq, .. }) => seqs.push(seq),
            other => panic!("{other:?}"),
        }
    }
    seqs.sort();
    assert_eq!(seqs, vec![1, 2]);
}

/// Locking while a request waits at the door tells it the desktop closed, and nothing of it is written.
#[test]
fn a_request_waiting_when_the_desktop_locks_is_told_it_closed() {
    if super::alone_in(module_path!(), "a_request_waiting_when_the_desktop_locks_is_told_it_closed") {
        return;
    }
    let (b, mut shell) = Bench::new("lockwait");
    let (tx, rx) = std::sync::mpsc::channel();
    let tx = std::sync::Mutex::new(tx);
    shell.door_waker = Some(std::sync::Arc::new(move || {
        let _ = tx.lock().map(|t| t.send(()));
    }));
    // The waker was handed to the door when it opened: open it again with this one.
    let _ = apply(&mut shell, Action::Lock);
    answers!(app::action::apply_settled(&mut shell, Action::Unlock { pin: PIN.into() }), Applied::Unlocked);
    shell.drain();
    let before = head(&shell);
    let h = send(&b.place(), b.ask("annotate", &[("note", "late")]).to_bytes());
    rx.recv_timeout(std::time::Duration::from_secs(30)).expect("the frame was woken when the request came");
    let _ = apply(&mut shell, Action::Lock);
    let r = h.join().ok().flatten().and_then(|x| Reply::of_bytes(&x));
    assert_eq!(r, Some(Reply::Closed));
    answers!(app::action::apply_settled(&mut shell, Action::Unlock { pin: PIN.into() }), Applied::Unlocked);
    assert_eq!(head(&shell), before, "nothing of it was written");
}

/// The command line's `anchor` follows the setting: left to the user, nothing is sent and the request is told
/// so; otherwise the estimate is asked first, like the send key (refused here, with no network). An empty queue
/// is reported as the send key reports it. The setting is stored in the machine settings.
#[test]
fn anchor_goes_the_way_the_setting_says() {
    if super::alone_in(module_path!(), "anchor_goes_the_way_the_setting_says") {
        return;
    }
    let (b, mut shell) = Bench::new("anchor");
    let code = |r: Option<Reply>| match r {
        Some(Reply::Refused { code, .. }) => code,
        other => format!("{other:?}"),
    };
    // Nothing queued (a home with no entry yet): refused by the batch's own name, whatever the setting.
    let empty = b.root.join("empty");
    let _ = apply(&mut shell, Action::OpenHome { root: empty.display().to_string() });
    settle(&mut shell);
    let empty_place = zikaron_glue::door::place(&b.machine, &std::fs::canonicalize(&empty).expect("empty"));
    let empty_ask = Request { verb: "anchor".into(), home: empty.display().to_string(), args: Vec::new() };
    assert_eq!(shell.queue.sendable(), 0);
    assert_eq!(code(served(&mut shell, send(&empty_place, empty_ask.to_bytes()))), app::fault::Known::QueueEmpty.as_str(), "nothing queued");
    let _ = apply(&mut shell, Action::OpenHome { root: b.home.display().to_string() });
    settle(&mut shell);
    let file = b.root.join("w.txt");
    std::fs::write(&file, b"w").expect("file");
    let before = shell.queue.sendable();
    assert!(matches!(served(&mut shell, send(&b.place(), b.ask("history", &[("file", &file.display().to_string())]).to_bytes())), Some(Reply::Wrote { .. })));
    let queued = shell.queue.sendable();
    assert_eq!(queued, before + 1, "the record is queued as a click's is");
    answers!(apply(&mut shell, Action::SetCliAnchor { to: app::machine::CliAnchor::Queue }), Applied::CliAnchorSet(app::machine::CliAnchor::Queue));
    assert_eq!(app::machine::read().expect("reads").cli_anchor, app::machine::CliAnchor::Queue, "written to the machine settings");
    let q_before = shell.queue.clone();
    shell.door_told.clear();
    assert_eq!(served(&mut shell, send(&b.place(), b.ask("anchor", &[]).to_bytes())), Some(Reply::Queued { count: queued as u64 }));
    assert_eq!(shell.queue, q_before, "nothing sent, the queue as it was");
    assert!(matches!(shell.door_told.as_slice(), [Applied::SendAsked { count }] if *count == queued), "the request is told: {:?}", shell.door_told);
    answers!(apply(&mut shell, Action::SetCliAnchor { to: app::machine::CliAnchor::Send }), Applied::CliAnchorSet(_));
    let r = served(&mut shell, send(&b.place(), b.ask("anchor", &[]).to_bytes()));
    assert_eq!(code(r), app::fault::Known::NoChainId.as_str(), "sent the send key's way: the estimate first, refused by its name");
}

/// A record package export through the door matches the desktop export: here, with no network for its exit
/// gate, refused by the same name, and nothing written at the destination.
#[test]
fn a_record_package_through_the_door_is_the_desktops_export() {
    if super::alone_in(module_path!(), "a_record_package_through_the_door_is_the_desktops_export") {
        return;
    }
    let (b, mut shell) = Bench::new("kit");
    let out = b.root.join("kit-out");
    let pressed = match apply(&mut shell, Action::ExportKit { from: String::new(), to: String::new(), ids: String::new(), attach: String::new(), note: "n".into(), out: out.display().to_string() }) {
        Applied::Trouble(f) => f.which().map(|k| k.as_str().to_string()).unwrap_or_default(),
        other => panic!("{other:?}"),
    };
    let r = served(&mut shell, send(&b.place(), b.ask("kit-export", &[("out", &out.display().to_string()), ("note", "n")]).to_bytes()));
    assert!(matches!(&r, Some(Reply::Refused { code, .. }) if *code == pressed), "{r:?} against {pressed}");
    assert!(!out.exists(), "nothing written");
}

/// A door that cannot open (its place taken by something else) is kept by name as the shell's reading, never
/// among its troubles (nobody pressed anything; the command line is told on its side), and opens once the
/// place is free. Unix only: there the door's place is a file path; on Windows it is a pipe name, and writing
/// to it reaches the pipe instead of taking its place.
#[cfg(unix)]
#[test]
fn a_door_that_cannot_open_is_kept_by_name() {
    if super::alone_in(module_path!(), "a_door_that_cannot_open_is_kept_by_name") {
        return;
    }
    let (b, mut shell) = Bench::new("taken");
    // Lock (the door goes), put something that is not a door at its place, unlock: the door cannot open.
    let _ = apply(&mut shell, Action::Lock);
    std::fs::write(b.place(), b"not a door").expect("taken");
    answers!(app::action::apply_settled(&mut shell, Action::Unlock { pin: PIN.into() }), Applied::Unlocked);
    for _ in 0..5 {
        shell.drain();
    }
    // The shell keeps the home path as opened; the bench resolves links (`/tmp` is a link on macOS).
    let kept = shell.door_shut.as_ref().map(|(h, f)| (std::fs::canonicalize(h).unwrap_or_else(|_| h.clone()), f.which()));
    assert_eq!(kept, Some((b.home.clone(), Some(app::fault::Known::DoorShut))), "kept by name as the shell's reading");
    assert!(shell.faults.iter().all(|f| f.which() != Some(app::fault::Known::DoorShut)), "never among the troubles: nobody pressed anything");
    assert!(shell.door.is_none());
    assert_eq!(std::fs::read(b.place()).expect("kept"), b"not a door", "what was there is left as it is");
    // With the place free again, the next drain opens it.
    std::fs::remove_file(b.place()).expect("freed");
    shell.drain();
    assert!(shell.door.is_some() && shell.door_shut.is_none());
}

/// The door's verbs and flags match `CLI-SCHEMA.md` §11 row by row; each verb maps to its action (the reverse
/// of `Action::verb`, in one place), and every flag reaches that action.
#[test]
fn the_door_table_is_the_contracts_and_each_flag_reaches_its_action() {
    if super::alone_in(module_path!(), "the_door_table_is_the_contracts_and_each_flag_reaches_its_action") {
        return;
    }
    let doc = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../CLI-SCHEMA.md")).expect("CLI-SCHEMA.md");
    let table = doc.split("## 11 · `--home`").nth(1).and_then(|s| s.split("| Verb | Flags beside `--home` |").nth(1)).expect("§11 table");
    let rows: Vec<(String, Vec<String>)> = table
        .lines()
        .filter_map(|l| l.strip_prefix("| `"))
        .map(|l| {
            let (verb, rest) = l.split_once('`').expect("verb");
            (verb.to_string(), rest.split('|').nth(1).unwrap_or_default().split('`').skip(1).step_by(2).map(str::to_string).collect())
        })
        .collect();
    let real: Vec<(String, Vec<String>)> = app::door::verbs().iter().map(|(v, f)| (v.to_string(), f.iter().map(|x| x.to_string()).collect())).collect();
    assert_eq!(rows, real, "the door's table is §11's");
    let (_b, shell) = Bench::new("table");
    for (verb, flags) in app::door::verbs() {
        // `anchors` takes the law's JSON form; the marker sits in its members, which reach the page's rows unchanged.
        let value = |f: &str| match f {
            "anchors" => format!(r#"[{{"chainId":1,"content":"v-{f}-1","payloadKind":"bare","tx":"v-{f}-1"}}]"#),
            _ => format!("v-{f}-1"),
        };
        let args: Vec<(String, String)> = flags.iter().map(|f| (f.to_string(), value(f))).collect();
        let q = Request { verb: verb.to_string(), home: String::new(), args };
        let act = match app::door::turn_of(&shell, &q).unwrap_or_else(|f| panic!("{verb}: {f:?}")) {
            app::door::Turn::Act(a) => a,
            app::door::Turn::Send { count } => Action::SendBatch { count },
        };
        assert_eq!(act.verb(), Some(verb), "{verb} becomes the action whose verb it is");
        let said = format!("{act:?}");
        for f in flags.iter() {
            assert!(said.contains(&format!("v-{f}-1")), "{verb} --{f} reaches the action: {said}");
        }
    }
}

/// Idle locking counts from the user's last input alone: only the window's input handling moves it, and the
/// door's path never does (a busy command line does not keep the desktop unlocked).
#[test]
fn the_door_does_not_hold_off_idle_locking() {
    let window = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/window/mod.rs")).expect("window");
    let moves: Vec<&str> = window.lines().filter(|l| l.contains("last_input =")).collect();
    assert_eq!(moves.len(), 1, "{moves:?}");
    let at = window.find(moves[0]).expect("there");
    let before = &window[window[..at].rfind('\n').map(|i| window[..i].rfind('\n').unwrap_or(0)).unwrap_or(0)..at];
    assert!(before.contains("i.events.is_empty()"), "moved only by input events: {before}");
    for f in ["src/door.rs", "src/shell.rs"] {
        let text = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(f)).expect("source");
        assert!(!text.contains("last_input"), "{f} never moves the idle moment");
    }
}

/// "Enable command line" goes through the action layer: its location is read by the platform layer (here, a
/// test binary with no command line beside it: not supported, named), and enabling is refused by that name with
/// nothing created (the target is moved to this test's folder; system folders are untouched). Unix only: on
/// Windows (MSVC) cargo names binaries in the build folder without a hash, so a `zikaron.exe` is beside it.
#[cfg(unix)]
#[test]
fn the_command_line_switch_reads_and_refuses_through_the_action_layer() {
    if super::alone_in(module_path!(), "the_command_line_switch_reads_and_refuses_through_the_action_layer") {
        return;
    }
    let (b, mut shell) = Bench::new("clipath");
    let dir = b.root.join("bin");
    // SAFETY: this test runs alone in its own process.
    unsafe { std::env::set_var(zikaron_os::cli_path::ENV_DIR, &dir) };
    let beside = app::action::cli_beside().expect("beside this program");
    assert!(!beside.is_file(), "a test binary has no command line beside it");
    answers!(apply(&mut shell, Action::ReadCliPath), Applied::CliPathRead(zikaron_os::cli_path::State::Unsupported(_)));
    assert!(matches!(shell.cli_path, Some(zikaron_os::cli_path::State::Unsupported(_))), "the row reads the shell's reading");
    let first = apply(&mut shell, Action::SetCliPath { on: true });
    assert_eq!(first, Applied::Started(app::task::Kind::CliPath));
    let mut said = None;
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while said.is_none() && std::time::Instant::now() < until {
        said = shell.drain().into_iter().find(|o| o.kind == app::task::Kind::CliPath).map(|o| o.result.err().and_then(|f| f.which()));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(said, Some(Some(app::fault::Known::CliPathUnsupported)));
    assert!(!dir.exists(), "nothing made where the switch writes");
    unsafe { std::env::remove_var(zikaron_os::cli_path::ENV_DIR) };
}

/// Dropping a shell closes its door: the next shell on the same home opens its own, and a command reaches it.
#[test]
fn a_shell_let_go_of_closes_its_door() {
    if super::alone_in(module_path!(), "a_shell_let_go_of_closes_its_door") {
        return;
    }
    let (b, shell) = Bench::new("dropped");
    assert!(shell.door.is_some());
    drop(shell);
    assert_eq!(zikaron_os::door::connect(&b.place()).err().map(|e| e.kind()), Some(std::io::ErrorKind::NotFound), "let go: no door");
    let mut next = boot();
    let _ = apply(&mut next, Action::OpenHome { root: b.home.display().to_string() });
    next.drain();
    assert!(next.door.is_some(), "the next shell opens its own");
    let r = served(&mut next, send(&b.place(), b.ask("annotate", &[("note", "after")]).to_bytes()));
    assert!(matches!(r, Some(Reply::Wrote { .. })), "{r:?}");
}

/// A co-signature is both halves or none, checked only in the adoption action (for the page and the door alike):
/// one half alone is refused naming the missing half, with nothing written; neither adopts without one; both are
/// verified, and a failing one is refused with nothing written. The door passes the halves on unchanged.
#[test]
fn a_cosignature_is_both_halves_or_none_judged_once() {
    let (b, mut shell) = Bench::new("cosign");
    let rows = format!("31337 0x{} bare 0x{}", "11".repeat(32), "77".repeat(32));
    let seq = |shell: &Shell| shell.home.as_ref().and_then(|h| app::ledgerx::head(h).ok().flatten()).map(|(n, _)| n);
    let attestor = format!("0x{}", "66".repeat(20));
    let attestation = format!("0x{}", "88".repeat(65));
    for (who, sig, missing) in [(attestor.as_str(), "", "attestation"), ("", attestation.as_str(), "attestor")] {
        let before = seq(&shell);
        let got = apply(&mut shell, Action::AdoptAnchors { rows: rows.clone(), attestor: who.into(), attestation: sig.into() });
        match got {
            Applied::Trouble(f) => assert_eq!((f.which(), f.tail()), (Some(app::fault::Known::FieldMissing), missing), "half a co-signature"),
            other => panic!("half a co-signature is refused, not {other:?}"),
        }
        assert_eq!(seq(&shell), before, "nothing written");
        // The door passes the halves on as given; only the action checks them.
        let q = b.ask("adopt", &[("anchors", &format!(r#"[{{"chainId":31337,"content":"0x{}","payloadKind":"bare","tx":"0x{}"}}]"#, "77".repeat(32), "11".repeat(32))), ("attestor", who), ("attestation", sig)]);
        match app::door::turn_of(&shell, &q) {
            Ok(app::door::Turn::Act(Action::AdoptAnchors { attestor, attestation, .. })) => assert_eq!((attestor.as_str(), attestation.as_str()), (who, sig), "as given"),
            other => panic!("the door judges nothing of a co-signature: {:?}", other.err()),
        }
    }
    let before = seq(&shell);
    let both = apply(&mut shell, Action::AdoptAnchors { rows: rows.clone(), attestor: attestor.clone(), attestation: attestation.clone() });
    assert!(matches!(&both, Applied::Trouble(f) if f.which() == Some(app::fault::Known::CosignRefused)), "both: checked, {both:?}");
    assert_eq!(seq(&shell), before, "a co-signature that does not verify writes nothing");
    let neither = apply(&mut shell, Action::AdoptAnchors { rows, attestor: String::new(), attestation: String::new() });
    assert!(matches!(neither, Applied::Adopted { cosigned: false, .. }), "neither: adopted without one, {neither:?}");
    assert_eq!(seq(&shell), before.map(|n| n + 1), "one entry more");
    let door = super::code_only(&super::read_src_file("door.rs").expect("door.rs"));
    assert!(!door.contains("attestation.trim()") && !door.contains("attestor.trim()"), "the door keeps no judge of its own");
}
