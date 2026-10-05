//! The single window: the only place in the app that uses the egui family; no other module uses eframe.
//!
//! Every field on these pages has a real source: the build from `cfg`, the anchor key from the key vault, the
//! address derived from that key, the home's mode from its lock, usage and entry counts from background disk
//! walks, chain figures from background queries, the pen state from the last reconciliation's label. What
//! cannot be read is said to be unread, never shown as a colored reading.
//!
//! No sentence is hard-coded: every text goes through `lang::t` (the tests scan this module and fail on any
//! Chinese literal).
//!
//! The UI frame never blocks and never touches the disk or network: it only drains; disk reads and chain
//! queries run in background tasks.
//!
//! Sizes, colors and motion come only from `zikaron_ui`; pages lay out its controls and say nothing about
//! pixels or colors themselves.

use crate::action::{apply, Action, Applied};
use crate::auditx::Pen;
use backup::{Bk, BkStep, RestoreFrom};
use crate::firstrun::{Author, Grantee};
use crate::lang::{fill1, fill2, fill3, t, Key};
use crate::nav::{History, Place, Route, Section, Stack};
use crate::shell::{build_kind, Page, Shell};
use crate::task::Done;
use crate::trace::{self, Sink};
use zikaron_ui::button::{self as key, Phase, Role};
use zikaron_ui::egui;
use zikaron_ui::icons::Glyph;
use zikaron_ui::kv::Val;
use zikaron_ui::mark::Mark;
use zikaron_ui::palette::{c, Tone as PillTone, C};
use zikaron_ui::toast::{Toasts, Tone};
use zikaron_ui::tokens::{self as tk, Type};
use zikaron_ui::{card, datepick, drop, fold, full, input, kv, mark, menu, motion, page, paint, pick, pin, rail, seg, sheet, skin, states, table, toggle, width};

/// Initial window size, defined once.
pub const W: f32 = 1180.0;
pub const H: f32 = 760.0;

#[derive(Default)]
struct Typed {
    home: String,
    cap: String,
    migrate: String,
    adopt: String,
    /// Import a folder of grants: the folder's path.
    grant_dir: String,
    endpoints: String,
    mirror_out: String,
    // Ledger, anchoring and queue.
    annotate_subject: String,
    annotate_note: String,
    work_note: String,
    /// Files of a batch signing (as many as were dropped). Empty means one entry for the content at hand.
    batch_files: Vec<String>,
    /// The four "recorded for" fields (application, other party's identity, number, role), passed to the
    /// action layer as typed.
    for_fields: [String; 4],
    /// Where to fetch a restored identity's ledger from: kit, ledger directory, backup or publication
    /// address.
    fetch_from: String,
    repo_path: String,
    // Kits, depth, grants and checklist.
    pick_from: String,
    /// Entries named in the entry picker (ids, comma-separated; any subset via one switch per entry).
    pick_ids: String,
    pick_to: String,
    kit_attach: String,
    kit_note: String,
    kit_out: String,
    /// Badge location (the shared location picker; empty means this home's kits).
    badge_out: String,
    g_grantee: String,
    g_work: String,
    g_terms: String,
    /// Grant file location (empty means the home's kits).
    gf_out: String,
    /// The settings publication address, and the local kit the publication check compares with.
    publish: String,
    publish_local: String,
    g_history: String,
    g_from: String,
    g_to: String,
    g_scope: String,
    g_upstream: String,
    g_exclusive: bool,
    wiz_said: String,
    // Revocation, adoption, succession and others' ledgers.
    ad_rows: String,
    ad_sig: String,
    /// Import existing anchors: the key address field (empty means this key).
    ad_key: String,
    /// Attest for someone: the pasted claim text.
    at_text: String,
    sc_to: String,
    sc_kind: String,
    sc_statement: String,
    rd_address: String,
    rd_dir: String,
    dg_address: String,
    dg_dir: String,
    dg_work: String,
    dg_from: String,
    dg_to: String,
    dg_snapshot: String,
    vf_path: String,
    vf_work: String,
    vt_typed: String,
    /// The names typed with a grant being added (kept on this machine only).
    vt_note: String,
    vt_issuer_note: String,
    vt_grant: String,
    vt_dir: String,
    ck_typed: String,
    ck_ledgers: String,
    ck_endpoints: String,
    ck_registry: String,
    ck_from: String,
    ck_now: String,
    /// Check page: the work file and the terms file held against the grant (optional).
    ck_file: String,
    ck_terms: String,
}

struct Win {
    shell: Shell,
    toasts: Toasts,
    typed: Typed,
    reaped: Option<crate::task::Reaped>,
    /// The opened entry (details read from disk now). Lives only on the interface side.
    opened: Option<crate::ledgerx::Detail>,
    /// When the self-audit clock last started on its own (interface clock).
    last_tick: f64,
    /// Whether the double-sale dialog is up.
    clash_modal: bool,
    /// The three fields at the last overlap check. It runs again only when they change.
    clash_key: String,
    /// How many recorded troubles have been told (an index into `Shell::faults`).
    faults_told: usize,
    ux: Ux,
}

/// The four places things are written: the landing toast says where and, when it differs from the choice,
/// why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Out {
    Kit,
    Badge,
    Mirror,
    Snapshot,
}

/// The toast key that opens the old data the last fetch left (not a task kind).
const OLD_DATA_TAG: u64 = u64::MAX;

