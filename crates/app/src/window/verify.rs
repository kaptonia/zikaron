use super::*;
use zikaron_ui::inform;

pub(super) fn mismatch_said(m: &crate::verifyx::Mismatch<'_>) -> String {
    use crate::verifyx::Mismatch;
    match m {
        Mismatch::Kit { subject, .. } => fill1(Key::U4KitBad, &width::file_name(subject)),
        Mismatch::Invalid { path, .. } => fill1(Key::U4MismatchInvalid, path),
        Mismatch::Rejected(r) => format!("{} \u{b7} {}", width::file_name(&r.file), r.human()),
        Mismatch::Unanchored(id) => fill1(Key::U4MismatchUnanchored, id),
        Mismatch::WorkAbsent => t(Key::U4MismatchWorkAbsent).to_string(),
    }
}

fn kit_said(x: &crate::verifyx::Verified) -> (Mark, String) {
    match &x.kit {
        Some(k) if k.ok => (Mark::Ok, t(Key::U4KitGood).to_string()),
        Some(k) => (Mark::Bad, fill1(Key::U4KitBad, &width::file_name(&k.subject))),
        None => (Mark::Todo, t(Key::U4NotAKit).to_string()),
    }
}

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
    pub(super) fn verify_input(&self) -> String {
        format!("{}\n{}", self.typed.vf_path.trim(), self.typed.vf_work.trim())
    }

    pub(super) fn verify_page(&mut self, ui: &mut egui::Ui, tab: u8, now: f64) {
        use crate::nav::tab as T;
        let list = crate::nav::tabs(crate::nav::View::Verify, self.shell.settings.role);
        let labels: Vec<seg::Cell> = list.iter().map(|x| seg::Cell::from(t(tab_key(crate::nav::View::Verify, *x)))).collect();
        let cur = list.iter().position(|x| *x == tab).unwrap_or(0);
        let picked = stagger(ui, 0, |ui| ui.horizontal(|ui| seg::seg(ui, "verify-tabs", &labels, cur)).inner);
        ui.add_space(tk::S4 - ui.spacing().item_spacing.y);
        if let Some(i) = picked {
            self.seg_tab(Place::View(crate::nav::View::Verify, list[i]), now);
        }
        tab_body(ui, "verify", u64::from(tab), |ui| match tab {
            T::VERIFY_WORK => self.kit_verify(ui, now),
            T::VERIFY_OTHERS => self.others_page(ui, now),
            _ => self.check_page(ui, now),
        })
    }

    fn kit_verify(&mut self, ui: &mut egui::Ui, now: f64) {
        let for_this = self.ux.u4.verify_asked.as_deref().is_none_or(|p| p == self.verify_input());
        let v = self.shell.verified.clone().filter(|_| for_this);
        let busy = self.shell.tasks.in_flight(crate::task::Kind::Verify);
        let mut go = false;
        let mut add_network: Option<crate::kitsindex::AnchoredOn> = None;
        if !busy && std::mem::take(&mut self.ux.u4.verify_again) {
            go = true;
        }
        if self.ux.u4.verify_autorun && !self.typed.vf_path.trim().is_empty() {
            self.ux.u4.verify_autorun = false;
            go = true;
        }
        let d = stagger(ui, 1, |ui| drop::zone(ui, "verify-drop", Some(Glyph::Inbox), &[t(Key::KitvDrop), t(Key::DropClickAny)], None, 150.0, drop::Shape::Column, true));
        if let Some(p) = self.drop_or_pick(ui.ctx(), "verify-record", &d, crate::platform::Pick::FileOrFolder, now) {
            self.typed.vf_path = p;
            go = true;
        }
        let phase = self.phase_of(crate::task::Kind::Verify);
        stagger(ui, 2, |ui| {
            let work = self.typed.vf_work.trim().to_string();
            let summary = if work.is_empty() { t(Key::Nothing).to_string() } else { head_tail(&work) };
            card::form(ui, |ui, f| {
                let _ = f.fold(ui, "verify-more", t(Key::U3MoreOptions), &summary, |ui, f| {
                    f.panel(ui, |ui| {
                        fields::stack(ui, |ui, st| {
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
        let refused = for_this
            .then(|| self.ux.u4.verify_refused.clone().or_else(|| self.shell.failed.get(&crate::task::Kind::Verify).cloned()))
            .flatten();
        if busy {
            card::card(ui, |ui| states::skeleton_lines(ui, &[0.3, 0.6, 0.5, 0.7]));
        } else if let (None, Some(f)) = (v.as_ref(), refused) {
            card::form(ui, |ui, form| form.summary(ui, Glyph::No, card::Tint::Bad, f.human(), None, &f.next(), |_| ()));
            let shown = f.shown();
            if !shown.is_empty() {
                details_card(ui, "verify-err", &[(t(Key::U3RawError), Val::mono(shown))]);
            }
        } else if let Some(x) = v.as_ref() {
            let remote = match &x.source {
                crate::verifyx::Source::Remote(b) => Some(b.as_str().to_string()),
                _ => None,
            };
            let fetched = remote.as_ref().map(|b| fill2(Key::CheckFromAddress, b, &x.kit.as_ref().map(|k| k.entries + k.files + k.proofs + 1).unwrap_or(0).to_string())).unwrap_or_default();
            let kit = kit_said(x);
            let review = match &x.review {
                Ok(_) => (Mark::Ok, String::new()),
                Err(_) if x.not_added.is_some() => (Mark::Warn, t(Key::VerifyNotAdded).to_string()),
                Err(_) => (Mark::Todo, t(Key::U4AnchorsUnread).to_string()),
            };
            let (title, title_mark) = verified_head(x);
            let worst = [title_mark, review.0].into_iter().filter(|m| *m != Mark::Todo).max_by_key(|m| match m {
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
            let local = matches!(x.source, crate::verifyx::Source::Kit(_) | crate::verifyx::Source::File(_) | crate::verifyx::Source::Addressed(_));
            let take_why = extract_why(&x.takes);
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
                    if !x.mismatches.is_empty() && kit.0 == Mark::Bad {
                        let _ = f.info(ui, t(Key::PageKit), card::Show::Mark(kit.0, &kit.1));
                    }
                    if review.0 != Mark::Ok {
                        let _ = f.info(ui, t(Key::NavAnchoring), card::Show::Mark(review.0, &review.1));
                    }
                    if let Some(g) = &x.grant {
                        match (&g.package, g.verdict.as_deref()) {
                            (Err((token, _)), _) => {
                                let _ = f.info(ui, t(Key::PageCheck), card::Show::Mark(Mark::Bad, token));
                            }
                            (Ok(()), verdict) => {
                                let (m, words) = match crate::checkx::tone(verdict.unwrap_or_default()) {
                                    _ if g.undecided => (Mark::Warn, t(Key::CheckUndecided)),
                                    crate::checkx::Tone::Green => (Mark::Ok, t(Key::BadgeLive)),
                                    crate::checkx::Tone::Red => (Mark::Bad, t(Key::OsInvalid)),
                                    crate::checkx::Tone::Amber | crate::checkx::Tone::Grey => (Mark::Warn, t(Key::OsOpen)),
                                };
                                let said = if g.failed.is_empty() { words.to_string() } else { format!("{words} \u{b7} {}", g.failed.join(" ")) };
                                let _ = f.info(ui, t(Key::PageCheck), card::Show::Mark(m, &said));
                                let several = g.lights.len() > 1;
                                for (i, hop) in g.lights.iter().enumerate() {
                                    if several {
                                        let _ = f.info(ui, &fill1(Key::CheckHop, &(i + 1).to_string()), card::Show::Text(""));
                                    }
                                    for (tok, state) in hop {
                                        let gap = crate::checkx::notices_gap(&g.notes, i, tok, state).map(|x| gap_words(&x)).unwrap_or_default();
                                        let _ = f.info(ui, t(u3_check_name(tok)), card::Show::Mark(state_mark(state), &gap));
                                    }
                                }
                                if g.behind {
                                    f.note(ui, t(Key::LedgerBehindBook));
                                }
                                if g.undecided {
                                    f.note(ui, t(Key::VerifyGrantAuditThere));
                                }
                                let (m, files) = if g.delivers { (Mark::Ok, t(Key::GrantFilesMatch)) } else { (Mark::Todo, t(Key::GrantNoOriginal)) };
                                let _ = f.info(ui, t(Key::GrantFilesRow), card::Show::Mark(m, files));
                            }
                        }
                    }
                    if !x.mismatches.is_empty() {
                        f.note(ui, t(Key::U4MismatchNext));
                    }
                    if let Some(at) = &x.not_added {
                        let said = format!("{} \u{b7} {}", t(Key::VerifyAuthorSays), t(Key::VerifyAddNetwork));
                        if f.act(ui, &self.chain_label(at.chain_id), &said, key::Key::new(t(Key::ReadNetAdd), Role::Secondary)).clicked() {
                            add_network = Some(at.clone());
                        }
                    }
                    for m in &x.missed {
                        let _ = f.info(ui, &m.name, card::Show::Mark(Mark::Warn, t(m.key())));
                    }
                });
            });
            if take {
                self.extract_open();
            }
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
                                match &r.deleted {
                                    Some(d) => {
                                        let by = fill1(Key::U4DeletedBy, &d.seq.to_string());
                                        let sub = if sub.is_empty() { by } else { format!("{by} \u{b7} {sub}") };
                                        let _ = f.line(ui, None, &name, &sub, false, false, c(C::Ink3), |ui| inform::mark_words(ui, Mark::Todo, t(Key::V2Deleted)));
                                    }
                                    None => {
                                        let _ = f.line(ui, None, &name, &sub, false, false, c(C::Ink), |ui| inform::mark_words(ui, m, chain));
                                    }
                                }
                                for n in &r.notes {
                                    let _ = f.line(ui, None, &n.note, &format!("{} \u{b7} #{}", t(Key::KindAnnotation), n.seq), false, false, c(C::Ink2), |_| ());
                                }
                            }
                        });
                    });
                });
            }
            let what = if remote.is_some() { x.path.clone() } else { format!("{} \u{b7} {}", width::file_name(&x.path), t(source_key(&x.source))) };
            let what = if remote.is_some() { Val::mono(what) } else { Val::text(what) };
            let mut rows: Vec<(&str, Val)> = vec![(t(Key::U4Source), what), (t(Key::LedgerEntries), Val::figure(x.entries.to_string())), (t(Key::U4Rejected), Val::figure(x.rejected.len().to_string()))];
            rows.push((t(Key::PageKit), Val::text(kit.1.clone())));
            if x.review.is_err() {
                rows.push((t(Key::NavAnchoring), Val::text(review.1.clone())));
            }
            if x.review.is_ok() && !x.read.is_empty() {
                rows.push((t(Key::NetsRead), Val::text(x.read.iter().map(|c| self.chain_label(*c)).collect::<Vec<_>>().join(t(Key::ListJoin)))));
            }
            if x.review.is_ok() && !x.basis.is_empty() {
                let lines = x.basis.iter().map(|b| format!("{} \u{b7} {} \u{b7} {}\u{2013}{}", self.chain_label(b.chain), b.registries.join(" "), b.from_block, b.to_block)).collect::<Vec<_>>().join("\n");
                rows.push((t(Key::BasisRead), Val::mono(lines)));
            }
            let notices = x.grant.iter().flat_map(|g| g.notes.per.iter().flatten()).map(crate::noticex::basis_line).collect::<Vec<_>>();
            if !notices.is_empty() {
                rows.push((t(Key::NoticesBasis), Val::mono(notices.join("\n"))));
            }
            let hashes = x.records.iter().map(|r| r.id.clone()).collect::<Vec<_>>().join("\n");
            if !hashes.is_empty() {
                rows.push((t(Key::U4RecordHash), Val::mono(hashes)));
            }
            if let Some(k) = &x.kit {
                rows.push((t(Key::KitId), Val::mono(k.kit_id.clone())));
                if !k.verdict.is_empty() {
                    rows.push((t(Key::U3Verdict), Val::text(fill1(Key::U4KitBad, &width::file_name(&k.subject)))));
                }
            }
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
            let contracts = |revocation: bool| -> Vec<String> {
                let mut at: Vec<String> = firsts.iter().filter_map(|(_, f, _)| f.registry.clone().filter(|r| crate::auditx::revocation_contract(f.chain_id, r) == revocation)).collect();
                at.sort();
                at.dedup();
                at
            };
            if !chains.is_empty() {
                rows.push((t(Key::BasisChain), Val::figure(chains.join("\n"))));
            }
            for (revocation, name) in [(false, Key::SetRegistry), (true, Key::SetRevocation)] {
                let at = contracts(revocation);
                if !at.is_empty() {
                    rows.push((t(name), Val::mono(at.join("\n"))));
                }
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
            let said: Vec<String> = crate::verifyx::mismatch_list(x.kit.as_ref(), &x.rejected, &x.review, x.work_found).iter().map(mismatch_said).collect();
            if !said.is_empty() {
                rows.push((t(Key::DeliveryMismatch), Val::text(said.join("\n"))));
            }
            match &x.filed {
                Some(Ok(p)) => rows.push((t(Key::VerifyResultFile), Val::mono(p.clone()))),
                Some(Err(e)) => rows.push((t(Key::VerifyResultFile), Val::mono(e.clone()))),
                None => {}
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
                self.ux.u4.verify_asked = Some(self.verify_input());
                self.ux.u4.verify_refused = match self.act(a, now) {
                    Applied::Trouble(f) => Some(f),
                    _ => None,
                };
            }
        }
    }
}

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
    pub(super) fn chain_label(&self, chain: u64) -> String {
        let typed = self.shell.read_nets.as_ref().and_then(|ns| ns.iter().find(|n| n.chain_id == chain).and_then(|n| n.name.clone()));
        crate::readnets::chain_name(chain, typed.as_deref())
    }
}

