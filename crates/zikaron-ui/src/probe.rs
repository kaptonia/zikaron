//! Observation: where this frame's clickable list rows are, which page was drawn, and whether a detail page
//! and its history keys are there.
//! Tests read it after a headless frame; no decision is made here. The shipped app records it too
//! (a few rectangles per frame) with no reader.

const ROWS: &str = "zikaron-probe-rows";
const INNER: &str = "zikaron-probe-inner";
const HEAD: &str = "zikaron-probe-inner-head";
const HEAD_KEYS: &str = "zikaron-probe-head-keys";
const CELLS: &str = "zikaron-probe-cells";
const TILES: &str = "zikaron-probe-tiles";
const ROUTE: &str = "zikaron-probe-route";
const HISTORY: &str = "zikaron-probe-history";
const QR: &str = "zikaron-probe-qr";

/// Clear at the start of a frame.
pub fn begin(ctx: &egui::Context) {
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new(ROWS), Vec::<egui::Rect>::new());
        d.insert_temp(egui::Id::new(INNER), false);
        d.insert_temp(egui::Id::new(HEAD), None::<String>);
        d.insert_temp(egui::Id::new(HEAD_KEYS), false);
        d.insert_temp(egui::Id::new(CELLS), Vec::<egui::Rect>::new());
        d.insert_temp(egui::Id::new(TILES), Vec::<(egui::Rect, bool)>::new());
        d.insert_temp(egui::Id::new(ROUTE), String::new());
        d.insert_temp(egui::Id::new(HISTORY), (false, false));
        d.insert_temp(egui::Id::new(QR), Vec::<egui::Rect>::new());
    });
}

/// The text on this frame's inner-page back key.
pub fn inner_head(ctx: &egui::Context, label: &str) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(HEAD), Some(label.to_string())));
}

/// This frame's header drew the filter band and primary key of a list page.
pub fn head_keys(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(HEAD_KEYS), true));
}

/// The text on this frame's inner-page back key (`None` when absent).
pub fn inner_head_said(ctx: &egui::Context) -> Option<String> {
    ctx.data(|d| d.get_temp::<Option<String>>(egui::Id::new(HEAD))).flatten()
}

/// Whether this frame's header drew list header keys.
pub fn head_keys_shown(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new(HEAD_KEYS))).unwrap_or(false)
}

/// One clickable list row.
pub fn row(ctx: &egui::Context, rect: egui::Rect) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<egui::Rect>>(egui::Id::new(ROWS)).push(rect));
}

/// This frame drew an inner page (the back key is there).
pub fn inner(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(INNER), true));
}

/// The list rows recorded this frame.
pub fn rows(ctx: &egui::Context) -> Vec<egui::Rect> {
    ctx.data(|d| d.get_temp::<Vec<egui::Rect>>(egui::Id::new(ROWS))).unwrap_or_default()
}

/// Whether this frame drew an inner page.
pub fn inner_shown(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new(INNER))).unwrap_or(false)
}

/// One equal-grid cell: its rectangle. Justification is not recorded here (the layout is up to the controls,
/// and it would always read false); tests count it from this frame's text jobs (`job.justify`).
pub fn cell(ctx: &egui::Context, rect: egui::Rect) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<egui::Rect>>(egui::Id::new(CELLS)).push(rect));
}

/// The grid cells recorded this frame.
pub fn cells(ctx: &egui::Context) -> Vec<egui::Rect> {
    ctx.data(|d| d.get_temp::<Vec<egui::Rect>>(egui::Id::new(CELLS))).unwrap_or_default()
}

/// Where a dashboard tile was drawn and whether its title fit (true when not elided).
pub fn tile(ctx: &egui::Context, rect: egui::Rect, title_whole: bool) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<(egui::Rect, bool)>>(egui::Id::new(TILES)).push((rect, title_whole)));
}

/// The dashboard tiles recorded this frame.
pub fn tiles(ctx: &egui::Context) -> Vec<(egui::Rect, bool)> {
    ctx.data(|d| d.get_temp::<Vec<(egui::Rect, bool)>>(egui::Id::new(TILES))).unwrap_or_default()
}

/// The page drawn this frame, by its route name.
pub fn route(ctx: &egui::Context, name: &str) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(ROUTE), name.to_string()));
}

/// The page drawn this frame (empty when none was recorded).
pub fn route_drawn(ctx: &egui::Context) -> String {
    ctx.data(|d| d.get_temp::<String>(egui::Id::new(ROUTE))).unwrap_or_default()
}

/// Whether the toolbar showed a back key and a forward key that can be pressed this frame.
pub fn history(ctx: &egui::Context, back: bool, fwd: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(HISTORY), (back, fwd)));
}

pub fn history_shown(ctx: &egui::Context) -> (bool, bool) {
    ctx.data(|d| d.get_temp::<(bool, bool)>(egui::Id::new(HISTORY))).unwrap_or((false, false))
}

/// Where a QR code's plate was drawn.
pub fn qr(ctx: &egui::Context, plate: egui::Rect) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<egui::Rect>>(egui::Id::new(QR)).push(plate));
}

/// The QR plates recorded this frame.
pub fn qrs(ctx: &egui::Context) -> Vec<egui::Rect> {
    ctx.data(|d| d.get_temp::<Vec<egui::Rect>>(egui::Id::new(QR))).unwrap_or_default()
}
