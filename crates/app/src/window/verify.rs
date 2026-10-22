//! The verify view: its tabs (verify a grant, verify records, others' records) under the page's title, and the
//! record verification.

use super::*;
use zikaron_ui::inform;

/// What the kit verified is: whole and signed, damaged (naming it), or no kit at all.
fn kit_said(x: &crate::verifyx::Verified) -> (Mark, String) {
    match &x.kit {
        Some(k) if k.ok => (Mark::Ok, t(Key::U4KitGood).to_string()),
        Some(k) => (Mark::Bad, fill1(Key::U4KitBad, &width::file_name(&k.subject))),
        None => (Mark::Todo, t(Key::U4NotAKit).to_string()),
    }
}

/// A record verification's head: how many do not match; else what the kit is; else all match. The one judgement
/// the verify page's title and the home page's recent line both read.
pub(super) fn verified_head(x: &crate::verifyx::Verified) -> (String, Mark) {
    if !x.mismatches.is_empty() {
        (fill1(Key::U4Mismatches, &x.mismatches.len().to_string()), Mark::Bad)
    } else if x.kit.is_some() {
        let (m, s) = kit_said(x);
        (s, m)
    } else {
        (t(Key::U4NoMismatch).to_string(), Mark::Ok)
    }
}

impl Win {
    pub(super) fn verify_page(&mut self, ui: &mut egui::Ui, tab: u8, now: f64) {
        use crate::nav::tab as T;
        let list = crate::nav::tabs(crate::nav::View::Verify, self.shell.settings.role);
        let labels: Vec<seg::Cell> = list.iter().map(|x| seg::Cell::from(t(tab_key(crate::nav::View::Verify, *x)))).collect();
        let cur = list.iter().position(|x| *x == tab).unwrap_or(0);
        let picked = stagger(ui, 0, |ui| ui.horizontal(|ui| seg::seg(ui, "verify-tabs", &labels, cur)).inner);
        // A segment above a block stands 16 from it, closer than two blocks.
        ui.add_space(tk::S4 - ui.spacing().item_spacing.y);
        if let Some(i) = picked {
            self.set_tab(Place::View(crate::nav::View::Verify, list[i]), now);
        }
        match tab {
            T::VERIFY_WORK => self.kit_verify(ui, now),
            T::VERIFY_OTHERS => self.others_page(ui, now),
            _ => self.check_page(ui, now),
        }
    }

