use super::{seated, vault_open};
use app::action::{apply, apply_settled, Action, Applied};
use app::fault::Known;
use app::lang::{t, Key};
use std::path::{Path, PathBuf};

fn base(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("zk-r30b4-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("a scratch folder");
    d
}

fn boot() -> app::shell::Shell {
    let ctx = zikaron_ui::egui::Context::default();
    app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx))
}

fn author(at: &Path) -> app::shell::Shell {
    vault_open();
    let mut shell = boot();
    seated(&mut shell, at);
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: at.display().to_string() }), Applied::Homed { .. }));
    assert!(matches!(apply_settled(&mut shell, Action::Genesis { statement: "r30b4".into() }), Applied::Genesised { .. }));
    shell.seat_identities(app::register::view().ok());
    shell
}

fn ledger_landed(shell: &mut app::shell::Shell) {
    let _ = apply(shell, Action::ReadLedger);
    let from = std::time::Instant::now();
    while shell.tasks.in_flight(app::task::Kind::Ledger) {
        assert!(from.elapsed() < std::time::Duration::from_secs(60), "the ledger read never landed");
        let _ = shell.drain();
        std::thread::yield_now();
    }
    assert!(shell.rows_current(), "the table is read at this ledger");
}

fn window_shell(ctx: &zikaron_ui::egui::Context) -> app::shell::Shell {
    vault_open();
    app::shell::Shell::boot(zikaron_ui::skin::dress(ctx))
}

