//! The first-run wizard: a full-window cover, six steps on the left (done ones checked, those that can wait
//! dashed), one card per step on the right that slides in by direction. A finished step shows the drawn
//! check and "done" (with its value under details); the genesis confirmation sheet floats above. A way out
//! stays at the foot of the steps (and Esc) except on the true first run: it returns to the seat and page the
//! wizard was opened from; finished steps stay finished and the three required ones stay required.

use super::*;

impl Win {
    pub(super) fn wizard_layer(&mut self, ctx: &egui::Context, now: f64) {
        use crate::nav::Step;
        let Some(want) = self.ux.wizard else { return };
        let pr = self.progress();
        let role = self.shell.settings.role;
        // Whatever turned the wizard to whatever step, the step first passes `Progress::gate`.
        let step = pr.gate(want);
        self.ux.wizard = Some(step);
        let at = Step::ALL.iter().position(|s| *s == step).unwrap_or(0);
        let last = at + 1 == Step::ALL.len();
        let done = pr.done(step);
        // Which way the card slides: by where the last step was.
        let dir_id = egui::Id::new("zikaron-wizard-dir");
        let (was, dir) = ctx.data(|d| d.get_temp::<(usize, f32)>(dir_id)).unwrap_or((at, 0.0));
        let dir = if was == at { dir } else if at > was { 1.0 } else { -1.0 };
        ctx.data_mut(|d| d.insert_temp(dir_id, (at, dir)));
        let (mut move_to, mut close, mut confirm, mut copied, mut open_import) = (None, false, false, false, false);
        let (mut open_restore, mut open_export, mut exit) = (false, false, false);
        let exit_shown = self.wizard_exit_shown();
        let mut act: Option<Action> = None;
        let fresh = self.shell.new_words.as_ref().map(|f| (f.words(), f.picks));
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault);
        full::cover(ctx, "wizard", |ui, drop| {
            let screen = ui.max_rect();
            let side = egui::Rect::from_min_size(screen.min, egui::vec2(260.0, screen.height()));
            ui.painter().rect_filled(side, 0.0, c(C::Rail));
            ui.painter().vline(side.right(), side.y_range(), egui::Stroke::new(1.0_f32, c(C::Line2)));
            // The left column: the steps.
            let mut left = ui.new_child(egui::UiBuilder::new().max_rect(side.shrink2(egui::vec2(22.0, 0.0))).layout(egui::Layout::top_down(egui::Align::Min)));
            left.add_space(64.0);
            left.spacing_mut().item_spacing.y = 2.0;
            paint::text(&mut left, t(Key::PageFirstRun), Type::Card, c(C::Ink));
            paint::text(&mut left, t(Key::AppName), Type::Small, c(C::Ink3));
            left.add_space(20.0);
            let rows: Vec<full::Step> = Step::ALL
                .iter()
                .enumerate()
                .map(|(i, s)| full::Step { label: t(s.title()), done: pr.done(*s), later: s.deferrable(role), reachable: i != at && (pr.done(*s) || i < at) })
                .collect();
            if let Some(i) = full::steps(&mut left, &rows, at) {
                move_to = Some(i);
            }
            // The way out, under the steps (not on the true first run).
            if exit_shown {
                left.add_space(24.0);
                exit = key::key(&mut left, t(Key::WizExit), Role::Plain, true).clicked();
            }
            // The right: this step's card.
            let main = egui::Rect::from_min_max(egui::pos2(side.right(), screen.top()), screen.max).shrink(40.0);
            // The gas step's card holds the key's address and a code side by side: its column is as wide as
            // they need (never narrower than the other steps').
            let col = if step == Step::Gas { gas_card_w(ui.ctx()).max(520.0) } else { 520.0 };
            // Side by side when the column the window leaves holds the gas card whole: decided here from the
            // window alone, never from a width measured while drawing (which the layout itself could move).
            let gas_side = full::centered_w(main, col) + 0.5 >= gas_card_w(ui.ctx());
            full::centered(ui, "wizard-main", main, col, drop, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                paint::text(ui, &fill2(Key::WizCount, &(at + 1).to_string(), &Step::ALL.len().to_string()), Type::Small, c(C::Ink3));
                paint::text(ui, t(step.title()), Type::Page, c(C::Ink));
                ui.add_space(16.0);
                let pad = if step == Step::Gas { egui::vec2(GAS_PAD, GAS_PAD) } else { egui::vec2(tk::CARD_PAD_X, tk::CARD_PAD_Y) };
                card::card_pad(ui, pad, |ui| {
                    let p = motion::enter(ui.ctx(), egui::Id::new("zikaron-wizard-step"), at as u64, 0.0, tk::STEP, motion::Curve::Ease);
                    motion::shifted(ui, egui::vec2(24.0 * dir * (1.0 - p), 0.0), p, |ui| {
                        ui.spacing_mut().item_spacing.y = tk::S3;
                        self.wizard_body(ui, step, done, fresh.as_ref(), busy, gas_side, now);
                    });
                });
                ui.add_space(16.0);
                // The bottom row: "back" and "later" on the left; the step's own keys on the right, replaced by
                // "next" ("finish" on the last step) once it is done.
                let w = ui.available_width();
                ui.allocate_ui_with_layout(egui::vec2(w, tk::KEY_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = tk::S2;
                    if done {
                        if page::Page::new().primary(ui, t(if last { Key::WizFinish } else { Key::WizardNext })).1.clicked() {
                            // The network row was changed after choosing: next changes the choice.
                            if step == Step::Network && self.shell.machine.network.as_deref() != Some(self.wiz_network_pick().as_str()) {
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
                            Step::Key => match (fresh.as_ref(), self.ux.id_confirming) {
                                (None, _) => {
                                    if page::Page::new().primary(ui, t(Key::WizDoGenerate)).1.clicked() {
                                        act = Some(Action::NewIdentity);
                                    }
                                    open_import = key::key(ui, t(Key::IdDoImportExisting), Role::Secondary, true).clicked();
                                    open_restore = key::key(ui, t(Key::DoRestoreBackup), Role::Secondary, !busy).clicked();
                                }
                                (Some(_), false) => {
                                    copied = page::Page::new().primary_with(ui, t(Key::IdDoCopied), self.ux.id_words_seen).1.clicked();
                                    if self.ux.id_words_open && key::key(ui, t(Key::IdHideNewWords), Role::Secondary, true).clicked() {
                                        self.ux.id_words_open = false;
                                    }
                                }
                                (Some((_, picks)), true) => {
                                    if page::Page::new().primary_with(ui, t(Key::IdDoConfirm), !busy).1.clicked() {
                                        let answers = picks.iter().zip(self.ux.id_confirm.iter()).map(|(p, w)| (*p, w.clone())).collect();
                                        act = Some(Action::ConfirmIdentity { answers, label: self.ux.id_new_label.clone() });
                                    }
                                    if busy {
                                        paint::text(ui, t(Key::VaultBusy), Type::Small, c(C::Ink3));
                                    }
                                }
                            },
                            // The default row is chosen already: next writes it to this Mac's settings.
                            Step::Network => {
                                if page::Page::new().primary(ui, t(Key::WizardNext)).1.clicked() {
                                    act = Some(Action::ChooseNetwork { name: self.wiz_network_pick() });
                                    move_to = Some(at + 1);
                                }
                            }
                            Step::Genesis => confirm = page::Guide::key(ui, t(Key::WizGenesisTitle), self.shell.home.is_some()).clicked(),
                            Step::Gas => {
                                if self.long_key(ui, t(Key::WizSent), Role::Primary, true, crate::task::Kind::Chain) {
                                    act = Some(Action::ReadChain);
                                    self.ux.wiz_gas_asked = Some(self.shell.tasks.landings(crate::task::Kind::Chain));
                                    self.ux.wiz_gas_seen = None;
                                    self.ux.wiz_gas_not_seen = false;
                                }
                            }
                            Step::Backup => open_export = page::Page::new().primary_with(ui, t(Key::DoExportBackup), self.shell.unlocked()).1.clicked(),
                        }
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = tk::S2;
                        if at > 0 && key::key(ui, t(Key::WizPrev), Role::Secondary, true).clicked() {
                            move_to = Some(at - 1);
                        }
                        if step.deferrable(role) && !done && key::key(ui, t(if last { Key::WizLaterDo } else { Key::WizLater }), Role::Plain, true).clicked() {
                            if last {
                                close = true;
                            } else {
                                move_to = Some(at + 1);
                            }
                        }
                    });
                });
            });
        });
        if confirm {
            self.ux.confirm_genesis = true;
        }
        if copied {
            // Turning to the three cells: the words are masked again first.
            self.ux.id_confirming = true;
            self.ux.id_words_open = false;
        }
        if open_import {
            // The wizard's own sheet: the wizard stays under it.
            self.ux.id_open(IdModal::Import);
        }
        if open_restore {
            self.bk_open(Bk::Restore(RestoreFrom::FirstRun));
        }
        if open_export {
            self.bk_open(Bk::Export);
        }
        // The way out (the key, or Esc with no sheet up): new words shown and not yet checked are said first.
        let esc = exit_shown && ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !self.any_sheet_open();
        // The Esc that asks is not also the Esc that closes the ask, in the same frame.
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
            let (mut keep, mut leave) = (false, false);
            let out = sheet::show(
                ctx,
                sheet::Spec::new("wiz-exit", tk::SHEET_W),
                self,
                |ui, _me| {
                    sheet::title(ui, t(Key::WizExitTitle), "");
                    states::note_box(ui, t(Key::WizExitWords));
                },
                |ui, _me| {
                    leave = key::key(ui, t(Key::WizExitGo), Role::Secondary, true).clicked();
                    keep = key::key(ui, t(Key::WizExitKeep), Role::Secondary, true).clicked();
                },
            );
            if keep || (out.esc && !asked_now) {
                self.ux.wiz_exit_ask = false;
            } else if leave {
                self.wizard_exit(now);
                return;
            }
        }
        // The gas step after "I have sent it": once its reading of the chain has landed, a balance shows the
        // step done for a second and then goes on; none keeps the step and says the chain does not show it yet.
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
                self.ux.wiz_gas_not_seen = matches!(&self.shell.chain, Some(Done::Chain { gas_wei: Some(0), .. }));
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

    /// One step's card.
    #[allow(clippy::too_many_arguments)]
    fn wizard_body(&mut self, ui: &mut egui::Ui, step: crate::nav::Step, done: bool, fresh: Option<&(Vec<String>, [usize; 3])>, busy: bool, gas_side: bool, now: f64) {
        use crate::nav::Step;
        let len = crate::keybox::PIN_LEN;
        // A finished step (other than the network and the balance, which stay readable) is the drawn check.
        if done && !matches!(step, Step::Network | Step::Gas) {
            states::done_state(ui, &format!("wizard-done-{}", step as u8), t(Key::Done), "");
            match step {
                Step::Key => {
                    if let Some(a) = self.shell.anchor {
                        details(ui, "wizard-key-details", &[(t(Key::IdAddress), Val::mono(a.hex()))]);
                    }
                }
                Step::Backup => {
                    if let Some(b) = &self.shell.machine.backup {
                        details(ui, "wizard-backup-details", &[(t(Key::DataBackupFile), Val::mono(b.path.clone()))]);
                    }
                }
                _ => {}
            }
            return;
        }
        match step {
            Step::Pin => {
                ui.vertical_centered(|ui| {
                    ui.add_space(4.0);
                    // One row filled twice: the first fill's shape is judged at once, the second completes it.
                    let full = pin::pin_row(ui, "wiz-pin", &mut self.ux.pin, len, self.ux.pin_shake, !busy, false).full;
                    if full && self.ux.pin_again.is_empty() {
                        self.take_first_pin(now, false);
                    } else if full {
                        let a = Action::SetPin { pin: self.ux.pin_again.clone(), again: std::mem::take(&mut self.ux.pin) };
                        let r = self.act(a, now);
                        self.vault_or(VaultSite::Wizard, r, now);
                    }
                    ui.add_space(8.0);
                    let lines: Vec<&str> = if busy {
                        vec![t(Key::VaultBusy)]
                    } else if self.ux.pin_again.is_empty() {
                        vec![t(Key::PinRules), t(Key::WizPinSay)]
                    } else {
                        vec![t(Key::PinAgain)]
                    };
                    for l in lines {
                        paint::text(ui, l, Type::Note, c(C::Ink2));
                    }
                });
            }
            Step::Key => match fresh {
                None => {
                    // The label (optional) is asked only here, before the words.
                    field(ui, t(Key::IdLabel), None, |ui| input::line(ui, &mut self.ux.id_new_label, t(Key::IdLabelHint)));
                    ui.vertical_centered(|ui| {
                        ui.add_space(14.0);
                        paint::text(ui, t(Key::WizKeySay), Type::Note, c(C::Ink2));
                    });
                }
                Some((words, _)) if !self.ux.id_confirming => {
                    let shown = self.ux.id_words_open.then_some(words.as_slice());
                    if pin::mask(ui, "wiz-words", shown, t(Key::IdShowNewWords), 4, 106.0).clicked() && !self.ux.id_words_open {
                        self.ux.id_words_open = true;
                        self.ux.id_words_seen = true;
                    }
                }
                Some((_, picks)) => {
                    paint::text(ui, t(Key::IdConfirmSay), Type::Note, c(C::Ink2));
                    card::grid(ui, "wiz-word-picks", picks.len(), 120.0, |ui, k| {
                        field(ui, &fill1(Key::IdWordN, &(picks[k] + 1).to_string()), None, |ui| input::secret_line(ui, &mut self.ux.id_confirm[k], ""));
                    });
                }
            },
            Step::Network => {
                let picked = self.wiz_network_pick();
                // The wizard offers the default network and a custom one; another known network (the testnet)
                // is reached through the custom row, and shows here only when this machine already uses it.
                let mut rows: Vec<(String, String, String, bool)> = crate::deploy::KNOWN
                    .iter()
                    .filter(|d| d.name == crate::deploy::DEFAULT || d.name == picked)
                    .map(|d| (d.name.to_string(), t(d.label).to_string(), t(Key::WizNetPublic).to_string(), d.name == crate::deploy::DEFAULT))
                    .collect();
                rows.push((crate::deploy::CUSTOM.to_string(), t(Key::U3Custom).to_string(), t(Key::WizNetCustomSay).to_string(), false));
                ui.spacing_mut().item_spacing.y = tk::S2;
                for (name, title, say, recommended) in rows {
                    let id = egui::Id::new(("wiz-network", name.as_str()));
                    if full::radio_row(ui, id, picked == name, &title, recommended.then(|| t(Key::WizNetRecommended)), &say).clicked() {
                        self.ux.wiz_network = Some(name);
                    }
                }
                hint(ui, t(Key::WizNetNote));
            }
            Step::Genesis => {
                field(ui, t(Key::WizGenesisSay), None, |ui| input::line(ui, &mut self.ux.wiz_statement, t(Key::StatementHint)));
            }
            Step::Gas => {
                let gas = match &self.shell.chain {
                    Some(Done::Chain { gas_wei: Some(w), .. }) => fill1(Key::SetGasSay, &eth(*w)),
                    _ => t(Key::SetNotRead).to_string(),
                };
                let addr = self.shell.anchor.map(|a| a.hex());
                gas_face(ui, addr.as_deref(), &gas, gas_side);
                self.stage_line(ui, crate::task::Kind::Chain);
                if self.ux.wiz_gas_not_seen {
                    hint(ui, t(Key::WizGasNotSeen));
                }
            }
            Step::Backup => {
                paint::text(ui, t(Key::BackupRule), Type::Note, c(C::Ink2));
            }
        }
    }

    /// Create the ledger: what it will say, that it costs nothing (written locally), that it cannot change;
    /// its place under details. Floats above the wizard.
    pub(super) fn genesis_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.confirm_genesis {
            return;
        }
        let root = self.shell.home.as_ref().map(|h| h.root().display().to_string()).unwrap_or_default();
        let said = if self.ux.wiz_statement.trim().is_empty() { t(Key::CfBlank).to_string() } else { self.ux.wiz_statement.clone() };
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("create-ledger", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, t(Key::WizGenesisTitle), "");
                kv::kv(ui, &[(t(Key::CfWhat), Val::text(said.clone())), (t(Key::CfCost), Val::text(t(Key::CfCostLocal)))]);
                states::note_box(ui, t(Key::CfGenesisNote));
                details(ui, "create-ledger-details", &[(t(Key::CfWhere), Val::mono(root.clone()))]);
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
}

