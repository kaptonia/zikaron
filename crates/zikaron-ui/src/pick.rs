//! **Pickers**: every "open it and choose a row from a list" is one floating card with a search field on top
//! (and, for lists whose rows carry a time, "start date · end date" under it), then the rows in a scrolling
//! column. The card is a menu's card ([`crate::menu`]): the floating surface, grown from its anchor's corner
//! from 0.97 over 120 ms and played backwards when it leaves; a press outside it ([`crate::layer::menu_guard`]),
//! Esc, or picking a row closes it. Rows are a menu's rows ([`crate::menu::Row`], [`crate::menu::Who`]),
//! drawn by the same functions; rows that cannot be picked stay grey.
//!
//! **This control only draws and picks.** Which rows match the words typed and the days set is the caller's:
//! it is asked once per row through `keep`, with the row's index, the words and the two days.

use crate::datepick;
use crate::menu::{self, Item};
use crate::motion::{self, Curve};
use crate::paint;
use crate::palette::{c, Lift, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

/// The date fields of a picker whose rows carry a time.
pub struct Dates<'a> {
    pub words: &'a datepick::Words<'a>,
    pub today: datepick::Day,
    pub from_hint: &'a str,
    pub to_hint: &'a str,
}

/// What a picker says and how wide it is.
pub struct Spec<'a> {
    /// The search field's hint.
    pub hint: &'a str,
    /// What the card says when no row matches.
    pub empty: &'a str,
    /// The card's width (a record picker is wider: three columns).
    pub w: f32,
    pub dates: Option<Dates<'a>>,
}

/// What a row filter is asked: the row's index, the words typed, the start day and the end day
/// (`YYYY-MM-DD` or empty).
pub type Keep<'k> = &'k dyn Fn(usize, &str, &str, &str) -> bool;

const PAD: f32 = 5.0;
const INSET: f32 = 10.0;
/// The rows' column at most this high; more rows scroll.
const LIST_MAX_H: f32 = 336.0;
const DATE_W: f32 = 136.0;

/// The words typed and the two days of an open picker.
#[derive(Clone, Default)]
struct Typed {
    query: String,
    from: String,
    to: String,
    /// The `opened` count these were typed under (a new opening starts empty).
    opened: u64,
}

/// A key that opens a picker (a secondary key, with a caret when `caret`); returns the row picked.
pub fn key(ui: &mut egui::Ui, id_salt: &str, label: &str, enabled: bool, caret: bool, spec: &Spec, items: &[Item], keep: Keep) -> Option<usize> {
    let id = ui.id().with(("zikaron-pick", id_salt));
    let mut k = crate::button::Key::new(label, crate::button::Role::Secondary).enabled(enabled);
    if caret {
        k = k.trail(crate::icons::Glyph::Down);
    }
    let resp = crate::button::show(ui, k);
    if resp.clicked() {
        menu::toggle(ui.ctx(), id);
    }
    show(ui.ctx(), id, resp.rect, false, spec, items, keep)
}