pub(super) fn extract_why(takes: &Option<Result<zikaron_glue::extract::Plan, zikaron_glue::extract::Refused>>) -> Option<&'static str> {
    use zikaron_glue::extract::Refused;
    match takes {
        Some(Ok(p)) if !p.take.is_empty() => None,
        Some(Ok(_)) | Some(Err(Refused::Nothing)) => Some(t(Key::ExtractWhyNone)),
        Some(Err(r @ (Refused::Name(_) | Refused::Clash(_) | Refused::Path(_)))) => r.words().map(|(zh, en)| match crate::lang::lang() {
            crate::lang::Lang::Zh => zh,
            crate::lang::Lang::En => en,
        }),
        Some(Err(_)) | None => Some(t(Key::ExtractWhyKit)),
    }
}

#[cfg(test)]
mod extract_why_tests {
    use super::*;
    use zikaron_glue::extract::{words, Refused};

    #[test]
    fn a_package_that_passed_is_not_said_to_have_failed() {
        let why = |r: Refused| extract_why(&Some(Err(r)));
        let said = |w: (&'static str, &'static str)| [Some(w.0), Some(w.1)];
        assert!(said(words::NAME).contains(&why(Refused::Name(vec!["a:b".into()]))));
        assert!(said(words::CLASH).contains(&why(Refused::Clash(vec!["data".into()]))));
        assert!(said(words::PATH).contains(&why(Refused::Path("A".into()))));
        for failed in [why(Refused::Kit { verdict: "E_KIT_FILE".into(), subject: None }), extract_why(&None)] {
            assert!(failed.is_some() && ![words::NAME, words::CLASH, words::PATH].into_iter().any(|w| said(w).contains(&failed)));
        }
    }
}

