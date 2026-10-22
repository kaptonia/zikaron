//! The address half of a node item is read everywhere by one reader (`rpc::read_address`), as its spelling is
//! by the one spelling reader. The app's node cell (`SetEndpoints`) and the check page refuse an item whose
//! address does not read, as the command line does; an item already on disk is read by its spelling alone and
//! kept (named where it is used), never silently dropped. Places are set before any vault or shell use
//! (`vault_open`); each test runs alone in its own process.

use super::vault_open;
use app::action::{apply, Action, Applied};

/// The address forms, each with whether the address reads (`rpc::read_address`).
const FORMS: [(&str, bool); 11] = [
    ("1==https://a", false),
    ("1=wss://a.example", false),
    ("1=a.example", false),
    ("1=https://a.example:65536", false),
    ("1=https://a.example:", false),
    ("1=https://[zz]:1", false),
    ("1=https://u:p@a.example", false),
    ("1=https://a.example", true),
    ("1=HTTP://a.example:8545/x", true),
    ("1=https://[::1]:8545", true),
    ("1=http://127.0.0.1:8545", true),
];

#[test]
fn the_node_cell_reads_an_address_as_the_command_line_does() {
    if super::alone_in(module_path!(), "the_node_cell_reads_an_address_as_the_command_line_does") {
        return;
    }
    vault_open();
    let base = std::env::temp_dir().join(format!("zk-b13-cell-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    answers!(apply(&mut shell, Action::OpenHome { root: base.join("home").display().to_string() }), Applied::Homed { .. });
    for (spec, reads) in FORMS {
        let one = zikaron_anchor::rpc::endpoint_spec(spec).map(|(_, a)| zikaron_anchor::rpc::read_address(&a).is_ok());
        assert_eq!(one, Ok(reads), "the one reader on {spec:?}");
        match apply(&mut shell, Action::SetEndpoints { specs: spec.to_string() }) {
            Applied::Trouble(f) => {
                assert!(!reads, "{spec:?} refused: {f:?}");
                assert_eq!(f.which(), Some(app::fault::Known::SettingsShape), "{spec:?}");
            }
            _ => {
                assert!(reads, "{spec:?} taken");
                assert_eq!(shell.endpoints.len(), 1, "{spec:?}");
            }
        }
        let typed = app::chainx::Endpoint::typed(spec);
        assert_eq!(typed.is_ok(), reads, "{spec:?}");
    }
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// The check page's typed nodes are read the same way: an item whose address does not read is refused by name
/// before anything is asked.
#[test]
fn the_check_page_reads_an_address_as_the_command_line_does() {
    if super::alone_in(module_path!(), "the_check_page_reads_an_address_as_the_command_line_does") {
        return;
    }
    vault_open();
    let base = std::env::temp_dir().join(format!("zk-b13-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    answers!(apply(&mut shell, Action::OpenHome { root: base.join("home").display().to_string() }), Applied::Homed { .. });
    let check = |shell: &mut app::shell::Shell, endpoints: &str| {
        apply(
            shell,
            Action::CheckPayload {
                typed: "zikaron-grant:not-a-code".into(),
                ledgers: String::new(),
                endpoints: endpoints.into(),
                registry: String::new(),
                from_block: String::new(),
                now: String::new(),
                file: String::new(),
                terms: String::new(),
            },
        )
    };
    match check(&mut shell, "1==https://a") {
        Applied::Trouble(f) => assert_eq!(f.which(), Some(app::fault::Known::SettingsShape), "{f:?}"),
        other => panic!("taken: {other:?}"),
    }
    assert!(!matches!(check(&mut shell, "1=https://a.example"), Applied::Trouble(ref f) if f.which() == Some(app::fault::Known::SettingsShape)));
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// An item already on disk (saved by an older version that did not read addresses) is read by its spelling
/// alone and kept, never silently dropped; where it is used, its address is named (`SETTINGS_SHAPE`).
#[test]
fn an_item_on_disk_whose_address_does_not_read_is_kept() {
    assert!(app::chainx::Endpoint::parse("1==https://a").is_some());
    assert!(app::chainx::Endpoint::typed("1==https://a").is_err());
}

/// More kernel locks than this process may hold at once (`zikaron_os::LOCKS_MAX`): taking a home's writer lock
/// at the app is refused by name (the limit said), never taken as a reader or as nothing; once one lock is let
/// go, the same home is taken as its writer. Runs alone in its own process: it fills the process's lock list.
#[test]
fn a_lock_past_the_process_limit_is_refused_by_name_at_the_app() {
    if super::alone_in(module_path!(), "a_lock_past_the_process_limit_is_refused_by_name_at_the_app") {
        return;
    }
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-b13-limit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir.join("home")).expect("a home");
    std::fs::create_dir_all(dir.join("fill")).expect("fill");
    let mut o = zikaron_os::Options::new();
    o.create(true).read(true).write(true);
    let mut fills: Vec<zikaron_os::LockFile> = (0..).map_while(|i| zikaron_os::LockFile::open(&o, &dir.join("fill").join(format!("{i}"))).ok()).collect();
    assert!(!fills.is_empty() && fills.len() <= zikaron_os::LOCKS_MAX, "the list filled ({} more)", fills.len());
    let refused = app::lock::take(&home).err().expect("refused while the list is full");
    let said = format!("{} {}", refused.said(), refused.tail());
    assert!(said.contains(&format!("more than {} held at once", zikaron_os::LOCKS_MAX)), "{said}");
    drop(fills.pop());
    assert_eq!(app::lock::take(&home).expect("taken once one is let go").mode(), app::lock::Mode::Writer);
    drop(fills);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A home's writer lock released while children are being spawned on other threads (the system-proxy reading
/// spawns one): the next take of the same home is a writer every time, never a reader blocked by a child's
/// inherited copy of the lock. Children start through `zikaron_os::spawn`, as the product's do; the test runs
/// alone in its own process so no other test spawns any.
#[cfg(unix)]
#[test]
fn a_lock_let_go_while_children_start_is_taken_again_as_a_writer() {
    if super::alone_in(module_path!(), "a_lock_let_go_while_children_start_is_taken_again_as_a_writer") {
        return;
    }
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-b13-lock-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("a home");
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let begun = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let starters: Vec<_> = (0..4)
        .map(|_| {
            let (stop, begun) = (stop.clone(), begun.clone());
            std::thread::spawn(move || {
                let mut n = 0usize;
                while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                    if let Ok(mut c) = zikaron_os::spawn(std::process::Command::new("/usr/bin/true").stdin(std::process::Stdio::null())) {
                        begun.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        let _ = c.wait();
                        n += 1;
                    }
                }
                n
            })
        })
        .collect();
    // The takes start once children are starting (the starters' first child), however busy the machine is.
    let from = std::time::Instant::now();
    while begun.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        assert!(from.elapsed() < std::time::Duration::from_secs(20), "no child started");
        std::thread::yield_now();
    }
    let mut readers = 0usize;
    let mut modes: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for _ in 0..3000 {
        let l = app::lock::take(&home).expect("taken");
        if l.mode() != app::lock::Mode::Writer {
            readers += 1;
            *modes.entry(format!("{:?}", l.mode())).or_default() += 1;
        }
        drop(l);
    }
    stop.store(true, std::sync::atomic::Ordering::SeqCst);
    let started: usize = starters.into_iter().map(|h| h.join().unwrap_or(0)).sum();
    assert!(started > 0, "children were started meanwhile");
    assert_eq!(readers, 0, "takes read as a reader while {started} children started: {modes:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
