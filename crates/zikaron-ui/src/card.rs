//! Cards and layout blocks: the raised card, the flat card, a titled section, dashboard tiles, the
//! settings-style form, and the equal grids pages lay blocks side by side with.

use crate::icons::{self, Glyph};
use crate::mark::{self, Mark};
use crate::motion::{self, Curve};
use crate::paint;
use crate::material::{self, Level};
use crate::palette::{c, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

/// A raised card: white, corner 16, 22 × 24 inside, a half-point rim and the card shadow.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    card_pad(ui, egui::vec2(tokens::CARD_PAD_X, tokens::CARD_PAD_Y), add)
}

/// A block of content padded by `pad` (and the width of its edge line, `edge`, inside that) whose height follows
/// its content ([`crate::flow::grow`]); its ground is drawn under it, at the height shown, by `ground`. Every
/// card of this file is one.
fn block<R>(ui: &mut egui::Ui, pad: egui::Vec2, edge: f32, add: impl FnOnce(&mut egui::Ui) -> R, ground: impl FnOnce(Rect) -> Vec<egui::Shape>) -> (R, Rect) {
    let slot = ui.painter().add(egui::Shape::Noop);
    let id = ui.next_auto_id().with("zikaron-card");
    let (inner, shown) = crate::flow::grow(ui, id, |ui| {
        let r = egui::Frame::new().inner_margin(egui::Margin::symmetric((pad.x + edge) as i8, (pad.y + edge) as i8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        });
        (r.inner, r.response.rect)
    });
    ui.painter().set(slot, egui::Shape::Vec(ground(shown)));
    (inner, shown)
}

/// A raised card with its own padding (strips 12 × 18; lists 4 × 6).
pub fn card_pad<R>(ui: &mut egui::Ui, pad: egui::Vec2, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    // In a sheet a card lays no ground of its own (its content lies on the sheet).
    let bare = crate::sheet::inside(ui.ctx());
    block(ui, pad, 0.0, add, |r| if bare { Vec::new() } else { material::shapes(r, Radius::Card, Level::Card, 0.0) }).0
}

/// A card that opens something (a held grant on "my grants"): it lifts on hover and presses to 0.995; the
/// whole card is the target.
pub fn open_card<R>(ui: &mut egui::Ui, id: egui::Id, add: impl FnOnce(&mut egui::Ui) -> R) -> (R, egui::Response) {
    let slot = ui.painter().add(egui::Shape::Noop);
    let gid = ui.next_auto_id().with("zikaron-card");
    let (inner, rect) = crate::flow::grow(ui, gid, |ui| {
        let r = egui::Frame::new().inner_margin(egui::Margin::symmetric(tokens::CARD_PAD_X as i8, 16)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        });
        (r.inner, r.response.rect)
    });
    let resp = ui.interact(rect, id, egui::Sense::click());
    let st = motion::states(ui.ctx(), id, resp.hovered(), resp.is_pointer_button_down_on(), resp.has_focus(), true);
    let press = 1.0 + (0.995 - 1.0) * st.press;
    let r = paint::scaled(rect, press);
    ui.painter().set(slot, egui::Shape::Vec(material::shapes(r, Radius::Card, Level::Card, st.lift())));
    crate::probe::row(ui.ctx(), rect);
    (inner, resp.on_hover_cursor(egui::CursorIcon::PointingHand))
}

/// A choice block in a list (identities, networks): the chosen one on the selection ground with a wash
/// edge, the others flat with a line; 12 × 14 inside.
pub fn choice<R>(ui: &mut egui::Ui, on: bool, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let (fill, line) = if on { (c(C::Sel), c(C::AccentWash)) } else { (c(C::Surface), c(C::Line)) };
    block(ui, vec2(14.0, 12.0), 1.0, add, |r| vec![egui::epaint::RectShape::new(r, Radius::Card.egui(), fill, egui::Stroke::new(1.0_f32, line), egui::StrokeKind::Inside).into()]).0
}

/// A flat card: a one-point line instead of a shadow (side notes, the "how the recipient checks" card).
pub fn flat<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    block(ui, vec2(tokens::CARD_PAD_X, tokens::CARD_PAD_Y), 1.0, add, |r| vec![egui::epaint::RectShape::new(r, Radius::Card.egui(), c(C::Surface), egui::Stroke::new(1.0_f32, c(C::Line)), egui::StrokeKind::Inside).into()]).0
}

/// A small card title inside a card (14, semibold).
pub fn card_title(ui: &mut egui::Ui, s: &str) {
    paint::text(ui, s, Type::Card, c(C::Ink));
}

/// The title of a flat side card (13, semibold, first ink).
pub fn flat_title(ui: &mut egui::Ui, s: &str) {
    ui.label(egui::RichText::new(s).font(egui::FontId::new(Type::Small.size(), crate::fonts::strong())).color(c(C::Ink)).line_height(Some(Type::Note.line())));
}

/// A group title above a card: 13 semibold in the second ink, 4 in from the card's edge, 8 above the card.
pub fn group_title(ui: &mut egui::Ui, s: &str) {
    indented(ui, paint::wrapped(ui, s, egui::FontId::new(Type::Small.size(), crate::fonts::strong()), c(C::Ink2), Type::Small.line(), ui.available_width() - 4.0));
    ui.add_space((8.0 - ui.spacing().item_spacing.y).max(0.0));
}

/// A footnote under a card: 13 in the second ink, 4 in, 10 below the card.
pub fn group_foot(ui: &mut egui::Ui, s: &str) {
    ui.add_space((10.0 - ui.spacing().item_spacing.y).max(0.0));
    indented(ui, paint::wrapped(ui, s, Type::Small.font(), c(C::Ink2), Type::Small.line(), ui.available_width() - 4.0));
}

/// Words 4 in, in one cell of the caller's layout (`width::cell`).
fn indented(ui: &mut egui::Ui, words: std::sync::Arc<egui::Galley>) {
    let size = vec2(4.0 + words.size().x, words.size().y);
    crate::width::cell(ui, size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.add_space(4.0);
        ui.add(egui::Label::new(words));
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
    let hot = motion::states(ui.ctx(), id, resp.hovered(), resp.is_pointer_button_down_on(), resp.has_focus(), t.clickable).hover;
    let press = motion::to(ui.ctx(), id.with("press"), if t.clickable && resp.is_pointer_button_down_on() { 0.99 } else { 1.0 }, tokens::FAST, Curve::Ease);
    let r = paint::scaled(rect.translate(vec2(0.0, -hot)), press);
    let p = ui.painter();
    material::paint(p, r, Radius::Card, Level::Card, hot);
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
    let line = Rect::from_min_max(pos2(x, fy), pos2(inner.right(), fy + Type::Figure.line()));
    paint::rolling(ui, id.with("figure"), line, &fit, fig_font, colour);
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

/// A group of rows (the settings, details and overview cards): rows 56 high (48 in a sheet), 20 in from the card's
/// sides; a hairline between two rows, inset 20 on both sides; a row that opens something lights a band under the
/// pointer, inset 3 above and below and 8 from the card's sides. In a sheet the group lays no ground of its own.
pub fn form<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui, &mut Form) -> R) -> R {
    let card_slot = ui.painter().add(egui::Shape::Noop);
    let fill_slot = ui.painter().add(egui::Shape::Noop);
    let top = ui.cursor().min;
    let w = ui.available_width();
    let gid = ui.next_auto_id().with("zikaron-form");
    let label_w = ui.ctx().data(|d| d.get_temp::<f32>(gid.with("label"))).unwrap_or(0.0);
    let sheet = crate::sheet::inside(ui.ctx());
    let mut f = Form {
        rows: 0,
        width: w,
        bottom: top.y,
        fills: Vec::new(),
        lights: Vec::new(),
        lines: Vec::new(),
        id: gid,
        label_w,
        label_max: 0.0,
        top,
        group: None,
        open_row: None,
        next_key: None,
        quiet: false,
        sheet,
        next_tag: None,
        tail: 0.0,
        fold_last: false,
        corner: if sheet { ui.ctx().data(|d| d.get_temp::<f32>(crate::sheet::rows_corner_id())).unwrap_or(Radius::Card.px()) } else { Radius::Card.px() },
    };
    let (r, shown) = crate::flow::grow(ui, gid, |ui| {
        let r = ui
            .vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                add(ui, &mut f)
            })
            .inner;
        let bottom = f.bottom;
        f.close_row(ui, bottom);
        // In a sheet or under a column of fields, rows ending in a fold say where they end and the blank under the
        // fold's words (the keys under them take it off the room above them: `sheet::show`, `fields::Stack::foot`).
        if f.sheet && f.fold_last {
            let at = (f.bottom, f.tail);
            ui.ctx().data_mut(|d| d.insert_temp(crate::sheet::trailing(), at));
        }
        if let Some(g) = f.group.take() {
            let _ = g.finish(ui.ctx());
        }
        // The card is as tall as its rows.
        (r, Rect::from_min_size(top, vec2(w, (f.bottom - top.y).max(0.0))))
    });
    let rect = Rect::from_min_size(top, vec2(w, shown.height()));
    if f.label_max != f.label_w {
        ui.ctx().data_mut(|d| d.insert_temp(gid.with("label"), f.label_max));
        ui.ctx().request_discard("a card's label column changed");
    }
    crate::probe::boxed(ui.ctx(), ui.layer_id(), rect);
    if !sheet {
        ui.painter().set(card_slot, egui::Shape::Vec(material::shapes(rect, Radius::Card, Level::Card, 0.0)));
    }
    let fills: Vec<egui::Shape> = f
        .fills
        .iter()
        .map(|(r, col)| {
            let (b, round) = f.band_of(rect, *r);
            let (slot, card) = f.bounds_of(rect, *r);
            crate::probe::lit(ui.ctx(), crate::probe::Lit { block: b, corners: round, slot, card, card_corner: f.corner });
            egui::epaint::RectShape::filled(b, round, *col).into()
        })
        .chain(f.lights.iter().filter_map(|(r, at, col)| {
            let (b, round) = f.band_of(rect, *r);
            material::glow_shape_corners(b, round, *at, *col)
        }))
        .collect();
    ui.painter().set(fill_slot, egui::Shape::Vec(fills));
    r
}

