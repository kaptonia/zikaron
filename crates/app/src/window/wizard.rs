//! The first-run wizard: a full-window cover with six steps on the left (done ones checked, optional ones
//! dashed) and one card per step on the right that slides in by direction. A finished step shows a check
//! and "done" (what it made in a row of its own); the genesis confirmation sheet floats above. The left column's foot
//! holds the language and, once this machine has a passcode and an identity, the way out ("Exit setup", or
//! Esc): it returns to the seat and page the wizard was opened from; finished steps stay finished and the three
//! required ones stay required.

use super::*;
use zikaron_ui::covers;
use zikaron_ui::sheetparts as parts;

/// What a row inside a step's card asked for this frame (the identity step's three ways, the ledger step's
/// adoption).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WizRow {
    Generate,
    Import,
    Restore,
    Adopt,
}

impl Win {
    pub(super) fn wizard_layer(&mut self, ctx: &egui::Context, now: f64) {
        use crate::nav::Step;
        let Some(want) = self.ux.wizard else { return };
        let pr = self.progress();
        let role = self.shell.settings.role;
        // However the wizard reached this step, the step first passes `Progress::gate`.
        let step = pr.gate(want);
        self.ux.wizard = Some(step);
        let at = Step::ALL.iter().position(|s| *s == step).unwrap_or(0);
        let last = at + 1 == Step::ALL.len();
        let done = pr.done(step);
        // The card slides in from the side of the previous step.
        let dir_id = egui::Id::new("zikaron-wizard-dir");
        let (was, dir) = ctx.data(|d| d.get_temp::<(usize, f32)>(dir_id)).unwrap_or((at, 0.0));
        let dir = if was == at { dir } else if at > was { 1.0 } else { -1.0 };
        ctx.data_mut(|d| d.insert_temp(dir_id, (at, dir)));
        let (mut move_to, mut close, mut confirm, mut copied) = (None, false, false, false);
        let (mut open_export, mut exit) = (false, false);
        let mut row: Option<WizRow> = None;
        let exit_shown = self.wizard_exit_shown();
        let mut act: Option<Action> = None;
        let fresh = self.shell.new_words.as_ref().map(|f| (f.words(), f.picks));
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault);
        // This step's page and where "back" goes from it (`nav::prev`, the one place that decides).
        let sub = if step == Step::Key && fresh.is_some() && self.ux.id_confirming { crate::nav::Sub::Check } else { crate::nav::Sub::First };
        let back = crate::nav::prev(step, sub);
        let mut go_back = false;
        let mut lang_to: Option<crate::lang::Lang> = None;
        // The cover: the column of steps at the left on the rail's ground, the page ground at the right; the
        // step's card in a layer of its own above it (it slides from step to step).
        let mut main_rect = egui::Rect::NOTHING;
        let (_, over) = covers::cover(ctx, "wizard", covers::Ground::Clear, |ui| {
            let screen = ui.max_rect();
            let side = egui::Rect::from_min_size(screen.min, egui::vec2(zikaron_ui::tokens::RAIL_W, screen.height()));
            main_rect = egui::Rect::from_min_max(egui::pos2(side.right(), screen.top()), screen.max);
            covers::rail_ground(ui.painter(), side, 1.0);
            ui.painter().rect_filled(main_rect, 0.0, c(C::Ground));
            // The column: the title and the app's name under the window's buttons, the steps, and at its foot the
            // language and the way out (seen once a passcode and an identity are here; its room kept before).
            let inner = egui::Rect::from_min_max(egui::pos2(side.left() + covers::RAIL_PAD_X, side.top() + tk::RAIL_TOP), egui::pos2(side.right() - covers::RAIL_PAD_X, side.bottom() - 14.0));
            let mut left = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
            left.spacing_mut().item_spacing.y = 0.0;
            covers::rail_head(&mut left, t(Key::PageFirstRun), t(Key::AppName));
            let rows: Vec<covers::Step> = Step::ALL
                .iter()
                .enumerate()
                .map(|(i, s)| covers::Step { label: t(s.title()), done: pr.done(*s), reachable: i != at && (pr.done(*s) || i < at) })
                .collect();
            if let Some(i) = covers::steps(&mut left, &rows, at) {
                move_to = Some(i);
            }
            left.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 10.0;
                exit = covers::exit_item(ui, t(Key::WizExit), "esc", exit_shown).clicked();
                lang_to = lang_pick(ui, "wiz-lang");
            });
        });
        let dir_now = if was == at { 0.0 } else { dir };
        covers::pane(ctx, over, at as u64, dir_now, main_rect, |ui| {
            let main = ui.max_rect().shrink(32.0);
            full::centered(ui, "wizard-main", main, WIZ_CARD_W, 0.0, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                covers::step_head(ui, &fill2(Key::WizCount, &(at + 1).to_string(), &Step::ALL.len().to_string()), t(step.title()));
                // One card per step: its fields and rows, then its keys in the card's foot ("back" and "later" at the
                // left, the step's one main key at the right, "next" or "finish" once it is done).
                let open = self.ux.id_words_open;
                card::card_pad(ui, egui::vec2(24.0, 24.0), |ui| {
                    parts::bare(ui, |ui| {
                        fields::stack(ui, |ui, st| {
                            ui.spacing_mut().item_spacing.y = 18.0;
                            row = ui
                                .vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 18.0;
                                    self.wizard_body(ui, st, step, done, fresh.as_ref(), busy, now)
                                })
                                .inner;
                            let later = t(if last { Key::WizLaterDo } else { Key::WizLater });
                            let later_on = step.deferrable(role) && !done;
                            let words_shown = step == Step::Key && !done && fresh.is_some() && !self.ux.id_confirming;
                            let has_keys = done || step != Step::Pin || busy;
                            if !has_keys {
                                return;
                            }
                            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), tk::KEY_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.spacing_mut().item_spacing.x = tk::S2;
                                if done {
                                    if page::Page::new().primary(ui, t(if last { Key::WizFinish } else { Key::WizardNext })).1.clicked() {
                                        // The network row was changed after choosing: "next" changes the choice.
                                        if step == Step::Network && self.wiz_network_chosen().as_deref() != Some(self.wiz_network_pick().as_str()) {
                                            act = Some(Action::ChooseNetwork { name: self.wiz_network_pick() });
                                        }
                                        if last {
                                            close = true;
                                        } else {
                                            move_to = Some(at + 1);
                                        }
                                    }
                                } else {
                                    match step {
                                        Step::Pin => {}
                                        // The three ways to an identity are rows in the card; once words are made, the
                                        // step's key moves them on.
                                        Step::Key => match (fresh.as_ref(), self.ux.id_confirming) {
                                            (None, _) => {}
                                            (Some(_), false) => copied = page::Page::new().primary_with(ui, t(Key::IdDoCopied), self.ux.id_words_seen).1.clicked(),
                                            (Some((_, picks)), true) => {
                                                if page::Page::new().primary_with(ui, t(Key::IdDoConfirm), !busy).1.clicked() {
                                                    let answers = picks.iter().zip(self.ux.id_confirm.iter()).map(|(p, w)| (*p, w.clone())).collect();
                                                    act = Some(Action::ConfirmIdentity { answers, label: self.ux.id_new_label.clone(), network: self.wiz_network_pick() });
                                                }
                                            }
                                        },
                                        // The default row is already selected: "next" makes it this identity's network.
                                        Step::Network => {
                                            if page::Page::new().primary(ui, t(Key::WizardNext)).1.clicked() {
                                                act = Some(Action::ChooseNetwork { name: self.wiz_network_pick() });
                                                move_to = Some(at + 1);
                                            }
                                        }
                                        // Creating the ledger opens its confirmation; adopting one that exists is a row
                                        // in the card.
                                        Step::Genesis => confirm = page::Guide::key(ui, t(Key::WizGenesisTitle), self.shell.home.is_some()).clicked(),
                                        Step::Gas => {
                                            if self.long_key(ui, t(Key::WizSent), Role::Primary, true, crate::task::Kind::Chain) {
                                                act = Some(Action::ReadChain);
                                                self.ux.wiz_gas_asked = Some(self.shell.tasks.landings(crate::task::Kind::Chain));
                                                self.ux.wiz_gas_seen = None;
                                                self.ux.wiz_gas_not_seen = false;
                                            }
                                        }
                                        Step::Backup => open_export = page::Page::new().primary_with(ui, t(Key::BackupExportTitle), self.shell.unlocked()).1.clicked(),
                                    }
                                }
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.spacing_mut().item_spacing.x = tk::S2;
                                    // With the words shown, "hide" takes the place of "back".
                                    if words_shown && open {
                                        if key::key(ui, t(Key::IdHideNewWords), Role::Secondary, true).clicked() {
                                            self.ux.id_words_open = false;
                                        }
                                    } else if back.is_some() && key::key(ui, t(Key::WizPrev), Role::Secondary, true).clicked() {
                                        go_back = true;
                                    }
                                    if later_on && key::key(ui, later, Role::Secondary, true).clicked() {
                                        if last {
                                            close = true;
                                        } else {
                                            move_to = Some(at + 1);
                                        }
                                    }
                                    if busy && step != Step::Pin {
                                        sheet::busy_note(ui, t(Key::VaultBusy), false);
                                    }
                                });
                            });
                        });
                    });
                });
            });
        });
        if confirm {
            self.ux.confirm_genesis = true;
        }
        match row {
            Some(WizRow::Generate) => act = Some(Action::NewIdentity),
            // The wizard's own sheet; the wizard stays under it.
            Some(WizRow::Import) => self.ux.id_open(IdModal::Import),
            Some(WizRow::Restore) => self.bk_open(Bk::Restore(RestoreFrom::FirstRun)),
            Some(WizRow::Adopt) => self.ux.confirm_adopt = true,
            None => {}
        }
        if let Some(lang) = lang_to {
            let _ = self.act(Action::SetLang { lang }, now);
        }
        // "Back" lands where `nav::prev` says: the words' check goes back to the same words (the check cells
        // emptied), anything else to the step before.
        if let (true, Some((to, to_sub))) = (go_back, back) {
            if to == step {
                self.ux.id_confirming = to_sub == crate::nav::Sub::Check;
                for c in self.ux.id_confirm.iter_mut() {
                    *c = crate::secret::Secret::default();
                }
            } else {
                move_to = Step::ALL.iter().position(|s| *s == to);
            }
        }
        if copied {
            // Moving on to the three check cells: mask the words again first.
            self.ux.id_confirming = true;
            self.ux.id_words_open = false;
        }
        if open_export {
            self.bk_open(Bk::Export);
        }
        // The way out (the key, or Esc with nothing opened over the wizard): new words shown but not yet checked
        // trigger a warning first. A menu, list or sheet opened over the wizard takes Esc first.
        let esc = exit_shown && zikaron_ui::layer::esc(ctx, egui::Id::new("zikaron-wizard")) && !self.any_sheet_open();
        // The Esc that opens the question must not also close it in the same frame.
        let mut asked_now = false;
        if (exit || esc) && !self.ux.wiz_exit_ask {
            if self.shell.new_words.is_some() {
                self.ux.wiz_exit_ask = true;
                asked_now = true;
            } else {
                self.wizard_exit(now);
                return;
            }
        }
        if self.ux.wiz_exit_ask {
            // Going on with the check is the main key; leaving discards the words.
            let (mut keep, mut leave) = (false, false);
            let out = sheet::show(
                ctx,
                sheet::Spec::new("wiz-exit", tk::SHEET_W),
                self,
                |ui, _me| {
                    sheet::title(ui, t(Key::WizExitTitle), "");
                },
                |ui, _me| {
                    keep = page::Page::new().primary(ui, t(Key::WizExitKeep)).1.clicked();
                    leave = key::key(ui, t(Key::WizExitGo), Role::Secondary, true).clicked();
                },
            );
            if keep || (out.esc && !asked_now) {
                self.ux.wiz_exit_ask = false;
            } else if leave {
                self.wizard_exit(now);
                return;
            }
        }
        // The gas step after "I have sent it": once its chain reading lands, a balance shows the step done for a
        // second and then moves on; no balance keeps the step and says the chain does not show it yet.
        let answered = matches!(self.ux.wiz_gas_asked, Some(stamp) if self.shell.tasks.answered_since(crate::task::Kind::Chain, stamp));
        if step != Step::Gas {
            self.ux.wiz_gas_asked = None;
            self.ux.wiz_gas_seen = None;
            self.ux.wiz_gas_not_seen = false;
        } else if answered {
            if done {
                let seen = *self.ux.wiz_gas_seen.get_or_insert(now);
                if now - seen >= GAS_SEEN_SECS {
                    self.ux.wiz_gas_asked = None;
                    self.ux.wiz_gas_seen = None;
                    move_to = move_to.or(Some(at + 1));
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(GAS_SEEN_SECS - (now - seen)));
                }
            } else {
                self.ux.wiz_gas_asked = None;
                self.ux.wiz_gas_not_seen = matches!(self.chain_read(), Ok(Some(Done::Chain { gas_wei: Some(0), .. })));
            }
        }
        if let Some(a) = act {
            let was_pin = matches!(a, Action::SetPin { .. });
            let r = self.act(a, now);
            if was_pin {
                self.vault_or(VaultSite::Wizard, r, now);
            }
        }
        if close {
            self.ux.wizard = None;
            self.toasts.say(t(Key::WizFinished), Tone::Note, now);
        } else if let Some(i) = move_to {
            self.ux.wizard = Step::ALL.get(i).copied();
        }
    }

    /// One step's fields and rows in its card; the row asked for this frame, if any.
    #[allow(clippy::too_many_arguments)]
    fn wizard_body(&mut self, ui: &mut egui::Ui, st: &mut fields::Stack, step: crate::nav::Step, done: bool, fresh: Option<&(Vec<crate::secret::Secret>, [usize; 3])>, busy: bool, now: f64) -> Option<WizRow> {
        use crate::nav::Step;
        let len = crate::keybox::PIN_LEN;
        // A finished step (other than network and balance, which stay readable) is drawn as a check and "done",
        // with what it made in a row of its own: the key's address, the ledger's place, the backup file (copied
        // from the row's end).
        if done && !matches!(step, Step::Network | Step::Gas) {
            parts::done(ui, &format!("wizard-done-{}", step as u8), t(Key::Done));
            let made = match step {
                Step::Key => self.shell.anchor.map(|a| (Key::IdAddress, a.hex())),
                Step::Genesis => self.shell.home.as_ref().map(|h| (Key::CfWhere, h.root().display().to_string())),
                Step::Backup => self.shell.machine.backup.as_ref().map(|b| (Key::DataBackupFile, b.path.clone())),
                _ => None,
            };
            if let Some((k, v)) = made {
                let copied = zikaron_ui::fields::rows(ui, |ui, f| f.info(ui, t(k), card::Show::Copy(&v)));
                tell_copied(ui.ctx(), copied);
            }
            return None;
        }
        let mut row = None;
        match step {
            // One row filled twice, in the middle of the card: the first entry's shape is checked at once (its rules
            // under the cells), the second, named so, completes it.
            Step::Pin => {
                let again = !self.ux.pin_again.is_empty();
                let full = ui
                    .vertical_centered(|ui| {
                        ui.spacing_mut().item_spacing.y = 8.0;
                        if again {
                            fields::name(ui, t(Key::PinAgain), None);
                        }
                        let r = parts::pins(ui, "wiz-pin", &mut self.ux.pin, len, self.ux.pin_shake, !busy);
                        if !again && !busy {
                            paint::text(ui, t(Key::PinRules), Type::Small, c(C::Ink2));
                        }
                        r
                    })
                    .inner
                    .full;
                if full && !again {
                    self.take_first_pin(now, false);
                } else if full {
                    let a = Action::SetPin { pin: self.ux.pin_again.clone(), again: std::mem::take(&mut self.ux.pin) };
                    let r = self.act(a, now);
                    self.vault_or(VaultSite::Wizard, r, now);
                }
                if busy {
                    sheet::busy_note(ui, t(Key::VaultBusy), true);
                }
            }
            Step::Key => match fresh {
                // The optional label, then the three ways to an identity as rows.
                None => {
                    st.field(ui, t(Key::IdLabel), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut self.ux.id_new_label, t(Key::IdLabelExample), w, fields::Look::default()));
                    zikaron_ui::fields::rows(ui, |ui, f| {
                        f.tag_next(t(Key::WizNetRecommended));
                        if f.go(ui, t(Key::WizDoGenerate), "", true, "").clicked() {
                            row = Some(WizRow::Generate);
                        }
                        if f.go(ui, t(Key::IdDoImportExisting), "", true, "").clicked() {
                            row = Some(WizRow::Import);
                        }
                        if f.go(ui, t(Key::BackupRestoreTitle), "", !busy, "").clicked() {
                            row = Some(WizRow::Restore);
                        }
                    });
                }
                // The twelve words in three columns under a veil until clicked; hidden again from the card's foot.
                Some((words, _)) if !self.ux.id_confirming => {
                    if parts::words(ui, "wiz-words", Some(words.as_slice()), self.ux.id_words_open, t(Key::IdShowNewWords)).clicked() {
                        self.ux.id_words_open = true;
                        self.ux.id_words_seen = true;
                    }
                }
                Some((_, picks)) => {
                    for (k, p) in picks.iter().enumerate() {
                        st.field(ui, &fill1(Key::IdWordN, &(p + 1).to_string()), None, None, |ui, w| input::secret_line_w_marked(ui, &mut self.ux.id_confirm[k], "", w, false));
                    }
                }
            },
            Step::Network => {
                let picked = self.wiz_network_pick();
                // The wizard offers the default network and a custom one; another known network (the testnet) is
                // reached through the custom row, and shows here only when this machine already uses it.
                let mut rows: Vec<(String, String, bool)> = crate::deploy::KNOWN
                    .iter()
                    .filter(|d| d.name == crate::deploy::DEFAULT || d.name == picked)
                    .map(|d| (d.name.to_string(), t(d.label).to_string(), d.name == crate::deploy::DEFAULT))
                    .collect();
                rows.push((crate::deploy::CUSTOM.to_string(), t(Key::U3Custom).to_string(), false));
                zikaron_ui::fields::rows(ui, |ui, f| {
                    for (name, title, recommended) in rows {
                        let on = picked == name;
                        if recommended {
                            f.tag_next(t(Key::WizNetRecommended));
                        }
                        let (resp, _) = f.line(ui, None, &title, "", false, false, c(C::Ink), |ui| parts::radio(ui, &format!("wiz-network-{name}"), on));
                        // The whole row chooses it.
                        let hit = ui.interact(resp.rect, egui::Id::new(("wiz-network", name.as_str())), egui::Sense::click());
                        if hit.clicked() {
                            self.ux.wiz_network = Some(name);
                        }
                        let _ = hit.on_hover_cursor(egui::CursorIcon::PointingHand);
                    }
                });
            }
            // What the ledger will say (optional), and adopting one that exists, as a row: offered while this place
            // has no ledger, which is while this step is not done (the same adoption as Settings > Local data).
            Step::Genesis => {
                st.field(ui, t(Key::CfWhat), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut self.ux.wiz_statement, t(Key::StatementHint), w, fields::Look::default()));
                zikaron_ui::fields::rows(ui, |ui, f| {
                    if f.go(ui, t(Key::AdoptGroup), "", self.shell.home.is_some(), "").clicked() {
                        row = Some(WizRow::Adopt);
                    }
                });
            }
            Step::Gas => {
                let gas = match self.chain_read() {
                    Err(f) => (Mark::Bad, fill1(Key::SetReadFailedNow, f.human())),
                    Ok(Some(Done::Chain { gas_wei: Some(w), .. })) => (if *w > 0 { Mark::Ok } else { Mark::Bad }, fill1(Key::SetGasSay, &eth_held(*w))),
                    _ => (Mark::Todo, t(Key::SetNotRead).to_string()),
                };
                let addr = self.shell.anchor.map(|a| a.hex());
                // No code until the chain is known: a code without it lets a wallet pay on whichever network it is on.
                let code = addr.as_deref().zip(self.shell.settings.chain_id).map(|(a, chain)| gas_request(a, chain));
                let (copied, _) = gas_face(ui, addr.as_deref(), code.as_deref(), &gas);
                tell_copied(ui.ctx(), copied);
                self.stage_line(ui, crate::task::Kind::Chain);
                if self.ux.wiz_gas_not_seen {
                    hint(ui, t(Key::WizGasNotSeen));
                }
            }
            // What the backup holds, in one row.
            Step::Backup => {
                let content = self.data_content();
                zikaron_ui::fields::rows(ui, |ui, f| {
                    super::backup::content_row(f, ui, &content);
                });
            }
        }
        row
    }

    /// Create the ledger: what it will say, that it is free (written locally), where it will be, in one group;
    /// that it is permanent, in a call box. Floats above the wizard.
    pub(super) fn genesis_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.confirm_genesis {
            return;
        }
        let root = self.shell.home.as_ref().map(|h| h.root().display().to_string()).unwrap_or_default();
        let blank = self.ux.wiz_statement.trim().is_empty();
        let said = if blank { t(Key::CfBlank).to_string() } else { self.ux.wiz_statement.clone() };
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("create-ledger", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, t(Key::WizGenesisTitle), "");
                fields::stack(ui, |ui, _st| {
                    let copied = zikaron_ui::fields::rows(ui, |ui, f| {
                        let _ = f.info(ui, t(Key::CfWhat), if blank { card::Show::Empty(&said) } else { card::Show::Text(&said) });
                        let _ = f.info(ui, t(Key::CfCost), card::Show::Text(t(Key::CfCostLocal)));
                        f.info(ui, t(Key::CfWhere), if root.is_empty() { card::Show::Empty(t(Key::PickNone)) } else { card::Show::Copy(&root) })
                    });
                    tell_copied(ui.ctx(), copied);
                });
            },
            |ui, _me| {
                go = page::Pen::new().press(ui, t(Key::WizGenesisTitle), true).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.ux.confirm_genesis = false;
        } else if go {
            self.ux.confirm_genesis = false;
            self.act(Action::Genesis { statement: self.ux.wiz_statement.clone() }, now);
        }
    }

    /// Adopt an existing ledger at the step that creates one: a ledger folder or a ledger mirror, through the same
    /// adoption as Settings > Local data (`Action::Adopt`: its entries checked one by one and sealed into this data
    /// folder; another ledger's, or entries that are not one whole ledger, refused by name). Adopted, this place has
    /// its root and the step stands done, as after creating the ledger. The folder in one row (the documents folder
    /// until one is chosen at the bottom left). Floats above the wizard.
    pub(super) fn wizard_adopt_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.confirm_adopt {
            return;
        }
        if self.ux.wiz_adopt.trim().is_empty() {
            self.ux.wiz_adopt = self.shell.documents.clone();
        }
        let (mut go, mut close, mut picked) = (false, false, None);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("wizard-adopt", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::AdoptGroup), "");
                zikaron_ui::fields::rows(ui, |ui, f| f.folder(ui, t(Key::WizAdoptFolder), &me.ux.wiz_adopt, t(Key::PickNone)));
            },
            |ui, me| {
                go = page::Pen::new().press(ui, t(Key::DoAdopt), !me.ux.wiz_adopt.trim().is_empty()).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                picked = foot_pick(ui, "wizard-adopt", crate::platform::Pick::Folder);
            },
        );
        if let Some(p) = picked {
            self.ux.wiz_adopt = p;
        }
        if close || out.esc {
            self.ux.confirm_adopt = false;
        } else if go {
            self.ux.confirm_adopt = false;
            self.act(Action::Adopt { dir: self.ux.wiz_adopt.trim().to_string() }, now);
        }
    }
}

