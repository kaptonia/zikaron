use super::*;
use super::works::{anchor_rows, detail_acts, DetailActs, DATE_W};

impl Win {
    /// Read the table when it has not been read at the current generation, so opening a page shows the ledger
    /// without the user pressing anything; a stale table stays shown until the new reading lands.
    ///
    /// It starts a background pass (no disk I/O in the frame). A task of this kind starts itself only once:
    /// the condition checks whether `Tasks` recorded an attempt (success or failure), so a failed read does
    /// not start another pass every frame.
    pub(super) fn ensure_rows(&mut self, now: f64) {
        if self.shell.rows_current() || self.shell.home.is_none() || self.shell.tasks.attempted(crate::task::Kind::Ledger) {
            return;
        }
        self.auto(Action::ReadLedger, now);
    }

    /// The key scan's reading, returned only when it is about the address now in the form.
    pub(super) fn sighting_now(&self) -> Option<(String, usize, usize)> {
        let want = self.typed.sc_to.trim();
        self.shell.sighting.as_ref().filter(|(who, _, _)| !want.is_empty() && who.eq_ignore_ascii_case(want)).cloned()
    }

    /// Read the register once, like [`Self::ensure_rows`].
    pub(super) fn ensure_grants(&mut self, now: f64) {
        if self.shell.grants_current() || self.shell.home.is_none() || self.shell.tasks.attempted(crate::task::Kind::Grants) {
            return;
        }
        self.auto(Action::ReadGrants, now);
    }

    /// The chain's current time, taken only from the chain pass's reading.
    pub(super) fn chain_now(&self) -> Option<u64> {
        self.shell.chain_now()
    }

    /// The ledger's toolbar: the filter and "more" (import existing records, change key or hand over, add a
    /// note, sign a claim for someone).
    pub(super) fn ledger_acts(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        let items = [menu::row(t(Key::U3AdoptTitle)), menu::row(t(Key::TagHandover)), menu::row(t(Key::U3AnnotateGo)), menu::row(t(Key::U3AttestTitle))];
        if let Some(i) = menu::menu_key(ui, "ledger-more", t(Key::NavMore), false, 190.0, &items) {
            self.u3_form_open([U3Form::Adopt, U3Form::Succeed, U3Form::Annotate, U3Form::Attest][i]);
        }
        // "Unconfirmed": every entry the chain has not confirmed yet (`Lamp::confirmed`), a wider set than the
        // queue the Records page counts as "Pending".
        let cells = [seg::Cell::from(t(Key::U3FilterAll)), seg::Cell::from(t(Key::LedgerUnconfirmed)), seg::Cell::from(t(Key::KindGrant))];
        // The list changes in place: rows that stay slide, the others open or close where they are
        // (`table::table`); the page does not enter again.
        if let Some(i) = seg::seg(ui, "ledger-filter", &cells, self.ux.u3.led_filter) {
            self.ux.u3.led_filter = i;
        }
    }

