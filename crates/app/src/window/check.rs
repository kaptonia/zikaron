use super::*;
use zikaron_ui::inform;

impl Win {
    pub(super) fn check_page(&mut self, ui: &mut egui::Ui, now: f64) {
        self.takes_drops();
        if self.typed.ck_endpoints.trim().is_empty() && !self.shell.endpoints.is_empty() {
            self.typed.ck_endpoints = self.shell.endpoints.iter().map(|e| e.spec()).collect::<Vec<_>>().join("\n");
        }
        if self.typed.ck_registry.trim().is_empty() {
            if let Some(r) = self.shell.settings.registry {
                self.typed.ck_registry = r.hex();
                self.typed.ck_from = self.shell.settings.from_block.to_string();
            }
        }
        let mut go = false;
        stagger(ui, 1, |ui| self.check_form(ui, now, &mut go));
        stagger(ui, 2, |ui| {
            if self.check_result(ui, now) {
                go = true;
            }
        });
        if go {
            self.start_check(now);
        }
    }

    fn check_form(&mut self, ui: &mut egui::Ui, now: f64, go: &mut bool) {
        let phase = self.phase_of(crate::task::Kind::Check);
        let reg_bad = crate::addressedx::not_address(&self.typed.ck_registry, crate::addressedx::Takes::Address);
        let ready = !self.typed.ck_typed.trim().is_empty() && !reg_bad;
        let opt = t(Key::U3Optional);
        let mono = fields::Look { mono: true, ..Default::default() };
        let mut hops: Vec<String> = self.typed.ck_ledgers.lines().map(|l| l.trim().to_string()).collect();
        if hops.is_empty() {
            hops.push(String::new());
        }
        let summary = fill1(Key::CheckHopsSay, &hops.len().to_string());
        fields::card(ui, |ui, st| {
            st.field(ui, t(Key::U3CheckPaste), None, None, |ui, w| {
                let (_, pick) = fields::joined(ui, &mut self.typed.ck_typed, t(Key::WbCheckHint), w, mono, Some(|ui: &mut egui::Ui| fields::inner_key(ui, t(Key::PickFile), None).clicked()));
                if let Some(p) = path_answer(ui.ctx(), egui::Id::new("zikaron-path-check-typed"), pick.unwrap_or(false), crate::platform::Pick::File) {
                    self.typed.ck_typed = p;
                }
            });
            zikaron_ui::fields::rows(ui, |ui, f| {
                for which in [OneStop::File, OneStop::Terms] {
                    self.check_side_row(ui, f, which, now);
                }
                let _ = f.fold(ui, "check-basis", t(Key::U3MoreOptions), &summary, |ui, f| {
                    let mut changed = false;
                    let room = (ui.available_width() * 0.5).max(80.0);
                    for (i, hop) in hops.iter_mut().enumerate() {
                        let (_, asked) = f.line(ui, None, &fill1(Key::CheckHop, &(i + 1).to_string()), "", false, false, c(C::Ink), |ui| {
                            let k = key::show_w(ui, key::Key::new(t(Key::PickFolder), Role::Secondary), 92.0).clicked();
                            inform::place(ui, hop, t(Key::PickNone), room);
                            k
                        });
                        if let Some(p) = path_answer(ui.ctx(), egui::Id::new(("zikaron-path-check-hop", i)), asked, crate::platform::Pick::Folder) {
                            *hop = p;
                            changed = true;
                        }
                    }
                    let add = f.add_row(ui, t(Key::CheckAddHop)).clicked();
                    if let Some(p) = path_answer(ui.ctx(), egui::Id::new("zikaron-path-check-add-hop"), add, crate::platform::Pick::Folder) {
                        hops.push(p);
                        changed = true;
                    }
                    if changed {
                        while hops.last().map(|h| h.is_empty()).unwrap_or(false) {
                            hops.pop();
                        }
                        self.typed.ck_ledgers = hops.join("\n");
                    }
                    f.panel(ui, |ui| {
                        fields::stack(ui, |ui, st| {
                            st.field(ui, t(Key::CheckNodes), Some(opt), None, |ui, _| input::area_hint(ui, &mut self.typed.ck_endpoints, 2, t(Key::OnePerLine)));
                            st.field(ui, t(Key::CheckRegistryFor), Some(opt), reg_bad.then(|| (t(Key::KitToWhyAddress), true)), |ui, w| fields::line(ui, &mut self.typed.ck_registry, "0x\u{2026}", w, fields::Look { bad: reg_bad, ..mono }));
                            st.pair(
                                ui,
                                |ui, st| st.field(ui, t(Key::CheckFrom), None, None, |ui, w| fields::line(ui, &mut self.typed.ck_from, t(Key::BatchSizeHint), w, mono)),
                                |ui, st| st.field(ui, t(Key::CheckAsOfSecs), Some(opt), None, |ui, w| fields::line(ui, &mut self.typed.ck_now, "", w, mono)),
                            );
                        });
                    });
                });
            });
            st.foot(ui, |ui| {
                let _page = page::Page::new();
                if key::show(ui, key::Key::new(t(Key::NavVerifyView), Role::Primary).enabled(ready).why(t(if reg_bad { Key::KitToWhyAddress } else { Key::U3CheckNotYet })).phase(phase)).clicked() {
                    *go = true;
                }
            });
        });
        self.stage_line(ui, crate::task::Kind::Check);
    }

