//! Inputs: 38 high, 15 text, corner 8, a one-point line; focused, the line turns blue with a soft blue ring.
//! Multi-line areas use the monospace face. Masked fields never show their text.

use crate::icons::{self, Glyph};
use crate::motion;
use crate::palette::{self, c, C};
use crate::tokens::{self, Radius, Type};
use egui::{vec2, Rect};

/// The states of the border. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edge {
    Rest,
    Focus,
    /// The field's text is wrong (the caller decides; this only draws). Red wins over blue: a wrong field
    /// shows while the person types in it.
    Bad,
}

/// A field's frame as one shape: the soft focus ring (eased in and out over 120 ms), the fill and the line.
/// Fields draw their text first, so focus is known, and put this shape into a slot reserved under it.
pub fn frame_shape(ctx: &egui::Context, rect: Rect, id: egui::Id, edge: Edge) -> egui::Shape {
    let focus = motion::flag(ctx, id.with("focus"), edge == Edge::Focus, tokens::FAST);
    let line = match edge {
        Edge::Bad => c(C::Bad),
        _ => palette::mix(c(C::Line), c(C::Accent), focus),
    };
    let mut v: Vec<egui::Shape> = Vec::new();
    if focus > 0.0 {
        v.push(egui::epaint::RectShape::stroke(rect.expand(1.5), egui::CornerRadius::same(10), egui::Stroke::new(3.0_f32, c(C::Focus).gamma_multiply(focus)), egui::StrokeKind::Middle).into());
    }
    v.push(egui::epaint::RectShape::new(rect, Radius::Ctl.egui(), c(C::Surface), egui::Stroke::new(1.0_f32, line), egui::StrokeKind::Inside).into());
    egui::Shape::Vec(v)
}

/// How a single-line field looks.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Look {
    /// Monospace text (addresses, hashes, numbers the person types).
    pub mono: bool,
    /// A search glyph before the text.
    pub search: bool,
    /// Red frame: the text is wrong.
    pub bad: bool,
}

/// Single-line field, full width.
pub fn line(ui: &mut egui::Ui, text: &mut String, hint: &str) -> egui::Response {
    let w = ui.available_width();
    field(ui, text, hint, w, Look::default())
}

/// Single-line field of a given width.
pub fn line_w(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32) -> egui::Response {
    field(ui, text, hint, w, Look::default())
}

/// Single-line monospace field, full width.
pub fn mono(ui: &mut egui::Ui, text: &mut String, hint: &str) -> egui::Response {
    let w = ui.available_width();
    field(ui, text, hint, w, Look { mono: true, ..Default::default() })
}

/// The search field of a list page.
pub fn search(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32) -> egui::Response {
    field(ui, text, hint, w, Look { search: true, ..Default::default() })
}

/// A field's id, stable while the page around it changes: the id of the layout it sits in, what kind of field
/// it is, its hint, and its place among fields with that same kind and hint in that layout this pass. An id
/// counted from the widgets drawn before it would change when a line or a table above it comes or goes, and
/// the field would lose focus in the middle of typing.
fn field_id(ui: &egui::Ui, kind: &str, hint: &str) -> egui::Id {
    let base = ui.id().with((kind, hint));
    let pass = ui.ctx().cumulative_pass_nr();
    let key = egui::Id::new("zikaron-field-places");
    let seen: Option<(u64, Vec<(egui::Id, u32)>)> = ui.ctx().data(|d| d.get_temp(key));
    let mut table = match seen {
        Some((p, v)) if p == pass => v,
        _ => Vec::new(),
    };
    let n = match table.iter_mut().find(|(b, _)| *b == base) {
        Some((_, c)) => {
            *c += 1;
            *c
        }
        None => {
            table.push((base, 0));
            0
        }
    };
    ui.ctx().data_mut(|d| d.insert_temp(key, (pass, table)));
    base.with(n)
}

/// The one single-line field.
pub fn field(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32, look: Look) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(vec2(w, tokens::INPUT_H), egui::Sense::hover());
    let id = field_id(ui, "zikaron-line", hint);
    let slot = ui.painter().add(egui::Shape::Noop);
    let mut inner = rect.shrink2(vec2(tokens::INPUT_PAD_X, 4.0));
    if look.search {
        icons::glyph_at(ui.painter(), Glyph::Search, egui::pos2(inner.left() + 7.0, inner.center().y), 14.0, c(C::Ink3));
        inner.min.x += 14.0 + tokens::S2;
    }
    let t = if look.mono { Type::Mono } else { Type::Body };
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let resp = child.add(
        egui::TextEdit::singleline(text)
            .id(id)
            .font(t.font())
            .text_color(c(C::Ink))
            .hint_text(egui::RichText::new(hint).font(t.font()).color(c(C::Ink3)))
            .frame(false)
            .desired_width(inner.width())
            .vertical_align(egui::Align::Center),
    );
    let edge = if look.bad {
        Edge::Bad
    } else if resp.has_focus() {
        Edge::Focus
    } else {
        Edge::Rest
    };
    ui.painter().set(slot, frame_shape(ui.ctx(), rect, id, edge));
    resp
}