impl Win {
    /// One frame. The window's `update` only calls this; it does not use `eframe::Frame`, so tests can
    /// call the same code in a headless frame (`probe_face`).
    fn draw(&mut self, ctx: &egui::Context) {
        zikaron_ui::probe::begin(ctx);
        let now = ctx.input(|i| i.time);
        skin::follow(ctx, self.appearance());
        let landed = self.shell.drain_at(now);
        // An anchoring landed and talked to the node: ask the chain once more and record chain time.
        if landed.iter().any(|o| matches!(o.result, Ok(Done::Anchored { .. }))) && self.shell.anchor.is_some() && !self.shell.endpoints.is_empty() {
            self.auto(Action::ReadChain, now);
        }
        // A new check landing closes the issuer-ledger field: it was opened for the last pass's gap.
        if landed.iter().any(|o| matches!(o.result, Ok(Done::Checked(_)))) {
            self.ux.u3.ck_source_open = None;
        }
        self.sync_landed(&landed, now);
        // A refused exit gate is the answer of the place that pressed the export (a refusal from a gate started
        // on an earlier source belongs to that one).
        for o in &landed {
            if let (crate::task::Kind::Gate, false, Err(f)) = (o.kind, o.stale, &o.result) {
                if let Some(site) = self.ux.gate_site.take() {
                    self.vault_back(site, Applied::Trouble(f.clone()), now);
                }
            }
        }
        // The kinds that answer in `said` are told where that answer is taken (below), not here.
        let claimed: Vec<_> = landed.into_iter().filter(|o| !crate::action::lands_said(o.kind)).filter(|o| self.ux.claim(o.kind)).collect();
        for o in claimed {
            let kind = o.kind;
            // A long key shows how the task the person started landed (a check, or red and a shake).
            let good = match &o.result {
                Ok(d) => !done_is_bad(d),
                Err(_) => false,
            };
            // A passed exit gate is not how the export went: the export runs where the gate landed and its own
            // answer sets the key (below, from `gate_said`).
            if !matches!(o.result, Ok(Done::Submitted { .. }) | Ok(Done::GatePassed { .. })) {
                self.ux.landed.insert(kind, (good, now));
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
                // Files that were no longer the chosen records' originals when the kit was written did not go
                // in: say how many.
                Ok(Done::Kit { path, left_out, unreadable, .. }) if !left_out.is_empty() || !unreadable.is_empty() => (
                    crate::lang::filln(Key::SaidKitLeftOut, &[&folder_of(&path), &left_out.len().to_string(), &unreadable.len().to_string()]),
                    Tone::Bad,
                ),
                Ok(Done::Kit { path, .. }) => (fill1(Key::SaidKit, &folder_of(&std::path::Path::new(&path).parent().map(|p| p.display().to_string()).unwrap_or_default())), Tone::Note),
                Ok(Done::Keystore(crate::task::Keystore::BackedUp { path, .. })) => (fill1(Key::SaidKeyBackedUp, &folder_of(&path)), Tone::Note),
                Ok(Done::BackupMade { path, .. }) => (fill1(Key::SaidBackupMade, &width::file_name(&path)), Tone::Note),
                // The backup's contents go on the confirmation card: no toast.
                Ok(Done::BackupSeen { .. }) => continue,
                // Passcode tasks are finished by the shell; the answer is in `vault_said` and told below.
                Ok(Done::Vault(_)) => continue,
                // Those kinds were filtered out above (`said`).
                Ok(Done::Gas { .. }) | Ok(Done::Took { .. }) | Ok(Done::Hashed { .. }) | Ok(Done::Copied { .. }) => continue,
                // An export's gate passed: the shell ran the export, its answer is in `gate_said` and told below.
                Ok(Done::GatePassed { .. }) => continue,
                // Attachment digests land silently: their row updates itself.
                Ok(Done::Vetted(_)) => continue,
                // The existing-anchor table and a read claim: results are on the card, no toast.
                Ok(Done::KeyAnchors { .. }) | Ok(Done::Claim { .. }) => continue,
                Ok(Done::Adopt { proofs, .. }) => (
                    fill2(Key::SaidProofs, &proofs.len().to_string(), &proofs.iter().filter(|p| p.ok()).count().to_string()),
                    if proofs.iter().all(|p| p.ok()) { Tone::Note } else { Tone::Bad },
                ),
                // The new key's history is read on its sheet: no toast.
                Ok(Done::Sighting { .. }) => continue,
                // A read-only network's reading shows on its own row: no toast.
                Ok(Done::NetRead { .. }) => continue,
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
                // Vault listing gets no toast: a reading done on opening and after imports.
                Ok(Done::Held { .. }) => continue,
                Ok(Done::Published { read, .. }) => {
                    if read.complete() {
                        (format!("{} · {}", t(Key::PublishOk), fill1(Key::PublishOkSay, &read.total.to_string())), Tone::Note)
                    } else {
                        (format!("{} · {}", t(Key::PublishPartial), fill2(Key::PublishPartialSay, &read.missing.len().to_string(), &read.differ.len().to_string())), Tone::Bad)
                    }
                }
                Ok(Done::Badge(b)) => (fill2(Key::SaidBadge, &folder_of(&b.txt.parent().map(|p| p.display().to_string()).unwrap_or_default()), &b.hops.to_string()), Tone::Note),
                // A conflict is answered by its card on the home page, not by a toast.
                Ok(Done::FetchConflict { .. }) => continue,
                // A tail check the product started itself says nothing: a passing one lifts the read-only bar,
                // a gap changes what the bar says.
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
                // Broadcast landed: the shell starts the receipt wait. The person's task toasts when the
                // receipt lands, so its claim is returned.
                Ok(Done::Submitted { .. }) => {
                    self.ux.asked.push(crate::task::Kind::Anchor);
                    continue;
                }
                // "Sent, but failed on chain" is only for a receipt status other than 1.
                Ok(Done::Anchored { tx, state, .. }) if crate::action::receipt_failed(&state) => {
                    self.say_fault(&crate::fault::Fault::known(crate::fault::Known::SendFailed, tx), now);
                    continue;
                }
                // Included says "N anchored"; not yet receipted says "N submitted, waiting".
                Ok(Done::Anchored { sent, confirmed, .. }) => {
                    if confirmed {
                        (fill1(Key::SaidSent, &sent.to_string()), Tone::Note)
                    } else {
                        (fill1(Key::SaidSubmitted, &sent.to_string()), Tone::Note)
                    }
                }
                // Failed: the shell recorded it; non-network faults are told by `tell_faults` (with details);
                // network faults do not go that way, and this task was started by the person.
                Err(f) => {
                    if crate::watchx::is_network(&f) && f.then_key() != Some(Key::ExitRetryLater) {
                        self.say_fault(&f, now);
                    }
                    continue;
                }
            };
            // A task the person left the page of says where to look ("view" goes back there).
            let view = self.task_away(kind).then(|| vec![(t(Key::NavLook).to_string(), zikaron_ui::toast::Act::Tag(kind as u64))]).unwrap_or_default();
            let why = out.and_then(|x| self.landing_note(x)).unwrap_or_default();
            self.toasts.say_keys(said, &why, "", tone, now, view);
        }

        // The actions whose slow half ran in the background landed: each answer goes back to where it started.
        for k in crate::task::Kind::ALL {
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
        // An export whose gate passed ran where the gate landed: its answer is told here, and the long key shows
        // how the export itself went (a gate read again for another home starts over).
        if let Some(a) = self.shell.gate_said.take() {
            if !matches!(a, Applied::Started(_)) {
                self.ux.landed.insert(crate::task::Kind::Gate, (!matches!(a, Applied::Trouble(_) | Applied::Refused(_)), now));
            }
            let a = self.told(a, None, now);
            // The export's own answer goes back to the place that pressed it (a gate read again keeps waiting).
            if !matches!(a, Applied::Started(_)) {
                if let Some(site) = self.ux.gate_site.take() {
                    self.vault_back(site, a, now);
                }
            }
        }
        // A file dialog that could not open says why (the platform interface keeps it; read once per frame).
        if let Some(f) = crate::platform::take_trouble() {
            self.shell.faults.push(f);
        }
        // Resume a submitted anchor after a while: the receipt wait has a deadline; from then on it asks every
        // fifteen seconds until included. Never resent.
        if self.shell.queue.submitted().is_empty() {
            self.shell.resume_blocked = None;
        } else if now - self.shell.resumed_at > 15.0 {
            self.shell.resumed_at = now;
            // Through the action layer's own entry (`Action::Resume`); asked first whether that entry holds it
            // back now (locked, resealing), so a known refusal is not sent every fifteen seconds.
            if crate::action::held_back(&self.shell, &Action::Resume).is_none() {
                let _ = crate::action::apply(&mut self.shell, Action::Resume);
            }
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((15.0 - (now - self.shell.resumed_at)).max(0.5)));
        }
        set_receipts_stalled(self.shell.resume_blocked.is_some());
        // Idle auto-lock: counted from the last human input; at the configured time, the lock path.
        if ctx.input(|i| !i.events.is_empty() || i.pointer.delta() != egui::Vec2::ZERO) {
            self.ux.last_input = now;
        }
        if self.shell.idle_tick(self.ux.last_input, now) {
            self.ux.gate_clear();
            self.sheets_clear();
        }
        if self.shell.unlocked() {
            // Something must wake a frame at that moment (egui does not repaint while idle).
            let due = self.ux.last_input + self.shell.machine.auto_lock_secs as f64 - now;
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(due.max(0.5)));
        }
        // What just rang (the sentinel's alarms first, then the notices), read once: the one toast says the
        // first and counts the rest, which the alerts page lists. Held until this start has asked whether the
        // first-run wizard opens (it may open this very frame); while the wizard is open nothing rang is said
        // (the alerts page still lists it), so setting up is not interrupted.
        let quiet = self.ux.wizard.is_some();
        let mut rang: Vec<String> = Vec::new();
        if self.ux.wizard_asked {
            rang = std::mem::take(&mut self.shell.rung)
                .into_iter()
                .map(|a| match a.kind {
                    crate::sentinelx::Kind::Revoked => fill1(Key::SaidAlarm, t(Key::AlarmRevoked)),
                    crate::sentinelx::Kind::Handed => fill1(Key::SaidAlarm, t(Key::AlarmHandedPlain)),
                })
                .collect();
            for n in std::mem::take(&mut self.shell.fresh) {
                rang.push(match subject_said(n.item) {
                    true => fill2(Key::SaidNotice, t(item_key(n.item)), &self.entry_say(&n.subject)),
                    false => fill1(Key::SaidAlarm, t(item_key(n.item))),
                });
            }
        }
        if let (Some(first), false) = (rang.first(), quiet) {
            let more = if rang.len() > 1 { fill1(Key::SaidMoreAlerts, &(rang.len() - 1).to_string()) } else { String::new() };
            self.toasts.say_full(first.clone(), &more, "", Tone::Alert, now);
        }
        // The first unlock after upgrading settled the primary identity: said once, by what recovers it.
        if self.shell.primary_settled.take().is_some() {
            let k = match self.shell.primary {
                Some((_, crate::keybox::PrimaryKind::KeyFile)) => Key::PrimaryOnlyKeyFile,
                _ => Key::PrimaryOnlyWords,
            };
            self.toasts.say_full(t(k).to_string(), "", "", Tone::Note, now);
        }
        // Something rang: request the system's attention (the dock icon bounces), once per ringing.
        if std::mem::take(&mut self.shell.attention) && !quiet {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Critical));
        }

        self.shortcuts(ctx, now);
        // Rail, toolbar and page; the first-run wizard and the passcode gate as full-window layers; sheets
        // above all of them.
        self.chrome(ctx, now);
        self.tick(ctx, now);
        self.tell_faults(now);
        self.toasts.set_labels(t(Key::ToastDetail), t(Key::ToastHideDetail), t(Key::ToastClose));
        let left = if self.rail_shown() { tk::RAIL_W } else { 0.0 };
        let drawn = self.toasts.draw(ctx, left);
        if let Some(tag) = drawn.pressed {
            if tag == OLD_DATA_TAG {
                if let Some(p) = self.shell.last_aside.clone() {
                    self.act(Action::ViewOldData { root: p.display().to_string() }, now);
                }
            } else {
                self.view_task(tag, now);
            }
        }
        zikaron_ui::layer::fade_out(ctx);
        // Background work lands through a channel the frame only drains: while any is in flight, look again
        // shortly (moving pieces ask for their own frames).
        if !self.shell.tasks.flying().is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// This machine's chosen appearance (light when never chosen).
    fn appearance(&self) -> skin::Appearance {
        self.shell.machine.appearance.as_deref().and_then(skin::Appearance::named).unwrap_or_default()
    }
}

/// Whether a landed result reads as failed on its long key (red and a shake) though it is not an error.
fn done_is_bad(d: &Done) -> bool {
    match d {
        Done::Verified(v) => !v.mismatches.is_empty(),
        Done::Delivery(d) => !d.matched(),
        Done::Checked(x) => x.judged.verdict == zikaron_kit::tokens::CheckVerdict::Fail.as_str(),
        Done::Diligence(r) => r.double_sold(),
        Done::Published { read, .. } => !read.complete(),
        Done::Chain { gas_wei: None, .. } => true,
        Done::Kit { left_out, unreadable, .. } => !left_out.is_empty() || !unreadable.is_empty(),
        _ => false,
    }
}

