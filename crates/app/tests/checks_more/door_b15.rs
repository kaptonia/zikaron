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
    fn new(tag: &str) -> (Bench, Shell) {
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

fn home_with_genesis(shell: &mut Shell, at: &Path) -> PathBuf {
    super::seated(shell, at);
    let opened = apply(shell, Action::OpenHome { root: at.display().to_string() });
    assert!(matches!(opened, Applied::Homed { .. }), "{opened:?}");
    let made = apply(shell, Action::Genesis { statement: "一本账的开端".into() });
    assert!(matches!(made, Applied::Genesised { .. }), "{made:?}");
    std::fs::canonicalize(at).expect("home")
}

fn send(place: &Path, bytes: Vec<u8>) -> std::thread::JoinHandle<Option<Vec<u8>>> {
    let place = place.to_path_buf();
    std::thread::spawn(move || {
        let mut s = zikaron_os::door::connect(&place).ok()?;
        zikaron_glue::door::put(&mut s, &bytes).ok()?;
        zikaron_glue::door::take(&mut s).ok()
    })
}

fn served(shell: &mut Shell, h: std::thread::JoinHandle<Option<Vec<u8>>>) -> Option<Reply> {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !h.is_finished() && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    h.join().ok().flatten().and_then(|b| Reply::of_bytes(&b))
}

fn own_marks(from: usize) -> Vec<&'static str> {
    let all: Vec<&str> = app::trace::ring().into_iter().skip(from.saturating_sub(app::trace::dropped())).collect();
    let first = all.iter().position(|m| *m == app::feature::Feature::W1.id()).unwrap_or(all.len());
    all[first..].to_vec()
}

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

fn entry_bytes(shell: &Shell, id: &str) -> Vec<u8> {
    let pile = shell.home.as_ref().expect("home").ledger().expect("ledger").pile().expect("pile");
    pile.items.into_iter().find(|b| zikaron::hexfmt::encode(&zikaron::entry::entry_id(b)).eq_ignore_ascii_case(id)).expect("the entry")
}

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
    let other = home_with_genesis(&mut shell, &b.root.join("h2"));
    shell.drain();
    assert_eq!(zikaron_os::door::connect(&at).err().map(|e| e.kind()), Some(std::io::ErrorKind::NotFound), "the first home's door is shut");
    assert!(zikaron_os::door::connect(&zikaron_glue::door::place(&b.machine, &other)).is_ok(), "the open home has the door");
    apply(&mut shell, Action::Quit);
    shell.drain();
    assert!(shell.door.is_none() && zikaron_os::door::connect(&zikaron_glue::door::place(&b.machine, &other)).is_err(), "quit: shut");
    {
        let (left, _never_closed) = zikaron_os::door::listen(&b.place()).expect("left behind");
        drop(left);
        let mut again = boot();
        let _ = apply(&mut again, Action::OpenHome { root: b.home.display().to_string() });
        again.drain();
        assert!(again.door.is_some() && zikaron_os::door::connect(&b.place()).is_ok(), "a door left behind gave way");
    }
}

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

#[test]
fn a_request_is_done_by_the_action_layer_as_a_click_is() {
    if super::alone_in(module_path!(), "a_request_is_done_by_the_action_layer_as_a_click_is") {
        return;
    }
    let (b, mut shell) = Bench::new("same");
    let (_, genesis) = head(&shell);
    let first = b.home.clone();
    let twin = home_with_genesis(&mut shell, &b.root.join("twin"));
    settle(&mut shell);
    let marks = app::trace::ring().len() + app::trace::dropped();
    let pressed = apply(&mut shell, Action::Annotate { subject: genesis.clone(), note_md: "n".into() });
    let pressed_marks = own_marks(marks);
    let Applied::Annotated(pressed_id) = pressed else { panic!("{pressed:?}") };
    let pressed_bytes = entry_bytes(&shell, &pressed_id);
    assert_ne!(twin, first);
    super::seat_home(&first);
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

#[test]
fn each_writing_verb_lands_its_entry() {
    if super::alone_in(module_path!(), "each_writing_verb_lands_its_entry") {
        return;
    }
    let (b, mut shell) = Bench::new("each");
    super::venue::in_venue(&mut shell, &super::venue::venue());
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
    assert!(matches!(&r, Reply::Noticed { withdrawn: true, notice, .. } if notice == "counted"), "revoke: {r:?}");
    assert_eq!(head(&shell).0, 3, "the revocation entry landed");
    let r = ask(&mut shell, b.ask("retract", &[("subject", &work_entry), ("note", "gone")]));
    assert!(matches!(r, Reply::Wrote { seq: 4, .. }), "retract: {r:?}");
    let before = head(&shell);
    let r = ask(&mut shell, b.ask("migrate", &[("effective", "5")]));
    assert!(matches!(&r, Reply::Refused { code, network: false, .. } if code == "DOOR_UNREAD"), "migrate: {r:?}");
    assert_eq!(head(&shell), before, "nothing written");
    assert!(!me.is_empty());
    let r = ask(&mut shell, b.ask("retract", &[("subject", "not-an-id")]));
    assert!(matches!(&r, Reply::RuleRefused { rule: zikaron_glue::door::Rule::Retraction, token: Some(t), .. } if t == "E_RETRACT_SHAPE"), "{r:?}");
}

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
    let place = b.place();
    let over = std::thread::spawn(move || {
        use std::io::Write;
        let mut s = zikaron_os::door::connect(&place).ok()?;
        s.write_all(&((zikaron_glue::door::CAP + 1) as u32).to_be_bytes()).ok()?;
        zikaron_glue::door::take(&mut s).ok()
    });
    assert_eq!(code(served(&mut shell, over)), unread, "over the size");
    drop(zikaron_os::door::connect(&b.place()).expect("in"));
    let silent = zikaron_os::door::connect(&b.place()).expect("in");
    let started = std::time::Instant::now();
    let r = served(&mut shell, send(&b.place(), b.ask("annotate", &[("note", "after")]).to_bytes()));
    assert!(matches!(r, Some(Reply::Wrote { .. })), "{r:?}");
    assert!(started.elapsed() >= app::door::REQUEST_WAIT - std::time::Duration::from_millis(500), "the one who said nothing was waited for, no longer");
    drop(silent);
}

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

