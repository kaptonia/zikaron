use crate::icons::{self, Glyph};
use crate::mark::{self, Mark};
use crate::motion;
use crate::paint;
use crate::palette::{c, Tone, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};
use std::hash::{Hash, Hasher};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Col {
    Px(f32),
    Fr(f32),
    Fit { min: f32, max: f32 },
    Auto { min: f32, max: f32 },
}

pub fn fit_width(widest_words: f32, min: f32, max: f32) -> f32 {
    (widest_words + tokens::TAG_PAD).clamp(min, max.max(min))
}

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

pub const SEQ: Column<'static> = Column { head: "", width: Col::Px(tokens::SEQ_W), right: true };
pub const TYPE: Column<'static> = Column { head: "", width: Col::Fit { min: tokens::TYPE_W, max: tokens::TYPE_MAX_W }, right: false };
pub const MARK: Column<'static> = Column { head: "", width: Col::Px(20.0), right: false };
pub const CHEV: Column<'static> = Column { head: "", width: Col::Px(16.0), right: true };

pub const AUTO_MIN: f32 = 56.0;
pub const ROW_TWO_H: f32 = 60.0;
const KEY_PAD: f32 = 32.0;
const KEY_H: f32 = 32.0;
const SHORT_HEAD: usize = 4;

#[derive(Clone, Debug)]
pub enum Cell {
    Seq(u64),
    Tag(String),
    Text(String),
    Mono(String),
    Amount(crate::layer::Amount),
    Pill(String, Tone),
    PillLive(String, Tone),
    Mark(Mark),
    Chev,
    Switch(bool, bool),
    Two(String, String),
    Lamp(Mark, String),
    Key(String),
    Note(String),
    Small(String),
    Short(String),
    Empty,
}

impl Cell {
    fn fixed(&self) -> bool {
        matches!(self, Cell::Seq(_) | Cell::Tag(_) | Cell::Mono(_) | Cell::Amount(_) | Cell::Pill(..) | Cell::PillLive(..) | Cell::Mark(_) | Cell::Chev | Cell::Switch(..) | Cell::Lamp(..) | Cell::Key(_) | Cell::Short(_))
    }

    fn words(&self) -> bool {
        matches!(self, Cell::Text(_) | Cell::Two(..))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Row {
    pub cells: Vec<Cell>,
    pub click: bool,
    pub gone: bool,
    pub on: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Hit {
    pub clicked: Option<usize>,
    pub key: Option<(usize, usize)>,
    pub head: Option<usize>,
}

#[derive(Clone, Copy, Debug)]
pub struct Spec<'a> {
    pub cols: &'a [Column<'a>],
    pub head: bool,
    pub empty: &'a str,
    pub generation: u64,
    pub wide: bool,
    pub sortable: &'a [usize],
    pub sorted: Option<usize>,
}

impl<'a> Spec<'a> {
    pub fn new(cols: &'a [Column<'a>], generation: u64) -> Spec<'a> {
        Spec { cols, head: false, empty: "", generation, wide: false, sortable: &[], sorted: None }
    }

    pub fn head(mut self, head: bool) -> Spec<'a> {
        self.head = head;
        self
    }

    pub fn empty(mut self, empty: &'a str) -> Spec<'a> {
        self.empty = empty;
        self
    }

    pub fn wide(mut self) -> Spec<'a> {
        self.wide = true;
        self
    }

    pub fn sort(mut self, sortable: &'a [usize], sorted: Option<usize>) -> Spec<'a> {
        self.sortable = sortable;
        self.sorted = sorted;
        self
    }
}

pub fn widths(cols: &[Column], inner: f32) -> Vec<f32> {
    widths_kept(cols, inner, &[])
}