/// A row's sizes: its least height (in a card, in a sheet), its padding above and below, the title's line,
/// the gap under it and the second line's line; the least width of a key in a row.
const ROW_H: f32 = 56.0;
const SHEET_ROW_H: f32 = 48.0;
const TITLE_LINE: f32 = 21.0;
const SUB_GAP: f32 = 1.0;
const KEY_MIN_W: f32 = 92.0;
const TRAIL_GAP: f32 = 8.0;
const COPY_SIZE: f32 = 15.0;

/// A row title's font: 15, medium.
fn title_font() -> egui::FontId {
    egui::FontId::new(15.0, crate::fonts::medium())
}

/// What a row's right end holds, in the second ink at 14 unless said otherwise.
fn trail_font() -> egui::FontId {
    egui::FontId::new(14.0, egui::FontFamily::Proportional)
}

/// The rows of a group being drawn.
pub struct Form {
    rows: usize,
    width: f32,
    bottom: f32,
    /// Rows lit under the pointer: the row's slot, the colour of its block.
    fills: Vec<(Rect, egui::Color32)>,
    /// The light under the pointer on the lit row: the row's slot, where the pointer is, the light's colour.
    lights: Vec<(Rect, egui::Pos2, egui::Color32)>,
    /// Where a line between two rows was drawn (its height), for the lit block to keep its gap to it.
    lines: Vec<f32>,
    id: egui::Id,
    /// The details list's label column (from last frame), and the widest seen this frame.
    label_w: f32,
    label_max: f32,
    /// The card's top left; with [`Form::keep`], the rows kept continuous, the row being drawn (its identity,
    /// top and first shape), and the identity given for the next row.
    top: egui::Pos2,
    group: Option<crate::flow::Group>,
    open_row: Option<(u64, f32, usize)>,
    next_key: Option<u64>,
    /// No line above the next row (it follows an overview's head or figures, or opens a card's tail).
    quiet: bool,
    /// The group lies in a sheet (rows 48 high).
    sheet: bool,
    /// The corner of what these rows light within: this card's, or, in a sheet or a form card, that one's.
    corner: f32,
    /// A word right after the next row's title ([`Form::tag_next`], [`Form::quiet_next`]): its words and whether
    /// it is the quiet word (else a small blue tag).
    next_tag: Option<(String, bool)>,
    /// The blank the last row keeps under what it shows (its words, key or field), where the row kind knows it; none
    /// for a row that lights (its block takes that blank).
    tail: f32,
    /// The last row is a fold row (`Form::fold_from`): in a sheet its blank goes to the keys under it.
    fold_last: bool,
}

