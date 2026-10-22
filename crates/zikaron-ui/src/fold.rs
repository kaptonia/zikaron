//! The fold: a quiet title with a caret that turns a quarter when open; the body grows to its height and fades
//! in over 0.3 s (the curve every height in the library follows), and shuts the same way. Opened with too little
//! room below, it takes the page (or the sheet) along as it grows, so the body ends up seen whole, 32 above the
//! bottom, without the head going past the top. Raw values (addresses, digests, paths, error codes) live only
//! inside folds.

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
        if open {
            let now = crate::motion::now(ui.ctx());
            ui.data_mut(|d| d.insert_temp(id.with("opened-at"), now));
        }
    }
    ui.data_mut(|d| d.insert_temp(id, open));
    let t = motion::flag(ui.ctx(), id.with("t"), open, tokens::SLOW);
    let hot = motion::states(ui.ctx(), id.with("head"), resp.hovered(), resp.is_pointer_button_down_on(), resp.has_focus(), true).hover;
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
    // Under reduced motion the body takes or gives up its height at once (it still fades with `t`).
    let grow = if motion::reduced(ui.ctx()) { if open { 1.0 } else { 0.0 } } else { t };
    let shown_h = if grow >= 1.0 { full } else { full * grow };
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
    let take = if grow >= 1.0 { measured } else { measured.min(full.max(measured) * grow) };
    if grow < 1.0 {
        // On its way: whatever holds the fold moves with it ([`crate::flow::mark_moving`]).
        crate::flow::mark_moving(ui.ctx());
    }
    ui.advance_cursor_after_rect(Rect::from_min_size(top, vec2(w, take.max(0.0))));
    // Opened by a press just now: the page follows the body as it grows (its foot 32 above the bottom; the
    // scroll never takes the head past the top).
    let now = crate::motion::now(ui.ctx());
    let opened = ui.data(|d| d.get_temp::<f64>(id.with("opened-at")));
    if open && opened.map(|at| now >= at && ((now - at) as f32) < tokens::SLOW + 0.05).unwrap_or(false) {
        reveal(ui, Rect::from_min_max(head.min, pos2(head.right(), top.y + take + REVEAL_FOOT)));
    }
    Some(r)
}

/// How far above the bottom a body opened into view ends: the one measure for a fold and a floating card.
pub use crate::layer::REVEAL_FOOT;

/// Bring `want` into view in the area `ui` scrolls in, at once (the caller runs it each frame of its own
/// growth): no scroll when it is in view; else the least scroll that shows it, never taking its top past the
/// top of the view. A page's bottom bar held over the foot of the view ([`crate::formparts::bar_over`]) is not
/// view: what opens ends above it.
pub fn reveal(ui: &egui::Ui, want: Rect) {
    let clip = ui.clip_rect();
    let over = crate::formparts::bar_over(ui.ctx(), clip);
    let view = Rect::from_min_max(clip.min, egui::pos2(clip.right(), clip.bottom() - over));
    if want.bottom() > view.bottom() + 0.5 && want.top() > view.top() {
        // Scrolled to a place, not by an amount: asked on every frame of the opening (and twice in a frame laid out
        // again), it lands where it would have once. egui aligns to its view less its clip margin and keeps the
        // scrolled content's spacing from the edge; both are the style's, and are given back here.
        let style = ui.ctx().global_style();
        let (margin, gap) = (style.visuals.clip_rect_margin, style.spacing.item_spacing.y);
        let (want, align) = if want.height() <= view.height() {
            (Rect::from_min_max(want.min, egui::pos2(want.right(), want.bottom() + margin - gap + over)), egui::Align::BOTTOM)
        } else {
            (Rect::from_min_max(egui::pos2(want.left(), want.top() - margin + gap), want.max), egui::Align::TOP)
        };
        ui.scroll_to_rect_animation(want, Some(align), egui::style::ScrollAnimation::none());
        crate::scroll::asked(ui.ctx());
    }
}
