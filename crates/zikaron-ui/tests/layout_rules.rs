//! Two layout rules of the library, on its committed source and in headless frames: every scroll area goes in
//! through `scroll` and says whether it is a page's or a nested one, and a nested one at its end keeps the wheel
//! (GALEED's probe, 2026-10-16: an inner list at its end carried the page 597 points); and no widget opens a row
//! of its own in the layout it was handed (it takes one cell, `width::cell`), only a container in a region it owns.

use std::path::Path;
use zikaron_ui::egui::{self, Pos2, Rect};

fn files() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("src")).expect("src/ reads") {
        let p = e.expect("a directory entry").path();
        if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            out.push((p.file_name().unwrap().to_string_lossy().to_string(), std::fs::read_to_string(&p).expect("a source file reads")));
        }
    }
    out.sort();
    out
}

/// Code without its comment lines: the scan is of code, not prose.
fn code_only(s: &str) -> String {
    s.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n")
}

/// The name of the call whose parentheses hold `at` (the path just before the nearest unclosed `(`).
fn enclosing_call(code: &str, at: usize) -> String {
    let b = code.as_bytes();
    let mut depth = 0i32;
    let mut i = at;
    while i > 0 {
        i -= 1;
        match b[i] {
            b')' => depth += 1,
            b'(' if depth == 0 => {
                let head = &code[..i];
                let start = head.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == ':')).map(|k| k + 1).unwrap_or(0);
                return head[start..].to_string();
            }
            b'(' => depth -= 1,
            _ => {}
        }
    }
    String::new()
}

/// The function a byte of `code` is in (the last `fn` before it).
fn enclosing_fn(code: &str, at: usize) -> String {
    let head = &code[..at];
    let Some(k) = head.rfind("fn ") else { return String::new() };
    head[k + 3..].chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect()
}

#[test]
fn every_scroll_area_goes_in_through_scroll() {
    let mut seen = 0;
    for (name, text) in files() {
        if name == "scroll.rs" {
            continue;
        }
        let code = code_only(&text);
        for (at, _) in code.match_indices("ScrollArea::") {
            let call = enclosing_call(&code, at);
            assert!(["scroll::page", "scroll::nested", "scroll::nested_rows"].iter().any(|w| call.ends_with(w)), "{name}: a scroll area shown by `{call}`, not through `scroll`");
            seen += 1;
        }
    }
    // The page body, the rail, a whole-window column, a picker's list and a sheet's body.
    assert_eq!(seen, 5, "the scan reaches the library's five scroll areas");
}

#[test]
fn no_widget_opens_a_row_in_the_layout_it_was_handed() {
    // Containers laying their own parts in a region they made: the key-value table's rows (inside its own column),
    // a picker's two date fields (inside its own floating card).
    let owned = [("kv.rs", "kv"), ("pick.rs", "show_from")];
    let mut met = std::collections::BTreeSet::new();
    for (name, text) in files() {
        let code = code_only(&text);
        for opener in [".horizontal(", ".horizontal_top(", ".horizontal_wrapped(", ".horizontal_centered("] {
            for (at, _) in code.match_indices(opener) {
                let f = enclosing_fn(&code, at);
                assert!(owned.contains(&(name.as_str(), f.as_str())), "{name}: `{f}` opens a row ({opener}) in the layout it was handed; take one cell (`width::cell`)");
                met.insert((name.clone(), f));
            }
        }
    }
    for (file, f) in owned {
        assert!(met.contains(&(file.to_string(), f.to_string())), "the scan no longer meets {file} `{f}`: drop it from the containers");
    }
}

/// An outer page area with a nested one (150 high, much longer inside) between its lines, in an area of the window
/// (a layer the pointer is known to rest on); the pointer rests on the nested one and the wheel turns a line a
/// frame. Returns the two areas' offsets after `frames` frames.
fn wheel_over_nested(frames: usize, nested: bool) -> (f32, f32) {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let mut inner_rect = Rect::NOTHING;
    let (mut outer_y, mut inner_y) = (0.0, 0.0);
    for i in 0..frames {
        let mut events = Vec::new();
        if inner_rect.is_positive() {
            events.push(egui::Event::PointerMoved(inner_rect.center()));
            events.push(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, -3.0), modifiers: egui::Modifiers::NONE });
        }
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))), time: Some(i as f64 / 60.0), events, ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            egui::Area::new(egui::Id::new("window")).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                let outer = zikaron_ui::scroll::page(ui, egui::ScrollArea::vertical().id_salt("outer").max_height(560.0), |ui| {
                    for k in 0..10 {
                        ui.label(format!("页面行 {k}"));
                    }
                    let area = egui::ScrollArea::vertical().id_salt("inner").max_height(150.0);
                    let lines = |ui: &mut egui::Ui| {
                        for k in 0..40 {
                            ui.label(format!("编辑器行 {k}"));
                        }
                    };
                    let r = if nested { zikaron_ui::scroll::nested(ui, area, lines) } else { area.show(ui, lines) };
                    inner_rect = r.inner_rect;
                    inner_y = r.state.offset.y;
                    for k in 0..80 {
                        ui.label(format!("页面下文 {k}"));
                    }
                });
                outer_y = outer.state.offset.y;
            });
        });
    }
    (outer_y, inner_y)
}

