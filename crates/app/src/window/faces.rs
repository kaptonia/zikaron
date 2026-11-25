use super::*;

impl Win {
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

    pub(super) fn remember_landing(&mut self, out: Out, c: &crate::home::Chosen) {
        self.ux.landings.retain(|(o, _)| *o != out);
        self.ux.landings.push((out, c.clone()));
    }

    pub(super) fn landing_note(&mut self, out: Out) -> Option<String> {
        let at = self.ux.landings.iter().position(|(o, _)| *o == out)?;
        let (_, c) = self.ux.landings.remove(at);
        c.why.say().map(|k| fill1(k, &width::file_name(&c.at.display().to_string())))
    }

    pub(super) fn landing_ok(given: &str) -> bool {
        crate::home::landing(given).is_ok()
    }

    pub(super) fn wiz_network_pick(&self) -> String {
        self.ux.wiz_network.clone().or_else(|| self.wiz_network_chosen()).unwrap_or_else(|| crate::machine::pick(&self.shell.machine))
    }

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
        crate::grantx::Badge::Unnoticed => Key::BadgeUnnoticed,
        crate::grantx::Badge::Unknown => Key::BadgeUnknown,
    }
}

pub(super) fn badge_tone(b: crate::grantx::Badge) -> PillTone {
    match b {
        crate::grantx::Badge::Live => PillTone::Ok,
        crate::grantx::Badge::Expired | crate::grantx::Badge::Unnoticed => PillTone::Warn,
        crate::grantx::Badge::NotYet | crate::grantx::Badge::Revoked | crate::grantx::Badge::Unknown => PillTone::Grey,
    }
}

pub(super) fn head_tail(a: &str) -> String {
    if a.len() <= 12 {
        return a.to_string();
    }
    format!("{}\u{2026}{}", &a[..6], &a[a.len() - 4..])
}

pub(super) fn network_label(name: &str) -> String {
    match crate::deploy::named(name) {
        Some(d) => t(d.label).to_string(),
        None => t(Key::U3Custom).to_string(),
    }
}

pub(super) fn appearance_label(a: &str) -> &'static str {
    match a {
        "dark" => t(Key::AppearanceDark),
        "system" => t(Key::ZoneSystem),
        _ => t(Key::AppearanceLight),
    }
}

pub(super) fn cli_anchor_label(c: crate::machine::CliAnchor) -> Key {
    match c {
        crate::machine::CliAnchor::Send => Key::CliAnchorSend,
        crate::machine::CliAnchor::Queue => Key::CliAnchorQueue,
    }
}

pub(super) fn proxy_label(c: &str) -> String {
    match c {
        crate::machine::proxy::SYSTEM => t(Key::ProxySystem).to_string(),
        crate::machine::proxy::NONE => t(Key::ProxyOff).to_string(),
        other => other.to_string(),
    }
}

pub(super) fn kind_key(k: zikaron_glue::book::Kind) -> Key {
    match k {
        zikaron_glue::book::Kind::Genesis => Key::WizGenesisTitle,
        zikaron_glue::book::Kind::History => Key::KindHistory,
        zikaron_glue::book::Kind::Grant => Key::KindGrant,
        zikaron_glue::book::Kind::Revocation => Key::KindRevocation,
        zikaron_glue::book::Kind::Adoption => Key::KindAdoption,
        zikaron_glue::book::Kind::Succession => Key::KindSuccession,
        zikaron_glue::book::Kind::Migration => Key::KindMigration,
        zikaron_glue::book::Kind::Annotation => Key::KindAnnotation,
        zikaron_glue::book::Kind::Retraction => Key::V2KindRetraction,
        zikaron_glue::book::Kind::Other => Key::KindOther,
        zikaron_glue::book::Kind::Face => Key::KindFace,
    }
}

pub(super) fn lamp_mark(l: crate::ledgerx::Lamp) -> Mark {
    use crate::ledgerx::Lamp;
    match l {
        Lamp::Anchored | Lamp::Remembered => Mark::Ok,
        Lamp::RememberedStale => Mark::Todo,
        Lamp::Included | Lamp::Submitted if receipts_stalled() => Mark::Warn,
        Lamp::Included | Lamp::Submitted => Mark::Busy,
        Lamp::Queued | Lamp::Deleted | Lamp::LocalDeletion | Lamp::ChainUnread | Lamp::Sealed => Mark::Todo,
        Lamp::Landed => Mark::Warn,
        Lamp::Reverted | Lamp::Refused => Mark::Bad,
    }
}

