//! A container's size moves with its content, never half a beat after it: new content (a page, a tab, a step) is
//! first seen at its own size; a block around a block that is easing moves as one with it; a fold's rows give up
//! their height as they fade, so the card shrinks with them, not after them.

use egui::{Pos2, Rect, Vec2};
use zikaron_ui::{card, flow, motion};

const SCREEN: Vec2 = Vec2::new(800.0, 900.0);

struct Frames {
    ctx: egui::Context,
    t: f64,
}

impl Frames {
    fn new() -> Frames {
        let ctx = egui::Context::default();
        let _ = zikaron_ui::skin::dress(&ctx);
        Frames { ctx, t: 1.0 }
    }

    /// One frame `dt` after the last; the height `add` took in the panel.
    fn run(&mut self, dt: f64, mut add: impl FnMut(&mut egui::Ui)) -> f32 {
        self.t += dt;
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(self.t), ..Default::default() };
        let mut h = 0.0;
        let _ = self.ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show_inside(root, |ui| {
                let top = ui.cursor().min.y;
                add(ui);
                h = ui.cursor().min.y - top;
            });
        });
        h
    }
}

/// A card `h` high inside an entrance keyed `entry` (as a page, a tab or a sheet step draws its body).
fn entered(ui: &mut egui::Ui, entry: u64, h: f32) {
    motion::shifted(ui, Vec2::ZERO, 1.0, entry, |ui| {
        card::card(ui, |ui| {
            let _ = ui.allocate_exact_size(egui::vec2(200.0, h), egui::Sense::hover());
        });
    });
}

#[test]
fn new_content_is_first_seen_at_its_own_size() {
    let mut f = Frames::new();
    let mut tall = 0.0;
    for _ in 0..4 {
        tall = f.run(0.05, |ui| entered(ui, 1, 300.0));
    }
    let short = f.run(0.0, |ui| entered(ui, 2, 100.0));
    for _ in 0..3 {
        let _ = f.run(0.0, |ui| entered(ui, 2, 100.0));
    }
    let settled = f.run(0.5, |ui| entered(ui, 2, 100.0));
    assert!(tall > short + 150.0, "the new page's card is its own height at once, not the old one's: {tall} then {short}");
    assert!((short - settled).abs() < 0.5, "and stays there: {short} then {settled}");
    // The same content changing size eases.
    let grown = f.run(0.0, |ui| entered(ui, 2, 200.0));
    let _ = f.run(0.0, |ui| entered(ui, 2, 200.0));
    assert!(grown < settled + 100.0 - 0.5, "the same page's card eases to its new height: {settled} toward {grown}");
}

#[test]
fn a_block_around_an_easing_block_moves_with_it() {
    let mut f = Frames::new();
    let draw = |h: f32| {
        move |ui: &mut egui::Ui| {
            let _ = flow::grow(ui, egui::Id::new("outer"), |ui| {
                let top = ui.cursor().min;
                let (_, inner) = flow::grow(ui, egui::Id::new("inner"), |ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(200.0, h), egui::Sense::hover());
                    ((), r)
                });
                ((), Rect::from_min_max(top, inner.max))
            });
        }
    };
    for _ in 0..4 {
        let _ = f.run(0.05, draw(100.0));
    }
    let start = f.run(0.0, draw(100.0));
    let mut gaps = Vec::new();
    for _ in 0..30 {
        gaps.push(f.run(1.0 / 60.0, draw(300.0)));
    }
    // The outer height moves from the first frames on, in step with the inner one (one ease, not one after another):
    // it is past half way by the inner ease's own half time, and still by its end.
    let moving: Vec<_> = gaps.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(moving.iter().take(4).all(|d| *d > 0.5), "the outer block moves from the first frames on: {gaps:?}");
    let end = *gaps.last().unwrap_or(&0.0);
    let slow = 0.3 * zikaron_ui::tokens::PACE;
    let at_end_of_ease = gaps[(slow * 60.0).ceil() as usize];
    assert!((at_end_of_ease - end).abs() < 1.0, "it has arrived when the inner ease ends ({slow} s), not one ease later: {start} {gaps:?}");
}

