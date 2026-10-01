//! The three states of a block (empty, loading, failed) and the small boxes that speak on a page: notes,
//! banners, hints and one-line readings.

use crate::icons::{self, Glyph};
use crate::mark::{self, Mark};
use crate::motion;
use crate::paint;
use crate::palette::{self, c, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

/// Empty: a quiet line glyph and one sentence, centered, 40 above and below.
pub fn empty(ui: &mut egui::Ui, g: Glyph, s: &str) {
    empty_pad(ui, g, s, 40.0)
}

pub fn empty_pad(ui: &mut egui::Ui, g: Glyph, s: &str, pad_y: f32) {
    let w = ui.available_width();
    let text = paint::galley(ui, s, Type::Body, c(C::Ink2));
    let h = pad_y * 2.0 + 28.0 + 10.0 + text.size().y;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
    let p = ui.painter();
    icons::glyph_at(p, g, pos2(rect.center().x, rect.top() + pad_y + 14.0), 28.0, c(C::Ink3));
    p.galley(pos2(rect.center().x - text.size().x / 2.0, rect.top() + pad_y + 28.0 + 10.0), text, c(C::Ink2));
}

/// One skeleton bar, shimmering once every 1.2 s (drawn only while something loads).
pub fn skeleton(ui: &mut egui::Ui, w: f32, h: f32, radius: f32) -> Rect {
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
    paint_skeleton(ui, rect, radius);
    rect
}

/// Paint a shimmering bar into `rect`.
pub fn paint_skeleton(ui: &egui::Ui, rect: Rect, radius: f32) {
    let phase = motion::cycle(ui.ctx(), tokens::CYCLE);
    let lo = c(C::Skel);
    let hi = c(C::SkelHi);
    // The highlight's center sweeps from past the right edge to past the left edge.
    let w = rect.width().max(1.0);
    let center = rect.right() + w * 0.5 - phase * w * 2.0;
    let colour_at = |x: f32| {
        let d = ((x - center) / w).abs().min(1.0);
        palette::mix(hi, lo, d)
    };
    let n = 24usize;
    let mut mesh = egui::Mesh::default();
    let r = radius.min(rect.height() / 2.0).min(rect.width() / 2.0);
    let edge = |x: f32| -> (f32, f32) {
        // Top and bottom of the rounded bar at x.
        let dx = if x < rect.left() + r {
            rect.left() + r - x
        } else if x > rect.right() - r {
            x - (rect.right() - r)
        } else {
            0.0
        };
        let dy = if dx > 0.0 { r - (r * r - dx * dx).max(0.0).sqrt() } else { 0.0 };
        (rect.top() + dy, rect.bottom() - dy)
    };
    for i in 0..=n {
        let x = rect.left() + rect.width() * i as f32 / n as f32;
        let (t, b) = edge(x);
        let col = colour_at(x);
        mesh.colored_vertex(pos2(x, t), col);
        mesh.colored_vertex(pos2(x, b), col);
        if i > 0 {
            let k = (i as u32) * 2;
            mesh.add_triangle(k - 2, k - 1, k);
            mesh.add_triangle(k - 1, k + 1, k);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

/// A block of skeleton lines at the given widths (fractions of the block).
pub fn skeleton_lines(ui: &mut egui::Ui, widths: &[f32]) {
    let w = ui.available_width();
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = tokens::S3;
        for f in widths {
            skeleton(ui, w * f, 12.0, 6.0);
        }
    });
}

/// The failed state: a red-edged box with what happened in bold, what to do next, and the raw error folded
/// under "error details".
pub fn err_box(ui: &mut egui::Ui, id_salt: &str, what: &str, next: &str, raw_title: &str, raw: &str) {
    boxed(ui, c(C::BadBox), c(C::BadEdge), |ui| {
        ui.label(egui::RichText::new(what).font(egui::FontId::new(15.0, crate::fonts::strong())).color(c(C::BadInk)));
        if !next.is_empty() {
            paint::text(ui, next, Type::Note, c(C::Ink2));
        }
        if !raw.is_empty() {
            ui.add_space(4.0);
            crate::fold::fold(ui, id_salt, raw_title, |ui| {
                paint::text(ui, raw, Type::MonoSmall, c(C::Ink3));
            });
        }
    });
}

fn boxed(ui: &mut egui::Ui, fill: egui::Color32, line: egui::Color32, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(1.0_f32, line))
        .corner_radius(Radius::Menu.egui())
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 2.0;
            add(ui);
        });
}

