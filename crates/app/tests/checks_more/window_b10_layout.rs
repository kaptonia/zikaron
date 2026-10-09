//! The window at its least size in English, Esc over the first-run wizard, the window handle on a maximized
//! window, and the proxy section with no "now" line: each read from the real window in windowless frames (the
//! product's own probes), on a temporary machine directory of this test process.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use app::lang::{t, Key, Lang};
use app::nav::{Place, Section, Step};
use zikaron_ui::egui::{self, pos2, Event, PointerButton, Rect};
use super::vault_open;


/// A booted shell for the window probes.
fn probe_shell(ctx: &egui::Context) -> app::shell::Shell {
    vault_open();
    app::shell::Shell::boot(zikaron_ui::skin::dress(ctx))
}

/// The language is one switch for the whole process: every test here runs its body alone, in English, and puts
/// the language back after (also when the body fails).
fn english<R>(body: impl FnOnce() -> R) -> R {
    static ONE: Mutex<()> = Mutex::new(());
    let _held = ONE.lock().unwrap_or_else(|e| e.into_inner());
    struct Back(Lang);
    impl Drop for Back {
        fn drop(&mut self) {
            app::lang::set(self.0);
        }
    }
    let _back = Back(app::lang::lang());
    app::lang::set(Lang::En);
    body()
}

/// An identity's address for the steps past the identity step (the wizard gates them on it).
fn some_anchor() -> app::key::Address {
    app::key::Address([0x5d; 20])
}

/// One text drawn: its words, its drawn rectangle, and the rectangle it is clipped to.
type Seen = (String, Rect, Rect);

/// Every text shape in a frame's output (nested ones too).
fn texts_in(shapes: &[egui::epaint::ClippedShape]) -> Vec<Seen> {
    fn walk(s: &egui::Shape, clip: Rect, out: &mut Vec<Seen>) {
        match s {
            egui::Shape::Text(x) => out.push((x.galley.job.text.clone(), x.visual_bounding_rect(), clip)),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, clip, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for c in shapes {
        walk(&c.shape, c.clip_rect, &mut out);
    }
    out
}

/// A tap on the context, through egui's own plugin hooks (no product code): the windowing layer's report of
/// whether the window is maximized is set on each frame's input, and each frame's texts are kept (the last pass
/// of each frame).
struct Tap {
    ctx: egui::Context,
    maximized: Option<bool>,
    frames: Arc<Mutex<BTreeMap<u64, Vec<Seen>>>>,
}

impl egui::Plugin for Tap {
    fn debug_name(&self) -> &'static str {
        "b10-tap"
    }

    fn input_hook(&mut self, input: &mut egui::RawInput) {
        if let Some(m) = self.maximized {
            let at = input.viewport_id;
            input.viewports.entry(at).or_default().maximized = Some(m);
        }
    }

    fn output_hook(&mut self, output: &mut egui::FullOutput) {
        let nr = self.ctx.cumulative_frame_nr();
        self.frames.lock().unwrap().insert(nr, texts_in(&output.shapes));
    }
}

/// A fresh context with the tap on it; the frames it keeps, in order.
fn tapped(maximized: Option<bool>) -> (egui::Context, Arc<Mutex<BTreeMap<u64, Vec<Seen>>>>) {
    let ctx = egui::Context::default();
    let frames = Arc::new(Mutex::new(BTreeMap::new()));
    ctx.add_plugin(Tap { ctx: ctx.clone(), maximized, frames: frames.clone() });
    (ctx, frames)
}

fn frames_of(frames: &Arc<Mutex<BTreeMap<u64, Vec<Seen>>>>) -> Vec<Vec<Seen>> {
    frames.lock().unwrap().values().cloned().collect()
}

/// Where `text` is drawn in a frame (exactly that text).
fn at(frame: &[Seen], text: &str) -> Option<Rect> {
    frame.iter().find(|(s, _, _)| s == text).map(|(_, r, _)| *r)
}

/// A click at `p`: the pointer moves there in one frame, presses and releases in the next.
fn click(p: egui::Pos2) -> Vec<(f64, Vec<Event>)> {
    let press = |down| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: down, modifiers: Default::default() };
    vec![(0.1, vec![Event::PointerMoved(p)]), (0.1, vec![press(true), press(false)]), (0.1, vec![Event::PointerGone])]
}

