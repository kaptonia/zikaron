//! Leaving layers: sheets, menus and full-window covers stay on screen for a moment after their owner stops
//! drawing them, fading (and shrinking a little) instead of vanishing.
//!
//! A layer that may leave is recorded each frame it is drawn: its shapes are kept. The first frame it is not
//! drawn, its kept shapes are drawn again as its entrance played backwards: over the same time, from full
//! opacity and size down to where it entered from, on the same curve run in reverse (so it leaves as gently as
//! it came, not most of the way in the first frame). Pages are not recorded: a page only enters.

use crate::motion::Curve;
use egui::{Context, Id, LayerId};

#[derive(Clone)]
struct Snap {
    layer: LayerId,
    shapes: Vec<egui::epaint::ClippedShape>,
    pass: u64,
    left_at: Option<f64>,
    dur: f32,
    /// Scale reached at the end of leaving, about `center`.
    to_scale: f32,
    center: egui::Pos2,
}

fn table_id() -> Id {
    Id::new("zikaron-leaving-layers")
}

/// Record the shapes a layer drew this pass (call right after drawing it). `dur` and `to_scale` are its
/// entrance's time and the scale it entered from.
pub fn keep(ctx: &Context, key: Id, layer: LayerId, dur: f32, to_scale: f32, center: egui::Pos2) {
    let shapes: Vec<egui::epaint::ClippedShape> = ctx.graphics(|g| g.get(layer).map(|l| l.all_entries().cloned().collect()).unwrap_or_default());
    let pass = ctx.cumulative_pass_nr();
    ctx.data_mut(|d| {
        let t = d.get_temp_mut_or_default::<Vec<(Id, Snap)>>(table_id());
        t.retain(|(k, _)| *k != key);
        t.push((key, Snap { layer, shapes, pass, left_at: None, dur, to_scale, center }));
    });
}

/// Draw the layers that have left and are still fading. Call once per frame, after everything else.
pub fn fade_out(ctx: &Context) {
    let pass = ctx.cumulative_pass_nr();
    let now = ctx.input(|i| i.time);
    let mut snaps: Vec<(Id, Snap)> = ctx.data(|d| d.get_temp::<Vec<(Id, Snap)>>(table_id())).unwrap_or_default();
    let mut keep_going = Vec::new();
    for (key, mut s) in snaps.drain(..) {
        if s.pass == pass {
            keep_going.push((key, s));
            continue;
        }
        let at = *s.left_at.get_or_insert(now);
        let p = ((now - at) as f32 / s.dur).clamp(0.0, 1.0);
        if p >= 1.0 {
            continue;
        }
        // The entrance backwards: at leaving time t it looks as it did at entering time dur - t.
        let alpha = Curve::Ease.at(1.0 - p);
        let scale = s.to_scale + (1.0 - s.to_scale) * alpha;
        let ghost = LayerId::new(s.layer.order, s.layer.id.with("leaving"));
        let painter = ctx.layer_painter(ghost);
        for cs in &s.shapes {
            let mut shape = cs.shape.clone();
            egui::epaint::shape_transform::adjust_colors(&mut shape, move |col| *col = col.gamma_multiply(alpha));
            painter.with_clip_rect(cs.clip_rect).add(shape);
        }
        ctx.set_transform_layer(ghost, egui::emath::TSTransform { scaling: scale, translation: s.center.to_vec2() * (1.0 - scale) });
        ctx.request_repaint();
        keep_going.push((key, s));
    }
    ctx.data_mut(|d| d.insert_temp(table_id(), keep_going));
}
