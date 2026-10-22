use super::*;

/// Height of the home page's top row (the drop zone and the card beside it).
const TOP_H: f32 = 150.0;

/// The least height of the grantee's drop zone: its row is as tall as the check card beside it.
const GRANTEE_TOP_H: f32 = 110.0;

/// The home pages' top row: the first block takes 1.25 parts of the row and the second one part, 16 apart;
/// stacked when the second would be narrower than `HOME_PAIR_MIN`.
fn home_pair(ui: &mut egui::Ui, draw: impl FnMut(&mut egui::Ui, card::Side)) {
    let w = ui.available_width();
    let first = ((w - tk::CARD_GAP) * 1.25 / 2.25).floor();
    card::form_and_result(ui, first, HOME_PAIR_MIN, draw);
}

/// The narrowest the second block of the home pages' top row lies beside the first.
const HOME_PAIR_MIN: f32 = 270.0;

/// The least height of a home page's figure tile.
const TILE_H: f32 = 122.0;

/// The home pages' figure tiles sit a card gap under the top row.
fn tiles_close_up(ui: &mut egui::Ui) {
    ui.add_space(tk::CARD_GAP - ui.spacing().item_spacing.y);
}

/// A home page's figure tile, `TILE_H` high at least: its name (small, second ink), 6 under it the figure
/// (22, semibold, a mark before it when given), and the small line under that in the third ink; 18 × 20
/// inside. It lifts a point on hover and presses to 0.99.
fn home_tile(ui: &mut egui::Ui, tile: &card::Tile) -> egui::Response {
    const PAD: egui::Vec2 = egui::vec2(20.0, 18.0);
    const FIG_LINE: f32 = 28.0;
    let w = ui.available_width();
    let content = Type::Small.line() + 6.0 + FIG_LINE + if tile.sub.is_empty() { 0.0 } else { 6.0 + Type::Small.line() };
    let h = (content + PAD.y * 2.0).max(TILE_H).max(zikaron_ui::grid::cell_height(ui));
    let sense = if tile.clickable { egui::Sense::click() } else { egui::Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, h), sense);
    let st = motion::states(ui.ctx(), resp.id, resp.hovered(), resp.is_pointer_button_down_on(), resp.has_focus(), tile.clickable);
    let lift = if tile.clickable { st.hover } else { 0.0 };
    let press = 1.0 - 0.01 * if tile.clickable { st.press } else { 0.0 };
    let r = paint::scaled(rect.translate(egui::vec2(0.0, -lift)), press);
    let p = ui.painter();
    zikaron_ui::material::paint(p, r, tk::Radius::Card, zikaron_ui::material::Level::Card, lift);
    let inner = r.shrink2(PAD);
    // The lines centred in the tile: a tile made taller than its lines keeps as much above them as below.
    let y0 = inner.top() + ((inner.height() - content) / 2.0).max(0.0);
    let title = width::elide_to(ui, tile.title, Type::Small.font(), inner.width());
    p.text(egui::pos2(inner.left(), y0), egui::Align2::LEFT_TOP, &title, Type::Small.font(), c(C::Ink2));
    let fy = y0 + Type::Small.line() + 6.0;
    let mut x = inner.left();
    if let Some(m) = tile.mark {
        mark::paint_mark(ui.ctx(), p, egui::pos2(x + 9.0, fy + FIG_LINE / 2.0), m, tk::MARK);
        x += 18.0 + 8.0;
    }
    let font = if tile.mono_figure { Type::Mono.font() } else { Type::Figure.font() };
    let fit = width::elide_to(ui, tile.figure, font.clone(), (inner.right() - x).max(0.0));
    let line = egui::Rect::from_min_max(egui::pos2(x, fy), egui::pos2(inner.right(), fy + FIG_LINE));
    paint::rolling(ui, resp.id.with("figure"), line, &fit, font, tile.figure_colour.unwrap_or(c(C::Ink)));
    if !tile.sub.is_empty() {
        let fit = width::elide_to(ui, tile.sub, Type::Small.font(), inner.width());
        p.text(egui::pos2(inner.left(), fy + FIG_LINE + 6.0), egui::Align2::LEFT_TOP, fit, Type::Small.font(), c(C::Ink3));
    }
    zikaron_ui::probe::tile(ui.ctx(), rect, title == tile.title);
    if tile.clickable {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

impl Win {
    pub(super) fn page_home(&mut self, ui: &mut egui::Ui, now: f64) {
        self.home_fetch(ui, now);
        match self.shell.settings.role {
            crate::roles::Role::Author => self.home_author(ui, now),
            crate::roles::Role::Grantee => self.home_grantee(ui, now),
        }
    }

    /// When this home is behind the chain (the exit gate found anchors it lacks): one banner with its key, leading
    /// to the fetch form.
    fn home_fetch(&mut self, ui: &mut egui::Ui, now: f64) {
        // While a conflict waits on its confirmation sheet (`conflict_sheet`), the fetch banner stays hidden.
        if self.shell.fetch_conflict.is_some() {
            return;
        }
        let Some(crate::restorex::State::NewerElsewhere { missing }) = self.shell.unfetched else { return };
        let go = states::banner(ui, states::Banner::Bad, t(Key::FetchCardTitle), |ui| key::key(ui, t(Key::DoFetchLedger), Role::Secondary, true).clicked());
        zikaron_ui::callout::callout(ui, &fill1(Key::FetchCardSay, &missing.to_string()));
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
            home_pair(ui, |ui, side| match side {
                card::Side::Main => {
                    let d = drop::zone(ui, "home-drop", Some(Glyph::Inbox), &[t(Key::WbDropTitle), t(Key::DropClickAny)], None, TOP_H, drop::Shape::Column, true);
                    new_record = self.drop_or_pick(ui.ctx(), "home-drop-1", &d, crate::platform::Pick::FileOrFolder, now);
                }
                card::Side::Side => {
                    if let Some(n) = self.send_card(ui) {
                        send = Some(n);
                    }
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
        tiles_close_up(ui);
        stagger(ui, 1, |ui| {
            card::grid3(ui, "home-tiles", 3, |ui, i| {
                let tile = match i {
                    0 => card::Tile { title: t(Key::WbAudit), figure: &audit_fig, mark: audit_mark, figure_colour: None, sub: &audit_sub, mono_figure: false, clickable: true },
                    1 => card::Tile { title: t(Key::NavWorksView), figure: &works_fig, mark: None, figure_colour: None, sub: &works_sub, mono_figure: false, clickable: true },
                    _ => card::Tile { title: t(Key::WbExpiring), figure: &exp_fig, mark: None, figure_colour: None, sub: &exp_sub, mono_figure: false, clickable: true },
                };
                if home_tile(ui, &tile).clicked() {
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

    /// The home page's "to be anchored" card: its name, the count, and how it stands at its foot. Waiting
    /// entries: the whole card is the key (nothing more is said); in flight: a spinner (a warning when no node
    /// can be asked for the receipt); all anchored: a check. Returns the batch to send when clicked.
    fn send_card(&mut self, ui: &mut egui::Ui) -> Option<usize> {
        let sendable = self.shell.queue.sendable();
        let busy = self.shell.queue.len().saturating_sub(sendable);
        let state = if sendable > 0 { 0 } else if busy > 0 { 1 } else { 2 };
        let n = if state == 1 { busy } else { sendable };
        let foot = match state {
            // Waiting: the card is the key, and says so.
            0 => Some((None, t(Key::SendClickToAnchor))),
            // In flight: spin, unless no node can be asked for the receipt (the queue page says why).
            1 if self.shell.resume_blocked.is_none() => Some((Some(Mark::Busy), t(Key::LampSubmitted))),
            1 => Some((Some(Mark::Warn), t(Key::LampSubmitted))),
            _ => Some((Some(Mark::Ok), t(Key::SendAllDone))),
        };
        let figure = fill1(Key::U3EntriesCount, &n.to_string());
        let clicked = zikaron_ui::figcard::figure_card(ui, t(Key::ItemUnanchored), &figure, foot, TOP_H, state == 0).clicked();
        (state == 0 && clicked).then_some(sendable)
    }

    fn home_grantee(&mut self, ui: &mut egui::Ui, now: f64) {
        let mut to: Option<Place> = None;
        let mut check = false;
        let mut held: Option<String> = None;
        self.ensure_held(now);
        // The check card (the grant code, "start check" at its foot) and, as tall as it, the record kit's drop zone.
        let mut card_h = GRANTEE_TOP_H;
        stagger(ui, 0, |ui| {
            home_pair(ui, |ui, side| match side {
                card::Side::Main => {
                    let (can, phase) = (!self.typed.ck_typed.trim().is_empty(), self.phase_of(crate::task::Kind::Check));
                    let top = ui.cursor().min.y;
                    fields::card(ui, |ui, st| {
                        st.field(ui, t(Key::WbCheckTitle), None, None, |ui, _| input::area_hint(ui, &mut self.typed.ck_typed, 2, t(Key::WbCheckHint)));
                        st.foot(ui, |ui| check |= key::show(ui, key::Key::new(t(Key::DoCheck), Role::Primary).enabled(can).phase(phase)).clicked());
                    });
                    card_h = (ui.min_rect().bottom() - top).max(GRANTEE_TOP_H);
                }
                card::Side::Side => {
                    let d = drop::zone(ui, "home-verify", Some(Glyph::Inbox), &[t(Key::WbVerifyDrop), t(Key::DropClickAny)], None, card_h, drop::Shape::Column, true);
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
        let (received, received_mark) = match self.shell.checked.as_ref().map(|x| &x.file) {
            Some(crate::checkx::Side::Compared(d)) if d.matched() => (t(Key::DeliveryMatch).to_string(), Some(Mark::Ok)),
            Some(crate::checkx::Side::Compared(_)) => (t(Key::DeliveryMismatch).to_string(), Some(Mark::Bad)),
            _ => (dash.clone(), None),
        };
        tiles_close_up(ui);
        stagger(ui, 1, |ui| {
            card::grid3(ui, "home-tiles", 3, |ui, i| {
                let tile = match i {
                    0 => card::Tile { title: t(Key::WbVault), figure: &need, mark: None, figure_colour: need_red.then(|| c(C::BadInk)), sub: &need_sub, mono_figure: false, clickable: true },
                    1 => card::Tile { title: t(Key::U4HeldLive), figure: &live, mark: None, figure_colour: None, sub: &live_sub, mono_figure: false, clickable: true },
                    _ => card::Tile { title: t(Key::PageDelivery), figure: &received, mark: received_mark, figure_colour: None, sub: "", mono_figure: false, clickable: true },
                };
                if home_tile(ui, &tile).clicked() {
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
            // The check page's own summary (grant, file and terms together).
            let (k, m) = match crate::checkx::summary(x) {
                crate::checkx::Summary::AllMatch => (Key::U3CheckAllPass, Mark::Ok),
                crate::checkx::Summary::Mismatched(_) => (Key::U3CheckFail, Mark::Bad),
                crate::checkx::Summary::Incomplete => (Key::U3CheckSomeMissing, Mark::Warn),
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
            // The verify page's own head.
            let ok = super::verify::verified_head(v).1 == Mark::Ok;
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
