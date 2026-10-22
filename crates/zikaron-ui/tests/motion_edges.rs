//! The edges of the library's motion, read frame by frame in headless egui frames: a run turned midway goes on
//! from where it is and how fast it goes; a card's rows that come and go open and close in place while the rows
//! under them slide one way, and a card that changes width puts its rows at once; a pill that changes state
//! crosses in place; under reduced motion moves and sizes (rows, folds, sheets) arrive at once while fades stay.

use zikaron_ui::egui::{self, pos2, Event, PointerButton, Pos2, Rect};
use zikaron_ui::palette::Tone;
use zikaron_ui::{card, mark, motion};

const SCREEN: egui::Vec2 = egui::Vec2 { x: 1180.0, y: 800.0 };
const DT: f64 = 1.0 / 60.0;

struct Frames {
    ctx: egui::Context,
    t: f64,
}

impl Frames {
    fn new(reduced: bool) -> Frames {
        let ctx = egui::Context::default();
        let _ = zikaron_ui::skin::dress(&ctx);
        let f = Frames { ctx, t: 1.0 };
        motion::set_reduced(&f.ctx, reduced);
        f
    }

    fn run(&mut self, dt: f64, events: Vec<Event>, mut add: impl FnMut(&mut egui::Ui)) -> egui::FullOutput {
        self.t += dt;
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(self.t), events, ..Default::default() };
        self.ctx.run_ui(input, |ui| add(ui))
    }
}

fn press_at(p: Pos2) -> Vec<Event> {
    vec![
        Event::PointerMoved(p),
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Default::default() },
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Default::default() },
    ]
}

/// One text drawn: its words, where, its opacity, and the alpha of its first glyph's colour.
struct Said {
    words: String,
    rect: Rect,
    opacity: f32,
    alpha: u8,
}

fn texts(out: &egui::FullOutput) -> Vec<Said> {
    fn walk(s: &egui::Shape, out: &mut Vec<Said>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            egui::Shape::Text(t) => {
                let alpha = t.galley.rows.iter().flat_map(|r| r.row.visuals.mesh.vertices.iter()).map(|v| v.color.a()).max().unwrap_or(0);
                out.push(Said { words: t.galley.text().to_string(), rect: t.galley.rect.translate(t.pos.to_vec2()), opacity: t.opacity_factor, alpha })
            }
            _ => {}
        }
    }
    let mut v = Vec::new();
    for cs in &out.shapes {
        walk(&cs.shape, &mut v);
    }
    v
}

fn find<'a>(out: &'a [Said], w: &str) -> Option<&'a Said> {
    out.iter().find(|s| s.words == w)
}

// ───────────────────────── A run turned midway ─────────────────────────

/// A value on its way to one end is sent to the other: the first frame after goes on from where it was at no
/// more than the speed it had (no jump), and it reaches the new end, exactly, within its run.
#[test]
fn a_run_turned_midway_goes_on_from_its_place_and_speed() {
    let mut f = Frames::new(false);
    let id = egui::Id::new("edge-run");
    let mut v = 0.0;
    f.run(DT, vec![], |ui| v = motion::to(ui.ctx(), id, 0.0, 0.3, motion::Curve::Ease));
    let mut seen = Vec::new();
    for _ in 0..6 {
        f.run(DT, vec![], |ui| v = motion::to(ui.ctx(), id, 100.0, 0.3, motion::Curve::Ease));
        seen.push(v);
    }
    let (before, last) = (seen[seen.len() - 2], seen[seen.len() - 1]);
    assert!(last > 5.0 && last < 95.0, "midway: {seen:?}");
    let speed = last - before;
    f.run(DT, vec![], |ui| v = motion::to(ui.ctx(), id, -100.0, 0.3, motion::Curve::Ease));
    assert!((v - last).abs() <= speed.abs() * 1.5 + 0.01, "no jump on turning: {last} → {v} (step was {speed})");
    let mut after = vec![v];
    for _ in 0..24 {
        f.run(DT, vec![], |ui| v = motion::to(ui.ctx(), id, -100.0, 0.3, motion::Curve::Ease));
        after.push(v);
    }
    assert_eq!(*after.last().unwrap(), -100.0, "at the new end, exactly: {after:?}");
    for w in after.windows(2) {
        assert!((w[1] - w[0]).abs() < 40.0, "no jump on the way: {after:?}");
    }
}

