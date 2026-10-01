//! Drop zones and file rows. A drop zone is a dashed, lightly tinted block that turns blue while a file hovers
//! or the pointer is over it; the whole block is clickable (the caller opens the system picker). Chosen, it
//! turns into a green-edged block naming the file. The drag veil covers the whole window while a file is
//! dragged over a page that takes drops as a whole.

use crate::icons::{self, Glyph};
use crate::motion::{self, Curve};
use crate::paint;
use crate::palette::{self, c, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

/// What a drop zone received this frame.
#[derive(Default, Debug, Clone)]
pub struct Drop {
    /// Paths dropped on it this frame.
    pub dropped: Vec<String>,
    /// Something is being dragged over the window.
    pub hovering: bool,
    /// The zone was clicked (the caller opens the system picker).
    pub clicked: bool,
}

/// Whether files are being dragged over the window now.
pub fn dragging(ctx: &egui::Context) -> bool {
    ctx.input(|i| !i.raw.hovered_files.is_empty())
}

/// Take the files dropped this frame (the first taker gets them).
pub fn take_dropped(ctx: &egui::Context) -> Vec<String> {
    ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files)).into_iter().filter_map(|f| f.path.map(|p| p.display().to_string())).collect()
}

/// The zones that took drops in the last pass and in this one, in drawing order.
#[derive(Clone, Default)]
struct Zones {
    pass: u64,
    this: Vec<(egui::Id, Rect)>,
    last: Vec<(egui::Id, Rect)>,
}

fn zones_id() -> egui::Id {
    egui::Id::new("zikaron-drop-zones")
}

fn zones_at(ctx: &egui::Context, pass: u64) -> Zones {
    let z = ctx.data(|d| d.get_temp::<Zones>(zones_id())).unwrap_or_default();
    if z.pass == pass {
        z
    } else if z.pass + 1 == pass {
        Zones { pass, this: Vec::new(), last: z.this }
    } else {
        Zones { pass, ..Default::default() }
    }
}

/// One frame has one receiver: of the zones that took drops last pass, the one under the pointer, else the
/// first drawn (the window does not say where a file lands, so the pointer is known only sometimes). Only the
/// receiver lights up while a file is dragged, and only it takes the file: the lit zone is the one that gets
/// it.
fn is_receiver(ctx: &egui::Context, me: egui::Id) -> bool {
    let pass = ctx.cumulative_pass_nr();
    let pointer = ctx.input(|i| i.pointer.latest_pos());
    let z = zones_at(ctx, pass);
    if z.last.iter().any(|(id, _)| *id == me) {
        let under = z.last.iter().find(|(_, r)| pointer.is_some_and(|p| r.contains(p)));
        under.or(z.last.first()).map(|(id, _)| *id) == Some(me)
    } else {
        z.last.is_empty() && z.this.is_empty()
    }
}

fn register(ctx: &egui::Context, me: egui::Id, rect: Rect) {
    let pass = ctx.cumulative_pass_nr();
    let mut z = zones_at(ctx, pass);
    z.this.push((me, rect));
    ctx.data_mut(|d| d.insert_temp(zones_id(), z));
}

/// A dashed rounded outline (dashes are drawn twice the stroke long and one stroke apart by the callers).
pub fn dashed(p: &egui::Painter, rect: Rect, radius: f32, stroke: egui::Stroke, dash: f32, gap: f32) {
    // Walk the outline: straight edges and quarter arcs, cut into dashes along its length.
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let mut pts: Vec<egui::Pos2> = Vec::new();
    let arc = |c: egui::Pos2, from: f32, pts: &mut Vec<egui::Pos2>| {
        for k in 0..=8 {
            let a = (from + 90.0 * k as f32 / 8.0).to_radians();
            pts.push(pos2(c.x + r * a.cos(), c.y + r * a.sin()));
        }
    };
    pts.push(pos2(rect.left() + r, rect.top()));
    pts.push(pos2(rect.right() - r, rect.top()));
    arc(pos2(rect.right() - r, rect.top() + r), 270.0, &mut pts);
    pts.push(pos2(rect.right(), rect.bottom() - r));
    arc(pos2(rect.right() - r, rect.bottom() - r), 0.0, &mut pts);
    pts.push(pos2(rect.left() + r, rect.bottom()));
    arc(pos2(rect.left() + r, rect.bottom() - r), 90.0, &mut pts);
    pts.push(pos2(rect.left(), rect.top() + r));
    arc(pos2(rect.left() + r, rect.top() + r), 180.0, &mut pts);
    p.extend(egui::Shape::dashed_line(&pts, stroke, dash, gap));
}

/// How a drop zone lays out its words.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// Glyph over bold words over a note, centered.
    Column,
    /// Glyph left of bold words over a note (the compact zone on the records page).
    Row,
    /// Only the words, one line, centered (small slots).
    Line,
}

