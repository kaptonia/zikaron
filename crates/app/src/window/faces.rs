use super::*;

impl Win {
    /// The typed path of one optional file on the check page.
    pub(super) fn ck_side(&mut self, which: OneStop) -> &mut String {
        match which {
            OneStop::File => &mut self.typed.ck_file,
            OneStop::Terms => &mut self.typed.ck_terms,
        }
    }

    pub(super) fn check_action(&self) -> Action {
        Action::CheckPayload {
            typed: self.typed.ck_typed.clone(),
            ledgers: self.typed.ck_ledgers.clone(),
            endpoints: self.typed.ck_endpoints.clone(),
            registry: self.typed.ck_registry.clone(),
            from_block: self.typed.ck_from.clone(),
            now: self.typed.ck_now.clone(),
            file: self.typed.ck_file.clone(),
            terms: self.typed.ck_terms.clone(),
        }
    }

    /// When an export is pressed, remember the chosen location (one per export kind, latest only).
    pub(super) fn remember_landing(&mut self, out: Out, c: &crate::home::Chosen) {
        self.ux.landings.retain(|(o, _)| *o != out);
        self.ux.landings.push((out, c.clone()));
    }

    /// Where the last export of this kind landed and why that differs from the choice (a second toast line);
    /// none when it landed where chosen.
    pub(super) fn landing_note(&mut self, out: Out) -> Option<String> {
        let at = self.ux.landings.iter().position(|(o, _)| *o == out)?;
        let (_, c) = self.ux.landings.remove(at);
        // The sentence names where it went: the new subfolder or the numbered name.
        c.why.say().map(|k| fill1(k, &width::file_name(&c.at.display().to_string())))
    }

    /// Whether the location field is well formed: the same decision as the action layer (`home::landing`); each
    /// location's main button is enabled by it.
    pub(super) fn landing_ok(given: &str) -> bool {
        crate::home::landing(given).is_ok()
    }

    /// The shared location row: "save to" on the left (132), the chosen folder's name, "choose folder…" on
    /// the right; the whole path is on hover. Choosing writes into `slot`; cancelling changes nothing.
    pub(super) fn place_row(ui: &mut egui::Ui, label: Key, slot: &mut String) {
        let asked = path_row(ui, t(label), slot, t(Key::PickFolder), t(Key::PickNone));
        if let Some(p) = path_answer(ui.ctx(), ui.id().with(("zikaron-place-row", label as u32)), asked, crate::platform::Pick::Folder) {
            *slot = p;
        }
    }

    /// The row the wizard's network step has selected now: the one clicked, else the one already chosen
    /// ([`wiz_network_chosen`](Self::wiz_network_chosen)), else the sheet's default.
    pub(super) fn wiz_network_pick(&self) -> String {
        self.ux.wiz_network.clone().or_else(|| self.wiz_network_chosen()).unwrap_or_else(|| crate::machine::pick(&self.shell.machine))
    }

    /// What the wizard's network step stands for now: the current identity's network when it has chosen one of
    /// this build's choices (the step decides that identity's network), else this machine's last choice.
    pub(super) fn wiz_network_chosen(&self) -> Option<String> {
        let row = self.shell.identities.as_ref().and_then(|r| r.now()).and_then(|(row, _)| row.network.as_ref().map(|c| c.name().to_string()));
        row.filter(|n| crate::deploy::is_choice(n)).or_else(|| self.shell.machine.network.clone())
    }
}

pub(super) fn tone_mark(t: crate::checkx::Tone) -> Mark {
    match t {
        crate::checkx::Tone::Green => Mark::Ok,
        crate::checkx::Tone::Amber => Mark::Warn,
        crate::checkx::Tone::Red => Mark::Bad,
        crate::checkx::Tone::Grey => Mark::Todo,
    }
}

pub(super) fn light_mark(l: crate::watchx::Light) -> Mark {
    match l {
        crate::watchx::Light::Ok => Mark::Ok,
        crate::watchx::Light::Warn => Mark::Warn,
        crate::watchx::Light::Bad => Mark::Bad,
        crate::watchx::Light::Unknown => Mark::Todo,
    }
}

pub(super) fn item_key(i: crate::watchx::Item) -> Key {
    use crate::watchx::Item;
    match i {
        Item::Unanchored => Key::ItemUnanchored,
        Item::QueueBacklog => Key::ItemQueueBacklog,
        Item::WindowExpiring => Key::ItemWindowExpiring,
        Item::Backup => Key::WizBackupTitle,
        Item::AuditLabel => Key::ItemAuditLabel,
        Item::HoldingExpiring => Key::ItemHoldingExpiring,
        Item::Revoked => Key::ItemRevoked,
        Item::Handed => Key::ItemHanded,
        Item::UpstreamRed => Key::ItemUpstreamRed,
    }
}

/// Whether an alert's subject names something the person knows (a record, a grant, an author's address) and is
/// said with it, or is the watch's own key for the row (the backup row, an audit label) and is not said.
pub(super) fn subject_said(i: crate::watchx::Item) -> bool {
    use crate::watchx::Item;
    match i {
        Item::Unanchored | Item::QueueBacklog | Item::WindowExpiring | Item::HoldingExpiring | Item::Revoked | Item::Handed | Item::UpstreamRed => true,
        Item::Backup | Item::AuditLabel => false,
    }
}

pub(super) fn badge_key(b: crate::grantx::Badge) -> Key {
    match b {
        crate::grantx::Badge::Live => Key::BadgeLive,
        crate::grantx::Badge::Expired => Key::BadgeExpired,
        crate::grantx::Badge::NotYet => Key::BadgeNotYet,
        crate::grantx::Badge::Revoked => Key::BadgeRevoked,
        crate::grantx::Badge::Unknown => Key::BadgeUnknown,
    }
}

/// A grant's state as a pill tone: live green, expired amber, not yet or revoked grey.
pub(super) fn badge_tone(b: crate::grantx::Badge) -> PillTone {
    match b {
        crate::grantx::Badge::Live => PillTone::Ok,
        crate::grantx::Badge::Expired => PillTone::Warn,
        crate::grantx::Badge::NotYet | crate::grantx::Badge::Revoked | crate::grantx::Badge::Unknown => PillTone::Grey,
    }
}

pub(super) fn step_name(s: crate::wizard::Step) -> Key {
    match s {
        crate::wizard::Step::Terms => Key::StepTerms,
        crate::wizard::Step::Anchor => Key::StepAnchor,
    }
}

/// An address in head-tail form (`0xc294…08a3`): menus of recent addresses only need it recognizable.
pub(super) fn head_tail(a: &str) -> String {
    if a.len() <= 12 {
        return a.to_string();
    }
    format!("{}\u{2026}{}", &a[..6], &a[a.len() - 4..])
}

/// A network row's name in the UI (the table row's name; "custom" has its own text).
pub(super) fn network_label(name: &str) -> String {
    match crate::deploy::named(name) {
        Some(d) => t(d.label).to_string(),
        None => t(Key::U3Custom).to_string(),
    }
}

