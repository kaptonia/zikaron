use crate::icons::{self, Glyph};
use crate::motion::{self, Curve};
use crate::palette::{self, c, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Color32, Rect};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Primary,
    Secondary,
    Guide,
    Commit,
    Plain,
    Danger,
}

impl Role {
    pub const ALL: [Role; 6] = [Role::Primary, Role::Secondary, Role::Guide, Role::Commit, Role::Plain, Role::Danger];

    fn solid(self) -> bool {
        matches!(self, Role::Primary | Role::Commit)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    pub fill: Color32,
    pub hover: Color32,
    pub press: Color32,
    pub line: Color32,
    pub text: Color32,
    pub hover_text: Color32,
}

pub fn face(r: Role) -> Face {
    match r {
        Role::Primary => Face { fill: c(C::Accent), hover: c(C::AccentPress), press: c(C::AccentPress), line: Color32::TRANSPARENT, text: palette::ON_SOLID, hover_text: palette::ON_SOLID },
        Role::Commit => Face { fill: c(C::Pen), hover: c(C::PenPress), press: c(C::PenPress), line: Color32::TRANSPARENT, text: palette::ON_SOLID, hover_text: palette::ON_SOLID },
        Role::Secondary => Face { fill: c(C::KeyFace), hover: c(C::KeyFaceHover), press: c(C::KeyFacePress), line: c(C::KeyEdge), text: c(C::Ink), hover_text: c(C::Ink) },
        Role::Guide => {
            let hover = palette::mix(c(C::PenWash), c(C::PenEdge), 0.3);
            Face { fill: c(C::PenWash), hover, press: hover, line: c(C::PenEdge), text: c(C::Pen), hover_text: c(C::Pen) }
        }
        Role::Plain => Face { fill: Color32::TRANSPARENT, hover: c(C::Hover), press: c(C::Hover), line: Color32::TRANSPARENT, text: c(C::Ink2), hover_text: c(C::Ink) },
        Role::Danger => {
            let hover = palette::mix(c(C::BadWash), c(C::BadEdge), 0.3);
            Face { fill: c(C::BadWash), hover, press: hover, line: c(C::BadEdge), text: c(C::BadInk), hover_text: c(C::BadInk) }
        }
    }
}

struct Solid {
    foot: [u8; 2],
    top: u8,
    rim: [u8; 2],
}

const PRIMARY: Solid = Solid { foot: [36, 41], top: 36, rim: [20, 26] };
const COMMIT: Solid = Solid { foot: [41, 46], top: 41, rim: [26, 31] };

pub fn primary_fill() -> Color32 {
    c(C::Accent)
}

pub fn commit_fill() -> Color32 {
    c(C::Pen)
}

#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Phase {
    #[default]
    Idle,
    Busy { frac: Option<f32> },
    Done { at: f64 },
    Fail { at: f64 },
}

pub const DONE_HOLD: f64 = 0.9;
pub const FAIL_HOLD: f64 = 1.1;

impl Phase {
    pub fn now(self, now: f64) -> Phase {
        match self {
            Phase::Done { at } if now - at > DONE_HOLD => Phase::Idle,
            Phase::Fail { at } if now - at > FAIL_HOLD => Phase::Idle,
            p => p,
        }
    }

    pub fn busy(self) -> bool {
        matches!(self, Phase::Busy { .. })
    }
}

#[derive(Clone, Copy)]
pub struct Key<'a> {
    pub text: &'a str,
    pub role: Role,
    pub enabled: bool,
    pub phase: Phase,
    pub lead: Option<Glyph>,
    pub trail: Option<Glyph>,
    pub busy_text: Option<&'a str>,
    pub why: Option<&'a str>,
}

impl<'a> Key<'a> {
    pub fn new(text: &'a str, role: Role) -> Key<'a> {
        Key { text, role, enabled: true, phase: Phase::Idle, lead: None, trail: None, busy_text: None, why: None }
    }

    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }

    pub fn phase(mut self, p: Phase) -> Self {
        self.phase = p;
        self
    }

    pub fn busy_text(mut self, t: &'a str) -> Self {
        self.busy_text = Some(t);
        self
    }

    pub fn why(mut self, t: &'a str) -> Self {
        if !t.is_empty() {
            self.why = Some(t);
        }
        self
    }

    pub fn lead(mut self, g: Glyph) -> Self {
        self.lead = Some(g);
        self
    }

    pub fn trail(mut self, g: Glyph) -> Self {
        self.trail = Some(g);
        self
    }
}

fn font(_role: Role) -> egui::FontId {
    key_font()
}

const BUSY_RING_R: f32 = 6.0;
const BUSY_RING_W: f32 = BUSY_RING_R * 2.0 + 8.0;

pub fn key_font() -> egui::FontId {
    egui::FontId::new(Type::Key.size(), crate::fonts::medium())
}