/// Entry kinds in plain words on lists: genesis says "create the ledger", history "record", succession
/// "change key or hand over"; others as on the ledger.
pub(super) fn user_kind_key(k: zikaron::tokens::EntryType) -> Key {
    match k {
        zikaron::tokens::EntryType::Genesis => Key::WizGenesisTitle,
        zikaron::tokens::EntryType::History => Key::TagAnchor,
        zikaron::tokens::EntryType::Succession => Key::TagHandover,
        other => kind_key(other),
    }
}

/// The gas card: its padding on all four sides, the key column, the gaps (rows, key to value, words to code)
/// and the address's characters per line (two lines, whatever the width).
const GAS_PAD: f32 = 28.0;
/// How long the gas step stays, done, before going on by itself (seconds).
const GAS_SEEN_SECS: f64 = 1.0;
const GAS_KEY_W: f32 = 88.0;
const GAS_COL_GAP: f32 = 20.0;
const GAS_ROW_GAP: f32 = 20.0;
const GAS_GROUP_GAP: f32 = 28.0;
const GAS_ADDR_LINE: usize = 21;
/// The code's side, and the caption's distance under its plate.
const QR_SIDE: f32 = 160.0;
const QR_CAPTION_GAP: f32 = 10.0;

/// The width of one address line (monospace: every line of it the same).
fn gas_addr_w(ctx: &egui::Context) -> f32 {
    ctx.fonts(|f| f.layout_no_wrap("0".repeat(GAS_ADDR_LINE), Type::Mono.font(), egui::Color32::BLACK).size().x).ceil()
}