/// What an information row shows at its right end (one right edge per card).
#[derive(Clone, Copy, Debug)]
pub enum Show<'a> {
    Text(&'a str),
    Mono(&'a str),
    /// A long value: monospace, one line, elided in the middle (whole on hover), a copy key after it.
    Copy(&'a str),
    /// An address or digest written short (its first six and last four), whole on hover, a copy key after it.
    Short(&'a str),
    /// A value that is not there (not chosen, none, never): quiet words.
    Empty(&'a str),
    /// Words, then a mark at the right end.
    Mark(Mark, &'a str),
    Pill(&'a str, crate::palette::Tone),
}

/// What a row holds at its right end.
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

/// The tone of an overview's icon tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    Ok,
    Blue,
    Warn,
    Bad,
}

/// One figure in an overview's strip: words, monospace, or words in the warning ink.
#[derive(Clone, Copy, Debug)]
pub enum Fact<'a> {
    Text(&'a str),
    Mono(&'a str),
    Warn(&'a str),
    Quiet(&'a str),
}

/// A value of the details list (the one details list).
#[derive(Clone, Debug)]
/// One details row's value. Its kind sets only the face and whether a copy key goes with it; a value with line
/// breaks is drawn a line each, left-aligned from the label's line, the row as tall as its lines, whatever its kind.
pub enum Detail {
    /// A machine value (to be pasted elsewhere): monospace, elided in the middle, a copy key at the end (one for
    /// all its lines).
    Mono(String),
    /// A figure read, not pasted (a count, a size, a date or time, a state's token): monospace, no copy key.
    Figure(String),
    /// Words: no copy key.
    Text(String),
    /// Words standing for a value that is not there (an empty seat, a value not read yet): quiet ink, and no
    /// copy key, as there is nothing to copy.
    Quiet(String),
}

/// The short form of an address or digest: its first six and last four, joined by an ellipsis.
pub fn short(v: &str) -> String {
    let n = v.chars().count();
    if n <= 13 {
        return v.to_string();
    }
    let head: String = v.chars().take(6).collect();
    let tail: String = v.chars().skip(n - 4).collect();
    format!("{head}\u{2026}{tail}")
}

/// The gap between a row's title and the tag after it.
const TAG_GAP: f32 = 10.0;

fn tag_font() -> egui::FontId {
    egui::FontId::new(Type::Micro.size(), crate::fonts::medium())
}

/// The gap between a row's title and the quiet word after it.
const QUIET_GAP: f32 = 8.0;

/// What follows a title, with the gap before it: a tag's words and 10 either side; a quiet word's words.
fn title_tag_w(ui: &egui::Ui, s: &str, quiet: bool) -> f32 {
    if quiet {
        return QUIET_GAP + ui.painter().layout_no_wrap(s.to_string(), Type::Small.font(), c(C::Ink3)).size().x;
    }
    TAG_GAP + ui.painter().layout_no_wrap(s.to_string(), tag_font(), c(C::AccentInk)).size().x + 20.0
}

/// What follows a title, its left edge `at`'s (the title's end) and its middle on `at`'s line: a tag 20 high, fully
/// round, the accent's wash with its ink, 11 medium, 10 after the title; or the quiet word, 13 in the third ink, 8
/// after it ("optional").
fn title_tag(ui: &egui::Ui, at: egui::Pos2, s: &str, quiet: bool) {
    let p = ui.painter();
    if quiet {
        p.text(pos2(at.x + QUIET_GAP, at.y), egui::Align2::LEFT_CENTER, s, Type::Small.font(), c(C::Ink3));
        return;
    }
    let g = p.layout_no_wrap(s.to_string(), tag_font(), c(C::AccentInk));
    let r = Rect::from_min_size(pos2(at.x + TAG_GAP, at.y - 10.0), vec2(g.size().x + 20.0, 20.0));
    p.rect_filled(r, egui::CornerRadius::same(10), c(C::AccentWash));
    p.galley(pos2(r.left() + 10.0, r.center().y - g.size().y / 2.0), g, c(C::AccentInk));
}

impl Form {
    /// Rows' side inset: 20 in a card; none in a sheet, whose rows run to its own padding.
    fn pad_x(&self) -> f32 {
        if self.sheet {
            0.0
        } else {
            tokens::FORM_PAD_X
        }
    }

    /// What a row's value at its right end may take: the row less its insets, its lead (`lead` wide), its title
    /// whole and the gap after it. The title is never cut for the value.
    fn value_room(&self, ui: &egui::Ui, title: &str, lead: f32) -> f32 {
        let title_w = ui.painter().layout_no_wrap(title.to_string(), title_font(), c(C::Ink)).size().x;
        (self.width - 2.0 * self.pad_x() - lead - title_w - 2.0 * TRAIL_GAP).max(40.0)
    }

    fn rule(&mut self, ui: &mut egui::Ui, top: f32) {
        self.begin(ui, top);
        let quiet = std::mem::take(&mut self.quiet);
        if self.rows > 0 && !quiet {
            let left = ui.max_rect().left();
            ui.painter().hline((left + self.pad_x())..=(left + self.width - self.pad_x()), top, egui::Stroke::new(1.0_f32, c(C::Line2)));
            self.lines.push(top);
        }
        self.rows += 1;
    }

    /// A row starts at `top` (the last one ends there).
    fn begin(&mut self, ui: &egui::Ui, top: f32) {
        self.close_row(ui, top);
        self.tail = 0.0;
        self.fold_last = false;
        if let Some(g) = &self.group {
            let k = self.next_key.take().unwrap_or_else(|| egui::Id::new(("zikaron-form-row", self.rows)).value());
            self.open_row = Some((k, top, g.start(ui.ctx())));
        }
    }

    /// The row being drawn ends at `bottom` (kept continuous with [`Form::keep`]).
    fn close_row(&mut self, ui: &egui::Ui, bottom: f32) {
        if let (Some(g), Some((k, top, start))) = (self.group.as_mut(), self.open_row.take()) {
            let rect = Rect::from_min_max(pos2(self.top.x, top), pos2(self.top.x + self.width, bottom.max(top)));
            g.row(ui.ctx(), k, rect, start);
        }
    }

    /// Keep this card's rows continuous (call before its first row): a row that comes opens in place and fades
    /// in, one that goes closes and fades where it was, and the rows under them slide one way to where they
    /// belong. Rows are told apart by [`Form::key`], else by their place in the card.
    pub fn keep(&mut self, ui: &egui::Ui) {
        if self.group.is_none() && self.rows == 0 {
            self.group = Some(crate::flow::Group::new(ui, self.id.with("keep"), self.top, self.width));
        }
    }

    /// The identity of the next row (in a card kept with [`Form::keep`]).
    pub fn key(&mut self, k: impl std::hash::Hash) {
        self.next_key = Some(egui::Id::new(k).value());
    }

    /// Whether a row that opens something is lit now (the one row under the pointer this frame, at once; none
    /// while the page scrolls): its band, and the light under the pointer, laid under it.
    fn lit(&mut self, ui: &egui::Ui, id: egui::Id, rect: Rect, resp: &egui::Response, open: bool) -> f32 {
        let on = open && crate::flow::pointer_on(ui, rect);
        if open && on && resp.is_pointer_button_down_on() {
            self.fills.push((rect, c(C::Press)));
        } else if on {
            self.fills.push((rect, c(C::Hover)));
        }
        let (glow, at) = material::glow_at(ui, id, rect, on);
        if glow > 0.0 {
            self.lights.push((rect, at, c(C::Accent).gamma_multiply(0.04 * glow)));
        }
        if on { 1.0 } else { 0.0 }
    }

    /// The lit block of the row in `slot`, in the card `card` ([`crate::band`]): bounded across by the card's
    /// edges (in a sheet, by where a card's edges would be, 20 outside the rows), above and below by the line
    /// drawn there or else the card's edge. A sheet draws no card: the ends of its list are no edge to keep a gap
    /// from, so its first and last rows are lit as tall as the rows between, each block on its row.
    fn band_of(&self, card: Rect, slot: Rect) -> (Rect, egui::CornerRadius) {
        use crate::band::Bound;
        let (slot, _) = self.bounds_of(card, slot);
        let end = if self.sheet { Bound::Line } else { Bound::Edge };
        let at = |y: f32| if self.lines.iter().any(|l| (l - y).abs() < 0.5) && (y - card.top()).abs() >= 0.5 && (y - card.bottom()).abs() >= 0.5 { Bound::Line } else { end };
        crate::band::block(slot, self.corner, at(slot.top()), at(slot.bottom()))
    }

    /// A row's slot across the edges of what it lies in (a sheet's or a form card's, its padding out from the
    /// rows), and that.
    fn bounds_of(&self, card: Rect, slot: Rect) -> (Rect, Rect) {
        let across = if self.sheet { tokens::SHEET_PAD } else { 0.0 };
        let wide = |r: Rect| Rect::from_min_max(pos2(r.left() - across, r.top()), pos2(r.right() + across, r.bottom()));
        (wide(slot), wide(card))
    }

    /// The rectangle of the next row, `h` high, under the line above it; recorded as a row of this card.
    fn place(&mut self, ui: &mut egui::Ui, h: f32, sense: egui::Sense) -> (Rect, egui::Response) {
        let top = ui.cursor().min.y;
        self.rule(ui, top);
        let rect = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(self.width, h));
        let resp = ui.allocate_rect(rect, sense);
        crate::probe::part(ui.ctx(), ui.layer_id(), rect);
        self.bottom = rect.bottom();
        (rect, resp)
    }

    /// How tall a row is with a second line or without.
    fn height(&self, sub: &str) -> f32 {
        let pad = if self.sheet { 8.0 } else { 10.0 };
        let least = if self.sheet { SHEET_ROW_H } else { ROW_H };
        let words = TITLE_LINE + if sub.is_empty() { 0.0 } else { SUB_GAP + Type::Small.line() };
        (words + pad * 2.0).max(least)
    }

    /// The title (and its second line) at the left of a row, from `x`, within `room`, centred as a pair.
    #[allow(clippy::too_many_arguments)]
    fn words(ui: &egui::Ui, rect: Rect, x: f32, title: &str, sub: &str, sub_mono: bool, room: f32, ink: egui::Color32, tag: Option<(&str, bool)>) {
        // A row's title that says again the title of the sheet it is in is not drawn (`sheet::echoes_title`).
        let title = if crate::sheet::echoes_title(ui.ctx(), title) { "" } else { title };
        let p = ui.painter();
        let font = title_font();
        let tag_w = tag.map(|(s, quiet)| title_tag_w(ui, s, quiet)).unwrap_or(0.0);
        let ty = if sub.is_empty() { rect.center().y - TITLE_LINE / 2.0 } else { rect.center().y - (TITLE_LINE + SUB_GAP + Type::Small.line()) / 2.0 };
        let fit = crate::width::elide_to(ui, title, font.clone(), (room - tag_w).max(0.0));
        let tr = p.text(pos2(x, ty + TITLE_LINE / 2.0), egui::Align2::LEFT_CENTER, fit, font, ink);
        if let Some((s, quiet)) = tag {
            title_tag(ui, pos2(tr.right(), ty + TITLE_LINE / 2.0), s, quiet);
        }
        if sub.is_empty() {
            return;
        }
        let sy = ty + TITLE_LINE + SUB_GAP + Type::Small.line() / 2.0;
        let sfont = if sub_mono { Type::MonoSub.font() } else { Type::Small.font() };
        let sfit = if sub_mono { crate::width::elide_mid(ui, sub, sfont.clone(), room) } else { crate::width::elide_to(ui, sub, sfont.clone(), room) };
        p.text(pos2(x, sy), egui::Align2::LEFT_CENTER, sfit, sfont, c(C::Ink2));
    }

    /// The one row: an optional lead (a glyph in a 28-point box, or a mark), the title and its second line, and
    /// what `trail` draws at the right end (right to left, 8 apart). `nav` makes the whole row open something
    /// (its band lights under the pointer, a chevron ends it). Returns the row's response and `trail`'s answer.
    #[allow(clippy::too_many_arguments)]
    pub fn line<R>(&mut self, ui: &mut egui::Ui, lead: Option<Lead>, title: &str, sub: &str, sub_mono: bool, nav: bool, ink: egui::Color32, trail: impl FnOnce(&mut egui::Ui) -> R) -> (egui::Response, R) {
        self.line_as(ui, lead, title, sub, sub_mono, nav, nav, ink, trail)
    }

    /// A row to choose from a list: lit and taking a press while it can be chosen (no chevron: it goes nowhere),
    /// grey and still when it cannot; the check after what `trail` draws when it is the one chosen. Returns the
    /// row's response.
    pub fn choice<R>(&mut self, ui: &mut egui::Ui, open: bool, chosen: bool, trail: impl FnOnce(&mut egui::Ui) -> R) -> (egui::Response, R) {
        let ink = if open { c(C::Ink) } else { c(C::Ink3) };
        self.line_as(ui, None, "", "", false, open, false, ink, |ui| {
            if chosen {
                let (r, _) = ui.allocate_exact_size(vec2(COPY_SIZE, COPY_SIZE), egui::Sense::hover());
                icons::glyph_at(ui.painter(), Glyph::Check, r.center(), COPY_SIZE, c(C::Accent));
            }
            trail(ui)
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn line_as<R>(&mut self, ui: &mut egui::Ui, lead: Option<Lead>, title: &str, sub: &str, sub_mono: bool, open: bool, nav: bool, ink: egui::Color32, trail: impl FnOnce(&mut egui::Ui) -> R) -> (egui::Response, R) {
        let h = self.height(sub);
        let (rect, resp) = self.place(ui, h, if open { egui::Sense::click() } else { egui::Sense::hover() });
        let hot = self.lit(ui, resp.id, rect, &resp, open);
        let x0 = rect.left() + self.pad_x();
        let right = rect.right() - self.pad_x();
        let p = ui.painter().clone();
        let mut x = x0;
        match lead {
            Some(Lead::Glyph(g)) => {
                icons::glyph_at(&p, g, pos2(x + 14.0, rect.center().y), 18.0, crate::palette::mix(c(C::Ink2), c(C::Ink), hot));
                x += 28.0 + 16.0;
            }
            Some(Lead::Mark(m)) => {
                mark::paint_mark(ui.ctx(), &p, pos2(x + 9.0, rect.center().y), m, tokens::MARK);
                x += 18.0 + 12.0;
            }
            None => {}
        }
        let mut rx = right;
        if nav {
            icons::glyph_at(&p, Glyph::Chev, pos2(rx - 7.5, rect.center().y), COPY_SIZE, c(C::Ink3));
            rx -= COPY_SIZE + TRAIL_GAP;
        }
        let ctl = Rect::from_min_max(pos2(x, rect.top()), pos2(rx, rect.bottom()));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(ctl).layout(egui::Layout::right_to_left(egui::Align::Center)));
        child.spacing_mut().item_spacing.x = TRAIL_GAP;
        let r = trail(&mut child);
        let used = child.min_rect();
        // What it shows is centred: its words, or what the trail laid if that stands taller.
        let words_h = TITLE_LINE + if sub.is_empty() { 0.0 } else { SUB_GAP + Type::Small.line() };
        self.tail = if open { 0.0 } else { ((rect.height() - words_h.max(used.height())) / 2.0).max(0.0) };
        let title_room = ((if used.width() > 1.0 { used.left() - 16.0 } else { rx }) - x).max(0.0);
        let tag = self.next_tag.take();
        Self::words(ui, rect, x, title, sub, sub_mono, title_room, ink, tag.as_ref().map(|(s, q)| (s.as_str(), *q)));
        // A row to choose takes the press anywhere on it, its words included (drawn after it, they would take it).
        let resp = if open && !nav { ui.interact(rect, resp.id.with("choose"), egui::Sense::click()) } else { resp };
        let resp = if open { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp };
        (resp, r)
    }

    /// A row: title (and note), and a value or chevron on the right. Returns the row's response (clickable
    /// when it navigates).
    pub fn row(&mut self, ui: &mut egui::Ui, title: &str, note: &str, value: Value) -> egui::Response {
        self.row_with(ui, None, title, note, value, |_| {})
    }

    /// A row with a mark before its title and a control on its right drawn by `right`.
    pub fn row_with(&mut self, ui: &mut egui::Ui, lead: Option<Mark>, title: &str, note: &str, value: Value, right: impl FnOnce(&mut egui::Ui)) -> egui::Response {
        let nav = matches!(value, Value::Nav | Value::NavWith(_));
        let off = matches!(value, Value::Off);
        let ink = if off { c(C::Ink3) } else { c(C::Ink) };
        let (resp, _) = self.line(ui, lead.map(Lead::Mark), title, note, false, nav, ink, |ui| {
            match value {
                Value::NavWith(s) | Value::Text(s) => {
                    ui.label(egui::RichText::new(s).font(trail_font()).color(c(C::Ink2)));
                }
                Value::Mono(s) => {
                    ui.label(egui::RichText::new(s).font(Type::MonoSmall.font()).color(c(C::Ink)));
                }
                Value::Off => {
                    let (r, _) = ui.allocate_exact_size(vec2(COPY_SIZE, COPY_SIZE), egui::Sense::hover());
                    icons::glyph_at(ui.painter(), Glyph::Chev, r.center(), COPY_SIZE, c(C::Ink3).gamma_multiply(tokens::OFF));
                }
                Value::None | Value::Nav => {}
            }
            right(ui);
        });
        resp
    }

    /// An information row: the name at the left, the value at the right edge. A long value (`Show::Copy`,
    /// `Show::Short`) has a copy key after it; returns the words copied when that key is pressed.
    pub fn info(&mut self, ui: &mut egui::Ui, title: &str, show: Show) -> Option<String> {
        let room = self.value_room(ui, title, 0.0);
        self.line(ui, None, title, "", false, false, c(C::Ink), |ui| show_value(ui, show, room)).1
    }

    /// An action row: the name (and its state on a second line) at the left, one key at the right edge, at least
    /// 92 wide. Returns the key's response.
    pub fn act(&mut self, ui: &mut egui::Ui, title: &str, sub: &str, k: crate::button::Key) -> egui::Response {
        self.line(ui, None, title, sub, false, false, c(C::Ink), |ui| crate::button::show_w(ui, k, KEY_MIN_W)).1
    }

    /// An action row with a value before its key (usage before "measure"): returns the key's response.
    pub fn act_value(&mut self, ui: &mut egui::Ui, title: &str, sub: &str, value: &str, k: crate::button::Key) -> egui::Response {
        self.line(ui, None, title, sub, false, false, c(C::Ink), |ui| {
            let r = crate::button::show_w(ui, k, KEY_MIN_W);
            ui.label(egui::RichText::new(value).font(trail_font()).color(c(C::Ink2)));
            r
        })
        .1
    }

    /// An action row whose second line is a path or address (monospace, 12.5, elided in the middle, whole on
    /// hover): returns the key's response.
    pub fn act_path(&mut self, ui: &mut egui::Ui, title: &str, path: &str, k: crate::button::Key) -> egui::Response {
        let (row, key) = self.line(ui, None, title, path, true, false, c(C::Ink), |ui| crate::button::show_w(ui, k, KEY_MIN_W));
        let _ = crate::layer::tip(row, path);
        key
    }

    /// A switch row: the name (and what the setting does now on a second line) at the left, the switch at the
    /// right edge. Clicking the switch flips it; returns whether it flipped this frame.
    pub fn switch(&mut self, ui: &mut egui::Ui, title: &str, sub: &str, on: bool, enabled: bool) -> bool {
        self.line(ui, None, title, sub, false, false, c(C::Ink), |ui| crate::toggle::switch(ui, on, enabled).clicked()).1
    }

    /// A row with any control at its right end (a segmented control, a menu key, a small input with its unit and
    /// key): returns what `right` answers.
    pub fn with<R>(&mut self, ui: &mut egui::Ui, title: &str, sub: &str, right: impl FnOnce(&mut egui::Ui) -> R) -> R {
        self.line(ui, None, title, sub, false, false, c(C::Ink), right).1
    }

    /// A navigation row: the name, the current value in words and a chevron at the right; the whole row opens
    /// it. Off: dimmed, not clickable (why on hover).
    /// The next row's title carries a small blue tag right after it (the wizard's "recommended").
    pub fn tag_next(&mut self, tag: &str) {
        self.next_tag = Some((tag.to_string(), false));
    }

    /// The next row's title carries a quiet word right after it ("optional"), within the title's room.
    pub fn quiet_next(&mut self, word: &str) {
        self.next_tag = Some((word.to_string(), true));
    }

    pub fn go(&mut self, ui: &mut egui::Ui, title: &str, value: &str, enabled: bool, why: &str) -> egui::Response {
        let v = if !enabled { Value::Off } else if value.is_empty() { Value::Nav } else { Value::NavWith(value) };
        let resp = self.row(ui, title, "", v);
        if !enabled && !why.is_empty() { crate::layer::tip(resp, why) } else { resp }
    }

    /// A sheet's place row: a folder glyph, the name, and the folder chosen at the right end (monospace,
    /// elided in the middle, whole on hover), or `none` in the quiet ink.
    pub fn folder(&mut self, ui: &mut egui::Ui, label: &str, path: &str, none: &str) {
        let room = self.value_room(ui, label, 28.0 + 16.0);
        let _ = self.line(ui, Some(Lead::Glyph(Glyph::Folder)), label, "", false, false, c(C::Ink), |ui| {
            if path.trim().is_empty() {
                ui.label(egui::RichText::new(none).font(trail_font()).color(c(C::Ink3)));
            } else {
                let font = Type::MonoSmall.font();
                let shown = crate::width::elide_mid(ui, path, font.clone(), room);
                let r = ui.label(egui::RichText::new(&shown).font(font).color(c(C::Ink)));
                if shown != path {
                    let _ = crate::layer::tip(r, path);
                }
            }
        });
    }

    /// A navigation row with a second line (a read-only network's chain): the whole row opens it.
    pub fn go_sub(&mut self, ui: &mut egui::Ui, title: &str, sub: &str) -> egui::Response {
        self.line(ui, None, title, sub, false, true, c(C::Ink), |_| ()).0
    }

    /// A navigation row whose title is in the accent ink (the last row that adds one more: "add network"): no
    /// chevron.
    pub fn add_row(&mut self, ui: &mut egui::Ui, title: &str) -> egui::Response {
        let h = self.height("");
        let (rect, resp) = self.place(ui, h, egui::Sense::click());
        let _ = self.lit(ui, resp.id, rect, &resp, true);
        let x = rect.left() + self.pad_x();
        Self::words(ui, rect, x, title, "", false, rect.width() - 2.0 * self.pad_x(), c(C::AccentInk), None);
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    /// A navigation row whose value is shown as an information row shows it (a pill, a mark, quiet words),
    /// before a chevron; the whole row opens it.
    pub fn go_show(&mut self, ui: &mut egui::Ui, title: &str, show: Show) -> egui::Response {
        let room = self.width * 0.5;
        self.line(ui, None, title, "", false, true, c(C::Ink), |ui| show_value(ui, show, room)).0
    }

    /// A navigation row in red words (delete this identity, delete this record) or in the pen's ink (revoke): the
    /// whole row opens its confirmation sheet.
    pub fn danger(&mut self, ui: &mut egui::Ui, title: &str) -> egui::Response {
        self.line(ui, None, title, "", false, true, c(C::BadInk), |_| ()).0
    }

    /// As [`Form::danger`], in the pen's ink (revoking a grant).
    pub fn pen(&mut self, ui: &mut egui::Ui, title: &str) -> egui::Response {
        self.line(ui, None, title, "", false, true, c(C::Pen), |_| ()).0
    }

    /// A fold row: the name at the left, what it holds now in words at the right, a chevron that turns; the
    /// rows it holds open under it inside the card, their height easing in and out.
    pub fn fold<R>(&mut self, ui: &mut egui::Ui, id_salt: &str, title: &str, summary: &str, add: impl FnOnce(&mut egui::Ui, &mut Form) -> R) -> Option<R> {
        self.fold_from(ui, id_salt, title, summary, false, add)
    }

    /// A fold row whose open state the page holds (`open`): the page may open or close it, and a click on the
    /// row is written back.
    pub fn fold_held<R>(&mut self, ui: &mut egui::Ui, id_salt: &str, title: &str, summary: &str, open: &mut bool, add: impl FnOnce(&mut egui::Ui, &mut Form) -> R) -> Option<R> {
        let id = self.id.with(("fold", id_salt));
        ui.data_mut(|d| d.insert_temp(id, *open));
        let r = self.fold_from(ui, id_salt, title, summary, *open, add);
        *open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(*open);
        r
    }

    /// A fold row that starts open when `open_first` (until it is clicked).
    pub fn fold_from<R>(&mut self, ui: &mut egui::Ui, id_salt: &str, title: &str, summary: &str, open_first: bool, add: impl FnOnce(&mut egui::Ui, &mut Form) -> R) -> Option<R> {
        let id = self.id.with(("fold", id_salt));
        let h = self.height("");
        let (rect, resp) = self.place(ui, h, egui::Sense::click());
        let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(open_first);
        if resp.clicked() {
            open = !open;
            if open {
                let now = motion::now(ui.ctx());
                ui.data_mut(|d| d.insert_temp(id.with("opened-at"), now));
            }
        }
        ui.data_mut(|d| d.insert_temp(id, open));
        let t = motion::flag(ui.ctx(), id.with("t"), open, tokens::SLOW);
        // A fold is not a list's row: it lights no block under the pointer, a press opens it.
        let hot = self.lit(ui, resp.id, rect, &resp, false);
        let p = ui.painter().clone();
        let right = rect.right() - self.pad_x();
        icons::draw_glyph_turned(&p, Glyph::Chev, Rect::from_center_size(pos2(right - 7.5, rect.center().y), vec2(COPY_SIZE, COPY_SIZE)), c(C::Ink3), t * std::f32::consts::FRAC_PI_2);
        let x = rect.left() + self.pad_x();
        // The title measured in the face it is drawn in; the summary takes what the title leaves, and the title
        // what the summary then takes.
        let title_w = ui.painter().layout_no_wrap(title.to_string(), title_font(), c(C::Ink)).size().x;
        let lane = (right - COPY_SIZE - TRAIL_GAP - x).max(0.0);
        let room = (lane - title_w - 16.0).max(0.0);
        let summary_w = if summary.is_empty() {
            0.0
        } else {
            paint::at(&p, ui, pos2(right - COPY_SIZE - TRAIL_GAP, rect.center().y), egui::Align2::RIGHT_CENTER, summary, Type::Note, crate::palette::mix(c(C::Ink2), c(C::Ink), hot), room).width() + 16.0
        };
        Self::words(ui, rect, x, title, "", false, (lane - summary_w).max(0.0), c(C::Ink), None);
        let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let row_tail = ((h - TITLE_LINE) / 2.0).max(0.0);
        if t <= 0.0 {
            self.tail = row_tail;
            self.fold_last = true;
            return None;
        }
        // The rows it holds: in this card, fading with the fold and taking their height with it (open and close
        // alike), drawn in a child of their own and clipped to what they have taken; what follows in the card is
        // placed under what they take, and the card moves with them (`flow::mark_moving`), never after them.
        let ctx = ui.ctx().clone();
        let layer = ui.layer_id();
        let start = crate::flow::mark(&ctx, layer);
        let top0 = ui.cursor().min;
        let w = ui.available_width();
        let grow = if motion::reduced(&ctx) { if open { 1.0 } else { 0.0 } } else { t };
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(top0, vec2(w, f32::INFINITY))).layout(egui::Layout::top_down(egui::Align::Min)));
        child.spacing_mut().item_spacing.y = 0.0;
        child.multiply_opacity(t);
        let mut inner = Form {
            rows: 1,
            width: self.width,
            bottom: top0.y,
            fills: Vec::new(),
            lights: Vec::new(),
            lines: Vec::new(),
            id: id.with("rows"),
            label_w: self.label_w,
            label_max: 0.0,
            top: top0,
            group: None,
            open_row: None,
            next_key: None,
            quiet: false,
            sheet: self.sheet,
            next_tag: None,
            tail: 0.0,
            fold_last: false,
            corner: self.corner,
        };
        let r = add(&mut child, &mut inner);
        let laid = (inner.bottom.max(child.min_rect().bottom()) - top0.y).max(0.0);
        let take = if grow >= 1.0 { laid } else { laid * grow };
        self.label_max = self.label_max.max(inner.label_max);
        if grow >= 1.0 {
            // Lit rows are drawn only while the rows stand whole.
            self.fills.extend(inner.fills);
            self.lights.extend(inner.lights);
        } else {
            crate::flow::clip_below(&ctx, layer, start, top0.y + take);
            crate::flow::mark_moving(&ctx);
        }
        self.lines.extend(inner.lines.into_iter().filter(|y| *y <= top0.y + take + 0.5));
        ui.advance_cursor_after_rect(Rect::from_min_size(top0, vec2(w, take)));
        self.bottom = self.bottom.max(top0.y + take);
        self.rows += 1;
        // The blank under it: the row's while the rows are still to come, their last one's once they stand.
        self.tail = row_tail * (1.0 - grow) + inner.tail * grow;
        self.fold_last = true;
        // Opened by a press just now: the page follows the rows as they come, as a fold's body does
        // ([`crate::fold::reveal`]: their foot 32 above the bottom, the head never past the top).
        let now = motion::now(&ctx);
        let opened = ui.data(|d| d.get_temp::<f64>(id.with("opened-at")));
        if open && opened.is_some_and(|at| now >= at && ((now - at) as f32) < tokens::SLOW + 0.05) {
            crate::fold::reveal(ui, Rect::from_min_max(rect.min, pos2(rect.right(), top0.y + take + crate::fold::REVEAL_FOOT)));
        }
        Some(r)
    }

    /// The details fold: a row named `title` that opens the one details list — the label column as wide as
    /// the widest label (both languages measured by the caller's labels), each value in the face its kind gives
    /// ([`Detail`]: a machine value monospace and elided in the middle, whole on hover, with a copy key at the row's
    /// end; a figure monospace; words in the body face), a line between rows, a value with line breaks a line each
    /// from the label's line. Returns the words copied.
    pub fn details(&mut self, ui: &mut egui::Ui, id_salt: &str, title: &str, rows: &[(&str, Detail)], widest: f32) -> Option<String> {
        self.fold(ui, id_salt, title, "", |ui, f| f.detail_rows(ui, id_salt, rows, widest)).flatten()
    }

    /// [`Form::details`] whose open state the page holds (`open`, as [`Form::fold_held`]).
    pub fn details_held(&mut self, ui: &mut egui::Ui, id_salt: &str, title: &str, rows: &[(&str, Detail)], widest: f32, open: &mut bool) -> Option<String> {
        self.fold_held(ui, id_salt, title, "", open, |ui, f| f.detail_rows(ui, id_salt, rows, widest)).flatten()
    }

    /// The details list's rows themselves (inside a fold, or open in a sheet).
    pub fn detail_rows(&mut self, ui: &mut egui::Ui, id_salt: &str, rows: &[(&str, Detail)], widest: f32) -> Option<String> {
        let col = widest.ceil();
        let mut copied = None;
        for (i, (label, v)) in rows.iter().enumerate() {
            // A line a break, for every kind alike.
            let lines: Vec<&str> = match v {
                Detail::Mono(s) | Detail::Figure(s) | Detail::Text(s) | Detail::Quiet(s) => s.split('\n').collect(),
            };
            let quiet = matches!(v, Detail::Quiet(_));
            let text = matches!(v, Detail::Text(_)) || quiet;
            // Only a machine value is taken elsewhere: a copy key goes with it and nothing else.
            let copies = matches!(v, Detail::Mono(_)) && !lines.iter().all(|l| l.trim().is_empty());
            let line_h = if lines.len() > 1 { 20.0 } else { 22.0 };
            let h = (lines.len() as f32 * line_h + 20.0).max(44.0);
            let (rect, _) = self.place(ui, h, egui::Sense::hover());
            let x = rect.left() + self.pad_x();
            let right = rect.right() - self.pad_x();
            let p = ui.painter().clone();
            let first_y = rect.top() + 10.0 + line_h / 2.0;
            let ly = if lines.len() > 1 { first_y } else { rect.center().y };
            p.text(pos2(x, ly), egui::Align2::LEFT_CENTER, *label, egui::FontId::new(14.0, egui::FontFamily::Proportional), c(C::Ink2));
            let key = Rect::from_center_size(pos2(right - (COPY_SIZE + 4.0) / 2.0, ly), vec2(COPY_SIZE + 4.0, COPY_SIZE + 4.0));
            let whole = lines.join("\n");
            if copies {
                let kr = ui.interact(key, self.id.with((id_salt, "copy", i)), egui::Sense::click());
                if kr.hovered() {
                    p.rect_filled(key, egui::CornerRadius::same(6), c(C::Tone));
                }
                icons::glyph_at(&p, Glyph::Copy, key.center(), COPY_SIZE, if kr.hovered() { c(C::Ink) } else { c(C::Ink3) });
                if kr.clicked() {
                    ui.ctx().copy_text(whole.clone());
                    copied = Some(whole.clone());
                }
            }
            let vx = x + col + 28.0;
            let room = (if copies { key.left() - 28.0 } else { right } - vx).max(20.0);
            let font = if text { egui::FontId::new(14.0, egui::FontFamily::Proportional) } else { Type::MonoSmall.font() };
            for (k, l) in lines.iter().enumerate() {
                let y = if lines.len() > 1 { first_y + k as f32 * line_h } else { rect.center().y };
                let shown = if text { crate::width::elide_to(ui, l, font.clone(), room) } else { crate::width::elide_mid(ui, l, font.clone(), room) };
                let at = p.text(pos2(vx, y), egui::Align2::LEFT_CENTER, &shown, font.clone(), if quiet { c(C::Ink3) } else { c(C::Ink) });
                if shown != *l {
                    let _ = crate::layer::tip(ui.interact(at, self.id.with((id_salt, "whole", i, k)), egui::Sense::hover()), *l);
                }
            }
        }
        copied
    }

    /// The head of an overview card: an icon in a tinted tile, the thing's name large with its state after
    /// it, a line under it, and its keys at the right (`keys`, right to left). Returns what `keys` answers.
    #[allow(clippy::too_many_arguments)]
    pub fn summary<R>(&mut self, ui: &mut egui::Ui, icon: Glyph, tint: Tint, title: &str, state: Option<(Mark, &str)>, sub: &str, keys: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let words = 24.0 + if sub.is_empty() { 0.0 } else { Type::Small.line() };
        let h = 18.0 * 2.0 + words.max(40.0);
        let top = ui.cursor().min.y;
        self.begin(ui, top);
        self.rows += 1;
        let rect = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(self.width, h));
        ui.allocate_rect(rect, egui::Sense::hover());
        crate::probe::part(ui.ctx(), ui.layer_id(), rect);
        self.bottom = rect.bottom();
        self.quiet = true;
        let x = rect.left() + self.pad_x();
        let right = rect.right() - self.pad_x();
        let p = ui.painter().clone();
        let tile = Rect::from_min_size(pos2(x, rect.center().y - 20.0), vec2(40.0, 40.0));
        let (wash, ink) = match tint {
            Tint::Ok => (c(C::OkWash), c(C::Ok)),
            Tint::Blue => (c(C::AccentWash), c(C::Accent)),
            Tint::Warn => (c(C::WarnWash), c(C::Warn)),
            Tint::Bad => (c(C::BadWash), c(C::Bad)),
        };
        p.rect_filled(tile, egui::CornerRadius::same(12), wash);
        icons::glyph_at(&p, icon, tile.center(), 21.0, ink);
        let kr = Rect::from_min_max(pos2(tile.right() + 14.0, rect.top()), pos2(right, rect.bottom()));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(kr).layout(egui::Layout::right_to_left(egui::Align::Center)));
        child.spacing_mut().item_spacing.x = TRAIL_GAP;
        let r = keys(&mut child);
        let used = child.min_rect();
        let tx = tile.right() + 14.0;
        let room = ((if used.width() > 1.0 { used.left() - 14.0 } else { right }) - tx).max(0.0);
        let font = egui::FontId::new(17.0, crate::fonts::strong());
        let ty = rect.center().y - words / 2.0;
        // The state after the title is laid within the same room: the title gives way to it.
        let state_w = state.map(|(_, w)| {
            let dot = p.layout_no_wrap(" \u{b7} ".to_string(), font.clone(), c(C::Ink)).size().x;
            let sw = p.layout_no_wrap(w.to_string(), egui::FontId::new(15.0, crate::fonts::strong()), c(C::Ink)).size().x;
            dot + 16.0 + 6.0 + sw
        });
        let fit = crate::width::elide_to(ui, title, font.clone(), (room - state_w.unwrap_or(0.0)).max(0.0));
        let g = p.layout_no_wrap(fit, font.clone(), c(C::Ink));
        let tw = g.size().x;
        p.galley(pos2(tx, ty + 12.0 - g.size().y / 2.0), g, c(C::Ink));
        if let Some((m, w)) = state {
            let sx = tx + tw;
            let dot = p.layout_no_wrap(" \u{b7} ".to_string(), font.clone(), c(C::Ink));
            let dw = dot.size().x;
            p.galley(pos2(sx, ty + 12.0 - dot.size().y / 2.0), dot, c(C::Ink));
            mark::paint_mark(ui.ctx(), &p, pos2(sx + dw + 8.0, ty + 12.0), m, 16.0);
            let sw = p.layout_no_wrap(w.to_string(), egui::FontId::new(15.0, crate::fonts::strong()), c(C::Ink));
            p.galley(pos2(sx + dw + 16.0 + 6.0, ty + 12.0 - sw.size().y / 2.0), sw, c(C::Ink));
        }
        if !sub.is_empty() {
            paint::at(&p, ui, pos2(tx, ty + 24.0 + Type::Small.line() / 2.0), egui::Align2::LEFT_CENTER, sub, Type::Small, c(C::Ink2), room);
        }
        r
    }

    /// An overview card's strip of figures under its head: equal cells, a name over each figure (each on a line
    /// of 22, 2 apart, 16 under), a hairline between two cells the strip's full height; no line above.
    pub fn facts(&mut self, ui: &mut egui::Ui, facts: &[(&str, Fact)]) {
        let h = 22.0 + 2.0 + 22.0 + 16.0;
        let top = ui.cursor().min.y;
        self.begin(ui, top);
        self.rows += 1;
        let rect = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(self.width, h));
        ui.allocate_rect(rect, egui::Sense::hover());
        crate::probe::part(ui.ctx(), ui.layer_id(), rect);
        self.bottom = rect.bottom();
        self.quiet = true;
        let n = facts.len().max(1) as f32;
        let cw = rect.width() / n;
        let p = ui.painter().clone();
        for (i, (label, v)) in facts.iter().enumerate() {
            let x0 = rect.left() + cw * i as f32;
            if i > 0 {
                p.vline(x0, rect.top()..=rect.bottom(), egui::Stroke::new(1.0_f32, c(C::Line2)));
            }
            let x = x0 + 20.0;
            let room = (cw - 40.0).max(0.0);
            paint::at(&p, ui, pos2(x, rect.top() + 11.0), egui::Align2::LEFT_CENTER, label, Type::Tiny, c(C::Ink2), room);
            let (s, font, ink) = match v {
                Fact::Text(s) => (*s, egui::FontId::new(15.0, crate::fonts::strong()), c(C::Ink)),
                Fact::Mono(s) => (*s, egui::FontId::new(14.0, egui::FontFamily::Monospace), c(C::Ink)),
                Fact::Warn(s) => (*s, egui::FontId::new(15.0, crate::fonts::strong()), c(C::WarnInk)),
                Fact::Quiet(s) => (*s, egui::FontId::new(15.0, crate::fonts::strong()), c(C::Ink3)),
            };
            let fit = crate::width::elide_to(ui, s, font.clone(), room);
            p.text(pos2(x, rect.top() + 24.0 + 11.0), egui::Align2::LEFT_CENTER, fit, font, ink);
        }
    }

    /// An overview head without its icon tile: the name, a line under it after a mark, at most one key. Returns that
    /// key's response.
    pub fn head(&mut self, ui: &mut egui::Ui, title: &str, sub: &str, mark: Option<Mark>, k: Option<crate::button::Key>) -> Option<egui::Response> {
        let line = mark.map(|m| (m, sub));
        let mut resp = None;
        let shown_sub = if mark.is_some() { "" } else { sub };
        self.summary(ui, Glyph::Info, Tint::Blue, title, None, shown_sub, |ui| {
            if let Some(k) = k {
                resp = Some(crate::button::show(ui, k));
            }
        });
        if let Some((m, s)) = line.filter(|(_, s)| !s.is_empty()) {
            let _ = self.info(ui, "", Show::Mark(m, s));
        }
        resp
    }

    /// A field row whose control takes several lines: the name level with the control's first line, the row as tall
    /// as the control. Returns `add`'s answer.
    pub fn field_tall<R>(&mut self, ui: &mut egui::Ui, label: &str, optional: Option<&str>, add: impl FnOnce(&mut egui::Ui, f32) -> R) -> R {
        let label_g = paint::galley(ui, label, Type::Body, c(C::Ink));
        let opt_w = optional.map(|o| tokens::S1 + paint::galley(ui, o, Type::Small, c(C::Ink3)).size().x).unwrap_or(0.0);
        let col = (label_g.size().x + opt_w).ceil().clamp(tokens::LABEL_W - tokens::FORM_PAD_X, tokens::LABEL_MAX_W);
        let top = ui.cursor().min.y;
        self.rule(ui, top);
        let left = ui.max_rect().left();
        let x = left + self.pad_x();
        let right = left + self.width - self.pad_x();
        let cx = x + col + tokens::S4;
        let ctl_top = top + 9.0;
        let w = (right - cx).max(40.0);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(cx, ctl_top), vec2(w, 4096.0))).layout(egui::Layout::top_down(egui::Align::Min)));
        let r = add(&mut child, w);
        let ch = child.min_rect().height().max(tokens::INPUT_H);
        let rect = Rect::from_min_size(pos2(left, top), vec2(self.width, (ch + 18.0).max(self.height(""))));
        ui.allocate_rect(rect, egui::Sense::hover());
        crate::probe::part(ui.ctx(), ui.layer_id(), rect);
        self.bottom = rect.bottom();
        let p = ui.painter().clone();
        let ly = ctl_top + tokens::INPUT_H / 2.0;
        let lw = label_g.size().x;
        p.galley(pos2(x, ly - label_g.size().y / 2.0), label_g, c(C::Ink));
        if let Some(o) = optional {
            p.text(pos2(x + lw + tokens::S1, ly), egui::Align2::LEFT_CENTER, o, Type::Small.font(), c(C::Ink3));
        }
        r
    }

    /// A navigation row led by a line glyph (the settings home): the glyph in its 28-point box, the name, a
    /// chevron; the whole row opens it.
    pub fn go_glyph(&mut self, ui: &mut egui::Ui, glyph: Glyph, title: &str) -> egui::Response {
        let (resp, _) = self.line(ui, Some(Lead::Glyph(glyph)), title, "", false, true, c(C::Ink), |_| ());
        crate::probe::row(ui.ctx(), resp.rect);
        resp
    }

    /// A field row: the label at the left in the card's label column, the control after it.
    pub fn field<R>(&mut self, ui: &mut egui::Ui, label: &str, optional: Option<&str>, check: Option<(&str, bool)>, add: impl FnOnce(&mut egui::Ui, f32) -> R) -> R {
        let label_g = paint::galley(ui, label, Type::Body, c(C::Ink));
        let opt_w = optional.map(|o| tokens::S1 + paint::galley(ui, o, Type::Small, c(C::Ink3)).size().x).unwrap_or(0.0);
        let col = (label_g.size().x + opt_w).ceil().clamp(tokens::LABEL_W - tokens::FORM_PAD_X, tokens::LABEL_MAX_W);
        let check_h = check.filter(|(s, _)| !s.is_empty()).map(|_| tokens::S1 + Type::Small.line()).unwrap_or(0.0);
        let h = (tokens::INPUT_H + 18.0 + check_h).max(self.height(""));
        let (rect, _) = self.place(ui, h, egui::Sense::hover());
        let x = rect.left() + self.pad_x();
        let right = rect.right() - self.pad_x();
        let cx = x + col + tokens::S4;
        let ctl_top = rect.top() + 9.0;
        let p = ui.painter().clone();
        let ly = ctl_top + tokens::INPUT_H / 2.0;
        let lw = label_g.size().x;
        p.galley(pos2(x, ly - label_g.size().y / 2.0), label_g, c(C::Ink));
        if let Some(o) = optional {
            p.text(pos2(x + lw + tokens::S1, ly), egui::Align2::LEFT_CENTER, o, Type::Small.font(), c(C::Ink3));
        }
        let w = (right - cx).max(40.0);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(cx, ctl_top), vec2(w, tokens::INPUT_H))).layout(egui::Layout::left_to_right(egui::Align::Center)));
        child.spacing_mut().item_spacing.x = tokens::S2;
        let r = add(&mut child, w);
        if let Some((s, bad)) = check.filter(|(s, _)| !s.is_empty()) {
            paint::at(&p, ui, pos2(cx, ctl_top + tokens::INPUT_H + tokens::S1), egui::Align2::LEFT_TOP, s, Type::Small, if bad { c(C::BadInk) } else { c(C::Ink2) }, w);
        }
        r
    }

    /// A row of keys at the right end (a card's foot): no line above it.
    pub fn keys<R>(&mut self, ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let w = self.width;
        let top = ui.cursor().min.y;
        self.begin(ui, top);
        self.rows += 1;
        let rect = Rect::from_min_size(pos2(ui.max_rect().left(), top), vec2(w, tokens::KEY_H + 18.0 + 4.0));
        ui.allocate_rect(rect, egui::Sense::hover());
        crate::probe::part(ui.ctx(), ui.layer_id(), rect);
        let inner = Rect::from_min_max(pos2(rect.left() + self.pad_x(), rect.top() + 4.0), pos2(rect.right() - self.pad_x(), rect.bottom() - 18.0));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::right_to_left(egui::Align::Center)));
        child.spacing_mut().item_spacing.x = tokens::S2;
        let r = add(&mut child);
        self.bottom = rect.bottom();
        self.tail = rect.bottom() - inner.bottom();
        r
    }

    /// A row holding any content, padded like the others.
    pub fn free<R>(&mut self, ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let top = ui.cursor().min.y;
        self.rule(ui, top);
        let shown = egui::Frame::new().inner_margin(egui::Margin::symmetric(self.pad_x() as i8, 10)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(self.height("") - 20.0);
            add(ui)
        });
        self.bottom = shown.response.rect.bottom();
        self.tail = 10.0;
        crate::probe::part(ui.ctx(), ui.layer_id(), shown.response.rect);
        shown.inner
    }

    /// A panel under the row above it (a hairline between): the card's own padding at the sides, 16 above and 18
    /// below; what `add` lays (fields with their names above them, say).
    pub fn panel<R>(&mut self, ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let top = ui.cursor().min.y;
        self.rule(ui, top);
        let shown = egui::Frame::new().inner_margin(egui::Margin { left: self.pad_x() as i8, right: self.pad_x() as i8, top: 16, bottom: 18 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        });
        self.bottom = shown.response.rect.bottom();
        self.tail = 18.0;
        crate::probe::part(ui.ctx(), ui.layer_id(), shown.response.rect);
        shown.inner
    }

    /// A short paragraph in the card's own padding (a reason kept because it matters): quiet words, wrapping.
    pub fn note(&mut self, ui: &mut egui::Ui, s: &str) {
        self.free(ui, |ui| paint::text(ui, s, Type::Small, c(C::Ink2)));
    }

    /// A large navigation row (72 high, title and small note, chevron).
    pub fn nav_big(&mut self, ui: &mut egui::Ui, title: &str, note: &str) -> egui::Response {
        self.big(ui, None, title, note, Some(""))
    }

    /// A large row (alerts): an optional mark before a title and a small note. With `go` the whole row opens
    /// something: the words (if any) and a chevron stand on the right.
    pub fn big(&mut self, ui: &mut egui::Ui, lead: Option<Mark>, title: &str, note: &str, go: Option<&str>) -> egui::Response {
        let nav = go.is_some();
        let words = go.filter(|w| !w.is_empty());
        let (resp, _) = self.line(ui, lead.map(Lead::Mark), title, note, false, nav, c(C::Ink), |ui| {
            if let Some(w) = words {
                ui.label(egui::RichText::new(w).font(trail_font()).color(c(C::Ink2)));
            }
        });
        if nav {
            crate::probe::row(ui.ctx(), resp.rect);
        }
        resp
    }
}

