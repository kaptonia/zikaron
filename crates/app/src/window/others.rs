//! Others' ledgers (both seats; the grantee's reading adds the grant conflict check and the key lineage):
//! read by author address, an overview and the entry list, each entry opening a read-only detail page.

use super::*;

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
        stagger(ui, 0, |ui| {
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S4;
                field(ui, t(Key::U3AuthorAddress), None, |ui| {
                    let address = if grantee { &mut self.typed.dg_address } else { &mut self.typed.rd_address };
                    let heads: Vec<String> = book.iter().map(|a| head_tail(a)).collect();
                    let items: Vec<menu::Item> = heads.iter().map(|h| menu::Item::Row(menu::Row { lead: t(Key::Address), label: h, mono: true, ..Default::default() })).collect();
                    let spec = pick::Spec { hint: t(Key::SearchAddress), empty: t(Key::SearchNone), w: PICK_W, dates: None };
                    let keep = |i: usize, q: &str, _: &str, _: &str| matches(q, &[&book[i]]);
                    let mut picked = None;
                    width::then(
                        ui,
                        |ui| picked = pick::key(ui, "others-book", t(Key::U3SeenBeforeDots), !book.is_empty(), false, &spec, &items, &keep),
                        |ui, room| input::field(ui, address, "0x\u{2026}", room, input::Look { mono: true, ..Default::default() }),
                    );
                    if let Some(i) = picked {
                        *address = book[i].clone();
                    }
                });
                fold::fold(ui, "others-more", t(Key::U3MoreOptions), |ui| {
                    ui.spacing_mut().item_spacing.y = tk::S3;
                    // Record content: a local file or folder (kit, grant file, ledger folder) or an https
                    // publish address.
                    field(ui, t(Key::U3BytesWhere), None, |ui| {
                        let dir = if grantee { &mut self.typed.dg_dir } else { &mut self.typed.rd_dir };
                        let (_, pick) = width::line_then(ui, dir, t(Key::BytesWhereHint), true, |ui| key::key(ui, t(Key::PickFolder), Role::Secondary, true).clicked());
                        if pick {
                            if let Some(p) = crate::platform::choose_path(crate::platform::Pick::FileOrFolder) {
                                *dir = p;
                            }
                        }
                        if crate::fetchx::is_address(dir) {
                            if let Err(f) = crate::fetchx::base_of(dir) {
                                states::hint_ex(ui, f.human(), true);
                            }
                        }
                    });
                    // The grantee also gives the record, the period wanted, and where a snapshot goes.
                    if grantee {
                        field(ui, t(Key::VerifierWork), None, |ui| input::mono(ui, &mut self.typed.dg_work, "0x\u{2026}"));
                        field(ui, t(Key::U4MyWindow), None, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = tk::S2;
                                input::field(ui, &mut self.typed.dg_from, t(Key::U4FromSecs), 180.0, input::Look { mono: true, ..Default::default() });
                                input::field(ui, &mut self.typed.dg_to, t(Key::U4ToSecs), 180.0, input::Look { mono: true, ..Default::default() });
                            });
                        });
                        if self.typed.dg_snapshot.trim().is_empty() {
                            if let Some(h) = self.shell.home.as_ref() {
                                self.typed.dg_snapshot = h.dir(crate::home::Slot::Kits).display().to_string();
                            }
                        }
                        Self::place_row(ui, Key::SnapshotOut, &mut self.typed.dg_snapshot);
                        snapshot = key::key(ui, t(Key::DoSnapshot), Role::Secondary, Self::landing_ok(&self.typed.dg_snapshot)).clicked();
                    }
                });
                let address = if grantee { &self.typed.dg_address } else { &self.typed.rd_address };
                let filled = !address.trim().is_empty();
                let known = book.iter().any(|a| a.eq_ignore_ascii_case(address.trim()));
                keys_row(ui, |ui| {
                    let _page = page::Page::new();
                    read = key::show(ui, key::Key::new(t(Key::U3ReadIt), Role::Primary).enabled(filled).phase(phase)).clicked();
                    if known {
                        forget = key::key(ui, t(Key::U3ForgetAddr), Role::Secondary, true).clicked();
                    } else {
                        remember = key::key(ui, t(Key::U3RememberAddr), Role::Secondary, filled).clicked();
                    }
                });
                self.stage_line(ui, kind);
            });
        });
        let address = if grantee { self.typed.dg_address.clone() } else { self.typed.rd_address.clone() };
        if remember {
            self.act(Action::RememberAddress { address: address.clone() }, now);
        }
        if forget {
            self.act(Action::ForgetAddress { address: address.clone() }, now);
        }
        if snapshot {
            let who: String = self.typed.dg_address.trim().trim_start_matches("0x").chars().take(10).collect();
            let chosen = crate::home::choose(&crate::home::Kind::File { stem: format!("snapshot-{who}"), ext: "json".into() }, std::path::Path::new(self.typed.dg_snapshot.trim()));
            self.remember_landing(Out::Snapshot, &chosen);
            let a = Action::SaveSnapshot { to: chosen.at.display().to_string() };
            self.act(a, now);
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
            card::grid(ui, "others-skeleton", 4, 150.0, |ui, _| {
                card::card(ui, |ui| states::skeleton_lines(ui, &[0.5, 0.4, 0.7]));
            });
            return;
        }
        let Some(v) = self.other_view() else {
            stagger(ui, 1, |ui| card::flat(ui, |ui| states::empty_pad(ui, Glyph::Others, t(Key::U3OthersNotRead), 28.0)));
            return;
        };
        stagger(ui, 1, |ui| {
            if let Some(i) = seg::tabs(ui, "others-tabs", &[t(Key::U3Overview), t(Key::U3EntryByEntry)], self.ux.u3.others_tab) {
                self.ux.u3.others_tab = i;
            }
        });
        let key = self.ux.u3.others_tab as u64;
        motion::swap(ui, egui::Id::new("others-tab-body"), key, |ui| {
            if self.ux.u3.others_tab == 0 {
                self.others_overview(ui, &v);
            } else if let Some(seq) = self.others_list(ui, &v) {
                self.push(Route::Other(seq), now);
            }
        });
    }

    /// Four tiles, where the ledger came from, the grantee's conflict check, and the grant history (no
    /// addresses in front).
    fn others_overview(&mut self, ui: &mut egui::Ui, v: &OtherView) {
        let grantee = v.dil.is_some();
        // The ledger's bytes go through the four sources: which one supplied them, which failed.
        let supply = match (grantee, self.shell.book.as_ref(), v.dil.as_ref()) {
            (false, Some(Done::Book { from, files, misses, .. }), _) => Some((from.clone(), *files, misses.clone())),
            (true, _, Some(x)) => Some((x.from.clone(), x.files, x.misses.clone())),
            _ => None,
        };
        let unobtained = matches!(supply, Some((None, _, _)));
        // Networks this pass could not read, each named.
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
        // Content not obtained: the record and grant counts say so, never 0 ("none in the ledger").
        let count = |n: usize| if unobtained { t(Key::NotObtained).to_string() } else { n.to_string() };
        let when = v.latest.map(crate::when::day).or_else(|| last.map(|s| format!("#{s}"))).unwrap_or_else(|| t(Key::None_).to_string());
        let last_say = last.map(|s| format!("#{s}")).unwrap_or_default();
        let newest = works.iter().max_by_key(|w| w.seq).map(|w| human_summary(w)).unwrap_or_default();
        let entries_say = fill1(Key::U3EntriesCount, &v.entries.to_string());
        let anchors_say = fill1(Key::U3AnchorsSeen, &v.anchors.to_string());
        let works_n = count(works.len());
        let grants_n = count(v.grants.len());
        stagger(ui, 2, |ui| {
            card::grid(ui, "others-overview", 4, 150.0, |ui, i| {
                let tile = match i {
                    0 => card::Tile { title: t(Key::WbAudit), figure: &audit, mark: Some(mark), figure_colour: None, sub: &entries_say, mono_figure: false, clickable: false },
                    1 => card::Tile { title: t(Key::U3Works), figure: &works_n, mark: None, figure_colour: None, sub: &newest, mono_figure: false, clickable: false },
                    2 => card::Tile { title: t(Key::ReaderGrants), figure: &grants_n, mark: None, figure_colour: None, sub: &anchors_say, mono_figure: false, clickable: false },
                    _ => card::Tile { title: t(Key::U3LastAnchored), figure: &when, mark: None, figure_colour: None, sub: &last_say, mono_figure: true, clickable: false },
                };
                card::tile(ui, &tile);
            });
        });
        stagger(ui, 3, |ui| {
            match &supply {
                Some((Some((level, place)), files, misses)) => {
                    let said = match (level, files) {
                        (crate::supplyx::Level::Remote, Some(n)) => format!("{} \u{b7} {}", fill1(Key::CheckSourceLine, t(level_key(*level))), fill2(Key::CheckFromAddress, &width::file_name(place), &n.to_string())),
                        _ => fill1(Key::CheckSourceLine, t(level_key(*level))),
                    };
                    paint::text(ui, &said, Type::Note, c(C::Ink2));
                    for (level, f) in misses {
                        hint(ui, &fill2(Key::CheckMissRow, t(level_key(*level)), f.human()));
                    }
                }
                Some((None, _, misses)) => {
                    states::note_box(ui, t(Key::ReaderNoContent));
                    for (level, f) in misses {
                        hint(ui, &fill2(Key::CheckMissRow, t(level_key(*level)), f.human()));
                    }
                }
                None => {}
            }
            for m in &missed {
                states::okline(ui, Mark::Warn, &format!("{} {}", m.name, t(m.reading.key())));
            }
        });
        if let Some(x) = v.dil.as_ref() {
            stagger(ui, 4, |ui| {
                card::flat(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = tk::S2;
                    card::flat_title(ui, t(Key::DoubleSale));
                    let (m, said) = if x.work.is_empty() {
                        (Mark::Todo, t(Key::DoubleSaleNoWork).to_string())
                    } else if x.window.is_none() {
                        (Mark::Todo, t(Key::DoubleSaleNoWindow).to_string())
                    } else if x.double_sold() {
                        (Mark::Bad, fill1(Key::DoubleSaleRed, &x.clash.len().to_string()))
                    } else {
                        (Mark::Ok, t(Key::DoubleSaleGreen).to_string())
                    };
                    states::okline(ui, m, &said);
                    fold::fold(ui, "others-lineage", t(Key::SetEvidence), |ui| {
                        let mut rows: Vec<(String, Val)> = vec![(
                            t(Key::Lineage).to_string(),
                            Val::mono(if x.lineage.is_empty() { t(Key::None_).to_string() } else { x.lineage.join(" \u{2192} ") }),
                        )];
                        for one in &x.successions {
                            rows.push((format!("#{} \u{b7} {}", one.seq, one.kind), Val::mono(one.to.clone())));
                        }
                        if let Some(three) = x.depth.clone().map(crate::depthx::three) {
                            // A network left out this pass: "deepest" with no anchor read says the chain was not
                            // read (the same rule as the verify page, `depthx::said`).
                            let deepest = match crate::depthx::said(&three, !x.missed.is_empty()).1 {
                                crate::depthx::Said::Is(n) => n.to_string(),
                                _ => t(Key::U4ChainUnread).to_string(),
                            };
                            rows.push((t(Key::U3Deepest).to_string(), Val::mono(deepest)));
                            rows.push((t(Key::AuditLabel).to_string(), Val::mono(three.label.clone())));
                        }
                        let refs: Vec<(&str, Val)> = rows.iter().map(|(a, b)| (a.as_str(), b.clone())).collect();
                        kv::kv(ui, &refs);
                    });
                });
            });
        }
        stagger(ui, 5, |ui| {
            card::flat(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S2;
                card::flat_title(ui, t(Key::ReaderGrants));
                if v.grants.is_empty() {
                    hint(ui, t(Key::U3NoGrants));
                    return;
                }
                let name_of = |work: &str| {
                    v.timeline
                        .iter()
                        .find(|r| r.kind == zikaron::tokens::EntryType::History && r.work.as_deref().map(|w| w.eq_ignore_ascii_case(work)).unwrap_or(false))
                        .map(human_summary)
                        .unwrap_or_else(|| t(Key::UnnamedRecord).to_string())
                };
                let cols = [table::col(t(Key::OsWork), table::Col::Fr(1.0)), table::col(t(Key::U3Window), table::Col::Fr(1.2)), table::col_r(t(Key::U4RecordFirstAt), table::Col::Px(190.0))];
                let rows: Vec<table::Row> = v
                    .grants
                    .iter()
                    .map(|g| {
                        let at = v.timeline.iter().find(|r| r.id.eq_ignore_ascii_case(&g.id)).and_then(|r| r.anchored_at);
                        table::Row { cells: vec![table::Cell::Text(name_of(&g.work)), table::Cell::Mono(window_short(g.window)), table::Cell::Mono(first_anchor_say(at))], ..Default::default() }
                    })
                    .collect();
                table::table(ui, "others-grants", &cols, true, &rows, "");
            });
        });
    }

    /// The entry list, newest first; deleted records struck through by this desk's reading. Returns the seq
    /// of a clicked row.
    fn others_list(&mut self, ui: &mut egui::Ui, v: &OtherView) -> Option<u64> {
        let mut timeline = v.timeline.clone();
        timeline.sort_by(|a, b| b.seq.cmp(&a.seq));
        let reading = crate::retractx::read(&timeline);
        let cols = [table::SEQ, table::TYPE, table::col("", table::Col::Fr(1.0)), table::col("", table::Col::Px(160.0)), table::MARK, table::CHEV];
        let rows: Vec<table::Row> = timeline
            .iter()
            .map(|row| {
                let (tag, summary, struck) = row_face(&timeline, &reading, row);
                table::Row {
                    cells: vec![
                        table::Cell::Seq(row.seq),
                        table::Cell::Tag(tag.to_string()),
                        table::Cell::Text(summary),
                        table::Cell::Mono(first_anchor_say(row.anchored_at)),
                        table::Cell::Mark(lamp_mark(row.lamp)),
                        table::Cell::Chev,
                    ],
                    click: true,
                    gone: struck,
                    on: false,
                }
            })
            .collect();
        let hit = stagger(ui, 2, |ui| table::table(ui, "others-entries", &cols, false, &rows, t(Key::OnlyAnchors)));
        hit.clicked.map(|i| timeline[i].seq)
    }

    /// Another ledger's entry, read only: summary, on-chain state and the first anchor time; the author and
    /// the entry id under details.
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
            card::hero(ui, &format!("#{} \u{b7} {}", row.seq, tag), &summary, struck, |ui| lamp_pill_ui(ui, row.lamp, row.anchored_at));
        });
        stagger(ui, 1, |ui| {
            kv_section(
                ui,
                t(Key::BasicInfo),
                &[
                    (t(Key::U3Entry), Val::text(summary.clone())),
                    (t(Key::NavAnchoring), Val::text(lamp_label(row.lamp, true, row.anchored_at))),
                    (t(Key::U4RecordFirstAt), Val::mono(first_anchor_say(row.anchored_at))),
                ],
            );
        });
        stagger(ui, 2, |ui| details_card(ui, "other-entry-details", &[(t(Key::EntryAuthor), Val::mono(v.who.clone())), (t(Key::DetailPick), Val::mono(row.id.clone()))]));
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

/// Which source supplied the material, in plain words (this Mac, the vault, a record kit, a publish address).
pub(super) fn level_key(l: crate::supplyx::Level) -> Key {
    match l {
        crate::supplyx::Level::Local => Key::SourceLocal,
        crate::supplyx::Level::Vault => Key::SourceVault,
        crate::supplyx::Level::Kit => Key::PageKit,
        crate::supplyx::Level::Remote => Key::SetPublish,
    }
}

/// The words beside a grey check: what is missing (where to get it is the gap's sentence).
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

/// Where exclusivity comes from, in plain words (recorded at signing, the former list, none).
pub(super) fn exclusive_key(x: crate::termsx::Exclusive) -> Key {
    match x {
        crate::termsx::Exclusive::Signed => Key::ExclusiveSigned,
        crate::termsx::Exclusive::Legacy => Key::ExclusiveLegacy,
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
