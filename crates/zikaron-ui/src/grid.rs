const TARGET: &str = "zikaron-grid-cell-h";

pub fn per_row(w: f32, n: usize, gap: f32) -> usize {
    per_row_min(w, n, gap, crate::tokens::GRID3_MIN_W)
}

pub fn per_row_min(w: f32, n: usize, gap: f32, min_w: f32) -> usize {
    let fit = ((w + gap) / (min_w + gap)).floor();
    let fit = if fit.is_finite() && fit >= 1.0 { fit as usize } else { 1 };
    fit.clamp(1, n.max(1))
}

pub fn tiles<R>(ui: &mut egui::Ui, id_salt: &str, n: usize, cell: impl FnMut(&mut egui::Ui, usize) -> R) -> Vec<R> {
    tiles_min(ui, id_salt, n, crate::tokens::GRID3_MIN_W, cell)
}

pub fn tiles_min<R>(ui: &mut egui::Ui, id_salt: &str, n: usize, min_w: f32, mut cell: impl FnMut(&mut egui::Ui, usize) -> R) -> Vec<R> {
    let n = n.max(1);
    let gap = ui.spacing().item_spacing.x;
    let gap_y = ui.spacing().item_spacing.y;
    let w = ui.available_width().max(0.0);
    let across = per_row_min(w, n, gap, min_w);
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
            let room = egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, tall.clamp(1.0, 100_000.0)));
            let mut child = ui.new_child(egui::UiBuilder::new().id_salt((id_salt, i)).max_rect(room).layout(egui::Layout::top_down(egui::Align::Min)));
            child.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            child.set_width(cw);
            child.ctx().data_mut(|d| d.insert_temp(egui::Id::new(TARGET), tall));
            out.push(cell(&mut child, i));
            highest = highest.max(child.min_rect().height());
            child.expand_to_include_rect(egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, tall)));
            let r = child.min_rect();
            row_h = row_h.max(r.height());
            cells.push(egui::Rect::from_min_size(egui::pos2(x, y), egui::vec2(cw, r.height())));
        }
        if (highest - tall).abs() > 0.5 {
            ui.ctx().data_mut(|d| d.insert_temp(rid, highest));
            ui.ctx().request_discard("grid row height changed");
        }
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

pub fn cell_height(ui: &egui::Ui) -> f32 {
    ui.ctx().data(|d| d.get_temp::<f32>(egui::Id::new(TARGET))).unwrap_or(0.0)
}
