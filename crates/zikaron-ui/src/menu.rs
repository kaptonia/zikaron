use crate::icons::{self, Glyph};
use crate::mark;
use crate::motion::{self, Curve};
use crate::paint;
use crate::palette::{c, Tone, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

#[derive(Clone, Copy, Debug, Default)]
pub struct Row<'a> {
    pub label: &'a str,
    pub lead: &'a str,
    pub mono: bool,
    pub sub: &'a str,
    pub sub_ok: bool,
    pub check: Option<bool>,
    pub tick_right: bool,
    pub pill: Option<(&'a str, Tone)>,
    pub trail: &'a str,
    pub struck: bool,
    pub disabled: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Who<'a> {
    pub name: &'a str,
    pub kind: &'a str,
    pub tags: &'a [&'a str],
    pub current: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Item<'a> {
    Head(&'a str),
    Sep,
    Row(Row<'a>),
    Who(Who<'a>),
}

pub const WHO_H: f32 = 48.0;

const PAD: f32 = crate::band::GAP;
const SEP_H: f32 = 2.0 * PAD + 1.0;

pub fn row(label: &str) -> Item<'_> {
    Item::Row(Row { label, ..Default::default() })
}

fn open_id(id: egui::Id) -> egui::Id {
    id.with("open")
}

pub fn is_open(ctx: &egui::Context, id: egui::Id) -> bool {
    ctx.data(|d| d.get_temp::<bool>(open_id(id))).unwrap_or(false)
}

pub fn toggle(ctx: &egui::Context, id: egui::Id) {
    let now = is_open(ctx, id);
    set(ctx, id, !now);
}

pub fn set(ctx: &egui::Context, id: egui::Id, open: bool) {
    ctx.data_mut(|d| d.insert_temp(open_id(id), open));
    if open {
        motion::restart(ctx, id.with("age"));
        ctx.data_mut(|d| {
            let n = d.get_temp::<u64>(id.with("opened")).unwrap_or(0) + 1;
            d.insert_temp(id.with("opened"), n);
            d.remove::<usize>(id.with("keyed"));
        });
    }
}

pub fn show(ctx: &egui::Context, id: egui::Id, anchor: Rect, left: bool, min_w: f32, items: &[Item]) -> Option<usize> {
    show_from(ctx, None, id, anchor, ctx.content_rect(), left, min_w, items)
}

#[allow(clippy::too_many_arguments)]
pub fn show_from(ctx: &egui::Context, parent: Option<egui::LayerId>, id: egui::Id, anchor: Rect, bounds: Rect, left: bool, min_w: f32, items: &[Item]) -> Option<usize> {
    if !is_open(ctx, id) {
        return None;
    }
    let e = crate::layer::entrance(ctx, &[id.with("keep")], id.with("age"), tokens::FAST);
    let row_w = |it: &Item| -> f32 {
        match it {
            Item::Row(r) => {
                let f = if r.mono { egui::FontId::new(14.0, egui::FontFamily::Monospace) } else { Type::Key.font() };
                let mut w = ctx.fonts_mut(|fo| fo.layout_no_wrap(r.label.to_string(), f, egui::Color32::BLACK).size().x) + 24.0;
                if !r.lead.is_empty() {
                    w += 36.0;
                }
                if r.check.is_some() {
                    w += 22.0;
                }
                if let Some((p, _)) = r.pill {
                    w += ctx.fonts_mut(|fo| fo.layout_no_wrap(p.to_string(), Type::Small.font(), egui::Color32::BLACK).size().x) + 32.0;
                }
                if !r.trail.is_empty() || r.tick_right {
                    w += 40.0;
                }
                w
            }
            Item::Who(wr) => {
                let tags = if wr.tags.is_empty() { String::new() } else { format!(" \u{b7} {}", wr.tags.join(" \u{b7} ")) };
                let a = ctx.fonts_mut(|fo| fo.layout_no_wrap(wr.name.to_string(), Type::Key.font(), egui::Color32::BLACK).size().x);
                let b = ctx.fonts_mut(|fo| fo.layout_no_wrap(format!("{}{tags}", wr.kind), Type::Small.font(), egui::Color32::BLACK).size().x);
                a.max(b) + 24.0
            }
            _ => 0.0,
        }
    };
    let w = items.iter().map(row_w).fold(min_w, f32::max).min(ctx.content_rect().width() - 32.0) + 2.0 * PAD;
    let h: f32 = items
        .iter()
        .map(|it| match it {
            Item::Head(_) => 24.0,
            Item::Sep => SEP_H,
            Item::Row(r) => if r.sub.is_empty() { 34.0 } else { 44.0 },
            Item::Who(_) => WHO_H,
        })
        .sum::<f32>()
        + 2.0 * PAD;
    let drop = crate::layer::drop_card(ctx, anchor, bounds, vec2(w, h), left);
    let (rect, origin) = (drop.rect, drop.origin);
    crate::layer::card_at(ctx, id, rect);
    let layer = egui::LayerId::new(egui::Order::Foreground, id.with("menu"));
    drop.enter(ctx, layer, e);
    let mut picked = None;
    let lens = items.iter().any(|it| matches!(it, Item::Who(_)));
    let mut who_rows: Vec<(usize, f32)> = Vec::new();
    {
        let mut y = rect.top() + PAD;
        for (i, it) in items.iter().enumerate() {
            match it {
                Item::Head(_) => y += 24.0,
                Item::Sep => y += SEP_H,
                Item::Row(r) => y += if r.sub.is_empty() { 34.0 } else { 44.0 },
                Item::Who(_) => {
                    who_rows.push((i, y));
                    y += WHO_H;
                }
            }
        }
    }
    let keyed_id = id.with("keyed");
    if lens && !who_rows.is_empty() {
        let (up, down, enter) = ctx.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::Enter)));
        let home = who_rows.iter().position(|(i, _)| matches!(items[*i], Item::Who(w) if w.current)).unwrap_or(0);
        let mut at = ctx.data(|d| d.get_temp::<usize>(keyed_id)).unwrap_or(home);
        let n = who_rows.len();
        if up {
            at = (at + n - 1) % n;
        }
        if down {
            at = (at + 1) % n;
        }
        if up || down {
            ctx.data_mut(|d| d.insert_temp(keyed_id, at));
        }
        if enter {
            picked = Some(who_rows[at].0);
        }
    }
    crate::layer::place(ctx, layer, parent);
    let mut guarded = false;
    let shown = crate::layer::over(ctx, layer).show(ctx, |ui| {
        guarded = crate::layer::under(ui, crate::layer::Under::Guard);
        crate::layer::body(ui, rect);
        ui.multiply_opacity(e);
        crate::material::paint(ui.painter(), rect, Radius::Menu, crate::material::Level::Float, 0.0);
        card_is(ctx, rect);
        let lens_slot = ui.painter().add(egui::Shape::Noop);
        let mut y = rect.top() + PAD;
        let mut seps = Vec::new();
        for (i, it) in items.iter().enumerate() {
            match it {
                Item::Head(s) => {
                    ui.painter().text(pos2(rect.left() + PAD + 10.0, y + 14.0), egui::Align2::LEFT_CENTER, *s, Type::Micro.font(), c(C::Ink3));
                    y += 24.0;
                }
                Item::Sep => {
                    seps.push(y + PAD + crate::band::LINE_HALF);
                    y += SEP_H;
                }
                Item::Row(r) => {
                    let rh = row_h(r);
                    let rr = Rect::from_min_size(pos2(rect.left() + PAD, y), vec2(w - 2.0 * PAD, rh));
                    let bound = |k: Option<&Item>, sep: crate::band::Bound| match k {
                        None => crate::band::Bound::Edge,
                        Some(Item::Sep) => sep,
                        Some(_) => crate::band::Bound::Row,
                    };
                    let above = bound(i.checked_sub(1).and_then(|k| items.get(k)), crate::band::Bound::Rule);
                    if draw_row(ui, id.with(("row", i)), rr, r, (above, bound(items.get(i + 1), crate::band::Bound::Line))).clicked() {
                        picked = Some(i);
                    }
                    y += rh;
                }
                Item::Who(wr) => {
                    let rr = Rect::from_min_size(pos2(rect.left() + PAD, y), vec2(w - 2.0 * PAD, WHO_H));
                    let resp = draw_who(ui, id.with(("who", i)), rr, wr);
                    if crate::flow::pointer_on(ui, rr) {
                        ctx.data_mut(|d| d.insert_temp(id.with("hovered-who"), i));
                    }
                    if resp.clicked() {
                        picked = Some(i);
                    }
                    y += WHO_H;
                }
            }
        }
        for sy in seps {
            ui.painter().hline((rect.left() + PAD + 6.0)..=(rect.right() - PAD - 6.0), sy, egui::Stroke::new(1.0_f32, c(C::Line2)));
        }
        if lens && !who_rows.is_empty() {
            let hovered = ctx.data(|d| d.get_temp::<usize>(id.with("hovered-who")));
            let pointer_in = ctx.input(|i| i.pointer.hover_pos()).map(|p| who_rows.iter().any(|(_, y)| Rect::from_min_size(pos2(rect.left() + PAD, *y), vec2(w - 2.0 * PAD, WHO_H)).contains(p))).unwrap_or(false);
            let home = who_rows.iter().position(|(i, _)| matches!(items[*i], Item::Who(w) if w.current)).unwrap_or(0);
            let keyed = ctx.data(|d| d.get_temp::<usize>(keyed_id));
            let target = match (pointer_in, hovered) {
                (true, Some(h)) => who_rows.iter().position(|(i, _)| *i == h).unwrap_or(home),
                _ => keyed.unwrap_or(home),
            };
            let opened = ctx.data(|d| d.get_temp::<u64>(id.with("opened"))).unwrap_or(0);
            let top = motion::to(ctx, id.with(("lens", opened)), who_rows[target].1, tokens::LENS, Curve::Ease);
            let lr = Rect::from_min_size(pos2(rect.left() + PAD, top), vec2(w - 2.0 * PAD, WHO_H));
            ui.painter().set(lens_slot, egui::epaint::RectShape::filled(lr, crate::band::corners(Radius::Menu.px(), WHO_H, crate::band::Bound::Row, crate::band::Bound::Row), c(C::Hover)));
        }
        ui.allocate_rect(rect, egui::Sense::hover());
    });
    crate::layer::keep(ctx, id.with("keep"), layer, tokens::FAST, 0.97, origin, drop.moved);
    let esc = crate::layer::esc(ctx, id);
    if picked.is_some() || guarded || esc {
        set(ctx, id, false);
    }
    let _ = shown;
    picked
}

