//! A drop zone that says its words on one line: the glyph, the bold title and the quiet note side by side,
//! 12 apart, centred (the compact zone above a list). It is a [`crate::drop::zone`] in every other respect: the
//! same ground, dashes, lighting and receiving.

use crate::drop::{self, Shape};
use crate::icons::{self, Glyph};
use crate::palette::{c, C};
use crate::tokens::Type;
use egui::{pos2, vec2, Rect};

/// The gap between the glyph, the title and the note.
const GAP: f32 = 12.0;
/// The glyph's size.
const GLYPH: f32 = 22.0;

/// Draw the one-line zone `height` high; what it received this frame.
pub fn inline(ui: &mut egui::Ui, id_salt: &str, glyph: Option<Glyph>, title: &str, note: &str, height: f32, accept: bool) -> drop::Drop {
    let top = ui.cursor().min;
    let w = ui.available_width();
    let got = drop::zone(ui, id_salt, None, &[], None, height, Shape::Row, accept);
    let h = height.max(crate::grid::cell_height(ui));
    let rect = Rect::from_min_size(top, vec2(w, h));
    let p = ui.painter();
    let heavy = Type::Strong.font();
    let tg = p.layout_no_wrap(title.to_string(), heavy, c(C::Ink));
    let ng = p.layout_no_wrap(note.to_string(), Type::Note.font(), c(C::Ink2));
    let gw = if glyph.is_some() { GLYPH + GAP } else { 0.0 };
    let nw = if note.is_empty() { 0.0 } else { GAP + ng.size().x };
    let total = gw + tg.size().x + nw;
    let mut x = rect.center().x - total / 2.0;
    let y = rect.center().y;
    if let Some(g) = glyph {
        icons::glyph_at(p, g, pos2(x + GLYPH / 2.0, y), GLYPH, c(C::Ink3));
        x += gw;
    }
    let tw = tg.size().x;
    p.galley(pos2(x, y - tg.size().y / 2.0), tg, c(C::Ink));
    if !note.is_empty() {
        p.galley(pos2(x + tw + GAP, y - ng.size().y / 2.0), ng, c(C::Ink2));
    }
    got
}
