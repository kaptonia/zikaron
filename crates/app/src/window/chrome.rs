//! The shell of the window: the side rail, the toolbar with each view's history, the page body with its
//! entrance, the full-window layers (the first-run wizard and the passcode gate), the sheets above them, the
//! whole-window drop, and the keyboard.
//!
//! Every destination comes from the `nav` table and every button goes through `action::apply`; nothing here
//! decides anything about the ledger.

use super::*;

impl Win {
    // ─── History ───

    /// The current view's history (a view seen for the first time stands on its own page).
    pub(super) fn hist(&mut self) -> &History {
        let stack = self.ux.stack;
        let root = self.root_of(stack);
        self.ux.hist.entry(stack).or_insert_with(|| History::at(root))
    }

    fn hist_mut(&mut self) -> &mut History {
        let stack = self.ux.stack;
        let root = self.root_of(stack);
        self.ux.hist.entry(stack).or_insert_with(|| History::at(root))
    }

    /// The page on screen.
    pub(super) fn route(&mut self) -> Route {
        self.hist().cur.clone()
    }

    /// A view's own page when it has no history yet.
    fn root_of(&self, stack: Stack) -> Place {
        match stack {
            Stack::Home => Place::Home,
            Stack::Settings => Place::SettingsHome,
            Stack::View(v) => crate::nav::rail(self.shell.settings.role)
                .iter()
                .flat_map(|g| g.items.iter())
                .find_map(|i| match i.place {
                    Place::View(x, t) if x == v => Some(Place::View(x, t)),
                    _ => None,
                })
                .unwrap_or(Place::View(v, 0)),
        }
    }

    /// The page changed: set how it enters, restart the entrance animation, sync the shell's page (tests and the
    /// trace channel read it), and drop what belonged to the previous page.
    fn entered(&mut self, how: motion::Entry, now: f64) {
        self.ux.entry = how;
        self.ux.entry_key = self.ux.entry_key.wrapping_add(1);
        self.ux.scroll_y = 0.0;
        self.ux.u3.drop_armed = None;
        let route = self.route();
        let bottom = self.hist().bottom().clone();
        if let Route::Root(p) = &bottom {
            self.ux.place = Some(*p);
        }
        let page = route_page(&route, self.ux.place.unwrap_or(Place::Home), self.shell.settings.role);
        if let Some(p) = page {
            if self.shell.page != p {
                self.auto(Action::Show(p), now);
            }
        }
        // A detail page reads its entry's bytes once, off the frame (the action layer reads the disk).
        match &route {
            Route::Work(id) | Route::Entry(id) | Route::Grant(id) | Route::Pending(id) => {
                if self.opened.as_ref().map(|d| !d.id.eq_ignore_ascii_case(id)).unwrap_or(true) {
                    self.auto(Action::OpenEntry { id: id.clone() }, now);
                }
            }
            _ => {}
        }
    }

    /// Go into a page of the current view.
    pub(super) fn push(&mut self, r: Route, now: f64) {
        crate::nav::History::push(self.hist_mut(), r);
        self.entered(motion::Entry::Push, now);
    }

    /// Back one page in the current view.
    pub(super) fn back(&mut self, now: f64) {
        if self.hist_mut().back() {
            self.entered(motion::Entry::Pop, now);
        }
    }

    /// Forward one page in the current view.
    pub(super) fn fwd(&mut self, now: f64) {
        if self.hist_mut().fwd() {
            self.entered(motion::Entry::Push, now);
        }
    }

    /// A tab of the view's own page (the segmented control in the toolbar): the page is replaced in place and
    /// fades.
    pub(super) fn set_tab(&mut self, place: Place, now: f64) {
        let h = self.hist_mut();
        h.cur = Route::Root(place);
        h.back.clear();
        h.fwd.clear();
        self.entered(motion::Entry::Fade, now);
    }

