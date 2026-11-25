use crate::motion::{self, Curve};
use crate::paint;
use crate::palette::{c, Tone, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Color32, Painter, Pos2};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Mark {
    Ok,
    Todo,
    Warn,
    Bad,
    Busy,
    Work,
}

impl Mark {
    pub const ALL: [Mark; 6] = [Mark::Ok, Mark::Todo, Mark::Warn, Mark::Bad, Mark::Busy, Mark::Work];

    pub fn as_str(self) -> &'static str {
        match self {
            Mark::Ok => "ok",
            Mark::Todo => "todo",
            Mark::Warn => "warn",
            Mark::Bad => "bad",
            Mark::Busy => "busy",
            Mark::Work => "work",
        }
    }

    pub fn spins(self) -> bool {
        matches!(self, Mark::Busy | Mark::Work)
    }
}

pub fn paint_mark(ctx: &egui::Context, p: &Painter, at: Pos2, m: Mark, size: f32) {
    let r = size / 2.0;
    let k = size / 18.0;
    let pt = |x: f32, y: f32| pos2(at.x - r + x * k, at.y - r + y * k);
    let w = 1.8 * k;
    match m {
        Mark::Ok => {
            p.circle_filled(at, r, c(C::OkWash));
            p.add(egui::Shape::line(vec![pt(5.2, 9.0), pt(7.6, 11.4), pt(12.6, 6.5)], egui::Stroke::new(w, c(C::Ok))));
        }
        Mark::Warn => {
            p.circle_filled(at, r, c(C::WarnWash));
            let ink = c(C::Warn);
            p.line_segment([pt(9.0, 4.8), pt(9.0, 10.0)], egui::Stroke::new(w, ink));
            p.circle_filled(pt(9.0, 12.9), 1.15 * k, ink);
        }
        Mark::Bad => {
            p.circle_filled(at, r, c(C::BadWash));
            let s = egui::Stroke::new(w, c(C::Bad));
            let d = 4.5 * k * std::f32::consts::FRAC_1_SQRT_2;
            p.line_segment([pos2(at.x - d, at.y - d), pos2(at.x + d, at.y + d)], s);
            p.line_segment([pos2(at.x + d, at.y - d), pos2(at.x - d, at.y + d)], s);
        }
        Mark::Todo => {
            p.circle_stroke(at, r - 0.75 * k, egui::Stroke::new(1.5 * k, c(C::Dash)));
        }
        Mark::Busy | Mark::Work => {
            let colour = if m == Mark::Busy { c(C::Warn) } else { c(C::Accent) };
            spinner(ctx, p, at, r - 2.0 * k - 0.9 * k, w, colour, 0.9);
        }
    }
}

pub fn spinner(ctx: &egui::Context, p: &Painter, at: Pos2, radius: f32, width: f32, colour: Color32, period: f32) {
    let turn = motion::cycle(ctx, period) * std::f32::consts::TAU;
    let n = 24;
    let from = turn + std::f32::consts::FRAC_PI_4;
    let span = std::f32::consts::TAU * 0.75;
    let pts: Vec<Pos2> = (0..=n)
        .map(|i| {
            let a = from + span * i as f32 / n as f32;
            pos2(at.x + radius * a.cos(), at.y + radius * a.sin())
        })
        .collect();
    p.add(egui::Shape::line(pts, egui::Stroke::new(width, colour)));
}

pub fn mark(ui: &mut egui::Ui, m: Mark) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(tokens::MARK, tokens::MARK), egui::Sense::hover());
    paint_mark(ui.ctx(), ui.painter(), rect.center(), m, tokens::MARK);
    resp
}

pub fn mark_line(ui: &mut egui::Ui, m: Mark, s: &str, t: Type, colour: Color32) -> egui::Response {
    mark_words(ui, m, s, t, colour, tokens::S2)
}

pub fn mark_words(ui: &mut egui::Ui, m: Mark, s: &str, t: Type, colour: Color32, gap: f32) -> egui::Response {
    let words = paint::wrapped(ui, s, t.font(), colour, t.line(), ui.available_width() - tokens::MARK - gap);
    let size = vec2(tokens::MARK + gap + words.size().x, words.size().y.max(tokens::MARK));
    crate::width::cell(ui, size, egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = gap;
        mark(ui, m);
        ui.add(egui::Label::new(words));
    })
    .1
}

#[derive(Clone, Copy)]
struct Pop {
    key: u64,
    t0: f64,
}

fn popped(ctx: &egui::Context, id: egui::Id, key: u64, force: bool, delay: f32) -> f32 {
    let now = crate::motion::now(ctx);
    let t0 = ctx.data_mut(|d| {
        let first = d.get_temp::<Pop>(id).is_none();
        let s = d.get_temp_mut_or_insert_with(id, || Pop { key, t0: if force { now } else { f64::NEG_INFINITY } });
        if !first && s.key != key {
            *s = Pop { key, t0: now };
        }
        s.t0
    });
    let a = (now - t0) as f32 - delay;
    if a < 0.0 {
        ctx.request_repaint();
        return 0.0;
    }
    let p = (a / tokens::MID).clamp(0.0, 1.0);
    if p < 1.0 {
        ctx.request_repaint();
    }
    p
}

