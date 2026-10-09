//! Cards and layout blocks: the raised card, the flat card, a titled section, dashboard tiles, the
//! settings-style form, and the equal grids pages lay blocks side by side with.

use crate::icons::{self, Glyph};
use crate::mark::{self, Mark};
use crate::motion::{self, Curve};
use crate::paint;
use crate::palette::{c, Lift, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

/// A raised card: white, corner 12, 22 × 24 inside, the card shadow.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card_pad(ui, egui::vec2(tokens::CARD_PAD_X, tokens::CARD_PAD_Y), add)
}

/// A raised card with its own padding (strips 12 × 18; lists 4 × 6).
pub fn card_pad<R>(ui: &mut egui::Ui, pad: egui::Vec2, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let slot = ui.painter().add(egui::Shape::Noop);
    let shown = egui::Frame::new().inner_margin(egui::Margin::symmetric(pad.x as i8, pad.y as i8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    let rect = shown.response.rect;
    let tmp = ui.painter().clone();
    let mut shapes = Vec::new();
    raised_shapes(&tmp, rect, Radius::Card, c(C::Surface), Lift::Card, &mut shapes);
    ui.painter().set(slot, egui::Shape::Vec(shapes));
    shown.inner
}

/// A card that opens something (a held grant on "my grants"): it lifts on hover and presses to 0.995; the
/// whole card is the target.
pub fn open_card<R>(ui: &mut egui::Ui, id: egui::Id, add: impl FnOnce(&mut egui::Ui) -> R) -> (R, egui::Response) {
    let slot = ui.painter().add(egui::Shape::Noop);
    let hot_id = id.with("hot");
    let press_id = id.with("press");
    let shown = egui::Frame::new().inner_margin(egui::Margin::symmetric(tokens::CARD_PAD_X as i8, 16)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    let rect = shown.response.rect;
    let resp = ui.interact(rect, id, egui::Sense::click());
    let hot = motion::flag(ui.ctx(), hot_id, resp.hovered(), tokens::FAST);
    let press = motion::to(ui.ctx(), press_id, if resp.is_pointer_button_down_on() { 0.995 } else { 1.0 }, tokens::FAST, Curve::Ease);
    let r = paint::scaled(rect, press);
    let tmp = ui.painter().clone();
    let mut shapes = Vec::new();
    raised_shapes(&tmp, r, Radius::Card, c(C::Surface), if hot > 0.5 { Lift::Hover } else { Lift::Card }, &mut shapes);
    ui.painter().set(slot, egui::Shape::Vec(shapes));
    crate::probe::row(ui.ctx(), rect);
    (shown.inner, resp.on_hover_cursor(egui::CursorIcon::PointingHand))
}

/// A choice block in a list (identities, networks): the chosen one on the selection ground with a wash
/// edge, the others flat with a line; 12 × 14 inside.
pub fn choice<R>(ui: &mut egui::Ui, on: bool, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let (fill, line) = if on { (c(C::Sel), c(C::AccentWash)) } else { (c(C::Surface), c(C::Line)) };
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0_f32, line))
        .corner_radius(Radius::Card.egui())
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// The shapes of a raised surface (so a card can put them under content drawn first).
pub fn raised_shapes(_p: &egui::Painter, rect: Rect, radius: Radius, fill: egui::Color32, lift: Lift, out: &mut Vec<egui::Shape>) {
    let (ring, shadows) = crate::palette::lift(lift);
    let r = radius.egui();
    for s in shadows.iter().rev() {
        if s.color.a() == 0 {
            continue;
        }
        let sh = egui::Shadow { offset: [0, s.y.round() as i8], blur: s.blur.round().clamp(0.0, 255.0) as u8, spread: 0, color: s.color };
        out.push(sh.as_shape(rect, r).into());
    }
    out.push(egui::epaint::RectShape::new(rect, r, fill, egui::Stroke::new(0.5_f32, ring), egui::StrokeKind::Outside).into());
}

/// A flat card: a one-point line instead of a shadow (side notes, the "how the recipient checks" card).
pub fn flat<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(c(C::Surface))
        .stroke(egui::Stroke::new(1.0_f32, c(C::Line)))
        .corner_radius(Radius::Card.egui())
        .inner_margin(egui::Margin::symmetric(tokens::CARD_PAD_X as i8, tokens::CARD_PAD_Y as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A small card title inside a card (14, semibold).
pub fn card_title(ui: &mut egui::Ui, s: &str) {
    paint::text(ui, s, Type::Card, c(C::Ink));
}

/// The title of a flat side card (13, semibold, first ink).
pub fn flat_title(ui: &mut egui::Ui, s: &str) {
    ui.label(egui::RichText::new(s).font(egui::FontId::new(Type::Small.size(), crate::fonts::strong())).color(c(C::Ink)).line_height(Some(Type::Note.line())));
}

/// A group title above a card: 14 semibold in the second ink, 4 in from the card's edge, 10 above the card.
pub fn group_title(ui: &mut egui::Ui, s: &str) {
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(s).font(egui::FontId::new(14.0, crate::fonts::strong())).color(c(C::Ink2)).line_height(Some(Type::Note.line())));
    });
    ui.add_space((10.0 - ui.spacing().item_spacing.y).max(0.0));
}

/// A footnote under a card: 13 in the second ink, 4 in, 10 below the card.
pub fn group_foot(ui: &mut egui::Ui, s: &str) {
    ui.add_space((10.0 - ui.spacing().item_spacing.y).max(0.0));
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        paint::text(ui, s, Type::Small, c(C::Ink2));
    });
}

