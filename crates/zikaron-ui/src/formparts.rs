//! Parts of forms and pages beyond the fields and rows: the callout (an amber note with its info glyph), the page's
//! bottom bar (its keys held at the bottom of the page's view), a picker opened by a small key inside a joined
//! input, and the one-line drop zone with its key inside.

use crate::icons::{self, Glyph};
use crate::palette::{c, C};
use crate::tokens;
use egui::{pos2, vec2, Rect};

/// The bottom bar's keys: 14 above them and 16 below.
const BAR_ABOVE: f32 = 14.0;
const BAR_BELOW: f32 = 16.0;

/// Where a page's bottom bar last stood: the view it was held at the foot of, its height, and the pass it was drawn
/// in ([`bar_over`]).
#[derive(Clone, Copy, Debug)]
struct Floor {
    view: Rect,
    h: f32,
    pass: u64,
}

const FLOOR: &str = "zikaron-page-bar-floor";

/// How much of the foot of `view` a page's bottom bar covers: its height when one was drawn at the foot of this view
/// this pass or the one before, else zero. What opens into view ([`crate::fold::reveal`]) ends above it.
pub fn bar_over(ctx: &egui::Context, view: Rect) -> f32 {
    let now = ctx.cumulative_pass_nr();
    match ctx.data(|d| d.get_temp::<Floor>(egui::Id::new(FLOOR))) {
        Some(f) if f.pass + 1 >= now && (f.view.bottom() - view.bottom()).abs() < 0.5 && f.view.intersects(view) => f.h,
        _ => 0.0,
    }
}

/// A page's bottom bar: its keys at the right edge of the page's column (laid right to left, 10 apart), held at
/// the bottom of the page's view whether the page is short or scrolls, on a band of the page ground that fades in
/// over its first 14. Drawn last on the page: the page keeps room at its end so its last block clears the bar,
/// and the band takes the pointer from what scrolls under it. Returns what `add` answers.
pub fn page_bar<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let h = BAR_ABOVE + tokens::KEY_H + BAR_BELOW;
    let view = ui.clip_rect();
    let col = ui.max_rect();
    // Room at the page's end: the bar stands over the page's own bottom margin and this.
    ui.allocate_space(vec2(col.width(), (h - tokens::PAGE_PAD).max(0.0)));
    let band = Rect::from_min_max(pos2(view.left(), view.bottom() - h), view.max);
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(FLOOR), Floor { view, h, pass }));
    let bar = ui.interact(band, ui.id().with("zikaron-page-bar"), egui::Sense::click());
    crate::hand::sink(ui.ctx(), bar.id);
    let ground = c(C::Ground);
    let clear = ground.gamma_multiply(0.0);
    let fade = Rect::from_min_max(band.min, pos2(band.right(), band.top() + BAR_ABOVE));
    let mut mesh = egui::Mesh::default();
    let i = mesh.vertices.len() as u32;
    mesh.colored_vertex(fade.left_top(), clear);
    mesh.colored_vertex(fade.right_top(), clear);
    mesh.colored_vertex(fade.right_bottom(), ground);
    mesh.colored_vertex(fade.left_bottom(), ground);
    mesh.add_triangle(i, i + 1, i + 2);
    mesh.add_triangle(i, i + 2, i + 3);
    let p = ui.painter().with_clip_rect(view);
    p.add(egui::Shape::mesh(mesh));
    p.rect_filled(Rect::from_min_max(pos2(band.left(), fade.bottom()), band.max), egui::CornerRadius::ZERO, ground);
    let keys = Rect::from_min_max(pos2(col.left(), view.bottom() - BAR_BELOW - tokens::KEY_H), pos2(col.right(), view.bottom() - BAR_BELOW));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(keys).layout(egui::Layout::right_to_left(egui::Align::Center)));
    child.set_clip_rect(view);
    child.spacing_mut().item_spacing.x = 10.0;
    add(&mut child)
}