pub fn menu_key(ui: &mut egui::Ui, id_salt: &str, label: &str, left: bool, min_w: f32, items: &[Item]) -> Option<usize> {
    let id = ui.id().with(("zikaron-menu", id_salt));
    let resp = crate::button::show(ui, crate::button::Key::new(label, crate::button::Role::Secondary).trail(Glyph::Down));
    if resp.clicked() {
        toggle(ui.ctx(), id);
    }
    let picked = show_from(ui.ctx(), Some(ui.layer_id()), id, resp.rect, ui.clip_rect(), left, min_w, items);
    stay(ui, id, resp.rect);
    picked
}

pub fn inner_menu_key(ui: &mut egui::Ui, id_salt: &str, label: &str, min_w: f32, items: &[Item]) -> Option<usize> {
    let id = ui.id().with(("zikaron-menu", id_salt));
    let resp = crate::fields::inner_key(ui, label, Some(Glyph::Down));
    if resp.clicked() {
        toggle(ui.ctx(), id);
    }
    let picked = show_from(ui.ctx(), Some(ui.layer_id()), id, resp.rect, ui.clip_rect(), false, min_w, items);
    stay(ui, id, resp.rect);
    picked
}

pub(crate) fn stay(ui: &egui::Ui, id: egui::Id, anchor: Rect) {
    if is_open(ui.ctx(), id) && crate::layer::follow(ui, id, anchor) {
        set(ui.ctx(), id, false);
    }
}

