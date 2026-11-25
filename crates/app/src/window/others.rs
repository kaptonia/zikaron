use super::*;
use super::works::anchor_rows;

pub(super) struct OtherView {
    who: String,
    entries: usize,
    timeline: Vec<crate::ledgerx::Row>,
    grants: Vec<crate::grantx::Row>,
    dil: Option<crate::diligx::Read>,
    state: crate::sendx::behind::State<crate::checkx::Audit, crate::fault::Fault>,
    seen: Option<crate::readerx::Audited>,
}

impl OtherView {
    fn audit(&self) -> Option<&crate::checkx::Audit> {
        match &self.state {
            crate::sendx::behind::State::Done(a) => Some(a),
            _ => None,
        }
    }
}

impl Win {
    pub(super) fn other_held(&self) -> std::sync::Arc<Option<OtherView>> {
        let grantee = self.shell.settings.role == crate::roles::Role::Grantee;
        self.ux.other_held.get_or((self.shell.moved, grantee), || std::sync::Arc::new(self.other_view()))
    }

    fn other_view(&self) -> Option<OtherView> {
        let (mut v, key) = if self.shell.settings.role == crate::roles::Role::Grantee {
            let x = self.shell.diligence.clone()?;
            let key = x.audit_key;
            (OtherView { who: x.who.clone(), entries: x.entries, timeline: x.timeline.clone(), grants: x.grants.clone(), dil: Some(x), state: crate::sendx::behind::State::Unaudited, seen: None }, key)
        } else {
            match self.shell.book.clone() {
                Some(Done::Book { who, entries, timeline, grants, audit_key, .. }) => (OtherView { who, entries, timeline, grants, dil: None, state: crate::sendx::behind::State::Unaudited, seen: None }, audit_key),
                _ => return None,
            }
        };
        v.state = self.shell.audits.state(&key);
        v.seen = v.audit().and_then(|a| crate::readerx::Audited::of_value(&a.value));
        Some(v)
    }