/// The last part of a path: toasts name the folder, never the whole path.
fn folder_of(path: &str) -> String {
    width::file_name(path)
}

impl eframe::App for Win {
    fn update(&mut self, ctx: &egui::Context, _f: &mut eframe::Frame) {
        self.draw(ctx);
    }

    /// Clear the canvas to the page ground. It defaults to black, and a gap between two surfaces would show
    /// it as a dark band.
    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        zikaron_ui::palette::canvas()
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Applied::Stopped(r) = apply(&mut self.shell, Action::Quit) {
            self.reaped = Some(r);
        }
        // Quitting locks: the master key is wiped at once and the next window starts at the passcode gate.
        crate::keybox::lock();
    }
}

/// Gaps come with an action sentence. Where a watch row's "next" leads: an unanswered node goes to network
/// settings; expiring holdings and missing chains go to my grants.
fn gap_target(g: crate::watchx::Gap) -> Place {
    use crate::watchx::Say;
    match g.say {
        Say::AuditUnavailable => Place::Settings(Section::Network),
        Say::HoldingNoNow | Say::HoldingExpired | Say::HoldingSoon => Place::Page(Page::Vault),
        _ => Place::Page(g.go),
    }
}

/// As above, the settled place (the link is named after it).
fn gap_place(g: crate::watchx::Gap, role: crate::roles::Role) -> Place {
    match gap_target(g) {
        Place::Page(p) => crate::nav::home_of(p, role),
        other => other,
    }
}

fn gap_say(g: crate::watchx::Gap) -> String {
    use crate::watchx::Say;
    let n = g.n.to_string();
    // Windows are said in days (rounded up: something due in a few hours is due within a day).
    let days = g.secs.map(|s| s.div_ceil(86_400).max(1).to_string()).unwrap_or_default();
    match g.say {
        Say::UnanchoredRead => fill1(Key::GapUnanchoredRead, &n),
        Say::Unanchored => fill1(Key::GapUnanchored, &n),
        Say::Queue => fill1(Key::GapQueue, &n),
        Say::WindowRead => fill1(Key::GapWindowRead, &n),
        Say::NoChainTime => fill1(Key::GapNoChainTime, &n),
        Say::Window => fill2(Key::GapWindow, &n, &days),
        Say::BackupRead => fill1(Key::GapMirrorRead, &n),
        Say::BackupNever => t(Key::GapBackupNever).to_string(),
        Say::BackupBehind => fill1(Key::GapBackupBehind, &n),
        Say::AuditRun => fill1(Key::GapAuditRun, &n),
        Say::AuditGaps => fill1(Key::GapAuditGaps, &n),
        Say::AuditUnavailable => fill1(Key::GapAuditUnavailable, &n),
        Say::AuditBroken => fill1(Key::GapAuditBroken, &n),
        Say::HoldingRead => fill1(Key::GapHoldingRead, &n),
        Say::HoldingNoNow => fill1(Key::GapHoldingNoNow, &n),
        Say::HoldingExpired => fill1(Key::GapHoldingExpired, &n),
        Say::HoldingSoon => fill2(Key::GapHoldingSoon, &n, &days),
        Say::Revoked => fill1(Key::GapRevoked, &n),
        Say::Handed => fill1(Key::GapHanded, &n),
        Say::UpstreamRed => fill1(Key::GapUpstreamRed, &n),
    }
}

impl Win {
    /// What a task in flight is called: by its kind, except the fetch kind's flight while it checks the tail
    /// (that check is not a fetch, and is not called one).
    fn task_word(&self, k: crate::task::Kind) -> Key {
        if k == crate::task::Kind::Fetch && self.shell.fetch_checks_tail {
            Key::TaskCheckTail
        } else {
            task_key(k)
        }
    }
}

fn task_key(k: crate::task::Kind) -> Key {
    use crate::task::Kind;
    match k {
        Kind::SelfCheck => Key::TaskSelfCheck,
        Kind::Archive => Key::TaskArchive,
        Kind::Chain => Key::TaskChain,
        Kind::Reconcile => Key::TaskReconcile,
        Kind::Audit => Key::TaskAudit,
        Kind::Ledger => Key::TaskLedger,
        Kind::Anchor => Key::TaskAnchor,
        Kind::Depth => Key::TaskDepth,
        Kind::Kit => Key::TaskKit,
        Kind::Grants => Key::TaskGrants,
        Kind::Adopt => Key::TaskAdopt,
        Kind::Sighting => Key::TaskSighting,
        Kind::Book => Key::TaskBook,
        Kind::Diligence => Key::TaskDiligence,
        Kind::Verify => Key::TaskVerify,
        Kind::Delivery => Key::TaskDelivery,
        Kind::Review => Key::TaskReview,
        Kind::Held => Key::TaskHeld,
        Kind::Badge => Key::TaskBadge,
        Kind::Check => Key::TaskCheck,
        Kind::Keystore => Key::TaskKeystore,
        Kind::Vault => Key::TaskVault,
        Kind::Publish => Key::DoCheckPublished,
        Kind::Fetch => Key::DoFetchLedger,
        Kind::Vet => Key::TaskVet,
        Kind::Gate => Key::TaskGate,
        Kind::Backup => Key::WizBackupTitle,
        Kind::ReadNet => Key::TaskReadNet,
        Kind::Gas => Key::U3GasEstimate,
        Kind::Take => Key::TaskTake,
        Kind::Record => Key::TaskRecord,
        Kind::Migrate => Key::TaskMigrate,
    }
}

impl Win {
    /// Apply an action and tell the result as a toast; there is no silent branch. Returns its `Applied`, so
    /// the page whose key was pressed reads its own result instead of guessing from the shared shell.
    fn act(&mut self, a: Action, now: f64) -> Applied {
        // Clear the previous entry before opening another: stale details on screen would pose as current.
        if matches!(a, Action::OpenEntry { .. }) {
            self.opened = None;
        }
        let want = match &a {
            Action::OpenEntry { id } => Some(id.clone()),
            _ => None,
        };
        // The sequence number the next entry takes, read before the table goes stale (toasts name entries by
        // number).
        self.ux.next_seq = self.shell.rows.as_ref().map(|(r, _)| r.iter().map(|x| x.seq + 1).max().unwrap_or(0));
        // A long action records where it was started, so the rail can go back there and its landing can
        // say "view" when the person has left.
        if let Some(k) = task_of(&a) {
            self.ux.origin.retain(|(x, _, _)| *x != k);
            let h = self.hist().clone();
            self.ux.origin.push((k, self.ux.stack, h));
        }
        // Tell the troubles recorded before this press first; those this press records are told below.
        self.tell_faults(now);
        let applied = apply(&mut self.shell, a);
        // A new exit gate is the only one whose answer is awaited: a place recorded for an earlier gate no
        // longer waits (the place that pressed this one records itself, `vault_or`).
        if applied == Applied::Started(crate::task::Kind::Gate) {
            self.ux.gate_site = None;
        }
        self.told(applied, want, now)
    }

    /// The next entry's number as a sentence piece ("#12"), or empty when the table was not read.
    fn new_seq(&self) -> String {
        self.ux.next_seq.map(|n| format!("#{n}")).unwrap_or_default()
    }

