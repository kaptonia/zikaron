//! Checks of the control library, on the committed bytes: each either computes the library's own functions or
//! scans its source. Expected numbers and colors are the design's, written here, not read from the tables
//! under test.

use std::path::{Path, PathBuf};

fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn files() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(src()).expect("读不出 src/") {
        let p = e.expect("目录项").path();
        if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            out.push((name, std::fs::read_to_string(&p).expect("读不出源")));
        }
    }
    out.sort();
    out
}

/// Comments and docs also mention the scanned words (this file itself names `include_bytes!`), so comment
/// lines are removed first: the scan is of code, not prose.
fn code_only(s: &str) -> String {
    s.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn hex(c: zikaron_ui::egui::Color32) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

/// Bytes live only in the faces the closed table names `Place::Embedded` on this target: monospace and Latin body
/// everywhere, and the Chinese face on Linux (macOS takes it from the system). Embedding happens only in
/// `fonts.rs`; the files this build carries (an `include_bytes!` gated to another OS does not ship) are exactly
/// the embedded files of the table, and each reports its licence.
#[test]
fn bytes_are_embedded_only_where_the_closed_table_says_so() {
    use zikaron_ui::fonts::{embedded, Place, OFL, ROLES};
    for (name, text) in files() {
        if name == "fonts.rs" {
            continue;
        }
        assert!(!code_only(&text).contains("include_bytes!"), "{name} 里有 include_bytes!:字节只住 fonts.rs");
    }
    let mut declared: Vec<&str> = ROLES.iter().filter(|(_, _, _, p)| *p == Place::Embedded).map(|(_, f, _, _)| *f).collect();
    declared.sort();
    declared.dedup();
    let in_source = std::fs::read_to_string(src().join("fonts.rs")).expect("读不出 fonts.rs");
    let code = code_only(&in_source);
    let lines: Vec<&str> = code.lines().map(str::trim).collect();
    let mut carried: Vec<&str> = Vec::new();
    for (i, l) in lines.iter().enumerate() {
        let Some(at) = l.find("include_bytes!(\"") else { continue };
        let gate = if i > 0 { lines[i - 1] } else { "" };
        let elsewhere = gate.starts_with("#[cfg(target_os") && !gate.contains(&format!("\"{}\"", std::env::consts::OS));
        if elsewhere {
            continue;
        }
        let path = &l[at + "include_bytes!(\"".len()..];
        let path = &path[..path.find('"').expect("include_bytes! 的路没收尾")];
        carried.push(path.rsplit('/').next().unwrap_or(path));
    }
    carried.sort();
    carried.dedup();
    assert_eq!(carried, declared, "本构建随带的字体档,恰是表里本目标内嵌的那几档");
    for (role, file, _, place) in ROLES {
        let bytes = embedded(file);
        match place {
            Place::Embedded => {
                assert!(bytes.map(|b| !b.is_empty()).unwrap_or(false), "{} 说自己是内嵌的,字节却不在本件里", role.as_str());
            }
            Place::System => assert!(bytes.is_none(), "{} 是系统取的,字节不许随二进制走", role.as_str()),
        }
    }
    // Embedded faces report a licence; system faces are not distributed by this crate.
    for face in zikaron_ui::fonts::find().faces {
        let want = (face.place == Place::Embedded).then_some(OFL);
        assert_eq!(face.licence(), want, "{} 的许可报错了", face.role.as_str());
    }
}

/// Two color tables of one closed set of roles; the values the design names, light and dark.
#[test]
fn the_two_colour_tables_hold_the_designed_values() {
    use zikaron_ui::palette::{C, DARK, LIGHT};
    assert_eq!(LIGHT.len(), C::ALL.len(), "the light table has one value per role");
    assert_eq!(DARK.len(), C::ALL.len(), "the dark table has one value per role");
    let at = |k: C| C::ALL.iter().position(|x| *x == k).expect("a role of the set");
    for (k, l, d) in [
        (C::Ground, "#FAFAFC", "#17181B"),
        (C::Surface, "#FFFFFF", "#1F2024"),
        (C::Rail, "#F2F2F5", "#1B1C20"),
        (C::Line, "#E5E5EA", "#34353B"),
        (C::Ink, "#1D1D1F", "#F2F2F5"),
        (C::Ink2, "#6E6E73", "#A1A1AA"),
        (C::Ink3, "#A1A1A6", "#6E6E76"),
        (C::Accent, "#3B82F6", "#4F8FF7"),
        (C::Pen, "#A83A2C", "#C4533F"),
        (C::Ok, "#2F9E44", "#3FB25A"),
        (C::Warn, "#D9A12C", "#E0A93A"),
        (C::Bad, "#D13B30", "#E5534B"),
    ] {
        assert_eq!(hex(LIGHT[at(k)]), l, "{k:?} in the light table");
        assert_eq!(hex(DARK[at(k)]), d, "{k:?} in the dark table");
    }
    // The scrim darkens only: black at 30 % light, 50 % dark.
    assert_eq!((LIGHT[at(C::Scrim)].r(), LIGHT[at(C::Scrim)].a()), (0, 77), "light scrim is black at 30 %");
    assert_eq!((DARK[at(C::Scrim)].r(), DARK[at(C::Scrim)].a()), (0, 128), "dark scrim is black at 50 %");
}

/// The appearance blend is one value for the whole process, and tests run side by side: a test that moves it
/// and a test that reads colors through it hold this lock, and each starts from light.
fn light() -> std::sync::MutexGuard<'static, ()> {
    static BLEND: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let held = BLEND.lock().unwrap_or_else(|e| e.into_inner());
    zikaron_ui::palette::set_blend(0.0);
    held
}

