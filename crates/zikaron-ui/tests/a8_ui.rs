use zikaron_ui::egui::{self, Event, Pos2, Rect};
use zikaron_ui::fonts::{self, Face, Found, Place, Role};
use zikaron_ui::{button, input, layer};

fn central<R>(ctx: &egui::Context, panel: egui::CentralPanel, add: impl FnOnce(&mut egui::Ui) -> R) -> egui::InnerResponse<R> {
    let mut top = egui::Ui::new(ctx.clone(), egui::Id::new((ctx.viewport_id(), "central_panel")), egui::UiBuilder::new().layer_id(egui::LayerId::background()).max_rect(ctx.content_rect()));
    top.set_clip_rect(ctx.content_rect());
    panel.show_inside(&mut top, add)
}

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

    fn run(&mut self, dt: f64, events: Vec<Event>, mut ui: impl FnMut(&egui::Context)) -> egui::FullOutput {
        self.t += dt;
        let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(self.t), events, ..Default::default() };
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(2.0);
        self.ctx.run_ui(input, |root| {
            let ctx = &root.ctx().clone();
            ui(ctx);
            layer::fade_out(ctx);
        })
    }
}

#[test]
fn a_missing_font_role_is_handed_back_by_name() {
    let ctx = egui::Context::default();
    let missing = fonts::install(&ctx, &Found::none());
    assert_eq!(missing, Role::ALL.to_vec(), "with nothing found every role is said missing");
    let mut names: Vec<&str> = missing.iter().map(|r| r.as_str()).collect();
    assert!(names.iter().all(|n| !n.trim().is_empty()), "every missing role has a name: {names:?}");
    names.sort();
    names.dedup();
    assert_eq!(names.len(), Role::ALL.len(), "each missing role has a name of its own");
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

struct Seen {
    focus: bool,
    id: egui::Id,
}

fn page(ctx: &egui::Context, text: &mut String, row: bool, area: bool, ask: bool, plain: bool) -> Seen {
    let mut seen = Seen { focus: false, id: egui::Id::NULL };
    central(ctx, egui::CentralPanel::default(), |ui| {
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
        let mut with_row = Seen { focus: false, id: egui::Id::NULL };
        f.run(0.1, typed("甲"), |ctx| with_row = page(ctx, &mut text, true, area, false, false));
        assert_eq!(with_row.id, first.id, "{which}: the same id with a row above it");
        assert!(with_row.focus, "{which}: still focused with a row above it");
        assert_eq!(text, "甲", "{which}: the keystroke landed in the box");
        let mut without = Seen { focus: false, id: egui::Id::NULL };
        f.run(0.1, typed("乙"), |ctx| without = page(ctx, &mut text, false, area, false, false));
        assert_eq!(without.id, first.id, "{which}: the same id once the row is gone");
        assert!(without.focus, "{which}: still focused once the row is gone");
        assert_eq!(text, "甲乙", "{which}: the next keystroke landed in the box too");
    }
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
