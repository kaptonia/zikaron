use crate::motion::{self, Curve};
use egui::{Context, Id, LayerId, Pos2, Rect, Vec2};

#[derive(Clone)]
struct Snap {
    layer: LayerId,
    shapes: Vec<egui::epaint::ClippedShape>,
    pass: u64,
    left_at: Option<f64>,
    dur: f32,
    to_scale: f32,
    center: egui::Pos2,
    moved: Vec2,
}

fn table_id() -> Id {
    Id::new("zikaron-leaving-layers")
}

pub fn area(id: Id, order: egui::Order) -> egui::Area {
    egui::Area::new(id).order(order).fade_in(false)
}

fn cover(ctx: &Context, layer: LayerId) -> Rect {
    let screen = ctx.content_rect();
    ctx.layer_transform_from_global(layer).map(|t| t * screen).unwrap_or(screen)
}

pub fn over(ctx: &Context, layer: LayerId) -> egui::Area {
    area(layer.id, layer.order).fixed_pos(cover(ctx, layer).min).constrain(false)
}

pub fn keep(ctx: &Context, key: Id, layer: LayerId, dur: f32, to_scale: f32, center: egui::Pos2, moved: Vec2) {
    let k = crate::motion::travel(ctx);
    let (to_scale, moved) = (1.0 + (to_scale - 1.0) * k, moved * k);
    let shapes: Vec<egui::epaint::ClippedShape> = ctx.graphics(|g| g.get(layer).map(|l| l.all_entries().cloned().collect()).unwrap_or_default());
    let pass = crate::motion::pass(ctx);
    ctx.data_mut(|d| {
        let t = d.get_temp_mut_or_default::<Vec<(Id, Snap)>>(table_id());
        t.retain(|(k, _)| *k != key);
        t.push((key, Snap { layer, shapes, pass, left_at: None, dur, to_scale, center, moved }));
    });
}

pub fn forget(ctx: &Context, gone: impl Fn(Id) -> bool) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<(Id, Snap)>>(table_id()).retain(|(k, _)| !gone(*k)));
}

pub fn steer(ctx: &Context, key: Id, moved: Vec2) {
    let moved = moved * crate::motion::travel(ctx);
    ctx.data_mut(|d| {
        if let Some(t) = d.get_temp_mut_or_default::<Vec<(Id, Snap)>>(table_id()).iter_mut().find(|(k, _)| *k == key) {
            t.1.moved = moved;
        }
    });
}

fn leaving_alpha(s: &Snap, now: f64) -> Option<f32> {
    let at = s.left_at?;
    let p = ((now - at) as f32 / s.dur).clamp(0.0, 1.0);
    (p < 1.0).then(|| Curve::Ease.at(1.0 - p))
}

pub fn entrance(ctx: &Context, keys: &[Id], age_id: Id, dur: f32) -> f32 {
    let now = crate::motion::now(ctx);
    let back = ctx.data_mut(|d| {
        let t = d.get_temp_mut_or_default::<Vec<(Id, Snap)>>(table_id());
        let mut at: Option<f32> = None;
        for (k, s) in t.iter_mut() {
            if keys.contains(k) {
                if let Some(a) = leaving_alpha(s, now) {
                    at = Some(at.map_or(a, |x| x.max(a)));
                }
            }
        }
        if at.is_some() {
            t.retain(|(k, _)| !keys.contains(k));
        }
        at
    });
    if let Some(a) = back {
        motion::set_age(ctx, age_id, 0, dur * Curve::Ease.time_of(a));
    }
    let age = motion::age(ctx, age_id, 0);
    if age < dur {
        ctx.request_repaint();
    }
    Curve::Ease.at((age / dur).clamp(0.0, 1.0))
}

pub fn leaving_colour(col: egui::Color32, alpha: f32) -> egui::Color32 {
    if col == egui::Color32::PLACEHOLDER {
        return col;
    }
    col.gamma_multiply(alpha)
}

