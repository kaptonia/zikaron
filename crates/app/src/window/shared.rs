use super::*;

/// Wei read as ETH with four decimals (display only, never for a decision).
pub(super) fn eth(wei: u128) -> String {
    let unit: u128 = 1_000_000_000_000_000_000;
    format!("{}.{:04}", wei / unit, (wei % unit) / 100_000_000_000_000)
}

/// Hours and minutes of a wall-clock second in the chosen zone (the rail's "last synced" line; never used
/// for a decision).
pub(super) fn hm_zone(secs: u64) -> String {
    crate::when::when(secs).chars().skip(11).take(5).collect()
}

pub(super) fn wall_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

impl Win {
    /// This seat's wizard reading now (gathered by `nav::Progress::of`, not here).
    pub(super) fn progress(&self) -> crate::nav::Progress {
        crate::nav::Progress::of(&self.shell)
    }

    /// An action the product starts itself (such as reading a table when a page opens): the same `apply`,
    /// without a toast. Troubles still go to the trouble list, readings land on the page.
    pub(super) fn auto(&mut self, a: Action, now: f64) {
        let _ = now;
        let want = match &a {
            Action::OpenEntry { id } => {
                self.opened = None;
                Some(id.clone())
            }
            _ => None,
        };
        if let Applied::Opened { .. } = apply(&mut self.shell, a) {
            self.read_detail(want.as_deref());
        }
    }

    /// Only one file per drop. With more than one, say so at once and take none.
    pub(super) fn one_drop(&mut self, d: &drop::Drop, now: f64) -> Option<String> {
        match d.dropped.as_slice() {
            [] => None,
            [one] => Some(one.clone()),
            _ => {
                self.toasts.say(t(Key::U3OneAtATime), Tone::Bad, now);
                None
            }
        }
    }

    /// The path a drop zone received this frame. One dropped file is it (several give a toast and nothing is
    /// taken); clicking the zone opens the system file dialog, allowing files or directories by `kind`, and
    /// the chosen one is it, `None` on cancel.
    pub(super) fn drop_or_pick(&mut self, d: &drop::Drop, kind: crate::platform::Pick, now: f64) -> Option<String> {
        match self.one_drop(d, now) {
            Some(p) => Some(p),
            None if d.clicked => crate::platform::choose_path(kind),
            None => None,
        }
    }

    /// Take that entry's bytes; if unreadable, it goes by name to the trouble list (never swallowed as "no
    /// details").
    pub(super) fn read_detail(&mut self, want: Option<&str>) {
        let got = match (want, self.shell.home.as_ref()) {
            (Some(w), Some(home)) => Some(crate::ledgerx::detail(home, w)),
            _ => None,
        };
        match got {
            // Once read, clear the "tried" mark: only an entry that could not be read stops retrying by itself.
            Some(Ok(d)) => {
                self.opened = Some(d);
                self.ux.u3.opened_tried = None;
            }
            Some(Err(f)) => {
                self.opened = None;
                self.shell.trouble(f);
            }
            None => {}
        }
    }

    /// The issuer's name of a held grant: the issuer ledger's statement, or "unnamed issuer".
    pub(super) fn held_issuer_name(&self, grant: &str) -> String {
        let issuer = self.shell.held.as_ref().and_then(|h| h.iter().find(|x| x.id.eq_ignore_ascii_case(grant))).map(|h| h.author.clone()).unwrap_or_default();
        self.issuer_name(&issuer)
    }

    /// An issuer named by its ledger's statement (the genesis note of that ledger, read with the grant), or
    /// "unnamed issuer".
    pub(super) fn issuer_name(&self, author: &str) -> String {
        // The person's own name for this issuer on this machine comes first.
        let a = author.trim().to_ascii_lowercase();
        if let Some((_, n)) = self.shell.settings.issuer_notes.iter().find(|(x, n)| *x == a && !n.trim().is_empty()) {
            return n.clone();
        }
        self.shell
            .cards
            .as_ref()
            .and_then(|(cards, _)| cards.iter().find(|c| c.author.eq_ignore_ascii_case(author)).and_then(|c| c.issuer_name.clone()))
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| t(Key::UnnamedIssuer).to_string())
    }
}
