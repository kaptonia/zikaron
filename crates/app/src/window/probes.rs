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
    run_hooked(start, None)
}

/// What a driver outside the product (the door) hangs on a window it opens, to read how the window draws: whether
/// the window floats over the others (a window covered by others is not drawn), what it adds to the input of each
/// frame before the window takes it (the pointer moves and presses of a walk through the window), and what it reads
/// after each frame (that frame's own work). The product itself hangs none.
pub struct Hooks {
    pub float: bool,
    pub input: Box<dyn FnMut(&egui::Context, &mut egui::RawInput)>,
    pub frame: Box<dyn FnMut(&egui::Context, Option<f32>)>,
}

/// The window with a driver's [`Hooks`] on it.
struct Hooked {
    win: Win,
    hooks: Hooks,
}

impl eframe::App for Hooked {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        (self.hooks.input)(ctx, raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, f: &mut eframe::Frame) {
        eframe::App::ui(&mut self.win, ui, f);
        (self.hooks.frame)(ui.ctx(), f.info().cpu_usage);
    }

    fn clear_color(&self, v: &egui::Visuals) -> [f32; 4] {
        eframe::App::clear_color(&self.win, v)
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        eframe::App::on_exit(&mut self.win, gl);
    }
}

/// [`run_at`] with a driver's [`Hooks`] on the window.
pub fn run_hooked(start: Start, hooks: Option<Hooks>) -> i32 {
    let float = hooks.as_ref().is_some_and(|h| h.float);
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
            .with_titlebar_shown(false)
            // Transparent where the system can put its own material under the window (`zikaron_os::backdrop`).
            .with_transparent(zikaron_os::backdrop_available())
            .with_window_level(if float { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal }),
        // On macOS through Metal: one frame presented per refresh, at the screen's own rate.
        #[cfg(target_os = "macos")]
        renderer: eframe::Renderer::Wgpu,
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
                Start::Scene(scene) => {
                    if let Scene::Page(p) | Scene::Sheet(p, _) | Scene::Data(p, _) = scene {
                        ux.nav.land(crate::nav::history_of(p, shell.settings.role));
                    }
                    ux.wizard_asked = true;
                }
            }
            let typed = Typed { home: root, ..Default::default() };
            let mut win = fresh_win(shell, typed, ux);
            if let Start::Scene(scene) = &start {
                let scene = *scene;
                stage(&mut win, scene);
            }
            win.asks_motion = true;
            // The system's material under the window, in the app's appearance, where the system has one.
            if zikaron_os::backdrop_available() {
                use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                let w = match cc.window_handle().map(|h| h.as_raw()) {
                    Ok(RawWindowHandle::AppKit(h)) => zikaron_os::Window::AppKit(h.ns_view.as_ptr()),
                    Ok(RawWindowHandle::Win32(h)) => zikaron_os::Window::Win32(h.hwnd.get()),
                    _ => zikaron_os::Window::Other,
                };
                let dark = match win.appearance() {
                    skin::Appearance::Dark => true,
                    skin::Appearance::Light => false,
                    skin::Appearance::System => cc.egui_ctx.system_theme() == Some(egui::Theme::Dark),
                };
                let follows = win.appearance() == skin::Appearance::System;
                if zikaron_os::backdrop(w, dark, follows) != zikaron_os::Backdrop::Tint {
                    zikaron_ui::material::set_glass(true);
                    win.backdrop = Some((w, dark, follows));
                }
            }
            Ok(match hooks {
                Some(hooks) => Box::new(Hooked { win, hooks }) as Box<dyn eframe::App>,
                None => Box::new(win),
            })
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
        ctx.run_ui(input, |ui| win.draw(ui))
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
        let out = ctx.run_ui(input, |ui| win.draw(ui));
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
        let _ = ctx.run_ui(input, |ui| win.draw(ui));
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
        let out = ctx.run_ui(input, |ui| win.draw(ui));
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
        let out = ctx.run_ui(input, |ui| win.draw(ui));
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
        ctx.run_ui(input, |ui| win.draw(ui))
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
        ctx.run_ui(input, |ui| win.draw(ui))
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
        (t, ctx.run_ui(input, |ui| win.draw(ui)))
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
        ctx.run_ui(input, |ui| win.draw(ui))
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
        (t, ctx.run_ui(input, |ui| win.draw(ui)))
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
        let _ = ctx.run_ui(input, |ui| win.draw(ui));
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
        (t, ctx.run_ui(input, |ui| win.draw(ui)))
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
    // The question is drawn (its title and its leaving key on the frame), and leaving is the press of that key.
    let drawn = last.as_ref().map(|(_, out)| texts_of(out)).unwrap_or_default();
    let title_drawn = drawn.iter().any(|d| d.text == crate::lang::t(Key::WizExitTitle));
    let leave_at = drawn.iter().find(|d| d.text == crate::lang::t(Key::WizExitGo)).map(|d| d.rect.center());
    let asked_drawn = asked && title_drawn && leave_at.is_some();
    if let (true, Some(at)) = (asked, leave_at) {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at)]);
        let _ = run(&mut win, vec![press(true), press(false)]);
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
        asked_drawn,
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
        outs.push(ctx.run_ui(input, |ui| win.draw(ui)));
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
        ctx.run_ui(input, |ui| win.draw(ui))
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
        ctx.run_ui(input, |ui| win.draw(ui))
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
            out = Some(ctx.run_ui(input, |ui| win.draw(ui)));
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
    reading.sheet_drawn = win.ux.confirm_adopt && texts_of(&out).iter().any(|d| d.text == t(Key::WizAdoptFolder));
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
        ctx.run_ui(input, |ui| win.draw(ui))
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
        ctx.run_ui(input, |ui| win.draw(ui))
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
        ctx.run_ui(input, |ui| win.draw(ui))
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

/// The first-run wizard on its passcode step, `pin` typed in by real key events one character a frame, twice;
/// the second entry's task waited for; then the step's "next" key pressed where it is drawn.
pub fn probe_wizard_pin_typed(ctx: &egui::Context, shell: Shell, pin: &str, width: f32, height: f32) -> (Shell, WizPinTyped) {
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
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    win.wizard_open(crate::nav::Step::Pin);
    for _ in 0..8 {
        let _ = run(&mut win, Vec::new());
    }
    let mut reading = WizPinTyped::default();
    for ch in pin.chars() {
        let _ = run(&mut win, vec![egui::Event::Text(ch.to_string())]);
        reading.first.push(win.ux.pin.chars());
    }
    let _ = run(&mut win, Vec::new());
    reading.asked_again = !win.ux.pin_again.is_empty();
    for ch in pin.chars() {
        let _ = run(&mut win, vec![egui::Event::Text(ch.to_string())]);
        reading.second.push(win.ux.pin.chars());
    }
    // The second entry starts the store's task; the window takes its landing in a frame, as it always does.
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while win.shell.tasks.in_flight(crate::task::Kind::Vault) && std::time::Instant::now() < until {
        let _ = run(&mut win, Vec::new());
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let mut out = run(&mut win, Vec::new());
    for _ in 0..6 {
        out = run(&mut win, Vec::new());
    }
    reading.vault = Some(win.shell.vault.clone());
    let next = texts_of(&out).iter().find(|d| d.text == t(Key::WizardNext)).map(|d| d.rect.center());
    reading.next_drawn = next.is_some();
    if let Some(at) = next {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at), press(true), press(false)]);
        for _ in 0..4 {
            let _ = run(&mut win, Vec::new());
        }
    }
    reading.after = win.ux.wizard;
    // The cover's column beside the card: the other language's cell pressed where it is drawn.
    let before = crate::lang::lang();
    let other = crate::lang::Lang::ALL.into_iter().find(|l| *l != before).unwrap_or(before);
    let out = run(&mut win, Vec::new());
    if let Some(at) = texts_of(&out).iter().find(|d| d.text == other.label()).map(|d| d.rect.center()) {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at), press(true), press(false)]);
        let _ = run(&mut win, vec![egui::Event::PointerGone]);
        reading.cover_lang = Some((before, crate::lang::lang()));
    }
    (win.shell, reading)
}

/// The identity menu opened from the identity chip and the identity named `name` pressed in it, where the menu
/// draws it, over the shell as handed (a switch the caller has in flight stays in flight).
pub fn probe_id_menu_press(ctx: &egui::Context, shell: Shell, name: &str, width: f32, height: f32) -> (Shell, IdMenuPress) {
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
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    for _ in 0..6 {
        let _ = run(&mut win, Vec::new());
    }
    let menu = egui::Id::new("zikaron-id-menu");
    zikaron_ui::menu::set(ctx, menu, true);
    let mut out = run(&mut win, Vec::new());
    for _ in 0..6 {
        out = run(&mut win, Vec::new());
    }
    // The menu opens under the chip, which says the current identity's name too: the row is the lower of the two.
    let at = texts_of(&out).into_iter().filter(|d| d.text == name).max_by(|a, b| a.rect.center().y.total_cmp(&b.rect.center().y)).map(|d| d.rect.center());
    let mut reading = IdMenuPress { pressed: at.is_some(), ..Default::default() };
    if let Some(at) = at {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at), press(true), press(false)]);
        let _ = run(&mut win, Vec::new());
    }
    reading.wish = win.shell.identity_wish.clone();
    reading.flying = win.shell.tasks.flying().iter().map(|k| k.as_str().to_string()).collect();
    reading.flying.sort();
    zikaron_ui::menu::set(ctx, menu, false);
    (win.shell, reading)
}

