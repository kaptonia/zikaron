//! The side rail, 216 wide: the identity chip at the top (words only), the seat switch, the navigation items
//! with a white block that slides lightly to the lit one (0.42 s, no stretch), and at the bottom settings, lock
//! and the status line with the sync key. Navigation items and the two foot items are one item ([`item`]): the
//! same height, width, gap, type, glyph and words places, lit by the same block, lit by the same light under
//! the pointer. When the seat switches, items that stay slide to their new places, new ones open and fade in
//! where they belong and the others close and fade out where they were ([`crate::flow::Group`]).

use crate::icons::{self, Glyph};
use crate::mark::{self, Count};
use crate::motion::{self, Curve};
use crate::palette::{self, c, C};
use crate::seg;
use crate::tokens::{self, Type};
use egui::{pos2, vec2, Rect};

/// The rail panel's frame: rail ground, 10 across, 46 kept at the top for the window's buttons.
pub fn panel_frame() -> egui::Frame {
    egui::Frame::new().fill(crate::material::rail_ground()).inner_margin(egui::Margin { left: tokens::RAIL_PAD as i8, right: tokens::RAIL_PAD as i8, top: tokens::RAIL_TOP as i8, bottom: tokens::RAIL_PAD as i8 })
}

/// The one-point line between the rail and the page.
pub fn edge(ctx: &egui::Context, panel: Rect) {
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("zikaron-rail-edge")));
    p.vline(panel.right() - 0.5, panel.y_range(), egui::Stroke::new(1.0_f32, c(C::Line2)));
}

/// The page's frame: page ground, nothing inside.
pub fn page_frame() -> egui::Frame {
    egui::Frame::new().fill(c(C::Ground))
}

/// The identity chip: name (13 semibold) over kind (11 quiet), a caret at the right. Returns its response
/// (a click opens the identity menu).
pub fn id_chip(ui: &mut egui::Ui, name: &str, kind: &str) -> egui::Response {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 44.0), egui::Sense::click());
    let hot = motion::states(ui.ctx(), resp.id, resp.hovered(), resp.is_pointer_button_down_on(), resp.has_focus(), true).hover;
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(rect, egui::CornerRadius::same(10), c(C::Press));
    } else if hot > 0.0 {
        p.rect_filled(rect, egui::CornerRadius::same(10), c(C::Hover).gamma_multiply(hot));
    }
    let room = rect.width() - 16.0 - 22.0;
    let heavy = egui::FontId::new(Type::Small.size(), crate::fonts::strong());
    let fit = crate::width::elide_to(ui, name, heavy.clone(), room);
    p.text(pos2(rect.left() + 8.0, rect.top() + 6.0), egui::Align2::LEFT_TOP, fit, heavy, c(C::Ink));
    crate::paint::at(p, ui, pos2(rect.left() + 8.0, rect.top() + 24.0), egui::Align2::LEFT_TOP, kind, Type::Micro, c(C::Ink3), room);
    icons::glyph_at(p, Glyph::Down, pos2(rect.right() - 14.0, rect.center().y), 12.0, c(C::Ink3));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The seat switch: two equal cells, the white thumb sliding between them.
pub fn seat(ui: &mut egui::Ui, labels: [&str; 2], current: usize) -> Option<usize> {
    ui.add_space(10.0 - ui.spacing().item_spacing.y.min(10.0));
    let cells = [seg::Cell::from(labels[0]), seg::Cell::from(labels[1])];
    let hit = seg::seg_fill(ui, "rail-seat", &cells, current, 32.0);
    ui.add_space((6.0 - ui.spacing().item_spacing.y).max(0.0));
    hit
}

