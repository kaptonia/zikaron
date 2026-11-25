use std::path::PathBuf;
use zikaron_ui::egui::{self, pos2, vec2, Event, PointerButton, Pos2, Rect};
use zikaron_ui::{button, datepick, full, input, layer, mark, menu, pick, sheet, toast, tokens, width};

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

fn press_at(p: Pos2) -> Vec<Event> {
    vec![
        Event::PointerMoved(p),
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Default::default() },
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Default::default() },
    ]
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

fn page_key(ctx: &egui::Context) -> bool {
    let mut clicked = false;
    central(ctx, egui::CentralPanel::default(), |ui| {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(400.0, 300.0), vec2(200.0, 40.0))));
        clicked = button::key(&mut child, "底下的键", button::Role::Secondary, true).clicked();
    });
    clicked
}

const ANCHOR: Rect = Rect { min: Pos2 { x: 40.0, y: 40.0 }, max: Pos2 { x: 140.0, y: 74.0 } };

fn rows() -> Vec<menu::Item<'static>> {
    vec![menu::row("一"), menu::row("二"), menu::row("三")]
}

#[test]
fn a_press_outside_a_menu_closes_it_and_reaches_nothing_under_it() {
    let id = egui::Id::new("u1-menu");
    let on_key = pos2(420.0, 317.0);
    let mut f = Frames::new();
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            page_key(ctx);
        });
    }
    let mut got = false;
    f.run(0.1, press_at(on_key), |ctx| got = page_key(ctx));
    assert!(got, "with nothing open the press presses the key");
    let mut f = Frames::new();
    menu::set(&f.ctx, id, true);
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            page_key(ctx);
            menu::show(ctx, id, ANCHOR, true, 160.0, &rows());
        });
    }
    assert!(menu::is_open(&f.ctx, id));
    let mut got = false;
    f.run(0.1, press_at(on_key), |ctx| {
        got = page_key(ctx);
        menu::show(ctx, id, ANCHOR, true, 160.0, &rows());
    });
    assert!(!got, "the press that closes the menu reaches nothing under it");
    assert!(!menu::is_open(&f.ctx, id), "the press outside closed the menu");
    menu::set(&f.ctx, id, true);
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            menu::show(ctx, id, ANCHOR, true, 160.0, &rows());
        });
    }
    let mut picked = Some(9);
    f.run(0.1, press_at(pos2(ANCHOR.left() + 40.0, ANCHOR.bottom() + 6.0 + 2.0)), |ctx| picked = menu::show(ctx, id, ANCHOR, true, 160.0, &rows()));
    assert!(picked.is_none() && menu::is_open(&f.ctx, id), "a press on the card itself neither picks nor closes");
    menu::set(&f.ctx, id, true);
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            menu::show(ctx, id, ANCHOR, true, 160.0, &rows());
        });
    }
    let mut picked = None;
    let first_row = pos2(ANCHOR.left() + 40.0, ANCHOR.bottom() + 6.0 + 5.0 + 17.0);
    f.run(0.1, press_at(first_row), |ctx| picked = menu::show(ctx, id, ANCHOR, true, 160.0, &rows()));
    assert_eq!(picked, Some(0), "a press on a row picks it");
}

#[test]
fn a_press_outside_a_pick_list_reaches_nothing_under_it() {
    let id = egui::Id::new("u1-pick");
    let spec = pick::Spec { hint: "找", empty: "无", w: 300.0, dates: None };
    let keep: pick::Keep = &|_, _, _, _| true;
    let mut f = Frames::new();
    menu::set(&f.ctx, id, true);
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            page_key(ctx);
            pick::show(ctx, id, ANCHOR, true, &spec, &rows(), keep);
        });
    }
    let mut got = false;
    f.run(0.1, press_at(pos2(420.0, 317.0)), |ctx| {
        got = page_key(ctx);
        pick::show(ctx, id, ANCHOR, true, &spec, &rows(), keep);
    });
    assert!(!got && !menu::is_open(&f.ctx, id), "closed, and the key under it untouched");
}

#[test]
fn every_overlay_area_is_made_in_one_place_without_egui_fade_in() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut found = Vec::new();
    for dir in [root.join("src"), root.join("../app/src"), root.join("../app/src/window")] {
        for e in std::fs::read_dir(&dir).expect("source directory").flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            for (n, line) in text.lines().enumerate() {
                let code = line.split("//").next().unwrap_or("");
                if code.contains("egui::Area::new(") || code.contains("Area::new(") && !code.contains("layer::area") {
                    found.push(format!("{}:{}", p.display(), n + 1));
                }
            }
        }
    }
    assert_eq!(found.len(), 1, "one place opens areas: {found:?}");
    assert!(found[0].contains("layer.rs"), "and it is the layer module: {found:?}");
    let layer_rs = std::fs::read_to_string(root.join("src/layer.rs")).unwrap();
    let body = layer_rs.split("pub fn area(").nth(1).and_then(|s| s.split('}').next()).unwrap_or("");
    assert!(body.contains(".fade_in(false)"), "that one place turns egui's fade-in off: {body}");
}

fn menu_transform(ctx: &egui::Context, id: egui::Id) -> egui::emath::TSTransform {
    ctx.layer_transform_to_global(egui::LayerId::new(egui::Order::Foreground, id.with("menu"))).unwrap_or_default()
}