/// Entry kinds in plain words on lists: genesis is "create the ledger", history "record", succession
/// "change key or hand over"; others as on the ledger.
pub(super) fn user_kind_key(k: zikaron::tokens::EntryType) -> Key {
    match k {
        zikaron::tokens::EntryType::Genesis => Key::WizGenesisTitle,
        zikaron::tokens::EntryType::History => Key::TagAnchor,
        zikaron::tokens::EntryType::Succession => Key::TagHandover,
        other => kind_key(other),
    }
}

/// How long the gas step stays shown as done before moving on by itself (seconds).
const GAS_SEEN_SECS: f64 = 1.0;
/// Every step's card: one width, the gas step's code beside its rows included.
const WIZ_CARD_W: f32 = 520.0;
/// The code's plate (its margin included), and the gap between the rows and the plate.
const QR_PLATE: f32 = 112.0;
const GAS_GAP: f32 = 20.0;

/// The rows' least width beside the code; narrower, the code goes under them.
const GAS_ROWS_MIN: f32 = 300.0;

/// The key's address (written short, copied whole from its row's end) and balance (with its mark) in one group at
/// the left; the code a wallet scans to pay (`code`, the payment request; none while the chain is not known) on its
/// plate at the right, both centred on one middle line. When the card is too narrow for both side by side (its
/// width is the window's, never one measured from this face), the code stands centred under the rows. Returns the
/// words copied and where the rows lie.
fn gas_face(ui: &mut egui::Ui, addr: Option<&str>, code: Option<&str>, gas: &(Mark, String)) -> (Option<String>, egui::Rect) {
    let room = ui.available_width();
    // No code (the chain is not known): the rows take the whole card, nothing is kept for a plate.
    let side = code.is_some() && room - GAS_GAP - QR_PLATE >= GAS_ROWS_MIN;
    let rows_w = if side { room - GAS_GAP - QR_PLATE } else { room };
    let rows = |ui: &mut egui::Ui| {
        ui.allocate_ui_with_layout(egui::vec2(rows_w, if side { QR_PLATE } else { 0.0 }), egui::Layout::top_down(egui::Align::Min).with_main_align(egui::Align::Center), |ui| {
            ui.set_width(rows_w);
            let copied = zikaron_ui::fields::rows(ui, |ui, f| {
                let got = match addr {
                    Some(a) => f.info(ui, t(Key::IdAddress), card::Show::Short(a)),
                    None => f.info(ui, t(Key::IdAddress), card::Show::Empty(t(Key::SetNoKey))),
                };
                let _ = f.info(ui, t(Key::IdGas), card::Show::Mark(gas.0, &gas.1));
                got
            });
            (copied, ui.min_rect())
        })
        .inner
    };
    let plate = |ui: &mut egui::Ui| match code {
        Some(code) => {
            let at = qr_plate(ui, code);
            let _ = zikaron_ui::layer::tip(ui.interact(at, egui::Id::new("zikaron-gas-code"), egui::Sense::hover()), t(Key::WizGasScan));
        }
        None => {
            ui.add_space(QR_PLATE);
        }
    };
    if side {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAS_GAP;
            let r = rows(ui);
            plate(ui);
            r
        })
        .inner
    } else {
        let r = rows(ui);
        if code.is_some() {
            ui.vertical_centered(|ui| plate(ui));
        }
        r
    }
}

