//! Motion: easing curves, values that ease to a new target, entrance ages, keyframes and loops.
//!
//! Everything here asks for the next frame only while something is moving; an idle window draws nothing.
//! Times come from egui's frame clock (`input.time`), so a headless frame with a given time moves the same
//! way as the window.

use crate::tokens;
use egui::{Context, Id, Vec2};

/// Easing curves. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Curve {
    /// `cubic-bezier(.2,.8,.2,1)`: every ordinary transition.
    Ease,
    /// `cubic-bezier(.3,1.5,.5,1)`: checks, switch knobs, pills popping in (overshoots a little).
    Spring,
    /// `ease-in-out`: loops (the sweep and the breathing dot).
    InOut,
    Linear,
}

impl Curve {
    pub fn at(self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Curve::Ease => bezier(0.2, 0.8, 0.2, 1.0, t),
            Curve::Spring => bezier(0.3, 1.5, 0.5, 1.0, t),
            Curve::InOut => bezier(0.42, 0.0, 0.58, 1.0, t),
            Curve::Linear => t,
        }
    }
}

/// A CSS cubic Bézier from (0,0) to (1,1) with control points (x1,y1) and (x2,y2), read at `x = t`.
pub fn bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    let cx = |s: f32| 3.0 * (1.0 - s) * (1.0 - s) * s * x1 + 3.0 * (1.0 - s) * s * s * x2 + s * s * s;
    let cy = |s: f32| 3.0 * (1.0 - s) * (1.0 - s) * s * y1 + 3.0 * (1.0 - s) * s * s * y2 + s * s * s;
    let dx = |s: f32| 3.0 * (1.0 - s) * (1.0 - s) * x1 + 6.0 * (1.0 - s) * s * (x2 - x1) + 3.0 * s * s * (1.0 - x2);
    // Newton first, bisection when the slope is too flat to trust.
    let mut s = t;
    for _ in 0..8 {
        let e = cx(s) - t;
        if e.abs() < 1e-5 {
            return cy(s);
        }
        let d = dx(s);
        if d.abs() < 1e-6 {
            break;
        }
        s = (s - e / d).clamp(0.0, 1.0);
    }
    let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
    s = t;
    for _ in 0..30 {
        let x = cx(s);
        if (x - t).abs() < 1e-5 {
            break;
        }
        if x < t {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) / 2.0;
    }
    cy(s)
}

fn now(ctx: &Context) -> f64 {
    ctx.input(|i| i.time)
}

#[derive(Clone, Copy)]
struct Tween {
    from: f32,
    to: f32,
    t0: f64,
    dur: f32,
    curve: Curve,
}

impl Tween {
    fn value(&self, now: f64) -> f32 {
        if self.dur <= 0.0 {
            return self.to;
        }
        let p = ((now - self.t0) as f32 / self.dur).clamp(0.0, 1.0);
        self.from + (self.to - self.from) * self.curve.at(p)
    }

    fn moving(&self, now: f64) -> bool {
        ((now - self.t0) as f32) < self.dur
    }
}

/// A value that eases to `target` over `dur` whenever the target changes. The first call starts at the
/// target (nothing moves on first sight; entrances use [`age`]).
pub fn to(ctx: &Context, id: Id, target: f32, dur: f32, curve: Curve) -> f32 {
    to_from(ctx, id, target, target, dur, curve)
}

/// As [`to`], starting from `start` the first time it is asked (a control that should animate in).
pub fn to_from(ctx: &Context, id: Id, target: f32, start: f32, dur: f32, curve: Curve) -> f32 {
    let t = now(ctx);
    let (v, moving) = ctx.data_mut(|d| {
        let tw = d.get_temp_mut_or_insert_with(id, || Tween { from: start, to: target, t0: t, dur, curve });
        if tw.to != target {
            let cur = tw.value(t);
            *tw = Tween { from: cur, to: target, t0: t, dur, curve };
        }
        (tw.value(t), tw.moving(t))
    });
    if moving {
        ctx.request_repaint();
    }
    v
}

/// 0 or 1 eased: a hover, press or open state.
pub fn flag(ctx: &Context, id: Id, on: bool, dur: f32) -> f32 {
    to(ctx, id, if on { 1.0 } else { 0.0 }, dur, Curve::Ease)
}

#[derive(Clone, Copy)]
struct Seen {
    key: u64,
    t0: f64,
    pass: u64,
}

/// Seconds since `id` appeared, or since `key` changed. Not drawn for a pass counts as leaving: the next time
/// it is drawn it appears again.
pub fn age(ctx: &Context, id: Id, key: u64) -> f32 {
    let t = now(ctx);
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_insert_with(id, || Seen { key, t0: t, pass });
        if s.key != key || s.pass + 1 < pass {
            *s = Seen { key, t0: t, pass };
        }
        s.pass = pass;
        (t - s.t0) as f32
    })
}

