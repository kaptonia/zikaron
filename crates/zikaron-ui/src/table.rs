//! Tables: a list card of single-line rows. Head 40, rows 56; the sequence column 36 right-aligned, the type
//! column as wide as this table's widest tag (at least 84, at most 148; a tag wider still is elided and shows
//! whole on hover), 12 between cells. Narrow, the body column keeps its floor while the fixed columns give way
//! first. One row at a time is lit: the row under the pointer this frame, at once, never two half lit; none
//! while the list scrolls (it lights again once the list stands still). A row that opens a detail shows a
//! chevron while lit, and the light under the pointer. Rules between rows hide around the lit row.
//!
//! Rows keep their identity from one frame to the next (an entry's sequence number, else its first words): when
//! the rows change in place (a filter, a refresh), rows that stay slide to their new places, new ones open and
//! fade in and gone ones close and fade out where they were, and the card's height follows
//! ([`crate::flow`]).

use crate::icons::{self, Glyph};
use crate::mark::{self, Mark};
use crate::motion;
use crate::paint;
use crate::palette::{c, Tone, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

/// A column width: fixed, a share of what the fixed columns leave, or as wide as the widest tag in it (between
/// `min` and `max`; measured each frame, then fixed like `Px`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Col {
    Px(f32),
    Fr(f32),
    Fit { min: f32, max: f32 },
}

/// The width of a column fitted to its widest tag's words: the words and the tag's padding, between `min` and
/// `max`.
pub fn fit_width(widest_words: f32, min: f32, max: f32) -> f32 {
    (widest_words + tokens::TAG_PAD).clamp(min, max.max(min))
}

/// A column: its head words, width, and whether its cells align right.
#[derive(Clone, Copy, Debug)]
pub struct Column<'a> {
    pub head: &'a str,
    pub width: Col,
    pub right: bool,
}

pub const fn col(head: &str, width: Col) -> Column<'_> {
    Column { head, width, right: false }
}

pub const fn col_r(head: &str, width: Col) -> Column<'_> {
    Column { head, width, right: true }
}

/// The standard columns: sequence (36), type tag (fitted, 84 to 148), a status mark (20), the chevron (16).
pub const SEQ: Column<'static> = Column { head: "", width: Col::Px(tokens::SEQ_W), right: true };
pub const TYPE: Column<'static> = Column { head: "", width: Col::Fit { min: tokens::TYPE_W, max: tokens::TYPE_MAX_W }, right: false };
pub const MARK: Column<'static> = Column { head: "", width: Col::Px(20.0), right: false };
pub const CHEV: Column<'static> = Column { head: "", width: Col::Px(16.0), right: true };

/// One cell.
#[derive(Clone, Debug)]
pub enum Cell {
    /// `#n`, monospace 13.
    Seq(u64),
    /// A type tag.
    Tag(String),
    /// Body words (elided at the end).
    Text(String),
    /// Small monospace in the third ink (times, numbers).
    Mono(String),
    /// A status pill.
    Pill(String, Tone),
    /// A status pill with a spinner.
    PillLive(String, Tone),
    /// A status mark.
    Mark(Mark),
    /// The chevron of a row that opens something.
    Chev,
    /// A switch (on, enabled): pick lists inside a table.
    Switch(bool, bool),
    Empty,
}

/// One row.
#[derive(Clone, Debug, Default)]
pub struct Row {
    pub cells: Vec<Cell>,
    /// The row opens something: hover ground, hand cursor, chevron.
    pub click: bool,
    /// The row's thing is gone (a deleted record): struck through in the quiet ink.
    pub gone: bool,
    /// Selected.
    pub on: bool,
}

/// What a table did this frame.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hit {
    pub clicked: Option<usize>,
}

