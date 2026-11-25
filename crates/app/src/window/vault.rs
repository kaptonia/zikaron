use super::*;

pub(super) fn card_verdict(c: &crate::vaultx::Card) -> (Key, PillTone, Mark) {
    if c.undecided.is_some() {
        return (Key::OsOpen, PillTone::Warn, Mark::Warn);
    }
    let face = verdict_face(&c.verdict, &crate::vaultx::states(&c.checks));
    let gaps = c.gaps();
    if face.0 == Key::U3CheckSomeMissing && !gaps.is_empty() && gaps.iter().all(|g| *g == crate::checkx::Gap::LedgerBehind) {
        return (Key::GapLedgerBehind, PillTone::Warn, Mark::Warn);
    }
    face
}

fn undecided_note(u: zikaron_glue::book::Undecided) -> Key {
    match u {
        zikaron_glue::book::Undecided::OldLaw => Key::VaultOldLawNote,
        zikaron_glue::book::Undecided::Continued => Key::VaultContinuedNote,
    }
}

pub(super) fn verdict_face(verdict: &str, states: &[(String, String)]) -> (Key, PillTone, Mark) {
    use zikaron_kit::tokens::CheckVerdict as V;
    let revoked = states.iter().any(|(tok, st)| tok == zikaron_kit::tokens::Check::Revoked.as_str() && state_mark(st) == Mark::Bad);
    if verdict == V::Green.as_str() {
        (Key::U3CheckAllPass, PillTone::Ok, Mark::Ok)
    } else if revoked {
        (Key::U4Revoked, PillTone::Bad, Mark::Bad)
    } else if verdict == V::Fail.as_str() {
        (Key::U3CheckFail, PillTone::Bad, Mark::Bad)
    } else {
        (Key::U3CheckSomeMissing, PillTone::Warn, Mark::Warn)
    }
}

pub(super) fn countdown_say(c: &crate::vaultx::Countdown) -> String {
    use crate::vaultx::Countdown as K;
    match c {
        K::NoWindow => t(Key::CountdownNoWindow).to_string(),
        K::NoNow => t(Key::U4NoChainTime).to_string(),
        K::NotYet { starts_in } => fill1(Key::U4StartsInDays, &days_of(*starts_in)),
        K::Live { remaining } => fill1(Key::U4EndsInDays, &days_of(*remaining)),
        K::Expired { since } => fill1(Key::U4ExpiredDays, &days_of(*since)),
    }
}

pub(super) fn days_of(secs: u64) -> String {
    secs.div_ceil(86_400).to_string()
}

pub(super) fn anchor_age_say(secs: u64) -> String {
    match secs / 86_400 {
        0 => t(Key::U4AnchorAgeToday).to_string(),
        n => fill1(Key::U4AnchorAgeDays, &n.to_string()),
    }
}

const VERDICT_W: f32 = 120.0;

impl Win {
    pub(super) fn ensure_held(&mut self, now: f64) {
        if self.shell.held.is_some() || self.shell.home.is_none() || self.shell.tasks.attempted(crate::task::Kind::Held) {
            return;
        }
        self.auto(Action::ListHeld, now);
    }

