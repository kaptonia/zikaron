//! The shell of the window: the side rail, the toolbar with each view's history, the page body with its
//! entrance, the full-window layers (the first-run wizard and the passcode gate), the sheets above them, the
//! whole-window drop, and the keyboard.
//!
//! No page is named here: histories, the rail's list and the page area are the shell's own
//! (`zikaron_ui::nav`), and every page, place and route comes from this product's table (`Pages`, given in
//! `pages`). Every button goes through `action::apply`; nothing here decides anything about the ledger.

use super::*;
use zikaron_ui::nav::{self as shell_nav, Pages};

/// This product's place and route, as the shell knows them (through its table).
type P = <Win as Pages>::Place;
type R = <Win as Pages>::Route;

impl Win {
    // ─── History ───

    /// The current view's history (a view seen for the first time stands on its own page).
    pub(super) fn hist(&mut self) -> &zikaron_ui::nav::History<R> {
        shell_nav::hist(self)
    }

    /// The page on screen.
    pub(super) fn route(&mut self) -> R {
        shell_nav::route(self)
    }

    /// Go into a page of the current view.
    pub(super) fn push(&mut self, r: R, now: f64) {
        shell_nav::push(self, r, now);
    }

    /// Back one page in the current view.
    pub(super) fn back(&mut self, now: f64) {
        shell_nav::back(self, now);
    }

    /// Forward one page in the current view.
    pub(super) fn fwd(&mut self, now: f64) {
        shell_nav::fwd(self, now);
    }

    /// A segment of the view's own page picked on it: the page stays, the segment's thumb slides and what is under
    /// it swaps.
    pub(super) fn seg_tab(&mut self, place: P, now: f64) {
        shell_nav::seg_tab(self, place, now);
    }

    /// A rail item: the same view goes back to its own page; another view shows where that view was left.
    fn rail_to(&mut self, place: P, now: f64) {
        shell_nav::rail_to(self, place, now);
    }

    /// Whether the rail is on screen (not under the gate or the first-run wizard).
    pub(super) fn rail_shown(&self) -> bool {
        !self.shell.vault.gate_up() && self.ux.wizard.is_none()
    }

    // ─── The frame ───

    /// One frame of the shell: rail, toolbar and page; the wizard and the gate over them; sheets over all.
    pub(super) fn chrome(&mut self, ui: &mut egui::Ui, now: f64) {
        let ctx = &ui.ctx().clone();
        self.ux.now = now;
        if self.ux.nav.place.is_none() {
            let start = self.ux.nav.hist.get(&self.ux.nav.stack).map(|h| h.bottom().clone());
            match start.and_then(|r| self.root_place(&r)) {
                Some(p) => self.ux.nav.place = Some(p),
                None => {
                    let h = self.history_of(self.start_place());
                    self.ux.nav.land(h);
                    let bottom = self.hist().bottom().clone();
                    if let Some(p) = self.root_place(&bottom) {
                        self.ux.nav.place = Some(p);
                    }
                }
            }
        }
        self.settle_seat();
        // Apply the language chosen in settings once at startup.
        if !self.ux.lang_applied {
            self.ux.lang_applied = true;
            if let Some(l) = self.shell.speaks() {
                if l != crate::lang::lang() {
                    crate::lang::set(l);
                }
            }
            crate::when::set(self.shell.settings.zone.unwrap_or(crate::when::Zone::Utc));
        }
        self.follow_page(now);
        // A store that turns into no store (reset, or the machine set aside) starts setup over: the wizard is asked
        // again.
        let absent = self.shell.vault.absent();
        if self.ux.vault_absent == Some(false) && absent {
            self.ux.wizard_asked = false;
        }
        self.ux.vault_absent = Some(absent);
        // Whether the wizard opens by itself is asked once, and only with the gate down: while the vault is
        // locked the key cannot be read, and a locked machine would read as one without a key.
        let gate = self.shell.vault.gate_up();
        if !self.ux.wizard_asked && !gate {
            self.ux.wizard_asked = true;
            self.wizard_by_itself();
        }
        if self.rail_shown() {
            self.chrome_rail(ui, now);
        }
        // A page that takes drops in its own zones says so as it draws (`takes_drops`).
        self.ux.page_drops = false;
        self.chrome_page(ui, now);
        // The full-window layers: the first-run wizard, and the passcode gate above it.
        if self.ux.wizard.is_some() && !gate {
            self.wizard_layer(ctx, now);
        } else {
            // However the wizard is left (its way out, its last step), it goes at once and the window comes in as
            // a page does.
            zikaron_ui::covers::gone(ctx, "wizard");
        }
        self.gate(ctx, now);
        // Sheets lie above everything, the gate and the wizard included.
        self.sheets(ctx, now);
        self.whole_drop(ctx, now);
    }