// ───────────────────────── A card's rows ─────────────────────────

/// A card of rows kept continuous: the first row, then the middle one when `middle`, then the last (named by the
/// first three Chinese ordinal signs).
fn rows(ui: &mut egui::Ui, middle: bool, width: f32) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(40.0, 40.0), egui::vec2(width, 600.0))), |ui| {
        card::form(ui, |ui, f| {
            f.keep(ui);
            f.key("a");
            let _ = f.info(ui, "甲", card::Show::Text("一"));
            if middle {
                f.key("b");
                let _ = f.info(ui, "乙", card::Show::Text("二"));
            }
            f.key("c");
            let _ = f.info(ui, "丙", card::Show::Text("三"));
        });
    });
}

/// A row comes between two: the row under it starts where it was, slides down in one direction and is in place
/// within 0.4 s; the row that came fades in where it belongs.
#[test]
fn a_row_that_comes_opens_in_place_and_the_rows_below_slide_one_way() {
    let mut f = Frames::new(false);
    f.run(DT, vec![], |ui| rows(ui, false, 600.0));
    let before = f.run(DT, vec![], |ui| rows(ui, false, 600.0));
    let from = find(&texts(&before), "丙").expect("丙").rect.center().y;
    let mut ys = Vec::new();
    let mut first_alpha = None;
    for _ in 0..30 {
        let out = f.run(DT, vec![], |ui| rows(ui, true, 600.0));
        let said = texts(&out);
        ys.push(find(&said, "丙").expect("丙").rect.center().y);
        if first_alpha.is_none() {
            first_alpha = find(&said, "乙").map(|s| s.alpha);
        }
    }
    let to = *ys.last().unwrap();
    assert!(to > from + 20.0, "the row below moved down: {from} → {to}");
    assert!((ys[0] - from).abs() < 2.0, "the first frame starts where it was: {} vs {from}", ys[0]);
    for w in ys.windows(2) {
        assert!(w[1] >= w[0] - 0.01, "one direction: {ys:?}");
    }
    let there = ys.iter().position(|y| (y - to).abs() < 0.5).unwrap_or(ys.len());
    assert!(there as f64 * DT <= 0.4, "in place within 0.4 s ({there} frames)");
    assert!(first_alpha.is_some_and(|a| a < 250), "the row that came fades in: {first_alpha:?}");
}

/// A row leaves: the row under it moves up in one direction from where it was; the row that left is drawn
/// where it was, fading, and then is gone.
#[test]
fn a_row_that_goes_closes_where_it_was_and_the_rows_below_slide_up() {
    let mut f = Frames::new(false);
    f.run(DT, vec![], |ui| rows(ui, true, 600.0));
    let before = f.run(DT, vec![], |ui| rows(ui, true, 600.0));
    let from = find(&texts(&before), "丙").expect("丙").rect.center().y;
    let mut ys = Vec::new();
    let mut ghost = Vec::new();
    for _ in 0..30 {
        let out = f.run(DT, vec![], |ui| rows(ui, false, 600.0));
        let said = texts(&out);
        ys.push(find(&said, "丙").expect("丙").rect.center().y);
        ghost.push(find(&said, "乙").is_some());
    }
    assert!((ys[0] - from).abs() < 2.0, "the first frame starts where it was");
    for w in ys.windows(2) {
        assert!(w[1] <= w[0] + 0.01, "one direction: {ys:?}");
    }
    assert!(*ys.last().unwrap() < from - 20.0, "it moved up");
    assert!(ghost[0] && !*ghost.last().unwrap(), "the row that left fades where it was, then is gone");
}

