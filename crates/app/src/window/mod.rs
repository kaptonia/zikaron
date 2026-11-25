use crate::action::{apply, Action, Applied};
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
use zikaron_ui::{card, datepick, drop, fields, fold, full, input, mark, menu, motion, page, paint, pick, pin, rail, seg, sheet, skin, states, table, toggle, width};

pub const W: f32 = 1180.0;
pub const H: f32 = 760.0;
pub const MIN_W: f32 = zikaron_ui::tokens::RAIL_W + 2.0 * zikaron_ui::tokens::PAGE_PAD + zikaron_ui::tokens::MAIN_MIN_W;
pub const MIN_H: f32 = 560.0;

#[derive(Default)]
struct Typed {
    proxy: crate::proxycells::Cells,
    home: String,
    migrate: String,
    adopt: String,
    grant_dir: String,
    endpoints: String,
    rpc_new: String,
    rest_base: String,
    rest_key: crate::secret::Secret,
    mirror_out: String,
    annotate_subject: String,
    annotate_note: String,
    work_note: String,
    batch_files: Vec<String>,
    for_fields: [String; 4],
    fetch_from: String,
    repo_path: String,
    pick_from: String,
    pick_ids: String,
    pick_to: String,
    kit_attach: String,
    kit_note: String,
    kit_out: String,
    kit_to: String,
    kit_to_key: String,
    badge_out: String,
    g_grantee: String,
    g_work: String,
    g_terms: String,
    gf_out: String,
    gf_original: bool,
    gf_listed: String,
    gf_sealed: bool,
    publish: String,
    publish_local: String,
    g_history: String,
    g_from: String,
    g_to: String,
    g_start_day: String,
    g_start_at: [String; 2],
    g_lasts: [String; 3],
    g_scope: String,
    g_upstream: String,
    g_exclusive: bool,
    rd_address: String,
    rd_dir: String,
    dg_address: String,
    dg_dir: String,
    dg_work: String,
    dg_from: String,
    dg_to: String,
    dg_snapshot: String,
    extract_to: String,
    vf_path: String,
    vf_work: String,
    vt_typed: String,
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
    ck_file: String,
    ck_terms: String,
}

struct Win {
    shell: Shell,
    toasts: Toasts,
    typed: Typed,
    reaped: Option<crate::task::Reaped>,
    opened: Option<crate::ledgerx::Detail>,
    last_tick: f64,
    clash_modal: bool,
    clash_key: String,
    faults_told: usize,
    card_follows: bool,
    ux: Ux,
    asker: crate::platform::Asker,
    asks_which: bool,
    backdrop: Option<(zikaron_os::Window, bool, bool)>,
    asks_motion: bool,
    reads_behind: bool,
    reduced: bool,
    motion_asked: Option<f64>,
    painted_lang: crate::lang::Lang,
    drops: Option<crate::platform::drops::Watcher>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Out {
    Kit,
    Badge,
    Mirror,
    Snapshot,
}

const OLD_DATA_TAG: u64 = u64::MAX;

const CLI_SWITCHES_TAG: u64 = u64::MAX - 1;

pub const ROT_READ_EVERY: f64 = 15.0;

impl Win {
    fn draw(&mut self, ui: &mut egui::Ui) {
        self.turn(ui.ctx());
        self.paint(ui);
    }

    fn turn(&mut self, ctx: &egui::Context) {
        let _logic = zikaron_ui::motion::Logic::begin(ctx);
        let _road = crate::reads::Road::begin();
        let now = zikaron_ui::motion::now(ctx);
        self.land(ctx, now);
        if self.shell.rows_wanted.swap(false, std::sync::atomic::Ordering::Relaxed) {
            self.ensure_rows(now);
        }
        for a in std::mem::take(&mut self.shell.door_told) {
            self.told(a, None, now);
        }
        if let Some((tx, cap)) = self.ux.u3.bump_press.clone() {
            if !self.shell.tasks.in_flight(crate::task::Kind::Anchor) {
                self.ux.u3.bump_press = None;
                let offer = self.shell.stuck.as_ref().filter(|s| s.txs.last() == Some(&tx)).map(|s| s.offer.clone());
                match offer {
                    Some(crate::task::Offer::Resend { fees, .. } | crate::task::Offer::Unheld { fees, .. }) if fees.max_fee <= cap => self.bump_go(tx, cap, now),
                    Some(crate::task::Offer::Resend { .. } | crate::task::Offer::Unheld { .. }) => {}
                    _ => self.ux.u3.confirm = None,
                }
            }
        }
        if self.shell.queue.submitted().is_empty() {
            self.shell.resume_blocked = None;
        } else if self.ux.u3.bump_press.is_some() {
        } else if now - self.shell.resumed_at > 15.0 {
            self.shell.resumed_at = now;
            if crate::action::held_back(&self.shell, &Action::Resume).is_none() {
                let _ = crate::action::apply(&mut self.shell, Action::Resume);
            }
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((15.0 - (now - self.shell.resumed_at)).max(0.5)));
        }
        set_receipts_stalled(self.shell.resume_blocked.is_some());
        if ctx.input(|i| !i.events.is_empty() || i.pointer.delta() != egui::Vec2::ZERO) {
            self.ux.last_input = now;
        }
        if self.shell.idle_tick(self.ux.last_input, now) {
            self.ux.gate_clear();
            self.sheets_clear();
            self.ux.reroot = Default::default();
        }
        if self.shell.unlocked() {
            let due = self.ux.last_input + self.shell.machine.auto_lock_secs as f64 - now;
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(due.max(0.5)));
        }
        let quiet = self.ux.wizard.is_some();
        let mut rang: Vec<String> = Vec::new();
        if self.ux.wizard_asked {
            rang = self.own_rang();
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
        if self.ux.wizard_asked && !quiet {
            if let Some(was) = crate::home::moved_unsaid() {
                self.toasts.say(fill1(Key::MachineMovedFrom, &was.display().to_string()), Tone::Note, now);
            }
        }
        if std::mem::take(&mut self.shell.attention) && !quiet {
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(egui::UserAttentionType::Critical));
        }
        self.own_turn(ctx, now);
        self.tick(ctx, now);
        self.keep_draining(ctx);
    }

    fn keep_draining(&self, ctx: &egui::Context) {
        if self.shell.tasks.polled() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn paint(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let now = zikaron_ui::motion::now(&ctx);
        if self.asks_motion && self.motion_asked.map(|at| now < at || now - at > 2.0).unwrap_or(true) {
            self.motion_asked = Some(now);
            self.reduced = zikaron_os::reduce_motion();
        }
        zikaron_ui::motion::set_reduced(&ctx, self.reduced);
        self.draw_frame(ui);
        if ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::new("zikaron-copied"))).is_some() {
            self.toasts.say(t(Key::SaidCopied), Tone::Note, now);
        }
        let spoke = crate::lang::lang();
        if spoke != self.painted_lang {
            self.painted_lang = spoke;
            zikaron_ui::flow::retext(&ctx);
        }
        zikaron_ui::hand::finish(&ctx);
    }

    fn draw_frame(&mut self, ui: &mut egui::Ui) {
        let ctx = &ui.ctx().clone();
        zikaron_ui::probe::begin(ctx);
        let now = zikaron_ui::motion::now(&ctx);
        let blend = skin::follow(ctx, self.appearance());
        let follows = self.appearance() == skin::Appearance::System;
        if let Some((w, dark, followed)) = self.backdrop.as_mut() {
            if (blend > 0.5) != *dark || follows != *followed {
                (*dark, *followed) = (blend > 0.5, follows);
                zikaron_os::backdrop_appearance(*w, *dark, *followed);
            }
        }
        self.shortcuts(ctx, now);
        self.chrome(ui, now);
        self.paths_ask(ctx);
        self.tell_faults(now);
        self.toasts.set_labels(t(Key::ToastDetail), t(Key::ToastHideDetail), t(Key::ToastClose));
        let left = if self.rail_shown() { tk::RAIL_W } else { 0.0 };
        let drawn = self.toasts.draw(ctx, left);
        if let Some(tag) = drawn.pressed {
            self.toast_pressed(tag, now);
        }
        zikaron_ui::layer::fade_out(ctx);
        self.keep_draining(ctx);
    }

    fn toast_pressed(&mut self, tag: u64, now: f64) {
        if tag == OLD_DATA_TAG {
            if let Some(p) = self.shell.last_aside.clone() {
                self.act(Action::ViewOldData { root: p.display().to_string() }, now);
            }
        } else if tag == CLI_SWITCHES_TAG {
            self.go_cli_switches(now);
        } else {
            self.view_task(tag, now);
        }
    }

    fn appearance(&self) -> skin::Appearance {
        self.shell.machine.appearance.as_deref().and_then(skin::Appearance::named).unwrap_or_default()
    }
}