#[test]
fn a_nested_area_at_its_end_keeps_the_wheel_and_the_page_stays() {
    // As egui leaves it, the page is carried once the inner area is at its end (the probe's own red leg).
    let (carried, _) = wheel_over_nested(200, false);
    assert!(carried > 0.0, "the probe moves no page without the rule ({carried})");
    let (outer, inner) = wheel_over_nested(200, true);
    assert!(inner > 0.0, "the nested area scrolled ({inner})");
    assert_eq!(outer, 0.0, "the page stayed where it was");
}

#[test]
fn a_nested_area_with_nothing_more_to_show_lets_the_wheel_reach_the_page() {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let mut inner_rect = Rect::NOTHING;
    let mut outer_y = 0.0;
    for i in 0..120 {
        let mut events = Vec::new();
        if inner_rect.is_positive() {
            events.push(egui::Event::PointerMoved(inner_rect.center()));
            events.push(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, -3.0), modifiers: egui::Modifiers::NONE });
        }
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))), time: Some(i as f64 / 60.0), events, ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            egui::Area::new(egui::Id::new("window")).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                outer_y = zikaron_ui::scroll::page(ui, egui::ScrollArea::vertical().id_salt("outer").max_height(560.0), |ui| {
                    ui.label("页面行");
                    // Three short lines in an area 150 high: nothing to scroll in it.
                    inner_rect = zikaron_ui::scroll::nested(ui, egui::ScrollArea::vertical().id_salt("inner").max_height(150.0), |ui| {
                        for k in 0..3 {
                            ui.label(format!("短行 {k}"));
                        }
                    })
                    .inner_rect;
                    for k in 0..80 {
                        ui.label(format!("页面下文 {k}"));
                    }
                })
                .state
                .offset
                .y;
            });
        });
    }
    assert!(outer_y > 0.0, "the page scrolled under a nested area that had nothing more ({outer_y})");
}

/// A nested area under another layer (a sheet floating over the page) keeps nothing of the wheel turned over that
/// layer: the pointer rests on the floating layer's own long list, drawn over the page's nested one, and that list
/// scrolls while the page's nested area under it stays.
#[test]
fn a_nested_area_under_another_layer_keeps_nothing_of_its_wheel() {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let mut top_rect = Rect::NOTHING;
    let (mut under_y, mut top_y) = (0.0, 0.0);
    let lines = |ui: &mut egui::Ui, word: &str| {
        for k in 0..60 {
            ui.label(format!("{word} {k}"));
        }
    };
    for i in 0..120 {
        let mut events = Vec::new();
        if top_rect.is_positive() {
            events.push(egui::Event::PointerMoved(top_rect.center()));
            events.push(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, -3.0), modifiers: egui::Modifiers::NONE });
        }
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))), time: Some(i as f64 / 60.0), events, ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            egui::Area::new(egui::Id::new("window")).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                under_y = zikaron_ui::scroll::nested(ui, egui::ScrollArea::vertical().id_salt("under").max_height(300.0), |ui| lines(ui, "页面行")).state.offset.y;
            });
            egui::Area::new(egui::Id::new("sheet")).order(egui::Order::Foreground).fixed_pos(Pos2::new(0.0, 50.0)).show(ctx, |ui| {
                let r = zikaron_ui::scroll::nested(ui, egui::ScrollArea::vertical().id_salt("over").max_height(200.0), |ui| lines(ui, "浮卡行"));
                top_rect = r.inner_rect;
                top_y = r.state.offset.y;
            });
        });
    }
    assert!(top_y > 0.0, "the floating layer's list scrolled ({top_y})");
    assert_eq!(under_y, 0.0, "the page's nested area under it stayed");
}

/// A cell's id is its parent's whatever was laid before it (`width::cell`): a field inside keeps its focus and state
/// while a line appears or goes above it.
#[test]
fn a_cell_keeps_its_id_whatever_was_laid_before_it() {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let id_with = |above: bool| {
        let mut got = None;
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(800.0, 600.0))), ..Default::default() };
        let _ = ctx.run(input, |ctx| {
            egui::Area::new(egui::Id::new("window")).fixed_pos(Pos2::ZERO).show(ctx, |ui| {
                if above {
                    ui.label("一行在上");
                }
                got = Some(zikaron_ui::width::cell(ui, egui::vec2(100.0, 20.0), egui::Layout::left_to_right(egui::Align::Center), |ui| ui.id()).0);
            });
        });
        got.expect("drawn")
    };
    assert_eq!(id_with(false), id_with(true), "the same cell, the same id");
}