    /// The ledger page: the overview card (with the ledger check folding open in it), search, and every entry.
    pub(super) fn ledger_page(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        self.ensure_grants(now);
        let rows: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
        stagger(ui, 0, |ui| self.audit_strip(ui, now));
        let read = self.shell.rows.is_some();
        let reading = crate::retractx::read(&rows);
        let mut query = self.ux.search.get("ledger").cloned().unwrap_or_default();
        let mut range = self.ux.range.get("ledger").cloned().unwrap_or_default();
        let today = (self.shell.clock)();
        stagger(ui, 1, |ui| search_row(ui, "ledger", &mut query, t(Key::SearchLedger), &mut range, today));
        // Newest on top (the table is read newest first).
        let hide = self.shell.settings.hide_local_deletions;
        let shown: Vec<&crate::ledgerx::Row> = rows
            .iter()
            .filter(|r| !(hide && r.lamp.local()))
            .filter(|r| match self.ux.u3.led_filter {
                // Entries the last pass recorded as anchored are not listed as waiting.
                1 => !r.lamp.confirmed(),
                2 => r.kind == zikaron::tokens::EntryType::Grant,
                _ => true,
            })
            .filter(|row| {
                let (tag, summary, _) = row_face(&rows, &reading, row);
                matches(&query, &[&format!("#{}", row.seq), tag, &summary, &row.id])
            })
            .filter(|row| crate::when::within(row.anchored_at, &range.0, &range.1))
            .collect();
        let mut open: Option<String> = None;
        // The recorder's seat with no ledger here yet: the empty list says so and offers "create the ledger", named
        // as the wizard's step it opens, the same key as on the sublicense page.
        let set_up_here = read && !self.shell.rooted && self.shell.settings.role == crate::roles::Role::Author;
        let mut set_up = false;
        stagger(ui, 2, |ui| {
            let create = || key::Key::new(t(Key::WizGenesisTitle), Role::Primary);
            // What an empty list says: one judgement for the table and the "create the ledger" row.
            let empty = if !read {
                t(Key::WbNotRead)
            } else if rows.is_empty() {
                t(Key::WbRecentNone)
            } else if !query.trim().is_empty() || !range.0.is_empty() || !range.1.is_empty() {
                t(Key::SearchNone)
            } else {
                t(Key::U3NoneOfThisKind)
            };
            if set_up_here && shown.is_empty() {
                card::form(ui, |ui, f| set_up = f.act(ui, empty, "", create()).clicked());
                return;
            }
            let cols = [table::SEQ, table::TYPE, table::col("", table::Col::Fr(1.0)), table::col("", table::Col::Px(DATE_W)), table::MARK];
            let trs: Vec<table::Row> = shown
                .iter()
                .map(|r| {
                    let (tag, summary, struck) = row_face(&rows, &reading, r);
                    table::Row {
                        cells: vec![table::Cell::Seq(r.seq), table::Cell::Tag(tag.to_string()), table::Cell::Text(summary), table::Cell::Mono(first_anchor_say(r.anchored_at)), table::Cell::Mark(lamp_mark(r.lamp))],
                        click: true,
                        gone: struck,
                        on: false,
                    }
                })
                .collect();
            if let Some(i) = table::table(ui, "ledger", &cols, false, &trs, empty).clicked {
                open = shown.get(i).map(|r| r.id.clone());
            }
            if set_up_here {
                ui.add_space(tk::CARD_GAP);
                card::form(ui, |ui, f| set_up = f.act(ui, t(Key::AuRoot), "", create()).clicked());
            }
        });
        if set_up {
            self.wizard_open(crate::nav::Step::Genesis);
        }
        self.ux.search.insert("ledger", query);
        self.ux.range.insert("ledger", range);
        if let Some(id) = open {
            self.push(Route::Entry(id), now);
        }
    }