#[test]
fn a_vault_change_is_settled_by_the_try_started_or_not() {
    if super::alone_in(module_path!(), "a_vault_change_is_settled_by_the_try_started_or_not") {
        return;
    }
    let at = base("settle");
    let mut shell = author(&at.join("home"));
    shell.vault_changed = true;
    assert!(shell.review_due(0.0, 0.0), "a change is due at once");
    let held = shell.home.as_ref().expect("a home").dir(app::home::Slot::GrantsHeld);
    let _ = std::fs::remove_dir_all(&held);
    std::fs::write(&held, b"not a folder").expect("a file in its place");
    let faults = shell.faults.len();
    let first = apply(&mut shell, Action::ReviewVault);
    assert!(matches!(first, Applied::Started(app::task::Kind::Review)), "{first:?}");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while shell.tasks.in_flight(app::task::Kind::Review) && std::time::Instant::now() < until {
        let _ = shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(shell.faults.len(), faults + 1, "said once, where it landed");
    assert!(!shell.review_due(0.0, 0.0), "settled by the try: not tried again on the next turn");
    std::fs::remove_file(&held).expect("the file gone");
    assert!(matches!(apply(&mut shell, Action::ReviewVault), Applied::Started(app::task::Kind::Review)));
    shell.vault_changed = true;
    assert!(matches!(apply(&mut shell, Action::ReviewVault), Applied::Refused(app::task::Kind::Review)), "the pass is out");
    assert!(shell.vault_changed, "kept for the pass after it");
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while shell.tasks.in_flight(app::task::Kind::Review) && std::time::Instant::now() < until {
        let _ = shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(shell.review_due(0.0, 0.0), "due once the pass landed");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_change_walks_only_past_its_start() {
    use app::rotatex::Stage;
    for (s, walking) in [
        (Stage::Behind, false),
        (Stage::Owed { n: 1 }, false),
        (Stage::Bookless { chains: Vec::new() }, false),
        (Stage::Ready, false),
        (Stage::Unread { chain: 7, written: false }, false),
        (Stage::Declare { chain: 7 }, true),
        (Stage::Write, true),
        (Stage::Anchoring { id: String::new() }, true),
        (Stage::Accede { chain: 7 }, true),
        (Stage::Sweep { chain: 7, send: Some(1), then: Some(Box::new(Stage::Anchoring { id: String::new() })) }, true),
        (Stage::Sweep { chain: 7, send: None, then: None }, true),
        (Stage::Done, false),
    ] {
        assert_eq!(s.walking(), walking, "{}", s.as_str());
    }
}

#[test]
fn the_key_change_sheet_comes_back_only_for_a_change_walked_in_this_run() {
    use app::rotatex::Stage;
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = window_shell(&ctx);
    for stage in [Stage::Declare { chain: 7 }, Stage::Write, Stage::Anchoring { id: String::new() }, Stage::Accede { chain: 7 }, Stage::Sweep { chain: 7, send: Some(1), then: None }] {
        let (s, back) = app::window::probe_rotate_return(shell, &stage, false, false);
        assert!(!back, "{}: never walked in this run, stays put", stage.as_str());
        let (s, back) = app::window::probe_rotate_return(s, &stage, true, false);
        assert!(back, "{}: walked in this run, comes back", stage.as_str());
        let (s, back) = app::window::probe_rotate_return(s, &stage, true, true);
        assert!(!back, "{}: put away, stays put", stage.as_str());
        shell = s;
    }
    for stage in [Stage::Unread { chain: 7, written: false }, Stage::Ready, Stage::Owed { n: 1 }, Stage::Behind, Stage::Done] {
        let (s, back) = app::window::probe_rotate_return(shell, &stage, true, false);
        assert!(!back, "{}: not under way", stage.as_str());
        shell = s;
    }
}

#[test]
fn an_identity_without_words_has_no_key_made_here() {
    if super::alone_in(module_path!(), "an_identity_without_words_has_no_key_made_here") {
        return;
    }
    let ctx = zikaron_ui::egui::Context::default();
    let at = base("c1ways");
    let mut shell = window_shell(&ctx);
    seated(&mut shell, &at.join("home"));
    shell.seat_identities(app::register::view().ok());
    let local = t(Key::RotModeLocal).to_string();
    let given = t(Key::RotModeGiven).to_string();
    let new_key = t(Key::RotNewKey).to_string();
    let (shell, texts) = app::window::probe_rotate_ways(&ctx, shell, false, 1180.0, 760.0);
    assert!(!texts.contains(&local), "no key made here: {texts:?}");
    assert!(texts.contains(&new_key), "the given address asked: {texts:?}");
    let (_, texts) = app::window::probe_rotate_ways(&ctx, shell, true, 1180.0, 760.0);
    assert!(texts.contains(&local) && texts.contains(&given), "recovery words: both ways: {texts:?}");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_rail_item_shows_its_views_own_page() {
    if super::alone_in(module_path!(), "a_rail_item_shows_its_views_own_page") {
        return;
    }
    let ctx = zikaron_ui::egui::Context::default();
    let at = base("c4rail");
    let mut shell = author(&at.join("home"));
    let w = at.join("w.txt");
    std::fs::write(&w, b"a record").expect("a file");
    let _ = apply_settled(&mut shell, Action::RecordWork { note_md: "一件".into(), files: vec![w.display().to_string()], for_: None });
    let _ = zikaron_ui::skin::dress(&ctx);
    ledger_landed(&mut shell);
    let (_, walk) = app::window::probe_rail_walk(&ctx, shell, 1180.0, 760.0);
    let mut seen = 0;
    for w in &walk {
        if let (Some(detail), Some(back)) = (&w.detail, &w.from_elsewhere) {
            seen += 1;
            assert_eq!(back, &w.entered, "{}: from another view, its own page", w.item);
            assert_eq!(w.forward.as_ref(), Some(detail), "{}: the page left there is one step forward", w.item);
        }
    }
    assert!(seen > 0, "a view with a detail page was walked: {walk:?}");
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn a_drop_no_zone_took_is_said() {
    let ctx = zikaron_ui::egui::Context::default();
    let shell = window_shell(&ctx);
    let (shell, r) = app::window::probe_drop_untaken(&ctx, shell, false, &["/x/a.txt".to_string()], 1180.0, 760.0);
    let said = app::lang::fill1(Key::DropElsewhere, t(Key::NavWorksView));
    assert_eq!(r.toast.as_deref(), Some(said.as_str()), "{r:?}");
    assert!(r.taken.is_none() && r.sheet.is_none(), "nothing taken: {r:?}");
    let (_, r) = app::window::probe_drop_untaken(&ctx, shell, true, &["/x/a.txt".to_string()], 1180.0, 760.0);
    assert_eq!(r.toast.as_deref(), Some(t(Key::DropCloseSheet)), "over a sheet: {r:?}");
    assert!(r.taken.is_none() && r.sheet.is_none(), "nothing taken: {r:?}");
}

#[test]
fn a_signed_grant_clears_its_form() {
    if super::alone_in(module_path!(), "a_signed_grant_clears_its_form") {
        return;
    }
    let ctx = zikaron_ui::egui::Context::default();
    let at = base("c4grant");
    let mut shell = author(&at.join("home"));
    super::venue::in_venue(&mut shell, &super::venue::venue());
    let w = at.join("w.txt");
    std::fs::write(&w, b"the granted work").expect("a file");
    let _ = apply_settled(&mut shell, Action::RecordWork { note_md: "一件".into(), files: vec![w.display().to_string()], for_: None });
    let work = zikaron::hexfmt::encode(&zikaron_glue::recording::content_of(b"the granted work"));
    let _ = zikaron_ui::skin::dress(&ctx);
    let (_, word, again) = app::window::probe_grant_signed(&ctx, shell, &format!("0x{}", "33".repeat(20)), &work, &format!("0x{}", "44".repeat(32)), 1180.0, 760.0);
    assert_eq!(word, "granted");
    assert!(!again, "the form is cleared: confirm cannot sign it again");
    let _ = std::fs::remove_dir_all(&at);
}

fn set_pen(home: &app::home::Home, pen: zikaron_seal::handover::Pen) {
    let root = app::ledgerx::root_of(home).expect("a root");
    let root: [u8; 32] = zikaron::hexfmt::decode(&root).and_then(|b| <[u8; 32]>::try_from(b.as_slice()).ok()).expect("hex32");
    let machine = app::home::machine_dir().expect("machine");
    let mut pens = zikaron_seal::pens::read(&machine).expect("the list");
    pens.set(&zikaron_seal::pens::key_of(&machine, &root).expect("key"), pen);
    zikaron_seal::pens::write(&machine, &pens).expect("written");
}

#[test]
fn a_handed_off_ledger_is_a_read_only_shell() {
    if super::alone_in(module_path!(), "a_handed_off_ledger_is_a_read_only_shell") {
        return;
    }
    use zikaron_seal::handover::{HandId, Pen};
    let at = base("c18");
    let mut shell = author(&at.join("home"));
    let home = app::home::Home::bare(shell.home.as_ref().expect("a home").root().to_path_buf());
    assert!(shell.writable() && shell.handed_off.is_none());
    set_pen(&home, Pen::Handed { id: HandId([7; 16]), after: None });
    super::look_landed(&mut shell, 1_000.0);
    assert_eq!(shell.handed_off.as_deref(), Some(HandId([7; 16]).hex().as_str()));
    assert!(!shell.pen_here(), "a shell: the pen is elsewhere");
    assert_eq!(shell.banner(), app::shell::Banner::HandedOff);
    assert!(shell.writable() && shell.may_save_settings().is_ok(), "settings still saved");
    match apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "while handed".into() }) {
        Applied::Trouble(f) => assert_eq!(f.which(), Some(Known::HandedOff), "refused by its own name"),
        other => panic!("refused: {other:?}"),
    }
    let root = home.root().display().to_string();
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: root.clone() }), Applied::Homed { .. }));
    assert!(shell.handed_off.is_some(), "read with the home");
    let ctx = zikaron_ui::egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let (s, r) = app::window::probe_face(&ctx, shell, app::nav::Place::Home, 1180.0, 760.0);
    assert!(r.texts.iter().any(|x| x == t(Key::PenAwayBar)), "the strip says so: {:?}", r.texts);
    let (mut shell, r) = app::window::probe_face(&ctx, s, app::nav::Place::Settings(app::nav::Section::Data), 1180.0, 760.0);
    assert!(r.texts.iter().any(|x| x.ends_with(t(Key::SetPenReadOnly))), "the header asks the same pen: {:?}", r.texts);
    set_pen(&home, Pen::Holds { took: Some(HandId([8; 16])) });
    super::look_landed(&mut shell, 2_000.0);
    assert!(shell.handed_off.is_none() && shell.pen_here());
    assert!(matches!(apply(&mut shell, Action::Annotate { subject: String::new(), note_md: "back".into() }), Applied::Annotated(_)));
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn the_local_data_header_asks_the_pen_every_write_asks() {
    if super::alone_in(module_path!(), "the_local_data_header_asks_the_pen_every_write_asks") {
        return;
    }
    let at = base("c227");
    let shell = author(&at.join("home"));
    let ctx = zikaron_ui::egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let data = app::nav::Place::Settings(app::nav::Section::Data);
    let (mut shell, r) = app::window::probe_face(&ctx, shell, data, 1180.0, 760.0);
    assert!(r.texts.iter().any(|x| x.ends_with(t(Key::SetPenOpen))), "{:?}", r.texts);
    shell.handed = Some(format!("0x{}", "22".repeat(20)));
    assert!(!shell.pen_here());
    let (_, r) = app::window::probe_face(&ctx, shell, data, 1180.0, 760.0);
    assert!(r.texts.iter().any(|x| x.ends_with(t(Key::SetPenReadOnly))), "{:?}", r.texts);
    assert!(!r.texts.iter().any(|x| x.ends_with(t(Key::SetPenOpen))), "{:?}", r.texts);
    let _ = std::fs::remove_dir_all(&at);
}

