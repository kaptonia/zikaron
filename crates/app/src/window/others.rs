//! Others' records, for both seats (the grantee's reading adds the grant conflict check and key lineage):
//! read by author address, shown as an overview and an entry list, each entry opening a read-only detail page.

use super::*;
use super::works::anchor_rows;

/// One reading of another ledger, the same shape for both seats.
struct OtherView {
    who: String,
    anchors: usize,
    entries: usize,
    label: String,
    timeline: Vec<crate::ledgerx::Row>,
    grants: Vec<crate::grantx::Row>,
    latest: Option<u64>,
    dil: Option<crate::diligx::Read>,
}

impl Win {
    /// The reading on hand for this seat: the book (author) or the diligence report (grantee).
    fn other_view(&self) -> Option<OtherView> {
        if self.shell.settings.role == crate::roles::Role::Grantee {
            self.shell.diligence.clone().map(|x| OtherView {
                who: x.who.clone(),
                anchors: x.anchors,
                entries: x.entries,
                label: x.label.clone(),
                timeline: x.timeline.clone(),
                grants: x.grants.clone(),
                latest: x.latest,
                dil: Some(x),
            })
        } else {
            match self.shell.book.clone() {
                Some(Done::Book { who, anchors, entries, label, timeline, grants, latest, .. }) => Some(OtherView { who, anchors, entries, label, timeline, grants, latest, dil: None }),
                _ => None,
            }
        }
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
            // The advanced options' summary: where the record content comes from, else none.
            let dir_now = if grantee { self.typed.dg_dir.clone() } else { self.typed.rd_dir.clone() };
            let summary = if dir_now.trim().is_empty() { t(Key::Nothing).to_string() } else { width::file_name(&dir_now) };
            let bad = crate::fetchx::is_address(&dir_now).then(|| crate::fetchx::base_of(&dir_now).err()).flatten();
            fields::card(ui, |ui, st| {
                // The author's address, typed or taken from the address book inside its field.
                st.field(ui, t(Key::U3AuthorAddress), None, None, |ui, w| {
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
                                // Record content: a local file or folder (kit, grant file, ledger folder) or an https
                                // publish address; its shape is checked under it.
                                let said = bad.as_ref().map(|b| (b.human(), true));
                                st.field(ui, t(Key::RecordContent), Some(opt), said, |ui, w| {
                                    let dir = if grantee { &mut self.typed.dg_dir } else { &mut self.typed.rd_dir };
                                    let look = fields::Look { mono: true, bad: bad.is_some(), ..Default::default() };
                                    let (_, pick) = fields::joined(ui, dir, t(Key::BytesWhereHint), w, look, Some(|ui: &mut egui::Ui| fields::inner_key(ui, t(Key::PickFolder), None).clicked()));
                                    if let Some(p) = path_answer(ui.ctx(), egui::Id::new(("zikaron-path-bytes-where", grantee)), pick.unwrap_or(false), crate::platform::Pick::FileOrFolder) {
                                        *dir = p;
                                    }
                                });
                                // The grantee also gives the record and the period wanted.
                                if grantee {
                                    st.field(ui, t(Key::U4RecordHash), Some(opt), None, |ui, w| fields::line(ui, &mut self.typed.dg_work, "0x\u{2026}", w, mono));
                                    st.field(ui, t(Key::WindowWanted), Some(opt), None, |ui, w| {
                                        // The two ends side by side, 14 apart (as two fields of a pair).
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
                        // The grantee saves a snapshot of the reading (its sheet takes the folder).
                        if grantee {
                            snapshot = f.go(ui, t(Key::DoSnapshot), "", true, "").clicked();
                        }
                    });
                });
                let address = if grantee { &self.typed.dg_address } else { &self.typed.rd_address };
                let filled = !address.trim().is_empty();
                let known = book.iter().any(|a| a.eq_ignore_ascii_case(address.trim()));
                st.foot(ui, |ui| {
                    let _page = page::Page::new();
                    read = key::show(ui, key::Key::new(t(Key::U3ReadIt), Role::Primary).enabled(filled).why(t(Key::U3OthersNotRead)).phase(phase)).clicked();
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
            self.act(a, now);
        }
        if self.shell.tasks.in_flight(kind) {
            card::card(ui, |ui| states::skeleton_lines(ui, &[0.3, 0.6, 0.5, 0.7]));
            return;
        }
        let Some(v) = self.other_view() else {
            return;
        };
        stagger(ui, 2, |ui| {
            ui.horizontal(|ui| {
                if let Some(i) = seg::seg(ui, "others-tabs", &[seg::Cell::from(t(Key::U3Overview)), seg::Cell::from(t(Key::U3EntryByEntry))], self.ux.u3.others_tab) {
                    self.ux.u3.others_tab = i;
                }
            });
        });
        // A segment above a block stands 16 from it, closer than two blocks.
        ui.add_space(tk::S4 - ui.spacing().item_spacing.y);
        let key = self.ux.u3.others_tab as u64;
        motion::swap(ui, egui::Id::new("others-tab-body"), key, |ui| {
            if self.ux.u3.others_tab == 0 {
                self.others_overview(ui, &v);
            } else if let Some(seq) = self.others_list(ui, &v) {
                self.push(Route::Other(seq), now);
            }
        });
    }

    /// Open "save snapshot": the folder last chosen for it, else the home's kits folder, else the documents folder.
    pub(super) fn snapshot_open(&mut self) {
        if self.typed.dg_snapshot.trim().is_empty() {
            self.typed.dg_snapshot = match self.shell.home.as_ref() {
                Some(h) => h.dir(crate::home::Slot::Kits).display().to_string(),
                None => self.shell.documents.clone(),
            };
        }
        self.ux.snapshot_sheet = true;
    }

    /// "Save snapshot": the folder in one row, "choose folder" at the bottom left, cancel and save at the
    /// bottom right; saving writes the reading there and closes the sheet.
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

    /// The overview: the ledger's state with its author and entry count, three figures (records, grants,
    /// last on chain); where its content came from and what failed, only when that says more; the grantee's
    /// conflict check; the grant history as rows; the raw values in the details list at the bottom.
    fn others_overview(&mut self, ui: &mut egui::Ui, v: &OtherView) {
        let grantee = v.dil.is_some();
        // The ledger's bytes are sought from four sources: which one supplied them, which failed.
        let supply = match (grantee, self.shell.book.as_ref(), v.dil.as_ref()) {
            (false, Some(Done::Book { from, files, misses, .. }), _) => Some((from.clone(), *files, misses.clone())),
            (true, _, Some(x)) => Some((x.from.clone(), x.files, x.misses.clone())),
            _ => None,
        };
        let unobtained = matches!(supply, Some((None, _, _)));
        // Name each network this pass could not read.
        let missed: Vec<crate::widex::Missed> = match (grantee, self.shell.book.as_ref(), v.dil.as_ref()) {
            (false, Some(Done::Book { missed, .. }), _) => missed.clone(),
            (true, _, Some(x)) => x.missed.clone(),
            _ => Vec::new(),
        };
        let deleted = crate::retractx::read(&v.timeline);
        let works: Vec<&crate::ledgerx::Row> = v.timeline.iter().filter(|r| r.kind == zikaron::tokens::EntryType::History && !deleted.is_deleted(&r.id)).collect();
        let last = v.timeline.iter().filter(|r| r.lamp == crate::ledgerx::Lamp::Anchored).map(|r| r.seq).max();
        let (audit, mark) = if v.entries == 0 {
            (t(Key::OnlyAnchors).to_string(), Mark::Todo)
        } else {
            (label_human(&v.label), if v.label == zikaron::tokens::Label::Complete.as_str() { Mark::Ok } else { Mark::Warn })
        };
        let (glyph, tint) = match mark {
            Mark::Ok => (Glyph::Ok, card::Tint::Ok),
            Mark::Todo => (Glyph::Info, card::Tint::Blue),
            _ => (Glyph::Warn, card::Tint::Warn),
        };
        // Content not obtained: the record and grant counts say so, never 0 (which would mean "none in the ledger").
        let count = |n: usize| if unobtained { t(Key::NotObtained).to_string() } else { n.to_string() };
        let when = match (v.latest.map(crate::when::day), last) {
            (Some(d), Some(s)) => format!("{d} \u{b7} #{s}"),
            (Some(d), None) => d,
            (None, Some(s)) => format!("#{s}"),
            (None, None) => t(Key::None_).to_string(),
        };
        let works_n = count(works.len());
        let grants_n = count(v.grants.len());
        let sub = format!("{} \u{b7} {}", card::short(&v.who), fill1(Key::U3EntriesCount, &v.entries.to_string()));
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
                // Where the content came from, only when it failed somewhere (the source itself is under details).
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
                    let _ = f.info(ui, &m.name, card::Show::Mark(Mark::Warn, t(m.reading.key())));
                }
            });
        });
        if let Some(x) = v.dil.as_ref() {
            let (m, said) = if x.work.is_empty() {
                (Mark::Todo, t(Key::DoubleSaleNoWork).to_string())
            } else if x.window.is_none() {
                (Mark::Todo, t(Key::DoubleSaleNoWindow).to_string())
            } else if x.double_sold() {
                (Mark::Bad, fill1(Key::DoubleSaleRed, &x.clash.len().to_string()))
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
        // The grant history: one row per grant, its record, its validity and when it was first anchored.
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
                            .find(|r| r.kind == zikaron::tokens::EntryType::History && r.work.as_deref().map(|w| w.eq_ignore_ascii_case(work)).unwrap_or(false))
                            .map(human_summary)
                            .unwrap_or_else(|| t(Key::UnnamedRecord).to_string())
                    };
                    for g in &v.grants {
                        let at = v.timeline.iter().find(|r| r.id.eq_ignore_ascii_case(&g.id)).and_then(|r| r.anchored_at);
                        let _ = f.line(ui, None, &name_of(&g.work), "", false, false, c(C::Ink), |ui| {
                            ui.label(egui::RichText::new(first_anchor_say(at)).font(Type::MonoSmall.font()).color(c(C::Ink3)));
                            ui.label(egui::RichText::new(window_short(g.window)).font(Type::Note.font()).color(c(C::Ink2)));
                        });
                    }
                });
            });
        });
        // Details: the author, the ledger's head, where its content came from, the anchors on chain, the
        // check's label; the grantee's key lineage, each handover and the deepest anchor.
        let head = v.timeline.iter().max_by_key(|r| r.seq).map(|r| r.id.clone());
        let mut rows: Vec<(&str, Val)> = vec![(t(Key::EntryAuthor), Val::mono(v.who.clone()))];
        if let Some(h) = head {
            rows.push((t(Key::AuHead), Val::mono(h)));
        }
        if let Some((_, said)) = &source {
            rows.push((t(Key::LedgerSources), Val::text(said.clone())));
        }
        rows.push((t(Key::AnchorsOnChain), Val::figure(v.anchors.to_string())));
        rows.push((t(Key::AuditLabel), Val::figure(v.label.clone())));
        if let Some(x) = v.dil.as_ref() {
            rows.push((t(Key::Lineage), if x.lineage.is_empty() { Val::text(t(Key::None_)) } else { Val::mono(x.lineage.join(" \u{2192} ")) }));
            if !x.successions.is_empty() {
                rows.push((t(Key::TagHandover), Val::mono(x.successions.iter().map(|one| format!("#{} \u{b7} {} \u{b7} {}", one.seq, one.kind, one.to)).collect::<Vec<_>>().join("\n"))));
            }
            if let Some(three) = x.depth.clone().map(crate::depthx::three) {
                // A network left out this pass: "deepest" with no anchor read says the chain was not read (same
                // rule as the verify page, `depthx::said`).
                let deepest = match crate::depthx::said(&three, !x.missed.is_empty()).1 {
                    crate::depthx::Said::Is(n) => Val::figure(n.to_string()),
                    _ => Val::text(t(Key::U4ChainUnread)),
                };
                rows.push((t(Key::U3Deepest), deepest));
                rows.push((t(Key::AuditLabel), Val::figure(three.label.clone())));
            }
        }
        stagger(ui, 6, |ui| details_card(ui, "others-lineage", &rows));
    }

    /// The entry list, newest first, with records this desk reads as deleted struck through. Returns the seq of
    /// a clicked row.
    fn others_list(&mut self, ui: &mut egui::Ui, v: &OtherView) -> Option<u64> {
        let mut timeline = v.timeline.clone();
        timeline.sort_by(|a, b| b.seq.cmp(&a.seq));
        let reading = crate::retractx::read(&timeline);
        // The numbers stand at the column's left, level with the card's other left edges.
        let cols = [table::SEQ, table::TYPE, table::col("", table::Col::Fr(1.0)), table::col("", table::Col::Px(200.0)), table::MARK];
        let rows: Vec<table::Row> = timeline
            .iter()
            .map(|row| {
                let (tag, summary, struck) = row_face(&timeline, &reading, row);
                table::Row {
                    cells: vec![table::Cell::Seq(row.seq), table::Cell::Tag(tag.to_string()), table::Cell::Text(summary), table::Cell::Mono(first_anchor_say(row.anchored_at)), table::Cell::Mark(lamp_mark(row.lamp))],
                    click: true,
                    gone: struck,
                    on: false,
                }
            })
            .collect();
        let hit = stagger(ui, 3, |ui| table::table(ui, "others-entries", &cols, false, &rows, t(Key::OnlyAnchors)));
        hit.clicked.map(|i| timeline[i].seq)
    }

    /// Another ledger's entry, read only: its name large with its number and kind under it and its state on the
    /// right; the basic information (state on chain with the first anchor's time, entry number); the author and
    /// entry id under details.
    pub(super) fn other_entry(&mut self, ui: &mut egui::Ui, seq: u64, _now: f64) {
        let Some(v) = self.other_view() else {
            states::empty(ui, Glyph::Others, t(Key::U3OthersNotRead));
            return;
        };
        let reading = crate::retractx::read(&v.timeline);
        let Some(row) = v.timeline.iter().find(|r| r.seq == seq).cloned() else {
            states::empty(ui, Glyph::Others, t(Key::DetailGone));
            return;
        };
        let (tag, summary, struck) = row_face(&v.timeline, &reading, &row);
        stagger(ui, 0, |ui| {
            card::hero(ui, &summary, &format!("#{} \u{b7} {}", row.seq, tag), struck, &[lamp_hero(row.lamp, row.anchored_at)]);
        });
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::BasicInfo), "", |ui| {
                card::form(ui, |ui, f| {
                    anchor_rows(ui, f, row.lamp, row.anchored_at, row.anchored_at);
                    let _ = f.info(ui, t(Key::V2RecordNo), card::Show::Mono(&format!("#{}", row.seq)));
                });
            });
        });
        stagger(ui, 2, |ui| {
            details_card(
                ui,
                "other-entry-details",
                &[(t(Key::EntryAuthor), Val::mono(v.who.clone())), (t(Key::DetailPick), Val::mono(row.id.clone())), (t(Key::U4RecordFirstAt), Val::figure(first_anchor_say(row.anchored_at)))],
            )
        });
    }

    /// The toolbar title of another ledger's entry.
    pub(super) fn other_title(&self, seq: u64) -> String {
        self.other_view()
            .and_then(|v| {
                let reading = crate::retractx::read(&v.timeline);
                v.timeline.iter().find(|r| r.seq == seq).map(|r| format!("#{} \u{b7} {}", r.seq, row_face(&v.timeline, &reading, r).0))
            })
            .unwrap_or_else(|| format!("#{seq}"))
    }
}

