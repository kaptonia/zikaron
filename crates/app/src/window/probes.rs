use super::*;

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

pub fn probe_ledger_set_up(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, bool, String) {
    let (shell, drawn, step, _) = probe_ledger_set_up_read(ctx, shell, width, height);
    (shell, drawn, step)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TableRead {
    pub at_frame: usize,
    pub read: bool,
}

const SETTLE_WAIT: std::time::Duration = std::time::Duration::from_secs(60);

pub fn probe_ledger_set_up_read(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, bool, String, TableRead) {
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
    let until = std::time::Instant::now() + SETTLE_WAIT;
    let mut read = TableRead::default();
    let mut frames = 0usize;
    loop {
        let landed = win.shell.rows_current() && !win.shell.tasks.in_flight(crate::task::Kind::Ledger);
        if landed && !read.read {
            read = TableRead { at_frame: frames, read: true };
        }
        if landed && frames >= 10 {
            break;
        }
        if std::time::Instant::now() > until {
            break;
        }
        if !landed {
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        out = run(&mut win, Vec::new());
        frames += 1;
    }
    for _ in 0..2 {
        out = run(&mut win, Vec::new());
    }
    let word = t(Key::WizGenesisTitle);
    let rows = zikaron_ui::probe::rows(ctx);
    let at = texts_of(&out).iter().find(|d| d.text == word && !rows.iter().any(|r| r.contains(d.rect.center()))).map(|d| d.rect.center());
    let Some(at) = at else { return (win.shell, false, String::new(), read) };
    let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
    let _ = run(&mut win, vec![egui::Event::PointerMoved(at), press(true), press(false)]);
    for _ in 0..4 {
        let _ = run(&mut win, Vec::new());
    }
    let step = win.ux.wizard.map(|s| format!("{s:?}")).unwrap_or_default();
    (win.shell, true, step, read)
}

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

pub fn probe_gate_aside(ctx: &egui::Context, shell: Shell, press: bool, confirm: bool, width: f32, height: f32) -> (Shell, GateAside) {
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

pub fn probe_upgrade_sheet(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, bool, Vec<Drawn>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
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
    let open = win.ux.upgrade_open;
    (win.shell, open, drawn)
}

pub fn probe_upgrade_walk(ctx: &egui::Context, shell: Shell, seconds: f64) -> (Shell, bool, Vec<String>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 760.0));
    let began = std::time::Instant::now();
    let mut clock = 0.0;
    let frame = |win: &mut Win, clock: &mut f64| {
        *clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(*clock), ..Default::default() };
        let _ = ctx.run_ui(input, |ui| win.draw(ui));
    };
    win.upgrade_sheet_open();
    while win.shell.upgrade_cost.is_none() && began.elapsed().as_secs_f64() < seconds {
        frame(&mut win, &mut clock);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let offered = win.shell.upgrade_cost.is_some();
    win.ux.upgrade_auto = true;
    win.shell.failed.remove(&crate::task::Kind::Anchor);
    let mut seen: Vec<String> = Vec::new();
    while began.elapsed().as_secs_f64() < seconds {
        frame(&mut win, &mut clock);
        let stage = win.shell.upgrade.as_ref().map(|s| s.as_str().to_string()).unwrap_or_default();
        if seen.last() != Some(&stage) {
            seen.push(stage.clone());
        }
        if stage == "done" || !win.ux.upgrade_auto {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    (win.shell, offered, seen)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendEntry {
    Pending,
    Ledger,
    Alerts,
}

pub(super) fn pressable(ctx: &egui::Context, at: egui::Pos2) -> bool {
    let under: Vec<(egui::LayerId, egui::Rect, bool)> = ctx.viewport(|v| {
        v.prev_pass.widgets.layers().flat_map(|(_, ws)| ws.iter().map(|w| (w.layer_id, w.interact_rect, w.enabled && w.sense.senses_click()))).collect()
    });
    under
        .into_iter()
        .map(|(layer, r, live)| (ctx.layer_transform_to_global(layer).map_or(r, |to| to * r), live))
        .filter(|(r, _)| r.contains(at))
        .min_by(|a, b| a.0.area().total_cmp(&b.0.area()))
        .is_some_and(|(_, live)| live)
}

pub fn send_probe_at(ctx: &egui::Context, shell: Shell, entry: SendEntry) -> SendProbe {
    let role = shell.settings.role;
    let place = match entry {
        SendEntry::Pending => Place::View(crate::nav::View::Works, crate::nav::tab::WORKS_PENDING),
        SendEntry::Ledger => Place::View(crate::nav::View::Log, 0),
        SendEntry::Alerts => Place::View(crate::nav::View::Alerts, 0),
    };
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 1600.0));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut quiet = 0;
    let (key, drawn) = loop {
        let before = win.shell.tasks.flying().is_empty();
        let out = run(&mut win, Vec::new());
        let at = texts_of(&out).iter().find(|d| d.text == t(Key::U3SendAll)).map(|d| d.rect.center());
        let key = at.filter(|at| pressable(ctx, *at));
        quiet = if before && win.shell.tasks.flying().is_empty() { quiet + 1 } else { 0 };
        if key.is_some() || quiet >= 30 || std::time::Instant::now() >= until {
            break (key, at.is_some());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    };
    let held = match (key, drawn) {
        (Some(_), _) => None,
        (None, false) => Some(SendHeld::NotDrawn),
        (None, true) => Some(match win.send_now() {
            super::ledger::SendNow::Reading => SendHeld::Reading,
            super::ledger::SendNow::Out => SendHeld::Out,
            super::ledger::SendNow::Nothing => SendHeld::Nothing,
            super::ledger::SendNow::Can(_) => SendHeld::Greyed,
        }),
    };
    if let Some(at) = key {
        let click = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at)]);
        let _ = run(&mut win, vec![click(true)]);
        let _ = run(&mut win, vec![click(false)]);
        for _ in 0..3 {
            let _ = run(&mut win, Vec::new());
        }
    }
    SendProbe::over(win, clock, held)
}

pub fn probe_upgrade_step(shell: Shell) -> (Shell, bool) {
    let mut ux = Ux::default();
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    win.ux.upgrade_auto = true;
    win.upgrade_walk(1.0);
    let up = win.ux.u3.confirm.is_some();
    (win.shell, up)
}

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

#[derive(Clone, Debug)]
pub enum NetPress {
    AddRpc(String),
    AddRest(String, String),
    AtRowEnd(String),
    Key(String),
}