#[test]
fn an_imported_identity_is_unchecked_from_the_import_on() {
    if super::alone_in(module_path!(), "an_imported_identity_is_unchecked_from_the_import_on") {
        return;
    }
    vault_open();
    let mut shell = boot();
    let words = app::action::ImportForm::Words("legal winner thank year wave sausage worth useful legal winner thank yellow".to_string().into());
    let got = apply_settled(&mut shell, Action::ImportIdentity { form: words, seat: app::roles::Role::Author, label: "c19".into(), network: app::deploy::CUSTOM.into() });
    assert!(!matches!(got, Applied::Trouble(_)), "{got:?}");
    let _ = shell.drain();
    assert_eq!(shell.unfetched, Some(app::restorex::State::Unfetched), "marked from the import on");
    let home = shell.home.as_ref().expect("the seat's home");
    assert_eq!(app::restorex::read(home).ok().flatten(), Some(app::restorex::State::Unfetched), "and on the disk");
    let refused = apply(&mut shell, Action::Genesis { statement: "c19".into() });
    assert!(matches!(&refused, Applied::Trouble(f) if f.which() == Some(Known::LedgerNotFetched)), "{refused:?}");
    let ctx = zikaron_ui::egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    assert_eq!(shell.banner(), app::shell::Banner::Unfetched(app::restorex::State::Unfetched));
    let (shell, r) = app::window::probe_face(&ctx, shell, app::nav::Place::Home, 1180.0, 760.0);
    assert!(r.texts.iter().any(|x| x == t(Key::FetchLedgerTitle)), "the strip on every page: {:?}", r.texts);
    let (_, r) = app::window::probe_face(&ctx, shell, app::nav::Place::Settings(app::nav::Section::Data), 1180.0, 760.0);
    assert_eq!(r.texts.iter().filter(|x| *x == t(Key::FetchLedgerTitle)).count(), 1, "the frame, once, where it is fetched: {:?}", r.texts);
}