static RECEIPTS_STALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(super) fn set_receipts_stalled(stalled: bool) {
    RECEIPTS_STALLED.store(stalled, std::sync::atomic::Ordering::Relaxed);
}

fn receipts_stalled() -> bool {
    RECEIPTS_STALLED.load(std::sync::atomic::Ordering::Relaxed)
}

pub(super) fn lamp_pill(l: crate::ledgerx::Lamp) -> (PillTone, bool) {
    use crate::ledgerx::Lamp;
    match l {
        Lamp::Anchored | Lamp::Remembered => (PillTone::Ok, false),
        Lamp::RememberedStale => (PillTone::Grey, false),
        Lamp::Submitted if receipts_stalled() => (PillTone::Warn, false),
        Lamp::Submitted => (PillTone::Blue, true),
        Lamp::Included => (PillTone::Blue, false),
        Lamp::Queued => (PillTone::Grey, false),
        Lamp::Landed | Lamp::Deleted | Lamp::LocalDeletion | Lamp::ChainUnread | Lamp::Sealed => (PillTone::Grey, false),
        Lamp::Reverted | Lamp::Refused => (PillTone::Bad, false),
    }
}

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
        Lamp::Sealed => Key::LampSealed,
    }
}

pub fn lamp_label(l: crate::ledgerx::Lamp, detail: bool, at: Option<u64>) -> String {
    use crate::ledgerx::Lamp;
    match l {
        Lamp::Remembered | Lamp::RememberedStale if !detail => fill1(Key::LampRememberedShort, &at.map(hm_of).unwrap_or_default()),
        Lamp::Remembered | Lamp::RememberedStale => fill1(Key::LampRemembered, &at.map(hhmm_of).unwrap_or_default()),
        _ => t(lamp_key(l, detail)).to_string(),
    }
}

pub(super) fn hm_of(secs: u64) -> String {
    crate::when::when(secs).chars().skip(11).take(5).collect()
}

pub(super) fn hhmm_of(secs: u64) -> String {
    crate::when::short(secs)
}

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

pub(super) fn id_seat_row_key(r: crate::roles::Role) -> Key {
    match r {
        crate::roles::Role::Author => Key::IdSeatRowAuthor,
        crate::roles::Role::Grantee => Key::IdSeatRowGrantee,
    }
}

pub(super) fn id_kind_key(k: crate::identity::Kind) -> Key {
    match k {
        crate::identity::Kind::Words => Key::IdKindWords,
        crate::identity::Kind::Existing => Key::IdKindExisting,
    }
}

pub const ARGS_LINE: &str = "E_ARGS the window takes no arguments";

pub fn refuse_arguments() -> i32 {
    speak_this_machines_language();
    without_window(ARGS_LINE, t(Key::SaidNoWindowArgs).to_string());
    2
}

pub(super) fn without_window(line: &str, sentence: String) {
    crate::platform::say_without_window(line, &sentence);
}

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
        card_follows: false,
        ux,
        asker: crate::platform::ask_path,
        asks_which: crate::platform::asks_file_or_folder(),
        backdrop: None,
        asks_motion: false,
        reads_behind: false,
        reduced: false,
        motion_asked: None,
        painted_lang: crate::lang::lang(),
        drops: None,
    }
}

pub struct SendProbe {
    win: Win,
    t: f64,
    held: Option<SendHeld>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendHeld {
    NotDrawn,
    Reading,
    Out,
    Nothing,
    Greyed,
}

impl SendProbe {
    pub fn pressed(shell: Shell, count: usize) -> SendProbe {
        let mut ux = Ux::default();
        ux.wizard_asked = true;
        ux.u3.confirm = Some(U3Confirm::Send { count });
        ux.u3.sending = Some(shell.tasks.landings(crate::task::Kind::Anchor));
        let mut win = fresh_win(shell, Typed::default(), ux);
        win.faults_told = usize::MAX;
        SendProbe { win, t: 0.0, held: None }
    }

