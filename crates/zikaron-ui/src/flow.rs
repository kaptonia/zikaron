//! Flow: what keeps the window continuous when what it shows changes in place. One path for every page, sheet,
//! rail and toolbar, so a page or a sheet added later gets it by using the library's blocks.
//!
//! - **Heights** ([`grow`]): a block (a card, a list card, a fold's body, a sheet's body) takes the height of its
//!   content over [`tokens::SLOW`] when that content grows or shrinks, so what lies below moves on in one run
//!   instead of jumping; what lies past the height shown is clipped. First sight, a change of width (the window
//!   resized) and reduced motion take the new height at once.
//! - **Rows** ([`Group`]): a list's rows are matched by identity from one frame to the next. A row that stays and
//!   moves slides from where it was; a new row opens at its place and fades in; a row that leaves closes where it
//!   was and fades out (its last drawing, played back). One time and one curve for all of them and for the
//!   height of the block they are in, so the rows below move on in one direction and stop where they belong.
//! - **Words** ([`retext`]): when the window's words change language, the words that changed fade out where they
//!   were (0.2 s) while the new ones come in by their colour (0.24 s); words that did not change are left alone.
//!   Keys and labels meanwhile grow or shrink to their new words ([`morphing`], [`crate::motion::size`]).

use crate::motion::{self, Curve};
use crate::tokens;
use egui::epaint::ClippedShape;
use egui::layers::ShapeIdx;
use egui::{Context, Id, LayerId, Pos2, Rect, Vec2};

// ───────────────────────── Heights ─────────────────────────

#[derive(Clone, Copy)]
struct Grown {
    w: f32,
    pass: u64,
}

/// The index the next shape painted on `layer` will take.
pub fn mark(ctx: &Context, layer: LayerId) -> usize {
    ctx.graphics(|g| g.get(layer).map(|l| l.next_idx().0).unwrap_or(0))
}

/// Clip the shapes painted on `layer` from `start` on to above `bottom`.
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

/// A height is on its way this pass (a block easing to its content, a fold opening or closing): whatever holds it
/// learns so through [`watch_moving`] and follows it, instead of easing again after it.
pub fn mark_moving(ctx: &Context) {
    ctx.data_mut(|d| d.insert_temp(moving_id(), true));
}

/// Run `f` and say whether a height inside it was on its way this pass ([`mark_moving`]); the scope around this one
/// learns it as well.
pub fn watch_moving<R>(ctx: &Context, f: impl FnOnce() -> R) -> (R, bool) {
    let outer = ctx.data(|d| d.get_temp::<bool>(moving_id())).unwrap_or(false);
    ctx.data_mut(|d| d.insert_temp(moving_id(), false));
    let r = f();
    let inner = ctx.data(|d| d.get_temp::<bool>(moving_id())).unwrap_or(false);
    ctx.data_mut(|d| d.insert_temp(moving_id(), outer || inner));
    (r, inner)
}

/// A block whose height follows its content over [`tokens::SLOW`] (see the file header). `add` draws the block
/// in `ui` itself (so whatever it holds keeps its identity), and answers its laid-out
/// rectangle; the block then takes the height shown: what lies past it is clipped, and what comes after it in
/// `ui` is placed from the height shown (the cursor moved back up, or on down). A block whose content is itself
/// changing height this pass (an inner block easing, a fold) takes its content's height as it is, so the two move
/// as one instead of the outer one easing again behind the inner. Returns the content's answer and the block's
/// rectangle as shown.
pub fn grow<R>(ui: &mut egui::Ui, id: Id, add: impl FnOnce(&mut egui::Ui) -> (R, Rect)) -> (R, Rect) {
    let ctx = ui.ctx().clone();
    let layer = ui.layer_id();
    let start = mark(&ctx, layer);
    let ((r, laid), inner) = watch_moving(&ctx, || add(ui));
    let shown = height_following(&ctx, id, laid.height(), laid.width(), inner);
    if shown + 0.5 < laid.height() {
        clip_below(&ctx, layer, start, laid.top() + shown);
    }
    // Within half a point of its content it stands as laid out (a run's last fraction moves nothing).
    let d = shown - laid.height();
    if d.abs() < 0.5 {
        return (r, laid);
    }
    ui.add_space(d);
    (r, Rect::from_min_size(laid.min, egui::vec2(laid.width(), shown)))
}

