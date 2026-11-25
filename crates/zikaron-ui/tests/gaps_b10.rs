use zikaron_ui::egui::{self, pos2, vec2, Event, PointerButton, Pos2, Rect};
use zikaron_ui::{layer, mark, menu, table};

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

fn button(p: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Default::default() }
}

fn press_at(p: Pos2) -> Vec<Event> {
    vec![Event::PointerMoved(p), button(p, true), button(p, false)]
}

fn esc_key() -> Vec<Event> {
    vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }]
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

fn rows() -> Vec<menu::Item<'static>> {
    vec![menu::row("一"), menu::row("二"), menu::row("三")]
}

#[derive(Clone, Copy, Debug)]
enum Anchor {
    MenuKey,
    PlainKey,
}

fn page(ctx: &egui::Context, which: Anchor) -> (Rect, egui::Id) {
    let mut at = (Rect::NOTHING, egui::Id::NULL);
    central(ctx, egui::CentralPanel::default(), |ui| {
        let mut child = ui.new_child(egui::UiBuilder::new().id_salt("anchor").max_rect(Rect::from_min_size(pos2(400.0, 300.0), vec2(200.0, 40.0))));
        let id = child.id().with(("zikaron-menu", "m"));
        match which {
            Anchor::MenuKey => {
                let _ = menu::menu_key(&mut child, "m", "选项", true, 160.0, &rows());
            }
            Anchor::PlainKey => {
                let _ = menu::plain_key(&mut child, "m", "最近", true, true, 160.0, &rows());
            }
        }
        at = (child.min_rect(), id);
    });
    at
}

fn opened(f: &mut Frames, which: Anchor) -> (Rect, egui::Id) {
    let mut at = (Rect::NOTHING, egui::Id::NULL);
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| at = page(ctx, which));
    }
    assert!(!menu::is_open(&f.ctx, at.1), "{which:?}: closed before the press");
    let key = at.0.center();
    f.run(0.1, press_at(key), |ctx| at = page(ctx, which));
    for _ in 0..4 {
        f.run(0.1, vec![], |ctx| at = page(ctx, which));
    }
    assert!(menu::is_open(&f.ctx, at.1), "{which:?}: a press on the key opens its menu");
    at
}

#[test]
fn pressing_the_menus_own_key_while_it_is_open_closes_it_and_it_stays_closed() {
    for which in [Anchor::MenuKey, Anchor::PlainKey] {
        for split in [false, true] {
            let mut f = Frames::new();
            let (key, id) = opened(&mut f, which);
            let p = key.center();
            if split {
                f.run(0.1, vec![Event::PointerMoved(p), button(p, true)], |ctx| {
                    page(ctx, which);
                });
                f.run(0.1, vec![button(p, false)], |ctx| {
                    page(ctx, which);
                });
            } else {
                f.run(0.1, press_at(p), |ctx| {
                    page(ctx, which);
                });
            }
            assert!(!menu::is_open(&f.ctx, id), "{which:?} split={split}: the press on its own key closed the menu");
            for i in 0..6 {
                f.run(0.1, vec![], |ctx| {
                    page(ctx, which);
                });
                assert!(!menu::is_open(&f.ctx, id), "{which:?} split={split}: still closed {} frames later", i + 1);
            }
        }
    }
}

#[test]
fn esc_closes_an_open_key_menu() {
    for which in [Anchor::MenuKey, Anchor::PlainKey] {
        let mut f = Frames::new();
        let (_, id) = opened(&mut f, which);
        f.run(0.1, esc_key(), |ctx| {
            page(ctx, which);
        });
        assert!(!menu::is_open(&f.ctx, id), "{which:?}: Esc closed the menu");
        for _ in 0..3 {
            f.run(0.1, vec![], |ctx| {
                page(ctx, which);
            });
        }
        assert!(!menu::is_open(&f.ctx, id), "{which:?}: and it stays closed");
    }
}

#[test]
fn a_pill_cut_short_shows_its_whole_words_on_hover() {
    let words = "等待节点确认收据中";
    let mut f = Frames::new();
    let mut r = Rect::NOTHING;
    let draw = |ctx: &egui::Context, r: &mut Rect| {
        central(ctx, egui::CentralPanel::default(), |ui| {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(100.0, 100.0), vec2(60.0, 30.0))));
            *r = mark::pill(&mut child, words, zikaron_ui::palette::Tone::Warn).rect;
        });
    };
    let mut out = f.run(0.1, vec![], |ctx| draw(ctx, &mut r));
    out = if texts(&out).is_empty() { f.run(0.1, vec![], |ctx| draw(ctx, &mut r)) } else { out };
    let shown: Vec<String> = texts(&out).into_iter().map(|(s, _)| s).filter(|s| s.starts_with('等')).collect();
    let cut = shown.first().cloned().expect("the pill's words drawn");
    assert!(cut.ends_with('…') && cut.chars().count() < words.chars().count(), "cut short: {cut}");
    assert!(!shown.iter().any(|s| s == words), "whole words drawn nowhere before the hover: {shown:?}");
    f.run(0.1, vec![Event::PointerMoved(r.center())], |ctx| draw(ctx, &mut r));
    for _ in 0..12 {
        out = f.run(0.1, vec![], |ctx| draw(ctx, &mut r));
    }
    let all: Vec<String> = texts(&out).into_iter().map(|(s, _)| s).collect();
    assert!(all.iter().any(|s| s == words), "the hover tip carries the whole words: {all:?}");
}

#[test]
fn a_table_tag_past_its_cap_is_elided_and_whole_on_hover() {
    let tag = "一枚远远宽过类型列上限的类型标签文字";
    let row = table::Row { cells: vec![table::Cell::Tag(tag.into()), table::Cell::Text("封面设计".into())], ..Default::default() };
    let cols = [table::TYPE, table::col("记录", table::Col::Fr(1.0))];
    let draw = |ctx: &egui::Context| {
        central(ctx, egui::CentralPanel::default(), |ui| {
            table::table(ui, "tags", &cols, false, std::slice::from_ref(&row), "无");
        });
    };
    let mut f = Frames::new();
    let mut out = f.run(0.1, vec![], draw);
    out = if texts(&out).is_empty() { f.run(0.1, vec![], draw) } else { out };
    let painted: Vec<(String, Rect)> = texts(&out).into_iter().filter(|(s, _)| s.starts_with('一')).collect();
    let (cut, at) = painted.first().cloned().expect("the tag drawn");
    assert!(cut.ends_with('…') && cut.chars().count() < tag.chars().count(), "elided: {cut}");
    assert!(!painted.iter().any(|(s, _)| s == tag), "the whole tag drawn nowhere before the hover");
    f.run(0.1, vec![Event::PointerMoved(at.center())], draw);
    for _ in 0..12 {
        out = f.run(0.1, vec![], draw);
    }
    let all: Vec<String> = texts(&out).into_iter().map(|(s, _)| s).collect();
    assert!(all.iter().any(|s| s == tag), "hovered, the whole tag shows: {all:?}");
}