/// A field's box `w` wide as one cell of the caller's row, under its own stable identity (`salt`), so what is
/// drawn over its right end lands inside it.
fn boxed_cell<R>(ui: &mut egui::Ui, salt: &str, w: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> (R, egui::Rect) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, tk::INPUT_H), egui::Sense::hover());
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(("zikaron-boxed", salt)).max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    (add(&mut child), rect)
}

/// The words at the right end inside a field's box: laid on the field's own ground, so a long entry passes
/// under them.
fn inside_end<R>(ui: &mut egui::Ui, frame: egui::Rect, w: f32, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let zone = egui::Rect::from_min_max(egui::pos2(frame.right() - tk::INPUT_PAD_X - w, frame.top() + 1.0), egui::pos2(frame.right() - 1.0, frame.bottom() - 1.0));
    ui.painter().rect_filled(zone, egui::CornerRadius { nw: 0, sw: 0, ne: 7, se: 7 }, c(C::Surface));
    let inner = egui::Rect::from_min_max(egui::pos2(zone.left() + tk::S2, zone.top()), egui::pos2(zone.right() - tk::INPUT_PAD_X + 1.0, zone.bottom()));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Center)));
    add(&mut child)
}

/// A new password for a file (a key file, a backup): masked, `hint` in it while empty (the rule it must meet),
/// its strength at the right end inside the box once something is typed. The strength warns and never blocks.
pub(super) fn secret_strength(ui: &mut egui::Ui, salt: &str, text: &mut crate::secret::Secret, hint: &str, w: f32) -> egui::Response {
    let (r, frame) = boxed_cell(ui, salt, w, |ui| input::secret_line_w_marked(ui, text, hint, w, false));
    if !text.is_empty() {
        let (lit, m, k) = match crate::keystore::strength(text.expose()) {
            crate::keystore::Strength::Weak => (1, Mark::Bad, Key::StrengthWeak),
            crate::keystore::Strength::Fair => (2, Mark::Warn, Key::StrengthFair),
            crate::keystore::Strength::Strong => (3, Mark::Ok, Key::StrengthStrong),
        };
        // Three segments 34 wide, 4 apart, 4 before the reading (the kit's strength bar).
        let bar = 3.0 * (34.0 + 4.0) + 4.0;
        let words = paint::galley(ui, t(k), Type::Tiny, c(C::Ink2)).size().x;
        inside_end(ui, frame, bar + words + tk::S2, |ui| pin::strength(ui, lit, m, t(k)));
    }
    r
}