/// A drop zone. `glyph` goes above (or before) the words; `lines` are the bold title and the note; `chosen`
/// names the file once chosen (with `change` under it). `accept` false draws without taking drops; of the
/// zones that accept, one per frame receives (see `is_receiver`).
#[allow(clippy::too_many_arguments)]
pub fn zone(ui: &mut egui::Ui, id_salt: &str, glyph: Option<Glyph>, lines: &[&str], chosen: Option<(&str, &str, &str)>, height: f32, shape: Shape, accept: bool) -> Drop {
    // Under an open sheet only the sheet's own zones take drops.
    let in_sheet = ui.layer_id().order == egui::Order::Foreground;
    let accept = accept && (in_sheet || !crate::sheet::up(ui.ctx()));
    let me = ui.id().with(("zikaron-drop", id_salt));
    let receiver = accept && is_receiver(ui.ctx(), me);
    let hovering = receiver && dragging(ui.ctx());
    let dropped = if receiver { take_dropped(ui.ctx()) } else { Vec::new() };
    let w = ui.available_width();
    let height = height.max(crate::grid::cell_height(ui));
    let (rect, resp) = ui.allocate_exact_size(vec2(w, height), egui::Sense::click());
    if accept {
        register(ui.ctx(), me, rect);
    }
    let hot = motion::flag(ui.ctx(), resp.id.with("hot"), hovering || resp.hovered(), tokens::FAST);
    let press = motion::to(ui.ctx(), resp.id.with("press"), if resp.is_pointer_button_down_on() { 0.995 } else { 1.0 }, tokens::FAST, Curve::Ease);
    let r = paint::scaled(rect, press);
    let p = ui.painter();
    match chosen {
        Some((name, size, change)) => {
            p.rect(r, Radius::Menu.egui(), c(C::OkBox), egui::Stroke::new(1.0_f32, c(C::OkEdge)), egui::StrokeKind::Inside);
            let heavy = egui::FontId::new(15.0, crate::fonts::strong());
            let room = r.width() - 32.0;
            let total = 22.0 + if size.is_empty() { 0.0 } else { 4.0 + 18.0 } + if change.is_empty() { 0.0 } else { 6.0 + 18.0 };
            let mut y = r.center().y - total / 2.0;
            let fit = crate::width::elide_to(ui, name, heavy.clone(), room);
            p.text(pos2(r.left() + 16.0, y), egui::Align2::LEFT_TOP, fit, heavy, c(C::Ink));
            y += 22.0;
            if !size.is_empty() {
                y += 4.0;
                p.text(pos2(r.left() + 16.0, y), egui::Align2::LEFT_TOP, size, Type::Small.font(), c(C::Ink2));
                y += 18.0;
            }
            if !change.is_empty() {
                y += 6.0;
                p.text(pos2(r.left() + 16.0, y), egui::Align2::LEFT_TOP, change, Type::Small.font(), c(C::Ink3));
            }
        }
        None => {
            p.rect_filled(r, Radius::Menu.egui(), palette::mix(c(C::Drop), c(C::DropHot), hot));
            dashed(p, r.shrink(0.75), 10.0, egui::Stroke::new(1.5_f32, palette::mix(c(C::Dash), c(C::Accent), hot)), 3.0, 1.5);
            let title = lines.first().copied().unwrap_or("");
            let note = lines.get(1).copied().unwrap_or("");
            let heavy = egui::FontId::new(15.0, crate::fonts::strong());
            match shape {
                Shape::Column => {
                    let gh = if glyph.is_some() { 22.0 + 4.0 } else { 0.0 };
                    let total = gh + 22.0 + if note.is_empty() { 0.0 } else { 2.0 + 20.0 };
                    let mut y = r.center().y - total / 2.0;
                    if let Some(g) = glyph {
                        icons::glyph_at(p, g, pos2(r.center().x, y + 11.0), 22.0, c(C::Ink3));
                        y += gh;
                    }
                    p.text(pos2(r.center().x, y + 11.0), egui::Align2::CENTER_CENTER, crate::width::elide_to(ui, title, heavy.clone(), r.width() - 24.0), heavy, c(C::Ink));
                    if !note.is_empty() {
                        p.text(pos2(r.center().x, y + 22.0 + 2.0 + 10.0), egui::Align2::CENTER_CENTER, crate::width::elide_to(ui, note, Type::Note.font(), r.width() - 24.0), Type::Note.font(), c(C::Ink2));
                    }
                }
                Shape::Row => {
                    let gw = if glyph.is_some() { 22.0 + 14.0 } else { 0.0 };
                    let tw = ui.fonts(|f| f.layout_no_wrap(title.to_string(), heavy.clone(), egui::Color32::BLACK).size().x.max(f.layout_no_wrap(note.to_string(), Type::Note.font(), egui::Color32::BLACK).size().x));
                    let x0 = r.center().x - (gw + tw) / 2.0;
                    if let Some(g) = glyph {
                        icons::glyph_at(p, g, pos2(x0 + 11.0, r.center().y), 22.0, c(C::Ink3));
                    }
                    p.text(pos2(x0 + gw, r.center().y - 1.0), egui::Align2::LEFT_BOTTOM, title, heavy, c(C::Ink));
                    p.text(pos2(x0 + gw, r.center().y + 1.0), egui::Align2::LEFT_TOP, note, Type::Note.font(), c(C::Ink2));
                }
                Shape::Line => {
                    p.text(r.center(), egui::Align2::CENTER_CENTER, crate::width::elide_to(ui, title, Type::Note.font(), r.width() - 24.0), Type::Note.font(), c(C::Ink2));
                }
            }
        }
    }
    let clicked = resp.clicked();
    let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
    Drop { dropped, hovering, clicked }
}

