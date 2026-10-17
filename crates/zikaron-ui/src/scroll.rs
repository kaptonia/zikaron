//! Scroll areas, one way in. egui's `ScrollArea` spends only the part of the wheel it can still scroll and leaves
//! the rest in the frame's input, where an area around it reads it and scrolls too: at the end of a long list or
//! text inside a page, the page runs on and the reader loses their place. So every scroll area of this crate and
//! of the products built on it is shown through here, and says which kind it is:
//!
//! - [`page`]: the outermost area of a page (or of the rail, or of a whole-window column). It scrolls as egui does;
//!   nothing outside it scrolls.
//! - [`nested`] (and [`nested_rows`]): an area inside something else. While the pointer rests on it and it has more
//!   than it shows, it keeps the wheel: what it could not spend this frame is dropped, so the area around it stays
//!   where it is. An area that has nothing more to show keeps nothing, and the wheel with the pointer elsewhere is
//!   never its to keep (a focused field inside it does not hold the page still): the wheel goes on to the page.

/// The outermost area of a page: shown as egui shows it.
pub fn page<R>(ui: &mut egui::Ui, area: egui::ScrollArea, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    area.show(ui, add)
}

/// An area inside something that scrolls: it keeps the wheel while it is the one being scrolled.
pub fn nested<R>(ui: &mut egui::Ui, area: egui::ScrollArea, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    let out = area.show(ui, add);
    keep(ui, &out);
    out
}

/// [`nested`] for a long list drawn a screenful at a time (`ScrollArea::show_rows`).
pub fn nested_rows<R>(ui: &mut egui::Ui, area: egui::ScrollArea, row_h: f32, rows: usize, add: impl FnOnce(&mut egui::Ui, std::ops::Range<usize>) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    let out = area.show_rows(ui, row_h, rows, add);
    keep(ui, &out);
    out
}

/// Drop what is left of this frame's wheel when the area just shown is the one being scrolled: it has more than it
/// shows, and the pointer rests on it (on the part its parent lets show, in the layer on top there: an area under a
/// sheet keeps nothing of the sheet's wheel).
fn keep<R>(ui: &egui::Ui, out: &egui::scroll_area::ScrollAreaOutput<R>) {
    let shown = out.inner_rect;
    let more = out.content_size.x > shown.width() + 0.5 || out.content_size.y > shown.height() + 0.5;
    if more && ui.rect_contains_pointer(shown) {
        ui.ctx().input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
    }
}
