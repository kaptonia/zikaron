use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use app::action::{apply, Action, Applied};
use app::lang::{fill1, t, Key};
use app::nav::{Place, View};
use app::shell::Shell;
use app::task::{Done, Kind};
use zikaron_ui::egui;

type Frame = Vec<String>;

struct Capture {
    frames: Arc<Mutex<Vec<Frame>>>,
    hold: usize,
    release: Arc<AtomicBool>,
}

fn texts_in(s: &egui::Shape, out: &mut Frame) {
    match s {
        egui::Shape::Text(x) => out.push(x.galley.job.text.clone()),
        egui::Shape::Vec(v) => v.iter().for_each(|x| texts_in(x, out)),
        _ => {}
    }
}

impl egui::Plugin for Capture {
    fn debug_name(&self) -> &'static str {
        "refresh-capture"
    }

    fn output_hook(&mut self, output: &mut egui::FullOutput) {
        let mut frame = Vec::new();
        for c in &output.shapes {
            texts_in(&c.shape, &mut frame);
        }
        let mut frames = self.frames.lock().unwrap();
        frames.push(frame);
        if frames.len() >= self.hold {
            self.release.store(true, Ordering::SeqCst);
        }
    }
}

fn settle(shell: &mut Shell) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    shell.drain();
    while !shell.tasks.flying().is_empty() && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    shell.drain();
}

fn a_home(tag: &str) -> Shell {
    use zikaron_glue::book::{self, Law};
    super::vault_open();
    let ctx = egui::Context::default();
    let mut shell = Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.reread_vault();
    let base = std::env::temp_dir().join(format!("zk-refresh-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let root = base.join("home");
    super::seated(&mut shell, &root);
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: root.display().to_string() }), Applied::Homed { .. }));
    settle(&mut shell);
    let secret = app::key::load(app::register::account_now().ok().flatten().as_deref()).expect("the vault").expect("a key");
    let author = secret.address().expect("an address").hex();
    let ledger = shell.home.as_ref().expect("a home").ledger().expect("its ledger");
    let lay = |law: Law, kind: &str, seq: u64, prev: Option<&str>, body: zikaron::json::Value| -> String {
        let e = app::entryx::seal_under(law, &secret, kind, seq, prev, body).expect("sealed");
        let name = zikaron_store::EntryName::parse(e.id.trim_start_matches("0x")).expect("a name");
        ledger.append(&name, &e.bytes).expect("laid");
        e.id
    };
    let obj = |m: Vec<(&str, zikaron::json::Value)>| zikaron::json::Value::Obj(m.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
    let st = |x: &str| zikaron::json::Value::Str(x.to_string());
    let g = lay(Law::One, "genesis", 0, None, obj(vec![("statement_md", st("refresh"))]));
    let mode = obj(vec![("mark", st("v1")), ("toolchain", st(&format!("0x{}", "0b".repeat(32))))]);
    let h = lay(Law::One, "history", 1, Some(&g), obj(vec![("content", st(&format!("0x{}", "0a".repeat(32)))), ("mode", mode)]));
    let r = lay(Law::One, zikaron_glue::retraction::ENTRY_TYPE, 2, Some(&h), obj(vec![("note_md", st("gone")), ("subject", st(&h))]));
    let c = lay(Law::One, "succession", 3, Some(&r), book::closing_body(&author, 1, "moved"));
    lay(Law::Two, "genesis", 0, None, book::genesis_body("refresh", Some((&author, &c))));
    let _ = apply(&mut shell, Action::ReadLedger);
    settle(&mut shell);
    assert!(shell.rows_current(), "the table read");
    assert_eq!(app::retractx::old_retractions(&shell.rows.as_ref().expect("rows").0), 1, "the old segment's deletion is in the table");
    shell
}

fn land_a_report(shell: &mut Shell) {
    land_a_report_naming(shell, zikaron::v2::tokens::Key::UnknownType);
}

fn land_a_report_naming(shell: &mut Shell, item: zikaron::v2::tokens::Key) {
    use zikaron::json::Value;
    let entries = shell.rows.as_ref().map(|(r, _)| r.len()).unwrap_or(0);
    let report = Value::Obj(vec![(item.as_str().to_string(), Value::Arr(vec![Value::Str("deletion".into())]))]);
    let done = Done::Audited {
        label: zikaron::v2::tokens::Label::Complete.as_str().to_string(),
        complete: true,
        broken: false,
        entries,
        report,
        unanswered: Vec::new(), unproven: Vec::new(),
        asked: 1,
        single_source: true,
        fragment: app::auditx::empty_fragment(),
        recounted: false,
    };
    assert_eq!(shell.tasks.spawn(Kind::Audit, move || Ok(done)), app::task::Spawned::Started);
    settle(shell);
}

const SETTLE: usize = 5;
const HELD: usize = 20;