    /// An entry named for a toast: "#seq" when the table knows it, else nothing longer than a short id.
    fn entry_say(&self, id: &str) -> String {
        self.shell
            .rows
            .as_ref()
            .and_then(|(rows, _)| rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).map(|r| format!("#{}", r.seq)))
            .unwrap_or_else(|| crate::ledgerx::short(id))
    }

    /// A record named by its content fingerprint: "#seq · name" when this ledger anchored it.
    fn work_seq_say(&self, work: &str) -> String {
        self.shell
            .rows
            .as_ref()
            .and_then(|(rows, _)| {
                rows.iter()
                    .find(|r| r.kind == zikaron::tokens::EntryType::History && r.work.as_deref().map(|w| w.eq_ignore_ascii_case(work)).unwrap_or(false))
                    .map(|r| format!("#{} · {}", r.seq, human_summary(r)))
            })
            .unwrap_or_else(|| t(Key::UnnamedRecord).to_string())
    }

    /// An identity's name as shown (its label, or "unnamed").
    fn id_name(&self, id: &str) -> String {
        self.shell
            .identities
            .as_ref()
            .and_then(|reg| reg.find(id))
            .map(|r| r.label.clone())
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| t(Key::Unnamed).to_string())
    }

    /// Tell one result (toast and the card's two sentences). Results done in the frame and passcode tasks
    /// landing (`vault_said`) both go here.
    fn told(&mut self, applied: Applied, want: Option<String>, now: f64) -> Applied {
        let (said, tone) = match applied.clone() {
            // Turning to a page is navigation: no toast.
            Applied::Shown(_) => (String::new(), Tone::Note),
            // A task the person started toasts once when it lands; while running only the rail mentions it.
            Applied::Started(k) => {
                self.ux.asked.push(k);
                (String::new(), Tone::Note)
            }
            Applied::Refused(k) => (fill1(Key::SaidInFlight, t(self.task_word(k))), Tone::Bad),
            Applied::Stopped(r) => (fill1(Key::SaidReaped, &r.joined.to_string()), Tone::Note),
            Applied::AnchorKey(_) => (t(Key::SaidAnchorKeyPlain).to_string(), Tone::Note),
            Applied::Seated(r) => (fill1(Key::SaidSeated, t(id_seat_key(r))), Tone::Note),
            // Reading identities, generating or wiping new words, showing or hiding: the result is on the
            // sheet, no toast.
            Applied::Identities(_) | Applied::FreshWords | Applied::WordsShown | Applied::WordsHidden => (String::new(), Tone::Note),
            // Unlock and lock get no toast (the gate itself changes); set, change and recovery each get one.
            Applied::Unlocked | Applied::LockedUp => (String::new(), Tone::Note),
            Applied::PinChanged => (t(Key::SaidPinChanged).to_string(), Tone::Note),
            Applied::AutoAnchor(on) => (fill1(Key::SaidAutoAnchor, t(if on { Key::On } else { Key::Off })), Tone::Note),
            Applied::HideLocalDeletions(on) => (fill1(Key::SaidHideLocalDeletions, t(if on { Key::On } else { Key::Off })), Tone::Note),
            Applied::NetworkChosen { name, .. } => (fill1(Key::SaidNetworkChosen, &network_label(&name)), Tone::Note),
            // Idle locking: "idle N minutes locks" when on or when the time changes, "auto-lock off" when off.
            Applied::AutoLockSet { on: true, secs } => (fill1(Key::SaidAutoLockOn, &(secs / 60).to_string()), Tone::Note),
            Applied::AutoLockSet { on: false, .. } => (t(Key::SaidAutoLockOff).to_string(), Tone::Note),
            Applied::PrimarySet { id } => (fill1(Key::SaidPrimarySet, &self.id_name(&id)), Tone::Note),
            Applied::BackupRestored(sm) => (
                format!("{} · {}", t(Key::SaidBackupRestored), backup_content(&sm)),
                Tone::Note,
            ),
            // Background resumptions and the catch-up after unlocking: their own results speak when they land.
            Applied::Resumed(_) | Applied::CaughtUp { .. } => (String::new(), Tone::Note),
            // The reset closes the gate and the wizard returns to step 1; a sentence is still said because a
            // file was deleted, and that must leave a trace on screen.
            Applied::KeyboxReset => (t(Key::SaidKeyboxReset).to_string(), Tone::Note),
            Applied::Recovered => (t(Key::SaidRecovered).to_string(), Tone::Note),
            Applied::FreshDropped => {
                self.ux.id_confirming = false;
                self.ux.id_confirm = Default::default();
                self.ux.id_words_open = false;
                self.ux.id_words_seen = false;
                (String::new(), Tone::Note)
            }
            Applied::IdentityMade { restored, .. } => {
                self.ux.id_confirming = false;
                self.ux.id_confirm = Default::default();
                self.ux.id_words_open = false;
                self.ux.id_words_seen = false;
                // Hitting an identity already in the registry restored the vault slot it was missing.
                let name = if self.ux.id_new_label.trim().is_empty() { t(Key::Unnamed).to_string() } else { self.ux.id_new_label.trim().to_string() };
                if !restored && self.shell.unfetched.is_some() {
                    // An identity restored from its secret: said on the spot that only what the chain holds
                    // comes back (its ledger is fetched from a whole-machine backup).
                    self.toasts.say_full(fill1(Key::SaidIdentityMade, &name), t(Key::ChainOnlyRestore), "", Tone::Note, now);
                    (String::new(), Tone::Note)
                } else {
                    (if restored { t(Key::SaidKeyRestoredPlain).to_string() } else { fill1(Key::SaidIdentityMade, &name) }, Tone::Note)
                }
            }
            // Clearing the name and renaming are two sentences.
            Applied::IdentityNamed { label: name, .. } => {
                if name.is_empty() {
                    (t(Key::SaidNameCleared).to_string(), Tone::Note)
                } else {
                    (fill1(Key::SaidRenamed, &name), Tone::Note)
                }
            }
            Applied::IdentitySwitched { id } => (fill1(Key::SaidIdentitySwitched, &self.id_name(&id)), Tone::Note),
            Applied::IdentityDeleted { .. } => (fill1(Key::SaidIdentityDeleted, &self.ux.id_deleting.take().unwrap_or_else(|| t(Key::Unnamed).to_string())), Tone::Note),
            Applied::KeyBackedUp { path, .. } => (fill1(Key::SaidKeyBackedUp, &folder_of(&path)), Tone::Note),
            Applied::FactsForgotten { .. } => (t(Key::SaidRecheckAll).to_string(), Tone::Note),
            Applied::Homed { root, mode } => (fill2(Key::SaidHomed, &folder_of(&root), t(if mode.writable() { Key::NoteWriter } else { Key::NoteReader })), Tone::Note),
            Applied::Migrated { root } => (fill1(Key::SaidMigrated, &folder_of(&root)), Tone::Note),
            Applied::Capped(n) => (fill1(Key::SaidCapped, &size_say(n)), Tone::Note),
            // Topping up an old bundle and writing a new one are two sentences.
            Applied::Mirrored { path, entries, added, topped_up } => (
                if topped_up { fill2(Key::SaidMirrorToppedUp, &added.to_string(), &folder_of(&path)) } else { fill2(Key::SaidMirrored, &entries.to_string(), &folder_of(&path)) },
                Tone::Note,
            ),
            Applied::Endpoints(n) => (fill1(Key::SaidEndpoints, &n.to_string()), Tone::Note),
            Applied::Genesised { .. } => {
                self.shell.stale_rows();
                // Genesis is written, so the wizard's statement is no longer needed.
                self.ux.wiz_statement.clear();
                (t(Key::SaidGenesisedPlain).to_string(), Tone::Note)
            }
            Applied::AdoptedInPlace { entries, linked, label } => (fill3(Key::SaidAdopted, &entries.to_string(), &linked.to_string(), &label_human(&label)), Tone::Note),
            Applied::Opened { .. } => {
                // Details are read from disk by the action layer (never in the frame); this only shows them.
                self.read_detail(want.as_deref());
                (String::new(), Tone::Note)
            }
            Applied::Annotated(_) => {
                let seq = self.new_seq();
                // An entry was written, so the table is stale: clear it to reread.
                self.shell.stale_rows();
                (fill1(Key::SaidAnnotated, &seq), Tone::Note)
            }
            Applied::Retracted { queued, local, .. } => {
                let seq = self.new_seq();
                self.shell.stale_rows();
                self.shell.stale_grants();
                if local {
                    // Never published: the pair stays local and is not anchored.
                    (fill1(Key::SaidRetractedLocal, &seq), Tone::Note)
                } else {
                    (fill2(Key::SaidRetracted, &seq, &queued.to_string()), Tone::Note)
                }
            }
            Applied::Basis { chain } => (fill1(Key::SaidBasis, &chain.to_string()), Tone::Note),
            Applied::Every(n) => (fill1(Key::SaidAuditEvery, &n.to_string()), Tone::Note),
            // The fingerprint is computed quietly: the chosen file shows only its name and size.
            Applied::Took { .. } => (String::new(), Tone::Note),
            Applied::Recorded { queued, .. } => {
                let seq = self.new_seq();
                self.shell.stale_rows();
                (fill2(Key::SaidQueued, &seq, &queued.to_string()), Tone::Note)
            }
            Applied::RecordedBatch { ids, stopped, queued, .. } => {
                self.shell.stale_rows();
                match stopped {
                    None => (fill2(Key::SaidBatch, &ids.len().to_string(), &queued.to_string()), Tone::Note),
                    // The stop sentence carries the reason (its raw text in the details); the same fault the
                    // action recorded is told here, not again as a toast of its own that would replace this one.
                    Some((i, p, f)) => {
                        self.faults_told = self.shell.faults.len();
                        let said = fill3(Key::SaidBatchStopped, &ids.len().to_string(), &(i + 1).to_string(), &format!("{} · {}", folder_of(&p), f.human()));
                        self.toasts.say_full(said, "", &f.raw(), Tone::Bad, now);
                        (String::new(), Tone::Bad)
                    }
                }
            }
            Applied::GrantCode { .. } => (String::new(), Tone::Note),
            Applied::KitLinked { link, .. } => (fill1(Key::SaidKitLinked, &link.unwrap_or_else(|| t(Key::KitNoLink).to_string())), Tone::Note),
            Applied::KitDropped { .. } => {
                self.ux.u3.drop_armed = None;
                (t(Key::SaidKitDroppedPlain).to_string(), Tone::Note)
            }
            Applied::Registered { path } => (fill1(Key::SaidRepo, &folder_of(&path)), Tone::Note),
            Applied::Since { grew, .. } => (fill1(Key::SaidSincePlain, &grew.map(|n| n.to_string()).unwrap_or_else(|| t(Key::RepoUnknown).to_string())), Tone::Note),
            Applied::Gas { gas, .. } => (fill1(Key::SaidGas, &gas.to_string()), Tone::Note),
            Applied::Picked { items, pulled } => (fill2(Key::SaidPicked, &items.to_string(), &pulled.to_string()), Tone::Note),
            Applied::Queued { id, queued, .. } => {
                let seq = self.entry_say(&id);
                self.shell.stale_rows();
                (fill2(Key::SaidQueued, &seq, &queued.to_string()), Tone::Note)
            }
            Applied::Granted { queued, .. } => {
                let seq = self.new_seq();
                self.shell.stale_rows();
                self.shell.stale_grants();
                self.ux.u4.relicense_signed = true;
                (fill2(Key::SaidGranted, &seq, &queued.to_string()), Tone::Note)
            }
            Applied::GrantFileExported { path, why, hops, terms, .. } => {
                // When the location differs from the choice, a second sentence naming where it went.
                let second = why.say().map(|k| fill1(k, &width::file_name(&path))).unwrap_or_default();
                self.toasts.say_full(fill3(Key::SaidGrantFile, &folder_of(&path), &hops.to_string(), &terms.to_string()), &second, "", Tone::Note, now);
                (String::new(), Tone::Note)
            }
            // The read-only network table changed: the list shows it, no toast.
            Applied::ReadNets(_) => (String::new(), Tone::Note),
            Applied::PublishSet { url } => match url {
                Some(u) => (fill1(Key::SaidPublishSet, &u), Tone::Note),
                None => (t(Key::SaidPublishCleared).to_string(), Tone::Note),
            },
            Applied::Clashed(n) => {
                // On an overlap, raise the sheet: a hard warning, not a quiet line.
                self.clash_modal = n > 0;
                (fill1(Key::SaidClashed, &n.to_string()), if n > 0 { Tone::Bad } else { Tone::Note })
            }
            Applied::Ticked { step, next } => (fill2(Key::SaidTicked, step, next.unwrap_or(t(Key::WizardDone))), Tone::Note),
            Applied::Restarted => (t(Key::SaidRestarted).to_string(), Tone::Note),
            Applied::Revoked { queued, .. } => {
                let seq = self.ux.u3.revoking.take().unwrap_or_default();
                self.shell.stale_rows();
                (fill2(Key::SaidRevoked, &seq, &queued.to_string()), Tone::Note)
            }
            Applied::Storied { grant, revocations } => (fill2(Key::SaidStoried, &self.entry_say(&grant), &revocations.to_string()), Tone::Note),
            // A claim was read: the result is on the sheet. Signed: the signature is on the sheet.
            Applied::ClaimRead => (String::new(), Tone::Note),
            Applied::Attested { .. } => (t(Key::SaidAttestedPlain).to_string(), Tone::Note),
            Applied::Cosigned { .. } => (t(Key::SaidCosignedPlain).to_string(), Tone::Note),
            Applied::Adopted { cosigned, .. } => {
                let seq = self.new_seq();
                self.shell.stale_rows();
                (fill2(Key::SaidAdopted2, &seq, t(if cosigned { Key::Yes } else { Key::NotYet })), Tone::Note)
            }
            Applied::Succeeded { queued, .. } => {
                self.shell.stale_rows();
                (fill1(Key::SaidSucceededPlain, &queued.to_string()), Tone::Note)
            }
            Applied::Snapshot { path, .. } => (fill1(Key::SaidSnapshotPlain, &folder_of(&path)), Tone::Note),
            Applied::HeldPartly { ids, refused } => {
                // Refused files are recorded one by one at the end of the shell's list; their reasons go into
                // this toast's details instead of one toast each.
                let from = self.shell.faults.len().saturating_sub(refused);
                let why = self.shell.faults[from..].iter().map(|f| format!("{} · {}", f.human(), f.raw())).collect::<Vec<_>>().join("\n");
                self.faults_told = self.shell.faults.len();
                self.toasts.say_full(fill2(Key::SaidHeldPartly, &ids.len().to_string(), &refused.to_string()), "", &why, Tone::Bad, now);
                (String::new(), Tone::Bad)
            }
            Applied::Held { ids, .. } => (fill1(Key::SaidHeldPlain, &ids.len().to_string()), Tone::Note),
            // Names typed with the grant go in with it: the add's own toast already said so.
            Applied::HeldNoted { .. } => (String::new(), Tone::Note),
            Applied::Upstream { grant } => {
                let issuer = self.held_issuer_name(&grant);
                (fill1(Key::SaidUpstream, &issuer), Tone::Note)
            }
            Applied::ReviewEvery(n) => (fill1(Key::SaidReviewEvery, &n.to_string()), Tone::Note),
            Applied::Spoken(l) => (fill1(Key::SaidLang, l.label()), Tone::Note),
            Applied::Zoned(z) => (fill1(Key::SaidZone, zone_label(z)), Tone::Note),
            Applied::Appeared(a) => (fill1(Key::SaidAppearance, appearance_label(&a)), Tone::Note),
            Applied::Booked { on, .. } => (t(if on { Key::SaidBookedOn } else { Key::SaidBookedOff }).to_string(), Tone::Note),
            Applied::Trouble(f) => {
                // When the double-sale guard hits, its sheet comes up (the refusal is CONFLICT).
                if f.which() == Some(crate::fault::Known::Conflict) {
                    self.clash_modal = true;
                }
                // Network faults do not go through `tell_faults`: this press is told here.
                if crate::watchx::is_network(&f) {
                    self.say_fault(&f, now);
                }
                (String::new(), Tone::Bad)
            }
        };
        // What comes after queueing, answered once: with auto-anchor on, estimate gas and show the
        // confirmation sheet; off, the toast adds "waiting in the ledger to be anchored by hand".
        let next = match &applied {
            Applied::Genesised { next, .. }
            | Applied::Recorded { next, .. }
            | Applied::RecordedBatch { next, .. }
            | Applied::Queued { next, .. }
            | Applied::Granted { next, .. }
            | Applied::Revoked { next, .. }
            | Applied::Adopted { next, .. }
            | Applied::Succeeded { next, .. }
            | Applied::Retracted { next, .. } => Some(*next),
            _ => None,
        };
        let out = match &applied {
            Applied::Mirrored { .. } => Some(Out::Mirror),
            Applied::Snapshot { .. } => Some(Out::Snapshot),
            _ => None,
        };
        let why = out.and_then(|x| self.landing_note(x));
        // Recorded while the chain cannot be read: one more line (anchoring checks against the chain first).
        let offline = matches!(applied, Applied::Recorded { .. } | Applied::Queued { .. }) && self.shell.status.is_some();
        if !said.is_empty() {
            if offline {
                self.toasts.say_full(said, t(Key::OfflineCheckFirst), "", tone, now);
            } else if next == Some(crate::action::Next::Wait) {
                self.toasts.say_full(said, t(Key::QueueWaitNext), "", tone, now);
            } else if let Some(w) = why {
                self.toasts.say_full(said, &w, "", tone, now);
            } else {
                self.toasts.say(said, tone, now);
            }
        }
        if next == Some(crate::action::Next::Send) {
            let n = self.shell.queue.sendable();
            self.u3_open_confirm(U3Confirm::Send { count: n });
        }
        // While an identity sheet is open, the refusal's two sentences stay on the sheet.
        if let Applied::Trouble(f) = &applied {
            if self.ux.id_modal.is_some() {
                self.ux.id_trouble = Some(f.clone());
            }
        }
        applied
    }

    /// When an action whose answer goes back to its place starts a background task (a passcode task, or an
    /// export's exit gate), record where it started; when it does not, answer that place at once.
    fn vault_or(&mut self, site: VaultSite, r: Applied, now: f64) {
        if r == Applied::Started(crate::task::Kind::Vault) {
            self.ux.vault_site = Some(site);
        } else if r == Applied::Started(crate::task::Kind::Gate) {
            self.ux.gate_site = Some(site);
        } else {
            self.vault_back(site, r, now);
        }
    }

    /// The answer to a passcode place (both immediate and landed in the background go here).
    fn vault_back(&mut self, site: VaultSite, r: Applied, now: f64) {
        match site {
            VaultSite::Wizard => match r {
                // Once set, wipe the digits in hand and stop the shake.
                Applied::Unlocked => self.ux.gate_clear(),
                // Refused: shake the row and clear both, so the next try starts over.
                Applied::Trouble(_) => {
                    self.ux.pin_shake = Some(now);
                    self.ux.pin.clear();
                    self.ux.pin_again.clear();
                }
                _ => {}
            },
            VaultSite::Change => match r {
                Applied::PinChanged => {
                    self.ux.gate_clear();
                    self.ux.id_close();
                }
                Applied::Trouble(f) => {
                    self.ux.pin_shake = Some(now);
                    self.ux.pin_again.clear();
                    self.ux.pin_old.clear();
                    self.ux.id_trouble = Some(f);
                }
                _ => {}
            },
            VaultSite::Gate => match r {
                Applied::Unlocked | Applied::Recovered | Applied::KeyboxReset => {
                    self.ux.gate_clear();
                }
                Applied::Trouble(f) => {
                    // One failure: shake the row; the sentence under the cells.
                    self.ux.pin_shake = Some(now);
                    self.ux.pin_again.clear();
                    // On the final failure the "wrong passcode" line is not written: the whole card turns into
                    // "locked · 5 wrong passcodes", and "0 tries left" would contradict it on the same frame.
                    let burnt = self.shell.vault.is(crate::keybox::State::LockedOut) && f.which() == Some(crate::fault::Known::PinWrong);
                    // Derivation parameters below the floor: the gate switches to the reseal path.
                    if f.which() == Some(crate::fault::Known::KdfBelowFloor) {
                        self.ux.gate_reseal = true;
                        self.ux.pin_shake = None;
                    }
                    self.ux.gate_trouble = if burnt {
                        None
                    } else if f.which() == Some(crate::fault::Known::PinWrong) {
                        // "Wrong passcode" carries the tries left (in the refusal's tail).
                        Some(fill1(Key::PinWrongLeft, f.tail()))
                    } else if f.which() == Some(crate::fault::Known::KdfBelowFloor) {
                        Some(format!("{} {}", f.human(), f.next()))
                    } else {
                        Some(f.human().to_string())
                    };
                }
                _ => {}
            },
            VaultSite::Words => match r {
                Applied::WordsShown => self.ux.pin_shake = None,
                Applied::Trouble(f) => {
                    self.ux.pin_shake = Some(now);
                    self.ux.id_trouble = Some(f);
                }
                _ => {}
            },
            VaultSite::Backup => self.bk_back(r, now),
            VaultSite::Attest => match r {
                Applied::Attested { .. } => {
                    self.ux.pin_shake = None;
                    self.ux.u3.attest_trouble = None;
                }
                Applied::Trouble(f) => {
                    self.ux.pin_shake = Some(now);
                    self.ux.u3.attest_trouble = Some(f);
                }
                _ => {}
            },
            VaultSite::Id => match r {
                Applied::Trouble(f) => {
                    self.ux.pin_shake = Some(now);
                    self.ux.id_trouble = Some(f);
                }
                Applied::FreshDropped
                | Applied::WordsHidden
                | Applied::IdentityMade { .. }
                | Applied::IdentitySwitched { .. }
                | Applied::IdentityDeleted { .. }
                | Applied::IdentityNamed { .. }
                | Applied::KeyBackedUp { .. }
                // Once the key backup's background task starts, close the sheet: encryption runs in the
                // background, the identity page says so, and the landing toasts.
                | Applied::Started(crate::task::Kind::Keystore) => self.ux.id_close(),
                _ => {}
            },
        }
    }

    /// Judge the first passcode entry's shape as soon as it has eight characters.
    ///
    /// Same closed table as the action layer (`keybox::pin_trouble`); the interface only says it earlier: a
    /// valid shape moves on to "enter again", an invalid one is refused at once (the row shakes, the cells
    /// clear). The action layer still judges; this only saves typing a second time.
    ///
    /// `inline` writes the sentence under the cells; otherwise a toast. Returns true when accepted.
    fn take_first_pin(&mut self, now: f64, inline: bool) -> bool {
        let pin = std::mem::take(&mut self.ux.pin);
        match crate::keybox::pin_trouble(pin.expose()) {
            None => {
                self.ux.pin_again = pin;
                true
            }
            Some(why) => {
                self.ux.pin_shake = Some(now);
                let f = crate::fault::Fault::known(crate::fault::Known::PinShape, why.as_str().to_string());
                // All-same, sequential and date-like passcodes get "too simple"; a wrong shape the rules.
                let said = t(if why == crate::keybox::PinTrouble::Shape { Key::PinRules } else { Key::PinTooSimple });
                if inline {
                    self.ux.gate_trouble = Some(said.to_string());
                } else {
                    self.toasts.say_full(said.to_string(), "", &f.raw(), Tone::Bad, now);
                }
                false
            }
        }
    }

    fn say_fault(&mut self, f: &crate::fault::Fault, now: f64) {
        let (what, next) = self.import_face(f).unwrap_or((f.human(), f.next()));
        let (what, mut next) = (what.to_string(), next.to_string());
        if f.then_key() == Some(Key::ExitBehindSay) {
            next = fill1(Key::ExitBehindSay, f.tail());
        }
        // With no passcode on this machine yet, "enter the passcode to unlock" cannot be done: set one first.
        if f.which() == Some(crate::fault::Known::Locked) && self.shell.vault.absent() {
            next = t(Key::FaultNextNoPinYet).to_string();
        }
        // Insufficient balance says what is needed and what there is, and offers "copy address" (the address
        // to fund is this signing address).
        let mut act: Option<(String, String)> = None;
        if f.which() == Some(crate::fault::Known::InsufficientFunds) {
            if let Some((need, have)) = crate::action::funds_of(f.tail()) {
                next = fill2(Key::FundsNeedHave, &eth_cap(need), &eth_held(have));
            }
            act = self.shell.anchor.map(|a| (t(Key::IdDoCopy).to_string(), a.hex()));
        }
        self.toasts.say_with(what, &next, &f.raw(), Tone::Bad, now, act);
    }

    /// Troubles are told when they happen. Each recorded and untold trouble becomes a toast with details;
    /// network faults only reach the rail's status line. This is the raw text's only outlet.
    fn tell_faults(&mut self, now: f64) {
        let n = self.shell.faults.len();
        if self.faults_told > n {
            self.faults_told = n;
        }
        // While the gate is up, "the vault is locked" gets no toast: the gate says the same. Nothing is said
        // as a toast about the gate at all: its errors are said under its cells.
        let shut = self.shell.vault.gate_up();
        for i in self.faults_told..n {
            let f = self.shell.faults[i].clone();
            let locked = f.which() == Some(crate::fault::Known::Locked);
            let pin = matches!(f.which(), Some(crate::fault::Known::PinWrong));
            let exit_unread = f.then_key() == Some(Key::ExitRetryLater);
            if (crate::watchx::is_network(&f) && !exit_unread) || (shut && (locked || pin)) {
                continue;
            }
            self.say_fault(&f, now);
        }
        self.faults_told = n;
    }

    /// Text pasted into the add-grant sheet: when read as a path that does not exist, the screen says the
    /// content is not recognized, with the reason folded into the error details.
    fn import_face(&self, f: &crate::fault::Fault) -> Option<(&'static str, &'static str)> {
        let typed = self.typed.vt_typed.trim();
        let not_a_file = f.which() == Some(crate::fault::Known::FileMissing) && !std::path::Path::new(typed).exists();
        (self.ux.u4.import_open && not_a_file).then(|| (t(Key::V2ImportUnreadable), t(Key::V2ImportUnreadableNext)))
    }
}

