//! What an input method is still composing never enters a field's value (`compose::around`), read in windowless
//! frames for each of the kit's text boxes: the single-line field and the multi-line area, the joined and the small
//! field, and the two kinds of number cells. Victim: whoever types with a Chinese or Japanese input method, the
//! letters mid-composition read by a filter, a key that submits or a save, and kept on the chain where it is too
//! late to change them.

use std::path::PathBuf;
use zikaron_ui::egui::{self, pos2, Event, ImeEvent, Rect};
use zikaron_ui::{cells, fields, input, lasts};

const SCREEN: Rect = Rect { min: pos2(0.0, 0.0), max: pos2(900.0, 600.0) };

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Line,
    Area,
    Joined,
    Small,
    Cells,
    Lasts,
}

const KINDS: [Kind; 6] = [Kind::Line, Kind::Area, Kind::Joined, Kind::Small, Kind::Cells, Kind::Lasts];

struct Bench {
    ctx: egui::Context,
    t: f64,
    kind: Kind,
    value: Vec<String>,
}

impl Bench {
    /// A box of `kind` holding `start`.
    fn new(kind: Kind, start: &str) -> Bench {
        let ctx = egui::Context::default();
        let _ = zikaron_ui::skin::dress(&ctx);
        Bench { ctx, t: 0.0, kind, value: vec![start.to_string()] }
    }

    /// One frame with `events`; the box is asked to take the focus when `focus`, and the focus is let go when not.
    fn frame(&mut self, events: Vec<Event>, focus: bool) {
        self.t += 0.1;
        let input = egui::RawInput { screen_rect: Some(SCREEN), time: Some(self.t), events, ..Default::default() };
        let (kind, value) = (self.kind, &mut self.value);
        let _ = self.ctx.run_ui(input, |root| {
            egui::CentralPanel::default().show_inside(root, |ui| {
                let id = match kind {
                    Kind::Line => Some(input::line(ui, &mut value[0], "名称").id),
                    Kind::Area => Some(input::area_words(ui, &mut value[0], 3, "说明").id),
                    Kind::Joined => Some(fields::line(ui, &mut value[0], "搜索", 400.0, fields::Look::default()).id),
                    Kind::Small => Some(fields::small(ui, &mut value[0], "数", 120.0, false).id),
                    Kind::Cells => Some(cells::cells(ui, "compose", value, &["天"], 48.0, "0", &[false], true)[0].id),
                    Kind::Lasts => {
                        lasts::boxed(ui, "compose", value, &["天"], "0", &[false], true);
                        Some(egui::Id::new(("span-box", "compose")).with(0usize))
                    }
                };
                if let Some(id) = id {
                    if focus {
                        ui.memory_mut(|m| m.request_focus(id));
                    } else {
                        ui.memory_mut(|m| m.surrender_focus(id));
                    }
                }
            });
        });
    }

    fn value(&self) -> &str {
        &self.value[0]
    }
}

fn preedit(s: &str) -> Event {
    Event::Ime(ImeEvent::Preedit(s.into()))
}

fn commit(s: &str) -> Event {
    Event::Ime(ImeEvent::Commit(s.into()))
}

fn enter() -> Event {
    Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }
}

/// The words composed and the one committed for a box: number cells take digits only, so their method composes
/// toward a digit.
fn words(kind: Kind) -> (&'static str, &'static str, &'static str, &'static str) {
    match kind {
        Kind::Cells | Kind::Lasts => ("12", "yi", "yiq", "7"),
        _ => ("ab", "ni", "nih", "你"),
    }
}

/// While the method composes, the value holds none of its letters (in the frame it composes, in a frame with no
/// word from it, as it composes more); the committed character is put in and stays.
#[test]
fn a_box_holds_only_what_was_committed() {
    for kind in KINDS {
        let (start, first, more, chosen) = words(kind);
        let mut b = Bench::new(kind, start);
        b.frame(vec![], true);
        b.frame(vec![], true);
        b.frame(vec![Event::Ime(ImeEvent::Enabled), preedit(first)], true);
        assert_eq!(b.value(), start, "{kind:?}: the letters being composed are not in the value");
        b.frame(vec![], true);
        assert_eq!(b.value(), start, "{kind:?}: nor in a frame with no word from the method");
        b.frame(vec![preedit(more)], true);
        assert_eq!(b.value(), start, "{kind:?}: nor as it composes more");
        b.frame(vec![commit(chosen), Event::Ime(ImeEvent::Disabled)], true);
        let v = b.value().to_string();
        assert_eq!((v.matches(chosen).count(), v.chars().count()), (1, start.chars().count() + chosen.chars().count()), "{kind:?}: the committed character is put in: {v}");
        assert!(!v.contains('y') && !v.contains('n'), "{kind:?}: and no letter of the composing: {v}");
        b.frame(vec![], true);
        assert_eq!(b.value(), v, "{kind:?}: and stays");
    }
}