#[test]
fn a_record_package_through_the_door_is_the_desktops_export() {
    if super::alone_in(module_path!(), "a_record_package_through_the_door_is_the_desktops_export") {
        return;
    }
    let (b, mut shell) = Bench::new("kit");
    let out = b.root.join("kit-out");
    let pressed = match apply(&mut shell, Action::ExportKit { from: String::new(), to: String::new(), ids: String::new(), attach: String::new(), note: "n".into(), out: out.display().to_string(), form: "folder".into(), recipient: String::new(), recipient_key: String::new(), originals: "none".into(), companion: true }) {
        Applied::Trouble(f) => f.which().map(|k| k.as_str().to_string()).unwrap_or_default(),
        other => panic!("{other:?}"),
    };
    let r = served(&mut shell, send(&b.place(), b.ask("kit-export", &[("out", &out.display().to_string()), ("note", "n")]).to_bytes()));
    assert!(matches!(&r, Some(Reply::Refused { code, .. }) if *code == pressed), "{r:?} against {pressed}");
    assert!(!out.exists(), "nothing written");
}

#[cfg(unix)]
#[test]
fn a_door_that_cannot_open_is_kept_by_name() {
    if super::alone_in(module_path!(), "a_door_that_cannot_open_is_kept_by_name") {
        return;
    }
    let (b, mut shell) = Bench::new("taken");
    let _ = apply(&mut shell, Action::Lock);
    std::fs::write(b.place(), b"not a door").expect("taken");
    answers!(app::action::apply_settled(&mut shell, Action::Unlock { pin: PIN.into() }), Applied::Unlocked);
    for _ in 0..5 {
        shell.drain();
    }
    let kept = shell.door_shut.as_ref().map(|(h, f)| (std::fs::canonicalize(h).unwrap_or_else(|_| h.clone()), f.which()));
    assert_eq!(kept, Some((b.home.clone(), Some(app::fault::Known::DoorShut))), "kept by name as the shell's reading");
    assert!(shell.faults.iter().all(|f| f.which() != Some(app::fault::Known::DoorShut)), "never among the troubles: nobody pressed anything");
    assert!(shell.door.is_none());
    assert!(matches!(shell.door_why(), Some(app::door::DoorWhy::Refused(_))), "{:?}", shell.door_why());
    assert_eq!(std::fs::read(b.place()).expect("kept"), b"not a door", "what was there is left as it is");
    std::fs::remove_file(b.place()).expect("freed");
    shell.drain();
    assert!(shell.door.is_some() && shell.door_shut.is_none());
    assert_eq!(shell.door_why(), None, "open: nothing to say");
    let _ = apply(&mut shell, Action::Lock);
    std::fs::write(b.place(), b"not a door").expect("taken");
    answers!(app::action::apply_settled(&mut shell, Action::Unlock { pin: PIN.into() }), Applied::Unlocked);
    let _ = apply(&mut shell, Action::SetCliDesktop { on: false });
    shell.drain();
    assert_eq!(shell.door_why(), None, "not wanted: nothing to say");
    std::fs::remove_file(b.place()).expect("freed");
}

