//! A figure card: its name (13, second ink) at the top, one large figure (26, semibold), and at its foot a
//! line saying how it stands (a mark or glyph and 13 words), spread over the card's height (22 × 24 inside).
//! A card that does something when clicked lifts on hover and presses like an opening card.

use crate::icons::{self, Glyph};
use crate::mark::{self, Mark};
use crate::material::{self, Level};
use crate::motion;
use crate::paint;
use crate::palette::{c, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2};

/// The figure's size and line.
const FIGURE: f32 = 26.0;
const FIGURE_LINE: f32 = 34.0;

/// Draw the card `min_h` high at least, as wide as the room; `foot` is the line at its foot (none: the figure
/// sits at the foot). Returns the card's response (clicks only when `clickable`).
pub fn figure_card(ui: &mut egui::Ui, label: &str, figure: &str, foot: Option<(Option<Mark>, &str)>, min_h: f32, clickable: bool) -> egui::Response {
    let w = ui.available_width();
    let pad = vec2(tokens::CARD_PAD_X, tokens::CARD_PAD_Y);
    let content = Type::Small.line() + 8.0 + FIGURE_LINE + if foot.is_some() { 8.0 + Type::Small.line() } else { 0.0 };
    let h = (content + pad.y * 2.0).max(min_h).max(crate::grid::cell_height(ui));
    let sense = if clickable { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), sense);
    let st = motion::states(ui.ctx(), resp.id, resp.hovered(), resp.is_pointer_button_down_on(), resp.has_focus(), clickable);
    let lift = if clickable { st.lift() } else { 0.0 };
    let press = 1.0 + (0.995 - 1.0) * if clickable { st.press } else { 0.0 };
    let r = paint::scaled(rect.translate(vec2(0.0, -lift)), press);
    let p = ui.painter();
    material::paint(p, r, Radius::Card, Level::Card, lift);
    let inner = r.shrink2(pad);
    paint::at(p, ui, inner.left_top() + vec2(0.0, Type::Small.line() / 2.0), egui::Align2::LEFT_CENTER, label, Type::Small, c(C::Ink2), inner.width());
    let font = egui::FontId::new(FIGURE, crate::fonts::strong());
    let fit = crate::width::elide_to(ui, figure, font.clone(), inner.width());
    match foot {
        Some((m, words)) => {
            p.text(pos2(inner.left(), inner.center().y), egui::Align2::LEFT_CENTER, fit, font, c(C::Ink));
            let ly = inner.bottom() - Type::Small.line() / 2.0;
            let ink = match m {
                Some(Mark::Ok) => c(C::OkInk),
                Some(Mark::Warn) => c(C::WarnInk),
                Some(Mark::Bad) => c(C::BadInk),
                _ => c(C::Ink2),
            };
            let x = inner.left();
            let lead = match m {
                Some(Mark::Ok) => {
                    icons::glyph_at(p, Glyph::Ok, pos2(x + 7.0, ly), 14.0, ink);
                    14.0 + 6.0
                }
                Some(m) => {
                    mark::paint_mark(ui.ctx(), p, pos2(x + 7.0, ly), m, 14.0);
                    14.0 + 6.0
                }
                // Words alone (what a press on the card does).
                None => 0.0,
            };
            paint::at(p, ui, pos2(x + lead, ly), egui::Align2::LEFT_CENTER, words, Type::Small, ink, (inner.width() - lead).max(0.0));
        }
        None => {
            p.text(pos2(inner.left(), inner.bottom() - FIGURE_LINE / 2.0), egui::Align2::LEFT_CENTER, fit, font, c(C::Ink));
        }
    }
    crate::probe::tile(ui.ctx(), rect, true);
    if clickable {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}
