//! The passcode gate: a full-window cover with eight passcode cells and an error line that opens under them
//! only when there is an error (no toasts on the gate). "Forgot passcode" recovers by the primary identity's
//! kind (its twelve words, or its key file and that file's password, then a new passcode twice); restoring a
//! whole-machine backup is a third way.
//!
//! The gate only opens the vault; actions that need a key are refused by the action layer either way.

use super::*;

impl Win {
    pub(super) fn gate(&mut self, ctx: &egui::Context, now: f64) {
        let state = self.shell.vault.clone();
        // `Vault::gate_up` decides whether the gate or the shell draws.
        if !state.gate_up() {
            return;
        }
        // The store file exists but cannot be read: show the store's refusal and offer nothing that would build a
        // new store over the keys it holds (no wizard, no passcode cells).
        if let crate::shell::Vault::Damaged(f) = &state {
            full::cover(ctx, "gate", |ui, drop| {
                let rect = ui.max_rect();
                full::centered(ui, "gate", rect, tk::SHEET_W, drop, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        paint::text(ui, t(Key::AppName), Type::Small, c(C::Ink3));
                        paint::text(ui, t(Key::VaultDamagedTitle), Type::Page, c(C::Ink));
                        ui.add_space(18.0);
                    });
                    states::err_box(ui, "gate-damaged", f.human(), f.next(), t(Key::U3RawError), &f.raw());
                });
            });
            return;
        }
        let locked_out = state.is(crate::keybox::State::LockedOut);
        if locked_out {
            self.ux.gate_recover = true;
        }
        // An empty vault that is locked out (passcode set, no identity) cannot be recovered and holds nothing to
        // lose, so the only way on is to reset it and run the wizard again.
        let empty_out = locked_out && !self.shell.vault_recoverable;
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault);
        let recover = self.ux.gate_recover;
        // The recovery way follows the primary identity's kind, read from the vault header before unlocking.
        // Vaults written before the primary was recorded offer both ways.
        let (by_words, by_file) = match self.shell.primary.as_ref().map(|(_, k)| *k) {
            Some(crate::keybox::PrimaryKind::Words) => (true, false),
            Some(crate::keybox::PrimaryKind::KeyFile) => (false, true),
            None => (true, true),
        };
        let mut from_backup = false;
        let len = crate::keybox::PIN_LEN;
        let mut first_pin = false;
        let mut act: Option<Action> = None;
        full::cover(ctx, "gate", |ui, drop| {
            let rect = ui.max_rect();
            full::centered(ui, "gate", rect, tk::SHEET_W, drop, |ui| {
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    paint::text(ui, t(Key::AppName), Type::Small, c(C::Ink3));
                    let title = if !recover {
                        Key::PinGateTitle
                    } else if locked_out {
                        Key::PinLockedTitle
                    } else {
                        Key::PinRecoverTitle
                    };
                    paint::text(ui, t(title), Type::Page, c(C::Ink));
                });
                if !recover {
                    ui.vertical_centered(|ui| {
                        ui.add_space(28.0);
                        let row = pin::pin_row(ui, "gate-pin", &mut self.ux.pin, len, self.ux.pin_shake, !busy, false);
                        // Typing again clears the last refusal, except on the reseal path, whose sentence says what
                        // the next passcode will do.
                        if row.changed && !self.ux.gate_reseal {
                            self.ux.gate_trouble = None;
                        }
                        if row.full && !busy {
                            let pin = std::mem::take(&mut self.ux.pin);
                            act = Some(if self.ux.gate_reseal { Action::Reseal { pin } } else { Action::Unlock { pin } });
                        }
                        gate_message(ui, busy, self.ux.gate_trouble.as_deref(), 18.0);
                        ui.add_space(16.0);
                        if key::key(ui, t(Key::PinForgot), Role::Plain, true).clicked() {
                            self.ux.gate_recover = true;
                            self.ux.gate_trouble = None;
                        }
                    });
                    return;
                }
                ui.vertical_centered(|ui| {
                    ui.add_space(6.0);
                    if empty_out {
                        paint::text(ui, t(Key::PinEmptySay), Type::Note, c(C::Ink2));
                    } else {
                        let how = t(if by_words { Key::GateByWords } else { Key::GateByFile });
                        let say = if locked_out { format!("{}{}", t(Key::GateLockedFive), how) } else { how.to_string() };
                        paint::text(ui, &say, Type::Note, c(C::Ink2));
                    }
                });
                ui.add_space(18.0);
                if empty_out {
                    ui.vertical_centered(|ui| {
                        if key::key(ui, t(Key::PinDoReset), Role::Secondary, true).clicked() {
                            act = Some(Action::ResetEmptyKeybox);
                        }
                        gate_message(ui, false, self.ux.gate_trouble.as_deref(), 12.0);
                    });
                    return;
                }
                ui.spacing_mut().item_spacing.y = tk::S3;
                if by_words {
                    // Twelve always-masked cells; a pasted phrase fills them all; an unknown word turns red without
                    // being shown.
                    let bad = crate::cryptx::strangers(&self.ux.pin_words);
                    pin::words_grid_marked(ui, "gate-words", &mut self.ux.pin_words, &bad);
                    hint(ui, t(Key::PinWordsHint));
                    let ready = self.ux.pin_words.iter().all(|w| !w.expose().trim().is_empty());
                    // Once all twelve are filled, release focus so the next eight characters go to the passcode row.
                    if ready && !self.ux.gate_words_ready {
                        if let Some(id) = ui.memory(|m| m.focused()) {
                            ui.memory_mut(|m| m.surrender_focus(id));
                        }
                }
                self.ux.gate_words_ready = ready;
                // The two ways exclude each other: touching the key file fold makes it the active one.
                let by_file = !self.ux.pin_ks_path.trim().is_empty() || !self.ux.pin_ks_pw.is_empty();
                if ready {
                    field(ui, t(if self.ux.pin_again.is_empty() { Key::WizPinTitle } else { Key::PinAgain }), None, |ui| {
                        let row = pin::pin_row(ui, "gate-new-pin", &mut self.ux.pin, len, self.ux.pin_shake, !by_file && !busy, true);
                        if row.full && self.ux.pin_again.is_empty() {
                            first_pin = true;
                        } else if row.full && !busy {
                            act = Some(Action::RecoverWords { words: joined(&self.ux.pin_words), pin: self.ux.pin_again.clone(), again: std::mem::take(&mut self.ux.pin) });
                        }
                    });
                }
                }
                if by_words {
                    gate_message(ui, busy, self.ux.gate_trouble.as_deref(), 4.0);
                }
                // An imported-key primary recovers with the key file it exported and that file's password (shown
                // open; folded under the words only for vaults written before the primary was recorded).
                let file_way = |me: &mut Self, ui: &mut egui::Ui, first_pin: &mut bool, act: &mut Option<Action>| {
                    ui.spacing_mut().item_spacing.y = tk::S3;
                    paint::text(ui, t(Key::GateDropKeyFile), Type::Note, c(C::Ink2));
                    pick_path(ui, &mut me.ux.pin_ks_path, crate::platform::Pick::File);
                    field(ui, t(Key::GateFilePassword), None, |ui| input::secret_line(ui, &mut me.ux.pin_ks_pw, ""));
                    // The passcode row is enabled only once file and password are both filled: only one row may
                    // hold focus, or the two entries would split between rows and never match.
                    let file_ready = !me.ux.pin_ks_path.trim().is_empty() && !me.ux.pin_ks_pw.is_empty();
                    if file_ready && !me.ux.gate_file_ready {
                        if let Some(id) = ui.memory(|m| m.focused()) {
                            ui.memory_mut(|m| m.surrender_focus(id));
                        }
                    }
                    me.ux.gate_file_ready = file_ready;
                    field(ui, t(if me.ux.pin_again.is_empty() { Key::WizPinTitle } else { Key::PinAgain }), None, |ui| {
                        let row = pin::pin_row(ui, "gate-file-pin", &mut me.ux.pin, len, me.ux.pin_shake, file_ready && !busy, true);
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
                    });
                };
                if by_file && !by_words {
                    file_way(self, ui, &mut first_pin, &mut act);
                    gate_message(ui, busy, self.ux.gate_trouble.as_deref(), 4.0);
                }
                if by_file && by_words {
                    fold::fold(ui, "gate-keystore", t(Key::PinByFile), |ui| {
                        ui.spacing_mut().item_spacing.y = tk::S3;
                        paint::text(ui, t(Key::PinByFileSay), Type::Note, c(C::Ink2));
                        pick_path(ui, &mut self.ux.pin_ks_path, crate::platform::Pick::File);
                        field(ui, t(Key::IdPassword), None, |ui| input::secret_line(ui, &mut self.ux.pin_ks_pw, ""));
                        // Same focus rule as in `file_way` above.
                        let file_ready = !self.ux.pin_ks_path.trim().is_empty() && !self.ux.pin_ks_pw.is_empty();
                        if file_ready && !self.ux.gate_file_ready {
                            if let Some(id) = ui.memory(|m| m.focused()) {
                                ui.memory_mut(|m| m.surrender_focus(id));
                            }
                        }
                        self.ux.gate_file_ready = file_ready;
                        field(ui, t(if self.ux.pin_again.is_empty() { Key::WizPinTitle } else { Key::PinAgain }), None, |ui| {
                            let row = pin::pin_row(ui, "gate-file-pin", &mut self.ux.pin, len, self.ux.pin_shake, file_ready && !busy, true);
                            if row.full && self.ux.pin_again.is_empty() {
                                first_pin = true;
                            } else if row.full && !busy {
                                act = Some(Action::RecoverKeystore {
                                    path: self.ux.pin_ks_path.clone(),
                                    password: self.ux.pin_ks_pw.clone(),
                                    pin: self.ux.pin_again.clone(),
                                    again: std::mem::take(&mut self.ux.pin),
                                });
                            }
                        });
                    });
                }
                // The third way: restore from a whole-machine backup (new passcode, everything resealed).
                ui.vertical_centered(|ui| {
                    from_backup = key::key(ui, t(Key::DoRestoreBackup), Role::Plain, !busy).clicked();
                });
                if !locked_out {
                    ui.vertical_centered(|ui| {
                        ui.add_space(2.0);
                        if key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked() {
                            self.ux.gate_back();
                        }
                    });
                }
            });
        });
        if from_backup {
            self.bk_open(Bk::Restore(RestoreFrom::Locked));
        }
        if first_pin {
            self.take_first_pin(now, true);
        }
        if let Some(a) = act {
            let r = self.act(a, now);
            self.vault_or(VaultSite::Gate, r, now);
        }
    }
}

/// The line under the gate's cells: nothing (no height), "verifying…", or the refusal in a grey box.
/// It opens and closes by animating its height.
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
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Center)));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child.multiply_opacity(open);
    let r = child.vertical_centered(|ui| {
        if busy {
            sheet::busy_note(ui, t(Key::VaultBusy), true);
        } else if let Some(s) = trouble {
            states::note_box(ui, s);
        }
    });
    let got = r.response.rect.height();
    if on && (got - full_h).abs() > 0.5 {
        ui.ctx().data_mut(|d| d.insert_temp(h_id, got));
        // Lay out again at once with the new size so no frame is drawn with the old one.
        ui.ctx().request_discard("gate message height changed");
    }
}
