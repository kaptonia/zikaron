//! The material: one language of surfaces by elevation, which every raised thing in the kit is painted with.
//!
//! The page ground is the lowest level and takes no surface; a faint accent wash lies in its top left corner
//! ([`ground_tint`]). Above it: a card, a float (menus, search pickers and the date picker open over the page
//! beside what opened them), a toast, and a sheet over everything. Every surface has a half-point rim instead of a
//! line. A card is the plain surface with a short and a long soft shadow. A float, a toast and a sheet are glass:
//! a translucent ground with a faint accent wash at the top, a thread of light along the top edge, and the longer
//! shadow of their height. Small raised parts (keys, the segment thumb, the switch knob) take their own rim and
//! drops ([`drop`], [`rim`], [`inset_edge`]); solid keys are never haloed in their own colour.
//!
//! Painting only: no surface here changes where anything lies.

use crate::palette::{blend, c, mix, C};
use std::sync::atomic::{AtomicBool, Ordering};
use crate::tokens::Radius;
use egui::{pos2, Color32, Rect, Shape};

static GLASS: AtomicBool = AtomicBool::new(false);

/// Say whether the system's own material lies under the window (set once the window has it): the rail's ground
/// then lets it through.
pub fn set_glass(on: bool) {
    GLASS.store(on, Ordering::Relaxed);
}

pub fn glass() -> bool {
    GLASS.load(Ordering::Relaxed)
}

/// How much of the rail's tint lies over the system's glass, light and dark: the system's glass stands for the
/// whole-machine preview's blurred, saturated wallpaper and its sheen, and this tint for the preview's glass fill
/// over them (`--lg-fill`: light 0.38, dark 0.42), so the rail reads as clear as the preview's.
pub const RAIL_GLASS_TINT: (f32, f32) = (0.38, 0.42);

/// The rail's ground (the side rail, the first-run wizard's steps): over the system's material a translucent
/// tint of it ([`RAIL_GLASS_TINT`], light to dark as the theme blends), else the tint itself, opaque.
pub fn rail_ground() -> Color32 {
    if glass() {
        let (light, dark) = RAIL_GLASS_TINT;
        c(C::Rail).gamma_multiply(light + (dark - light) * crate::palette::blend())
    } else {
        c(C::Rail)
    }
}

/// A raised surface's level above the page ground.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Level {
    Card,
    Float,
    Toast,
    Sheet,
}

impl Level {
    pub const ALL: [Level; 4] = [Level::Card, Level::Float, Level::Toast, Level::Sheet];

    /// Glass: a translucent ground with an accent wash at the top and a thread of light along the top edge.
    fn glass(self) -> bool {
        !matches!(self, Level::Card)
    }

    /// How far down the glass wash runs before it is gone (a sheet's stops 120 below its top; the others run
    /// their whole height).
    fn wash_reach(self) -> f32 {
        if self == Level::Sheet { 120.0 } else { f32::INFINITY }
    }
}

/// One soft shadow: y offset, blur, alpha (of the shadow ink).
#[derive(Clone, Copy)]
struct Soft {
    y: f32,
    blur: f32,
    a: u8,
}

const fn soft(y: f32, blur: f32, a: u8) -> Soft {
    Soft { y, blur, a }
}

const NONE: Soft = soft(0.0, 0.0, 0);

/// A level's two shadows in one appearance; a card lifted under the pointer takes `LIFT`.
fn shadows(level: Level, dark: bool) -> [Soft; 2] {
    match (level, dark) {
        (Level::Card, false) => [soft(1.0, 2.0, 8), soft(6.0, 20.0, 11)],
        (Level::Card, true) => [soft(1.0, 2.0, 77), soft(6.0, 20.0, 56)],
        (Level::Float, false) => [soft(2.0, 4.0, 13), soft(12.0, 28.0, 20)],
        (Level::Float, true) => [soft(12.0, 28.0, 89), NONE],
        (Level::Toast, false) => [soft(4.0, 12.0, 20), soft(1.0, 3.0, 10)],
        (Level::Toast, true) => [soft(4.0, 14.0, 102), NONE],
        (Level::Sheet, false) => [soft(18.0, 48.0, 46), soft(2.0, 6.0, 15)],
        (Level::Sheet, true) => [soft(18.0, 48.0, 140), NONE],
    }
}

