//! The first-run wizard's step card holds its own room (`covers::pane`), read in windowless frames: every part
//! drawn in the card that asks whether it lies on top (`layer::on_top`) finds the card's layer there, and the
//! cover's column beside the card stays the cover's. Victim of each: the person setting up a new machine, whose
//! passcode cells take no key, whose card rows never light and whose column of steps stops answering.

use zikaron_ui::egui::{self, pos2, vec2, Event, PointerButton, Pos2, Rect};
use zikaron_ui::{covers, flow, layer, material, pin, secret::Secret, sheetparts};

const SCREEN: Rect = Rect { min: pos2(0.0, 0.0), max: pos2(1180.0, 760.0) };
/// The card, right of the column, as the wizard lays it out.
const CARD: Rect = Rect { min: pos2(380.0, 80.0), max: pos2(1100.0, 680.0) };
/// A part of the cover's column (the steps, the language, the way out live there).
const COLUMN: Rect = Rect { min: pos2(40.0, 300.0), max: pos2(300.0, 340.0) };

/// What the parts saw in one frame.
#[derive(Default, Clone, Copy)]
struct Seen {
    /// `on_top` of the sheet passcode row's rectangle, asked in the card.
    pins_top: bool,
    /// `on_top` of the kit passcode row's rectangle, asked in the card.
    row_top: bool,
    /// The light under the pointer on a card row.
    glow: f32,
    /// A card row lit under the pointer.
    lit: bool,
    /// `on_top` of the column part, asked in the cover.
    column_top: bool,
}

struct Wizard {
    ctx: egui::Context,
    t: f64,
    pins: Secret,
    row: Secret,
}

impl Wizard {
    fn new() -> Wizard {
        let ctx = egui::Context::default();
        let _ = zikaron_ui::skin::dress(&ctx);
        Wizard { ctx, t: 0.0, pins: Secret::default(), row: Secret::default() }
    }

    /// One frame with `events`: the cover with its column part, and over it the step card holding the sheet
    /// passcode row (`which` 0), the kit passcode row (`which` 1), and a row lit and glowing under the pointer.
    fn frame(&mut self, events: Vec<Event>, which: usize) -> Seen {
        self.t += 0.1;
        let input = egui::RawInput { screen_rect: Some(SCREEN), time: Some(self.t), events, ..Default::default() };
        let mut seen = Seen::default();
        let (pins, row) = (&mut self.pins, &mut self.row);
        let _ = self.ctx.run_ui(input, |root| {
            let ctx = root.ctx().clone();
            let (_, over) = covers::cover(&ctx, "pane-on-top", covers::Ground::Clear, |ui| {
                seen.column_top = layer::on_top(ui, COLUMN);
            });
            covers::pane(&ctx, over, 0, 1.0, CARD, |ui| {
                ui.add_space(60.0);
                if which == 0 {
                    let r = ui.horizontal(|ui| sheetparts::pins(ui, "pane-pins", pins, 8, None, true)).response.rect;
                    seen.pins_top = layer::on_top(ui, r);
                } else {
                    let r = ui.horizontal(|ui| pin::pin_row(ui, "pane-row", row, 8, None, true, false)).response.rect;
                    seen.row_top = layer::on_top(ui, r);
                }
                ui.add_space(40.0);
                let (rect, _) = ui.allocate_exact_size(vec2(400.0, 48.0), egui::Sense::hover());
                seen.glow = material::glow_at(ui, egui::Id::new("pane-glow"), rect, true).0;
                seen.lit = flow::pointer_on(ui, rect);
            });
            layer::fade_out(&ctx);
        });
        seen
    }
}

fn moved(p: Pos2) -> Vec<Event> {
    vec![Event::PointerMoved(p)]
}

/// Where the lit row lies: under the passcode row, 60 + 44 + 40 down the card (the kit row is a little taller,
/// and the row's middle is inside either way).
fn row_point() -> Pos2 {
    pos2(CARD.left() + 200.0, CARD.top() + 60.0 + 54.0 + 40.0 + 24.0)
}

/// The sheet passcode row in the step card lies on top and takes what is typed (the wizard's "set a passcode"
/// step, typed twice).
#[test]
fn the_wizards_sheet_passcode_cells_take_keys_in_the_step_card() {
    let mut w = Wizard::new();
    let mut seen = Seen::default();
    for _ in 0..4 {
        seen = w.frame(vec![], 0);
    }
    assert!(seen.pins_top, "the sheet passcode row lies on top in the step card");
    let _ = w.frame(vec![Event::Text("ab3".into())], 0);
    let _ = w.frame(vec![], 0);
    assert_eq!(w.pins.chars(), 3, "what is typed lands in the cells");
}

/// The kit passcode row in the step card lies on top and takes what is typed.
#[test]
fn the_kit_passcode_row_takes_keys_in_the_step_card() {
    let mut w = Wizard::new();
    let mut seen = Seen::default();
    for _ in 0..4 {
        seen = w.frame(vec![], 1);
    }
    assert!(seen.row_top, "the kit passcode row lies on top in the step card");
    let _ = w.frame(vec![Event::Text("7k2".into())], 1);
    let _ = w.frame(vec![], 1);
    assert_eq!(w.row.chars(), 3, "what is typed lands in the row");
}

/// The light under the pointer shows on a row of the step card (`material::glow_at`).
#[test]
fn the_light_follows_the_pointer_in_the_step_card() {
    let mut w = Wizard::new();
    let mut seen = Seen::default();
    for _ in 0..6 {
        seen = w.frame(moved(row_point()), 0);
    }
    assert!(seen.glow > 0.5, "the row glows under the pointer: {}", seen.glow);
}

/// A row of the step card lights under the pointer (`flow::pointer_on`).
#[test]
fn a_row_lights_under_the_pointer_in_the_step_card() {
    let mut w = Wizard::new();
    let mut seen = Seen::default();
    for _ in 0..3 {
        seen = w.frame(moved(row_point()), 0);
    }
    assert!(seen.lit, "the row under the pointer is lit");
}

/// The cover's column beside the card stays the cover's: a part there lies on top, the pointer over it and pressed.
#[test]
fn the_covers_column_beside_the_card_stays_on_top() {
    let mut w = Wizard::new();
    let mut seen = Seen::default();
    for _ in 0..3 {
        seen = w.frame(moved(COLUMN.center()), 0);
    }
    assert!(seen.column_top, "the column part lies on top beside the card");
    let p = COLUMN.center();
    let press = vec![
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Default::default() },
        Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Default::default() },
    ];
    let seen = w.frame(press, 0);
    assert!(seen.column_top, "still on top under a press");
}
