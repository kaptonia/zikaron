use crate::icons::{self, Glyph};
use crate::motion::{self, Curve};
use crate::palette::{self, c, C};
use crate::tokens::Type;
use egui::{pos2, vec2, Rect};

pub fn glide(t: f32) -> f32 {
    motion::bezier(0.22, 1.0, 0.36, 1.0, t.clamp(0.0, 1.0))
}

const FADE: f32 = 0.3 * crate::tokens::PACE;
const SETTLE: f32 = 0.4 * crate::tokens::PACE;
const FROM: f32 = 1.004;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ground {
    Lit,
    Clear,
}

pub fn cover<R>(ctx: &egui::Context, id_salt: &str, ground: Ground, add: impl FnOnce(&mut egui::Ui) -> R) -> (R, egui::LayerId) {
    let id = egui::Id::new(("zikaron-cover-as-designed", id_salt));
    let screen = ctx.content_rect();
    let a = crate::layer::entrance(ctx, &[id.with("keep")], id.with("age"), FADE);
    let age = motion::age(ctx, id.with("age"), 0);
    if age < SETTLE {
        ctx.request_repaint();
    }
    let scale = 1.0 + (FROM - 1.0) * (1.0 - glide(age / SETTLE)) * motion::travel(ctx);
    let layer = egui::LayerId::new(egui::Order::Middle, id);
    crate::layer::place(ctx, layer, None);
    ctx.set_transform_layer(layer, egui::emath::TSTransform { scaling: scale, translation: screen.center().to_vec2() * (1.0 - scale) });
    let r = crate::layer::over(ctx, layer).show(ctx, |ui| {
        crate::layer::under(ui, crate::layer::Under::Ground(egui::Color32::TRANSPARENT));
        if ground == Ground::Lit {
            let all = crate::layer::ground(ui);
            ui.painter().rect_filled(all, 0.0, c(C::Ground).gamma_multiply(a));
            lit(ui.painter(), screen, a);
        }
        ui.set_clip_rect(screen);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(screen).layout(egui::Layout::top_down(egui::Align::Min)));
        child.multiply_opacity(a);
        add(&mut child)
    });
    crate::layer::keep(ctx, id.with("keep"), layer, FADE, FROM, screen.center(), egui::Vec2::ZERO);
    (r.inner, layer)
}

pub fn gone(ctx: &egui::Context, id_salt: &str) {
    let cover = egui::Id::new(("zikaron-cover-as-designed", id_salt)).with("keep");
    let panes = ctx.data(|d| d.get_temp::<Vec<egui::Id>>(panes_id())).unwrap_or_default();
    crate::layer::forget(ctx, |k| k == cover || panes.contains(&k));
}

fn panes_id() -> egui::Id {
    egui::Id::new("zikaron-wizard-pane-keeps")
}

fn lit(p: &egui::Painter, rect: Rect, alpha: f32) {
    let centre = pos2(rect.left() + rect.width() * 0.5, rect.top() + rect.height() * 0.4);
    let (rx, ry) = (rect.width() * 0.7, rect.height() * 0.6);
    let (inner, outer) = (c(C::Surface), c(C::Ground));
    let rings = 24usize;
    let segs = 72usize;
    let mut mesh = egui::Mesh::default();
    let at = |k: usize, j: usize| {
        let t = k as f32 / rings as f32;
        let ang = std::f32::consts::TAU * j as f32 / segs as f32;
        pos2(centre.x + rx * t * ang.cos(), centre.y + ry * t * ang.sin())
    };
    for k in 0..=rings {
        let col = palette::mix(inner, outer, k as f32 / rings as f32).gamma_multiply(alpha);
        for j in 0..segs {
            mesh.colored_vertex(at(k, j), col);
        }
    }
    for k in 0..rings {
        for j in 0..segs {
            let a0 = (k * segs + j) as u32;
            let a1 = (k * segs + (j + 1) % segs) as u32;
            let b0 = ((k + 1) * segs + j) as u32;
            let b1 = ((k + 1) * segs + (j + 1) % segs) as u32;
            mesh.add_triangle(a0, b0, b1);
            mesh.add_triangle(a0, b1, a1);
        }
    }
    p.with_clip_rect(rect).add(egui::Shape::mesh(mesh));
}