#[test]
fn creating_the_ledger_can_wait_and_the_upgrade_waits_for_a_chain() {
    use app::nav::Step;
    use app::roles::Role;
    for seat in [Role::Author, Role::Grantee] {
        assert!(Step::Genesis.deferrable(seat), "{seat:?}");
        assert!(!Step::Network.deferrable(seat) && !Step::Key.deferrable(seat) && !Step::Pin.deferrable(seat));
    }
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = window_shell(&ctx);
    shell.upgrade = Some(app::upgradex::Stage::Anchor { owed: 1 });
    let (mut shell, up) = app::window::probe_upgrade_comes_up(&ctx, shell, 1180.0, 760.0);
    assert!(!up, "no chain set: the sheet waits");
    shell.settings.chain_id = Some(31337);
    shell.settings.registry = Some(app::key::Address([0x11; 20]));
    shell.endpoints = app::chainx::nodes_of(&{
        let mut s = shell.settings.clone();
        s.endpoints = vec!["31337=http://127.0.0.1:9".into()];
        s
    });
    assert!(shell.chain_set());
    let anchor = Some(app::upgradex::Stage::Anchor { owed: 1 });
    let (_, up) = app::window::probe_upgrade_comes_up_steps(&ctx, shell, &[(anchor.clone(), false, true), (anchor, true, true), (Some(app::upgradex::Stage::Done), true, true)], 1180.0, 760.0);
    assert_eq!(up, [false, true, false], "a chain set: unread it waits, read and not done it comes up, done it stays down");
}

