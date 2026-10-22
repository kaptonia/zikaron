//! My grants (the grantee seat): held grants in a table (record name, issuer name, validity, first anchor,
//! verdict), optionally one table per issuer; a grant's detail page (credential, sublicense, upstream folder,
//! details); and the sublicense page opened from it.

use super::*;

/// A card's verdict in plain words, pill tone and mark.
pub(super) fn card_verdict(c: &crate::vaultx::Card) -> (Key, PillTone, Mark) {
    verdict_face(&c.verdict, &crate::vaultx::states(&c.checks))
}

/// The verdict and the six checks as a face (from this pass's cards or the last pass's saved verdict).
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

/// The countdown in plain words (chain time only; says so when there is none).
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

/// Seconds as whole days, rounded up (display only).
pub(super) fn days_of(secs: u64) -> String {
    secs.div_ceil(86_400).to_string()
}

/// The verdict column's least width (the pills of every table line up at the widest).
const VERDICT_W: f32 = 120.0;

impl Win {
    /// List the vault once (started by the app itself, no toast).
    pub(super) fn ensure_held(&mut self, now: f64) {
        if self.shell.held.is_some() || self.shell.home.is_none() || self.shell.tasks.attempted(crate::task::Kind::Held) {
            return;
        }
        self.auto(Action::ListHeld, now);
    }

    /// A held grant's record name: the user's own name for it on this machine, else the issuer ledger's, else
    /// "unnamed record".
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

    /// The toolbar title of a held grant's page.
    pub(super) fn held_title(&self, grant: &str) -> String {
        self.held_record_name(grant)
    }

    /// A held grant's verdict face: this pass's card, else the last pass's saved verdict (grey when stale, with
    /// when it was checked), else "not verified".
    fn held_face(&self, id: &str) -> (String, PillTone, Mark) {
        let card = self.shell.cards.as_ref().and_then(|(cards, _)| cards.iter().find(|x| x.id.eq_ignore_ascii_case(id)).cloned());
        if let Some(k) = card {
            let (key, tone, m) = card_verdict(&k);
            return (t(key).to_string(), tone, m);
        }
        match self.shell.verdicts.iter().find(|(x, _)| x.eq_ignore_ascii_case(id)).map(|(_, v)| v.clone()) {
            Some(v) => {
                let (key, tone, m) = verdict_face(&v.verdict, &v.checks);
                let stale = crate::lastread::stale(v.at, (self.shell.clock)());
                (format!("{} \u{b7} {}", t(key), fill1(Key::LastChecked, &hhmm_of(v.at))), if stale { PillTone::Grey } else { tone }, if stale { Mark::Todo } else { m })
            }
            None => (t(Key::U4NotChecked).to_string(), PillTone::Grey, Mark::Todo),
        }
    }

    /// The keys at the right of "my grants": all or by issuer, verify again, add a grant.
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