    fn check_side_row(&mut self, ui: &mut egui::Ui, f: &mut card::Form, which: OneStop, now: f64) {
        let drop_key = match which {
            OneStop::File => Key::OsFileDrop,
            OneStop::Terms => Key::OsTermsDrop,
        };
        let held = ui.ctx().input(|i| !i.raw.dropped_files.is_empty());
        if drop::dragging(ui.ctx()) || held {
            let d = f.free(ui, |ui| drop::zone(ui, which.salt(), None, &[t(drop_key)], None, tk::INPUT_H, drop::Shape::Line, true));
            if let Some(p) = self.drop_or_pick(ui.ctx(), which.salt(), &d, crate::platform::Pick::File, now) {
                *self.ck_side(which) = p;
            }
            return;
        }
        let path = self.ck_side(which).clone();
        let title = t(which.title());
        let chosen = !path.trim().is_empty();
        let said = if chosen {
            let size = self.size_of_path(&path, now).map(size_say).unwrap_or_default();
            if size.is_empty() { width::file_name(&path) } else { format!("{} \u{b7} {}", width::file_name(&path), size) }
        } else {
            String::new()
        };
        let room = (ui.available_width() * 0.45).max(80.0);
        f.quiet_next(t(Key::U3Optional));
        let (_, (pick, remove)) = f.line(ui, None, title, "", false, false, c(C::Ink), |ui| {
            if chosen {
                let remove = key::key(ui, t(Key::KitRemove), Role::Secondary, true).clicked();
                let pick = key::key(ui, t(Key::U3TermsSwap), Role::Secondary, true).clicked();
                paint::line(ui, &said, Type::Note, c(C::Ink2), room);
                (pick, remove)
            } else {
                let pick = key::show_w(ui, key::Key::new(t(Key::PickFile), Role::Secondary), 92.0).clicked();
                inform::place(ui, "", t(Key::PickNone), room);
                (pick, false)
            }
        });
        if remove {
            self.ck_side(which).clear();
        } else if let Some(p) = path_answer(ui.ctx(), egui::Id::new(("zikaron-path-drop", which.salt())), pick, crate::platform::Pick::File) {
            *self.ck_side(which) = p;
        }
    }

    pub(super) fn start_check(&mut self, now: f64) {
        let a = self.check_action();
        self.ux.u3.check_for = Some(format!("{a:?}"));
        self.ux.u3.check_refused = match self.act(a, now) {
            Applied::Trouble(f) => Some(f),
            _ => None,
        };
    }

