//! Passcode cells, the twelve word cells, the masked word display and the strength bar.

use crate::motion::{self, Curve};
use crate::paint;
use crate::palette::{self, c, C};
use crate::tokens::{self, Type};
use egui::{pos2, vec2, Rect};

/// What a passcode row did this frame.
pub struct PinRow {
    /// Filled this frame (the caller acts on it).
    pub full: bool,
    /// Changed this frame (the caller clears its error line).
    pub changed: bool,
}

/// The passcode row: eight cells (40 × 54, or 30 × 40 when `small`), dots only. The row takes the keyboard:
/// ASCII letters and digits in, backspace out; pasting takes the letters and digits of the pasted text. A
/// filled dot pops in; the cell being typed has a blue edge and ring; a refused entry shakes the row for
/// 400 ms (from `shake_at`). Returns `full` once when the eighth character lands.
pub fn pin_row(ui: &mut egui::Ui, id_salt: &str, value: &mut crate::secret::Secret, len: usize, shake_at: Option<f64>, active: bool, small: bool) -> PinRow {
    let (cw, ch) = if small { (tokens::PIN_SMALL_W, tokens::PIN_SMALL_H) } else { (tokens::PIN_W, tokens::PIN_H) };
    let w = cw * len as f32 + tokens::PIN_GAP * (len as f32 - 1.0);
    let (rect, resp) = ui.push_id(("zikaron-pin", id_salt), |ui| ui.allocate_exact_size(vec2(w, ch), egui::Sense::click())).inner;
    // Focus is real focus and keys are taken only with it; `active` only means "take focus when nobody holds
    // it", and a holder is never displaced. A row that takes focus this frame does not take this frame's keys.
    let held = resp.has_focus();
    if resp.clicked() {
        resp.request_focus();
    } else if active && ui.memory(|m| m.focused().is_none()) {
        resp.request_focus();
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
    let dx = motion::shake_at(ui.ctx(), shake_at, 0.4);
    let filled = value.chars();
    let id = resp.id;
    let focus = motion::flag(ui.ctx(), id.with("focus"), resp.has_focus(), tokens::FAST);
    let p = ui.painter();
    for i in 0..len {
        let x = rect.left() + dx + (cw + tokens::PIN_GAP) * i as f32;
        let cell = Rect::from_min_size(pos2(x, rect.top()), vec2(cw, ch));
        let here = i == filled.min(len - 1) && filled < len;
        let r = if small { 7 } else { 9 };
        if here && focus > 0.0 {
            p.rect_stroke(cell.expand(1.5), egui::CornerRadius::same(r + 2), egui::Stroke::new(3.0_f32, c(C::Focus).gamma_multiply(focus)), egui::StrokeKind::Middle);
        }
        let line = if here { palette::mix(c(C::Line), c(C::Accent), focus) } else { c(C::Line) };
        p.rect(cell, egui::CornerRadius::same(r), c(C::Surface), egui::Stroke::new(1.0_f32, line), egui::StrokeKind::Inside);
        if i < filled {
            // Each dot pops in with a spring the first time it is drawn.
            let a = motion::age(ui.ctx(), id.with(("dot", i)), 0);
            let k = if a < tokens::FAST {
                ui.ctx().request_repaint();
                0.6 + 0.4 * Curve::Spring.at(a / tokens::FAST)
            } else {
                1.0
            };
            let dr = if small { 4.0 } else { 5.0 };
            p.circle_filled(cell.center(), dr * k, c(C::Ink).gamma_multiply((a / tokens::FAST).clamp(0.0, 1.0).max(0.2)));
        }
    }
    let full = filled == len;
    if full && changed && resp.has_focus() {
        resp.surrender_focus();
    }
    PinRow { full, changed }
}

/// Twelve recovery word cells (three across), each masked and numbered. A whole phrase pasted into any cell
/// is split and fills the cells in order. `bad` marks cells whose word is not in the list. Returns whether
/// anything changed.
pub fn words_grid_marked(ui: &mut egui::Ui, id_salt: &str, words: &mut [crate::secret::Secret; 12], bad: &[bool]) -> bool {
    let mut changed = false;
    let cols = 3usize;
    let w = ((ui.available_width() - tokens::PIN_GAP * (cols as f32 - 1.0)) / cols as f32).floor();
    ui.push_id(("zikaron-words", id_salt), |ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = tokens::PIN_GAP;
            for row in 0..4 {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = tokens::PIN_GAP;
                    for col in 0..cols {
                        let i = row * cols + col;
                        let red = bad.get(i).copied().unwrap_or(false);
                        if crate::input::secret_numbered(ui, &mut words[i], i + 1, w, red).changed() {
                            changed = true;
                        }
                    }
                });
            }
        });
    });
    // Split: from the cell a phrase was pasted into, fill onward in order. Each piece stays a secret type.
    if changed {
        if let Some(at) = words.iter().position(|x| x.expose().split_whitespace().count() > 1) {
            let got: Vec<crate::secret::Secret> = words[at].expose().split_whitespace().map(crate::secret::Secret::of).collect();
            for (k, word) in got.into_iter().enumerate() {
                if at + k < words.len() {
                    words[at + k] = word;
                }
            }
        }
    }
    changed
}