/// Switching appearance blends the tables: every role reads the light value at 0 and the dark one at 1.
#[test]
fn the_blend_moves_every_role_between_its_two_values() {
    use zikaron_ui::palette::{c, set_blend, C, DARK, LIGHT};
    let _light = light();
    for (i, k) in C::ALL.iter().enumerate() {
        set_blend(0.0);
        assert_eq!(c(*k), LIGHT[i], "{k:?} at blend 0");
        set_blend(1.0);
        assert_eq!(c(*k), DARK[i], "{k:?} at blend 1");
    }
    set_blend(0.0);
}

/// Corners: card 12, keys and inputs 8, type tags 6, sheets 14, pills fully round.
#[test]
fn the_corners_are_the_designed_family() {
    use zikaron_ui::tokens::{Radius, PILL_H};
    assert_eq!(Radius::Card.px(), 12.0, "card corners");
    assert_eq!(Radius::Ctl.px(), 8.0, "key and input corners");
    assert_eq!(Radius::Tag.px(), 6.0, "type tag corners");
    assert_eq!(Radius::Sheet.px(), 14.0, "sheet corners");
    assert!(Radius::Pill.px() >= PILL_H / 2.0, "pills are fully round");
}

/// Type as the design sets it: page 22/30, card title 16/24, body 15/22, note 14/20, small 13/18, mono 14,
/// small mono 13, rail group titles and the status line 12/16, the identity kind and menu headings 11/14;
/// the rail's count 12.
#[test]
fn the_type_scale_is_the_designs() {
    use zikaron_ui::tokens::{Type, COUNT_TEXT};
    assert_eq!(COUNT_TEXT, 12.0, "the rail's count");
    for (t, size, line) in [
        (Type::Page, 22.0, 30.0),
        (Type::Card, 16.0, 24.0),
        (Type::Body, 15.0, 22.0),
        (Type::Note, 14.0, 20.0),
        (Type::Small, 13.0, 18.0),
        (Type::Mono, 14.0, 20.0),
        (Type::MonoSmall, 13.0, 18.0),
        (Type::Tiny, 12.0, 16.0),
        (Type::Micro, 11.0, 14.0),
    ] {
        assert_eq!((t.size(), t.line()), (size, line), "{t:?}");
    }
    for t in Type::ALL {
        assert!(t.size() >= 11.0, "{t:?} is {} under the smallest the design uses", t.size());
    }
    // Sizes under 14 come only from the type roles: no control writes one of its own.
    for (name, text) in files() {
        for (n, line) in code_only(&text).lines().enumerate() {
            for small in ["FontId::new(13.0", "FontId::new(12.0", "FontId::new(11.0", "FontId::new(12.5", "FontId::new(13.5"] {
                assert!(!line.contains(small), "{name}:{} writes a small size of its own: {line}", n + 1);
            }
        }
    }
}