/// The task a press starts, for actions that run in the background (their keys run the long-action phases).
fn task_of(a: &Action) -> Option<crate::task::Kind> {
    use crate::task::Kind;
    Some(match a {
        Action::CheckPayload { .. } => Kind::Check,
        Action::VerifyWork { .. } => Kind::Verify,
        Action::ReadBook { .. } => Kind::Book,
        Action::Diligence { .. } => Kind::Diligence,
        Action::ExportKit { .. } => Kind::Kit,
        Action::ExportBadge { .. } => Kind::Badge,
        Action::ExportGrantFile { .. } | Action::ExportMirror { .. } => Kind::Gate,
        Action::ReviewVault => Kind::Review,
        Action::Reconcile => Kind::Reconcile,
        Action::Measure => Kind::Archive,
        Action::ReadChain => Kind::Chain,
        Action::ReadReadNetwork { .. } => Kind::ReadNet,
        Action::SelfCheck => Kind::SelfCheck,
        Action::CheckPublished { .. } => Kind::Publish,
        Action::ReadDepth { .. } => Kind::Depth,
        Action::SendBatch { .. } => Kind::Anchor,
        Action::FetchLedger { .. } => Kind::Fetch,
        Action::FetchAside { .. } => Kind::Fetch,
        _ => return None,
    })
}

