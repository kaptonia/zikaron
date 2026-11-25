use zikaron_ui::egui::{self, Pos2, Rect};
use zikaron_ui::{card, fields, probe};

fn frame(ctx: &egui::Context, events: Vec<egui::Event>, mut add: impl FnMut(&mut egui::Ui)) -> egui::FullOutput {
    let t = ctx.input(|i| i.time) + 0.1;
    let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 400.0))), time: Some(t), events, ..Default::default() };
    input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(2.0);
    ctx.run_ui(input, |root| {
        probe::begin(root.ctx());
        egui::CentralPanel::default().show_inside(root, |ui| add(ui));
    })
}

fn dressed() -> egui::Context {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    ctx
}

fn key(k: egui::Key) -> Vec<egui::Event> {
    vec![egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }, egui::Event::Key { key: k, physical_key: None, pressed: false, repeat: false, modifiers: egui::Modifiers::NONE }]
}

fn press_on(out: &egui::FullOutput, said: &str) -> Vec<egui::Event> {
    let at = out
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text() == said => Some(egui::Rect::from_min_size(t.pos, t.galley.size()).center()),
            _ => None,
        })
        .min_by(|a, b| a.x.total_cmp(&b.x))
        .unwrap_or_else(|| panic!("{said:?} is drawn"));
    let b = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    vec![egui::Event::PointerMoved(at), b(true), b(false)]
}

fn words(out: &egui::FullOutput) -> Vec<String> {
    let mut v = Vec::new();
    for cs in &out.shapes {
        if let egui::Shape::Text(t) = &cs.shape {
            v.push(t.galley.text().to_string());
        }
    }
    v
}

const WORDS: card::SettingWords<'static> = card::SettingWords { save: "保存间隔", saved: "已保存", unit: "秒", hint: "仅手动", key_w: 92.0 };

fn check(x: &str) -> Result<(), &'static str> {
    if x.is_empty() || x.parse::<u64>().is_ok() { Ok(()) } else { Err("须为整数") }
}

#[test]
fn a_setting_saves_on_enter_says_saved_and_says_why_it_will_not() {
    let ctx = dressed();
    let mut edit = fields::SavedEdit::default();
    let mut stored = "600".to_string();
    let mut got = None;
    let draw = |ctx: &egui::Context, events: Vec<egui::Event>, edit: &mut fields::SavedEdit, stored: &str, got: &mut Option<String>| {
        frame(ctx, events, |ui| {
            card::form(ui, |ui, f| {
                if let Some(v) = f.setting(ui, "审计间隔", edit, stored, 110.0, WORDS, check) {
                    *got = Some(v);
                }
            });
        })
    };
    let out = draw(&ctx, Vec::new(), &mut edit, &stored, &mut got);
    draw(&ctx, press_on(&out, "600"), &mut edit, &stored, &mut got);
    draw(&ctx, key(egui::Key::Enter), &mut edit, &stored, &mut got);
    assert_eq!(got, None, "nothing changed: nothing saved");
    draw(&ctx, press_on(&out, "600"), &mut edit, &stored, &mut got);
    draw(&ctx, key(egui::Key::End), &mut edit, &stored, &mut got);
    for _ in 0..3 {
        draw(&ctx, key(egui::Key::Backspace), &mut edit, &stored, &mut got);
    }
    draw(&ctx, vec![egui::Event::Text("30".into())], &mut edit, &stored, &mut got);
    assert!(edit.dirty(&stored), "typed over: {edit:?}");
    draw(&ctx, Vec::new(), &mut edit, &stored, &mut got);
    draw(&ctx, key(egui::Key::Enter), &mut edit, &stored, &mut got);
    assert_eq!(got.as_deref(), Some("30"), "Enter saved what was typed");
    stored = "30".into();
    edit.land();
    let out = draw(&ctx, Vec::new(), &mut edit, &stored, &mut got);
    assert!(words(&out).iter().any(|w| w == WORDS.saved), "saved words stand: {:?}", words(&out));
    draw(&ctx, press_on(&out, "30"), &mut edit, &stored, &mut got);
    draw(&ctx, key(egui::Key::End), &mut edit, &stored, &mut got);
    draw(&ctx, vec![egui::Event::Text("x".into())], &mut edit, &stored, &mut got);
    let out = draw(&ctx, Vec::new(), &mut edit, &stored, &mut got);
    let said = words(&out);
    assert!(said.iter().any(|w| w == "须为整数"), "why it will not save: {said:?}");
    assert!(!said.iter().any(|w| w == WORDS.saved), "the next edit withdrew the saved words");
    got = None;
    draw(&ctx, key(egui::Key::Enter), &mut edit, &stored, &mut got);
    assert_eq!(got, None, "Enter on a refused value saves nothing");
}

#[test]
fn a_length_setting_saves_its_parts_total_on_enter() {
    let ctx = dressed();
    let mut edit = fields::SavedEdit::default();
    let parts = [(99, 86_400), (23, 3_600), (59, 60)];
    let units = ["天", "小时", "分钟"];
    let mut got = None;
    let draw = |events: Vec<egui::Event>, edit: &mut fields::SavedEdit, got: &mut Option<u64>| {
        frame(&ctx, events, |ui| {
            card::form(ui, |ui, f| {
                if let Some(v) = f.setting_lasts(ui, "keep", "保留时长", edit, 3_600, &parts, &units, WORDS) {
                    *got = Some(v);
                }
            });
        })
    };
    let out = draw(Vec::new(), &mut edit, &mut got);
    draw(press_on(&out, WORDS.hint), &mut edit, &mut got);
    draw(vec![egui::Event::Text("2".into())], &mut edit, &mut got);
    draw(Vec::new(), &mut edit, &mut got);
    draw(key(egui::Key::Enter), &mut edit, &mut got);
    assert_eq!(got, Some(2 * 86_400 + 3_600), "two days and the hour stored");
}

#[test]
fn the_words_beside_a_setting_box_stand_off_it() {
    let ctx = dressed();
    let mut edit = fields::SavedEdit::default();
    edit.land();
    let mut out = None;
    for _ in 0..3 {
        out = Some(frame(&ctx, Vec::new(), |ui| {
            card::form(ui, |ui, f| {
                let _ = f.setting(ui, "审计间隔", &mut edit, "600", 110.0, WORDS, check);
            });
        }));
    }
    let out = out.expect("a frame");
    let words = out
        .shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            egui::Shape::Text(t) if t.galley.text() == WORDS.saved => Some(egui::Rect::from_min_size(t.pos, t.galley.size())),
            _ => None,
        })
        .expect("the saved words drawn");
    let field = out
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Rect(r) if (r.rect.width() - 110.0).abs() < 0.5 && (r.rect.height() - zikaron_ui::tokens::KEY_H).abs() < 0.5 => Some(r.rect),
            _ => None,
        })
        .next()
        .expect("the box drawn");
    let gap = field.left() - words.right();
    assert!(gap >= 7.0, "the words stand off the box: gap {gap}");
}
