//! A segment picked on a view's own page (the verify page's three, the records page's two) keeps the page: its
//! entrance does not play again and its scroll stays, as every other segment; the place, the shell's page and the
//! history follow the segment as a tab would. Read through the window's own probe, headless, each test alone in its
//! own process ([`super::alone_in`]).

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

/// The verify page, scrolled a little, its "verify records" segment pressed: the same entrance, the same scroll, the
/// route on that tab with no history. The window is low enough that both segments scroll further than that (a scroll
/// past what the new segment holds is brought back to its end by the scroll area itself, as under every segment). Victim: whoever switches the verify page's segments and sees the whole page
/// play in again, back at its top.
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

/// The records page, its "not on chain" segment pressed: the same entrance, and the shell's page is the queue's (what
/// reads the queue for that tab still runs). Victim: whoever opens the queue's tab and is shown a page that never read
/// the queue.
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
