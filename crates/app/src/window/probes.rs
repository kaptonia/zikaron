//! This product's windowless drivers and probes: the window started at one of this product's pages or places,
//! and each probe that sets the window on a place, a page or a wizard step of this product before it draws.
//! The readings they return and the helpers they share are the window's own (`faces`); the places and steps
//! are this product's (`crate::nav`).

use super::*;

/// Open the window. Returns the exit code. The shipped build takes this path: starts at home, and opens the
/// first-run wizard by itself when the anchor key is missing or the author seat has no ledger yet.
pub fn run() -> i32 {
    run_at(Start::Place(Place::Home, false))
}

/// Open the window on a given page. The shipped binary has no path that calls it; only the test hooks do.
pub fn run_on(start: Page) -> i32 {
    run_at(Start::Page(start))
}

/// Open the window at a given place (the test hooks use it; the shipped build only goes through `run`).
pub fn run_at(start: Start) -> i32 {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(t(Key::AppName))
            // Matches the installed desktop entry (`zikaron-desk.desktop`) so the desktop shows the app's icon.
            .with_app_id("zikaron-desk")
            .with_icon(std::sync::Arc::new(egui::IconData {
                rgba: include_bytes!("../../assets/icon-256.rgba").to_vec(),
                width: 256,
                height: 256,
            }))
            .with_inner_size([W, H])
            .with_min_inner_size([MIN_W, MIN_H])
            // The side rail reaches the top and the window buttons float over it; no separate title bar.
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_shown(false),
        ..Default::default()
    };
    // The window system the platform interface chose (on Linux, X11 when there is an X display: files are
    // dropped on the window there).
    #[cfg(target_os = "linux")]
    let opts = {
        let mut opts = opts;
        if crate::platform::window_backend() == crate::platform::Backend::X11 {
            opts.event_loop_builder = Some(Box::new(|b| {
                use winit::platform::x11::EventLoopBuilderExtX11;
                b.with_x11();
            }));
        }
        opts
    };
    match eframe::run_native(
        t(Key::AppName),
        opts,
        Box::new(|cc| {
            let dressed = skin::dress(&cc.egui_ctx);
            // Settle the machine directory once; if settling fails the window still opens, and the error is shown
            // (never silent).
            let settled = crate::home::settle_machine();
            let mut shell = Shell::boot(dressed);
            if let Err(f) = settled {
                shell.faults.push(f);
            }
            // A request on the command-line channel wakes a frame (even when minimized or covered: the frame then
            // drains, and the channel's turn handles the request).
            let wake = cc.egui_ctx.clone();
            shell.door_waker = Some(std::sync::Arc::new(move || wake.request_repaint()));
            // The product builds its own preconditions: local data is sealed and the vault starts locked, so the
            // home opens, its lock is taken, the key is asked for and the home measured right after unlocking
            // (`Shell::after_unlock`). A vault already open here (only the test hooks start that way) opens it now.
            let root = crate::action::start(&mut shell);
            if let Some(l) = shell.speaks() {
                crate::lang::set(l);
            }
            crate::when::set(shell.settings.zone.unwrap_or(crate::when::Zone::Utc));
            let mut ux = Ux::default();
            match start {
                Start::Page(p) => shell.page = p,
                Start::Place(place, wizard) => {
                    if let Place::Page(p) = place {
                        shell.page = p;
                    }
                    ux.nav.land(crate::nav::history_of(place, shell.settings.role));
                    ux.wizard_forced = wizard;
                }
            }
            let typed = Typed { home: root, ..Default::default() };
            Ok(Box::new(fresh_win(shell, typed, ux)))
        }),
    ) {
        Ok(()) => 0,
        Err(e) => {
            // The standard-error line in the language set so far; the message in this machine's language.
            let why = e.to_string();
            let line = fill1(Key::SaidWindowFailed, &why);
            speak_this_machines_language();
            without_window(&line, fill1(Key::SaidNoWindowFailed, &why));
            1
        }
    }
}

/// Draw one page in a few windowless frames and measure its layout at a `width × height` viewport (no window,
/// no wall clock: frame time is given here). The caller prepares the shell and gets it back unchanged. The
/// first-run wizard and passcode gate are not opened here (this measures the page layer).
pub fn probe_face(ctx: &egui::Context, shell: Shell, place: Place, width: f32, height: f32) -> (Shell, FaceReading) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>, step: f64| -> egui::FullOutput {
        t += step;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), events, ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    let edges = |out: &egui::FullOutput| -> (f32, f32) {
        let widgets = ctx.viewport(|v| v.prev_pass.widgets.layers().flat_map(|(_, ws)| ws.iter().map(|w| w.rect.max.x)).fold(0.0_f32, f32::max));
        let mut text = 0.0_f32;
        for c in &out.shapes {
            if let egui::Shape::Text(s) = &c.shape {
                text = text.max(s.visual_bounding_rect().max.x);
            }
        }
        (widgets, text)
    };
    // Frames until the page has settled: the first sets the layout, the next set equal-cell heights, and the
    // entrance (at most 240 ms plus a 200 ms stagger) is over by the last.
    for _ in 0..5 {
        let _ = run(&mut win, Vec::new(), 0.1);
    }
    let out = run(&mut win, Vec::new(), 0.1);
    let (right_widget, right_text) = edges(&out);
    let texts: Vec<(egui::Rect, usize, bool)> = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(s) => Some((s.visual_bounding_rect(), s.galley.rows.len(), s.galley.job.justify)),
            _ => None,
        })
        .collect();
    let justified_texts = texts.iter().filter(|(_, _, j)| *j).count();
    let cells: Vec<CellFit> = zikaron_ui::probe::cells(ctx)
        .into_iter()
        .map(|r| {
            let inside: Vec<(egui::Rect, usize)> = texts
                .iter()
                .filter(|(t, _, _)| t.min.x >= r.min.x - 0.5 && t.min.x < r.max.x && r.y_range().contains(t.center().y))
                .map(|(t, n, _)| (*t, *n))
                .collect();
            CellFit {
                width: r.width(),
                over: inside.iter().filter(|(t, _)| t.width() > r.width() + 0.5 || t.max.x > r.max.x + 0.5).count(),
                wrapped: inside.iter().filter(|(_, n)| *n > 1).count(),
            }
        })
        .collect();
    let tiles = zikaron_ui::probe::tiles(ctx);
    let rows = zikaron_ui::probe::rows(ctx);
    let route = zikaron_ui::probe::route_drawn(ctx);
    let words: Vec<String> = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(s) => Some(s.galley.job.text.clone()),
            _ => None,
        })
        .collect();
    let (inner, inner_right, inner_head, inner_head_keys, inner_history, inner_route) = match rows.first() {
        None => (None, None, None, None, None, None),
        Some(r) => {
            let at = r.center();
            let _ = run(&mut win, vec![egui::Event::PointerMoved(at)], 0.1);
            let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
            let _ = run(&mut win, vec![press(true), press(false)], 0.1);
            let _ = run(&mut win, vec![egui::Event::PointerGone], 0.1);
            for _ in 0..4 {
                let _ = run(&mut win, Vec::new(), 0.1);
            }
            let out = run(&mut win, Vec::new(), 0.1);
            let shown = zikaron_ui::probe::inner_shown(ctx);
            let (a, b) = edges(&out);
            (
                Some(shown),
                Some(a.max(b)),
                zikaron_ui::probe::inner_head_said(ctx),
                Some(zikaron_ui::probe::head_keys_shown(ctx)),
                Some(zikaron_ui::probe::history_shown(ctx)),
                Some(zikaron_ui::probe::route_drawn(ctx)),
            )
        }
    };
    (
        win.shell,
        FaceReading { width, right_widget, right_text, rows: rows.len(), inner, inner_right, inner_head, inner_head_keys, inner_history, route, inner_route, texts: words, cells, justified_texts, tiles },
    )
}