    fn check_result(&mut self, ui: &mut egui::Ui, now: f64) -> bool {
        let running = self.shell.tasks.in_flight(crate::task::Kind::Check);
        if running {
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S3;
                for _ in 0..3 {
                    states::skeleton_lines(ui, &[0.2, 0.62]);
                }
                states::skeleton_lines(ui, &[0.7, 0.55, 0.8, 0.45, 0.6]);
            });
            return false;
        }
        if self.ux.u3.check_for.as_deref().is_some_and(|a| a != format!("{:?}", self.check_action())) {
            return false;
        }
        if let Some(f) = self.ux.u3.check_refused.clone().or_else(|| self.shell.failed.get(&crate::task::Kind::Check).cloned()) {
            card::form(ui, |ui, form| form.summary(ui, Glyph::No, card::Tint::Bad, t(Key::U3CheckFailed), None, f.human(), |_| ()));
            let shown = f.shown();
            if !shown.is_empty() {
                details_card(ui, "check-err", &[(t(Key::U3RawError), Val::mono(shown))]);
            }
            return false;
        }
        let Some(x) = self.shell.checked.clone() else {
            return false;
        };
        let x = crate::checkx::as_shown(&x, &self.shell.audits.state(&x.audit_key));
        let mut go = false;
        let mut go_settings = false;
        let grant_tone = crate::checkx::tone(&x.judged.verdict);
        let (grant_pill, grant_badge) = match grant_tone {
            crate::checkx::Tone::Green => (Key::BadgeLive, PillTone::Ok),
            crate::checkx::Tone::Red => (Key::OsInvalid, PillTone::Bad),
            crate::checkx::Tone::Amber | crate::checkx::Tone::Grey => (Key::OsOpen, PillTone::Warn),
        };
        let (verdict, glyph, tint) = match crate::checkx::summary(&x) {
            crate::checkx::Summary::AllMatch => (t(Key::U4NoMismatch).to_string(), Glyph::Ok, card::Tint::Ok),
            crate::checkx::Summary::Mismatched(n) => (fill1(Key::OsMismatchN, &n.to_string()), Glyph::No, card::Tint::Bad),
            crate::checkx::Summary::Incomplete => (t(Key::OsIncomplete).to_string(), Glyph::Warn, card::Tint::Warn),
            crate::checkx::Summary::Undecided => (t(Key::CheckUndecided).to_string(), Glyph::Warn, card::Tint::Warn),
        };
        let side_line = |side: &crate::checkx::Side| match side {
            crate::checkx::Side::Refused(f) => f.human().to_string(),
            _ => side_face(side).0.unwrap_or_default(),
        };
        let mut gaps: Vec<crate::checkx::Gap> = Vec::new();
        for (i, h) in x.judged.hops.iter().enumerate() {
            for (tok, state) in &h.lights {
                if let Some(g) = crate::checkx::gap(&x, i, tok, state) {
                    if !gaps.iter().any(|y| std::mem::discriminant(y) == std::mem::discriminant(&g)) {
                        gaps.push(g);
                    }
                }
            }
        }
        let last = x.judged.hops.last();
        let record = last.map(|h| self.record_name_of(&h.work, x.record.as_deref())).unwrap_or_default();
        let grantor = last.map(|h| card::short(&h.author)).unwrap_or_default();
        let window = last.map(|h| self.window_days(h.window)).unwrap_or_default();
        let undecided_last = x.undecided.iter().any(|(i, _)| *i + 1 == x.judged.hops.len());
        let first = last.map(|h| if undecided_last { t(Key::FirstAnchorNotRead).to_string() } else { first_anchor_say(h.anchored_at) }).unwrap_or_default();
        let take_why = x.takes.as_ref().map(|(_, plan)| super::verify::extract_why(plan));
        let mut take = false;
        stagger(ui, 3, |ui| {
            card::form(ui, |ui, f| {
                take = f.summary(ui, glyph, tint, &verdict, None, &record, |ui| match take_why {
                    Some(why) => key::show(ui, key::Key::new(t(Key::DoExtract), Role::Secondary).enabled(why.is_none()).why(why.unwrap_or(""))).clicked(),
                    None => false,
                });
                if last.is_some() {
                    f.facts(ui, &[(t(Key::OsGrantor), card::Fact::Mono(&grantor)), (t(Key::U3Window), card::Fact::Text(&window)), (t(Key::U4RecordFirstAt), card::Fact::Mono(&first))]);
                }
            });
        });
        if let (true, Some((path, plan))) = (take, x.takes.as_ref()) {
            self.extract_open_from(ExtractFrom { path: path.clone(), takes: plan.clone(), names: Vec::new() });
        }
        stagger(ui, 4, |ui| {
            card::section(ui, t(Key::OsSixChecks), "", |ui| {
                card::form(ui, |ui, f| {
                    if grant_tone != crate::checkx::Tone::Green {
                        let _ = f.line(ui, None, t(Key::KindGrant), verdict_human(&x.judged.verdict), false, false, c(C::Ink), |ui| mark::pill(ui, t(grant_pill), grant_badge));
                    }
                    for (side, name) in [(&x.file, Key::U3File), (&x.terms, Key::OsTerms)] {
                        if !matches!(side, crate::checkx::Side::NotGiven) {
                            let (_, p, tone) = side_face(side);
                            let _ = f.line(ui, None, t(name), &side_line(side), false, false, c(C::Ink), |ui| mark::pill(ui, t(p), tone));
                        }
                    }
                    for g in &gaps {
                        if *g == crate::checkx::Gap::NoNode {
                            go_settings |= f.act(ui, t(gap_note(g)), "", key::Key::new(t(Key::CheckGoSettings), Role::Secondary)).clicked();
                        } else {
                            f.note(ui, t(gap_note(g)));
                        }
                    }
                    for m in &x.missed {
                        let _ = f.info(ui, &m.name, card::Show::Mark(Mark::Warn, t(m.key())));
                    }
                    for r in &x.refused {
                        let _ = f.info(ui, &width::file_name(&r.file), card::Show::Mark(Mark::Warn, &r.human()));
                    }
                    let several = x.judged.hops.len() > 1;
                    for (i, h) in x.judged.hops.iter().enumerate() {
                        if several {
                            let head = fill3(Key::U3HopHead, &(i + 1).to_string(), &self.issuer_name(&h.author), verdict_human(&h.verdict));
                            let _ = f.row_with(ui, Some(tone_mark(crate::checkx::tone(&h.verdict))), &head, "", card::Value::None, |_| {});
                        } else if i + 1 < x.judged.hops.len() {
                            continue;
                        }
                        for (tok, state) in &h.lights {
                            let gap = crate::checkx::gap(&x, i, tok, state).map(|g| gap_words(&g)).unwrap_or_default();
                            let _ = f.info(ui, t(u3_check_name(tok)), card::Show::Mark(state_mark(state), &gap));
                        }
                    }
                    if several {
                        if let Some(ch) = x.judged.chain.as_ref() {
                            for (k, l) in ch.links.iter().enumerate() {
                                let m = match l {
                                    Some(true) => Mark::Ok,
                                    Some(false) => Mark::Bad,
                                    None => Mark::Todo,
                                };
                                let _ = f.info(ui, &fill2(Key::U3LinkRow, &(k + 1).to_string(), &(k + 2).to_string()), card::Show::Mark(m, ""));
                            }
                        }
                    }
                });
            });
        });
        let mut audit = false;
        stagger(ui, 5, |ui| {
            card::section(ui, t(Key::AuditSection), "", |ui| {
                card::form(ui, |ui, f| audit = audit_rows(ui, f, &self.shell.audits.state(&x.audit_key)));
            });
        });
        if audit {
            self.act(Action::AuditCheck, now);
        }
        stagger(ui, 6, |ui| {
            card::section(ui, t(Key::LedgerSources), "", |ui| {
                card::form(ui, |ui, f| {
                    for i in 0..x.judged.hops.len() {
                        self.check_source(ui, f, &x, i, &mut go);
                    }
                });
            });
        });
        let several = x.judged.hops.len() > 1;
        let mut rows: Vec<(String, Val)> = Vec::new();
        if let Some(h) = last {
            rows.push((t(Key::OsGrantor).to_string(), Val::mono(h.author.clone())));
            rows.push((t(Key::U3ToWhom).to_string(), Val::mono(h.grantee.clone())));
            rows.push((t(Key::OsWork).to_string(), Val::mono(h.work.clone())));
        }
        rows.push((t(Key::U3Verdict).to_string(), Val::text(verdict_human(&x.judged.verdict).to_string())));
        if let Ok(b) = &x.basis {
            rows.push((t(Key::BasisRead).to_string(), Val::mono(format!("{} \u{b7} {} \u{b7} {}\u{2013}{}", b.chain, b.registry, b.from_block, b.to_block))));
        }
        for (i, n) in x.notes.per.iter().enumerate() {
            if let Some(n) = n {
                let label = if several { format!("{} \u{b7} {}", fill1(Key::CheckHop, &(i + 1).to_string()), t(Key::NoticesBasis)) } else { t(Key::NoticesBasis).to_string() };
                rows.push((label, Val::mono(crate::noticex::basis_line(n))));
            }
        }
        if !x.refused.is_empty() {
            rows.push((t(Key::RefusedFiles).to_string(), Val::text(x.refused.iter().map(|r| format!("{} \u{b7} {}", width::file_name(&r.file), r.human())).collect::<Vec<_>>().join("\n"))));
        }
        for (side, name) in [(&x.file, Key::U3File), (&x.terms, Key::OsTerms)] {
            if let crate::checkx::Side::Refused(f) = side {
                rows.push((t(name).to_string(), Val::mono(f.shown())));
            }
        }
        rows.push((t(Key::EquivalentVerb).to_string(), Val::mono(x.verb().to_string())));
        for (i, h) in x.judged.hops.iter().enumerate() {
            for (tok, state) in &h.lights {
                let name = t(u3_check_name(tok));
                let label = if several { format!("{} \u{b7} {name}", fill1(Key::CheckHop, &(i + 1).to_string())) } else { name.to_string() };
                rows.push((label, Val::text(state_human(state).to_string())));
            }
        }
        if let Some(ch) = x.judged.chain.as_ref() {
            let at = ch.failing_index.map(|i| fill1(Key::CheckHop, &(i + 1).to_string()));
            rows.push((t(Key::U3Hops).to_string(), Val::text([Some(verdict_human(&ch.verdict).to_string()), at].into_iter().flatten().collect::<Vec<_>>().join(" \u{b7} "))));
        }
        for (which, side) in [(OneStop::File, &x.file), (OneStop::Terms, &x.terms)] {
            if let crate::checkx::Side::Compared(d) = side {
                rows.push((t(which.title()).to_string(), Val::mono(format!("{} \u{b7} {} \u{b7} {}", d.bytes, d.got_hex(), d.want_hex()))));
            }
        }
        let refs: Vec<(&str, Val)> = rows.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        stagger(ui, 7, |ui| details_card(ui, "check-evidence", &refs));
        if go_settings {
            self.go(Place::Settings(Section::Network), now);
        }
        go
    }

}