    /// Verify records: drop a record kit or record file (the content place and record hash are under advanced
    /// options, with their key); the result is an overview (what the kit is, three figures, what is wrong), the
    /// records as rows, the raw values in one details list at the bottom.
    fn kit_verify(&mut self, ui: &mut egui::Ui, now: f64) {
        let v = self.shell.verified.clone();
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Verify);
        let mut go = false;
        let mut add_network: Option<crate::kitsindex::AnchoredOn> = None;
        // A file changed while a pass ran is verified again once that pass ends.
        if !busy && std::mem::take(&mut self.ux.u4.verify_again) {
            go = true;
        }
        // A kit dropped on home or on the window starts as soon as the page is up.
        if self.ux.u4.verify_autorun && !self.typed.vf_path.trim().is_empty() {
            self.ux.u4.verify_autorun = false;
            go = true;
        }
        // The zone stays as it is: what was verified is named under details.
        let d = stagger(ui, 1, |ui| drop::zone(ui, "verify-drop", Some(Glyph::Inbox), &[t(Key::KitvDrop), t(Key::DropClickAny)], None, 150.0, drop::Shape::Column, true));
        if let Some(p) = self.drop_or_pick(ui.ctx(), "verify-record", &d, crate::platform::Pick::FileOrFolder, now) {
            self.typed.vf_path = p;
            go = true;
        }
        let phase = self.phase_of(crate::task::Kind::Verify);
        stagger(ui, 2, |ui| {
            // The advanced options' summary: the record hash given, else none.
            let work = self.typed.vf_work.trim().to_string();
            let summary = if work.is_empty() { t(Key::Nothing).to_string() } else { head_tail(&work) };
            card::form(ui, |ui, f| {
                let _ = f.fold(ui, "verify-more", t(Key::U3MoreOptions), &summary, |ui, f| {
                    f.panel(ui, |ui| {
                        fields::stack(ui, |ui, st| {
                            // A publish address cannot be dropped, so it is typed here; its shape is checked under it.
                            let bad = crate::fetchx::is_address(&self.typed.vf_path).then(|| crate::fetchx::base_of(&self.typed.vf_path).err()).flatten();
                            let said = bad.as_ref().map(|f| (f.human(), true));
                            let look = fields::Look { mono: true, bad: bad.is_some(), ..Default::default() };
                            st.field(ui, t(Key::RecordContent), None, said, |ui, w| fields::line(ui, &mut self.typed.vf_path, t(Key::BytesWhereHint), w, look));
                            st.field(ui, t(Key::U4RecordHash), Some(t(Key::U3Optional)), None, |ui, w| fields::line(ui, &mut self.typed.vf_work, "0x\u{2026}", w, fields::Look { mono: true, ..Default::default() }));
                            st.foot(ui, |ui| {
                                let _page = page::Page::new();
                                if key::show(ui, key::Key::new(t(Key::DoVerifyWork), Role::Primary).enabled(!self.typed.vf_path.trim().is_empty()).why(t(Key::KitvDrop)).phase(phase)).clicked() {
                                    go = true;
                                }
                            });
                        });
                    });
                });
            });
            self.stage_line(ui, crate::task::Kind::Verify);
        });
        if busy {
            card::card(ui, |ui| states::skeleton_lines(ui, &[0.3, 0.6, 0.5, 0.7]));
        } else if let Some(x) = v.as_ref() {
            // What was verified, fetched from a publish address: the address, with "fetch again".
            let remote = match &x.source {
                crate::verifyx::Source::Remote(b) => Some(b.as_str().to_string()),
                _ => None,
            };
            let fetched = remote.as_ref().map(|b| fill2(Key::CheckFromAddress, b, &x.kit.as_ref().map(|k| k.entries + k.files + k.proofs + 1).unwrap_or(0).to_string())).unwrap_or_default();
            let kit = kit_said(x);
            let review = match &x.review {
                Ok(a) if a.unanchored.is_empty() && a.unread.is_empty() && a.anchored > 0 => (Mark::Ok, fill1(Key::U4AnchorsMatch, &a.anchored.to_string())),
                // A network left out this pass: what no read chain reaches is not called unanchored.
                Ok(a) if a.unanchored.is_empty() && !a.unread.is_empty() => (Mark::Todo, t(Key::U4AnchorsUnread).to_string()),
                Ok(a) => (Mark::Warn, fill1(Key::U4AnchorsBehind, &a.unanchored.len().to_string())),
                Err(_) if x.not_added.is_some() => (Mark::Warn, t(Key::VerifyNotAdded).to_string()),
                Err(_) => (Mark::Todo, t(Key::U4AnchorsUnread).to_string()),
            };
            let label = match &x.review {
                // A kit carries only the chosen entries and the ledger's spine, so gaps in seq are normal
                // ("part of the ledger"), not a warning.
                Ok(a) if x.kit.is_some() && a.label == zikaron::tokens::Label::Gaps.as_str() => (Mark::Ok, t(Key::U4IssuerPartial).to_string()),
                Ok(a) => (if a.label == zikaron::tokens::Label::Complete.as_str() { Mark::Ok } else { Mark::Warn }, label_human(&a.label)),
                Err(_) => (Mark::Todo, t(Key::U4IssuerAuditUnread).to_string()),
            };
            let (title, title_mark) = verified_head(x);
            let worst = [title_mark, review.0, label.0].into_iter().filter(|m| *m != Mark::Todo).max_by_key(|m| match m {
                Mark::Bad => 2,
                Mark::Warn => 1,
                _ => 0,
            });
            let (glyph, tint) = match worst {
                Some(Mark::Bad) => (Glyph::No, card::Tint::Bad),
                Some(Mark::Warn) => (Glyph::Warn, card::Tint::Warn),
                Some(_) => (Glyph::Ok, card::Tint::Ok),
                None => (Glyph::Info, card::Tint::Blue),
            };
            // The chain readings: when the records were first anchored, how deep, how continuous.
            let three = x.depth.clone().map(crate::depthx::three).map(|three| {
                // A chain not read this pass (none read, or a network left out): a measure that reads "none" or
                // falls short may have its anchor on the unread chain, so it says the chain was not read. Values
                // from chains that were read show as they are.
                use crate::depthx::Said;
                let (first, deepest, span) = crate::depthx::said(&three, x.review.is_err() || !x.missed.is_empty());
                let first = match first {
                    Said::Is(at) => (crate::when::when(at), true),
                    Said::Nothing => (t(Key::None_).to_string(), false),
                    Said::ChainUnread => (t(Key::U4ChainUnread).to_string(), false),
                };
                let deepest = match deepest {
                    Said::Is(n) => (n.to_string(), true),
                    _ => (t(Key::U4ChainUnread).to_string(), false),
                };
                let span = match span {
                    Said::Is((a, s)) => (format!("{a} / {s}"), true),
                    _ => (t(Key::U4ChainUnread).to_string(), false),
                };
                [first, deepest, span]
            });
            // A package read from this machine (a folder or one file) gives its originals out: the key is live when it passed
            // and holds an original that matches its record, and says why not otherwise. One fetched from a publish
            // address is fetched again instead.
            let local = matches!(x.source, crate::verifyx::Source::Kit(_) | crate::verifyx::Source::File(_) | crate::verifyx::Source::Addressed(_));
            let take_why = extract_why(x);
            let mut take = false;
            stagger(ui, 3, |ui| {
                card::form(ui, |ui, f| {
                    let pressed = f.summary(ui, glyph, tint, &title, None, &fetched, |ui| {
                        if remote.is_some() {
                            key::key(ui, t(Key::CheckRefetch), Role::Secondary, true).clicked()
                        } else if local {
                            key::show(ui, key::Key::new(t(Key::DoExtract), Role::Secondary).enabled(take_why.is_none()).why(take_why.unwrap_or(""))).clicked()
                        } else {
                            false
                        }
                    });
                    if pressed && remote.is_some() {
                        go = true;
                    } else if pressed {
                        take = true;
                    }
                    if let Some(three) = &three {
                        // The first anchoring is a date: monospace, as dates are on the other pages.
                        let first = if three[0].1 { card::Fact::Mono(&three[0].0) } else { fact_of(&three[0].0, true) };
                        f.facts(ui, &[(t(Key::U3FirstAnchored), first), (t(Key::U3Deepest), fact_of(&three[1].0, !three[1].1)), (t(Key::U3Continuity), fact_of(&three[2].0, !three[2].1))]);
                    }
                    // What the head does not already say: the kit when mismatches lead, the chain and the issuer
                    // ledger when they fall short.
                    if !x.mismatches.is_empty() && kit.0 == Mark::Bad {
                        let _ = f.info(ui, t(Key::PageKit), card::Show::Mark(kit.0, &kit.1));
                    }
                    if review.0 != Mark::Ok {
                        let _ = f.info(ui, t(Key::NavAnchoring), card::Show::Mark(review.0, &review.1));
                    }
                    if label.0 != Mark::Ok {
                        let _ = f.info(ui, t(Key::CheckIssuerLedger), card::Show::Mark(label.0, &label.1));
                    }
                    if !x.mismatches.is_empty() {
                        f.note(ui, t(Key::U4MismatchNext));
                    }
                    // The kit names a network not yet added: no chain was read; one key adds it.
                    if let Some(at) = &x.not_added {
                        let said = format!("{} \u{b7} {}", t(Key::VerifyAuthorSays), t(Key::VerifyAddNetwork));
                        if f.act(ui, &self.chain_label(at.chain_id), &said, key::Key::new(t(Key::ReadNetAdd), Role::Secondary)).clicked() {
                            add_network = Some(at.clone());
                        }
                    }
                    // The networks not read this pass, each with why.
                    for m in &x.missed {
                        let _ = f.info(ui, &m.name, card::Show::Mark(Mark::Warn, t(m.reading.key())));
                    }
                });
            });
            if take {
                self.extract_open();
            }
            // Record by record: its name, its state on chain; the original in the kit when it does not match.
            if !x.records.is_empty() {
                stagger(ui, 4, |ui| {
                    card::section(ui, t(Key::U4RecordsTitle), "", |ui| {
                        card::form(ui, |ui, f| {
                            for r in &x.records {
                                use crate::verifyx::{OnChain, Original};
                                let (chain, m) = match r.chain {
                                    OnChain::Anchored { .. } => (t(Key::LampAnchored), Mark::Ok),
                                    OnChain::NotAnchored => (t(Key::V2StateLanded), Mark::Warn),
                                    OnChain::Unread => (t(Key::U4ChainUnread), Mark::Todo),
                                };
                                let (sub, m) = match r.original {
                                    Original::Match => (String::new(), m),
                                    Original::Mismatch => (format!("{} {}", t(Key::U4RecordOriginal), t(Key::U4OriginalMismatch)), Mark::Bad),
                                    Original::Missing => (format!("{} {}", t(Key::U4RecordOriginal), t(Key::U4OriginalMissing)), m),
                                };
                                let name = r.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| t(Key::UnnamedRecord).to_string());
                                let _ = f.line(ui, None, &name, &sub, false, false, c(C::Ink), |ui| inform::mark_words(ui, m, chain));
                            }
                        });
                    });
                });
            }
            // Details: the raw values, every label a name.
            let what = if remote.is_some() { x.path.clone() } else { format!("{} \u{b7} {}", width::file_name(&x.path), t(source_key(&x.source))) };
            // A remote source is its path or address (taken elsewhere: a copy key); a local one, its file's name and
            // the words for where it came from.
            let what = if remote.is_some() { Val::mono(what) } else { Val::text(what) };
            let mut rows: Vec<(&str, Val)> = vec![(t(Key::U4Source), what), (t(Key::LedgerEntries), Val::figure(x.entries.to_string())), (t(Key::U4Rejected), Val::figure(x.rejected.len().to_string()))];
            rows.push((t(Key::PageKit), Val::text(kit.1.clone())));
            rows.push((t(Key::NavAnchoring), Val::text(review.1.clone())));
            rows.push((t(Key::CheckIssuerLedger), Val::text(label.1.clone())));
            if x.review.is_ok() && !x.read.is_empty() {
                rows.push((t(Key::NetsRead), Val::text(x.read.iter().map(|c| self.chain_label(*c)).collect::<Vec<_>>().join(t(Key::ListJoin)))));
            }
            let hashes = x.records.iter().map(|r| r.id.clone()).collect::<Vec<_>>().join("\n");
            if !hashes.is_empty() {
                rows.push((t(Key::U4RecordHash), Val::mono(hashes)));
            }
            if let Some(k) = &x.kit {
                rows.push((t(Key::KitId), Val::mono(k.kit_id.clone())));
                rows.push((t(Key::U3Verdict), Val::figure(k.verdict.clone())));
            }
            // Where each record's first anchor is: chain, registry, and per record its time, block and full
            // transaction.
            let firsts: Vec<(String, &crate::auditx::FirstAnchor, Option<u64>)> = x
                .records
                .iter()
                .filter_map(|r| {
                    let at = match r.chain {
                        crate::verifyx::OnChain::Anchored { first_at } => Some(first_at),
                        _ => None,
                    };
                    r.first.as_ref().map(|f| (r.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| t(Key::UnnamedRecord).to_string()), f, at))
                })
                .collect();
            let mut chains: Vec<String> = firsts.iter().map(|(_, f, _)| f.chain_id.to_string()).collect();
            chains.dedup();
            let mut registries: Vec<String> = firsts.iter().filter_map(|(_, f, _)| f.registry.clone()).collect();
            registries.sort();
            registries.dedup();
            if !chains.is_empty() {
                rows.push((t(Key::BasisChain), Val::figure(chains.join("\n"))));
            }
            if !registries.is_empty() {
                rows.push((t(Key::SetRegistry), Val::mono(registries.join("\n"))));
            }
            if !firsts.is_empty() {
                let placed = firsts
                    .iter()
                    .map(|(n, f, at)| match at {
                        Some(at) => format!("{n} \u{b7} {} \u{b7} {} \u{b7} {}", crate::when::when(*at), f.block_number, f.tx),
                        None => format!("{n} \u{b7} {} \u{b7} {}", f.block_number, f.tx),
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                rows.push((t(Key::FirstAnchorWhere), Val::mono(placed)));
            }
            if !x.mismatches.is_empty() {
                rows.push((t(Key::DeliveryMismatch), Val::mono(x.mismatches.join("\n"))));
            }
            match &x.filed {
                Some(Ok(p)) => rows.push((t(Key::VerifyResultFile), Val::mono(p.clone()))),
                Some(Err(e)) => rows.push((t(Key::VerifyResultFile), Val::mono(e.clone()))),
                None => {}
            }
            match &x.review {
                Ok(a) => rows.push((t(Key::AuditLabel), Val::figure(a.label.clone()))),
                Err(said) => rows.push((t(Key::U3RawError), Val::mono(said.clone()))),
            }
            stagger(ui, 5, |ui| details_card(ui, "verify-evidence", &rows));
        }
        if let Some(at) = add_network {
            self.add_stated_network(&at, now);
        }
        if go {
            if busy {
                self.ux.u4.verify_again = true;
            } else {
                let a = Action::VerifyWork { path: self.typed.vf_path.clone(), work: self.typed.vf_work.clone(), then_to: String::new() };
                self.act(a, now);
            }
        }
    }
}