pub fn lock_head(ui: &mut egui::Ui, app: &str, title: &str) -> f32 {
    let end = ui.vertical_centered(|ui| {
        ui.spacing_mut().item_spacing.y = 14.0;
        ui.label(egui::RichText::new(app).font(egui::FontId::new(Type::MonoSub.size(), egui::FontFamily::Proportional)).color(c(C::Ink2)));
        let (at, g, _) = egui::Label::new(egui::RichText::new(title).font(egui::FontId::new(22.0, crate::fonts::strong())).color(c(C::Ink))).layout_in_ui(ui);
        let end = at.y + g.mesh_bounds.max.y;
        ui.painter().galley(at, g, c(C::Ink));
        end
    });
    ui.add_space(LOCK_HEAD_FOOT);
    end.inner
}

pub const LOCK_HEAD_FOOT: f32 = 6.0;

pub fn small_key(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
    let k = crate::button::Key::new(text, crate::button::Role::Secondary).enabled(enabled);
    let words = ui.painter().layout_no_wrap(text.to_string(), crate::button::key_font(), egui::Color32::BLACK).size().x;
    let (rect, resp) = ui.allocate_exact_size(vec2(words + 22.0, 28.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    crate::button::paint(ui, rect, &resp, &k, crate::button::Phase::Idle);
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

pub const RAIL_PAD_X: f32 = 18.0;

pub fn rail_ground(p: &egui::Painter, rect: Rect, alpha: f32) {
    p.rect_filled(rect, 0.0, crate::material::rail_ground().gamma_multiply(alpha));
    p.vline(rect.right() - 0.5, rect.y_range(), egui::Stroke::new(1.0_f32, c(C::Line2).gamma_multiply(alpha)));
}

pub fn rail_head(ui: &mut egui::Ui, title: &str, app: &str) {
    ui.add_space(14.0);
    ui.label(egui::RichText::new(title).font(egui::FontId::new(15.0, crate::fonts::strong())).color(c(C::Ink)));
    ui.add_space(2.0 - ui.spacing().item_spacing.y);
    ui.label(egui::RichText::new(app).font(Type::Tiny.font()).color(c(C::Ink2)));
    ui.add_space((18.0 - ui.spacing().item_spacing.y).max(0.0));
}

pub struct Step<'a> {
    pub label: &'a str,
    pub done: bool,
    pub reachable: bool,
}

const TURN: f32 = 0.35 * crate::tokens::PACE;
const POP2: f32 = 0.5 * crate::tokens::PACE;

pub fn steps_width(ui: &egui::Ui, labels: &[&str]) -> f32 {
    let font = egui::FontId::new(Type::InnerKey.size(), crate::fonts::strong());
    STEP_TEXT_X + labels.iter().map(|l| crate::width::width_in(ui, l, font.clone())).fold(0.0, f32::max)
}

const STEP_TEXT_X: f32 = 30.0;

pub fn steps(ui: &mut egui::Ui, steps: &[Step], current: usize) -> Option<usize> {
    let id = ui.id().with("zikaron-wizard-steps-as-designed");
    let w = ui.available_width();
    let (row_h, gap) = (32.0, 4.0);
    let n = steps.len() as f32;
    let (area, _) = ui.allocate_exact_size(vec2(w, row_h * n + gap * (n - 1.0).max(0.0)), egui::Sense::hover());
    let p = ui.painter().clone();
    let mut hit = None;
    for (i, s) in steps.iter().enumerate() {
        let r = Rect::from_min_size(pos2(area.left(), area.top() + (row_h + gap) * i as f32), vec2(w, row_h));
        let resp = ui.interact(r, id.with(i), if s.reachable { egui::Sense::click() } else { egui::Sense::hover() });
        let on = i == current;
        let done = motion::flag(ui.ctx(), id.with(("done", i)), s.done, TURN);
        let lit = motion::flag(ui.ctx(), id.with(("on", i)), on && !s.done, TURN);
        let hot = motion::states(ui.ctx(), id.with(("step", i)), resp.hovered(), resp.is_pointer_button_down_on(), resp.has_focus(), s.reachable).hover;
        let since = motion::age(ui.ctx(), id.with(("pop", i)), u64::from(s.done));
        let pop = if s.done { (since / POP2).clamp(0.0, 1.0) } else { 1.0 };
        if pop < 1.0 {
            ui.ctx().request_repaint();
        }
        let k = 1.0 - 0.15 * (1.0 - Curve::Spring.at(pop)) * motion::travel(ui.ctx());
        let bc = pos2(r.left() + STEP_TEXT_X - 20.0, r.center().y);
        let ring = palette::mix(c(C::Line), c(C::Accent), lit);
        p.circle_stroke(bc, 9.25 * k, egui::Stroke::new(1.5_f32, ring.gamma_multiply(1.0 - done)));
        if done > 0.0 {
            p.circle_filled(bc, 10.0 * k, c(C::Ok).gamma_multiply(done));
            icons::glyph_at(&p, Glyph::Ok, bc, 11.0 * k, egui::Color32::WHITE.gamma_multiply(done));
        }
        if done < 1.0 {
            let num = palette::mix(c(C::Ink3), c(C::Accent), lit).gamma_multiply(1.0 - done);
            p.text(bc, egui::Align2::CENTER_CENTER, (i + 1).to_string(), Type::Micro.font(), num);
        }
        let ink = if on {
            c(C::Ink)
        } else if s.done {
            palette::mix(c(C::Ink2), c(C::Ink), hot)
        } else {
            palette::mix(c(C::Ink3), c(C::Ink2), hot)
        };
        let font = if on { egui::FontId::new(Type::InnerKey.size(), crate::fonts::strong()) } else { egui::FontId::new(Type::InnerKey.size(), egui::FontFamily::Proportional) };
        let room = (r.right() - (bc.x + 20.0)).max(0.0);
        let fit = crate::width::elide_to(ui, s.label, font.clone(), room);
        p.text(pos2(bc.x + 20.0, r.center().y), egui::Align2::LEFT_CENTER, fit, font, ink);
        if s.reachable && resp.clicked() {
            hit = Some(i);
        }
        if s.reachable {
            let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
    }
    hit
}

pub fn exit_item(ui: &mut egui::Ui, label: &str, kbd: &str, shown: bool) -> egui::Response {
    let id = ui.id().with(("zikaron-wizard-exit", label.len()));
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 4.0 + 18.0 + 4.0), if shown { egui::Sense::click() } else { egui::Sense::hover() });
    let a = motion::flag(ui.ctx(), id.with("shown"), shown, FADE);
    let since = motion::age(ui.ctx(), id.with("rise"), u64::from(shown));
    if since < SETTLE {
        ui.ctx().request_repaint();
    }
    let rise = if shown { 4.0 * (1.0 - glide(since / SETTLE)) * motion::travel(ui.ctx()) } else { 4.0 * (1.0 - a) };
    if a <= 0.0 {
        return resp;
    }
    let hot = motion::states(ui.ctx(), id.with("hot"), resp.hovered(), resp.is_pointer_button_down_on(), resp.has_focus(), shown).hover;
    let ink = palette::mix(c(C::Ink2), c(C::Ink), hot).gamma_multiply(a);
    let p = ui.painter();
    let y = rect.center().y + rise;
    icons::glyph_at(p, Glyph::Back, pos2(rect.left() + 7.0, y), 14.0, ink);
    p.text(pos2(rect.left() + 14.0 + 6.0, y), egui::Align2::LEFT_CENTER, label, Type::Small.font(), ink);
    if !kbd.is_empty() {
        p.text(pos2(rect.right(), y), egui::Align2::RIGHT_CENTER, kbd, egui::FontId::new(Type::Micro.size(), egui::FontFamily::Monospace), c(C::Ink3).gamma_multiply(a));
    }
    if shown {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

pub fn step_head(ui: &mut egui::Ui, count: &str, title: &str) {
    ui.label(egui::RichText::new(count).font(egui::FontId::new(Type::MonoSub.size(), egui::FontFamily::Proportional)).color(c(C::Ink2)));
    ui.add_space(2.0 - ui.spacing().item_spacing.y);
    ui.label(egui::RichText::new(title).font(Type::Page.font()).color(c(C::Ink)).line_height(Some(Type::Page.line())));
    ui.add_space((14.0 - ui.spacing().item_spacing.y).max(0.0));
}

pub fn seg_small(ui: &mut egui::Ui, id_salt: &str, labels: &[&str], current: usize) -> Option<usize> {
    let cells: Vec<crate::seg::Cell> = labels.iter().map(|l| crate::seg::Cell::from(*l)).collect();
    crate::seg::seg_small(ui, id_salt, &cells, current)
}

const PANE_IN: f32 = 0.38 * crate::tokens::PACE;
const PANE_FROM: f32 = 12.0;
const PANE_OUT: f32 = 0.16 * crate::tokens::PACE;
const PANE_TO: f32 = 10.0;

pub fn pane<R>(ctx: &egui::Context, over: egui::LayerId, step: u64, dir: f32, rect: Rect, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let id = egui::Id::new(("zikaron-wizard-pane", step));
    let layer = egui::LayerId::new(egui::Order::Middle, id);
    crate::layer::place(ctx, layer, Some(over));
    let keep = egui::Id::new("zikaron-wizard-pane-keep").with(step);
    let last = egui::Id::new("zikaron-wizard-pane-last");
    let was = ctx.data(|d| d.get_temp::<u64>(last));
    if let Some(w) = was.filter(|w| *w != step) {
        crate::layer::steer(ctx, egui::Id::new("zikaron-wizard-pane-keep").with(w), vec2(-PANE_TO * dir.signum(), 0.0));
    }
    ctx.data_mut(|d| d.insert_temp(last, step));
    let age = motion::age(ctx, id.with("age"), 0);
    let t = (age / PANE_IN).clamp(0.0, 1.0);
    if t < 1.0 {
        ctx.request_repaint();
    }
    let e = Curve::Spring.at(t);
    let off = if was.is_some() { PANE_FROM * dir * (1.0 - e) * motion::travel(ctx) } else { 0.0 };
    let alpha = if was.is_some() { e.clamp(0.0, 1.0) } else { 1.0 };
    ctx.set_transform_layer(layer, egui::emath::TSTransform { scaling: 1.0, translation: vec2(off, 0.0) });
    let r = crate::layer::area(layer.id, layer.order).fixed_pos(rect.min).constrain(false).show(ctx, |ui| {
        ui.expand_to_include_rect(rect);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Min)));
        child.set_clip_rect(rect);
        child.multiply_opacity(alpha);
        add(&mut child)
    });
    crate::layer::keep(ctx, keep, layer, PANE_OUT, 1.0, rect.center(), egui::Vec2::ZERO);
    ctx.data_mut(|d| {
        let v = d.get_temp_mut_or_default::<Vec<egui::Id>>(panes_id());
        if !v.contains(&keep) {
            v.push(keep);
        }
    });
    r.inner
}

