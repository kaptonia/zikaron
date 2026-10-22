//! Overlay layers: sheets, menus, pick lists, the date card and full-window covers. One place for how they
//! come, sit, take keys and go.
//!
//! - **Areas.** Every overlay area this library opens is made by [`area`]: egui's own fade-in is off, so the
//!   only entrance is the library's own (no second fade laid over it).
//! - **Leaving.** A layer that may leave is recorded each frame it is drawn ([`keep`]): its shapes are kept. The
//!   first frame it is not drawn, its kept shapes are drawn again as its entrance played backwards: over the
//!   same time, from full opacity, size and place back to the scale and the offset it entered from, on the same
//!   curve run in reverse (so it leaves as gently as it came, and back the way it came). Pages are not
//!   recorded: a page only enters.
//! - **Coming back while leaving.** An overlay drawn again while its leaving still plays takes its entrance up
//!   where the leaving had reached ([`entrance`]): no jump back to nothing.
//! - **Placing.** Every overlay is one layer, placed by [`place`]: raised to the top the frame it appears (so the
//!   overlay opened last lies on top), never again after; a card opened from inside another overlay hangs
//!   directly above that overlay whatever is raised later. Inside its layer, before anything else, comes what
//!   lies under it ([`under`], [`Under`]): a floating card (a menu, a pick list, the date card) on a clear guard
//!   that takes a press outside it: that press closes the card and never reaches what is under it; a sheet or a
//!   full-window cover on a painted ground that takes every press and drag; a toast on nothing, so presses
//!   beside it reach the page. Then the card's body ([`body`]): a press on the card off its rows does nothing.
//!   Guard, body and rows share one layer, so no press can fall between them however layers are raised.
//!   A floating card hangs under its anchor, or above it when the window has no room below, between the side
//!   edges of the room its anchor is drawn in (the anchor's own clip, never past it onto what lies beside:
//!   [`drop_card`]).
//! - **Tips.** Hover tips are egui's own, drawn above everything and gone once the pointer leaves the widget
//!   (so a tip never stands over a press); every one is attached here ([`tip`]).
//! - **Esc.** One Esc closes one overlay: the one opened last of those on screen ([`esc`]).
//! - **On top.** Whether what a widget draws at a place lies on top there ([`on_top`]): no overlay lies over it.
//!   A widget that takes keys without a press (a passcode row taking focus nobody holds) asks it first.

use crate::motion::{self, Curve};
use egui::{Context, Id, LayerId, Pos2, Rect, Vec2};

#[derive(Clone)]
struct Snap {
    layer: LayerId,
    shapes: Vec<egui::epaint::ClippedShape>,
    pass: u64,
    left_at: Option<f64>,
    dur: f32,
    /// Scale reached at the end of leaving, about `center`.
    to_scale: f32,
    center: egui::Pos2,
    /// Offset reached at the end of leaving (where the entrance came from).
    moved: Vec2,
}

fn table_id() -> Id {
    Id::new("zikaron-leaving-layers")
}

/// The one way this library opens an overlay area: in `order`, with egui's own fade-in off.
pub fn area(id: Id, order: egui::Order) -> egui::Area {
    egui::Area::new(id).order(order).fade_in(false)
}

/// The window in `layer`'s own coordinates (under the transform it has this frame).
fn cover(ctx: &Context, layer: LayerId) -> Rect {
    let screen = ctx.content_rect();
    ctx.layer_transform_from_global(layer).map(|t| t * screen).unwrap_or(screen)
}

/// The area of an overlay with something under it ([`under`]): it spans the whole window, so the window's every
/// point lies on this layer for egui too (a wheel or a press anywhere is this overlay's, not the page's under
/// it). The card is drawn inside at its own place (its content in children with their own rects). Set the
/// layer's transform for the frame before calling it.
pub fn over(ctx: &Context, layer: LayerId) -> egui::Area {
    area(layer.id, layer.order).fixed_pos(cover(ctx, layer).min).constrain(false)
}

/// Record the shapes a layer drew this pass (call right after drawing it). `dur`, `to_scale` and `moved` are
/// its entrance's time and the scale and offset it entered from.
pub fn keep(ctx: &Context, key: Id, layer: LayerId, dur: f32, to_scale: f32, center: egui::Pos2, moved: Vec2) {
    // Under reduced motion a leaving only fades: no scale and no travel, for every overlay alike.
    let k = crate::motion::travel(ctx);
    let (to_scale, moved) = (1.0 + (to_scale - 1.0) * k, moved * k);
    let shapes: Vec<egui::epaint::ClippedShape> = ctx.graphics(|g| g.get(layer).map(|l| l.all_entries().cloned().collect()).unwrap_or_default());
    let pass = crate::motion::pass(ctx);
    ctx.data_mut(|d| {
        let t = d.get_temp_mut_or_default::<Vec<(Id, Snap)>>(table_id());
        t.retain(|(k, _)| *k != key);
        t.push((key, Snap { layer, shapes, pass, left_at: None, dur, to_scale, center, moved }));
    });
}