/// Where the window starts.
pub enum Start {
    /// An old page (a test driver's `show <page>`), landing on its current place (`nav::home_of`).
    Page(Page),
    /// A place; true in the second field opens the wizard at start (a test driver's `show Wizard`).
    Place(Place, bool),
}

/// Layout readings of one face at a given viewport (read by a test driver's `page --viewport`; the judgment
/// is elsewhere).
pub struct FaceReading {
    /// Viewport width.
    pub width: f32,
    /// The rightmost edge of every widget rectangle this frame (whole rectangles, not clipped).
    pub right_widget: f32,
    /// The rightmost edge of every text shape this frame (not clipped).
    pub right_text: f32,
    /// Clickable rows on a list page (recorded by `zikaron_ui::probe`).
    pub rows: usize,
    /// Whether clicking the first row drew a detail page (`None` without rows).
    pub inner: Option<bool>,
    /// The rightmost edge of the detail page (widgets and text; `None` without one).
    pub inner_right: Option<f32>,
    /// The detail page's title (`None` without one).
    pub inner_head: Option<String>,
    /// Whether the detail page drew keys in its toolbar besides back and forward (`None` without one).
    pub inner_head_keys: Option<bool>,
    /// The detail page's history keys: back and forward shown (`None` without one).
    pub inner_history: Option<(bool, bool)>,
    /// The route the face drew, and the detail page's route after clicking the first row.
    pub route: String,
    pub inner_route: Option<String>,
    /// Every string the settled face drew (before any click).
    pub texts: Vec<String>,
    /// This frame's equal-grid cells, one per cell.
    pub cells: Vec<CellFit>,
    /// How many text shapes this frame were justified.
    pub justified_texts: usize,
    /// This frame's dashboard tile rectangles, and whether each title fit.
    pub tiles: Vec<(egui::Rect, bool)>,
}

/// Whether a cell's text fits (a test driver's `page --viewport` counts by it; the judgment is
/// elsewhere).
pub struct CellFit {
    pub width: f32,
    /// Text shapes starting in this cell that are wider than the cell or pass its right edge.
    pub over: usize,
    /// Text shapes in this cell that wrap to more than one line.
    pub wrapped: usize,
}

