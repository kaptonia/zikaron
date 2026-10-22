//! A refresh keeps what is shown until the new reading is in hand: the ledger table stays drawn while it is read
//! again ("not read" belongs only to a table never read), and the ledger's state keeps its last conclusion while
//! a new report waits for the table read for its lamps (it is judged only from one generation, never from a new
//! report and an old table or the other way round). Read frame by frame over the window's own pages.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use app::action::{apply, Action, Applied};
use app::lang::{fill1, t, Key};
use app::nav::{Place, View};
use app::shell::Shell;
use app::task::{Done, Kind};
use zikaron_ui::egui;

type Frame = Vec<String>;

/// Every frame's texts, as the window drew them (egui's output hook on the probe's own context); after `hold`
/// frames it lets the held table read go.
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

/// A home with a first entry, a record and that record deleted here (an entry type the core does not know, which
/// this desk recognizes as its own deletion), its table read.
fn a_home(tag: &str) -> Shell {
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
    assert!(matches!(apply(&mut shell, Action::Genesis { statement: "refresh".into() }), Applied::Genesised { .. }));
    settle(&mut shell);
    let file = base.join("one.txt");
    std::fs::write(&file, b"refresh").expect("a file");
    let _ = apply(&mut shell, Action::TakeContent { source: app::anchorx::Source::File, path: file.display().to_string() });
    settle(&mut shell);
    let record = match apply(&mut shell, Action::RecordWork { note_md: "refresh".into(), files: Vec::new(), for_: None }) {
        Applied::Recorded { id, .. } => id,
        other => panic!("record: {other:?}"),
    };
    settle(&mut shell);
    let gone = apply(&mut shell, Action::Retract { subject: record, note_md: "gone".into() });
    assert!(!matches!(gone, Applied::Trouble(_)), "{gone:?}");
    settle(&mut shell);
    let _ = apply(&mut shell, Action::ReadLedger);
    settle(&mut shell);
    assert!(shell.rows_current(), "the table read");
    shell
}

/// A report of this ledger with nothing wrong but the one deletion the core does not know (`UNKNOWN_TYPE`, which
/// this desk counts as its own), landed as an audit pass lands.
fn land_a_report(shell: &mut Shell) {
    land_a_report_naming(shell, zikaron::tokens::Key::UnknownType);
}

/// As [`land_a_report`], the one item it names being `item`.
fn land_a_report_naming(shell: &mut Shell, item: zikaron::tokens::Key) {
    use zikaron::json::Value;
    let entries = shell.rows.as_ref().map(|(r, _)| r.len()).unwrap_or(0);
    let report = Value::Obj(vec![(item.as_str().to_string(), Value::Arr(vec![Value::Str("deletion".into())]))]);
    let done = Done::Audited {
        label: zikaron::tokens::Label::Complete.as_str().to_string(),
        complete: true,
        broken: false,
        entries,
        report,
        unanswered: Vec::new(),
        asked: 1,
        single_source: true,
        fragment: app::auditx::empty_fragment(),
    };
    assert_eq!(shell.tasks.spawn(Kind::Audit, move || Ok(done)), app::task::Spawned::Started);
    settle(shell);
}

/// The probe settles this many frames (0.1 s apart) before its own steps: by then the page's entrance is over.
const SETTLE: usize = 5;
/// Frames the table's new reading is held back, from the first.
const HELD: usize = 20;

/// Draw `place` frame by frame from now with the table's new reading in flight and held back for the first
/// `HELD` frames (it is the current table read again, at the current generation, so it lands as any read does),
/// then let go; each frame's texts and the shell.
fn frames(mut shell: Shell, place: Place, n: usize) -> (Shell, Vec<Frame>) {
    let release = Arc::new(AtomicBool::new(false));
    let (rows, strays) = shell.rows.clone().expect("a table held");
    let (handed, r#gen, go) = (shell.handed.clone(), shell.rows_gen, release.clone());
    let started = shell.tasks.spawn(Kind::Ledger, move || {
        while !go.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        Ok(Done::Ledger { rows, strays, handed, r#gen })
    });
    assert_eq!(started, app::task::Spawned::Started);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    ctx.add_plugin(Capture { frames: seen.clone(), hold: HELD, release });
    let steps = (0..n).map(|_| (0.1, Vec::new())).collect();
    let (mut shell, _) = app::window::probe_shell_input(&ctx, shell, place, steps, 1180.0, 760.0);
    settle(&mut shell);
    // The window's own clock may have started another read meanwhile (it looks at the disk), which leaves the
    // held one behind; what is current is read once more, as the next frame would.
    if !shell.rows_current() {
        let _ = apply(&mut shell, Action::ReadLedger);
        settle(&mut shell);
    }
    let frames = seen.lock().unwrap().clone();
    (shell, frames)
}

const LEDGER: Place = Place::View(View::Log, 0);

/// The table stays drawn while it is read again: every frame from the refresh on shows the entry count, and the
/// new reading lands.
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

/// The ledger's state holds its conclusion while a new report waits for its table: no frame says "problems",
/// though the report alone (with no table of its generation) would count this desk's own deletion as unknown.
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
    // A second pass lands: the table goes stale, and the state is judged again once its table is read.
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

/// A problem the report names by itself is judged the moment the report lands, without waiting for its table;
/// a report about an older ledger (an entry written while it ran) is not paired with a table of the newer one.
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
    // Asked at this mark; the ledger moves on before the report lands.
    shell.audit_asked = Some(shell.book_mark);
    shell.book_changed();
    land_a_report(&mut shell);
    assert_ne!(shell.audit.as_ref().map(|a| a.mark), Some(shell.book_mark));
    shell.ledger_ok = None;
    let _ = apply(&mut shell, Action::ReadLedger);
    settle(&mut shell);
    assert!(shell.rows_current(), "the table of the report's generation landed");
    assert_eq!(shell.ledger_ok, None, "an older ledger's report is not counted against the newer table");
    // A report naming a missing entry: a problem before its table is read.
    shell.audit_asked = None;
    land_a_report_naming(&mut shell, zikaron::tokens::Key::Missing);
    assert!(!shell.rows_current(), "its table not read yet");
    assert_eq!(shell.ledger_ok, Some(false), "judged on the report alone");
}