/// A titled section: group title, the block, an optional footnote.
pub fn section<R>(ui: &mut egui::Ui, title: &str, foot: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        if !title.is_empty() {
            group_title(ui, title);
        }
        let r = add(ui);
        if !foot.is_empty() {
            group_foot(ui, foot);
        }
        r
    })
    .inner
}

/// A dashboard tile: a note-sized title, a large figure (optionally with a mark before it, optionally in a
/// tone), a small line under it. Clickable tiles lift a point on hover and press to 0.99.
pub struct Tile<'a> {
    pub title: &'a str,
    pub figure: &'a str,
    pub mark: Option<Mark>,
    pub figure_colour: Option<egui::Color32>,
    pub sub: &'a str,
    pub mono_figure: bool,
    pub clickable: bool,
}

pub fn tile(ui: &mut egui::Ui, t: &Tile) -> egui::Response {
    let w = ui.available_width();
    let h_target = crate::grid::cell_height(ui).max(0.0);
    let pad = vec2(18.0, 16.0);
    let fig_font = if t.mono_figure { egui::FontId::new(16.0, egui::FontFamily::Monospace) } else { Type::Figure.font() };
    let content_h = Type::Note.line() + 6.0 + Type::Figure.line() + if t.sub.is_empty() { 0.0 } else { 2.0 + Type::Small.line() };
    let h = (content_h + pad.y * 2.0).max(h_target);
    let sense = if t.clickable { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    let id = resp.id;
    let hot = motion::flag(ui.ctx(), id.with("hot"), t.clickable && resp.hovered(), tokens::FAST);
    let press = motion::to(ui.ctx(), id.with("press"), if t.clickable && resp.is_pointer_button_down_on() { 0.99 } else { 1.0 }, tokens::FAST, Curve::Ease);
    let r = paint::scaled(rect.translate(vec2(0.0, -hot)), press);
    let p = ui.painter();
    paint::surface(p, r, Radius::Card, c(C::Surface), if hot > 0.5 { Lift::Hover } else { Lift::Card });
    let inner = r.shrink2(pad);
    let room = inner.width();
    let title_fit = crate::width::elide_to(ui, t.title, Type::Note.font(), room);
    p.text(inner.left_top(), egui::Align2::LEFT_TOP, &title_fit, Type::Note.font(), c(C::Ink2));
    let fy = inner.top() + Type::Note.line() + 6.0;
    let mut x = inner.left();
    if let Some(m) = t.mark {
        mark::paint_mark(ui.ctx(), p, pos2(x + 9.0, fy + Type::Figure.line() / 2.0), m, tokens::MARK);
        x += 18.0 + 8.0;
    }
    let colour = t.figure_colour.unwrap_or(c(C::Ink));
    let fit = crate::width::elide_to(ui, t.figure, fig_font.clone(), (inner.right() - x).max(0.0));
    p.text(pos2(x, fy + Type::Figure.line() / 2.0), egui::Align2::LEFT_CENTER, fit, fig_font, colour);
    if !t.sub.is_empty() {
        let sy = fy + Type::Figure.line() + 2.0;
        let fit = crate::width::elide_to(ui, t.sub, Type::Small.font(), room);
        p.text(pos2(inner.left(), sy), egui::Align2::LEFT_TOP, fit, Type::Small.font(), c(C::Ink3));
    }
    crate::probe::tile(ui.ctx(), rect, title_fit == t.title);
    if t.clickable {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// `n` blocks side by side: three across on a wide page, two on a narrow one.
pub fn grid3<R>(ui: &mut egui::Ui, id_salt: &str, n: usize, cell: impl FnMut(&mut egui::Ui, usize) -> R) -> Vec<R> {
    grid(ui, id_salt, n, tokens::GRID3_MIN_W, cell)
}

/// Two blocks side by side that stack on a narrow page.
pub fn grid2<R>(ui: &mut egui::Ui, id_salt: &str, n: usize, cell: impl FnMut(&mut egui::Ui, usize) -> R) -> Vec<R> {
    grid(ui, id_salt, n, tokens::GRID2_MIN_W, cell)
}

/// `n` blocks across, each at least `min_w`, 16 apart.
pub fn grid<R>(ui: &mut egui::Ui, id_salt: &str, n: usize, min_w: f32, cell: impl FnMut(&mut egui::Ui, usize) -> R) -> Vec<R> {
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing = vec2(tokens::CARD_GAP, tokens::CARD_GAP);
        crate::grid::tiles_min(ui, id_salt, n, min_w, cell)
    })
    .inner
}

/// Which side of a two-column block.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Main,
    Side,
}

