use super::*;

struct AlertLine {
    mark: Mark,
    title: String,
    said: String,
    go: Option<Place>,
    to: Option<Place>,
    item: Option<crate::watchx::Item>,
}

impl Win {
    pub(super) fn banner(&mut self, ui: &mut egui::Ui, now: f64) {
        if let Some(v) = self.shell.old_view.clone() {
            let back = states::banner(ui, states::Banner::Bad, &fill1(Key::OldDataBar, &crate::when::day(v.at)), |ui| key::key(ui, t(Key::OldDataBack), Role::Secondary, true).clicked());
            if back {
                self.act(Action::LeaveOldData, now);
            }
            return;
        }
        match self.shell.banner() {
            crate::shell::Banner::None => {}
            crate::shell::Banner::Handed(_) => states::note_box(ui, t(Key::HandedBarPlain)),
            crate::shell::Banner::HandedOff => states::note_box(ui, t(Key::PenAwayBar)),
            crate::shell::Banner::Unfetched(_) if self.route() == crate::nav::Route::Section(Section::Data) => {}
            crate::shell::Banner::Unfetched(s) => {
                let title = mark_title(&s);
                let go = states::banner(ui, states::Banner::Bad, &title, |ui| key::key(ui, t(Key::NavLook), Role::Secondary, true).clicked());
                if go {
                    self.go(Place::Settings(Section::Data), now);
                }
            }
            crate::shell::Banner::Broken => {
                let go = states::banner(ui, states::Banner::Bad, t(Key::BrokenBar), |ui| key::key(ui, t(Key::DoRecover), Role::Secondary, true).clicked());
                if go {
                    self.go(Place::Settings(Section::Data), now);
                }
            }
            which @ (crate::shell::Banner::OtherMachine | crate::shell::Banner::MarkUnread) => {
                let say = if which == crate::shell::Banner::MarkUnread { Key::SetMarkUnread } else { Key::SetOtherMachine };
                let take = states::banner(ui, states::Banner::Warn, t(say), |ui| key::key(ui, t(Key::SetTakeWriter), Role::Secondary, true).clicked());
                if take {
                    self.act(Action::TakeWriter, now);
                }
            }
            crate::shell::Banner::ReadOnly(who) => {
                let why = if who.is_empty() { t(Key::HomeNoLock).to_string() } else { who };
                states::note_box(ui, &fill1(Key::ReadOnlyBar, &why));
            }
        }
    }