/// A key that submits pressed while the method composes (in the same frame, or the frame after), and the focus
/// taken away mid-composition: the value holds none of the letters, then or after.
#[test]
fn submitting_or_leaving_mid_composition_keeps_the_letters_out() {
    for kind in KINDS {
        let (start, first, more, _) = words(kind);
        let clean = |v: &str| !v.contains('y') && !v.contains('n') && !v.contains('i') && !v.contains('h');
        // Composing, then the submitting key in the next frame.
        let mut b = Bench::new(kind, start);
        b.frame(vec![], true);
        b.frame(vec![], true);
        b.frame(vec![Event::Ime(ImeEvent::Enabled), preedit(first)], true);
        b.frame(vec![enter()], true);
        assert!(clean(b.value()), "{kind:?}: the submitting key takes no letter: {}", b.value());
        b.frame(vec![], false);
        b.frame(vec![], false);
        assert!(clean(b.value()), "{kind:?}: nor later: {}", b.value());
        // Composing and the submitting key in one frame.
        let mut b = Bench::new(kind, start);
        b.frame(vec![], true);
        b.frame(vec![], true);
        b.frame(vec![Event::Ime(ImeEvent::Enabled), preedit(more), enter()], true);
        assert!(clean(b.value()), "{kind:?}: composing and submitting in one frame: {}", b.value());
        // Composing, then the focus taken away.
        let mut b = Bench::new(kind, start);
        b.frame(vec![], true);
        b.frame(vec![], true);
        b.frame(vec![Event::Ime(ImeEvent::Enabled), preedit(more)], true);
        b.frame(vec![], false);
        assert_eq!(b.value(), start, "{kind:?}: leaving mid-composition leaves the value as it was");
        b.frame(vec![], false);
        b.frame(vec![], false);
        assert_eq!(b.value(), start, "{kind:?}: and it stays so");
    }
}

/// The method's words are the focused box's alone: another box whose value matches the words keeps its value.
#[test]
fn a_box_without_the_focus_keeps_its_value_while_another_composes() {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let (mut a, mut b) = (String::from("xy"), String::from("nb"));
    let mut t = 0.0;
    let mut frame = |events: Vec<Event>, a: &mut String, b: &mut String| {
        t += 0.1;
        let raw = egui::RawInput { events, screen_rect: Some(SCREEN), time: Some(t), ..Default::default() };
        let _ = ctx.run_ui(raw, |root| {
            egui::CentralPanel::default().show_inside(root, |ui| {
                let ra = input::line(ui, a, "甲");
                let _ = input::line(ui, b, "乙");
                ra.request_focus();
            });
        });
    };
    frame(vec![], &mut a, &mut b);
    frame(vec![], &mut a, &mut b);
    frame(vec![Event::Ime(ImeEvent::Enabled), preedit("n")], &mut a, &mut b);
    frame(vec![preedit("nb")], &mut a, &mut b);
    assert_eq!(b, "nb", "the box without the focus keeps its value");
    assert_eq!(a, "xy", "the focused box holds none of the letters");
}

/// Every text box of the kit is drawn through the one entrance: in the kit's source, each text edit made is made
/// inside `compose::around`, except the masked field (which takes no input method). A box added later without it
/// is named here.
#[test]
fn every_text_box_of_the_kit_draws_through_the_one_entrance() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut stray = Vec::new();
    let mut seen = 0;
    for e in std::fs::read_dir(&src).expect("the kit's source") {
        let path = e.expect("an entry").path();
        if path.extension().and_then(|x| x.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("a source file");
        let lines: Vec<&str> = text.lines().collect();
        for (i, l) in lines.iter().enumerate() {
            if !(l.contains("TextEdit::singleline(") || l.contains("TextEdit::multiline(")) {
                continue;
            }
            seen += 1;
            let before = &lines[i.saturating_sub(4)..i];
            let after = &lines[i..(i + 12).min(lines.len())];
            // The masked field's mark, spelled here in two halves (it is the one place's own word).
            let mark = format!(".{}(true)", "password");
            let masked = after.iter().any(|x| x.contains(&mark));
            let entered = before.iter().any(|x| x.contains("compose::around("));
            if !masked && !entered {
                stray.push(format!("{}:{}", path.file_name().and_then(|x| x.to_str()).unwrap_or(""), i + 1));
            }
        }
    }
    assert!(seen >= 7, "the kit's text edits found: {seen}");
    assert!(stray.is_empty(), "text boxes drawn around the entrance: {stray:?}");
}
