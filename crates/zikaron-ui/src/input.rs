use crate::icons::{self, Glyph};
use crate::motion;
use crate::palette::{self, c, C};
use crate::tokens::{self, Radius, Type};
use egui::{vec2, Rect};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Edge {
    Rest,
    Hover,
    Focus,
    Bad,
}

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

fn told_id() -> egui::Id {
    egui::Id::new("zikaron-field-told")
}

pub(crate) fn telling<R>(ui: &mut egui::Ui, bad: bool, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let id = told_id();
    let was: Option<Option<egui::LayerId>> = ui.ctx().data(|d| d.get_temp(id));
    let layer = ui.layer_id();
    ui.ctx().data_mut(|d| d.insert_temp(id, bad.then_some(layer)));
    let r = add(ui);
    ui.ctx().data_mut(|d| match was {
        Some(w) => {
            d.insert_temp(id, w);
        }
        None => {
            d.remove::<Option<egui::LayerId>>(id);
        }
    });
    r
}

pub(crate) fn told(ui: &egui::Ui) -> bool {
    ui.ctx().data(|d| d.get_temp::<Option<egui::LayerId>>(told_id())).flatten() == Some(ui.layer_id())
}

pub fn frame_of(ctx: &egui::Context, rect: Rect, id: egui::Id, open: bool) -> egui::Shape {
    frame_shape(ctx, rect, id, if open { Edge::Focus } else { Edge::Rest })
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Look {
    pub mono: bool,
    pub search: bool,
    pub bad: bool,
    pub centred: bool,
}

pub fn line(ui: &mut egui::Ui, text: &mut String, hint: &str) -> egui::Response {
    let w = ui.available_width();
    field(ui, text, hint, w, Look::default())
}

pub fn line_w(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32) -> egui::Response {
    field(ui, text, hint, w, Look::default())
}

pub fn mono(ui: &mut egui::Ui, text: &mut String, hint: &str) -> egui::Response {
    let w = ui.available_width();
    field(ui, text, hint, w, Look { mono: true, ..Default::default() })
}

pub fn search(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32) -> egui::Response {
    field(ui, text, hint, w, Look { search: true, ..Default::default() })
}

fn unframed() -> egui::Frame {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 2))
}

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

const HINT_SLACK: f32 = 4.0;

pub fn field(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32, look: Look) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(vec2(w, tokens::INPUT_H), egui::Sense::hover());
    let id = field_id(ui, "zikaron-line", hint);
    let slot = ui.painter().add(egui::Shape::Noop);
    let mut inner = rect.shrink2(vec2(tokens::INPUT_PAD_X, 4.0));
    if look.search {
        icons::glyph_at(ui.painter(), Glyph::Search, egui::pos2(rect.left() + 19.0, inner.center().y), 16.0, c(C::Ink3));
        inner.min.x = rect.left() + 34.0;
    }
    let t = if look.mono { Type::Mono } else { Type::Body };
    let hint_type = if look.search { Type::Small } else { t };
    let hint = crate::width::elide_to(ui, hint, hint_type.font(), (inner.width() - HINT_SLACK).max(0.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
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
    let edge = if look.bad || told(ui) {
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

pub fn area(ui: &mut egui::Ui, text: &mut String, rows: usize) -> egui::Response {
    area_hint(ui, text, rows, "")
}

pub fn area_hint(ui: &mut egui::Ui, text: &mut String, rows: usize, hint: &str) -> egui::Response {
    area_ex(ui, text, rows, hint, Type::Mono)
}

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
    let edge = if told(ui) {
        Edge::Bad
    } else if shown.inner.has_focus() {
        Edge::Focus
    } else if ui.rect_contains_pointer(shown.response.rect) {
        Edge::Hover
    } else {
        Edge::Rest
    };
    ui.painter().set(slot, frame_shape(ui.ctx(), shown.response.rect, id, edge));
    shown.inner
}

pub fn secret_line(ui: &mut egui::Ui, text: &mut crate::secret::Secret, hint: &str) -> egui::Response {
    let w = ui.available_width();
    secret_line_w_marked(ui, text, hint, w, false)
}

pub fn secret_line_w_marked(ui: &mut egui::Ui, text: &mut crate::secret::Secret, hint: &str, w: f32, bad: bool) -> egui::Response {
    secret_field(ui, text, hint, w, bad, Face::Plain)
}

pub fn secret_key_line(ui: &mut egui::Ui, text: &mut crate::secret::Secret, hint: &str, w: f32, bad: bool) -> egui::Response {
    secret_field(ui, text, hint, w, bad, Face::Key)
}

#[derive(Clone, Copy)]
enum Face {
    Plain,
    Key,
    Word(usize),
}

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
        Face::Plain => (Type::Mono.font(), Type::Body.font()),
        Face::Key => (Type::MonoSmall.font(), Type::MonoSmall.font()),
        Face::Word(_) => (Type::Mono.font(), Type::Mono.font()),
    };
    let id = field_id(ui, "zikaron-secret", &format!("{hint}#{}", number.unwrap_or(0)));
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
    if let Some(mut st) = egui::TextEdit::load_state(ui.ctx(), id) {
        st.clear_undoer();
        st.store(ui.ctx(), id);
    }
    let edge = if bad || told(ui) {
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