fn pill_size(ui: &egui::Ui, s: &str, live: bool) -> egui::Vec2 {
    let g = paint::galley(ui, s, Type::Small, Color32::BLACK);
    let spin = if live { 10.0 + 5.0 } else { 0.0 };
    vec2(g.size().x + 20.0 + spin, tokens::PILL_H)
}

#[derive(Clone)]
struct Was {
    key: u64,
    wash: Color32,
    ink: Color32,
    words: String,
    at: f64,
}

fn paint_pill(ui: &egui::Ui, id: egui::Id, rect: egui::Rect, s: &str, state: &str, tone: Tone, live: bool, pop: f32) {
    let p = ui.painter();
    let now = crate::motion::now(ui.ctx());
    let k = if pop < 1.0 { 0.6 + 0.4 * Curve::Spring.at(pop) } else { 1.0 };
    let k = 1.0 + (k - 1.0) * motion::travel(ui.ctx());
    let alpha = if pop < 1.0 { pop } else { 1.0 };
    let r = paint::scaled(rect, k);
    let key = motion::key_of(&(state, tone as u8));
    let wid = id.with("was");
    let was = ui.ctx().data(|d| d.get_temp::<Was>(wid));
    let was = match was {
        Some(w) if w.key == key => w,
        Some(w) => {
            let back = Was { key, wash: w.wash, ink: w.ink, words: w.words.clone(), at: now };
            ui.ctx().data_mut(|d| d.insert_temp(wid, back.clone()));
            back
        }
        None => {
            let w = Was { key, wash: tone.wash(), ink: tone.ink(), words: s.to_string(), at: f64::NEG_INFINITY };
            ui.ctx().data_mut(|d| d.insert_temp(wid, w.clone()));
            w
        }
    };
    let t = if now >= was.at { ((now - was.at) as f32 / tokens::SLOW).clamp(0.0, 1.0) } else { 1.0 };
    if t < 1.0 {
        ui.ctx().request_repaint();
    } else if was.words != s || was.wash != tone.wash() || was.ink != tone.ink() {
        ui.ctx().data_mut(|d| d.insert_temp(wid, Was { key, wash: tone.wash(), ink: tone.ink(), words: s.to_string(), at: f64::NEG_INFINITY }));
    }
    let e = Curve::Ease.at(t);
    let fill = crate::palette::mix(was.wash, tone.wash(), e).gamma_multiply(alpha);
    let ink = crate::palette::mix(was.ink, tone.ink(), e).gamma_multiply(alpha);
    p.rect_filled(r, Radius::Pill.egui(), fill);
    let font = egui::FontId::new(Type::Small.size() * k, egui::FontFamily::Proportional);
    let mut x = r.left() + 10.0 * k;
    if live {
        spinner(ui.ctx(), p, pos2(x + 5.0 * k, r.center().y), 4.25 * k, 1.5 * k, ink, 0.9);
        x += 15.0 * k;
    }
    let clip = p.with_clip_rect(r.intersect(p.clip_rect()));
    if t < 1.0 && was.words != s {
        let old = clip.layout_no_wrap(was.words.clone(), font.clone(), ink.gamma_multiply(1.0 - e));
        clip.galley(pos2(x, r.center().y - old.size().y / 2.0), old, ink.gamma_multiply(1.0 - e));
        let g = clip.layout_no_wrap(s.to_string(), font, ink.gamma_multiply(e));
        clip.galley(pos2(x, r.center().y - g.size().y / 2.0), g, ink.gamma_multiply(e));
    } else {
        let g = p.layout_no_wrap(s.to_string(), font, ink);
        p.galley(pos2(x, r.center().y - g.size().y / 2.0), g, ink);
    }
}

pub fn pill(ui: &mut egui::Ui, s: &str, tone: Tone) -> egui::Response {
    pill_ex(ui, s, tone, false, false, 0.0)
}

pub fn pill_live(ui: &mut egui::Ui, s: &str, tone: Tone) -> egui::Response {
    pill_ex(ui, s, tone, true, false, 0.0)
}

pub fn pill_pop(ui: &mut egui::Ui, s: &str, tone: Tone, delay: f32) -> egui::Response {
    pill_ex(ui, s, tone, false, true, delay)
}

fn pill_ex(ui: &mut egui::Ui, s: &str, tone: Tone, live: bool, force: bool, delay: f32) -> egui::Response {
    let natural = pill_size(ui, s, live);
    let room = ui.available_width();
    let (size, shown) = if room.is_finite() && room >= 0.0 && natural.x > room + 1.0 {
        let spin = if live { 10.0 + 5.0 } else { 0.0 };
        (vec2(room, natural.y), crate::width::elide_to(ui, s, Type::Small.font(), (room - 20.0 - spin).max(0.0)))
    } else {
        (natural, s.to_string())
    };
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let pop = if force { popped(ui.ctx(), resp.id.with("pop"), motion::key_of(&(s, tone as u8)), force, delay) } else { 1.0 };
    paint_pill(ui, resp.id, rect, &shown, s, tone, live, pop);
    if shown != s { crate::layer::tip(resp, s) } else { resp }
}

