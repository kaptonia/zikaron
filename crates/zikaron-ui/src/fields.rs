//! Fields with their names above them: a form card (22 × 24 inside, fields 20 apart, its keys at the foot on the
//! right) and the plain stack a sheet lays its fields in (14 apart). A field is its name (14, medium; an optional
//! word after it, quiet), 8 below it the control as wide as the column, and under the control a line that says
//! what is wrong (red) or what holds now.
//!
//! The controls fields take: the joined input (38 high, corner 10, a flat tone; an icon before the words, small
//! keys inside its right end after a hairline), and the small input a row takes at its right (34 high, corner 9,
//! figures to the right).

use crate::icons::{self, Glyph};
use crate::palette::{c, C};
use crate::tokens::{self, Type};
use egui::{pos2, vec2, Color32, Rect};

/// The flat tone of a field at rest, under the pointer and pressed.
fn tone(step: u8) -> Color32 {
    c(match step {
        0 => C::Tone,
        1 => C::ToneHover,
        _ => C::TonePress,
    })
}

/// A form card: fields 20 apart inside 22 × 24, on the card ground.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui, &mut Stack) -> R) -> R {
    // Rows laid in it light within this card, by its corner.
    let id = crate::sheet::rows_corner_id();
    let was = ui.ctx().data(|d| d.get_temp::<f32>(id));
    ui.ctx().data_mut(|d| d.insert_temp(id, crate::tokens::Radius::Card.px()));
    let r = crate::card::card_pad(ui, vec2(24.0, 22.0), |ui| stack_gap(ui, 20.0, add));
    ui.ctx().data_mut(|d| {
        if let Some(c) = was {
            d.insert_temp(id, c);
        } else {
            d.remove::<f32>(id);
        }
    });
    r
}

/// A sheet's fields: 14 apart, on the sheet.
pub fn stack<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui, &mut Stack) -> R) -> R {
    stack_gap(ui, 14.0, add)
}

fn stack_gap<R>(ui: &mut egui::Ui, gap: f32, add: impl FnOnce(&mut egui::Ui, &mut Stack) -> R) -> R {
    // What rows ending in a fold noted before this stack is not this stack's ([`Stack::foot`] reads its own).
    ui.ctx().data_mut(|d| d.remove::<(f32, f32)>(crate::sheet::trailing()));
    ui.vertical(|ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = gap;
        let mut s = Stack { gap };
        add(ui, &mut s)
    })
    .inner
}

/// Two columns laid in a region made here, `lw` and `rw` wide and 14 apart, as tall as the taller.
fn side_by_side<A, B>(ui: &mut egui::Ui, gap: f32, lw: f32, rw: f32, left: impl FnOnce(&mut egui::Ui, &mut Stack) -> A, right: impl FnOnce(&mut egui::Ui, &mut Stack) -> B) -> (A, B) {
    let at = ui.cursor().min;
    let column = |ui: &mut egui::Ui, x: f32, w: f32, salt: &str| {
        let mut c = ui.new_child(egui::UiBuilder::new().id_salt(salt).max_rect(Rect::from_min_size(pos2(x, at.y), vec2(w, f32::INFINITY))).layout(egui::Layout::top_down(egui::Align::Min)));
        c.set_width(w);
        c
    };
    let mut lc = column(ui, at.x, lw, "zikaron-pair-l");
    let a = stack_gap(&mut lc, gap, left);
    let mut rc = column(ui, at.x + lw + 14.0, rw, "zikaron-pair-r");
    let b = stack_gap(&mut rc, gap, right);
    let h = lc.min_rect().height().max(rc.min_rect().height());
    ui.allocate_exact_size(vec2(lw + 14.0 + rw, h), egui::Sense::hover());
    (a, b)
}

/// The fields of a form or sheet being laid.
pub struct Stack {
    gap: f32,
}

