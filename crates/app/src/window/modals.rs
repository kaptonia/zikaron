use super::*;

impl Win {
    pub(super) fn any_sheet(&self) -> bool {
        let u3 = &self.ux.u3;
        u3.new_anchor.is_some()
            || u3.confirm.is_some()
            || u3.form.is_some()
            || u3.kit_pick.is_some()
            || u3.work_pick
            || self.clash_modal
            || self.ux.u4.import_open
            || self.ux.u4.confirm.is_some()
            || self.ux.id_modal.is_some()
            || self.ux.bk.is_some()
            || self.ux.wiz_exit_ask
            || self.ux.cli_sheet
            || self.ux.extract_sheet
            || self.ux.confirm_genesis
            || self.ux.confirm_adopt
            || self.ux.upgrade_open
            || self.ux.rotate_open
            || self.ux.move_open
            || self.net_sheet_open()
            || self.ux.dir_sheet.is_some()
            || self.ux.snapshot_sheet
            || self.ux.out_sheet.is_some()
            || self.ux.clean_sheet
            || self.ux.pick_which.is_some()
            || self.ux.persona.sheet.is_some()
            || self.ux.persona.consent
            || self.ux.pass_open
            || self.ux.reroot.open
    }

    pub(super) fn sheets_clear(&mut self) {
        self.ux.persona.sheet = None;
        self.ux.persona.consent = false;
        self.ux.pass_open = false;
        self.ux.u3.new_anchor = None;
        self.ux.u3.confirm = None;
        self.ux.u3.send_after_estimate = None;
        self.ux.u3.sending = None;
        self.ux.u3.bump_press = None;
        self.ux.u3.form = None;
        self.ux.u3.kit_pick = None;
        self.ux.u3.work_pick = false;
        self.clash_modal = false;
        self.ux.u4.import_open = false;
        self.ux.u4.import_err = None;
        self.ux.u4.confirm = None;
        if self.ux.id_modal.is_some() {
            self.ux.id_close();
        }
        self.ux.confirm_genesis = false;
        self.ux.confirm_adopt = false;
        self.ux.upgrade_open = false;
        self.ux.rotate_open = false;
        self.ux.move_open = false;
        self.ux.sheet_shake = None;
        self.ux.bk_clear();
        self.cli_close();
        self.net_sheets_clear();
        self.ux.dir_sheet = None;
        self.ux.dir_waiting = false;
        self.ux.snapshot_sheet = false;
        self.ux.out_sheet = None;
        self.ux.out_waiting = false;
        self.ux.clean_sheet = false;
        self.ux.pick_which = None;
        self.ux.extract_sheet = false;
    }

    pub(super) fn layers_clear(&mut self) {
        self.sheets_clear();
        self.ux.wizard = None;
    }

    pub(super) fn u4_import_open(&mut self) {
        self.sheets_clear();
        self.ux.u4.import_open = true;
    }

    pub(super) fn u4_confirm_open(&mut self, cf: U4Confirm) {
        self.sheets_clear();
        self.ux.u4.confirm = Some(cf);
    }

    pub(super) fn id_layer_open(&mut self, m: IdModal) {
        self.sheets_clear();
        if let IdModal::Delete(id) = &m {
            let row = self.shell.identities.clone().unwrap_or_default().find(id).cloned();
            let (seats, troubles) = row.as_ref().map(crate::action::seats_with_entries).unwrap_or_default();
            self.ux.id_delete_ledgers = seats;
            self.shell.faults.extend(troubles);
            self.ux.id_delete_backup = row.as_ref().map(crate::identity::backup_seen);
        }
        self.ux.id_open(m);
    }

    pub(super) fn wizard_open(&mut self, step: crate::nav::Step) {
        self.layers_clear();
        if self.ux.wizard.is_none() {
            self.ux.wiz_from = Some((self.shell.settings.role, self.ux.nav.stack, self.ux.nav.place, self.ux.nav.hist.get(&self.ux.nav.stack).cloned()));
        }
        self.ux.wizard = Some(step);
        self.own_wiz_enter(step);
    }

    pub(super) fn wizard_exit_shown(&self) -> bool {
        let identity = self.shell.identities.as_ref().map(|r| !r.rows.is_empty()).unwrap_or(false);
        !self.shell.vault.absent() && identity
    }

    pub(super) fn wizard_exit(&mut self, now: f64) {
        if self.shell.new_words.is_some() {
            self.act(Action::DropFresh, now);
        }
        self.ux.wiz_exit_ask = false;
        self.ux.wizard = None;
        self.ux.confirm_genesis = false;
        self.ux.confirm_adopt = false;
        if let Some((role, stack, place, hist)) = self.ux.wiz_from.take() {
            let seated = self.shell.identities.as_ref().and_then(|r| r.now()).map(|(row, _)| row.address(role).is_some()).unwrap_or(false);
            if self.shell.settings.role != role && seated {
                self.act(Action::SwitchRole, now);
            }
            self.ux.nav.stack = stack;
            if let Some(p) = place {
                self.go(p, now);
            }
            if let Some(h) = hist {
                self.ux.nav.hist.insert(stack, h);
            }
        }
    }

    pub(super) fn u3_new_anchor_open(&mut self) {
        self.sheets_clear();
        self.ux.u3.new_anchor = Some(0);
    }

    pub(super) fn u3_form_open(&mut self, f: U3Form) {
        self.sheets_clear();
        self.ux.annotate_why = None;
        self.ux.u3.form = Some(f);
    }

    pub(super) fn annotate_open(&mut self, subject: String) {
        self.typed.annotate_subject = subject;
        self.u3_form_open(U3Form::Annotate);
    }

    pub(super) fn u3_open_confirm(&mut self, cf: U3Confirm) {
        self.sheets_clear();
        if let U3Confirm::Send { count } = cf {
            self.ux.u3.send_after_estimate = Some(count);
            self.ux.u3.estimate_from = self.ux.now;
        }
        self.ux.u3.confirm = Some(cf);
    }

    pub(super) fn anchor_form_clear(&mut self) {
        self.typed.work_note.clear();
        self.typed.batch_files.clear();
        self.typed.for_fields = Default::default();
        self.shell.content = None;
    }

    pub(super) fn draft(&self) -> crate::grantx::Draft {
        crate::grantx::Draft {
            grantee: self.typed.g_grantee.clone(),
            work: self.typed.g_work.clone(),
            terms: self.typed.g_terms.clone(),
            history: self.typed.g_history.clone(),
            from: self.typed.g_from.clone(),
            to: self.typed.g_to.clone(),
            scope_md: self.typed.g_scope.clone(),
            upstream: self.typed.g_upstream.clone(),
            ledger: String::new(),
            ..Default::default()
        }
    }

    pub(super) fn take_for_record(&mut self, paths: Vec<String>, now: f64) {
        if paths.len() > 1 {
            self.typed.batch_files = paths;
            self.shell.content = None;
            return;
        }
        let Some(p) = paths.into_iter().next() else { return };
        self.typed.batch_files.clear();
        self.act(Action::TakeDropped { path: p }, now);
    }

    pub(super) fn took_named(&mut self) {
        if self.typed.work_note.trim().is_empty() {
            if let Some(c) = self.shell.content.as_ref() {
                self.typed.work_note = width::file_name(&c.subject);
            }
        }
    }