/// An appearance's name in the UI.
pub(super) fn appearance_label(a: &str) -> &'static str {
    match a {
        "dark" => t(Key::AppearanceDark),
        "system" => t(Key::ZoneSystem),
        _ => t(Key::AppearanceLight),
    }
}

/// How the command line's anchoring works, as the settings row and its toast name it.
pub(super) fn cli_anchor_label(c: crate::machine::CliAnchor) -> Key {
    match c {
        crate::machine::CliAnchor::Send => Key::CliAnchorSend,
        crate::machine::CliAnchor::Queue => Key::CliAnchorQueue,
    }
}

/// The proxy choice for display: follow the system, none, or the address itself.
pub(super) fn proxy_label(c: &str) -> String {
    match c {
        crate::machine::proxy::SYSTEM => t(Key::ProxySystem).to_string(),
        crate::machine::proxy::NONE => t(Key::ProxyOff).to_string(),
        other => other.to_string(),
    }
}

pub(super) fn kind_key(k: zikaron::tokens::EntryType) -> Key {
    match k {
        zikaron::tokens::EntryType::Genesis => Key::WizGenesisTitle,
        zikaron::tokens::EntryType::History => Key::KindHistory,
        zikaron::tokens::EntryType::Grant => Key::KindGrant,
        zikaron::tokens::EntryType::Revocation => Key::KindRevocation,
        zikaron::tokens::EntryType::Adoption => Key::KindAdoption,
        zikaron::tokens::EntryType::Succession => Key::KindSuccession,
        zikaron::tokens::EntryType::Annotation => Key::KindAnnotation,
        zikaron::tokens::EntryType::Other => Key::KindOther,
    }
}

/// An anchoring state as a status mark: confirmed a check; on its way a spinner; queued an empty circle; not
/// on chain an exclamation; reverted and not sent a cross; local members an empty circle.
pub(super) fn lamp_mark(l: crate::ledgerx::Lamp) -> Mark {
    use crate::ledgerx::Lamp;
    match l {
        Lamp::Anchored | Lamp::Remembered => Mark::Ok,
        // Last verified long ago: not a check (that would pose as fresh), not a spinner (nothing moves).
        Lamp::RememberedStale => Mark::Todo,
        // Waiting for the receipt spins only while it is being asked for; with no node to ask, it stands still.
        Lamp::Included | Lamp::Submitted if receipts_stalled() => Mark::Warn,
        Lamp::Included | Lamp::Submitted => Mark::Busy,
        Lamp::Queued | Lamp::Deleted | Lamp::LocalDeletion | Lamp::ChainUnread => Mark::Todo,
        Lamp::Landed => Mark::Warn,
        Lamp::Reverted | Lamp::Refused => Mark::Bad,
    }
}

/// Whether submitted entries cannot be asked about now (no node for their chain): read once per frame from
/// the shell, so every spinner for them stands still together.
static RECEIPTS_STALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(super) fn set_receipts_stalled(stalled: bool) {
    RECEIPTS_STALLED.store(stalled, std::sync::atomic::Ordering::Relaxed);
}

fn receipts_stalled() -> bool {
    RECEIPTS_STALLED.load(std::sync::atomic::Ordering::Relaxed)
}

/// An anchoring state as a pill: its tone, and whether it spins.
pub(super) fn lamp_pill(l: crate::ledgerx::Lamp) -> (PillTone, bool) {
    use crate::ledgerx::Lamp;
    match l {
        Lamp::Anchored | Lamp::Remembered => (PillTone::Ok, false),
        Lamp::RememberedStale => (PillTone::Grey, false),
        Lamp::Submitted => (PillTone::Warn, !receipts_stalled()),
        Lamp::Included | Lamp::Queued => (PillTone::Warn, false),
        Lamp::Landed | Lamp::Deleted | Lamp::LocalDeletion | Lamp::ChainUnread => (PillTone::Grey, false),
        Lamp::Reverted | Lamp::Refused => (PillTone::Bad, false),
    }
}

/// Anchor state words. One owner: detail pages and lists all read this. `detail` true is the detail form.
pub(super) fn lamp_key(l: crate::ledgerx::Lamp, detail: bool) -> Key {
    use crate::ledgerx::Lamp;
    match l {
        Lamp::Anchored => Key::U3Anchored,
        Lamp::Included => Key::LampIncluded,
        Lamp::Submitted => Key::LampSubmitted,
        Lamp::Queued => Key::ItemUnanchored,
        Lamp::Reverted => Key::LampReverted,
        Lamp::Refused => Key::LampRefused,
        Lamp::Landed => Key::V2StateLanded,
        Lamp::Deleted if detail => Key::LampDeletedDetail,
        Lamp::Deleted => Key::V2Deleted,
        Lamp::LocalDeletion => Key::LampLocalDeletion,
        Lamp::Remembered | Lamp::RememberedStale => Key::LampRemembered,
        Lamp::ChainUnread => Key::U4ChainUnread,
    }
}

/// The anchor state sentence: the two "last verified" members carry the last pass's time; the others use
/// `lamp_key`'s words. `at` is the time of the set on disk.
pub fn lamp_label(l: crate::ledgerx::Lamp, detail: bool, at: Option<u64>) -> String {
    use crate::ledgerx::Lamp;
    match l {
        Lamp::Remembered | Lamp::RememberedStale if !detail => fill1(Key::LampRememberedShort, &at.map(hm_of).unwrap_or_default()),
        Lamp::Remembered | Lamp::RememberedStale => fill1(Key::LampRemembered, &at.map(hhmm_of).unwrap_or_default()),
        _ => t(lamp_key(l, detail)).to_string(),
    }
}

/// A time read as "hh:mm" only.
pub(super) fn hm_of(secs: u64) -> String {
    crate::when::when(secs).chars().skip(11).take(5).collect()
}


/// Month, day, hour and minute (dated, so a check from days ago does not read as today).
pub(super) fn hhmm_of(secs: u64) -> String {
    crate::when::short(secs)
}

/// Join the twelve words into one string (space separated), built inside a secret type.
pub(super) fn joined(words: &[crate::secret::Secret]) -> crate::secret::Secret {
    let mut out = crate::secret::Secret::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(w.expose().trim());
    }
    out
}

pub(super) fn id_seat_key(r: crate::roles::Role) -> Key {
    match r {
        crate::roles::Role::Author => Key::IdSeatAuthor,
        crate::roles::Role::Grantee => Key::IdSeatGrantee,
    }
}

/// Titles of the two seat rows (author and grantee).
pub(super) fn id_seat_row_key(r: crate::roles::Role) -> Key {
    match r {
        crate::roles::Role::Author => Key::IdSeatRowAuthor,
        crate::roles::Role::Grantee => Key::IdSeatRowGrantee,
    }
}

/// The two kinds of identity.
pub(super) fn id_kind_key(k: crate::identity::Kind) -> Key {
    match k {
        crate::identity::Kind::Words => Key::IdKindWords,
        crate::identity::Kind::Existing => Key::IdKindExisting,
    }
}

