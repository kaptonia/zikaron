//! Where things land in a row, read in headless egui frames: a widget put in a row of `width::then` sits on the
//! row's middle line as the key at its end does (GALEED's probe, 2026-10-16: `states::okline` hung 9 points low,
//! its top on the row's middle), every widget of the library that stands in rows likewise; and a
//! table's lit row keeps one gap to the card's edge around it, with the card's corner less the gap there, and meets
//! a line square, its edge on the line's middle under the line drawn over it (in a table, a form card and a menu).

use zikaron_ui::egui::{self, Pos2, Rect};
use zikaron_ui::{button, card, mark, pin, states, table, width};

const SCREEN: egui::Vec2 = egui::Vec2 { x: 1180.0, y: 800.0 };

fn frame(ctx: &egui::Context, events: Vec<egui::Event>, mut add: impl FnMut(&mut egui::Ui)) -> egui::FullOutput {
    // A tenth of a second a frame, so what eases in (a row's hover ground) gets there.
    let t = ctx.input(|i| i.time) + 0.1;
    let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(t), events, ..Default::default() };
    // Two pixels a point, as the window has on the screens it ships to (layout rounds to pixels).
    input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(2.0);
    ctx.run_ui(input, |root| {
        egui::CentralPanel::default().show_inside(root, |ui| add(ui));
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
            egui::Shape::Rect(r) if r.fill.a() > 0 && r.rect.contains(pointer) && r.rect.height() <= zikaron_ui::tokens::ROW_H + 0.5 => Some(r.corner_radius),
            _ => None,
        })
        .expect("the hovered row's ground is painted")
}

#[test]
fn a_tables_lit_row_is_round_on_every_corner() {
    let round = egui::CornerRadius::same((zikaron_ui::tokens::Radius::Card.px() - zikaron_ui::band::GAP) as u8);
    assert_eq!(hovered_ground(0, true), round, "under the head's rule and over a line");
    assert_eq!(hovered_ground(1, true), round, "between two lines");
    assert_eq!(hovered_ground(2, true), round, "the last row, at the card's foot");
    assert_eq!(hovered_ground(0, false), round, "with no head, the first row, at the card's top");
}

/// A lit block keeps one gap to the card's edge on every side that faces it; where it meets a line its edge lies on
/// the line's middle (the slot's boundary); a side that reaches the next row keeps no gap. Every corner is the
/// card's corner less the gap.
#[test]
fn a_lit_block_keeps_the_gap_to_the_card_and_meets_the_lines() {
    use zikaron_ui::band::{block, Bound, GAP};
    let slot = egui::Rect::from_min_max(egui::pos2(100.0, 200.0), egui::pos2(600.0, 256.0));
    let r = 16.0;
    let want = egui::CornerRadius::same((r - GAP) as u8);
    let gap = |s: Bound| if s == Bound::Edge { GAP } else { 0.0 };
    for (top, bottom) in [(Bound::Edge, Bound::Line), (Bound::Line, Bound::Line), (Bound::Line, Bound::Edge), (Bound::Edge, Bound::Edge), (Bound::Row, Bound::Row)] {
        let (b, c) = block(slot, r, top, bottom);
        assert_eq!((b.left() - slot.left(), slot.right() - b.right(), b.top() - slot.top(), slot.bottom() - b.bottom()), (GAP, GAP, gap(top), gap(bottom)), "{top:?} {bottom:?}");
        assert_eq!(c, want, "{top:?} {bottom:?}");
    }
}