/// The same password again: masked, a check at the right end inside the box once it matches the first.
pub(super) fn secret_again(ui: &mut egui::Ui, salt: &str, text: &mut crate::secret::Secret, first: &crate::secret::Secret, w: f32) -> egui::Response {
    let (r, frame) = boxed_cell(ui, salt, w, |ui| input::secret_line_w_marked(ui, text, "", w, false));
    if !text.is_empty() && text.expose() == first.expose() {
        let at = egui::pos2(frame.right() - tk::INPUT_PAD_X - tk::MARK / 2.0, frame.center().y);
        mark::paint_mark(ui.ctx(), ui.painter(), at, Mark::Ok, tk::MARK);
    }
    r
}

/// A code a wallet scans, on its plate (the widget library's QR painter), the plate 112 across. The code is
/// computed once per text and cached. Returns where the plate lies.
fn qr_plate(ui: &mut egui::Ui, text: &str) -> egui::Rect {
    let id = egui::Id::new(("zikaron-qr", text));
    let code = ui.ctx().data_mut(|d| d.get_temp::<Option<std::sync::Arc<crate::qr::Code>>>(id)).flatten().or_else(|| {
        let c = crate::qr::encode(text.as_bytes()).map(std::sync::Arc::new);
        ui.ctx().data_mut(|d| d.insert_temp(id, c.clone()));
        c
    });
    let side = QR_PLATE - 2.0 * paint::QR_PAD;
    match code {
        Some(code) => paint::qr(ui, &code.modules, side),
        None => ui.allocate_exact_size(egui::vec2(QR_PLATE, QR_PLATE), egui::Sense::hover()).0,
    }
}