    /// The side rail.
    fn chrome_rail(&mut self, ui: &mut egui::Ui, now: f64) {
        let ctx = &ui.ctx().clone();
        let role = self.shell.settings.role;
        let in_settings = self.ux.nav.stack == self.stack_of(self.settings_place());
        let mut picked: Option<P> = None;
        let mut lock = false;
        let mut switch = false;
        let mut settings = false;
        let mut chip: Option<egui::Rect> = None;
        let mut status = rail::Status::default();
        let (words, voice, progress, sync) = self.status_say();
        let pin_set = !self.shell.vault.absent();
        let (id_name, id_kind) = self.id_chip_words();
        let panel = egui::Panel::left("rail")
            .exact_size(tk::RAIL_W)
            .resizable(false)
            .show_separator_line(false)
            .frame(rail::panel_frame())
            .show_inside(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                // On macOS the top of the rail is the window's handle (there is no title bar): dragging here
                // drags the window. Elsewhere the system's title bar does that.
                #[cfg(target_os = "macos")]
                {
                    let full = ui.max_rect();
                    let grip = egui::Rect::from_min_max(egui::pos2(full.left() - tk::RAIL_PAD, full.top() - tk::RAIL_TOP), egui::pos2(full.right() + tk::RAIL_PAD, full.top()));
                    grip_acts(ctx, &ui.interact(grip, ui.id().with("rail-grip"), egui::Sense::click_and_drag()));
                }
                let r = rail::id_chip(ui, &id_name, &id_kind);
                if r.clicked() {
                    menu::toggle(ctx, egui::Id::new("zikaron-id-menu"));
                }
                chip = Some(r.rect);
                // The seat a switch goes to shows at once, while the switch runs in the background (`Shell::seat_wish`).
                let cur = usize::from(self.shell.seat_wish.unwrap_or(role) == crate::roles::Role::Grantee);
                if rail::seat(ui, [t(Key::IdSeatAuthor), t(Key::IdSeatGrantee)], cur).is_some() {
                    switch = true;
                }
                let foot_h = tk::RAIL_ITEM_H * if pin_set { 2.0 } else { 1.0 } + 2.0 + 13.0 + 32.0;
                let list_h = (ui.available_height() - foot_h).max(tk::RAIL_ITEM_H);
                picked = shell_nav::rail_list(ui, &*self, list_h);
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    status = rail::status(ui, &words, voice, progress, sync, t(Key::NavRefresh));
                    if pin_set && rail::foot_item(ui, Glyph::Lock, t(Key::NavLock), false, "\u{2318}L").clicked() {
                        lock = true;
                    }
                    if rail::foot_item(ui, Glyph::Gear, t(Key::NavSettings), in_settings, "").clicked() {
                        settings = true;
                    }
                    rail::foot_rule(ui);
                });
            });
        rail::edge(ctx, panel.response.rect);
        if let Some(r) = chip {
            self.id_menu(ctx, r, panel.response.rect, now);
        }
        if lock {
            self.lock_now(now);
        }
        if switch {
            self.act(Action::SwitchRole, now);
            shell_nav::home(self, motion::Entry::Root, now);
        }
        if status.sync_clicked {
            self.refresh_chain(now);
        }
        if status.line_clicked {
            if let Some(k) = self.running_task() {
                self.view_task(k as u64, now);
            }
        }
        if settings {
            self.rail_to(self.settings_place(), now);
        }
        if let Some(p) = picked {
            self.rail_to(p, now);
        }
    }

    /// Lock: back to the passcode gate; the master key is wiped at once, and nothing typed survives.
    pub(super) fn lock_now(&mut self, now: f64) {
        self.act(Action::Lock, now);
        self.ux.gate_clear();
        self.sheets_clear();
    }

    /// The identity chip's two lines: the name (or "unnamed") and the kind.
    fn id_chip_words(&self) -> (String, String) {
        let row = self.shell.identities.as_ref().and_then(|r| r.now().map(|(x, _)| x.clone()));
        match row {
            Some(r) => (if r.label.trim().is_empty() { t(Key::Unnamed).to_string() } else { r.label.clone() }, t(id_kind_key(r.kind())).to_string()),
            None => (t(Key::Unnamed).to_string(), t(Key::IdNone).to_string()),
        }
    }

    /// The identity menu under the chip: switch identity (each identity's name over its kind, the one in use
    /// saying so), new identity, import key, identity keys. No lock here (the rail has its own).
    /// It stays inside the rail: as far from the rail's right edge as the chip is from its left, a long name cut.
    fn id_menu(&mut self, ctx: &egui::Context, anchor: egui::Rect, rail: egui::Rect, now: f64) {
        let id = egui::Id::new("zikaron-id-menu");
        if !menu::is_open(ctx, id) {
            return;
        }
        let rows: Vec<crate::identity::Row> = self.shell.identities.as_ref().map(|r| r.rows.clone()).unwrap_or_default();
        let current = self.shell.identities.as_ref().and_then(|r| r.now().map(|(x, _)| x.id.clone()));
        let primary = self.shell.primary.as_ref().map(|(p, _)| p.clone());
        // The menu's width less its own 10 of padding; a row takes its label and 24 more.
        let min_w = (rail.width() - 2.0 * (anchor.left() - rail.left()).max(0.0) - 10.0).max(120.0);
        // The identity list: each identity is two lines, its name and its
        // kind, with "primary" and "in use" at the end of the second line.
        let names: Vec<String> = rows.iter().map(|r| elide_in(ctx, if r.label.trim().is_empty() { t(Key::Unnamed) } else { r.label.as_str() }, Type::Key.font(), min_w - 24.0)).collect();
        let tags: Vec<Vec<&str>> = rows
            .iter()
            .map(|r| {
                let mut v = Vec::new();
                if primary.as_deref().map(|p| p.eq_ignore_ascii_case(&r.id)).unwrap_or(false) {
                    v.push(t(Key::IdPrimaryTag));
                }
                if current.as_deref() == Some(r.id.as_str()) {
                    v.push(t(Key::IdIsCurrent));
                }
                v
            })
            .collect();
        // A title, the identities (arrows and return move among them and pick one), then the actions in two
        // groups.
        let mut items: Vec<menu::Item> = vec![menu::Item::Head(t(Key::IdSwitchTitle))];
        for (i, r) in rows.iter().enumerate() {
            items.push(menu::Item::Who(menu::Who { name: &names[i], kind: t(id_kind_key(r.kind())), tags: &tags[i], current: current.as_deref() == Some(r.id.as_str()) }));
        }
        items.push(menu::Item::Sep);
        items.push(menu::row(t(Key::IdNewTitle)));
        items.push(menu::row(t(Key::IdImportTitle)));
        items.push(menu::Item::Sep);
        items.push(menu::row(t(Key::SetKeys)));
        if let Some(i) = menu::show(ctx, id, anchor, true, min_w, &items) {
            let n = rows.len();
            if i >= 1 && i <= n {
                // Whether the press moves anything is the action's to say; only one that moved goes home.
                let r = &rows[i - 1];
                if !matches!(self.act(Action::SwitchIdentity { id: r.id.clone() }, now), Applied::IdentityKept { .. }) {
                    shell_nav::home(self, motion::Entry::Fade, now);
                }
            } else if i == n + 2 {
                self.id_layer_open(IdModal::New);
                self.act(Action::NewIdentity, now);
            } else if i == n + 3 {
                self.id_layer_open(IdModal::Import);
            } else if i == n + 5 {
                self.go(self.keys_place(), now);
            }
        }
    }

    /// The status line's text, voice, progress and the sync button's state. A long task in progress comes first
    /// ("in progress: …" with its bar), then syncing, a failed sync, the last sync time, or never.
    fn status_say(&mut self) -> (String, rail::Voice, Option<Option<f32>>, rail::Sync) {
        let sync = if !self.ux.syncing.is_empty() {
            rail::Sync::Busy
        } else if let Some(at) = self.ux.synced_at {
            rail::Sync::Done { at }
        } else {
            rail::Sync::Idle
        };
        if let Some(k) = self.running_task() {
            let others = self.ux.asked.iter().filter(|x| **x != k && self.shell.tasks.in_flight(**x)).count();
            let name = t(self.task_word(k));
            let words = if others > 0 { fill2(Key::NavDoingMore, name, &(others + 1).to_string()) } else { fill1(Key::NavDoing, name) };
            return (words, rail::Voice::Task, Some(crate::task::stage(k).and_then(|s| s.frac())), sync);
        }
        if !self.ux.syncing.is_empty() {
            return (t(Key::NavReading).to_string(), rail::Voice::Quiet, None, sync);
        }
        if self.shell.status.is_some() {
            return (t(Key::NavReadFailed).to_string(), rail::Voice::Bad, None, sync);
        }
        let words = match self.shell.chain_read_at {
            Some(at) => {
                let ago = (self.ux.now - at).max(0.0) as u64;
                let zone = if self.shell.settings.zone.unwrap_or(crate::when::Zone::Utc) == crate::when::Zone::Utc { " UTC" } else { "" };
                let wall = wall_secs();
                fill1(Key::NavReadAt, &format!("{}{zone}", crate::when::clock_or_day(wall.saturating_sub(ago), wall)))
            }
            None => t(Key::NavReadNever).to_string(),
        };
        (words, rail::Voice::Quiet, None, sync)
    }

    /// Whether the status line at the rail's foot reports a task of kind `k` while it runs (every long task the
    /// person starts, wherever the rail is drawn): what it reports is not drawn again in the page.
    pub(super) fn status_reports(&self, k: crate::task::Kind) -> bool {
        task_long(k) && self.rail_shown()
    }

    /// The long task the person started that is still running (the latest one).
    pub(super) fn running_task(&self) -> Option<crate::task::Kind> {
        self.ux.asked.iter().rev().find(|k| self.shell.tasks.in_flight(**k) && task_long(**k)).copied()
    }

    /// Whether the person left the page a task was started on.
    pub(super) fn task_away(&self, k: crate::task::Kind) -> bool {
        match self.ux.origin.iter().find(|(x, _, _)| *x == k) {
            Some((_, stack, h)) => self.ux.nav.stack != *stack || self.ux.nav.hist.get(stack).map(|now| now.cur != h.cur).unwrap_or(true),
            None => false,
        }
    }

    /// Go back to where a task was started ("view" on its toast, the rail's task line).
    pub(super) fn view_task(&mut self, tag: u64, now: f64) {
        let Some((_, stack, h)) = self.ux.origin.iter().find(|(x, _, _)| *x as u64 == tag).cloned() else { return };
        self.ux.nav.land((stack, h));
        shell_nav::entered(self, motion::Entry::Root, now);
    }

    /// Refresh chain state (the sync button): the chain query, and the self-audit or the vault re-check for the
    /// seat. The interface does not wait. With no node configured it says where to add one.
    pub(super) fn refresh_chain(&mut self, now: f64) {
        if self.shell.endpoints.is_empty() {
            self.toasts.say_full(t(Key::NavNoNodes), "", "", Tone::Bad, now);
            return;
        }
        let mut wants = vec![Action::ReadChain];
        match self.shell.settings.role {
            crate::roles::Role::Author => {
                if self.shell.rooted {
                    wants.push(Action::Audit);
                }
            }
            crate::roles::Role::Grantee => wants.push(Action::ReviewVault),
        }
        self.ux.syncing.clear();
        self.ux.synced_at = None;
        for a in wants {
            match apply(&mut self.shell, a) {
                // The round reports once, when all of it has landed (`sync_landed`).
                Applied::Started(k) => self.ux.syncing.push(k),
                Applied::Refused(k) => self.toasts.say(fill1(Key::SaidInFlight, t(self.task_word(k))), Tone::Bad, now),
                Applied::Trouble(f) if crate::watchx::is_network(&f) => self.say_fault(&f, now),
                _ => {}
            }
        }
    }

    /// A sync round's landings: when the last of its tasks lands, one toast reports both readings ("key balance
    /// · ledger check"), and the button shows its check mark.
    pub(super) fn sync_landed(&mut self, landed: &[crate::task::Outcome], now: f64) {
        if self.ux.syncing.is_empty() {
            return;
        }
        for o in landed {
            if self.ux.syncing.contains(&o.kind) {
                self.ux.syncing.retain(|k| *k != o.kind);
                self.ux.sync_said.push(o.result.clone());
            }
        }
        if !self.ux.syncing.is_empty() {
            return;
        }
        let results = std::mem::take(&mut self.ux.sync_said);
        let mut parts: Vec<String> = Vec::new();
        let mut bad: Option<crate::fault::Fault> = None;
        let mut failed = false;
        for r in results {
            match r {
                Ok(Done::Chain { gas_wei: Some(w), .. }) => parts.push(fill1(Key::SaidChain, &eth_held(w))),
                Ok(Done::Chain { gas_wei: None, .. }) => {
                    failed = true;
                    parts.push(t(Key::SaidChainNone).to_string());
                }
                Ok(Done::Audited { label, complete, entries, .. }) => {
                    failed |= !complete;
                    parts.push(fill2(Key::SaidAudited, &label_human(&label), &entries.to_string()));
                }
                Ok(Done::Reviewed { cards, .. }) => parts.push(fill1(Key::SaidReviewed, &cards.len().to_string())),
                Ok(_) => {}
                Err(f) => {
                    failed = true;
                    bad = Some(f);
                }
            }
        }
        match bad {
            Some(f) => self.say_fault(&f, now),
            None => {
                if !failed {
                    self.ux.synced_at = Some(now);
                }
                self.toasts.say(parts.join(" \u{b7} "), if failed { Tone::Bad } else { Tone::Note }, now);
            }
        }
    }

    /// The page area: toolbar and body (the shell's, over this product's pages).
    fn chrome_page(&mut self, ui: &mut egui::Ui, now: f64) {
        let ctx = &ui.ctx().clone();
        let covered = !self.rail_shown();
        // Under a cover (the wizard, the gate) the page paints no ground: the cover paints its own, and the
        // wizard's column lets the system's material through.
        let frame = if covered { egui::Frame::NONE } else { rail::page_frame() };
        egui::CentralPanel::default().frame(frame).show_inside(ui, |ui| {
            if covered {
                return;
            }
            let full = ui.max_rect();
            let grip = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), 10.0));
            grip_acts(ctx, &ui.interact(grip, ui.id().with("page-grip"), egui::Sense::click_and_drag()));
            shell_nav::page_area(ui, self, (t(Key::NavBack), t(Key::NavFwd)), now);
        });
    }

    // ─── Keyboard, drops ───

    /// ⌘1–⌘6 the rail's items, ⌘[ ⌘] back and forward, ⌘, settings, ⌘L lock (with a passcode set). Not under
    /// the gate, the wizard, or a sheet (Esc there closes the sheet or menu).
    pub(super) fn shortcuts(&mut self, ctx: &egui::Context, now: f64) {
        if !self.rail_shown() || sheet::up(ctx) || self.any_sheet() {
            return;
        }
        let pressed = |k: egui::Key| chord(ctx, k);
        if pressed(egui::Key::OpenBracket) {
            self.back(now);
        }
        if pressed(egui::Key::CloseBracket) {
            self.fwd(now);
        }
        if pressed(egui::Key::Comma) {
            self.rail_to(self.settings_place(), now);
        }
        if !self.shell.vault.absent() && pressed(egui::Key::L) {
            self.lock_now(now);
        }
        let items: Vec<P> = self.rail().iter().flat_map(|g| g.items.iter().map(|i| i.place)).collect();
        let keys = [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4, egui::Key::Num5, egui::Key::Num6];
        for (i, k) in keys.iter().enumerate() {
            if pressed(*k) {
                if let Some(p) = items.get(i) {
                    self.rail_to(*p, now);
                }
            }
        }
    }

    /// Whether the page on screen takes dropped files in its own zones (the whole-window drop stands aside): what
    /// the page declared as it drew this frame ([`Win::takes_drops`]), never a list of pages kept here.
    fn page_takes_drops(&mut self) -> bool {
        self.ux.page_drops
    }

    /// Declared by a page while it draws: it takes dropped files in its own zones this frame, so the
    /// whole-window drop stands aside.
    pub(super) fn takes_drops(&mut self) {
        self.ux.page_drops = true;
    }

    /// The whole-window drop: while a file is dragged over a page that takes drops as a whole, the veil covers
    /// the window; when dropped, the author seat opens the new-record sheet with the file and the grantee seat
    /// goes to record verification. Several files at once are refused with the usual message.
    fn whole_drop(&mut self, ctx: &egui::Context, now: f64) {
        let open = self.rail_shown() && !self.any_sheet() && !self.page_takes_drops();
        let author = self.shell.settings.role == crate::roles::Role::Author;
        let (title, note) = if author { (t(Key::V2NewAnchor), t(Key::U3DropTitle)) } else { (t(Key::V2TabVerifyWork), t(Key::KitvDrop)) };
        drop::veil(ctx, open && drop::dragging(ctx), title, note);
        if !open {
            return;
        }
        let got = drop::take_dropped(ctx);
        match got.as_slice() {
            [] => {}
            [one] => {
                if author {
                    self.u3_new_anchor_open();
                    self.take_for_record(vec![one.clone()], now);
                } else {
                    self.typed.vf_path = one.clone();
                    self.ux.u4.verify_autorun = true;
                    self.go(self.record_check_place(), now);
                }
            }
            _ => self.toasts.say(t(Key::U3OneAtATime), Tone::Bad, now),
        }
    }

}