    /// Land on a place with a fresh history (links on pages, gaps, tests): its view becomes current and
    /// shows that place, with the view's own page behind it when the place is a pushed page.
    pub(super) fn go(&mut self, place: Place, now: f64) {
        if let Place::Page(p) = place {
            self.arrive(p);
        }
        let role = self.shell.settings.role;
        let (stack, h) = crate::nav::history_of(place, role);
        let same = self.ux.stack == stack && self.ux.hist.get(&stack) == Some(&h);
        self.ux.stack = stack;
        self.ux.hist.insert(stack, h);
        if !same {
            self.entered(motion::Entry::Root, now);
        }
    }

    /// A rail item: the same view goes back to its own page; another view shows where that view was left.
    fn rail_to(&mut self, place: Place, now: f64) {
        let stack = crate::nav::stack_of(place);
        if self.ux.stack == stack {
            let root = self.root_of(stack);
            let h = self.hist_mut();
            if h.has_history() || !h.cur.is_root() {
                h.root(root);
                self.entered(motion::Entry::Pop, now);
            }
            return;
        }
        self.ux.stack = stack;
        let root = self.root_of(stack);
        self.ux.hist.entry(stack).or_insert_with(|| History::at(root));
        self.entered(motion::Entry::Root, now);
    }

    /// The intent of pages folded into a parent item. Exhaustive: each new `Page` makes the compiler ask
    /// whether it has an intent here.
    pub(super) fn arrive(&mut self, p: Page) {
        match p {
            Page::Audit => self.ux.u3.audit_open = true,
            Page::Adopt => self.u3_form_open(U3Form::Adopt),
            Page::Succeed => self.u3_form_open(U3Form::Succeed),
            Page::Upstreams => self.ux.u4.vault_by_issuer = true,
            Page::FirstRun => {
                let step = self.progress().first_open().unwrap_or(crate::nav::Step::Pin);
                self.wizard_open(step);
            }
            Page::Ledger
            | Page::Anchoring
            | Page::Queue
            | Page::Grant
            | Page::Grants
            | Page::Revoke
            | Page::FirstWindow
            | Page::Kit
            | Page::Depth
            | Page::Reader
            | Page::Check
            | Page::Watch
            | Page::Diligence
            | Page::Verifier
            | Page::Delivery
            | Page::Vault
            | Page::Sentinel
            | Page::Relicense
            | Page::Badge
            | Page::Identity
            | Page::Archive
            | Page::Mirror
            | Page::Skeleton
            | Page::About => {}
        }
    }

    /// Whether the rail is on screen (not under the gate or the first-run wizard).
    pub(super) fn rail_shown(&self) -> bool {
        !self.shell.vault.gate_up() && self.ux.wizard.is_none()
    }

    // ─── The frame ───

    /// One frame of the shell: rail, toolbar and page; the wizard and the gate over them; sheets over all.
    pub(super) fn chrome(&mut self, ctx: &egui::Context, now: f64) {
        self.ux.now = now;
        if self.ux.place.is_none() {
            let start = self.ux.hist.get(&self.ux.stack).map(|h| h.bottom().clone());
            match start {
                Some(Route::Root(p)) => self.ux.place = Some(p),
                _ => {
                    let place = Place::Page(self.shell.page);
                    let (stack, h) = crate::nav::history_of(place, self.shell.settings.role);
                    self.ux.stack = stack;
                    self.ux.hist.insert(stack, h);
                    let bottom = self.hist().bottom().clone();
                    if let Route::Root(p) = bottom {
                        self.ux.place = Some(p);
                    }
                }
            }
        }
        // After switching seat, a view that seat lacks lands on home.
        if let Some(p) = self.ux.place {
            let settled = crate::nav::settle(p, self.shell.settings.role);
            if settled != p && matches!(settled, Place::Home) {
                self.ux.hist.clear();
                self.ux.stack = Stack::Home;
                self.ux.place = Some(Place::Home);
            }
        }
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
        // Whether the wizard opens by itself is asked once, and only with the gate down: while the vault is
        // locked the key cannot be read, and a locked machine would read as one without a key.
        let gate = self.shell.vault.gate_up();
        if !self.ux.wizard_asked && !gate {
            self.ux.wizard_asked = true;
            let pr = self.progress();
            if self.ux.wizard_forced || pr.wants_wizard(self.shell.settings.role) {
                self.wizard_open(pr.first_open().unwrap_or(crate::nav::Step::Pin));
            }
        }
        if self.rail_shown() {
            self.chrome_rail(ctx, now);
        }
        self.chrome_page(ctx, now);
        // The full-window layers: the first-run wizard, and the passcode gate above it.
        if self.ux.wizard.is_some() && !gate {
            self.wizard_layer(ctx, now);
        }
        self.gate(ctx, now);
        // Sheets lie above everything, the gate and the wizard included.
        self.sheets(ctx, now);
        self.whole_drop(ctx, now);
    }

