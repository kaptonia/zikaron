use super::*;

impl Win {
    /// The grants list: search, then one row per grant (record, validity, number, state, on chain).
    pub(super) fn grants_list(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
        let all = self.shell.grants.clone();
        let chain_now = self.chain_now();
        let mut query = self.ux.search.get("grants").cloned().unwrap_or_default();
        let mut range = self.ux.range.get("grants").cloned().unwrap_or_default();
        let today = (self.shell.clock)();
        stagger(ui, 0, |ui| search_row(ui, "grants", &mut query, t(Key::SearchGrants), &mut range, today));
        let lamps: Vec<(String, crate::ledgerx::Lamp)> = self.shell.rows.as_ref().map(|(rs, _)| rs.iter().map(|r| (r.id.clone(), r.lamp)).collect()).unwrap_or_default();
        // A grant's anchor time is that of its entry row in this ledger (a grant is an entry).
        let anchored: Vec<(String, Option<u64>)> = self.shell.rows.as_ref().map(|(rs, _)| rs.iter().map(|r| (r.id.clone(), r.anchored_at)).collect()).unwrap_or_default();
        let anchored_at = |id: &str| anchored.iter().find(|(x, _)| x.eq_ignore_ascii_case(id)).and_then(|(_, at)| *at);
        let shown: Vec<crate::grantx::Row> = all
            .clone()
            .unwrap_or_default()
            .into_iter()
            .rev()
            .filter(|row| {
                let badge = row.badge(chain_now);
                matches(&query, &[&row.grantee, &self.work_label(&row.work), &row.work, t(badge_key(badge)), &format!("#{}", row.seq)])
            })
            .filter(|row| crate::when::within(anchored_at(&row.id), &range.0, &range.1))
            .collect();
        let mut open: Option<String> = None;
        stagger(ui, 1, |ui| {
            let cols = [
                table::col(t(Key::U3Work), table::Col::Fr(1.0)),
                table::col(t(Key::U3Window), table::Col::Fr(1.1)),
                table::col(t(Key::GrantNo), table::Col::Px(90.0)),
                table::col(t(Key::U3State), table::Col::Px(84.0)),
                table::MARK,
                table::CHEV,
            ];
            let rows: Vec<table::Row> = shown
                .iter()
                .map(|g| {
                    let badge = g.badge(chain_now);
                    let lamp = lamps.iter().find(|(id, _)| *id == g.id).map(|(_, l)| *l).unwrap_or(crate::ledgerx::Lamp::Queued);
                    table::Row {
                        cells: vec![
                            table::Cell::Text(self.work_label(&g.work)),
                            table::Cell::Mono(window_long(g.window)),
                            table::Cell::Mono(format!("#{}", g.seq)),
                            table::Cell::Pill(t(badge_key(badge)).to_string(), badge_tone(badge)),
                            table::Cell::Mark(lamp_mark(lamp)),
                            table::Cell::Chev,
                        ],
                        click: true,
                        gone: false,
                        on: false,
                    }
                })
                .collect();
            let empty = match all.as_ref() {
                None => t(Key::WbNotRead),
                Some(g) if g.is_empty() => t(Key::U3NoGrants),
                Some(_) => t(Key::SearchNone),
            };
            if let Some(i) = table::table(ui, "grants", &cols, true, &rows, empty).clicked {
                open = shown.get(i).map(|g| g.id.clone());
            }
        });
        self.ux.search.insert("grants", query);
        self.ux.range.insert("grants", range);
        if let Some(id) = open {
            self.push(Route::Grant(id), now);
        }
    }