/// A main column and a side column of `side_w` (300 by default), 16 apart; stacked when the main column
/// would be narrower than `min_main`.
pub fn two_cols(ui: &mut egui::Ui, side_w: f32, min_main: f32, draw: impl FnMut(&mut egui::Ui, Side)) {
    cols(ui, false, side_w, min_main, draw);
}

/// A form column of at most `form_w` on the left and the result taking the rest (the check page): stacked
/// when the result would be narrower than `min_rest`. `Side::Main` is the form, `Side::Side` the result.
pub fn form_and_result(ui: &mut egui::Ui, form_w: f32, min_rest: f32, draw: impl FnMut(&mut egui::Ui, Side)) {
    cols(ui, true, form_w, min_rest, draw);
}

fn cols(ui: &mut egui::Ui, fixed_left: bool, fixed_w: f32, min_rest: f32, mut draw: impl FnMut(&mut egui::Ui, Side)) {
    let w = ui.available_width();
    let gap = tokens::CARD_GAP;
    let side_w = fixed_w;
    let min_main = min_rest;
    if w < min_main + gap + side_w {
        ui.vertical(|ui| {
            ui.set_width(w);
            ui.spacing_mut().item_spacing.y = tokens::CARD_GAP;
            draw(ui, Side::Main);
            draw(ui, Side::Side);
        });
        return;
    }
    let lw = if fixed_left { side_w } else { w - side_w - gap };
    let side_w = if fixed_left { w - lw - gap } else { side_w };
    let top = ui.cursor().min;
    let l = ui
        .scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(top, vec2(lw, f32::INFINITY))), |ui| {
            ui.set_width(lw);
            ui.spacing_mut().item_spacing.y = tokens::CARD_GAP;
            draw(ui, Side::Main);
        })
        .response
        .rect;
    let r = ui
        .scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(top.x + lw + gap, top.y), vec2(side_w, f32::INFINITY))), |ui| {
            ui.set_width(side_w);
            ui.spacing_mut().item_spacing.y = tokens::CARD_GAP;
            draw(ui, Side::Side);
        })
        .response
        .rect;
    let bottom = l.bottom().max(r.bottom());
    ui.allocate_rect(Rect::from_min_max(top, pos2(top.x + w, bottom)), egui::Sense::hover());
}

