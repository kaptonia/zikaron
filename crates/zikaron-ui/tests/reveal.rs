//! What opens under a press near the window's foot is brought into view as it opens: a fold in a card, a fold of its
//! own and a drop-down's card each end 32 above the bottom of what they scroll in once open, the head never taken
//! past the top; on a page whose keys stand in a bar at the view's foot, they end 32 above the bar.

use egui::{Pos2, Rect, Vec2};
use zikaron_ui::{card, fold, menu};

const SCREEN: Vec2 = Vec2::new(800.0, 600.0);

struct Frames {
    ctx: egui::Context,
    t: f64,
}

impl Frames {
    fn new() -> Frames {
        let ctx = egui::Context::default();
        let _ = zikaron_ui::skin::dress(&ctx);
        Frames { ctx, t: 1.0 }
    }

    fn run(&mut self, events: Vec<egui::Event>, mut add: impl FnMut(&mut egui::Ui)) -> egui::FullOutput {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, SCREEN)), time: Some(self.t), events, ..Default::default() };
        self.ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                let _ = zikaron_ui::scroll::page(ui, egui::ScrollArea::vertical().id_salt("page"), |ui| add(ui));
            });
        })
    }
}

fn press(at: Pos2) -> Vec<egui::Event> {
    let b = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    vec![egui::Event::PointerMoved(at), b(true), b(false)]
}

/// The page: room above, then what is tested, then room under it (so the page can scroll). Answers its head, its
/// whole, and the view the page scrolls in.
fn page(ui: &mut egui::Ui, above: f32, mut what: impl FnMut(&mut egui::Ui) -> Rect, out: &mut (Rect, Rect, Rect)) {
    ui.add_space(above);
    let head = what(ui);
    let end = ui.min_rect().bottom();
    ui.add_space(400.0);
    *out = (head, Rect::from_min_max(head.min, egui::pos2(head.right(), end)), ui.clip_rect());
}

/// Opens what `what` draws by pressing `at_head` (its head's middle) and lets it settle; where its head and its whole
/// lie in the window then.
fn opened(above: f32, mut what: impl FnMut(&mut egui::Ui) -> Rect) -> (Rect, Rect, Rect) {
    let mut f = Frames::new();
    let mut got = (Rect::NOTHING, Rect::NOTHING, Rect::NOTHING);
    for _ in 0..3 {
        let _ = f.run(Vec::new(), |ui| page(ui, above, &mut what, &mut got));
    }
    let at = got.0.center();
    let _ = f.run(press(at), |ui| page(ui, above, &mut what, &mut got));
    for _ in 0..40 {
        let _ = f.run(Vec::new(), |ui| page(ui, above, &mut what, &mut got));
    }
    got
}

#[test]
fn a_fold_in_a_card_is_brought_into_view_as_it_opens() {
    let (head, whole, view) = opened(520.0, |ui| {
        let mut head = Rect::NOTHING;
        card::form(ui, |ui, f| {
            let top = ui.cursor().min.y;
            let _ = f.fold_from(ui, "more", "详细信息", "", false, |ui, f| {
                for i in 0..6 {
                    f.note(ui, &format!("第 {i} 行"));
                }
            });
            head = Rect::from_min_size(egui::pos2(ui.min_rect().left(), top), egui::vec2(400.0, 56.0));
        });
        head
    });
    assert!(head.top() >= view.top(), "the head stays in view: {head:?} in {view:?}");
    assert!((view.bottom() - whole.bottom() - 32.0).abs() < 2.0, "its rows end 32 above the view's foot: {whole:?} in {view:?}");
}

#[test]
fn a_fold_of_its_own_is_brought_into_view_as_it_opens() {
    let (head, whole, view) = opened(540.0, |ui| {
        let top = ui.cursor().min;
        let _ = fold::fold(ui, "own", "高级选项", |ui| {
            for i in 0..6 {
                ui.label(format!("第 {i} 行"));
            }
        });
        Rect::from_min_size(top, egui::vec2(80.0, 26.0))
    });
    assert!(head.top() >= view.top(), "{head:?} in {view:?}");
    assert!((view.bottom() - whole.bottom() - 32.0).abs() < 2.0, "{whole:?} in {view:?}");
}

