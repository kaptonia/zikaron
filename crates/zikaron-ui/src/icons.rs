//! Hand-drawn icons: no icon font is present everywhere, and a few simple shapes carry a native feel.
//!
//! No icon font dependency: each icon is a list of points in a unit square, scaled to the given rectangle
//! when drawn. One color per icon, no gradients.

/// The seven shapes. Closed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    /// Ledger: stacked lines.
    Ledger,
    /// Grant: a key outline.
    Grant,
    /// Kit: a box.
    Kit,
    /// Anchor: a shank and a curve.
    Anchor,
    /// Done: a check.
    Check,
    /// Warning: a triangle.
    Warn,
    /// Settings: a square gear.
    Gear,
}

impl Icon {
    pub const ALL: [Icon; 7] = [
        Icon::Ledger,
        Icon::Grant,
        Icon::Kit,
        Icon::Anchor,
        Icon::Check,
        Icon::Warn,
        Icon::Gear,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Icon::Ledger => "ledger",
            Icon::Grant => "grant",
            Icon::Kit => "kit",
            Icon::Anchor => "anchor",
            Icon::Check => "check",
            Icon::Warn => "warn",
            Icon::Gear => "gear",
        }
    }
}

/// An icon's strokes: each a list of points in the unit square (0..1 × 0..1). The only place icon shapes are
/// made.
pub fn path(i: Icon) -> Vec<Vec<(f32, f32)>> {
    match i {
        Icon::Ledger => vec![
            vec![(0.15, 0.2), (0.85, 0.2)],
            vec![(0.15, 0.4), (0.85, 0.4)],
            vec![(0.15, 0.6), (0.85, 0.6)],
            vec![(0.15, 0.8), (0.6, 0.8)],
        ],
        Icon::Grant => vec![
            vec![
                (0.35, 0.5),
                (0.3, 0.38),
                (0.38, 0.28),
                (0.5, 0.32),
                (0.52, 0.45),
                (0.42, 0.52),
                (0.35, 0.5),
            ],
            vec![(0.45, 0.5), (0.8, 0.82)],
            vec![(0.66, 0.68), (0.58, 0.78)],
        ],
        Icon::Kit => vec![
            vec![(0.15, 0.35), (0.85, 0.35), (0.85, 0.82), (0.15, 0.82), (0.15, 0.35)],
            vec![(0.15, 0.5), (0.85, 0.5)],
            vec![(0.38, 0.35), (0.38, 0.22), (0.62, 0.22), (0.62, 0.35)],
        ],
        Icon::Anchor => vec![
            vec![(0.5, 0.22), (0.5, 0.82)],
            vec![(0.32, 0.38), (0.68, 0.38)],
            vec![(0.2, 0.6), (0.24, 0.76), (0.5, 0.84), (0.76, 0.76), (0.8, 0.6)],
        ],
        Icon::Check => vec![vec![(0.2, 0.52), (0.42, 0.74), (0.8, 0.28)]],
        Icon::Warn => vec![
            vec![(0.5, 0.2), (0.86, 0.8), (0.14, 0.8), (0.5, 0.2)],
            vec![(0.5, 0.42), (0.5, 0.6)],
        ],
        Icon::Gear => vec![
            vec![
                (0.42, 0.16),
                (0.58, 0.16),
                (0.6, 0.28),
                (0.72, 0.34),
                (0.83, 0.28),
                (0.9, 0.42),
                (0.8, 0.5),
                (0.9, 0.58),
                (0.83, 0.72),
                (0.72, 0.66),
                (0.6, 0.72),
                (0.58, 0.84),
                (0.42, 0.84),
                (0.4, 0.72),
                (0.28, 0.66),
                (0.17, 0.72),
                (0.1, 0.58),
                (0.2, 0.5),
                (0.1, 0.42),
                (0.17, 0.28),
                (0.28, 0.34),
                (0.4, 0.28),
                (0.42, 0.16),
            ],
            vec![(0.42, 0.5), (0.5, 0.42), (0.58, 0.5), (0.5, 0.58), (0.42, 0.5)],
        ],
    }
}

/// Draw one icon, in one color.
pub fn draw(p: &egui::Painter, i: Icon, rect: egui::Rect, colour: egui::Color32) {
    strokes(p, &path(i), rect, colour);
}

