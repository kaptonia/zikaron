use super::*;
use zikaron_ui::covers;
use zikaron_ui::sheetparts as parts;

impl Win {
    pub(super) fn gate(&mut self, ctx: &egui::Context, now: f64) {
        let state = self.shell.vault.clone();
        let up = state.gate_up();
        covers::app_in(ctx, up);
        if !up {
            return;
        }
        if let crate::shell::Vault::Damaged(f) = &state {
            let mut ask = false;
            let unplanned = self.ux.gate_aside.is_none().then(|| self.ux.gate_aside_trouble.as_ref().map(said)).flatten();
            let _ = covers::cover(ctx, "gate", covers::Ground::Lit, |ui| {
                let rect = ui.max_rect();
                full::centered(ui, "gate", rect, tk::SHEET_W, 0.0, |ui| {
                    let around = gate_head(ui, t(Key::VaultDamagedTitle));
                    states::err_box(ui, "gate-damaged", f.human(), &f.next(), t(Key::U3RawError), &f.shown());
                    ui.add_space(around);
                    ask = ui.vertical_centered(|ui| key::key(ui, t(Key::GateAside), Role::Secondary, true).clicked()).inner;
                    if let Some(s) = unplanned.as_deref() {
                        ui.add_space(tk::S3);
                        states::note_box(ui, s);
                    }
                });
            });
            if ask {
                match crate::home::aside_plan() {
                    Ok(plan) => {
                        self.ux.gate_aside = Some(plan);
                        self.ux.gate_aside_trouble = None;
                    }
                    Err(f) => self.ux.gate_aside_trouble = Some(f),
                }
            }
            self.gate_aside_sheet(ctx, now);
            return;
        }
        let locked_out = state.is(crate::keybox::State::LockedOut);
        if locked_out {
            self.ux.gate_recover = true;
        }
        let empty_out = locked_out && !self.shell.vault_recoverable;
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault);
        let recover = self.ux.gate_recover;
        let (by_words, by_file) = match self.shell.primary.as_ref().map(|(_, k)| *k) {
            Some(crate::keybox::PrimaryKind::Words) => (true, true),
            Some(crate::keybox::PrimaryKind::KeyFile) => (false, true),
            None => (false, false),
        };
        let mut from_backup = false;
        let len = crate::keybox::PIN_LEN;
        let mut first_pin = false;
        let mut act: Option<Action> = None;
        let _ = covers::cover(ctx, "gate", covers::Ground::Lit, |ui| {
            let rect = ui.max_rect();
            full::centered(ui, "gate", rect, tk::SHEET_W, 0.0, |ui| {
                let title = if !recover {
                    Key::PinGateTitle
                } else if locked_out {
                    Key::PinLockedTitle
                } else {
                    Key::PinRecoverTitle
                };
                let around = gate_head(ui, t(title));
                if !recover {
                    let row = ui.vertical_centered(|ui| parts::pins(ui, "gate-pin", &mut self.ux.pin, len, self.ux.pin_shake, !busy)).inner;
                    if row.changed && !self.ux.gate_reseal {
                        self.ux.gate_trouble = None;
                    }
                    if row.full && !busy {
                        let pin = std::mem::take(&mut self.ux.pin);
                        act = Some(if self.ux.gate_reseal { Action::Reseal { pin } } else { Action::Unlock { pin } });
                    }
                    gate_message(ui, busy, self.ux.gate_trouble.as_deref(), around);
                    ui.add_space(around);
                    if ui.vertical_centered(|ui| covers::small_key(ui, t(Key::PinForgot), true).clicked()).inner {
                        self.ux.gate_recover = true;
                        self.ux.gate_trouble = None;
                    }
                    return;
                }
                if empty_out {
                    centred_note(ui, t(Key::PinEmptySay));
                    gate_message(ui, false, self.ux.gate_trouble.as_deref(), around);
                    ui.add_space(around);
                    if ui.vertical_centered(|ui| key::key(ui, t(Key::PinDoReset), Role::Secondary, true).clicked()).inner {
                        act = Some(Action::ResetEmptyKeybox);
                    }
                    return;
                }
                if by_words || by_file {
                    let how = t(if by_words { Key::GateByWords } else { Key::GateByFile });
                    let say = if locked_out { format!("{}{}", t(Key::GateLockedFive), how) } else { how.to_string() };
                    centred_note(ui, &say);
                    ui.add_space(tk::S3);
                }
                ui.spacing_mut().item_spacing.y = tk::S3;
                if by_words {
                    let by_file = !self.ux.pin_ks_path.trim().is_empty() || !self.ux.pin_ks_pw.is_empty();
                    let title = t(if self.ux.pin_again.is_empty() { Key::WizPinTitle } else { Key::PinAgain });
                    let again = self.ux.row_turn(now);
                    let row = words_way(ui, &mut self.ux.pin_words, &mut self.ux.gate_words_ready, &mut self.ux.pin, len, self.ux.pin_shake, title, !by_file && !busy, again);
                    if row.is_some_and(|r| r.full) {
                        if self.ux.pin_again.is_empty() {
                            first_pin = true;
                        } else if !busy {
                            act = Some(Action::RecoverWords { words: joined(&self.ux.pin_words), pin: self.ux.pin_again.clone(), again: std::mem::take(&mut self.ux.pin) });
                        }
                    }
                    gate_message(ui, busy, self.ux.gate_trouble.as_deref(), 4.0);
                }
                let by_key_file = |me: &mut Self, ui: &mut egui::Ui, first_pin: &mut bool, act: &mut Option<Action>| {
                    let title = t(if me.ux.pin_again.is_empty() { Key::WizPinTitle } else { Key::PinAgain });
                    let again = me.ux.row_turn(now);
                    let row = file_way(ui, &mut me.ux.pin_ks_path, &mut me.ux.pin_ks_pw, &mut me.ux.gate_file_ready, &mut me.ux.pin, len, me.ux.pin_shake, t(Key::GateDropKeyFile), t(Key::GateFilePassword), title, busy, again);
                    if row.full && me.ux.pin_again.is_empty() {
                        *first_pin = true;
                    } else if row.full && !busy {
                        *act = Some(Action::RecoverKeystore {
                            path: me.ux.pin_ks_path.clone(),
                            password: me.ux.pin_ks_pw.clone(),
                            pin: me.ux.pin_again.clone(),
                            again: std::mem::take(&mut me.ux.pin),
                        });
                    }
                };
                if by_file && !by_words {
                    by_key_file(self, ui, &mut first_pin, &mut act);
                    gate_message(ui, busy, self.ux.gate_trouble.as_deref(), 4.0);
                }
                if by_file && by_words {
                    fold::fold(ui, "gate-keystore", t(Key::PinByFile), |ui| by_key_file(self, ui, &mut first_pin, &mut act));
                }
                gate_keys(ui, around, |ui| {
                    if !locked_out && key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked() {
                        self.ux.gate_back();
                    }
                    from_backup = lead_key(ui, t(Key::BackupRestoreTitle), Role::Plain, !busy);
                });
            });
        });
        if from_backup {
            self.bk_open(Bk::Restore(RestoreFrom::Locked));
        }
        if first_pin {
            self.take_first_pin(now, PinSaid::Gate);
        }
        if let Some(a) = act {
            let r = self.act(a, now);
            self.vault_or(VaultSite::Gate, r, now);
        }
    }
}

