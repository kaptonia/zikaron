use super::*;

impl Win {
    /// Records in this ledger, newest first, each with its deletion read by the convention.
    pub(super) fn work_lines(&self) -> Vec<WorkLine> {
        let Some((rows, _)) = self.shell.rows.as_ref() else { return Vec::new() };
        let reading = crate::retractx::read(rows);
        let mut out: Vec<WorkLine> = rows
            .iter()
            .filter(|r| r.kind == zikaron::tokens::EntryType::History)
            .filter_map(|r| {
                let work = r.work.clone()?;
                let deleted = reading.deleted.iter().find(|(k, _)| k.eq_ignore_ascii_case(&r.id)).map(|(_, (seq, _))| *seq);
                Some(WorkLine { name: human_summary(r), work, id: r.id.clone(), seq: r.seq, lamp: r.lamp, deleted, row: r.clone() })
            })
            .collect();
        out.sort_by(|a, b| b.seq.cmp(&a.seq));
        out
    }

    /// Ledger state ("normal / problems"): normal when the audit has a reading, the chain is not broken and
    /// there is no real fault. Entries awaiting anchoring are not faults; this desk's deletions count as
    /// recognized, not as unknown types.
    pub(super) fn ledger_state(&self) -> Option<bool> {
        use zikaron::tokens::Key as T;
        let a = self.shell.audit.as_ref()?;
        if a.broken {
            return Some(false);
        }
        let items = crate::auditx::items(&a.report);
        let n = |k: T| items.iter().find(|i| i.key == k.as_str()).and_then(|i| i.count).unwrap_or(0);
        let convention = self.shell.rows.as_ref().map(|(r, _)| crate::retractx::read(r).count).unwrap_or(0);
        let bad = n(T::Findings) + n(T::Missing) + n(T::Malformed) + n(T::Void) + n(T::AdoptionUnproven) + n(T::Unproven) + n(T::UnknownType).saturating_sub(convention);
        Some(bad == 0)
    }

    /// The records page: a compact drop zone for a new record, search and "check a file", then the table.
    pub(super) fn works(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
        let works = self.work_lines();
        let read = self.shell.rows.is_some();
        let mut open: Option<String> = None;
        let mut new_record: Option<String> = None;
        let mut check = false;
        let mut query = self.ux.search.get("works").cloned().unwrap_or_default();
        stagger(ui, 0, |ui| {
            let d = drop::zone(ui, "works-new", Some(Glyph::Inbox), &[t(Key::V2NewAnchor), t(Key::DropClickAny)], None, 84.0, drop::Shape::Row, true);
            new_record = self.drop_or_pick(&d, crate::platform::Pick::FileOrFolder, now);
        });
        stagger(ui, 1, |ui| {
            let (_, hit) = width::then(ui, |ui| key::key(ui, t(Key::VerifyTitle), Role::Secondary, true).clicked(), |ui, room| input::search(ui, &mut query, t(Key::SearchWorks), room));
            check = hit;
            // "Check a file": the file's digest is read and the ledger answers which entry it is, or that it
            // is not here.
            let gen = self.shell.rows_gen;
            if let Some(v) = self.ux.u3.file_verdict.as_ref().filter(|(g, _)| *g == gen).map(|(_, v)| v.clone()) {
                motion::swap(ui, egui::Id::new("works-verdict"), motion::key_of(&verify_said(&v)), |ui| {
                    states::okline(ui, verify_mark(&v), &verify_said(&v));
                    if let Some((n, _)) = v.signed_as.as_ref() {
                        hint(ui, &fill1(Key::VerifySignedAs, n));
                    }
                });
            }
        });
        let at = self.shell.remembered.as_ref().map(|r| r.at);
        let shown: Vec<&WorkLine> = works.iter().filter(|w| matches(&query, &[&w.name, &w.work, &format!("#{}", w.seq), &work_state(w, at).0])).collect();
        stagger(ui, 2, |ui| {
            let cols = [
                table::col(t(Key::U3Work), table::Col::Fr(1.0)),
                table::col(t(Key::NavAnchoring), table::Col::Px(120.0)),
                table::col(t(Key::U4RecordFirstAt), table::Col::Px(190.0)),
                table::col_r(t(Key::V2RecordNo), table::Col::Px(84.0)),
                table::CHEV,
            ];
            let rows: Vec<table::Row> = shown
                .iter()
                .map(|w| {
                    let (label, tone, live) = work_state(w, at);
                    table::Row {
                        cells: vec![
                            table::Cell::Text(w.name.clone()),
                            if live { table::Cell::PillLive(label, tone) } else { table::Cell::Pill(label, tone) },
                            table::Cell::Mono(first_anchor_say(w.row.anchored_at)),
                            table::Cell::Seq(w.seq),
                            table::Cell::Chev,
                        ],
                        click: true,
                        gone: w.deleted.is_some(),
                        on: false,
                    }
                })
                .collect();
            let empty = t(if !read {
                Key::WbNotRead
            } else if works.is_empty() {
                Key::V2WorksEmpty
            } else {
                Key::SearchNone
            });
            if let Some(i) = table::table(ui, "works", &cols, true, &rows, empty).clicked {
                open = shown.get(i).map(|w| w.id.clone());
            }
        });
        self.ux.search.insert("works", query);
        if check {
            if let Some(p) = crate::platform::choose_path(crate::platform::Pick::File) {
                self.act(Action::VerifyFile { path: p }, now);
            }
        }
        if let Some(path) = new_record {
            self.u3_new_anchor_open();
            self.take_for_record(vec![path], now);
        }
        if let Some(id) = open {
            self.push(Route::Work(id), now);
        }
    }

