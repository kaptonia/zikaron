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
    /// A page with a folded corner: records.
    Doc,
    /// A shield with a check: verifying.
    Shield,
    /// Appearance.
    Sun,
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
    /// Language: the Chinese sign for writing and an A side by side, no frame.
    Lang,
    /// The command line: a prompt and a cursor.
    Terminal,
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
    pub const ALL: [Glyph; 49] = [
        Glyph::Home,
        Glyph::Ledger,
        Glyph::Doc,
        Glyph::Shield,
        Glyph::Sun,
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
        Glyph::Lang,
        Glyph::Terminal,
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

/// The one chevron every arrow is made from (back and forward, open and shut, down), pointing right on the
/// twenty-four-cell grid: the others are it mirrored or turned, so a pair is always one size and one stroke.
fn chevron() -> Vec<Vec<(f32, f32)>> {
    svg("m9 6 6 6-6 6")
}

/// Strokes with each point moved by `f` in the unit square (a mirror or a quarter turn about its middle).
fn turned(v: Vec<Vec<(f32, f32)>>, f: impl Fn((f32, f32)) -> (f32, f32)) -> Vec<Vec<(f32, f32)>> {
    v.into_iter().map(|s| s.into_iter().map(&f).collect()).collect()
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

/// Strokes from an SVG path on the twenty-four-cell grid the design draws its icons on, mapped to the unit
/// square: `M L H V C S Q A Z` and their relative forms, curves and arcs sampled to polylines. Each subpath is
/// one stroke.
fn svg(d: &str) -> Vec<Vec<(f32, f32)>> {
    let mut nums: Vec<f32> = Vec::new();
    let mut out: Vec<Vec<(f32, f32)>> = Vec::new();
    let mut cur: Vec<(f32, f32)> = Vec::new();
    let (mut x, mut y, mut sx, mut sy) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut last_c: Option<(f32, f32)> = None;
    let mut ops: Vec<(char, Vec<f32>)> = Vec::new();
    let mut op = ' ';
    let b = d.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let ch = b[i] as char;
        if ch.is_ascii_alphabetic() {
            if op != ' ' {
                ops.push((op, std::mem::take(&mut nums)));
            }
            op = ch;
            i += 1;
        } else if ch == '-' || ch == '.' || ch.is_ascii_digit() {
            let start = i;
            let mut dot = ch == '.';
            i += 1;
            while i < b.len() {
                let c = b[i] as char;
                if c.is_ascii_digit() {
                    i += 1;
                } else if c == '.' && !dot {
                    dot = true;
                    i += 1;
                } else {
                    break;
                }
            }
            nums.push(d[start..i].parse().unwrap_or(0.0));
        } else {
            i += 1;
        }
    }
    if op != ' ' {
        ops.push((op, nums));
    }
    let flush = |cur: &mut Vec<(f32, f32)>, out: &mut Vec<Vec<(f32, f32)>>| {
        if cur.len() >= 2 {
            out.push(std::mem::take(cur));
        } else {
            cur.clear();
        }
    };
    let cubic = |cur: &mut Vec<(f32, f32)>, p0: (f32, f32), c1: (f32, f32), c2: (f32, f32), p3: (f32, f32)| {
        for k in 1..=12 {
            let t = k as f32 / 12.0;
            let u = 1.0 - t;
            cur.push((
                u * u * u * p0.0 + 3.0 * u * u * t * c1.0 + 3.0 * u * t * t * c2.0 + t * t * t * p3.0,
                u * u * u * p0.1 + 3.0 * u * u * t * c1.1 + 3.0 * u * t * t * c2.1 + t * t * t * p3.1,
            ));
        }
    };
    for (op, n) in ops {
        let rel = op.is_ascii_lowercase();
        match op.to_ascii_uppercase() {
            'M' => {
                for (j, c) in n.chunks(2).enumerate() {
                    let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
                    x = ox + c[0];
                    y = oy + c[1];
                    if j == 0 {
                        flush(&mut cur, &mut out);
                        sx = x;
                        sy = y;
                    }
                    cur.push((x, y));
                }
                last_c = None;
            }
            'L' => {
                for c in n.chunks(2) {
                    let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
                    x = ox + c[0];
                    y = oy + c[1];
                    cur.push((x, y));
                }
                last_c = None;
            }
            'H' => {
                for v in n {
                    x = if rel { x + v } else { v };
                    cur.push((x, y));
                }
                last_c = None;
            }
            'V' => {
                for v in n {
                    y = if rel { y + v } else { v };
                    cur.push((x, y));
                }
                last_c = None;
            }
            'C' => {
                for c in n.chunks(6) {
                    let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
                    let c1 = (ox + c[0], oy + c[1]);
                    let c2 = (ox + c[2], oy + c[3]);
                    let p3 = (ox + c[4], oy + c[5]);
                    cubic(&mut cur, (x, y), c1, c2, p3);
                    last_c = Some(c2);
                    (x, y) = p3;
                }
            }
            'S' => {
                for c in n.chunks(4) {
                    let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
                    let c1 = last_c.map(|(cx, cy)| (2.0 * x - cx, 2.0 * y - cy)).unwrap_or((x, y));
                    let c2 = (ox + c[0], oy + c[1]);
                    let p3 = (ox + c[2], oy + c[3]);
                    cubic(&mut cur, (x, y), c1, c2, p3);
                    last_c = Some(c2);
                    (x, y) = p3;
                }
            }
            'Q' => {
                for c in n.chunks(4) {
                    let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
                    let q = (ox + c[0], oy + c[1]);
                    let p3 = (ox + c[2], oy + c[3]);
                    let c1 = (x + 2.0 / 3.0 * (q.0 - x), y + 2.0 / 3.0 * (q.1 - y));
                    let c2 = (p3.0 + 2.0 / 3.0 * (q.0 - p3.0), p3.1 + 2.0 / 3.0 * (q.1 - p3.1));
                    cubic(&mut cur, (x, y), c1, c2, p3);
                    last_c = None;
                    (x, y) = p3;
                }
            }
            'A' => {
                for c in n.chunks(7) {
                    let (ox, oy) = if rel { (x, y) } else { (0.0, 0.0) };
                    let (x2, y2) = (ox + c[5], oy + c[6]);
                    arc_to(&mut cur, (x, y), c[0], c[1], c[2], c[3] != 0.0, c[4] != 0.0, (x2, y2));
                    (x, y) = (x2, y2);
                }
                last_c = None;
            }
            'Z' => {
                cur.push((sx, sy));
                (x, y) = (sx, sy);
                flush(&mut cur, &mut out);
                cur.push((x, y));
                last_c = None;
            }
            _ => {}
        }
    }
    flush(&mut cur, &mut out);
    out.into_iter().map(|s| s.into_iter().map(|(x, y)| (x / 24.0, y / 24.0)).collect()).collect()
}

/// An SVG elliptical arc from `p0` to `p1`, sampled into `cur` (the endpoint form turned to center form).
#[allow(clippy::too_many_arguments)]
fn arc_to(cur: &mut Vec<(f32, f32)>, p0: (f32, f32), rx: f32, ry: f32, rot: f32, large: bool, sweep: bool, p1: (f32, f32)) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx == 0.0 || ry == 0.0 {
        cur.push(p1);
        return;
    }
    let (s, c) = rot.to_radians().sin_cos();
    let dx = (p0.0 - p1.0) / 2.0;
    let dy = (p0.1 - p1.1) / 2.0;
    let x1 = c * dx + s * dy;
    let y1 = -s * dx + c * dy;
    let l = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if l > 1.0 {
        rx *= l.sqrt();
        ry *= l.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut k = if den == 0.0 { 0.0 } else { (num / den).sqrt() };
    if large == sweep {
        k = -k;
    }
    let cx1 = k * rx * y1 / ry;
    let cy1 = -k * ry * x1 / rx;
    let cx = c * cx1 - s * cy1 + (p0.0 + p1.0) / 2.0;
    let cy = s * cx1 + c * cy1 + (p0.1 + p1.1) / 2.0;
    let ang = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let t1 = ang(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = ang((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
    if !sweep && dt > 0.0 {
        dt -= std::f32::consts::TAU;
    } else if sweep && dt < 0.0 {
        dt += std::f32::consts::TAU;
    }
    let n = ((dt.abs() / std::f32::consts::TAU) * 32.0).ceil().max(2.0) as usize;
    for j in 1..=n {
        let t = t1 + dt * j as f32 / n as f32;
        let (st, ct) = t.sin_cos();
        cur.push((cx + c * rx * ct - s * ry * st, cy + s * rx * ct + c * ry * st));
    }
}

/// A circle on the twenty-four-cell grid, mapped to the unit square.
fn ring(cx: f32, cy: f32, r: f32) -> Vec<(f32, f32)> {
    (0..=32).map(|k| {
        let a = std::f32::consts::TAU * k as f32 / 32.0;
        ((cx + r * a.cos()) / 24.0, (cy + r * a.sin()) / 24.0)
    }).collect()
}

/// A rounded rectangle on the twenty-four-cell grid, mapped to the unit square.
fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> Vec<(f32, f32)> {
    let d = format!(
        "M{} {}h{}a{r} {r} 0 0 1 {r} {r}v{}a{r} {r} 0 0 1 -{r} {r}h-{}a{r} {r} 0 0 1 -{r} -{r}v-{}a{r} {r} 0 0 1 {r} -{r}z",
        x + r, y, w - 2.0 * r, h - 2.0 * r, w - 2.0 * r, h - 2.0 * r
    );
    svg(&d).into_iter().next().unwrap_or_default()
}

/// The strokes of a navigation glyph; the only place their shapes are made.
pub fn glyph_path(k: Glyph) -> Vec<Vec<(f32, f32)>> {
    match k {
        Glyph::Lock => { let mut v = vec![rounded(5.0, 11.0, 14.0, 10.0, 2.0)]; v.extend(svg("M8 11V7a4 4 0 0 1 8 0v4")); v },
        Glyph::Home => svg("M3 10.5 12 3l9 7.5V20a1 1 0 0 1-1 1h-5v-6h-6v6H4a1 1 0 0 1-1-1z"),
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
        Glyph::Queue => svg("M4 6h16M4 12h16M4 18h10"),
        Glyph::Grant => { let mut v = vec![ring(8.0, 15.0, 4.0)]; v.extend(svg("m11 12 8-8M16 7l2 2M14 9l2 2")); v },
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
        Glyph::Watch => svg("M6 9a6 6 0 0 1 12 0c0 6 2.5 7.5 2.5 7.5h-17S6 15 6 9zM10 20a2 2 0 0 0 4 0"),
        Glyph::Verify => vec![
            g(&[(3.0, 3.0), (10.0, 3.0), (13.0, 6.0), (13.0, 13.0), (3.0, 13.0), (3.0, 3.0)]),
            g(&[(5.0, 9.0), (7.0, 11.0), (10.0, 8.0)]),
        ],
        Glyph::Vault => { let mut v = vec![rounded(3.0, 5.0, 18.0, 15.0, 2.0), ring(12.0, 12.5, 3.0)]; v.extend(svg("M12 9.5V8M7 20v1M17 20v1")); v },
        Glyph::Delivery => vec![
            g(&[(2.0, 4.0), (11.0, 4.0), (11.0, 12.0), (2.0, 12.0), (2.0, 4.0)]),
            g(&[(11.0, 7.0), (14.0, 7.0), (14.0, 12.0), (11.0, 12.0)]),
        ],
        Glyph::Receipt => vec![
            g(&[(4.0, 2.0), (12.0, 2.0), (12.0, 14.0), (10.0, 13.0), (8.0, 14.0), (6.0, 13.0), (4.0, 14.0), (4.0, 2.0)]),
            g(&[(6.0, 6.0), (10.0, 6.0)]),
            g(&[(6.0, 9.0), (10.0, 9.0)]),
        ],
        Glyph::Refresh => svg("M20 11a8 8 0 0 0-14.3-4.9L4 8M4 4v4h4M4 13a8 8 0 0 0 14.3 4.9L20 16M20 20v-4h-4"),
        Glyph::Gear => { let mut v = vec![ring(12.0, 12.0, 3.0)]; v.extend(svg("M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z")); v },
        Glyph::Globe => { let mut v = vec![ring(12.0, 12.0, 9.0)]; v.extend(svg("M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18")); v },
        Glyph::Half => vec![
            arc(8.0, 8.0, 6.0, 0.0, 360.0),
            g(&[(8.0, 2.0), (8.0, 14.0)]),
            g(&[(8.0, 5.0), (11.0, 5.0)]),
            g(&[(8.0, 8.0), (13.5, 8.0)]),
            g(&[(8.0, 11.0), (11.0, 11.0)]),
        ],
        Glyph::Lang => svg("M8 3v2.2M3 5.8h10M10.7 5.8c-.7 3.4-3 6-6.9 7.7M5.3 6.3c1.1 3.3 3.5 5.7 7.3 7.2M13.5 20.5l3.8-9.5 3.8 9.5M14.9 17.3h4.8"),
        Glyph::Terminal => { let mut v = vec![rounded(3.0, 4.0, 18.0, 16.0, 2.0)]; v.extend(svg("m7 9 3 3-3 3M13 15h4")); v },
        Glyph::Key => { let mut v = vec![ring(8.0, 8.0, 4.0)]; v.extend(svg("m11 11 9 9M17 17l2-2M15 15l2-2")); v },
        Glyph::Net => vec![
            arc(8.0, 3.5, 1.5, 0.0, 360.0),
            arc(3.5, 12.5, 1.5, 0.0, 360.0),
            arc(12.5, 12.5, 1.5, 0.0, 360.0),
            g(&[(7.2, 4.8), (4.3, 11.2)]),
            g(&[(8.8, 4.8), (11.7, 11.2)]),
            g(&[(5.0, 12.5), (11.0, 12.5)]),
        ],
        Glyph::Mirror => vec![g(&[(2.0, 4.0), (10.0, 4.0), (10.0, 12.0), (2.0, 12.0), (2.0, 4.0)]), g(&[(6.0, 2.0), (14.0, 2.0), (14.0, 10.0), (12.0, 10.0)])],
        Glyph::Data => { let rim: Vec<(f32, f32)> = (0..=32).map(|k| { let a = std::f32::consts::TAU * k as f32 / 32.0; ((12.0 + 8.0 * a.cos()) / 24.0, (6.0 + 3.0 * a.sin()) / 24.0) }).collect(); let mut v = vec![rim]; v.extend(svg("M4 6v12c0 1.7 3.6 3 8 3s8-1.3 8-3V6M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3")); v },
        Glyph::Info => { let mut v = vec![ring(12.0, 12.0, 9.0)]; v.extend(svg("M12 11v5M12 8v.01")); v },
        Glyph::Person => vec![arc(8.0, 5.5, 2.8, 0.0, 360.0), arc(8.0, 15.0, 5.5, 200.0, 340.0)],
        Glyph::Swap => vec![
            g(&[(3.0, 5.0), (12.0, 5.0)]),
            g(&[(10.0, 3.0), (12.0, 5.0), (10.0, 7.0)]),
            g(&[(13.0, 11.0), (4.0, 11.0)]),
            g(&[(6.0, 9.0), (4.0, 11.0), (6.0, 13.0)]),
        ],
        Glyph::Plus => svg("M12 5v14M5 12h14"),
        Glyph::Back => turned(chevron(), |(x, y)| (1.0 - x, y)),
        Glyph::Fwd | Glyph::Chev => chevron(),
        Glyph::Down => turned(chevron(), |(x, y)| (1.0 - y, x)),
        Glyph::File => svg("M7 3h7l5 5v12a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM14 3v5h5M9 13h7M9 17h5"),
        Glyph::Inbox => svg("M4 13h4l2 3h4l2-3h4M5.5 5h13L21 13v6a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-6z"),
        Glyph::Ok => svg("m5 12.5 4.5 4.5L19 7.5"),
        Glyph::No => svg("M6 6l12 12M18 6 6 18"),
        Glyph::Warn => vec![g(&[(8.0, 2.5), (14.5, 13.5), (1.5, 13.5), (8.0, 2.5)]), g(&[(8.0, 6.5), (8.0, 9.5)]), g(&[(8.0, 11.5), (8.0, 11.8)])],
        Glyph::Copy => { let mut v = vec![rounded(9.0, 9.0, 11.0, 11.0, 2.0)]; v.extend(svg("M5 15V5a1 1 0 0 1 1-1h9")); v },
        Glyph::Folder => svg("M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"),
        Glyph::Eye => vec![
            g(&[(1.5, 8.0), (4.0, 4.8), (8.0, 3.5), (12.0, 4.8), (14.5, 8.0), (12.0, 11.2), (8.0, 12.5), (4.0, 11.2), (1.5, 8.0)]),
            arc(8.0, 8.0, 2.2, 0.0, 360.0),
        ],
        Glyph::Search => { let mut v = vec![ring(11.0, 11.0, 7.0)]; v.extend(svg("m20 20-3.5-3.5")); v },
        Glyph::More => vec![ring(5.0, 12.0, 1.3), ring(12.0, 12.0, 1.3), ring(19.0, 12.0, 1.3)],
        Glyph::Calendar => { let mut v = vec![rounded(4.0, 5.0, 16.0, 15.0, 2.0)]; v.extend(svg("M4 10h16M9 3v4M15 3v4")); v },
        Glyph::Doc => svg("M7 3h7l5 5v12a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1zM14 3v5h5M9 13h7M9 17h5"),
        Glyph::Shield => svg("M12 3 5 6v6c0 4.5 3 7.7 7 9 4-1.3 7-4.5 7-9V6zM9 12l2 2 4-4"),
        Glyph::Sun => { let mut v = vec![ring(12.0, 12.0, 4.0)]; v.extend(svg("M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4")); v },
    }
}

/// Draw a glyph turned by `turn` radians about the rectangle's center (the fold caret turns a quarter).
pub fn draw_glyph_turned(p: &egui::Painter, k: Glyph, rect: egui::Rect, colour: egui::Color32, turn: f32) {
    let (s, co) = turn.sin_cos();
    let c = rect.center();
    let stroke = pen(rect.width(), colour);
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

/// The pen for a glyph drawn `side` points wide: `ICON_STROKE` of every twenty-four cells, as the design draws
/// its icons, never under one point.
pub fn pen(side: f32, colour: egui::Color32) -> egui::Stroke {
    egui::Stroke::new((crate::tokens::ICON_STROKE * side / 24.0).max(1.0), colour)
}

fn strokes(p: &egui::Painter, paths: &[Vec<(f32, f32)>], rect: egui::Rect, colour: egui::Color32) {
    let stroke = pen(rect.width(), colour);
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

/// Draw a glyph part way: its strokes laid end to end and drawn up to `part` (0 to 1) of their length (a check
/// drawn as a pen would).
pub fn glyph_drawn(p: &egui::Painter, k: Glyph, center: egui::Pos2, size: f32, colour: egui::Color32, part: f32) {
    let rect = egui::Rect::from_center_size(center, egui::vec2(size, size));
    let paths: Vec<Vec<egui::Pos2>> = glyph_path(k).iter().map(|sp| sp.iter().map(|(x, y)| egui::pos2(rect.left() + x * rect.width(), rect.top() + y * rect.height())).collect()).collect();
    let total: f32 = paths.iter().map(|pts| pts.windows(2).map(|w| w[0].distance(w[1])).sum::<f32>()).sum();
    let mut left = total * part.clamp(0.0, 1.0);
    let stroke = pen(size, colour);
    for pts in paths {
        if left <= 0.0 {
            break;
        }
        let mut out = vec![pts[0]];
        for w in pts.windows(2) {
            let d = w[0].distance(w[1]);
            if d <= left {
                out.push(w[1]);
                left -= d;
            } else {
                out.push(w[0] + (w[1] - w[0]) * (left / d.max(1e-6)));
                left = 0.0;
                break;
            }
        }
        if out.len() >= 2 {
            p.add(egui::Shape::line(out, stroke));
        }
    }
}
