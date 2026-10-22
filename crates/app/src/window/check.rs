//! Verify a grant in one go: the grant, and optionally the file and the terms held against it. The form on
//! top, the result under it.

use super::*;
use zikaron_ui::inform;

impl Win {
    pub(super) fn check_page(&mut self, ui: &mut egui::Ui, now: f64) {
        // Its file and terms rows take dropped files.
        self.takes_drops();
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

    /// The form: the grant code (pasted, or a grant file chosen inside its field), the file and the terms as
    /// rows (optional), the basis under advanced options (each hop's ledger folder a row, "add next hop", then
    /// the nodes, the registry, the block and time to check from), and the key at its foot.
    fn check_form(&mut self, ui: &mut egui::Ui, now: f64, go: &mut bool) {
        let phase = self.phase_of(crate::task::Kind::Check);
        let ready = !self.typed.ck_typed.trim().is_empty();
        let opt = t(Key::U3Optional);
        let mono = fields::Look { mono: true, ..Default::default() };
        // Line i is hop i; a cleared hop keeps its empty line; empty lines at the end go.
        let mut hops: Vec<String> = self.typed.ck_ledgers.lines().map(|l| l.trim().to_string()).collect();
        if hops.is_empty() {
            hops.push(String::new());
        }
        // The advanced options' summary: how many issuer ledgers the check stands on.
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
                    // Each hop's folder is a row; "add next hop" chooses the next one's folder at once.
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
                            st.field(ui, t(Key::CheckRegistryFor), Some(opt), None, |ui, w| fields::line(ui, &mut self.typed.ck_registry, "0x\u{2026}", w, mono));
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
                if key::show(ui, key::Key::new(t(Key::NavVerifyView), Role::Primary).enabled(ready).why(t(Key::U3CheckNotYet)).phase(phase)).clicked() {
                    *go = true;
                }
            });
        });
        self.stage_line(ui, crate::task::Kind::Check);
    }

    /// The file or the terms file on the form, a row: its name with "optional" after it; "none selected"
    /// and "choose file" until chosen, then the chosen file's name and size with "change" and "remove". While a
    /// file is dragged over the window the row turns into a one-line drop zone that takes it.
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
            let size = self.size_of_path(&path).map(size_say).unwrap_or_default();
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

    /// Start a check from whichever page asks (this page, or the home page's quick box): a refusal on the spot is
    /// this press's own result and replaces the last one; a started task records its landing by kind.
    pub(super) fn start_check(&mut self, now: f64) {
        let a = self.check_action();
        self.ux.u3.check_refused = match self.act(a, now) {
            Applied::Trouble(f) => Some(f),
            _ => None,
        };
    }

    /// The result: nothing before the first check, shimmering while one runs, the failure and its raw error on a
    /// failure; else the overview (the verdict, the record, the grantor, the validity and the first anchor; the
    /// grant, the file and the terms when they say more; what is missing), the six checks, each hop's ledger
    /// source a row, and the raw values in the details list at the bottom. Returns whether a check was asked
    /// again (fetch again, or an issuer ledger chosen or entered).
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
        // Only this kind's own failures (another task's failure is not borrowed onto this page).
        if let Some(f) = self.ux.u3.check_refused.clone().or_else(|| self.shell.failed.get(&crate::task::Kind::Check).cloned()) {
            card::form(ui, |ui, form| form.summary(ui, Glyph::No, card::Tint::Bad, t(Key::U3CheckFailed), None, t(Key::U3CheckFailedNext), |_| ()));
            details_card(ui, "check-err", &[(t(Key::U3RawError), Val::mono(f.raw()))]);
            return false;
        }
        let Some(x) = self.shell.checked.clone() else {
            return false;
        };
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
        };
        // A side's line under its name: the file compared, or why it could not be read.
        let side_line = |side: &crate::checkx::Side| match side {
            crate::checkx::Side::Refused(f) => f.human().to_string(),
            _ => side_face(side).0.unwrap_or_default(),
        };
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
        let last = x.judged.hops.last();
        let record = last.map(|h| self.record_name_of(&h.work)).unwrap_or_default();
        let grantor = last.map(|h| card::short(&h.author)).unwrap_or_default();
        let window = last.map(|h| self.window_days(h.window)).unwrap_or_default();
        let first = last.map(|h| first_anchor_say(h.anchored_at)).unwrap_or_default();
        // The overview: the verdict, the record under it, three figures, alone in its card.
        stagger(ui, 3, |ui| {
            card::form(ui, |ui, f| {
                f.summary(ui, glyph, tint, &verdict, None, &record, |_| ());
                if last.is_some() {
                    f.facts(ui, &[(t(Key::OsGrantor), card::Fact::Mono(&grantor)), (t(Key::U3Window), card::Fact::Text(&window)), (t(Key::U4RecordFirstAt), card::Fact::Mono(&first))]);
                }
            });
        });
        // The six checks in their own card under their group title: first the grant, the file and the terms only
        // when they say more than the verdict, and what is missing; then each check a row with its mark (what is
        // missing beside a grey one); several hops: each hop's six under its line, then one row per link.
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
                            go_settings |= f.act(ui, t(Key::NoEndpointYet), "", key::Key::new(t(Key::CheckGoSettings), Role::Secondary)).clicked();
                        } else {
                            f.note(ui, t(gap_note(g)));
                        }
                    }
                    // Read-only networks this pass could not read, each with why.
                    for m in &x.missed {
                        let _ = f.info(ui, &m.name, card::Show::Mark(Mark::Warn, t(m.reading.key())));
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
        // Where each hop's ledger came from, one row each.
        stagger(ui, 5, |ui| {
            card::section(ui, t(Key::LedgerSources), "", |ui| {
                card::form(ui, |ui, f| {
                    for i in 0..x.judged.hops.len() {
                        self.check_source(ui, f, &x, i, &mut go);
                    }
                });
            });
        });
        // Details: the raw values, every label a name.
        let several = x.judged.hops.len() > 1;
        let mut rows: Vec<(String, Val)> = Vec::new();
        if let Some(h) = last {
            rows.push((t(Key::OsGrantor).to_string(), Val::mono(h.author.clone())));
            rows.push((t(Key::U3ToWhom).to_string(), Val::mono(h.grantee.clone())));
            rows.push((t(Key::OsWork).to_string(), Val::mono(h.work.clone())));
        }
        rows.push((t(Key::U3Verdict).to_string(), Val::figure(x.judged.verdict.clone())));
        // The evidence behind the summary: each refused file's reason, for each side.
        if !x.refused.is_empty() {
            rows.push((t(Key::RefusedFiles).to_string(), Val::mono(x.refused.iter().map(|r| format!("{} \u{b7} {}", width::file_name(&r.file), r.why)).collect::<Vec<_>>().join("\n"))));
        }
        for (side, name) in [(&x.file, Key::U3File), (&x.terms, Key::OsTerms)] {
            if let crate::checkx::Side::Refused(f) = side {
                rows.push((t(name).to_string(), Val::mono(f.raw())));
            }
        }
        rows.push((t(Key::EquivalentVerb).to_string(), Val::mono(x.verb().to_string())));
        for (i, h) in x.judged.hops.iter().enumerate() {
            for (tok, state) in &h.lights {
                let name = t(u3_check_name(tok));
                let label = if several { format!("{} \u{b7} {name}", fill1(Key::CheckHop, &(i + 1).to_string())) } else { name.to_string() };
                rows.push((label, Val::figure(state.clone())));
            }
        }
        if let Some(ch) = x.judged.chain.as_ref() {
            rows.push((t(Key::U3Hops).to_string(), Val::figure(format!("{} {} {}", ch.verdict, ch.token, ch.failing_kind))));
        }
        for (which, side) in [(OneStop::File, &x.file), (OneStop::Terms, &x.terms)] {
            if let crate::checkx::Side::Compared(d) = side {
                rows.push((t(which.title()).to_string(), Val::mono(format!("{} \u{b7} {} \u{b7} {}", d.bytes, d.got_hex(), d.want_hex()))));
            }
        }
        let refs: Vec<(&str, Val)> = rows.iter().map(|(k, v)| (k.as_str(), v.clone())).collect();
        stagger(ui, 6, |ui| details_card(ui, "check-evidence", &refs));
        if go_settings {
            self.go(Place::Settings(Section::Network), now);
        }
        go
    }

    /// A hop's ledger source, a row: its mark and where it came from ("this Mac", or "none"), with "replace" or
    /// "add" (none once supplied here and all six green); from a publish address a row with the address, the
    /// file count and "fetch again"; each source that failed, a row with why. "Replace/add" opens the issuer
    /// ledger field for that hop (line i of the advanced hop list): choosing a folder there, or entering a path,
    /// checks again.
    fn check_source(&mut self, ui: &mut egui::Ui, f: &mut card::Form, x: &crate::checkx::Checked, hop: usize, go: &mut bool) {
        let src = x.found.get(hop).cloned().unwrap_or_default();
        let (said, open_key, m) = match &src.from {
            Some((level, _)) => (t(level_key(*level)).to_string(), Key::U3TermsSwap, Mark::Ok),
            None => (t(Key::Nothing).to_string(), Key::ReadNetAdd, Mark::Warn),
        };
        let title = fill1(Key::CheckSourceLine, &said);
        // Supplied here and all six green: nothing to fill, so no "replace".
        let settled = crate::checkx::source_settled(x, hop);
        let mut open = !settled && self.ux.u3.ck_source_open == Some(hop);
        let remote = match (&src.from, src.files) {
            (Some((crate::supplyx::Level::Remote, url)), Some(n)) => Some(fill2(Key::CheckFromAddress, url, &n.to_string())),
            _ => None,
        };
        // The keys of this card are as wide as the widest of them.
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
                        let (resp, pick_hit) = fields::joined(ui, &mut lines[hop], t(Key::CheckIssuerLedgerHint), w, mono, Some(|ui: &mut egui::Ui| fields::inner_key(ui, t(Key::PickFolder), None).clicked()));
                        changed |= resp.changed();
                        // A path entered by hand checks on Enter; a folder chosen checks at once.
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