fn done_is_bad(d: &Done) -> bool {
    match d {
        Done::Verified(v) => !v.mismatches.is_empty(),
        Done::Delivery(d) => !d.matched(),
        Done::Checked(x) => x.judged.verdict == zikaron_kit::tokens::CheckVerdict::Fail.as_str(),
        Done::Diligence(r) => r.double_sold(),
        Done::Published { read, .. } => !read.complete(),
        Done::Chain { gas_wei: None, .. } => true,
        Done::Kit { left_out, unreadable, withdrawn, .. } => !left_out.is_empty() || !unreadable.is_empty() || !withdrawn.is_empty(),
        _ => false,
    }
}

fn folder_of(path: &str) -> String {
    std::path::Path::new(path).parent().map(|d| width::file_name(&d.display().to_string())).unwrap_or_default()
}

fn grant_file_said(path: &str) -> String {
    fill2(Key::SaidGrantFile, &width::file_name(path), &folder_of(path))
}

impl eframe::App for Win {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        if let Some(w) = self.drops.as_ref() {
            feed_drops(raw, w.hovering(), w.take());
        }
    }

    fn logic(&mut self, ctx: &egui::Context, _f: &mut eframe::Frame) {
        self.turn(ctx);
        if self.reads_behind {
            self.bar_turn(ctx, zikaron_ui::motion::now(ctx));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _f: &mut eframe::Frame) {
        self.paint(ui);
        if self.shell.rows_wanted.load(std::sync::atomic::Ordering::Relaxed) {
            ui.ctx().request_repaint();
        }
        if self.reads_behind {
            self.about_window(ui.ctx());
        }
    }

    fn clear_color(&self, _v: &egui::Visuals) -> [f32; 4] {
        if zikaron_ui::material::glass() { [0.0; 4] } else { zikaron_ui::palette::canvas() }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.drops_let_go();
        if let Applied::Stopped(r) = apply(&mut self.shell, Action::Quit) {
            self.reaped = Some(r);
        }
        crate::keybox::lock();
    }
}

impl Win {
    pub(super) fn drops_let_go(&mut self) {
        if let Some(w) = self.drops.take() {
            w.stop();
        }
    }
}

pub(super) fn feed_drops(raw: &mut egui::RawInput, hovering: bool, dropped: Vec<std::path::PathBuf>) {
    if hovering && raw.hovered_files.is_empty() {
        raw.hovered_files.push(egui::HoveredFile::default());
    }
    raw.dropped_files.extend(dropped.into_iter().map(|p| egui::DroppedFile { path: Some(p), ..Default::default() }));
}

fn gap_say(g: crate::watchx::Gap) -> String {
    use crate::watchx::Say;
    let n = g.n.to_string();
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
        Say::BackupFailed => t(Key::GapBackupFailed).to_string(),
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
    fn task_word(&self, k: crate::task::Kind) -> Key {
        if k == crate::task::Kind::Fetch && self.shell.fetch_checks_tail {
            Key::TaskCheckTail
        } else {
            task_key(k)
        }
    }
}

impl Win {
    fn paths_landed(&mut self, ctx: &egui::Context, landed: &[crate::task::Outcome]) {
        let pass = zikaron_ui::motion::pass(ctx);
        for o in landed.iter().filter(|o| o.kind == crate::task::Kind::Path) {
            let ticket = ctx.data_mut(|d| d.get_temp_mut_or_default::<PathMail>(path_mail()).ticket.take());
            match &o.result {
                Ok(Done::Path(Some(p))) => {
                    if let Some(site) = ticket {
                        ctx.data_mut(|d| d.get_temp_mut_or_default::<PathMail>(path_mail()).answer = Some((site, p.clone(), pass)));
                    }
                }
                Ok(_) | Err(_) => {}
            }
        }
        ctx.data_mut(|d| {
            let m = d.get_temp_mut_or_default::<PathMail>(path_mail());
            if m.answer.as_ref().is_some_and(|a| a.2 + 2 < pass) {
                m.answer = None;
            }
        });
    }

    fn paths_ask(&mut self, ctx: &egui::Context) {
        let Some((site, kind)) = ctx.data_mut(|d| d.get_temp_mut_or_default::<PathMail>(path_mail()).asked.take()) else {
            return;
        };
        if self.shell.tasks.in_flight(crate::task::Kind::Path) {
            let now = zikaron_ui::motion::now(&ctx);
            self.toasts.say(t(Key::PathAlreadyOpen), Tone::Note, now);
            return;
        }
        if kind == crate::platform::Pick::FileOrFolder && self.asks_which {
            self.ux.pick_which = Some(site);
            return;
        }
        match (self.asker)(kind) {
            Ok(wait) => {
                if crate::action::wait_path(&mut self.shell, wait) == crate::task::Spawned::Started {
                    ctx.data_mut(|d| d.get_temp_mut_or_default::<PathMail>(path_mail()).ticket = Some(site));
                }
            }
            Err(f) => self.shell.faults.push(f),
        }
    }
}


impl Win {
    fn act(&mut self, a: Action, now: f64) -> Applied {
        if matches!(a, Action::OpenEntry { .. }) {
            self.opened = None;
        }
        let want = match &a {
            Action::OpenEntry { id } => Some(id.clone()),
            _ => None,
        };
        if let Some(k) = task_of(&a) {
            self.ux.origin.retain(|(x, _, _)| *x != k);
            let h = self.hist().clone();
            self.ux.origin.push((k, self.ux.nav.stack, h));
        }
        self.tell_faults(now);
        let applied = apply(&mut self.shell, a);
        if applied == Applied::Started(crate::task::Kind::Gate) {
            self.ux.gate_site = None;
        }
        self.told(applied, want, now)
    }