#[test]
fn a_menu_leaves_back_up_the_way_it_came_down() {
    let id = egui::Id::new("u6-leave");
    let mut f = Frames::new();
    menu::set(&f.ctx, id, true);
    for _ in 0..4 {
        f.run(0.1, vec![], |ctx| {
            menu::show(ctx, id, ANCHOR, true, 160.0, &rows());
        });
    }
    menu::set(&f.ctx, id, false);
    f.run(0.016, vec![], |_| {});
    f.run(f64::from(tokens::FAST) / 2.0, vec![], |_| {});
    let ghost = egui::LayerId::new(egui::Order::Foreground, id.with("menu").with("leaving"));
    let tr = f.ctx.layer_transform_to_global(ghost).expect("the leaving copy is drawn");
    let alpha = (tr.scaling - 0.97) / 0.03;
    assert!(alpha > 0.05 && alpha < 0.95, "halfway out: {alpha}");
    let origin_y = ANCHOR.bottom() + 6.0;
    let want = origin_y * (1.0 - tr.scaling) - 4.0 * (1.0 - alpha);
    assert!((tr.translation.y - want).abs() < 0.05, "up the 4 it came down, by the same amount: {} vs {want}", tr.translation.y);
}

#[test]
fn a_menu_opened_again_halfway_out_comes_back_from_halfway() {
    let id = egui::Id::new("u6-resume");
    let mut f = Frames::new();
    menu::set(&f.ctx, id, true);
    for _ in 0..4 {
        f.run(0.1, vec![], |ctx| {
            menu::show(ctx, id, ANCHOR, true, 160.0, &rows());
        });
    }
    menu::set(&f.ctx, id, false);
    f.run(0.016, vec![], |_| {});
    f.run(f64::from(tokens::FAST) / 2.0, vec![], |_| {});
    let ghost = egui::LayerId::new(egui::Order::Foreground, id.with("menu").with("leaving"));
    let leaving = (f.ctx.layer_transform_to_global(ghost).expect("leaving").scaling - 0.97) / 0.03;
    menu::set(&f.ctx, id, true);
    f.run(0.0, vec![], |ctx| {
        menu::show(ctx, id, ANCHOR, true, 160.0, &rows());
    });
    let back = (menu_transform(&f.ctx, id).scaling - 0.97) / 0.03;
    assert!(leaving > 0.2, "it was well on screen when opened again: {leaving}");
    assert!((back - leaving).abs() < 0.03, "the entrance takes up where the leaving stood: {back} vs {leaving}");
    let fresh = egui::Id::new("u6-fresh");
    menu::set(&f.ctx, fresh, true);
    f.run(0.0, vec![], |ctx| {
        menu::show(ctx, fresh, ANCHOR, true, 160.0, &rows());
    });
    assert!((menu_transform(&f.ctx, fresh).scaling - 0.97).abs() < 0.005, "a fresh opening starts at the entrance's start");
}

#[test]
fn a_card_opened_inside_an_overlay_stays_above_it() {
    let mut f = Frames::new();
    let outer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("u6-outer"));
    let inner = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("u6-inner"));
    let draw = |ctx: &egui::Context, raise_outer: bool| {
        layer::area(outer.id, egui::Order::Foreground).fixed_pos(pos2(100.0, 100.0)).show(ctx, |ui| {
            ui.allocate_rect(Rect::from_min_size(pos2(100.0, 100.0), vec2(300.0, 300.0)), egui::Sense::click());
        });
        layer::place(ctx, inner, Some(outer));
        layer::over(ctx, inner).show(ctx, |ui| {
            layer::under(ui, layer::Under::Guard);
            ui.allocate_rect(Rect::from_min_size(pos2(150.0, 150.0), vec2(100.0, 100.0)), egui::Sense::click());
        });
        if raise_outer {
            ctx.move_to_top(outer);
        }
    };
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| draw(ctx, false));
    }
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| draw(ctx, true));
    }
    assert_eq!(f.ctx.layer_id_at(pos2(200.0, 200.0)), Some(inner), "where both cards lie, the inner one is on top");
    assert_eq!(f.ctx.layer_id_at(pos2(350.0, 350.0)), Some(inner), "over the outer card, the inner card's guard");
}

#[test]
fn a_menu_in_a_sheet_closed_by_a_press_outside_works_when_opened_again() {
    let mut f = Frames::new();
    let id = egui::Id::new("u6-in-sheet");
    let anchor = std::cell::Cell::new(None);
    let draw = |ctx: &egui::Context, picked: &mut Option<usize>| {
        let mut unit = ();
        sheet::show(
            ctx,
            sheet::Spec::new("u6-sheet", tokens::SHEET_W),
            &mut unit,
            |ui, _| {
                let r = ui.allocate_exact_size(vec2(120.0, 34.0), egui::Sense::hover()).0;
                anchor.set(Some(r));
                *picked = menu::show_from(ui.ctx(), Some(ui.layer_id()), id, r, ui.clip_rect(), true, 160.0, &rows());
            },
            |_, _| (),
        );
    };
    let mut picked = None;
    menu::set(&f.ctx, id, true);
    for _ in 0..4 {
        f.run(0.1, vec![], |ctx| draw(ctx, &mut picked));
    }
    let a = anchor.get().expect("drawn");
    let outside = pos2(a.left() + 4.0, a.top() - 8.0);
    f.run(0.1, press_at(outside), |ctx| draw(ctx, &mut picked));
    assert!(!menu::is_open(&f.ctx, id), "the press outside closed the menu");
    for _ in 0..4 {
        f.run(0.1, vec![], |ctx| draw(ctx, &mut picked));
    }
    menu::set(&f.ctx, id, true);
    for _ in 0..4 {
        f.run(0.1, vec![], |ctx| draw(ctx, &mut picked));
    }
    let row2 = pos2(a.left() + 40.0, a.bottom() + 6.0 + 6.0 + 34.0 + 17.0);
    let mut got = None;
    f.run(0.1, press_at(row2), |ctx| {
        draw(ctx, &mut picked);
        got = picked;
    });
    assert_eq!(got, Some(1), "the menu opened again picks its row");
}