/// The gate over the shell's key store as it stands: whether it draws the key that sets the machine directory
/// aside; with `press`, that key pressed and every text of the confirming sheet read; with `confirm` too, the
/// sheet's confirming key pressed, then the store's state, the wizard step and the sheet's refusal read.
pub fn probe_gate_aside(ctx: &egui::Context, shell: Shell, press: bool, confirm: bool, width: f32, height: f32) -> (Shell, GateAside) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    // As in a session the gate came up in after it was unlocked once.
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    let tap = |at: egui::Pos2| {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        vec![egui::Event::PointerMoved(at), press(true), press(false)]
    };
    let mut out = run(&mut win, Vec::new());
    for _ in 0..6 {
        out = run(&mut win, Vec::new());
    }
    let key = texts_of(&out).iter().find(|d| d.text == t(Key::GateAside)).map(|d| d.rect.center());
    let mut reading = GateAside { key_drawn: key.is_some(), ..Default::default() };
    if let (true, Some(at)) = (press, key) {
        let _ = run(&mut win, tap(at));
        for _ in 0..8 {
            out = run(&mut win, Vec::new());
        }
        reading.sheet = texts_of(&out).into_iter().map(|d| d.text).collect();
        let go = texts_of(&out).iter().find(|d| d.text == t(Key::GateAsideGo)).map(|d| d.rect.center());
        if let (true, Some(at)) = (confirm, go) {
            reading.confirmed = true;
            let _ = run(&mut win, tap(at));
            for _ in 0..8 {
                let _ = run(&mut win, Vec::new());
            }
        }
    }
    reading.vault = Some(win.shell.vault.clone());
    reading.wizard = win.ux.wizard;
    reading.trouble = win.ux.gate_aside_trouble.clone();
    (win.shell, reading)
}

/// The gate over an empty store that is locked out, in a session the gate came up in after it was unlocked once:
/// its reset key pressed where it is drawn, then the store's state and the wizard step read.
pub fn probe_gate_reset_empty(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, GateReset) {
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
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    let mut out = run(&mut win, Vec::new());
    for _ in 0..6 {
        out = run(&mut win, Vec::new());
    }
    let key = texts_of(&out).iter().find(|d| d.text == t(Key::PinDoReset)).map(|d| d.rect.center());
    let mut reading = GateReset { pressed: key.is_some(), ..Default::default() };
    if let Some(at) = key {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at), press(true), press(false)]);
        for _ in 0..8 {
            let _ = run(&mut win, Vec::new());
        }
    }
    reading.vault = Some(win.shell.vault.clone());
    reading.wizard = win.ux.wizard;
    (win.shell, reading)
}

/// The passcode gate (the vault locked) at rest, with `trouble` said under its cells when given: where its title's
/// strokes end, its cells, the refusal's box and the "forgot passcode" key lie.
pub fn probe_gate_column(ctx: &egui::Context, shell: Shell, trouble: Option<&str>, width: f32, height: f32) -> (Shell, GateColumn) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    ux.gate_trouble = trouble.map(str::to_string);
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut out = None;
    for k in 0..12 {
        // Two pixels a point, as the window has on the screens it ships to (layout rounds to pixels).
        let mut input = egui::RawInput { screen_rect: Some(screen), time: Some(0.1 * (k + 1) as f64), ..Default::default() };
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(2.0);
        out = Some(ctx.run_ui(input, |ui| win.draw(ui)));
    }
    let out = out.unwrap_or_default();
    let mut reading = GateColumn { window_h: height, group_top: 0.0, title_ink_bottom: 0.0, cells: egui::Rect::NOTHING, said: None, forgot: egui::Rect::NOTHING };
    let mut rects = Vec::new();
    for c in &out.shapes {
        match &c.shape {
            egui::Shape::Text(s) if s.galley.job.text == t(Key::PinGateTitle) => {
                reading.title_ink_bottom = s.visual_bounding_rect().bottom();
            }
            egui::Shape::Text(s) if s.galley.job.text == t(Key::AppName) => reading.group_top = s.pos.y,
            egui::Shape::Rect(r) => rects.push(r.rect),
            _ => {}
        }
    }
    let forgot_at = texts_of(&out).iter().find(|d| d.text == t(Key::PinForgot)).map(|d| d.rect.center());
    let cells: Vec<egui::Rect> = rects.iter().copied().filter(|r| (r.width() - 36.0).abs() < 0.5 && (r.height() - 44.0).abs() < 0.5).collect();
    reading.cells = cells.iter().fold(egui::Rect::NOTHING, |a, b| a.union(*b));
    if let Some(at) = forgot_at {
        // The key's own face: the last drawn of the smallest under its words (its shadow is drawn first, offset).
        reading.forgot = rects.iter().rev().copied().filter(|r| r.contains(at) && r.height() < 40.0).min_by(|a, b| a.area().total_cmp(&b.area())).unwrap_or(egui::Rect::NOTHING);
    }
    if let Some(said) = trouble {
        let at = texts_of(&out).iter().find(|d| d.text == said).map(|d| d.rect.center());
        reading.said = at.and_then(|p| rects.iter().rev().copied().filter(|r| r.contains(p) && r.width() < width - 100.0).min_by(|a, b| a.area().total_cmp(&b.area())));
    }
    (win.shell, reading)
}

/// The passcode gate (`wizard` false) or the wizard's passcode step (`wizard` true) drawn from `shell` (the caller
/// has the vault's task running): the passcode cells' row and the busy words drawn under it.
pub fn probe_busy_under_cells(ctx: &egui::Context, shell: Shell, wizard: bool, width: f32, height: f32) -> (Shell, egui::Rect, Option<egui::Rect>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    if wizard {
        win.wizard_open(crate::nav::Step::Pin);
    }
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut out = None;
    for k in 0..12 {
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(0.1 * (k + 1) as f64), ..Default::default() };
        out = Some(ctx.run_ui(input, |ui| win.draw(ui)));
    }
    let out = out.unwrap_or_default();
    let cells = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Rect(r) if (r.rect.width() - 36.0).abs() < 0.5 && (r.rect.height() - 44.0).abs() < 0.5 => Some(r.rect),
            _ => None,
        })
        .fold(egui::Rect::NOTHING, |a, b| a.union(b));
    let busy = texts_of(&out).iter().find(|d| d.text == t(Key::VaultBusy)).map(|d| d.rect);
    (win.shell, cells, busy)
}

/// The new-grant page opened fresh over the grants list and drawn until the chain-time read it asks for has
/// landed (or `frames` frames have gone, 20 ms apart): the start written for its validity (`g_from`, seconds) and
/// every text drawn on the last frame.
pub fn probe_grant_start(ctx: &egui::Context, shell: Shell, frames: usize, width: f32, height: f32) -> (Shell, String, Vec<String>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::View(crate::nav::View::Grants, 0), role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    win.grant_form_fresh();
    win.push(Route::NewGrant, 0.0);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut out = egui::FullOutput::default();
    let mut asked = false;
    for k in 0..frames {
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(0.1 * (k + 1) as f64), ..Default::default() };
        out = ctx.run_ui(input, |ui| win.draw(ui));
        let flying = win.shell.tasks.in_flight(crate::task::Kind::ChainTime);
        asked |= flying || win.shell.tasks.attempted(crate::task::Kind::ChainTime);
        if asked && !flying && k > 2 {
            // Landed: one more frame draws what it brought.
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(0.1 * (k + 2) as f64), ..Default::default() };
            out = ctx.run_ui(input, |ui| win.draw(ui));
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let texts = texts_of(&out).into_iter().map(|d| d.text).collect();
    let from = win.typed.g_from.clone();
    (win.shell, from, texts)
}

/// A view's own page at `from`, scrolled down by `scroll`, and the segment drawn as `words` pressed by the window's
/// own input.
pub fn probe_seg_tab(ctx: &egui::Context, shell: Shell, from: Place, words: &str, scroll: f32, width: f32, height: f32) -> (Shell, SegTab) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(from, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>, frames: usize| -> egui::FullOutput {
        let mut out = egui::FullOutput::default();
        for k in 0..frames {
            clock += 0.1;
            let ev = if k == 0 { events.clone() } else { Vec::new() };
            out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), time: Some(clock), events: ev, ..Default::default() }, |ui| win.draw(ui));
        }
        out
    };
    let mut r = SegTab::default();
    let _ = run(&mut win, Vec::new(), 10);
    let mid = egui::pos2(width * 0.6, height * 0.6);
    let wheel = vec![egui::Event::PointerMoved(mid), egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, -scroll), phase: egui::TouchPhase::Move, modifiers: egui::Modifiers::NONE }];
    let out = run(&mut win, wheel, 10);
    r.scroll_before = win.ux.nav.scroll_y;
    r.entry_before = win.ux.nav.entry_key;
    let at = texts_of(&out).into_iter().find(|d| d.text == words).map(|d| d.rect.center());
    if let Some(at) = at {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at), press(true), press(false)], 10);
        r.pressed = true;
    }
    r.entry_after = win.ux.nav.entry_key;
    r.scroll_after = win.ux.nav.scroll_y;
    r.route = win.route().name();
    r.page = format!("{:?}", win.shell.page);
    let h = win.hist();
    r.history_empty = h.back.is_empty() && h.fwd.is_empty();
    (win.shell, r)
}

