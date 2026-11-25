use std::cell::Cell as Count;
use zikaron_ui::egui::{self, Pos2, Rect};
use zikaron_ui::{mark, palette, probe, scroll, table, tokens};

fn frame(ctx: &egui::Context, size: egui::Vec2, events: Vec<egui::Event>, mut add: impl FnMut(&mut egui::Ui)) -> egui::FullOutput {
    let t = ctx.input(|i| i.time) + 0.1;
    let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)), time: Some(t), events, ..Default::default() };
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

const SCREEN: egui::Vec2 = egui::Vec2 { x: 900.0, y: 600.0 };

fn row(i: usize) -> table::Row {
    table::Row { cells: vec![table::Cell::Seq(i as u64), table::Cell::Tag(format!("类{}", i % 7)), table::Cell::Text(format!("第 {i} 行")), table::Cell::Mono("2026-11-18 12:00".into())], click: true, ..Default::default() }
}

fn cols() -> [table::Column<'static>; 4] {
    [table::SEQ, table::TYPE, table::col("", table::Col::Fr(1.0)), table::col("", table::Col::Px(136.0))]
}

fn asked(n: usize, frames: usize, generation: impl Fn(usize) -> u64) -> Vec<(usize, probe::Took)> {
    let ctx = dressed();
    let cols = cols();
    let mut out = Vec::new();
    for f in 0..frames {
        let calls = Count::new(0usize);
        frame(&ctx, SCREEN, Vec::new(), |ui| {
            scroll::page(ui, egui::ScrollArea::vertical().id_salt("page"), |ui| {
                let _ = table::table_with(ui, "sight", table::Spec::new(&cols, generation(f)), n, |i| {
                    calls.set(calls.get() + 1);
                    row(i)
                });
            });
        });
        let took = probe::tooks(&ctx).into_iter().find(|t| t.salt == "sight").expect("the table said what it took");
        out.push((calls.get(), took));
    }
    out
}

#[test]
fn the_rows_a_frame_asks_for_go_with_the_rows_in_sight_not_with_how_many_there_are() {
    let in_sight = (SCREEN.y / tokens::ROW_H).ceil() as usize + 2;
    let mut steady = Vec::new();
    for n in [500, 1000] {
        let got = asked(n, 4, |_| 7);
        assert_eq!(got[0].1.measured, n, "the first frame measures the columns over every row");
        for (calls, took) in &got[1..] {
            assert_eq!(took.measured, 0, "the same data is never measured again: {took:?}");
            assert_eq!(took.rows, n);
            assert_eq!(*calls, took.drawn, "the rows asked are the rows drawn");
            assert!(*calls <= in_sight + 1 && *calls > 0, "{calls} rows asked at {n} rows; in sight about {in_sight}");
        }
        steady.push(got[3].0);
    }
    assert_eq!(steady[0], steady[1], "N and 2N rows ask for as many rows a frame");
}

#[test]
fn the_columns_are_measured_once_per_generation_of_the_data() {
    let got = asked(300, 5, |f| if f < 2 { 1 } else { 2 });
    let measured: Vec<usize> = got.iter().map(|(_, t)| t.measured).collect();
    assert_eq!(measured, vec![300, 0, 300, 0, 0]);
}

#[test]
fn a_key_in_a_row_is_its_own_press_and_a_two_line_name_makes_the_rows_sixty_high() {
    let ctx = dressed();
    let cols = [table::col("名", table::Col::Auto { min: 56.0, max: 400.0 }), table::col("状态", table::Col::Auto { min: 56.0, max: 300.0 }), table::col_r("", table::Col::Auto { min: 56.0, max: 200.0 })];
    let rows: Vec<table::Row> = (0..3)
        .map(|i| table::Row {
            cells: vec![
                table::Cell::Two(format!("第 {i} 位"), "0x1234…abcd".into()),
                table::Cell::Lamp(mark::Mark::Ok, "在任".into()),
                table::Cell::Key("复制".into()),
            ],
            click: true,
            ..Default::default()
        })
        .collect();
    let extra = [table::Cell::Note("注".into()), table::Cell::Small("小".into()), table::Cell::Short("0xab…cd".into())];
    let mut hit = table::Hit::default();
    let mut laid = Vec::new();
    let draw = |ui: &mut egui::Ui, hit: &mut table::Hit| {
        *hit = table::table(ui, "keys", &cols, true, &rows, "");
        let more = [table::Row { cells: extra.to_vec(), ..Default::default() }];
        let _ = table::table(ui, "more", &[table::col("", table::Col::Fr(1.0)), table::col("", table::Col::Px(80.0)), table::col("", table::Col::Px(120.0))], false, &more, "");
    };
    for _ in 0..3 {
        frame(&ctx, SCREEN, Vec::new(), |ui| draw(ui, &mut hit));
        laid = probe::rows(&ctx);
    }
    assert_eq!(laid.len(), 3, "three rows that open");
    for r in &laid {
        assert!((r.height() - table::ROW_TWO_H).abs() < 0.01, "a row with a two-line name is 60 high: {r:?}");
    }
    let last = laid[2];
    let at = egui::pos2(last.right() - tokens::ROW_INSET - 10.0, last.center().y);
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    frame(&ctx, SCREEN, vec![egui::Event::PointerMoved(at)], |ui| draw(ui, &mut hit));
    frame(&ctx, SCREEN, vec![press(true)], |ui| draw(ui, &mut hit));
    frame(&ctx, SCREEN, vec![press(false)], |ui| draw(ui, &mut hit));
    assert_eq!(hit.key, Some((2, 2)), "the key answers as its row and column");
    assert_eq!(hit.clicked, None, "and the row is not pressed with it");
}