    pub fn frame(&mut self, ctx: &egui::Context) -> bool {
        self.t += 0.1;
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 760.0));
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(self.t), ..Default::default() };
        let win = &mut self.win;
        let _ = ctx.run_ui(input, |ui| win.draw(ui));
        self.win.ux.u3.confirm.is_some()
    }

    pub fn shell(&mut self) -> &mut Shell {
        &mut self.win.shell
    }

    pub fn open(shell: Shell, count: usize) -> SendProbe {
        let mut ux = Ux::default();
        ux.wizard_asked = true;
        ux.u3.confirm = Some(U3Confirm::Send { count });
        let mut win = fresh_win(shell, Typed::default(), ux);
        win.faults_told = usize::MAX;
        SendProbe { win, t: 0.0, held: None }
    }

    pub fn home(shell: Shell) -> SendProbe {
        let mut ux = Ux::default();
        ux.wizard_asked = true;
        let mut win = fresh_win(shell, Typed::default(), ux);
        win.faults_told = usize::MAX;
        win.send_all_unanchored(0.0);
        SendProbe { win, t: 0.0, held: None }
    }

    pub(super) fn over(win: Win, t: f64, held: Option<SendHeld>) -> SendProbe {
        SendProbe { win, t, held }
    }

    pub fn held(&self) -> Option<SendHeld> {
        self.held
    }

    pub fn into_shell(self) -> Shell {
        self.win.shell
    }

    pub fn press(&mut self) -> bool {
        let Some(U3Confirm::Send { count }) = self.win.ux.u3.confirm.clone() else { return false };
        self.win.send_go(count, self.t);
        true
    }

    pub fn sheet(&self) -> SendSheet {
        match &self.win.ux.u3.confirm {
            Some(U3Confirm::Send { count }) => SendSheet::Batch { count: *count, sending: self.win.ux.u3.sending.is_some(), gas: self.win.shell.gas.filter(|(n, _)| n == count).map(|(_, g)| g) },
            Some(U3Confirm::NotSent { count, why, next, raw }) => SendSheet::NotSent { count: *count, why: why.clone(), next: next.clone(), raw: raw.clone() },
            Some(U3Confirm::NoEstimate { count, raw, .. }) => SendSheet::NoEstimate { count: *count, raw: raw.clone() },
            Some(_) => SendSheet::Other,
            None => SendSheet::Closed,
        }
    }

    pub fn drawn(&mut self, ctx: &egui::Context) -> Vec<String> {
        self.t += 0.1;
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 760.0));
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(self.t), ..Default::default() };
        let win = &mut self.win;
        let out = ctx.run_ui(input, |ui| win.draw(ui));
        texts_of(&out).into_iter().map(|d| d.text).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SendSheet {
    Closed,
    Batch { count: usize, sending: bool, gas: Option<u64> },
    NotSent { count: usize, why: String, next: String, raw: String },
    NoEstimate { count: usize, raw: String },
    Other,
}

pub struct KitProbe {
    win: Win,
    t: f64,
}

impl KitProbe {
    pub fn new(shell: Shell, on: bool) -> KitProbe {
        let mut ux = Ux::default();
        ux.wizard_asked = true;
        ux.u3.kit_addressed = on;
        let mut win = fresh_win(shell, Typed::default(), ux);
        win.faults_told = usize::MAX;
        KitProbe { win, t: 0.0 }
    }

    pub fn shell(&mut self) -> &mut Shell {
        &mut self.win.shell
    }

    pub fn lands(&mut self, ctx: &egui::Context) -> bool {
        self.win.ux.asked.push(crate::task::Kind::Kit);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 760.0));
        let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while self.win.ux.asked.contains(&crate::task::Kind::Kit) && std::time::Instant::now() < until {
            self.t += 0.1;
            let input = egui::RawInput { screen_rect: Some(screen), time: Some(self.t), ..Default::default() };
            let win = &mut self.win;
            let _ = ctx.run_ui(input, |ui| win.draw(ui));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        self.win.ux.u3.kit_addressed
    }
}

pub struct UnseenProbe {
    win: Win,
    t: f64,
    frame: eframe::Frame,
}