/// Open the window. Returns the exit code. The shipped build takes this path: starts at home, and opens the
/// first-run wizard by itself when the anchor key is missing or the author seat has no ledger yet.
pub fn run() -> i32 {
    run_at(Start::Place(Place::Home, false))
}

/// The line standard error gets when the window takes arguments (the misuse exit, code 2).
pub const ARGS_LINE: &str = "E_ARGS the window takes no arguments";

/// The window program was given arguments: report it once without a window (the standard-error line plus
/// the message for a person) and return the misuse exit code. The window is not opened.
pub fn refuse_arguments() -> i32 {
    speak_this_machines_language();
    without_window(ARGS_LINE, t(Key::SaidNoWindowArgs).to_string());
    2
}

/// Say why there is no window through the platform interface (`platform::say_without_window`), the sentence
/// in the language this machine last chose: the window never came up to set it, so it is read from the
/// machine settings here.
fn without_window(line: &str, sentence: String) {
    crate::platform::say_without_window(line, &sentence);
}

/// Speak the language this machine last chose (`machine.json`'s `lang`; unreadable or never chosen: the
/// build's default), before any window or home has set it.
fn speak_this_machines_language() {
    if let Some(l) = crate::machine::read().ok().and_then(|m| m.lang) {
        crate::lang::set(l);
    }
}

/// Open the window on a given page. The shipped binary has no path that calls it; only the test hooks do.
pub fn run_on(start: Page) -> i32 {
    run_at(Start::Page(start))
}

fn fresh_win(shell: Shell, typed: Typed, ux: Ux) -> Win {
    Win {
        shell,
        toasts: Toasts::new(),
        typed,
        reaped: None,
        opened: None,
        last_tick: 0.0,
        clash_modal: false,
        clash_key: String::new(),
        faults_told: 0,
        ux,
        asker: crate::platform::ask_path,
    }
}

/// Open the window at a given place (the test hooks use it; the shipped build only goes through `run`).
pub fn run_at(start: Start) -> i32 {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(t(Key::AppName))
            // Matches the installed desktop entry (`zikaron-desk.desktop`) so the desktop shows the app's icon.
            .with_app_id("zikaron-desk")
            .with_icon(std::sync::Arc::new(egui::IconData {
                rgba: include_bytes!("../../assets/icon-256.rgba").to_vec(),
                width: 256,
                height: 256,
            }))
            .with_inner_size([W, H])
            .with_min_inner_size([MIN_W, MIN_H])
            // The side rail reaches the top and the window buttons float over it; no separate title bar.
            .with_fullsize_content_view(true)
            .with_title_shown(false)
            .with_titlebar_shown(false),
        ..Default::default()
    };
    // The window system the platform interface chose (on Linux, X11 when there is an X display: files are
    // dropped on the window there).
    #[cfg(target_os = "linux")]
    let opts = {
        let mut opts = opts;
        if crate::platform::window_backend() == crate::platform::Backend::X11 {
            opts.event_loop_builder = Some(Box::new(|b| {
                use winit::platform::x11::EventLoopBuilderExtX11;
                b.with_x11();
            }));
        }
        opts
    };
    match eframe::run_native(
        t(Key::AppName),
        opts,
        Box::new(|cc| {
            let dressed = skin::dress(&cc.egui_ctx);
            // Settle the machine directory once; if settling fails the window still opens, and the error is shown
            // (never silent).
            let settled = crate::home::settle_machine();
            let mut shell = Shell::boot(dressed);
            if let Err(f) = settled {
                shell.faults.push(f);
            }
            // A request on the command-line channel wakes a frame (even when minimized or covered: the frame then
            // drains, and the channel's turn handles the request).
            let wake = cc.egui_ctx.clone();
            shell.door_waker = Some(std::sync::Arc::new(move || wake.request_repaint()));
            // The product builds its own preconditions: local data is sealed and the vault starts locked, so the
            // home opens, its lock is taken, the key is asked for and the home measured right after unlocking
            // (`Shell::after_unlock`). A vault already open here (only the test hooks start that way) opens it now.
            let root = crate::action::start(&mut shell);
            if let Some(l) = shell.speaks() {
                crate::lang::set(l);
            }
            crate::when::set(shell.settings.zone.unwrap_or(crate::when::Zone::Utc));
            let mut ux = Ux::default();
            match start {
                Start::Page(p) => shell.page = p,
                Start::Place(place, wizard) => {
                    if let Place::Page(p) = place {
                        shell.page = p;
                    }
                    let (stack, h) = crate::nav::history_of(place, shell.settings.role);
                    ux.stack = stack;
                    ux.hist.insert(stack, h);
                    ux.wizard_forced = wizard;
                }
            }
            let typed = Typed { home: root, ..Default::default() };
            Ok(Box::new(fresh_win(shell, typed, ux)))
        }),
    ) {
        Ok(()) => 0,
        Err(e) => {
            // The standard-error line in the language set so far; the message in this machine's language.
            let why = e.to_string();
            let line = fill1(Key::SaidWindowFailed, &why);
            speak_this_machines_language();
            without_window(&line, fill1(Key::SaidNoWindowFailed, &why));
            1
        }
    }
}

/// The anchoring sheet after its button was pressed, frame by frame without a window (no wall clock: each
/// frame's time is given here): the caller starts the anchoring task as it likes (held, released, failing) and
/// asks after each frame whether the sheet is still open. This measures the sheet's own rule: it stays open
/// until that task has landed, successfully or not, and closes on the landing itself.
pub struct SendProbe {
    win: Win,
    t: f64,
}

impl SendProbe {
    /// The sheet for the first `count` entries with its button pressed now (the press's stamp taken from the
    /// anchoring task's landings, as the button does).
    pub fn pressed(shell: Shell, count: usize) -> SendProbe {
        let mut ux = Ux::default();
        ux.wizard_asked = true;
        ux.u3.confirm = Some(U3Confirm::Send { count });
        ux.u3.sending = Some(shell.tasks.landings(crate::task::Kind::Anchor));
        let mut win = fresh_win(shell, Typed::default(), ux);
        win.faults_told = usize::MAX;
        SendProbe { win, t: 0.0 }
    }

    /// Draw one frame; whether the sheet is open after it.
    pub fn frame(&mut self, ctx: &egui::Context) -> bool {
        self.t += 0.1;
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 760.0));
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(self.t), ..Default::default() };
        let win = &mut self.win;
        let _ = ctx.run(input, |c| win.draw(c));
        self.win.ux.u3.confirm.is_some()
    }

    /// The shell under the sheet (the caller's task lives in it).
    pub fn shell(&mut self) -> &mut Shell {
        &mut self.win.shell
    }
}

