//! Sheets: a white card centered over a dimming scrim, 460, 520 or 640 wide. It condenses out of the place it
//! was opened from: from 0.92 about the press, to its own size in 0.44 s, its card and scrim coming in over
//! 200 ms and its content after them (from 0.07 s, over 0.24 s). It leaves quickly: back to 0.97, fading, in
//! 0.14 s. Under reduced motion it only fades. A click outside does nothing; Esc is cancel (unless a menu,
//! list or calendar opened over it is still up: that one takes the Esc first).
//!
//! Its height follows its content in place, on the curve every height in the library follows (0.3 s): a step,
//! a tab, a row shown or a file chosen inside it changes its height smoothly, never by reopening or jumping. A
//! sheet with steps slides from one step to the next inside the same card; after any step or tab change the
//! new content fades in once the height is on its way. The content scrolls inside the card; the keys at the
//! bottom stay put and the content passes under them. Groups inside a sheet lay no ground of their own: rows
//! with a fine line between them ([`inside`]). Sheets lie above the first-run wizard and the passcode gate.

use crate::motion::{self, Curve};
use crate::paint;
use crate::palette::{c, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Color32, Rect};

/// How a step change moves.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slide {
    /// A tab or content change: only the height eases.
    None,
    Forward,
    Back,
}

/// One sheet's description.
#[derive(Clone, Copy, Debug)]
pub struct Spec<'a> {
    pub id: &'a str,
    pub width: f32,
    /// Changes when the sheet moves to another step.
    pub step: u64,
    pub slide: Slide,
    /// A refused press shakes the card (frame time it started).
    pub shake_at: Option<f64>,
}

impl<'a> Spec<'a> {
    pub fn new(id: &'a str, width: f32) -> Spec<'a> {
        Spec { id, width, step: 0, slide: Slide::None, shake_at: None }
    }

    pub fn step(mut self, step: u64, slide: Slide) -> Self {
        self.step = step;
        self.slide = slide;
        self
    }

    pub fn shake(mut self, at: Option<f64>) -> Self {
        self.shake_at = at;
        self
    }
}

/// What a sheet answered this frame.
pub struct Out<B, F> {
    pub body: B,
    pub foot: F,
    /// Esc was pressed: the caller cancels.
    pub esc: bool,
}

#[derive(Clone, Copy)]
struct StepState {
    step: u64,
    slide: Slide,
    t0: f64,
}

/// How the sheet condenses: the scale it grows from, over how long; its content's delay and fade; and how it
/// leaves.
const GROW_FROM: f32 = 0.92;
const GROW: f32 = 0.44 * crate::tokens::PACE;
const CONTENT_DELAY: f32 = 0.07 * crate::tokens::PACE;
const CONTENT_IN: f32 = 0.24 * crate::tokens::PACE;
const LEAVE_TO: f32 = 0.97;
const LEAVE: f32 = 0.14 * crate::tokens::PACE;

pub(crate) fn inside_id() -> egui::Id {
    egui::Id::new("zikaron-sheet-inside")
}

/// The title the sheet being drawn now has said (its body says it first, through [`title`] or
/// `sheetparts::title_seg`).
fn title_id() -> egui::Id {
    egui::Id::new("zikaron-sheet-title")
}

/// Whether `label` says again what the sheet being drawn now said as its title: a field or row named so draws no
/// name, the title being its name already. The one judgement every name in a sheet asks (`fields::Stack::field`,
/// `card::Form`'s rows).
pub fn echoes_title(ctx: &egui::Context, label: &str) -> bool {
    let label = label.trim();
    !label.is_empty() && inside(ctx) && ctx.data(|d| d.get_temp::<String>(title_id())).map(|t| t.trim() == label).unwrap_or(false)
}

/// Note the title the sheet being drawn now says.
pub(crate) fn said_title(ctx: &egui::Context, s: &str) {
    ctx.data_mut(|d| d.insert_temp(title_id(), s.to_string()));
}

/// The corner of what the rows being drawn now lie in, when that is not a card of their own (a sheet, a form
/// card): a lit row's block takes it less its gap (`crate::band`).
pub(crate) fn rows_corner_id() -> egui::Id {
    egui::Id::new("zikaron-rows-corner")
}

/// Whether what is being drawn now lies in a sheet's body (groups there lay no ground of their own).
pub fn inside(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(inside_id())).unwrap_or(false)
}