    /// The overview card: the ledger's state after its name and the entry count under it, with "put all on
    /// chain" when entries can be sent; a root not confirmed on chain; the last batch's resend when one is offered
    /// (or that resends are spent); and the ledger check folding open in the card (the audit page and home's tile
    /// open it): its items two to a row, then its details (conclusion, head, root, basis, entries, when it ran).
    fn audit_strip(&mut self, ui: &mut egui::Ui, now: f64) {
        let (m, state) = match self.ledger_state() {
            Some(true) => (Mark::Ok, t(Key::V2Normal)),
            Some(false) => (Mark::Bad, t(Key::V2Abnormal)),
            // With no network configured, say why nothing was checked.
            None if self.shell.settings.chain_id.is_none() || self.shell.settings.registry.is_none() || self.shell.endpoints.is_empty() => (Mark::Warn, t(Key::U3AuditChipNoNetwork)),
            None => (Mark::Todo, t(Key::U3AuditChipNever)),
        };
        let total = self.shell.rows.as_ref().map(|(r, _)| fill1(Key::U3EntriesTotal, &r.len().to_string())).unwrap_or_default();
        let root_anchored = self.shell.rows.as_ref().and_then(|(r, _)| crate::ledgerx::root_anchored(r));
        // "Put all on chain" only when entries can be sent (the same reading as the export page's "now").
        let sendable = !self.unanchored_now().is_empty();
        // A batch sent but not included by the end of its wait: offer a resend with higher fees when the current
        // price is above its cap; after three resends, say so.
        let stuck = self.shell.stuck.clone();
        let bump = stuck.as_ref().filter(|s| matches!(s.offer, crate::task::Offer::Resend { .. } | crate::task::Offer::Unheld { .. })).and_then(|s| s.txs.last().cloned());
        let spent = stuck.as_ref().is_some_and(|s| s.offer == crate::task::Offer::Spent);
        // Held by no node and its nonce unused: say so, and the key resends it at the current price.
        let unheld = stuck.as_ref().is_some_and(|s| matches!(s.offer, crate::task::Offer::Unheld { .. }));
        let audit = self.shell.audit.clone();
        let summary = audit.as_ref().map(|a| label_human(&a.label)).unwrap_or_else(|| t(Key::U3AuditChipNever).to_string());
        let check = audit.as_ref().map(|a| (self.audit_body(a), self.audit_details(ui, a)));
        let (mut send_all, mut bump_open, mut copied) = (false, false, None);
        card::form(ui, |ui, f| {
            send_all = f.summary(ui, Glyph::Queue, card::Tint::Blue, t(Key::WbAudit), Some((m, state)), &total, |ui| sendable && key::key(ui, t(Key::U3SendAll), Role::Secondary, true).clicked());
            // Root not confirmed on chain, read from the genesis entry's state.
            if root_anchored == Some(false) {
                let _ = f.info(ui, t(Key::AuRoot), card::Show::Mark(Mark::Bad, t(Key::U3RootUnanchored)));
            }
            if bump.is_some() {
                let k = key::Key::new(t(if unheld { Key::U3ResendKey } else { Key::U3BumpKey }), Role::Secondary);
                bump_open = f.act(ui, t(Key::U3LastSend), if unheld { t(Key::U3Unheld) } else { "" }, k).clicked();
            } else if spent {
                let _ = f.info(ui, t(Key::U3LastSend), card::Show::Text(&fill1(Key::U3BumpSpent, &crate::queue::RESENDS_MAX.to_string())));
            }
            // The page's own flag (set by the audit page and by home's tile) holds the fold open or shut.
            copied = f
                .fold_held(ui, "ledger-check", t(Key::PageAudit), &summary, &mut self.ux.u3.audit_open, |ui, f| match &check {
                    Some((rows, (details, widest))) => {
                        f.free(ui, |ui| states::checks(ui, rows, true));
                        f.detail_rows(ui, "ledger-check-details", details, *widest)
                    }
                    None => {
                        f.note(ui, t(Key::U3AuditNeverHint));
                        None
                    }
                })
                .flatten();
        });
        tell_copied(ui.ctx(), copied);
        if let (true, Some(tx)) = (bump_open, bump) {
            self.u3_open_confirm(U3Confirm::Bump { tx });
        }
        if send_all {
            self.send_all_unanchored(now);
        }
    }

    /// The entries that can be put on chain now, each with whether it still has to be queued first.
    fn unanchored_now(&self) -> Vec<(String, bool)> {
        self.shell.rows.as_ref().map(|(r, _)| crate::kitx::unanchored(&r.iter().collect::<Vec<_>>()).1).unwrap_or_default()
    }

    /// "Put all on chain": queue the entries not yet queued, then the confirmation counts the sendable ones.
    pub(super) fn send_all_unanchored(&mut self, now: f64) {
        for (id, fresh) in self.unanchored_now() {
            if fresh {
                self.act(Action::QueueEntry { id }, now);
            }
        }
        let count = self.shell.queue.sendable();
        if count > 0 {
            self.u3_open_confirm(U3Confirm::Send { count });
        }
    }