/// What leads a row: a line glyph in its box, or a mark.
#[derive(Clone, Copy, Debug)]
pub enum Lead {
    Glyph(Glyph),
    Mark(Mark),
}


/// A value drawn at the right end of a row (in `ui`, laid right to left); the words copied when its copy key
/// is pressed.
fn show_value(ui: &mut egui::Ui, show: Show, room: f32) -> Option<String> {
    let mut copied = None;
    let mono = Type::MonoSmall.font();
    let mut copy_key = |ui: &mut egui::Ui, v: &str| {
        let (r, resp) = ui.allocate_exact_size(vec2(COPY_SIZE + 4.0, COPY_SIZE + 4.0), egui::Sense::click());
        let hot = resp.hovered();
        if hot {
            ui.painter().rect_filled(r, egui::CornerRadius::same(6), c(C::Tone));
        }
        icons::glyph_at(ui.painter(), Glyph::Copy, r.center(), COPY_SIZE, if hot { c(C::Ink) } else { c(C::Ink3) });
        if resp.clicked() {
            ui.ctx().copy_text(v.to_string());
            copied = Some(v.to_string());
        }
        let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    };
    match show {
        Show::Text(v) => {
            ui.label(egui::RichText::new(crate::width::elide_to(ui, v, trail_font(), room)).font(trail_font()).color(c(C::Ink2)));
        }
        Show::Empty(v) => {
            ui.label(egui::RichText::new(v).font(trail_font()).color(c(C::Ink3)));
        }
        Show::Mono(v) => {
            ui.label(egui::RichText::new(crate::width::elide_mid(ui, v, mono.clone(), room)).font(mono).color(c(C::Ink)));
        }
        Show::Copy(v) => {
            copy_key(ui, v);
            // The copy key is the glyph and 4 round it; a point more for rounding.
            let shown = crate::width::elide_mid(ui, v, mono.clone(), (room - (COPY_SIZE + 4.0) - TRAIL_GAP - 1.0).max(20.0));
            let r = ui.label(egui::RichText::new(&shown).font(mono).color(c(C::Ink)));
            if shown != v {
                let _ = crate::layer::tip(r, v);
            }
        }
        Show::Short(v) => {
            copy_key(ui, v);
            let r = ui.label(egui::RichText::new(short(v)).font(mono).color(c(C::Ink)));
            let _ = crate::layer::tip(r, v);
        }
        Show::Mark(m, v) => {
            let (r, _) = ui.allocate_exact_size(vec2(tokens::MARK, tokens::MARK), egui::Sense::hover());
            mark::paint_mark(ui.ctx(), ui.painter(), r.center(), m, tokens::MARK);
            if !v.is_empty() {
                ui.label(egui::RichText::new(crate::width::elide_to(ui, v, trail_font(), (room - tokens::MARK - TRAIL_GAP).max(20.0))).font(trail_font()).color(c(C::Ink2)));
            }
        }
        Show::Pill(v, tone) => {
            mark::pill(ui, v, tone);
        }
    }
    copied
}