#[test]
fn a_folds_rows_give_up_their_height_as_they_fade() {
    let mut f = Frames::new();
    let draw = |ui: &mut egui::Ui| {
        card::form(ui, |ui, f| {
            let _ = f.fold_from(ui, "rows", "Rows", "", true, |ui, f| {
                for i in 0..4 {
                    f.note(ui, &format!("row {i}"));
                }
            });
        });
    };
    let mut open = 0.0;
    for _ in 0..8 {
        open = f.run(0.1, draw);
    }
    // Pressed shut: the fold's head is its first row; its middle line is where to press.
    let head_y = 22.0;
    let press = |at: Pos2| vec![egui::Event::PointerMoved(at), egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() }, egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() }];
    f.t += 1.0 / 60.0;
    let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(f.t), events: press(Pos2::new(200.0, head_y + 8.0)), ..Default::default() };
    let _ = f.ctx.run_ui(input, |root| {
        egui::CentralPanel::default().show_inside(root, |ui| draw(ui));
    });
    let mut heights = Vec::new();
    for _ in 0..30 {
        heights.push(f.run(1.0 / 60.0, draw));
    }
    let closed = *heights.last().unwrap_or(&0.0);
    assert!(closed < open - 40.0, "it closed: {open} to {closed}");
    // Half way through the fold's own time the card is well on its way down, not still standing at full height.
    let half = heights[((0.3 * zikaron_ui::tokens::PACE) / 2.0 * 60.0) as usize];
    assert!(half < open - (open - closed) * 0.25, "the card shrinks while the rows fade: {open} → {half} → {closed} ({heights:?})");
}

/// A key is as wide as what it says now: at rest its words, while busy its busy saying (its ring with it); it eases
/// from one width to the other as one saying comes for the other, and back when the work ends.
#[test]
fn a_key_is_as_wide_as_what_it_says_now() {
    use zikaron_ui::button::{self, Phase, Role};
    let mut f = Frames::new();
    let key = |phase| button::Key::new("迁移", Role::Secondary).phase(phase).busy_text("正在迁移数据目录");
    let run = |f: &mut Frames, phase: Phase, dt: f64| {
        let mut w = 0.0;
        let _ = f.run(dt, |ui| {
            ui.horizontal(|ui| w = button::show(ui, key(phase)).rect.width());
        });
        w
    };
    let rest = run(&mut f, Phase::Idle, 0.1);
    let _ = run(&mut f, Phase::Idle, 0.5);
    let rest = rest.max(run(&mut f, Phase::Idle, 0.016));
    let mut busy = Vec::new();
    for _ in 0..40 {
        busy.push(run(&mut f, Phase::Busy { frac: None }, 0.016));
    }
    let wide = *busy.last().expect("frames");
    assert!(wide > rest + 40.0, "busy, the key takes its saying's width: {rest} at rest, {wide} busy");
    assert!(busy.iter().any(|w| *w > rest + 1.0 && *w < wide - 1.0), "on its way through the widths between: {busy:?}");
    let mut back = 0.0;
    for _ in 0..40 {
        back = run(&mut f, Phase::Idle, 0.016);
    }
    assert!((back - rest).abs() < 0.5, "at rest again, its words' width: {back} against {rest}");
    let mut g = Frames::new();
    let mut plain = 0.0;
    for _ in 0..3 {
        let _ = g.run(0.1, |ui| {
            ui.horizontal(|ui| plain = button::show(ui, button::Key::new("迁移", Role::Secondary)).rect.width());
        });
    }
    assert!((rest - plain).abs() < 0.5, "at rest no room is kept for the busy saying: {rest} against {plain} without one");
}
