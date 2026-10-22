//! The switch: 34 × 20, a grey tone off and blue on. Flipped, the knob first stretches toward where it goes (16 to 22
//! wide, 0.22 s), then crosses (0.34 s from the flip) and settles round there, without bouncing; under reduced
//! motion it is simply there. Every true or false setting uses it (there is no check box anywhere).

use crate::motion::{self, Curve};
use crate::palette::{self, c, C};
use crate::tokens::{self, Type};
use egui::{pos2, vec2, Rect};

/// Paint a switch in `rect` (34 × 20) for state `on`.
/// How long the knob takes to cross, and to stretch before it settles.
pub const CROSS: f32 = 0.34 * crate::tokens::PACE;
pub const STRETCH: f32 = 0.22 * crate::tokens::PACE;

/// When the switch under `id` last flipped and which way it stands.
#[derive(Clone, Copy)]
struct Flip {
    on: bool,
    at: f64,
}

pub fn paint_switch(ui: &egui::Ui, rect: Rect, id: egui::Id, on: bool, enabled: bool) {
    let ctx = ui.ctx();
    let now = crate::motion::now(ui.ctx());
    let ground = motion::flag(ctx, id.with("ground"), on, tokens::FAST);
    let knob = motion::glide(ctx, id.with("knob"), if on { 1.0 } else { 0.0 }, CROSS, Curve::Ease);
    // The stretch: up to 22 by STRETCH after the flip, back to round as the knob arrives.
    let fid = id.with("flip");
    let flip = ctx.data(|d| d.get_temp::<Flip>(fid));
    let at = match flip {
        Some(f) if f.on == on => f.at,
        Some(_) => {
            ctx.data_mut(|d| d.insert_temp(fid, Flip { on, at: now }));
            now
        }
        None => {
            ctx.data_mut(|d| d.insert_temp(fid, Flip { on, at: f64::NEG_INFINITY }));
            f64::NEG_INFINITY
        }
    };
    let since = (now - at) as f32;
    let extra = if since >= 0.0 && since < CROSS {
        ctx.request_repaint();
        6.0 * motion::keys(&[(0.0, 0.0), (STRETCH / CROSS, 1.0), (1.0, 0.0)], since / CROSS) * motion::travel(ctx)
    } else {
        0.0
    };
    let st = motion::states(ctx, id, false, false, false, enabled);
    let alpha = tokens::OFF + (1.0 - tokens::OFF) * st.enabled;
    let p = ui.painter();
    let fill = palette::mix(c(C::TonePress), c(C::Accent), ground).gamma_multiply(alpha);
    p.rect_filled(rect, egui::CornerRadius::same(10), fill);
    let x = rect.left() + 10.0 + 14.0 * knob;
    // The knob: round at rest; on its way, longer on the side it goes to (kept inside the track).
    let (l, r) = if on { (x - 8.0, (x + 8.0 + extra).min(rect.right() - 2.0)) } else { ((x - 8.0 - extra).max(rect.left() + 2.0), x + 8.0) };
    let body = Rect::from_min_max(pos2(l, rect.center().y - 8.0), pos2(r, rect.center().y + 8.0));
    // A white drop, a breath greyer at its foot, with a faint rim and a short shadow.
    let round = egui::CornerRadius::same(8);
    let knob_a = alpha.max(0.8);
    p.add(crate::material::drop(body, round, 1.0, 2.0, crate::material::ink(41.0).gamma_multiply(alpha)));
    p.add(crate::material::ground_with_run(body, 8.0, c(C::Knob).gamma_multiply(knob_a), egui::Color32::TRANSPARENT, palette::KNOB_FOOT.gamma_multiply(knob_a), f32::INFINITY));
    p.add(crate::material::rim(body, round, crate::material::ink(15.0).gamma_multiply(alpha)));
    let _ = vec2;
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
                let l = ui.add(egui::Label::new(egui::RichText::new(label).font(Type::Body.font()).color(c(C::Ink))).selectable(false).sense(sense));
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
