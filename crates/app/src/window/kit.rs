use super::*;

impl Win {
    /// Export a record kit: entries, the chosen records' originals, attachments, a note and destination;
    /// "check and export" runs in the background. Beside it: how the recipient checks, the depth reading, and
    /// the kits exported on this machine.
    pub(super) fn kit_page(&mut self, ui: &mut egui::Ui, now: f64) {
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
        let mut export = false;
        let mut open_picker = false;
        let mut anchor_now: Vec<(String, bool)> = Vec::new();
        let reading_work = in_pick.iter().find(|r| r.kind == zikaron::tokens::EntryType::History).and_then(|r| r.work.clone().map(|w| (w, r.seq, human_summary(r))));
        let mut read_depth: Option<String> = None;
        let mut blocked = false;
        card::two_cols(ui, tk::SIDE_W, tk::MAIN_MIN_W, |ui, side| match side {
            card::Side::Main => {
                stagger(ui, 0, |ui| {
                    card::card(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = tk::S4;
                        // Which entries: a summary and "choose…", which opens the whole ledger with a switch per
                        // entry.
                        let range = match (self.typed.pick_ids.trim(), self.typed.pick_from.trim(), self.typed.pick_to.trim()) {
                            ("", "", "") => fill1(Key::KitEntriesAll, &in_pick.len().to_string()),
                            ("", a, b) => fill3(Key::KitEntriesRange, a, b, &in_pick.len().to_string()),
                            _ => fill1(Key::KitPickCount, &in_pick.len().to_string()),
                        };
                        width::then(
                            ui,
                            |ui| open_picker = key::key(ui, t(Key::KitPickOpen), Role::Secondary, true).clicked(),
                            |ui, room| {
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    paint::line(ui, t(Key::KitEntries), Type::Note, c(C::Ink2), room);
                                    paint::line(ui, &range, Type::Body, c(C::Ink), room);
                                });
                            },
                        );
                        if let Some(e) = pick_err.as_ref() {
                            states::hint_ex(ui, e, true);
                        } else if in_pick.is_empty() {
                            hint(ui, t(Key::U3KitNothingPicked));
                        }
                        // Chosen entries not yet on chain are shown in red with "put on chain now"; what the kit
                        // holds follows under it.
                        let (unanchored, sendable) = crate::kitx::unanchored(&in_pick.iter().collect::<Vec<_>>());
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 0.0;
                            if !unanchored.is_empty() {
                                width::then(
                                    ui,
                                    |ui| {
                                        if page::Guide::key(ui, t(Key::U3SendNow), !sendable.is_empty()).clicked() {
                                            anchor_now = sendable.clone();
                                        }
                                    },
                                    |ui, room| {
                                        paint::line(ui, &fill1(Key::KitUnanchored, &unanchored.len().to_string()), Type::Small, c(C::BadInk), room);
                                    },
                                );
                            }
                            hint(ui, t(Key::KitContainsWhole));
                            ui.add_space(4.0);
                            paint::rule(ui, 0.0);
                        });
                        // The chosen records' originals, from the local index, one row each with "remove". A file
                        // no longer at its signing place, or changed since, is flagged in red and left out.
                        field(ui, t(Key::KitOriginalsLabel), None, |ui| match self.ux.u3.kit_orig.as_ref().map(|(_, r)| r.clone()) {
                            Some(Ok(list)) if !list.is_empty() => {
                                let mut removed = 0usize;
                                let admit = self.ux.u3.kit_admit.as_ref().and_then(|(_, a)| a.clone());
                                for one in &list {
                                    if self.ux.u3.kit_orig_off.contains(&one.path) {
                                        removed += 1;
                                        continue;
                                    }
                                    let changed = matches!((self.shell.vetted.get(&one.path), admit.as_ref()), (Some(Ok(d)), Some(a)) if !a.admits_hex(d));
                                    let (sub, bad) = if !one.present {
                                        (format!("#{} \u{b7} {}", one.seq, t(Key::KitOriginalGone)), true)
                                    } else if changed {
                                        (format!("#{} \u{b7} {}", one.seq, t(Key::KitNotAnOriginalRow)), true)
                                    } else {
                                        (format!("#{}", one.seq), false)
                                    };
                                    if drop::file_row(ui, &format!("{}#{}", one.seq, one.path), &one.name, &sub, bad, t(Key::KitRemove)) {
                                        self.ux.u3.kit_orig_off.insert(one.path.clone());
                                    }
                                }
                                if removed > 0 {
                                    // The row is one key tall so the words sit centered beside the key.
                                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), tk::KEY_H), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                        ui.spacing_mut().item_spacing.x = tk::S3;
                                        paint::text(ui, &fill1(Key::KitOriginalsRemoved, &removed.to_string()), Type::Small, c(C::Ink3));
                                        if key::key(ui, t(Key::KitOriginalsPutBack), Role::Secondary, true).clicked() {
                                            self.ux.u3.kit_orig_off.clear();
                                        }
                                    });
                                }
                            }
                            Some(Err(e)) => hint(ui, &e),
                            _ => hint(ui, t(Key::KitOriginalsNone)),
                        });
                        // Attachments: any number; each is read and digested in the background.
                        field(ui, t(Key::U3AttachLabel), None, |ui| {
                            let lines_in: Vec<String> = self.typed.kit_attach.lines().map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
                            let listed: Vec<String> = match self.ux.u3.kit_orig.as_ref().map(|(_, r)| r) {
                                Some(Ok(list)) => list.iter().filter(|o| o.present).map(|o| o.path.clone()).collect(),
                                _ => Vec::new(),
                            };
                            let pending: Vec<String> = lines_in.iter().chain(listed.iter()).filter(|x| !self.shell.vetted.contains_key(*x)).cloned().collect();
                            if !pending.is_empty() && !self.shell.tasks.in_flight(crate::task::Kind::Vet) {
                                self.auto(Action::VetAttachments { paths: pending }, now);
                            }
                            if !lines_in.is_empty() {
                                if self.ux.u3.kit_names.as_ref().map(|(k, _)| k != &self.typed.kit_attach).unwrap_or(true) {
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
                                let mut gone: Option<String> = None;
                                let mut renamed = false;
                                for (n, (path, named)) in self.ux.u3.kit_names.as_ref().map(|(_, r)| r.clone()).unwrap_or_default().into_iter().enumerate() {
                                    let (sub, bad, blocks) = match (self.shell.vetted.get(&path), &named) {
                                        (None, _) => (t(Key::KitVetting).to_string(), false, false),
                                        (Some(Err(f)), _) => (f.human().to_string(), true, false),
                                        (Some(Ok(d)), _) if admit.as_ref().map(|a| !a.admits_hex(d)).unwrap_or(false) => (t(Key::KitNotAnOriginalRow).to_string(), true, false),
                                        (Some(Ok(_)), Ok(Some((n, changed)))) => {
                                            renamed |= *changed;
                                            (fill1(Key::KitExportAs, n), false, false)
                                        }
                                        (Some(Ok(_)), Ok(None)) => (t(Key::KitNameRefused).to_string(), true, true),
                                        (Some(Ok(_)), Err(e)) => (e.clone(), true, false),
                                    };
                                    blocked |= blocks;
                                    if drop::file_row(ui, &format!("{n}#{path}"), &width::file_name(&path), &sub, bad, t(Key::KitRemove)) {
                                        gone = Some(path.clone());
                                    }
                                }
                                if renamed {
                                    hint(ui, t(Key::KitNamesRecorded));
                                }
                                if let Some(g) = gone {
                                    self.shell.vetted.remove(&g);
                                    self.typed.kit_attach = self.typed.kit_attach.lines().filter(|x| x.trim() != g).collect::<Vec<_>>().join("\n");
                                }
                            }
                            let d = drop::zone(ui, "kit-attach", None, &[t(Key::U3AttachDrop), t(Key::DropClickAny)], None, 72.0, drop::Shape::Column, true);
                            let mut got = d.dropped.clone();
                            got.extend(path_answer(ui.ctx(), egui::Id::new("zikaron-path-kit-attach"), got.is_empty() && d.clicked, crate::platform::Pick::FileOrFolder));
                            for p in got {
                                // Dropping the same path again re-checks it (the file may have changed).
                                self.shell.vetted.remove(p.trim());
                                if !self.typed.kit_attach.is_empty() && !self.typed.kit_attach.ends_with('\n') {
                                    self.typed.kit_attach.push('\n');
                                }
                                self.typed.kit_attach.push_str(&p);
                            }
                            if !lines_in.is_empty() && key::key(ui, t(Key::U3AttachClear), Role::Secondary, true).clicked() {
                                for x in &lines_in {
                                    self.shell.vetted.remove(x);
                                }
                                self.typed.kit_attach.clear();
                            }
                        });
                        field(ui, t(Key::KitNoteLabel), None, |ui| input::area_words(ui, &mut self.typed.kit_note, 2, ""));
                        Self::place_row(ui, Key::IdStore, &mut self.typed.kit_out);
                        paint::rule(ui, 0.0);
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = tk::S2;
                            let enabled = !in_pick.is_empty() && !blocked && Self::landing_ok(&self.typed.kit_out);
                            // The page's only primary key.
                            let phase = self.phase_of(crate::task::Kind::Kit);
                            let _page = page::Page::new();
                            if key::show(ui, key::Key::new(t(Key::U3KitKey), Role::Primary).enabled(enabled).phase(phase)).clicked() {
                                export = true;
                            }
                            self.stage_line(ui, crate::task::Kind::Kit);
                            if let Some(Done::Kit { entries, files, left_out, unreadable, .. }) = self.shell.kit.clone() {
                                motion::swap(ui, egui::Id::new("kit-made"), (entries + files) as u64, |ui| {
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
                    });
                });
            }
            card::Side::Side => {
                stagger(ui, 1, |ui| {
                    card::flat(ui, |ui| {
                        card::flat_title(ui, t(Key::U3KitWhatTheyCanDo));
                        hint(ui, t(Key::U3KitWhatTheyCanDoSay));
                    });
                });
                // The reading against its baseline (kit law §9): shown only here; the export carries none of it.
                if let Some((work, seq, name)) = reading_work.clone() {
                    stagger(ui, 2, |ui| {
                        card::flat(ui, |ui| {
                            card::flat_title(ui, t(Key::KitReadingTitle));
                            hint(ui, &fill1(Key::KitReadingOf, &format!("#{seq} \u{b7} {name}")));
                            match self.shell.depth.clone().filter(|(w, _)| w.eq_ignore_ascii_case(&work)).map(|(_, v)| crate::depthx::three(v)) {
                                Some(x) => {
                                    let first = x.earliest.map(crate::when::day).unwrap_or_else(|| t(Key::None_).to_string());
                                    let span = format!("{} / {}", x.anchored, x.span);
                                    stat_cells(ui, &[(t(Key::U3FirstAnchored), &first), (t(Key::U3Deepest), &x.deepest.to_string()), (t(Key::U3Continuity), &span)]);
                                    hint(ui, &fill1(Key::KitReadingBasis, &if x.label.is_empty() { t(Key::None_).to_string() } else { label_human(&x.label) }));
                                }
                                None => {
                                    width::then(ui, |ui| {
                                        if self.long_key(ui, t(Key::DoReadDepth), Role::Secondary, true, crate::task::Kind::Depth) {
                                            read_depth = Some(work.clone());
                                        }
                                    }, |ui, room| {
                                        paint::line(ui, t(Key::U3DepthNotRead), Type::Small, c(C::Ink3), room);
                                    });
                                    self.stage_line(ui, crate::task::Kind::Depth);
                                }
                            }
                        });
                    });
                }
                stagger(ui, 3, |ui| self.kits_index(ui, now));
            }
        });
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
            let stem = crate::kitx::landing_stem(&rows, &pick);
            let chosen = crate::home::choose(&crate::home::Kind::Bundle { stem: stem.clone() }, std::path::Path::new(self.typed.kit_out.trim()));
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
            };
            self.act(a, now);
        }
    }

    /// The kits exported on this machine from this ledger: date and entry count, with place, id and fetch
    /// address in details. "Delete" removes the local copy on a second press; the ledger is untouched.
    fn kits_index(&mut self, ui: &mut egui::Ui, now: f64) {
        let rows = self.shell.kits_index.clone();
        let mut act: Option<Action> = None;
        card::flat(ui, |ui| {
            card::flat_title(ui, t(Key::KitsLocal));
            let Some(rows) = rows else {
                hint(ui, t(Key::WbNotRead));
                return;
            };
            // The index covers the whole machine; only this ledger's kits are listed.
            let mine: Vec<&crate::kitsindex::Row> = rows.iter().filter(|r| self.shell.kits_root.as_deref().map(|x| r.root.eq_ignore_ascii_case(x)).unwrap_or(false)).collect();
            if self.shell.kits_root.is_none() {
                hint(ui, t(Key::WbNotRead));
            } else if mine.is_empty() {
                hint(ui, t(Key::KitsNone));
            }
            for r in mine.into_iter().rev() {
                paint::rule(ui, 6.0);
                paint::text(ui, &crate::when::day(r.created), Type::Note, c(C::Ink2));
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
                fold::fold(ui, &format!("kit-row-{}", r.id), t(Key::SetEvidence), |ui| {
                    kv::kv(ui, &[(t(Key::U3KitOut), Val::mono(r.path.clone())), (t(Key::KitId), Val::mono(r.id.clone()))]);
                    field(ui, t(Key::KitLink), None, |ui| {
                        let (resp, save) = width::line_then(ui, &mut self.ux.u3.link_typed[at].1, t(Key::KitLinkHint), true, |ui| key::key(ui, t(Key::DoSaveLink), Role::Secondary, true).clicked());
                        if resp.changed() {
                            self.ux.u3.link_typed[at].2 = true;
                        }
                        if save {
                            act = Some(Action::SetKitLink { path: r.path.clone(), link: self.ux.u3.link_typed[at].1.clone() });
                        }
                    });
                    hint(ui, &r.link.clone().unwrap_or_else(|| t(Key::KitNoLink).to_string()));
                });
                let armed = self.ux.u3.drop_armed.as_deref() == Some(r.path.as_str());
                keys_row(ui, |ui| {
                    if armed {
                        if page::Guide::key(ui, t(Key::DropCopyConfirm), true).clicked() {
                            act = Some(Action::DropKitCopy { path: r.path.clone() });
                        }
                        if key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked() {
                            self.ux.u3.drop_armed = None;
                        }
                    } else if page::Guide::key(ui, t(Key::DoDropCopy), true).clicked() {
                        self.ux.u3.drop_armed = Some(r.path.clone());
                    }
                });
            }
        });
        if let Some(a) = act {
            // The draft link is dropped only once saved; a refused save keeps it for the user to fix.
            let path = if let Action::SetKitLink { path, .. } = &a { Some(path.clone()) } else { None };
            if let (Applied::KitLinked { .. }, Some(p)) = (self.act(a, now), path) {
                self.ux.u3.link_typed.retain(|(k, _, _)| *k != p);
            }
        }
    }

    /// The entry picker sheet: the whole ledger with a switch per entry, any subset; "all", "clear", cancel and
    /// done. Only "done" writes back.
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
                let Some(st) = me.ux.u3.kit_pick.as_mut() else { return };
                egui::ScrollArea::vertical().id_salt("kit-pick-list").max_height(260.0).show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    for row in rows.iter().filter(|r| keep(r, &query, &range)) {
                        let (tag, summary, struck) = row_face(&rows, &reading, row);
                        let on = st.ids.iter().any(|x| x.eq_ignore_ascii_case(&row.id));
                        let mut flipped = false;
                        let resp = table::pick_row(ui, egui::Id::new(("kit-pick", row.seq)), on, true, 40.0, |ui| {
                            flipped = toggle::switch(ui, on, true).clicked();
                            ui.allocate_ui(egui::vec2(40.0, 20.0), |ui| {
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| paint::text(ui, &format!("#{}", row.seq), Type::MonoSmall, c(C::Ink3)));
                            });
                            mark::tag_in(ui, tag, tk::PICK_TYPE_W);
                            let room = (ui.available_width() - 28.0).max(0.0);
                            let r = paint::line(ui, &summary, if struck { Type::Small } else { Type::Body }, if struck { c(C::Ink3) } else { c(C::Ink) }, room);
                            if struck {
                                ui.painter().hline(r.rect.x_range(), r.rect.center().y, egui::Stroke::new(1.0_f32, c(C::Ink3)));
                            }
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| mark::mark(ui, lamp_mark(row.lamp)));
                        });
                        if resp.clicked() || flipped {
                            if on {
                                st.ids.retain(|x| !x.eq_ignore_ascii_case(&row.id));
                            } else {
                                st.ids.push(row.id.to_ascii_lowercase());
                            }
                        }
                    }
                });
                // Revocations of chosen grants are included too (the selector closes over them); this note comes
                // first.
                hint(ui, t(Key::KitPickRevocationNote));
            },
            |ui, me| {
                if page::Page::new().primary(ui, t(Key::KitPickDone)).1.clicked() {
                    done = true;
                }
                if key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked() {
                    close = true;
                }
                let n = me.ux.u3.kit_pick.as_ref().map(|s| s.ids.len()).unwrap_or(0);
                // "All" is every row the search and date range keep.
                if key::key(ui, t(Key::U3FilterAll), Role::Secondary, true).clicked() {
                    let (query, range) = typed.borrow().clone();
                    me.ux.u3.kit_pick = Some(KitPick { ids: rows.iter().filter(|r| keep(r, &query, &range)).map(|r| r.id.to_ascii_lowercase()).collect() });
                }
                if key::key(ui, t(Key::KitPickNone), Role::Secondary, n > 0).clicked() {
                    me.ux.u3.kit_pick = Some(KitPick { ids: Vec::new() });
                }
                sheet::foot_note(ui, &fill1(Key::KitPickCount, &n.to_string()));
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

/// Frameless stat cells in a row, each a quiet title over a figure (the depth reading).
pub(super) fn stat_cells(ui: &mut egui::Ui, cells: &[(&str, &str)]) {
    let w = ui.available_width();
    let gap = 10.0;
    let n = cells.len().max(1);
    let cw = (w - gap * (n as f32 - 1.0)) / n as f32;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 18.0 + 2.0 + 24.0), egui::Sense::hover());
    for (i, (k, v)) in cells.iter().enumerate() {
        let x = rect.left() + (cw + gap) * i as f32;
        let p = ui.painter();
        paint::at(p, ui, egui::pos2(x, rect.top()), egui::Align2::LEFT_TOP, k, Type::Small, c(C::Ink3), cw);
        paint::at(p, ui, egui::pos2(x, rect.top() + 20.0), egui::Align2::LEFT_TOP, v, Type::Stat, c(C::Ink), cw);
    }
}
