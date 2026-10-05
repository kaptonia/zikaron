//! One primary key per page, and at most one commit key per page.
//!
//! The "exactly one" is carried by the type: `Page::primary` consumes `self`, so a second call on the same
//! page does not compile.
//!
//! The primary key is the only block filled with `button::primary_fill`, so counting fills of that color
//! counts primary keys; on-screen diagnostics and measurements rely on that.

use crate::button::{self, Role};

/// A page, before its primary key is issued.
pub struct Page {
    _seal: (),
}

/// What remains of a page after its primary key. It has no `primary`.
pub struct Sealed {
    _seal: (),
}

impl Default for Page {
    fn default() -> Self {
        Page::new()
    }
}

impl Page {
    pub fn new() -> Page {
        Page { _seal: () }
    }

    /// The page's one primary key (blue, the one button shape via `button::paint`). Calling it gives up the
    /// page, so there is no second.
    pub fn primary(self, ui: &mut egui::Ui, text: &str) -> (Sealed, egui::Response) {
        self.primary_with(ui, text, true)
    }

    /// As above, optionally enabled (disabled is 45%, no shadow, no response). Also gives up the page. Sized
    /// to its text.
    pub fn primary_with(self, ui: &mut egui::Ui, text: &str, enabled: bool) -> (Sealed, egui::Response) {
        let resp = button::key(ui, text, Role::Primary, enabled);
        (Sealed { _seal: () }, resp)
    }
}

/// Commit key: at most one per page. Solid deep red; pressing it commits to the ledger or chain with no
/// further step, so it appears only on the final confirmation card. `press` consumes `self`, so one token
/// cannot press twice.
///
/// Keys that still have a step before committing (opening a confirmation card, moving to a page) are
/// [`Guide`].
pub struct Pen {
    _seal: (),
}

impl Default for Pen {
    fn default() -> Self {
        Pen::new()
    }
}

impl Pen {
    pub fn new() -> Pen {
        Pen { _seal: () }
    }

    /// The solid cinnabar key.
    pub fn press(self, ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
        button::key(ui, text, Role::Commit, enabled)
    }

    /// The same key with a phase, saying `busy` with a turning ring while it is busy (`button::Key::busy_text`).
    pub fn press_saying(self, ui: &mut egui::Ui, text: &str, busy: &str, enabled: bool, phase: button::Phase) -> egui::Response {
        button::show(ui, button::Key::new(text, Role::Commit).enabled(enabled).phase(phase).busy_text(busy))
    }

    /// The solid cinnabar key running a long action (its phase: busy, done, failed).
    pub fn press_long(self, ui: &mut egui::Ui, text: &str, enabled: bool, phase: button::Phase) -> egui::Response {
        button::show(ui, button::Key::new(text, Role::Commit).enabled(enabled).phase(phase))
    }

    /// The same key, saying `busy` with a turning ring while its phase is busy (`button::Key::busy_text`).
    pub fn press_long_saying(self, ui: &mut egui::Ui, text: &str, busy: &str, enabled: bool, phase: button::Phase) -> egui::Response {
        button::show(ui, button::Key::new(text, Role::Commit).enabled(enabled).phase(phase).busy_text(busy))
    }
}

/// Guide key: white with red text and border. It only opens a confirmation card or moves to a page, with one
/// more step before any commit, so any number may share a screen (no token needed). Sized to its text.
///
/// The two red keys differ in their names: the commit key ([`Pen`]) commits when pressed, the guide key leads
/// to the next step. On screen the commit key is solid deep red and the guide key white with red text and
/// border (`Role::Guide`).
pub struct Guide;

impl Guide {
    /// One guide key.
    pub fn key(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
        button::key(ui, text, Role::Guide, enabled)
    }

    /// The last guide key on a detail page (delete, revoke): alone at the bottom, 4 above it and 8 below.
    pub fn last(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
        ui.add_space(4.0);
        let r = Self::key(ui, text, enabled);
        ui.add_space(8.0);
        r
    }
}

/// How many commit keys a frame has: count blocks filled with the commit color (as for primary keys). "No
/// solid deep red outside confirmation cards" is computed from what was drawn.
pub fn commits_in(shapes: &[egui::epaint::ClippedShape]) -> usize {
    fills_of(shapes, button::commit_fill())
}

/// How many primary keys a frame has: count blocks filled with the primary color. This reads the shapes egui
/// actually drew this frame, not what the code claims.
pub fn primaries_in(shapes: &[egui::epaint::ClippedShape]) -> usize {
    fills_of(shapes, button::primary_fill())
}

/// How many blocks in a frame are filled with a color; both counts go through here.
fn fills_of(shapes: &[egui::epaint::ClippedShape], want: egui::Color32) -> usize {
    fn count(s: &egui::Shape, want: egui::Color32, n: &mut usize) {
        match s {
            egui::Shape::Rect(r) => {
                if r.fill == want {
                    *n += 1;
                }
            }
            egui::Shape::Vec(v) => {
                for x in v {
                    count(x, want, n);
                }
            }
            _ => {}
        }
    }
    let mut n = 0;
    for c in shapes {
        count(&c.shape, want, &mut n);
    }
    n
}