pub fn probe_network_press(ctx: &egui::Context, shell: Shell, press: NetPress, width: f32, height: f32) -> (Shell, Vec<String>, Vec<String>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Settings(crate::nav::Section::Network), role));
    ux.wizard_asked = true;
    let mut typed = Typed::default();
    match &press {
        NetPress::AddRpc(text) => typed.rpc_new = text.clone(),
        NetPress::AddRest(base, key) => {
            typed.rest_base = base.clone();
            typed.rest_key = crate::secret::Secret::of(key);
        }
        NetPress::AtRowEnd(_) | NetPress::Key(_) => {}
    }
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
    for _ in 0..8 {
        out = run(&mut win, Vec::new());
    }
    let drawn = texts_of(&out);
    let before: Vec<String> = drawn.iter().map(|d| d.text.clone()).collect();
    let mut adds: Vec<egui::Rect> = drawn.iter().filter(|d| d.text == t(Key::ReadNetAdd)).map(|d| d.rect).collect();
    adds.sort_by(|a, b| a.top().total_cmp(&b.top()));
    let row_end = |words: &str| -> Option<egui::Pos2> {
        let r = drawn.iter().find(|d| d.text == words)?.rect;
        let keys = ctx.viewport(|v| v.prev_pass.widgets.layers().flat_map(|(_, ws)| ws.iter().filter(|w| w.sense.senses_click() && w.rect.width() < 80.0 && w.rect.height() < 60.0).map(|w| w.rect).collect::<Vec<_>>()).collect::<Vec<_>>());
        keys.into_iter().filter(|k| k.top() <= r.center().y && k.bottom() >= r.center().y && k.left() > r.left()).max_by(|a, b| a.left().total_cmp(&b.left())).map(|k| k.center())
    };
    let at = match &press {
        NetPress::AddRpc(_) => adds.first().map(|r| r.center()),
        NetPress::AddRest(..) => drawn.iter().find(|d| d.text == t(Key::NetRestKey)).and_then(|k| adds.iter().find(|r| r.top() > k.rect.bottom())).map(|r| r.center()),
        NetPress::AtRowEnd(words) => row_end(words),
        NetPress::Key(words) => drawn.iter().find(|d| d.text == *words).map(|d| d.rect.center()),
    };
    if let Some(at) = at {
        let click = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at), click(true)]);
        let _ = run(&mut win, vec![click(false)]);
    }
    for _ in 0..6 {
        out = run(&mut win, Vec::new());
    }
    let after = texts_of(&out).into_iter().map(|d| d.text).collect();
    (win.shell, before, after)
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostInput {
    Typed,
    Pasted,
    PastedByChar,
}

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
    for label in labels.clone() {
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
                        if let Some(row) = zikaron_ui::probe::rows(ctx).first().map(|r| r.center()) {
                            click(&mut win, &mut run, row);
                            let out = settle(&mut win, &mut run);
                            let other = labels.iter().find(|l| **l != label).and_then(|o| texts_of(&out).iter().find(|d| d.text == *o && d.rect.center().x < tk::RAIL_W).map(|d| d.rect.center()));
                            if let Some(other) = other {
                                click(&mut win, &mut run, other);
                                let out = settle(&mut win, &mut run);
                                if let Some(item) = on_rail(&out) {
                                    click(&mut win, &mut run, item);
                                    let _ = settle(&mut win, &mut run);
                                    w.from_elsewhere = Some(zikaron_ui::probe::route_drawn(ctx));
                                    let key = egui::Event::Key { key: egui::Key::CloseBracket, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND };
                                    let _ = run(&mut win, vec![key]);
                                    let _ = settle(&mut win, &mut run);
                                    w.forward = Some(zikaron_ui::probe::route_drawn(ctx));
                                }
                            }
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

#[derive(Clone, Debug, Default)]
pub struct HeldBackReading {
    pub said: String,
    pub toast: Option<String>,
    pub keys: Vec<String>,
    pub route_after_key: String,
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Page(Place),
    Wizard(crate::nav::Step),
    Sheet(Place, SheetScene),
    Gate(GateScene),
    IdMenu,
    Data(Place, DataScene),
}

const BAD_CELL: &str = "0x12zz";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataScene {
    CheckNoNode,
    CheckSourceOpen,
    VerifyKit,
    VerifyGrant,
    OthersRead,
    OthersList,
    OtherEntry,
    OthersBadAuthor,
    GrantBadGrantee,
    CheckBadRegistry,
    NewRecordBatch,
    GrantDetail,
    HeldListed,
    HeldDetail,
    WorkDetail,
    EntryDetail,
    PendingDetail,
    KitExport,
    KitPageSealed,
    KitExportSealed,
    KitExportSealedTo,
    KitExportSealedKey,
    KeysDetails,
}

pub const LONG_RELEASE: &str = "name \u{b7} The Quite Long Registered Name Of A Studio Whose Binding Is Let Go Here, Read Whole On Its Sheet";

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
    SendFollows,
    NoEstimate,
    Clash,
    Annotate,
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
    HeldUpstreamSaved,
    PublishEdit,
    ExtractOriginals,
    AuditCard,
    Upgrade,
    Rotate,
    RotateOwed,
    RotateBookless,
    RotateLocal,
    RotateGiven,
    RotateConfirm,
    RotateDeclared,
    RotateTakenUp,
    MoveChain,
    Clean,
    Consent,
    ConsentMade,
    CliPass,
    CliPassMade,
    PersonaName,
    PersonaAccount,
    PersonaKeyHere,
    PersonaKeyOther,
    PersonaKeyBad,
    ConsentBad,
    RotateGivenBad,
    MoveChainBad,
    NetEditBad,
    ReadNetBad,
    PersonaRelease,
    PersonaReleaseLong,
    PersonaHome,
    IdPrimary,
    IdDeleteLedgers,
    RestoreNewPin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateScene {
    Unlock,
    RecoverWords,
    RecoverFile,
    RecoverNone,
    LockedOut,
    RestoreLocked,
    RestoreNewPin,
    Damaged,
    Aside,
}

impl Scene {
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
    let persona = Place::View(View::Persona, crate::nav::tab::PERSONA_OVERVIEW);
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
        (keys, SheetScene::Rotate),
        (Place::Settings(Section::Network), SheetScene::MoveChain),
        (data, SheetScene::Clean),
        (Place::Home, SheetScene::WizExit),
        (Place::Home, SheetScene::CreateLedger),
        (Place::Home, SheetScene::WizardAdopt),
        (Place::Settings(Section::Network), SheetScene::NetEdit),
        (Place::Settings(Section::Network), SheetScene::ReadNet),
        (Place::Settings(Section::Network), SheetScene::NetEditBad),
        (Place::Settings(Section::Network), SheetScene::MoveChainBad),
        (Place::Settings(Section::Network), SheetScene::ReadNetBad),
        (Place::Settings(Section::Network), SheetScene::PublishCheck),
        (Place::Settings(Section::Network), SheetScene::PublishEdit),
        (Place::View(View::Verify, crate::nav::tab::VERIFY_WORK), SheetScene::ExtractOriginals),
        (data, SheetScene::DataFolder),
        (keys, SheetScene::Consent),
        (keys, SheetScene::ConsentMade),
        (Place::Settings(Section::Cli), SheetScene::CliPass),
        (Place::Settings(Section::Cli), SheetScene::CliPassMade),
        (persona, SheetScene::PersonaName),
        (persona, SheetScene::PersonaAccount),
        (persona, SheetScene::PersonaKeyHere),
        (persona, SheetScene::PersonaKeyOther),
        (persona, SheetScene::PersonaKeyBad),
        (keys, SheetScene::ConsentBad),
        (persona, SheetScene::PersonaRelease),
        (persona, SheetScene::PersonaReleaseLong),
        (persona, SheetScene::PersonaHome),
        (keys, SheetScene::IdPrimary),
        (keys, SheetScene::IdDeleteLedgers),
        (data, SheetScene::RestoreNewPin),
    ];
    match role {
        Role::Author => sheets.extend([
            (Place::Home, SheetScene::NewRecord),
            (Place::Home, SheetScene::NewRecordConfirm),
            (log, SheetScene::Send),
            (log, SheetScene::SendFollows),
            (log, SheetScene::NoEstimate),
            (Place::View(View::Grants, 1), SheetScene::Clash),
            (Place::View(View::Grants, 1), SheetScene::GrantConfirm),
            (log, SheetScene::Annotate),
            (log, SheetScene::AuditCard),
            (log, SheetScene::Upgrade),
            (keys, SheetScene::RotateOwed),
            (keys, SheetScene::RotateBookless),
            (keys, SheetScene::RotateLocal),
            (keys, SheetScene::RotateGiven),
            (keys, SheetScene::RotateConfirm),
            (keys, SheetScene::RotateGivenBad),
            (keys, SheetScene::RotateDeclared),
            (keys, SheetScene::RotateTakenUp),
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
            (Place::View(View::MyGrants, 0), SheetScene::HeldUpstreamSaved),
            (Place::View(View::Verify, crate::nav::tab::VERIFY_OTHERS), SheetScene::Snapshot),
        ]),
    }
    out.extend(sheets.into_iter().map(|(p, s)| Scene::Sheet(p, s)));
    out.extend([GateScene::Unlock, GateScene::RecoverWords, GateScene::RecoverFile, GateScene::RecoverNone, GateScene::LockedOut, GateScene::RestoreLocked, GateScene::RestoreNewPin, GateScene::Damaged, GateScene::Aside].map(Scene::Gate));
    out.push(Scene::IdMenu);
    let check = Place::View(View::Verify, crate::nav::tab::VERIFY_CHECK);
    out.extend([Scene::Data(check, DataScene::CheckNoNode), Scene::Data(check, DataScene::CheckSourceOpen), Scene::Data(check, DataScene::CheckBadRegistry)]);
    out.extend([
        Scene::Data(Place::View(View::Verify, crate::nav::tab::VERIFY_WORK), DataScene::VerifyKit),
        Scene::Data(Place::View(View::Verify, crate::nav::tab::VERIFY_WORK), DataScene::VerifyGrant),
        Scene::Data(Place::View(View::Verify, crate::nav::tab::VERIFY_OTHERS), DataScene::OthersRead),
        Scene::Data(Place::View(View::Verify, crate::nav::tab::VERIFY_OTHERS), DataScene::OthersList),
        Scene::Data(Place::View(View::Verify, crate::nav::tab::VERIFY_OTHERS), DataScene::OtherEntry),
        Scene::Data(Place::Settings(Section::Keys), DataScene::KeysDetails),
        Scene::Data(Place::View(View::Verify, crate::nav::tab::VERIFY_OTHERS), DataScene::OthersBadAuthor),
    ]);
    if role == Role::Author {
        out.extend([Scene::Data(Place::Home, DataScene::NewRecordBatch)]);
        out.push(Scene::Data(Place::View(View::Grants, 0), DataScene::GrantDetail));
        out.push(Scene::Data(Place::View(View::Grants, crate::nav::tab::GRANTS_NEW), DataScene::GrantBadGrantee));
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

pub fn scene_named(name: &str) -> Option<Scene> {
    let all = [crate::roles::Role::Author, crate::roles::Role::Grantee].into_iter().flat_map(scenes);
    all.into_iter().find(|s| s.name() == name || matches!(s, Scene::Sheet(_, x) if format!("{x:?}") == name))
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoWidget {
    pub order: String,
    pub rect: egui::Rect,
    pub click: bool,
    pub drag: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Geometry {
    pub widgets: Vec<GeoWidget>,
    pub texts: Vec<(String, egui::Rect)>,
    pub text_clips: Vec<egui::Rect>,
    pub strays: Vec<egui::Rect>,
    pub cards: Vec<(String, egui::Rect)>,
    pub rows: Vec<(String, egui::Rect)>,
    pub lits: Vec<zikaron_ui::probe::Lit>,
    pub cut: Vec<(String, egui::Rect, egui::Rect)>,
    pub sheets: Vec<zikaron_ui::probe::Sheet>,
    pub grey_keys: Vec<(bool, String)>,
    pub cut_short: Vec<(bool, String)>,
    pub whole_on_hover: Vec<String>,
}

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
    let kept = (win.shell.vault.clone(), win.shell.anchor.clone(), win.shell.rooted, win.shell.machine.backup.clone(), win.shell.primary.clone(), win.shell.backup_peek.clone(), win.shell.gas, win.shell.vault_recoverable);
    let kept_data = (win.shell.checked.clone(), win.shell.held.clone(), win.shell.verified.clone(), win.shell.book.clone(), win.shell.diligence.clone());
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
    let until = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        while !win.shell.tasks.flying().is_empty() && std::time::Instant::now() < until {
            win.shell.drain();
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let mut quiet = true;
        for _ in 0..6 {
            t += 0.1;
            let before = win.shell.tasks.flying().is_empty();
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
            out = Some(ctx.run_ui(input, |ui| win.draw(ui)));
            quiet &= before && win.shell.tasks.flying().is_empty();
        }
        if quiet || std::time::Instant::now() >= until {
            break;
        }
    }
    let out = out.expect("frames ran");
    let sheets = zikaron_ui::probe::sheets(ctx);
    let on_sheet = |v: Vec<(egui::LayerId, String)>| -> Vec<(bool, String)> {
        let mut out: Vec<(bool, String)> = v.into_iter().map(|(l, t)| (l.order == egui::Order::Foreground, t)).collect();
        out.sort();
        out.dedup();
        out
    };
    let (grey_keys, cut_short) = (on_sheet(zikaron_ui::probe::grey_keys(ctx)), on_sheet(zikaron_ui::probe::cuts_short(ctx)));
    let hovered = zikaron_ui::probe::wholes_on_hover(ctx);
    let mut whole_on_hover: Vec<String> = cut_short.iter().map(|(_, t)| t.clone()).filter(|t| hovered.contains(t)).collect();
    whole_on_hover.sort();
    whole_on_hover.dedup();
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
    let mut text_clips = Vec::new();
    let mut cut = Vec::new();
    fn walk(s: &egui::Shape, clip: egui::Rect, out: &mut Vec<(String, egui::Rect)>, clips: &mut Vec<egui::Rect>, cut: &mut Vec<(String, egui::Rect, egui::Rect)>) {
        match s {
            egui::Shape::Text(x) => {
                let r = x.galley.rect.translate(x.pos.to_vec2());
                if r.width() > 0.0 && (r.left() < clip.left() - 0.5 || r.right() > clip.right() + 0.5) && r.intersects(clip) {
                    cut.push((x.galley.job.text.clone(), r, clip));
                }
                out.push((x.galley.job.text.clone(), r));
                clips.push(clip);
            }
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, clip, out, clips, cut)),
            _ => {}
        }
    }
    for c in &out.shapes {
        walk(&c.shape, c.clip_rect, &mut texts, &mut text_clips, &mut cut);
    }
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
    (shell.checked, shell.held, shell.verified, shell.book, shell.diligence) = kept_data;
    let cards = boxes.iter().map(|(l, r)| (format!("{:?}", l.order), *r)).collect();
    let rows = parts.iter().map(|(l, r)| (format!("{:?}", l.order), *r)).collect();
    (shell, Geometry { widgets, texts, text_clips, strays, cards, rows, lits, cut, sheets, grey_keys, cut_short, whole_on_hover })
}

fn stage_data(win: &mut Win, d: DataScene) {
    match d {
        DataScene::CheckNoNode | DataScene::CheckSourceOpen => {
            let rows = win.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
            let grant = rows.iter().find(|r| r.kind == zikaron_glue::book::Kind::Grant).map(|r| r.id.clone()).unwrap_or_default();
            let (bytes, ledger) = match win.shell.home.as_ref() {
                Some(h) => (crate::ledgerx::detail(h, &grant).map(|x| x.bytes).unwrap_or_default(), h.dir(crate::home::Slot::Ledger).display().to_string()),
                None => (Vec::new(), String::new()),
            };
            win.typed.ck_typed = zikaron_kit::v2::doc::badge_encode(&[bytes]).unwrap_or_default();
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
            let rows = win.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
            let works: Vec<&crate::ledgerx::Row> = rows.iter().filter(|r| r.kind == zikaron_glue::book::Kind::History).collect();
            let at = win.shell.home.as_ref().map(|h| h.dir(crate::home::Slot::Kits).join("kit-geometry")).unwrap_or_default();
            win.typed.vf_path = at.display().to_string();
            let n = works.len();
            let take = works
                .iter()
                .map(|r| {
                    let name = r.facts.note.clone().filter(|x| !x.trim().is_empty()).unwrap_or_else(|| r.id.clone());
                    let content = r.work.clone().unwrap_or_default();
                    zikaron_glue::extract::Original { content: content.clone(), kept: zikaron_glue::extract::Kept::File, name: name.clone(), package_name: name.clone(), members: vec![zikaron_glue::extract::Member { path: name, rel: Vec::new(), sha: content }], names_hold: true, strict: true }
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
                    deleted: None,
                    notes: Vec::new(),
                })
                .collect();
            win.shell.verified = Some(crate::verifyx::Verified {
                path: win.typed.vf_path.clone(),
                source: crate::verifyx::Source::Kit(at),
                kit: Some(crate::verifyx::KitFacts { ok: true, entries: rows.len(), files: n, proofs: n, kit_id: works.first().map(|r| r.id.clone()).unwrap_or_default(), verdict: String::new(), subject: String::new(), invalid: Vec::new() }),
                entries: rows.len(),
                rejected: Vec::new(),
                review: Ok(crate::verifyx::AnchorReview { anchors: n, asked: n, unanchored: Vec::new(), anchored: n, unread: Vec::new() }),
                work: String::new(),
                work_found: None,
                mismatches: Vec::new(),
                records,
                takes: Some(Ok(zikaron_glue::extract::Plan { take, left: Vec::new() })),
                not_added: None,
                missed: Vec::new(),
                unanswered: Vec::new(),
                read: vec![31337],
                basis: vec![crate::verifyx::BasisRead { chain: 31337, registries: vec![format!("0x{}", "5f".repeat(20))], from_block: 0, to_block: 64 }],
                grant: None,
                filed: None,
            });
        }
        DataScene::VerifyGrant => {
            stage_data(win, DataScene::VerifyKit);
            if let Some(v) = win.shell.verified.as_mut() {
                v.grant = Some(crate::verifyx::GrantRead { package: Ok(()), verdict: Some(zikaron_kit::tokens::CheckVerdict::Green.as_str().to_string()), failed: Vec::new(), delivers: true, behind: false, lights: Vec::new(), notes: Default::default(), undecided: false });
            }
        }
        DataScene::OthersRead => stage_others(win),
        DataScene::OthersBadAuthor => {
            win.typed.rd_address = BAD_CELL.into();
            win.typed.dg_address = BAD_CELL.into();
        }
        DataScene::GrantBadGrantee => win.typed.g_grantee = BAD_CELL.into(),
        DataScene::CheckBadRegistry => win.typed.ck_registry = BAD_CELL.into(),
        DataScene::OthersList => {
            stage_others(win);
            win.ux.u3.others_tab = 1;
        }
        DataScene::OtherEntry => {
            stage_others(win);
            let seq = win.shell.rows.as_ref().and_then(|(r, _)| r.iter().find(|x| x.kind == zikaron_glue::book::Kind::History).map(|x| x.seq)).unwrap_or(0);
            win.push(Route::Other(seq), 0.0);
        }
        DataScene::NewRecordBatch => {
            win.u3_new_anchor_open();
            win.typed.batch_files = vec!["/geometry/one.txt".into(), "/geometry/two.txt".into()];
        }
        DataScene::GrantDetail => {
            let grant = win.shell.grants.as_ref().and_then(|g| g.first().map(|r| r.id.clone())).unwrap_or_default();
            win.push(crate::nav::Route::Grant(grant), 0.0);
        }
        DataScene::HeldListed | DataScene::HeldDetail => {
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
            use zikaron_glue::book::Kind as E;
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

fn stage_memory(ctx: &egui::Context, scene: Scene) {
    let up = matches!(scene, Scene::Data(_, DataScene::KitExport | DataScene::KitExportSealed | DataScene::KitExportSealedTo | DataScene::KitExportSealedKey));
    super::kit::kit_export_up(ctx, up);
}

fn stage_others(win: &mut Win) {
    let who = win.shell.anchor.map(|a| a.hex()).unwrap_or_default();
    let timeline = win.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
    let grants = win.shell.grants.clone().unwrap_or_default();
    let anchors = timeline.iter().filter(|r| r.lamp == crate::ledgerx::Lamp::Anchored).count();
    let label = zikaron::v2::tokens::Label::Complete.as_str().to_string();
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
            window: None,
            clash: Vec::new(),
            undecided: Vec::new(),
            successions: Vec::new(),
            lineage: Vec::new(),
            now: Some(GEO_TIME),
            latest: Some(GEO_TIME),
            from,
            files: None,
            misses: Vec::new(),
            pile: Vec::new(),
            audit_key: crate::sendx::behind::Key([0; 32]),
            missed: Vec::new(),
        });
    } else {
        win.typed.rd_address = who.clone();
        win.shell.book = Some(Done::Book { who, anchors, asked: anchors, entries, label, timeline, grants, latest: Some(GEO_TIME), from, files: None, misses: Vec::new(), missed: Vec::new(), face: crate::facex::State::Unread, moves: Vec::new(), pile: Vec::new(), audit_key: crate::sendx::behind::Key([0; 32]) });
    }
}

const GEO_TIME: u64 = 1_760_500_000;

pub(super) fn stage_scene(win: &mut Win, scene: Scene) {
    stage(win, scene);
}

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
            let history = rows.iter().find(|r| r.kind == zikaron_glue::book::Kind::History).map(|r| r.id.clone()).unwrap_or_default();
            let grant = win.shell.grants.as_ref().and_then(|g| g.first().map(|r| r.id.clone())).unwrap_or_default();
            match s {
                SheetScene::BackupExport => win.bk_open(Bk::Export),
                SheetScene::RestoreFirstRun => win.bk_open(Bk::Restore(RestoreFrom::FirstRun)),
                SheetScene::RestoreSettings => win.bk_open(Bk::Restore(RestoreFrom::Settings)),
                SheetScene::Mirror => win.bk_open(Bk::Own(OwnBk::Mirror)),
                SheetScene::Clean => {
                    use zikaron_seal::exports::{Item, Kind, Plan, Why};
                    let at = |n: &str| crate::home::machine_dir().map(|m| m.join(n).display().to_string()).unwrap_or_default();
                    let remove = vec![
                        Item { kind: Kind::Kit, path: at("kit-2026-10-28.zkit"), link: Some("https://example.org/kit-2026-10-28.zkit".into()) },
                        Item { kind: Kind::GrantPackage, path: at("grant-0a1b2c3d4e.zkgrant"), link: None },
                    ];
                    let keep = vec![(Item { kind: Kind::Mirror, path: at("ledger-mirror"), link: None }, Why::Changed)];
                    win.shell.cleaned = Some((Plan { remove, keep }, false));
                    win.ux.clean_sheet = true;
                }
                SheetScene::AuditCard => win.ux.u3.audit_open = true,
                SheetScene::Upgrade => {
                    if win.shell.upgrade.is_none() {
                        win.shell.upgrade = Some(crate::upgradex::Stage::Anchor { owed: 2 });
                    }
                    win.upgrade_sheet_open();
                    let stage = win.shell.upgrade.clone().unwrap_or(crate::upgradex::Stage::Nothing);
                    win.shell.upgrade_read = true;
                    win.ux.upgrade_cost_for = Some(stage.clone());
                    win.shell.upgrade_cost = Some((stage, 3, 1_260_000_000_000_000));
                }
                SheetScene::Rotate => {
                    let next = [0x5a; 20];
                    if let Some(reg) = win.shell.identities.as_mut() {
                        if let Some((id, seat)) = reg.current.clone() {
                            if let Some(row) = reg.rows.iter_mut().find(|r| r.id == id) {
                                let now = row.office(seat);
                                let o = now.map(|o| crate::identity::Office { next: Some(next), ..o });
                                match seat {
                                    crate::roles::Role::Author => row.author_office = o,
                                    crate::roles::Role::Grantee => row.grantee_office = o,
                                }
                            }
                        }
                    }
                    let chain = win.shell.settings.chain_id.unwrap_or(1);
                    let root = win.shell.anchor.map(|a| a.0).unwrap_or([0; 20]);
                    let of_root = crate::action::ledger_root(&win.shell).map(|a| a.0).unwrap_or(root);
                    win.shell.rotation = Some(vec![crate::rotatex::Chain { chain, book: crate::rotatex::Book::Read { pending: next, current: root, root: of_root, own: next } }]);
                    win.shell.queue.items.clear();
                    win.shell.audit = None;
                    win.ux.rotate_open = true;
                }
                SheetScene::RotateOwed => {
                    { let b = held_book(&win.shell); probe_rotation(win, &["0x01", "0x02"], b); }
                }
                SheetScene::RotateBookless => {
                    probe_rotation(win, &[], crate::rotatex::Book::Off);
                }
                SheetScene::RotateLocal => {
                    { let b = held_book(&win.shell); probe_rotation(win, &[], b); }
                    if let Some(reg) = win.shell.identities.as_mut() {
                        if let Some((id, seat)) = reg.current.clone() {
                            if let Some(row) = reg.rows.iter_mut().find(|r| r.id == id) {
                                if let Some(a) = row.address(seat) {
                                    row.keys = crate::identity::Keys::Both { author: a, grantee: crate::key::Address([0x5b; 20]) };
                                }
                            }
                        }
                    }
                }
                SheetScene::RotateGivenBad => {
                    if let Some(reg) = win.shell.identities.as_mut() {
                        if let Some((id, seat)) = reg.current.clone() {
                            if let Some(row) = reg.rows.iter_mut().find(|r| r.id == id) {
                                let o = row.office(seat).map(|o| crate::identity::Office { next: None, ..o });
                                match seat {
                                    crate::roles::Role::Author => row.author_office = o,
                                    crate::roles::Role::Grantee => row.grantee_office = o,
                                }
                            }
                        }
                    }
                    win.shell.rot_given = None;
                    { let b = held_book(&win.shell); probe_rotation(win, &[], b); }
                    win.ux.rot_to_given = true;
                    win.ux.rot_addr = BAD_CELL.into();
                }
                SheetScene::RotateDeclared | SheetScene::RotateTakenUp => {
                    if let Some(reg) = win.shell.identities.as_mut() {
                        if let Some((id, seat)) = reg.current.clone() {
                            if let Some(row) = reg.rows.iter_mut().find(|r| r.id == id) {
                                let o = row.office(seat).map(|o| crate::identity::Office { next: None, ..o });
                                match seat {
                                    crate::roles::Role::Author => row.author_office = o,
                                    crate::roles::Role::Grantee => row.grantee_office = o,
                                }
                            }
                        }
                    }
                    win.shell.rot_given = None;
                    let (current, root) = match held_book(&win.shell) {
                        crate::rotatex::Book::Read { current, root, .. } => (current, root),
                        _ => ([0; 20], [0; 20]),
                    };
                    let book = if matches!(s, SheetScene::RotateDeclared) {
                        crate::rotatex::Book::Read { pending: [0x8a; 20], current, root, own: [0x8a; 20] }
                    } else {
                        crate::rotatex::Book::Read { pending: [0; 20], current: [0x7c; 20], root, own: [0; 20] }
                    };
                    probe_rotation(win, &[], book);
                }
                SheetScene::RotateGiven | SheetScene::RotateConfirm => {
                    { let b = held_book(&win.shell); probe_rotation(win, &[], b); }
                    win.ux.rot_to_given = true;
                    win.ux.rot_addr = "0x70997970c51812dc3a010c7d01b50e0d17dc79c8".into();
                    if matches!(s, SheetScene::RotateConfirm) {
                        win.shell.rot_vetted = crate::key::Address::parse(&win.ux.rot_addr);
                    }
                }
                SheetScene::MoveChainBad => {
                    win.ux.move_open = true;
                    win.ux.move_cells = ["31338".into(), BAD_CELL.into(), "0".into(), "https://node.example".into()];
                }
                SheetScene::MoveChain => {
                    win.ux.move_open = true;
                    win.ux.move_cells = ["31338".into(), format!("0x{}", "36".repeat(20)), "0".into(), "https://node.example".into()];
                }
                SheetScene::Cli => win.cli_open(),
                SheetScene::NewRecord => win.u3_new_anchor_open(),
                SheetScene::NewRecordConfirm => {
                    win.u3_new_anchor_open();
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
                SheetScene::SendFollows => {
                    win.sheets_clear();
                    win.shell.gas = Some((1, 21_000));
                    win.ux.u3.send_after_estimate = None;
                    win.ux.u3.follow = win.shell.queue.items.last().map(|q| q.id.clone());
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
                SheetScene::Annotate => win.annotate_open(String::new()),
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
                SheetScene::IdDelete | SheetScene::IdDeleteLedgers => {
                    win.sheets_clear();
                    win.shell.primary = None;
                    win.ux.id_modal = Some(IdModal::Delete(id.clone().unwrap_or_default()));
                    if s == SheetScene::IdDeleteLedgers {
                        win.ux.id_delete_ledgers = vec![crate::roles::Role::Author, crate::roles::Role::Grantee];
                    }
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
                SheetScene::NetEditBad => {
                    win.net_edit_open();
                    win.ux.basis_registry = BAD_CELL.into();
                }
                SheetScene::ReadNetBad => {
                    win.read_net_new_open();
                    win.read_net_registry_typed(BAD_CELL);
                }
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
                SheetScene::Consent | SheetScene::ConsentMade => {
                    win.consent_open();
                    if s == SheetScene::ConsentMade {
                        win.ux.persona.consent_out = Some(format!("zikaron-consent:0x{}:0x{}:payout:0x{}", "11".repeat(20), "22".repeat(20), "ab".repeat(65)));
                    }
                }
                SheetScene::CliPass | SheetScene::CliPassMade => {
                    win.pass_open();
                    if s == SheetScene::CliPassMade {
                        win.ux.pass_out = Some(crate::secret::Secret::from(format!("{}{}", zikaron_seal::credential::PASS_PREFIX, "a7".repeat(32))));
                    }
                }
                SheetScene::PersonaName => win.persona_open(persona::Sheet::Name),
                SheetScene::PersonaAccount => win.persona_open(persona::Sheet::Account { platform: String::new(), handle: String::new() }),
                SheetScene::PersonaKeyHere | SheetScene::PersonaKeyOther => {
                    let here = win.shell.anchor.map(|a| a.hex()).unwrap_or_default();
                    let address = if s == SheetScene::PersonaKeyHere { here } else { format!("0x{}", "36".repeat(20)) };
                    win.persona_open(persona::Sheet::Key { address, role: String::new() });
                }
                SheetScene::PersonaKeyBad => win.persona_open(persona::Sheet::Key { address: BAD_CELL.into(), role: String::new() }),
                SheetScene::ConsentBad => {
                    win.consent_open();
                    win.ux.persona.consent_id = BAD_CELL.into();
                }
                SheetScene::PersonaRelease => win.persona_open(persona::Sheet::Release { subject: format!("0x{}", "44".repeat(32)), what: "github \u{b7} geometry".into() }),
                SheetScene::PersonaReleaseLong => win.persona_open(persona::Sheet::Release { subject: format!("0x{}", "45".repeat(32)), what: LONG_RELEASE.into() }),
                SheetScene::PersonaHome => win.persona_open(persona::Sheet::Home),
                SheetScene::IdPrimary => win.id_layer_open(IdModal::Primary(id.clone().unwrap_or_default())),
                SheetScene::RestoreNewPin => {
                    win.bk_open(Bk::Restore(RestoreFrom::Settings));
                    win.ux.bk_step = BkStep::NewPin;
                }
                SheetScene::Badge | SheetScene::HeldUpstream | SheetScene::HeldUpstreamSaved => {
                    win.sheets_clear();
                    let held = win.shell.held.as_ref().and_then(|h| h.first().map(|x| x.id.clone())).unwrap_or_default();
                    win.out_open(if s == SheetScene::Badge { grants::OutSheet::Badge(held) } else { grants::OutSheet::Upstream(held) });
                    if s == SheetScene::HeldUpstreamSaved {
                        win.typed.vt_dir = "/tmp/upstream".into();
                    }
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AlertsReading {
    pub count: usize,
    pub texts: Vec<String>,
}

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

pub struct Picture {
    pub prims: Vec<egui::ClippedPrimitive>,
    pub textures: std::collections::HashMap<egui::TextureId, egui::ColorImage>,
    pub ppp: f32,
}

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
    let kept_data = (win.shell.checked.clone(), win.shell.held.clone(), win.shell.verified.clone(), win.shell.book.clone(), win.shell.diligence.clone());
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
    (shell.checked, shell.held, shell.verified, shell.book, shell.diligence) = kept_data;
    (shell, Picture { prims, textures, ppp: out.pixels_per_point })
}

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

pub fn probe_more_open(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, Option<Picture>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::View(crate::nav::View::Log, 0), role));
    ux.wizard_asked = true;
    ux.lang_applied = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut textures: std::collections::HashMap<egui::TextureId, egui::ColorImage> = std::collections::HashMap::new();
    let mut clock = 0.0;
    let mut out = egui::FullOutput::default();
    for frame in 0..24 {
        clock += 0.1;
        let events = if frame == 12 {
            texts_of(&out).iter().rev().find(|d| d.text == t(Key::NavMore)).map(|d| d.rect.center()).map_or(Vec::new(), |at| {
                let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
                vec![egui::Event::PointerMoved(at), press(true), press(false)]
            })
        } else {
            Vec::new()
        };
        out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() }, |ui| win.draw(ui));
        take_textures(&mut textures, &out);
    }
    let ppp = out.pixels_per_point;
    let prims = ctx.tessellate(out.shapes, ppp);
    (win.shell, Some(Picture { prims, textures, ppp }))
}

fn held_book(shell: &crate::shell::Shell) -> crate::rotatex::Book {
    let office = shell.anchor.map(|a| a.0).unwrap_or([0; 20]);
    crate::rotatex::Book::Read { pending: [0; 20], current: office, root: crate::action::ledger_root(shell).map(|a| a.0).unwrap_or(office), own: [0; 20] }
}

fn probe_rotation(win: &mut Win, unanchored: &[&str], book: crate::rotatex::Book) {
    use zikaron::json::Value;
    let chain = win.shell.settings.chain_id.unwrap_or(1);
    win.shell.settings.chain_id = Some(chain);
    win.shell.settings.registry = win.shell.settings.registry.or(Some(crate::key::Address([0x11; 20])));
    win.shell.settings.earlier.clear();
    win.shell.queue.items.clear();
    let report = Value::Obj(vec![("unanchored".to_string(), Value::Arr(unanchored.iter().map(|x| Value::Str(x.to_string())).collect()))]);
    win.shell.audit = Some(crate::shell::AuditRead {
        complete: true,
        label: zikaron::v2::tokens::Label::Complete.as_str().to_string(),
        broken: false,
        entries: 0,
        report,
        unanswered: Vec::new(), unproven: Vec::new(),
        asked: 1,
        single_source: true,
        fragment: Value::Obj(Vec::new()),
        at: 0.0,
        mark: win.shell.book_mark,
        seen: None,
        rows_gen: win.shell.rows_gen,
        nodes: win.shell.endpoints.clone(),
    });
    win.shell.rotation = Some(vec![crate::rotatex::Chain { chain, book }]);
    win.ux.rotate_open = true;
}

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
    win.annotate_open(entry.to_string());
    win.typed.annotate_note = "one".to_string();
    reading.wrote_first = press_on(&mut win, &mut run, t(Key::U3AnnotateGo));
    let _ = settle(&mut win, &mut run);
    reading.after_write = win.typed.annotate_subject.clone();
    reading.closed = win.ux.u3.form.is_none();
    let _ = press_on(&mut win, &mut run, t(Key::NavMore)) && press_on(&mut win, &mut run, t(Key::U3AnnotateGo));
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

pub fn probe_wizard_backup_after_genesis(ctx: &egui::Context, shell: Shell, place: Place, width: f32, height: f32) -> (Shell, Vec<String>, Option<(usize, usize, usize)>, Option<(usize, usize)>) {
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
        (t, ctx.run_ui(input, |ui| win.draw(ui)))
    };
    let settle = |win: &mut Win, run: &mut dyn FnMut(&mut Win) -> (f64, egui::FullOutput)| {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut out = run(win);
        for _ in 0..3 {
            out = run(win);
        }
        while win.shell.tasks.in_flight(crate::task::Kind::Archive) && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(10));
            out = run(win);
        }
        for _ in 0..3 {
            out = run(win);
        }
        out
    };
    win.wizard_open(crate::nav::Step::Genesis);
    if win.shell.archive.is_none() {
        let _ = crate::action::apply(&mut win.shell, Action::Measure);
    }
    let _ = settle(&mut win, &mut run);
    let before = win.shell.archive.as_ref().and_then(|a| a.backed);
    let now = run(&mut win).0;
    win.act(Action::Genesis { statement: win.ux.wiz_statement.clone() }, now);
    let _ = settle(&mut win, &mut run);
    win.ux.wizard = Some(crate::nav::Step::Backup);
    let (_, out) = settle(&mut win, &mut run);
    let texts = texts_of(&out).into_iter().map(|d| d.text).collect();
    let carried = win.shell.backup_carries();
    (win.shell, texts, carried, before)
}