/// A file row: a small file tile, the name (and a second line, red when it says why the file is out), and
/// a key at the right. Returns whether the key was pressed.
pub fn file_row(ui: &mut egui::Ui, id_salt: &str, name: &str, sub: &str, bad: bool, key: &str) -> bool {
    let id = ui.id().with(("zikaron-file-row", id_salt));
    let enter = motion::enter(ui.ctx(), id, motion::key_of(&name), 0.0, tokens::MID, Curve::Ease);
    let mut hit = false;
    motion::shifted(ui, vec2(0.0, 6.0 * (1.0 - enter)), enter, |ui| {
        let w = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(vec2(w, 52.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect(rect, Radius::Menu.egui(), c(C::Surface), egui::Stroke::new(1.0_f32, c(C::Line)), egui::StrokeKind::Inside);
        let ic = Rect::from_min_size(pos2(rect.left() + 12.0, rect.center().y - 15.0), vec2(30.0, 30.0));
        p.rect_filled(ic, egui::CornerRadius::same(7), c(C::AccentWash));
        icons::glyph_at(p, Glyph::File, ic.center(), 16.0, c(C::AccentInk));
        let key_rect = if key.is_empty() {
            None
        } else {
            let kw = crate::button::size_of(ui, &crate::button::Key::new(key, crate::button::Role::Secondary)).x;
            Some(Rect::from_min_size(pos2(rect.right() - 8.0 - kw, rect.center().y - tokens::KEY_H / 2.0), vec2(kw, tokens::KEY_H)))
        };
        let room = key_rect.map(|k| k.left() - 12.0).unwrap_or(rect.right() - 12.0) - (ic.right() + 12.0);
        let tx = ic.right() + 12.0;
        if sub.is_empty() {
            paint::at(p, ui, pos2(tx, rect.center().y), egui::Align2::LEFT_CENTER, name, Type::Body, c(C::Ink), room);
        } else {
            paint::at(p, ui, pos2(tx, rect.center().y - 1.0), egui::Align2::LEFT_BOTTOM, name, Type::Body, c(C::Ink), room);
            paint::at(p, ui, pos2(tx, rect.center().y + 1.0), egui::Align2::LEFT_TOP, sub, Type::Small, if bad { c(C::BadInk) } else { c(C::Ink3) }, room);
        }
        if let Some(kr) = key_rect {
            let resp = ui.interact(kr, id.with("key"), egui::Sense::click());
            crate::button::paint(ui, kr, &resp, &crate::button::Key::new(key, crate::button::Role::Secondary), crate::button::Phase::Idle);
            hit = resp.clicked();
        }
    });
    hit
}

/// The drag veil: a dimming layer over the whole window with a dashed blue frame 32 in, a glyph, a bold line
/// and a note. Fades in over 120 ms; the frame settles from 0.98 over 200 ms.
pub fn veil(ctx: &egui::Context, on: bool, title: &str, note: &str) {
    let id = egui::Id::new("zikaron-veil");
    let a = motion::flag(ctx, id.with("a"), on, tokens::FAST);
    if a <= 0.0 {
        return;
    }
    let s = motion::to(ctx, id.with("s"), if on { 1.0 } else { 0.98 }, tokens::MID, Curve::Ease);
    let screen = ctx.screen_rect();
    let layer = egui::LayerId::new(egui::Order::Middle, id);
    let p = ctx.layer_painter(layer);
    p.rect_filled(screen, 0.0, c(C::Veil).gamma_multiply(a));
    let frame = paint::scaled(screen.shrink(tokens::S6), s);
    dashed(&p, frame, 18.0, egui::Stroke::new(2.0_f32, c(C::Accent).gamma_multiply(a)), 4.0, 2.0);
    let cy = frame.center().y;
    icons::glyph_at(&p, Glyph::Inbox, pos2(frame.center().x, cy - 30.0), 28.0, c(C::AccentInk).gamma_multiply(a));
    p.text(pos2(frame.center().x, cy + 4.0), egui::Align2::CENTER_CENTER, title, egui::FontId::new(17.0, crate::fonts::strong()), c(C::AccentInk).gamma_multiply(a));
    p.text(pos2(frame.center().x, cy + 30.0), egui::Align2::CENTER_CENTER, note, Type::Note.font(), c(C::Ink2).gamma_multiply(a));
}
