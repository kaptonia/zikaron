use super::a8_send::{bench, chain, first_param, node, tidy, Node, Reply, PLENTY};
use app::queue::Kept as _;
use app::action::{apply, apply_settled, Action, Applied};
use app::fault::Known;
use app::task::{Done, Kind};
use std::time::{Duration, Instant};

fn landed(shell: &mut app::shell::Shell, kind: Kind) -> Option<Result<Done, app::fault::Fault>> {
    let until = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(o) = shell.drain().into_iter().find(|o| o.kind == kind) {
            return Some(o.result);
        }
        if Instant::now() >= until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn other_chain() -> Node {
    node(|m, b| if m == "eth_chainId" { Reply::Result("\"0x1\"".into()) } else { chain(m, b, PLENTY) })
}

fn dead() -> String {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    drop(l);
    url
}

#[test]
fn a_table_with_no_node_of_the_chain_is_refused_at_the_estimate() {
    if super::alone_in(module_path!(), "a_table_with_no_node_of_the_chain_is_refused_at_the_estimate") {
        return;
    }
    let live = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30b1-a4", &[&live]);
    let other = other_chain();
    let gone = dead();
    let mut estimate = |eps: Vec<app::chainx::Endpoint>| -> app::fault::Fault {
        app::chainx::forget_serving();
        shell.endpoints = eps;
        match apply_settled(&mut shell, Action::EstimateGas { count: 1 }) {
            Applied::Trouble(f) => f,
            other => panic!("refused at the estimate: {other:?}"),
        }
    };
    let at = |u: &str| app::chainx::Endpoint::at(31337, app::chainx::NodeAddr::new(u));
    let none = estimate(vec![app::chainx::Endpoint::at(1, app::chainx::NodeAddr::new(&other.url))]);
    assert_eq!(none.which(), Some(Known::NoEndpoint), "{none:?}");
    let alone = estimate(vec![at(&gone)]);
    let beside = estimate(vec![at(&gone), at(&other.url)]);
    assert_eq!(alone.which(), beside.which(), "two forms, one answer: {alone:?} / {beside:?}");
    assert!(beside.tail().contains(&other.url), "the other chain's node is named: {}", beside.tail());
    let wrong = estimate(vec![at(&other.url)]);
    assert_eq!(wrong.which(), Some(Known::WrongChain), "{wrong:?}");
    assert!(other.asked("eth_estimateGas").is_empty() && other.asked("eth_sendRawTransaction").is_empty(), "the other chain's node never estimates or takes a send");
    let poor = node(|m, b| chain(m, b, "\"0x0\""));
    let mut shell = bench("r30b1-a4-poor", &[&poor]);
    let id = shell.next_batch_here(1).first().cloned().expect("a queued entry");
    let _ = super::a8_send::send(&mut shell);
    let disk = app::queue::Queue::read(shell.home.as_ref().expect("a home")).expect("the queue reads");
    let code = app::ledgerx::refusal_of(&id, &disk).map(str::to_string);
    assert_eq!(code.as_deref(), Some(Known::InsufficientFunds.as_str()), "the refusal's code kept with the entry");
    assert_eq!(code.as_deref().and_then(Known::of_str).map(Known::what), Some(Known::InsufficientFunds.what()), "said in the table's words");
    assert_eq!(app::ledgerx::refusal_of("0x00", &disk), None, "an entry not refused says nothing");
    tidy();
}

#[test]
fn a_receipt_this_home_holds_is_on_chain_while_the_node_lags() {
    if super::alone_in(module_path!(), "a_receipt_this_home_holds_is_on_chain_while_the_node_lags") {
        return;
    }
    let n = node(|m, b| match m {
        "eth_getTransactionReceipt" => Reply::Result("{\"status\":\"0x1\",\"blockNumber\":\"0x30\"}".into()),
        "eth_getTransactionByHash" => Reply::Result(format!("{{\"hash\":\"{}\",\"blockNumber\":\"0x30\",\"from\":\"0x{}\",\"input\":\"0x\"}}", first_param(b), "11".repeat(20))),
        _ => chain(m, b, PLENTY),
    });
    let mut shell = bench("r30b1-a2", &[&n]);
    let _ = apply(&mut shell, Action::Audit);
    let _ = landed(&mut shell, Kind::Audit);
    assert!(shell.audit_in_step(), "the audit is in step with the ledger it read");
    let id = shell.next_batch_here(1).first().cloned().expect("a queued entry");
    let sent = super::a8_send::send(&mut shell);
    assert!(matches!(sent, Ok(Done::Submitted { .. })), "{sent:?}");
    let got = landed(&mut shell, Kind::Anchor).expect("the receipt landed");
    assert!(matches!(got, Ok(Done::Anchored { confirmed: true, .. })), "{got:?}");
    let on = shell.on_chain();
    assert!(on.anywhere(&id) && on.on(31337, &id), "on chain by this home's receipt");
    assert!(shell.audit_in_step(), "a batch landing does not make the audit behind");
    if shell.tasks.in_flight(Kind::Audit) {
        let _ = landed(&mut shell, Kind::Audit);
    }
    let on = shell.on_chain();
    let report = shell.audit.as_ref().map(|a| a.report.clone());
    let queued: Vec<String> = shell.queue.items.iter().map(|q| q.id.clone()).collect();
    let with = app::rotatex::owed(report.as_ref(), &app::rotatex::Owing { on: &on, queued: &queued, own: None }, &Default::default());
    let without = app::rotatex::owed(report.as_ref(), &app::rotatex::Owing { on: &zikaron_send::batch::OnChain::default(), queued: &queued, own: None }, &Default::default());
    assert_eq!(with + 1, without, "the received entry is not owed: {with} / {without}");
    let at = shell.audit.as_ref().map(|a| a.at).unwrap_or_default();
    assert!(!shell.audit_lags(at + 1.0), "not before the pause");
    assert!(shell.audit_lags(at + app::shell::AUDIT_LAG_AGAIN), "the report finds the received entry unanchored: asked again");
    let probe = super::a8_send::homes().join("r30b1-a2").join("more.txt");
    std::fs::write(&probe, b"more").expect("a file");
    let _ = apply_settled(&mut shell, Action::TakeContent { source: app::anchorx::Source::File, path: probe.display().to_string() });
    let _ = apply_settled(&mut shell, Action::RecordWork { note_md: String::new(), files: Vec::new(), for_: None });
    let _ = apply(&mut shell, Action::ReadLedger);
    let _ = landed(&mut shell, Kind::Ledger);
    assert!(!shell.audit_in_step(), "an entry the report never read");
    let newest = shell.rows.as_ref().and_then(|(r, _)| r.first()).map(|r| r.id.clone()).expect("the newest row");
    shell.queue.blocks.push(app::queue::Block { id: newest, tx: id.clone(), chain: 31337, block: 0x31 });
    assert!(shell.audit_in_step(), "every entry the report did not read has this home's receipt");
    tidy();
}

#[test]
fn a_marked_homes_tail_is_asked_again_when_its_ledger_changes() {
    if super::alone_in(module_path!(), "a_marked_homes_tail_is_asked_again_when_its_ledger_changes") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30b1-a1", &[&n]);
    let spec = format!("31337={}", n.url);
    shell.commit_settings(|s| {
        s.chain_id = Some(31337);
        s.registry = Some(app::key::Address([0x11; 20]));
        s.endpoints = vec![spec.clone()];
    }).expect("saved");
    let home = app::home::Home::bare(shell.home.as_ref().map(|h| h.root().to_path_buf()).expect("a home"));
    let marked = app::restorex::State::NewerElsewhere { missing: 1, lacks: Vec::new() };
    app::restorex::write(&home, &marked).expect("marked");
    shell.unfetched = Some(marked.clone());
    shell.seat_identities(app::register::view().ok());
    shell.tail_asked = Some(shell.book_mark);
    let _ = shell.drain_at(1.0);
    assert!(!shell.tasks.in_flight(Kind::Fetch), "nothing changed, nothing asked");
    let look_lands = |shell: &mut app::shell::Shell, now: f64| {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
        while shell.tasks.in_flight(Kind::Look) && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(1));
            let _ = shell.drain_at(now);
        }
    };
    look_lands(&mut shell, 1.0);
    shell.seen_entries = Some(0);
    let _ = shell.drain_at(100.0);
    look_lands(&mut shell, 100.0);
    assert!(shell.tasks.in_flight(Kind::Fetch) && shell.fetch_checks_tail, "the tail is asked again");
    let got = landed(&mut shell, Kind::Fetch).expect("the tail landed");
    assert!(matches!(got, Ok(Done::TailChecked { .. })), "{got:?}");
    assert_eq!(shell.unfetched, None, "passed: the mark is lifted");
    assert_eq!(app::restorex::read(&home).ok().flatten(), None, "and off the disk");
    fn audit_lands(shell: &mut app::shell::Shell, home: &app::home::Home, marked: app::restorex::State, label: zikaron::v2::tokens::Label, now: f64) -> bool {
        app::restorex::write(home, &marked).expect("marked again");
        shell.unfetched = Some(marked.clone());
        shell.tail_asked = Some(shell.book_mark);
        let complete = label == zikaron::v2::tokens::Label::Complete;
        let report = zikaron::json::Value::Obj(vec![("anchored".into(), zikaron::json::Value::Arr(Vec::new())), ("missing".into(), zikaron::json::Value::Arr(Vec::new()))]);
        let label = label.as_str().to_string();
        let _ = shell.tasks.spawn(Kind::Audit, move || Ok(Done::Audited { label, complete, broken: false, entries: 2, report, unanswered: Vec::new(), unproven: Vec::new(), asked: 1, single_source: false, fragment: zikaron::json::Value::Null, recounted: false }));
        let until = Instant::now() + Duration::from_secs(60);
        loop {
            if shell.drain_at(now).iter().any(|o| o.kind == Kind::Audit) {
                let asked = shell.tasks.in_flight(Kind::Fetch);
                let _ = landed(shell, Kind::Fetch);
                return asked;
            }
            assert!(Instant::now() < until, "the audit landed");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    assert!(audit_lands(&mut shell, &home, marked.clone(), zikaron::v2::tokens::Label::Complete, 200.0), "complete, nothing missing: asked again");
    assert_eq!(shell.unfetched, None, "and the pass lifts the mark");
    assert!(!audit_lands(&mut shell, &home, marked, zikaron::v2::tokens::Label::Gaps, 300.0), "with gaps: not asked");
    tidy();
}

#[test]
fn the_chain_is_read_again_when_the_node_table_changes() {
    if super::alone_in(module_path!(), "the_chain_is_read_again_when_the_node_table_changes") {
        return;
    }
    let a = node(|m, b| chain(m, b, PLENTY));
    let b = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30b1-c16", &[&a]);
    let specs = |ns: &[&Node]| ns.iter().map(|n| format!("31337={}", n.url)).collect::<Vec<_>>().join(" ");
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: specs(&[&a, &b]) }), Applied::Endpoints(2)));
    assert!(shell.chain.is_none(), "the reading over the table before is gone");
    let got = landed(&mut shell, Kind::Chain).expect("read again");
    assert!(matches!(got, Ok(Done::Chain { sources: 2, .. })), "both nodes now: {got:?}");
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: specs(&[&a]) }), Applied::Endpoints(1)));
    assert!(shell.chain.is_none());
    let got = landed(&mut shell, Kind::Chain).expect("read again");
    assert!(matches!(got, Ok(Done::Chain { sources: 1, .. })), "one node now, never \"2 agree\": {got:?}");
    assert_eq!(shell.tasks.started_over(Kind::Chain), None, "landed, none in flight names no table");
    assert!(matches!(apply(&mut shell, Action::ReadChain), Applied::Started(Kind::Chain)));
    let old = shell.tasks.started_over(Kind::Chain);
    assert!(old.is_some(), "the reading in flight names the table it started over");
    assert!(matches!(apply(&mut shell, Action::ReadChain), Applied::Refused(Kind::Chain)), "one in flight: nothing starts");
    assert_eq!(shell.tasks.started_over(Kind::Chain), old, "the table unmoved: the one in flight is the old reading");
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: specs(&[&a, &b]) }), Applied::Endpoints(2)));
    assert!(shell.tasks.started_over(Kind::Chain) > old, "the table moved: the reading in flight started over a later one");
    let until = Instant::now() + Duration::from_secs(60);
    while shell.tasks.started_over(Kind::Chain).is_some() && Instant::now() < until {
        let _ = shell.drain();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(shell.tasks.started_over(Kind::Chain), None, "both landed, none in flight names no table");
    assert!(matches!(shell.chain, Some(Done::Chain { sources: 2, .. })), "the old table's reading set nothing");
    tidy();
}

