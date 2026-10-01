//! Segmented controls and tabs. The white thumb of a segmented control and the blue underline of the tabs
//! slide to the chosen cell over 200 ms.

use crate::motion::{self, Curve};
use crate::palette::{c, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

/// One segment: its words and an optional count after them (in monospace).
#[derive(Clone, Copy, Debug)]
pub struct Cell<'a> {
    pub text: &'a str,
    pub count: Option<usize>,
}

impl<'a> From<&'a str> for Cell<'a> {
    fn from(text: &'a str) -> Self {
        Cell { text, count: None }
    }
}

fn heavy_small() -> egui::FontId {
    egui::FontId::new(Type::Key.size(), crate::fonts::strong())
}

fn cell_w(ui: &egui::Ui, cell: &Cell) -> f32 {
    let t = ui.painter().layout_no_wrap(cell.text.to_string(), heavy_small(), egui::Color32::BLACK).size().x;
    let n = cell.count.map(|n| ui.painter().layout_no_wrap(n.to_string(), egui::FontId::new(Type::Tiny.size(), egui::FontFamily::Monospace), egui::Color32::BLACK).size().x + 4.0).unwrap_or(0.0);
    (t + n + 28.0).max(56.0)
}

/// Paint the sliding thumb.
fn thumb(p: &egui::Painter, r: Rect) {
    p.add(egui::Shadow { offset: [0, 1], blur: 2, spread: 0, color: egui::Color32::from_black_alpha(20) }.as_shape(r, Radius::Thumb.egui()));
    p.rect(r, Radius::Thumb.egui(), c(C::Surface), egui::Stroke::new(0.5_f32, egui::Color32::from_black_alpha(10)), egui::StrokeKind::Outside);
}

/// A segmented control sized to its words; returns the clicked index when it differs from `current`.
pub fn seg(ui: &mut egui::Ui, id_salt: &str, cells: &[Cell], current: usize) -> Option<usize> {
    let widths: Vec<f32> = cells.iter().map(|x| cell_w(ui, x)).collect();
    let total = widths.iter().sum::<f32>() + 4.0;
    let (rect, _) = ui.allocate_exact_size(vec2(total, tokens::SEG_H), egui::Sense::hover());
    paint_seg(ui, id_salt, rect, cells, &widths, current, Type::Key)
}

/// A segmented control over a given width, cells sharing it equally (the rail's seat switch).
pub fn seg_fill(ui: &mut egui::Ui, id_salt: &str, cells: &[Cell], current: usize, height: f32) -> Option<usize> {
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, height), egui::Sense::hover());
    let each = (w - 4.0) / cells.len().max(1) as f32;
    let widths = vec![each; cells.len()];
    paint_seg(ui, id_salt, rect, cells, &widths, current, Type::Small)
}