/// The new-grant form on custom validity (its start `start_day` at `start_at`, its length `lasts`), driven by the
/// window's own input: the end block pressed, the start's own day pressed on the calendar it opens, then the day `end`
/// (on the page the calendar opened on); then the length's days part set to `days_then`. With `picture`, the frame
/// with the calendar open is kept.
#[allow(clippy::too_many_arguments)]
pub fn probe_grant_end_day(ctx: &egui::Context, shell: Shell, start_day: &str, start_at: [&str; 2], lasts: [&str; 3], end: &str, days_then: &str, picture: bool, width: f32, height: f32) -> (Shell, GrantEnd) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::View(crate::nav::View::Grants, 0), role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    win.grant_form_fresh();
    win.push(Route::NewGrant, 0.0);
    win.ux.u3.grant_days = 0;
    win.typed.g_start_day = start_day.to_string();
    win.typed.g_start_at = start_at.map(str::to_string);
    win.typed.g_lasts = lasts.map(str::to_string);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut textures: std::collections::HashMap<egui::TextureId, egui::ColorImage> = std::collections::HashMap::new();
    let mut clock = 0.0;
    let mut run = |win: &mut Win, textures: &mut std::collections::HashMap<egui::TextureId, egui::ColorImage>, events: Vec<egui::Event>, frames: usize| -> egui::FullOutput {
        let mut out = egui::FullOutput::default();
        for k in 0..frames {
            clock += 0.1;
            let ev = if k == 0 { events.clone() } else { Vec::new() };
            out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), time: Some(clock), events: ev, ..Default::default() }, |ui| win.draw(ui));
            take_textures(textures, &out);
        }
        out
    };
    let click = |at: egui::Pos2| {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        vec![egui::Event::PointerMoved(at), press(true), press(false)]
    };
    let spot = |name: &str| zikaron_ui::probe::spots(ctx).into_iter().find(|(n, _)| n == name).map(|(_, r)| r.center());
    let days = || zikaron_ui::probe::spots(ctx).into_iter().map(|(n, _)| n).filter(|n| n.as_bytes().first().is_some_and(u8::is_ascii_digit)).collect::<Vec<String>>();
    let mut r = GrantEnd::default();
    let _ = run(&mut win, &mut textures, Vec::new(), 8);
    r.live = zikaron_ui::probe::spots(ctx).into_iter().map(|(n, _)| n).collect();
    let Some(block) = spot(&datepick::block_salt("grant-span", 1)) else { return (win.shell, r) };
    let out = run(&mut win, &mut textures, click(block), 6);
    r.days = days();
    if picture {
        r.picture = Some(Picture { prims: ctx.tessellate(out.shapes.clone(), out.pixels_per_point), textures: textures.clone(), ppp: out.pixels_per_point });
    }
    let start_off = r.days.iter().find(|d| d.starts_with(start_day) && d.ends_with(" off")).cloned();
    if let Some(at) = start_off.and_then(|d| spot(&d)) {
        let _ = run(&mut win, &mut textures, click(at), 4);
    }
    r.off_lasts = win.typed.g_lasts.clone();
    r.off_open = !days().is_empty();
    if let Some(at) = spot(end) {
        let _ = run(&mut win, &mut textures, click(at), 6);
    }
    r.lasts = win.typed.g_lasts.clone();
    r.from = win.typed.g_from.clone();
    r.to = win.typed.g_to.clone();
    r.open_after = !days().is_empty();
    win.typed.g_lasts[0] = days_then.to_string();
    let _ = run(&mut win, &mut textures, Vec::new(), 2);
    r.to_then = win.typed.g_to.clone();
    (win.shell, r)
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
        ctx.run_ui(input, |ui| win.draw(ui))
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
    // The days cell is the number before the word "days": its middle sits half a number and the gap left of the word.
    // Both are read off the frame each press lands on (the line under the box takes room only with words, so what
    // is below it moves).
    let cell_in = |texts: &[Drawn]| texts.iter().find(|d| d.text == t(Key::CliLastsDays)).map(|d| egui::pos2(d.rect.left() - settings::CLI_SPAN_GAP - settings::CLI_SPAN_NUM_W / 2.0, d.rect.center().y));
    let toggle_in = |texts: &[Drawn]| texts.iter().find(|d| d.text == t(Key::CliLastsNever)).map(|d| d.rect.center());
    let mut reading = CliSheet::default();
    let Some(cell) = cell_in(&drawn) else { return (win.shell, reading) };
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
    let Some(toggle) = toggle_in(&texts) else { return (win.shell, reading) };
    let _ = run(&mut win, click(toggle));
    let texts = settle(&mut win, &mut run);
    reading.never_on = win.ux.cli_never;
    let Some(cell) = cell_in(&texts) else { return (win.shell, reading) };
    let _ = run(&mut win, click(cell));
    let _ = run(&mut win, vec![egui::Event::Text("9".into())]);
    let texts = settle(&mut win, &mut run);
    reading.days_while_never = win.ux.cli_span[0].clone();
    reading.quiet_while_never = !texts.iter().any(|d| !until.is_empty() && d.text.starts_with(&until));
    (win.shell, reading)
}

/// A new grant's "choose a record" sheet over `rows` (the ledger as read), on the grant form: the words drawn
/// once it is up, then a press on the record named `grey` and one on the record named `open`, each where its name
/// is drawn, each followed by a few frames.
pub fn probe_work_pick(ctx: &egui::Context, mut shell: Shell, rows: Vec<crate::ledgerx::Row>, grey: &str, open: &str, width: f32, height: f32) -> (Shell, WorkPick) {
    let n = rows.len();
    shell.rows = Some((rows, n));
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::View(crate::nav::View::Grants, crate::nav::tab::GRANTS_NEW), role));
    ux.wizard_asked = true;
    ux.u3.work_pick = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    let settle = |win: &mut Win, run: &mut dyn FnMut(&mut Win, Vec<egui::Event>) -> egui::FullOutput| {
        let mut out = run(win, Vec::new());
        for _ in 0..6 {
            out = run(win, Vec::new());
        }
        texts_of(&out)
    };
    let press = |at: egui::Pos2| {
        let b = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        vec![egui::Event::PointerMoved(at), b(true), b(false)]
    };
    let mut reading = WorkPick::default();
    let drawn = settle(&mut win, &mut run);
    reading.drawn = drawn.iter().map(|d| d.text.clone()).collect();
    let at = |texts: &[Drawn], name: &str| texts.iter().rev().find(|d| d.text == name).map(|d| d.rect.center());
    if let Some(p) = at(&drawn, grey) {
        let _ = run(&mut win, vec![egui::Event::PointerMoved(p)]);
        let _ = run(&mut win, press(p));
    }
    let drawn = settle(&mut win, &mut run);
    reading.after_grey = (win.typed.g_work.clone(), win.ux.u3.work_pick);
    if let Some(p) = at(&drawn, open) {
        let _ = run(&mut win, vec![egui::Event::PointerMoved(p)]);
        let _ = run(&mut win, press(p));
    }
    let _ = settle(&mut win, &mut run);
    reading.after_open = (win.typed.g_work.clone(), win.ux.u3.work_pick);
    (win.shell, reading)
}

/// A new grant's "choose a record" sheet over `rows`: the pointer rested on each record named in `names` in turn,
/// where its name is drawn, and the lit block drawn under it then (none when nothing lit there).
pub fn probe_pick_lits(ctx: &egui::Context, mut shell: Shell, rows: Vec<crate::ledgerx::Row>, names: &[&str], width: f32, height: f32) -> (Shell, Vec<Option<zikaron_ui::probe::Lit>>) {
    let n = rows.len();
    shell.rows = Some((rows, n));
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::View(crate::nav::View::Grants, crate::nav::tab::GRANTS_NEW), role));
    ux.wizard_asked = true;
    ux.u3.work_pick = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    let mut out = run(&mut win, Vec::new());
    for _ in 0..8 {
        out = run(&mut win, Vec::new());
    }
    let drawn = texts_of(&out);
    let mut got = Vec::new();
    for name in names {
        let Some(at) = drawn.iter().rev().find(|d| d.text == *name).map(|d| d.rect.center()) else {
            got.push(None);
            continue;
        };
        for _ in 0..3 {
            let _ = run(&mut win, vec![egui::Event::PointerMoved(at)]);
        }
        got.push(zikaron_ui::probe::lits(ctx).into_iter().find(|l| l.block.contains(at)));
    }
    (win.shell, got)
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
        out = Some(ctx.run_ui(input, |ui| win.draw(ui)));
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
        out = ctx.run_ui(input, |ui| win.draw(ui));
    }
    let texts = texts_of(&out).into_iter().map(|d| d.text).collect();
    (win.shell, texts)
}

/// Settings, network, at rest and then with a chain read the person started in flight (held running until the
/// probe is done): where the section under the network's card starts, before and while it runs, and whether the
/// rail's status line says the read is in progress.
pub fn probe_network_reading(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, Option<f32>, Option<f32>, bool) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Settings(crate::nav::Section::Network), role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut settle = |win: &mut Win| {
        let mut out = egui::FullOutput::default();
        for _ in 0..10 {
            clock += 0.1;
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), ..Default::default() };
            out = ctx.run_ui(input, |ui| win.draw(ui));
        }
        out
    };
    let under = |out: &egui::FullOutput| texts_of(out).iter().find(|d| d.text == t(Key::NetGroup)).map(|d| d.rect.top());
    let before = under(&settle(&mut win));
    let (hold, wait) = std::sync::mpsc::channel::<()>();
    let _ = win.shell.tasks.spawn_turnless(crate::task::Kind::Chain, move || {
        let _ = wait.recv();
        Err(crate::fault::Fault::known(crate::fault::Known::Denied, "held by the probe"))
    });
    win.ux.asked.push(crate::task::Kind::Chain);
    let out = settle(&mut win);
    let during = under(&out);
    let said = fill1(Key::NavDoing, t(Key::TaskChain));
    let reported = texts_of(&out).iter().any(|d| d.text == said);
    drop(hold);
    (win.shell, before, during, reported)
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
        out = ctx.run_ui(input, |ui| win.draw(ui));
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
        ctx.run_ui(input, |ui| win.draw(ui))
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
        let _ = ctx.run_ui(input, |ui| win.draw(ui));
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
        ctx.run_ui(input, |ui| win.draw(ui))
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
        let _ = ctx.run_ui(input, |ui| win.draw(ui));
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

// ───────────────────────── Geometry of still frames ─────────────────────────

/// One still frame the geometry reading reads: a page, a first-run wizard step, a sheet over a page, the passcode
/// gate in one of its states, or the rail's identity menu open over home.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Page(Place),
    Wizard(crate::nav::Step),
    Sheet(Place, SheetScene),
    Gate(GateScene),
    IdMenu,
    /// A page or a sheet in a state of its data that the plain readings do not reach.
    Data(Place, DataScene),
}