impl Win {
    fn gate_aside_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let Some(plan) = self.ux.gate_aside.clone() else { return };
        let (mut go, mut cancel) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("gate-aside", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::GateAside), "");
                ui.spacing_mut().item_spacing.y = tk::S4;
                field(ui, t(Key::GateAsideMoves), None, |ui| path_words(ui, &plan.machine.display().to_string()));
                field(ui, t(Key::GateAsideKeeps), None, |ui| match &plan.kept {
                    Some(k) => path_words(ui, &k.display().to_string()),
                    None => {
                        paint::text(ui, t(Key::GateAsideKeepsNone), Type::Body, c(C::Ink3));
                    }
                });
                field(ui, t(Key::GateAsideBack), None, |ui| {
                    ui.add(egui::Label::new(egui::RichText::new(t(Key::GateAsideBackSay)).font(Type::Body.font()).color(c(C::Ink))).wrap());
                });
                if let Some(f) = me.ux.gate_aside_trouble.as_ref() {
                    states::note_box(ui, &said(f));
                }
            },
            |ui, _me| {
                go = key::key(ui, t(Key::GateAsideGo), Role::Danger, true).clicked();
                cancel = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if cancel || out.esc {
            self.ux.gate_aside = None;
            self.ux.gate_aside_trouble = None;
        } else if go {
            let r = self.act(Action::SetMachineAside, now);
            if let Applied::Trouble(f) = &r {
                self.ux.gate_aside_trouble = Some(f.clone());
            }
            self.vault_or(VaultSite::Gate, r, now);
        }
    }
}

