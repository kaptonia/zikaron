//! The whole-machine backup sheets: export a backup, restore from one (three entry points: first run,
//! settings, the lock screen card), and export the ledger mirror (a ledger export that restores nothing).
//! Every button hands its work to the action layer; the frame reads no disk.

use super::*;

/// Which backup sheet is open.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Bk {
    /// Export a whole-machine backup.
    Export,
    /// Restore from a backup, from one of three entries.
    Restore(RestoreFrom),
    /// Export the ledger mirror.
    Mirror,
}

/// Where a restore starts: the three entry points.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum RestoreFrom {
    FirstRun,
    Settings,
    Locked,
}

/// The lock screen card's restore has three steps: choose the file and its password, confirm the
/// replacement, set a new passcode.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum BkStep {
    #[default]
    Pick,
    Confirm,
    NewPin,
}

impl Win {
    /// Open a backup sheet (every other sheet closes; what was typed before is gone).
    pub(super) fn bk_open(&mut self, b: Bk) {
        self.sheets_clear();
        self.ux.bk_clear();
        if self.ux.bk_dir.is_empty() {
            if let Some(b) = &self.shell.machine.backup {
                self.ux.bk_dir = std::path::Path::new(&b.path).parent().map(|p| p.display().to_string()).unwrap_or_default();
            }
        }
        self.shell.backup_peek = None;
        self.ux.bk = Some(b);
    }

