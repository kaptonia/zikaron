use crate::motion::{self, Curve};
use crate::paint;
use crate::palette::{c, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Color32, Rect};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slide {
    None,
    Forward,
    Back,
}

#[derive(Clone, Copy, Debug)]
pub struct Spec<'a> {
    pub id: &'a str,
    pub width: f32,
    pub step: u64,
    pub slide: Slide,
    pub shake_at: Option<f64>,
    pub keys_above: f32,
}

impl<'a> Spec<'a> {
    pub fn new(id: &'a str, width: f32) -> Spec<'a> {
        Spec { id, width, step: 0, slide: Slide::None, shake_at: None, keys_above: FOOT_ABOVE + BODY_FOOT }
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

    pub fn keys_above(mut self, room: f32) -> Self {
        self.keys_above = room;
        self
    }
}

pub struct Out<B, F> {
    pub body: B,
    pub foot: F,
    pub esc: bool,
}

#[derive(Clone, Copy)]
struct StepState {
    step: u64,
    slide: Slide,
    t0: f64,
}

const GROW_FROM: f32 = 0.92;
const GROW: f32 = 0.44 * crate::tokens::PACE;
const CONTENT_DELAY: f32 = 0.07 * crate::tokens::PACE;
const CONTENT_IN: f32 = 0.24 * crate::tokens::PACE;
const LEAVE_TO: f32 = 0.97;
const LEAVE: f32 = 0.14 * crate::tokens::PACE;

pub(crate) fn inside_id() -> egui::Id {
    egui::Id::new("zikaron-sheet-inside")
}

fn title_id() -> egui::Id {
    egui::Id::new("zikaron-sheet-title")
}

pub fn echoes_title(ctx: &egui::Context, label: &str) -> bool {
    let label = label.trim();
    !label.is_empty() && inside(ctx) && ctx.data(|d| d.get_temp::<String>(title_id())).map(|t| t.trim() == label).unwrap_or(false)
}

pub(crate) fn said_title(ctx: &egui::Context, s: &str) {
    ctx.data_mut(|d| d.insert_temp(title_id(), s.to_string()));
}

fn title_line_id() -> egui::Id {
    egui::Id::new("zikaron-sheet-title-line")
}

pub(crate) fn said_title_line(ctx: &egui::Context, line: Rect, ends: f32) {
    ctx.data_mut(|d| d.insert_temp(title_line_id(), (line, ends)));
}

pub(crate) fn title_line(ctx: &egui::Context) -> Option<(Rect, f32)> {
    ctx.data(|d| d.get_temp::<(Rect, f32)>(title_line_id()))
}

pub(crate) fn rows_corner_id() -> egui::Id {
    egui::Id::new("zikaron-rows-corner")
}

pub fn inside(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(inside_id())).unwrap_or(false)
}

pub fn up(ctx: &egui::Context) -> bool {
    let pass = crate::motion::pass(ctx);
    ctx.data(|d| d.get_temp::<u64>(egui::Id::new("zikaron-sheet-up"))).map(|at| at + 1 >= pass).unwrap_or(false)
}

pub fn entered_at(ctx: &egui::Context, id: &str) -> Option<f64> {
    crate::motion::started(ctx, egui::Id::new(("zikaron-sheet", id)).with("age"))
}

pub fn show<T, B, F>(ctx: &egui::Context, spec: Spec, state: &mut T, body: impl FnOnce(&mut egui::Ui, &mut T) -> B, foot: impl FnOnce(&mut egui::Ui, &mut T) -> F) -> Out<B, F> {
    let id = egui::Id::new(("zikaron-sheet", spec.id));
    let now = crate::motion::now(ctx);
    let pass = crate::motion::pass(ctx);
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("zikaron-sheet-up"), pass));
    let screen = ctx.content_rect();
    let grow = crate::layer::entrance(ctx, &[id.with("card-keep")], id.with("age"), GROW);
    let age = motion::age(ctx, id.with("age"), 0);
    let enter = Curve::Ease.at((age / tokens::MID).clamp(0.0, 1.0)).max(grow);
    let content = Curve::Ease.at(((age - CONTENT_DELAY) / CONTENT_IN).clamp(0.0, 1.0)).max(if grow >= 1.0 { 1.0 } else { 0.0 });
    let origin_id = id.with("origin");
    if age <= 0.0 {
        let at = ctx.input(|i| i.pointer.latest_pos()).filter(|p| screen.contains(*p)).unwrap_or(screen.center());
        ctx.data_mut(|d| d.insert_temp(origin_id, at));
    }
    let origin = ctx.data(|d| d.get_temp::<egui::Pos2>(origin_id)).unwrap_or(screen.center());
    let layer = egui::LayerId::new(egui::Order::Foreground, id.with("card"));
    crate::layer::place(ctx, layer, None);

    let max_h = (screen.height() - 64.0).max(160.0);
    let keys_w_id = id.with("keys-w");
    let w = spec.width.max(ctx.data(|d| d.get_temp::<f32>(keys_w_id)).unwrap_or(0.0)).min(screen.width() - 32.0);
    let foot_id = id.with("foot-h");
    let body_id = id.with("body-h");
    let foot_h = ctx.data(|d| d.get_temp::<f32>(foot_id)).unwrap_or(66.0);
    let body_h = ctx.data(|d| d.get_temp::<f32>(body_id)).unwrap_or(0.0);
    let trim_id = id.with("trim");
    let trim = ctx.data(|d| d.get_temp::<f32>(trim_id)).unwrap_or(0.0);
    let foot_now = (foot_h - trim).max(0.0);
    let natural = (body_h + foot_now).min(max_h).max(foot_h + 40.0);
    let fits = crate::scroll::holds(body_h + foot_now, max_h);
    let st_id = id.with("step");
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
    let step_p = (since / tokens::SLOW).clamp(0.0, 1.0);
    let step_in = Curve::Ease.at(step_p);
    if step_p < 1.0 || step_in < 1.0 {
        ctx.request_repaint();
    }
    let inner_id = id.with("inner-moving");
    let inner_was = ctx.data(|d| d.get_temp::<bool>(inner_id)).unwrap_or(false);
    let card_h = crate::flow::height_following(ctx, id.with("card-h"), natural, w, inner_was).round();
    let shake = motion::shake_at(ctx, spec.shake_at, 0.36 * crate::tokens::PACE);
    let card = Rect::from_center_size(screen.center(), vec2(w, card_h)).translate(vec2(shake, 0.0));
    crate::probe::sheet(ctx, crate::probe::Sheet { id: spec.id.to_string(), card, content: body_h + foot_now, room: max_h, bar: !fits });
    let scale = 1.0 - (1.0 - GROW_FROM) * (1.0 - grow) * motion::travel(ctx);
    ctx.set_transform_layer(layer, egui::emath::TSTransform { scaling: scale, translation: origin.to_vec2() * (1.0 - scale) });
    let esc = crate::layer::esc(ctx, id);
    let out = crate::layer::over(ctx, layer)
        .show(ctx, |ui| {
            let screen = crate::layer::ground(ui);
            ui.painter().rect_filled(screen, 0.0, c(C::Scrim).gamma_multiply(enter));
            crate::layer::under(ui, crate::layer::Under::Taken);
            crate::layer::body(ui, card);
            ui.multiply_opacity(enter);
            ui.set_clip_rect(card.expand(64.0));
            crate::material::paint(ui.painter(), card, Radius::Sheet, crate::material::Level::Sheet, 0.0);
            ui.multiply_opacity(if enter > 0.0 { (content / enter).min(1.0) } else { 0.0 });
            let inner_clip = card;
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
                    let r = egui::Frame::new().inner_margin(egui::Margin { left: SIDE, right: SIDE, top: 22, bottom: BODY_FOOT as i8 }).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.spacing_mut().item_spacing.y = tokens::S3;
                        let was = inside(ui.ctx());
                        ui.ctx().data_mut(|d| d.insert_temp(inside_id(), true));
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
            let ends = ui.ctx().data(|d| d.get_temp::<(f32, f32)>(trailing()));
            let trim_now = ends.filter(|(bottom, _)| (bottom + BODY_FOOT - content_bottom).abs() < crate::scroll::SLACK).map(|(_, t)| t.round()).unwrap_or(0.0);
            if (content_h - body_h).abs() > 0.5 || (trim_now - trim).abs() > 0.5 {
                ui.ctx().data_mut(|d| d.insert_temp(body_id, content_h));
                ui.ctx().data_mut(|d| d.insert_temp(trim_id, trim_now));
                ui.ctx().request_discard("sheet body height changed");
            }
            let foot_rect = Rect::from_min_max(pos2(card.left(), card.bottom() - foot_now), card.max);
            foot_band(ui.painter(), foot_rect, Radius::Sheet.px());
            let mut fui = ui.new_child(egui::UiBuilder::new().max_rect(foot_rect).layout(egui::Layout::top_down(egui::Align::Min)));
            let fr = egui::Frame::new().inner_margin(egui::Margin { left: SIDE, right: SIDE, top: (spec.keys_above - BODY_FOOT - trim).max(0.0) as i8, bottom: 22 }).show(&mut fui, |ui| {
                ui.set_width(ui.available_width());
                ui.allocate_ui_with_layout(vec2(ui.available_width(), tokens::KEY_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = tokens::S2;
                    let row = ui.max_rect();
                    let keys = foot(ui, state);
                    let laid = ui.min_rect();
                    let pressed: Vec<Rect> = ui.ctx().viewport(|v| v.this_pass.widgets.get_layer(ui.layer_id()).filter(|w| w.sense.senses_click() && laid.expand(0.5).contains_rect(w.interact_rect)).map(|w| w.rect).collect());
                    let asked = keys_width(&pressed);
                    let wants = if asked > spec.width + 0.5 { asked } else { 0.0 };
                    if (ui.ctx().data(|d| d.get_temp::<f32>(keys_w_id)).unwrap_or(0.0) - wants).abs() > 0.5 {
                        ui.ctx().data_mut(|d| d.insert_temp(keys_w_id, wants));
                        ui.ctx().request_discard("sheet keys wider than the sheet");
                    }
                    if !ui.ctx().will_discard() && (crate::probe::overruns(row, laid) || crate::probe::overlap(&pressed)) {
                        crate::probe::foot_out(crate::probe::FootOut { id: spec.id.to_string(), row, laid });
                    }
                    keys
                })
                .inner
            });
            let fh = fr.response.rect.height() + trim;
            if (fh - foot_h).abs() > 0.5 {
                ui.ctx().data_mut(|d| d.insert_temp(foot_id, fh));
                ui.ctx().request_discard("sheet foot height changed");
            }
            ui.allocate_rect(card, egui::Sense::hover());
            Out { body: b, foot: fr.inner, esc }
        })
        .inner;
    crate::layer::keep(ctx, id.with("card-keep"), layer, LEAVE, LEAVE_TO, card.center(), egui::Vec2::ZERO);
    out
}