#[test]
fn each_flag_of_the_door_table_reaches_its_action() {
    if super::alone_in(module_path!(), "each_flag_of_the_door_table_reaches_its_action") {
        return;
    }
    let (_b, shell) = Bench::new("table");
    for (verb, flags) in app::door::verbs() {
        let value = |f: &str| match f {
            "anchors" => format!(r#"[{{"chainId":1,"content":"v-{f}-1","payloadKind":"bare","tx":"v-{f}-1"}}]"#),
            "chain" => "4242".to_string(),
            _ => format!("v-{f}-1"),
        };
        let args: Vec<(String, String)> = flags.iter().map(|f| (f.to_string(), value(f))).collect();
        let q = Request { verb: verb.to_string(), home: String::new(), args };
        let act = match app::door::turn_of(&shell, &q).unwrap_or_else(|f| panic!("{verb}: {f:?}")) {
            app::door::Turn::Act(a) => a,
            app::door::Turn::Send { count } => Action::SendBatch { count },
            app::door::Turn::Await => Action::SendAsked { count: 0 },
            app::door::Turn::Follow => Action::SendBatch { count: 0 },
        };
        assert_eq!(act.verb(), Some(verb), "{verb} becomes the action whose verb it is");
        let said = format!("{act:?}");
        for f in flags.iter() {
            let marker = if *f == "chain" { "4242".to_string() } else { format!("v-{f}-1") };
            assert!(said.contains(&marker), "{verb} --{f} reaches the action: {said}");
        }
    }
}

#[test]
fn a_window_not_shown_answers_the_door_and_still_locks() {
    if super::alone_in(module_path!(), "a_window_not_shown_answers_the_door_and_still_locks") {
        return;
    }
    let (b, mut shell) = Bench::new("unseen");
    settle(&mut shell);
    shell.machine.auto_lock = true;
    shell.machine.auto_lock_secs = 900;
    let ctx = zikaron_ui::egui::Context::default();
    let mut w = app::window::UnseenProbe::new(shell);
    let ask = |w: &mut app::window::UnseenProbe, q: Request| -> Option<Reply> {
        let h = send(&b.place(), q.to_bytes());
        let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while !h.is_finished() && std::time::Instant::now() < until {
            w.pass(&ctx, 0.001);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        h.join().ok().flatten().and_then(|b| Reply::of_bytes(&b))
    };
    let file = b.root.join("work.txt");
    std::fs::write(&file, b"the work").expect("file");
    let wrote = ask(&mut w, b.ask("history", &[("file", &file.display().to_string()), ("note", "a work")]));
    let (seq, id) = head(w.shell());
    let ledger = w.shell().home.as_ref().unwrap().dir(app::home::Slot::Ledger).display().to_string();
    assert_eq!(wrote, Some(Reply::Wrote { entry_id: id, ledger, seq }), "a write through the door, the window not shown");
    let cleaned = ask(&mut w, b.ask("clean", &[]));
    assert!(matches!(cleaned, Some(Reply::Cleaned { done: false, .. })), "a clean-up list through the door, the window not shown: {cleaned:?}");
    assert!(w.shell().unlocked());
    w.pass(&ctx, 901.0);
    assert!(!w.shell().unlocked(), "the idle lock falls due in a pass the window does not draw");
}

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

#[test]
fn an_anchor_through_the_door_asks_each_node_what_the_command_line_asks() {
    use super::a8_send::{chain, first_param, hash_of, node, Reply as Says, PLENTY};
    if super::alone_in(module_path!(), "an_anchor_through_the_door_asks_each_node_what_the_command_line_asks") {
        return;
    }
    let (b, mut shell) = Bench::new("two-roads");
    let sent: std::sync::Arc<std::sync::Mutex<Option<String>>> = std::sync::Arc::default();
    let answering = |sent: std::sync::Arc<std::sync::Mutex<Option<String>>>| {
        move |m: &str, body: &str| match m {
            "eth_sendRawTransaction" => {
                let h = hash_of(&first_param(body));
                *sent.lock().expect("sent") = Some(h.clone());
                Says::Result(format!("\"{h}\""))
            }
            "eth_getTransactionReceipt" => match sent.lock().expect("sent").clone() {
                Some(h) => Says::Result(format!("{{\"blockHash\":\"0x{}\",\"blockNumber\":\"0x40\",\"logs\":[],\"status\":\"0x1\",\"transactionHash\":\"{h}\"}}", "aa".repeat(32))),
                None => Says::Result("null".into()),
            },
            _ => chain(m, body, PLENTY),
        }
    };
    let (one, two) = (node(answering(sent.clone())), node(answering(sent.clone())));
    let file = b.root.join("w.txt");
    std::fs::write(&file, b"w").expect("file");
    assert!(matches!(served(&mut shell, send(&b.place(), b.ask("history", &[("file", &file.display().to_string())]).to_bytes())), Some(Reply::Wrote { .. })));
    settle(&mut shell);
    shell.settings.chain_id = Some(31337);
    shell.settings.registry = Some(app::key::Address([0x11; 20]));
    shell.send_backoff = Vec::new();
    shell.receipt_backoff = Vec::new();
    shell.endpoints = [&one, &two].iter().map(|n| app::chainx::Endpoint::at(31337, n.url.clone())).collect();
    answers!(apply(&mut shell, Action::SetCliAnchor { to: app::machine::CliAnchor::Send }), Applied::CliAnchorSet(_));
    let r = served(&mut shell, send(&b.place(), b.ask("anchor", &[]).to_bytes()));
    assert!(matches!(r, Some(Reply::Anchored { .. })), "anchored through the door: {r:?}");
    for (n, at) in [("one", &one), ("two", &two)] {
        let asked = at.methods();
        let upto = asked.iter().position(|m| m == "eth_getTransactionCount").map(|i| i + 1).unwrap_or(asked.len());
        let count = |m: &str| asked[..upto].iter().filter(|x| *x == m).count();
        println!("SAID door anchor {n}: {:?}", &asked[..upto]);
        assert_eq!(count("eth_chainId"), 1, "{n}: the chain id, once: {asked:?}");
        assert_eq!(count("eth_blockNumber"), 1, "{n}: the head, once: {asked:?}");
        assert_eq!(count("eth_getLogs"), 1, "{n}: the lineage's window, once: {asked:?}");
    }
}

#[test]
fn an_anchor_over_nodes_at_two_heads_asks_each_node_the_window_once() {
    use super::a8_send::{chain, first_param, hash_of, node, Reply as Says, PLENTY};
    if super::alone_in(module_path!(), "an_anchor_over_nodes_at_two_heads_asks_each_node_the_window_once") {
        return;
    }
    let (b, mut shell) = Bench::new("two-heads");
    let sent: std::sync::Arc<std::sync::Mutex<Option<String>>> = std::sync::Arc::default();
    let answering = |sent: std::sync::Arc<std::sync::Mutex<Option<String>>>, head: &'static str| {
        move |m: &str, body: &str| match m {
            "eth_blockNumber" => Says::Result(head.into()),
            "eth_sendRawTransaction" => {
                let h = hash_of(&first_param(body));
                *sent.lock().expect("sent") = Some(h.clone());
                Says::Result(format!("\"{h}\""))
            }
            "eth_getTransactionReceipt" => match sent.lock().expect("sent").clone() {
                Some(h) => Says::Result(format!("{{\"blockHash\":\"0x{}\",\"blockNumber\":\"0x40\",\"logs\":[],\"status\":\"0x1\",\"transactionHash\":\"{h}\"}}", "aa".repeat(32))),
                None => Says::Result("null".into()),
            },
            _ => chain(m, body, PLENTY),
        }
    };
    let (ahead, behind) = (node(answering(sent.clone(), "\"0x40\"")), node(answering(sent.clone(), "\"0x3f\"")));
    let file = b.root.join("w.txt");
    std::fs::write(&file, b"w").expect("file");
    assert!(matches!(served(&mut shell, send(&b.place(), b.ask("history", &[("file", &file.display().to_string())]).to_bytes())), Some(Reply::Wrote { .. })));
    settle(&mut shell);
    shell.settings.chain_id = Some(31337);
    shell.settings.registry = Some(app::key::Address([0x11; 20]));
    shell.send_backoff = Vec::new();
    shell.receipt_backoff = Vec::new();
    shell.endpoints = [&ahead, &behind].iter().map(|n| app::chainx::Endpoint::at(31337, n.url.clone())).collect();
    answers!(apply(&mut shell, Action::SetCliAnchor { to: app::machine::CliAnchor::Send }), Applied::CliAnchorSet(_));
    let r = served(&mut shell, send(&b.place(), b.ask("anchor", &[]).to_bytes()));
    assert!(matches!(r, Some(Reply::Anchored { .. })), "anchored through the door: {r:?}");
    for (n, at) in [("ahead", &ahead), ("behind", &behind)] {
        let asked = at.methods();
        let upto = asked.iter().position(|m| m == "eth_getTransactionCount").map(|i| i + 1).unwrap_or(asked.len());
        let count = |m: &str| asked[..upto].iter().filter(|x| *x == m).count();
        assert_eq!(count("eth_getLogs"), 1, "{n}: the lineage's window, once: {asked:?}");
    }
}