pub fn probe_clean_sheet_open(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, &'static str, usize, Vec<String>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win| {
        t += 0.05;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        (t, ctx.run_ui(input, |ui| win.draw(ui)))
    };
    let (now, _) = run(&mut win);
    win.shell.cleaned = None;
    win.ux.landed.remove(&crate::task::Kind::Clean);
    win.ux.clean_sheet = true;
    win.act(Action::CleanExports { yes: String::new() }, now);
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let _ = run(&mut win);
    while win.shell.cleaned.is_none() && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(5));
        let _ = run(&mut win);
    }
    let (_, out) = run(&mut win);
    let phase = match win.phase_of(crate::task::Kind::Clean) {
        Phase::Idle => "idle",
        Phase::Busy { .. } => "busy",
        Phase::Done { .. } => "done",
        Phase::Fail { .. } => "fail",
    };
    let listed = win.shell.cleaned.as_ref().map(|(p, _)| p.remove.len()).unwrap_or(0);
    let texts = texts_of(&out).into_iter().map(|d| d.text).collect();
    (win.shell, phase, listed, texts)
}

pub fn probe_rotate_return(shell: Shell, stage: &crate::rotatex::Stage, walked: bool, put_away: bool) -> (Shell, bool) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    ux.rot_walked = walked;
    ux.rotate_put_away = put_away;
    let win = fresh_win(shell, Typed::default(), ux);
    let back = win.rotate_comes_back(Some(stage));
    (win.shell, back)
}

