//! The window shell's navigation knows no product's pages: a page table of two pages and one rail group,
//! written here and nowhere in the shell, is drawn through the same rail list and page area, moved through by
//! pressing what they draw (into a page, back with the toolbar's key, the rail's item from a pushed page), and
//! read through the same probes.

use zikaron_ui::egui::{self, pos2, Event, PointerButton, Pos2, Rect};
use zikaron_ui::icons::Glyph;
use zikaron_ui::nav::{self, Frame, Group, History, Item, Pages};
use zikaron_ui::{probe, tokens};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Place {
    Shelf,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum Route {
    Shelf,
    Book,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
struct Only;

#[derive(Clone, Copy)]
enum Word {
    Reading,
    Shelf,
}

const RAIL: [Group<Place, Word>; 1] = [Group { title: Some(Word::Reading), items: &[Item { place: Place::Shelf, name: Word::Shelf, glyph: Glyph::Ledger }] }];

/// The outside product: its table, and the shell's state over it.
#[derive(Default)]
struct Outside {
    frame: Frame<Place, Route, Only>,
    open: bool,
    /// How many times the product's part of a landing ran.
    landings: usize,
}

impl Pages for Outside {
    type Place = Place;
    type Route = Route;
    type Stack = Only;
    type Word = Word;

    fn frame(&self) -> &Frame<Place, Route, Only> {
        &self.frame
    }
    fn frame_mut(&mut self) -> &mut Frame<Place, Route, Only> {
        &mut self.frame
    }
    fn word(&self, w: Word) -> String {
        match w {
            Word::Reading => "Reading",
            Word::Shelf => "Shelf",
        }
        .to_string()
    }
    fn rail(&self) -> &'static [Group<Place, Word>] {
        &RAIL
    }
    fn home(&self) -> Place {
        Place::Shelf
    }
    fn stack_of(&self, _place: Place) -> Only {
        Only
    }
    fn root_of(&self, _stack: Only) -> Place {
        Place::Shelf
    }
    fn history_of(&self, _place: Place) -> (Only, History<Route>) {
        (Only, History::at(Route::Shelf))
    }
    fn root(&self, _place: Place) -> Route {
        Route::Shelf
    }
    fn root_place(&self, route: &Route) -> Option<Place> {
        match route {
            Route::Shelf => Some(Place::Shelf),
            Route::Book => None,
        }
    }
    fn lit(&self) -> Option<Place> {
        self.frame.place
    }
    fn name(&self, route: &Route) -> String {
        format!("{route:?}")
    }
    fn detail(&self, route: &Route) -> bool {
        *route == Route::Book
    }
    fn title(&self, route: &Route) -> String {
        match route {
            Route::Shelf => "The shelf".into(),
            Route::Book => "A book".into(),
        }
    }
    fn toolbar(&mut self, _ui: &mut egui::Ui, _route: &Route, _now: f64) {}
    fn entered(&mut self, _now: f64) {
        self.landings += 1;
    }
    fn body(&mut self, ui: &mut egui::Ui, route: &Route, _now: f64) {
        match route {
            Route::Shelf => self.open = ui.button("Open the book").clicked(),
            Route::Book => {
                ui.label("Its pages");
            }
        }
    }
}

const RAIL_W: f32 = 220.0;

struct Bench {
    ctx: egui::Context,
    t: f64,
    o: Outside,
}

impl Bench {
    fn new() -> Bench {
        let ctx = egui::Context::default();
        let _ = zikaron_ui::skin::dress(&ctx);
        Bench { ctx, t: 0.0, o: Outside::default() }
    }

    /// One frame of the shell over the outside table: the rail's list and the page area; a press on the shelf's
    /// key goes into the book.
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.t += 0.1;
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(1180.0, 800.0))), time: Some(self.t), events, ..Default::default() };
        let (o, now) = (&mut self.o, self.t);
        self.ctx.run_ui(input, |root| {
            let ctx = &root.ctx().clone();
            probe::begin(ctx);
            let mut picked = None;
            egui::Panel::left("rail").exact_size(RAIL_W).resizable(false).frame(egui::Frame::NONE).show_inside(root, |ui| {
                picked = nav::rail_list(ui, &*o, 400.0);
            });
            egui::CentralPanel::default().frame(egui::Frame::NONE).show_inside(root, |ui| {
                nav::page_area(ui, o, ("Back", "Forward"), now);
            });
            if let Some(p) = picked {
                nav::rail_to(o, p, now);
            }
            if std::mem::take(&mut o.open) {
                nav::push(o, Route::Book, now);
            }
        })
    }

    fn settle(&mut self) -> egui::FullOutput {
        let mut out = self.frame(Vec::new());
        for _ in 0..4 {
            out = self.frame(Vec::new());
        }
        out
    }

    fn press(&mut self, at: Pos2) {
        let p = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Default::default() };
        let _ = self.frame(vec![Event::PointerMoved(at)]);
        let _ = self.frame(vec![p(true), p(false)]);
        let _ = self.frame(vec![Event::PointerGone]);
    }

    /// The route drawn and whether back and forward were shown.
    fn said(&self) -> (String, bool, bool) {
        let (b, f) = probe::history_shown(&self.ctx);
        (probe::route_drawn(&self.ctx), b, f)
    }
}

