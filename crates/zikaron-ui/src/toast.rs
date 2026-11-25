use crate::button::{self, Key, Role};
use crate::icons::{self, Glyph};
use crate::motion::{self, Curve};
use crate::palette::{self, c, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Note,
    Bad,
    Alert,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Act {
    Copy(String),
    Tag(u64),
}

#[derive(Clone, Debug)]
struct Slip {
    id: u64,
    text: String,
    next: String,
    raw: String,
    tone: Tone,
    born: f64,
    entered: f64,
    bumped: f64,
    open: bool,
    keys: Vec<(String, Act)>,
    said: f64,
}

pub const LIFE_NOTE: f64 = 3.2;
pub const LIFE_BAD: f64 = 6.0;
const IN: f32 = 0.5 * crate::tokens::PACE;
const NARROW: f32 = 0.55;
const WORDS_FROM: f32 = 0.18 * crate::tokens::PACE;
const WORDS_IN: f32 = 0.24 * crate::tokens::PACE;
const OUT: f32 = 0.26 * crate::tokens::PACE;
const LEAVE_TO: f32 = 0.85;

impl Slip {
    fn life(&self) -> f64 {
        match self.tone {
            Tone::Note => LIFE_NOTE,
            Tone::Bad | Tone::Alert => LIFE_BAD,
        }
    }
}

#[derive(Default)]
pub struct Toasts {
    cur: Option<Slip>,
    queued: std::collections::VecDeque<Slip>,
    leaving: Vec<(Slip, f64)>,
    minted: u64,
    labels: [String; 3],
}

#[derive(Default)]
pub struct Drawn {
    pub rects: Vec<Rect>,
    pub pressed: Option<u64>,
}

impl Toasts {
    pub fn new() -> Toasts {
        Toasts::default()
    }

    pub fn set_labels(&mut self, detail: &str, hide: &str, close: &str) {
        self.labels = [detail.to_string(), hide.to_string(), close.to_string()];
    }

    pub fn say(&mut self, text: impl Into<String>, tone: Tone, now: f64) {
        self.say_keys(text, "", "", tone, now, Vec::new());
    }

    pub fn say_full(&mut self, text: impl Into<String>, next: &str, raw: &str, tone: Tone, now: f64) {
        self.say_keys(text, next, raw, tone, now, Vec::new());
    }

    pub fn say_with(&mut self, text: impl Into<String>, next: &str, raw: &str, tone: Tone, now: f64, act: Option<(String, String)>) {
        let keys = act.map(|(l, t)| vec![(l, Act::Copy(t))]).unwrap_or_default();
        self.say_keys(text, next, raw, tone, now, keys);
    }

    pub fn say_keys(&mut self, text: impl Into<String>, next: &str, raw: &str, tone: Tone, now: f64, keys: Vec<(String, Act)>) {
        let text = text.into();
        if let Some(cur) = self.cur.as_mut() {
            if cur.text == text {
                cur.born = now;
                cur.bumped = now;
                return;
            }
            if cur.said == now {
                if !self.queued.iter().any(|q| q.text == text) {
                    self.minted += 1;
                    self.queued.push_back(Slip { id: self.minted, text, next: next.to_string(), raw: raw.to_string(), tone, born: now, entered: now, bumped: f64::NEG_INFINITY, open: false, keys, said: now });
                }
                return;
            }
        }
        self.queued.clear();
        if let Some(old) = self.cur.take() {
            self.leaving.push((old, now));
        }
        let enter = self.leaving.iter().map(|(_, at)| at + OUT as f64).fold(now, f64::max);
        self.minted += 1;
        self.cur = Some(Slip { id: self.minted, text, next: next.to_string(), raw: raw.to_string(), tone, born: enter, entered: enter, bumped: f64::NEG_INFINITY, open: false, keys, said: now });
    }

    fn next_in(&mut self, now: f64) {
        if let Some(mut q) = self.queued.pop_front() {
            let enter = now + OUT as f64;
            q.born = enter;
            q.entered = enter;
            self.cur = Some(q);
        }
    }

    pub fn waiting(&self) -> usize {
        self.queued.len()
    }

    pub fn live(&self) -> usize {
        usize::from(self.cur.is_some())
    }

    pub fn showing(&self) -> Option<&str> {
        self.cur.as_ref().map(|s| s.text.as_str())
    }

    pub fn showing_next(&self) -> Option<&str> {
        self.cur.as_ref().map(|s| s.next.as_str())
    }

    pub fn showing_keys(&self) -> Vec<(String, Act)> {
        self.cur.as_ref().map(|s| s.keys.clone()).unwrap_or_default()
    }

    pub fn showing_tone(&self) -> Option<Tone> {
        self.cur.as_ref().map(|s| s.tone)
    }

    pub fn draw(&mut self, ctx: &egui::Context, left: f32) -> Drawn {
        let now = crate::motion::now(ctx);
        let mut out = Drawn::default();
        if let Some(cur) = &self.cur {
            if !cur.open && now - cur.born > cur.life() {
                let s = self.cur.take().expect("checked above");
                self.leaving.push((s, now));
                self.next_in(now);
            }
        }
        self.leaving.retain(|(_, at)| now - at < OUT as f64);
        if let Some(cur) = &self.cur {
            if !cur.open {
                let left_s = cur.life() - (now - cur.born);
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(left_s.max(0.02)));
            }
        }
        if !self.leaving.is_empty() {
            ctx.request_repaint();
        }
        let screen = ctx.content_rect();
        let center_x = (left + screen.right()) / 2.0;
        let labels = self.labels.clone();
        let mut close = false;
        let mut flip = false;
        let travel = motion::travel(ctx);
        for (s, at) in self.leaving.clone() {
            let p = ((now - at) as f32 / OUT).clamp(0.0, 1.0);
            let e = Curve::Ease.at(1.0 - p);
            let wide = 1.0 - (1.0 - LEAVE_TO) * (1.0 - e) * travel;
            let r = paint_slip(ctx, &s, center_x, screen, &labels, Shown { card: e, words: e, wide, scale: 1.0 }, false);
            out.rects.push(r.0);
        }
        if let Some(s) = self.cur.as_ref().filter(|s| now < s.entered) {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((s.entered - now).max(0.0)));
        }
        if let Some(s) = self.cur.clone().filter(|s| now >= s.entered) {
            let since = (now - s.entered) as f32;
            let enter_t = (since / IN).clamp(0.0, 1.0);
            let e = Curve::Ease.at(enter_t);
            let card = Curve::Ease.at((since / (0.2 * crate::tokens::PACE)).clamp(0.0, 1.0));
            let words = Curve::Ease.at(((since - WORDS_FROM) / WORDS_IN).clamp(0.0, 1.0));
            if enter_t < 1.0 || words < 1.0 {
                ctx.request_repaint();
            }
            let bump_t = ((now - s.bumped) as f32 / (0.22 * crate::tokens::PACE)).clamp(0.0, 1.0);
            let bump = if bump_t < 1.0 {
                ctx.request_repaint();
                motion::keys(&[(0.0, 1.0), (0.45, 1.035), (1.0, 1.0)], Curve::Ease.at(bump_t))
            } else {
                1.0
            };
            let wide = 1.0 - (1.0 - NARROW) * (1.0 - e) * travel;
            let (r, act) = paint_slip(ctx, &s, center_x, screen, &labels, Shown { card, words, wide, scale: bump }, true);
            out.rects.insert(0, r);
            match act {
                Some(SlipAct::Close) => close = true,
                Some(SlipAct::Flip) => flip = true,
                Some(SlipAct::Key(i)) => {
                    if let Some((_, a)) = s.keys.get(i) {
                        match a {
                            Act::Copy(t) => ctx.copy_text(t.clone()),
                            Act::Tag(tag) => out.pressed = Some(*tag),
                        }
                    }
                    close = true;
                }
                None => {}
            }
        }
        if close {
            if let Some(s) = self.cur.take() {
                self.leaving.push((s, now));
                self.next_in(now);
            }
            ctx.request_repaint();
        }
        if flip {
            if let Some(s) = self.cur.as_mut() {
                s.open = !s.open;
                if !s.open {
                    s.born = now;
                }
            }
            ctx.request_repaint();
        }
        out
    }
}

