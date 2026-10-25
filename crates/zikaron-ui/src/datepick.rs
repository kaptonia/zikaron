//! **The date field** (the "from · to" pair of a search row): one field in the single-line field's frame
//! (38 high, corner 10, 12 inside left and right) that opens a calendar on a floating card.
//!
//! The card is drawn the way the menus are ([`crate::menu`]): a floating surface (menu corner, menu shadow),
//! 5 inside, growing from its anchor corner from 0.97 and rising 4 over 120 ms, played backwards when it
//! leaves; a press outside the card, Esc, or picking a day closes it. Opened inside a pick list or a sheet it
//! hangs above that overlay, and its Esc closes the calendar only ([`crate::layer`]). It stays with its field
//! while the page scrolls under it ([`crate::layer::follow`]).
//!
//! The calendar has three levels in the same card, one size: **days** (a head "‹ month ›", one row of weekdays
//! in the smallest type, six rows of seven day cells 32 × 30, corner 6), **months** (the head "‹ year ›" and the
//! twelve months, four rows of three) and **years** (the head "‹ first – last ›" and twelve years, four rows of
//! three: a decade and the year either side of it). The head's words of the days and months levels are a key:
//! pressed, the card goes up a level; a month or a year picked goes back down one, and a day picked closes the
//! card. ‹ › turn a month, a year or ten years. A level coming in fades and grows in place (0.2 s). Cells light
//! at once under the pointer (the soft ground; a step darker pressed); the chosen day, month or year is a solid
//! accent key's ground with white figures; today, this month and this year are framed by a 1.5-point accent
//! line; days outside the month and the two years outside the decade are in ink three (still pickable). Under a
//! separator, one row of two menu rows, "today · clear" (the whole row filled with the accent on hover, like a
//! menu row), on every level.
//!
//! A calendar may be given a bound: the days up to it (itself included) are faint and take no press, nor does
//! "today" when it is one of them.
//!
//! **This control only picks a day**, written `YYYY-MM-DD`. What counts as today, which zone the days are in
//! and how a list is filtered by them are the caller's.

use crate::icons::{self, Glyph};

use crate::palette::{self, c, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

/// The words the calendar says (the caller gives them in its language).
pub struct Words<'a> {
    /// Monday to Sunday.
    pub weekdays: [&'a str; 7],
    /// The month row's words for a year and a month (the year and the month, each followed by its word in
    /// the current language), written by the caller.
    pub month: fn(i32, u32) -> String,
    /// A year alone and a month alone (the months and years levels).
    pub year: fn(i32) -> String,
    pub month_alone: fn(u32) -> String,
    pub today: &'a str,
    pub clear: &'a str,
}

/// One day: year, month, day.
pub type Day = (i32, u32, u32);

const CELL_W: f32 = 32.0;
const CELL_H: f32 = 30.0;
const PAD: f32 = 5.0;
const HEAD_H: f32 = 36.0;
const WEEK_H: f32 = 24.0;
const SEP_H: f32 = 11.0;
const ROW_H: f32 = 34.0;

/// How many days this month has in the Gregorian calendar.
pub fn days_in(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        _ => 28,
    }
}

/// The weekday, zero for Monday (Sakamoto's method).
pub fn weekday(y: i32, m: u32, d: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let yy = if m < 3 { y - 1 } else { y };
    let sun0 = (yy + yy / 4 - yy / 100 + yy / 400 + T[(m - 1) as usize] + d as i32).rem_euclid(7);
    ((sun0 + 6) % 7) as u32
}

/// `YYYY-MM-DD` read as a day; any other shape is `None`.
pub fn parse(s: &str) -> Option<Day> {
    let b = s.trim();
    let (y, rest) = b.split_once('-')?;
    let (m, d) = rest.split_once('-')?;
    let (y, m, d) = (y.parse::<i32>().ok()?, m.parse::<u32>().ok()?, d.parse::<u32>().ok()?);
    ((1..=12).contains(&m) && d >= 1 && d <= days_in(y, m) && b.len() == 10).then_some((y, m, d))
}

/// A day written `YYYY-MM-DD`.
pub fn fmt((y, m, d): Day) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

