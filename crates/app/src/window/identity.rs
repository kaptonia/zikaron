//! The identity sheets: new identity (the words, then three of them back), import a key (phrase, private
//! key, key file), show the phrase, switch identity, delete, back up the key, name it, change the passcode.
//! Toasts name identities by name, never by address.

use super::*;

impl Win {
    /// The network an identity made on these sheets takes: the one chosen here, else the one selected first
    /// (`machine::pick`). Inside the wizard the wizard's own network step decides, so its choice stands.
    pub(super) fn id_network_now(&self) -> String {
        if self.ux.wizard.is_some() {
            return self.wiz_network_pick();
        }
        self.ux.id_network.clone().unwrap_or_else(|| crate::machine::pick(&self.shell.machine))
    }

    /// The "network" row under the note: every row of the known table, then "custom" (`deploy::choices`). Not
    /// shown inside the wizard, whose network step comes next.
    fn id_network_row(&mut self, ui: &mut egui::Ui, salt: &str) {
        if self.ux.wizard.is_some() {
            return;
        }
        let now = self.id_network_now();
        let names = crate::deploy::choices();
        let labels: Vec<String> = names.iter().map(|n| network_label(n)).collect();
        let at = names.iter().position(|n| *n == now).unwrap_or(0);
        let items: Vec<menu::Item> = labels.iter().enumerate().map(|(i, l)| menu::Item::Row(menu::Row { label: l, check: Some(i == at), ..Default::default() })).collect();
        if let Some(i) = field(ui, t(Key::SetChain), None, |ui| menu::menu_key(ui, salt, &labels[at], false, 160.0, &items)) {
            self.ux.id_network = names.get(i).map(|n| n.to_string());
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
                states::err_box(ui, "id-trouble", f.human(), f.next(), t(Key::U3RawError), &f.raw());
            }
        };
        // A passcode task runs in the background: the sheet says it is verifying and its keys wait.
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault);
        let busy_note = |ui: &mut egui::Ui| {
            if busy {
                sheet::busy_note(ui, t(Key::VaultBusy), false);
            }
        };
        let len = crate::keybox::PIN_LEN;
        // Whether the current identity is the primary one (its words or key file recover the passcode).
        let is_primary = match (&self.shell.primary, &current) {
            (Some((p, _)), Some((r, _))) => p.eq_ignore_ascii_case(&r.id),
            _ => false,
        };
        let current_words = current.as_ref().map(|(r, _)| r.kind() == crate::identity::Kind::Words).unwrap_or(false);
        let (mut close, mut ask_words, mut copied, mut first_pin) = (false, false, false, false);
        let mut esc;
        let mut go: Option<Action> = None;
        match which {
            IdModal::New => {
                let fresh = self.shell.new_words.as_ref().map(|f| (f.words(), f.picks));
                let confirming = self.ux.id_confirming;
                let seen = self.ux.id_words_seen;
                let slide = if confirming { sheet::Slide::Forward } else { sheet::Slide::Back };
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-new", tk::SHEET_WIDE).step(u64::from(confirming), slide),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdNewTitle), "");
                        if !confirming {
                            // The label is kept through both steps and lands with the identity.
                            field(ui, t(Key::IdLabel), None, |ui| input::line(ui, &mut me.ux.id_new_label, t(Key::IdLabelHint)));
                            me.id_network_row(ui, "id-new-network");
                            paint::text(ui, t(Key::IdCopyWords), Type::Note, c(C::Ink2));
                            // Masked by default: twelve words in plain view reach anyone behind, screen
                            // recording and sharing. Click to show; hide again any time.
                            let words = fresh.as_ref().map(|(w, _)| w.clone());
                            let shown = if me.ux.id_words_open { words.as_deref() } else { None };
                            if pin::mask(ui, "id-new-words", shown, t(Key::IdShowNewWords), 3, 106.0).clicked() && !me.ux.id_words_open && words.is_some() {
                                me.ux.id_words_open = true;
                                me.ux.id_words_seen = true;
                            }
                            if me.ux.id_words_open && key::key(ui, t(Key::IdHideNewWords), Role::Secondary, true).clicked() {
                                me.ux.id_words_open = false;
                            }
                        } else {
                            paint::text(ui, t(Key::IdConfirmSay), Type::Note, c(C::Ink2));
                            if let Some((_, picks)) = fresh.as_ref() {
                                for (k, p) in picks.iter().enumerate() {
                                    field(ui, &fill1(Key::IdWordN, &(p + 1).to_string()), None, |ui| input::secret_line(ui, &mut me.ux.id_confirm[k], ""));
                                }
                            }
                        }
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
                        busy_note(ui);
                    },
                );
                esc = out.esc;
                if close || esc {
                    go = Some(Action::DropFresh);
                    (close, esc) = (false, false);
                }
            }
            IdModal::Import => {
                let tab = self.ux.id_tab;
                let into_seat = self.ux.id_seat.unwrap_or(seat);
                // No primary yet: this import makes it. A bare private key then lands its key file in the
                // same pass (the action layer refuses it without one), so the cells are asked here.
                let makes_primary = self.shell.primary.is_none();
                let ready = match tab {
                    0 => self.ux.id_words.iter().all(|w| !w.expose().trim().is_empty()),
                    1 => {
                        !self.ux.id_hex.expose().trim().is_empty()
                            && (!makes_primary || (!self.ux.id_pw.is_empty() && !self.ux.id_pw2.is_empty() && Self::landing_ok(&self.ux.id_dir)))
                    }
                    _ => !self.ux.id_ks_path.trim().is_empty(),
                };
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-import", tk::SHEET_WIDE).step(tab as u64, sheet::Slide::None),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdImportTitle), t(if makes_primary { Key::IdImportPrimarySay } else { Key::IdImportSay }));
                        if let Some(i) = seg::tabs(ui, "id-import-tabs", &[t(Key::IdTabWords), t(Key::IdTabHex), t(Key::IdTabFile)], tab) {
                            me.ux.id_tab = i;
                            me.ux.id_trouble = None;
                        }
                        match tab {
                            0 => {
                                field(ui, t(Key::IdTabWords), None, |ui| {
                                    // Twelve numbered cells: a word not in the list turns red at once; whether
                                    // the twelve form a phrase is said by name by the action layer.
                                    let bad = crate::cryptx::strangers(&me.ux.id_words);
                                    pin::words_grid_marked(ui, "id-import-words", &mut me.ux.id_words, &bad);
                                });
                                hint(ui, t(Key::PinWordsHint));
                                paint::text(ui, t(Key::IdNoAssets), Type::Note, c(C::Ink2));
                            }
                            1 => {
                                field(ui, t(Key::IdTabHex), None, |ui| input::secret_line(ui, &mut me.ux.id_hex, t(Key::IdHexHint)));
                                if makes_primary {
                                    paint::text(ui, t(Key::IdImportKeyFileSay), Type::Note, c(C::Ink2));
                                    key_file_cells(ui, me);
                                }
                            }
                            _ => {
                                let chosen = (!me.ux.id_ks_path.trim().is_empty()).then(|| width::file_name(&me.ux.id_ks_path));
                                let d = drop::zone(
                                    ui,
                                    "id-ks-drop",
                                    None,
                                    &[t(Key::IdFileDrop), t(Key::DropClickFile)],
                                    chosen.as_deref().map(|n| (n, "", t(Key::DropClickSwap))),
                                    72.0,
                                    drop::Shape::Column,
                                    true,
                                );
                                if let Some(p) = me.drop_or_pick(&d, crate::platform::Pick::File, now) {
                                    me.ux.id_ks_path = p;
                                }
                                field(ui, t(Key::IdFilePassword), None, |ui| input::secret_line(ui, &mut me.ux.id_ks_pw, ""));
                            }
                        }
                        // A private key or key file fills only the seat chosen here; a phrase fills both.
                        if tab != 0 {
                            field(ui, t(Key::IdImportSeat), None, |ui| {
                                let cells = [seg::Cell::from(t(Key::IdSeatAuthor)), seg::Cell::from(t(Key::IdSeatGrantee))];
                                if let Some(i) = seg::seg(ui, "id-import-seat", &cells, usize::from(into_seat == crate::roles::Role::Grantee)) {
                                    me.ux.id_seat = Some(if i == 0 { crate::roles::Role::Author } else { crate::roles::Role::Grantee });
                                }
                            });
                            hint(ui, t(Key::IdImportSeatSay));
                        }
                        field(ui, t(Key::IdLabel), None, |ui| input::line(ui, &mut me.ux.id_new_label, t(Key::IdLabelHint)));
                        me.id_network_row(ui, "id-import-network");
                        show_trouble(ui);
                    },
                    |ui, me| {
                        if page::Page::new().primary_with(ui, t(Key::IdDoImportGo), ready && !busy).1.clicked() {
                            let form = match me.ux.id_tab {
                                0 => ImportForm::Words(joined(&me.ux.id_words)),
                                1 => ImportForm::PrivateKey {
                                    key: me.ux.id_hex.clone(),
                                    keyfile: makes_primary.then(|| crate::action::KeyFileOut {
                                        password: me.ux.id_pw.clone(),
                                        again: me.ux.id_pw2.clone(),
                                        dir: me.ux.id_dir.clone(),
                                    }),
                                },
                                _ => ImportForm::Keystore { path: me.ux.id_ks_path.clone(), password: me.ux.id_ks_pw.clone() },
                            };
                            go = Some(Action::ImportIdentity { form, seat: into_seat, label: me.ux.id_new_label.clone(), network: me.id_network_now() });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                        busy_note(ui);
                    },
                );
                esc = out.esc;
            }
            IdModal::Words => {
                let has_words = current.as_ref().map(|(r, _)| r.kind() == crate::identity::Kind::Words).unwrap_or(false);
                let words = self.shell.words.clone();
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-words", tk::SHEET_W).step(u64::from(words.is_some()), sheet::Slide::None),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdWordsTitle), "");
                        states::err_box(ui, "id-words-warn", t(Key::IdWordsWarn), t(Key::IdWordsWarnNext), "", "");
                        if has_words {
                            // Shown only after the passcode.
                            match words.as_deref() {
                                None => {
                                    paint::text(ui, t(Key::IdWordsPinSay), Type::Note, c(C::Ink2));
                                    if pin::pin_row(ui, "id-words-pin", &mut me.ux.pin, len, me.ux.pin_shake, !busy, true).full && !busy {
                                        ask_words = true;
                                    }
                                }
                                Some(w) => {
                                    pin::mask(ui, "id-words-grid", Some(w), "", 3, 106.0);
                                    paint::text(ui, t(Key::IdNoAssets), Type::Note, c(C::Ink2));
                                    if is_primary {
                                        paint::text(ui, t(Key::IdPrimaryRecovers), Type::Note, c(C::Ink2));
                                    }
                                    paint::text(ui, t(Key::IdWordsMachineWide), Type::Note, c(C::Ink2));
                                }
                            }
                        } else {
                            paint::text(ui, t(Key::IdNoWordsHint), Type::Note, c(C::Ink2));
                        }
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
                let open = self.ux.id_switch_open.clone();
                let primary = self.shell.primary.as_ref().map(|(p, _)| p.clone());
                let mut toggle: Option<String> = None;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-switch", tk::SHEET_WIDE).step(motion::key_of(&open), sheet::Slide::None),
                    self,
                    |ui, _me| {
                        sheet::title(ui, t(Key::IdSwitchTitle), "");
                        ui.spacing_mut().item_spacing.y = tk::S2;
                        // Each identity is "kind · name"; clicking one opens it in place to show both seats'
                        // addresses and "switch". Only that key switches.
                        for r in &reg.rows {
                            let is_cur = current.as_ref().map(|(c, _)| c.id == r.id).unwrap_or(false);
                            let is_open = open.as_deref() == Some(r.id.as_str());
                            let name = if r.label.trim().is_empty() { t(Key::Unnamed).to_string() } else { r.label.clone() };
                            let head = format!("{} \u{b7} {}", t(id_kind_key(r.kind())), name);
                            // The whole card opens and closes it: the click is registered under the card's contents, so
                            // the "switch" key inside still takes its own click.
                            let whole = ui.scope_builder(egui::UiBuilder::new().sense(egui::Sense::click()), |ui| card::choice(ui, is_cur, |ui| {
                                width::then(
                                    ui,
                                    |ui| {
                                        if is_cur {
                                            mark::pill(ui, t(Key::IdIsCurrent), PillTone::Ok);
                                        } else if is_open && key::key(ui, t(Key::IdDoSwitchGo), Role::Secondary, true).clicked() {
                                            go = Some(Action::SwitchIdentity { id: r.id.clone() });
                                        }
                                        // The primary identity is marked here as in the identity menu.
                                        if primary.as_deref().map(|p| p.eq_ignore_ascii_case(&r.id)).unwrap_or(false) {
                                            mark::pill(ui, t(Key::IdPrimaryTag), PillTone::Blue);
                                        }
                                    },
                                    |ui, room| paint::line(ui, &head, Type::Body, c(C::Ink), room),
                                );
                                if is_open {
                                    let addr = |which: crate::roles::Role| r.address(which).map(|a| Val::mono(a.hex())).unwrap_or_else(|| Val::text(t(Key::IdSeatEmpty)));
                                    motion::swap(ui, egui::Id::new(("id-switch-open", &r.id)), 1, |ui| {
                                        kv::kv(ui, &[(t(Key::IdSeatRowAuthor), addr(crate::roles::Role::Author)), (t(Key::IdSeatRowGrantee), addr(crate::roles::Role::Grantee))]);
                                    });
                                }
                            }));
                            if whole.response.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                                toggle = Some(r.id.clone());
                            }
                        }
                        show_trouble(ui);
                    },
                    |ui, _me| {
                        close = key::key(ui, t(Key::IdDoClose), Role::Secondary, true).clicked();
                    },
                );
                esc = out.esc;
                if let Some(id) = toggle {
                    self.ux.id_switch_open = if open.as_deref() == Some(id.as_str()) { None } else { Some(id) };
                }
            }
            IdModal::Delete(id) if self.shell.primary.as_ref().map(|(p, _)| p.eq_ignore_ascii_case(&id)).unwrap_or(false) => {
                // The primary identity is not deleted directly: another one is made primary first.
                let others = reg.rows.iter().any(|r| !r.id.eq_ignore_ascii_case(&id));
                let (mut to_primary, mut to_new) = (false, false);
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-delete-primary", tk::SHEET_W),
                    self,
                    |ui, _me| {
                        sheet::title(ui, t(Key::PrimaryNoDeleteTitle), t(if others { Key::FaultNextPrimaryDelete } else { Key::PrimaryNoDeleteNew }));
                        show_trouble(ui);
                    },
                    |ui, _me| {
                        if others {
                            to_primary = key::key(ui, t(Key::PrimaryGoSet), Role::Secondary, true).clicked();
                        } else {
                            to_new = key::key(ui, t(Key::IdDoNew), Role::Secondary, true).clicked();
                        }
                        close = key::key(ui, t(Key::IdDoClose), Role::Secondary, true).clicked();
                    },
                );
                esc = out.esc;
                if to_primary {
                    // Switch to another identity and open its keys page, where "set as primary" is.
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
                let name = |r: &crate::identity::Row| if r.label.trim().is_empty() { t(Key::Unnamed).to_string() } else { r.label.clone() };
                let words = row.as_ref().map(|r| r.kind() == crate::identity::Kind::Words).unwrap_or(false);
                // An imported-key identity must have exported its key file first.
                let need_file = !words && !row.as_ref().map(|r| r.backed_file).unwrap_or(false);
                let pin_full = self.ux.id_pin.chars() == len;
                let rekeying = self.shell.rekeying;
                let mut export = false;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-primary", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::PrimaryTitle), "");
                        kv::kv(
                            ui,
                            &[
                                (t(Key::PrimaryNew), Val::text(row.as_ref().map(name).unwrap_or_default())),
                                (t(Key::PrimaryOld), Val::text(old.as_ref().map(|r| fill1(Key::PrimaryOldDemoted, &name(r))).unwrap_or_else(|| t(Key::IdNone).to_string()))),
                                (t(Key::PrimaryRecovers), Val::text(t(if words { Key::PrimaryByWords } else { Key::PrimaryByFile }))),
                            ],
                        );
                        if need_file {
                            states::err_box(ui, "id-primary-file", t(Key::PrimaryExportFirst), "", "", "");
                        }
                        field(ui, t(Key::IdPinGate), None, |ui| {
                            pin::pin_row(ui, "id-primary-pin", &mut me.ux.id_pin, len, me.ux.pin_shake, !need_file && !rekeying, true);
                        });
                        if rekeying {
                            sheet::foot_note(ui, t(Key::PrimaryResealing));
                        }
                        show_trouble(ui);
                    },
                    |ui, me| {
                        if page::Pen::new().press(ui, t(Key::PrimaryTitle), row.is_some() && !need_file && pin_full && !busy && !rekeying).clicked() {
                            go = Some(Action::SetPrimary { id: id.clone(), pin: std::mem::take(&mut me.ux.id_pin) });
                        }
                        if need_file {
                            export = key::key(ui, t(Key::IdDoBackup), Role::Secondary, true).clicked();
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, !rekeying).clicked();
                        busy_note(ui);
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
                let author = seat == crate::roles::Role::Author;
                // Both seats' ledgers and the backup file were read once when the sheet opened.
                let ledgers: Vec<&'static str> = self.ux.id_delete_ledgers.iter().map(|s| t(id_seat_row_key(*s))).collect();
                let seen = self.ux.id_delete_backup.clone();
                let pin_full = self.ux.id_pin.chars() == len;
                let addr = row.as_ref().and_then(|r| r.address(seat).or_else(|| r.address(r.first_seat()))).map(|a| a.hex()).unwrap_or_default();
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-delete", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdDeleteTitle), "");
                        kv::kv(
                            ui,
                            &[
                                (t(Key::IdDeleteEffect), Val::text(t(if author { Key::IdDeleteAuthorEffect } else { Key::IdDeleteGranteeEffect }))),
                                (
                                    t(Key::IdDeleteLedgers),
                                    if ledgers.is_empty() { Val::text(t(Key::IdDeleteLedgerNone)) } else { Val::Mark(Mark::Warn, fill1(Key::IdDeleteLedgerSome, &ledgers.join(" \u{b7} "))) },
                                ),
                            ],
                        );
                        if let Some(b) = &seen {
                            if b.marked && !(b.exists && b.opens) {
                                states::err_box(ui, "id-delete-backup", t(Key::IdBackupGone), t(Key::FaultNextBackupNotLanded), t(Key::U3RawError), &b.at);
                            }
                        }
                        states::note_box(ui, t(if author { Key::IdDeleteAuthorNote } else { Key::IdDeleteGranteeNote }));
                        details(ui, "id-delete-details", &[(t(Key::IdAddress), Val::mono(addr.clone()))]);
                        // The person is identified before keys go: the same passcode and failure count as at
                        // launch.
                        field(ui, t(Key::IdPinGate), None, |ui| {
                            pin::pin_row(ui, "id-delete-pin", &mut me.ux.id_pin, len, me.ux.pin_shake, true, true);
                        });
                        show_trouble(ui);
                    },
                    |ui, me| {
                        if page::Pen::new().press(ui, t(Key::IdDoDeleteGo), row.is_some() && pin_full && !busy).clicked() {
                            go = Some(Action::DeleteIdentity { id: id.clone(), pin: std::mem::take(&mut me.ux.id_pin) });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                        busy_note(ui);
                    },
                );
                esc = out.esc;
                if go.is_some() {
                    self.ux.id_deleting = row.map(|r| if r.label.trim().is_empty() { t(Key::Unnamed).to_string() } else { r.label.clone() });
                }
            }
            IdModal::Backup => {
                let pin_full = self.ux.id_pin.chars() == len;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-backup", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdDoBackupGo), t(Key::IdBackupSay));
                        // The local passcode first; then this key file's own password (each row says which).
                        field(ui, t(Key::IdPinGate), None, |ui| {
                            pin::pin_row(ui, "id-backup-pin", &mut me.ux.id_pin, len, me.ux.pin_shake, true, true);
                        });
                        key_file_cells(ui, me);
                        paint::text(ui, t(Key::IdBackupMachineWide), Type::Note, c(C::Ink2));
                        if is_primary && !current_words {
                            paint::text(ui, t(Key::IdPrimaryRecovers), Type::Note, c(C::Ink2));
                        }
                        show_trouble(ui);
                    },
                    |ui, me| {
                        if page::Page::new().primary_with(ui, t(Key::IdDoBackupGo), pin_full && !busy && Self::landing_ok(&me.ux.id_dir)).1.clicked() {
                            go = Some(Action::BackupKey { pin: std::mem::take(&mut me.ux.id_pin), password: me.ux.id_pw.clone(), again: me.ux.id_pw2.clone(), dir: me.ux.id_dir.clone() });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                        busy_note(ui);
                    },
                );
                esc = out.esc;
            }
            IdModal::Name(id) => {
                // Only a change enables the key; clearing the field clears the name, which is a change.
                let had = reg.find(&id).map(|r| r.label.clone()).unwrap_or_default();
                let changed = self.ux.id_label.trim() != had;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-name", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdRenameTitle), t(Key::IdRenameSay));
                        field(ui, t(Key::IdLabel), None, |ui| input::line(ui, &mut me.ux.id_label, t(Key::IdRenameHint)));
                        show_trouble(ui);
                    },
                    |ui, me| {
                        if page::Page::new().primary_with(ui, t(Key::IdDoRenameGo), changed).1.clicked() {
                            go = Some(Action::NameIdentity { id: id.clone(), label: me.ux.id_label.clone() });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                    },
                );
                esc = out.esc;
            }
            IdModal::Pin => {
                // The old passcode once, the new one twice; the row being typed in holds the focus.
                let old_full = self.ux.pin_old.chars() == len;
                let mut change: Option<Action> = None;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-pin", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::ChangePinTitle), t(Key::PinRules));
                        field(ui, t(Key::PinOld), None, |ui| {
                            pin::pin_row(ui, "set-pin-old", &mut me.ux.pin_old, len, None, !old_full, true);
                        });
                        let label = t(if me.ux.pin_again.is_empty() { Key::WizPinTitle } else { Key::PinAgain });
                        field(ui, label, None, |ui| {
                            let row = pin::pin_row(ui, "set-pin-new", &mut me.ux.pin, len, me.ux.pin_shake, old_full && !busy, true);
                            if row.full && me.ux.pin_again.is_empty() {
                                first_pin = true;
                            } else if row.full {
                                change = Some(Action::ChangePin { old: me.ux.pin_old.clone(), pin: me.ux.pin_again.clone(), again: std::mem::take(&mut me.ux.pin) });
                            }
                        });
                        show_trouble(ui);
                    },
                    |ui, _me| {
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                        busy_note(ui);
                    },
                );
                esc = out.esc;
                if let Some(a) = change {
                    let r = self.act(a, now);
                    self.vault_or(VaultSite::Change, r, now);
                }
            }
        }
        if first_pin {
            self.take_first_pin(now, false);
        }
        if copied {
            // Turning to the three cells: the words are masked again first.
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
            let r = self.act(a, now);
            self.vault_or(VaultSite::Id, r, now);
        } else if close || esc {
            self.ux.id_close();
        }
    }
}

/// A key file's cells: its password (with the strength reading, which warns and never blocks), the password
/// again, and the folder it lands in. Exporting a key file and importing a key that becomes primary show
/// these same cells.
fn key_file_cells(ui: &mut egui::Ui, me: &mut Win) {
    field(ui, t(Key::IdKeyFilePassword), None, |ui| {
        input::secret_line(ui, &mut me.ux.id_pw, "");
        if !me.ux.id_pw.is_empty() {
            let (lit, m, k) = match crate::keystore::strength(me.ux.id_pw.expose()) {
                crate::keystore::Strength::Weak => (1, Mark::Bad, Key::StrengthWeak),
                crate::keystore::Strength::Fair => (2, Mark::Warn, Key::StrengthFair),
                crate::keystore::Strength::Strong => (3, Mark::Ok, Key::StrengthStrong),
            };
            pin::strength(ui, lit, m, t(k));
        }
    });
    field(ui, t(Key::IdPasswordAgain), None, |ui| input::secret_line(ui, &mut me.ux.id_pw2, ""));
    Win::place_row(ui, Key::IdStore, &mut me.ux.id_dir);
}