impl Stack {
    /// A field: its name (with `optional`'s word after it when the field may stay empty), the control `add` draws
    /// (given its width), and under it `line` (its words, and whether they say something is wrong). A name that
    /// says again the title of the sheet it is in is not drawn, nor the room it takes (`sheet::echoes_title`).
    pub fn field<R>(&mut self, ui: &mut egui::Ui, label: &str, optional: Option<&str>, line: Option<(&str, bool)>, add: impl FnOnce(&mut egui::Ui, f32) -> R) -> R {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            if !crate::sheet::echoes_title(ui.ctx(), label) {
                name(ui, label, optional);
                ui.add_space(name_gap() - 8.0);
            }
            let w = ui.available_width();
            let r = add(ui, w);
            if let Some((s, bad)) = line.filter(|(s, _)| !s.is_empty()) {
                ui.add_space(-4.0);
                crate::paint::line(ui, s, Type::Small, if bad { c(C::BadInk) } else { c(C::Ink2) }, w);
            }
            r
        })
        .inner
    }

    /// Two fields side by side, 14 apart (the chain id and the start block).
    pub fn pair<A, B>(&mut self, ui: &mut egui::Ui, left: impl FnOnce(&mut egui::Ui, &mut Stack) -> A, right: impl FnOnce(&mut egui::Ui, &mut Stack) -> B) -> (A, B) {
        let cw = ((ui.available_width() - 14.0) / 2.0).max(0.0);
        side_by_side(ui, self.gap, cw, cw, left, right)
    }

    /// Two fields side by side, 14 apart, the right one `right_w` wide (the proxy's host and port).
    pub fn pair_w<A, B>(&mut self, ui: &mut egui::Ui, right_w: f32, left: impl FnOnce(&mut egui::Ui, &mut Stack) -> A, right: impl FnOnce(&mut egui::Ui, &mut Stack) -> B) -> (A, B) {
        let lw = (ui.available_width() - 14.0 - right_w).max(0.0);
        side_by_side(ui, self.gap, lw, right_w, left, right)
    }

    /// Lines under the fields (what is wrong, what holds): a mark and words each, 6 apart.
    pub fn says(&mut self, ui: &mut egui::Ui, lines: &[(crate::mark::Mark, &str)]) {
        if lines.is_empty() {
            return;
        }
        let w = ui.available_width();
        for (k, (m, s)) in lines.iter().enumerate() {
            if k > 0 {
                ui.add_space(6.0);
            }
            let ink = if *m == crate::mark::Mark::Bad { c(C::BadInk) } else { c(C::Ink2) };
            let galley = ui.painter().layout(s.to_string(), Type::Small.font(), ink, (w - 24.0).max(1.0));
            let h = galley.size().y.max(Type::Small.line());
            let (r, _) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
            crate::mark::paint_mark(ui.ctx(), ui.painter(), pos2(r.left() + 8.0, r.top() + Type::Small.line() / 2.0), *m, 16.0);
            ui.painter().galley(pos2(r.left() + 24.0, r.top() + (Type::Small.line() - Type::Small.size() * 1.25).max(0.0) / 2.0), galley, ink);
        }
    }

    /// The form's keys at its foot, on the right, 8 apart. Rows ending in a fold just above the keys stand off them
    /// as a field does: the blank under the fold's words comes off the room above the keys (the rows note where they
    /// end and that blank as they end, `card::form`; a sheet's keys take it the same way, `sheet::show`).
    pub fn foot<R>(&mut self, ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ends = ui.ctx().data_mut(|d| d.remove_temp::<(f32, f32)>(crate::sheet::trailing()));
        let top = ui.cursor().top();
        let trim = ends.filter(|(bottom, _)| (bottom + self.gap - top).abs() < crate::scroll::SLACK).map(|(_, tail)| tail.round()).unwrap_or(0.0);
        ui.add_space(4.0 - self.gap.min(4.0) - trim);
        ui.allocate_ui_with_layout(vec2(ui.available_width(), tokens::KEY_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = tokens::S2;
            add(ui)
        })
        .inner
    }
}

/// From a field's name to the control under it, the one measure for every name over its control: what the
/// credential sheet's validity row leaves between its name (centred on a key's height, the switch beside it) and
/// the box under it.
pub fn name_gap() -> f32 {
    (tokens::KEY_H - Type::Note.line()) / 2.0 + tokens::S2
}

/// A field's name: 14 medium, and the quiet word after it.
pub fn name(ui: &mut egui::Ui, label: &str, optional: Option<&str>) {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), Type::Note.line()), egui::Sense::hover());
    let p = ui.painter();
    let named = p.text(pos2(r.left(), r.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::new(14.0, crate::fonts::medium()), c(C::Ink));
    if let Some(o) = optional {
        p.text(pos2(named.right() + 8.0, r.center().y), egui::Align2::LEFT_CENTER, o, Type::Small.font(), c(C::Ink3));
    }
}