/// A small key inside a joined input that opens a picker under it (its right edge on the key's): 30 high, corner
/// 8; with `face` it has a key's face and edge ("change"), else none until the pointer is on it; a caret after its
/// words with `caret`. A key that cannot open now is quiet and takes no clicks. Returns the row picked.
#[allow(clippy::too_many_arguments)]
pub fn inner_pick(ui: &mut egui::Ui, id_salt: &str, label: &str, caret: bool, face: bool, enabled: bool, spec: &crate::pick::Spec, items: &[crate::menu::Item], keep: crate::pick::Keep) -> Option<usize> {
    let id = ui.id().with(("zikaron-pick", id_salt));
    let resp = inner_key(ui, label, caret, face, enabled);
    if resp.clicked() {
        crate::menu::toggle(ui.ctx(), id);
    }
    let picked = crate::pick::show_from(ui.ctx(), Some(ui.layer_id()), id, resp.rect, ui.clip_rect(), false, spec, items, keep);
    crate::menu::stay(ui, id, resp.rect);
    picked
}

/// A small key with a key's face inside a joined input that opens a sheet of the caller's (a picker too long for a
/// floating card), enabled or not.
pub fn inner_open(ui: &mut egui::Ui, label: &str, enabled: bool) -> egui::Response {
    inner_key(ui, label, false, true, enabled)
}

/// The small key inside a joined input, enabled or not (`fields::inner_key` and `fields::inner_face` take no
/// state).
fn inner_key(ui: &mut egui::Ui, label: &str, caret: bool, face: bool, enabled: bool) -> egui::Response {
    let font = egui::FontId::new(crate::tokens::Type::Small.size(), crate::fonts::medium());
    let ink = if enabled { c(C::Ink) } else { c(C::Ink3) };
    let g = ui.painter().layout_no_wrap(label.to_string(), font, ink);
    let w = 12.0 + g.size().x + if caret { 6.0 + 13.0 } else { 0.0 } + 12.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    let p = ui.painter();
    if face {
        p.rect(rect, egui::CornerRadius::same(8), c(C::KeyFace), egui::Stroke::new(0.5_f32, c(C::KeyEdge)), egui::StrokeKind::Outside);
    } else if enabled && resp.hovered() {
        p.rect_filled(rect, egui::CornerRadius::same(8), c(C::Tone));
    }
    p.galley(pos2(rect.left() + 12.0, rect.center().y - g.size().y / 2.0), g, ink);
    if caret {
        icons::glyph_at(p, Glyph::Down, pos2(rect.right() - 12.0 - 6.5, rect.center().y), 13.0, if enabled { c(C::Ink2) } else { c(C::Ink3) });
    }
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// The one-line drop zone's height.
const LINE_H: f32 = 56.0;

/// A one-line drop zone (56 high, corner 12, dashed): the inbox glyph and `words` at its left, a key at its right
/// (6 in). A file dropped on it, a click on it or on its key: the caller takes the file or opens the system
/// picker. Returns what the zone received and whether its key was pressed.
pub fn drop_line(ui: &mut egui::Ui, id_salt: &str, words: &str, key: &str, accept: bool) -> (crate::drop::Drop, bool) {
    let at = ui.cursor().min;
    let w = ui.available_width();
    let d = crate::drop::zone(ui, id_salt, None, &[""], None, LINE_H, crate::drop::Shape::Line, accept);
    let rect = Rect::from_min_size(at, vec2(w, LINE_H.max(crate::grid::cell_height(ui))));
    let p = ui.painter();
    icons::glyph_at(p, Glyph::Inbox, pos2(rect.left() + 16.0 + 9.0, rect.center().y), 18.0, c(C::Ink2));
    let k = crate::button::Key::new(key, crate::button::Role::Secondary);
    let kw = crate::button::size_of(ui, &k).x;
    let kr = Rect::from_min_size(pos2(rect.right() - 6.0 - kw, rect.center().y - tokens::KEY_H / 2.0), vec2(kw, tokens::KEY_H));
    let tx = rect.left() + 16.0 + 18.0 + 12.0;
    crate::paint::at(p, ui, pos2(tx, rect.center().y), egui::Align2::LEFT_CENTER, words, crate::tokens::Type::Note, c(C::Ink2), (kr.left() - 12.0 - tx).max(0.0));
    let resp = ui.interact(kr, ui.id().with(("zikaron-drop-line-key", id_salt)), egui::Sense::click());
    crate::button::paint(ui, kr, &resp, &k, crate::button::Phase::Idle);
    let hit = resp.clicked();
    let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    (d, hit)
}
