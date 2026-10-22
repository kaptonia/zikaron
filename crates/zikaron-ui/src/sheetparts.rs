//! Parts the sheets and the first-run wizard's cards take: the passcode cells at a sheet's size (36 × 44, from
//! the left), the twelve words in three columns (under a veil until opened), the radio dot at a row's right end,
//! a sheet's title with its segmented tabs at the right, the done mark of a finished step, and drawing inside a
//! card that is already one without a ground of its own.

use crate::motion::{self, Curve};
use crate::palette::{self, c, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

/// A passcode cell at a sheet's size.
const CELL_W: f32 = 36.0;
const CELL_H: f32 = 44.0;

/// The passcode row at a sheet's size: eight cells 36 × 44, 8 apart, from the left; otherwise as
/// [`crate::pin::pin_row`] (dots only; letters and digits in, backspace out, a paste takes its letters and
/// digits; the cell being typed has the accent's edge and ring; a refused entry shakes the row).
pub fn pins(ui: &mut egui::Ui, id_salt: &str, value: &mut crate::secret::Secret, len: usize, shake_at: Option<f64>, active: bool) -> crate::pin::PinRow {
    let (cw, ch) = (CELL_W, CELL_H);
    let w = cw * len as f32 + tokens::PIN_GAP * (len as f32 - 1.0);
    let (rect, resp) = ui.push_id(("zikaron-pin", id_salt), |ui| ui.allocate_exact_size(vec2(w, ch), egui::Sense::click())).inner;
    // Focus as the kit's passcode row takes it: a holder is never displaced, and a row under an overlay takes
    // neither focus nor keys.
    let top = crate::layer::on_top(ui, rect);
    let held = resp.has_focus() && top;
    if resp.clicked() {
        resp.request_focus();
    } else if active && top && ui.memory(|m| m.focused().is_none()) {
        resp.request_focus();
    }
    // A row typed into, as a field is: the text beam over it.
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
    let mut changed = false;
    let events = if held { ui.input(|i| i.events.clone()) } else { Vec::new() };
    for e in events {
        match e {
            egui::Event::Text(txt) | egui::Event::Paste(txt) => {
                for ch in txt.chars().filter(|c| c.is_ascii_alphanumeric()) {
                    if value.chars() < len && value.push(ch) {
                        changed = true;
                    }
                }
            }
            egui::Event::Key { key: egui::Key::Backspace, pressed: true, .. } => {
                if !value.is_empty() {
                    value.pop();
                    changed = true;
                }
            }
            _ => {}
        }
    }
    let dx = shake_of(ui.ctx(), shake_at) * motion::travel(ui.ctx());
    let filled = value.chars();
    let id = resp.id;
    let focus = motion::flag(ui.ctx(), id.with("focus"), resp.has_focus(), tokens::FAST);
    let p = ui.painter();
    for i in 0..len {
        let x = rect.left() + dx + (cw + tokens::PIN_GAP) * i as f32;
        let cell = Rect::from_min_size(pos2(x, rect.top()), vec2(cw, ch));
        let here = i == filled.min(len - 1) && filled < len;
        let lit = if here { focus } else { 0.0 };
        if lit > 0.0 {
            p.rect_stroke(cell.expand(2.0), egui::CornerRadius::same(12), egui::Stroke::new(4.0_f32, c(C::Focus).gamma_multiply(lit)), egui::StrokeKind::Middle);
        }
        p.rect(cell, egui::CornerRadius::same(10), palette::mix(c(C::Tone), c(C::Surface), lit), egui::Stroke::new(1.0_f32, c(C::Accent).gamma_multiply(0.7 * lit)), egui::StrokeKind::Inside);
        if i < filled {
            // Each dot pops in once: from half its size, on the spring, over 0.35 s.
            let a = motion::age(ui.ctx(), id.with(("dot", i)), 0);
            let t = (a / DOT_POP).clamp(0.0, 1.0);
            if t < 1.0 {
                ui.ctx().request_repaint();
            }
            let k = 0.5 + 0.5 * Curve::Spring.at(t);
            p.circle_filled(cell.center(), 4.5 * k, c(C::Ink));
        }
    }
    let full = filled == len;
    if full && changed && resp.has_focus() {
        resp.surrender_focus();
    }
    crate::pin::PinRow { full, changed }
}

/// How long a dot takes to pop in, and a refused row to shake.
const DOT_POP: f32 = 0.35 * crate::tokens::PACE;
const SHAKE: f32 = 0.42 * crate::tokens::PACE;

/// The passcode row's shake started at frame time `at`: four swings, 8 left, 7 right, 5 left, 3 right, over
/// 0.42 s; zero when not shaking.
pub fn shake_of(ctx: &egui::Context, at: Option<f64>) -> f32 {
    let Some(at) = at else { return 0.0 };
    let p = ((crate::motion::now(ctx) - at) as f32 / SHAKE).max(0.0);
    if p >= 1.0 {
        return 0.0;
    }
    ctx.request_repaint();
    motion::keys(&[(0.0, 0.0), (0.2, -8.0), (0.4, 7.0), (0.6, -5.0), (0.8, 3.0), (1.0, 0.0)], p)
}

/// The twelve words in three columns, 8 apart: each cell 36 high, corner 10, on the field tone, its number
/// (12, quiet) then the word (monospace 14). Until `open` (or while there are no words) the cells hold blurred
/// marks under a veil that says `cover`; a click on the veil asks to open. The words are painted, never
/// selectable. Returns the grid's response (clicked while veiled).
pub fn words(ui: &mut egui::Ui, id_salt: &str, words: Option<&[crate::secret::Secret]>, open: bool, cover: &str) -> egui::Response {
    let id = ui.id().with(("zikaron-words-grid", id_salt));
    let w = ui.available_width();
    let gap = tokens::PIN_GAP;
    let cols = 3usize;
    let h = tokens::WORD_H * 4.0 + gap * 3.0;
    let shown = words.filter(|_| open);
    let sense = if shown.is_none() { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    let p = ui.painter();
    let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
    // The veil clears over 0.3 s once the words are opened.
    let veil = motion::flag(ui.ctx(), id.with("veil"), shown.is_none(), tokens::SLOW);
    for i in 0..12 {
        let (r, cc) = (i / cols, i % cols);
        let cell = Rect::from_min_size(pos2(rect.left() + (cw + gap) * cc as f32, rect.top() + (tokens::WORD_H + gap) * r as f32), vec2(cw, tokens::WORD_H));
        p.rect_filled(cell, Radius::Field.egui(), c(C::Tone));
        let nx = cell.left() + 12.0;
        p.text(pos2(nx, cell.center().y), egui::Align2::LEFT_CENTER, (i + 1).to_string(), Type::Tiny.font(), c(C::Ink3).gamma_multiply(1.0 - 0.6 * veil));
        let wx = nx + 16.0 + 8.0;
        match shown.and_then(|ws| ws.get(i)) {
            Some(word) => {
                p.text(pos2(wx, cell.center().y), egui::Align2::LEFT_CENTER, word.expose(), egui::FontId::new(14.0, egui::FontFamily::Monospace), c(C::Ink).gamma_multiply(1.0 - veil));
            }
            None => {
                // A blurred word: a soft bar of a word's length, never the word itself.
                let bw = 34.0 + ((i * 7) % 5) as f32 * 7.0;
                let bar = Rect::from_min_size(pos2(wx, cell.center().y - 5.0), vec2(bw.min(cell.right() - wx - 12.0).max(0.0), 10.0));
                p.rect_filled(bar, egui::CornerRadius::same(5), c(C::Ink3).gamma_multiply(0.22));
            }
        }
    }
    if veil > 0.0 {
        let hot = resp.hovered();
        p.rect_filled(rect, egui::CornerRadius::same(12), c(C::Surface).gamma_multiply(if hot { 0.62 } else { 0.55 } * veil));
        p.text(rect.center(), egui::Align2::CENTER_CENTER, cover, egui::FontId::new(14.0, crate::fonts::medium()), c(C::Ink).gamma_multiply(veil));
    }
    if shown.is_none() {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// A radio dot at a row's right end: 18 across, a ring in the dashed ink, or the accent's (over 0.2 s) with its
/// dot popping in from half its size over 0.3 s when on.
pub fn radio(ui: &mut egui::Ui, id_salt: &str, on: bool) {
    let id = ui.id().with(("zikaron-radio", id_salt));
    let (r, _) = ui.allocate_exact_size(vec2(18.0, 18.0), egui::Sense::hover());
    let lit = motion::flag(ui.ctx(), id, on, tokens::MID);
    let p = ui.painter();
    p.circle_stroke(r.center(), 8.25, egui::Stroke::new(1.5_f32, palette::mix(c(C::Dash), c(C::Accent), lit)));
    if on {
        let a = motion::age(ui.ctx(), id.with("dot"), u64::from(on));
        let t = (a / (0.3 * crate::tokens::PACE)).clamp(0.0, 1.0);
        if t < 1.0 {
            ui.ctx().request_repaint();
        }
        p.circle_filled(r.center(), 4.5 * (0.5 + 0.5 * crate::covers::glide(t)), c(C::Accent));
    }
}

/// A sheet's title with its tabs as a small segmented control (each cell as wide as its words) at the right end
/// of the same line, then the room a title leaves under it. Returns the tab clicked when it is not the current one.
pub fn title_seg(ui: &mut egui::Ui, title: &str, id_salt: &str, labels: &[&str], current: usize) -> Option<usize> {
    crate::sheet::said_title(ui.ctx(), title);
    let w = ui.available_width();
    // The title keeps the line every sheet title has; the seg, taller, sits on the title's middle and reaches
    // as far above the line as below it.
    let line = Type::Sheet.line();
    let (rect, _) = ui.allocate_exact_size(vec2(w, line), egui::Sense::hover());
    let seg_rect = egui::Rect::from_center_size(rect.center(), vec2(w, tokens::SEG_SM_H.max(line)));
    let cells: Vec<crate::seg::Cell> = labels.iter().map(|l| crate::seg::Cell::from(*l)).collect();
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(seg_rect).layout(egui::Layout::right_to_left(egui::Align::Center)));
    let hit = crate::seg::seg_small(&mut child, id_salt, &cells, current);
    let seg_left = child.min_rect().left();
    let room = (seg_left - 16.0 - rect.left()).max(0.0);
    let fit = crate::width::elide_to(ui, title, Type::Sheet.font(), room);
    ui.painter().text(pos2(rect.left(), rect.center().y), egui::Align2::LEFT_CENTER, fit, Type::Sheet.font(), c(C::Ink));
    ui.add_space((tokens::S3 + 2.0 + (seg_rect.height() - line) / 2.0 - ui.spacing().item_spacing.y).max(0.0));
    hit
}

/// The done mark of a finished step: a 44 ring in the ok ink popping in from 0.85 on the spring (0.6 s), its
/// check drawn stroke by stroke over 0.5 s from 0.15 s in, and the word under it, 6 apart, centred.
pub fn done(ui: &mut egui::Ui, id_salt: &str, word: &str) {
    let id = ui.id().with(("zikaron-done-mark", id_salt));
    let a = motion::age(ui.ctx(), id, 0);
    let pop = (a / (0.6 * crate::tokens::PACE)).clamp(0.0, 1.0);
    let draw = ((a - 0.15 * crate::tokens::PACE) / (0.5 * crate::tokens::PACE)).clamp(0.0, 1.0);
    if pop < 1.0 || draw < 1.0 {
        ui.ctx().request_repaint();
    }
    let k = 1.0 - 0.15 * (1.0 - Curve::Spring.at(pop)) * motion::travel(ui.ctx());
    let w = ui.available_width();
    let h = 6.0 + 44.0 + 6.0 + Type::Strong.line() + 2.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
    let p = ui.painter();
    let centre = pos2(rect.center().x, rect.top() + 6.0 + 22.0);
    p.circle_stroke(centre, 21.0 * k, egui::Stroke::new(2.0_f32, c(C::Ok)));
    // The check: two strokes, drawn along their length.
    let pts = [pos2(-6.5, 0.5), pos2(-2.0, 5.0), pos2(7.0, -5.0)].map(|q| centre + q.to_vec2() * k);
    let (l1, l2) = ((pts[1] - pts[0]).length(), (pts[2] - pts[1]).length());
    let along = (l1 + l2) * Curve::Ease.at(draw);
    if along > 0.0 {
        let mut line = vec![pts[0]];
        if along <= l1 {
            line.push(pts[0] + (pts[1] - pts[0]) * (along / l1));
        } else {
            line.push(pts[1]);
            line.push(pts[1] + (pts[2] - pts[1]) * ((along - l1) / l2));
        }
        p.add(egui::Shape::line(line, egui::Stroke::new(2.4_f32, c(C::Ok))));
    }
    p.text(pos2(rect.center().x, rect.top() + 6.0 + 44.0 + 6.0), egui::Align2::CENTER_TOP, word, egui::FontId::new(15.0, crate::fonts::strong()), c(C::Ink));
}

/// Lay `add` as a sheet's body lays its groups: rows with a fine line between them and no ground of their own
/// (inside a card that is the ground already). The flag is the sheet's own (`sheet::inside`).
pub fn bare<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let key = egui::Id::new("zikaron-sheet-inside");
    let was = crate::sheet::inside(ui.ctx());
    ui.ctx().data_mut(|d| d.insert_temp(key, true));
    let r = add(ui);
    ui.ctx().data_mut(|d| d.insert_temp(key, was));
    r
}

