//! A card made of rows says where it is and where each of its rows is (`probe::boxed`, `probe::part`), so a
//! reading can tell a control laid in a row from one laid loose in the card.

use zikaron_ui::egui::{self, pos2, Pos2, Rect};
use zikaron_ui::{button, card, probe};

/// Draw one card: two rows with a key each, and (when `loose`) a key laid straight in the card between them. The
/// controls that lie in the card but in none of its rows.
fn strays(loose: bool) -> Vec<Rect> {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let mut out = Vec::new();
    for t in [1.0, 1.1] {
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1000.0, 700.0))), time: Some(t), ..Default::default() };
        let _ = ctx.run_ui(input, |ui| {
            probe::begin(ui.ctx());
            ui.scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(40.0, 40.0), egui::vec2(600.0, 600.0))), |ui| {
                card::form(ui, |ui, f| {
                    let _ = f.act(ui, "一行", "", button::Key::new("键", button::Role::Secondary));
                    if loose {
                        let _ = button::key(ui, "散键", button::Role::Secondary, true);
                    }
                    let _ = f.act(ui, "二行", "", button::Key::new("键二", button::Role::Secondary));
                });
            });
        });
        let (parts, boxes) = (probe::parts(&ctx), probe::boxes(&ctx));
        out = ctx.viewport(|v| {
            v.prev_pass
                .widgets
                .layers()
                .flat_map(|(layer, ws)| ws.iter().filter(|w| w.sense.senses_click()).map(move |w| (*layer, w.rect)))
                .filter(|(l, r)| boxes.iter().any(|(bl, b)| bl == l && b.expand(0.5).contains_rect(*r)))
                .filter(|(l, r)| !parts.iter().any(|(pl, p)| pl == l && p.expand(0.5).contains_rect(*r)))
                .map(|(_, r)| r)
                .collect()
        });
    }
    out
}

#[test]
fn a_key_in_a_row_is_in_its_row_and_a_loose_key_is_told() {
    assert!(strays(false).is_empty(), "the row's key lies in its row");
    assert_eq!(strays(true).len(), 1, "the loose key is told");
}
