//! Inputs: 38 high, 15 text, corner 10, a flat grey tone (a step deeper under the pointer); focused, the field
//! turns to the surface with a blue edge and a soft blue ring. Multi-line areas use the monospace face. Masked
//! fields never show their text.

use crate::icons::{self, Glyph};
use crate::motion;
use crate::palette::{self, c, C};
use crate::tokens::{self, Radius, Type};
use egui::{vec2, Rect};

/// The states of the frame. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edge {
    Rest,
    /// At rest under the pointer.
    Hover,
    Focus,
    /// The field's text is wrong (the caller decides; this only draws). Red wins over blue: a wrong field
    /// shows while the person types in it.
    Bad,
}

/// A field's frame as one shape: the soft focus ring (eased in and out over 120 ms), the ground and its edge.
/// Fields draw their text first, so focus is known, and put this shape into a slot reserved under it.
pub fn frame_shape(ctx: &egui::Context, rect: Rect, id: egui::Id, edge: Edge) -> egui::Shape {
    let focus = motion::flag(ctx, id.with("focus"), edge == Edge::Focus, tokens::FAST);
    let hot = motion::flag(ctx, id.with("hot"), edge == Edge::Hover, tokens::FAST);
    let round = Radius::Field.egui();
    let mut v: Vec<egui::Shape> = Vec::new();
    if focus > 0.0 {
        let ring = egui::CornerRadius::same(Radius::Field.px() as u8 + 2);
        v.push(egui::epaint::RectShape::stroke(rect.expand(2.0), ring, egui::Stroke::new(4.0_f32, c(C::Focus).gamma_multiply(focus)), egui::StrokeKind::Middle).into());
    }
    let ground = palette::mix(palette::mix(c(C::Tone), c(C::ToneHover), hot), c(C::Surface), focus);
    v.push(egui::epaint::RectShape::filled(rect, round, ground).into());
    if edge == Edge::Bad {
        v.push(egui::epaint::RectShape::stroke(rect, round, egui::Stroke::new(1.5_f32, c(C::Bad)), egui::StrokeKind::Outside).into());
    } else if focus > 0.0 {
        v.push(egui::epaint::RectShape::stroke(rect, round, egui::Stroke::new(1.0_f32, c(C::Accent).gamma_multiply(0.7 * focus)), egui::StrokeKind::Inside).into());
    }
    egui::Shape::Vec(v)
}

/// The single-line field's frame for a control that is not a text field but looks like one (the date field):
/// open, it draws the focused state. The shape is still [`frame_shape`]'s, the one source of the frame.
pub fn frame_of(ctx: &egui::Context, rect: Rect, id: egui::Id, open: bool) -> egui::Shape {
    frame_shape(ctx, rect, id, if open { Edge::Focus } else { Edge::Rest })
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
    /// The text (and its hint) centred in the field: a short number in a small cell.
    pub centred: bool,
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
/// A text edit's frame when the kit paints the field itself: nothing painted, nothing across (the field's own
/// 12 is the whole inset, as in every field), 2 down.
fn unframed() -> egui::Frame {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 2))
}

/// A field's identity: its words and which of the fields with those words in its `ui` it is, so it keeps its
/// focus and state while the page around it changes. When the words change language ([`crate::flow::morphing`])
/// the field keeps the identity it had (the `n`th field of its kind in its `ui` stays the same field), so what is
/// being typed keeps its focus and cursor.
pub(crate) fn field_id(ui: &egui::Ui, kind: &str, hint: &str) -> egui::Id {
    let worded = worded_id(ui, kind, hint);
    let ctx = ui.ctx();
    let pass = crate::motion::pass(ctx);
    let order_key = egui::Id::new("zikaron-field-order");
    let mut order: (u64, Vec<(egui::Id, u32)>) = ctx.data(|d| d.get_temp(order_key)).filter(|o: &(u64, Vec<(egui::Id, u32)>)| o.0 == pass).unwrap_or((pass, Vec::new()));
    let place = ui.id().with(kind);
    let nth = match order.1.iter_mut().find(|(p, _)| *p == place) {
        Some((_, c)) => {
            *c += 1;
            *c
        }
        None => {
            order.1.push((place, 0));
            0
        }
    };
    ctx.data_mut(|d| d.insert_temp(order_key, order));
    let slot = place.with(("zikaron-field-slot", nth));
    let kept: Option<(egui::Id, egui::Id)> = ctx.data(|d| d.get_temp(slot));
    let id = match kept {
        Some((w, id)) if w == worded => id,
        Some((_, id)) if crate::flow::morphing(ctx) => id,
        _ => worded,
    };
    ctx.data_mut(|d| d.insert_temp(slot, (worded, id)));
    id
}

