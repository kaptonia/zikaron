//! Key-value tables: a key column in the second ink as wide as the table's widest key (132 to 180; a wider
//! key wraps), values in body text (monospace for hashes, addresses and times), rows 14 apart, columns 20
//! apart. Long values wrap.

use crate::mark::{self, Mark};
use crate::palette::{c, C};
use crate::tokens::{self, Type};

/// One value. Its kind says whether a copy key goes with it: only a machine value (an address, a digest or
/// fingerprint, an entry or transaction id, a contract or node address, a path, a code) is taken elsewhere to be
/// pasted; words, figures, states and empty values are read, not copied.
#[derive(Clone, Debug)]
pub enum Val {
    /// Words: no copy key.
    Text(String),
    /// A machine value, in the monospace face, with a copy key.
    Mono(String),
    /// A figure in the monospace face that is read, not pasted (a count, a size, a date or time, a state's
    /// token): no copy key.
    Figure(String),
    /// A status mark and words.
    Mark(Mark, String),
    /// Words in a quiet tone (a value that is not there yet).
    Quiet(String),
    /// A value still being read: a shimmering bar of this width.
    Loading(f32),
}

impl Val {
    pub fn text(s: impl Into<String>) -> Val {
        Val::Text(s.into())
    }

    pub fn mono(s: impl Into<String>) -> Val {
        Val::Mono(s.into())
    }

    pub fn figure(s: impl Into<String>) -> Val {
        Val::Figure(s.into())
    }

    /// Whether a copy key goes with this value (a machine value with something in it).
    pub fn copies(&self) -> bool {
        matches!(self, Val::Mono(s) if !s.trim().is_empty())
    }
}

/// The key column's width for a table whose widest key's words are `widest` wide: those words, at least 132,
/// at most 180 (same way as the type column of tables).
pub fn key_width(widest: f32) -> f32 {
    widest.ceil().clamp(tokens::LABEL_W, tokens::LABEL_MAX_W)
}

/// A key-value table.
pub fn kv(ui: &mut egui::Ui, rows: &[(&str, Val)]) {
    let widest = rows.iter().map(|(k, _)| ui.painter().layout_no_wrap(k.to_string(), Type::Body.font(), egui::Color32::BLACK).size().x).fold(0.0, f32::max);
    let key_w = key_width(widest);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = tokens::KV_ROW_GAP;
        for (k, v) in rows {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = tokens::KV_COL_GAP;
                ui.allocate_ui_with_layout(egui::vec2(key_w, Type::Body.line()), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_min_width(key_w);
                    ui.set_max_width(key_w);
                    ui.add(egui::Label::new(egui::RichText::new(*k).font(Type::Body.font()).color(c(C::Ink2)).line_height(Some(Type::Body.line()))).wrap());
                });
                let w = ui.available_width();
                ui.allocate_ui_with_layout(egui::vec2(w, Type::Body.line()), egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.set_max_width(w);
                    value(ui, v);
                });
            });
            // A row with a status mark stands 3 taller: the mark sits 3 below the line's top in the design.
            if matches!(v, Val::Mark(..)) {
                ui.add_space(3.0);
            }
        }
    });
}

