use app::action::{apply, Action, Applied};
use app::lang::{t, Key};
use zikaron_ui::egui;

const W: f32 = 1180.0;
const H: f32 = 900.0;

fn shell_with_ledger(ctx: &egui::Context, name: &str) -> app::shell::Shell {
    super::vault_open();
    let mut s = app::shell::Shell::boot(zikaron_ui::skin::dress(ctx));
    let dir = std::env::temp_dir().join(format!("zk-r34b7-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    super::seated(&mut s, &dir);
    answers!(apply(&mut s, Action::OpenHome { root: dir.display().to_string() }), Applied::Homed { .. });
    answers!(apply(&mut s, Action::Genesis { statement: name.into() }), Applied::Genesised { .. });
    s
}

#[test]
fn a_refusal_a_sheet_shows_is_not_toasted_again() {
    if super::alone_in(module_path!(), "a_refusal_a_sheet_shows_is_not_toasted_again") {
        return;
    }
    let ctx = egui::Context::default();
    let s = shell_with_ledger(&ctx, "sheet-says");
    let (_, drawn, toast) = app::window::probe_sheet_refusal(&ctx, s, false, W, H);
    assert!(drawn.contains(&t(Key::IdRenameTitle).to_string()), "the sheet stays: {drawn:?}");
    assert_eq!(toast, None, "said on the sheet, not toasted as well: {drawn:?}");
}

#[test]
fn an_empty_note_pressed_is_the_field_framed_red_and_no_toast() {
    if super::alone_in(module_path!(), "an_empty_note_pressed_is_the_field_framed_red_and_no_toast") {
        return;
    }
    let ctx = egui::Context::default();
    let s = shell_with_ledger(&ctx, "note-empty");
    let (_, drawn, toast) = app::window::probe_sheet_refusal(&ctx, s, true, W, H);
    assert!(drawn.contains(&t(Key::U3AnnotateTitle).to_string()), "the sheet stays: {drawn:?}");
    assert!(!drawn.iter().any(|d| d.contains(t(Key::Tail019))), "no words for an empty field: {drawn:?}");
    assert_eq!(toast, None, "no toast: {drawn:?}");
}