fn idle(n: usize) -> Vec<(f64, Vec<Event>)> {
    (0..n).map(|_| (0.1, Vec::new())).collect()
}

fn esc() -> Vec<(f64, Vec<Event>)> {
    vec![(0.1, vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }])]
}

/// The probes settle five frames before the first step: frame `5 + i` is step `i`.
const SETTLE: usize = 5;

fn inside(inner: Rect, outer: Rect) -> bool {
    inner.min.x >= outer.min.x - 0.5 && inner.max.x <= outer.max.x + 0.5 && inner.min.y >= outer.min.y - 0.5 && inner.max.y <= outer.max.y + 0.5
}

// ───────────────────── 1 · The wizard's bottom row at the least width ─────────────────────

/// At the window's least size in English, the identity step's bottom row lays "Back" where it does not cross
/// the step's own keys ("Generate recovery words", "Import an existing key…", "Restore from backup…").
#[test]
fn at_the_least_width_the_wizards_back_key_does_not_cross_the_steps_own_keys() {
    if super::alone_in(module_path!(), "at_the_least_width_the_wizards_back_key_does_not_cross_the_steps_own_keys") {
        return;
    }
    english(|| {
        let ctx = egui::Context::default();
        let shell = probe_shell(&ctx);
        assert!(shell.anchor.is_none(), "the identity step stands open (no identity on this machine yet)");
        let (_, texts, _) = app::window::probe_wizard_step(&ctx, shell, Place::Home, Step::Key, app::window::MIN_W, app::window::MIN_H);
        let find = |k: Key| texts.iter().find(|d| d.text == t(k)).map(|d| d.rect).unwrap_or_else(|| panic!("{:?} is drawn: {:?}", t(k), texts.iter().map(|d| &d.text).collect::<Vec<_>>()));
        let back = find(Key::WizPrev);
        let right: Vec<(Key, Rect)> = [Key::WizDoGenerate, Key::IdDoImportExisting, Key::DoRestoreBackup].into_iter().map(|k| (k, find(k))).collect();
        for (k, r) in &right {
            assert!(!back.intersects(*r), "\"Back\" {back:?} crosses {:?} {r:?}", t(*k));
            assert!(r.max.x <= app::window::MIN_W + 0.5, "{:?} runs past the window: {r:?}", t(*k));
        }
        assert!(back.min.x >= 0.0 && back.max.x <= app::window::MIN_W + 0.5, "\"Back\" stays in the window: {back:?}");
        // Every key stays in the step's own column (the count line above it marks the column's left edge):
        // none is laid onto the list of steps beside it.
        let count = app::lang::fill2(Key::WizCount, "2", &Step::ALL.len().to_string());
        let left = texts.iter().find(|d| d.text == count).map(|d| d.rect.min.x).expect("the step count is drawn");
        for (k, r) in right.iter().chain(std::iter::once(&(Key::WizPrev, back))) {
            assert!(r.min.x >= left - 0.5, "{:?} {r:?} reaches left of the step's column ({left})", t(*k));
        }
    })
}

// ───────────────────── 2 · The custom network row's note at the least width ─────────────────────

/// At the least size in English, the network step's "Custom" row holds its whole note: every text of the row
/// (its title and the note under it) lies inside the row's own rectangle.
#[test]
fn at_the_least_width_the_custom_network_rows_note_stays_inside_its_row() {
    if super::alone_in(module_path!(), "at_the_least_width_the_custom_network_rows_note_stays_inside_its_row") {
        return;
    }
    english(|| {
        let ctx = egui::Context::default();
        let mut shell = probe_shell(&ctx);
        shell.anchor = Some(some_anchor());
        let (_, texts, _) = app::window::probe_wizard_step(&ctx, shell, Place::Home, Step::Network, app::window::MIN_W, app::window::MIN_H);
        let title = texts.iter().find(|d| d.text == t(Key::U3Custom)).map(|d| d.rect).expect("the Custom row is drawn");
        let note = texts.iter().find(|d| d.text == t(Key::WizNetCustomSay)).expect("the Custom row's note is drawn");
        // The row: the smallest clickable widget around its title (the row is one click target).
        let row = ctx
            .viewport(|v| v.prev_pass.widgets.layers().flat_map(|(_, ws)| ws.iter().filter(|w| w.sense.senses_click()).map(|w| w.rect).collect::<Vec<_>>()).collect::<Vec<_>>())
            .into_iter()
            .filter(|r| r.contains(title.center()))
            .min_by(|a, b| a.area().total_cmp(&b.area()))
            .expect("the Custom row is a widget");
        assert!(inside(title, row), "the title {title:?} stays in its row {row:?}");
        assert!(inside(note.rect, row), "the note {:?} ({} rows) stays in its row {row:?}", note.rect, note.rows);
        assert!(row.max.x <= app::window::MIN_W + 0.5, "the row stays in the window: {row:?}");
    })
}

