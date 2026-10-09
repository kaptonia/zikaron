use super::*;

/// Height of the home page's top row (the drop zone and the card beside it).
const TOP_H: f32 = 150.0;

impl Win {
    pub(super) fn page_home(&mut self, ui: &mut egui::Ui, now: f64) {
        self.home_fetch(ui, now);
        match self.shell.settings.role {
            crate::roles::Role::Author => self.home_author(ui, now),
            crate::roles::Role::Grantee => self.home_grantee(ui, now),
        }
    }

    /// When this home is behind the chain (the exit gate found anchors it lacks): a card leading to the fetch form.
    fn home_fetch(&mut self, ui: &mut egui::Ui, now: f64) {
        // While a conflict waits on its confirmation sheet (`conflict_sheet`), the fetch card stays hidden.
        if self.shell.fetch_conflict.is_some() {
            return;
        }
        let Some(crate::restorex::State::NewerElsewhere { missing }) = self.shell.unfetched else { return };
        let mut go = false;
        card::card(ui, |ui| {
            ui.spacing_mut().item_spacing.y = tk::S3;
            states::banner(ui, states::Banner::Bad, t(Key::FetchCardTitle), |_| ());
            hint(ui, &fill1(Key::FetchCardSay, &missing.to_string()));
            keys_row(ui, |ui| go = key::key(ui, t(Key::DoFetchLedger), Role::Secondary, true).clicked());
        });
        if go {
            self.go(Place::Settings(Section::Data), now);
        }
    }

    fn home_author(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
        let mut to: Option<Place> = None;
        let mut entry: Option<String> = None;
        let mut new_record: Option<String> = None;
        let mut send: Option<usize> = None;
        let mut open_audit = false;
        stagger(ui, 0, |ui| {
            card::grid2(ui, "home-top", 2, |ui, i| {
                if i == 0 {
                    let d = drop::zone(ui, "home-drop", Some(Glyph::Inbox), &[t(Key::WbDropTitle), t(Key::DropClickAny)], None, TOP_H, drop::Shape::Column, true);
                    new_record = self.drop_or_pick(ui.ctx(), "home-drop-1", &d, crate::platform::Pick::FileOrFolder, now);
                } else if let Some(n) = self.send_card(ui) {
                    send = Some(n);
                }
            });
        });
        // Ledger state, records, and grants expiring within 30 days. A cell without a reading shows a dash with the
        // reason under it.
        let dash = t(Key::None_).to_string();
        let total = self.shell.rows.as_ref().map(|(r, _)| r.len());
        let (audit_fig, audit_mark, audit_sub) = match self.ledger_state() {
            Some(true) => (t(Key::V2Normal).to_string(), Some(Mark::Ok), total.map(|n| fill1(Key::U3EntriesTotal, &n.to_string())).unwrap_or_default()),
            Some(false) => (t(Key::V2Abnormal).to_string(), Some(Mark::Bad), total.map(|n| fill1(Key::U3EntriesTotal, &n.to_string())).unwrap_or_default()),
            None => (dash.clone(), Some(Mark::Todo), t(Key::WbNotRead).to_string()),
        };
        let works = self.work_lines();
        let live: Vec<&WorkLine> = works.iter().filter(|w| w.deleted.is_none()).collect();
        let (works_fig, works_sub) = if self.shell.rows.is_some() {
            (live.len().to_string(), live.first().map(|w| w.name.clone()).unwrap_or_default())
        } else {
            (dash.clone(), t(Key::WbNotRead).to_string())
        };
        let (exp_fig, exp_sub) = match (self.shell.grants.as_ref(), self.chain_now()) {
            (Some(g), Some(n)) => {
                let mut soon: Vec<&crate::grantx::Row> = g.iter().filter(|r| !r.revoked && r.window.map(|(_, end)| end >= n && end <= n + 30 * 86_400).unwrap_or(false)).collect();
                soon.sort_by_key(|r| r.window.map(|(_, e)| e).unwrap_or(u64::MAX));
                let sub = soon.first().map(|r| format!("#{} \u{b7} {}", r.seq, r.window.map(|(_, e)| crate::when::day(e)).unwrap_or_default())).unwrap_or_default();
                (soon.len().to_string(), sub)
            }
            (None, _) => (dash.clone(), t(Key::WbNotRead).to_string()),
            (Some(_), None) => (dash.clone(), t(Key::WbNoChainTime).to_string()),
        };
        stagger(ui, 1, |ui| {
            card::grid3(ui, "home-tiles", 3, |ui, i| {
                let tile = match i {
                    0 => card::Tile { title: t(Key::WbAudit), figure: &audit_fig, mark: audit_mark, figure_colour: None, sub: &audit_sub, mono_figure: false, clickable: true },
                    1 => card::Tile { title: t(Key::NavWorksView), figure: &works_fig, mark: None, figure_colour: None, sub: &works_sub, mono_figure: false, clickable: true },
                    _ => card::Tile { title: t(Key::WbExpiring), figure: &exp_fig, mark: None, figure_colour: None, sub: &exp_sub, mono_figure: false, clickable: true },
                };
                if card::tile(ui, &tile).clicked() {
                    match i {
                        0 => {
                            open_audit = true;
                            to = Some(Place::View(crate::nav::View::Log, 0));
                        }
                        1 => to = Some(Place::View(crate::nav::View::Works, crate::nav::tab::WORKS_ALL)),
                        _ => to = Some(Place::View(crate::nav::View::Grants, crate::nav::tab::GRANTS_LIST)),
                    }
                }
            });
        });
        // The latest five entries (number, type, summary, state); each opens its detail in the ledger.
        stagger(ui, 2, |ui| {
            card::section(ui, t(Key::U4Recent), "", |ui| {
                let rows_all: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
                let reading = crate::retractx::read(&rows_all);
                let at = self.shell.remembered.as_ref().map(|r| r.at);
                let latest: Vec<&crate::ledgerx::Row> = rows_all.iter().take(5).collect();
                let cols = [table::SEQ, table::TYPE, table::col("", table::Col::Fr(1.0)), table::col_r("", table::Col::Px(110.0))];
                let rows: Vec<table::Row> = latest
                    .iter()
                    .map(|r| {
                        let (tag, summary, struck) = row_face(&rows_all, &reading, r);
                        let (tone, live) = lamp_pill(r.lamp);
                        let words = lamp_label(r.lamp, false, at);
                        table::Row {
                            cells: vec![table::Cell::Seq(r.seq), table::Cell::Tag(tag.to_string()), table::Cell::Text(summary), if live { table::Cell::PillLive(words, tone) } else { table::Cell::Pill(words, tone) }],
                            click: true,
                            gone: struck,
                            on: false,
                        }
                    })
                    .collect();
                let empty = t(if self.shell.rows.is_some() { Key::WbRecentNone } else { Key::WbNotRead });
                if let Some(i) = table::table(ui, "home-recent", &cols, false, &rows, empty).clicked {
                    entry = latest.get(i).map(|r| r.id.clone());
                }
            });
        });
        if let Some(path) = new_record {
            self.u3_new_anchor_open();
            self.take_for_record(vec![path], now);
        }
        if let Some(n) = send {
            self.u3_open_confirm(U3Confirm::Send { count: n });
        }
        if let Some(p) = to {
            self.go(p, now);
            if open_audit {
                self.ux.u3.audit_open = true;
            }
        }
        if let Some(id) = entry {
            self.go(Place::View(crate::nav::View::Log, 0), now);
            self.push(Route::Entry(id), now);
        }
    }