#[test]
fn the_overlay_opened_last_lies_on_top() {
    let mut f = Frames::new();
    let first = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("u6-first"));
    let second = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("u6-second"));
    let card = |ctx: &egui::Context, l: egui::LayerId, at: Pos2| {
        layer::place(ctx, l, None);
        layer::area(l.id, egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
            ui.allocate_rect(Rect::from_min_size(at, vec2(200.0, 200.0)), egui::Sense::click());
        });
    };
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| card(ctx, first, pos2(100.0, 100.0)));
    }
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            card(ctx, second, pos2(200.0, 200.0));
            card(ctx, first, pos2(100.0, 100.0));
        });
    }
    assert_eq!(f.ctx.layer_id_at(pos2(250.0, 250.0)), Some(second), "the one opened last is on top");
    f.run(0.1, press_at(pos2(150.0, 150.0)), |ctx| {
        card(ctx, second, pos2(200.0, 200.0));
        card(ctx, first, pos2(100.0, 100.0));
    });
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            card(ctx, second, pos2(200.0, 200.0));
            card(ctx, first, pos2(100.0, 100.0));
        });
    }
    assert_eq!(f.ctx.layer_id_at(pos2(250.0, 250.0)), Some(first), "the one pressed is on top and stays");
}

#[test]
fn one_esc_closes_the_calendar_and_not_the_sheet_under_it() {
    let mut f = Frames::new();
    let words = datepick::Words { weekdays: ["一", "二", "三", "四", "五", "六", "日"], month: |y, m| format!("{y}年{m}月"), year: |y| format!("{y}年"), month_alone: |m| format!("{m}月"), today: "今天", clear: "清除" };
    let mut date_id = None;
    let mut value = String::new();
    let mut sheet_esc = false;
    let draw = |ctx: &egui::Context, date_id: &mut Option<egui::Id>, value: &mut String, open_date: bool| -> bool {
        let out = sheet::show(
            ctx,
            sheet::Spec::new("u7-sheet", tokens::SHEET_W),
            value,
            |ui, v| {
                let id = ui.id().with(("zikaron-date", "d"));
                *date_id = Some(id);
                if open_date && !menu::is_open(ui.ctx(), id) {
                    menu::set(ui.ctx(), id, true);
                }
                datepick::field(ui, "d", v, "开始", 140.0, (2026, 10, 6), &words);
            },
            |_, _| (),
        );
        out.esc
    };
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            draw(ctx, &mut date_id, &mut value, false);
        });
    }
    f.run(0.1, vec![], |ctx| {
        draw(ctx, &mut date_id, &mut value, true);
    });
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            draw(ctx, &mut date_id, &mut value, false);
        });
    }
    let did = date_id.expect("the date field drew");
    assert!(menu::is_open(&f.ctx, did), "the calendar is open over the sheet");
    f.run(0.1, esc_key(), |ctx| sheet_esc = draw(ctx, &mut date_id, &mut value, false));
    assert!(!menu::is_open(&f.ctx, did), "Esc closed the calendar");
    assert!(!sheet_esc, "and not the sheet under it");
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            draw(ctx, &mut date_id, &mut value, false);
        });
    }
    f.run(0.1, esc_key(), |ctx| sheet_esc = draw(ctx, &mut date_id, &mut value, false));
    assert!(sheet_esc, "the next Esc is the sheet's");
}

#[test]
fn esc_goes_to_the_overlay_opened_last_whatever_order_they_ask_in() {
    let mut f = Frames::new();
    let (a, b) = (egui::Id::new("u7-a"), egui::Id::new("u7-b"));
    f.run(0.1, vec![], |ctx| {
        layer::esc(ctx, a);
    });
    f.run(0.1, vec![], |ctx| {
        layer::esc(ctx, a);
    });
    f.run(0.1, vec![], |ctx| {
        layer::esc(ctx, b);
        layer::esc(ctx, a);
    });
    let mut got = (false, false);
    f.run(0.1, esc_key(), |ctx| got = (layer::esc(ctx, b), layer::esc(ctx, a)));
    assert_eq!(got, (true, false), "b was opened last: only b takes it (asked first)");
    let mut got = (false, false);
    f.run(0.1, vec![], |ctx| {
        layer::esc(ctx, a);
        layer::esc(ctx, b);
    });
    f.run(0.1, esc_key(), |ctx| got = (layer::esc(ctx, a), layer::esc(ctx, b)));
    assert_eq!(got, (false, true), "and asked last");
    f.run(0.1, vec![], |ctx| {
        layer::esc(ctx, a);
    });
    f.run(0.1, vec![], |ctx| {
        layer::esc(ctx, a);
    });
    let mut got_a = false;
    f.run(0.1, esc_key(), |ctx| got_a = layer::esc(ctx, a));
    assert!(got_a, "with b gone, a takes Esc");
    let mut none = true;
    f.run(0.1, vec![], |ctx| none = !layer::esc(ctx, a));
    assert!(none);
}