pub fn size_of(ui: &egui::Ui, k: &Key) -> egui::Vec2 {
    let words = |t: &str| ui.painter().layout_no_wrap(t.to_string(), font(k.role), Color32::BLACK).size().x;
    let busy = k.phase.now(crate::motion::now(ui.ctx())).busy();
    let w = match k.busy_text {
        Some(b) if busy => words(b) + BUSY_RING_W,
        _ => words(k.text),
    };
    let glyphs = [k.lead, k.trail].iter().flatten().count() as f32 * (12.0 + 6.0);
    let pad = if k.trail.is_some() { tokens::KEY_PAD_X + 12.0 } else { tokens::KEY_PAD_X * 2.0 };
    vec2(w + glyphs + pad, tokens::KEY_H)
}

pub fn key(ui: &mut egui::Ui, text: &str, role: Role, enabled: bool) -> egui::Response {
    show(ui, Key::new(text, role).enabled(enabled))
}

pub fn show(ui: &mut egui::Ui, k: Key) -> egui::Response {
    show_w(ui, k, 0.0)
}

pub fn show_w(ui: &mut egui::Ui, k: Key, min_w: f32) -> egui::Response {
    let natural = size_of(ui, &k);
    let size_id = ui.next_auto_id().with("zikaron-key-size");
    let mut want = motion::size(ui.ctx(), size_id, vec2(natural.x.max(min_w), natural.y));
    if k.busy_text.is_some() {
        let id = size_id.with("saying");
        if crate::flow::morphing(ui.ctx()) {
            motion::snap(ui.ctx(), id, want.x);
        } else {
            want.x = motion::glide(ui.ctx(), id, want.x, tokens::SLOW, motion::Curve::Ease);
        }
    }
    let now = crate::motion::now(ui.ctx());
    let phase = k.phase.now(now);
    let live = k.enabled && !phase.busy();
    let sense = if live { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(want, sense);
    if !k.enabled && !phase.busy() && k.why.is_none_or(str::is_empty) && matches!(k.role, Role::Primary | Role::Commit | Role::Danger) {
        crate::probe::grey_key(ui.ctx(), ui.layer_id(), k.text);
    }
    let start = crate::flow::mark(ui.ctx(), ui.layer_id());
    paint(ui, rect, &resp, &k, phase);
    crate::flow::note(ui, motion::key_of(&("key", k.text)), rect, start);
    if live {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else if let Some(w) = k.why.filter(|_| !k.enabled) {
        crate::layer::tip(resp, w)
    } else {
        resp
    }
}

pub fn paint(ui: &egui::Ui, rect: Rect, resp: &egui::Response, k: &Key, phase: Phase) {
    let ctx = ui.ctx();
    let now = crate::motion::now(ui.ctx());
    let id = resp.id;
    let f = face(k.role);
    let down = k.enabled && resp.is_pointer_button_down_on();
    let hot = k.enabled && resp.hovered();
    let st = motion::states(ctx, id, hot, down, resp.has_focus(), k.enabled || phase.busy());
    let press = 1.0 + (tokens::KEY_PRESS - 1.0) * st.press;
    let hover = st.hover;
    let p = ui.painter();
    let alpha = tokens::OFF + (1.0 - tokens::OFF) * st.enabled;
    let dim = |col: Color32| col.gamma_multiply(alpha);

    let (fail, shake) = match phase {
        Phase::Fail { at } => (true, motion::shake_at(ctx, Some(at), 0.36 * crate::tokens::PACE)),
        _ => (false, 0.0),
    };
    let lift = st.lift();
    let sink = st.press;
    let r = crate::paint::scaled(rect, press).translate(vec2(shake, 0.0));
    let radius = Radius::Key.egui();
    let rk = Radius::Key.px();
    let mut fill = palette::mix(palette::mix(f.fill, f.hover, hover), f.press, sink);
    let mut text = palette::mix(f.text, f.hover_text, hover);
    let solid = k.role.solid() || fail;
    if fail {
        fill = c(C::Bad);
        text = palette::ON_SOLID;
    }
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let ink = crate::material::ink;
    if solid {
        let sh = if k.role == Role::Primary { PRIMARY } else { COMMIT };
        let (y, blur, a) = (lerp(1.0, 2.0, lift), lerp(1.5, 6.0, lift), lerp(46.0, 41.0, lift) * (1.0 - sink));
        if a > 0.5 {
            p.add(crate::material::drop(r, radius, y, blur, dim(ink(a))));
        }
        p.add(crate::material::solid(r, rk, dim(fill)));
        let foot = lerp(sh.foot[0] as f32, sh.foot[1] as f32, lift) * (1.0 - sink);
        if foot > 0.5 {
            p.add(crate::material::inset_edge(r, rk, -1.0, dim(Color32::from_black_alpha(foot.round() as u8)), false));
        }
        if sink > 0.0 {
            p.add(crate::material::inset_edge(r, rk, 1.0, dim(Color32::from_black_alpha((sh.top as f32 * sink).round() as u8)), true));
        }
        let rim = lerp(sh.rim[0] as f32, sh.rim[1] as f32, lift.max(sink));
        p.add(crate::material::rim(r, radius, dim(Color32::from_black_alpha(rim.round() as u8))));
    } else if k.role == Role::Plain {
        p.rect_filled(r, radius, dim(fill));
    } else if matches!(k.role, Role::Guide | Role::Danger) {
        p.add(crate::material::drop(r, radius, 1.0, 1.0, dim(c(C::KeyDrop))));
        p.rect_filled(r, radius, dim(fill));
        p.add(crate::material::rim(r, radius, dim(f.line)));
    } else {
        let near = (lerp(lerp(1.0, 1.0, lift), 0.0, sink), lerp(lerp(1.0, 2.0, lift), 1.0, sink));
        let far = (lerp(1.0, 3.0, lift), lerp(3.0, 8.0, lift), 1.0 - sink);
        if far.2 > 0.0 {
            p.add(crate::material::drop(r, radius, far.0, far.1, dim(c(C::KeyDrop2).gamma_multiply(far.2))));
        }
        p.add(crate::material::drop(r, radius, near.0, near.1, dim(c(C::KeyDrop))));
        p.rect_filled(r, radius, dim(fill));
        p.add(crate::material::rim(r, radius, dim(palette::mix(f.line, c(C::KeyEdgeHover), hover.max(sink)))));
    }
    let (glow, at) = crate::material::glow_at(ui, id, r, k.enabled && !phase.busy() && !fail);
    if glow > 0.0 {
        let tint = if k.role.solid() { Color32::from_white_alpha(26) } else { c(C::Accent).gamma_multiply(0.04) };
        crate::material::glow(p, r, rk, at, tint.gamma_multiply(glow));
    }
    if st.focus > 0.0 {
        p.rect_stroke(r.expand(3.0), egui::CornerRadius::same((rk + 3.0) as u8), egui::Stroke::new(3.0_f32, c(C::Focus).gamma_multiply(st.focus)), egui::StrokeKind::Inside);
    }

    let busy = motion::flag(ctx, id.with("busy"), phase.busy(), WORDS_OUT);
    let done = motion::flag(ctx, id.with("done"), matches!(phase, Phase::Done { .. }), WORDS_OUT);
    let since = match phase {
        Phase::Done { at } | Phase::Fail { at } => {
            let s = (now - at) as f32;
            if s < CHECK_SPRING {
                ctx.request_repaint();
            }
            Some(s.max(0.0))
        }
        _ => None,
    };
    let words = (1.0 - busy.max(done)).min(if fail { 0.0 } else { 1.0 });
    if words > 0.0 {
        let colour = dim(text).gamma_multiply(words);
        let g = p.layout_no_wrap(k.text.to_string(), egui::FontId::new(Type::Key.size() * press, crate::fonts::medium()), colour);
        let glyph_w = 12.0 * press;
        let mut total = g.size().x;
        if k.lead.is_some() {
            total += glyph_w + 6.0;
        }
        if k.trail.is_some() {
            total += glyph_w + 6.0;
        }
        let lift = -4.0 * busy;
        let mut x = if k.trail.is_some() { r.left() + tokens::KEY_PAD_X * press } else { r.center().x - total / 2.0 };
        let y = r.center().y + lift;
        if let Some(gl) = k.lead {
            icons::glyph_at(p, gl, pos2(x + glyph_w / 2.0, y), glyph_w, colour);
            x += glyph_w + 6.0;
        }
        p.galley(pos2(x, y - g.size().y / 2.0), g, colour);
        if let Some(gl) = k.trail {
            let cc = if k.role == Role::Secondary { c(C::Ink3).gamma_multiply(alpha * words) } else { colour };
            icons::glyph_at(p, gl, pos2(r.right() - 12.0 * press - glyph_w / 2.0, y), glyph_w, cc);
        }
    }
    if busy > 0.0 && k.busy_text.is_some() {
        let colour = text.gamma_multiply(busy);
        let g = p.layout_no_wrap(k.busy_text.unwrap_or_default().to_string(), egui::FontId::new(Type::Key.size(), crate::fonts::medium()), colour);
        let total = BUSY_RING_W + g.size().x;
        let x = r.center().x - total / 2.0;
        let y = r.center().y;
        crate::mark::spinner(ctx, p, pos2(x + BUSY_RING_R, y), BUSY_RING_R, 1.6, colour, tokens::CYCLE);
        p.galley(pos2(x + BUSY_RING_W, y - g.size().y / 2.0), g, colour);
    } else if busy > 0.0 {
        let bar = Rect::from_min_max(pos2(r.left() + 14.0, r.center().y - 1.5), pos2(r.right() - 14.0, r.center().y + 1.5));
        let (track, fill_c) = if k.role.solid() {
            (Color32::from_white_alpha(77), Color32::WHITE)
        } else if k.role == Role::Guide {
            (c(C::Line), c(C::Pen))
        } else if k.role == Role::Danger {
            (c(C::Line), c(C::BadInk))
        } else {
            (c(C::Line), c(C::Accent))
        };
        p.rect_filled(bar, egui::CornerRadius::same(2), track.gamma_multiply(busy));
        let clip = p.with_clip_rect(bar);
        match phase {
            Phase::Busy { frac: Some(fr) } => {
                let shown = motion::to(ctx, id.with("frac"), fr.clamp(0.0, 1.0), tokens::MID, Curve::Ease);
                let seg = Rect::from_min_size(bar.min, vec2(bar.width() * shown, bar.height()));
                clip.rect_filled(seg, egui::CornerRadius::same(2), fill_c.gamma_multiply(busy));
            }
            _ => {
                let ph = Curve::InOut.at(motion::cycle(ctx, tokens::CYCLE));
                let w = bar.width() * 0.4;
                let left = bar.left() - w + (bar.width() + w) * ph;
                let seg = Rect::from_min_size(pos2(left, bar.top()), vec2(w, bar.height()));
                clip.rect_filled(seg, egui::CornerRadius::same(2), fill_c.gamma_multiply(busy));
            }
        }
    }
    if let Some(s) = since {
        let colour = if fail || k.role.solid() { palette::ON_SOLID } else { c(C::Ok) };
        let spring = motion::keys(&[(0.0, 0.86), (0.55, 1.06), (1.0, 1.0)], Curve::Ease.at((s / CHECK_SPRING).clamp(0.0, 1.0)));
        let size = 16.0 * (1.0 + (spring - 1.0) * motion::travel(ctx));
        if fail {
            let a = Curve::Ease.at((s / tokens::MID).clamp(0.0, 1.0));
            icons::glyph_at(p, Glyph::No, r.center(), size, colour.gamma_multiply(a));
        } else {
            icons::glyph_drawn(p, Glyph::Ok, r.center(), size, colour, Curve::Ease.at((s / CHECK_DRAW).clamp(0.0, 1.0)));
        }
    }
}

const WORDS_OUT: f32 = 0.16 * crate::tokens::PACE;
const CHECK_DRAW: f32 = 0.34 * crate::tokens::PACE;
const CHECK_SPRING: f32 = 0.42 * crate::tokens::PACE;

pub fn icon_key(ui: &mut egui::Ui, g: Glyph, size: f32, enabled: bool, tip: &str) -> egui::Response {
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), sense);
    paint_icon_key(ui, rect, &resp, g, enabled);
    let resp = if tip.is_empty() { resp } else { crate::layer::tip(resp, tip) };
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

pub fn paint_icon_key(ui: &egui::Ui, rect: Rect, resp: &egui::Response, g: Glyph, enabled: bool) {
    paint_icon_key_at(ui, rect, resp, g, enabled, Radius::Pill.px());
}

pub fn paint_icon_key_at(ui: &egui::Ui, rect: Rect, resp: &egui::Response, g: Glyph, enabled: bool, corner: f32) {
    paint_icon_key_sized(ui, rect, resp, g, enabled, corner, 14.0);
}

pub fn paint_icon_key_sized(ui: &egui::Ui, rect: Rect, resp: &egui::Response, g: Glyph, enabled: bool, corner: f32, size: f32) {
    let ctx = ui.ctx();
    let st = motion::states(ctx, resp.id, resp.hovered(), resp.is_pointer_button_down_on(), resp.has_focus(), enabled);
    let hot = st.hover;
    let press = 1.0 + (0.94 - 1.0) * st.press;
    let r = crate::paint::scaled(rect, press);
    let p = ui.painter();
    let corner = corner.min(r.width() / 2.0).min(r.height() / 2.0);
    let round = egui::CornerRadius::same(corner.round().clamp(0.0, 255.0) as u8);
    if hot > 0.0 {
        p.rect_filled(r, round, palette::mix(c(C::Tone), c(C::TonePress), st.press).gamma_multiply(hot));
    }
    let (glow, at) = crate::material::glow_at(ui, resp.id, r, enabled);
    if glow > 0.0 {
        crate::material::glow(p, r, corner, at, c(C::Accent).gamma_multiply(0.04 * glow));
    }
    let alpha = 0.35 + 0.65 * st.enabled;
    let colour = palette::mix(c(C::Ink2), c(C::Ink), hot).gamma_multiply(alpha);
    icons::glyph_at(p, g, r.center(), size * press, colour);
}