/// The data states the geometry reading sets up beyond the ordinary scenes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataScene {
    /// The check page after a grant was checked with no node set: "go to settings" and the source row's key.
    CheckNoNode,
    /// The same with the source row open.
    CheckSourceOpen,
    /// Record verification after a kit of this home's records was verified.
    VerifyKit,
    /// Others' records after this home's own ledger was read as another's.
    OthersRead,
    /// The same on its entry list.
    OthersList,
    /// One entry of that reading, on its own page.
    OtherEntry,
    /// The new-record sheet holding a batch of files: the "clear" key under them.
    NewRecordBatch,
    /// The import sheet listing an earlier anchor of this key: the "select all" key.
    AdoptListed,
    /// A grant's detail page (this home's grant).
    GrantDetail,
    /// My grants holding one grant.
    HeldListed,
    /// A held grant's detail page.
    HeldDetail,
    /// A record's detail page (the first record of the ledger).
    WorkDetail,
    /// A ledger entry's detail page (the ledger's first entry that is not a grant).
    EntryDetail,
    /// The detail page of the first entry waiting to go on chain.
    PendingDetail,
    /// The record kit page with its export sheet up (where the kit lands).
    KitExport,
    /// The record kit page with "encrypt the record package" on (the folder switch off and greyed), no sheet.
    KitPageSealed,
    /// The same with "encrypt the record package" on: the sheet's two recipient fields, both empty.
    KitExportSealed,
    /// The same with an address in the first field: the public key field greyed.
    KitExportSealedTo,
    /// The same with this identity's own public key in the second field: the sheet ready to export (for the window
    /// bench, where the export key is pressed and its progress and check are seen).
    KitExportSealedKey,
    /// Settings, keys: the details open (the key address and, under it, the public key).
    KeysDetails,
}

/// A sheet the geometry reading opens, each in a state its ordinary content needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetScene {
    BackupExport,
    RestoreFirstRun,
    RestoreSettings,
    Mirror,
    Cli,
    NewRecord,
    NewRecordConfirm,
    Send,
    NoEstimate,
    Clash,
    Adopt,
    Succeed,
    SucceedConfirm,
    Annotate,
    Attest,
    GrantConfirm,
    DeleteRecord,
    Revoke,
    KitPick,
    WorkPick,
    AddGrant,
    IdNew,
    IdImportWords,
    IdImportKey,
    IdImportFile,
    IdBackup,
    IdPin,
    IdWords,
    IdSwitch,
    IdName,
    IdDelete,
    WizExit,
    CreateLedger,
    WizardAdopt,
    NetEdit,
    ReadNet,
    PublishCheck,
    DataFolder,
    Snapshot,
    GrantFile,
    Badge,
    HeldUpstream,
    /// The published address's own sheet (its one field).
    PublishEdit,
    /// Taking the originals out of the kit verified (the record verification's data scene staged under it).
    ExtractOriginals,
}

/// The passcode gate's states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateScene {
    Unlock,
    RecoverWords,
    RecoverFile,
    RecoverNone,
    LockedOut,
    RestoreLocked,
    /// Restoring a backup at the gate, at its new passcode.
    RestoreNewPin,
    /// A key store file that cannot be read: its refusal and the one way on.
    Damaged,
    /// The same, with the sheet confirming the machine directory set aside.
    Aside,
}

impl Scene {
    /// A name for the reading's table: what it is, by the product's own route names.
    pub fn name(&self) -> String {
        match self {
            Scene::Page(p) => format!("page {}", crate::nav::Route::Root(*p).name()),
            Scene::Wizard(s) => format!("wizard {s:?}"),
            Scene::Sheet(p, s) => format!("sheet {s:?} over {}", crate::nav::Route::Root(*p).name()),
            Scene::Gate(g) => format!("gate {g:?}"),
            Scene::IdMenu => "menu identity".into(),
            Scene::Data(p, d) => format!("data {d:?} over {}", crate::nav::Route::Root(*p).name()),
        }
    }
}

/// Every scene of `role`: each place on its rail with each tab, the settings home and its sections, each wizard
/// step, each sheet over its page, the gate's states and the identity menu.
pub fn scenes(role: crate::roles::Role) -> Vec<Scene> {
    use crate::nav::{Section, Step, View};
    use crate::roles::Role;
    let mut out = vec![Scene::Page(Place::Home)];
    let views: &[(View, u8)] = match role {
        Role::Author => &[(View::Works, 0), (View::Works, 1), (View::Works, 2), (View::Grants, 0), (View::Grants, 1), (View::Verify, 0), (View::Verify, 1), (View::Verify, 2), (View::Log, 0), (View::Alerts, 0)],
        Role::Grantee => &[(View::MyGrants, 0), (View::Verify, 0), (View::Verify, 1), (View::Verify, 2), (View::Alerts, 0)],
    };
    out.extend(views.iter().map(|(v, t)| Scene::Page(Place::View(*v, *t))));
    out.push(Scene::Page(Place::SettingsHome));
    out.extend(Section::ALL.iter().map(|s| Scene::Page(Place::Settings(*s))));
    out.extend(Step::ALL.iter().map(|s| Scene::Wizard(*s)));
    let data = Place::Settings(Section::Data);
    let keys = Place::Settings(Section::Keys);
    let log = Place::View(View::Log, 0);
    let mut sheets = vec![
        (data, SheetScene::BackupExport),
        (data, SheetScene::RestoreSettings),
        (data, SheetScene::Mirror),
        (Place::Home, SheetScene::RestoreFirstRun),
        (Place::Settings(Section::Cli), SheetScene::Cli),
        (keys, SheetScene::IdNew),
        (keys, SheetScene::IdImportWords),
        (keys, SheetScene::IdImportKey),
        (keys, SheetScene::IdImportFile),
        (keys, SheetScene::IdBackup),
        (keys, SheetScene::IdPin),
        (keys, SheetScene::IdWords),
        (keys, SheetScene::IdSwitch),
        (keys, SheetScene::IdName),
        (keys, SheetScene::IdDelete),
        (Place::Home, SheetScene::WizExit),
        (Place::Home, SheetScene::CreateLedger),
        (Place::Home, SheetScene::WizardAdopt),
        (Place::Settings(Section::Network), SheetScene::NetEdit),
        (Place::Settings(Section::Network), SheetScene::ReadNet),
        (Place::Settings(Section::Network), SheetScene::PublishCheck),
        (Place::Settings(Section::Network), SheetScene::PublishEdit),
        (Place::View(View::Verify, crate::nav::tab::VERIFY_WORK), SheetScene::ExtractOriginals),
        (data, SheetScene::DataFolder),
    ];
    match role {
        Role::Author => sheets.extend([
            (Place::Home, SheetScene::NewRecord),
            (Place::Home, SheetScene::NewRecordConfirm),
            (log, SheetScene::Send),
            (log, SheetScene::NoEstimate),
            (Place::View(View::Grants, 1), SheetScene::Clash),
            (Place::View(View::Grants, 1), SheetScene::GrantConfirm),
            (log, SheetScene::Adopt),
            (log, SheetScene::Succeed),
            (log, SheetScene::SucceedConfirm),
            (log, SheetScene::Annotate),
            (log, SheetScene::Attest),
            (Place::View(View::Works, 0), SheetScene::DeleteRecord),
            (Place::View(View::Grants, 0), SheetScene::Revoke),
            (Place::View(View::Works, 2), SheetScene::KitPick),
            (Place::View(View::Grants, crate::nav::tab::GRANTS_NEW), SheetScene::WorkPick),
            (Place::View(View::Grants, 0), SheetScene::GrantFile),
        ]),
        Role::Grantee => sheets.extend([
            (Place::View(View::MyGrants, 0), SheetScene::AddGrant),
            (Place::View(View::MyGrants, 0), SheetScene::Badge),
            (Place::View(View::MyGrants, 0), SheetScene::HeldUpstream),
            (Place::View(View::Verify, crate::nav::tab::VERIFY_OTHERS), SheetScene::Snapshot),
        ]),
    }
    out.extend(sheets.into_iter().map(|(p, s)| Scene::Sheet(p, s)));
    out.extend([GateScene::Unlock, GateScene::RecoverWords, GateScene::RecoverFile, GateScene::RecoverNone, GateScene::LockedOut, GateScene::RestoreLocked, GateScene::RestoreNewPin, GateScene::Damaged, GateScene::Aside].map(Scene::Gate));
    out.push(Scene::IdMenu);
    let check = Place::View(View::Verify, crate::nav::tab::VERIFY_CHECK);
    out.extend([Scene::Data(check, DataScene::CheckNoNode), Scene::Data(check, DataScene::CheckSourceOpen)]);
    out.extend([
        Scene::Data(Place::View(View::Verify, crate::nav::tab::VERIFY_WORK), DataScene::VerifyKit),
        Scene::Data(Place::View(View::Verify, crate::nav::tab::VERIFY_OTHERS), DataScene::OthersRead),
        Scene::Data(Place::View(View::Verify, crate::nav::tab::VERIFY_OTHERS), DataScene::OthersList),
        Scene::Data(Place::View(View::Verify, crate::nav::tab::VERIFY_OTHERS), DataScene::OtherEntry),
        Scene::Data(Place::Settings(Section::Keys), DataScene::KeysDetails),
    ]);
    if role == Role::Author {
        out.extend([Scene::Data(Place::Home, DataScene::NewRecordBatch), Scene::Data(log, DataScene::AdoptListed)]);
        out.push(Scene::Data(Place::View(View::Grants, 0), DataScene::GrantDetail));
        out.extend([
            Scene::Data(Place::View(View::Works, 0), DataScene::WorkDetail),
            Scene::Data(log, DataScene::EntryDetail),
            Scene::Data(Place::View(View::Works, 1), DataScene::PendingDetail),
            Scene::Data(Place::View(View::Works, 2), DataScene::KitExport),
            Scene::Data(Place::View(View::Works, 2), DataScene::KitPageSealed),
            Scene::Data(Place::View(View::Works, 2), DataScene::KitExportSealed),
            Scene::Data(Place::View(View::Works, 2), DataScene::KitExportSealedTo),
            Scene::Data(Place::View(View::Works, 2), DataScene::KitExportSealedKey),
        ]);
    } else {
        let mine = Place::View(View::MyGrants, 0);
        out.extend([Scene::Data(mine, DataScene::HeldListed), Scene::Data(mine, DataScene::HeldDetail)]);
    }
    out
}