fn frames(mut shell: Shell, place: Place, n: usize) -> (Shell, Vec<Frame>) {
    let release = Arc::new(AtomicBool::new(false));
    let (rows, strays) = shell.rows.clone().expect("a table held");
    let (handed, r#gen, go) = (shell.handed.clone(), shell.rows_gen, release.clone());
    let upgrade = shell.upgrade.clone().unwrap_or(app::upgradex::Stage::Nothing);
    let started = shell.tasks.spawn(Kind::Ledger, move || {
        while !go.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        Ok(Done::Ledger { rows, strays, handed, office: None, upgrade, upgrade_read: false, seats: Default::default(), faces: Default::default(), places: Default::default(), r#gen })
    });
    assert_eq!(started, app::task::Spawned::Started);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    ctx.add_plugin(Capture { frames: seen.clone(), hold: HELD, release });
    let steps = (0..n).map(|_| (0.1, Vec::new())).collect();
    let (mut shell, _) = app::window::probe_shell_input(&ctx, shell, place, steps, 1180.0, 760.0);
    settle(&mut shell);
    if !shell.rows_current() {
        let _ = apply(&mut shell, Action::ReadLedger);
        settle(&mut shell);
    }
    let frames = seen.lock().unwrap().clone();
    (shell, frames)
}

const LEDGER: Place = Place::View(View::Log, 0);

#[test]
fn a_refreshed_table_is_drawn_until_the_new_reading_lands() {
    if super::alone_in(module_path!(), "a_refreshed_table_is_drawn_until_the_new_reading_lands") {
        return;
    }
    let mut shell = a_home("table");
    let n = shell.rows.as_ref().map(|(r, _)| r.len()).expect("rows");
    let count = fill1(Key::U3EntriesTotal, &n.to_string());
    shell.stale_rows();
    assert!(!shell.rows_current() && shell.rows.is_some(), "stale, and still held");
    let (shell, frames) = frames(shell, LEDGER, 30);
    assert!(frames.len() > HELD);
    for (i, f) in frames.iter().enumerate().skip(SETTLE) {
        assert!(f.iter().any(|x| x.contains(&count)), "frame {i}: the table is drawn ({count}): {f:?}");
        assert!(!f.iter().any(|x| x == t(Key::WbNotRead)), "frame {i}: never \"not read\"");
    }
    assert!(shell.rows_current(), "the new reading landed");
}

#[test]
fn the_ledger_state_holds_its_conclusion_while_a_new_report_is_paired() {
    if super::alone_in(module_path!(), "the_ledger_state_holds_its_conclusion_while_a_new_report_is_paired") {
        return;
    }
    let mut shell = a_home("state");
    land_a_report(&mut shell);
    let _ = apply(&mut shell, Action::ReadLedger);
    settle(&mut shell);
    assert_eq!(shell.ledger_ok, Some(true), "judged from one generation: the deletion is this desk's own");
    land_a_report(&mut shell);
    assert!(!shell.rows_current(), "the table waits for its new reading");
    assert_eq!(shell.ledger_ok, Some(true), "the last conclusion stands meanwhile");
    for place in [LEDGER, Place::Home] {
        let (back, frames) = frames(shell, place, 30);
        shell = back;
        assert!(shell.rows_current() && shell.ledger_ok == Some(true), "{place:?}: the held reading landed and the state was judged again (current {}, ok {:?}, flying {:?}, gen {})", shell.rows_current(), shell.ledger_ok, shell.tasks.flying(), shell.rows_gen);
        for (i, f) in frames.iter().enumerate().skip(SETTLE) {
            assert!(!f.iter().any(|x| x.contains(t(Key::V2Abnormal))), "{place:?} frame {i}: never \"problems\": {f:?}");
            assert!(f.iter().any(|x| x.contains(t(Key::V2Normal))), "{place:?} frame {i}: \"normal\" throughout");
        }
        land_a_report(&mut shell);
    }
}

#[test]
fn a_report_is_judged_on_what_it_says_alone_and_never_against_a_newer_ledger() {
    if super::alone_in(module_path!(), "a_report_is_judged_on_what_it_says_alone_and_never_against_a_newer_ledger") {
        return;
    }
    let mut shell = a_home("alone");
    land_a_report(&mut shell);
    let _ = apply(&mut shell, Action::ReadLedger);
    settle(&mut shell);
    assert_eq!(shell.ledger_ok, Some(true));
    shell.audit_asked = Some(shell.book_mark);
    shell.book_changed();
    shell.count_here();
    land_a_report(&mut shell);
    assert_ne!(shell.audit.as_ref().map(|a| a.mark), Some(shell.book_mark));
    shell.ledger_ok = None;
    let _ = apply(&mut shell, Action::ReadLedger);
    settle(&mut shell);
    assert!(shell.rows_current(), "the table of the report's generation landed");
    assert_eq!(shell.ledger_ok, None, "an older ledger's report is not counted against the newer table");
    shell.audit_asked = None;
    land_a_report_naming(&mut shell, zikaron::v2::tokens::Key::Missing);
    assert!(!shell.rows_current(), "its table not read yet");
    assert_eq!(shell.ledger_ok, Some(false), "judged on the report alone");
}

#[test]
fn a_ledger_not_moved_has_its_root_and_takes_no_new_entry() {
    if super::alone_in(module_path!(), "a_ledger_not_moved_has_its_root_and_takes_no_new_entry") {
        return;
    }
    use zikaron_glue::book::Law;
    super::vault_open();
    let ctx = egui::Context::default();
    let mut shell = Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.reread_vault();
    let base = std::env::temp_dir().join(format!("zk-refresh-old-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let root = base.join("home");
    super::seated(&mut shell, &root);
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: root.display().to_string() }), Applied::Homed { .. }));
    settle(&mut shell);
    let secret = app::key::load(app::register::account_now().ok().flatten().as_deref()).expect("the vault").expect("a key");
    let ledger = shell.home.as_ref().expect("a home").ledger().expect("its ledger");
    let st = |x: &str| zikaron::json::Value::Str(x.to_string());
    let g = app::entryx::seal_under(Law::One, &secret, "genesis", 0, None, zikaron::json::Value::Obj(vec![("statement_md".into(), st("old"))])).expect("sealed");
    ledger.append(&zikaron_store::EntryName::parse(g.id.trim_start_matches("0x")).expect("a name"), &g.bytes).expect("laid");
    let mode = zikaron::json::Value::Obj(vec![("mark".into(), st("v1")), ("toolchain".into(), st(&format!("0x{}", "0b".repeat(32))))]);
    let h = app::entryx::seal_under(Law::One, &secret, "history", 1, Some(&g.id), zikaron::json::Value::Obj(vec![("content".into(), st(&format!("0x{}", "0a".repeat(32)))), ("mode".into(), mode)])).expect("sealed");
    ledger.append(&zikaron_store::EntryName::parse(h.id.trim_start_matches("0x")).expect("a name"), &h.bytes).expect("laid");
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: root.display().to_string() }), Applied::Homed { .. }));
    settle(&mut shell);
    assert!(shell.rooted, "an old ledger has its root");
    let before = ledger.pile().expect("the pile").items.len();
    let got = apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "after".into() });
    settle(&mut shell);
    let f = match got {
        Applied::Trouble(f) => f,
        other => panic!("not refused: {other:?}"),
    };
    assert_eq!(f.which(), Some(app::fault::Known::NotMoved), "{} · {}", f.said(), f.tail());
    assert_eq!(ledger.pile().expect("the pile").items.len(), before, "not one entry landed");
}