fn said(f: &crate::fault::Fault) -> String {
    format!("{} {}", f.human(), f.next())
}

fn path_words(ui: &mut egui::Ui, path: &str) {
    ui.add(egui::Label::new(egui::RichText::new(path).font(Type::Mono.font()).color(c(C::Ink))).wrap());
}

fn gate_head(ui: &mut egui::Ui, title: &str) -> f32 {
    ui.spacing_mut().item_spacing.y = 0.0;
    let strokes_end = covers::lock_head(ui, t(Key::AppName), title);
    ui.add_space(14.0);
    ui.cursor().min.y - strokes_end
}

fn gate_keys(ui: &mut egui::Ui, gap: f32, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(gap);
    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), tk::KEY_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = tk::S2;
        add(ui)
    });
}

fn lead_key(ui: &mut egui::Ui, text: &str, role: Role, enabled: bool) -> bool {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| key::key(ui, text, role, enabled).clicked()).inner
}

fn words_way(ui: &mut egui::Ui, words: &mut [crate::secret::Secret; 12], was_full: &mut bool, pin: &mut crate::secret::Secret, len: usize, shake: Option<f64>, title: &str, row_on: bool, again: bool) -> Option<pin::PinRow> {
    ui.spacing_mut().item_spacing.y = tk::S3;
    let bad = crate::cryptx::strangers(words);
    let phrase = ui.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Paste(x) if x.split_whitespace().count() > 1)));
    let changed = pin::words_grid_marked(ui, "gate-words", words, &bad);
    let full = words.iter().all(|w| !w.expose().trim().is_empty());
    let pasted = changed && phrase && full && !*was_full;
    if pasted {
        if let Some(id) = ui.memory(|m| m.focused()) {
            ui.memory_mut(|m| m.surrender_focus(id));
        }
    }
    *was_full = full;
    let hand = again || pasted || ui.input(|i| i.key_pressed(egui::Key::Enter));
    full.then(|| field(ui, title, None, |ui| parts::pins(ui, "gate-new-pin", pin, len, shake, row_on && hand)))
}

fn file_way(ui: &mut egui::Ui, path: &mut String, password: &mut crate::secret::Secret, ready: &mut bool, pin: &mut crate::secret::Secret, len: usize, shake: Option<f64>, file_title: &str, password_title: &str, title: &str, busy: bool, again: bool) -> pin::PinRow {
    ui.spacing_mut().item_spacing.y = tk::S3;
    field(ui, file_title, None, |ui| pick_path(ui, path, crate::platform::Pick::File));
    field(ui, password_title, None, |ui| input::secret_line(ui, password, ""));
    *ready = !path.trim().is_empty() && !password.is_empty();
    let hand = again || ui.input(|i| i.key_pressed(egui::Key::Enter));
    field(ui, title, None, |ui| parts::pins(ui, "gate-file-pin", pin, len, shake, *ready && !busy && hand))
}