/// A scene by its name (`Scene::name`, or a sheet's own name such as `NetEdit`), among both seats' scenes.
pub fn scene_named(name: &str) -> Option<Scene> {
    let all = [crate::roles::Role::Author, crate::roles::Role::Grantee].into_iter().flat_map(scenes);
    all.into_iter().find(|s| s.name() == name || matches!(s, Scene::Sheet(_, x) if format!("{x:?}") == name))
}

/// One widget of a still frame: its layer's order (`Background`, `Middle`, `Foreground`, `Tooltip`, `Debug`), its
/// rectangle, and whether it takes clicks or drags.
#[derive(Clone, Debug, PartialEq)]
pub struct GeoWidget {
    pub order: String,
    pub rect: egui::Rect,
    pub click: bool,
    pub drag: bool,
}

/// A still frame's geometry: every widget rectangle and every text drawn with its laid-out rectangle (where the
/// layout put it, not where its glyphs' ink falls, which follows the font's rasterizer).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geometry {
    pub widgets: Vec<GeoWidget>,
    pub texts: Vec<(String, egui::Rect)>,
    /// Controls (widgets that take clicks or drags) inside a card made of rows but in none of its rows.
    pub strays: Vec<egui::Rect>,
    /// The cards made of rows (their layer's order and rectangle), and their rows and parts (alike): what a
    /// reading of alignment measures against.
    pub cards: Vec<(String, egui::Rect)>,
    pub rows: Vec<(String, egui::Rect)>,
    /// The lit blocks seen with the pointer on each control that takes a click, one by one.
    pub lits: Vec<zikaron_ui::probe::Lit>,
    /// Texts wider than the clip they are drawn in: the words, where they were laid, and the clip that cuts them.
    pub cut: Vec<(String, egui::Rect, egui::Rect)>,
}

/// Draw `scene` in windowless frames until it is still (entrances, sheet openings and the appearance's blend
/// over) and read its geometry: what egui recorded of every widget in the last pass, and every text shape. The
/// state a scene needs is set in memory for the reading and put back after it; the shell comes back as it came.
pub fn probe_geometry(ctx: &egui::Context, shell: Shell, scene: Scene, width: f32, height: f32) -> (Shell, Geometry) {
    let role = shell.settings.role;
    let place = match scene {
        Scene::Page(p) | Scene::Sheet(p, _) | Scene::Data(p, _) => p,
        _ => Place::Home,
    };
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    ux.lang_applied = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    // What a scene changes in the shell, put back after the reading.
    let kept = (win.shell.vault.clone(), win.shell.anchor.clone(), win.shell.rooted, win.shell.machine.backup.clone(), win.shell.primary.clone(), win.shell.backup_peek.clone(), win.shell.gas, win.shell.vault_recoverable);
    let kept_data = (win.shell.checked.clone(), win.shell.key_anchors.clone(), win.shell.held.clone(), win.shell.verified.clone(), win.shell.book.clone(), win.shell.diligence.clone());
    stage(&mut win, scene);
    stage_memory(ctx, scene);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut out = None;
    for _ in 0..14 {
        t += 0.1;
        if let Scene::IdMenu = scene {
            zikaron_ui::menu::set(ctx, egui::Id::new("zikaron-id-menu"), true);
        }
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        out = Some(ctx.run_ui(input, |ui| win.draw(ui)));
    }
    // What the scene started in the background lands before the reading (a reading never depends on how fast
    // this machine ran it), then a few frames more draw it.
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while !win.shell.tasks.flying().is_empty() && std::time::Instant::now() < until {
        win.shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    for _ in 0..6 {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        out = Some(ctx.run_ui(input, |ui| win.draw(ui)));
    }
    let out = out.expect("frames ran");
    // Every control in a card made of rows lies in one of the rows (on its own layer).
    let (parts, boxes) = (zikaron_ui::probe::parts(ctx), zikaron_ui::probe::boxes(ctx));
    let strays: Vec<egui::Rect> = ctx.viewport(|v| {
        v.prev_pass
            .widgets
            .layers()
            .flat_map(|(layer, ws)| ws.iter().filter(|w| w.sense.senses_click() || w.sense.senses_drag()).map(move |w| (*layer, w.rect)))
            .filter(|(layer, r)| r.is_finite() && boxes.iter().any(|(l, b)| l == layer && b.expand(0.5).contains_rect(*r)))
            .filter(|(layer, r)| !parts.iter().any(|(l, p)| l == layer && p.expand(0.5).contains_rect(*r)))
            .map(|(_, r)| r)
            .collect()
    });
    let mut widgets: Vec<GeoWidget> = ctx.viewport(|v| {
        v.prev_pass
            .widgets
            .layers()
            .flat_map(|(layer, ws)| ws.iter().map(move |w| GeoWidget { order: format!("{:?}", layer.order), rect: w.rect, click: w.sense.senses_click(), drag: w.sense.senses_drag() }))
            .collect()
    });
    widgets.retain(|w| w.rect.is_finite() && w.rect.width() > 0.0 && w.rect.height() > 0.0);
    let mut texts = Vec::new();
    let mut cut = Vec::new();
    fn walk(s: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect)>, cut: &mut Vec<(String, egui::Rect, egui::Rect)>) {
        match s {
            egui::Shape::Text(x) => {
                let r = x.galley.rect.translate(x.pos.to_vec2());
                // Cut across: laid wider than the clip it is drawn in (a row scrolled half out is cut down, not across).
                if r.width() > 0.0 && (r.left() < clip.left() - 0.5 || r.right() > clip.right() + 0.5) && r.intersects(clip) {
                    cut.push((x.galley.job.text.clone(), r, clip));
                }
                out.push((x.galley.job.text.clone(), r));
            }
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, clip, out, cut)),
            _ => {}
        }
    }
    for c in &out.shapes {
        walk(&c.shape, c.clip_rect, &mut texts, &mut cut);
    }
    // The pointer on each control that takes a click, a frame each: the block it lights, as drawn.
    let mut lits: Vec<zikaron_ui::probe::Lit> = Vec::new();
    for w in widgets.iter().filter(|w| w.click) {
        let at = w.rect.center();
        if let Scene::IdMenu = scene {
            zikaron_ui::menu::set(ctx, egui::Id::new("zikaron-id-menu"), true);
        }
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), events: vec![egui::Event::PointerMoved(at)], ..Default::default() };
        let _ = ctx.run_ui(input, |ui| win.draw(ui));
        for l in zikaron_ui::probe::lits(ctx) {
            if !lits.iter().any(|k| k.block == l.block) {
                lits.push(l);
            }
        }
    }
    let input = egui::RawInput { screen_rect: Some(screen), time: Some(t + 0.1), events: vec![egui::Event::PointerGone], ..Default::default() };
    let _ = ctx.run_ui(input, |ui| win.draw(ui));
    zikaron_ui::menu::set(ctx, egui::Id::new("zikaron-id-menu"), false);
    let mut shell = win.shell;
    (shell.vault, shell.anchor, shell.rooted, shell.machine.backup, shell.primary, shell.backup_peek, shell.gas, shell.vault_recoverable) = kept;
    (shell.checked, shell.key_anchors, shell.held, shell.verified, shell.book, shell.diligence) = kept_data;
    let cards = boxes.iter().map(|(l, r)| (format!("{:?}", l.order), *r)).collect();
    let rows = parts.iter().map(|(l, r)| (format!("{:?}", l.order), *r)).collect();
    (shell, Geometry { widgets, texts, strays, cards, rows, lits, cut })
}