/// The height shown for a block under `id` whose content is `natural` high at width `w` (see [`grow`]).
pub fn height(ctx: &Context, id: Id, natural: f32, w: f32) -> f32 {
    height_following(ctx, id, natural, w, false)
}

/// [`height`], taking `natural` as it is when the content is itself on its way (`inner`): the content already
/// moves, and a second ease after it would trail by its own length.
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

// ───────────────────────── Rows ─────────────────────────

/// One row as drawn last pass: its identity, its rectangle from the group's origin, and its shapes.
#[derive(Clone)]
struct Drawn {
    key: u64,
    rect: Rect,
    shapes: Vec<ClippedShape>,
}

/// A row that has left, closing where it was.
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
    /// When each row that came in this run of passes arrived.
    born: Vec<(u64, f64)>,
}

/// A list whose rows keep their identity from one frame to the next (see the file header). Make one before
/// drawing the rows, give each row with [`Group::row`] after drawing it, and [`Group::finish`] after the last.
pub struct Group {
    id: Id,
    layer: LayerId,
    origin: Pos2,
    clip: Rect,
    width: f32,
    rows: Vec<(u64, Rect, usize, usize)>,
}

impl Group {
    /// A group under `id`, its rows placed from `origin` (the list's top left: rows are compared from it, so a
    /// list that scrolls or moves as a whole does not slide its rows), `width` wide.
    pub fn new(ui: &egui::Ui, id: Id, origin: Pos2, width: f32) -> Group {
        Group { id: id.with("zikaron-flow-group"), layer: ui.layer_id(), origin, clip: ui.clip_rect(), width, rows: Vec::new() }
    }

    /// Where the shapes of the next row begin (call before drawing it).
    pub fn start(&self, ctx: &Context) -> usize {
        mark(ctx, self.layer)
    }

    /// A row drawn since `start`, with its identity and its rectangle.
    pub fn row(&mut self, ctx: &Context, key: u64, rect: Rect, start: usize) {
        let end = mark(ctx, self.layer);
        self.rows.push((key, rect.translate(-self.origin.to_vec2()), start, end));
    }

    /// Compare with the last pass and move, open, fade and close the rows. Returns how much taller (positive) or
    /// shorter the list shows than it is laid out: what the rows still opening lack and the rows still closing
    /// add (a list that draws its own block adds it to the block's height; one inside [`grow`] needs nothing).
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
            // Rows that left: kept as they were last drawn, closing where they were.
            for d in &mem.rows {
                if !self.rows.iter().any(|r| r.0 == d.key) && !mem.leaving.iter().any(|l| l.row.key == d.key) {
                    mem.leaving.push(Leaving { row: d.clone(), at: now });
                }
            }
            // Rows that came: opening where they are.
            for r in &self.rows {
                if !mem.rows.iter().any(|d| d.key == r.0) && !mem.born.iter().any(|b| b.0 == r.0) {
                    mem.born.push((r.0, now));
                }
            }
        }
        // A row back while it was leaving stops leaving.
        mem.leaving.retain(|l| !self.rows.iter().any(|r| r.0 == l.row.key) && ((now - l.at) as f32) < dur && now >= l.at);
        mem.born.retain(|b| self.rows.iter().any(|r| r.0 == b.0) && ((now - b.1) as f32) < dur && now >= b.1);
        let moving = !mem.leaving.is_empty() || !mem.born.is_empty();
        // Each row's place eases to where it is laid out now (rows that stay slide from where they were).
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
            // Under reduced motion a row that comes is at its full height at once; only its fade stays.
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
            // The rows leaving, drawn as they were, closing from their foot and fading.
            for lv in &mem.leaving {
                let p = Curve::Ease.at(((now - lv.at) as f32 / dur).clamp(0.0, 1.0));
                let a = 1.0 - p;
                // Under reduced motion a row that goes keeps its height while it fades where it was.
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
        // What each row drew this pass, from the group's origin (a row that leaves next pass is played from it).
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
        mem.pass = pass;
        mem.width = self.width;
        if moving {
            ctx.request_repaint();
        }
        ctx.data_mut(|d| d.insert_temp(self.id, mem));
        extra
    }
}

// ───────────────────────── Words ─────────────────────────

/// The time the words changed language, if they did lately.
fn retext_id() -> Id {
    Id::new("zikaron-flow-retext")
}