pub fn probe_rotate_ways(ctx: &egui::Context, shell: Shell, words: bool, width: f32, height: f32) -> (Shell, Vec<String>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    stage(&mut win, Scene::Sheet(Place::Home, if words { SheetScene::RotateLocal } else { SheetScene::RotateOwed }));
    if !words {
        { let b = held_book(&win.shell); probe_rotation(&mut win, &[], b); }
    }
    win.ux.rot_walked = true;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut out = None;
    for _ in 0..6 {
        t += 0.2;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        out = Some(ctx.run_ui(input, |ui| win.draw(ui)));
    }
    let texts = out.map(|o| texts_of(&o).into_iter().map(|d| d.text).collect()).unwrap_or_default();
    (win.shell, texts)
}

pub fn probe_rotate_sheet(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, Vec<String>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Settings(crate::nav::Section::Keys), role));
    ux.wizard_asked = true;
    ux.rotate_open = true;
    ux.rot_walked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut out = None;
    for _ in 0..6 {
        t += 0.2;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        out = Some(ctx.run_ui(input, |ui| win.draw(ui)));
    }
    let texts = out.map(|o| texts_of(&o).into_iter().map(|d| d.text).collect()).unwrap_or_default();
    (win.shell, texts)
}

#[derive(Clone, Debug, Default)]
pub struct RotateWait {
    pub before: String,
    pub after: String,
    pub read_by_itself: bool,
    pub texts: Vec<String>,
}