pub fn widths_kept(cols: &[Column], inner: f32, keep: &[f32]) -> Vec<f32> {
    let gaps = tokens::CELL_GAP * (cols.len().saturating_sub(1)) as f32;
    let px = |c: &Column| match c.width {
        Col::Px(w) => w,
        Col::Fit { min, .. } | Col::Auto { min, .. } => min,
        Col::Fr(_) => 0.0,
    };
    let kept = |i: usize, c: &Column| keep.get(i).copied().unwrap_or(0.0).clamp(0.0, px(c));
    let fixed: f32 = cols.iter().map(px).sum();
    let firm: f32 = cols.iter().enumerate().map(|(i, c)| kept(i, c)).sum();
    let frs: f32 = cols.iter().map(|c| if let Col::Fr(f) = c.width { f } else { 0.0 }).sum();
    let floor = if frs > 0.0 { tokens::BODY_MIN_W } else { 0.0 };
    let room = (inner - gaps).max(0.0);
    let give = fixed - firm;
    let (beyond, s, rest) = if fixed + floor <= room {
        (true, 1.0, room - fixed)
    } else if firm + floor <= room && give > 0.0 {
        (true, (room - floor - firm) / give, floor)
    } else if firm <= room {
        (true, 0.0, room - firm)
    } else {
        (false, if firm > 0.0 { room / firm } else { 0.0 }, 0.0)
    };
    cols.iter()
        .enumerate()
        .map(|(i, c)| match c.width {
            Col::Fr(f) => if frs > 0.0 { rest.max(0.0) * f / frs } else { 0.0 },
            _ if beyond => kept(i, c) + (px(c) - kept(i, c)) * s,
            _ => kept(i, c) * s,
        })
        .collect()
}

pub fn lit_block(card_x: (f32, f32), row: Rect, head_rule: Option<f32>, first: bool, last: bool) -> (Rect, egui::CornerRadius, Rect) {
    use crate::band::Bound;
    let (top, above) = match (first, head_rule) {
        (true, Some(y)) => (y - crate::band::LINE_HALF, Bound::Rule),
        (true, None) => (row.top() - tokens::LIST_PAD_Y, Bound::Edge),
        (false, _) => (row.top(), Bound::Line),
    };
    let (bottom, below) = if last { (row.bottom() + tokens::LIST_PAD_Y, Bound::Edge) } else { (row.bottom(), Bound::Line) };
    let slot = Rect::from_min_max(pos2(card_x.0, top), pos2(card_x.1, bottom));
    let (block, corners) = crate::band::block(slot, Radius::Card.px(), above, below);
    (block, corners, slot)
}

fn base_key(row: &Row) -> u64 {
    if let Some(n) = row.cells.iter().find_map(|c| if let Cell::Seq(n) = c { Some(*n) } else { None }) {
        return motion::key_of(&("seq", n));
    }
    let first = row.cells.iter().find_map(|c| match c {
        Cell::Text(s) | Cell::Mono(s) | Cell::Tag(s) | Cell::Note(s) | Cell::Short(s) => Some(s.as_str()),
        Cell::Two(s, _) => Some(s.as_str()),
        _ => None,
    });
    motion::key_of(&("words", first))
}

fn row_keys(rows: &[Row]) -> Vec<u64> {
    let mut seen: Vec<u64> = Vec::new();
    rows.iter()
        .map(|row| {
            let b = base_key(row);
            let nth = seen.iter().filter(|x| **x == b).count();
            seen.push(b);
            if nth == 0 { b } else { motion::key_of(&("again", b, nth)) }
        })
        .collect()
}

fn rows_gen(rows: &[Row]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    rows.len().hash(&mut h);
    for r in rows {
        (r.click, r.gone, r.on).hash(&mut h);
        for c in &r.cells {
            std::mem::discriminant(c).hash(&mut h);
            match c {
                Cell::Seq(n) => n.hash(&mut h),
                Cell::Tag(s) | Cell::Text(s) | Cell::Mono(s) | Cell::Key(s) | Cell::Note(s) | Cell::Small(s) | Cell::Short(s) => s.hash(&mut h),
                Cell::Amount(a) => (&a.shown, &a.whole).hash(&mut h),
                Cell::Pill(s, t) | Cell::PillLive(s, t) => (s, std::mem::discriminant(t)).hash(&mut h),
                Cell::Mark(m) => m.hash(&mut h),
                Cell::Switch(a, b) => (a, b).hash(&mut h),
                Cell::Two(a, b) => (a, b).hash(&mut h),
                Cell::Lamp(m, s) => (m, s).hash(&mut h),
                Cell::Chev | Cell::Empty => {}
            }
        }
    }
    h.finish()
}