    /// The backup sheets. The lock screen's restore draws above the passcode gate; the others only when the
    /// gate is down.
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
                states::err_box(ui, "bk-trouble", f.human(), f.next(), t(Key::U3RawError), &f.raw());
            }
        };
        let (mut close, mut first_pin) = (false, false);
        let mut go: Option<Action> = None;
        let esc;
        match which {
            Bk::Export => {
                let pin_full = self.ux.bk_pin.chars() == len;
                let stem = format!("{}.{}", crate::backup::file_stem((self.shell.clock)()), crate::backup::EXT);
                let content = self.data_content();
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("bk-export", tk::SHEET_WIDE),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::BackupExportTitle), "");
                        kv::kv(ui, &[(t(Key::DataContent), Val::text(content.clone()))]);
                        field(ui, t(Key::IdPinGate), None, |ui| {
                            pin::pin_row(ui, "bk-export-pin", &mut me.ux.bk_pin, len, me.ux.pin_shake, !busy, true);
                        });
                        field(ui, t(Key::BackupPassword), None, |ui| {
                            input::secret_line(ui, &mut me.ux.bk_pw, "");
                            // The strength bar is only a hint (12 or more characters with a symbol reads as strong).
                            if !me.ux.bk_pw.is_empty() {
                                let (lit, m, k) = match crate::keystore::strength(me.ux.bk_pw.expose()) {
                                    crate::keystore::Strength::Weak => (1, Mark::Bad, Key::StrengthWeak),
                                    crate::keystore::Strength::Fair => (2, Mark::Warn, Key::StrengthFair),
                                    crate::keystore::Strength::Strong => (3, Mark::Ok, Key::StrengthStrong),
                                };
                                pin::strength(ui, lit, m, t(k));
                            }
                        });
                        field(ui, t(Key::BackupPasswordAgain), None, |ui| input::secret_line(ui, &mut me.ux.bk_pw2, ""));
                        Self::place_row(ui, Key::IdStore, &mut me.ux.bk_dir);
                        states::note_box(ui, t(Key::BackupForgetNote));
                        details(
                            ui,
                            "bk-export-details",
                            &[
                                (t(Key::BackupFileName), Val::mono(stem.clone())),
                                (t(Key::BackupHeader), Val::mono(t(Key::BackupHeaderSay).to_string())),
                                (t(Key::BackupCipher), Val::mono(t(Key::DataBackupHowSay).to_string())),
                                (t(Key::BackupWithout), Val::text(t(Key::BackupWithoutSay))),
                            ],
                        );
                        show_trouble(ui);
                    },
                    |ui, me| {
                        let ready = pin_full && !me.ux.bk_pw.is_empty() && Self::landing_ok(&me.ux.bk_dir) && !busy;
                        if page::Page::new().primary_with(ui, t(Key::IdExportGroup), ready).1.clicked() {
                            go = Some(Action::ExportBackup {
                                pin: std::mem::take(&mut me.ux.bk_pin),
                                password: me.ux.bk_pw.clone(),
                                again: me.ux.bk_pw2.clone(),
                                dir: me.ux.bk_dir.clone(),
                            });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                        if busy {
                            sheet::foot_note(ui, t(Key::VaultBusy));
                        }
                    },
                );
                esc = out.esc;
            }
            Bk::Restore(from) if from != RestoreFrom::Locked || self.ux.bk_step == BkStep::Pick => {
                let settings = from == RestoreFrom::Settings;
                let pin_full = self.ux.bk_pin.chars() == len;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("bk-restore", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::BackupRestoreTitle), "");
                        pick_path(ui, &mut me.ux.bk_path, crate::platform::Pick::File);
                        field(ui, t(Key::BackupPasswordPlain), None, |ui| input::secret_line(ui, &mut me.ux.bk_pw, ""));
                        if settings {
                            field(ui, t(Key::IdPinGate), None, |ui| {
                                pin::pin_row(ui, "bk-restore-pin", &mut me.ux.bk_pin, len, me.ux.pin_shake, !busy, true);
                            });
                            states::note_box(ui, t(Key::BackupReplaceNote));
                        }
                        if busy {
                            sheet::foot_note(ui, t(Key::PrimaryResealing));
                        }
                        show_trouble(ui);
                    },
                    |ui, me| {
                        let ready = !me.ux.bk_path.trim().is_empty() && !me.ux.bk_pw.is_empty() && (!settings || pin_full) && !busy;
                        // On the lock screen card this button only opens the backup to inspect it (a confirmation and a new
                        // passcode follow), so it is the guide button; from settings and at first run it is the final step.
                        let pressed = if from == RestoreFrom::Locked {
                            page::Guide::key(ui, t(Key::PinRecoverTitle), ready).clicked()
                        } else {
                            page::Pen::new().press(ui, t(Key::PinRecoverTitle), ready).clicked()
                        };
                        if pressed {
                            let path = me.ux.bk_path.trim().to_string();
                            let password = me.ux.bk_pw.clone();
                            go = Some(match from {
                                // The lock screen card opens the backup first and asks before replacing anything.
                                RestoreFrom::Locked => Action::PeekBackup { path, password },
                                RestoreFrom::FirstRun => Action::RestoreBackup { path, password, how: crate::action::RestoreHow::FirstRun },
                                RestoreFrom::Settings => Action::RestoreBackup { path, password, how: crate::action::RestoreHow::Settings { pin: std::mem::take(&mut me.ux.bk_pin) } },
                            });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, !busy).clicked();
                    },
                );
                esc = out.esc && !busy;
                // The lock screen card: once the backup has opened with its password, confirm before replacing.
                if from == RestoreFrom::Locked && self.shell.backup_peek.is_some() {
                    self.ux.bk_step = BkStep::Confirm;
                }
            }
            Bk::Restore(_) if self.ux.bk_step == BkStep::Confirm => {
                let content = self.shell.backup_peek.as_ref().map(backup_content).unwrap_or_default();
                let file = width::file_name(&self.ux.bk_path);
                let mut sure = false;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("bk-confirm", tk::SHEET_W),
                    self,
                    |ui, _me| {
                        sheet::title(ui, t(Key::BackupConfirmTitle), "");
                        kv::kv(ui, &[(t(Key::BackupConfirmFile), Val::text(file.clone())), (t(Key::DataContent), Val::text(content.clone()))]);
                        states::note_box(ui, t(Key::BackupConfirmNote));
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
                // The new passcode, set once and entered again (the passcode rules); then everything is
                // resealed under a new master key.
                let label = t(if self.ux.pin_again.is_empty() { Key::BackupNewPin } else { Key::PinAgain });
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("bk-new-pin", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, label, t(Key::PinRules));
                        ui.vertical_centered(|ui| {
                            let row = pin::pin_row(ui, "bk-new-pin", &mut me.ux.pin, len, me.ux.pin_shake, !busy, true);
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
                        if busy {
                            sheet::foot_note(ui, t(Key::PrimaryResealing));
                        }
                        show_trouble(ui);
                    },
                    |ui, _me| {
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, !busy).clicked();
                    },
                );
                esc = out.esc && !busy;
            }
            Bk::Mirror => {
                let last = self.shell.settings.mirror.clone();
                let since = match self.shell.archive.as_ref().map(|a| (a.items, a.mirror.clone())) {
                    Some((n, crate::mirror::Mirrored::At { entries, .. })) => fill1(Key::U3EntriesCount, &n.saturating_sub(entries).to_string()),
                    _ => "\u{2014}".to_string(),
                };
                if self.typed.mirror_out.is_empty() {
                    if let Some(r) = &last {
                        self.typed.mirror_out = crate::mirror::folder_of(std::path::Path::new(&r.path)).display().to_string();
                    }
                }
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("bk-mirror", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::MirrorTitle), t(Key::MirrorNote));
                        Self::place_row(ui, Key::MirrorWhere, &mut me.typed.mirror_out);
                        kv::kv(
                            ui,
                            &[
                                (t(Key::MirrorLastOut), last.as_ref().map(|r| Val::mono(crate::when::when(r.at))).unwrap_or_else(|| Val::text(t(Key::DataBackupNever)))),
                                (t(Key::MirrorSince), Val::text(since.clone())),
                            ],
                        );
                        show_trouble(ui);
                    },
                    |ui, me| {
                        if page::Page::new().primary_with(ui, t(Key::DoExportMirrorGo), Self::landing_ok(&me.typed.mirror_out)).1.clicked() {
                            go = Some(Action::ExportMirror { to: me.typed.mirror_out.trim().to_string() });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                    },
                );
                esc = out.esc;
            }
        }
        if first_pin {
            self.take_first_pin(now, false);
        }
        if let Some(a) = go {
            self.ux.bk_trouble = None;
            self.shell.failed.remove(&crate::task::Kind::Backup);
            let r = self.act(a, now);
            self.vault_or(VaultSite::Backup, r, now);
        } else if close || esc {
            self.ux.bk_close();
        }
    }

    /// A backup sheet's answer (immediate or landed in the background).
    pub(super) fn bk_back(&mut self, r: Applied, now: f64) {
        match r {
            // Exporting: the passcode passed and the backup is being written; the toast says when it lands.
            Applied::Started(crate::task::Kind::Backup) if self.ux.bk == Some(Bk::Export) => self.ux.bk_close(),
            Applied::Mirrored { .. } => self.ux.bk_close(),
            Applied::BackupRestored(_) => {
                // Restored: the first-run wizard is done (its steps now read the restored state).
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

    /// What this machine holds, in one line (identities; this home's ledger entries and records; settings).
    pub(super) fn data_content(&self) -> String {
        let ids = self.shell.identities.as_ref().map(|r| r.rows.len()).unwrap_or(0);
        let (items, records) = self.shell.archive.as_ref().map(|a| (a.items, a.records)).unwrap_or((0, 0));
        crate::lang::filln(Key::BackupContent, &[&ids.to_string(), &items.to_string(), &records.to_string()])
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