/// How long the old words take to go, the new ones to come, and keys and labels to reach their new size.
const WORDS_OUT: f32 = 0.2 * crate::tokens::PACE;
const WORDS_IN: f32 = 0.24 * crate::tokens::PACE;

/// The words just changed language: start the crossfade (see the file header). Call it the frame the language is
/// switched, before or while drawing that frame.
pub fn retext(ctx: &Context) {
    let now = crate::motion::now(ctx);
    ctx.plugin_or_default::<Words>();
    ctx.data_mut(|d| d.insert_temp(retext_id(), now));
    ctx.with_plugin::<Words, _>(|w| w.start = Some((now, w.last.clone())));
    ctx.request_repaint();
}

/// Whether keys and labels are on their way to the size of their new words (within [`tokens::SLOW`] of a
/// [`retext`]).
pub fn morphing(ctx: &Context) -> bool {
    let now = crate::motion::now(ctx);
    ctx.data(|d| d.get_temp::<f64>(retext_id())).map(|at| now >= at && ((now - at) as f32) < tokens::SLOW + 0.05).unwrap_or(false)
}

/// One run of words as drawn: where, in what clip, and the galley.
#[derive(Clone)]
struct Said {
    text: String,
    clip: Rect,
    shape: egui::epaint::TextShape,
}

/// The window's words, kept every frame so a change of language can play the old ones out. Installed with the
/// skin ([`crate::skin::dress`]).
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

/// Fade in by `a` every run of words in `s` that is not among `old` (taking each old one once).
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
                // Words in both are left alone; new words come in by their colour; old words go where they were.
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
                        // Laid on the frame just after the last shape in the same clip (its block), else on top.
                        let at = output.shapes.iter().rposition(|cs| cs.clip_rect == s.clip).map(|i| i + 1).unwrap_or(output.shapes.len());
                        output.shapes.insert(at, ClippedShape { clip_rect: s.clip, shape: egui::Shape::Text(shape) });
                    }
                }
            }
        }
        self.last = now_texts;
    }
}

// ───────────────────────── Hover ─────────────────────────

/// Whether the row in `rect` is the one lit under the pointer now, as it is laid out this frame, on the layer on top
/// there, with nothing scrolling: at most one place a pass is lit, and none while something scrolls (it lights
/// again once it stands still). A row owns its top edge and not its bottom one, so the pointer on the line between
/// two rows lights the lower only; the first place that answers yes in a pass holds the light for that pass.
pub fn pointer_on(ui: &egui::Ui, rect: Rect) -> bool {
    if crate::scroll::moving(ui.ctx()) {
        return false;
    }
    let Some(at) = ui.ctx().pointer_hover_pos() else { return false };
    // Where the pointer is among this layer's rows as they are drawn this frame: a layer scaled as it comes in (a
    // sheet, a menu) is read through its transform, not where its rows would be unscaled.
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

// ───────────────────────── Keys of a bar ─────────────────────────

fn collect_id() -> Id {
    Id::new("zikaron-flow-collect")
}

#[derive(Clone, Default)]
struct Collected {
    on: bool,
    rows: Vec<(u64, Rect, usize, usize)>,
    layer: Option<LayerId>,
}

/// Start gathering the keys and segmented controls drawn next on `ui`'s layer (a bar whose keys change with
/// the page: [`crate::toolbar`]); [`gathered`] takes them.
pub fn gather(ui: &egui::Ui) {
    ui.ctx().data_mut(|d| d.insert_temp(collect_id(), Collected { on: true, rows: Vec::new(), layer: Some(ui.layer_id()) }));
}

/// A key or a control drawn since `start` with its identity (its words) and rectangle, when a bar gathers.
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

/// Stop gathering, and keep the gathered keys continuous as a [`Group`] under `id` placed from `origin`:
/// keys that stay keep their place or slide to it, new ones fade in, gone ones fade out where they were.
pub fn gathered(ui: &egui::Ui, id: Id, origin: Pos2, width: f32) {
    let c = ui.ctx().data_mut(|d| d.remove_temp::<Collected>(collect_id())).unwrap_or_default();
    let mut g = Group::new(ui, id, origin, width);
    for (key, rect, s, e) in c.rows {
        g.rows.push((key, rect.translate(-origin.to_vec2()), s, e));
    }
    let _ = g.finish(ui.ctx());
}
