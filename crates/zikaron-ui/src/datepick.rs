//! **The date field** (the "from · to" pair of a search row): one field in the single-line field's frame
//! (38 high, corner 8, 12 inside left and right) that opens a month calendar on a floating card.
//!
//! The card is drawn the way the menus are ([`crate::menu`]): a floating surface (menu corner, menu shadow),
//! 5 inside, growing from its anchor corner from 0.97 and rising 4 over 120 ms, played backwards when it
//! leaves; a press outside the card, Esc, or picking a day closes it. Opened inside a pick list or a sheet it
//! hangs above that overlay, and its Esc closes the calendar only ([`crate::layer`]).
//!
//! The calendar: a head row "‹ month ›"; under it one row of weekdays (the smallest type, ink three); six rows
//! of seven day cells (32 × 30, corner 6): light grey on hover, a step darker when pressed, the chosen day
//! solid accent with white figures, today framed by a 2-point accent line, days outside the month in ink
//! three (still pickable). Under a separator, one row of two menu rows, "today · clear" (the whole row
//! filled with the accent on hover, like a menu row).
//!
//! **This control only picks a day**, written `YYYY-MM-DD`. What counts as today, which zone the days are in
//! and how a list is filtered by them are the caller's.

use crate::icons::{self, Glyph};