/// Forget the recorded layers `gone` names (by their `key`): they leave at once, with no copy played back over
/// what is under them (a cover that hands the whole window to the page it uncovers).
pub fn forget(ctx: &Context, gone: impl Fn(Id) -> bool) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<(Id, Snap)>>(table_id()).retain(|(k, _)| !gone(*k)));
}

/// Turn the leaving of a recorded layer (`key`) toward `moved`: the offset it reaches at the end of its leaving
/// (a wizard step's card leaves toward the side it is left on, known only when the next one comes).
pub fn steer(ctx: &Context, key: Id, moved: Vec2) {
    let moved = moved * crate::motion::travel(ctx);
    ctx.data_mut(|d| {
        if let Some(t) = d.get_temp_mut_or_default::<Vec<(Id, Snap)>>(table_id()).iter_mut().find(|(k, _)| *k == key) {
            t.1.moved = moved;
        }
    });
}

/// How far a recorded layer's leaving has played back: its opacity now (1 just left, 0 gone), or `None` when it
/// is not leaving.
fn leaving_alpha(s: &Snap, now: f64) -> Option<f32> {
    let at = s.left_at?;
    let p = ((now - at) as f32 / s.dur).clamp(0.0, 1.0);
    (p < 1.0).then(|| Curve::Ease.at(1.0 - p))
}

/// The eased entrance of an overlay drawn this frame (its age under `age_id`, over `dur`). When one of `keys`
/// (the layers it records with [`keep`]) is still leaving, the entrance takes up where the leaving stands now
/// and that leaving stops: an overlay opened again halfway out comes back from halfway, not from nothing.
pub fn entrance(ctx: &Context, keys: &[Id], age_id: Id, dur: f32) -> f32 {
    let now = crate::motion::now(ctx);
    let back = ctx.data_mut(|d| {
        let t = d.get_temp_mut_or_default::<Vec<(Id, Snap)>>(table_id());
        let mut at: Option<f32> = None;
        for (k, s) in t.iter_mut() {
            if keys.contains(k) {
                if let Some(a) = leaving_alpha(s, now) {
                    at = Some(at.map_or(a, |x| x.max(a)));
                }
            }
        }
        if at.is_some() {
            t.retain(|(k, _)| !keys.contains(k));
        }
        at
    });
    if let Some(a) = back {
        motion::set_age(ctx, age_id, 0, dur * Curve::Ease.time_of(a));
    }
    let age = motion::age(ctx, age_id, 0);
    if age < dur {
        ctx.request_repaint();
    }
    Curve::Ease.at((age / dur).clamp(0.0, 1.0))
}

/// One colour of a leaving layer at `alpha`: faded, except egui's placeholder (`Color32::PLACEHOLDER`, the mark a
/// text vertex carries until the painter puts the text's own colour in its place). Faded, the placeholder would no
/// longer be recognised and leaving text would show in the placeholder's colour; left as it is, the painter puts
/// the text's fallback colour there, and that colour fades with the rest.
pub fn leaving_colour(col: egui::Color32, alpha: f32) -> egui::Color32 {
    if col == egui::Color32::PLACEHOLDER {
        return col;
    }
    col.gamma_multiply(alpha)
}

