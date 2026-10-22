//! The read-only strip on top of every page, the self-audit and re-check timers, and the alerts page with
//! the rail's count.

use super::*;

/// One line of the alerts page: its mark, what it is, how it stands now, and where "go to" leads.
struct AlertLine {
    mark: Mark,
    title: String,
    said: String,
    /// The settled place (the link is named after it).
    go: Option<Place>,
    /// Where a click leads: the page itself, so arriving opens what the line is about (the check fold, the
    /// grouping by issuer).
    to: Option<Place>,
    /// The watched item the line reads (none for a sentinel alarm).
    item: Option<crate::watchx::Item>,
}

impl Win {
    /// The strip at the top of every page: a broken chain (read only; go restore), a ledger handed over, or
    /// not writable for another reason. Always shown, never silent.
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

    /// The self-audit and re-check timers: when due, start a background task (the frame touches no disk or
    /// network). A period of zero never runs by itself; with the basis incomplete, no node, or a task already
    /// in flight, nothing starts and nothing is said.
    pub(super) fn tick(&mut self, ctx: &egui::Context, now: f64) {
        // egui draws only on events, so each thing on a clock requests a frame for the moment it falls due, or it
        // would never fire: the look at the disk while the command line writes alone (`Shell::follow_wake`, every
        // second), and the configured self-audit and re-check periods. One wake, the earliest, asked here only.
        let mut wake: Option<f64> = self.shell.follow_wake(now);
        if self.shell.home.is_some() && !self.shell.endpoints.is_empty() {
            for (every, kind) in [(self.shell.settings.audit_every, crate::task::Kind::Audit), (self.shell.settings.review_every, crate::task::Kind::Review)] {
                if every > 0 {
                    let last = self.shell.tasks.landed_at(kind).unwrap_or(self.last_tick);
                    let due = last + every as f64 - now;
                    wake = Some(wake.map_or(due, |w: f64| w.min(due)));
                }
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
        // A marked home whose tail can be checked now (nodes back, the ledger moved): check it once.
        if self.shell.take_tail_due() {
            apply(&mut self.shell, Action::CheckTail);
        }
        // `audit_stale` covers a stale report in hand, `audit_due` the period: either one audits now.
        if !self.shell.audit_stale() && !self.shell.audit_due(now, self.last_tick) {
            return;
        }
        self.last_tick = now;
        apply(&mut self.shell, Action::Audit);
    }

    /// The lines the alerts page shows: only what needs attention — a watched item that warns or is bad; never
    /// one that is fine or not yet read, never the whole-machine backup (its state lives in settings, local data,
    /// and in the settings check). When the queue row says the same as the unanchored row above it and leads to
    /// the same place, the two merge into one line. Sentinel alarms follow, one line each.
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

    /// The rail's count: the lines on the alerts page (each one needs something done).
    pub(super) fn alert_count(&self) -> usize {
        self.alert_lines().len()
    }

    /// Alerts: a mark, the item and its state. Entries waiting to go on chain carry "put all on chain" at the
    /// line's end; every other line is clicked whole and leads where it is handled (its chevron says so).
    /// Nothing to attend to: a check and one sentence in the card.
    pub(super) fn alerts_page(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
        let lines = self.alert_lines();
        let can_send = self.shell.queue.sendable() > 0 || self.shell.rows.as_ref().is_some_and(|(r, _)| !crate::kitx::unanchored(&r.iter().collect::<Vec<_>>()).1.is_empty());
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
        // The last chain read's sentence with its code goes under details; the rail says it in plain words.
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

/// The all-clear card's padding: 20 at the sides, 44 above and below.
const ALL_CLEAR_PAD_X: f32 = 20.0;
const ALL_CLEAR_PAD_Y: f32 = 44.0;

/// The all-clear state: a large check, 10 below it one sentence in the second ink, centred.
fn all_clear(ui: &mut egui::Ui, s: &str) {
    const MARK: f32 = 40.0;
    let w = ui.available_width();
    let text = paint::galley(ui, s, Type::Body, c(C::Ink2));
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, MARK + 10.0 + text.size().y), egui::Sense::hover());
    mark::paint_mark(ui.ctx(), ui.painter(), egui::pos2(rect.center().x, rect.top() + MARK / 2.0), Mark::Ok, MARK);
    ui.painter().galley(egui::pos2(rect.center().x - text.size().x / 2.0, rect.top() + MARK + 10.0), text, c(C::Ink2));
}
