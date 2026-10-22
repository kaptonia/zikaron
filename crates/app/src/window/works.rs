use super::*;

/// The one width of a date column in the record lists (the records, the pending entries, the ledger).
pub(super) const DATE_W: f32 = 200.0;

/// The width of the records list's state column.
const STATE_W: f32 = 160.0;

/// The height of the records page's one-line drop zone.
const DROP_ROW_H: f32 = 84.0;

impl Win {
    /// Records in this ledger, newest first, each with its deletion as read by the retraction convention.
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

    /// Ledger state ("normal / problems"): the conclusion the shell judged from the last report
    /// (`Shell::ledger_ok`), which stands until the next judgement: a refresh never shows a mix of two.
    pub(super) fn ledger_state(&self) -> Option<bool> {
        self.shell.audit.as_ref().and(self.shell.ledger_ok)
    }

    /// The records page: a one-line drop zone for a new record, search, then the table (record, state, first
    /// anchor; each row opens the record).
    pub(super) fn works(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
        let works = self.work_lines();
        let read = self.shell.rows.is_some();
        let mut open: Option<String> = None;
        let mut new_record: Option<String> = None;
        let mut query = self.ux.search.get("works").cloned().unwrap_or_default();
        let mut range = self.ux.range.get("works").cloned().unwrap_or_default();
        let today = (self.shell.clock)();
        stagger(ui, 0, |ui| {
            let d = zikaron_ui::dropline::inline(ui, "works-new", Some(Glyph::Inbox), t(Key::WbDropTitle), t(Key::DropClickAny), DROP_ROW_H, true);
            new_record = self.drop_or_pick(ui.ctx(), "works-new", &d, crate::platform::Pick::FileOrFolder, now);
        });
        stagger(ui, 1, |ui| search_row(ui, "works", &mut query, t(Key::SearchWorks), &mut range, today));
        let at = self.shell.remembered.as_ref().map(|r| r.at);
        let hide = self.shell.settings.hide_local_deletions;
        let shown: Vec<&WorkLine> = works
            .iter()
            .filter(|w| !(hide && w.lamp.local()))
            .filter(|w| matches(&query, &[&w.name, &w.work, &format!("#{}", w.seq), &work_state(w, at).0]))
            .filter(|w| crate::when::within(w.row.anchored_at, &range.0, &range.1))
            .collect();
        stagger(ui, 2, |ui| {
            let cols = [table::col(t(Key::U3Work), table::Col::Fr(1.0)), table::col(t(Key::NavAnchoring), table::Col::Px(STATE_W)), table::col(t(Key::U4RecordFirstAt), table::Col::Px(DATE_W))];
            let rows: Vec<table::Row> = shown
                .iter()
                .map(|w| {
                    let (label, tone, live) = work_state(w, at);
                    table::Row {
                        cells: vec![
                            table::Cell::Text(w.name.clone()),
                            if live { table::Cell::PillLive(label, tone) } else { table::Cell::Pill(label, tone) },
                            table::Cell::Mono(first_anchor_say(w.row.anchored_at)),
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
        self.ux.range.insert("works", range);
        if let Some(path) = new_record {
            self.u3_new_anchor_open();
            self.take_for_record(vec![path], now);
        }
        if let Some(id) = open {
            self.push(Route::Work(id), now);
        }
    }

    /// The pending tab: search, what waits to go on chain (number, type, summary, when queued, state; each row
    /// opens it), and this batch as a group: its entries, its gas estimate, "put all on chain", the last batch
    /// sent and its transaction, and "sync".
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
            let cols = [table::SEQ, table::TYPE, table::col("", table::Col::Fr(1.0)), table::col("", table::Col::Px(DATE_W)), table::MARK];
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
                            table::Cell::Mono(crate::when::when(q.at)),
                            table::Cell::Mark(lamp_mark(lamp)),
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
        // This batch: the entries the key sends (the sendable ones), their gas estimate, "put all on chain"; the last
        // batch sent and its transaction; "sync".
        let sent = self.shell.sent.clone();
        let mut copied = None;
        stagger(ui, 2, |ui| {
            card::section(ui, t(Key::U3BatchLabel), "", |ui| {
                card::form(ui, |ui, f| {
                    let can = self.shell.queue.sendable();
                    let say = match self.shell.gas {
                        Some((n, g)) if n == can && n > 0 => fill2(Key::U3BatchEstimate, &n.to_string(), &g.to_string()),
                        _ => fill1(Key::U3BatchNoEstimate, &can.to_string()),
                    };
                    if f.act(ui, &say, "", key::Key::new(t(Key::U3SendAll), Role::Guide).enabled(can > 0)).clicked() {
                        confirm = Some(U3Confirm::Send { count: can });
                    }
                    if let Some(Done::Anchored { tx, confirmed, sent, dropped, chain, .. }) = sent.clone() {
                        let said = fill2(Key::U3SentSay, &sent.to_string(), &dropped.to_string());
                        let _ = f.info(ui, t(Key::U3LastSend), card::Show::Mark(if confirmed { Mark::Ok } else { Mark::Busy }, &said));
                        copied = f.info(ui, t(Key::SendTx), card::Show::Copy(&format!("{chain} \u{b7} {tx}")));
                    }
                    sync = f.act(ui, t(Key::NavRefresh), "", key::Key::new(t(Key::NavRefresh), Role::Secondary)).clicked();
                });
            });
        });
        tell_copied(ui.ctx(), copied);
        // Say plainly when resuming fails: a submitted batch is waiting for its receipt and this chain has no
        // node now.
        if let Some(chain) = self.shell.resume_blocked {
            states::okline(ui, Mark::Warn, &fill1(Key::QueueResumeNoNode, &chain.to_string()));
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
        // Its position among the sendable entries (a submitted one is not among them).
        let queued_at = if w.lamp == crate::ledgerx::Lamp::Submitted {
            None
        } else {
            self.shell.queue.items.iter().filter(|q| !q.step.in_flight()).position(|q| q.id == w.id)
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
            card::section(ui, t(Key::BasicInfo), "", |ui| {
                card::form(ui, |ui, f| {
                    anchor_rows(ui, f, w.lamp, w.row.anchored_at, at);
                    let _ = f.info(ui, t(Key::V2RecordNo), card::Show::Mono(&format!("#{}", w.seq)));
                    if !files.is_empty() {
                        let _ = f.info(ui, t(Key::U3File), card::Show::Text(&files.join(", ")));
                    }
                });
            });
        });
        if w.lamp == crate::ledgerx::Lamp::Submitted {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
        }
        if w.deleted.is_none() {
            // Granting waits for the chain: until anchored the key is off and says why on hover (verified last pass
            // but not yet this pass: "waiting for this check", never telling the user to anchor again).
            let anchored = queued_at.is_none() && w.lamp == crate::ledgerx::Lamp::Anchored;
            let remembered = matches!(w.lamp, crate::ledgerx::Lamp::Remembered | crate::ledgerx::Lamp::RememberedStale);
            let why = if anchored { "" } else { t(if queued_at.is_none() && remembered { Key::V2AwaitThisCheck } else { Key::V2AnchorFirstHint }) };
            let offer = DetailActs { chain: queued_at.map(|i| Some(i + 1)), kit: true, grant: Some((anchored, why, Key::V2GrantToOthers)), annotate: false };
            stagger(ui, 3, |ui| {
                let hit = detail_acts(ui, &offer);
                if let Some(n) = hit.send {
                    confirm = Some(U3Confirm::Send { count: n });
                }
                grant = hit.grant;
                kit = hit.kit;
            });
        }
        stagger(ui, 4, |ui| {
            let mut rows = vec![
                (t(Key::ContentHash), Val::mono(w.work.clone())),
                (t(Key::DetailPick), Val::mono(w.id.clone())),
                (t(Key::EntryPrev), w.row.prev.clone().map(Val::mono).unwrap_or_else(|| Val::text(t(Key::None_)))),
                (t(Key::CanonBytes), Val::figure(w.row.bytes.to_string())),
            ];
            if let Some((chain, tx)) = w.row.tx.clone() {
                rows.push((t(Key::AnchorTx), Val::mono(format!("{chain} \u{b7} {tx}"))));
            }
            details_card(ui, "work-details", &rows);
        });
        // Deleting: a red row, confirmed in its sheet.
        if w.deleted.is_none() {
            stagger(ui, 5, |ui| {
                card::form(ui, |ui, f| {
                    if f.danger(ui, t(Key::V2DeleteWork)).clicked() {
                        confirm = Some(U3Confirm::Retract { subject: w.id.clone() });
                    }
                });
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
        let pos = items.iter().filter(|x| !x.step.in_flight()).position(|x| x.id == q.id);
        let (tag, summary) = row.as_ref().map(|x| {
            let f = row_face(&rows_all, &reading, x);
            (f.0.to_string(), f.1)
        })
        .unwrap_or_else(|| (t(Key::None_).to_string(), t(Key::UnnamedRecord).to_string()));
        let lamp = row.as_ref().map(|x| x.lamp).unwrap_or(crate::ledgerx::Lamp::Queued);
        let at = self.shell.remembered.as_ref().map(|r| r.at);
        let mut confirm = None;
        stagger(ui, 0, |ui| {
            let sub = row.as_ref().map(|x| format!("#{} \u{b7} {tag}", x.seq)).unwrap_or_else(|| tag.clone());
            card::hero(ui, &summary, &sub, false, |ui| lamp_pill_ui(ui, lamp, at));
        });
        // Basic information: where it stands on chain and its number, as on any entry's page; then when it was
        // queued, and "put on chain now" in the row that says which batch it goes with.
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::BasicInfo), "", |ui| {
                card::form(ui, |ui, f| {
                    anchor_rows(ui, f, lamp, row.as_ref().and_then(|r| r.anchored_at), at);
                    if let Some(r) = row.as_ref() {
                        let _ = f.info(ui, t(Key::V2RecordNo), card::Show::Mono(&format!("#{}", r.seq)));
                    }
                    let _ = f.info(ui, t(Key::U3QueuedAt), card::Show::Mono(&crate::when::when(q.at)));
                    match pos {
                        Some(k) => {
                            if f.act(ui, t(Key::U3SendsWith), &fill1(Key::U3FirstN, &(k + 1).to_string()), key::Key::new(t(Key::U3SendNow), Role::Guide)).clicked() {
                                confirm = Some(U3Confirm::Send { count: k + 1 });
                            }
                        }
                        None => {
                            let _ = f.info(ui, t(Key::U3SendsWith), card::Show::Text(t(Key::U3Submitted)));
                        }
                    }
                });
            });
        });
        // The raw values, as on an entry's own detail page.
        let mut raw = Vec::new();
        if let Some(w) = row.as_ref().and_then(|r| r.work.clone()) {
            raw.push((t(Key::ContentHash), Val::mono(w)));
        }
        raw.push((t(Key::DetailPick), Val::mono(q.id.clone())));
        if let Some(r) = row.as_ref() {
            raw.push((t(Key::EntryPrev), r.prev.clone().map(Val::mono).unwrap_or_else(|| Val::text(t(Key::None_)))));
            raw.push((t(Key::CanonBytes), Val::figure(r.bytes.to_string())));
        }
        stagger(ui, 3, |ui| details_card(ui, "pending-details", &raw));
        if let Some(cf) = confirm {
            self.u3_open_confirm(cf);
        }
    }
}

/// What a detail page's "actions" group offers: putting it on chain (`Some(Some(n))` sends the first n of the
/// pending entries, `Some(None)` adds it to them first), exporting a record kit, granting (whether it can now,
/// and why not on hover), adding a note.
pub(super) struct DetailActs<'a> {
    pub(super) chain: Option<Option<usize>>,
    pub(super) kit: bool,
    /// Granting: whether it can be taken now, why not, and the page's own word for it.
    pub(super) grant: Option<(bool, &'a str, Key)>,
    pub(super) annotate: bool,
}

/// What was pressed in a detail page's "actions" group.
#[derive(Default)]
pub(super) struct DetailHit {
    pub(super) send: Option<usize>,
    pub(super) queue: bool,
    pub(super) kit: bool,
    pub(super) grant: bool,
    pub(super) annotate: bool,
}

/// A detail page's "actions" group: one navigation row per action, each named once (a record and a ledger
/// entry name the same action alike); the whole row opens its page or sheet (an action that cannot be taken yet
/// is dimmed and says why on hover). Nothing when there is nothing to offer.
pub(super) fn detail_acts(ui: &mut egui::Ui, a: &DetailActs) -> DetailHit {
    let mut hit = DetailHit::default();
    if a.chain.is_none() && !a.kit && a.grant.is_none() && !a.annotate {
        return hit;
    }
    card::section(ui, t(Key::RecordActs), "", |ui| {
        card::form(ui, |ui, f| {
            match a.chain {
                Some(Some(n)) => {
                    if f.go(ui, t(Key::U3SendNow), "", true, "").clicked() {
                        hit.send = Some(n);
                    }
                }
                Some(None) => hit.queue = f.go(ui, t(Key::U3QueueIt), "", true, "").clicked(),
                None => {}
            }
            if let Some((on, why, word)) = a.grant {
                hit.grant = f.go(ui, t(word), "", on, why).clicked();
            }
            if a.kit {
                hit.kit = f.go(ui, t(Key::U3UseForKit), "", true, "").clicked();
            }
            if a.annotate {
                hit.annotate = f.go(ui, t(Key::U3AnnotateGo), "", true, "").clicked();
            }
        });
    });
    hit
}

/// The anchoring state of a detail page's basic information: confirmed on chain, a check and the first anchor's
/// block time in one row; otherwise the state's pill, and the first anchor's time in a row of its own when the
/// chain was read for it.
pub(super) fn anchor_rows(ui: &mut egui::Ui, f: &mut card::Form, lamp: crate::ledgerx::Lamp, anchored_at: Option<u64>, at: Option<u64>) {
    match anchored_at {
        Some(a) if lamp == crate::ledgerx::Lamp::Anchored => {
            let when = crate::when::when(a);
            let _ = f.line(ui, None, t(Key::NavAnchoring), "", false, false, c(C::Ink), |ui| {
                ui.label(egui::RichText::new(&when).font(Type::MonoSmall.font()).color(c(C::Ink)));
                mark::mark(ui, Mark::Ok);
            });
        }
        _ => {
            let (tone, _) = lamp_pill(lamp);
            let _ = f.info(ui, t(Key::NavAnchoring), card::Show::Pill(&lamp_label(lamp, true, at), tone));
            if let Some(a) = anchored_at {
                let _ = f.info(ui, t(Key::U4RecordFirstAt), card::Show::Mono(&crate::when::when(a)));
            }
        }
    }
}
