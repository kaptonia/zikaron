//! Menus and pop-ups: rows 34 high on a floating card; the hovered row fills with blue and its words turn
//! white. A menu grows out of its anchor's corner from 0.97 and fades in over 120 ms, and leaves the same way
//! backwards (120 ms, back to 0.97). A click outside, Esc, or picking a row closes it.
//!
//! The identity lens (only the switch-identity menu uses it): identity rows ([`Item::Who`]) are 48 high with
//! two lines (the name; the kind, with "primary" and "in use" in the accent ink at its end). The current or
//! hovered one is framed by one blue lens (a 2-point accent border, 7-point corners, no fill) that slides
//! between identity rows only (top and height over 220 ms; on opening it sits on the identity in use at once).
//! Where it lands: the hovered identity, else the one the arrow keys moved to, else the one in use; the other
//! rows of that menu are plain (a light grey ground on hover, darker when pressed) and the lens never lands on
//! them. ↑ ↓ move among identity rows, Enter picks the framed one, Esc closes.

use crate::icons::{self, Glyph};
use crate::mark;
use crate::motion::{self, Curve};
use crate::paint;
use crate::palette::{self, c, Lift, Tone, C};
use crate::tokens::{self, Radius, Type};
use egui::{pos2, vec2, Rect};

/// One row of a menu.
#[derive(Clone, Copy, Debug, Default)]
pub struct Row<'a> {
    pub label: &'a str,
    /// Small words before the label (such as "address").
    pub lead: &'a str,
    /// The label in monospace (short addresses).
    pub mono: bool,
    /// A second, smaller line under the label; green when `sub_ok`.
    pub sub: &'a str,
    pub sub_ok: bool,
    /// A check column before the label: `Some(true)` checked, `Some(false)` empty.
    pub check: Option<bool>,
    /// A check at the right end (the identity in use).
    pub tick_right: bool,
    /// A pill at the right end (why a row cannot be picked).
    pub pill: Option<(&'a str, Tone)>,
    /// Small words at the right end ("#3").
    pub trail: &'a str,
    /// Struck through (a deleted record).
    pub struck: bool,
    pub disabled: bool,
}

/// One identity row of the lens menu: two lines, always 48 high.
#[derive(Clone, Copy, Debug, Default)]
pub struct Who<'a> {
    /// The name (upper line).
    pub name: &'a str,
    /// The kind (lower line).
    pub kind: &'a str,
    /// Words after the kind in the accent ink ("primary", "in use"); changing them changes only this line's end.
    pub tags: &'a [&'a str],
    /// The identity in use: where the lens rests.
    pub current: bool,
}