/// Whether any sheet was drawn this pass or the last (drop zones and page keys under a sheet stay still).
pub fn up(ctx: &egui::Context) -> bool {
    let pass = crate::motion::pass(ctx);
    ctx.data(|d| d.get_temp::<u64>(egui::Id::new("zikaron-sheet-up"))).map(|at| at + 1 >= pass).unwrap_or(false)
}

/// Draw a sheet. `body` draws the scrolling content (title included); `foot` draws the keys at the bottom
/// right (laid right to left, 8 apart). Both get the caller's state.
pub fn show<T, B, F>(ctx: &egui::Context, spec: Spec, state: &mut T, body: impl FnOnce(&mut egui::Ui, &mut T) -> B, foot: impl FnOnce(&mut egui::Ui, &mut T) -> F) -> Out<B, F> {
    let id = egui::Id::new(("zikaron-sheet", spec.id));
    let now = crate::motion::now(ctx);
    // The pass number is read before taking the data lock (asking the context inside it would deadlock).
    let pass = crate::motion::pass(ctx);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("zikaron-sheet-up"), pass));
    let screen = ctx.content_rect();
    let grow = crate::layer::entrance(ctx, &[id.with("card-keep")], id.with("age"), GROW);
    let age = motion::age(ctx, id.with("age"), 0);
    let enter = Curve::Ease.at((age / tokens::MID).clamp(0.0, 1.0)).max(grow);
    let content = Curve::Ease.at(((age - CONTENT_DELAY) / CONTENT_IN).clamp(0.0, 1.0)).max(if grow >= 1.0 { 1.0 } else { 0.0 });
    // Where it condenses from: the press that opened it (the pointer when it first shows), else its centre.
    let origin_id = id.with("origin");
    if age <= 0.0 {
        let at = ctx.input(|i| i.pointer.latest_pos()).filter(|p| screen.contains(*p)).unwrap_or(screen.center());
        ctx.data_mut(|d| d.insert_temp(origin_id, at));
    }
    let origin = ctx.data(|d| d.get_temp::<egui::Pos2>(origin_id)).unwrap_or(screen.center());
    let layer = egui::LayerId::new(egui::Order::Foreground, id.with("card"));
    crate::layer::place(ctx, layer, None);

    let max_h = (screen.height() - 64.0).max(160.0);
    let w = spec.width.min(screen.width() - 32.0);
    let foot_id = id.with("foot-h");
    let body_id = id.with("body-h");
    let foot_h = ctx.data(|d| d.get_temp::<f32>(foot_id)).unwrap_or(66.0);
    let body_h = ctx.data(|d| d.get_temp::<f32>(body_id)).unwrap_or(0.0);
    // The blank a fold row ending the body keeps under what it shows ([`trailing`]): the keys come up by it, so the
    // fold's words (or, open, its last row's) stand off the keys as the body's last block does.
    let trim_id = id.with("trim");
    let trim = ctx.data(|d| d.get_temp::<f32>(trim_id)).unwrap_or(0.0);
    let foot_now = (foot_h - trim).max(0.0);
    let natural = (body_h + foot_now).min(max_h).max(foot_h + 40.0);
    // The body shown whole when the sheet is not capped: no scroll bar then (`scroll::holds`, the library's one
    // judgment of "more than it shows").
    let fits = crate::scroll::holds(body_h + foot_now, max_h);
    let st_id = id.with("step");
    // A sheet opened anew starts on its step with no slide (the last opening's step is forgotten).
    let fresh = age <= 0.0;
    let st = ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_insert_with(st_id, || StepState { step: spec.step, slide: spec.slide, t0: f64::NEG_INFINITY });
        if fresh {
            *s = StepState { step: spec.step, slide: spec.slide, t0: f64::NEG_INFINITY };
        }
        if s.step != spec.step {
            s.step = spec.step;
            s.slide = spec.slide;
            s.t0 = now;
        }
        *s
    });
    let since = if now >= st.t0 { (now - st.t0) as f32 } else { f32::INFINITY };
    // A step or tab change: the new content slides and fades in on the height's own clock, from the same frame, so
    // content and card arrive together.
    let step_p = (since / tokens::SLOW).clamp(0.0, 1.0);
    let step_in = Curve::Ease.at(step_p);
    if step_p < 1.0 || step_in < 1.0 {
        ctx.request_repaint();
    }
    // Height: follows the content in place, always (the library's one height curve); when the content was itself
    // changing height last pass (a card or fold in it easing), the card takes it as it is and moves with it.
    let inner_id = id.with("inner-moving");
    let inner_was = ctx.data(|d| d.get_temp::<bool>(inner_id)).unwrap_or(false);
    let card_h = crate::flow::height_following(ctx, id.with("card-h"), natural, w, inner_was).round();
    let shake = motion::shake_at(ctx, spec.shake_at, 0.36 * crate::tokens::PACE);
    let card = Rect::from_center_size(screen.center(), vec2(w, card_h)).translate(vec2(shake, 0.0));
    let scale = 1.0 - (1.0 - GROW_FROM) * (1.0 - grow) * motion::travel(ctx);
    ctx.set_transform_layer(layer, egui::emath::TSTransform { scaling: scale, translation: origin.to_vec2() * (1.0 - scale) });
    let esc = crate::layer::esc(ctx, id);
    let out = crate::layer::over(ctx, layer)
        .show(ctx, |ui| {
            // The scrim: the ground the card lies on in its own layer, taking every press and drag so nothing
            // under it reacts; then the card's body.
            let screen = crate::layer::ground(ui);
            ui.painter().rect_filled(screen, 0.0, c(C::Scrim).gamma_multiply(enter));
            crate::layer::under(ui, crate::layer::Under::Taken);
            crate::layer::body(ui, card);
            ui.multiply_opacity(enter);
            ui.set_clip_rect(card.expand(64.0));
            crate::material::paint(ui.painter(), card, Radius::Sheet, crate::material::Level::Sheet, 0.0);
            // The content comes after the card.
            ui.multiply_opacity(if enter > 0.0 { (content / enter).min(1.0) } else { 0.0 });
            let inner_clip = card;
            // The body: scrolls inside what the foot leaves.
            let body_room = (card_h - foot_now).max(0.0);
            let body_rect = Rect::from_min_size(card.min, vec2(w, body_room));
            let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(body_rect).layout(egui::Layout::top_down(egui::Align::Min)));
            bui.set_clip_rect(body_rect.intersect(inner_clip));
            let slide_x = match st.slide {
                Slide::Forward if step_p < 1.0 => 24.0 * (1.0 - Curve::Ease.at(step_p)),
                Slide::Back if step_p < 1.0 => -24.0 * (1.0 - Curve::Ease.at(step_p)),
                _ => 0.0,
            };
            let slide_a = if matches!(st.slide, Slide::Forward | Slide::Back) && step_p < 1.0 { Curve::Ease.at(step_p).min(step_in.max(Curve::Ease.at(step_p))) } else { step_in };
            let slide_x = slide_x * motion::travel(ctx);
            ui.ctx().data_mut(|d| d.remove::<(f32, f32)>(trailing()));
            let bar = if fits { egui::scroll_area::ScrollBarVisibility::AlwaysHidden } else { egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded };
            let (b, content_h, content_bottom) = crate::scroll::nested(
                &mut bui,
                egui::ScrollArea::vertical()
                    .id_salt(("zikaron-sheet-scroll", spec.id))
                    .scroll_source(egui::scroll_area::ScrollSource { drag: false, ..egui::scroll_area::ScrollSource::ALL })
                    .scroll_bar_visibility(bar)
                    .max_height(body_room)
                    .auto_shrink([false, true]),
                |ui| {
                    let r = egui::Frame::new().inner_margin(egui::Margin { left: 24, right: 24, top: 22, bottom: BODY_FOOT as i8 }).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = tokens::S3;
                        let was = inside(ui.ctx());
                        ui.ctx().data_mut(|d| d.insert_temp(inside_id(), true));
                        // This sheet's title is said by its body; one said by a sheet under it is not this one's.
                        let title_was = ui.ctx().data(|d| d.get_temp::<String>(title_id()));
                        ui.ctx().data_mut(|d| d.remove::<String>(title_id()));
                        let corner_was = ui.ctx().data(|d| d.get_temp::<f32>(rows_corner_id()));
                        ui.ctx().data_mut(|d| d.insert_temp(rows_corner_id(), Radius::Sheet.px()));
                        let (b, inner) = crate::flow::watch_moving(&ui.ctx().clone(), || motion::shifted(ui, vec2(slide_x, 0.0), slide_a, motion::key_of(&st.step), |ui| body(ui, state)));
                        ui.ctx().data_mut(|d| d.insert_temp(inner_id, inner));
                        ui.ctx().data_mut(|d| d.insert_temp(inside_id(), was));
                        ui.ctx().data_mut(|d| match title_was {
                            Some(t) => {
                                d.insert_temp(title_id(), t);
                            }
                            None => {
                                d.remove::<String>(title_id());
                            }
                        });
                        ui.ctx().data_mut(|d| {
        if let Some(c) = corner_was {
            d.insert_temp(rows_corner_id(), c);
        } else {
            d.remove::<f32>(rows_corner_id());
        }
    });
                        b
                    });
                    (r.inner, r.response.rect.height(), r.response.rect.bottom())
                })
                .inner;
            // A fold row ending the body (its last rows ending where the body does) gives its blank to the keys.
            let ends = ui.ctx().data(|d| d.get_temp::<(f32, f32)>(trailing()));
            let trim_now = ends.filter(|(bottom, _)| (bottom + BODY_FOOT - content_bottom).abs() < crate::scroll::SLACK).map(|(_, t)| t.round()).unwrap_or(0.0);
            if (content_h - body_h).abs() > 0.5 || (trim_now - trim).abs() > 0.5 {
                ui.ctx().data_mut(|d| d.insert_temp(body_id, content_h));
                ui.ctx().data_mut(|d| d.insert_temp(trim_id, trim_now));
                // Laid out again at once with the new size, so no frame is shown placed by the old one.
                ui.ctx().request_discard("sheet body height changed");
            }
            // The foot: pinned under the body.
            let foot_rect = Rect::from_min_max(pos2(card.left(), card.bottom() - foot_now), card.max);
            foot_band(ui.painter(), foot_rect, Radius::Sheet.px());
            let mut fui = ui.new_child(egui::UiBuilder::new().max_rect(foot_rect).layout(egui::Layout::top_down(egui::Align::Min)));
            let fr = egui::Frame::new().inner_margin(egui::Margin { left: 24, right: 24, top: (FOOT_ABOVE - trim).max(0.0) as i8, bottom: 22 }).show(&mut fui, |ui| {
                ui.set_width(ui.available_width());
                ui.allocate_ui_with_layout(vec2(ui.available_width(), tokens::KEY_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = tokens::S2;
                    foot(ui, state)
                })
                .inner
            });
            // The foot's own height, as with the whole room above the keys (the trim is given back here).
            let fh = fr.response.rect.height() + trim;
            if (fh - foot_h).abs() > 0.5 {
                ui.ctx().data_mut(|d| d.insert_temp(foot_id, fh));
                // Laid out again at once with the new size, so no frame is shown placed by the old one.
                ui.ctx().request_discard("sheet foot height changed");
            }
            ui.allocate_rect(card, egui::Sense::hover());
            Out { body: b, foot: fr.inner, esc }
        })
        .inner;
    crate::layer::keep(ctx, id.with("card-keep"), layer, LEAVE, LEAVE_TO, card.center(), egui::Vec2::ZERO);
    out
}