/// A passcode field of a sheet or a step's card: its name above, the cells at a sheet's size under it, and the
/// line under them when there is one (red when `bad`).
#[allow(clippy::too_many_arguments)]
pub(super) fn pin_field(st: &mut fields::Stack, ui: &mut egui::Ui, label: &str, line: Option<(&str, bool)>, salt: &str, value: &mut crate::secret::Secret, shake: Option<f64>, active: bool) -> pin::PinRow {
    st.field(ui, label, None, line, |ui, _| parts::pins(ui, salt, value, crate::keybox::PIN_LEN, shake, active))
}

/// A file in one row of a sheet's group: a file glyph, the name, and the file chosen (its name, whole on hover)
/// or "none selected" in the quiet ink. The file is chosen at the sheet's bottom left.
pub(super) fn file_row(f: &mut card::Form, ui: &mut egui::Ui, label: &str, path: &str) {
    let room = (ui.available_width() * 0.55).max(40.0);
    let _ = f.line(ui, Some(card::Lead::Glyph(Glyph::File)), label, "", false, false, c(C::Ink), |ui| {
        if path.trim().is_empty() {
            paint::line(ui, t(Key::PickNone), Type::Note, c(C::Ink3), room);
        } else {
            let name = width::file_name(path);
            let r = paint::line(ui, &name, Type::Note, c(C::Ink), room);
            let _ = zikaron_ui::layer::tip(r, path);
        }
    });
}