/// Draw the layers that have left and are still fading. Call once per frame, after everything else.
pub fn fade_out(ctx: &Context) {
    let pass = crate::motion::pass(ctx);
    let now = crate::motion::now(ctx);
    let mut snaps: Vec<(Id, Snap)> = ctx.data(|d| d.get_temp::<Vec<(Id, Snap)>>(table_id())).unwrap_or_default();
    let mut keep_going = Vec::new();
    for (key, mut s) in snaps.drain(..) {
        if s.pass == pass {
            keep_going.push((key, s));
            continue;
        }
        s.left_at.get_or_insert(now);
        let Some(alpha) = leaving_alpha(&s, now) else {
            continue;
        };
        // The entrance backwards: at leaving time t it looks as it did at entering time dur - t.
        let scale = s.to_scale + (1.0 - s.to_scale) * alpha;
        let ghost = LayerId::new(s.layer.order, s.layer.id.with("leaving"));
        let painter = ctx.layer_painter(ghost);
        for cs in &s.shapes {
            let mut shape = cs.shape.clone();
            egui::epaint::shape_transform::adjust_colors(&mut shape, move |col| *col = leaving_colour(*col, alpha));
            painter.with_clip_rect(cs.clip_rect).add(shape);
        }
        let translation = s.center.to_vec2() * (1.0 - scale) + s.moved * (1.0 - alpha);
        ctx.set_transform_layer(ghost, egui::emath::TSTransform { scaling: scale, translation });
        ctx.request_repaint();
        keep_going.push((key, s));
    }
    ctx.data_mut(|d| d.insert_temp(table_id(), keep_going));
}

/// What lies between an overlay and the page, drawn first inside the overlay's own layer ([`under`]): the card
/// is drawn after it in the same layer, so it can never come to lie under it.
#[derive(Clone, Copy, Debug)]
pub enum Under {
    /// A clear guard over the whole window: a press on it (outside the card) is reported, closes the card and
    /// reaches nothing under it. Menus, pick lists, the date card.
    Guard,
    /// A painted ground over the whole window, taking every press and drag. A cover's ground.
    Ground(egui::Color32),
    /// The same ground, painted by the overlay itself over [`ground`]'s rectangle: only its presses and drags
    /// are taken here. A sheet's scrim.
    Taken,
    /// Nothing: the card takes the presses on itself and every other press reaches the page. Toasts.
    Open,
}

/// Place an overlay (`card`, its own layer) among the others; call it before drawing the card. Opened from
/// inside another overlay of the same order (`parent`), it hangs directly above that overlay (above that
/// overlay's other cards too when it is itself hung above one: egui nests one level), so raising that overlay
/// later never covers it. Otherwise it is raised to the top when it appears (the frame after one it was not
/// drawn in), so the overlay opened last lies on top; while it stays, egui's own order holds (a press on an
/// overlay raises it).
pub fn place(ctx: &Context, card: LayerId, parent: Option<LayerId>) {
    let root = parent.filter(|p| p.order == card.order && *p != card).map(|p| ctx.memory(|m| m.areas().parent_layer(p)).unwrap_or(p));
    match root {
        Some(r) => ctx.set_sublayer(r, card),
        None => {
            if !ctx.memory(|m| m.areas().visible_last_frame(&card)) {
                ctx.move_to_top(card);
            }
        }
    }
}

/// What lies under an overlay, drawn first inside its area (`ui`): over the whole window as it is shown (the
/// window mapped back through the layer's own transform, so a card scaled in its entrance still leaves no edge
/// uncovered). Returns whether it was pressed this frame (a press outside the card: only [`Under::Guard`]
/// reports one).
pub fn under(ui: &mut egui::Ui, under: Under) -> bool {
    let sense = match under {
        Under::Open => return false,
        Under::Guard => egui::Sense::click(),
        Under::Ground(_) | Under::Taken => egui::Sense::click_and_drag(),
    };
    let all = cover(ui.ctx(), ui.layer_id());
    ui.expand_to_include_rect(all);
    let clip = ui.clip_rect();
    if let Under::Ground(fill) = under {
        let painted = ground(ui);
        ui.painter().rect_filled(painted, 0.0, fill);
    }
    let under_resp = ui.interact(all, ui.id().with("zikaron-under"), sense);
    crate::hand::sink(ui.ctx(), under_resp.id);
    let pressed = under_resp.clicked();
    ui.set_clip_rect(clip);
    matches!(under, Under::Guard) && pressed
}

/// How far past each edge of the window a painted ground reaches, as a share of the window's longer side: the
/// layer's leaving plays its shapes back shrinking toward the card ([`keep`], to 0.96 at most about a centred
/// card), and the ground must still cover the window then. A share well under any visible shrink, so a ground
/// drawn short of the window is still seen short.
const GROUND_PAST: f32 = 0.03;

/// The rectangle a painted ground covers, in the overlay's own coordinates: the window and [`GROUND_PAST`]
/// beyond each edge. The clip is opened to it (the overlay narrows it again for what it draws next).
pub fn ground(ui: &mut egui::Ui) -> Rect {
    let all = cover(ui.ctx(), ui.layer_id());
    let painted = all.expand(GROUND_PAST * all.width().max(all.height()));
    ui.set_clip_rect(painted);
    painted
}

