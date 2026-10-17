//! The window shell's navigation, for any product's page table: each view's page history, the rail's groups
//! and items, the moves between pages (into a page, back, forward, a tab, a fresh landing, a rail item), and
//! the two drawn parts that follow them (the rail's list and the page area: toolbar and scrolling body).
//!
//! Nothing here names a page. The product gives its table once, as [`Pages`]: its places, routes and
//! histories, its rail, and what each route draws. The shell keeps its state in a [`Frame`] the product holds.

use crate::icons::Glyph;
use crate::{motion, probe, rail, toolbar, tokens as tk, width};
use std::collections::BTreeMap;

/// One history: pages behind, the page shown, pages ahead.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct History<R> {
    pub back: Vec<R>,
    pub cur: R,
    pub fwd: Vec<R>,
}

impl<R> History<R> {
    /// A history standing on one page.
    pub fn at(root: R) -> History<R> {
        History { back: Vec::new(), cur: root, fwd: Vec::new() }
    }

    /// Go into a page: the current one goes behind, pages ahead are dropped.
    pub fn push(&mut self, r: R) {
        let was = std::mem::replace(&mut self.cur, r);
        self.back.push(was);
        self.fwd.clear();
    }

    /// Back one page; false when there is none.
    pub fn back(&mut self) -> bool {
        match self.back.pop() {
            Some(prev) => {
                let was = std::mem::replace(&mut self.cur, prev);
                self.fwd.insert(0, was);
                true
            }
            None => false,
        }
    }

    /// Forward one page; false when there is none.
    pub fn fwd(&mut self) -> bool {
        if self.fwd.is_empty() {
            return false;
        }
        let next = self.fwd.remove(0);
        let was = std::mem::replace(&mut self.cur, next);
        self.back.push(was);
        true
    }

    /// Back to one page, forgetting the history.
    pub fn root(&mut self, root: R) {
        *self = History::at(root);
    }

    /// The page at the bottom of the history (the first page behind, or the current one).
    pub fn bottom(&self) -> &R {
        self.back.first().unwrap_or(&self.cur)
    }

    /// Whether there is anything behind or ahead.
    pub fn has_history(&self) -> bool {
        !self.back.is_empty() || !self.fwd.is_empty()
    }
}

/// One rail item: where it goes, its name, its glyph.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Item<P, W> {
    pub place: P,
    pub name: W,
    pub glyph: Glyph,
}

/// One rail group: its optional title and its items.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Group<P: 'static, W: 'static> {
    pub title: Option<W>,
    pub items: &'static [Item<P, W>],
}

/// The shell's state over one product's table: where the person is, each history, and how the page on screen
/// entered.
#[derive(Clone, Debug)]
pub struct Frame<P, R, S> {
    /// The current place. `None` until the product settles it.
    pub place: Option<P>,
    /// Which history is shown, and each one.
    pub stack: S,
    pub hist: BTreeMap<S, History<R>>,
    /// How the page on screen entered, and a counter that changes on every page change (entrance animations
    /// key on it).
    pub entry: motion::Entry,
    pub entry_key: u64,
    /// The page's body scroll offset last frame (the toolbar's hairline and detail titles follow it).
    pub scroll_y: f32,
}

impl<P, R, S: Default> Default for Frame<P, R, S> {
    fn default() -> Self {
        Frame { place: None, stack: S::default(), hist: BTreeMap::new(), entry: motion::Entry::default(), entry_key: 0, scroll_y: 0.0 }
    }
}

impl<P, R, S: Ord + Copy> Frame<P, R, S> {
    /// Stand on a history (the product's answer for a place): it becomes the shown one.
    pub fn land(&mut self, (stack, h): (S, History<R>)) {
        self.stack = stack;
        self.hist.insert(stack, h);
    }
}

/// A product's page table, the one place the shell asks about pages.
pub trait Pages {
    /// Where the rail and links go.
    type Place: Copy + PartialEq + std::fmt::Debug + 'static;
    /// One page in a history.
    type Route: Clone + PartialEq + std::fmt::Debug;
    /// Which history.
    type Stack: Copy + Ord + Default + std::hash::Hash + std::fmt::Debug;
    /// A word of the product's table (rail names and titles).
    type Word: Copy + 'static;

