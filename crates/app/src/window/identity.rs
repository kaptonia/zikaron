use super::*;
use zikaron_ui::sheetparts as parts;

impl Win {
    pub(super) fn set_keys(&mut self, ui: &mut egui::Ui, now: f64) {
        let seat = self.shell.settings.role;
        let reg = self.shell.identities.clone().unwrap_or_default();
        let current = reg.now().map(|(r, s)| (r.clone(), s));
        let unseated = self.shell.seat_unseated();
        let has_words = current.as_ref().map(|(r, _)| r.kind() == crate::identity::Kind::Words).unwrap_or(false);
        let can_sign = !unseated && self.shell.anchor.is_some();
        let name = current.as_ref().map(|(r, _)| id_name(r)).unwrap_or_else(|| t(Key::IdNone).to_string());
        let kind = current.as_ref().map(|(r, _)| format!("{} \u{b7} {}", t(id_seat_key(seat)), t(id_kind_key(r.kind())))).unwrap_or_default();
        if self.shell.tasks.in_flight(crate::task::Kind::Keystore) && !self.status_reports(crate::task::Kind::Keystore) {
            states::note_box(ui, t(Key::V2Encrypting));
        }
        if unseated {
            zikaron_ui::callout::callout(ui, t(Key::IdSeatEmptyNote));
        }
        let created = match current.as_ref().map(|(r, _)| r.created.clone()) {
            Some(ref c) if c != crate::identity::NO_CREATED => c.get(0..10).unwrap_or(c).to_string(),
            _ => t(Key::IdCreatedNone).to_string(),
        };
        let root = current.as_ref().and_then(|(r, _)| r.address(seat)).or(self.shell.anchor);
        let addr = self.shell.anchor.filter(|_| !unseated).and(root).map(|a| a.hex());
        let gas = self.own_gas_fact().unwrap_or_else(|| match self.chain_read() {
            Err(f) => (Mark::Bad, no_amount(fill1(Key::SetReadFailedNow, f.human()))),
            Ok(Some(Done::Chain { gas_wei: Some(w), .. })) => (if *w > 0 { Mark::Ok } else { Mark::Bad }, held_amount(*w, |x| fill1(Key::SetGasSay, x))),
            _ => (Mark::Todo, no_amount(t(Key::SetNotRead))),
        });
        let seen = self.shell.backup_seen.clone();
        let file = seen.as_ref().map(file_backup);
        let backed = match &current {
            Some((r, _)) => {
                let has_file = matches!(file, Some(FileBackup::There | FileBackup::NoPlace));
                let k = match (r.backed_words, has_file) {
                    (true, true) => Key::IdBackedBoth,
                    (true, false) => Key::IdBackedWords,
                    (false, true) => Key::IdBackedFile,
                    (false, false) => Key::IdNotBacked,
                };
                (if r.backed_words || has_file { Mark::Ok } else { Mark::Warn }, t(k).to_string())
            }
            None => (Mark::Todo, t(Key::SetNotRead).to_string()),
        };
        let backup_at = match file {
            Some(FileBackup::Never) => (Mark::Todo, t(Key::IdBackupNever).to_string()),
            Some(FileBackup::There) => (Mark::Ok, t(Key::IdBackupThere).to_string()),
            Some(FileBackup::NoPlace) => (Mark::Ok, t(Key::IdBackupNoPlace).to_string()),
            Some(FileBackup::Gone) => (Mark::Bad, t(Key::IdBackupGone).to_string()),
            None => (Mark::Todo, t(Key::SetNotRead).to_string()),
        };
        let primary = self.shell.primary.clone();
        let this_primary = match (&primary, &current) {
            (Some((p, _)), Some((r, _))) => p.eq_ignore_ascii_case(&r.id),
            _ => false,
        };
        let primary_name = primary
            .as_ref()
            .and_then(|(p, _)| reg.find(p))
            .map(id_name)
            .unwrap_or_else(|| t(Key::IdNone).to_string());
        let mut sub = match &current {
            Some(_) if this_primary => format!("{} \u{b7} {}", t(id_seat_key(seat)), t(Key::IdPrimaryTag)),
            Some(_) => kind.clone(),
            None => String::new(),
        };
        if let Some(more) = current.as_ref().and_then(|_| self.own_card_more(seat)) {
            sub = if sub.is_empty() { more } else { format!("{sub} \u{b7} {more}") };
        }
        let addr_short = addr.as_deref().map(card::short).unwrap_or_default();
        let addr_fact = match (&addr, unseated) {
            (Some(_), _) => card::Fact::Mono(&addr_short),
            (None, true) => card::Fact::Warn(t(Key::IdSeatEmpty)),
            (None, false) => card::Fact::Warn(t(if current.is_some() { Key::IdKeyMissing } else { Key::SetNoKey })),
        };
        let gas_fact = match gas.0 {
            Mark::Ok => card::Fact::Amount(&gas.1),
            Mark::Todo => card::Fact::Quiet(&gas.1.shown),
            _ => card::Fact::Warn(&gas.1.shown),
        };
        let backed_fact = if backup_at.0 == Mark::Bad {
            card::Fact::Warn(&backup_at.1)
        } else if backed.0 == Mark::Ok {
            card::Fact::Text(&backed.1)
        } else {
            card::Fact::Warn(&backed.1)
        };
        let mut copy = false;
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                copy = f.summary(ui, Glyph::Key, card::Tint::Blue, &name, None, &sub, |ui| addr.is_some() && key::key(ui, t(Key::IdDoCopy), Role::Secondary, true).clicked());
                f.facts(ui, &[(t(Key::IdRoot), addr_fact), (t(Key::IdGas), gas_fact), (t(Key::IdBackupState), backed_fact)]);
            });
        });
        let pin_set = !self.shell.vault.absent();
        let lock_on = self.shell.machine.auto_lock;
        let lock_now = self.shell.machine.auto_lock_secs;
        let lock_items: Vec<String> = crate::machine::LOCK_CHOICES.iter().map(|x| fill1(Key::MinutesN, &(x / 60).to_string())).collect();
        let lock_label = fill1(Key::MinutesN, &(lock_now / 60).to_string());
        let (mut change, mut set_pin, mut lock_pick, mut flip) = (false, false, None, false);
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::IdPin), "", |ui| {
                card::form(ui, |ui, f| {
                    if pin_set {
                        change = f.go(ui, t(Key::ChangePinTitle), "", true, "").clicked();
                        flip = f.switch(ui, t(Key::IdAutoLock), "", lock_on, true);
                        if lock_on {
                            f.with(ui, t(Key::IdIdleFor), "", |ui| {
                                let items: Vec<menu::Item> = lock_items
                                    .iter()
                                    .enumerate()
                                    .map(|(i, s)| menu::Item::Row(menu::Row { label: s, check: Some(crate::machine::LOCK_CHOICES[i] == lock_now), ..Default::default() }))
                                    .collect();
                                lock_pick = menu::menu_key(ui, "auto-lock", &lock_label, false, 150.0, &items);
                            });
                        }
                    } else {
                        set_pin = f.act(ui, t(Key::IdPin), t(Key::IdPinNone), key::Key::new(t(Key::IdDoSetPin), Role::Secondary)).clicked();
                    }
                });
            });
        });
        let (mut backup, mut words) = (false, false);
        stagger(ui, 2, |ui| {
            card::section(ui, t(Key::IdExportGroup), "", |ui| {
                card::form(ui, |ui, f| {
                    backup = f.go(ui, t(Key::IdDoBackupGo), "", can_sign, "").clicked();
                    words = f.go(ui, t(Key::IdWordsTitle), "", !unseated && has_words, if has_words || current.is_none() { "" } else { t(Key::IdNoWordsHint) }).clicked();
                });
            });
        });
        let (mut set_primary, mut switch, mut new, mut import, mut rename) = (false, false, false, false, false);
        let mut own_press = None;
        if let Some(f) = crate::register::write_back_trouble() {
            states::err_box(ui, "register-write-back", f.human(), &f.next(), t(Key::U3RawError), &f.shown());
        }
        let primary_how = self.own_primary_how(primary.as_ref().and_then(|(p, _)| reg.find(p)));
        stagger(ui, 3, |ui| {
            card::section(ui, t(Key::IdGroup), "", |ui| {
                card::form(ui, |ui, f| {
                    let _ = f.info(ui, t(Key::IdPrimaryTag), card::Show::Text(if this_primary { t(Key::IdPrimaryThis) } else { &primary_name }));
                    if let Some((name, how)) = &primary_how {
                        let _ = f.info(ui, name, card::Show::Text(how));
                    }
                    if !this_primary {
                        set_primary = f.go(ui, t(Key::PrimaryTitle), "", current.is_some() && !unseated, "").clicked();
                    }
                    if reg.rows.len() > 1 {
                        switch = f.go(ui, t(Key::IdSwitchTitle), &fill1(Key::CountN, &reg.rows.len().to_string()), true, "").clicked();
                    } else {
                        let _ = f.info(ui, t(Key::IdSwitchTitle), card::Show::Empty(t(Key::IdOnlyOne)));
                    }
                    new = f.go(ui, t(Key::IdNewTitle), "", true, "").clicked();
                    import = f.go(ui, t(Key::IdImportTitle), "", true, "").clicked();
                    rename = f.go(ui, t(Key::IdDoRenameGo), "", current.is_some(), "").clicked();
                    own_press = self.own_id_rows(ui, f, current.is_some(), unseated, can_sign);
                });
            });
        });
        self.own_keys_after(ui, now);
        let registry = self.ux.register_at.get_or_insert_with(|| crate::register::path().map(|p| p.display().to_string()).unwrap_or_default()).clone();
        let seat_addr = |which: crate::roles::Role| current.as_ref().and_then(|(r, _)| r.address(which)).map(|a| Val::mono(a.hex())).unwrap_or_else(|| Val::text(t(Key::IdSeatEmpty)));
        let mut raw: Vec<(&str, Val)> = Vec::new();
        if let Some(a) = self.shell.anchor.filter(|_| !unseated) {
            if let Some(r) = &addr {
                raw.push((t(Key::IdRoot), Val::mono(r.clone())));
            }
            raw.push((t(Key::IdAddress), Val::mono(a.hex())));
            raw.extend(self.own_key_rows(current.as_ref().map(|(r, _)| r), seat));
        }
        for which in crate::roles::Role::ALL {
            raw.push((t(id_seat_row_key(which)), seat_addr(which)));
        }
        if let Some(b) = seen.as_ref().filter(|b| b.at != crate::identity::NO_BACKUP_AT) {
            raw.push((t(Key::IdBackupAt), Val::figure(b.at.clone())));
        }
        raw.push((t(Key::IdBackupAt), Val::text(backup_at.1.clone())));
        raw.push((t(Key::IdCreated), Val::figure(created.clone())));
        raw.push((t(Key::IdStore), Val::text(t(Key::IdKeybox))));
        if has_words {
            raw.push((t(Key::IdPaths), Val::mono(crate::roles::Role::ALL.iter().map(|r| crate::family::path_text(*r)).collect::<Vec<_>>().join(" \u{b7} "))));
        }
        raw.push((t(Key::IdRegistry), Val::mono(registry)));
        raw.extend(self.own_details(current.is_some() && !unseated, seat));
        let mut open = self.ux.u3.keys_details;
        stagger(ui, 4, |ui| details_card_held(ui, "keys-details", &raw, &mut open));
        self.ux.u3.keys_details = open;
        let mut delete = false;
        stagger(ui, 5, |ui| {
            if current.is_some() {
                card::form(ui, |ui, f| delete = f.danger(ui, t(Key::IdDoDeleteGo)).clicked());
            }
        });
        if let Some(p) = own_press {
            self.own_id_pressed(p, now);
        }
        if copy {
            if let Some(a) = &addr {
                ui.ctx().copy_text(a.clone());
                self.toasts.say(t(Key::SaidCopiedAddress), Tone::Note, now);
            }
        }
        if let Some(i) = lock_pick {
            let secs = crate::machine::LOCK_CHOICES[i];
            if secs != lock_now {
                self.act(Action::SetAutoLock { on: true, secs }, now);
            }
        }
        if flip {
            self.act(Action::SetAutoLock { on: !lock_on, secs: lock_now }, now);
        }
        if set_primary {
            if let Some((r, _)) = &current {
                self.id_layer_open(IdModal::Primary(r.id.clone()));
            }
        }
        if change {
            self.id_layer_open(IdModal::Pin);
        }
        if set_pin {
            self.wizard_open(crate::nav::Step::Pin);
        }
        if backup {
            self.id_layer_open(IdModal::Backup);
        }
        if words {
            self.id_layer_open(IdModal::Words);
        }
        if switch {
            self.id_layer_open(IdModal::Switch);
        }
        if new {
            self.id_layer_open(IdModal::New);
            self.act(Action::NewIdentity, now);
        }
        if import {
            self.id_layer_open(IdModal::Import);
        }
        if rename {
            if let Some((r, _)) = &current {
                let (id, label) = (r.id.clone(), r.label.clone());
                self.id_layer_open(IdModal::Name(id));
                self.ux.id_label = label;
            }
        }
        if delete {
            if let Some((r, _)) = &current {
                self.id_layer_open(IdModal::Delete(r.id.clone()));
            }
        }
    }

    pub(super) fn id_sheets(&mut self, ctx: &egui::Context, now: f64) {
        use crate::action::ImportForm;
        let Some(which) = self.ux.id_modal.clone() else { return };
        let seat = self.shell.settings.role;
        let reg = self.shell.identities.clone().unwrap_or_default();
        let current = reg.now().map(|(r, s)| (r.clone(), s));
        let trouble = self.ux.id_trouble.clone();
        let show_trouble = |ui: &mut egui::Ui| {
            if let Some(f) = &trouble {
                states::err_box(ui, "id-trouble", f.human(), &f.next(), t(Key::U3RawError), &f.shown());
            }
        };
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault);
        let busy_note = |ui: &mut egui::Ui| {
            if busy {
                sheet::busy_note(ui, t(Key::VaultBusy), false);
            }
        };
        let len = crate::keybox::PIN_LEN;
        let name_of = id_name;
        let (mut close, mut ask_words, mut copied, mut first_pin) = (false, false, false, false);
        let mut esc;
        let mut go: Option<Action> = None;
        match which {
            IdModal::New => {
                let fresh = self.shell.new_words.as_ref().map(|f| (f.words(), f.picks));
                let confirming = self.ux.id_confirming;
                let seen = self.ux.id_words_seen;
                let open = self.ux.id_words_open;
                let slide = if confirming { sheet::Slide::Forward } else { sheet::Slide::Back };
                let (mut hide, mut back) = (false, false);
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-new", tk::SHEET_WIDE).step(u64::from(confirming), slide),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdNewTitle), "");
                        fields::stack(ui, |ui, st| {
                            if !confirming {
                                st.field(ui, t(Key::IdLabel), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut me.ux.id_new_label, t(Key::IdLabelExample), w, fields::Look::default()));
                                if me.ux.wizard.is_none() {
                                    zikaron_ui::fields::rows(ui, |ui, f| me.own_network_row(f, ui, "id-new-network"));
                                }
                                let words = fresh.as_ref().map(|(w, _)| w.clone());
                                let asked = words.is_some() && st.field(ui, t(Key::IdTabWords), None, None, |ui, _| parts::words(ui, "id-new-words", words.as_deref(), open, t(Key::IdShowNewWords)).clicked());
                                if asked {
                                    me.ux.id_words_open = true;
                                    me.ux.id_words_seen = true;
                                }
                            } else if let Some((_, picks)) = fresh.as_ref() {
                                for (k, p) in picks.iter().enumerate() {
                                    st.field(ui, &fill1(Key::IdWordN, &(p + 1).to_string()), None, None, |ui, w| input::secret_line_w_marked(ui, &mut me.ux.id_confirm[k], "", w, false));
                                }
                            }
                        });
                        show_trouble(ui);
                    },
                    |ui, me| {
                        match (fresh.as_ref(), confirming) {
                            (None, _) => {
                                if page::Page::new().primary(ui, t(Key::IdDoGenerate)).1.clicked() {
                                    go = Some(Action::NewIdentity);
                                }
                            }
                            (Some(_), false) => copied = page::Page::new().primary_with(ui, t(Key::IdDoCopied), seen).1.clicked(),
                            (Some((_, picks)), true) => {
                                if page::Page::new().primary_with(ui, t(Key::IdDoConfirm), !busy).1.clicked() {
                                    let answers = picks.iter().zip(me.ux.id_confirm.iter()).map(|(i, w)| (*i, w.clone())).collect();
                                    go = Some(Action::ConfirmIdentity { answers, label: me.ux.id_new_label.clone(), network: me.id_network_now() });
                                }
                            }
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                        if busy {
                            busy_note(ui);
                        } else if confirming {
                            back = foot_left(ui, t(Key::WizPrev));
                        } else if open && fresh.is_some() {
                            hide = foot_left(ui, t(Key::IdHideNewWords));
                        }
                    },
                );
                esc = out.esc;
                if hide {
                    self.ux.id_words_open = false;
                }
                if back {
                    self.ux.id_confirming = false;
                    self.ux.id_confirm = Default::default();
                }
                if close || esc {
                    go = Some(Action::DropFresh);
                    (close, esc) = (false, false);
                }
            }
            IdModal::Import => {
                let tabs = own::IMPORT_TABS;
                let tab_at = self.ux.id_tab.min(tabs.len().saturating_sub(1));
                let tab = tabs.get(tab_at).copied().unwrap_or(ImportTab::Words);
                let into_seat = self.ux.id_seat.unwrap_or(seat);
                let makes_primary = self.shell.primary.is_none();
                if tab == ImportTab::Key && makes_primary && self.ux.id_dir.trim().is_empty() {
                    self.ux.id_dir = self.shell.documents.clone();
                }
                let ready = match tab {
                    ImportTab::Words => self.ux.id_words.iter().all(|w| !w.expose().trim().is_empty()),
                    ImportTab::Key => {
                        !self.ux.id_hex.expose().trim().is_empty()
                            && (!makes_primary || (!self.ux.id_pw.is_empty() && !self.ux.id_pw2.is_empty() && Self::landing_ok(&self.ux.id_dir)))
                    }
                    ImportTab::File => !self.ux.id_ks_path.trim().is_empty(),
                };
                let why = match tab {
                    ImportTab::Words => still_needs(&[(!ready, Key::IdTabWords)]),
                    ImportTab::Key => still_needs(&[
                        (self.ux.id_hex.expose().trim().is_empty(), Key::IdTabHex),
                        (makes_primary && !Self::landing_ok(&self.ux.id_dir), Key::IdStore),
                        (makes_primary && (self.ux.id_pw.is_empty() || self.ux.id_pw2.is_empty()), Key::IdFilePassword),
                    ]),
                    ImportTab::File => still_needs(&[(!ready, Key::IdTabFile)]),
                };
                let mut dropped: Option<drop::Drop> = None;
                let mut picked = None;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-import", tk::SHEET_WIDE).step(tab_at as u64, sheet::Slide::None),
                    self,
                    |ui, me| {
                        let words: Vec<&str> = tabs.iter().map(|x| t(x.word())).collect();
                        if let Some(i) = parts::title_seg(ui, t(Key::IdImportTitle), "id-import-tabs", &words, tab_at) {
                            me.ux.id_tab = i;
                            me.ux.id_trouble = None;
                        }
                        let seat_row = |ui: &mut egui::Ui, me: &mut Win| {
                            if !own::IMPORT_SEAT_ROW {
                                return;
                            }
                            let cells: Vec<seg::Cell> = crate::roles::Role::ALL.iter().map(|r| seg::Cell::from(t(id_seat_key(*r)))).collect();
                            let mut pick = None;
                            zikaron_ui::fields::rows(ui, |ui, f| pick = f.with(ui, t(Key::IdImportSeat), "", |ui| seg::seg(ui, "id-import-seat", &cells, crate::roles::Role::ALL.iter().position(|r| *r == into_seat).unwrap_or(0))));
                            if let Some(i) = pick {
                                me.ux.id_seat = crate::roles::Role::ALL.get(i).copied();
                            }
                        };
                        fields::stack(ui, |ui, st| {
                            match tab {
                                ImportTab::Words => {
                                    st.field(ui, t(Key::IdTabWords), None, None, |ui, _| {
                                        let bad = crate::cryptx::strangers(&me.ux.id_words);
                                        pin::words_grid_marked(ui, "id-import-words", &mut me.ux.id_words, &bad);
                                    });
                                }
                                ImportTab::Key => {
                                    st.field(ui, t(Key::IdTabHex), None, None, |ui, w| input::secret_key_line(ui, &mut me.ux.id_hex, t(Key::IdHexHint), w, false));
                                    seat_row(ui, me);
                                    if makes_primary {
                                        key_file_fields(st, ui, me);
                                    }
                                }
                                ImportTab::File => {
                                    dropped = Some(drop::Drop { dropped: drop::take_dropped(ui.ctx()), hovering: false, clicked: false });
                                    zikaron_ui::fields::rows(ui, |ui, f| file_row(f, ui, t(Key::IdTabFile), &me.ux.id_ks_path));
                                    st.field(ui, t(Key::IdFilePassword), None, None, |ui, w| input::secret_line_w_marked(ui, &mut me.ux.id_ks_pw, "", w, false));
                                    seat_row(ui, me);
                                }
                            }
                            st.field(ui, t(Key::IdLabel), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut me.ux.id_new_label, t(Key::IdLabelExample), w, fields::Look::default()));
                            if me.ux.wizard.is_none() {
                                zikaron_ui::fields::rows(ui, |ui, f| me.own_network_row(f, ui, "id-import-network"));
                            }
                        });
                        show_trouble(ui);
                    },
                    |ui, me| {
                        if page::Page::new().primary_why(ui, t(Key::IdDoImportGo), ready && !busy, &why).1.clicked() {
                            let form = match tab {
                                ImportTab::Words => ImportForm::Words(joined(&me.ux.id_words)),
                                ImportTab::Key => ImportForm::PrivateKey {
                                    key: me.ux.id_hex.clone(),
                                    keyfile: makes_primary.then(|| crate::action::KeyFileOut {
                                        password: me.ux.id_pw.clone(),
                                        again: me.ux.id_pw2.clone(),
                                        dir: me.ux.id_dir.clone(),
                                    }),
                                },
                                ImportTab::File => ImportForm::Keystore { path: me.ux.id_ks_path.clone(), password: me.ux.id_ks_pw.clone() },
                            };
                            go = Some(Action::ImportIdentity { form, seat: into_seat, label: me.ux.id_new_label.clone(), network: me.id_import_network_now() });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                        if busy {
                            busy_note(ui);
                        } else if tab == ImportTab::Key && makes_primary {
                            picked = foot_pick(ui, "id-import-dir", crate::platform::Pick::Folder);
                        } else if tab == ImportTab::File {
                            picked = foot_pick(ui, "id-import-file", crate::platform::Pick::File);
                        }
                    },
                );
                esc = out.esc;
                if let Some(p) = dropped.as_ref().and_then(|d| self.one_drop(d, now)) {
                    self.ux.id_ks_path = p;
                }
                if let Some(p) = picked {
                    if tab == ImportTab::File {
                        self.ux.id_ks_path = p;
                    } else {
                        self.ux.id_dir = p;
                    }
                }
            }
            IdModal::Words => {
                let has_words = current.as_ref().map(|(r, _)| r.kind() == crate::identity::Kind::Words).unwrap_or(false);
                let words = self.shell.words.clone();
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-words", tk::SHEET_NARROW).step(u64::from(words.is_some()), sheet::Slide::None),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdWordsTitle), "");
                        fields::stack(ui, |ui, st| {
                            if !has_words {
                                zikaron_ui::callout::callout(ui, t(Key::IdNoWordsHint));
                                return;
                            }
                            match words.as_deref() {
                                None => {
                                    if pin_field(st, ui, t(Key::IdPinGate), None, "id-words-pin", &mut me.ux.pin, me.ux.pin_shake, !busy).full && !busy {
                                        ask_words = true;
                                    }
                                }
                                Some(w) => {
                                    parts::words(ui, "id-words-grid", Some(w), true, "");
                                }
                            }
                        });
                        show_trouble(ui);
                    },
                    |ui, _me| {
                        close = key::key(ui, t(Key::IdDoClose), Role::Secondary, true).clicked();
                        busy_note(ui);
                    },
                );
                esc = out.esc;
                if close || esc {
                    go = Some(Action::HideWords);
                    (close, esc) = (false, false);
                }
            }
            IdModal::Switch => {
                let primary = self.shell.primary.as_ref().map(|(p, _)| p.clone());
                let mut details_open = self.ux.id_switch_open.is_some();
                let mut addrs: Vec<(String, card::Detail)> = Vec::new();
                for r in &reg.rows {
                    for (which, label) in crate::roles::Role::ALL.map(|r| (r, id_seat_row_key(r))) {
                        let v = match r.address(which) {
                            Some(a) => card::Detail::Mono(a.hex()),
                            None => card::Detail::Quiet(t(Key::IdSeatEmpty).to_string()),
                        };
                        addrs.push((format!("{} \u{b7} {}", name_of(r), t(label)), v));
                    }
                }
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-switch", tk::SHEET_W),
                    self,
                    |ui, _me| {
                        sheet::title(ui, t(Key::IdSwitchTitle), "");
                        zikaron_ui::fields::rows(ui, |ui, f| {
                            f.keep(ui);
                            for r in &reg.rows {
                                let is_cur = current.as_ref().map(|(c, _)| c.id == r.id).unwrap_or(false);
                                let is_primary = primary.as_deref().map(|p| p.eq_ignore_ascii_case(&r.id)).unwrap_or(false);
                                let sub = if is_primary { format!("{} \u{b7} {}", t(id_kind_key(r.kind())), t(Key::IdPrimaryTag)) } else { t(id_kind_key(r.kind())).to_string() };
                                f.key(("id-switch-row", &r.id));
                                let _ = f.line(ui, None, &name_of(r), &sub, false, false, c(C::Ink), |ui| {
                                    if is_cur {
                                        mark::pill(ui, t(Key::IdIsCurrent), PillTone::Ok);
                                    } else if key::show_w(ui, key::Key::new(t(Key::IdDoSwitchGo), Role::Secondary), 92.0).clicked() {
                                        go = Some(Action::SwitchIdentity { id: r.id.clone() });
                                    }
                                });
                            }
                            f.key("id-switch-details");
                            let widest = addrs.iter().map(|(k, _)| paint::galley(ui, k, Type::Note, c(C::Ink2)).size().x).fold(0.0, f32::max);
                            let rows: Vec<(&str, card::Detail)> = addrs.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                            let mut open = details_open;
                            let got = f.fold_held(ui, "id-switch-details", t(Key::SetEvidence), "", &mut open, |ui, f| f.detail_rows(ui, "id-switch-details", &rows, widest)).flatten();
                            details_open = open;
                            tell_copied(ui.ctx(), got);
                        });
                        show_trouble(ui);
                    },
                    |ui, _me| {
                        close = key::key(ui, t(Key::ToastClose), Role::Secondary, true).clicked();
                    },
                );
                esc = out.esc;
                self.ux.id_switch_open = details_open.then(|| current.as_ref().map(|(r, _)| r.id.clone()).unwrap_or_default());
            }
            IdModal::Delete(id) if self.shell.primary.as_ref().map(|(p, _)| p.eq_ignore_ascii_case(&id)).unwrap_or(false) => {
                let others = reg.rows.iter().any(|r| !r.id.eq_ignore_ascii_case(&id));
                let (mut to_primary, mut to_new) = (false, false);
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-delete-primary", tk::SHEET_W),
                    self,
                    |ui, _me| {
                        sheet::title(ui, t(Key::PrimaryNoDeleteTitle), "");
                        zikaron_ui::callout::callout(ui, t(if others { Key::FaultNextPrimaryDelete } else { Key::PrimaryNoDeleteNew }));
                        show_trouble(ui);
                    },
                    |ui, _me| {
                        if others {
                            to_primary = key::key(ui, t(Key::PrimaryGoSet), Role::Secondary, true).clicked();
                        } else {
                            to_new = key::key(ui, t(Key::IdNewTitle), Role::Secondary, true).clicked();
                        }
                        close = key::key(ui, t(Key::IdDoClose), Role::Secondary, true).clicked();
                    },
                );
                esc = out.esc;
                if to_primary {
                    if let Some(r) = reg.rows.iter().find(|r| !r.id.eq_ignore_ascii_case(&id)) {
                        self.act(Action::SwitchIdentity { id: r.id.clone() }, now);
                    }
                    self.ux.id_close();
                    self.go(Place::Settings(Section::Keys), now);
                    return;
                }
                if to_new {
                    self.id_layer_open(IdModal::New);
                    self.act(Action::NewIdentity, now);
                    return;
                }
            }
            IdModal::Primary(id) => {
                let row = reg.find(&id).cloned();
                let old = self.shell.primary.as_ref().and_then(|(p, _)| reg.find(p)).cloned();
                let words = row.as_ref().map(|r| r.kind() == crate::identity::Kind::Words).unwrap_or(false);
                let need_file = !words && !row.as_ref().map(|r| r.backed_file).unwrap_or(false);
                let pin_full = self.ux.id_pin.chars() == len;
                let rekeying = self.shell.rekeying;
                let new_name = row.as_ref().map(name_of).unwrap_or_default();
                let old_said = old.as_ref().map(|r| fill1(Key::PrimaryOldDemoted, &name_of(r))).unwrap_or_else(|| t(Key::IdNone).to_string());
                let mut export = false;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-primary", tk::SHEET_NARROW),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::PrimaryTitle), "");
                        fields::stack(ui, |ui, st| {
                            zikaron_ui::fields::rows(ui, |ui, f| {
                                let _ = f.info(ui, t(Key::PrimaryNew), card::Show::Text(&new_name));
                                let _ = f.info(ui, t(Key::PrimaryOld), card::Show::Text(&old_said));
                                let _ = f.info(ui, t(Key::PrimaryRecovers), card::Show::Text(t(if words { Key::PrimaryByWords } else { Key::PrimaryByFile })));
                            });
                            if need_file {
                                zikaron_ui::callout::callout(ui, t(Key::PrimaryExportFirst));
                            }
                            pin_field(st, ui, t(Key::IdPinGate), None, "id-primary-pin", &mut me.ux.id_pin, me.ux.pin_shake, !need_file && !rekeying);
                        });
                        show_trouble(ui);
                    },
                    |ui, me| {
                        let why = if need_file { t(Key::PrimaryExportFirst).to_string() } else { still_needs(&[(!pin_full, Key::IdPinGate)]) };
                        if page::Pen::new().press_why(ui, t(Key::PrimaryTitle), row.is_some() && !need_file && pin_full && !busy && !rekeying, &why).clicked() {
                            go = Some(Action::SetPrimary { id: id.clone(), pin: std::mem::take(&mut me.ux.id_pin) });
                        }
                        if need_file {
                            export = key::key(ui, t(Key::IdDoBackupGo), Role::Secondary, true).clicked();
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, !rekeying).clicked();
                        if rekeying {
                            sheet::foot_note(ui, t(Key::PrimaryResealing));
                        } else {
                            busy_note(ui);
                        }
                    },
                );
                esc = out.esc && !rekeying;
                if export {
                    self.id_layer_open(IdModal::Backup);
                    return;
                }
            }
            IdModal::Delete(id) => {
                let row = reg.find(&id).cloned();
                let ledgers: Vec<&'static str> = self.ux.id_delete_ledgers.iter().map(|s| t(id_seat_row_key(*s))).collect();
                let seen = self.ux.id_delete_backup.clone();
                let pin_full = self.ux.id_pin.chars() == len;
                let addr = row.as_ref().and_then(|r| r.address(seat).or_else(|| r.address(r.first_seat()))).map(|a| a.hex()).unwrap_or_default();
                let name = row.as_ref().map(name_of).unwrap_or_default();
                let effect = t(own_delete_effect(seat));
                let ledgers_say = if ledgers.is_empty() { None } else { Some(fill1(Key::IdDeleteLedgerSome, &ledgers.join(" \u{b7} "))) };
                let mut delete = false;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-delete", tk::SHEET_NARROW),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdDeleteTitle), "");
                        fields::stack(ui, |ui, st| {
                            let mut copied = None;
                            zikaron_ui::fields::rows(ui, |ui, f| {
                                let _ = f.info(ui, t(Key::IdGroup), card::Show::Text(&name));
                                copied = f.info(ui, t(Key::IdAddress), card::Show::Short(&addr));
                                f.info_words(ui, t(Key::IdDeleteEffect), effect);
                                match &ledgers_say {
                                    Some(v) => {
                                        let _ = f.info(ui, t(Key::IdDeleteLedgers), card::Show::Mark(Mark::Warn, v));
                                    }
                                    None => f.info_words(ui, t(Key::IdDeleteLedgers), t(Key::IdDeleteLedgerNone)),
                                }
                            });
                            tell_copied(ui.ctx(), copied);
                            if let Some(b) = &seen {
                                if file_backup(b) == FileBackup::Gone {
                                    states::err_box(ui, "id-delete-backup", t(Key::IdBackupGone), t(Key::FaultNextBackupNotLanded), t(Key::U3RawError), &b.at);
                                }
                            }
                            pin_field(st, ui, t(Key::IdPinGate), None, "id-delete-pin", &mut me.ux.id_pin, me.ux.pin_shake, true);
                        });
                        show_trouble(ui);
                    },
                    |ui, _me| {
                        delete = page::Erase::press_why(ui, t(Key::IdDoDeleteGo), row.is_some() && pin_full && !busy, &still_needs(&[(!pin_full, Key::IdPinGate)])).clicked();
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                        busy_note(ui);
                    },
                );
                esc = out.esc;
                if delete {
                    go = Some(Action::DeleteIdentity { id: id.clone(), pin: std::mem::take(&mut self.ux.id_pin) });
                    self.ux.id_deleting = row.map(|r| name_of(&r));
                }
            }
            IdModal::Backup => {
                let pin_full = self.ux.id_pin.chars() == len;
                if self.ux.id_dir.trim().is_empty() {
                    self.ux.id_dir = self.shell.documents.clone();
                }
                let who = current.as_ref().map(|(r, _)| {
                    let primary = self.shell.primary.as_ref().is_some_and(|(p, _)| p.eq_ignore_ascii_case(&r.id));
                    if primary { format!("{} \u{b7} {}", name_of(r), t(Key::IdPrimaryTag)) } else { name_of(r) }
                });
                let mut picked = None;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-backup", tk::SHEET_NARROW),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdDoBackupGo), "");
                        fields::stack(ui, |ui, st| {
                            if let Some(w) = who.as_deref() {
                                zikaron_ui::fields::rows(ui, |ui, f| {
                                    let _ = f.info(ui, t(Key::IdGroup), card::Show::Text(w));
                                    f.folder(ui, t(Key::IdStore), &me.ux.id_dir, t(Key::PickNone));
                                });
                                key_file_passwords(st, ui, me);
                            } else {
                                key_file_fields(st, ui, me);
                            }
                            pin_field(st, ui, t(Key::IdPinGate), None, "id-backup-pin", &mut me.ux.id_pin, me.ux.pin_shake, true);
                        });
                        show_trouble(ui);
                    },
                    |ui, me| {
                        let why = still_needs(&[(!Self::landing_ok(&me.ux.id_dir), Key::IdStore), (!pin_full, Key::IdPinGate)]);
                        if page::Page::new().primary_why(ui, t(Key::IdDoBackupGo), pin_full && !busy && Self::landing_ok(&me.ux.id_dir), &why).1.clicked() {
                            go = Some(Action::BackupKey { pin: std::mem::take(&mut me.ux.id_pin), password: me.ux.id_pw.clone(), again: me.ux.id_pw2.clone(), dir: me.ux.id_dir.clone() });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                        if busy {
                            busy_note(ui);
                        } else {
                            picked = foot_pick(ui, "id-backup-dir", crate::platform::Pick::Folder);
                        }
                    },
                );
                if let Some(p) = picked {
                    self.ux.id_dir = p;
                }
                esc = out.esc;
            }
            IdModal::Name(id) => {
                let had = reg.find(&id).map(|r| r.label.clone()).unwrap_or_default();
                let changed = self.ux.id_label.trim() != had;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-name", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdRenameTitle), "");
                        fields::stack(ui, |ui, st| {
                            st.field(ui, t(Key::IdLabel), None, None, |ui, w| fields::line(ui, &mut me.ux.id_label, t(Key::IdRenameHint), w, fields::Look::default()));
                        });
                        show_trouble(ui);
                    },
                    |ui, me| {
                        if page::Page::new().primary_why(ui, t(Key::IdDoRenameGo), changed, t(Key::IdRenameSame)).1.clicked() {
                            go = Some(Action::NameIdentity { id: id.clone(), label: me.ux.id_label.clone() });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                    },
                );
                esc = out.esc;
            }
            IdModal::Pin => {
                let old_full = self.ux.pin_old.chars() == len;
                let past_id = egui::Id::new("zikaron-id-pin-past-old");
                let past_old = old_full && ctx.data(|d| d.get_temp::<bool>(past_id)).unwrap_or(false);
                let again = !self.ux.pin_again.is_empty();
                let stage: u64 = if !past_old { 0 } else if !again { 1 } else { 2 };
                let new_full = self.ux.pin.chars() == len;
                let (mut next, mut change) = (false, None);
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-pin", tk::SHEET_NARROW).step(stage, sheet::Slide::Forward),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::ChangePinTitle), "");
                        fields::stack(ui, |ui, st| match stage {
                            0 => {
                                let _ = pin_field(st, ui, t(Key::PinOld), None, "set-pin-old", &mut me.ux.pin_old, me.ux.pin_shake, true);
                            }
                            1 => {
                                let why = me.ux.pin_why.filter(|_| me.ux.pin.is_empty());
                                let _ = pin_field_why(st, ui, t(Key::WizPinTitle), Some(t(Key::PinRules)), why, "set-pin-new", &mut me.ux.pin, me.ux.pin_shake, !busy);
                                if !me.ux.pin.is_empty() {
                                    me.ux.pin_why = None;
                                }
                            }
                            _ => {
                                let _ = pin_field(st, ui, t(Key::PinAgain), None, "set-pin-again", &mut me.ux.pin, me.ux.pin_shake, !busy);
                            }
                        });
                        show_trouble(ui);
                    },
                    |ui, me| {
                        let (word, ready, field) = match stage {
                            0 => (t(Key::WizardNext), old_full, Key::PinOld),
                            1 => (t(Key::WizardNext), new_full, Key::WizPinTitle),
                            _ => (t(Key::ChangePinTitle), new_full && !busy, Key::PinAgain),
                        };
                        let why = still_needs(&[(!ready, field)]);
                        if page::Page::new().primary_why(ui, word, ready, &why).1.clicked() {
                            match stage {
                                0 => next = true,
                                1 => first_pin = true,
                                _ => change = Some(Action::ChangePin { old: me.ux.pin_old.clone(), pin: me.ux.pin_again.clone(), again: std::mem::take(&mut me.ux.pin) }),
                            }
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                        busy_note(ui);
                    },
                );
                esc = out.esc;
                let old_now = self.ux.pin_old.chars() == len;
                let new_now = self.ux.pin.chars() == len;
                if stage == 0 && old_now {
                    next = true;
                }
                if stage == 1 && new_now && !busy && self.ux.pin_why.is_none() {
                    first_pin = true;
                }
                ctx.data_mut(|d| d.insert_temp(past_id, (past_old || next) && old_now));
                if let Some(a) = change {
                    let r = self.act(a, now);
                    self.vault_or(VaultSite::Change, r, now);
                }
            }
        }
        if first_pin {
            self.take_first_pin(now, PinSaid::Sheet);
        }
        if copied {
            self.ux.id_confirming = true;
            self.ux.id_words_open = false;
        }
        if ask_words {
            self.ux.id_trouble = None;
            let pin = std::mem::take(&mut self.ux.pin);
            let r = self.act(Action::RevealWords { pin }, now);
            self.vault_or(VaultSite::Words, r, now);
        }
        if let Some(a) = go {
            match self.own_hold(&a) {
                Some(f) => self.ux.id_trouble = Some(f),
                None => {
                    let r = self.act(a, now);
                    self.vault_or(VaultSite::Id, r, now);
                }
            }
        } else if close || esc {
            self.ux.id_close();
        }
    }
}