    /// The ledger check's details, each under its name in words: the conclusion, the ledger's head and root
    /// entries, the chain basis and the entry count the report gives, and when the check ran; the label column as
    /// wide as the widest label in either language.
    fn audit_details(&self, ui: &egui::Ui, a: &crate::shell::AuditRead) -> (Vec<(&'static str, card::Detail)>, f32) {
        use zikaron::tokens::Key as T;
        let items = crate::auditx::items(&a.report);
        let said = |tok: T| items.iter().find(|i| i.key == tok.as_str()).map(|i| i.said.clone()).filter(|x| !x.is_empty());
        let rows = self.shell.rows.as_ref().map(|(r, _)| r.as_slice()).unwrap_or(&[]);
        let mut out: Vec<(&'static str, card::Detail)> = vec![(t(Key::AuditLabel), card::Detail::Text(label_human(&a.label)))];
        if let Some(head) = rows.first() {
            out.push((t(Key::AuHead), card::Detail::Mono(head.id.clone())));
        }
        if let Some(root) = said(T::Root).or_else(|| rows.last().map(|r| r.id.clone())) {
            out.push((t(Key::AuRootEntry), card::Detail::Mono(root)));
        }
        if let Some(basis) = said(T::Basis) {
            out.push((t(Key::AuBasis), card::Detail::Mono(basis)));
        }
        out.push((t(Key::LedgerEntries), card::Detail::Text(said(T::Entries).unwrap_or_else(|| a.entries.to_string()))));
        // When it ran, read off the interface clock against this machine's clock (display only).
        let ago = (self.ux.now - a.at).max(0.0) as u64;
        out.push((t(Key::AuLastRun), card::Detail::Text(crate::when::when((self.shell.clock)().saturating_sub(ago)))));
        let widest = out
            .iter()
            .flat_map(|(k, _)| [Some(*k), crate::lang::twin(k)])
            .flatten()
            .map(|k| paint::galley(ui, k, Type::Note, c(C::Ink)).size().x)
            .fold(0.0, f32::max);
        (out, widest)
    }

    /// The ledger check: the core's report item by item in plain words, each with its mark and count.
    fn audit_body(&self, a: &crate::shell::AuditRead) -> Vec<(Mark, String, Option<String>)> {
        use zikaron::tokens::Key as T;
        let items = crate::auditx::items(&a.report);
        let convention = self.shell.rows.as_ref().map(|(r, _)| crate::retractx::read(r).count).unwrap_or(0);
        // Whether an entry still needs anchoring is judged as the anchor queue and the alerts judge it
        // (`Lamp::local`): a local deletion pair stays local by intent, so of the report's unanchored entries those
        // are counted on a grey line of their own and never as owed (with the table not read yet, the report's
        // own count stands).
        let unanchored: Vec<String> = crate::auditx::rows_of(&a.report, T::Unanchored).iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        let local: Option<usize> = self.shell.rows.as_ref().map(|(r, _)| unanchored.iter().filter(|id| r.iter().any(|x| x.lamp.local() && x.id.eq_ignore_ascii_case(id))).count());
        let lists: [(T, Key, bool); 12] = [
            (T::Anchored, Key::LampAnchored, true),
            (T::Unanchored, Key::U3AuUnanchored, false),
            (T::Missing, Key::U3AuMissing, false),
            (T::Findings, Key::U3AuFindings, false),
            (T::Excluded, Key::U3AuExcluded, false),
            (T::AdoptionUnproven, Key::U3AuAdoptionUnproven, false),
            (T::UnknownType, Key::U3AuUnknownType, false),
            (T::Malformed, Key::U3AuMalformed, false),
            (T::Unavailable, Key::U3AuUnavailable, false),
            (T::Unproven, Key::U3AuUnproven, false),
            (T::Void, Key::U3AuVoid, false),
            (T::Discarded, Key::U3AuDiscarded, false),
        ];
        let mut rows: Vec<(Mark, String, Option<String>)> = lists
            .iter()
            .map(|(tok, k, good)| {
                let n = items.iter().find(|i| i.key == tok.as_str()).and_then(|i| i.count);
                let n = match (tok, n, local) {
                    (T::Unanchored, Some(all), Some(l)) => Some(all.saturating_sub(l)),
                    _ => n,
                };
                // This desk's own deletions are recognized by count and are not a fault; only extra ones are unknown.
                if *tok == T::UnknownType && convention > 0 && n == Some(convention) {
                    return (Mark::Ok, fill1(Key::V2ConventionEntries, &convention.to_string()), None);
                }
                let m = match (n, good) {
                    (None, _) => Mark::Todo,
                    (Some(0), true) => Mark::Todo,
                    (Some(_), true) => Mark::Ok,
                    (Some(0), false) => Mark::Ok,
                    (Some(_), false) => {
                        if matches!(tok, T::Unanchored | T::Excluded | T::Unavailable | T::Discarded) {
                            Mark::Warn
                        } else {
                            Mark::Bad
                        }
                    }
                };
                (m, t(*k).to_string(), Some(n.map(|x| x.to_string()).unwrap_or_else(|| t(Key::None_).to_string())))
            })
            .collect();
        // Deleted locally: right under the unanchored line (the list is laid out two to a row), grey, never a
        // warning.
        if let Some(l) = local {
            let at = rows.iter().position(|(_, label, _)| label == t(Key::U3AuUnanchored)).map(|i| (i + 2).min(rows.len())).unwrap_or(rows.len());
            rows.insert(at, (Mark::Todo, t(Key::LampLocalDeletion).to_string(), Some(l.to_string())));
        }
        rows
    }

    /// A ledger entry's detail page. A grant shows the grant page. Any other entry: its name with its number and
    /// kind and its state; its anchoring state, number and (a record) file; the actions it takes (a record grants
    /// and exports; an entry off chain goes on chain, or into the queue first; an anchored entry of another kind
    /// takes a note); its details; and a record not deleted, "delete record" in red.
    pub(super) fn entry_detail(&mut self, ui: &mut egui::Ui, id: &str, now: f64) {
        use zikaron::tokens::EntryType as E;
        self.ensure_rows(now);
        let rows_all: Vec<crate::ledgerx::Row> = self.shell.rows.as_ref().map(|(r, _)| r.clone()).unwrap_or_default();
        let Some(row) = rows_all.iter().find(|r| r.id.eq_ignore_ascii_case(id)).cloned() else {
            states::empty(ui, Glyph::Ledger, t(if self.shell.rows.is_some() { Key::DetailGone } else { Key::WbNotRead }));
            return;
        };
        if row.kind == E::Grant {
            self.grant_detail(ui, &row, now);
            return;
        }
        let reading = crate::retractx::read(&rows_all);
        let (tag, summary, struck) = row_face(&rows_all, &reading, &row);
        let at = self.shell.remembered.as_ref().map(|r| r.at);
        let mut confirm: Option<U3Confirm> = None;
        let mut queue_it = false;
        let mut to_kit = false;
        let mut to_grant = false;
        let mut annotate = false;
        let files = if row.kind == E::History { self.files_of(&row.id) } else { Vec::new() };
        stagger(ui, 0, |ui| {
            card::hero(ui, &summary, &format!("#{} \u{b7} {tag}", row.seq), struck, |ui| {
                if struck {
                    mark::pill(ui, t(Key::V2Deleted), PillTone::Grey);
                } else {
                    lamp_pill_ui(ui, row.lamp, at);
                }
            });
        });
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::BasicInfo), "", |ui| {
                card::form(ui, |ui, f| {
                    anchor_rows(ui, f, row.lamp, row.anchored_at, at);
                    let _ = f.info(ui, t(Key::V2RecordNo), card::Show::Mono(&format!("#{}", row.seq)));
                    if !files.is_empty() {
                        let _ = f.info(ui, t(Key::U3File), card::Show::Text(&files.join(", ")));
                    }
                });
            });
        });
        // Available actions: a record exports and grants; an entry off chain goes on chain (or into the queue
        // first); an anchored entry of another kind takes a note.
        let offer = !matches!(
            row.lamp,
            crate::ledgerx::Lamp::Anchored
                | crate::ledgerx::Lamp::Remembered
                | crate::ledgerx::Lamp::RememberedStale
                | crate::ledgerx::Lamp::Included
                | crate::ledgerx::Lamp::Submitted
                | crate::ledgerx::Lamp::Deleted
                | crate::ledgerx::Lamp::LocalDeletion
        );
        let pos = self.shell.queue.items.iter().filter(|q| !q.step.in_flight()).position(|q| q.id == row.id);
        let anchored = row.lamp == crate::ledgerx::Lamp::Anchored;
        let record = row.kind == E::History && !struck;
        // Granting waits for the chain: until anchored the key is off and says why on hover, as on the record's own
        // page.
        let remembered = matches!(row.lamp, crate::ledgerx::Lamp::Remembered | crate::ledgerx::Lamp::RememberedStale);
        let why = if anchored { "" } else { t(if remembered { Key::V2AwaitThisCheck } else { Key::V2AnchorFirstHint }) };
        let acts = DetailActs {
            // The genesis entry and entries whose queueing failed can be queued here too.
            chain: offer.then_some(pos.map(|i| i + 1)),
            kit: record,
            grant: record.then_some((anchored, why, Key::U3UseForGrant)),
            annotate: !offer && anchored && !matches!(row.kind, E::History),
        };
        stagger(ui, 2, |ui| {
            let hit = detail_acts(ui, &acts);
            if let Some(n) = hit.send {
                confirm = Some(U3Confirm::Send { count: n });
            }
            queue_it = hit.queue;
            to_kit = hit.kit;
            to_grant = hit.grant;
            annotate = hit.annotate;
        });
        stagger(ui, 3, |ui| {
            let mut kv = Vec::new();
            if let Some(w) = row.work.clone() {
                kv.push((t(Key::ContentHash), Val::mono(w)));
            }
            kv.extend([
                (t(Key::DetailPick), Val::mono(row.id.clone())),
                (t(Key::U3RawType), Val::figure(if row.facts.raw_type.is_empty() { row.kind.as_str().to_string() } else { row.facts.raw_type.clone() })),
                (t(Key::U3RawSeq), Val::figure(row.seq.to_string())),
                (t(Key::EntryAuthor), Val::mono(row.author.clone())),
                (t(Key::EntryPrev), row.prev.clone().map(Val::mono).unwrap_or_else(|| Val::text(t(Key::None_)))),
                (t(Key::CanonBytes), Val::figure(row.bytes.to_string())),
            ]);
            if let Some((chain, tx)) = row.tx.clone() {
                kv.push((t(Key::AnchorTx), Val::mono(format!("{chain} \u{b7} {tx}"))));
            }
            details_card(ui, "entry-details", &kv);
        });
        // A record not deleted: deleting it is a red row, confirmed in its sheet.
        if record {
            stagger(ui, 4, |ui| {
                card::form(ui, |ui, f| {
                    if f.danger(ui, t(Key::V2DeleteWork)).clicked() {
                        confirm = Some(U3Confirm::Retract { subject: row.id.clone() });
                    }
                });
            });
        }
        if to_kit {
            self.typed.pick_ids = row.id.to_ascii_lowercase();
            self.typed.pick_from.clear();
            self.typed.pick_to.clear();
            self.go(Place::View(crate::nav::View::Works, crate::nav::tab::WORKS_KIT), now);
        }
        if to_grant {
            if let Some(w) = row.work.clone() {
                self.typed.g_work = w;
                self.typed.g_history = row.id.clone();
            }
            self.go(Place::View(crate::nav::View::Grants, crate::nav::tab::GRANTS_NEW), now);
        }
        if queue_it {
            self.act(Action::QueueEntry { id: row.id.clone() }, now);
        }
        if annotate {
            self.typed.annotate_subject = row.id.clone();
            self.u3_form_open(U3Form::Annotate);
        }
        if let Some(cf) = confirm {
            self.u3_open_confirm(cf);
        }
    }

    /// A validity window as two dates "from to", or a note when there is none.
    pub(super) fn window_days(&self, w: Option<(u64, u64)>) -> String {
        window_long(w)
    }
}