    /// The pending tab: what waits to go on chain, and the batch card.
    pub(super) fn works_pending(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        let items = self.shell.queue.items.clone();
        let rows_all: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
        let reading = crate::retractx::read(&rows_all);
        let mut query = self.ux.search.get("queue").cloned().unwrap_or_default();
        let mut open: Option<String> = None;
        let mut confirm: Option<U3Confirm> = None;
        let mut sync = false;
        stagger(ui, 0, |ui| {
            let w = ui.available_width();
            input::search(ui, &mut query, t(Key::SearchQueue), w);
        });
        let shown: Vec<(&crate::queue::Queued, Option<&crate::ledgerx::Row>)> = items
            .iter()
            .map(|q| (q, rows_all.iter().find(|x| x.id == q.id)))
            .filter(|(q, row)| {
                let face = row.map(|x| row_face(&rows_all, &reading, x));
                let seq = row.map(|x| format!("#{}", x.seq)).unwrap_or_default();
                matches(&query, &[&seq, face.as_ref().map(|f| f.0).unwrap_or(""), face.as_ref().map(|f| f.1.as_str()).unwrap_or(""), &q.id])
            })
            .collect();
        stagger(ui, 1, |ui| {
            let cols = [table::SEQ, table::TYPE, table::col("", table::Col::Fr(1.0)), table::col("", table::Col::Px(96.0)), table::MARK, table::CHEV];
            let rows: Vec<table::Row> = shown
                .iter()
                .map(|(q, row)| {
                    let (tag, summary) = row.map(|x| {
                        let f = row_face(&rows_all, &reading, x);
                        (f.0.to_string(), f.1)
                    })
                    .unwrap_or_else(|| (t(Key::None_).to_string(), t(Key::UnnamedRecord).to_string()));
                    let lamp = row.map(|x| x.lamp).unwrap_or(crate::ledgerx::Lamp::Queued);
                    table::Row {
                        cells: vec![
                            row.map(|x| table::Cell::Seq(x.seq)).unwrap_or(table::Cell::Empty),
                            table::Cell::Tag(tag),
                            table::Cell::Text(summary),
                            table::Cell::Mono(crate::when::day(q.at)),
                            table::Cell::Mark(lamp_mark(lamp)),
                            table::Cell::Chev,
                        ],
                        click: true,
                        gone: false,
                        on: false,
                    }
                })
                .collect();
            let empty = t(if items.is_empty() { Key::U3QueueEmpty } else { Key::SearchNone });
            if let Some(i) = table::table(ui, "queue", &cols, false, &rows, empty).clicked {
                open = shown.get(i).map(|(q, _)| q.id.clone());
            }
        });
        // Failing to resume is said plainly: a submitted batch waits for its receipt and this chain has no
        // node now.
        if let Some(chain) = self.shell.resume_blocked {
            states::okline(ui, Mark::Warn, &fill1(Key::QueueResumeNoNode, &chain.to_string()));
        }
        let sent = self.shell.sent.clone();
        if !items.is_empty() || sent.is_some() {
            stagger(ui, 2, |ui| {
                card::card(ui, |ui| {
                    // This line counts the same batch the key beside it sends (the sendable ones).
                    let can = self.shell.queue.sendable();
                    let est = match self.shell.gas {
                        Some((n, g)) if n == can && n > 0 => fill2(Key::U3BatchEstimate, &n.to_string(), &g.to_string()),
                        _ => fill1(Key::U3BatchNoEstimate, &can.to_string()),
                    };
                    kv::kv(ui, &[(t(Key::U3BatchLabel), Val::text(est))]);
                    hint(ui, t(Key::V2PendingNote));
                    ui.add_space(2.0);
                    keys_row(ui, |ui| {
                        if page::Guide::key(ui, t(Key::U3SendAll), can > 0).clicked() {
                            confirm = Some(U3Confirm::Send { count: can });
                        }
                        if key::key(ui, t(Key::NavRefresh), Role::Secondary, true).clicked() {
                            sync = true;
                        }
                    });
                    if let Some(Done::Anchored { tx, confirmed, sent, dropped, chain, .. }) = sent.clone() {
                        paint::rule(ui, tk::S2);
                        kv::kv(ui, &[(t(Key::U3LastSend), Val::Mark(if confirmed { Mark::Ok } else { Mark::Busy }, fill2(Key::U3SentSay, &sent.to_string(), &dropped.to_string())))]);
                        details(ui, "queue-sent", &[(t(Key::SendTx), Val::mono(format!("{chain} \u{b7} {tx}")))]);
                    }
                });
            });
        }
        self.ux.search.insert("queue", query);
        if sync {
            self.refresh_chain(now);
        }
        if let Some(cf) = confirm {
            self.u3_open_confirm(cf);
        }
        if let Some(id) = open {
            self.push(Route::Pending(id), now);
        }
    }

