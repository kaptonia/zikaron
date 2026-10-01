//! My grants (the grantee seat): the held grants as cards (the record's name, the issuer's name, validity,
//! last on chain, the verdict), grouped by issuer on request; a grant's detail page (credential, sublicense,
//! upstream, details); and the sublicense page pushed from it.

use super::*;

/// A card's verdict in plain words, pill tone and mark.
pub(super) fn card_verdict(c: &crate::vaultx::Card) -> (Key, PillTone, Mark) {
    verdict_face(&c.verdict, &crate::vaultx::states(&c.checks))
}

/// The verdict and the six checks read as a face (this pass's cards and the last pass's saved verdict alike).
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

impl Win {
    /// List the vault once (started by the product, no toast).
    pub(super) fn ensure_held(&mut self, now: f64) {
        if self.shell.held.is_some() || self.shell.home.is_none() || self.shell.tasks.attempted(crate::task::Kind::Held) {
            return;
        }
        self.auto(Action::ListHeld, now);
    }

    /// A held grant's record name: the person's own name for it on this machine, else the issuer's ledger's;
    /// "unnamed record" when neither says.
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

    /// A held grant's verdict face: this pass's card, else the last pass's saved verdict (grey when stale,
    /// with when it was checked), else "not verified".
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

    pub(super) fn vault_page(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_held(now);
        let (cards, chain_now) = self.shell.cards.clone().unwrap_or((Vec::new(), None));
        let held = self.shell.held.clone().unwrap_or_default();
        let rejected = self.shell.held_rejected.clone();
        let mut query = self.ux.search.get("vault").cloned().unwrap_or_default();
        let mut open: Option<String> = None;
        stagger(ui, 0, |ui| {
            let w = ui.available_width();
            input::search(ui, &mut query, t(Key::SearchVault), w);
        });
        for x in &rejected {
            states::err_box(ui, &format!("vault-rej-{}", x.file), &fill1(Key::VaultRejected, &width::file_name(&x.file)), t(Key::U4RejectedNext), t(Key::U3RawError), &x.why);
        }
        if held.is_empty() {
            stagger(ui, 1, |ui| card::card(ui, |ui| states::empty(ui, Glyph::Vault, t(if self.shell.held.is_some() { Key::U4VaultEmpty } else { Key::WbNotRead }))));
            self.ux.search.insert("vault", query);
            return;
        }
        let faces: Vec<(crate::vaultx::Held, String, String, (String, PillTone, Mark))> = held
            .iter()
            .map(|h| {
                let name = self.held_record_name(&h.id);
                let issuer = self.issuer_name(&h.author);
                let face = self.held_face(&h.id);
                (h.clone(), name, issuer, face)
            })
            .collect();
        let age_of = |id: &str| {
            cards
                .iter()
                .find(|k| k.id.eq_ignore_ascii_case(id))
                .and_then(|k| crate::vaultx::anchor_age(k.latest_anchor, chain_now))
                .map(|a| fill1(Key::U4AnchorAgeDays, &days_of(a)))
                .unwrap_or_else(|| t(Key::U4AnchorAgeUnread).to_string())
        };
        let shown = |f: &(crate::vaultx::Held, String, String, (String, PillTone, Mark))| matches(&query, &[&f.1, &f.2, &f.0.author, &(f.3).0]);
        let draw = |ui: &mut egui::Ui, f: &(crate::vaultx::Held, String, String, (String, PillTone, Mark)), open: &mut Option<String>| {
            let (h, name, issuer, (verdict, tone, _)) = f;
            let (_, resp) = card::open_card(ui, egui::Id::new(("held-card", &h.id)), |ui| {
                ui.spacing_mut().item_spacing.y = tk::S1;
                width::then_at(ui, Type::Card.line(), |ui| mark::pill(ui, verdict, *tone), |ui, room| paint::line(ui, name, Type::Card, c(C::Ink), room));
                let room = ui.available_width();
                paint::line(ui, &format!("{} \u{b7} {} \u{b7} {}", issuer, window_short(h.window), age_of(&h.id)), Type::Note, c(C::Ink2), room);
            });
            if resp.clicked() {
                *open = Some(h.id.clone());
            }
        };
        if self.ux.u4.vault_by_issuer {
            let groups = crate::vaultx::groups(&cards, chain_now);
            for (gi, g) in groups.iter().enumerate() {
                let members: Vec<&(crate::vaultx::Held, String, String, (String, PillTone, Mark))> =
                    g.cards.iter().filter_map(|k| faces.iter().find(|f| f.0.id.eq_ignore_ascii_case(&k.id))).filter(|f| shown(f)).collect();
                if members.is_empty() {
                    continue;
                }
                let m = if g.red {
                    Mark::Bad
                } else if g.label == zikaron::tokens::Label::Complete.as_str() {
                    Mark::Ok
                } else if g.label.is_empty() {
                    Mark::Todo
                } else {
                    Mark::Warn
                };
                let age = g.anchor_age.map(|a| fill1(Key::U4AnchorAgeDays, &days_of(a))).unwrap_or_else(|| t(Key::U4AnchorAgeUnread).to_string());
                let label = if g.label.is_empty() { t(Key::U4AuditUnread).to_string() } else { fill1(Key::U4IssuerAudit, &label_human(&g.label)) };
                let who = self.issuer_name(&g.author);
                stagger(ui, gi + 1, |ui| {
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = tk::S2;
                        group_head(ui, m, &who, &format!("{label} \u{b7} {age}"));
                        for f in members {
                            draw(ui, f, &mut open);
                        }
                    });
                });
            }
            // Grants never verified have no issuer reading: a group of their own.
            let loose: Vec<&(crate::vaultx::Held, String, String, (String, PillTone, Mark))> =
                faces.iter().filter(|f| !cards.iter().any(|k| k.id.eq_ignore_ascii_case(&f.0.id))).filter(|f| shown(f)).collect();
            if !loose.is_empty() {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = tk::S2;
                    group_head(ui, Mark::Todo, t(Key::U4NotChecked), t(Key::U4ReviewToGroup));
                    for f in loose {
                        draw(ui, f, &mut open);
                    }
                });
            }
        } else {
            stagger(ui, 1, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = tk::S2;
                    for f in faces.iter().filter(|f| shown(f)) {
                        draw(ui, f, &mut open);
                    }
                });
            });
        }
        if !faces.iter().any(shown) {
            card::card(ui, |ui| states::empty(ui, Glyph::Search, t(Key::SearchNone)));
        }
        self.ux.search.insert("vault", query);
        if let Some(id) = open {
            self.typed.vt_grant = id.clone();
            self.typed.vt_dir = self.shell.settings.upstreams.iter().find(|(g, _)| g.eq_ignore_ascii_case(&id)).map(|(_, d)| d.clone()).unwrap_or_default();
            self.push(Route::Held(id), now);
        }
    }

    /// A held grant: record, issuer and validity in the head with the verdict; what is wrong said under it;
    /// the basic facts; the credential and the sublicense; the upstream and details folded.
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
        let window = self.window_days(h.window);
        stagger(ui, 0, |ui| {
            card::hero(ui, &name, &format!("{issuer} \u{b7} {window}"), false, |ui| {
                mark::pill(ui, &verdict, tone);
            });
        });
        if let Some(k) = card.as_ref() {
            let (key, _, m) = card_verdict(k);
            stagger(ui, 1, |ui| {
                if key == Key::U4Revoked {
                    states::err_box(ui, "held-revoked", t(Key::U4RevokedWhat), t(Key::U4RevokedNext), t(Key::U3RawError), &k.said);
                } else if m != Mark::Ok {
                    // The sentence follows the gap: no issuer ledger from any source, or found and waiting to
                    // be anchored.
                    states::note_box(ui, t(if k.from.is_none() { Key::NoteNoLedger } else { Key::NoteNotYetAnchored }));
                }
                if k.handed.is_some() {
                    states::note_box(ui, t(Key::AlarmHandedPlain));
                }
            });
        }
        let remaining = match (&card, &last) {
            (Some(k), _) => match self.shell.verdicts.iter().find(|(x, _)| x.eq_ignore_ascii_case(id)) {
                Some((_, v)) => format!("{} \u{b7} {}", countdown_say(&k.countdown), fill1(Key::LastChecked, &hhmm_of(v.at))),
                None => countdown_say(&k.countdown),
            },
            // Not verified this pass: the last pass's chain time with when it was checked; a stale one only
            // says when (an old chain time would count down wrongly).
            (None, Some(v)) if crate::lastread::stale(v.at, (self.shell.clock)()) => format!("{} \u{b7} {}", t(Key::U4NotChecked), fill1(Key::LastChecked, &hhmm_of(v.at))),
            (None, Some(v)) => format!("{} \u{b7} {}", countdown_say(&crate::vaultx::countdown(h.window, v.chain_now)), fill1(Key::LastChecked, &hhmm_of(v.at))),
            (None, None) => t(Key::U4NotChecked).to_string(),
        };
        let upstream = match (&h.upstream, card.as_ref(), &last) {
            (None, _, _) => t(Key::U4NoUpstream).to_string(),
            (Some(_), Some(k), _) if !k.upstream_label.is_empty() => fill1(Key::U4UpstreamAudit, &label_human(&k.upstream_label)),
            (Some(_), None, Some(v)) if !v.upstream_label.is_empty() => fill1(Key::U4UpstreamAudit, &label_human(&v.upstream_label)),
            (Some(_), _, _) => t(Key::U4UpstreamUnread).to_string(),
        };
        stagger(ui, 2, |ui| {
            kv_section(
                ui,
                t(Key::BasicInfo),
                &[
                    (t(Key::U3Issuer), Val::text(issuer.clone())),
                    (t(Key::U3Window), Val::text(window.clone())),
                    (t(Key::U4RecordFirstAt), Val::mono(first_anchor_say(card.as_ref().and_then(|k| k.anchored_at)))),
                    (t(Key::U4Remaining), Val::text(remaining)),
                    (t(Key::U4Upstream), Val::text(upstream)),
                ],
            );
        });
        let green = card.as_ref().map(|k| k.verdict == zikaron_kit::tokens::CheckVerdict::Green.as_str()).unwrap_or(false);
        if self.typed.badge_out.trim().is_empty() {
            if let Some(home) = self.shell.home.as_ref() {
                self.typed.badge_out = home.dir(crate::home::Slot::Kits).display().to_string();
            }
        }
        let (mut badge, mut relicense, mut set_upstream) = (false, false, false);
        // The credential is shown only on the grant it was made for.
        let made = self.shell.badge.clone().filter(|b| b.grant.eq_ignore_ascii_case(id));
        stagger(ui, 3, |ui| {
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S3;
                Self::place_row(ui, Key::IdStore, &mut self.typed.badge_out);
                keys_row(ui, |ui| {
                    badge = self.long_key(ui, t(Key::V2MakeBadge), Role::Secondary, Self::landing_ok(&self.typed.badge_out), crate::task::Kind::Badge);
                    if green {
                        relicense = key::key(ui, t(Key::V2RelicenseOthers), Role::Secondary, true).clicked();
                    }
                });
                self.stage_line(ui, crate::task::Kind::Badge);
                if let Some(b) = made.as_ref() {
                    paint::rule(ui, 0.0);
                    motion::swap(ui, egui::Id::new("held-badge"), motion::key_of(&b.txt), |ui| {
                        // What was made on the left; the credential's code on the right edge, under the "choose
                        // folder" key. The code sits on a plate `QR_PAD` wider on each side.
                        const QR: f32 = 116.0;
                        let plate = QR + 2.0 * paint::QR_PAD;
                        let (top, right) = (ui.cursor().top(), ui.max_rect().right());
                        let room = (ui.available_width() - plate - 20.0).max(160.0);
                        ui.allocate_ui(egui::vec2(room, 0.0), |ui| {
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = tk::S2;
                                card::flat_title(ui, t(Key::U4BadgeMade));
                                kv::kv(
                                    ui,
                                    &[
                                        (t(Key::U3Hops), Val::text(fill1(Key::CheckHopsCount, &b.hops.to_string()))),
                                        (t(Key::U4BadgeText), Val::mono(width::file_name(&b.txt.display().to_string()))),
                                        (t(Key::U4BadgeImage), Val::mono(width::file_name(&b.svg.display().to_string()))),
                                    ],
                                );
                            });
                        });
                        let at = egui::Rect::from_min_size(egui::pos2(right - plate, top), egui::vec2(plate, plate));
                        ui.scope_builder(egui::UiBuilder::new().max_rect(at), |ui| {
                            paint::qr(ui, &b.modules, QR);
                        });
                    });
                }
            });
        });
        stagger(ui, 4, |ui| {
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S2;
                fold::fold(ui, "held-more", t(Key::U3MoreOptions), |ui| {
                    field(ui, t(Key::U4UpstreamDir), None, |ui| pick_path(ui, &mut self.typed.vt_dir, crate::platform::Pick::Folder));
                    set_upstream = key::key(ui, t(Key::DoSetUpstream), Role::Secondary, true).clicked();
                });
                let mut rows: Vec<(String, Val)> = vec![
                    (t(Key::U3Issuer).to_string(), Val::mono(h.author.clone())),
                    (t(Key::U4ToMe).to_string(), Val::mono(h.grantee.clone())),
                    (t(Key::DetailPick).to_string(), Val::mono(h.id.clone())),
                    (t(Key::U3TermsHash).to_string(), Val::mono(h.terms.clone())),
                    (t(Key::ContentHash).to_string(), Val::mono(h.work.clone())),
                ];
                if let Some(k) = card.as_ref() {
                    rows.push((t(Key::U3Verdict).to_string(), Val::mono(k.verdict.clone())));
                    for (tok, st) in crate::vaultx::states(&k.checks) {
                        rows.push((tok, Val::mono(st)));
                    }
                    rows.push((t(Key::AuditLabel).to_string(), Val::mono(k.upstream_label.clone())));
                }
                let refs: Vec<(&str, Val)> = rows.iter().map(|(a, b)| (a.as_str(), b.clone())).collect();
                details(ui, "held-details", &refs);
            });
        });
        if badge {
            let out = match self.shell.home.as_ref() {
                Some(_) => {
                    let chosen = crate::home::choose(
                        &crate::home::Kind::Bundle { stem: format!("badge-{}", h.id.trim_start_matches("0x").chars().take(10).collect::<String>()) },
                        std::path::Path::new(self.typed.badge_out.trim()),
                    );
                    self.remember_landing(Out::Badge, &chosen);
                    chosen.at.display().to_string()
                }
                None => String::new(),
            };
            self.act(Action::ExportBadge { grant: h.id.clone(), out }, now);
        }
        if relicense {
            self.u4_confirm_open(U4Confirm::Relicense { grant: h.id.clone() });
        }
        if set_upstream {
            let a = Action::SetUpstream { grant: h.id.clone(), dir: self.typed.vt_dir.clone() };
            self.act(a, now);
        }
    }

    /// Sublicense (pushed from a held grant): the upstream's record, issuer and validity; then the grant form
    /// with that record fixed. Without a key or a ledger, what to do first.
    pub(super) fn relicense_page(&mut self, ui: &mut egui::Ui, now: f64) {
        let upstream = self.typed.g_upstream.trim().to_string();
        let held = self.shell.held.clone().unwrap_or_default().into_iter().find(|h| h.id.eq_ignore_ascii_case(&upstream));
        let name = self.held_record_name(&upstream);
        let issuer = held.as_ref().map(|h| self.issuer_name(&h.author)).unwrap_or_else(|| t(Key::UnnamedIssuer).to_string());
        let window = self.window_days(held.as_ref().and_then(|h| h.window));
        stagger(ui, 0, |ui| {
            card::flat(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S3;
                card_title(ui, t(Key::U4RelicenseTitle));
                kv::kv(ui, &[(t(Key::U3Work), Val::text(name.clone())), (t(Key::U3Issuer), Val::text(issuer.clone())), (t(Key::U4WindowCap), Val::text(window.clone()))]);
                details(
                    ui,
                    "relicense-page-details",
                    &[
                        (t(Key::U4Upstream), Val::mono(if upstream.is_empty() { t(Key::None_).to_string() } else { upstream.clone() })),
                        (t(Key::U3Issuer), Val::mono(held.as_ref().map(|h| h.author.clone()).unwrap_or_default())),
                    ],
                );
            });
        });
        match crate::relicx::guide(self.shell.anchor.is_some(), self.shell.rooted) {
            crate::relicx::Guide::Ready => self.grant_form(ui, Some(name), now),
            g @ crate::relicx::Guide::NeedKey => {
                states::err_box(ui, "relic-key", t(Key::U4NeedKeyWhat), t(Key::U4NeedKeyNext), t(Key::U3RawError), g.as_str());
                if key::key(ui, t(Key::U4GoKeys), Role::Secondary, true).clicked() {
                    // Making the key is a wizard step; without a passcode the wizard starts at the passcode.
                    self.wizard_open(crate::nav::Step::Key);
                }
            }
            g @ crate::relicx::Guide::NeedGenesis => {
                states::err_box(ui, "relic-genesis", t(Key::U4NeedLedgerWhat), t(Key::U4NeedLedgerNext), t(Key::U3RawError), g.as_str());
                if key::key(ui, t(Key::U4OpenWizard), Role::Secondary, true).clicked() {
                    self.wizard_open(crate::nav::Step::Genesis);
                }
            }
        }
    }
}

/// A group head on "my grants" by issuer: a mark, the issuer's name, and the issuer ledger's state.
fn group_head(ui: &mut egui::Ui, m: Mark, who: &str, state: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = tk::S2;
        ui.add_space(4.0);
        mark::mark(ui, m);
        paint::text(ui, who, Type::Strong, c(C::Ink2));
        let room = ui.available_width();
        paint::line(ui, state, Type::Note, c(C::Ink2), room);
    });
}
