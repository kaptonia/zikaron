use app::action::{apply, Action, Applied};
use app::nav::{tab as T, Place, Step, View};
use zikaron_ui::egui;

const W: f32 = 1180.0;
const H: f32 = 760.0;

fn shell(ctx: &egui::Context) -> app::shell::Shell {
    super::vault_open();
    app::shell::Shell::boot(zikaron_ui::skin::dress(ctx))
}

fn shell_with_ledger(ctx: &egui::Context, name: &str) -> app::shell::Shell {
    let mut s = shell(ctx);
    let dir = std::env::temp_dir().join(format!("zk-habits-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    super::seated(&mut s, &dir);
    answers!(apply(&mut s, Action::OpenHome { root: dir.display().to_string() }), Applied::Homed { .. });
    answers!(apply(&mut s, Action::Genesis { statement: name.into() }), Applied::Genesised { .. });
    s
}

#[test]
fn back_from_the_words_check_returns_to_the_same_words() {
    if super::alone_in(module_path!(), "back_from_the_words_check_returns_to_the_same_words") {
        return;
    }
    let ctx = egui::Context::default();
    let mut s = shell(&ctx);
    answers!(apply(&mut s, Action::NewIdentity), Applied::FreshWords);
    let (_, r) = app::window::probe_wizard_back(&ctx, s, Place::Home, W, H);
    assert!(r.pressed_on_check && r.pressed_on_words, "back drawn on both pages: {r:?}");
    assert_eq!((r.step_after_check.as_str(), r.checking_after, r.same_words), ("Key", false, true), "{r:?}");
    assert_eq!(r.step_after_words, "Pin", "{r:?}");
}

#[test]
fn the_language_switches_on_the_wizard_and_the_gate_keeps_it() {
    if super::alone_in(module_path!(), "the_language_switches_on_the_wizard_and_the_gate_keeps_it") {
        return;
    }
    super::vault_open();
    let m = app::home::machine_dir().expect("the machine directory");
    let _ = std::fs::remove_file(m.join(app::places::keybox_file()));
    app::keybox::lock();
    let ctx = egui::Context::default();
    let s = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let (mut s, wizard) = app::window::probe_lang_pick(&ctx, s, Place::Home, false, Step::Pin, W, H);
    assert!(wizard.drawn && wizard.after != wizard.before && wizard.machine_after == wizard.after && wizard.still_up, "{wizard:?}");
    answers!(app::action::apply_settled(&mut s, Action::SetPin { pin: "27618394".into(), again: "27618394".into() }), Applied::Unlocked);
    apply(&mut s, Action::Lock);
    let (_, gate) = app::window::probe_lang_pick(&ctx, s, Place::Home, true, Step::Pin, W, H);
    assert!(!gate.drawn && gate.after == gate.before && gate.before == wizard.after && gate.still_up, "{gate:?}");
}

#[test]
fn typing_under_an_overlay_never_lands_in_the_passcode_row() {
    if super::alone_in(module_path!(), "typing_under_an_overlay_never_lands_in_the_passcode_row") {
        return;
    }
    let ctx = egui::Context::default();
    super::vault_open();
    app::keybox::lock();
    let s = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let (_, r) = app::window::probe_typed_under(&ctx, s, W, H);
    assert_eq!(r.bare, 3, "with nothing over it the row takes what is typed: {r:?}");
    assert!(r.sheet_up && r.pressed, "the sheet was up and pressed: {r:?}");
    assert_eq!(r.under, 0, "under the sheet the row takes nothing: {r:?}");
    assert!(r.locked, "{r:?}");
}

#[test]
fn the_passcode_cells_sit_in_the_middle_of_the_gates_group() {
    if super::alone_in(module_path!(), "the_passcode_cells_sit_in_the_middle_of_the_gates_group") {
        return;
    }
    let ctx = egui::Context::default();
    super::vault_open();
    app::keybox::lock();
    for lang in app::lang::Lang::ALL {
        app::lang::set(lang);
        let (_, g) = app::window::probe_gate_column(&ctx, app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx)), None, W, H);
        let above = g.cells.top() - g.title_ink_bottom;
        let below = g.forgot.top() - g.cells.bottom();
        assert!(above > 10.0 && (above - below).abs() < 0.5, "{lang:?}: the key stands as far under the cells as the title's strokes stand over them: {above} and {below} ({g:?})");
        let (top, bottom) = (g.group_top, g.window_h - g.forgot.bottom());
        assert!((top - bottom).abs() < 1.5, "{lang:?}: the group is centred in the window: {top} over, {bottom} under");
        let (_, g) = app::window::probe_gate_column(&ctx, app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx)), Some("refused"), W, H);
        let said = g.said.expect("the refusal is said in its box");
        let above = g.cells.top() - g.title_ink_bottom;
        assert!((said.top() - g.cells.bottom() - above).abs() < 0.5 && (g.forgot.top() - said.bottom() - above).abs() < 0.5, "{lang:?}: {above}, then {} and {} ({g:?})", said.top() - g.cells.bottom(), g.forgot.top() - said.bottom());
    }
    app::lang::set(app::lang::Lang::Zh);
}