/// A card lifted under the pointer, in both appearances.
const LIFT: [Soft; 2] = [soft(2.0, 4.0, 10), soft(12.0, 28.0, 20)];

/// The shadow ink at alpha `a` (0 to 255): a cool near-black in the light appearance, black in the dark.
pub fn ink(a: f32) -> Color32 {
    let t = blend();
    let light = Color32::from_rgba_unmultiplied(16, 24, 40, a.round().clamp(0.0, 255.0) as u8);
    let dark = Color32::from_rgba_unmultiplied(0, 0, 0, a.round().clamp(0.0, 255.0) as u8);
    mix(light, dark, t)
}

/// The shapes of a surface at `level` over `rect`, a card's hover (`hover` 0 to 1) lifting it.
pub fn shapes(rect: Rect, radius: Radius, level: Level, hover: f32) -> Vec<Shape> {
    let t = blend();
    let lerp = |a: f32, b: f32| a + (b - a) * t;
    let at = |dark: bool| {
        let s = shadows(level, dark);
        if level == Level::Card && hover > 0.0 {
            let m = |a: Soft, b: Soft| Soft { y: a.y + (b.y - a.y) * hover, blur: a.blur + (b.blur - a.blur) * hover, a: (a.a as f32 + (b.a as f32 - a.a as f32) * hover) as u8 };
            [m(s[0], LIFT[0]), m(s[1], LIFT[1])]
        } else {
            s
        }
    };
    let (l, d) = (at(false), at(true));
    let r = radius.egui();
    let mut out = Vec::new();
    for i in (0..2).rev() {
        let (a, b) = (l[i], d[i]);
        let alpha = lerp(a.a as f32, b.a as f32);
        if alpha < 0.5 {
            continue;
        }
        // A shadow that is there in one appearance only keeps the other's offset and blur while it fades.
        let (ay, ab) = if a.a == 0 { (b.y, b.blur) } else { (a.y, a.blur) };
        let (by, bb) = if b.a == 0 { (a.y, a.blur) } else { (b.y, b.blur) };
        out.push(drop(rect, r, lerp(ay, by), lerp(ab, bb), ink(alpha)));
    }
    if level.glass() {
        out.push(ground_with_run(rect, radius.px(), c(C::Glass), c(C::GlassTint), Color32::TRANSPARENT, level.wash_reach()));
    } else {
        out.push(egui::epaint::RectShape::filled(rect, r, c(C::Surface)).into());
    }
    out.push(rim(rect, r, c(C::Ring)));
    if level.glass() {
        let inset = radius.px().min(rect.width() / 2.0);
        if rect.width() > 2.0 * inset {
            out.push(Shape::line_segment([pos2(rect.left() + inset, rect.top() + 0.5), pos2(rect.right() - inset, rect.top() + 0.5)], egui::Stroke::new(1.0_f32, c(C::Hi))));
        }
    }
    out
}

/// A plate whose ground is `ground` exactly, in both appearances, inside a key's half-point edge (a QR code's
/// white plate: any reader must find pure white behind the modules).
pub fn plate(p: &egui::Painter, rect: Rect, radius: Radius, ground: Color32) {
    p.add(egui::epaint::RectShape::filled(rect, radius.egui(), ground));
    p.add(rim(rect, radius.egui(), c(C::KeyEdge)));
}

/// Paint a surface at `level`.
pub fn paint(p: &egui::Painter, rect: Rect, radius: Radius, level: Level, hover: f32) {
    p.extend(shapes(rect, radius, level, hover));
}

/// One soft drop shadow under `rect`: `y` down, blurred over `blur`.
pub fn drop(rect: Rect, radius: egui::CornerRadius, y: f32, blur: f32, colour: Color32) -> Shape {
    egui::epaint::RectShape::filled(rect.translate(egui::vec2(0.0, y)), radius, colour).with_blur_width(blur.max(0.0)).into()
}

/// A half-point rim just outside `rect`.
pub fn rim(rect: Rect, radius: egui::CornerRadius, colour: Color32) -> Shape {
    egui::epaint::RectShape::stroke(rect, radius, egui::Stroke::new(0.5_f32, colour), egui::StrokeKind::Outside).into()
}

