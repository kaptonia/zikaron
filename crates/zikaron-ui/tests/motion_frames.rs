//! The library's motion, read frame by frame in headless egui frames: the segmented thumb's stretch,
//! the switch's drop, a long action's key, one lit row in a list, rows that stay sliding while others close, the
//! rail's foot items being its items, the calendar's three levels, a fold and a floating card taking the page
//! along, words changing language in place, a sheet's height following its content, and reduced motion.

use std::cell::Cell;
use zikaron_ui::egui::{self, pos2, vec2, Event, PointerButton, Pos2, Rect};
use zikaron_ui::palette::{c, C};
use zikaron_ui::{button, datepick, flow, fold, motion, rail, scroll, seg, sheet, table, toggle};

const SCREEN: egui::Vec2 = egui::Vec2 { x: 1180.0, y: 800.0 };

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

    fn run(&mut self, dt: f64, events: Vec<Event>, mut add: impl FnMut(&mut egui::Ui)) -> egui::FullOutput {
        self.t += dt;
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(self.t), events, ..Default::default() };
        self.ctx.run_ui(input, |ui| {
            add(ui);
            zikaron_ui::layer::fade_out(ui.ctx());
        })
    }
}

fn moved(p: Pos2) -> Vec<Event> {
    vec![Event::PointerMoved(p)]
}

fn press_at(p: Pos2) -> Vec<Event> {
    vec![
        Event::PointerMoved(p),
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Default::default() },
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Default::default() },
    ]
}

fn wheel(at: Pos2, dy: f32) -> Vec<Event> {
    vec![Event::PointerMoved(at), Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, dy), phase: egui::TouchPhase::Move, modifiers: egui::Modifiers::NONE }]
}

/// Every shape drawn, flattened, with its clip.
fn flat(out: &egui::FullOutput) -> Vec<(Rect, egui::Shape)> {
    fn walk(s: &egui::Shape, clip: Rect, out: &mut Vec<(Rect, egui::Shape)>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, clip, out)),
            x => out.push((clip, x.clone())),
        }
    }
    let mut v = Vec::new();
    for cs in &out.shapes {
        walk(&cs.shape, cs.clip_rect, &mut v);
    }
    v
}

/// Every text drawn: words, where, and how opaque.
fn texts(out: &egui::FullOutput) -> Vec<(String, Rect, f32)> {
    flat(out)
        .into_iter()
        .filter_map(|(_, s)| match s {
            egui::Shape::Text(t) => Some((t.galley.text().to_string(), t.galley.rect.translate(t.pos.to_vec2()), t.opacity_factor)),
            _ => None,
        })
        .collect()
}

/// Rectangles filled with exactly `fill`.
fn fills(out: &egui::FullOutput, fill: egui::Color32) -> Vec<Rect> {
    flat(out)
        .into_iter()
        .filter_map(|(_, s)| match s {
            egui::Shape::Rect(r) if r.fill == fill && r.blur_width == 0.0 => Some(r.rect),
            _ => None,
        })
        .collect()
}

// ───────────────────────── Liquid ①: the segmented thumb ─────────────────────────