/// A pill at the head of a detail page; `live` spins it.
pub struct HeroPill {
    pub words: String,
    pub tone: crate::palette::Tone,
    pub live: bool,
}

/// The head of a detail page: a large title (24/32 bold), a line under it, and its pills on the right on the
/// title's middle, the first rightmost; flush with the page's edges, 2 above and 6 below.
pub fn hero(ui: &mut egui::Ui, title: &str, sub: &str, struck: bool, pills: &[HeroPill]) -> Rect {
    egui::Frame::new().inner_margin(egui::Margin { left: 0, right: 0, top: 2, bottom: 6 }).show(ui, |ui| hero_row(ui, title, sub, struck, pills)).response.rect
}

fn hero_row(ui: &mut egui::Ui, title: &str, sub: &str, struck: bool, pills: &[HeroPill]) -> Rect {
    let w = ui.available_width();
    // The pills' room is what they measure: a pill narrows to the room it is given.
    let gap = ui.spacing().item_spacing.x;
    let slot = pills.iter().map(|p| mark::pill_w(ui, &p.words, p.live)).sum::<f32>() + gap * pills.len().saturating_sub(1) as f32;
    let r = ui
        .allocate_ui_with_layout(vec2(w, 0.0), egui::Layout::right_to_left(egui::Align::Min), |ui| {
            // The pills on the title's middle, not the middle of title and sub together.
            ui.allocate_ui_with_layout(vec2(slot, 32.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                for p in pills {
                    if p.live {
                        mark::pill_live(ui, &p.words, p.tone);
                    } else {
                        mark::pill(ui, &p.words, p.tone);
                    }
                }
            });
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