#[test]
fn a_hint_too_long_for_its_field_ends_in_an_ellipsis_inside_the_frame() {
    let mut f = Frames::new();
    let long = "可选,如:作品名,只存本机,不会写进账本也不会离开这台机器";
    for (w, hint, cut) in [(180.0, long, true), (600.0, long, false), (180.0, "短", false)] {
        let mut text = String::new();
        let mut rect = Rect::NOTHING;
        let mut out = f.run(0.1, vec![], |ctx| {
            central(ctx, egui::CentralPanel::default(), |ui| rect = input::field(ui, &mut text, hint, w, input::Look::default()).rect);
        });
        out = if out.shapes.is_empty() { f.run(0.1, vec![], |_| {}) } else { out };
        let shown: Vec<(String, Rect)> = texts(&out).into_iter().filter(|(s, _)| s.starts_with(hint.chars().next().unwrap())).collect();
        let (s, r) = shown.first().cloned().unwrap_or_else(|| panic!("hint drawn at width {w}"));
        if cut {
            assert!(s.ends_with('…') && s.chars().count() < hint.chars().count(), "cut with an ellipsis: {s}");
        } else {
            assert_eq!(s, hint, "fits: shown whole");
        }
        assert!(r.right() <= rect.right() + 0.5, "inside the field's frame: {r:?} in {rect:?}");
    }
}

#[test]
fn a_pill_in_too_little_room_ends_in_an_ellipsis() {
    let mut f = Frames::new();
    let words = "等待节点确认收据中";
    let natural = {
        let mut w = 0.0;
        f.run(0.1, vec![], |ctx| {
            central(ctx, egui::CentralPanel::default(), |ui| w = mark::pill_w(ui, words, false));
        });
        w
    };
    for (x, room) in [(100.0, 60.0_f32), (100.4, natural), (100.6, natural), (100.0, 400.0)] {
        let mut r = Rect::NOTHING;
        let out = f.run(0.1, vec![], |ctx| {
            central(ctx, egui::CentralPanel::default(), |ui| {
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(x, 100.0), vec2(room, 30.0))));
                r = mark::pill(&mut child, words, zikaron_ui::palette::Tone::Warn).rect;
            });
        });
        let shown: Vec<String> = texts(&out).into_iter().map(|(s, _)| s).filter(|s| s.starts_with('等')).collect();
        let s = shown.first().cloned().expect("the pill's words drawn");
        if room < natural - 1.0 {
            assert!(r.width() <= room + 0.5, "never wider than its room: {} > {room}", r.width());
            assert!(s.ends_with('…'), "its words cut with an ellipsis: {s}");
        } else {
            assert_eq!(s, words, "with room (or within a point of it), whole");
        }
    }
}

#[test]
fn a_detail_heads_pills_are_drawn_whole() {
    use zikaron_ui::card;
    use zikaron_ui::palette::Tone;
    let mut f = Frames::new();
    let pills = [
        card::HeroPill { words: "等待节点确认收据中".into(), tone: Tone::Blue, live: true },
        card::HeroPill { words: "争议中".into(), tone: Tone::Warn, live: false },
    ];
    let mut head = Rect::NOTHING;
    let mut out = f.run(0.1, vec![], |_| {});
    for _ in 0..3 {
        out = f.run(0.1, vec![], |ctx| {
            central(ctx, egui::CentralPanel::default(), |ui| head = card::hero(ui, "授权一", "#3 · 授权", false, &pills));
        });
    }
    let shown = texts(&out);
    let at = |w: &str| shown.iter().find(|(s, _)| s == w).map(|(_, r)| *r);
    let title = at("授权一").expect("the title drawn");
    let first = at("等待节点确认收据中").unwrap_or_else(|| panic!("the first pill's words whole: {shown:?}"));
    let second = at("争议中").unwrap_or_else(|| panic!("the second pill's words whole: {shown:?}"));
    assert!(first.left() > second.right(), "the first pill rightmost: {first:?} {second:?}");
    assert!(second.left() > title.right(), "both right of the title: {second:?} {title:?}");
    assert!(first.right() <= head.right() + 0.5, "inside the head: {first:?} {head:?}");
    assert!((first.center().y - title.center().y).abs() < 2.0, "on the title's middle: {first:?} {title:?}");
}

#[test]
fn a_file_name_is_read_with_either_separator() {
    for (path, want) in [
        ("x.json", "x.json"),
        ("/home/a/x.json", "x.json"),
        ("C:\\Users\\a\\x.json", "x.json"),
        ("/home/a/folder/", "folder"),
        ("C:\\Users\\a\\folder\\", "folder"),
        ("C:\\Users\\a/mixed\\x.json", "x.json"),
        ("C:/Users\\a/folder\\/", "folder"),
        ("\\\\server\\share\\", "share"),
        ("C:\\", "C:"),
        ("/", ""),
        ("\\", ""),
        ("", ""),
        ("  /home/a/x y.json  ", "x y.json"),
    ] {
        assert_eq!(width::file_name(path), want, "{path:?}");
    }
}

#[test]
fn a_pill_in_a_table_cell_its_own_width_stays_whole() {
    use zikaron_ui::table;
    let mut f = Frames::new();
    for words in ["已撤销", "未知", "有效", "Revoked", "Pending"] {
        let row = table::Row { cells: vec![table::Cell::Text("封面设计".into()), table::Cell::Pill(words.into(), zikaron_ui::palette::Tone::Grey)], click: true, ..Default::default() };
        let mut out = f.run(0.1, vec![], |ctx| {
            central(ctx, egui::CentralPanel::default(), |ui| {
                table::table(ui, "pills", &[table::col("记录", table::Col::Fr(1.0)), table::col("状态", table::Col::Px(84.0))], true, std::slice::from_ref(&row), "无");
            });
        });
        out = if texts(&out).is_empty() { f.run(0.1, vec![], |_| {}) } else { out };
        let shown: Vec<String> = texts(&out).into_iter().map(|(s, _)| s).collect();
        assert!(shown.iter().any(|s| s == words), "{words} whole in its cell: {shown:?}");
    }
}

