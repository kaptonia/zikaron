use super::*;
use super::works::{anchor_rows, detail_acts, DetailActs, DATE_W};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SendNow {
    Can(usize),
    Reading,
    Out,
    Nothing,
}

impl Win {
    pub(super) fn ensure_rows(&mut self, now: f64) {
        if self.shell.rows_current() || self.shell.home.is_none() || self.shell.tasks.attempted(crate::task::Kind::Ledger) {
            return;
        }
        self.auto(Action::ReadLedger, now);
    }

    pub(super) fn ensure_grants(&mut self, now: f64) {
        if self.shell.grants_current() || self.shell.home.is_none() || self.shell.tasks.attempted(crate::task::Kind::Grants) {
            return;
        }
        self.auto(Action::ReadGrants, now);
    }

    pub(super) fn chain_now(&self) -> Option<u64> {
        self.shell.chain_now()
    }

    pub(super) fn ledger_acts(&mut self, ui: &mut egui::Ui, now: f64) {
        self.ensure_rows(now);
        let items = [menu::row(t(Key::U3AnnotateGo))];
        if menu::menu_key(ui, "ledger-more", t(Key::NavMore), false, 190.0, &items).is_some() {
            self.annotate_open(String::new());
        }
        let cells = [seg::Cell::from(t(Key::U3FilterAll)), seg::Cell::from(t(Key::LedgerUnconfirmed)), seg::Cell::from(t(Key::KindGrant))];
        if let Some(i) = seg::seg(ui, "ledger-filter", &cells, self.ux.u3.led_filter) {
            self.ux.u3.led_filter = i;
        }
    }