/// Whether `day` can be picked under the bound `after` (the days up to it, itself included, cannot).
pub fn pickable(day: Day, after: Option<Day>) -> bool {
    after.is_none_or(|a| day > a)
}

/// The month `by` months away, across years.
fn shift((y, m): (i32, u32), by: i32) -> (i32, u32) {
    let k = y * 12 + (m as i32 - 1) + by;
    (k.div_euclid(12), (k.rem_euclid(12) + 1) as u32)
}

/// The six rows of seven days the month page shows (the first is the Monday of the week holding the 1st).
fn grid((y, m): (i32, u32)) -> Vec<Day> {
    let lead = weekday(y, m, 1);
    let (py, pm) = shift((y, m), -1);
    let prev_n = days_in(py, pm);
    let n = days_in(y, m);
    (0..42)
        .map(|i: u32| {
            if i < lead {
                (py, pm, prev_n - lead + i + 1)
            } else if i - lead < n {
                (y, m, i - lead + 1)
            } else {
                let (ny, nm) = shift((y, m), 1);
                (ny, nm, i - lead - n + 1)
            }
        })
        .collect()
}

/// A date field: `value` is the day now (empty is no bound), `hint` what it says while empty, `w` its width,
/// `today` the caller's today (framed on the page, picked by "today"). Returns whether the value changed this
/// frame.
pub fn field(ui: &mut egui::Ui, id_salt: &str, value: &mut String, hint: &str, w: f32, today: Day, words: &Words) -> bool {
    let id = ui.id().with(("zikaron-date", id_salt));
    let (rect, resp) = ui.allocate_exact_size(vec2(w, tokens::INPUT_H), egui::Sense::click());
    let open = crate::menu::is_open(ui.ctx(), id);
    let edge = if open {
        crate::input::Edge::Focus
    } else if resp.hovered() {
        crate::input::Edge::Hover
    } else {
        crate::input::Edge::Rest
    };
    ui.painter().add(crate::input::frame_shape(ui.ctx(), rect, id, edge));
    let inner = rect.shrink2(vec2(tokens::INPUT_PAD_X, 0.0));
    let icon_x = inner.right() - tokens::INPUT_ICON / 2.0;
    icons::glyph_at(ui.painter(), Glyph::Calendar, pos2(icon_x, rect.center().y), tokens::INPUT_ICON, c(C::Ink3));
    let room = (inner.width() - tokens::INPUT_ICON - tokens::KEY_GAP).max(0.0);
    let (text, font, ink) = if value.is_empty() { (hint.to_string(), Type::Body.font(), c(C::Ink3)) } else { (value.clone(), Type::Mono.font(), c(C::Ink)) };
    let shown = crate::width::elide_to(ui, &text, font.clone(), room);
    ui.painter().text(pos2(inner.left(), rect.center().y), egui::Align2::LEFT_CENTER, shown, font, ink);
    let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        crate::menu::toggle(ui.ctx(), id);
        // Opening turns to the month of the day now (today's month when there is none).
        let (y, m, _) = parse(value).unwrap_or(today);
        ui.ctx().data_mut(|d| d.insert_temp(id.with("view"), (y, m)));
    }
    let picked = calendar(ui.ctx(), Some(ui.layer_id()), id, rect, ui.clip_rect(), value, None, today, words);
    crate::menu::stay(ui, id, rect);
    match picked {
        Some(v) if v != *value => {
            *value = v;
            true
        }
        _ => false,
    }
}

/// A calendar hung from a control the caller drew (a date block of [`span`], whose calendar goes by
/// [`block_salt`], from the same `ui`): pressing `resp` opens or closes it on the month of `value` (when there is
/// none, the bound's month, else today's); a day picked lands in `value`, the days up to `after` not pickable.
/// "Clear" leaves `value` as it was (a block always holds a day). Returns whether a day was picked this frame, the
/// day it held among them.
#[allow(clippy::too_many_arguments)]
pub fn hang(ui: &mut egui::Ui, id_salt: &str, resp: &egui::Response, value: &mut String, after: Option<Day>, today: Day, words: &Words) -> bool {
    let id = ui.id().with(("zikaron-date", id_salt));
    if resp.clicked() {
        crate::menu::toggle(ui.ctx(), id);
        let (y, m, _) = parse(value).or(after).unwrap_or(today);
        ui.ctx().data_mut(|d| d.insert_temp(id.with("view"), (y, m)));
    }
    let picked = calendar(ui.ctx(), Some(ui.layer_id()), id, resp.rect, ui.clip_rect(), value, after, today, words);
    crate::menu::stay(ui, id, resp.rect);
    match picked {
        Some(v) if !v.is_empty() => {
            *value = v;
            true
        }
        _ => false,
    }
}

