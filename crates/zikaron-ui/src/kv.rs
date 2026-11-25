use crate::mark::{self, Mark};
use crate::palette::{c, C};
use crate::tokens::{self, Type};

#[derive(Clone, Debug)]
pub enum Val {
    Text(String),
    Mono(String),
    Figure(String),
    Mark(Mark, String),
    Amount(crate::layer::Amount),
    Quiet(String),
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

    pub fn copies(&self) -> bool {
        matches!(self, Val::Mono(s) if !s.trim().is_empty())
    }
}

pub fn key_width(widest: f32) -> f32 {
    widest.ceil().clamp(tokens::LABEL_W, tokens::LABEL_MAX_W)
}

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
            if matches!(v, Val::Mark(..)) {
                ui.add_space(3.0);
            }
        }
    });
}

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
            Val::Amount(a) => vec![a.shown.clone()],
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
                    Val::Mono(_) | Val::Figure(_) | Val::Amount(_) => (Type::Mono, c(C::Ink)),
                    Val::Quiet(_) => (Type::Body, c(C::Ink3)),
                    _ => (Type::Body, c(C::Ink)),
                };
                for (j, l) in lines.iter().enumerate() {
                    let shown = if matches!(v, Val::Mono(_) | Val::Figure(_)) { crate::width::elide_mid(ui, l, t.font(), room) } else { crate::width::elide_to(ui, l, t.font(), room) };
                    let at = p.text(egui::pos2(x, top + line_h * (j as f32 + 0.5)), egui::Align2::LEFT_CENTER, &shown, t.font(), ink);
                    if let Val::Amount(a) = v {
                        if a.cut() || shown != *l {
                            let _ = crate::layer::tip(ui.interact(at, id.with(("whole", i, j)), egui::Sense::hover()), a.whole.as_str());
                        }
                    } else if shown != *l {
                        let _ = crate::layer::tip(ui.interact(at, id.with(("whole", i, j)), egui::Sense::hover()), l.as_str());
                    }
                }
            }
        }
    }
    copied
}

pub fn value(ui: &mut egui::Ui, v: &Val) {
    let room = ui.available_width();
    let wrap = |ui: &mut egui::Ui, s: &str, t: Type, colour: egui::Color32, width: f32| {
        let mut job = egui::text::LayoutJob::single_section(s.to_string(), egui::TextFormat { font_id: t.font(), color: colour, line_height: Some(Type::Body.line()), ..Default::default() });
        job.wrap = egui::text::TextWrapping { max_width: width.max(1.0), break_anywhere: false, ..Default::default() };
        ui.label(job);
    };
    match v {
        Val::Text(s) => wrap(ui, s, Type::Body, c(C::Ink), room),
        Val::Quiet(s) => wrap(ui, s, Type::Body, c(C::Ink3), room),
        Val::Mono(s) | Val::Figure(s) => wrap(ui, s, Type::Mono, c(C::Ink), room),
        Val::Amount(a) => {
            let r = ui.label(egui::RichText::new(&a.shown).font(Type::Mono.font()).color(c(C::Ink)));
            let _ = crate::layer::amount_tip(r, a);
        }
        Val::Loading(w) => {
            ui.allocate_ui_with_layout(egui::vec2(*w, Type::Body.line()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                crate::states::skeleton(ui, *w, 12.0, 6.0);
            });
        }
        Val::Mark(m, s) => {
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

    #[test]
    fn the_key_column_fits_its_widest_key_between_its_bounds() {
        assert_eq!(key_width(60.0), tokens::LABEL_W, "short keys: the floor");
        assert_eq!(key_width(140.3), 141.0, "a longer key widens it, to whole points");
        assert_eq!(key_width(400.0), tokens::LABEL_MAX_W, "the ceiling; the key wraps");
    }
}