pub fn fade_out(ctx: &Context) {
    let pass = crate::motion::pass(ctx);
    let now = crate::motion::now(ctx);
    let mut snaps: Vec<(Id, Snap)> = ctx.data(|d| d.get_temp::<Vec<(Id, Snap)>>(table_id())).unwrap_or_default();
    let mut keep_going = Vec::new();
    for (key, mut s) in snaps.drain(..) {
        if s.pass == pass {
            keep_going.push((key, s));
            continue;
        }
        s.left_at.get_or_insert(now);
        let Some(alpha) = leaving_alpha(&s, now) else {
            continue;
        };
        let scale = s.to_scale + (1.0 - s.to_scale) * alpha;
        let ghost = LayerId::new(s.layer.order, s.layer.id.with("leaving"));
        let painter = ctx.layer_painter(ghost);
        for cs in &s.shapes {
            let mut shape = cs.shape.clone();
            egui::epaint::shape_transform::adjust_colors(&mut shape, move |col| *col = leaving_colour(*col, alpha));
            painter.with_clip_rect(cs.clip_rect).add(shape);
        }
        let translation = s.center.to_vec2() * (1.0 - scale) + s.moved * (1.0 - alpha);
        ctx.set_transform_layer(ghost, egui::emath::TSTransform { scaling: scale, translation });
        ctx.request_repaint();
        keep_going.push((key, s));
    }
    ctx.data_mut(|d| d.insert_temp(table_id(), keep_going));
}

#[derive(Clone, Copy, Debug)]
pub enum Under {
    Guard,
    Ground(egui::Color32),
    Taken,
    Open,
}

pub fn place(ctx: &Context, card: LayerId, parent: Option<LayerId>) {
    let root = parent.filter(|p| p.order == card.order && *p != card).map(|p| ctx.memory(|m| m.areas().parent_layer(p)).unwrap_or(p));
    match root {
        Some(r) => ctx.set_sublayer(r, card),
        None => {
            if !ctx.memory(|m| m.areas().visible_last_frame(&card)) {
                ctx.move_to_top(card);
            }
        }
    }
}

pub fn under(ui: &mut egui::Ui, under: Under) -> bool {
    let sense = match under {
        Under::Open => return false,
        Under::Guard => egui::Sense::click(),
        Under::Ground(_) | Under::Taken => egui::Sense::click_and_drag(),
    };
    let all = cover(ui.ctx(), ui.layer_id());
    ui.expand_to_include_rect(all);
    let clip = ui.clip_rect();
    if let Under::Ground(fill) = under {
        let painted = ground(ui);
        ui.painter().rect_filled(painted, 0.0, fill);
    }
    let under_resp = ui.interact(all, ui.id().with("zikaron-under"), sense);
    crate::hand::sink(ui.ctx(), under_resp.id);
    let pressed = under_resp.clicked();
    ui.set_clip_rect(clip);
    matches!(under, Under::Guard) && pressed
}

const GROUND_PAST: f32 = 0.03;

pub fn ground(ui: &mut egui::Ui) -> Rect {
    let all = cover(ui.ctx(), ui.layer_id());
    let painted = all.expand(GROUND_PAST * all.width().max(all.height()));
    ui.set_clip_rect(painted);
    painted
}

pub fn body(ui: &mut egui::Ui, card: Rect) {
    let r = ui.interact(card, ui.id().with("zikaron-card-body"), egui::Sense::click());
    crate::hand::sink(ui.ctx(), r.id);
}

pub struct Drop {
    pub rect: Rect,
    pub origin: Pos2,
    pub moved: Vec2,
}

impl Drop {
    pub fn enter(&self, ctx: &Context, layer: egui::LayerId, e: f32) {
        let k = crate::motion::travel(ctx);
        let scale = 1.0 - 0.03 * (1.0 - e) * k;
        ctx.set_transform_layer(layer, egui::emath::TSTransform { scaling: scale, translation: self.origin.to_vec2() * (1.0 - scale) + self.moved * (1.0 - e) * k });
    }
}

pub fn drop_card(ctx: &Context, anchor: Rect, bounds: Rect, size: Vec2, left: bool) -> Drop {
    let screen = ctx.content_rect();
    let (room_l, room_r) = (bounds.left().max(screen.left()), bounds.right().min(screen.right()));
    let (room_l, room_r) = if room_r > room_l { (room_l, room_r) } else { (screen.left(), screen.right()) };
    let x = if left { anchor.left() } else { anchor.right() - size.x };
    let x = x.clamp(room_l + 8.0, (room_r - size.x - 8.0).max(room_l + 8.0));
    let below = anchor.bottom() + 6.0;
    let above = anchor.top() - 6.0 - size.y;
    let room_below = screen.bottom() - 8.0 - below;
    let room_above = anchor.top() - 6.0 - (screen.top() + 8.0);
    let scroll = crate::scroll::room_below(ctx, anchor).min((anchor.top() - bounds.top().max(screen.top())).max(0.0));
    let carried = size.y > room_below && size.y + REVEAL_FOOT - 8.0 <= room_below + scroll;
    let up = if size.y <= room_below || carried { false } else if size.y <= room_above { true } else { room_above > room_below };
    let y = if up { above } else { below };
    let y = if carried { y } else { y.clamp(screen.top() + 8.0, (screen.bottom() - 8.0 - size.y).max(screen.top() + 8.0)) };
    let rect = Rect::from_min_size(egui::pos2(x, y), size);
    let origin = match (left, up) {
        (true, false) => rect.left_top(),
        (false, false) => rect.right_top(),
        (true, true) => rect.left_bottom(),
        (false, true) => rect.right_bottom(),
    };
    Drop { rect, origin, moved: egui::vec2(0.0, if up { 4.0 } else { -4.0 }) }
}