    /// The home page's "to be anchored" card; the whole card is the key. Amber edge with a count when entries
    /// wait, a spinner and no clicks while a batch is in flight, green "all anchored" when nothing waits.
    /// Returns the batch to send when clicked.
    fn send_card(&mut self, ui: &mut egui::Ui) -> Option<usize> {
        let sendable = self.shell.queue.sendable();
        let busy = self.shell.queue.len().saturating_sub(sendable);
        let state = if sendable > 0 { 0 } else if busy > 0 { 1 } else { 2 };
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, TOP_H.max(zikaron_ui::grid::cell_height(ui))), if state == 0 { egui::Sense::click() } else { egui::Sense::hover() });
        let id = resp.id;
        let hot = motion::flag(ui.ctx(), id.with("hot"), state == 0 && resp.hovered(), tk::FAST);
        let press = motion::to(ui.ctx(), id.with("press"), if state == 0 && resp.is_pointer_button_down_on() { 0.99 } else { 1.0 }, tk::FAST, motion::Curve::Ease);
        // The edge fades between amber (waiting or in flight) and green (all anchored) over 300 ms.
        let amber = motion::flag(ui.ctx(), id.with("amber"), state != 2, tk::SLOW);
        let r = paint::scaled(rect.translate(egui::vec2(0.0, -hot)), press);
        let p = ui.painter();
        paint::surface(p, r, tk::Radius::Card, c(C::Surface), if hot > 0.5 { zikaron_ui::palette::Lift::Hover } else { zikaron_ui::palette::Lift::Card });
        let edge = zikaron_ui::palette::mix(c(C::OkEdge), zikaron_ui::palette::mix(c(C::WarnEdge), c(C::Warn), hot), amber);
        p.rect_stroke(r, tk::Radius::Card.egui(), egui::Stroke::new(1.0_f32, edge), egui::StrokeKind::Outside);
        let n = if state == 1 { busy } else { sendable };
        let x = r.left() + 22.0;
        let lines = [r.top() + r.height() * 0.25, r.center().y, r.top() + r.height() * 0.75];
        p.text(egui::pos2(x, lines[0]), egui::Align2::LEFT_CENTER, t(Key::ItemUnanchored), Type::Note.font(), c(C::Ink2));
        p.text(egui::pos2(x, lines[1]), egui::Align2::LEFT_CENTER, fill1(Key::U3EntriesCount, &n.to_string()), Type::Figure.font(), c(C::Ink));
        let (words, colour) = match state {
            0 => (t(Key::SendClickToAnchor), c(C::WarnInk)),
            1 => (t(Key::LampSubmitted), c(C::WarnInk)),
            _ => (t(Key::SendAllDone), c(C::OkInk)),
        };
        let mut sx = x;
        // In flight: spin, unless no node can be asked for the receipt (the queue page says why).
        if state == 1 && self.shell.resume_blocked.is_none() {
            mark::spinner(ui.ctx(), p, egui::pos2(sx + 5.0, lines[2]), 4.25, 1.5, colour, 0.9);
            sx += 16.0;
        }
        p.text(egui::pos2(sx, lines[2]), egui::Align2::LEFT_CENTER, words, Type::Small.font(), colour);
        if state == 0 {
            let clicked = resp.clicked();
            let _ = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
            return clicked.then_some(sendable);
        }
        None
    }

    fn home_grantee(&mut self, ui: &mut egui::Ui, now: f64) {
        let mut to: Option<Place> = None;
        let mut check = false;
        let mut held: Option<String> = None;
        self.ensure_held(now);
        stagger(ui, 0, |ui| {
            card::grid2(ui, "home-top", 2, |ui, i| {
                if i == 0 {
                    card::card(ui, |ui| {
                        ui.set_min_height(TOP_H - tk::CARD_PAD_Y * 2.0);
                        ui.spacing_mut().item_spacing.y = 10.0;
                        card_title(ui, t(Key::WbCheckTitle));
                        egui::ScrollArea::vertical().id_salt("home-code").max_height(52.0).show(ui, |ui| input::area(ui, &mut self.typed.ck_typed, 2));
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = tk::S3;
                            if self.long_key(ui, t(Key::DoCheck), Role::Primary, !self.typed.ck_typed.trim().is_empty(), crate::task::Kind::Check) {
                                check = true;
                            }
                            paint::text(ui, t(Key::WbCheckHint), Type::Small, c(C::Ink3));
                        });
                    });
                } else {
                    let d = drop::zone(ui, "home-verify", Some(Glyph::Kit), &[t(Key::WbVerifyDrop), t(Key::DropClickAny)], None, TOP_H, drop::Shape::Column, true);
                    // A dropped or picked record kit or record file goes straight to record verification, which
                    // runs at once.
                    if let Some(first) = self.drop_or_pick(ui.ctx(), "home-drop-2", &d, crate::platform::Pick::FileOrFolder, now) {
                        self.typed.vf_path = first;
                        self.ux.u4.verify_autorun = true;
                        to = Some(Place::View(crate::nav::View::Verify, crate::nav::tab::VERIFY_WORK));
                    }
                }
            });
        });
        let dash = t(Key::None_).to_string();
        let green = zikaron_kit::tokens::CheckVerdict::Green.as_str();
        let (need, need_sub) = match self.shell.cards.as_ref() {
            Some((cards, _)) => (cards.iter().filter(|x| x.verdict != green).count().to_string(), String::new()),
            // Not re-checked yet this run: use the verdicts saved by the last pass, with their time.
            None if !self.shell.verdicts.is_empty() => {
                let v = &self.shell.verdicts;
                let n = v.iter().filter(|(_, x)| x.verdict != green).count();
                let at = v.iter().map(|(_, x)| x.at).min().unwrap_or(0);
                (n.to_string(), fill1(Key::LastChecked, &hhmm_of(at)))
            }
            None => (dash.clone(), t(Key::WbNotRead).to_string()),
        };
        let need_red = need.parse::<usize>().map(|n| n > 0).unwrap_or(false);
        let (live, live_sub) = match (self.shell.cards.as_ref(), self.shell.held.as_ref()) {
            (Some((cards, _)), _) => {
                let n = cards.iter().filter(|x| matches!(x.countdown, crate::vaultx::Countdown::Live { .. })).count();
                let mut a: Vec<String> = cards.iter().map(|x| x.author.to_ascii_lowercase()).collect();
                a.sort();
                a.dedup();
                (n.to_string(), fill1(Key::U4IssuersCount, &a.len().to_string()))
            }
            (None, Some(h)) => (h.len().to_string(), t(Key::U4NotChecked).to_string()),
            (None, None) => (dash.clone(), t(Key::WbNotRead).to_string()),
        };
        let (received, received_colour) = match self.shell.checked.as_ref().map(|x| &x.file) {
            Some(crate::checkx::Side::Compared(d)) if d.matched() => (t(Key::DeliveryMatch).to_string(), Some(c(C::OkInk))),
            Some(crate::checkx::Side::Compared(_)) => (t(Key::DeliveryMismatch).to_string(), Some(c(C::BadInk))),
            _ => (dash.clone(), None),
        };
        stagger(ui, 1, |ui| {
            card::grid3(ui, "home-tiles", 3, |ui, i| {
                let tile = match i {
                    0 => card::Tile { title: t(Key::WbVault), figure: &need, mark: None, figure_colour: need_red.then(|| c(C::BadInk)), sub: &need_sub, mono_figure: false, clickable: true },
                    1 => card::Tile { title: t(Key::U4HeldLive), figure: &live, mark: None, figure_colour: None, sub: &live_sub, mono_figure: false, clickable: true },
                    _ => card::Tile { title: t(Key::PageDelivery), figure: &received, mark: None, figure_colour: received_colour, sub: t(Key::U4DeliveryTileSub), mono_figure: false, clickable: true },
                };
                if card::tile(ui, &tile).clicked() {
                    to = Some(match i {
                        0 | 1 => Place::View(crate::nav::View::MyGrants, 0),
                        _ => Place::View(crate::nav::View::Verify, crate::nav::tab::VERIFY_CHECK),
                    });
                }
            });
        });
        // Recent: this run's check, record verification and revocation readings, and any relicense waiting to be
        // anchored.
        let mut recent: Vec<(Mark, String, Place, Option<String>)> = Vec::new();
        if let Some(x) = self.shell.checked.as_ref() {
            let v = x.judged.verdict.as_str();
            let (k, m) = if v == green {
                (Key::U3CheckAllPass, Mark::Ok)
            } else if v == zikaron_kit::tokens::CheckVerdict::Fail.as_str() {
                (Key::U3CheckFail, Mark::Bad)
            } else {
                (Key::U3CheckSomeMissing, Mark::Warn)
            };
            recent.push((m, fill1(Key::U4RecentChecked, t(k)), Place::View(crate::nav::View::Verify, crate::nav::tab::VERIFY_CHECK), None));
        }
        for a in &self.shell.alarms {
            let name = self.held_record_name(&a.grant);
            let k = match a.kind {
                crate::sentinelx::Kind::Revoked => Key::U4RecentRevoked,
                crate::sentinelx::Kind::Handed => Key::U4RecentHanded,
            };
            recent.push((Mark::Bad, fill1(k, &name), Place::View(crate::nav::View::MyGrants, 0), Some(a.grant.clone())));
        }
        if let Some(v) = self.shell.verified.as_ref() {
            let ok = v.mismatches.is_empty();
            recent.push((if ok { Mark::Ok } else { Mark::Bad }, fill2(Key::U4RecentVerified, &width::file_name(&v.path), t(if ok { Key::U4NoMismatch } else { Key::U3CheckFail })), Place::View(crate::nav::View::Verify, crate::nav::tab::VERIFY_WORK), None));
        }
        let queued = self.shell.queue.len();
        if queued > 0 {
            recent.push((Mark::Warn, fill1(Key::U4RecentQueued, &queued.to_string()), Place::View(crate::nav::View::Works, crate::nav::tab::WORKS_PENDING), None));
        }
        stagger(ui, 2, |ui| {
            card::section(ui, t(Key::U4Recent), "", |ui| {
                let cols = [table::MARK, table::col("", table::Col::Fr(1.0)), table::CHEV];
                let rows: Vec<table::Row> = recent.iter().map(|(m, s, _, _)| table::Row { cells: vec![table::Cell::Mark(*m), table::Cell::Text(s.clone()), table::Cell::Chev], click: true, gone: false, on: false }).collect();
                if let Some(i) = table::table(ui, "home-recent-g", &cols, false, &rows, t(Key::U4RecentNone)).clicked {
                    if let Some((_, _, p, g)) = recent.get(i) {
                        to = Some(*p);
                        held = g.clone();
                    }
                }
            });
        });
        if check {
            self.start_check(now);
            to = Some(Place::View(crate::nav::View::Verify, crate::nav::tab::VERIFY_CHECK));
        }
        if let Some(p) = to {
            self.go(p, now);
            if let Some(g) = held {
                self.push(Route::Held(g), now);
            }
        }
    }
}