/// Settings-style form: a card of rows 56 high (title and note on the left, value or control on the right),
/// rules between rows inset 20. Row hover grounds follow the card's corners on its first and last row.
pub fn form<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui, &mut Form) -> R) -> R {
    let card_slot = ui.painter().add(egui::Shape::Noop);
    let fill_slot = ui.painter().add(egui::Shape::Noop);
    let top = ui.cursor().min;
    let w = ui.available_width();
    let mut f = Form { rows: 0, width: w, bottom: top.y, fills: Vec::new() };
    let r = ui
        .vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            add(ui, &mut f)
        })
        .inner;
    let rect = Rect::from_min_size(top, vec2(w, (f.bottom - top.y).max(0.0)));
    let mut shapes = Vec::new();
    raised_shapes(ui.painter(), rect, Radius::Card, c(C::Surface), Lift::Card, &mut shapes);
    ui.painter().set(card_slot, egui::Shape::Vec(shapes));
    let last = f.rows.saturating_sub(1);
    let fills: Vec<egui::Shape> = f
        .fills
        .iter()
        .map(|(r, col, i)| {
            let k = Radius::Card.px() as u8;
            let cr = egui::CornerRadius { nw: if *i == 0 { k } else { 0 }, ne: if *i == 0 { k } else { 0 }, sw: if *i == last { k } else { 0 }, se: if *i == last { k } else { 0 } };
            egui::epaint::RectShape::filled(*r, cr, *col).into()
        })
        .collect();
    ui.painter().set(fill_slot, egui::Shape::Vec(fills));
    r
}

/// The rows of a form being drawn.
pub struct Form {
    rows: usize,
    width: f32,
    bottom: f32,
    /// Hover and press grounds: rectangle, color, row index.
    fills: Vec<(Rect, egui::Color32, usize)>,
}

