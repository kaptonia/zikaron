//! Window-layer checks for the chain reading after a failed read, the nodes left out of a fee reading, and a
//! resend pressed while a receipt check is out. Headless: no window, no wall clock, no network (the probe shell
//! has no home, so no node is configured; the one background task here is started by the test itself).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use app::lang::{fill1, t, Key};
use app::nav::{Place, Step, View};
use app::task::{Done, Kind, Offer, Stuck};
use zikaron_ui::egui;
use super::vault_open;


fn probe_shell(ctx: &egui::Context) -> app::shell::Shell {
    vault_open();
    app::shell::Shell::boot(zikaron_ui::skin::dress(ctx))
}

fn failed_read() -> app::fault::Fault {
    app::fault::Fault::known(app::fault::Known::Unreachable, "http://node.invalid".to_string())
}

fn a_reading(wei: u128) -> Done {
    Done::Chain { gas_wei: Some(wei), sources: 3, single_source: false, unanswered: Vec::new(), head_time: None }
}

/// The words before `{0}` of a sentence, in whichever language the run speaks.
fn head_of(k: Key) -> String {
    t(k).split("{0}").next().unwrap_or("").to_string()
}

/// 7 wei as the window says a held amount (every decimal down to wei).
const SEVEN_WEI: &str = "0.000000000000000007";
/// 1.5 ETH in wei, and as the window says it (short enough for the grants page's narrow column).
const ONE_AND_A_HALF: u128 = 1_500_000_000_000_000_000;

// ───────────────────── Reading what a frame drew ─────────────────────

/// Every text a frame drew, with where.
type Frame = Vec<(String, egui::Rect)>;

/// What the capture shares with the test: each frame's texts, and (for a held task) the text that, once
/// drawn, lets the task go, and the task's word that it has returned.
#[derive(Clone, Default)]
struct Seen {
    frames: Arc<Mutex<Vec<Frame>>>,
    release_on: Option<String>,
    released: Arc<AtomicBool>,
    returned: Arc<AtomicBool>,
}

/// An egui plugin on the probe's own context (egui's public output hook): it reads the shapes each frame
/// handed out. When `release_on` is drawn it lets the held task go, and before the next frame's input it waits
/// for that task to have returned (and its outcome to be sent), so the landing is in the next frame.
struct Capture {
    seen: Seen,
    synced: bool,
}

fn texts_in(s: &egui::Shape, out: &mut Frame) {
    match s {
        egui::Shape::Text(x) => out.push((x.galley.job.text.clone(), x.visual_bounding_rect())),
        egui::Shape::Vec(v) => v.iter().for_each(|x| texts_in(x, out)),
        _ => {}
    }
}

impl egui::Plugin for Capture {
    fn debug_name(&self) -> &'static str {
        "b10-capture"
    }

    fn input_hook(&mut self, _input: &mut egui::RawInput) {
        if self.seen.released.load(Ordering::SeqCst) && !self.synced {
            let from = std::time::Instant::now();
            while !self.seen.returned.load(Ordering::SeqCst) {
                assert!(from.elapsed() < std::time::Duration::from_secs(20), "the held task never returned");
                std::thread::yield_now();
            }
            // The outcome is sent right after the closure returns.
            std::thread::sleep(std::time::Duration::from_millis(50));
            self.synced = true;
        }
    }

    fn output_hook(&mut self, output: &mut egui::FullOutput) {
        let mut frame = Vec::new();
        for c in &output.shapes {
            texts_in(&c.shape, &mut frame);
        }
        if let Some(w) = &self.seen.release_on {
            if frame.iter().any(|(t, _)| t == w) {
                self.seen.released.store(true, Ordering::SeqCst);
            }
        }
        self.seen.frames.lock().unwrap().push(frame);
    }
}

/// A fresh context dressed as the window dresses its own (the faces installed).
fn dressed() -> egui::Context {
    let ctx = egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    ctx
}

fn watched(seen: &Seen) -> egui::Context {
    let ctx = dressed();
    ctx.add_plugin(Capture { seen: seen.clone(), synced: false });
    ctx
}

fn last_frame(seen: &Seen) -> Frame {
    seen.frames.lock().unwrap().last().cloned().unwrap_or_default()
}