/// The thumb's leading edge gets there first and its trailing edge follows: on the way it is longer (by six
/// tenths of the distance, at most 56; most near half way), neither edge ever moves back, it lands at its own
/// size within 0.32 s at the kit's pace, and it never passes where it is going. Read at 120 frames a second.
#[test]
fn the_thumb_stretches_on_its_way_and_lands_at_its_size() {
    let mut f = Frames::new();
    let id = egui::Id::new("u1-thumb");
    let mut got = (0.0, 0.0);
    f.run(0.0, vec![], |ui| got = seg::slide(ui.ctx(), id, 0.0, 60.0));
    assert_eq!(got, (0.0, 60.0), "first seen, it is where it belongs");
    let mut seen: Vec<(f32, f32, f64)> = Vec::new();
    let t0 = f.t + 1.0 / 120.0;
    for _ in 0..60 {
        f.run(1.0 / 120.0, vec![], |ui| got = seg::slide(ui.ctx(), id, 200.0, 60.0));
        seen.push((got.0, got.1, f.t - t0));
    }
    let widest = seen.iter().map(|s| s.1).fold(0.0, f32::max);
    assert!(widest > 60.0 + 40.0 && widest <= 60.0 + 56.0 + 0.01, "stretched by at most 56: {widest}");
    for w in seen.windows(2) {
        assert!(w[1].0 >= w[0].0 - 0.01, "the trailing edge never moves back: {:?}", w);
        assert!(w[1].0 + w[1].1 >= w[0].0 + w[0].1 - 0.01 || w[1].0 + w[1].1 >= 260.0 - 0.01, "the leading edge never moves back: {:?}", w);
    }
    for s in &seen {
        assert!(s.0 <= 200.0 + 0.01 && s.0 + s.1 <= 260.0 + 56.0, "it never passes where it goes: {s:?}");
    }
    let leading_first = seen.iter().find(|s| s.0 + s.1 >= 259.0).map(|s| s.2).unwrap_or(9.0);
    let trailing_last = seen.iter().find(|s| s.0 >= 199.0).map(|s| s.2).unwrap_or(9.0);
    assert!(leading_first < trailing_last, "the leading edge arrives first ({leading_first} < {trailing_last})");
    let by = 0.32 * f64::from(zikaron_ui::tokens::PACE);
    let end = seen.iter().find(|s| s.2 >= by).expect("a frame past the slide");
    assert!((end.0 - 200.0).abs() < 0.5 && (end.1 - 60.0).abs() < 0.5, "landed at its size by {by} s: {end:?}");
}

/// Under reduced motion the thumb is where it goes at once.
#[test]
fn under_reduced_motion_the_thumb_arrives_at_once() {
    let mut f = Frames::new();
    let id = egui::Id::new("u1-thumb-still");
    let mut got = (0.0, 0.0);
    f.run(0.0, vec![], |ui| {
        motion::set_reduced(ui.ctx(), true);
        got = seg::slide(ui.ctx(), id, 0.0, 60.0);
    });
    f.run(1.0 / 60.0, vec![], |ui| got = seg::slide(ui.ctx(), id, 200.0, 60.0));
    assert_eq!(got, (200.0, 60.0));
}

// ───────────────────────── Liquid ④: the switch ─────────────────────────

/// Flipped, the knob first stretches (16 to 22 by 0.22 s), crossing as it does, and settles round where it goes
/// by 0.34 s, both at the kit's pace; it never passes the far end.
#[test]
fn the_knob_stretches_then_settles_round() {
    let mut f = Frames::new();
    let rect = Rect::from_min_size(pos2(100.0, 100.0), vec2(34.0, 20.0));
    let id = egui::Id::new("u1-switch");
    let knob = |out: &egui::FullOutput| fills(out, c(C::Knob)).into_iter().next().expect("the knob");
    let out = f.run(0.0, vec![], |ui| toggle::paint_switch(ui, rect, id, false, true));
    assert_eq!(knob(&out).width(), 16.0);
    let t0 = f.t + 1.0 / 60.0;
    let mut seen = Vec::new();
    for _ in 0..30 {
        let out = f.run(1.0 / 60.0, vec![], |ui| toggle::paint_switch(ui, rect, id, true, true));
        seen.push((knob(&out), f.t - t0));
    }
    let widest = seen.iter().map(|s| s.0.width()).fold(0.0, f32::max);
    assert!(widest > 20.0 && widest <= 22.01, "stretched toward 22: {widest}");
    let at_widest = seen.iter().find(|s| s.0.width() >= widest - 0.01).map(|s| s.1).unwrap_or(0.0);
    let pace = f64::from(zikaron_ui::tokens::PACE);
    assert!((0.15 * pace..=0.27 * pace).contains(&at_widest), "widest about {} s after the flip: {at_widest}", 0.22 * pace);
    for s in &seen {
        assert!(s.0.right() <= rect.right() - 2.0 + 0.01, "inside the track: {s:?}");
    }
    let end = seen.iter().find(|s| s.1 >= 0.35 * pace).expect("a frame past the cross");
    assert!((end.0.width() - 16.0).abs() < 0.01 && (end.0.right() - (rect.left() + 10.0 + 14.0 + 8.0)).abs() < 0.5, "round and there: {end:?}");
}

// ───────────────────────── Liquid ③: a long action's key ─────────────────────────

