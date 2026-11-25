use super::*;
use zikaron_ui::covers;
use zikaron_ui::sheetparts as parts;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WizRow {
    Generate,
    Import,
    Restore,
}

impl Win {
    pub(super) fn wizard_layer(&mut self, ctx: &egui::Context, now: f64) {
        use crate::nav::Step;
        let Some(want) = self.ux.wizard else { return };
        let pr = self.progress();
        let role = self.shell.settings.role;
        let step = pr.gate(want);
        self.ux.wizard = Some(step);
        let at = Step::ALL.iter().position(|s| *s == step).unwrap_or(0);
        let last = at + 1 == Step::ALL.len();
        let done = pr.done(step);
        let dir_id = egui::Id::new("zikaron-wizard-dir");
        let (was, dir) = ctx.data(|d| d.get_temp::<(usize, f32)>(dir_id)).unwrap_or((at, 0.0));
        let dir = if was == at { dir } else if at > was { 1.0 } else { -1.0 };
        ctx.data_mut(|d| d.insert_temp(dir_id, (at, dir)));
        let (mut move_to, mut close, mut copied) = (None, false, false);
        let (mut open_export, mut exit, mut rail_extra) = (false, false, false);
        let mut row: Option<WizRow> = None;
        let exit_shown = self.wizard_exit_shown();
        let mut act: Option<Action> = None;
        let fresh = self.shell.new_words.as_ref().map(|f| (f.words(), f.picks));
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault);
        let sub = if step == Step::Key && fresh.is_some() && self.ux.id_confirming { crate::nav::Sub::Check } else { crate::nav::Sub::First };
        let back = crate::nav::prev(step, sub);
        let mut go_back = false;
        let mut lang_to: Option<crate::lang::Lang> = None;
        let mut main_rect = egui::Rect::NOTHING;
        let (_, over) = covers::cover(ctx, "wizard", covers::Ground::Clear, |ui| {
            let screen = ui.max_rect();
            let wid = egui::Id::new(("zikaron-wizard-steps-w", ui.ctx().pixels_per_point().to_bits()));
            let steps_w = match ui.ctx().data(|d| d.get_temp::<f32>(wid)) {
                Some(w) => w,
                None => {
                    let names: Vec<&str> = Step::ALL.iter().flat_map(|s| {
                        let (zh, en) = crate::lang::both(s.title());
                        [zh, en]
                    }).collect();
                    let w = covers::steps_width(ui, &names);
                    ui.ctx().data_mut(|d| d.insert_temp(wid, w));
                    w
                }
            };
            let side_w = zikaron_ui::tokens::RAIL_W.max(steps_w + 2.0 * covers::RAIL_PAD_X);
            let side = egui::Rect::from_min_size(screen.min, egui::vec2(side_w, screen.height()));
            main_rect = egui::Rect::from_min_max(egui::pos2(side.right(), screen.top()), screen.max);
            covers::rail_ground(ui.painter(), side, 1.0);
            ui.painter().rect_filled(main_rect, 0.0, c(C::Ground));
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
                rail_extra = self.own_rail_extra(ui);
                lang_to = lang_pick(ui, "wiz-lang");
            });
        });
        let dir_now = if was == at { 0.0 } else { dir };
        covers::pane(ctx, over, at as u64, dir_now, main_rect, |ui| {
            let main = ui.max_rect().shrink(32.0);
            full::centered(ui, "wizard-main", main, WIZ_CARD_W, 0.0, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                covers::step_head(ui, &fill2(Key::WizCount, &(at + 1).to_string(), &Step::ALL.len().to_string()), t(step.title()));
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
                                        if let Step::Own(o) = step {
                                            if let Some(a) = self.own_wiz_next(o) {
                                                act = Some(a);
                                            }
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
                                            (None, _) => {}
                                            (Some(_), false) => copied = page::Page::new().primary_with(ui, t(Key::IdDoCopied), self.ux.id_words_seen).1.clicked(),
                                            (Some((_, picks)), true) => {
                                                if page::Page::new().primary_with(ui, t(Key::IdDoConfirm), !busy).1.clicked() {
                                                    let answers = picks.iter().zip(self.ux.id_confirm.iter()).map(|(p, w)| (*p, w.clone())).collect();
                                                    act = Some(Action::ConfirmIdentity { answers, label: self.ux.id_new_label.clone(), network: self.wiz_network_pick() });
                                                }
                                            }
                                        },
                                        Step::Own(o) => {
                                            let (asked, next) = self.own_wiz_keys(ui, o);
                                            if asked.is_some() {
                                                act = asked;
                                            }
                                            if next {
                                                move_to = Some(at + 1);
                                            }
                                        }
                                        Step::Backup => open_export = page::Page::new().primary_with(ui, t(Key::BackupExportTitle), self.shell.unlocked()).1.clicked(),
                                    }
                                }
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.spacing_mut().item_spacing.x = tk::S2;
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
        match row {
            Some(WizRow::Generate) => act = Some(Action::NewIdentity),
            Some(WizRow::Import) => {
                let label = std::mem::take(&mut self.ux.id_new_label);
                self.ux.id_open(IdModal::Import);
                self.ux.id_new_label = label;
            }
            Some(WizRow::Restore) => self.bk_open(Bk::Restore(RestoreFrom::FirstRun)),
            None => {}
        }
        if let Some(lang) = lang_to {
            let _ = self.act(Action::SetLang { lang }, now);
        }
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
            self.ux.id_confirming = true;
            self.ux.id_words_open = false;
        }
        if open_export {
            self.bk_open(Bk::Export);
        }
        if rail_extra {
            self.own_rail_go(now);
        }
        let esc = exit_shown && zikaron_ui::layer::esc(ctx, egui::Id::new("zikaron-wizard")) && !self.any_sheet();
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
        if self.own_wiz_turn(ctx, step, done, now) {
            move_to = move_to.or(Some(at + 1));
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
            let said = self.own_finished_words().unwrap_or_else(|| t(Key::WizFinished).to_string());
            self.toasts.say(said, Tone::Note, now);
        } else if let Some(i) = move_to {
            self.ux.wizard = Step::ALL.get(i).copied();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn wizard_body(&mut self, ui: &mut egui::Ui, st: &mut fields::Stack, step: crate::nav::Step, done: bool, fresh: Option<&(Vec<crate::secret::Secret>, [usize; 3])>, busy: bool, now: f64) -> Option<WizRow> {
        use crate::nav::Step;
        let len = crate::keybox::PIN_LEN;
        let readable = match step {
            Step::Own(o) => self.own_wiz_readable(o),
            _ => false,
        };
        if done && !readable {
            let nth = Step::ALL.iter().position(|s| *s == step).unwrap_or(0);
            parts::done(ui, &format!("wizard-done-{nth}"), t(Key::Done));
            let made = match step {
                Step::Key => self.shell.anchor.map(|a| (Key::IdAddress, a.hex())),
                Step::Own(o) => self.own_wiz_made(o),
                Step::Backup => self.shell.machine.backup.as_ref().map(|b| (Key::DataBackupFile, b.path.clone())),
                Step::Pin => None,
            };
            if let Some((k, v)) = made {
                let copied = zikaron_ui::fields::rows(ui, |ui, f| f.info(ui, t(k), card::Show::Copy(&v)));
                tell_copied(ui.ctx(), copied);
            }
            return None;
        }
        let mut row = None;
        match step {
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
                    self.take_first_pin(now, PinSaid::Toast);
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
                None => {
                    st.field(ui, t(Key::IdLabel), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut self.ux.id_new_label, t(Key::IdLabelExample), w, fields::Look::default()));
                    zikaron_ui::fields::rows(ui, |ui, f| {
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
            Step::Own(o) => self.own_wiz_card(ui, st, o, now),
            Step::Backup => {
                self.measure_if_due(now);
                let content = self.data_content();
                zikaron_ui::fields::rows(ui, |ui, f| {
                    super::backup::content_row(f, ui, &content);
                });
            }
        }
        row
    }

}

const WIZ_CARD_W: f32 = 520.0;

impl Win {
    pub(super) fn gas_step_card(&mut self, ui: &mut egui::Ui) {
        let gas = match self.chain_read() {
            Err(f) => (Mark::Bad, no_amount(fill1(Key::SetReadFailedNow, f.human()))),
            Ok(Some(Done::Chain { gas_wei: Some(w), .. })) => (if *w > 0 { Mark::Ok } else { Mark::Bad }, held_amount(*w, |x| fill1(Key::SetGasSay, x))),
            _ => (Mark::Todo, no_amount(t(Key::SetNotRead))),
        };
        let addr = self.shell.anchor.map(|a| a.hex());
        let code = addr.as_deref().zip(self.shell.settings.chain_id).map(|(a, chain)| gas_request(a, chain));
        let (copied, _) = gas_face(ui, addr.as_deref(), code.as_deref(), &gas);
        tell_copied(ui.ctx(), copied);
        if addr.is_some() && code.is_none() {
            hint(ui, t(Key::WizGasNoCode));
        }
        self.stage_line(ui, crate::task::Kind::Chain);
        if self.ux.wiz_gas_not_seen {
            hint(ui, t(Key::WizGasNotSeen));
        }
    }

    pub(super) fn gas_step_key(&mut self, ui: &mut egui::Ui) -> Option<Action> {
        if self.long_key(ui, t(Key::WizSent), Role::Primary, true, crate::task::Kind::Chain) {
            self.ux.wiz_gas_asked = Some(self.shell.tasks.landings(crate::task::Kind::Chain));
            self.ux.wiz_gas_seen = None;
            self.ux.wiz_gas_not_seen = false;
            return Some(Action::ReadChain);
        }
        None
    }

    pub(super) fn gas_step_turn(&mut self, ctx: &egui::Context, standing: bool, done: bool, now: f64) -> bool {
        let answered = matches!(self.ux.wiz_gas_asked, Some(stamp) if self.shell.tasks.answered_since(crate::task::Kind::Chain, stamp));
        if !standing {
            self.ux.wiz_gas_asked = None;
            self.ux.wiz_gas_seen = None;
            self.ux.wiz_gas_not_seen = false;
        } else if answered {
            if done {
                let seen = *self.ux.wiz_gas_seen.get_or_insert(now);
                if now - seen >= GAS_SEEN_SECS {
                    self.ux.wiz_gas_asked = None;
                    self.ux.wiz_gas_seen = None;
                    return true;
                } else {
                    ctx.request_repaint_after(std::time::Duration::from_secs_f64(GAS_SEEN_SECS - (now - seen)));
                }
            } else {
                self.ux.wiz_gas_asked = None;
                self.ux.wiz_gas_not_seen = matches!(self.chain_read(), Ok(Some(Done::Chain { gas_wei: Some(0), .. })));
            }
        }
        false
    }
}

const GAS_SEEN_SECS: f64 = 1.0;

const QR_PLATE: f32 = 112.0;
const GAS_GAP: f32 = 20.0;

const GAS_ROWS_MIN: f32 = 300.0;

fn gas_face(ui: &mut egui::Ui, addr: Option<&str>, code: Option<&str>, gas: &(Mark, zikaron_ui::layer::Amount)) -> (Option<String>, egui::Rect) {
    let room = ui.available_width();
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
                let _ = f.info(ui, t(Key::IdGas), card::Show::MarkAmount(gas.0, &gas.1));
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

fn gas_request(address: &str, chain_id: u64) -> String {
    format!("ethereum:{address}@{chain_id}")
}

#[cfg(test)]
mod gas_address {
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
                let (c, r) = gas_face(ui, Some(ADDR), Some(&gas_request(ADDR, 11_155_111)), &(Mark::Ok, no_amount("0.0000 ETH")));
                list.set(r);
                if c.is_some() {
                    *copied.borrow_mut() = c;
                }
            });
        })
    }

    #[test]
    fn copies_the_address_whole_from_its_row() {
        let ctx = egui::Context::default();
        ctx.set_pixels_per_point(1.0);
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