/// A one-point shade inside the edge of a rounded `rect`, as a shape shifted by `dy` would leave it: `dy` below
/// zero shades the foot (thickest at the bottom, thinning to nothing up the sides), above zero the top. `soft`
/// fades it inward over its depth (a blurred shade) instead of a hard band.
pub fn inset_edge(rect: Rect, radius: f32, dy: f32, colour: Color32, soft: bool) -> Shape {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.0);
    let (cy, s) = if dy < 0.0 { (rect.bottom() - r, 1.0) } else { (rect.top() + r, -1.0) };
    // The half of the outline on the shaded side, left to right.
    let mut line = Vec::new();
    for k in 0..=12 {
        let a = (180.0 - 180.0 * k as f32 / 12.0_f32).to_radians();
        let cx = if k <= 6 { rect.left() + r } else { rect.right() - r };
        line.push(pos2(cx + r * a.cos(), cy + s * r * a.sin()));
        if k == 6 {
            line.push(pos2(rect.right() - r, cy + s * r));
        }
    }
    let inner = if soft { Color32::TRANSPARENT } else { colour };
    let mut mesh = egui::Mesh::default();
    for p in &line {
        mesh.colored_vertex(*p, colour);
        mesh.colored_vertex(*p + egui::vec2(0.0, dy), inner);
    }
    for i in 0..(line.len() as u32 - 1) {
        let a = 2 * i;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    Shape::mesh(mesh)
}

/// A solid key's ground: its colour, flat.
pub fn solid(rect: Rect, radius: f32, fill: Color32) -> Shape {
    egui::epaint::RectShape::filled(rect, egui::CornerRadius::same(radius.round().clamp(0.0, 255.0) as u8), fill).into()
}

/// A rounded rectangle of `ground` (one layer, its rim smoothed as every rectangle's), with a wash laid over it
/// from `top` at the top edge to `foot` `reach` below it (and `foot` on down from there): the ground keeps the
/// opacity it was given.
pub fn ground_with_run(rect: Rect, radius: f32, ground: Color32, top: Color32, foot: Color32, reach: f32) -> Shape {
    let base: Shape = egui::epaint::RectShape::filled(rect, egui::CornerRadius::same(radius.round().clamp(0.0, 255.0) as u8), ground).into();
    Shape::Vec(vec![base, run(rect, radius, top, foot, reach)])
}

/// The wash: bands across the rounded rectangle, at every point of its corners and where the wash ends, each band
/// as wide as the shape is there, so the colour runs exactly between the knots.
fn run(rect: Rect, radius: f32, top: Color32, foot: Color32, reach: f32) -> Shape {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0).max(0.0);
    let reach = reach.max(f32::EPSILON).min(rect.height().max(f32::EPSILON));
    let colour = |y: f32| mix(top, foot, ((y - rect.top()) / reach).clamp(0.0, 1.0));
    let mut ys: Vec<f32> = Vec::new();
    for k in 0..=8 {
        let a = (90.0 * k as f32 / 8.0_f32).to_radians();
        ys.push(rect.top() + r * (1.0 - a.cos()));
        ys.push(rect.bottom() - r * (1.0 - a.cos()));
    }
    ys.push(rect.top() + reach);
    ys.retain(|y| *y >= rect.top() && *y <= rect.bottom());
    ys.sort_by(|a, b| a.total_cmp(b));
    ys.dedup_by(|a, b| (*a - *b).abs() < 0.01);
    let inset = |y: f32| {
        let d = if y < rect.top() + r { rect.top() + r - y } else if y > rect.bottom() - r { y - (rect.bottom() - r) } else { 0.0 };
        r - (r * r - d.min(r) * d.min(r)).max(0.0).sqrt()
    };
    let mut mesh = egui::Mesh::default();
    for y in &ys {
        let i = inset(*y);
        mesh.colored_vertex(pos2(rect.left() + i, *y), colour(*y));
        mesh.colored_vertex(pos2(rect.right() - i, *y), colour(*y));
    }
    for k in 0..(ys.len().saturating_sub(1) as u32) {
        let a = 2 * k;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    Shape::mesh(mesh)
}