/// A key keeps its width through a long action: idle, busy, done and back.
#[test]
fn a_long_action_keeps_its_width() {
    let mut f = Frames::new();
    let mut widths = Vec::new();
    for (k, phase) in [button::Phase::Idle, button::Phase::Busy { frac: None }, button::Phase::Done { at: 1.2 }, button::Phase::Idle].into_iter().enumerate() {
        for _ in 0..4 {
            f.run(0.05, vec![], |ui| {
                let r = button::show(ui, button::Key::new("对账", button::Role::Primary).phase(phase));
                widths.push((k, r.rect.width()));
            });
        }
    }
    assert!(widths.iter().all(|w| (w.1 - widths[0].1).abs() < 0.01), "{widths:?}");
}

// ───────────────────────── One lit row ─────────────────────────

fn five_rows() -> Vec<table::Row> {
    ["甲", "乙", "丙", "丁", "戊"].iter().enumerate().map(|(i, s)| table::Row { cells: vec![table::Cell::Seq(i as u64 + 1), table::Cell::Text(s.to_string())], click: true, ..Default::default() }).collect()
}

const COLS: [table::Column<'static>; 2] = [table::SEQ, table::Column { head: "", width: table::Col::Fr(1.0), right: false }];

/// A list lights one row: the row under the pointer this frame, at once; moving to the next row lights it in
/// the very next frame and leaves no other half lit.
#[test]
fn a_list_lights_the_one_row_under_the_pointer() {
    let mut f = Frames::new();
    let rows = five_rows();
    let draw = |ui: &mut egui::Ui| {
        let _ = table::table(ui, "u1-rows", &COLS, false, &rows, "");
    };
    f.run(0.0, vec![], draw);
    f.run(0.1, vec![], draw);
    let hover = c(C::RowHover);
    for y in [10.0 + 4.0 + 56.0 * 1.5, 10.0 + 4.0 + 56.0 * 2.5, 10.0 + 4.0 + 56.0 * 3.5, 10.0 + 4.0 + 56.0 * 0.5] {
        let out = f.run(1.0 / 60.0, moved(pos2(200.0, y)), draw);
        let lit = fills(&out, hover);
        assert_eq!(lit.len(), 1, "one row lit at {y}: {lit:?}");
        assert!(lit[0].contains(pos2(200.0, y)), "the one under the pointer: {lit:?} at {y}");
    }
}

/// While the list scrolls no row is lit; once it stands still, the row under the pointer lights again.
#[test]
fn a_scrolling_list_lights_no_row() {
    let mut f = Frames::new();
    let rows: Vec<table::Row> = (0..40).map(|i| table::Row { cells: vec![table::Cell::Seq(i + 1), table::Cell::Text(format!("第 {i} 行"))], click: true, ..Default::default() }).collect();
    let draw = |ui: &mut egui::Ui| {
        scroll::page(ui, egui::ScrollArea::vertical().id_salt("u1-scroll"), |ui| {
            let _ = table::table(ui, "u1-long", &COLS, false, &rows, "");
        });
    };
    f.run(0.0, vec![], draw);
    f.run(0.1, moved(pos2(300.0, 300.0)), draw);
    let hover = c(C::RowHover);
    let mut dark = 0;
    for _ in 0..6 {
        let out = f.run(1.0 / 60.0, wheel(pos2(300.0, 300.0), -40.0), draw);
        dark += usize::from(fills(&out, hover).is_empty());
    }
    assert_eq!(dark, 6, "no row lit while it scrolls");
    let mut lit = 0;
    for _ in 0..30 {
        let out = f.run(1.0 / 60.0, vec![], draw);
        lit = fills(&out, hover).len();
    }
    assert_eq!(lit, 1, "still again, the row under the pointer lights");
}

// ───────────────────────── Rows in place ─────────────────────────

/// A row leaves: the rows below move up from where they were to where they belong in one direction, within
/// 0.4 s; the row that left fades out where it was; the list's card follows.
#[test]
fn rows_below_a_row_that_leaves_move_up_in_one_run() {
    let mut f = Frames::new();
    let all = five_rows();
    let fewer: Vec<table::Row> = all.iter().enumerate().filter(|(i, _)| *i != 1).map(|(_, r)| r.clone()).collect();
    let y_of = |out: &egui::FullOutput, w: &str| texts(out).into_iter().find(|t| t.0 == w).map(|t| t.1.center().y);
    let mut shown = all.clone();
    let mut draw = |ui: &mut egui::Ui, rows: &Vec<table::Row>| {
        let _ = table::table(ui, "u1-leave", &COLS, false, rows, "");
        shown = rows.clone();
    };
    f.run(0.0, vec![], |ui| draw(ui, &all));
    let before = f.run(0.1, vec![], |ui| draw(ui, &all));
    let from = y_of(&before, "丁").expect("丁 drawn");
    let mut ys = Vec::new();
    let mut ghost = Vec::new();
    let t0 = f.t + 1.0 / 60.0;
    for _ in 0..30 {
        let out = f.run(1.0 / 60.0, vec![], |ui| draw(ui, &fewer));
        ys.push((y_of(&out, "丁").expect("丁 drawn"), f.t - t0));
        ghost.push(texts(&out).into_iter().any(|t| t.0 == "乙"));
    }
    assert!((ys[0].0 - from).abs() < 2.0, "the first frame starts where it was: {} vs {from}", ys[0].0);
    for w in ys.windows(2) {
        assert!(w[1].0 <= w[0].0 + 0.01, "one direction: {w:?}");
    }
    let to = from - 56.0;
    let arrived = ys.iter().find(|y| (y.0 - to).abs() < 0.5).map(|y| y.1).unwrap_or(9.0);
    assert!(arrived <= 0.4, "in place within 0.4 s: {arrived}");
    assert!(ghost[0] && !*ghost.last().unwrap(), "the row that left fades out where it was, then is gone");
}

// ───────────────────────── The rail ─────────────────────────

/// The foot items are the rail's items: same height, width and left edge, the glyph and the words at the same
/// places.
#[test]
fn the_rail_foot_items_are_its_items() {
    let mut f = Frames::new();
    let draw = |ui: &mut egui::Ui| {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(Rect::from_min_size(pos2(10.0, 50.0), vec2(196.0, 700.0))));
        let lines = [rail::Line::Group(None), rail::Line::Item { glyph: zikaron_ui::icons::Glyph::Ledger, label: "账本", count: 0 }, rail::Line::Item { glyph: zikaron_ui::icons::Glyph::File, label: "存证", count: 2 }];
        let _ = rail::nav(&mut child, &lines, Some(0));
        rail::foot_rule(&mut child);
        let _ = rail::foot_item(&mut child, zikaron_ui::icons::Glyph::Gear, "设置", false, "");
        let _ = rail::foot_item(&mut child, zikaron_ui::icons::Glyph::Lock, "锁定", true, "");
    };
    f.run(0.0, vec![], draw);
    let out = f.run(0.1, vec![], draw);
    let words = texts(&out);
    let x = |w: &str| words.iter().find(|t| t.0 == w).map(|t| t.1.left()).expect(w);
    assert!((x("账本") - x("设置")).abs() < 0.01 && (x("存证") - x("锁定")).abs() < 0.01, "the words at one place");
    let rects: Vec<Rect> = f.ctx.viewport(|v| v.prev_pass.widgets.layers().flat_map(|(_, ws)| ws.iter().filter(|w| w.sense.senses_click()).map(|w| w.rect).collect::<Vec<_>>()).collect());
    let items: Vec<&Rect> = rects.iter().filter(|r| (r.height() - zikaron_ui::tokens::RAIL_ITEM_H).abs() < 0.01).collect();
    assert_eq!(items.len(), 4, "{rects:?}");
    assert!(items.iter().all(|r| (r.width() - items[0].width()).abs() < 0.01 && (r.left() - items[0].left()).abs() < 0.01), "{items:?}");
}

