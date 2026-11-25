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
    Done::Chain { gas_wei: Some(wei), sources: 3, single_source: false, unanswered: Vec::new(), differed: Vec::new(), head_time: None }
}

fn head_of(k: Key) -> String {
    t(k).split("{0}").next().unwrap_or("").to_string()
}

const SEVEN_WEI: &str = "0.000000000000000007";

type Frame = Vec<(String, egui::Rect)>;

#[derive(Clone, Default)]
struct Seen {
    frames: Arc<Mutex<Vec<Frame>>>,
    release_on: Option<String>,
    released: Arc<AtomicBool>,
    returned: Arc<AtomicBool>,
}

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
        left: left.into_iter().map(zikaron_send::Why::Text).collect(),
        unpriced: None,
    }
}

fn eth(s: &str) -> String {
    fill1(Key::SetGasSay, s)
}

fn ledger_landed(shell: &mut app::shell::Shell) {
    let _ = app::action::apply(shell, app::action::Action::ReadLedger);
    let from = std::time::Instant::now();
    while shell.tasks.in_flight(Kind::Ledger) {
        assert!(from.elapsed() < std::time::Duration::from_secs(60), "the ledger read never landed");
        let _ = shell.drain();
        std::thread::yield_now();
    }
    assert!(shell.rows_current(), "the table is read at this ledger");
}

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

fn bump_said(shell: &app::shell::Shell) -> Vec<String> {
    use app::fault::Known;
    shell.faults.iter().filter(|f| matches!(f.which(), Some(Known::QueueEmpty | Known::GasNotShown)) && f.raw().contains(TX)).map(|f| f.raw()).collect()
}

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
    assert!(!p.own.gas && !p.done(Step::Gas), "a failed read leaves gas not done: {p:?}");
    shell.failed.remove(&Kind::Chain);
    let p = app::nav::Progress::of(&shell);
    assert!(p.own.gas && p.done(Step::Gas), "the reading counts once nothing failed: {p:?}");
}