fn worded_id(ui: &egui::Ui, kind: &str, hint: &str) -> egui::Id {
    let base = ui.id().with((kind, hint));
    let pass = crate::motion::pass(ui.ctx());
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

/// What the text edit keeps inside its own width beyond the hint's words (its cursor's margin).
const HINT_SLACK: f32 = 4.0;

/// The one single-line field.
pub fn field(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32, look: Look) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(vec2(w, tokens::INPUT_H), egui::Sense::hover());
    let id = field_id(ui, "zikaron-line", hint);
    let slot = ui.painter().add(egui::Shape::Noop);
    let mut inner = rect.shrink2(vec2(tokens::INPUT_PAD_X, 4.0));
    if look.search {
        // The glyph 16 with its centre 19 in; the words from 34.
        icons::glyph_at(ui.painter(), Glyph::Search, egui::pos2(rect.left() + 19.0, inner.center().y), 16.0, c(C::Ink3));
        inner.min.x = rect.left() + 34.0;
    }
    let t = if look.mono { Type::Mono } else { Type::Body };
    // A search box's hint is the small size, as designed.
    let hint_type = if look.search { Type::Small } else { t };
    // The hint fits the room the field gives it, measured in its own size: longer words end in "…" instead of
    // running under the frame's edge.
    let hint = crate::width::elide_to(ui, hint, hint_type.font(), (inner.width() - HINT_SLACK).max(0.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    // A centred field's hint is drawn here, on the field's middle: the text edit lays a hint out from its start.
    let painter = ui.painter().clone();
    let resp = crate::compose::around(&mut child, id, text, |child, text| {
        let edit = egui::TextEdit::singleline(text)
            .id(id)
            .font(t.font())
            .text_color(c(C::Ink))
            .frame(unframed())
            .desired_width(inner.width())
            .horizontal_align(if look.centred { egui::Align::Center } else { egui::Align::Min })
            .vertical_align(egui::Align::Center);
        let edit = if look.centred { edit } else { edit.hint_text(egui::RichText::new(hint.clone()).font(hint_type.font()).color(c(C::Ink3))) };
        let resp = child.add(edit);
        if look.centred && text.is_empty() {
            painter.text(resp.rect.center(), egui::Align2::CENTER_CENTER, hint, t.font(), c(C::Ink3));
        }
        resp
    });
    let edge = if look.bad {
        Edge::Bad
    } else if resp.has_focus() {
        Edge::Focus
    } else if ui.rect_contains_pointer(rect) {
        Edge::Hover
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
        crate::compose::around(ui, id, text, |ui, text| {
            ui.add(
                egui::TextEdit::multiline(text)
                    .id(id)
                    .font(t.font())
                    .text_color(c(C::Ink))
                    .hint_text(egui::RichText::new(hint).font(t.font()).color(c(C::Ink3)))
                    .desired_rows(rows)
                    .desired_width(f32::INFINITY)
                    .frame(unframed()),
            )
        })
    });
    let edge = if shown.inner.has_focus() {
        Edge::Focus
    } else if ui.rect_contains_pointer(shown.response.rect) {
        Edge::Hover
    } else {
        Edge::Rest
    };
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
    secret_field(ui, text, hint, w, bad, Face::Plain)
}

/// The masked field for a key written out in characters (a private key): its hint in the same 13 monospace as
/// what is typed.
pub fn secret_key_line(ui: &mut egui::Ui, text: &mut crate::secret::Secret, hint: &str, w: f32, bad: bool) -> egui::Response {
    secret_field(ui, text, hint, w, bad, Face::Key)
}

/// What a masked field holds: a passcode or password, a key in characters, or one of the twelve words.
#[derive(Clone, Copy)]
enum Face {
    Plain,
    Key,
    Word(usize),
}

/// A word cell of the twelve: 36 high, its number the hint until a word is typed (14 monospace, quiet ink).
pub fn secret_numbered(ui: &mut egui::Ui, text: &mut crate::secret::Secret, n: usize, w: f32, bad: bool) -> egui::Response {
    secret_field(ui, text, "", w, bad, Face::Word(n))
}

fn secret_field(ui: &mut egui::Ui, text: &mut crate::secret::Secret, hint: &str, w: f32, bad: bool, face: Face) -> egui::Response {
    let number = match face {
        Face::Word(n) => Some(n),
        _ => None,
    };
    let h = if number.is_some() { tokens::WORD_H } else { tokens::INPUT_H };
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
    let slot = ui.painter().add(egui::Shape::Noop);
    let inner = rect.shrink2(vec2(tokens::INPUT_PAD_X, 4.0));
    let hint = number.map(|n| n.to_string()).unwrap_or_else(|| hint.to_string());
    let (font, hint_font) = match face {
        // A passcode's hint is words, in the plain face; a key's and a word cell's are in the monospace they take.
        Face::Plain => (Type::Mono.font(), Type::Body.font()),
        Face::Key => (Type::MonoSmall.font(), Type::MonoSmall.font()),
        Face::Word(_) => (Type::Mono.font(), Type::Mono.font()),
    };
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
            .font(font)
            .text_color(c(C::Ink))
            .hint_text(egui::RichText::new(&hint).font(hint_font).color(c(C::Ink3)))
            .password(true)
            .frame(unframed())
            .desired_width(inner.width())
            .vertical_align(egui::Align::Center),
    );
    if resp.has_focus() {
        ui.ctx().output_mut(|o| o.ime = None);
    }
    // No plain text kept in the undo stack from frame to frame: the text control pushes the previous text (a
    // `String` egui frees without wiping) every frame, and a masked field needs no undo.
    if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), id) {
        st.clear_undoer();
        st.store(ui.ctx(), id);
    }
    let edge = if bad {
        Edge::Bad
    } else if resp.has_focus() {
        Edge::Focus
    } else if ui.rect_contains_pointer(rect) {
        Edge::Hover
    } else {
        Edge::Rest
    };
    ui.painter().set(slot, frame_shape(ui.ctx(), rect, id, edge));
    resp
}
