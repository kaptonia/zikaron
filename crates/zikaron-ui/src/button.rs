//! Keys: one shape (34 high, 14 text, 16 across, a capsule), six roles, and the four phases of a long action.
//!
//! Color tells the role: blue primary (at most one per screen, `page::Page`), white secondary, a pale cinnabar
//! wash with cinnabar words for keys that lead to a confirmation card (`page::Guide`), solid cinnabar for the key
//! that commits on a confirmation card (`page::Pen`), a plain key without a frame, and the bad ink on its pale wash
//! for the last step of a deletion (`page::Erase`). There are no sizes: a key
//! is as wide as its words.
//!
//! A white key sits a hair above its card: a half-point edge and a one-point shadow; under the pointer it rises
//! a touch (a longer shadow, a lit edge), pressed it sinks. A solid key has a faint shade along its foot, a
//! half-point dark rim and a short shadow; pressed, the shade moves to its top and the shadow goes.
//!
//! A long action runs in four phases: pressed (shrinks to 0.97), in progress (the words give way to a bar
//! inside the key, filled by its fraction or sweeping when there is none), done (a check springs in and stays
//! about 400 ms) and failed (the key turns red and shakes once).

use crate::icons::{self, Glyph};
use crate::motion::{self, Curve};
use crate::palette::{self, c, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Color32, Rect};

/// The six roles. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Primary,
    Secondary,
    Guide,
    Commit,
    Plain,
    /// A deletion: the bad ink on its pale wash with a half-point bad edge.
    Danger,
}

impl Role {
    pub const ALL: [Role; 6] = [Role::Primary, Role::Secondary, Role::Guide, Role::Commit, Role::Plain, Role::Danger];

    fn solid(self) -> bool {
        matches!(self, Role::Primary | Role::Commit)
    }
}

/// A key's colors at rest, on hover and pressed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    pub fill: Color32,
    pub hover: Color32,
    pub press: Color32,
    /// The half-point edge (white, guide and deletion keys; solid keys take a dark rim of their own).
    pub line: Color32,
    pub text: Color32,
    pub hover_text: Color32,
}

/// The only source of key faces.
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

/// A solid key's shades at rest, lit and pressed: the foot's shade, the dark rim, the drop under it (alphas).
struct Solid {
    foot: [u8; 2],
    top: u8,
    rim: [u8; 2],
}

const PRIMARY: Solid = Solid { foot: [36, 41], top: 36, rim: [20, 26] };
const COMMIT: Solid = Solid { foot: [41, 46], top: 41, rim: [26, 31] };

/// The fill of the primary key and of the commit key: counting blocks of these colors counts those keys.
pub fn primary_fill() -> Color32 {
    c(C::Accent)
}

pub fn commit_fill() -> Color32 {
    c(C::Pen)
}

/// Where a long action stands.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum Phase {
    #[default]
    Idle,
    /// Running; `frac` when the work knows how far it is.
    Busy { frac: Option<f32> },
    /// Finished at frame time `at`: a check for about 400 ms, then the words come back.
    Done { at: f64 },
    /// Failed at frame time `at`: red and one shake, then the words come back.
    Fail { at: f64 },
}

/// How long a finished key shows its check, and a failed key its cross.
pub const DONE_HOLD: f64 = 0.9;
pub const FAIL_HOLD: f64 = 1.1;

impl Phase {
    /// The phase as drawn now: a done or failed phase past its hold is idle again.
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

/// One key's description.
#[derive(Clone, Copy)]
pub struct Key<'a> {
    pub text: &'a str,
    pub role: Role,
    pub enabled: bool,
    pub phase: Phase,
    /// A glyph before the words (back) and after them (a menu key's caret).
    pub lead: Option<Glyph>,
    pub trail: Option<Glyph>,
    /// What the key says while it is busy, after a small turning ring, in place of the bar inside the key
    /// (a key whose work has no measure of its own: an estimate, a fingerprint, a copy).
    pub busy_text: Option<&'a str>,
    /// Why a disabled key cannot be pressed now: shown on hover, never written beside the key.
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

    /// Say `t` with a turning ring while busy (see [`Key::busy_text`]).
    pub fn busy_text(mut self, t: &'a str) -> Self {
        self.busy_text = Some(t);
        self
    }