    /// A grant's detail page (also used when the grant is opened from the ledger).
    pub(super) fn grant_detail(&mut self, ui: &mut egui::Ui, row: &crate::ledgerx::Row, now: f64) {
        self.ensure_grants(now);
        let g = self.shell.grants.as_ref().and_then(|g| g.iter().find(|x| x.id == row.id).cloned());
        let badge = g.as_ref().map(|x| x.badge(self.chain_now()));
        let at = self.shell.remembered.as_ref().map(|r| r.at);
        let opened = self.opened.clone().filter(|d| d.id == row.id);
        let record = g.as_ref().map(|x| self.work_label(&x.work)).unwrap_or_else(|| t(Key::WbNotRead).to_string());
        let window = g.as_ref().map(|x| window_long(x.window)).unwrap_or_default();
        let mut copy = false;
        let mut export = false;
        let mut story = false;
        let mut confirm: Option<U3Confirm> = None;
        stagger(ui, 0, |ui| {
            card::hero(ui, &format!("#{} \u{b7} {}", row.seq, t(Key::KindGrant)), &format!("{record} \u{b7} {window}"), false, |ui| {
                if let Some(b) = badge {
                    mark::pill(ui, t(badge_key(b)), badge_tone(b));
                }
            });
        });
        stagger(ui, 1, |ui| {
            kv_section(
                ui,
                t(Key::BasicInfo),
                &[
                    (t(Key::U3Work), Val::text(record.clone())),
                    (t(Key::U3Window), Val::text(window.clone())),
                    (t(Key::U3State), Val::text(badge.map(|b| t(badge_key(b)).to_string()).unwrap_or_else(|| t(Key::WbNotRead).to_string()))),
                    (t(Key::NavAnchoring), Val::Mark(lamp_mark(row.lamp), lamp_label(row.lamp, true, at))),
                    (t(Key::U4RecordFirstAt), Val::mono(first_anchor_say(row.anchored_at))),
                    // Exclusivity is the record made once at signing; read-only here.
                    (t(Key::ExclusiveMark), Val::text(g.as_ref().map(|x| t(exclusive_key(x.exclusive_from)).to_string()).unwrap_or_else(|| t(Key::WbNotRead).to_string()))),
                    (t(Key::U3TermsDoc), Val::text(g.as_ref().and_then(|x| x.doc_name.clone()).unwrap_or_else(|| t(Key::TermsDocNone).to_string()))),
                ],
            );
        });
        if self.typed.gf_out.trim().is_empty() {
            if let Some(home) = self.shell.home.as_ref() {
                self.typed.gf_out = home.dir(crate::home::Slot::Kits).display().to_string();
            }
        }
        let inq = self.shell.queue.items.iter().filter(|q| !q.step.in_flight()).position(|q| q.id == row.id);
        stagger(ui, 2, |ui| {
            card::card(ui, |ui| {
                let ready = opened.is_some();
                keys_row(ui, |ui| {
                    copy = page::Page::new().primary_with(ui, t(Key::V2CopyCode), ready).1.clicked();
                    // Exporting reads the chain first (the exit gate, in the background): the key shows it running.
                    export = self.long_key(ui, t(Key::V2ExportGrantFile), Role::Secondary, ready && Self::landing_ok(&self.typed.gf_out), crate::task::Kind::Gate);
                });
                self.stage_line(ui, crate::task::Kind::Gate);
                ui.add_space(2.0);
                Self::place_row(ui, Key::IdStore, &mut self.typed.gf_out);
                if let Some(i) = inq {
                    paint::rule(ui, tk::S2);
                    if page::Guide::key(ui, t(Key::U3SendNow), true).clicked() {
                        confirm = Some(U3Confirm::Send { count: i + 1 });
                    }
                }
            });
        });
        stagger(ui, 3, |ui| {
            card::card(ui, |ui| {
                fold::fold(ui, "grant-details", t(Key::SetEvidence), |ui| {
                    let mut kv_rows = vec![
                        (t(Key::U3ToWhom), Val::mono(g.as_ref().map(|x| x.grantee.clone()).unwrap_or_default())),
                        (t(Key::U3TermsHash), Val::mono(g.as_ref().map(|x| x.terms.clone()).unwrap_or_default())),
                        (t(Key::DetailPick), Val::mono(row.id.clone())),
                        (t(Key::U3RawType), Val::mono(row.kind.as_str().to_string())),
                        (t(Key::U3RawSeq), Val::mono(row.seq.to_string())),
                        (t(Key::EntryAuthor), Val::mono(row.author.clone())),
                        (t(Key::EntryPrev), Val::mono(row.prev.clone().unwrap_or_else(|| t(Key::None_).to_string()))),
                        (t(Key::CanonBytes), Val::mono(row.bytes.to_string())),
                    ];
                    if let Some((chain, tx)) = row.tx.clone() {
                        kv_rows.push((t(Key::AnchorTx), Val::mono(format!("{chain} \u{b7} {tx}"))));
                    }
                    kv::kv(ui, &kv_rows);
                    ui.add_space(4.0);
                    story = key::key(ui, t(Key::DoReadStory), Role::Secondary, true).clicked();
                    if let Some((_, _, revs)) = self.shell.story.clone().filter(|(a, _, _)| a.eq_ignore_ascii_case(&row.id)) {
                        let rows_all: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
                        let lines: Vec<(Mark, String, Option<String>)> = if revs.is_empty() {
                            vec![(Mark::Ok, t(Key::StoryNone).to_string(), None)]
                        } else {
                            revs.iter()
                                .map(|(id, _)| {
                                    let r = rows_all.iter().find(|x| x.id.eq_ignore_ascii_case(id));
                                    (Mark::Bad, fill1(Key::StoryRevokedBy, &r.map(|x| x.seq.to_string()).unwrap_or_default()), r.map(|x| first_anchor_say(x.anchored_at)))
                                })
                                .collect()
                        };
                        motion::swap(ui, egui::Id::new("grant-story"), revs.len() as u64, |ui| states::checks(ui, &lines, false));
                    }
                });
            });
        });
        if g.as_ref().map(|x| !x.revoked).unwrap_or(false) {
            stagger(ui, 4, |ui| {
                if page::Guide::last(ui, t(Key::V2RevokeGrantDots), true).clicked() {
                    confirm = Some(U3Confirm::Revoke { grant: row.id.clone() });
                }
            });
        }
        // Copy the whole code (every hop to the root) in the same encoding as the badge and the grant file; the
        // action layer reports any refusal.
        if copy {
            if let Applied::GrantCode { text } = self.act(Action::CopyGrantCode { grant: row.id.clone() }, now) {
                ui.ctx().copy_text(text);
                self.toasts.say(t(Key::U3CopiedGrantText), Tone::Note, now);
            }
        }
        if export {
            let a = Action::ExportGrantFile { id: row.id.clone(), to: self.typed.gf_out.clone() };
            self.act(a, now);
        }
        if story {
            self.act(Action::ReadStory { grant: row.id.clone() }, now);
        }
        if let Some(cf) = confirm {
            if let U3Confirm::Revoke { .. } = &cf {
                self.ux.u3.revoking = Some(format!("#{}", row.seq));
            }
            self.u3_open_confirm(cf);
        }
    }