/// Spacing and sizes: the scale 4 · 8 · 12 · 16 · 24 · 32 · 48; card 22 × 24 inside, 16 between; page 32;
/// sheet 24; key 34 high, 16 sides; input 38; table head 40, row 56, seq 36, tag 84, gaps 12; key-value 132
/// key column, rows 14 apart, columns 20; switch 34 × 20; pill and tag 24, tag at least 52; marks 18; rail 216.
#[test]
fn the_spacing_and_sizes_are_the_designed_ones() {
    use zikaron_ui::tokens as k;
    assert_eq!(k::SCALE, [4.0, 8.0, 12.0, 16.0, 24.0, 32.0, 48.0], "the spacing scale");
    assert_eq!((k::CARD_PAD_Y, k::CARD_PAD_X, k::CARD_GAP), (22.0, 24.0, 16.0), "cards");
    assert_eq!((k::PAGE_PAD, k::SHEET_PAD), (32.0, 24.0), "page and sheet margins");
    assert_eq!((k::KEY_H, k::KEY_PAD_X, k::KEY_PRESS), (34.0, 16.0, 0.97), "keys");
    assert_eq!(k::INPUT_H, 38.0, "inputs");
    assert_eq!((k::TABLE_HEAD_H, k::ROW_H, k::SEQ_W, k::TYPE_W, k::CELL_GAP), (40.0, 56.0, 36.0, 84.0, 12.0), "tables");
    assert_eq!((k::LABEL_W, k::KV_ROW_GAP, k::KV_COL_GAP), (132.0, 14.0, 20.0), "key-value rows");
    assert_eq!((k::SWITCH_W, k::SWITCH_H, k::SEG_H), (34.0, 20.0, 34.0), "switch and segments");
    assert_eq!((k::PILL_H, k::TAG_H, k::TAG_MIN_W, k::MARK), (24.0, 24.0, 52.0, 18.0), "pills, tags and marks");
    assert_eq!((k::SHEET_W, k::SHEET_WIDE, k::SHEET_XWIDE), (440.0, 520.0, 640.0), "sheet widths");
    assert_eq!(k::RAIL_W, 216.0, "the rail");
}

/// Motion: fast 120, middle 200, slow 300 ms; loops of 1.2 s.
#[test]
fn the_timings_are_the_designed_ones() {
    use zikaron_ui::tokens as k;
    assert_eq!((k::FAST, k::MID, k::SLOW, k::CYCLE), (0.12, 0.2, 0.3, 1.2));
    // The eased-out curve ends where it should and never overshoots; the spring does, and comes back.
    use zikaron_ui::motion::Curve;
    assert!((Curve::Ease.at(1.0) - 1.0).abs() < 1e-4 && Curve::Ease.at(0.0).abs() < 1e-4);
    assert!((0..=100).all(|i| Curve::Ease.at(i as f32 / 100.0) <= 1.0001), "ease-out does not overshoot");
    assert!((0..=100).any(|i| Curve::Spring.at(i as f32 / 100.0) > 1.0), "the spring overshoots");
}

#[test]
fn eliding_never_cuts_a_character_in_half() {
    use zikaron_ui::width::elide_chars;
    let s = "账本目录与披露包的落处都在这一条路上,读起来最好从中间省略";
    let got = elide_chars(s, 12);
    assert_eq!(got.chars().count(), 12);
    assert!(got.contains('…'));
    assert_eq!(elide_chars("短", 12), "短", "没超就一个字不动");
}