    /// A record's detail page.
    pub(super) fn work_detail(&mut self, ui: &mut egui::Ui, id: &str, now: f64) {
        self.ensure_rows(now);
        let Some(w) = self.work_lines().into_iter().find(|w| w.id.eq_ignore_ascii_case(id)) else {
            states::empty(ui, Glyph::File, t(if self.shell.rows.is_some() { Key::DetailGone } else { Key::WbNotRead }));
            return;
        };
        // Its place among the sendable entries (a submitted one is not among them).
        let queued_at = if w.lamp == crate::ledgerx::Lamp::Submitted {
            None
        } else {
            self.shell.queue.items.iter().filter(|q| !matches!(q.step, crate::queue::Step::Submitted { .. })).position(|q| q.id == w.id)
        };
        let at = self.shell.remembered.as_ref().map(|r| r.at);
        let mut confirm: Option<U3Confirm> = None;
        let mut grant = false;
        let mut kit = false;
        stagger(ui, 0, |ui| {
            let (label, tone, live) = work_state(&w, at);
            card::hero(ui, &w.name, &format!("#{} \u{b7} {}", w.seq, t(Key::TagAnchor)), w.deleted.is_some(), |ui| {
                if live {
                    mark::pill_live(ui, &label, tone);
                } else {
                    mark::pill(ui, &label, tone);
                }
            });
        });
        if let Some(n) = w.deleted {
            stagger(ui, 1, |ui| states::note_box(ui, &fill1(Key::V2DeletedNote, &n.to_string())));
        }
        let files = self.files_of(&w.id);
        stagger(ui, 2, |ui| {
            let mut rows = vec![
                (t(Key::NavAnchoring), Val::Mark(lamp_mark(w.lamp), lamp_label(w.lamp, true, at))),
                (t(Key::U4RecordFirstAt), Val::mono(first_anchor_say(w.row.anchored_at))),
                (t(Key::V2RecordNo), Val::mono(format!("#{}", w.seq))),
            ];
            if !files.is_empty() {
                rows.push((t(Key::U3File), Val::text(files.join(", "))));
            }
            kv_section(ui, t(Key::BasicInfo), &rows);
        });
        if w.lamp == crate::ledgerx::Lamp::Submitted {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }
        if w.deleted.is_none() {
            stagger(ui, 3, |ui| {
                keys_row(ui, |ui| match queued_at {
                    Some(i) => {
                        if page::Guide::key(ui, t(Key::U3SendNow), true).clicked() {
                            confirm = Some(U3Confirm::Send { count: i + 1 });
                        }
                        kit = key::key(ui, t(Key::U3UseForKit), Role::Secondary, true).clicked();
                        paint::text(ui, t(Key::V2AnchorFirstHint), Type::Small, c(C::Ink3));
                    }
                    None => {
                        let anchored = w.lamp == crate::ledgerx::Lamp::Anchored;
                        grant = key::key(ui, t(Key::V2GrantToOthers), Role::Secondary, anchored).clicked();
                        kit = key::key(ui, t(Key::U3UseForKit), Role::Secondary, true).clicked();
                        if !anchored {
                            // Verified last pass but not yet this pass: "waiting for this check", never telling
                            // the person to anchor again.
                            let remembered = matches!(w.lamp, crate::ledgerx::Lamp::Remembered | crate::ledgerx::Lamp::RememberedStale);
                            paint::text(ui, t(if remembered { Key::V2AwaitThisCheck } else { Key::V2AnchorFirstHint }), Type::Small, c(C::Ink3));
                        }
                    }
                });
            });
        }
        stagger(ui, 4, |ui| {
            let mut rows = vec![
                (t(Key::ContentHash), Val::mono(w.work.clone())),
                (t(Key::DetailPick), Val::mono(w.id.clone())),
                (t(Key::EntryPrev), Val::mono(w.row.prev.clone().unwrap_or_else(|| t(Key::None_).to_string()))),
                (t(Key::CanonBytes), Val::mono(w.row.bytes.to_string())),
            ];
            if let Some((chain, tx)) = w.row.tx.clone() {
                rows.push((t(Key::AnchorTx), Val::mono(format!("{chain} \u{b7} {tx}"))));
            }
            details_card(ui, "work-details", &rows);
        });
        if w.deleted.is_none() {
            stagger(ui, 5, |ui| {
                if page::Guide::last(ui, t(Key::V2DeleteWork), true).clicked() {
                    confirm = Some(U3Confirm::Retract { subject: w.id.clone() });
                }
            });
        }
        if grant {
            self.typed.g_work = w.work.clone();
            self.typed.g_history = w.id.clone();
            self.go(Place::View(crate::nav::View::Grants, crate::nav::tab::GRANTS_NEW), now);
        }
        if kit {
            // Arriving from a record preselects that record's anchor entry.
            self.typed.pick_ids = w.id.to_ascii_lowercase();
            self.typed.pick_from.clear();
            self.typed.pick_to.clear();
            self.push(Route::Kit, now);
        }
        if let Some(cf) = confirm {
            self.u3_open_confirm(cf);
        }
    }

