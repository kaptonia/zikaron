use egui::{CornerRadius, Rect, pos2};

pub const GAP: f32 = 6.0;

pub const LINE_HALF: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    Edge,
    Line,
    Rule,
    Row,
}

pub fn block(slot: Rect, corner: f32, top: Bound, bottom: Bound) -> (Rect, CornerRadius) {
    let inset = |b: Bound| match b {
        Bound::Edge => GAP,
        Bound::Line | Bound::Rule | Bound::Row => 0.0,
    };
    let r = pos2(slot.left() + GAP, slot.top() + inset(top));
    let s = pos2(slot.right() - GAP, slot.bottom() - inset(bottom));
    let rect = Rect::from_min_max(r, s);
    (rect, corners(corner, rect.height(), top, bottom))
}

pub fn corners(corner: f32, height: f32, top: Bound, bottom: Bound) -> CornerRadius {
    let round = (corner - GAP).max(0.0).min(height / 2.0).round() as u8;
    let t = match top {
        Bound::Rule => 0,
        Bound::Edge | Bound::Line | Bound::Row => round,
    };
    let b = match bottom {
        Bound::Edge | Bound::Line | Bound::Rule | Bound::Row => round,
    };
    CornerRadius { nw: t, ne: t, sw: b, se: b }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{pos2, Rect};

    #[test]
    fn only_the_top_under_a_heading_rule_is_square() {
        let round = (14.0_f32 - GAP).round() as u8;
        let all = [Bound::Edge, Bound::Line, Bound::Rule, Bound::Row];
        for top in all {
            for bottom in all {
                let c = corners(14.0, 40.0, top, bottom);
                let want_top = if top == Bound::Rule { 0 } else { round };
                assert_eq!((c.nw, c.ne), (want_top, want_top), "{top:?} above");
                assert_eq!((c.sw, c.se), (round, round), "{bottom:?} below");
            }
        }
        let slot = Rect::from_min_max(pos2(0.0, 0.0), pos2(200.0, 40.0));
        let (one, c) = block(slot, 14.0, Bound::Edge, Bound::Edge);
        assert_eq!(c, CornerRadius::same(round), "one row in its card: round on all four");
        assert_eq!(one, Rect::from_min_max(pos2(GAP, GAP), pos2(200.0 - GAP, 40.0 - GAP)), "the gap kept on all four");
        let (first, c) = block(slot, 14.0, Bound::Rule, Bound::Line);
        assert_eq!(c, CornerRadius { nw: 0, ne: 0, sw: round, se: round }, "the first row under a head rule: square on top");
        assert_eq!((first.top(), first.bottom()), (0.0, 40.0), "on the lines' middles, as before");
        let (between, c) = block(slot, 14.0, Bound::Line, Bound::Line);
        assert_eq!(c, CornerRadius::same(round), "a row between two lines: round");
        assert_eq!((between.top(), between.bottom()), (0.0, 40.0), "on the lines' middles");
        let run = Rect::from_min_max(pos2(0.0, 0.0), pos2(200.0, 120.0));
        let (_, c) = block(run, 14.0, Bound::Rule, Bound::Edge);
        assert_eq!((c.nw, c.ne, c.sw, c.se), (0, 0, round, round), "a run: by its two ends");
        assert_eq!(corners(4.0, 40.0, Bound::Edge, Bound::Edge), CornerRadius::ZERO, "a corner under the gap: none");
    }
}