pub(super) fn audit_rows(ui: &mut egui::Ui, f: &mut card::Form, state: &crate::sendx::behind::State<crate::checkx::Audit, crate::fault::Fault>) -> bool {
    use crate::sendx::behind::State;
    let room = (ui.available_width() * 0.5).max(80.0);
    match state {
        State::Unaudited => {
            let (_, go) = f.line(ui, None, t(Key::AuditWhole), "", false, false, c(C::Ink), |ui| {
                let go = key::show_w(ui, key::Key::new(t(Key::AuditDo), Role::Secondary), 92.0).clicked();
                paint::line(ui, t(Key::AuditNone), Type::Note, c(C::Ink2), room);
                go
            });
            go
        }
        State::Auditing | State::Before(_) => {
            let _ = f.info(ui, t(Key::AuditWhole), card::Show::Mark(Mark::Work, t(Key::AuditRunning)));
            false
        }
        State::Failed(why) => {
            let (_, go) = f.line(ui, None, t(Key::AuditWhole), "", false, false, c(C::Ink), |ui| {
                let go = key::show_w(ui, key::Key::new(t(Key::AuditAgain), Role::Secondary), 92.0).clicked();
                paint::line(ui, &format!("{} \u{b7} {}", t(Key::PsNotReadPill), why.human()), Type::Note, c(C::Ink2), room);
                go
            });
            go
        }
        State::Done(a) => {
            let (words, tone) = match crate::checkx::tone(&a.verdict) {
                crate::checkx::Tone::Green => (Key::BadgeLive, PillTone::Ok),
                crate::checkx::Tone::Red => (Key::OsInvalid, PillTone::Bad),
                crate::checkx::Tone::Amber | crate::checkx::Tone::Grey => (Key::OsOpen, PillTone::Warn),
            };
            if !a.verdict.is_empty() {
                let _ = f.info(ui, t(Key::U3Verdict), card::Show::Pill(t(words), tone));
            }
            if !a.label.is_empty() {
                use zikaron::v2::tokens::Label;
                let tone = if a.label == Label::Complete.as_str() {
                    PillTone::Ok
                } else if a.label == Label::BrokenChain.as_str() {
                    PillTone::Bad
                } else {
                    PillTone::Warn
                };
                let _ = f.line(ui, None, t(Key::WbAudit), "", false, false, c(C::Ink), |ui| {
                    paint::line(ui, &fill1(Key::U3EntriesCount, &a.entries.to_string()), Type::Note, c(C::Ink2), room);
                    mark::pill(ui, &label_human(&a.label), tone)
                });
            }
            let at = a.at.map(crate::when::when).unwrap_or_default();
            let to = fill2(Key::AuditToAt, &at, &a.block.to_string());
            let (_, go) = f.line(ui, None, t(Key::AuditTo), "", false, false, c(C::Ink), |ui| {
                let go = key::show_w(ui, key::Key::new(t(Key::AuditAgain), Role::Secondary), 92.0).clicked();
                paint::line(ui, &to, Type::Mono, c(C::Ink), room);
                go
            });
            go
        }
    }
}