fn keys_width(keys: &[Rect]) -> f32 {
    if keys.is_empty() {
        return 0.0;
    }
    keys.iter().map(|r| r.width()).sum::<f32>() + tokens::S2 * (keys.len() - 1) as f32 + 2.0 * tokens::SHEET_SIDE
}

const FOOT_ABOVE: f32 = 34.0;

const BODY_FOOT: f32 = 4.0;

const SIDE: i8 = tokens::SHEET_SIDE as i8;

pub(crate) fn trailing() -> egui::Id {
    egui::Id::new("zikaron-sheet-trailing")
}

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

pub fn title(ui: &mut egui::Ui, s: &str, sub: &str) {
    said_title(ui.ctx(), s);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let w = ui.available_width();
        let said = ui.label(egui::RichText::new(s).font(Type::Sheet.font()).color(c(C::Ink)).line_height(Some(Type::Sheet.line())));
        said_title_line(ui.ctx(), Rect::from_min_size(said.rect.min, vec2(w, Type::Sheet.line())), said.rect.right());
        if !sub.is_empty() {
            paint::text(ui, sub, Type::Note, c(C::Ink2));
        }
    });
    ui.add_space(6.0 - tokens::S3);
}

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
    let room = ui.available_width();
    let g = paint::wrapped(ui, s, Type::Small.font(), c(C::Ink2), Type::Small.line(), room);
    let h = g.size().y.max(Type::Small.line());
    crate::width::cell(ui, vec2(room, h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        let (r, _) = ui.allocate_exact_size(vec2(g.size().x, h), egui::Sense::hover());
        ui.painter().galley(pos2(r.left(), r.center().y - g.size().y / 2.0), g, c(C::Ink2));
    });
}
