//! A sheet's title with its tabs on the same line: the title at the left, the tabs at the right end, each as
//! wide as its words and 16 apart, no line under the row. The chosen tab is in the body ink and heavier, a
//! two-point blue line under its words that slides to the tab chosen next.

use crate::motion::{self, Curve};
use crate::palette::{c, C};
use crate::tokens::{self, Type};
use egui::{pos2, vec2, Rect};

/// The title and the tabs; returns the tab clicked when it is not the current one.
pub fn title_tabs(ui: &mut egui::Ui, title: &str, id_salt: &str, labels: &[&str], current: usize) -> Option<usize> {
    let id = ui.id().with(("zikaron-head-tabs", id_salt));
    let w = ui.available_width();
    let h = Type::Sheet.line();
    let (rect, _) = ui.allocate_exact_size(vec2(w, h), egui::Sense::hover());
    let font_on = egui::FontId::new(Type::Body.size(), crate::fonts::strong());
    let widths: Vec<f32> = labels.iter().map(|l| ui.painter().layout_no_wrap(l.to_string(), font_on.clone(), egui::Color32::BLACK).size().x + 2.0).collect();
    let total = widths.iter().sum::<f32>() + tokens::S4 * labels.len().saturating_sub(1) as f32;
    let mut x = rect.right() - total;
    let tabs_left = x;
    let mut lefts = Vec::new();
    for tw in &widths {
        lefts.push(x);
        x += tw + tokens::S4;
    }
    let p = ui.painter().clone();
    let room = (tabs_left - tokens::S4 - rect.left()).max(0.0);
    let fit = crate::width::elide_to(ui, title, Type::Sheet.font(), room);
    p.text(pos2(rect.left(), rect.center().y), egui::Align2::LEFT_CENTER, fit, Type::Sheet.font(), c(C::Ink));
    let cur = current.min(labels.len().saturating_sub(1));
    let ul = motion::glide(ui.ctx(), id.with("ul-x"), lefts.get(cur).copied().unwrap_or(rect.right()) - rect.left(), tokens::MID, Curve::Ease);
    let uw = motion::glide(ui.ctx(), id.with("ul-w"), widths.get(cur).copied().unwrap_or(0.0), tokens::MID, Curve::Ease);
    let mut hit = None;
    for (i, l) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(lefts[i], rect.top()), vec2(widths[i], h));
        let resp = ui.interact(r, id.with(i), egui::Sense::click());
        let on = i == cur;
        if resp.clicked() && !on {
            hit = Some(i);
        }
        let hot = motion::states(ui.ctx(), id.with(("tab", i)), on || resp.hovered(), resp.is_pointer_button_down_on(), resp.has_focus(), true).hover;
        let colour = crate::palette::mix(c(C::Ink2), c(C::Ink), hot);
        let font = if on { font_on.clone() } else { Type::Body.font() };
        p.text(pos2(r.left() + 1.0, r.center().y), egui::Align2::LEFT_CENTER, *l, font, colour);
        if on {
            // Already chosen: nothing to press.
            crate::hand::sink(ui.ctx(), resp.id);
        }
    }
    if !labels.is_empty() {
        let y = rect.center().y + Type::Body.line() / 2.0 + 1.0;
        p.rect_filled(Rect::from_min_size(pos2(rect.left() + ul, y), vec2(uw, 2.0)), egui::CornerRadius::same(1), c(C::Accent));
    }
    ui.add_space((tokens::S4 - ui.spacing().item_spacing.y).max(0.0));
    hit
}
