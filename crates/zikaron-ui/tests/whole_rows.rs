use zikaron_ui::egui;

fn rows_of(v: &str, w: f32) -> Vec<String> {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))), time: Some(0.1), ..Default::default() };
    let out = ctx.run_ui(input, |root| {
        egui::CentralPanel::default().show_inside(root, |ui| {
            let _ = zikaron_ui::card::whole(ui, v, w);
        });
    });
    fn walk(s: &egui::Shape, text: &str, out: &mut Option<Vec<String>>) {
        match s {
            egui::Shape::Text(x) if x.galley.job.text == text && out.is_none() => {
                *out = Some(x.galley.rows.iter().map(|r| r.row.glyphs.iter().map(|g| g.chr).collect()).collect());
            }
            egui::Shape::Vec(v) => v.iter().for_each(|y| walk(y, text, out)),
            _ => {}
        }
    }
    let mut got = None;
    for c in &out.shapes {
        walk(&c.shape, v, &mut got);
    }
    got.expect("the value is drawn")
}

#[test]
fn a_value_shown_whole_fills_each_line() {
    let key = format!("zkp_{}", "a7".repeat(32));
    let rows = rows_of(&key, 300.0);
    assert!(rows.len() >= 2, "{rows:?}");
    assert_eq!(rows.concat(), key, "the whole key, nothing cut");
    let full = rows[0].chars().count();
    assert!(full > 4, "the mark not alone on its line: {rows:?}");
    for r in &rows[..rows.len() - 1] {
        assert_eq!(r.chars().count(), full, "each line but the last as full as the first: {rows:?}");
    }
}