/// Restart an entrance: the next [`age`] of `id` counts from now.
pub fn restart(ctx: &Context, id: Id) {
    let t = now(ctx);
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        if let Some(mut s) = d.get_temp::<Seen>(id) {
            s.t0 = t;
            s.pass = pass;
            d.insert_temp(id, s);
        }
    });
}

/// Eased progress of an entrance that starts `delay` after `id` appeared and runs `dur`; asks for frames
/// until done.
pub fn enter(ctx: &Context, id: Id, key: u64, delay: f32, dur: f32, curve: Curve) -> f32 {
    let a = age(ctx, id, key) - delay;
    let p = (a / dur).clamp(0.0, 1.0);
    if p < 1.0 {
        ctx.request_repaint();
    }
    curve.at(p)
}

/// The phase of a loop of `period` seconds (0..1). Calling it asks for the next frame, so call it only while
/// the loop is on screen.
pub fn cycle(ctx: &Context, period: f32) -> f32 {
    ctx.request_repaint();
    ((now(ctx) as f32) / period).fract()
}

/// Keyframes by linear pieces: `(at, value)` sorted by `at` in 0..1.
pub fn keys(frames: &[(f32, f32)], t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    for w in frames.windows(2) {
        let (a, va) = w[0];
        let (b, vb) = w[1];
        if t <= b {
            let p = if b > a { (t - a) / (b - a) } else { 1.0 };
            return va + (vb - va) * p;
        }
    }
    frames.last().map(|x| x.1).unwrap_or(0.0)
}

/// The shake of a refused input or a failed key: 360 ms, four swings.
pub fn shake(t: f32) -> f32 {
    keys(&[(0.0, 0.0), (0.15, -4.0), (0.35, 4.0), (0.55, -3.0), (0.75, 2.0), (1.0, 0.0)], t)
}

/// Horizontal shake offset of `id` started at frame time `at` (`dur` 0.36 for keys and sheets, 0.4 for the
/// passcode row); zero when not shaking.
pub fn shake_at(ctx: &Context, at: Option<f64>, dur: f32) -> f32 {
    match at {
        Some(a) => {
            let p = ((now(ctx) - a) as f32) / dur;
            if p < 1.0 {
                ctx.request_repaint();
                shake(p)
            } else {
                0.0
            }
        }
        None => 0.0,
    }
}

/// How a page enters. Pages only enter; the page that leaves is simply gone.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Entry {
    /// Changing view from the rail: from 8 below, fading in, 200 ms.
    #[default]
    Root,
    /// Into a detail: from 24 to the right, 240 ms.
    Push,
    /// Back: from 24 to the left, 240 ms.
    Pop,
    /// The same page redrawn with new content: fade only.
    Fade,
}

impl Entry {
    /// Offset and opacity at eased progress `p`.
    pub fn at(self, p: f32) -> (Vec2, f32) {
        let q = 1.0 - p;
        match self {
            Entry::Root => (egui::vec2(0.0, 8.0 * q), p),
            Entry::Push => (egui::vec2(24.0 * q, 0.0), p),
            Entry::Pop => (egui::vec2(-24.0 * q, 0.0), p),
            Entry::Fade => (Vec2::ZERO, p),
        }
    }

    pub fn dur(self) -> f32 {
        match self {
            Entry::Push | Entry::Pop => tokens::PUSH,
            Entry::Root | Entry::Fade => tokens::MID,
        }
    }
}

/// Draw `add` moved by `off` and faded to `alpha`, while taking the space it would take unmoved. Input
/// follows the drawing, so a moving block is clicked where it is seen. Moving or at rest, the block is drawn in
/// the same child: the ids of what it holds (their open states, their animations' clocks) do not change the
/// moment an entrance ends.
pub fn shifted<R>(ui: &mut egui::Ui, off: Vec2, alpha: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let top = ui.cursor().min;
    let w = ui.available_width();
    let rect = egui::Rect::from_min_size(top + off, egui::vec2(w, f32::INFINITY));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(*ui.layout()));
    if alpha < 1.0 {
        child.multiply_opacity(alpha.clamp(0.0, 1.0));
    }
    let r = add(&mut child);
    let used = child.min_rect();
    ui.advance_cursor_after_rect(egui::Rect::from_min_size(top, egui::vec2(used.width().min(w), used.height())));
    r
}

/// One block of a staggered list: each enters from 6 below, 200 ms, 30 ms after the one before (at most 200).
pub fn stagger<R>(ui: &mut egui::Ui, id: Id, key: u64, index: usize, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let delay = (index as f32 * 0.03).min(0.2);
    let p = enter(ui.ctx(), id, key, delay, tokens::MID, Curve::Ease);
    shifted(ui, egui::vec2(0.0, 6.0 * (1.0 - p)), p, add)
}

/// A block swapped in with new content: fades in from 6 below over 200 ms whenever `key` changes.
pub fn swap<R>(ui: &mut egui::Ui, id: Id, key: u64, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    stagger(ui, id, key, 0, add)
}

/// A key for a piece of text or any hashable value (entrance keys).
pub fn key_of<T: std::hash::Hash>(v: &T) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}
