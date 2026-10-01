//! Full-window covers: the passcode gate and the first-run wizard. The cover lies on the page ground over
//! the whole window, fades in over 200 ms, and its content settles up from 10 below over 300 ms; when it
//! goes it fades out over 200 ms. No icon, no mark: words and cells only.

use crate::motion::{self, Curve};
use crate::palette::{c, C};
use crate::tokens;
use egui::{pos2, vec2, Rect};

/// Draw a cover. `add` gets a child the size of the window; the value it returns comes back.
pub fn cover<R>(ctx: &egui::Context, id_salt: &str, add: impl FnOnce(&mut egui::Ui, f32) -> R) -> R {
    let id = egui::Id::new(("zikaron-cover", id_salt));
    let screen = ctx.screen_rect();
    let age = motion::age(ctx, id.with("age"), 0);
    let a = Curve::Ease.at((age / tokens::MID).clamp(0.0, 1.0));
    let settle = Curve::Ease.at((age / tokens::SLOW).clamp(0.0, 1.0));
    if age < tokens::SLOW {
        ctx.request_repaint();
    }
    let layer = egui::LayerId::new(egui::Order::Middle, id);
    let r = egui::Area::new(layer.id).order(egui::Order::Middle).fixed_pos(screen.min).constrain(false).show(ctx, |ui| {
        ui.set_clip_rect(screen);
        ui.painter().rect_filled(screen, 0.0, c(C::Ground).gamma_multiply(a));
        ui.allocate_rect(screen, egui::Sense::click_and_drag());
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(screen).layout(egui::Layout::top_down(egui::Align::Min)));
        child.multiply_opacity(a);
        add(&mut child, 10.0 * (1.0 - settle))
    });
    crate::layer::keep(ctx, id.with("keep"), layer, tokens::MID, 1.0, screen.center());
    r.inner
}

/// The column width [`centered`] gives in `rect` for a wanted `width`.
pub fn centered_w(rect: Rect, width: f32) -> f32 {
    width.min(rect.width() - 32.0).max(0.0)
}

/// A column of `width` centered in `rect` (vertically too, by the height it had last frame), `drop` points
/// lower while it settles. Taller than the room, it scrolls.
pub fn centered<R>(ui: &mut egui::Ui, id_salt: &str, rect: Rect, width: f32, drop: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let id = egui::Id::new(("zikaron-centered", id_salt));
    let h = ui.ctx().data(|d| d.get_temp::<f32>(id)).unwrap_or(0.0);
    let w = centered_w(rect, width);
    let top = (rect.center().y - h / 2.0).max(rect.top() + 24.0) + drop;
    let col = Rect::from_min_size(pos2(rect.center().x - w / 2.0, rect.top()), vec2(w, rect.height()));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(col).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(rect);
    let out = egui::ScrollArea::vertical().id_salt(id.with("scroll")).max_height(rect.height()).auto_shrink([false, true]).show(&mut child, |ui| {
        ui.add_space((top - rect.top()).max(0.0));
        let r = ui.vertical(|ui| {
            ui.set_width(w);
            add(ui)
        });
        ui.add_space(24.0);
        (r.inner, r.response.rect.height())
    });
    let (inner, got) = out.inner;
    if (got - h).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(id, got));
        // Laid out again at once with the new size, so no frame is shown placed by the old one.
        ui.ctx().request_discard("centered column height changed");
    }
    inner
}

/// One step in the wizard's left column.
pub struct Step<'a> {
    pub label: &'a str,
    pub done: bool,
    /// A step that can be done later: its circle is dashed.
    pub later: bool,
    /// Done steps and those before the current one can be clicked.
    pub reachable: bool,
}