#[test]
fn the_busy_words_under_the_passcode_cells_are_centred_under_them() {
    if super::alone_in(module_path!(), "the_busy_words_under_the_passcode_cells_are_centred_under_them") {
        return;
    }
    let ctx = egui::Context::default();
    super::vault_open();
    app::keybox::lock();
    let lead = 4.25 * 2.0 + 6.0;
    for lang in app::lang::Lang::ALL {
        app::lang::set(lang);
        for wizard in [false, true] {
            let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
            let (hold, wait) = std::sync::mpsc::channel::<()>();
            let _ = shell.tasks.spawn(app::task::Kind::Vault, move || {
                let _ = wait.recv();
                Err(app::fault::Fault::known(app::fault::Known::FileMissing, ""))
            });
            let (_, cells, busy) = app::window::probe_busy_under_cells(&ctx, shell, wizard, W, H);
            drop(hold);
            let busy = busy.unwrap_or_else(|| panic!("{lang:?}, wizard {wizard}: the busy words are drawn"));
            assert!(cells.is_positive(), "{lang:?}, wizard {wizard}: the passcode cells are drawn");
            let middle = (busy.left() - lead + busy.right()) / 2.0;
            assert!((middle - cells.center().x).abs() < 1.0, "{lang:?}, wizard {wizard}: the busy line's middle {middle} is the cells' middle {}", cells.center().x);
            assert!(busy.top() > cells.bottom(), "{lang:?}, wizard {wizard}: the busy words are under the cells");
        }
    }
    app::lang::set(app::lang::Lang::Zh);
}

#[test]
fn a_chain_read_the_status_line_reports_grows_no_card() {
    if super::alone_in(module_path!(), "a_chain_read_the_status_line_reports_grows_no_card") {
        return;
    }
    let ctx = egui::Context::default();
    super::vault_open();
    let s = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let (_, before, during, reported) = app::window::probe_network_reading(&ctx, s, W, H);
    assert!(reported, "the rail's status line says the read is in progress");
    assert!(before.is_some() && before == during, "what is under the network's card stays where it was while the read runs: {before:?} then {during:?}");
}

#[test]
fn a_page_set_from_outside_the_window_is_shown_next() {
    if super::alone_in(module_path!(), "a_page_set_from_outside_the_window_is_shown_next") {
        return;
    }
    let ctx = egui::Context::default();
    let s = shell_with_ledger(&ctx, "show");
    let (_, before, after, page) = app::window::probe_show_follow(&ctx, s, Place::Home, app::shell::Page::Ledger, W, H);
    assert_ne!(before, after, "the window moved");
    assert_eq!(page, "Ledger", "and left the shell's page where it was set");
}

#[test]
fn pages_with_drop_zones_say_so_and_the_window_stands_aside() {
    if super::alone_in(module_path!(), "pages_with_drop_zones_say_so_and_the_window_stands_aside") {
        return;
    }
    use app::shell::Page;
    let ctx = egui::Context::default();
    let mut s = shell_with_ledger(&ctx, "drops");
    s.settings.role = app::roles::Role::Author;
    let file = std::env::temp_dir().join(format!("zk-habits-drop-{}.txt", std::process::id()));
    std::fs::write(&file, b"one").expect("a file");
    let one = file.display().to_string();
    for (place, takes) in [
        (Place::Page(Page::Kit), true),
        (Place::Page(Page::Grant), true),
        (Place::View(View::Verify, T::VERIFY_CHECK), true),
        (Place::View(View::Log, 0), false),
    ] {
        let (back, r) = app::window::probe_drop(&ctx, s, place, std::slice::from_ref(&one), W, H);
        s = back;
        assert_eq!(r.page_drops, takes, "{place:?} declares: {r:?}");
        assert_eq!(r.sheet.is_some(), !takes, "{place:?}: the whole-window drop {}: {r:?}", if takes { "stands aside" } else { "takes it" });
    }
    let _ = std::fs::remove_file(&file);
}

