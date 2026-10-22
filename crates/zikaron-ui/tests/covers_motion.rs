//! The lock's and the first-run wizard's motion, read frame by frame in windowless frames: each part is drawn at
//! set times and what it drew (sizes, places, opacities, layer transforms) is read back against the design's
//! times and curves. `--nocapture` prints the readings.

use zikaron_ui::egui::{self, pos2, vec2, Rect};
use zikaron_ui::{covers, motion, sheetparts};

const SCREEN: Rect = Rect { min: pos2(0.0, 0.0), max: pos2(1180.0, 760.0) };

fn frame(ctx: &egui::Context, t: f64, mut f: impl FnMut(&mut egui::Ui)) -> egui::FullOutput {
    let input = egui::RawInput { screen_rect: Some(SCREEN), time: Some(t), ..Default::default() };
    ctx.run_ui(input, |ui| f(ui))
}

fn ctx() -> egui::Context {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    ctx
}

/// Every circle drawn (centre, radius, filled), nested ones too.
fn circles(out: &egui::FullOutput) -> Vec<(egui::Pos2, f32, bool)> {
    fn walk(s: &egui::Shape, out: &mut Vec<(egui::Pos2, f32, bool)>) {
        match s {
            egui::Shape::Circle(c) => out.push((c.center, c.radius, c.fill.a() > 0)),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for c in &out.shapes {
        walk(&c.shape, &mut v);
    }
    v
}

/// Every text drawn: its words, where, and its colour's opacity.
fn texts(out: &egui::FullOutput) -> Vec<(String, Rect, u8)> {
    fn walk(s: &egui::Shape, out: &mut Vec<(String, Rect, u8)>) {
        match s {
            egui::Shape::Text(x) => {
                let a = x.galley.rows.first().and_then(|r| r.glyphs.first()).map(|_| x.fallback_color.a()).unwrap_or(0);
                let a = if x.opacity_factor < 1.0 { (a as f32 * x.opacity_factor) as u8 } else { a };
                out.push((x.galley.job.text.clone(), x.visual_bounding_rect(), a))
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
            _ => {}
        }
    }
    let mut v = Vec::new();
    for c in &out.shapes {
        walk(&c.shape, &mut v);
    }
    v
}

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

/// The cover comes in over 0.3 s, settling from 1.004 to its size over 0.4 s; under reduced motion it does not
/// scale.
#[test]
fn a_cover_fades_in_and_settles_from_1004() {
    let c = ctx();
    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new(("zikaron-cover-as-designed", "t")));
    let mut seen = Vec::new();
    for k in 0..=10 {
        let t = 1.0 + k as f64 * 0.05;
        let _ = frame(&c, t, |ui| {
            let _ = covers::cover(ui.ctx(), "t", covers::Ground::Lit, |_| ());
        });
        let s = c.layer_transform_to_global(layer).map(|x| x.scaling).unwrap_or(1.0);
        seen.push((k as f32 * 0.05, s));
    }
    eprintln!("cover scale by time: {seen:?}");
    assert!(close(seen[0].1, 1.004, 0.0005), "starts at 1.004");
    assert!(seen[4].1 > 1.0 && seen[4].1 < 1.002, "on its way at 0.2 s");
    assert!(close(seen[8].1, 1.0, 0.0001), "settled at 0.4 s");
    let r = ctx();
    motion::set_reduced(&r, true);
    let _ = frame(&r, 1.0, |ui| {
        motion::set_reduced(ui.ctx(), true);
        let _ = covers::cover(ui.ctx(), "t", covers::Ground::Lit, |_| ());
    });
    let _ = frame(&r, 1.01, |ui| {
        motion::set_reduced(ui.ctx(), true);
        let _ = covers::cover(ui.ctx(), "t", covers::Ground::Lit, |_| ());
    });
    assert!(close(r.layer_transform_to_global(layer).map(|x| x.scaling).unwrap_or(1.0), 1.0, 1e-4), "reduced motion: no scale");
}

/// Once the lock goes, the window comes back from half its opacity and 0.993 to the whole over 0.45 s.
#[test]
fn the_window_comes_back_from_half_and_0993_over_045() {
    let c = ctx();
    let _ = frame(&c, 1.0, |ui| covers::app_in(ui.ctx(), true));
    let mut seen = Vec::new();
    for k in 0..=10 {
        let t = 1.1 + k as f64 * 0.05;
        let _ = frame(&c, t, |ui| covers::app_in(ui.ctx(), false));
        seen.push((k as f32 * 0.05, covers::app_in_reading(&c)));
    }
    eprintln!("window coming back (opacity, scale) by time: {seen:?}");
    let (o0, s0) = seen[0].1.expect("reading at the start");
    assert!(close(o0, 0.5, 0.01) && close(s0, 0.993, 0.0005), "from half and 0.993");
    let (o4, s4) = seen[4].1.expect("on its way");
    assert!(o4 > 0.8 && o4 < 1.0 && s4 > 0.998, "glides: most of the way by 0.2 s");
    assert!(seen[10].1.is_none(), "whole again by 0.5 s");
}

/// A passcode dot pops in from half its size on the spring over 0.35 s; a refused row shakes 8, 7, 5, 3 over 0.42 s;
/// both at the kit's pace (read at those shares of the paced times).
#[test]
fn a_dot_pops_and_a_refused_row_shakes() {
    let c = ctx();
    let mut pin = zikaron_ui::secret::Secret::of("a");
    let mut radii = Vec::new();
    for k in 0..=8 {
        let t = 2.0 + k as f64 * 0.05 * f64::from(zikaron_ui::tokens::PACE);
        let out = frame(&c, t, |ui| {
            let _ = sheetparts::pins(ui, "dots", &mut pin, 8, None, false);
        });
        let r = circles(&out).iter().filter(|x| x.2).map(|x| x.1).fold(0.0, f32::max);
        radii.push((k as f32 * 0.05, r));
    }
    eprintln!("dot radius by time: {radii:?}");
    assert!(close(radii[0].1, 2.25, 0.05), "from half (4.5 × 0.5)");
    assert!(radii[3].1 > 4.0, "most of the way by 0.15 s (spring)");
    assert!(close(radii[8].1, 4.5, 0.01), "whole by 0.4 s");
    let at = 5.0;
    let mut swings = Vec::new();
    for p in [0.0f64, 0.2, 0.4, 0.6, 0.8, 1.0] {
        let _ = frame(&c, at + p * 0.42 * f64::from(zikaron_ui::tokens::PACE), |_| ());
        swings.push((p, sheetparts::shake_of(&c, Some(at))));
    }
    eprintln!("shake by share of 0.42 s: {swings:?}");
    let want = [0.0, -8.0, 7.0, -5.0, 3.0, 0.0];
    for ((_, got), w) in swings.iter().zip(want) {
        assert!(close(*got, w, 0.01), "{swings:?}");
    }
}

/// The wizard's steps: a step that becomes done pops from 0.85 on the spring over 0.5 s; the ring and fill turn
/// over 0.35 s.
#[test]
fn a_step_that_becomes_done_pops_and_turns() {
    let c = ctx();
    let draw = |done: bool| {
        move |ui: &mut egui::Ui| {
            let rows = [covers::Step { label: "one", done, reachable: false }, covers::Step { label: "two", done: false, reachable: false }];
            let _ = covers::steps(ui, &rows, 1);
        }
    };
    let _ = frame(&c, 1.0, draw(false));
    let _ = frame(&c, 1.5, draw(false));
    let mut seen = Vec::new();
    for k in 0..=12 {
        let t = 2.0 + k as f64 * 0.05;
        let out = frame(&c, t, draw(true));
        // The first step's filled circle (the ok colour behind its check).
        let r = circles(&out).iter().filter(|x| x.2 && x.0.y < 20.0).map(|x| x.1).fold(0.0, f32::max);
        seen.push((k as f32 * 0.05, r));
    }
    eprintln!("done step's fill radius by time: {seen:?}");
    // The fill fades in with the turn (0.35 s): its first drawn frame is already under way from 0.85.
    assert!(seen[1].1 > 8.5 && seen[1].1 < 9.7, "from 0.85 of 10");
    assert!(close(seen[12].1, 10.0, 0.02), "whole by 0.6 s");
}

/// The way out fades in over 0.3 s rising 4 over 0.4 s; hidden, it keeps its room and is not seen.
#[test]
fn the_way_out_fades_in_rising_4() {
    let c = ctx();
    let draw = |shown: bool| move |ui: &mut egui::Ui| {
        let _ = covers::exit_item(ui, "Exit setup", "esc", shown);
    };
    let out = frame(&c, 1.0, draw(false));
    assert!(texts(&out).iter().all(|t| t.0 != "Exit setup"), "hidden: not drawn");
    let mut seen = Vec::new();
    for k in 0..=10 {
        let t = 2.0 + k as f64 * 0.05;
        let out = frame(&c, t, draw(true));
        let y = texts(&out).iter().find(|t| t.0 == "Exit setup").map(|t| t.1.center().y).unwrap_or(f32::NAN);
        seen.push((k as f32 * 0.05, y));
    }
    eprintln!("way out's words centre y by time: {seen:?}");
    let rest = seen[10].1;
    assert!(seen[1].1 > rest + 1.0, "below where it rests at first");
    assert!(close(seen[8].1, rest, 0.05), "risen by 0.4 s");
}

/// A step's card comes in from 12 on the side it is reached from on the spring over 0.38 s; the card it
/// replaces leaves toward the side it is left on.
#[test]
fn a_steps_card_slides_in_and_the_old_one_leaves_the_other_way() {
    let c = ctx();
    let over = egui::LayerId::new(egui::Order::Middle, egui::Id::new("over"));
    let rect = Rect::from_min_max(pos2(zikaron_ui::tokens::RAIL_W, 0.0), pos2(1180.0, 760.0));
    let draw = |step: u64, dir: f32| move |ui: &mut egui::Ui| {
        covers::pane(ui.ctx(), over, step, dir, rect, |ui| {
            ui.label(format!("step {step}"));
        });
        zikaron_ui::layer::fade_out(ui.ctx());
    };
    let _ = frame(&c, 1.0, draw(0, 0.0));
    let _ = frame(&c, 1.5, draw(0, 0.0));
    let new = egui::LayerId::new(egui::Order::Middle, egui::Id::new(("zikaron-wizard-pane", 1u64)));
    let old_ghost = egui::LayerId::new(egui::Order::Middle, egui::Id::new(("zikaron-wizard-pane", 0u64)).with("leaving"));
    let mut seen = Vec::new();
    for k in 0..=8 {
        let t = 2.0 + k as f64 * 0.05;
        let _ = frame(&c, t, draw(1, 1.0));
        // A layer at rest carries no transform.
        let x_new = c.layer_transform_to_global(new).map(|x| x.translation.x).unwrap_or(0.0);
        let x_old = c.layer_transform_to_global(old_ghost).map(|x| x.translation.x).unwrap_or(0.0);
        seen.push((k as f32 * 0.05, x_new, x_old));
    }
    eprintln!("(time, new card x, old card x): {seen:?}");
    assert!(close(seen[0].1, 12.0, 0.01), "the new card starts 12 to the right going forward");
    assert!(close(seen[8].1, 0.0, 0.3), "and is in place by 0.4 s");
    assert!(seen[2].2 < -1.0, "the old card leaves to the left going forward");
}

/// The done mark pops from 0.85 on the spring over 0.6 s; its check is drawn from 0.15 s in, over 0.5 s.
#[test]
fn the_done_mark_pops_and_draws_its_check() {
    let c = ctx();
    let mut seen = Vec::new();
    for k in 0..=14 {
        let t = 1.0 + k as f64 * 0.05;
        let out = frame(&c, t, |ui| sheetparts::done(ui, "d", "Done"));
        let ring = circles(&out).iter().filter(|x| !x.2).map(|x| x.1).fold(0.0, f32::max);
        let mut along = 0.0;
        for cs in &out.shapes {
            if let egui::Shape::Path(p) = &cs.shape {
                along += p.points.windows(2).map(|w| (w[1] - w[0]).length()).sum::<f32>();
            }
        }
        seen.push((k as f32 * 0.05, ring, along));
    }
    eprintln!("(time, ring radius, check drawn length): {seen:?}");
    assert!(close(seen[0].1, 21.0 * 0.85, 0.2), "ring from 0.85");
    assert!(close(seen[14].1, 21.0, 0.05), "ring whole by 0.7 s");
    assert!(seen[2].2 < 0.5, "no check before 0.15 s");
    assert!(seen[6].2 > 1.0 && seen[6].2 < seen[14].2, "the check is being drawn at 0.3 s");
}

/// A chosen radio's dot pops from half its size over 0.3 s.
#[test]
fn a_chosen_radio_dot_pops() {
    let c = ctx();
    let _ = frame(&c, 1.0, |ui| sheetparts::radio(ui, "r", false));
    let mut seen = Vec::new();
    for k in 0..=7 {
        let t = 2.0 + k as f64 * 0.05;
        let out = frame(&c, t, |ui| sheetparts::radio(ui, "r", true));
        seen.push((k as f32 * 0.05, circles(&out).iter().filter(|x| x.2).map(|x| x.1).fold(0.0, f32::max)));
    }
    eprintln!("radio dot radius by time: {seen:?}");
    assert!(close(seen[0].1, 2.25, 0.05) && close(seen[7].1, 4.5, 0.02), "{seen:?}");
}

/// The words' veil clears over 0.3 s once they are opened.
#[test]
fn the_words_veil_clears_over_03() {
    let c = ctx();
    let words: Vec<zikaron_ui::secret::Secret> = (0..12).map(|i| zikaron_ui::secret::Secret::of(&format!("w{i}"))).collect();
    let _ = frame(&c, 1.0, |ui| {
        let _ = sheetparts::words(ui, "v", Some(&words), false, "show");
    });
    let mut seen = Vec::new();
    for k in 0..=8 {
        let t = 2.0 + k as f64 * 0.05;
        let out = frame(&c, t, |ui| {
            let _ = sheetparts::words(ui, "v", Some(&words), true, "show");
        });
        let cover = texts(&out).iter().find(|x| x.0 == "show").map(|x| x.2).unwrap_or(0);
        seen.push((k as f32 * 0.05, cover));
    }
    eprintln!("veil words' opacity by time: {seen:?}");
    assert!(seen[0].1 > 200, "veiled at first");
    assert!(seen[2].1 > 0 && seen[2].1 < seen[0].1, "clearing at 0.1 s");
    assert_eq!(seen[7].1, 0, "clear by 0.35 s");
    let _ = vec2(0.0, 0.0);
}

/// A cover that has gone (`covers::gone`) takes itself and its step card away in the frame it goes: nothing of
/// either is played back over the window it uncovers. A cover left to its leaving would show both for a while.
#[test]
fn a_cover_that_has_gone_leaves_no_copy_over_the_window() {
    let drawn = |gone: bool| {
        let ctx = ctx();
        let mut t = 1.0;
        for _ in 0..30 {
            t += 1.0 / 60.0;
            let _ = frame(&ctx, t, |ui| {
                let (_, over) = covers::cover(ui.ctx(), "gone-test", covers::Ground::Clear, |ui| {
                    ui.label("cover words");
                });
                covers::pane(ui.ctx(), over, 0, 0.0, Rect::from_min_size(pos2(400.0, 200.0), vec2(300.0, 200.0)), |ui| {
                    ui.label("card words");
                });
                zikaron_ui::layer::fade_out(ui.ctx());
            });
        }
        let mut seen = Vec::new();
        for _ in 0..6 {
            t += 1.0 / 60.0;
            let out = frame(&ctx, t, |ui| {
                ui.label("page words");
                if gone {
                    covers::gone(ui.ctx(), "gone-test");
                }
                zikaron_ui::layer::fade_out(ui.ctx());
            });
            seen.extend(texts(&out).into_iter().map(|(s, _, _)| s));
        }
        seen
    };
    let left = drawn(false);
    assert!(left.iter().any(|s| s == "cover words") && left.iter().any(|s| s == "card words"), "left to its leaving, the cover and its card fade over the page: {left:?}");
    let gone = drawn(true);
    assert!(!gone.iter().any(|s| s == "cover words" || s == "card words"), "gone, neither is drawn again: {gone:?}");
    assert!(gone.iter().any(|s| s == "page words"), "and the page is: {gone:?}");
}