#[test]
fn the_gas_step_says_why_there_is_no_code() {
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = window_shell(&ctx);
    shell.anchor = Some(app::key::Address([0x42; 20]));
    shell.settings.chain_id = None;
    let say = t(Key::WizGasNoCode).to_string();
    let (mut shell, texts, _) = app::window::probe_wizard_step(&ctx, shell, app::nav::Place::Home, app::nav::Step::Gas, 1180.0, 760.0);
    assert!(texts.iter().any(|d| d.text == say), "said: {:?}", texts.iter().map(|d| &d.text).collect::<Vec<_>>());
    shell.settings.chain_id = Some(11155111);
    let (_, texts, _) = app::window::probe_wizard_step(&ctx, shell, app::nav::Place::Home, app::nav::Step::Gas, 1180.0, 760.0);
    assert!(!texts.iter().any(|d| d.text == say), "a code: no line");
}

#[test]
fn a_dropped_file_list_reads_into_local_paths() {
    use app::platform::drops::paths_of_uri_list;
    let list = b"# a comment\r\nfile:///srv/u/%E5%8E%9F%E4%BB%B6%20one.txt\r\nfile://localhost/srv/two\r\nfile:///tmp/three?x=1\r\n\r\n";
    assert_eq!(paths_of_uri_list(list), vec![PathBuf::from("/srv/u/原件 one.txt"), PathBuf::from("/srv/two"), PathBuf::from("/tmp/three")]);
    let bad = b"file://elsewhere/srv/u/a\nhttps://example.com/b\nfile:///srv/u/c%4\nfile:///srv/u/d%zz\nfile:///srv/u/e%00f\nfile:relative\nfile:///srv/u/kept\n";
    assert_eq!(paths_of_uri_list(bad), vec![PathBuf::from("/srv/u/kept")], "the bad lines left out, the good one kept");
    assert!(paths_of_uri_list(b"").is_empty());
}

#[test]
fn a_drag_hovers_only_over_this_window_and_a_drop_lands_once() {
    use app::platform::drops::{Drops, Watcher};
    let shared = std::sync::Arc::new(std::sync::Mutex::new(Drops::default()));
    let w = Watcher::over(shared.clone());
    let feed = |f: &dyn Fn(&mut Drops)| f(&mut shared.lock().expect("the state"));
    feed(&|d| d.entered(false, true));
    assert!(!w.hovering(), "another surface");
    feed(&|d| d.entered(true, false));
    assert!(!w.hovering(), "not a list of files");
    feed(&|d| d.entered(true, true));
    assert!(w.hovering());
    feed(&|d| d.left());
    assert!(!w.hovering(), "dragged away");
    feed(&|d| d.entered(true, true));
    feed(&|d| d.delivered(b"file:///srv/u/a.txt\r\nfile:///srv/u/b\r\n"));
    assert!(!w.hovering(), "dropped: no longer hovering");
    assert_eq!(w.take(), vec![PathBuf::from("/srv/u/a.txt"), PathBuf::from("/srv/u/b")]);
    assert!(w.take().is_empty(), "once");
}

#[test]
fn the_window_feeds_the_pieces_drag_into_the_frame() {
    let a = PathBuf::from("/x/a");
    let b = PathBuf::from("/x/b");
    assert_eq!(app::window::probe_feed_drops(false, Vec::new(), vec![a.clone()]), (0, vec![a.clone()]), "nothing from the piece: as given");
    assert_eq!(app::window::probe_feed_drops(true, Vec::new(), Vec::new()), (1, Vec::new()));
    assert_eq!(app::window::probe_feed_drops(false, vec![b.clone()], vec![a.clone()]), (0, vec![a, b]));
}

