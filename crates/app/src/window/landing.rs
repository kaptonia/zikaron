//! The window's side of task landings. Every landed task is routed by the one registry ([`crate::landing`]):
//! on success it is toasted, kept quiet, or sent back to the place that started it; on failure it is toasted
//! or sent back. The wording of each result is built here. Nothing else in the window decides a landing.

use super::*;
use crate::landing::{self, Back, OnErr, OnOk, Then};

impl Win {
    /// The current chain reading: `Err` with the refusal when the last read failed (the chain kind keeps no
    /// reading across a failure, `Kept::Failed`), else the last reading, if any.
    pub(super) fn chain_read(&self) -> Result<Option<&Done>, &crate::fault::Fault> {
        self.shell.chain_reading()
    }

    /// Take this frame's landings and dispatch each according to its kind's registry row.
    pub(super) fn land(&mut self, ctx: &egui::Context, now: f64) {
        let landed = self.shell.drain_at(now);
        // File dialog answers go to the place that asked (`Back::Path`).
        self.paths_landed(ctx, &landed);
        // Follow-ups after a successful landing (`Then`): an anchoring that reached the node (included or receipted)
        // reads the chain again and records chain time; a new check closes the issuer-ledger field opened for the
        // last pass's gap.
        let then = |w: Then| landed.iter().any(|o| o.result.is_ok() && landing::of(o.kind).then == w);
        if then(Then::ReadChain) && landed.iter().any(|o| matches!(o.result, Ok(Done::Anchored { .. }))) && self.shell.anchor.is_some() && !self.shell.endpoints.is_empty() {
            self.auto(Action::ReadChain, now);
        }
        if then(Then::CloseIssuerField) {
            self.ux.u3.ck_source_open = None;
        }
        self.sync_landed(&landed, now);
        // A refused exit gate answers the place that pressed the export (a refusal from a gate started for an
        // earlier source belongs to that source).
        for o in &landed {
            if let (OnErr::Back(Back::Gate), false, Err(f)) = (landing::of(o.kind).err, o.stale, &o.result) {
                if let Some(site) = self.ux.gate_site.take() {
                    self.vault_back(site, Applied::Trouble(f.clone()), now);
                }
            }
        }
        // Kinds that answer through `said` are handled below, where that answer is taken.
        let claimed: Vec<_> = landed.into_iter().filter(|o| !landing::goes_back(o.kind, Back::Said)).filter(|o| self.ux.claim(o.kind)).collect();
        for o in claimed {
            let kind = o.kind;
            // A long key shows how the user's task landed (a check mark, or red and a shake).
            let good = match &o.result {
                Ok(d) => !done_is_bad(d),
                Err(_) => false,
            };
            // A passed exit gate does not say how the export went: the export runs where the gate landed and its
            // own answer sets the key (below, from `gate_said`). A success that no longer applies (a source left
            // behind, a reading dropped) shows no check either, since nothing was saved.
            if !matches!(o.result, Ok(Done::Submitted { .. }) | Ok(Done::GatePassed { .. })) && !(o.stale && o.result.is_ok()) {
                self.ux.landed.insert(kind, (good, now));
            }
            // Quiet kinds say nothing on success (their reading shows where it is read), and neither does a stale
            // success; only trouble is reported, by the shell.
            if o.result.is_ok() && (landing::of(kind).ok == OnOk::Quiet || o.stale) {
                continue;
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
                    (fill2(Key::SaidAudited, &label_human(&label), &entries.to_string()), if complete { Tone::Note } else { Tone::Bad })
                }
                Ok(Done::Ledger { rows, strays, .. }) => (fill2(Key::SaidLedger, &rows.len().to_string(), &strays.to_string()), if strays == 0 { Tone::Note } else { Tone::Bad }),
                Ok(Done::Depth { work, .. }) => (fill1(Key::SaidDepth, &self.work_seq_say(&work)), Tone::Note),
                // Files that were no longer the chosen records' originals when the kit was written were left out:
                // say how many.
                Ok(Done::Kit { path, left_out, unreadable, .. }) if !left_out.is_empty() || !unreadable.is_empty() => (
                    crate::lang::filln(Key::SaidKitLeftOut, &[&folder_of(&path), &left_out.len().to_string(), &unreadable.len().to_string()]),
                    Tone::Bad,
                ),
                Ok(Done::Kit { path, .. }) => (fill1(Key::SaidKit, &folder_of(&std::path::Path::new(&path).parent().map(|p| p.display().to_string()).unwrap_or_default())), Tone::Note),
                Ok(Done::Keystore(crate::task::Keystore::BackedUp { path, .. })) => (fill1(Key::SaidKeyBackedUp, &folder_of(&path)), Tone::Note),
                Ok(Done::BackupMade { path, .. }) => (fill1(Key::SaidBackupMade, &width::file_name(&path)), Tone::Note),
                // The backup's contents go on the confirmation card, not a toast.
                Ok(Done::BackupSeen { .. }) => continue,
                // The shell finishes passcode tasks; the answer is in `vault_said`, handled below.
                Ok(Done::Vault(_)) => continue,
                // File dialog answers already went to the place that asked (`paths_landed`).
                Ok(Done::Path(_)) => continue,
                // The command line was added to or removed from the terminal path; its row shows the current state.
                Ok(Done::CliPath(zikaron_os::cli_path::State::On)) => (t(Key::SaidCliPathOn).to_string(), Tone::Note),
                Ok(Done::CliPath(zikaron_os::cli_path::State::Off)) => (t(Key::SaidCliPathOff).to_string(), Tone::Note),
                Ok(Done::CliPath(_)) => continue,
                // These kinds were filtered out above (`said`).
                Ok(Done::Gas { .. }) | Ok(Done::Took { .. }) | Ok(Done::Hashed { .. }) | Ok(Done::Copied { .. }) => continue,
                // An export's gate passed: the shell ran the export; its answer is in `gate_said`, handled below.
                Ok(Done::GatePassed { .. }) => continue,
                // Attachment digests land silently; their row updates itself.
                Ok(Done::Vetted(_)) => continue,
                // The existing-anchor table and a read claim show their results on the card, not a toast.
                Ok(Done::KeyAnchors { .. }) | Ok(Done::Claim { .. }) => continue,
                Ok(Done::Adopt { proofs, .. }) => (
                    fill2(Key::SaidProofs, &proofs.len().to_string(), &proofs.iter().filter(|p| p.ok()).count().to_string()),
                    if proofs.iter().all(|p| p.ok()) { Tone::Note } else { Tone::Bad },
                ),
                // The new key's history is shown on its sheet, not a toast.
                Ok(Done::Sighting { .. }) => continue,
                // A read-only network's reading shows on its own row, not a toast.
                Ok(Done::NetRead { .. }) => continue,
                // The main network's chain settings checked on save: saved, or not saved because of the fingerprint
                // (a missing node is a named fault, reported with the faults). When nodes are saved after an
                // unchecked cell, the reading shows beside the cell, and a fingerprint other than the pinned build
                // is reported (the cell is kept).
                Ok(Done::BasisRead { registry, reading: crate::widex::Reading::Fingerprint, after_nodes: true, .. }) => (fill1(Key::SaidBasisFingerprintKept, &registry.hex()), Tone::Bad),
                Ok(Done::BasisRead { after_nodes: true, .. }) => continue,
                Ok(Done::BasisRead { chain, reading, .. }) if reading.admits() => (fill1(Key::SaidBasis, &chain.to_string()), Tone::Note),
                Ok(Done::BasisRead { registry, reading: crate::widex::Reading::Fingerprint, .. }) => (fill1(Key::SaidBasisFingerprint, &registry.hex()), Tone::Bad),
                Ok(Done::BasisRead { .. }) => continue,
                Ok(Done::Book { anchors, entries, .. }) => (fill2(Key::SaidBook, &anchors.to_string(), &entries.to_string()), if entries == 0 { Tone::Bad } else { Tone::Note }),
                Ok(Done::Grants { rows, .. }) => (fill1(Key::SaidGrants, &rows.len().to_string()), Tone::Note),
                Ok(Done::Diligence(r)) => (
                    fill2(Key::SaidDiligence, &if r.label.is_empty() { t(Key::OnlyAnchors).to_string() } else { label_human(&r.label) }, &r.anchors.to_string()),
                    if r.double_sold() { Tone::Bad } else { Tone::Note },
                ),
                Ok(Done::Verified(v)) => (fill1(Key::SaidVerifiedWork, &v.mismatches.len().to_string()), if v.mismatches.is_empty() { Tone::Note } else { Tone::Bad }),
                Ok(Done::Delivery(d)) => (
                    fill1(Key::SaidDelivery, t(if d.matched() { Key::DeliveryMatch } else { Key::DeliveryMismatch })),
                    if d.matched() { Tone::Note } else { Tone::Bad },
                ),
                Ok(Done::Reviewed { cards, .. }) => (
                    fill1(Key::SaidReviewed, &cards.len().to_string()),
                    if cards.iter().any(|x| x.verdict == zikaron_kit::tokens::CheckVerdict::Fail.as_str()) { Tone::Bad } else { Tone::Note },
                ),
                // The vault listing gets no toast: it is read on opening and after imports.
                Ok(Done::Held { .. }) => continue,
                Ok(Done::Published { read, .. }) => {
                    if read.complete() {
                        (format!("{} · {}", t(Key::PublishOk), fill1(Key::PublishOkSay, &read.total.to_string())), Tone::Note)
                    } else {
                        (format!("{} · {}", t(Key::PublishPartial), fill2(Key::PublishPartialSay, &read.missing.len().to_string(), &read.differ.len().to_string())), Tone::Bad)
                    }
                }
                Ok(Done::Badge(b)) => (fill2(Key::SaidBadge, &folder_of(&b.txt.parent().map(|p| p.display().to_string()).unwrap_or_default()), &b.hops.to_string()), Tone::Note),
                // A conflict is answered by its card on the home page, not a toast.
                Ok(Done::FetchConflict { .. }) => continue,
                // A tail check started by the app itself says nothing: a pass lifts the read-only bar, a gap
                // changes what the bar says.
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
                    fill1(Key::SaidChecked, verdict_human(&x.judged.verdict)),
                    if x.judged.verdict == zikaron_kit::tokens::CheckVerdict::Fail.as_str() { Tone::Bad } else { Tone::Note },
                ),
                // Broadcast landed: the shell starts the receipt wait. The user's task toasts when the receipt
                // lands, so its claim is restored.
                Ok(Done::Submitted { .. }) => {
                    self.ux.asked.push(crate::task::Kind::Anchor);
                    continue;
                }
                // "Sent, but failed on chain" is only for a receipt status other than 1.
                Ok(Done::Anchored { tx, state, .. }) if crate::action::receipt_failed(&state) => {
                    self.say_fault(&crate::fault::Fault::known(crate::fault::Known::SendFailed, tx), now);
                    continue;
                }
                // Voided: the shell already reported it (`BatchVoided`) and the entries are back in the queue.
                Ok(Done::Anchored { voided: true, .. }) => continue,
                // Included: "N anchored"; not yet receipted: "N submitted, waiting".
                Ok(Done::Anchored { sent, confirmed, .. }) => {
                    if confirmed {
                        (fill1(Key::SaidSent, &sent.to_string()), Tone::Note)
                    } else {
                        (fill1(Key::SaidSubmitted, &sent.to_string()), Tone::Note)
                    }
                }
                // Failed: the shell recorded it and `tell_faults` reports non-network faults (with details); network
                // faults are reported here, since the user started this task.
                Err(f) => {
                    if crate::watchx::is_network(&f) && f.then_key() != Some(Key::ExitRetryLater) {
                        self.say_fault(&f, now);
                    }
                    continue;
                }
            };
            // If the user has left the task's page, the toast offers "view" to go back there.
            let view = self.task_away(kind).then(|| vec![(t(Key::NavLook).to_string(), zikaron_ui::toast::Act::Tag(kind as u64))]).unwrap_or_default();
            let why = out.and_then(|x| self.landing_note(x)).unwrap_or_default();
            self.toasts.say_keys(said, &why, "", tone, now, view);
        }

        // Actions whose slow half ran in the background: each answer goes back to where it started.
        for k in crate::task::Kind::ALL.into_iter().filter(|k| landing::goes_back(*k, Back::Said)) {
            if let Some(a) = self.shell.said.remove(&k) {
                self.said_back(k, a, now);
            }
        }
        // A passcode task landed: the answer goes back to whoever started it.
        if let Some(a) = self.shell.vault_said.take() {
            let site = self.ux.vault_site.take();
            let a = self.told(a, None, now);
            if let Some(site) = site {
                self.vault_back(site, a, now);
            }
        }
        // An export whose gate passed ran where the gate landed: report its answer here, and let the long key show
        // how the export went (a gate re-read for another home starts over).
        if let Some(a) = self.shell.gate_said.take() {
            if !matches!(a, Applied::Started(_)) {
                self.ux.landed.insert(crate::task::Kind::Gate, (!matches!(a, Applied::Trouble(_) | Applied::Refused(_)), now));
            }
            let a = self.told(a, None, now);
            // The export's answer goes back to the place that pressed it (a re-read gate keeps waiting).
            if !matches!(a, Applied::Started(_)) {
                if let Some(site) = self.ux.gate_site.take() {
                    self.vault_back(site, a, now);
                }
            }
        }
    }
}