/// Room between the body's last block and the keys (the body keeps [`BODY_FOOT`] at its own foot).
const FOOT_ABOVE: f32 = 34.0;

/// What the body keeps under its last block.
const BODY_FOOT: f32 = 4.0;

/// Where a group of rows drawn in a sheet or under a column of fields notes, as it ends, its bottom and the blank
/// its last row keeps under what it shows when that row is a fold (`card::form`); the sheet reads it after its body
/// ([`show`]), a form's keys before they are laid (`fields::Stack::foot`).
pub(crate) fn trailing() -> egui::Id {
    egui::Id::new("zikaron-sheet-trailing")
}

/// The foot's band: the sheet's glass again under the keys, fading in over its first 16, so what scrolls under
/// the keys passes behind a lighter band (its lower corners the sheet's).
fn foot_band(p: &egui::Painter, foot: Rect, radius: f32) {
    let glass = c(C::Glass);
    let fade = 16.0;
    let mut mesh = egui::Mesh::default();
    let top = foot.top();
    let mid = (top + fade).min(foot.bottom());
    mesh.colored_vertex(pos2(foot.left(), top), Color32::TRANSPARENT);
    mesh.colored_vertex(pos2(foot.right(), top), Color32::TRANSPARENT);
    mesh.colored_vertex(pos2(foot.right(), mid), glass);
    mesh.colored_vertex(pos2(foot.left(), mid), glass);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    p.add(egui::Shape::mesh(mesh));
    let r = radius.round().clamp(0.0, 255.0) as u8;
    let corners = egui::CornerRadius { nw: 0, ne: 0, sw: r, se: r };
    p.rect_filled(Rect::from_min_max(pos2(foot.left(), mid), foot.max), corners, glass);
}