pub fn probe_rotate_wait(ctx: &egui::Context, shell: Shell, wait: f64, width: f32, height: f32) -> (Shell, RotateWait) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Settings(crate::nav::Section::Keys), role));
    ux.wizard_asked = true;
    ux.rotate_open = true;
    ux.rot_walked = true;
    ux.rot_read_at = 0.2;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let frame = |win: &mut Win, t: f64| ctx.run_ui(egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() }, |ui| win.draw(ui));
    let stage = |win: &Win| crate::action::rotation_stage(&win.shell).map(|s| s.as_str().to_string()).unwrap_or_default();
    let _ = frame(&mut win, 0.2);
    let before = stage(&win);
    let flying_before = win.shell.tasks.in_flight(crate::task::Kind::Rotate);
    let mut t = 0.2 + wait;
    let _ = frame(&mut win, t);
    let read_by_itself = !flying_before && win.shell.tasks.in_flight(crate::task::Kind::Rotate);
    for _ in 0..3000 {
        if !win.shell.tasks.in_flight(crate::task::Kind::Rotate) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
        t += 0.02;
        let _ = frame(&mut win, t);
    }
    let out = frame(&mut win, t + 0.02);
    let after = stage(&win);
    let texts = texts_of(&out).into_iter().map(|d| d.text).collect();
    (win.shell, RotateWait { before, after, read_by_itself, texts })
}

