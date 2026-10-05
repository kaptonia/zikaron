use super::*;

/// Wei read as ETH for an upper bound (a fee cap, what a send needs): the last digit shown rounds up, so a cap
/// is never said smaller than it is. Display only, never for a decision.
pub(super) fn eth_cap(wei: u128) -> String {
    eth_digits(wei, true)
}

/// Wei read as ETH for what is held (a balance, what there is): the last digit shown rounds down, so a balance
/// is never said larger than it is. Display only, never for a decision.
pub(super) fn eth_held(wei: u128) -> String {
    eth_digits(wei, false)
}

/// The one rule for both: as many decimals as the larger of four and what reaching the third significant digit
/// takes (never past wei, 18), the dropped rest rounding up or down as asked, then trailing zeros trimmed while
/// more than four remain. Exactly zero is "0": a non-zero amount never reads as all zeros, however small.
fn eth_digits(wei: u128, up: bool) -> String {
    const WEI_DECIMALS: u32 = 18;
    if wei == 0 {
        return "0".to_string();
    }
    let digits = wei.ilog10() + 1;
    // Position after the point of the first significant digit (it is in the whole part when `digits > 18`).
    let first = (WEI_DECIMALS + 1).saturating_sub(digits);
    let places = 4.max(first + 2).min(WEI_DECIMALS);
    let step = 10u128.pow(WEI_DECIMALS - places);
    let mut scaled = wei / step;
    if up && wei % step != 0 {
        scaled += 1;
    }
    let scale = 10u128.pow(places);
    let mut frac = format!("{:0width$}", scaled % scale, width = places as usize);
    while frac.len() > 4 && frac.ends_with('0') {
        frac.pop();
    }
    format!("{}.{}", scaled / scale, frac)
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
        let a = crate::lastread::issuer_form(author);
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

#[cfg(test)]
mod amounts {
    use super::{eth_cap, eth_held};

    const ETH: u128 = 1_000_000_000_000_000_000;
    const GWEI: u128 = 1_000_000_000;

    #[test]
    fn zero_is_zero() {
        assert_eq!(eth_cap(0), "0");
        assert_eq!(eth_held(0), "0");
    }

    #[test]
    fn one_wei_is_not_zero() {
        assert_eq!(eth_cap(1), "0.000000000000000001");
        assert_eq!(eth_held(1), "0.000000000000000001");
    }

    #[test]
    fn exactly_at_the_fourth_decimal() {
        assert_eq!(eth_cap(ETH / 10_000), "0.0001");
        assert_eq!(eth_held(ETH / 10_000), "0.0001");
    }

    #[test]
    fn six_gwei() {
        assert_eq!(eth_cap(6 * GWEI), "0.000000006");
        assert_eq!(eth_held(6 * GWEI), "0.000000006");
    }

    #[test]
    fn small_amounts_keep_three_significant_digits() {
        assert_eq!(eth_cap(94_500_000_000_000), "0.0000945");
        assert_eq!(eth_held(94_500_000_000_000), "0.0000945");
        assert_eq!(eth_cap(420_000_000_000_000), "0.00042");
        assert_eq!(eth_held(420_000_000_000_000), "0.00042");
        // A cap like 200,000 gas at a low fee: past the third digit, up for the cap, down for a balance.
        assert_eq!(eth_cap(94_512_345_678_901), "0.0000946");
        assert_eq!(eth_held(94_512_345_678_901), "0.0000945");
    }

    #[test]
    fn whole_amounts_keep_four_decimals() {
        assert_eq!(eth_cap(2 * ETH), "2.0000");
        assert_eq!(eth_held(2 * ETH), "2.0000");
        assert_eq!(eth_cap(2 * ETH + 1), "2.0001");
        assert_eq!(eth_held(2 * ETH + 1), "2.0000");
        assert_eq!(eth_held(12_345_678_900_000_000_000), "12.3456");
        assert_eq!(eth_cap(12_345_678_900_000_000_000), "12.3457");
    }

    #[test]
    fn the_top_of_u128() {
        assert_eq!(eth_cap(u128::MAX), "340282366920938463463.3747");
        assert_eq!(eth_held(u128::MAX), "340282366920938463463.3746");
    }

    #[test]
    fn rounding_up_carries_across_places() {
        // 0.00009995: up carries through every nine to 0.0001000, trimmed to four places; down keeps it.
        assert_eq!(eth_cap(99_950_000_000_000), "0.0001");
        assert_eq!(eth_held(99_950_000_000_000), "0.0000999");
        assert_eq!(eth_cap(999_999_999_999_999_999), "1.0000");
        assert_eq!(eth_held(999_999_999_999_999_999), "0.9999");
    }
}