/// Chosen rows that touch are lit as one block (one rounded rectangle over them all, no line between them); a
/// chosen row alone is its own block.
#[test]
fn chosen_rows_that_touch_are_one_block() {
    let ctx = dressed();
    let on = [false, true, true, false, true];
    let rows: Vec<table::Row> = on.iter().enumerate().map(|(i, o)| table::Row { cells: vec![table::Cell::Text(format!("第 {i} 行"))], click: true, on: *o, ..Default::default() }).collect();
    let cols = [table::col("名", table::Col::Fr(1.0))];
    let mut out = None;
    for _ in 0..4 {
        out = Some(frame(&ctx, Vec::new(), |ui| {
            let _ = table::table(ui, "t", &cols, true, &rows, "");
        }));
    }
    let all = painted(&out.expect("a frame"));
    let sel = zikaron_ui::palette::c(zikaron_ui::palette::C::Sel);
    let blocks: Vec<egui::epaint::RectShape> = all.iter().filter_map(|s| match s { egui::Shape::Rect(r) if r.fill == sel => Some(r.clone()), _ => None }).collect();
    assert_eq!(blocks.len(), 2, "rows 1 and 2 are one block, row 4 another: {blocks:?}");
    let row_h = zikaron_ui::tokens::ROW_H;
    assert!((blocks[0].rect.height() - 2.0 * row_h).abs() < 0.5, "the first block spans two rows: {:?}", blocks[0].rect);
    let round = egui::CornerRadius::same((zikaron_ui::tokens::Radius::Card.px() - zikaron_ui::band::GAP) as u8);
    assert!(blocks.iter().all(|b| b.corner_radius == round), "every corner of each block round: {blocks:?}");
    let joint = blocks[0].rect.center().y;
    let crossed = all.iter().any(|s| matches!(s, egui::Shape::LineSegment { points: [a, b], .. } if (a.y - b.y).abs() < 0.01 && (a.y - joint).abs() < 0.5));
    assert!(!crossed, "no line where the two chosen rows meet");
}

/// A fold in a card is not a list's row: under the pointer it lights no block.
#[test]
fn a_fold_lights_no_block() {
    let ctx = dressed();
    let draw = |ui: &mut egui::Ui| {
        card::form(ui, |ui, f| {
            let _ = f.fold_from(ui, "more", "高级选项", "", false, |ui, f| f.note(ui, "x"));
        });
    };
    let mut top = Pos2::ZERO;
    let _ = frame(&ctx, Vec::new(), |ui| top = ui.cursor().min);
    let at = egui::pos2(400.0, top.y + 28.0);
    let mut out = None;
    for _ in 0..10 {
        out = Some(frame(&ctx, vec![egui::Event::PointerMoved(at)], draw));
    }
    let hover = zikaron_ui::palette::c(zikaron_ui::palette::C::Hover);
    let lit = painted(&out.expect("a frame")).into_iter().any(|s| matches!(s, egui::Shape::Rect(r) if r.fill == hover && r.rect.contains(at)));
    assert!(!lit, "the fold's head lights nothing under the pointer");
}

/// Every shape of a frame in the order it is painted.
fn painted(out: &egui::FullOutput) -> Vec<egui::Shape> {
    fn flat(s: &egui::Shape, all: &mut Vec<egui::Shape>) {
        match s {
            egui::Shape::Vec(v) => v.iter().for_each(|x| flat(x, all)),
            x => all.push(x.clone()),
        }
    }
    let mut all = Vec::new();
    out.shapes.iter().for_each(|cs| flat(&cs.shape, &mut all));
    all
}

/// The lit block under `pointer` after the pointer has rested there, and how many lines it meets: each line across
/// one of its edges has that edge on its middle and is painted after it (over it).
fn lit_meets_lines(ctx: &egui::Context, pointer: Pos2, mut add: impl FnMut(&mut egui::Ui)) -> usize {
    let mut out = None;
    for _ in 0..30 {
        out = Some(frame(ctx, vec![egui::Event::PointerMoved(pointer)], &mut add));
    }
    let all = painted(&out.expect("a frame"));
    let (at, block) = all
        .iter()
        .enumerate()
        .filter_map(|(k, s)| match s {
            egui::Shape::Rect(r) if r.fill.a() > 0 && r.rect.contains(pointer) && r.rect.height() <= 60.0 => Some((k, r.rect)),
            _ => None,
        })
        .min_by(|a, b| a.1.area().total_cmp(&b.1.area()))
        .unwrap_or_else(|| panic!("the row under the pointer {pointer:?} is lit: {:?}", all.iter().filter_map(|s| if let egui::Shape::Rect(r) = s { Some((r.rect, r.fill)) } else { None }).collect::<Vec<_>>()));
    let mut met = 0;
    for (k, s) in all.iter().enumerate() {
        let egui::Shape::LineSegment { points: [a, b], .. } = s else { continue };
        if (a.y - b.y).abs() > 0.01 || a.x.max(b.x) < block.left() || a.x.min(b.x) > block.right() {
            continue;
        }
        for edge in [block.top(), block.bottom()] {
            if (a.y - edge).abs() <= 1.5 {
                assert!((a.y - edge).abs() < 0.01, "the block's edge {edge} lies on the line's middle {}", a.y);
                assert!(k > at, "the line at {} is painted over the block, not under it", a.y);
                met += 1;
            }
        }
    }
    met
}