fn text_at(out: &egui::FullOutput, text: &str, left_of: Option<f32>) -> Option<Pos2> {
    fn walk(s: &egui::Shape, text: &str, out: &mut Vec<Rect>) {
        match s {
            egui::Shape::Text(t) if t.galley.text() == text => out.push(t.galley.rect.translate(t.pos.to_vec2())),
            egui::Shape::Vec(v) => v.iter().for_each(|y| walk(y, text, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    out.shapes.iter().for_each(|cs| walk(&cs.shape, text, &mut found));
    found.into_iter().find(|r| left_of.map(|x| r.center().x < x).unwrap_or(true)).map(|r| r.center())
}

#[test]
fn an_outside_page_table_is_drawn_moved_through_and_backed_out_of_by_the_same_shell() {
    let mut b = Bench::new();
    let out = b.settle();
    assert_eq!(b.said(), ("Shelf".into(), false, false), "the table's own page stands first");
    assert!(text_at(&out, "Shelf", Some(RAIL_W)).is_some(), "its rail item is drawn on the rail");
    assert!(text_at(&out, "Reading", Some(RAIL_W)).is_none(), "the first group's title is not drawn");
    assert!(text_at(&out, "The shelf", None).is_some(), "its title is the toolbar's");

    let open = text_at(&out, "Open the book", None).expect("the shelf's key");
    b.press(open);
    let out = b.settle();
    assert_eq!(b.said(), ("Book".into(), true, false), "into the book: back shown");
    assert!(probe::inner_shown(&b.ctx), "the book is a detail");
    assert!(text_at(&out, "Its pages", None).is_some());

    // The toolbar's back key, where the toolbar draws it.
    let back = pos2(RAIL_W + 22.0 + 14.0, 10.0 + (tokens::TOOLBAR_H - 10.0) / 2.0);
    b.press(back);
    let out = b.settle();
    assert_eq!(b.said(), ("Shelf".into(), false, true), "back on the shelf: forward shown");

    let open = text_at(&out, "Open the book", None).expect("the shelf's key");
    b.press(open);
    let out = b.settle();
    assert_eq!(b.said().0, "Book");
    let item = text_at(&out, "Shelf", Some(RAIL_W)).expect("the rail item");
    b.press(item);
    let _ = b.settle();
    assert_eq!(b.said(), ("Shelf".into(), false, false), "the current item back to its own page, history forgotten");
}

/// The shell's source names no page of any product: the outside table's names and the home product's page
/// and view names are nowhere in it.
#[test]
fn the_shell_names_no_page() {
    let src = include_str!("../src/nav.rs");
    for name in ["Shelf", "Book", "Route::", "Place::", "View::", "Stack::", "Page::", "Works", "Grants", "Settings", "Home"] {
        assert!(!src.contains(name), "the shell names {name}");
    }
}

/// A segment picked on its page stands the history on that page's own place, history behind and ahead dropped and the
/// product's part of the landing run, while the page keeps its entrance and its scroll; a tab set the other way enters
/// anew from the top. Victim: whoever switches between a page's segments and sees the whole page play in again, the
/// list jumping to its top, unlike every other segment.
#[test]
fn a_segment_picked_on_its_page_keeps_the_page() {
    let mut o = Outside::default();
    nav::push(&mut o, Route::Book, 0.0);
    o.frame.scroll_y = 40.0;
    let (key, landings) = (o.frame.entry_key, o.landings);
    nav::seg_tab(&mut o, Place::Shelf, 1.0);
    let h = nav::hist(&mut o).clone();
    assert_eq!((h.cur, h.back.is_empty(), h.fwd.is_empty()), (Route::Shelf, true, true));
    assert_eq!((o.frame.entry_key, o.frame.scroll_y, o.frame.place, o.landings), (key, 40.0, Some(Place::Shelf), landings + 1));
    nav::set_tab(&mut o, Place::Shelf, 2.0);
    assert_eq!((o.frame.entry_key, o.frame.scroll_y, o.landings), (key + 1, 0.0, landings + 2));
}