/// The same twelve cells without marks.
pub fn words_grid(ui: &mut egui::Ui, id_salt: &str, words: &mut [crate::secret::Secret; 12]) -> bool {
    words_grid_marked(ui, id_salt, words, &[false; 12])
}

/// The masked display: until opened, a dashed striped block with "click to show"; opened, the twelve numbered
/// words in `cols` columns, each cell entering 25 ms after the one before. The words are painted, not
/// selectable, so they cannot be copied. Returns the block's response (a click while masked reveals).
pub fn mask(ui: &mut egui::Ui, id_salt: &str, words: Option<&[crate::secret::Secret]>, cover: &str, cols: usize, height: f32) -> egui::Response {
    let id = ui.id().with(("zikaron-mask", id_salt));
    let w = ui.available_width();
    let cols = cols.max(1);
    let rows = 12usize.div_ceil(cols);
    let cell_h = 38.0;
    let h = match words {
        Some(_) => cell_h * rows as f32 + tokens::PIN_GAP * (rows as f32 - 1.0),
        None => height,
    };
    let sense = if words.is_none() { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    let p = ui.painter();
    match words {
        None => {
            let hot = motion::flag(ui.ctx(), id.with("hot"), resp.hovered(), tokens::FAST);
            p.rect_filled(rect, egui::CornerRadius::same(10), c(C::Drop));
            // Diagonal stripes, 8 wide.
            let clip = p.with_clip_rect(rect.shrink(1.0));
            let mut x = rect.left() - rect.height();
            while x < rect.right() {
                let a = pos2(x, rect.bottom());
                let b = pos2(x + rect.height(), rect.top());
                clip.line_segment([a, b], egui::Stroke::new(5.7_f32, c(C::Ground)));
                x += 16.0 * std::f32::consts::SQRT_2;
            }
            crate::drop::dashed(p, rect.shrink(0.75), 10.0, egui::Stroke::new(1.5_f32, palette::mix(c(C::Dash), c(C::Accent), hot)), 3.0, 1.5);
            p.text(rect.center(), egui::Align2::CENTER_CENTER, cover, Type::Body.font(), c(C::Ink2));
        }
        Some(ws) => {
            let gap = tokens::PIN_GAP;
            let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
            let a = motion::age(ui.ctx(), id.with("reveal"), motion::key_of(&ws.len()));
            for (i, word) in ws.iter().enumerate().take(12) {
                let (r, cc) = (i / cols, i % cols);
                let t = ((a - i as f32 * 0.025) / tokens::MID).clamp(0.0, 1.0);
                if t < 1.0 {
                    ui.ctx().request_repaint();
                }
                let e = Curve::Ease.at(t);
                let cell = Rect::from_min_size(pos2(rect.left() + (cw + gap) * cc as f32, rect.top() + (cell_h + gap) * r as f32 + 6.0 * (1.0 - e)), vec2(cw, cell_h));
                p.rect(cell, egui::CornerRadius::same(8), c(C::Surface).gamma_multiply(e), egui::Stroke::new(1.0_f32, c(C::Line).gamma_multiply(e)), egui::StrokeKind::Inside);
                p.text(pos2(cell.left() + 12.0 + 16.0, cell.center().y), egui::Align2::RIGHT_CENTER, (i + 1).to_string(), Type::Tiny.font(), c(C::Ink3).gamma_multiply(e));
                p.text(pos2(cell.left() + 12.0 + 16.0 + 8.0, cell.center().y), egui::Align2::LEFT_CENTER, word.expose(), Type::MonoSmall.font(), c(C::Ink).gamma_multiply(e));
            }
        }
    }
    if words.is_none() {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// The strength bar: three segments 34 × 4, `lit` of them in the tone's color, then the reading.
pub fn strength(ui: &mut egui::Ui, lit: usize, tone: crate::mark::Mark, text: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let colour = match tone {
            crate::mark::Mark::Ok => c(C::Ok),
            crate::mark::Mark::Warn => c(C::Warn),
            _ => c(C::Bad),
        };
        for i in 0..3 {
            let (r, _) = ui.allocate_exact_size(vec2(34.0, 4.0), egui::Sense::hover());
            let on = motion::flag(ui.ctx(), ui.id().with(("strength", i)), i < lit, tokens::MID);
            ui.painter().rect_filled(r, egui::CornerRadius::same(2), palette::mix(c(C::Line), colour, on));
        }
        ui.add_space(4.0);
        paint::text(ui, text, Type::Tiny, c(C::Ink2));
    });
}
