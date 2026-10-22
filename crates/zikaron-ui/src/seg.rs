//! Segmented controls and tabs. A segmented control is a capsule of grey tone; its white thumb slides to the
//! chosen cell like a drop (0.32 s): its leading edge gets there first and its trailing edge follows, so it
//! stretches on the way (by six tenths of the distance, at most 56) and lands at its own size, without bouncing.
//! The blue underline of the tabs slides to the chosen tab over 200 ms. Under reduced motion both arrive at once.

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

/// Paint the sliding thumb: the surface with a half-point rim and one short shadow.
fn thumb(p: &egui::Painter, r: Rect) {
    p.add(crate::material::drop(r, Radius::Thumb.egui(), 1.0, 2.0, crate::material::ink(15.0)));
    p.rect_filled(r, Radius::Thumb.egui(), c(C::Surface));
    p.add(crate::material::rim(r, Radius::Thumb.egui(), c(C::Ring)));
}

/// A segmented control sized to its words; returns the clicked index when it differs from `current`.
pub fn seg(ui: &mut egui::Ui, id_salt: &str, cells: &[Cell], current: usize) -> Option<usize> {
    seg_high(ui, id_salt, cells, current, tokens::SEG_H)
}

/// The small one (30 high), for tabs on a sheet's title line.
pub fn seg_small(ui: &mut egui::Ui, id_salt: &str, cells: &[Cell], current: usize) -> Option<usize> {
    seg_high(ui, id_salt, cells, current, tokens::SEG_SM_H)
}

fn seg_high(ui: &mut egui::Ui, id_salt: &str, cells: &[Cell], current: usize, h: f32) -> Option<usize> {
    let widths: Vec<f32> = cells.iter().map(|x| cell_w(ui, x)).collect();
    let total = widths.iter().sum::<f32>() + 4.0;
    let size = motion::size(ui.ctx(), ui.next_auto_id().with("zikaron-seg-size"), vec2(total, h));
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let start = crate::flow::mark(ui.ctx(), ui.layer_id());
    let hit = paint_seg(ui, id_salt, rect, cells, &widths, current, Type::Key);
    crate::flow::note(ui, motion::key_of(&("seg", id_salt)), rect, start);
    hit
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
    p.rect_filled(rect, Radius::Track.egui(), c(C::Tone));
    let inner = rect.shrink(2.0);
    let mut lefts = Vec::with_capacity(cells.len());
    let mut x = inner.left();
    for w in widths {
        lefts.push(x);
        x += w;
    }
    let cur = current.min(cells.len().saturating_sub(1));
    let want_l = lefts.get(cur).copied().unwrap_or(inner.left()) - rect.left();
    let want_w = widths.get(cur).copied().unwrap_or(0.0);
    let (tl, tw) = slide(ui.ctx(), id, want_l, want_w);
    // The stretch never takes the thumb outside its track.
    thumb(&p, Rect::from_min_size(pos2(rect.left() + tl, inner.top()), vec2(tw, inner.height())).intersect(inner));
    let mut hit = None;
    for (i, cell) in cells.iter().enumerate() {
        let r = Rect::from_min_size(pos2(lefts[i], inner.top()), vec2(widths[i], inner.height()));
        let resp = ui.interact(r, id.with(i), egui::Sense::click());
        if resp.clicked() && i != cur {
            hit = Some(i);
        }
        let on = i == cur;
        let hot = motion::states(ui.ctx(), id.with(("cell", i)), resp.hovered() || on, resp.is_pointer_button_down_on(), resp.has_focus(), true).hover;
        let colour = crate::palette::mix(c(C::Ink2), c(C::Ink), hot);
        let font = if on { egui::FontId::new(t.size(), crate::fonts::strong()) } else { t.font() };
        let g = p.layout_no_wrap(crate::width::elide_to(ui, cell.text, font.clone(), (widths[i] - 12.0).max(0.0)), font, colour);
        let ng = cell.count.map(|n| p.layout_no_wrap(n.to_string(), egui::FontId::new(Type::Tiny.size(), egui::FontFamily::Monospace), colour));
        let total = g.size().x + ng.as_ref().map(|n| n.size().x + 4.0).unwrap_or(0.0);
        let x0 = r.center().x - total / 2.0;
        let gw = g.size().x;
        p.galley(pos2(x0, r.center().y - g.size().y / 2.0), g, colour);
        if let Some(n) = ng {
            let h = n.size().y;
            p.galley(pos2(x0 + gw + 4.0, r.center().y - h / 2.0 + 0.5), n, colour);
        }
        if on {
            // Already chosen: nothing to press.
            crate::hand::sink(ui.ctx(), resp.id);
        }
    }
    hit
}