/// One date block's words: the month (with the year, when the span crosses one) over the day.
#[derive(Clone, Copy, Debug)]
pub struct Block<'a> {
    pub month: &'a str,
    pub day: &'a str,
}

/// A block's inside margin, least width, and the room between a block and the words beside it.
const BLOCK_PAD_X: f32 = 12.0;
const BLOCK_MIN_W: f32 = 52.0;
const BLOCK_GAP: f32 = 12.0;

/// How wide one block is: its wider line and its margins.
fn block_w(ui: &egui::Ui, b: Block) -> f32 {
    let w = |s: &str, t: Type| ui.painter().layout_no_wrap(s.to_string(), t.font(), egui::Color32::BLACK).size().x;
    (w(b.month, Type::Micro).max(w(b.day, Type::Strong)) + BLOCK_PAD_X * 2.0).max(BLOCK_MIN_W).ceil()
}

/// How wide a [`span`] is.
pub fn span_width(ui: &egui::Ui, from: Block, between: &str, to: Block) -> f32 {
    let mid = ui.painter().layout_no_wrap(between.to_string(), Type::Note.font(), egui::Color32::BLACK).size().x.ceil();
    block_w(ui, from) + BLOCK_GAP + mid + BLOCK_GAP + block_w(ui, to)
}

/// The id salt the calendar of block `i` of a [`span`] goes by (0 the start, 1 the end), for [`hang`].
pub fn block_salt(id_salt: &str, i: usize) -> String {
    format!("{id_salt}-{i}")
}

/// A span of days (a validity window): two date blocks as tall as a field, each the month small over the day,
/// and between them the words that say how long it is. The months, days and words roll in place when they change
/// (a preset picked). A block `live` (`live[0]` the start, `live[1]` the end) is a control, lit under the pointer,
/// that the caller hangs a calendar from ([`hang`] with [`block_salt`]); the others only show. Returns the two
/// blocks' responses.
pub fn span(ui: &mut egui::Ui, id_salt: &str, from: Block, between: &str, to: Block, live: [bool; 2]) -> [egui::Response; 2] {
    let id = ui.id().with(("zikaron-span", id_salt));
    let (fw, tw) = (block_w(ui, from), block_w(ui, to));
    let w = span_width(ui, from, between, to);
    let (rect, _) = ui.allocate_exact_size(vec2(w, tokens::INPUT_H), egui::Sense::hover());
    let mut out: Vec<egui::Response> = Vec::with_capacity(2);
    for (i, (b, bw, left)) in [(from, fw, rect.left()), (to, tw, rect.right() - tw)].into_iter().enumerate() {
        let r = Rect::from_min_size(pos2(left, rect.top()), vec2(bw, tokens::INPUT_H));
        let bid = id.with(i);
        let live = live[i];
        let resp = ui.interact(r, bid, if live { egui::Sense::click() } else { egui::Sense::hover() });
        let open = live && crate::menu::is_open(ui.ctx(), ui.id().with(("zikaron-date", block_salt(id_salt, i).as_str())));
        block_ground(ui, r, bid, live, open, live && crate::flow::pointer_on(ui, r), resp.is_pointer_button_down_on());
        let line = |s: &str, t: Type, y: f32, ink: egui::Color32, salt: &str| {
            let sw = ui.painter().layout_no_wrap(s.to_string(), t.font(), egui::Color32::BLACK).size().x;
            let l = Rect::from_center_size(pos2(r.center().x, y), vec2(sw, t.line()));
            crate::paint::rolling(ui, bid.with(salt), l, s, t.font(), ink);
        };
        line(b.month, Type::Micro, r.top() + 3.0 + Type::Micro.line() / 2.0, c(C::Ink2), "month");
        line(b.day, Type::Strong, r.top() + 1.0 + Type::Micro.line() + Type::Strong.line() / 2.0, c(C::Ink), "day");
        if live {
            crate::probe::spot(ui.ctx(), &block_salt(id_salt, i), r);
        }
        out.push(if live { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp });
    }
    let mid = Rect::from_min_max(pos2(rect.left() + fw + BLOCK_GAP, rect.top()), pos2(rect.right() - tw - BLOCK_GAP, rect.bottom()));
    crate::paint::rolling(ui, id.with("between"), mid, between, Type::Note.font(), c(C::Ink2));
    let to_resp = out.pop().expect("two blocks");
    let from_resp = out.pop().expect("two blocks");
    [from_resp, to_resp]
}