    /// The detail of an entry waiting to go on chain.
    pub(super) fn pending_detail(&mut self, ui: &mut egui::Ui, id: &str, now: f64) {
        self.ensure_rows(now);
        let items = self.shell.queue.items.clone();
        let Some(q) = items.iter().find(|q| q.id.eq_ignore_ascii_case(id)).cloned() else {
            // Sent and gone from the queue: its entry detail says where it stands now.
            self.entry_detail(ui, id, now);
            return;
        };
        let rows_all: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
        let reading = crate::retractx::read(&rows_all);
        let row = rows_all.iter().find(|x| x.id == q.id).cloned();
        let pos = items.iter().filter(|x| !matches!(x.step, crate::queue::Step::Submitted { .. })).position(|x| x.id == q.id);
        let (tag, summary) = row.as_ref().map(|x| {
            let f = row_face(&rows_all, &reading, x);
            (f.0.to_string(), f.1)
        })
        .unwrap_or_else(|| (t(Key::None_).to_string(), t(Key::UnnamedRecord).to_string()));
        let lamp = row.as_ref().map(|x| x.lamp).unwrap_or(crate::ledgerx::Lamp::Queued);
        let at = self.shell.remembered.as_ref().map(|r| r.at);
        let mut confirm = None;
        stagger(ui, 0, |ui| {
            let title = row.as_ref().map(|x| format!("#{} \u{b7} {tag}", x.seq)).unwrap_or_else(|| tag.clone());
            card::hero(ui, &title, &summary, false, |ui| lamp_pill_ui(ui, lamp, at));
        });
        stagger(ui, 1, |ui| {
            kv_section(
                ui,
                t(Key::BasicInfo),
                &[
                    (t(Key::U3Entry), Val::text(summary.clone())),
                    (t(Key::U3QueuedAt), Val::mono(crate::when::when(q.at))),
                    (t(Key::U3SendsWith), Val::text(match pos {
                        Some(k) => fill1(Key::U3FirstN, &(k + 1).to_string()),
                        None => t(Key::U3Submitted).to_string(),
                    })),
                    (t(Key::U4RecordFirstAt), Val::mono(first_anchor_say(row.as_ref().and_then(|r| r.anchored_at)))),
                ],
            );
        });
        if let Some(k) = pos {
            stagger(ui, 2, |ui| {
                if page::Guide::key(ui, t(Key::U3SendNow), true).clicked() {
                    confirm = Some(U3Confirm::Send { count: k + 1 });
                }
            });
        }
        stagger(ui, 3, |ui| details_card(ui, "pending-details", &[(t(Key::DetailPick), Val::mono(q.id.clone()))]));
        if let Some(cf) = confirm {
            self.u3_open_confirm(cf);
        }
    }
}