    fn conflict_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let chain_now = self.shell.chain_now();
        let Some(c) = self.shell.fetch_conflict.clone() else { return };
        let ready = !self.typed.fetch_from.trim().is_empty() && !self.ux.fetch_held.is_empty() && !self.shell.tasks.in_flight(crate::task::Kind::Fetch);
        let row = |i: usize| {
            let (r, at) = &c.rows[i];
            table::Row { cells: vec![table::Cell::Seq(r.seq), table::Cell::Text(human_summary(r, chain_now)), table::Cell::Mono(at.map(crate::when::short).unwrap_or_default())], ..Default::default() }
        };
        let generation = self.shell.moved ^ crate::lang::lang() as u64;
        let (mut sure, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("fetch-conflict", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, &fill1(Key::ConflictTitle, &c.offline.to_string()), "");
                let cols = [table::SEQ, table::col(t(Key::U3Entry), table::Col::Fr(1.0)), table::col_r(t(Key::U3QueuedAt), table::Col::Px(120.0))];
                let _ = table::table_with(ui, "fetch-conflict", table::Spec::new(&cols, generation).head(true), c.rows.len(), &row);
            },
            |ui, _me| {
                sure = page::Pen::new().press(ui, t(Key::DoFetchReplace), ready).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.shell.fetch_conflict = None;
            self.ux.fetch_held = crate::secret::Secret::default();
        } else if sure {
            let a = Action::FetchAside { from: self.typed.fetch_from.clone(), password: std::mem::take(&mut self.ux.fetch_held) };
            self.act(a, now);
        }
    }

    pub(super) fn sheets(&mut self, ctx: &egui::Context, now: f64) {
        self.bk_sheets(ctx, now);
        if self.shell.vault.gate_up() {
            return;
        }
        self.cli_sheet(ctx, now);
        self.net_edit_sheet(ctx, now);
        self.read_net_sheet(ctx, now);
        self.publish_sheet(ctx, now);
        self.publish_edit_sheet(ctx, now);
        self.extract_sheet(ctx, now);
        self.clean_sheet(ctx, now);
        self.dir_sheet(ctx, now);
        self.estimate_due(ctx);
        self.new_anchor_sheet(ctx, now);
        self.conflict_sheet(ctx, now);
        self.card_follow_up();
        match self.ux.u3.confirm.clone() {
            Some(U3Confirm::Send { count }) => self.send_sheet(ctx, count, None, now),
            Some(U3Confirm::Bump { tx }) => self.bump_sheet(ctx, &tx, now),
            Some(U3Confirm::NoEstimate { count, why, next, raw }) => self.send_sheet(ctx, count, Some((Key::U3NoEstimateTitle, why, next, raw)), now),
            Some(U3Confirm::NotSent { count, why, next, raw }) => self.send_sheet(ctx, count, Some((Key::LampRefused, why, next, raw)), now),
            Some(U3Confirm::Retract { subject }) => self.delete_record_sheet(ctx, &subject, now),
            Some(U3Confirm::Grant) => self.grant_confirm_sheet(ctx, now),
            Some(U3Confirm::Revoke { grant }) => self.revoke_sheet(ctx, &grant, now),
            None => {}
        }
        match self.ux.u3.form {
            Some(U3Form::Annotate) => self.annotate_sheet(ctx, now),
            None => {}
        }
        self.kit_pick_sheet(ctx);
        self.grant_work_pick_sheet(ctx);
        self.clash_sheet(ctx);
        self.add_grant_sheet(ctx, now);
        self.relicense_sheet(ctx, now);
        self.id_sheets(ctx, now);
        self.genesis_sheet(ctx, now);
        self.wizard_adopt_sheet(ctx, now);
        self.upgrade_sheet(ctx, now);
        self.rotate_sheet(ctx, now);
        self.move_sheet(ctx, now);
        self.persona_sheets(ctx, now);
        self.consent_sheet(ctx, now);
        self.reroot_sheet(ctx, now);
        self.pass_sheet(ctx, now);
        self.pick_which_sheet(ctx);
    }

    fn pick_which_sheet(&mut self, ctx: &egui::Context) {
        let Some(site) = self.ux.pick_which else { return };
        let (mut kind, mut close) = (None, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("pick-which", tk::SHEET_W),
            self,
            |ui, _me| sheet::title(ui, t(Key::PickWhich), ""),
            |ui, _me| {
                if page::Page::new().primary(ui, t(Key::PickFile)).1.clicked() {
                    kind = Some(crate::platform::Pick::File);
                }
                if key::key(ui, t(Key::PickFolder), Role::Secondary, true).clicked() {
                    kind = Some(crate::platform::Pick::Folder);
                }
                close = foot_left(ui, t(Key::CfBack));
            },
        );
        if close || out.esc {
            self.ux.pick_which = None;
        } else if let Some(k) = kind {
            self.ux.pick_which = None;
            ctx.data_mut(|d| d.get_temp_mut_or_default::<PathMail>(path_mail()).asked = Some((site, k)));
        }
    }

    fn move_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.move_open {
            return;
        }
        let rows = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
        let me = self.shell.anchor.map(|a| a.hex()).unwrap_or_default();
        let old: Vec<&crate::ledgerx::Row> = rows.iter().filter(|r| r.law == zikaron_glue::book::Law::One).collect();
        let lineage = old.iter().any(|r| (r.seq == 0 && r.author.eq_ignore_ascii_case(&me)) || r.facts.to.as_deref().is_some_and(|to| to.eq_ignore_ascii_case(&me)));
        let left_out = !old.is_empty() && !lineage;
        let (mut go, mut close) = (false, false);
        let mono = fields::Look::default();
        let reg_bad = crate::addressedx::not_address(&self.ux.move_cells[1], crate::addressedx::Takes::Address);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("move-chain", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::ChainMoveTitle), "");
                fields::stack(ui, |ui, st| {
                    let [c, r, f, n] = &mut me.ux.move_cells;
                    st.field(ui, t(Key::BasisChain), None, None, |ui, w| fields::line(ui, c, "", w, mono));
                    st.field(ui, t(Key::SetRegistry), None, reg_bad.then(|| (t(Key::KitToWhyAddress), true)), |ui, w| fields::line(ui, r, "", w, fields::Look { bad: reg_bad, ..mono }));
                    st.field(ui, t(Key::BasisFrom), None, None, |ui, w| fields::line(ui, f, "", w, mono));
                    st.field(ui, t(Key::SetNodes), None, None, |ui, w| fields::line(ui, n, "", w, mono));
                });
                if left_out {
                    card::form(ui, |ui, f| f.note(ui, t(Key::ChainOldUnread)));
                }
            },
            |ui, _me| {
                go = page::Page::new().primary_why(ui, t(Key::ChainMoveGo), !reg_bad, t(Key::KitToWhyAddress)).1.clicked() && !reg_bad;
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.ux.move_open = false;
        } else if go {
            let [c, r, f, n] = self.ux.move_cells.clone();
            let a = self.act(Action::MoveChain { chain: c, registry: r, from_block: f, nodes: n }, now);
            if matches!(a, Applied::Moved { .. }) {
                self.ux.move_open = false;
                self.ux.move_cells = Default::default();
            }
        }
    }

    pub(super) fn rotate_sheet_open(&mut self, now: f64) {
        self.sheets_clear();
        self.ux.rotate_open = true;
        self.ux.rot_words = 0;
        self.ux.rot_to_given = false;
        self.ux.rot_new_words = false;
        self.ux.rot_addr.clear();
        self.ux.rot_asked.clear();
        self.ux.rot_sweep_addr = None;
        self.ux.rot_sweep_asked.clear();
        self.ux.rot_sweep_said = None;
        self.shell.rot_vetted = None;
        self.rot_pending_read();
        if self.ux.rot_bookless || self.ux.rot_unchecked.is_some() {
            return;
        }
        if self.shell.rows.is_none() {
            let _ = apply(&mut self.shell, Action::ReadLedger);
        }
        if !matches!(self.shell.persona, crate::facex::State::Read(_)) && !self.shell.tasks.in_flight(crate::task::Kind::Persona) && self.shell.home.is_some() {
            let _ = apply(&mut self.shell, Action::FaceReread);
        }
        self.rot_look(now);
    }

    fn rot_look(&mut self, now: f64) {
        if self.shell.tasks.in_flight(crate::task::Kind::Rotate) {
            return;
        }
        self.ux.rot_read_at = now;
        self.act(Action::Rotate { step: crate::action::RotateStep::Look }, now);
    }

    pub(super) fn rot_pending_read(&mut self) {
        let id = self.shell.identities.as_ref().and_then(|r| r.now()).map(|(r, _)| r.id.clone()).unwrap_or_default();
        self.ux.rot_pending = !id.is_empty() && crate::action::words_pending(&id);
        self.ux.rot_unhomed = self.shell.identities.as_ref().and_then(|r| r.now()).is_some_and(|(r, _)| r.seats().into_iter().any(|s| crate::action::seat_ledger(r, s) == crate::action::SeatLedger::None));
        self.ux.rot_words_unchecked = self.shell.identities.as_ref().and_then(|r| r.now()).and_then(|(r, _)| r.seats().into_iter().find(|s| crate::action::seat_ledger(r, *s) == crate::action::SeatLedger::Unchecked));
        (self.ux.rot_bookless, self.ux.rot_unchecked) = match self.shell.set_up_seat() {
            None => (false, None),
            Some(crate::shell::SetUp::Create) => (true, None),
            Some(crate::shell::SetUp::Fetch) => (false, self.marked_title()),
        };
        self.ux.rot_words_note = if self.ux.rot_pending {
            self.shell.identities.as_ref().and_then(|reg| reg.now().and_then(|(r, s)| crate::action::words_note(reg, &r.id, s)))
        } else {
            None
        };
    }

    fn rotate_sheet(&mut self, ctx: &egui::Context, now: f64) {
        use crate::action::RotateStep;
        use crate::rotatex::{Source, Stage};
        if !self.ux.rotate_open {
            return;
        }
        let mut stage = crate::action::rotation_stage(&self.shell).unwrap_or(Stage::Ready);
        if stage == Stage::Done && !self.ux.rot_walked && self.shell.handed.is_none() {
            stage = crate::action::start_stage(&self.shell).unwrap_or(Stage::Ready);
        }
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Rotate) || self.shell.tasks.in_flight(crate::task::Kind::Anchor);
        let sending = self.shell.rot_sending;
        let held = match &stage {
            Stage::Sweep { chain, .. } => self.shell.rot_sweep_held.as_ref().filter(|(c, _, _)| c == chain).map(|(_, v, _)| *v),
            _ => None,
        };
        if (stage.waits_on_chain() || held.is_some()) && !busy && sending.is_none() {
            let due = self.ux.rot_read_at + ROT_READ_EVERY - now;
            if due <= 0.0 {
                self.rot_look(now);
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(due));
            }
        }
        let busy = busy || self.shell.tasks.in_flight(crate::task::Kind::Rotate);
        let unread = !busy && self.shell.rot_unread.is_some();
        let first_chain = self.shell.basis_chains().first().map(|(c, _, _)| *c).unwrap_or_default();
        let out: Vec<String> = self.shell.queue.items.iter().filter(|q| q.step.in_flight()).map(|q| q.id.clone()).collect();
        let anchor_out = self.shell.tasks.in_flight(crate::task::Kind::Anchor);
        let words = self.shell.identities.as_ref().and_then(|r| r.now()).map(|(r, _)| r.kind() == crate::identity::Kind::Words).unwrap_or(false);
        if !words {
            self.ux.rot_to_given = true;
        }
        let made = self.shell.identities.as_ref().and_then(|r| r.now()).and_then(|(r, s)| r.office(s)).and_then(|o| o.next).map(crate::key::Address);
        let handed = self.shell.handed.as_deref().and_then(crate::key::Address::parse);
        let given = self.shell.rot_given.or(handed);
        let next = made.or(given);
        let sweep_to = handed.map(|h| self.shell.rot_sweep_to.unwrap_or(h));
        let handed_sweep = handed.is_some() && (matches!(stage, Stage::Sweep { .. }) || !crate::action::swept_now(&self.shell).is_empty());
        if let (Some(to), Stage::Sweep { .. }, None) = (sweep_to, &stage, &self.ux.rot_sweep_addr) {
            self.ux.rot_sweep_addr = Some(to.hex());
        }
        let typed_to = self.ux.rot_sweep_addr.as_deref().map(str::trim).unwrap_or_default().to_string();
        let to_changed = handed.is_some() && crate::key::Address::parse(&typed_to) != sweep_to;
        let sweep_said = self.ux.rot_sweep_said.clone().filter(|_| self.ux.rot_sweep_asked == typed_to);
        let current = self.shell.identities.as_ref().and_then(|r| r.now()).and_then(|(r, s)| r.office(s)).map(|o| crate::key::Address(o.current));
        let short = self.shell.rot_short_said();
        let fresh = self.shell.new_words.as_ref().map(|f| (f.words(), f.picks));
        let unhomed = self.ux.rot_unhomed;
        let pending = self.ux.rot_pending;
        let (bookless, words_note) = (self.ux.rot_bookless, self.ux.rot_words_note);
        let unchecked = self.ux.rot_unchecked.clone();
        let (mut go, mut close, mut to_chain, mut to_network, mut back) = (None::<Action>, false, false, false, false);
        let mut fetch = false;
        let at = stage.step().unwrap_or(0);
        let addr = crate::key::Address::parse(&self.ux.rot_addr);
        let addr_bad = crate::addressedx::not_address(&self.ux.rot_addr, crate::addressedx::Takes::Address);
        let to_bad = crate::addressedx::not_address(&typed_to, crate::addressedx::Takes::Address);
        let to_why = if to_bad { t(Key::KitToWhyAddress).to_string() } else { still_needs(&[(typed_to.is_empty(), Key::RotSweepTo)]) };
        let addr_why = if addr_bad { t(Key::KitToWhyAddress).to_string() } else { still_needs(&[(addr.is_none(), Key::RotNewKey)]) };
        let confirming = stage == Stage::Ready && self.ux.rot_to_given && addr.is_some() && addr == self.shell.rot_vetted;
        let refused = if !busy && self.ux.rot_to_given && !self.ux.rot_asked.is_empty() && self.ux.rot_asked == self.ux.rot_addr.trim() {
            self.shell.failed.get(&crate::task::Kind::Rotate).map(|f| f.human().to_string())
        } else {
            None
        };
        let spec = sheet::Spec::new("rotate", tk::SHEET_W);
        let spec = if matches!(stage, Stage::Owed { .. } | Stage::Bookless { .. } | Stage::Ready | Stage::Declared { .. } | Stage::TakenUp { .. }) && self.ux.rot_words == 0 { spec.keys_above(tk::S5) } else { spec };
        let out = sheet::show(
            ctx,
            spec,
            self,
            |ui, me| {
                if me.ux.rot_words > 0 {
                    sheet::title(ui, t(Key::RotTitle), "");
                    if unhomed {
                        card::form(ui, |ui, f| f.note(ui, t(Key::RotUnhomedNote)));
                    }
                    fields::stack(ui, |ui, st| {
                        if me.ux.rot_words == 1 {
                            let w = fresh.as_ref().map(|(w, _)| w.clone());
                            if st.field(ui, t(Key::IdTabWords), None, None, |ui, _| zikaron_ui::sheetparts::words(ui, "rot-words", w.as_deref(), me.ux.rot_words_open, t(Key::IdShowNewWords)).clicked()) {
                                me.ux.rot_words_open = true;
                            }
                        } else if let Some((_, picks)) = fresh.as_ref() {
                            for (k, p) in picks.iter().enumerate() {
                                st.field(ui, &fill1(Key::IdWordN, &(p + 1).to_string()), None, None, |ui, w| input::secret_line_w_marked(ui, &mut me.ux.rot_confirm[k], "", w, false));
                            }
                        }
                    });
                    return;
                }
                if bookless {
                    sheet::title(ui, t(Key::RotTitle), "");
                    card::form(ui, |ui, f| f.note(ui, t(Key::RotNoLedgerSeat)));
                    return;
                }
                if let Some(title) = &unchecked {
                    sheet::title(ui, t(Key::RotTitle), "");
                    card::form(ui, |ui, f| fetch = f.act(ui, title, "", key::Key::new(t(Key::DoFetchLedger), Role::Primary)).clicked());
                    return;
                }
                match &stage {
                    Stage::Owed { n } => {
                        sheet::title(ui, t(Key::RotOwedTitle), "");
                        ui.add_space(tk::S2 - 6.0);
                        paint::text(ui, t(Key::RotOwedLede), Type::Body, c(C::Ink2));
                        ui.add_space(tk::S4 - tk::S3);
                        zikaron_ui::sheetparts::fact(ui, t(Key::ItemUnanchored), &fill1(Key::U3EntriesCount, &n.to_string()));
                        return;
                    }
                    Stage::Bookless { chains } => {
                        sheet::title(ui, t(Key::RotBooklessTitle), "");
                        ui.add_space(tk::S4 - 6.0);
                        ui.spacing_mut().item_spacing.y = tk::S2;
                        for (chain, why) in chains {
                            let said = match why {
                                _ if unread => fill1(Key::RotUnread, &chain.to_string()),
                                crate::rotatex::Lack::NoRow => t(Key::RotNoBook).to_string(),
                                crate::rotatex::Lack::WrongCode => t(Key::RotWrongBook).to_string(),
                            };
                            zikaron_ui::sheetparts::fact(ui, &fill1(Key::ChainNumbered, &chain.to_string()), &said);
                        }
                        return;
                    }
                    Stage::Unseated { .. } => {
                        sheet::title(ui, t(Key::FaultWhatRotateUnseated), "");
                        return;
                    }
                    Stage::Declared { chain, to } => {
                        sheet::title(ui, t(Key::RotTitle), "");
                        ui.add_space(tk::S2 - 6.0);
                        paint::text(ui, &fill2(Key::RotDeclaredLine, &chain.to_string(), &zikaron::hexfmt::encode(to)), Type::Body, c(C::Ink2));
                        return;
                    }
                    Stage::TakenUp { chain, by } => {
                        sheet::title(ui, t(Key::RotTitle), "");
                        ui.add_space(tk::S2 - 6.0);
                        paint::text(ui, &fill2(Key::RotTakenUpLine, &chain.to_string(), &zikaron::hexfmt::encode(by)), Type::Body, c(C::Ink2));
                        return;
                    }
                    Stage::Unentered { chains } => {
                        sheet::title(ui, t(Key::FaultWhatRotateUnentered), "");
                        ui.add_space(tk::S4 - 6.0);
                        ui.spacing_mut().item_spacing.y = tk::S2;
                        for chain in chains {
                            zikaron_ui::sheetparts::fact(ui, &fill1(Key::ChainNumbered, &chain.to_string()), t(Key::FaultNextRotateUnentered));
                        }
                        return;
                    }
                    Stage::Ready if confirming => {
                        sheet::title(ui, t(Key::RotConfirmTitle), "");
                        card::form(ui, |ui, f| {
                            let hex = addr.map(|a| a.hex()).unwrap_or_default();
                            let _ = f.info(ui, t(Key::RotNewKey), card::Show::Mono(&format!("{}\u{2026}{}", &hex[..hex.len().min(6)], &hex[hex.len().saturating_sub(4)..])));
                            let _ = f.info(ui, t(Key::RotConfirmHow), card::Show::Text(t(Key::RotConfirmGiven)));
                        });
                        ui.add_space(tk::S2 - tk::S3);
                        paint::text(ui, &format!("{} {} {}", t(Key::RotConfirmWarn), t(Key::RotConfirmRevoke), t(Key::RotBooksLater)), Type::Small, c(C::Ink2));
                        return;
                    }
                    Stage::Ready => {
                        sheet::title(ui, t(Key::RotTitle), "");
                        if words {
                            card::form(ui, |ui, f| {
                                for (to_given, key) in [(false, Key::RotModeLocal), (true, Key::RotModeGiven)] {
                                    let on = me.ux.rot_to_given == to_given;
                                    let (row, _) = f.line(ui, None, t(key), "", false, false, c(C::Ink), |ui| zikaron_ui::sheetparts::radio(ui, if to_given { "rot-given" } else { "rot-local" }, on));
                                    let hit = ui.interact(row.rect, egui::Id::new(("rot-mode", to_given)), egui::Sense::click());
                                    if hit.clicked() {
                                        me.ux.rot_to_given = to_given;
                                    }
                                    let _ = hit.on_hover_cursor(egui::CursorIcon::PointingHand);
                                }
                            });
                            ui.add_space(14.0 - tk::S3);
                        }
                        if me.ux.rot_to_given {
                            fields::stack(ui, |ui, st| {
                                let line = if addr_bad { Some((t(Key::KitToWhyAddress), true)) } else { refused.as_deref().map(|s| (s, true)) };
                                st.field(ui, t(Key::RotNewKey), None, line, |ui, w| fields::line(ui, &mut me.ux.rot_addr, "0x\u{2026}", w, fields::Look { mono: true, bad: addr_bad || refused.is_some(), ..Default::default() }));
                            });
                        } else if words {
                            let held = me.ux.rot_words_unchecked.filter(|_| me.ux.rot_new_words && !pending).map(|s| fill1(Key::RotWordsSeatUnchecked, t(id_seat_key(s))));
                            fields::stack(ui, |ui, st| {
                                st.field(ui, t(Key::RotSource), None, held.as_deref().map(|h| (h, true)), |ui, _| {
                                    let cells = [t(Key::RotSourceSame).into(), t(Key::RotSourceNew).into()];
                                    if let Some(i) = seg::seg(ui, "rot-source", &cells, usize::from(me.ux.rot_new_words)) {
                                        me.ux.rot_new_words = i == 1;
                                    }
                                });
                            });
                        }
                        if !me.ux.rot_to_given {
                            ui.add_space(tk::S2);
                            paint::text(ui, t(Key::RotBooksLater), Type::Small, c(C::Ink2));
                        }
                        return;
                    }
                    _ => {}
                }
                sheet::title(ui, t(Key::RotTitle), "");
                card::form(ui, |ui, f| {
                    if stage == Stage::Behind {
                        f.note(ui, t(Key::FaultWhatAuditBehind));
                        return;
                    }
                    let steps = [(1, Key::RotStepKey), (2, Key::RotStepWrite), (3, Key::RotStepSweep), (4, Key::RotStepChain)];
                    let many = me.shell.basis_chains().len() > 1;
                    let ended = stage == Stage::Done || matches!(&stage, Stage::Sweep { then: None, .. });
                    let on_chain = match &stage {
                        Stage::Sweep { then, .. } => then.as_deref(),
                        s => Some(s),
                    };
                    let failed_row = me.shell.rot_reverted.as_ref().map(|r| (match r.step { "declare" => 1, "sweep" => 3, _ => 4 }, if r.out_of_gas() { fill2(Key::RotOutOfGas, &r.used.unwrap_or_default().to_string(), &r.limit.to_string()) } else { t(Key::RotFailedInBlock).to_string() }));
                    let anchor_failed = |id: &str| me.shell.queue.items.iter().any(|q| q.id.eq_ignore_ascii_case(id) && matches!(q.step, crate::queue::Step::Reverted { .. }));
                    let chain_row = |s: &Stage| -> (Mark, String) {
                        match s {
                            Stage::Anchoring { id } if crate::rotatex::succession_out(id, anchor_out, &out) => (Mark::Busy, t(Key::UpWaiting).to_string()),
                            s if crate::rotatex::step_waits(s, sending) && me.shell.rot_sweep_out.is_none() => (Mark::Busy, t(Key::UpWaiting).to_string()),
                            _ if short && !busy => (Mark::Warn, t(Key::RotNewShort).to_string()),
                            Stage::Anchoring { id } if anchor_failed(id) => (Mark::Warn, t(Key::RotFailedInBlock).to_string()),
                            Stage::Anchoring { .. } if unread && matches!(stage, Stage::Anchoring { .. }) => (Mark::Warn, fill1(Key::RotUnread, &first_chain.to_string())),
                            _ => (Mark::Todo, String::new()),
                        }
                    };
                    if at > 0 {
                        for (n, k) in steps.iter().filter(|(n, _)| given.is_none() || handed_sweep || *n != 3) {
                            let n = *n;
                            if n == 3 {
                                if let Stage::Sweep { chain, send, .. } = &stage {
                                    let (mark, said) = match (held, send) {
                                        (Some(v), _) => (Mark::Busy, fill1(Key::RotSweepOut, &eth_exact(v))),
                                        _ if crate::rotatex::step_waits(&stage, sending) => (Mark::Busy, t(Key::UpWaiting).to_string()),
                                        _ if unread => (Mark::Warn, fill1(Key::RotUnread, &chain.to_string())),
                                        (None, Some(v)) => (Mark::Todo, fill1(if handed.is_some() { Key::RotSweepOutSend } else { Key::RotSweepSend }, &eth_exact(*v))),
                                        _ if busy => (Mark::Busy, t(Key::RotReading).to_string()),
                                        _ => (Mark::Warn, fill1(Key::RotUnread, &chain.to_string())),
                                    };
                                    if many {
                                        let _ = f.info(ui, t(*k), card::Show::Mark(Mark::Todo, ""));
                                        let _ = f.info(ui, &crate::readnets::chain_name(*chain, None), card::Show::Mark(mark, &said));
                                    } else {
                                        let _ = f.info(ui, t(*k), card::Show::Mark(mark, &said));
                                    }
                                } else if at > 3 && !crate::action::sweep_open(&me.shell) {
                                    let rows = swept_rows(&me.shell);
                                    match rows.as_slice() {
                                        [(_, said)] if !many => {
                                            let _ = f.info(ui, t(*k), card::Show::MarkAmount(Mark::Ok, said));
                                        }
                                        _ => {
                                            let _ = f.info(ui, t(*k), card::Show::Mark(Mark::Ok, ""));
                                            for (chain, said) in rows {
                                                let _ = f.info(ui, &crate::readnets::chain_name(chain, None), card::Show::MarkAmount(Mark::Ok, &said));
                                            }
                                        }
                                    }
                                } else {
                                    let _ = f.info(ui, t(*k), card::Show::Mark(Mark::Todo, ""));
                                }
                                if let Some((3, said)) = &failed_row {
                                    let _ = f.info(ui, t(Key::RotLastSend), card::Show::Mark(Mark::Warn, said));
                                }
                                continue;
                            }
                            let (mark, said) = if ended || (n < at && n != 4) {
                                (Mark::Ok, String::new())
                            } else if n == 4 {
                                on_chain.filter(|_| at >= 3).map(chain_row).unwrap_or((Mark::Todo, String::new()))
                            } else if n == at {
                                match &stage {
                                    Stage::Unread { .. } if busy => (Mark::Busy, t(Key::RotReading).to_string()),
                                    Stage::Unread { chain, .. } => (Mark::Warn, fill1(Key::RotUnread, &chain.to_string())),
                                    s if crate::rotatex::step_waits(s, sending) => (Mark::Busy, t(Key::UpWaiting).to_string()),
                                    _ => (Mark::Todo, String::new()),
                                }
                            } else {
                                (Mark::Todo, String::new())
                            };
                            let _ = f.info(ui, t(*k), card::Show::Mark(mark, &said));
                            if let Some((_, said)) = failed_row.as_ref().filter(|(row, _)| *row == n && !ended) {
                                let _ = f.info(ui, t(Key::RotLastSend), card::Show::Mark(Mark::Warn, said));
                            }
                        }
                    }
                    if let Some(n) = next.or(current.filter(|_| at >= 3 && given.is_none())) {
                        let _ = f.info(ui, t(Key::RotNewKey), card::Show::Copy(&n.hex()));
                    }
                    match words_note.filter(|_| pending) {
                        Some(crate::action::WordsNote::Kept) => {
                            f.note(ui, t(Key::RotOldWordsKept));
                        }
                        Some(crate::action::WordsNote::OthersFollow) => {
                            f.note(ui, t(Key::RotOthersFollow));
                        }
                        None => {}
                    }
                });
                if let (Some(_), Stage::Sweep { .. }) = (handed, &stage) {
                    fields::stack(ui, |ui, st| {
                        let line = if to_bad { Some((t(Key::KitToWhyAddress), true)) } else { sweep_said.as_deref().map(|s| (s, true)) };
                        if let Some(addr) = me.ux.rot_sweep_addr.as_mut() {
                            st.field(ui, t(Key::RotSweepTo), None, line, |ui, w| fields::line(ui, addr, "0x\u{2026}", w, fields::Look { mono: true, bad: line.is_some(), ..Default::default() }));
                        }
                    });
                }
            },
            |ui, me| {
                let mut backed = false;
                if me.ux.rot_words == 1 {
                    if page::Page::new().primary_with(ui, t(Key::IdDoCopied), me.ux.rot_words_open).1.clicked() {
                        me.ux.rot_words = 2;
                    }
                    close = rot_back_key(ui, &mut backed);
                    return;
                }
                if me.ux.rot_words == 2 {
                    if page::Page::new().primary_with(ui, t(Key::IdDoConfirm), !busy).1.clicked() {
                        let answers = fresh.as_ref().map(|(_, picks)| picks.iter().zip(me.ux.rot_confirm.iter()).map(|(i, w)| (*i, w.clone())).collect()).unwrap_or_default();
                        go = Some(Action::Rotate { step: RotateStep::Begin { source: Source::NewWords, answers } });
                    }
                    close = rot_back_key(ui, &mut backed);
                    return;
                }
                if bookless || unchecked.is_some() {
                    close = key::key(ui, t(Key::WizLater), Role::Secondary, true).clicked();
                    return;
                }
                let mut on: Option<Stage> = None;
                let step_key = |ui: &mut egui::Ui, label: &str, may: bool, at_once: bool| {
                    if at_once {
                        page::Pen::new().press(ui, label, may).clicked()
                    } else {
                        page::Page::new().primary_with(ui, label, may).1.clicked()
                    }
                };
                match &stage {
                    Stage::Done => close = page::Page::new().primary(ui, t(Key::WizFinish)).1.clicked(),
                    Stage::Owed { .. } => {
                        to_chain = page::Page::new().primary(ui, t(Key::GoAnchor)).1.clicked();
                        close = rot_back_key(ui, &mut backed);
                    }
                    Stage::Bookless { .. } => {
                        to_network = page::Page::new().primary(ui, t(Key::SetChain)).1.clicked();
                        close = rot_back_key(ui, &mut backed);
                    }
                    Stage::Unseated { .. } | Stage::Unentered { .. } => {
                        close = rot_back_key(ui, &mut backed);
                    }
                    Stage::Behind => {
                        if page::Page::new().primary_with(ui, t(Key::DoReconcile), !busy && !me.shell.tasks.in_flight(crate::task::Kind::Audit)).1.clicked() {
                            go = Some(Action::Audit);
                        }
                    }
                    Stage::Unread { .. } => {}
                    Stage::Declared { chain, .. } => {
                        if step_key(ui, t(Key::RotWithdraw), !busy, true) {
                            go = Some(Action::Rotate { step: RotateStep::Withdraw { chain: *chain } });
                        }
                        close = rot_back_key(ui, &mut backed);
                    }
                    Stage::TakenUp { .. } => {
                        close = rot_back_key(ui, &mut backed);
                    }
                    Stage::Ready if confirming => {
                        if step_key(ui, t(Key::RotConfirmTitle), !busy, true) {
                            go = Some(Action::Rotate { step: RotateStep::Hand { to: me.ux.rot_addr.trim().to_string() } });
                        }
                        close = rot_back_key(ui, &mut backed);
                        back = foot_left(ui, t(Key::WizPrev));
                    }
                    Stage::Ready if me.ux.rot_to_given => {
                        if page::Page::new().primary_why(ui, t(Key::WizardNext), addr.is_some() && !busy, &addr_why).1.clicked() {
                            me.ux.rot_asked = me.ux.rot_addr.trim().to_string();
                            go = Some(Action::Rotate { step: RotateStep::Vet { to: me.ux.rot_asked.clone() } });
                        }
                        close = rot_back_key(ui, &mut backed);
                    }
                    Stage::Ready => {
                        let new_words = words && me.ux.rot_new_words;
                        let held = me.ux.rot_words_unchecked.filter(|_| new_words && !pending).map(|s| fill1(Key::RotWordsSeatUnchecked, t(id_seat_key(s))));
                        let label = t(if new_words { Key::WizardNext } else { Key::RotStart });
                        let pressed = match &held {
                            Some(why) => page::Page::new().primary_why(ui, label, false, why).1.clicked(),
                            None => page::Page::new().primary_with(ui, label, !busy).1.clicked(),
                        };
                        if pressed {
                            let source = if !new_words {
                                Some(Source::SameWords)
                            } else if pending {
                                Some(Source::NewWords)
                            } else {
                                None
                            };
                            match source {
                                Some(source) => go = Some(Action::Rotate { step: RotateStep::Begin { source, answers: Vec::new() } }),
                                None => {
                                    go = Some(Action::NewIdentity);
                                    me.ux.rot_words = 1;
                                    me.ux.rot_words_open = false;
                                    me.ux.rot_confirm = Default::default();
                                }
                            }
                        }
                        close = rot_back_key(ui, &mut backed);
                    }
                    Stage::Declare { chain } => {
                        if step_key(ui, t(Key::Continue), !busy, at_once(&stage)) {
                            go = Some(Action::Rotate { step: RotateStep::Declare { chain: *chain } });
                        }
                    }
                    Stage::Write => {
                        if step_key(ui, t(Key::Continue), !busy, at_once(&stage)) {
                            go = Some(Action::Rotate { step: RotateStep::Write });
                        }
                    }
                    Stage::Anchoring { .. } | Stage::Accede { .. } => {
                        let (label, may) = on_chain_key(&stage, busy, me.send_now());
                        if step_key(ui, t(label), may, at_once(&stage)) {
                            on = Some(stage.clone());
                        }
                    }
                    Stage::Sweep { chain, send, then } => {
                        let value = send.filter(|_| held.is_none() && !to_changed);
                        if to_changed && held.is_none() {
                            if page::Page::new().primary_why(ui, t(Key::RotSweepToUse), !busy && !to_bad && !typed_to.is_empty(), &to_why).1.clicked() {
                                me.ux.rot_sweep_asked = typed_to.clone();
                                go = Some(Action::Rotate { step: RotateStep::SweepTo { to: typed_to.clone() } });
                            }
                        }
                        if let Some(value) = value {
                            let label = if handed.is_some() { Key::RotSweepOutGo } else { Key::RotFund };
                            if step_key(ui, t(label), !busy, at_once(&stage)) {
                                go = Some(Action::Rotate { step: RotateStep::Sweep { chain: *chain, value } });
                            }
                        }
                        match then.as_deref() {
                            Some(step) => {
                                let (label, may) = on_chain_key(step, busy, me.send_now());
                                let pressed = if value.is_some() { key::key(ui, t(label), Role::Secondary, may).clicked() } else { step_key(ui, t(label), may, at_once(step)) };
                                if pressed {
                                    on = Some(step.clone());
                                }
                            }
                            None => {
                                if held.is_none() && key::key(ui, t(Key::RotLeave), Role::Secondary, !busy).clicked() {
                                    go = Some(Action::Rotate { step: RotateStep::Leave { chain: *chain } });
                                }
                            }
                        }
                    }
                }
                match on {
                    Some(Stage::Anchoring { .. }) => {
                        let n = me.shell.sendable_now();
                        me.ux.rot_walked = true;
                        me.u3_open_confirm(U3Confirm::Send { count: n });
                    }
                    Some(Stage::Accede { chain }) => go = Some(Action::Rotate { step: RotateStep::Accede { chain } }),
                    _ => {}
                }
                if !backed && stage != Stage::Done {
                    close |= key::key(ui, t(Key::WizLater), Role::Secondary, true).clicked();
                }
                if !matches!(stage, Stage::Ready) && handed.is_none() && next.is_some() && foot_left(ui, t(Key::RotCancel)) {
                    go = Some(Action::Rotate { step: RotateStep::Cancel });
                }
            },
        );
        if close || out.esc {
            self.ux.rotate_open = false;
            self.ux.rotate_put_away = true;
            if self.ux.rot_words > 0 {
                self.ux.rot_words = 0;
                self.act(Action::DropFresh, now);
            }
            return;
        }
        if to_chain {
            self.ux.rotate_open = false;
            self.ux.rotate_put_away = true;
            self.go(Place::View(crate::nav::View::Works, crate::nav::tab::WORKS_PENDING), now);
            return;
        }
        if fetch {
            self.ux.rotate_open = false;
            self.ux.rotate_put_away = true;
            self.go(Place::Settings(crate::nav::Section::Data), now);
            return;
        }
        if back {
            self.shell.rot_vetted = None;
        }
        if to_network {
            self.ux.rotate_open = false;
            self.ux.rotate_put_away = true;
            self.go(Place::Settings(crate::nav::Section::Network), now);
            return;
        }
        if let Some(a) = go {
            let begin = matches!(a, Action::Rotate { step: RotateStep::Begin { .. } | RotateStep::Hand { .. } | RotateStep::Cancel });
            let wrote = matches!(a, Action::Rotate { step: RotateStep::Write });
            let sweep_to = matches!(a, Action::Rotate { step: RotateStep::SweepTo { .. } });
            self.ux.rot_walked |= matches!(a, Action::Rotate { step: RotateStep::Begin { .. } | RotateStep::Hand { .. } | RotateStep::Declare { .. } | RotateStep::Write | RotateStep::Accede { .. } | RotateStep::Sweep { .. } });
            let applied = self.act(a, now);
            if sweep_to {
                match applied {
                    Applied::Trouble(f) => self.ux.rot_sweep_said = Some(f.human().to_string()),
                    _ => {
                        self.ux.rot_sweep_said = None;
                        self.rot_look(now);
                    }
                }
            }
            if begin {
                self.ux.rot_words = 0;
                self.rot_look(now);
            }
            if begin || wrote {
                self.rot_pending_read();
            }
        }
    }

    pub(super) fn upgrade_sheet_open(&mut self) {
        self.sheets_clear();
        self.ux.upgrade_open = true;
        self.ux.upgrade_came_up = false;
        self.ux.upgrade_cost_for = None;
    }

    fn upgrade_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.upgrade_open {
            return;
        }
        let stage = self.shell.upgrade.clone().unwrap_or(crate::upgradex::Stage::Nothing);
        let at = stage.step().unwrap_or(0);
        let auto = self.ux.upgrade_auto;
        let early = self.shell.failed.get(&crate::task::Kind::Anchor).is_some_and(|f| f.which() == Some(crate::fault::Known::TooEarly));
        let leg = crate::upgradex::leg(&stage, &self.shell.queue);
        let read = self.shell.upgrade.is_some() && crate::upgradex::known(self.shell.upgrade_read, leg);
        if read && !auto && !matches!(stage, crate::upgradex::Stage::Done | crate::upgradex::Stage::Handed | crate::upgradex::Stage::Nothing) && self.ux.upgrade_cost_for.as_ref() != Some(&stage) {
            if self.act(Action::UpgradeCost, now) != Applied::Refused(crate::task::Kind::Rotate) {
                self.ux.upgrade_cost_for = Some(stage.clone());
            }
        }
        let cost = crate::upgradex::cost_now(&stage, read, leg, self.shell.upgrade_cost.as_ref());
        let costing = !read || self.shell.tasks.in_flight(crate::task::Kind::Rotate);
        let stopped = self.shell.failed.clone();
        let (mut go, mut close) = (false, false);
        let handed = stage == crate::upgradex::Stage::Handed;
        let done = stage == crate::upgradex::Stage::Done;
        let out = sheet::show(
            ctx,
            sheet::Spec::new("upgrade", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, t(Key::UpTitle), "");
                card::form(ui, |ui, f| {
                    if handed {
                        f.note(ui, t(Key::FaultWhatUpgradeHanded));
                        f.note(ui, t(Key::FaultNextUpgradeHanded));
                        return;
                    }
                    let steps = [Key::UpStepOld, Key::UpStepClose, Key::UpStepGenesis, Key::UpStepWrite];
                    for (i, k) in steps.iter().enumerate() {
                        let n = i + 1;
                        let (mark, said) = if n < at || done {
                            (Mark::Ok, String::new())
                        } else if n == at {
                            match &stage {
                                crate::upgradex::Stage::Anchor { owed } => (if auto { Mark::Busy } else { Mark::Todo }, fill1(Key::UpOwed, &owed.to_string())),
                                crate::upgradex::Stage::Closing { .. } | crate::upgradex::Stage::Continuing { .. } => {
                                    let leg = leg.unwrap_or(crate::upgradex::Leg::Unsent);
                                    let mark = if leg == crate::upgradex::Leg::Out || auto || early { Mark::Busy } else { Mark::Todo };
                                    (mark, t(crate::upgradex::leg_words(leg, early)).to_string())
                                }
                                _ => (if auto { Mark::Busy } else { Mark::Todo }, String::new()),
                            }
                        } else {
                            (Mark::Todo, String::new())
                        };
                        let _ = f.info(ui, t(*k), card::Show::Mark(mark, &said));
                    }
                    if !auto && !done {
                        match cost {
                            Some((txs, wei)) if txs > 0 => f.note(ui, &fill2(Key::UpConfirmSends, &txs.to_string(), &eth_cap(wei))),
                            Some(_) => {}
                            None if costing => f.note(ui, t(Key::PeekReading)),
                            None => {}
                        }
                    }
                    if !auto && !done {
                        if let Some(f2) = crate::upgradex::WALKED.iter().find_map(|k| stopped.get(k)).filter(|f| f.which() != Some(crate::fault::Known::TooEarly)) {
                            let _ = f.info(ui, t(Key::UpStopped), card::Show::Mark(Mark::Bad, &f2.human()));
                        }
                    }
                });
            },
            |ui, _me| {
                if done || handed {
                    close = page::Page::new().primary(ui, t(Key::WizFinish)).1.clicked();
                } else {
                    if !auto {
                        go = page::Page::new().primary_why(ui, t(Key::UpConfirmGo), cost.is_some(), t(Key::PeekReading)).1.clicked();
                    }
                    close = key::key(ui, t(Key::WizLater), Role::Secondary, true).clicked();
                }
            },
        );
        if close || out.esc {
            self.ux.upgrade_open = false;
            self.ux.upgrade_put_away = true;
        } else if go {
            self.ux.upgrade_auto = true;
            for k in crate::upgradex::WALKED {
                self.shell.failed.remove(&k);
            }
            self.upgrade_walk(now);
        }
    }

    pub(super) fn upgrade_walk(&mut self, now: f64) {
        use crate::task::Kind;
        use crate::upgradex::Stage;
        if !self.ux.upgrade_auto {
            return;
        }
        let stage = self.shell.upgrade.clone().unwrap_or(Stage::Nothing);
        if matches!(stage, Stage::Nothing | Stage::Done | Stage::Handed) {
            self.ux.upgrade_auto = false;
            return;
        }
        if [Kind::Anchor, Kind::Audit, Kind::Ledger, Kind::Gas].iter().any(|k| self.shell.tasks.in_flight(*k)) {
            return;
        }
        if crate::upgradex::WALKED.iter().any(|k| self.shell.failed.get(k).is_some_and(|f| f.which() != Some(crate::fault::Known::TooEarly))) {
            self.ux.upgrade_auto = false;
            return;
        }
        let n = self.shell.sendable_now();
        if matches!(self.send_now(), super::ledger::SendNow::Reading | super::ledger::SendNow::Out) {
            return;
        }
        if n > 0 {
            if self.shell.gas.map(|(c, _)| c) == Some(n) {
                self.act(Action::SendBatch { count: n }, now);
            } else {
                self.act(Action::EstimateGas { count: n }, now);
            }
            return;
        }
        match stage {
            Stage::Anchor { .. } | Stage::Close | Stage::Genesis => {
                if let Applied::Trouble(_) = self.act(Action::Upgrade, now) {
                    self.ux.upgrade_auto = false;
                }
            }
            Stage::Closing { id } | Stage::Continuing { id } if !self.shell.queue.items.iter().any(|q| q.id.eq_ignore_ascii_case(&id)) => {
                if let Applied::Trouble(_) = self.act(Action::Upgrade, now) {
                    self.ux.upgrade_auto = false;
                }
            }
            Stage::Closing { .. } | Stage::Continuing { .. } => {
                if now - self.ux.upgrade_read_at > 5.0 {
                    self.ux.upgrade_read_at = now;
                    let _ = self.act(Action::Audit, now);
                }
            }
            Stage::Nothing | Stage::Done | Stage::Handed => {}
        }
    }

    fn estimate_due(&mut self, ctx: &egui::Context) {
        let Some(count) = self.ux.u3.send_after_estimate else { return };
        if !matches!(self.ux.u3.confirm, Some(U3Confirm::Send { .. })) {
            self.ux.u3.send_after_estimate = None;
            return;
        }
        let wait = (tk::MID as f64 + 0.05) - (self.ux.now - self.ux.u3.estimate_from);
        if wait > 0.0 {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
            return;
        }
        match self.ux.u3.follow_audit {
            None if self.shell.queue.submitted().is_empty() && self.shell.queue_uncovered_now() => {
                if matches!(apply(&mut self.shell, Action::Audit), Applied::Started(_) | Applied::Refused(_)) {
                    self.ux.u3.follow_audit = Some(0);
                    ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    return;
                }
            }
            Some(stage @ (0 | 1)) => {
                if self.shell.tasks.in_flight(crate::task::Kind::Audit) {
                    ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    return;
                }
                if self.shell.catch_up_from_report() {
                    self.ux.u3.follow_audit = Some(2);
                    ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    return;
                }
                if stage == 0 && !self.shell.failed.contains_key(&crate::task::Kind::Audit) && !self.shell.audit_in_step() && matches!(apply(&mut self.shell, Action::Audit), Applied::Started(_)) {
                    self.ux.u3.follow_audit = Some(1);
                    ctx.request_repaint_after(std::time::Duration::from_millis(200));
                    return;
                }
            }
            Some(2) if self.shell.catching_up() => {
                ctx.request_repaint_after(std::time::Duration::from_millis(200));
                return;
            }
            _ => {}
        }
        let followed = self.ux.u3.follow_audit.take().is_some();
        let count = if followed {
            let n = self.shell.sendable_now().max(count);
            if let Some(U3Confirm::Send { count: shown }) = self.ux.u3.confirm.as_mut() {
                *shown = n;
            }
            n
        } else {
            count
        };
        self.ux.u3.send_after_estimate = None;
        let before = self.shell.faults.len();
        match apply(&mut self.shell, Action::EstimateGas { count }) {
            Applied::Started(_) | Applied::Refused(_) => self.ux.u3.estimating = Some(count),
            Applied::Trouble(f) => {
                if self.faults_told >= before {
                    self.faults_told = self.shell.faults.len();
                }
                self.ux.u3.confirm = Some(U3Confirm::NoEstimate { count, why: f.human().to_string(), next: f.next().to_string(), raw: f.shown() });
            }
            _ => {}
        }
    }

    pub(super) fn gas_back(&mut self, a: Applied) {
        let asked = self.ux.u3.estimating.take();
        let Applied::Trouble(f) = a else { return };
        let on_sheet = matches!((&self.ux.u3.confirm, asked), (Some(U3Confirm::Send { count }), Some(n)) if *count == n);
        if !on_sheet {
            return;
        }
        let n = self.shell.faults.len();
        if self.faults_told + 1 >= n {
            self.faults_told = n;
        }
        let count = asked.unwrap_or_default();
        self.ux.u3.confirm = Some(U3Confirm::NoEstimate { count, why: f.human().to_string(), next: f.next().to_string(), raw: f.shown() });
    }

    fn new_anchor_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if self.ux.u3.new_anchor.is_none() {
            return;
        }
        if self.typed.batch_files.len() == 1 && !self.shell.tasks.in_flight(crate::task::Kind::Take) && !self.shell.tasks.in_flight(crate::task::Kind::Record) {
            let one = std::mem::take(&mut self.typed.batch_files);
            self.take_for_record(one, now);
        }
        let content = self.shell.content.clone();
        let batch = self.typed.batch_files.clone();
        let ready = content.is_some() || !batch.is_empty();
        let hashing_now = self.shell.tasks.in_flight(crate::task::Kind::Take);
        let recording_now = self.shell.tasks.in_flight(crate::task::Kind::Record);
        let root = self.shell.home.as_ref().map(|h| h.root().display().to_string()).unwrap_or_default();
        let repo_say = match (&self.shell.settings.repo, self.shell.repo_since.as_ref()) {
            (None, _) => t(Key::U3RepoNone).to_string(),
            (Some(_), Some((_, Some(n)))) => fill1(Key::U3RepoGrew, &n.to_string()),
            (Some(_), _) => t(Key::U3RepoUnread).to_string(),
        };
        let for_say = {
            let f = &self.typed.for_fields;
            let said = f.iter().map(|x| x.trim()).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" \u{b7} ");
            (!said.is_empty()).then_some(said)
        };
        let auto = self.shell.settings.auto_anchor;
        let go = t(self.anchor_key());
        let cost = t(if auto { Key::U3CostSendNext } else { Key::U3CostQueue });
        let mut picked: Option<Vec<String>> = None;
        let (mut repo_picked, mut chose): (Option<String>, Option<String>) = (None, None);
        let (mut close, mut commit, mut ask_repo, mut check_repo, mut clear_batch) = (false, false, false, false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("new-record", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::V2NewAnchor), "");
                let dropped = drop::take_dropped(ui.ctx());
                if !dropped.is_empty() {
                    picked = Some(dropped);
                }
                if me.typed.repo_path.is_empty() {
                    if let Some(r) = &me.shell.settings.repo {
                        me.typed.repo_path = r.path.clone();
                    }
                }
                card::form(ui, |ui, f| {
                    let file = Some(card::Lead::Glyph(Glyph::File));
                    if !batch.is_empty() {
                        clear_batch = f.act(ui, t(Key::U3File), &fill1(Key::BatchChosen, &batch.len().to_string()), key::Key::new(t(Key::PickClear), Role::Secondary)).clicked();
                        for p in &batch {
                            let _ = f.line(ui, file, &width::file_name(p), "", false, false, c(C::Ink), |_| ());
                        }
                    } else {
                        match content.as_ref() {
                            Some(x) => {
                                let size = x.size.map(size_say).unwrap_or_else(|| x.detail.clone());
                                let name = width::file_name(&x.subject);
                                let room = (ui.available_width() * 0.55).max(40.0);
                                let _ = f.line(ui, file, t(Key::U3File), "", false, false, c(C::Ink), |ui| {
                                    if !size.is_empty() {
                                        paint::text(ui, &size, Type::Small, c(C::Ink3));
                                    }
                                    let shown = width::elide_mid(ui, &name, Type::Note.font(), room);
                                    let r = paint::text(ui, &shown, Type::Note, c(C::Ink));
                                    if shown != name {
                                        let _ = zikaron_ui::layer::tip(r, &name);
                                    }
                                });
                                let hex = x.hex();
                                if let Some(v) = f.info(ui, t(Key::ContentHash), card::Show::Short(&hex)) {
                                    tell_copied(ui.ctx(), Some(v));
                                }
                            }
                            None => {
                                let _ = f.line(ui, file, t(Key::U3File), "", false, false, c(C::Ink), |ui| paint::text(ui, t(Key::PickNone), Type::Note, c(C::Ink3)));
                            }
                        }
                    }
                    let _ = f.info(ui, t(Key::CfCost), card::Show::Text(cost));
                    details_row(ui, f, "new-record-details", &[(t(Key::CfWhere), Val::mono(root.clone()))]);
                });
                fields::stack(ui, |ui, st| {
                    st.field(ui, t(Key::U3WorkName), None, None, |ui, w| fields::line(ui, &mut me.typed.work_note, "", w, fields::Look::default()));
                });
                card::form(ui, |ui, f| {
                    let for_summary = for_say.clone().unwrap_or_else(|| t(Key::U3Optional).to_string());
                    let _ = f.fold(ui, "new-record-for", t(Key::ForTitle), &for_summary, |ui, f| {
                        f.panel(ui, |ui| {
                            fields::stack(ui, |ui, st| {
                                for (i, k) in [Key::ForApp, Key::ForIdentity, Key::ForRef, Key::ForSeatName].into_iter().enumerate() {
                                    st.field(ui, t(k), None, None, |ui, w| fields::line(ui, &mut me.typed.for_fields[i], "", w, fields::Look::default()));
                                }
                            });
                        });
                    });
                    let repo = me.typed.repo_path.trim().to_string();
                    let linked = if repo.is_empty() { t(Key::U3RepoNone).to_string() } else { width::file_name(&repo) };
                    let _ = f.fold(ui, "new-record-more", t(Key::U3MoreOptions), &linked, |ui, f| {
                        let sub = if repo.is_empty() { t(Key::PickNone).to_string() } else { width::file_name(&repo) };
                        ask_repo = f.act(ui, t(Key::RepoName), &sub, key::Key::new(t(Key::DoRegisterRepo), Role::Secondary)).clicked();
                        check_repo = f.act(ui, &repo_say, "", key::Key::new(t(Key::DoCheckRepo), Role::Secondary)).clicked();
                    });
                });
                repo_picked = path_answer(ui.ctx(), egui::Id::new("zikaron-path-new-record-repo"), ask_repo, crate::platform::Pick::Folder);
            },
            |ui, _me| {
                let busy = hashing_now || recording_now;
                let phase = if busy { Phase::Busy { frac: None } } else { Phase::Idle };
                let busy_say = t(if hashing_now { Key::U3Hashing } else { Key::U3Recording });
                let why = still_needs(&[(!ready, Key::RecordContent)]);
                commit = page::Pen::new().press_saying_why(ui, go, busy_say, ready, phase, &why).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                let asked = ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| key::key(ui, pick_label(crate::platform::Pick::FileOrFolder), Role::Secondary, true).clicked()).inner;
                chose = path_answer(ui.ctx(), egui::Id::new("zikaron-path-new-record"), asked, crate::platform::Pick::FileOrFolder);
            },
        );
        if let Some(paths) = picked.or(chose.map(|p| vec![p])) {
            self.take_for_record(paths, now);
        }
        if clear_batch {
            self.typed.batch_files.clear();
        }
        if let Some(p) = repo_picked {
            self.typed.repo_path = p;
            let a = Action::RegisterRepo { path: self.typed.repo_path.clone() };
            self.act(a, now);
        }
        if check_repo {
            self.act(Action::CheckRepo, now);
        }
        if close || out.esc {
            self.ux.u3.new_anchor = None;
        }
        if commit && !hashing_now && !recording_now {
            self.record_now(now);
        }
    }

    fn record_now(&mut self, now: f64) {
        let f = &self.typed.for_fields;
        let for_ = match crate::anchorx::For::from_fields(&f[0], &f[1], &f[2], &f[3]) {
            Ok(x) => x,
            Err(fault) => {
                let r = self.shell.trouble(fault);
                self.told(r, None, now);
                return;
            }
        };
        let files = self.typed.batch_files.clone();
        let a = Action::RecordWork { note_md: self.typed.work_note.clone(), files: files.clone(), for_ };
        let r = self.act(a, now);
        if let Applied::Started(_) = r {
            self.ux.u3.recording = Some(files);
            return;
        }
        self.ux.u3.new_anchor = None;
        self.recorded(r, &files);
    }

    fn recorded(&mut self, r: Applied, files: &[String]) {
        match r {
            Applied::Recorded { .. } | Applied::RecordedBatch { stopped: None, .. } => self.anchor_form_clear(),
            Applied::RecordedBatch { stopped: Some((i, _, _)), .. } => self.typed.batch_files = files[i..].to_vec(),
            _ => {}
        }
    }

    pub(super) fn said_back(&mut self, k: crate::task::Kind, a: Applied, now: f64) {
        use crate::task::Kind;
        match k {
            Kind::Gas => self.gas_back(a),
            Kind::Take => {
                let a = self.told(a, None, now);
                if let Applied::Took { .. } = a {
                    self.took_named();
                }
            }
            Kind::Record => {
                let a = self.told(a, None, now);
                let files = self.ux.u3.recording.take().unwrap_or_default();
                if !matches!(a, Applied::Trouble(_)) {
                    self.ux.u3.new_anchor = None;
                }
                self.recorded(a, &files);
            }
            _ => {
                self.told(a, None, now);
            }
        }
    }

    fn send_sheet(&mut self, ctx: &egui::Context, count: usize, failed: Option<(Key, String, String, String)>, now: f64) {
        if let Some(stamp) = self.ux.u3.sending {
            let gone = self.shell.gas.map(|(n, _)| n != count).unwrap_or(true);
            let landed = self.shell.tasks.answered_since(crate::task::Kind::Anchor, stamp);
            if gone || landed {
                self.ux.u3.sending = None;
                self.ux.u3.confirm = self.shell.failed.get(&crate::task::Kind::Anchor).filter(|_| landed).map(|f| not_sent(count, f));
                if self.ux.u3.confirm.is_some() {
                    self.ux.u3.follow = None;
                }
                return;
            }
        }
        let sending = self.ux.u3.sending.is_some();
        let out = self.shell.tasks.in_flight(crate::task::Kind::Gas);
        let held = self.shell.gas.map(|(n, _)| n == count).unwrap_or(false);
        if !sending && !out && !held && failed.is_none() && self.ux.u3.send_after_estimate.is_none() {
            self.ux.u3.estimating = None;
            self.ux.u3.send_after_estimate = Some(count);
            self.ux.u3.estimate_from = self.ux.now;
        }
        let asking = self.ux.u3.send_after_estimate == Some(count);
        let gas = self.shell.gas.filter(|(n, _)| *n == count && !asking).map(|(_, g)| g);
        let cap_say = eth_said(self.shell.fees.unwrap_or_else(zikaron_anchor::send::Fees::fallback).cap_wei());
        let fees_now = self.shell.fees;
        let fees_left = self.shell.fees_left.clone();
        let phase = if sending {
            self.phase_of(crate::task::Kind::Anchor)
        } else if out || asking {
            Phase::Busy { frac: None }
        } else {
            Phase::Idle
        };
        let first = fill1(Key::U3FirstN, &count.to_string());
        let then = self.ux.u3.follow.clone().filter(|e| self.goes_later(e, count)).map(|e| fill1(Key::PsSendThen, &super::persona::row_named(&self.shell, &e)));
        let busy = self.busy_said(sending, &[crate::task::Kind::Gas]);
        let (mut go, mut close, mut retry) = (false, false, false);
        let step = u64::from(failed.is_some());
        let slide = if failed.is_some() { sheet::Slide::Forward } else { sheet::Slide::Back };
        let out = sheet::show(
            ctx,
            sheet::Spec::new("send", tk::SHEET_W).step(step, slide),
            self,
            |ui, _me| match &failed {
                None => {
                    sheet::title(ui, t(Key::U3SendTitle), "");
                    let gas_say = gas.map(|g| g.to_string()).unwrap_or_default();
                    card::form(ui, |ui, f| {
                        let _ = f.info(ui, t(Key::LedgerEntries), card::Show::Text(&first));
                        if let Some(then) = &then {
                            let _ = f.info(ui, then, card::Show::Empty(""));
                        }
                        let _ = f.info(ui, t(Key::U3GasEstimate), if gas.is_some() { card::Show::Mono(&gas_say) } else { card::Show::Empty(t(Key::U3Estimating)) });
                        let _ = f.info(ui, t(Key::U3FeeCap), if gas.is_some() { card::Show::Mono(&cap_say) } else { card::Show::Empty(t(Key::U3Estimating)) });
                    });
                    if let (Some(_), Some(k)) = (gas, crate::chainx::fee_source_line(fees_now.as_ref())) {
                        zikaron_ui::callout::callout(ui, t(k));
                    }
                    if gas.is_some() && !fees_left.is_empty() {
                        zikaron_ui::callout::callout(ui, &fill1(Key::U3FeeLeftNodes, &fees_left.join(" · ")));
                    }
                }
                Some((title, why, next, raw)) => {
                    sheet::title(ui, t(*title), "");
                    err_call(ui, "no-estimate", why, next, "", "");
                    card::form(ui, |ui, f| {
                        let _ = f.info(ui, t(Key::LedgerEntries), card::Show::Text(&first));
                        if !raw.is_empty() {
                            details_row(ui, f, "no-estimate-details", &[(t(Key::U3NoEstimateWhy), Val::mono(raw.clone()))]);
                        }
                    });
                }
            },
            |ui, me| {
                if failed.is_none() {
                    go = page::Pen::new().press_long_saying(ui, t(Key::U3SendGo), busy, gas.is_some() && !sending, phase).clicked();
                    close = key::key(ui, t(Key::CfBack), Role::Secondary, !sending).clicked();
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| me.stage_line(ui, crate::task::Kind::Anchor));
                } else {
                    retry = page::Page::new().primary(ui, t(Key::U3Retry)).1.clicked();
                    close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                }
            },
        );
        if close || (out.esc && !sending) {
            self.ux.u3.confirm = None;
            self.ux.u3.send_after_estimate = None;
            self.ux.u3.follow = None;
            return;
        }
        if retry {
            self.ux.u3.confirm = Some(U3Confirm::Send { count });
            self.ux.u3.send_after_estimate = Some(count);
            self.ux.u3.estimate_from = self.ux.now;
        }
        if go {
            self.send_go(count, now);
        }
    }

    pub(super) fn send_go(&mut self, count: usize, now: f64) {
        match self.act(Action::SendBatch { count }, now) {
            Applied::Started(_) => self.ux.u3.sending = Some(self.shell.tasks.landings(crate::task::Kind::Anchor)),
            Applied::Trouble(f) => self.ux.u3.confirm = Some(not_sent(count, &f)),
            _ => self.ux.u3.confirm = None,
        }
    }

    fn bump_sheet(&mut self, ctx: &egui::Context, tx: &str, now: f64) {
        let offer = self.shell.stuck.clone().filter(|s| s.txs.last().map(String::as_str) == Some(tx));
        let Some(stuck) = offer else {
            self.ux.u3.confirm = None;
            self.ux.u3.bump_press = None;
            return;
        };
        let (fees, base_now, unheld) = match stuck.offer {
            crate::task::Offer::Resend { fees, base_now } => (fees, base_now, false),
            crate::task::Offer::Unheld { fees, base_now } => (fees, base_now, true),
            _ => {
                self.ux.u3.confirm = None;
                self.ux.u3.bump_press = None;
                return;
            }
        };
        let entries = self.shell.queue.items.iter().filter(|q| q.step.awaited().is_some_and(|(t, _)| t == stuck.txs)).count();
        let waiting = self.ux.u3.bump_press.is_some();
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("bump", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, t(if unheld { Key::U3ResendKey } else { Key::U3BumpTitle }), "");
                let at_gas = |per_gas: u64| eth_said(u128::from(per_gas) * u128::from(stuck.sent.gas_limit));
                card::form(ui, |ui, f| {
                    let _ = f.info(ui, t(Key::LedgerEntries), card::Show::Text(&entries.to_string()));
                    if !unheld {
                        let _ = f.info(ui, t(Key::U3BumpOldCap), card::Show::Mono(&at_gas(stuck.sent.max_fee)));
                    }
                    let _ = f.info(ui, t(Key::U3BumpPriceNow), card::Show::Mono(&at_gas(base_now)));
                    let _ = f.info(ui, t(Key::U3BumpNewCap), card::Show::Mono(&eth_said(fees.cap_wei())));
                    details_row(ui, f, "bump-details", &[(t(Key::U3GasEstimate), Val::mono(stuck.sent.gas_limit.to_string())), (t(Key::SendTx), Val::mono(stuck.txs.join("\n")))]);
                });
                if !stuck.left.is_empty() {
                    zikaron_ui::callout::callout(ui, &fill1(Key::U3FeeLeftNodes, &stuck.left.iter().map(crate::chainx::said_of).collect::<Vec<_>>().join(" · ")));
                }
            },
            |ui, _me| {
                let phase = if waiting { Phase::Busy { frac: None } } else { Phase::Idle };
                go = page::Pen::new().press_long_saying(ui, t(if unheld { Key::U3ResendKey } else { Key::U3BumpKey }), t(Key::U3BumpWaiting), !waiting, phase).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.ux.u3.confirm = None;
            self.ux.u3.bump_press = None;
            return;
        }
        if go {
            if self.shell.tasks.in_flight(crate::task::Kind::Anchor) {
                self.ux.u3.bump_press = Some((tx.to_string(), fees.max_fee));
            } else {
                self.bump_go(tx.to_string(), fees.max_fee, now);
            }
        }
    }

    pub(super) fn bump_go(&mut self, tx: String, cap: u64, now: f64) {
        let _ = self.act(Action::BumpFee { tx, cap }, now);
        self.ux.u3.confirm = None;
    }

    fn delete_record_sheet(&mut self, ctx: &egui::Context, subject: &str, now: f64) {
        let chain_now = self.shell.chain_now();
        let row = self.shell.rows.as_ref().and_then(|(r, _)| r.iter().find(|x| x.id == subject).cloned());
        let root = self.shell.home.as_ref().map(|h| h.root().display().to_string()).unwrap_or_default();
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("delete-record", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, t(Key::V2DeleteTitle), "");
                card::form(ui, |ui, f| {
                    let _ = f.info(ui, t(Key::U3Work), card::Show::Text(&row.as_ref().map(|x| human_summary(x, chain_now)).unwrap_or_default()));
                    details_row(ui, f, "delete-details", &[(t(Key::ContentHash), Val::mono(row.as_ref().and_then(|x| x.work.clone()).unwrap_or_default())), (t(Key::CfWhere), Val::mono(root.clone()))]);
                });
            },
            |ui, _me| {
                go = page::Erase::press(ui, t(Key::V2DeleteGo), true).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.ux.u3.confirm = None;
        } else if go {
            self.ux.u3.confirm = None;
            self.act(Action::Retract { subject: subject.to_string(), note_md: String::new() }, now);
        }
    }

    fn grant_confirm_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let upstream = self.typed.g_upstream.trim().to_string();
        let record = if upstream.is_empty() { self.work_label(self.typed.g_work.trim()) } else { self.held_record_name(&upstream) };
        let window = match (self.typed.g_from.trim().parse::<u64>(), self.typed.g_to.trim().parse::<u64>()) {
            (Ok(a), Ok(b)) => Some((a, b)),
            _ => None,
        };
        let grantee = self.typed.g_grantee.trim().to_string();
        let terms = self.ux.u3.terms.as_ref().filter(|x| x.hex == self.typed.g_terms.trim()).map(|x| x.name.clone()).unwrap_or_else(|| t(Key::TermsTyped).to_string());
        let terms_ok = self.ux.u3.terms.as_ref().is_some_and(|x| x.hex == self.typed.g_terms.trim());
        let more = [
            t(if self.typed.g_exclusive { Key::ExclusiveMark } else { Key::GrantSumNonExclusive }).to_string(),
            if upstream.is_empty() { t(Key::GrantSumNoUpstream).to_string() } else { head_tail(&upstream) },
            t(if self.typed.g_scope.trim().is_empty() { Key::GrantSumNoScope } else { Key::GrantSumScope }).to_string(),
        ]
        .join(" \u{b7} ");
        let cost = t(if self.shell.settings.auto_anchor { Key::U3CostSendNext } else { Key::U3CostQueue });
        let go_label = t(self.anchor_key());
        let mut raw: Vec<(&str, Val)> = Vec::new();
        if !upstream.is_empty() {
            raw.push((t(Key::U4Upstream), Val::mono(upstream.clone())));
        }
        raw.push((t(Key::U3ToWhom), Val::mono(grantee.clone())));
        raw.push((t(Key::ContentHash), Val::mono(self.typed.g_work.trim().to_string())));
        raw.push((t(Key::U3TermsHash), Val::mono(self.typed.g_terms.trim().to_string())));
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("grant-confirm", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, t(if upstream.is_empty() { Key::U3GrantKey } else { Key::U4RelicenseGo }), "");
                let mut copied = None;
                card::form(ui, |ui, f| {
                    copied = f.info(ui, t(Key::U3ToWhom), card::Show::Short(&grantee));
                    let _ = f.info(ui, t(Key::U3Work), card::Show::Text(&record));
                    f.with(ui, t(Key::U3Window), "", |ui| grants::span_trail_days(ui, window));
                    let _ = f.info(ui, t(Key::U3TermsDoc), if terms_ok { card::Show::Text(&terms) } else { card::Show::Empty(&terms) });
                    f.info_words(ui, t(Key::U3GrantMore), &more);
                    let _ = f.info(ui, t(Key::CfCost), card::Show::Text(cost));
                });
                tell_copied(ui.ctx(), copied);
                card::form(ui, |ui, f| details_row(ui, f, "grant-confirm-details", &raw));
            },
            |ui, _me| {
                go = page::Pen::new().press(ui, go_label, true).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.ux.u3.confirm = None;
        } else if go {
            self.ux.u3.confirm = None;
            let _ = self.grant_go(now);
        }
    }

    pub(super) fn grant_go(&mut self, now: f64) -> Applied {
        let terms_file = self.ux.u3.terms.as_ref().filter(|x| x.hex == self.typed.g_terms.trim()).map(|x| x.path.clone());
        let a = Action::DraftGrant { draft: Box::new(self.draft()), exclusive: self.typed.g_exclusive, terms_file };
        let got = self.act(a, now);
        match &got {
            Applied::Granted { .. } => self.grant_signed(),
            Applied::Started(crate::task::Kind::Gate) => self.ux.u4.grant_in_gate = true,
            _ => {}
        }
        got
    }

    fn revoke_sheet(&mut self, ctx: &egui::Context, grant: &str, now: f64) {
        let g = self.shell.grants.as_ref().and_then(|x| x.iter().find(|r| r.id == grant).cloned());
        let record = g.as_ref().map(|x| self.work_label(&x.work)).unwrap_or_default();
        let window = grants::span_say(g.as_ref().and_then(|x| x.window));
        let grantee = g.as_ref().map(|x| x.grantee.clone()).unwrap_or_default();
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("revoke", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::U3RevokeTitle), "");
                let mut copied = None;
                card::form(ui, |ui, f| {
                    let _ = f.info(ui, t(Key::U3Work), card::Show::Text(&record));
                    let _ = f.info(ui, t(Key::U3OldWindow), card::Show::Text(&window));
                    copied = if grantee.is_empty() { f.info(ui, t(Key::U3ToWhom), card::Show::Empty(t(Key::WbNotRead))) } else { f.info(ui, t(Key::U3ToWhom), card::Show::Short(&grantee)) };
                });
                tell_copied(ui.ctx(), copied);
                fields::stack(ui, |ui, st| {
                    st.field(ui, t(Key::RevokeCaseName), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut me.ux.u3.revoke_case, "0x\u{2026}", w, fields::Look { mono: true, ..Default::default() }));
                });
            },
            |ui, _me| {
                go = page::Pen::new().press(ui, t(Key::U3RevokeGo), true).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.ux.u3.confirm = None;
        } else if go {
            self.ux.u3.confirm = None;
            let a = Action::Revoke { grant: grant.to_string(), case: std::mem::take(&mut self.ux.u3.revoke_case) };
            self.act(a, now);
        }
    }

    fn clash_sheet(&mut self, ctx: &egui::Context) {
        if !self.clash_modal {
            return;
        }
        let rows = self.shell.clash.clone();
        let mut close = false;
        let out = sheet::show(
            ctx,
            sheet::Spec::new("clash", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, t(Key::ClashTitle), "");
                err_call(ui, "clash-what", t(Key::ClashBody), t(Key::ExclusiveNote), t(Key::U3RawError), &rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>().join("\n"));
                card::form(ui, |ui, f| {
                    for r in &rows {
                        f.row_with(ui, Some(Mark::Bad), &fill2(Key::ClashRow, &r.seq.to_string(), &grants::span_say(r.window)), "", card::Value::Text(t(Key::ExclusiveMark)), |_| {});
                    }
                });
            },
            |ui, _me| {
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.clash_modal = false;
        }
    }

    fn annotate_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let chain_now = self.shell.chain_now();
        self.ensure_rows(now);
        let rows: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
        let read = self.shell.rows.is_some();
        let reading = crate::retractx::read(&rows);
        let (mut go, mut close) = (false, false);
        let why = self.ux.annotate_why.clone();
        let note_line = why.as_ref().filter(|(entry, _)| !entry).map(|_| ("", true));
        let entry_line = why.as_ref().filter(|(entry, _)| *entry).map(|(_, f)| (f.human(), true));
        let was = (self.typed.annotate_note.clone(), self.typed.annotate_subject.clone());
        let out = sheet::show(
            ctx,
            sheet::Spec::new("annotate", tk::SHEET_W).shake(self.ux.sheet_shake),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::U3AnnotateTitle), "");
                fields::stack(ui, |ui, st| {
                    st.field(ui, t(Key::AnnotateText), None, note_line, |ui, _| input::area_words(ui, &mut me.typed.annotate_note, 3, ""));
                    st.field(ui, t(Key::U3AnnotateWhich), None, entry_line, |ui, w| {
                        let faces: Vec<(String, String, String)> = std::iter::once((String::new(), t(Key::LedgerAnnotateNone).to_string(), String::new()))
                            .chain(rows.iter().map(|row| {
                                let (tag, summary, _) = row_face(&rows, &reading, row, chain_now);
                                let at = row.anchored_at.map(crate::when::when).unwrap_or_else(|| t(Key::V2StateLanded).to_string());
                                (format!("#{}", row.seq), format!("{tag} \u{b7} {summary}"), at)
                            }))
                            .collect();
                        let items: Vec<menu::Item> = faces.iter().map(|(seq, label, at)| menu::Item::Row(menu::Row { lead: seq, label, trail: at, ..Default::default() })).collect();
                        let words = date_words();
                        let spec = pick::Spec {
                            hint: t(Key::SearchLedger),
                            empty: t(Key::SearchNone),
                            w: PICK_WIDE_W,
                            dates: Some(pick::Dates { words: &words, today: today_of((me.shell.clock)()), from_hint: t(Key::DateFrom), to_hint: t(Key::DateTo) }),
                        };
                        let keep = |i: usize, q: &str, from: &str, to: &str| match i.checked_sub(1) {
                            None => matches(q, &[&faces[0].1]),
                            Some(r) => matches(q, &[&faces[i].0, &faces[i].1, &rows[r].id]) && crate::when::within(rows[r].anchored_at, from, to),
                        };
                        let current = rows.iter().position(|r| me.typed.annotate_subject.trim().eq_ignore_ascii_case(&r.id)).map(|r| r + 1);
                        let mut shown = current.map(|i| format!("{} {}", faces[i].0, faces[i].1)).unwrap_or_else(|| if read && me.typed.annotate_subject.trim().is_empty() { faces[0].1.clone() } else { String::new() });
                        let hint = if read { t(Key::U3AnnotatePickHint) } else { t(Key::U3AnnotateNoRows) };
                        let look = fields::Look { icon: Some(Glyph::File), read_only: true, ..Default::default() };
                        let (_, picked) = fields::joined(ui, &mut shown, hint, w, look, Some(|ui: &mut egui::Ui| zikaron_ui::formparts::inner_pick(ui, "annotate-pick", t(Key::KitPickOpen), false, true, read, &spec, &items, &keep)));
                        if let Some(i) = picked.flatten() {
                            me.typed.annotate_subject = i.checked_sub(1).map(|r| rows[r].id.clone()).unwrap_or_default();
                        }
                    });
                    zikaron_ui::fields::rows(ui, |ui, f| {
                        f.fold(ui, "annotate-more", t(Key::U3MoreOptions), "", |ui, f| {
                            f.panel(ui, |ui| {
                                fields::stack(ui, |ui, st| {
                                    st.field(ui, t(Key::U3AnnotateTyped), None, None, |ui, w| fields::line(ui, &mut me.typed.annotate_subject, t(Key::DetailPickHint), w, fields::Look { mono: true, ..Default::default() }));
                                });
                            });
                        });
                    });
                });
            },
            |ui, _me| {
                go = page::Pen::new().press(ui, t(Key::U3AnnotateGo), true).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if (self.typed.annotate_note.clone(), self.typed.annotate_subject.clone()) != was {
            self.ux.annotate_why = None;
        }
        if close || out.esc {
            self.ux.u3.form = None;
            self.ux.sheet_shake = None;
            self.ux.annotate_why = None;
        } else if go {
            let a = Action::Annotate { subject: self.typed.annotate_subject.clone(), note_md: self.typed.annotate_note.clone() };
            match self.act(a, now) {
                Applied::Annotated(_) => {
                    self.ux.u3.form = None;
                    self.ux.sheet_shake = None;
                    self.ux.annotate_why = None;
                    self.typed.annotate_note.clear();
                    self.typed.annotate_subject.clear();
                }
                Applied::Trouble(f) => {
                    self.ux.sheet_shake = Some(now);
                    use crate::fault::Known;
                    self.ux.annotate_why = match f.which() {
                        Some(Known::EntryRefused) if self.typed.annotate_note.trim().is_empty() => Some((false, f)),
                        Some(Known::ContentShape | Known::SubjectMissing | Known::NotATarget) => Some((true, f)),
                        _ => None,
                    };
                }
                _ => self.ux.sheet_shake = Some(now),
            }
        }
    }

    fn add_grant_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.u4.import_open {
            return;
        }
        let err = self.ux.u4.import_err.clone();
        let typed = self.typed.vt_typed.trim().to_string();
        if self.ux.u4.grant_file_seen.as_ref().map(|(k, _)| *k != typed).unwrap_or(true) {
            if typed.is_empty() {
                self.ux.u4.grant_file_seen = Some((typed.clone(), None));
            } else if let Some(crate::peekx::Got::GrantFile(read)) = self.peek(crate::peekx::Ask::GrantFile { path: typed.clone() }, now) {
                let seen = read.map(|r| r.map(|(hops, terms)| fill3(Key::GrantFileVerified, &width::file_name(&typed), &hops.to_string(), &terms.to_string())));
                self.ux.u4.grant_file_seen = Some((typed.clone(), seen));
            }
        }
        let seen = self.ux.u4.grant_file_seen.as_ref().filter(|(k, _)| *k == typed).and_then(|(_, x)| x.clone());
        let code = typed.to_ascii_lowercase().starts_with(zikaron_kit::tokens::BADGE_PREFIX);
        let check: Option<(String, bool)> = match &seen {
            Some(Ok(said)) => Some((said.clone(), false)),
            Some(Err(f)) => Some((f.human().to_string(), true)),
            None if code => Some((t(Key::V2CodeCarriesNoLedger).to_string(), false)),
            None => None,
        };
        let file = seen.is_some().then(|| width::file_name(&typed));
        let code_line = check;
        let (mut go, mut close) = (false, false);
        let (mut dropped, mut picked): (Option<String>, Option<String>) = (None, None);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("add-grant", tk::SHEET_W).shake(self.ux.sheet_shake),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::V2AddGrant), "");
                dropped = drop::take_dropped(ui.ctx()).into_iter().next();
                fields::stack(ui, |ui, st| {
                    st.field(ui, t(Key::U3CheckPaste), None, code_line.as_ref().map(|(s, bad)| (s.as_str(), *bad)), |ui, _| {
                        if file.is_some() {
                            let mut fresh = String::new();
                            let _ = input::area_hint(ui, &mut fresh, 3, t(Key::V2PasteGrant));
                            if !fresh.is_empty() {
                                me.typed.vt_typed = fresh;
                            }
                        } else {
                            let _ = input::area_hint(ui, &mut me.typed.vt_typed, 3, t(Key::V2PasteGrant));
                        }
                    });
                    card::form(ui, |ui, f| {
                        let _ = f.line(ui, Some(card::Lead::Glyph(Glyph::File)), t(Key::GrantFileRow), "", false, false, c(C::Ink), |ui| match file.as_deref() {
                            Some(n) => ui.label(egui::RichText::new(n).font(Type::Note.font()).color(c(C::Ink))),
                            None => ui.label(egui::RichText::new(t(Key::PickNone)).font(Type::Note.font()).color(c(C::Ink3))),
                        });
                    });
                    st.field(ui, t(Key::V2GrantNote), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut me.typed.vt_note, t(Key::AddGrantNoteHint), w, fields::Look::default()));
                    st.field(ui, t(Key::V2IssuerNote), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut me.typed.vt_issuer_note, t(Key::AddIssuerNoteHint), w, fields::Look::default()));
                });
                if let Some((what, next, raw)) = err.as_ref() {
                    states::err_box(ui, "add-grant-err", what, next, t(Key::U3RawError), raw);
                }
            },
            |ui, me| {
                let why = still_needs(&[(me.typed.vt_typed.trim().is_empty(), Key::U3CheckPaste)]);
                go = page::Page::new().primary_why(ui, t(Key::V2AddGrantGo), !me.typed.vt_typed.trim().is_empty(), &why).1.clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                picked = foot_pick(ui, "add-grant", crate::platform::Pick::File);
            },
        );
        if let Some(p) = dropped.or(picked) {
            self.typed.vt_typed = p;
            self.ux.u4.import_err = None;
        }
        if close || out.esc {
            self.ux.u4.import_open = false;
            self.ux.u4.import_err = None;
            self.ux.sheet_shake = None;
        } else if go {
            match self.act(Action::ImportGrant { typed: self.typed.vt_typed.clone() }, now) {
                Applied::Held { grant, .. } | Applied::HeldRenewed { grant } => {
                    self.ux.u4.import_open = false;
                    self.ux.u4.import_err = None;
                    self.ux.sheet_shake = None;
                    self.typed.vt_typed.clear();
                    let (note, issuer_note) = (std::mem::take(&mut self.typed.vt_note), std::mem::take(&mut self.typed.vt_issuer_note));
                    if !grant.is_empty() && (!note.trim().is_empty() || !issuer_note.trim().is_empty()) {
                        self.act(Action::NoteHeld { grant, note, issuer_note }, now);
                    }
                }
                Applied::Trouble(f) => {
                    self.ux.sheet_shake = Some(now);
                    self.ux.u4.import_err = Some(match self.import_face(&f) {
                        Some((what, next)) => (what.to_string(), next.to_string(), f.shown()),
                        None => (f.human().to_string(), f.next().to_string(), f.shown()),
                    });
                }
                _ => {}
            }
        }
    }

    fn relicense_sheet(&mut self, ctx: &egui::Context, now: f64) {
        self.out_sheet(ctx, now);
        let Some(U4Confirm::Relicense { grant }) = self.ux.u4.confirm.clone() else { return };
        let h = self.shell.held.clone().unwrap_or_default().into_iter().find(|x| x.id.eq_ignore_ascii_case(&grant));
        let record = self.held_record_name(&grant);
        let issuer = self.held_issuer_name(&grant);
        let window = h.as_ref().and_then(|x| x.window);
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("relicense", tk::SHEET_WIDE),
            self,
            |ui, _me| {
                sheet::title(ui, t(Key::U4RelicenseTitle), "");
                card::form(ui, |ui, f| {
                    let _ = f.info(ui, t(Key::U3Work), card::Show::Text(&record));
                    let _ = f.info(ui, t(Key::U3Issuer), card::Show::Text(&issuer));
                    f.with(ui, t(Key::U4WindowCap), "", |ui| grants::span_trail(ui, window));
                });
                card::form(ui, |ui, f| {
                    details_row(
                        ui,
                        f,
                        "relicense-details",
                        &[
                            (t(Key::U4Upstream), Val::mono(grant.clone())),
                            (t(Key::U3Issuer), Val::mono(h.as_ref().map(|x| x.author.clone()).unwrap_or_default())),
                            (t(Key::U3Work), Val::mono(h.as_ref().map(|x| x.work.clone()).unwrap_or_default())),
                        ],
                    )
                });
            },
            |ui, _me| {
                go = page::Guide::key(ui, t(Key::U4ToDrafter), h.is_some()).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.ux.u4.confirm = None;
        } else if go {
            self.ux.u4.confirm = None;
            if let Some(h) = h {
                let d = crate::relicx::prefill(&h);
                self.grant_form_fresh();
                self.typed.g_upstream = d.upstream;
                self.typed.g_work = d.work;
                self.typed.g_history.clear();
                self.ux.u4.relicense_signed = false;
            }
            self.push(Route::Relicense, now);
        }
    }
}

