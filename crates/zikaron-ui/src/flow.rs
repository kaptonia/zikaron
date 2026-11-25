use crate::motion::{self, Curve};
use crate::tokens;
use egui::epaint::ClippedShape;
use egui::layers::ShapeIdx;
use egui::{Context, Id, LayerId, Pos2, Rect, Vec2};

#[derive(Clone, Copy)]
struct Grown {
    w: f32,
    pass: u64,
}

pub fn mark(ctx: &Context, layer: LayerId) -> usize {
    ctx.graphics(|g| g.get(layer).map(|l| l.next_idx().0).unwrap_or(0))
}

pub(crate) fn clip_below(ctx: &Context, layer: LayerId, start: usize, bottom: f32) {
    ctx.graphics_mut(|g| {
        if let Some(l) = g.get_mut(layer) {
            let end = l.next_idx().0;
            for i in start..end {
                l.mutate_shape(ShapeIdx(i), |cs| cs.clip_rect.max.y = cs.clip_rect.max.y.min(bottom));
            }
        }
    });
}

fn moving_id() -> Id {
    Id::new("zikaron-height-moving")
}

pub fn mark_moving(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(moving_id(), true));
}

pub fn watch_moving<R>(ctx: &Context, f: impl FnOnce() -> R) -> (R, bool) {
    let outer = ctx.data(|d| d.get_temp::<bool>(moving_id())).unwrap_or(false);
    ctx.data_mut(|d| d.insert_temp(moving_id(), false));
    let r = f();
    let inner = ctx.data(|d| d.get_temp::<bool>(moving_id())).unwrap_or(false);
    ctx.data_mut(|d| d.insert_temp(moving_id(), outer || inner));
    (r, inner)
}

pub fn grow<R>(ui: &mut egui::Ui, id: Id, add: impl FnOnce(&mut egui::Ui) -> (R, Rect)) -> (R, Rect) {
    let ctx = ui.ctx().clone();
    let layer = ui.layer_id();
    let start = mark(&ctx, layer);
    let ((r, laid), inner) = watch_moving(&ctx, || add(ui));
    let shown = height_following(&ctx, id, laid.height(), laid.width(), inner);
    if shown + 0.5 < laid.height() {
        clip_below(&ctx, layer, start, laid.top() + shown);
    }
    let d = shown - laid.height();
    if d.abs() < 0.5 {
        return (r, laid);
    }
    ui.add_space(d);
    (r, Rect::from_min_size(laid.min, egui::vec2(laid.width(), shown)))
}

pub fn height(ctx: &Context, id: Id, natural: f32, w: f32) -> f32 {
    height_following(ctx, id, natural, w, false)
}

pub fn height_following(ctx: &Context, id: Id, natural: f32, w: f32, inner: bool) -> f32 {
    let pass = crate::motion::pass(ctx);
    let hid = id.with("zikaron-grow");
    let prev = ctx.data(|d| d.get_temp::<Grown>(hid));
    let steady = prev.map(|p| p.pass + 1 >= pass && (p.w - w).abs() < 0.5).unwrap_or(false);
    ctx.data_mut(|d| d.insert_temp(hid, Grown { w, pass }));
    if steady && !inner {
        let shown = motion::glide(ctx, hid.with("h"), natural, tokens::SLOW, Curve::Ease);
        if (shown - natural).abs() >= 0.5 {
            mark_moving(ctx);
        }
        shown
    } else {
        motion::snap(ctx, hid.with("h"), natural);
        natural
    }
}

#[derive(Clone)]
struct Drawn {
    key: u64,
    rect: Rect,
    shapes: Vec<ClippedShape>,
}

#[derive(Clone)]
struct Leaving {
    row: Drawn,
    at: f64,
}

#[derive(Clone, Default)]
struct Memory {
    pass: u64,
    width: f32,
    rows: Vec<Drawn>,
    leaving: Vec<Leaving>,
    born: Vec<(u64, f64)>,
    window: Option<Rect>,
}

pub struct Group {
    id: Id,
    layer: LayerId,
    origin: Pos2,
    clip: Rect,
    width: f32,
    rows: Vec<(u64, Rect, usize, usize)>,
    window: Option<Rect>,
}

