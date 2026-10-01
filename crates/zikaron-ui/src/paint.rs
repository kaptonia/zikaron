//! Painting shared by every control: raised surfaces, text by role, and single lines that elide instead of
//! wrapping.

use crate::palette::{self, c, Lift, C};
use crate::tokens::{Radius, Type};
use egui::{Color32, Painter, Rect};

/// A raised surface: soft shadows, a fill, and the half-point ring of its elevation.
pub fn surface(p: &Painter, rect: Rect, radius: Radius, fill: Color32, lift: Lift) {
    let (ring, shadows) = palette::lift(lift);
    let r = radius.egui();
    for s in shadows.iter().rev() {
        if s.color.a() == 0 {
            continue;
        }
        let sh = egui::Shadow { offset: [0, s.y.round() as i8], blur: s.blur.round().clamp(0.0, 255.0) as u8, spread: 0, color: s.color };
        p.add(sh.as_shape(rect, r));
    }
    p.rect(rect, r, fill, egui::Stroke::new(0.5_f32, ring), egui::StrokeKind::Outside);
}

/// A flat surface: fill and a one-point line.
pub fn flat(p: &Painter, rect: Rect, radius: Radius, fill: Color32, line: Color32) {
    p.rect(rect, radius.egui(), fill, egui::Stroke::new(1.0_f32, line), egui::StrokeKind::Inside);
}

/// Lay out one line of text in a role.
pub fn galley(ui: &egui::Ui, s: &str, t: Type, colour: Color32) -> std::sync::Arc<egui::Galley> {
    ui.painter().layout_no_wrap(s.to_string(), t.font(), colour)
}

/// Text in a role, wrapping at the available width.
pub fn text(ui: &mut egui::Ui, s: &str, t: Type, colour: Color32) -> egui::Response {
    ui.add(egui::Label::new(egui::RichText::new(s).font(t.font()).color(colour).line_height(Some(t.line()))).wrap())
}

/// Text in a role on one line, elided at the end to fit `max_w` (the whole text shows on hover when cut).
pub fn line(ui: &mut egui::Ui, s: &str, t: Type, colour: Color32, max_w: f32) -> egui::Response {
    let fit = crate::width::elide_to(ui, s, t.font(), max_w.max(0.0));
    let r = ui.add(egui::Label::new(egui::RichText::new(&fit).font(t.font()).color(colour)).wrap_mode(egui::TextWrapMode::Extend));
    if fit != s {
        r.on_hover_text(s)
    } else {
        r
    }
}

/// Paint text at a point in a role, elided to `max_w`; returns the drawn rectangle.
pub fn at(p: &Painter, ui: &egui::Ui, pos: egui::Pos2, align: egui::Align2, s: &str, t: Type, colour: Color32, max_w: f32) -> Rect {
    let fit = crate::width::elide_to(ui, s, t.font(), max_w.max(0.0));
    p.text(pos, align, fit, t.font(), colour)
}

/// Body ink, second ink, quiet ink.
pub fn ink() -> Color32 {
    c(C::Ink)
}

pub fn ink2() -> Color32 {
    c(C::Ink2)
}

pub fn ink3() -> Color32 {
    c(C::Ink3)
}

/// A faint full-width rule with `gap` above and below.
pub fn rule(ui: &mut egui::Ui, gap: f32) {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, gap * 2.0 + 1.0), egui::Sense::hover());
    ui.painter().hline(rect.x_range(), rect.center().y, egui::Stroke::new(1.0_f32, c(C::Line2)));
}

/// Scale a rectangle about its center.
pub fn scaled(r: Rect, k: f32) -> Rect {
    Rect::from_center_size(r.center(), r.size() * k)
}

/// The white plate's margin around a QR code (the code's quiet zone).
pub const QR_PAD: f32 = 12.0;

/// A QR matrix of `side` points on a white plate (`QR_PAD` round it): each dark module a dot in `QR_INK`, each
/// of the three finder patterns a rounded ring round a rounded block in `QR_FINDER`. The same in both
/// appearances so any reader scans it. Answers the plate's rectangle.
pub fn qr(ui: &mut egui::Ui, modules: &[Vec<bool>], side: f32) -> Rect {
    let n = modules.len();
    let (plate, _) = ui.allocate_exact_size(egui::vec2(side + 2.0 * QR_PAD, side + 2.0 * QR_PAD), egui::Sense::hover());
    let p = ui.painter_at(plate.expand(24.0));
    surface(&p, plate, Radius::Sheet, palette::QR_PLATE, Lift::Card);
    crate::probe::qr(ui.ctx(), plate);
    if n < 21 {
        return plate;
    }
    let cell = side / n as f32;
    let o = plate.min + egui::vec2(QR_PAD, QR_PAD);
    let finders = [(0usize, 0usize), (n - 7, 0), (0, n - 7)];
    let in_finder = |x: usize, y: usize| finders.iter().any(|(fx, fy)| x >= *fx && x < fx + 7 && y >= *fy && y < fy + 7);
    for (y, row) in modules.iter().enumerate() {
        for (x, on) in row.iter().enumerate() {
            if *on && !in_finder(x, y) {
                p.circle_filled(o + egui::vec2((x as f32 + 0.5) * cell, (y as f32 + 0.5) * cell), 0.43 * cell, palette::QR_INK);
            }
        }
    }
    let round = |k: f32| egui::CornerRadius::same((k * cell).round().clamp(0.0, 255.0) as u8);
    for (fx, fy) in finders {
        let at = o + egui::vec2(fx as f32 * cell, fy as f32 * cell);
        let ring = Rect::from_min_size(at + egui::vec2(0.5 * cell, 0.5 * cell), egui::vec2(6.0 * cell, 6.0 * cell));
        p.rect_stroke(ring, round(1.9), egui::Stroke::new(cell, palette::QR_FINDER), egui::StrokeKind::Middle);
        p.rect_filled(Rect::from_min_size(at + egui::vec2(2.0 * cell, 2.0 * cell), egui::vec2(3.0 * cell, 3.0 * cell)), round(1.0), palette::QR_FINDER);
    }
    plate
}
