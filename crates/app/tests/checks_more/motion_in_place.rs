//! The window's in-place changes, read frame by frame over its own pages: switching seats moves the
//! rail's rows that stay in one run from where they were to where they belong; switching the language cross-fades
//! the changed words in place from the very frame of the switch and is done within 0.3 s, on the same page.

use std::sync::{Arc, Mutex};

use app::action::{apply, Action, Applied};
use app::lang::{t, Key};
use app::nav::{Place, Section};
use app::shell::Shell;
use zikaron_ui::egui;

type Said = (String, egui::Rect, f32);

/// Every frame's words as the window drew them: where, and how opaque.
struct Capture {
    frames: Arc<Mutex<Vec<Vec<Said>>>>,
}

fn walk(s: &egui::Shape, out: &mut Vec<Said>) {
    match s {
        egui::Shape::Text(x) => out.push((x.galley.job.text.clone(), x.galley.rect.translate(x.pos.to_vec2()), x.opacity_factor)),
        egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
        _ => {}
    }
}

impl egui::Plugin for Capture {
    fn debug_name(&self) -> &'static str {
        "u1-capture"
    }

    fn output_hook(&mut self, output: &mut egui::FullOutput) {
        let mut frame = Vec::new();
        for c in &output.shapes {
            walk(&c.shape, &mut frame);
        }
        self.frames.lock().unwrap().push(frame);
    }
}

fn settle(shell: &mut Shell) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    shell.drain();
    while !shell.tasks.flying().is_empty() && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    shell.drain();
}

fn a_home(tag: &str) -> Shell {
    super::vault_open();
    let ctx = egui::Context::default();
    let mut shell = Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.reread_vault();
    let root = std::env::temp_dir().join(format!("zk-u1-{tag}-{}", std::process::id())).join("home");
    let _ = std::fs::remove_dir_all(&root);
    super::seated(&mut shell, &root);
    assert!(matches!(apply(&mut shell, Action::OpenHome { root: root.display().to_string() }), Applied::Homed { .. }));
    settle(&mut shell);
    shell
}

/// Draw `place` (five frames to settle, then `steps`), every frame's words.
fn frames(shell: Shell, place: Place, steps: Vec<(f64, Vec<egui::Event>)>) -> (Shell, Vec<Vec<Said>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    ctx.add_plugin(Capture { frames: seen.clone() });
    let (shell, _) = app::window::probe_shell_input(&ctx, shell, place, steps, 1180.0, 760.0);
    let got = seen.lock().unwrap().clone();
    (shell, got)
}

fn click(at: egui::Pos2) -> Vec<egui::Event> {
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    vec![egui::Event::PointerMoved(at), press(true), press(false)]
}

fn find<'a>(f: &'a [Said], w: &str, x_max: f32) -> Option<&'a Said> {
    f.iter().find(|s| s.0 == w && s.1.center().x < x_max && s.2 > 0.5)
}

/// Switching seats: a rail row both seats have (the alerts item) moves from where it was to where it belongs,
/// starting where it was, in one direction, and is there within 0.4 s.
#[test]
fn switching_seats_moves_the_rail_rows_in_one_run() {
    if super::alone_in(module_path!(), "switching_seats_moves_the_rail_rows_in_one_run") {
        return;
    }
    let shell = a_home("seat");
    let (shell, before) = frames(shell, Place::Home, vec![(0.1, Vec::new())]);
    let last = before.last().expect("frames");
    let seat = find(last, t(Key::IdSeatGrantee), 216.0).map(|s| s.1.center()).expect("the seat switch");
    let alerts = t(Key::NavAlertsView);
    let from = find(last, alerts, 216.0).map(|s| s.1.center().y).expect("the alerts item");
    let mut steps = vec![(1.0 / 60.0, click(seat))];
    steps.extend((0..40).map(|_| (1.0 / 60.0, Vec::new())));
    let (shell, seen) = frames(shell, Place::Home, steps);
    assert_eq!(shell.settings.role, app::roles::Role::Grantee, "the seat switched");
    let ys: Vec<f32> = seen.iter().skip(5).filter_map(|f| find(f, alerts, 216.0).map(|s| s.1.center().y)).collect();
    assert!(ys.len() >= 30, "the item drawn every frame: {}", ys.len());
    let to = *ys.last().unwrap();
    assert!((to - from).abs() > 20.0, "it moved: {from} → {to}");
    assert!((ys[0] - from).abs() < 2.0, "the first frame starts where it was: {} vs {from}", ys[0]);
    let dir = (to - from).signum();
    for w in ys.windows(2) {
        assert!((w[1] - w[0]) * dir >= -0.01, "one direction, no jump back: {ys:?}");
    }
    let there = ys.iter().position(|y| (y - to).abs() < 0.5).unwrap_or(ys.len());
    assert!(there as f32 / 60.0 <= 0.4, "there within 0.4 s ({there} frames)");
}

/// Switching the language: the frame after the press is already crossing (changed words coming in by their
/// colour, the old ones going where they were), the page is the same page, and within 0.3 s every word is whole.
#[test]
fn switching_the_language_cross_fades_in_place() {
    if super::alone_in(module_path!(), "switching_the_language_cross_fades_in_place") {
        return;
    }
    let shell = a_home("lang");
    let place = Place::Settings(Section::Language);
    let (shell, before) = frames(shell, place, vec![(0.1, Vec::new())]);
    let last = before.last().expect("frames");
    let old_word = t(Key::Language).to_string();
    let other = app::lang::Lang::ALL.iter().copied().find(|l| *l != app::lang::lang()).expect("two languages");
    let cell = find(last, other.label(), 2000.0).map(|s| s.1.center()).expect("the other language's cell");
    let mut steps = vec![(1.0 / 60.0, click(cell))];
    steps.extend((0..24).map(|_| (1.0 / 60.0, Vec::new())));
    let (_shell, seen) = frames(shell, place, steps);
    assert_eq!(app::lang::lang(), other, "the language switched");
    let new_word = t(Key::Language).to_string();
    assert_ne!(old_word, new_word);
    // The frame after the press: the old title fading where it was, the new one coming in.
    let next = &seen[6];
    let old_now = next.iter().find(|s| s.0 == old_word);
    let new_now = next.iter().find(|s| s.0 == new_word);
    assert!(old_now.map(|s| s.2 < 1.0 && s.2 > 0.0).unwrap_or(false), "the old words fading in place: {old_now:?}");
    assert!(new_now.map(|s| s.2 < 1.0).unwrap_or(false), "the new words coming in: {new_now:?}");
    let end = seen.iter().skip(6 + 18).next().expect("a frame 0.3 s on");
    assert!(end.iter().all(|s| s.2 >= 0.999) && !end.iter().any(|s| s.0 == old_word), "whole within 0.3 s");
    let _ = app::lang::set(app::lang::Lang::ALL[0]);
}