#[test]
fn a_failed_chain_read_is_said_on_the_wizards_gas_step() {
    if super::alone_in(module_path!(), "a_failed_chain_read_is_said_on_the_wizards_gas_step") {
        return;
    }
    let ctx = egui::Context::default();
    let mut shell = probe_shell(&ctx);
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

#[test]
fn the_send_sheet_names_the_nodes_left_out_of_the_fee_reading() {
    if super::alone_in(module_path!(), "the_send_sheet_names_the_nodes_left_out_of_the_fee_reading") {
        return;
    }
    let left_tail = t(Key::U3FeeLeftNodes).rsplit("{0}").next().unwrap_or("").to_string();
    for (left, want) in [(vec!["http://a.invalid".to_string(), "http://b.invalid".to_string()], true), (Vec::new(), false)] {
        let seen = Seen::default();
        let ctx = watched(&seen);
        let mut shell = probe_shell(&ctx);
        shell.gas = Some((1, 21_000));
        shell.fees = Some(zikaron_anchor::send::Fees { max_fee: 20 * GWEI, priority: GWEI, gas_limit: 21_000, from_chain: true });
        shell.fees_left = left.clone();
        let mut probe = app::window::SendProbe::pressed(shell, 1);
        for i in 0..8 {
            assert!(probe.frame(&ctx), "frame {i}: the send sheet is up");
        }
        let f = last_frame(&seen);
        assert!(drawn(&f, t(Key::U3SendGo)), "the send sheet is drawn: {f:?}");
        if want {
            let line = fill1(Key::U3FeeLeftNodes, &left.join(" · "));
            assert!(drawn(&f, &line), "the nodes left out are named: {f:?}");
        } else {
            assert!(!f.iter().any(|(x, _)| x.ends_with(&left_tail)), "none left out, no line: {f:?}");
        }
    }
}

#[test]
fn the_resend_sheet_names_the_nodes_left_out_of_the_price_reading() {
    if super::alone_in(module_path!(), "the_resend_sheet_names_the_nodes_left_out_of_the_price_reading") {
        return;
    }
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

enum Lands {
    Included,
    Offer(Stuck),
}

struct SetOnDrop(std::sync::Arc<std::sync::atomic::AtomicBool>);

impl Drop for SetOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

thread_local! {
    static ENDED: std::cell::RefCell<Option<SetOnDrop>> = const { std::cell::RefCell::new(None) };
}

fn press_during_check(lands: Lands) -> (app::shell::Shell, Frame, Vec<String>) {
    let ctx = egui::Context::default();
    let mut shell = probe_shell(&ctx);
    let dir = std::env::temp_dir().join(format!("zk-b10-resend-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    super::seated(&mut shell, &dir);
    assert!(matches!(app::action::apply(&mut shell, app::action::Action::OpenHome { root: dir.display().to_string() }), app::action::Applied::Homed { .. }));
    shell.handed = Some("0x3333333333333333333333333333333333333333".to_string());
    ledger_landed(&mut shell);
    shell.stuck = Some(stuck_at(25, 30, Vec::new()));
    let (mut shell, strip, go, _) = bump_keys(shell);
    let seen = Seen { release_on: Some(t(Key::U3BumpWaiting).to_string()), ..Default::default() };
    let (released, returned) = (seen.released.clone(), seen.returned.clone());
    let done = match lands {
        Lands::Included => Done::Anchored { tx: TX.to_string(), chain: 31337, confirmed: true, state: "1".to_string(), sent: 1, dropped: 0, queue: Vec::new(), gas: None, calldata: String::new(), stuck: None, voided: false, waited: 0, kept: None, whole: None },
        Lands::Offer(s) => Done::Anchored { tx: TX.to_string(), chain: 31337, confirmed: false, state: String::new(), sent: 0, dropped: 0, queue: Vec::new(), gas: None, calldata: String::new(), stuck: Some(s), voided: false, waited: 0, kept: None, whole: None },
    };
    let started = shell.tasks.spawn(Kind::Anchor, move || {
        while !released.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        ENDED.with(|e| *e.borrow_mut() = Some(SetOnDrop(returned)));
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

#[test]
fn a_send_refused_says_why_on_the_sheet_it_was_pressed_on() {
    let ctx = egui::Context::default();
    let mut shell = probe_shell(&ctx);
    shell.gas = Some((1, 21_000));
    let f = app::fault::Fault::known(app::fault::Known::InsufficientFunds, "0x00".to_string());
    let why = f.human().to_string();
    let sent = f.clone();
    assert_eq!(shell.tasks.spawn(Kind::Anchor, move || Err(sent)), app::task::Spawned::Started);
    let mut probe = app::window::SendProbe::pressed(shell, 1);
    while !probe.shell().tasks.finished_in_flight(Kind::Anchor) {
        std::thread::yield_now();
    }
    let drawn = (0..5).map(|_| probe.drawn(&ctx)).last().unwrap_or_default();
    assert!(matches!(probe.sheet(), app::window::SendSheet::NotSent { count: 1, why: ref w, .. } if *w == why), "{:?}", probe.sheet());
    assert!(drawn.iter().any(|x| x.contains(&why)) && drawn.iter().any(|x| x == t(Key::LampRefused)), "said on the sheet: {drawn:?}");
    assert!(drawn.iter().any(|x| x == t(Key::U3Retry)), "with a retry: {drawn:?}");
    let mut at_once = app::window::SendProbe::open(probe_shell(&ctx), 1);
    assert!(at_once.press());
    assert!(matches!(at_once.sheet(), app::window::SendSheet::NotSent { count: 1, .. }), "{:?}", at_once.sheet());
}

#[test]
fn every_put_all_on_chain_opens_the_sheet_one_way() {
    let src = |f: &str| std::fs::read_to_string(format!("{}/src/window/{f}", env!("CARGO_MANIFEST_DIR"))).expect("the source");
    let home = src("home.rs");
    assert!(home.contains("self.send_all_unanchored(now);") && !home.contains("U3Confirm::Send {"), "home's card");
    let works = src("works.rs");
    let pending = &works[works.find("fn works_pending(").expect("the pending tab")..];
    let pending = &pending[..pending.find("\n    }\n").expect("its end")];
    assert!(pending.contains("self.send_all_unanchored(now);") && !pending.contains("U3Confirm::Send {"), "the pending tab");
    assert!(src("watch.rs").contains("self.send_all_unanchored(now);"), "the alerts page");
    let ledger = src("ledger.rs");
    assert!(ledger.contains("self.send_all_unanchored(now);"), "the ledger page");
    let one = &ledger[ledger.find("fn send_all_unanchored(").expect("the one way")..];
    assert!(one[..one.find("\n    }\n").expect("its end")].contains("self.shell.sendable_here()"), "the next batch counted");
}

#[test]
fn the_last_batch_is_read_off_the_queue_on_disk() {
    use app::queue::{last_batch, Block, LastBatch, Queued, Stands, Step};
    let (tx1, tx2, tx3) = (format!("0x{}", "01".repeat(32)), format!("0x{}", "02".repeat(32)), format!("0x{}", "03".repeat(32)));
    let id = |n: u8| format!("0x{}", format!("{n:02x}").repeat(32));
    let mut q = app::queue::Queue { chain: 31337, ..Default::default() };
    assert_eq!(last_batch(&q, None), None, "nothing sent");
    q.blocks.push(Block { id: id(1), tx: tx1.clone(), chain: 31337, block: 5 });
    q.blocks.push(Block { id: id(2), tx: tx2.clone(), chain: 31337, block: 9 });
    q.blocks.push(Block { id: id(3), tx: tx2.clone(), chain: 31337, block: 9 });
    assert_eq!(last_batch(&q, None), Some(LastBatch { tx: tx2.clone(), chain: 31337, stands: Stands::Included(2) }), "the highest block's");
    let ended = Done::Anchored { tx: tx1.clone(), chain: 31337, confirmed: true, state: "1".into(), sent: 1, dropped: 1, queue: Vec::new(), gas: None, calldata: String::new(), stuck: None, voided: false, waited: 0, kept: None, whole: None };
    assert_eq!(last_batch(&q, Some(&ended)).map(|b| b.stands), Some(Stands::Ended { confirmed: true, sent: 1, dropped: 1 }), "this run's send");
    q.items.push(Queued { id: id(4), at: 1, step: Step::Submitted { tx: tx3.clone(), chain: 31337, nonce: Some(4) }, chain: 31337 });
    assert_eq!(last_batch(&q, Some(&ended)), Some(LastBatch { tx: tx3.clone(), chain: 31337, stands: Stands::Awaited(1) }), "the one out");
    q.items.clear();
    let ctx = egui::Context::default();
    let mut shell = probe_shell(&ctx);
    shell.queue = q;
    let (_, r) = app::window::probe_face(&ctx, shell, Place::Home, 1180.0, 760.0);
    assert!(r.texts.iter().any(|x| x == t(Key::U3LastSend)), "the last batch line: {:?}", r.texts);
    assert!(r.texts.iter().any(|x| x.starts_with("31337 \u{b7} 0x02")), "its transaction: {:?}", r.texts);
}

#[test]
fn each_export_starts_plain() {
    let ctx = dressed();
    let made = Done::Kit {
        root: std::path::PathBuf::new(),
        publish: None,
        path: "kit.zkit".into(),
        kit_id: String::new(),
        entries: 1,
        files: 0,
        pulled: Vec::new(),
        dropped: Vec::new(),
        originals: Vec::new(),
        left_out: Vec::new(),
        unreadable: Vec::new(),
        withdrawn: Vec::new(),
        facts: app::kitsindex::RowFacts { one_time: Some("0x01".into()), ..Default::default() },
    };
    let mut probe = app::window::KitProbe::new(probe_shell(&ctx), true);
    let _ = probe.shell().tasks.spawn(Kind::Kit, move || Ok(made));
    assert!(!probe.lands(&ctx), "made: the next export starts plain");
    let mut probe = app::window::KitProbe::new(probe_shell(&ctx), true);
    let _ = probe.shell().tasks.spawn(Kind::Kit, || Err(failed_read()));
    assert!(probe.lands(&ctx), "refused: the switch as it was set");
}

#[test]
fn the_last_batch_is_drawn_where_a_batch_is_sent_from() {
    use app::queue::Block;
    let tx = format!("0x{}", "02".repeat(32));
    let mut q = app::queue::Queue { chain: 31337, ..Default::default() };
    q.blocks.push(Block { id: format!("0x{}", "01".repeat(32)), tx: tx.clone(), chain: 31337, block: 9 });
    let ctx = egui::Context::default();
    for (place, grantee) in [(Place::Home, true), (Place::View(app::nav::View::Log, 0), false), (Place::View(app::nav::View::Alerts, 0), false)] {
        let mut shell = probe_shell(&ctx);
        shell.queue = q.clone();
        if grantee {
            shell.settings.role = app::roles::Role::Grantee;
        }
        let (_, r) = app::window::probe_face(&ctx, shell, place, 1180.0, 1600.0);
        assert!(r.texts.iter().any(|x| x == t(Key::U3LastSend)), "{place:?}: the last batch line: {:?}", r.texts);
        assert!(r.texts.iter().any(|x| x.starts_with("31337 \u{b7} 0x02")), "{place:?}: its transaction: {:?}", r.texts);
    }
}