/// The wizard's steps: a circle per step (its number; a check once done), joined by a line; the current
/// one in the first ink and the accent. Returns a clicked reachable step.
pub fn steps(ui: &mut egui::Ui, steps: &[Step], current: usize) -> Option<usize> {
    let id = ui.id().with("zikaron-wizard-steps");
    let w = ui.available_width();
    let row_h = 40.0;
    let (area, _) = ui.allocate_exact_size(vec2(w, row_h * steps.len() as f32), egui::Sense::hover());
    let p = ui.painter().clone();
    let mut hit = None;
    for (i, s) in steps.iter().enumerate() {
        let r = Rect::from_min_size(pos2(area.left(), area.top() + row_h * i as f32), vec2(w, row_h));
        let cy = r.center().y;
        let bc = pos2(r.left() + 11.0, cy);
        if i > 0 {
            p.line_segment([pos2(bc.x, cy - row_h / 2.0 - 9.0 + 11.0), pos2(bc.x, cy - 11.0)], egui::Stroke::new(1.5_f32, c(C::Line)));
        }
        let resp = ui.interact(r, id.with(i), if s.reachable { egui::Sense::click() } else { egui::Sense::hover() });
        let on = i == current;
        let done = motion::flag(ui.ctx(), id.with(("done", i)), s.done, tokens::MID);
        let lit = motion::flag(ui.ctx(), id.with(("on", i)), on, tokens::MID);
        let hot = motion::flag(ui.ctx(), id.with(("hot", i)), s.reachable && resp.hovered(), tokens::FAST);
        let edge = crate::palette::mix(crate::palette::mix(c(C::Line), c(C::Accent), lit), c(C::Ok), done);
        p.circle_filled(bc, 11.0, crate::palette::mix(c(C::Surface), c(C::Ok), done));
        if s.later && !s.done {
            let n = 16;
            for k in 0..n {
                if k % 2 == 0 {
                    let a0 = std::f32::consts::TAU * k as f32 / n as f32;
                    let a1 = std::f32::consts::TAU * (k + 1) as f32 / n as f32;
                    p.line_segment([pos2(bc.x + 10.25 * a0.cos(), bc.y + 10.25 * a0.sin()), pos2(bc.x + 10.25 * a1.cos(), bc.y + 10.25 * a1.sin())], egui::Stroke::new(1.5_f32, edge));
                }
            }
        } else {
            p.circle_stroke(bc, 10.25, egui::Stroke::new(1.5_f32, edge));
        }
        if s.done {
            crate::icons::glyph_at(&p, crate::icons::Glyph::Ok, bc, 12.0, egui::Color32::WHITE.gamma_multiply(done));
        } else {
            let num = crate::palette::mix(c(C::Ink3), c(C::Accent), lit);
            p.text(bc, egui::Align2::CENTER_CENTER, (i + 1).to_string(), egui::FontId::new(tokens::Type::Micro.size(), crate::fonts::strong()), num);
        }
        let ink = crate::palette::mix(crate::palette::mix(c(C::Ink3), c(C::Ink), hot), c(C::Ink), lit);
        let font = if on { egui::FontId::new(tokens::Type::Small.size(), crate::fonts::strong()) } else { crate::tokens::Type::Small.font() };
        p.text(pos2(r.left() + 34.0, cy), egui::Align2::LEFT_CENTER, s.label, font, ink);
        if s.reachable && resp.clicked() {
            hit = Some(i);
        }
        if s.reachable {
            let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
    }
    hit
}

/// A choice with a radio (the wizard's networks): a title, an optional pill, a line under it; the chosen
/// one on the selection ground with an accent edge, its dot popping in.
pub fn radio_row(ui: &mut egui::Ui, id: egui::Id, on: bool, title: &str, pill: Option<&str>, sub: &str) -> egui::Response {
    let w = ui.available_width();
    let h = 12.0 + crate::tokens::Type::Strong.line() + 2.0 + crate::tokens::Type::Small.line() + 12.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), egui::Sense::click());
    let lit = motion::flag(ui.ctx(), id.with("on"), on, tokens::FAST);
    let dot = motion::to(ui.ctx(), id.with("dot"), if on { 1.0 } else { 0.0 }, tokens::FAST, Curve::Spring);
    let p = ui.painter().clone();
    let fill = crate::palette::mix(c(C::Surface), c(C::Sel), lit);
    let edge = crate::palette::mix(c(C::Line), c(C::Accent), lit);
    p.rect(rect, crate::tokens::Radius::Menu.egui(), fill, egui::Stroke::new(1.0_f32, edge), egui::StrokeKind::Inside);
    let rc = pos2(rect.left() + 14.0 + 8.0, rect.center().y);
    p.circle_stroke(rc, 7.5, egui::Stroke::new(1.5_f32, edge));
    if dot > 0.0 {
        p.circle_filled(rc, 4.5 * dot, c(C::Accent));
    }
    let x = rc.x + 8.0 + 10.0;
    let ty = rect.top() + 12.0;
    let tr = p.text(pos2(x, ty), egui::Align2::LEFT_TOP, title, crate::tokens::Type::Strong.font(), c(C::Ink));
    if let Some(s) = pill {
        let pw = crate::mark::pill_w(ui, s, false);
        let pr = Rect::from_min_size(pos2(tr.right() + 8.0, tr.center().y - tokens::PILL_H / 2.0), vec2(pw, tokens::PILL_H));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(pr));
        crate::mark::pill(&mut child, s, crate::palette::Tone::Ok);
    }
    p.text(pos2(x, ty + crate::tokens::Type::Strong.line() + 2.0), egui::Align2::LEFT_TOP, sub, crate::tokens::Type::Small.font(), c(C::Ink2));
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}
