//! The fold: a quiet title with a caret that turns a quarter when open; the body grows to its height and fades
//! in over 200 ms. Raw values (addresses, digests, paths, error codes) live only inside folds.

use crate::icons::{self, Glyph};
use crate::motion;
use crate::palette::{self, c, C};
use crate::tokens::{self, Type};
use egui::{pos2, vec2, Rect};

/// Whether a fold is open now (for callers that read before drawing).
pub fn is_open(ui: &egui::Ui, id_salt: &str) -> bool {
    let id = ui.make_persistent_id(("zikaron-fold", id_salt));
    ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false)
}

/// Set a fold open or shut from outside (a page that opens its details on arrival).
pub fn set_open(ui: &egui::Ui, id_salt: &str, open: bool) {
    let id = ui.make_persistent_id(("zikaron-fold", id_salt));
    ui.data_mut(|d| d.insert_temp(id, open));
}

/// A fold, shut at first. Returns the body's answer while the body is drawn.
pub fn fold<R>(ui: &mut egui::Ui, id_salt: &str, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
    fold_ex(ui, id_salt, title, false, add)
}

/// A fold with its first state given.
pub fn fold_ex<R>(ui: &mut egui::Ui, id_salt: &str, title: &str, first_open: bool, add: impl FnOnce(&mut egui::Ui) -> R) -> Option<R> {
    let id = ui.make_persistent_id(("zikaron-fold", id_salt));
    let mut open = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(first_open);
    // The head (26 tall): caret, then the title in the second ink, darker on hover.
    let g = crate::paint::galley(ui, title, Type::Small, c(C::Ink2));
    let head_w = 12.0 + 6.0 + g.size().x;
    let (head, resp) = ui.allocate_exact_size(vec2(head_w, 26.0), egui::Sense::click());
    if resp.clicked() {
        open = !open;
    }
    ui.data_mut(|d| d.insert_temp(id, open));
    let t = motion::flag(ui.ctx(), id.with("t"), open, tokens::MID);
    let hot = motion::flag(ui.ctx(), id.with("hot"), resp.hovered(), tokens::FAST);
    let p = ui.painter();
    let turn = t * std::f32::consts::FRAC_PI_2;
    icons::draw_glyph_turned(p, Glyph::Chev, Rect::from_center_size(pos2(head.left() + 6.0, head.center().y), vec2(12.0, 12.0)), c(C::Ink3), turn);
    let colour = palette::mix(c(C::Ink2), c(C::Ink), hot);
    p.galley(pos2(head.left() + 18.0, head.center().y - g.size().y / 2.0), g, colour);
    let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    if t <= 0.0 {
        return None;
    }
    // The body, drawn at full height into a clipped child whose visible height grows with `t`.
    let h_id = id.with("h");
    let full = ui.data(|d| d.get_temp::<f32>(h_id)).unwrap_or(0.0);
    let shown_h = if t >= 1.0 { full } else { full * t };
    let top = ui.cursor().min;
    let w = ui.available_width();
    let room = Rect::from_min_size(top, vec2(w, f32::INFINITY));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(room).layout(egui::Layout::top_down(egui::Align::Min)));
    let mut clip = child.clip_rect();
    clip.max.y = clip.max.y.min(top.y + shown_h + 1.0);
    child.set_clip_rect(clip);
    child.multiply_opacity(t);
    child.add_space(tokens::S3);
    let r = add(&mut child);
    let measured = child.min_rect().height();
    if (measured - full).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(h_id, measured));
        // Laid out again at once with the new size, so no frame is shown placed by the old one.
        ui.ctx().request_discard("fold height changed");
    }
    let take = if t >= 1.0 { measured } else { measured.min(full.max(measured) * t) };
    ui.advance_cursor_after_rect(Rect::from_min_size(top, vec2(w, take.max(0.0))));
    Some(r)
}