/// A block of a page that enters one after another (the stagger of lists and detail pages), keyed on the
/// page's entrance.
pub(super) fn stagger<Out>(ui: &mut egui::Ui, i: usize, add: impl FnOnce(&mut egui::Ui) -> Out) -> Out {
    let key = shell_nav::entry_key(ui.ctx());
    motion::stagger(ui, egui::Id::new(("zikaron-stagger", i)), key, i, add)
}

/// What is under a segment of a view's own page: swapped in when the segment changes on the page, and on the page's
/// own entrance coming with the page rather than swapping in over it.
pub(super) fn tab_body<Out>(ui: &mut egui::Ui, salt: &str, key: u64, add: impl FnOnce(&mut egui::Ui) -> Out) -> Out {
    let id = egui::Id::new(("zikaron-tab-body", salt));
    let entry = shell_nav::entry_key(ui.ctx());
    if ui.ctx().data(|d| d.get_temp::<u64>(id.with("entry"))) != Some(entry) {
        ui.ctx().data_mut(|d| d.insert_temp(id.with("entry"), entry));
        motion::set_age(ui.ctx(), id, key, 3600.0);
    }
    motion::swap(ui, id, key, add)
}

/// Tasks that run long enough to show on a button and on the rail.
fn task_long(k: crate::task::Kind) -> bool {
    use crate::task::Kind;
    !matches!(k, Kind::Ledger | Kind::Grants | Kind::Held | Kind::Vet | Kind::Vault)
}

