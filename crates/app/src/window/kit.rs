use super::*;

impl Win {
    /// Export a record kit: entries, the chosen records' originals, attachments, a note, file or folder and where
    /// it lands (this home's kits folder until another is chosen, the last one chosen after); "verify and export"
    /// runs in the background. Beside it: the depth reading and the kits exported on this machine.
    pub(super) fn kit_page(&mut self, ui: &mut egui::Ui, now: f64) {
        // Its attachment zone takes dropped files.
        self.takes_drops();
        self.ensure_rows(now);
        if self.typed.kit_out.trim().is_empty() {
            if let Some(h) = self.shell.home.as_ref() {
                self.typed.kit_out = h.dir(crate::home::Slot::Kits).display().to_string();
            }
        }
        let rows: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
        // The list and the export share one selection (`kitx::preview`), recomputed when the pick, the table or the
        // home changes.
        let home_key = self.shell.home.as_ref().map(|h| h.root().display().to_string()).unwrap_or_default();
        let key = format!("{}|{}|{}|{}|{}|{}", home_key, self.shell.rows_gen, self.typed.pick_from.trim(), self.typed.pick_to.trim(), self.typed.pick_ids.trim(), rows.len());
        if self.ux.u3.kit_preview.as_ref().map(|(k, _)| k != &key).unwrap_or(true) {
            let parsed = crate::kitx::Pick::parse_ids(&self.typed.pick_from, &self.typed.pick_to, &self.typed.pick_ids);
            let got = match (self.shell.home.as_ref(), &parsed) {
                (Some(h), Ok(p)) => crate::kitx::preview(h, p).map_err(|f| f.human().to_string()),
                (_, Err(f)) => Err(f.human().to_string()),
                (None, _) => Err(t(Key::WbNotRead).to_string()),
            };
            let orig = match (self.shell.home.as_ref(), &parsed) {
                (Some(h), Ok(p)) => crate::kitx::originals(h, p).map_err(|f| f.human().to_string()),
                (_, Err(f)) => Err(f.human().to_string()),
                (None, _) => Err(t(Key::WbNotRead).to_string()),
            };
            self.ux.u3.kit_orig = Some((key.clone(), orig));
            let admit = match (self.shell.home.as_ref(), &parsed) {
                (Some(h), Ok(p)) => crate::kitx::choose(h, p).ok().map(|x| crate::kitx::Originals::of(&x)),
                _ => None,
            };
            self.ux.u3.kit_admit = Some((key.clone(), admit));
            self.ux.u3.kit_preview = Some((key, got));
        }
        let (picked, pick_err) = match self.ux.u3.kit_preview.as_ref().map(|(_, r)| r) {
            Some(Ok((ids, _))) => (ids.clone(), None),
            Some(Err(e)) => (Vec::new(), Some(e.clone())),
            None => (Vec::new(), None),
        };
        let in_pick: Vec<crate::ledgerx::Row> = rows.iter().filter(|r| picked.iter().any(|i| i.eq_ignore_ascii_case(&r.id))).cloned().collect();
        let mut open_picker = false;
        let mut anchor_now: Vec<(String, bool)> = Vec::new();
        let reading_work = in_pick.iter().find(|r| r.kind == zikaron::tokens::EntryType::History).and_then(|r| r.work.clone().map(|w| (w, r.seq, human_summary(r))));
        let mut read_depth: Option<String> = None;
        let mut blocked = false;
        // Files dropped on the page are attachments (the page takes drops itself; none while a sheet is up).
        let mut got: Vec<String> = if sheet::up(ui.ctx()) { Vec::new() } else { drop::take_dropped(ui.ctx()) };
        let mut clear = false;
        let mut gone: Option<String> = None;
        card::two_cols(ui, tk::SIDE_W, tk::MAIN_MIN_W, |ui, side| match side {
            card::Side::Main => {
                // Which entries: the range with "select" (the whole ledger with a switch per entry); chosen entries
                // not yet on chain, with "put on chain now".
                let range = match (self.typed.pick_ids.trim(), self.typed.pick_from.trim(), self.typed.pick_to.trim()) {
                    ("", "", "") => fill1(Key::KitEntriesAll, &in_pick.len().to_string()),
                    ("", a, b) => fill3(Key::KitEntriesRange, a, b, &in_pick.len().to_string()),
                    _ => fill1(Key::KitPickCount, &in_pick.len().to_string()),
                };
                let (unanchored, sendable) = crate::kitx::unanchored(&in_pick.iter().collect::<Vec<_>>());
                stagger(ui, 0, |ui| {
                    card::form(ui, |ui, f| {
                        let (sub, value) = match pick_err.as_ref() {
                            Some(e) => (e.as_str(), ""),
                            None if in_pick.is_empty() => ("", t(Key::U3KitNothingPicked)),
                            None => ("", range.as_str()),
                        };
                        open_picker = f.act_value(ui, t(Key::KitEntries), sub, value, key::Key::new(t(Key::KitPickOpen), Role::Secondary)).clicked();
                        if !unanchored.is_empty() {
                            let k = key::Key::new(t(Key::U3SendNow), Role::Guide).enabled(!sendable.is_empty());
                            // The whole sentence (what the package will lack), then the key that sends them.
                            let said = fill1(Key::KitUnanchored, &unanchored.len().to_string());
                            f.panel(ui, |ui| zikaron_ui::callout::callout(ui, &said));
                            if f.keys(ui, |ui| key::show(ui, k).clicked()) {
                                anchor_now = sendable.clone();
                            }
                        }
                    });
                });
                // The chosen records' originals, from the local index: one row each (its file, its entry number,
                // "remove"). A file no longer at its signing place, or changed since, says so and is left out.
                stagger(ui, 1, |ui| {
                    let orig = self.ux.u3.kit_orig.as_ref().map(|(_, r)| r.clone());
                    let admit = self.ux.u3.kit_admit.as_ref().and_then(|(_, a)| a.clone());
                    card::section(ui, t(Key::KitOriginalsLabel), "", |ui| {
                        card::form(ui, |ui, f| {
                            f.keep(ui);
                            match orig {
                                Some(Ok(list)) if !list.is_empty() => {
                                    let mut removed = 0usize;
                                    for one in &list {
                                        if self.ux.u3.kit_orig_off.contains(&one.path) {
                                            removed += 1;
                                            continue;
                                        }
                                        let changed = matches!((self.shell.vetted.get(&one.path), admit.as_ref()), (Some(Ok(d)), Some(a)) if !a.admits_any_hex(d));
                                        let sub = if !one.present {
                                            t(Key::KitOriginalGone)
                                        } else if changed {
                                            t(Key::KitNotAnOriginalRow)
                                        } else {
                                            ""
                                        };
                                        f.key(("kit-orig", one.seq, one.path.as_str()));
                                        let (_, hit) = f.line(ui, Some(card::Lead::Glyph(Glyph::File)), &one.name, sub, false, false, c(C::Ink), |ui| {
                                            let hit = key::show_w(ui, key::Key::new(t(Key::KitRemove), Role::Secondary), ROW_KEY_W).clicked();
                                            ui.label(egui::RichText::new(format!("#{}", one.seq)).font(Type::MonoSmall.font()).color(c(C::Ink2)));
                                            hit
                                        });
                                        if hit {
                                            self.ux.u3.kit_orig_off.insert(one.path.clone());
                                        }
                                    }
                                    if removed > 0 {
                                        f.key("kit-orig-back");
                                        if f.act(ui, &fill1(Key::KitOriginalsRemoved, &removed.to_string()), "", key::Key::new(t(Key::KitOriginalsPutBack), Role::Secondary)).clicked() {
                                            self.ux.u3.kit_orig_off.clear();
                                        }
                                    }
                                }
                                Some(Err(e)) => {
                                    let _ = f.line(ui, Some(card::Lead::Mark(Mark::Bad)), &e, "", false, false, c(C::BadInk), |_| ());
                                }
                                _ => {
                                    let _ = f.line(ui, None, t(Key::Nothing), "", false, false, c(C::Ink3), |_| ());
                                }
                            }
                        })
                    });
                });
                // Attachments: any number, each read and digested in the background; "add" chooses one (files
                // dropped on the page are added too).
                stagger(ui, 2, |ui| {
                    let lines_in: Vec<String> = self.typed.kit_attach.lines().map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
                    let listed: Vec<String> = match self.ux.u3.kit_orig.as_ref().map(|(_, r)| r) {
                        Some(Ok(list)) => list.iter().filter(|o| o.present).map(|o| o.path.clone()).collect(),
                        _ => Vec::new(),
                    };
                    let pending: Vec<String> = lines_in.iter().chain(listed.iter()).filter(|x| !self.shell.vetted.contains_key(*x)).cloned().collect();
                    if !pending.is_empty() && !self.shell.tasks.in_flight(crate::task::Kind::Vet) {
                        self.auto(Action::VetAttachments { paths: pending }, now);
                    }
                    if !lines_in.is_empty() && self.ux.u3.kit_names.as_ref().map(|(k, _)| k != &self.typed.kit_attach).unwrap_or(true) {
                        let names: Vec<(String, Result<Option<(String, bool)>, String>)> = lines_in
                            .iter()
                            .map(|x| match crate::kitx::preview_names(std::path::Path::new(x)) {
                                Ok(v) if v.iter().any(|(_, n)| n.is_none()) => (x.clone(), Ok(None)),
                                Ok(v) => {
                                    let folder = v.is_empty() || v.iter().any(|(o, _)| o.contains('/'));
                                    let renamed = v.iter().any(|(o, n)| n.as_deref() != Some(o.as_str()));
                                    let base = width::file_name(x);
                                    let said = if folder { format!("{}/ \u{b7} {}", crate::kitx::kit_segment(&base).unwrap_or(base.clone()), v.len()) } else { v.first().and_then(|(_, n)| n.clone()).unwrap_or_default() };
                                    (x.clone(), Ok(Some((said, renamed))))
                                }
                                Err(f) => (x.clone(), Err(f.human().to_string())),
                            })
                            .collect();
                        self.ux.u3.kit_names = Some((self.typed.kit_attach.clone(), names));
                    }
                    let admit = self.ux.u3.kit_admit.as_ref().and_then(|(_, a)| a.clone());
                    let names = if lines_in.is_empty() { Vec::new() } else { self.ux.u3.kit_names.as_ref().map(|(_, r)| r.clone()).unwrap_or_default() };
                    let mut add = false;
                    let title = format!("{} \u{b7} {}", t(Key::KitAttach), t(Key::U3Optional));
                    card::section(ui, &title, "", |ui| {
                        card::form(ui, |ui, f| {
                            f.keep(ui);
                            for (path, named) in &names {
                                let sub = match (self.shell.vetted.get(path), named) {
                                    (None, _) => t(Key::KitVetting).to_string(),
                                    (Some(Err(e)), _) => e.human().to_string(),
                                    (Some(Ok(d)), _) if admit.as_ref().map(|a| !a.admits_any_hex(d)).unwrap_or(false) => t(Key::KitNotAnOriginalRow).to_string(),
                                    (Some(Ok(_)), Ok(Some((n, _)))) => fill1(Key::KitExportAs, n),
                                    (Some(Ok(_)), Ok(None)) => {
                                        blocked = true;
                                        t(Key::KitNameRefused).to_string()
                                    }
                                    (Some(Ok(_)), Err(e)) => e.clone(),
                                };
                                f.key(("kit-attach", path.as_str()));
                                let (_, hit) = f.line(ui, Some(card::Lead::Glyph(Glyph::File)), &width::file_name(path), &sub, false, false, c(C::Ink), |ui| key::show_w(ui, key::Key::new(t(Key::KitRemove), Role::Secondary), ROW_KEY_W).clicked());
                                if hit {
                                    gone = Some(path.clone());
                                }
                            }
                            f.key("kit-attach-add");
                            add = f.add_row(ui, t(Key::ReadNetAdd)).clicked();
                            if lines_in.len() > 1 {
                                f.key("kit-attach-clear");
                                clear = f.act(ui, t(Key::U3AttachClear), "", key::Key::new(t(Key::PickClear), Role::Secondary)).clicked();
                            }
                        });
                    });
                    got.extend(path_answer(ui.ctx(), egui::Id::new("zikaron-path-kit-attach"), add, crate::platform::Pick::FileOrFolder));
                    if clear {
                        for x in &lines_in {
                            self.shell.vetted.remove(x);
                        }
                    }
                });
                // The note (several lines), and whether it is one file or a folder.
                stagger(ui, 3, |ui| {
                    fields::card(ui, |ui, st| {
                        st.field(ui, t(Key::KitNoteLabel), None, None, |ui, _| input::area_words(ui, &mut self.typed.kit_note, 2, ""));
                    });
                });
                stagger(ui, 4, |ui| {
                    card::form(ui, |ui, f| {
                        // Encrypted, the package is one file: the folder switch is off and greyed meanwhile.
                        let sealed = self.ux.u3.kit_addressed;
                        if f.switch(ui, t(Key::KitAsFolder), "", self.ux.u3.kit_folder && !sealed, !sealed) {
                            self.ux.u3.kit_folder = !self.ux.u3.kit_folder;
                        }
                        if f.switch(ui, t(Key::KitAddressed), "", sealed, true) {
                            self.ux.u3.kit_addressed = !sealed;
                        }
                    });
                });
            }
            card::Side::Side => {
                // The reading against its baseline (kit law §9): shown only here; the export carries none of it.
                if let Some((work, seq, name)) = reading_work.clone() {
                    let depth = self.shell.depth.clone().filter(|(w, _)| w.eq_ignore_ascii_case(&work)).map(|(_, v)| crate::depthx::three(v));
                    let phase = self.phase_of(crate::task::Kind::Depth);
                    stagger(ui, 5, |ui| {
                        card::section(ui, t(Key::KitReadingTitle), "", |ui| {
                            card::form(ui, |ui, f| {
                                let _ = f.info(ui, &fill1(Key::KitReadingOf, &format!("#{seq} \u{b7} {name}")), card::Show::Empty(""));
                                match &depth {
                                    Some(x) => {
                                        let first = x.earliest.map(crate::when::day).unwrap_or_else(|| t(Key::None_).to_string());
                                        let _ = f.info(ui, t(Key::U3FirstAnchored), card::Show::Mono(&first));
                                        let _ = f.info(ui, t(Key::U3Deepest), card::Show::Text(&x.deepest.to_string()));
                                        let _ = f.info(ui, t(Key::U3Continuity), card::Show::Text(&format!("{} / {}", x.anchored, x.span)));
                                        let basis = if x.label.is_empty() { t(Key::None_).to_string() } else { label_human(&x.label) };
                                        let _ = f.info(ui, &fill1(Key::KitReadingBasis, &basis), card::Show::Empty(""));
                                    }
                                    None => {
                                        if f.act(ui, t(Key::U3Deepest), "", key::Key::new(t(Key::DoReadDepth), Role::Secondary).phase(phase)).clicked() {
                                            read_depth = Some(work.clone());
                                        }
                                    }
                                }
                            });
                            self.stage_line(ui, crate::task::Kind::Depth);
                        });
                    });
                }
                stagger(ui, 6, |ui| self.kits_index(ui));
            }
        });
        // The page's foot: "cancel" goes back; "verify and export" opens the sheet that says where it lands.
        let phase = self.phase_of(crate::task::Kind::Kit);
        let ready = !in_pick.is_empty() && !blocked;
        let (mut cancel, mut export_open) = (false, false);
        stagger(ui, 7, |ui| {
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), tk::KEY_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = tk::S2;
                // The page's only primary key.
                let why = t(if in_pick.is_empty() { Key::U3KitNothingPicked } else { Key::KitNameRefused });
                export_open = key::show(ui, key::Key::new(t(Key::U3KitKey), Role::Primary).enabled(ready).phase(phase).why(why)).clicked();
                cancel = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
            });
            self.stage_line(ui, crate::task::Kind::Kit);
            if let Some(Done::Kit { entries, files, left_out, unreadable, .. }) = self.shell.kit.clone() {
                motion::swap(ui, egui::Id::new("kit-made"), 1, |ui| {
                    states::banner(ui, states::Banner::Ok, &fill2(Key::KitMadeBanner, &entries.to_string(), &files.to_string()), |_| ());
                });
                for p in &left_out {
                    hint(ui, &format!("{} \u{b7} {}", width::file_name(p), t(Key::KitNotAnOriginalRow)));
                }
                for (p, f) in &unreadable {
                    hint(ui, &format!("{} \u{b7} {}", width::file_name(p), f.human()));
                }
            }
        });
        if let Some(g) = gone {
            self.shell.vetted.remove(&g);
            self.typed.kit_attach = self.typed.kit_attach.lines().filter(|x| x.trim() != g).collect::<Vec<_>>().join("\n");
        }
        if clear {
            self.typed.kit_attach.clear();
        }
        for p in got {
            // Adding the same path again re-checks it (the file may have changed).
            self.shell.vetted.remove(p.trim());
            if !self.typed.kit_attach.is_empty() && !self.typed.kit_attach.ends_with('\n') {
                self.typed.kit_attach.push('\n');
            }
            self.typed.kit_attach.push_str(&p);
        }
        if export_open {
            kit_sheet_set(ui.ctx(), Some(KitSheet::Export { waiting: false, sealed: false }));
        }
        if cancel {
            self.back(now);
        }
        let export = self.kit_sheets(ui.ctx(), in_pick.len(), ready);
        if let Some(work) = read_depth {
            self.act(Action::ReadDepth { work }, now);
        }
        if !anchor_now.is_empty() {
            // Queue the entries not yet queued, then open the confirmation for the last of them.
            for (id, fresh) in &anchor_now {
                if *fresh {
                    self.act(Action::QueueEntry { id: id.clone() }, now);
                }
            }
            let pos = self
                .shell
                .queue
                .items
                .iter()
                .filter(|q| !q.step.in_flight())
                .enumerate()
                .filter(|(_, q)| anchor_now.iter().any(|(id, _)| *id == q.id))
                .map(|(i, _)| i + 1)
                .max();
            if let Some(count) = pos {
                self.u3_open_confirm(U3Confirm::Send { count });
            }
        }
        if open_picker {
            // The picker opens with the current choice (including one preselected from a record).
            self.ux.u3.kit_pick = Some(KitPick { ids: picked.iter().map(|x| x.to_ascii_lowercase()).collect() });
        }
        if export {
            // Exporting into a folder creates a new kit folder under it, named after the first chosen record's
            // digest.
            let pick = crate::kitx::Pick::parse_ids(&self.typed.pick_from, &self.typed.pick_to, &self.typed.pick_ids).unwrap_or_default();
            // An encrypted package's name takes nothing from what it holds (`kit.zkit`); a plain one is named after the
            // first chosen record's digest.
            let sealed = self.ux.u3.kit_addressed;
            let stem = if sealed { zikaron_glue::addressed::DEFAULT_STEM.to_string() } else { crate::kitx::landing_stem(&rows, &pick) };
            let folder = self.ux.u3.kit_folder && !sealed;
            // One `.zkit` file unless a folder was asked for.
            let kind = if folder {
                crate::home::Kind::Bundle { stem: stem.clone() }
            } else {
                crate::home::Kind::File { stem: stem.clone(), ext: zikaron_glue::container::EXT_KIT.to_string() }
            };
            let chosen = crate::home::choose(&kind, std::path::Path::new(self.typed.kit_out.trim()));
            self.remember_landing(Out::Kit, &chosen);
            let out = chosen.at.display().to_string();
            // Originals still on disk and not removed come first, then dropped items, one per line.
            let mut attach: Vec<String> = match self.ux.u3.kit_orig.as_ref().map(|(_, r)| r) {
                Some(Ok(list)) => list.iter().filter(|o| o.present && !self.ux.u3.kit_orig_off.contains(&o.path)).map(|o| o.path.clone()).collect(),
                _ => Vec::new(),
            };
            for x in self.typed.kit_attach.lines().map(|x| x.trim().to_string()).filter(|x| !x.is_empty()) {
                if !attach.contains(&x) {
                    attach.push(x);
                }
            }
            let a = Action::ExportKit {
                from: self.typed.pick_from.clone(),
                to: self.typed.pick_to.clone(),
                ids: self.typed.pick_ids.clone(),
                attach: attach.join("\n"),
                note: self.typed.kit_note.clone(),
                out,
                form: if folder { zikaron_glue::pack::Form::Folder } else { zikaron_glue::pack::Form::File }.word().to_string(),
                recipient: if sealed { self.typed.kit_to.trim().to_string() } else { String::new() },
                recipient_key: if sealed { self.typed.kit_to_key.trim().to_string() } else { String::new() },
                originals: "none".to_string(),
            };
            // The sheet stays up, its key running, until the export lands (an encrypted one's key shows its check a
            // moment first).
            let started = matches!(self.act(a, now), Applied::Started(crate::task::Kind::Kit));
            kit_sheet_set(ui.ctx(), started.then_some(KitSheet::Export { waiting: true, sealed }));
        }
    }

    /// The kits exported on this machine from this ledger, newest first: one row each (its file, the day it was
    /// made); the row opens the kit's sheet.
    fn kits_index(&mut self, ui: &mut egui::Ui) {
        let rows = self.shell.kits_index.clone();
        // The index covers the whole machine; only this ledger's kits are listed.
        let mine: Vec<crate::kitsindex::Row> = match (rows.as_ref(), self.shell.kits_root.as_deref()) {
            (Some(rows), Some(root)) => rows.iter().filter(|r| r.root.eq_ignore_ascii_case(root)).cloned().collect(),
            _ => Vec::new(),
        };
        let mut open: Option<String> = None;
        card::section(ui, t(Key::KitsLocal), "", |ui| {
            card::form(ui, |ui, f| {
                // None listed: one row saying so (not read yet, or none).
                if mine.is_empty() {
                    let none = if rows.is_none() || self.shell.kits_root.is_none() { t(Key::WbNotRead) } else { t(Key::KitsNone) };
                    let _ = f.line(ui, None, none, "", false, false, c(C::Ink3), |_| ());
                }
                for r in mine.iter().rev() {
                    if f.go_sub(ui, &width::file_name(&r.path), &crate::when::day(r.created)).clicked() {
                        open = Some(r.path.clone());
                    }
                }
            });
        });
        if let Some(p) = open {
            self.ux.u3.drop_armed = None;
            kit_sheet_set(ui.ctx(), Some(KitSheet::Local(p)));
        }
    }

    /// The kit page's own sheets: where the export lands (the entries and the folder, "choose folder" at the
    /// bottom left, "export"; up until the export lands), and a kit exported on this machine (its place and id,
    /// its fetch address with "save", "delete local copy" with a second press to confirm, "close"). Any other
    /// sheet opening closes them. Returns whether "export" was pressed. `ready` is the page's own judgement (entries
    /// picked, no attachment refused), judged again every frame the sheet is up.
    fn kit_sheets(&mut self, ctx: &egui::Context, entries: usize, ready: bool) -> bool {
        let Some(open) = kit_sheet_get(ctx) else { return false };
        if self.any_sheet() {
            kit_sheet_set(ctx, None);
            return false;
        }
        match open {
            KitSheet::Export { waiting, sealed } => {
                let running = self.shell.tasks.in_flight(crate::task::Kind::Kit);
                let phase = self.phase_of(crate::task::Kind::Kit);
                // Landed: the sheet goes; an encrypted export that landed well keeps it up while its key shows the
                // check, then it goes by itself. A refusal closes it at once (the toast says why).
                if waiting && !running && !(sealed && matches!(phase, Phase::Done { .. })) {
                    kit_sheet_set(ctx, None);
                    return false;
                }
                let at = self.typed.kit_out.clone();
                // Encrypted: the recipient's address or public key, one of the two, in its form.
                let addressed = self.ux.u3.kit_addressed;
                let (to, to_key) = (self.typed.kit_to.trim().to_string(), self.typed.kit_to_key.trim().to_string());
                let to_bad = !to.is_empty() && !crate::addressedx::address_ok(&to);
                let key_bad = !to_key.is_empty() && crate::addressedx::key_of(&to_key).is_none();
                let recipient_ok = !addressed || (to.is_empty() != to_key.is_empty() && !to_bad && !key_bad);
                let ready = ready && Self::landing_ok(&at) && recipient_ok;
                let why = if recipient_ok { "" } else if to_bad { t(Key::KitToWhyAddress) } else if key_bad { t(Key::KitToWhyKey) } else { t(Key::KitToWhy) };
                let (mut go, mut close, mut picked) = (false, false, None);
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("kit-export", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, t(Key::U3UseForKit), "");
                        card::form(ui, |ui, f| {
                            let _ = f.info(ui, t(Key::LedgerEntries), card::Show::Text(&fill1(Key::U3EntriesCount, &entries.to_string())));
                            f.folder(ui, t(Key::IdStore), &at, t(Key::PickNone));
                        });
                        if addressed {
                            // The two ways to name the recipient: filling one greys the other.
                            fields::stack(ui, |ui, st| {
                                st.field(ui, t(Key::KitToAddress), None, to_bad.then(|| (t(Key::KitToWhyAddress), true)), |ui, w| {
                                    fields::off(ui, !to_key.is_empty(), |ui| fields::line(ui, &mut me.typed.kit_to, "0x", w, fields::Look { mono: true, bad: to_bad, ..Default::default() }))
                                });
                                st.field(ui, t(Key::KitToKey), None, key_bad.then(|| (t(Key::KitToWhyKey), true)), |ui, w| {
                                    fields::off(ui, !to.is_empty(), |ui| fields::line(ui, &mut me.typed.kit_to_key, "0x", w, fields::Look { mono: true, bad: key_bad, ..Default::default() }))
                                });
                            });
                        }
                        me.stage_line(ui, crate::task::Kind::Kit);
                    },
                    |ui, _me| {
                        go = key::show(ui, key::Key::new(t(Key::IdExportGroup), Role::Primary).enabled(ready).phase(phase).why(why)).clicked();
                        close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                        picked = foot_pick(ui, "kit-out", crate::platform::Pick::Folder);
                    },
                );
                if let Some(p) = picked {
                    self.typed.kit_out = p;
                }
                if close || out.esc {
                    kit_sheet_set(ctx, None);
                    return false;
                }
                go && ready && !running
            }
            KitSheet::Local(path) => {
                let Some(r) = self.shell.kits_index.as_ref().and_then(|rows| rows.iter().find(|r| r.path == path).cloned()) else {
                    kit_sheet_set(ctx, None);
                    return false;
                };
                let at = match self.ux.u3.link_typed.iter().position(|(k, _, _)| *k == r.path) {
                    Some(i) => i,
                    None => {
                        self.ux.u3.link_typed.push((r.path.clone(), String::new(), false));
                        self.ux.u3.link_typed.len() - 1
                    }
                };
                if !self.ux.u3.link_typed[at].2 {
                    self.ux.u3.link_typed[at].1 = r.link.clone().unwrap_or_default();
                }
                let armed = self.ux.u3.drop_armed.as_deref() == Some(r.path.as_str());
                let (mut save_link, mut drop_copy, mut close) = (None, false, false);
                let out = sheet::show(
                    ctx,
                    sheet::Spec::new("kit-local", tk::SHEET_W),
                    self,
                    |ui, me| {
                        sheet::title(ui, &width::file_name(&r.path), "");
                        details_list(ui, "kit-local", &[(t(Key::U3KitOut), Val::mono(r.path.clone())), (t(Key::KitId), Val::mono(r.id.clone()))]);
                        fields::stack(ui, |ui, st| {
                            st.field(ui, t(Key::KitLink), Some(t(Key::U3Optional)), None, |ui, w| {
                                let look = fields::Look { mono: true, ..Default::default() };
                                let (resp, save) = fields::joined(ui, &mut me.ux.u3.link_typed[at].1, t(Key::KitLinkHint), w, look, Some(|ui: &mut egui::Ui| fields::inner_key(ui, t(Key::DoSaveLink), None).clicked()));
                                if resp.changed() {
                                    me.ux.u3.link_typed[at].2 = true;
                                }
                                if save == Some(true) {
                                    save_link = Some(me.ux.u3.link_typed[at].1.clone());
                                }
                            });
                        });
                    },
                    |ui, me| {
                        close = page::Page::new().primary(ui, t(Key::ToastClose)).1.clicked();
                        // Deleting the local copy (the ledger is untouched) takes a second press.
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            if armed {
                                if key::key(ui, t(Key::DropCopyConfirm), Role::Danger, true).clicked() {
                                    drop_copy = true;
                                }
                            } else if key::key(ui, t(Key::DoDropCopy), Role::Secondary, true).clicked() {
                                me.ux.u3.drop_armed = Some(r.path.clone());
                            }
                        });
                    },
                );
                if close || out.esc {
                    self.ux.u3.drop_armed = None;
                    kit_sheet_set(ctx, None);
                }
                let now = self.ux.now;
                if let Some(link) = save_link {
                    // The draft link is dropped only once saved; a refused save keeps it for the user to fix.
                    if let Applied::KitLinked { .. } = self.act(Action::SetKitLink { path: r.path.clone(), link }, now) {
                        self.ux.u3.link_typed.retain(|(k, _, _)| *k != r.path);
                    }
                }
                if drop_copy {
                    self.act(Action::DropKitCopy { path: r.path.clone() }, now);
                    self.ux.u3.drop_armed = None;
                    kit_sheet_set(ctx, None);
                }
                false
            }
        }
    }

    /// The entry picker sheet: the search, then the list in one group — its head counts the chosen and offers
    /// "all" (every row the search keeps) and "clear"; a row per entry (its number and summary, its kind, a
    /// switch; the whole row flips it); cancel and done at the foot. Only "done" writes back.
    pub(super) fn kit_pick_sheet(&mut self, ctx: &egui::Context) {
        if self.ux.u3.kit_pick.is_none() {
            return;
        }
        let rows: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
        let reading = crate::retractx::read(&rows);
        let mut done = false;
        let mut close = false;
        // The search text and date range filter the list and drive "all" (two closures read them, one writes).
        let typed = std::cell::RefCell::new((self.ux.search.get("kit-pick").cloned().unwrap_or_default(), self.ux.range.get("kit-pick").cloned().unwrap_or_default()));
        let today = (self.shell.clock)();
        // The rows the search and date range keep (within them, the switches pick a subset).
        let keep = |row: &crate::ledgerx::Row, query: &str, range: &(String, String)| {
            let (tag, summary, _) = row_face(&rows, &reading, row);
            matches(query, &[&format!("#{}", row.seq), tag, &summary, &row.id]) && crate::when::within(row.anchored_at, &range.0, &range.1)
        };
        let out = sheet::show(
            ctx,
            sheet::Spec::new("kit-pick", tk::SHEET_WIDE),
            self,
            |ui, me| {
                sheet::title(ui, t(Key::KitPickTitle), "");
                {
                    let (query, range) = &mut *typed.borrow_mut();
                    search_row(ui, "kit-pick", query, t(Key::SearchLedger), range, today);
                }
                let (query, range) = typed.borrow().clone();
                let n = me.ux.u3.kit_pick.as_ref().map(|s| s.ids.len()).unwrap_or(0);
                let (mut all, mut none) = (false, false);
                let mut flip: Option<String> = None;
                let picked: Vec<String> = me.ux.u3.kit_pick.as_ref().map(|s| s.ids.clone()).unwrap_or_default();
                zikaron_ui::scroll::nested(ui, egui::ScrollArea::vertical().id_salt("kit-pick-list").max_height(PICK_LIST_H), |ui| {
                    card::form(ui, |ui, f| {
                        let _ = f.line(ui, None, &fill1(Key::KitPickCount, &n.to_string()), "", false, false, c(C::Ink), |ui| {
                            // The two keys share one width: the wider one's, an action row's key width at least.
                            let (k_none, k_all) = (key::Key::new(t(Key::KitPickNone), Role::Secondary).enabled(n > 0), key::Key::new(t(Key::U3FilterAll), Role::Secondary));
                            let w = key::size_of(ui, &k_none).x.max(key::size_of(ui, &k_all).x).max(ROW_KEY_W);
                            none = key::show_w(ui, k_none, w).clicked();
                            all = key::show_w(ui, k_all, w).clicked();
                        });
                        for row in rows.iter().filter(|r| keep(r, &query, &range)) {
                            let (tag, summary, struck) = row_face(&rows, &reading, row);
                            let on = picked.iter().any(|x| x.eq_ignore_ascii_case(&row.id));
                            let mut switched = false;
                            let (resp, _) = f.line(ui, None, "", "", false, false, c(C::Ink), |ui| {
                                switched = toggle::switch(ui, on, true).clicked();
                                mark::tag(ui, tag);
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.spacing_mut().item_spacing.x = PICK_SEQ_GAP;
                                    paint::text(ui, &format!("#{}", row.seq), Type::MonoSmall, c(C::Ink3));
                                    let room = (ui.available_width() - PICK_SEQ_GAP).max(0.0);
                                    let r = paint::line(ui, &summary, Type::Body, if struck { c(C::Ink3) } else { c(C::Ink) }, room);
                                    if struck {
                                        ui.painter().hline(r.rect.x_range(), r.rect.center().y, egui::Stroke::new(1.0_f32, c(C::Ink3)));
                                    }
                                });
                            });
                            let row_click = ui.interact(resp.rect, egui::Id::new(("kit-pick-row", row.seq)), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked();
                            if switched || row_click {
                                flip = Some(row.id.clone());
                            }
                        }
                    });
                });
                if all {
                    me.ux.u3.kit_pick = Some(KitPick { ids: rows.iter().filter(|r| keep(r, &query, &range)).map(|r| r.id.to_ascii_lowercase()).collect() });
                }
                if none {
                    me.ux.u3.kit_pick = Some(KitPick { ids: Vec::new() });
                }
                if let (Some(id), Some(st)) = (flip, me.ux.u3.kit_pick.as_mut()) {
                    if st.ids.iter().any(|x| x.eq_ignore_ascii_case(&id)) {
                        st.ids.retain(|x| !x.eq_ignore_ascii_case(&id));
                    } else {
                        st.ids.push(id.to_ascii_lowercase());
                    }
                }
            },
            |ui, _me| {
                if page::Page::new().primary(ui, t(Key::KitPickDone)).1.clicked() {
                    done = true;
                }
                if key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked() {
                    close = true;
                }
            },
        );
        let (query, range) = typed.into_inner();
        self.ux.search.insert("kit-pick", query);
        self.ux.range.insert("kit-pick", range);
        if out.esc {
            close = true;
        }
        if done {
            // Write back the named entries and clear the range cells (the two ways do not stack).
            if let Some(st) = self.ux.u3.kit_pick.take() {
                self.typed.pick_ids = st.ids.join(",");
                self.typed.pick_from.clear();
                self.typed.pick_to.clear();
            }
        } else if close {
            self.ux.u3.kit_pick = None;
        }
    }
}