    pub(super) fn others_page(&mut self, ui: &mut egui::Ui, now: f64) {
        let grantee = self.shell.settings.role == crate::roles::Role::Grantee;
        let kind = if grantee { crate::task::Kind::Diligence } else { crate::task::Kind::Book };
        let (mut read, mut forget, mut remember, mut snapshot) = (false, false, false, false);
        let book = self.shell.settings.book.clone();
        let phase = self.phase_of(kind);
        let opt = t(Key::U3Optional);
        let mono = fields::Look { mono: true, ..Default::default() };
        stagger(ui, 1, |ui| {
            let dir_now = if grantee { self.typed.dg_dir.clone() } else { self.typed.rd_dir.clone() };
            let summary = if dir_now.trim().is_empty() { t(Key::Nothing).to_string() } else { width::file_name(&dir_now) };
            let bad = crate::fetchx::is_address(&dir_now).then(|| crate::fetchx::base_of(&dir_now).err()).flatten();
            fields::card(ui, |ui, st| {
                let shape_bad = crate::addressedx::not_address(if grantee { &self.typed.dg_address } else { &self.typed.rd_address }, crate::addressedx::Takes::Address);
                st.field(ui, t(Key::U3AuthorAddress), None, shape_bad.then(|| (t(Key::KitToWhyAddress), true)), |ui, w| {
                    let address = if grantee { &mut self.typed.dg_address } else { &mut self.typed.rd_address };
                    let heads: Vec<String> = book.iter().map(|a| head_tail(a)).collect();
                    let items: Vec<menu::Item> = heads.iter().map(|h| menu::Item::Row(menu::Row { lead: t(Key::Address), label: h, mono: true, ..Default::default() })).collect();
                    let spec = pick::Spec { hint: t(Key::SearchAddress), empty: t(Key::SearchNone), w: PICK_W, dates: None };
                    let keep = |i: usize, q: &str, _: &str, _: &str| matches(q, &[&book[i]]);
                    let inner = |ui: &mut egui::Ui| zikaron_ui::formparts::inner_pick(ui, "others-book", t(Key::U3SeenBeforeDots), false, false, !book.is_empty(), &spec, &items, &keep);
                    let (_, picked) = fields::joined(ui, address, "0x\u{2026}", w, mono, Some(inner));
                    if let Some(i) = picked.flatten() {
                        *address = book[i].clone();
                    }
                });
                zikaron_ui::fields::rows(ui, |ui, f| {
                    let _ = f.fold(ui, "others-more", t(Key::U3MoreOptions), &summary, |ui, f| {
                        f.panel(ui, |ui| {
                            fields::stack(ui, |ui, st| {
                                let said = bad.as_ref().map(|b| (b.human(), true));
                                st.field(ui, t(Key::RecordContent), Some(opt), said, |ui, w| {
                                    let dir = if grantee { &mut self.typed.dg_dir } else { &mut self.typed.rd_dir };
                                    let look = fields::Look { mono: true, bad: bad.is_some(), ..Default::default() };
                                    let (_, pick) = fields::joined(ui, dir, t(Key::BytesWhereHint), w, look, Some(|ui: &mut egui::Ui| fields::inner_key(ui, pick_label(crate::platform::Pick::FileOrFolder), None).clicked()));
                                    if let Some(p) = path_answer(ui.ctx(), egui::Id::new(("zikaron-path-bytes-where", grantee)), pick.unwrap_or(false), crate::platform::Pick::FileOrFolder) {
                                        *dir = p;
                                    }
                                });
                                if grantee {
                                    st.field(ui, t(Key::U4RecordHash), Some(opt), None, |ui, w| fields::line(ui, &mut self.typed.dg_work, "0x\u{2026}", w, mono));
                                    st.field(ui, t(Key::WindowWanted), Some(opt), None, |ui, w| {
                                        let half = ((w - 14.0) / 2.0).max(40.0);
                                        let (row, _) = ui.allocate_exact_size(egui::vec2(w, tk::INPUT_H), egui::Sense::hover());
                                        let left = egui::Rect::from_min_size(row.min, egui::vec2(half, tk::INPUT_H));
                                        let right = egui::Rect::from_min_size(egui::pos2(row.right() - half, row.top()), egui::vec2(half, tk::INPUT_H));
                                        ui.scope_builder(egui::UiBuilder::new().max_rect(left), |ui| fields::line(ui, &mut self.typed.dg_from, t(Key::U4FromSecs), half, mono));
                                        ui.scope_builder(egui::UiBuilder::new().max_rect(right), |ui| fields::line(ui, &mut self.typed.dg_to, t(Key::U4ToSecs), half, mono));
                                    });
                                }
                            });
                        });
                        if grantee {
                            let held = !matches!(crate::action::snapshot_of(&self.shell), crate::action::Snapshot::NotHeld);
                            snapshot = f.go(ui, t(Key::DoSnapshot), "", held, t(Key::SnapshotWholeGoneNext)).clicked();
                        }
                    });
                });
                let address = if grantee { &self.typed.dg_address } else { &self.typed.rd_address };
                let filled = !address.trim().is_empty();
                let known = book.iter().any(|a| a.eq_ignore_ascii_case(address.trim()));
                st.foot(ui, |ui| {
                    let _page = page::Page::new();
                    let shape_bad = crate::addressedx::not_address(address, crate::addressedx::Takes::Address);
                    let why = if shape_bad { t(Key::KitToWhyAddress) } else { t(Key::U3OthersNotRead) };
                    read = key::show(ui, key::Key::new(t(Key::U3ReadIt), Role::Primary).enabled(filled && !shape_bad).why(why).phase(phase)).clicked();
                    if known {
                        forget = key::key(ui, t(Key::U3ForgetAddr), Role::Secondary, true).clicked();
                    } else {
                        remember = key::show(ui, key::Key::new(t(Key::U3RememberAddr), Role::Secondary).enabled(filled).why(t(Key::U3OthersNotRead))).clicked();
                    }
                });
            });
            self.stage_line(ui, kind);
        });
        let address = if grantee { self.typed.dg_address.clone() } else { self.typed.rd_address.clone() };
        if remember {
            self.act(Action::RememberAddress { address: address.clone() }, now);
        }
        if forget {
            self.act(Action::ForgetAddress { address: address.clone() }, now);
        }
        if snapshot {
            self.snapshot_open();
        }
        if grantee {
            self.snapshot_sheet(ui.ctx(), now);
        }
        if read {
            let a = if grantee {
                Action::Diligence { address: self.typed.dg_address.clone(), dir: self.typed.dg_dir.clone(), work: self.typed.dg_work.clone(), from: self.typed.dg_from.clone(), to: self.typed.dg_to.clone() }
            } else {
                Action::ReadBook { address: self.typed.rd_address.clone(), dir: self.typed.rd_dir.clone() }
            };
            self.ux.u3.others_for = Some(format!("{a:?}"));
            self.act(a, now);
        }
        if self.shell.tasks.in_flight(kind) {
            card::card(ui, |ui| states::skeleton_lines(ui, &[0.3, 0.6, 0.5, 0.7]));
            return;
        }
        let input = if grantee {
            Action::Diligence { address: self.typed.dg_address.clone(), dir: self.typed.dg_dir.clone(), work: self.typed.dg_work.clone(), from: self.typed.dg_from.clone(), to: self.typed.dg_to.clone() }
        } else {
            Action::ReadBook { address: self.typed.rd_address.clone(), dir: self.typed.rd_dir.clone() }
        };
        if self.ux.u3.others_for.as_deref().is_some_and(|a| a != format!("{input:?}")) {
            return;
        }
        let held = self.other_held();
        let Some(v) = held.as_ref().as_ref() else {
            return;
        };
        stagger(ui, 2, |ui| {
            ui.horizontal(|ui| {
                if let Some(i) = seg::seg(ui, "others-tabs", &[seg::Cell::from(t(Key::U3Overview)), seg::Cell::from(t(Key::U3EntryByEntry))], self.ux.u3.others_tab) {
                    self.ux.u3.others_tab = i;
                }
            });
        });
        ui.add_space(tk::S4 - ui.spacing().item_spacing.y);
        let key = self.ux.u3.others_tab as u64;
        tab_body(ui, "others", key, |ui| {
            if self.ux.u3.others_tab == 0 {
                if self.others_overview(ui, &v) {
                    self.act(Action::AuditOthers, now);
                }
            } else if let Some(seq) = self.others_list(ui, &v) {
                self.push(Route::Other(seq), now);
            }
        });
    }