/// The page ground's faint accent wash: an ellipse 1100 across and 520 down around the top left corner of
/// `rect`, fading out at 62 % of the way to its edge.
pub fn ground_tint(p: &egui::Painter, rect: Rect) {
    let tint = c(C::GroundTint);
    if tint.a() == 0 {
        return;
    }
    let (rx, ry, stop) = (1100.0_f32, 520.0_f32, 0.62_f32);
    let area = Rect::from_min_size(rect.min, egui::vec2((rx * stop).min(rect.width()), (ry * stop).min(rect.height())));
    let (nx, ny) = (24u32, 16u32);
    let mut mesh = egui::Mesh::default();
    for j in 0..=ny {
        for i in 0..=nx {
            let q = pos2(area.left() + area.width() * i as f32 / nx as f32, area.top() + area.height() * j as f32 / ny as f32);
            let d = (((q.x - rect.left()) / rx).powi(2) + ((q.y - rect.top()) / ry).powi(2)).sqrt();
            mesh.colored_vertex(q, tint.gamma_multiply((1.0 - d / stop).max(0.0)));
        }
    }
    let w = nx + 1;
    for j in 0..ny {
        for i in 0..nx {
            let a = j * w + i;
            mesh.add_triangle(a, a + 1, a + w);
            mesh.add_triangle(a + 1, a + w + 1, a + w);
        }
    }
    p.add(Shape::mesh(mesh));
}

/// How far the light under the pointer reaches.
pub const GLOW_R: f32 = 64.0;

/// The soft light under the pointer on a key, a rail item or a clickable row ([`GLOW_R`] across): `colour` at the
/// pointer, fading to nothing at the edge of its reach, inside `rect` with corners `radius`. A white key takes the
/// accent at 4 %, a solid key white at 10 % (the caller's colour carries it).
pub fn glow(p: &egui::Painter, rect: Rect, radius: f32, at: egui::Pos2, colour: Color32) {
    if let Some(s) = glow_shape(rect, radius, at, colour) {
        p.add(s);
    }
}

/// [`glow`] as a shape (for a block that lays its grounds in a slot under its content).
pub fn glow_shape(rect: Rect, radius: f32, at: egui::Pos2, colour: Color32) -> Option<Shape> {
    let r = radius.clamp(0.0, 255.0).round() as u8;
    glow_shape_corners(rect, egui::CornerRadius::same(r), at, colour)
}

/// [`glow_shape`] over a block whose corners differ (a lit row's block, square where it meets a line).
pub fn glow_shape_corners(rect: Rect, corners: egui::CornerRadius, at: egui::Pos2, colour: Color32) -> Option<Shape> {
    if colour.a() == 0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return None;
    }
    let most = (rect.width() / 2.0).min(rect.height() / 2.0);
    // A point of the grid held inside the rounded rectangle, by the corner of its quarter.
    let inside = |q: egui::Pos2| {
        let left = q.x < rect.center().x;
        let up = q.y < rect.center().y;
        let r = f32::from(match (up, left) {
            (true, true) => corners.nw,
            (true, false) => corners.ne,
            (false, true) => corners.sw,
            (false, false) => corners.se,
        })
        .min(most);
        let c = egui::pos2(q.x.clamp(rect.left() + r, rect.right() - r), q.y.clamp(rect.top() + r, rect.bottom() - r));
        let d = q - c;
        if d.length() > r && r > 0.0 { c + d.normalized() * r } else { q }
    };
    let step = 6.0_f32;
    let nx = (rect.width() / step).ceil().max(1.0) as u32;
    let ny = (rect.height() / step).ceil().max(1.0) as u32;
    let mut mesh = egui::Mesh::default();
    for j in 0..=ny {
        for i in 0..=nx {
            let q = egui::pos2(rect.left() + rect.width() * i as f32 / nx as f32, rect.top() + rect.height() * j as f32 / ny as f32);
            let q = inside(q);
            let k = (1.0 - q.distance(at) / GLOW_R).max(0.0);
            mesh.colored_vertex(q, colour.gamma_multiply(k * k));
        }
    }
    let w = nx + 1;
    for j in 0..ny {
        for i in 0..nx {
            let a = j * w + i;
            mesh.add_triangle(a, a + 1, a + w);
            mesh.add_triangle(a + 1, a + w + 1, a + w);
        }
    }
    Some(Shape::mesh(mesh))
}