// ───────────────────────── The calendar ─────────────────────────

fn words() -> datepick::Words<'static> {
    datepick::Words { weekdays: ["一", "二", "三", "四", "五", "六", "日"], month: |y, m| format!("{y} 年 {m} 月"), year: |y| format!("{y} 年"), month_alone: |m| format!("{m} 月"), today: "今天", clear: "清除" }
}

/// Days, months, years: the head's words go up a level; a year picked goes to its months, a month picked to
/// its days; the card keeps one size (its foot row stays put).
#[test]
fn the_calendar_goes_up_and_down_its_three_levels() {
    let mut f = Frames::new();
    let mut value = String::from("2026-10-06");
    let field = Rect::from_min_size(pos2(40.0, 40.0), vec2(180.0, 38.0));
    let mut draw = |ui: &mut egui::Ui| {
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(field));
        let _ = datepick::field(&mut child, "u1-date", &mut value, "起", 180.0, (2026, 10, 18), &words());
    };
    f.run(0.0, vec![], &mut draw);
    f.run(0.1, press_at(field.center()), &mut draw);
    let out = f.run(0.3, vec![], &mut draw);
    let at = |out: &egui::FullOutput, w: &str| texts(out).into_iter().find(|t| t.0 == w && t.2 > 0.99).map(|t| t.1);
    let head = at(&out, "2026 年 10 月").expect("the days level's head");
    let foot = at(&out, "今天").expect("the foot");
    f.run(0.05, press_at(head.center()), &mut draw);
    let out = f.run(0.3, vec![], &mut draw);
    assert!(at(&out, "3 月").is_some() && at(&out, "2026 年").is_some(), "the months of 2026");
    assert_eq!(at(&out, "今天"), Some(foot), "one card size");
    let year_head = at(&out, "2026 年").unwrap();
    f.run(0.05, press_at(year_head.center()), &mut draw);
    let out = f.run(0.3, vec![], &mut draw);
    let y2028 = at(&out, "2028 年").expect("twelve years, 2028 among them");
    assert!(at(&out, "2019 年 \u{2013} 2030 年").is_some(), "the years level's head: the decade and a year either side");
    f.run(0.05, press_at(y2028.center()), &mut draw);
    let out = f.run(0.3, vec![], &mut draw);
    let march = at(&out, "3 月").expect("the months of 2028");
    assert!(at(&out, "2028 年").is_some());
    f.run(0.05, press_at(march.center()), &mut draw);
    let out = f.run(0.3, vec![], &mut draw);
    assert!(at(&out, "2028 年 3 月").is_some(), "the days of March 2028");
    assert_eq!(at(&out, "今天"), Some(foot), "one card size");
}