/// How the joined input's words are set.
#[derive(Clone, Copy, Debug, Default)]
pub struct Look {
    pub mono: bool,
    pub bad: bool,
    pub icon: Option<Glyph>,
    pub read_only: bool,
}

/// The joined input: a 38-high tone field as wide as `w`, its words (or `hint`), an icon before them when given,
/// and what `inner` draws at its right end after a hairline (small keys: [`inner_key`], [`inner_face`]). Focused,
/// it turns to the surface with the accent's ring. Returns the words' response and `inner`'s answer.
pub fn joined<R>(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32, look: Look, inner: Option<impl FnOnce(&mut egui::Ui) -> R>) -> (egui::Response, Option<R>) {
    let (rect, _) = ui.allocate_exact_size(vec2(w, tokens::INPUT_H), egui::Sense::hover());
    let slot = ui.painter().add(egui::Shape::Noop);
    let mut right = rect.right();
    let mut answer = None;
    if let Some(f) = inner {
        let room = Rect::from_min_max(pos2(rect.left() + 38.0, rect.top()), pos2(rect.right() - 4.0, rect.bottom()));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(room).layout(egui::Layout::right_to_left(egui::Align::Center)));
        child.spacing_mut().item_spacing.x = 4.0;
        answer = Some(f(&mut child));
        let used = child.min_rect();
        if used.width() > 1.0 {
            let sx = used.left() - 4.0;
            ui.painter().vline(sx, (rect.top() + 8.0)..=(rect.bottom() - 8.0), egui::Stroke::new(1.0_f32, c(C::Line)));
            right = sx;
        }
    }
    let mut x = rect.left();
    if let Some(g) = look.icon {
        icons::glyph_at(ui.painter(), g, pos2(x + 19.0, rect.center().y), 17.0, c(C::Ink2));
        x += 38.0;
    }
    let font = if look.mono { Type::MonoSmall.font() } else { egui::FontId::new(14.0, egui::FontFamily::Proportional) };
    let edit_rect = Rect::from_min_max(pos2(x + if look.icon.is_some() { 0.0 } else { 12.0 }, rect.top()), pos2(right - 8.0, rect.bottom()));
    // Named as every field is (`input::field_id`), so it keeps its focus while what is above it comes and goes.
    let id = crate::input::field_id(ui, "zikaron-joined", hint);
    let resp = ui
        .scope_builder(egui::UiBuilder::new().max_rect(edit_rect).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
            crate::compose::around(ui, id, text, |ui, text| {
                ui.add(
                    egui::TextEdit::singleline(text)
                        .id(id)
                        .frame(egui::Frame::NONE)
                        .font(font.clone())
                        .text_color(c(C::Ink))
                        .hint_text(egui::RichText::new(hint).font(font.clone()).color(c(C::Ink3)))
                        .desired_width(edit_rect.width())
                        .interactive(!look.read_only),
                )
            })
        })
        .inner;
    let hot = resp.hovered();
    let focus = resp.has_focus();
    // Red wins over blue, as on every field (`input::frame_shape`): a wrong value is said while it is typed.
    let shape: egui::Shape = if focus && !look.bad {
        egui::Shape::Vec(vec![
            egui::epaint::RectShape::filled(rect.expand(3.0), egui::CornerRadius::same(13), c(C::Focus)).into(),
            egui::epaint::RectShape::new(rect, egui::CornerRadius::same(10), c(C::Surface), egui::Stroke::new(1.5_f32, c(C::Accent).gamma_multiply(0.7)), egui::StrokeKind::Inside).into(),
        ])
    } else if look.bad {
        egui::epaint::RectShape::new(rect, egui::CornerRadius::same(10), tone(0), egui::Stroke::new(1.5_f32, c(C::Bad)), egui::StrokeKind::Inside).into()
    } else {
        egui::epaint::RectShape::filled(rect, egui::CornerRadius::same(10), tone(if hot { 1 } else { 0 })).into()
    };
    ui.painter().set(slot, shape);
    (resp, answer)
}

/// A control greyed and taking nothing while `off` (the field beside it was filled instead): drawn as it is, faint
/// (`tokens::OFF`), in a region of its own that takes no input; as it is otherwise. The one place a field is greyed
/// so (the cells box and the span box grey themselves the same way).
pub fn off<R>(ui: &mut egui::Ui, off: bool, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    ui.scope(|ui| {
        if off {
            ui.disable();
            ui.multiply_opacity(tokens::OFF);
        }
        add(ui)
    })
    .inner
}