/// Navigation glyphs for the side rail and its two bottom keys. Closed; shapes drawn on a sixteen-cell grid.
///
/// Separate from [`Icon`]: `Icon` says what kind of thing, `Glyph` marks a page. Both are closed and drawn by
/// the same painter.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Glyph {
    Home,
    /// Lock (the bottom of the side rail).
    Lock,
    Ledger,
    Anchor,
    Queue,
    Grant,
    Grants,
    Kit,
    Fingerprint,
    Depth,
    Others,
    Check,
    Watch,
    Verify,
    Vault,
    Delivery,
    Receipt,
    Refresh,
    Gear,
    /// Settings sections and page marks.
    Globe,
    Half,
    Key,
    Net,
    Mirror,
    Data,
    Info,
    Person,
    Swap,
    /// Small marks inside controls.
    Plus,
    Back,
    Fwd,
    Chev,
    Down,
    File,
    Inbox,
    Ok,
    No,
    Warn,
    Copy,
    Folder,
    Eye,
    Search,
    More,
    /// The date field's mark.
    Calendar,
}

impl Glyph {
    pub const ALL: [Glyph; 44] = [
        Glyph::Home,
        Glyph::Ledger,
        Glyph::Anchor,
        Glyph::Queue,
        Glyph::Grant,
        Glyph::Grants,
        Glyph::Kit,
        Glyph::Fingerprint,
        Glyph::Depth,
        Glyph::Others,
        Glyph::Check,
        Glyph::Watch,
        Glyph::Verify,
        Glyph::Vault,
        Glyph::Delivery,
        Glyph::Receipt,
        Glyph::Refresh,
        Glyph::Gear,
        Glyph::Lock,
        Glyph::Globe,
        Glyph::Half,
        Glyph::Key,
        Glyph::Net,
        Glyph::Mirror,
        Glyph::Data,
        Glyph::Info,
        Glyph::Person,
        Glyph::Swap,
        Glyph::Plus,
        Glyph::Back,
        Glyph::Fwd,
        Glyph::Chev,
        Glyph::Down,
        Glyph::File,
        Glyph::Inbox,
        Glyph::Ok,
        Glyph::No,
        Glyph::Warn,
        Glyph::Copy,
        Glyph::Folder,
        Glyph::Eye,
        Glyph::Search,
        Glyph::More,
        Glyph::Calendar,
    ];
}

/// A polyline on the sixteen-cell grid, mapped to the unit square.
fn g(points: &[(f32, f32)]) -> Vec<(f32, f32)> {
    points.iter().map(|(x, y)| (x / 16.0, y / 16.0)).collect()
}

/// An arc on the sixteen-cell grid (degrees, zero pointing right, clockwise positive), mapped to the unit
/// square.
fn arc(cx: f32, cy: f32, r: f32, from: f32, to: f32) -> Vec<(f32, f32)> {
    let n = 16;
    (0..=n)
        .map(|k| {
            let a = (from + (to - from) * k as f32 / n as f32).to_radians();
            ((cx + r * a.cos()) / 16.0, (cy + r * a.sin()) / 16.0)
        })
        .collect()
}

/// A gear outline on the sixteen-cell grid as one closed stroke: a body circle of radius `body`, `teeth`
/// square teeth evenly spaced, each `2 × half` wide with parallel sides reaching radius `tip`, arcs along the
/// body between them. Square teeth close to the body read as a gear even at sixteen cells (thin spokes with
/// gaps read as a sun).
fn gear(cx: f32, cy: f32, body: f32, tip: f32, half: f32, teeth: usize) -> Vec<(f32, f32)> {
    let step = 360.0 / teeth as f32;
    // The angles (from a tooth's centerline) where a tooth's side meets the body and tip circles.
    let at_body = (half / body).asin().to_degrees();
    let at_tip = (half / tip).asin().to_degrees();
    let pt = |r: f32, deg: f32| {
        let a = deg.to_radians();
        ((cx + r * a.cos()) / 16.0, (cy + r * a.sin()) / 16.0)
    };
    let mut out = Vec::new();
    for k in 0..teeth {
        // The first tooth points straight up; teeth are symmetric, one each up, down, left and right.
        let mid = -90.0 + step * k as f32;
        out.push(pt(body, mid - at_body));
        out.push(pt(tip, mid - at_tip));
        out.push(pt(tip, mid + at_tip));
        out.push(pt(body, mid + at_body));
        // The arc between teeth, along the body to the next tooth.
        let from = mid + at_body;
        let to = mid + step - at_body;
        let n = 4;
        for j in 1..n {
            out.push(pt(body, from + (to - from) * j as f32 / n as f32));
        }
    }
    out.push(out[0]);
    out
}

