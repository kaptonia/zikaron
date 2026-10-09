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
    let starters: Vec<_> = (0..4)
        .map(|_| {
            let stop = stop.clone();
            std::thread::spawn(move || {
                let mut n = 0usize;
                while !stop.load(std::sync::atomic::Ordering::SeqCst) {
                    if let Ok(mut c) = zikaron_os::spawn(std::process::Command::new("/usr/bin/true").stdin(std::process::Stdio::null())) {
                        let _ = c.wait();
                        n += 1;
                    }
                }
                n
            })
        })
        .collect();
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
