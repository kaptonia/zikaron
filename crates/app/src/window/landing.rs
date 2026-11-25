use super::*;
use crate::landing::{self, Back, OnErr, OnOk, Then};

impl Win {
    pub(super) fn chain_read(&self) -> Result<Option<&Done>, &crate::fault::Fault> {
        self.shell.chain_reading()
    }

    pub(super) fn rotate_comes_back(&self, stage: Option<&crate::rotatex::Stage>) -> bool {
        self.ux.rot_walked
            && stage.is_some_and(|s| s.walking())
            && !self.ux.rotate_put_away
            && !self.any_sheet()
            && self.ux.wizard.is_none()
    }

    pub(super) fn land(&mut self, ctx: &egui::Context, now: f64) {
        let landed = self.shell.drain_at(now);
        if self.shell.followed && self.shell.rows_current() && std::mem::take(&mut self.shell.followed) && self.shell.settings.auto_anchor && self.shell.sendable_now() > 0 && self.ux.u3.confirm.is_none() {
            let n = self.shell.sendable_now();
            self.u3_open_confirm(U3Confirm::Send { count: n });
        }
        if self.shell.upgrade_read && self.shell.upgrade.as_ref().is_some_and(crate::upgradex::Stage::pending) && self.shell.chain_set() && !self.ux.upgrade_put_away && !self.any_sheet() && self.ux.wizard.is_none() {
            self.upgrade_sheet_open();
            self.ux.upgrade_came_up = true;
        }
        if self.ux.upgrade_open && self.ux.upgrade_came_up && !self.ux.upgrade_auto && self.shell.upgrade_read && self.shell.upgrade == Some(crate::upgradex::Stage::Done) {
            self.ux.upgrade_open = false;
            self.ux.upgrade_came_up = false;
        }
        self.upgrade_walk(now);
        if self.rotate_comes_back(crate::action::rotation_stage(&self.shell).as_ref()) {
            self.rotate_sheet_open(now);
        }
        self.paths_landed(ctx, &landed);
        let then = |w: Then| landed.iter().any(|o| o.result.is_ok() && landing::of(o.kind).then == w);
        if then(Then::ReadChain) && landed.iter().any(|o| matches!(o.result, Ok(Done::Anchored { .. }))) && self.shell.anchor.is_some() && !self.shell.endpoints.is_empty() {
            self.auto(Action::ReadChain, now);
        }
        let changing = matches!(crate::action::rotation_stage(&self.shell), Some(crate::rotatex::Stage::Anchoring { .. } | crate::rotatex::Stage::Accede { .. } | crate::rotatex::Stage::Unread { written: true, .. }));
        if changing && landed.iter().any(|o| matches!(o.result, Ok(Done::Anchored { .. }))) {
            self.auto(Action::Rotate { step: crate::action::RotateStep::Look }, now);
        }
        if then(Then::CloseIssuerField) {
            self.ux.u3.ck_source_open = None;
        }
        self.sync_landed(&landed, now);
        for o in &landed {
            if let (OnErr::Back(Back::Gate), false, Err(f)) = (landing::of(o.kind).err, o.stale, &o.result) {
                self.ux.u4.grant_in_gate = false;
                if let Some(site) = self.ux.gate_site.take() {
                    self.vault_back(site, Applied::Trouble(f.clone()), now);
                }
            }
        }
        if landed.iter().any(|o| !o.stale && matches!(o.result, Ok(Done::Rotation { withdrawn: true, .. }))) {
            self.toasts.say_keys(t(Key::SaidDeclarationWithdrawn).to_string(), "", "", Tone::Note, now, Vec::new());
        }
        let claimed: Vec<_> = landed.into_iter().filter(|o| !landing::goes_back(o.kind, Back::Said)).filter(|o| self.ux.claim(o.kind)).collect();
        for o in claimed {
            let kind = o.kind;
            if o.renoded {
                continue;
            }
            let good = match &o.result {
                Ok(d) => !done_is_bad(d),
                Err(_) => false,
            };
            if !matches!(o.result, Ok(Done::Submitted { .. }) | Ok(Done::GatePassed { .. }) | Ok(Done::Issued { .. }) | Ok(Done::Withdrew { .. }) | Ok(Done::FaceCleared { .. })) && !(o.stale && o.result.is_ok()) {
                self.ux.landed.insert(kind, (good, now));
            }
            if o.result.is_ok() && (landing::of(kind).ok == OnOk::Quiet || o.stale) {
                continue;
            }
            if matches!(o.result, Ok(Done::Kit { .. })) {
                self.ux.u3.kit_addressed = false;
            }
            let out = match &o.result {
                Ok(Done::Kit { .. }) => Some(Out::Kit),
                Ok(Done::Badge(_)) => Some(Out::Badge),
                _ => None,
            };
            let (said, tone) = match o.result {
                Ok(Done::Check(r)) => (fill2(Key::SaidSelfCheck, &r.found().to_string(), &r.marks.to_string()), Tone::Note),
                Ok(Done::Archive { bytes, items, .. }) => (fill2(Key::SaidMeasured, &size_say(bytes), &items.to_string()), Tone::Note),
                Ok(Done::Chain { gas_wei, .. }) => match gas_wei {
                    Some(w) => (fill1(Key::SaidChain, &eth_held(w)), Tone::Note),
                    None => (t(Key::SaidChainNone).to_string(), Tone::Bad),
                },
                Ok(Done::Reconciled { label, complete, .. }) => (fill1(Key::SaidReconciled, &label_human(&label)), if complete { Tone::Note } else { Tone::Bad }),
                Ok(Done::Audited { label, complete, entries, .. }) => {
                    let (said, fine) = self.own_audit_said(&label);
                    (fill2(Key::SaidAudited, &said, &entries.to_string()), if complete || fine { Tone::Note } else { Tone::Bad })
                }
                Ok(Done::Ledger { rows, strays, .. }) => (fill2(Key::SaidLedger, &rows.len().to_string(), &strays.to_string()), if strays == 0 { Tone::Note } else { Tone::Bad }),
                Ok(Done::Kit { path, left_out, unreadable, withdrawn, .. }) if !withdrawn.is_empty() => (
                    crate::lang::filln(Key::SaidKitLeftOutDeleted, &[&folder_of(&path), &withdrawn.len().to_string(), &left_out.len().to_string(), &unreadable.len().to_string()]),
                    Tone::Bad,
                ),
                Ok(Done::Kit { path, left_out, unreadable, .. }) if !left_out.is_empty() || !unreadable.is_empty() => (
                    crate::lang::filln(Key::SaidKitLeftOut, &[&folder_of(&path), &left_out.len().to_string(), &unreadable.len().to_string()]),
                    Tone::Bad,
                ),
                Ok(Done::Kit { facts, .. }) if facts.one_time.is_some() => (t(Key::SaidKitAddressed).to_string(), Tone::Note),
                Ok(Done::Kit { path, .. }) => (fill1(Key::SaidKit, &folder_of(&path)), Tone::Note),
                Ok(Done::Keystore(crate::task::Keystore::BackedUp { path, .. })) => (fill1(Key::SaidKeyBackedUp, &width::file_name(&path)), Tone::Note),
                Ok(Done::BackupMade { path, .. }) => (fill1(Key::SaidBackupMade, &width::file_name(&path)), Tone::Note),
                Ok(Done::BackupSeen { .. }) => continue,
                Ok(Done::Vault(_)) => continue,
                Ok(Done::Path(_)) => continue,
                Ok(Done::CliPath(zikaron_os::cli_path::State::On)) => (t(Key::SaidCliPathOn).to_string(), Tone::Note),
                Ok(Done::CliPath(zikaron_os::cli_path::State::Off)) => (t(Key::SaidCliPathOff).to_string(), Tone::Note),
                Ok(Done::CliPath(_)) => continue,
                Ok(Done::Gas { .. }) | Ok(Done::Took { .. }) | Ok(Done::Hashed { .. }) | Ok(Done::Copied { .. }) | Ok(Done::Seated(_)) => continue,
                Ok(Done::Extracted { landed, .. }) => (fill1(Key::SaidExtracted, &landed.names.len().to_string()), Tone::Note),
                Ok(Done::Cleaned { done: false, .. }) => continue,
                Ok(Done::Cleaned { plan, .. }) => (crate::lang::filln(Key::SaidCleaned, &[&plan.remove.len().to_string(), &plan.keep.len().to_string()]), Tone::Note),
                Ok(Done::GatePassed { .. }) | Ok(Done::Issued { .. }) | Ok(Done::Withdrew { .. }) | Ok(Done::FaceCleared { .. }) => continue,
                Ok(Done::Vetted(_)) | Ok(Done::Peeked(_)) | Ok(Done::Looked(_)) | Ok(Done::Regained { .. }) | Ok(Done::Kept(_)) => continue,
                Ok(Done::Rotation { .. }) | Ok(Done::RotateVetted { .. }) | Ok(Done::UpgradeCost { .. }) => continue,
                Ok(Done::ChainTime(_)) => continue,
                Ok(Done::Persona(..)) | Ok(Done::FaceBooks(_)) | Ok(Done::RerootFee(_)) | Ok(Done::FaceJoined(_)) | Ok(Done::Behind { .. }) => continue,
                Ok(Done::NetRead { .. }) => continue,
                Ok(Done::BasisRead { registry, reading: crate::widex::Reading::Fingerprint, after_nodes: true, .. }) => (fill1(Key::SaidBasisFingerprintKept, &registry.hex()), Tone::Bad),
                Ok(Done::BasisRead { after_nodes: true, .. }) => continue,
                Ok(Done::BasisRead { chain, reading, .. }) if reading.admits() => (fill1(Key::SaidBasis, &chain.to_string()), Tone::Note),
                Ok(Done::BasisRead { registry, reading: crate::widex::Reading::Fingerprint, .. }) => (fill1(Key::SaidBasisFingerprint, &registry.hex()), Tone::Bad),
                Ok(Done::BasisRead { .. }) => continue,
                Ok(Done::Book { anchors, entries, .. }) => (fill2(Key::SaidBook, &anchors.to_string(), &entries.to_string()), if entries == 0 { Tone::Bad } else { Tone::Note }),
                Ok(Done::Grants { rows, .. }) => (fill1(Key::SaidGrants, &rows.len().to_string()), Tone::Note),
                Ok(Done::Diligence(r)) => (diligence_said(&r), if r.double_sold() { Tone::Bad } else { Tone::Note }),
                Ok(Done::Verified(v)) if v.mismatches.is_empty() && v.review.is_err() => (t(Key::SaidVerifiedUncompared).to_string(), Tone::Note),
                Ok(Done::Verified(v)) => (fill1(Key::SaidVerifiedWork, &v.mismatches.len().to_string()), if v.mismatches.is_empty() { Tone::Note } else { Tone::Bad }),
                Ok(Done::Delivery(d)) => (
                    fill1(Key::SaidDelivery, t(if d.matched() { Key::DeliveryMatch } else { Key::DeliveryMismatch })),
                    if d.matched() { Tone::Note } else { Tone::Bad },
                ),
                Ok(Done::Reviewed { cards, .. }) => (
                    fill1(Key::SaidReviewed, &cards.len().to_string()),
                    if cards.iter().any(|x| x.verdict == zikaron_kit::tokens::CheckVerdict::Fail.as_str()) { Tone::Bad } else { Tone::Note },
                ),
                Ok(Done::Held { .. }) => continue,
                Ok(Done::Published { read, .. }) => {
                    if read.complete() {
                        (format!("{} · {}", t(Key::PublishOk), fill1(Key::PublishOkSay, &read.total.to_string())), Tone::Note)
                    } else {
                        (format!("{} · {}", t(Key::PublishPartial), fill2(Key::PublishPartialSay, &read.missing.len().to_string(), &read.differ.len().to_string())), Tone::Bad)
                    }
                }
                Ok(Done::Badge(b)) => (fill2(Key::SaidBadge, &folder_of(&b.txt.display().to_string()), &b.hops.to_string()), Tone::Note),
                Ok(Done::FetchConflict { .. }) => continue,
                Ok(Done::TailChecked { .. }) => continue,
                Ok(Done::FetchedAside { fetched, .. }) => {
                    let said = match *fetched {
                        Done::Fetched { entries, tail: crate::restorex::Tail::Pass { anchors }, .. } => fill2(Key::SaidFetched, &entries.to_string(), &anchors.to_string()),
                        Done::Fetched { entries, tail: crate::restorex::Tail::NewerElsewhere { missing, .. }, .. } => fill2(Key::SaidFetchedNewer, &entries.to_string(), &missing.to_string()),
                        _ => String::new(),
                    };
                    let view = vec![(t(Key::NavLook).to_string(), zikaron_ui::toast::Act::Tag(OLD_DATA_TAG))];
                    self.toasts.say_keys(said, t(Key::KeptAsOld), "", Tone::Note, now, view);
                    continue;
                }
                Ok(Done::Fetched { entries, tail, .. }) => match tail {
                    crate::restorex::Tail::Pass { anchors } => (fill2(Key::SaidFetched, &entries.to_string(), &anchors.to_string()), Tone::Note),
                    crate::restorex::Tail::NewerElsewhere { missing, .. } => (fill2(Key::SaidFetchedNewer, &entries.to_string(), &missing.to_string()), Tone::Bad),
                },
                Ok(Done::Checked(x)) => (
                    fill1(Key::SaidChecked, if x.undecided.is_empty() { verdict_human(&x.judged.verdict) } else { t(Key::CheckUndecided) }),
                    if x.judged.verdict == zikaron_kit::tokens::CheckVerdict::Fail.as_str() { Tone::Bad } else { Tone::Note },
                ),
                Ok(Done::Submitted { .. }) => {
                    self.ux.asked.push(crate::task::Kind::Anchor);
                    continue;
                }
                Ok(Done::Anchored { tx, state, .. }) if crate::action::receipt_failed(&state) => {
                    self.say_fault(&crate::fault::Fault::known(crate::fault::Known::SendFailed, tx), now);
                    continue;
                }
                Ok(Done::Anchored { voided: true, .. }) => continue,
                Ok(Done::Anchored { sent, confirmed, .. }) => {
                    if confirmed {
                        (fill1(Key::SaidSent, &sent.to_string()), Tone::Note)
                    } else {
                        (fill1(Key::SaidSubmitted, &sent.to_string()), Tone::Note)
                    }
                }
                Err(f) => {
                    if crate::watchx::is_network(&f) && f.then_key() != Some(Key::ExitRetryLater) {
                        self.say_fault(&f, now);
                    }
                    continue;
                }
            };
            let view = self.task_away(kind).then(|| vec![(t(Key::NavLook).to_string(), zikaron_ui::toast::Act::Tag(kind as u64))]).unwrap_or_default();
            let why = out.and_then(|x| self.landing_note(x)).unwrap_or_default();
            self.toasts.say_keys(said, &why, "", tone, now, view);
        }

        for k in crate::task::Kind::ALL.into_iter().filter(|k| landing::goes_back(*k, Back::Said)) {
            if let Some(a) = self.shell.said.remove(&k) {
                self.said_back(k, a, now);
            }
        }
        if let Some(a) = self.shell.vault_said.take() {
            let site = self.ux.vault_site.take();
            self.card_follows = site == Some(VaultSite::Persona);
            let a = self.told(a, None, now);
            self.card_follows = false;
            if let Some(site) = site {
                self.vault_back(site, a, now);
            }
        }
        if let Some(a) = self.shell.gate_said.take() {
            if !matches!(a, Applied::Started(_)) {
                self.ux.landed.insert(crate::task::Kind::Gate, (!matches!(a, Applied::Trouble(_) | Applied::Refused(_)), now));
            }
            self.card_follows = self.ux.gate_site == Some(VaultSite::Persona);
            let a = self.told(a, None, now);
            self.card_follows = false;
            if !matches!(a, Applied::Started(_)) {
                if let Some(site) = self.ux.gate_site.take() {
                    self.vault_back(site, a, now);
                }
            }
        }
    }
}

pub(super) fn diligence_said(r: &crate::diligx::Read) -> String {
    said_of_reading(&r.label, r.anchors, r.entries, r.grants.len())
}

fn said_of_reading(label: &str, anchors: usize, entries: usize, grants: usize) -> String {
    if label.is_empty() {
        return fill2(Key::SaidDiligenceRead, &entries.to_string(), &grants.to_string());
    }
    fill2(Key::SaidDiligence, &label_human(label), &anchors.to_string())
}

#[cfg(test)]
mod said_tests {
    use super::*;

    #[test]
    fn a_first_level_read_says_what_it_read_and_an_audit_its_conclusion() {
        crate::lang::set(crate::lang::Lang::En);
        let said = said_of_reading("", 0, 2, 1);
        assert_eq!(said, fill2(Key::SaidDiligenceRead, "2", "1"));
        assert!(!said.contains(t(Key::OnlyAnchors)), "{said}");
        assert_eq!(said_of_reading(zikaron::v2::tokens::Label::Complete.as_str(), 3, 2, 1), fill2(Key::SaidDiligence, &label_human(zikaron::v2::tokens::Label::Complete.as_str()), "3"));
    }
}