#[derive(Clone, Debug, Default)]
pub struct PickWhich {
    pub asked: bool,
    pub texts: Vec<String>,
    pub opened: Vec<crate::platform::Pick>,
}

static PICKED: std::sync::Mutex<Vec<crate::platform::Pick>> = std::sync::Mutex::new(Vec::new());

fn pick_recorder(kind: crate::platform::Pick) -> Result<crate::platform::Wait, crate::fault::Fault> {
    if let Ok(mut v) = PICKED.lock() {
        v.push(kind);
    }
    Ok(Box::new(|| Ok(None)))
}

pub fn probe_pick_which(ctx: &egui::Context, shell: Shell, asks: bool, folder: bool, width: f32, height: f32) -> (Shell, PickWhich) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    win.asker = pick_recorder;
    win.asks_which = asks;
    if let Ok(mut v) = PICKED.lock() {
        v.clear();
    }
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.2;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    let _ = run(&mut win, Vec::new());
    let _ = path_answer(ctx, egui::Id::new("probe-pick-which"), true, crate::platform::Pick::FileOrFolder);
    let mut out = run(&mut win, Vec::new());
    for _ in 0..4 {
        out = run(&mut win, Vec::new());
    }
    let mut r = PickWhich { asked: win.ux.pick_which.is_some(), texts: texts_of(&out).into_iter().map(|d| d.text).collect(), opened: Vec::new() };
    if r.asked {
        let want = t(if folder { Key::PickFolder } else { Key::PickFile });
        if let Some(at) = texts_of(&out).iter().rev().find(|d| d.text == want).map(|d| d.rect.center()) {
            let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
            let _ = run(&mut win, vec![egui::Event::PointerMoved(at)]);
            let _ = run(&mut win, vec![press(true), press(false)]);
            for _ in 0..3 {
                let _ = run(&mut win, Vec::new());
            }
        }
    }
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while win.shell.tasks.in_flight(crate::task::Kind::Path) && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(5));
        let _ = run(&mut win, Vec::new());
    }
    r.opened = PICKED.lock().map(|v| v.clone()).unwrap_or_default();
    (win.shell, r)
}

pub fn probe_drop_untaken(ctx: &egui::Context, shell: Shell, over_sheet: bool, paths: &[String], width: f32, height: f32) -> (Shell, DropReading) {
    let role = shell.settings.role;
    let place = Place::View(crate::nav::View::Grants, crate::nav::tab::GRANTS_NEW);
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let typed = Typed { g_terms: format!("0x{}", "55".repeat(32)), ..Default::default() };
    let mut win = fresh_win(shell, typed, ux);
    win.faults_told = usize::MAX;
    if over_sheet {
        win.ux.u3.confirm = Some(U3Confirm::Grant);
    }
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

pub fn probe_feed_drops(hovering: bool, dropped: Vec<std::path::PathBuf>, already: Vec<std::path::PathBuf>) -> (usize, Vec<std::path::PathBuf>) {
    let mut raw = egui::RawInput { dropped_files: already.into_iter().map(|p| egui::DroppedFile { path: Some(p), ..Default::default() }).collect(), ..Default::default() };
    super::feed_drops(&mut raw, hovering, dropped);
    (raw.hovered_files.len(), raw.dropped_files.into_iter().filter_map(|f| f.path).collect())
}

pub fn probe_grant_signed(ctx: &egui::Context, shell: Shell, grantee: &str, work: &str, terms: &str, width: f32, height: f32) -> (Shell, String, bool) {
    let role = shell.settings.role;
    let place = Place::View(crate::nav::View::Grants, crate::nav::tab::GRANTS_NEW);
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let typed = Typed { g_grantee: grantee.into(), g_work: work.into(), g_terms: terms.into(), ..Default::default() };
    let mut win = fresh_win(shell, typed, ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let input = egui::RawInput { screen_rect: Some(screen), time: Some(0.1), ..Default::default() };
    let _ = ctx.run_ui(input, |ui| win.draw(ui));
    let got = win.grant_go(0.2);
    if matches!(got, Applied::Started(crate::task::Kind::Gate)) {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while win.shell.tasks.in_flight(crate::task::Kind::Gate) && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let seen = win.shell.home.as_ref().and_then(|h| crate::grantx::table(h).ok()).map(|g| g.len()).unwrap_or(0);
        let mut t = 0.3;
        for _ in 0..20 {
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
            let _ = ctx.run_ui(input, |ui| win.draw(ui));
            win.land(ctx, t);
            t += 0.1;
            if !win.ux.u4.grant_in_gate {
                break;
            }
        }
        let signed = win.shell.home.as_ref().and_then(|h| crate::grantx::table(h).ok()).map(|g| g.len()).unwrap_or(0) > seen;
        if signed && !win.ux.u4.grant_in_gate {
            let again = !win.typed.g_grantee.trim().is_empty() && !win.typed.g_work.trim().is_empty() && !win.typed.g_terms.trim().is_empty();
            return (win.shell, "granted".to_string(), again);
        }
    }
    let word = match &got {
        Applied::Granted { .. } => "granted".to_string(),
        Applied::Trouble(f) => f.which().map(|k| k.as_str().to_string()).unwrap_or_default(),
        other => format!("{other:?}"),
    };
    let again = !win.typed.g_grantee.trim().is_empty() && !win.typed.g_work.trim().is_empty() && !win.typed.g_terms.trim().is_empty();
    (win.shell, word, again)
}

pub fn probe_upgrade_comes_up(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, bool) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    for _ in 0..3 {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        let _ = ctx.run_ui(input, |ui| win.draw(ui));
    }
    let up = win.ux.upgrade_open;
    (win.shell, up)
}

pub fn probe_close_with_drops(shell: Shell, w: crate::platform::drops::Watcher) -> (Shell, bool) {
    let mut win = fresh_win(shell, Typed::default(), Ux::default());
    win.drops = Some(w);
    win.drops_let_go();
    let gone = win.drops.is_none();
    (win.shell, gone)
}

pub fn probe_each_sheet_is_one(shell: Shell) -> (Shell, Vec<(&'static str, bool, bool)>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    ux.rot_walked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    let stage = crate::rotatex::Stage::Declare { chain: 31337 };
    let mut out = Vec::new();
    let sets: [(&'static str, fn(&mut Win)); 5] = [
        ("backup", |w| w.ux.bk = Some(super::backup::Bk::Export)),
        ("wizardLeave", |w| w.ux.wiz_exit_ask = true),
        ("commandLine", |w| w.ux.cli_sheet = true),
        ("takeOut", |w| w.ux.extract_sheet = true),
        ("network", |w| w.ux.net_sheet = true),
    ];
    for (name, set) in sets {
        set(&mut win);
        out.push((name, win.any_sheet(), win.rotate_comes_back(Some(&stage))));
        win.sheets_clear();
        win.ux.wiz_exit_ask = false;
    }
    (win.shell, out)
}

pub fn probe_sheet_refusal(ctx: &egui::Context, shell: Shell, annotate: bool, width: f32, height: f32) -> (Shell, Vec<String>, Option<String>) {
    let mut ux = Ux::default();
    ux.wizard_asked = true;
    let go = if annotate {
        ux.u3.form = Some(U3Form::Annotate);
        t(Key::U3AnnotateGo)
    } else {
        ux.id_modal = Some(IdModal::Name(format!("0x{}", "ee".repeat(20))));
        ux.id_label = "named".into();
        t(Key::IdDoRenameGo)
    };
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
    if let Some(at) = texts_of(&out).into_iter().find(|d| d.text == go).map(|d| d.rect.center()) {
        let click = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at), click(true)]);
        let _ = run(&mut win, vec![click(false)]);
    }
    for _ in 0..6 {
        out = run(&mut win, Vec::new());
    }
    let drawn = texts_of(&out).into_iter().map(|d| d.text).collect();
    let toast = win.toasts.showing().map(str::to_string);
    (win.shell, drawn, toast)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tie {
    Check,
    Records,
    Others,
}

pub fn probe_input_tie(ctx: &egui::Context, shell: Shell, place: Place, tie: Tie, ask: &str, width: f32, height: f32) -> (Shell, Vec<String>, Vec<String>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    match tie {
        Tie::Check => {
            win.typed.ck_ledgers = ask.to_string();
            win.ux.u3.check_for = Some(format!("{:?}", win.check_action()));
        }
        Tie::Records => {
            win.typed.vf_path = ask.to_string();
            win.ux.u4.verify_asked = Some(win.verify_input());
        }
        Tie::Others => {
            win.typed.rd_address = ask.to_string();
            win.typed.dg_address = ask.to_string();
            let a = if role == crate::roles::Role::Grantee {
                Action::Diligence { address: ask.to_string(), dir: String::new(), work: String::new(), from: String::new(), to: String::new() }
            } else {
                Action::ReadBook { address: ask.to_string(), dir: String::new() }
            };
            win.ux.u3.others_for = Some(format!("{a:?}"));
        }
    }
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut texts = |win: &mut Win| -> Vec<String> {
        let mut out = egui::FullOutput::default();
        for _ in 0..6 {
            t += 0.1;
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
            out = ctx.run_ui(input, |ui| win.draw(ui));
        }
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(s) => Some(s.galley.text().to_string()),
                _ => None,
            })
            .collect()
    };
    let before = texts(&mut win);
    match tie {
        Tie::Check => win.typed.ck_ledgers.push_str("-other"),
        Tie::Records => win.typed.vf_path.push_str("-other"),
        Tie::Others => {
            win.typed.rd_address.push('0');
            win.typed.dg_address.push('0');
        }
    }
    let after = texts(&mut win);
    (win.shell, before, after)
}