/// The wake the window asks for after frames with no input: `place` drawn `frames` times, the clock starting at
/// `start` and stepping `step` seconds (each frame turns the shell's clock as the window's does), and the delay the
/// last frame asked before the next one, in seconds (`None`: it asked none). What the window wakes for while idle is
/// read off its own output, not computed beside it.
pub fn probe_wake(ctx: &egui::Context, shell: Shell, place: Place, start: f64, frames: usize, step: f64, width: f32, height: f32) -> (Shell, Option<f64>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut delay: Option<f64> = None;
    for k in 0..frames.max(1) {
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(start + step * k as f64), ..Default::default() };
        let out = ctx.run(input, |c| win.draw(c));
        delay = out.viewport_output.get(&egui::ViewportId::ROOT).map(|v| v.repaint_delay).filter(|d| *d < std::time::Duration::MAX).map(|d| d.as_secs_f64());
    }
    (win.shell, delay)
}

/// Walk a list page's history: settle, open the first row, go back (⌘[), go forward (⌘]). After each: the
/// route drawn and whether back and forward were shown.
pub fn probe_history(ctx: &egui::Context, shell: Shell, place: Place, width: f32, height: f32) -> (Shell, Vec<(String, bool, bool)>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), events, modifiers: egui::Modifiers::NONE, ..Default::default() };
        let _ = ctx.run(input, |c| win.draw(c));
    };
    let settle = |win: &mut Win, run: &mut dyn FnMut(&mut Win, Vec<egui::Event>)| {
        for _ in 0..5 {
            run(win, Vec::new());
        }
    };
    let said = || {
        let (b, f) = zikaron_ui::probe::history_shown(ctx);
        (zikaron_ui::probe::route_drawn(ctx), b, f)
    };
    let mut out = Vec::new();
    settle(&mut win, &mut run);
    out.push(said());
    if let Some(r) = zikaron_ui::probe::rows(ctx).first().copied() {
        let at = r.center();
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        run(&mut win, vec![egui::Event::PointerMoved(at)]);
        run(&mut win, vec![press(true), press(false)]);
        run(&mut win, vec![egui::Event::PointerGone]);
        settle(&mut win, &mut run);
        out.push(said());
        let key = |k: egui::Key| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND };
        run(&mut win, vec![key(egui::Key::OpenBracket)]);
        settle(&mut win, &mut run);
        out.push(said());
        run(&mut win, vec![key(egui::Key::CloseBracket)]);
        settle(&mut win, &mut run);
        out.push(said());
    }
    (win.shell, out)
}

/// Drive the window shell's own input over `place` in windowless frames, after a few empty ones to settle:
/// each step is one frame, `dt` seconds after the one before, with its events. After each step: the route
/// drawn and the window commands sent in that frame (`Debug` words of egui's viewport commands).
pub fn probe_shell_input(ctx: &egui::Context, shell: Shell, place: Place, steps: Vec<(f64, Vec<egui::Event>)>, width: f32, height: f32) -> (Shell, Vec<(String, Vec<String>)>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>, dt: f64| -> Vec<String> {
        t += dt;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), events, modifiers: egui::Modifiers::NONE, ..Default::default() };
        let out = ctx.run(input, |c| win.draw(c));
        out.viewport_output.values().flat_map(|v| v.commands.iter().map(|c| format!("{c:?}"))).collect()
    };
    for _ in 0..5 {
        run(&mut win, Vec::new(), 0.1);
    }
    let mut out = Vec::new();
    for (dt, events) in steps {
        let sent = run(&mut win, events, dt);
        out.push((zikaron_ui::probe::route_drawn(ctx), sent));
    }
    (win.shell, out)
}