/// The columns' widths in `inner` (fitted columns already resolved to `Px`; an unresolved one counts as its
/// `min`). The fixed columns keep their widths while the share columns keep [`tokens::BODY_MIN_W`] between them;
/// narrower, the fixed columns give way first, together (their words elide), down to half; narrower still the
/// share columns go below their floor; and when even half the fixed columns do not fit, they all narrow
/// together to the edge.
pub fn widths(cols: &[Column], inner: f32) -> Vec<f32> {
    let gaps = tokens::CELL_GAP * (cols.len().saturating_sub(1)) as f32;
    let px = |c: &Column| match c.width {
        Col::Px(w) => w,
        Col::Fit { min, .. } => min,
        Col::Fr(_) => 0.0,
    };
    let fixed: f32 = cols.iter().map(px).sum();
    let frs: f32 = cols.iter().map(|c| if let Col::Fr(f) = c.width { f } else { 0.0 }).sum();
    let floor = if frs > 0.0 { tokens::BODY_MIN_W } else { 0.0 };
    let room = (inner - gaps).max(0.0);
    let squeeze = if fixed <= 0.0 || fixed + floor <= room {
        1.0
    } else if fixed * 0.5 + floor <= room {
        (room - floor) / fixed
    } else if fixed * 0.5 <= room {
        0.5
    } else {
        room / fixed
    };
    let rest = (room - fixed * squeeze).max(0.0);
    cols.iter()
        .map(|c| match c.width {
            Col::Fr(f) => if frs > 0.0 { rest * f / frs } else { 0.0 },
            _ => px(c) * squeeze,
        })
        .collect()
}

/// A lit row's block ([`crate::band`]): across, the card's edges; above, the head's rule, the line over the row
/// or the card's top; below, the line under it or the card's bottom (`last`, the card ending `LIST_PAD_Y` under).
/// Answers the block, its corners and the slot it was cut from.
pub fn lit_block(card_x: (f32, f32), row: Rect, head_rule: Option<f32>, first: bool, last: bool) -> (Rect, egui::CornerRadius, Rect) {
    use crate::band::Bound;
    let (top, above) = match (first, head_rule) {
        // The head's rule is drawn centred half a point above the head's foot (`y`): the slot starts at its middle,
        // as it does at a line between rows.
        (true, Some(y)) => (y - crate::band::LINE_HALF, Bound::Line),
        (true, None) => (row.top() - tokens::LIST_PAD_Y, Bound::Edge),
        (false, _) => (row.top(), Bound::Line),
    };
    let (bottom, below) = if last { (row.bottom() + tokens::LIST_PAD_Y, Bound::Edge) } else { (row.bottom(), Bound::Line) };
    let slot = Rect::from_min_max(pos2(card_x.0, top), pos2(card_x.1, bottom));
    let (block, corners) = crate::band::block(slot, Radius::Card.px(), above, below);
    (block, corners, slot)
}

/// Each row's identity in this table, one per row: its sequence number when it has one, else its first words (a
/// row whose other cells change, a date landing, is the same row); rows alike in that are told apart by their order
/// among their likes, so no two rows of a table share one.
fn row_keys(rows: &[Row]) -> Vec<u64> {
    let base = |row: &Row| -> u64 {
        if let Some(n) = row.cells.iter().find_map(|c| if let Cell::Seq(n) = c { Some(*n) } else { None }) {
            return motion::key_of(&("seq", n));
        }
        let first = row.cells.iter().find_map(|c| match c {
            Cell::Text(s) | Cell::Mono(s) | Cell::Tag(s) => Some(s.as_str()),
            _ => None,
        });
        motion::key_of(&("words", first))
    };
    let mut seen: Vec<u64> = Vec::new();
    rows.iter()
        .map(|row| {
            let b = base(row);
            let nth = seen.iter().filter(|x| **x == b).count();
            seen.push(b);
            if nth == 0 { b } else { motion::key_of(&("again", b, nth)) }
        })
        .collect()
}

/// A list card with an optional head, the rows, and an empty line when there are none.
pub fn table(ui: &mut egui::Ui, id_salt: &str, cols: &[Column], head: bool, rows: &[Row], empty: &str) -> Hit {
    let id = ui.id().with(("zikaron-table", id_salt));
    let slot = ui.painter().add(egui::Shape::Noop);
    let (hit, shown) = crate::flow::grow(ui, id, |ui| table_body(ui, id, cols, head, rows, empty));
    ui.painter().set(slot, egui::Shape::Vec(crate::material::shapes(shown, Radius::Card, crate::material::Level::Card, 0.0)));
    hit
}