    pub(super) fn snapshot_open(&mut self) {
        if self.typed.dg_snapshot.trim().is_empty() {
            self.typed.dg_snapshot = match self.shell.home.as_ref() {
                Some(h) => h.dir(crate::home::Slot::Kits).display().to_string(),
                None => self.shell.documents.clone(),
            };
        }
        self.ux.snapshot_sheet = true;
    }

    fn snapshot_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.snapshot_sheet {
            return;
        }
        let at = self.typed.dg_snapshot.clone();
        let ready = Self::landing_ok(&at);
        let (mut go, mut close, mut picked) = (false, false, None);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("others-snapshot", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, t(Key::DoSnapshot), "");
                card::form(ui, |ui, f| f.folder(ui, t(Key::SnapshotOut), &at, t(Key::PickNone)));
            },
            |ui, _me| {
                go = key::key(ui, t(Key::DoSnapshot), Role::Primary, ready).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                picked = foot_pick(ui, "others-snapshot", crate::platform::Pick::Folder);
            },
        );
        if let Some(p) = picked {
            self.typed.dg_snapshot = p;
        }
        if close || out.esc {
            self.ux.snapshot_sheet = false;
            return;
        }
        if go && ready {
            let who: String = self.typed.dg_address.trim().trim_start_matches("0x").chars().take(10).collect();
            let chosen = crate::home::choose(&crate::home::Kind::File { stem: format!("snapshot-{who}"), ext: "json".into() }, std::path::Path::new(self.typed.dg_snapshot.trim()));
            self.remember_landing(Out::Snapshot, &chosen);
            let a = Action::SaveSnapshot { to: chosen.at.display().to_string() };
            self.act(a, now);
            self.ux.snapshot_sheet = false;
        }
    }

    fn others_overview(&mut self, ui: &mut egui::Ui, v: &OtherView) -> bool {
        let chain_now = self.shell.chain_now();
        let mut audit_go = false;
        let grantee = v.dil.is_some();
        let supply = match (grantee, self.shell.book.as_ref(), v.dil.as_ref()) {
            (false, Some(Done::Book { from, files, misses, .. }), _) => Some((from.clone(), *files, misses.clone())),
            (true, _, Some(x)) => Some((x.from.clone(), x.files, x.misses.clone())),
            _ => None,
        };
        let unobtained = matches!(supply, Some((None, _, _)));
        let missed: Vec<crate::widex::Missed> = v.seen.as_ref().map(|s| s.missed.clone()).unwrap_or_default();
        let deleted = crate::retractx::read(&v.timeline);
        let works: Vec<&crate::ledgerx::Row> = v.timeline.iter().filter(|r| r.kind == zikaron_glue::book::Kind::History && !deleted.is_deleted(&r.id)).collect();
        let last = v.seen.as_ref().and_then(|s| v.timeline.iter().filter(|r| s.row(&r.id).is_some_and(|(l, _)| l == crate::ledgerx::Lamp::Anchored)).map(|r| r.seq).max());
        let (audit, mark) = match (v.audit(), v.entries) {
            (Some(_), 0) => (t(Key::OnlyAnchors).to_string(), Mark::Todo),
            (Some(a), _) => (label_human(&a.label), if a.label == zikaron::v2::tokens::Label::Complete.as_str() { Mark::Ok } else { Mark::Warn }),
            (None, 0) => (t(Key::NotObtained).to_string(), Mark::Todo),
            (None, _) => (t(Key::AuditNone).to_string(), Mark::Todo),
        };
        let (glyph, tint) = match mark {
            Mark::Ok => (Glyph::Ok, card::Tint::Ok),
            Mark::Todo => (Glyph::Info, card::Tint::Blue),
            _ => (Glyph::Warn, card::Tint::Warn),
        };
        let count = |n: usize| if unobtained { t(Key::NotObtained).to_string() } else { n.to_string() };
        let when = match (v.seen.as_ref().map(|s| s.latest.map(crate::when::day)), last) {
            (None, _) => t(Key::AuditNone).to_string(),
            (Some(Some(d)), Some(s)) => format!("{d} \u{b7} #{s}"),
            (Some(Some(d)), None) => d,
            (Some(None), Some(s)) => format!("#{s}"),
            (Some(None), None) => t(Key::None_).to_string(),
        };
        let works_n = count(works.len());
        let grants_n = count(v.grants.len());
        let sub = match v.audit() {
            Some(a) => format!("{} \u{b7} {}", card::short(&v.who), fill1(Key::U3EntriesCount, &a.entries.to_string())),
            None => card::short(&v.who),
        };
        let source = match &supply {
            Some((Some((level, place)), files, _)) => Some(match (level, files) {
                (crate::supplyx::Level::Remote, Some(n)) => (Mark::Ok, format!("{} \u{b7} {}", t(level_key(*level)), fill2(Key::CheckFromAddress, &width::file_name(place), &n.to_string()))),
                _ => (Mark::Ok, t(level_key(*level)).to_string()),
            }),
            Some((None, _, _)) => Some((Mark::Warn, t(Key::NotObtained).to_string())),
            None => None,
        };
        stagger(ui, 3, |ui| {
            card::form(ui, |ui, f| {
                f.summary(ui, glyph, tint, &audit, None, &sub, |_| ());
                f.facts(ui, &[(t(Key::U3Works), fact_of(&works_n, unobtained)), (t(Key::ReaderGrants), fact_of(&grants_n, unobtained)), (t(Key::U3LastAnchored), card::Fact::Mono(&when))]);
                if let Some((_, _, misses)) = &supply {
                    if unobtained || !misses.is_empty() {
                        if let Some((m, whence)) = &source {
                            let _ = f.row_with(ui, Some(*m), &fill1(Key::CheckSourceLine, whence), "", card::Value::None, |_| ());
                        }
                        for (level, miss) in misses {
                            let _ = f.row_with(ui, Some(Mark::Warn), &fill2(Key::CheckMissRow, t(level_key(*level)), miss.human()), "", card::Value::None, |_| ());
                        }
                    }
                }
                if unobtained {
                    f.note(ui, t(Key::ReaderNoContent));
                }
                for m in &missed {
                    let _ = f.info(ui, &m.name, card::Show::Mark(Mark::Warn, t(m.key())));
                }
            });
        });
        if !unobtained {
            stagger(ui, 3, |ui| {
                card::section(ui, t(Key::AuditSection), "", |ui| {
                    card::form(ui, |ui, f| {
                        if super::check::audit_rows(ui, f, &v.state) {
                            audit_go = true;
                        }
                    });
                });
            });
        }
        if let Some(x) = v.seen.as_ref().and_then(|s| s.diligence.as_ref()) {
            let (m, said) = if x.work.is_empty() {
                (Mark::Todo, t(Key::DoubleSaleNoWork).to_string())
            } else if !x.window {
                (Mark::Todo, t(Key::DoubleSaleNoWindow).to_string())
            } else if x.clash > 0 {
                (Mark::Bad, fill1(Key::DoubleSaleRed, &x.clash.to_string()))
            } else if x.undecided > 0 {
                (Mark::Warn, fill1(Key::DoubleSaleUndecided, &x.undecided.to_string()))
            } else {
                (Mark::Ok, t(Key::DoubleSaleGreen).to_string())
            };
            stagger(ui, 4, |ui| {
                card::section(ui, t(Key::DoubleSale), "", |ui| {
                    card::form(ui, |ui, f| {
                        let _ = f.info(ui, &said, card::Show::Mark(m, ""));
                    });
                });
            });
        }
        if let (false, Some(Done::Book { face, moves, who, .. })) = (grantee, self.shell.book.as_ref()) {
            let key = format!("{who}:{}", moves.len());
            if self.ux.other_hop.0 != key {
                self.ux.other_hop = (key, 0);
            }
            let b = super::persona::other_block(&self.shell, face, moves, self.ux.other_hop.1);
            if let Some(super::persona::Does::Next) = stagger(ui, 4, |ui| super::persona::draw_block(ui, &b)) {
                self.ux.other_hop.1 = (self.ux.other_hop.1 + 1).min(moves.len());
            }
        }
        stagger(ui, 5, |ui| {
            card::section(ui, t(Key::ReaderGrants), "", |ui| {
                card::form(ui, |ui, f| {
                    if v.grants.is_empty() {
                        let _ = f.info(ui, t(Key::U3NoGrants), card::Show::Empty(""));
                        return;
                    }
                    let name_of = |work: &str| {
                        v.timeline
                            .iter()
                            .find(|r| r.kind == zikaron_glue::book::Kind::History && r.work.as_deref().map(|w| w.eq_ignore_ascii_case(work)).unwrap_or(false))
                            .map(|x| human_summary(x, chain_now))
                            .unwrap_or_else(|| t(Key::UnnamedRecord).to_string())
                    };
                    for g in &v.grants {
                        let at = v.seen.as_ref().and_then(|s| s.row(&g.id)).and_then(|(_, t)| t);
                        let undecided = v.seen.as_ref().and_then(|s| s.grant(&g.id)).and_then(|(_, u)| u);
                        let _ = f.line(ui, None, &name_of(&g.work), "", false, false, c(C::Ink), |ui| {
                            ui.label(egui::RichText::new(first_anchor_say(at)).font(Type::MonoSmall.font()).color(c(C::Ink3)));
                            ui.label(egui::RichText::new(window_short(g.window, chain_now)).font(Type::Note.font()).color(c(C::Ink2)));
                            if let Some(u) = &undecided {
                                ui.label(egui::RichText::new(fill1(Key::WithdrawalUndecided, &u.words())).font(Type::Note.font()).color(c(C::WarnInk)));
                            }
                        });
                    }
                });
            });
        });
        let head = v.timeline.iter().max_by_key(|r| r.seq).map(|r| r.id.clone());
        let mut rows: Vec<(&str, Val)> = vec![(t(Key::EntryAuthor), Val::mono(v.who.clone()))];
        if let Some(h) = head {
            rows.push((t(Key::AuHead), Val::mono(h)));
        }
        if let Some((_, said)) = &source {
            rows.push((t(Key::LedgerSources), Val::text(said.clone())));
        }
        if let Some(a) = v.audit() {
            rows.push((t(Key::AuditLabel), Val::figure(a.label.clone())));
        }
        if let Some(x) = v.seen.as_ref().and_then(|s| s.diligence.as_ref()) {
            rows.push((t(Key::Lineage), if x.lineage.is_empty() { Val::text(t(Key::None_)) } else { Val::mono(x.lineage.join(" \u{2192} ")) }));
            if !x.successions.is_empty() {
                rows.push((t(Key::TagHandover), Val::mono(x.successions.iter().map(|(seq, kind, to)| format!("#{seq} \u{b7} {kind} \u{b7} {to}")).collect::<Vec<_>>().join("\n"))));
            }
        }
        stagger(ui, 6, |ui| details_card(ui, "others-lineage", &rows));
        audit_go
    }

    fn others_list(&mut self, ui: &mut egui::Ui, v: &OtherView) -> Option<u64> {
        let chain_now = self.shell.chain_now();
        let (listed, generation) = kept(ui.ctx(), self.shell.moved, "others-entries", v.timeline.len(), || {
            let mut timeline = v.timeline.clone();
            timeline.sort_by(|a, b| b.seq.cmp(&a.seq));
            let reading = crate::retractx::read(&timeline);
            timeline
                .iter()
                .map(|row| {
                    let (tag, summary, struck) = row_face(&timeline, &reading, row, chain_now);
                    let r = table::Row {
                        cells: match v.seen.as_ref().and_then(|s| s.row(&row.id)) {
                            Some((lamp, at)) => vec![table::Cell::Seq(row.seq), table::Cell::Tag(tag.to_string()), table::Cell::Text(summary), table::Cell::Mono(first_anchor_say(at)), table::Cell::Mark(lamp_mark(lamp))],
                            None => vec![table::Cell::Seq(row.seq), table::Cell::Tag(tag.to_string()), table::Cell::Text(summary), table::Cell::Text(String::new()), table::Cell::Text(String::new())],
                        },
                        click: true,
                        gone: struck,
                        on: false,
                    };
                    (r, row.seq.to_string())
                })
                .collect::<Listed>()
        });
        let cols = [table::SEQ, table::TYPE, table::col("", table::Col::Fr(1.0)), table::col("", table::Col::Px(200.0)), table::MARK];
        let hit = stagger(ui, 3, |ui| listed_table(ui, "others-entries", table::Spec::new(&cols, generation).empty(t(Key::OnlyAnchors)), &listed));
        hit.and_then(|s| s.parse().ok())
    }

    pub(super) fn other_entry(&mut self, ui: &mut egui::Ui, seq: u64, _now: f64) {
        let held = self.other_held();
        let Some(v) = held.as_ref().as_ref() else {
            states::empty(ui, Glyph::Others, t(Key::U3OthersNotRead));
            return;
        };
        let face = self.other_face(seq);
        let Some((row, tag, summary, struck)) = face.as_ref().clone() else {
            states::empty(ui, Glyph::Others, t(Key::DetailGone));
            return;
        };
        let anchored = v.seen.as_ref().and_then(|s| s.row(&row.id));
        let pills: Vec<zikaron_ui::card::HeroPill> = anchored.map(|(l, at)| lamp_hero(l, at)).into_iter().collect();
        stagger(ui, 0, |ui| {
            card::hero(ui, &summary, &format!("#{} \u{b7} {}", row.seq, tag), struck, &pills);
        });
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::BasicInfo), "", |ui| {
                card::form(ui, |ui, f| {
                    if let Some((l, at)) = anchored {
                        anchor_rows(ui, f, l, at, at, None);
                    }
                    let _ = f.info(ui, t(Key::V2RecordNo), card::Show::Mono(&format!("#{}", row.seq)));
                });
            });
        });
        stagger(ui, 2, |ui| {
            details_card(
                ui,
                "other-entry-details",
                &[(t(Key::EntryAuthor), Val::mono(v.who.clone())), (t(Key::DetailPick), Val::mono(row.id.clone())), (t(Key::U4RecordFirstAt), Val::figure(anchored.map(|(_, at)| first_anchor_say(at)).unwrap_or_else(|| t(Key::AuditNone).to_string())))],
            )
        });
    }

    pub(super) fn other_title(&self, seq: u64) -> String {
        match self.other_face(seq).as_ref() {
            Some((r, tag, _, _)) => format!("#{} \u{b7} {tag}", r.seq),
            None => format!("#{seq}"),
        }
    }

    fn other_face(&self, seq: u64) -> std::sync::Arc<Option<(crate::ledgerx::Row, &'static str, String, bool)>> {
        let chain_now = self.shell.chain_now();
        let grantee = self.shell.settings.role == crate::roles::Role::Grantee;
        self.ux.other_face.get_or((self.shell.moved, grantee, seq), || {
            let held = self.other_held();
            std::sync::Arc::new(held.as_ref().as_ref().and_then(|v| {
                let reading = crate::retractx::read(&v.timeline);
                v.timeline.iter().find(|r| r.seq == seq).map(|r| {
                    let (tag, summary, struck) = row_face(&v.timeline, &reading, r, chain_now);
                    (r.clone(), tag, summary, struck)
                })
            }))
        })
    }
}