/// One row of the records list.
struct WorkLine {
    name: String,
    work: String,
    id: String,
    seq: u64,
    lamp: crate::ledgerx::Lamp,
    /// The deletion read by the convention: the retraction's seq.
    deleted: Option<u64>,
    row: crate::ledgerx::Row,
}

/// Interface-side state of the window. Lives only on the interface side; not one byte goes to disk.
#[derive(Default)]
struct Ux {
    /// Where the wizard was opened from (seat, stack, place): its way out returns there.
    wiz_from: Option<(crate::roles::Role, Stack, Option<Place>)>,
    /// The wizard was opened on a true first run (no passcode and no identity at that moment): it offers no
    /// way out for the whole run, whatever its steps set meanwhile.
    wiz_fresh: bool,
    /// The wizard's "exit?" card is up (new words not yet checked).
    wiz_exit_ask: bool,
    /// The backup sheet open now, its step, and what was typed on it.
    bk: Option<Bk>,
    bk_step: BkStep,
    bk_path: String,
    bk_pw: crate::secret::Secret,
    bk_pw2: crate::secret::Secret,
    bk_pin: crate::secret::Secret,
    bk_dir: String,
    bk_trouble: Option<crate::fault::Fault>,
    /// The backup password for fetching a restored identity's ledger.
    fetch_pw: crate::secret::Secret,
    /// The backup password of the last fetch, kept until fetching answers: a conflict's "fetch and replace"
    /// uses it (the card asks nothing more).
    fetch_held: crate::secret::Secret,
    /// Where each writer decided to land when pressed (and why it differs from the choice).
    landings: Vec<(Out, crate::home::Chosen)>,
    /// The text in each list page's search field (per face; interface only).
    search: std::collections::BTreeMap<&'static str, String>,
    /// The date range under each list's search field: start and end day, `YYYY-MM-DD` or empty (per face;
    /// interface only).
    range: std::collections::BTreeMap<&'static str, (String, String)>,
    /// The current place. `None` until settled (the first frame follows `shell.page`).
    place: Option<Place>,
    /// Which view the person is in, and each view's history of pages.
    stack: Stack,
    hist: std::collections::BTreeMap<Stack, History>,
    /// How the page on screen entered, and a counter that changes with every page change (entrances key on
    /// it).
    entry: motion::Entry,
    entry_key: u64,
    /// The page's body scroll offset last frame (the toolbar's hairline and detail titles follow it).
    scroll_y: f32,
    /// Whether the wizard is open, and at which step.
    wizard: Option<crate::nav::Step>,
    /// Whether this start has asked "open the wizard by itself" yet.
    wizard_asked: bool,
    /// A test driver asked to open the wizard.
    wizard_forced: bool,
    /// The confirmation sheet after the key that writes genesis.
    confirm_genesis: bool,
    /// Whether the language from settings has been applied.
    lang_applied: bool,
    /// This frame's interface time.
    now: f64,
    /// Inputs opened in settings sections.
    open_home: bool,
    open_grant_dir: bool,
    open_nodes: bool,
    /// The settings publication field was filled once from the settings file.
    publish_seeded: bool,
    open_cadence: bool,
    /// Background tasks the person started (one per kind, may repeat). Only these toast when they land.
    asked: Vec<crate::task::Kind>,
    /// How each kind of task last landed: whether it went well, and when (long keys read it).
    landed: std::collections::BTreeMap<crate::task::Kind, (bool, f64)>,
    /// Where each long task was started: its view and that view's history then.
    origin: Vec<(crate::task::Kind, Stack, History)>,
    /// The sync key's round: the kinds still out, what has landed of it, and when it last landed well.
    syncing: Vec<crate::task::Kind>,
    sync_said: Vec<Result<Done, crate::fault::Fault>>,
    synced_at: Option<f64>,
    /// The next entry's sequence number, read before a write (toasts name entries by number).
    next_seq: Option<u64>,
    /// Wizard and settings inputs.
    wiz_statement: String,
    /// The network row chosen in the wizard's network step.
    wiz_network: Option<String>,
    /// The gas step: "I have sent it" was pressed and its reading of the chain not yet looked at; when the
    /// reading showed a balance (the step goes on a second later); the reading showed none.
    /// After "I have sent it": the chain reading's landing count at that press (the answer is a landing after
    /// it, never one that was already there).
    wiz_gas_asked: Option<u64>,
    wiz_gas_seen: Option<f64>,
    wiz_gas_not_seen: bool,
    audit_every: String,
    review_every: String,
    basis_chain: String,
    basis_registry: String,
    basis_from: String,
    /// Which preset filled the network editor's cells (0: by hand; see `settings::preset_menu`).
    basis_preset: usize,
    /// The read-only networks being edited on the settings' network page.
    readnet: settings::ReadNetUx,
    /// State of the author pages.
    u3: U3,
    u4: U4,
    // Identity sheets.
    id_modal: Option<IdModal>,
    id_tab: usize,
    /// The twelve cells on the import sheet (same as gate recovery and the wizard).
    id_words: [crate::secret::Secret; 12],
    /// Which seat to import into. Unset means the current seat.
    id_seat: Option<crate::roles::Role>,
    /// What was typed on the rename sheet.
    id_label: String,
    /// The note field for a new or imported identity.
    id_new_label: String,
    /// The network chosen on the new-identity or import sheet; unset means the one selected first
    /// (`machine::pick`).
    id_network: Option<String>,
    /// The identity switcher row currently expanded (by id).
    id_switch_open: Option<String>,
    /// The name of the identity being deleted (for its toast).
    id_deleting: Option<String>,
    /// Two readings taken when the delete sheet opens: which seats' ledgers already hold entries, and
    /// whether the backup file is on disk now. Both walk the disk, so they are read when the sheet opens.
    id_delete_ledgers: Vec<crate::roles::Role>,
    id_delete_backup: Option<crate::identity::BackupSeen>,
    /// Where a move would land: computed once when the directory is chosen.
    migrate_landing: Option<crate::home::Chosen>,
    id_hex: crate::secret::Secret,
    id_ks_path: String,
    id_ks_pw: crate::secret::Secret,
    id_confirm: [crate::secret::Secret; 3],
    id_confirming: bool,
    /// Whether the new twelve words are revealed now (masked by default).
    id_words_open: bool,
    /// Whether the new words were revealed at all.
    id_words_seen: bool,
    id_pw: crate::secret::Secret,
    id_pw2: crate::secret::Secret,
    id_dir: String,
    id_trouble: Option<crate::fault::Fault>,
    id_auth_waiting: bool,
    /// The local passcode cells on the export and delete sheets (taken when used).
    id_pin: crate::secret::Secret,
    /// The digits typed in the gate and other passcode cells (memory only; taken when used).
    pin: crate::secret::Secret,
    pin_again: crate::secret::Secret,
    pin_old: crate::secret::Secret,
    /// The gate is on the recovery path.
    gate_recover: bool,
    /// The error line under the gate's cells.
    gate_trouble: Option<String>,
    pin_words: [crate::secret::Secret; 12],
    /// Whether the gate's twelve cells are complete now.
    gate_words_ready: bool,
    /// Whether the key file path and password are complete.
    gate_file_ready: bool,
    pin_ks_path: String,
    pin_ks_pw: crate::secret::Secret,
    /// The frame time of a wrong entry: the row shakes.
    pin_shake: Option<f64>,
    /// Where a passcode task started.
    vault_site: Option<VaultSite>,
    /// Where the export whose exit gate is in flight was pressed, when its answer goes back to that place.
    gate_site: Option<VaultSite>,
    /// The gate is on the reseal path.
    gate_reseal: bool,
    /// The last human input (interface clock, seconds; the idle lock reads it).
    last_input: f64,
    /// When a sheet refused a press (it shakes).
    sheet_shake: Option<f64>,
    /// Sizes of chosen files, read once per path.
    sizes: std::collections::BTreeMap<String, Option<u64>>,
    /// The file names each record was signed from (the local records index), read once per entry.
    files_of: std::collections::BTreeMap<String, Vec<String>>,
}

/// Where a passcode task started. Closed: when it lands, each member returns the answer to its place.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum VaultSite {
    /// Setting the passcode in the wizard's first step.
    Wizard,
    /// Changing the passcode (its sheet).
    Change,
    /// The passcode gate (unlock, both recoveries, reseal).
    Gate,
    /// The show-words sheet.
    Words,
    /// Identity sheets (delete identity, export key file).
    Id,
    /// The attest-for-someone sheet.
    Attest,
    /// The whole-machine backup sheets.
    Backup,
}

/// Identity sheets. Closed.
#[derive(Clone, PartialEq, Eq, Debug)]
enum IdModal {
    New,
    Import,
    Words,
    Switch,
    Delete(String),
    Backup,
    /// Name an identity (nothing reads the name).
    Name(String),
    /// Change the passcode.
    Pin,
    /// Make this identity the primary one.
    Primary(String),
}

impl Ux {
    /// Open an identity sheet (clearing the previous one's refusal).
    fn id_open(&mut self, m: IdModal) {
        self.id_trouble = None;
        self.id_modal = Some(m);
    }