/// The words beside the code: the key column, its gap, the address.
fn gas_words_w(ctx: &egui::Context) -> f32 {
    GAS_KEY_W + GAS_COL_GAP + gas_addr_w(ctx)
}

/// The code's plate with its margin.
fn qr_plate_w() -> f32 {
    QR_SIDE + 2.0 * paint::QR_PAD
}

/// The gas card's width: the words, the gap, the code, and the padding either side.
pub(super) fn gas_card_w(ctx: &egui::Context) -> f32 {
    2.0 * GAS_PAD + gas_words_w(ctx) + GAS_GROUP_GAP + qr_plate_w()
}

/// A text laid out on its own, and where its first baseline falls below its top.
fn gas_text(ui: &egui::Ui, s: &str, t: Type, colour: egui::Color32, wrap: Option<f32>) -> (std::sync::Arc<egui::Galley>, f32) {
    let mut job = egui::text::LayoutJob::single_section(s.to_string(), egui::TextFormat { font_id: t.font(), color: colour, line_height: Some(Type::Body.line()), ..Default::default() });
    // A wrapped value breaks by width, not by a line break in its text: what a person copies is the text itself.
    if let Some(w) = wrap {
        job.wrap = egui::text::TextWrapping { max_width: w, break_anywhere: true, ..Default::default() };
    }
    let g = ui.fonts(|f| f.layout_job(job));
    let base = g.rows.first().and_then(|r| r.glyphs.first()).map(|x| x.pos.y).unwrap_or(0.0);
    (g, base)
}