/// Draw an open picker under `anchor` (aligned to its left edge when `left`, its right edge otherwise).
/// Returns the picked row's index (into `items`).
pub fn show(ctx: &egui::Context, id: egui::Id, anchor: Rect, left: bool, spec: &Spec, items: &[Item], keep: Keep) -> Option<usize> {
    if !menu::is_open(ctx, id) {
        return None;
    }
    let age = motion::age(ctx, id.with("age"), 0);
    let e = Curve::Ease.at((age / tokens::FAST).clamp(0.0, 1.0));
    if age < tokens::FAST {
        ctx.request_repaint();
    }
    // What was typed belongs to this opening only.
    let opened = ctx.data(|d| d.get_temp::<u64>(id.with("opened"))).unwrap_or(0);
    let mut typed = ctx.data(|d| d.get_temp::<Typed>(id.with("typed"))).filter(|t| t.opened == opened).unwrap_or(Typed { opened, ..Default::default() });
    let fresh = ctx.data(|d| d.get_temp::<u64>(id.with("focused"))) != Some(opened);
    let kept: Vec<usize> = (0..items.len()).filter(|i| matches!(items[*i], Item::Row(_) | Item::Who(_)) && keep(*i, &typed.query, &typed.from, &typed.to)).collect();
    let row_h = |it: &Item| match it {
        Item::Row(r) => menu::row_h(r),
        Item::Who(_) => menu::WHO_H,
        _ => 0.0,
    };
    let list_h: f32 = kept.iter().map(|i| row_h(&items[*i])).sum::<f32>().max(34.0).min(LIST_MAX_H);
    let head_h = tokens::INPUT_H + if spec.dates.is_some() { tokens::S2 + tokens::INPUT_H } else { 0.0 };
    let w = spec.w.min(ctx.screen_rect().width() - 16.0);
    let h = PAD + INSET + head_h + INSET + list_h + PAD;
    let x = if left { anchor.left() } else { anchor.right() - w };
    let x = x.clamp(8.0, (ctx.screen_rect().right() - w - 8.0).max(8.0));
    let rect = Rect::from_min_size(pos2(x, anchor.bottom() + 6.0), vec2(w, h));
    let origin = if left { rect.left_top() } else { rect.right_top() };
    let layer = egui::LayerId::new(egui::Order::Foreground, id.with("menu"));
    let scale = 0.97 + 0.03 * e;
    ctx.set_transform_layer(layer, egui::emath::TSTransform { scaling: scale, translation: origin.to_vec2() * (1.0 - scale) + vec2(0.0, -4.0 * (1.0 - e)) });
    let guarded = crate::layer::menu_guard(ctx, layer);
    // Above its guard now, so a date card opened inside it lands above both.
    ctx.move_to_top(layer);
    let mut picked = None;
    egui::Area::new(layer.id).fade_in(false).order(egui::Order::Foreground).fixed_pos(rect.min).constrain(false).show(ctx, |ui| {
        ui.multiply_opacity(e);
        paint::surface(ui.painter(), rect, Radius::Menu, c(C::Surface), Lift::Menu);
        let inner = Rect::from_min_max(pos2(rect.left() + INSET, rect.top() + PAD + INSET), pos2(rect.right() - INSET, rect.bottom() - PAD));
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
        child.spacing_mut().item_spacing.y = tokens::S2;
        let resp = crate::input::search(&mut child, &mut typed.query, spec.hint, inner.width());
        if fresh {
            resp.request_focus();
            ctx.data_mut(|d| d.insert_temp(id.with("focused"), opened));
        }
        if let Some(dates) = spec.dates.as_ref() {
            child.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = tokens::S2;
                datepick::field(ui, "pick-from", &mut typed.from, dates.from_hint, DATE_W, dates.today, dates.words);
                datepick::field(ui, "pick-to", &mut typed.to, dates.to_hint, DATE_W, dates.today, dates.words);
            });
        }
        let top = inner.top() + head_h + INSET;
        ui.painter().hline((rect.left() + 11.0)..=(rect.right() - 11.0), top - INSET / 2.0, egui::Stroke::new(1.0_f32, c(C::Line2)));
        let list = Rect::from_min_max(pos2(rect.left() + PAD, top), pos2(rect.right() - PAD, top + list_h));
        let mut lu = ui.new_child(egui::UiBuilder::new().max_rect(list).layout(egui::Layout::top_down(egui::Align::Min)));
        if kept.is_empty() {
            lu.painter().text(list.center(), egui::Align2::CENTER_CENTER, spec.empty, Type::Note.font(), c(C::Ink3));
            return;
        }
        egui::ScrollArea::vertical().id_salt(id.with("rows")).max_height(list_h).auto_shrink([false, true]).show(&mut lu, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for i in &kept {
                let it = &items[*i];
                let (rr, _) = ui.allocate_exact_size(vec2(list.width(), row_h(it)), egui::Sense::hover());
                let resp = match it {
                    Item::Row(r) => menu::draw_row(ui, id.with(("row", *i)), rr, r, false),
                    Item::Who(wr) => menu::draw_who(ui, id.with(("who", *i)), rr, wr),
                    _ => continue,
                };
                if resp.clicked() {
                    picked = Some(*i);
                }
            }
        });
        ui.allocate_rect(rect, egui::Sense::hover());
    });
    ctx.data_mut(|d| d.insert_temp(id.with("typed"), typed));
    crate::layer::keep(ctx, id.with("keep"), layer, tokens::FAST, 0.97, origin);
    let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    if picked.is_some() || guarded || esc {
        menu::set(ctx, id, false);
    }
    picked
}