/// Multi-line area: same frame, monospace, 10 × 12 inside.
pub fn area(ui: &mut egui::Ui, text: &mut String, rows: usize) -> egui::Response {
    area_hint(ui, text, rows, "")
}

/// Multi-line area with a hint.
pub fn area_hint(ui: &mut egui::Ui, text: &mut String, rows: usize, hint: &str) -> egui::Response {
    area_ex(ui, text, rows, hint, Type::Mono)
}

/// Multi-line area in body text (notes and scopes a person writes in words).
pub fn area_words(ui: &mut egui::Ui, text: &mut String, rows: usize, hint: &str) -> egui::Response {
    area_ex(ui, text, rows, hint, Type::Note)
}

fn area_ex(ui: &mut egui::Ui, text: &mut String, rows: usize, hint: &str, t: Type) -> egui::Response {
    let id = field_id(ui, "zikaron-area", hint);
    let slot = ui.painter().add(egui::Shape::Noop);
    let shown = egui::Frame::new().inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.add(
            egui::TextEdit::multiline(text)
                .id(id)
                .font(t.font())
                .text_color(c(C::Ink))
                .hint_text(egui::RichText::new(hint).font(t.font()).color(c(C::Ink3)))
                .desired_rows(rows)
                .desired_width(f32::INFINITY)
                .frame(false),
        )
    });
    let edge = if shown.inner.has_focus() { Edge::Focus } else { Edge::Rest };
    ui.painter().set(slot, frame_shape(ui.ctx(), shown.response.rect, id, edge));
    shown.inner
}

/// A password field: same frame, dots instead of text. It stores a secret type, edited in place.
pub fn secret_line(ui: &mut egui::Ui, text: &mut crate::secret::Secret, hint: &str) -> egui::Response {
    let w = ui.available_width();
    secret_line_w_marked(ui, text, hint, w, false)
}

/// The masked field with a given width and the "text is wrong" state (red frame; text still hidden).
///
/// Masking is answered here: the twelve word cells and every passcode and password field get it without the
/// caller writing anything, and there is no key that reveals plain text.
pub fn secret_line_w_marked(ui: &mut egui::Ui, text: &mut crate::secret::Secret, hint: &str, w: f32, bad: bool) -> egui::Response {
    secret_field(ui, text, hint, w, bad, None)
}

/// A masked field with a number before the text (the twelve word cells).
pub fn secret_numbered(ui: &mut egui::Ui, text: &mut crate::secret::Secret, n: usize, w: f32, bad: bool) -> egui::Response {
    secret_field(ui, text, "", w, bad, Some(n))
}

fn secret_field(ui: &mut egui::Ui, text: &mut crate::secret::Secret, hint: &str, w: f32, bad: bool, number: Option<usize>) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(vec2(w, tokens::INPUT_H), egui::Sense::hover());
    let slot = ui.painter().add(egui::Shape::Noop);
    let mut inner = rect.shrink2(vec2(tokens::INPUT_PAD_X, 4.0));
    if let Some(n) = number {
        let at = egui::pos2(inner.left() + 16.0, inner.center().y);
        ui.painter().text(at, egui::Align2::RIGHT_CENTER, n.to_string(), Type::Tiny.font(), c(C::Ink3));
        inner.min.x += 16.0 + tokens::S2;
    }
    let id = field_id(ui, "zikaron-secret", &format!("{hint}#{}", number.unwrap_or(0)));
    // Masked fields do not open the input method: while composing, the candidate bar and composing text are
    // plain text drawn by the system, and composed text can leak into the next field. So while this field has
    // focus, input-method events are dropped, and after drawing the frame's input-method request is withdrawn,
    // so keys produce characters directly.
    let focused = ui.memory(|m| m.has_focus(id));
    if focused {
        ui.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Ime(_))));
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let resp = child.add(
        egui::TextEdit::singleline(text)
            .id(id)
            .font(Type::Mono.font())
            .text_color(c(C::Ink))
            .hint_text(egui::RichText::new(hint).color(c(C::Ink3)))
            .password(true)
            .frame(false)
            .desired_width(inner.width())
            .vertical_align(egui::Align::Center),
    );
    if resp.has_focus() {
        ui.ctx().output_mut(|o| o.ime = None);
    }
    // No plain text in the undo stack: the text control pushes the previous text (a `String` that is never
    // wiped) every frame, and a masked field needs no undo.
    if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), id) {
        st.clear_undoer();
        st.store(ui.ctx(), id);
    }
    let edge = if bad {
        Edge::Bad
    } else if resp.has_focus() {
        Edge::Focus
    } else {
        Edge::Rest
    };
    ui.painter().set(slot, frame_shape(ui.ctx(), rect, id, edge));
    resp
}
