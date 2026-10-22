//! Motion: easing curves, values that ease to a new target, entrance ages, keyframes and loops, and the one set
//! of transitions every control's states share.
//!
//! Everything here asks for the next frame only while something is moving; an idle window draws nothing.
//! Times come from egui's frame clock (`input.time`), so a headless frame with a given time moves the same
//! way as the window. A clock that runs backwards (a headless reader starting a new scene at zero) finds every
//! value already where it was going.
//!
//! **Interruptible.** A value whose target changes while it is still moving goes on from where it is and at the
//! speed it has: the new run starts at the current value with the current velocity, which then gives way to the
//! curve, so a reversed or redirected move never jumps or stops dead ([`to`]).
//!
//! **Reduced motion.** When the system asks for less motion ([`set_reduced`]), whatever moves things from place
//! to place or scales them ([`glide`], [`travel`]) arrives at once; fades stay.

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
    /// The slope at the start (d value / d progress at 0).
    fn slope0(self) -> f32 {
        match self {
            Curve::Ease => 4.0,
            Curve::Spring => 5.0,
            Curve::InOut => 0.0,
            Curve::Linear => 1.0,
        }
    }

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

/// The clock every motion in the window reads (and the window's own `now`): the time of the frame on the screen,
/// stepping evenly from frame to frame. The window does not draw at even times (on macOS through OpenGL two frames
/// fall in one refresh, one at once and one late), so a motion sampled when each frame starts moves unevenly on
/// the screen; this clock moves about one refresh a frame (the refresh read from the frames themselves), is never
/// ahead of the time nor more than two refreshes behind it, and is the time itself after a rest or a stall (a frame
/// nothing moving asked for is a rest).
pub fn now(ctx: &Context) -> f64 {
    /// How many moving frames the refresh is read from.
    const SEEN: usize = 16;
    #[derive(Clone, Copy)]
    struct Clock {
        frame: u64,
        real: f64,
        shown: f64,
        /// The spacing of the last moving frames (a ring), how many are in it, and where the next goes.
        gaps: [f64; SEEN],
        count: usize,
        next: usize,
    }
    impl Clock {
        /// The refresh: the mean spacing of the last moving frames (a sixtieth of a second until there are any).
        /// Every moving frame counts, the late with the early, so frames that come two to a refresh read as one
        /// refresh each.
        fn period(&self) -> f64 {
            if self.count == 0 { 1.0 / 60.0 } else { self.gaps[..self.count].iter().sum::<f64>() / self.count as f64 }
        }
        fn saw(mut self, dt: f64) -> Clock {
            self.gaps[self.next] = dt;
            self.next = (self.next + 1) % SEEN;
            self.count = (self.count + 1).min(SEEN);
            self
        }
    }
    let id = egui::Id::new("zikaron-motion-clock");
    let frame = ctx.cumulative_frame_nr();
    let real = ctx.input(|i| i.time);
    let last = ctx.data(|d| d.get_temp::<Clock>(id));
    if let Some(c) = last.filter(|c| c.frame == frame) {
        return c.shown;
    }
    // Only a frame the last one asked for at once (something moving) steps on from it; any other frame (a timer's,
    // an input's after a rest) is drawn at the time itself and teaches the clock no refresh.
    let moving = ctx.requested_repaint_last_pass();
    let c = match last {
        Some(c) if moving && real > c.real && real - c.real < REST => {
            let dt = real - c.real;
            let c = Clock { frame, real, ..c }.saw(dt);
            let period = c.period();
            if dt > 2.5 * period {
                // A frame more than two and a half refreshes after the last is a stall: the clock is the time again.
                Clock { shown: real, ..c }
            } else {
                // One refresh on, drawn a fifth of the way to the time itself (so it never falls behind for good).
                let step = last.map(|l| l.shown).unwrap_or(real) + period;
                Clock { shown: (step + (real - step) * 0.2).clamp(real - 2.0 * period, real), ..c }
            }
        }
        Some(c) => Clock { frame, real, shown: real, ..c },
        None => Clock { frame, real, shown: real, gaps: [0.0; SEEN], count: 0, next: 0 },
    };
    ctx.data_mut(|d| d.insert_temp(id, c));
    c.shown
}

/// A gap between frames this long is a rest, not a refresh.
const REST: f64 = 0.05;

