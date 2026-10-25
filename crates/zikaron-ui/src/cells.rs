//! Number cells in one box: a few short number cells side by side inside one field frame, each followed by its
//! unit in the second ink (a span of time typed as days, hours and minutes). The box is one control: one frame,
//! blue while any cell is typed in, red while any cell is wrong; the wrong cell's digits turn red too. Turned
//! off, the whole box is faint and takes nothing.

use crate::input::{self, Edge};
use crate::palette::{c, C};
use crate::tokens::{self, Type};
use egui::{pos2, vec2, Rect};

/// The room before the first cell, after each unit and after the last unit.
const LEAD: f32 = 4.0;
const AFTER_UNIT: f32 = tokens::S3;

/// How wide the box is for `units` in cells `cell_w` wide.
pub fn width(ui: &egui::Ui, units: &[&str], cell_w: f32) -> f32 {
    let words: f32 = units.iter().map(|u| crate::paint::galley(ui, u, Type::Body, c(C::Ink2)).size().x).sum();
    LEAD + (cell_w + tokens::S2) * units.len() as f32 + words + AFTER_UNIT * units.len() as f32
}

/// The box: one cell per unit (`values[i]` typed in the cell before `units[i]`), each `cell_w` wide with its
/// number centred and `hint` in it while empty. `bad[i]` marks a wrong cell. Returns each cell's response.
pub fn cells(ui: &mut egui::Ui, id_salt: &str, values: &mut [String], units: &[&str], cell_w: f32, hint: &str, bad: &[bool], enabled: bool) -> Vec<egui::Response> {
    let w = width(ui, units, cell_w);
    let (rect, _) = ui.allocate_exact_size(vec2(w, tokens::INPUT_H), egui::Sense::hover());
    let id = egui::Id::new(("zikaron-cells", id_salt));
    let slot = ui.painter().add(egui::Shape::Noop);
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(("zikaron-cells", id_salt)).max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    if !enabled {
        child.disable();
        child.multiply_opacity(tokens::OFF);
    }
    let mut out = Vec::new();
    let mut x = rect.left() + LEAD;
    for (i, unit) in units.iter().enumerate() {
        let Some(value) = values.get_mut(i) else { break };
        let wrong = bad.get(i).copied().unwrap_or(false);
        let cell = Rect::from_min_size(pos2(x, rect.top() + 4.0), vec2(cell_w, rect.height() - 8.0));
        let mut cu = child.new_child(egui::UiBuilder::new().max_rect(cell).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let painter = child.painter().clone();
        let r = crate::compose::around(&mut cu, id.with(i), value, |cu, value| {
            let edit = egui::TextEdit::singleline(value)
                .id(id.with(i))
                .font(Type::Mono.font())
                .text_color(if wrong { c(C::Bad) } else { c(C::Ink) })
                .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(4, 2)))
                .desired_width(cell_w - 8.0)
                .horizontal_align(egui::Align::Center)
                .vertical_align(egui::Align::Center);
            let r = cu.add(edit);
            if value.is_empty() {
                painter.text(cell.center(), egui::Align2::CENTER_CENTER, hint, Type::Mono.font(), c(C::Ink3));
            }
            r
        });
        x = cell.right() + tokens::S2;
        let g = crate::paint::galley(ui, unit, Type::Body, c(C::Ink2));
        let gw = g.size().x;
        child.painter().galley(pos2(x, rect.center().y - g.size().y / 2.0), g, c(C::Ink2));
        x += gw + AFTER_UNIT;
        out.push(r);
    }
    let edge = if bad.iter().any(|b| *b) {
        Edge::Bad
    } else if out.iter().any(|r| r.has_focus()) {
        Edge::Focus
    } else if enabled && ui.rect_contains_pointer(rect) {
        Edge::Hover
    } else {
        Edge::Rest
    };
    let frame = input::frame_shape(ui.ctx(), rect, id, edge);
    ui.painter().set(slot, if enabled { frame } else { fade(frame) });
    out
}

/// A frame shape at the faint strength of a control that is off.
fn fade(s: egui::Shape) -> egui::Shape {
    match s {
        egui::Shape::Vec(v) => egui::Shape::Vec(v.into_iter().map(fade).collect()),
        egui::Shape::Rect(mut r) => {
            r.fill = r.fill.gamma_multiply(tokens::OFF);
            r.stroke.color = r.stroke.color.gamma_multiply(tokens::OFF);
            egui::Shape::Rect(r)
        }
        other => other,
    }
}
