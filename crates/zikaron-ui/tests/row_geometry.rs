//! Where things land in a row, read in headless egui frames: a widget put in a row of `width::then` sits on the
//! row's middle line as the key at its end does (GALEED's probe, 2026-10-16: `states::okline` hung 9 points low,
//! its top on the row's middle), every widget of the library that stands in rows likewise; and the first row
//! under a table's head keeps its top corners square on the head's rule.

use zikaron_ui::egui::{self, Pos2, Rect};
use zikaron_ui::{button, card, mark, pin, states, table, width};

const SCREEN: egui::Vec2 = egui::Vec2 { x: 1180.0, y: 800.0 };

fn frame(ctx: &egui::Context, events: Vec<egui::Event>, mut add: impl FnMut(&mut egui::Ui)) -> egui::FullOutput {
    // A tenth of a second a frame, so what eases in (a row's hover ground) gets there.
    let t = ctx.input(|i| i.time) + 0.1;
    let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(t), events, ..Default::default() };
    // Two pixels a point, as the window has on the screens it ships to (layout rounds to pixels).
    input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(2.0);
    ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| add(ui));
    })
}

fn dressed() -> egui::Context {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    ctx
}

/// The middle line of what `lead` draws in a `width::then` row with a key at its end, and of the key, after two
/// frames (the second laid out with what the first measured).
fn centers(lead: impl Fn(&mut egui::Ui) -> Rect) -> (f32, f32) {
    let ctx = dressed();
    let mut got = (0.0, 0.0);
    for _ in 0..2 {
        frame(&ctx, Vec::new(), |ui| {
            card::card(ui, |ui| {
                let (l, t) = width::then(ui, |ui| button::show(ui, button::Key::new("读取报价栏目", button::Role::Secondary)).rect, |ui, _| lead(ui));
                got = (l.center().y, t.center().y);
            });
        });
    }
    got
}

#[test]
fn a_reading_line_in_a_row_sits_on_the_keys_middle_line() {
    let (lead, key) = centers(|ui| ui.scope(|ui| states::okline(ui, mark::Mark::Warn, "未读取报价栏目")).response.rect);
    assert!((lead - key).abs() < 0.5, "the reading line's middle {lead} against the key's {key}");
}

#[test]
fn every_widget_that_stands_in_rows_sits_on_the_rows_middle_line() {
    let cases: Vec<(&str, Box<dyn Fn(&mut egui::Ui) -> Rect>)> = vec![
        ("mark_line", Box::new(|ui| mark::mark_line(ui, mark::Mark::Ok, "已上链", zikaron_ui::tokens::Type::Body, egui::Color32::BLACK).rect)),
        ("strength", Box::new(|ui| ui.scope(|ui| pin::strength(ui, 2, mark::Mark::Warn, "中")).response.rect)),
        ("group_title", Box::new(|ui| ui.scope(|ui| card::group_title(ui, "读取网络")).response.rect)),
        ("mark", Box::new(|ui| mark::mark(ui, mark::Mark::Todo).rect)),

    ];
    for (name, lead) in cases {
        let (l, k) = centers(lead);
        assert!((l - k).abs() < 0.5, "{name}: its middle {l} against the key's {k}");
    }
}

/// Two lines where a row's words go (a name over its kind, beside a key): in a row as tall as they are, their middle
/// is the key's.
#[test]
fn two_lines_in_a_row_as_tall_as_they_are_sit_on_the_keys_middle_line() {
    use zikaron_ui::tokens::Type;
    let ctx = dressed();
    let lines = [("台子", Type::Page, egui::Color32::BLACK), ("记录者 · 助记词身份", Type::Note, egui::Color32::GRAY)];
    let mut got = (0.0, 0.0);
    for _ in 0..2 {
        frame(&ctx, Vec::new(), |ui| {
            card::card(ui, |ui| {
                let h = zikaron_ui::paint::lines_height(ui, &lines, 2.0).max(zikaron_ui::tokens::INPUT_H);
                let (l, t) = width::then_at(ui, h, |ui| button::show(ui, button::Key::new("复制地址", button::Role::Secondary)).rect, |ui, room| zikaron_ui::paint::lines(ui, &lines, 2.0, room).rect);
                got = (l.center().y, t.center().y);
            });
        });
    }
    assert!((got.0 - got.1).abs() < 0.5, "the lines' middle {} against the key's {}", got.0, got.1);
}

/// The ground of a row under a table's head: the rectangle painted with the hover fill, and its corners.
fn hovered_ground(row: usize, head: bool) -> egui::CornerRadius {
    let ctx = dressed();
    let rows: Vec<table::Row> = (0..3).map(|i| table::Row { cells: vec![table::Cell::Text(format!("第 {i} 行"))], click: true, ..Default::default() }).collect();
    let cols = [table::col("名", table::Col::Fr(1.0))];
    // Where the rows are, from a first frame; then the pointer rests on the one asked for until its ground is lit.
    let mut at = Rect::NOTHING;
    frame(&ctx, Vec::new(), |ui| {
        let top = ui.cursor().min;
        let _ = table::table(ui, "t", &cols, head, &rows, "");
        at = Rect::from_min_size(top, egui::vec2(ui.min_rect().width(), 0.0));
    });
    let y = at.top() + if head { zikaron_ui::tokens::TABLE_HEAD_H } else { zikaron_ui::tokens::LIST_PAD_Y } + zikaron_ui::tokens::ROW_H * (row as f32 + 0.5);
    let pointer = egui::pos2(at.center().x, y);
    let mut out = None;
    for _ in 0..30 {
        out = Some(frame(&ctx, vec![egui::Event::PointerMoved(pointer)], |ui| {
            let _ = table::table(ui, "t", &cols, head, &rows, "");
        }));
    }
    let out = out.expect("a frame");
    // The row's ground is the one filled rectangle a row high under the pointer (the card's surface is the whole table).
    out.shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            egui::Shape::Rect(r) if r.fill.a() > 0 && r.rect.contains(pointer) && (r.rect.height() - zikaron_ui::tokens::ROW_H).abs() < 0.5 => Some(r.corner_radius),
            _ => None,
        })
        .expect("the hovered row's ground is painted")
}

#[test]
fn the_first_row_under_a_head_keeps_its_top_corners_square() {
    let round = zikaron_ui::tokens::Radius::Ctl.egui();
    let first = hovered_ground(0, true);
    assert_eq!((first.nw, first.ne), (0, 0), "the first row's top corners meet the head's rule square");
    assert_eq!((first.sw, first.se), (round.sw, round.se), "its bottom corners stay round");
    assert_eq!(hovered_ground(1, true), round, "a row below the first is round all round");
    assert_eq!(hovered_ground(0, false), round, "with no head, the first row is round all round");
}
