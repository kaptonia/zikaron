use super::{seated, vault_open};
use app::action::{apply, apply_settled, Action, Applied};
use std::path::{Path, PathBuf};

fn base(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("zk-frameturn-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("a scratch folder");
    d
}

fn author(at: &Path) -> app::shell::Shell {
    vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    seated(&mut shell, at);
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: at.display().to_string() }), Applied::Homed { .. }));
    assert!(matches!(apply_settled(&mut shell, Action::Genesis { statement: "frame turn".into() }), Applied::Genesised { .. }));
    shell.seat_identities(app::register::view().ok());
    shell
}

fn base_now(shell: &app::shell::Shell) -> app::shell::Base {
    app::shell::Base { r#gen: shell.seen_gen, entries: shell.seen_entries, queue: shell.seen_queue, pens: shell.seen_pens, rooted: shell.rooted }
}

fn queue_moved_elsewhere(root: &Path) -> app::queue::Queue {
    use app::queue::Kept as _;
    let home = app::home::Home::bare(root.to_path_buf());
    let id = app::queue::Queue::read(&home).expect("the queue").items.first().map(|q| q.id.clone()).expect("a queued entry");
    let sub = app::queue::Step::Submitted { tx: format!("0x{}", "5e".repeat(32)), chain: 31337, nonce: Some(0) };
    app::queue::amend(&home, |q| q.mark(std::slice::from_ref(&id), sub)).expect("written behind the window").1
}

#[test]
fn a_look_lands_only_against_the_baseline_it_asked() {
    if super::alone_in(module_path!(), "a_look_lands_only_against_the_baseline_it_asked") {
        return;
    }
    let at = base("look");
    let root = at.join("home");
    let mut shell = author(&root);
    let other = at.join("other");
    std::fs::create_dir_all(&other).expect("another folder");
    let moved = queue_moved_elsewhere(&root);
    let dropped = shell.looks_dropped;
    let mut stale = base_now(&shell);
    stale.r#gen = stale.r#gen.wrapping_sub(1);
    assert!(!shell.looked(&app::shell::look(&root, stale)), "an older baseline lands nothing");
    assert!(!shell.looked(&app::shell::look(&other, base_now(&shell))), "another home's look lands nothing");
    shell.swapping = true;
    assert!(!shell.looked(&app::shell::look(&root, base_now(&shell))), "a frozen home lands nothing");
    shell.swapping = false;
    assert_eq!(shell.looks_dropped, dropped + 3, "each dropped form counted");
    assert_ne!(shell.queue, moved, "nothing of a dropped look was taken");
    assert!(shell.looked(&app::shell::look(&root, base_now(&shell))), "the look against this baseline lands");
    assert_eq!(shell.queue.items, moved.items, "the queue as the other writer left it");
    let l = app::shell::look(&root, base_now(&shell));
    let _ = apply(&mut shell, Action::Lock);
    assert!(!shell.looked(&l), "a locked window lands nothing");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn the_look_at_the_disk_is_a_task_behind_the_window() {
    if super::alone_in(module_path!(), "the_look_at_the_disk_is_a_task_behind_the_window") {
        return;
    }
    let at = base("task");
    let root = at.join("home");
    let mut shell = author(&root);
    let moved = queue_moved_elsewhere(&root);
    let now = if shell.followed_at.is_finite() { shell.followed_at.max(0.0) } else { 0.0 } + 100.0;
    let starts = shell.tasks.starts(app::task::Kind::Look);
    shell.follow_disk(now);
    assert_eq!(shell.tasks.starts(app::task::Kind::Look), starts + 1, "the look started as a task");
    assert_ne!(shell.queue.items, moved.items, "nothing read on the turn itself");
    super::look_landed(&mut shell, now);
    assert_eq!(shell.queue.items, moved.items, "landed at the drain after it");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn letting_the_lock_go_counts_nothing_and_a_write_after_it_is_followed() {
    if super::alone_in(module_path!(), "letting_the_lock_go_counts_nothing_and_a_write_after_it_is_followed") {
        return;
    }
    let at = base("letgo");
    let root = at.join("home");
    let mut shell = author(&root);
    assert!(matches!(apply_settled(&mut shell, Action::SetCliAlone { lasts: app::machine::Lasts::Never, pin: "27618394".into() }), Applied::CliAloneSet(_)));
    let _ = shell.drain();
    assert!(matches!(apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "the window's own".into() }), Applied::Annotated(_)));
    let counted = shell.seen_entries;
    let _ = shell.drain();
    assert!(shell.lock.as_ref().is_some_and(|l| !l.held()), "let go between writes");
    assert_eq!(shell.seen_entries, counted, "letting go counts nothing");
    let queue_before = shell.seen_queue;
    let moved = queue_moved_elsewhere(&root);
    let now = if shell.followed_at.is_finite() { shell.followed_at.max(0.0) } else { 0.0 } + 100.0;
    super::look_landed(&mut shell, now);
    assert_ne!(shell.seen_queue, queue_before, "the other writer's change seen");
    assert_eq!(shell.queue.items, moved.items, "and read in");
    assert_eq!(shell.seen_entries, counted, "no entry of its own taken for another's");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn the_lock_taken_again_behind_stops_by_itself_and_takes_it_once_free() {
    if super::alone_in(module_path!(), "the_lock_taken_again_behind_stops_by_itself_and_takes_it_once_free") {
        return;
    }
    let at = base("regain");
    let root = at.join("home");
    let mut shell = author(&root);
    assert!(matches!(apply_settled(&mut shell, Action::SetCliAlone { lasts: app::machine::Lasts::Never, pin: "27618394".into() }), Applied::CliAloneSet(_)));
    let _ = shell.drain();
    let lock = shell.lock.as_ref().expect("the lock");
    assert!(!lock.held(), "let go while writing alone");
    let hand = lock.hand();
    let machine = app::home::machine_dir().expect("the machine directory");
    let cli = zikaron_seal::writer::take(&root, &machine).expect("the command line's turn");
    assert!(matches!(cli.took, zikaron_seal::writer::Took::Writer));
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (h, r, s) = (hand.clone(), root.clone(), stop.clone());
    let asking = std::thread::spawn(move || app::lock::regain_behind(&h, &r, &s));
    std::thread::sleep(std::time::Duration::from_millis(300));
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    assert!(matches!(asking.join().expect("joined"), Ok(app::lock::Regained::Stopped)), "stopped when told");
    assert!(!shell.lock.as_ref().expect("the lock").held(), "nothing taken");
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (h, r, s) = (hand.clone(), root.clone(), stop.clone());
    let asking = std::thread::spawn(move || app::lock::regain_behind(&h, &r, &s));
    std::thread::sleep(std::time::Duration::from_millis(300));
    drop(cli);
    assert!(matches!(asking.join().expect("joined"), Ok(app::lock::Regained::Took)), "taken once free");
    assert!(shell.lock.as_ref().expect("the lock").held(), "in the window's hand");
    shell.lock.as_mut().expect("the lock").release();
    let cli = zikaron_seal::writer::take(&root, &machine).expect("the command line's turn again");
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let (h, r, s) = (hand, root.clone(), stop);
    let asking = std::thread::spawn(move || app::lock::regain_behind(&h, &r, &s));
    std::thread::sleep(std::time::Duration::from_millis(300));
    shell.lock = None;
    assert!(matches!(asking.join().expect("joined"), Ok(app::lock::Regained::Stopped)), "stopped once the home's lock is let go");
    drop(cli);
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn the_next_batch_after_a_write_is_read_with_its_table() {
    if super::alone_in(module_path!(), "the_next_batch_after_a_write_is_read_with_its_table") {
        return;
    }
    let at = base("lane");
    let root = at.join("home");
    let mut shell = author(&root);
    let _ = apply(&mut shell, Action::ReadLedger);
    assert!(matches!(apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "one more".into() }), Applied::Annotated(_)));
    assert!(!shell.rows_current(), "the table is of the ledger before the write");
    let here = shell.sendable_here();
    assert!(here > 0, "a pressed key reads the batch off the ledger");
    assert_eq!(shell.sendable_now(), here, "and the frame takes what the pressed key read for this generation");
    assert!(matches!(apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "and one more".into() }), Applied::Annotated(_)));
    assert_eq!(shell.sendable_now(), 0, "a frame before the table lands: still reading, nothing read off the disk");
    let _ = apply(&mut shell, Action::ReadLedger);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !shell.rows_current() && std::time::Instant::now() < until {
        let _ = shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(shell.sendable_now() > 0, "the table landed with the batch");
    assert_eq!(shell.sendable_now(), shell.sendable_here(), "the frame has what a pressed key reads");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_reading_started_before_a_write_here_lands_nothing_and_is_read_again() {
    if super::alone_in(module_path!(), "a_reading_started_before_a_write_here_lands_nothing_and_is_read_again") {
        return;
    }
    use app::reads::{ticket, Copies};
    use app::shell::{Keep, Kept};
    let at = base("copies");
    let root = at.join("home");
    let mut shell = author(&root);
    let home = app::home::Home::bare(root.clone());
    let _ = shell.drain();
    let t = ticket(Copies::Settings);
    let old = app::settings::Settings::read(&home).expect("the settings");
    let _ = apply(&mut shell, Action::SetAutoAnchor { on: !old.auto_anchor });
    shell.kept(&[Kept::Settings { root: root.clone(), settings: Ok(old.clone()), at: t }]);
    assert_eq!(shell.settings.auto_anchor, !old.auto_anchor, "the settings keep what was pressed");
    assert!(shell.keeps.iter().any(|k| matches!(k, Keep::Settings { .. })), "the settings read again");
    assert!(matches!(apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "queued".into() }), Applied::Annotated(_)));
    let t = ticket(Copies::Queue);
    let old_queue = { use app::queue::Kept as _; app::queue::Queue::read(&home).expect("the queue") };
    let pressed = queue_moved_elsewhere(&root);
    shell.queue = pressed.clone();
    shell.kept(&[Kept::Queue { root: root.clone(), queue: Ok(old_queue), at: t }]);
    assert_eq!(shell.queue.items, pressed.items, "the queue keeps what was written");
    assert!(shell.keeps.iter().any(|k| matches!(k, Keep::Queue { .. })), "the queue read again");
    let t = ticket(Copies::Machine);
    let old_machine = app::machine::read().expect("the machine settings");
    let look = if old_machine.appearance.as_deref() == Some("dark") { "light" } else { "dark" };
    let _ = apply(&mut shell, Action::SetAppearance { appearance: look.into() });
    shell.kept(&[Kept::Machine(Ok(old_machine), t)]);
    assert_eq!(shell.machine.appearance.as_deref(), Some(look), "the machine settings keep what was pressed");
    assert!(shell.keeps.iter().any(|k| matches!(k, Keep::Machine)), "the machine settings read again");
    let t = ticket(Copies::Register);
    let old_reg = app::register::view().expect("the register");
    let id = shell.identities.as_ref().and_then(|r| r.now()).map(|(r, _)| r.id.clone()).expect("an identity");
    let _ = apply(&mut shell, Action::NameIdentity { id: id.clone(), label: "renamed here".into() });
    shell.kept(&[Kept::Register(Ok(old_reg), t)]);
    let label = |shell: &app::shell::Shell| shell.identities.as_ref().and_then(|r| r.rows.iter().find(|x| x.id == id).map(|x| x.label.clone()));
    assert_eq!(label(&shell).as_deref(), Some("renamed here"), "the register keeps what was pressed");
    assert!(shell.keeps.iter().any(|k| matches!(k, Keep::Register)), "the register read again");
    let _ = shell.drain();
    assert_eq!(shell.settings.auto_anchor, !old.auto_anchor);
    assert_eq!(shell.queue.items, pressed.items);
    assert_eq!(shell.machine.appearance.as_deref(), Some(look));
    assert_eq!(label(&shell).as_deref(), Some("renamed here"));
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_write_for_a_home_left_is_under_its_own_lock() {
    if super::alone_in(module_path!(), "a_write_for_a_home_left_is_under_its_own_lock") {
        return;
    }
    let at = base("ownlock");
    let root = at.join("home");
    let mut shell = author(&root);
    let other = at.join("other");
    std::fs::create_dir_all(&other).expect("another folder");
    let theirs = app::lock::take(&app::home::Home::bare(other.clone())).expect("the other home's lock");
    shell.lock.as_mut().expect("the lock").release();
    let machine = app::home::machine_dir().expect("the machine directory");
    let busy = app::lock::settling(Some(&theirs.hand()), &root, || matches!(zikaron_seal::writer::take(&root, &machine).map(|t| t.took), Ok(zikaron_seal::writer::Took::Busy { .. })));
    assert_eq!(busy.ok(), Some(true), "the left home's lock is held for its write, not the other home's");
    drop(theirs);
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn an_empty_slot_asks_for_the_table_and_says_reading() {
    if super::alone_in(module_path!(), "an_empty_slot_asks_for_the_table_and_says_reading") {
        return;
    }
    let at = base("slot");
    let root = at.join("home");
    let mut shell = author(&root);
    let _ = apply(&mut shell, Action::ReadLedger);
    let _ = shell.drain();
    assert!(matches!(apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "after".into() }), Applied::Annotated(_)));
    shell.rows_wanted.store(false, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(shell.sendable_now(), 0, "nothing counted off the disk in a frame");
    assert!(shell.sendable_reading(), "the count waits on the table's reading");
    assert!(shell.rows_wanted.load(std::sync::atomic::Ordering::Relaxed), "the table is wanted");
    let _ = apply(&mut shell, Action::ReadLedger);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while !shell.rows_current() && std::time::Instant::now() < until {
        let _ = shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(shell.sendable_now() > 0, "counted once the table landed");
    assert!(!shell.sendable_reading(), "nothing waits");
    let _ = std::fs::remove_dir_all(&at);
}
