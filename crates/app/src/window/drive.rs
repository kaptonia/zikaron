use super::*;

pub fn run() -> i32 {
    run_at(Start::Place(Place::Home, false))
}

pub fn run_on(start: Page) -> i32 {
    run_at(Start::Page(start))
}

pub fn run_at(start: Start) -> i32 {
    run_hooked(start, None)
}

pub struct Hooks {
    pub float: bool,
    pub input: Box<dyn FnMut(&egui::Context, &mut egui::RawInput)>,
    pub frame: Box<dyn FnMut(&egui::Context, Option<f32>)>,
}

struct Hooked {
    win: Win,
    hooks: Hooks,
}

impl eframe::App for Hooked {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw: &mut egui::RawInput) {
        eframe::App::raw_input_hook(&mut self.win, ctx, raw);
        (self.hooks.input)(ctx, raw);
    }

    fn logic(&mut self, ctx: &egui::Context, f: &mut eframe::Frame) {
        eframe::App::logic(&mut self.win, ctx, f);
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

pub fn run_hooked(start: Start, hooks: Option<Hooks>) -> i32 {
    let float = hooks.as_ref().is_some_and(|h| h.float);
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(t(Key::AppName))
            .with_app_id(own::APP_ID)
            .with_icon(std::sync::Arc::new(own::icon()))
            .with_inner_size([W, H])
            .with_min_inner_size([MIN_W, MIN_H])
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_shown(false)
            .with_transparent(zikaron_os::backdrop_available())
            .with_window_level(if float { egui::WindowLevel::AlwaysOnTop } else { egui::WindowLevel::Normal }),
        #[cfg(target_os = "macos")]
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
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
            let settled = crate::home::settle_machine();
            let mut shell = Shell::boot(dressed);
            if let Err(f) = settled {
                shell.faults.push(f);
            }
            let wake = cc.egui_ctx.clone();
            shell.door_waker = Some(std::sync::Arc::new(move || wake.request_repaint()));
            let woken = cc.egui_ctx.clone();
            shell.tasks.wake_with(Some(std::sync::Arc::new(move || woken.request_repaint())));
            if own::MENU_BAR {
                let wake = cc.egui_ctx.clone();
                crate::platform::menubar::on_ask(Box::new(move || wake.request_repaint()));
            }
            let root = crate::action::start(&mut shell);
            if let Some(l) = shell.speaks() {
                crate::lang::set(l);
            }
            crate::when::set(crate::when::chosen(shell.settings.zone));
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
                Start::Scene(_) => own_start(&start, &mut ux, shell.settings.role),
            }
            let typed = Typed { home: root, ..Default::default() };
            let mut win = fresh_win(shell, typed, ux);
            own_stage(&mut win, &start);
            win.asks_motion = true;
            win.reads_behind = true;
            {
                use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
                let at = match (cc.display_handle().map(|h| h.as_raw()), cc.window_handle().map(|h| h.as_raw())) {
                    (Ok(RawDisplayHandle::Wayland(d)), Ok(RawWindowHandle::Wayland(w))) => crate::platform::drops::Window::Wayland { display: d.display.as_ptr(), surface: w.surface.as_ptr() },
                    _ => crate::platform::drops::Window::Other,
                };
                let wake = cc.egui_ctx.clone();
                win.drops = crate::platform::drops::watch(at, std::sync::Arc::new(move || wake.request_repaint()));
            }
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
            let why = e.to_string();
            let line = fill1(Key::SaidWindowFailed, &why);
            speak_this_machines_language();
            without_window(&line, fill1(Key::SaidNoWindowFailed, &why));
            1
        }
    }
}

pub fn probe_face(ctx: &egui::Context, shell: Shell, place: Place, width: f32, height: f32) -> (Shell, FaceReading) {
    face_read(ctx, shell, place, width, height, false, None)
}

pub fn probe_face_landed(ctx: &egui::Context, shell: Shell, place: Place, width: f32, height: f32) -> (Shell, FaceReading) {
    face_read(ctx, shell, place, width, height, true, None)
}

pub fn probe_face_landed_still(ctx: &egui::Context, shell: Shell, place: Place, width: f32, height: f32, frames: usize, edge: &mut dyn FnMut(bool)) -> (Shell, FaceReading) {
    face_read(ctx, shell, place, width, height, true, Some((frames, edge)))
}