pub const REVEAL_FOOT: f32 = 32.0;

fn card_rect_id(id: Id) -> Id {
    id.with("zikaron-card-at")
}

pub fn card_at(ctx: &Context, id: Id, rect: Rect) {
    let (now, pass) = (crate::motion::now(ctx), crate::motion::pass(ctx));
    ctx.data_mut(|d| {
        let opened = match d.get_temp::<(f64, u64)>(card_rect_id(id).with("seen")) {
            Some((at, last)) if last + 1 >= pass => at,
            _ => now,
        };
        d.insert_temp(card_rect_id(id).with("seen"), (opened, pass));
        d.insert_temp(card_rect_id(id), rect);
    });
}

pub fn follow(ui: &egui::Ui, id: Id, anchor: Rect) -> bool {
    let ctx = ui.ctx();
    let Some(card) = ctx.data(|d| d.get_temp::<Rect>(card_rect_id(id))) else { return false };
    let view = ui.clip_rect();
    if !view.intersects(anchor) {
        return true;
    }
    let over_card = ctx.pointer_hover_pos().map(|p| card.contains(p)).unwrap_or(false);
    let wheel = ctx.input(|i| i.smooth_scroll_delta);
    if !over_card && wheel != Vec2::ZERO {
        ui.scroll_with_delta_animation(wheel, egui::style::ScrollAnimation::none());
        ctx.input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
    }
    let opened = ctx.data(|d| d.get_temp::<(f64, u64)>(card_rect_id(id).with("seen"))).map(|(at, _)| at).unwrap_or(0.0);
    let opening = ((crate::motion::now(ctx) - opened) as f32) < crate::tokens::SLOW + 0.05;
    let screen = ctx.content_rect();
    if opening && card.top() >= anchor.bottom() && card.bottom() + REVEAL_FOOT > screen.bottom() + 0.5 {
        crate::fold::reveal(ui, Rect::from_min_max(anchor.min, egui::pos2(anchor.right(), card.bottom() + REVEAL_FOOT)));
    }
    false
}

pub fn on_top(ui: &egui::Ui, at: Rect) -> bool {
    ui.ctx().layer_id_at(at.center()).is_none_or(|l| l == ui.layer_id())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Amount {
    pub shown: String,
    pub whole: String,
}

impl Amount {
    pub fn new(shown: impl Into<String>, whole: impl Into<String>) -> Amount {
        Amount { shown: shown.into(), whole: whole.into() }
    }

    pub fn cut(&self) -> bool {
        self.shown != self.whole
    }
}

pub fn amount_tip(resp: egui::Response, a: &Amount) -> egui::Response {
    if a.cut() { tip(resp, a.whole.as_str()) } else { resp }
}

pub fn tip(resp: egui::Response, text: impl Into<egui::WidgetText>) -> egui::Response {
    let text: egui::WidgetText = text.into();
    crate::probe::whole_on_hover(&resp.ctx, text.text());
    resp.on_hover_text(text)
}

#[derive(Clone, Copy)]
struct Esc {
    key: Id,
    seq: u64,
    first: u64,
    pass: u64,
}

pub fn esc(ctx: &Context, key: Id) -> bool {
    let pass = crate::motion::pass(ctx);
    let pressed = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let tid = Id::new("zikaron-esc-owners");
    ctx.data_mut(|d| {
        let t = d.get_temp_mut_or_default::<(u64, Vec<Esc>)>(tid);
        t.1.retain(|e| e.pass + 1 >= pass);
        let owner = t.1.iter().filter(|e| e.first < pass).max_by_key(|e| e.seq).map(|e| e.key);
        match t.1.iter_mut().find(|e| e.key == key) {
            Some(e) => e.pass = pass,
            None => {
                t.0 += 1;
                let seq = t.0;
                t.1.push(Esc { key, seq, first: pass, pass });
            }
        }
        pressed && owner == Some(key)
    })
}