fn table_body(ui: &mut egui::Ui, id: egui::Id, cols: &[Column], head: bool, rows: &[Row], empty: &str) -> (Hit, Rect) {
    let w = ui.available_width();
    let top = ui.cursor().min;
    let mut hit = Hit::default();
    let inner_x = top.x + tokens::LIST_PAD_X;
    let inner_w = w - tokens::LIST_PAD_X * 2.0;
    let cell_room = inner_w - tokens::ROW_INSET * 2.0;
    // A fitted column takes this table's widest tag in it.
    let fitted: Vec<Column> = cols
        .iter()
        .enumerate()
        .map(|(i, col)| match col.width {
            Col::Fit { min, max } => {
                let widest = rows.iter().filter_map(|r| match r.cells.get(i) {
                    Some(Cell::Tag(s)) => Some(paint::galley(ui, s, Type::Small, c(C::Ink2)).size().x),
                    _ => None,
                });
                Column { width: Col::Px(fit_width(widest.fold(0.0, f32::max), min, max)), ..*col }
            }
            _ => *col,
        })
        .collect();
    let cols = &fitted[..];
    let ws = widths(cols, cell_room);
    let mut y = top.y + tokens::LIST_PAD_Y;
    let mut head_rule = y;
    let mut head_line = None;
    let p = ui.painter().clone();
    if head {
        // The head spans the card edge to edge, with the rule under it (drawn after the rows: the first row's lit
        // edge lies under it).
        let hr = Rect::from_min_size(pos2(top.x, top.y), vec2(w, tokens::TABLE_HEAD_H));
        let mut x = inner_x + tokens::ROW_INSET;
        for (col, cw) in cols.iter().zip(&ws) {
            if !col.head.is_empty() {
                let (ax, al) = if col.right { (x + cw, egui::Align2::RIGHT_CENTER) } else { (x, egui::Align2::LEFT_CENTER) };
                paint::at(&p, ui, pos2(ax, hr.center().y), al, col.head, Type::Small, c(C::Ink2), *cw);
            }
            x += cw + tokens::CELL_GAP;
        }
        head_line = Some(egui::Shape::hline(hr.x_range(), hr.bottom() - crate::band::LINE_HALF, egui::Stroke::new(1.0_f32, c(C::Line2))));
        head_rule = hr.bottom();
        y = hr.bottom();
    }
    // The one lit row: under the pointer this frame, with nothing scrolling (rules around it hide).
    let rects: Vec<Rect> = (0..rows.len()).map(|i| Rect::from_min_size(pos2(inner_x, y + tokens::ROW_H * i as f32), vec2(inner_w, tokens::ROW_H))).collect();
    let hovered = rects.iter().position(|r| crate::flow::pointer_on(ui, *r));
    let mut group = crate::flow::Group::new(ui, id, top, w);
    if rows.is_empty() {
        let r = Rect::from_min_size(pos2(inner_x, y), vec2(inner_w, 96.0));
        p.text(r.center(), egui::Align2::CENTER_CENTER, empty, Type::Body.font(), c(C::Ink3));
        y = r.bottom();
    } else {
        let keys = row_keys(rows);
        for (i, row) in rows.iter().enumerate() {
            let r = rects[i];
            let start = group.start(ui.ctx());
            let sense = if row.click { egui::Sense::click() } else { egui::Sense::hover() };
            let rid = id.with(("row", i));
            let resp = ui.interact(r, rid.with("hit"), sense);
            let hot = if hovered == Some(i) { 1.0 } else { 0.0 };
            let down = row.click && hot > 0.0 && resp.is_pointer_button_down_on();
            // Chosen rows that touch are one block, drawn once, from the run's first row to its last.
            let run = row.on.then(|| {
                let first = (0..=i).rev().take_while(|k| rows[*k].on).last().unwrap_or(i);
                let last = (i..rows.len()).take_while(|k| rows[*k].on).last().unwrap_or(i);
                (first, last)
            });
            let ground = if let Some((first, _)) = run {
                (first == i).then(|| c(C::Sel))
            } else if down {
                Some(c(C::Press))
            } else if hot > 0.0 {
                Some(c(C::RowHover))
            } else {
                None
            };
            let (band, corners, slot) = match run {
                Some((first, last)) => lit_block((top.x, top.x + w), rects[first].union(rects[last]), head.then_some(head_rule), first == 0, last + 1 == rows.len()),
                None => lit_block((top.x, top.x + w), r, head.then_some(head_rule), i == 0, i + 1 == rows.len()),
            };
            if let Some(g) = ground {
                p.rect_filled(band, corners, g);
                let card = Rect::from_min_max(top, pos2(top.x + w, rects.last().map(|l| l.bottom()).unwrap_or(y) + tokens::LIST_PAD_Y));
                crate::probe::lit(ui.ctx(), crate::probe::Lit { block: band, corners, slot, card, card_corner: Radius::Card.px() });
            }
            if row.click {
                let (glow, at) = crate::material::glow_at(ui, rid, r, hot > 0.0);
                if glow > 0.0 {
                    if let Some(s) = crate::material::glow_shape_corners(band, corners, at, c(C::Accent).gamma_multiply(0.04 * glow)) {
                        p.add(s);
                    }
                }
            }
            // The line between two rows stays when either is lit, drawn over the lit block, whose edge lies on its
            // middle ([`crate::band`]); two chosen rows in one block have none between them.
            if i > 0 && !(row.on && rows[i - 1].on) {
                p.hline((r.left() + tokens::ROW_INSET)..=(r.right() - tokens::ROW_INSET), r.top(), egui::Stroke::new(1.0_f32, c(C::Line2).gamma_multiply(0.7)));
            }
            let mut x = r.left() + tokens::ROW_INSET;
            let cy = r.center().y;
            for ((cell, cw), col) in row.cells.iter().zip(&ws).zip(cols) {
                paint_cell(ui, &p, rid, cell, (x, cy), *cw, col.right, (row.gone, hot, row.click));
                x += cw + tokens::CELL_GAP;
            }
            if row.click {
                crate::probe::row(ui.ctx(), r);
                if resp.clicked() {
                    hit.clicked = Some(i);
                }
                let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            }
            group.row(ui.ctx(), keys[i], r, start);
        }
        y = rects.last().map(|r| r.bottom()).unwrap_or(y);
    }
    let _ = group.finish(ui.ctx());
    if let Some(line) = head_line {
        p.add(line);
    }
    let card = Rect::from_min_max(top, pos2(top.x + w, y + tokens::LIST_PAD_Y));
    ui.advance_cursor_after_rect(card);
    (hit, card)
}