// ───────────────────── 3 · The gas code's caption at the least width ─────────────────────

/// At the least size in English, the caption under the gas step's code ("Scan with a wallet to pay") is drawn
/// whole and within the code's plate width: every letter laid out, no wider than the plate, within the plate's
/// width under it, and not cut by what it is clipped to.
#[test]
fn at_the_least_width_the_gas_codes_caption_is_whole_within_the_plate() {
    if super::alone_in(module_path!(), "at_the_least_width_the_gas_codes_caption_is_whole_within_the_plate") {
        return;
    }
    english(|| {
        let (ctx, frames) = tapped(None);
        let mut shell = probe_shell(&ctx);
        shell.anchor = Some(some_anchor());
        let (_, texts, plates) = app::window::probe_wizard_step(&ctx, shell, Place::Home, Step::Gas, app::window::MIN_W, app::window::MIN_H);
        let plate = *plates.first().expect("the code's plate is drawn");
        assert_eq!(plates.len(), 1, "one code: {plates:?}");
        let said = t(Key::WizGasScan);
        let cap = texts.iter().find(|d| d.text == said).expect("the caption is drawn");
        let glyphs: usize = cap.row_chars.iter().sum();
        assert!(glyphs >= said.chars().filter(|c| !c.is_whitespace()).count(), "every letter is laid out: {glyphs} of {said:?}");
        assert!(cap.rect.width() <= plate.width() + 0.5, "the caption {:?} is no wider than the plate {plate:?}", cap.rect);
        assert!(cap.rect.min.x >= plate.min.x - 0.5 && cap.rect.max.x <= plate.max.x + 0.5, "the caption {:?} lies within the plate's width {plate:?}", cap.rect);
        assert!(cap.rect.min.y >= plate.max.y - 0.5, "the caption is under the plate: {:?} {plate:?}", cap.rect);
        assert!(cap.rect.max.x <= app::window::MIN_W + 0.5 && cap.rect.max.y <= app::window::MIN_H + 0.5, "the caption is not cut by the window: {:?}", cap.rect);
        let last = frames_of(&frames).pop().expect("frames ran");
        let (_, _, clip) = last.iter().find(|(s, _, _)| s == said).expect("the caption is in the last frame");
        assert!(inside(cap.rect, *clip), "the caption {:?} is not cut by its clip {clip:?}", cap.rect);
    })
}

// ───────────────────── 4 · Esc over the wizard ─────────────────────