impl UnseenProbe {
    pub fn new(shell: Shell) -> UnseenProbe {
        let mut ux = Ux::default();
        ux.wizard_asked = true;
        let mut win = fresh_win(shell, Typed::default(), ux);
        win.faults_told = usize::MAX;
        UnseenProbe { win, t: 0.0, frame: eframe::Frame::_new_kittest() }
    }

    pub fn pass(&mut self, ctx: &egui::Context, step: f64) {
        self.t += step;
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 760.0));
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(self.t), ..Default::default() };
        let (win, frame) = (&mut self.win, &mut self.frame);
        let _ = ctx.run_ui(input, |ui| eframe::App::logic(win, ui.ctx(), frame));
    }

    pub fn drawn(&mut self, ctx: &egui::Context, step: f64) {
        self.t += step;
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1180.0, 760.0));
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(self.t), ..Default::default() };
        let win = &mut self.win;
        let _ = ctx.run_ui(input, |ui| win.draw(ui));
    }

    pub fn shell(&mut self) -> &mut Shell {
        &mut self.win.shell
    }
}

pub struct PathFrame {
    pub ask: Vec<(&'static str, crate::platform::Pick)>,
    pub take: Vec<&'static str>,
    pub answer: bool,
}

pub struct PathRead {
    pub took: Vec<(&'static str, Option<String>)>,
    pub faults: Vec<crate::fault::Fault>,
    pub open: bool,
}

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
        let _ = ctx.run_ui(input, |ui| {
            let c = &ui.ctx().clone();
            if c.current_pass_index() == 0 {
                for (n, kind) in &f.ask {
                    let _ = path_answer(c, site(n), true, *kind);
                }
            }
            win.draw(ui);
        });
        let input = egui::RawInput { screen_rect: Some(screen), time: Some(t + 0.05), ..Default::default() };
        t += 0.05;
        let _ = ctx.run_ui(input, |ui| {
            let c = &ui.ctx().clone();
            win.draw(ui);
            if c.current_pass_index() == 0 {
                for n in &f.take {
                    took.push((*n, path_answer(c, site(n), false, crate::platform::Pick::File)));
                }
            }
        });
        out.push(PathRead { took, faults: win.shell.faults.clone(), open: win.shell.tasks.in_flight(crate::task::Kind::Path) });
    }
    (win.shell, out)
}

pub struct RangReading {
    pub toast: Option<String>,
    pub tone: Option<zikaron_ui::toast::Tone>,
    pub attention: bool,
}

pub struct Drawn {
    pub text: String,
    pub rect: egui::Rect,
    pub rows: usize,
    pub row_chars: Vec<usize>,
    pub baseline: f32,
}

pub struct WizFrame {
    pub step: String,
    pub texts: Vec<Drawn>,
    pub check: usize,
}

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

#[derive(Clone, Debug)]
pub struct DropReading {
    pub route: String,
    pub sheet: Option<u8>,
    pub taken: Option<String>,
    pub name: String,
    pub verify_path: String,
    pub toast: Option<String>,
    pub page_drops: bool,
}

pub struct WizardExit {
    pub before: String,
    pub opened: bool,
    pub after: String,
    pub offered: bool,
    pub still_open: bool,
    pub asked: bool,
    pub asked_drawn: bool,
    pub words_dropped: bool,
    pub wiz_from: String,
    pub place_after: String,
}

#[derive(Clone, Debug, Default)]
pub struct WizardBack {
    pub pressed_on_check: bool,
    pub step_after_check: String,
    pub checking_after: bool,
    pub same_words: bool,
    pub pressed_on_words: bool,
    pub step_after_words: String,
}