fn swept_rows(shell: &crate::shell::Shell) -> Vec<(u64, zikaron_ui::layer::Amount)> {
    use crate::rotatex::Settled;
    crate::action::swept_now(shell)
        .into_iter()
        .map(|(chain, how)| {
            let said = match how {
                Settled::Sent { value, .. } => no_amount(fill1(if shell.handed.is_some() { Key::RotSweepOutSent } else { Key::RotSweepSent }, &eth_exact(value))),
                Settled::Short { held, .. } => held_amount(held, |x| fill1(Key::RotSweepShort, x)),
                Settled::Left { held: Some(held) } => held_amount(held, |x| fill1(Key::RotSweepLeft, x)),
                Settled::Left { held: None } => no_amount(t(Key::RotSweepLeftUnread)),
            };
            (chain, said)
        })
        .collect()
}

fn at_once(stage: &crate::rotatex::Stage) -> bool {
    matches!(stage, crate::rotatex::Stage::Declare { .. } | crate::rotatex::Stage::Accede { .. } | crate::rotatex::Stage::Sweep { .. })
}

fn on_chain_key(stage: &crate::rotatex::Stage, busy: bool, now: super::ledger::SendNow) -> (Key, bool) {
    use super::ledger::SendNow;
    match (stage, now) {
        (crate::rotatex::Stage::Accede { .. }, _) => (Key::RotAccede, !busy),
        (_, SendNow::Reading) => (Key::SendableReading, false),
        (_, SendNow::Out) => (Key::U3Sending, false),
        (_, SendNow::Can(_)) => (Key::Continue, !busy),
        (_, SendNow::Nothing) => (Key::Continue, false),
    }
}