#[test]
fn a_lit_blocks_edge_lies_under_the_line_it_meets() {
    // A table under its head: the middle row meets the line above and the one below.
    let ctx = dressed();
    let rows: Vec<table::Row> = (0..3).map(|i| table::Row { cells: vec![table::Cell::Text(format!("第 {i} 行"))], click: true, ..Default::default() }).collect();
    let cols = [table::col("名", table::Col::Fr(1.0))];
    let mut top = Pos2::ZERO;
    let _ = frame(&ctx, Vec::new(), |ui| top = ui.cursor().min);
    for (row, lines) in [(0, 2), (1, 2), (2, 1)] {
        let y = top.y + zikaron_ui::tokens::TABLE_HEAD_H + zikaron_ui::tokens::ROW_H * (row as f32 + 0.5);
        let met = lit_meets_lines(&ctx, egui::pos2(400.0, y), |ui| {
            let _ = table::table(ui, "t", &cols, true, &rows, "");
        });
        assert_eq!(met, lines, "table row {row}");
    }
    // A form card of rows that open something: the middle row meets two lines, the last one.
    let ctx = dressed();
    let draw = |ui: &mut egui::Ui| {
        card::form(ui, |ui, f| {
            for i in 0..3 {
                let _ = f.line(ui, None, &format!("第 {i} 行"), "", false, true, egui::Color32::BLACK, |_| ());
            }
        });
    };
    let mut rows_at = Vec::new();
    let out = frame(&ctx, Vec::new(), draw);
    for s in painted(&out) {
        if let egui::Shape::LineSegment { points: [a, b], .. } = s {
            if (a.y - b.y).abs() < 0.01 {
                rows_at.push(a.y);
            }
        }
    }
    rows_at.sort_by(f32::total_cmp);
    assert_eq!(rows_at.len(), 2, "two lines between three rows: {rows_at:?}");
    assert_eq!(lit_meets_lines(&ctx, egui::pos2(400.0, (rows_at[0] + rows_at[1]) / 2.0), draw), 2, "the form's middle row");
    assert_eq!(lit_meets_lines(&ctx, egui::pos2(400.0, rows_at[1] + 20.0), draw), 1, "the form's last row");
    // A menu: the rows on either side of its separator each meet it.
    use zikaron_ui::menu;
    let ctx = dressed();
    let id = egui::Id::new("m");
    menu::set(&ctx, id, true);
    let items = [menu::Item::Row(menu::Row { label: "甲", ..Default::default() }), menu::Item::Sep, menu::Item::Row(menu::Row { label: "乙", ..Default::default() }), menu::Item::Row(menu::Row { label: "丙", ..Default::default() })];
    let anchor = Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(120.0, 30.0));
    let show = |ui: &mut egui::Ui| {
        let _ = menu::show(ui.ctx(), id, anchor, true, 200.0, &items);
    };
    let mut sep = None;
    for _ in 0..30 {
        let out = frame(&ctx, Vec::new(), show);
        sep = painted(&out).into_iter().find_map(|s| match s {
            // The separator, below the card's first row (the card's own top light is a line too).
            egui::Shape::LineSegment { points: [a, b], .. } if (a.y - b.y).abs() < 0.01 && a.y > anchor.bottom() + 30.0 => Some(a.y),
            _ => None,
        });
    }
    let sep = sep.expect("the separator is drawn");
    let x = anchor.left() + 60.0;
    assert_eq!(lit_meets_lines(&ctx, egui::pos2(x, sep - 20.0), show), 1, "the menu row above the separator");
    assert_eq!(lit_meets_lines(&ctx, egui::pos2(x, sep + 20.0), show), 1, "the menu row under the separator");
}