fn drawn(f: &Frame, text: &str) -> bool {
    f.iter().any(|(t, _)| t == text)
}

fn drawn_from(f: &Frame, head: &str) -> bool {
    f.iter().any(|(t, _)| t.starts_with(head))
}

fn click(at: egui::Pos2) -> Vec<(f64, Vec<egui::Event>)> {
    let press = |down| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: down, modifiers: Default::default() };
    vec![(0.1, vec![egui::Event::PointerMoved(at)]), (0.1, vec![press(true), press(false)]), (0.1, vec![egui::Event::PointerGone])]
}

fn idle(n: usize) -> Vec<(f64, Vec<egui::Event>)> {
    (0..n).map(|_| (0.1, Vec::new())).collect()
}

const W: f32 = 1180.0;
const H: f32 = 760.0;
const LEDGER: Place = Place::View(View::Log, 0);
const GWEI: u64 = 1_000_000_000;
const TX: &str = "0x00000000000000000000000000000000000000000000000000000000000000b1";

/// A batch the last receipt wait did not see included, offered a resend: it went out at a 10 gwei cap, the
/// price now is `base` gwei, the resend's cap `cap` gwei (100 000 gas each).
fn stuck_at(base: u64, cap: u64, left: Vec<String>) -> Stuck {
    let fees = |max: u64| zikaron_anchor::send::Fees { max_fee: max * GWEI, priority: GWEI, gas_limit: 100_000, from_chain: true };
    Stuck {
        txs: vec![TX.to_string()],
        chain: 31337,
        from: [0x22; 20],
        nonce: 4,
        to: [0x11; 20],
        input: vec![0xab],
        sent: fees(10),
        offer: Offer::Resend { fees: fees(cap), base_now: base * GWEI },
        left,
    }
}

fn eth(s: &str) -> String {
    fill1(Key::SetGasSay, s)
}

/// Where on the ledger page the strip's resend key is, and the resend sheet's own key once that opens it
/// (read from what was drawn, at this size). Two windowless walks over fresh contexts.
fn bump_keys(shell: app::shell::Shell) -> (app::shell::Shell, egui::Pos2, egui::Pos2, Frame) {
    let key = t(Key::U3BumpKey);
    let seen = Seen::default();
    let ctx = watched(&seen);
    let (shell, _) = app::window::probe_shell_input(&ctx, shell, LEDGER, idle(1), W, H);
    let strip = last_frame(&seen).into_iter().find(|(t, _)| t == key).map(|(_, r)| r.center()).expect("the strip offers the resend");
    let seen = Seen::default();
    let ctx = watched(&seen);
    let mut steps = click(strip);
    steps.extend(idle(10));
    let (shell, _) = app::window::probe_shell_input(&ctx, shell, LEDGER, steps, W, H);
    let open = last_frame(&seen);
    assert!(drawn(&open, t(Key::U3BumpTitle)), "the strip's key opens the resend sheet: {open:?}");
    let go = open.iter().filter(|(t, _)| t == key).map(|(_, r)| r.center()).find(|c| c.distance(strip) > 1.0).expect("the sheet's own key");
    (shell, strip, go, open)
}

/// A resend refused by the action layer for this batch (what `Action::BumpFee` says when it was applied).
fn bump_said(shell: &app::shell::Shell) -> Vec<String> {
    use app::fault::Known;
    shell.faults.iter().filter(|f| matches!(f.which(), Some(Known::QueueEmpty | Known::GasNotShown)) && f.raw().contains(TX)).map(|f| f.raw()).collect()
}

// ───────────────────── 1 · The wizard's reading after a failed chain read ─────────────────────

/// After a chain read that failed, the wizard's reading does not count gas as done on the reading before it;
/// once the failure is gone, that reading counts again.
#[test]
fn a_failed_chain_read_leaves_gas_not_done() {
    if super::alone_in(module_path!(), "a_failed_chain_read_leaves_gas_not_done") {
        return;
    }
    let ctx = egui::Context::default();
    let mut shell = probe_shell(&ctx);
    shell.chain = Some(a_reading(7));
    shell.failed.insert(Kind::Chain, failed_read());
    let p = app::nav::Progress::of(&shell);
    assert!(!p.gas && !p.done(Step::Gas), "a failed read leaves gas not done: {p:?}");
    shell.failed.remove(&Kind::Chain);
    let p = app::nav::Progress::of(&shell);
    assert!(p.gas && p.done(Step::Gas), "the reading counts once nothing failed: {p:?}");
}