    /// After the gate opens, clear what was typed in its cells (passcode and words never outlive the frame in
    /// interface state).
    fn gate_clear(&mut self) {
        self.pin.clear();
        self.pin_again.clear();
        self.pin_old.clear();
        for w in self.pin_words.iter_mut() {
            w.clear();
        }
        self.pin_ks_path.clear();
        self.pin_ks_pw.clear();
        self.pin_shake = None;
        self.gate_recover = false;
        self.gate_reseal = false;
        self.gate_trouble = None;
        self.gate_words_ready = false;
        self.gate_file_ready = false;
    }

    /// "Back" on the recovery panel: return to the unlock card and clear the twelve cells, the key file
    /// password and both new passcode cells (wiped inside the secret type).
    fn gate_back(&mut self) {
        for w in self.pin_words.iter_mut() {
            w.clear();
        }
        self.pin_ks_pw.clear();
        self.pin.clear();
        self.pin_again.clear();
        self.gate_recover = false;
        self.gate_trouble = None;
        self.gate_words_ready = false;
        self.gate_file_ready = false;
    }

    fn id_close(&mut self) {
        self.id_modal = None;
        self.id_trouble = None;
        self.id_auth_waiting = false;
        self.id_confirming = false;
        self.id_confirm = Default::default();
        self.id_words_open = false;
        self.id_words_seen = false;
        for w in self.id_words.iter_mut() {
            w.clear();
        }
        self.id_label.clear();
        self.id_new_label.clear();
        self.id_network = None;
        self.id_hex.clear();
        self.id_ks_pw.clear();
        self.id_pw.clear();
        self.id_pw2.clear();
        self.id_pin.clear();
        // The show-words and change-passcode sheets also have passcode rows, cleared when the sheet closes.
        self.pin.clear();
        self.pin_again.clear();
        self.pin_old.clear();
        self.pin_shake = None;
    }

    /// Whether a landing should toast: when it is a kind the person started, consume one and answer yes.
    fn claim(&mut self, k: crate::task::Kind) -> bool {
        match self.asked.iter().position(|x| *x == k) {
            Some(i) => {
                self.asked.remove(i);
                true
            }
            None => false,
        }
    }
}

/// Which commit a confirmation sheet is waiting on.
#[derive(Clone, Debug, PartialEq)]
enum U3Confirm {
    /// Delete a work record (the retraction convention): the `history` entry's id.
    Retract { subject: String },
    /// Send: the first few queued entries go out as one batch (gas estimate shown first).
    Send { count: usize },
    /// Gas could not be estimated: say so, with a retry that estimates the same batch again.
    NoEstimate { count: usize, why: String, next: String, raw: String },
    /// Issue a grant.
    Grant,
    /// Revoke a grant.
    Revoke { grant: String },
}

/// The sheets under the ledger's "more".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum U3Form {
    Adopt,
    /// Key rotation or handover: the form, then its confirmation step.
    Succeed,
    Annotate,
    /// Attest for someone.
    Attest,
}

/// Interface state of the author pages. Lives only on the interface side.
#[derive(Default)]
struct U3 {
    confirm: Option<U3Confirm>,
    form: Option<U3Form>,
    /// The succession sheet is on its confirmation step.
    succeed_confirm: bool,
    /// The batch whose gas the open send sheet is still to estimate, and when that sheet opened (the node is
    /// asked once the sheet has come up).
    send_after_estimate: Option<usize>,
    estimate_from: f64,
    /// The batch a gas estimate is out for (`Kind::Gas`); its answer lands in `shell.said`.
    estimating: Option<usize>,
    /// The files a record in the background is writing (`Kind::Record`): what stays in the form when it stops.
    recording: Option<Vec<String>>,
    /// "Confirm and send" was pressed on the send sheet: the anchoring task's landing count then (the sheet
    /// closes on a landing after it, never on one already there).
    sending: Option<u64>,
    led_filter: usize,
    audit_open: bool,
    /// Grant drafting: validity preset (days; zero means custom).
    grant_days: u32,
    grant_days_set: bool,
    /// The new-record sheet: open, and on which step (0 the form, 1 the confirmation).
    new_anchor: Option<u8>,
    others_tab: usize,
    /// Which entry the detail page read bytes for (each tried once).
    opened_tried: Option<String>,
    /// Grant check page: the refusal returned when the check key was pressed.
    check_refused: Option<crate::fault::Fault>,
    /// Grant check page: which hop's "change…/add…" is open (the issuer ledger field).
    ck_source_open: Option<usize>,
    /// The export page kit (by path) whose "delete local copy" was pressed once and awaits confirmation.
    drop_armed: Option<String>,
    /// Text of each kit's link field on the export page (kit path, text, edited or not).
    link_typed: Vec<(String, String, bool)>,
    revoke_case: String,
    /// The grant being revoked ("#n"), for its toast.
    revoking: Option<String>,
    /// Disclosure kit: the key made from the pick fields and the table size, and that pick's reading.
    kit_preview: Option<(String, Result<(Vec<String>, Vec<String>), String>)>,
    /// Kit name preview for the attachment list.
    kit_names: Option<(String, Vec<(String, Result<Option<(String, bool)>, String>)>)>,
    /// The chosen records' originals: the pick key and the list `kitx::originals` built.
    kit_orig: Option<(String, Result<Vec<crate::kitx::Listed>, String>)>,
    /// Originals the person removed (by path); the rest travel with the kit.
    kit_orig_off: std::collections::BTreeSet<String>,
    /// This pick's originals gate (same key as the list).
    kit_admit: Option<(String, Option<crate::kitx::Originals>)>,
    /// The pick list's working state while open (written back only on "done").
    kit_pick: Option<KitPick>,
    /// Adoption: which text the per-row check ran against.
    adopt_checked: Option<String>,
    /// Key rotation or handover: the address already scanned automatically.
    sighting_asked: Option<String>,
    /// Import existing anchors: the key already listed (empty means this key).
    adopt_asked: Option<String>,
    /// Import existing anchors: rows the person switched off (tx|digest); all on by default.
    adopt_off: std::collections::BTreeSet<String>,
    /// Import existing anchors: paste was pressed; the next frame's paste event goes into the signature
    /// field.
    sig_paste: bool,
    /// Attest for someone: the claim text already read.
    at_read: Option<String>,
    /// Attest for someone: the refusal when the text could not be read or signing was refused.
    attest_trouble: Option<crate::fault::Fault>,
    /// The terms file of a new grant (dropped or chosen; its fingerprint fills the advanced field).
    terms: Option<TermsFile>,
}

/// Entries named in the pick list now (by id; an empty list chose nothing).
struct KitPick {
    ids: Vec<String>,
}

/// The terms file of a new grant: file name, size, fingerprint, and the disk path (read at signing to keep a
/// copy).
struct TermsFile {
    path: String,
    name: String,
    size: Option<u64>,
    hex: String,
}

/// The two optional files of the check page. Closed set.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OneStop {
    File,
    Terms,
}

impl OneStop {
    fn title(self) -> Key {
        match self {
            OneStop::File => Key::U3File,
            OneStop::Terms => Key::OsTerms,
        }
    }

    fn salt(self) -> &'static str {
        match self {
            OneStop::File => "check-file",
            OneStop::Terms => "check-terms",
        }
    }
}

/// State of the grantee pages (kept apart from the author pages).
#[derive(Default)]
struct U4 {
    /// My grants: grouped by issuer.
    vault_by_issuer: bool,
    /// The file changed while verifying: verify that one again when this pass ends.
    verify_again: bool,
    /// Whether the add-grant sheet is open.
    import_open: bool,
    /// The last add-grant failure's two sentences and raw error.
    import_err: Option<(String, String, String)>,
    /// The unpacked grant file from the add-grant sheet (the text, its reading).
    grant_file_seen: Option<(String, Option<Result<String, crate::fault::Fault>>)>,
    /// The confirmation sheet after a cinnabar key.
    confirm: Option<U4Confirm>,
    /// A relicense was signed on this page: it offers "put on chain".
    relicense_signed: bool,
    /// Record verification runs as soon as its page opens (a kit dropped on home or on the window).
    verify_autorun: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum U4Confirm {
    /// Relicense: the upstream grant. After confirmation, go draft the relicense.
    Relicense { grant: String },
}

mod backup;
mod check;
mod chrome;
mod common;
mod faces;
mod gate;
mod grants;
mod home;
mod identity;
mod kit;
mod ledger;
mod modals;
mod others;
mod settings;
mod shared;
mod vault;
mod verify;
mod watch;
mod wizard;
mod works;
pub use self::faces::*;
use self::chrome::stagger;
use self::common::*;
use self::others::*;
use self::shared::*;
use self::wizard::*;

/// What a backup holds, in one line: identities, ledger entries, records, settings.
pub(super) fn backup_content(s: &crate::backup::Summary) -> String {
    crate::lang::filln(Key::BackupContent, &[&s.identities.to_string(), &s.entries.to_string(), &s.records.to_string()])
}

#[cfg(test)]
mod tests {
    /// Every kind of task is called by its own name in the side bar and the task list: no kind borrows another
    /// kind's words (a borrowed name tells the person a different task is running).
    #[test]
    fn every_task_kind_has_its_own_name() {
        let mut seen: Vec<super::Key> = Vec::new();
        for k in crate::task::Kind::ALL {
            let key = super::task_key(k);
            assert!(!seen.contains(&key), "{} borrows another kind's name", k.as_str());
            seen.push(key);
        }
    }
}