/// What a form row holds on its right.
pub enum Value<'a> {
    None,
    Text(&'a str),
    Mono(&'a str),
    /// A chevron: the whole row opens something.
    Nav,
    /// A chevron after words ("go to …").
    NavWith(&'a str),
    /// A row that would open something but cannot now: dimmed, not clickable.
    Off,
}

impl Form {
    fn rule(&mut self, ui: &mut egui::Ui, top: f32, inset: f32) {
        if self.rows > 0 {
            let x0 = ui.max_rect().left() + inset;
            ui.painter().hline(x0..=ui.max_rect().left() + self.width, top, egui::Stroke::new(1.0_f32, c(C::Line2)));
        }
        self.rows += 1;
    }

    fn ground(&mut self, rect: Rect, col: egui::Color32) {
        self.fills.push((rect, col, self.rows.saturating_sub(1)));
    }

    /// A row: title (and note), and a value or chevron on the right. Returns the row's response (clickable
    /// when it navigates).
    pub fn row(&mut self, ui: &mut egui::Ui, title: &str, note: &str, value: Value) -> egui::Response {
        self.row_with(ui, None, title, note, value, |_| {})
    }

    /// A row with a mark before its title and a control on its right drawn by `right`.
    pub fn row_with(&mut self, ui: &mut egui::Ui, lead: Option<Mark>, title: &str, note: &str, value: Value, right: impl FnOnce(&mut egui::Ui)) -> egui::Response {
        let w = self.width;
        let top = ui.cursor().min.y;
        self.rule(ui, top, tokens::FORM_PAD_X);
        let nav = matches!(value, Value::Nav | Value::NavWith(_));
        let off = matches!(value, Value::Off);
        let note_h = if note.is_empty() { 0.0 } else { 3.0 + Type::Small.line() };
        let h = (20.0 + Type::Body.line() + note_h).max(tokens::FORM_ROW_H);
        let rect = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(w, h));
        let sense = if nav { egui::Sense::click() } else { egui::Sense::hover() };
        let resp = ui.allocate_rect(rect, sense);
        let hot = motion::flag(ui.ctx(), resp.id.with("hot"), nav && resp.hovered(), tokens::FAST);
        let press = nav && resp.is_pointer_button_down_on();
        if hot > 0.0 || press {
            let col = if press { c(C::Press) } else { c(C::Hover).gamma_multiply(hot) };
            self.ground(rect, col);
        }
        let mut x = rect.left() + tokens::FORM_PAD_X;
        let p = ui.painter().clone();
        if let Some(m) = lead {
            mark::paint_mark(ui.ctx(), &p, pos2(x + 9.0, rect.center().y), m, tokens::MARK);
            x += 18.0 + 12.0;
        }
        // Right side first, so the title knows its room.
        let right_edge = rect.right() - tokens::FORM_PAD_X;
        let mut rx = right_edge;
        match value {
            Value::Nav => {
                icons::glyph_at(&p, Glyph::Chev, pos2(rx - 6.0, rect.center().y), 12.0, c(C::Ink3));
                rx -= 12.0 + 8.0;
            }
            Value::NavWith(s) => {
                icons::glyph_at(&p, Glyph::Chev, pos2(rx - 6.0, rect.center().y), 12.0, c(C::Ink3));
                rx -= 12.0 + 8.0;
                let colour = crate::palette::mix(c(C::Ink2), c(C::Ink), hot);
                let r = p.text(pos2(rx, rect.center().y), egui::Align2::RIGHT_CENTER, s, Type::Note.font(), colour);
                rx = r.left() - 8.0;
            }
            Value::Text(s) => {
                let room = (right_edge - x) * 0.6;
                let r = paint::at(&p, ui, pos2(rx, rect.center().y), egui::Align2::RIGHT_CENTER, s, Type::Body, c(C::Ink2), room);
                rx = r.left() - 8.0;
            }
            Value::Mono(s) => {
                let room = (right_edge - x) * 0.6;
                let r = paint::at(&p, ui, pos2(rx, rect.center().y), egui::Align2::RIGHT_CENTER, s, Type::Mono, c(C::Ink2), room);
                rx = r.left() - 8.0;
            }
            Value::Off => {
                icons::glyph_at(&p, Glyph::Chev, pos2(rx - 6.0, rect.center().y), 12.0, c(C::Ink3).gamma_multiply(tokens::OFF));
                rx -= 12.0 + 8.0;
            }
            Value::None => {}
        }
        // A control on the right (switch, segmented control, menu key).
        let ctl = Rect::from_min_max(pos2(x, rect.top()), pos2(rx, rect.bottom()));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(ctl).layout(egui::Layout::right_to_left(egui::Align::Center)));
        right(&mut child);
        let used = child.min_rect();
        let title_room = ((if used.width() > 1.0 { used.left() - 12.0 } else { rx }) - x).max(0.0);
        let ink = if off { c(C::Ink3) } else { c(C::Ink) };
        if note.is_empty() {
            paint::at(&p, ui, pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, title, Type::Body, ink, title_room);
        } else {
            let ty = rect.center().y - (Type::Body.line() + note_h) / 2.0;
            paint::at(&p, ui, pos2(x, ty), egui::Align2::LEFT_TOP, title, Type::Body, ink, title_room);
            paint::at(&p, ui, pos2(x, ty + Type::Body.line() + 3.0), egui::Align2::LEFT_TOP, note, Type::Small, c(C::Ink2), title_room);
        }
        self.bottom = rect.bottom();
        if nav {
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            resp
        }
    }

    /// A settings-home row: 72 high, a title and a small note, a chevron; the whole row opens the section.
    pub fn nav_big(&mut self, ui: &mut egui::Ui, title: &str, note: &str) -> egui::Response {
        self.big(ui, None, title, note, Some(""))
    }

    /// A large row (settings home, alerts): 72 high, an optional mark before a title and a small note. With
    /// `go` the whole row opens something: the words (if any) and a chevron stand on the right.
    pub fn big(&mut self, ui: &mut egui::Ui, lead: Option<Mark>, title: &str, note: &str, go: Option<&str>) -> egui::Response {
        let w = self.width;
        let top = ui.cursor().min.y;
        self.rule(ui, top, tokens::SET_PAD_X);
        let rect = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(w, tokens::SET_ROW_H));
        let nav = go.is_some();
        let resp = ui.allocate_rect(rect, if nav { egui::Sense::click() } else { egui::Sense::hover() });
        let hot = motion::flag(ui.ctx(), resp.id.with("hot"), nav && resp.hovered(), tokens::FAST);
        let p = ui.painter().clone();
        if nav && resp.is_pointer_button_down_on() {
            self.ground(rect, c(C::Press));
        } else if hot > 0.0 {
            self.ground(rect, c(C::Hover).gamma_multiply(hot));
        }
        let mut x = rect.left() + tokens::SET_PAD_X;
        if let Some(m) = lead {
            mark::paint_mark(ui.ctx(), &p, pos2(x + 9.0, rect.center().y), m, tokens::MARK);
            x += 18.0 + 14.0;
        }
        let mut rx = rect.right() - tokens::SET_PAD_X;
        if let Some(words) = go {
            icons::glyph_at(&p, Glyph::Chev, pos2(rx - 7.0, rect.center().y), 14.0, c(C::Ink3));
            rx -= 14.0 + 8.0;
            if !words.is_empty() {
                let colour = crate::palette::mix(c(C::Ink2), c(C::Ink), hot);
                rx = p.text(pos2(rx, rect.center().y), egui::Align2::RIGHT_CENTER, words, Type::Note.font(), colour).left() - 12.0;
            }
        }
        let room = (rx - x).max(0.0);
        let ty = rect.center().y - (Type::Row.line() + 3.0 + Type::Small.line()) / 2.0;
        paint::at(&p, ui, pos2(x, ty), egui::Align2::LEFT_TOP, title, Type::Row, c(C::Ink), room);
        paint::at(&p, ui, pos2(x, ty + Type::Row.line() + 3.0), egui::Align2::LEFT_TOP, note, Type::Small, c(C::Ink2), room);
        self.bottom = rect.bottom();
        if nav {
            crate::probe::row(ui.ctx(), rect);
            resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        } else {
            resp
        }
    }

    /// A row of keys at the right end.
    pub fn keys<R>(&mut self, ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let w = self.width;
        let top = ui.cursor().min.y;
        self.rule(ui, top, tokens::FORM_PAD_X);
        let rect = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(w, tokens::FORM_ROW_H));
        ui.allocate_rect(rect, egui::Sense::hover());
        let inner = rect.shrink2(vec2(tokens::FORM_PAD_X, 10.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::right_to_left(egui::Align::Center)));
        child.spacing_mut().item_spacing.x = tokens::S2;
        let r = add(&mut child);
        self.bottom = rect.bottom();
        r
    }

    /// A row holding any content (an input and a key), padded like the others.
    pub fn free<R>(&mut self, ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let top = ui.cursor().min.y;
        self.rule(ui, top, tokens::FORM_PAD_X);
        let shown = egui::Frame::new().inner_margin(egui::Margin::symmetric(tokens::FORM_PAD_X as i8, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(tokens::FORM_ROW_H - 20.0);
            add(ui)
        });
        self.bottom = shown.response.rect.bottom();
        shown.inner
    }

    /// A centered red row (delete this identity…).
    pub fn danger(&mut self, ui: &mut egui::Ui, title: &str) -> egui::Response {
        let w = self.width;
        let top = ui.cursor().min.y;
        self.rule(ui, top, tokens::FORM_PAD_X);
        let rect = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(w, tokens::FORM_ROW_H));
        let resp = ui.allocate_rect(rect, egui::Sense::click());
        let hot = motion::flag(ui.ctx(), resp.id.with("hot"), resp.hovered(), tokens::FAST);
        if hot > 0.0 {
            self.ground(rect, c(C::BadBox).gamma_multiply(hot));
        }
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, title, Type::Body.font(), c(C::BadInk));
        self.bottom = rect.bottom();
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}

