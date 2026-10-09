//! The sheets of both seats' pages: new record (two steps), send (gas estimate first), delete record, grant
//! and revoke confirmations, the exclusive-grant clash, import existing records, key change or handover (two
//! steps), annotate, attest for someone, add a held grant, and the sublicense confirmation.
//!
//! One sheet at a time: opening one closes the others. Sheets lie above the first-run wizard, which stays
//! open under them; while the passcode gate is up no sheet is drawn.

use super::*;

impl Win {
    // ─── Which sheet is open ───

    /// Whether any sheet is open (keyboard shortcuts and the whole-window drop stand aside).
    pub(super) fn any_sheet(&self) -> bool {
        let u3 = &self.ux.u3;
        u3.new_anchor.is_some()
            || u3.confirm.is_some()
            || u3.form.is_some()
            || u3.kit_pick.is_some()
            || self.clash_modal
            || self.ux.u4.import_open
            || self.ux.u4.confirm.is_some()
            || self.ux.id_modal.is_some()
            || self.ux.confirm_genesis
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
        self.clash_modal = false;
        self.ux.u4.import_open = false;
        self.ux.u4.import_err = None;
        self.ux.u4.confirm = None;
        if self.ux.id_modal.is_some() {
            self.ux.id_close();
        }
        self.ux.confirm_genesis = false;
        self.ux.sheet_shake = None;
        self.ux.bk_clear();
    }