/// Draw one page in a few windowless frames and measure its layout at a `width × height` viewport (no window,
/// no wall clock: frame time is given here). The caller prepares the shell and gets it back unchanged. The
/// first-run wizard and passcode gate are not opened here (this measures the page layer).
pub fn probe_face(ctx: &egui::Context, shell: Shell, place: Place, width: f32, height: f32) -> (Shell, FaceReading) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    let (stack, h) = crate::nav::history_of(place, role);
    ux.stack = stack;
    ux.hist.insert(stack, h);
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>, step: f64| -> egui::FullOutput {
        t += step;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), events, ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    let edges = |out: &egui::FullOutput| -> (f32, f32) {
        let widgets = ctx.viewport(|v| v.prev_pass.widgets.layers().flat_map(|(_, ws)| ws.iter().map(|w| w.rect.max.x)).fold(0.0_f32, f32::max));
        let mut text = 0.0_f32;
        for c in &out.shapes {
            if let egui::Shape::Text(s) = &c.shape {
                text = text.max(s.visual_bounding_rect().max.x);
            }
        }
        (widgets, text)
    };
    // Frames until the page has settled: the first sets the layout, the next set equal-cell heights, and the
    // entrance (at most 240 ms plus a 200 ms stagger) is over by the last.
    for _ in 0..5 {
        let _ = run(&mut win, Vec::new(), 0.1);
    }
    let out = run(&mut win, Vec::new(), 0.1);
    let (right_widget, right_text) = edges(&out);
    let texts: Vec<(egui::Rect, usize, bool)> = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(s) => Some((s.visual_bounding_rect(), s.galley.rows.len(), s.galley.job.justify)),
            _ => None,
        })
        .collect();
    let justified_texts = texts.iter().filter(|(_, _, j)| *j).count();
    let cells: Vec<CellFit> = zikaron_ui::probe::cells(ctx)
        .into_iter()
        .map(|r| {
            let inside: Vec<(egui::Rect, usize)> = texts
                .iter()
                .filter(|(t, _, _)| t.min.x >= r.min.x - 0.5 && t.min.x < r.max.x && r.y_range().contains(t.center().y))
                .map(|(t, n, _)| (*t, *n))
                .collect();
            CellFit {
                width: r.width(),
                over: inside.iter().filter(|(t, _)| t.width() > r.width() + 0.5 || t.max.x > r.max.x + 0.5).count(),
                wrapped: inside.iter().filter(|(_, n)| *n > 1).count(),
            }
        })
        .collect();
    let tiles = zikaron_ui::probe::tiles(ctx);
    let rows = zikaron_ui::probe::rows(ctx);
    let route = zikaron_ui::probe::route_drawn(ctx);
    let words: Vec<String> = out
        .shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(s) => Some(s.galley.job.text.clone()),
            _ => None,
        })
        .collect();
    let (inner, inner_right, inner_head, inner_head_keys, inner_history, inner_route) = match rows.first() {
        None => (None, None, None, None, None, None),
        Some(r) => {
            let at = r.center();
            let _ = run(&mut win, vec![egui::Event::PointerMoved(at)], 0.1);
            let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
            let _ = run(&mut win, vec![press(true), press(false)], 0.1);
            let _ = run(&mut win, vec![egui::Event::PointerGone], 0.1);
            for _ in 0..4 {
                let _ = run(&mut win, Vec::new(), 0.1);
            }
            let out = run(&mut win, Vec::new(), 0.1);
            let shown = zikaron_ui::probe::inner_shown(ctx);
            let (a, b) = edges(&out);
            (
                Some(shown),
                Some(a.max(b)),
                zikaron_ui::probe::inner_head_said(ctx),
                Some(zikaron_ui::probe::head_keys_shown(ctx)),
                Some(zikaron_ui::probe::history_shown(ctx)),
                Some(zikaron_ui::probe::route_drawn(ctx)),
            )
        }
    };
    (
        win.shell,
        FaceReading { width, right_widget, right_text, rows: rows.len(), inner, inner_right, inner_head, inner_head_keys, inner_history, route, inner_route, texts: words, cells, justified_texts, tiles },
    )
}

/// Walk a list page's history: settle, open the first row, go back (⌘[), go forward (⌘]). After each: the
/// route drawn and whether back and forward were shown.
pub fn probe_history(ctx: &egui::Context, shell: Shell, place: Place, width: f32, height: f32) -> (Shell, Vec<(String, bool, bool)>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    let (stack, h) = crate::nav::history_of(place, role);
    ux.stack = stack;
    ux.hist.insert(stack, h);
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), events, modifiers: egui::Modifiers::NONE, ..Default::default() };
        let _ = ctx.run(input, |c| win.draw(c));
    };
    let settle = |win: &mut Win, run: &mut dyn FnMut(&mut Win, Vec<egui::Event>)| {
        for _ in 0..5 {
            run(win, Vec::new());
        }
    };
    let said = || {
        let (b, f) = zikaron_ui::probe::history_shown(ctx);
        (zikaron_ui::probe::route_drawn(ctx), b, f)
    };
    let mut out = Vec::new();
    settle(&mut win, &mut run);
    out.push(said());
    if let Some(r) = zikaron_ui::probe::rows(ctx).first().copied() {
        let at = r.center();
        let press = |pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE };
        run(&mut win, vec![egui::Event::PointerMoved(at)]);
        run(&mut win, vec![press(true), press(false)]);
        run(&mut win, vec![egui::Event::PointerGone]);
        settle(&mut win, &mut run);
        out.push(said());
        let key = |k: egui::Key| egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::COMMAND };
        run(&mut win, vec![key(egui::Key::OpenBracket)]);
        settle(&mut win, &mut run);
        out.push(said());
        run(&mut win, vec![key(egui::Key::CloseBracket)]);
        settle(&mut win, &mut run);
        out.push(said());
    }
    (win.shell, out)
}

/// One frame of a file-dialog walk ([`probe_paths`]): which places ask (by name) and with what, which take
/// what landed for them, then one frame of the window.
pub struct PathFrame {
    pub ask: Vec<(&'static str, crate::platform::Pick)>,
    pub take: Vec<&'static str>,
    /// Before this frame the person answers the open dialog (`answer` is called, then the walk waits for the
    /// dialog's wait to end, never for a time).
    pub answer: bool,
}

/// What a file-dialog walk read after each frame: what each taking place got, the troubles the shell holds,
/// and whether a dialog is out.
pub struct PathRead {
    pub took: Vec<(&'static str, Option<String>)>,
    pub faults: Vec<crate::fault::Fault>,
    pub open: bool,
}

/// Walk the file dialog the way the places use it, in windowless frames, with `asker` in place of the system's
/// dialog: each frame, the places in `ask` ask (as pressing their "choose…" button would), the window draws
/// (it opens the dialog at the end of the frame), and the places in `take` take what landed for them (as they
/// do the next frame they are drawn). The places are named; each name is its own key.
pub fn probe_paths(ctx: &egui::Context, shell: Shell, asker: crate::platform::Asker, answer: &mut dyn FnMut(), frames: Vec<PathFrame>) -> (Shell, Vec<PathRead>) {
    let mut ux = Ux::default();
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.asker = asker;
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(W, H));
    let site = |n: &str| egui::Id::new(("zikaron-probe-path", n));
    let mut t = 0.0;
    let mut out = Vec::new();
    for f in frames {
        if f.answer {
            answer();
            while win.shell.tasks.in_flight(crate::task::Kind::Path) && !win.shell.tasks.finished_in_flight(crate::task::Kind::Path) {
                std::thread::yield_now();
            }
        }
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        let mut took = Vec::new();
        let _ = ctx.run(input, |c| {
            for (n, kind) in &f.ask {
                let _ = path_answer(c, site(n), true, *kind);
            }
            win.draw(c);
        });
        // Taken the next frame they are drawn, as the places do (the answer landed at this frame's start).
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t + 0.05), ..Default::default() };
        t += 0.05;
        let _ = ctx.run(input, |c| {
            win.draw(c);
            for n in &f.take {
                took.push((*n, path_answer(c, site(n), false, crate::platform::Pick::File)));
            }
        });
        out.push(PathRead { took, faults: win.shell.faults.clone(), open: win.shell.tasks.in_flight(crate::task::Kind::Path) });
    }
    (win.shell, out)
}

