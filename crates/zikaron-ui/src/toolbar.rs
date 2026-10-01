//! The toolbar above each page: back and forward keys when the view has history, the page title, and the
//! page's own keys at the right. The title is never animated when the page changes (its words are simply the
//! page's; only the body below enters); on a detail page it stays hidden until the page's own large title
//! scrolls away. A hairline appears under the toolbar once the page scrolls.

use crate::button;
use crate::icons::Glyph;
use crate::motion;
use crate::palette::{c, C};
use crate::tokens::{self, Type};
use egui::{pos2, vec2, Rect};

/// What the toolbar's history keys did.
#[derive(Clone, Copy, Debug, Default)]
pub struct Nav {
    pub back: bool,
    pub fwd: bool,
}

/// Draw the toolbar into `rect` (56 high). `title_alpha` hides the title on a detail page until its large
/// title scrolls away; `scrolled` shows the hairline under it.
#[allow(clippy::too_many_arguments)]
pub fn toolbar<R>(
    ui: &mut egui::Ui,
    rect: Rect,
    title: &str,
    title_alpha: f32,
    history: (bool, bool),
    scrolled: bool,
    tips: (&str, &str),
    acts: impl FnOnce(&mut egui::Ui) -> R,
) -> (Nav, R) {
    let id = ui.id().with("zikaron-toolbar");
    let mut nav = Nav::default();
    let inner = Rect::from_min_max(pos2(rect.left() + 22.0, rect.top() + 10.0), pos2(rect.right() - tokens::PAGE_PAD, rect.bottom()));
    let mut x = inner.left();
    let cy = inner.center().y;
    if history.0 || history.1 {
        for (k, (g, on, tip)) in [(Glyph::Back, history.0, tips.0), (Glyph::Fwd, history.1, tips.1)].into_iter().enumerate() {
            let r = Rect::from_min_size(pos2(x, cy - 14.0), vec2(28.0, 28.0));
            let resp = ui.interact(r, id.with(("nav", k)), if on { egui::Sense::click() } else { egui::Sense::hover() });
            button::paint_icon_key(ui, r, &resp, g, on);
            if on && resp.clicked() {
                if k == 0 {
                    nav.back = true;
                } else {
                    nav.fwd = true;
                }
            }
            if on {
                let _ = resp.on_hover_text(tip).on_hover_cursor(egui::CursorIcon::PointingHand);
            }
            x += 28.0 + 2.0;
        }
        x += 2.0 + tokens::S3 - 2.0;
    }
    // The page's keys, right to left, first; the title takes what they leave.
    let acts_rect = Rect::from_min_max(pos2(x, inner.top()), inner.max);
    let mut aui = ui.new_child(egui::UiBuilder::new().max_rect(acts_rect).layout(egui::Layout::right_to_left(egui::Align::Center)));
    aui.spacing_mut().item_spacing.x = tokens::S2;
    let r = acts(&mut aui);
    let used = aui.min_rect();
    let title_right = if used.width() > 1.0 { used.left() - tokens::S3 } else { inner.right() };
    // Keyed by the words: a page's new title starts where it should be (no fade on changing page); only the
    // detail page's show-on-scroll fades.
    let fade = motion::to(ui.ctx(), id.with(("title-a", title)), title_alpha, tokens::FAST, motion::Curve::Ease);
    let room = (title_right - x).max(0.0);
    crate::paint::at(ui.painter(), ui, pos2(x, cy), egui::Align2::LEFT_CENTER, title, Type::Page, c(C::Ink).gamma_multiply(fade), room);
    let sep = motion::flag(ui.ctx(), id.with("sep"), scrolled, tokens::FAST);
    if sep > 0.0 {
        ui.painter().hline(rect.x_range(), rect.bottom() - 0.5, egui::Stroke::new(1.0_f32, c(C::Line2).gamma_multiply(sep)));
    }
    (nav, r)
}
