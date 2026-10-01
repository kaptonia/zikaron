//! Colors: one closed set of roles, each with a light and a dark value. Pages take colors only from here.
//!
//! Switching appearance blends the two tables: `skin` moves the blend from 0 (light) to 1 (dark) over
//! `tokens::SLOW`, and every `c(role)` in between is the mix of its two values, so the whole window changes
//! color at once instead of flashing.

use egui::Color32;
use std::sync::atomic::{AtomicU32, Ordering};

/// Color roles. Closed; the order is the tables' index.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum C {
    /// Page ground.
    Ground,
    /// Cards, lists, inputs and keys.
    Surface,
    /// Side rail.
    Rail,
    /// Segmented track and switch track.
    Segment,
    /// Hover, press and selected row grounds.
    Hover,
    Press,
    Sel,
    /// Hairlines: borders and the fainter rules between rows.
    Line,
    Line2,
    /// Three inks: body, secondary, quiet.
    Ink,
    Ink2,
    Ink3,
    /// Blue: primary keys, links, focus.
    Accent,
    AccentPress,
    AccentWash,
    AccentInk,
    Focus,
    /// Cinnabar: keys that commit to the ledger or chain.
    Pen,
    PenPress,
    PenEdge,
    PenWash,
    /// Green, amber and red, each with a wash, a dark ink, an edge; green and red also a box ground.
    Ok,
    OkWash,
    OkInk,
    OkEdge,
    OkBox,
    Warn,
    WarnWash,
    WarnInk,
    WarnEdge,
    Bad,
    BadWash,
    BadInk,
    BadEdge,
    BadBox,
    /// Type tags and neutral pills.
    Tag,
    /// Dashed drop edges and empty status circles.
    Dash,
    /// Drop zone ground at rest and hot.
    Drop,
    DropHot,
    /// Skeleton shimmer, low and high.
    Skel,
    SkelHi,
    /// The sheet scrim.
    Scrim,
    /// Switch knob.
    Knob,
    /// The drag veil over the whole window.
    Veil,
}

impl C {
    pub const ALL: [C; 44] = [
        C::Ground,
        C::Surface,
        C::Rail,
        C::Segment,
        C::Hover,
        C::Press,
        C::Sel,
        C::Line,
        C::Line2,
        C::Ink,
        C::Ink2,
        C::Ink3,
        C::Accent,
        C::AccentPress,
        C::AccentWash,
        C::AccentInk,
        C::Focus,
        C::Pen,
        C::PenPress,
        C::PenEdge,
        C::PenWash,
        C::Ok,
        C::OkWash,
        C::OkInk,
        C::OkEdge,
        C::OkBox,
        C::Warn,
        C::WarnWash,
        C::WarnInk,
        C::WarnEdge,
        C::Bad,
        C::BadWash,
        C::BadInk,
        C::BadEdge,
        C::BadBox,
        C::Tag,
        C::Dash,
        C::Drop,
        C::DropHot,
        C::Skel,
        C::SkelHi,
        C::Scrim,
        C::Knob,
        C::Veil,
    ];
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Premultiplied from an unpremultiplied color with alpha `a` (0..=255).
const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_premultiplied(
        ((r as u16 * a as u16 + 127) / 255) as u8,
        ((g as u16 * a as u16 + 127) / 255) as u8,
        ((b as u16 * a as u16 + 127) / 255) as u8,
        a,
    )
}

