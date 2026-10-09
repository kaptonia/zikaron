//! Two guarantees of the control library, checked in headless egui frames: a missing font role is reported
//! to the caller (`fonts::install`), and an input box keeps its id while the page above it changes
//! (`input::field`, `input::area`).

use zikaron_ui::egui::{self, Event, Pos2, Rect};
use zikaron_ui::fonts::{self, Face, Found, Place, Role};
use zikaron_ui::{button, input, layer};

const SCREEN: egui::Vec2 = egui::Vec2 { x: 1180.0, y: 800.0 };

struct Frames {
    ctx: egui::Context,
    t: f64,
}

impl Frames {
    fn new() -> Frames {
        let ctx = egui::Context::default();
        let _ = zikaron_ui::skin::dress(&ctx);
        Frames { ctx, t: 0.0 }
    }

    /// One frame `dt` after the last, with `events`, as the window runs one.
    fn run(&mut self, dt: f64, events: Vec<Event>, mut ui: impl FnMut(&egui::Context)) -> egui::FullOutput {
        self.t += dt;
        let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(self.t), events, ..Default::default() };
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(2.0);
        self.ctx.run(input, |ctx| {
            ui(ctx);
            layer::fade_out(ctx);
        })
    }
}

/// A font role that is missing is named, never drawn silently as boxes: `install` hands back every role it
/// could not install, by the role itself, whose name (`Role::as_str`) is its own and not empty. Nothing found
/// hands back all five; a face that was found but whose file does not read (a system face whose path is gone)
/// counts as missing and is handed back by name too, while the faces that did install are not.
#[test]
fn a_missing_font_role_is_handed_back_by_name() {
    let ctx = egui::Context::default();
    // Nothing found: every role comes back, each by its own name.
    let missing = fonts::install(&ctx, &Found::none());
    assert_eq!(missing, Role::ALL.to_vec(), "with nothing found every role is said missing");
    let mut names: Vec<&str> = missing.iter().map(|r| r.as_str()).collect();
    assert!(names.iter().all(|n| !n.trim().is_empty()), "every missing role has a name: {names:?}");
    names.sort();
    names.dedup();
    assert_eq!(names.len(), Role::ALL.len(), "each missing role has a name of its own");
    // The embedded faces found, and one system face whose file is not on disk: only that one comes back.
    let gone = std::env::temp_dir().join(format!("zk-a8-font-gone-{}", std::process::id())).join("PingFang.ttc");
    let mut faces: Vec<Face> = fonts::find().faces.into_iter().filter(|f| f.place == Place::Embedded).collect();
    assert!(!faces.is_empty(), "the embedded faces are always found");
    faces.retain(|f| f.role != Role::Strong);
    faces.push(Face { role: Role::Strong, file: "PingFang.ttc", index: 11, place: Place::System, path: Some(gone), bytes: 0 });
    let installed: Vec<Role> = faces.iter().filter(|f| f.place == Place::Embedded).map(|f| f.role).collect();
    let missing = fonts::install(&ctx, &Found { faces, missing: Vec::new() });
    assert!(missing.contains(&Role::Strong), "an unreadable face is said missing: {missing:?}");
    assert_eq!(missing.iter().find(|r| **r == Role::Strong).map(|r| r.as_str()), Some("strong"), "by its name");
    for r in installed {
        assert!(!missing.contains(&r), "{} installed and is not said missing", r.as_str());
    }
}

/// What one frame of the page drew: whether the field had focus, and its response id.
struct Seen {
    focus: bool,
    id: egui::Id,
}

/// The page: a key and a line of words above the box when `row` is set, then the box. `area` picks the
/// multi-line area instead of the single-line field. On `ask` the box asks for focus through its own
/// response.
fn page(ctx: &egui::Context, text: &mut String, row: bool, area: bool, ask: bool, plain: bool) -> Seen {
    let mut seen = Seen { focus: false, id: egui::Id::NULL };
    egui::CentralPanel::default().show(ctx, |ui| {
        if row {
            let _ = button::key(ui, "上面多出来的一行", button::Role::Secondary, true);
            ui.label("又一行字");
        }
        let resp = if plain {
            ui.add(egui::TextEdit::singleline(text))
        } else if area {
            input::area_words(ui, text, 3, "备注")
        } else {
            input::line(ui, text, "名字")
        };
        if ask {
            resp.request_focus();
        }
        seen = Seen { focus: resp.has_focus(), id: resp.id };
    });
    seen
}

fn typed(s: &str) -> Vec<Event> {
    vec![Event::Text(s.to_string())]
}

/// An input box keeps its id: a row added above it, or taken away again, while it has focus does not move
/// its id, so it keeps focus and the next keystrokes land in it. Both the single-line field and the
/// multi-line area. The same row above does move the id egui counts for a box drawn without one (a plain
/// `TextEdit`), which loses focus: the change on the page is the one that would lose it.
#[test]
fn an_input_box_keeps_focus_when_a_row_comes_or_goes_above_it() {
    for area in [false, true] {
        let which = if area { "area" } else { "line" };
        let mut f = Frames::new();
        let mut text = String::new();
        f.run(0.1, vec![], |ctx| {
            page(ctx, &mut text, false, area, true, false);
        });
        let mut first = Seen { focus: false, id: egui::Id::NULL };
        f.run(0.1, vec![], |ctx| first = page(ctx, &mut text, false, area, false, false));
        assert!(first.focus, "{which}: focused");
        // A row comes above it; the person goes on typing.
        let mut with_row = Seen { focus: false, id: egui::Id::NULL };
        f.run(0.1, typed("甲"), |ctx| with_row = page(ctx, &mut text, true, area, false, false));
        assert_eq!(with_row.id, first.id, "{which}: the same id with a row above it");
        assert!(with_row.focus, "{which}: still focused with a row above it");
        assert_eq!(text, "甲", "{which}: the keystroke landed in the box");
        // The row goes away again.
        let mut without = Seen { focus: false, id: egui::Id::NULL };
        f.run(0.1, typed("乙"), |ctx| without = page(ctx, &mut text, false, area, false, false));
        assert_eq!(without.id, first.id, "{which}: the same id once the row is gone");
        assert!(without.focus, "{which}: still focused once the row is gone");
        assert_eq!(text, "甲乙", "{which}: the next keystroke landed in the box too");
    }
    // The same change around a box with a counted id: its id moves and focus is lost, so the row above is a
    // change that would lose focus.
    let mut f = Frames::new();
    let mut text = String::new();
    f.run(0.1, vec![], |ctx| {
        page(ctx, &mut text, false, false, true, true);
    });
    let mut before = Seen { focus: false, id: egui::Id::NULL };
    f.run(0.1, vec![], |ctx| before = page(ctx, &mut text, false, false, false, true));
    assert!(before.focus, "the plain box took focus");
    let mut after = Seen { focus: false, id: egui::Id::NULL };
    f.run(0.1, typed("丙"), |ctx| after = page(ctx, &mut text, true, false, false, true));
    assert_ne!(after.id, before.id, "a counted id moves when a row comes above it");
    assert!(!after.focus && text.is_empty(), "and the plain box lost focus and the keystroke");
}