/// On a page whose keys stand in a bar held at the view's foot, what opens ends 32 above the bar, not behind it:
/// the bar is not view.
#[test]
fn a_fold_opened_over_a_bottom_bar_ends_above_the_bar() {
    use zikaron_ui::formparts;
    let bar = 14.0 + zikaron_ui::tokens::KEY_H + 16.0;
    let mut f = Frames::new();
    let mut got = (Rect::NOTHING, Rect::NOTHING, Rect::NOTHING);
    let draw = |ui: &mut egui::Ui, got: &mut (Rect, Rect, Rect)| {
        ui.add_space(480.0);
        let mut head = Rect::NOTHING;
        card::form(ui, |ui, f| {
            let top = ui.cursor().min.y;
            let _ = f.fold_from(ui, "more", "独占、转授权与范围", "", false, |ui, f| {
                for i in 0..6 {
                    f.note(ui, &format!("第 {i} 行"));
                }
            });
            head = Rect::from_min_size(egui::pos2(ui.min_rect().left(), top), egui::vec2(400.0, 56.0));
        });
        let end = ui.min_rect().bottom();
        ui.add_space(400.0);
        *got = (head, Rect::from_min_max(head.min, egui::pos2(head.right(), end)), ui.clip_rect());
        let _ = formparts::page_bar(ui, |ui| ui.button("确认"));
    };
    for _ in 0..3 {
        let _ = f.run(Vec::new(), |ui| draw(ui, &mut got));
    }
    let at = got.0.center();
    let _ = f.run(press(at), |ui| draw(ui, &mut got));
    for _ in 0..40 {
        let _ = f.run(Vec::new(), |ui| draw(ui, &mut got));
    }
    let (head, whole, view) = got;
    assert!(head.top() >= view.top(), "the head stays in view: {head:?} in {view:?}");
    assert!((view.bottom() - bar - whole.bottom() - 32.0).abs() < 2.0, "its rows end 32 above the bar ({bar} high): {whole:?} in {view:?}");
}

#[test]
fn a_drop_down_near_the_foot_takes_the_page_along() {
    let items = [menu::Item::Row(menu::Row { label: "甲", ..Default::default() }), menu::Item::Row(menu::Row { label: "乙", ..Default::default() }), menu::Item::Row(menu::Row { label: "丙", ..Default::default() }), menu::Item::Row(menu::Row { label: "丁", ..Default::default() })];
    let mut f = Frames::new();
    let mut key = Rect::NOTHING;
    let draw = |ui: &mut egui::Ui, key: &mut Rect| {
        ui.add_space(540.0);
        let top = ui.cursor().min;
        let _ = menu::menu_key(ui, "m", "选择", true, 120.0, &items);
        *key = Rect::from_min_size(top, egui::vec2(100.0, 34.0));
        ui.add_space(400.0);
    };
    for _ in 0..3 {
        let _ = f.run(Vec::new(), |ui| draw(ui, &mut key));
    }
    let _ = f.run(press(key.center()), |ui| draw(ui, &mut key));
    let mut out = None;
    for _ in 0..40 {
        out = Some(f.run(Vec::new(), |ui| draw(ui, &mut key)));
    }
    // The card: the largest filled rectangle on a layer of its own under the key.
    let out = out.expect("a frame");
    let card = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Rect(r) if r.fill.a() > 0 && r.rect.top() >= key.bottom() && r.rect.height() > 100.0 && r.rect.width() < 400.0 => Some(r.rect),
            _ => None,
        })
        .max_by(|a, b| a.area().total_cmp(&b.area()));
    let card = card.expect("the drop-down's card is drawn under its key");
    assert!(key.top() >= 0.0, "{key:?}");
    assert!(card.bottom() <= SCREEN.y - 32.0 + 2.0 && card.bottom() >= SCREEN.y - 32.0 - 10.0, "the card ends about 32 above the window's foot: {card:?}");
}

/// While the window scrolls on its own to bring an opened fold into view, a list above keeps its rows' comings: a row
/// that comes then still opens in (it is not simply there), as it would on a page standing still.
#[test]
fn a_row_that_comes_while_the_window_scrolls_on_its_own_still_opens_in() {
    use zikaron_ui::table;
    let cols = [table::col("名", table::Col::Fr(1.0))];
    let row = |s: &str| table::Row { cells: vec![table::Cell::Text(s.to_string())], ..Default::default() };
    let mut f = Frames::new();
    let mut head = Rect::NOTHING;
    let mut with_c = false;
    let draw = |ui: &mut egui::Ui, with_c: bool, head: &mut Rect| {
        let rows: Vec<table::Row> = if with_c { vec![row("甲"), row("乙"), row("丙")] } else { vec![row("甲"), row("乙")] };
        let _ = table::table(ui, "t", &cols, false, &rows, "");
        ui.add_space(380.0);
        let top = ui.cursor().min;
        let _ = fold::fold(ui, "own", "高级选项", |ui| {
            for i in 0..8 {
                ui.label(format!("第 {i} 行"));
            }
        });
        *head = Rect::from_min_size(top, egui::vec2(80.0, 26.0));
        ui.add_space(400.0);
    };
    for _ in 0..4 {
        let _ = f.run(Vec::new(), |ui| draw(ui, with_c, &mut head));
    }
    let _ = f.run(press(head.center()), |ui| draw(ui, with_c, &mut head));
    let _ = f.run(Vec::new(), |ui| draw(ui, with_c, &mut head));
    with_c = true;
    let mut alphas = Vec::new();
    for _ in 0..6 {
        let out = f.run(Vec::new(), |ui| draw(ui, with_c, &mut head));
        let a = out.shapes.iter().find_map(|c| match &c.shape {
            // The words' drawn ink: a row opening in has it faded.
            egui::Shape::Text(t) if t.galley.job.text == "丙" => t.galley.rows.iter().flat_map(|r| r.visuals.mesh.vertices.iter()).map(|v| v.color.a()).max(),
            _ => None,
        });
        alphas.push(a.unwrap_or(0));
    }
    assert!(alphas.iter().any(|a| *a < 250), "the new row opens in while the window scrolls on its own: {alphas:?}");
}