pub(super) fn fact_of(s: &str, missing: bool) -> card::Fact<'_> {
    if missing { card::Fact::Quiet(s) } else { card::Fact::Text(s) }
}

pub(super) fn level_key(l: crate::supplyx::Level) -> Key {
    match l {
        crate::supplyx::Level::Local => Key::SourceLocal,
        crate::supplyx::Level::Vault => Key::SourceVault,
        crate::supplyx::Level::Kit => Key::PageKit,
        crate::supplyx::Level::Remote => Key::SetPublish,
    }
}

pub(super) fn gap_words(g: &crate::checkx::Gap) -> String {
    use crate::checkx::Gap;
    match g {
        Gap::NoLedger => t(Key::GapNoLedger).to_string(),
        Gap::LedgerRefused(_) => t(Key::GapLedgerRefused).to_string(),
        Gap::LedgerBehind => t(Key::GapLedgerBehind).to_string(),
        Gap::NoNode => format!("{} \u{b7} {}", t(Key::NoEndpointYet), t(Key::CheckGoSettings)),
        Gap::ChainUnread(_) => t(Key::GapChainUnread).to_string(),
        Gap::NotYetAnchored => t(Key::GapNotYetAnchored).to_string(),
        Gap::NoTime => t(Key::GapNoTime).to_string(),
        Gap::NoticesUnread(w) => w.clone(),
        Gap::Undecided(_) => String::new(),
    }
}