/// The least width of the "remove" key at a row's end (an action row's key width).
const ROW_KEY_W: f32 = 92.0;

/// The picker's list: at most this high before it scrolls; the gap after an entry's number.
const PICK_LIST_H: f32 = 300.0;
const PICK_SEQ_GAP: f32 = 8.0;

/// Which of the kit page's own sheets is up (kept in the window's memory; the page draws them).
#[derive(Clone, Debug, PartialEq, Eq)]
enum KitSheet {
    /// Where the export lands; `waiting` once the export started (the sheet closes when it lands; `sealed`, an
    /// encrypted export, once its key's check has shown).
    Export { waiting: bool, sealed: bool },
    /// A kit exported on this machine, by its path.
    Local(String),
}

fn kit_sheet_id() -> egui::Id {
    egui::Id::new("zikaron-kit-sheet")
}

/// The sheet up, as long as the page drew it in the last pass too (a sheet left by going elsewhere is gone when
/// the page comes back); kept for this pass.
fn kit_sheet_get(ctx: &egui::Context) -> Option<KitSheet> {
    let pass = zikaron_ui::motion::pass(ctx);
    let kept = ctx.data(|d| d.get_temp::<Option<(KitSheet, u64)>>(kit_sheet_id())).flatten();
    let now = kept.filter(|(_, at)| at + 1 >= pass).map(|(s, _)| s);
    kit_sheet_set(ctx, now.clone());
    now
}

fn kit_sheet_set(ctx: &egui::Context, s: Option<KitSheet>) {
    let pass = zikaron_ui::motion::pass(ctx);
    ctx.data_mut(|d| d.insert_temp(kit_sheet_id(), s.map(|s| (s, pass))));
}

/// Put the export sheet up or take any of the kit page's sheets down (the still-frame readings stage it).
pub(super) fn kit_export_up(ctx: &egui::Context, up: bool) {
    kit_sheet_set(ctx, up.then_some(KitSheet::Export { waiting: false, sealed: false }));
}