fn face_read(ctx: &egui::Context, shell: Shell, place: Place, width: f32, height: f32, landed: bool, still: Option<(usize, &mut dyn FnMut(bool))>) -> (Shell, FaceReading) {
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
    for _ in 0..5 {
        let _ = run(&mut win, Vec::new(), 0.1);
    }
    for _ in 0..if landed { 30 } else { 0 } {
        if win.shell.tasks.flying().is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
        let _ = run(&mut win, Vec::new(), 0.1);
    }
    if let Some((frames, edge)) = still {
        edge(true);
        for _ in 0..frames {
            let _ = run(&mut win, Vec::new(), 0.1);
        }
        edge(false);
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
    let tables = zikaron_ui::probe::tooks(ctx);
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
        FaceReading { width, right_widget, right_text, rows: rows.len(), tables, inner, inner_right, inner_head, inner_head_keys, inner_history, route, inner_route, texts: words, cells, justified_texts, tiles },
    )
}

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

pub fn probe_relicense(ctx: &egui::Context, shell: Shell, place: Place, upstream: &str, signed: bool, width: f32, height: f32) -> (Shell, Vec<Drawn>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    ux.u4.relicense_signed = signed;
    let typed = Typed { g_upstream: upstream.to_string(), ..Typed::default() };
    let mut win = fresh_win(shell, typed, ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win| {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    let _ = run(&mut win);
    win.push(Route::Relicense, 0.1);
    for _ in 0..8 {
        let _ = run(&mut win);
    }
    let out = run(&mut win);
    (win.shell, texts_of(&out))
}

pub fn probe_relicense_sign(ctx: &egui::Context, shell: Shell, place: Place, form: [&str; 4], scroll: f32, width: f32, height: f32) -> (Shell, RelicenseSign) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let [upstream, grantee, work, terms] = form.map(str::to_string);
    let typed = Typed { g_upstream: upstream, g_grantee: grantee, g_work: work, g_terms: terms, ..Typed::default() };
    let mut win = fresh_win(shell, typed, ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        ctx.run_ui(egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() }, |ui| win.draw(ui))
    };
    let press_at = |at: egui::Pos2| {
        let b = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        vec![egui::Event::PointerMoved(at), b(true), b(false)]
    };
    let mut r = RelicenseSign::default();
    let _ = run(&mut win, Vec::new());
    win.push(Route::Relicense, 0.1);
    for _ in 0..8 {
        let _ = run(&mut win, Vec::new());
    }
    let mid = egui::pos2(width * 0.6, height * 0.6);
    let wheel = vec![egui::Event::PointerMoved(mid), egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, -scroll), phase: egui::TouchPhase::Move, modifiers: egui::Modifiers::NONE }];
    let _ = run(&mut win, wheel);
    let mut out = run(&mut win, Vec::new());
    for _ in 0..8 {
        out = run(&mut win, Vec::new());
    }
    r.scroll_before = win.ux.nav.scroll_y;
    if let Some(at) = texts_of(&out).into_iter().filter(|d| d.text == t(Key::GrantConfirm)).last().map(|d| d.rect.center()) {
        let _ = run(&mut win, press_at(at));
        r.confirm_pressed = true;
    }
    for _ in 0..6 {
        out = run(&mut win, Vec::new());
    }
    let go = t(win.anchor_key());
    if let Some(at) = texts_of(&out).into_iter().filter(|d| d.text == go).last().map(|d| d.rect.center()) {
        let _ = run(&mut win, press_at(at));
        r.go_pressed = true;
    }
    for k in 0..400 {
        if win.ux.u4.relicense_signed {
            r.signed = true;
            r.frames_to_sign = k;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        let _ = run(&mut win, Vec::new());
    }
    for _ in 0..8 {
        out = run(&mut win, Vec::new());
    }
    r.scroll_after = win.ux.nav.scroll_y;
    r.texts = texts_of(&out);
    let wheel = vec![egui::Event::PointerMoved(mid), egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: egui::vec2(0.0, -scroll), phase: egui::TouchPhase::Move, modifiers: egui::Modifiers::NONE }];
    let _ = run(&mut win, wheel);
    for _ in 0..8 {
        let _ = run(&mut win, Vec::new());
    }
    r.scroll_again = win.ux.nav.scroll_y;
    (win.shell, r)
}

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
    for _ in 0..15 {
        let (_, out) = run(&mut win, 0.02);
        let band: Vec<String> = texts_of(&out).into_iter().filter(|d| d.rect.top() < tk::TOOLBAR_H && d.rect.left() > tk::RAIL_W && d.text.trim().len() > 0).map(|d| d.text).collect();
        frames.push(band);
    }
    (win.shell, frames)
}

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

