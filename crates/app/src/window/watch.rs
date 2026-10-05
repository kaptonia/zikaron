//! The read-only strip on top of every page, the self-audit and re-check clocks, and the alerts page with
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
}

impl Win {
    /// The strip at the top of every page: a broken chain (read only; go restore), a ledger handed over, or
    /// not writable for another reason. Said, never silent.
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
            crate::shell::Banner::ReadOnly(who) => {
                let why = if who.is_empty() { t(Key::HomeNoLock).to_string() } else { who };
                states::note_box(ui, &fill1(Key::ReadOnlyBar, &why));
            }
        }
    }

    /// The self-audit and re-check clocks: when due, start a background task (the frame touches no disk and
    /// no network). A period of zero never runs by itself; with the basis incomplete, no node, or one in
    /// flight, nothing starts and nothing is said.
    pub(super) fn tick(&mut self, ctx: &egui::Context, now: f64) {
        // egui draws only on events: a configured clock asks for a frame at the moment it falls due, or it
        // never rings (and an idle window draws nothing in between).
        if self.shell.home.is_some() && !self.shell.endpoints.is_empty() {
            let mut wake: Option<f64> = None;
            for (every, kind) in [(self.shell.settings.audit_every, crate::task::Kind::Audit), (self.shell.settings.review_every, crate::task::Kind::Review)] {
                if every > 0 {
                    let last = self.shell.tasks.landed_at(kind).unwrap_or(self.last_tick);
                    let due = last + every as f64 - now;
                    wake = Some(wake.map_or(due, |w: f64| w.min(due)));
                }
            }
            if let Some(d) = wake {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(d.max(0.5)));
            }
        }
        if self.shell.rekeying || self.shell.swapping {
            return;
        }
        if self.shell.review_due(now, self.last_tick) {
            apply(&mut self.shell, Action::ReviewVault);
        }
        // A marked home whose tail can be checked now (nodes back, the ledger moved): checked once.
        if self.shell.take_tail_due() {
            apply(&mut self.shell, Action::CheckTail);
        }
        // `audit_stale` covers "the report in hand is stale", `audit_due` the period: either audits now.
        if !self.shell.audit_stale() && !self.shell.audit_due(now, self.last_tick) {
            return;
        }
        self.last_tick = now;
        apply(&mut self.shell, Action::Audit);
    }

    /// The lines the alerts page shows. When the queue row says the same as the unanchored row above it and
    /// leads to the same place, the two are one line. Sentinel alarms follow, one line each.
    fn alert_lines(&self) -> Vec<AlertLine> {
        let role = self.shell.settings.role;
        let mut out: Vec<AlertLine> = Vec::new();
        let mut above: Option<(String, Option<Place>)> = None;
        for row in self.shell.watch_rows() {
            let said = match (row.gap, row.item) {
                (Some(g), _) => gap_say(g),
                (None, crate::watchx::Item::AuditLabel) => label_human(&row.detail),
                (None, _) => t(Key::U3WatchAllGood).to_string(),
            };
            let go = row.gap.map(|g| gap_place(g, role));
            let same = (said.clone(), go);
            if row.item == crate::watchx::Item::QueueBacklog && above.as_ref() == Some(&same) {
                continue;
            }
            above = Some(same);
            let to = row.gap.map(gap_target);
            out.push(AlertLine { mark: light_mark(row.light), title: t(item_key(row.item)).to_string(), said, go, to });
        }
        for a in &self.shell.alarms {
            let said = match a.kind {
                crate::sentinelx::Kind::Revoked => fill1(Key::AlarmRevokedOf, &self.held_record_name(&a.grant)),
                crate::sentinelx::Kind::Handed => t(Key::AlarmHandedPlain).to_string(),
            };
            let go = Some(crate::nav::home_of(Page::Vault, role));
            out.push(AlertLine { mark: Mark::Bad, title: t(Key::PageSentinel).to_string(), said, go, to: go });
        }
        out
    }

    /// The rail's count: the lines on the alerts page that need something done.
    pub(super) fn alert_count(&self) -> usize {
        self.alert_lines().iter().filter(|l| matches!(l.mark, Mark::Warn | Mark::Bad)).count()
    }

    /// Alerts: a mark, the item and how it stands; "go to …" on the right when something needs doing, and
    /// the whole line leads there.
    pub(super) fn alerts_page(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
        let lines = self.alert_lines();
        let links: Vec<Option<String>> = lines.iter().map(|l| l.go.map(|p| fill1(Key::GoPlace, self.place_title(p)))).collect();
        let mut go: Option<Place> = None;
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                for (l, link) in lines.iter().zip(&links) {
                    if f.big(ui, Some(l.mark), &l.title, &l.said, link.as_deref()).clicked() {
                        go = l.to;
                    }
                }
            });
        });
        // The last chain read's sentence with its code goes under details; the rail says it in plain words.
        if let Some((k, detail)) = self.shell.status.clone() {
            stagger(ui, 1, |ui| {
                card::card(ui, |ui| {
                    fold::fold(ui, "watch-status", t(Key::SetEvidence), |ui| paint::text(ui, &fill1(k, &detail), Type::MonoSmall, c(C::Ink2)));
                });
            });
        }
        if let Some(p) = go {
            self.go(p, now);
        }
    }

    /// A place's name as the rail says it (links name where they land).
    pub(super) fn place_title(&self, place: Place) -> &'static str {
        use crate::nav::{tab, View};
        match crate::nav::settle(place, self.shell.settings.role) {
            Place::Home => t(Key::NavHome),
            Place::SettingsHome | Place::Settings(_) => t(Key::NavSettings),
            Place::View(View::Grants, tab::GRANTS_NEW) => t(Key::V2NewGrant),
            Place::View(View::MyGrants, tab::HELD_RELICENSE) => t(Key::U4RelicenseTitle),
            Place::View(View::Works, tab::WORKS_KIT) => t(Key::U3UseForKit),
            Place::View(v, _) => t(v.key()),
            Place::Page(p) => p.title(),
        }
    }
}
