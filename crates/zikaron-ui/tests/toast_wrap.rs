//! A toast's sentence breaks where its words allow: English at the spaces between words (only a single word
//! longer than the line is cut inside itself), Chinese between any two characters. Read from the shapes a
//! real toast draws.

use zikaron_ui::egui;
use zikaron_ui::toast::{Toasts, Tone};

/// The rows of the first text a toast draws whose words are `text`, each row's characters.
fn rows_of(text: &str) -> Vec<String> {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let mut t = Toasts::new();
    t.set_labels("Details", "Hide details", "Close");
    t.say(text, Tone::Note, 0.0);
    // A few frames: the toast is on screen and settled by the last one.
    let mut out = egui::FullOutput::default();
    for time in [0.1, 0.5, 1.0] {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 800.0))), time: Some(time), ..Default::default() };
        out = ctx.run(input, |ctx| {
            let _ = t.draw(ctx, 216.0);
        });
    }
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
        walk(&c.shape, text, &mut got);
    }
    got.unwrap_or_else(|| {
        let mut seen = Vec::new();
        fn all(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(x) => out.push(x.galley.job.text.clone()),
                egui::Shape::Vec(v) => v.iter().for_each(|y| all(y, out)),
                _ => {}
            }
        }
        for c in &out.shapes {
            all(&c.shape, &mut seen);
        }
        panic!("the toast draws {text:?}; drawn: {seen:?}")
    })
}

#[test]
fn an_english_toast_breaks_between_words_and_a_chinese_one_anywhere() {
    // English, long enough for several rows: every row but the last ends at a space, so no word is split.
    let english = "The nodes could not be reached: check the network or choose another node in Settings, then try again from this window when the connection is back.";
    let rows = rows_of(english);
    assert!(rows.len() > 1, "wrapped: {rows:?}");
    for r in &rows[..rows.len() - 1] {
        assert!(r.ends_with(char::is_whitespace), "a row ends inside a word: {rows:?}");
    }
    assert_eq!(rows.concat(), english, "every character laid out once");
    // A single word longer than the line is the one thing cut inside itself (nothing else to break at).
    let long_word = "x".repeat(400);
    let rows = rows_of(&long_word);
    assert!(rows.len() > 1, "an overlong word is still wrapped: {} rows", rows.len());
    // Chinese, no spaces: broken between characters.
    let chinese = "配的节点一处也连不上,请检查网络,或在设置里换一处节点,然后在这一个窗口里再试一次;连接恢复之前,这一页上的读数都还是上一次读到的那一份,不是现在的。".repeat(2);
    let rows = rows_of(&chinese);
    assert!(rows.len() > 1, "wrapped: {rows:?}");
    assert!(rows[..rows.len() - 1].iter().any(|r| !r.ends_with(char::is_whitespace)), "Chinese breaks between characters: {rows:?}");
    assert_eq!(rows.concat(), chinese);
}

/// The rows of the text a key-value table's value draws, the table laid out as a page lays it (`width::body`, a
/// page `width` wide; `kv::kv` with one row), each row's characters.
fn value_rows(text: &str, mark: bool, width: f32) -> Vec<String> {
    use zikaron_ui::kv::Val;
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 800.0))), time: Some(0.1), ..Default::default() };
    let v = if mark { Val::Mark(zikaron_ui::mark::Mark::Warn, text.to_string()) } else { Val::text(text) };
    let out = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.allocate_ui(egui::vec2(width, 600.0), |ui| zikaron_ui::width::body(ui, |ui| zikaron_ui::kv::kv(ui, &[("Chain reading", v.clone())])));
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
        walk(&c.shape, text, &mut got);
    }
    got.unwrap_or_else(|| panic!("the value draws {text:?}"))
}

/// A key-value table's value breaks where its words allow, as a toast does: an English value (plain, and
/// with its status mark) ends every row but the last at a space; a Chinese value breaks between characters;
/// every character is laid out once.
#[test]
fn an_english_value_breaks_between_words_and_a_chinese_one_anywhere() {
    let english = "Only 1 node responded; no cross-check · 1 node(s) did not answer; see What each said under the details below";
    for mark in [false, true] {
        let rows = value_rows(english, mark, 520.0);
        assert!(rows.len() > 1, "wrapped (mark {mark}): {rows:?}");
        for r in &rows[..rows.len() - 1] {
            assert!(r.ends_with(char::is_whitespace), "a row ends inside a word (mark {mark}): {rows:?}");
        }
        assert_eq!(rows.concat(), english);
    }
    let chinese = "只有 1 处节点作答,无从互核 · 1 处节点未作答,各自的原话见「各处原话」,这一句在窄栏里要排成好几行才放得下".repeat(2);
    for mark in [false, true] {
        let rows = value_rows(&chinese, mark, 520.0);
        assert!(rows.len() > 1, "wrapped (mark {mark}): {rows:?}");
        assert!(rows[..rows.len() - 1].iter().any(|r| !r.ends_with(char::is_whitespace)), "Chinese breaks between characters: {rows:?}");
        assert_eq!(rows.concat(), chinese);
    }
}

/// A two-line toast's keys sit on its first line, with the icon: opening the details below (the toast grows
/// downward) moves neither the keys nor the icon. Read from where a real toast draws its key's words.
#[test]
fn opening_a_toasts_details_moves_none_of_its_keys() {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let mut t = Toasts::new();
    t.set_labels("Details", "Hide details", "Close");
    t.say_full("The nodes could not be reached.", "Check the network.", "E_UNREACHABLE: connect refused", Tone::Bad, 0.0);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 800.0));
    let mut time = 0.0;
    let mut frame = |t: &mut Toasts, events: Vec<egui::Event>| -> egui::FullOutput {
        time += 0.2;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(time), events, ..Default::default() };
        ctx.run(input, |ctx| {
            let _ = t.draw(ctx, 216.0);
        })
    };
    let at = |out: &egui::FullOutput, text: &str| -> Option<egui::Rect> {
        out.shapes.iter().find_map(|c| match &c.shape {
            egui::Shape::Text(x) if x.galley.job.text == text => Some(x.galley.rect.translate(x.pos.to_vec2())),
            _ => None,
        })
    };
    let mut out = frame(&mut t, Vec::new());
    for _ in 0..3 {
        out = frame(&mut t, Vec::new());
    }
    let shut = at(&out, "Details").expect("the details key is drawn");
    let close_shut = at(&out, "Close").expect("the close key is drawn");
    let p = shut.center();
    let press = |pressed| egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    let _ = frame(&mut t, vec![egui::Event::PointerMoved(p), press(true), press(false)]);
    for _ in 0..3 {
        out = frame(&mut t, Vec::new());
    }
    let open = at(&out, "Hide details").expect("the key now hides the details");
    assert!(at(&out, "E_UNREACHABLE: connect refused").is_some(), "the details are shown");
    assert!((open.center().y - shut.center().y).abs() < 0.5, "the key stays on the first line: {shut:?} then {open:?}");
    let close_open = at(&out, "Close").expect("the close key is drawn");
    assert!((close_open.center().y - close_shut.center().y).abs() < 0.5, "close stays too: {close_shut:?} then {close_open:?}");
}
