use app::action::{apply, Action, Applied};
use app::lang::{t, Key};
use app::nav::{tab as T, Place, View};
use zikaron_ui::egui;

fn shell_with_ledger(ctx: &egui::Context, name: &str) -> app::shell::Shell {
    super::vault_open();
    let mut s = app::shell::Shell::boot(zikaron_ui::skin::dress(ctx));
    let dir = std::env::temp_dir().join(format!("zk-segments-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    super::seated(&mut s, &dir);
    answers!(apply(&mut s, Action::OpenHome { root: dir.display().to_string() }), Applied::Homed { .. });
    answers!(apply(&mut s, Action::Genesis { statement: name.into() }), Applied::Genesised { .. });
    s
}

#[test]
fn the_verify_pages_segment_keeps_the_page() {
    if super::alone_in(module_path!(), "the_verify_pages_segment_keeps_the_page") {
        return;
    }
    let ctx = egui::Context::default();
    let s = shell_with_ledger(&ctx, "verify");
    let (_, r) = app::window::probe_seg_tab(&ctx, s, Place::View(View::Verify, T::VERIFY_CHECK), t(Key::V2TabVerifyWork), 24.0, 1180.0, 300.0);
    assert!(r.pressed, "{r:?}");
    assert_eq!(r.entry_after, r.entry_before, "the page does not enter again: {r:?}");
    assert!(r.scroll_before > 0.0 && r.scroll_after == r.scroll_before, "the scroll stays: {r:?}");
    assert_eq!((r.route.as_str(), r.history_empty), ("Verify.1", true), "{r:?}");
}

#[test]
fn the_records_pages_segment_keeps_the_page_and_shows_the_queue() {
    if super::alone_in(module_path!(), "the_records_pages_segment_keeps_the_page_and_shows_the_queue") {
        return;
    }
    let ctx = egui::Context::default();
    let s = shell_with_ledger(&ctx, "works");
    let (_, r) = app::window::probe_seg_tab(&ctx, s, Place::View(View::Works, T::WORKS_ALL), t(Key::ItemUnanchored), 0.0, 1180.0, 760.0);
    assert!(r.pressed, "{r:?}");
    assert_eq!(r.entry_after, r.entry_before, "the page does not enter again: {r:?}");
    assert_eq!((r.route.as_str(), r.page.as_str(), r.history_empty), ("Works.1", "Queue", true), "{r:?}");
}
