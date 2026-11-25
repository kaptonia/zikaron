use crate::icons::{self, Glyph};
use crate::palette::{c, C};
use crate::tokens::{self, Type};
use egui::{pos2, vec2, Color32, Rect};

fn tone(step: u8) -> Color32 {
    c(match step {
        0 => C::Tone,
        1 => C::ToneHover,
        _ => C::TonePress,
    })
}

pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui, &mut Stack) -> R) -> R {
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

pub fn stack<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui, &mut Stack) -> R) -> R {
    stack_gap(ui, 14.0, add)
}

fn stack_gap<R>(ui: &mut egui::Ui, gap: f32, add: impl FnOnce(&mut egui::Ui, &mut Stack) -> R) -> R {
    ui.ctx().data_mut(|d| d.remove::<(f32, f32)>(crate::sheet::trailing()));
    ui.vertical(|ui| {
        ui.set_width(ui.available_width());
        ui.spacing_mut().item_spacing.y = gap;
        let mut s = Stack { gap };
        add(ui, &mut s)
    })
    .inner
}

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

pub struct Stack {
    gap: f32,
}

impl Stack {
    pub fn field<R>(&mut self, ui: &mut egui::Ui, label: &str, optional: Option<&str>, line: Option<(&str, bool)>, add: impl FnOnce(&mut egui::Ui, f32) -> R) -> R {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            let w = ui.available_width();
            let left = ui.cursor().left();
            let at = if crate::sheet::echoes_title(ui.ctx(), label) {
                crate::sheet::title_line(ui.ctx())
            } else {
                let row = Rect::from_min_size(ui.cursor().min, vec2(w, Type::Note.line()));
                let ends = name_at(ui, label, optional);
                ui.add_space(name_gap() - 8.0);
                Some((row, ends))
            };
            let said = line.filter(|(s, _)| !s.is_empty());
            let on_line = match (said, at) {
                (Some((s, _)), Some((row, ends))) => crate::width::width_in(ui, s, Type::Small.font()) <= row.right() - ends - tokens::S3,
                _ => true,
            };
            if let (Some((s, bad)), false) = (said, on_line) {
                let ink = if bad { c(C::BadInk) } else { c(C::Ink2) };
                let g = crate::paint::wrapped(ui, s, Type::Small.font(), ink, Type::Small.line(), w);
                let (r, _) = ui.allocate_exact_size(vec2(w, g.size().y), egui::Sense::hover());
                ui.painter().galley(r.left_top(), g, ink);
            }
            let top = ui.cursor().top();
            let r = crate::input::telling(ui, line.is_some_and(|(_, bad)| bad), |ui| add(ui, w));
            let control = Rect::from_min_max(pos2(left, top), pos2(left + w, ui.min_rect().bottom()));
            if let (Some((s, bad)), Some((row, ends)), true) = (said, at, on_line) {
                said_at(ui, pos2(row.right(), row.center().y), row.right() - ends - tokens::S3, s, bad, control);
            }
            r
        })
        .inner
    }

    pub fn pair<A, B>(&mut self, ui: &mut egui::Ui, left: impl FnOnce(&mut egui::Ui, &mut Stack) -> A, right: impl FnOnce(&mut egui::Ui, &mut Stack) -> B) -> (A, B) {
        let cw = ((ui.available_width() - 14.0) / 2.0).max(0.0);
        side_by_side(ui, self.gap, cw, cw, left, right)
    }

    pub fn pair_w<A, B>(&mut self, ui: &mut egui::Ui, right_w: f32, left: impl FnOnce(&mut egui::Ui, &mut Stack) -> A, right: impl FnOnce(&mut egui::Ui, &mut Stack) -> B) -> (A, B) {
        let lw = (ui.available_width() - 14.0 - right_w).max(0.0);
        side_by_side(ui, self.gap, lw, right_w, left, right)
    }

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

pub fn name_gap() -> f32 {
    (tokens::KEY_H - Type::Note.line()) / 2.0 + tokens::S2
}

pub fn name(ui: &mut egui::Ui, label: &str, optional: Option<&str>) {
    let _ = name_at(ui, label, optional);
}

fn name_at(ui: &mut egui::Ui, label: &str, optional: Option<&str>) -> f32 {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), Type::Note.line()), egui::Sense::hover());
    let font = egui::FontId::new(14.0, crate::fonts::medium());
    let quiet = optional.map(|o| (o, ui.painter().layout_no_wrap(o.to_string(), Type::Small.font(), c(C::Ink3)).size().x + 8.0));
    let left = if ui.layout().horizontal_align() == egui::Align::Center {
        let w = ui.painter().layout_no_wrap(label.to_string(), font.clone(), c(C::Ink)).size().x + quiet.map(|(_, w)| w).unwrap_or(0.0);
        r.center().x - w / 2.0
    } else {
        r.left()
    };
    let p = ui.painter();
    let named = p.text(pos2(left, r.center().y), egui::Align2::LEFT_CENTER, label, font, c(C::Ink));
    match quiet {
        Some((o, _)) => p.text(pos2(named.right() + 8.0, r.center().y), egui::Align2::LEFT_CENTER, o, Type::Small.font(), c(C::Ink3)).right(),
        None => named.right(),
    }
}

