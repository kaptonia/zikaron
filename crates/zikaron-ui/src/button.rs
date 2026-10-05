//! Keys: one shape (34 high, 14 text, 16 across, corner 8), six roles, and the four phases of a long action.
//!
//! Color tells the role: blue primary (at most one per screen, `page::Page`), white secondary, white with
//! cinnabar words for keys that lead to a confirmation card (`page::Guide`), solid cinnabar for the key that
//! commits on a confirmation card (`page::Pen`), a link, and a plain key without a frame. There are no sizes:
//! a key is as wide as its words.
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
    Link,
    Plain,
}

impl Role {
    pub const ALL: [Role; 6] = [Role::Primary, Role::Secondary, Role::Guide, Role::Commit, Role::Link, Role::Plain];

    fn solid(self) -> bool {
        matches!(self, Role::Primary | Role::Commit)
    }
}

/// A key's colors at rest and on hover.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    pub fill: Color32,
    pub hover: Color32,
    pub line: Color32,
    pub text: Color32,
    pub hover_text: Color32,
}

/// The only source of key faces.
pub fn face(r: Role) -> Face {
    match r {
        Role::Primary => Face { fill: c(C::Accent), hover: c(C::AccentPress), line: c(C::Accent), text: palette::ON_SOLID, hover_text: palette::ON_SOLID },
        Role::Commit => Face { fill: c(C::Pen), hover: c(C::PenPress), line: c(C::Pen), text: palette::ON_SOLID, hover_text: palette::ON_SOLID },
        Role::Secondary => Face { fill: c(C::Surface), hover: c(C::Hover), line: c(C::Line), text: c(C::Ink), hover_text: c(C::Ink) },
        Role::Guide => Face { fill: c(C::Surface), hover: c(C::PenWash), line: c(C::PenEdge), text: c(C::Pen), hover_text: c(C::Pen) },
        Role::Plain => Face { fill: Color32::TRANSPARENT, hover: c(C::Hover), line: Color32::TRANSPARENT, text: c(C::Ink2), hover_text: c(C::Ink) },
        Role::Link => Face { fill: Color32::TRANSPARENT, hover: Color32::TRANSPARENT, line: Color32::TRANSPARENT, text: c(C::AccentInk), hover_text: c(C::AccentInk) },
    }
}

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
}

