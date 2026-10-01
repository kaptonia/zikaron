//! Key-value tables: a 132 key column in the second ink, values in body text (monospace for hashes,
//! addresses and times), rows 14 apart, columns 20 apart. Long values wrap anywhere.

use crate::mark::{self, Mark};
use crate::palette::{c, C};
use crate::tokens::{self, Type};

/// One value.
#[derive(Clone, Debug)]
pub enum Val {
    Text(String),
    Mono(String),
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
}

/// A key-value table.
pub fn kv(ui: &mut egui::Ui, rows: &[(&str, Val)]) {
    let key_w = tokens::LABEL_W;
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

/// One value, drawn where it stands (also used outside tables).
pub fn value(ui: &mut egui::Ui, v: &Val) {
    // The width a value may take is read here, in the value's own column: inside a horizontal row (the
    // status mark's) the available width no longer bounds the text, so the words wrap to this width.
    let room = ui.available_width();
    let wrap = |ui: &mut egui::Ui, s: &str, t: Type, colour: egui::Color32, width: f32| {
        let mut job = egui::text::LayoutJob::single_section(s.to_string(), egui::TextFormat { font_id: t.font(), color: colour, line_height: Some(Type::Body.line()), ..Default::default() });
        job.wrap = egui::text::TextWrapping { max_width: width.max(1.0), break_anywhere: true, ..Default::default() };
        ui.label(job);
    };
    match v {
        Val::Text(s) => wrap(ui, s, Type::Body, c(C::Ink), room),
        Val::Quiet(s) => wrap(ui, s, Type::Body, c(C::Ink3), room),
        Val::Mono(s) => wrap(ui, s, Type::Mono, c(C::Ink), room),
        Val::Loading(w) => {
            ui.allocate_ui_with_layout(egui::vec2(*w, Type::Body.line()), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                crate::states::skeleton(ui, *w, 12.0, 6.0);
            });
        }
        Val::Mark(m, s) => {
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = tokens::S2;
                let (slot, _) = ui.allocate_exact_size(egui::vec2(tokens::MARK, Type::Body.line()), egui::Sense::hover());
                mark::paint_mark(ui.ctx(), ui.painter(), slot.center(), *m, tokens::MARK);
                wrap(ui, s, Type::Body, c(C::Ink), room - tokens::MARK - tokens::S2);
            });
        }
    }
}
