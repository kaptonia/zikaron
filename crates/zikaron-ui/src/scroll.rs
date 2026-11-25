pub fn page<R>(ui: &mut egui::Ui, area: egui::ScrollArea, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    let out = area.show(ui, add);
    clamp_now(ui.ctx(), &out);
    note(ui.ctx(), &out);
    out
}

fn clamp_now<R>(ctx: &egui::Context, out: &egui::scroll_area::ScrollAreaOutput<R>) {
    let laid = ctx.data(|d| d.get_temp::<egui::Vec2>(out.id.with("zikaron-scroll-at")));
    let most = (out.content_size.y - out.inner_rect.height()).max(0.0);
    if laid.is_some_and(|at| at.y > most + 0.5) && out.state.offset.y <= most + 0.5 {
        ctx.request_discard("a page grew shorter");
    }
}

pub fn nested<R>(ui: &mut egui::Ui, area: egui::ScrollArea, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    let out = area.show(ui, add);
    note(ui.ctx(), &out);
    keep(ui, &out);
    out
}

pub fn nested_rows<R>(ui: &mut egui::Ui, area: egui::ScrollArea, row_h: f32, rows: usize, add: impl FnOnce(&mut egui::Ui, std::ops::Range<usize>) -> R) -> egui::scroll_area::ScrollAreaOutput<R> {
    let out = area.show_rows(ui, row_h, rows, add);
    note(ui.ctx(), &out);
    keep(ui, &out);
    out
}

fn moved_id() -> egui::Id {
    egui::Id::new("zikaron-scroll-moved")
}

#[derive(Clone, Default)]
struct Rooms {
    pass: u64,
    this: Vec<(egui::Rect, f32)>,
    last: Vec<(egui::Rect, f32)>,
}

fn rooms_id() -> egui::Id {
    egui::Id::new("zikaron-scroll-rooms")
}

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

pub fn room_below(ctx: &egui::Context, at: egui::Rect) -> f32 {
    let pass = crate::motion::pass(ctx);
    let r = ctx.data(|d| d.get_temp::<Rooms>(rooms_id())).unwrap_or_default();
    let list = if r.pass == pass { &r.last } else { &r.this };
    list.iter().filter(|(rect, _)| rect.contains(at.center())).min_by(|a, b| a.0.area().total_cmp(&b.0.area())).map(|x| x.1).unwrap_or(0.0)
}

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

pub const SETTLE: f64 = 0.15;

fn asked_id() -> egui::Id {
    egui::Id::new("zikaron-scroll-asked")
}

pub fn asked(ctx: &egui::Context) {
    let now = ctx.input(|i| i.time);
    ctx.data_mut(|d| d.insert_temp(asked_id(), now));
}

pub fn by_hand(ctx: &egui::Context) -> bool {
    let (now, wheel) = ctx.input(|i| (i.time, i.smooth_scroll_delta != egui::Vec2::ZERO || i.is_scrolling()));
    if wheel {
        return true;
    }
    let at = |id| ctx.data(|d| d.get_temp::<f64>(id));
    let recent = |t: Option<f64>| t.is_some_and(|at| now >= at && now - at < SETTLE);
    recent(at(egui::Id::new("zikaron-scroll-wheel-seen"))) || (recent(at(moved_id())) && !recent(at(asked_id())))
}

fn keep<R>(ui: &egui::Ui, out: &egui::scroll_area::ScrollAreaOutput<R>) {
    let shown = out.inner_rect;
    let more = !holds(out.content_size.x, shown.width()) || !holds(out.content_size.y, shown.height());
    if more && ui.rect_contains_pointer(shown) {
        ui.ctx().input_mut(|i| i.smooth_scroll_delta = egui::Vec2::ZERO);
    }
}

pub const SLACK: f32 = 0.5;

pub fn holds(content: f32, shown: f32) -> bool {
    content <= shown + SLACK
}
