//! Status marks, pills, type tags and counts.
//!
//! A status is either a word in a pill (where the status is a word) or an 18-point mark: a check (passed,
//! confirmed), an empty circle (not started, waiting to be anchored, not read), an exclamation (needs
//! attention), a cross (failed) or a spinner (in progress). One size everywhere.

use crate::motion::{self, Curve};
use crate::paint;
use crate::palette::{c, Tone, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Color32, Painter, Pos2};

/// The status marks. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Mark {
    /// Passed, confirmed.
    Ok,
    /// Not started, waiting, not read.
    Todo,
    /// Needs attention.
    Warn,
    /// Failed.
    Bad,
    /// In progress on the chain (confirming, waiting to be anchored): an amber spinner.
    Busy,
    /// In progress here (syncing, reading): a blue spinner.
    Work,
}

impl Mark {
    pub const ALL: [Mark; 6] = [Mark::Ok, Mark::Todo, Mark::Warn, Mark::Bad, Mark::Busy, Mark::Work];

    pub fn as_str(self) -> &'static str {
        match self {
            Mark::Ok => "ok",
            Mark::Todo => "todo",
            Mark::Warn => "warn",
            Mark::Bad => "bad",
            Mark::Busy => "busy",
            Mark::Work => "work",
        }
    }

    /// Whether this mark turns (and so asks for frames while shown).
    pub fn spins(self) -> bool {
        matches!(self, Mark::Busy | Mark::Work)
    }
}

/// Paint a mark of `size` (18 on pages) centered at `at`.
pub fn paint_mark(ctx: &egui::Context, p: &Painter, at: Pos2, m: Mark, size: f32) {
    let r = size / 2.0;
    let k = size / 18.0;
    let pt = |x: f32, y: f32| pos2(at.x - r + x * k, at.y - r + y * k);
    let w = 1.8 * k;
    match m {
        Mark::Ok => {
            p.circle_filled(at, r, c(C::OkWash));
            p.add(egui::Shape::line(vec![pt(5.2, 9.0), pt(7.6, 11.4), pt(12.6, 6.5)], egui::Stroke::new(w, c(C::OkInk))));
        }
        Mark::Warn => {
            p.circle_filled(at, r, c(C::WarnWash));
            let ink = c(C::WarnInk);
            p.line_segment([pt(9.0, 4.8), pt(9.0, 10.0)], egui::Stroke::new(w, ink));
            p.circle_filled(pt(9.0, 12.9), 1.15 * k, ink);
        }
        Mark::Bad => {
            p.circle_filled(at, r, c(C::BadWash));
            let s = egui::Stroke::new(w, c(C::BadInk));
            let d = 4.5 * k * std::f32::consts::FRAC_1_SQRT_2;
            p.line_segment([pos2(at.x - d, at.y - d), pos2(at.x + d, at.y + d)], s);
            p.line_segment([pos2(at.x + d, at.y - d), pos2(at.x - d, at.y + d)], s);
        }
        Mark::Todo => {
            p.circle_stroke(at, r - 0.75 * k, egui::Stroke::new(1.5 * k, c(C::Dash)));
        }
        Mark::Busy | Mark::Work => {
            let colour = if m == Mark::Busy { c(C::Warn) } else { c(C::Accent) };
            spinner(ctx, p, at, r - 2.0 * k - 0.9 * k, w, colour, 0.9);
        }
    }
}

/// A three-quarter ring turning once every `period` seconds.
pub fn spinner(ctx: &egui::Context, p: &Painter, at: Pos2, radius: f32, width: f32, colour: Color32, period: f32) {
    let turn = motion::cycle(ctx, period) * std::f32::consts::TAU;
    let n = 24;
    let from = turn + std::f32::consts::FRAC_PI_4;
    let span = std::f32::consts::TAU * 0.75;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = from + span * i as f32 / n as f32;
            pos2(at.x + radius * a.cos(), at.y + radius * a.sin())
        })
        .collect();
    p.add(egui::Shape::line(pts, egui::Stroke::new(width, colour)));
}

/// A mark in its own 18 × 18 cell.
pub fn mark(ui: &mut egui::Ui, m: Mark) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(tokens::MARK, tokens::MARK), egui::Sense::hover());
    paint_mark(ui.ctx(), ui.painter(), rect.center(), m, tokens::MARK);
    resp
}

/// A mark and a line of text, 8 apart (10 in check lists), centered on the text's first line.
pub fn mark_line(ui: &mut egui::Ui, m: Mark, s: &str, t: Type, colour: Color32) -> egui::Response {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = tokens::S2;
        mark(ui, m);
        paint::text(ui, s, t, colour);
    })
    .response
}

#[derive(Clone, Copy)]
struct Pop {
    key: u64,
    t0: f64,
}

/// Pop progress of a pill whose words changed after it was first seen (0..1 then settled at 1). `delay`
/// staggers pills that pop together.
fn popped(ctx: &egui::Context, id: egui::Id, key: u64, force: bool, delay: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    let t0 = ctx.data_mut(|d| {
        let first = d.get_temp::<Pop>(id).is_none();
        let s = d.get_temp_mut_or_insert_with(id, || Pop { key, t0: if force { now } else { f64::NEG_INFINITY } });
        if !first && s.key != key {
            *s = Pop { key, t0: now };
        }
        s.t0
    });
    let a = (now - t0) as f32 - delay;
    if a < 0.0 {
        ctx.request_repaint();
        return 0.0;
    }
    let p = (a / tokens::MID).clamp(0.0, 1.0);
    if p < 1.0 {
        ctx.request_repaint();
    }
    p
}