    /// Say `t` on hover while the key is disabled (what is still missing).
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

/// The turning ring a busy key says its words after: its radius, and the room it takes with its gap.
const BUSY_RING_R: f32 = 6.0;
const BUSY_RING_W: f32 = BUSY_RING_R * 2.0 + 8.0;

/// The words on a key: 14 in the medium face.
pub fn key_font() -> egui::FontId {
    egui::FontId::new(Type::Key.size(), crate::fonts::medium())
}

/// The size a key takes.
pub fn size_of(ui: &egui::Ui, k: &Key) -> egui::Vec2 {
    let words = |t: &str| ui.painter().layout_no_wrap(t.to_string(), font(k.role), Color32::BLACK).size().x;
    // A key is as wide as what it says now: its words, or while busy its busy saying after the ring and its gap
    // (`show_w` eases it from one width to the other as the saying changes).
    let busy = k.phase.now(crate::motion::now(ui.ctx())).busy();
    let w = match k.busy_text {
        Some(b) if busy => words(b) + BUSY_RING_W,
        _ => words(k.text),
    };
    let glyphs = [k.lead, k.trail].iter().flatten().count() as f32 * (12.0 + 6.0);
    // A menu key keeps 16 before its words and 12 after its caret.
    let pad = if k.trail.is_some() { tokens::KEY_PAD_X + 12.0 } else { tokens::KEY_PAD_X * 2.0 };
    vec2(w + glyphs + pad, tokens::KEY_H)
}

/// One key of a role, enabled or not.
pub fn key(ui: &mut egui::Ui, text: &str, role: Role, enabled: bool) -> egui::Response {
    show(ui, Key::new(text, role).enabled(enabled))
}

/// Draw a key. A disabled key and a key in progress take no clicks. Its size follows its words (growing or
/// shrinking to new words when the language changes: [`motion::size`]).
pub fn show(ui: &mut egui::Ui, k: Key) -> egui::Response {
    show_w(ui, k, 0.0)
}

/// [`show`] at least `min_w` wide (the keys of one card share the widest one's width).
pub fn show_w(ui: &mut egui::Ui, k: Key, min_w: f32) -> egui::Response {
    let natural = size_of(ui, &k);
    let size_id = ui.next_auto_id().with("zikaron-key-size");
    let mut want = motion::size(ui.ctx(), size_id, vec2(natural.x.max(min_w), natural.y));
    // A key with a busy saying eases between its words' width and its saying's as one comes for the other, at the
    // pace a size takes everywhere (a change of language eases it already).
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

/// Paint a key into `rect` (keys in toasts and table cells are placed by their owners).
pub fn paint(ui: &egui::Ui, rect: Rect, resp: &egui::Response, k: &Key, phase: Phase) {
    let ctx = ui.ctx();
    let now = crate::motion::now(ui.ctx());
    let id = resp.id;
    let f = face(k.role);
    let down = k.enabled && resp.is_pointer_button_down_on();
    let hot = k.enabled && resp.hovered();
    // One set of states for every control: hover, press, focus and enabled each ease over the same time.
    let st = motion::states(ctx, id, hot, down, resp.has_focus(), k.enabled || phase.busy());
    let press = 1.0 + (tokens::KEY_PRESS - 1.0) * st.press;
    let hover = st.hover;
    let p = ui.painter();
    let alpha = tokens::OFF + (1.0 - tokens::OFF) * st.enabled;
    let dim = |col: Color32| col.gamma_multiply(alpha);

    // Failed: red and one shake.
    let (fail, shake) = match phase {
        Phase::Fail { at } => (true, motion::shake_at(ctx, Some(at), 0.36 * crate::tokens::PACE)),
        _ => (false, 0.0),
    };
    // Hovered, the key rises: its shadow deepens (the key and its words stay where they are laid); pressed, it
    // sinks.
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
        // The short drop: 0 1 1.5 at rest, 0 2 6 lit, gone pressed.
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
        // The near drop (0 1 1 at rest, 0 1 2 lit, 0 0 1 pressed) and the soft one (0 1 3, 0 3 8, gone pressed).
        let near = (lerp(lerp(1.0, 1.0, lift), 0.0, sink), lerp(lerp(1.0, 2.0, lift), 1.0, sink));
        let far = (lerp(1.0, 3.0, lift), lerp(3.0, 8.0, lift), 1.0 - sink);
        if far.2 > 0.0 {
            p.add(crate::material::drop(r, radius, far.0, far.1, dim(c(C::KeyDrop2).gamma_multiply(far.2))));
        }
        p.add(crate::material::drop(r, radius, near.0, near.1, dim(c(C::KeyDrop))));
        p.rect_filled(r, radius, dim(fill));
        p.add(crate::material::rim(r, radius, dim(palette::mix(f.line, c(C::KeyEdgeHover), hover.max(sink)))));
    }
    // The light under the pointer: the accent at 4 % on a white key, white at 10 % on a solid one.
    let (glow, at) = crate::material::glow_at(ui, id, r, k.enabled && !phase.busy() && !fail);
    if glow > 0.0 {
        let tint = if k.role.solid() { Color32::from_white_alpha(26) } else { c(C::Accent).gamma_multiply(0.04) };
        crate::material::glow(p, r, rk, at, tint.gamma_multiply(glow));
    }
    if st.focus > 0.0 {
        p.rect_stroke(r.expand(3.0), egui::CornerRadius::same((rk + 3.0) as u8), egui::Stroke::new(3.0_f32, c(C::Focus).gamma_multiply(st.focus)), egui::StrokeKind::Inside);
    }

    // The words, and what replaces them while busy, done or failed.
    // Pressed into a long action, the words fade out (0.16 s) for the flowing bar.
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
    // The busy saying: a turning ring and the words, in place of the bar.
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
    // The check, drawn as a pen draws it (0.34 s) with one light spring (0.42 s); or the cross.
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

/// How long a long action's words take to fade out for its bar, its check to be drawn, and its check's spring.
const WORDS_OUT: f32 = 0.16 * crate::tokens::PACE;
const CHECK_DRAW: f32 = 0.34 * crate::tokens::PACE;
const CHECK_SPRING: f32 = 0.42 * crate::tokens::PACE;

/// An icon key in the toolbar and the rail: 26 (or `size`) across and round, the glyph in the second ink, a grey
/// tone on hover, pressed to 0.94.
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

/// [`paint_icon_key`] with its own corners (a copy key at the end of a row is a rounded square, 6).
pub fn paint_icon_key_at(ui: &egui::Ui, rect: Rect, resp: &egui::Response, g: Glyph, enabled: bool, corner: f32) {
    paint_icon_key_sized(ui, rect, resp, g, enabled, corner, 14.0);
}

/// [`paint_icon_key_at`] with its glyph `size` (the toolbar's history arrows draw theirs at 18).
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
