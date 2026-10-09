use super::*;

impl Win {
    /// Read the table once if it has not been read, so opening a page shows the ledger without the user
    /// pressing anything.
    ///
    /// It starts a background pass (no disk I/O in the frame). A task of this kind starts itself only once:
    /// the condition checks whether `Tasks` recorded an attempt (success or failure), so a failed read does
    /// not start another pass every frame.
    pub(super) fn ensure_rows(&mut self, now: f64) {
        if self.shell.rows.is_some() || self.shell.home.is_none() || self.shell.tasks.attempted(crate::task::Kind::Ledger) {
            return;
        }
        self.auto(Action::ReadLedger, now);
    }

    /// The key scan's reading, returned only when it is about the address now in the form.
    pub(super) fn sighting_now(&self) -> Option<(String, usize, usize)> {
        let want = self.typed.sc_to.trim();
        self.shell.sighting.as_ref().filter(|(who, _, _)| !want.is_empty() && who.eq_ignore_ascii_case(want)).cloned()
    }

    /// Read the register once, like [`Self::ensure_rows`].
    pub(super) fn ensure_grants(&mut self, now: f64) {
        if self.shell.grants.is_some() || self.shell.home.is_none() || self.shell.tasks.attempted(crate::task::Kind::Grants) {
            return;
        }
        self.auto(Action::ReadGrants, now);
    }

    /// The chain's current time, taken only from the chain pass's reading.
    pub(super) fn chain_now(&self) -> Option<u64> {
        self.shell.chain_now()
    }

    /// The ledger's toolbar: the filter and "more" (import existing records, change key or hand over, add a
    /// note, sign a claim for someone).
    pub(super) fn ledger_acts(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        let items = [menu::row(t(Key::U3AdoptMenu)), menu::row(t(Key::U3SucceedMenu)), menu::row(t(Key::U3AnnotateMenu)), menu::row(t(Key::U3AttestMenu))];
        if let Some(i) = menu::menu_key(ui, "ledger-more", t(Key::NavMore), false, 190.0, &items) {
            self.u3_form_open([U3Form::Adopt, U3Form::Succeed, U3Form::Annotate, U3Form::Attest][i]);
        }
        let cells = [seg::Cell::from(t(Key::U3FilterAll)), seg::Cell::from(t(Key::ItemUnanchored)), seg::Cell::from(t(Key::KindGrant))];
        if let Some(i) = seg::seg(ui, "ledger-filter", &cells, self.ux.u3.led_filter) {
            self.ux.u3.led_filter = i;
            self.ux.entry = motion::Entry::Fade;
            self.ux.entry_key = self.ux.entry_key.wrapping_add(1);
        }
    }

    /// The ledger page: the status strip (with the ledger check folding open under it), search, and every entry.
    pub(super) fn ledger_page(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
        let rows: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
        stagger(ui, 0, |ui| self.audit_strip(ui, now));
        let read = self.shell.rows.is_some();
        let reading = crate::retractx::read(&rows);
        let mut query = self.ux.search.get("ledger").cloned().unwrap_or_default();
        let mut range = self.ux.range.get("ledger").cloned().unwrap_or_default();
        let today = (self.shell.clock)();
        stagger(ui, 1, |ui| search_row(ui, "ledger", &mut query, t(Key::SearchLedger), &mut range, today));
        // Newest on top (the table is read newest first).
        let hide = self.shell.settings.hide_local_deletions;
        let shown: Vec<&crate::ledgerx::Row> = rows
            .iter()
            .filter(|r| !(hide && r.lamp.local()))
            .filter(|r| match self.ux.u3.led_filter {
                // Entries the last pass recorded as anchored are not listed as waiting.
                1 => !r.lamp.confirmed(),
                2 => r.kind == zikaron::tokens::EntryType::Grant,
                _ => true,
            })
            .filter(|row| {
                let (tag, summary, _) = row_face(&rows, &reading, row);
                matches(&query, &[&format!("#{}", row.seq), tag, &summary, &row.id])
            })
            .filter(|row| crate::when::within(row.anchored_at, &range.0, &range.1))
            .collect();
        let mut open: Option<String> = None;
        stagger(ui, 2, |ui| {
            let cols = [table::SEQ, table::TYPE, table::col("", table::Col::Fr(1.0)), table::col("", table::Col::Px(180.0)), table::MARK, table::CHEV];
            let trs: Vec<table::Row> = shown
                .iter()
                .map(|r| {
                    let (tag, summary, struck) = row_face(&rows, &reading, r);
                    table::Row {
                        cells: vec![table::Cell::Seq(r.seq), table::Cell::Tag(tag.to_string()), table::Cell::Text(summary), table::Cell::Mono(first_anchor_say(r.anchored_at)), table::Cell::Mark(lamp_mark(r.lamp)), table::Cell::Chev],
                        click: true,
                        gone: struck,
                        on: false,
                    }
                })
                .collect();
            let empty = if !read {
                t(Key::WbNotRead)
            } else if rows.is_empty() {
                t(Key::WbRecentNone)
            } else if !query.trim().is_empty() || !range.0.is_empty() || !range.1.is_empty() {
                t(Key::SearchNone)
            } else {
                t(Key::U3NoneOfThisKind)
            };
            if let Some(i) = table::table(ui, "ledger", &cols, false, &trs, empty).clicked {
                open = shown.get(i).map(|r| r.id.clone());
            }
        });
        self.ux.search.insert("ledger", query);
        self.ux.range.insert("ledger", range);
        if let Some(id) = open {
            self.push(Route::Entry(id), now);
        }
    }