#[test]
fn each_word_names_one_judgement() {
    use app::lang::{Key, TABLE};
    let row = |k: Key| TABLE.iter().find(|(x, _, _)| *x == k).map(|(_, zh, en)| (*zh, *en)).expect("a row");
    assert_eq!(row(Key::LedgerUnconfirmed), ("未确认", "Unconfirmed"));
    assert_eq!(row(Key::ItemUnanchored), ("待上链", "Pending"));
    assert_eq!(row(Key::U3Work), ("记录", "Entry"));
    let ledger = super::code_only(&super::read_src_file("window/ledger.rs").expect("ledger.rs"));
    let at = ledger.find("\"ledger-filter\"").expect("the ledger's filter");
    let cells = &ledger[ledger[..at].rfind("let cells").expect("its cells")..at];
    assert!(cells.contains("Key::LedgerUnconfirmed") && !cells.contains("Key::ItemUnanchored"), "{cells}");
}

#[test]
fn the_ui_kit_files_name_no_page_of_this_product() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = std::fs::read_to_string(root.join("family-components.json")).expect("the component registry");
    let at = registry.find("\"id\": \"ui-kit\"").expect("the ui-kit component");
    let home = &registry[at..];
    let home = &home[home.find("\"home\"").expect("its home")..];
    let home = &home[..home.find(']').expect("its home's end")];
    let files: Vec<&str> = home.split('"').filter(|x| x.starts_with("crates/app/src/") && x.ends_with(".rs")).collect();
    assert!(files.contains(&"crates/app/src/window/chrome.rs") && files.contains(&"crates/app/src/window/mod.rs"), "{files:?}");
    const MEETS: [&str; 2] = ["crates/app/src/window/skeleton.rs", "crates/app/src/window/drive.rs"];
    for m in MEETS {
        assert!(files.contains(&m), "{m} is in the kit's home: {files:?}");
    }
    for f in files.into_iter().filter(|f| !MEETS.contains(f)) {
        let code = super::code_only(&std::fs::read_to_string(root.join(f)).expect(f));
        for name in ["Route::", "Place::", "View::", "Section::", "Stack::", "Page::", "tab::", "Step::", "nav::rail(", "nav::history_of", "nav::stack_of"] {
            let named = code.match_indices(name).any(|(i, _)| !code[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_'));
            assert!(!named, "{f} names `{name}`");
        }
    }
}

fn closing_node() -> String {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            drop(s);
        }
    });
    url
}

fn block_node(at: u64) -> String {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            std::thread::spawn(move || {
                let mut s = s;
                let mut raw = Vec::new();
                let mut buf = [0u8; 4096];
                let req = loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => raw.extend_from_slice(&buf[..n]),
                    }
                    let text = String::from_utf8_lossy(&raw).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        let len: usize = text[..i].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|n| n.trim().parse().ok())).unwrap_or(0);
                        if raw.len() >= i + 4 + len {
                            break text[i + 4..].to_string();
                        }
                    }
                };
                let id = req.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                let body = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{{\"number\":\"0x10\",\"timestamp\":\"0x{at:x}\"}}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    url
}

#[test]
fn a_new_grant_starts_from_the_chain_time_read_as_it_opens() {
    if super::alone_in(module_path!(), "a_new_grant_starts_from_the_chain_time_read_as_it_opens") {
        return;
    }
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let ctx = egui::Context::default();
    app::lang::set(app::lang::Lang::Zh);
    let (heard, fresh) = (1_790_000_000_u64, 1_790_000_000_u64 + 5 * 86_400);
    let staged = |url: &str| {
        let mut s = shell(&ctx);
        s.settings.chain_id = Some(31337);
        s.endpoints = vec![app::chainx::Endpoint::at(31337, url)];
        s.note_chain_time(heard);
        s
    };
    let stale = app::lang::filln(app::lang::Key::GrantChainTimeStale, &[&app::when::day(heard)]);
    let (_, from, texts) = app::window::probe_grant_start(&ctx, staged(&block_node(fresh)), 150, W, H);
    assert_eq!(from, fresh.to_string(), "the start is the latest block's time");
    assert!(!texts.iter().any(|t| *t == stale), "no stale note when the read landed: {texts:?}");
    let (_, from, texts) = app::window::probe_grant_start(&ctx, staged(&closing_node()), 150, W, H);
    assert_eq!(from, heard.to_string(), "the start falls back to the time heard last");
    assert!(texts.iter().any(|t| *t == stale), "the fallback is said: {stale:?} among {texts:?}");
}