#[test]
fn a_folder_has_the_dialog_too() {
    use app::platform::Pick;
    let ctx = zikaron_ui::egui::Context::default();
    let shell = window_shell(&ctx);
    let (shell, r) = app::window::probe_pick_which(&ctx, shell, false, false, 1180.0, 760.0);
    assert!(!r.asked && r.opened == vec![Pick::FileOrFolder], "where nothing is asked first: the dialog at once: {r:?}");
    let (shell, r) = app::window::probe_pick_which(&ctx, shell, true, true, 1180.0, 760.0);
    assert!(r.asked && r.texts.iter().any(|x| x == t(Key::PickWhich)), "{r:?}");
    assert_eq!(r.opened, vec![Pick::Folder], "the folder key: a folder dialog");
    let (_, r) = app::window::probe_pick_which(&ctx, shell, true, false, 1180.0, 760.0);
    assert_eq!(r.opened, vec![Pick::File], "the file key: a file dialog");
}

#[test]
fn only_linux_asks_file_or_folder_first() {
    assert_eq!(app::platform::asks_file_or_folder(), cfg!(target_os = "linux"));
}

#[test]
fn the_piece_receives_drops_only_where_the_windowing_layer_does_not() {
    use app::platform::{backend_of, drops, Backend};
    use drops::Devices::{Every, NewestOnly};
    let plain = ["wl_compositor", "wl_seat", "wl_data_device_manager", "xdg_wm_base"];
    assert_eq!(drops::devices_of_globals(plain), Every, "no gtk_shell1: every device has the drags");
    assert_eq!(drops::devices_of_globals(plain.into_iter().chain(["gtk_shell1"])), NewestOnly, "gtk_shell1: the newest alone");
    assert!(drops::binds_beside_clipboard(Every));
    assert!(!drops::binds_beside_clipboard(NewestOnly), "beside the clipboard's, one of the two would go without");
    let (x, empty) = (Some(std::ffi::OsStr::new(":0")), Some(std::ffi::OsStr::new("")));
    for display in [x, empty, None] {
        assert_eq!(backend_of(Some(Every), display), Backend::Default, "every device: a Wayland window, whatever X display ({display:?})");
        assert_eq!(backend_of(None, display), Backend::Default, "no compositor read: as the windowing layer picks ({display:?})");
    }
    assert_eq!(backend_of(Some(NewestOnly), x), Backend::X11, "the newest alone with an X display: X11");
    assert_eq!(backend_of(Some(NewestOnly), empty), Backend::Default, "an empty one names none");
    assert_eq!(backend_of(Some(NewestOnly), None), Backend::Default, "no X display: a Wayland window, the piece binding none");
    let mut cell = 0u8;
    let p = (&mut cell as *mut u8).cast::<std::ffi::c_void>();
    assert!(drops::taken_here(drops::Window::Wayland { display: p, surface: p }), "a Wayland window: the piece");
    assert!(!drops::taken_here(drops::Window::Other), "an X11 window (or any other): the windowing layer's own");
    assert!(!drops::taken_here(drops::Window::Wayland { display: std::ptr::null_mut(), surface: p }), "no display given: nothing");
    let w = drops::watch(drops::Window::Other, std::sync::Arc::new(|| ()));
    assert!(w.is_none(), "nothing started for an X11 window");
    drops::Watcher::over(std::sync::Arc::new(std::sync::Mutex::new(drops::Drops::default()))).stop();
    #[cfg(not(target_os = "linux"))]
    assert_eq!(app::platform::window_backend(), Backend::Default, "macOS and Windows: as the windowing layer picks");
}

#[test]
fn the_window_stops_the_piece_once_as_it_closes() {
    use app::platform::drops;
    let n = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let m = n.clone();
    let ctx = zikaron_ui::egui::Context::default();
    let shell = window_shell(&ctx);
    let w = drops::Watcher::stopped_by(std::sync::Arc::new(std::sync::Mutex::new(drops::Drops::default())), std::sync::Arc::new(move || {
        m.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }));
    let (_, gone) = app::window::probe_close_with_drops(shell, w);
    assert!(gone, "the window no longer holds the piece");
    assert_eq!(n.load(std::sync::atomic::Ordering::SeqCst), 1);
}

use app::restorex::{forget_on, moved, passed_on, read_on, On, State as MarkState, READ_ON};

const A: On = On { chain: 31337, registry: [0xa1; 20] };
const B: On = On { chain: 31337, registry: [0xb2; 20] };
const OTHER_CHAIN: On = On { chain: 1, registry: [0xa1; 20] };