pub fn probe_input_tie_check(ctx: &egui::Context, shell: Shell, grant: &str, ledger: &str, nodes: &str, width: f32, height: f32) -> (Shell, Vec<String>, Vec<String>) {
    let asked = shell.check_asked.clone();
    let place = Place::View(crate::nav::View::Verify, crate::nav::tab::VERIFY_CHECK);
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(place, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    win.typed.ck_typed = grant.to_string();
    win.typed.ck_ledgers = ledger.to_string();
    win.typed.ck_endpoints = nodes.to_string();
    if let Some(Ok(g)) = asked.as_ref().map(|a| a.ground.clone()) {
        win.typed.ck_registry = g.registry.hex();
        win.typed.ck_from = g.from_block.to_string();
    }
    if let Some(n) = asked.as_ref().and_then(|a| a.injected) {
        win.typed.ck_now = n.to_string();
    }
    win.ux.u3.check_for = Some(format!("{:?}", win.check_action()));
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut texts = |win: &mut Win| -> Vec<String> {
        let mut out = egui::FullOutput::default();
        for _ in 0..6 {
            t += 0.1;
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
            out = ctx.run_ui(input, |ui| win.draw(ui));
        }
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(s) => Some(s.galley.text().to_string()),
                _ => None,
            })
            .collect()
    };
    let before = texts(&mut win);
    win.typed.ck_ledgers.push_str("-other");
    let after = texts(&mut win);
    (win.shell, before, after)
}

pub fn probe_settings_key(chars: &str) -> (bool, bool) {
    crate::platform::menu_bar(&super::about::bar_words());
    crate::platform::menubar::set_settings_reachable(true);
    let _ = crate::platform::menubar::asked();
    let took = crate::platform::menu_takes_key(chars);
    let asked = crate::platform::menubar::asked().contains(&crate::platform::menubar::Act::Settings);
    (took, asked)
}

pub fn probe_comma_chord(named: bool) -> bool {
    let ctx = egui::Context::default();
    let _ = named;
    let input = egui::RawInput { events: vec![egui::Event::Key { key: egui::Key::Comma, physical_key: Some(egui::Key::Comma), pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND }], ..Default::default() };
    let mut hit = false;
    let _ = ctx.run_ui(input, |ui| hit = super::chrome::chord(ui.ctx(), egui::Key::Comma));
    hit
}

pub fn probe_files_follow_the_table(shell: Shell) -> ((Vec<String>, bool), (Vec<String>, bool)) {
    let mut win = super::faces::fresh_win(shell, Typed::default(), Ux::default());
    win.ux.files_of.insert("ab".into(), (0, vec!["old.txt".into()]));
    win.shell.rows_gen = 1;
    let shown = win.files_of("ab", 0.0);
    let asked = win.shell.tasks.in_flight(crate::task::Kind::Peek) || win.ux.files_of.get("ab").is_some_and(|(at, _)| *at == 1);
    let stale = (shown, asked);
    let mut win = super::faces::fresh_win(Shell::boot(zikaron_ui::skin::dress(&egui::Context::default())), Typed::default(), Ux::default());
    win.ux.files_of.insert("ab".into(), (win.shell.rows_gen, vec!["new.txt".into()]));
    let current = (win.files_of("ab", 0.0), win.shell.tasks.in_flight(crate::task::Kind::Peek));
    (stale, current)
}

pub fn probe_menu_bar(lang: crate::lang::Lang, live: bool) -> Vec<(String, Vec<(String, String, bool)>)> {
    crate::lang::set(lang);
    crate::platform::menu_bar(&super::about::bar_words());
    crate::platform::menubar::set_settings_reachable(live);
    crate::platform::menu_bar_read()
}

pub fn probe_pin_moves_on(ctx: &egui::Context, shell: Shell, old: &str, new: &str, width: f32, height: f32) -> (Shell, (bool, bool)) {
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
    win.id_layer_open(IdModal::Pin);
    for _ in 0..6 {
        let _ = run(&mut win, Vec::new());
    }
    for ch in old.chars() {
        let _ = run(&mut win, vec![egui::Event::Text(ch.to_string())]);
    }
    for _ in 0..3 {
        let _ = run(&mut win, Vec::new());
    }
    let at_new = ctx.data(|d| d.get_temp::<bool>(egui::Id::new("zikaron-id-pin-past-old"))).unwrap_or(false) && win.ux.pin_again.is_empty();
    for ch in new.chars() {
        let _ = run(&mut win, vec![egui::Event::Text(ch.to_string())]);
    }
    for _ in 0..3 {
        let _ = run(&mut win, Vec::new());
    }
    let at_again = !win.ux.pin_again.is_empty();
    (win.shell, (at_new, at_again))
}

pub fn probe_setting_rows_follow(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, (bool, bool)) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Settings(crate::nav::Section::Notify), role));
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
    win.ux.review_edit.edited("abc".into(), "600");
    win.ux.audit_edit.land();
    for _ in 0..2 {
        run(&mut win);
    }
    let stood = win.ux.review_edit.typed.as_deref() == Some("abc") && win.ux.audit_edit.landed;
    win.shell.settings.role = role.other();
    for _ in 0..2 {
        run(&mut win);
    }
    let gone = win.ux.review_edit == zikaron_ui::fields::SavedEdit::default() && win.ux.audit_edit == zikaron_ui::fields::SavedEdit::default();
    (win.shell, (stood, gone))
}

#[derive(Debug)]
pub struct KitExportRun {
    pub before: Vec<String>,
    pub pressed: (bool, bool),
    pub after: Vec<String>,
    pub done: Option<crate::task::Done>,
}

