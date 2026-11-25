use crate::palette::{c, C};
use crate::tokens::{self, Type};
use egui::vec2;

pub fn place(ui: &mut egui::Ui, path: &str, none: &str, room: f32) {
    if path.trim().is_empty() {
        ui.label(egui::RichText::new(none).font(Type::Note.font()).color(c(C::Ink3)));
        return;
    }
    let font = Type::MonoSmall.font();
    let shown = crate::width::elide_mid(ui, path, font.clone(), room);
    let r = ui.label(egui::RichText::new(&shown).font(font).color(c(C::Ink)));
    if shown != path {
        let _ = crate::layer::tip(r, path);
    }
}

pub fn mark_words(ui: &mut egui::Ui, m: crate::mark::Mark, words: &str) {
    if !words.is_empty() {
        ui.label(egui::RichText::new(words).font(Type::Note.font()).color(c(C::Ink2)));
    }
    let (r, _) = ui.allocate_exact_size(vec2(tokens::MARK, tokens::MARK), egui::Sense::hover());
    crate::mark::paint_mark(ui.ctx(), ui.painter(), r.center(), m, tokens::MARK);
}
