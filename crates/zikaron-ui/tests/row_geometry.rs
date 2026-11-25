use zikaron_ui::egui::{self, Pos2, Rect};
use zikaron_ui::{button, card, mark, pin, states, table, width};

const SCREEN: egui::Vec2 = egui::Vec2 { x: 1180.0, y: 800.0 };

fn frame(ctx: &egui::Context, events: Vec<egui::Event>, mut add: impl FnMut(&mut egui::Ui)) -> egui::FullOutput {
    let t = ctx.input(|i| i.time) + 0.1;
    let mut input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(t), events, ..Default::default() };
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

fn hovered_ground(row: usize, head: bool) -> egui::CornerRadius {
    let ctx = dressed();
    let rows: Vec<table::Row> = (0..3).map(|i| table::Row { cells: vec![table::Cell::Text(format!("第 {i} 行"))], click: true, ..Default::default() }).collect();
    let cols = [table::col("名", table::Col::Fr(1.0))];
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
    out.shapes
        .iter()
        .find_map(|cs| match &cs.shape {
            egui::Shape::Rect(r) if r.fill.a() > 0 && r.rect.contains(pointer) && r.rect.height() <= zikaron_ui::tokens::ROW_H + 0.5 => Some(r.corner_radius),
            _ => None,
        })
        .expect("the hovered row's ground is painted")
}

#[test]
fn a_tables_lit_row_is_square_only_under_its_head_rule() {
    let r = (zikaron_ui::tokens::Radius::Card.px() - zikaron_ui::band::GAP) as u8;
    let cr = |t: u8, b: u8| egui::CornerRadius { nw: t, ne: t, sw: b, se: b };
    assert_eq!(hovered_ground(0, true), cr(0, r), "under the head's rule, over a line");
    assert_eq!(hovered_ground(1, true), cr(r, r), "between two lines");
    assert_eq!(hovered_ground(2, true), cr(r, r), "the last row, at the card's foot");
    assert_eq!(hovered_ground(0, false), cr(r, r), "with no head, the first row, at the card's top");
}