/// A figure that is not there (content not obtained, chain not read) in the quiet ink, else as words.
pub(super) fn fact_of(s: &str, missing: bool) -> card::Fact<'_> {
    if missing { card::Fact::Quiet(s) } else { card::Fact::Text(s) }
}

/// Which source supplied the material, in plain words (this machine, the vault, a record kit, a publish address).
pub(super) fn level_key(l: crate::supplyx::Level) -> Key {
    match l {
        crate::supplyx::Level::Local => Key::SourceLocal,
        crate::supplyx::Level::Vault => Key::SourceVault,
        crate::supplyx::Level::Kit => Key::PageKit,
        crate::supplyx::Level::Remote => Key::SetPublish,
    }
}

/// The words beside a grey check: what is missing (where to get it is in the gap's sentence).
pub(super) fn gap_words(g: &crate::checkx::Gap) -> String {
    use crate::checkx::Gap;
    match g {
        Gap::NoLedger => t(Key::GapNoLedger).to_string(),
        Gap::LedgerRefused(_) => t(Key::GapLedgerRefused).to_string(),
        Gap::NoNode => format!("{} \u{b7} {}", t(Key::NoEndpointYet), t(Key::CheckGoSettings)),
        Gap::ChainUnread(_) => t(Key::GapChainUnread).to_string(),
        Gap::NotYetAnchored => t(Key::GapNotYetAnchored).to_string(),
        Gap::NoTime => t(Key::GapNoTime).to_string(),
    }
}

/// The sentence of a kind of gap (what is missing, where to get it).
pub(super) fn gap_note(g: &crate::checkx::Gap) -> Key {
    use crate::checkx::Gap;
    match g {
        Gap::NoLedger => Key::NoteNoLedger,
        Gap::LedgerRefused(_) => Key::NoteLedgerRefused,
        Gap::NoNode => Key::NoteNoNode,
        Gap::ChainUnread(_) => Key::NoteChainUnread,
        Gap::NotYetAnchored => Key::NoteNotYetAnchored,
        Gap::NoTime => Key::NoteNoTime,
    }
}

/// Whether a grant is exclusive, in plain words (recorded at signing, or not).
pub(super) fn exclusive_key(x: crate::termsx::Exclusive) -> Key {
    match x {
        crate::termsx::Exclusive::Signed => Key::ExclusiveSigned,
        crate::termsx::Exclusive::No => Key::ExclusiveNo,
    }
}

/// The six checks' names in plain words (the raw token is under details).
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
