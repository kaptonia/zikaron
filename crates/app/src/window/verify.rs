//! The verify view: its tabs (verify a grant, verify records, others' ledgers), and the grantee's record
//! verification.

use super::*;

impl Win {
    pub(super) fn verify_page(&mut self, ui: &mut egui::Ui, tab: u8, now: f64) {
        use crate::nav::tab as T;
        match tab {
            T::VERIFY_WORK => self.kit_verify(ui, now),
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
        let mut add_network: Option<crate::kitsindex::AnchoredOn> = None;
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
                        Ok(a) if a.unanchored.is_empty() && a.unread.is_empty() && a.anchored > 0 => (Mark::Ok, fill1(Key::U4AnchorsMatch, &a.anchored.to_string())),
                        // A network left out this pass: what no read chain reaches is not called unanchored.
                        Ok(a) if a.unanchored.is_empty() && !a.unread.is_empty() => (Mark::Todo, t(Key::U4AnchorsUnread).to_string()),
                        Ok(a) => (Mark::Warn, fill1(Key::U4AnchorsBehind, &a.unanchored.len().to_string())),
                        Err(_) if x.not_added.is_some() => (Mark::Warn, t(Key::VerifyNotAdded).to_string()),
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
                    // The kit names a network that is not added: no chain was read; one key adds it.
                    if let Some(at) = &x.not_added {
                        let said = format!("{} · {} · {}", self.chain_label(at.chain_id), t(Key::VerifyAuthorSays), t(Key::VerifyAddNetwork));
                        if states::banner(ui, states::Banner::Warn, &said, |ui| key::key(ui, t(Key::ReadNetAdd), Role::Secondary, true).clicked()) {
                            add_network = Some(at.clone());
                        }
                    }
                    // Networks read, by name; those not read, each with why.
                    if x.review.is_ok() && !x.read.is_empty() {
                        hint(ui, &x.read.iter().map(|c| self.chain_label(*c)).collect::<Vec<_>>().join(t(Key::ListJoin)));
                    }
                    for m in &x.missed {
                        states::okline(ui, Mark::Warn, &format!("{} {}", m.name, t(m.reading.key())));
                    }
                    if let Some(three) = x.depth.clone().map(crate::depthx::three) {
                        // A chain not read this pass (none read, or a network left out): a measure that reads
                        // "none" or falls short may have its anchor on the chain not read, so it says the chain
                        // was not read. Values read on the chains that were read show as they are.
                        use crate::depthx::Said;
                        let (first, deepest, span) = crate::depthx::said(&three, x.review.is_err() || !x.missed.is_empty());
                        let first = match first {
                            Said::Is(at) => crate::when::when(at),
                            Said::Nothing => t(Key::None_).to_string(),
                            Said::ChainUnread => t(Key::U4ChainUnread).to_string(),
                        };
                        let deepest = match deepest {
                            Said::Is(n) => n.to_string(),
                            _ => t(Key::U4ChainUnread).to_string(),
                        };
                        let span = match span {
                            Said::Is((a, s)) => format!("{a} / {s}"),
                            _ => t(Key::U4ChainUnread).to_string(),
                        };
                        super::kit::stat_cells(ui, &[(t(Key::U3FirstAnchored), &first), (t(Key::U3Deepest), &deepest), (t(Key::U3Continuity), &span)]);
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
                        // Where each record's first anchor is: chain, registry, block and transaction in full.
                        let firsts: Vec<(String, &crate::auditx::FirstAnchor)> = x
                            .records
                            .iter()
                            .filter_map(|r| r.first.as_ref().map(|f| (r.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| t(Key::UnnamedRecord).to_string()), f)))
                            .collect();
                        let mut chains: Vec<String> = firsts.iter().map(|(_, f)| f.chain_id.to_string()).collect();
                        chains.dedup();
                        let mut registries: Vec<String> = firsts.iter().filter_map(|(_, f)| f.registry.clone()).collect();
                        registries.sort();
                        registries.dedup();
                        if !chains.is_empty() {
                            rows.push((t(Key::BasisChain), Val::mono(chains.join("\n"))));
                        }
                        if !registries.is_empty() {
                            rows.push((t(Key::SetRegistry), Val::mono(registries.join("\n"))));
                        }
                        let placed: Vec<(String, String)> = firsts.iter().map(|(n, f)| (n.clone(), format!("{}\n{}", f.block_number, f.tx))).collect();
                        for (n, at) in &placed {
                            rows.push((n.as_str(), Val::mono(at.clone())));
                        }
                        match &x.filed {
                            Some(Ok(p)) => rows.push((t(Key::VerifyResultFile), Val::mono(p.clone()))),
                            Some(Err(e)) => rows.push((t(Key::VerifyResultFile), Val::Mark(Mark::Bad, e.clone()))),
                            None => {}
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
        if let Some(at) = add_network {
            self.add_stated_network(&at, now);
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

impl Win {
    /// A chain's name on the face: the name table's, else the one typed for that chain among the read-only
    /// networks, else "chain <id>".
    pub(super) fn chain_label(&self, chain: u64) -> String {
        let typed = self.shell.read_nets.as_ref().and_then(|ns| ns.iter().find(|n| n.chain_id == chain).and_then(|n| n.name.clone()));
        crate::readnets::chain_name(chain, typed.as_deref())
    }
}
