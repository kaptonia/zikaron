use egui::{Context, CursorIcon, Id};

fn sinks_id() -> Id {
    Id::new("zikaron-hand-sinks")
}

pub fn sink(ctx: &Context, id: Id) {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Vec<Id>>(sinks_id()).push(id));
}

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