pub fn app_in(ctx: &egui::Context, up: bool) {
    const DUR: f32 = 0.45 * crate::tokens::PACE;
    let id = egui::Id::new("zikaron-app-in");
    let now = crate::motion::now(ctx);
    let (was_up, since) = ctx.data(|d| d.get_temp::<(bool, Option<f64>)>(id)).unwrap_or((up, None));
    let since = if was_up && !up { Some(now) } else if up { None } else { since };
    ctx.data_mut(|d| d.insert_temp(id, (up, since)));
    let Some(at) = since else { return };
    let t = ((now - at) as f32 / DUR).clamp(0.0, 1.0);
    let e = glide(t);
    let screen = ctx.content_rect();
    let scale = 1.0 - 0.007 * (1.0 - e) * motion::travel(ctx);
    ctx.set_transform_layer(egui::LayerId::background(), egui::emath::TSTransform { scaling: scale, translation: screen.center().to_vec2() * (1.0 - scale) });
    if t >= 1.0 {
        ctx.data_mut(|d| d.insert_temp(id, (up, None::<f64>)));
        return;
    }
    ctx.request_repaint();
    let veil = egui::LayerId::new(egui::Order::Middle, id.with("veil"));
    ctx.layer_painter(veil).rect_filled(screen, 0.0, c(C::Ground).gamma_multiply(0.5 * (1.0 - e)));
    ctx.data_mut(|d| d.insert_temp(id.with("reading"), (0.5 + 0.5 * e, scale)));
}

pub fn app_in_reading(ctx: &egui::Context) -> Option<(f32, f32)> {
    let id = egui::Id::new("zikaron-app-in");
    let on = ctx.data(|d| d.get_temp::<(bool, Option<f64>)>(id)).and_then(|(_, s)| s).is_some();
    if on { ctx.data(|d| d.get_temp::<(f32, f32)>(id.with("reading"))) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_glide_runs_from_nothing_to_whole() {
        assert!(glide(0.0).abs() < 1e-4 && (glide(1.0) - 1.0).abs() < 1e-4);
        let mut last = 0.0;
        for k in 1..=20 {
            let v = glide(k as f32 / 20.0);
            assert!(v >= last - 1e-4 && v <= 1.0 + 1e-4, "monotone, no overshoot");
            last = v;
        }
    }
}
