use crate::tokens;
use egui::{Context, Id, Vec2};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Curve {
    Ease,
    Spring,
    InOut,
    Linear,
}

impl Curve {
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

pub fn bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    let cx = |s: f32| 3.0 * (1.0 - s) * (1.0 - s) * s * x1 + 3.0 * (1.0 - s) * s * s * x2 + s * s * s;
    let cy = |s: f32| 3.0 * (1.0 - s) * (1.0 - s) * s * y1 + 3.0 * (1.0 - s) * s * s * y2 + s * s * s;
    let dx = |s: f32| 3.0 * (1.0 - s) * (1.0 - s) * x1 + 6.0 * (1.0 - s) * s * (x2 - x1) + 3.0 * s * s * (1.0 - x2);
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

pub fn now(ctx: &Context) -> f64 {
    const SEEN: usize = 16;
    #[derive(Clone, Copy)]
    struct Clock {
        frame: u64,
        real: f64,
        shown: f64,
        gaps: [f64; SEEN],
        count: usize,
        next: usize,
    }
    impl Clock {
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
    let moving = ctx.requested_repaint_last_pass();
    let c = match last {
        Some(c) if moving && real > c.real && real - c.real < REST => {
            let dt = real - c.real;
            let c = Clock { frame, real, ..c }.saw(dt);
            let period = c.period();
            if dt > 2.5 * period {
                Clock { shown: real, ..c }
            } else {
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

const REST: f64 = 0.05;

#[derive(Clone, Copy)]
struct Tween {
    from: f32,
    to: f32,
    t0: f64,
    dur: f32,
    curve: Curve,
    carry: f32,
}

impl Tween {
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
        self.from + (self.to - self.from) * self.curve.at(p) + self.carry * self.dur * p * (1.0 - p) * (1.0 - p)
    }

    fn velocity(&self, now: f64) -> f32 {
        if self.progress(now) >= 1.0 {
            return 0.0;
        }
        let h = 1.0 / 240.0;
        let back = (now - h).max(self.t0);
        (self.value(now + h) - self.value(back)) / ((now + h - back) as f32)
    }

    fn moving(&self, now: f64) -> bool {
        self.progress(now) < 1.0
    }

    fn redirect(&self, now: f64, to: f32, dur: f32, curve: Curve) -> Tween {
        let cur = self.value(now);
        let carry = if self.moving(now) && dur > 0.0 { self.velocity(now) - (to - cur) * curve.slope0() / dur } else { 0.0 };
        Tween { from: cur, to, t0: now, dur, curve, carry }
    }
}

pub fn to(ctx: &Context, id: Id, target: f32, dur: f32, curve: Curve) -> f32 {
    to_from(ctx, id, target, target, dur, curve)
}

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

pub fn moving(ctx: &Context, id: Id) -> bool {
    let t = now(ctx);
    ctx.data(|d| d.get_temp::<Tween>(id)).map(|tw| tw.moving(t)).unwrap_or(false)
}

pub fn snap(ctx: &Context, id: Id, target: f32) {
    let t = now(ctx);
    ctx.data_mut(|d| d.insert_temp(id, Tween { from: target, to: target, t0: t, dur: 0.0, curve: Curve::Ease, carry: 0.0 }));
}

fn reduced_id() -> Id {
    Id::new("zikaron-reduced-motion")
}

pub fn set_reduced(ctx: &Context, on: bool) {
    ctx.data_mut(|d| d.insert_temp(reduced_id(), on));
}

pub fn reduced(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(reduced_id())).unwrap_or(false)
}

pub fn travel(ctx: &Context) -> f32 {
    if reduced(ctx) {
        0.0
    } else {
        1.0
    }
}

pub fn glide(ctx: &Context, id: Id, target: f32, dur: f32, curve: Curve) -> f32 {
    if reduced(ctx) {
        snap(ctx, id, target);
        return target;
    }
    to(ctx, id, target, dur, curve)
}

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

pub fn states(ctx: &Context, id: Id, hovered: bool, pressed: bool, focused: bool, enabled: bool) -> States {
    States {
        hover: flag(ctx, id.with("st-hover"), hovered && enabled, tokens::FAST),
        press: flag(ctx, id.with("st-press"), pressed && enabled, tokens::FAST),
        focus: flag(ctx, id.with("st-focus"), focused, tokens::FAST),
        enabled: to_from(ctx, id.with("st-enabled"), if enabled { 1.0 } else { 0.0 }, if enabled { 1.0 } else { 0.0 }, tokens::FAST, Curve::Ease),
    }
}

pub fn size(ctx: &Context, id: Id, want: Vec2) -> Vec2 {
    if !crate::flow::morphing(ctx) {
        snap(ctx, id.with("sz-x"), want.x);
        snap(ctx, id.with("sz-y"), want.y);
        return want;
    }
    egui::vec2(glide(ctx, id.with("sz-x"), want.x, tokens::SLOW, Curve::Ease), glide(ctx, id.with("sz-y"), want.y, tokens::SLOW, Curve::Ease))
}

pub fn flag(ctx: &Context, id: Id, on: bool, dur: f32) -> f32 {
    to(ctx, id, if on { 1.0 } else { 0.0 }, dur, Curve::Ease)
}

#[derive(Clone, Copy)]
struct Seen {
    key: u64,
    t0: f64,
    pass: u64,
}

pub fn pass(ctx: &Context) -> u64 {
    let now = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let p = d.get_temp_mut_or_default::<Passes>(passes_id());
        if !p.logic && p.egui != now {
            p.egui = now;
            p.drawn += 1;
        }
        p.drawn
    })
}

pub fn drawn(ctx: &Context) -> u64 {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Passes>(passes_id()).drawn)
}

#[derive(Clone, Copy, Default)]
struct Passes {
    egui: u64,
    drawn: u64,
    logic: bool,
}

fn passes_id() -> Id {
    Id::new("zikaron-drawn-pass")
}

pub struct Logic(Context);

impl Logic {
    pub fn begin(ctx: &Context) -> Logic {
        ctx.data_mut(|d| d.get_temp_mut_or_default::<Passes>(passes_id()).logic = true);
        Logic(ctx.clone())
    }
}

impl Drop for Logic {
    fn drop(&mut self) {
        self.0.data_mut(|d| d.get_temp_mut_or_default::<Passes>(passes_id()).logic = false);
    }
}

pub fn age(ctx: &Context, id: Id, key: u64) -> f32 {
    let t = now(ctx);
    let pass = crate::motion::pass(ctx);
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_insert_with(id, || Seen { key, t0: t, pass });
        if s.key != key || s.pass + 1 < pass {
            *s = Seen { key, t0: t, pass };
        } else if t < s.t0 {
            s.t0 = t - 3600.0;
        }
        s.pass = pass;
        (t - s.t0) as f32
    })
}