    pub(super) fn held_record_name(&self, grant: &str) -> String {
        let g = crate::lastread::grant_form(grant);
        if let Some((_, n)) = self.shell.settings.grant_notes.iter().find(|(x, n)| *x == g && !n.trim().is_empty()) {
            return n.clone();
        }
        self.shell
            .cards
            .as_ref()
            .and_then(|(cards, _)| cards.iter().find(|c| c.id.eq_ignore_ascii_case(grant)).and_then(|c| c.record_name.clone()))
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| t(Key::UnnamedRecord).to_string())
    }

    pub(super) fn held_title(&self, grant: &str) -> String {
        self.held_record_name(grant)
    }

    fn held_face(&self, id: &str) -> (String, PillTone, Mark) {
        let card = self.shell.cards.as_ref().and_then(|(cards, _)| cards.iter().find(|x| x.id.eq_ignore_ascii_case(id) && x.told()).cloned());
        if let Some(k) = card {
            let (key, tone, m) = card_verdict(&k);
            return (t(key).to_string(), tone, m);
        }
        match self.shell.verdicts.iter().find(|(x, _)| x.eq_ignore_ascii_case(id)).map(|(_, v)| v.clone()) {
            Some(v) => {
                let (key, tone, m) = if v.undecided.is_some() { (Key::OsOpen, PillTone::Warn, Mark::Warn) } else { verdict_face(&v.verdict, &v.checks) };
                let stale = crate::lastread::stale(v.at, (self.shell.clock)());
                (format!("{} \u{b7} {}", t(key), fill1(Key::LastChecked, &hhmm_of(v.at))), if stale { PillTone::Grey } else { tone }, if stale { Mark::Todo } else { m })
            }
            None if self.shell.tasks.in_flight(crate::task::Kind::Review) => (t(Key::U4Checking).to_string(), PillTone::Grey, Mark::Work),
            None => (t(Key::U4NotChecked).to_string(), PillTone::Grey, Mark::Todo),
        }
    }

    pub(super) fn vault_acts(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_held(now);
        let (_s, r) = page::Page::new().primary(ui, t(Key::V2AddGrant));
        if r.clicked() {
            self.u4_import_open();
        }
        if self.long_key(ui, t(Key::U4ReviewAll), Role::Secondary, true, crate::task::Kind::Review) {
            self.act(Action::ReviewVault, now);
        }
        let cells = [seg::Cell::from(t(Key::U3FilterAll)), seg::Cell::from(t(Key::U4ByIssuer))];
        if let Some(i) = seg::seg(ui, "vault-by", &cells, usize::from(self.ux.u4.vault_by_issuer)) {
            self.ux.u4.vault_by_issuer = i == 1;
        }
    }

    pub(super) fn vault_page(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_held(now);
        let rejected = self.shell.held_rejected.clone();
        let mut query = self.ux.search.get("vault").cloned().unwrap_or_default();
        let mut range = self.ux.range.get("vault").cloned().unwrap_or_default();
        let today = (self.shell.clock)();
        let mut open: Option<String> = None;
        stagger(ui, 0, |ui| search_row(ui, "vault", &mut query, t(Key::SearchVault), &mut range, today));
        for x in &rejected {
            states::err_box(ui, &format!("vault-rej-{}", x.file), &fill1(Key::VaultRejected, &width::file_name(&x.file)), t(Key::U4RejectedNext), t(Key::U3RawError), &x.why);
        }
        if self.shell.held.as_ref().is_none_or(|h| h.is_empty()) {
            stagger(ui, 1, |ui| card::card(ui, |ui| states::empty(ui, Glyph::Vault, t(if self.shell.held.is_some() { Key::U4VaultEmpty } else { Key::WbNotRead }))));
            self.ux.search.insert("vault", query);
            self.ux.range.insert("vault", range);
            return;
        }
        let by_issuer = self.ux.u4.vault_by_issuer;
        let (view, generation) = kept(ui.ctx(), self.shell.moved, "vault", (&query, &range, by_issuer), || {
            let (cards, chain_now) = self.shell.cards.clone().unwrap_or((Vec::new(), None));
            let held: &[crate::vaultx::Held] = self.shell.held.as_deref().unwrap_or_default();
            type Face<'h> = (&'h crate::vaultx::Held, String, String, (String, PillTone, Mark));
            let faces: Vec<Face> = held
                .iter()
                .map(|h| {
                    let name = self.held_record_name(&h.id);
                    let issuer = self.issuer_name(&h.author);
                    let face = self.held_face(&h.id);
                    (h, name, issuer, face)
                })
                .collect();
            let shell = &self.shell;
            let anchored_say = |id: &str| shell.held_anchored_at(id).map(crate::when::when).unwrap_or_else(|| t(Key::U4AnchorAgeUnread).to_string());
            let shown = |f: &Face| matches(&query, &[&f.1, &f.2, &f.0.author, &(f.3).0]) && crate::when::within(shell.held_anchored_at(&f.0.id), &range.0, &range.1);
            let pill_w = faces.iter().map(|f| mark::pill_w(ui, &(f.3).0, false)).fold(VERDICT_W, f32::max).min(260.0).ceil();
            let rows_of = |members: &[&Face]| -> Listed {
                members
                    .iter()
                    .map(|(h, name, issuer, (verdict, tone, _))| {
                        let mut cells = vec![table::Cell::Text(name.clone())];
                        if !by_issuer {
                            cells.push(table::Cell::Text(issuer.clone()));
                        }
                        cells.extend([table::Cell::Short(super::faces::window_short(h.window, chain_now)), if shell.held_anchored_at(&h.id).is_some() { table::Cell::Mono(anchored_say(&h.id)) } else { table::Cell::Text(anchored_say(&h.id)) }, table::Cell::Pill(verdict.clone(), *tone)]);
                        (table::Row { cells, click: true, gone: false, on: false }, h.id.clone())
                    })
                    .collect()
            };
            let mut tables: Vec<(Option<String>, Listed)> = Vec::new();
            if by_issuer {
                for g in crate::vaultx::groups(&cards, chain_now) {
                    let members: Vec<&Face> = g.cards.iter().filter_map(|k| faces.iter().find(|f| f.0.id.eq_ignore_ascii_case(&k.id))).filter(|f| shown(f)).collect();
                    if members.is_empty() {
                        continue;
                    }
                    let age = g.anchor_age.map(anchor_age_say).unwrap_or_else(|| t(Key::U4AnchorAgeUnread).to_string());
                    tables.push((Some(format!("{} \u{b7} {age}", self.issuer_name(&g.author))), rows_of(&members)));
                }
                let loose: Vec<&Face> = faces.iter().filter(|f| !cards.iter().any(|k| k.id.eq_ignore_ascii_case(&f.0.id))).filter(|f| shown(f)).collect();
                if !loose.is_empty() {
                    tables.push((None, rows_of(&loose)));
                }
            } else {
                let members: Vec<&Face> = faces.iter().filter(|f| shown(f)).collect();
                tables.push((None, rows_of(&members)));
            }
            (pill_w, tables)
        });
        let (pill_w, tables) = (view.0, &view.1);
        let cols: Vec<table::Column> = [
            Some(table::col(t(Key::U3Work), table::Col::Fr(1.0))),
            (!by_issuer).then(|| table::col(t(Key::U3Issuer), table::Col::Fr(0.8))),
            Some(table::col(t(Key::U3Window), table::Col::Auto { min: 120.0, max: 320.0 })),
            Some(table::col(t(Key::U4RecordFirstAt), table::Col::Px(190.0))),
            Some(table::col(t(Key::U3Verdict), table::Col::Px(pill_w))),
        ]
        .into_iter()
        .flatten()
        .collect();
        let spec = table::Spec::new(&cols, generation).head(true);
        if by_issuer {
            for (gi, (title, rows)) in tables.iter().enumerate() {
                match title {
                    Some(title) => stagger(ui, gi + 1, |ui| {
                        card::section(ui, title, "", |ui| {
                            if let Some(id) = listed_table(ui, &format!("vault-{gi}"), spec, rows) {
                                open = Some(id);
                            }
                        });
                    }),
                    None => {
                        let title = format!("{} \u{b7} {}", t(Key::U4NotChecked), t(Key::U4ReviewToGroup));
                        card::section(ui, &title, "", |ui| {
                            if let Some(id) = listed_table(ui, "vault-loose", spec, rows) {
                                open = Some(id);
                            }
                        });
                    }
                }
            }
            if tables.is_empty() {
                card::card(ui, |ui| states::empty(ui, Glyph::Search, t(Key::SearchNone)));
            }
        } else if let Some((_, rows)) = tables.first() {
            stagger(ui, 1, |ui| {
                if let Some(id) = listed_table(ui, "vault", spec.empty(t(Key::SearchNone)), rows) {
                    open = Some(id);
                }
            });
        }
        self.ux.search.insert("vault", query);
        self.ux.range.insert("vault", range);
        if let Some(id) = open {
            self.typed.vt_grant = id.clone();
            self.typed.vt_dir = self.shell.settings.upstreams.iter().find(|(g, _)| g.eq_ignore_ascii_case(&id)).map(|(_, d)| d.clone()).unwrap_or_default();
            self.push(Route::Held(id), now);
        }
    }

    pub(super) fn held_detail(&mut self, ui: &mut egui::Ui, id: &str, now: f64) {
        self.ensure_held(now);
        let Some(h) = self.shell.held.clone().unwrap_or_default().into_iter().find(|x| x.id.eq_ignore_ascii_case(id)) else {
            states::empty(ui, Glyph::Vault, t(if self.shell.held.is_some() { Key::DetailGone } else { Key::WbNotRead }));
            return;
        };
        let card = self.shell.cards.as_ref().and_then(|(cards, _)| cards.iter().find(|x| x.id.eq_ignore_ascii_case(id)).cloned());
        let last = if card.is_none() { self.shell.verdicts.iter().find(|(x, _)| x.eq_ignore_ascii_case(id)).map(|(_, v)| v.clone()) } else { None };
        let name = self.held_record_name(id);
        let issuer = self.issuer_name(&h.author);
        let (verdict, tone, _) = self.held_face(id);
        stagger(ui, 0, |ui| {
            card::hero(ui, &name, &issuer, false, &[card::HeroPill { words: verdict.clone(), tone, live: false }]);
        });
        if let (None, Some(u)) = (card.as_ref(), last.as_ref().and_then(|v| v.undecided)) {
            stagger(ui, 1, |ui| states::note_box(ui, t(undecided_note(u))));
        }
        if let Some(k) = card.as_ref() {
            let (key, _, _) = card_verdict(k);
            stagger(ui, 1, |ui| {
                if key == Key::U4Revoked {
                    states::err_box(ui, "held-revoked", t(Key::U4RevokedWhat), t(Key::U4RevokedNext), t(Key::U3RawError), &k.said);
                } else if let Some(u) = k.undecided {
                    states::note_box(ui, t(undecided_note(u)));
                } else if let Some(g) = k.gaps().first() {
                    states::note_box(ui, t(gap_note(g)));
                }
                if k.handed.is_some() {
                    states::note_box(ui, t(Key::AlarmHandedPlain));
                }
                for m in &k.missed {
                    states::okline(ui, Mark::Warn, &format!("{} {}", m.name, t(m.key())));
                }
            });
        }
        let remaining = match (&card, &last) {
            (Some(k), _) if k.undecided.is_some() => match self.shell.verdicts.iter().find(|(x, _)| x.eq_ignore_ascii_case(id)) {
                Some((_, v)) => format!("{} \u{b7} {}", t(Key::U4NotChecked), fill1(Key::LastChecked, &hhmm_of(v.at))),
                None => t(Key::U4NotChecked).to_string(),
            },
            (Some(k), _) => match self.shell.verdicts.iter().find(|(x, _)| x.eq_ignore_ascii_case(id)) {
                Some((_, v)) => format!("{} \u{b7} {}", countdown_say(&k.countdown), fill1(Key::LastChecked, &hhmm_of(v.at))),
                None => countdown_say(&k.countdown),
            },
            (None, Some(v)) if v.undecided.is_some() || crate::lastread::stale(v.at, (self.shell.clock)()) => format!("{} \u{b7} {}", t(Key::U4NotChecked), fill1(Key::LastChecked, &hhmm_of(v.at))),
            (None, Some(v)) => format!("{} \u{b7} {}", countdown_say(&crate::vaultx::countdown(h.window, v.chain_now)), fill1(Key::LastChecked, &hhmm_of(v.at))),
            (None, None) => t(Key::U4NotChecked).to_string(),
        };
        let undecided = card.as_ref().map(|k| k.undecided).unwrap_or_else(|| last.as_ref().and_then(|v| v.undecided));
        let first = if undecided.is_some() { t(Key::FirstAnchorNotRead).to_string() } else { first_anchor_say(card.as_ref().and_then(|k| k.anchored_at)) };
        let mut copied = None;
        stagger(ui, 2, |ui| {
            card::section(ui, t(Key::BasicInfo), "", |ui| {
                card::form(ui, |ui, f| {
                    copied = f.info(ui, t(Key::U3Issuer), card::Show::Short(&h.author));
                    f.with(ui, t(Key::U3Window), "", |ui| super::grants::span_trail(ui, h.window));
                    let _ = f.info(ui, t(Key::U4RecordFirstAt), card::Show::Mono(&first));
                    let _ = f.info(ui, t(Key::U4Remaining), card::Show::Text(&remaining));
                    let _ = f.info(ui, t(Key::U4Upstream), h.upstream.as_deref().map(card::Show::Short).unwrap_or(card::Show::Empty(t(Key::U4NoUpstream))));
                });
            });
        });
        tell_copied(ui.ctx(), copied);
        let green = card.as_ref().map(|k| k.verdict == zikaron_kit::tokens::CheckVerdict::Green.as_str()).unwrap_or(false);
        let saved_dir = self.shell.settings.upstreams.iter().find(|(g, _)| g.eq_ignore_ascii_case(&h.id)).map(|(_, d)| d.clone());
        let badge_phase = self.phase_of(crate::task::Kind::Badge);
        let (mut badge, mut relicense, mut set_upstream) = (false, false, false);
        let made = self.shell.badge.clone().filter(|b| b.grant.eq_ignore_ascii_case(id));
        stagger(ui, 3, |ui| {
            card::section(ui, t(Key::PageBadge), "", |ui| {
                card::form(ui, |ui, f| {
                    badge = f.act(ui, t(Key::V2MakeBadge), if made.is_some() { t(Key::U4BadgeMade) } else { "" }, key::Key::new(t(Key::V2MakeBadge), Role::Secondary).phase(badge_phase)).clicked();
                    if let Some(b) = made.as_ref() {
                        let _ = f.info(ui, t(Key::U3Hops), card::Show::Text(&fill1(Key::CheckHopsCount, &b.hops.to_string())));
                        let _ = f.info(ui, t(Key::U4BadgeText), card::Show::Mono(&width::file_name(&b.txt.display().to_string())));
                        let _ = f.info(ui, t(Key::U4BadgeImage), card::Show::Mono(&width::file_name(&b.svg.display().to_string())));
                        f.free(ui, |ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                motion::swap(ui, egui::Id::new("held-badge"), motion::key_of(&b.txt), |ui| paint::qr(ui, &b.modules, 116.0));
                            });
                        });
                    }
                    if green {
                        relicense = f.go(ui, t(Key::V2RelicenseOthers), "", true, "").clicked();
                    }
                });
            });
        });
        stagger(ui, 4, |ui| {
            card::form(ui, |ui, f| {
                set_upstream = match saved_dir.as_deref() {
                    Some(d) => f.act_path(ui, t(Key::UpstreamDirName), d, key::Key::new(t(Key::DoChangeDir), Role::Secondary)),
                    None => f.act(ui, t(Key::UpstreamDirName), t(Key::SetNone), key::Key::new(t(Key::DoChangeDir), Role::Secondary)),
                }
                .clicked();
            });
        });
        let mut rows: Vec<(String, Val)> = vec![
            (t(Key::U3Issuer).to_string(), Val::mono(h.author.clone())),
            (t(Key::U4ToMe).to_string(), Val::mono(h.grantee.clone())),
            (t(Key::DetailPick).to_string(), Val::mono(h.id.clone())),
            (t(Key::U3TermsHash).to_string(), Val::mono(h.terms.clone())),
            (t(Key::ContentHash).to_string(), Val::mono(h.work.clone())),
        ];
        if let Some(d) = saved_dir.as_ref() {
            rows.push((t(Key::UpstreamDirName).to_string(), Val::mono(d.clone())));
        }
        if let Some(k) = card.as_ref() {
            rows.push((t(Key::U3Verdict).to_string(), Val::figure(k.verdict.clone())));
            for (tok, st) in crate::vaultx::states(&k.checks) {
                rows.push((tok, Val::figure(st)));
            }
        }
        if let Some(b) = made.as_ref() {
            rows.push((t(Key::U4BadgeText).to_string(), Val::mono(b.txt.display().to_string())));
            rows.push((t(Key::U4BadgeImage).to_string(), Val::mono(b.svg.display().to_string())));
        }
        let refs: Vec<(&str, Val)> = rows.iter().map(|(a, b)| (a.as_str(), b.clone())).collect();
        stagger(ui, 5, |ui| details_card(ui, "held-details", &refs));
        if badge {
            self.out_open(super::grants::OutSheet::Badge(h.id.clone()));
        }
        if relicense {
            self.u4_confirm_open(U4Confirm::Relicense { grant: h.id.clone() });
        }
        if set_upstream {
            self.typed.vt_dir = saved_dir.unwrap_or_default();
            self.out_open(super::grants::OutSheet::Upstream(h.id.clone()));
        }
    }

    pub(super) fn on_relicense_page(&mut self) -> bool {
        self.route() == Route::Relicense
    }

    pub(super) fn relicense_page(&mut self, ui: &mut egui::Ui, now: f64) {
        self.takes_drops();
        let upstream = self.typed.g_upstream.trim().to_string();
        let held = self.shell.held.clone().unwrap_or_default().into_iter().find(|h| h.id.eq_ignore_ascii_case(&upstream));
        let name = self.held_record_name(&upstream);
        let issuer = held.as_ref().map(|h| self.issuer_name(&h.author)).unwrap_or_else(|| t(Key::UnnamedIssuer).to_string());
        let window = held.as_ref().and_then(|h| h.window);
        stagger(ui, 0, |ui| {
            card::form(ui, |ui, f| {
                let _ = f.info(ui, t(Key::U3Work), card::Show::Text(&name));
                let _ = f.info(ui, t(Key::U3Issuer), card::Show::Text(&issuer));
                f.with(ui, t(Key::U4WindowCap), "", |ui| super::grants::span_trail(ui, window));
            });
            details_card(
                ui,
                "relicense-page-details",
                &[
                    (t(Key::U4Upstream), if upstream.is_empty() { Val::text(t(Key::None_)) } else { Val::mono(upstream.clone()) }),
                    (t(Key::U3Issuer), Val::mono(held.as_ref().map(|h| h.author.clone()).unwrap_or_default())),
                ],
            );
        });
        match crate::relicx::guide(self.shell.anchor.is_some(), self.shell.rooted) {
            crate::relicx::Guide::Ready => self.grant_form(ui, Some(name), now),
            crate::relicx::Guide::NeedKey => {
                states::err_box(ui, "relic-key", t(Key::U4NeedKeyWhat), t(Key::U4NeedKeyNext), t(Key::U3RawError), "");
                let mut go = false;
                card::form(ui, |ui, f| go = f.go(ui, t(Key::U4GoKeys), "", true, "").clicked());
                if go {
                    self.wizard_open(crate::nav::Step::Key);
                }
            }
            crate::relicx::Guide::NeedGenesis if self.marked_title().is_some() => {
                let title = self.marked_title().unwrap_or_default();
                let mut fetch = false;
                card::form(ui, |ui, f| fetch = f.act(ui, &title, "", key::Key::new(t(Key::DoFetchLedger), Role::Primary)).clicked());
                if fetch {
                    self.go(Place::Settings(crate::nav::Section::Data), now);
                }
            }
            crate::relicx::Guide::NeedGenesis => {
                self.genesis_offered();
                states::err_box(ui, "relic-genesis", t(Key::U4NeedLedgerWhat), t(Key::U4NeedLedgerNext), t(Key::U3RawError), "");
                let mut go = false;
                card::form(ui, |ui, f| go = f.go(ui, t(Key::WizGenesisTitle), "", true, "").clicked());
                if go {
                    self.wizard_open(crate::nav::Step::Genesis);
                }
            }
        }
    }
}