    pub(super) fn tick(&mut self, ctx: &egui::Context, now: f64) {
        let mut wake: Option<f64> = self.shell.follow_wake(now);
        if self.shell.home.is_some() && !self.shell.endpoints.is_empty() {
            for (every, kind) in [(self.shell.settings.audit_every, crate::task::Kind::Audit), (self.shell.settings.review_every, crate::task::Kind::Review)] {
                if every > 0 {
                    let last = self.shell.tasks.landed_at(kind).unwrap_or(self.last_tick);
                    let due = last + every as f64 - now;
                    wake = Some(wake.map_or(due, |w: f64| w.min(due)));
                }
            }
            if let Some(at) = self.shell.audit_lag_at().filter(|_| !self.shell.tasks.in_flight(crate::task::Kind::Audit)) {
                let due = at - now;
                wake = Some(wake.map_or(due, |w: f64| w.min(due)));
            }
        }
        if let Some(d) = wake {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(d.max(0.5)));
        }
        if self.shell.rekeying || self.shell.swapping {
            return;
        }
        if self.shell.review_due(now, self.last_tick) {
            apply(&mut self.shell, Action::ReviewVault);
        }
        if self.shell.take_tail_due() {
            apply(&mut self.shell, Action::CheckTail);
        }
        let (stale, due, lags) = (self.shell.audit_stale(), self.shell.audit_due(now, self.last_tick), self.shell.audit_lags(now));
        if !stale && !due && !lags {
            return;
        }
        self.last_tick = now;
        if stale && !due && !lags && self.own_recount() {
            return;
        }
        apply(&mut self.shell, Action::Audit);
    }

    fn alert_lines(&self) -> Vec<AlertLine> {
        let role = self.shell.settings.role;
        let mut out: Vec<AlertLine> = Vec::new();
        let mut above: Option<(String, Option<Place>)> = None;
        let rows = self.ux.watch_rows.clone().unwrap_or_else(|| self.shell.watch_rows());
        for row in crate::watchx::needing_attention(rows) {
            let said = match (row.gap, row.item) {
                (Some(g), _) => gap_say(g),
                (None, crate::watchx::Item::AuditLabel) => label_human(&row.detail),
                (None, _) => String::new(),
            };
            let go = row.gap.map(|g| gap_place(g, role));
            let same = (said.clone(), go);
            if row.item == crate::watchx::Item::QueueBacklog && above.as_ref() == Some(&same) {
                continue;
            }
            above = Some(same);
            let to = row.gap.map(gap_target);
            out.push(AlertLine { mark: light_mark(row.light), title: t(item_key(row.item)).to_string(), said, go, to, item: Some(row.item) });
        }
        if let Some(ids) = self.shell.audit_lag_stopped() {
            let mut named: Vec<String> = ids.iter().take(3).map(|id| format!("{}…", &id[..id.len().min(10)])).collect();
            if ids.len() > 3 {
                named.push(format!("+{}", ids.len() - 3));
            }
            let key = if self.shell.settings.audit_every > 0 { Key::U3LagStopped } else { Key::U3LagStoppedHand };
            out.push(AlertLine { mark: Mark::Warn, title: t(Key::ItemAuditLabel).to_string(), said: fill1(key, &named.join(t(Key::ListJoin))), go: None, to: None, item: None });
        }
        for a in &self.shell.alarms {
            let said = match a.kind {
                crate::sentinelx::Kind::Revoked => fill1(Key::AlarmRevokedOf, &self.held_record_name(&a.grant)),
                crate::sentinelx::Kind::Handed => t(Key::AlarmHandedPlain).to_string(),
            };
            let go = Some(crate::nav::home_of(Page::Vault, role));
            out.push(AlertLine { mark: Mark::Bad, title: t(Key::PageSentinel).to_string(), said, go, to: go, item: None });
        }
        out
    }

    pub(super) fn alert_count(&self) -> usize {
        self.alert_lines().len()
    }

    pub(super) fn alerts_page(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
        let lines = self.alert_lines();
        let can_send = self.send_all_ready();
        let mut go: Option<Place> = None;
        let mut send = false;
        stagger(ui, 0, |ui| {
            if lines.is_empty() {
                card::card_pad(ui, egui::vec2(ALL_CLEAR_PAD_X, ALL_CLEAR_PAD_Y), |ui| all_clear(ui, t(Key::AlertsNone)));
                return;
            }
            card::form(ui, |ui, f| {
                for l in &lines {
                    let sends = matches!(l.item, Some(crate::watchx::Item::Unanchored | crate::watchx::Item::QueueBacklog)) && can_send;
                    if sends {
                        let (_, hit) = f.line(ui, Some(card::Lead::Mark(l.mark)), &l.title, &l.said, false, false, c(C::Ink), |ui| key::key(ui, t(Key::U3SendAll), Role::Secondary, true).clicked());
                        send |= hit;
                    } else if f.big(ui, Some(l.mark), &l.title, &l.said, l.go.map(|_| "")).clicked() {
                        go = l.to;
                    }
                }
            });
        });
        stagger(ui, 1, |ui| self.last_batch_card(ui, false));
        if let Some((k, detail)) = self.shell.status.clone() {
            stagger(ui, 1, |ui| {
                card::form(ui, |ui, f| {
                    let _ = f.fold(ui, "watch-status", t(Key::SetEvidence), "", |ui, f| f.note(ui, &fill1(k, &detail)));
                });
            });
        }
        if send {
            self.send_all_unanchored(now);
        }
        if let Some(p) = go {
            self.go(p, now);
        }
    }
}

const ALL_CLEAR_PAD_X: f32 = 20.0;
const ALL_CLEAR_PAD_Y: f32 = 44.0;

fn all_clear(ui: &mut egui::Ui, s: &str) {
    const MARK: f32 = 40.0;
    let w = ui.available_width();
    let text = paint::galley(ui, s, Type::Body, c(C::Ink2));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, MARK + 10.0 + text.size().y), egui::Sense::hover());
    mark::paint_mark(ui.ctx(), ui.painter(), egui::pos2(rect.center().x, rect.top() + MARK / 2.0), Mark::Ok, MARK);
    ui.painter().galley(egui::pos2(rect.center().x - text.size().x / 2.0, rect.top() + MARK + 10.0), text, c(C::Ink2));
}

pub(super) fn mark_title(s: &crate::restorex::State) -> String {
    match s {
        crate::restorex::State::Unfetched => t(Key::FetchLedgerTitle).to_string(),
        crate::restorex::State::NewerElsewhere { missing, .. } => fill1(Key::FetchNewerTitle, &missing.to_string()),
    }
}

impl Win {
    pub(super) fn marked_title(&self) -> Option<String> {
        match self.shell.set_up_seat() {
            Some(crate::shell::SetUp::Fetch) => self.shell.unfetched.as_ref().map(mark_title),
            _ => None,
        }
    }

    pub(super) fn genesis_offered(&mut self) {
        if self.ux.wizard.is_none() && self.shell.take_unwritten_due() {
            crate::action::ask_unwritten(&mut self.shell);
        }
    }
}