/// Drive the window shell's own input over `place` in windowless frames, after a few empty ones to settle:
/// each step is one frame, `dt` seconds after the one before, with its events. After each step: the route
/// drawn and the window commands sent in that frame (`Debug` words of egui's viewport commands).
pub fn probe_shell_input(ctx: &egui::Context, shell: Shell, place: Place, steps: Vec<(f64, Vec<egui::Event>)>, width: f32, height: f32) -> (Shell, Vec<(String, Vec<String>)>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    let (stack, h) = crate::nav::history_of(place, role);
    ux.stack = stack;
    ux.hist.insert(stack, h);
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>, dt: f64| -> Vec<String> {
        t += dt;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), events, modifiers: egui::Modifiers::NONE, ..Default::default() };
        let out = ctx.run(input, |c| win.draw(c));
        out.viewport_output.values().flat_map(|v| v.commands.iter().map(|c| format!("{c:?}"))).collect()
    };
    for _ in 0..5 {
        run(&mut win, Vec::new(), 0.1);
    }
    let mut out = Vec::new();
    for (dt, events) in steps {
        let sent = run(&mut win, events, dt);
        out.push((zikaron_ui::probe::route_drawn(ctx), sent));
    }
    (win.shell, out)
}

/// What the window said when the watch rang: the toast's words and tone, and whether it asked for the
/// system's attention.
pub struct RangReading {
    pub toast: Option<String>,
    pub tone: Option<zikaron_ui::toast::Tone>,
    pub attention: bool,
}

/// Ring `notices` over `place` in a few windowless frames, with the first-run wizard open at `wizard` or not.
pub fn probe_rang(ctx: &egui::Context, shell: Shell, place: Place, notices: Vec<crate::watchx::Notice>, wizard: Option<crate::nav::Step>, width: f32, height: f32) -> (Shell, RangReading) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    let (stack, h) = crate::nav::history_of(place, role);
    ux.stack = stack;
    ux.hist.insert(stack, h);
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut attention = false;
    let mut run = |win: &mut Win| {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        let out = ctx.run(input, |c| win.draw(c));
        out.viewport_output.values().any(|v| v.commands.iter().any(|c| matches!(c, egui::ViewportCommand::RequestUserAttention(_))))
    };
    // Only this call's ringing is read: what rang before is dropped, and the wizard (when asked for)
    // is open from the first frame.
    win.shell.fresh.clear();
    win.shell.rung.clear();
    win.shell.attention = false;
    if let Some(step) = wizard {
        win.wizard_open(step);
    }
    for _ in 0..3 {
        let _ = run(&mut win);
    }
    win.shell.fresh = notices;
    win.shell.attention = true;
    for _ in 0..3 {
        attention |= run(&mut win);
    }
    let reading = RangReading { toast: win.toasts.showing().map(str::to_string), tone: win.toasts.showing_tone(), attention };
    (win.shell, reading)
}

/// One text drawn: its words, its rectangle, its rows, and its first baseline (absolute).
pub struct Drawn {
    pub text: String,
    pub rect: egui::Rect,
    pub rows: usize,
    /// How many characters each drawn row holds (rows as laid out: a wrapped text's rows, not its line breaks).
    pub row_chars: Vec<usize>,
    pub baseline: f32,
}