pub fn probe_kit_export(ctx: &egui::Context, shell: Shell, seconds: f64, width: f32, height: f32) -> (Shell, KitExportRun) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::View(crate::nav::View::Works, crate::nav::tab::WORKS_KIT), role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let began = std::time::Instant::now();
    let mut clock = 0.0;
    let mut frame = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    let busy = |win: &Win| !win.shell.tasks.flying().is_empty();
    let mut out = frame(&mut win, Vec::new());
    while (win.ux.u3.kit_orig.is_none() || busy(&win)) && began.elapsed().as_secs_f64() < seconds {
        std::thread::sleep(std::time::Duration::from_millis(20));
        out = frame(&mut win, Vec::new());
    }
    for _ in 0..4 {
        out = frame(&mut win, Vec::new());
    }
    let before: Vec<String> = texts_of(&out).into_iter().map(|d| d.text).collect();
    let mut press = |win: &mut Win, out: &egui::FullOutput, label: &str| -> (bool, egui::FullOutput) {
        let Some(at) = texts_of(out).iter().find(|d| d.text == label).map(|d| d.rect.center()) else { return (false, frame(win, Vec::new())) };
        let click = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = frame(win, vec![egui::Event::PointerMoved(at), click(true), click(false)]);
        let mut o = frame(win, vec![egui::Event::PointerGone]);
        for _ in 0..4 {
            o = frame(win, Vec::new());
        }
        (true, o)
    };
    let (page, out) = press(&mut win, &out, t(Key::U3KitKey));
    let (sheet, mut out) = press(&mut win, &out, t(Key::IdExportGroup));
    while busy(&win) && began.elapsed().as_secs_f64() < seconds * 4.0 {
        std::thread::sleep(std::time::Duration::from_millis(20));
        out = frame(&mut win, Vec::new());
    }
    for _ in 0..4 {
        out = frame(&mut win, Vec::new());
    }
    let after: Vec<String> = texts_of(&out).into_iter().map(|d| d.text).collect();
    let done = win.shell.kit.clone();
    (win.shell, KitExportRun { before, pressed: (page, sheet), after, done })
}

#[derive(Debug)]
pub struct UpgradePress {
    pub before: Vec<String>,
    pub pressed: bool,
    pub started: Vec<String>,
    pub after: Vec<String>,
}

pub fn probe_upgrade_press(ctx: &egui::Context, shell: Shell, frames: usize, width: f32, height: f32) -> (Shell, UpgradePress) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut frame = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    win.upgrade_sheet_open();
    let mut out = frame(&mut win, Vec::new());
    for _ in 0..frames {
        std::thread::sleep(std::time::Duration::from_millis(10));
        out = frame(&mut win, Vec::new());
    }
    let before: Vec<String> = texts_of(&out).into_iter().map(|d| d.text).collect();
    let at = texts_of(&out).iter().find(|d| d.text == t(Key::UpConfirmGo)).map(|d| d.rect.center());
    let mut started = Vec::new();
    if let Some(at) = at {
        let click = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        let _ = frame(&mut win, vec![egui::Event::PointerMoved(at), click(true), click(false)]);
        started = win.shell.tasks.flying().iter().map(|k| k.as_str().to_string()).collect();
        out = frame(&mut win, vec![egui::Event::PointerGone]);
        for _ in 0..4 {
            out = frame(&mut win, Vec::new());
        }
    }
    let pressed = win.ux.upgrade_auto;
    let after: Vec<String> = texts_of(&out).into_iter().map(|d| d.text).collect();
    (win.shell, UpgradePress { before, pressed, started, after })
}

pub fn probe_id_sheet_answer(shell: Shell, answer: Applied) -> (Shell, bool) {
    let mut ux = Ux::default();
    ux.wizard_asked = true;
    ux.id_open(IdModal::Primary(String::new()));
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.vault_back(VaultSite::Id, answer, 0.0);
    let open = win.ux.id_modal.is_some();
    (win.shell, open)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SetUpOffer {
    pub create: bool,
    pub fetch: bool,
}

pub fn probe_set_up_faces(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, [SetUpOffer; 3]) {
    let role = shell.settings.role;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut shell = shell;
    let mut offers = [SetUpOffer::default(); 3];
    let places = [None, Some(Place::Settings(crate::nav::Section::About)), Some(Place::View(crate::nav::View::MyGrants, crate::nav::tab::HELD_RELICENSE))];
    for (i, place) in places.into_iter().enumerate() {
        let fresh = |shell: Shell| {
            let mut ux = Ux::default();
            ux.nav.land(crate::nav::history_of(place.unwrap_or(Place::Home), role));
            ux.wizard_asked = true;
            let mut win = fresh_win(shell, Typed::default(), ux);
            win.faults_told = usize::MAX;
            if place.is_none() {
                win.wizard_open(crate::nav::Step::Genesis);
            }
            win
        };
        let mut clock = 0.0;
        let mut run = |win: &mut Win, events: Vec<egui::Event>| {
            clock += 0.1;
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), events, ..Default::default() };
            ctx.run_ui(input, |ui| win.draw(ui))
        };
        let mut win = fresh(shell);
        let mut out = run(&mut win, Vec::new());
        for _ in 0..10 {
            out = run(&mut win, Vec::new());
        }
        let at = |word: &str| texts_of(&out).iter().filter(|d| d.text == word).map(|d| d.rect.center()).collect::<Vec<_>>();
        let (creates, fetches) = (at(t(Key::WizGenesisTitle)), at(t(Key::DoFetchLedger)));
        shell = win.shell;
        for (spots, fetch) in [(creates, false), (fetches, true)] {
            for spot in spots {
                let mut win = fresh(shell);
                for _ in 0..10 {
                    let _ = run(&mut win, Vec::new());
                }
                let press = |pressed| egui::Event::PointerButton { pos: spot, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
                let _ = run(&mut win, vec![egui::Event::PointerMoved(spot), press(true), press(false)]);
                for _ in 0..3 {
                    let _ = run(&mut win, Vec::new());
                }
                if fetch {
                    let (stack, h) = crate::nav::history_of(Place::Settings(crate::nav::Section::Data), role);
                    offers[i].fetch |= win.ux.nav.stack == stack && win.ux.nav.hist.get(&stack) == Some(&h) && win.ux.wizard.is_none();
                } else {
                    offers[i].create |= win.ux.confirm_genesis || (place.is_some() && win.ux.wizard == Some(crate::nav::Step::Genesis));
                }
                shell = win.shell;
            }
        }
    }
    (shell, offers)
}

#[allow(clippy::type_complexity)]
pub fn probe_fault_toast(ctx: &egui::Context, shell: Shell, fault: crate::fault::Fault, width: f32, height: f32) -> (Shell, Option<(String, String, Vec<(String, String)>)>) {
    let mut win = fresh_win(shell, Typed::default(), Ux::default());
    win.faults_told = win.shell.faults.len();
    win.shell.faults.push(fault);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    for i in 0..2 {
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(0.1 * (i + 1) as f64), ..Default::default() };
        let _ = ctx.run_ui(input, |ui| win.draw(ui));
    }
    let said = win.toasts.showing().map(|text| {
        let keys = win
            .toasts
            .showing_keys()
            .into_iter()
            .map(|(label, act)| match act {
                zikaron_ui::toast::Act::Copy(what) => (label, what),
                zikaron_ui::toast::Act::Tag(_) => (label, String::new()),
            })
            .collect();
        (text.to_string(), win.toasts.showing_next().unwrap_or_default().to_string(), keys)
    });
    (win.shell, said)
}

pub fn probe_rotate_sheet_opened(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, Vec<String>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Settings(crate::nav::Section::Keys), role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    win.rotate_sheet_open(0.0);
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut out = None;
    for _ in 0..6 {
        t += 0.2;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        out = Some(ctx.run_ui(input, |ui| win.draw(ui)));
    }
    let texts = out.map(|o| texts_of(&o).into_iter().map(|d| d.text).collect()).unwrap_or_default();
    (win.shell, texts)
}

pub fn probe_rotate_new_words(ctx: &egui::Context, shell: Shell, width: f32, height: f32) -> (Shell, Vec<String>, bool) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Settings(crate::nav::Section::Keys), role));
    ux.wizard_asked = true;
    ux.rotate_open = true;
    ux.rot_walked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    win.rot_pending_read();
    win.ux.rot_new_words = true;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> egui::FullOutput {
        t += 0.2;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), events, ..Default::default() };
        ctx.run_ui(input, |ui| win.draw(ui))
    };
    let mut out = run(&mut win, Vec::new());
    for _ in 0..5 {
        out = run(&mut win, Vec::new());
    }
    let drawn = texts_of(&out);
    let texts: Vec<String> = drawn.iter().map(|d| d.text.clone()).collect();
    let next = drawn.iter().rev().find(|d| d.text == t_key(crate::lang::Key::WizardNext)).map(|d| d.rect.center());
    if let Some(at) = next {
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
        let _ = run(&mut win, vec![egui::Event::PointerMoved(at)]);
        let _ = run(&mut win, vec![press(true)]);
        let _ = run(&mut win, vec![press(false)]);
        for _ in 0..2 {
            let _ = run(&mut win, Vec::new());
        }
    }
    let went_on = win.ux.rot_words > 0 || win.shell.new_words.is_some();
    (win.shell, texts, went_on)
}

fn t_key(k: crate::lang::Key) -> String {
    crate::lang::t(k).to_string()
}

pub fn probe_upgrade_comes_up_steps(ctx: &egui::Context, shell: Shell, steps: &[(Option<crate::upgradex::Stage>, bool, bool)], width: f32, height: f32) -> (Shell, Vec<bool>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    ux.nav.land(crate::nav::history_of(Place::Home, role));
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut clock = 0.0;
    let mut open = Vec::new();
    for (stage, read, fresh) in steps {
        if *fresh {
            win.ux.upgrade_open = false;
            win.ux.upgrade_put_away = false;
        }
        win.shell.upgrade = stage.clone();
        win.shell.upgrade_read = *read;
        clock += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(clock), ..Default::default() };
        let _ = ctx.run_ui(input, |ui| win.draw(ui));
        open.push(win.ux.upgrade_open);
    }
    (win.shell, open)
}
