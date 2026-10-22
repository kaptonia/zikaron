//! The pointing hand over anything that takes a click, decided in one place after the frame is drawn ([`finish`]):
//! when nothing in the frame chose a cursor (a text field's beam, a grip's, a resize) and the widget under the
//! pointer takes a click, is enabled and is not dragged, the cursor is the hand. A widget never has to remember to
//! ask for it. What takes a click only to swallow it (the ground under an overlay, a card's body, the page bar's
//! band) or offers nothing to press (the tab or segment already chosen) says so with [`sink`] and keeps the arrow.

use egui::{Context, CursorIcon, Id};

fn sinks_id() -> Id {
    Id::new("zikaron-hand-sinks")
}

/// `id` takes a click but offers nothing to press: no hand over it this frame.
pub fn sink(ctx: &Context, id: Id) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<Id>>(sinks_id()).push(id));
}

/// After the frame: the hand when the widget under the pointer takes a click and nothing else chose a cursor.
/// Clears the frame's sinks.
pub fn finish(ctx: &Context) {
    let sinks = ctx.data_mut(|d| std::mem::take(d.get_temp_mut_or_default::<Vec<Id>>(sinks_id())));
    if ctx.output(|o| o.cursor_icon) != CursorIcon::Default {
        return;
    }
    let hovered = ctx.interaction_snapshot(|i| i.hovered.clone());
    let takes = hovered.iter().any(|id| {
        !sinks.contains(id) && ctx.read_response(*id).is_some_and(|r| r.enabled() && r.sense.senses_click() && !r.sense.senses_drag())
    });
    if takes {
        ctx.set_cursor_icon(CursorIcon::PointingHand);
    }
}