/// Ring `notices` over `place` in a few windowless frames, with the first-run wizard open at `wizard` or not.
pub fn probe_rang(ctx: &egui::Context, shell: Shell, place: Place, notices: Vec<crate::watchx::Notice>, wizard: Option<crate::nav::Step>, width: f32, height: f32) -> (Shell, RangReading) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut attention = false;
    let mut run = |win: &mut Win| {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        let out = ctx.run(input, |c| win.draw(c));
        out.viewport_output.values().any(|v| v.commands.iter().any(|c| matches!(c, egui::ViewportCommand::RequestUserAttention(_))))
    };
    // Only this call's ringing is read: what rang before is dropped, and the wizard (when asked for)
    // is open from the first frame.
    win.shell.fresh.clear();
    win.shell.rung.clear();
    win.shell.attention = false;
    if let Some(step) = wizard {
        win.wizard_open(step);
    }
    for _ in 0..3 {
        let _ = run(&mut win);
    }
    win.shell.fresh = notices;
    win.shell.attention = true;
    for _ in 0..3 {
        attention |= run(&mut win);
    }
    let reading = RangReading { toast: win.toasts.showing().map(str::to_string), tone: win.toasts.showing_tone(), attention };
    (win.shell, reading)
}

/// The first-run wizard open at `step` over `place`, settled in windowless frames: every text drawn and every QR
/// plate.
pub fn probe_wizard_step(ctx: &egui::Context, shell: Shell, place: Place, step: crate::nav::Step, width: f32, height: f32) -> (Shell, Vec<Drawn>, Vec<egui::Rect>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win| {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    win.wizard_open(step);
    for _ in 0..6 {
        let _ = run(&mut win);
    }
    let out = run(&mut win);
    let texts = texts_of(&out);
    let plates = zikaron_ui::probe::qrs(ctx);
    (win.shell, texts, plates)
}

/// The wizard open at `from`, settled; then turned to `to` the way its "next" button turns it (`ux.wizard`),
/// and read every 20 ms for `frames` frames.
pub fn probe_wizard_walk(ctx: &egui::Context, shell: Shell, place: Place, from: crate::nav::Step, to: crate::nav::Step, frames: usize, width: f32, height: f32) -> (Shell, Vec<WizFrame>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, step: f64| {
        t += step;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    win.wizard_open(from);
    for _ in 0..8 {
        let _ = run(&mut win, 0.1);
    }
    win.ux.wizard = Some(to);
    let mut out = Vec::new();
    for _ in 0..frames {
        let o = run(&mut win, 0.02);
        out.push(WizFrame { step: win.ux.wizard.map(|s| format!("{s:?}")).unwrap_or_default(), texts: texts_of(&o), check: check_points(&o) });
    }
    (win.shell, out)
}

/// The wizard on the ledger step with its confirmation sheet up; then "create" pressed the way the sheet's
/// button presses it, and read every 20 ms for `frames` frames: how far the drawn check has gone.
pub fn probe_wizard_genesis(ctx: &egui::Context, shell: Shell, place: Place, frames: usize, width: f32, height: f32) -> (Shell, Vec<usize>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, step: f64| {
        t += step;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        (t, ctx.run(input, |c| win.draw(c)))
    };
    win.wizard_open(crate::nav::Step::Genesis);
    for _ in 0..6 {
        let _ = run(&mut win, 0.1);
    }
    win.ux.confirm_genesis = true;
    let mut now = 0.0;
    for _ in 0..6 {
        now = run(&mut win, 0.1).0;
    }
    win.ux.confirm_genesis = false;
    win.act(Action::Genesis { statement: win.ux.wiz_statement.clone() }, now);
    let mut out = Vec::new();
    for _ in 0..frames {
        let (_, o) = run(&mut win, 0.02);
        out.push(check_points(&o));
    }
    (win.shell, out)
}

/// The wizard on the ledger step; its confirmation sheet opened, then closed the way its "back" button closes
/// it. Every 20 ms from opening: the opacity (0–255) of the sheet's own note `said` in that frame, as drawn
/// (0 when not drawn).
pub fn probe_sheet_leave(ctx: &egui::Context, shell: Shell, place: Place, said: &str, open_frames: usize, close_frames: usize, width: f32, height: f32) -> (Shell, Vec<u8>, Vec<u8>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, step: f64| {
        t += step;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    let alpha_of = |o: &egui::FullOutput| -> u8 {
        o.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(s) if s.galley.job.text == said => {
                    s.galley.rows.iter().flat_map(|r| r.visuals.mesh.vertices.iter()).map(|v| v.color.a()).max().map(|a| (a as f32 * s.opacity_factor) as u8)
                }
                _ => None,
            })
            .max()
            .unwrap_or(0)
    };
    win.wizard_open(crate::nav::Step::Genesis);
    for _ in 0..6 {
        let _ = run(&mut win, 0.1);
    }
    win.ux.confirm_genesis = true;
    let opening: Vec<u8> = (0..open_frames).map(|_| alpha_of(&run(&mut win, 0.02))).collect();
    win.ux.confirm_genesis = false;
    let closing: Vec<u8> = (0..close_frames).map(|_| alpha_of(&run(&mut win, 0.02))).collect();
    (win.shell, opening, closing)
}

/// Leave `from` for `to` through the rail's own path (`go`), and read every frame from the first one after:
/// the texts drawn in the toolbar band.
pub fn probe_title_swap(ctx: &egui::Context, shell: Shell, from: Place, to: Place, width: f32, height: f32) -> (Shell, Vec<Vec<String>>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(from, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, step: f64| {
        t += step;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        (t, ctx.run(input, |c| win.draw(c)))
    };
    for _ in 0..5 {
        let _ = run(&mut win, 0.1);
    }
    let (now, _) = run(&mut win, 0.1);
    win.go(to, now);
    let mut frames = Vec::new();
    // Every 20 ms, covering a full crossfade (90 ms out, 120 ms in) and beyond.
    for _ in 0..15 {
        let (_, out) = run(&mut win, 0.02);
        let band: Vec<String> = texts_of(&out).into_iter().filter(|d| d.rect.top() < tk::TOOLBAR_H && d.rect.left() > tk::RAIL_W && d.text.trim().len() > 0).map(|d| d.text).collect();
        frames.push(band);
    }
    (win.shell, frames)
}

/// Drop `paths` on the window over `place` in a few windowless frames. A place without its own drop area (the
/// records page, say) leaves the drop to the window as a whole.
pub fn probe_drop(ctx: &egui::Context, shell: Shell, place: Place, paths: &[String], width: f32, height: f32) -> (Shell, DropReading) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, dropped: Vec<egui::DroppedFile>| {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), dropped_files: dropped, ..Default::default() };
        let _ = ctx.run(input, |c| win.draw(c));
    };
    for _ in 0..3 {
        run(&mut win, Vec::new());
    }
    run(&mut win, paths.iter().map(|p| egui::DroppedFile { path: Some(p.into()), ..Default::default() }).collect());
    for _ in 0..4 {
        run(&mut win, Vec::new());
    }
    let reading = DropReading {
        route: zikaron_ui::probe::route_drawn(ctx),
        sheet: win.ux.u3.new_anchor,
        taken: win.shell.content.as_ref().map(|c| c.subject.clone()),
        name: win.typed.work_note.clone(),
        verify_path: win.typed.vf_path.clone(),
        toast: win.toasts.showing().map(|s| s.to_string()),
        page_drops: win.ux.page_drops,
    };
    (win.shell, reading)
}

