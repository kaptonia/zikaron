const ROWS: &str = "zikaron-probe-rows";
const INNER: &str = "zikaron-probe-inner";
const HEAD: &str = "zikaron-probe-inner-head";
const HEAD_KEYS: &str = "zikaron-probe-head-keys";
const CELLS: &str = "zikaron-probe-cells";
const TILES: &str = "zikaron-probe-tiles";
const ROUTE: &str = "zikaron-probe-route";
const HISTORY: &str = "zikaron-probe-history";
const QR: &str = "zikaron-probe-qr";
const PARTS: &str = "zikaron-probe-parts";
const BOXES: &str = "zikaron-probe-boxes";
const LITS: &str = "zikaron-probe-lits";
const SPOTS: &str = "zikaron-probe-spots";
const SHEETS: &str = "zikaron-probe-sheets";
const GREY_KEYS: &str = "zikaron-probe-grey-keys";
const CUT_SHORT: &str = "zikaron-probe-cut-short";
const WHOLE_ON_HOVER: &str = "zikaron-probe-whole-on-hover";
const TABLES: &str = "zikaron-probe-tables";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lit {
    pub block: egui::Rect,
    pub corners: egui::CornerRadius,
    pub slot: egui::Rect,
    pub card: egui::Rect,
    pub card_corner: f32,
}

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
        d.insert_temp(egui::Id::new(PARTS), Vec::<(egui::LayerId, egui::Rect)>::new());
        d.insert_temp(egui::Id::new(BOXES), Vec::<(egui::LayerId, egui::Rect)>::new());
        d.insert_temp(egui::Id::new(LITS), Vec::<Lit>::new());
        d.insert_temp(egui::Id::new(SPOTS), Vec::<(String, egui::Rect)>::new());
        d.insert_temp(egui::Id::new(SHEETS), Vec::<Sheet>::new());
        d.insert_temp(egui::Id::new(GREY_KEYS), Vec::<(egui::LayerId, String)>::new());
        d.insert_temp(egui::Id::new(CUT_SHORT), Vec::<(egui::LayerId, String)>::new());
        d.insert_temp(egui::Id::new(WHOLE_ON_HOVER), Vec::<String>::new());
        d.insert_temp(egui::Id::new(TABLES), Vec::<Took>::new());
    });
}

pub fn inner_head(ctx: &egui::Context, label: &str) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(HEAD), Some(label.to_string())));
}

pub fn head_keys(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(HEAD_KEYS), true));
}

pub fn inner_head_said(ctx: &egui::Context) -> Option<String> {
    ctx.data(|d| d.get_temp::<Option<String>>(egui::Id::new(HEAD))).flatten()
}

pub fn head_keys_shown(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new(HEAD_KEYS))).unwrap_or(false)
}

pub fn row(ctx: &egui::Context, rect: egui::Rect) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<egui::Rect>>(egui::Id::new(ROWS)).push(rect));
}

pub fn inner(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(INNER), true));
}

pub fn rows(ctx: &egui::Context) -> Vec<egui::Rect> {
    ctx.data(|d| d.get_temp::<Vec<egui::Rect>>(egui::Id::new(ROWS))).unwrap_or_default()
}

pub fn inner_shown(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new(INNER))).unwrap_or(false)
}

pub fn cell(ctx: &egui::Context, rect: egui::Rect) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<egui::Rect>>(egui::Id::new(CELLS)).push(rect));
}

pub fn cells(ctx: &egui::Context) -> Vec<egui::Rect> {
    ctx.data(|d| d.get_temp::<Vec<egui::Rect>>(egui::Id::new(CELLS))).unwrap_or_default()
}

pub fn tile(ctx: &egui::Context, rect: egui::Rect, title_whole: bool) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<(egui::Rect, bool)>>(egui::Id::new(TILES)).push((rect, title_whole)));
}