fn key_file_fields(st: &mut fields::Stack, ui: &mut egui::Ui, me: &mut Win) {
    zikaron_ui::fields::rows(ui, |ui, f| f.folder(ui, t(Key::IdStore), &me.ux.id_dir, t(Key::PickNone)));
    key_file_passwords(st, ui, me);
}

fn key_file_passwords(st: &mut fields::Stack, ui: &mut egui::Ui, me: &mut Win) {
    st.field(ui, t(Key::IdPassword), None, None, |ui, w| secret_strength(ui, "id-key-file-pw", &mut me.ux.id_pw, t(Key::IdPasswordHint), w));
    st.field(ui, t(Key::IdPasswordAgain), None, None, |ui, w| secret_again(ui, "id-key-file-pw2", &mut me.ux.id_pw2, &me.ux.id_pw, w));
}

pub(super) fn id_name(r: &crate::identity::Row) -> String {
    id_name_of(&r.label, crate::roles::Role::ALL.iter().find_map(|s| r.address(*s)))
}

fn id_name_of(label: &str, addr: Option<crate::key::Address>) -> String {
    if !label.trim().is_empty() {
        return label.to_string();
    }
    addr.map(|a| card::short(&a.hex())).unwrap_or_else(|| t(Key::Unnamed).to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FileBackup {
    There,
    NoPlace,
    Gone,
    Never,
}

pub(super) fn file_backup(b: &crate::identity::BackupSeen) -> FileBackup {
    match (b.at == crate::identity::NO_BACKUP_AT, b.marked, b.exists && b.opens) {
        (false, _, true) => FileBackup::There,
        (false, _, false) => FileBackup::Gone,
        (true, true, _) => FileBackup::NoPlace,
        (true, false, _) => FileBackup::Never,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unnamed_identity_is_said_by_its_address_cut_short() {
        let (a, b) = (crate::key::Address([0x11; 20]), crate::key::Address([0x22; 20]));
        assert_eq!(id_name_of("bench", Some(a)), "bench");
        assert_eq!(id_name_of("  ", Some(a)), card::short(&a.hex()));
        assert_ne!(id_name_of("", Some(a)), id_name_of("", Some(b)), "two unnamed identities told apart");
        assert_eq!(crate::lang::both_of(|| id_name_of("", None)), crate::lang::both_of(|| t(Key::Unnamed)));
    }

    #[test]
    fn a_key_files_backup_is_judged_once() {
        use crate::identity::{BackupSeen, NO_BACKUP_AT};
        let seen = |at: &str, marked: bool, there: bool| BackupSeen { at: at.to_string(), marked, exists: there, opens: there };
        assert_eq!(file_backup(&seen(NO_BACKUP_AT, true, false)), FileBackup::NoPlace, "imported from a key file");
        assert_eq!(file_backup(&seen(NO_BACKUP_AT, false, false)), FileBackup::Never);
        assert_eq!(file_backup(&seen("/k/key.json", true, true)), FileBackup::There);
        assert_eq!(file_backup(&seen("/k/key.json", true, false)), FileBackup::Gone);
        assert_eq!(file_backup(&BackupSeen { at: "/k/key.json".into(), marked: true, exists: true, opens: false }), FileBackup::Gone, "there but not opening as this key");
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ImportTab {
    Words,
    Key,
    File,
}

impl ImportTab {
    pub(super) fn word(self) -> Key {
        match self {
            ImportTab::Words => Key::IdTabWords,
            ImportTab::Key => Key::IdTabHex,
            ImportTab::File => Key::IdTabFile,
        }
    }
}