/// Set the window's data for a data scene: a real check of this home's grant (no node set, so its gaps name the
/// missing node), a kit of this home's records verified, this home's ledger read as another's (its overview, its
/// list, one entry), a batch of two files, an earlier anchor of this key listed.
fn stage_data(win: &mut Win, d: DataScene) {
    match d {
        DataScene::CheckNoNode | DataScene::CheckSourceOpen => {
            let rows = win.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
            let grant = rows.iter().find(|r| r.kind == zikaron::tokens::EntryType::Grant).map(|r| r.id.clone()).unwrap_or_default();
            let (bytes, ledger) = match win.shell.home.as_ref() {
                Some(h) => (crate::ledgerx::detail(h, &grant).map(|x| x.bytes).unwrap_or_default(), h.dir(crate::home::Slot::Ledger).display().to_string()),
                None => (Vec::new(), String::new()),
            };
            win.typed.ck_typed = zikaron_kit::badge::encode(&[bytes]).unwrap_or_default();
            win.typed.ck_ledgers = ledger;
            win.typed.ck_endpoints = String::new();
            win.typed.ck_registry = String::new();
            let a = win.check_action();
            let _ = crate::action::apply(&mut win.shell, a);
            let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
            while win.shell.tasks.in_flight(crate::task::Kind::Check) && std::time::Instant::now() < until {
                win.shell.drain();
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
            win.shell.drain();
            win.ux.u3.ck_source_open = (d == DataScene::CheckSourceOpen).then_some(0);
        }
        DataScene::VerifyKit => {
            // A whole kit of this home's records, every one anchored and matching (the geometry home reads no
            // chain, so the reading is laid from its own ledger).
            let rows = win.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
            let works: Vec<&crate::ledgerx::Row> = rows.iter().filter(|r| r.kind == zikaron::tokens::EntryType::History).collect();
            let at = win.shell.home.as_ref().map(|h| h.dir(crate::home::Slot::Kits).join("kit-geometry")).unwrap_or_default();
            win.typed.vf_path = at.display().to_string();
            let n = works.len();
            // Each record's original goes out under its name (a file each), as taking them out would take them.
            let take = works
                .iter()
                .map(|r| {
                    let name = r.facts.note.clone().filter(|x| !x.trim().is_empty()).unwrap_or_else(|| r.id.clone());
                    let content = r.work.clone().unwrap_or_default();
                    zikaron_glue::extract::Original { content: content.clone(), kept: zikaron_glue::extract::Kept::File, name: name.clone(), package_name: name.clone(), members: vec![zikaron_glue::extract::Member { path: name, rel: Vec::new(), sha: content }], names_hold: true }
                })
                .collect();
            let records = works
                .iter()
                .map(|r| crate::verifyx::RecordRow {
                    id: r.id.clone(),
                    content: r.work.clone().unwrap_or_default(),
                    name: r.facts.note.clone(),
                    original: crate::verifyx::Original::Match,
                    chain: crate::verifyx::OnChain::Anchored { first_at: r.anchored_at.unwrap_or(GEO_TIME) },
                    first: None,
                })
                .collect();
            let depth = zikaron::json::parse(format!(r#"{{"valid":true,"label":"COMPLETE","found":true,"earliest":{GEO_TIME},"deepest":4,"continuity":{{"anchored":{n},"span":{n}}}}}"#).as_bytes()).ok();
            win.shell.verified = Some(crate::verifyx::Verified {
                path: win.typed.vf_path.clone(),
                source: crate::verifyx::Source::Kit(at),
                kit: Some(crate::verifyx::KitFacts { ok: true, entries: rows.len(), files: n, proofs: n, kit_id: works.first().map(|r| r.id.clone()).unwrap_or_default(), verdict: String::new(), subject: String::new(), invalid: Vec::new() }),
                entries: rows.len(),
                rejected: Vec::new(),
                review: Ok(crate::verifyx::AnchorReview { label: zikaron::tokens::Label::Complete.as_str().to_string(), anchors: n, asked: n, unanchored: Vec::new(), anchored: n, unread: Vec::new() }),
                work: String::new(),
                depth,
                mismatches: Vec::new(),
                records,
                takes: Some(Ok(zikaron_glue::extract::Plan { take, left: Vec::new() })),
                not_added: None,
                missed: Vec::new(),
                read: vec![31337],
                filed: None,
            });
        }
        DataScene::OthersRead => stage_others(win),
        DataScene::OthersList => {
            stage_others(win);
            win.ux.u3.others_tab = 1;
        }
        DataScene::OtherEntry => {
            stage_others(win);
            let seq = win.shell.rows.as_ref().and_then(|(r, _)| r.iter().find(|x| x.kind == zikaron::tokens::EntryType::History).map(|x| x.seq)).unwrap_or(0);
            win.push(Route::Other(seq), 0.0);
        }
        DataScene::NewRecordBatch => {
            win.u3_new_anchor_open();
            win.typed.batch_files = vec!["/geometry/one.txt".into(), "/geometry/two.txt".into()];
        }
        DataScene::AdoptListed => {
            win.u3_form_open(U3Form::Adopt);
            win.ux.u3.adopt_asked = Some(String::new());
            let row = crate::adoptx::AnchorRow { chain_id: 1, tx: format!("0x{}", "ab".repeat(32)), payload_kind: "single".into(), content: format!("0x{}", "cd".repeat(32)) };
            win.shell.key_anchors = Some((String::new(), vec![crate::adoptx::KeyAnchor { row, block: 100, time: 1_760_000_000, in_ledger: false, proof: None }]));
        }
        DataScene::GrantDetail => {
            let grant = win.shell.grants.as_ref().and_then(|g| g.first().map(|r| r.id.clone())).unwrap_or_default();
            win.push(crate::nav::Route::Grant(grant), 0.0);
        }
        DataScene::HeldListed | DataScene::HeldDetail => {
            // One held grant, as the vault lists it, issued to this seat by another key.
            let now = (win.shell.clock)();
            let me = win.shell.anchor.map(|a| a.hex()).unwrap_or_default();
            let h = crate::vaultx::Held {
                id: format!("0x{}", "66".repeat(32)),
                bytes: Vec::new(),
                author: format!("0x{}", "7b".repeat(20)),
                grantee: me,
                work: format!("0x{}", "44".repeat(32)),
                terms: format!("0x{}", "55".repeat(32)),
                window: Some((now.saturating_sub(10 * 86_400), now + 44 * 86_400)),
                upstream: None,
                seq: 1,
            };
            let id = h.id.clone();
            win.shell.held = Some(vec![h]);
            if d == DataScene::HeldDetail {
                win.push(crate::nav::Route::Held(id), 0.0);
            }
        }
        DataScene::WorkDetail | DataScene::EntryDetail => {
            use zikaron::tokens::EntryType as E;
            let rows = win.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
            if d == DataScene::WorkDetail {
                if let Some(r) = rows.iter().find(|r| r.kind == E::History) {
                    win.push(crate::nav::Route::Work(r.id.clone()), 0.0);
                }
            } else if let Some(r) = rows.iter().find(|r| r.kind != E::Grant) {
                win.push(crate::nav::Route::Entry(r.id.clone()), 0.0);
            }
        }
        DataScene::PendingDetail => {
            if let Some(q) = win.shell.queue.items.first() {
                let id = q.id.clone();
                win.push(crate::nav::Route::Pending(id), 0.0);
            }
        }
        // The sheet itself is kept in the window's memory (`stage_memory`).
        DataScene::KitExport => win.ux.u3.kit_addressed = false,
        DataScene::KitPageSealed => {
            win.ux.u3.kit_folder = true;
            win.ux.u3.kit_addressed = true;
        }
        DataScene::KitExportSealed => {
            win.ux.u3.kit_addressed = true;
            win.typed.kit_to.clear();
            win.typed.kit_to_key.clear();
        }
        DataScene::KitExportSealedTo => {
            win.ux.u3.kit_addressed = true;
            win.typed.kit_to = format!("0x{}", "5b".repeat(20));
            win.typed.kit_to_key.clear();
        }
        DataScene::KitExportSealedKey => {
            win.ux.u3.kit_addressed = true;
            win.typed.kit_to.clear();
            win.typed.kit_to_key = win.shell.anchor_public.map(|p| zikaron::hexfmt::encode(&p)).unwrap_or_default();
        }
        DataScene::KeysDetails => win.ux.u3.keys_details = true,
    }
}

/// Set what a scene keeps in the window's memory rather than in its state: the record kit's export sheet.
fn stage_memory(ctx: &egui::Context, scene: Scene) {
    let up = matches!(scene, Scene::Data(_, DataScene::KitExport | DataScene::KitExportSealed | DataScene::KitExportSealedTo | DataScene::KitExportSealedKey));
    super::kit::kit_export_up(ctx, up);
}

/// This home's own ledger, read by its author's address as another's would be (laid from the ledger itself: the
/// geometry home reads no chain).
fn stage_others(win: &mut Win) {
    let who = win.shell.anchor.map(|a| a.hex()).unwrap_or_default();
    let timeline = win.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
    let grants = win.shell.grants.clone().unwrap_or_default();
    let anchors = timeline.iter().filter(|r| r.lamp == crate::ledgerx::Lamp::Anchored).count();
    let label = zikaron::tokens::Label::Complete.as_str().to_string();
    let from = Some((crate::supplyx::Level::Local, win.shell.home.as_ref().map(|h| h.dir(crate::home::Slot::Ledger).display().to_string()).unwrap_or_default()));
    let entries = timeline.len();
    if win.shell.settings.role == crate::roles::Role::Grantee {
        win.typed.dg_address = who.clone();
        win.shell.diligence = Some(crate::diligx::Read {
            who,
            anchors,
            asked: anchors,
            entries,
            label,
            timeline,
            grants,
            work: String::new(),
            depth: None,
            window: None,
            clash: Vec::new(),
            successions: Vec::new(),
            lineage: Vec::new(),
            now: Some(GEO_TIME),
            latest: Some(GEO_TIME),
            from,
            files: None,
            misses: Vec::new(),
            missed: Vec::new(),
        });
    } else {
        win.typed.rd_address = who.clone();
        win.shell.book = Some(Done::Book { who, anchors, asked: anchors, entries, label, timeline, grants, latest: Some(GEO_TIME), from, files: None, misses: Vec::new(), missed: Vec::new() });
    }
}

/// The time the data scenes' readings stand at.
const GEO_TIME: u64 = 1_760_500_000;

/// Set the window's state for `scene`, as the scene's own opener does.
fn stage(win: &mut Win, scene: Scene) {
    use crate::keybox::{PrimaryKind, State};
    use crate::nav::Step;
    use crate::shell::Vault;
    let id = win.shell.identities.as_ref().and_then(|r| r.current.clone().map(|(i, _)| i));
    match scene {
        Scene::Page(_) | Scene::IdMenu => {}
        Scene::Data(_, d) => stage_data(win, d),
        Scene::Wizard(step) => {
            match step {
                Step::Pin => win.shell.vault = Vault::Read(State::Absent),
                Step::Key => {
                    win.shell.anchor = None;
                    win.shell.anchor_public = None;
                }
                Step::Genesis => win.shell.rooted = false,
                Step::Backup => win.shell.machine.backup = None,
                Step::Network | Step::Gas => {}
            }
            win.wizard_open(step);
        }
        Scene::Gate(g) => {
            win.shell.vault = Vault::Read(if g == GateScene::LockedOut { State::LockedOut } else { State::Locked { wrong: 0 } });
            match g {
                GateScene::Unlock | GateScene::LockedOut => {}
                GateScene::Damaged | GateScene::Aside => {
                    let file = crate::home::machine_dir().map(|m| m.join(crate::places::keybox_file()).display().to_string()).unwrap_or_default();
                    win.shell.vault = Vault::Damaged(crate::fault::Fault::known(crate::fault::Known::KeyboxShape, file));
                    if g == GateScene::Aside {
                        win.ux.gate_aside = crate::home::aside_plan().ok();
                    }
                }
                GateScene::RecoverWords => {
                    win.ux.gate_recover = true;
                    win.shell.primary = id.clone().map(|i| (i, PrimaryKind::Words));
                }
                GateScene::RecoverFile => {
                    win.ux.gate_recover = true;
                    win.shell.primary = id.clone().map(|i| (i, PrimaryKind::KeyFile));
                }
                GateScene::RecoverNone => {
                    win.ux.gate_recover = true;
                    win.shell.primary = None;
                }
                GateScene::RestoreLocked => {
                    win.shell.backup_peek = None;
                    win.bk_open(Bk::Restore(RestoreFrom::Locked));
                }
                GateScene::RestoreNewPin => {
                    win.shell.backup_peek = None;
                    win.bk_open(Bk::Restore(RestoreFrom::Locked));
                    win.ux.bk_step = BkStep::NewPin;
                }
            }
        }
        Scene::Sheet(_, s) => {
            let rows = win.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
            let history = rows.iter().find(|r| r.kind == zikaron::tokens::EntryType::History).map(|r| r.id.clone()).unwrap_or_default();
            let grant = win.shell.grants.as_ref().and_then(|g| g.first().map(|r| r.id.clone())).unwrap_or_default();
            match s {
                SheetScene::BackupExport => win.bk_open(Bk::Export),
                SheetScene::RestoreFirstRun => win.bk_open(Bk::Restore(RestoreFrom::FirstRun)),
                SheetScene::RestoreSettings => win.bk_open(Bk::Restore(RestoreFrom::Settings)),
                SheetScene::Mirror => win.bk_open(Bk::Mirror),
                SheetScene::Cli => win.cli_open(),
                SheetScene::NewRecord => win.u3_new_anchor_open(),
                SheetScene::NewRecordConfirm => {
                    win.u3_new_anchor_open();
                    // One file chosen, its fingerprint already taken.
                    let bytes = b"geometry one";
                    win.shell.content = Some(crate::anchorx::Content {
                        source: crate::anchorx::Source::File,
                        digest: crate::anchorx::file_digest(bytes),
                        subject: "/geometry/one.txt".into(),
                        detail: format!("{} bytes", bytes.len()),
                        size: Some(bytes.len() as u64),
                    });
                    win.ux.u3.new_anchor = Some(1);
                }
                SheetScene::Send => {
                    win.sheets_clear();
                    win.shell.gas = Some((1, 21_000));
                    win.ux.u3.send_after_estimate = None;
                    win.ux.u3.confirm = Some(U3Confirm::Send { count: 1 });
                }
                SheetScene::NoEstimate => {
                    win.sheets_clear();
                    win.ux.u3.confirm = Some(U3Confirm::NoEstimate { count: 1, why: "why".into(), next: "next".into(), raw: "raw".into() });
                }
                SheetScene::Clash => {
                    win.sheets_clear();
                    win.clash_modal = true;
                }
                SheetScene::Adopt => {
                    win.u3_form_open(U3Form::Adopt);
                    win.ux.u3.adopt_asked = Some(String::new());
                }
                SheetScene::Succeed => win.u3_form_open(U3Form::Succeed),
                SheetScene::SucceedConfirm => {
                    win.u3_form_open(U3Form::Succeed);
                    win.ux.u3.succeed_confirm = true;
                }
                SheetScene::Annotate => win.annotate_open(String::new()),
                SheetScene::Attest => win.u3_form_open(U3Form::Attest),
                SheetScene::GrantConfirm => {
                    win.sheets_clear();
                    win.typed.g_grantee = format!("0x{}", "33".repeat(20));
                    win.typed.g_work = format!("0x{}", "44".repeat(32));
                    win.typed.g_terms = format!("0x{}", "55".repeat(32));
                    win.ux.u3.confirm = Some(U3Confirm::Grant);
                }
                SheetScene::DeleteRecord => {
                    win.sheets_clear();
                    win.ux.u3.confirm = Some(U3Confirm::Retract { subject: history });
                }
                SheetScene::Revoke => {
                    win.sheets_clear();
                    win.ux.u3.confirm = Some(U3Confirm::Revoke { grant });
                }
                SheetScene::KitPick => {
                    win.sheets_clear();
                    win.ux.u3.kit_pick = Some(KitPick { ids: rows.iter().map(|r| r.id.clone()).collect() });
                }
                SheetScene::WorkPick => {
                    win.sheets_clear();
                    win.ux.u3.work_pick = true;
                }
                SheetScene::AddGrant => win.u4_import_open(),
                SheetScene::IdNew => win.id_layer_open(IdModal::New),
                SheetScene::IdImportWords | SheetScene::IdImportKey | SheetScene::IdImportFile => {
                    win.id_layer_open(IdModal::Import);
                    win.ux.id_tab = match s {
                        SheetScene::IdImportWords => 0,
                        SheetScene::IdImportKey => 1,
                        _ => 2,
                    };
                }
                SheetScene::IdBackup => win.id_layer_open(IdModal::Backup),
                SheetScene::IdPin => win.id_layer_open(IdModal::Pin),
                SheetScene::IdWords => win.id_layer_open(IdModal::Words),
                SheetScene::IdSwitch => win.id_layer_open(IdModal::Switch),
                SheetScene::IdName => win.id_layer_open(IdModal::Name(id.clone().unwrap_or_default())),
                SheetScene::IdDelete => {
                    win.sheets_clear();
                    win.ux.id_modal = Some(IdModal::Delete(id.clone().unwrap_or_default()));
                }
                SheetScene::WizExit => {
                    win.wizard_open(Step::Key);
                    win.ux.wiz_exit_ask = true;
                }
                SheetScene::CreateLedger => {
                    win.shell.rooted = false;
                    win.wizard_open(Step::Genesis);
                    win.ux.confirm_genesis = true;
                }
                SheetScene::WizardAdopt => {
                    win.shell.rooted = false;
                    win.wizard_open(Step::Genesis);
                    win.ux.confirm_adopt = true;
                }
                SheetScene::NetEdit => win.net_edit_open(),
                SheetScene::ReadNet => win.read_net_new_open(),
                SheetScene::PublishCheck => win.ux.publish_sheet = true,
                SheetScene::PublishEdit => win.ux.publish_edit = true,
                SheetScene::ExtractOriginals => {
                    stage_data(win, DataScene::VerifyKit);
                    win.extract_open();
                }
                SheetScene::DataFolder => win.dir_open(settings::DirSheet::Home),
                SheetScene::Snapshot => win.snapshot_open(),
                SheetScene::GrantFile => {
                    win.sheets_clear();
                    win.out_open(grants::OutSheet::GrantFile(grant));
                }
                SheetScene::Badge | SheetScene::HeldUpstream => {
                    win.sheets_clear();
                    let held = win.shell.held.as_ref().and_then(|h| h.first().map(|x| x.id.clone())).unwrap_or_default();
                    win.out_open(if s == SheetScene::Badge { grants::OutSheet::Badge(held) } else { grants::OutSheet::Upstream(held) });
                }
            }
        }
    }
}

/// What the alerts page and the rail's count read with given watch rows.
#[derive(Clone, Debug, PartialEq)]
pub struct AlertsReading {
    /// The rail's count.
    pub count: usize,
    /// Every text the page drew under its toolbar (the page body), in drawing order.
    pub texts: Vec<String>,
}

/// Draw the alerts page with `rows` as the watch's reading (in place of the shell's) in a few windowless frames;
/// what the page drew and the rail's count.
pub fn probe_alerts(ctx: &egui::Context, shell: Shell, rows: Vec<crate::watchx::Row>, width: f32, height: f32) -> (Shell, AlertsReading) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::View(crate::nav::View::Alerts, 0), role));
    ux.wizard_asked = true;
    ux.watch_rows = Some(rows);
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut out = None;
    let mut t = 0.0;
    for _ in 0..6 {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        out = Some(ctx.run_ui(input, |ui| win.draw(ui)));
    }
    let out = out.expect("frames ran");
    let texts = texts_of(&out).into_iter().filter(|d| d.rect.top() > zikaron_ui::tokens::TOOLBAR_H && d.rect.left() > zikaron_ui::tokens::RAIL_W).map(|d| d.text).collect();
    let count = win.alert_count();
    (win.shell, AlertsReading { count, texts })
}