/// The strokes of a navigation glyph; the only place their shapes are made.
pub fn glyph_path(k: Glyph) -> Vec<Vec<(f32, f32)>> {
    match k {
        // A padlock: a square body with a curved shackle.
        Glyph::Lock => vec![
            g(&[(3.5, 7.0), (12.5, 7.0), (12.5, 14.0), (3.5, 14.0), (3.5, 7.0)]),
            arc(8.0, 7.0, 3.0, 180.0, 360.0),
        ],
        Glyph::Home => vec![g(&[(2.0, 8.0), (8.0, 3.0), (14.0, 8.0), (14.0, 14.0), (2.0, 14.0), (2.0, 8.0)])],
        Glyph::Ledger => vec![
            g(&[(3.0, 3.0), (13.0, 3.0), (13.0, 13.0), (3.0, 13.0), (3.0, 3.0)]),
            g(&[(5.0, 6.0), (11.0, 6.0)]),
            g(&[(5.0, 9.0), (11.0, 9.0)]),
        ],
        Glyph::Anchor => vec![
            g(&[(8.0, 2.0), (8.0, 11.0)]),
            g(&[(4.0, 7.0), (8.0, 11.0), (12.0, 7.0)]),
            g(&[(3.0, 14.0), (13.0, 14.0)]),
        ],
        Glyph::Queue => vec![
            g(&[(3.0, 4.0), (13.0, 4.0)]),
            g(&[(3.0, 8.0), (13.0, 8.0)]),
            g(&[(3.0, 12.0), (9.0, 12.0)]),
        ],
        Glyph::Grant => vec![g(&[(4.0, 12.0), (12.0, 4.0)]), g(&[(5.0, 4.0), (12.0, 4.0), (12.0, 11.0)])],
        Glyph::Grants => vec![
            g(&[(3.0, 4.0), (13.0, 4.0), (13.0, 12.0), (3.0, 12.0), (3.0, 4.0)]),
            g(&[(3.0, 7.0), (13.0, 7.0)]),
        ],
        Glyph::Kit => vec![
            g(&[(2.0, 5.0), (14.0, 5.0), (14.0, 13.0), (2.0, 13.0), (2.0, 5.0)]),
            g(&[(5.0, 5.0), (5.0, 3.0), (11.0, 3.0), (11.0, 5.0)]),
        ],
        Glyph::Fingerprint => vec![arc(8.0, 8.0, 6.0, -90.0, 0.0), arc(8.0, 8.0, 3.0, -90.0, 0.0), g(&[(8.0, 8.0), (8.0, 14.0)])],
        Glyph::Depth => vec![g(&[(2.0, 12.0), (6.0, 7.0), (9.0, 10.0), (14.0, 4.0)])],
        Glyph::Others => vec![arc(8.0, 6.0, 3.0, 0.0, 360.0), arc(8.0, 14.0, 6.0, 180.0, 360.0)],
        Glyph::Check => vec![g(&[(3.0, 8.0), (6.0, 11.0), (13.0, 4.0)])],
        Glyph::Watch => {
            let mut bell = arc(8.0, 6.0, 4.0, 180.0, 360.0);
            bell.extend(g(&[(12.0, 9.0), (13.0, 11.0), (3.0, 11.0), (4.0, 9.0), (4.0, 6.0)]));
            vec![bell, arc(8.0, 13.0, 2.0, 0.0, 180.0)]
        }
        Glyph::Verify => vec![
            g(&[(3.0, 3.0), (10.0, 3.0), (13.0, 6.0), (13.0, 13.0), (3.0, 13.0), (3.0, 3.0)]),
            g(&[(5.0, 9.0), (7.0, 11.0), (10.0, 8.0)]),
        ],
        Glyph::Vault => vec![
            g(&[(2.0, 3.0), (14.0, 3.0), (14.0, 13.0), (2.0, 13.0), (2.0, 3.0)]),
            g(&[(10.0, 8.0), (12.0, 8.0)]),
        ],
        Glyph::Delivery => vec![
            g(&[(2.0, 4.0), (11.0, 4.0), (11.0, 12.0), (2.0, 12.0), (2.0, 4.0)]),
            g(&[(11.0, 7.0), (14.0, 7.0), (14.0, 12.0), (11.0, 12.0)]),
        ],
        Glyph::Receipt => vec![
            g(&[(4.0, 2.0), (12.0, 2.0), (12.0, 14.0), (10.0, 13.0), (8.0, 14.0), (6.0, 13.0), (4.0, 14.0), (4.0, 2.0)]),
            g(&[(6.0, 6.0), (10.0, 6.0)]),
            g(&[(6.0, 9.0), (10.0, 9.0)]),
        ],
        Glyph::Refresh => vec![arc(8.0, 8.0, 5.0, 0.0, 315.0), g(&[(13.0, 2.0), (13.0, 5.0), (10.0, 5.0)])],
        Glyph::Gear => vec![gear(8.0, 8.0, 5.0, 7.0, 1.3, 6), arc(8.0, 8.0, 2.0, 0.0, 360.0)],
        Glyph::Globe => {
            let meridian: Vec<(f32, f32)> = arc(8.0, 8.0, 6.0, -90.0, 90.0).into_iter().map(|(x, y)| ((8.0 + (x * 16.0 - 8.0) * 0.45) / 16.0, y)).collect();
            vec![arc(8.0, 8.0, 6.0, 0.0, 360.0), g(&[(2.0, 8.0), (14.0, 8.0)]), g(&[(8.0, 2.0), (8.0, 14.0)]), meridian]
        }
        Glyph::Half => vec![
            arc(8.0, 8.0, 6.0, 0.0, 360.0),
            g(&[(8.0, 2.0), (8.0, 14.0)]),
            g(&[(8.0, 5.0), (11.0, 5.0)]),
            g(&[(8.0, 8.0), (13.5, 8.0)]),
            g(&[(8.0, 11.0), (11.0, 11.0)]),
        ],
        Glyph::Key => vec![arc(5.5, 8.0, 3.0, 0.0, 360.0), g(&[(8.5, 8.0), (14.0, 8.0), (14.0, 10.5)]), g(&[(11.5, 8.0), (11.5, 10.0)])],
        Glyph::Net => vec![
            arc(8.0, 3.5, 1.5, 0.0, 360.0),
            arc(3.5, 12.5, 1.5, 0.0, 360.0),
            arc(12.5, 12.5, 1.5, 0.0, 360.0),
            g(&[(7.2, 4.8), (4.3, 11.2)]),
            g(&[(8.8, 4.8), (11.7, 11.2)]),
            g(&[(5.0, 12.5), (11.0, 12.5)]),
        ],
        Glyph::Mirror => vec![g(&[(2.0, 4.0), (10.0, 4.0), (10.0, 12.0), (2.0, 12.0), (2.0, 4.0)]), g(&[(6.0, 2.0), (14.0, 2.0), (14.0, 10.0), (12.0, 10.0)])],
        Glyph::Data => {
            let squash = |cy: f32, v: Vec<(f32, f32)>| -> Vec<(f32, f32)> { v.into_iter().map(|(x, y)| (x, (cy + (y * 16.0 - cy) * 0.4) / 16.0)).collect() };
            vec![
                squash(4.0, arc(8.0, 4.0, 5.0, 0.0, 360.0)),
                g(&[(3.0, 4.0), (3.0, 12.0)]),
                g(&[(13.0, 4.0), (13.0, 12.0)]),
                squash(12.0, arc(8.0, 12.0, 5.0, 0.0, 180.0)),
                squash(8.0, arc(8.0, 8.0, 5.0, 0.0, 180.0)),
            ]
        }
        Glyph::Info => vec![arc(8.0, 8.0, 6.0, 0.0, 360.0), g(&[(8.0, 7.0), (8.0, 11.5)]), g(&[(8.0, 4.6), (8.0, 4.9)])],
        Glyph::Person => vec![arc(8.0, 5.5, 2.8, 0.0, 360.0), arc(8.0, 15.0, 5.5, 200.0, 340.0)],
        Glyph::Swap => vec![
            g(&[(3.0, 5.0), (12.0, 5.0)]),
            g(&[(10.0, 3.0), (12.0, 5.0), (10.0, 7.0)]),
            g(&[(13.0, 11.0), (4.0, 11.0)]),
            g(&[(6.0, 9.0), (4.0, 11.0), (6.0, 13.0)]),
        ],
        Glyph::Plus => vec![g(&[(8.0, 3.0), (8.0, 13.0)]), g(&[(3.0, 8.0), (13.0, 8.0)])],
        Glyph::Back => vec![g(&[(10.0, 3.0), (5.0, 8.0), (10.0, 13.0)])],
        Glyph::Fwd => vec![g(&[(6.0, 3.0), (11.0, 8.0), (6.0, 13.0)])],
        Glyph::Chev => vec![g(&[(6.0, 4.0), (10.0, 8.0), (6.0, 12.0)])],
        Glyph::Down => vec![g(&[(4.0, 6.0), (8.0, 10.0), (12.0, 6.0)])],
        Glyph::File => vec![g(&[(4.0, 2.0), (10.0, 2.0), (13.0, 5.0), (13.0, 14.0), (4.0, 14.0), (4.0, 2.0)]), g(&[(10.0, 2.0), (10.0, 5.0), (13.0, 5.0)])],
        Glyph::Inbox => vec![
            g(&[(2.0, 9.0), (4.0, 3.0), (12.0, 3.0), (14.0, 9.0), (14.0, 14.0), (2.0, 14.0), (2.0, 9.0)]),
            g(&[(2.0, 9.0), (6.0, 9.0), (7.0, 11.0), (9.0, 11.0), (10.0, 9.0), (14.0, 9.0)]),
        ],
        Glyph::Ok => vec![g(&[(3.0, 8.5), (6.5, 12.0), (13.0, 4.5)])],
        Glyph::No => vec![g(&[(4.0, 4.0), (12.0, 12.0)]), g(&[(12.0, 4.0), (4.0, 12.0)])],
        Glyph::Warn => vec![g(&[(8.0, 2.5), (14.5, 13.5), (1.5, 13.5), (8.0, 2.5)]), g(&[(8.0, 6.5), (8.0, 9.5)]), g(&[(8.0, 11.5), (8.0, 11.8)])],
        Glyph::Copy => vec![g(&[(5.0, 5.0), (14.0, 5.0), (14.0, 14.0), (5.0, 14.0), (5.0, 5.0)]), g(&[(2.0, 11.0), (2.0, 2.0), (11.0, 2.0)])],
        Glyph::Folder => vec![g(&[(2.0, 4.0), (6.0, 4.0), (7.5, 5.5), (14.0, 5.5), (14.0, 13.0), (2.0, 13.0), (2.0, 4.0)])],
        Glyph::Eye => vec![
            g(&[(1.5, 8.0), (4.0, 4.8), (8.0, 3.5), (12.0, 4.8), (14.5, 8.0), (12.0, 11.2), (8.0, 12.5), (4.0, 11.2), (1.5, 8.0)]),
            arc(8.0, 8.0, 2.2, 0.0, 360.0),
        ],
        Glyph::Search => vec![arc(7.0, 7.0, 4.5, 0.0, 360.0), g(&[(10.3, 10.3), (14.0, 14.0)])],
        Glyph::More => vec![g(&[(3.0, 8.0), (3.2, 8.0)]), g(&[(8.0, 8.0), (8.2, 8.0)]), g(&[(13.0, 8.0), (13.2, 8.0)])],
        Glyph::Calendar => vec![
            g(&[(2.5, 3.5), (13.5, 3.5), (13.5, 13.5), (2.5, 13.5), (2.5, 3.5)]),
            g(&[(2.5, 6.5), (13.5, 6.5)]),
            g(&[(5.5, 2.0), (5.5, 5.0)]),
            g(&[(10.5, 2.0), (10.5, 5.0)]),
        ],
    }
}