/// The first-run wizard open at `step` over `place`, settled in windowless frames: every text drawn and every QR
/// plate.
pub fn probe_wizard_step(ctx: &egui::Context, shell: Shell, place: Place, step: crate::nav::Step, width: f32, height: f32) -> (Shell, Vec<Drawn>, Vec<egui::Rect>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    let (stack, h) = crate::nav::history_of(place, role);
    ux.stack = stack;
    ux.hist.insert(stack, h);
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win| {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    win.wizard_open(step);
    for _ in 0..6 {
        let _ = run(&mut win);
    }
    let out = run(&mut win);
    let texts = texts_of(&out);
    let plates = zikaron_ui::probe::qrs(ctx);
    (win.shell, texts, plates)
}

/// One frame of the wizard: the step it stood on, every text drawn with where, and how far a drawn check has
/// gone (see [`check_points`]).
pub struct WizFrame {
    pub step: String,
    pub texts: Vec<Drawn>,
    pub check: usize,
}

/// How far the drawn check of a finished step has gone this frame: the points of the longest open line in
/// the success colour (it only ever grows while the check draws; back to few means it started again).
fn check_points(o: &egui::FullOutput) -> usize {
    let ok = c(C::Ok);
    let mut most = 0usize;
    let mut look = |s: &egui::Shape| {
        if let egui::Shape::Path(p) = s {
            if !p.closed && p.stroke.color == egui::epaint::ColorMode::Solid(ok) {
                most = most.max(p.points.len());
            }
        }
    };
    for cl in &o.shapes {
        match &cl.shape {
            egui::Shape::Vec(v) => v.iter().for_each(&mut look),
            s => look(s),
        }
    }
    most
}

/// The wizard open at `from`, settled; then turned to `to` the way its "next" button turns it (`ux.wizard`),
/// and read every 20 ms for `frames` frames.
pub fn probe_wizard_walk(ctx: &egui::Context, shell: Shell, place: Place, from: crate::nav::Step, to: crate::nav::Step, frames: usize, width: f32, height: f32) -> (Shell, Vec<WizFrame>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    let (stack, h) = crate::nav::history_of(place, role);
    ux.stack = stack;
    ux.hist.insert(stack, h);
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, step: f64| {
        t += step;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    win.wizard_open(from);
    for _ in 0..8 {
        let _ = run(&mut win, 0.1);
    }
    win.ux.wizard = Some(to);
    let mut out = Vec::new();
    for _ in 0..frames {
        let o = run(&mut win, 0.02);
        out.push(WizFrame { step: win.ux.wizard.map(|s| format!("{s:?}")).unwrap_or_default(), texts: texts_of(&o), check: check_points(&o) });
    }
    (win.shell, out)
}

/// The wizard on the ledger step with its confirmation sheet up; then "create" pressed the way the sheet's
/// button presses it, and read every 20 ms for `frames` frames: how far the drawn check has gone.
pub fn probe_wizard_genesis(ctx: &egui::Context, shell: Shell, place: Place, frames: usize, width: f32, height: f32) -> (Shell, Vec<usize>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    let (stack, h) = crate::nav::history_of(place, role);
    ux.stack = stack;
    ux.hist.insert(stack, h);
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, step: f64| {
        t += step;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        (t, ctx.run(input, |c| win.draw(c)))
    };
    win.wizard_open(crate::nav::Step::Genesis);
    for _ in 0..6 {
        let _ = run(&mut win, 0.1);
    }
    win.ux.confirm_genesis = true;
    let mut now = 0.0;
    for _ in 0..6 {
        now = run(&mut win, 0.1).0;
    }
    win.ux.confirm_genesis = false;
    win.act(Action::Genesis { statement: win.ux.wiz_statement.clone() }, now);
    let mut out = Vec::new();
    for _ in 0..frames {
        let (_, o) = run(&mut win, 0.02);
        out.push(check_points(&o));
    }
    (win.shell, out)
}

/// The wizard on the ledger step; its confirmation sheet opened, then closed the way its "back" button closes
/// it. Every 20 ms from opening: the opacity (0–255) of the sheet's own note `said` in that frame, as drawn
/// (0 when not drawn).
pub fn probe_sheet_leave(ctx: &egui::Context, shell: Shell, place: Place, said: &str, open_frames: usize, close_frames: usize, width: f32, height: f32) -> (Shell, Vec<u8>, Vec<u8>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    let (stack, h) = crate::nav::history_of(place, role);
    ux.stack = stack;
    ux.hist.insert(stack, h);
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, step: f64| {
        t += step;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        ctx.run(input, |c| win.draw(c))
    };
    let alpha_of = |o: &egui::FullOutput| -> u8 {
        o.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(s) if s.galley.job.text == said => {
                    s.galley.rows.iter().flat_map(|r| r.visuals.mesh.vertices.iter()).map(|v| v.color.a()).max().map(|a| (a as f32 * s.opacity_factor) as u8)
                }
                _ => None,
            })
            .max()
            .unwrap_or(0)
    };
    win.wizard_open(crate::nav::Step::Genesis);
    for _ in 0..6 {
        let _ = run(&mut win, 0.1);
    }
    win.ux.confirm_genesis = true;
    let opening: Vec<u8> = (0..open_frames).map(|_| alpha_of(&run(&mut win, 0.02))).collect();
    win.ux.confirm_genesis = false;
    let closing: Vec<u8> = (0..close_frames).map(|_| alpha_of(&run(&mut win, 0.02))).collect();
    (win.shell, opening, closing)
}

fn texts_of(out: &egui::FullOutput) -> Vec<Drawn> {
    out.shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(s) => Some(Drawn {
                text: s.galley.job.text.clone(),
                rect: s.visual_bounding_rect(),
                rows: s.galley.rows.len(),
                row_chars: s.galley.rows.iter().map(|r| r.glyphs.len()).collect(),
                baseline: s.pos.y + s.galley.rows.first().and_then(|r| r.glyphs.first()).map(|g| g.pos.y).unwrap_or(0.0),
            }),
            _ => None,
        })
        .collect()
}

/// Leave `from` for `to` through the rail's own path (`go`), and read every frame from the first one after:
/// the texts drawn in the toolbar band.
pub fn probe_title_swap(ctx: &egui::Context, shell: Shell, from: Place, to: Place, width: f32, height: f32) -> (Shell, Vec<Vec<String>>) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    let (stack, h) = crate::nav::history_of(from, role);
    ux.stack = stack;
    ux.hist.insert(stack, h);
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, step: f64| {
        t += step;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), ..Default::default() };
        (t, ctx.run(input, |c| win.draw(c)))
    };
    for _ in 0..5 {
        let _ = run(&mut win, 0.1);
    }
    let (now, _) = run(&mut win, 0.1);
    win.go(to, now);
    let mut frames = Vec::new();
    // Every 20 ms, covering a full crossfade (90 ms out, 120 ms in) and beyond.
    for _ in 0..15 {
        let (_, out) = run(&mut win, 0.02);
        let band: Vec<String> = texts_of(&out).into_iter().filter(|d| d.rect.top() < tk::TOOLBAR_H && d.rect.left() > tk::RAIL_W && d.text.trim().len() > 0).map(|d| d.text).collect();
        frames.push(band);
    }
    (win.shell, frames)
}

/// What the window did with files dropped on it as a whole.
pub struct DropReading {
    /// The route drawn after the drop.
    pub route: String,
    /// The new-record sheet's step, when it is open.
    pub sheet: Option<u8>,
    /// What the new-record sheet took (its subject), and the record name it filled in.
    pub taken: Option<String>,
    pub name: String,
    /// The path handed to record verification.
    pub verify_path: String,
    /// The toast on screen.
    pub toast: Option<String>,
}

/// Drop `paths` on the window over `place` in a few windowless frames. A place without its own drop area (the
/// records page, say) leaves the drop to the window as a whole.
pub fn probe_drop(ctx: &egui::Context, shell: Shell, place: Place, paths: &[String], width: f32, height: f32) -> (Shell, DropReading) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    let (stack, h) = crate::nav::history_of(place, role);
    ux.stack = stack;
    ux.hist.insert(stack, h);
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, dropped: Vec<egui::DroppedFile>| {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), dropped_files: dropped, ..Default::default() };
        let _ = ctx.run(input, |c| win.draw(c));
    };
    for _ in 0..3 {
        run(&mut win, Vec::new());
    }
    run(&mut win, paths.iter().map(|p| egui::DroppedFile { path: Some(p.into()), ..Default::default() }).collect());
    for _ in 0..4 {
        run(&mut win, Vec::new());
    }
    let reading = DropReading {
        route: zikaron_ui::probe::route_drawn(ctx),
        sheet: win.ux.u3.new_anchor,
        taken: win.shell.content.as_ref().map(|c| c.subject.clone()),
        name: win.typed.work_note.clone(),
        verify_path: win.typed.vf_path.clone(),
        toast: win.toasts.showing().map(|s| s.to_string()),
    };
    (win.shell, reading)
}

/// What the wizard's way out did.
pub struct WizardExit {
    /// The route drawn before the wizard opened and after the way out; whether the wizard was up in between.
    pub before: String,
    pub opened: bool,
    pub after: String,
    /// Whether the way out was offered, and whether the wizard was still open after Esc.
    pub offered: bool,
    pub still_open: bool,
    /// With new words shown and not yet checked: whether Esc asked first (and said the words would be void),
    /// and whether leaving dropped them.
    pub asked: bool,
    pub said_void: bool,
    pub words_dropped: bool,
    /// The window's own structural state, not what was drawn: where the wizard recorded it was opened from
    /// (`ux.wiz_from`: seat, stack, place) while it was up, and the seat, stack and place after leaving.
    pub wiz_from: String,
    pub place_after: String,
}

