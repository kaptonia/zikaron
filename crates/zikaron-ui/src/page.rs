use crate::button::{self, Role};

pub struct Page {
    _seal: (),
}

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

    pub fn primary(self, ui: &mut egui::Ui, text: &str) -> (Sealed, egui::Response) {
        self.primary_with(ui, text, true)
    }

    pub fn primary_lead(self, ui: &mut egui::Ui, text: &str, glyph: crate::icons::Glyph) -> (Sealed, egui::Response) {
        let resp = button::show(ui, button::Key::new(text, Role::Primary).lead(glyph));
        (Sealed { _seal: () }, resp)
    }

    pub fn primary_with(self, ui: &mut egui::Ui, text: &str, enabled: bool) -> (Sealed, egui::Response) {
        let resp = button::key(ui, text, Role::Primary, enabled);
        (Sealed { _seal: () }, resp)
    }

    pub fn primary_why(self, ui: &mut egui::Ui, text: &str, enabled: bool, why: &str) -> (Sealed, egui::Response) {
        let resp = button::show(ui, button::Key::new(text, Role::Primary).enabled(enabled).why(why));
        (Sealed { _seal: () }, resp)
    }
}

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

    pub fn press(self, ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
        button::key(ui, text, Role::Commit, enabled)
    }

    pub fn press_why(self, ui: &mut egui::Ui, text: &str, enabled: bool, why: &str) -> egui::Response {
        button::show(ui, button::Key::new(text, Role::Commit).enabled(enabled).why(why))
    }

    pub fn press_saying(self, ui: &mut egui::Ui, text: &str, busy: &str, enabled: bool, phase: button::Phase) -> egui::Response {
        button::show(ui, button::Key::new(text, Role::Commit).enabled(enabled).phase(phase).busy_text(busy))
    }

    pub fn press_saying_why(self, ui: &mut egui::Ui, text: &str, busy: &str, enabled: bool, phase: button::Phase, why: &str) -> egui::Response {
        button::show(ui, button::Key::new(text, Role::Commit).enabled(enabled).phase(phase).busy_text(busy).why(why))
    }

    pub fn press_long(self, ui: &mut egui::Ui, text: &str, enabled: bool, phase: button::Phase) -> egui::Response {
        button::show(ui, button::Key::new(text, Role::Commit).enabled(enabled).phase(phase))
    }

    pub fn press_long_saying(self, ui: &mut egui::Ui, text: &str, busy: &str, enabled: bool, phase: button::Phase) -> egui::Response {
        button::show(ui, button::Key::new(text, Role::Commit).enabled(enabled).phase(phase).busy_text(busy))
    }
}

pub struct Erase;

impl Erase {
    pub fn press(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
        button::key(ui, text, Role::Danger, enabled)
    }

    pub fn press_why(ui: &mut egui::Ui, text: &str, enabled: bool, why: &str) -> egui::Response {
        button::show(ui, button::Key::new(text, Role::Danger).enabled(enabled).why(why))
    }
}

pub struct Guide;

impl Guide {
    pub fn key(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
        button::key(ui, text, Role::Guide, enabled)
    }

    pub fn last(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
        ui.add_space(4.0);
        let r = Self::key(ui, text, enabled);
        ui.add_space(8.0);
        r
    }
}

pub fn commits_in(shapes: &[egui::epaint::ClippedShape]) -> usize {
    fills_of(shapes, button::commit_fill())
}

pub fn primaries_in(shapes: &[egui::epaint::ClippedShape]) -> usize {
    fills_of(shapes, button::primary_fill())
}

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