#[test]
fn the_chain_a_mark_came_off_on_holds_for_that_chain_alone() {
    let home = super::labelled_home("r36a-on");
    assert!(!moved(&home, Some(A)), "no record: not moved");
    passed_on(&home, A).expect("recorded");
    assert_eq!(read_on(&home).expect("reads"), Some(A));
    assert!(!moved(&home, Some(A)), "the same chain and registry");
    assert!(moved(&home, Some(B)), "another registry on that chain");
    assert!(moved(&home, Some(OTHER_CHAIN)), "another chain");
    assert!(moved(&home, None), "no chain set is not that chain");
    assert_eq!(app::local::home_doc(&format!("{}/{READ_ON}", app::home::Slot::Settings.as_str())), Some(app::local::Doc::Unfetched));
    let at = home.dir(app::home::Slot::Settings).join(READ_ON);
    std::fs::write(&at, b"not sealed").expect("spoil it");
    assert!(read_on(&home).is_err(), "unreadable is an error, never absent");
    assert!(moved(&home, Some(A)), "unreadable counts as moved");
    assert!(passed_on(&home, A).is_err(), "never written over while it does not read");
    assert_eq!(std::fs::read(&at).expect("still there"), b"not sealed");
    forget_on(&home).expect("removed");
    forget_on(&home).expect("removing none is no error");
    assert_eq!(read_on(&home).expect("reads"), None);
    assert!(!moved(&home, Some(B)));
    let _ = std::fs::remove_dir_all(home.root());
}

