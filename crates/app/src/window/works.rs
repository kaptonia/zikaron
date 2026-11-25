use super::*;

pub(super) const DATE_W: f32 = 200.0;

const STATE_W: f32 = 160.0;

const DROP_ROW_H: f32 = 84.0;

impl Win {
    pub(super) fn work_lines(&self) -> Vec<WorkLine> {
        let chain_now = self.shell.chain_now();
        let Some((rows, _)) = self.shell.rows.as_ref() else { return Vec::new() };
        let reading = crate::retractx::read(rows);
        let mut out: Vec<WorkLine> = rows
            .iter()
            .filter(|r| r.kind == zikaron_glue::book::Kind::History)
            .filter_map(|r| {
                let work = r.work.clone()?;
                let deleted = crate::retractx::deleted_by(rows, &reading, &r.id);
                Some(WorkLine { name: human_summary(r, chain_now), work, id: r.id.clone(), seq: r.seq, lamp: r.lamp, deleted, row: r.clone() })
            })
            .collect();
        out.sort_by(|a, b| b.seq.cmp(&a.seq));
        out
    }

    pub(super) fn works(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
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
        let (view, generation) = kept(ui.ctx(), self.shell.moved, "works", (&query, &range, at, hide), || {
            let works = self.work_lines();
            let listed: Listed = works
                .iter()
                .filter(|w| !(hide && w.lamp.local()))
                .filter(|w| matches(&query, &[&w.name, &w.work, &format!("#{}", w.seq), &work_state(w, at).0]))
                .filter(|w| crate::when::within(w.row.anchored_at, &range.0, &range.1))
                .map(|w| {
                    let (label, tone, live) = work_state(w, at);
                    let row = table::Row {
                        cells: vec![
                            table::Cell::Text(w.name.clone()),
                            if live { table::Cell::PillLive(label, tone) } else { table::Cell::Pill(label, tone) },
                            table::Cell::Mono(first_anchor_say(w.row.anchored_at)),
                        ],
                        click: true,
                        gone: w.deleted.is_some(),
                        on: false,
                    };
                    (row, w.id.clone())
                })
                .collect();
            (works.is_empty(), listed)
        });
        stagger(ui, 2, |ui| {
            let cols = [table::col(t(Key::U3Work), table::Col::Fr(1.0)), table::col(t(Key::NavAnchoring), table::Col::Px(STATE_W)), table::col(t(Key::U4RecordFirstAt), table::Col::Px(DATE_W))];
            let empty = t(if !read {
                Key::WbNotRead
            } else if view.0 {
                Key::V2WorksEmpty
            } else {
                Key::SearchNone
            });
            open = listed_table(ui, "works", table::Spec::new(&cols, generation).head(true).empty(empty), &view.1);
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

    pub(super) fn works_pending(&mut self, ui: &mut egui::Ui, now: f64) {
        let chain_now = self.shell.chain_now();
        self.ensure_rows(now);
        let items_none = self.shell.queue.items.is_empty();
        let mut query = self.ux.search.get("queue").cloned().unwrap_or_default();
        let mut open: Option<String> = None;
        let mut send_all = false;
        let mut sync = false;
        stagger(ui, 0, |ui| {
            let w = ui.available_width();
            input::search(ui, &mut query, t(Key::SearchQueue), w);
        });
        let (listed, generation) = kept(ui.ctx(), self.shell.moved, "queue", &query, || {
            let rows_all: &[crate::ledgerx::Row] = self.shell.rows.as_ref().map(|(r, _)| r.as_slice()).unwrap_or_default();
            let reading = crate::retractx::read(rows_all);
            self.shell
                .queue
                .items
                .iter()
                .map(|q| (q, rows_all.iter().find(|x| x.id == q.id)))
                .filter_map(|(q, row)| {
                    let face = row.map(|x| row_face(rows_all, &reading, x, chain_now));
                    let seq = row.map(|x| format!("#{}", x.seq)).unwrap_or_default();
                    if !matches(&query, &[&seq, face.as_ref().map(|f| f.0).unwrap_or(""), face.as_ref().map(|f| f.1.as_str()).unwrap_or(""), &q.id]) {
                        return None;
                    }
                    let (tag, summary) = face.map(|f| (f.0.to_string(), f.1)).unwrap_or_else(|| (t(Key::None_).to_string(), t(Key::UnnamedRecord).to_string()));
                    let lamp = row.map(|x| x.lamp).unwrap_or(crate::ledgerx::Lamp::Queued);
                    let r = table::Row {
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
                    };
                    Some((r, q.id.clone()))
                })
                .collect::<Listed>()
        });
        stagger(ui, 1, |ui| {
            let cols = [table::SEQ, table::TYPE, table::col("", table::Col::Fr(1.0)), table::col("", table::Col::Px(DATE_W)), table::MARK];
            let empty = t(if items_none { Key::U3QueueEmpty } else { Key::SearchNone });
            open = listed_table(ui, "queue", table::Spec::new(&cols, generation).empty(empty), &listed);
        });
        let mut copied = None;
        stagger(ui, 2, |ui| {
            card::section(ui, t(Key::U3BatchLabel), "", |ui| {
                card::form(ui, |ui, f| {
                    let can = self.shell.queue.sendable();
                    let say = match self.shell.gas {
                        Some((n, g)) if n == can && n > 0 => fill2(Key::U3BatchEstimate, &n.to_string(), &g.to_string()),
                        _ => fill1(Key::U3BatchNoEstimate, &can.to_string()),
                    };
                    if f.act(ui, &say, "", key::Key::new(t(Key::U3SendAll), Role::Guide).enabled(self.send_all_ready())).clicked() {
                        send_all = true;
                    }
                    copied = self.last_batch_rows(ui, f);
                    sync = f.act(ui, t(Key::NavRefresh), "", key::Key::new(t(Key::NavRefresh), Role::Secondary)).clicked();
                });
            });
        });
        tell_copied(ui.ctx(), copied);
        if let Some(chain) = self.shell.resume_blocked {
            states::okline(ui, Mark::Warn, &fill1(Key::QueueResumeNoNode, &chain.to_string()));
        }
        self.ux.search.insert("queue", query);
        if sync {
            self.refresh_chain(now);
        }
        if send_all {
            self.send_all_unanchored(now);
        }
        if let Some(id) = open {
            self.push(Route::Pending(id), now);
        }
    }

    pub(super) fn work_detail(&mut self, ui: &mut egui::Ui, id: &str, now: f64) {
        self.ensure_rows(now);
        let Some(w) = self.work_lines().into_iter().find(|w| w.id.eq_ignore_ascii_case(id)) else {
            states::empty(ui, Glyph::File, t(if self.shell.rows.is_some() { Key::DetailGone } else { Key::WbNotRead }));
            return;
        };
        let queued_at = if w.lamp == crate::ledgerx::Lamp::Submitted {
            None
        } else {
            self.shell.queue.items.iter().filter(|q| !q.step.in_flight()).position(|q| q.id == w.id)
        };
        let at = self.shell.remembered.as_ref().map(|r| r.at);
        let mut confirm: Option<U3Confirm> = None;
        let mut send_this = false;
        let mut grant = false;
        let mut kit = false;
        stagger(ui, 0, |ui| {
            let (label, tone, live) = work_state(&w, at);
            card::hero(ui, &w.name, &format!("#{} \u{b7} {}", w.seq, t(Key::TagAnchor)), w.deleted.is_some(), &[card::HeroPill { words: label.clone(), tone, live }]);
        });
        if let Some(n) = w.deleted {
            stagger(ui, 1, |ui| states::note_box(ui, &fill1(Key::V2DeletedNote, &n.to_string())));
        }
        let files = self.files_of(&w.id, now);
        stagger(ui, 2, |ui| {
            card::section(ui, t(Key::BasicInfo), "", |ui| {
                card::form(ui, |ui, f| {
                    anchor_rows(ui, f, w.lamp, w.row.anchored_at, at, crate::ledgerx::refusal_of(&w.id, &self.shell.queue));
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
            let anchored = queued_at.is_none() && w.lamp == crate::ledgerx::Lamp::Anchored;
            let remembered = matches!(w.lamp, crate::ledgerx::Lamp::Remembered | crate::ledgerx::Lamp::RememberedStale);
            let why = if anchored { "" } else { t(if queued_at.is_none() && remembered { Key::V2AwaitThisCheck } else { Key::V2AnchorFirstHint }) };
            let offer = DetailActs { chain: queued_at.map(|i| Some(i + 1)), kit: true, grant: Some((anchored, why, Key::V2GrantToOthers)), annotate: false };
            stagger(ui, 3, |ui| {
                let hit = detail_acts(ui, &offer);
                send_this |= hit.send.is_some();
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
            self.typed.pick_ids = w.id.to_ascii_lowercase();
            self.typed.pick_from.clear();
            self.typed.pick_to.clear();
            self.push(Route::Kit, now);
        }
        if let Some(cf) = confirm {
            self.u3_open_confirm(cf);
        }
        if send_this {
            self.send_this(w.id.clone(), now);
        }
    }

    pub(super) fn pending_detail(&mut self, ui: &mut egui::Ui, id: &str, now: f64) {
        let chain_now = self.shell.chain_now();
        self.ensure_rows(now);
        let items = self.shell.queue.items.clone();
        let Some(q) = items.iter().find(|q| q.id.eq_ignore_ascii_case(id)).cloned() else {
            self.entry_detail(ui, id, now);
            return;
        };
        let rows_all: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
        let reading = crate::retractx::read(&rows_all);
        let row = rows_all.iter().find(|x| x.id == q.id).cloned();
        let pos = items.iter().filter(|x| !x.step.in_flight()).position(|x| x.id == q.id);
        let (tag, summary) = row.as_ref().map(|x| {
            let f = row_face(&rows_all, &reading, x, chain_now);
            (f.0.to_string(), f.1)
        })
        .unwrap_or_else(|| (t(Key::None_).to_string(), t(Key::UnnamedRecord).to_string()));
        let lamp = row.as_ref().map(|x| x.lamp).unwrap_or(crate::ledgerx::Lamp::Queued);
        let at = self.shell.remembered.as_ref().map(|r| r.at);
        let mut send_this = false;
        stagger(ui, 0, |ui| {
            let sub = row.as_ref().map(|x| format!("#{} \u{b7} {tag}", x.seq)).unwrap_or_else(|| tag.clone());
            card::hero(ui, &summary, &sub, false, &[lamp_hero(lamp, at)]);
        });
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::BasicInfo), "", |ui| {
                card::form(ui, |ui, f| {
                    anchor_rows(ui, f, lamp, row.as_ref().and_then(|r| r.anchored_at), at, crate::ledgerx::refusal_of(&q.id, &self.shell.queue));
                    if let Some(r) = row.as_ref() {
                        let _ = f.info(ui, t(Key::V2RecordNo), card::Show::Mono(&format!("#{}", r.seq)));
                    }
                    let _ = f.info(ui, t(Key::U3QueuedAt), card::Show::Mono(&crate::when::when(q.at)));
                    match pos {
                        Some(_) => {
                            let batch = self.shell.next_batch(usize::MAX);
                            let with = match batch.iter().position(|b| b.eq_ignore_ascii_case(&q.id)) {
                                Some(k) => fill1(Key::U3FirstN, &(k + 1).to_string()),
                                None => t(Key::U3GoesLater).to_string(),
                            };
                            if f.act(ui, t(Key::U3SendsWith), &with, key::Key::new(t(Key::U3SendNow), Role::Guide)).clicked() {
                                send_this = true;
                            }
                        }
                        None => {
                            let _ = f.info(ui, t(Key::U3SendsWith), card::Show::Text(t(Key::U3Submitted)));
                        }
                    }
                });
            });
        });
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
        if send_this {
            self.send_this(q.id.clone(), now);
        }
    }
}

pub(super) struct DetailActs<'a> {
    pub(super) chain: Option<Option<usize>>,
    pub(super) kit: bool,
    pub(super) grant: Option<(bool, &'a str, Key)>,
    pub(super) annotate: bool,
}

#[derive(Default)]
pub(super) struct DetailHit {
    pub(super) send: Option<usize>,
    pub(super) queue: bool,
    pub(super) kit: bool,
    pub(super) grant: bool,
    pub(super) annotate: bool,
}

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

pub(super) fn anchor_rows(ui: &mut egui::Ui, f: &mut card::Form, lamp: crate::ledgerx::Lamp, anchored_at: Option<u64>, at: Option<u64>, refused: Option<&str>) {
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
            if let (crate::ledgerx::Lamp::Refused, Some(code)) = (lamp, refused) {
                let _ = f.info(ui, t(Key::U3NoEstimateWhy), card::Show::Text(&crate::fault::refusal_words(code)));
            }
            if let Some(a) = anchored_at {
                let _ = f.info(ui, t(Key::U4RecordFirstAt), card::Show::Mono(&crate::when::when(a)));
            }
        }
    }
}