/// The light table, in `C::ALL` order.
pub const LIGHT: [Color32; 44] = [
    rgb(0xfa, 0xfa, 0xfc),
    rgb(0xff, 0xff, 0xff),
    rgb(0xf2, 0xf2, 0xf5),
    rgb(0xe9, 0xe9, 0xee),
    rgb(0xf5, 0xf7, 0xfc),
    rgb(0xec, 0xef, 0xf5),
    rgb(0xf3, 0xf7, 0xff),
    rgb(0xe5, 0xe5, 0xea),
    rgb(0xef, 0xef, 0xf3),
    rgb(0x1d, 0x1d, 0x1f),
    rgb(0x6e, 0x6e, 0x73),
    rgb(0xa1, 0xa1, 0xa6),
    rgb(0x3b, 0x82, 0xf6),
    rgb(0x2f, 0x74, 0xe6),
    rgb(0xe8, 0xf0, 0xfe),
    rgb(0x1d, 0x4e, 0xd8),
    rgba(59, 130, 246, 71),
    rgb(0xa8, 0x3a, 0x2c),
    rgb(0x94, 0x30, 0x1f),
    rgb(0xe3, 0xbd, 0xb7),
    rgb(0xff, 0xf6, 0xf5),
    rgb(0x2f, 0x9e, 0x44),
    rgb(0xe6, 0xf5, 0xe9),
    rgb(0x1f, 0x7a, 0x32),
    rgb(0xbf, 0xe3, 0xc6),
    rgb(0xf2, 0xfa, 0xf3),
    rgb(0xd9, 0xa1, 0x2c),
    rgb(0xfd, 0xf3, 0xdc),
    rgb(0x8a, 0x5a, 0x12),
    rgb(0xef, 0xd9, 0xa6),
    rgb(0xd1, 0x3b, 0x30),
    rgb(0xfd, 0xec, 0xeb),
    rgb(0xa5, 0x26, 0x1c),
    rgb(0xf0, 0xc9, 0xc4),
    rgb(0xff, 0xf6, 0xf5),
    rgb(0xee, 0xf1, 0xf6),
    rgb(0xc5, 0xcb, 0xd6),
    rgb(0xfb, 0xfc, 0xff),
    rgb(0xf3, 0xf7, 0xff),
    rgb(0xec, 0xec, 0xf0),
    rgb(0xf7, 0xf7, 0xfa),
    rgba(0, 0, 0, 77),
    rgb(0xff, 0xff, 0xff),
    rgba(250, 250, 252, 184),
];

/// The dark table, in `C::ALL` order.
pub const DARK: [Color32; 44] = [
    rgb(0x17, 0x18, 0x1b),
    rgb(0x1f, 0x20, 0x24),
    rgb(0x1b, 0x1c, 0x20),
    rgb(0x2a, 0x2b, 0x30),
    rgb(0x24, 0x26, 0x2c),
    rgb(0x2c, 0x2e, 0x35),
    rgb(0x1d, 0x2a, 0x42),
    rgb(0x34, 0x35, 0x3b),
    rgb(0x2a, 0x2b, 0x31),
    rgb(0xf2, 0xf2, 0xf5),
    rgb(0xa1, 0xa1, 0xaa),
    rgb(0x6e, 0x6e, 0x76),
    rgb(0x4f, 0x8f, 0xf7),
    rgb(0x3f, 0x7f, 0xe8),
    rgb(0x1e, 0x2a, 0x40),
    rgb(0x9c, 0xc0, 0xff),
    rgba(79, 143, 247, 89),
    rgb(0xc4, 0x53, 0x3f),
    rgb(0xad, 0x45, 0x33),
    rgb(0x6b, 0x32, 0x28),
    rgb(0x2e, 0x1b, 0x18),
    rgb(0x3f, 0xb2, 0x5a),
    rgb(0x17, 0x32, 0x22),
    rgb(0x7e, 0xdc, 0x95),
    rgb(0x25, 0x5a, 0x37),
    rgb(0x15, 0x26, 0x1b),
    rgb(0xe0, 0xa9, 0x3a),
    rgb(0x3a, 0x2e, 0x14),
    rgb(0xf1, 0xc6, 0x6b),
    rgb(0x5c, 0x48, 0x18),
    rgb(0xe5, 0x53, 0x4b),
    rgb(0x3b, 0x1b, 0x1a),
    rgb(0xff, 0x8a, 0x80),
    rgb(0x6b, 0x2b, 0x27),
    rgb(0x2b, 0x18, 0x17),
    rgb(0x2a, 0x2c, 0x32),
    rgb(0x44, 0x47, 0x4f),
    rgb(0x1c, 0x1e, 0x23),
    rgb(0x1e, 0x2b, 0x41),
    rgb(0x2a, 0x2b, 0x31),
    rgb(0x33, 0x34, 0x3a),
    rgba(0, 0, 0, 128),
    rgb(0xf2, 0xf2, 0xf5),
    rgba(23, 24, 27, 199),
];

/// The blend between the two tables: 0 light, 1 dark, stored as `f32` bits. Set once per frame by `skin`.
static BLEND: AtomicU32 = AtomicU32::new(0);

pub fn set_blend(b: f32) {
    BLEND.store(b.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
}

pub fn blend() -> f32 {
    f32::from_bits(BLEND.load(Ordering::Relaxed))
}

/// One color, from the current blend of the two tables.
pub fn c(k: C) -> Color32 {
    mix(LIGHT[k as usize], DARK[k as usize], blend())
}

/// Mix two colors channel by channel (premultiplied, as a CSS transition does).
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    if t <= 0.0 {
        return a;
    }
    if t >= 1.0 {
        return b;
    }
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()), l(a.a(), b.a()))
}

