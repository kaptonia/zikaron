use super::a8_send::{bench, chain, node, tidy, PLENTY};
use app::action::{apply, Action, Applied};
use app::fault::{Fault, Known};
use app::task::{Done, Kind};
use std::time::{Duration, Instant};
use zikaron::json::Value;

fn audit_lands(shell: &mut app::shell::Shell, result: Result<Done, Fault>, now: f64) {
    let _ = shell.tasks.spawn(Kind::Audit, move || result);
    let until = Instant::now() + Duration::from_secs(60);
    while !shell.drain_at(now).iter().any(|o| o.kind == Kind::Audit) {
        assert!(Instant::now() < until, "the audit landed");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn report(unanchored: &[String], complete: bool) -> Done {
    let rows = |ids: &[String]| Value::Arr(ids.iter().map(|i| Value::Str(i.clone())).collect());
    let report = Value::Obj(vec![
        ("anchored".into(), Value::Arr(Vec::new())),
        ("missing".into(), Value::Arr(Vec::new())),
        (zikaron::v2::tokens::Key::Unanchored.as_str().into(), rows(unanchored)),
    ]);
    let label = if complete { zikaron::v2::tokens::Label::Complete } else { zikaron::v2::tokens::Label::Gaps }.as_str().to_string();
    Done::Audited { label, complete, broken: false, entries: 2, report, unanswered: Vec::new(), unproven: Vec::new(), asked: 1, single_source: false, fragment: Value::Null, recounted: false }
}

#[test]
fn a_report_behind_the_receipts_is_asked_again_a_pause_after_each_try() {
    if super::alone_in(module_path!(), "a_report_behind_the_receipts_is_asked_again_a_pause_after_each_try") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30fc-r5", &[&n]);
    let id = shell.next_batch_here(1).first().cloned().expect("a queued entry");
    let pause = app::shell::AUDIT_LAG_AGAIN;
    audit_lands(&mut shell, Ok(report(std::slice::from_ref(&id), true)), 10.0);
    assert_eq!(shell.audit_lag_at(), None, "no receipt for it yet: nothing to ask again");
    shell.queue.blocks.push(app::queue::Block { id: id.clone(), tx: format!("0x{}", "ab".repeat(32)), chain: 31337, block: 0x30 });
    assert_eq!(shell.audit_lag_at(), Some(10.0 + pause), "a pause after the report landed");
    assert!(!shell.audit_lags(11.0) && shell.audit_lags(10.0 + pause));
    audit_lands(&mut shell, Err(Fault::known(Known::Unreachable, "node".to_string())), 10.0 + pause);
    assert!(shell.audit.is_some(), "the report in hand stays");
    assert_eq!(shell.audit_lag_at(), Some(10.0 + 2.0 * pause), "timed from the try");
    assert!(!shell.audit_lags(10.0 + pause + 0.5), "not at the frame the failure brings");
    assert!(shell.audit_lags(10.0 + 2.0 * pause), "asked again a pause later");
    audit_lands(&mut shell, Ok(report(&[], true)), 50.0);
    assert_eq!(shell.audit_lag_at(), None);
    tidy();
}

#[test]
fn every_change_of_the_node_table_reads_the_chain_again() {
    if super::alone_in(module_path!(), "every_change_of_the_node_table_reads_the_chain_again") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30fc-r6", &[&n]);
    shell.commit_settings(|s| {
        s.endpoints = vec![format!("31337={}", n.url)];
        s.rest = vec!["https://127.0.0.1:9/api".into()];
    })
    .expect("saved");
    for on in [true, false] {
        assert!(matches!(apply(&mut shell, Action::SetRestMode { on }), Applied::RestMode(x) if x == on), "switched");
        assert!(shell.chain.is_none(), "the reading over the table before is gone");
        assert!(shell.tasks.in_flight(Kind::Chain), "read again at once (REST rows {on})");
        let until = Instant::now() + Duration::from_secs(60);
        while !shell.drain().iter().any(|o| o.kind == Kind::Chain) {
            assert!(Instant::now() < until, "the reading landed");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!matches!(shell.chain_reading(), Ok(None)), "read over the table as it stands: {:?}", shell.chain_reading());
    }
    let _ = apply(&mut shell, Action::SetRestMode { on: true });
    assert!(shell.endpoints.iter().any(|e| e.chain == 31337 && e.url.kind() == zikaron_send::node::Kind::Rest), "the row serves the basis chain");
    let moved = apply(&mut shell, Action::SetBasis { chain: "31338".into(), registry: format!("0x{}", "11".repeat(20)), from_block: "0".into() });
    assert!(!matches!(moved, Applied::Trouble(_)), "{moved:?}");
    assert!(shell.endpoints.iter().any(|e| e.chain == 31338 && e.url.kind() == zikaron_send::node::Kind::Rest), "the moved basis's table is the shell's at once: {:?}", shell.endpoints);
    assert!(shell.chain.is_none(), "the reading over the table before is gone");
    tidy();
}

#[test]
fn the_wizard_opens_by_itself_only_for_a_step_that_cannot_wait() {
    use app::nav::Progress;
    use app::roles::Role;
    let all = Progress { key: true, pin: true, own: app::own::OwnProgress { network: true, gas: true, genesis: true }, backup: true };
    for role in [Role::Author, Role::Grantee] {
        assert!(!all.wants_wizard(role), "everything done");
        assert!(Progress { pin: false, ..all }.wants_wizard(role), "no passcode: {role:?}");
        assert!(Progress { key: false, ..all }.wants_wizard(role), "no identity: {role:?}");
        assert!(!Progress { own: app::own::OwnProgress { genesis: false, gas: false, ..all.own }, backup: false, ..all }.wants_wizard(role), "only steps that can wait: {role:?}");
        assert!(!Progress { own: app::own::OwnProgress { network: false, ..all.own }, ..all }.wants_wizard(role), "the network as before: no opening by itself: {role:?}");
    }
}

#[test]
fn every_sheet_the_window_draws_is_counted_as_up() {
    let ctx = zikaron_ui::egui::Context::default();
    super::vault_open();
    let shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let (_, each) = app::window::probe_each_sheet_is_one(shell);
    assert_eq!(each.len(), 5);
    for (name, up, back) in each {
        assert!(up, "{name} is a sheet up");
        assert!(!back, "the key change does not come back over {name}");
    }
}

#[test]
fn a_tail_check_that_writes_nothing_does_not_ask_the_audit_again() {
    if super::alone_in(module_path!(), "a_tail_check_that_writes_nothing_does_not_ask_the_audit_again") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30fc-278", &[&n]);
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf()).expect("a home");
    let home = app::home::Home::bare(root.clone());
    let marked = app::restorex::State::NewerElsewhere { missing: 1, lacks: Vec::new() };
    audit_lands(&mut shell, Ok(report(&[], true)), 10.0);
    app::restorex::write(&home, &marked).expect("marked");
    shell.unfetched = Some(marked.clone());
    shell.audit_asked = Some(shell.book_mark);
    shell.tail_asked = Some(shell.book_mark);
    assert!(!shell.audit_stale(), "the report answers for this ledger");
    let elsewhere = app::restorex::Tail::NewerElsewhere { missing: 1, anchors: 3, lacks: vec!["ab".repeat(32)] };
    let at = root.clone();
    let _ = shell.tasks.spawn(Kind::Fetch, move || Ok(Done::TailChecked { checked: vec![(at, Ok(elsewhere), None)], unwritten: false }));
    let until = Instant::now() + Duration::from_secs(60);
    while !shell.drain_at(11.0).iter().any(|o| o.kind == Kind::Fetch) {
        assert!(Instant::now() < until, "the tail landed");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(matches!(shell.unfetched, Some(app::restorex::State::NewerElsewhere { missing: 1, .. })), "the mark stays: {:?}", shell.unfetched);
    assert!(!shell.audit_stale(), "a tail check that wrote nothing does not make the self-audit due again");
    assert!(!shell.tail_due(), "nor the tail, at this ledger");
    tidy();
}

#[test]
fn what_a_write_asks_is_what_is_said_and_a_send_asks_the_lock_too() {
    if super::alone_in(module_path!(), "what_a_write_asks_is_what_is_said_and_a_send_asks_the_lock_too") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut w = bench("r30fc-gates", &[&n]);
    assert!(w.pen_here() && w.may_send_entries().is_ok());
    w.office_trouble = Some(Fault::known(Known::OfficeNotInWords, String::new()));
    assert!(!w.pen_here(), "the header asks the key in office as every write does");
    w.office_trouble = None;
    let to = format!("0x{}", "77".repeat(20));
    w.handed = Some(to.clone());
    assert!(w.may_send_entries().is_ok(), "a ledger handed over still sends its succession");
    assert!(matches!(w.may_write_entries(), Err(f) if f.which() == Some(Known::HandedOver)), "and takes no entry");
    assert!(w.may_save_settings().is_ok(), "settings answer to their own writer: a ledger handed over is read only as a ledger");
    let was = w.reconciled.clone();
    w.reconciled = Some((zikaron::v2::tokens::Label::BrokenChain.as_str().to_string(), Default::default(), Default::default()));
    assert!(w.broken() && !w.writable() && w.may_save_settings().is_ok(), "a broken ledger is read only as a ledger and its nodes can still be fixed");
    w.reconciled = was;
    let root = w.home.as_ref().map(|h| h.root().display().to_string()).expect("a home");
    let ctx = zikaron_ui::egui::Context::default();
    let mut r = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    assert!(matches!(apply(&mut r, Action::OpenHome { root }), Applied::Homed { mode: app::lock::Mode::Reader, .. }));
    r.handed = Some(to);
    assert!(matches!(r.may_send_entries(), Err(f) if f.which() == Some(Known::ReadOnly)), "the reader sends nothing beside the writer");
    drop(r);
    tidy();
}

#[test]
fn a_report_stale_only_for_a_write_here_is_worked_out_again_without_the_chain() {
    if super::alone_in(module_path!(), "a_report_stale_only_for_a_write_here_is_worked_out_again_without_the_chain") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30fc-recount", &[&n]);
    let lands = |shell: &mut app::shell::Shell, now: f64| {
        let until = Instant::now() + Duration::from_secs(60);
        while !shell.drain_at(now).iter().any(|o| o.kind == Kind::Audit) {
            assert!(Instant::now() < until, "the audit landed");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    assert_eq!(apply(&mut shell, Action::Audit), Applied::Started(Kind::Audit));
    lands(&mut shell, 10.0);
    let read = shell.audit.clone().expect("a report read from the chain");
    assert!(read.fragment.member("anchors").is_some(), "{:?}", read.fragment);
    let questions = |n: &super::a8_send::Node| ["eth_chainId", "eth_blockNumber", "eth_getLogs", "eth_getBlockByNumber"].iter().map(|m| n.asked(m).len()).sum::<usize>();
    let asked = questions(&n);
    shell.book_mark += 1;
    assert!(shell.audit_stale());
    assert!(app::action::recount_audit(&mut shell), "worked out again here");
    lands(&mut shell, 20.0);
    let again = shell.audit.clone().expect("the report");
    assert_eq!(questions(&n), asked, "the nodes asked nothing");
    assert_eq!((again.mark, again.at, again.label.clone()), (shell.book_mark, read.at, read.label.clone()), "this ledger's, the chain reading's time kept");
    assert!(!shell.audit_stale());
    shell.book_mark += 1;
    shell.endpoints.push(app::chainx::Endpoint::at(31337, "http://127.0.0.1:9"));
    assert!(!app::action::recount_audit(&mut shell), "another node table: the chain's to read");
    tidy();
}

#[test]
fn a_confirmed_batch_goes_into_the_report_from_its_own_facts_without_the_chain() {
    if super::alone_in(module_path!(), "a_confirmed_batch_goes_into_the_report_from_its_own_facts_without_the_chain") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30fc-merge", &[&n]);
    let lands = |shell: &mut app::shell::Shell, now: f64| {
        let until = Instant::now() + Duration::from_secs(60);
        while !shell.drain_at(now).iter().any(|o| o.kind == Kind::Audit) {
            assert!(Instant::now() < until, "the audit landed");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    assert_eq!(apply(&mut shell, Action::Audit), Applied::Started(Kind::Audit));
    lands(&mut shell, 10.0);
    let read = shell.audit.clone().expect("a report read from the chain");
    let ids: Vec<String> = shell.queue.items.iter().map(|q| q.id.to_ascii_lowercase()).collect();
    assert!(!ids.is_empty());
    let unanchored = |a: &app::shell::AuditRead| app::auditx::entry_ids(&a.report, zikaron::v2::tokens::Key::Unanchored);
    assert!(ids.iter().all(|id| unanchored(&read).contains(id)), "before the send the batch is unanchored");
    let questions = |n: &super::a8_send::Node| ["eth_chainId", "eth_blockNumber", "eth_getLogs", "eth_getBlockByNumber"].iter().map(|m| n.asked(m).len()).sum::<usize>();
    let asked = questions(&n);
    let me = shell.anchor.expect("the sending key").0;
    let tx = [0x44u8; 32];
    let recs: Vec<zikaron_anchor::scan::AnchorRec> = ids
        .iter()
        .map(|id| {
            let hash: [u8; 32] = zikaron::hexfmt::decode(id).and_then(|b| b.try_into().ok()).expect("an entry id");
            zikaron_anchor::scan::AnchorRec { chain_id: 31337, block_number: 0x41, block_timestamp: 0x65, tx, sender: me, hash, verdict: zikaron::tokens::Verdict::Counted }
        })
        .collect();
    let landed_read = zikaron_anchor::scan::fragment(&zikaron_anchor::scan::Scanned { anchors: recs, basis: Value::Null, candidates: Vec::new() });
    let kept = zikaron_send::batch::Kept { anchors: landed_read.member("anchors").and_then(Value::as_arr).cloned().unwrap_or_default(), single_source: false };
    let landed = |tx: [u8; 32], kept: Option<zikaron_send::batch::Kept>| Done::Anchored {
        tx: zikaron::hexfmt::encode(&tx),
        chain: 31337,
        confirmed: true,
        state: "status 1 · block 65".into(),
        sent: ids.len(),
        dropped: 0,
        queue: Vec::new(),
        gas: None,
        calldata: String::new(),
        stuck: None,
        voided: false,
        waited: 0,
        kept,
        whole: None,
    };
    shell.book_mark += 1;
    app::action::audit_after_send(&mut shell, &landed(tx, Some(kept.clone())));
    lands(&mut shell, 20.0);
    let merged = shell.audit.clone().expect("the report");
    assert_eq!(questions(&n), asked, "the nodes asked nothing");
    assert!(ids.iter().all(|id| !unanchored(&merged).contains(id)), "the batch is anchored in the report: {:?}", merged.report);
    let to = merged.fragment.member("basis").and_then(|b| b.member("chains")).and_then(|c| c.as_arr()).and_then(|c| c.first()).and_then(|c| c.member("toBlock")).cloned();
    assert_eq!(to, Some(Value::Int(0x41)), "the window reaches the batch's block");
    assert_eq!((merged.mark, merged.at), (shell.book_mark, 20.0), "this ledger's, last checked at the receipt's reading");
    assert!(read.at < merged.at, "last checked moved on from the chain read before the send");
    shell.book_mark += 1;
    app::action::audit_after_send(&mut shell, &landed([0x45; 32], None));
    lands(&mut shell, 30.0);
    assert!(questions(&n) > asked, "read from the chain");
    tidy();
}

#[test]
fn a_change_behind_only_for_a_write_here_is_refused_and_worked_out_again_without_the_chain() {
    if super::alone_in(module_path!(), "a_change_behind_only_for_a_write_here_is_refused_and_worked_out_again_without_the_chain") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30fc-behind", &[&n]);
    let lands = |shell: &mut app::shell::Shell, now: f64| {
        let until = Instant::now() + Duration::from_secs(60);
        while !shell.drain_at(now).iter().any(|o| o.kind == Kind::Audit) {
            assert!(Instant::now() < until, "the audit landed");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    assert_eq!(apply(&mut shell, Action::Audit), Applied::Started(Kind::Audit));
    lands(&mut shell, 10.0);
    let questions = |n: &super::a8_send::Node| ["eth_chainId", "eth_blockNumber", "eth_getLogs", "eth_getBlockByNumber"].iter().map(|m| n.asked(m).len()).sum::<usize>();
    let asked = questions(&n);
    let _ = app::action::apply_settled(&mut shell, Action::Annotate { subject: String::new(), note_md: "after the audit".into() });
    assert!(shell.audit_stale(), "written since");
    let begin = || Action::Rotate { step: app::action::RotateStep::Begin { source: app::rotatex::Source::SameWords, answers: Vec::new() } };
    match apply(&mut shell, begin()) {
        Applied::Trouble(f) => assert_eq!(f.which(), Some(Known::AuditBehind), "{f:?}"),
        other => panic!("refused as behind: {other:?}"),
    }
    assert!(shell.tasks.in_flight(Kind::Audit), "a self-audit started");
    lands(&mut shell, 20.0);
    assert_eq!(questions(&n), asked, "the nodes asked nothing");
    assert!(!shell.audit_stale() && shell.audit_report_in_step().is_some(), "the report in step");
    match apply(&mut shell, begin()) {
        Applied::Trouble(f) => assert_ne!(f.which(), Some(Known::AuditBehind), "not behind again: {f:?}"),
        _ => {}
    }
    tidy();
}

#[test]
fn a_batch_on_an_earlier_chain_is_offered_again_to_that_chains_registry() {
    if super::alone_in(module_path!(), "a_batch_on_an_earlier_chain_is_offered_again_to_that_chains_registry") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let main = node(|m, b| if m == "eth_chainId" { super::a8_send::Reply::Result("\"0x7a6a\"".into()) } else { chain(m, b, PLENTY) });
    let mut shell = bench("r31fc-earlier", &[&n]);
    shell.anchor_wait = Duration::from_millis(300);
    let lands = |shell: &mut app::shell::Shell| {
        let until = Instant::now() + Duration::from_secs(60);
        while shell.tasks.in_flight(Kind::Anchor) {
            assert!(Instant::now() < until, "the wait landed");
            let _ = shell.drain();
            std::thread::sleep(Duration::from_millis(5));
        }
        let _ = shell.drain();
    };
    assert!(super::a8_send::send(&mut shell).is_ok(), "the batch went out");
    lands(&mut shell);
    shell.settings.chain_id = Some(31338);
    shell.settings.registry = Some(app::key::Address([0x44; 20]));
    shell.settings.earlier = vec![zikaron_seal::ground::Earlier { chain_id: 31337, registry: [0x11; 20], from_block: 0 }];
    shell.endpoints.push(app::chainx::Endpoint::at(31338, main.url.clone()));
    shell.stuck = None;
    assert!(app::action::resume(&mut shell), "the wait resumed");
    lands(&mut shell);
    let stuck = shell.stuck.clone().expect("the batch no node holds is offered again");
    assert!(matches!(stuck.offer, app::task::Offer::Unheld { .. }), "{:?}", stuck.offer);
    assert_eq!((stuck.chain, stuck.to), (31337, [0x11; 20]), "offered to the registry of the batch's own chain");
    tidy();
}