pub(super) fn gap_note(g: &crate::checkx::Gap) -> Key {
    use crate::checkx::Gap;
    match g {
        Gap::NoLedger => Key::NoteNoLedger,
        Gap::LedgerRefused(_) => Key::NoteLedgerRefused,
        Gap::LedgerBehind => Key::NoteLedgerBehind,
        Gap::NoNode => Key::NoteNoNode,
        Gap::ChainUnread(_) => Key::NoteChainUnread,
        Gap::NotYetAnchored => Key::NoteNotYetAnchored,
        Gap::NoTime => Key::NoteNoTime,
        Gap::NoticesUnread(_) => Key::NoticesUnread,
        Gap::Undecided(zikaron_glue::book::Undecided::OldLaw) => Key::NoteOldLaw,
        Gap::Undecided(zikaron_glue::book::Undecided::Continued) => Key::NoteContinued,
    }
}

pub(super) fn exclusive_key(x: crate::termsx::Exclusive) -> Key {
    match x {
        crate::termsx::Exclusive::Signed => Key::ExclusiveSigned,
        crate::termsx::Exclusive::No => Key::ExclusiveNo,
    }
}

pub(super) fn u3_check_name(token: &str) -> Key {
    use zikaron_kit::tokens::Check as K;
    if token == K::BadSig.as_str() {
        Key::U3CkSig
    } else if token == K::BrokenLedger.as_str() {
        Key::U3CkLedger
    } else if token == K::NotInLedger.as_str() {
        Key::U3CkInLedger
    } else if token == K::Unanchored.as_str() {
        Key::U3CkAnchored
    } else if token == K::Expired.as_str() {
        Key::U3CkWindow
    } else if token == K::Revoked.as_str() {
        Key::U3CkNotRevoked
    } else {
        Key::KindOther
    }
}