// ───────────────────── 2 · Said on the wizard's gas step and the grants page too ─────────────────────

/// After a chain read that failed, the wizard's gas step says this read failed (with why), not the balance
/// read before it; once the failure is gone, that balance shows.
#[test]
fn a_failed_chain_read_is_said_on_the_wizards_gas_step() {
    if super::alone_in(module_path!(), "a_failed_chain_read_is_said_on_the_wizards_gas_step") {
        return;
    }
    let ctx = egui::Context::default();
    let mut shell = probe_shell(&ctx);
    // The gas step stands only with a passcode and an identity (`Progress::gate`).
    shell.anchor = Some(app::key::Address([0x22; 20]));
    shell.chain = Some(a_reading(7));
    shell.failed.insert(Kind::Chain, failed_read());
    let failed_head = head_of(Key::SetReadFailedNow);
    let balance = eth(SEVEN_WEI);
    let (mut shell, texts, _) = app::window::probe_wizard_step(&ctx, shell, Place::Home, Step::Gas, W, H);
    let words: Vec<&str> = texts.iter().map(|d| d.text.as_str()).collect();
    assert!(words.contains(&t(Key::WizGasTitle)), "the wizard stands on the gas step: {words:?}");
    assert!(words.iter().any(|w| w.starts_with(&failed_head)), "the failed read is said: {words:?}");
    assert!(!words.contains(&balance.as_str()), "the balance before it is not shown: {words:?}");
    shell.failed.remove(&Kind::Chain);
    let ctx = dressed();
    let (_, texts, _) = app::window::probe_wizard_step(&ctx, shell, Place::Home, Step::Gas, W, H);
    let words: Vec<&str> = texts.iter().map(|d| d.text.as_str()).collect();
    assert!(words.contains(&balance.as_str()), "a landed reading shows: {words:?}");
    assert!(!words.iter().any(|w| w.starts_with(&failed_head)), "nothing failed now: {words:?}");
}

/// After a chain read that failed, the grants page's checks before issuing say this read failed (with why),
/// not the key balance read before it; once the failure is gone, that balance shows.
#[test]
fn a_failed_chain_read_is_said_on_the_grants_page() {
    if super::alone_in(module_path!(), "a_failed_chain_read_is_said_on_the_grants_page") {
        return;
    }
    let place = Place::View(View::Grants, app::nav::tab::GRANTS_NEW);
    let ctx = egui::Context::default();
    let mut shell = probe_shell(&ctx);
    shell.chain = Some(a_reading(ONE_AND_A_HALF));
    shell.failed.insert(Kind::Chain, failed_read());
    let failed_head = head_of(Key::SetReadFailedNow);
    let balance = fill1(Key::U3GasLeft, "1.5000");
    let (mut shell, r) = app::window::probe_face(&ctx, shell, place, W, H);
    assert!(r.texts.iter().any(|t| t == app::lang::t(Key::U3BeforeSigning)), "the checks before issuing are drawn: {:?}", r.texts);
    assert!(r.texts.iter().any(|t| t.starts_with(&failed_head)), "the failed read is said: {:?}", r.texts);
    assert!(!r.texts.iter().any(|t| *t == balance), "the balance before it is not shown: {:?}", r.texts);
    shell.failed.remove(&Kind::Chain);
    let ctx = dressed();
    let (_, r) = app::window::probe_face(&ctx, shell, place, W, H);
    assert!(r.texts.iter().any(|t| *t == balance), "a landed reading shows: {:?}", r.texts);
    assert!(!r.texts.iter().any(|t| t.starts_with(&failed_head)), "nothing failed now: {:?}", r.texts);
}

// ───────────────────── 3 · The nodes left out of a fee reading ─────────────────────