/// The key's address (two lines of equal length) and its balance on the left, the code a wallet scans to pay
/// on the right with its caption under it; the two groups centered on one middle line, each key on its
/// value's first baseline. Where the window leaves too narrow a column for the two side by side (`side`,
/// decided by the caller from the window), the code goes under the words (nothing overlaps, nothing is cut).
fn gas_face(ui: &mut egui::Ui, addr: Option<&str>, gas: &str, side: bool) {
    let addr_said = match addr {
        Some(a) => a.to_string(),
        None => t(Key::SetNoKey).to_string(),
    };
    let value_type = if addr.is_some() { Type::Mono } else { Type::Body };
    // The address wraps at the width of GAS_ADDR_LINE monospace characters: two equal lines on screen, one
    // unbroken address when copied.
    let addr_wrap = addr.map(|_| gas_addr_w(ui.ctx()) + 0.5);
    let rows = [(t(Key::IdAddress), addr_said, value_type, addr_wrap), (t(Key::IdGas), gas.to_string(), Type::Body, None)];
    let laid: Vec<_> = rows
        .iter()
        .map(|(k, v, vt, wrap)| (gas_text(ui, k, Type::Body, c(C::Ink2), None), gas_text(ui, v, *vt, c(C::Ink), *wrap)))
        .collect();
    // Each row as tall as its taller side once both share the first baseline.
    let heights: Vec<(f32, f32)> = laid
        .iter()
        .map(|((kg, kb), (vg, vb))| {
            let base = kb.max(*vb);
            (base, (base - kb + kg.size().y).max(base - vb + vg.size().y))
        })
        .collect();
    let words_h = heights.iter().map(|(_, h)| *h).sum::<f32>() + GAS_ROW_GAP * (rows.len() as f32 - 1.0);
    let caption = gas_text(ui, t(Key::WizGasScan), Type::Small, c(C::Ink3), None);
    let code_w = qr_plate_w();
    let code_h = code_w + QR_CAPTION_GAP + caption.0.size().y;
    let room = ui.available_width();
    let (w, h) = if side { (room, words_h.max(code_h)) } else { (room, words_h + GAS_ROW_GAP + code_h) };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
    let (words_at, code_at) = if side {
        (egui::pos2(rect.left(), rect.center().y - words_h / 2.0), egui::pos2(rect.right() - code_w, rect.center().y - code_h / 2.0))
    } else {
        (egui::pos2(rect.left(), rect.top()), egui::pos2(rect.center().x - code_w / 2.0, rect.top() + words_h + GAS_ROW_GAP))
    };
    let mut y = words_at.y;
    for (i, (((kg, kb), (vg, vb)), (base, row_h))) in laid.into_iter().zip(heights).enumerate() {
        ui.painter().galley(egui::pos2(words_at.x, y + base - kb), kg, c(C::Ink2));
        let at = egui::pos2(words_at.x + GAS_KEY_W + GAS_COL_GAP, y + base - vb);
        if i == 0 && addr.is_some() {
            // The address is text a person selects and copies (the same lines, where they were drawn).
            // In a child of its own: placing it must not move this ui's cursor back up over what follows.
            let mut line = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(at, vg.size())));
            line.add(egui::Label::new(vg).selectable(true));
        } else {
            ui.painter().galley(at, vg, c(C::Ink));
        }
        y += row_h + GAS_ROW_GAP;
    }
    let code_rect = egui::Rect::from_min_size(code_at, egui::vec2(code_w, code_h));
    let mut code_ui = ui.new_child(egui::UiBuilder::new().max_rect(code_rect).layout(egui::Layout::top_down(egui::Align::Center)));
    if let Some(a) = addr {
        qr_plate(&mut code_ui, &format!("ethereum:{a}"));
    } else {
        code_ui.add_space(code_w);
    }
    code_ui.add_space(QR_CAPTION_GAP);
    let (cg, _) = caption;
    let cr = code_ui.allocate_exact_size(cg.size(), egui::Sense::hover()).0;
    code_ui.painter().galley(cr.min, cg, c(C::Ink3));
}