/// The joined input with nothing inside its right end: returns the words' response.
pub fn line(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32, look: Look) -> egui::Response {
    joined::<()>(ui, text, hint, w, look, None::<fn(&mut egui::Ui)>).0
}

/// A small key inside a joined input: 30 high, corner 8, no face until the pointer is on it.
pub fn inner_key(ui: &mut egui::Ui, label: &str, glyph: Option<Glyph>) -> egui::Response {
    inner(ui, label, glyph, false)
}

/// A small key inside a joined input with a key's face and edge ("change").
pub fn inner_face(ui: &mut egui::Ui, label: &str) -> egui::Response {
    inner(ui, label, None, true)
}

fn inner(ui: &mut egui::Ui, label: &str, glyph: Option<Glyph>, face: bool) -> egui::Response {
    let font = Type::InnerKey.font();
    let g = ui.painter().layout_no_wrap(label.to_string(), font.clone(), c(C::Ink));
    let w = 12.0 + g.size().x + glyph.map(|_| 6.0 + 13.0).unwrap_or(0.0) + 12.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, 30.0), egui::Sense::click());
    let p = ui.painter();
    if face {
        p.rect(rect, egui::CornerRadius::same(8), c(C::KeyFace), egui::Stroke::new(0.5_f32, c(C::KeyEdge)), egui::StrokeKind::Outside);
    } else if resp.hovered() {
        p.rect_filled(rect, egui::CornerRadius::same(8), tone(0));
    }
    p.galley(pos2(rect.left() + 12.0, rect.center().y - g.size().y / 2.0), g, c(C::Ink));
    if let Some(gl) = glyph {
        icons::glyph_at(p, gl, pos2(rect.right() - 12.0 - 6.5, rect.center().y), 13.0, c(C::Ink2));
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The small input a row takes at its right end (an interval, a limit): `w` wide, 34 high, corner 9, a flat
/// tone, monospace figures to the right; `hint` (quiet) when empty.
pub fn small(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32, bad: bool) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(vec2(w, tokens::KEY_H), egui::Sense::hover());
    let slot = ui.painter().add(egui::Shape::Noop);
    let font = Type::MonoSmall.font();
    let inner = rect.shrink2(vec2(10.0, 0.0));
    let id = crate::input::field_id(ui, "zikaron-small", hint);
    let resp = ui
        .scope_builder(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| {
            crate::compose::around(ui, id, text, |ui, text| {
                ui.add(
                    egui::TextEdit::singleline(text)
                        .id(id)
                        .frame(egui::Frame::NONE)
                        .font(font.clone())
                        .text_color(c(C::Ink))
                        .horizontal_align(egui::Align::Max)
                        .hint_text(egui::RichText::new(hint).font(Type::Small.font()).color(c(C::Ink3)))
                        .desired_width(inner.width()),
                )
            })
        })
        .inner;
    let shape: egui::Shape = if resp.has_focus() && !bad {
        egui::Shape::Vec(vec![
            egui::epaint::RectShape::filled(rect.expand(3.0), egui::CornerRadius::same(12), c(C::Focus)).into(),
            egui::epaint::RectShape::new(rect, egui::CornerRadius::same(9), c(C::Surface), egui::Stroke::new(1.5_f32, c(C::Accent).gamma_multiply(0.7)), egui::StrokeKind::Inside).into(),
        ])
    } else if bad {
        egui::epaint::RectShape::new(rect, egui::CornerRadius::same(9), tone(0), egui::Stroke::new(1.5_f32, c(C::Bad)), egui::StrokeKind::Inside).into()
    } else {
        egui::epaint::RectShape::filled(rect, egui::CornerRadius::same(9), tone(if resp.hovered() { 1 } else { 0 })).into()
    };
    ui.painter().set(slot, shape);
    resp
}

/// The flat tone of a field (for a page's own wells: the words grid, the date tiles).
pub fn field_tone() -> Color32 {
    tone(0)
}

/// The rows under a column of fields (a form card's or a sheet's): no ground of their own, 48 high, their words
/// level with the fields' left edge. Returns what `add` answers.
pub fn rows<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui, &mut crate::card::Form) -> R) -> R {
    let flag = crate::sheet::inside_id();
    let was = crate::sheet::inside(ui.ctx());
    ui.ctx().data_mut(|d| d.insert_temp(flag, true));
    let r = crate::card::form(ui, add);
    ui.ctx().data_mut(|d| d.insert_temp(flag, was));
    r
}