    /// Whether any sheet is up (Esc belongs to it, not to the layer under it).
    pub(super) fn any_sheet_open(&self) -> bool {
        self.ux.id_modal.is_some()
            || self.ux.bk.is_some()
            || self.ux.confirm_genesis
            || self.ux.wiz_exit_ask
            || self.ux.u3.new_anchor.is_some()
            || self.ux.u3.confirm.is_some()
            || self.ux.u3.form.is_some()
            || self.ux.u3.kit_pick.is_some()
            || self.ux.u4.import_open
            || self.ux.u4.confirm.is_some()
            || self.clash_modal
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
            self.ux.wiz_from = Some((self.shell.settings.role, self.ux.stack, self.ux.place, self.ux.hist.get(&self.ux.stack).cloned()));
            // Whether this is a true first run (`firstrun::fresh_machine`) is recorded once as the wizard opens and
            // holds for the whole run (setting the passcode in step 1 must not make a way out appear halfway
            // through).
            self.ux.wiz_fresh = crate::firstrun::fresh_machine(&self.shell);
        }
        self.ux.wizard = Some(step);
    }

    /// Whether the wizard offers a way out: always, except when it was opened on a true first run (no passcode
    /// and no identity), where there is nothing to go back to.
    pub(super) fn wizard_exit_shown(&self) -> bool {
        !self.ux.wiz_fresh
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
        if let Some((role, stack, place, hist)) = self.ux.wiz_from.take() {
            if self.shell.settings.role != role && !self.shell.seat_unseated() {
                self.act(Action::SwitchRole, now);
            }
            self.ux.stack = stack;
            if let Some(p) = place {
                self.go(p, now);
            }
            // Back to the exact page it was opened on (a settings section, a detail page), with its history.
            if let Some(h) = hist {
                self.ux.hist.insert(stack, h);
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

    /// After content is taken, an empty record name takes the content's name (a file without its extension, a
    /// folder or repository whole).
    pub(super) fn took_named(&mut self) {
        if self.typed.work_note.trim().is_empty() {
            if let Some(c) = self.shell.content.as_ref() {
                let name = width::file_name(&c.subject);
                // A file's name loses its extension; a folder or repository keeps its whole name.
                let stem = match (c.size, name.rsplit_once('.')) {
                    (Some(_), Some((s, _))) if !s.is_empty() => s.to_string(),
                    _ => name,
                };
                self.typed.work_note = stem;
            }
        }
    }

    // ─── Drawing ───

    /// The confirmation sheet shown when a fetch finds this home at odds with the fetched ledger: the entries
    /// that would remain only in the old data, "cancel", and "fetch and replace" (this home is kept as old data
    /// and a fresh one receives the fetched ledger, using the password the fetch was given).
    fn conflict_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let Some(c) = self.shell.fetch_conflict.clone() else { return };
        let ready = !self.typed.fetch_from.trim().is_empty() && !self.ux.fetch_held.is_empty() && !self.shell.tasks.in_flight(crate::task::Kind::Fetch);
        let rows: Vec<(String, Val)> = c.rows.iter().map(|(r, at)| (format!("#{} \u{b7} {}", r.seq, human_summary(r)), Val::text(at.map(crate::when::short).unwrap_or_default()))).collect();
        let (mut sure, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("fetch-conflict", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, &fill1(Key::ConflictTitle, &c.offline.to_string()), "");
                states::note_box(ui, t(Key::ConflictSay));
                let refs: Vec<(&str, Val)> = rows.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                kv::kv(ui, &refs);
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
        self.clash_sheet(ctx);
        self.add_grant_sheet(ctx, now);
        self.relicense_sheet(ctx, now);
        self.id_sheets(ctx, now);
        self.genesis_sheet(ctx, now);
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

    /// New record: choose the content, name it, optionally say for whom; then the confirmation step (file, for
    /// whom, cost; the fingerprint and the ledger's place under details).
    fn new_anchor_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let Some(step) = self.ux.u3.new_anchor else { return };
        let content = self.shell.content.clone();
        let batch = self.typed.batch_files.clone();
        let ready = content.is_some() || !batch.is_empty();
        let hashing_now = self.shell.tasks.in_flight(crate::task::Kind::Take);
        let recording_now = self.shell.tasks.in_flight(crate::task::Kind::Record);
        // Both keys of the sheet (the guide key, then the commit) say what the write will do.
        let go = t(self.anchor_key());
        let go_commit = t(self.anchor_key());
        let note = t(if self.shell.settings.auto_anchor { Key::U3AnchorHintOn } else { Key::U3AnchorHint });
        let root = self.shell.home.as_ref().map(|h| h.root().display().to_string()).unwrap_or_default();
        let repo_say = match (&self.shell.settings.repo, self.shell.repo_since.as_ref()) {
            (None, _) => t(Key::U3RepoNone).to_string(),
            (Some(_), Some((_, Some(n)))) => fill1(Key::U3RepoGrew, &n.to_string()),
            (Some(_), _) => t(Key::U3RepoUnread).to_string(),
        };
        let for_say = {
            let f = &self.typed.for_fields;
            if f.iter().all(|x| x.trim().is_empty()) {
                t(Key::None_).to_string()
            } else {
                f.iter().map(|x| x.trim()).filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" \u{b7} ")
            }
        };
        let cost = t(if self.shell.settings.auto_anchor { Key::U3CostSendNext } else { Key::U3CostQueue });
        let mut picked: Option<Vec<String>> = None;
        let (mut next, mut back, mut close, mut commit, mut register, mut check_repo, mut clear_batch) = (false, false, false, false, false, false, false);
        let slide = if step == 1 { sheet::Slide::Forward } else { sheet::Slide::Back };
        let out = sheet::show(
            ctx,
            sheet::Spec::new("new-record", tk::SHEET_WIDE).step(step as u64, slide),
            self,
            |ui, me| {
                if step == 0 {
                    sheet::title(ui, t(Key::V2NewAnchor), "");
                    let chosen: Option<(String, String)> = if !batch.is_empty() {
                        Some((fill1(Key::BatchChosen, &batch.len().to_string()), String::new()))
                    } else {
                        content.as_ref().map(|x| (fill1(Key::U3Chosen, &width::file_name(&x.subject)), x.size.map(size_say).unwrap_or_else(|| x.detail.clone())))
                    };
                    let d = drop::zone(
                        ui,
                        "new-record-drop",
                        Some(Glyph::Inbox),
                        &[t(Key::U3DropTitle), t(Key::DropClickAny)],
                        chosen.as_ref().map(|(a, b)| (a.as_str(), b.as_str(), t(Key::DropClickSwap))),
                        140.0,
                        drop::Shape::Column,
                        true,
                    );
                    if !d.dropped.is_empty() {
                        picked = Some(d.dropped.clone());
                    } else if let Some(p) = path_answer(ui.ctx(), egui::Id::new("zikaron-path-new-record"), d.clicked, crate::platform::Pick::FileOrFolder) {
                        picked = Some(vec![p]);
                    }
                    if !batch.is_empty() {
                        for p in &batch {
                            let room = ui.available_width();
                            paint::line(ui, &width::file_name(p), Type::Small, c(C::Ink2), room);
                        }
                        clear_batch = key::link(ui, t(Key::PickClear)).clicked();
                    }
                    field(ui, t(Key::U3WorkName), None, |ui| input::line(ui, &mut me.typed.work_note, ""));
                    fold::fold(ui, "new-record-for", t(Key::ForFold), |ui| {
                        card::grid(ui, "new-record-for-grid", 4, 180.0, |ui, i| {
                            let k = [Key::ForApp, Key::ForIdentity, Key::ForRef, Key::ForSeat][i];
                            field(ui, t(k), None, |ui| input::line(ui, &mut me.typed.for_fields[i], ""));
                        });
                        hint(ui, t(Key::ForNote));
                    });
                    fold::fold(ui, "new-record-more", t(Key::U3MoreOptions), |ui| {
                        if me.typed.repo_path.is_empty() {
                            if let Some(r) = &me.shell.settings.repo {
                                me.typed.repo_path = r.path.clone();
                            }
                        }
                        field(ui, t(Key::U3RepoLabel), None, |ui| pick_path(ui, &mut me.typed.repo_path, crate::platform::Pick::Folder));
                        hint(ui, &repo_say);
                        keys_row(ui, |ui| {
                            register = key::key(ui, t(Key::DoRegisterRepo), Role::Secondary, !me.typed.repo_path.trim().is_empty()).clicked();
                            check_repo = key::key(ui, t(Key::DoCheckRepo), Role::Secondary, true).clicked();
                        });
                    });
                } else {
                    sheet::title(ui, go_commit, "");
                    let file = if batch.is_empty() { content.as_ref().map(|x| width::file_name(&x.subject)).unwrap_or_default() } else { fill1(Key::BatchChosen, &batch.len().to_string()) };
                    kv::kv(ui, &[(t(Key::U3File), Val::text(file)), (t(Key::ForFold), Val::text(for_say.clone())), (t(Key::CfCost), Val::text(cost))]);
                    states::note_box(ui, t(Key::U3AnchorNote));
                    let hex = if batch.is_empty() { content.as_ref().map(|x| x.hex()).unwrap_or_default() } else { t(Key::None_).to_string() };
                    details(ui, "new-record-details", &[(t(Key::ContentHash), Val::mono(hex)), (t(Key::CfWhere), Val::mono(root.clone()))]);
                }
            },
            |ui, _me| {
                if step == 0 {
                    // While the fingerprint is computed (`Kind::Take`) the key shows a spinner and ignores presses;
                    // "back" stays.
                    let hashing = if hashing_now { Phase::Busy { frac: None } } else { Phase::Idle };
                    next = key::show(ui, key::Key::new(go, Role::Guide).enabled(ready).phase(hashing).busy_text(t(Key::U3Hashing))).clicked();
                    close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                    sheet::foot_note(ui, note);
                } else {
                    // While fingerprints are computed and entries written (`Kind::Record`) the key shows a spinner
                    // and ignores presses; "back" stays.
                    let recording = if recording_now { Phase::Busy { frac: None } } else { Phase::Idle };
                    commit = page::Pen::new().press_saying(ui, go_commit, t(Key::U3Recording), ready, recording).clicked();
                    close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        back = key::show(ui, key::Key::new(t(Key::WizPrev), Role::Plain).lead(Glyph::Back)).clicked();
                    });
                }
            },
        );
        if let Some(paths) = picked {
            self.take_for_record(paths, now);
        }
        if clear_batch {
            self.typed.batch_files.clear();
        }
        if register {
            let a = Action::RegisterRepo { path: self.typed.repo_path.clone() };
            self.act(a, now);
        }
        if check_repo {
            self.act(Action::CheckRepo, now);
        }
        if next {
            self.ux.u3.new_anchor = Some(1);
        }
        if back {
            self.ux.u3.new_anchor = Some(0);
        }
        if close || out.esc {
            self.ux.u3.new_anchor = None;
        }
        if commit {
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

    /// Put on chain: the first n entries and the gas cap (loading until estimated), with the gas figure under
    /// details. Pressing runs "sign, broadcast" in the key; the sheet closes once the broadcast lands. A failed
    /// estimate turns the sheet to "cannot estimate gas" with a retry.
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
                    sheet::title(ui, t(Key::U3SendTitle), "");
                    let cap_val = if gas.is_some() { Val::mono(fill1(Key::SetGasSay, &cap)) } else { Val::Loading(110.0) };
                    kv::kv(ui, &[(t(Key::LedgerEntries), Val::text(first.clone())), (t(Key::U3FeeCap), cap_val)]);
                    // Where the cap came from: one line when it is the fallback pair, once the figures are in.
                    if let (Some(_), Some(k)) = (gas, crate::chainx::fee_source_line(fees_now.as_ref())) {
                        states::okline(ui, Mark::Warn, t(k));
                    }
                    // Name each node the fees were not read from (it serves another chain).
                    if gas.is_some() && !fees_left.is_empty() {
                        states::okline(ui, Mark::Warn, &fill1(Key::U3FeeLeftNodes, &fees_left.join(" · ")));
                    }
                    if let Some(g) = gas {
                        details(ui, "send-details", &[(t(Key::U3GasEstimate), Val::mono(g.to_string()))]);
                    }
                    states::note_box(ui, t(Key::U3SendNote));
                }
                Some((why, next, raw)) => {
                    sheet::title(ui, t(Key::U3NoEstimateTitle), "");
                    kv::kv(ui, &[(t(Key::LedgerEntries), Val::text(first.clone())), (t(Key::U3NoEstimateWhy), Val::text(why.clone()))]);
                    fold::fold(ui, "no-estimate-details", t(Key::SetEvidence), |ui| {
                        paint::text(ui, raw, Type::MonoSmall, c(C::Ink2));
                    });
                    states::note_box(ui, next);
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
                sheet::title(ui, t(if unheld { Key::U3ResendKey } else { Key::U3BumpTitle }), "");
                let at_gas = |per_gas: u64| Val::mono(fill1(Key::SetGasSay, &eth_cap(u128::from(per_gas) * u128::from(stuck.sent.gas_limit))));
                let mut rows = vec![(t(Key::LedgerEntries), Val::text(entries.to_string()))];
                if !unheld {
                    rows.push((t(Key::U3BumpOldCap), at_gas(stuck.sent.max_fee)));
                }
                rows.push((t(Key::U3BumpPriceNow), at_gas(base_now)));
                rows.push((t(Key::U3BumpNewCap), Val::mono(fill1(Key::SetGasSay, &eth_cap(fees.cap_wei())))));
                kv::kv(ui, &rows);
                // Name each node the current price was not read from (it serves another chain).
                if !stuck.left.is_empty() {
                    states::okline(ui, Mark::Warn, &fill1(Key::U3FeeLeftNodes, &stuck.left.join(" · ")));
                }
                states::note_box(ui, &fill1(if unheld { Key::U3UnheldNote } else { Key::U3BumpNote }, &crate::queue::RESENDS_MAX.to_string()));
                details(ui, "bump-details", &[(t(Key::U3GasEstimate), Val::mono(stuck.sent.gas_limit.to_string())), (t(Key::CfWhere), Val::mono(stuck.txs.join(" ")))]);
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
    /// details.
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
                kv::kv(ui, &[(t(Key::U3Work), Val::text(row.as_ref().map(human_summary).unwrap_or_default()))]);
                states::note_box(ui, t(Key::V2DeleteNote));
                details(ui, "delete-details", &[(t(Key::ContentHash), Val::mono(row.as_ref().and_then(|x| x.work.clone()).unwrap_or_default())), (t(Key::CfWhere), Val::mono(root.clone()))]);
            },
            |ui, _me| {
                go = page::Pen::new().press(ui, t(Key::V2DeleteGo), true).clicked();
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

    /// Sign a grant (a sublicense when it has an upstream): record, validity, terms file and cost up front;
    /// upstream, grantee and fingerprints under details.
    fn grant_confirm_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let upstream = self.typed.g_upstream.trim().to_string();
        let record = if upstream.is_empty() { self.work_label(self.typed.g_work.trim()) } else { self.held_record_name(&upstream) };
        let window = self.window_days(match (self.typed.g_from.trim().parse::<u64>(), self.typed.g_to.trim().parse::<u64>()) {
            (Ok(a), Ok(b)) => Some((a, b)),
            _ => None,
        });
        let terms = self.ux.u3.terms.as_ref().filter(|x| x.hex == self.typed.g_terms.trim()).map(|x| x.name.clone()).unwrap_or_else(|| t(Key::TermsTyped).to_string());
        let cost = t(if self.shell.settings.auto_anchor { Key::U3CostSendNext } else { Key::U3CostQueue });
        let go_label = t(self.anchor_key());
        let mut raw: Vec<(&str, Val)> = Vec::new();
        if !upstream.is_empty() {
            raw.push((t(Key::U4Upstream), Val::mono(upstream.clone())));
        }
        raw.push((t(Key::U3ToWhom), Val::mono(self.typed.g_grantee.trim().to_string())));
        raw.push((t(Key::ContentHash), Val::mono(self.typed.g_work.trim().to_string())));
        raw.push((t(Key::U3TermsHash), Val::mono(self.typed.g_terms.trim().to_string())));
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("grant-confirm", tk::SHEET_WIDE),
            self,
            |ui, _me| {
                sheet::title(ui, t(if upstream.is_empty() { Key::U3GrantKey } else { Key::U4RelicenseGo }), "");
                kv::kv(ui, &[(t(Key::U3Work), Val::text(record.clone())), (t(Key::U3Window), Val::text(window.clone())), (t(Key::U3TermsFile), Val::text(terms.clone())), (t(Key::CfCost), Val::text(cost))]);
                states::note_box(ui, t(Key::U3GrantNote));
                details(ui, "grant-confirm-details", &raw);
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

    /// Revoke a grant: the record and its original validity, with the grantee and the optional ruling file
    /// fingerprint under details.
    fn revoke_sheet(&mut self, ctx: &egui::Context, grant: &str, now: f64) {
        let g = self.shell.grants.as_ref().and_then(|x| x.iter().find(|r| r.id == grant).cloned());
        let record = g.as_ref().map(|x| self.work_label(&x.work)).unwrap_or_default();
        let window = self.window_days(g.as_ref().and_then(|x| x.window));
        let grantee = g.as_ref().map(|x| x.grantee.clone()).unwrap_or_default();
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("revoke", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::U3RevokeTitle), "");
                kv::kv(ui, &[(t(Key::U3Work), Val::text(record.clone())), (t(Key::U3OldWindow), Val::text(window.clone()))]);
                states::note_box(ui, t(Key::U3RevokeNote));
                fold::fold(ui, "revoke-details", t(Key::SetEvidence), |ui| {
                    kv::kv(ui, &[(t(Key::U3ToWhom), Val::mono(grantee.clone()))]);
                    field(ui, t(Key::RevokeCase), None, |ui| input::mono(ui, &mut me.ux.u3.revoke_case, t(Key::U3Optional)));
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

    /// The exclusive-grant clash (read only): what happened, what to do, the ids under error details, and the
    /// grants it clashes with.
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
                states::err_box(ui, "clash-what", t(Key::ClashBody), t(Key::ExclusiveNote), t(Key::U3RawError), &rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>().join("\n"));
                let lines: Vec<(Mark, String, Option<String>)> = rows
                    .iter()
                    .map(|r| (Mark::Bad, fill2(Key::ClashRow, &r.seq.to_string(), &window_short(r.window)), Some(t(Key::ExclusiveMark).to_string())))
                    .collect();
                states::checks(ui, &lines, false);
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
        let table = |ui: &mut egui::Ui, off: &std::collections::BTreeSet<String>, flip: &mut Option<String>, select_all: &mut bool| {
            let title = t(if other { Key::U3AdOnChain } else { Key::U3AdEarlier });
            match (&listed, &failed) {
                (Some(rows), _) => {
                    width::then_line(
                        ui,
                        |ui| {
                            if !rows.is_empty() && key::link(ui, t(Key::U3AdSelectAll)).clicked() {
                                *select_all = true;
                            }
                        },
                        |ui, room| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = tk::S1;
                                ui.set_max_width(room);
                                paint::text(ui, title, Type::Note, c(C::Ink2));
                                paint::text(ui, &fill2(Key::U3AdCount, &chain_name, &rows.len().to_string()), Type::Small, c(C::Ink3));
                            });
                        },
                    );
                    if rows.is_empty() {
                        hint(ui, t(Key::Nothing));
                        return;
                    }
                    let cols = [table::col("", table::Col::Px(48.0)), table::col(t(Key::U3AdColTime), table::Col::Fr(1.0)), table::col_r(t(Key::U3State), table::Col::Px(110.0))];
                    let lines: Vec<table::Row> = rows
                        .iter()
                        .map(|k| {
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
                            table::Row {
                                cells: vec![table::Cell::Switch(on, passed), table::Cell::Mono(crate::when::when(k.time)), table::Cell::Pill(say.to_string(), tone)],
                                click: passed,
                                gone: false,
                                on,
                            }
                        })
                        .collect();
                    if let Some(i) = table::table(ui, "adopt-rows", &cols, true, &lines, "").clicked {
                        *flip = Some(key_of(&rows[i]));
                    }
                    let raw: Vec<(String, String)> = rows.iter().map(|k| (crate::when::when(k.time), fill2(Key::AdBlockAt, &k.block.to_string(), &crate::ledgerx::short(&k.row.content)))).collect();
                    let raw_rows: Vec<(&str, Val)> = raw.iter().map(|(a, b)| (a.as_str(), Val::mono(b.clone()))).collect();
                    details(ui, "adopt-rows-details", &raw_rows);
                }
                (None, Some(f)) => {
                    paint::text(ui, title, Type::Note, c(C::Ink2));
                    states::checks(ui, &[(Mark::Bad, f.human().to_string(), None)], false);
                }
                (None, None) => {
                    paint::text(ui, title, Type::Note, c(C::Ink2));
                    // Do not query an address still being typed: say what it should look like instead of "verifying".
                    let say = if !formed {
                        Key::FaultNextAddressShape
                    } else if can_list {
                        Key::VaultBusy
                    } else {
                        Key::U4ChainUnread
                    };
                    hint(ui, t(say));
                }
            }
        };
        let out = sheet::show(
            ctx,
            sheet::Spec::new("adopt", tk::SHEET_XWIDE),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::U3AdoptTitle), "");
                let off = me.ux.u3.adopt_off.clone();
                if !other {
                    table(ui, &off, &mut flip, &mut select_all);
                }
                field(ui, t(Key::IdAddress), None, |ui| {
                    let heads: Vec<String> = book.iter().map(|a| head_tail(a)).collect();
                    let items: Vec<menu::Item> = heads.iter().map(|h| menu::Item::Row(menu::Row { lead: t(Key::Address), label: h, mono: true, ..Default::default() })).collect();
                    let spec = pick::Spec { hint: t(Key::SearchAddress), empty: t(Key::SearchNone), w: PICK_W, dates: None };
                    let keep = |i: usize, q: &str, _: &str, _: &str| matches(q, &[&book[i]]);
                    let mut picked = None;
                    width::then(
                        ui,
                        |ui| picked = pick::key(ui, "adopt-book", t(Key::U3SeenBeforeDots), !book.is_empty(), false, &spec, &items, &keep),
                        |ui, room| input::field(ui, &mut me.typed.ad_key, "0x\u{2026}", room, input::Look { mono: true, ..Default::default() }),
                    );
                    if let Some(i) = picked {
                        booked = book.get(i).cloned();
                    }
                });
                if other {
                    table(ui, &off, &mut flip, &mut select_all);
                    card::flat(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = tk::S2;
                        width::then(
                            ui,
                            |ui| {
                                let (word, tone) = match sig_ok {
                                    None => (t(Key::U3AdWait), PillTone::Warn),
                                    Some(true) => (t(Key::AuditPass), PillTone::Ok),
                                    Some(false) => (t(Key::DeliveryMismatch), PillTone::Bad),
                                };
                                mark::pill_pop(ui, word, tone, 0.0);
                            },
                            |ui, room| {
                                paint::line(ui, t(Key::U3AdCosign), Type::Strong, c(C::Ink), room);
                            },
                        );
                        hint(ui, t(Key::U3AdSendTo));
                        width::then(
                            ui,
                            |ui| {
                                if key::key(ui, t(Key::U3AdCopy), Role::Secondary, text.is_some()).clicked() {
                                    copy = text.clone();
                                }
                            },
                            |ui, room| paint::line(ui, t(Key::ChannelText), Type::Body, c(C::Ink), room),
                        );
                        width::then(
                            ui,
                            |ui| paste = key::key(ui, t(Key::U3AdPaste), Role::Secondary, true).clicked(),
                            |ui, room| paint::line(ui, t(Key::U3AdSig), Type::Body, c(C::Ink), room),
                        );
                        hint(ui, t(Key::U3AdResign));
                        details(
                            ui,
                            "adopt-sig-details",
                            &[
                                (t(Key::ChannelText), Val::mono(text.clone().unwrap_or_else(|| t(Key::None_).to_string()))),
                                (t(Key::U3AdSig), Val::mono(if sig.is_empty() { t(Key::None_).to_string() } else { sig.clone() })),
                            ],
                        );
                    });
                }
                fold::fold(ui, "adopt-hand", t(Key::U3AdByHand), |ui| {
                    input::area_hint(ui, &mut me.typed.ad_rows, 2, t(Key::AdHandHint));
                    verify_hand = key::key(ui, t(Key::U3AdCheckHand), Role::Secondary, !hand.is_empty() && !busy).clicked();
                    if let (Some(p), Some(rows)) = (me.shell.proofs.as_ref(), me.shell.proofs_rows.as_deref()) {
                        if rows == hand {
                            hint(ui, &fill2(Key::SaidProofs, &p.len().to_string(), &p.iter().filter(|x| x.ok()).count().to_string()));
                        }
                    }
                });
            },
            |ui, _me| {
                go = page::Pen::new().press(ui, &fill1(Key::U3AdGo, &n.to_string()), n > 0).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                sheet::foot_note(ui, &fill1(Key::U3AdCan, &n.to_string()));
            },
        );
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
                if !step {
                    field(ui, t(Key::U3HandTo), None, |ui| {
                        input::mono(ui, &mut me.typed.sc_to, "0x\u{2026}");
                        if busy && formed {
                            paint::text(ui, t(Key::VaultBusy), Type::Small, c(C::Ink3));
                        } else if let Some((_, anchors, _)) = &sighting {
                            states::okline(ui, if *anchors == 0 { Mark::Ok } else { Mark::Bad }, &fill1(Key::SaidSighting, &anchors.to_string()));
                        } else if let Some(f) = &failed {
                            states::okline(ui, Mark::Bad, f.human());
                        }
                    });
                    field(ui, t(Key::U3HandKind), None, |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = tk::S3;
                            let items: Vec<menu::Item> = words.iter().enumerate().map(|(i, w)| menu::Item::Row(menu::Row { label: w, check: Some(kind_now == Some(i)), ..Default::default() })).collect();
                            picked = menu::menu_key(ui, "succeed-kind", t(Key::U3HandKindPick), true, 160.0, &items);
                            if kind_now.is_some() {
                                paint::text(ui, &fill1(Key::PickerChosen, &kind_words), Type::Note, c(C::Ink2));
                            }
                        });
                    });
                    field(ui, t(Key::U3HandSay), None, |ui| input::line(ui, &mut me.typed.sc_statement, t(Key::U3Optional)));
                    states::note_box(ui, t(Key::U3SucceedNote));
                } else {
                    kv::kv(ui, &[(t(Key::U3HandKind), Val::text(kind_words.clone())), (t(Key::U3HandSay), Val::text(say.clone()))]);
                    states::note_box(ui, t(Key::U3SucceedNote));
                    details(ui, "succeed-details", &[(t(Key::U3HandTo), Val::mono(to.clone()))]);
                }
            },
            |ui, _me| {
                if !step {
                    next = page::Page::new().primary_with(ui, t(Key::U3SucceedGo), clean && kind_now.is_some()).1.clicked();
                    close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                } else {
                    commit = page::Pen::new().press(ui, t(Key::U3SucceedGo), true).clicked();
                    close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        back = key::show(ui, key::Key::new(t(Key::WizPrev), Role::Plain).lead(Glyph::Back)).clicked();
                    });
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
                field(ui, t(Key::AnnotateText), None, |ui| input::area_words(ui, &mut me.typed.annotate_note, 2, ""));
                states::note_box(ui, t(Key::U3AnnotateNote));
                field(ui, t(Key::U3AnnotateWhich), None, |ui| {
                    if !read {
                        hint(ui, t(Key::U3AnnotateNoRows));
                        return;
                    }
                    // A picker over this ledger, newest first: number, type, summary and first anchor time.
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
                    let mut picked = None;
                    width::then(
                        ui,
                        |ui| picked = pick::key(ui, "annotate-pick", t(Key::KitPickOpen), true, false, &spec, &items, &keep),
                        |ui, room| match current {
                            Some(i) => {
                                paint::line(ui, &format!("{} {}", faces[i].0, faces[i].1), Type::Body, c(C::Ink), room);
                            }
                            None => {
                                paint::line(ui, t(Key::U3AnnotatePickHint), Type::Note, c(C::Ink2), room);
                            }
                        },
                    );
                    if let Some(i) = picked {
                        me.typed.annotate_subject = rows[i].id.clone();
                    }
                });
                fold::fold(ui, "annotate-more", t(Key::U3MoreOptions), |ui| {
                    field(ui, t(Key::U3AnnotateTyped), None, |ui| input::mono(ui, &mut me.typed.annotate_subject, t(Key::DetailPickHint)));
                });
            },
            |ui, _me| {
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
        let (mut sign, mut close) = (false, false);
        let mut copy: Option<String> = None;
        let out = sheet::show(
            ctx,
            sheet::Spec::new("attest", tk::SHEET_W),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::U3AttestTitle), "");
                field(ui, t(Key::U3AttestPaste), None, |ui| input::area(ui, &mut me.typed.at_text, 3));
                if let Some(f) = &trouble {
                    states::hint_ex(ui, f.human(), true);
                }
                if let Some((cl, blocks)) = &claim {
                    let anchors: Vec<String> = cl
                        .rows
                        .iter()
                        .zip(blocks.iter())
                        .map(|(r, b)| format!("{} \u{b7} {}", b.map(|x| x.to_string()).unwrap_or_else(|| t(Key::None_).to_string()), crate::ledgerx::short(&r.content)))
                        .collect();
                    kv::kv(ui, &[(t(Key::U3AttestAnchors), Val::text(fill1(Key::AnchorsN, &cl.rows.len().to_string())))]);
                    details(
                        ui,
                        "attest-details",
                        &[
                            (t(Key::U3AttestWho), Val::mono(cl.adopter.clone())),
                            (t(Key::U3AttestAnchors), Val::mono(format!("{} \u{b7} {}", cl.rows.len(), anchors.join(" / ")))),
                            (t(Key::U3AttestHead), Val::mono(cl.prev.clone())),
                        ],
                    );
                    match &signed {
                        Some((_, s)) => {
                            width::then(
                                ui,
                                |ui| {
                                    if key::key(ui, t(Key::U3AdCopy), Role::Secondary, true).clicked() {
                                        copy = Some(s.clone());
                                    }
                                },
                                |ui, room| paint::line(ui, t(Key::U3AttestDone), Type::Body, c(C::Ink), room),
                            );
                        }
                        None => {
                            field(ui, t(Key::U3AttestPin), None, |ui| {
                                if pin::pin_row(ui, "attest-pin", &mut me.ux.pin, crate::keybox::PIN_LEN, me.ux.pin_shake, !busy, false).full {
                                    sign = true;
                                }
                            });
                        }
                    }
                }
            },
            |ui, _me| {
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if let Some(x) = copy {
            ctx.copy_text(x);
            self.toasts.say(t(Key::U3CopiedSig), Tone::Note, now);
        }
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

    /// Add a held grant: paste the grant code or drop a grant file (read and checked at once). It is stored
    /// only after verification; otherwise the two sentences are shown with the raw error folded.
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
        let (mut go, mut close) = (false, false);
        let mut dropped: Option<String> = None;
        let out = sheet::show(
            ctx,
            sheet::Spec::new("add-grant", tk::SHEET_W).shake(self.ux.sheet_shake),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::V2AddGrant), "");
                input::area_hint(ui, &mut me.typed.vt_typed, 3, t(Key::V2PasteGrant));
                let d = drop::zone(ui, "add-grant-drop", None, &[t(Key::U3CheckDrop), t(Key::DropClickFile)], None, 64.0, drop::Shape::Column, true);
                dropped = match d.dropped.first() {
                    Some(p) => Some(p.clone()),
                    None => path_answer(ui.ctx(), egui::Id::new("zikaron-path-add-grant"), d.clicked, crate::platform::Pick::File),
                };
                match &seen {
                    Some(Ok(said)) => states::okline(ui, Mark::Ok, said),
                    Some(Err(f)) => states::okline(ui, Mark::Bad, f.human()),
                    None => {}
                }
                // A pasted grant code (unlike a grant file) carries no issuer ledger: say so before adding.
                if me.typed.vt_typed.trim().to_ascii_lowercase().starts_with(zikaron_kit::tokens::BADGE_PREFIX) {
                    states::okline(ui, Mark::Warn, t(Key::V2CodeCarriesNoLedger));
                }
                // The user's own names for this grant and its issuer, kept on this machine only.
                field(ui, t(Key::V2GrantNote), None, |ui| input::line(ui, &mut me.typed.vt_note, t(Key::V2GrantNoteHint)));
                field(ui, t(Key::V2IssuerNote), None, |ui| input::line(ui, &mut me.typed.vt_issuer_note, t(Key::V2IssuerNoteHint)));
                if let Some((what, next, raw)) = err.as_ref() {
                    states::err_box(ui, "add-grant-err", what, next, t(Key::U3RawError), raw);
                }
            },
            |ui, me| {
                go = page::Page::new().primary_with(ui, t(Key::V2AddGrantGo), !me.typed.vt_typed.trim().is_empty()).1.clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            },
        );
        if let Some(p) = dropped {
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

    /// Sublicense: which record, whose, and the upstream validity, with ids under details. "Draft…" fills the
    /// upstream and record into the form and opens the sublicense page.
    fn relicense_sheet(&mut self, ctx: &egui::Context, now: f64) {
        let Some(U4Confirm::Relicense { grant }) = self.ux.u4.confirm.clone() else { return };
        let h = self.shell.held.clone().unwrap_or_default().into_iter().find(|x| x.id.eq_ignore_ascii_case(&grant));
        let record = self.held_record_name(&grant);
        let issuer = self.held_issuer_name(&grant);
        let window = self.window_days(h.as_ref().and_then(|x| x.window));
        let (mut go, mut close) = (false, false);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("relicense", tk::SHEET_WIDE),
            self,
            |ui, _me| {
                sheet::title(ui, t(Key::U4RelicenseTitle), "");
                kv::kv(ui, &[(t(Key::U3Work), Val::text(record.clone())), (t(Key::U3Issuer), Val::text(issuer.clone())), (t(Key::U4WindowCap), Val::text(window.clone()))]);
                states::note_box(ui, t(Key::U4RelicenseConfirmNote));
                details(
                    ui,
                    "relicense-details",
                    &[
                        (t(Key::U4Upstream), Val::mono(grant.clone())),
                        (t(Key::U3Issuer), Val::mono(h.as_ref().map(|x| x.author.clone()).unwrap_or_default())),
                        (t(Key::U3Work), Val::mono(h.as_ref().map(|x| x.work.clone()).unwrap_or_default())),
                    ],
                );
            },
            |ui, _me| {
                go = page::Pen::new().press(ui, t(Key::U4ToDrafter), h.is_some()).clicked();
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