/// Open the wizard at `step` over `place`, then press Esc in a few windowless frames. With new words in hand
/// (made first), Esc asks first; this then leaves the way the prompt's leave button does.
pub fn probe_wizard_exit(ctx: &egui::Context, shell: Shell, place: Place, step: crate::nav::Step, width: f32, height: f32) -> (Shell, WizardExit) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> (f64, egui::FullOutput) {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), events, ..Default::default() };
        (t, ctx.run(input, |c| win.draw(c)))
    };
    for _ in 0..4 {
        let _ = run(&mut win, Vec::new());
    }
    let before = zikaron_ui::probe::route_drawn(ctx);
    win.wizard_open(step);
    for _ in 0..4 {
        let _ = run(&mut win, Vec::new());
    }
    let opened = win.ux.wizard.is_some();
    let offered = win.wizard_exit_shown();
    let wiz_from = win.ux.wiz_from.as_ref().map(|(r, s, p, _)| format!("{}:{s:?}:{p:?}", r.as_str())).unwrap_or_default();
    let had_words = win.shell.new_words.is_some();
    let esc = egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
    let _ = run(&mut win, vec![esc]);
    let mut last = None;
    for _ in 0..4 {
        last = Some(run(&mut win, Vec::new()));
    }
    let asked = had_words && win.ux.wiz_exit_ask;
    let said_void = last
        .as_ref()
        .map(|(_, out)| out.shapes.iter().any(|c| matches!(&c.shape, egui::Shape::Text(s) if s.galley.job.text.contains(t_void()))))
        .unwrap_or(false);
    if asked {
        let now = last.as_ref().map(|(n, _)| *n).unwrap_or(0.0);
        win.wizard_exit(now);
        for _ in 0..4 {
            let _ = run(&mut win, Vec::new());
        }
    }
    let reading = WizardExit {
        before,
        opened,
        after: zikaron_ui::probe::route_drawn(ctx),
        offered,
        still_open: win.ux.wizard.is_some(),
        asked,
        said_void,
        words_dropped: had_words && win.shell.new_words.is_none(),
        wiz_from,
        place_after: format!("{}:{:?}:{:?}", win.shell.settings.role.as_str(), win.ux.nav.stack, win.ux.nav.place),
    };
    (win.shell, reading)
}

/// Frames of the window over `place`, the wizard open at `wizard` when given, for drawing to an image: every
/// frame's output in order (textures arrive as deltas across them), the last one settled.
pub fn probe_frames(ctx: &egui::Context, shell: Shell, place: Place, wizard: Option<crate::nav::Step>, width: f32, height: f32, frames: usize) -> (Shell, Vec<egui::FullOutput>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    if let Some(step) = wizard {
        win.wizard_open(step);
    }
    let mut outs = Vec::new();
    for i in 0..frames.max(1) {
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(0.1 * (i + 1) as f64), ..Default::default() };
        outs.push(ctx.run(input, |c| win.draw(c)));
    }
    (win.shell, outs)
}

/// What the wizard's way out did through one first run ([`probe_wizard_way_out`]).
#[derive(Clone, Debug, Default)]
pub struct WayOut {
    /// At each phase (as it opened by itself, with the passcode set, with an identity too): whether a way out was
    /// offered, whether "Exit setup" was drawn in the sidebar, and the wizard's step.
    pub offered: Vec<bool>,
    pub exit_drawn: Vec<bool>,
    pub steps: Vec<String>,
    /// Whether "Exit setup" was clicked where it is drawn (the way out taken).
    pub left_by_click: bool,
    /// After leaving: whether the wizard is still open, the seat, stack and place, and whether that seat has a key.
    pub still_open: bool,
    pub place_after: String,
    pub seat_has_key_after: bool,
    /// A second window over the same machine, as after a restart: whether the wizard opened by itself, at which
    /// step, and whether it offers a way out.
    pub reopened: bool,
    pub reopened_step: String,
    pub reopened_offered: bool,
}

/// One window through a first run: the wizard opens by itself over the home page; `between(shell, 1)` then
/// `between(shell, 2)` change the machine the way the wizard's steps do (the caller drives the product's
/// actions), a few frames drawn after each; then "Exit setup" is clicked where it is drawn. Then a second window
/// over the same shell, as after a restart, opens the wizard by itself again.
pub fn probe_wizard_way_out(ctx: &egui::Context, shell: Shell, mut between: impl FnMut(&mut Shell, usize), width: f32, height: f32) -> (Shell, WayOut) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    let exit_word = t(Key::WizExit);
    let mut reading = WayOut::default();
    win.wizard_by_itself();
    let mut exit_at = None;
    for phase in 0..3 {
        if phase > 0 {
            between(&mut win.shell, phase);
        }
        let mut out = run(&mut win, Vec::new());
        for _ in 0..4 {
            out = run(&mut win, Vec::new());
        }
        reading.offered.push(win.wizard_exit_shown());
        exit_at = texts_of(&out).iter().find(|d| d.text == exit_word).map(|d| d.rect.center());
        reading.exit_drawn.push(exit_at.is_some());
        reading.steps.push(win.ux.wizard.map(|s| format!("{s:?}")).unwrap_or_default());
    }
    if let Some(at) = exit_at {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at), press(true), press(false)]);
        reading.left_by_click = true;
    }
    for _ in 0..4 {
        let _ = run(&mut win, Vec::new());
    }
    reading.still_open = win.ux.wizard.is_some();
    reading.place_after = format!("{}:{:?}:{:?}", win.shell.settings.role.as_str(), win.ux.nav.stack, win.ux.nav.place);
    reading.seat_has_key_after = !win.shell.seat_unseated() && win.shell.identities.as_ref().and_then(|r| r.now()).is_some();
    let shell = win.shell;
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut again = fresh_win(shell, Typed::default(), ux);
    again.faults_told = usize::MAX;
    again.wizard_by_itself();
    for _ in 0..4 {
        let _ = run(&mut again, Vec::new());
    }
    reading.reopened = again.ux.wizard.is_some();
    reading.reopened_step = again.ux.wizard.map(|s| format!("{s:?}")).unwrap_or_default();
    reading.reopened_offered = again.wizard_exit_shown();
    (again.shell, reading)
}