/// Draw a glyph turned by `turn` radians about the rectangle's center (the fold caret turns a quarter).
pub fn draw_glyph_turned(p: &egui::Painter, k: Glyph, rect: egui::Rect, colour: egui::Color32, turn: f32) {
    let (s, co) = turn.sin_cos();
    let c = rect.center();
    let stroke = egui::Stroke::new(crate::tokens::ICON_STROKE, colour);
    for path in glyph_path(k) {
        let pts: Vec<egui::Pos2> = path
            .iter()
            .map(|(x, y)| {
                let px = rect.left() + x * rect.width() - c.x;
                let py = rect.top() + y * rect.height() - c.y;
                egui::pos2(c.x + px * co - py * s, c.y + px * s + py * co)
            })
            .collect();
        if pts.len() >= 2 {
            p.add(egui::Shape::line(pts, stroke));
        }
    }
}

/// Draw a glyph of `size` centered in `rect`.
pub fn glyph_at(p: &egui::Painter, k: Glyph, center: egui::Pos2, size: f32, colour: egui::Color32) {
    draw_glyph(p, k, egui::Rect::from_center_size(center, egui::vec2(size, size)), colour);
}

/// Draw a navigation glyph with the same painter and pen as [`draw`].
pub fn draw_glyph(p: &egui::Painter, k: Glyph, rect: egui::Rect, colour: egui::Color32) {
    strokes(p, &glyph_path(k), rect, colour);
}

fn strokes(p: &egui::Painter, paths: &[Vec<(f32, f32)>], rect: egui::Rect, colour: egui::Color32) {
    let stroke = egui::Stroke::new(crate::tokens::ICON_STROKE, colour);
    for stroke_path in paths {
        let pts: Vec<egui::Pos2> = stroke_path
            .iter()
            .map(|(x, y)| egui::pos2(rect.left() + x * rect.width(), rect.top() + y * rect.height()))
            .collect();
        if pts.len() >= 2 {
            p.add(egui::Shape::line(pts, stroke));
        }
    }
}