impl Win {
    fn check_source(&mut self, ui: &mut egui::Ui, f: &mut card::Form, x: &crate::checkx::Checked, hop: usize, go: &mut bool) {
        let src = x.found.get(hop).cloned().unwrap_or_default();
        let (said, open_key, m) = match &src.from {
            Some((level, _)) => (t(level_key(*level)).to_string(), Key::U3TermsSwap, Mark::Ok),
            None => (t(Key::Nothing).to_string(), Key::ReadNetAdd, Mark::Warn),
        };
        let title = fill1(Key::CheckSourceLine, &said);
        let settled = crate::checkx::source_settled(x, hop);
        let mut open = !settled && self.ux.u3.ck_source_open == Some(hop);
        let remote = match (&src.from, src.files) {
            (Some((crate::supplyx::Level::Remote, url)), Some(n)) => Some(fill2(Key::CheckFromAddress, url, &n.to_string())),
            _ => None,
        };
        let open_k = key::Key::new(t(open_key), Role::Secondary);
        let again_k = key::Key::new(t(Key::CheckRefetch), Role::Secondary);
        let kw = key::size_of(ui, &open_k).x.max(if remote.is_some() { key::size_of(ui, &again_k).x } else { 0.0 });
        let mut hit = false;
        let _ = f.row_with(ui, Some(m), &title, "", card::Value::None, |ui| {
            if !settled {
                hit = key::show_w(ui, open_k, kw).clicked();
            }
        });
        if hit {
            open = !open;
        }
        if let Some(r) = &remote {
            let _ = f.row_with(ui, None, r, "", card::Value::None, |ui| {
                if key::show_w(ui, again_k, kw).clicked() {
                    *go = true;
                }
            });
        }
        for (level, miss) in &src.misses {
            let _ = f.row_with(ui, Some(Mark::Warn), &fill2(Key::CheckMissRow, t(level_key(*level)), miss.human()), "", card::Value::None, |_| ());
        }
        self.ux.u3.ck_source_open = if open { Some(hop) } else if self.ux.u3.ck_source_open == Some(hop) { None } else { self.ux.u3.ck_source_open };
        if open {
            let mut lines: Vec<String> = self.typed.ck_ledgers.lines().map(|l| l.to_string()).collect();
            while lines.len() <= hop {
                lines.push(String::new());
            }
            let mut changed = false;
            f.panel(ui, |ui| {
                fields::stack(ui, |ui, st| {
                    st.field(ui, t(Key::CheckIssuerLedger), None, None, |ui, w| {
                        let mono = fields::Look { mono: true, ..Default::default() };
                        let (resp, pick_hit) = fields::joined(ui, &mut lines[hop], t(Key::CheckIssuerLedgerHint), w, mono, Some(|ui: &mut egui::Ui| fields::inner_key(ui, pick_label(crate::platform::Pick::FileOrFolder), None).clicked()));
                        changed |= resp.changed();
                        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !lines[hop].trim().is_empty() {
                            *go = true;
                        }
                        if let Some(p) = path_answer(ui.ctx(), egui::Id::new(("zikaron-path-issuer-ledger", hop)), pick_hit.unwrap_or(false), crate::platform::Pick::FileOrFolder) {
                            lines[hop] = p;
                            changed = true;
                            *go = true;
                        }
                    });
                });
            });
            if changed {
                while lines.last().map(|h| h.trim().is_empty()).unwrap_or(false) {
                    lines.pop();
                }
                self.typed.ck_ledgers = lines.join("\n");
            }
        }
    }