use crate::paint;
use crate::palette::{self, c, Lift, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

/// The words the calendar says (the caller gives them in its language).
pub struct Words<'a> {
    /// Monday to Sunday.
    pub weekdays: [&'a str; 7],
    /// The month row's words for a year and a month (the year and the month, each followed by its word in
    /// the current language), written by the caller.
    pub month: fn(i32, u32) -> String,
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
    ui.painter().add(crate::input::frame_of(ui.ctx(), rect, id, open));
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
    match calendar(ui.ctx(), Some(ui.layer_id()), id, rect, value, today, words) {
        Some(v) if v != *value => {
            *value = v;
            true
        }
        _ => false,
    }
}

/// The open calendar card; returns the day picked (`Some("")` is clear).
fn calendar(ctx: &egui::Context, parent: Option<egui::LayerId>, id: egui::Id, anchor: Rect, value: &str, today: Day, words: &Words) -> Option<String> {
    if !crate::menu::is_open(ctx, id) {
        return None;
    }
    let e = crate::layer::entrance(ctx, &[id.with("keep")], id.with("age"), tokens::FAST);
    let view_id = id.with("view");
    let view = ctx.data(|d| d.get_temp::<(i32, u32)>(view_id)).unwrap_or((today.0, today.1));
    let w = CELL_W * 7.0 + PAD * 2.0 + 6.0;
    let h = PAD + HEAD_H + WEEK_H + CELL_H * 6.0 + SEP_H + ROW_H + PAD;
    let drop = crate::layer::drop_card(ctx, anchor, vec2(w, h), true);
    let (rect, origin) = (drop.rect, drop.origin);
    let layer = egui::LayerId::new(egui::Order::Foreground, id.with("menu"));
    let scale = 0.97 + 0.03 * e;
    ctx.set_transform_layer(layer, egui::emath::TSTransform { scaling: scale, translation: origin.to_vec2() * (1.0 - scale) + drop.moved * (1.0 - e) });
    let chosen = parse(value);
    let mut picked: Option<String> = None;
    let mut turn = 0;
    crate::layer::place(ctx, layer, parent);
    let mut guarded = false;
    crate::layer::over(ctx, layer).show(ctx, |ui| {
        // The guard over the whole window, then the card's body, in the card's own layer: a press outside the
        // card closes it and reaches nothing under it; a press on the card off its rows does nothing.
        guarded = crate::layer::under(ui, crate::layer::Under::Guard);
        crate::layer::body(ui, rect);
        ui.multiply_opacity(e);
        paint::surface(ui.painter(), rect, Radius::Menu, c(C::Surface), Lift::Menu);
        let left = rect.left() + PAD + 3.0;
        // Head row: ‹ month ›.
        let head = Rect::from_min_size(pos2(left, rect.top() + PAD), vec2(CELL_W * 7.0, HEAD_H));
        for (k, glyph, at) in [(-1, Glyph::Back, head.left()), (1, Glyph::Fwd, head.right() - CELL_W)] {
            let r = Rect::from_min_size(pos2(at, head.top() + (HEAD_H - CELL_H) / 2.0), vec2(CELL_W, CELL_H));
            let resp = ui.interact(r, id.with(("turn", k)), egui::Sense::click());
            if resp.hovered() {
                let press = resp.is_pointer_button_down_on();
                ui.painter().rect_filled(r, egui::CornerRadius::same(6), c(if press { C::Press } else { C::Hover }));
            }
            icons::glyph_at(ui.painter(), glyph, r.center(), 14.0, c(C::Ink2));
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                turn = k;
            }
        }
        let title = (words.month)(view.0, view.1);
        ui.painter().text(head.center(), egui::Align2::CENTER_CENTER, title, Type::Key.font(), c(C::Ink));
        // Weekday row.
        let wy = head.bottom() + WEEK_H / 2.0;
        for (i, wd) in words.weekdays.iter().enumerate() {
            ui.painter().text(pos2(left + CELL_W * (i as f32 + 0.5), wy), egui::Align2::CENTER_CENTER, *wd, Type::Micro.font(), c(C::Ink3));
        }
        // Six rows of seven days.
        let top = head.bottom() + WEEK_H;
        for (i, day) in grid(view).into_iter().enumerate() {
            let (col, row) = ((i % 7) as f32, (i / 7) as f32);
            let r = Rect::from_min_size(pos2(left + CELL_W * col, top + CELL_H * row), vec2(CELL_W, CELL_H)).shrink(1.0);
            let resp = ui.interact(r, id.with(("day", i)), egui::Sense::click());
            let on = chosen == Some(day);
            let here = (day.0, day.1) == view;
            if on {
                ui.painter().rect_filled(r, egui::CornerRadius::same(6), c(C::Accent));
            } else if resp.hovered() {
                let press = resp.is_pointer_button_down_on();
                ui.painter().rect_filled(r, egui::CornerRadius::same(6), c(if press { C::Press } else { C::Hover }));
            }
            if day == today && !on {
                ui.painter().rect_stroke(r, egui::CornerRadius::same(7), egui::Stroke::new(2.0_f32, c(C::Accent)), egui::StrokeKind::Inside);
            }
            let ink = if on { palette::ON_SOLID } else if here { c(C::Ink) } else { c(C::Ink3) };
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, day.2.to_string(), Type::Body.font(), ink);
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                picked = Some(fmt(day));
            }
        }
        // The separator and the foot row of two menu rows: today · clear.
        let sep_y = top + CELL_H * 6.0 + SEP_H / 2.0;
        ui.painter().hline((rect.left() + 11.0)..=(rect.right() - 11.0), sep_y, egui::Stroke::new(1.0_f32, c(C::Line2)));
        let foot_top = top + CELL_H * 6.0 + SEP_H;
        let half = (rect.width() - PAD * 2.0) / 2.0;
        for (k, label) in [(0, words.today), (1, words.clear)] {
            let r = Rect::from_min_size(pos2(rect.left() + PAD + half * k as f32, foot_top), vec2(half, ROW_H));
            let resp = ui.interact(r, id.with(("foot", k)), egui::Sense::click());
            let hot = resp.hovered();
            if hot {
                ui.painter().rect_filled(r, egui::CornerRadius::same(6), c(C::Accent));
            }
            ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, Type::Key.font(), if hot { palette::ON_SOLID } else { c(C::Ink) });
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                picked = Some(if k == 0 { fmt(today) } else { String::new() });
            }
        }
        ui.allocate_rect(rect, egui::Sense::hover());
    });
    if turn != 0 {
        ctx.data_mut(|d| d.insert_temp(view_id, shift(view, turn)));
    }
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
}