pub fn tiles(ctx: &egui::Context) -> Vec<(egui::Rect, bool)> {
    ctx.data(|d| d.get_temp::<Vec<(egui::Rect, bool)>>(egui::Id::new(TILES))).unwrap_or_default()
}

pub fn route(ctx: &egui::Context, name: &str) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(ROUTE), name.to_string()));
}

pub fn route_drawn(ctx: &egui::Context) -> String {
    ctx.data(|d| d.get_temp::<String>(egui::Id::new(ROUTE))).unwrap_or_default()
}

pub fn history(ctx: &egui::Context, back: bool, fwd: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(HISTORY), (back, fwd)));
}

pub fn history_shown(ctx: &egui::Context) -> (bool, bool) {
    ctx.data(|d| d.get_temp::<(bool, bool)>(egui::Id::new(HISTORY))).unwrap_or((false, false))
}

pub fn qr(ctx: &egui::Context, plate: egui::Rect) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<egui::Rect>>(egui::Id::new(QR)).push(plate));
}

pub fn qrs(ctx: &egui::Context) -> Vec<egui::Rect> {
    ctx.data(|d| d.get_temp::<Vec<egui::Rect>>(egui::Id::new(QR))).unwrap_or_default()
}

pub fn part(ctx: &egui::Context, layer: egui::LayerId, rect: egui::Rect) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<(egui::LayerId, egui::Rect)>>(egui::Id::new(PARTS)).push((layer, rect)));
}

pub fn boxed(ctx: &egui::Context, layer: egui::LayerId, rect: egui::Rect) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<(egui::LayerId, egui::Rect)>>(egui::Id::new(BOXES)).push((layer, rect)));
}

pub fn parts(ctx: &egui::Context) -> Vec<(egui::LayerId, egui::Rect)> {
    ctx.data(|d| d.get_temp::<Vec<(egui::LayerId, egui::Rect)>>(egui::Id::new(PARTS))).unwrap_or_default()
}

pub fn boxes(ctx: &egui::Context) -> Vec<(egui::LayerId, egui::Rect)> {
    ctx.data(|d| d.get_temp::<Vec<(egui::LayerId, egui::Rect)>>(egui::Id::new(BOXES))).unwrap_or_default()
}

pub fn lit(ctx: &egui::Context, l: Lit) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<Lit>>(egui::Id::new(LITS)).push(l));
}

pub fn lits(ctx: &egui::Context) -> Vec<Lit> {
    ctx.data(|d| d.get_temp::<Vec<Lit>>(egui::Id::new(LITS))).unwrap_or_default()
}

pub fn spot(ctx: &egui::Context, name: &str, rect: egui::Rect) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<(String, egui::Rect)>>(egui::Id::new(SPOTS)).push((name.to_string(), rect)));
}

pub fn spots(ctx: &egui::Context) -> Vec<(String, egui::Rect)> {
    ctx.data(|d| d.get_temp::<Vec<(String, egui::Rect)>>(egui::Id::new(SPOTS))).unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sheet {
    pub id: String,
    pub card: egui::Rect,
    pub content: f32,
    pub room: f32,
    pub bar: bool,
}

pub fn sheet(ctx: &egui::Context, s: Sheet) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<Sheet>>(egui::Id::new(SHEETS)).push(s));
}

pub fn sheets(ctx: &egui::Context) -> Vec<Sheet> {
    ctx.data(|d| d.get_temp::<Vec<Sheet>>(egui::Id::new(SHEETS))).unwrap_or_default()
}

#[derive(Clone, Debug, PartialEq)]
pub struct FootOut {
    pub id: String,
    pub row: egui::Rect,
    pub laid: egui::Rect,
}

pub fn overruns(row: egui::Rect, laid: egui::Rect) -> bool {
    const SLACK: f32 = 0.5;
    laid.left() < row.left() - SLACK || laid.right() > row.right() + SLACK || laid.height() > row.height() + SLACK
}