/// The ledger page ([`Place::View`] `Log`) settled: whether the "Create the ledger" key is drawn (outside the
/// table's rows); when it is, it is clicked where it is drawn, and the wizard's step after is read.
pub fn probe_ledger_set_up(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, bool, String) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::View(crate::nav::View::Log, 0), role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    let mut out = run(&mut win, Vec::new());
    for _ in 0..10 {
        out = run(&mut win, Vec::new());
    }
    // The key, not the genesis entry's own tag in a table row (which reads the same).
    let word = t(Key::WizGenesisTitle);
    let rows = zikaron_ui::probe::rows(ctx);
    let at = texts_of(&out).iter().find(|d| d.text == word && !rows.iter().any(|r| r.contains(d.rect.center()))).map(|d| d.rect.center());
    let Some(at) = at else { return (win.shell, false, String::new()) };
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    let _ = run(&mut win, vec![egui::Event::PointerMoved(at), press(true), press(false)]);
    for _ in 0..4 {
        let _ = run(&mut win, Vec::new());
    }
    let step = win.ux.wizard.map(|s| format!("{s:?}")).unwrap_or_default();
    (win.shell, true, step)
}

/// The first-run wizard at "create the ledger" on a place with no ledger: its "adopt existing ledger folder" key
/// pressed where it is drawn; in the sheet that opens, `dir` stands as the folder chosen (where the folder dialog's
/// answer lands: the dialog itself is the system's, not drawn here) and "adopt in place" pressed where it is drawn.
pub fn probe_wizard_adopt(ctx: &egui::Context, shell: Shell, dir: &str, width: f32, height: f32) -> (Shell, WizardAdopt) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    win.wizard_open(crate::nav::Step::Genesis);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    // Frames with these events, then eight without; the last one's output.
    let mut run = |win: &mut Win, events: Vec<Vec<egui::Event>>| -> egui::FullOutput {
        let mut out = None;
        for e in events.into_iter().chain(std::iter::repeat_n(Vec::new(), 8)) {
            clock += 0.1;
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events: e, ..Default::default() };
            out = Some(ctx.run(input, |c| win.draw(c)));
        }
        out.expect("a frame")
    };
    let settle = |win: &mut Win, run: &mut dyn FnMut(&mut Win, Vec<Vec<egui::Event>>) -> egui::FullOutput| run(win, Vec::new());
    // Press the text `word` where it is drawn (the topmost of its kind); whether it was drawn.
    let press = |win: &mut Win, run: &mut dyn FnMut(&mut Win, Vec<Vec<egui::Event>>) -> egui::FullOutput, out: &egui::FullOutput, word: &str| -> bool {
        let Some(at) = texts_of(out).iter().rev().find(|d| d.text == word).map(|d| d.rect.center()) else { return false };
        let click = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(win, vec![vec![egui::Event::PointerMoved(at)], vec![click(true), click(false)], vec![egui::Event::PointerGone]]);
        true
    };
    let mut reading = WizardAdopt::default();
    let out = settle(&mut win, &mut run);
    reading.key_drawn = press(&mut win, &mut run, &out, t(Key::AdoptGroup));
    let out = settle(&mut win, &mut run);
    reading.sheet_drawn = win.ux.confirm_adopt && texts_of(&out).iter().any(|d| d.text == t(Key::WizAdoptNote));
    win.ux.wiz_adopt = dir.to_string();
    let out = settle(&mut win, &mut run);
    let told = win.shell.faults.len();
    reading.adopt_pressed = press(&mut win, &mut run, &out, t(Key::DoAdopt));
    let out = settle(&mut win, &mut run);
    reading.step_after = win.ux.wizard.map(|s| format!("{s:?}")).unwrap_or_default();
    reading.done_after = crate::nav::Progress::of(&win.shell).done(crate::nav::Step::Genesis);
    reading.next_drawn_after = texts_of(&out).iter().any(|d| d.text == t(Key::WizardNext));
    reading.key_drawn_after = texts_of(&out).iter().any(|d| d.text == t(Key::AdoptGroup));
    reading.toast = win.toasts.showing().map(str::to_string);
    reading.refused = win.shell.faults.get(told..).and_then(|t| t.last()).map(|f| (f.human().to_string(), f.next()));
    (win.shell, reading)
}