/// One line of the navigation list.
#[derive(Clone, Copy, Debug)]
pub enum Line<'a> {
    /// A group title (none for the first group: a 2-point gap).
    Group(Option<&'a str>),
    Item { glyph: Glyph, label: &'a str, count: usize },
}

/// How long the lit block takes to slide to another item.
pub const BLOCK: f32 = 0.42 * crate::tokens::PACE;

/// What an item shows at its right end.
#[derive(Clone, Copy, Debug)]
pub enum Trail<'a> {
    None,
    /// A count (alerts): pops in and out, jumps when it changes.
    Count(usize),
    /// A key hint ("⌘,").
    Kbd(&'a str),
}

/// One rail item in `r` (36 high): its hover ground, the light under the pointer, the glyph and the words, and
/// what it shows at its right end. Navigation items and the foot items are drawn here alike. `on` is lit (the
/// block under it is drawn by the caller).
pub fn item(ui: &egui::Ui, id: egui::Id, r: Rect, glyph: Glyph, label: &str, on: bool, trail: Trail) -> egui::Response {
    let resp = ui.interact(r, id, egui::Sense::click());
    let p = ui.painter();
    let st = motion::states(ui.ctx(), id, resp.hovered() && !on, resp.is_pointer_button_down_on() && !on, resp.has_focus(), true);
    if st.press > 0.0 {
        p.rect_filled(r, egui::CornerRadius::same(8), palette::mix(c(C::Hover), c(C::Press), st.press).gamma_multiply(st.hover.max(st.press)));
    } else if st.hover > 0.0 {
        p.rect_filled(r, egui::CornerRadius::same(8), c(C::Hover).gamma_multiply(st.hover));
    }
    let (glow, at) = crate::material::glow_at(ui, id, r, true);
    if glow > 0.0 {
        crate::material::glow(p, r, 8.0, at, c(C::Accent).gamma_multiply(0.04 * glow));
    }
    let lit = motion::flag(ui.ctx(), id.with("lit"), on, tokens::FAST);
    icons::glyph_at(p, glyph, pos2(r.left() + 10.0 + 7.5, r.center().y), 17.0, palette::mix(c(C::Ink2), c(C::Accent), lit));
    let font = if on { egui::FontId::new(14.5, crate::fonts::strong()) } else { Type::Rail.font() };
    p.text(pos2(r.left() + 10.0 + 15.0 + 10.0, r.center().y), egui::Align2::LEFT_CENTER, label, font, c(C::Ink));
    match trail {
        Trail::Count(n) => {
            mark::count_live(ui, id.with("count"), r.right() - 10.0, r.center().y, n, if on { Count::Selected } else { Count::Alert });
        }
        Trail::Kbd(k) if !k.is_empty() => {
            p.text(pos2(r.right() - 10.0, r.center().y), egui::Align2::RIGHT_CENTER, k, Type::MonoSmall.font(), c(C::Ink3));
        }
        _ => {}
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A rail item's identity across frames: its glyph and which of the items with that glyph it is.
fn item_key(glyph: Glyph, nth: usize) -> u64 {
    motion::key_of(&(format!("{glyph:?}"), nth))
}

/// The navigation list with the sliding block. `lit` is the index (among items only) of the lit one.
/// Returns the clicked item's index.
pub fn nav(ui: &mut egui::Ui, lines: &[Line], lit: Option<usize>) -> Option<usize> {
    let id = ui.id().with("zikaron-rail-nav");
    let w = ui.available_width();
    // Lay out first: every item's rectangle.
    let top = ui.cursor().min.y + 4.0;
    let mut y = top;
    let mut rects: Vec<(Rect, Glyph, &str, usize)> = Vec::new();
    let mut titles: Vec<(f32, &str)> = Vec::new();
    for l in lines {
        match l {
            Line::Group(None) => y += 2.0,
            Line::Group(Some(t)) => {
                titles.push((y + 14.0, t));
                y += 14.0 + 16.0 + 6.0;
            }
            Line::Item { glyph, label, count } => {
                rects.push((Rect::from_min_size(pos2(ui.cursor().min.x, y), vec2(w, tokens::RAIL_ITEM_H)), *glyph, label, *count));
                y += tokens::RAIL_ITEM_H + 2.0;
            }
        }
    }
    let area = Rect::from_min_max(pos2(ui.cursor().min.x, top - 4.0), pos2(ui.cursor().min.x + w, y));
    ui.allocate_rect(area, egui::Sense::hover());
    let p = ui.painter().clone();
    // The sliding block under the lit item, drawn first (under every item).
    let shown = motion::flag(ui.ctx(), id.with("hl-a"), lit.is_some(), tokens::FAST);
    let slot = p.add(egui::Shape::Noop);
    let mut group = crate::flow::Group::new(ui, id, area.min, w);
    for (ty, t) in &titles {
        let s = group.start(ui.ctx());
        p.text(pos2(area.left() + 10.0, *ty), egui::Align2::LEFT_TOP, *t, Type::Tiny.font(), c(C::Ink3));
        group.row(ui.ctx(), motion::key_of(&("title", *t)), Rect::from_min_size(pos2(area.left(), *ty - 14.0), vec2(w, 36.0)), s);
    }
    let mut hit = None;
    let mut seen: Vec<Glyph> = Vec::new();
    for (i, (r, glyph, label, count)) in rects.iter().enumerate() {
        let nth = seen.iter().filter(|g| *g == glyph).count();
        seen.push(*glyph);
        let s = group.start(ui.ctx());
        let resp = item(ui, id.with(i), *r, *glyph, label, lit == Some(i), Trail::Count(*count));
        group.row(ui.ctx(), item_key(*glyph, nth), *r, s);
        if resp.clicked() {
            hit = Some(i);
        }
    }
    let _ = group.finish(ui.ctx());
    if let Some(i) = lit.and_then(|i| rects.get(i).map(|r| (i, r.0))) {
        let hy = motion::glide(ui.ctx(), id.with("hl-y"), i.1.top() - top, BLOCK, Curve::Ease);
        let block = Rect::from_min_size(pos2(area.left(), top + hy), vec2(w, tokens::RAIL_ITEM_H));
        p.set(slot, egui::Shape::Vec(block_shapes(block, shown)));
    }
    hit
}

/// The lit block: a white card under the item (every material alike), with a half-point rim, a short and a
/// softer shadow, and a thread of light along its top.
fn block_shapes(block: Rect, alpha: f32) -> Vec<egui::Shape> {
    if alpha <= 0.0 {
        return Vec::new();
    }
    let round = egui::CornerRadius::same(8);
    vec![
        crate::material::drop(block, round, 4.0, 10.0, crate::material::ink(13.0).gamma_multiply(alpha)),
        crate::material::drop(block, round, 1.0, 2.0, crate::material::ink(15.0).gamma_multiply(alpha)),
        egui::epaint::RectShape::filled(block, round, c(C::Surface).gamma_multiply(alpha)).into(),
        crate::material::rim(block, round, c(C::Ring).gamma_multiply(alpha)),
        egui::Shape::line_segment([pos2(block.left() + 8.0, block.top() + 0.5), pos2(block.right() - 8.0, block.top() + 0.5)], egui::Stroke::new(1.0_f32, c(C::Hi).gamma_multiply(alpha))),
    ]
}

fn paint_block(p: &egui::Painter, block: Rect, alpha: f32) {
    p.extend(block_shapes(block, alpha));
}

/// The 13 of room above the rail's foot: the foot is set apart by the room alone, without a rule.
pub fn foot_rule(ui: &mut egui::Ui) {
    let w = ui.available_width();
    let _ = ui.allocate_exact_size(vec2(w, 13.0), egui::Sense::hover());
}

/// A foot item (settings, lock): the same item as the navigation's, lit by the same block when `on`; `kbd` at the
/// right end.
pub fn foot_item(ui: &mut egui::Ui, glyph: Glyph, label: &str, on: bool, kbd: &str) -> egui::Response {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, tokens::RAIL_ITEM_H), egui::Sense::hover());
    let lit = motion::flag(ui.ctx(), resp.id.with("on"), on, tokens::FAST);
    paint_block(ui.painter(), r, lit);
    item(ui, resp.id.with("item"), r, glyph, label, on, Trail::Kbd(kbd))
}

/// What the status line says.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Voice {
    Quiet,
    /// A task in progress: blue, clickable (goes back to the task's page).
    Task,
    Bad,
}