#[allow(clippy::too_many_arguments)]
fn paint_cell(ui: &mut egui::Ui, p: &egui::Painter, id: egui::Id, cell: &Cell, at: (f32, f32), w: f32, right: bool, row: (bool, f32, bool)) {
    let (x, cy) = at;
    let (gone, hot, click) = row;
    match cell {
        Cell::Seq(n) => {
            p.text(pos2(x + w, cy), egui::Align2::RIGHT_CENTER, format!("#{n}"), Type::MonoSmall.font(), c(C::Ink3));
        }
        Cell::Tag(s) => {
            // A tag as wide as its words (the column, as wide as the widest tag, keeps the cells after it in line);
            // words wider than the column are elided and show whole on hover.
            let fit = crate::width::elide_to(ui, s, Type::Small.font(), (w - tokens::TAG_PAD).max(0.0));
            let g = paint::galley(ui, &fit, Type::Small, c(C::Ink2));
            let tw = (g.size().x + tokens::TAG_PAD).min(w);
            let rect = Rect::from_min_size(pos2(x, cy - tokens::TAG_H / 2.0), vec2(tw, tokens::TAG_H));
            mark::paint_tag(p, rect, g);
            if fit != *s {
                let _ = crate::layer::tip(ui.interact(rect, id.with(("tag-whole", s.as_str())), egui::Sense::hover()), s.as_str());
            }
        }
        Cell::Text(s) => {
            let colour = if gone { c(C::Ink3) } else { c(C::Ink) };
            let r = paint::at(p, ui, pos2(x, cy), egui::Align2::LEFT_CENTER, s, Type::Body, colour, w);
            if gone {
                p.hline(r.x_range(), cy, egui::Stroke::new(1.0_f32, c(C::Ink3)));
            }
        }
        Cell::Mono(s) => {
            let (ax, al) = if right { (x + w, egui::Align2::RIGHT_CENTER) } else { (x, egui::Align2::LEFT_CENTER) };
            paint::at(p, ui, pos2(ax, cy), al, s, Type::MonoSmall, c(C::Ink3), w);
        }
        Cell::Switch(on, enabled) => {
            let r = Rect::from_min_size(pos2(x, cy - tokens::SWITCH_H / 2.0), vec2(tokens::SWITCH_W, tokens::SWITCH_H));
            crate::toggle::paint_switch(ui, r, id.with("switch"), *on, *enabled);
        }
        Cell::Pill(s, tone) | Cell::PillLive(s, tone) => {
            let live = matches!(cell, Cell::PillLive(..));
            let pw = mark::pill_w(ui, s, live).min(w);
            let x0 = if right { x + w - pw } else { x };
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(x0, cy - tokens::PILL_H / 2.0), vec2(pw, tokens::PILL_H))));
            if live {
                mark::pill_live(&mut child, s, *tone);
            } else {
                mark::pill(&mut child, s, *tone);
            }
        }
        Cell::Mark(m) => mark::paint_mark(ui.ctx(), p, pos2(x + 9.0, cy), *m, tokens::MARK),
        Cell::Chev => {
            if click && hot > 0.0 {
                icons::glyph_at(p, Glyph::Chev, pos2(x + w - 6.0 + 2.0 * hot, cy), 12.0, c(C::Ink3).gamma_multiply(hot));
            }
        }
        Cell::Empty => {}
    }
}