/// Open the wizard at `step` over `place`, then press Esc in a few windowless frames. With new words in hand
/// (made first), Esc asks first; this then leaves the way the prompt's leave button does.
pub fn probe_wizard_exit(ctx: &egui::Context, shell: Shell, place: Place, step: crate::nav::Step, width: f32, height: f32) -> (Shell, WizardExit) {
    let role = shell.settings.role;
    let mut ux = Ux::default();
    let (stack, h) = crate::nav::history_of(place, role);
    ux.stack = stack;
    ux.hist.insert(stack, h);
    ux.wizard_asked = true;
    let mut win = fresh_win(shell, Typed::default(), ux);
    win.faults_told = usize::MAX;
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, height));
    let mut t = 0.0;
    let mut run = |win: &mut Win, events: Vec<egui::Event>| -> (f64, egui::FullOutput) {
        t += 0.1;
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t), events, ..Default::default() };
        (t, ctx.run(input, |c| win.draw(c)))
    };
    for _ in 0..4 {
        let _ = run(&mut win, Vec::new());
    }
    let before = zikaron_ui::probe::route_drawn(ctx);
    win.wizard_open(step);
    for _ in 0..4 {
        let _ = run(&mut win, Vec::new());
    }
    let opened = win.ux.wizard.is_some();
    let offered = win.wizard_exit_shown();
    let wiz_from = win.ux.wiz_from.as_ref().map(|(r, s, p, _)| format!("{}:{s:?}:{p:?}", r.as_str())).unwrap_or_default();
    let had_words = win.shell.new_words.is_some();
    let esc = egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
    let _ = run(&mut win, vec![esc]);
    let mut last = None;
    for _ in 0..4 {
        last = Some(run(&mut win, Vec::new()));
    }
    let asked = had_words && win.ux.wiz_exit_ask;
    let said_void = last
        .as_ref()
        .map(|(_, out)| out.shapes.iter().any(|c| matches!(&c.shape, egui::Shape::Text(s) if s.galley.job.text.contains(t_void()))))
        .unwrap_or(false);
    if asked {
        let now = last.as_ref().map(|(n, _)| *n).unwrap_or(0.0);
        win.wizard_exit(now);
        for _ in 0..4 {
            let _ = run(&mut win, Vec::new());
        }
    }
    let reading = WizardExit {
        before,
        opened,
        after: zikaron_ui::probe::route_drawn(ctx),
        offered,
        still_open: win.ux.wizard.is_some(),
        asked,
        said_void,
        words_dropped: had_words && win.shell.new_words.is_none(),
        wiz_from,
        place_after: format!("{}:{:?}:{:?}", win.shell.settings.role.as_str(), win.ux.stack, win.ux.place),
    };
    (win.shell, reading)
}

fn t_void() -> &'static str {
    t(Key::WizExitWords)
}

/// Whether a rail item is the lit one (views ignore tabs).
pub(super) fn same_item(lit: Place, item: Place) -> bool {
    match (lit, item) {
        (Place::View(a, _), Place::View(b, _)) => a == b,
        (a, b) => a == b,
    }
}

/// A record row's state pill: deleted grey, confirmed green, on its way amber, failed red.
pub(super) fn work_state(w: &WorkLine, at: Option<u64>) -> (String, PillTone, bool) {
    match w.deleted {
        Some(_) => (t(Key::V2Deleted).to_string(), PillTone::Grey, false),
        None => {
            let (tone, live) = lamp_pill(w.lamp);
            (lamp_label(w.lamp, false, at), tone, live)
        }
    }
}

/// An entry row's kind label and summary (lists and detail pages decide in one place).
pub fn row_face_words(rows: &[crate::ledgerx::Row], r: &crate::ledgerx::Row) -> (String, String) {
    let (k, said, _) = row_face(rows, &crate::retractx::read(rows), r);
    (k.to_string(), said)
}

/// Why a delete is invalid, in plain words.
pub(super) fn invalid_key(why: crate::retractx::Invalid) -> Key {
    use crate::retractx::Invalid;
    match why {
        Invalid::Shape => Key::V2InvalidShape,
        Invalid::NotInLedger => Key::V2InvalidNotInLedger,
        Invalid::NotAWork => Key::V2InvalidNotAWork,
        Invalid::Repeated => Key::V2InvalidRepeated,
    }
}

/// The display of a ledger entry row: kind label, summary, whether struck through. Delete entries say which
/// record they delete (or why they are invalid); deleted records are struck through. Summaries use names,
/// numbers and dates, never addresses or digests.
pub(super) fn row_face(rows: &[crate::ledgerx::Row], reading: &crate::retractx::Reading, r: &crate::ledgerx::Row) -> (&'static str, String, bool) {
    if crate::retractx::is_retraction(r) {
        let said = match reading.invalid.get(&r.id) {
            Some((_, why)) => fill1(Key::V2RetractInvalid, t(invalid_key(*why))),
            None => {
                let subject = r.facts.subject.clone().unwrap_or_default();
                let name = rows.iter().find(|x| x.id.eq_ignore_ascii_case(&subject)).map(human_summary).unwrap_or_else(|| t(Key::UnnamedRecord).to_string());
                fill1(Key::V2RetractSummary, &name)
            }
        };
        return (t(Key::V2KindRetraction), said, false);
    }
    let struck = r.kind == zikaron::tokens::EntryType::History && reading.is_deleted(&r.id);
    (t(user_kind_key(r.kind)), summary_in(rows, r), struck)
}

/// A row's summary in plain words, reading the table for names: a grant is "record name · MM-DD to MM-DD",
/// a revocation "revoke grant #n", a handover "handed to a new key".
pub(super) fn summary_in(rows: &[crate::ledgerx::Row], row: &crate::ledgerx::Row) -> String {
    use zikaron::tokens::EntryType as E;
    let f = &row.facts;
    match row.kind {
        E::Grant => {
            let work = f.work.clone().unwrap_or_default();
            let name = rows
                .iter()
                .find(|x| x.kind == E::History && x.work.as_deref().map(|w| w.eq_ignore_ascii_case(&work)).unwrap_or(false))
                .map(human_summary)
                .unwrap_or_else(|| t(Key::UnnamedRecord).to_string());
            format!("{name} \u{b7} {}", window_short(f.window))
        }
        E::Revocation => {
            let subject = f.subject.clone().unwrap_or_default();
            match rows.iter().find(|x| x.id.eq_ignore_ascii_case(&subject)) {
                Some(g) => fill1(Key::SumRevokeGrant, &g.seq.to_string()),
                None => t(Key::PageRevoke).to_string(),
            }
        }
        E::Succession => t(Key::SumHandedToNewKey).to_string(),
        _ => human_summary(row),
    }
}