pub fn table(ui: &mut egui::Ui, id_salt: &str, cols: &[Column], head: bool, rows: &[Row], empty: &str) -> Hit {
    table_with(ui, id_salt, Spec::new(cols, rows_gen(rows)).head(head).empty(empty), rows.len(), |i| rows[i].clone())
}

#[derive(Clone, Debug)]
struct Measured {
    key: u64,
    cols: Vec<Col>,
    keep: Vec<f32>,
    widest: Vec<f32>,
    row_h: f32,
}

fn ink(ui: &egui::Ui, cell: &Cell) -> f32 {
    let w = |s: &str, t: Type| paint::galley(ui, s, t, c(C::Ink)).size().x;
    match cell {
        Cell::Seq(n) => w(&format!("#{n}"), Type::MonoSmall),
        Cell::Tag(s) => w(s, Type::Small) + tokens::TAG_PAD,
        Cell::Text(s) | Cell::Short(s) => w(s, Type::Body),
        Cell::Mono(s) => w(s, Type::MonoSmall),
        Cell::Amount(a) => w(&a.shown, Type::MonoSmall),
        Cell::Pill(s, _) => mark::pill_w(ui, s, false),
        Cell::PillLive(s, _) => mark::pill_w(ui, s, true),
        Cell::Mark(_) => tokens::MARK,
        Cell::Chev => 12.0,
        Cell::Switch(..) => tokens::SWITCH_W,
        Cell::Two(a, b) => w(a, Type::Body).max(w(b, Type::Small)),
        Cell::Lamp(_, s) => tokens::MARK + tokens::S2 + w(s, Type::Body),
        Cell::Key(s) => w(s, Type::Body) + KEY_PAD,
        Cell::Note(s) => w(s, Type::Note),
        Cell::Small(s) => w(s, Type::Small),
        Cell::Empty => 0.0,
    }
}

fn measure(ui: &egui::Ui, cols: &[Column], n: usize, row: &mut dyn FnMut(usize) -> Row, key: u64) -> Measured {
    let k = cols.len();
    let mut tag = vec![0.0_f32; k];
    let mut widest: Vec<f32> = cols.iter().map(|col| if col.head.is_empty() { 0.0 } else { paint::galley(ui, col.head, Type::Small, c(C::Ink2)).size().x }).collect();
    let mut keep: Vec<f32> = cols.iter().zip(&widest).map(|(col, w)| if col.head.trim().chars().count() <= SHORT_HEAD { *w } else { 0.0 }).collect();
    let mut words = vec![false; k];
    let mut two = false;
    for i in 0..n {
        let r = row(i);
        for (j, cell) in r.cells.iter().enumerate().take(k) {
            let wd = ink(ui, cell);
            widest[j] = widest[j].max(wd);
            if let Cell::Tag(s) = cell {
                tag[j] = tag[j].max(paint::galley(ui, s, Type::Small, c(C::Ink2)).size().x);
            }
            if cell.fixed() {
                keep[j] = keep[j].max(wd);
            }
            words[j] |= cell.words();
            two |= matches!(cell, Cell::Two(..));
        }
    }
    let share = cols.iter().any(|col| matches!(col.width, Col::Fr(_)));
    let main = if share { None } else { (0..k).find(|j| words[*j] && matches!(cols[*j].width, Col::Auto { .. })) };
    let resolved = cols
        .iter()
        .enumerate()
        .map(|(j, col)| match col.width {
            Col::Fit { min, max } => Col::Px(fit_width(tag[j], min, max)),
            Col::Auto { .. } if main == Some(j) => Col::Fr(1.0),
            Col::Auto { min, max } => Col::Px(widest[j].clamp(min, max.max(min))),
            w => w,
        })
        .collect();
    Measured { key, cols: resolved, keep, widest, row_h: if two { ROW_TWO_H } else { tokens::ROW_H } }
}