/// How strongly the light under the pointer shows on `id` now (0.18 s in and out; none while a list scrolls),
/// and where the pointer is.
pub fn glow_at(ui: &egui::Ui, id: egui::Id, rect: Rect, live: bool) -> (f32, egui::Pos2) {
    let at = ui.ctx().pointer_hover_pos();
    let on = live && at.map(|p| rect.contains(p)).unwrap_or(false) && !crate::scroll::moving(ui.ctx()) && crate::layer::on_top(ui, rect);
    let k = crate::motion::flag(ui.ctx(), id.with("glow"), on, 0.18 * crate::tokens::PACE);
    // Fading out, the light stays where the pointer left it.
    let last = id.with("glow-at");
    let pos = match at.filter(|_| on) {
        Some(p) => {
            ui.ctx().data_mut(|d| d.insert_temp(last, p));
            p
        }
        None => ui.ctx().data(|d| d.get_temp::<egui::Pos2>(last)).unwrap_or(rect.center()),
    };
    (k, pos)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A card is one opaque ground; a float, a toast and a sheet are glass: one ground layer at 97 % (the design
    /// draws 93 % over a blur this toolkit has not; at 97 % the page under it stays unreadable), with the accent
    /// wash at its top edge (nothing is stacked under or over to make the surface opaque).
    #[test]
    fn glass_keeps_its_translucency() {
        let rect = Rect::from_min_size(pos2(10.0, 10.0), egui::vec2(200.0, 160.0));
        for (level, want) in [(Level::Float, 0.97_f32), (Level::Toast, 0.97), (Level::Sheet, 0.97), (Level::Card, 1.0)] {
            fn walk(s: &Shape, grounds: &mut Vec<Color32>, tops: &mut Vec<u8>, top_y: f32) {
                match s {
                    Shape::Vec(v) => v.iter().for_each(|x| walk(x, grounds, tops, top_y)),
                    Shape::Rect(r) if r.fill.a() > 0 && r.blur_width == 0.0 => grounds.push(r.fill),
                    Shape::Mesh(m) => tops.extend(m.vertices.iter().filter(|v| (v.pos.y - top_y).abs() < 0.5).map(|v| v.color.a())),
                    _ => {}
                }
            }
            let (mut grounds, mut tops) = (Vec::new(), Vec::new());
            for s in shapes(rect, Radius::Sheet, level, 0.0) {
                walk(&s, &mut grounds, &mut tops, rect.top());
            }
            assert_eq!(grounds.len(), 1, "{level:?}: one ground layer");
            let a = grounds[0].a() as f32 / 255.0;
            assert!((a - want).abs() < 0.01, "{level:?}: ground opacity {a}, the design gives {want}");
            if level == Level::Card {
                assert!(tops.is_empty(), "{level:?}: no wash");
            } else {
                assert!(!tops.is_empty() && tops.iter().all(|&t| t == c(C::GlassTint).a()), "{level:?}: the wash starts at the top: {tops:?}");
            }
        }
    }

    /// A sheet's wash is gone 120 below its top; a menu's runs its whole height.
    #[test]
    fn a_sheets_wash_stops_short() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(300.0, 400.0));
        let Shape::Vec(v) = ground_with_run(rect, 22.0, Color32::WHITE, Color32::from_black_alpha(100), Color32::TRANSPARENT, 120.0) else { panic!("a ground and its wash") };
        let Shape::Mesh(m) = &v[1] else { panic!("the wash") };
        for vx in &m.vertices {
            if vx.pos.y >= 120.0 - 0.01 {
                assert_eq!(vx.color.a(), 0, "clear below the reach at {:?}", vx.pos);
            }
        }
        assert!(m.vertices.iter().any(|vx| (vx.pos.y - 60.0).abs() < 30.0 && vx.color.a() > 0) || m.vertices.iter().any(|vx| vx.pos.y < 30.0 && vx.color.a() > 0));
    }

    /// A solid key is its colour, opaque and flat: no halo, nothing beneath.
    #[test]
    fn a_solid_key_is_one_opaque_ground() {
        let rect = Rect::from_min_size(pos2(0.0, 0.0), egui::vec2(80.0, 34.0));
        let fill = Color32::from_rgb(0x3b, 0x82, 0xf6);
        let Shape::Rect(r) = solid(rect, 17.0, fill) else { panic!("one ground") };
        assert_eq!(r.fill, fill);
    }
}