/// The wizard on the identity step with new words in hand and their check showing (as after "I have copied
/// them"); then "back" pressed the way a click presses it (at the drawn key), twice: from the check, then
/// from the words. The shell needs new words in hand (`Action::NewIdentity`).
pub fn probe_wizard_back(ctx: &egui::Context, shell: Shell, place: Place, width: f32, height: f32) -> (Shell, WizardBack) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    let words = |win: &Win| win.shell.new_words.as_ref().map(|f| f.words().iter().map(|w| w.expose().to_string()).collect::<Vec<_>>());
    let before = words(&win);
    win.wizard_open(crate::nav::Step::Key);
    win.ux.id_confirming = true;
    // Press the drawn "back" key; whether it was drawn to be pressed.
    let mut press_back = |win: &mut Win| -> bool {
        let mut out = run(win, Vec::new());
        for _ in 0..6 {
            out = run(win, Vec::new());
        }
        let Some(at) = texts_of(&out).iter().find(|d| d.text == t(Key::WizPrev)).map(|d| d.rect.center()) else { return false };
        let _ = run(win, vec![egui::Event::PointerMoved(at)]);
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(win, vec![press(true), press(false)]);
        let _ = run(win, vec![egui::Event::PointerGone]);
        for _ in 0..4 {
            let _ = run(win, Vec::new());
        }
        true
    };
    let step_of = |win: &Win| win.ux.wizard.map(|s| format!("{s:?}")).unwrap_or_default();
    let pressed_on_check = press_back(&mut win);
    let step_after_check = step_of(&win);
    let checking_after = win.ux.id_confirming;
    let same_words = before.is_some() && words(&win) == before;
    let pressed_on_words = press_back(&mut win);
    let step_after_words = step_of(&win);
    let reading = WizardBack { pressed_on_check, step_after_check, checking_after, same_words, pressed_on_words, step_after_words };
    (win.shell, reading)
}

/// The first-run wizard opened at `step` (`gate`: false), or the passcode gate (`gate`: true, the vault locked);
/// then the other language's cell on its switch clicked the way a click presses it, when one is drawn (the gate
/// draws none: once the master key is made, the language is the one chosen last).
pub fn probe_lang_pick(ctx: &egui::Context, shell: Shell, place: Place, gate: bool, step: crate::nav::Step, width: f32, height: f32) -> (Shell, LangPick) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    if !gate {
        win.wizard_open(step);
    }
    let mut out = run(&mut win, Vec::new());
    for _ in 0..6 {
        out = run(&mut win, Vec::new());
    }
    let before = crate::lang::lang();
    let other = crate::lang::Lang::ALL.into_iter().find(|l| *l != before).unwrap_or(before);
    let at = texts_of(&out).iter().find(|d| d.text == other.label()).map(|d| d.rect.center());
    if let Some(at) = at {
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at)]);
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(&mut win, vec![press(true), press(false)]);
        let _ = run(&mut win, vec![egui::Event::PointerGone]);
        for _ in 0..4 {
            let _ = run(&mut win, Vec::new());
        }
    }
    let still_up = if gate { !win.shell.unlocked() } else { win.ux.wizard.is_some() };
    let reading = LangPick {
        drawn: at.is_some(),
        before: before.label().to_string(),
        after: crate::lang::lang().label().to_string(),
        machine_after: crate::machine::read_face().ok().and_then(|m| m.lang).map(|l| l.label().to_string()).unwrap_or_default(),
        still_up,
    };
    (win.shell, reading)
}

/// The passcode gate (the vault locked): three characters typed with nothing over it; the row emptied, the lock
/// screen's restore sheet opened over the gate the way its key opens it, a press on the sheet's title (no control
/// there, so nobody holds focus after it), and three characters typed again.
pub fn probe_typed_under(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, TypedUnder) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    let typed = || vec![egui::Event::Text("123".into())];
    for _ in 0..6 {
        let _ = run(&mut win, Vec::new());
    }
    let _ = run(&mut win, typed());
    let _ = run(&mut win, Vec::new());
    let bare = win.ux.pin.chars();
    win.ux.pin = crate::secret::Secret::default();
    win.bk_open(Bk::Restore(RestoreFrom::Locked));
    let mut out = run(&mut win, Vec::new());
    for _ in 0..6 {
        out = run(&mut win, Vec::new());
    }
    let title = texts_of(&out).iter().find(|d| d.text == t(Key::BackupRestoreTitle)).map(|d| d.rect.center());
    if let Some(at) = title {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at), press(true), press(false)]);
        for _ in 0..3 {
            let _ = run(&mut win, Vec::new());
        }
    }
    let _ = run(&mut win, typed());
    for _ in 0..3 {
        let _ = run(&mut win, Vec::new());
    }
    let reading = TypedUnder { bare, sheet_up: win.ux.bk.is_some(), pressed: title.is_some(), under: win.ux.pin.chars(), locked: !win.shell.unlocked() };
    (win.shell, reading)
}

/// The "with a credential" sheet opened over the command line's page and driven by the window's own input: the days cell
/// clicked and "400" typed, then cleared and "2" typed; the "does not expire" switch's title clicked, and "9"
/// typed into the days cell while it is on. Read after each: the cell, and the line under the switch.
pub fn probe_cli_sheet(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, CliSheet) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Settings(crate::nav::Section::Cli), role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    win.cli_open();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    let settle = |win: &mut Win, run: &mut dyn FnMut(&mut Win, Vec<egui::Event>) -> egui::FullOutput| {
        let mut out = run(win, Vec::new());
        for _ in 0..4 {
            out = run(win, Vec::new());
        }
        texts_of(&out)
    };
    let click = |at: egui::Pos2| {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        vec![egui::Event::PointerMoved(at), press(true), press(false)]
    };
    let key = |k: egui::Key| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
    let drawn = settle(&mut win, &mut run);
    let gas_said = drawn.iter().any(|d| d.text == t(Key::SetCliAloneGas));
    // The days cell is the field before the word "days": its middle sits half a cell and the gap left of the word.
    let unit = drawn.iter().find(|d| d.text == t(Key::CliLastsDays)).map(|d| d.rect);
    let toggle = drawn.iter().find(|d| d.text == t(Key::CliLastsNever)).map(|d| d.rect.center());
    let mut reading = CliSheet { gas_said, ..Default::default() };
    let (Some(unit), Some(toggle)) = (unit, toggle) else { return (win.shell, reading) };
    let cell = egui::pos2(unit.left() - tk::S2 - settings::CLI_CELL_W / 2.0, unit.center().y);
    let said_has = |texts: &[Drawn], s: &str| texts.iter().any(|d| d.text == s);
    let until = crate::lang::fill1(Key::CliAloneUntil, "");
    let _ = run(&mut win, click(cell));
    let _ = run(&mut win, vec![egui::Event::Text("400".into())]);
    let texts = settle(&mut win, &mut run);
    reading.days_typed = win.ux.cli_span[0].clone();
    reading.over_said = said_has(&texts, &crate::lang::filln(Key::CliLastsAtMost, &[t(Key::CliLastsDays), "365"]));
    let _ = run(&mut win, vec![key(egui::Key::Backspace), key(egui::Key::Backspace), key(egui::Key::Backspace)]);
    let _ = run(&mut win, vec![egui::Event::Text("2".into())]);
    let texts = settle(&mut win, &mut run);
    reading.days_fixed = win.ux.cli_span[0].clone();
    reading.expiry_said = texts.iter().any(|d| !until.is_empty() && d.text.starts_with(&until));
    let _ = run(&mut win, click(toggle));
    let _ = settle(&mut win, &mut run);
    reading.never_on = win.ux.cli_never;
    let _ = run(&mut win, click(cell));
    let _ = run(&mut win, vec![egui::Event::Text("9".into())]);
    let texts = settle(&mut win, &mut run);
    reading.days_while_never = win.ux.cli_span[0].clone();
    reading.quiet_while_never = !texts.iter().any(|d| !until.is_empty() && d.text.starts_with(&until));
    (win.shell, reading)
}