#[test]
fn every_overlay_is_placed_in_one_place() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut stray = Vec::new();
    let mut uneven = Vec::new();
    for dir in [root.join("src"), root.join("../app/src"), root.join("../app/src/window")] {
        for e in std::fs::read_dir(&dir).expect("source directory").flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("rs") || p.ends_with("layer.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&p).unwrap_or_default();
            let code: String = text.lines().map(|l| l.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
            for word in ["move_to_top(", "set_sublayer(", "on_hover_text(", "on_hover_ui(", "show_tooltip", "Tooltip::"] {
                if code.contains(word) {
                    stray.push(format!("{} {word}", p.display()));
                }
            }
            let areas = code.matches("layer::area(").count() + code.matches("layer::over(").count();
            let placed = code.matches("layer::place(").count();
            if areas != placed {
                uneven.push(format!("{} areas {areas} placed {placed}", p.display()));
            }
        }
    }
    assert!(stray.is_empty(), "raised, nested or tipped outside the layer module: {stray:?}");
    assert!(uneven.is_empty(), "an overlay area opened without being placed: {uneven:?}");
}

#[test]
fn a_press_on_a_sheets_scrim_reaches_nothing_under_it() {
    let on_key = pos2(410.0, 317.0);
    let mut f = Frames::new();
    let mut got = false;
    f.run(0.1, vec![], |ctx| got |= page_key(ctx));
    f.run(0.1, press_at(on_key), |ctx| got |= page_key(ctx));
    assert!(got, "with no sheet the press lands on the key");
    let mut f = Frames::new();
    let mut got = false;
    let mut drawn = 0;
    let mut draw = |ctx: &egui::Context, got: &mut bool| {
        *got |= page_key(ctx);
        let mut n = 0;
        sheet::show(ctx, sheet::Spec::new("t-sheet", 300.0), &mut n, |ui, _| ui.label("卡里"), |_, _| ());
        drawn += 1;
    };
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| draw(ctx, &mut got));
    }
    let card = egui::LayerId::new(egui::Order::Foreground, egui::Id::new(("zikaron-sheet", "t-sheet")).with("card"));
    assert!(on_key.x < SCREEN.x / 2.0 - 150.0, "the press lands outside the card (300 wide, centred)");
    assert_eq!(f.ctx.layer_id_at(on_key), Some(card), "the scrim spans the window in the sheet's layer");
    f.run(0.1, press_at(on_key), |ctx| draw(ctx, &mut got));
    f.run(0.1, vec![], |ctx| draw(ctx, &mut got));
    assert!(!got, "the key under the scrim is untouched");
    assert_eq!(f.ctx.layer_id_at(pos2(590.0, 400.0)), Some(card), "the card still lies above its scrim after the press");
}

#[test]
fn a_press_on_a_cover_reaches_nothing_under_it() {
    let on_key = pos2(410.0, 317.0);
    let mut f = Frames::new();
    let mut got = false;
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            got |= page_key(ctx);
            full::cover(ctx, "t-cover", 0.0, |ui, _| ui.label("盖着"));
        });
    }
    f.run(0.1, press_at(on_key), |ctx| {
        got |= page_key(ctx);
        full::cover(ctx, "t-cover", 0.0, |ui, _| ui.label("盖着"));
    });
    assert!(!got, "the key under the cover is untouched");
}

#[test]
fn a_toast_takes_no_press_beside_it_and_lies_above_a_sheet() {
    let on_key = pos2(410.0, 317.0);
    let mut f = Frames::new();
    let mut toasts = toast::Toasts::new();
    toasts.say("一句话", toast::Tone::Note, 0.0);
    let mut got = false;
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            got |= page_key(ctx);
            toasts.draw(ctx, 0.0);
        });
    }
    f.run(0.1, press_at(on_key), |ctx| {
        got |= page_key(ctx);
        toasts.draw(ctx, 0.0);
    });
    assert!(got, "the press beside the toast lands on the key");
    for _ in 0..3 {
        f.run(0.1, vec![], |ctx| {
            let mut n = 0;
            sheet::show(ctx, sheet::Spec::new("t-sheet-under-toast", 300.0), &mut n, |ui, _| ui.label("卡里"), |_, _| ());
            toasts.draw(ctx, 0.0);
        });
    }
    let on_toast = pos2(SCREEN.x / 2.0, 14.0 + 20.0);
    assert_eq!(f.ctx.layer_id_at(on_toast).map(|l| l.order), Some(egui::Order::Tooltip), "the toast lies above the sheet's scrim");
}

#[test]
fn a_hover_tip_does_not_stay_over_a_press() {
    let mut f = Frames::new();
    let on_key = pos2(420.0, 317.0);
    let mut under = false;
    let draw = |ctx: &egui::Context, under: &mut bool| {
        central(ctx, egui::CentralPanel::default(), |ui| {
            let k = ui.interact(Rect::from_min_size(pos2(400.0, 300.0), vec2(200.0, 40.0)), egui::Id::new("tipped"), egui::Sense::hover());
            let _ = layer::tip(k, "整句提示话");
            *under |= ui.interact(Rect::from_min_max(pos2(0.0, 342.0), pos2(SCREEN.x, SCREEN.y)), egui::Id::new("under-tip"), egui::Sense::click()).clicked();
        });
    };
    let mut out = f.run(0.1, vec![Event::PointerMoved(on_key)], |ctx| draw(ctx, &mut under));
    for _ in 0..12 {
        out = f.run(0.1, vec![], |ctx| draw(ctx, &mut under));
    }
    let tip = texts(&out).into_iter().find(|(t, _)| t == "整句提示话").map(|(_, r)| r).expect("the tip shows after a rest");
    assert!(tip.center().y > 342.0, "the tip lies over the clickable ground: {tip:?}");
    f.run(0.1, vec![Event::PointerMoved(tip.center())], |ctx| draw(ctx, &mut under));
    out = f.run(0.1, vec![], |ctx| draw(ctx, &mut under));
    assert!(!texts(&out).iter().any(|(t, _)| t == "整句提示话"), "the tip is gone once the pointer left its widget");
    f.run(0.1, press_at(tip.center()), |ctx| draw(ctx, &mut under));
    assert!(under, "the press where the tip was reaches what lies under it");
}