/// A details value with line breaks is drawn a line each, the row as tall as its lines, whatever its kind (a figure
/// as much as a machine value or words).
#[test]
fn a_value_with_line_breaks_is_drawn_a_line_each_whatever_its_kind() {
    use zikaron_ui::card::Detail;
    for v in [Detail::Figure("Inter\nNoto Sans SC\nJetBrains Mono".to_string()), Detail::Mono("0xaaaa\n0xbbbb\n0xcccc".to_string()), Detail::Text("one\ntwo\nthree".to_string())] {
        let ctx = dressed();
        let mut h = 0.0;
        let out = frame(&ctx, Vec::new(), |ui| {
            let top = ui.cursor().min.y;
            card::form(ui, |ui, f| {
                let _ = f.detail_rows(ui, "d", &[("Fonts", v.clone())], 80.0);
            });
            h = ui.cursor().min.y - top;
        });
        let ys: Vec<f32> = painted(&out).into_iter().filter_map(|s| match s {
            egui::Shape::Text(t) if t.galley.job.text != "Fonts" && !t.galley.job.text.is_empty() => Some(t.pos.y),
            _ => None,
        }).collect();
        assert_eq!(ys.len(), 3, "three lines drawn: {ys:?}");
        assert!(ys.windows(2).all(|w| w[1] - w[0] >= 18.0), "each on its own line: {ys:?}");
        assert!(h >= 3.0 * 20.0 + 20.0 - 0.5, "the row as tall as its lines: {h}");
    }
}