#[derive(Clone, Copy)]
struct Tween {
    from: f32,
    to: f32,
    t0: f64,
    dur: f32,
    curve: Curve,
    /// The velocity (units a second) it was moving at when it was redirected, beyond what the curve itself
    /// starts with; it gives way over the run (zero for a run started at rest).
    carry: f32,
}

impl Tween {
    /// How far along the run is: 0 before it starts (it stands at its start), 1 once done.
    fn progress(&self, now: f64) -> f32 {
        if self.dur <= 0.0 {
            return 1.0;
        }
        ((now - self.t0) as f32 / self.dur).clamp(0.0, 1.0)
    }

    fn value(&self, now: f64) -> f32 {
        let p = self.progress(now);
        if p >= 1.0 {
            return self.to;
        }
        // p(1-p)²: starts at the carried velocity, gone (with its slope) by the end.
        self.from + (self.to - self.from) * self.curve.at(p) + self.carry * self.dur * p * (1.0 - p) * (1.0 - p)
    }

    fn velocity(&self, now: f64) -> f32 {
        if self.progress(now) >= 1.0 {
            return 0.0;
        }
        // Sampled within the run: a sample before its start would read the start, not where it was going.
        let h = 1.0 / 240.0;
        let back = (now - h).max(self.t0);
        (self.value(now + h) - self.value(back)) / ((now + h - back) as f32)
    }

    fn moving(&self, now: f64) -> bool {
        self.progress(now) < 1.0
    }

    /// A new run to `to` from where this one stands at `now`, keeping its velocity when it is moving.
    fn redirect(&self, now: f64, to: f32, dur: f32, curve: Curve) -> Tween {
        let cur = self.value(now);
        let carry = if self.moving(now) && dur > 0.0 { self.velocity(now) - (to - cur) * curve.slope0() / dur } else { 0.0 };
        Tween { from: cur, to, t0: now, dur, curve, carry }
    }
}

/// A value that eases to `target` over `dur` whenever the target changes, interruptibly (a change mid-run goes
/// on from the current value and velocity). The first call starts at the target (nothing moves on first sight;
/// entrances use [`age`]).
pub fn to(ctx: &Context, id: Id, target: f32, dur: f32, curve: Curve) -> f32 {
    to_from(ctx, id, target, target, dur, curve)
}

/// As [`to`], starting from `start` the first time it is asked (a control that should animate in).
pub fn to_from(ctx: &Context, id: Id, target: f32, start: f32, dur: f32, curve: Curve) -> f32 {
    let t = now(ctx);
    let (v, moving) = ctx.data_mut(|d| {
        let tw = d.get_temp_mut_or_insert_with(id, || Tween { from: start, to: target, t0: t, dur, curve, carry: 0.0 });
        if tw.to != target {
            *tw = tw.redirect(t, target, dur, curve);
        }
        (tw.value(t), tw.moving(t))
    });
    if moving {
        ctx.request_repaint();
    }
    v
}

/// Whether the value under `id` is still on its way.
pub fn moving(ctx: &Context, id: Id) -> bool {
    let t = now(ctx);
    ctx.data(|d| d.get_temp::<Tween>(id)).map(|tw| tw.moving(t)).unwrap_or(false)
}

/// Settle the value under `id` at `target` at once (no run).
pub fn snap(ctx: &Context, id: Id, target: f32) {
    let t = now(ctx);
    ctx.data_mut(|d| d.insert_temp(id, Tween { from: target, to: target, t0: t, dur: 0.0, curve: Curve::Ease, carry: 0.0 }));
}

fn reduced_id() -> Id {
    Id::new("zikaron-reduced-motion")
}

/// Follow the system's "reduce motion" (the window asks the system and says it here each frame).
pub fn set_reduced(ctx: &Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(reduced_id(), on));
}

/// Whether motion is reduced: moves and scales arrive at once, fades stay.
pub fn reduced(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(reduced_id())).unwrap_or(false)
}

/// How much of a move or a scale to show: 1, or 0 under reduced motion (multiply offsets and scale deltas by it).
pub fn travel(ctx: &Context) -> f32 {
    if reduced(ctx) {
        0.0
    } else {
        1.0
    }
}