/// The send sheet names the nodes its fees were not read from (they serve another chain), each by name; with
/// none left out, no such line is drawn.
#[test]
fn the_send_sheet_names_the_nodes_left_out_of_the_fee_reading() {
    if super::alone_in(module_path!(), "the_send_sheet_names_the_nodes_left_out_of_the_fee_reading") {
        return;
    }
    // The sentence's fixed part (the nodes are its head): a line of it is one that ends so.
    let left_tail = t(Key::U3FeeLeftNodes).rsplit("{0}").next().unwrap_or("").to_string();
    for (left, want) in [(vec!["http://a.invalid".to_string(), "http://b.invalid".to_string()], true), (Vec::new(), false)] {
        let seen = Seen::default();
        let ctx = watched(&seen);
        let mut shell = probe_shell(&ctx);
        // The batch's estimate is in: the sheet shows its figures (and stays up, nothing landing).
        shell.gas = Some((1, 21_000));
        shell.fees = Some(zikaron_anchor::send::Fees { max_fee: 20 * GWEI, priority: GWEI, gas_limit: 21_000, from_chain: true });
        shell.fees_left = left.clone();
        let mut probe = app::window::SendProbe::pressed(shell, 1);
        for i in 0..8 {
            assert!(probe.frame(&ctx), "frame {i}: the send sheet is up");
        }
        let f = last_frame(&seen);
        assert!(drawn(&f, t(Key::U3SendTitle)), "the send sheet is drawn: {f:?}");
        if want {
            let line = fill1(Key::U3FeeLeftNodes, &left.join(" · "));
            assert!(drawn(&f, &line), "the nodes left out are named: {f:?}");
        } else {
            assert!(!f.iter().any(|(x, _)| x.ends_with(&left_tail)), "none left out, no line: {f:?}");
        }
    }
}

/// The resend sheet names the nodes the price now was not read from (they serve another chain), each by name;
/// with none left out, no such line is drawn.
#[test]
fn the_resend_sheet_names_the_nodes_left_out_of_the_price_reading() {
    if super::alone_in(module_path!(), "the_resend_sheet_names_the_nodes_left_out_of_the_price_reading") {
        return;
    }
    // The sentence's fixed part (the nodes are its head): a line of it is one that ends so.
    let left_tail = t(Key::U3FeeLeftNodes).rsplit("{0}").next().unwrap_or("").to_string();
    for (left, want) in [(vec!["http://c.invalid".to_string(), "http://d.invalid".to_string()], true), (Vec::new(), false)] {
        let ctx = egui::Context::default();
        let mut shell = probe_shell(&ctx);
        shell.stuck = Some(stuck_at(25, 30, left.clone()));
        let (_, _, _, open) = bump_keys(shell);
        assert!(drawn(&open, &eth("0.0030")), "the resend's cap is shown: {open:?}");
        if want {
            let line = fill1(Key::U3FeeLeftNodes, &left.join(" · "));
            assert!(drawn(&open, &line), "the nodes left out are named: {open:?}");
        } else {
            assert!(!open.iter().any(|(x, _)| x.ends_with(&left_tail)), "none left out, no line: {open:?}");
        }
    }
}

// ───────────────────── 4 · A resend pressed while a receipt check is out ─────────────────────

/// What the receipt check out at the press says when it lands.
enum Lands {
    Included,
    Offer(Stuck),
}

