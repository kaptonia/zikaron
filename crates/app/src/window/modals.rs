//! The sheets of both seats' pages: new record (two steps), send (gas estimate first), delete record, grant
//! and revoke confirmations, the exclusive-grant clash, import existing records, key change or handover (two
//! steps), annotate, attest for someone, add a held grant, and the sublicense confirmation.
//!
//! One sheet at a time: opening one closes the others. Sheets lie above the first-run wizard, which stays
//! open under them; while the passcode gate is up no sheet is drawn.

use super::*;
use zikaron_ui::sheetparts as parts;

impl Win {
    // ─── Which sheet is open ───

    /// Whether any sheet is open (keyboard shortcuts and the whole-window drop stand aside).
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
            || self.ux.confirm_genesis
            || self.ux.confirm_adopt
            || self.net_sheet_open()
            || self.ux.dir_sheet.is_some()
            || self.ux.snapshot_sheet
            || self.ux.out_sheet.is_some()
    }

    /// Close every sheet; the wizard stays.
    pub(super) fn sheets_clear(&mut self) {
        self.ux.u3.new_anchor = None;
        self.ux.u3.confirm = None;
        self.ux.u3.send_after_estimate = None;
        self.ux.u3.sending = None;
        self.ux.u3.bump_press = None;
        self.ux.u3.form = None;
        self.ux.u3.succeed_confirm = false;
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
        self.ux.sheet_shake = None;
        self.ux.bk_clear();
        self.cli_close();
        self.net_sheets_clear();
        self.ux.dir_sheet = None;
        self.ux.dir_waiting = false;
        self.ux.snapshot_sheet = false;
        self.ux.out_sheet = None;
        self.ux.out_waiting = false;
    }

    /// Whether any sheet is up (Esc belongs to it, not to the layer under it).
    pub(super) fn any_sheet_open(&self) -> bool {
        self.ux.id_modal.is_some()
            || self.ux.bk.is_some()
            || self.ux.confirm_genesis
            || self.ux.confirm_adopt
            || self.ux.wiz_exit_ask
            || self.ux.u3.new_anchor.is_some()
            || self.ux.u3.confirm.is_some()
            || self.ux.u3.form.is_some()
            || self.ux.u3.kit_pick.is_some()
            || self.ux.u3.work_pick
            || self.ux.u4.import_open
            || self.ux.u4.confirm.is_some()
            || self.clash_modal
            || self.net_sheet_open()
            || self.ux.dir_sheet.is_some()
            || self.ux.snapshot_sheet
            || self.ux.out_sheet.is_some()
    }

    /// Close every sheet and the wizard.
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

    /// Open an identity sheet. Importing a key from the wizard uses the wizard's own sheet, not this.
    pub(super) fn id_layer_open(&mut self, m: IdModal) {
        self.sheets_clear();
        // The delete sheet's two disk readings are taken once, when it opens (the frame reads no disk).
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
        // Remember where the wizard was opened from: its way out returns to that seat and page.
        if self.ux.wizard.is_none() {
            self.ux.wiz_from = Some((self.shell.settings.role, self.ux.nav.stack, self.ux.nav.place, self.ux.nav.hist.get(&self.ux.nav.stack).cloned()));
        }
        self.ux.wizard = Some(step);
    }

    /// Whether the wizard offers a way out: once a passcode and an identity are both on this machine, judged now
    /// (on the first run as soon as both are set, and the same after a restart); before that there is nothing to
    /// go back to.
    pub(super) fn wizard_exit_shown(&self) -> bool {
        let identity = self.shell.identities.as_ref().map(|r| !r.rows.is_empty()).unwrap_or(false);
        !self.shell.vault.absent() && identity
    }

    /// Leave the wizard, back to the seat and page it was opened from. Finished steps stay finished, and the
    /// wizard opens again by its usual rules. New words shown but not yet checked are dropped (the user was
    /// warned first).
    pub(super) fn wizard_exit(&mut self, now: f64) {
        if self.shell.new_words.is_some() {
            self.act(Action::DropFresh, now);
        }
        self.ux.wiz_exit_ask = false;
        self.ux.wizard = None;
        self.ux.confirm_genesis = false;
        self.ux.confirm_adopt = false;
        if let Some((role, stack, place, hist)) = self.ux.wiz_from.take() {
            // Back to the seat it was opened on only when that seat has a key now (a first run records the default
            // seat before any identity exists): never onto an empty seat.
            let seated = self.shell.identities.as_ref().and_then(|r| r.now()).map(|(row, _)| row.address(role).is_some()).unwrap_or(false);
            if self.shell.settings.role != role && seated {
                self.act(Action::SwitchRole, now);
            }
            self.ux.nav.stack = stack;
            if let Some(p) = place {
                self.go(p, now);
            }
            // Back to the exact page it was opened on (a settings section, a detail page), with its history.
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
        self.ux.u3.form = Some(f);
    }

    /// Open a confirmation. The send sheet opens at once with the gas cap still loading; the node is asked
    /// once the sheet is up (`estimate_due`).
    pub(super) fn u3_open_confirm(&mut self, cf: U3Confirm) {
        self.sheets_clear();
        if let U3Confirm::Send { count } = cf {
            // An estimate and its fees describe this batch, chain and moment, so opening the card always asks again
            // (an estimate already out for this batch is awaited, not repeated: `estimate_due`).
            self.ux.u3.send_after_estimate = Some(count);
            self.ux.u3.estimate_from = self.ux.now;
        }
        self.ux.u3.confirm = Some(cf);
    }

    /// Clear the new-record form after a record is written, so old content is never written again as the next
    /// entry.
    pub(super) fn anchor_form_clear(&mut self) {
        self.typed.work_note.clear();
        self.typed.batch_files.clear();
        // Clear the four "recorded for" fields too, or they would be written into the next entry.
        self.typed.for_fields = Default::default();
        self.shell.content = None;
    }

    /// The grant form's fields as a draft (the new-grant page and the sublicense page pass the same one).
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
        }
    }

    /// Take what was dropped or chosen for a new record: one path is read as a file, folder or git repository
    /// (its name fills an empty record name); several paths make a batch, one entry per file.
    pub(super) fn take_for_record(&mut self, paths: Vec<String>, now: f64) {
        if paths.len() > 1 {
            self.typed.batch_files = paths;
            self.shell.content = None;
            return;
        }
        let Some(p) = paths.into_iter().next() else { return };
        self.typed.batch_files.clear();
        // The fingerprint is computed in the background (`Kind::Take`); the record name is filled when it lands
        // (`took_named`).
        self.act(Action::TakeDropped { path: p }, now);
    }

    /// After content is taken, an empty record name takes the content's whole name.
    pub(super) fn took_named(&mut self) {
        if self.typed.work_note.trim().is_empty() {
            if let Some(c) = self.shell.content.as_ref() {
                // The whole name, extension and all, file or folder alike (`report.tar.gz` stays itself).
                self.typed.work_note = width::file_name(&c.subject);
            }
        }
    }

    // ─── Drawing ───

    /// The confirmation sheet shown when a fetch finds this home at odds with the fetched ledger: the entries
    /// that would remain only in the old data (a table), what that means, "cancel", and "fetch and replace" (this
    /// home is kept as old data and a fresh one receives the fetched ledger, using the password the fetch was
    /// given).
    fn conflict_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let Some(c) = self.shell.fetch_conflict.clone() else { return };
        let ready = !self.typed.fetch_from.trim().is_empty() && !self.ux.fetch_held.is_empty() && !self.shell.tasks.in_flight(crate::task::Kind::Fetch);
        // One row per entry: its number, what it is, when it was queued.
        let rows: Vec<table::Row> = c
            .rows
            .iter()
            .map(|(r, at)| table::Row { cells: vec![table::Cell::Seq(r.seq), table::Cell::Text(human_summary(r)), table::Cell::Mono(at.map(crate::when::short).unwrap_or_default())], ..Default::default() })
            .collect();
        let (mut sure, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("fetch-conflict", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, &fill1(Key::ConflictTitle, &c.offline.to_string()), "");
                let cols = [table::SEQ, table::col(t(Key::U3Entry), table::Col::Fr(1.0)), table::col_r(t(Key::U3QueuedAt), table::Col::Px(120.0))];
                let _ = table::table(ui, "fetch-conflict", &cols, true, &rows, "");
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

    /// Draw every open sheet, above the page, the wizard and the gate.
    pub(super) fn sheets(&mut self, ctx: &egui::Context, now: f64) {
        // Restoring from a backup on the locked card lies above the gate; the other backup sheets lie below it.
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
        self.dir_sheet(ctx, now);
        self.estimate_due(ctx);
        self.new_anchor_sheet(ctx, now);
        self.conflict_sheet(ctx, now);
        match self.ux.u3.confirm.clone() {
            Some(U3Confirm::Send { count }) => self.send_sheet(ctx, count, None, now),
            Some(U3Confirm::Bump { tx }) => self.bump_sheet(ctx, &tx, now),
            Some(U3Confirm::NoEstimate { count, why, next, raw }) => self.send_sheet(ctx, count, Some((why, next, raw)), now),
            Some(U3Confirm::Retract { subject }) => self.delete_record_sheet(ctx, &subject, now),
            Some(U3Confirm::Grant) => self.grant_confirm_sheet(ctx, now),
            Some(U3Confirm::Revoke { grant }) => self.revoke_sheet(ctx, &grant, now),
            None => {}
        }
        match self.ux.u3.form {
            Some(U3Form::Adopt) => self.adopt_sheet(ctx, now),
            Some(U3Form::Succeed) => self.succeed_sheet(ctx, now),
            Some(U3Form::Annotate) => self.annotate_sheet(ctx, now),
            Some(U3Form::Attest) => self.attest_sheet(ctx, now),
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
    }

    /// Start the send sheet's gas estimate once the sheet is up. A failed estimate is reported on the sheet
    /// only (it turns to "cannot estimate"), so the troubles this call records are marked as reported.
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
        self.ux.u3.send_after_estimate = None;
        // The estimate runs as a task; the frame does no I/O. Until it lands, the cap cell shows loading and the
        // send key stays disabled (`shell.gas` is cleared when the task starts). An estimate already out for this
        // batch is awaited, not repeated.
        let before = self.shell.faults.len();
        match apply(&mut self.shell, Action::EstimateGas { count }) {
            Applied::Started(_) | Applied::Refused(_) => self.ux.u3.estimating = Some(count),
            Applied::Trouble(f) => {
                if self.faults_told >= before {
                    self.faults_told = self.shell.faults.len();
                }
                self.ux.u3.confirm = Some(U3Confirm::NoEstimate { count, why: f.human().to_string(), next: f.next().to_string(), raw: f.raw() });
            }
            _ => {}
        }
    }

    /// A gas estimate landed. On the send sheet of the batch it was asked for, a refusal turns the sheet to
    /// "cannot estimate gas" and is reported there only; a success needs nothing more (the sheet reads
    /// `shell.gas`). If the sheet closed or moved to another batch meanwhile, a refusal is reported normally.
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
        self.ux.u3.confirm = Some(U3Confirm::NoEstimate { count, why: f.human().to_string(), next: f.next().to_string(), raw: f.raw() });
    }

    /// New record, one step: the content in a group (its file and size, or "not chosen", or a batch of files
    /// with "clear"; its fingerprint; what it costs; the ledger's place under details), its name, optionally
    /// for whom and a linked repository (two folds with their summaries), and the key that writes it at the foot
    /// ("choose file" at the bottom left, or a file dropped on the sheet).
    fn new_anchor_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if self.ux.u3.new_anchor.is_none() {
            return;
        }
        // A batch down to one file (the others signed before a stop) is that file chosen alone: it is taken as one,
        // so the sheet shows its size and fingerprint; a batch is two files or more.
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
        // The key says what the write does (into the ledger, or on chain at once), as the grant's confirmation does.
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
                // A file dropped while the sheet is up is taken as if chosen (the sheet draws no drop zone).
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
                    // The content: one file, folder or repository (its name whole, elided in the middle, and its
                    // size), or a batch of files (one row each, "clear" with their count).
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
                    // For whom: optional as a whole (said once, in its summary); within it the role may stay empty.
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
                    // A linked Git repository: "add" chooses its folder and links it; "check" reads its commits.
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
                // While the fingerprint is computed (`Kind::Take`) or the entries written (`Kind::Record`) the key
                // shows a spinner, says which, and ignores presses; "cancel" stays.
                let busy = hashing_now || recording_now;
                let phase = if busy { Phase::Busy { frac: None } } else { Phase::Idle };
                let busy_say = t(if hashing_now { Key::U3Hashing } else { Key::U3Recording });
                commit = page::Pen::new().press_saying(ui, go, busy_say, ready, phase).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                // "Choose file" at the bottom left: a file, a folder or a repository, as a drop takes.
                let asked = ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| key::key(ui, t(Key::PickFile), Role::Secondary, true).clicked()).inner;
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

    /// Write the record (one entry, or one per file of a batch). The four "recorded for" fields go through
    /// `anchorx::For` first: a missing field or malformed identity is refused by name and nothing is written.
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
        // Files are recorded in the background (`Kind::Record`): the sheet stays with its key busy and closes when
        // that lands (`record_back`). A single entry from content already taken is written at once.
        if let Applied::Started(_) = r {
            self.ux.u3.recording = Some(files);
            return;
        }
        self.ux.u3.new_anchor = None;
        self.recorded(r, &files);
    }

    /// The form after recording: a complete write clears it; a batch stopped at item i keeps that item and the
    /// rest (the signed ones leave the form).
    fn recorded(&mut self, r: Applied, files: &[String]) {
        match r {
            Applied::Recorded { .. } | Applied::RecordedBatch { stopped: None, .. } => self.anchor_form_clear(),
            Applied::RecordedBatch { stopped: Some((i, _, _)), .. } => self.typed.batch_files = files[i..].to_vec(),
            _ => {}
        }
    }

    /// Answers of actions whose slow half ran in the background (`Shell::said`), taken where the window reads
    /// its landings: each goes back to the place that started it and is reported as if it had run in the frame.
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

    /// Put on chain: the first n entries, the gas estimate and the gas cap (estimating until they land). Pressing
    /// runs "sign, broadcast" in the key; the sheet closes once the broadcast lands. A failed estimate turns the
    /// sheet to "cannot estimate gas" with a retry.
    fn send_sheet(&mut self, ctx: &egui::Context, count: usize, failed: Option<(String, String, String)>, now: f64) {
        // Pressed and landed: close (the broadcast clears the estimate; a failure is shown on the key).
        if let Some(stamp) = self.ux.u3.sending {
            let gone = self.shell.gas.map(|(n, _)| n != count).unwrap_or(true);
            // Landed, successfully or not: the anchoring task has landed since the press (judged by the landing
            // itself, not by the clock).
            let landed = self.shell.tasks.answered_since(crate::task::Kind::Anchor, stamp);
            if gone || landed {
                self.ux.u3.sending = None;
                self.ux.u3.confirm = None;
                return;
            }
        }
        let sending = self.ux.u3.sending.is_some();
        let out = self.shell.tasks.in_flight(crate::task::Kind::Gas);
        // No reading for this batch, none out, none about to be asked and no refusal: whatever was out landed
        // nothing for this card (for other nodes or chain, or another batch), so ask again; the card never waits
        // on a reading that will not come.
        let held = self.shell.gas.map(|(n, _)| n == count).unwrap_or(false);
        if !sending && !out && !held && failed.is_none() && self.ux.u3.send_after_estimate.is_none() {
            self.ux.u3.estimating = None;
            self.ux.u3.send_after_estimate = Some(count);
            self.ux.u3.estimate_from = self.ux.now;
        }
        // Until this opening's estimate is asked and lands, an older reading is neither shown nor sent.
        let asking = self.ux.u3.send_after_estimate == Some(count);
        let gas = self.shell.gas.filter(|(n, _)| *n == count && !asking).map(|(_, g)| g);
        let cap = eth_cap(self.shell.fees.unwrap_or_else(zikaron_anchor::send::Fees::fallback).cap_wei());
        let fees_now = self.shell.fees;
        let fees_left = self.shell.fees_left.clone();
        // While sending, show the anchoring task's phase; until the estimate lands, the key says it is estimating,
        // shows a spinner and ignores presses.
        let phase = if sending {
            self.phase_of(crate::task::Kind::Anchor)
        } else if out || asking {
            Phase::Busy { frac: None }
        } else {
            Phase::Idle
        };
        let first = fill1(Key::U3FirstN, &count.to_string());
        let (mut go, mut close, mut retry) = (false, false, false);
        let step = u64::from(failed.is_some());
        let slide = if failed.is_some() { sheet::Slide::Forward } else { sheet::Slide::Back };
        let out = sheet::show(
            ctx,
            sheet::Spec::new("send", tk::SHEET_W).step(step, slide),
            self,
            |ui, _me| match &failed {
                None => {
                    // The batch, its estimate and its cap.
                    sheet::title(ui, t(Key::U3SendTitle), "");
                    let cap_say = fill1(Key::SetGasSay, &cap);
                    let gas_say = gas.map(|g| g.to_string()).unwrap_or_default();
                    card::form(ui, |ui, f| {
                        let _ = f.info(ui, t(Key::LedgerEntries), card::Show::Text(&first));
                        // Until the estimate lands both figures say it is estimating.
                        let _ = f.info(ui, t(Key::U3GasEstimate), if gas.is_some() { card::Show::Mono(&gas_say) } else { card::Show::Empty(t(Key::U3Estimating)) });
                        let _ = f.info(ui, t(Key::U3FeeCap), if gas.is_some() { card::Show::Mono(&cap_say) } else { card::Show::Empty(t(Key::U3Estimating)) });
                    });
                    // Where the cap came from: one line when it is the fallback pair, once the figures are in.
                    if let (Some(_), Some(k)) = (gas, crate::chainx::fee_source_line(fees_now.as_ref())) {
                        zikaron_ui::callout::callout(ui, t(k));
                    }
                    // Name each node the fees were not read from (it serves another chain).
                    if gas.is_some() && !fees_left.is_empty() {
                        zikaron_ui::callout::callout(ui, &fill1(Key::U3FeeLeftNodes, &fees_left.join(" · ")));
                    }
                }
                Some((why, next, raw)) => {
                    // Why it could not be estimated and what to do first, then the batch, the raw error last.
                    sheet::title(ui, t(Key::U3NoEstimateTitle), "");
                    err_call(ui, "no-estimate", why, next, "", "");
                    card::form(ui, |ui, f| {
                        let _ = f.info(ui, t(Key::LedgerEntries), card::Show::Text(&first));
                        details_row(ui, f, "no-estimate-details", &[(t(Key::U3NoEstimateWhy), Val::mono(raw.clone()))]);
                    });
                }
            },
            |ui, me| {
                if failed.is_none() {
                    // While busy the key says what is happening: estimating before the figure lands, sending after
                    // the press.
                    let busy = t(if sending { Key::U3Sending } else { Key::U3Estimating });
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
            return;
        }
        if retry {
            self.ux.u3.confirm = Some(U3Confirm::Send { count });
            self.ux.u3.send_after_estimate = Some(count);
            self.ux.u3.estimate_from = self.ux.now;
        }
        if go {
            match self.act(Action::SendBatch { count }, now) {
                Applied::Started(_) => self.ux.u3.sending = Some(self.shell.tasks.landings(crate::task::Kind::Anchor)),
                // Refused at once (reported as a toast): close the sheet.
                _ => self.ux.u3.confirm = None,
            }
        }
    }

    /// Resend a stuck batch with higher fees: entry count, the cap it went out with, the current price at the
    /// same gas, and the resend's cap (from the last receipt wait, `Shell::stuck`). A press while a receipt
    /// check is out is held until that check ends (the key spins and says so; closing the card drops it). The
    /// sheet closes when the resend is taken or refused (refusals reported as usual), or when the offer is
    /// gone (included, or no longer a resend).
    fn bump_sheet(&mut self, ctx: &egui::Context, tx: &str, now: f64) {
        let offer = self.shell.stuck.clone().filter(|s| s.txs.last().map(String::as_str) == Some(tx));
        let Some(stuck) = offer else {
            self.ux.u3.confirm = None;
            self.ux.u3.bump_press = None;
            return;
        };
        // A resend replacing a transaction a node holds (higher fees), or the batch sent again when no node holds
        // it (at the current price; there is no original cap to show).
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
        // While this card is open, any anchoring task out is a receipt check (a taken resend closes the card), so
        // a press is held and sent when that check ends.
        let waiting = self.ux.u3.bump_press.is_some();
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("bump", tk::SHEET_W),
            self,
            |ui, _me| {
                // The figures (the details last: the gas and the batch's transactions, the first sent and each
                // resend, one a line).
                sheet::title(ui, t(if unheld { Key::U3ResendKey } else { Key::U3BumpTitle }), "");
                let at_gas = |per_gas: u64| fill1(Key::SetGasSay, &eth_cap(u128::from(per_gas) * u128::from(stuck.sent.gas_limit)));
                card::form(ui, |ui, f| {
                    let _ = f.info(ui, t(Key::LedgerEntries), card::Show::Text(&entries.to_string()));
                    if !unheld {
                        let _ = f.info(ui, t(Key::U3BumpOldCap), card::Show::Mono(&at_gas(stuck.sent.max_fee)));
                    }
                    let _ = f.info(ui, t(Key::U3BumpPriceNow), card::Show::Mono(&at_gas(base_now)));
                    let _ = f.info(ui, t(Key::U3BumpNewCap), card::Show::Mono(&fill1(Key::SetGasSay, &eth_cap(fees.cap_wei()))));
                    details_row(ui, f, "bump-details", &[(t(Key::U3GasEstimate), Val::mono(stuck.sent.gas_limit.to_string())), (t(Key::SendTx), Val::mono(stuck.txs.join("\n")))]);
                });
                // Name each node the current price was not read from (it serves another chain).
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
            // A receipt check is out: send once it ends (the key spins while it waits).
            if self.shell.tasks.in_flight(crate::task::Kind::Anchor) {
                self.ux.u3.bump_press = Some((tx.to_string(), fees.max_fee));
            } else {
                self.bump_go(tx.to_string(), fees.max_fee, now);
            }
        }
    }

    /// Send the resend the user pressed, once. Taken: the sheet closes and the new wait starts. Refused: the
    /// refusal is reported and the sheet closes (a changed offer shows its new figures on the next press).
    pub(super) fn bump_go(&mut self, tx: String, cap: u64, now: f64) {
        let _ = self.act(Action::BumpFee { tx, cap }, now);
        self.ux.u3.confirm = None;
    }

    /// Delete a record (retraction convention): which record, with the fingerprint and the ledger's place under
    /// details in the same group, then what deleting does.
    fn delete_record_sheet(&mut self, ctx: &egui::Context, subject: &str, now: f64) {
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
                    let _ = f.info(ui, t(Key::U3Work), card::Show::Text(&row.as_ref().map(human_summary).unwrap_or_default()));
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

    /// Sign a grant (a sublicense when it has an upstream), all of it at a glance: grantee, record, validity
    /// (its days and length), terms file, exclusivity with upstream and scope, and cost; that a grant once
    /// signed can only be revoked; the grantee's whole address, the upstream and the fingerprints under details.
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
                    let _ = f.info(ui, t(Key::U3GrantMore), card::Show::Text(&more));
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
            // The terms document is included only when the fingerprint matches the chosen file exactly.
            let terms_file = self.ux.u3.terms.as_ref().filter(|x| x.hex == self.typed.g_terms.trim()).map(|x| x.path.clone());
            let a = Action::DraftGrant { draft: Box::new(self.draft()), exclusive: self.typed.g_exclusive, terms_file };
            self.act(a, now);
        }
    }

    /// Revoke a grant: the record, its original validity and the grantee, the optional ruling file fingerprint
    /// as a field of its own.
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

    /// The exclusive-grant clash (read only, so its one key is "close"): what happened, what to do, the ids
    /// under error details, and one row for each grant it clashes with.
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

    /// Import existing records: anchors this key sent earlier, or another key's anchors with that key holder's
    /// signature; one switch per anchor, and "import n" writes one adoption entry.
    fn adopt_sheet(&mut self, ctx: &egui::Context, now: f64) {
        use crate::adoptx::RowState;
        let want = self.typed.ad_key.trim().to_ascii_lowercase();
        let other = !want.is_empty();
        let formed = !other || crate::key::Address::parse(&want).is_some();
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Adopt);
        let can_list = !self.shell.endpoints.is_empty() && (other || self.shell.audit.is_some());
        if formed && can_list && !busy && self.ux.u3.adopt_asked.as_deref() != Some(want.as_str()) {
            self.ux.u3.adopt_asked = Some(want.clone());
            self.auto(Action::ListKeyAnchors { address: want.clone() }, now);
        }
        if self.ux.u3.sig_paste {
            let pasted = ctx.input(|i| i.events.iter().find_map(|e| if let egui::Event::Paste(s) = e { Some(s.clone()) } else { None }));
            if let Some(p) = pasted {
                self.typed.ad_sig = p.trim().to_string();
                self.ux.u3.sig_paste = false;
            }
        }
        let listed: Option<Vec<crate::adoptx::KeyAnchor>> = self.shell.key_anchors.as_ref().filter(|(a, _)| *a == want).map(|(_, r)| r.clone());
        let failed = if listed.is_none() && !busy && self.ux.u3.adopt_asked.as_deref() == Some(want.as_str()) { self.shell.failed.get(&crate::task::Kind::Adopt).cloned() } else { None };
        let key_of = |k: &crate::adoptx::KeyAnchor| format!("{}|{}", k.row.tx.to_ascii_lowercase(), k.row.content.to_ascii_lowercase());
        let chosen: Vec<crate::adoptx::AnchorRow> = listed
            .iter()
            .flatten()
            .filter(|k| k.state() == Some(RowState::Passed) && !self.ux.u3.adopt_off.contains(&key_of(k)))
            .map(|k| k.row.clone())
            .collect();
        // Rows typed by hand join only once all of them pass the check against the current text.
        let hand = self.typed.ad_rows.trim().to_string();
        let hand_ok = !hand.is_empty()
            && self.ux.u3.adopt_checked.as_deref() == Some(hand.as_str())
            && self.shell.proofs_rows.as_deref() == Some(hand.as_str())
            && self.shell.proofs.as_ref().map(|p| !p.is_empty() && p.iter().all(|x| x.ok())).unwrap_or(false);
        let hand_rows: Vec<crate::adoptx::AnchorRow> = if hand_ok { crate::adoptx::rows_of(&hand).unwrap_or_default() } else { Vec::new() };
        let mut all_rows = chosen.clone();
        all_rows.extend(hand_rows);
        // The other key holder's signature: the text follows the selection and the ledger head; a pasted signature
        // is checked at once (`adoptx::cosigned`).
        let head = self.shell.rows.as_ref().and_then(|(r, _)| r.iter().max_by_key(|x| x.seq).map(|x| x.id.clone()));
        let me_key = self.shell.anchor;
        let text = match (other, me_key, head.as_ref()) {
            (true, Some(me), Some(h)) if !all_rows.is_empty() => Some(crate::adoptx::claim_text(&me, &all_rows, h)),
            _ => None,
        };
        let sig = self.typed.ad_sig.trim().to_string();
        let sig_ok = match (me_key, head.as_ref()) {
            (Some(me), Some(h)) if other && !sig.is_empty() && !all_rows.is_empty() => Some(crate::adoptx::cosigned(&me, &all_rows, h, &want, &sig).is_ok()),
            _ => None,
        };
        let chain = self.shell.settings.chain_id;
        let chain_name = chain
            .and_then(|c| crate::deploy::KNOWN.iter().find(|d| d.chain_id == c).map(|d| t(d.label).to_string()))
            .or_else(|| chain.map(|c| c.to_string()))
            .unwrap_or_default();
        let book: Vec<String> = self.recent_addresses();
        let n = all_rows.len();
        let (mut go, mut close, mut select_all, mut verify_hand, mut paste) = (false, false, false, false, false);
        let mut flip: Option<String> = None;
        let mut copy: Option<String> = None;
        let mut booked: Option<String> = None;
        // Whose anchors: this machine's key, or another key (its address typed, the field kept while empty).
        let mode_id = egui::Id::new("zikaron-adopt-other");
        let other_mode = other || ctx.data(|d| d.get_temp::<bool>(mode_id)).unwrap_or(false);
        let mut mode_to: Option<bool> = None;
        // The anchors found: a head row (what they are, how many, "select all"), one row per anchor with its state
        // and its switch, the raw values under details.
        let list = |ui: &mut egui::Ui, off: &std::collections::BTreeSet<String>, flip: &mut Option<String>, select_all: &mut bool| {
            let title = t(if other { Key::U3AdOnChain } else { Key::U3AdEarlier });
            zikaron_ui::fields::rows(ui, |ui, f| match (&listed, &failed) {
                (Some(rows), _) => {
                    let count = fill2(Key::U3AdCount, &chain_name, &rows.len().to_string());
                    let _ = f.line(ui, None, title, count.trim_start_matches(|ch: char| ch == '\u{b7}' || ch.is_whitespace()), false, false, c(C::Ink), |ui| {
                        if !rows.is_empty() && key::key(ui, t(Key::U3AdSelectAll), Role::Secondary, true).clicked() {
                            *select_all = true;
                        }
                    });
                    if rows.is_empty() {
                        let _ = f.info(ui, t(Key::Nothing), card::Show::Empty(""));
                        return;
                    }
                    for k in rows {
                        let state = k.state();
                        let passed = state == Some(RowState::Passed);
                        let on = passed && !off.contains(&key_of(k));
                        let (say, tone) = match state {
                            Some(RowState::Passed) => (t(Key::AuditPass), PillTone::Ok),
                            Some(RowState::NoSuchTx) => (t(Key::U3AdNoTx), PillTone::Bad),
                            Some(RowState::WrongSender) => (t(Key::U3AdWrongSender), PillTone::Bad),
                            Some(RowState::WrongDigest) => (t(Key::U3AdWrongDigest), PillTone::Bad),
                            Some(RowState::InLedger) => (t(Key::U3AdInLedger), PillTone::Grey),
                            None => (t(Key::None_), PillTone::Grey),
                        };
                        // The anchor's time leads its row in the monospace face.
                        let (row, flipped) = f.line(ui, None, "", "", false, false, c(C::Ink), |ui| {
                            let s = toggle::switch(ui, on, passed).clicked();
                            mark::pill(ui, say, tone);
                            s
                        });
                        ui.painter().text(egui::pos2(row.rect.left(), row.rect.center().y), egui::Align2::LEFT_CENTER, crate::when::when(k.time), Type::MonoSmall.font(), c(C::Ink));
                        if flipped {
                            *flip = Some(key_of(k));
                        }
                    }
                    let raw: Vec<(String, String)> = rows.iter().map(|k| (crate::when::when(k.time), fill2(Key::AdBlockAt, &k.block.to_string(), &crate::ledgerx::short(&k.row.content)))).collect();
                    let raw_rows: Vec<(&str, Val)> = raw.iter().map(|(a, b)| (a.as_str(), Val::figure(b.clone()))).collect();
                    details_row(ui, f, "adopt-rows-details", &raw_rows);
                }
                (None, Some(fl)) => {
                    let _ = f.line(ui, Some(card::Lead::Mark(Mark::Bad)), title, fl.human(), false, false, c(C::Ink), |_| ());
                }
                (None, None) => {
                    // Do not query an address still being typed: say what it should look like instead of "verifying".
                    let say = if !formed {
                        Key::FaultNextAddressShape
                    } else if can_list {
                        Key::VaultBusy
                    } else {
                        Key::U4ChainUnread
                    };
                    let _ = f.line(ui, None, title, t(say), false, false, c(C::Ink), |_| ());
                }
            });
        };
        let out = sheet::show(
            ctx,
            sheet::Spec::new("adopt", tk::SHEET_XWIDE),
            self,
            |ui, me| {
                if let Some(i) = parts::title_seg(ui, t(Key::U3AdoptTitle), "adopt-mode", &[t(Key::U3AdOwnKey), t(Key::U3AdOtherKey)], usize::from(other_mode)) {
                    mode_to = Some(i == 1);
                }
                let off = me.ux.u3.adopt_off.clone();
                fields::stack(ui, |ui, st| {
                    if other_mode {
                        // Another key's anchors: its address (the address book beside it), the holder's signature (its
                        // state, the text to send and the signature pasted back, both under details), then the anchors.
                        st.field(ui, t(Key::IdAddress), None, None, |ui, w| {
                            let heads: Vec<String> = book.iter().map(|a| head_tail(a)).collect();
                            let items: Vec<menu::Item> = heads.iter().map(|h| menu::Item::Row(menu::Row { lead: t(Key::Address), label: h, mono: true, ..Default::default() })).collect();
                            let spec = pick::Spec { hint: t(Key::SearchAddress), empty: t(Key::SearchNone), w: PICK_W, dates: None };
                            let keep = |i: usize, q: &str, _: &str, _: &str| matches(q, &[&book[i]]);
                            let mut picked = None;
                            ui.allocate_ui_with_layout(egui::vec2(w, tk::INPUT_H), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                width::then(
                                    ui,
                                    |ui| picked = pick::key(ui, "adopt-book", t(Key::U3SeenBeforeDots), !book.is_empty(), true, &spec, &items, &keep),
                                    |ui, room| fields::line(ui, &mut me.typed.ad_key, "0x\u{2026}", room, fields::Look { mono: true, ..Default::default() }),
                                );
                            });
                            if let Some(i) = picked {
                                booked = book.get(i).cloned();
                            }
                        });
                        zikaron_ui::fields::rows(ui, |ui, f| {
                            let (word, tone) = match sig_ok {
                                None => (t(Key::U3AdWait), PillTone::Warn),
                                Some(true) => (t(Key::AuditPass), PillTone::Ok),
                                Some(false) => (t(Key::DeliveryMismatch), PillTone::Bad),
                            };
                            let _ = f.info(ui, t(Key::U3AdCosign), card::Show::Pill(word, tone));
                            if f.act(ui, t(Key::ChannelText), "", key::Key::new(t(Key::U3AdCopy), Role::Secondary).enabled(text.is_some())).clicked() {
                                copy = text.clone();
                            }
                            paste = f.act(ui, t(Key::U3AdSig), "", key::Key::new(t(Key::U3AdPaste), Role::Secondary)).clicked();
                            details_row(
                                ui,
                                f,
                                "adopt-sig-details",
                                &[
                                    (t(Key::ChannelText), text.clone().map(Val::mono).unwrap_or_else(|| Val::text(t(Key::None_)))),
                                    (t(Key::U3AdSig), if sig.is_empty() { Val::text(t(Key::None_)) } else { Val::mono(sig.clone()) }),
                                ],
                            );
                        });
                    }
                    list(ui, &off, &mut flip, &mut select_all);
                    // Rows typed by hand: opened in place in its own group, what the check found on the fold's row.
                    let proved = match (me.shell.proofs.as_ref(), me.shell.proofs_rows.as_deref()) {
                        (Some(p), Some(rows)) if rows == hand => fill2(Key::SaidProofs, &p.len().to_string(), &p.iter().filter(|x| x.ok()).count().to_string()),
                        _ => String::new(),
                    };
                    zikaron_ui::fields::rows(ui, |ui, f| {
                        f.fold(ui, "adopt-hand", t(Key::U3AdByHand), &proved, |ui, f| {
                            f.free(ui, |ui| input::area_hint(ui, &mut me.typed.ad_rows, 2, t(Key::AdHandHint)));
                            f.keys(ui, |ui| verify_hand = key::key(ui, t(Key::U3AdCheckHand), Role::Secondary, !hand.is_empty() && !busy).clicked());
                        });
                    });
                });
            },
            |ui, _me| {
                go = page::Pen::new().press(ui, &fill1(Key::U3AdGo, &n.to_string()), n > 0).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        match mode_to {
            Some(true) => {
                ctx.data_mut(|d| d.insert_temp(mode_id, true));
            }
            // This machine's key: the address typed for another key is let go.
            Some(false) => {
                ctx.data_mut(|d| d.insert_temp(mode_id, false));
                self.typed.ad_key.clear();
                self.typed.ad_sig.clear();
            }
            None => {}
        }
        if close || out.esc || go {
            ctx.data_mut(|d| d.insert_temp(mode_id, false));
        }
        if let Some(k) = flip {
            if !self.ux.u3.adopt_off.remove(&k) {
                self.ux.u3.adopt_off.insert(k);
            }
        }
        if select_all {
            self.ux.u3.adopt_off.clear();
        }
        if let Some(a) = booked {
            self.typed.ad_key = a;
        }
        if let Some(x) = copy {
            ctx.copy_text(x);
            self.toasts.say(t(Key::U3CopiedText), Tone::Note, now);
        }
        if paste {
            self.ux.u3.sig_paste = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
        }
        if verify_hand {
            self.ux.u3.adopt_checked = Some(hand.clone());
            self.act(Action::VerifyAnchors { rows: hand }, now);
        }
        if close || out.esc {
            self.ux.u3.form = None;
        } else if go {
            let rows: Vec<String> = all_rows.iter().map(crate::adoptx::line_of).collect();
            // The co-signature is included only when it checked; without it the entries are still valid, just
            // unproven.
            let (attestor, attestation) = if sig_ok == Some(true) { (want.clone(), sig.clone()) } else { (String::new(), String::new()) };
            self.ux.u3.form = None;
            self.act(Action::AdoptAnchors { rows: rows.join("\n"), attestor, attestation }, now);
        }
    }

    /// Change key or hand over. Step one: the new key's address (scanned once well formed; a key that has sent
    /// anchors cannot be chosen), the kind and a statement. Step two: what will be written, the address under
    /// details, "back", and the commit.
    fn succeed_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let to = self.typed.sc_to.trim().to_string();
        let formed = crate::key::Address::parse(&to).is_some();
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Sighting);
        if formed && self.sighting_now().is_none() && !busy && self.ux.u3.sighting_asked.as_deref() != Some(to.as_str()) {
            self.ux.u3.sighting_asked = Some(to.clone());
            self.auto(Action::LookAtKey { to: to.clone() }, now);
        }
        let sighting = self.sighting_now();
        let failed = self.shell.failed.get(&crate::task::Kind::Sighting).filter(|_| self.ux.u3.sighting_asked.as_deref() == Some(to.as_str()) && sighting.is_none() && !busy).cloned();
        let clean = matches!(sighting, Some((_, 0, _)));
        let words: Vec<&str> = crate::succeedx::KINDS.iter().map(|k| crate::succeedx::kind_words(k).unwrap_or(k)).collect();
        let kind_now = crate::succeedx::KINDS.iter().position(|k| *k == self.typed.sc_kind.trim());
        let step = self.ux.u3.succeed_confirm;
        let kind_words = kind_now.map(|i| words[i].to_string()).unwrap_or_default();
        let say = if self.typed.sc_statement.trim().is_empty() { kind_words.clone() } else { self.typed.sc_statement.trim().to_string() };
        let (mut next, mut back, mut close, mut commit) = (false, false, false, false);
        let mut picked: Option<usize> = None;
        let slide = if step { sheet::Slide::Forward } else { sheet::Slide::Back };
        let out = sheet::show(
            ctx,
            sheet::Spec::new("succeed", tk::SHEET_W).step(u64::from(step), slide),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::TagHandover), "");
                fields::stack(ui, |ui, st| {
                    if !step {
                        // What the scan of the new key found stands under its field.
                        let found = if busy && formed {
                            Some((t(Key::VaultBusy).to_string(), false))
                        } else if let Some((_, anchors, _)) = &sighting {
                            Some((fill1(Key::SaidSighting, &anchors.to_string()), *anchors != 0))
                        } else {
                            failed.as_ref().map(|f| (f.human().to_string(), true))
                        };
                        let bad = found.as_ref().is_some_and(|(_, b)| *b);
                        st.field(ui, t(Key::U3HandTo), None, found.as_ref().map(|(s, bad)| (s.as_str(), *bad)), |ui, w| fields::line(ui, &mut me.typed.sc_to, "0x\u{2026}", w, fields::Look { mono: true, bad, ..Default::default() }));
                        st.field(ui, t(Key::U3HandKind), None, None, |ui, _| {
                            let items: Vec<menu::Item> = words.iter().enumerate().map(|(i, w)| menu::Item::Row(menu::Row { label: w, check: Some(kind_now == Some(i)), ..Default::default() })).collect();
                            let shown = if kind_now.is_some() { kind_words.as_str() } else { t(Key::U3HandKindPick) };
                            picked = menu::menu_key(ui, "succeed-kind", shown, true, 160.0, &items);
                        });
                        st.field(ui, t(Key::U3HandSay), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut me.typed.sc_statement, "", w, fields::Look::default()));
                    } else {
                        // What will be written, the new key's address among it (copied from its row).
                        let mut copied = None;
                        zikaron_ui::fields::rows(ui, |ui, f| {
                            copied = f.info(ui, t(Key::U3HandTo), card::Show::Short(&to));
                            let _ = f.info(ui, t(Key::U3HandKind), if kind_words.is_empty() { card::Show::Empty(t(Key::Nothing)) } else { card::Show::Text(&kind_words) });
                            // An empty statement is written as the kind's plain words: shown so, in the quiet ink.
                            let _ = f.info(ui, t(Key::U3HandSay), match (say.is_empty(), me.typed.sc_statement.trim().is_empty()) {
                                (true, _) => card::Show::Empty(t(Key::Nothing)),
                                (false, true) => card::Show::Empty(&say),
                                (false, false) => card::Show::Text(&say),
                            });
                        });
                        tell_copied(ui.ctx(), copied);
                    }
                });
            },
            |ui, _me| {
                if !step {
                    next = page::Page::new().primary_with(ui, t(Key::WizardNext), clean && kind_now.is_some()).1.clicked();
                    close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                } else {
                    commit = page::Pen::new().press(ui, t(Key::U3SucceedGo), true).clicked();
                    close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                    back = foot_left(ui, t(Key::WizPrev));
                }
            },
        );
        if let Some(i) = picked {
            self.typed.sc_kind = crate::succeedx::KINDS[i].to_string();
        }
        if next {
            self.ux.u3.succeed_confirm = true;
        }
        if back {
            self.ux.u3.succeed_confirm = false;
        }
        if close || out.esc {
            self.ux.u3.form = None;
            self.ux.u3.succeed_confirm = false;
        } else if commit {
            self.ux.u3.form = None;
            self.ux.u3.succeed_confirm = false;
            // The effective time is left empty for the action layer to fill with now; an empty statement gets the
            // kind's plain words.
            let a = Action::Succeed { to, kind: self.typed.sc_kind.trim().to_string(), effective: String::new(), statement_md: self.typed.sc_statement.clone() };
            self.act(a, now);
        }
    }

    /// Annotate an entry: the note, then which entry (this ledger, newest first; an id typed by hand under
    /// advanced options). A refused note shakes the sheet.
    fn annotate_sheet(&mut self, ctx: &egui::Context, now: f64) {
        self.ensure_rows(now);
        let rows: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
        let read = self.shell.rows.is_some();
        let reading = crate::retractx::read(&rows);
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("annotate", tk::SHEET_W).shake(self.ux.sheet_shake),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::U3AnnotateTitle), "");
                fields::stack(ui, |ui, st| {
                    st.field(ui, t(Key::AnnotateText), None, None, |ui, _| input::area_words(ui, &mut me.typed.annotate_note, 3, ""));
                    st.field(ui, t(Key::U3AnnotateWhich), None, None, |ui, w| {
                        // A picker over this ledger, newest first: number, type, summary and first anchor time; the
                        // entry chosen stands in the field, "select" inside its right end.
                        let faces: Vec<(String, String, String)> = rows
                            .iter()
                            .map(|row| {
                                let (tag, summary, _) = row_face(&rows, &reading, row);
                                let at = row.anchored_at.map(crate::when::when).unwrap_or_else(|| t(Key::V2StateLanded).to_string());
                                (format!("#{}", row.seq), format!("{tag} \u{b7} {summary}"), at)
                            })
                            .collect();
                        let items: Vec<menu::Item> = faces.iter().map(|(seq, label, at)| menu::Item::Row(menu::Row { lead: seq, label, trail: at, ..Default::default() })).collect();
                        let words = date_words();
                        let spec = pick::Spec {
                            hint: t(Key::SearchLedger),
                            empty: t(Key::SearchNone),
                            w: PICK_WIDE_W,
                            dates: Some(pick::Dates { words: &words, today: today_of((me.shell.clock)()), from_hint: t(Key::DateFrom), to_hint: t(Key::DateTo) }),
                        };
                        let keep = |i: usize, q: &str, from: &str, to: &str| matches(q, &[&faces[i].0, &faces[i].1, &rows[i].id]) && crate::when::within(rows[i].anchored_at, from, to);
                        let current = rows.iter().position(|r| me.typed.annotate_subject.trim().eq_ignore_ascii_case(&r.id));
                        let mut shown = current.map(|i| format!("{} {}", faces[i].0, faces[i].1)).unwrap_or_default();
                        let hint = if read { t(Key::U3AnnotatePickHint) } else { t(Key::U3AnnotateNoRows) };
                        let look = fields::Look { icon: Some(Glyph::File), read_only: true, ..Default::default() };
                        let (_, picked) = fields::joined(ui, &mut shown, hint, w, look, Some(|ui: &mut egui::Ui| zikaron_ui::formparts::inner_pick(ui, "annotate-pick", t(Key::KitPickOpen), false, true, read, &spec, &items, &keep)));
                        if let Some(i) = picked.flatten() {
                            me.typed.annotate_subject = rows[i].id.clone();
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
                // Always pressable: an empty note is refused by the action, which says why (one judgement).
                go = page::Pen::new().press(ui, t(Key::U3AnnotateGo), true).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if close || out.esc {
            self.ux.u3.form = None;
            self.ux.sheet_shake = None;
        } else if go {
            let a = Action::Annotate { subject: self.typed.annotate_subject.clone(), note_md: self.typed.annotate_note.clone() };
            match self.act(a, now) {
                Applied::Annotated(_) => {
                    self.ux.u3.form = None;
                    self.ux.sheet_shake = None;
                    self.typed.annotate_note.clear();
                }
                // Refused: the sheet stays, shakes, and the refusal is reported.
                _ => self.ux.sheet_shake = Some(now),
            }
        }
    }

    /// Sign a claim for someone (the other key holder's side): paste the text they sent; the sheet shows how
    /// many anchors (claimant, anchors and ledger head under details); sign with this machine's passcode; copy
    /// the signature to send back.
    fn attest_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let text = self.typed.at_text.trim().to_string();
        if !text.is_empty() && self.ux.u3.at_read.as_deref() != Some(text.as_str()) && !self.shell.tasks.in_flight(crate::task::Kind::Adopt) {
            self.ux.u3.at_read = Some(text.clone());
            self.ux.u3.attest_trouble = match self.act(Action::ReadClaim { text: text.clone() }, now) {
                Applied::Trouble(f) => Some(f),
                _ => None,
            };
        }
        let claim = self.shell.claim.clone().filter(|_| self.ux.u3.at_read.as_deref() == Some(text.as_str()) && !text.is_empty());
        let signed = self.shell.attested.clone();
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Vault);
        let trouble = self.ux.u3.attest_trouble.clone();
        let pin_full = self.ux.pin.chars() == crate::keybox::PIN_LEN;
        let (mut sign, mut close) = (false, false);
        // The passcode's last character signs, as the key does.
        let mut filled = false;
        let mut copy: Option<String> = None;
        let out = sheet::show(
            ctx,
            sheet::Spec::new("attest", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::U3AttestTitle), "");
                // The text pasted (why it does not read under it); who claims and how many anchors, their details
                // folded in the same group; then the passcode that signs, or the signature to copy back.
                fields::stack(ui, |ui, st| {
                    let line = trouble.as_ref().map(|f| (f.human(), true));
                    st.field(ui, t(Key::U3AttestPaste), None, line, |ui, _| input::area(ui, &mut me.typed.at_text, 3));
                    let Some((cl, blocks)) = &claim else { return };
                    let anchors: Vec<String> = cl
                        .rows
                        .iter()
                        .zip(blocks.iter())
                        .map(|(r, b)| format!("{} \u{b7} {}", b.map(|x| x.to_string()).unwrap_or_else(|| t(Key::None_).to_string()), crate::ledgerx::short(&r.content)))
                        .collect();
                    let mut copied = None;
                    zikaron_ui::fields::rows(ui, |ui, f| {
                        copied = f.info(ui, t(Key::U3AttestWho), card::Show::Short(&cl.adopter));
                        let _ = f.info(ui, t(Key::U3AttestAnchors), card::Show::Text(&fill1(Key::AnchorsN, &cl.rows.len().to_string())));
                        details_row(
                            ui,
                            f,
                            "attest-details",
                            &[
                                (t(Key::U3AttestWho), kv::Val::mono(cl.adopter.clone())),
                                (t(Key::U3AttestAnchors), kv::Val::mono(anchors.join("\n"))),
                                (t(Key::U3AttestHead), kv::Val::mono(cl.prev.clone())),
                            ],
                        );
                    });
                    tell_copied(ui.ctx(), copied);
                    match &signed {
                        Some((_, s)) => {
                            zikaron_ui::fields::rows(ui, |ui, f| {
                                if f.act(ui, t(Key::U3AttestDone), "", key::Key::new(t(Key::U3AdCopy), Role::Secondary)).clicked() {
                                    copy = Some(s.clone());
                                }
                            });
                        }
                        None => {
                            let row = pin_field(st, ui, t(Key::U3AttestPin), None, "attest-pin", &mut me.ux.pin, me.ux.pin_shake, !busy);
                            filled = row.full && row.changed;
                        }
                    }
                });
            },
            |ui, _me| {
                if signed.is_some() {
                    close = page::Page::new().primary(ui, t(Key::CfBack)).1.clicked();
                } else {
                    sign = page::Pen::new().press(ui, t(Key::U3AdSig), claim.is_some() && pin_full && !busy).clicked();
                    close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                }
            },
        );
        if let Some(x) = copy {
            ctx.copy_text(x);
            self.toasts.say(t(Key::U3CopiedSig), Tone::Note, now);
        }
        sign |= filled && claim.is_some() && !busy;
        if sign {
            let pin = std::mem::take(&mut self.ux.pin);
            let r = self.act(Action::AttestFor { text: text.clone(), pin }, now);
            self.vault_or(VaultSite::Attest, r, now);
        }
        if close || out.esc {
            self.ux.u3.form = None;
            self.typed.at_text.clear();
            self.ux.u3.at_read = None;
            self.ux.u3.attest_trouble = None;
            self.shell.attested = None;
            self.ux.pin.clear();
        }
    }

    /// Add a held grant: paste the grant code or drop a grant file (read and checked at once, said under the
    /// code), and this machine's own names for the grant and its issuer. It is stored only after verification;
    /// otherwise the two sentences are shown with the raw error folded.
    fn add_grant_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.u4.import_open {
            return;
        }
        let err = self.ux.u4.import_err.clone();
        let typed = self.typed.vt_typed.trim().to_string();
        if self.ux.u4.grant_file_seen.as_ref().map(|(k, _)| *k != typed).unwrap_or(true) {
            let p = std::path::Path::new(&typed);
            let seen = (!typed.is_empty() && crate::grantfilex::is_grant_file(p)).then(|| {
                crate::grantfilex::open(p).map(|o| fill3(Key::GrantFileVerified, &width::file_name(&typed), &o.hops.len().to_string(), &o.terms.len().to_string()))
            });
            self.ux.u4.grant_file_seen = Some((typed.clone(), seen));
        }
        let seen = self.ux.u4.grant_file_seen.as_ref().and_then(|(_, x)| x.clone());
        // What the code cell says under it: the grant file read, or why not; a pasted grant code (unlike a grant
        // file) carries no issuer ledger, said before adding.
        let code = typed.to_ascii_lowercase().starts_with(zikaron_kit::tokens::BADGE_PREFIX);
        let check: Option<(String, bool)> = match &seen {
            Some(Ok(said)) => Some((said.clone(), false)),
            Some(Err(f)) => Some((f.human().to_string(), true)),
            None if code => Some((t(Key::V2CodeCarriesNoLedger).to_string(), false)),
            None => None,
        };
        // The typed text names a grant file: the file shows in its row, and the code area starts empty.
        let file = seen.is_some().then(|| width::file_name(&typed));
        let (code_line, file_line) = if file.is_some() { (None, check.clone()) } else { (check.clone(), None) };
        let (mut go, mut close) = (false, false);
        let (mut dropped, mut picked): (Option<String>, Option<String>) = (None, None);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("add-grant", tk::SHEET_W).shake(self.ux.sheet_shake),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::V2AddGrant), "");
                // A grant file dropped while the sheet is up is taken as if chosen (the sheet draws no drop zone).
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
                    // The grant file: chosen at the bottom left, or dropped on the sheet.
                    card::form(ui, |ui, f| {
                        let _ = f.line(ui, Some(card::Lead::Glyph(Glyph::File)), t(Key::SourceGrantFile), "", false, false, c(C::Ink), |ui| match file.as_deref() {
                            Some(n) => ui.label(egui::RichText::new(n).font(Type::Note.font()).color(c(C::Ink))),
                            None => ui.label(egui::RichText::new(t(Key::PickNone)).font(Type::Note.font()).color(c(C::Ink3))),
                        });
                    });
                    if let Some((said, bad)) = file_line.as_ref() {
                        st.says(ui, &[(if *bad { Mark::Bad } else { Mark::Ok }, said.as_str())]);
                    }
                    // The user's own names for this grant and its issuer, kept on this machine only.
                    st.field(ui, t(Key::V2GrantNote), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut me.typed.vt_note, t(Key::AddGrantNoteHint), w, fields::Look::default()));
                    st.field(ui, t(Key::V2IssuerNote), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut me.typed.vt_issuer_note, t(Key::AddIssuerNoteHint), w, fields::Look::default()));
                });
                if let Some((what, next, raw)) = err.as_ref() {
                    states::err_box(ui, "add-grant-err", what, next, t(Key::U3RawError), raw);
                }
            },
            |ui, me| {
                go = page::Page::new().primary_with(ui, t(Key::V2AddGrantGo), !me.typed.vt_typed.trim().is_empty()).1.clicked();
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
                Applied::Held { grant, .. } => {
                    self.ux.u4.import_open = false;
                    self.ux.u4.import_err = None;
                    self.ux.sheet_shake = None;
                    self.typed.vt_typed.clear();
                    // The notes belong to the added grant (the chain's last hop), not its upstreams, whose issuers
                    // are others.
                    let (note, issuer_note) = (std::mem::take(&mut self.typed.vt_note), std::mem::take(&mut self.typed.vt_issuer_note));
                    if !grant.is_empty() && (!note.trim().is_empty() || !issuer_note.trim().is_empty()) {
                        self.act(Action::NoteHeld { grant, note, issuer_note }, now);
                    }
                }
                Applied::Trouble(f) => {
                    self.ux.sheet_shake = Some(now);
                    self.ux.u4.import_err = Some(match self.import_face(&f) {
                        Some((what, next)) => (what.to_string(), next.to_string(), f.raw()),
                        None => (f.human().to_string(), f.next().to_string(), f.raw()),
                    });
                }
                _ => {}
            }
        }
    }

    /// Sublicense: which record, whose, and the upstream validity, with ids under details. "Draft" (a guide key:
    /// it only opens the form) fills the upstream and record into the form and opens the sublicense page. The
    /// folder sheets over a grant's page are drawn from here too (`grants::OutSheet`).
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

/// A failure told in a sheet in the callout's form: the bad wash with no edge, corner 12, 10 × 12 inside, the
/// failure mark (16) before the words; what happened in the strong role and the bad ink, what to do next under
/// it, the raw error folded under its title.
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