/// [`to`] for a value that moves something from place to place or sizes it: under reduced motion it arrives
/// at once.
pub fn glide(ctx: &Context, id: Id, target: f32, dur: f32, curve: Curve) -> f32 {
    if reduced(ctx) {
        snap(ctx, id, target);
        return target;
    }
    to(ctx, id, target, dur, curve)
}

/// One control's states, eased the same way everywhere: hover, press, focus and enabled each go from one
/// state to the next over [`tokens::FAST`]. `lift` is how far a hovered control rises (shadow and one point up),
/// gone again while it is pressed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct States {
    pub hover: f32,
    pub press: f32,
    pub focus: f32,
    pub enabled: f32,
}

impl States {
    pub fn lift(&self) -> f32 {
        (self.hover * (1.0 - self.press)).clamp(0.0, 1.0)
    }
}

/// The eased states of the control under `id`.
pub fn states(ctx: &Context, id: Id, hovered: bool, pressed: bool, focused: bool, enabled: bool) -> States {
    States {
        hover: flag(ctx, id.with("st-hover"), hovered && enabled, tokens::FAST),
        press: flag(ctx, id.with("st-press"), pressed && enabled, tokens::FAST),
        focus: flag(ctx, id.with("st-focus"), focused, tokens::FAST),
        enabled: to_from(ctx, id.with("st-enabled"), if enabled { 1.0 } else { 0.0 }, if enabled { 1.0 } else { 0.0 }, tokens::FAST, Curve::Ease),
    }
}

/// A size that follows `want` over [`tokens::SLOW`] while [`crate::flow::morphing`] (the words changed language
/// and keys and labels grow or shrink to their new words); otherwise it is `want` at once.
pub fn size(ctx: &Context, id: Id, want: Vec2) -> Vec2 {
    if !crate::flow::morphing(ctx) {
        snap(ctx, id.with("sz-x"), want.x);
        snap(ctx, id.with("sz-y"), want.y);
        return want;
    }
    egui::vec2(glide(ctx, id.with("sz-x"), want.x, tokens::SLOW, Curve::Ease), glide(ctx, id.with("sz-y"), want.y, tokens::SLOW, Curve::Ease))
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

/// The library's pass: how many passes the app has drawn. egui runs passes the app draws nothing in (a window
/// covered by another or made small runs its logic and not its drawing, `eframe`'s `is_visible`), and everything that
/// asks "was this drawn last pass?" (an overlay's age, a layer's leaving, a sheet kept up) would read such a pass as
/// its thing having left, and play its coming again when the window shows once more. Counted here instead: a pass
/// counts the first time the library is asked in it, so only a pass the app draws in counts. Read it before taking
/// the context's data lock.
pub fn pass(ctx: &Context) -> u64 {
    #[derive(Clone, Copy, Default)]
    struct Passes {
        egui: u64,
        drawn: u64,
    }
    let now = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let p = d.get_temp_mut_or_default::<Passes>(Id::new("zikaron-drawn-pass"));
        if p.egui != now {
            p.egui = now;
            p.drawn += 1;
        }
        p.drawn
    })
}

/// Seconds since `id` appeared, or since `key` changed. Not drawn for a pass counts as leaving: the next time
/// it is drawn it appears again.
pub fn age(ctx: &Context, id: Id, key: u64) -> f32 {
    let t = now(ctx);
    let pass = crate::motion::pass(ctx);
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_insert_with(id, || Seen { key, t0: t, pass });
        if s.key != key || s.pass + 1 < pass {
            *s = Seen { key, t0: t, pass };
        } else if t < s.t0 {
            // The clock ran backwards (a headless reader's new scene): long since entered.
            s.t0 = t - 3600.0;
        }
        s.pass = pass;
        (t - s.t0) as f32
    })
}

/// Set an entrance's age now: the next [`age`] of `id` (with `key`) reads `age` (a layer that comes back while
/// it is still leaving takes its entrance up where its leaving had reached).
pub fn set_age(ctx: &Context, id: Id, key: u64, age: f32) {
    let t = now(ctx);
    let pass = crate::motion::pass(ctx);
    ctx.data_mut(|d| d.insert_temp(id, Seen { key, t0: t - f64::from(age.max(0.0)), pass }));
}