pub fn plain_key(ui: &mut egui::Ui, id_salt: &str, label: &str, enabled: bool, left: bool, min_w: f32, items: &[Item]) -> Option<usize> {
    let id = ui.id().with(("zikaron-menu", id_salt));
    let resp = crate::button::key(ui, label, crate::button::Role::Secondary, enabled);
    if resp.clicked() {
        toggle(ui.ctx(), id);
    }
    let picked = show_from(ui.ctx(), Some(ui.layer_id()), id, resp.rect, ui.clip_rect(), left, min_w, items);
    stay(ui, id, resp.rect);
    picked
}

pub(crate) fn draw_row(ui: &mut egui::Ui, id: egui::Id, rr: Rect, r: &Row, bounds: (crate::band::Bound, crate::band::Bound)) -> egui::Response {
    let sense = if r.disabled { egui::Sense::hover() } else { egui::Sense::click() };
    let lit = lit_rect(ui, rr, bounds);
    let resp = ui.interact(lit, id, sense);
    let hovered = !r.disabled && crate::flow::pointer_on(ui, lit);
    if hovered {
        let press = resp.is_pointer_button_down_on();
        let corners = crate::band::corners(Radius::Menu.px(), lit.height(), bounds.0, bounds.1);
        ui.painter().with_clip_rect(ui.clip_rect().union(lit)).rect_filled(lit, corners, c(if press { C::Press } else { C::Hover }));
        lit_probe(ui, rr, lit, corners, bounds);
    }
    let alpha = if r.disabled { 0.4 } else { 1.0 };
    let ink = c(C::Ink).gamma_multiply(alpha);
    let quiet = c(C::Ink3).gamma_multiply(alpha);
    let p = ui.painter();
    let mut tx = rr.left() + 12.0;
    let right_edge = rr.right() - 12.0;
    if let Some(on) = r.check {
        if on {
            icons::glyph_at(p, Glyph::Ok, pos2(tx + 7.0, rr.center().y), 14.0, c(C::Accent));
        }
        tx += 14.0 + 8.0;
    }
    if !r.lead.is_empty() {
        p.text(pos2(tx, rr.center().y), egui::Align2::LEFT_CENTER, r.lead, Type::Small.font(), quiet);
        tx += 36.0;
    }
    let mut rx = right_edge;
    if r.tick_right {
        icons::glyph_at(p, Glyph::Ok, pos2(rx - 7.0, rr.center().y), 14.0, c(C::Accent));
        rx -= 14.0 + 8.0;
    }
    if !r.trail.is_empty() {
        let t = p.text(pos2(rx, rr.center().y), egui::Align2::RIGHT_CENTER, r.trail, Type::Small.font(), quiet);
        rx = t.left() - 12.0;
    }
    if let Some((s, tone)) = r.pill {
        let pw = mark::pill_w(ui, s, false);
        let pr = Rect::from_min_size(pos2(rx - pw, rr.center().y - tokens::PILL_H / 2.0), vec2(pw, tokens::PILL_H));
        p.rect_filled(pr, Radius::Pill.egui(), tone.wash());
        p.text(pr.center(), egui::Align2::CENTER_CENTER, s, Type::Small.font(), tone.ink());
        rx = pr.left() - 12.0;
    }
    let font = if r.mono { egui::FontId::new(14.0, egui::FontFamily::Monospace) } else { Type::Key.font() };
    let room = (rx - tx).max(0.0);
    if r.sub.is_empty() {
        let lr = paint::at(p, ui, pos2(tx, rr.center().y), egui::Align2::LEFT_CENTER, r.label, if r.mono { Type::MonoSmall } else { Type::Key }, ink, room);
        let _ = font;
        if r.struck {
            p.hline(lr.x_range(), rr.center().y, egui::Stroke::new(1.0_f32, ink));
        }
    } else {
        paint::at(p, ui, pos2(tx, rr.top() + 5.0), egui::Align2::LEFT_TOP, r.label, Type::Small, ink, room);
        let sub_c = if r.sub_ok { c(C::OkInk) } else { c(C::Ink3) };
        paint::at(p, ui, pos2(tx, rr.top() + 23.0), egui::Align2::LEFT_TOP, r.sub, Type::Small, sub_c, room);
    }
    if r.disabled {
        resp
    } else {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}

pub(crate) fn row_h(r: &Row) -> f32 {
    if r.sub.is_empty() { 34.0 } else { 44.0 }
}

pub(crate) fn draw_who(ui: &mut egui::Ui, id: egui::Id, rr: Rect, wr: &Who) -> egui::Response {
    let resp = ui.interact(rr, id, egui::Sense::click());
    let p = ui.painter();
    let room = rr.width() - 24.0;
    paint::at(p, ui, pos2(rr.left() + 12.0, rr.top() + 6.0), egui::Align2::LEFT_TOP, wr.name, Type::Key, c(C::Ink), room);
    let kind_r = paint::at(p, ui, pos2(rr.left() + 12.0, rr.top() + 26.0), egui::Align2::LEFT_TOP, wr.kind, Type::Small, c(C::Ink3), room);
    let mut tx = kind_r.right();
    for t in wr.tags {
        let dot = p.text(pos2(tx, rr.top() + 26.0), egui::Align2::LEFT_TOP, " \u{b7} ", Type::Small.font(), c(C::Ink3));
        let tr = p.text(pos2(dot.right(), rr.top() + 26.0), egui::Align2::LEFT_TOP, *t, Type::Small.font(), c(C::Ink3));
        tx = tr.right();
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

const CARD: &str = "zikaron-menu-card";

pub(crate) fn card_is(ctx: &egui::Context, rect: Rect) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(CARD), rect));
}