fn not_sent(count: usize, f: &crate::fault::Fault) -> U3Confirm {
    U3Confirm::NotSent { count, why: f.human().to_string(), next: f.next(), raw: f.shown() }
}

fn err_call(ui: &mut egui::Ui, id_salt: &str, what: &str, next: &str, raw_title: &str, raw: &str) {
    const MARK: f32 = 16.0;
    const LINE: f32 = 19.0;
    egui::Frame::new().fill(c(C::BadWash)).corner_radius(tk::Radius::Tag.egui()).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 10.0;
            let (spot, _) = ui.allocate_exact_size(egui::vec2(MARK, LINE), egui::Sense::hover());
            mark::paint_mark(ui.ctx(), ui.painter(), spot.center(), Mark::Bad, MARK);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                let room = ui.available_width();
                ui.label(egui::RichText::new(what).font(Type::Strong.font()).color(c(C::BadInk)));
                if !next.is_empty() {
                    let g = paint::wrapped(ui, next, Type::Small.font(), c(C::Ink2), LINE, room);
                    let (r, _) = ui.allocate_exact_size(g.size(), egui::Sense::hover());
                    ui.painter().galley(r.min, g, c(C::Ink2));
                }
                if !raw.is_empty() {
                    ui.add_space(4.0);
                    fold::fold(ui, id_salt, raw_title, |ui| paint::text(ui, raw, Type::MonoSmall, c(C::Ink3)));
                }
            });
        });
    });
}
