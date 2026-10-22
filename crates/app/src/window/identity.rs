//! The identity sheets: new identity (the words, then three of them back), import a key (phrase, private
//! key, key file), show the phrase, switch identity, delete, back up the key, name it, change the passcode.
//! Toasts name identities by name, never by address.

use super::*;
use zikaron_ui::sheetparts as parts;

impl Win {
    /// The network for an identity made on these sheets: the one chosen here, else the first selected
    /// (`machine::pick`). Inside the wizard, the wizard's own network step decides.
    pub(super) fn id_network_now(&self) -> String {
        if self.ux.wizard.is_some() {
            return self.wiz_network_pick();
        }
        self.ux.id_network.clone().unwrap_or_else(|| crate::machine::pick(&self.shell.machine))
    }

    /// The "network" row: every known network, then "custom" (`deploy::choices`), in a menu at the row's right
    /// end. Not shown inside the wizard, whose network step comes next.
    fn id_network_row(&mut self, f: &mut card::Form, ui: &mut egui::Ui, salt: &str) {
        if self.ux.wizard.is_some() {
            return;
        }
        let now = self.id_network_now();
        let names = crate::deploy::choices();
        let labels: Vec<String> = names.iter().map(|n| network_label(n)).collect();
        let at = names.iter().position(|n| *n == now).unwrap_or(0);
        let items: Vec<menu::Item> = labels.iter().enumerate().map(|(i, l)| menu::Item::Row(menu::Row { label: l, check: Some(i == at), ..Default::default() })).collect();
        let mut picked = None;
        f.row_with(ui, None, t(Key::SetChain), "", card::Value::None, |ui| picked = menu::menu_key(ui, salt, &labels[at], false, 160.0, &items));
        if let Some(i) = picked {
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
                states::err_box(ui, "id-trouble", f.human(), &f.next(), t(Key::U3RawError), &f.raw());
            }
        };
        // While a passcode task runs in the background, the sheet says it is verifying and its keys wait.
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault);
        let busy_note = |ui: &mut egui::Ui| {
            if busy {
                sheet::busy_note(ui, t(Key::VaultBusy), false);
            }
        };
        let len = crate::keybox::PIN_LEN;
        let name_of = |r: &crate::identity::Row| if r.label.trim().is_empty() { t(Key::Unnamed).to_string() } else { r.label.clone() };
        let (mut close, mut ask_words, mut copied, mut first_pin) = (false, false, false, false);
        let mut esc;
        let mut go: Option<Action> = None;
        match which {
            // The label, the network and the twelve words under a veil; then three of them back. Hiding the
            // words again, and going back from the check, stand at the bottom left.
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
                                // The label is kept through both steps and lands with the identity.
                                st.field(ui, t(Key::IdLabel), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut me.ux.id_new_label, t(Key::IdLabelExample), w, fields::Look::default()));
                                if me.ux.wizard.is_none() {
                                    zikaron_ui::fields::rows(ui, |ui, f| me.id_network_row(f, ui, "id-new-network"));
                                }
                                // Under a veil by default: twelve words in plain view reach anyone behind the screen,
                                // and screen recording and sharing. A click on the veil shows them. No words yet (being
                                // made, or refused, said under the form): no empty grid holding their place.
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
                    // Back to the words: the check cells start empty again.
                    self.ux.id_confirming = false;
                    self.ux.id_confirm = Default::default();
                }
                if close || esc {
                    go = Some(Action::DropFresh);
                    (close, esc) = (false, false);
                }
            }
            // The tabs beside the title; each tab's own fields (a key file's place, and the key file itself, chosen
            // at the bottom left); which identity it fills; then the label and network.
            IdModal::Import => {
                let tab = self.ux.id_tab;
                let into_seat = self.ux.id_seat.unwrap_or(seat);
                // No primary yet, so this import makes it. A bare private key must then write its key file in the
                // same pass (the action layer refuses it otherwise), so the key file cells are asked here.
                let makes_primary = self.shell.primary.is_none();
                if tab == 1 && makes_primary && self.ux.id_dir.trim().is_empty() {
                    self.ux.id_dir = self.shell.documents.clone();
                }
                let ready = match tab {
                    0 => self.ux.id_words.iter().all(|w| !w.expose().trim().is_empty()),
                    1 => {
                        !self.ux.id_hex.expose().trim().is_empty()
                            && (!makes_primary || (!self.ux.id_pw.is_empty() && !self.ux.id_pw2.is_empty() && Self::landing_ok(&self.ux.id_dir)))
                    }
                    _ => !self.ux.id_ks_path.trim().is_empty(),
                };
                let mut dropped: Option<drop::Drop> = None;
                let mut picked = None;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-import", tk::SHEET_WIDE).step(tab as u64, sheet::Slide::None),
                    self,
                    |ui, me| {
                        if let Some(i) = parts::title_seg(ui, t(Key::IdImportTitle), "id-import-tabs", &[t(Key::IdTabWords), t(Key::IdTabHex), t(Key::IdTabFile)], tab) {
                            me.ux.id_tab = i;
                            me.ux.id_trouble = None;
                        }
                        // A private key or key file fills only the seat chosen here; a phrase fills both.
                        let seat_row = |ui: &mut egui::Ui, me: &mut Win| {
                            let cells = [seg::Cell::from(t(Key::IdSeatAuthor)), seg::Cell::from(t(Key::IdSeatGrantee))];
                            let mut pick = None;
                            zikaron_ui::fields::rows(ui, |ui, f| pick = f.with(ui, t(Key::IdImportSeat), "", |ui| seg::seg(ui, "id-import-seat", &cells, usize::from(into_seat == crate::roles::Role::Grantee))));
                            if let Some(i) = pick {
                                me.ux.id_seat = Some(if i == 0 { crate::roles::Role::Author } else { crate::roles::Role::Grantee });
                            }
                        };
                        fields::stack(ui, |ui, st| {
                            match tab {
                                0 => {
                                    st.field(ui, t(Key::IdTabWords), None, None, |ui, _| {
                                        // Twelve numbered cells; a word not in the list turns red at once. The action
                                        // layer reports whether the twelve form a valid phrase.
                                        let bad = crate::cryptx::strangers(&me.ux.id_words);
                                        pin::words_grid_marked(ui, "id-import-words", &mut me.ux.id_words, &bad);
                                    });
                                }
                                1 => {
                                    st.field(ui, t(Key::IdTabHex), None, None, |ui, w| input::secret_key_line(ui, &mut me.ux.id_hex, t(Key::IdHexHint), w, false));
                                    seat_row(ui, me);
                                    if makes_primary {
                                        key_file_fields(st, ui, me);
                                    }
                                }
                                _ => {
                                    // A key file dropped on the sheet is taken as chosen.
                                    dropped = Some(drop::Drop { dropped: drop::take_dropped(ui.ctx()), hovering: false, clicked: false });
                                    zikaron_ui::fields::rows(ui, |ui, f| file_row(f, ui, t(Key::IdTabFile), &me.ux.id_ks_path));
                                    st.field(ui, t(Key::IdFilePassword), None, None, |ui, w| input::secret_line_w_marked(ui, &mut me.ux.id_ks_pw, "", w, false));
                                    seat_row(ui, me);
                                }
                            }
                            st.field(ui, t(Key::IdLabel), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut me.ux.id_new_label, t(Key::IdLabelExample), w, fields::Look::default()));
                            if me.ux.wizard.is_none() {
                                zikaron_ui::fields::rows(ui, |ui, f| me.id_network_row(f, ui, "id-import-network"));
                            }
                        });
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
                        if busy {
                            busy_note(ui);
                        } else if tab == 1 && makes_primary {
                            picked = foot_pick(ui, "id-import-dir", crate::platform::Pick::Folder);
                        } else if tab == 2 {
                            picked = foot_pick(ui, "id-import-file", crate::platform::Pick::File);
                        }
                    },
                );
                esc = out.esc;
                if let Some(p) = dropped.as_ref().and_then(|d| self.one_drop(d, now)) {
                    self.ux.id_ks_path = p;
                }
                if let Some(p) = picked {
                    if tab == 2 {
                        self.ux.id_ks_path = p;
                    } else {
                        self.ux.id_dir = p;
                    }
                }
            }
            // What holding the words means, in a call box; the passcode, then the words.
            IdModal::Words => {
                let has_words = current.as_ref().map(|(r, _)| r.kind() == crate::identity::Kind::Words).unwrap_or(false);
                let words = self.shell.words.clone();
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-words", tk::SHEET_W).step(u64::from(words.is_some()), sheet::Slide::None),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdWordsTitle), "");
                        fields::stack(ui, |ui, st| {
                            if !has_words {
                                zikaron_ui::callout::callout(ui, t(Key::IdNoWordsHint));
                                return;
                            }
                            // Shown only after the passcode.
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
            // One row per identity: its name, its kind (and "primary") under it, and its own "switch" at the right
            // (the one in use says so); every identity's two addresses under details, copied from their rows.
            IdModal::Switch => {
                let primary = self.shell.primary.as_ref().map(|(p, _)| p.clone());
                let mut details_open = self.ux.id_switch_open.is_some();
                let mut addrs: Vec<(String, card::Detail)> = Vec::new();
                for r in &reg.rows {
                    for (which, label) in [(crate::roles::Role::Author, Key::IdSeatRowAuthor), (crate::roles::Role::Grantee, Key::IdSeatRowGrantee)] {
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
                            // Every identity's two addresses under details (held open by the switcher's state).
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
                // The primary identity cannot be deleted directly: another one must be made primary first.
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
            // What changes, in one group; a key file to export first, in a call box; the passcode last. The words
            // while it reseals stand in the foot.
            IdModal::Primary(id) => {
                let row = reg.find(&id).cloned();
                let old = self.shell.primary.as_ref().and_then(|(p, _)| reg.find(p)).cloned();
                let words = row.as_ref().map(|r| r.kind() == crate::identity::Kind::Words).unwrap_or(false);
                // An imported-key identity must have exported its key file first.
                let need_file = !words && !row.as_ref().map(|r| r.backed_file).unwrap_or(false);
                let pin_full = self.ux.id_pin.chars() == len;
                let rekeying = self.shell.rekeying;
                let new_name = row.as_ref().map(name_of).unwrap_or_default();
                let old_said = old.as_ref().map(|r| fill1(Key::PrimaryOldDemoted, &name_of(r))).unwrap_or_else(|| t(Key::IdNone).to_string());
                let mut export = false;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-primary", tk::SHEET_W),
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
                        if page::Pen::new().press(ui, t(Key::PrimaryTitle), row.is_some() && !need_file && pin_full && !busy && !rekeying).clicked() {
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
            // Which identity and what deleting it does, in one group; what to do first, in a call box; the
            // passcode last; the key that deletes, with its red edge.
            IdModal::Delete(id) => {
                let row = reg.find(&id).cloned();
                let author = seat == crate::roles::Role::Author;
                // Both seats' ledgers and the backup file were read once, when the sheet opened.
                let ledgers: Vec<&'static str> = self.ux.id_delete_ledgers.iter().map(|s| t(id_seat_row_key(*s))).collect();
                let seen = self.ux.id_delete_backup.clone();
                let pin_full = self.ux.id_pin.chars() == len;
                let addr = row.as_ref().and_then(|r| r.address(seat).or_else(|| r.address(r.first_seat()))).map(|a| a.hex()).unwrap_or_default();
                let name = row.as_ref().map(name_of).unwrap_or_default();
                let effect = t(if author { Key::IdDeleteAuthorEffect } else { Key::IdDeleteGranteeEffect });
                let ledgers_say = if ledgers.is_empty() { None } else { Some(fill1(Key::IdDeleteLedgerSome, &ledgers.join(" \u{b7} "))) };
                let mut delete = false;
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-delete", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::IdDeleteTitle), "");
                        fields::stack(ui, |ui, st| {
                            let mut copied = None;
                            zikaron_ui::fields::rows(ui, |ui, f| {
                                let _ = f.info(ui, t(Key::IdGroup), card::Show::Text(&name));
                                copied = f.info(ui, t(Key::IdAddress), card::Show::Short(&addr));
                                let _ = f.info(ui, t(Key::IdDeleteEffect), card::Show::Text(effect));
                                let _ = match &ledgers_say {
                                    Some(v) => f.info(ui, t(Key::IdDeleteLedgers), card::Show::Mark(Mark::Warn, v)),
                                    None => f.info(ui, t(Key::IdDeleteLedgers), card::Show::Text(t(Key::IdDeleteLedgerNone))),
                                };
                            });
                            tell_copied(ui.ctx(), copied);
                            if let Some(b) = &seen {
                                if b.marked && !(b.exists && b.opens) {
                                    states::err_box(ui, "id-delete-backup", t(Key::IdBackupGone), t(Key::FaultNextBackupNotLanded), t(Key::U3RawError), &b.at);
                                }
                            }
                            // The user is identified before keys are deleted, with the same passcode and failure count
                            // as at launch.
                            pin_field(st, ui, t(Key::IdPinGate), None, "id-delete-pin", &mut me.ux.id_pin, me.ux.pin_shake, true);
                        });
                        show_trouble(ui);
                    },
                    |ui, _me| {
                        delete = page::Erase::press(ui, t(Key::IdDoDeleteGo), row.is_some() && pin_full && !busy).clicked();
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
            // Which identity, and where the key file goes (chosen at the bottom left), one group; its password
            // twice; the passcode last.
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
                    sheet::Spec::new("id-backup", tk::SHEET_W),
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
                        if page::Page::new().primary_with(ui, t(Key::IdDoBackupGo), pin_full && !busy && Self::landing_ok(&me.ux.id_dir)).1.clicked() {
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
            // The name in one field; only a change enables the key.
            IdModal::Name(id) => {
                // Clearing the field clears the name, which counts as a change.
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
                        if page::Page::new().primary_with(ui, t(Key::IdDoRenameGo), changed).1.clicked() {
                            go = Some(Action::NameIdentity { id: id.clone(), label: me.ux.id_label.clone() });
                        }
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                    },
                );
                esc = out.esc;
            }
            IdModal::Pin => {
                // One field at a time: the old passcode, the new one (its rules under it), the new one again; the
                // key at the right moves on once the row is whole and changes it at the last.
                let old_full = self.ux.pin_old.chars() == len;
                let past_id = egui::Id::new("zikaron-id-pin-past-old");
                let past_old = old_full && ctx.data(|d| d.get_temp::<bool>(past_id)).unwrap_or(false);
                let again = !self.ux.pin_again.is_empty();
                let stage: u64 = if !past_old { 0 } else if !again { 1 } else { 2 };
                let new_full = self.ux.pin.chars() == len;
                let (mut next, mut change) = (false, None);
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("id-pin", tk::SHEET_W).step(stage, sheet::Slide::Forward),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::ChangePinTitle), "");
                        fields::stack(ui, |ui, st| match stage {
                            0 => {
                                let _ = pin_field(st, ui, t(Key::PinOld), None, "set-pin-old", &mut me.ux.pin_old, me.ux.pin_shake, true);
                            }
                            1 => {
                                let _ = pin_field(st, ui, t(Key::WizPinTitle), Some((t(Key::PinRules), false)), "set-pin-new", &mut me.ux.pin, me.ux.pin_shake, !busy);
                            }
                            _ => {
                                let _ = pin_field(st, ui, t(Key::PinAgain), None, "set-pin-again", &mut me.ux.pin, me.ux.pin_shake, !busy);
                            }
                        });
                        show_trouble(ui);
                    },
                    |ui, me| {
                        let (word, ready) = match stage {
                            0 => (t(Key::WizardNext), old_full),
                            1 => (t(Key::WizardNext), new_full),
                            _ => (t(Key::ChangePinTitle), new_full && !busy),
                        };
                        if page::Page::new().primary_with(ui, word, ready).1.clicked() {
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
                // Past the old passcode only by the key; a refused change empties it and starts there again.
                ctx.data_mut(|d| d.insert_temp(past_id, (past_old || next) && old_full));
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
            // Moving on to the three check cells: mask the words again first.
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

/// A key file's fields: where it is saved (one row, chosen at the sheet's bottom left), its password (with a
/// strength reading inside its box that warns but never blocks) and the password again (a check once it matches).
/// Shared by key file export and by importing a key that becomes primary.
fn key_file_fields(st: &mut fields::Stack, ui: &mut egui::Ui, me: &mut Win) {
    zikaron_ui::fields::rows(ui, |ui, f| f.folder(ui, t(Key::IdStore), &me.ux.id_dir, t(Key::PickNone)));
    key_file_passwords(st, ui, me);
}

/// A key file's password and the password again.
fn key_file_passwords(st: &mut fields::Stack, ui: &mut egui::Ui, me: &mut Win) {
    st.field(ui, t(Key::IdPassword), None, None, |ui, w| secret_strength(ui, "id-key-file-pw", &mut me.ux.id_pw, t(Key::IdPasswordHint), w));
    st.field(ui, t(Key::IdPasswordAgain), None, None, |ui, w| secret_again(ui, "id-key-file-pw2", &mut me.ux.id_pw2, &me.ux.id_pw, w));
}