    /// The side rail.
    fn chrome_rail(&mut self, ctx: &egui::Context, now: f64) {
        let role = self.shell.settings.role;
        let place = self.ux.place.unwrap_or(Place::Home);
        let lit = if self.ux.stack == Stack::Settings { None } else { crate::nav::lit(place, role) };
        let alerts = self.alert_count();
        let mut picked: Option<Place> = None;
        let mut lock = false;
        let mut switch = false;
        let mut settings = false;
        let mut chip: Option<egui::Rect> = None;
        let mut status = rail::Status::default();
        let (words, voice, progress, sync) = self.status_say();
        let pin_set = !self.shell.vault.absent();
        let (id_name, id_kind) = self.id_chip_words();
        let panel = egui::SidePanel::left("rail")
            .exact_width(tk::RAIL_W)
            .resizable(false)
            .show_separator_line(false)
            .frame(rail::panel_frame())
            .show(ctx, |ui| {
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
                let cur = usize::from(role == crate::roles::Role::Grantee);
                if rail::seat(ui, [t(Key::IdSeatAuthor), t(Key::IdSeatGrantee)], cur).is_some() {
                    switch = true;
                }
                let foot_h = tk::RAIL_ITEM_H * if pin_set { 2.0 } else { 1.0 } + 2.0 + 13.0 + 32.0;
                let list_h = (ui.available_height() - foot_h).max(tk::RAIL_ITEM_H);
                let groups = crate::nav::rail(role);
                let mut lines: Vec<rail::Line> = Vec::new();
                let mut places: Vec<Place> = Vec::new();
                let mut lit_i = None;
                for (gi, g) in groups.iter().enumerate() {
                    lines.push(rail::Line::Group(if gi == 0 { None } else { g.title.map(t) }));
                    for item in g.items {
                        if lit.map(|l| same_item(l, item.place)).unwrap_or(false) {
                            lit_i = Some(places.len());
                        }
                        let count = if matches!(item.place, Place::View(crate::nav::View::Alerts, _)) { alerts } else { 0 };
                        lines.push(rail::Line::Item { glyph: item.glyph, label: t(item.name), count });
                        places.push(item.place);
                    }
                }
                egui::ScrollArea::vertical().scroll_source(egui::scroll_area::ScrollSource { drag: false, ..egui::scroll_area::ScrollSource::ALL }).id_salt("rail-list").max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
                    if let Some(i) = rail::nav(ui, &lines, lit_i) {
                        picked = places.get(i).copied();
                    }
                });
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    status = rail::status(ui, &words, voice, progress, sync, t(Key::NavRefresh));
                    if pin_set && rail::foot_item(ui, Glyph::Lock, t(Key::NavLock), false, "\u{2318}L").clicked() {
                        lock = true;
                    }
                    if rail::foot_item(ui, Glyph::Gear, t(Key::NavSettings), self.ux.stack == Stack::Settings, "").clicked() {
                        settings = true;
                    }
                    rail::foot_rule(ui);
                });
            });
        rail::edge(ctx, panel.response.rect);
        if let Some(r) = chip {
            self.id_menu(ctx, r, now);
        }
        if lock {
            self.lock_now(now);
        }
        if switch {
            self.act(Action::SwitchRole, now);
            self.ux.hist.clear();
            self.ux.stack = Stack::Home;
            self.ux.place = Some(Place::Home);
            self.entered(motion::Entry::Root, now);
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
            self.rail_to(Place::SettingsHome, now);
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

    /// The identity menu under the chip: switch identity (each "kind · name", the current one marked), new
    /// identity, import key, identity keys. No lock here (the rail has its own).
    fn id_menu(&mut self, ctx: &egui::Context, anchor: egui::Rect, now: f64) {
        let id = egui::Id::new("zikaron-id-menu");
        if !menu::is_open(ctx, id) {
            return;
        }
        let rows: Vec<crate::identity::Row> = self.shell.identities.as_ref().map(|r| r.rows.clone()).unwrap_or_default();
        let current = self.shell.identities.as_ref().and_then(|r| r.now().map(|(x, _)| x.id.clone()));
        let primary = self.shell.primary.as_ref().map(|(p, _)| p.clone());
        // The identity list: each identity is two lines, its name and its
        // kind, with "primary" and "in use" at the end of the second line.
        let names: Vec<&str> = rows.iter().map(|r| if r.label.trim().is_empty() { t(Key::Unnamed) } else { r.label.as_str() }).collect();
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
        // The identity lens menu: a title, the identities (the one in use under the blue lens), then the
        // actions in two groups.
        let mut items: Vec<menu::Item> = vec![menu::Item::Head(t(Key::IdSwitchTitle))];
        for (i, r) in rows.iter().enumerate() {
            items.push(menu::Item::Who(menu::Who { name: names[i], kind: t(id_kind_key(r.kind())), tags: &tags[i], current: current.as_deref() == Some(r.id.as_str()) }));
        }
        items.push(menu::Item::Sep);
        items.push(menu::row(t(Key::IdDoNew)));
        items.push(menu::row(t(Key::IdDoImport)));
        items.push(menu::Item::Sep);
        items.push(menu::row(t(Key::SetKeys)));
        if let Some(i) = menu::show(ctx, id, anchor, true, 236.0, &items) {
            let n = rows.len();
            if i >= 1 && i <= n {
                let r = &rows[i - 1];
                if current.as_deref() != Some(r.id.as_str()) {
                    self.act(Action::SwitchIdentity { id: r.id.clone() }, now);
                    self.ux.hist.clear();
                    self.ux.stack = Stack::Home;
                    self.ux.place = Some(Place::Home);
                    self.entered(motion::Entry::Fade, now);
                }
            } else if i == n + 2 {
                self.id_layer_open(IdModal::New);
                self.act(Action::NewIdentity, now);
            } else if i == n + 3 {
                self.id_layer_open(IdModal::Import);
            } else if i == n + 5 {
                self.go(Place::Settings(Section::Keys), now);
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
                fill1(Key::NavReadAt, &format!("{}{zone}", hm_zone(wall_secs().saturating_sub(ago))))
            }
            None => t(Key::NavReadNever).to_string(),
        };
        (words, rail::Voice::Quiet, None, sync)
    }

    /// The long task the person started that is still running (the latest one).
    pub(super) fn running_task(&self) -> Option<crate::task::Kind> {
        self.ux.asked.iter().rev().find(|k| self.shell.tasks.in_flight(**k) && task_long(**k)).copied()
    }

    /// Whether the person left the page a task was started on.
    pub(super) fn task_away(&self, k: crate::task::Kind) -> bool {
        match self.ux.origin.iter().find(|(x, _, _)| *x == k) {
            Some((_, stack, h)) => self.ux.stack != *stack || self.ux.hist.get(stack).map(|now| now.cur != h.cur).unwrap_or(true),
            None => false,
        }
    }

    /// Go back to where a task was started ("view" on its toast, the rail's task line).
    pub(super) fn view_task(&mut self, tag: u64, now: f64) {
        let Some((_, stack, h)) = self.ux.origin.iter().find(|(x, _, _)| *x as u64 == tag).cloned() else { return };
        self.ux.stack = stack;
        self.ux.hist.insert(stack, h);
        self.entered(motion::Entry::Root, now);
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

    /// The page area: toolbar and body.
    fn chrome_page(&mut self, ctx: &egui::Context, now: f64) {
        let covered = !self.rail_shown();
        egui::CentralPanel::default().frame(rail::page_frame()).show(ctx, |ui| {
            if covered {
                return;
            }
            let full = ui.max_rect();
            let grip = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), 10.0));
            grip_acts(ctx, &ui.interact(grip, ui.id().with("page-grip"), egui::Sense::click_and_drag()));
            let route = self.route();
            let detail = !route.is_root() && matches!(route, Route::Work(_) | Route::Pending(_) | Route::Grant(_) | Route::Entry(_) | Route::Other(_) | Route::Held(_));
            zikaron_ui::probe::route(ctx, &route.name());
            if detail {
                zikaron_ui::probe::inner(ctx);
            }
            let title = self.title_of(&route);
            if detail {
                zikaron_ui::probe::inner_head(ctx, &title);
            }
            let (back, fwd) = {
                let h = self.hist();
                (!h.back.is_empty(), !h.fwd.is_empty())
            };
            zikaron_ui::probe::history(ctx, back, fwd);
            let bar = egui::Rect::from_min_size(full.min, egui::vec2(full.width(), tk::TOOLBAR_H));
            ui.allocate_rect(bar, egui::Sense::hover());
            // A detail page carries its large title in the page; the toolbar's title shows only once that has
            // scrolled away.
            let title_alpha = if detail { if self.ux.scroll_y > 44.0 { 1.0 } else { 0.0 } } else { 1.0 };
            let (nav, _) = zikaron_ui::toolbar::toolbar(ui, bar, &title, title_alpha, (back, fwd), self.ux.scroll_y > 4.0, (t(Key::NavBack), t(Key::NavFwd)), |ui| {
                if route.is_root() {
                    zikaron_ui::probe::head_keys(ui.ctx());
                    self.toolbar_acts(ui, &route, now);
                }
            });
            if nav.back {
                self.back(now);
            }
            if nav.fwd {
                self.fwd(now);
            }
            // The body scrolls; it enters by how the page changed.
            let body_rect = egui::Rect::from_min_max(egui::pos2(full.left(), bar.bottom()), full.max);
            let mut body = ui.new_child(egui::UiBuilder::new().max_rect(body_rect).layout(egui::Layout::top_down(egui::Align::Min)));
            body.set_clip_rect(body_rect);
            let key = self.ux.entry_key;
            ctx.data_mut(|d| d.insert_temp(egui::Id::new(ENTRY_KEY), key));
            let how = self.ux.entry;
            let p = motion::enter(ctx, egui::Id::new("zikaron-page-entry"), key, 0.0, how.dur(), motion::Curve::Ease);
            let (off, alpha) = how.at(p);
            let route_salt = motion::key_of(&(self.ux.stack, route.name(), key));
            let shown = egui::ScrollArea::vertical().scroll_source(egui::scroll_area::ScrollSource { drag: false, ..egui::scroll_area::ScrollSource::ALL }).id_salt(("page", route_salt)).auto_shrink([false, false]).show(&mut body, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin { left: tk::PAGE_PAD as i8, right: tk::PAGE_PAD as i8, top: tk::S3 as i8, bottom: tk::PAGE_PAD as i8 })
                    .show(ui, |ui| {
                        width::body(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = tk::CARD_GAP;
                            motion::shifted(ui, off, alpha, |ui| {
                                ui.spacing_mut().item_spacing.y = tk::CARD_GAP;
                                self.banner(ui, now);
                                self.page_body(ui, &route, now);
                            });
                        });
                    });
            });
            self.ux.scroll_y = shown.state.offset.y;
        });
    }

    /// The page for a route.
    fn page_body(&mut self, ui: &mut egui::Ui, route: &Route, now: f64) {
        use crate::nav::{tab as T, View};
        match route {
            Route::Root(Place::Home) => self.page_home(ui, now),
            Route::Root(Place::View(View::Works, tab)) => {
                if *tab == T::WORKS_PENDING {
                    self.works_pending(ui, now)
                } else {
                    self.works(ui, now)
                }
            }
            Route::Root(Place::View(View::Grants, _)) => self.grants_list(ui, now),
            Route::Root(Place::View(View::Verify, tab)) => self.verify_page(ui, *tab, now),
            Route::Root(Place::View(View::Log, _)) => self.ledger_page(ui, now),
            Route::Root(Place::View(View::Alerts, _)) => self.alerts_page(ui, now),
            Route::Root(Place::View(View::MyGrants, _)) => self.vault_page(ui, now),
            Route::Root(Place::SettingsHome) | Route::Root(Place::Settings(_)) => self.settings_home(ui, now),
            Route::Section(s) => self.settings_section(ui, *s, now),
            Route::Root(Place::Page(_)) => self.page_home(ui, now),
            Route::Work(id) => self.work_detail(ui, id, now),
            Route::Pending(id) => self.pending_detail(ui, id, now),
            Route::Kit => self.kit_page(ui, now),
            Route::NewGrant => self.grant_form_page(ui, now),
            Route::Grant(id) | Route::Entry(id) => self.entry_detail(ui, id, now),
            Route::Other(seq) => self.other_entry(ui, *seq, now),
            Route::Held(id) => self.held_detail(ui, id, now),
            Route::Relicense => self.relicense_page(ui, now),
        }
    }

    /// The toolbar title of a route.
    pub(super) fn title_of(&self, route: &Route) -> String {
        use crate::nav::View;
        let rows: &[crate::ledgerx::Row] = self.shell.rows.as_ref().map(|(r, _)| r.as_slice()).unwrap_or(&[]);
        let entry_title = |id: &str| rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).map(|r| format!("#{} \u{b7} {}", r.seq, t(user_kind_key(r.kind))));
        match route {
            Route::Root(Place::Home) | Route::Root(Place::Page(_)) => t(Key::NavHome).to_string(),
            Route::Root(Place::SettingsHome) | Route::Root(Place::Settings(_)) => t(Key::NavSettings).to_string(),
            Route::Root(Place::View(v, _)) => t(match v {
                View::Works => Key::NavWorksView,
                View::Grants => Key::KindGrant,
                View::Verify => Key::NavVerifyView,
                View::Log => Key::NavLogView,
                View::Alerts => Key::NavAlertsView,
                View::MyGrants => Key::NavMyGrantsView,
            })
            .to_string(),
            Route::Section(s) => t(s.key()).to_string(),
            Route::Work(id) => rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).map(human_summary).unwrap_or_default(),
            Route::Pending(id) | Route::Entry(id) => entry_title(id).unwrap_or_default(),
            Route::Grant(id) => rows.iter().find(|r| r.id.eq_ignore_ascii_case(id)).map(|r| format!("#{} \u{b7} {}", r.seq, t(Key::KindGrant))).unwrap_or_default(),
            Route::Other(seq) => self.other_title(*seq),
            Route::Held(id) => self.held_title(id),
            Route::Kit => t(Key::U3UseForKit).to_string(),
            Route::NewGrant => t(Key::V2NewGrant).to_string(),
            Route::Relicense => t(Key::U4RelicenseTitle).to_string(),
        }
    }

    /// The buttons at the right of a view's own page's toolbar.
    fn toolbar_acts(&mut self, ui: &mut egui::Ui, route: &Route, now: f64) {
        use crate::nav::{tab as T, View};
        let Route::Root(place) = route else { return };
        if matches!(self.shell.unfetched, Some(crate::restorex::State::NewerElsewhere { .. })) {
            mark::pill(ui, t(Key::NoteReader), PillTone::Warn);
        }
        match place {
            Place::View(View::Works, tab) => {
                self.ensure_rows(now);
                let cells = [seg::Cell::from(t(Key::U3FilterAll)), seg::Cell { text: t(Key::ItemUnanchored), count: Some(self.shell.queue.len()) }];
                if let Some(i) = seg::seg(ui, "works-tabs", &cells, usize::from(*tab == T::WORKS_PENDING)) {
                    self.set_tab(Place::View(View::Works, if i == 1 { T::WORKS_PENDING } else { T::WORKS_ALL }), now);
                }
            }
            Place::View(View::Grants, _) => {
                self.ensure_grants(now);
                let (_s, r) = page::Page::new().primary(ui, t(Key::V2NewGrant));
                if r.clicked() {
                    self.grant_form_fresh();
                    self.push(Route::NewGrant, now);
                }
            }
            Place::View(View::Verify, tab) => {
                let role = self.shell.settings.role;
                let list = crate::nav::tabs(View::Verify, role);
                let labels: Vec<seg::Cell> = list.iter().map(|x| seg::Cell::from(t(tab_key(View::Verify, *x)))).collect();
                let cur = list.iter().position(|x| x == tab).unwrap_or(0);
                if let Some(i) = seg::seg(ui, "verify-tabs", &labels, cur) {
                    self.set_tab(Place::View(View::Verify, list[i]), now);
                }
            }
            Place::View(View::Log, _) => self.ledger_acts(ui, now),
            Place::View(View::MyGrants, _) => self.vault_acts(ui, now),
            _ => {}
        }
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
            self.rail_to(Place::SettingsHome, now);
        }
        if !self.shell.vault.absent() && pressed(egui::Key::L) {
            self.lock_now(now);
        }
        let items: Vec<Place> = crate::nav::rail(self.shell.settings.role).iter().flat_map(|g| g.items.iter().map(|i| i.place)).collect();
        let keys = [egui::Key::Num1, egui::Key::Num2, egui::Key::Num3, egui::Key::Num4, egui::Key::Num5, egui::Key::Num6];
        for (i, k) in keys.iter().enumerate() {
            if pressed(*k) {
                if let Some(p) = items.get(i) {
                    self.rail_to(*p, now);
                }
            }
        }
    }

    /// Whether the page on screen takes dropped files in its own zones (the whole-window drop stands aside).
    fn page_takes_drops(&mut self) -> bool {
        use crate::nav::{tab as T, View};
        matches!(self.route(), Route::Kit | Route::NewGrant | Route::Relicense | Route::Root(Place::View(View::Verify, T::VERIFY_CHECK)))
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
                    self.go(Place::View(crate::nav::View::Verify, crate::nav::tab::VERIFY_WORK), now);
                }
            }
            _ => self.toasts.say(t(Key::U3OneAtATime), Tone::Bad, now),
        }
    }

}

