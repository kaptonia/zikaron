//! Sheets: a white card centered over a dimming scrim, 440, 520 or 640 wide. It grows from 0.96 and fades in
//! over 200 ms, and leaves the same way backwards (200 ms, back to 0.96). A click outside does nothing; Esc is cancel. A sheet with steps
//! slides from one step to the next inside the same card, the card's height easing along. The content
//! scrolls inside the card; the keys at the bottom stay put. Sheets lie above the first-run wizard and the
//! passcode gate.

use crate::motion::{self, Curve};
use crate::paint;
use crate::palette::{c, Lift, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

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
    from_h: f32,
    shown_h: f32,
}

/// Whether any sheet was drawn this pass or the last (drop zones and page keys under a sheet stay still).
pub fn up(ctx: &egui::Context) -> bool {
    let pass = ctx.cumulative_pass_nr();
    ctx.data(|d| d.get_temp::<u64>(egui::Id::new("zikaron-sheet-up"))).map(|at| at + 1 >= pass).unwrap_or(false)
}

/// Draw a sheet. `body` draws the scrolling content (title included); `foot` draws the keys at the bottom
/// right (laid right to left, 8 apart). Both get the caller's state.
pub fn show<T, B, F>(ctx: &egui::Context, spec: Spec, state: &mut T, body: impl FnOnce(&mut egui::Ui, &mut T) -> B, foot: impl FnOnce(&mut egui::Ui, &mut T) -> F) -> Out<B, F> {
    let id = egui::Id::new(("zikaron-sheet", spec.id));
    let now = ctx.input(|i| i.time);
    // The pass number is read before taking the data lock (asking the context inside it would deadlock).
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("zikaron-sheet-up"), pass));
    let screen = ctx.screen_rect();
    let age = motion::age(ctx, id.with("age"), 0);
    let enter = Curve::Ease.at((age / tokens::MID).clamp(0.0, 1.0));
    if age < tokens::MID {
        ctx.request_repaint();
    }
    // The scrim: its own layer, taking every click so nothing under it reacts.
    let scrim_layer = egui::LayerId::new(egui::Order::Foreground, id.with("scrim"));
    egui::Area::new(scrim_layer.id).order(egui::Order::Foreground).fixed_pos(screen.min).interactable(true).show(ctx, |ui| {
        ui.painter().rect_filled(screen, 0.0, c(C::Scrim).gamma_multiply(enter));
        ui.allocate_rect(screen, egui::Sense::click_and_drag());
    });
    crate::layer::keep(ctx, id.with("scrim-keep"), scrim_layer, tokens::MID, 1.0, screen.center());

    let max_h = (screen.height() - 64.0).max(160.0);
    let w = spec.width.min(screen.width() - 32.0);
    let foot_id = id.with("foot-h");
    let body_id = id.with("body-h");
    let foot_h = ctx.data(|d| d.get_temp::<f32>(foot_id)).unwrap_or(66.0);
    let body_h = ctx.data(|d| d.get_temp::<f32>(body_id)).unwrap_or(0.0);
    let natural = (body_h + foot_h).min(max_h).max(foot_h + 40.0);
    // Height: eases only across a step change; otherwise follows the content.
    let st_id = id.with("step");
    // A sheet opened anew starts on its step with no slide (the last opening's step is forgotten).
    let fresh = age <= 0.0;
    let st = ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_insert_with(st_id, || StepState { step: spec.step, slide: spec.slide, t0: f64::NEG_INFINITY, from_h: natural, shown_h: natural });
        if fresh {
            *s = StepState { step: spec.step, slide: spec.slide, t0: f64::NEG_INFINITY, from_h: natural, shown_h: natural };
        }
        if s.step != spec.step {
            s.from_h = s.shown_h;
            s.step = spec.step;
            s.slide = spec.slide;
            s.t0 = now;
        }
        let p = ((now - s.t0) as f32 / tokens::STEP).clamp(0.0, 1.0);
        s.shown_h = if p < 1.0 { s.from_h + (natural - s.from_h) * Curve::Ease.at(p) } else { natural };
        *s
    });
    let step_p = ((now - st.t0) as f32 / tokens::STEP).clamp(0.0, 1.0);
    if step_p < 1.0 {
        ctx.request_repaint();
    }
    let card_h = st.shown_h.round();
    let shake = motion::shake_at(ctx, spec.shake_at, 0.36);
    let card = Rect::from_center_size(screen.center(), vec2(w, card_h)).translate(vec2(shake, 0.0));
    let layer = egui::LayerId::new(egui::Order::Foreground, id.with("card"));
    let scale = 0.96 + 0.04 * enter;
    ctx.set_transform_layer(layer, egui::emath::TSTransform { scaling: scale, translation: card.center().to_vec2() * (1.0 - scale) });
    let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let out = egui::Area::new(layer.id)
        .order(egui::Order::Foreground)
        .fixed_pos(card.min)
        .constrain(false)
        .show(ctx, |ui| {
            ui.multiply_opacity(enter);
            ui.set_clip_rect(card.expand(64.0));
            paint::surface(ui.painter(), card, Radius::Sheet, c(C::Surface), Lift::Float);
            let inner_clip = card;
            // The body: scrolls inside what the foot leaves.
            let body_room = (card_h - foot_h).max(0.0);
            let body_rect = Rect::from_min_size(card.min, vec2(w, body_room));
            let mut bui = ui.new_child(egui::UiBuilder::new().max_rect(body_rect).layout(egui::Layout::top_down(egui::Align::Min)));
            bui.set_clip_rect(body_rect.intersect(inner_clip));
            let slide_x = match st.slide {
                Slide::Forward if step_p < 1.0 => 24.0 * (1.0 - Curve::Ease.at(step_p)),
                Slide::Back if step_p < 1.0 => -24.0 * (1.0 - Curve::Ease.at(step_p)),
                _ => 0.0,
            };
            let slide_a = if matches!(st.slide, Slide::Forward | Slide::Back) && step_p < 1.0 { Curve::Ease.at(step_p) } else { 1.0 };
            let (b, content_h) = egui::ScrollArea::vertical()
                .id_salt(("zikaron-sheet-scroll", spec.id))
                .drag_to_scroll(false)
                .max_height(body_room)
                .auto_shrink([false, true])
                .show(&mut bui, |ui| {
                    let r = egui::Frame::new().inner_margin(egui::Margin { left: 24, right: 24, top: 22, bottom: 4 }).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = tokens::S3;
                        motion::shifted(ui, vec2(slide_x, 0.0), slide_a, |ui| body(ui, state))
                    });
                    (r.inner, r.response.rect.height())
                })
                .inner;
            if (content_h - body_h).abs() > 0.5 {
                ui.ctx().data_mut(|d| d.insert_temp(body_id, content_h));
                // Laid out again at once with the new size, so no frame is shown placed by the old one.
                ui.ctx().request_discard("sheet body height changed");
            }
            // The foot: pinned under the body.
            let foot_rect = Rect::from_min_max(pos2(card.left(), card.bottom() - foot_h), card.max);
            let mut fui = ui.new_child(egui::UiBuilder::new().max_rect(foot_rect).layout(egui::Layout::top_down(egui::Align::Min)));
            let fr = egui::Frame::new().inner_margin(egui::Margin { left: 24, right: 24, top: 14, bottom: 18 }).show(&mut fui, |ui| {
                ui.set_width(ui.available_width());
                ui.allocate_ui_with_layout(vec2(ui.available_width(), tokens::KEY_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = tokens::S2;
                    foot(ui, state)
                })
                .inner
            });
            let fh = fr.response.rect.height();
            if (fh - foot_h).abs() > 0.5 {
                ui.ctx().data_mut(|d| d.insert_temp(foot_id, fh));
                // Laid out again at once with the new size, so no frame is shown placed by the old one.
                ui.ctx().request_discard("sheet foot height changed");
            }
            ui.allocate_rect(card, egui::Sense::hover());
            Out { body: b, foot: fr.inner, esc }
        })
        .inner;
    // The card rides directly above its own scrim: a press on the scrim raises the scrim's layer, and without
    // this the scrim would then cover the card for as long as the sheet is open.
    ctx.set_sublayer(scrim_layer, layer);
    crate::layer::keep(ctx, id.with("card-keep"), layer, tokens::MID, 0.96, card.center());
    out
}

/// A sheet's title (17/24 semibold) and an optional line under it.
pub fn title(ui: &mut egui::Ui, s: &str, sub: &str) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.label(egui::RichText::new(s).font(Type::Sheet.font()).color(c(C::Ink)).line_height(Some(Type::Sheet.line())));
        if !sub.is_empty() {
            paint::text(ui, sub, Type::Note, c(C::Ink2));
        }
    });
    ui.add_space((tokens::S4 - ui.spacing().item_spacing.y).max(0.0));
}

/// The quiet words at the left end of a sheet's foot (drawn last in the right-to-left row, so they take what
/// the keys leave).
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

pub fn foot_note(ui: &mut egui::Ui, s: &str) {
    if s.is_empty() {
        return;
    }
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
        let room = ui.available_width();
        paint::line(ui, s, Type::Small, c(C::Ink2), room);
    });
}