fn pill_size(ui: &egui::Ui, s: &str, live: bool) -> egui::Vec2 {
    let g = paint::galley(ui, s, Type::Small, Color32::BLACK);
    let spin = if live { 10.0 + 5.0 } else { 0.0 };
    vec2(g.size().x + 20.0 + spin, tokens::PILL_H)
}

fn paint_pill(ui: &egui::Ui, rect: egui::Rect, s: &str, tone: Tone, live: bool, pop: f32) {
    let p = ui.painter();
    let k = if pop < 1.0 { 0.6 + 0.4 * Curve::Spring.at(pop) } else { 1.0 };
    let alpha = if pop < 1.0 { pop } else { 1.0 };
    let r = paint::scaled(rect, k);
    let fill = tone.wash().gamma_multiply(alpha);
    let ink = tone.ink().gamma_multiply(alpha);
    p.rect_filled(r, Radius::Pill.egui(), fill);
    let g = p.layout_no_wrap(s.to_string(), egui::FontId::new(Type::Small.size() * k, egui::FontFamily::Proportional), ink);
    let mut x = r.left() + 10.0 * k;
    if live {
        spinner(ui.ctx(), p, pos2(x + 5.0 * k, r.center().y), 4.25 * k, 1.5 * k, ink, 0.9);
        x += 15.0 * k;
    }
    p.galley(pos2(x, r.center().y - g.size().y / 2.0), g, ink);
}

/// A status word in a pill: 24 high, 13 text, fully round. Pops in when its words change.
pub fn pill(ui: &mut egui::Ui, s: &str, tone: Tone) -> egui::Response {
    pill_ex(ui, s, tone, false, false, 0.0)
}

/// A pill with a small spinner before its words (a state that is still moving).
pub fn pill_live(ui: &mut egui::Ui, s: &str, tone: Tone) -> egui::Response {
    pill_ex(ui, s, tone, true, false, 0.0)
}

/// A pill that pops in on first sight, `delay` seconds late (result rows pop one after another).
pub fn pill_pop(ui: &mut egui::Ui, s: &str, tone: Tone, delay: f32) -> egui::Response {
    pill_ex(ui, s, tone, false, true, delay)
}

fn pill_ex(ui: &mut egui::Ui, s: &str, tone: Tone, live: bool, force: bool, delay: f32) -> egui::Response {
    let size = pill_size(ui, s, live);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let pop = popped(ui.ctx(), resp.id.with("pop"), motion::key_of(&(s, tone as u8)), force, delay);
    paint_pill(ui, rect, s, tone, live, pop);
    resp
}

/// The width a pill takes.
pub fn pill_w(ui: &egui::Ui, s: &str, live: bool) -> f32 {
    pill_size(ui, s, live).x
}

/// A type tag: 24 high, at least 52 wide, corner 6, quiet ground.
pub fn tag(ui: &mut egui::Ui, s: &str) -> egui::Response {
    tag_in(ui, s, tokens::TAG_MIN_W)
}

/// A type tag filling a column at least `col` wide (pick lists line their tags up; longer words widen it).
pub fn tag_in(ui: &mut egui::Ui, s: &str, col: f32) -> egui::Response {
    let g = paint::galley(ui, s, Type::Small, c(C::Ink2));
    let w = (g.size().x + 16.0).max(tokens::TAG_MIN_W).max(col);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, tokens::TAG_H), egui::Sense::hover());
    paint_tag(ui.painter(), rect, g);
    resp
}

/// Paint a tag in a given rectangle (table cells).
pub fn paint_tag(p: &Painter, rect: egui::Rect, g: std::sync::Arc<egui::Galley>) {
    p.rect_filled(rect, Radius::Tag.egui(), c(C::Tag));
    p.galley(pos2(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0), g, c(C::Ink2));
}

/// How a count is shown in the rail.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Count {
    /// Things to handle: pale red ground, dark red figure.
    Alert,
    /// On the selected item: blue ground, white figure.
    Selected,
    /// Neutral.
    Soft,
}

/// Paint a count ending at `right`, centered on `y`; returns its width.
pub fn paint_count(ui: &egui::Ui, right: f32, y: f32, n: usize, how: Count) -> f32 {
    let (fill, ink) = match how {
        Count::Alert => (c(C::BadWash), c(C::BadInk)),
        Count::Selected => (c(C::AccentInk), Color32::WHITE),
        Count::Soft => (c(C::Segment), c(C::Ink2)),
    };
    let g = ui.painter().layout_no_wrap(n.to_string(), egui::FontId::new(crate::tokens::COUNT_TEXT, crate::fonts::strong()), ink);
    let w = (g.size().x + 10.0).max(18.0);
    let rect = egui::Rect::from_min_size(pos2(right - w, y - 9.0), vec2(w, 18.0));
    ui.painter().rect_filled(rect, egui::CornerRadius::same(9), fill);
    ui.painter().galley(pos2(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0), g, ink);
    w
}