#[test]
fn a_node_table_change_reads_nothing_while_the_basis_chain_has_no_node() {
    if super::alone_in(module_path!(), "a_node_table_change_reads_nothing_while_the_basis_chain_has_no_node") {
        return;
    }
    let a = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r30b1-nonode", &[&a]);
    let only_31337 = format!("31337={}", a.url);
    shell.settings.chain_id = Some(1);
    let before = shell.faults.len();
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: only_31337.clone() }), Applied::Endpoints(1)));
    assert!(!shell.tasks.in_flight(Kind::Chain), "no node for the basis chain: nothing read again");
    assert!(shell.chain.is_none(), "the reading over the table before is gone");
    assert_eq!(shell.faults.len(), before, "and nothing is said");
    assert!(matches!(apply(&mut shell, Action::ReadChain), Applied::Started(Kind::Chain)));
    let asked = landed(&mut shell, Kind::Chain).expect("the asked read lands");
    assert!(matches!(&asked, Err(f) if f.which() == Some(Known::NoEndpoint)), "the asked read names the missing node: {asked:?}");
    shell.settings.chain_id = Some(31337);
    assert!(matches!(apply(&mut shell, Action::SetEndpoints { specs: only_31337 }), Applied::Endpoints(1)));
    assert!(shell.tasks.in_flight(Kind::Chain), "a node for the basis chain: read again");
    let got = landed(&mut shell, Kind::Chain).expect("read again");
    assert!(matches!(got, Ok(Done::Chain { sources: 1, .. })), "read over the one node: {got:?}");
    tidy();
}

#[test]
fn the_exit_gate_reads_over_the_nodes_that_serve_the_chain() {
    if super::alone_in(module_path!(), "the_exit_gate_reads_over_the_nodes_that_serve_the_chain") {
        return;
    }
    let live = node(|m, b| chain(m, b, PLENTY));
    let far = node(|m, b| match m {
        "eth_chainId" => Reply::Result("\"0x1\"".into()),
        "eth_blockNumber" => Reply::Result("\"0x100000\"".into()),
        _ => chain(m, b, PLENTY),
    });
    let mut shell = bench("r36-gate-serving", &[&live]);
    app::chainx::forget_serving();
    shell.endpoints.push(app::chainx::Endpoint::at(31337, far.url.clone()));
    let ask = app::exitgate::ask_of(&shell).expect("the gate's question");
    let got = app::exitgate::read(&ask);
    assert!(got.is_ok(), "read over the node that serves the chain: {:?}", got.err().map(|f| f.raw()));
    tidy();
}