    fn entry_say(&self, id: &str) -> String {
        let chain_now = self.shell.chain_now();
        self.shell
            .rows
            .as_ref()
            .and_then(|(rows, _)| rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).map(|x| human_summary(x, chain_now)))
            .unwrap_or_else(|| crate::ledgerx::short(id))
    }

    fn id_name(&self, id: &str) -> String {
        self.shell.identities.as_ref().and_then(|reg| reg.find(id)).map(identity::id_name).unwrap_or_else(|| t(Key::Unnamed).to_string())
    }

    fn told(&mut self, applied: Applied, want: Option<String>, now: f64) -> Applied {
        let (said, tone) = match applied.clone() {
            Applied::Shown(_) => (String::new(), Tone::Note),
            Applied::Consented { .. } => (String::new(), Tone::Note),
            Applied::RerootMade { .. } => (String::new(), Tone::Note),
            Applied::CliPassAdded { .. } => (String::new(), Tone::Note),
            Applied::CliPassDropped { .. } => (t(Key::SaidCliPassDropped).to_string(), Tone::Note),
            Applied::Started(k) => {
                self.ux.asked.push(k);
                (String::new(), Tone::Note)
            }
            Applied::Refused(k) => (fill1(Key::SaidInFlight, t(self.task_word(k))), Tone::Bad),
            Applied::Stopped(r) => (fill1(Key::SaidReaped, &r.joined.to_string()), Tone::Note),
            Applied::Seated(r) => (fill1(Key::SaidSeated, t(id_seat_key(r))), Tone::Note),
            Applied::Identities(_) | Applied::FreshWords | Applied::WordsShown | Applied::WordsHidden => (String::new(), Tone::Note),
            Applied::Unlocked | Applied::LockedUp => (String::new(), Tone::Note),
            Applied::PinChanged => (t(Key::SaidPinChanged).to_string(), Tone::Note),
            Applied::AutoAnchor(on) => (fill1(Key::SaidAutoAnchor, t(if on { Key::On } else { Key::Off })), Tone::Note),
            Applied::HideLocalDeletions(on) => (fill1(Key::SaidHideLocalDeletions, t(if on { Key::On } else { Key::Off })), Tone::Note),
            Applied::NetworkChosen { name, .. } => (fill1(Key::SaidNetworkChosen, &network_label(&name)), Tone::Note),
            Applied::AutoLockSet { on: true, secs } => (fill1(Key::SaidAutoLockOn, &(secs / 60).to_string()), Tone::Note),
            Applied::AutoLockSet { on: false, .. } => (t(Key::SaidAutoLockOff).to_string(), Tone::Note),
            Applied::PrimarySet { id } => (fill1(Key::SaidPrimarySet, &self.id_name(&id)), Tone::Note),
            Applied::BackupRestored(sm) => (
                format!("{} · {}", t(Key::SaidBackupRestored), backup_content(&sm)),
                Tone::Note,
            ),
            Applied::Resumed(_) | Applied::CaughtUp { .. } => (String::new(), Tone::Note),
            Applied::KeyboxReset => (t(Key::SaidKeyboxReset).to_string(), Tone::Note),
            Applied::MachineAside { to, .. } => (fill1(Key::SaidMachineAside, &to), Tone::Note),
            Applied::Recovered => (t(Key::SaidRecovered).to_string(), Tone::Note),
            Applied::FreshDropped => {
                self.ux.id_confirming = false;
                self.ux.id_confirm = Default::default();
                self.ux.id_words_open = false;
                self.ux.id_words_seen = false;
                (String::new(), Tone::Note)
            }
            Applied::IdentityMade { restored, id, .. } => {
                self.ux.id_confirming = false;
                self.ux.id_confirm = Default::default();
                self.ux.id_words_open = false;
                self.ux.id_words_seen = false;
                let name = if self.ux.id_new_label.trim().is_empty() { self.id_name(&id) } else { self.ux.id_new_label.trim().to_string() };
                if !restored && self.shell.unfetched.is_some() {
                    self.toasts.say_full(fill1(Key::SaidIdentityMade, &name), t(Key::ChainOnlyRestore), "", Tone::Note, now);
                    (String::new(), Tone::Note)
                } else {
                    (if restored { t(Key::SaidKeyRestoredPlain).to_string() } else { fill1(Key::SaidIdentityMade, &name) }, Tone::Note)
                }
            }
            Applied::IdentityNamed { label: name, .. } => {
                if name.is_empty() {
                    (t(Key::SaidNameCleared).to_string(), Tone::Note)
                } else {
                    (fill1(Key::SaidRenamed, &name), Tone::Note)
                }
            }
            Applied::IdentitySwitched { id } => (fill1(Key::SaidIdentitySwitched, &self.id_name(&id)), Tone::Note),
            Applied::IdentityKept { .. } => (String::new(), Tone::Note),
            Applied::IdentityDeleted { .. } => (fill1(Key::SaidIdentityDeleted, &self.ux.id_deleting.take().unwrap_or_else(|| t(Key::Unnamed).to_string())), Tone::Note),
            Applied::KeyBackedUp { path, .. } => (fill1(Key::SaidKeyBackedUp, &width::file_name(&path)), Tone::Note),
            Applied::FactsForgotten { .. } => (t(Key::SaidRecheckAll).to_string(), Tone::Note),
            Applied::Homed { root, mode } => (fill2(Key::SaidHomed, &width::file_name(&root), t(if mode.writable() { Key::NoteWriter } else { Key::NoteReader })), Tone::Note),
            Applied::Migrated { root } => (fill1(Key::SaidMigrated, &width::file_name(&root)), Tone::Note),
            Applied::Capped(n) => (fill1(Key::SaidCapped, &size_say(n)), Tone::Note),
            Applied::Mirrored { path, entries, added, topped_up } => (
                if topped_up { fill2(Key::SaidMirrorToppedUp, &added.to_string(), &width::file_name(&path)) } else { fill2(Key::SaidMirrored, &entries.to_string(), &width::file_name(&path)) },
                Tone::Note,
            ),
            Applied::Endpoints(n) => (fill1(Key::SaidEndpoints, &n.to_string()), Tone::Note),
            Applied::LogAsks(_) => (String::new(), Tone::Note),
            Applied::RestMode(on) => (t(if on { Key::SaidRestOn } else { Key::SaidRestOff }).to_string(), Tone::Note),
            Applied::RestAdded { base } => (fill1(Key::SaidRestAdded, &zikaron_net::sayable(&base)), Tone::Note),
            Applied::RestKeyed { base } => (fill1(Key::SaidRestKeyed, &zikaron_net::sayable(&base)), Tone::Note),
            Applied::RestRemoved { .. } => (t(Key::SaidRestRemoved).to_string(), Tone::Note),
            Applied::Genesised { .. } => {
                self.shell.reread(crate::shell::Reread::Wrote);
                self.ux.wiz_statement.clear();
                (t(Key::SaidGenesisedPlain).to_string(), Tone::Note)
            }
            Applied::AdoptedInPlace { entries, linked, label } => (fill3(Key::SaidAdopted, &entries.to_string(), &linked.to_string(), &label_human(&label)), Tone::Note),
            Applied::Opened { .. } => {
                self.read_detail(want.as_deref());
                (String::new(), Tone::Note)
            }
            Applied::Annotated(_) => {
                self.shell.reread(crate::shell::Reread::Wrote);
                (t(Key::SaidAnnotated).to_string(), Tone::Note)
            }
            Applied::Retracted { queued, local, .. } => {
                self.shell.reread(crate::shell::Reread::WroteGrant);
                if local {
                    (t(Key::SaidRetractedLocal).to_string(), Tone::Note)
                } else {
                    (fill1(Key::SaidRetracted, &queued.to_string()), Tone::Note)
                }
            }
            Applied::Basis { chain } => (fill1(Key::SaidBasis, &chain.to_string()), Tone::Note),
            Applied::Every(n) => (fill1(Key::SaidAuditEvery, &n.to_string()), Tone::Note),
            Applied::Took { .. } => (String::new(), Tone::Note),
            Applied::Recorded { queued, .. } => {
                self.shell.reread(crate::shell::Reread::Wrote);
                (fill1(Key::SaidQueued, &queued.to_string()), Tone::Note)
            }
            Applied::RecordedBatch { ids, stopped, queued, .. } => {
                self.shell.reread(crate::shell::Reread::Wrote);
                match stopped {
                    None => (fill2(Key::SaidBatch, &ids.len().to_string(), &queued.to_string()), Tone::Note),
                    Some((i, p, f)) => {
                        self.faults_told = self.shell.faults.len();
                        let said = fill3(Key::SaidBatchStopped, &ids.len().to_string(), &(i + 1).to_string(), &format!("{} · {}", width::file_name(&p), f.human()));
                        self.toasts.say_full(said, "", &f.shown(), Tone::Bad, now);
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
            Applied::Registered { path } => (fill1(Key::SaidRepo, &width::file_name(&path)), Tone::Note),
            Applied::Since { grew, .. } => (fill1(Key::SaidSincePlain, &grew.map(|n| n.to_string()).unwrap_or_else(|| t(Key::RepoUnknown).to_string())), Tone::Note),
            Applied::Gas { gas, .. } => (fill1(Key::SaidGas, &gas.to_string()), Tone::Note),
            Applied::Picked { items, pulled } => (fill2(Key::SaidPicked, &items.to_string(), &pulled.to_string()), Tone::Note),
            Applied::Queued { queued, .. } => {
                self.shell.reread(crate::shell::Reread::Wrote);
                (fill1(Key::SaidQueued, &queued.to_string()), Tone::Note)
            }
            Applied::Granted { queued, .. } => {
                self.shell.reread(crate::shell::Reread::WroteGrant);
                self.ux.u4.relicense_signed = true;
                self.ux.u4.relicense_reveal = self.on_relicense_page();
                if std::mem::take(&mut self.ux.u4.grant_in_gate) {
                    self.grant_signed();
                }
                (fill1(Key::SaidGranted, &queued.to_string()), Tone::Note)
            }
            Applied::GrantFileExported { path, why, .. } => {
                let second = why.say().map(|k| fill1(k, &width::file_name(&path))).unwrap_or_default();
                self.toasts.say_full(grant_file_said(&path), &second, "", Tone::Note, now);
                (String::new(), Tone::Note)
            }
            Applied::ReadNets(_) => (String::new(), Tone::Note),
            Applied::PublishSet { url } => match url {
                Some(u) => (fill1(Key::SaidPublishSet, &u), Tone::Note),
                None => (t(Key::SaidPublishCleared).to_string(), Tone::Note),
            },
            Applied::Clashed(n) => {
                self.clash_modal = n > 0;
                (fill1(Key::SaidClashed, &n.to_string()), if n > 0 { Tone::Bad } else { Tone::Note })
            }
            Applied::Revoked { notice: Some(n), .. } => {
                let _ = self.ux.u3.revoking.take();
                self.shell.reread(crate::shell::Reread::Wrote);
                if n.withdrawn {
                    (t(Key::SaidWithdrawn).to_string(), Tone::Note)
                } else {
                    (t(Key::SaidNotNoticed).to_string(), Tone::Bad)
                }
            }
            Applied::Revoked { queued, .. } => {
                let _ = self.ux.u3.revoking.take();
                self.shell.reread(crate::shell::Reread::Wrote);
                (fill1(Key::SaidRevoked, &queued.to_string()), Tone::Note)
            }
            Applied::GrantsMigrated { count, queued, .. } => {
                self.shell.reread(crate::shell::Reread::Wrote);
                (fill2(Key::SaidGrantsMigrated, &count.to_string(), &queued.to_string()), Tone::Note)
            }
            Applied::Storied { grant: _, revocations } => (fill1(Key::SaidStoried, &revocations.to_string()), Tone::Note),
            Applied::Rotated { .. } => (String::new(), Tone::Note),
            Applied::Moved { queued, left_out } => {
                self.shell.reread(crate::shell::Reread::Wrote);
                (
                    if left_out {
                        format!("{} · {} · {}", t(Key::ChainMoveTitle), fill1(Key::CountN, &queued.to_string()), t(Key::ChainOldUnread))
                    } else {
                        format!("{} · {}", t(Key::ChainMoveTitle), fill1(Key::CountN, &queued.to_string()))
                    },
                    Tone::Note,
                )
            }
            Applied::Upgraded { .. } => {
                self.shell.reread(crate::shell::Reread::Wrote);
                (String::new(), Tone::Note)
            }
            Applied::Snapshot { path, .. } => (fill1(Key::SaidSnapshotPlain, &width::file_name(&path)), Tone::Note),
            Applied::HeldPartly { ids, refused, renewed } => {
                let from = self.shell.faults.len().saturating_sub(refused);
                let why = self.shell.faults[from..].iter().map(|f| [f.human().to_string(), f.shown()].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ")).collect::<Vec<_>>().join("\n");
                self.faults_told = self.shell.faults.len();
                self.toasts.say_full(held_said(ids.len(), renewed, refused), "", &why, Tone::Bad, now);
                (String::new(), Tone::Bad)
            }
            Applied::Held { ids, renewed, .. } => (held_said(ids.len(), renewed, 0), Tone::Note),
            Applied::HeldRenewed { .. } => (t(Key::SaidHeldRenewed).to_string(), Tone::Note),
            Applied::HeldNoted { .. } => (String::new(), Tone::Note),
            Applied::Upstream { grant } => {
                let issuer = self.held_issuer_name(&grant);
                (fill1(Key::SaidUpstream, &issuer), Tone::Note)
            }
            Applied::ReviewEvery(n) => (fill1(Key::SaidReviewEvery, &n.to_string()), Tone::Note),
            Applied::Spoken(l) => (fill1(Key::SaidLang, l.label()), Tone::Note),
            Applied::Zoned(z) => (fill1(Key::SaidZone, zone_label(z)), Tone::Note),
            Applied::Appeared(a) => (fill1(Key::SaidAppearance, appearance_label(&a)), Tone::Note),
            Applied::ProxySet(c) => (fill1(Key::SaidProxy, &proxy_label(&c)), Tone::Note),
            Applied::CliPathRead(_) => (String::new(), Tone::Note),
            Applied::CliAnchorSet(to) => (fill1(Key::SaidCliAnchor, t(cli_anchor_label(to))), Tone::Note),
            Applied::CliDesktopSet(on) => (t(if on { Key::SaidCliDesktopOn } else { Key::SaidCliDesktopOff }).to_string(), Tone::Note),
            Applied::CliAloneSet(Some(expires)) => (fill1(Key::SaidCliAloneOn, &settings::cli_expiry_said(expires)), Tone::Note),
            Applied::CliAloneSet(None) => (t(Key::SaidCliAloneOff).to_string(), Tone::Note),
            Applied::SendAsked { count } => {
                self.shell.attention = true;
                (fill1(Key::SaidSendAsked, &count.to_string()), Tone::Alert)
            }
            Applied::Booked { on, .. } => (t(if on { Key::SaidBookedOn } else { Key::SaidBookedOff }).to_string(), Tone::Note),
            Applied::Trouble(f) => {
                if f.which() == Some(crate::fault::Known::Conflict) {
                    self.clash_modal = true;
                }
                if crate::watchx::is_network(&f) {
                    self.say_fault(&f, now);
                }
                (String::new(), Tone::Bad)
            }
        };
        let next = match &applied {
            Applied::Genesised { next, .. }
            | Applied::Recorded { next, .. }
            | Applied::RecordedBatch { next, .. }
            | Applied::Queued { next, .. }
            | Applied::Granted { next, .. }
            | Applied::Revoked { next, .. }
            | Applied::GrantsMigrated { next, .. }
            | Applied::Upgraded { next, .. }
            | Applied::Retracted { next, .. } => Some(*next),
            _ => None,
        };
        let out = match &applied {
            Applied::Mirrored { .. } => Some(Out::Mirror),
            Applied::Snapshot { .. } => Some(Out::Snapshot),
            _ => None,
        };
        let why = out.and_then(|x| self.landing_note(x));
        let followed = self.followed_write(&applied);
        let offline = matches!(applied, Applied::Recorded { .. } | Applied::Queued { .. }) && self.shell.status.is_some();
        if !said.is_empty() {
            if offline {
                self.toasts.say_full(said, t(Key::OfflineCheckFirst), "", tone, now);
            } else if next == Some(crate::action::Next::Wait) && !self.ux.reroot.open && followed.is_none() {
                self.toasts.say_full(said, t(Key::QueueWaitNext), "", tone, now);
            } else if let Some(w) = why {
                self.toasts.say_full(said, &w, "", tone, now);
            } else {
                self.toasts.say(said, tone, now);
            }
        }
        if next == Some(crate::action::Next::Send) && !matches!(applied, Applied::Upgraded { .. }) {
            let n = self.shell.sendable_here();
            if n > 0 {
                self.u3_open_confirm(U3Confirm::Send { count: n });
            } else if let Some(own) = self.shell.queue.items.iter().rev().find(|q| !q.step.in_flight()).map(|q| q.id.clone()) {
                self.ux.u3.follow = Some(own);
            }
        }
        if let Some(own) = followed {
            self.card_for(own);
        }
        if let Applied::Trouble(f) = &applied {
            if self.ux.id_modal.is_some() {
                self.ux.id_trouble = Some(f.clone());
            }
        }
        applied
    }

    pub(super) fn card_batch(&self) -> Option<usize> {
        let n = self.shell.sendable_here();
        (n > 0).then_some(n)
    }

    pub(super) fn goes_later(&self, own: &str, count: usize) -> bool {
        self.shell.queue.items.iter().any(|q| q.id.eq_ignore_ascii_case(own) && !q.step.in_flight()) && !self.shell.next_batch(count).iter().any(|b| b.eq_ignore_ascii_case(own))
    }

    pub(super) fn followed_write(&self, applied: &Applied) -> Option<String> {
        match applied {
            Applied::Queued { id, .. } if self.card_follows => Some(id.clone()),
            _ => None,
        }
    }

    pub(super) fn card_for(&mut self, own: String) {
        self.ux.u3.follow = Some(own);
        if let Some(n) = self.card_batch() {
            self.u3_open_confirm(U3Confirm::Send { count: n });
        }
    }

    pub(super) fn card_follow_up(&mut self) {
        let Some(own) = self.ux.u3.follow.clone() else { return };
        if !self.shell.queue.items.iter().any(|q| q.id.eq_ignore_ascii_case(&own) && !q.step.in_flight()) {
            self.ux.u3.follow = None;
            return;
        }
        if self.any_sheet() || self.ux.u3.sending.is_some() || self.shell.tasks.in_flight(crate::task::Kind::Anchor) {
            return;
        }
        if let Some(n) = self.card_batch() {
            self.u3_open_confirm(U3Confirm::Send { count: n });
        }
    }

    pub(super) fn act_carded(&mut self, a: Action, now: f64) -> Applied {
        self.card_follows = true;
        let r = self.act(a, now);
        self.card_follows = false;
        r
    }

    fn vault_or(&mut self, site: VaultSite, r: Applied, now: f64) {
        if r == Applied::Started(crate::task::Kind::Vault) {
            self.ux.vault_site = Some(site);
        } else if r == Applied::Started(crate::task::Kind::Gate) {
            self.ux.gate_site = Some(site);
        } else {
            self.vault_back(site, r, now);
        }
    }

    fn vault_back(&mut self, site: VaultSite, r: Applied, now: f64) {
        match site {
            VaultSite::Wizard => match r {
                Applied::Unlocked => self.ux.gate_clear(),
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
                Applied::Unlocked | Applied::Recovered | Applied::KeyboxReset | Applied::MachineAside { .. } => self.ux.gate_clear(),
                Applied::Trouble(f) => {
                    self.ux.pin_shake = Some(now);
                    self.ux.pin_again.clear();
                    let burnt = self.shell.vault.is(crate::keybox::State::LockedOut) && f.which() == Some(crate::fault::Known::PinWrong);
                    if f.which() == Some(crate::fault::Known::KdfBelowFloor) {
                        self.ux.gate_reseal = true;
                        self.ux.pin_shake = None;
                    }
                    self.ux.gate_trouble = if burnt {
                        None
                    } else if f.which() == Some(crate::fault::Known::PinWrong) {
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
            VaultSite::Persona => self.persona_back(r, now),
            VaultSite::Reroot => {
                if let Applied::Trouble(f) = r {
                    self.ux.reroot.landed = Some(f);
                }
            }
            VaultSite::CliPass => match r {
                Applied::CliPassAdded { pass, .. } => {
                    self.ux.pass_pin.clear();
                    self.ux.pass_out = Some(pass);
                }
                Applied::Trouble(f) => {
                    self.ux.pin_shake = Some(now);
                    self.ux.pass_pin.clear();
                    self.ux.pass_trouble = Some(f);
                }
                _ => {}
            },
            VaultSite::Cli => match r {
                Applied::CliAloneSet(_) => self.cli_close(),
                Applied::Trouble(f) => {
                    self.ux.pin_shake = Some(now);
                    self.ux.cli_trouble = Some(f);
                }
                _ => {}
            },
            VaultSite::Id => match r {
                Applied::Trouble(f) => {
                    self.ux.pin_shake = Some(now);
                    self.ux.id_trouble = Some(f);
                }
                r if !id_sheet_stays(&r) => self.ux.id_close(),
                _ => {}
            },
        }
    }
}

fn id_sheet_stays(r: &Applied) -> bool {
    use crate::task::Kind;
    match r {
        Applied::Trouble(_) | Applied::Refused(_) | Applied::FreshWords => true,
        Applied::Started(k) => !matches!(k, Kind::Seat | Kind::Keystore),
        _ => false,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PinSaid {
    Gate,
    Sheet,
    Toast,
}

impl Win {
    fn take_first_pin(&mut self, now: f64, said_at: PinSaid) -> bool {
        let pin = std::mem::take(&mut self.ux.pin);
        match crate::keybox::pin_trouble(pin.expose()) {
            None => {
                self.ux.pin_again = pin;
                true
            }
            Some(why) => {
                self.ux.pin_shake = Some(now);
                let f = crate::fault::Fault::known(crate::fault::Known::PinShape, why.as_str().to_string());
                let said = t(if why == crate::keybox::PinTrouble::Shape { Key::PinRules } else { Key::PinTooSimple });
                match said_at {
                    PinSaid::Gate => self.ux.gate_trouble = Some(said.to_string()),
                    PinSaid::Sheet => self.ux.pin_why = Some(said),
                    PinSaid::Toast => self.toasts.say_full(said.to_string(), "", &f.shown(), Tone::Bad, now),
                }
                false
            }
        }
    }

    fn say_fault(&mut self, f: &crate::fault::Fault, now: f64) {
        let (what, mut next) = match self.import_face(f) {
            Some((what, next)) => (what.to_string(), next.to_string()),
            None => (f.human().to_string(), f.next()),
        };
        if f.which() == Some(crate::fault::Known::Locked) && self.shell.vault.absent() {
            next = t(Key::FaultNextNoPinYet).to_string();
        }
        let mut act: Option<(String, String)> = None;
        if f.which() == Some(crate::fault::Known::InsufficientFunds) {
            let named = crate::action::funds_key(f.tail());
            let short = named.or(self.shell.anchor);
            match crate::action::funds_of(f.tail()) {
                Some((need, have)) => next = fill2(Key::FundsNeedHave, &eth_cap(need), &eth_held(have)),
                None if named.is_some() && named != self.shell.anchor => next.clear(),
                None => {}
            }
            act = short.map(|a| (t(Key::IdDoCopy).to_string(), a.hex()));
        }
        if f.which() == Some(crate::fault::Known::CliWriting) {
            let go = vec![(t(Key::CliWritingGo).to_string(), zikaron_ui::toast::Act::Tag(CLI_SWITCHES_TAG))];
            self.toasts.say_keys(what, &next, &f.shown(), Tone::Bad, now, go);
            return;
        }
        self.toasts.say_with(what, &next, &f.shown(), Tone::Bad, now, act);
    }

    fn tell_faults(&mut self, now: f64) {
        let n = self.shell.faults.len();
        if self.faults_told > n {
            self.faults_told = n;
        }
        let shut = self.shell.vault.gate_up();
        for i in self.faults_told..n {
            let f = self.shell.faults[i].clone();
            let locked = f.which() == Some(crate::fault::Known::Locked);
            let pin = matches!(f.which(), Some(crate::fault::Known::PinWrong));
            let exit_unread = f.then_key() == Some(Key::ExitRetryLater);
            if (crate::watchx::is_network(&f) && !exit_unread) || (shut && (locked || pin)) || self.sheet_shows(&f) {
                continue;
            }
            self.say_fault(&f, now);
        }
        self.faults_told = n;
    }

    fn sheet_shows(&self, f: &crate::fault::Fault) -> bool {
        let ux = &self.ux;
        let backup = self.shell.failed.get(&crate::task::Kind::Backup);
        [
            ux.id_modal.is_some().then_some(ux.id_trouble.as_ref()).flatten(),
            ux.bk.is_some().then_some(ux.bk_trouble.as_ref()).flatten(),
            ux.bk.is_some().then_some(backup).flatten(),
            (ux.persona.sheet.is_some() || ux.persona.consent).then_some(ux.persona.trouble.as_ref()).flatten(),
            ux.reroot.open.then_some(ux.reroot.trouble.as_ref()).flatten(),
            ux.cli_sheet.then_some(ux.cli_trouble.as_ref()).flatten(),
            ux.pass_open.then_some(ux.pass_trouble.as_ref()).flatten(),
            matches!(ux.u3.form, Some(U3Form::Annotate)).then_some(ux.annotate_why.as_ref().map(|(_, x)| x)).flatten(),
        ]
        .into_iter()
        .flatten()
        .any(|x| x == f)
            || (ux.u4.import_open && ux.u4.import_err.as_ref().is_some_and(|(_, _, raw)| *raw == f.shown()))
    }

    fn import_face(&self, f: &crate::fault::Fault) -> Option<(&'static str, &'static str)> {
        let not_a_file = f.which() == Some(crate::fault::Known::FileMissing);
        (self.ux.u4.import_open && not_a_file).then(|| (t(Key::V2ImportUnreadable), t(Key::V2ImportUnreadableNext)))
    }
}

fn task_of(a: &Action) -> Option<crate::task::Kind> {
    use crate::task::Kind;
    Some(match a {
        Action::CheckPayload { .. } => Kind::Check,
        Action::AuditCheck | Action::AuditOthers => Kind::Behind,
        Action::VerifyWork { .. } => Kind::Verify,
        Action::ExtractOriginals { .. } => Kind::Extract,
        Action::ReadBook { .. } => Kind::Book,
        Action::Diligence { .. } => Kind::Diligence,
        Action::ExportKit { .. } => Kind::Kit,
        Action::ExportBadge { .. } => Kind::Badge,
        Action::ExportGrantFile { .. } | Action::ExportMirror { .. } => Kind::Gate,
        Action::ReviewVault => Kind::Review,
        Action::Reconcile => Kind::Reconcile,
        Action::Measure => Kind::Archive,
        Action::ReadChain => Kind::Chain,
        Action::ReadChainTime => Kind::ChainTime,
        Action::ReadReadNetwork { .. } => Kind::ReadNet,
        Action::SelfCheck => Kind::SelfCheck,
        Action::CheckPublished { .. } => Kind::Publish,
        Action::SendBatch { .. } | Action::BumpFee { .. } => Kind::Anchor,
        Action::FetchLedger { .. } => Kind::Fetch,
        Action::FetchAside { .. } => Kind::Fetch,
        _ => return None,
    })
}

pub enum Start {
    Page(Page),
    Place(Place, bool),
    Scene(Scene),
}

pub struct FaceReading {
    pub width: f32,
    pub right_widget: f32,
    pub right_text: f32,
    pub rows: usize,
    pub tables: Vec<zikaron_ui::probe::Took>,
    pub inner: Option<bool>,
    pub inner_right: Option<f32>,
    pub inner_head: Option<String>,
    pub inner_head_keys: Option<bool>,
    pub inner_history: Option<(bool, bool)>,
    pub route: String,
    pub inner_route: Option<String>,
    pub texts: Vec<String>,
    pub cells: Vec<CellFit>,
    pub justified_texts: usize,
    pub tiles: Vec<(egui::Rect, bool)>,
}

pub struct CellFit {
    pub width: f32,
    pub over: usize,
    pub wrapped: usize,
}

#[derive(Clone, Debug)]
struct ExtractFrom {
    path: String,
    takes: Option<Result<zikaron_glue::extract::Plan, zikaron_glue::extract::Refused>>,
    names: Vec<(String, String)>,
}

struct WorkLine {
    name: String,
    work: String,
    id: String,
    seq: u64,
    lamp: crate::ledgerx::Lamp,
    deleted: Option<u64>,
    row: crate::ledgerx::Row,
}

#[derive(Default)]
struct Ux {
    about_open: bool,
    about_icon: Option<egui::TextureHandle>,
    register_at: Option<String>,
    watch_rows: Option<Vec<crate::watchx::Row>>,
    wiz_from: Option<(crate::roles::Role, Stack, Option<Place>, Option<History>)>,
    wiz_exit_ask: bool,
    bk: Option<Bk>,
    bk_step: BkStep,
    bk_path: String,
    bk_pw: crate::secret::Secret,
    bk_pw2: crate::secret::Secret,
    bk_pin: crate::secret::Secret,
    bk_dir: String,
    bk_trouble: Option<crate::fault::Fault>,
    cli_sheet: bool,
    cli_pin: crate::secret::Secret,
    cli_span: [String; 3],
    cli_never: bool,
    cli_trouble: Option<crate::fault::Fault>,
    pass_open: bool,
    pass_note: String,
    pass_pin: crate::secret::Secret,
    pass_out: Option<crate::secret::Secret>,
    pass_for_ai: bool,
    pass_trouble: Option<crate::fault::Fault>,
    fetch_pw: crate::secret::Secret,
    fetch_held: crate::secret::Secret,
    landings: Vec<(Out, crate::home::Chosen)>,
    search: std::collections::BTreeMap<&'static str, String>,
    range: std::collections::BTreeMap<&'static str, (String, String)>,
    nav: skeleton::Frame,
    wizard: Option<crate::nav::Step>,
    wizard_asked: bool,
    vault_absent: Option<bool>,
    wizard_forced: bool,
    confirm_genesis: bool,
    confirm_adopt: bool,
    wiz_adopt: String,
    upgrade_open: bool,
    upgrade_put_away: bool,
    upgrade_came_up: bool,
    upgrade_auto: bool,
    upgrade_read_at: f64,
    upgrade_cost_for: Option<crate::upgradex::Stage>,
    rotate_open: bool,
    rotate_put_away: bool,
    rot_read_at: f64,
    rot_words: u8,
    rot_words_open: bool,
    rot_pending: bool,
    rot_unhomed: bool,
    rot_words_note: Option<crate::action::WordsNote>,
    rot_bookless: bool,
    rot_unchecked: Option<String>,
    rot_words_unchecked: Option<crate::roles::Role>,
    rot_confirm: [crate::secret::Secret; 3],
    rot_walked: bool,
    rot_to_given: bool,
    rot_new_words: bool,
    rot_addr: String,
    rot_asked: String,
    rot_sweep_addr: Option<String>,
    rot_sweep_asked: String,
    rot_sweep_said: Option<String>,
    move_open: bool,
    move_cells: [String; 4],
    lang_applied: bool,
    page_seen: Option<Page>,
    page_drops: bool,
    now: f64,
    dir_sheet: Option<settings::DirSheet>,
    dir_waiting: bool,
    cap_edit: fields::SavedEdit,
    asks_edit: fields::SavedEdit,
    net_sheet: bool,
    rpc_err: Option<String>,
    rest_err: Option<(bool, String)>,
    basis_waiting: bool,
    publish_sheet: bool,
    publish_edit: bool,
    proxy_manual: bool,
    proxy_seeded: bool,
    asked: Vec<crate::task::Kind>,
    landed: std::collections::BTreeMap<crate::task::Kind, (bool, f64)>,
    origin: Vec<(crate::task::Kind, Stack, History)>,
    syncing: Vec<crate::task::Kind>,
    sync_said: Vec<Result<Done, crate::fault::Fault>>,
    synced_at: Option<f64>,
    wiz_statement: String,
    wiz_network: Option<String>,
    wiz_gas_asked: Option<u64>,
    wiz_gas_seen: Option<f64>,
    wiz_gas_not_seen: bool,
    audit_edit: fields::SavedEdit,
    review_edit: fields::SavedEdit,
    edits_owner: Option<(Option<std::path::PathBuf>, Option<String>, Option<crate::roles::Role>)>,
    other_held: crate::reads::Cell<(u64, bool), std::sync::Arc<Option<others::OtherView>>>,
    other_face: crate::reads::Cell<(u64, bool, u64), std::sync::Arc<Option<(crate::ledgerx::Row, &'static str, String, bool)>>>,
    basis_chain: String,
    basis_registry: String,
    basis_from: String,
    basis_custom: bool,
    readnet: settings::ReadNetUx,
    u3: U3,
    u4: U4,
    persona: persona::PersonaUx,
    reroot: reroot::RerootUx,
    other_hop: (String, usize),
    id_modal: Option<IdModal>,
    id_tab: usize,
    id_words: [crate::secret::Secret; 12],
    id_seat: Option<crate::roles::Role>,
    id_label: String,
    id_new_label: String,
    id_network: Option<String>,
    id_switch_open: Option<String>,
    id_deleting: Option<String>,
    id_delete_ledgers: Vec<crate::roles::Role>,
    id_delete_backup: Option<crate::identity::BackupSeen>,
    migrate_landing: Option<crate::home::Chosen>,
    id_hex: crate::secret::Secret,
    id_ks_path: String,
    id_ks_pw: crate::secret::Secret,
    id_confirm: [crate::secret::Secret; 3],
    id_confirming: bool,
    id_words_open: bool,
    id_words_seen: bool,
    id_pw: crate::secret::Secret,
    id_pw2: crate::secret::Secret,
    id_dir: String,
    id_trouble: Option<crate::fault::Fault>,
    id_auth_waiting: bool,
    id_pin: crate::secret::Secret,
    pin: crate::secret::Secret,
    pin_again: crate::secret::Secret,
    pin_old: crate::secret::Secret,
    gate_recover: bool,
    gate_trouble: Option<String>,
    pin_words: [crate::secret::Secret; 12],
    gate_words_ready: bool,
    gate_file_ready: bool,
    pin_ks_path: String,
    pin_ks_pw: crate::secret::Secret,
    pin_shake: Option<f64>,
    pin_why: Option<&'static str>,
    annotate_why: Option<(bool, crate::fault::Fault)>,
    vault_site: Option<VaultSite>,
    gate_site: Option<VaultSite>,
    gate_reseal: bool,
    gate_aside: Option<crate::home::AsidePlan>,
    gate_aside_trouble: Option<crate::fault::Fault>,
    last_input: f64,
    cli_path_seen: f64,
    sheet_shake: Option<f64>,
    sizes: std::collections::BTreeMap<String, Option<u64>>,
    files_of: std::collections::BTreeMap<String, (u64, Vec<String>)>,
    snapshot_sheet: bool,
    extract_sheet: bool,
    extract_from: Option<ExtractFrom>,
    out_sheet: Option<grants::OutSheet>,
    out_waiting: bool,
    clean_sheet: bool,
    pick_which: Option<egui::Id>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum VaultSite {
    Wizard,
    Change,
    Gate,
    Words,
    Id,
    Backup,
    Cli,
    Persona,
    Reroot,
    CliPass,
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum IdModal {
    New,
    Import,
    Words,
    Switch,
    Delete(String),
    Backup,
    Name(String),
    Pin,
    Primary(String),
}

impl Ux {
    fn id_open(&mut self, m: IdModal) {
        self.id_form_clear();
        self.pin_why = None;
        self.id_modal = Some(m);
    }

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
        self.gate_aside = None;
        self.gate_aside_trouble = None;
    }

    fn row_turn(&self, now: f64) -> bool {
        (!self.pin_again.is_empty() && self.pin.is_empty()) || self.pin_shake.is_some_and(|at| now - at < 0.5)
    }

    fn gate_back(&mut self) {
        for w in self.pin_words.iter_mut() {
            w.clear();
        }
        self.pin_ks_path.clear();
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
        self.id_form_clear();
    }

    fn id_form_clear(&mut self) {
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
        self.pin.clear();
        self.pin_again.clear();
        self.pin_old.clear();
        self.pin_shake = None;
    }

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

#[derive(Clone, Debug, PartialEq)]
enum U3Confirm {
    Retract { subject: String },
    Send { count: usize },
    Bump { tx: String },
    NoEstimate { count: usize, why: String, next: String, raw: String },
    NotSent { count: usize, why: String, next: String, raw: String },
    Grant,
    Revoke { grant: String },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum U3Form {
    Annotate,
}

#[derive(Default)]
struct U3 {
    confirm: Option<U3Confirm>,
    form: Option<U3Form>,
    send_after_estimate: Option<usize>,
    follow_audit: Option<u8>,
    estimate_from: f64,
    estimating: Option<usize>,
    bump_press: Option<(String, u64)>,
    recording: Option<Vec<String>>,
    sending: Option<u64>,
    follow: Option<String>,
    led_filter: usize,
    audit_open: bool,
    grant_days: u32,
    grant_days_set: bool,
    chain_time_asked: bool,
    new_anchor: Option<u8>,
    others_tab: usize,
    opened_tried: Option<String>,
    check_refused: Option<crate::fault::Fault>,
    check_for: Option<String>,
    others_for: Option<String>,
    ck_source_open: Option<usize>,
    drop_armed: Option<String>,
    link_typed: Vec<(String, String, bool)>,
    revoke_case: String,
    revoking: Option<String>,
    kit_preview: Option<(String, Result<(Vec<String>, Vec<String>), String>)>,
    kit_made_for: Option<String>,
    kit_drawn_at: Option<u64>,
    kit_names: Option<(String, Vec<(String, Result<Option<(String, bool)>, String>)>)>,
    kit_orig: Option<(String, Result<Vec<crate::kitx::Listed>, String>)>,
    kit_withdrawn: Vec<String>,
    kit_orig_off: std::collections::BTreeSet<String>,
    kit_folder: bool,
    kit_addressed: bool,
    kit_no_companion: bool,
    keys_details: bool,
    kit_admit: Option<(String, Option<crate::kitx::Originals>)>,
    kit_pick: Option<KitPick>,
    work_pick: bool,
    terms: Option<TermsFile>,
}

struct KitPick {
    ids: Vec<String>,
}

struct TermsFile {
    path: String,
    name: String,
    size: Option<u64>,
    hex: String,
}

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

#[derive(Default)]
struct U4 {
    vault_by_issuer: bool,
    verify_again: bool,
    import_open: bool,
    import_err: Option<(String, String, String)>,
    grant_file_seen: Option<(String, Option<Result<String, crate::fault::Fault>>)>,
    confirm: Option<U4Confirm>,
    relicense_signed: bool,
    relicense_reveal: bool,
    grant_in_gate: bool,
    verify_autorun: bool,
    verify_asked: Option<String>,
    verify_refused: Option<crate::fault::Fault>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum U4Confirm {
    Relicense { grant: String },
}

use own::{own_delete_effect, own_net_key, own_stage, own_start, task_key, task_long, OwnBk};

mod about;
mod backup;
mod check;
mod chrome;
mod common;
mod drive;
mod faces;
mod gate;
mod grants;
mod home;
mod landing;
mod identity;
mod kit;
mod ledger;
mod mirror;
mod modals;
mod network;
mod pages;
mod probes;
mod others;
mod persona;
mod reroot;
mod own;
mod settings;
mod skeleton;
mod shared;
mod vault;
mod verify;
mod watch;
mod wizard;
mod wizard_own;
mod works;
pub use self::faces::*;
pub use self::drive::*;
pub use self::probes::*;
pub use self::persona::{model as persona_model, other_block, probe_persona_run, Block as PersonaBlock, Does as PersonaDoes, Line as PersonaLine, Model as PersonaModel, PersonaOut, PersonaStep, Press as PersonaPress, Sheet as PersonaSheet, Value as PersonaValue};
use self::pages::*;
use self::chrome::{stagger, tab_body};
use self::common::*;
use self::others::*;
use self::shared::*;

pub(super) fn backup_content(s: &crate::backup::Summary) -> String {
    crate::lang::filln(Key::BackupContent, &[&s.identities.to_string(), &s.entries.to_string(), &s.records.to_string()])
}

#[cfg(test)]
mod tests {
    #[test]
    fn each_opening_of_the_identity_card_starts_from_an_empty_form() {
        let mut ux = super::Ux::default();
        ux.id_new_label = "named in the first run".into();
        ux.id_open(super::IdModal::New);
        assert_eq!(ux.id_new_label, "", "a name the first run left is not carried into the card");
        ux.id_new_label = "half typed".into();
        ux.id_close();
        ux.id_open(super::IdModal::New);
        assert_eq!(ux.id_new_label, "", "a card closed half filled opens empty");
        ux.id_new_label = "half typed".into();
        ux.id_open(super::IdModal::Import);
        assert_eq!((ux.id_new_label.as_str(), ux.id_modal.clone()), ("", Some(super::IdModal::Import)), "one card opened over another starts empty");
    }

    #[test]
    fn a_mismatch_is_said_by_its_kind_never_its_token() {
        use crate::verifyx::{Mismatch, Rejected};
        let refused = Rejected::plain("/p/x.entry", "UNREADABLE");
        let cases = [
            (Mismatch::Kit { verdict: "E_KIT_FILE", subject: "/k/files/a.txt" }, "a.txt"),
            (Mismatch::Invalid { path: "entries/y", token: "BAD_SIG" }, "entries/y"),
            (Mismatch::Rejected(&refused), "x.entry"),
            (Mismatch::Unanchored("0x02"), "0x02"),
            (Mismatch::WorkAbsent, ""),
        ];
        for (m, value) in &cases {
            let said = crate::lang::both_of(|| super::verify::mismatch_said(m));
            for words in [&said.zh, &said.en] {
                assert!(!words.is_empty() && words.contains(value), "{m:?}: {words}");
                for token in ["E_KIT_FILE", "BAD_SIG", "UNREADABLE", "unanchored", "work absent"] {
                    assert!(!words.contains(token), "{m:?} says {token}: {words}");
                }
            }
        }
    }

    #[test]
    fn the_grant_file_toast_names_the_folder_it_stands_in() {
        use crate::lang::{both_of, fill2, Key};
        let file = "/srv/exports/grant-ab.zkgrant";
        assert_eq!(super::folder_of(file), "exports");
        let said = both_of(|| super::grant_file_said(file));
        let want = both_of(|| fill2(Key::SaidGrantFile, "grant-ab.zkgrant", "exports"));
        assert_eq!((said.zh, said.en), (want.zh, want.en));
    }

    fn row(kind: zikaron_glue::book::Kind, raw: &str) -> crate::ledgerx::Row {
        crate::ledgerx::Row {
            law: zikaron_glue::book::Law::Two,
            seq: 3,
            kind,
            id: format!("0x{}", "ab".repeat(32)),
            prev: None,
            author: format!("0x{}", "cd".repeat(20)),
            summary: String::new(),
            lamp: crate::ledgerx::Lamp::Landed,
            tx: None,
            bytes: 0,
            work: None,
            facts: crate::ledgerx::Facts { raw_type: raw.to_string(), ..Default::default() },
            anchored_at: None,
        }
    }

    #[test]
    fn the_type_row_says_the_kind_in_words() {
        use zikaron_glue::book::Kind as K;
        assert_eq!(super::type_words(&row(K::History, "history")), (super::Key::TagAnchor, None));
        assert_eq!(super::type_words(&row(K::Grant, "grant")), (super::Key::KindGrant, None));
        assert_eq!(super::type_words(&row(K::Revocation, "revocation")), (super::Key::KindRevocation, None));
        assert_eq!(super::type_words(&row(K::Retraction, crate::retractx::ENTRY_TYPE)), (super::Key::V2KindRetraction, None));
        assert_eq!(super::type_words(&row(K::Other, "someone-elses-type")), (super::Key::KindOther, Some("someone-elses-type".to_string())));
        for k in [K::Genesis, K::History, K::Grant, K::Revocation, K::Adoption, K::Succession, K::Migration, K::Annotation] {
            let (key, raw) = super::type_words(&row(k, k.as_str()));
            assert!(raw.is_none(), "{k:?}: no type as written for a known kind");
            assert_ne!(crate::lang::t(key), k.as_str(), "{k:?}: said in words, not by its program name");
        }
    }

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

pub fn held_said(added: usize, renewed: usize, refused: usize) -> String {
    let (a, n, r) = (added.to_string(), renewed.to_string(), refused.to_string());
    match (renewed, refused) {
        (0, 0) => fill1(Key::SaidHeldPlain, &a),
        (_, 0) if added == 0 => fill1(Key::SaidHeldRenewedN, &n),
        (_, 0) => fill2(Key::SaidHeldAndRenewed, &a, &n),
        (0, _) => fill2(Key::SaidHeldPartly, &a, &r),
        _ => fill3(Key::SaidHeldRenewedPartly, &a, &n, &r),
    }
}