impl Curve {
    /// The time (0..=1) at which this curve reads `v`: the inverse of [`Curve::at`] for the curves that only
    /// rise (all but [`Curve::Spring`], whose overshoot reads some values twice; it gives the first).
    pub fn time_of(self, v: f32) -> f32 {
        let v = v.clamp(0.0, 1.0);
        let (mut lo, mut hi) = (0.0_f32, 1.0_f32);
        for _ in 0..40 {
            let mid = (lo + hi) / 2.0;
            if self.at(mid) < v {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        (lo + hi) / 2.0
    }
}

/// Restart an entrance: the next [`age`] of `id` counts from now.
pub fn restart(ctx: &Context, id: Id) {
    let t = now(ctx);
    let pass = crate::motion::pass(ctx);
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
    // The remainder taken in the clock's own precision: a long-running app's spinners stay smooth.
    (now(ctx).rem_euclid(period as f64) / period as f64) as f32
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
                // Under reduced motion nothing swings (the refusal is still said and coloured).
                shake(p) * travel(ctx)
            } else {
                0.0
            }
        }
        None => 0.0,
    }
}

/// How a page enters: the whole page grows from a little smaller and fades in, moving only a little. Pages
/// only enter; the page that leaves is simply gone.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Entry {
    /// Changing view from the rail: from 0.985 and 4 below, fading in, 200 ms.
    #[default]
    Root,
    /// Into a detail: from 0.99 and 12 to the right, 240 ms.
    Push,
    /// Back: from 0.99 and 12 to the left, 240 ms.
    Pop,
    /// The same page with other content (a tab): the content fades in rising from 6 below; the title and the
    /// tabs above it stay.
    Fade,
}

impl Entry {
    /// Offset and opacity at eased progress `p`.
    pub fn at(self, p: f32) -> (Vec2, f32) {
        let q = 1.0 - p;
        match self {
            Entry::Root => (egui::vec2(0.0, 4.0 * q), p),
            Entry::Push => (egui::vec2(12.0 * q, 0.0), p),
            Entry::Pop => (egui::vec2(-12.0 * q, 0.0), p),
            Entry::Fade => (egui::vec2(0.0, 6.0 * q), p),
        }
    }

    /// The page's scale at eased progress `p`.
    pub fn scale(self, p: f32) -> f32 {
        let q = 1.0 - p;
        match self {
            Entry::Root => 1.0 - 0.015 * q,
            Entry::Push | Entry::Pop => 1.0 - 0.01 * q,
            Entry::Fade => 1.0,
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
/// moment an entrance ends. They are the entrance's own (`entry`, the key of what entered): a new page, tab, step or
/// swapped content is first seen with its own size, never eased from what stood there before.
pub fn shifted<R>(ui: &mut egui::Ui, off: Vec2, alpha: f32, entry: u64, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let top = ui.cursor().min;
    let w = ui.available_width();
    let rect = egui::Rect::from_min_size(top + off, egui::vec2(w, f32::INFINITY));
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(("zikaron-entry", entry)).max_rect(rect).layout(*ui.layout()));
    if alpha < 1.0 {
        child.multiply_opacity(alpha.clamp(0.0, 1.0));
    }
    let r = add(&mut child);
    // What the block shows, not what it laid out: a block easing its height (`flow::grow`) moves the cursor back to
    // the height it shows, while the child's laid-out rectangle keeps the whole; what follows is placed from the
    // height shown, so it moves with the block rather than ahead of it.
    let used = child.min_rect();
    let shown_bottom = (child.cursor().min.y - child.spacing().item_spacing.y).min(used.bottom()).max(used.top());
    ui.advance_cursor_after_rect(egui::Rect::from_min_size(top, egui::vec2(used.width().min(w), (shown_bottom - (top.y + off.y)).max(0.0))));
    r
}

/// One block of a staggered list: each enters from 6 below, 200 ms, 30 ms after the one before (at most 200).
pub fn stagger<R>(ui: &mut egui::Ui, id: Id, key: u64, index: usize, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let delay = (index as f32 * 0.03).min(0.2) * crate::tokens::PACE;
    let p = enter(ui.ctx(), id, key, delay, tokens::MID, Curve::Ease);
    shifted(ui, egui::vec2(0.0, 6.0 * (1.0 - p)), p, key, add)
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