/// The ledger's check card, folded open on the records view (as the audit page opens it): every text drawn once the
/// fold has opened and the page settled.
pub fn probe_audit_card(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, Vec<Drawn>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::View(crate::nav::View::Log, 0), role));
    ux.wizard_asked = true;
    ux.u3.audit_open = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut out = None;
    for _ in 0..10 {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), ..Default::default() };
        out = Some(ctx.run(input, |c| win.draw(c)));
    }
    let drawn = out.as_ref().map(texts_of).unwrap_or_default();
    (win.shell, drawn)
}

/// Settings, network: the proxy section drawn (the saved proxy fills its own line as the page fills it); every
/// text drawn once the page has settled.
pub fn probe_proxy_section(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, Vec<String>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Settings(crate::nav::Section::Network), role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut out = egui::FullOutput::default();
    for _ in 0..12 {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), ..Default::default() };
        out = ctx.run(input, |c| win.draw(c));
    }
    let texts = texts_of(&out).into_iter().map(|d| d.text).collect();
    (win.shell, texts)
}

/// The proxy section with "custom" open and its cells filled as given (as if typed or pasted, no cell holding the
/// focus), a few frames drawn: the cells after (a whole address in the host cell split into them) and the words
/// drawn (each wrong cell's reason, whether a password is kept). The password cell is handed back as it is.
pub fn probe_proxy_cells(ctx: &egui::Context, shell: Shell, cells: crate::proxycells::Cells, width: f32, height: f32) -> (Shell, crate::proxycells::Cells, Vec<String>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Settings(crate::nav::Section::Network), role));
    ux.wizard_asked = true;
    ux.proxy_manual = true;
    ux.proxy_seeded = true;
    let typed = Typed { proxy: cells, ..Typed::default() };
    let mut win = fresh_win(shell, typed, ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut out = egui::FullOutput::default();
    for _ in 0..8 {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), ..Default::default() };
        out = ctx.run(input, |c| win.draw(c));
    }
    let texts = texts_of(&out).into_iter().map(|d| d.text).collect();
    let after = win.typed.proxy.clone();
    (win.shell, after, texts)
}

/// How text reaches the proxy's host cell in [`probe_proxy_typed`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostInput {
    /// One character a frame, as a person types.
    Typed,
    /// The whole text in one paste.
    Pasted,
    /// One character a frame, each as a paste (as some input tools put text in).
    PastedByChar,
}

/// The proxy section with "custom" open and `cells` filled; the host cell clicked, then `text` given to it as
/// `how` says (a frame drawn after each character, or after the one paste), then Tab pressed to leave it. Read:
/// the cells after each frame of input, and the cells once the host cell is left and the page has settled.
pub fn probe_proxy_typed(ctx: &egui::Context, shell: Shell, cells: crate::proxycells::Cells, text: &str, how: HostInput, width: f32, height: f32) -> (Shell, Vec<crate::proxycells::Cells>, crate::proxycells::Cells) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Settings(crate::nav::Section::Network), role));
    ux.wizard_asked = true;
    ux.proxy_manual = true;
    ux.proxy_seeded = true;
    let typed = Typed { proxy: cells, ..Typed::default() };
    let mut win = fresh_win(shell, typed, ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    let mut out = run(&mut win, Vec::new());
    for _ in 0..6 {
        out = run(&mut win, Vec::new());
    }
    // The host cell is where its hint is drawn (the hint may end in "…" when the cell is narrow).
    let hint: String = t(Key::ProxyHint).chars().take(4).collect();
    let Some(at) = texts_of(&out).iter().find(|d| d.text.starts_with(&hint)).map(|d| d.rect.center()) else {
        let now = win.typed.proxy.clone();
        return (win.shell, Vec::new(), now);
    };
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    let _ = run(&mut win, vec![egui::Event::PointerMoved(at), press(true), press(false)]);
    let _ = run(&mut win, Vec::new());
    let mut steps = Vec::new();
    match how {
        HostInput::Typed => {
            for ch in text.chars() {
                let _ = run(&mut win, vec![egui::Event::Text(ch.to_string())]);
                steps.push(win.typed.proxy.clone());
            }
        }
        HostInput::PastedByChar => {
            for ch in text.chars() {
                let _ = run(&mut win, vec![egui::Event::Paste(ch.to_string())]);
                steps.push(win.typed.proxy.clone());
            }
        }
        HostInput::Pasted => {
            let _ = run(&mut win, vec![egui::Event::Paste(text.to_string())]);
            steps.push(win.typed.proxy.clone());
        }
    }
    let tab = egui::Event::Key { key: egui::Key::Tab, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
    let _ = run(&mut win, vec![tab]);
    for _ in 0..4 {
        let _ = run(&mut win, Vec::new());
    }
    let after = win.typed.proxy.clone();
    (win.shell, steps, after)
}

/// The window over `place`, then the shell's page set from outside the window (`Action::Show`, as a link or a
/// command-line request sets it) and a few frames drawn: the route drawn before and after, and the shell's page
/// after (the window's following never moves it back).
pub fn probe_show_follow(ctx: &egui::Context, shell: Shell, place: Place, page: Page, width: f32, height: f32) -> (Shell, String, String, String) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win| {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), ..Default::default() };
        let _ = ctx.run(input, |c| win.draw(c));
    };
    for _ in 0..4 {
        run(&mut win);
    }
    let before = zikaron_ui::probe::route_drawn(ctx);
    let _ = crate::action::apply(&mut win.shell, Action::Show(page));
    for _ in 0..4 {
        run(&mut win);
    }
    let after = zikaron_ui::probe::route_drawn(ctx);
    let page_after = format!("{:?}", win.shell.page);
    (win.shell, before, after, page_after)
}