#[cfg(test)]
mod r36_tests {
    use super::*;

    #[test]
    fn how_long_ago_rounds_down_and_the_countdown_up() {
        let said = |secs: u64| crate::lang::both_of(|| anchor_age_say(secs));
        assert_eq!(said(240), crate::lang::both_of(|| t(Key::U4AnchorAgeToday).to_string()));
        assert_eq!(said(0), said(86_399));
        assert_eq!(said(90_000), crate::lang::both_of(|| fill1(Key::U4AnchorAgeDays, "1")));
        assert_eq!(said(86_400 * 3 + 5), crate::lang::both_of(|| fill1(Key::U4AnchorAgeDays, "3")));
        assert_eq!((days_of(240), days_of(90_000)), ("1".to_string(), "2".to_string()), "the countdown rounds up");
    }

    #[test]
    fn a_short_window_drops_the_years_only_within_this_year() {
        use super::super::faces::window_short;
        crate::when::set(crate::when::Zone::Utc);
        let day = |y: u32, m: u32, d: u32| -> u64 {
            match (y, m, d) {
                (2023, 11, 14) => 1_699_963_200,
                (2033, 1, 22) => 1_990_008_000,
                (2026, 2, 1) => 1_769_947_200,
                (2026, 11, 30) => 1_796_040_000,
                (2026, 11, 21) => 1_795_262_400,
                _ => 1_799_150_400,
            }
        };
        let ten_years = Some((day(2023, 11, 14), day(2033, 1, 22)));
        let this_year = Some((day(2026, 2, 1), day(2026, 11, 30)));
        let now = Some(day(2026, 11, 21));
        let both = |w, n| crate::lang::both_of(|| window_short(w, n));
        let long = |w| crate::lang::both_of(|| super::super::faces::window_long(w));
        assert_eq!(both(ten_years, now), long(ten_years), "ends in other years: both years said");
        assert_eq!(both(ten_years, None), long(ten_years));
        assert_eq!(both(this_year, None), long(this_year), "this year not known: the years said");
        let short = both(this_year, now);
        assert_ne!(short, long(this_year), "both ends this year: no years");
        assert!(short.zh.starts_with("02-01") && short.en.starts_with("02-01"), "{short:?}");
        assert_eq!(both(Some((day(2026, 2, 1), day(2027, 1, 5))), now), long(Some((day(2026, 2, 1), day(2027, 1, 5)))), "one end next year");
        assert_eq!(both(None, now), crate::lang::both_of(|| t(Key::CountdownNoWindow).to_string()));
    }
}