pub fn started(ctx: &Context, id: Id) -> Option<f64> {
    ctx.data(|d| d.get_temp::<Seen>(id)).map(|s| s.t0)
}

pub fn set_age(ctx: &Context, id: Id, key: u64, age: f32) {
    let t = now(ctx);
    let pass = crate::motion::pass(ctx);
    ctx.data_mut(|d| d.insert_temp(id, Seen { key, t0: t - f64::from(age.max(0.0)), pass }));
}

impl Curve {
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

pub fn enter(ctx: &Context, id: Id, key: u64, delay: f32, dur: f32, curve: Curve) -> f32 {
    let a = age(ctx, id, key) - delay;
    let p = (a / dur).clamp(0.0, 1.0);
    if p < 1.0 {
        ctx.request_repaint();
    }
    curve.at(p)
}

pub fn cycle(ctx: &Context, period: f32) -> f32 {
    ctx.request_repaint();
    (now(ctx).rem_euclid(period as f64) / period as f64) as f32
}

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

pub fn shake(t: f32) -> f32 {
    keys(&[(0.0, 0.0), (0.15, -4.0), (0.35, 4.0), (0.55, -3.0), (0.75, 2.0), (1.0, 0.0)], t)
}

pub fn shake_at(ctx: &Context, at: Option<f64>, dur: f32) -> f32 {
    match at {
        Some(a) => {
            let p = ((now(ctx) - a) as f32) / dur;
            if p < 1.0 {
                ctx.request_repaint();
                shake(p) * travel(ctx)
            } else {
                0.0
            }
        }
        None => 0.0,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Entry {
    #[default]
    Root,
    Push,
    Pop,
    Fade,
}

impl Entry {
    pub fn at(self, p: f32) -> (Vec2, f32) {
        let q = 1.0 - p;
        match self {
            Entry::Root => (egui::vec2(0.0, 4.0 * q), p),
            Entry::Push => (egui::vec2(12.0 * q, 0.0), p),
            Entry::Pop => (egui::vec2(-12.0 * q, 0.0), p),
            Entry::Fade => (egui::vec2(0.0, 6.0 * q), p),
        }
    }

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

pub fn shifted<R>(ui: &mut egui::Ui, off: Vec2, alpha: f32, entry: u64, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let top = ui.cursor().min;
    let w = ui.available_width();
    let rect = egui::Rect::from_min_size(top + off, egui::vec2(w, f32::INFINITY));
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(("zikaron-entry", entry)).max_rect(rect).layout(*ui.layout()));
    if alpha < 1.0 {
        child.multiply_opacity(alpha.clamp(0.0, 1.0));
    }
    let r = add(&mut child);
    let used = child.min_rect();
    let shown_bottom = (child.cursor().min.y - child.spacing().item_spacing.y).min(used.bottom()).max(used.top());
    ui.advance_cursor_after_rect(egui::Rect::from_min_size(top, egui::vec2(used.width().min(w), (shown_bottom - (top.y + off.y)).max(0.0))));
    r
}

pub fn stagger<R>(ui: &mut egui::Ui, id: Id, key: u64, index: usize, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let delay = (index as f32 * 0.03).min(0.2) * crate::tokens::PACE;
    let p = enter(ui.ctx(), id, key, delay, tokens::MID, Curve::Ease);
    shifted(ui, egui::vec2(0.0, 6.0 * (1.0 - p)), p, key, add)
}

pub fn changed_age(ctx: &Context, id: Id, key: u64) -> f32 {
    let t = now(ctx);
    let pass = crate::motion::pass(ctx);
    ctx.data_mut(|d| {
        let s = d.get_temp_mut_or_insert_with(id, || Seen { key, t0: t - 3600.0, pass });
        if s.key != key {
            *s = Seen { key, t0: t, pass };
        } else if t < s.t0 {
            s.t0 = t - 3600.0;
        }
        s.pass = pass;
        (t - s.t0) as f32
    })
}

pub fn swap<R>(ui: &mut egui::Ui, id: Id, key: u64, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let p = (changed_age(ui.ctx(), id, key) / tokens::MID).clamp(0.0, 1.0);
    if p < 1.0 {
        ui.ctx().request_repaint();
    }
    let p = Curve::Ease.at(p);
    shifted(ui, egui::vec2(0.0, 6.0 * (1.0 - p)), p, key, add)
}

pub fn key_of<T: std::hash::Hash>(v: &T) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    v.hash(&mut h);
    h.finish()
}