    fn frame(&self) -> &Frame<Self::Place, Self::Route, Self::Stack>;
    fn frame_mut(&mut self) -> &mut Frame<Self::Place, Self::Route, Self::Stack>;
    /// A word's text in the language spoken now.
    fn word(&self, w: Self::Word) -> String;
    /// The rail, in order.
    fn rail(&self) -> &'static [Group<Self::Place, Self::Word>];
    /// Where the window starts over (a seat or identity switched).
    fn home(&self) -> Self::Place;
    /// Which history a place belongs to.
    fn stack_of(&self, place: Self::Place) -> Self::Stack;
    /// A history's own place when it has none yet.
    fn root_of(&self, stack: Self::Stack) -> Self::Place;
    /// The history to show for a place (its own page, or a page pushed over it).
    fn history_of(&self, place: Self::Place) -> (Self::Stack, History<Self::Route>);
    /// A place's own page.
    fn root(&self, place: Self::Place) -> Self::Route;
    /// The place a page is the own page of; `None` for a pushed page.
    fn root_place(&self, route: &Self::Route) -> Option<Self::Place>;
    /// The rail item lit now.
    fn lit(&self) -> Option<Self::Place>;
    /// Whether `item` is the rail item `lit` lights.
    fn same_item(&self, lit: Self::Place, item: Self::Place) -> bool {
        lit == item
    }
    /// The count on a rail item.
    fn count(&self, _item: Self::Place) -> usize {
        0
    }
    /// A page's name (tests read which page a frame drew).
    fn name(&self, route: &Self::Route) -> String;
    /// Whether a page is a detail (its large title is in the page).
    fn detail(&self, route: &Self::Route) -> bool;
    /// A page's toolbar title.
    fn title(&self, route: &Self::Route) -> String;
    /// The keys at the right of a place's own page's toolbar.
    fn toolbar(&mut self, ui: &mut egui::Ui, route: &Self::Route, now: f64);
    /// A page's body.
    fn body(&mut self, ui: &mut egui::Ui, route: &Self::Route, now: f64);
    /// The page changed (after the shell's own bookkeeping).
    fn entered(&mut self, _now: f64) {}
}

/// The history shown (one seen for the first time stands on its own page).
pub fn hist<T: Pages + ?Sized>(t: &mut T) -> &mut History<T::Route> {
    let stack = t.frame().stack;
    let root = t.root(t.root_of(stack));
    t.frame_mut().hist.entry(stack).or_insert_with(|| History::at(root))
}

/// The page on screen.
pub fn route<T: Pages + ?Sized>(t: &mut T) -> T::Route {
    hist(t).cur.clone()
}

/// The page changed: how it enters, a new entrance, the body at its top, the place from the history's own page,
/// then the product's part.
pub fn entered<T: Pages + ?Sized>(t: &mut T, how: motion::Entry, now: f64) {
    let f = t.frame_mut();
    f.entry = how;
    f.entry_key = f.entry_key.wrapping_add(1);
    f.scroll_y = 0.0;
    let bottom = hist(t).bottom().clone();
    if let Some(p) = t.root_place(&bottom) {
        t.frame_mut().place = Some(p);
    }
    t.entered(now);
}

/// Go into a page of the history shown.
pub fn push<T: Pages + ?Sized>(t: &mut T, r: T::Route, now: f64) {
    hist(t).push(r);
    entered(t, motion::Entry::Push, now);
}

/// Back one page.
pub fn back<T: Pages + ?Sized>(t: &mut T, now: f64) {
    if hist(t).back() {
        entered(t, motion::Entry::Pop, now);
    }
}

/// Forward one page.
pub fn fwd<T: Pages + ?Sized>(t: &mut T, now: f64) {
    if hist(t).fwd() {
        entered(t, motion::Entry::Push, now);
    }
}

/// A tab of a place's own page: the page is replaced in place and fades.
pub fn set_tab<T: Pages + ?Sized>(t: &mut T, place: T::Place, now: f64) {
    let r = t.root(place);
    let h = hist(t);
    h.cur = r;
    h.back.clear();
    h.fwd.clear();
    entered(t, motion::Entry::Fade, now);
}

/// Land on a place with a fresh history: its history becomes the shown one, standing where the product says.
pub fn go<T: Pages + ?Sized>(t: &mut T, place: T::Place, now: f64) {
    let (stack, h) = t.history_of(place);
    let f = t.frame_mut();
    let same = f.stack == stack && f.hist.get(&stack) == Some(&h);
    f.land((stack, h));
    if !same {
        entered(t, motion::Entry::Root, now);
    }
}

/// A rail item: the history shown goes back to its own page; another shows where it was left.
pub fn rail_to<T: Pages + ?Sized>(t: &mut T, place: T::Place, now: f64) {
    let stack = t.stack_of(place);
    let root = t.root(t.root_of(stack));
    if t.frame().stack == stack {
        let cur = route(t);
        if hist(t).has_history() || t.root_place(&cur).is_none() {
            hist(t).root(root);
            entered(t, motion::Entry::Pop, now);
        }
        return;
    }
    let f = t.frame_mut();
    f.stack = stack;
    f.hist.entry(stack).or_insert_with(|| History::at(root));
    entered(t, motion::Entry::Root, now);
}

/// Start over at the product's home, every history forgotten.
pub fn home<T: Pages + ?Sized>(t: &mut T, how: motion::Entry, now: f64) {
    let home = t.home();
    let stack = t.stack_of(home);
    let f = t.frame_mut();
    f.hist.clear();
    f.stack = stack;
    f.place = Some(home);
    entered(t, how, now);
}

