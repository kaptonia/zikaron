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

//!
//! Every area says here how far it has scrolled, so the library knows when something is scrolling ([`moving`]):
//! lists light no row under the pointer then, and rows do not slide.

/// The outermost area of a page: shown as egui shows it.
pub fn page<R>(ui: &mut egui::Ui, area: egui::ScrollArea, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    let out = area.show(ui, add);
    note(ui.ctx(), &out);
    out
}

/// An area inside something that scrolls: it keeps the wheel while it is the one being scrolled.
pub fn nested<R>(ui: &mut egui::Ui, area: egui::ScrollArea, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    let out = area.show(ui, add);
    note(ui.ctx(), &out);
    keep(ui, &out);
    out
}

/// [`nested`] for a long list drawn a screenful at a time (`ScrollArea::show_rows`).
pub fn nested_rows<R>(ui: &mut egui::Ui, area: egui::ScrollArea, row_h: f32, rows: usize, add: impl FnOnce(&mut egui::Ui, std::ops::Range<usize>) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    let out = area.show_rows(ui, row_h, rows, add);
    note(ui.ctx(), &out);
    keep(ui, &out);
    out
}

fn moved_id() -> egui::Id {
    egui::Id::new("zikaron-scroll-moved")
}

/// Every area shown this pass and the last: where it shows and how much further down it can still scroll.
#[derive(Clone, Default)]
struct Rooms {
    pass: u64,
    this: Vec<(egui::Rect, f32)>,
    last: Vec<(egui::Rect, f32)>,
}

fn rooms_id() -> egui::Id {
    egui::Id::new("zikaron-scroll-rooms")
}

/// Remember where an area stands; a change since last pass is a scroll this pass. Remember too how much further
/// down it can scroll ([`room_below`]).
fn note<R>(ctx: &egui::Context, out: &egui::scroll_area::ScrollAreaOutput<R>) {
    let id = out.id.with("zikaron-scroll-at");
    let at = out.state.offset;
    let pass = crate::motion::pass(ctx);
    let moved = ctx.data(|d| d.get_temp::<egui::Vec2>(id)).map(|p| (p - at).length() > 0.1).unwrap_or(false);
    let room = (out.content_size.y - out.inner_rect.height() - at.y).max(0.0);
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| {
        d.insert_temp(id, at);
        if moved {
            d.insert_temp(moved_id(), now);
        }
        let r = d.get_temp_mut_or_default::<Rooms>(rooms_id());
        if r.pass != pass {
            r.last = std::mem::take(&mut r.this);
            r.pass = pass;
        }
        r.this.push((out.inner_rect, room));
    });
}

/// How much further down the innermost area showing `at` can scroll (as of the last pass); none outside any.
pub fn room_below(ctx: &egui::Context, at: egui::Rect) -> f32 {
    let pass = crate::motion::pass(ctx);
    let r = ctx.data(|d| d.get_temp::<Rooms>(rooms_id())).unwrap_or_default();
    let list = if r.pass == pass { &r.last } else { &r.this };
    list.iter().filter(|(rect, _)| rect.contains(at.center())).min_by(|a, b| a.0.area().total_cmp(&b.0.area())).map(|x| x.1).unwrap_or(0.0)
}

/// Whether something is scrolling: wheel motion is being spent (egui spreads a turn over the next frames) or
/// still coming (a trackpad's gesture and its momentum, between their events too), or an area moved, within the
/// last [`SETTLE`] of the time. Settled only that long after the last of these, so the frames between two events
/// of one movement never read as settled.
pub fn moving(ctx: &egui::Context) -> bool {
    let (now, wheel) = ctx.input(|i| (i.time, i.smooth_scroll_delta != egui::Vec2::ZERO || i.is_scrolling()));
    let seen = egui::Id::new("zikaron-scroll-wheel-seen");
    if wheel {
        ctx.data_mut(|d| d.insert_temp(seen, now));
        return true;
    }
    let last = |id| ctx.data(|d| d.get_temp::<f64>(id)).is_some_and(|at| now >= at && now - at < SETTLE);
    last(seen) || last(moved_id())
}

/// How long after the last wheel event or move a scroll counts as settled.
pub const SETTLE: f64 = 0.15;

fn asked_id() -> egui::Id {
    egui::Id::new("zikaron-scroll-asked")
}

/// The window scrolls on its own this frame (bringing what opens into view, [`crate::fold::reveal`]): a move it makes
/// is not the person's.
pub fn asked(ctx: &egui::Context) {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.insert_temp(asked_id(), now));
}

/// Whether the person is scrolling: the wheel (or a trackpad and its momentum), or an area moved that the window did
/// not move itself, within the last [`SETTLE`]. A list keeps its rows' comings and goings through a scroll the
/// window makes ([`crate::flow::Group`]); the light under the pointer goes out for any scroll ([`moving`]).
pub fn by_hand(ctx: &egui::Context) -> bool {
    let (now, wheel) = ctx.input(|i| (i.time, i.smooth_scroll_delta != egui::Vec2::ZERO || i.is_scrolling()));
    if wheel {
        return true;
    }
    let at = |id| ctx.data(|d| d.get_temp::<f64>(id));
    let recent = |t: Option<f64>| t.is_some_and(|at| now >= at && now - at < SETTLE);
    recent(at(egui::Id::new("zikaron-scroll-wheel-seen"))) || (recent(at(moved_id())) && !recent(at(asked_id())))
}

/// Drop what is left of this frame's wheel when the area just shown is the one being scrolled: it has more than it
/// shows, and the pointer rests on it (on the part its parent lets show, in the layer on top there: an area under a
/// sheet keeps nothing of the sheet's wheel).
fn keep<R>(ui: &egui::Ui, out: &egui::scroll_area::ScrollAreaOutput<R>) {
    let shown = out.inner_rect;
    let more = !holds(out.content_size.x, shown.width()) || !holds(out.content_size.y, shown.height());
    if more && ui.rect_contains_pointer(shown) {
        ui.ctx().input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
    }
}

/// How far content may run past what shows and still count as shown whole: a layout's fractions of a point (a height
/// rounded to whole points, a re-measure that waits for half a point of change).
pub const SLACK: f32 = 0.5;

/// Whether `content` long fits in `shown`: the library's one judgment of "it has more than it shows", for keeping
/// the wheel ([`nested`]) and for whether a scroll bar shows at all (a sheet that holds its body whole shows none,
/// `crate::sheet::show`).
pub fn holds(content: f32, shown: f32) -> bool {
    content <= shown + SLACK
}