/// A pick list inside a sheet: rows 40 high (a switch or nothing first), a selected ground, a disabled look.
pub fn pick_row(ui: &mut egui::Ui, id: egui::Id, on: bool, enabled: bool, h: f32, add: impl FnOnce(&mut egui::Ui)) -> egui::Response {
    let w = ui.available_width();
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    // Lit at once while under the pointer this frame (a list lights one row), never while it scrolls.
    let hot = enabled && crate::flow::pointer_on(ui, rect);
    let _ = id;
    if on {
        ui.painter().rect_filled(rect, Radius::Field.egui(), c(C::Sel));
    } else if hot {
        ui.painter().rect_filled(rect, Radius::Field.egui(), c(C::RowHover));
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(10.0, 0.0))).layout(egui::Layout::left_to_right(egui::Align::Center)));
    if !enabled {
        child.multiply_opacity(0.55);
    }
    child.spacing_mut().item_spacing.x = 10.0;
    add(&mut child);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The type column takes its widest tag's words and padding, at least 84, at most 148.
    #[test]
    fn the_type_column_fits_its_widest_tag_between_its_bounds() {
        assert_eq!(fit_width(30.0, tokens::TYPE_W, tokens::TYPE_MAX_W), tokens::TYPE_W, "short words: the floor");
        assert_eq!(fit_width(100.0, tokens::TYPE_W, tokens::TYPE_MAX_W), 100.0 + tokens::TAG_PAD, "longer words widen it");
        assert_eq!(fit_width(400.0, tokens::TYPE_W, tokens::TYPE_MAX_W), tokens::TYPE_MAX_W, "the ceiling; the words elide");
        assert_eq!(fit_width(0.0, 84.0, 10.0), 84.0, "a ceiling under the floor is the floor");
    }

    fn list() -> Vec<Column<'static>> {
        vec![SEQ, col("", Col::Px(84.0)), col("", Col::Fr(1.0)), col("", Col::Px(180.0)), MARK, CHEV]
    }

    /// Wide, every fixed column keeps its width and the body takes the rest; narrower, the body keeps its floor
    /// while the fixed columns give way (down to half); narrower still the body goes under its floor; past half
    /// the fixed columns, everything narrows to the edge. Never past the edge.
    #[test]
    fn narrow_the_fixed_columns_give_way_before_the_body() {
        let cols = list();
        let gaps = tokens::CELL_GAP * 5.0;
        let fixed = 36.0 + 84.0 + 180.0 + 20.0 + 16.0;
        let sum = |w: &[f32]| w.iter().sum::<f32>() + gaps;
        let wide = widths(&cols, 900.0);
        assert_eq!((wide[0], wide[1], wide[3]), (36.0, 84.0, 180.0));
        assert!((wide[2] - (900.0 - gaps - fixed)).abs() < 0.01);
        let at_floor = fixed + tokens::BODY_MIN_W + gaps;
        let just_under = widths(&cols, at_floor - 40.0);
        assert!((just_under[2] - tokens::BODY_MIN_W).abs() < 0.01, "the body keeps its floor: {just_under:?}");
        assert!(just_under[3] < 180.0 && just_under[1] < 84.0, "the fixed columns gave way");
        assert!((sum(&just_under) - (at_floor - 40.0)).abs() < 0.01);
        let half = fixed * 0.5 + gaps;
        let under_half_floor = widths(&cols, half + 60.0);
        assert!((under_half_floor[3] - 90.0).abs() < 0.01, "fixed at half: {under_half_floor:?}");
        assert!((under_half_floor[2] - 60.0).abs() < 0.01, "the body below its floor only now");
        let tiny = widths(&cols, gaps + 50.0);
        assert!(tiny[2] == 0.0 && (sum(&tiny) - (gaps + 50.0)).abs() < 0.01, "to the edge: {tiny:?}");
        // No share column: the fixed columns narrow together to the edge.
        let only_fixed = widths(&[SEQ, col("", Col::Px(200.0))], 100.0 + tokens::CELL_GAP);
        assert!((only_fixed.iter().sum::<f32>() - 100.0).abs() < 0.01);
        // A fitted column not yet measured counts as its floor.
        assert_eq!(widths(&[TYPE, col("", Col::Fr(1.0))], 1000.0)[0], tokens::TYPE_W);
    }
}