pub fn overlap(keys: &[egui::Rect]) -> bool {
    keys.iter().enumerate().any(|(i, a)| keys[i + 1..].iter().any(|b| a.shrink(0.5).intersects(b.shrink(0.5))))
}

static FEET_OUT: std::sync::Mutex<Vec<FootOut>> = std::sync::Mutex::new(Vec::new());

const FEET_KEPT: usize = 64;

pub fn foot_out(f: FootOut) {
    let mut v = FEET_OUT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if v.len() < FEET_KEPT && !v.contains(&f) {
        v.push(f);
    }
}

pub fn feet_out() -> Vec<FootOut> {
    FEET_OUT.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

#[cfg(test)]
mod foot_tests {
    use super::*;

    #[test]
    fn a_foot_laid_past_its_row_is_out() {
        let row = egui::Rect::from_min_max(egui::pos2(100.0, 500.0), egui::pos2(572.0, 536.0));
        let keys = egui::Rect::from_min_max(egui::pos2(380.0, 500.0), egui::pos2(572.0, 536.0));
        assert!(!overruns(row, keys), "two keys at the right");
        assert!(!overruns(row, row), "keys from side to side");
        let past = egui::Rect::from_min_max(egui::pos2(-112.0, 500.0), egui::pos2(572.0, 536.0));
        assert!(overruns(row, past), "a block as wide as the row and a key after it");
        let tall = egui::Rect::from_min_max(egui::pos2(100.0, 470.0), egui::pos2(572.0, 566.0));
        assert!(overruns(row, tall), "a note taller than a key");
        let (a, b) = (egui::Rect::from_min_max(egui::pos2(380.0, 500.0), egui::pos2(470.0, 536.0)), egui::Rect::from_min_max(egui::pos2(478.0, 500.0), egui::pos2(572.0, 536.0)));
        assert!(!overlap(&[a, b]), "two keys side by side, a gap between");
        assert!(!overlap(&[a, b.translate(egui::vec2(-8.0, 0.0))]), "two keys touching");
        assert!(overlap(&[a, b.translate(egui::vec2(-40.0, 0.0))]), "a key laid over another");
    }
}

pub fn grey_key(ctx: &egui::Context, layer: egui::LayerId, text: &str) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<(egui::LayerId, String)>>(egui::Id::new(GREY_KEYS)).push((layer, text.to_string())));
}

pub fn grey_keys(ctx: &egui::Context) -> Vec<(egui::LayerId, String)> {
    ctx.data(|d| d.get_temp::<Vec<(egui::LayerId, String)>>(egui::Id::new(GREY_KEYS))).unwrap_or_default()
}

pub fn cut_short(ctx: &egui::Context, layer: egui::LayerId, whole: &str) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<(egui::LayerId, String)>>(egui::Id::new(CUT_SHORT)).push((layer, whole.to_string())));
}

pub fn whole_on_hover(ctx: &egui::Context, whole: &str) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<String>>(egui::Id::new(WHOLE_ON_HOVER)).push(whole.to_string()));
}

pub fn wholes_on_hover(ctx: &egui::Context) -> Vec<String> {
    ctx.data(|d| d.get_temp::<Vec<String>>(egui::Id::new(WHOLE_ON_HOVER))).unwrap_or_default()
}

pub fn cuts_short(ctx: &egui::Context) -> Vec<(egui::LayerId, String)> {
    ctx.data(|d| d.get_temp::<Vec<(egui::LayerId, String)>>(egui::Id::new(CUT_SHORT))).unwrap_or_default()
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Took {
    pub salt: String,
    pub rows: usize,
    pub drawn: usize,
    pub measured: usize,
}

pub fn took(ctx: &egui::Context, t: Took) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<Took>>(egui::Id::new(TABLES)).push(t));
}

pub fn tooks(ctx: &egui::Context) -> Vec<Took> {
    ctx.data(|d| d.get_temp::<Vec<Took>>(egui::Id::new(TABLES))).unwrap_or_default()
}