    pub(super) fn ledger_page(&mut self, ui: &mut egui::Ui, now: f64) {
        let chain_now = self.shell.chain_now();
        self.ensure_rows(now);
        self.ensure_grants(now);
        stagger(ui, 0, |ui| self.audit_strip(ui, now));
        let read = self.shell.rows.is_some();
        let mut query = self.ux.search.get("ledger").cloned().unwrap_or_default();
        let mut range = self.ux.range.get("ledger").cloned().unwrap_or_default();
        let today = (self.shell.clock)();
        stagger(ui, 1, |ui| search_row(ui, "ledger", &mut query, t(Key::SearchLedger), &mut range, today));
        let hide = self.shell.settings.hide_local_deletions;
        let filter = self.ux.u3.led_filter;
        let (view, generation) = kept(ui.ctx(), self.shell.moved, "ledger", (&query, &range, hide, filter), || {
            let rows: &[crate::ledgerx::Row] = self.shell.rows.as_ref().map(|(r, _)| r.as_slice()).unwrap_or_default();
            let reading = crate::retractx::read(rows);
            let shown: Vec<&crate::ledgerx::Row> = rows
                .iter()
                .filter(|r| !(hide && r.lamp.local()))
                .filter(|r| match filter {
                    1 => !r.lamp.confirmed(),
                    2 => r.kind == zikaron_glue::book::Kind::Grant,
                    _ => true,
                })
                .filter(|row| {
                    let (tag, summary, _) = row_face(rows, &reading, row, chain_now);
                    matches(&query, &[&format!("#{}", row.seq), tag, &summary, &row.id])
                })
                .filter(|row| crate::when::within(row.anchored_at, &range.0, &range.1))
                .collect();
            let tr = |r: &&crate::ledgerx::Row| {
                let (tag, summary, struck) = row_face(rows, &reading, r, chain_now);
                let row = table::Row {
                    cells: vec![table::Cell::Seq(r.seq), table::Cell::Tag(tag.to_string()), table::Cell::Text(summary), table::Cell::Mono(first_anchor_say(r.anchored_at)), table::Cell::Mark(lamp_mark(r.lamp))],
                    click: true,
                    gone: struck,
                    on: false,
                };
                (row, r.id.clone())
            };
            let (new, old): (Vec<&crate::ledgerx::Row>, Vec<&crate::ledgerx::Row>) = shown.iter().partition(|r| r.law == zikaron_glue::book::Law::Two);
            let all: Listed = shown.iter().map(tr).collect();
            let split = (!new.is_empty() && !old.is_empty()).then(|| (new.iter().map(tr).collect::<Listed>(), old.iter().map(tr).collect::<Listed>()));
            (rows.is_empty(), all, split)
        });
        let (none_at_all, all, split) = (&view.0, &view.1, &view.2);
        let mut open: Option<String> = None;
        let offer = if read { self.shell.set_up() } else { None };
        let set_up_here = offer.is_some();
        let marked: Option<String> = match offer {
            Some(crate::shell::SetUp::Fetch) => self.shell.unfetched.as_ref().map(super::watch::mark_title),
            Some(crate::shell::SetUp::Create) | None => None,
        };
        if offer == Some(crate::shell::SetUp::Create) {
            self.genesis_offered();
        }
        let (mut set_up, mut fetch) = (false, false);
        stagger(ui, 2, |ui| {
            let create = || key::Key::new(t(Key::WizGenesisTitle), Role::Primary);
            let fetch_key = || key::Key::new(t(Key::DoFetchLedger), Role::Primary);
            let empty = if !read {
                t(Key::WbNotRead)
            } else if *none_at_all {
                t(Key::WbRecentNone)
            } else if !query.trim().is_empty() || !range.0.is_empty() || !range.1.is_empty() {
                t(Key::SearchNone)
            } else {
                t(Key::U3NoneOfThisKind)
            };
            if set_up_here && all.is_empty() {
                match &marked {
                    Some(title) => card::form(ui, |ui, f| fetch = f.act(ui, title, "", fetch_key()).clicked()),
                    None => card::form(ui, |ui, f| set_up = f.act(ui, empty, "", create()).clicked()),
                }
                return;
            }
            let cols = [table::SEQ, table::TYPE, table::col("", table::Col::Fr(1.0)), table::col("", table::Col::Px(DATE_W)), table::MARK];
            match split {
                None => open = listed_table(ui, "ledger", table::Spec::new(&cols, generation).empty(empty), all),
                Some((new, old)) => {
                    open = listed_table(ui, "ledger", table::Spec::new(&cols, generation).empty(empty), new);
                    ui.add_space(tk::CARD_GAP);
                    card::section(ui, t(Key::SegOld), "", |ui| {
                        if let Some(id) = listed_table(ui, "ledger-old", table::Spec::new(&cols, generation).empty(empty), old) {
                            open = Some(id);
                        }
                    });
                }
            }
            if set_up_here {
                ui.add_space(tk::CARD_GAP);
                match &marked {
                    Some(title) => card::form(ui, |ui, f| fetch = f.act(ui, title, "", fetch_key()).clicked()),
                    None => card::form(ui, |ui, f| set_up = f.act(ui, t(Key::AuRoot), "", create()).clicked()),
                }
            }
        });
        if set_up {
            self.wizard_open(crate::nav::Step::Genesis);
        }
        if fetch {
            self.go(Place::Settings(Section::Data), now);
        }
        self.ux.search.insert("ledger", query);
        self.ux.range.insert("ledger", range);
        if let Some(id) = open {
            self.push(Route::Entry(id), now);
        }
    }

    pub(super) fn own_audit_said(&self, label: &str) -> (String, bool) {
        match self.shell.ledger_verdict() {
            Some(crate::shell::Verdict::HandedOn(_)) => (t(Key::V2HandedOn).to_string(), true),
            _ => (label_human(label), false),
        }
    }