fn measure_key(ctx: &egui::Context, spec: &Spec, n: usize) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (spec.generation, n, crate::flow::words_epoch(ctx), ctx.pixels_per_point().to_bits()).hash(&mut h);
    for col in spec.cols {
        col.head.hash(&mut h);
        col.right.hash(&mut h);
        std::mem::discriminant(&col.width).hash(&mut h);
        match col.width {
            Col::Px(w) | Col::Fr(w) => w.to_bits().hash(&mut h),
            Col::Fit { min, max } | Col::Auto { min, max } => (min.to_bits(), max.to_bits()).hash(&mut h),
        }
    }
    h.finish()
}

pub fn table_with(ui: &mut egui::Ui, id_salt: &str, spec: Spec, n: usize, mut row: impl FnMut(usize) -> Row) -> Hit {
    let id = ui.id().with(("zikaron-table", id_salt));
    let mid = id.with("measured");
    let key = measure_key(ui.ctx(), &spec, n);
    let held = ui.ctx().data(|d| d.get_temp::<std::sync::Arc<Measured>>(mid)).filter(|m| m.key == key);
    let (m, measured) = match held {
        Some(m) => (m, 0),
        None => {
            let m = std::sync::Arc::new(measure(ui, spec.cols, n, &mut row, key));
            ui.ctx().data_mut(|d| d.insert_temp(mid, m.clone()));
            (m, n)
        }
    };
    let slot = ui.painter().add(egui::Shape::Noop);
    let mut drawn = 0;
    let (hit, shown) = crate::flow::grow(ui, id, |ui| {
        let w = ui.available_width();
        let cols: Vec<Column> = spec.cols.iter().zip(&m.cols).map(|(c, w)| Column { width: *w, ..*c }).collect();
        let natural = tokens::LIST_PAD_X * 2.0
            + tokens::ROW_INSET * 2.0
            + tokens::CELL_GAP * (cols.len().saturating_sub(1)) as f32
            + cols.iter().zip(&m.widest).map(|(c, wd)| match c.width { Col::Px(p) => p, _ => wd.max(tokens::BODY_MIN_W) }).sum::<f32>();
        if spec.wide && n > 0 && natural > w {
            let top = ui.cursor().min;
            let out = crate::scroll::nested(ui, egui::ScrollArea::horizontal().id_salt(id.with("wide")), |ui| {
                ui.set_min_width(natural);
                table_body(ui, id, &spec, &cols, &m, natural, n, &mut row, &mut drawn)
            });
            let (hit, card) = out.inner;
            (hit, Rect::from_min_size(top, vec2(w, card.height())))
        } else {
            table_body(ui, id, &spec, &cols, &m, w, n, &mut row, &mut drawn)
        }
    });
    ui.painter().set(slot, egui::Shape::Vec(crate::material::shapes(shown, Radius::Card, crate::material::Level::Card, 0.0)));
    crate::probe::took(ui.ctx(), crate::probe::Took { salt: id_salt.to_string(), rows: n, drawn, measured });
    hit
}