// ───────────────────────── Folds and floating cards take the page along ─────────────────────────

/// A fold opened near the window's foot takes the page along as it grows: once open, its body ends 32 above the
/// foot, and its head has not gone past the top.
#[test]
fn a_fold_opened_low_takes_the_page_along() {
    let mut f = Frames::new();
    let head = Cell::new(Rect::NOTHING);
    let body_end = Cell::new(0.0_f32);
    let mut draw = |ui: &mut egui::Ui| {
        scroll::page(ui, egui::ScrollArea::vertical().id_salt("u1-fold-page"), |ui| {
            ui.add_space(700.0);
            let top = ui.cursor().min.y;
            let _ = fold::fold(ui, "u1-fold", "详细信息", |ui| {
                for i in 0..8 {
                    ui.label(format!("第 {i} 行"));
                }
            });
            head.set(Rect::from_min_size(pos2(ui.min_rect().left(), top), vec2(80.0, 26.0)));
            body_end.set(ui.cursor().min.y);
            ui.add_space(600.0);
        });
    };
    f.run(0.0, vec![], &mut draw);
    f.run(0.1, vec![], &mut draw);
    f.run(0.05, press_at(head.get().center()), &mut draw);
    for _ in 0..40 {
        f.run(1.0 / 60.0, vec![], &mut draw);
    }
    let (head, body_end) = (head.get(), body_end.get());
    assert!(body_end <= SCREEN.y - fold::REVEAL_FOOT + 12.0, "the body seen whole: ends at {body_end}");
    assert!(head.top() >= 0.0, "the head stays in view: {head:?}");
}