/// Esc over the wizard goes through the one Esc owner (`zikaron_ui::layer::esc`): with a sheet opened over the
/// wizard (the identity step's "Import an existing key…"), one Esc closes the sheet only and the wizard stays;
/// the next Esc leaves the wizard (its way out, offered off the true first run) back to the page it was opened
/// from. Driven through the window's own input: the wizard opened with "Run wizard again" on Settings, About.
#[test]
fn one_esc_over_the_wizard_closes_only_the_top_layer() {
    if super::alone_in(module_path!(), "one_esc_over_the_wizard_closes_only_the_top_layer") {
        return;
    }
    english(|| {
        let (w, h) = (1180.0, 1400.0);
        let place = Place::Settings(Section::About);
        // Where the keys are drawn: read from runs of the same walk, each on a fresh context and shell.
        let walk = |steps: Vec<(f64, Vec<Event>)>| {
            let (ctx, frames) = tapped(None);
            let n = steps.len();
            let (_, said) = app::window::probe_shell_input(&ctx, probe_shell(&ctx), place, steps, w, h);
            let frames = frames_of(&frames);
            assert_eq!(frames.len(), SETTLE + n, "one kept frame per frame run");
            (frames, said)
        };
        let (f, _) = walk(idle(1));
        let again = at(f.last().unwrap(), t(Key::DoWizardAgain)).expect("Settings, About offers \"Run wizard again\"");
        let mut open = click(again.center());
        open.extend(idle(6));
        let (f, _) = walk(open.clone());
        let wizard = f.last().unwrap();
        assert!(at(wizard, &app::lang::fill2(Key::WizCount, "2", &Step::ALL.len().to_string())).is_some(), "the wizard opens on the identity step");
        let import = at(wizard, t(Key::IdDoImportExisting)).expect("the identity step offers importing a key");
        let mut steps = open.clone();
        steps.extend(click(import.center()));
        steps.extend(idle(6));
        let sheet_up = steps.len() - 1;
        steps.extend(esc());
        steps.extend(idle(6));
        let one_esc = steps.len() - 1;
        steps.extend(esc());
        steps.extend(idle(6));
        let two_esc = steps.len() - 1;
        let (f, said) = walk(steps);
        let frame = |i: usize| &f[SETTLE + i];
        let shows = |i: usize, k: Key| at(frame(i), t(k)).is_some();
        // The wizard's own count line ("Step 2 of 6") says the wizard is up on the identity step.
        let count = app::lang::fill2(Key::WizCount, "2", &Step::ALL.len().to_string());
        let wizard_up = |i: usize| at(frame(i), &count).is_some();
        let before = said[0].0.clone();
        assert!(shows(sheet_up, Key::IdImportTitle) && wizard_up(sheet_up), "the import sheet is up over the wizard");
        assert!(!shows(one_esc, Key::IdImportTitle), "one Esc closes the sheet");
        assert!(wizard_up(one_esc), "the same Esc leaves the wizard under it standing");
        assert!(!wizard_up(two_esc), "the next Esc leaves the wizard");
        // Back on the very page it was opened from (the settings section, not only its stack).
        assert_eq!(said[two_esc].0, before, "back on the page the wizard was opened from");
    })
}

/// The wizard asks for Esc in the one place every overlay asks (`zikaron_ui::layer::esc`, under the wizard's
/// own key), and the kit's rule holds: of two overlays on screen, one Esc goes to the one opened last only.
#[test]
fn the_wizard_asks_the_one_esc_owner_and_one_esc_goes_to_the_last_opened() {
    if super::alone_in(module_path!(), "the_wizard_asks_the_one_esc_owner_and_one_esc_goes_to_the_last_opened") {
        return;
    }
    let src = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/window/wizard.rs")).expect("wizard.rs reads");
    assert_eq!(src.matches("zikaron_ui::layer::esc(ctx, egui::Id::new(\"zikaron-wizard\"))").count(), 1, "the wizard asks the one Esc owner, once");
    assert!(!src.contains("Key::Escape"), "the wizard reads no Esc key of its own");
    let ctx = egui::Context::default();
    let (under, over) = (egui::Id::new("b10-under"), egui::Id::new("b10-over"));
    let frame = |pressed: bool, over_shown: bool| -> (bool, bool) {
        let events = if pressed { esc().remove(0).1 } else { Vec::new() };
        let mut got = (false, false);
        let _ = ctx.run(egui::RawInput { events, ..Default::default() }, |c| {
            got.0 = zikaron_ui::layer::esc(c, under);
            if over_shown {
                got.1 = zikaron_ui::layer::esc(c, over);
            }
        });
        got
    };
    let _ = frame(false, false);
    let _ = frame(false, true);
    let _ = frame(false, true);
    assert_eq!(frame(true, true), (false, true), "one Esc goes to the overlay opened last only");
    let _ = frame(false, false);
    let _ = frame(false, false);
    assert_eq!(frame(true, false), (true, false), "with it gone, the next Esc goes to the one under it");
}

// ───────────────────── 5 · The handle on a maximized window ─────────────────────

