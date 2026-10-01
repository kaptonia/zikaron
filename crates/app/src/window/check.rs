//! Verify a grant in one go: the grant, and optionally the file and the terms held against it. The form on
//! the left, the result beside it (under it when narrow).

use super::*;

impl Win {
    pub(super) fn check_page(&mut self, ui: &mut egui::Ui, now: f64) {
        // An empty basis borrows the home's nodes and registry (shown, so the person sees what the verdict
        // stands on).
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
        card::form_and_result(ui, tk::SHEET_W, 360.0, |ui, side| match side {
            card::Side::Main => stagger(ui, 0, |ui| {
                card::card(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = tk::S4;
                    field(ui, t(Key::KindGrant), None, |ui| {
                        let mut pick = false;
                        width::then(
                            ui,
                            |ui| pick = key::key(ui, t(Key::PickFile), Role::Secondary, true).clicked(),
                            |ui, room| input::field(ui, &mut self.typed.ck_typed, t(Key::U3CheckPaste), room, input::Look { mono: true, ..Default::default() }),
                        );
                        if pick {
                            if let Some(p) = crate::platform::choose_path(crate::platform::Pick::File) {
                                self.typed.ck_typed = p;
                            }
                        }
                    });
                    for (which, drop_key) in [(OneStop::File, Key::OsFileDrop), (OneStop::Terms, Key::OsTermsDrop)] {
                        field(ui, t(which.title()), Some(t(Key::U3Optional)), |ui| {
                            let path = self.ck_side(which).clone();
                            if path.trim().is_empty() {
                                let d = drop::zone(ui, which.salt(), None, &[t(drop_key)], None, 56.0, drop::Shape::Line, true);
                                if let Some(p) = self.drop_or_pick(&d, crate::platform::Pick::File, now) {
                                    *self.ck_side(which) = p;
                                }
                            } else {
                                let size = self.size_of_path(&path).map(size_say).unwrap_or_default();
                                if drop::file_row(ui, which.salt(), &width::file_name(&path), &size, false, t(Key::KitRemove)) {
                                    self.ck_side(which).clear();
                                }
                            }
                        });
                    }
                    fold::fold(ui, "check-basis", t(Key::U3MoreOptions), |ui| {
                        ui.spacing_mut().item_spacing.y = tk::S3;
                        field(ui, t(Key::CheckLedgers), None, |ui| {
                            // Line i is hop i; a cleared hop keeps its empty line; empty lines at the end go.
                            let mut hops: Vec<String> = self.typed.ck_ledgers.lines().map(|l| l.trim().to_string()).collect();
                            if hops.is_empty() {
                                hops.push(String::new());
                            }
                            let mut changed = false;
                            for (i, hop) in hops.iter_mut().enumerate() {
                                ui.push_id(("check-hop", i), |ui| {
                                    if path_row(ui, &fill1(Key::CheckHop, &(i + 1).to_string()), hop, t(Key::PickFolder), t(Key::PickNone)) {
                                        if let Some(p) = crate::platform::choose_path(crate::platform::Pick::Folder) {
                                            *hop = p;
                                            changed = true;
                                        }
                                    }
                                });
                            }
                            if key::key(ui, t(Key::CheckAddHop), Role::Secondary, true).clicked() {
                                if let Some(p) = crate::platform::choose_path(crate::platform::Pick::Folder) {
                                    hops.push(p);
                                    changed = true;
                                }
                            }
                            if changed {
                                while hops.last().map(|h| h.is_empty()).unwrap_or(false) {
                                    hops.pop();
                                }
                                self.typed.ck_ledgers = hops.join("\n");
                            }
                        });
                        field(ui, t(Key::CheckEndpoints), None, |ui| input::area(ui, &mut self.typed.ck_endpoints, 2));
                        field(ui, t(Key::CheckRegistry), None, |ui| input::mono(ui, &mut self.typed.ck_registry, "0x\u{2026}"));
                        card::grid(ui, "check-numbers", 2, 150.0, |ui, i| {
                            if i == 0 {
                                field(ui, t(Key::CheckFrom), None, |ui| input::mono(ui, &mut self.typed.ck_from, t(Key::BatchSizeHint)));
                            } else {
                                field(ui, t(Key::CheckNow), None, |ui| input::mono(ui, &mut self.typed.ck_now, t(Key::BatchSizeHint)));
                            }
                        });
                    });
                    let _page = page::Page::new();
                    if key::show(ui, key::Key::new(t(Key::NavVerifyView), Role::Primary).enabled(!self.typed.ck_typed.trim().is_empty()).phase(self.phase_of(crate::task::Kind::Check))).clicked() {
                        go = true;
                    }
                    self.stage_line(ui, crate::task::Kind::Check);
                });
            }),
            card::Side::Side => stagger(ui, 1, |ui| {
                if self.check_result(ui, now) {
                    go = true;
                }
            }),
        });
        if go {
            self.start_check(now);
        }
    }

    /// Start a check from whatever page asks (this page, or home's quick box): a refusal on the spot is this
    /// press's own result and replaces the last one; a started task records its landing by kind.
    pub(super) fn start_check(&mut self, now: f64) {
        let a = self.check_action();
        self.ux.u3.check_refused = match self.act(a, now) {
            Applied::Trouble(f) => Some(f),
            _ => None,
        };
    }

    /// The result card: empty before the first check, shimmering while one runs, the error box on a failure,
    /// else the three answers, the grant's facts, where each hop's ledger came from, the six checks and the
    /// raw values folded, and the conclusion. Returns whether "check again" was asked.
    fn check_result(&mut self, ui: &mut egui::Ui, now: f64) -> bool {
        let running = self.shell.tasks.in_flight(crate::task::Kind::Check);
        if running {
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S3;
                for _ in 0..3 {
                    states::skeleton_lines(ui, &[0.2, 0.62]);
                }
                paint::rule(ui, 0.0);
                states::skeleton_lines(ui, &[0.7, 0.55, 0.8, 0.45, 0.6]);
            });
            return false;
        }
        // Only this kind's own failures (another task's failure is not borrowed onto this page).
        if let Some(f) = self.ux.u3.check_refused.clone().or_else(|| self.shell.failed.get(&crate::task::Kind::Check).cloned()) {
            card::card(ui, |ui| states::err_box(ui, "check-err", t(Key::U3CheckFailed), t(Key::U3CheckFailedNext), t(Key::U3RawError), &f.raw()));
            return false;
        }
        let Some(x) = self.shell.checked.clone() else {
            card::card(ui, |ui| states::empty(ui, Glyph::Verify, t(Key::U3CheckNotYet)));
            return false;
        };
        let mut go = false;
        let mut go_settings = false;
        card::card(ui, |ui| {
            ui.spacing_mut().item_spacing.y = tk::S3;
            let grant_tone = crate::checkx::tone(&x.judged.verdict);
            let (grant_pill, grant_badge) = match grant_tone {
                crate::checkx::Tone::Green => (Key::BadgeLive, PillTone::Ok),
                crate::checkx::Tone::Red => (Key::OsInvalid, PillTone::Bad),
                crate::checkx::Tone::Amber | crate::checkx::Tone::Grey => (Key::OsOpen, PillTone::Warn),
            };
            let grant_say = if grant_tone == crate::checkx::Tone::Green { t(Key::OsSixPass) } else { verdict_human(&x.judged.verdict) };
            let (fs, fp, ft) = side_face(&x.file);
            let (ts, tp, tt) = side_face(&x.terms);
            let fs = fs.unwrap_or_default();
            let ts = ts.unwrap_or_default();
            states::answers(
                ui,
                &[
                    states::Answer { title: t(Key::KindGrant), detail: grant_say, pill: t(grant_pill), tone: grant_badge },
                    states::Answer { title: t(Key::U3File), detail: &fs, pill: t(fp), tone: ft },
                    states::Answer { title: t(Key::OsTerms), detail: &ts, pill: t(tp), tone: tt },
                ],
            );
            paint::rule(ui, 0.0);
            if let Some(h) = x.judged.hops.last() {
                kv::kv(
                    ui,
                    &[
                        (t(Key::OsWork), Val::text(self.record_name_of(&h.work))),
                        (t(Key::U3Window), Val::text(self.window_days(h.window))),
                        (t(Key::U4RecordFirstAt), Val::mono(first_anchor_say(h.anchored_at))),
                    ],
                );
            }
            for i in 0..x.judged.hops.len() {
                self.check_source(ui, &x, i, &mut go);
            }
            fold::fold(ui, "check-six", t(Key::OsSixChecks), |ui| {
                // Several hops: each hop's six checks, then one row per link.
                if x.judged.hops.len() > 1 {
                    for (i, h) in x.judged.hops.iter().enumerate() {
                        states::okline(ui, tone_mark(crate::checkx::tone(&h.verdict)), &fill3(Key::U3HopHead, &(i + 1).to_string(), &self.issuer_name(&h.author), verdict_human(&h.verdict)));
                        let rows: Vec<(Mark, String, Option<String>)> = h.lights.iter().map(|(tok, state)| (state_mark(state), t(u3_check_name(tok)).to_string(), crate::checkx::gap(&x, i, tok, state).map(|g| gap_words(&g)))).collect();
                        states::checks(ui, &rows, true);
                    }
                    if let Some(ch) = x.judged.chain.as_ref() {
                        let links: Vec<(Mark, String, Option<String>)> = ch
                            .links
                            .iter()
                            .enumerate()
                            .map(|(k, l)| {
                                let m = match l {
                                    Some(true) => Mark::Ok,
                                    Some(false) => Mark::Bad,
                                    None => Mark::Todo,
                                };
                                (m, fill2(Key::U3LinkRow, &(k + 1).to_string(), &(k + 2).to_string()), None)
                            })
                            .collect();
                        states::checks(ui, &links, false);
                    }
                } else if let Some(h) = x.judged.hops.last() {
                    let rows: Vec<(Mark, String, Option<String>)> =
                        h.lights.iter().map(|(tok, state)| (state_mark(state), t(u3_check_name(tok)).to_string(), crate::checkx::gap(&x, 0, tok, state).map(|g| gap_words(&g)))).collect();
                    states::checks(ui, &rows, true);
                }
            });
            // One sentence per kind of gap (missing ledger, no node, chain unread, waiting to be anchored).
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
            for g in &gaps {
                states::note_box(ui, t(gap_note(g)));
            }
            if gaps.contains(&crate::checkx::Gap::NoNode) && key::link(ui, t(Key::CheckGoSettings)).clicked() {
                go_settings = true;
            }
            for r in &x.refused {
                hint(ui, &fill2(Key::CheckRejectedRow, &width::file_name(&r.file), &r.human()));
            }
            for side in [&x.file, &x.terms] {
                if let crate::checkx::Side::Refused(f) = side {
                    hint(ui, f.said());
                }
            }
            fold::fold(ui, "check-evidence", t(Key::SetEvidence), |ui| {
                let mut rows: Vec<(String, Val)> = Vec::new();
                if let Some(h) = x.judged.hops.last() {
                    rows.push((t(Key::OsGrantor).to_string(), Val::mono(h.author.clone())));
                    rows.push((t(Key::U3ToWhom).to_string(), Val::mono(h.grantee.clone())));
                    rows.push((t(Key::OsWork).to_string(), Val::mono(h.work.clone())));
                }
                rows.push((t(Key::U3Verdict).to_string(), Val::mono(x.judged.verdict.clone())));
                rows.push((t(Key::EquivalentVerb).to_string(), Val::mono(x.verb().to_string())));
                for h in &x.judged.hops {
                    for (tok, state) in &h.lights {
                        rows.push((tok.clone(), Val::mono(state.clone())));
                    }
                }
                if let Some(ch) = x.judged.chain.as_ref() {
                    rows.push((t(Key::U3Verdict).to_string(), Val::mono(format!("{} {} {}", ch.verdict, ch.token, ch.failing_kind))));
                }
                for (which, side) in [(OneStop::File, &x.file), (OneStop::Terms, &x.terms)] {
                    if let crate::checkx::Side::Compared(d) = side {
                        rows.push((t(which.title()).to_string(), Val::mono(format!("{} \u{b7} {} \u{b7} {}", d.bytes, d.got_hex(), d.want_hex()))));
                    }
                }
                let refs: Vec<(&str, Val)> = rows.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
                kv::kv(ui, &refs);
            });
            paint::rule(ui, 0.0);
            match crate::checkx::summary(&x) {
                crate::checkx::Summary::AllMatch => states::banner(ui, states::Banner::Ok, t(Key::U4NoMismatch), |_| ()),
                crate::checkx::Summary::Mismatched(n) => states::banner(ui, states::Banner::Bad, &fill1(Key::OsMismatchN, &n.to_string()), |_| ()),
                crate::checkx::Summary::Incomplete => states::note_box(ui, t(Key::OsIncomplete)),
            }
        });
        if go_settings {
            self.go(Place::Settings(Section::Network), now);
        }
        go
    }

    /// A hop's ledger source: "ledger source: this Mac" with "replace…", or "none" with "add…"; from a
    /// publish address, the address, the file count, "fetch again". "Add…/replace…" opens the issuer ledger
    /// field for that hop (line i of the advanced hop list).
    fn check_source(&mut self, ui: &mut egui::Ui, x: &crate::checkx::Checked, hop: usize, go: &mut bool) {
        let src = x.found.get(hop).cloned().unwrap_or_default();
        let (said, open_key, m) = match &src.from {
            Some((level, _)) => (t(level_key(*level)).to_string(), Key::U3TermsSwap, Mark::Ok),
            None => (t(Key::Nothing).to_string(), Key::CheckSourceAdd, Mark::Warn),
        };
        // Supplied here and all six green: nothing to fill, so no "replace…".
        let settled = crate::checkx::source_settled(x, hop);
        let mut open = !settled && self.ux.u3.ck_source_open == Some(hop);
        paint::rule(ui, 0.0);
        let (_, hit) = width::then_line(
            ui,
            |ui| !settled && key::link(ui, t(open_key)).clicked(),
            |ui, room| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    mark::mark(ui, m);
                    paint::line(ui, &fill1(Key::CheckSourceLine, &said), Type::Body, c(C::Ink), room - 28.0);
                });
            },
        );
        if hit {
            open = !open;
        }
        if let (Some((crate::supplyx::Level::Remote, url)), Some(n)) = (&src.from, src.files) {
            let (_, again) = width::then_line(ui, |ui| key::link(ui, t(Key::CheckRefetch)).clicked(), |ui, room| paint::line(ui, &fill2(Key::CheckFromAddress, url, &n.to_string()), Type::Small, c(C::Ink2), room));
            if again {
                *go = true;
            }
        }
        for (level, f) in &src.misses {
            hint(ui, &fill2(Key::CheckMissRow, t(level_key(*level)), f.human()));
        }
        self.ux.u3.ck_source_open = if open { Some(hop) } else if self.ux.u3.ck_source_open == Some(hop) { None } else { self.ux.u3.ck_source_open };
        if open {
            let mut lines: Vec<String> = self.typed.ck_ledgers.lines().map(|l| l.to_string()).collect();
            while lines.len() <= hop {
                lines.push(String::new());
            }
            let can_check = !lines[hop].trim().is_empty();
            let mut changed = false;
            field(ui, t(Key::CheckIssuerLedger), None, |ui| {
                let (resp, (check_hit, pick_hit)) = width::line_then(ui, &mut lines[hop], t(Key::CheckIssuerLedgerHint), true, |ui| {
                    let check_hit = key::key(ui, t(Key::DoCheck), Role::Secondary, can_check).clicked();
                    let pick_hit = key::key(ui, t(Key::PickFolder), Role::Secondary, true).clicked();
                    (check_hit, pick_hit)
                });
                changed |= resp.changed();
                if pick_hit {
                    if let Some(p) = crate::platform::choose_path(crate::platform::Pick::FileOrFolder) {
                        lines[hop] = p;
                        changed = true;
                    }
                }
                if check_hit {
                    *go = true;
                }
            });
            if changed {
                while lines.last().map(|h| h.trim().is_empty()).unwrap_or(false) {
                    lines.pop();
                }
                self.typed.ck_ledgers = lines.join("\n");
            }
        }
    }

    /// A record named by its content fingerprint: this ledger's name for it, else the issuer ledger's (read
    /// with the held grants), else "unnamed record".
    pub(super) fn record_name_of(&self, work: &str) -> String {
        let mine = self.shell.rows.as_ref().and_then(|(rows, _)| {
            rows.iter()
                .filter(|r| r.kind == zikaron::tokens::EntryType::History)
                .find(|r| r.work.as_deref().map(|w| w.eq_ignore_ascii_case(work)).unwrap_or(false))
                .and_then(|r| r.facts.note.clone())
        });
        // The person's own name for a held grant on this record, kept on this machine.
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

    /// The file names a record was signed from, from this Mac's records index (read once per entry; an
    /// entry signed elsewhere or from a folder has none).
    pub(super) fn files_of(&mut self, id: &str) -> Vec<String> {
        if let Some(v) = self.ux.files_of.get(id) {
            return v.clone();
        }
        let names: Vec<String> = crate::home::machine_dir()
            .and_then(|m| crate::recordsx::read(&m))
            .map(|rows| rows.into_iter().filter(|r| r.id.eq_ignore_ascii_case(id)).map(|r| r.name).collect())
            .unwrap_or_default();
        self.ux.files_of.insert(id.to_string(), names.clone());
        names
    }

    /// A file's size, read once per path (a chosen file shows its name and size).
    pub(super) fn size_of_path(&mut self, path: &str) -> Option<u64> {
        if let Some(n) = self.ux.sizes.get(path) {
            return *n;
        }
        let n = crate::anchorx::file_size(std::path::Path::new(path));
        self.ux.sizes.insert(path.to_string(), n);
        n
    }
}