    /// Clear the grant form for a new grant.
    pub(super) fn grant_form_fresh(&mut self) {
        self.typed.g_grantee.clear();
        self.typed.g_work.clear();
        self.typed.g_history.clear();
        self.typed.g_terms.clear();
        self.typed.g_upstream.clear();
        self.typed.g_scope.clear();
        self.typed.g_exclusive = false;
        self.ux.u3.terms = None;
        self.ux.u3.grant_days = 30;
        self.ux.u3.grant_days_set = true;
    }

    /// The new-grant page.
    pub(super) fn grant_form_page(&mut self, ui: &mut egui::Ui, now: f64) {
        self.grant_form(ui, None, now);
    }

    /// The grant form, shared by a new grant and a relicense (`upstream` names the upstream grant's record).
    /// Left: the form fields and the key to the confirmation sheet. Right: the checks before issuing.
    pub(super) fn grant_form(&mut self, ui: &mut egui::Ui, upstream: Option<String>, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
        if !self.ux.u3.grant_days_set {
            self.ux.u3.grant_days = 30;
            self.ux.u3.grant_days_set = true;
        }
        // The double-sale check runs in the background as the user types, and only reruns when the record, the
        // validity, the register or the exclusive list changes.
        let key = format!("{}|{}|{}|{}|{}", self.typed.g_work.trim(), self.typed.g_from.trim(), self.typed.g_to.trim(), self.shell.grants_gen, self.shell.settings.exclusive.join(","));
        let whole = self.typed.g_from.trim().is_empty() == self.typed.g_to.trim().is_empty();
        if key != self.clash_key && self.shell.grants.is_some() {
            self.clash_key = key;
            if !self.typed.g_work.trim().is_empty() && whole {
                self.auto(Action::CheckClash { work: self.typed.g_work.clone(), from: self.typed.g_from.clone(), to: self.typed.g_to.clone() }, now);
            } else {
                self.shell.clash.clear();
            }
        }
        let chain_now = self.chain_now();
        // Presets fill both cells from chain time (deadlines use only chain time).
        if self.ux.u3.grant_days > 0 {
            match chain_now {
                Some(n) => {
                    let (f, to) = (n.to_string(), (n + self.ux.u3.grant_days as u64 * 86_400).to_string());
                    if self.typed.g_from != f || self.typed.g_to != to {
                        self.typed.g_from = f;
                        self.typed.g_to = to;
                    }
                }
                None => {
                    self.typed.g_from.clear();
                    self.typed.g_to.clear();
                }
            }
        }
        // Checksum (mixed-case) addresses copied from wallets are refused by the ledger law: say so at once and
        // keep the key disabled.
        let mixed_case = [&self.typed.g_grantee, &self.typed.g_terms, &self.typed.g_upstream].iter().any(|x| x.trim().strip_prefix("0x").map(|h| h.bytes().any(|b| b.is_ascii_uppercase())).unwrap_or(false));
        let ready = crate::grantx::draft_ready(&self.typed.g_grantee, &self.typed.g_work, &self.typed.g_terms, &self.typed.g_upstream, self.ux.u3.grant_days, chain_now);
        let mut confirm = false;
        let mut to_queue = false;
        card::two_cols(ui, tk::SIDE_W, tk::MAIN_MIN_W, |ui, side| match side {
            card::Side::Main => {
                stagger(ui, 0, |ui| {
                    card::card(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = tk::S4;
                        // Grantee: the field with "recent addresses" beside it.
                        field(ui, t(Key::U3ToWhom), None, |ui| {
                            let recent = self.recent_addresses();
                            let heads: Vec<String> = recent.iter().map(|a| head_tail(a)).collect();
                            let items: Vec<menu::Item> = heads.iter().map(|h| menu::Item::Row(menu::Row { lead: t(Key::Address), label: h, mono: true, ..Default::default() })).collect();
                            let spec = pick::Spec { hint: t(Key::SearchAddress), empty: t(Key::SearchNone), w: PICK_W, dates: None };
                            let keep = |i: usize, q: &str, _: &str, _: &str| matches(q, &[&recent[i]]);
                            let mut picked = None;
                            width::then(
                                ui,
                                |ui| picked = pick::key(ui, "grant-recent", t(Key::U3RecentDots), !recent.is_empty(), false, &spec, &items, &keep),
                                |ui, room| input::field(ui, &mut self.typed.g_grantee, t(Key::U3ToWhomHint), room, input::Look { mono: true, ..Default::default() }),
                            );
                            if let Some(i) = picked {
                                self.typed.g_grantee = recent[i].clone();
                            }
                            if mixed_case {
                                states::note_box(ui, t(Key::U3AddrMixedCase));
                            }
                        });
                        // The record: fixed by the upstream grant for a relicense; otherwise chosen from anchored
                        // records (the menu says why the others cannot be chosen).
                        field(ui, t(Key::U3WhichWork), None, |ui| match &upstream {
                            Some(name) => {
                                paint::text(ui, name, Type::Body, c(C::Ink));
                            }
                            None => {
                                let lines = self.work_lines();
                                let current = lines.iter().find(|w| w.work.eq_ignore_ascii_case(self.typed.g_work.trim())).map(|w| format!("#{} {}", w.seq, w.name));
                                let labels: Vec<String> = lines.iter().map(|w| w.name.clone()).collect();
                                let seqs: Vec<String> = lines.iter().map(|w| format!("#{}", w.seq)).collect();
                                // The third column: the first anchor's block time, or "not on chain".
                                let times: Vec<String> = lines.iter().map(|w| w.row.anchored_at.map(crate::when::when).unwrap_or_else(|| t(Key::V2StateLanded).to_string())).collect();
                                let items: Vec<menu::Item> = lines
                                    .iter()
                                    .enumerate()
                                    .map(|(i, w)| {
                                        use crate::ledgerx::Lamp;
                                        let why = match (w.deleted, w.lamp) {
                                            (Some(_), _) => Some((t(Key::V2Deleted), PillTone::Grey)),
                                            (None, Lamp::Anchored) => None,
                                            (None, Lamp::Submitted | Lamp::Included) => Some((t(Key::V2OnChainWait), PillTone::Warn)),
                                            (None, Lamp::Remembered | Lamp::RememberedStale) => Some((t(Key::V2AwaitThisCheck), PillTone::Warn)),
                                            (None, _) => Some((t(Key::V2PendingGrey), PillTone::Warn)),
                                        };
                                        menu::Item::Row(menu::Row { lead: &seqs[i], label: &labels[i], struck: w.deleted.is_some(), disabled: why.is_some(), pill: why, trail: &times[i], ..Default::default() })
                                    })
                                    .collect();
                                let words = date_words();
                                let spec = pick::Spec {
                                    hint: t(Key::SearchWorks),
                                    empty: t(Key::SearchNone),
                                    w: PICK_WIDE_W,
                                    dates: Some(pick::Dates { words: &words, today: today_of((self.shell.clock)()), from_hint: t(Key::DateFrom), to_hint: t(Key::DateTo) }),
                                };
                                let keep = |i: usize, q: &str, from: &str, to: &str| {
                                    let w = &lines[i];
                                    matches(q, &[&w.name, &w.work, &seqs[i]]) && crate::when::within(w.row.anchored_at, from, to)
                                };
                                let mut picked = None;
                                width::then(
                                    ui,
                                    |ui| picked = pick::key(ui, "grant-work", t(Key::V2PickWorkBtn), !lines.is_empty(), false, &spec, &items, &keep),
                                    |ui, room| match &current {
                                        Some(s) => {
                                            paint::line(ui, s, Type::Body, c(C::Ink), room);
                                        }
                                        None => {
                                            paint::line(ui, t(if lines.is_empty() { Key::U3NoWorksYet } else { Key::U3PickWorkHint }), Type::Note, c(C::Ink2), room);
                                        }
                                    },
                                );
                                if let Some(i) = picked {
                                    self.typed.g_work = lines[i].work.clone();
                                    self.typed.g_history = lines[i].id.clone();
                                }
                            }
                        });
                        // The terms file, dropped or chosen; its fingerprint fills the advanced field. A
                        // fingerprint typed by hand shows as its own row.
                        field(ui, t(Key::U3TermsFile), None, |ui| {
                            if self.ux.u3.terms.as_ref().map(|x| x.hex != self.typed.g_terms.trim()).unwrap_or(false) {
                                self.ux.u3.terms = None;
                            }
                            let mut pick = false;
                            match self.ux.u3.terms.as_ref().map(|x| (x.name.clone(), x.size.map(size_say).unwrap_or_default())) {
                                Some((name, size)) => pick = drop::file_row(ui, "terms", &name, &size, false, t(Key::U3TermsSwap)),
                                None if !self.typed.g_terms.trim().is_empty() => {
                                    drop::file_row(ui, "terms-typed", t(Key::TermsTyped), "", false, "");
                                }
                                None => {
                                    let d = drop::zone(ui, "terms-file", None, &[t(Key::U3TermsDrop), t(Key::DropClickFile)], None, 84.0, drop::Shape::Column, true);
                                    if let Some(p) = self.one_drop(&d, now) {
                                        self.take_terms(&p, now);
                                    } else if d.clicked {
                                        pick = true;
                                    }
                                    hint(ui, t(Key::U3TermsNote));
                                }
                            }
                            if let Some(p) = path_answer(ui.ctx(), egui::Id::new("zikaron-path-grant-terms"), pick, crate::platform::Pick::File) {
                                self.take_terms(&p, now);
                            }
                        });
                        // Validity: presets or custom seconds; the dates; the overlap check.
                        field(ui, t(Key::U3Window), None, |ui| {
                            let presets = [(7u32, t(Key::U3Days7)), (30, t(Key::U3Days30)), (90, t(Key::U3Days90)), (0, t(Key::U3Custom))];
                            let cells: Vec<seg::Cell> = presets.iter().map(|(_, l)| seg::Cell::from(*l)).collect();
                            let cur = presets.iter().position(|(d, _)| *d == self.ux.u3.grant_days).unwrap_or(3);
                            if let Some(i) = seg::seg(ui, "grant-days", &cells, cur) {
                                self.ux.u3.grant_days = presets[i].0;
                            }
                            if self.ux.u3.grant_days == 0 {
                                card::grid2(ui, "grant-custom", 2, |ui, i| {
                                    if i == 0 {
                                        field(ui, t(Key::GrantFrom), None, |ui| input::mono(ui, &mut self.typed.g_from, ""));
                                    } else {
                                        field(ui, t(Key::GrantTo), None, |ui| input::mono(ui, &mut self.typed.g_to, ""));
                                    }
                                });
                            }
                            let span = match (self.typed.g_from.trim().parse::<u64>(), self.typed.g_to.trim().parse::<u64>()) {
                                (Ok(a), Ok(b)) => fill2(Key::U3FromTo, &crate::when::day(a), &crate::when::day(b)),
                                _ if chain_now.is_none() && self.ux.u3.grant_days > 0 => t(Key::U3NoChainTimeForPreset).to_string(),
                                _ => t(Key::CountdownNoWindow).to_string(),
                            };
                            hint(ui, &span);
                            // Unreadable custom cells mean the check did not run: show "not checked yet", never zero.
                            let n = self.shell.clash.len();
                            let (fs, ts) = (self.typed.g_from.trim(), self.typed.g_to.trim());
                            let window_ok = (fs.is_empty() && ts.is_empty()) || (fs.parse::<u64>().is_ok() && ts.parse::<u64>().is_ok());
                            if self.typed.g_work.trim().is_empty() || !window_ok {
                                states::okline(ui, Mark::Todo, t(Key::U3ClashNotYet));
                            } else {
                                states::okline(ui, if n == 0 { Mark::Ok } else { Mark::Bad }, &fill1(Key::U3ClashLine, &n.to_string()));
                            }
                        });
                        fold::fold_ex(ui, "grant-more", t(Key::U3GrantMore), upstream.is_some(), |ui| {
                            ui.spacing_mut().item_spacing.y = tk::S3;
                            toggle::toggle(ui, &mut self.typed.g_exclusive, t(Key::GrantExclusive), t(Key::GrantExclusiveNote), true);
                            field(ui, t(Key::U3Upstream), None, |ui| input::mono(ui, &mut self.typed.g_upstream, "0x\u{2026}"));
                            field(ui, t(Key::U3ScopeNote), None, |ui| input::area_words(ui, &mut self.typed.g_scope, 2, ""));
                        });
                        fold::fold(ui, "grant-adv", t(Key::U3MoreOptions), |ui| {
                            field(ui, t(Key::U3TermsHash), None, |ui| input::mono(ui, &mut self.typed.g_terms, "0x\u{2026}"));
                            let from_card = self.ux.u3.terms.as_ref().map(|x| x.hex == self.typed.g_terms.trim()).unwrap_or(false);
                            hint(ui, t(if from_card { Key::U3TermsFilled } else { Key::U3TermsNote }));
                        });
                        paint::rule(ui, 0.0);
                        let w = ui.available_width();
                        ui.allocate_ui_with_layout(egui::vec2(w, tk::KEY_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if page::Guide::key(ui, t(self.anchor_key()), ready).clicked() {
                                confirm = true;
                            }
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                paint::text(ui, t(Key::U3GrantHint), Type::Note, c(C::Ink2));
                            });
                        });
                        // A relicense signed here joins this seat's queue, so the key to anchor it is offered here.
                        if upstream.is_some() && self.ux.u4.relicense_signed && self.shell.queue.len() > 0 {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = tk::S3;
                                to_queue = key::key(ui, t(Key::GoAnchor), Role::Secondary, true).clicked();
                                paint::text(ui, &fill1(Key::U4RecentQueued, &self.shell.queue.len().to_string()), Type::Note, c(C::Ink2));
                            });
                        }
                    });
                });
            }
            card::Side::Side => {
                stagger(ui, 1, |ui| self.grant_checks(ui, now));
            }
        });
        if confirm {
            self.u3_open_confirm(U3Confirm::Grant);
        }
        if to_queue {
            self.go(Place::View(crate::nav::View::Works, crate::nav::tab::WORKS_PENDING), now);
        }
    }

    /// Take a terms file: compute its fingerprint (an unreadable file gives a named toast), show the file in the
    /// form and fill the fingerprint field.
    pub(super) fn take_terms(&mut self, path: &str, now: f64) {
        match crate::anchorx::of_file(std::path::Path::new(path)) {
            Ok(x) => {
                let name = width::file_name(&x.subject);
                self.typed.g_terms = x.hex();
                self.ux.u3.terms = Some(TermsFile { path: path.to_string(), name, size: x.size, hex: x.hex() });
            }
            Err(f) => self.say_fault(&f, now),
        }
    }

    /// "Checks before issuing": the first-grant checklist's two manual steps and the key's balance.
    fn grant_checks(&mut self, ui: &mut egui::Ui, now: f64) {
        let w = self.shell.wizard.clone();
        let next = w.next();
        card::flat(ui, |ui| {
            ui.spacing_mut().item_spacing.y = tk::S3;
            card_title(ui, t(Key::U3BeforeSigning));
            let mut rows: Vec<(Mark, String, Option<String>)> = crate::wizard::Step::ALL
                .iter()
                .map(|s| {
                    let m = if w.has(*s) {
                        Mark::Ok
                    } else if Some(*s) == next {
                        Mark::Warn
                    } else {
                        Mark::Todo
                    };
                    (m, t(step_name(*s)).to_string(), None)
                })
                .collect();
            rows.push(match self.chain_read() {
                Err(f) => (Mark::Bad, fill1(Key::SetReadFailedNow, f.human()), None),
                Ok(Some(Done::Chain { gas_wei: Some(g), .. })) if *g > 0 => (Mark::Ok, fill1(Key::U3GasLeft, &eth_held(*g)), None),
                Ok(Some(Done::Chain { gas_wei: Some(_), .. })) => (Mark::Bad, t(Key::GuideGas).to_string(), None),
                _ => (Mark::Todo, t(Key::U3GasUnread).to_string(), None),
            });
            states::checks(ui, &rows, false);
            match next {
                Some(s) => {
                    input::line(ui, &mut self.typed.wiz_said, t(Key::WizardSaid));
                    if key::key(ui, &fill1(Key::U3TickStep, t(step_name(s))), Role::Secondary, true).clicked() {
                        let a = Action::WizardTick { step: s.as_str().to_string(), said: self.typed.wiz_said.clone() };
                        self.act(a, now);
                        self.typed.wiz_said.clear();
                    }
                }
                None => hint(ui, t(Key::WizardDone)),
            }
            if w.has(crate::wizard::Step::ALL[0]) && key::key(ui, t(Key::DoWizardReset), Role::Secondary, true).clicked() {
                self.act(Action::WizardReset, now);
            }
        });
    }

    /// Label of the key that writes an entry, set by the "auto put on chain" setting: "put on chain now" when
    /// on, "add to ledger" when off. Used by the record and grant forms and their confirmation sheets.
    pub(super) fn anchor_key(&self) -> Key {
        if self.shell.settings.auto_anchor {
            Key::AnchorNowKey
        } else {
            Key::AddToLedgerKey
        }
    }
}