#[derive(Clone, Debug, Default)]
pub struct WayOut {
    pub offered: Vec<bool>,
    pub exit_drawn: Vec<bool>,
    pub steps: Vec<String>,
    pub left_by_click: bool,
    pub still_open: bool,
    pub place_after: String,
    pub seat_has_key_after: bool,
    pub reopened: bool,
    pub reopened_step: String,
    pub reopened_offered: bool,
}

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
    if !reading.reopened {
        again.arrive(Page::FirstRun);
        for _ in 0..4 {
            let _ = run(&mut again, Vec::new());
        }
    }
    reading.reopened_step = again.ux.wizard.map(|s| format!("{s:?}")).unwrap_or_default();
    reading.reopened_offered = again.wizard_exit_shown();
    (again.shell, reading)
}

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
        let mut input = egui::RawInput { screen_rect: Some(screen), time: Some(0.1 * (k + 1) as f64), ..Default::default() };
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(2.0);
        out = Some(ctx.run_ui(input, |ui| win.draw(ui)));
    }
    let out = out.unwrap_or_default();
    let mut reading = GateColumn { window_h: height, group_top: 0.0, title_ink_bottom: 0.0, cells: egui::Rect::NOTHING, said: None, forgot: egui::Rect::NOTHING, own_key: egui::Rect::NOTHING };
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
        reading.forgot = rects.iter().rev().copied().filter(|r| r.contains(at) && r.height() < 40.0).min_by(|a, b| a.area().total_cmp(&b.area())).unwrap_or(egui::Rect::NOTHING);
    }
    if let Some(at) = own::GATE_KEY.and_then(|k| texts_of(&out).iter().find(|d| d.text == t(k)).map(|d| d.rect.center())) {
        reading.own_key = rects.iter().rev().copied().filter(|r| r.contains(at) && r.height() < 40.0).min_by(|a, b| a.area().total_cmp(&b.area())).unwrap_or(egui::Rect::NOTHING);
    }
    if let Some(said) = trouble {
        let at = texts_of(&out).iter().find(|d| d.text == said).map(|d| d.rect.center());
        reading.said = at.and_then(|p| rects.iter().rev().copied().filter(|r| r.contains(p) && r.width() < width - 100.0).min_by(|a, b| a.area().total_cmp(&b.area())));
    }
    (win.shell, reading)
}

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
    let at = texts_of(&out).into_iter().filter(|d| d.text == words).min_by(|a, b| a.rect.top().total_cmp(&b.rect.top())).map(|d| d.rect.center());
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

pub type RailFrame = (String, Vec<String>);

pub fn probe_rail_back(ctx: &egui::Context, shell: Shell, place: Place, away: crate::nav::View, width: f32, height: f32) -> (Shell, Vec<RailFrame>) {
    let role = shell.settings.role;
    let rail_words = |to: Place| crate::nav::rail(role).iter().flat_map(|g| g.items.iter()).find(|i| i.place == to).map(|i| t(i.name));
    let (Some(back_words), Some(away_words)) = (rail_words(place), rail_words(Place::View(away, 0))) else { return (shell, Vec::new()) };
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, modifiers: egui::Modifiers::NONE, ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    let press_on = |out: &egui::FullOutput, words: &str| -> Option<Vec<egui::Event>> {
        let at = texts_of(out).into_iter().find(|d| d.rect.right() <= tk::RAIL_W && d.text == words)?.rect.center();
        let button = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        Some(vec![egui::Event::PointerMoved(at), button(true), button(false)])
    };
    let frame_of = |out: &egui::FullOutput| -> RailFrame {
        let texts = texts_of(out).into_iter().filter(|d| d.rect.left() > tk::RAIL_W && !d.text.trim().is_empty()).map(|d| d.text).collect();
        (zikaron_ui::probe::route_drawn(ctx), texts)
    };
    let mut out = run(&mut win, Vec::new());
    for _ in 0..5 {
        out = run(&mut win, Vec::new());
    }
    let Some(away_press) = press_on(&out, away_words) else { return (win.shell, Vec::new()) };
    let _ = run(&mut win, away_press);
    let _ = run(&mut win, vec![egui::Event::PointerGone]);
    for _ in 0..5 {
        out = run(&mut win, Vec::new());
    }
    let Some(back_press) = press_on(&out, back_words) else { return (win.shell, Vec::new()) };
    let mut frames = Vec::new();
    out = run(&mut win, back_press);
    frames.push(frame_of(&out));
    out = run(&mut win, vec![egui::Event::PointerGone]);
    frames.push(frame_of(&out));
    for _ in 0..5 {
        out = run(&mut win, Vec::new());
        frames.push(frame_of(&out));
    }
    (win.shell, frames)
}

pub fn probe_wizard_pin(ctx: &egui::Context, shell: Shell, typed: &str, width: f32, height: f32) -> (Shell, Vec<String>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    win.wizard_open(crate::nav::Step::Pin);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    for _ in 0..6 {
        let _ = run(&mut win, Vec::new());
    }
    let _ = run(&mut win, typed.chars().map(|c| egui::Event::Text(c.to_string())).collect());
    let mut out = None;
    for _ in 0..4 {
        out = Some(run(&mut win, Vec::new()));
    }
    let texts = out.as_ref().map(texts_of).unwrap_or_default().into_iter().map(|d| d.text).collect();
    (win.shell, texts)
}