/// What a record verification read, in words: a whole kit, a single file, a grant file, a publish address.
fn source_key(s: &crate::verifyx::Source) -> Key {
    match s {
        crate::verifyx::Source::Kit(_) => Key::SourceKit,
        crate::verifyx::Source::Bytes(_) => Key::SourceBytes,
        crate::verifyx::Source::File(_) => Key::SourceGrantFile,
        crate::verifyx::Source::Addressed(_) => Key::SourceAddressed,
        crate::verifyx::Source::Remote(_) => Key::SetPublish,
    }
}

impl Win {
    /// A chain's display name: from the name table, else the name typed for that chain among the read-only
    /// networks, else "chain <id>".
    pub(super) fn chain_label(&self, chain: u64) -> String {
        let typed = self.shell.read_nets.as_ref().and_then(|ns| ns.iter().find(|n| n.chain_id == chain).and_then(|n| n.name.clone()));
        crate::readnets::chain_name(chain, typed.as_deref())
    }
}

/// Why the originals of what was verified cannot be taken out, or `None` when they can: the package did not pass
/// (or was not read again to see), or it holds no original that matches its record.
fn extract_why(x: &crate::verifyx::Verified) -> Option<&'static str> {
    match &x.takes {
        Some(Ok(p)) if !p.take.is_empty() => None,
        Some(Ok(_)) | Some(Err(zikaron_glue::extract::Refused::Nothing)) => Some(t(Key::ExtractWhyNone)),
        Some(Err(_)) | None => Some(t(Key::ExtractWhyKit)),
    }
}