impl Group {
    pub fn new(ui: &egui::Ui, id: Id, origin: Pos2, width: f32) -> Group {
        Group { id: id.with("zikaron-flow-group"), layer: ui.layer_id(), origin, clip: ui.clip_rect(), width, rows: Vec::new(), window: None }
    }

    pub fn in_sight(&mut self, at: Rect) {
        self.window = Some(at.translate(-self.origin.to_vec2()));
    }

    pub fn start(&self, ctx: &Context) -> usize {
        mark(ctx, self.layer)
    }

    pub fn row(&mut self, ctx: &Context, key: u64, rect: Rect, start: usize) {
        let end = mark(ctx, self.layer);
        self.rows.push((key, rect.translate(-self.origin.to_vec2()), start, end));
    }

    pub fn finish(self, ctx: &Context) -> f32 {
        let pass = crate::motion::pass(ctx);
        let now = crate::motion::now(ctx);
        let mut mem = ctx.data(|d| d.get_temp::<Memory>(self.id)).unwrap_or_default();
        let steady = mem.pass + 1 >= pass && mem.pass != 0 && (mem.width - self.width).abs() < 0.5 && !crate::scroll::by_hand(ctx) && !morphing(ctx);
        let dur = tokens::SLOW;
        let still = motion::reduced(ctx);
        let mut extra = 0.0;
        if !steady {
            mem.leaving.clear();
            mem.born.clear();
        } else {
            let (now_w, was_w) = (self.window, mem.window);
            let now_seen = |r: &Rect| now_w.is_none_or(|w| w.intersects(*r));
            let was_seen = |r: &Rect| was_w.is_none_or(|w| w.intersects(*r));
            for d in &mem.rows {
                if !self.rows.iter().any(|r| r.0 == d.key) && !mem.leaving.iter().any(|l| l.row.key == d.key) && now_seen(&d.rect) {
                    mem.leaving.push(Leaving { row: d.clone(), at: now });
                }
            }
            for r in &self.rows {
                if !mem.rows.iter().any(|d| d.key == r.0) && !mem.born.iter().any(|b| b.0 == r.0) && was_seen(&r.1) {
                    mem.born.push((r.0, now));
                }
            }
        }
        mem.leaving.retain(|l| !self.rows.iter().any(|r| r.0 == l.row.key) && ((now - l.at) as f32) < dur && now >= l.at);
        mem.born.retain(|b| self.rows.iter().any(|r| r.0 == b.0) && ((now - b.1) as f32) < dur && now >= b.1);
        let moving = !mem.leaving.is_empty() || !mem.born.is_empty();
        let mut shifts: Vec<(usize, usize, Vec2, f32, Rect)> = Vec::new();
        for (key, rect, s, e) in &self.rows {
            let kid = self.id.with(("row", *key));
            let (x, y) = if steady && !still {
                (motion::to(ctx, kid.with("x"), rect.left(), dur, Curve::Ease), motion::to(ctx, kid.with("y"), rect.top(), dur, Curve::Ease))
            } else {
                motion::snap(ctx, kid.with("x"), rect.left());
                motion::snap(ctx, kid.with("y"), rect.top());
                (rect.left(), rect.top())
            };
            let off = egui::vec2(x - rect.left(), y - rect.top());
            let open = mem.born.iter().find(|b| b.0 == *key).map(|b| Curve::Ease.at(((now - b.1) as f32 / dur).clamp(0.0, 1.0))).unwrap_or(1.0);
            if open < 1.0 && !still {
                extra -= rect.height() * (1.0 - open);
            }
            if off.length() > 0.01 || open < 1.0 {
                shifts.push((*s, *e, off, open, rect.translate(self.origin.to_vec2())));
            }
        }
        let origin = self.origin;
        let clip = self.clip;
        ctx.graphics_mut(|g| {
            let l = g.entry(self.layer);
            for (s, e, off, open, rect) in &shifts {
                let shown = Rect::from_min_size(rect.min + *off, egui::vec2(rect.width(), rect.height() * if still { 1.0 } else { *open }));
                for i in *s..*e {
                    l.mutate_shape(ShapeIdx(i), |cs| {
                        cs.shape.translate(*off);
                        if *open < 1.0 {
                            let a = *open;
                            egui::epaint::shape_transform::adjust_colors(&mut cs.shape, move |c| *c = crate::layer::leaving_colour(*c, a));
                            cs.clip_rect = cs.clip_rect.intersect(shown.expand2(egui::vec2(64.0, 0.0)));
                        }
                    });
                }
            }
            for lv in &mem.leaving {
                let p = Curve::Ease.at(((now - lv.at) as f32 / dur).clamp(0.0, 1.0));
                let a = 1.0 - p;
                let h = if still { 1.0 } else { a };
                if !still {
                    extra += lv.row.rect.height() * a;
                }
                let at = lv.row.rect.translate(origin.to_vec2());
                let keep = Rect::from_min_size(at.min, egui::vec2(at.width(), at.height() * h)).expand2(egui::vec2(64.0, 0.0));
                for cs in &lv.row.shapes {
                    let mut shape = cs.shape.clone();
                    shape.translate(origin.to_vec2());
                    egui::epaint::shape_transform::adjust_colors(&mut shape, move |c| *c = crate::layer::leaving_colour(*c, a));
                    l.add(clip.intersect(keep), shape);
                }
            }
        });
        let rows: Vec<Drawn> = ctx.graphics(|g| {
            let all: Vec<&ClippedShape> = g.get(self.layer).map(|l| l.all_entries().collect()).unwrap_or_default();
            self.rows
                .iter()
                .map(|(key, rect, s, e)| Drawn {
                    key: *key,
                    rect: *rect,
                    shapes: all
                        .get(*s..*e)
                        .map(|v| {
                            v.iter()
                                .map(|cs| {
                                    let mut c = (*cs).clone();
                                    c.shape.translate(-origin.to_vec2());
                                    c
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                })
                .collect()
        });
        mem.rows = rows;
        mem.window = self.window;
        mem.pass = pass;
        mem.width = self.width;
        if moving {
            ctx.request_repaint();
        }
        ctx.data_mut(|d| d.insert_temp(self.id, mem));
        extra
    }
}

fn retext_id() -> Id {
    Id::new("zikaron-flow-retext")
}

const WORDS_OUT: f32 = 0.2 * crate::tokens::PACE;
const WORDS_IN: f32 = 0.24 * crate::tokens::PACE;

pub fn retext(ctx: &Context) {
    let now = crate::motion::now(ctx);
    ctx.plugin_or_default::<Words>();
    ctx.data_mut(|d| d.insert_temp(retext_id(), now));
    ctx.with_plugin::<Words, _>(|w| w.start = Some((now, w.last.clone())));
    ctx.request_repaint();
}

pub fn words_epoch(ctx: &Context) -> u64 {
    ctx.data(|d| d.get_temp::<f64>(retext_id())).map(f64::to_bits).unwrap_or(0)
}

pub fn morphing(ctx: &Context) -> bool {
    let now = crate::motion::now(ctx);
    ctx.data(|d| d.get_temp::<f64>(retext_id())).map(|at| now >= at && ((now - at) as f32) < tokens::SLOW + 0.05).unwrap_or(false)
}

#[derive(Clone)]
struct Said {
    text: String,
    clip: Rect,
    shape: egui::epaint::TextShape,
}

#[derive(Default)]
pub struct Words {
    last: Vec<Said>,
    start: Option<(f64, Vec<Said>)>,
    now: f64,
}

fn texts(shapes: &[ClippedShape]) -> Vec<Said> {
    fn walk(s: &egui::Shape, clip: Rect, out: &mut Vec<Said>) {
        match s {
            egui::Shape::Text(t) => out.push(Said { text: t.galley.job.text.clone(), clip, shape: t.clone() }),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for cs in shapes {
        walk(&cs.shape, cs.clip_rect, &mut out);
    }
    out
}

fn fade_new(s: &mut egui::Shape, old: &mut Vec<String>, a: f32) {
    match s {
        egui::Shape::Text(t) => {
            if let Some(i) = old.iter().position(|o| *o == t.galley.job.text) {
                old.swap_remove(i);
            } else {
                t.opacity_factor *= a;
            }
        }
        egui::Shape::Vec(v) => v.iter_mut().for_each(|x| fade_new(x, old, a)),
        _ => {}
    }
}

impl egui::Plugin for Words {
    fn debug_name(&self) -> &'static str {
        "zikaron-words"
    }

    fn on_end_pass(&mut self, ui: &mut egui::Ui) {
        self.now = crate::motion::now(ui.ctx());
        if let Some((at, _)) = &self.start {
            if self.now >= *at && ((self.now - at) as f32) < WORDS_IN.max(WORDS_OUT) {
                ui.ctx().request_repaint();
            }
        }
    }

    fn output_hook(&mut self, output: &mut egui::FullOutput) {
        let now_texts = texts(&output.shapes);
        if let Some((at, old)) = &self.start {
            let t = (self.now - at) as f32;
            if !(0.0..WORDS_IN.max(WORDS_OUT)).contains(&t) {
                self.start = None;
            } else {
                let mut pool: Vec<String> = old.iter().map(|s| s.text.clone()).collect();
                let a_in = Curve::Ease.at((t / WORDS_IN).clamp(0.0, 1.0));
                for cs in output.shapes.iter_mut() {
                    fade_new(&mut cs.shape, &mut pool, a_in);
                }
                let mut gone: Vec<String> = now_texts.iter().map(|s| s.text.clone()).collect();
                let a_out = 1.0 - Curve::Ease.at((t / WORDS_OUT).clamp(0.0, 1.0));
                if a_out > 0.0 {
                    for s in old {
                        if let Some(i) = gone.iter().position(|g| *g == s.text) {
                            gone.swap_remove(i);
                            continue;
                        }
                        let mut shape = s.shape.clone();
                        shape.opacity_factor *= a_out;
                        let at = output.shapes.iter().rposition(|cs| cs.clip_rect == s.clip).map(|i| i + 1).unwrap_or(output.shapes.len());
                        output.shapes.insert(at, ClippedShape { clip_rect: s.clip, shape: egui::Shape::Text(shape) });
                    }
                }
            }
        }
        self.last = now_texts;
    }
}

pub fn pointer_on(ui: &egui::Ui, rect: Rect) -> bool {
    if crate::scroll::moving(ui.ctx()) {
        return false;
    }
    let Some(at) = ui.ctx().pointer_hover_pos() else { return false };
    let p = ui.ctx().layer_transform_to_global(ui.layer_id()).map(|tr| tr.inverse() * at).unwrap_or(at);
    let owns = |r: Rect| p.x >= r.min.x && p.x < r.max.x && p.y >= r.min.y && p.y < r.max.y;
    if !(owns(rect) && ui.clip_rect().contains(p) && crate::layer::on_top(ui, Rect::from_center_size(at, Vec2::ZERO))) {
        return false;
    }
    let held = egui::Id::new("zikaron-lit-this-pass");
    let pass = crate::motion::pass(ui.ctx());
    let key = (ui.layer_id(), [rect.min.x, rect.min.y, rect.max.x, rect.max.y].map(f32::to_bits));
    match ui.ctx().data(|d| d.get_temp::<(u64, (egui::LayerId, [u32; 4]))>(held)) {
        Some((at, k)) if at == pass => k == key,
        _ => {
            ui.ctx().data_mut(|d| d.insert_temp(held, (pass, key)));
            true
        }
    }
}

fn collect_id() -> Id {
    Id::new("zikaron-flow-collect")
}

#[derive(Clone, Default)]
struct Collected {
    on: bool,
    rows: Vec<(u64, Rect, usize, usize)>,
    layer: Option<LayerId>,
}

pub fn gather(ui: &egui::Ui) {
    ui.ctx().data_mut(|d| d.insert_temp(collect_id(), Collected { on: true, rows: Vec::new(), layer: Some(ui.layer_id()) }));
}

pub fn note(ui: &egui::Ui, key: u64, rect: Rect, start: usize) {
    let layer = ui.layer_id();
    let end = mark(ui.ctx(), layer);
    ui.ctx().data_mut(|d| {
        let c = d.get_temp_mut_or_default::<Collected>(collect_id());
        if c.on && c.layer == Some(layer) {
            c.rows.push((key, rect, start, end));
        }
    });
}

pub fn gathered(ui: &egui::Ui, id: Id, origin: Pos2, width: f32) {
    let c = ui.ctx().data_mut(|d| d.remove_temp::<Collected>(collect_id())).unwrap_or_default();
    let mut g = Group::new(ui, id, origin, width);
    for (key, rect, s, e) in c.rows {
        g.rows.push((key, rect.translate(-origin.to_vec2()), s, e));
    }
    let _ = g.finish(ui.ctx());
}