/// Walk the rail in windowless frames, each item in a window started on home: press the item where it is
/// drawn; on its page open the first row; when that row is a page of the same view, go back, open it again and
/// press the same item. The route drawn after each move.
pub fn probe_rail_walk(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, Vec<RailWalk>) {
    let role = shell.settings.role;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, modifiers: egui::Modifiers::NONE, ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    let settle = |win: &mut Win, run: &mut dyn FnMut(&mut Win, Vec<egui::Event>) -> egui::FullOutput| {
        let mut out = run(win, Vec::new());
        for _ in 0..5 {
            out = run(win, Vec::new());
        }
        out
    };
    let click = |win: &mut Win, run: &mut dyn FnMut(&mut Win, Vec<egui::Event>) -> egui::FullOutput, at: egui::Pos2| {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(win, vec![egui::Event::PointerMoved(at)]);
        let _ = run(win, vec![press(true), press(false)]);
        let _ = run(win, vec![egui::Event::PointerGone]);
    };
    let labels: Vec<String> = crate::nav::rail(role).iter().flat_map(|g| g.items.iter().map(|i| t(i.name).to_string())).collect();
    let mut shell = shell;
    let mut walk = Vec::new();
    for label in labels {
        let mut ux = Ux::default();
        ux.nav.land(crate::nav::history_of(Place::Home, role));
        ux.wizard_asked = true;
        let mut win = fresh_win(shell, Typed::default(), ux);
        win.faults_told = usize::MAX;
        let mut w = RailWalk { item: label.clone(), ..Default::default() };
        let on_rail = |out: &egui::FullOutput| texts_of(out).iter().find(|d| d.text == label && d.rect.center().x < tk::RAIL_W).map(|d| d.rect.center());
        let out = settle(&mut win, &mut run);
        if let Some(item) = on_rail(&out) {
            w.pressed = true;
            click(&mut win, &mut run, item);
            let _ = settle(&mut win, &mut run);
            w.entered = zikaron_ui::probe::route_drawn(ctx);
            let view = win.ux.nav.stack;
            if let Some(row) = zikaron_ui::probe::rows(ctx).first().map(|r| r.center()) {
                click(&mut win, &mut run, row);
                let _ = settle(&mut win, &mut run);
                let there = zikaron_ui::probe::route_drawn(ctx);
                if win.ux.nav.stack != view {
                    w.link = Some(there);
                } else {
                    w.detail = Some(there);
                    let key = egui::Event::Key { key: egui::Key::OpenBracket, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND };
                    let _ = run(&mut win, vec![key]);
                    let _ = settle(&mut win, &mut run);
                    w.out = Some(zikaron_ui::probe::route_drawn(ctx));
                    if let Some(row) = zikaron_ui::probe::rows(ctx).first().map(|r| r.center()) {
                        click(&mut win, &mut run, row);
                        let out = settle(&mut win, &mut run);
                        if let Some(item) = on_rail(&out) {
                            click(&mut win, &mut run, item);
                            let _ = settle(&mut win, &mut run);
                            w.rail_again = Some(zikaron_ui::probe::route_drawn(ctx));
                        }
                    }
                }
            }
        }
        walk.push(w);
        shell = win.shell;
    }
    (shell, walk)
}

/// What a press that was held back says, and where its toast's key goes ([`probe_held_back`]).
#[derive(Clone, Debug, Default)]
pub struct HeldBackReading {
    /// The answer the press got (its code when refused).
    pub said: String,
    /// The toast's sentence and its keys' labels.
    pub toast: Option<String>,
    pub keys: Vec<String>,
    /// The route drawn after the toast's own key was pressed (empty when it has none).
    pub route_after_key: String,
}

/// Press `action` over `place` in windowless frames, read the toast it leaves, then press that toast's own key
/// (the first one that hands a tag back) the way a click does, and read the route drawn after.
pub fn probe_held_back(ctx: &egui::Context, shell: Shell, place: Place, action: Action, width: f32, height: f32) -> (Shell, HeldBackReading) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = win.shell.faults.len();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win| {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        let _ = ctx.run(input, |c| win.draw(c));
    };
    run(&mut win);
    let applied = apply(&mut win.shell, action);
    let said = match &applied {
        Applied::Trouble(f) => f.which().map(|k| k.as_str().to_string()).unwrap_or_default(),
        other => format!("{other:?}").split([' ', '(', '{']).next().unwrap_or_default().to_string(),
    };
    for _ in 0..2 {
        run(&mut win);
    }
    let toast = win.toasts.showing().map(str::to_string);
    let keys_now = win.toasts.showing_keys();
    let keys = keys_now.iter().map(|(l, _)| l.clone()).collect();
    let mut route_after_key = String::new();
    if let Some(tag) = keys_now.iter().find_map(|(_, a)| match a {
        zikaron_ui::toast::Act::Tag(t) => Some(*t),
        _ => None,
    }) {
        win.toast_pressed(tag, 1.0);
        for _ in 0..3 {
            run(&mut win);
        }
        route_after_key = zikaron_ui::probe::route_drawn(ctx);
    }
    (win.shell, HeldBackReading { said, toast, keys, route_after_key })
}