/// A key at the left end of a sheet's foot ("back", "hide"): drawn after the keys at the right. Whether it was
/// pressed.
pub(super) fn foot_left(ui: &mut egui::Ui, text: &str) -> bool {
    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| key::key(ui, text, Role::Secondary, true).clicked()).inner
}

/// The payment request the gas step's code carries (EIP-681): this address on the chain of the network the
/// wizard chose (`ethereum:<address>@<chain id>`), so a wallet set to another network asks to switch rather than
/// paying on that one.
fn gas_request(address: &str, chain_id: u64) -> String {
    format!("ethereum:{address}@{chain_id}")
}

#[cfg(test)]
mod gas_address {
    /// The code names the address and the chain, in EIP-681's form: a wallet on another network is asked to
    /// switch to this one, never left to pay on its own.
    #[test]
    fn the_gas_code_names_the_chosen_networks_chain() {
        let a = "0x5d5a0de8ac3b41b649e0985c141d1735832cbf8f";
        assert_eq!(super::gas_request(a, 11_155_111), "ethereum:0x5d5a0de8ac3b41b649e0985c141d1735832cbf8f@11155111");
        assert_eq!(super::gas_request(a, 1), "ethereum:0x5d5a0de8ac3b41b649e0985c141d1735832cbf8f@1");
    }

