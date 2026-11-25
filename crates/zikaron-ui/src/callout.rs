use crate::icons::{self, Glyph};
use crate::palette::{c, C};
use crate::tokens::Type;
use egui::{pos2, vec2};

const LINE: f32 = 19.0;
const GLYPH: f32 = 16.0;
const GAP: f32 = 10.0;

pub fn callout(ui: &mut egui::Ui, s: &str) {
    if s.is_empty() {
        return;
    }
    let w = ui.available_width();
    let pad = vec2(12.0, 10.0);
    let room = (w - pad.x * 2.0 - GLYPH - GAP).max(20.0);
    let font = Type::Small.font();
    let mut job = egui::text::LayoutJob::simple(s.to_string(), font, c(C::WarnInk), room);
    if let Some(first) = job.sections.first_mut() {
        first.format.line_height = Some(LINE);
    }
    let galley = ui.painter().layout_job(job);
    let h = galley.size().y.max(LINE) + pad.y * 2.0;
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, egui::CornerRadius::same(12), c(C::WarnWash));
    icons::glyph_at(p, Glyph::Info, pos2(rect.left() + pad.x + GLYPH / 2.0, rect.top() + pad.y + LINE / 2.0), GLYPH, c(C::WarnInk));
    p.galley(pos2(rect.left() + pad.x + GLYPH + GAP, rect.top() + pad.y), galley, c(C::WarnInk));
}