/// The card is laid out narrower (the window was resized): its values stand at the new right edge at once, no
/// row slides.
#[test]
fn a_card_that_changes_width_puts_its_rows_at_once() {
    let mut f = Frames::new(false);
    f.run(DT, vec![], |ui| rows(ui, true, 600.0));
    let wide = f.run(DT, vec![], |ui| rows(ui, true, 600.0));
    let right_wide = find(&texts(&wide), "三").expect("三").rect.right();
    let narrow = f.run(DT, vec![], |ui| rows(ui, true, 500.0));
    let said = texts(&narrow);
    let right = find(&said, "三").expect("三").rect.right();
    assert!((right_wide - right - 100.0).abs() < 1.0, "the value is at the new right edge at once: {right_wide} → {right}");
    let later = f.run(0.5, vec![], |ui| rows(ui, true, 500.0));
    let y_now = |out: &[Said], w: &str| find(out, w).map(|s| s.rect.center().y);
    assert_eq!(y_now(&said, "丙"), y_now(&texts(&later), "丙"), "no row slides");
}

// ───────────────────────── A pill ─────────────────────────

fn pill(ui: &mut egui::Ui, s: &str, tone: Tone) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(40.0, 40.0), egui::vec2(300.0, 40.0))), |ui| {
        ui.push_id("edge-pill", |ui| mark::pill(ui, s, tone));
    });
}

/// A pill changes state: from the frame it changes, the old words go and the new come in the same place; within 0.4 s
/// only the new words stand.
#[test]
fn a_pill_that_changes_state_crosses_in_place() {
    for reduced in [false, true] {
        let mut f = Frames::new(reduced);
        f.run(DT, vec![], |ui| pill(ui, "待上链", Tone::Warn));
        let before = f.run(DT, vec![], |ui| pill(ui, "待上链", Tone::Warn));
        let left = find(&texts(&before), "待上链").expect("old words").rect.left();
        f.run(DT, vec![], |ui| pill(ui, "已上链", Tone::Ok));
        let next = texts(&f.run(DT * 4.0, vec![], |ui| pill(ui, "已上链", Tone::Ok)));
        let (old, new) = (find(&next, "待上链"), find(&next, "已上链"));
        assert!(old.is_some_and(|o| o.alpha < 250) && new.is_some_and(|n| n.alpha < 250), "both cross, reduced: {reduced}");
        assert!(new.is_some_and(|n| (n.rect.left() - left).abs() < 0.5), "in the same place");
        let end = texts(&f.run(0.4, vec![], |ui| pill(ui, "已上链", Tone::Ok)));
        assert!(find(&end, "待上链").is_none() && find(&end, "已上链").is_some(), "only the new words, reduced: {reduced}");
    }
}

// ───────────────────────── Reduced motion ─────────────────────────

/// Under reduced motion a row that comes puts the rows under it in place at once and is at its full height,
/// while it still fades in.
#[test]
fn under_reduced_motion_rows_arrive_at_once_and_still_fade() {
    let mut f = Frames::new(true);
    f.run(DT, vec![], |ui| rows(ui, false, 600.0));
    f.run(DT, vec![], |ui| rows(ui, false, 600.0));
    let first = texts(&f.run(DT, vec![], |ui| rows(ui, true, 600.0)));
    let settled = texts(&f.run(0.5, vec![], |ui| rows(ui, true, 600.0)));
    let y = |out: &[Said], w: &str| find(out, w).expect(w).rect.center().y;
    assert!((y(&first, "丙") - y(&settled, "丙")).abs() < 0.5, "the row below is in place at once");
    assert!((y(&first, "乙") - y(&settled, "乙")).abs() < 0.5, "the row that came is in place at once");
    assert!(find(&first, "乙").expect("乙").alpha < find(&settled, "乙").expect("乙").alpha, "it still fades in");
}