pub(crate) fn said_at(ui: &egui::Ui, end: egui::Pos2, room: f32, s: &str, bad: bool, control: Rect) {
    let ink = if bad { c(C::BadInk) } else { c(C::Ink2) };
    let fit = crate::width::elide_to(ui, s, Type::Small.font(), room.max(0.0));
    let drawn = ui.painter().text(end, egui::Align2::RIGHT_CENTER, &fit, Type::Small.font(), ink);
    if fit != s {
        let id = ui.id().with(("zikaron-said-at", control.min.x.to_bits(), control.min.y.to_bits()));
        let _ = crate::layer::tip(ui.interact(drawn, id.with("words"), egui::Sense::hover()), s);
        let _ = crate::layer::tip(ui.interact(control, id.with("control"), egui::Sense::hover()), s);
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Look {
    pub mono: bool,
    pub bad: bool,
    pub icon: Option<Glyph>,
    pub read_only: bool,
}

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
    let id = crate::input::field_id(ui, "zikaron-joined", hint);
    let mut edit = ui.new_child(egui::UiBuilder::new().max_rect(edit_rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let resp = crate::compose::around(&mut edit, id, text, |ui, text| {
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
    });
    let hot = resp.hovered();
    let focus = resp.has_focus();
    let bad = look.bad || crate::input::told(ui);
    let shape: egui::Shape = if focus && !bad {
        egui::Shape::Vec(vec![
            egui::epaint::RectShape::filled(rect.expand(3.0), egui::CornerRadius::same(13), c(C::Focus)).into(),
            egui::epaint::RectShape::new(rect, egui::CornerRadius::same(10), c(C::Surface), egui::Stroke::new(1.5_f32, c(C::Accent).gamma_multiply(0.7)), egui::StrokeKind::Inside).into(),
        ])
    } else if bad {
        egui::epaint::RectShape::new(rect, egui::CornerRadius::same(10), tone(0), egui::Stroke::new(1.5_f32, c(C::Bad)), egui::StrokeKind::Inside).into()
    } else {
        egui::epaint::RectShape::filled(rect, egui::CornerRadius::same(10), tone(if hot { 1 } else { 0 })).into()
    };
    ui.painter().set(slot, shape);
    (resp, answer)
}

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

pub fn line(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32, look: Look) -> egui::Response {
    joined::<()>(ui, text, hint, w, look, None::<fn(&mut egui::Ui)>).0
}

pub fn inner_key(ui: &mut egui::Ui, label: &str, glyph: Option<Glyph>) -> egui::Response {
    inner(ui, label, glyph, false)
}

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

pub fn small(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32, bad: bool) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(vec2(w, tokens::KEY_H), egui::Sense::hover());
    let slot = ui.painter().add(egui::Shape::Noop);
    let font = Type::MonoSmall.font();
    let inner = rect.shrink2(vec2(SMALL_PAD_X, 0.0));
    let id = crate::input::field_id(ui, "zikaron-small", hint);
    let mut edit = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let resp = crate::compose::around(&mut edit, id, text, |ui, text| {
        ui.add(
            egui::TextEdit::singleline(text)
                .id(id)
                .frame(egui::Frame::NONE)
                .font(font.clone())
                .text_color(c(C::Ink))
                .hint_text(egui::RichText::new(hint).font(Type::Small.font()).color(c(C::Ink3)))
                .desired_width(inner.width()),
        )
    });
    let bad = bad || crate::input::told(ui);
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

pub fn small_said(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32, said: Option<&str>) -> egui::Response {
    small_told(ui, text, hint, w, said.map(|s| (s, true)))
}

pub fn small_told(ui: &mut egui::Ui, text: &mut String, hint: &str, w: f32, said: Option<(&str, bool)>) -> egui::Response {
    let bad = said.is_some_and(|(_, bad)| bad);
    let resp = crate::input::telling(ui, bad, |ui| small(ui, text, hint, w, false));
    if let Some((s, bad)) = said.filter(|(s, _)| !s.is_empty()) {
        let room = (ui.available_width() - tokens::LABEL_W).max(0.0);
        let fit = crate::width::elide_to(ui, s, Type::Small.font(), room);
        let wide = ui.painter().layout_no_wrap(fit, Type::Small.font(), c(C::BadInk)).size().x;
        let _ = ui.allocate_exact_size(vec2(wide + SAID_GAP, Type::Small.line()), egui::Sense::hover());
        said_at(ui, pos2(resp.rect.left() - SMALL_PAD_X - SAID_GAP, resp.rect.center().y), wide + 0.5, s, bad, resp.rect);
    }
    resp
}

const SAID_GAP: f32 = 8.0;

const SMALL_PAD_X: f32 = 10.0;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedEdit {
    pub typed: Option<String>,
    pub landed: bool,
}

impl SavedEdit {
    pub fn shown(&self, stored: &str) -> String {
        self.typed.clone().unwrap_or_else(|| stored.to_string())
    }

    pub fn dirty(&self, stored: &str) -> bool {
        self.typed.as_deref().is_some_and(|t| t.trim() != stored.trim())
    }

    pub fn edited(&mut self, now: String, stored: &str) {
        self.typed = if now.trim() == stored.trim() { None } else { Some(now) };
        self.landed = false;
    }

    pub fn land(&mut self) {
        self.typed = None;
        self.landed = true;
    }
}

pub fn field_tone() -> Color32 {
    tone(0)
}

pub fn rows<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui, &mut crate::card::Form) -> R) -> R {
    let flag = crate::sheet::inside_id();
    let was = crate::sheet::inside(ui.ctx());
    ui.ctx().data_mut(|d| d.insert_temp(flag, true));
    let r = crate::card::form(ui, add);
    ui.ctx().data_mut(|d| d.insert_temp(flag, was));
    r
}