/// How long the thumb takes to slide, how much of the distance it stretches by, and at most how much.
pub const SLIDE: f32 = 0.32 * crate::tokens::PACE;
const STRETCH: f32 = 0.6;
const STRETCH_MAX: f32 = 56.0;

/// The thumb's left edge and width now, on its way to `want_l` and `want_w` (from the control's left edge). Its
/// centre and its own width ease over [`SLIDE`], interruptibly; on the way it is wider by the stretch, most at
/// half way and nothing at either end, laid on the side it moves toward: the leading edge runs ahead and the
/// trailing edge catches up.
pub fn slide(ctx: &egui::Context, id: egui::Id, want_l: f32, want_w: f32) -> (f32, f32) {
    let want_c = want_l + want_w / 2.0;
    let span_id = id.with("thumb-span");
    let cid = id.with("thumb-c");
    let before = ctx.data(|d| d.get_temp::<(f32, f32)>(span_id));
    let c = motion::glide(ctx, cid, want_c, SLIDE, Curve::Ease);
    let w = motion::glide(ctx, id.with("thumb-w"), want_w, SLIDE, Curve::Ease);
    // The run's span: from where the centre stood when the target changed.
    let (span, target) = match before {
        Some((span, target)) if (target - want_c).abs() < 0.01 => (span, target),
        _ => {
            let span = (want_c - c).abs();
            ctx.data_mut(|d| d.insert_temp(span_id, (span, want_c)));
            (span, want_c)
        }
    };
    let p = if span > 0.5 { (1.0 - (target - c).abs() / span).clamp(0.0, 1.0) } else { 1.0 };
    let stretch = (span * STRETCH).min(STRETCH_MAX) * (std::f32::consts::PI * p).sin() * motion::travel(ctx);
    let dir = if target >= c { 1.0 } else { -1.0 };
    // The stretch lies ahead: the left edge stays with the centre's run when moving right, and moves ahead when
    // moving left.
    let left = c - w / 2.0 - if dir < 0.0 { stretch } else { 0.0 };
    (left, w + stretch)
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
    let ul = motion::glide(ui.ctx(), id.with("ul-x"), lefts.get(cur).copied().unwrap_or(rect.left()) - rect.left(), tokens::MID, Curve::Ease);
    let uw = motion::glide(ui.ctx(), id.with("ul-w"), widths.get(cur).copied().unwrap_or(0.0), tokens::MID, Curve::Ease);
    let mut hit = None;
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(lefts[i], rect.top()), vec2(widths[i], h));
        let resp = ui.interact(r, id.with(i), egui::Sense::click());
        let on = i == cur;
        if resp.clicked() && !on {
            hit = Some(i);
        }
        let hot = motion::states(ui.ctx(), id.with(("tab", i)), on || resp.hovered(), resp.is_pointer_button_down_on(), resp.has_focus(), true).hover;
        let colour = crate::palette::mix(c(C::Ink2), c(C::Ink), hot);
        let font = if on { font_on.clone() } else { Type::Body.font() };
        p.text(pos2(r.left() + 1.0, r.top() + 6.0 + Type::Body.line() / 2.0), egui::Align2::LEFT_CENTER, *l, font, colour);
        if on {
            // Already chosen: nothing to press.
            crate::hand::sink(ui.ctx(), resp.id);
        }
    }
    let line = Rect::from_min_size(pos2(rect.left() + ul, rect.bottom() - 2.0), vec2(uw, 2.0));
    p.rect_filled(line, egui::CornerRadius::same(1), c(C::Accent));
    ui.add_space((tokens::S4 - ui.spacing().item_spacing.y).max(0.0));
    hit
}