    /// The status strip: "ledger status · (mark) normal · N entries", plus "put all on chain" when entries can
    /// be sent; clicking the words folds the ledger check open under it.
    fn audit_strip(&mut self, ui: &mut egui::Ui, now: f64) {
        let (m, state) = match self.ledger_state() {
            Some(true) => (Mark::Ok, t(Key::V2Normal)),
            Some(false) => (Mark::Bad, t(Key::V2Abnormal)),
            // With no network configured, say why nothing was checked.
            None if self.shell.settings.chain_id.is_none() || self.shell.settings.registry.is_none() || self.shell.endpoints.is_empty() => (Mark::Warn, t(Key::U3AuditChipNoNetwork)),
            None => (Mark::Todo, t(Key::U3AuditChipNever)),
        };
        let count = self.shell.rows.as_ref().map(|(r, _)| fill1(Key::U3EntriesTotal, &r.len().to_string()));
        let root_anchored = self.shell.rows.as_ref().and_then(|(r, _)| crate::ledgerx::root_anchored(r));
        // "Put all on chain" only when entries can be sent (the same reading as the export page's "now").
        let sendable: Vec<(String, bool)> = self.shell.rows.as_ref().map(|(r, _)| crate::kitx::unanchored(&r.iter().collect::<Vec<_>>()).1).unwrap_or_default();
        let open = self.ux.u3.audit_open;
        let mut flip = false;
        let mut send_all = false;
        // A batch sent but not included by the end of its wait: offer a resend with higher fees when the current
        // price is above its cap; after three resends, say so.
        let stuck = self.shell.stuck.clone();
        let bump = stuck.as_ref().filter(|s| matches!(s.offer, crate::task::Offer::Resend { .. } | crate::task::Offer::Unheld { .. })).and_then(|s| s.txs.last().cloned());
        let spent = stuck.as_ref().is_some_and(|s| s.offer == crate::task::Offer::Spent);
        // Held by no node and its nonce unused: say so, and the key resends it at the current price.
        let unheld = stuck.as_ref().is_some_and(|s| matches!(s.offer, crate::task::Offer::Unheld { .. }));
        let mut bump_open = false;
        card::card_pad(ui, egui::vec2(18.0, 12.0), |ui| {
            let w = ui.available_width();
            ui.allocate_ui_with_layout(egui::vec2(w, 32.0), egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if !sendable.is_empty() && key::key(ui, t(Key::U3SendAll), Role::Secondary, true).clicked() {
                    send_all = true;
                }
                if bump.is_some() && key::key(ui, t(if unheld { Key::U3ResendKey } else { Key::U3BumpKey }), Role::Secondary, true).clicked() {
                    bump_open = true;
                }
                let resp = ui
                    .with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = tk::S2;
                        let (slot, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                        let turn = motion::flag(ui.ctx(), egui::Id::new("ledger-strip-caret"), open, tk::MID) * std::f32::consts::FRAC_PI_2;
                        zikaron_ui::icons::draw_glyph_turned(ui.painter(), Glyph::Chev, slot, c(C::Ink3), turn);
                        paint::text(ui, &format!("{} \u{b7}", t(Key::WbAudit)), Type::Body, c(C::Ink2));
                        mark::mark(ui, m);
                        paint::text(ui, state, Type::Body, c(C::Ink));
                        if let Some(n) = count.as_deref() {
                            paint::text(ui, &format!("\u{b7} {n}"), Type::Body, c(C::Ink2));
                        }
                        // Root not confirmed on chain, read from the genesis entry's state.
                        if root_anchored == Some(false) {
                            paint::text(ui, &format!("\u{b7} {}", t(Key::U3RootUnanchored)), Type::Body, c(C::BadInk));
                        }
                        if spent {
                            paint::text(ui, &format!("\u{b7} {}", fill1(Key::U3BumpSpent, &crate::queue::RESENDS_MAX.to_string())), Type::Body, c(C::Ink2));
                        }
                        if unheld {
                            paint::text(ui, &format!("\u{b7} {}", t(Key::U3Unheld)), Type::Body, c(C::BadInk));
                        }
                        ui.allocate_space(egui::vec2(ui.available_width(), 1.0));
                    })
                    .response;
                if ui.interact(resp.rect, egui::Id::new("ledger-strip-fold"), egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    flip = true;
                }
            });
            // The fold's body opens under the strip.
            let t_open = motion::flag(ui.ctx(), egui::Id::new("ledger-strip-open"), open, tk::MID);
            if t_open > 0.0 {
                let h_id = egui::Id::new("ledger-strip-h");
                let full = ui.ctx().data(|d| d.get_temp::<f32>(h_id)).unwrap_or(0.0);
                let top = ui.cursor().min;
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(top, egui::vec2(w, f32::INFINITY))));
                let mut clip = child.clip_rect();
                clip.max.y = clip.max.y.min(top.y + full * t_open + 1.0);
                child.set_clip_rect(clip);
                child.multiply_opacity(t_open);
                child.spacing_mut().item_spacing.y = tk::S3;
                paint::rule(&mut child, tk::S2);
                self.audit_body(&mut child);
                let measured = child.min_rect().height();
                if (measured - full).abs() > 0.5 {
                    ui.ctx().data_mut(|d| d.insert_temp(h_id, measured));
                    // Lay out again at once with the new size so no frame is drawn with the old one.
                    ui.ctx().request_discard("audit fold height changed");
                }
                ui.advance_cursor_after_rect(egui::Rect::from_min_size(top, egui::vec2(w, measured.min(full.max(1.0)) * t_open)));
            }
        });
        if flip && !send_all && !bump_open {
            self.ux.u3.audit_open = !open;
        }
        if let (true, Some(tx)) = (bump_open, bump) {
            self.u3_open_confirm(U3Confirm::Bump { tx });
        }
        if send_all {
            // Queue the entries not yet queued, then the confirmation counts the sendable ones.
            for (id, fresh) in &sendable {
                if *fresh {
                    self.act(Action::QueueEntry { id: id.clone() }, now);
                }
            }
            let count = self.shell.queue.sendable();
            if count > 0 {
                self.u3_open_confirm(U3Confirm::Send { count });
            }
        }
    }

    /// The ledger check: the core's report item by item in plain words, with the raw terms in details.
    fn audit_body(&mut self, ui: &mut egui::Ui) {
        use zikaron::tokens::Key as T;
        let Some(a) = self.shell.audit.clone() else {
            hint(ui, t(Key::U3AuditNeverHint));
            return;
        };
        let items = crate::auditx::items(&a.report);
        let convention = self.shell.rows.as_ref().map(|(r, _)| crate::retractx::read(r).count).unwrap_or(0);
        let lists: [(T, Key, bool); 12] = [
            (T::Anchored, Key::LampAnchored, true),
            (T::Unanchored, Key::U3AuUnanchored, false),
            (T::Missing, Key::U3AuMissing, false),
            (T::Findings, Key::U3AuFindings, false),
            (T::Excluded, Key::U3AuExcluded, false),
            (T::AdoptionUnproven, Key::U3AuAdoptionUnproven, false),
            (T::UnknownType, Key::U3AuUnknownType, false),
            (T::Malformed, Key::U3AuMalformed, false),
            (T::Unavailable, Key::U3AuUnavailable, false),
            (T::Unproven, Key::U3AuUnproven, false),
            (T::Void, Key::U3AuVoid, false),
            (T::Discarded, Key::U3AuDiscarded, false),
        ];
        card::flat_title(ui, &fill1(Key::U3AuditTitle, &label_human(&a.label)));
        let rows: Vec<(Mark, String, Option<String>)> = lists
            .iter()
            .map(|(tok, k, good)| {
                let n = items.iter().find(|i| i.key == tok.as_str()).and_then(|i| i.count);
                // This desk's own deletions are recognized by count and are not a fault; only extra ones are unknown.
                if *tok == T::UnknownType && convention > 0 && n == Some(convention) {
                    return (Mark::Ok, fill1(Key::V2ConventionEntries, &convention.to_string()), None);
                }
                let m = match (n, good) {
                    (None, _) => Mark::Todo,
                    (Some(0), true) => Mark::Todo,
                    (Some(_), true) => Mark::Ok,
                    (Some(0), false) => Mark::Ok,
                    (Some(_), false) => {
                        if matches!(tok, T::Unanchored | T::Excluded | T::Unavailable | T::Discarded) {
                            Mark::Warn
                        } else {
                            Mark::Bad
                        }
                    }
                };
                (m, t(*k).to_string(), Some(n.map(|x| x.to_string()).unwrap_or_else(|| t(Key::None_).to_string())))
            })
            .collect();
        states::checks(ui, &rows, true);
        hint(ui, &fill2(Key::U3AuditHint, &a.entries.to_string(), &self.shell.settings.audit_every.to_string()));
        let mut raw: Vec<(&str, Val)> = vec![(t(Key::AuditLabel), Val::mono(a.label.clone()))];
        raw.extend(items.iter().map(|i| (i.key, Val::mono(i.count.map(|n| n.to_string()).unwrap_or_else(|| width::elide_chars(&i.said, 40))))));
        details(ui, "audit-evidence", &raw);
    }

    /// A ledger entry's detail page. A grant shows the grant page; a record shows its readings, "export" and
    /// "use for grant"; any entry not on chain shows the way to anchor it.
    pub(super) fn entry_detail(&mut self, ui: &mut egui::Ui, id: &str, now: f64) {
        use zikaron::tokens::EntryType as E;
        self.ensure_rows(now);
        let rows_all: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
        let Some(row) = rows_all.iter().find(|r| r.id.eq_ignore_ascii_case(id)).cloned() else {
            states::empty(ui, Glyph::Ledger, t(if self.shell.rows.is_some() { Key::DetailGone } else { Key::WbNotRead }));
            return;
        };
        if row.kind == E::Grant {
            self.grant_detail(ui, &row, now);
            return;
        }
        let reading = crate::retractx::read(&rows_all);
        let (tag, summary, struck) = row_face(&rows_all, &reading, &row);
        let at = self.shell.remembered.as_ref().map(|r| r.at);
        let mut confirm: Option<U3Confirm> = None;
        let mut queue_it = false;
        let mut to_kit = false;
        let mut to_grant = false;
        let mut annotate = false;
        stagger(ui, 0, |ui| {
            card::hero(ui, &format!("#{} \u{b7} {tag}", row.seq), &summary, struck, |ui| {
                if struck {
                    mark::pill(ui, t(Key::V2Deleted), PillTone::Grey);
                } else {
                    lamp_pill_ui(ui, row.lamp, at);
                }
            });
        });
        stagger(ui, 1, |ui| {
            let first = (t(Key::U4RecordFirstAt), Val::mono(first_anchor_say(row.anchored_at)));
            let state = (t(Key::NavAnchoring), Val::Mark(lamp_mark(row.lamp), lamp_label(row.lamp, true, at)));
            let rows = if row.kind == E::History {
                vec![(t(Key::U3Work), Val::text(human_summary(&row))), state, first]
            } else {
                vec![(t(Key::U3Entry), Val::text(summary.clone())), state, first]
            };
            kv_section(ui, t(Key::BasicInfo), &rows);
        });
        // Available actions: a record exports and grants; an entry off chain goes on chain (or into the queue
        // first); an anchored entry of another kind takes a note.
        let offer = !matches!(
            row.lamp,
            crate::ledgerx::Lamp::Anchored
                | crate::ledgerx::Lamp::Remembered
                | crate::ledgerx::Lamp::RememberedStale
                | crate::ledgerx::Lamp::Included
                | crate::ledgerx::Lamp::Submitted
                | crate::ledgerx::Lamp::Deleted
                | crate::ledgerx::Lamp::LocalDeletion
        );
        let pos = self.shell.queue.items.iter().filter(|q| !q.step.in_flight()).position(|q| q.id == row.id);
        let anchored = row.lamp == crate::ledgerx::Lamp::Anchored;
        stagger(ui, 2, |ui| {
            keys_row(ui, |ui| {
                if row.kind == E::History && !struck {
                    to_kit = page::Page::new().primary(ui, t(Key::U3UseForKit)).1.clicked();
                    to_grant = key::key(ui, t(Key::U3UseForGrant), Role::Secondary, anchored).clicked();
                }
                if offer {
                    match pos {
                        Some(i) => {
                            if page::Guide::key(ui, t(Key::U3SendNow), true).clicked() {
                                confirm = Some(U3Confirm::Send { count: i + 1 });
                            }
                        }
                        // The genesis entry and entries whose queueing failed can be queued here too.
                        None => queue_it = key::key(ui, t(Key::U3QueueIt), Role::Secondary, true).clicked(),
                    }
                } else if anchored && !matches!(row.kind, E::History) {
                    annotate = key::key(ui, t(Key::U3AnnotateDots), Role::Secondary, true).clicked();
                }
            });
        });
        stagger(ui, 3, |ui| {
            let mut kv = Vec::new();
            if let Some(w) = row.work.clone() {
                kv.push((t(Key::ContentHash), Val::mono(w)));
            }
            kv.extend([
                (t(Key::DetailPick), Val::mono(row.id.clone())),
                (t(Key::U3RawType), Val::mono(if row.facts.raw_type.is_empty() { row.kind.as_str().to_string() } else { row.facts.raw_type.clone() })),
                (t(Key::U3RawSeq), Val::mono(row.seq.to_string())),
                (t(Key::EntryAuthor), Val::mono(row.author.clone())),
                (t(Key::EntryPrev), Val::mono(row.prev.clone().unwrap_or_else(|| t(Key::None_).to_string()))),
                (t(Key::CanonBytes), Val::mono(row.bytes.to_string())),
            ]);
            if let Some((chain, tx)) = row.tx.clone() {
                kv.push((t(Key::AnchorTx), Val::mono(format!("{chain} \u{b7} {tx}"))));
            }
            details_card(ui, "entry-details", &kv);
        });
        if to_kit {
            self.typed.pick_ids = row.id.to_ascii_lowercase();
            self.typed.pick_from.clear();
            self.typed.pick_to.clear();
            self.go(Place::View(crate::nav::View::Works, crate::nav::tab::WORKS_KIT), now);
        }
        if to_grant {
            if let Some(w) = row.work.clone() {
                self.typed.g_work = w;
                self.typed.g_history = row.id.clone();
            }
            self.go(Place::View(crate::nav::View::Grants, crate::nav::tab::GRANTS_NEW), now);
        }
        if queue_it {
            self.act(Action::QueueEntry { id: row.id.clone() }, now);
        }
        if annotate {
            self.typed.annotate_subject = row.id.clone();
            self.u3_form_open(U3Form::Annotate);
        }
        if let Some(cf) = confirm {
            self.u3_open_confirm(cf);
        }
    }

    /// A validity window as two dates "from to", or a note when there is none.
    pub(super) fn window_days(&self, w: Option<(u64, u64)>) -> String {
        window_long(w)
    }
}