fn lit_rect(ui: &egui::Ui, rr: Rect, bounds: (crate::band::Bound, crate::band::Bound)) -> Rect {
    use crate::band::Bound;
    let clip = ui.clip_rect();
    let reach = PAD + crate::band::LINE_HALF;
    let at_line = |b: Bound| matches!(b, Bound::Line | Bound::Rule);
    let up = if at_line(bounds.0) && rr.top() >= clip.top() - 0.5 { reach } else { 0.0 };
    let down = if at_line(bounds.1) && rr.bottom() <= clip.bottom() + 0.5 { reach } else { 0.0 };
    Rect::from_min_max(pos2(rr.left(), rr.top() - up), pos2(rr.right(), rr.bottom() + down))
}

fn lit_probe(ui: &egui::Ui, rr: Rect, lit: Rect, corners: egui::CornerRadius, bounds: (crate::band::Bound, crate::band::Bound)) {
    use crate::band::Bound;
    let card = ui.ctx().data(|d| d.get_temp::<Rect>(egui::Id::new(CARD))).unwrap_or(rr.expand(PAD));
    let out = |b: Bound, gap_to_edge: f32| match b {
        Bound::Line | Bound::Rule => PAD + crate::band::LINE_HALF,
        _ if (gap_to_edge - PAD).abs() < 0.5 => PAD,
        _ => 0.0,
    };
    let up = out(bounds.0, rr.top() - card.top());
    let down = out(bounds.1, card.bottom() - rr.bottom());
    let slot = Rect::from_min_max(pos2(card.left(), rr.top() - up), pos2(card.right(), rr.bottom() + down));
    crate::probe::lit(ui.ctx(), crate::probe::Lit { block: lit, corners, slot, card, card_corner: Radius::Menu.px() });
}