pub fn pill_w(ui: &egui::Ui, s: &str, live: bool) -> f32 {
    pill_size(ui, s, live).x
}

pub fn tag(ui: &mut egui::Ui, s: &str) -> egui::Response {
    tag_in(ui, s, tokens::TAG_MIN_W)
}

pub fn tag_in(ui: &mut egui::Ui, s: &str, col: f32) -> egui::Response {
    let g = paint::galley(ui, s, Type::Small, c(C::Ink2));
    let w = (g.size().x + tokens::TAG_PAD).max(tokens::TAG_MIN_W).max(col);
    let (rect, resp) = ui.allocate_exact_size(vec2(w, tokens::TAG_H), egui::Sense::hover());
    paint_tag(ui.painter(), rect, g);
    resp
}

pub fn paint_tag(p: &Painter, rect: egui::Rect, g: std::sync::Arc<egui::Galley>) {
    p.rect_filled(rect, Radius::Tag.egui(), c(C::Tone));
    p.galley(pos2(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0), g, c(C::Ink2));
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Count {
    Alert,
    Selected,
    Soft,
}

pub fn count_live(ui: &egui::Ui, id: egui::Id, right: f32, y: f32, n: usize, how: Count) -> f32 {
    let ctx = ui.ctx();
    let now = crate::motion::now(ui.ctx());
    let shown = motion::flag(ctx, id.with("count-on"), n > 0, crate::tokens::MID);
    let kid = id.with("count-n");
    let (last, mut changed) = ctx.data(|d| d.get_temp::<(usize, f64)>(kid)).unwrap_or((n, f64::NEG_INFINITY));
    if n > 0 && n != last {
        if last > 0 {
            changed = now;
        }
        ctx.data_mut(|d| d.insert_temp(kid, (n, changed)));
    }
    let figure = if n > 0 { n } else { last };
    if shown <= 0.0 || figure == 0 {
        return 0.0;
    }
    let jump_t = ((now - changed) as f32 / (0.22 * crate::tokens::PACE)).clamp(0.0, 1.0);
    if jump_t < 1.0 {
        ctx.request_repaint();
    }
    let jump = motion::keys(&[(0.0, 1.0), (0.45, 1.12), (1.0, 1.0)], Curve::Ease.at(jump_t));
    let pop = 0.6 + 0.4 * Curve::Spring.at(shown);
    let k = 1.0 + (pop * jump - 1.0) * motion::travel(ctx);
    let selected = motion::flag(ctx, id.with("count-sel"), how == Count::Selected, crate::tokens::FAST);
    let (fill, ink) = match how {
        Count::Soft => (c(C::Segment), c(C::Ink2)),
        _ => (crate::palette::mix(c(C::BadWash), c(C::AccentInk), selected), crate::palette::mix(c(C::BadInk), Color32::WHITE, selected)),
    };
    let g = ui.painter().layout_no_wrap(figure.to_string(), egui::FontId::new(crate::tokens::COUNT_TEXT * k, crate::fonts::strong()), ink.gamma_multiply(shown.min(1.0)));
    let w = (g.size().x / k + 10.0).max(18.0);
    let rest = egui::Rect::from_min_size(pos2(right - w, y - 9.0), vec2(w, 18.0));
    let rect = egui::Rect::from_center_size(rest.center(), rest.size() * k);
    ui.painter().rect_filled(rect, egui::CornerRadius::same((9.0 * k).round().clamp(0.0, 255.0) as u8), fill.gamma_multiply(shown.min(1.0)));
    ui.painter().galley(pos2(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0), g, ink);
    w * shown.min(1.0)
}

pub fn paint_count(ui: &egui::Ui, right: f32, y: f32, n: usize, how: Count) -> f32 {
    let (fill, ink) = match how {
        Count::Alert => (c(C::BadWash), c(C::BadInk)),
        Count::Selected => (c(C::AccentInk), Color32::WHITE),
        Count::Soft => (c(C::Segment), c(C::Ink2)),
    };
    let g = ui.painter().layout_no_wrap(n.to_string(), egui::FontId::new(crate::tokens::COUNT_TEXT, crate::fonts::strong()), ink);
    let w = (g.size().x + 10.0).max(18.0);
    let rect = egui::Rect::from_min_size(pos2(right - w, y - 9.0), vec2(w, 18.0));
    ui.painter().rect_filled(rect, egui::CornerRadius::same(9), fill);
    ui.painter().galley(pos2(rect.center().x - g.size().x / 2.0, rect.center().y - g.size().y / 2.0), g, ink);
    w
}