/// A menu's items. Closed.
#[derive(Clone, Copy, Debug)]
pub enum Item<'a> {
    Head(&'a str),
    Sep,
    Row(Row<'a>),
    /// An identity row: the menu becomes a lens menu (see the file header).
    Who(Who<'a>),
}

/// Identity rows are this high (two lines).
pub const WHO_H: f32 = 48.0;

pub fn row(label: &str) -> Item<'_> {
    Item::Row(Row { label, ..Default::default() })
}

fn open_id(id: egui::Id) -> egui::Id {
    id.with("open")
}

/// Whether a menu is open.
pub fn is_open(ctx: &egui::Context, id: egui::Id) -> bool {
    ctx.data(|d| d.get_temp::<bool>(open_id(id))).unwrap_or(false)
}

/// Open or close a menu (its anchor calls this when clicked).
pub fn toggle(ctx: &egui::Context, id: egui::Id) {
    let now = is_open(ctx, id);
    set(ctx, id, !now);
}

pub fn set(ctx: &egui::Context, id: egui::Id, open: bool) {
    ctx.data_mut(|d| d.insert_temp(open_id(id), open));
    if open {
        motion::restart(ctx, id.with("age"));
        // A new opening: the lens and the arrow keys start over (the lens sits on the identity in use at
        // once, without sliding there).
        ctx.data_mut(|d| {
            let n = d.get_temp::<u64>(id.with("opened")).unwrap_or(0) + 1;
            d.insert_temp(id.with("opened"), n);
            d.remove::<usize>(id.with("keyed"));
        });
    }
}

/// Draw an open menu under `anchor` (aligned to its left edge when `left`, its right edge otherwise), at
/// least `min_w` wide. Returns the picked row's index.
pub fn show(ctx: &egui::Context, id: egui::Id, anchor: Rect, left: bool, min_w: f32, items: &[Item]) -> Option<usize> {
    if !is_open(ctx, id) {
        return None;
    }
    let age = motion::age(ctx, id.with("age"), 0);
    let e = Curve::Ease.at((age / tokens::FAST).clamp(0.0, 1.0));
    if age < tokens::FAST {
        ctx.request_repaint();
    }
    // Width: the widest row, at least `min_w`.
    let row_w = |it: &Item| -> f32 {
        match it {
            Item::Row(r) => {
                let f = if r.mono { egui::FontId::new(14.0, egui::FontFamily::Monospace) } else { Type::Key.font() };
                let mut w = ctx.fonts(|fo| fo.layout_no_wrap(r.label.to_string(), f, egui::Color32::BLACK).size().x) + 24.0;
                if !r.lead.is_empty() {
                    w += 36.0;
                }
                if r.check.is_some() {
                    w += 22.0;
                }
                if let Some((p, _)) = r.pill {
                    w += ctx.fonts(|fo| fo.layout_no_wrap(p.to_string(), Type::Small.font(), egui::Color32::BLACK).size().x) + 32.0;
                }
                if !r.trail.is_empty() || r.tick_right {
                    w += 40.0;
                }
                w
            }
            Item::Who(wr) => {
                let tags = if wr.tags.is_empty() { String::new() } else { format!(" \u{b7} {}", wr.tags.join(" \u{b7} ")) };
                let a = ctx.fonts(|fo| fo.layout_no_wrap(wr.name.to_string(), Type::Key.font(), egui::Color32::BLACK).size().x);
                let b = ctx.fonts(|fo| fo.layout_no_wrap(format!("{}{tags}", wr.kind), Type::Small.font(), egui::Color32::BLACK).size().x);
                a.max(b) + 24.0
            }
            _ => 0.0,
        }
    };
    let w = items.iter().map(row_w).fold(min_w, f32::max).min(ctx.screen_rect().width() - 32.0) + 10.0;
    let h: f32 = items
        .iter()
        .map(|it| match it {
            Item::Head(_) => 24.0,
            Item::Sep => 11.0,
            Item::Row(r) => if r.sub.is_empty() { 34.0 } else { 44.0 },
            Item::Who(_) => WHO_H,
        })
        .sum::<f32>()
        + 10.0;
    let x = if left { anchor.left() } else { anchor.right() - w };
    let x = x.clamp(8.0, (ctx.screen_rect().right() - w - 8.0).max(8.0));
    let rect = Rect::from_min_size(pos2(x, anchor.bottom() + 6.0), vec2(w, h));
    let origin = if left { rect.left_top() } else { rect.right_top() };
    let layer = egui::LayerId::new(egui::Order::Foreground, id.with("menu"));
    let scale = 0.97 + 0.03 * e;
    ctx.set_transform_layer(layer, egui::emath::TSTransform { scaling: scale, translation: origin.to_vec2() * (1.0 - scale) + vec2(0.0, -4.0 * (1.0 - e)) });
    let mut picked = None;
    // The lens: which identity rows there are, where each sits, and which one it frames now.
    let lens = items.iter().any(|it| matches!(it, Item::Who(_)));
    let mut who_rows: Vec<(usize, f32)> = Vec::new();
    {
        let mut y = rect.top() + 5.0;
        for (i, it) in items.iter().enumerate() {
            match it {
                Item::Head(_) => y += 24.0,
                Item::Sep => y += 11.0,
                Item::Row(r) => y += if r.sub.is_empty() { 34.0 } else { 44.0 },
                Item::Who(_) => {
                    who_rows.push((i, y));
                    y += WHO_H;
                }
            }
        }
    }
    let keyed_id = id.with("keyed");
    if lens && !who_rows.is_empty() {
        let (up, down, enter) = ctx.input(|i| (i.key_pressed(egui::Key::ArrowUp), i.key_pressed(egui::Key::ArrowDown), i.key_pressed(egui::Key::Enter)));
        let home = who_rows.iter().position(|(i, _)| matches!(items[*i], Item::Who(w) if w.current)).unwrap_or(0);
        let mut at = ctx.data(|d| d.get_temp::<usize>(keyed_id)).unwrap_or(home);
        let n = who_rows.len();
        if up {
            at = (at + n - 1) % n;
        }
        if down {
            at = (at + 1) % n;
        }
        if up || down {
            ctx.data_mut(|d| d.insert_temp(keyed_id, at));
        }
        if enter {
            picked = Some(who_rows[at].0);
        }
    }
    let shown = egui::Area::new(layer.id).order(egui::Order::Foreground).fixed_pos(rect.min).constrain(false).show(ctx, |ui| {
        ui.multiply_opacity(e);
        paint::surface(ui.painter(), rect, Radius::Menu, c(C::Surface), Lift::Float);
        let mut y = rect.top() + 5.0;
        for (i, it) in items.iter().enumerate() {
            match it {
                Item::Head(s) => {
                    ui.painter().text(pos2(rect.left() + 15.0, y + 14.0), egui::Align2::LEFT_CENTER, *s, Type::Micro.font(), c(C::Ink3));
                    y += 24.0;
                }
                Item::Sep => {
                    ui.painter().hline((rect.left() + 11.0)..=(rect.right() - 11.0), y + 5.5, egui::Stroke::new(1.0_f32, c(C::Line2)));
                    y += 11.0;
                }
                Item::Row(r) => {
                    let rh = if r.sub.is_empty() { 34.0 } else { 44.0 };
                    let rr = Rect::from_min_size(pos2(rect.left() + 5.0, y), vec2(w - 10.0, rh));
                    let sense = if r.disabled { egui::Sense::hover() } else { egui::Sense::click() };
                    let resp = ui.interact(rr, id.with(("row", i)), sense);
                    let hovered = !r.disabled && resp.hovered();
                    // In a lens menu the plain rows keep their ink: a light grey ground on hover, darker when
                    // pressed; elsewhere the hovered row fills with blue and its words turn white.
                    let hot = hovered && !lens;
                    if hot {
                        ui.painter().rect_filled(rr, egui::CornerRadius::same(6), c(C::Accent));
                    } else if hovered {
                        let press = resp.is_pointer_button_down_on();
                        ui.painter().rect_filled(rr, egui::CornerRadius::same(6), c(if press { C::Press } else { C::Hover }));
                    }
                    let alpha = if r.disabled { 0.4 } else { 1.0 };
                    let ink = if hot { palette::ON_SOLID } else { c(C::Ink) }.gamma_multiply(alpha);
                    let quiet = if hot { palette::ON_SOLID } else { c(C::Ink3) }.gamma_multiply(alpha);
                    let p = ui.painter();
                    let mut tx = rr.left() + 12.0;
                    let right_edge = rr.right() - 12.0;
                    if let Some(on) = r.check {
                        if on {
                            icons::glyph_at(p, Glyph::Ok, pos2(tx + 7.0, rr.center().y), 14.0, if hot { palette::ON_SOLID } else { c(C::Accent) });
                        }
                        tx += 14.0 + 8.0;
                    }
                    if !r.lead.is_empty() {
                        p.text(pos2(tx, rr.center().y), egui::Align2::LEFT_CENTER, r.lead, Type::Small.font(), quiet);
                        tx += 36.0;
                    }
                    let mut rx = right_edge;
                    if r.tick_right {
                        icons::glyph_at(p, Glyph::Ok, pos2(rx - 7.0, rr.center().y), 14.0, if hot { palette::ON_SOLID } else { c(C::Accent) });
                        rx -= 14.0 + 8.0;
                    }
                    if !r.trail.is_empty() {
                        let t = p.text(pos2(rx, rr.center().y), egui::Align2::RIGHT_CENTER, r.trail, Type::Small.font(), quiet);
                        rx = t.left() - 12.0;
                    }
                    if let Some((s, tone)) = r.pill {
                        let pw = mark::pill_w(ui, s, false);
                        let pr = Rect::from_min_size(pos2(rx - pw, rr.center().y - tokens::PILL_H / 2.0), vec2(pw, tokens::PILL_H));
                        p.rect_filled(pr, Radius::Pill.egui(), tone.wash());
                        p.text(pr.center(), egui::Align2::CENTER_CENTER, s, Type::Small.font(), tone.ink());
                        rx = pr.left() - 12.0;
                    }
                    let font = if r.mono { egui::FontId::new(14.0, egui::FontFamily::Monospace) } else { Type::Key.font() };
                    let room = (rx - tx).max(0.0);
                    if r.sub.is_empty() {
                        let lr = paint::at(p, ui, pos2(tx, rr.center().y), egui::Align2::LEFT_CENTER, r.label, if r.mono { Type::MonoSmall } else { Type::Key }, ink, room);
                        let _ = font;
                        if r.struck {
                            p.hline(lr.x_range(), rr.center().y, egui::Stroke::new(1.0_f32, ink));
                        }
                    } else {
                        paint::at(p, ui, pos2(tx, rr.top() + 5.0), egui::Align2::LEFT_TOP, r.label, Type::Small, ink, room);
                        let sub_c = if hot { palette::ON_SOLID } else if r.sub_ok { c(C::OkInk) } else { c(C::Ink3) };
                        paint::at(p, ui, pos2(tx, rr.top() + 23.0), egui::Align2::LEFT_TOP, r.sub, Type::Small, sub_c, room);
                    }
                    if resp.clicked() {
                        picked = Some(i);
                    }
                    if !r.disabled {
                        let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    }
                    y += rh;
                }
                Item::Who(wr) => {
                    let rr = Rect::from_min_size(pos2(rect.left() + 5.0, y), vec2(w - 10.0, WHO_H));
                    let resp = ui.interact(rr, id.with(("who", i)), egui::Sense::click());
                    if resp.hovered() {
                        ctx.data_mut(|d| d.insert_temp(id.with("hovered-who"), i));
                    }
                    let p = ui.painter();
                    let room = rr.width() - 24.0;
                    // Two lines, each always there: the name, then the kind with its tags in the accent ink.
                    paint::at(p, ui, pos2(rr.left() + 12.0, rr.top() + 6.0), egui::Align2::LEFT_TOP, wr.name, Type::Key, c(C::Ink), room);
                    let kind_r = paint::at(p, ui, pos2(rr.left() + 12.0, rr.top() + 26.0), egui::Align2::LEFT_TOP, wr.kind, Type::Small, c(C::Ink3), room);
                    let mut tx = kind_r.right();
                    for t in wr.tags {
                        let dot = p.text(pos2(tx, rr.top() + 26.0), egui::Align2::LEFT_TOP, " \u{b7} ", Type::Small.font(), c(C::Ink3));
                        let tr = p.text(pos2(dot.right(), rr.top() + 26.0), egui::Align2::LEFT_TOP, *t, Type::Small.font(), c(C::AccentInk));
                        tx = tr.right();
                    }
                    if resp.clicked() {
                        picked = Some(i);
                    }
                    let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                    y += WHO_H;
                }
            }
        }
        // The lens: one blue frame over the identity rows only (hovered > moved to by keys > in use).
        if lens && !who_rows.is_empty() {
            let hovered = ctx.data(|d| d.get_temp::<usize>(id.with("hovered-who")));
            let pointer_in = ctx.input(|i| i.pointer.hover_pos()).map(|p| who_rows.iter().any(|(_, y)| Rect::from_min_size(pos2(rect.left() + 5.0, *y), vec2(w - 10.0, WHO_H)).contains(p))).unwrap_or(false);
            let home = who_rows.iter().position(|(i, _)| matches!(items[*i], Item::Who(w) if w.current)).unwrap_or(0);
            let keyed = ctx.data(|d| d.get_temp::<usize>(keyed_id));
            let target = match (pointer_in, hovered) {
                (true, Some(h)) => who_rows.iter().position(|(i, _)| *i == h).unwrap_or(home),
                _ => keyed.unwrap_or(home),
            };
            let opened = ctx.data(|d| d.get_temp::<u64>(id.with("opened"))).unwrap_or(0);
            let top = motion::to(ctx, id.with(("lens", opened)), who_rows[target].1, tokens::LENS, Curve::Ease);
            let lr = Rect::from_min_size(pos2(rect.left() + 5.0, top), vec2(w - 10.0, WHO_H));
            ui.painter().rect_stroke(lr, egui::CornerRadius::same(7), egui::Stroke::new(2.0_f32, c(C::Accent)), egui::StrokeKind::Inside);
        }
        ui.allocate_rect(rect, egui::Sense::hover());
    });
    crate::layer::keep(ctx, id.with("keep"), layer, tokens::FAST, 0.97, origin);
    // Close on pick, Esc, or a press outside the menu and its anchor.
    let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().map(|p| !rect.contains(p) && !anchor.contains(p)).unwrap_or(false));
    let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    if picked.is_some() || outside || esc {
        set(ctx, id, false);
    }
    let _ = shown;
    picked
}

/// A key that opens a menu: a secondary key with a caret; returns the picked row.
pub fn menu_key(ui: &mut egui::Ui, id_salt: &str, label: &str, left: bool, min_w: f32, items: &[Item]) -> Option<usize> {
    let id = ui.id().with(("zikaron-menu", id_salt));
    let resp = crate::button::show(ui, crate::button::Key::new(label, crate::button::Role::Secondary).trail(Glyph::Down));
    if resp.clicked() {
        toggle(ui.ctx(), id);
    }
    show(ui.ctx(), id, resp.rect, left, min_w, items)
}

/// A key without a caret that opens a menu (the "recent addresses" key beside a field).
pub fn plain_key(ui: &mut egui::Ui, id_salt: &str, label: &str, enabled: bool, left: bool, min_w: f32, items: &[Item]) -> Option<usize> {
    let id = ui.id().with(("zikaron-menu", id_salt));
    let resp = crate::button::key(ui, label, crate::button::Role::Secondary, enabled);
    if resp.clicked() {
        toggle(ui.ctx(), id);
    }
    show(ui.ctx(), id, resp.rect, left, min_w, items)
}