impl Win {
    pub(super) fn extract_open(&mut self) {
        let Some(x) = self.shell.verified.clone() else { return };
        let names = x.records.iter().filter_map(|r| r.name.clone().filter(|n| !n.trim().is_empty()).map(|n| (r.id.clone(), n))).collect();
        self.extract_open_from(ExtractFrom { path: x.path.clone(), takes: x.takes.clone(), names });
    }

    pub(super) fn extract_open_from(&mut self, from: ExtractFrom) {
        if self.typed.extract_to.trim().is_empty() {
            self.typed.extract_to = self.shell.documents.clone();
        }
        self.ux.extract_from = Some(from);
        self.ux.extract_sheet = true;
    }

    pub(super) fn extract_sheet(&mut self, ctx: &egui::Context, now: f64) {
        if !self.ux.extract_sheet {
            return;
        }
        let Some(x) = self.ux.extract_from.clone() else {
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
        let name_of = |id: &str| x.names.iter().find(|(r, _)| r == id).map(|(_, n)| n.clone()).unwrap_or_else(|| t(Key::UnnamedRecord).to_string());
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
                        let sub = if o.names_hold || o.strict { "" } else { t(Key::ExtractPackageName) };
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
                let why = if plan.take.is_empty() { t(Key::ExtractNone).to_string() } else { still_needs(&[(to.trim().is_empty(), Key::ExtractTo)]) };
                go = key::show(ui, key::Key::new(t(Key::ExtractGo), Role::Primary).enabled(ready).why(&why)).clicked();
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