/// The page shell records for a view's tab (`shell.page`: tests and the trace channel read it).
pub(super) fn view_page(v: crate::nav::View, tab: u8, role: crate::roles::Role) -> Page {
    use crate::nav::{tab as T, View};
    match v {
        View::Works => match tab {
            T::WORKS_PENDING => Page::Queue,
            T::WORKS_KIT => Page::Kit,
            _ => Page::Anchoring,
        },
        View::Grants => match tab {
            T::GRANTS_NEW => Page::Grant,
            _ => Page::Grants,
        },
        View::Verify => match tab {
            T::VERIFY_WORK => Page::Verifier,
            T::VERIFY_OTHERS => match role {
                crate::roles::Role::Author => Page::Reader,
                crate::roles::Role::Grantee => Page::Diligence,
            },
            _ => Page::Check,
        },
        View::Log => Page::Ledger,
        View::Alerts => Page::Watch,
        View::MyGrants => match tab {
            T::HELD_RELICENSE => Page::Relicense,
            _ => Page::Vault,
        },
    }
}

/// The text of a verify tab.
pub(super) fn tab_key(v: crate::nav::View, tab: u8) -> Key {
    use crate::nav::{tab as T, View};
    match (v, tab) {
        (View::Verify, T::VERIFY_WORK) => Key::V2TabVerifyWork,
        (View::Verify, T::VERIFY_OTHERS) => Key::V2TabOthers,
        (View::Verify, _) => Key::V2TabCheckGrant,
        (_, _) => v.key(),
    }
}

/// A check's three states to a mark.
pub(super) fn state_mark(s: &str) -> Mark {
    tone_mark(crate::checkx::light_tone(s))
}

/// How a supplied file reads on the check page: its name and size, the pill word and tone.
pub(super) fn side_face(side: &crate::checkx::Side) -> (Option<String>, Key, PillTone) {
    match side {
        crate::checkx::Side::NotGiven => (None, Key::OsNotGiven, PillTone::Grey),
        crate::checkx::Side::Compared(d) => {
            let say = Some(width::file_name(&d.path));
            if d.matched() {
                (say, Key::DeliveryMatch, PillTone::Ok)
            } else {
                (say, Key::DeliveryMismatch, PillTone::Bad)
            }
        }
        crate::checkx::Side::Refused(_) => (None, Key::OsUnread, PillTone::Warn),
    }
}

/// First-anchor block time of a record, or a dash until a pass has read the chain.
pub(super) fn first_anchor_say(at: Option<u64>) -> String {
    at.map(crate::when::when).unwrap_or_else(|| t(Key::FirstAnchorUnread).to_string())
}

/// Byte count in plain words, in the units the system file dialog uses (decimal: KB rounded, MB and GB with
/// one decimal, whole numbers without .0). Display only, never for a decision.
pub(super) fn size_say(n: u64) -> String {
    let one = |x: u64, unit: u64| {
        let tenths = (x * 10 + unit / 2) / unit;
        if tenths % 10 == 0 { (tenths / 10).to_string() } else { format!("{}.{}", tenths / 10, tenths % 10) }
    };
    if n < 1000 {
        fill1(Key::SizeBytes, &n.to_string())
    } else if (n + 500) / 1000 < 1000 {
        fill1(Key::SizeKb, &((n + 500) / 1000).to_string())
    } else if n < 999_950_000 {
        fill1(Key::SizeMb, &one(n, 1_000_000))
    } else {
        fill1(Key::SizeGb, &one(n, 1_000_000_000))
    }
}

/// The name of a time zone on screen.
pub(super) fn zone_label(z: crate::when::Zone) -> &'static str {
    match z {
        crate::when::Zone::Utc => t(Key::ZoneUtc),
        crate::when::Zone::System => t(Key::ZoneSystem),
    }
}

/// A row's summary in plain words from the row alone (records by their name; the others by their note).
pub(super) fn human_summary(row: &crate::ledgerx::Row) -> String {
    use zikaron::tokens::EntryType as E;
    let f = &row.facts;
    let or_summary = |x: Option<String>| x.unwrap_or_else(|| row.summary.clone());
    match row.kind {
        E::History => f.note.clone().unwrap_or_else(|| t(Key::UnnamedRecord).to_string()),
        E::Grant => window_short(f.window),
        E::Revocation => t(Key::PageRevoke).to_string(),
        E::Adoption => {
            let n = fill1(Key::U3SumAnchors, &f.count.unwrap_or(0).to_string());
            if f.cosigned { format!("{n} \u{b7} {}", t(Key::U3SumCosigned)) } else { n }
        }
        E::Succession => t(Key::SumHandedToNewKey).to_string(),
        E::Annotation => or_summary(f.note.clone()),
        E::Genesis | E::Other => or_summary(f.note.clone()),
    }
}

/// Audit labels and check verdicts in plain words: passed, has gaps, failed. The raw word lives only in
/// details.
pub(super) fn verdict_key(token: &str) -> Option<Key> {
    use zikaron::tokens::Label as L;
    if let Some(l) = L::ALL.iter().find(|l| l.as_str() == token) {
        return Some(match l {
            L::Complete => Key::AuditPass,
            L::Gaps | L::Unavailable | L::NoLabel => Key::U3CheckSomeMissing,
            L::BrokenChain => Key::U3CheckFail,
        });
    }
    use zikaron_kit::tokens::CheckVerdict as V;
    if [V::Green, V::Partial, V::Fail].iter().any(|v| v.as_str() == token) {
        return Some(match crate::checkx::tone(token) {
            crate::checkx::Tone::Green => Key::U3CheckAllPass,
            crate::checkx::Tone::Amber => Key::U3CheckSomeMissing,
            crate::checkx::Tone::Red => Key::U3CheckFail,
            crate::checkx::Tone::Grey => Key::BadgeUnknown,
        });
    }
    None
}

/// An audit label in plain words (table in `verdict_key`).
pub(super) fn label_human(label: &str) -> String {
    verdict_key(label).map(|k| t(k).to_string()).unwrap_or_else(|| t(Key::BadgeUnknown).to_string())
}

/// A check verdict in plain words (table in `verdict_key`).
pub(super) fn verdict_human(verdict: &str) -> &'static str {
    t(verdict_key(verdict).unwrap_or(Key::BadgeUnknown))
}


/// A window as "MM-DD to MM-DD".
pub(super) fn window_short(w: Option<(u64, u64)>) -> String {
    let md = |s: u64| crate::when::day(s).chars().skip(5).collect::<String>();
    match w {
        Some((a, b)) => fill2(Key::U3FromTo, &md(a), &md(b)),
        None => t(Key::CountdownNoWindow).to_string(),
    }
}

/// A window as "YYYY-MM-DD to YYYY-MM-DD".
pub(super) fn window_long(w: Option<(u64, u64)>) -> String {
    match w {
        Some((a, b)) => fill2(Key::U3FromTo, &crate::when::day(a), &crate::when::day(b)),
        None => t(Key::CountdownNoWindow).to_string(),
    }
}
