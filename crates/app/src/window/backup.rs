use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Bk {
    Export,
    Restore(RestoreFrom),
    Own(OwnBk),
    Peek,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum RestoreFrom {
    FirstRun,
    Settings,
    Locked,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum BkStep {
    #[default]
    Pick,
    Confirm,
    NewPin,
}

impl Win {
    pub(super) fn bk_open(&mut self, b: Bk) {
        self.sheets_clear();
        self.ux.bk_clear();
        if self.ux.bk_dir.is_empty() {
            if let Some(b) = &self.shell.machine.backup {
                self.ux.bk_dir = std::path::Path::new(&b.path).parent().map(|p| p.display().to_string()).unwrap_or_default();
            }
        }
        if self.ux.bk_dir.is_empty() {
            self.ux.bk_dir = self.shell.documents.clone();
        }
        self.shell.backup_peek = None;
        self.shell.backup_made = None;
        self.ux.pin_why = None;
        self.ux.bk = Some(b);
    }

    pub(super) fn bk_sheets(&mut self, ctx: &egui::Context, now: f64) {
        let Some(which) = self.ux.bk else { return };
        let gate = self.shell.vault.gate_up();
        if gate != (which == Bk::Restore(RestoreFrom::Locked)) {
            return;
        }
        let len = crate::keybox::PIN_LEN;
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault) || self.shell.tasks.in_flight(crate::task::Kind::Backup);
        let failed = self.shell.failed.get(&crate::task::Kind::Backup).cloned();
        let trouble = self.ux.bk_trouble.clone().or(failed);
        let show_trouble = |ui: &mut egui::Ui| {
            if let Some(f) = &trouble {
                states::err_box(ui, "bk-trouble", f.human(), &f.next(), t(Key::U3RawError), &f.shown());
            }
        };
        let (mut close, mut first_pin) = (false, false);
        let mut go: Option<Action> = None;
        let esc;
        if which == Bk::Export {
            self.measure_if_due(now);
        }
        match which {
            Bk::Export => {
                if self.shell.backup_made.take().is_some() {
                    self.ux.bk_close();
                    return;
                }
                let pin_full = self.ux.bk_pin.chars() == len;
                let content = self.data_content();
                let mut picked = None;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("bk-export", tk::SHEET_NARROW),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::BackupExportTitle), "");
                        fields::stack(ui, |ui, st| {
                            zikaron_ui::fields::rows(ui, |ui, f| {
                                content_row(f, ui, &content);
                                f.folder(ui, t(Key::IdStore), &me.ux.bk_dir, t(Key::PickNone));
                                let stem = format!("{}.{}", crate::backup::file_stem((me.shell.clock)()), crate::backup::EXT);
                                details_row(
                                    ui,
                                    f,
                                    "bk-export-details",
                                    &[
                                        (t(Key::BackupFileName), Val::mono(stem)),
                                        (t(Key::BackupHeader), Val::figure(t(Key::BackupHeaderSay).to_string())),
                                        (t(Key::BackupCipher), Val::figure(t(Key::DataBackupHowSay).to_string())),
                                        (t(Key::BackupWithout), Val::text(t(Key::BackupWithoutSay))),
                                    ],
                                );
                            });
                            st.field(ui, t(Key::BackupPasswordPlain), None, None, |ui, w| secret_strength(ui, "bk-export-pw", &mut me.ux.bk_pw, t(Key::IdPasswordHint), w));
                            st.field(ui, t(Key::BackupPasswordAgain), None, None, |ui, w| secret_again(ui, "bk-export-pw2", &mut me.ux.bk_pw2, &me.ux.bk_pw, w));
                            pin_field(st, ui, t(Key::IdPinGate), None, "bk-export-pin", &mut me.ux.bk_pin, me.ux.pin_shake, !busy);
                        });
                        show_trouble(ui);
                    },
                    |ui, me| {
                        let ready = pin_full && !me.ux.bk_pw.is_empty() && Self::landing_ok(&me.ux.bk_dir) && !busy;
                        let why = still_needs(&[(!Self::landing_ok(&me.ux.bk_dir), Key::IdStore), (me.ux.bk_pw.is_empty(), Key::BackupPasswordPlain), (!pin_full, Key::IdPinGate)]);
                        if page::Page::new().primary_why(ui, t(Key::IdExportGroup), ready, &why).1.clicked() {
                            go = Some(Action::ExportBackup {
                                pin: std::mem::take(&mut me.ux.bk_pin),
                                password: me.ux.bk_pw.clone(),
                                again: me.ux.bk_pw2.clone(),
                                dir: me.ux.bk_dir.clone(),
                            });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, !busy).clicked();
                        if me.shell.tasks.in_flight(crate::task::Kind::Backup) {
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| me.stage_row(ui, crate::task::Kind::Backup));
                        } else if busy {
                            sheet::foot_note(ui, t(Key::VaultBusy));
                        } else {
                            picked = foot_pick(ui, "bk-export", crate::platform::Pick::Folder);
                        }
                    },
                );
                if let Some(p) = picked {
                    self.ux.bk_dir = p;
                }
                esc = out.esc && !busy;
            }
            Bk::Restore(from) if from != RestoreFrom::Locked || self.ux.bk_step == BkStep::Pick => {
                let settings = from == RestoreFrom::Settings;
                let pin_full = self.ux.bk_pin.chars() == len;
                let mut picked = None;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("bk-restore", tk::SHEET_NARROW),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::BackupRestoreTitle), "");
                        fields::stack(ui, |ui, st| {
                            zikaron_ui::fields::rows(ui, |ui, f| file_row(f, ui, t(Key::DataBackupFile), &me.ux.bk_path));
                            st.field(ui, t(Key::BackupPasswordPlain), None, None, |ui, w| input::secret_line_w_marked(ui, &mut me.ux.bk_pw, "", w, false));
                            if settings {
                                pin_field(st, ui, t(Key::IdPinGate), None, "bk-restore-pin", &mut me.ux.bk_pin, me.ux.pin_shake, !busy);
                            }
                        });
                        show_trouble(ui);
                    },
                    |ui, me| {
                        let ready = !me.ux.bk_path.trim().is_empty() && !me.ux.bk_pw.is_empty() && (!settings || pin_full) && !busy;
                        let why = still_needs(&[(me.ux.bk_path.trim().is_empty(), Key::DataBackupFile), (me.ux.bk_pw.is_empty(), Key::BackupPasswordPlain), (settings && !pin_full, Key::IdPinGate)]);
                        let pressed = if from == RestoreFrom::Locked {
                            page::Guide::key(ui, t(Key::PinRecoverTitle), ready).clicked()
                        } else {
                            page::Pen::new().press_why(ui, t(Key::PinRecoverTitle), ready, &why).clicked()
                        };
                        if pressed {
                            let path = me.ux.bk_path.trim().to_string();
                            let password = me.ux.bk_pw.clone();
                            go = Some(match from {
                                RestoreFrom::Locked => Action::PeekBackup { path, password },
                                RestoreFrom::FirstRun => Action::RestoreBackup { path, password, how: crate::action::RestoreHow::FirstRun },
                                RestoreFrom::Settings => Action::RestoreBackup { path, password, how: crate::action::RestoreHow::Settings { pin: std::mem::take(&mut me.ux.bk_pin) } },
                            });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, !busy).clicked();
                        if busy {
                            sheet::foot_note(ui, t(Key::PrimaryResealing));
                        } else {
                            picked = foot_pick(ui, "bk-restore", crate::platform::Pick::File);
                        }
                    },
                );
                if let Some(p) = picked {
                    self.ux.bk_path = p;
                }
                esc = out.esc && !busy;
                if from == RestoreFrom::Locked && self.shell.peeked(&self.ux.bk_path).is_some() {
                    self.ux.bk_step = BkStep::Confirm;
                }
            }
            Bk::Restore(_) if self.ux.bk_step == BkStep::Confirm => {
                let content = self.shell.peeked(&self.ux.bk_path).map(backup_content).unwrap_or_default();
                let file = width::file_name(&self.ux.bk_path);
                let mut sure = false;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("bk-confirm", tk::SHEET_W),
                    self,
                    |ui, _me| {
                        sheet::title(ui, t(Key::BackupConfirmTitle), "");
                        fields::stack(ui, |ui, _st| {
                            zikaron_ui::fields::rows(ui, |ui, f| {
                                let _ = f.info(ui, t(Key::BackupConfirmFile), card::Show::Text(&file));
                                content_row(f, ui, &content);
                            });
                        });
                    },
                    |ui, _me| {
                        sure = page::Pen::new().press(ui, t(Key::DoReplace), true).clicked();
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                    },
                );
                esc = out.esc;
                if sure {
                    self.ux.bk_step = BkStep::NewPin;
                    self.ux.pin.clear();
                    self.ux.pin_again.clear();
                }
            }
            Bk::Restore(_) => {
                let again = !self.ux.pin_again.is_empty();
                let label = t(if again { Key::PinAgain } else { Key::BackupNewPin });
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("bk-new-pin", tk::SHEET_NARROW),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::BackupNewPin), "");
                        fields::stack(ui, |ui, st| {
                            let rules = (!again).then(|| t(Key::PinRules));
                            let why = me.ux.pin_why.filter(|_| me.ux.pin.is_empty() && !again);
                            let row = pin_field_why(st, ui, label, rules, why, "bk-new-pin", &mut me.ux.pin, me.ux.pin_shake, !busy);
                            if !me.ux.pin.is_empty() {
                                me.ux.pin_why = None;
                            }
                            if row.full && me.ux.pin_again.is_empty() {
                                first_pin = true;
                            } else if row.full && !busy {
                                go = Some(Action::RestoreBackup {
                                    path: me.ux.bk_path.trim().to_string(),
                                    password: me.ux.bk_pw.clone(),
                                    how: crate::action::RestoreHow::Locked { pin: me.ux.pin_again.clone(), again: std::mem::take(&mut me.ux.pin) },
                                });
                            }
                        });
                        show_trouble(ui);
                    },
                    |ui, _me| {
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, !busy).clicked();
                        if busy {
                            sheet::foot_note(ui, t(Key::PrimaryResealing));
                        }
                    },
                );
                esc = out.esc && !busy;
            }
            Bk::Peek => {
                let seen = self.shell.peeked(&self.ux.bk_path).cloned();
                let mut picked = None;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("bk-peek", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::BackupPeekTitle), "");
                        fields::stack(ui, |ui, st| {
                            zikaron_ui::fields::rows(ui, |ui, f| {
                                file_row(f, ui, t(Key::DataBackupFile), &me.ux.bk_path);
                                if let Some(s) = &seen {
                                    let made = crate::when::when(s.created);
                                    let _ = f.info(ui, t(Key::IdCreated), card::Show::Text(&made));
                                    content_row(f, ui, &backup_content(s));
                                }
                            });
                            if seen.is_none() {
                                st.field(ui, t(Key::BackupPasswordPlain), None, None, |ui, w| input::secret_line_w_marked(ui, &mut me.ux.bk_pw, "", w, false));
                            }
                        });
                        show_trouble(ui);
                    },
                    |ui, me| {
                        if seen.is_none() {
                            let ready = !me.ux.bk_path.trim().is_empty() && !me.ux.bk_pw.is_empty() && !busy;
                            if page::Guide::key(ui, t(Key::BackupPeekGo), ready).clicked() {
                                go = Some(Action::PeekBackup { path: me.ux.bk_path.trim().to_string(), password: me.ux.bk_pw.clone() });
                            }
                        }
                        close = key::key(ui, t(Key::ToastClose), Role::Secondary, !busy).clicked();
                        if busy {
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| me.stage_row(ui, crate::task::Kind::Backup));
                        } else if seen.is_none() {
                            picked = foot_pick(ui, "bk-peek", crate::platform::Pick::File);
                        }
                    },
                );
                if let Some(p) = picked {
                    self.ux.bk_path = p;
                }
                esc = out.esc && !busy;
            }
            Bk::Own(own) => {
                let (asked, closed, escaped) = self.own_bk_sheet(ctx, own, trouble.clone(), now);
                go = asked;
                close = closed;
                esc = escaped;
            }
        }
        if first_pin {
            self.take_first_pin(now, PinSaid::Sheet);
        }
        if let Some(a) = go {
            if let Some(f) = self.own_hold(&a) {
                self.ux.bk_trouble = Some(f);
            } else {
                self.ux.bk_trouble = None;
                self.shell.failed.remove(&crate::task::Kind::Backup);
                let r = self.act(a, now);
                self.vault_or(VaultSite::Backup, r, now);
            }
        } else if close || esc {
            self.ux.bk_close();
        }
    }

    pub(super) fn backup_group(&mut self, ui: &mut egui::Ui, now: f64) {
        let (mut export, mut peek, mut restore, mut own) = (false, false, false, None);
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::WizBackupTitle), "", |ui| {
                card::form(ui, |ui, f| {
                    export = f.go(ui, t(Key::BackupExportTitle), "", self.shell.unlocked(), "").clicked();
                    peek = f.go(ui, t(Key::BackupPeekTitle), "", true, "").clicked();
                    restore = f.go(ui, t(Key::BackupRestoreTitle), "", true, "").clicked();
                    own = self.own_backup_rows(ui, f);
                });
            });
        });
        if export {
            self.bk_open(Bk::Export);
        }
        if peek {
            self.bk_open(Bk::Peek);
        }
        if restore {
            self.bk_open(Bk::Restore(RestoreFrom::Settings));
        }
        if let Some(o) = own {
            self.bk_open(Bk::Own(o));
        }
        self.own_backup_after(ui, now);
    }

    pub(super) fn bk_back(&mut self, r: Applied, now: f64) {
        if self.own_bk_done(&r) {
            self.ux.bk_close();
            return;
        }
        match r {
            Applied::BackupRestored(_) => {
                if self.ux.bk == Some(Bk::Restore(RestoreFrom::FirstRun)) {
                    self.ux.wizard = None;
                }
                self.ux.gate_clear();
                self.ux.bk_close();
            }
            Applied::Trouble(f) => {
                self.ux.pin_shake = Some(now);
                self.ux.pin.clear();
                self.ux.pin_again.clear();
                self.ux.bk_trouble = Some(f);
            }
            _ => {}
        }
    }

    pub(super) fn measure_if_due(&mut self, now: f64) {
        if self.shell.take_measure_due() {
            self.auto(Action::Measure, now);
        }
    }

    pub(super) fn data_content(&self) -> String {
        match self.shell.backup_carries() {
            Some((ids, items, records)) => crate::lang::filln(Key::BackupContent, &[&ids.to_string(), &items.to_string(), &records.to_string()]),
            None => t(Key::SetNotRead).to_string(),
        }
    }
}

impl Ux {
    pub(super) fn bk_clear(&mut self) {
        self.bk = None;
        self.bk_step = BkStep::Pick;
        self.bk_path.clear();
        self.bk_pw.clear();
        self.bk_pw2.clear();
        self.bk_pin.clear();
        self.bk_trouble = None;
    }

    pub(super) fn bk_close(&mut self) {
        self.bk_clear();
    }
}

pub(super) fn content_row(f: &mut card::Form, ui: &mut egui::Ui, content: &str) {
    f.info_words(ui, t(Key::DataContent), content);
}

#[cfg(test)]
mod peek_only {
    #[test]
    fn the_sheet_that_views_a_backup_asks_to_read_it_and_nothing_else() {
        let src = include_str!("backup.rs");
        let at = src.find("            Bk::Peek => {").expect("the sheet");
        let end = at + src[at..].find("            Bk::Own(").expect("the sheet's end");
        let arm = &src[at..end];
        let asked: Vec<&str> = arm.match_indices("Action::").map(|(i, _)| arm[i + 8..].split(|c: char| !c.is_alphanumeric()).next().unwrap_or_default()).collect();
        assert_eq!(asked, ["PeekBackup"], "the one action it asks");
    }
}