/// A grey note in the rail's ground: 14/20 in the second ink.
pub fn note_box(ui: &mut egui::Ui, s: &str) {
    egui::Frame::new()
        .fill(c(C::Rail))
        .corner_radius(Radius::Ctl.egui())
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            paint::text(ui, s, Type::Note, c(C::Ink2));
        });
}

/// Banner kinds. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Banner {
    /// A conclusion that holds: a green check and words, no box.
    Ok,
    /// Something that is wrong: a red box.
    Bad,
    /// Something to mind: an amber box (read-only, handed over).
    Warn,
    /// A plain note in a lined box.
    Note,
}

/// A banner: one line in bold with its mark; `key` draws a key at its right end.
pub fn banner<R>(ui: &mut egui::Ui, kind: Banner, s: &str, key: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let (fill, line, ink) = match kind {
        Banner::Ok => (egui::Color32::TRANSPARENT, egui::Color32::TRANSPARENT, c(C::OkInk)),
        Banner::Bad => (c(C::BadBox), c(C::BadEdge), c(C::BadInk)),
        Banner::Warn => (c(C::WarnWash), c(C::WarnEdge), c(C::WarnInk)),
        Banner::Note => (c(C::Ground), c(C::Line), c(C::Ink2)),
    };
    let pad = if kind == Banner::Ok { egui::Margin::ZERO } else { egui::Margin::symmetric(14, 10) };
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(if kind == Banner::Ok { 0.0_f32 } else { 1.0_f32 }, line))
        .corner_radius(Radius::Menu.egui())
        .inner_margin(pad)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.allocate_ui_with_layout(vec2(ui.available_width(), 22.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let r = key(ui);
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = tokens::S2;
                    match kind {
                        Banner::Ok => {
                            let (slot, _) = ui.allocate_exact_size(vec2(16.0, 16.0), egui::Sense::hover());
                            icons::glyph_at(ui.painter(), Glyph::Ok, slot.center(), 16.0, ink);
                        }
                        Banner::Bad => {
                            mark::mark(ui, Mark::Bad);
                        }
                        Banner::Warn => {
                            mark::mark(ui, Mark::Warn);
                        }
                        Banner::Note => {}
                    }
                    let font = if kind == Banner::Note { Type::Body.font() } else { egui::FontId::new(15.0, crate::fonts::strong()) };
                    let room = ui.available_width();
                    let fit = crate::width::elide_to(ui, s, font.clone(), room);
                    ui.label(egui::RichText::new(fit).font(font).color(ink));
                });
                r
            })
            .inner
        })
        .inner
}

/// A hint under a field: 13/18, second ink (red when it says the field is wrong), 8 above.
pub fn hint(ui: &mut egui::Ui, s: &str) {
    hint_ex(ui, s, false)
}

pub fn hint_ex(ui: &mut egui::Ui, s: &str, bad: bool) {
    if s.is_empty() {
        return;
    }
    ui.add_space((tokens::S2 - ui.spacing().item_spacing.y).max(0.0));
    paint::text(ui, s, Type::Small, if bad { c(C::BadInk) } else { c(C::Ink2) });
}

/// One reading line: a mark and 13 words, 10 apart.
pub fn okline(ui: &mut egui::Ui, m: Mark, s: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 10.0;
        mark::mark(ui, m);
        paint::text(ui, s, Type::Small, c(C::Ink));
    });
}

/// A stack of check rows: a mark, a name, and an optional quiet figure at the right.
pub fn checks(ui: &mut egui::Ui, rows: &[(Mark, String, Option<String>)], two_across: bool) {
    let w = ui.available_width();
    let across = if two_across && w > 360.0 { 2 } else { 1 };
    let cw = (w - 16.0 * (across as f32 - 1.0)) / across as f32;
    let row_h = 26.0;
    let lines = rows.len().div_ceil(across);
    let (rect, _) = ui.allocate_exact_size(vec2(w, lines as f32 * row_h), egui::Sense::hover());
    let p = ui.painter();
    for (i, (m, name, tail)) in rows.iter().enumerate() {
        let x = rect.left() + (cw + 16.0) * (i % across) as f32;
        let y = rect.top() + row_h * (i / across) as f32 + row_h / 2.0;
        mark::paint_mark(ui.ctx(), p, pos2(x + 9.0, y), *m, tokens::MARK);
        let tail_w = tail.as_ref().map(|t| p.layout_no_wrap(t.clone(), Type::MonoSmall.font(), c(C::Ink3)).size().x + 12.0).unwrap_or(0.0);
        paint::at(p, ui, pos2(x + 28.0, y), egui::Align2::LEFT_CENTER, name, Type::Small, c(C::Ink), (cw - 28.0 - tail_w).max(0.0));
        if let Some(t) = tail {
            p.text(pos2(x + cw - 12.0, y), egui::Align2::RIGHT_CENTER, t, Type::MonoSmall.font(), c(C::Ink3));
        }
    }
}