/// The rail's list of groups and items, `list_h` high and scrolling; the first group's title is not drawn.
/// Returns the place of the item clicked.
pub fn rail_list<T: Pages + ?Sized>(ui: &mut egui::Ui, t: &T, list_h: f32) -> Option<T::Place> {
    let lit = t.lit();
    let mut titles: Vec<Option<String>> = Vec::new();
    let mut items: Vec<(Glyph, String, usize, T::Place)> = Vec::new();
    let mut at: Vec<(usize, usize)> = Vec::new();
    let mut lit_i = None;
    for (gi, g) in t.rail().iter().enumerate() {
        titles.push(if gi == 0 { None } else { g.title.map(|w| t.word(w)) });
        let from = items.len();
        for item in g.items {
            if lit.map(|l| t.same_item(l, item.place)).unwrap_or(false) {
                lit_i = Some(items.len());
            }
            items.push((item.glyph, t.word(item.name), t.count(item.place), item.place));
        }
        at.push((from, items.len()));
    }
    let mut lines: Vec<rail::Line> = Vec::new();
    for (title, (from, to)) in titles.iter().zip(at) {
        lines.push(rail::Line::Group(title.as_deref()));
        for (glyph, label, count, _) in &items[from..to] {
            lines.push(rail::Line::Item { glyph: *glyph, label, count: *count });
        }
    }
    let mut picked = None;
    crate::scroll::page(ui, egui::ScrollArea::vertical().scroll_source(egui::scroll_area::ScrollSource { drag: false, ..egui::scroll_area::ScrollSource::ALL }).id_salt("rail-list").max_height(list_h).auto_shrink([false, false]), |ui| {
        if let Some(i) = rail::nav(ui, &lines, lit_i) {
            picked = items.get(i).map(|x| x.3);
        }
    });
    picked
}

/// Where the page's entrance counter is kept for the frame (blocks stagger on it).
const ENTRY_KEY: &str = "zikaron-entry-key";

/// This frame's page entrance counter (set by [`page_area`]).
pub fn entry_key(ctx: &egui::Context) -> u64 {
    ctx.data(|d| d.get_temp::<u64>(egui::Id::new(ENTRY_KEY))).unwrap_or(0)
}

/// The page area in `ui`: the toolbar (back and forward with their tips, the title, the own page's keys) and
/// the scrolling body entering by how the page changed.
pub fn page_area<T: Pages + ?Sized>(ui: &mut egui::Ui, t: &mut T, tips: (&str, &str), now: f64) {
    let ctx = ui.ctx().clone();
    let full = ui.max_rect();
    let route = route(t);
    let root = t.root_place(&route).is_some();
    let detail = !root && t.detail(&route);
    let name = t.name(&route);
    probe::route(&ctx, &name);
    if detail {
        probe::inner(&ctx);
    }
    let title = t.title(&route);
    if detail {
        probe::inner_head(&ctx, &title);
    }
    let (back_on, fwd_on) = {
        let h = hist(t);
        (!h.back.is_empty(), !h.fwd.is_empty())
    };
    probe::history(&ctx, back_on, fwd_on);
    let bar = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), tk::TOOLBAR_H));
    ui.allocate_rect(bar, egui::Sense::hover());
    // A detail page carries its large title in the page; the toolbar's title shows only once that has scrolled
    // away.
    let scroll_y = t.frame().scroll_y;
    let title_alpha = if detail { if scroll_y > 44.0 { 1.0 } else { 0.0 } } else { 1.0 };
    let (moved, _) = toolbar::toolbar(ui, bar, &title, title_alpha, (back_on, fwd_on), scroll_y > 4.0, tips, |ui| {
        if root {
            probe::head_keys(ui.ctx());
            t.toolbar(ui, &route, now);
        }
    });
    if moved.back {
        back(t, now);
    }
    if moved.fwd {
        fwd(t, now);
    }
    // The body scrolls; it enters by how the page changed.
    let body_rect = egui::Rect::from_min_max(egui::pos2(full.left(), bar.bottom()), full.max);
    let mut body = ui.new_child(egui::UiBuilder::new().max_rect(body_rect).layout(egui::Layout::top_down(egui::Align::Min)));
    body.set_clip_rect(body_rect);
    let (stack, key, how) = {
        let f = t.frame();
        (f.stack, f.entry_key, f.entry)
    };
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(ENTRY_KEY), key));
    let p = motion::enter(&ctx, egui::Id::new("zikaron-page-entry"), key, 0.0, how.dur(), motion::Curve::Ease);
    let (off, alpha) = how.at(p);
    let route_salt = motion::key_of(&(stack, name, key));
    let shown = crate::scroll::page(&mut body, egui::ScrollArea::vertical().scroll_source(egui::scroll_area::ScrollSource { drag: false, ..egui::scroll_area::ScrollSource::ALL }).id_salt(("page", route_salt)).auto_shrink([false, false]), |ui| {
        egui::Frame::new()
            .inner_margin(egui::Margin { left: tk::PAGE_PAD as i8, right: tk::PAGE_PAD as i8, top: tk::S3 as i8, bottom: tk::PAGE_PAD as i8 })
            .show(ui, |ui| {
                width::body(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = tk::CARD_GAP;
                    motion::shifted(ui, off, alpha, |ui| {
                        ui.spacing_mut().item_spacing.y = tk::CARD_GAP;
                        t.body(ui, &route, now);
                    });
                });
            });
    });
    t.frame_mut().scroll_y = shown.state.offset.y;
}