#[test]
fn a_menu_with_no_room_under_its_anchor_opens_above_it() {
    let tops = |anchor: Rect| -> Vec<f32> {
        let mut f = Frames::new();
        let id = egui::Id::new(("u-flip", anchor.top() as i32));
        menu::set(&f.ctx, id, true);
        let mut out = f.run(0.1, vec![], |ctx| {
            menu::show(ctx, id, anchor, true, 160.0, &rows());
        });
        for _ in 0..4 {
            out = f.run(0.1, vec![], |ctx| {
                menu::show(ctx, id, anchor, true, 160.0, &rows());
            });
        }
        texts(&out).into_iter().filter(|(t, _)| ["一", "二", "三"].contains(&t.as_str())).map(|(_, r)| r.top()).collect()
    };
    let low = Rect::from_min_size(pos2(40.0, SCREEN.y - 50.0), vec2(100.0, 34.0));
    let up = tops(low);
    assert_eq!(up.len(), 3, "the rows are drawn: {up:?}");
    assert!(up.iter().all(|y| *y < low.top() && *y > 8.0), "above the anchor, inside the window: {up:?}");
    let down = tops(ANCHOR);
    assert!(down.iter().all(|y| *y > ANCHOR.bottom()), "with room, under the anchor: {down:?}");
}

#[test]
fn a_pick_list_and_the_date_card_with_no_room_under_open_above() {
    let low = Rect::from_min_size(pos2(40.0, SCREEN.y - 50.0), vec2(100.0, 34.0));
    let spec = pick::Spec { hint: "找", empty: "无", w: 300.0, dates: None };
    let keep: pick::Keep = &|_, _, _, _| true;
    let mut f = Frames::new();
    let id = egui::Id::new("u-flip-pick");
    menu::set(&f.ctx, id, true);
    let mut out = f.run(0.1, vec![], |ctx| {
        pick::show(ctx, id, low, true, &spec, &rows(), keep);
    });
    for _ in 0..4 {
        out = f.run(0.1, vec![], |ctx| {
            pick::show(ctx, id, low, true, &spec, &rows(), keep);
        });
    }
    let ys: Vec<f32> = texts(&out).into_iter().filter(|(t, _)| ["一", "二", "三"].contains(&t.as_str())).map(|(_, r)| r.top()).collect();
    assert_eq!(ys.len(), 3, "the pick rows are drawn: {ys:?}");
    assert!(ys.iter().all(|y| *y < low.top() && *y > 8.0), "the pick list above its anchor, inside the window: {ys:?}");
    let words = datepick::Words { weekdays: ["一", "二", "三", "四", "五", "六", "日"], month: |y, m| format!("{y}年{m}月"), year: |y| format!("{y}年"), month_alone: |m| format!("{m}月"), today: "今天", clear: "清除" };
    let mut f = Frames::new();
    let mut value = String::new();
    let mut field = Rect::NOTHING;
    let mut draw = |ctx: &egui::Context, open: bool, field: &mut Rect| {
        central(ctx, egui::CentralPanel::default(), |ui| {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(low));
            let id = child.id().with(("zikaron-date", "d"));
            if open && !menu::is_open(ctx, id) {
                menu::set(ctx, id, true);
            }
            datepick::field(&mut child, "d", &mut value, "开始", 140.0, (2026, 10, 6), &words);
            *field = child.min_rect();
        });
    };
    let mut out = f.run(0.1, vec![], |ctx| draw(ctx, true, &mut field));
    for _ in 0..4 {
        out = f.run(0.1, vec![], |ctx| draw(ctx, false, &mut field));
    }
    let month = texts(&out).into_iter().find(|(t, _)| t == "2026年10月").map(|(_, r)| r).expect("the calendar is open");
    assert!(month.bottom() < field.top() && month.top() > 8.0, "the calendar above its field, inside the window: {month:?} {field:?}");
}

#[test]
fn a_card_with_room_on_neither_side_stays_inside_the_window() {
    let mut f = Frames::new();
    let mut got = Vec::new();
    f.run(0.1, vec![], |ctx| {
        for top in [300.0, 500.0] {
            let anchor = Rect::from_min_size(pos2(40.0, top), vec2(100.0, 34.0));
            got.push((top, layer::drop_card(ctx, anchor, ctx.content_rect(), vec2(200.0, 600.0), true).rect));
        }
    });
    for (top, r) in got {
        assert!(r.top() >= 8.0 - 0.01 && r.bottom() <= SCREEN.y - 8.0 + 0.01, "inside the window from {top}: {r:?}");
    }
}