/// A calendar open over a page: the wheel turned off the card scrolls the page, the card staying with its
/// field; scrolled out of view, the field's card closes.
#[test]
fn a_floating_card_stays_with_its_anchor_while_the_page_scrolls() {
    let mut f = Frames::new();
    let mut value = String::new();
    let field = Cell::new(Rect::NOTHING);
    let mut draw = |ui: &mut egui::Ui| {
        scroll::page(ui, egui::ScrollArea::vertical().id_salt("u1-pop-page"), |ui| {
            ui.add_space(200.0);
            let top = ui.cursor().min;
            let _ = datepick::field(ui, "u1-pop", &mut value, "起", 180.0, (2026, 10, 18), &words());
            field.set(Rect::from_min_size(top, vec2(180.0, 38.0)));
            ui.add_space(1600.0);
        });
    };
    f.run(0.0, vec![], &mut draw);
    f.run(0.1, press_at(field.get().center()), &mut draw);
    let out = f.run(0.3, vec![], &mut draw);
    let card_gap = |out: &egui::FullOutput, field: Rect| texts(out).into_iter().find(|t| t.0 == "今天").map(|t| t.1.top() - field.bottom());
    let gap0 = card_gap(&out, field.get()).expect("open");
    let y0 = field.get().top();
    for _ in 0..4 {
        f.run(1.0 / 60.0, wheel(pos2(900.0, 600.0), -30.0), &mut draw);
    }
    let out = f.run(0.3, vec![], &mut draw);
    assert!(field.get().top() < y0 - 20.0, "the page scrolled: {} from {y0}", field.get().top());
    let gap = card_gap(&out, field.get()).expect("still open");
    assert!((gap - gap0).abs() < 1.5, "the card stays with its field: {gap} vs {gap0}");
    for _ in 0..30 {
        f.run(1.0 / 60.0, wheel(pos2(900.0, 600.0), -60.0), &mut draw);
    }
    let out = f.run(0.3, vec![], &mut draw);
    assert!(field.get().bottom() < 0.0, "the field scrolled away: {:?}", field.get());
    assert!(card_gap(&out, field.get()).is_none(), "and its card closed");
}

// ───────────────────────── Words in place ─────────────────────────

/// The words change language: from the very frame of the switch, the changed words come in by their colour
/// while the old ones fade out where they were; words that did not change stay whole; by 0.3 s it is done.
#[test]
fn changed_words_cross_fade_in_place() {
    let mut f = Frames::new();
    let mut zh = true;
    let draw = |ui: &mut egui::Ui, zh: bool| {
        ui.label(if zh { "账本" } else { "Ledger" });
        ui.label("0x3c44…93bc");
    };
    f.run(0.0, vec![], |ui| draw(ui, zh));
    f.run(0.1, vec![], |ui| draw(ui, zh));
    zh = false;
    let out = f.run(1.0 / 60.0, vec![], |ui| {
        draw(ui, zh);
        flow::retext(ui.ctx());
    });
    let words = texts(&out);
    let new = words.iter().find(|t| t.0 == "Ledger").expect("the new words");
    let old = words.iter().find(|t| t.0 == "账本").expect("the old words, fading where they were");
    let same = words.iter().find(|t| t.0 == "0x3c44…93bc").expect("unchanged words");
    assert!(new.2 < 0.5 && old.2 > 0.5 && same.2 >= 0.999, "in transition from the first frame: {words:?}");
    let mut last = Vec::new();
    for _ in 0..20 {
        last = texts(&f.run(1.0 / 60.0, vec![], |ui| draw(ui, zh)));
    }
    assert!(last.iter().all(|t| t.2 >= 0.999) && !last.iter().any(|t| t.0 == "账本"), "done by 0.3 s: {last:?}");
}

// ───────────────────────── A sheet's height ─────────────────────────

/// A sheet whose content grows changes its height in place over 0.3 s, never in one jump: its keys, pinned to its
/// foot, move down step by step.
#[test]
fn a_sheet_follows_its_content_in_height() {
    let mut f = Frames::new();
    let draw = |ui: &mut egui::Ui, rows: usize| {
        let ctx = ui.ctx().clone();
        let mut st = ();
        let _ = sheet::show(
            &ctx,
            sheet::Spec::new("u1-sheet", 440.0),
            &mut st,
            |ui, _| {
                for i in 0..rows {
                    ui.label(format!("第 {i} 行"));
                }
            },
            |ui, _| {
                let _ = button::key(ui, "好", button::Role::Primary, true);
            },
        );
    };
    let key_y = |out: &egui::FullOutput| texts(out).into_iter().find(|t| t.0 == "好").map(|t| t.1.center().y);
    let mut y = 0.0;
    for _ in 0..40 {
        y = key_y(&f.run(1.0 / 60.0, vec![], |ui| draw(ui, 2))).unwrap_or(y);
    }
    let settled = y;
    let mut ys = Vec::new();
    for _ in 0..40 {
        ys.push(key_y(&f.run(1.0 / 60.0, vec![], |ui| draw(ui, 12))).expect("the key, every frame"));
    }
    let end = *ys.last().unwrap();
    assert!(end > settled + 50.0, "taller: its keys from {settled} to {end}");
    let jumps = std::iter::once(ys[0] - settled).chain(ys.windows(2).map(|w| w[1] - w[0])).fold(0.0, f32::max);
    assert!(jumps < (end - settled) * 0.5, "never in one jump: largest step {jumps} of {}: {ys:?}", end - settled);
    let there = ys.iter().position(|v| (v - end).abs() < 0.5).unwrap_or(99);
    assert!(there as f32 / 60.0 <= 0.4, "there within 0.4 s: {there} frames");
}

