//! The verify view: its tabs (verify a grant, verify records, others' ledgers), and the grantee's record
//! verification.

use super::*;

impl Win {
    pub(super) fn verify_page(&mut self, ui: &mut egui::Ui, tab: u8, now: f64) {
        use crate::nav::tab as T;
        match tab {
            T::VERIFY_WORK if self.shell.settings.role == crate::roles::Role::Grantee => self.kit_verify(ui, now),
            T::VERIFY_OTHERS => self.others_page(ui, now),
            _ => self.check_page(ui, now),
        }
    }

    /// Verify records: drop a record kit or record file; the content place and the record hash under
    /// advanced options; the result names each record.
    fn kit_verify(&mut self, ui: &mut egui::Ui, now: f64) {
        let v = self.shell.verified.clone();
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Verify);
        let mut go = false;
        // A file changed while a pass ran is verified once that pass ends.
        if !busy && std::mem::take(&mut self.ux.u4.verify_again) {
            go = true;
        }
        // A kit dropped on home or on the window starts as soon as the page is up.
        if self.ux.u4.verify_autorun && !self.typed.vf_path.trim().is_empty() {
            self.ux.u4.verify_autorun = false;
            go = true;
        }
        let name = (!self.typed.vf_path.trim().is_empty()).then(|| fill1(Key::U3Chosen, &width::file_name(&self.typed.vf_path)));
        let d = stagger(ui, 0, |ui| {
            drop::zone(
                ui,
                "verify-drop",
                Some(Glyph::Kit),
                &[t(Key::KitvDrop), t(Key::DropClickAny)],
                name.as_deref().map(|n| (n, "", t(Key::DropClickSwap))),
                120.0,
                drop::Shape::Column,
                true,
            )
        });
        if let Some(p) = self.drop_or_pick(&d, crate::platform::Pick::FileOrFolder, now) {
            self.typed.vf_path = p;
            go = true;
        }
        stagger(ui, 1, |ui| {
            card::card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = tk::S4;
                fold::fold(ui, "verify-more", t(Key::U3MoreOptions), |ui| {
                    ui.spacing_mut().item_spacing.y = tk::S3;
                    // The form that cannot be dropped (a publish address) is typed here.
                    field(ui, t(Key::U3BytesWhere), None, |ui| input::mono(ui, &mut self.typed.vf_path, t(Key::BytesWhereHint)));
                    if crate::fetchx::is_address(&self.typed.vf_path) {
                        if let Err(f) = crate::fetchx::base_of(&self.typed.vf_path) {
                            states::hint_ex(ui, f.human(), true);
                        }
                    }
                    field(ui, t(Key::VerifierWork), None, |ui| input::mono(ui, &mut self.typed.vf_work, "0x\u{2026}"));
                });
                let _page = page::Page::new();
                if key::show(ui, key::Key::new(t(Key::DoVerifyWork), Role::Primary).enabled(!self.typed.vf_path.trim().is_empty()).phase(self.phase_of(crate::task::Kind::Verify))).clicked() {
                    go = true;
                }
                self.stage_line(ui, crate::task::Kind::Verify);
            });
        });
        if busy {
            card::card(ui, |ui| states::skeleton_lines(ui, &[0.3, 0.6, 0.5, 0.7]));
        } else if let Some(x) = v.as_ref() {
            stagger(ui, 2, |ui| {
                card::card(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = tk::S3;
                    card_title(ui, t(Key::U4VerifyResult));
                    // Fetched from a publish address: the source is named, with "fetch again".
                    if let crate::verifyx::Source::Remote(b) = &x.source {
                        let n = x.kit.as_ref().map(|k| k.entries + k.files + k.proofs + 1).unwrap_or(0);
                        let (_, again) = width::then(ui, |ui| key::link(ui, t(Key::CheckRefetch)).clicked(), |ui, room| paint::line(ui, &fill2(Key::CheckFromAddress, b.as_str(), &n.to_string()), Type::Small, c(C::Ink2), room));
                        if again {
                            go = true;
                        }
                    }
                    let kit = match &x.kit {
                        Some(k) if k.ok => (Mark::Ok, t(Key::U4KitGood).to_string()),
                        Some(k) => (Mark::Bad, fill1(Key::U4KitBad, &width::file_name(&k.subject))),
                        None => (Mark::Todo, t(Key::U4NotAKit).to_string()),
                    };
                    let review = match &x.review {
                        Ok(a) if a.unanchored.is_empty() && a.anchored > 0 => (Mark::Ok, fill1(Key::U4AnchorsMatch, &a.anchored.to_string())),
                        Ok(a) => (Mark::Warn, fill1(Key::U4AnchorsBehind, &a.unanchored.len().to_string())),
                        Err(_) => (Mark::Todo, t(Key::U4AnchorsUnread).to_string()),
                    };
                    let label = match &x.review {
                        // A kit carries only the chosen entries and the ledger's spine: gaps in seq are
                        // normal for a kit ("part of the ledger"), not a warning.
                        Ok(a) if x.kit.is_some() && a.label == zikaron::tokens::Label::Gaps.as_str() => (Mark::Ok, t(Key::U4IssuerPartial).to_string()),
                        Ok(a) => (if a.label == zikaron::tokens::Label::Complete.as_str() { Mark::Ok } else { Mark::Warn }, fill1(Key::U4IssuerAudit, &label_human(&a.label))),
                        Err(_) => (Mark::Todo, t(Key::U4IssuerAuditUnread).to_string()),
                    };
                    states::checks(ui, &[(kit.0, kit.1, None), (review.0, review.1, None), (label.0, label.1, None)], false);
                    if let Some(three) = x.depth.clone().map(crate::depthx::three) {
                        let first = three.earliest.map(crate::when::when).unwrap_or_else(|| t(Key::None_).to_string());
                        let span = format!("{} / {}", three.anchored, three.span);
                        super::kit::stat_cells(ui, &[(t(Key::U3FirstAnchored), &first), (t(Key::U3Deepest), &three.deepest.to_string()), (t(Key::U3Continuity), &span)]);
                    }
                    if !x.records.is_empty() {
                        card::flat_title(ui, t(Key::U4RecordsTitle));
                        let cols = [
                            table::col(t(Key::U3Work), table::Col::Fr(1.0)),
                            table::col(t(Key::U4RecordOriginal), table::Col::Px(90.0)),
                            table::col(t(Key::NavAnchoring), table::Col::Px(90.0)),
                            table::col_r(t(Key::U4RecordFirstAt), table::Col::Px(190.0)),
                        ];
                        let rows: Vec<table::Row> = x
                            .records
                            .iter()
                            .map(|r| {
                                use crate::verifyx::{OnChain, Original};
                                let (orig, tone) = match r.original {
                                    Original::Match => (t(Key::U4OriginalMatch), PillTone::Ok),
                                    Original::Mismatch => (t(Key::U4OriginalMismatch), PillTone::Bad),
                                    Original::Missing => (t(Key::U4OriginalMissing), PillTone::Grey),
                                };
                                let (chain, when) = match r.chain {
                                    OnChain::Anchored { first_at } => (t(Key::LampAnchored).to_string(), crate::when::when(first_at)),
                                    OnChain::NotAnchored => (t(Key::V2StateLanded).to_string(), t(Key::None_).to_string()),
                                    OnChain::Unread => (t(Key::U4ChainUnread).to_string(), t(Key::None_).to_string()),
                                };
                                let name = r.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| t(Key::UnnamedRecord).to_string());
                                table::Row { cells: vec![table::Cell::Text(name), table::Cell::Pill(orig.to_string(), tone), table::Cell::Text(chain), table::Cell::Mono(when)], ..Default::default() }
                            })
                            .collect();
                        table::table(ui, "verify-records", &cols, true, &rows, "");
                    }
                    if x.mismatches.is_empty() {
                        states::banner(ui, states::Banner::Ok, t(Key::U4NoMismatch), |_| ());
                    } else {
                        states::err_box(ui, "verify-mismatch", &fill1(Key::U4Mismatches, &x.mismatches.len().to_string()), t(Key::U4MismatchNext), t(Key::U3RawError), &x.mismatches.join("\n"));
                    }
                    fold::fold(ui, "verify-evidence", t(Key::SetEvidence), |ui| {
                        let mut rows: Vec<(&str, Val)> = vec![
                            (
                                t(Key::U4Source),
                                Val::text(t(match x.source {
                                    crate::verifyx::Source::Kit(_) => Key::SourceKit,
                                    crate::verifyx::Source::Bytes(_) => Key::SourceBytes,
                                    crate::verifyx::Source::File(_) => Key::SourceGrantFile,
                                    crate::verifyx::Source::Remote(_) => Key::SetPublish,
                                })),
                            ),
                            (t(Key::LedgerEntries), Val::mono(x.entries.to_string())),
                            (t(Key::U4Rejected), Val::mono(x.rejected.len().to_string())),
                        ];
                        let hashes = x.records.iter().map(|r| r.id.clone()).collect::<Vec<_>>().join("\n");
                        if !hashes.is_empty() {
                            rows.push((t(Key::U4RecordHash), Val::mono(hashes)));
                        }
                        if let Some(k) = &x.kit {
                            rows.push((t(Key::KitId), Val::mono(k.kit_id.clone())));
                            rows.push((t(Key::U3Verdict), Val::mono(k.verdict.clone())));
                        }
                        match &x.review {
                            Ok(a) => rows.push((t(Key::AuditLabel), Val::mono(a.label.clone()))),
                            Err(said) => rows.push((t(Key::U3RawError), Val::mono(said.clone()))),
                        }
                        kv::kv(ui, &rows);
                    });
                });
            });
        }
        if go {
            if busy {
                self.ux.u4.verify_again = true;
            } else {
                let a = Action::VerifyWork { path: self.typed.vf_path.clone(), work: self.typed.vf_work.clone() };
                self.act(a, now);
            }
        }
    }
}