    /// My grants: search, then one table of held grants (record, issuer, validity, first anchor, verdict); by
    /// issuer, one table per issuer under a group title saying how that issuer's ledger stands, and the grants
    /// never verified in a group of their own.
    pub(super) fn vault_page(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_held(now);
        let (cards, chain_now) = self.shell.cards.clone().unwrap_or((Vec::new(), None));
        let held = self.shell.held.clone().unwrap_or_default();
        let rejected = self.shell.held_rejected.clone();
        let mut query = self.ux.search.get("vault").cloned().unwrap_or_default();
        let mut range = self.ux.range.get("vault").cloned().unwrap_or_default();
        let today = (self.shell.clock)();
        let mut open: Option<String> = None;
        stagger(ui, 0, |ui| search_row(ui, "vault", &mut query, t(Key::SearchVault), &mut range, today));
        for x in &rejected {
            states::err_box(ui, &format!("vault-rej-{}", x.file), &fill1(Key::VaultRejected, &width::file_name(&x.file)), t(Key::U4RejectedNext), t(Key::U3RawError), &x.why);
        }
        if held.is_empty() {
            stagger(ui, 1, |ui| card::card(ui, |ui| states::empty(ui, Glyph::Vault, t(if self.shell.held.is_some() { Key::U4VaultEmpty } else { Key::WbNotRead }))));
            self.ux.search.insert("vault", query);
            self.ux.range.insert("vault", range);
            return;
        }
        type Face = (crate::vaultx::Held, String, String, (String, PillTone, Mark));
        let faces: Vec<Face> = held
            .iter()
            .map(|h| {
                let name = self.held_record_name(&h.id);
                let issuer = self.issuer_name(&h.author);
                let face = self.held_face(&h.id);
                (h.clone(), name, issuer, face)
            })
            .collect();
        // The block time the grant was anchored at in its issuer's ledger, which the date range filters by (this
        // run's re-check, else the cached verdict), in the record rows' format; "not read" only while neither has it.
        let shell = &self.shell;
        let anchored_say = |id: &str| shell.held_anchored_at(id).map(crate::when::when).unwrap_or_else(|| t(Key::U4AnchorAgeUnread).to_string());
        let shown = |f: &Face| matches(&query, &[&f.1, &f.2, &f.0.author, &(f.3).0]) && crate::when::within(shell.held_anchored_at(&f.0.id), &range.0, &range.1);
        // The verdict column is as wide as its widest pill, so the pills of every table line up.
        let pill_w = faces.iter().map(|f| mark::pill_w(ui, &(f.3).0, false)).fold(VERDICT_W, f32::max).min(260.0).ceil();
        let by_issuer = self.ux.u4.vault_by_issuer;
        let cols: Vec<table::Column> = [
            Some(table::col(t(Key::U3Work), table::Col::Fr(1.0))),
            (!by_issuer).then(|| table::col(t(Key::U3Issuer), table::Col::Fr(0.8))),
            Some(table::col(t(Key::U3Window), table::Col::Fr(1.0))),
            Some(table::col(t(Key::U4RecordFirstAt), table::Col::Px(190.0))),
            Some(table::col(t(Key::U3Verdict), table::Col::Px(pill_w))),
        ]
        .into_iter()
        .flatten()
        .collect();
        let rows_of = |members: &[&Face]| -> Vec<table::Row> {
            members
                .iter()
                .map(|(h, name, issuer, (verdict, tone, _))| {
                    let mut cells = vec![table::Cell::Text(name.clone())];
                    if !by_issuer {
                        cells.push(table::Cell::Text(issuer.clone()));
                    }
                    cells.extend([table::Cell::Text(super::faces::window_short(h.window)), if shell.held_anchored_at(&h.id).is_some() { table::Cell::Mono(anchored_say(&h.id)) } else { table::Cell::Text(anchored_say(&h.id)) }, table::Cell::Pill(verdict.clone(), *tone)]);
                    table::Row { cells, click: true, gone: false, on: false }
                })
                .collect()
        };
        if by_issuer {
            let groups = crate::vaultx::groups(&cards, chain_now);
            let mut any = false;
            for (gi, g) in groups.iter().enumerate() {
                let members: Vec<&Face> = g.cards.iter().filter_map(|k| faces.iter().find(|f| f.0.id.eq_ignore_ascii_case(&k.id))).filter(|f| shown(f)).collect();
                if members.is_empty() {
                    continue;
                }
                any = true;
                let age = g.anchor_age.map(|a| fill1(Key::U4AnchorAgeDays, &days_of(a))).unwrap_or_else(|| t(Key::U4AnchorAgeUnread).to_string());
                let label = if g.label.is_empty() { t(Key::U4AuditUnread).to_string() } else { fill1(Key::U4IssuerAudit, &label_human(&g.label)) };
                let title = format!("{} \u{b7} {label} \u{b7} {age}", self.issuer_name(&g.author));
                let rows = rows_of(&members);
                stagger(ui, gi + 1, |ui| {
                    card::section(ui, &title, "", |ui| {
                        if let Some(i) = table::table(ui, &format!("vault-{gi}"), &cols, true, &rows, "").clicked {
                            open = members.get(i).map(|f| f.0.id.clone());
                        }
                    });
                });
            }
            // Grants never verified have no issuer reading, so they form a group of their own.
            let loose: Vec<&Face> = faces.iter().filter(|f| !cards.iter().any(|k| k.id.eq_ignore_ascii_case(&f.0.id))).filter(|f| shown(f)).collect();
            if !loose.is_empty() {
                any = true;
                let title = format!("{} \u{b7} {}", t(Key::U4NotChecked), t(Key::U4ReviewToGroup));
                let rows = rows_of(&loose);
                card::section(ui, &title, "", |ui| {
                    if let Some(i) = table::table(ui, "vault-loose", &cols, true, &rows, "").clicked {
                        open = loose.get(i).map(|f| f.0.id.clone());
                    }
                });
            }
            if !any {
                card::card(ui, |ui| states::empty(ui, Glyph::Search, t(Key::SearchNone)));
            }
        } else {
            let members: Vec<&Face> = faces.iter().filter(|f| shown(f)).collect();
            let rows = rows_of(&members);
            stagger(ui, 1, |ui| {
                if let Some(i) = table::table(ui, "vault", &cols, true, &rows, t(Key::SearchNone)).clicked {
                    open = members.get(i).map(|f| f.0.id.clone());
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

    /// A held grant, laid as a grant's page: the head (the record, its issuer, the verdict), what is wrong under
    /// it, the basic facts (the issuer's address first), the credential (make it; sublicense once every check
    /// passed), the upstream ledger folder (each opens its sheet), and details.
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
            card::hero(ui, &name, &issuer, false, |ui| {
                mark::pill(ui, &verdict, tone);
            });
        });
        if let Some(k) = card.as_ref() {
            let (key, _, m) = card_verdict(k);
            stagger(ui, 1, |ui| {
                if key == Key::U4Revoked {
                    states::err_box(ui, "held-revoked", t(Key::U4RevokedWhat), t(Key::U4RevokedNext), t(Key::U3RawError), &k.said);
                } else if m != Mark::Ok {
                    // The sentence depends on the gap: no issuer ledger from any source, or found and waiting to be
                    // anchored.
                    states::note_box(ui, t(if k.from.is_none() { Key::NoteNoLedger } else { Key::NoteNotYetAnchored }));
                }
                if k.handed.is_some() {
                    states::note_box(ui, t(Key::AlarmHandedPlain));
                }
                // Name each network this re-check could not read, with why (in the check page's words).
                for m in &k.missed {
                    states::okline(ui, Mark::Warn, &format!("{} {}", m.name, t(m.reading.key())));
                }
            });
        }
        let remaining = match (&card, &last) {
            (Some(k), _) => match self.shell.verdicts.iter().find(|(x, _)| x.eq_ignore_ascii_case(id)) {
                Some((_, v)) => format!("{} \u{b7} {}", countdown_say(&k.countdown), fill1(Key::LastChecked, &hhmm_of(v.at))),
                None => countdown_say(&k.countdown),
            },
            // Not verified this pass: show the last pass's chain time with when it was checked; a stale one only says
            // when (an old chain time would count down wrongly).
            (None, Some(v)) if crate::lastread::stale(v.at, (self.shell.clock)()) => format!("{} \u{b7} {}", t(Key::U4NotChecked), fill1(Key::LastChecked, &hhmm_of(v.at))),
            (None, Some(v)) => format!("{} \u{b7} {}", countdown_say(&crate::vaultx::countdown(h.window, v.chain_now)), fill1(Key::LastChecked, &hhmm_of(v.at))),
            (None, None) => t(Key::U4NotChecked).to_string(),
        };
        let upstream = match (&h.upstream, card.as_ref(), &last) {
            (None, _, _) => None,
            (Some(_), Some(k), _) if !k.upstream_label.is_empty() => Some(fill1(Key::U4UpstreamAudit, &label_human(&k.upstream_label))),
            (Some(_), None, Some(v)) if !v.upstream_label.is_empty() => Some(fill1(Key::U4UpstreamAudit, &label_human(&v.upstream_label))),
            (Some(_), _, _) => Some(t(Key::U4UpstreamUnread).to_string()),
        };
        let first = first_anchor_say(card.as_ref().and_then(|k| k.anchored_at));
        let mut copied = None;
        stagger(ui, 2, |ui| {
            card::section(ui, t(Key::BasicInfo), "", |ui| {
                card::form(ui, |ui, f| {
                    copied = f.info(ui, t(Key::U3Issuer), card::Show::Short(&h.author));
                    f.with(ui, t(Key::U3Window), "", |ui| super::grants::span_trail(ui, h.window));
                    let _ = f.info(ui, t(Key::U4RecordFirstAt), card::Show::Mono(&first));
                    let _ = f.info(ui, t(Key::U4Remaining), card::Show::Text(&remaining));
                    let _ = f.info(ui, t(Key::U4Upstream), upstream.as_deref().map(card::Show::Text).unwrap_or(card::Show::Empty(t(Key::U4NoUpstream))));
                });
            });
        });
        tell_copied(ui.ctx(), copied);
        let green = card.as_ref().map(|k| k.verdict == zikaron_kit::tokens::CheckVerdict::Green.as_str()).unwrap_or(false);
        // The upstream ledger folder saved for this grant (the sheet edits the typed copy).
        let saved_dir = self.shell.settings.upstreams.iter().find(|(g, _)| g.eq_ignore_ascii_case(&h.id)).map(|(_, d)| d.clone());
        let badge_phase = self.phase_of(crate::task::Kind::Badge);
        let (mut badge, mut relicense, mut set_upstream) = (false, false, false);
        // The credential is shown only on the grant it was made for.
        let made = self.shell.badge.clone().filter(|b| b.grant.eq_ignore_ascii_case(id));
        // The credential: make it (and what was made, with its code), and "sublicense to others" once every check
        // passed.
        stagger(ui, 3, |ui| {
            card::section(ui, t(Key::PageBadge), "", |ui| {
                card::form(ui, |ui, f| {
                    badge = f.act(ui, t(Key::V2MakeBadge), if made.is_some() { t(Key::U4BadgeMade) } else { "" }, key::Key::new(t(Key::V2MakeBadge), Role::Secondary).phase(badge_phase)).clicked();
                    if let Some(b) = made.as_ref() {
                        let _ = f.info(ui, t(Key::U3Hops), card::Show::Text(&fill1(Key::CheckHopsCount, &b.hops.to_string())));
                        let _ = f.info(ui, t(Key::U4BadgeText), card::Show::Mono(&width::file_name(&b.txt.display().to_string())));
                        let _ = f.info(ui, t(Key::U4BadgeImage), card::Show::Mono(&width::file_name(&b.svg.display().to_string())));
                        // The credential's code at the card's right line, on its plate.
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
        // The upstream ledger folder this grant is checked against.
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
            rows.push((t(Key::AuditLabel).to_string(), Val::figure(k.upstream_label.clone())));
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

    /// Sublicense (opened from a held grant): the upstream's record, issuer and validity, then the grant form
    /// with that record fixed. Without a key or a ledger, says what to do first.
    pub(super) fn relicense_page(&mut self, ui: &mut egui::Ui, now: f64) {
        // Its terms file zone takes dropped files.
        self.takes_drops();
        let upstream = self.typed.g_upstream.trim().to_string();
        let held = self.shell.held.clone().unwrap_or_default().into_iter().find(|h| h.id.eq_ignore_ascii_case(&upstream));
        let name = self.held_record_name(&upstream);
        let issuer = held.as_ref().map(|h| self.issuer_name(&h.author)).unwrap_or_else(|| t(Key::UnnamedIssuer).to_string());
        let window = held.as_ref().and_then(|h| h.window);
        // The page's title already says "sublicense": the card holds only its rows.
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
            g @ crate::relicx::Guide::NeedKey => {
                states::err_box(ui, "relic-key", t(Key::U4NeedKeyWhat), t(Key::U4NeedKeyNext), t(Key::U3RawError), g.as_str());
                let mut go = false;
                card::form(ui, |ui, f| go = f.go(ui, t(Key::U4GoKeys), "", true, "").clicked());
                if go {
                    // Making the key is a wizard step; without a passcode the wizard starts at the passcode.
                    self.wizard_open(crate::nav::Step::Key);
                }
            }
            g @ crate::relicx::Guide::NeedGenesis => {
                states::err_box(ui, "relic-genesis", t(Key::U4NeedLedgerWhat), t(Key::U4NeedLedgerNext), t(Key::U3RawError), g.as_str());
                let mut go = false;
                card::form(ui, |ui, f| go = f.go(ui, t(Key::WizGenesisTitle), "", true, "").clicked());
                if go {
                    self.wizard_open(crate::nav::Step::Genesis);
                }
            }
        }
    }
}
