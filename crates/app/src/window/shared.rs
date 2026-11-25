use super::*;

pub(super) fn eth_said(wei: u128) -> String {
    fill1(Key::SetGasSay, &eth_cap(wei))
}

pub(super) fn eth_exact(wei: u128) -> String {
    const WEI: u128 = 1_000_000_000_000_000_000;
    let frac = format!("{:018}", wei % WEI);
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        (wei / WEI).to_string()
    } else {
        format!("{}.{frac}", wei / WEI)
    }
}

pub(super) fn held_amount(wei: u128, say: impl Fn(&str) -> String) -> zikaron_ui::layer::Amount {
    zikaron_ui::layer::Amount::new(say(&eth_held(wei)), say(&eth_exact(wei)))
}

pub(super) fn no_amount(words: impl Into<String>) -> zikaron_ui::layer::Amount {
    let w = words.into();
    zikaron_ui::layer::Amount::new(w.clone(), w)
}

impl Win {
    pub(super) fn peek(&mut self, ask: crate::peekx::Ask, now: f64) -> Option<crate::peekx::Got> {
        if let Some(g) = self.shell.peek_landed(&ask) {
            return Some(g.clone());
        }
        if !self.reads_behind {
            let got = crate::peekx::read(&ask);
            self.shell.peeks.retain(|(a, _)| a.which() != ask.which());
            self.shell.peeks.push((ask, got.clone()));
            return Some(got);
        }
        if !self.shell.tasks.in_flight(crate::task::Kind::Peek) {
            self.auto(Action::Peek { ask }, now);
        }
        None
    }

    pub(super) fn open_entry(&mut self, id: &str) {
        self.opened = None;
        if let Applied::Opened { .. } = apply(&mut self.shell, Action::OpenEntry { id: id.to_string() }) {
            self.read_detail(Some(id));
        }
    }

    pub(super) fn read_detail(&mut self, want: Option<&str>) {
        let got = match (want, self.shell.home.as_ref()) {
            (Some(w), Some(home)) => Some(crate::ledgerx::detail(home, w)),
            _ => None,
        };
        match got {
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

    pub(super) fn held_issuer_name(&self, grant: &str) -> String {
        let issuer = self.shell.held.as_ref().and_then(|h| h.iter().find(|x| x.id.eq_ignore_ascii_case(grant))).map(|h| h.author.clone()).unwrap_or_default();
        self.issuer_name(&issuer)
    }

    pub(super) fn issuer_name(&self, author: &str) -> String {
        let a = crate::lastread::issuer_form(author);
        if let Some((_, n)) = self.shell.settings.issuer_notes.iter().find(|(x, n)| *x == a && !n.trim().is_empty()) {
            return n.clone();
        }
        if author.trim().is_empty() {
            return t(Key::UnnamedIssuer).to_string();
        }
        width::elide_chars(author.trim(), ISSUER_CHARS)
    }
}

const ISSUER_CHARS: usize = 13;

pub(super) fn user_kind_key(k: zikaron_glue::book::Kind) -> Key {
    match k {
        zikaron_glue::book::Kind::Genesis => Key::WizGenesisTitle,
        zikaron_glue::book::Kind::History => Key::TagAnchor,
        zikaron_glue::book::Kind::Succession => Key::TagHandover,
        other => kind_key(other),
    }
}

#[cfg(test)]
mod amounts {
    use super::eth_said;

    #[test]
    fn an_amount_said_carries_its_unit() {
        assert_eq!(eth_said(30_700_000_000_000), "0.0000307 ETH");
        assert_eq!(eth_said(0), "0 ETH");
    }
}