    fn audit_strip(&mut self, ui: &mut egui::Ui, now: f64) {
        use crate::shell::Verdict;
        let verdict = self.shell.ledger_verdict();
        let (m, state) = match verdict {
            Some(Verdict::HandedOn(_)) => (Mark::Ok, t(Key::V2HandedOn)),
            Some(Verdict::Normal) => (Mark::Ok, t(Key::V2Normal)),
            Some(Verdict::Abnormal) => (Mark::Bad, t(Key::V2Abnormal)),
            None if self.shell.settings.chain_id.is_none() || self.shell.settings.registry.is_none() || self.shell.endpoints.is_empty() => (Mark::Warn, t(Key::U3AuditChipNoNetwork)),
            None => (Mark::Todo, t(Key::U3AuditChipNever)),
        };
        let total = match verdict {
            Some(Verdict::HandedOn(n)) => fill1(Key::V2HandedLater, &n.to_string()),
            Some(Verdict::Normal | Verdict::Abnormal) | None => self.shell.rows.as_ref().map(|(r, _)| fill1(Key::U3EntriesTotal, &r.len().to_string())).unwrap_or_default(),
        };
        let root_anchored = self.shell.rows.as_ref().and_then(|(r, _)| crate::ledgerx::root_anchored(r));
        let sendable = !self.unanchored_now().is_empty();
        let ready = self.send_all_ready();
        let stuck = self.shell.stuck.clone();
        let bump = stuck.as_ref().filter(|s| matches!(s.offer, crate::task::Offer::Resend { .. } | crate::task::Offer::Unheld { .. })).and_then(|s| s.txs.last().cloned());
        let spent = stuck.as_ref().is_some_and(|s| s.offer == crate::task::Offer::Spent);
        let unheld = stuck.as_ref().is_some_and(|s| matches!(s.offer, crate::task::Offer::Unheld { .. }));
        let unpriced = stuck.as_ref().and_then(|s| s.unpriced).map(unpriced_key);
        let audit = self.shell.audit.clone();
        let summary = audit.as_ref().map(|a| self.own_audit_said(&a.label).0).unwrap_or_else(|| t(Key::U3AuditChipNever).to_string());
        let check = audit.as_ref().map(|a| (self.audit_body(a), self.audit_details(ui, a)));
        let (mut send_all, mut bump_open, mut copied, mut upgrade) = (false, false, None, false);
        card::form(ui, |ui, f| {
            send_all = f.summary(ui, Glyph::Queue, card::Tint::Blue, t(Key::WbAudit), Some((m, state)), &total, |ui| sendable && key::key(ui, t(Key::U3SendAll), Role::Secondary, ready).clicked());
            if self.shell.upgrade.as_ref().is_some_and(crate::upgradex::Stage::pending) {
                upgrade = f.act(ui, t(Key::UpPending), "", key::Key::new(t(Key::Continue), Role::Primary)).clicked();
            }
            if root_anchored == Some(false) {
                let _ = f.info(ui, t(Key::AuRoot), card::Show::Mark(Mark::Bad, t(Key::U3RootUnanchored)));
            }
            if bump.is_some() {
                let k = key::Key::new(t(if unheld { Key::U3ResendKey } else { Key::U3BumpKey }), Role::Secondary);
                bump_open = f.act(ui, t(Key::U3LastSend), if unheld { t(Key::U3Unheld) } else { "" }, k).clicked();
            } else if spent {
                let _ = f.info(ui, t(Key::U3LastSend), card::Show::Text(&fill1(Key::U3BumpSpent, &crate::queue::RESENDS_MAX.to_string())));
            } else if let Some(k) = unpriced {
                let _ = f.info(ui, t(Key::U3LastSend), card::Show::Text(t(k)));
            } else {
                copied = self.last_batch_rows(ui, f);
            }
            copied = copied.take().or(f
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
                .flatten());
        });
        tell_copied(ui.ctx(), copied);
        if let (true, Some(tx)) = (bump_open, bump) {
            self.u3_open_confirm(U3Confirm::Bump { tx });
        }
        if send_all {
            self.send_all_unanchored(now);
        }
        if upgrade {
            self.upgrade_sheet_open();
        }
    }