/// The details list, the one shape every "details" takes: a label column as wide as the widest label
/// (`label_w`: the caller measures its labels in both languages and gives the larger), values in one line —
/// monospace values elided in the middle (whole on hover) — each row with a copy key at its end when its value
/// can be copied, a fine line between rows across the whole width. A value of several lines lists them left
/// aligned, its first line on the label's line, in their order. Returns the words copied this frame.
pub fn list(ui: &mut egui::Ui, id_salt: &str, rows: &[(&str, Val)], label_w: f32) -> Option<String> {
    let id = ui.id().with(("zikaron-details", id_salt));
    let w = ui.available_width();
    let widest = rows.iter().map(|(k, _)| ui.painter().layout_no_wrap(k.to_string(), Type::Body.font(), egui::Color32::BLACK).size().x).fold(label_w, f32::max).ceil();
    let line_h = Type::Body.line();
    let pad = 9.0;
    let mut copied = None;
    for (i, (k, v)) in rows.iter().enumerate() {
        let lines: Vec<String> = match v {
            Val::Text(s) | Val::Mono(s) | Val::Figure(s) | Val::Quiet(s) | Val::Mark(_, s) => s.split('\n').map(str::to_string).collect(),
            Val::Loading(_) => vec![String::new()],
        };
        let n = lines.len().max(1) as f32;
        let h = pad * 2.0 + line_h * n;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        crate::probe::part(ui.ctx(), ui.layer_id(), rect);
        let p = ui.painter().clone();
        if i > 0 {
            p.hline(rect.x_range(), rect.top(), egui::Stroke::new(1.0_f32, c(C::Line2)));
        }
        let top = rect.top() + pad;
        p.text(egui::pos2(rect.left(), top + line_h / 2.0), egui::Align2::LEFT_CENTER, *k, Type::Body.font(), c(C::Ink2));
        let x = rect.left() + widest + tokens::KV_COL_GAP;
        let copyable = v.copies();
        let key_w = if copyable { 26.0 + tokens::S2 } else { 0.0 };
        let room = (rect.right() - key_w - x).max(20.0);
        if copyable {
            let kr = egui::Rect::from_center_size(egui::pos2(rect.right() - 13.0, top + line_h / 2.0), egui::vec2(26.0, 26.0));
            let r = ui.interact(kr, id.with(("copy", i)), egui::Sense::click());
            crate::button::paint_icon_key_at(ui, kr, &r, crate::icons::Glyph::Copy, true, 6.0);
            if r.clicked() {
                let all = lines.join("\n");
                ui.ctx().copy_text(all.clone());
                copied = Some(all);
            }
        }
        match v {
            Val::Loading(lw) => {
                let r = egui::Rect::from_min_size(egui::pos2(x, top + 3.0), egui::vec2(lw.min(room), 12.0));
                crate::states::paint_skeleton(ui, r, 6.0);
            }
            Val::Mark(m, _) => {
                mark::paint_mark(ui.ctx(), &p, egui::pos2(x + tokens::MARK / 2.0, top + line_h / 2.0), *m, tokens::MARK);
                for (j, l) in lines.iter().enumerate() {
                    crate::paint::at(&p, ui, egui::pos2(x + tokens::MARK + tokens::S2, top + line_h * (j as f32 + 0.5)), egui::Align2::LEFT_CENTER, l, Type::Body, c(C::Ink), room - tokens::MARK - tokens::S2);
                }
            }
            _ => {
                let (t, ink) = match v {
                    Val::Mono(_) | Val::Figure(_) => (Type::Mono, c(C::Ink)),
                    Val::Quiet(_) => (Type::Body, c(C::Ink3)),
                    _ => (Type::Body, c(C::Ink)),
                };
                for (j, l) in lines.iter().enumerate() {
                    let shown = if matches!(v, Val::Mono(_) | Val::Figure(_)) { crate::width::elide_mid(ui, l, t.font(), room) } else { crate::width::elide_to(ui, l, t.font(), room) };
                    let at = p.text(egui::pos2(x, top + line_h * (j as f32 + 0.5)), egui::Align2::LEFT_CENTER, &shown, t.font(), ink);
                    if shown != *l {
                        let _ = crate::layer::tip(ui.interact(at, id.with(("whole", i, j)), egui::Sense::hover()), l.as_str());
                    }
                }
            }
        }
    }
    copied
}

/// One value, drawn where it stands (also used outside tables).
pub fn value(ui: &mut egui::Ui, v: &Val) {
    // The width a value may take is read here, in the value's own column: inside a horizontal row (the
    // status mark's) the available width no longer bounds the text, so the words wrap to this width.
    let room = ui.available_width();
    let wrap = |ui: &mut egui::Ui, s: &str, t: Type, colour: egui::Color32, width: f32| {
        let mut job = egui::text::LayoutJob::single_section(s.to_string(), egui::TextFormat { font_id: t.font(), color: colour, line_height: Some(Type::Body.line()), ..Default::default() });
        // Break where the words allow (`break_anywhere: false`): at a space between English words, between any
        // two Chinese characters, then at a dash or punctuation; only a single word longer than the line is cut
        // inside itself.
        job.wrap = egui::text::TextWrapping { max_width: width.max(1.0), break_anywhere: false, ..Default::default() };
        ui.label(job);
    };
    match v {
        Val::Text(s) => wrap(ui, s, Type::Body, c(C::Ink), room),
        Val::Quiet(s) => wrap(ui, s, Type::Body, c(C::Ink3), room),
        Val::Mono(s) | Val::Figure(s) => wrap(ui, s, Type::Mono, c(C::Ink), room),
        Val::Loading(w) => {
            ui.allocate_ui_with_layout(egui::vec2(*w, Type::Body.line()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                crate::states::skeleton(ui, *w, 12.0, 6.0);
            });
        }
        Val::Mark(m, s) => {
            // One cell (`width::cell`): the mark in a slot a line high, the words wrapping beside it, both from the top.
            let words = crate::paint::wrapped(ui, s, Type::Body.font(), c(C::Ink), Type::Body.line(), room - tokens::MARK - tokens::S2);
            let size = egui::vec2(tokens::MARK + tokens::S2 + words.size().x, words.size().y.max(Type::Body.line()));
            crate::width::cell(ui, size, egui::Layout::left_to_right(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.x = tokens::S2;
                let (slot, _) = ui.allocate_exact_size(egui::vec2(tokens::MARK, Type::Body.line()), egui::Sense::hover());
                mark::paint_mark(ui.ctx(), ui.painter(), slot.center(), *m, tokens::MARK);
                ui.add(egui::Label::new(words));
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The key column takes its widest key's words, at least 132, at most 180.
    #[test]
    fn the_key_column_fits_its_widest_key_between_its_bounds() {
        assert_eq!(key_width(60.0), tokens::LABEL_W, "short keys: the floor");
        assert_eq!(key_width(140.3), 141.0, "a longer key widens it, to whole points");
        assert_eq!(key_width(400.0), tokens::LABEL_MAX_W, "the ceiling; the key wraps");
    }
}