fn paint_seg(ui: &mut egui::Ui, id_salt: &str, rect: Rect, cells: &[Cell], widths: &[f32], current: usize, t: Type) -> Option<usize> {
    let id = ui.id().with(("zikaron-seg", id_salt));
    let p = ui.painter().clone();
    p.rect_filled(rect, Radius::Track.egui(), c(C::Segment));
    let inner = rect.shrink(2.0);
    let mut lefts = Vec::with_capacity(cells.len());
    let mut x = inner.left();
    for w in widths {
        lefts.push(x);
        x += w;
    }
    let cur = current.min(cells.len().saturating_sub(1));
    let tl = motion::to(ui.ctx(), id.with("thumb-x"), lefts.get(cur).copied().unwrap_or(inner.left()) - rect.left(), tokens::MID, Curve::Ease);
    let tw = motion::to(ui.ctx(), id.with("thumb-w"), widths.get(cur).copied().unwrap_or(0.0), tokens::MID, Curve::Ease);
    thumb(&p, Rect::from_min_size(pos2(rect.left() + tl, inner.top()), vec2(tw, inner.height())));
    let mut hit = None;
    for (i, cell) in cells.iter().enumerate() {
        let r = Rect::from_min_size(pos2(lefts[i], inner.top()), vec2(widths[i], inner.height()));
        let resp = ui.interact(r, id.with(i), egui::Sense::click());
        if resp.clicked() && i != cur {
            hit = Some(i);
        }
        let on = i == cur;
        let hot = motion::flag(ui.ctx(), id.with(("hot", i)), resp.hovered() || on, tokens::FAST);
        let colour = crate::palette::mix(c(C::Ink2), c(C::Ink), hot);
        let font = if on { egui::FontId::new(t.size(), crate::fonts::strong()) } else { t.font() };
        let g = p.layout_no_wrap(crate::width::elide_to(ui, cell.text, font.clone(), (widths[i] - 12.0).max(0.0)), font, colour);
        let ng = cell.count.map(|n| p.layout_no_wrap(n.to_string(), egui::FontId::new(Type::Tiny.size(), egui::FontFamily::Monospace), if on { c(C::Ink2) } else { c(C::Ink3) }));
        let total = g.size().x + ng.as_ref().map(|n| n.size().x + 4.0).unwrap_or(0.0);
        let x0 = r.center().x - total / 2.0;
        let gw = g.size().x;
        p.galley(pos2(x0, r.center().y - g.size().y / 2.0), g, colour);
        if let Some(n) = ng {
            let h = n.size().y;
            p.galley(pos2(x0 + gw + 4.0, r.center().y - h / 2.0 + 0.5), n, colour);
        }
        if !on {
            let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
    }
    hit
}

/// Tabs: 24 apart over a one-point line, the chosen one in body ink and heavier with a two-point blue
/// underline that slides. Returns the clicked index when it differs from `current`.
pub fn tabs(ui: &mut egui::Ui, id_salt: &str, labels: &[&str], current: usize) -> Option<usize> {
    let id = ui.id().with(("zikaron-tabs", id_salt));
    let w = ui.available_width();
    let font_on = egui::FontId::new(Type::Body.size(), crate::fonts::strong());
    let widths: Vec<f32> = labels.iter().map(|l| ui.painter().layout_no_wrap(l.to_string(), font_on.clone(), egui::Color32::BLACK).size().x + 2.0).collect();
    let h = 6.0 + Type::Body.line() + 10.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
    let p = ui.painter().clone();
    p.hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0_f32, c(C::Line)));
    let mut x = rect.left();
    let mut lefts = Vec::new();
    for w in &widths {
        lefts.push(x);
        x += w + tokens::S5;
    }
    let cur = current.min(labels.len().saturating_sub(1));
    let ul = motion::to(ui.ctx(), id.with("ul-x"), lefts.get(cur).copied().unwrap_or(rect.left()) - rect.left(), tokens::MID, Curve::Ease);
    let uw = motion::to(ui.ctx(), id.with("ul-w"), widths.get(cur).copied().unwrap_or(0.0), tokens::MID, Curve::Ease);
    let mut hit = None;
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(lefts[i], rect.top()), vec2(widths[i], h));
        let resp = ui.interact(r, id.with(i), egui::Sense::click());
        let on = i == cur;
        if resp.clicked() && !on {
            hit = Some(i);
        }
        let hot = motion::flag(ui.ctx(), id.with(("hot", i)), on || resp.hovered(), tokens::FAST);
        let colour = crate::palette::mix(c(C::Ink2), c(C::Ink), hot);
        let font = if on { font_on.clone() } else { Type::Body.font() };
        p.text(pos2(r.left() + 1.0, r.top() + 6.0 + Type::Body.line() / 2.0), egui::Align2::LEFT_CENTER, *l, font, colour);
        if !on {
            let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
    }
    let line = Rect::from_min_size(pos2(rect.left() + ul, rect.bottom() - 2.0), vec2(uw, 2.0));
    p.rect_filled(line, egui::CornerRadius::same(1), c(C::Accent));
    ui.add_space((tokens::S4 - ui.spacing().item_spacing.y).max(0.0));
    hit
}