#[test]
fn a_key_value_tables_widest_key_stands_whole() {
    let mut f = Frames::new();
    let draw = |ctx: &egui::Context| {
        central(ctx, egui::CentralPanel::default(), |ui| {
            ui.set_width(520.0);
            zikaron_ui::kv::kv(ui, &[("Network", zikaron_ui::kv::Val::text("Base")), ("Ledger description", zikaron_ui::kv::Val::text("值"))]);
        });
    };
    f.run(0.1, vec![], draw);
    let out = f.run(0.1, vec![], draw);
    let rows: Vec<(String, usize, Rect)> = out
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Text(t) => Some((t.galley.text().to_string(), t.galley.rows.len(), t.galley.rect.translate(t.pos.to_vec2()))),
            _ => None,
        })
        .collect();
    let key = rows.iter().find(|(t, _, _)| t == "Ledger description").expect("the key is drawn");
    let val = rows.iter().find(|(t, _, _)| t == "值").expect("the value is drawn");
    assert_eq!(key.1, 1, "the key on one line: {rows:?}");
    assert!(val.2.left() >= key.2.right() + tokens::KV_COL_GAP - 0.5, "the value right of the whole key: {rows:?}");
}

#[test]
fn a_leaving_layer_fades_every_colour_but_the_placeholder() {
    let p = egui::Color32::PLACEHOLDER;
    for alpha in [0.0, 0.25, 0.5, 1.0] {
        assert_eq!(layer::leaving_colour(p, alpha), p, "placeholder at {alpha}");
    }
    let c = zikaron_ui::palette::c(zikaron_ui::palette::C::Surface);
    assert_eq!(layer::leaving_colour(c, 0.5), c.gamma_multiply(0.5));
    assert_eq!(layer::leaving_colour(c, 1.0), c);
    assert_eq!(layer::leaving_colour(c, 0.0), egui::Color32::TRANSPARENT);
    let id = egui::Id::new("b13-leave");
    let mut f = Frames::new();
    menu::set(&f.ctx, id, true);
    for _ in 0..4 {
        f.run(0.1, vec![], |ctx| {
            menu::show(ctx, id, ANCHOR, true, 160.0, &rows());
        });
    }
    menu::set(&f.ctx, id, false);
    f.run(0.016, vec![], |_| {});
    let out = f.run(f64::from(tokens::FAST) / 2.0, vec![], |_| {});
    let meshes = f.ctx.tessellate(out.shapes, out.pixels_per_point);
    let mut vertices = 0usize;
    for m in &meshes {
        if let egui::epaint::Primitive::Mesh(mesh) = &m.primitive {
            vertices += mesh.vertices.len();
            assert!(mesh.vertices.iter().all(|v| v.color != p), "no placeholder vertex is left");
        }
    }
    assert!(vertices > 0, "the leaving menu was painted");
}

