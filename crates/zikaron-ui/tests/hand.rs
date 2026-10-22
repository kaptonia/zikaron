//! The pointing hand (`zikaron_ui::hand`): over whatever takes a click, decided once after the frame; never over
//! what takes a click only to swallow it, and never in place of a cursor the frame chose (a text field's beam).

use egui::{CursorIcon, Pos2, Rect, Sense, Vec2};

const SCREEN: Vec2 = Vec2::new(800.0, 600.0);

fn dressed() -> egui::Context {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    ctx
}

/// Two frames of `add` with the pointer at `at` (the first lays out, the second is hovered); the cursor the second
/// one leaves, after the hand is decided.
fn cursor(ctx: &egui::Context, at: Pos2, mut add: impl FnMut(&mut egui::Ui)) -> CursorIcon {
    let mut icon = CursorIcon::Default;
    for i in 0..3 {
        let t = ctx.input(|x| x.time) + 0.1;
        let events = if i == 0 { Vec::new() } else { vec![egui::Event::PointerMoved(at)] };
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(t), events, ..Default::default() };
        let out = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show_inside(root, |ui| add(ui));
            zikaron_ui::hand::finish(root.ctx());
        });
        icon = out.platform_output.cursor_icon;
    }
    icon
}

#[test]
fn whatever_takes_a_click_shows_the_hand_unless_it_only_swallows_it() {
    let ctx = dressed();
    let at = Pos2::new(60.0, 40.0);
    let spot = Rect::from_center_size(at, Vec2::splat(40.0));
    assert_eq!(cursor(&ctx, at, |ui| { let _ = ui.interact(spot, ui.id().with("plain"), Sense::click()); }), CursorIcon::PointingHand, "a bare click area, no cursor asked for");
    assert_eq!(
        cursor(&ctx, at, |ui| {
            let r = ui.interact(spot, ui.id().with("sink"), Sense::click());
            zikaron_ui::hand::sink(ui.ctx(), r.id);
        }),
        CursorIcon::Default,
        "a click sink keeps the arrow"
    );
    assert_eq!(cursor(&ctx, at, |ui| { let _ = ui.interact(spot, ui.id().with("hover"), Sense::hover()); }), CursorIcon::Default, "what takes no click keeps the arrow");
    assert_eq!(cursor(&ctx, at, |ui| { let _ = ui.interact(spot, ui.id().with("drag"), Sense::click_and_drag()); }), CursorIcon::Default, "a drag grip is not given the hand");
}

#[test]
fn the_details_copy_icon_shows_the_hand_and_its_label_does_not() {
    use zikaron_ui::card::{self, Detail};
    let ctx = dressed();
    let rows = [("Address", Detail::Mono("0x1234567890abcdef1234567890abcdef12345678".to_string()))];
    let draw = |ui: &mut egui::Ui| {
        card::form(ui, |ui, f| {
            let _ = f.detail_rows(ui, "d", &rows, 80.0);
        });
    };
    // The row's middle line, from a first frame (the copy key sits at its right end).
    let mut row = Rect::NOTHING;
    let _ = cursor(&ctx, Pos2::ZERO, |ui| {
        let top = ui.cursor().min;
        draw(ui);
        row = Rect::from_min_max(top, ui.min_rect().max);
    });
    let y = row.top() + 22.0;
    let hands: Vec<f32> = (0..40).map(|k| row.right() - 4.0 - k as f32).filter(|x| cursor(&ctx, Pos2::new(*x, y), draw) == CursorIcon::PointingHand).collect();
    assert!(!hands.is_empty(), "the copy icon at the row's right end shows the hand");
    assert_eq!(cursor(&ctx, Pos2::new(row.left() + 40.0, y), draw), CursorIcon::Default, "the label is not pressed");
}

#[test]
fn a_cursor_the_frame_chose_is_kept() {
    let ctx = dressed();
    let at = Pos2::new(60.0, 40.0);
    let spot = Rect::from_center_size(at, Vec2::splat(40.0));
    let got = cursor(&ctx, at, |ui| {
        let r = ui.interact(spot, ui.id().with("typed"), Sense::click());
        if r.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Text);
        }
    });
    assert_eq!(got, CursorIcon::Text, "a field's beam stands");
}

/// Only a machine value carries a copy key: an address, a digest, an id, a path (and several of them, one a line, one key);
/// words, figures (a count, a date), and an empty value carry none (`card::Detail`'s kinds).
#[test]
fn only_machine_values_carry_a_copy_key() {
    use zikaron_ui::card::{self, Detail};
    let rows = [
        ("Address", Detail::Mono("0x1234567890abcdef1234567890abcdef12345678".to_string())),
        ("Nodes", Detail::Mono("https://a.example\nhttps://b.example".to_string())),
        ("Entries", Detail::Figure("128".to_string())),
        ("When", Detail::Figure("2026-10-20 06:52:55".to_string())),
        ("State", Detail::Text("Confirmed".to_string())),
        ("Said", Detail::Text("one\ntwo".to_string())),
        ("Seat", Detail::Quiet("Not set".to_string())),
    ];
    let mut tops = Vec::new();
    let draw = |ui: &mut egui::Ui| {
        card::form(ui, |ui, f| {
            let _ = f.detail_rows(ui, "d", &rows, 80.0);
        });
    };
    // Each row's middle line, from a first frame: the rows are 44 high, two-line ones 60.
    let ctx = dressed();
    let mut right = 0.0;
    let _ = cursor(&ctx, Pos2::ZERO, |ui| {
        let top = ui.cursor().min.y;
        draw(ui);
        right = ui.min_rect().right();
        let mut y = top;
        for (_, v) in &rows {
            let n = match v {
                Detail::Mono(s) | Detail::Figure(s) | Detail::Text(s) | Detail::Quiet(s) => s.split('\n').count(),
            };
            let h = if n > 1 { (n as f32 * 20.0 + 20.0).max(44.0) } else { 44.0 };
            tops.push(y + if n > 1 { 10.0 + 10.0 } else { 22.0 });
            y += h;
        }
    });
    // The helper draws three frames: the first one's rows.
    tops.truncate(rows.len());
    let copies: Vec<bool> = tops.iter().map(|y| (0..40).any(|k| cursor(&ctx, Pos2::new(right - 4.0 - k as f32, *y), draw) == CursorIcon::PointingHand)).collect();
    assert_eq!(copies, vec![true, true, false, false, false, false, false], "a copy key on the machine values only");
}
