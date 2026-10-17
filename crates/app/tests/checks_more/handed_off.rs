//! A ledger handed off to another place (the family's handoff, `zikaron_seal::handover`; the command line's
//! `hand-over`) is not written by the desktop until a handoff after it comes back: refused by name
//! (`HANDED_OFF`), the ledger as it was. Every desktop write of an entry and every command-line write ask the pen
//! list (`zikaron_seal::pens`) first. Each test runs alone (its own machine directory).

use app::action::{apply, Action, Applied};
use zikaron_seal::handover::{HandId, Pen};

fn root_id(home: &app::home::Home) -> [u8; 32] {
    let r = app::ledgerx::root_of(home).expect("a root");
    zikaron::hexfmt::decode(&r).and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok()).expect("hex32")
}

fn set_pen(root: &[u8; 32], pen: Pen) {
    let machine = app::home::machine_dir().expect("machine");
    let mut pens = zikaron_seal::pens::read(&machine).expect("the list");
    pens.set(&zikaron_seal::pens::key_of(&machine, root).expect("key"), pen);
    zikaron_seal::pens::write(&machine, &pens).expect("written");
}

#[test]
fn a_handed_off_ledger_is_not_written_by_the_desktop_until_it_comes_back() {
    if super::alone_in(module_path!(), "a_handed_off_ledger_is_not_written_by_the_desktop_until_it_comes_back") {
        return;
    }
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.reread_vault();
    let at = std::env::temp_dir().join(format!("zk-handed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    super::seated(&mut shell, &at);
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: at.display().to_string() }), Applied::Homed { .. }));
    assert!(matches!(apply(&mut shell, Action::Genesis { statement: "start".into() }), Applied::Genesised { .. }));
    let home = app::home::Home::bare(shell.home.as_ref().expect("a home").root().to_path_buf());
    let root = root_id(&home);
    let count = |h: &app::home::Home| h.ledger().expect("ledger").pile().expect("pile").items.len();
    let before = count(&home);
    set_pen(&root, Pen::Handed { id: HandId([7; 16]), after: None });
    match apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "while handed".into() }) {
        Applied::Trouble(f) => assert!(f.said().starts_with("HANDED_OFF"), "{}", f.said()),
        other => panic!("refused by name: {other:?}"),
    }
    assert_eq!(count(&home), before, "not one entry written");
    set_pen(&root, Pen::Holds { took: Some(HandId([8; 16])) });
    assert!(matches!(apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "back".into() }), Applied::Annotated(_)), "the pen came back");
    assert_eq!(count(&home), before + 1);
    let _ = std::fs::remove_dir_all(&at);
}

/// Every writer asks the pen list: the desktop's one gate before an entry (`Shell::may_write_entries`), its
/// adoption of a folder's entries (`firstrun::adopt`), and the command line's one write gate (`seal_and_append`).
#[test]
fn every_writer_asks_the_pen_list_before_an_entry() {
    let code = |krate: &str, file: &str| std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(krate).join("src").join(file)).expect("source");
    let shell = code("app", "shell.rs");
    let at = shell.find("pub fn may_write_entries(").expect("the gate");
    assert!(shell[at..].split("\n    }\n").next().unwrap_or("").contains("zikaron_seal::pens::may_write("), "the desktop's gate");
    let firstrun = code("app", "firstrun.rs");
    let at = firstrun.find("pub fn adopt(").expect("adopt");
    let body = firstrun[at..].split("\n}\n").next().unwrap_or("");
    let asked = body.find("pens::may_write(").expect("adopt asks");
    assert!(asked < body.find("book.append(").expect("appends"), "before the first entry");
    let verbs = code("zikaron-cli", "verbs.rs");
    let at = verbs.find("fn seal_and_append(").expect("the write gate");
    let body = verbs[at..].split("\n}\n").next().unwrap_or("");
    assert!(body.find("handed_here(").expect("asks") < body.find("entry::seal(").expect("seals"), "before the entry is made");
}

/// The desktop wakes to look at the disk once a second while the command line writes alone and it is unlocked
/// (`Shell::follow_wake`), at no other time; the window asks that wake where it asks its other timers'
/// (`window/watch.rs` `tick`, one wake, the earliest).
#[test]
fn the_desktop_wakes_to_look_at_the_disk_only_while_writing_alone_and_unlocked() {
    if super::alone_in(module_path!(), "the_desktop_wakes_to_look_at_the_disk_only_while_writing_alone_and_unlocked") {
        return;
    }
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.reread_vault();
    let at = std::env::temp_dir().join(format!("zk-wake-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&at);
    super::seated(&mut shell, &at);
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: at.display().to_string() }), Applied::Homed { .. }));
    assert!(matches!(apply(&mut shell, Action::Genesis { statement: "start".into() }), Applied::Genesised { .. }));
    let now = 1_000.0;
    let _ = shell.drain_at(now);
    assert_eq!(shell.follow_wake(now + 0.2), None, "writing alone off: no wake for the disk");
    match app::action::apply_settled(&mut shell, Action::SetCliAlone { lasts: app::machine::Lasts::Never, pin: "27618394".into() }) {
        Applied::CliAloneSet(Some(_)) => {}
        other => panic!("writing alone on: {other:?}"),
    }
    let _ = shell.drain_at(now + 2.0);
    let wake = shell.follow_wake(now + 2.4).expect("a wake while writing alone");
    assert!(wake <= app::shell::FOLLOW_EVERY && wake > 0.0, "{wake}");
    assert_eq!(shell.follow_wake(now + 9.0), Some(0.0), "overdue: at once");
    assert!(matches!(apply(&mut shell, Action::Lock), Applied::LockedUp { .. }));
    assert_eq!(shell.follow_wake(now + 9.0), None, "locked: no wake for the disk");
    let watch = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/window/watch.rs")).expect("watch.rs");
    let at_tick = watch.find("pub(super) fn tick(").expect("tick");
    let tick = &watch[at_tick..];
    let asks = tick.find("follow_wake(").expect("the wake for the disk is asked in tick");
    assert!(asks < tick.find("request_repaint_after(").expect("one request"), "in the same one wake");
    assert_eq!(tick.split("\n    }\n").next().unwrap_or("").matches("request_repaint_after(").count(), 1, "one place asks");
    let _ = std::fs::remove_dir_all(&at);
}