/// The card's own body, drawn right after what lies under it: a press on the card that is on none of its rows
/// lands here and does nothing (it neither closes the card nor reaches what is under it). Its rows, drawn after
/// it, take their own presses.
pub fn body(ui: &mut egui::Ui, card: Rect) {
    let r = ui.interact(card, ui.id().with("zikaron-card-body"), egui::Sense::click());
    crate::hand::sink(ui.ctx(), r.id);
}

/// Where a floating card of `size` hangs off `anchor`: 6 under it, aligned to its left edge when `left` (its
/// right edge otherwise) and kept 8 inside the side edges of `bounds`; above it instead when the window has no
/// room under it and has room above; with room on neither side, on the side with more, kept 8 inside the window.
/// `bounds` is the room the anchor is drawn in (its ui's clip, `ui.clip_rect()`: a page's content area, so the
/// card never lies over the rail beside it); up and down a card may reach past that room (out of a sheet's body,
/// say) to the window's edges. `origin` is the corner it grows from, `moved` the offset it enters from (down
/// from above when under the anchor, up from below when above it).
pub struct Drop {
    pub rect: Rect,
    pub origin: Pos2,
    pub moved: Vec2,
}

impl Drop {
    /// The card's entrance at eased progress `e`: from 97 % of its size at `origin` and from `moved` away, both
    /// scaled by [`crate::motion::travel`] (none under "reduce motion"; the fade is the caller's). The one
    /// entrance of every drop card (menus, pickers, the date card); their leave is [`keep`].
    pub fn enter(&self, ctx: &Context, layer: egui::LayerId, e: f32) {
        let k = crate::motion::travel(ctx);
        let scale = 1.0 - 0.03 * (1.0 - e) * k;
        ctx.set_transform_layer(layer, egui::emath::TSTransform { scaling: scale, translation: self.origin.to_vec2() * (1.0 - scale) + self.moved * (1.0 - e) * k });
    }
}

pub fn drop_card(ctx: &Context, anchor: Rect, bounds: Rect, size: Vec2, left: bool) -> Drop {
    let screen = ctx.content_rect();
    let (room_l, room_r) = (bounds.left().max(screen.left()), bounds.right().min(screen.right()));
    let (room_l, room_r) = if room_r > room_l { (room_l, room_r) } else { (screen.left(), screen.right()) };
    let x = if left { anchor.left() } else { anchor.right() - size.x };
    let x = x.clamp(room_l + 8.0, (room_r - size.x - 8.0).max(room_l + 8.0));
    let below = anchor.bottom() + 6.0;
    let above = anchor.top() - 6.0 - size.y;
    // Under the anchor when it fits there, or when what the anchor scrolls in can scroll it there (the card's
    // owner then takes the page along: [`follow`]); else above when it fits there; fitting neither side, on the
    // side with more room, held inside the window (over the anchor if it must): never past an edge.
    let room_below = screen.bottom() - 8.0 - below;
    let room_above = anchor.top() - 6.0 - (screen.top() + 8.0);
    let scroll = crate::scroll::room_below(ctx, anchor).min((anchor.top() - bounds.top().max(screen.top())).max(0.0));
    let carried = size.y > room_below && size.y + REVEAL_FOOT - 8.0 <= room_below + scroll;
    let up = if size.y <= room_below || carried { false } else if size.y <= room_above { true } else { room_above > room_below };
    let y = if up { above } else { below };
    let y = if carried { y } else { y.clamp(screen.top() + 8.0, (screen.bottom() - 8.0 - size.y).max(screen.top() + 8.0)) };
    let rect = Rect::from_min_size(egui::pos2(x, y), size);
    let origin = match (left, up) {
        (true, false) => rect.left_top(),
        (false, false) => rect.right_top(),
        (true, true) => rect.left_bottom(),
        (false, true) => rect.right_bottom(),
    };
    Drop { rect, origin, moved: egui::vec2(0.0, if up { 4.0 } else { -4.0 }) }
}

/// How far above the window's bottom a card that took its page along ends.
pub const REVEAL_FOOT: f32 = 32.0;

fn card_rect_id(id: Id) -> Id {
    id.with("zikaron-card-at")
}

/// Record where a floating card under `id` lies this frame (its owner reads it in [`follow`]), and when it opened
/// (a card not drawn the pass before opens now).
pub fn card_at(ctx: &Context, id: Id, rect: Rect) {
    let (now, pass) = (crate::motion::now(ctx), crate::motion::pass(ctx));
    ctx.data_mut(|d| {
        let opened = match d.get_temp::<(f64, u64)>(card_rect_id(id).with("seen")) {
            Some((at, last)) if last + 1 >= pass => at,
            _ => now,
        };
        d.insert_temp(card_rect_id(id).with("seen"), (opened, pass));
        d.insert_temp(card_rect_id(id), rect);
    });
}

