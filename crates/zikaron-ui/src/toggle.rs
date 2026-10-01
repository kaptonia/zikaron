//! The switch: 34 × 20, grey off and blue on; the knob springs across in 200 ms. Every true or false setting
//! uses it (there is no check box anywhere).

use crate::motion::{self, Curve};
use crate::palette::{self, c, C};
use crate::tokens::{self, Type};
use egui::{pos2, vec2, Rect};

/// Paint a switch in `rect` (34 × 20) for state `on`.
pub fn paint_switch(ui: &egui::Ui, rect: Rect, id: egui::Id, on: bool, enabled: bool) {
    let ctx = ui.ctx();
    let ground = motion::flag(ctx, id.with("ground"), on, tokens::FAST);
    let knob = motion::to(ctx, id.with("knob"), if on { 1.0 } else { 0.0 }, tokens::MID, Curve::Spring);
    let alpha = if enabled { 1.0 } else { tokens::OFF };
    let p = ui.painter();
    let fill = palette::mix(c(C::Segment), c(C::Accent), ground).gamma_multiply(alpha);
    p.rect(rect, egui::CornerRadius::same(10), fill, egui::Stroke::new(0.5_f32, egui::Color32::from_black_alpha(15).gamma_multiply(1.0 - ground)), egui::StrokeKind::Inside);
    let x = rect.left() + 10.0 + 14.0 * knob;
    let at = pos2(x, rect.center().y);
    p.add(egui::Shadow { offset: [0, 1], blur: 2, spread: 0, color: egui::Color32::from_black_alpha(64).gamma_multiply(alpha) }.as_shape(Rect::from_center_size(at, vec2(16.0, 16.0)), egui::CornerRadius::same(8)));
    p.circle_filled(at, 8.0, c(C::Knob).gamma_multiply(alpha.max(0.8)));
}

/// A bare switch. Returns its response; a click means flip.
pub fn switch(ui: &mut egui::Ui, on: bool, enabled: bool) -> egui::Response {
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(tokens::SWITCH_W, tokens::SWITCH_H), sense);
    paint_switch(ui, rect, resp.id, on, enabled);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// A setting row: title and a quiet note on the left, the switch on the right. Clicking the switch or the
/// title flips `on`; returns whether it flipped this frame.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool, label: &str, note: &str, enabled: bool) -> bool {
    let mut flipped = false;
    let w = ui.available_width();
    ui.allocate_ui_with_layout(vec2(w, 0.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        let resp = switch(ui, *on, enabled);
        let words = ui
            .with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
                let l = ui.add(egui::Label::new(egui::RichText::new(label).font(Type::Body.font()).color(c(C::Ink))).sense(sense));
                if !note.is_empty() {
                    crate::paint::text(ui, note, Type::Small, c(C::Ink3));
                }
                l
            })
            .inner;
        if enabled && (resp.clicked() || words.clicked()) {
            *on = !*on;
            flipped = true;
        }
    });
    flipped
}