/// The sync key's three layers.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sync {
    Idle,
    Busy,
    /// Synced at frame time `at`: the check holds about 1 s.
    Done { at: f64 },
}

/// What the status line did this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Status {
    pub line_clicked: bool,
    pub sync_clicked: bool,
}

/// The status line: words at the left (12, quiet; blue for a task, red when sync failed), a thin progress bar
/// under a task, and the sync key at the right end.
pub fn status(ui: &mut egui::Ui, words: &str, voice: Voice, progress: Option<Option<f32>>, sync: Sync, sync_tip: &str) -> Status {
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 32.0), if voice == Voice::Task { egui::Sense::click() } else { egui::Sense::hover() });
    let id = resp.id;
    let p = ui.painter().clone();
    let mut out = Status::default();
    if voice == Voice::Task {
        let hot = motion::states(ui.ctx(), id.with("line"), resp.hovered(), resp.is_pointer_button_down_on(), resp.has_focus(), true).hover;
        if hot > 0.0 {
            p.rect_filled(r, egui::CornerRadius::same(8), c(C::Hover).gamma_multiply(hot));
        }
        out.line_clicked = resp.clicked();
    }
    let colour = match voice {
        Voice::Quiet => c(C::Ink3),
        Voice::Task => c(C::AccentInk),
        Voice::Bad => c(C::BadInk),
    };
    let key = Rect::from_min_size(pos2(r.right() - 4.0 - 26.0, r.center().y - 13.0), vec2(26.0, 26.0));
    crate::paint::at(&p, ui, pos2(r.left() + 10.0, r.center().y), egui::Align2::LEFT_CENTER, words, Type::Tiny, colour, key.left() - r.left() - 18.0);
    if let Some(fr) = progress {
        let bar = Rect::from_min_max(pos2(r.left() + 10.0, r.bottom() - 5.0), pos2(r.right() - 10.0, r.bottom() - 3.0));
        p.rect_filled(bar, egui::CornerRadius::same(1), c(C::Line));
        let clip = p.with_clip_rect(bar);
        match fr {
            Some(f) => {
                let shown = motion::to(ui.ctx(), id.with("frac"), f.clamp(0.0, 1.0), tokens::MID, Curve::Ease);
                clip.rect_filled(Rect::from_min_size(bar.min, vec2(bar.width() * shown, 2.0)), egui::CornerRadius::same(1), c(C::Accent));
            }
            None => {
                let ph = Curve::InOut.at(motion::cycle(ui.ctx(), tokens::CYCLE));
                let sw = bar.width() * 0.35;
                let x = bar.left() - sw + (bar.width() + sw) * ph;
                clip.rect_filled(Rect::from_min_size(pos2(x, bar.top()), vec2(sw, 2.0)), egui::CornerRadius::same(1), c(C::Accent));
            }
        }
    }
    // The sync key: refresh glyph, turning ring, check, crossfading in place.
    let now = crate::motion::now(ui.ctx());
    let layer = match sync {
        Sync::Busy => 1u8,
        Sync::Done { at } if now - at < 1.1 => 2,
        _ => 0,
    };
    let live = layer == 0;
    let kr = ui.interact(key, id.with("sync"), if live { egui::Sense::click() } else { egui::Sense::hover() });
    let hot = motion::states(ui.ctx(), id.with("sync"), kr.hovered(), kr.is_pointer_button_down_on(), kr.has_focus(), live).hover;
    if hot > 0.0 {
        p.rect_filled(key, egui::CornerRadius::same(7), c(C::Hover).gamma_multiply(hot));
    }
    let idle = motion::to(ui.ctx(), id.with("l0"), if layer == 0 { 1.0 } else { 0.0 }, 0.16 * crate::tokens::PACE, Curve::Ease);
    let ring = motion::to(ui.ctx(), id.with("l1"), if layer == 1 { 1.0 } else { 0.0 }, 0.16 * crate::tokens::PACE, Curve::Ease);
    let done = motion::to(ui.ctx(), id.with("l2"), if layer == 2 { 1.0 } else { 0.0 }, 0.16 * crate::tokens::PACE, Curve::Ease);
    let grow = |on: f32, lo: f32| lo + (1.0 - lo) * Curve::Spring.at(on);
    if idle > 0.0 {
        icons::glyph_at(&p, Glyph::Refresh, key.center(), 14.0 * grow(idle, 0.6), palette::mix(c(C::Ink2), c(C::Ink), hot).gamma_multiply(idle));
    }
    if ring > 0.0 {
        let rr = 6.5 * grow(ring, 0.6) - 0.8;
        p.circle_stroke(key.center(), rr, egui::Stroke::new(1.6_f32, c(C::Line).gamma_multiply(ring)));
        let turn = motion::cycle(ui.ctx(), 0.75) * std::f32::consts::TAU;
        let arc: Vec<egui::Pos2> = (0..=12)
            .map(|k| {
                let a = turn - std::f32::consts::FRAC_PI_2 - std::f32::consts::FRAC_PI_4 + std::f32::consts::FRAC_PI_2 * k as f32 / 12.0;
                pos2(key.center().x + rr * a.cos(), key.center().y + rr * a.sin())
            })
            .collect();
        p.add(egui::Shape::line(arc, egui::Stroke::new(1.6_f32, c(C::Accent).gamma_multiply(ring))));
    }
    if done > 0.0 {
        icons::glyph_at(&p, Glyph::Ok, key.center(), 14.0 * grow(done, 0.5), c(C::Ok).gamma_multiply(done));
    }
    if layer == 2 {
        ui.ctx().request_repaint();
    }
    if live {
        out.sync_clicked = kr.clicked();
        let _ = crate::layer::tip(kr, sync_tip).on_hover_cursor(egui::CursorIcon::PointingHand);
    }
    let _ = resp;
    out
}