/// A still frame of a scene as it would be painted: its tessellated shapes and every texture they sample, at
/// `ppp` physical pixels per point (test drivers turn it into a picture without a window).
pub struct Picture {
    pub prims: Vec<egui::ClippedPrimitive>,
    pub textures: std::collections::HashMap<egui::TextureId, egui::ColorImage>,
    pub ppp: f32,
}

/// The textures a frame set or patched, kept for painting a later frame.
fn take_textures(textures: &mut std::collections::HashMap<egui::TextureId, egui::ColorImage>, out: &egui::FullOutput) {
    for (id, delta) in &out.textures_delta.set {
        let egui::epaint::ImageData::Color(img) = &delta.image;
        match delta.pos {
            None => {
                textures.insert(*id, (**img).clone());
            }
            Some([x, y]) => {
                if let Some(t) = textures.get_mut(id) {
                    for row in 0..img.size[1] {
                        for col in 0..img.size[0] {
                            let (tx, ty) = (x + col, y + row);
                            if tx < t.size[0] && ty < t.size[1] {
                                t.pixels[ty * t.size[0] + tx] = img.pixels[row * img.size[0] + col];
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Draw `scene` the way [`probe_geometry`] does (windowless frames until still, background tasks landed) and keep
/// what a painter would need for its last frame.
pub fn probe_picture(ctx: &egui::Context, shell: Shell, scene: Scene, width: f32, height: f32, ppp: f32) -> (Shell, Picture) {
    let role = shell.settings.role;
    let place = match scene {
        Scene::Page(p) | Scene::Sheet(p, _) | Scene::Data(p, _) => p,
        _ => Place::Home,
    };
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    ux.lang_applied = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let kept = (win.shell.vault.clone(), win.shell.anchor.clone(), win.shell.rooted, win.shell.machine.backup.clone(), win.shell.primary.clone(), win.shell.backup_peek.clone(), win.shell.gas, win.shell.vault_recoverable);
    let kept_data = (win.shell.checked.clone(), win.shell.key_anchors.clone(), win.shell.held.clone(), win.shell.verified.clone(), win.shell.book.clone(), win.shell.diligence.clone());
    stage(&mut win, scene);
    stage_memory(ctx, scene);
    ctx.set_pixels_per_point(ppp);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut textures: std::collections::HashMap<egui::TextureId, egui::ColorImage> = std::collections::HashMap::new();
    let mut take = |out: &egui::FullOutput| take_textures(&mut textures, out);
    let mut t = 0.0;
    let mut out = None;
    for i in 0..20 {
        t += 0.1;
        if let Scene::IdMenu = scene {
            zikaron_ui::menu::set(ctx, egui::Id::new("zikaron-id-menu"), true);
        }
        if i == 14 {
            let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
            while !win.shell.tasks.flying().is_empty() && std::time::Instant::now() < until {
                win.shell.drain();
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        let o = ctx.run_ui(input, |ui| win.draw(ui));
        take(&o);
        out = Some(o);
    }
    let out = out.expect("frames ran");
    let prims = ctx.tessellate(out.shapes, out.pixels_per_point);
    zikaron_ui::menu::set(ctx, egui::Id::new("zikaron-id-menu"), false);
    let mut shell = win.shell;
    (shell.vault, shell.anchor, shell.rooted, shell.machine.backup, shell.primary, shell.backup_peek, shell.gas, shell.vault_recoverable) = kept;
    (shell.checked, shell.key_anchors, shell.held, shell.verified, shell.book, shell.diligence) = kept_data;
    (shell, Picture { prims, textures, ppp: out.pixels_per_point })
}

/// The whole-machine backup's export sheet, filled and pressed: every frame while the passcode check and the export
/// run (equal frames counted together), Esc and a press on its back key while the export runs, then the sheet after
/// it landed. With `picture`, the last frame drawn while the export is at that stage is kept.
pub fn probe_backup_export(ctx: &egui::Context, shell: Shell, pin: &str, password: &str, dir: &str, picture: Option<u8>, width: f32, height: f32) -> (Shell, BackupRun) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Settings(crate::nav::Section::Data), role));
    ux.wizard_asked = true;
    ux.lang_applied = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut textures = std::collections::HashMap::new();
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    let tap = |at: egui::Pos2| {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        vec![egui::Event::PointerMoved(at), press(true), press(false)]
    };
    let at = |out: &egui::FullOutput, s: &str| texts_of(out).iter().rev().find(|d| d.text == s).map(|d| d.rect.center());
    win.bk_open(Bk::Export);
    win.ux.bk_pin = pin.into();
    win.ux.bk_pw = password.into();
    win.ux.bk_pw2 = password.into();
    win.ux.bk_dir = dir.to_string();
    let mut out = run(&mut win, Vec::new());
    for _ in 0..8 {
        take_textures(&mut textures, &out);
        out = run(&mut win, Vec::new());
    }
    let mut reading = BackupRun::default();
    let Some(go) = at(&out, t(Key::IdExportGroup)) else { return (win.shell, reading) };
    reading.pressed = true;
    let words: Vec<&str> = stage_words(crate::task::Kind::Backup).iter().map(|k| t(*k)).collect();
    let mut events = tap(go);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(120);
    loop {
        let out = run(&mut win, std::mem::take(&mut events));
        take_textures(&mut textures, &out);
        let running = win.shell.tasks.in_flight(crate::task::Kind::Backup);
        let busy = running || win.shell.tasks.in_flight(crate::task::Kind::Vault);
        let stage = if running { crate::task::stage(crate::task::Kind::Backup).map(|s| s.at) } else { None };
        let drawn = texts_of(&out);
        let frame = BackupFrame {
            open: win.ux.bk == Some(Bk::Export),
            running,
            stage,
            words: words.iter().filter(|w| drawn.iter().any(|d| d.text == **w)).map(|w| w.to_string()).collect(),
            n: 1,
        };
        if picture.is_some() && stage == picture {
            reading.picture = Some(Picture { prims: ctx.tessellate(out.shapes.clone(), out.pixels_per_point), textures: textures.clone(), ppp: out.pixels_per_point });
        }
        match reading.frames.last_mut() {
            Some(last) if BackupFrame { n: last.n, ..frame.clone() } == *last => last.n += 1,
            _ => reading.frames.push(frame),
        }
        // While the export runs: Esc, then a press on the back key.
        if running && stage.is_some() {
            if reading.open_after_esc.is_none() {
                let _ = run(&mut win, vec![egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }]);
                let o = run(&mut win, Vec::new());
                reading.open_after_esc = Some(win.ux.bk == Some(Bk::Export));
                if let Some(back) = at(&o, t(Key::CfBack)) {
                    let _ = run(&mut win, tap(back));
                    let _ = run(&mut win, Vec::new());
                    reading.open_after_back = Some(win.ux.bk == Some(Bk::Export));
                }
            }
        }
        if !busy || std::time::Instant::now() > until {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    for _ in 0..8 {
        let _ = run(&mut win, Vec::new());
    }
    reading.open_after = win.ux.bk == Some(Bk::Export);
    reading.trouble = win.ux.bk_trouble.clone().or_else(|| win.shell.failed.get(&crate::task::Kind::Backup).cloned());
    (win.shell, reading)
}

/// The ledger's "add a note" sheet: opened with an entry chosen (as the entry's own page opens it) and cancelled;
/// opened from the ledger's "more" menu and cancelled; opened with the entry again and written; then opened from the
/// menu, an entry picked in its picker, then the picker's first item, and written. The entry the sheet names at each
/// point, and whether "no entry" stood in its field when it opened from the menu.
pub fn probe_annotate_target(ctx: &egui::Context, shell: Shell, entry: &str, seq: u64, width: f32, height: f32) -> (Shell, AnnotateTarget) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::View(crate::nav::View::Log, 0), role));
    ux.wizard_asked = true;
    ux.lang_applied = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    let tap = |at: egui::Pos2| {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        vec![egui::Event::PointerMoved(at), press(true), press(false)]
    };
    let settle = |win: &mut Win, run: &mut dyn FnMut(&mut Win, Vec<egui::Event>) -> egui::FullOutput| {
        let mut out = run(win, Vec::new());
        for _ in 0..8 {
            out = run(win, Vec::new());
        }
        texts_of(&out)
    };
    let at = |texts: &[Drawn], s: &str| texts.iter().rev().find(|d| d.text == s).map(|d| d.rect.center());
    let press_on = |win: &mut Win, run: &mut dyn FnMut(&mut Win, Vec<egui::Event>) -> egui::FullOutput, s: &str| -> bool {
        let texts = settle(win, run);
        match at(&texts, s) {
            Some(p) => {
                let _ = run(win, tap(p));
                true
            }
            None => false,
        }
    };
    let mut reading = AnnotateTarget::default();
    // From the entry's page (the entry chosen there), cancelled; then from the ledger's "more" menu.
    win.annotate_open(entry.to_string());
    reading.from_detail = win.typed.annotate_subject.clone();
    reading.cancelled = press_on(&mut win, &mut run, t(Key::CfBack));
    let _ = settle(&mut win, &mut run);
    let menu = press_on(&mut win, &mut run, t(Key::NavMore)) && press_on(&mut win, &mut run, t(Key::U3AnnotateGo));
    let texts = settle(&mut win, &mut run);
    reading.reopened = menu && matches!(win.ux.u3.form, Some(U3Form::Annotate));
    reading.reopened_names = win.typed.annotate_subject.clone();
    reading.none_shown = texts.iter().any(|d| d.text == t(Key::LedgerAnnotateNone));
    let _ = press_on(&mut win, &mut run, t(Key::CfBack));
    // From the entry's page again, written.
    win.annotate_open(entry.to_string());
    win.typed.annotate_note = "one".to_string();
    reading.wrote_first = press_on(&mut win, &mut run, t(Key::U3AnnotateGo));
    let _ = settle(&mut win, &mut run);
    reading.after_write = win.typed.annotate_subject.clone();
    reading.closed = win.ux.u3.form.is_none();
    let _ = press_on(&mut win, &mut run, t(Key::NavMore)) && press_on(&mut win, &mut run, t(Key::U3AnnotateGo));
    // The picker: an entry, then the first item.
    if press_on(&mut win, &mut run, t(Key::KitPickOpen)) && press_on(&mut win, &mut run, &format!("#{seq}")) {
        let _ = settle(&mut win, &mut run);
        reading.after_pick = win.typed.annotate_subject.clone();
    }
    if press_on(&mut win, &mut run, t(Key::KitPickOpen)) && press_on(&mut win, &mut run, t(Key::LedgerAnnotateNone)) {
        let _ = settle(&mut win, &mut run);
        reading.after_none = win.typed.annotate_subject.clone();
    }
    win.typed.annotate_note = "two".to_string();
    reading.wrote_second = press_on(&mut win, &mut run, t(Key::U3AnnotateGo));
    let _ = settle(&mut win, &mut run);
    (win.shell, reading)
}