    fn unanchored_now(&self) -> Vec<(String, bool)> {
        self.shell.rows.as_ref().map(|(r, _)| crate::kitx::unanchored(&r.iter().collect::<Vec<_>>()).1).unwrap_or_default()
    }

    pub(super) fn send_now(&self) -> SendNow {
        let n = self.shell.sendable_now();
        if n > 0 {
            SendNow::Can(n)
        } else if self.shell.sendable_reading() {
            SendNow::Reading
        } else if !self.shell.queue.submitted().is_empty() {
            SendNow::Out
        } else {
            SendNow::Nothing
        }
    }

    pub(super) fn send_all_ready(&self) -> bool {
        self.unanchored_now().iter().any(|(_, fresh)| *fresh) || matches!(self.send_now(), SendNow::Can(_))
    }

    pub(super) fn send_waits(&self) -> Option<&'static str> {
        match self.send_now() {
            SendNow::Reading => Some(t(Key::SendableReading)),
            SendNow::Out => Some(t(Key::SendWaitOut)),
            SendNow::Can(_) | SendNow::Nothing => None,
        }
    }

    pub(super) fn send_all_unanchored(&mut self, now: f64) {
        for (id, fresh) in self.unanchored_now() {
            if fresh {
                self.act(Action::QueueEntry { id }, now);
            }
        }
        let count = self.shell.sendable_here();
        if count > 0 {
            self.u3_open_confirm(U3Confirm::Send { count });
        } else if let Some(w) = self.send_waits() {
            self.toasts.say(w.to_string(), Tone::Note, now);
        }
    }

    pub(super) fn send_this(&mut self, own: String, now: f64) {
        self.card_for(own);
        if self.ux.u3.confirm.is_none() {
            if let Some(w) = self.send_waits() {
                self.toasts.say(w.to_string(), Tone::Note, now);
            }
        }
    }

    fn audit_details(&self, ui: &egui::Ui, a: &crate::shell::AuditRead) -> (Vec<(&'static str, card::Detail)>, f32) {
        use zikaron::v2::tokens::Key as T;
        let items = crate::auditx::items(&a.report);
        let said = |tok: T| items.iter().find(|i| i.key == tok.as_str()).map(|i| i.said.clone()).filter(|x| !x.is_empty());
        let rows = self.shell.rows.as_ref().map(|(r, _)| r.as_slice()).unwrap_or(&[]);
        let mut out: Vec<(&'static str, card::Detail)> = vec![(t(Key::AuditLabel), card::Detail::Text(label_human(&a.label)))];
        if let Some(head) = rows.first() {
            out.push((t(Key::AuHead), card::Detail::Mono(head.id.clone())));
        }
        if let Some(root) = said(T::Root) {
            out.push((t(Key::AuRoot), card::Detail::Mono(root)));
        }
        let chains = crate::ledgerx::chains_checked(&a.report);
        if !chains.is_empty() {
            let each: Vec<String> = chains.iter().map(|(c, n)| fill2(Key::AuChainOn, &self.chain_label(*c), &n.to_string())).collect();
            out.push((t(Key::AuBasis), card::Detail::Text(each.join("\n"))));
        }
        out.push((t(Key::LedgerEntries), card::Detail::Text(said(T::Entries).unwrap_or_else(|| a.entries.to_string()))));
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

    fn audit_body(&self, a: &crate::shell::AuditRead) -> Vec<(Mark, String, Option<String>)> {
        use zikaron::v2::tokens::Key as T;
        let items = crate::auditx::items(&a.report);
        let unknown = self.shell.rows.as_ref().map(|(r, _)| crate::auditx::unknown_of(&a.report, r));
        let unanchored: Vec<String> = crate::auditx::rows_of(&a.report, T::Unanchored).iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        let local: Option<usize> = self.shell.rows.as_ref().map(|(r, _)| unanchored.iter().filter(|id| r.iter().any(|x| x.lamp.local() && x.id.eq_ignore_ascii_case(id))).count());
        let lists: [(T, Key, bool); 11] = [
            (T::Anchored, Key::LampAnchored, true),
            (T::Unanchored, Key::U3AuUnanchored, false),
            (T::Missing, Key::U3AuMissing, false),
            (T::Findings, Key::U3AuFindings, false),
            (T::Excluded, Key::U3AuExcluded, false),
            (T::Retired, Key::U3AuRetired, false),
            (T::UnknownType, Key::U3AuUnknownType, false),
            (T::Malformed, Key::U3AuMalformed, false),
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
                if let Some(u) = unknown.filter(|u| *tok == T::UnknownType && n.is_some_and(|n| n > 0) && u.left == 0) {
                    return (Mark::Ok, if u.faces == 0 { fill1(Key::V2ConventionEntries, &u.convention.to_string()) } else { crate::lang::filln(Key::AuKnownTypes, &[&u.convention.to_string(), &u.faces.to_string()]) }, None);
                }
                let m = match (n, good) {
                    (None, _) => Mark::Todo,
                    (Some(0), true) => Mark::Todo,
                    (Some(_), true) => Mark::Ok,
                    (Some(0), false) => Mark::Ok,
                    (Some(_), false) => {
                        if matches!(tok, T::Unanchored | T::Excluded | T::Retired | T::Discarded) {
                            Mark::Warn
                        } else {
                            Mark::Bad
                        }
                    }
                };
                let first = match tok {
                    T::Excluded | T::Retired | T::Discarded => crate::auditx::rows_of(&a.report, *tok).first().map(first_row),
                    _ => None,
                };
                let count = n.map(|x| x.to_string()).unwrap_or_else(|| t(Key::None_).to_string());
                (m, t(*k).to_string(), Some(match first {
                    Some(f) => format!("{count} \u{b7} {f}"),
                    None => count,
                }))
            })
            .collect();
        if let Some(l) = local {
            let at = rows.iter().position(|(_, label, _)| label == t(Key::U3AuUnanchored)).map(|i| (i + 2).min(rows.len())).unwrap_or(rows.len());
            rows.insert(at, (Mark::Todo, t(Key::LampLocalDeletion).to_string(), Some(l.to_string())));
        }
        let old = self.shell.rows.as_ref().map(|(r, _)| r.iter().any(|x| x.law == zikaron_glue::book::Law::One)).unwrap_or(false);
        let continued = a.report.member(T::Continues.as_str()).is_some_and(|c| !matches!(c, zikaron::json::Value::Null));
        if old && !continued {
            rows.insert(0, (Mark::Warn, t(Key::U3AuPending).to_string(), None));
        }
        rows
    }

    pub(super) fn entry_detail(&mut self, ui: &mut egui::Ui, id: &str, now: f64) {
        let chain_now = self.shell.chain_now();
        use zikaron_glue::book::Kind as E;
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
        let (tag, summary, struck) = row_face(&rows_all, &reading, &row, chain_now);
        let at = self.shell.remembered.as_ref().map(|r| r.at);
        let mut confirm: Option<U3Confirm> = None;
        let mut send_this = false;
        let mut queue_it = false;
        let mut to_kit = false;
        let mut to_grant = false;
        let mut annotate = false;
        let files = if row.kind == E::History { self.files_of(&row.id, now) } else { Vec::new() };
        stagger(ui, 0, |ui| {
            card::hero(ui, &summary, &format!("#{} \u{b7} {tag}", row.seq), struck, &[if struck { card::HeroPill { words: t(Key::V2Deleted).to_string(), tone: PillTone::Grey, live: false } } else { lamp_hero(row.lamp, at) }]);
        });
        stagger(ui, 1, |ui| {
            card::section(ui, t(Key::BasicInfo), "", |ui| {
                card::form(ui, |ui, f| {
                    anchor_rows(ui, f, row.lamp, row.anchored_at, at, crate::ledgerx::refusal_of(&row.id, &self.shell.queue));
                    let _ = f.info(ui, t(Key::V2RecordNo), card::Show::Mono(&format!("#{}", row.seq)));
                    if !files.is_empty() {
                        let _ = f.info(ui, t(Key::U3File), card::Show::Text(&files.join(", ")));
                    }
                });
            });
        });
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
        let remembered = matches!(row.lamp, crate::ledgerx::Lamp::Remembered | crate::ledgerx::Lamp::RememberedStale);
        let why = if anchored { "" } else { t(if remembered { Key::V2AwaitThisCheck } else { Key::V2AnchorFirstHint }) };
        let acts = DetailActs {
            chain: offer.then_some(pos.map(|i| i + 1)),
            kit: record,
            grant: record.then_some((anchored, why, Key::U3UseForGrant)),
            annotate: !offer && anchored && !matches!(row.kind, E::History),
        };
        stagger(ui, 2, |ui| {
            let hit = detail_acts(ui, &acts);
            if hit.send.is_some() {
                send_this = true;
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
                (t(Key::U3RawType), Val::text(t(type_words(&row).0))),
                (t(Key::U3RawSeq), Val::figure(row.seq.to_string())),
                (t(Key::EntryAuthor), Val::mono(row.author.clone())),
                (t(Key::EntryPrev), row.prev.clone().map(Val::mono).unwrap_or_else(|| Val::text(t(Key::None_)))),
                (t(Key::CanonBytes), Val::figure(row.bytes.to_string())),
            ]);
            if let Some(raw) = type_words(&row).1 {
                kv.push((t(Key::U3TypeAsWritten), Val::mono(raw)));
            }
            let anchors = self.shell.audit.as_ref().map(|a| crate::ledgerx::anchors_of(&a.report, &row.id)).filter(|x| !x.is_empty());
            for (chain, tx) in anchors.or_else(|| row.tx.clone().map(|x| vec![x])).unwrap_or_default() {
                kv.push((t(Key::AnchorTx), Val::mono(format!("{} \u{b7} {tx}", self.chain_label(chain)))));
            }
            details_card(ui, "entry-details", &kv);
        });
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
            self.annotate_open(row.id.clone());
        }
        if let Some(cf) = confirm {
            self.u3_open_confirm(cf);
        }
        if send_this {
            self.send_this(row.id.clone(), now);
        }
    }

    pub(super) fn window_days(&self, w: Option<(u64, u64)>) -> String {
        window_long(w)
    }
}