impl<'a> Key<'a> {
    pub fn new(text: &'a str, role: Role) -> Key<'a> {
        Key { text, role, enabled: true, phase: Phase::Idle, lead: None, trail: None, busy_text: None }
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
    // A key that says something else while busy is as wide as the longer of its two sayings (the ring and its
    // gap included), so it does not change width when it starts.
    let w = words(k.text).max(k.busy_text.map(|b| words(b) + BUSY_RING_W).unwrap_or(0.0));
    let glyphs = [k.lead, k.trail].iter().flatten().count() as f32 * (12.0 + 6.0);
    if k.role == Role::Link {
        return vec2(w + glyphs, Type::Key.line());
    }
    // A menu key keeps 16 before its words and 12 after its caret.
    let pad = if k.trail.is_some() { tokens::KEY_PAD_X + 12.0 } else { tokens::KEY_PAD_X * 2.0 };
    vec2(w + glyphs + pad, tokens::KEY_H)
}

/// One key of a role, enabled or not.
pub fn key(ui: &mut egui::Ui, text: &str, role: Role, enabled: bool) -> egui::Response {
    show(ui, Key::new(text, role).enabled(enabled))
}

/// Draw a key. A disabled key and a key in progress take no clicks.
pub fn show(ui: &mut egui::Ui, k: Key) -> egui::Response {
    let want = size_of(ui, &k);
    let now = ui.input(|i| i.time);
    let phase = k.phase.now(now);
    let live = k.enabled && !phase.busy();
    let sense = if live { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(want, sense);
    paint(ui, rect, &resp, &k, phase);
    if live {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Paint a key into `rect` (keys in toasts and table cells are placed by their owners).
pub fn paint(ui: &egui::Ui, rect: Rect, resp: &egui::Response, k: &Key, phase: Phase) {
    let ctx = ui.ctx();
    let now = ui.input(|i| i.time);
    let id = resp.id;
    let f = face(k.role);
    let down = k.enabled && resp.is_pointer_button_down_on();
    let hot = k.enabled && resp.hovered();
    let press = motion::to(ctx, id.with("press"), if down { tokens::KEY_PRESS } else { 1.0 }, tokens::FAST, Curve::Ease);
    let hover = motion::flag(ctx, id.with("hover"), hot, tokens::FAST);
    let p = ui.painter();
    let alpha = if k.enabled || phase.busy() { 1.0 } else { tokens::OFF };
    let dim = |col: Color32| col.gamma_multiply(alpha);

    if k.role == Role::Link {
        let colour = dim(f.text);
        let g = p.layout_no_wrap(k.text.to_string(), font(k.role), colour);
        let pos = pos2(rect.left(), rect.center().y - g.size().y / 2.0);
        let w = g.size().x;
        p.galley(pos, g, colour);
        if hot {
            p.hline(rect.left()..=rect.left() + w, rect.center().y + 8.0, egui::Stroke::new(1.0_f32, colour));
        }
        return;
    }

    // Failed: red and one shake.
    let (fail, shake) = match phase {
        Phase::Fail { at } => (true, motion::shake_at(ctx, Some(at), 0.36)),
        _ => (false, 0.0),
    };
    let r = crate::paint::scaled(rect, press).translate(vec2(shake, 0.0));
    let radius = Radius::Ctl.egui();
    let mut fill = palette::mix(f.fill, f.hover, hover);
    let mut line = f.line;
    let mut text = palette::mix(f.text, f.hover_text, hover);
    if fail {
        fill = c(C::Bad);
        line = c(C::Bad);
        text = palette::ON_SOLID;
    }
    if k.enabled && !matches!(k.role, Role::Plain) {
        let drop = match (k.role, palette::blend() > 0.5) {
            (Role::Primary, _) => Color32::from_rgba_unmultiplied(37, 99, 235, 71),
            (Role::Commit, _) => Color32::from_rgba_unmultiplied(168, 58, 44, 71),
            (_, false) => Color32::from_rgba_unmultiplied(20, 20, 30, 18),
            (_, true) => Color32::from_rgba_unmultiplied(0, 0, 0, 102),
        };
        p.add(egui::Shadow { offset: [0, 1], blur: 2, spread: 0, color: drop }.as_shape(r, radius));
    }
    p.rect(r, radius, dim(fill), egui::Stroke::new(1.0_f32, dim(line)), egui::StrokeKind::Inside);
    if k.enabled && !k.role.solid() && !matches!(k.role, Role::Plain) && !fail {
        // The inset lip under white keys.
        let span = (r.left() + 8.0)..=(r.right() - 8.0);
        p.hline(span, r.bottom() - 1.5, egui::Stroke::new(1.0_f32, Color32::from_black_alpha(10)));
    }
    if resp.has_focus() {
        p.rect_stroke(r.expand(3.0), egui::CornerRadius::same(11), egui::Stroke::new(3.0_f32, c(C::Focus)), egui::StrokeKind::Inside);
    }

    // The words, and what replaces them while busy, done or failed.
    let busy = motion::flag(ctx, id.with("busy"), phase.busy(), tokens::FAST);
    let done = motion::flag(ctx, id.with("done"), matches!(phase, Phase::Done { .. }), tokens::FAST);
    let mark_in = match phase {
        Phase::Done { at } | Phase::Fail { at } => {
            let q = (((now - at) as f32) / tokens::MID).clamp(0.0, 1.0);
            if q < 1.0 {
                ctx.request_repaint();
            }
            q
        }
        _ => 0.0,
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
    // The check or the cross.
    if mark_in > 0.0 {
        let k2 = 0.5 + 0.5 * Curve::Spring.at(mark_in);
        let colour = if fail || k.role.solid() { palette::ON_SOLID } else { c(C::Ok) };
        let g = if fail { Glyph::No } else { Glyph::Ok };
        icons::glyph_at(p, g, r.center(), 16.0 * k2, colour.gamma_multiply(mark_in.min(1.0)));
    }
}

/// A link: words in the link blue, underlined on hover.
pub fn link(ui: &mut egui::Ui, text: &str) -> egui::Response {
    key(ui, text, Role::Link, true)
}

/// An icon key in the toolbar and the rail: 26 (or `size`) square, the glyph in the second ink, hover ground,
/// pressed to 0.94.
pub fn icon_key(ui: &mut egui::Ui, g: Glyph, size: f32, enabled: bool, tip: &str) -> egui::Response {
    let sense = if enabled { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(vec2(size, size), sense);
    paint_icon_key(ui, rect, &resp, g, enabled);
    let resp = if tip.is_empty() { resp } else { resp.on_hover_text(tip) };
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

pub fn paint_icon_key(ui: &egui::Ui, rect: Rect, resp: &egui::Response, g: Glyph, enabled: bool) {
    let ctx = ui.ctx();
    let hot = motion::flag(ctx, resp.id.with("hover"), enabled && resp.hovered(), tokens::FAST);
    let press = motion::to(ctx, resp.id.with("press"), if enabled && resp.is_pointer_button_down_on() { 0.94 } else { 1.0 }, tokens::FAST, Curve::Ease);
    let r = crate::paint::scaled(rect, press);
    let p = ui.painter();
    if hot > 0.0 {
        p.rect_filled(r, egui::CornerRadius::same(7), c(C::Hover).gamma_multiply(hot));
    }
    let alpha = if enabled { 1.0 } else { 0.35 };
    let colour = palette::mix(c(C::Ink2), c(C::Ink), hot).gamma_multiply(alpha);
    icons::glyph_at(p, g, r.center(), 14.0 * press, colour);
}