#[allow(clippy::too_many_arguments)]
fn table_body(ui: &mut egui::Ui, id: egui::Id, spec: &Spec, cols: &[Column], m: &Measured, w: f32, n: usize, row: &mut dyn FnMut(usize) -> Row, drawn: &mut usize) -> (Hit, Rect) {
    let top = ui.cursor().min;
    let mut hit = Hit::default();
    let inner_x = top.x + tokens::LIST_PAD_X;
    let inner_w = w - tokens::LIST_PAD_X * 2.0;
    let cell_room = inner_w - tokens::ROW_INSET * 2.0;
    let ws = widths_kept(cols, cell_room, &m.keep);
    let row_h = m.row_h;
    let mut y = top.y + tokens::LIST_PAD_Y;
    let mut head_rule = y;
    let mut head_line = None;
    let p = ui.painter().clone();
    if spec.head {
        let hr = Rect::from_min_size(pos2(top.x, top.y), vec2(w, tokens::TABLE_HEAD_H));
        let mut x = inner_x + tokens::ROW_INSET;
        for (ci, (col, cw)) in cols.iter().zip(&ws).enumerate() {
            let sorts = spec.sortable.contains(&ci);
            let ink = if sorts && spec.sorted == Some(ci) { c(C::Ink) } else { c(C::Ink2) };
            if !col.head.is_empty() {
                let (ax, al) = if col.right { (x + cw, egui::Align2::RIGHT_CENTER) } else { (x, egui::Align2::LEFT_CENTER) };
                paint::at(&p, ui, pos2(ax, hr.center().y), al, col.head, Type::Small, ink, *cw);
            }
            if sorts {
                let cell = Rect::from_min_size(pos2(x, hr.top()), vec2(*cw, hr.height()));
                let resp = ui.interact(cell, id.with(("head", ci)), egui::Sense::click());
                if resp.clicked() {
                    hit.head = Some(ci);
                }
                let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            }
            x += cw + tokens::CELL_GAP;
        }
        head_line = Some(egui::Shape::hline(hr.x_range(), hr.bottom() - crate::band::LINE_HALF, egui::Stroke::new(1.0_f32, c(C::Line2))));
        head_rule = hr.bottom();
        y = hr.bottom();
    }
    let rows_top = y;
    let rect_of = |i: usize| Rect::from_min_size(pos2(inner_x, rows_top + row_h * i as f32), vec2(inner_w, row_h));
    let rows_bottom = rows_top + row_h * n as f32;
    let card = Rect::from_min_max(top, pos2(top.x + w, if n == 0 { rows_top + 96.0 } else { rows_bottom } + tokens::LIST_PAD_Y));
    let mut group = crate::flow::Group::new(ui, id, top, w);
    if n == 0 {
        let r = Rect::from_min_size(pos2(inner_x, y), vec2(inner_w, 96.0));
        p.text(r.center(), egui::Align2::CENTER_CENTER, spec.empty, Type::Body.font(), c(C::Ink3));
    } else {
        let clip = ui.clip_rect();
        let sight = |at: f32| ((at - rows_top) / row_h).clamp(0.0, n as f32);
        let first = sight(clip.top()).floor() as usize;
        let last = (sight(clip.bottom()).ceil() as usize).max(first);
        let (f0, f1) = (first.saturating_sub(1), (last + 1).min(n));
        let rows: Vec<Row> = (f0..f1).map(&mut *row).collect();
        *drawn = rows.len();
        let at = |i: usize| &rows[i - f0];
        group.in_sight(Rect::from_min_max(pos2(top.x, clip.top()), pos2(top.x + w, clip.bottom())).shrink2(vec2(0.0, 1.0)));
        let hovered = (first..last).find(|i| crate::flow::pointer_on(ui, rect_of(*i)));
        let keys = row_keys(&rows);
        for i in f0..f1 {
            let r = rect_of(i);
            let this = at(i);
            let start = group.start(ui.ctx());
            let sense = if this.click { egui::Sense::click() } else { egui::Sense::hover() };
            let rid = id.with(("row", i));
            let resp = ui.interact(r, rid.with("hit"), sense);
            let hot = if hovered == Some(i) { 1.0 } else { 0.0 };
            let down = this.click && hot > 0.0 && resp.is_pointer_button_down_on();
            let run = this.on.then(|| {
                let first = (f0..=i).rev().take_while(|k| at(*k).on).last().unwrap_or(i);
                let last = (i..f1).take_while(|k| at(*k).on).last().unwrap_or(i);
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
                Some((first, last)) => lit_block((top.x, top.x + w), rect_of(first).union(rect_of(last)), spec.head.then_some(head_rule), first == 0, last + 1 == n),
                None => lit_block((top.x, top.x + w), r, spec.head.then_some(head_rule), i == 0, i + 1 == n),
            };
            if let Some(g) = ground {
                p.rect_filled(band, corners, g);
                crate::probe::lit(ui.ctx(), crate::probe::Lit { block: band, corners, slot, card, card_corner: Radius::Card.px() });
            }
            if this.click {
                let (glow, at) = crate::material::glow_at(ui, rid, r, hot > 0.0);
                if glow > 0.0 {
                    if let Some(s) = crate::material::glow_shape_corners(band, corners, at, c(C::Accent).gamma_multiply(0.04 * glow)) {
                        p.add(s);
                    }
                }
            }
            if i > f0 && !(this.on && at(i - 1).on) {
                p.hline((r.left() + tokens::ROW_INSET)..=(r.right() - tokens::ROW_INSET), r.top(), egui::Stroke::new(1.0_f32, c(C::Line2).gamma_multiply(0.7)));
            }
            let mut x = r.left() + tokens::ROW_INSET;
            let cy = r.center().y;
            let mut keyed = false;
            for (ci, ((cell, cw), col)) in this.cells.iter().zip(&ws).zip(cols).enumerate() {
                if let Cell::Key(words) = cell {
                    let kw = (paint::galley(ui, words, Type::Body, c(C::Ink)).size().x + KEY_PAD).min(*cw);
                    let kx = if col.right { x + cw - kw } else { x };
                    let kr = Rect::from_min_size(pos2(kx, cy - KEY_H / 2.0), vec2(kw, KEY_H));
                    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(kr).id_salt(rid.with(("key", ci))));
                    if crate::button::key(&mut child, words, crate::button::Role::Secondary, true).clicked() {
                        hit.key = Some((i, ci));
                        keyed = true;
                    }
                } else {
                    paint_cell(ui, &p, rid, cell, (x, cy), *cw, col.right, (this.gone, hot, this.click));
                }
                x += cw + tokens::CELL_GAP;
            }
            if this.click {
                crate::probe::row(ui.ctx(), r);
                if resp.clicked() && !keyed {
                    hit.clicked = Some(i);
                }
                let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            }
            group.row(ui.ctx(), keys[i - f0], r, start);
        }
    }
    let _ = group.finish(ui.ctx());
    if let Some(line) = head_line {
        p.add(line);
    }
    ui.advance_cursor_after_rect(card);
    (hit, card)
}