#[test]
fn a_lit_block_keeps_the_gap_to_the_card_and_meets_the_lines() {
    use zikaron_ui::band::{block, Bound, GAP};
    let slot = egui::Rect::from_min_max(egui::pos2(100.0, 200.0), egui::pos2(600.0, 256.0));
    let r = 16.0;
    let round = (r - GAP) as u8;
    let top_of = |s: Bound| if s == Bound::Rule { 0 } else { round };
    let gap = |s: Bound| if s == Bound::Edge { GAP } else { 0.0 };
    for (top, bottom) in [(Bound::Edge, Bound::Line), (Bound::Line, Bound::Line), (Bound::Line, Bound::Edge), (Bound::Edge, Bound::Edge), (Bound::Row, Bound::Row), (Bound::Rule, Bound::Line), (Bound::Rule, Bound::Edge)] {
        let (b, c) = block(slot, r, top, bottom);
        assert_eq!((b.left() - slot.left(), slot.right() - b.right(), b.top() - slot.top(), slot.bottom() - b.bottom()), (GAP, GAP, gap(top), gap(bottom)), "{top:?} {bottom:?}");
        assert_eq!(c, egui::CornerRadius { nw: top_of(top), ne: top_of(top), sw: round, se: round }, "{top:?} {bottom:?}");
    }
}

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
    let r = (zikaron_ui::tokens::Radius::Card.px() - zikaron_ui::band::GAP) as u8;
    assert_eq!(blocks[0].corner_radius, egui::CornerRadius::same(r), "rows 1 and 2, between two lines below the first row: round");
    assert_eq!(blocks[1].corner_radius, egui::CornerRadius::same(r), "row 4, under a line and at the card's foot: round");
    let joint = blocks[0].rect.center().y;
    let crossed = all.iter().any(|s| matches!(s, egui::Shape::LineSegment { points: [a, b], .. } if (a.y - b.y).abs() < 0.01 && (a.y - joint).abs() < 0.5));
    assert!(!crossed, "no line where the two chosen rows meet");
}

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
            egui::Shape::LineSegment { points: [a, b], .. } if (a.y - b.y).abs() < 0.01 && a.y > anchor.bottom() + 30.0 => Some(a.y),
            _ => None,
        });
    }
    let sep = sep.expect("the separator is drawn");
    let x = anchor.left() + 60.0;
    assert_eq!(lit_meets_lines(&ctx, egui::pos2(x, sep - 20.0), show), 1, "the menu row above the separator");
    assert_eq!(lit_meets_lines(&ctx, egui::pos2(x, sep + 20.0), show), 1, "the menu row under the separator");
}

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
    let ctx = dressed();
    let mut seen = 0;
    for k in 0..60 {
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

#[test]
fn under_a_scaled_layer_the_row_drawn_under_the_pointer_is_the_one_lit() {
    let ctx = dressed();
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("scaled"));
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

#[test]
fn a_small_inputs_figures_start_where_its_hint_starts() {
    use zikaron_ui::fields;
    let left_of_text = |typed: &str| {
        let ctx = dressed();
        let mut field = Rect::NOTHING;
        let mut out = None;
        for _ in 0..3 {
            out = Some(frame(&ctx, Vec::new(), |ui| {
                let mut t = typed.to_string();
                field = fields::small(ui, &mut t, "60 秒", 120.0, false).rect;
            }));
        }
        let xs: Vec<f32> = painted(&out.expect("a frame"))
            .iter()
            .filter_map(|s| match s {
                egui::Shape::Text(t) if !t.galley.text().is_empty() => Some(t.pos.x + t.galley.rect.left()),
                _ => None,
            })
            .collect();
        (xs.into_iter().fold(f32::INFINITY, f32::min), field)
    };
    let (hint_x, field) = left_of_text("");
    let (typed_x, _) = left_of_text("300");
    assert!((hint_x - typed_x).abs() < 1.0, "the hint at {hint_x}, the figures at {typed_x}");
    assert!(typed_x < field.center().x, "from the left half of the field {field:?}: {typed_x}");
}

#[test]
fn each_face_of_an_amount_shows_it_whole_on_hover_where_it_is_cut() {
    use zikaron_ui::layer::Amount;
    use zikaron_ui::{card, fields, kv};
    let cut = |n: usize| Amount::new(format!("0.{n}123 ETH"), format!("0.{n}12345678901234567 ETH"));
    let plain = |n: usize| Amount::new(format!("{n} ETH"), format!("{n} ETH"));
    assert!(cut(1).cut() && !plain(1).cut(), "the one judgement");
    let ctx = dressed();
    let mut wholes = Vec::new();
    for _ in 0..3 {
        frame(&ctx, Vec::new(), |ui| {
            zikaron_ui::probe::begin(ui.ctx());
            let cols = [table::col("额", table::Col::Fr(1.0))];
            let rows = vec![table::Row { cells: vec![table::Cell::Amount(cut(1))], ..Default::default() }, table::Row { cells: vec![table::Cell::Amount(plain(1))], ..Default::default() }];
            let _ = table::table(ui, "amounts", &cols, true, &rows, "");
            let (c2, p2) = (cut(2), plain(2));
            card::form(ui, |ui, f| f.facts(ui, &[("甲", card::Fact::Amount(&c2)), ("乙", card::Fact::Amount(&p2))]));
            kv::kv(ui, &[("丙", kv::Val::Amount(cut(3))), ("丁", kv::Val::Amount(plain(3)))]);
            let (c4, p4) = (cut(4), plain(4));
            fields::rows(ui, |ui, f| {
                let _ = f.info(ui, "戊", card::Show::MarkAmount(zikaron_ui::mark::Mark::Ok, &c4));
                let _ = f.info(ui, "己", card::Show::MarkAmount(zikaron_ui::mark::Mark::Ok, &p4));
            });
            wholes = zikaron_ui::probe::wholes_on_hover(ui.ctx());
        });
    }
    for n in 1..=4 {
        assert!(wholes.contains(&cut(n).whole), "the cut amount {n} is given whole on hover: {wholes:?}");
        assert!(!wholes.contains(&plain(n).whole), "the whole amount {n} gives nothing on hover: {wholes:?}");
    }
}

#[test]
fn a_form_rows_words_take_their_width_before_its_trail() {
    let ctx = dressed();
    let font = egui::FontId::new(15.0, egui::FontFamily::Proportional);
    for (title, long) in [("保留时长", false), ("一个很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长很长的标题", true), ("", false)] {
        let mut got = (Rect::NOTHING, Rect::NOTHING, 0.0_f32);
        for _ in 0..2 {
            frame(&ctx, Vec::new(), |ui| {
                card::form(ui, |ui, f| {
                    let mut vals = vec![String::from("1"), String::from("2"), String::from("3")];
                    let (row, boxed) = f.line(ui, None, title, "", false, false, zikaron_ui::palette::c(zikaron_ui::palette::C::Ink), |ui| {
                        let _ = zikaron_ui::lasts::boxed(ui, "c15", &mut vals, &["天", "时", "分"], "", &[false, false, false], true);
                        ui.min_rect()
                    });
                    got = (row.rect, boxed, zikaron_ui::width::width_in(ui, title, font.clone()));
                });
            });
        }
        let (row, boxed, words) = got;
        let words_left = row.left() + 16.0;
        if title.is_empty() {
            assert!(boxed.left() <= row.left() + 24.0, "no words: the trail takes the row from the padding: {boxed:?} in {row:?}");
        } else if long {
            assert!(boxed.width() >= 96.0 - 1.0, "long words leave the trail its floor: {boxed:?}");
            assert!(boxed.left() >= words_left + 40.0, "the long words still keep their room: {boxed:?}");
        } else {
            assert!(boxed.left() >= words_left + words * 0.8 + 16.0 - 1.0, "the trail starts after the words: {boxed:?}, words {words}");
        }
    }
}

fn after_the_box(layout: egui::Layout, joined: bool) -> (Rect, Rect) {
    use zikaron_ui::fields;
    let ctx = dressed();
    let mut got = (Rect::NOTHING, Rect::NOTHING);
    for _ in 0..2 {
        frame(&ctx, Vec::new(), |ui| {
            ui.with_layout(layout, |ui| {
                let start = ui.cursor();
                let mut t = String::from("300");
                let w = 120.0;
                let h = if joined {
                    let _ = fields::line(ui, &mut t, "an address", w, fields::Look::default());
                    zikaron_ui::tokens::INPUT_H
                } else {
                    let _ = fields::small(ui, &mut t, "60 秒", w, false);
                    zikaron_ui::tokens::KEY_H
                };
                let boxed = match layout.main_dir {
                    egui::Direction::RightToLeft => Rect::from_min_size(egui::pos2(start.max.x - w, start.min.y), egui::vec2(w, h)),
                    _ => Rect::from_min_size(start.min, egui::vec2(w, h)),
                };
                let (next, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                got = (boxed, next);
            });
        });
    }
    got
}

#[test]
fn what_follows_a_small_or_a_joined_field_starts_outside_its_box() {
    let mut inside = Vec::new();
    for joined in [false, true] {
        for (name, layout) in [("left to right", egui::Layout::left_to_right(egui::Align::Center)), ("right to left", egui::Layout::right_to_left(egui::Align::Center)), ("top down", egui::Layout::top_down(egui::Align::Min))] {
            let (boxed, next) = after_the_box(layout, joined);
            let outside = match layout.main_dir {
                egui::Direction::LeftToRight => next.left() >= boxed.right() - 0.5,
                egui::Direction::RightToLeft => next.right() <= boxed.left() + 0.5,
                _ => next.top() >= boxed.bottom() - 0.5,
            };
            if !outside {
                inside.push(format!("{} {name}: the box {boxed:?}, what follows {next:?}", if joined { "joined" } else { "small" }));
            }
        }
    }
    assert!(inside.is_empty(), "{inside:#?}");
}
