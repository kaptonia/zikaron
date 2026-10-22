//! The lit block under a row (hovered or pressed, or a run of chosen rows), the one shape every list in a card, a
//! sheet or a menu lights with. It is one rounded rectangle, every corner round: the card's corner less the gap, so
//! the curves share the card's centres. Facing the card's edge (its sides, the first row's top, the last row's
//! bottom) it keeps one gap; facing the line between two rows it keeps none: its edge lies on the line's middle and
//! the line is drawn over it. A card with one row keeps the gap on all four sides. Rows chosen together that touch
//! are lit as one block over all of them (no line, no corner where they meet).

use egui::{CornerRadius, Rect, pos2};

/// The gap between the block and the card's edge, the same on every side that faces it.
pub const GAP: f32 = 6.0;

/// Half the hairline between two rows (drawn 1 thick, centred on the rows' boundary): a list whose rows stop at the
/// line's edge reaches its block out this far further, to the line's middle.
pub const LINE_HALF: f32 = 0.5;

/// What lies beyond one side of a row's slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    /// The card's own edge (or, in a sheet or a menu, the end of the list).
    Edge,
    /// A line between this row and the next: the block's edge lies on the line (its middle), under it.
    Line,
    /// The next row, with no line between (a menu's rows): the block reaches the slot's edge.
    Row,
}

/// The block for a row whose slot spans the card from edge to edge across and runs from what bounds it above to
/// what bounds it below (a line's middle, where there is one); `corner` is the card's corner. Returns the block and
/// its corners.
pub fn block(slot: Rect, corner: f32, top: Bound, bottom: Bound) -> (Rect, CornerRadius) {
    let inset = |b: Bound| match b {
        Bound::Edge => GAP,
        Bound::Line | Bound::Row => 0.0,
    };
    let r = pos2(slot.left() + GAP, slot.top() + inset(top));
    let s = pos2(slot.right() - GAP, slot.bottom() - inset(bottom));
    let rect = Rect::from_min_max(r, s);
    (rect, corners(corner, rect.height()))
}

/// The block's corners alone, for a list that lays its rows `GAP` in from the card's sides itself (a menu):
/// the card's corner less the gap, on every corner.
pub fn corners(corner: f32, height: f32) -> CornerRadius {
    CornerRadius::same((corner - GAP).max(0.0).min(height / 2.0).round() as u8)
}