    use super::*;

    const ADDR: &str = "0x5d5a0de8ac3b41b649e0985c141d1735832cbf8f";

    fn frame(ctx: &egui::Context, events: Vec<egui::Event>, list: &std::cell::Cell<egui::Rect>, copied: &std::cell::RefCell<Option<String>>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 760.0))),
            events,
            ..Default::default()
        };
        ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show_inside(ui, |ui| {
                let (c, r) = gas_face(ui, Some(ADDR), Some(&gas_request(ADDR, 11_155_111)), &(Mark::Ok, "0.0000 ETH".to_string()));
                list.set(r);
                if c.is_some() {
                    *copied.borrow_mut() = c;
                }
            });
        })
    }

    /// The address stands in its row written short, on one line; the copy key at the end of its row copies exactly
    /// the address.
    #[test]
    fn copies_the_address_whole_from_its_row() {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.0);
        // Styled like the window (skin and fonts) so the address is laid out in the real faces.
        zikaron_ui::skin::dress(&ctx);
        let list = std::cell::Cell::new(egui::Rect::NOTHING);
        let copied = std::cell::RefCell::new(None);
        frame(&ctx, Vec::new(), &list, &copied);
        let out = frame(&ctx, Vec::new(), &list, &copied);
        let shown = out
            .shapes
            .iter()
            .find_map(|c| match &c.shape {
                egui::Shape::Text(t) if t.galley.text().starts_with(&ADDR[..6]) && t.galley.text().ends_with(&ADDR[ADDR.len() - 4..]) => Some((t.visual_bounding_rect(), t.galley.clone())),
                _ => None,
            })
            .expect("the address is drawn in its row");
        assert_eq!(shown.1.rows.len(), 1, "one line");
        let y = shown.0.center().y;
        // The copy key: right after the address, 8 on (its 19-point box).
        let key = egui::pos2(shown.0.right() + 8.0 + 9.5, y);
        assert!(list.get().contains(shown.0.left_center()), "the address stands in the rows' column");
        let m = egui::Modifiers::NONE;
        frame(&ctx, vec![egui::Event::PointerMoved(key)], &list, &copied);
        frame(&ctx, vec![egui::Event::PointerButton { pos: key, button: egui::PointerButton::Primary, pressed: true, modifiers: m }], &list, &copied);
        let out = frame(&ctx, vec![egui::Event::PointerButton { pos: key, button: egui::PointerButton::Primary, pressed: false, modifiers: m }], &list, &copied);
        let text: Vec<String> = out
            .platform_output
            .commands
            .iter()
            .filter_map(|c| match c {
                egui::OutputCommand::CopyText(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(text, vec![ADDR.to_string()], "copied exactly the address");
        assert_eq!(copied.borrow().as_deref(), Some(ADDR), "and said so to the window");
    }
}