#[test]
fn an_upgrade_step_queues_without_the_send_card() {
    if super::alone_in(module_path!(), "an_upgrade_step_queues_without_the_send_card") {
        return;
    }
    use zikaron_glue::book::Law;
    super::vault_open();
    let ctx = egui::Context::default();
    let mut shell = Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.reread_vault();
    let base = std::env::temp_dir().join(format!("zk-upgrade-step-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let root = base.join("home");
    super::seated(&mut shell, &root);
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: root.display().to_string() }), Applied::Homed { .. }));
    settle(&mut shell);
    let secret = app::key::load(app::register::account_now().ok().flatten().as_deref()).expect("the vault").expect("a key");
    let ledger = shell.home.as_ref().expect("a home").ledger().expect("its ledger");
    let st = |x: &str| zikaron::json::Value::Str(x.to_string());
    let g = app::entryx::seal_under(Law::One, &secret, "genesis", 0, None, zikaron::json::Value::Obj(vec![("statement_md".into(), st("old"))])).expect("sealed");
    ledger.append(&zikaron_store::EntryName::parse(g.id.trim_start_matches("0x")).expect("a name"), &g.bytes).expect("laid");
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: root.display().to_string() }), Applied::Homed { .. }));
    settle(&mut shell);
    shell
        .commit_settings(|s| {
            s.chain_id = Some(31337);
            s.registry = Some(app::key::Address([0x11; 20]));
            s.endpoints = vec!["31337=http://127.0.0.1:9".into()];
            s.auto_anchor = true;
        })
        .expect("the settings");
    shell.endpoints = app::chainx::nodes_of(&shell.settings);
    shell.upgrade = Some(app::upgradex::Stage::Anchor { owed: 1 });
    let (shell, up) = app::window::probe_upgrade_step(shell);
    assert!(!shell.queue.items.is_empty(), "the step queued what it owes: {:?}", shell.faults);
    assert!(!up, "and no send card came up");
}
