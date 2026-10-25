//! **The length box** (how long something lasts, or a time of day): one field-toned bar as tall as a field, cut
//! into equal parts by hairlines, one part per unit ("days, hours, minutes"; "hour, minute"). In each part the
//! number (monospace, right-aligned, three digits at most) and its unit (small, second ink) sit together in the
//! middle; a click anywhere in the part types into its number. A part past its most turns its number and the
//! bar's edge red; turned off, the whole bar is faint and takes nothing.
//!
//! The numbers are read here too ([`read`]): every page that asks for a length in these parts reads it the same
//! way, each with its own most per part.

use crate::paint;
use crate::palette::{c, C};
use crate::tokens::{self, Type};

/// The number in each part (three digits at most), and the room between it and its unit.
pub const NUM_W: f32 = 40.0;
pub const GAP: f32 = 6.0;

/// The box: `values[i]` typed before `units[i]`, `hint` in an empty number, `bad[i]` that part past its most.
/// Returns each part's number's response.
pub fn boxed(ui: &mut egui::Ui, id_salt: &str, values: &mut [String], units: &[&str], hint: &str, bad: &[bool], enabled: bool) -> Vec<egui::Response> {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), tokens::INPUT_H), egui::Sense::hover());
    let id = egui::Id::new(("span-box", id_salt));
    let slot = ui.painter().add(egui::Shape::Noop);
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(("span-box", id_salt)).max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    if !enabled {
        child.disable();
        child.multiply_opacity(tokens::OFF);
    }
    let n = units.len().max(1);
    let part_w = rect.width() / n as f32;
    let font = Type::Mono.font();
    let mut out = Vec::new();
    for (i, unit) in units.iter().enumerate() {
        let Some(value) = values.get_mut(i) else { break };
        let part = egui::Rect::from_min_size(egui::pos2(rect.left() + part_w * i as f32, rect.top()), egui::vec2(part_w, rect.height()));
        if i > 0 {
            child.painter().vline(part.left(), part.y_range(), egui::Stroke::new(1.0_f32, c(C::Line)));
        }
        let g = paint::galley(&child, unit, Type::Small, c(C::Ink2));
        let x = part.center().x - (NUM_W + GAP + g.size().x) / 2.0;
        let num = egui::Rect::from_min_size(egui::pos2(x, part.top() + 4.0), egui::vec2(NUM_W, part.height() - 8.0));
        let cell_id = id.with(i);
        if child.interact(part, id.with(("part", i)), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::Text).clicked() {
            child.memory_mut(|m| m.request_focus(cell_id));
        }
        let wrong = bad.get(i).copied().unwrap_or(false);
        let mut cu = child.new_child(egui::UiBuilder::new().max_rect(num).layout(egui::Layout::left_to_right(egui::Align::Center)));
        let painter = child.painter().clone();
        let r = crate::compose::around(&mut cu, cell_id, value, |cu, value| {
            let edit = egui::TextEdit::singleline(value)
                .id(cell_id)
                .font(font.clone())
                .text_color(if wrong { c(C::Bad) } else { c(C::Ink) })
                .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(2, 2)))
                .desired_width(NUM_W - 4.0)
                .horizontal_align(egui::Align::Max)
                .vertical_align(egui::Align::Center);
            let resp = cu.add(edit);
            if value.is_empty() {
                painter.text(egui::pos2(num.right() - 2.0, num.center().y), egui::Align2::RIGHT_CENTER, hint, font.clone(), c(C::Ink3));
            }
            resp
        });
        out.push(r);
        child.painter().galley(egui::pos2(num.right() + GAP, part.center().y - g.size().y / 2.0), g, c(C::Ink2));
    }
    for value in values.iter_mut() {
        *value = digits(value);
    }
    let edge = if bad.iter().any(|b| *b) {
        crate::input::Edge::Bad
    } else if out.iter().any(|r| r.has_focus()) {
        crate::input::Edge::Focus
    } else if enabled && ui.rect_contains_pointer(rect) {
        crate::input::Edge::Hover
    } else {
        crate::input::Edge::Rest
    };
    let frame = crate::input::frame_shape(ui.ctx(), rect, id, edge);
    ui.painter().set(slot, if enabled { frame } else { faint(frame) });
    out
}

/// A frame shape at the faint strength of a control that is off.
fn faint(s: egui::Shape) -> egui::Shape {
    match s {
        egui::Shape::Vec(v) => egui::Shape::Vec(v.into_iter().map(faint).collect()),
        egui::Shape::Rect(mut r) => {
            r.fill = r.fill.gamma_multiply(tokens::OFF);
            r.stroke.color = r.stroke.color.gamma_multiply(tokens::OFF);
            egui::Shape::Rect(r)
        }
        other => other,
    }
}

/// One number as typed, kept to digits without leading zeros.
pub fn digits(typed: &str) -> String {
    let d: String = typed.chars().filter(|ch| ch.is_ascii_digit()).collect();
    match d.trim_start_matches('0') {
        "" if d.is_empty() => String::new(),
        "" => "0".to_string(),
        rest => rest.to_string(),
    }
}

/// The numbers of a box read against its parts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Read {
    /// Nothing typed yet.
    Empty,
    /// These parts are past their most (or do not read as a number).
    Over(Vec<bool>),
    /// The total, every part times its unit (an empty part is zero).
    Total(u64),
}

/// Read `cells` against `parts`, each `(most, unit)`: the most a part takes and what one of it is worth.
pub fn read(cells: &[String], parts: &[(u64, u64)]) -> Read {
    if cells.iter().all(|x| x.is_empty()) {
        return Read::Empty;
    }
    let n: Vec<Option<u64>> = cells.iter().map(|x| if x.is_empty() { Some(0) } else { x.parse().ok() }).collect();
    let over: Vec<bool> = parts.iter().enumerate().map(|(i, (most, _))| n.get(i).copied().flatten().is_none_or(|v| v > *most)).collect();
    if over.contains(&true) {
        return Read::Over(over);
    }
    Read::Total(parts.iter().enumerate().map(|(i, (_, unit))| n.get(i).copied().flatten().unwrap_or(0) * unit).sum())
}

#[cfg(test)]
mod tests {
    use super::{digits, read, Read};

    const DHM: [(u64, u64); 3] = [(999, 86_400), (23, 3_600), (59, 60)];

    fn cells(a: &str, b: &str, c: &str) -> Vec<String> {
        vec![a.to_string(), b.to_string(), c.to_string()]
    }

    #[test]
    fn a_number_keeps_its_digits_without_leading_zeros() {
        assert_eq!(digits(""), "");
        assert_eq!(digits("007"), "7");
        assert_eq!(digits("00"), "0");
        assert_eq!(digits("1a2"), "12");
    }

    #[test]
    fn the_parts_add_up_each_to_its_own_most() {
        assert_eq!(read(&cells("", "", ""), &DHM), Read::Empty);
        assert_eq!(read(&cells("", "", "1"), &DHM), Read::Total(60));
        assert_eq!(read(&cells("1", "2", "3"), &DHM), Read::Total(86_400 + 7_200 + 180));
        assert_eq!(read(&cells("999", "23", "59"), &DHM), Read::Total(999 * 86_400 + 23 * 3_600 + 59 * 60));
        assert_eq!(read(&cells("1000", "24", "1"), &DHM), Read::Over(vec![true, true, false]));
        assert_eq!(read(&cells("0", "0", "60"), &DHM), Read::Over(vec![false, false, true]));
        assert_eq!(read(&cells("0", "", ""), &DHM), Read::Total(0));
    }
}
