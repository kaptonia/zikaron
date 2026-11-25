use zikaron_ui::egui::{self, pos2, Event, Pos2, Rect};
use zikaron_ui::palette::{c, C};
use zikaron_ui::{card, fields, layer, tokens};

const SCREEN: egui::Vec2 = egui::Vec2 { x: 900.0, y: 700.0 };

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

fn texts(out: &egui::FullOutput) -> Vec<(String, Rect)> {
    out.shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Text(t) => Some((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()))),
            _ => None,
        })
        .collect()
}

fn red_edge(out: &egui::FullOutput) -> bool {
    fn red(s: &egui::Shape) -> bool {
        match s {
            egui::Shape::Rect(r) => r.stroke.width > 0.0 && r.stroke.color == c(C::Bad),
            egui::Shape::Vec(v) => v.iter().any(red),
            _ => false,
        }
    }
    out.shapes.iter().any(|cs| red(&cs.shape))
}

fn stacked(f: &mut Frames, line: Option<(&'static str, bool)>, w: f32, real: bool) -> (Rect, f32, egui::FullOutput) {
    let mut got = (Rect::NOTHING, 0.0);
    let mut text = String::from("x");
    let mut out = egui::FullOutput::default();
    for _ in 0..3 {
        out = f.run(0.1, vec![], |ctx| {
            egui::Area::new(egui::Id::new("reason-stack")).fixed_pos(pos2(40.0, 40.0)).show(ctx, |ui| {
                ui.set_width(w);
                fields::stack(ui, |ui, st| {
                    let r = st.field(ui, "名称", None, line, |ui, w| if real { fields::line(ui, &mut text, "", w, fields::Look::default()).rect } else { ui.allocate_exact_size(egui::vec2(w, tokens::INPUT_H), egui::Sense::hover()).0 });
                    got = (r, ui.cursor().top());
                });
            });
        });
    }
    (got.0, got.1, out)
}

#[test]
fn a_stacked_field_keeps_its_height_and_its_control_its_place_whatever_it_says() {
    let mut f = Frames::new();
    let (none, end, _) = stacked(&mut f, None, 360.0, false);
    for line in [Some(("地址要以 0x 开头", true)), Some(("已存", false)), Some(("", true))] {
        let (at, after, out) = stacked(&mut f, line, 360.0, false);
        assert_eq!(at, none, "the control holds its place with {line:?}");
        assert_eq!(after, end, "the field is as tall with {line:?}");
        if let Some((s, _)) = line.filter(|(s, _)| !s.is_empty()) {
            let (_, r) = texts(&out).into_iter().find(|(t, _)| t == s).expect("the reason drawn");
            assert!(r.bottom() <= at.top(), "said above the control, never under it: {r:?} over {at:?}");
            assert!((r.right() - at.right()).abs() < 1.0, "its right end on the control's right edge: {r:?} {at:?}");
        }
    }
}

#[test]
fn a_row_field_keeps_its_height_and_its_control_its_place_whatever_it_says() {
    let mut f = Frames::new();
    let run = |f: &mut Frames, line: Option<(&'static str, bool)>| {
        let mut got = (Rect::NOTHING, 0.0);
        let mut out = egui::FullOutput::default();
        for _ in 0..3 {
            out = f.run(0.1, vec![], |ctx| {
                egui::Area::new(egui::Id::new("reason-row")).fixed_pos(pos2(40.0, 40.0)).show(ctx, |ui| {
                    ui.set_width(560.0);
                    card::form(ui, |ui, form| {
                        got.0 = form.field(ui, "周期", None, line, |ui, w| ui.allocate_exact_size(egui::vec2(w.min(200.0), tokens::INPUT_H), egui::Sense::hover()).0);
                    });
                    got.1 = ui.min_rect().bottom();
                });
            });
        }
        (got.0, got.1, out)
    };
    let (none, end, _) = run(&mut f, None);
    for line in [Some(("须为整数", true)), Some(("已存", false)), Some(("", true))] {
        let (at, after, out) = run(&mut f, line);
        assert_eq!(at, none, "the control holds its place with {line:?}");
        assert_eq!(after, end, "the row is as tall with {line:?}");
        if let Some((s, _)) = line.filter(|(s, _)| !s.is_empty()) {
            let (_, r) = texts(&out).into_iter().find(|(t, _)| t == s).expect("the reason drawn");
            assert!(r.right() <= at.left() && at.left() - r.right() <= tokens::S3, "its right end against the control: {r:?} {at:?}");
            assert!((r.center().y - at.center().y).abs() < 1.5, "on the control's middle line: {r:?} {at:?}");
        }
    }
}

#[test]
fn an_empty_field_told_wrong_is_framed_red_and_says_nothing() {
    let mut f = Frames::new();
    for _ in 0..3 {
        let _ = stacked(&mut f, None, 360.0, true);
    }
    let (_, _, quiet) = stacked(&mut f, None, 360.0, true);
    assert!(!red_edge(&quiet), "nothing told, nothing red");
    let (_, _, out) = stacked(&mut f, Some(("", true)), 360.0, true);
    assert!(red_edge(&out), "told wrong: the control's frame is red although the caller's look says nothing of it");
    let said: Vec<String> = texts(&out).into_iter().map(|(t, _)| t).filter(|t| t != "名称" && t != "x").collect();
    assert!(said.is_empty(), "no words: {said:?}");
}

#[test]
fn a_reason_too_long_stands_whole_under_the_name() {
    let long: &'static str = "这一格填的地址读不成:要以 https:// 开头,不带查询串,也不带片段,主机名要能解析";
    let mut f = Frames::new();
    let (at, _, out) = stacked(&mut f, Some((long, true)), 240.0, true);
    let drawn = texts(&out);
    assert!(drawn.iter().all(|(t, _)| !t.ends_with('\u{2026}')), "nothing cut short: {drawn:?}");
    let (_, words) = drawn.iter().find(|(t, _)| t == long).cloned().unwrap_or_else(|| panic!("the whole sentence drawn: {drawn:?}"));
    let (_, name) = drawn.iter().find(|(t, _)| t == "名称").cloned().unwrap_or_else(|| panic!("the name drawn: {drawn:?}"));
    assert!(words.top() >= name.bottom() - 0.5, "under the name, not on its line: {words:?} {name:?}");
    assert!(words.bottom() <= at.top() + 0.5, "before the control: {words:?} {at:?}");
    assert!((words.left() - name.left()).abs() < 0.5 && words.right() <= at.right() + 1.0, "from the name's left edge, within the field's right edge: {words:?} {name:?} {at:?}");
    assert!(words.height() > name.height() * 1.5, "wrapped onto more than one line: {words:?}");
}