/// How many members of a table are pairwise distinct (complete is `C::ALL.len()`).
pub fn distinct(table: &[Color32]) -> usize {
    table.iter().enumerate().filter(|(i, a)| !table[..*i].contains(a)).count()
}

/// Text on solid blue, cinnabar, green and red grounds.
pub const ON_SOLID: Color32 = Color32::WHITE;
/// A QR code on its plate, the same in both appearances so any reader scans it: the dots in the light
/// appearance's ink, the three finder patterns in its accent, on white.
pub const QR_INK: Color32 = Color32::from_rgb(0x1d, 0x1d, 0x1f);
pub const QR_FINDER: Color32 = Color32::from_rgb(0x3b, 0x82, 0xf6);
pub const QR_PLATE: Color32 = Color32::WHITE;

/// The status tones a pill can take.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Ok,
    Warn,
    Bad,
    Grey,
    Blue,
}

impl Tone {
    pub const ALL: [Tone; 5] = [Tone::Ok, Tone::Warn, Tone::Bad, Tone::Grey, Tone::Blue];

    /// Ground of this tone.
    pub fn wash(self) -> Color32 {
        c(match self {
            Tone::Ok => C::OkWash,
            Tone::Warn => C::WarnWash,
            Tone::Bad => C::BadWash,
            Tone::Grey => C::Tag,
            Tone::Blue => C::AccentWash,
        })
    }

    /// Text of this tone.
    pub fn ink(self) -> Color32 {
        c(match self {
            Tone::Ok => C::OkInk,
            Tone::Warn => C::WarnInk,
            Tone::Bad => C::BadInk,
            Tone::Grey => C::Ink2,
            Tone::Blue => C::AccentInk,
        })
    }
}

/// Elevation: cards at rest, lifted on hover, floating layers (sheets, menus), and the toast (a small
/// floating strip: a shorter, lighter shadow than a sheet's). Each is a half-point ring plus soft
/// shadows; the dark table carries its own.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lift {
    Card,
    Hover,
    Float,
    Toast,
}

/// One soft shadow: y offset, blur, color.
#[derive(Clone, Copy, Debug)]
pub struct Soft {
    pub y: f32,
    pub blur: f32,
    pub color: Color32,
}

const fn soft(y: f32, blur: f32, r: u8, g: u8, b: u8, a: u8) -> Soft {
    Soft { y, blur, color: rgba(r, g, b, a) }
}

/// The ring color and shadows of an elevation, from the current blend.
pub fn lift(l: Lift) -> (Color32, [Soft; 2]) {
    let (ring_l, sh_l, ring_d, sh_d) = match l {
        Lift::Card => (
            rgba(0, 0, 0, 15),
            [soft(1.0, 2.0, 16, 24, 40, 10), soft(6.0, 16.0, 16, 24, 40, 10)],
            rgba(255, 255, 255, 15),
            [soft(1.0, 2.0, 0, 0, 0, 77), soft(0.0, 0.0, 0, 0, 0, 0)],
        ),
        Lift::Hover => (
            rgba(0, 0, 0, 15),
            [soft(2.0, 4.0, 16, 24, 40, 13), soft(12.0, 28.0, 16, 24, 40, 20)],
            rgba(255, 255, 255, 20),
            [soft(12.0, 28.0, 0, 0, 0, 89), soft(0.0, 0.0, 0, 0, 0, 0)],
        ),
        Lift::Float => (
            rgba(0, 0, 0, 20),
            [soft(18.0, 48.0, 16, 24, 40, 46), soft(2.0, 6.0, 16, 24, 40, 15)],
            rgba(255, 255, 255, 20),
            [soft(18.0, 48.0, 0, 0, 0, 140), soft(0.0, 0.0, 0, 0, 0, 0)],
        ),
        Lift::Toast => (
            rgba(0, 0, 0, 15),
            [soft(4.0, 12.0, 16, 24, 40, 20), soft(1.0, 3.0, 16, 24, 40, 10)],
            rgba(255, 255, 255, 20),
            [soft(4.0, 12.0, 0, 0, 0, 90), soft(0.0, 0.0, 0, 0, 0, 0)],
        ),
    };
    let t = blend();
    let s = |a: Soft, b: Soft| Soft { y: a.y + (b.y - a.y) * t, blur: a.blur + (b.blur - a.blur) * t, color: mix(a.color, b.color, t) };
    (mix(ring_l, ring_d, t), [s(sh_l[0], sh_d[0]), s(sh_l[1], sh_d[1])])
}

/// The canvas color the window is cleared to before anything is drawn (the page ground).
pub fn canvas() -> [f32; 4] {
    c(C::Ground).to_normalized_gamma_f32()
}