/// A date block's ground: a grey tone when it only shows; a control's, a white key face with its half-point edge
/// and drops, a step darker under the pointer and pressed, ringed while its calendar is open.
fn block_ground(ui: &egui::Ui, r: Rect, id: egui::Id, live: bool, open: bool, lit: bool, down: bool) {
    let p = ui.painter();
    let round = Radius::Tag.egui();
    if !live {
        p.rect_filled(r, round, c(C::Tone));
        return;
    }
    let ring = crate::motion::flag(ui.ctx(), id.with("open"), open, tokens::FAST);
    if ring > 0.0 {
        p.rect_stroke(r.expand(2.0), egui::CornerRadius::same(Radius::Tag.px() as u8 + 2), egui::Stroke::new(4.0_f32, c(C::Focus).gamma_multiply(ring)), egui::StrokeKind::Middle);
    }
    p.add(crate::material::drop(r, round, 1.0, 3.0, c(C::KeyDrop2)));
    p.add(crate::material::drop(r, round, 1.0, 1.0, c(C::KeyDrop)));
    let face = if down { C::KeyFacePress } else if lit { C::KeyFaceHover } else { C::KeyFace };
    p.rect_filled(r, round, c(face));
    p.add(crate::material::rim(r, round, c(if lit || down { C::KeyEdgeHover } else { C::KeyEdge })));
}

/// Which level the calendar shows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Level {
    Days,
    Months,
    Years,
}

/// The calendar's page: its level, the year and month it stands on, and when the level last changed.
#[derive(Clone, Copy, Debug)]
struct View {
    level: Level,
    y: i32,
    m: u32,
    at: f64,
}

/// The first of the twelve years the years level shows around `y`: its decade, with the year before and the year
/// after it (those two in the quiet ink, as days outside the month are).
fn decade(y: i32) -> i32 {
    y.div_euclid(10) * 10 - 1
}

/// One cell of a calendar level: lit under the pointer, a step darker pressed, solid when chosen, framed when it
/// is today's; a month or a year (`wide`) lies on a grey tone even at rest; a day past the bound (`off`) is faint
/// and takes no press. Returns whether it was picked.
#[allow(clippy::too_many_arguments)]
fn cell(ui: &mut egui::Ui, id: egui::Id, r: Rect, words: &str, chosen: bool, now_ring: bool, quiet: bool, wide: bool, off: bool, alpha: f32) -> bool {
    if off {
        ui.interact(r, id, egui::Sense::hover());
        let a = alpha * tokens::OFF;
        if now_ring {
            ui.painter().rect_stroke(r, Radius::Cell.egui(), egui::Stroke::new(1.5_f32, c(C::Accent).gamma_multiply(a)), egui::StrokeKind::Inside);
        }
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, words, Type::Body.font(), c(C::Ink3).gamma_multiply(a));
        return false;
    }
    let resp = ui.interact(r, id, egui::Sense::click());
    let lit = crate::flow::pointer_on(ui, r);
    let p = ui.painter();
    let round = if wide { Radius::Menu } else { Radius::Cell };
    if chosen {
        p.add(crate::material::drop(r, round.egui(), 1.0, 2.0, crate::material::ink(46.0).gamma_multiply(alpha)));
        p.add(crate::material::solid(r, round.px(), c(C::Accent).gamma_multiply(alpha)));
    } else if lit {
        let press = resp.is_pointer_button_down_on();
        p.rect_filled(r, round.egui(), c(if press { C::TonePress } else { C::ToneHover }).gamma_multiply(alpha));
    } else if wide {
        p.rect_filled(r, round.egui(), c(C::Tone).gamma_multiply(alpha));
    }
    if now_ring && !chosen {
        p.rect_stroke(r, round.egui(), egui::Stroke::new(1.5_f32, c(C::Accent).gamma_multiply(alpha)), egui::StrokeKind::Inside);
    }
    let ink = if chosen { palette::ON_SOLID } else if now_ring { c(C::AccentInk) } else if quiet { c(C::Ink3) } else { c(C::Ink) };
    let font = if chosen || now_ring { egui::FontId::new(Type::Body.size(), crate::fonts::strong()) } else { Type::Body.font() };
    p.text(r.center(), egui::Align2::CENTER_CENTER, words, font, ink.gamma_multiply(alpha));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