/// Under reduced motion a fold that opens gives the card its height at once (what follows it is in place the
/// frame after the press) while its rows still fade in.
#[test]
fn under_reduced_motion_a_fold_opens_at_once_and_its_rows_fade() {
    let fold = |ui: &mut egui::Ui| {
        ui.scope_builder(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(40.0, 40.0), egui::vec2(600.0, 700.0))), |ui| {
            card::form(ui, |ui, f| {
                let _ = f.fold(ui, "edge-fold", "折叠", "两行", |ui, f| {
                    let _ = f.info(ui, "里一", card::Show::Text("一"));
                    let _ = f.info(ui, "里二", card::Show::Text("二"));
                });
            });
            ui.label("下方");
        });
    };
    let mut f = Frames::new(true);
    f.run(DT, vec![], fold);
    let shut = texts(&f.run(DT, vec![], fold));
    let row = find(&shut, "折叠").expect("the fold row").rect.center();
    f.run(DT, press_at(row), fold);
    let first = texts(&f.run(DT, vec![], fold));
    let settled = texts(&f.run(0.6, vec![], fold));
    let y = |out: &[Said]| find(out, "下方").expect("下方").rect.center().y;
    assert!(y(&settled) > y(&shut) + 40.0, "the fold opened");
    assert!((y(&first) - y(&settled)).abs() < 0.5, "what follows is in place at once: {} vs {}", y(&first), y(&settled));
    let inner = find(&first, "里一").expect("its rows are drawn").alpha;
    assert!(inner < find(&settled, "里一").expect("its rows").alpha, "its rows still fade in: {inner}");
}

/// Under reduced motion a sheet that opens is at its size and place the first frame it shows (no growing from
/// the press), while it still fades in.
#[test]
fn under_reduced_motion_a_sheet_opens_at_its_size_and_fades() {
    let draw = |ui: &mut egui::Ui| {
        let ctx = ui.ctx().clone();
        let mut state = ();
        let _ = zikaron_ui::sheet::show(&ctx, zikaron_ui::sheet::Spec::new("edge-sheet", 480.0), &mut state, |ui, _| zikaron_ui::sheet::title(ui, "弹窗标题", ""), |_, _| ());
    };
    let mut f = Frames::new(true);
    f.run(DT, vec![Event::PointerMoved(pos2(100.0, 100.0))], draw);
    // The words come in after the sheet's ground: the first frame they show.
    let mut first = Vec::new();
    for _ in 0..20 {
        first = texts(&f.run(DT, vec![], draw));
        if find(&first, "弹窗标题").is_some() {
            break;
        }
    }
    let mut settled = Vec::new();
    for _ in 0..40 {
        settled = texts(&f.run(DT, vec![], draw));
    }
    let a = find(&first, "弹窗标题").expect("the title, first frame");
    let b = find(&settled, "弹窗标题").expect("the title, settled");
    assert!((a.rect.width() - b.rect.width()).abs() < 0.5 && (a.rect.center() - b.rect.center()).length() < 0.5, "at its size and place at once: {:?} vs {:?}", a.rect, b.rect);
    assert!(a.alpha < b.alpha || a.opacity < b.opacity, "it still fades in");
}

/// A run turned twice in one frame (a pass done again at the same time, as a discarded pass is) goes on from where
/// it stands towards the last target, never far past either end: it does not take the start of a run not yet begun
/// for where that run was going.
#[test]
fn a_run_turned_twice_in_one_frame_stays_between_its_ends() {
    let mut f = Frames::new(false);
    let id = egui::Id::new("twice");
    let mut v = 0.0;
    f.run(DT, vec![], |ui| v = motion::to(ui.ctx(), id, 100.0, 0.3, motion::Curve::Spring));
    f.run(DT, vec![], |ui| v = motion::to(ui.ctx(), id, 200.0, 0.3, motion::Curve::Spring));
    // The same moment again, the target moved once more.
    f.run(0.0, vec![], |ui| v = motion::to(ui.ctx(), id, 210.0, 0.3, motion::Curve::Spring));
    let mut seen = vec![v];
    for _ in 0..30 {
        f.run(DT, vec![], |ui| v = motion::to(ui.ctx(), id, 210.0, 0.3, motion::Curve::Spring));
        seen.push(v);
    }
    let (lo, hi) = seen.iter().fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
    assert!(lo >= 90.0 && hi <= 240.0, "stays between its ends, a spring's overshoot aside: {lo}..{hi}");
    assert!((v - 210.0).abs() < 0.5, "and arrives: {v}");
}
