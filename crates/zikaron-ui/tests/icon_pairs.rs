//! Paired icons are one shape turned: back is forward mirrored, down is forward turned a quarter, so the two of a
//! pair are always one size and one stroke.

use zikaron_ui::icons::{glyph_path, Glyph};

fn points(k: Glyph) -> Vec<(f32, f32)> {
    glyph_path(k).into_iter().flatten().collect()
}

/// Width and height of a glyph's strokes in the unit square.
fn extent(k: Glyph) -> (f32, f32) {
    let p = points(k);
    let span = |f: fn(&(f32, f32)) -> f32| p.iter().map(f).fold(f32::MIN, f32::max) - p.iter().map(f).fold(f32::MAX, f32::min);
    (span(|q| q.0), span(|q| q.1))
}

fn same(a: &[(f32, f32)], b: &[(f32, f32)]) -> bool {
    let mut a: Vec<_> = a.to_vec();
    let mut b: Vec<_> = b.to_vec();
    let key = |p: &(f32, f32)| ((p.0 * 1000.0).round() as i32, (p.1 * 1000.0).round() as i32);
    a.sort_by_key(key);
    b.sort_by_key(key);
    a.len() == b.len() && a.iter().zip(&b).all(|(p, q)| (p.0 - q.0).abs() < 1e-4 && (p.1 - q.1).abs() < 1e-4)
}

#[test]
fn back_and_forward_are_one_shape_mirrored() {
    let fwd = points(Glyph::Fwd);
    let mirrored: Vec<_> = points(Glyph::Back).into_iter().map(|(x, y)| (1.0 - x, y)).collect();
    assert!(same(&fwd, &mirrored), "back mirrored is forward: {fwd:?} against {mirrored:?}");
    assert_eq!(extent(Glyph::Back), extent(Glyph::Fwd), "one size");
}

#[test]
fn open_and_shut_are_one_shape_turned() {
    let (w, h) = extent(Glyph::Chev);
    let (dw, dh) = extent(Glyph::Down);
    assert!((w - dh).abs() < 1e-4 && (h - dw).abs() < 1e-4, "down is the chevron a quarter turned: {:?} {:?}", (w, h), (dw, dh));
    let turned: Vec<_> = points(Glyph::Chev).into_iter().map(|(x, y)| (1.0 - y, x)).collect();
    assert!(same(&points(Glyph::Down), &turned));
    assert!(same(&points(Glyph::Chev), &points(Glyph::Fwd)), "the row's chevron is forward");
}
