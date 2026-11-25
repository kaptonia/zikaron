use app::action::{apply, Action, Applied};
use zikaron_ui::egui;

const W: f32 = 1180.0;
const H: f32 = 760.0;

const MUST: &[&str] = &[
    "persona-consent",
    "cli-pass",
    "persona-name",
    "persona-account",
    "persona-key",
    "persona-release",
    "persona-home",
    "id-primary",
    "id-pin",
    "id-backup",
    "id-delete",
    "bk-export",
    "bk-restore",
    "bk-new-pin",
    "cli-alone",
];

#[test]
fn every_sheet_is_as_tall_as_what_it_holds_and_shows_no_scroll_bar() {
    if super::alone_in(module_path!(), "every_sheet_is_as_tall_as_what_it_holds_and_shows_no_scroll_bar") {
        return;
    }
    super::vault_open();
    let ctx = egui::Context::default();
    let mut s = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let dir = std::env::temp_dir().join(format!("zk-r34n10-sheets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    super::seated(&mut s, &dir);
    answers!(apply(&mut s, Action::OpenHome { root: dir.display().to_string() }), Applied::Homed { .. });
    answers!(apply(&mut s, Action::Genesis { statement: "sheets".into() }), Applied::Genesised { .. });
    let _ = apply(&mut s, Action::ReadIdentities);
    let mut seen: Vec<String> = Vec::new();
    let mut wrong: Vec<String> = Vec::new();
    let mut grey: Vec<String> = Vec::new();
    let mut cut: Vec<String> = Vec::new();
    let mut feet: Vec<String> = Vec::new();
    let meant_short = |t: &str, hovered: &[String]| {
        let body = t.strip_prefix("zikaron:").unwrap_or(t);
        let place = t.starts_with('/') || t.starts_with('~') || t.contains(":\\");
        body.strip_prefix("0x").is_some_and(|h| !h.is_empty() && h.bytes().all(|b| b.is_ascii_hexdigit())) || (place && hovered.iter().any(|h| h == t))
    };
    for lang in app::lang::Lang::ALL {
    app::lang::set(lang);
    for role in [app::roles::Role::Author, app::roles::Role::Grantee] {
        s.settings.role = role;
        for sc in app::window::scenes(role) {
            let feet_before = zikaron_ui::probe::feet_out().len();
            let (back, g) = app::window::probe_geometry(&ctx, s, sc, W, H);
            s = back;
            for o in zikaron_ui::probe::feet_out().into_iter().skip(feet_before) {
                feet.push(format!("{lang:?} {}: {} keys laid {:?} in a row {:?}", sc.name(), o.id, o.laid, o.row));
            }
            for (on_sheet, text) in &g.grey_keys {
                if *on_sheet {
                    grey.push(format!("{lang:?} {}: {text}", sc.name()));
                }
            }
            for (on_sheet, text) in &g.cut_short {
                if !meant_short(text, &g.whole_on_hover) {
                    cut.push(format!("{lang:?} {} ({}): {text}", sc.name(), if *on_sheet { "sheet" } else { "page" }));
                }
            }
            for sh in &g.sheets {
                let id = sh.id.clone();
                if !seen.contains(&id) {
                    seen.push(id.clone());
                }
                if !sh.content.is_finite() || sh.content > sh.room || sh.bar || (sh.card.height() - sh.content).abs() > 1.0 {
                    wrong.push(format!("{} ({}): card {} high, content {}, room {}, bar {}", id, sc.name(), sh.card.height(), sh.content, sh.room, sh.bar));
                }
            }
        }
    }
    }
    assert!(wrong.is_empty(), "sheets not as tall as their content, or scrolling: {wrong:#?}");
    assert!(grey.is_empty(), "a sheet's key grey with no why: {grey:#?}");
    assert!(cut.is_empty(), "words cut short: {cut:#?}");
    assert!(feet.is_empty(), "a sheet's keys laid past its row of keys: {feet:#?}");
    for m in MUST {
        assert!(seen.iter().any(|x| x == m), "{m} read (seen: {seen:?})");
    }
}

fn sheets_shell(ctx: &egui::Context, tag: &str) -> app::shell::Shell {
    super::vault_open();
    let mut s = app::shell::Shell::boot(zikaron_ui::skin::dress(ctx));
    let dir = std::env::temp_dir().join(format!("zk-r34n10-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    super::seated(&mut s, &dir);
    answers!(apply(&mut s, Action::OpenHome { root: dir.display().to_string() }), Applied::Homed { .. });
    answers!(apply(&mut s, Action::Genesis { statement: "sheets".into() }), Applied::Genesised { .. });
    let _ = apply(&mut s, Action::ReadIdentities);
    s.settings.chain_id = Some(11_155_111);
    s.settings.registry = app::key::Address::parse("0xc29410b882c4c3b77e33659d2f06ac563e7b08a3");
    s
}

fn text_at(g: &app::window::Geometry, words: &str) -> Option<egui::Rect> {
    g.texts.iter().find(|(t, _)| t == words).map(|(_, r)| *r)
}

#[test]
fn a_lone_value_on_a_sheet_row_stands_at_its_left_edge() {
    if super::alone_in(module_path!(), "a_lone_value_on_a_sheet_row_stands_at_its_left_edge") {
        return;
    }
    let ctx = egui::Context::default();
    let mut s = sheets_shell(&ctx, "lone");
    let mut seen = 0;
    for lang in app::lang::Lang::ALL {
        app::lang::set(lang);
        for (scene, title, value) in [
            ("PersonaHome", app::lang::t(app::lang::Key::PsOpen).to_string(), app::readnets::chain_name(11_155_111, None)),
            ("PersonaRelease", app::lang::t(app::lang::Key::FaceSummaryRelease).to_string(), "github \u{b7} geometry".to_string()),
            ("PersonaReleaseLong", app::lang::t(app::lang::Key::FaceSummaryRelease).to_string(), app::window::LONG_RELEASE.to_string()),
        ] {
            let Some(sc) = app::window::scenes(app::roles::Role::Author).into_iter().find(|sc| sc.name().starts_with(&format!("sheet {scene} "))) else { panic!("{scene} among the scenes") };
            let (back, g) = app::window::probe_geometry(&ctx, s, sc, W, H);
            s = back;
            let card = g.sheets.iter().map(|sh| sh.card).last().unwrap_or_else(|| panic!("{lang:?} {scene}: a sheet drawn"));
            let on_card = |words: &str| g.texts.iter().find(|(t, r)| t == words && card.contains(r.center())).map(|(_, r)| *r);
            let (Some(t), Some(v)) = (on_card(&title), on_card(&value)) else { panic!("{lang:?} {scene}: title {title:?} and value {value:?} drawn on the sheet: {:?}", g.texts.iter().map(|(t, _)| t).collect::<Vec<_>>()) };
            assert!((v.left() - t.left()).abs() <= 1.0, "{lang:?} {scene}: the value at the left edge ({}), level with the title ({})", v.left(), t.left());
            if scene == "PersonaReleaseLong" {
                assert!(v.height() > 30.0, "{lang:?} {scene}: the long value wrapped over its lines ({})", v.height());
                assert!(v.right() <= card.right(), "{lang:?} {scene}: the long value within the card ({} ≤ {})", v.right(), card.right());
            }
            seen += 1;
        }
    }
    assert_eq!(seen, 6);
}

#[test]
fn a_one_sentence_row_on_a_card_stands_whole_over_its_lines() {
    if super::alone_in(module_path!(), "a_one_sentence_row_on_a_card_stands_whole_over_its_lines") {
        return;
    }
    let ctx = egui::Context::default();
    let mut s = sheets_shell(&ctx, "sentence");
    let dir = std::env::temp_dir().join(format!("zk-r34n10-sentence-file-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a folder");
    let file = dir.join("A Record Whose File Name Runs On Long Enough That The Send Card Line Cannot Hold It On One Line.txt");
    std::fs::write(&file, b"sentence").expect("a file");
    let got = app::action::apply_settled(&mut s, Action::RecordWork { note_md: String::new(), files: vec![file.display().to_string()], for_: None });
    let record = match &got {
        Applied::RecordedBatch { ids, stopped: None, .. } if ids.len() == 1 => ids[0].clone(),
        _ => panic!("the long-named record made: {got:?}"),
    };
    let queued: Vec<String> = s.queue.items.iter().map(|q| q.id.clone()).collect();
    assert_eq!(queued.last(), Some(&record), "the record last in the queue: {queued:?}");
    answers!(app::action::apply_settled(&mut s, Action::ReadLedger), Applied::Started(app::task::Kind::Ledger));
    let mut seen = 0;
    for lang in app::lang::Lang::ALL {
        app::lang::set(lang);
        let head = app::lang::t(app::lang::Key::PsSendThen).split("{0}").next().unwrap_or("").to_string();
        let title = app::lang::t(app::lang::Key::U3SendTitle).to_string();
        let Some(sc) = app::window::scenes(app::roles::Role::Author).into_iter().find(|sc| sc.name().starts_with("sheet SendFollows ")) else { panic!("SendFollows among the scenes") };
        let (back, g) = app::window::probe_geometry(&ctx, s, sc, W, H);
        s = back;
        let card = g.sheets.iter().map(|sh| sh.card).last().unwrap_or_else(|| panic!("{lang:?}: a card drawn"));
        let named = s.rows.as_ref().is_some_and(|(rows, _)| rows.iter().any(|r| r.id == record));
        assert!(named && s.queue.items.last().map(|q| q.id.as_str()) == Some(record.as_str()), "{lang:?}: the card drawn with the record last in the queue and in the table read: queue {:?}, table {:?}", s.queue.items.iter().map(|q| q.id.clone()).collect::<Vec<_>>(), s.rows.as_ref().map(|(rows, _)| rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>()));
        let on_card = |pick: &dyn Fn(&str) -> bool| g.texts.iter().find(|(t, r)| pick(t) && card.contains(r.center())).cloned();
        let Some((line, v)) = on_card(&|t: &str| t.starts_with(&head)) else { panic!("{lang:?}: the next-sends line drawn: {:?}", g.texts.iter().map(|(t, _)| t).collect::<Vec<_>>()) };
        let Some((_, t)) = on_card(&|x: &str| x == title) else { panic!("{lang:?}: the card's title drawn") };
        assert!(line.contains("Cannot Hold It On One Line") && !line.contains('\u{2026}'), "{lang:?}: the line whole, not cut short: {line:?}");
        assert!(v.height() > 30.0, "{lang:?}: the line over its lines ({})", v.height());
        assert!((v.left() - t.left()).abs() <= 1.0 && v.right() <= card.right(), "{lang:?}: at the card's left edge ({} / {}), within it", v.left(), t.left());
        seen += 1;
    }
    assert_eq!(seen, 2);
}

#[test]
fn a_form_cards_last_fold_stands_off_its_keys_as_its_fields_stand_off_each_other() {
    if super::alone_in(module_path!(), "a_form_cards_last_fold_stands_off_its_keys_as_its_fields_stand_off_each_other") {
        return;
    }
    let ctx = egui::Context::default();
    let mut s = sheets_shell(&ctx, "fold");
    let mut seen = 0;
    for lang in app::lang::Lang::ALL {
        app::lang::set(lang);
        for role in [app::roles::Role::Author, app::roles::Role::Grantee] {
            s.settings.role = role;
            let sc = app::window::Scene::Page(app::nav::Place::View(app::nav::View::Verify, app::nav::tab::VERIFY_OTHERS));
            let (back, g) = app::window::probe_geometry(&ctx, s, sc, W, H);
            s = back;
            let more = text_at(&g, app::lang::t(app::lang::Key::U3MoreOptions)).expect("the fold drawn");
            let read = text_at(&g, app::lang::t(app::lang::Key::U3ReadIt)).expect("the key drawn");
            let fold = g.rows.iter().map(|(_, r)| *r).find(|r| r.contains(more.center())).expect("the fold's row");
            let key = g.widgets.iter().map(|w| w.rect).filter(|r| r.contains(read.center()) && (r.height() - zikaron_ui::tokens::KEY_H).abs() < 0.5).last().expect("the key's rectangle");
            let words_bottom = fold.center().y + 21.0 / 2.0;
            let gap = key.top() - words_bottom;
            assert!((gap - 20.0).abs() <= 0.5, "{lang:?} {role:?}: the fold's words to the keys {gap}, the stack's gap 20");
            seen += 1;
        }
    }
    assert_eq!(seen, 4);
}