#[test]
fn a_home_without_genesis_takes_its_mark_back_when_its_chain_moves() {
    if super::alone_in(module_path!(), "a_home_without_genesis_takes_its_mark_back_when_its_chain_moves") {
        return;
    }
    vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut s = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let dir = std::env::temp_dir().join(format!("zk-r36a-moved-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    seated(&mut s, &dir);
    let open = |s: &mut app::shell::Shell| assert!(matches!(apply(s, Action::OpenHome { root: dir.display().to_string() }), Applied::Homed { .. }));
    open(&mut s);
    let set = |s: &mut app::shell::Shell, on: On| {
        s.commit_settings(|x| {
            x.chain_id = Some(on.chain);
            x.registry = Some(app::key::Address(on.registry));
            x.endpoints = vec![format!("{}=http://127.0.0.1:9", on.chain)];
        })
        .expect("saved")
    };
    set(&mut s, A);
    s.endpoints = app::chainx::nodes_of(&s.settings);
    assert_eq!(s.unfetched, None, "no record: a fresh home keeps writing");
    assert_eq!(s.set_up(), Some(app::shell::SetUp::Create));
    assert!(s.take_unwritten_due(), "asked once before creating");
    assert!(!s.take_unwritten_due(), "once a home a process");
    let home = app::home::Home::bare(s.home.as_ref().expect("open").root().to_path_buf());
    passed_on(&home, A).expect("recorded");
    s.commit_settings(|x| x.from_block = 5).expect("saved");
    assert_eq!(s.unfetched, None, "the same chain and registry");
    set(&mut s, B);
    assert_eq!(s.unfetched, Some(MarkState::Unfetched), "taken back in the same save");
    assert_eq!(app::restorex::read(&home).expect("reads"), Some(MarkState::Unfetched), "on disk too");
    assert_eq!(s.set_up(), Some(app::shell::SetUp::Fetch), "the way to fetch, no ledger to create");
    let refused = apply(&mut s, Action::Genesis { statement: "fork".into() });
    assert!(matches!(&refused, Applied::Trouble(f) if f.which() == Some(app::fault::Known::LedgerNotFetched)), "{refused:?}");
    let (mut s, drawn, _) = app::window::probe_ledger_set_up(&ctx, s, 1180.0, 760.0);
    assert!(!drawn, "the ledger page draws no \"create the ledger\" under the mark");
    app::restorex::clear(&home).expect("cleared");
    s.unfetched = None;
    set(&mut s, A);
    s.endpoints = app::chainx::nodes_of(&s.settings);
    passed_on(&home, A).expect("recorded");
    s.commit_settings(|x| {
        x.chain_id = None;
        x.registry = None;
        x.endpoints.clear();
    })
    .expect("cleared");
    s.endpoints = app::chainx::nodes_of(&s.settings);
    assert_eq!(s.unfetched, Some(MarkState::Unfetched), "no chain is not the chain it passed on");
    let refused = apply(&mut s, Action::Genesis { statement: "offline fork".into() });
    assert!(matches!(&refused, Applied::Trouble(f) if f.which() == Some(app::fault::Known::LedgerNotFetched)), "{refused:?}");
    set(&mut s, A);
    s.endpoints = app::chainx::nodes_of(&s.settings);
    s.tail_asked = None;
    assert!(s.tail_due(), "back on A: the tail is asked again before the mark comes off");
    set(&mut s, B);
    app::restorex::clear(&home).expect("cleared");
    s.unfetched = None;
    open(&mut s);
    assert_eq!(s.unfetched, Some(MarkState::Unfetched), "taken back on opening");
    app::restorex::clear(&home).expect("cleared");
    passed_on(&home, B).expect("recorded");
    open(&mut s);
    assert_eq!(s.unfetched, None);
    assert!(matches!(apply(&mut s, Action::Genesis { statement: "now one".into() }), Applied::Genesised { .. }));
    assert_eq!(read_on(&home).expect("reads"), None, "removed with the genesis");
    passed_on(&home, A).expect("a stale record");
    open(&mut s);
    assert_eq!(s.unfetched, None, "a home with a genesis never takes the mark back");
    assert_eq!(read_on(&home).expect("reads"), None, "removed on opening");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_mark_comes_off_only_on_the_chain_the_home_names_then() {
    if super::alone_in(module_path!(), "the_mark_comes_off_only_on_the_chain_the_home_names_then") {
        return;
    }
    vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut s = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let dir = std::env::temp_dir().join(format!("zk-r36b-race-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    seated(&mut s, &dir);
    assert!(matches!(apply(&mut s, Action::OpenHome { root: dir.display().to_string() }), Applied::Homed { .. }));
    let set = |s: &mut app::shell::Shell, on: On| {
        s.commit_settings(|x| {
            x.chain_id = Some(on.chain);
            x.registry = Some(app::key::Address(on.registry));
            x.endpoints = vec![format!("{}=http://127.0.0.1:9", on.chain)];
        })
        .expect("saved")
    };
    set(&mut s, A);
    s.endpoints = app::chainx::nodes_of(&s.settings);
    let root = s.home.as_ref().expect("open").root().to_path_buf();
    let home = app::home::Home::bare(root.clone());
    app::restorex::mark(&home).expect("marked, as an import marks it");
    s.unfetched = Some(MarkState::Unfetched);
    s.tail_asked = Some(s.book_mark);
    s.leave(app::shell::Keep::Tail { root: root.clone(), tail: app::restorex::Tail::Pass { anchors: 0 }, open: Some(root.clone()), on: Some(A) });
    set(&mut s, B);
    s.endpoints = app::chainx::nodes_of(&s.settings);
    s.keep_now();
    assert_eq!(app::restorex::read(&home).expect("reads"), Some(MarkState::Unfetched), "the mark stays on the disk");
    assert_eq!(read_on(&home).expect("reads"), None, "A is not recorded");
    assert_eq!(s.unfetched, Some(MarkState::Unfetched), "the mark stays in the shell");
    assert!(s.tail_due(), "the tail is asked again, on B");
    let refused = apply(&mut s, Action::Genesis { statement: "fork".into() });
    assert!(matches!(&refused, Applied::Trouble(f) if f.which() == Some(app::fault::Known::LedgerNotFetched)), "{refused:?}");
    s.tail_asked = Some(s.book_mark);
    s.leave(app::shell::Keep::Tail { root: root.clone(), tail: app::restorex::Tail::Pass { anchors: 0 }, open: Some(root.clone()), on: Some(B) });
    s.keep_now();
    assert_eq!(app::restorex::read(&home).expect("reads"), None, "off on B");
    assert_eq!(read_on(&home).expect("reads"), Some(B));
    assert_eq!(s.unfetched, None);
    let _ = std::fs::remove_dir_all(&dir);
}