#[test]
fn a_banner_wraps_a_long_sentence_and_never_cuts_it() {
    use zikaron_ui::states;
    let long = "This data's writer mark cannot be read by this version; read-only here. To write from this machine, press Write from this machine";
    let mut f = Frames::new();
    let mut drawn: Vec<(String, usize, egui::Rect)> = Vec::new();
    let out = f.run(0.1, vec![], |ctx| {
        central(ctx, egui::CentralPanel::default(), |ui| {
            ui.allocate_ui(egui::vec2(360.0, 400.0), |ui| {
                states::banner(ui, states::Banner::Warn, long, |ui| ui.button("Write"));
            });
        });
    });
    fn walk(s: &egui::Shape, out: &mut Vec<(String, usize, egui::Rect)>) {
        match s {
            egui::Shape::Text(t) => out.push((t.galley.job.text.clone(), t.galley.rows.len(), t.visual_bounding_rect())),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    for c in &out.shapes {
        walk(&c.shape, &mut drawn);
    }
    let (text, rows, rect) = drawn.iter().find(|(t, _, _)| t.starts_with("This data")).cloned().expect("the sentence is drawn");
    assert_eq!(text, long, "the whole sentence, nothing cut");
    assert!(!text.contains('\u{2026}'), "no ellipsis");
    assert!(rows > 1, "wrapped onto more than one row: {rows} {rect:?}");
    assert!(rect.max.x <= 360.0 + 16.0 + 8.0, "within the banner's width: {rect:?}");
}

#[test]
fn a_floating_card_stays_inside_its_anchors_room() {
    let lefts = |content_left: f32| -> Vec<f32> {
        let mut f = Frames::new();
        let content = Rect::from_min_max(pos2(content_left, 0.0), pos2(SCREEN.x, SCREEN.y));
        let draw = |ctx: &egui::Context, out_key: &mut Rect| {
            central(ctx, egui::CentralPanel::default().frame(egui::Frame::NONE), |ui| {
                let mut room = ui.new_child(egui::UiBuilder::new().max_rect(content));
                room.set_clip_rect(content);
                room.add_space(40.0);
                room.horizontal(|ui| {
                    ui.add_space(4.0);
                    let _ = menu::menu_key(ui, "wide", "选", false, 420.0, &rows());
                    *out_key = ui.min_rect();
                });
            });
        };
        let mut key = Rect::NOTHING;
        let _ = f.run(0.1, vec![], |ctx| draw(ctx, &mut key));
        let at = pos2(key.left() + 12.0, key.center().y);
        let _ = f.run(0.1, press_at(at), |ctx| draw(ctx, &mut key));
        let mut out = f.run(0.1, vec![], |ctx| draw(ctx, &mut key));
        for _ in 0..4 {
            out = f.run(0.1, vec![], |ctx| draw(ctx, &mut key));
        }
        texts(&out).into_iter().filter(|(t, _)| ["一", "二", "三"].contains(&t.as_str())).map(|(_, r)| r.left()).collect()
    };
    let rail = 260.0;
    let held = lefts(rail);
    assert_eq!(held.len(), 3, "the rows are drawn: {held:?}");
    assert!(held.iter().all(|x| *x >= rail + 8.0), "inside the content area, off the rail: {held:?}");
    let free = lefts(0.0);
    assert_eq!(free.len(), 3, "the rows are drawn: {free:?}");
    assert!(free.iter().all(|x| *x >= 8.0), "inside the window: {free:?}");
}

#[test]
fn a_row_with_no_line_under_its_title_centres_the_title() {
    use zikaron_ui::{card, tokens::Type};
    let mut f = Frames::new();
    let mut rows: Vec<Rect> = Vec::new();
    let draw = |ctx: &egui::Context, rows: &mut Vec<Rect>| {
        rows.clear();
        central(ctx, egui::CentralPanel::default(), |ui| {
            ui.set_width(520.0);
            rows.push(full::radio_row(ui, egui::Id::new("r-bare"), false, "Lone title", None, "").rect);
            rows.push(full::radio_row(ui, egui::Id::new("r-two"), false, "Two line title", None, "a line under it").rect);
            card::form(ui, |ui, f| {
                rows.push(f.big(ui, None, "Big lone title", "", None).rect);
            });
        });
    };
    let mut out = f.run(0.1, vec![], |ctx| draw(ctx, &mut rows));
    for _ in 0..2 {
        out = f.run(0.1, vec![], |ctx| draw(ctx, &mut rows));
    }
    let drawn = texts(&out);
    let at = |t: &str| drawn.iter().find(|(x, _)| x == t).map(|(_, r)| *r).unwrap_or_else(|| panic!("{t} drawn"));
    let line = Type::Strong.line();
    assert!((rows[0].height() - (24.0 + line)).abs() < 0.5, "one line high: {:?}", rows[0]);
    assert!((at("Lone title").center().y - rows[0].center().y).abs() < 0.5, "centred: {:?} in {:?}", at("Lone title"), rows[0]);
    assert!(at("Two line title").center().y < rows[1].center().y - 4.0, "with a line under it, the title sits above the middle");
    assert!((at("Big lone title").center().y - rows[2].center().y).abs() < 0.5, "a large row's lone title centred: {:?} in {:?}", at("Big lone title"), rows[2]);
}

#[test]
fn a_passcode_row_under_a_sheet_takes_no_keys() {
    use zikaron_ui::{pin, secret::Secret};
    let typed = |sheet: bool| -> usize {
        let mut f = Frames::new();
        let mut value = Secret::default();
        let mut words = Rect::NOTHING;
        let draw = |ctx: &egui::Context, value: &mut Secret, words: &mut Rect| {
            central(ctx, egui::CentralPanel::default(), |ui| {
                ui.add_space(200.0);
                let _ = pin::pin_row(ui, "u-pin", value, 8, None, true, false);
            });
            if sheet {
                let mut n = 0;
                let _ = sheet::show(ctx, sheet::Spec::new("u-pin-sheet", 360.0), &mut n, |ui, _| {
                    *words = ui.label("卡上的一句话").rect;
                }, |_, _| ());
            }
        };
        for _ in 0..4 {
            let _ = f.run(0.1, vec![], |ctx| draw(ctx, &mut value, &mut words));
        }
        if sheet {
            let _ = f.run(0.1, press_at(words.center()), |ctx| draw(ctx, &mut value, &mut words));
            let _ = f.run(0.1, vec![], |ctx| draw(ctx, &mut value, &mut words));
        }
        let _ = f.run(0.1, vec![Event::Text("123".into())], |ctx| draw(ctx, &mut value, &mut words));
        let _ = f.run(0.1, vec![], |ctx| draw(ctx, &mut value, &mut words));
        value.chars()
    };
    assert_eq!(typed(false), 3, "with nothing over it the row takes what is typed");
    assert_eq!(typed(true), 0, "under the sheet the row takes nothing");
}

#[test]
fn a_name_that_says_the_sheet_title_again_is_not_drawn() {
    use zikaron_ui::{card, fields};
    let mut f = Frames::new();
    let title = "发布地址";
    let draw = |ctx: &egui::Context| {
        let mut unit = ();
        let _ = sheet::show(
            ctx,
            sheet::Spec::new("u4-echo", tokens::SHEET_W),
            &mut unit,
            |ui, _| {
                sheet::title(ui, title, "");
                fields::stack(ui, |ui, st| {
                    st.field(ui, title, None, None, |ui, w| ui.allocate_exact_size(vec2(w, 38.0), egui::Sense::hover()));
                    st.field(ui, "名称", None, None, |ui, w| ui.allocate_exact_size(vec2(w, 38.0), egui::Sense::hover()));
                });
                card::form(ui, |ui, f| {
                    let _ = f.info(ui, title, card::Show::Text("x"));
                });
            },
            |_, _| (),
        );
        egui::Area::new(egui::Id::new("u4-page")).fixed_pos(pos2(20.0, 20.0)).show(ctx, |ui| {
            ui.set_width(300.0);
            fields::stack(ui, |ui, st| st.field(ui, title, None, None, |ui, w| ui.allocate_exact_size(vec2(w, 38.0), egui::Sense::hover())));
        });
    };
    let mut out = f.run(0.1, vec![], draw);
    for _ in 0..12 {
        out = f.run(0.1, vec![], draw);
    }
    let said = texts(&out);
    let n = |s: &str| said.iter().filter(|(t, _)| t == s).count();
    assert_eq!(n(title), 2, "the sheet's title and the field outside any sheet; the field and the row in the sheet draw no name: {said:?}");
    assert_eq!(n("名称"), 1, "a field named otherwise keeps its name");
}