#[test]
fn a_page_gives_up_itself_to_hand_out_its_one_primary() {
    // "Exactly one" is carried by the type: `primary` consumes `self`, so a second cannot be written. This
    // pins that signature: changing `self` to `&self` fails here.
    let text = std::fs::read_to_string(src().join("page.rs")).expect("读不出 page.rs");
    let code = code_only(&text);
    assert!(
        code.contains("pub fn primary(self, ui: &mut egui::Ui"),
        "重点钮要吃掉这一页;签名一改,一页两个重点钮就写得出来了"
    );
    assert!(
        !code.contains("impl Sealed"),
        "交出重点钮之后剩下的东西不许再长出方法"
    );
}

#[test]
fn every_icon_is_drawn_here_and_none_is_borrowed() {
    use zikaron_ui::icons::{path, Icon};
    assert_eq!(Icon::ALL.len(), 7, "七个简单形状");
    for i in Icon::ALL {
        let p = path(i);
        assert!(!p.is_empty(), "{} 一笔也没有", i.as_str());
        assert!(
            p.iter().all(|s| s.len() >= 2),
            "{} 有一笔只有一个点:画不出线",
            i.as_str()
        );
        for s in p {
            for (x, y) in s {
                assert!((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y), "{} 画出了单位方框", i.as_str());
            }
        }
    }
}

#[test]
fn the_font_roles_name_their_faces_and_the_cjk_one_is_not_face_zero() {
    use zikaron_ui::fonts::{find, Place, Role, ROLES};
    assert_eq!(ROLES.len(), Role::ALL.len());
    // The `Place` column is part of the rule: lookup must follow it. One check per member, reading the face
    // lookup actually returned (`bytes` and `path`), not what the table says.
    let found = find();
    for (role, file, index, place) in ROLES {
        assert!(!file.is_empty(), "{} 没有档名", role.as_str());
        if role == Role::Cjk {
            // Face 0 is the Hong Kong glyph set; mainland readers need another.
            assert_ne!(index, 0, "中文那一面不许取第 0 面");
        }
        let face = found
            .face(role)
            .unwrap_or_else(|| panic!("{} 这一面寻取没给出来(缺员 {:?})", role.as_str(), found.missing));
        assert_eq!(face.place, place, "{} 取法与表里写的那一格不同", role.as_str());
        match place {
            // An embedded face's bytes travel with the binary; it has no path on disk.
            Place::Embedded => {
                assert!(face.bytes > 0, "{} 是内嵌的,字节数不许为零", role.as_str());
                assert!(face.path.is_none(), "{} 是内嵌的,不许带盘上那一条路", role.as_str());
            }
            // A system face's bytes are on disk at a path found on this machine (the path itself is not
            // compared).
            Place::System => {
                let path = face
                    .path
                    .as_ref()
                    .unwrap_or_else(|| panic!("{} 是系统取的,路不许没有", role.as_str()));
                assert!(!path.as_os_str().is_empty(), "{} 是系统取的,路不许是空串", role.as_str());
            }
        }
    }
}

#[test]
fn every_glyph_is_drawn_here_inside_its_box() {
    use zikaron_ui::icons::{glyph_path, Glyph};
    for g in Glyph::ALL {
        let p = glyph_path(g);
        assert!(!p.is_empty(), "{g:?} has no stroke");
        for s in p {
            assert!(s.len() >= 2, "{g:?} has a stroke that draws no line");
            for (x, y) in s {
                assert!((-0.001..=1.001).contains(&x) && (-0.001..=1.001).contains(&y), "{g:?} leaves its box");
            }
        }
    }
}

#[test]
fn only_the_system_taken_faces_have_a_fallback_and_the_strong_face_is_the_simplified_semibold() {
    use zikaron_ui::fonts::{embedded, Place, Role, FALLBACK, ROLES};
    // The fallback table follows `Place`: embedded faces are always present, so a fallback for them would be
    // dead code; system faces need one, because missing glyphs as boxes would be a silent failure.
    for (role, file, _, place) in ROLES {
        let has = FALLBACK.iter().any(|(r, _, _)| *r == role);
        match place {
            Place::Embedded => {
                assert!(embedded(file).is_some(), "{} 说自己是内嵌的,字节却不在本件里", role.as_str());
                assert!(!has, "{} 是内嵌的,不该有退路", role.as_str());
            }
            Place::System => {
                assert!(embedded(file).is_none(), "{} 是系统取的,字节不该随二进制走", role.as_str());
                assert!(has, "{} 是系统取的,翻不到那一形要有退路", role.as_str());
            }
        }
    }
    let strong = ROLES.iter().find(|(r, _, _, _)| *r == Role::Strong).expect("有重字");
    assert_eq!((strong.1, strong.2), ("PingFang.ttc", 11), "重字取苹方简体 Semibold 第 11 面");
}