#[derive(Clone, Debug, Default)]
pub struct WizardAdopt {
    pub key_drawn: bool,
    pub sheet_drawn: bool,
    pub adopt_pressed: bool,
    pub step_after: String,
    pub done_after: bool,
    pub next_drawn_after: bool,
    pub key_drawn_after: bool,
    pub toast: Option<String>,
    pub refused: Option<(String, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct LangPick {
    pub drawn: bool,
    pub before: String,
    pub after: String,
    pub machine_after: String,
    pub still_up: bool,
}

#[derive(Clone, Debug, Default)]
pub struct WorkPick {
    pub drawn: Vec<String>,
    pub after_grey: (String, bool),
    pub after_open: (String, bool),
}

#[derive(Clone, Debug, Default)]
pub struct SegTab {
    pub pressed: bool,
    pub entry_before: u64,
    pub entry_after: u64,
    pub route: String,
    pub page: String,
    pub history_empty: bool,
    pub scroll_before: f32,
    pub scroll_after: f32,
}

#[derive(Default)]
pub struct RelicenseSign {
    pub scroll_before: f32,
    pub confirm_pressed: bool,
    pub go_pressed: bool,
    pub signed: bool,
    pub frames_to_sign: usize,
    pub scroll_after: f32,
    pub texts: Vec<Drawn>,
    pub scroll_again: f32,
}

#[derive(Default)]
pub struct GrantEnd {
    pub live: Vec<String>,
    pub days: Vec<String>,
    pub off_lasts: [String; 3],
    pub off_open: bool,
    pub lasts: [String; 3],
    pub from: String,
    pub to: String,
    pub open_after: bool,
    pub to_then: String,
    pub picture: Option<Picture>,
}

#[derive(Clone, Debug, Default)]
pub struct CliSheet {
    pub days_typed: String,
    pub over_said: bool,
    pub days_fixed: String,
    pub expiry_said: bool,
    pub never_on: bool,
    pub days_while_never: String,
    pub quiet_while_never: bool,
}

#[derive(Clone, Debug, Default)]
pub struct TypedUnder {
    pub bare: usize,
    pub sheet_up: bool,
    pub pressed: bool,
    pub under: usize,
    pub locked: bool,
}

#[derive(Clone, Debug, Default)]
pub struct WizPinTyped {
    pub first: Vec<usize>,
    pub asked_again: bool,
    pub second: Vec<usize>,
    pub vault: Option<crate::shell::Vault>,
    pub next_drawn: bool,
    pub after: Option<crate::nav::Step>,
    pub cover_lang: Option<(crate::lang::Lang, crate::lang::Lang)>,
}

#[derive(Clone, Debug, Default)]
pub struct IdMenuPress {
    pub pressed: bool,
    pub wish: Option<String>,
    pub flying: Vec<String>,
}

#[derive(Default)]
pub struct BackupRun {
    pub pressed: bool,
    pub frames: Vec<BackupFrame>,
    pub open_after_esc: Option<bool>,
    pub open_after_back: Option<bool>,
    pub open_after: bool,
    pub trouble: Option<crate::fault::Fault>,
    pub picture: Option<Picture>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BackupFrame {
    pub open: bool,
    pub running: bool,
    pub stage: Option<u8>,
    pub words: Vec<String>,
    pub n: usize,
}

#[derive(Clone, Debug, Default)]
pub struct AnnotateTarget {
    pub from_detail: String,
    pub cancelled: bool,
    pub wrote_first: bool,
    pub after_write: String,
    pub closed: bool,
    pub reopened: bool,
    pub reopened_names: String,
    pub none_shown: bool,
    pub after_pick: String,
    pub after_none: String,
    pub wrote_second: bool,
}

#[derive(Clone, Debug, Default)]
pub struct GateAside {
    pub key_drawn: bool,
    pub sheet: Vec<String>,
    pub confirmed: bool,
    pub vault: Option<crate::shell::Vault>,
    pub wizard: Option<crate::nav::Step>,
    pub trouble: Option<crate::fault::Fault>,
}

#[derive(Clone, Debug, Default)]
pub struct GateReset {
    pub pressed: bool,
    pub vault: Option<crate::shell::Vault>,
    pub wizard: Option<crate::nav::Step>,
}

#[derive(Clone, Debug)]
pub struct GateColumn {
    pub window_h: f32,
    pub group_top: f32,
    pub title_ink_bottom: f32,
    pub cells: egui::Rect,
    pub said: Option<egui::Rect>,
    pub forgot: egui::Rect,
    pub own_key: egui::Rect,
}

#[derive(Clone, Debug, Default)]
pub struct RailWalk {
    pub item: String,
    pub pressed: bool,
    pub entered: String,
    pub link: Option<String>,
    pub detail: Option<String>,
    pub out: Option<String>,
    pub rail_again: Option<String>,
    pub from_elsewhere: Option<String>,
    pub forward: Option<String>,
}

pub(super) fn work_state(w: &WorkLine, at: Option<u64>) -> (String, PillTone, bool) {
    match w.deleted {
        Some(_) => (t(Key::V2Deleted).to_string(), PillTone::Grey, false),
        None => {
            let (tone, live) = lamp_pill(w.lamp);
            (lamp_label(w.lamp, false, at), tone, live)
        }
    }
}

pub fn row_face_words(rows: &[crate::ledgerx::Row], r: &crate::ledgerx::Row, today: Option<u64>) -> (String, String) {
    let (k, said, _) = row_face(rows, &crate::retractx::read(rows), r, today);
    (k.to_string(), said)
}

pub(super) fn invalid_key(why: crate::retractx::Invalid) -> Key {
    use crate::retractx::Invalid;
    match why {
        Invalid::Shape => Key::V2InvalidShape,
        Invalid::NotInLedger => Key::V2InvalidNotInLedger,
        Invalid::NotAWork => Key::V2InvalidNotAWork,
        Invalid::Repeated => Key::V2InvalidRepeated,
    }
}

pub(super) fn row_face(rows: &[crate::ledgerx::Row], reading: &crate::retractx::Reading, r: &crate::ledgerx::Row, today: Option<u64>) -> (&'static str, String, bool) {
    if crate::retractx::is_retraction(r) {
        let said = match reading.invalid.get(&r.id) {
            Some((_, why)) => fill1(Key::V2RetractInvalid, t(invalid_key(*why))),
            None => {
                let subject = r.facts.subject.clone().unwrap_or_default();
                let name = rows.iter().find(|x| x.id.eq_ignore_ascii_case(&subject)).map(|x| human_summary(x, today)).unwrap_or_else(|| t(Key::UnnamedRecord).to_string());
                fill1(Key::V2RetractSummary, &name)
            }
        };
        return (t(Key::V2KindRetraction), said, false);
    }
    let struck = r.kind == zikaron_glue::book::Kind::History && reading.is_deleted(&r.id);
    (t(row_kind_key(r)), summary_in(rows, r, today), struck)
}

pub(super) fn summary_in(rows: &[crate::ledgerx::Row], row: &crate::ledgerx::Row, today: Option<u64>) -> String {
    use zikaron_glue::book::Kind as E;
    let f = &row.facts;
    match row.kind {
        E::Grant => {
            let work = f.work.clone().unwrap_or_default();
            let name = rows
                .iter()
                .find(|x| x.kind == E::History && x.work.as_deref().map(|w| w.eq_ignore_ascii_case(&work)).unwrap_or(false))
                .map(|x| human_summary(x, today))
                .unwrap_or_else(|| t(Key::UnnamedRecord).to_string());
            format!("{name} \u{b7} {}", window_short(f.window, today))
        }
        E::Revocation => {
            let subject = f.subject.clone().unwrap_or_default();
            match rows.iter().find(|x| x.id.eq_ignore_ascii_case(&subject)) {
                Some(g) => fill1(Key::SumRevokeGrant, &g.seq.to_string()),
                None => t(Key::PageRevoke).to_string(),
            }
        }
        E::Succession => succession_summary(row),
        _ => human_summary(row, today),
    }
}

pub(super) fn state_mark(s: &str) -> Mark {
    tone_mark(crate::checkx::light_tone(s))
}

pub(super) fn state_human(s: &str) -> &'static str {
    t(match crate::checkx::light_tone(s) {
        crate::checkx::Tone::Green => Key::AuditPass,
        crate::checkx::Tone::Red => Key::U3CheckFail,
        _ => Key::OsOpen,
    })
}

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

pub(super) fn first_anchor_say(at: Option<u64>) -> String {
    at.map(crate::when::when).unwrap_or_else(|| t(Key::FirstAnchorUnread).to_string())
}

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

pub(super) fn zone_label(z: crate::when::Zone) -> &'static str {
    match z {
        crate::when::Zone::Utc => t(Key::ZoneUtc),
        crate::when::Zone::System => t(Key::ZoneSystem),
    }
}

fn succession_summary(row: &crate::ledgerx::Row) -> String {
    t(if closes(row) { Key::SumClosesOldLedger } else { Key::SumHandedToNewKey }).to_string()
}

pub(super) fn closes(row: &crate::ledgerx::Row) -> bool {
    row.law == zikaron_glue::book::Law::One && row.kind == zikaron_glue::book::Kind::Succession && row.facts.to.as_deref().is_some_and(|to| to.eq_ignore_ascii_case(&row.author))
}

pub(super) fn row_kind_key(row: &crate::ledgerx::Row) -> Key {
    if closes(row) {
        Key::SumMovedToNewLaw
    } else {
        user_kind_key(row.kind)
    }
}

pub(super) fn type_words(row: &crate::ledgerx::Row) -> (Key, Option<String>) {
    if crate::retractx::is_retraction(row) {
        return (Key::V2KindRetraction, None);
    }
    let foreign = (row.kind == zikaron_glue::book::Kind::Other && !row.facts.raw_type.is_empty()).then(|| row.facts.raw_type.clone());
    (row_kind_key(row), foreign)
}

pub(super) fn human_summary(row: &crate::ledgerx::Row, today: Option<u64>) -> String {
    use zikaron_glue::book::Kind as E;
    let f = &row.facts;
    let or_summary = |x: Option<String>| x.unwrap_or_else(|| row.summary.clone());
    match row.kind {
        E::History => f.record_name().unwrap_or_else(|| t(Key::UnnamedRecord).to_string()),
        E::Grant => window_short(f.window, today),
        E::Revocation => t(Key::PageRevoke).to_string(),
        E::Adoption => t(Key::KindAdoption).to_string(),
        E::Succession => succession_summary(row),
        E::Annotation | E::Retraction | E::Migration => or_summary(f.note.clone()),
        E::Genesis | E::Other => or_summary(f.note.clone()),
        E::Face => face_summary(f).unwrap_or_else(|| row.summary.clone()),
    }
}

fn face_summary(f: &crate::ledgerx::Facts) -> Option<String> {
    let (shape, a, b) = f.face.as_ref()?;
    Some(match *shape {
        "home" => fill1(Key::FaceSummaryHome, a),
        "account" => crate::lang::filln(Key::FaceSummaryAccount, &[a, b]),
        "key" => crate::lang::filln(Key::FaceSummaryKey, &[&crate::ledgerx::short(a), b]),
        "persona" => t(Key::FaceSummaryPersona).to_string(),
        "release" => t(Key::FaceSummaryRelease).to_string(),
        "reroot" => format!("{} {}", t(Key::PsMovedTo), crate::ledgerx::short(a)),
        "origin" => format!("{} {}", t(Key::PsCameFrom), crate::ledgerx::short(a)),
        _ => return None,
    })
}

pub(super) fn verdict_key(token: &str) -> Option<Key> {
    use zikaron::v2::tokens::Label as L;
    if let Some(l) = L::ALL.iter().find(|l| l.as_str() == token) {
        return Some(match l {
            L::Complete => Key::AuditPass,
            L::Gaps | L::Unproven => Key::U3CheckSomeMissing,
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

pub(super) fn label_human(label: &str) -> String {
    verdict_key(label).map(|k| t(k).to_string()).unwrap_or_else(|| t(Key::BadgeUnknown).to_string())
}

pub(super) fn verdict_human(verdict: &str) -> &'static str {
    t(verdict_key(verdict).unwrap_or(Key::BadgeUnknown))
}

pub(super) fn window_short(w: Option<(u64, u64)>, now: Option<u64>) -> String {
    let year = |s: u64| crate::when::day(s).chars().take(4).collect::<String>();
    let md = |s: u64| crate::when::day(s).chars().skip(5).collect::<String>();
    match (w, now) {
        (Some((a, b)), Some(n)) if year(a) == year(n) && year(b) == year(n) => fill2(Key::U3FromTo, &md(a), &md(b)),
        _ => window_long(w),
    }
}

pub(super) fn window_long(w: Option<(u64, u64)>) -> String {
    match w {
        Some((a, b)) => fill2(Key::U3FromTo, &crate::when::day(a), &crate::when::day(b)),
        None => t(Key::CountdownNoWindow).to_string(),
    }
}
