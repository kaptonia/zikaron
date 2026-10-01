//! Tables: a list card of single-line rows. Head 40, rows 56; the sequence column 36 right-aligned, the type
//! column 84, 12 between cells. Rows darken on hover; a row that opens a detail shows a chevron on hover that
//! slides 2 to the right. Rules between rows hide around the hovered row.

use crate::icons::{self, Glyph};
use crate::mark::{self, Mark};
use crate::motion;
use crate::paint;
use crate::palette::{c, Lift, Tone, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

/// A column width: fixed, or a share of what the fixed columns leave.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Col {
    Px(f32),
    Fr(f32),
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

/// The standard columns: sequence (36), type tag (84), a status mark (20), the chevron (16).
pub const SEQ: Column<'static> = Column { head: "", width: Col::Px(tokens::SEQ_W), right: true };
pub const TYPE: Column<'static> = Column { head: "", width: Col::Px(tokens::TYPE_W), right: false };
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

fn widths(cols: &[Column], inner: f32) -> Vec<f32> {
    let gaps = tokens::CELL_GAP * (cols.len().saturating_sub(1)) as f32;
    let fixed: f32 = cols.iter().map(|c| if let Col::Px(w) = c.width { w } else { 0.0 }).sum();
    let frs: f32 = cols.iter().map(|c| if let Col::Fr(f) = c.width { f } else { 0.0 }).sum();
    let rest = (inner - fixed - gaps).max(0.0);
    // Narrower than the fixed columns: they narrow together (their words elide) rather than pass the edge.
    let squeeze = if fixed > 0.0 && fixed + gaps > inner { ((inner - gaps).max(0.0) / fixed).min(1.0) } else { 1.0 };
    cols.iter()
        .map(|c| match c.width {
            Col::Px(w) => w * squeeze,
            Col::Fr(f) => if frs > 0.0 { rest * f / frs } else { 0.0 },
        })
        .collect()
}

/// A list card with an optional head, the rows, and an empty line when there are none.
pub fn table(ui: &mut egui::Ui, id_salt: &str, cols: &[Column], head: bool, rows: &[Row], empty: &str) -> Hit {
    let id = ui.id().with(("zikaron-table", id_salt));
    let w = ui.available_width();
    let slot = ui.painter().add(egui::Shape::Noop);
    let top = ui.cursor().min;
    let mut hit = Hit::default();
    let inner_x = top.x + tokens::LIST_PAD_X;
    let inner_w = w - tokens::LIST_PAD_X * 2.0;
    let cell_room = inner_w - tokens::ROW_INSET * 2.0;
    let ws = widths(cols, cell_room);
    let mut y = top.y + tokens::LIST_PAD_Y;
    let p = ui.painter().clone();
    if head {
        // The head spans the card edge to edge, with the rule under it.
        let hr = Rect::from_min_size(pos2(top.x, top.y), vec2(w, tokens::TABLE_HEAD_H));
        let mut x = inner_x + tokens::ROW_INSET;
        for (col, cw) in cols.iter().zip(&ws) {
            if !col.head.is_empty() {
                let (ax, al) = if col.right { (x + cw, egui::Align2::RIGHT_CENTER) } else { (x, egui::Align2::LEFT_CENTER) };
                paint::at(&p, ui, pos2(ax, hr.center().y), al, col.head, Type::Small, c(C::Ink3), *cw);
            }
            x += cw + tokens::CELL_GAP;
        }
        p.hline(hr.x_range(), hr.bottom() - 0.5, egui::Stroke::new(1.0_f32, c(C::Line)));
        y = hr.bottom();
    }
    // Which row is hovered, from the pointer (rules around it hide).
    let pointer = ui.ctx().pointer_hover_pos();
    let rects: Vec<Rect> = (0..rows.len()).map(|i| Rect::from_min_size(pos2(inner_x, y + tokens::ROW_H * i as f32), vec2(inner_w, tokens::ROW_H))).collect();
    let hovered = pointer.and_then(|pt| rects.iter().position(|r| r.contains(pt)));
    if rows.is_empty() {
        let r = Rect::from_min_size(pos2(inner_x, y), vec2(inner_w, 96.0));
        p.text(r.center(), egui::Align2::CENTER_CENTER, empty, Type::Body.font(), c(C::Ink3));
        y = r.bottom();
    } else {
        for (i, row) in rows.iter().enumerate() {
            let r = rects[i];
            let sense = if row.click { egui::Sense::click() } else { egui::Sense::hover() };
            let rid = id.with(("row", i));
            let resp = ui.interact(r, rid.with("hit"), sense);
            let hot = motion::flag(ui.ctx(), rid, resp.hovered(), tokens::FAST);
            let down = row.click && resp.is_pointer_button_down_on();
            let ground = if row.on {
                Some(c(C::Sel))
            } else if down {
                Some(c(C::Press))
            } else if hot > 0.0 {
                Some(c(C::Hover).gamma_multiply(hot))
            } else {
                None
            };
            if let Some(g) = ground {
                p.rect_filled(r, Radius::Ctl.egui(), g);
            }
            if i > 0 && hovered != Some(i) && hovered != Some(i - 1) && !row.on && !rows[i - 1].on {
                p.hline((r.left() + tokens::ROW_INSET)..=(r.right() - tokens::ROW_INSET), r.top(), egui::Stroke::new(1.0_f32, c(C::Line2)));
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
        }
        y = rects.last().map(|r| r.bottom()).unwrap_or(y);
    }
    let card = Rect::from_min_max(top, pos2(top.x + w, y + tokens::LIST_PAD_Y));
    ui.advance_cursor_after_rect(card);
    let mut shapes = Vec::new();
    crate::card::raised_shapes(&p, card, Radius::Card, c(C::Surface), Lift::Card, &mut shapes);
    ui.painter().set(slot, egui::Shape::Vec(shapes));
    hit
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
            let g = paint::galley(ui, s, Type::Small, c(C::Ink2));
            // A tag fills the type column, so the tags down a list line up; longer words widen it.
            let tw = (g.size().x + 16.0).max(tokens::TAG_MIN_W).max(w.min(tokens::TYPE_W)).min(w);
            mark::paint_tag(p, Rect::from_min_size(pos2(x, cy - tokens::TAG_H / 2.0), vec2(tw, tokens::TAG_H)), g);
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
    let hot = motion::flag(ui.ctx(), id, enabled && resp.hovered(), tokens::FAST);
    if on {
        ui.painter().rect_filled(rect, Radius::Ctl.egui(), c(C::Sel));
    } else if hot > 0.0 {
        ui.painter().rect_filled(rect, Radius::Ctl.egui(), c(C::Hover).gamma_multiply(hot));
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