/// The done state: a drawn check, the circle first (520 ms) and then the tick (300 ms, 420 ms in), over
/// "done" and an optional line.
pub fn done_state(ui: &mut egui::Ui, id_salt: &str, done: &str, sub: &str) {
    let id = ui.id().with(("zikaron-done", id_salt));
    let a = motion::age(ui.ctx(), id, 0);
    let circle = ((a / 0.52).clamp(0.0, 1.0), a < 0.52);
    let tick = (((a - 0.42) / 0.3).clamp(0.0, 1.0), a < 0.72);
    if circle.1 || tick.1 {
        ui.ctx().request_repaint();
    }
    let w = ui.available_width();
    let h = 14.0 + 56.0 + 8.0 + Type::Card.line() + if sub.is_empty() { 0.0 } else { 8.0 + Type::Note.line() } + 6.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
    let p = ui.painter();
    let center = pos2(rect.center().x, rect.top() + 14.0 + 28.0);
    let k = 56.0 / 56.0;
    let e = crate::motion::Curve::Ease;
    let pc = e.at(circle.0);
    if pc > 0.0 {
        let n = 48usize;
        let pts: Vec<egui::Pos2> = (0..=((n as f32 * pc) as usize).max(1))
            .map(|i| {
                let ang = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * (i as f32 / n as f32);
                pos2(center.x + 26.0 * k * ang.cos(), center.y + 26.0 * k * ang.sin())
            })
            .collect();
        p.add(egui::Shape::line(pts, egui::Stroke::new(2.0_f32, c(C::Ok))));
    }
    let pt = e.at(tick.0);
    if pt > 0.0 {
        let a0 = pos2(center.x - 11.0, center.y + 1.0);
        let a1 = pos2(center.x - 3.5, center.y + 8.5);
        let a2 = pos2(center.x + 11.0, center.y - 7.0);
        let l1 = (a1 - a0).length();
        let l2 = (a2 - a1).length();
        let along = (l1 + l2) * pt;
        let mut pts = vec![a0];
        if along <= l1 {
            pts.push(a0 + (a1 - a0) * (along / l1));
        } else {
            pts.push(a1);
            pts.push(a1 + (a2 - a1) * ((along - l1) / l2));
        }
        p.add(egui::Shape::line(pts, egui::Stroke::new(2.6_f32, c(C::Ok))));
    }
    let ty = rect.top() + 14.0 + 56.0 + 8.0;
    p.text(pos2(rect.center().x, ty), egui::Align2::CENTER_TOP, done, Type::Card.font(), c(C::Ink));
    if !sub.is_empty() {
        p.text(pos2(rect.center().x, ty + Type::Card.line() + 8.0), egui::Align2::CENTER_TOP, sub, Type::Note.font(), c(C::Ink2));
    }
}

/// One answer of a result box: a bold title, an optional detail line, and a pill at the right.
pub struct Answer<'a> {
    pub title: &'a str,
    pub detail: &'a str,
    pub pill: &'a str,
    pub tone: palette::Tone,
}

/// A result box: answers one under another in a thin frame, their pills popping in one after another.
pub fn answers(ui: &mut egui::Ui, rows: &[Answer]) {
    egui::Frame::new()
        .stroke(egui::Stroke::new(1.0_f32, c(C::Line)))
        .corner_radius(Radius::Menu.egui())
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            for (k, a) in rows.iter().enumerate() {
                if k > 0 {
                    let x = ui.max_rect().x_range();
                    let y = ui.cursor().min.y;
                    ui.painter().hline(x, y, egui::Stroke::new(1.0_f32, c(C::Line2)));
                }
                egui::Frame::new().inner_margin(egui::Margin::symmetric(14, 12)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    crate::width::then(
                        ui,
                        |ui| mark::pill_pop(ui, a.pill, a.tone, 0.1 + 0.07 * k as f32),
                        |ui, room| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                paint::line(ui, a.title, Type::Strong, c(C::Ink), room);
                                if !a.detail.is_empty() {
                                    paint::line(ui, a.detail, Type::Small, c(C::Ink2), room);
                                }
                            });
                        },
                    );
                });
            }
        });
}
