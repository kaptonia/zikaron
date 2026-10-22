//! Painting shared by every control: raised surfaces, text by role, and single lines that elide instead of
//! wrapping.

use crate::palette::{self, c, C};
use crate::tokens::{Radius, Type};
use egui::{Color32, Painter, Rect};

/// A flat surface: fill and a one-point line.
pub fn flat(p: &Painter, rect: Rect, radius: Radius, fill: Color32, line: Color32) {
    p.rect(rect, radius.egui(), fill, egui::Stroke::new(1.0_f32, line), egui::StrokeKind::Inside);
}

/// Lay out one line of text in a role.
pub fn galley(ui: &egui::Ui, s: &str, t: Type, colour: Color32) -> std::sync::Arc<egui::Galley> {
    ui.painter().layout_no_wrap(s.to_string(), t.font(), colour)
}

/// Text laid out before it is placed: in `font`, lines `line_h` apart, wrapping at `max_w` where the words allow
/// (as a wrapping label does). A widget that takes one cell of its caller's layout measures its words with this.
pub fn wrapped(ui: &egui::Ui, s: &str, font: egui::FontId, colour: Color32, line_h: f32, max_w: f32) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::single_section(s.to_string(), egui::TextFormat { font_id: font, color: colour, line_height: Some(line_h), ..Default::default() });
    job.wrap = egui::text::TextWrapping { max_width: max_w.max(1.0), ..Default::default() };
    ui.painter().layout_job(job)
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
        crate::layer::tip(r, s)
    } else {
        r
    }
}

/// The height of [`lines`]: a row they stand in is at least this tall (`width::then_at`), so they sit on its
/// middle line rather than hang from its top.
pub fn lines_height(ui: &egui::Ui, rows: &[(&str, Type, Color32)], gap: f32) -> f32 {
    rows.iter().map(|(s, t, colour)| galley(ui, s, *t, *colour).size().y).sum::<f32>() + gap * rows.len().saturating_sub(1) as f32
}

/// Lines stacked `gap` apart, each in its role on one line elided to `max_w` (whole on hover when cut), in one cell
/// of the caller's layout (`width::cell`): two lines stand where a row's words go, centered in the row as one when
/// the row is as tall as they are ([`lines_height`]).
pub fn lines(ui: &mut egui::Ui, rows: &[(&str, Type, Color32)], gap: f32, max_w: f32) -> egui::Response {
    let h = lines_height(ui, rows, gap);
    crate::width::cell(ui, egui::vec2(max_w.max(0.0), h), egui::Layout::top_down(egui::Align::Min), |ui| {
        ui.spacing_mut().item_spacing.y = gap;
        for (s, t, colour) in rows {
            line(ui, s, *t, *colour, max_w);
        }
    })
    .1
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
    crate::material::plate(&p, plate, Radius::Tag, palette::QR_PLATE);
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

/// Words that roll when they change (a figure on a tile): the old words rise out of `line` and fade while the
/// new ones rise in from below (0.3 s); first seen, they are simply there. Drawn left-aligned in `line`,
/// centred on it, clipped to it.
pub fn rolling(ui: &egui::Ui, id: egui::Id, line: Rect, s: &str, font: egui::FontId, colour: Color32) {
    let ctx = ui.ctx();
    let now = crate::motion::now(ui.ctx());
    let kid = id.with("rolling");
    let (cur, prev, at) = ctx.data(|d| d.get_temp::<(String, String, f64)>(kid)).unwrap_or((s.to_string(), String::new(), f64::NEG_INFINITY));
    let (prev, at) = if cur != s {
        ctx.data_mut(|d| d.insert_temp(kid, (s.to_string(), cur.clone(), now)));
        (cur, now)
    } else {
        if ctx.data(|d| d.get_temp::<(String, String, f64)>(kid)).is_none() {
            ctx.data_mut(|d| d.insert_temp(kid, (s.to_string(), String::new(), f64::NEG_INFINITY)));
        }
        (prev, at)
    };
    let t = ((now - at) as f32 / crate::tokens::SLOW).clamp(0.0, 1.0);
    let p = ui.painter().with_clip_rect(line.expand2(egui::vec2(4.0, 2.0)).intersect(ui.clip_rect()));
    let y = line.center().y;
    if t >= 1.0 || prev.is_empty() || now < at {
        p.text(egui::pos2(line.left(), y), egui::Align2::LEFT_CENTER, s, font, colour);
        return;
    }
    ctx.request_repaint();
    let e = crate::motion::Curve::Ease.at(t);
    let dy = line.height() * 0.6 * crate::motion::travel(ctx);
    p.text(egui::pos2(line.left(), y - dy * e), egui::Align2::LEFT_CENTER, prev, font.clone(), colour.gamma_multiply(1.0 - e));
    p.text(egui::pos2(line.left(), y + dy * (1.0 - e)), egui::Align2::LEFT_CENTER, s, font, colour.gamma_multiply(e));
}
