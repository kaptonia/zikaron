//! The window's habits read through its own probes, headless (each test alone in its own process,
//! [`super::alone_in`], on that process's machine directory):
//!
//! - "Back" in the first-run wizard goes from the words' check to the same words, then to the step before.
//! - The language switch on the first-run wizard and on the passcode gate changes the language spoken and this
//!   machine's choice, the wizard (the gate) staying up.
//! - Typing at the passcode gate lands in its row with nothing over it, and never with the restore sheet over it.
//! - The shell's page set from outside the window is the page the window shows next.
//! - Pages with drop zones of their own (a record kit, a new grant, checking a grant; a relicense is the new
//!   grant's form and opens only with an upstream grant in hand) declare it as they draw and the whole-window
//!   drop stands aside; a page without one leaves the drop to the window.
//! - The window kit's files (the shell, the window's state and its own helpers) name no page, place, view,
//!   history or wizard step of this product: they reach them only through this product's table
//!   (`window/pages.rs`, `crate::nav`).

use app::action::{apply, Action, Applied};
use app::nav::{tab as T, Place, Step, View};
use zikaron_ui::egui;

const W: f32 = 1180.0;
const H: f32 = 760.0;

fn shell(ctx: &egui::Context) -> app::shell::Shell {
    super::vault_open();
    app::shell::Shell::boot(zikaron_ui::skin::dress(ctx))
}

/// A shell with an identity and its home holding a genesis.
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
    // The true first run: no vault yet (the passcode is the wizard's first step).
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
    // Once the master key is made the gate has no switch, and speaks the language chosen last.
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

/// The two words and their judgements: the ledger's filter says "unconfirmed" (the chain has not confirmed it)
/// and the records page's tab says "pending" (the queue); English names one record "Entry" (the author's rail
/// group over records and grants too, singular as the user's "Grant" group is). Read from the one table, and
/// the ledger's filter read from its source.
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

/// The window's own files carry no page of this product: no file the family's component registry lists under
/// the window kit (`ui-kit`'s home in `family-components.json`, read as it stands) writes a route, place, view,
/// settings section, history, old page, tab or wizard step of this product. The shell reaches them only through
/// this product's table (`window/pages.rs`, `crate::nav`); the product's drivers and probes live in
/// `window/probes.rs`. The kit crate (`zikaron-ui`) cannot name them at all (it does not depend on the app).
#[test]
fn the_ui_kit_files_name_no_page_of_this_product() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let registry = std::fs::read_to_string(root.join("family-components.json")).expect("the component registry");
    let at = registry.find("\"id\": \"ui-kit\"").expect("the ui-kit component");
    let home = &registry[at..];
    let home = &home[home.find("\"home\"").expect("its home")..];
    let home = &home[..home.find(']').expect("its home's end")];
    let files: Vec<&str> = home.split('"').filter(|x| x.starts_with("crates/app/") && x.ends_with(".rs")).collect();
    assert!(files.contains(&"crates/app/src/window/chrome.rs") && files.contains(&"crates/app/src/window/mod.rs"), "{files:?}");
    for f in files {
        let code = super::code_only(&std::fs::read_to_string(root.join(f)).expect(f));
        for name in ["Route::", "Place::", "View::", "Section::", "Stack::", "Page::", "tab::", "Step::", "nav::rail(", "nav::history_of", "nav::stack_of"] {
            let named = code.match_indices(name).any(|(i, _)| !code[..i].ends_with(|c: char| c.is_alphanumeric() || c == '_'));
            assert!(!named, "{f} names `{name}`");
        }
    }
}