/// A sheet's title (17/24 semibold) and an optional line under it.
pub fn title(ui: &mut egui::Ui, s: &str, sub: &str) {
    said_title(ui.ctx(), s);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(egui::RichText::new(s).font(Type::Sheet.font()).color(c(C::Ink)).line_height(Some(Type::Sheet.line())));
        if !sub.is_empty() {
            paint::text(ui, sub, Type::Note, c(C::Ink2));
        }
    });
    // 6 under the title before the body's first block (the body lays its blocks 12 apart).
    ui.add_space(6.0 - tokens::S3);
}

/// A line saying work is under way: a small turning ring, then the words on one line (elided to fit), at the
/// left or centred.
pub fn busy_note(ui: &mut egui::Ui, s: &str, centred: bool) {
    let t = Type::Small;
    let colour = c(C::Ink2);
    let ring = 4.25;
    let gap = 6.0;
    let room = ui.available_width();
    let fit = crate::width::elide_to(ui, s, t.font(), (room - ring * 2.0 - gap).max(0.0));
    let g = ui.painter().layout_no_wrap(fit, t.font(), colour);
    let w = ring * 2.0 + gap + g.size().x;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(if centred { room } else { w }, t.line().max(ring * 2.0)), egui::Sense::hover());
    let x = if centred { rect.center().x - w / 2.0 } else { rect.left() };
    let y = rect.center().y;
    crate::mark::spinner(ui.ctx(), ui.painter(), egui::pos2(x + ring, y), ring, 1.5, colour, crate::tokens::CYCLE);
    ui.painter().galley(egui::pos2(x + ring * 2.0 + gap, y - g.size().y / 2.0), g, colour);
}

/// The quiet words at the left end of a sheet's foot (drawn last in the right-to-left row, so they take what
/// the keys leave).
pub fn foot_note(ui: &mut egui::Ui, s: &str) {
    if s.is_empty() {
        return;
    }
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        let room = ui.available_width();
        paint::line(ui, s, Type::Small, c(C::Ink2), room);
    });
}