// ───────────────────────── Steady light ─────────────────────────

/// The pointer on the line between two rows of a form card lights one row, the lower: a row owns its top edge and
/// not its bottom one, and one place a pass holds the light.
#[test]
fn the_pointer_on_the_line_between_two_rows_lights_one() {
    use zikaron_ui::card;
    let mut f = Frames::new();
    let draw = |ui: &mut egui::Ui| {
        card::form(ui, |ui, f| {
            for i in 0..3 {
                let _ = f.line(ui, None, &format!("第 {i} 行"), "", false, true, egui::Color32::BLACK, |_| ());
            }
        });
    };
    let out = f.run(0.1, vec![], draw);
    let mut lines: Vec<f32> = flat(&out).into_iter().filter_map(|(_, s)| match s {
        egui::Shape::LineSegment { points: [a, b], .. } if (a.y - b.y).abs() < 0.01 => Some(a.y),
        _ => None,
    }).collect();
    lines.sort_by(f32::total_cmp);
    assert_eq!(lines.len(), 2, "{lines:?}");
    let hover = c(C::Hover);
    for y in [lines[0], lines[1], lines[0] - 0.25, lines[0] + 0.25] {
        let mut n = 0;
        for _ in 0..4 {
            let out = f.run(0.1, moved(pos2(300.0, y)), draw);
            n = fills(&out, hover).len();
        }
        assert_eq!(n, 1, "one row lit with the pointer at {y} (lines at {lines:?})");
    }
}

/// A trackpad's scroll and its momentum, its events thinning out and frames with none between them: no row is lit
/// at any frame while it goes on, and once it has stood still the row under the pointer lights and stays lit.
#[test]
fn momentum_scrolling_lights_no_row_until_it_stands_still() {
    let mut f = Frames::new();
    let rows: Vec<table::Row> = (0..60).map(|i| table::Row { cells: vec![table::Cell::Seq(i + 1), table::Cell::Text(format!("第 {i} 行"))], click: true, ..Default::default() }).collect();
    let draw = |ui: &mut egui::Ui| {
        scroll::page(ui, egui::ScrollArea::vertical().id_salt("u1-momentum"), |ui| {
            let _ = table::table(ui, "u1-momentum-list", &COLS, false, &rows, "");
        });
    };
    let at = pos2(300.0, 300.0);
    f.run(0.0, vec![], draw);
    f.run(0.1, moved(at), draw);
    let hover = c(C::RowHover);
    let step = 1.0 / 120.0;
    let mut lit_while_moving = 0;
    for dy in [-12.0, -12.0, -10.0, -8.0, -6.0, -4.0, -2.0, -1.0, -0.5, -0.2, -0.08, -0.03] {
        let out = f.run(step, wheel(at, dy), draw);
        lit_while_moving += fills(&out, hover).len();
        for _ in 0..2 {
            let out = f.run(step, vec![], draw);
            lit_while_moving += fills(&out, hover).len();
        }
    }
    assert_eq!(lit_while_moving, 0, "no row lit at any frame of the movement");
    let mut seen = Vec::new();
    for _ in 0..60 {
        let out = f.run(step, vec![], draw);
        seen.push(fills(&out, hover).len());
    }
    let first = seen.iter().position(|n| *n == 1).expect("the row under the pointer lights once it stands still");
    assert!(seen[first..].iter().all(|n| *n == 1), "and stays lit, never going dark again: {seen:?}");
}