enum SlipAct {
    Close,
    Flip,
    Key(usize),
}

#[derive(Clone, Copy)]
struct Shown {
    card: f32,
    words: f32,
    wide: f32,
    scale: f32,
}

fn paint_slip(ctx: &egui::Context, s: &Slip, center_x: f32, screen: Rect, labels: &[String; 3], shown: Shown, live: bool) -> (Rect, Option<SlipAct>) {
    let (alpha, scale) = (shown.card, shown.scale);
    let bad = s.tone == Tone::Bad;
    let two = !s.next.is_empty() || !s.raw.is_empty();
    let max_w = 560.0_f32.min(screen.width() - 32.0);
    let mut keys: Vec<(String, u8)> = s.keys.iter().enumerate().map(|(i, (l, _))| (l.clone(), i as u8)).collect();
    if !s.raw.is_empty() {
        keys.push((if s.open { labels[1].clone() } else { labels[0].clone() }, 200));
    }
    if bad || s.tone == Tone::Alert {
        keys.push((labels[2].clone(), 201));
    }
    let key_font = crate::button::key_font();
    let keys_w: f32 = ctx.fonts_mut(|f| keys.iter().map(|(l, _)| f.layout_no_wrap(l.clone(), key_font.clone(), egui::Color32::BLACK).size().x).sum::<f32>())
        + 14.0 * (keys.len().saturating_sub(1)) as f32
        + if keys.is_empty() { 0.0 } else { 10.0 };
    let (pad_l, pad_r, pad_y) = if two { (12.0, 18.0, 10.0) } else { (10.0, 18.0, 8.0) };
    let icon = 22.0;
    let text_max = (max_w - pad_l - pad_r - icon - 10.0 - keys_w).max(80.0);
    let wrap = |t: &str, font: egui::FontId, colour: egui::Color32, w: f32| {
        ctx.fonts_mut(|f| {
            let mut job = egui::text::LayoutJob::single_section(t.to_string(), egui::TextFormat { font_id: font, color: colour, line_height: Some(20.0), ..Default::default() });
            job.wrap = egui::text::TextWrapping { max_width: w, break_anywhere: false, ..Default::default() };
            f.layout_job(job)
        })
    };
    let first_font = if bad || s.tone == Tone::Alert { egui::FontId::new(14.0, crate::fonts::strong()) } else { Type::Note.font() };
    let first = wrap(&s.text, first_font, c(C::Ink), text_max);
    let second = (!s.next.is_empty()).then(|| wrap(&s.next, Type::Note.font(), c(C::Ink2), text_max));
    let raw = (s.open && !s.raw.is_empty()).then(|| wrap(&s.raw, Type::MonoSmall.font(), c(C::Ink3), text_max));
    let text_w = [Some(first.size().x), second.as_ref().map(|g| g.size().x), raw.as_ref().map(|g| g.size().x)].into_iter().flatten().fold(0.0, f32::max);
    let text_h = first.size().y + second.as_ref().map(|g| g.size().y).unwrap_or(0.0) + raw.as_ref().map(|g| 6.0 + g.size().y).unwrap_or(0.0);
    let w = pad_l + icon + 10.0 + text_w + keys_w + pad_r;
    let h = (text_h + pad_y * 2.0).max(40.0);
    let rect = Rect::from_min_size(pos2((center_x - w / 2.0).round(), screen.top() + 14.0), vec2(w, h));
    let layer = egui::LayerId::new(egui::Order::Tooltip, egui::Id::new(("zikaron-toast", s.id)));
    let c0 = rect.center();
    ctx.set_transform_layer(layer, egui::emath::TSTransform { scaling: scale, translation: c0.to_vec2() * (1.0 - scale) });
    let mut act = None;
    crate::layer::place(ctx, layer, None);
    crate::layer::area(layer.id, egui::Order::Tooltip).fixed_pos(rect.min).constrain(false).interactable(live).show(ctx, |ui| {
        ui.multiply_opacity(alpha);
        let (r, _) = ui.allocate_exact_size(rect.size(), egui::Sense::hover());
        let radius = if two { Radius::Sheet } else { Radius::Pill };
        let capsule = Rect::from_center_size(r.center(), vec2((r.width() * shown.wide).max(r.height()), r.height()));
        crate::material::paint(ui.painter(), capsule, radius, crate::material::Level::Toast, 0.0);
        ui.set_clip_rect(capsule.intersect(ui.clip_rect()));
        ui.multiply_opacity(if alpha > 0.0 { (shown.words / alpha).min(1.0) } else { 0.0 });
        let p = ui.painter();
        let line_y = if two { r.top() + pad_y + 10.0 } else { r.center().y };
        let ic = pos2(r.left() + pad_l + icon / 2.0, line_y);
        let (fill, glyph) = match s.tone {
            Tone::Note => (C::Ok, Glyph::Ok),
            Tone::Bad => (C::Bad, Glyph::No),
            Tone::Alert => (C::Warn, Glyph::Watch),
        };
        p.circle_filled(ic, icon / 2.0, c(fill));
        icons::glyph_at(p, glyph, ic, 12.0, palette::ON_SOLID);
        let tx = r.left() + pad_l + icon + 10.0;
        let mut y = if two { r.top() + pad_y } else { r.center().y - first.size().y / 2.0 };
        let fh = first.size().y;
        p.galley(pos2(tx, y), first, c(C::Ink));
        y += fh;
        if let Some(g) = second {
            let gh = g.size().y;
            p.galley(pos2(tx, y), g, c(C::Ink2));
            y += gh;
        }
        if let Some(g) = raw {
            p.galley(pos2(tx, y + 6.0), g, c(C::Ink3));
        }
        let mut kx = r.right() - pad_r - keys_w + if keys.is_empty() { 0.0 } else { 10.0 };
        for (label, which) in &keys {
            let kw = ui.fonts_mut(|f| f.layout_no_wrap(label.clone(), crate::button::key_font(), egui::Color32::BLACK).size().x);
            let kr = Rect::from_min_size(pos2(kx, line_y - 10.0), vec2(kw, 20.0));
            let resp = ui.interact(kr, egui::Id::new(("zikaron-toast-key", s.id, *which)), egui::Sense::click());
            let colour = if *which == 201 { c(C::Ink2) } else { c(C::AccentInk) };
            let k = Key::new(label, Role::Plain);
            if *which == 201 {
                ui.painter().text(kr.left_center(), egui::Align2::LEFT_CENTER, label, crate::button::key_font(), colour);
                if resp.hovered() {
                    ui.painter().hline(kr.x_range(), kr.center().y + 8.0, egui::Stroke::new(1.0_f32, colour));
                }
            } else {
                button::paint(ui, kr, &resp, &k, button::Phase::Idle);
            }
            if live && resp.clicked() {
                act = Some(match which {
                    200 => SlipAct::Flip,
                    201 => SlipAct::Close,
                    i => SlipAct::Key(*i as usize),
                });
            }
            let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            kx += kw + 14.0;
        }
    });
    let _ = tokens::FAST;
    (rect, act)
}