fn gate_message(ui: &mut egui::Ui, busy: bool, trouble: Option<&str>, gap: f32) {
    let id = ui.id().with("gate-message");
    let on = busy || trouble.is_some();
    let open = motion::flag(ui.ctx(), id, on, tk::MID);
    if open <= 0.0 {
        return;
    }
    let h_id = id.with("h");
    let full_h = ui.ctx().data(|d| d.get_temp::<f32>(h_id)).unwrap_or(40.0);
    let shown_h = (gap + full_h) * open;
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, shown_h), egui::Sense::hover());
    let inner = egui::Rect::from_min_size(egui::pos2(rect.left(), rect.top() + gap * open), egui::vec2(w, full_h));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child.multiply_opacity(open);
    let r = child.vertical(|ui| {
        if busy {
            sheet::busy_note(ui, t(Key::VaultBusy), true);
        } else if let Some(s) = trouble {
            states::note_box(ui, s);
        }
    });
    let got = r.response.rect.height();
    if on && (got - full_h).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(h_id, got));
        ui.ctx().request_discard("gate message height changed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::secret::Secret;

    const WORDS: [&str; 12] = ["abandon"; 12];
    const LAST: &str = "action";

    struct Drive {
        ctx: egui::Context,
        t: f64,
    }

    impl Drive {
        fn new() -> Drive {
            let ctx = egui::Context::default();
            let _ = zikaron_ui::skin::dress(&ctx);
            Drive { ctx, t: 0.0 }
        }

        fn frame(&mut self, events: Vec<egui::Event>, draw: &mut dyn FnMut(&mut egui::Ui)) {
            self.t += 0.05;
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(520.0, 900.0));
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(self.t), events, ..Default::default() };
            let _ = self.ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show_inside(ui, |ui| draw(ui));
            });
        }

        fn typed(&mut self, text: &str, draw: &mut dyn FnMut(&mut egui::Ui)) {
            for ch in text.chars() {
                self.frame(vec![egui::Event::Text(ch.to_string())], draw);
            }
        }
    }

    fn key(k: egui::Key) -> egui::Event {
        egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }
    }

    #[derive(Default)]
    struct Words {
        words: [Secret; 12],
        full: bool,
        pin: Secret,
    }

    fn words_draw(w: &mut Words) -> impl FnMut(&mut egui::Ui) + '_ {
        move |ui| {
            let _ = words_way(ui, &mut w.words, &mut w.full, &mut w.pin, crate::keybox::PIN_LEN, None, "passcode", true, false);
        }
    }

    fn type_twelve(d: &mut Drive, w: &mut Words) {
        for _ in 0..2 {
            d.frame(Vec::new(), &mut words_draw(w));
        }
        d.frame(vec![key(egui::Key::Tab)], &mut words_draw(w));
        for (i, word) in WORDS.iter().enumerate() {
            d.typed(if i == 11 { LAST } else { word }, &mut words_draw(w));
            if i < 11 {
                d.frame(vec![key(egui::Key::Tab)], &mut words_draw(w));
            }
        }
    }

    #[test]
    fn the_twelfth_word_typed_key_by_key_keeps_the_focus() {
        let (mut d, mut w) = (Drive::new(), Words::default());
        type_twelve(&mut d, &mut w);
        assert_eq!(w.words[11].expose(), LAST);
        assert!(w.words[..11].iter().all(|x| x.expose() == "abandon"));
        assert!(w.pin.is_empty(), "nothing reached the passcode row");
        d.typed("s", &mut words_draw(&mut w));
        assert_eq!(w.words[11].expose(), "actions");
        assert!(w.pin.is_empty());
    }

    #[test]
    fn enter_or_tab_in_the_last_cell_moves_to_the_passcode_row() {
        for k in [egui::Key::Enter, egui::Key::Tab] {
            let (mut d, mut w) = (Drive::new(), Words::default());
            type_twelve(&mut d, &mut w);
            d.frame(vec![key(k)], &mut words_draw(&mut w));
            d.typed("ab12", &mut words_draw(&mut w));
            assert_eq!(w.pin.expose(), "ab12", "{k:?}");
            assert_eq!(w.words[11].expose(), LAST, "{k:?}");
        }
    }

    #[test]
    fn a_pasted_phrase_that_fills_the_twelve_moves_to_the_passcode_row() {
        let phrase = format!("{} {LAST}", WORDS[..11].join(" "));
        let (mut d, mut w) = (Drive::new(), Words::default());
        for _ in 0..2 {
            d.frame(Vec::new(), &mut words_draw(&mut w));
        }
        d.frame(vec![key(egui::Key::Tab)], &mut words_draw(&mut w));
        d.frame(vec![egui::Event::Paste(phrase)], &mut words_draw(&mut w));
        assert_eq!(w.words[11].expose(), LAST);
        d.typed("ab12", &mut words_draw(&mut w));
        assert_eq!(w.pin.expose(), "ab12");

        let (mut d, mut w) = (Drive::new(), Words::default());
        for _ in 0..2 {
            d.frame(Vec::new(), &mut words_draw(&mut w));
        }
        d.frame(vec![key(egui::Key::Tab)], &mut words_draw(&mut w));
        d.frame(vec![egui::Event::Paste("abandon abandon".into())], &mut words_draw(&mut w));
        d.typed("x", &mut words_draw(&mut w));
        assert!(w.pin.is_empty() && w.words[0].expose() == "abandonx", "a short paste leaves focus in the cell");
    }

    #[derive(Default)]
    struct File {
        path: String,
        password: Secret,
        ready: bool,
        pin: Secret,
    }

    fn file_draw(f: &mut File) -> impl FnMut(&mut egui::Ui) + '_ {
        move |ui| {
            let _ = file_way(ui, &mut f.path, &mut f.password, &mut f.ready, &mut f.pin, crate::keybox::PIN_LEN, None, "file", "password", "passcode", false, false);
        }
    }

    #[test]
    fn the_key_file_password_typed_keeps_the_focus_until_enter_or_tab() {
        for k in [egui::Key::Enter, egui::Key::Tab] {
            let mut d = Drive::new();
            let mut f = File { path: "/somewhere/key.json".into(), ..Default::default() };
            for _ in 0..2 {
                d.frame(Vec::new(), &mut file_draw(&mut f));
            }
            for _ in 0..3 {
                d.frame(vec![key(egui::Key::Tab)], &mut file_draw(&mut f));
                d.typed("h", &mut file_draw(&mut f));
                if !f.password.is_empty() {
                    break;
                }
            }
            d.typed("unter2-long", &mut file_draw(&mut f));
            assert_eq!(f.password.expose(), "hunter2-long", "{k:?}");
            assert!(f.pin.is_empty(), "{k:?}: nothing reached the passcode row");
            d.frame(vec![key(k)], &mut file_draw(&mut f));
            d.typed("ab12", &mut file_draw(&mut f));
            assert_eq!(f.pin.expose(), "ab12", "{k:?}");
            assert_eq!(f.password.expose(), "hunter2-long", "{k:?}");
        }
    }

    #[test]
    fn a_file_chosen_after_the_password_hands_nothing_over() {
        let mut d = Drive::new();
        let mut f = File::default();
        for _ in 0..2 {
            d.frame(Vec::new(), &mut file_draw(&mut f));
        }
        for _ in 0..3 {
            d.frame(vec![key(egui::Key::Tab)], &mut file_draw(&mut f));
            d.typed("h", &mut file_draw(&mut f));
            if !f.password.is_empty() {
                break;
            }
        }
        assert_eq!(f.password.expose(), "h");
        if let Some(id) = d.ctx.memory(|m| m.focused()) {
            d.ctx.memory_mut(|m| m.surrender_focus(id));
        }
        f.path = "/somewhere/key.json".into();
        for _ in 0..3 {
            d.frame(Vec::new(), &mut file_draw(&mut f));
        }
        d.typed("ab12", &mut file_draw(&mut f));
        assert!(f.ready);
        assert!(f.pin.is_empty(), "no key reached the passcode row");
    }
}

fn centred_note(ui: &mut egui::Ui, s: &str) {
    let w = ui.available_width();
    let mut job = egui::text::LayoutJob::single_section(s.to_string(), egui::TextFormat { font_id: Type::Note.font(), color: c(C::Ink2), line_height: Some(Type::Note.line()), ..Default::default() });
    job.wrap.max_width = w;
    job.halign = egui::Align::Center;
    let g = ui.fonts_mut(|f| f.layout_job(job));
    let (r, _) = ui.allocate_exact_size(egui::vec2(w, g.size().y), egui::Sense::hover());
    ui.painter().galley(egui::pos2(r.center().x, r.top()), g, c(C::Ink2));
}
