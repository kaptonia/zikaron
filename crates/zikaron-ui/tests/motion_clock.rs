use zikaron_ui::motion;

fn frame(ctx: &egui::Context, t: f64) -> f64 {
    let mut shown = 0.0;
    let input = egui::RawInput { time: Some(t), ..Default::default() };
    let _ = ctx.run_ui(input, |ui| {
        shown = motion::now(ui.ctx());
        ui.ctx().request_repaint();
    });
    shown
}

fn quiet(ctx: &egui::Context, t: f64) -> f64 {
    let mut shown = 0.0;
    let input = egui::RawInput { time: Some(t), ..Default::default() };
    let _ = ctx.run_ui(input, |ui| shown = motion::now(ui.ctx()));
    shown
}

#[test]
fn uneven_frames_step_evenly() {
    let ctx = egui::Context::default();
    let mut t = 1.0;
    let _ = frame(&ctx, t);
    let mut shown = Vec::new();
    for k in 0..120 {
        t += if k % 2 == 0 { 0.0035 } else { 0.0131 };
        shown.push((t, frame(&ctx, t)));
    }
    let steps: Vec<f64> = shown.windows(2).skip(40).map(|w| w[1].1 - w[0].1).collect();
    let (lo, hi) = steps.iter().fold((f64::MAX, f64::MIN), |(a, b), s| (a.min(*s), b.max(*s)));
    assert!(hi - lo < 0.004, "the clock's steps stay near one refresh (8.3 ms): {lo:.4} to {hi:.4}, where the frames came 3.5 and 13.1 ms apart");
    assert!(shown.iter().skip(40).all(|(t, s)| (t - s).abs() < 0.0084), "and within a refresh of the time");
}

#[test]
fn after_a_rest_or_a_stall_it_is_the_time() {
    let ctx = egui::Context::default();
    let mut t = 1.0;
    for _ in 0..30 {
        t += 1.0 / 120.0;
        let _ = frame(&ctx, t);
    }
    t += 0.04;
    assert_eq!(frame(&ctx, t), t, "a frame five refreshes late shows the time itself");
    t += 0.5;
    assert_eq!(frame(&ctx, t), t, "so does the first frame after a rest");
}

#[test]
fn a_frame_nothing_moving_asked_for_teaches_it_no_refresh() {
    let ctx = egui::Context::default();
    let mut t = 1.0;
    for _ in 0..10 {
        t += 0.04;
        assert_eq!(quiet(&ctx, t), t);
    }
    let mut worst: f64 = 0.0;
    for _ in 0..30 {
        t += 1.0 / 120.0;
        worst = worst.max((frame(&ctx, t) - t).abs());
    }
    assert!(worst < 0.0084, "the timer's spacing was not taken for the refresh: {worst}");
}

#[test]
fn a_run_of_close_frames_does_not_lock_the_refresh_short() {
    let ctx = egui::Context::default();
    let mut t = 1.0;
    let _ = frame(&ctx, t);
    for _ in 0..40 {
        t += 0.0035;
        let _ = frame(&ctx, t);
    }
    let mut shown = Vec::new();
    for k in 0..80 {
        t += if k % 2 == 0 { 0.0131 } else { 0.0035 };
        shown.push(frame(&ctx, t));
    }
    let steps: Vec<f64> = shown.windows(2).skip(40).map(|w| w[1] - w[0]).collect();
    let (lo, hi) = steps.iter().fold((f64::MAX, f64::MIN), |(a, b), s| (a.min(*s), b.max(*s)));
    assert!(hi - lo < 0.004, "steps near one refresh again: {lo:.4} to {hi:.4}");
}
