use super::a8_send::{bench, chain, node, tidy, PLENTY};
use app::action::{apply, Action, Applied, RotateStep};
use app::fault::Known;
use app::task::Kind;
use std::time::{Duration, Instant};

fn settle(shell: &mut app::shell::Shell) {
    let until = Instant::now() + Duration::from_secs(60);
    while !shell.tasks.flying().is_empty() {
        assert!(Instant::now() < until, "the tasks in flight landed: {:?}", shell.tasks.flying());
        shell.drain();
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn read_rows(shell: &mut app::shell::Shell) {
    let until = Instant::now() + Duration::from_secs(60);
    while shell.tasks.in_flight(Kind::Ledger) {
        assert!(Instant::now() < until, "the reading out landed");
        shell.drain();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(apply(shell, Action::ReadLedger), Applied::Started(Kind::Ledger));
    while !shell.drain().iter().any(|o| o.kind == Kind::Ledger) {
        assert!(Instant::now() < until, "the table landed");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn home_texts(shell: app::shell::Shell) -> (app::shell::Shell, Vec<String>) {
    let ctx = zikaron_ui::egui::Context::default();
    zikaron_ui::skin::dress(&ctx);
    let (shell, r) = app::window::probe_face(&ctx, shell, app::nav::Place::Home, 1180.0, 760.0);
    (shell, r.texts)
}

#[test]
fn the_pending_card_says_all_on_chain_only_when_the_ledger_has_nothing_unqueued() {
    if super::alone_in(module_path!(), "the_pending_card_says_all_on_chain_only_when_the_ledger_has_nothing_unqueued") {
        return;
    }
    use app::lang::{fill1, t, Key};
    assert_eq!(app::watchx::unqueued(None, 0), None, "a ledger not read gives no count");
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r33b2-card", &[&n]);
    shell.settings.role = app::roles::Role::Author;
    let home = app::home::Home::bare(shell.home.as_ref().expect("a home").root().to_path_buf());
    settle(&mut shell);
    let (_, q) = app::queue::amend(&home, |q| q.items.clear()).expect("the queue");
    shell.queue = q;
    read_rows(&mut shell);
    let unqueued = app::watchx::unqueued(shell.rows.as_ref().map(|(r, _)| r.as_slice()), shell.queue.len()).expect("the ledger read");
    assert!(unqueued > 0, "entries recorded and not queued");
    let (mut shell, texts) = home_texts(shell);
    assert!(texts.iter().any(|x| *x == fill1(Key::GapUnanchored, &unqueued.to_string())), "the alerts page's count: {texts:?}");
    assert!(!texts.iter().any(|x| x == t(Key::SendAllDone)), "never all on chain: {texts:?}");
    settle(&mut shell);
    let ids: Vec<String> = shell.rows.as_ref().map(|(r, _)| r.iter().map(|x| x.id.clone()).collect()).unwrap_or_default();
    let (_, q) = app::queue::amend(&home, |q| {
        q.items.clear();
        q.anchored = ids.iter().map(|i| (i.clone(), 0)).collect();
    })
    .expect("the queue");
    shell.queue = q;
    read_rows(&mut shell);
    assert_eq!(app::watchx::unqueued(shell.rows.as_ref().map(|(r, _)| r.as_slice()), shell.queue.len()), Some(0));
    let (_shell, texts) = home_texts(shell);
    assert!(texts.iter().any(|x| x == t(Key::SendAllDone)), "all on chain: {texts:?}");
    tidy();
}

#[test]
fn after_a_handover_the_old_keys_balance_goes_where_the_person_says() {
    if super::alone_in(module_path!(), "after_a_handover_the_old_keys_balance_goes_where_the_person_says") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r33b2-sweep", &[&n]);
    let next = app::key::Address([0x77; 20]);
    let other = app::key::Address([0x78; 20]);
    let to = |a: &app::key::Address| Action::Rotate { step: RotateStep::SweepTo { to: a.hex() } };
    match apply(&mut shell, to(&other)) {
        Applied::Trouble(f) => assert_eq!(f.which(), Some(Known::RotateOwed), "{f:?}"),
        got => panic!("refused where nothing was handed on: {got:?}"),
    }
    shell.handed = Some(next.hex());
    shell.rot_left = Some(vec![app::rotatex::Left { chain: 31337, held: 10, cost: 1 }]);
    shell.rot_left_of = app::action::sweep_key(&shell).map(|(_, k)| k);
    match apply(&mut shell, Action::Rotate { step: RotateStep::SweepTo { to: "0x12".into() } }) {
        Applied::Trouble(f) => assert_eq!(f.which(), Some(Known::AddressShape), "{f:?}"),
        got => panic!("an address out of form refused by name: {got:?}"),
    }
    assert_eq!((shell.rot_sweep_to, shell.rot_left.is_some()), (None, true), "nothing kept, nothing read again");
    assert!(matches!(apply(&mut shell, to(&next)), Applied::Rotated { .. }));
    assert_eq!((shell.rot_sweep_to, shell.rot_left.is_some()), (None, true), "the next key: as it was");
    assert!(matches!(apply(&mut shell, to(&other)), Applied::Rotated { .. }));
    assert_eq!((shell.rot_sweep_to, shell.rot_left.clone()), (Some(other), None), "kept; the balance to be read for it");
    for step in [RotateStep::Sweep { chain: 31337, value: 1 }, RotateStep::Leave { chain: 31337 }] {
        match apply(&mut shell, Action::Rotate { step: step.clone() }) {
            Applied::Trouble(f) => assert_ne!(f.which(), Some(Known::HandedOver), "{step:?}: {f:?}"),
            got => panic!("{step:?}: not at that step yet: {got:?}"),
        }
    }
    tidy();
}

#[test]
fn a_given_address_that_holds_office_is_refused_in_its_own_words() {
    if super::alone_in(module_path!(), "a_given_address_that_holds_office_is_refused_in_its_own_words") {
        return;
    }
    use app::lang::Key;
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r33b2-vet", &[&n]);
    let root = shell.anchor.expect("the key in office");
    let here = app::succeedx::seated_here(&app::register::view().expect("the register"), &root).expect_err("held here");
    assert_eq!((here.which(), here.what_key(), here.then_key()), (Some(Known::AlreadyRooted), Some(Key::FaultWhatGivenSeatedHere), Some(Key::FaultNextGivenTaken)));
    assert_eq!(apply(&mut shell, Action::Rotate { step: RotateStep::Vet { to: root.hex() } }), Applied::Started(Kind::Rotate));
    let until = Instant::now() + Duration::from_secs(60);
    while shell.tasks.in_flight(Kind::Rotate) {
        assert!(Instant::now() < until, "the vetting landed");
        shell.drain();
        std::thread::sleep(Duration::from_millis(5));
    }
    let f = shell.failed.get(&Kind::Rotate).cloned().expect("refused");
    assert_eq!((f.which(), f.what_key(), f.then_key()), (Some(Known::AlreadyRooted), Some(Key::FaultWhatGivenInLineage), Some(Key::FaultNextGivenTaken)));
    assert_ne!(f.human(), app::lang::t(Key::FaultWhatAlreadyRooted), "never the words of opening a ledger");
    tidy();
}

#[test]
fn material_that_is_not_the_addresss_ledger_says_so_on_the_reader() {
    if super::alone_in(module_path!(), "material_that_is_not_the_addresss_ledger_says_so_on_the_reader") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let shell = bench("r33b2-reader", &[&n]);
    let items = shell.home.as_ref().expect("a home").ledger().expect("the ledger").pile().expect("its entries").items;
    let shelf = app::supplyx::Shelf { homes: Vec::new(), kept: None, upstreams: Vec::new(), manual: Vec::new(), carried: Some(("/carried".into(), items)), pointer: None, reads: Vec::new() };
    let found = app::supplyx::find_book(&shelf, &app::key::Address([0x55; 20]).hex());
    assert!(found.supply.is_none(), "not the address's ledger: not used");
    let (level, f) = found.misses.first().cloned().expect("named");
    assert_eq!((level, f.which()), (app::supplyx::Level::Kit, Some(Known::NotHisBook)), "{f:?}");
    tidy();
}

#[test]
fn diligence_with_nodes_in_a_read_only_network_alone_reads_through_them() {
    if super::alone_in(module_path!(), "diligence_with_nodes_in_a_read_only_network_alone_reads_through_them") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r33c3-diligence", &[&n]);
    shell.endpoints = Vec::new();
    let machine = app::home::machine_dir().expect("the machine folder");
    let registry = format!("0x{}", "12".repeat(20));
    let net = app::readnets::cells("", "31338", &registry, "0", &n.url).expect("a read-only network");
    app::readnets::save(&machine, None, net, None).expect("saved");
    let who = app::key::Address([0x22; 20]).hex();
    let ask = |shell: &mut app::shell::Shell| apply(shell, Action::Diligence { address: who.clone(), dir: String::new(), work: String::new(), from: String::new(), to: String::new() });
    assert_eq!(ask(&mut shell), Applied::Started(Kind::Diligence), "a read-only network's nodes are nodes");
    let until = Instant::now() + Duration::from_secs(60);
    while !shell.drain().iter().any(|o| o.kind == Kind::Diligence) {
        assert!(Instant::now() < until, "the check never landed");
        std::thread::sleep(Duration::from_millis(5));
    }
    app::readnets::remove(&machine, 31338, &app::key::Address::parse(&registry).expect("an address")).expect("removed");
    match ask(&mut shell) {
        Applied::Trouble(f) => assert_eq!(f.which(), Some(Known::NoEndpoint), "no node anywhere"),
        other => panic!("no node anywhere: {other:?}"),
    }
    tidy();
}