#[test]
fn a_pen_gives_itself_up_to_press_its_one_commit_key() {
    let text = std::fs::read_to_string(src().join("page.rs")).expect("读不出 page.rs");
    let code = code_only(&text);
    assert!(
        code.contains("pub fn press(self, ui: &mut egui::Ui"),
        "落笔键要吃掉令牌;签名一改,一枚令牌就按得出两枚朱砂键"
    );
    assert!(code.contains("pub fn press_long(self, ui: &mut egui::Ui"), "the long commit key also consumes its token");
}

/// One key shape, six roles: every face is distinguishable, all one height; a disabled key takes no clicks.
#[test]
fn one_key_shape_six_roles() {
    use zikaron_ui::button::{face, primary_fill, commit_fill, Role};
    let _light = light();
    assert_eq!(hex(primary_fill()), "#3B82F6", "the primary key is the accent");
    assert_eq!(hex(commit_fill()), "#A83A2C", "the commit key is cinnabar");
    assert_eq!(hex(face(Role::Guide).text), "#A83A2C", "the guide key's words are cinnabar");
    assert_eq!(hex(face(Role::Guide).fill), "#FFFFFF", "the guide key is white");
    let faces: Vec<_> = Role::ALL.iter().map(|k| face(*k)).collect();
    for (i, a) in faces.iter().enumerate() {
        for b in &faces[..i] {
            assert!((a.fill, a.line, a.text) != (b.fill, b.line, b.text), "two roles look the same");
        }
    }
    // Keys are painted only by button::paint: no other file fills a block with the primary or cinnabar color.
    for (name, text) in files() {
        if name == "button.rs" || name == "palette.rs" || name == "page.rs" {
            continue;
        }
        for line in code_only(&text).lines().filter(|l| l.contains("rect")) {
            assert!(!line.contains("primary_fill()"), "{name} fills a block with the primary color itself: {line}");
            assert!(!line.contains("commit_fill()"), "{name} fills a block with cinnabar itself: {line}");
        }
    }
    use zikaron_ui::egui;
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let mut sizes = Vec::new();
    let _ = ctx.run(egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))), ..Default::default() }, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            for k in [Role::Primary, Role::Secondary, Role::Guide, Role::Commit] {
                sizes.push(zikaron_ui::button::key(ui, "键", k, true).rect.height());
            }
            let off = zikaron_ui::button::key(ui, "键", Role::Secondary, false);
            assert!(!off.sense.senses_click(), "a disabled key takes no clicks");
        });
    });
    assert!(sizes.iter().all(|h| *h == 34.0), "every role is one height: {sizes:?}");
}

/// The parts draw in a headless frame and read egui's own input for it: a dropped file reaches the drop zone.
#[test]
fn the_parts_draw_and_read_egui_input() {
    use zikaron_ui::egui;
    use zikaron_ui::{card, drop, kv, mark, seg, states, table, toggle};
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0))), ..Default::default() };
    input.dropped_files.push(egui::DroppedFile { path: Some("/tmp/a-dropped-file".into()), ..Default::default() });
    let mut got_drop = Vec::new();
    let out = ctx.run(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            seg::seg(ui, "s", &[seg::Cell::from("a"), seg::Cell::from("b")], 0);
            seg::tabs(ui, "t", &["a", "b"], 0);
            let mut on = true;
            toggle::toggle(ui, &mut on, "switch", "note", true);
            card::card(ui, |ui| kv::kv(ui, &[("k", kv::Val::text("v")), ("m", kv::Val::Mark(mark::Mark::Ok, "ok".into()))]));
            card::tile(ui, &card::Tile { title: "k", figure: "1", mark: None, figure_colour: None, sub: "sub", mono_figure: false, clickable: true });
            table::table(ui, "tb", &[table::SEQ, table::col("x", table::Col::Fr(1.0))], true, &[table::Row { cells: vec![table::Cell::Seq(1), table::Cell::Text("one".into())], click: true, ..Default::default() }], "none");
            states::checks(ui, &[(mark::Mark::Ok, "ok".into(), None), (mark::Mark::Bad, "bad".into(), Some("why".into()))], true);
            mark::pill(ui, "pill", zikaron_ui::palette::Tone::Ok);
            mark::tag(ui, "tag");
            got_drop = drop::zone(ui, "d", None, &["drop"], None, 120.0, drop::Shape::Column, true).dropped;
        });
    });
    assert!(out.shapes.len() > 20, "too few shapes in a frame of parts: {}", out.shapes.len());
    assert_eq!(got_drop, vec!["/tmp/a-dropped-file".to_string()], "the drop zone reads egui's dropped files");
}