impl Win {
    /// Open "take out originals": the folder is the one used last in this run, else the documents folder.
    pub(super) fn extract_open(&mut self) {
        if self.typed.extract_to.trim().is_empty() {
            self.typed.extract_to = self.shell.documents.clone();
        }
        self.ux.extract_sheet = true;
    }

    /// "Take out originals": the folder in one row (the last one used, else the documents folder),
    /// "choose folder" at the bottom left, cancel and "take out" at the bottom right. Above the keys, how many
    /// originals go and each one's name (with the package's name when its own cannot name a file), and each record
    /// whose original does not match or is missing, named, not taken. Writing happens only on "take out", in the
    /// background (`Action::ExtractOriginals`); a toast says how many were taken.
    pub(super) fn extract_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.extract_sheet {
            return;
        }
        let Some(x) = self.shell.verified.clone() else {
            self.ux.extract_sheet = false;
            return;
        };
        let plan = match &x.takes {
            Some(Ok(p)) => p.clone(),
            _ => zikaron_glue::extract::Plan { take: Vec::new(), left: Vec::new() },
        };
        let to = self.typed.extract_to.clone();
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Extract);
        let ready = !plan.take.is_empty() && !to.trim().is_empty() && !busy;
        let name_of = |id: &str| x.records.iter().find(|r| r.id == id).and_then(|r| r.name.clone()).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| t(Key::UnnamedRecord).to_string());
        let (mut go, mut close, mut picked) = (false, false, None);
        let out = sheet::show(
            ctx,
            sheet::Spec::new("extract-originals", tk::SHEET_W),
            self,
            |ui, _me| {
                sheet::title(ui, t(Key::DoExtract), "");
                card::form(ui, |ui, f| f.folder(ui, t(Key::ExtractTo), &to, t(Key::PickNone)));
                card::form(ui, |ui, f| {
                    let _ = f.info(ui, t(Key::ExtractWill), card::Show::Text(&fill1(Key::ExtractCount, &plan.take.len().to_string())));
                    for o in &plan.take {
                        let glyph = if o.kept == zikaron_glue::extract::Kept::Dir { Glyph::Folder } else { Glyph::File };
                        let sub = if o.names_hold { "" } else { t(Key::ExtractPackageName) };
                        let _ = f.line(ui, Some(card::Lead::Glyph(glyph)), o.landing_name(), sub, false, false, c(C::Ink), |_| ());
                    }
                    for l in &plan.left {
                        let name = l.name.clone().unwrap_or_else(|| name_of(&l.entry_id));
                        let why = match l.why {
                            zikaron_glue::extract::Why::Mismatch => t(Key::ExtractLeftMismatch),
                            zikaron_glue::extract::Why::Missing => t(Key::ExtractLeftMissing),
                        };
                        let _ = f.info(ui, &name, card::Show::Mark(Mark::Warn, why));
                    }
                });
            },
            |ui, _me| {
                go = key::key(ui, t(Key::ExtractGo), Role::Primary, ready).clicked();
                close = key::key(ui, t(Key::CfBack), Role::Secondary, true).clicked();
                picked = foot_pick(ui, "extract-originals", crate::platform::Pick::Folder);
            },
        );
        if let Some(p) = picked {
            self.typed.extract_to = p;
        }
        if close || out.esc {
            self.ux.extract_sheet = false;
            return;
        }
        if go && ready {
            let a = Action::ExtractOriginals { kit: x.path.clone(), to: self.typed.extract_to.trim().to_string() };
            self.act(a, now);
            self.ux.extract_sheet = false;
        }
    }
}