/// A floating card stays with its anchor (call it from the anchor's own `ui`, after the card is drawn, while it
/// is open): the wheel turned anywhere off the card scrolls what the anchor scrolls in, and the card, placed from
/// its anchor each frame, goes along; a card that hangs past the window's foot as it opens takes the page along
/// until it ends [`REVEAL_FOOT`] above it (never taking the anchor past the top); and once the anchor has scrolled out of view
/// the card closes. Returns whether it should close.
pub fn follow(ui: &egui::Ui, id: Id, anchor: Rect) -> bool {
    let ctx = ui.ctx();
    let Some(card) = ctx.data(|d| d.get_temp::<Rect>(card_rect_id(id))) else { return false };
    let view = ui.clip_rect();
    if !view.intersects(anchor) {
        return true;
    }
    let over_card = ctx.pointer_hover_pos().map(|p| card.contains(p)).unwrap_or(false);
    let wheel = ctx.input(|i| i.smooth_scroll_delta);
    if !over_card && wheel != Vec2::ZERO {
        ui.scroll_with_delta_animation(wheel, egui::style::ScrollAnimation::none());
        ctx.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
    }
    // The page is taken along only as the card opens (as a fold's body is revealed while it grows): the wheel
    // after that is the person's, never turned back.
    let opened = ctx.data(|d| d.get_temp::<(f64, u64)>(card_rect_id(id).with("seen"))).map(|(at, _)| at).unwrap_or(0.0);
    let opening = ((crate::motion::now(ctx) - opened) as f32) < crate::tokens::SLOW + 0.05;
    let screen = ctx.content_rect();
    if opening && card.top() >= anchor.bottom() && card.bottom() + REVEAL_FOOT > screen.bottom() + 0.5 {
        crate::fold::reveal(ui, Rect::from_min_max(anchor.min, egui::pos2(anchor.right(), card.bottom() + REVEAL_FOOT)));
    }
    false
}

/// Whether `ui`'s layer is the one on top at `at`'s centre: the topmost layer there that takes presses (egui's
/// own reading of the last frame's layers, modal included) is `ui`'s, or there is none (the page under every
/// overlay). An overlay opened over it (a sheet, a cover, a floating card with its guard) lies over the whole
/// window, so it answers no.
pub fn on_top(ui: &egui::Ui, at: Rect) -> bool {
    ui.ctx().layer_id_at(at.center()).is_none_or(|l| l == ui.layer_id())
}

/// Attach a hover tip to `resp` (egui's own tip: drawn above everything, gone once the pointer leaves `resp`).
pub fn tip(resp: egui::Response, text: impl Into<egui::WidgetText>) -> egui::Response {
    resp.on_hover_text(text)
}

#[derive(Clone, Copy)]
struct Esc {
    key: Id,
    /// Its place in opening order among the overlays on screen.
    seq: u64,
    /// The pass it was first drawn in (this opening).
    first: u64,
    /// The pass it was last drawn in.
    pass: u64,
}

/// Whether Esc closes this overlay now: Esc was pressed this frame and of the overlays on screen last frame
/// this one was opened last. Every overlay that closes on Esc asks here, each frame it is drawn, under its own
/// key; one Esc so closes one overlay (the date card over a pick list, not the list too).
pub fn esc(ctx: &Context, key: Id) -> bool {
    let pass = crate::motion::pass(ctx);
    let pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let tid = Id::new("zikaron-esc-owners");
    ctx.data_mut(|d| {
        let t = d.get_temp_mut_or_default::<(u64, Vec<Esc>)>(tid);
        // Gone from the screen for a pass: not an overlay any more (opened again, it is opened last).
        t.1.retain(|e| e.pass + 1 >= pass);
        // The owner: opened last of those already on screen before this pass (one opened this very pass waits
        // for the next).
        let owner = t.1.iter().filter(|e| e.first < pass).max_by_key(|e| e.seq).map(|e| e.key);
        match t.1.iter_mut().find(|e| e.key == key) {
            Some(e) => e.pass = pass,
            None => {
                t.0 += 1;
                let seq = t.0;
                t.1.push(Esc { key, seq, first: pass, pass });
            }
        }
        pressed && owner == Some(key)
    })
}