/// Toasts: one at a time; the same sentence again stays one toast and restarts its time; another replaces it;
/// an error with raw words opens its details and then stays until closed.
#[test]
fn a_toast_is_one_at_a_time_and_a_failing_one_stays_open_with_its_raw_words() {
    use zikaron_ui::egui;
    use zikaron_ui::toast::{Toasts, Tone};
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let mut t = Toasts::new();
    t.set_labels("详情", "收起详情", "关闭");
    let run = |ctx: &egui::Context, t: &mut Toasts, time: f64, events: Vec<egui::Event>| {
        let mut rects = Vec::new();
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 800.0))), time: Some(time), events, ..Default::default() };
        let _ = ctx.run(input, |ctx| rects = t.draw(ctx, 216.0).rects);
        rects
    };
    t.say("已上链 1 条", Tone::Note, 0.0);
    t.say("已上链 1 条", Tone::Note, 2.0);
    assert_eq!(t.live(), 1, "the same sentence twice is one toast");
    let _ = run(&ctx, &mut t, 4.0, Vec::new());
    assert_eq!(t.showing(), Some("已上链 1 条"), "saying it again restarted its time");
    t.say_full("配的节点一处也连不上。", "检查网络,或在设置里换一处节点。", "UNREACHABLE · http://127.0.0.1:9", Tone::Bad, 4.1);
    assert_eq!(t.showing(), Some("配的节点一处也连不上。"), "another sentence replaces the one on screen");
    let r = run(&ctx, &mut t, 4.5, Vec::new());
    let bad = r[0];
    assert!((bad.center().x - (216.0 + 1180.0) / 2.0).abs() < 1.5, "centered over the page area: {bad:?}");
    // Open the details: the second-to-last key of the error row (details, then close).
    let click = |p: egui::Pos2| {
        vec![
            egui::Event::PointerMoved(p),
            egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() },
            egui::Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() },
        ]
    };
    let mut opened = bad;
    // Frame time only moves forward.
    let mut at = 4.5;
    let mut next = || {
        at += 0.01;
        at
    };
    for x in (bad.left() as i32..bad.right() as i32).rev().step_by(6) {
        let p = egui::pos2(x as f32, bad.center().y);
        let _ = run(&ctx, &mut t, next(), vec![egui::Event::PointerMoved(p)]);
        let _ = run(&ctx, &mut t, next(), click(p));
        let now = run(&ctx, &mut t, next(), Vec::new());
        if t.live() == 0 {
            // That was "close": say it again and keep looking left of it.
            let when = next();
            t.say_full("配的节点一处也连不上。", "检查网络,或在设置里换一处节点。", "UNREACHABLE · http://127.0.0.1:9", Tone::Bad, when);
            continue;
        }
        if now.first().map(|r| r.height() > bad.height() + 1.0).unwrap_or(false) {
            opened = now[0];
            break;
        }
    }
    assert!(opened.height() > bad.height(), "opening the details shows the raw words");
    let later = run(&ctx, &mut t, 60.0, Vec::new());
    assert_eq!(later.len(), 1, "an error with its details open stays past its time");
    assert_eq!(t.live(), 1);
}