/// A double click on the window's handle puts a maximized window back (`Maximized(false)`), as a title bar
/// does; on a window that is not maximized it maximizes. The windowing layer's report is given on each frame's
/// input (egui's input hook), as the system gives it.
#[test]
fn a_double_click_on_the_handle_of_a_maximized_window_puts_it_back() {
    if super::alone_in(module_path!(), "a_double_click_on_the_handle_of_a_maximized_window_puts_it_back") {
        return;
    }
    let at = pos2(zikaron_ui::tokens::RAIL_W + 200.0, 4.0);
    let press = |p, down| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: down, modifiers: Default::default() };
    let sent = |maximized: bool| -> Vec<String> {
        let (ctx, _) = tapped(Some(maximized));
        let two = vec![(0.1, vec![Event::PointerMoved(at)]), (0.05, vec![press(at, true), press(at, false)]), (0.1, vec![press(at, true), press(at, false)]), (0.5, vec![])];
        let (_, said) = app::window::probe_shell_input(&ctx, probe_shell(&ctx), Place::Home, two, 1180.0, 760.0);
        said.into_iter().flat_map(|(_, c)| c).filter(|c| c.starts_with("Maximized(")).collect()
    };
    assert_eq!(sent(true), vec!["Maximized(false)".to_string()], "a maximized window is put back");
    assert_eq!(sent(false), vec!["Maximized(true)".to_string()], "a window not maximized is maximized");
}

// ───────────────────── 6 · The proxy section: one card, no "now" line ─────────────────────

/// On Settings, Network, the proxy section shows the choice and (for a typed proxy) its address, with no line
/// under it saying how a new connection goes now: no frame draws any of the "now" sentences, before or after a
/// save; "Off" chosen over a typed proxy saves it, and the typed proxy saved again is what the machine keeps.
#[test]
fn the_proxy_section_draws_no_now_line_and_still_saves() {
    if super::alone_in(module_path!(), "the_proxy_section_draws_no_now_line_and_still_saves") {
        return;
    }
    english(|| {
        use app::action::{apply, Action, Applied};
        let (w, h) = (1180.0, 2400.0);
        let place = Place::Settings(Section::Network);
        let proxy = "http://127.0.0.1:8080";
        let head = t(Key::ProxyNowVia).split("{0}").next().unwrap().to_string();
        let now_keys = [Key::ProxyNowDirect, Key::ProxyNowLoopback, Key::ProxyNowUnread, Key::ProxyNowAutoConfig];
        let walk = |steps: Vec<(f64, Vec<Event>)>| {
            let (ctx, frames) = tapped(None);
            let mut shell = probe_shell(&ctx);
            answers!(apply(&mut shell, Action::SetProxy { choice: proxy.into() }), Applied::ProxySet(_), "the typed proxy is saved");
            let n = steps.len();
            let (back, _) = app::window::probe_shell_input(&ctx, shell, place, steps, w, h);
            let frames = frames_of(&frames);
            assert_eq!(frames.len(), SETTLE + n, "one kept frame per frame run");
            (frames, back)
        };
        let now_line = |f: &[Seen]| -> Option<String> { f.iter().map(|(s, _, _)| s).find(|s| s.starts_with(&head) || now_keys.iter().any(|k| s.as_str() == t(*k))).cloned() };
        let (f, _) = walk(idle(1));
        let first = f.last().unwrap();
        assert_eq!(now_line(first), None, "no line says how a new connection goes");
        let off = at(first, t(Key::ProxyOff)).expect("the choice offers \"Off\"");
        let manual = at(first, t(Key::ProxyManual)).expect("the choice offers \"Custom\"");
        let mut lead = click(off.center());
        lead.extend(idle(2));
        let (f, _) = walk(lead.clone());
        assert!(f.iter().all(|fr| now_line(fr).is_none()), "no frame draws a now line");
        assert_eq!(app::machine::read().expect("the machine settings read").proxy.as_deref(), Some(app::machine::proxy::NONE), "\"Off\" is saved");
        lead.extend(click(manual.center()));
        lead.extend(idle(2));
        let (f, _) = walk(lead.clone());
        let save = at(f.last().unwrap(), t(Key::ReadNetSave)).expect("\"Save\" stands once a proxy is to be entered");
        let mut steps = lead;
        steps.extend(click(save.center()));
        steps.extend(idle(2));
        let (f, _) = walk(steps);
        assert!(f.iter().all(|fr| now_line(fr).is_none()), "no frame draws a now line");
        assert_eq!(app::machine::read().expect("the machine settings read").proxy.as_deref(), Some(proxy), "and the typed proxy is what the machine keeps");
    })
}