fn first_row(r: &zikaron::json::Value) -> String {
    use zikaron::json::Value;
    let text = |k: &str| r.member(k).and_then(Value::as_str).map(crate::ledgerx::short).unwrap_or_default();
    let int = |k: &str| r.member(k).and_then(Value::as_int);
    match (int("seq"), int("blockNumber")) {
        (Some(seq), _) => format!("#{seq} {}", text("author")),
        (None, Some(block)) => format!("{} @{block}", text("sender")),
        _ => String::new(),
    }
}

pub(super) fn unpriced_key(u: crate::task::Unpriced) -> Key {
    match u {
        crate::task::Unpriced::NoNode => Key::U3UnpricedNoNode,
        crate::task::Unpriced::OtherChain => Key::U3UnpricedOtherChain,
        crate::task::Unpriced::Unread => Key::U3UnpricedUnread,
    }
}

#[cfg(test)]
mod unpriced {
    use crate::task::Unpriced;

    #[test]
    fn every_reason_a_batch_is_offered_no_price_is_said() {
        let mut said = Vec::new();
        for u in [Unpriced::NoNode, Unpriced::OtherChain, Unpriced::Unread] {
            let k = super::unpriced_key(u);
            let (_, zh, en) = crate::lang::TABLE.iter().find(|(key, _, _)| *key == k).copied().expect("a sentence");
            assert!(!zh.is_empty() && !en.is_empty(), "{u:?}");
            said.push(k);
        }
        said.dedup();
        assert_eq!(said.len(), 3, "each reason its own sentence");
    }
}