/// A code a wallet scans, on its plate (the widget library's QR painter). The code is computed once per text
/// and kept for the frames after.
fn qr_plate(ui: &mut egui::Ui, text: &str) {
    let id = egui::Id::new(("zikaron-qr", text));
    let code = ui.ctx().data_mut(|d| d.get_temp::<Option<std::sync::Arc<crate::qr::Code>>>(id)).flatten().or_else(|| {
        let c = crate::qr::encode(text.as_bytes()).map(std::sync::Arc::new);
        ui.ctx().data_mut(|d| d.insert_temp(id, c.clone()));
        c
    });
    match code {
        Some(code) => {
            paint::qr(ui, &code.modules, QR_SIDE);
        }
        None => ui.add_space(qr_plate_w()),
    }
}

#[cfg(test)]
mod gas_address {
    use super::*;

    const ADDR: &str = "0x5d5a0de8ac3b41b649e0985c141d1735832cbf8f";

    fn frame(ctx: &egui::Context, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 760.0))),
            events,
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| gas_face(ui, Some(ADDR), "0.0000 ETH", true));
        })
    }

    /// The address as drawn: two rows on screen, one galley whose text is the address unbroken; a person
    /// dragging across it and copying gets exactly the address.
    #[test]
    fn selects_and_copies_the_address_unbroken() {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.0);
        // Dressed as the window is (skin and fonts), so the address is laid out in the real faces.
        zikaron_ui::skin::dress(&ctx);
        frame(&ctx, Vec::new());
        let out = frame(&ctx, Vec::new());
        let drawn = out
            .shapes
            .iter()
            .find_map(|c| match &c.shape {
                egui::Shape::Text(t) if t.galley.text() == ADDR => Some((t.pos, t.galley.clone())),
                _ => None,
            })
            .expect("the address is drawn as one text");
        assert_eq!(drawn.1.rows.len(), 2, "two rows on screen");
        let rect = egui::Rect::from_min_size(drawn.0, drawn.1.size());
        let (from, to) = (rect.left_top() + egui::vec2(1.0, 4.0), rect.right_bottom() - egui::vec2(1.0, 4.0));
        let m = egui::Modifiers::NONE;
        frame(&ctx, vec![egui::Event::PointerMoved(from)]);
        frame(&ctx, vec![egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
        frame(&ctx, vec![egui::Event::PointerMoved(rect.center())]);
        frame(&ctx, vec![egui::Event::PointerMoved(to)]);
        frame(&ctx, vec![egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
        let copied = frame(&ctx, vec![egui::Event::Copy]);
        let text: Vec<String> = copied
            .platform_output
            .commands
            .iter()
            .filter_map(|c| match c {
                egui::OutputCommand::CopyText(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(text, vec![ADDR.to_string()], "copied exactly the address, no line break");
    }
}