/// The open calendar card; returns the day picked (`Some("")` is clear).
#[allow(clippy::too_many_arguments)]
fn calendar(ctx: &egui::Context, parent: Option<egui::LayerId>, id: egui::Id, anchor: Rect, bounds: Rect, value: &str, after: Option<Day>, today: Day, words: &Words) -> Option<String> {
    if !crate::menu::is_open(ctx, id) {
        return None;
    }
    let now = crate::motion::now(ctx);
    let e = crate::layer::entrance(ctx, &[id.with("keep")], id.with("age"), tokens::FAST);
    let view_id = id.with("view");
    let (vy, vm) = ctx.data(|d| d.get_temp::<(i32, u32)>(view_id)).unwrap_or((today.0, today.1));
    // The level: a new opening starts on the days of its month.
    let level_id = id.with("level");
    let opened = ctx.data(|d| d.get_temp::<u64>(id.with("opened"))).unwrap_or(0);
    let mut view = ctx.data(|d| d.get_temp::<(u64, View)>(level_id)).filter(|(o, _)| *o == opened).map(|(_, v)| v).unwrap_or(View { level: Level::Days, y: vy, m: vm, at: f64::NEG_INFINITY });
    if view.level == Level::Days {
        view.y = vy;
        view.m = vm;
    }
    let w = CELL_W * 7.0 + PAD * 2.0 + 6.0;
    let h = PAD + HEAD_H + WEEK_H + CELL_H * 6.0 + SEP_H + ROW_H + PAD;
    let drop = crate::layer::drop_card(ctx, anchor, bounds, vec2(w, h), true);
    let (rect, origin) = (drop.rect, drop.origin);
    crate::layer::card_at(ctx, id, rect);
    let layer = egui::LayerId::new(egui::Order::Foreground, id.with("menu"));
    drop.enter(ctx, layer, e);
    let chosen = parse(value);
    let mut picked: Option<String> = None;
    let mut next = view;
    crate::layer::place(ctx, layer, parent);
    let mut guarded = false;
    // A level coming in fades and grows in place.
    let come = ((now - view.at) as f32 / tokens::MID).clamp(0.0, 1.0);
    if come < 1.0 {
        ctx.request_repaint();
    }
    let come_e = crate::motion::Curve::Ease.at(if now < view.at { 1.0 } else { come });
    crate::layer::over(ctx, layer).show(ctx, |ui| {
        // The guard over the whole window, then the card's body, in the card's own layer: a press outside the
        // card closes it and reaches nothing under it; a press on the card off its rows does nothing.
        guarded = crate::layer::under(ui, crate::layer::Under::Guard);
        crate::layer::body(ui, rect);
        ui.multiply_opacity(e);
        crate::material::paint(ui.painter(), rect, Radius::Menu, crate::material::Level::Float, 0.0);
        let left = rect.left() + PAD + 3.0;
        // Head row: ‹ words ›.
        let head = Rect::from_min_size(pos2(left, rect.top() + PAD), vec2(CELL_W * 7.0, HEAD_H));
        let mut turn = 0;
        for (k, glyph, at) in [(-1, Glyph::Back, head.left()), (1, Glyph::Fwd, head.right() - CELL_W)] {
            let r = Rect::from_min_size(pos2(at, head.top() + (HEAD_H - CELL_H) / 2.0), vec2(CELL_W, CELL_H));
            let resp = ui.interact(r, id.with(("turn", k)), egui::Sense::click());
            crate::button::paint_icon_key_at(ui, r, &resp, glyph, true, Radius::Cell.px());
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                turn = k;
            }
        }
        let title = match view.level {
            Level::Days => (words.month)(view.y, view.m),
            Level::Months => (words.year)(view.y),
            Level::Years => {
                let d0 = decade(view.y);
                format!("{} \u{2013} {}", (words.year)(d0), (words.year)(d0 + 11))
            }
        };
        // The head's words: a key that goes up a level (the years level's words are not a key).
        let g = ui.painter().layout_no_wrap(title, Type::Key.font(), c(C::Ink));
        let tr = Rect::from_center_size(head.center(), vec2(g.size().x + 20.0, CELL_H));
        if view.level != Level::Years {
            let resp = ui.interact(tr, id.with("up"), egui::Sense::click());
            let lit = crate::flow::pointer_on(ui, tr);
            if lit {
                ui.painter().rect_filled(tr, Radius::Cell.egui(), c(if resp.is_pointer_button_down_on() { C::TonePress } else { C::Tone }));
            }
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                next = View { level: if view.level == Level::Days { Level::Months } else { Level::Years }, at: now, ..view };
            }
        }
        ui.painter().galley(pos2(tr.center().x - g.size().x / 2.0, tr.center().y - g.size().y / 2.0), g, c(C::Ink));
        if turn != 0 {
            next = match view.level {
                Level::Days => {
                    let (y, m) = shift((view.y, view.m), turn);
                    View { y, m, ..next }
                }
                Level::Months => View { y: view.y + turn, ..next },
                Level::Years => View { y: view.y + 10 * turn, ..next },
            };
            // Only the years a day can be written in (`YYYY`, read back by `parse`).
            next.y = next.y.clamp(1, 9999);
        }
        // The level's page: the room of the weekdays and the six rows of days.
        let top = head.bottom();
        let page = Rect::from_min_size(pos2(left, top), vec2(CELL_W * 7.0, WEEK_H + CELL_H * 6.0));
        let k = 0.96 + 0.04 * come_e * crate::motion::travel(ctx) + 0.04 * (1.0 - crate::motion::travel(ctx));
        let at = |r: Rect| Rect::from_center_size(page.center() + (r.center() - page.center()) * k, r.size() * k);
        let a = come_e;
        match view.level {
            Level::Days => {
                let wy = top + WEEK_H / 2.0;
                for (i, wd) in words.weekdays.iter().enumerate() {
                    let r = at(Rect::from_center_size(pos2(left + CELL_W * (i as f32 + 0.5), wy), vec2(CELL_W, WEEK_H)));
                    ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, *wd, Type::Micro.font(), c(C::Ink3).gamma_multiply(a));
                }
                let rows_top = top + WEEK_H;
                for (i, day) in grid((view.y, view.m)).into_iter().enumerate() {
                    let (col, row) = ((i % 7) as f32, (i / 7) as f32);
                    let r = at(Rect::from_min_size(pos2(left + CELL_W * col, rows_top + CELL_H * row), vec2(CELL_W, CELL_H)).shrink(1.0));
                    let here = (day.0, day.1) == (view.y, view.m);
                    let open = pickable(day, after);
                    crate::probe::spot(ui.ctx(), &if open { fmt(day) } else { format!("{} off", fmt(day)) }, r);
                    if cell(ui, id.with(("day", i)), r, &day.2.to_string(), chosen == Some(day), day == today, !here, false, !open, a) {
                        picked = Some(fmt(day));
                    }
                }
            }
            Level::Months => {
                let (cw, ch) = (page.width() / 3.0, page.height() / 4.0);
                for m in 1..=12u32 {
                    let (col, row) = (((m - 1) % 3) as f32, ((m - 1) / 3) as f32);
                    let r = at(Rect::from_min_size(pos2(page.left() + cw * col, page.top() + ch * row), vec2(cw, ch)).shrink2(vec2(4.0, 6.0)));
                    let on = chosen.map(|d| (d.0, d.1) == (view.y, m)).unwrap_or(false);
                    if cell(ui, id.with(("month", m)), r, &(words.month_alone)(m), on, (today.0, today.1) == (view.y, m), false, true, false, a) {
                        next = View { level: Level::Days, m, at: now, ..view };
                    }
                }
            }
            Level::Years => {
                let (cw, ch) = (page.width() / 3.0, page.height() / 4.0);
                let d0 = decade(view.y);
                for k in 0..12 {
                    let y = d0 + k;
                    let (col, row) = ((k % 3) as f32, (k / 3) as f32);
                    let r = at(Rect::from_min_size(pos2(page.left() + cw * col, page.top() + ch * row), vec2(cw, ch)).shrink2(vec2(4.0, 6.0)));
                    let on = chosen.map(|d| d.0 == y).unwrap_or(false);
                    if cell(ui, id.with(("year", k)), r, &(words.year)(y), on, today.0 == y, k == 0 || k == 11, true, false, a) {
                        next = View { level: Level::Months, y, at: now, ..view };
                    }
                }
            }
        }
        // The foot row of two keys on a grey tone, 6 apart: today · clear.
        let foot_top = top + WEEK_H + CELL_H * 6.0 + SEP_H;
        let half = (rect.width() - PAD * 2.0) / 2.0;
        for (k, label) in [(0, words.today), (1, words.clear)] {
            let r = Rect::from_min_size(pos2(rect.left() + PAD + half * k as f32, foot_top), vec2(half, ROW_H));
            let face = if k == 0 { Rect::from_min_max(r.min, pos2(r.right() - 3.0, r.bottom())) } else { Rect::from_min_max(pos2(r.left() + 3.0, r.top()), r.max) };
            // "Today" past the bound is faint and takes no press, as the day is.
            if k == 0 && !pickable(today, after) {
                ui.interact(r, id.with(("foot", k)), egui::Sense::hover());
                ui.painter().rect_filled(face, Radius::Cell.egui(), c(C::Tone).gamma_multiply(tokens::OFF));
                ui.painter().text(face.center(), egui::Align2::CENTER_CENTER, label, Type::Key.font(), c(C::Ink).gamma_multiply(tokens::OFF));
                continue;
            }
            let resp = ui.interact(r, id.with(("foot", k)), egui::Sense::click());
            let hot = crate::flow::pointer_on(ui, r);
            let ground = if hot && resp.is_pointer_button_down_on() { C::TonePress } else if hot { C::ToneHover } else { C::Tone };
            ui.painter().rect_filled(face, Radius::Cell.egui(), c(ground));
            ui.painter().text(face.center(), egui::Align2::CENTER_CENTER, label, Type::Key.font(), c(C::Ink));
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                picked = Some(if k == 0 { fmt(today) } else { String::new() });
            }
        }
        ui.allocate_rect(rect, egui::Sense::hover());
    });
    if next.level == Level::Days {
        ctx.data_mut(|d| d.insert_temp(view_id, (next.y, next.m)));
    }
    ctx.data_mut(|d| d.insert_temp(level_id, (opened, next)));
    crate::layer::keep(ctx, id.with("keep"), layer, tokens::FAST, 0.97, origin, drop.moved);
    // Esc closes this card only (the list or sheet it was opened from stays).
    let esc = crate::layer::esc(ctx, id);
    if picked.is_some() || guarded || esc {
        crate::menu::set(ctx, id, false);
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_arithmetic() {
        assert_eq!(days_in(2024, 2), 29);
        assert_eq!(days_in(2100, 2), 28);
        assert_eq!(days_in(2000, 2), 29);
        // 2026-10-01 is a Thursday (zero is Monday).
        assert_eq!(weekday(2026, 10, 1), 3);
        assert_eq!(weekday(2024, 1, 1), 0);
        assert_eq!(shift((2026, 12), 1), (2027, 1));
        assert_eq!(shift((2026, 1), -1), (2025, 12));
        let g = grid((2026, 10));
        assert_eq!(g[0], (2026, 9, 28));
        assert_eq!(g[3], (2026, 10, 1));
        assert_eq!(g.len(), 42);
        assert_eq!(parse("2026-02-29"), None);
        assert_eq!(parse("2024-02-29"), Some((2024, 2, 29)));
        assert_eq!(parse("2026-9-1"), None);
        assert_eq!(fmt((2026, 9, 1)), "2026-09-01");
    }

    /// The calendar's edges by its own rules: February in a leap year, a century year (1900 not, 2000 yes),
    /// every month's last day and the day after it, a month's last day read and the next month reached across
    /// a year, many months either way, the weekday on both sides of a century and of a year's turn, and a page
    /// that starts on a Monday (no days of the month before) and one that needs all six rows.
    #[test]
    fn calendar_edges() {
        for (y, feb) in [(2024, 29), (2023, 28), (1900, 28), (2000, 29), (2100, 28), (2400, 29), (1, 28), (4, 29)] {
            assert_eq!(days_in(y, 2), feb, "{y}");
        }
        for m in 1..=12u32 {
            let last = days_in(2026, m);
            assert_eq!(parse(&fmt((2026, m, last))), Some((2026, m, last)), "the last day of {m}");
            assert_eq!(parse(&fmt((2026, m, last + 1))), None, "the day after the last of {m}");
            assert_eq!(parse(&fmt((2026, m, 0))), None, "day zero of {m}");
        }
        assert_eq!(parse("2026-13-01"), None);
        assert_eq!(parse("2026-00-01"), None);
        assert_eq!(parse("1900-02-29"), None);
        assert_eq!(parse("2000-02-29"), Some((2000, 2, 29)));
        assert_eq!(parse(" 2026-10-06 "), Some((2026, 10, 6)), "white space around is taken");
        assert_eq!(parse("2026-10-06x"), None);
        assert_eq!(parse("+2026-10-06"), None);
        assert_eq!(shift((2026, 1), 1), (2026, 2));
        assert_eq!(shift((2026, 12), 13), (2028, 1));
        assert_eq!(shift((2026, 1), -13), (2024, 12));
        assert_eq!(shift((2026, 6), 0), (2026, 6));
        // Weekdays (zero is Monday) across a century and a year's turn.
        assert_eq!(weekday(1900, 1, 1), 0);
        assert_eq!(weekday(1899, 12, 31), 6);
        assert_eq!(weekday(2000, 2, 29), 1);
        assert_eq!(weekday(2000, 3, 1), 2);
        assert_eq!(weekday(2026, 12, 31), 3);
        assert_eq!(weekday(2027, 1, 1), 4);
        // June 2026 starts on a Monday: the page has no days of May.
        let june = grid((2026, 6));
        assert_eq!((june[0], june[29], june[30]), ((2026, 6, 1), (2026, 6, 30), (2026, 7, 1)));
        // August 2026 starts on a Saturday and has 31 days: it reaches the sixth row.
        let august = grid((2026, 8));
        assert_eq!((august[5], august[35], august[36], august[41]), ((2026, 8, 1), (2026, 8, 31), (2026, 9, 1), (2026, 9, 6)));
        // January's page takes its leading days from the year before.
        assert_eq!(grid((2027, 1))[0], (2026, 12, 28));
    }

    /// A bound leaves the days up to it, itself included, unpickable, across a month and a year; no bound leaves every
    /// day. Victim: whoever picks an end that is not after the start, signing a window of no length.
    #[test]
    fn a_bound_takes_the_days_up_to_it() {
        let at = Some((2026, 10, 3));
        assert!(!pickable((2026, 10, 3), at));
        assert!(!pickable((2026, 10, 2), at));
        assert!(!pickable((2026, 9, 30), at));
        assert!(!pickable((2025, 12, 31), at));
        assert!(pickable((2026, 10, 4), at));
        assert!(pickable((2026, 11, 1), at));
        assert!(pickable((2027, 1, 1), at));
        assert!(pickable((1, 1, 1), None));
    }
}