/// The one way the window reads a shortcut: ⌘ (Ctrl off macOS) with `k`, taken so nothing else reads it too.
///
/// The key read is the one egui reports for the press; the windowing layer reports the layout's own key when
/// egui knows it and the physical key otherwise. So on a layout without Latin letters (Cyrillic, Greek, kana)
/// ⌘ with the key where L sits arrives as L and locks. There is no further fallback to the physical key: on a
/// Latin layout that moves letters (Dvorak) the key where L sits types another letter, and ⌘ with it is that
/// letter's shortcut, not ⌘L.
pub(super) fn chord(ctx: &egui::Context, k: egui::Key) -> bool {
    ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, k))
}

/// What the window's handles do (the top of the rail on macOS, the band over the page): dragging moves the
/// window; a double click maximizes it, or puts a maximized one back, as a title bar does.
fn grip_acts(ctx: &egui::Context, r: &egui::Response) {
    if r.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    if r.double_clicked() {
        let maximized = ctx.input(|i| i.viewport().maximized).unwrap_or(false);
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
}

/// `s` cut with an ellipsis to fit `max_w` in `font` (for words laid out without a ui of their own).
fn elide_in(ctx: &egui::Context, s: &str, font: egui::FontId, max_w: f32) -> String {
    let width = |w: &str| ctx.fonts_mut(|f| f.layout_no_wrap(w.to_string(), font.clone(), egui::Color32::BLACK).size().x);
    if width(s) <= max_w {
        return s.to_string();
    }
    let cs: Vec<char> = s.chars().collect();
    let cut = |keep: usize| cs[..keep].iter().collect::<String>() + "\u{2026}";
    let (mut lo, mut hi) = (0usize, cs.len().saturating_sub(1));
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if width(&cut(mid)) <= max_w {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    cut(lo)
}