/// A menu row takes the pointer over the very block it lights: between a row and the separator under it, where its
/// block reaches the line, the row is lit; and while a sheet comes in (scaled about where it opens), any block lit
/// is under the pointer as the frame draws it (a sheet lights nothing until it is all but in: the next test reads
/// the rule under a scaled layer itself).
#[test]
fn a_menu_rows_lit_block_is_where_it_takes_the_pointer() {
    use zikaron_ui::menu;
    let items = [menu::Item::Row(menu::Row { label: "甲", ..Default::default() }), menu::Item::Sep, menu::Item::Row(menu::Row { label: "乙", ..Default::default() }), menu::Item::Row(menu::Row { label: "丙", ..Default::default() })];
    let anchor = Rect::from_min_size(egui::pos2(100.0, 100.0), egui::vec2(120.0, 30.0));
    let id = egui::Id::new("m-hit");
    let show = |ui: &mut egui::Ui| {
        let _ = menu::show(ui.ctx(), id, anchor, true, 200.0, &items);
    };
    let hover = zikaron_ui::palette::c(zikaron_ui::palette::C::Hover);
    // Settled: the separator's line, and the pointer 3 above it (past the row, inside its block).
    let ctx = dressed();
    menu::set(&ctx, id, true);
    let mut sep = 0.0;
    for _ in 0..20 {
        let out = frame(&ctx, Vec::new(), show);
        if let Some(y) = painted(&out).into_iter().find_map(|s| match s {
            egui::Shape::LineSegment { points: [a, b], .. } if (a.y - b.y).abs() < 0.01 && a.y > anchor.bottom() + 30.0 => Some(a.y),
            _ => None,
        }) {
            sep = y;
        }
    }
    let at = egui::pos2(anchor.left() + 60.0, sep - 3.0);
    let mut lit = Vec::new();
    for _ in 0..10 {
        let out = frame(&ctx, vec![egui::Event::PointerMoved(at)], show);
        lit = painted(&out).into_iter().filter_map(|s| match s { egui::Shape::Rect(r) if r.fill == hover => Some(r.rect), _ => None }).collect();
    }
    assert!(lit.len() == 1 && lit[0].contains(at), "the row above the separator is lit with the pointer in its block by the line: {lit:?}");
    // Coming in: a sheet of rows that open something, scaled as it comes; the pointer resting near a row's edge.
    // Every frame of the entrance, a block lit covers the pointer as the frame draws it.
    use zikaron_ui::{card, sheet};
    let ctx = dressed();
    let draw = |ctx: &egui::Context| {
        let mut n = 0;
        let _ = sheet::show(ctx, sheet::Spec::new("t-hit", 480.0), &mut n, |ui, _| {
            card::form(ui, |ui, f| {
                for i in 0..5 {
                    let _ = f.line(ui, None, &format!("第 {i} 行"), "", false, true, egui::Color32::BLACK, |_| ());
                }
            });
        }, |_, _| ());
    };
    // Where the rows stand once in: the third row's top edge.
    let mut edges = Vec::new();
    for _ in 0..40 {
        let out = frame(&ctx, Vec::new(), |ui| draw(ui.ctx()));
        edges = painted(&out).into_iter().filter_map(|s| match s {
            egui::Shape::LineSegment { points: [a, b], .. } if (a.y - b.y).abs() < 0.01 && (a.x - b.x).abs() > 200.0 => Some((a.x.min(b.x), a.y)),
            _ => None,
        }).collect();
    }
    edges.sort_by(|a, b| a.1.total_cmp(&b.1));
    assert!(edges.len() >= 2, "{edges:?}");
    let rest = egui::pos2(edges[1].0 + 120.0, edges[1].1 + 12.0);
    // The same sheet opened afresh with the pointer resting there.
    let ctx = dressed();
    let mut seen = 0;
    for k in 0..60 {
        // A sixtieth of a second a frame, to catch the entrance on its way.
        let t = ctx.input(|i| i.time) + 1.0 / 60.0;
        let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(t), events: vec![egui::Event::PointerMoved(rest)], ..Default::default() };
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(2.0);
        let out = ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show_inside(root, |ui| draw(ui.ctx()));
        });
        for r in painted(&out).into_iter().filter_map(|s| match s { egui::Shape::Rect(r) if r.fill == hover => Some(r.rect), _ => None }) {
            seen += 1;
            assert!(r.expand(0.5).contains(rest), "frame {k}: the block lit {r:?} is under the pointer {rest:?} as drawn");
        }
    }
    assert!(seen > 0, "a row lights under the pointer at all");
}

/// Under a layer drawn scaled (as a sheet or a menu is while it comes in), the row lit is the one drawn under the
/// pointer, not the one that would be there unscaled.
#[test]
fn under_a_scaled_layer_the_row_drawn_under_the_pointer_is_the_one_lit() {
    let ctx = dressed();
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("scaled"));
    // Laid out at (100, 100)–(150, 150); drawn at half size about the origin, at (50, 50)–(75, 75).
    let laid = Rect::from_min_max(egui::pos2(100.0, 100.0), egui::pos2(150.0, 150.0));
    let mut on = Vec::new();
    for at in [egui::pos2(60.0, 60.0), egui::pos2(120.0, 120.0)] {
        let mut lit = false;
        for _ in 0..3 {
            let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(ctx.input(|i| i.time) + 0.5), events: vec![egui::Event::PointerMoved(at)], ..Default::default() };
            input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(2.0);
            let _ = ctx.run_ui(input, |root| {
                let ctx = &root.ctx().clone();
                ctx.set_transform_layer(layer, egui::emath::TSTransform { scaling: 0.5, translation: egui::Vec2::ZERO });
                egui::Area::new(layer.id).order(layer.order).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                    let _ = ui.allocate_rect(Rect::from_min_size(Pos2::ZERO, egui::vec2(300.0, 300.0)), egui::Sense::hover());
                    lit = zikaron_ui::flow::pointer_on(ui, laid);
                });
            });
        }
        on.push(lit);
    }
    assert_eq!(on, vec![true, false], "lit where drawn (60, 60), not where it would be unscaled (120, 120)");
}