#[test]
fn a_head_that_sorts_answers_its_press() {
    let ctx = dressed();
    let cols = [table::col("名", table::Col::Fr(1.0)), table::col("时刻", table::Col::Px(160.0))];
    let rows: Vec<table::Row> = (0..2).map(|i| table::Row { cells: vec![table::Cell::Text(format!("{i}")), table::Cell::Mono("12:00".into())], ..Default::default() }).collect();
    let mut hit = table::Hit::default();
    let mut top = Pos2::ZERO;
    let mut w = 0.0;
    let draw = |ui: &mut egui::Ui, hit: &mut table::Hit, top: &mut Pos2, w: &mut f32| {
        *top = ui.cursor().min;
        *w = ui.available_width();
        *hit = table::table_with(ui, "sorted", table::Spec::new(&cols, 1).head(true).sort(&[1], Some(1)), rows.len(), |i| rows[i].clone());
    };
    frame(&ctx, SCREEN, Vec::new(), |ui| draw(ui, &mut hit, &mut top, &mut w));
    let at = egui::pos2(top.x + w - tokens::LIST_PAD_X - tokens::ROW_INSET - 40.0, top.y + tokens::TABLE_HEAD_H / 2.0);
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    frame(&ctx, SCREEN, vec![egui::Event::PointerMoved(at)], |ui| draw(ui, &mut hit, &mut top, &mut w));
    frame(&ctx, SCREEN, vec![press(true)], |ui| draw(ui, &mut hit, &mut top, &mut w));
    frame(&ctx, SCREEN, vec![press(false)], |ui| draw(ui, &mut hit, &mut top, &mut w));
    assert_eq!(hit.head, Some(1));
}

#[test]
fn a_table_kept_whole_scrolls_sideways_where_its_columns_are_wider_than_its_room() {
    let ctx = dressed();
    let cols = [table::col("名", table::Col::Fr(1.0)), table::col("甲", table::Col::Px(300.0)), table::col("乙", table::Col::Px(300.0)), table::col("丙", table::Col::Px(300.0))];
    let rows: Vec<table::Row> = (0..2).map(|i| table::Row { cells: vec![table::Cell::Text(format!("{i}")), table::Cell::Pill("在任".into(), palette::Tone::Ok), table::Cell::Mono("x".into()), table::Cell::Mono("y".into())], click: true, ..Default::default() }).collect();
    for (size, wide_expected) in [(egui::vec2(700.0, 500.0), true), (egui::vec2(1600.0, 500.0), false)] {
        let mut room = 0.0;
        for _ in 0..3 {
            frame(&ctx, size, Vec::new(), |ui| {
                room = ui.available_width();
                let _ = table::table_with(ui, "wide", table::Spec::new(&cols, 3).head(true).wide(), rows.len(), |i| rows[i].clone());
            });
        }
        let laid = probe::rows(&ctx);
        let row_w = laid.first().map(|r| r.width()).unwrap_or(0.0);
        if wide_expected {
            assert!(row_w > room, "kept whole, the rows are wider than the room: {row_w} > {room}");
        } else {
            assert!(row_w < room, "with room, the rows are laid in it: {row_w} < {room}");
        }
    }
}

#[test]
fn narrow_a_span_of_dates_is_kept_whole_and_the_words_give_way() {
    let ctx = dressed();
    let span = "2026-01-01 – 2026-12-31";
    let cols = [table::col("记录", table::Col::Fr(1.0)), table::col("有效期", table::Col::Auto { min: 120.0, max: 320.0 }), table::col("状态", table::Col::Px(84.0))];
    let rows = vec![table::Row { cells: vec![table::Cell::Text("一件名字很长很长很长很长很长很长的记录".into()), table::Cell::Short(span.into()), table::Cell::Pill("有效".into(), palette::Tone::Ok)], click: true, ..Default::default() }];
    let mut out = None;
    for _ in 0..3 {
        out = Some(frame(&ctx, egui::vec2(470.0, 400.0), Vec::new(), |ui| {
            let _ = table::table(ui, "narrow", &cols, true, &rows, "");
        }));
    }
    let said: Vec<String> = out.expect("a frame").shapes.iter().filter_map(|cs| match &cs.shape {
        egui::Shape::Text(t) => Some(t.galley.text().to_string()),
        _ => None,
    }).collect();
    assert!(said.iter().any(|s| s == span), "the span whole: {said:?}");
    assert!(said.iter().any(|s| s.ends_with('…') && s.starts_with("一件")), "the record's words gave way: {said:?}");
}