/// A block of a page that enters one after another (the stagger of lists and detail pages), keyed on the
/// page's entrance.
pub(super) fn stagger<R>(ui: &mut egui::Ui, i: usize, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let key = ui.ctx().data(|d| d.get_temp::<u64>(egui::Id::new(ENTRY_KEY))).unwrap_or(0);
    motion::stagger(ui, egui::Id::new(("zikaron-stagger", i)), key, i, add)
}

/// Where the page's entrance counter is kept for the frame (blocks stagger on it).
const ENTRY_KEY: &str = "zikaron-entry-key";

/// Which old page a route stands for (the shell records it: tests and the trace channel read it).
fn route_page(route: &Route, root: Place, role: crate::roles::Role) -> Option<Page> {
    use crate::nav::tab as T;
    Some(match route {
        Route::Kit => Page::Kit,
        Route::NewGrant => Page::Grant,
        Route::Relicense => Page::Relicense,
        Route::Work(_) => Page::Anchoring,
        Route::Pending(_) => Page::Queue,
        Route::Grant(_) => Page::Grants,
        Route::Entry(_) => Page::Ledger,
        Route::Other(_) => match role {
            crate::roles::Role::Author => Page::Reader,
            crate::roles::Role::Grantee => Page::Diligence,
        },
        Route::Held(_) => Page::Vault,
        Route::Section(_) | Route::Root(Place::SettingsHome) | Route::Root(Place::Settings(_)) | Route::Root(Place::Home) | Route::Root(Place::Page(_)) => return None,
        Route::Root(Place::View(v, tab)) => {
            let _ = root;
            let _ = T::WORKS_ALL;
            view_page(*v, *tab, role)
        }
    })
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