/// Open the resend sheet over the ledger page with a receipt check out (an anchoring task the test holds),
/// press the sheet's key, let the check land with `lands`, and read the frames after.
fn press_during_check(lands: Lands) -> (app::shell::Shell, Frame, Vec<String>) {
    let ctx = egui::Context::default();
    let mut shell = probe_shell(&ctx);
    // A ledger handed over still resends (`bump_batch` lets that refusal pass), so an applied resend gets as far
    // as naming this batch: it has no queued entries here, and says so by its transaction.
    shell.handed = Some("0x3333333333333333333333333333333333333333".to_string());
    shell.stuck = Some(stuck_at(25, 30, Vec::new()));
    let (mut shell, strip, go, _) = bump_keys(shell);
    let seen = Seen { release_on: Some(t(Key::U3BumpWaiting).to_string()), ..Default::default() };
    let (released, returned) = (seen.released.clone(), seen.returned.clone());
    let done = match lands {
        Lands::Included => Done::Anchored { tx: TX.to_string(), chain: 31337, confirmed: true, state: "1".to_string(), sent: 1, dropped: 0, queue: Vec::new(), gas: None, calldata: String::new(), stuck: None, voided: false },
        Lands::Offer(s) => Done::Anchored { tx: TX.to_string(), chain: 31337, confirmed: false, state: String::new(), sent: 0, dropped: 0, queue: Vec::new(), gas: None, calldata: String::new(), stuck: Some(s), voided: false },
    };
    let started = shell.tasks.spawn(Kind::Anchor, move || {
        while !released.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        returned.store(true, Ordering::SeqCst);
        Ok(done)
    });
    assert_eq!(started, app::task::Spawned::Started);
    let ctx = watched(&seen);
    let mut steps = click(strip);
    steps.extend(idle(10));
    steps.extend(click(go));
    steps.extend(idle(20));
    let (shell, _) = app::window::probe_shell_input(&ctx, shell, LEDGER, steps, W, H);
    assert!(seen.released.load(Ordering::SeqCst), "the press was held while the check was out (the key said it waits)");
    assert!(!shell.tasks.in_flight(Kind::Anchor), "the check landed");
    let said = bump_said(&shell);
    (shell, last_frame(&seen), said)
}

/// A resend pressed while a receipt check is out, and the batch included meanwhile: the held press is dropped,
/// nothing is sent and no refusal is said, and the sheet closes.
#[test]
fn a_held_resend_is_dropped_when_the_batch_was_included_meanwhile() {
    if super::alone_in(module_path!(), "a_held_resend_is_dropped_when_the_batch_was_included_meanwhile") {
        return;
    }
    let (shell, last, said) = press_during_check(Lands::Included);
    assert!(shell.stuck.is_none(), "included: nothing is offered");
    assert!(said.is_empty(), "no resend was asked, so none is refused: {said:?}");
    assert!(!drawn(&last, t(Key::U3BumpTitle)), "the sheet closed: {last:?}");
}

/// A resend pressed while a receipt check is out, and the check reads an offer above the cap the card showed:
/// nothing is sent, and the card stays up with the new figures.
#[test]
fn a_held_resend_above_the_cap_shown_sends_nothing_and_shows_the_new_figures() {
    if super::alone_in(module_path!(), "a_held_resend_above_the_cap_shown_sends_nothing_and_shows_the_new_figures") {
        return;
    }
    let (shell, last, said) = press_during_check(Lands::Offer(stuck_at(35, 40, Vec::new())));
    assert!(said.is_empty(), "nothing was sent: {said:?}");
    assert!(!shell.tasks.in_flight(Kind::Anchor), "no resend task started");
    assert!(drawn(&last, t(Key::U3BumpTitle)), "the card stays up: {last:?}");
    assert!(drawn(&last, &eth("0.0040")) && drawn(&last, &eth("0.0035")), "with the new cap and price now: {last:?}");
    assert!(!drawn(&last, &eth("0.0030")), "the cap it was pressed at is not shown: {last:?}");
    assert!(drawn(&last, t(Key::U3BumpKey)) && !drawn(&last, t(Key::U3BumpWaiting)), "its key takes a new press: {last:?}");
}

/// A resend pressed while a receipt check is out, and the check reads the same offer: the resend goes once
/// the check ends (the action layer takes it; here it names this batch as having no queued entries), and the
/// sheet closes.
#[test]
fn a_held_resend_goes_when_the_offer_is_unchanged() {
    if super::alone_in(module_path!(), "a_held_resend_goes_when_the_offer_is_unchanged") {
        return;
    }
    let (_, last, said) = press_during_check(Lands::Offer(stuck_at(25, 30, Vec::new())));
    assert_eq!(said.len(), 1, "the resend was applied once: {said:?}");
    assert!(said[0].starts_with("QUEUE_EMPTY"), "taken past the offer and the cap, to this batch's entries: {said:?}");
    assert!(!drawn(&last, t(Key::U3BumpTitle)), "the sheet closed: {last:?}");
}