#[allow(clippy::too_many_arguments)]
fn paint_cell(ui: &mut egui::Ui, p: &egui::Painter, id: egui::Id, cell: &Cell, at: (f32, f32), w: f32, right: bool, row: (bool, f32, bool)) {
    let (x, cy) = at;
    let (gone, hot, click) = row;
    let (ax, al) = if right { (x + w, egui::Align2::RIGHT_CENTER) } else { (x, egui::Align2::LEFT_CENTER) };
    match cell {
        Cell::Seq(n) => {
            p.text(pos2(x + w, cy), egui::Align2::RIGHT_CENTER, format!("#{n}"), Type::MonoSmall.font(), c(C::Ink3));
        }
        Cell::Tag(s) => {
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
            paint::at(p, ui, pos2(ax, cy), al, s, Type::MonoSmall, c(C::Ink3), w);
        }
        Cell::Amount(a) => {
            let r = paint::at(p, ui, pos2(ax, cy), al, &a.shown, Type::MonoSmall, c(C::Ink3), w);
            if a.cut() {
                let _ = crate::layer::amount_tip(ui.interact(r, id.with(("amount-whole", a.whole.as_str())), egui::Sense::hover()), a);
            }
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
        Cell::Two(a, b) => {
            let colour = if gone { c(C::Ink3) } else { c(C::Ink) };
            paint::at(p, ui, pos2(ax, cy - 9.0), al, a, Type::Body, colour, w);
            paint::at(p, ui, pos2(ax, cy + 11.0), al, b, Type::Small, c(C::Ink3), w);
        }
        Cell::Lamp(m, s) => {
            let room = (w - tokens::MARK - tokens::S2).max(0.0);
            if right {
                mark::paint_mark(ui.ctx(), p, pos2(x + w - tokens::MARK / 2.0, cy), *m, tokens::MARK);
                paint::at(p, ui, pos2(x + room, cy), egui::Align2::RIGHT_CENTER, s, Type::Body, c(C::Ink), room);
            } else {
                mark::paint_mark(ui.ctx(), p, pos2(x + tokens::MARK / 2.0, cy), *m, tokens::MARK);
                paint::at(p, ui, pos2(x + tokens::MARK + tokens::S2, cy), egui::Align2::LEFT_CENTER, s, Type::Body, c(C::Ink), room);
            }
        }
        Cell::Note(s) => {
            paint::at(p, ui, pos2(ax, cy), al, s, Type::Note, c(C::Ink2), w);
        }
        Cell::Small(s) => {
            paint::at(p, ui, pos2(ax, cy), al, s, Type::Small, c(C::Ink3), w);
        }
        Cell::Short(s) => {
            paint::at(p, ui, pos2(ax, cy), al, s, Type::Body, c(C::Ink), w);
        }
        Cell::Key(_) | Cell::Empty => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn narrow_the_free_words_give_way_then_the_body_and_last_what_is_kept() {
        let cols = list();
        let gaps = tokens::CELL_GAP * 5.0;
        let fixed = 36.0 + 84.0 + 180.0 + 20.0 + 16.0;
        let sum = |w: &[f32]| w.iter().sum::<f32>() + gaps;
        let keep = [30.0, 70.0, 0.0, 140.0, 20.0, 16.0];
        let firm: f32 = keep.iter().sum();
        let wide = widths_kept(&cols, 900.0, &keep);
        assert_eq!((wide[0], wide[1], wide[3]), (36.0, 84.0, 180.0));
        assert!((wide[2] - (900.0 - gaps - fixed)).abs() < 0.01);
        let at_floor = fixed + tokens::BODY_MIN_W + gaps;
        let just_under = widths_kept(&cols, at_floor - 20.0, &keep);
        assert!((just_under[2] - tokens::BODY_MIN_W).abs() < 0.01, "the body keeps its floor: {just_under:?}");
        assert!(just_under[3] < 180.0 && just_under[3] > 140.0 && just_under[1] < 84.0 && just_under[1] > 70.0, "what is not kept gave way: {just_under:?}");
        assert!((sum(&just_under) - (at_floor - 20.0)).abs() < 0.01);
        let kept_only = widths_kept(&cols, firm + gaps + 40.0, &keep);
        assert!(kept_only.iter().zip(&keep).enumerate().all(|(i, (w, k))| i == 2 || (w - k).abs() < 0.01), "only what is kept: {kept_only:?}");
        assert!((kept_only[2] - 40.0).abs() < 0.01, "the body under its floor only now: {kept_only:?}");
        let tiny = widths_kept(&cols, gaps + firm / 2.0, &keep);
        assert!(tiny[2] == 0.0 && (sum(&tiny) - (gaps + firm / 2.0)).abs() < 0.01, "to the edge: {tiny:?}");
        assert!((tiny[3] - 70.0).abs() < 0.01, "what is kept, in proportion: {tiny:?}");
        let none = widths(&cols, tokens::BODY_MIN_W + gaps + 10.0);
        assert!((none[2] - tokens::BODY_MIN_W).abs() < 0.01 && (sum(&none) - (tokens::BODY_MIN_W + gaps + 10.0)).abs() < 0.01, "{none:?}");
        let only_fixed = widths(&[SEQ, col("", Col::Px(200.0))], 100.0 + tokens::CELL_GAP);
        assert!((only_fixed.iter().sum::<f32>() - 100.0).abs() < 0.01);
        assert_eq!(widths(&[TYPE, col("", Col::Fr(1.0))], 1000.0)[0], tokens::TYPE_W);
    }
}
