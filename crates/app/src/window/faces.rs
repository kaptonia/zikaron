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
pub(super) fn without_window(line: &str, sentence: String) {
    crate::platform::say_without_window(line, &sentence);
}

/// Speak the language this machine last chose (`machine.json`'s `lang`; unreadable or never chosen: the
/// build's default), before any window or home has set it.
pub(super) fn speak_this_machines_language() {
    if let Some(l) = crate::machine::read_face().ok().and_then(|m| m.lang) {
        crate::lang::set(l);
    }
}

pub(super) fn fresh_win(shell: Shell, typed: Typed, ux: Ux) -> Win {
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

/// What the window said when the watch rang: the toast's words and tone, and whether it asked for the
/// system's attention.
pub struct RangReading {
    pub toast: Option<String>,
    pub tone: Option<zikaron_ui::toast::Tone>,
    pub attention: bool,
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

/// One frame of the wizard: the step it stood on, every text drawn with where, and how far a drawn check has
/// gone (see [`check_points`]).
pub struct WizFrame {
    pub step: String,
    pub texts: Vec<Drawn>,
    pub check: usize,
}

/// How far the drawn check of a finished step has gone this frame: the points of the longest open line in
/// the success colour (it only ever grows while the check draws; back to few means it started again).
pub(super) fn check_points(o: &egui::FullOutput) -> usize {
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

pub(super) fn texts_of(out: &egui::FullOutput) -> Vec<Drawn> {
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

/// What the window did with files dropped on it as a whole.
#[derive(Clone, Debug)]
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
    /// Whether the page drawn declared that it takes drops in its own zones (`Win::takes_drops`), so the
    /// whole-window drop stood aside.
    pub page_drops: bool,
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

/// What pressing the wizard's "back" key did, read from the window's own state after each press.
#[derive(Clone, Debug, Default)]
pub struct WizardBack {
    /// Whether the key was drawn (and so pressed) on the words' check page.
    pub pressed_on_check: bool,
    /// The step and whether the words' check was showing after that press.
    pub step_after_check: String,
    pub checking_after: bool,
    /// Whether the new words in hand were the same before and after (none dropped, none made anew).
    pub same_words: bool,
    /// Whether the key was drawn on the words' page, and the step after pressing it there.
    pub pressed_on_words: bool,
    pub step_after_words: String,
}

/// What the wizard's "adopt existing ledger folder" did at "create the ledger" (`probe_wizard_adopt`).
#[derive(Clone, Debug, Default)]
pub struct WizardAdopt {
    /// Whether the key was drawn beside "create the ledger" (and so pressed), and whether its sheet then showed.
    pub key_drawn: bool,
    pub sheet_drawn: bool,
    /// Whether the sheet's "adopt in place" was drawn (and so pressed) with the folder chosen.
    pub adopt_pressed: bool,
    /// The step the wizard stood at afterwards, and whether that step was done then (read from the shell).
    pub step_after: String,
    pub done_after: bool,
    /// Whether the step's "next" key, and the adopt key again, were drawn afterwards.
    pub next_drawn_after: bool,
    pub key_drawn_after: bool,
    /// The sentence the window showed for it, and the window's most recent trouble's sentences (first, next), if
    /// it was refused.
    pub toast: Option<String>,
    pub refused: Option<(String, String)>,
}

/// What clicking the other language on the first-run wizard's or the passcode gate's switch did.
#[derive(Clone, Debug, Default)]
pub struct LangPick {
    /// Whether the other language's cell was drawn (and so clicked).
    pub drawn: bool,
    /// The language spoken before and after the click (`lang::lang`), and the one this machine keeps after it.
    pub before: String,
    pub after: String,
    pub machine_after: String,
    /// Whether the wizard (or the gate) was still up after the click.
    pub still_up: bool,
}

/// What the "write alone" sheet did when its validity cells were typed into and its switch clicked
/// (`probe_cli_sheet`).
#[derive(Clone, Debug, Default)]
pub struct CliSheet {
    /// The days cell after "400" was typed into it, and whether the line under the switch said days are at
    /// most 365 (in red).
    pub days_typed: String,
    pub over_said: bool,
    /// The days cell after it was cleared and "2" typed, and whether the line then said when it expires.
    pub days_fixed: String,
    pub expiry_said: bool,
    /// Whether the switch was on after its title was clicked; the days cell after "9" was typed into it while
    /// on (not taken: unchanged); and whether the line under the switch was empty then.
    pub never_on: bool,
    pub days_while_never: String,
    pub quiet_while_never: bool,
    /// Whether the sheet said, as it opened, that the command line may spend this identity's gas while
    /// putting on chain from the command line sends automatically.
    pub gas_said: bool,
}

/// What typing at the passcode gate did, with nothing over it and with the lock screen's restore sheet over it.
#[derive(Clone, Debug, Default)]
pub struct TypedUnder {
    /// Characters the gate's passcode row held after three were typed with nothing over it.
    pub bare: usize,
    /// Whether the restore sheet was drawn over the gate, and its title pressed (a press on no control).
    pub sheet_up: bool,
    pub pressed: bool,
    /// Characters the gate's passcode row held after three were typed with the sheet over it.
    pub under: usize,
    /// Whether the vault stayed locked throughout.
    pub locked: bool,
}

/// One rail item walked by [`probe_rail_walk`]: its label and the route drawn after each move (`None` where the
/// page has no row to open, or its first row leads to another view).
#[derive(Clone, Debug, Default)]
pub struct RailWalk {
    pub item: String,
    /// The item was drawn on the rail and pressed.
    pub pressed: bool,
    /// After pressing it.
    pub entered: String,
    /// The page's first row leads to another view: the route drawn there.
    pub link: Option<String>,
    /// After opening the page's first row (a page of the same view).
    pub detail: Option<String>,
    /// After going back from that row (⌘[).
    pub out: Option<String>,
    /// After opening the row again and pressing the same rail item.
    pub rail_again: Option<String>,
}

pub(super) fn t_void() -> &'static str {
    t(Key::WizExitWords)
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