/// The head of a detail page: a large title (24/32 bold), a line under it, and a pill on the right; 2 in
/// from the page's edge, 2 above and 6 below.
pub fn hero(ui: &mut egui::Ui, title: &str, sub: &str, struck: bool, pill: impl FnOnce(&mut egui::Ui)) -> Rect {
    egui::Frame::new().inner_margin(egui::Margin { left: 2, right: 2, top: 2, bottom: 6 }).show(ui, |ui| hero_row(ui, title, sub, struck, pill)).response.rect
}

fn hero_row(ui: &mut egui::Ui, title: &str, sub: &str, struck: bool, pill: impl FnOnce(&mut egui::Ui)) -> Rect {
    let w = ui.available_width();
    let r = ui
        .allocate_ui_with_layout(vec2(w, 0.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            pill(ui);
            ui.add_space(tokens::S4);
            ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let room = ui.available_width();
                let font = egui::FontId::new(24.0, crate::fonts::strong());
                let fit = crate::width::elide_to(ui, title, font.clone(), room);
                let colour = if struck { c(C::Ink3) } else { c(C::Ink) };
                let tr = ui.add(egui::Label::new(egui::RichText::new(&fit).font(font).color(colour).line_height(Some(32.0))).wrap_mode(egui::TextWrapMode::Extend));
                if struck {
                    ui.painter().hline(tr.rect.x_range(), tr.rect.center().y, egui::Stroke::new(1.0_f32, c(C::Ink3)));
                }
                if !sub.is_empty() {
                    paint::line(ui, sub, Type::Note, c(C::Ink2), room);
                }
            });
        })
        .response
        .rect;
    r
}