    pub(super) fn record_name_of(&self, work: &str, read: Option<&str>) -> String {
        let read = read.map(str::to_string).filter(|n| !n.trim().is_empty());
        let mine = read.or_else(|| self.shell.rows.as_ref().and_then(|(rows, _)| {
            rows.iter()
                .filter(|r| r.kind == zikaron_glue::book::Kind::History)
                .find(|r| r.work.as_deref().map(|w| w.eq_ignore_ascii_case(work)).unwrap_or(false))
                .and_then(|r| r.facts.record_name())
        }));
        let noted = || {
            let held = self.shell.held.as_ref()?.iter().find(|h| h.work.eq_ignore_ascii_case(work))?;
            let g = crate::lastread::grant_form(&held.id);
            self.shell.settings.grant_notes.iter().find(|(x, n)| *x == g && !n.trim().is_empty()).map(|(_, n)| n.clone())
        };
        mine.or_else(noted)
            .or_else(|| {
                self.shell.cards.as_ref().and_then(|(cards, _)| cards.iter().find(|c| c.work.eq_ignore_ascii_case(work)).and_then(|c| c.record_name.clone()))
            })
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| t(Key::UnnamedRecord).to_string())
    }

    pub(super) fn files_of(&mut self, id: &str, now: f64) -> Vec<String> {
        let table = self.shell.rows_gen;
        let held = self.ux.files_of.get(id).cloned();
        if let Some((at, v)) = &held {
            if *at == table {
                return v.clone();
            }
        }
        match self.peek(crate::peekx::Ask::Files { id: id.to_string(), table }, now) {
            Some(crate::peekx::Got::Files(names)) => {
                self.ux.files_of.insert(id.to_string(), (table, names.clone()));
                names
            }
            _ => held.map(|(_, v)| v).unwrap_or_default(),
        }
    }

    pub(super) fn size_of_path(&mut self, path: &str, now: f64) -> Option<u64> {
        if let Some(n) = self.ux.sizes.get(path) {
            return *n;
        }
        match self.peek(crate::peekx::Ask::Size { path: path.to_string() }, now) {
            Some(crate::peekx::Got::Size(n)) => {
                self.ux.sizes.insert(path.to_string(), n);
                n
            }
            _ => None,
        }
    }
}
