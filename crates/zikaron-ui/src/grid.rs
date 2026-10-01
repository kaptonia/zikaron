//! Equal grid: cells side by side with equal widths; pages lay out side-by-side blocks only through here.
//!
//! egui's `ui.columns` gives each column a `top_down_justified` layout, so text in it is justified: long
//! strings (dates, addresses, digests) wrap and stretch to the column width, and columns sized by their own
//! content make a row of cards uneven. Here each cell uses `top_down(Align::Min)` without justification, text
//! never wraps (controls elide by measured width, see [`crate::width::elide_to`]), and cell height is the
//! tallest cell of the row, measured in the previous frame and applied in this one (another frame is
//! requested when it changes). Height is taken only from the measured row height, never from the available
//! height: the available height follows the outer frame (a centered card grows with content), and taking it
//! would oscillate frame to frame. Rows depend only on width: each row holds as many cells as fit at
//! [`crate::tokens::GRID3_MIN_W`], the rest wrap, and each row has its own equal height.
//!
//! No ledger-protocol decision is made here.

const TARGET: &str = "zikaron-grid-cell-h";

/// How many cells per row: how many [`crate::tokens::GRID3_MIN_W`]-wide cells (with `gap`) fit in width `w`;
/// at least one, at most `n`.
pub fn per_row(w: f32, n: usize, gap: f32) -> usize {
    per_row_min(w, n, gap, crate::tokens::GRID3_MIN_W)
}

/// As [`per_row`] with the least cell width given.
pub fn per_row_min(w: f32, n: usize, gap: f32, min_w: f32) -> usize {
    let fit = ((w + gap) / (min_w + gap)).floor();
    let fit = if fit.is_finite() && fit >= 1.0 { fit as usize } else { 1 };
    fit.clamp(1, n.max(1))
}

/// `n` cells side by side, wrapping when narrow: cells per row from [`per_row`], each row's width split
/// equally, cell gap `item_spacing.x`, row gap `item_spacing.y`. Each row has one height (the tallest cell
/// measured last frame; another frame is requested on change). `cell(ui, i)` draws cell `i`; returns each
/// cell's answer.
pub fn tiles<R>(ui: &mut egui::Ui, id_salt: &str, n: usize, cell: impl FnMut(&mut egui::Ui, usize) -> R) -> Vec<R> {
    tiles_min(ui, id_salt, n, crate::tokens::GRID3_MIN_W, cell)
}

/// As [`tiles`] with the least cell width given (two-across blocks that stack when narrow).
pub fn tiles_min<R>(ui: &mut egui::Ui, id_salt: &str, n: usize, min_w: f32, mut cell: impl FnMut(&mut egui::Ui, usize) -> R) -> Vec<R> {
    let n = n.max(1);
    let gap = ui.spacing().item_spacing.x;
    let gap_y = ui.spacing().item_spacing.y;
    let w = ui.available_width().max(0.0);
    let across = per_row_min(w, n, gap, min_w);
    // Cells land on whole pixels (width floored, left and top edges rounded): egui warns about unaligned
    // controls in debug builds and edges blur.
    let cw = ((w - gap * (across as f32 - 1.0)) / across as f32).max(0.0).floor();
    let id = ui.id().with(("zikaron-tiles", id_salt));
    let top = ui.cursor().min;
    let mut y = top.y;
    let mut out = Vec::with_capacity(n);
    let mut cells: Vec<egui::Rect> = Vec::with_capacity(n);
    let rows = n.div_ceil(across);
    for row in 0..rows {
        let rid = id.with(row);
        let tall = ui.ctx().data(|d| d.get_temp::<f32>(rid)).unwrap_or(0.0);
        let mut highest = 0.0_f32;
        let mut row_h = 0.0_f32;
        for j in 0..across {
            let i = row * across + j;
            if i >= n {
                break;
            }
            let x = (top.x + (cw + gap) * j as f32).round();
            // Cell height comes only from the row height measured last frame (the available height follows
            // the outer frame and would oscillate).
            let room = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, tall.clamp(1.0, 100_000.0)));
            let mut child = ui.new_child(egui::UiBuilder::new().id_salt((id_salt, i)).max_rect(room).layout(egui::Layout::top_down(egui::Align::Min)));
            child.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            child.set_width(cw);
            child.ctx().data_mut(|d| d.insert_temp(egui::Id::new(TARGET), tall));
            out.push(cell(&mut child, i));
            // Measure what the cell contains, then stretch the cell to the row height.
            highest = highest.max(child.min_rect().height());
            child.expand_to_include_rect(egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, tall)));
            let r = child.min_rect();
            row_h = row_h.max(r.height());
            cells.push(egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, r.height())));
        }
        if (highest - tall).abs() > 0.5 {
            ui.ctx().data_mut(|d| d.insert_temp(rid, highest));
            // Laid out again at once with the new size, so no frame is shown placed by the old one.
            ui.ctx().request_discard("grid row height changed");
        }
        // Row heights are rounded but the top is not, so the total height depends only on the rows, not on
        // where in the window the grid starts. Rounding absolute positions would change the total by a pixel
        // when a centered card moves half a pixel, and the card would change height frame to frame.
        y += row_h.round();
        if row + 1 < rows {
            y += gap_y.round();
        }
    }
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(TARGET), 0.0_f32));
    ui.allocate_rect(egui::Rect::from_min_size(top, egui::vec2(w, y - top.y)), egui::Sense::hover());
    for r in cells {
        crate::probe::cell(ui.ctx(), r);
    }
    out
}

/// The height the current cell should have (its row's tallest cell last frame; 0 outside a grid). Cards in a
/// cell stretch to it.
pub fn cell_height(ui: &egui::Ui) -> f32 {
    ui.ctx().data(|d| d.get_temp::<f32>(egui::Id::new(TARGET))).unwrap_or(0.0)
}
