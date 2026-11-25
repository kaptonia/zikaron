use super::vault_open;
use app::action::{apply, Action, Applied};
use app::secret::Secret;

const ETHERSCAN: &str = "https://api.etherscan.io/v2/api";
const KEY: &str = "ETHERSCAN-TEST-KEY-7f3a";

fn shell_with_home(name: &str) -> (app::shell::Shell, std::path::PathBuf) {
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-rest-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let home = app::home::Home::open_or_create(&dir).expect("a home");
    shell.lock = Some(app::lock::take(&home).expect("the lock"));
    shell.home = Some(home);
    (shell, dir)
}

fn settings_plain(dir: &std::path::Path) -> String {
    let file = dir.join("settings").join(app::settings::FILE);
    String::from_utf8(app::local::read(&file, app::local::Doc::Settings).expect("opens").expect("written")).expect("utf-8")
}

fn refused(a: Applied) -> String {
    match a {
        Applied::Trouble(f) => f.said().split([' ', ':']).next().unwrap_or_default().to_string(),
        other => panic!("refused by name: {other:?}"),
    }
}

#[test]
fn a_rest_row_keeps_its_key_on_this_machine_and_only_its_base_url_in_the_ledger() {
    if super::alone_in(module_path!(), "a_rest_row_keeps_its_key_on_this_machine_and_only_its_base_url_in_the_ledger") {
        return;
    }
    let (mut shell, dir) = shell_with_home("add");
    for (base, key) in [("http://api.etherscan.io/v2/api", KEY), ("https://api.etherscan.io/v2/api?apikey=K", KEY), (ETHERSCAN, ""), (ETHERSCAN, "a b")] {
        assert_eq!(refused(apply(&mut shell, Action::AddRest { base: base.into(), key: Secret::of(key) })), "SETTINGS_SHAPE", "{base} {key:?}");
    }
    assert!(shell.settings.rest.is_empty() && app::machine::read().expect("machine").rest_keys.0.is_empty(), "nothing written");
    match apply(&mut shell, Action::AddRest { base: ETHERSCAN.into(), key: Secret::of(KEY) }) {
        Applied::RestAdded { base } => assert_eq!(base, ETHERSCAN),
        other => panic!("{other:?}"),
    }
    assert_eq!(refused(apply(&mut shell, Action::AddRest { base: "https://API.Etherscan.io/api".into(), key: Secret::of("OTHER") })), "NETWORK_LISTED", "one service, one row");
    let plain = settings_plain(&dir);
    assert!(plain.contains(&format!("\"{}\":[\"{ETHERSCAN}\"]", zikaron_seal::ground::REST)), "{plain}");
    assert!(!plain.contains(KEY) && !plain.contains("7f3a"), "no byte of the key in the ledger's settings: {plain}");
    let machine = app::machine::read().expect("machine");
    assert_eq!(machine.rest_keys.of("api.etherscan.io"), Some(KEY), "the key is this machine's, by service");
    assert_eq!(shell.rest_key_tail(ETHERSCAN).as_deref(), Some("7f3a"));
    let plain_machine = std::fs::read_to_string(app::machine::path().expect("path")).unwrap_or_default();
    assert!(!plain_machine.contains(KEY), "never in the plain machine file");
    match apply(&mut shell, Action::RemoveRest { base: ETHERSCAN.into() }) {
        Applied::RestRemoved { left } => assert_eq!(left, 0),
        other => panic!("{other:?}"),
    }
    assert!(app::machine::read().expect("machine").rest_keys.of("api.etherscan.io").is_none(), "removing the row removes this machine's key");
    assert_eq!(shell.rest_key_tail(ETHERSCAN), None);
    assert_eq!(refused(apply(&mut shell, Action::RemoveRest { base: ETHERSCAN.into() })), "SETTINGS_SHAPE", "a row not listed");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_change_of_rest_rows_moves_the_nodes_voids_the_estimate_and_keeps_the_network() {
    if super::alone_in(module_path!(), "a_change_of_rest_rows_moves_the_nodes_voids_the_estimate_and_keeps_the_network") {
        return;
    }
    let (mut shell, dir) = shell_with_home("switch");
    shell
        .commit_settings(|s| {
            s.chain_id = Some(11155111);
            s.registry = app::key::Address::parse("0x36Ea8A857a5FE813429d4D9947000C644A88809A");
            s.endpoints = vec!["11155111=https://rpc.sepolia.example".into()];
            s.network = Some("sepolia".into());
        })
        .expect("a basis");
    shell.endpoints = app::chainx::nodes_of(&shell.settings);
    assert_eq!(shell.endpoints.len(), 1);
    let epoch = |s: &app::shell::Shell| s.gas_out_of_date();
    assert!(matches!(apply(&mut shell, Action::AddRest { base: ETHERSCAN.into(), key: Secret::of(KEY) }), Applied::RestAdded { .. }));
    assert_eq!(shell.endpoints.len(), 1, "off: the row stays and is not asked");
    shell.gas_asked = Some(shell.gas_epoch);
    shell.chain_read_at = Some(1.0);
    assert!(matches!(apply(&mut shell, Action::SetRestMode { on: true }), Applied::RestMode(true)));
    let urls: Vec<String> = shell.endpoints.iter().map(|e| e.url.for_transport().to_string()).collect();
    assert_eq!(urls, vec!["https://rpc.sepolia.example".to_string(), zikaron_anchor::rest::lane(ETHERSCAN, 11155111)]);
    assert_eq!(shell.endpoints[1].url.kind(), app::chainx::Kind::Rest);
    assert_eq!(shell.endpoints[0].url.kind(), app::chainx::Kind::Rpc);
    assert!(epoch(&shell), "an estimate made before is out of date");
    assert_eq!(shell.chain_read_at, None, "the chain is read again");
    assert_eq!(shell.settings.network.as_deref(), Some("sepolia"), "the network stays the preset it was");
    assert!(settings_plain(&dir).contains(&format!("\"{}\":true", zikaron_seal::ground::REST_ON)));
    assert_eq!(shell.settings.endpoints, vec!["11155111=https://rpc.sepolia.example".to_string()], "the JSON-RPC nodes are untouched");
    shell.gas_asked = Some(shell.gas_epoch);
    assert!(matches!(apply(&mut shell, Action::SetRestMode { on: false }), Applied::RestMode(false)));
    assert_eq!(shell.endpoints.len(), 1);
    assert!(epoch(&shell));
    assert_eq!(shell.settings.rest, vec![ETHERSCAN.to_string()], "off keeps the row");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_chain_is_read_again_only_when_the_nodes_asked_moved() {
    if super::alone_in(module_path!(), "the_chain_is_read_again_only_when_the_nodes_asked_moved") {
        return;
    }
    let (mut shell, dir) = shell_with_home("moved");
    shell
        .commit_settings(|s| {
            s.chain_id = Some(11155111);
            s.registry = app::key::Address::parse("0x36Ea8A857a5FE813429d4D9947000C644A88809A");
            s.endpoints = vec!["11155111=https://rpc.sepolia.example".into()];
        })
        .expect("a basis");
    shell.endpoints = app::chainx::nodes_of(&shell.settings);
    shell.chain_read_at = Some(1.0);
    assert!(matches!(apply(&mut shell, Action::SetRestMode { on: true }), Applied::RestMode(true)));
    assert_eq!(shell.chain_read_at, Some(1.0), "on with no row: nothing asked moved, the reading stands");
    assert!(matches!(apply(&mut shell, Action::SetRestMode { on: false }), Applied::RestMode(false)));
    assert!(matches!(apply(&mut shell, Action::AddRest { base: ETHERSCAN.into(), key: Secret::of(KEY) }), Applied::RestAdded { .. }));
    assert_eq!(shell.chain_read_at, Some(1.0), "off: a row added is not asked, the reading stands");
    assert!(matches!(apply(&mut shell, Action::RemoveRest { base: ETHERSCAN.into() }), Applied::RestRemoved { .. }));
    assert_eq!(shell.chain_read_at, Some(1.0), "off: a row removed was not asked, the reading stands");
    assert!(matches!(apply(&mut shell, Action::SetRestMode { on: true }), Applied::RestMode(true)));
    assert!(matches!(apply(&mut shell, Action::AddRest { base: ETHERSCAN.into(), key: Secret::of(KEY) }), Applied::RestAdded { .. }));
    assert_eq!(shell.chain_read_at, None, "on: a row added is asked, the chain is read again");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_rest_members_are_their_own_and_a_newer_versions_members_ride_along() {
    let (mut shell, dir) = shell_with_home("members");
    let file = dir.join("settings").join(app::settings::FILE);
    let written = format!(
        r#"{{"auditEvery":300,"capBytes":4294967296,"chainId":1,"endpoints":["1=https://rpc.example"],"fromBlock":0,"futureCell":{{"x":[1,"二"]}},"{}":["{ETHERSCAN}","https://eth.blockscout.com/api"],"{}":true,"reviewEvery":600,"role":"author"}}"#,
        zikaron_seal::ground::REST,
        zikaron_seal::ground::REST_ON
    );
    app::local::put(&dir.join("settings"), app::settings::FILE, app::local::Doc::Settings, written.as_bytes()).expect("laid");
    let h = app::home::Home::bare(dir.clone());
    let s = app::settings::Settings::read(&h).expect("reads");
    assert_eq!(s.rest, vec![ETHERSCAN.to_string(), "https://eth.blockscout.com/api".to_string()]);
    assert!(s.rest_on);
    assert_eq!(s.endpoints, vec!["1=https://rpc.example".to_string()], "the node list holds JSON-RPC nodes alone");
    let g = s.ground();
    assert_eq!(g.endpoints, s.endpoints, "the command line's reading of the node list: no REST row in it");
    assert_eq!(zikaron_send::node::nodes_of(&g, 1).len(), 3, "a JSON-RPC node and two REST rows on chain 1");
    shell.settings = s;
    shell.commit_settings(|s| s.audit_every = 600).expect("a save");
    let after = String::from_utf8(app::local::read(&file, app::local::Doc::Settings).expect("opens").expect("written")).expect("utf-8");
    assert!(after.contains("\"futureCell\":{\"x\":[1,\"二\"]}"), "a newer version's member is carried back: {after}");
    assert!(after.contains(zikaron_seal::ground::REST) && after.contains(zikaron_seal::ground::REST_ON), "{after}");
    app::local::put(&dir.join("settings"), app::settings::FILE, app::local::Doc::Settings, br#"{"chainId":1,"endpoints":["1=https://rpc.example"],"role":"author"}"#).expect("laid");
    let old = app::settings::Settings::read(&h).expect("reads");
    assert!(old.rest.is_empty() && !old.rest_on);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_keys_file_the_window_starts_with_keys_a_row_left_without_one() {
    const FORM: &str = "ZK_CHECKS_KEYS_FILE_FORM";
    const FILE_KEY: &str = "FILE-KEY-1c2d";
    let base = "https://rest.example.test/api";
    let full = format!("{}::a_keys_file_the_window_starts_with_keys_a_row_left_without_one", module_path!().split_once("::").map(|(_, m)| m).unwrap_or(module_path!()));
    match std::env::var(FORM).as_deref() {
        Ok("given") => {
            let (mut shell, dir) = shell_with_home("keys-file");
            assert_eq!(app::chainx::keys_file(), None, "the file reads");
            assert_eq!(app::chainx::keys_file_said(), None);
            assert_eq!(refused(apply(&mut shell, Action::AddRest { base: "https://other.example.test/api".into(), key: Secret::of("") })), "SETTINGS_SHAPE", "a service the file gives nothing for");
            match apply(&mut shell, Action::AddRest { base: base.into(), key: Secret::of("") }) {
                Applied::RestAdded { base: b } => assert_eq!(b, base),
                other => panic!("{other:?}"),
            }
            assert!(app::machine::read().expect("machine").rest_keys.0.is_empty(), "this machine's settings untouched");
            assert_eq!(shell.rest_key_said(base), app::lang::t(app::lang::Key::NetRestKeyFromFile));
            assert_eq!(zikaron_anchor::rest::key_of("rest.example.test").as_deref(), Some(FILE_KEY));
            assert_eq!(refused(apply(&mut shell, Action::AddRest { base: base.into(), key: Secret::of("") })), "NETWORK_LISTED", "one service, one row");
            match apply(&mut shell, Action::AddRest { base: base.into(), key: Secret::of("MACHINE-KEY-9b2c") }) {
                Applied::RestKeyed { .. } => {}
                other => panic!("{other:?}"),
            }
            assert_eq!(zikaron_anchor::rest::key_of("rest.example.test").as_deref(), Some("MACHINE-KEY-9b2c"), "this machine's key wins");
            assert_eq!(shell.rest_key_said(base), app::lang::fill1(app::lang::Key::NetRestKeyTail, "9b2c"));
            let plain = settings_plain(&dir);
            let sealed = app::machine::read().expect("machine").rest_keys.0;
            let plain_machine = std::fs::read_to_string(app::machine::path().expect("path")).unwrap_or_default();
            assert!(!plain.contains(FILE_KEY) && !plain_machine.contains(FILE_KEY) && sealed.iter().all(|(_, k)| k != FILE_KEY), "the file's key is written nowhere");
            let _ = std::fs::remove_dir_all(&dir);
        }
        Ok(form) => {
            let (mut shell, dir) = shell_with_home(&format!("keys-file-{form}"));
            let want = match form {
                "unreadable" => zikaron_anchor::rest::KeysFileBad::Unreadable,
                "notUtf8" => zikaron_anchor::rest::KeysFileBad::NotUtf8,
                _ => zikaron_anchor::rest::KeysFileBad::Shape,
            };
            assert_eq!(app::chainx::keys_file(), Some(want), "{form}");
            let said = app::chainx::keys_file_said().expect("said");
            assert!(said.contains(app::chainx::REST_KEYS_ENV) && !said.contains(FILE_KEY), "by the variable and its reason alone: {said}");
            assert_eq!(zikaron_anchor::rest::key_of("rest.example.test"), None, "none of it taken");
            assert_eq!(refused(apply(&mut shell, Action::AddRest { base: base.into(), key: Secret::of("") })), "SETTINGS_SHAPE", "{form}");
            let _ = std::fs::remove_dir_all(&dir);
        }
        Err(_) => {
            let at = std::env::temp_dir().join(format!("zk-rest-keys-file-{}", std::process::id()));
            let _ = std::fs::create_dir_all(&at);
            let me = std::env::current_exe().expect("this test binary");
            for (form, bytes) in [
                ("given", format!("# 钥匙文件\n\nrest.example.test {FILE_KEY}\n").into_bytes()),
                ("unreadable", Vec::new()),
                ("notUtf8", vec![0xff, 0xfe, b'\n']),
                ("shape", format!("rest.example.test {FILE_KEY} more\n").into_bytes()),
            ] {
                let file = at.join(form);
                if form != "unreadable" {
                    std::fs::write(&file, &bytes).expect("laid");
                }
                let out = super::output_of(std::process::Command::new(&me).args([full.as_str(), "--exact", "--nocapture", "--test-threads=1"]).env(FORM, form).env(app::chainx::REST_KEYS_ENV, &file)).expect("the child runs");
                assert!(out.status.success() && String::from_utf8_lossy(&out.stdout).contains("1 passed"), "{form}\n{}\n{}", super::child_lines(&out.stdout), super::child_lines(&out.stderr));
            }
            let _ = std::fs::remove_dir_all(&at);
        }
    }
}

use super::a8_send::{chain, node, Node, Reply, PLENTY};
use app::task::{Done, Kind};

const CAPPED: &str = "{\"code\":-32012,\"message\":\"ErrGetLogsExceededMaxAllowedRange\",\"data\":\"maxAllowedRange 30000 requestRange 376868\"}";

fn answering() -> Node {
    node(|m, b| match m {
        "eth_getCode" => Reply::Result("\"0x\"".into()),
        _ => chain(m, b, PLENTY),
    })
}

fn capped() -> Node {
    node(|m, b| match m {
        "eth_getLogs" => Reply::Error(CAPPED.into()),
        "eth_getCode" => Reply::Result("\"0x\"".into()),
        _ => chain(m, b, PLENTY),
    })
}

fn eps(nodes: &[&Node]) -> Vec<app::chainx::Endpoint> {
    nodes.iter().map(|n| app::chainx::Endpoint::at(31337, n.url.clone())).collect()
}

fn ground() -> app::auditx::Ground {
    app::auditx::Ground { chain: 31337, registry: app::key::Address([0x11; 20]), from_block: 0, to_block: 0x40, senders: vec![format!("0x{}", "22".repeat(20))], earlier: Vec::new() }
}

fn basis() -> Vec<u8> {
    zikaron::json::canon_bytes(&app::auditx::basis_of(&ground()))
}

fn place(n: &Node) -> String {
    zikaron_net::sayable(&n.url)
}

#[test]
fn the_single_source_reading_asks_the_table_in_order_and_names_each_refusal() {
    if super::alone_in(module_path!(), "the_single_source_reading_asks_the_table_in_order_and_names_each_refusal") {
        return;
    }
    vault_open();
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let (refusing, open) = (capped(), answering());
    let (_, _, said) = app::auditx::scan_table(&eps(&[&refusing, &open]), &basis()).unwrap_or_else(|u| panic!("read past the first: {}", u.fault.evidence()));
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with(&place(&refusing)) && said[0].contains("-32012") && said[0].contains("maxAllowedRange 30000"), "the first node named with its words: {said:?}");
    assert!(!open.asked("eth_getLogs").is_empty(), "the second node asked");
    let (first, spare) = (answering(), capped());
    let (_, _, said) = app::auditx::scan_table(&eps(&[&first, &spare]), &basis()).unwrap_or_else(|u| panic!("{}", u.fault.evidence()));
    assert!(said.is_empty(), "{said:?}");
    assert!(spare.asked("eth_getLogs").is_empty(), "a first node answering spares the next");
    let (one, two) = (capped(), capped());
    let Err(u) = app::auditx::scan_table(&eps(&[&one, &two]), &basis()) else { panic!("no node answered") };
    assert_eq!(u.unanswered.len(), 2, "{:?}", u.unanswered);
    assert!(u.unanswered[0].starts_with(&place(&one)) && u.unanswered[1].starts_with(&place(&two)), "each node, in table order: {:?}", u.unanswered);
    assert_eq!(u.fault.which(), Some(app::fault::Known::ScanRefused));
    assert_eq!(u.fault.tail(), u.unanswered.join("; "), "the refusal's evidence is the same words");
}

fn high(m: &str, b: &str) -> Reply {
    match m {
        "eth_blockNumber" => Reply::Result(format!("\"0x{HIGH:x}\"")),
        "eth_getCode" => Reply::Result("\"0x\"".into()),
        _ => chain(m, b, PLENTY),
    }
}

const HIGH: u64 = 0x400;

fn wide_basis() -> Vec<u8> {
    zikaron::json::canon_bytes(&app::auditx::basis_of(&app::auditx::Ground { to_block: HIGH, ..ground() }))
}

fn bounded(cap: u64) -> Node {
    node(move |m, b| match m {
        "eth_getLogs" => {
            let (from, to) = span(b);
            if to - from + 1 > cap {
                Reply::Error(format!("{{\"code\":-32012,\"message\":\"ErrGetLogsExceededMaxAllowedRange\",\"data\":\"maxAllowedRange {cap} requestRange {}\"}}", to - from + 1))
            } else {
                Reply::Result("[]".into())
            }
        }
        _ => high(m, b),
    })
}

fn whole() -> Node {
    node(|m, b| match m {
        "eth_getLogs" => Reply::Result("[]".into()),
        _ => high(m, b),
    })
}

fn limited() -> Node {
    node(|m, b| match m {
        "eth_getLogs" => Reply::Error("{\"code\":-32005,\"message\":\"daily request count exceeded, request rate limited\"}".into()),
        _ => high(m, b),
    })
}

fn silent() -> Node {
    node(|m, b| match m {
        "eth_getLogs" => Reply::Close,
        _ => high(m, b),
    })
}

fn span(body: &str) -> (u64, u64) {
    let at = |k: &str| body.split(&format!("\"{k}\":\"0x")).nth(1).and_then(|r| r.split('"').next()).and_then(|h| u64::from_str_radix(h, 16).ok()).unwrap_or(0);
    (at("fromBlock"), at("toBlock"))
}

fn pieces(n: &Node, cap: u64) -> Vec<(u64, u64)> {
    n.asked("eth_getLogs").iter().map(|b| span(b)).filter(|(a, b)| b - a + 1 <= cap).collect()
}

fn covers(spans: &[(u64, u64)]) -> bool {
    let mut next = 0;
    for (a, b) in spans {
        if *a != next {
            return false;
        }
        next = b + 1;
    }
    next == HIGH + 1
}

#[test]
fn every_round_refused_for_its_width_is_read_in_pieces() {
    if super::alone_in(module_path!(), "every_round_refused_for_its_width_is_read_in_pieces") {
        return;
    }
    vault_open();
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let (one, two) = (bounded(256), bounded(256));
    let (_, _, said) = app::auditx::scan_table(&eps(&[&one, &two]), &wide_basis()).unwrap_or_else(|u| panic!("read in pieces: {}", u.fault.evidence()));
    assert!(said.is_empty(), "the round read in pieces says no refusal: {said:?}");
    assert!(covers(&pieces(&one, 256)) || covers(&pieces(&two, 256)), "a node read the window in pieces it answered: {:?} {:?}", pieces(&one, 256), pieces(&two, 256));
    let (rated, capped_after) = (limited(), bounded(256));
    let (_, _, said) = app::auditx::scan_table(&eps(&[&rated, &capped_after]), &wide_basis()).unwrap_or_else(|u| panic!("read in pieces: {}", u.fault.evidence()));
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with(&place(&rated)), "the rate-limited node named: {said:?}");
    assert!(pieces(&rated, 256).is_empty(), "a rate-limited round is not cut: {:?}", pieces(&rated, 256));
    assert!(covers(&pieces(&capped_after, 256)), "the bounding node read in pieces: {:?}", pieces(&capped_after, 256));
    let (narrow, wider) = (bounded(64), bounded(256));
    let (_, _, said) = app::auditx::scan_table(&eps(&[&narrow, &wider]), &wide_basis()).unwrap_or_else(|u| panic!("read in pieces: {}", u.fault.evidence()));
    assert!(said.is_empty(), "the narrower cuts on its own too and nobody is named: {said:?}");
    assert!(covers(&pieces(&narrow, 64)) || covers(&pieces(&wider, 256)), "a node read the window in pieces it answered: {:?} {:?}", pieces(&narrow, 64), pieces(&wider, 256));
}

#[test]
fn a_round_reading_the_whole_range_cuts_none() {
    if super::alone_in(module_path!(), "a_round_reading_the_whole_range_cuts_none") {
        return;
    }
    vault_open();
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let (narrow, open) = (bounded(256), whole());
    let (_, _, said) = app::auditx::scan_table(&eps(&[&narrow, &open]), &wide_basis()).unwrap_or_else(|u| panic!("{}", u.fault.evidence()));
    assert_eq!(said.len(), 1, "{said:?}");
    assert!(said[0].starts_with(&place(&narrow)) && said[0].contains("maxAllowedRange 256"), "{said:?}");
    assert!(pieces(&narrow, 256).is_empty(), "no piece asked while a node takes the range: {:?}", pieces(&narrow, 256));
}

#[test]
fn rounds_rate_limited_or_unanswered_are_not_cut() {
    if super::alone_in(module_path!(), "rounds_rate_limited_or_unanswered_are_not_cut") {
        return;
    }
    vault_open();
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let (rated, quiet) = (limited(), silent());
    let Err(u) = app::auditx::scan_table(&eps(&[&rated, &quiet]), &wide_basis()) else { panic!("no node answered") };
    assert_eq!(u.fault.which(), Some(app::fault::Known::ScanRefused));
    assert_eq!(u.unanswered.len(), 2, "{:?}", u.unanswered);
    assert!(u.unanswered[0].starts_with(&place(&rated)) && u.unanswered[1].starts_with(&place(&quiet)), "{:?}", u.unanswered);
    let narrower = |n: &Node| n.asked("eth_getLogs").iter().map(|b| span(b)).filter(|s| *s != (0, HIGH)).collect::<Vec<_>>();
    assert!(narrower(&rated).is_empty() && narrower(&quiet).is_empty(), "no narrower question: {:?} {:?}", narrower(&rated), narrower(&quiet));
}

#[test]
fn each_reader_of_the_single_source_reading_reads_past_a_refusing_first_node() {
    if super::alone_in(module_path!(), "each_reader_of_the_single_source_reading_reads_past_a_refusing_first_node") {
        return;
    }
    vault_open();
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let (refusing, open) = (capped(), answering());
    let table = eps(&[&refusing, &open]);
    let g = ground();
    let who = app::key::Address([0x22; 20]);
    assert!(app::auditx::scan_once(&table, &g).is_ok(), "scan once");
    assert!(app::succeedx::look_at(&table, &g, &who).is_ok(), "a key looked at");
    assert!(app::readerx::audit_read(&table, g.clone(), &who, &[], &[]).is_ok(), "a ledger read");
    let w = app::widex::scan(Some((&table, &g)), &[], &g.senders, app::widex::Ask::First).unwrap_or_else(|f| panic!("across networks: {}", f.evidence()));
    assert!(w.unanswered.iter().any(|x| x.starts_with(&place(&refusing)) && x.contains("-32012")), "across networks, the node refused named: {:?}", w.unanswered);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.settings.chain_id = Some(31337);
    shell.settings.registry = Some(app::key::Address([0x11; 20]));
    shell.endpoints = table.clone();
    let started = apply(&mut shell, Action::Diligence { address: who.hex(), dir: String::new(), work: String::new(), from: String::new(), to: String::new() });
    assert_eq!(started, Applied::Started(Kind::Diligence));
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let landed = loop {
        if let Some(o) = shell.drain().into_iter().find(|o| o.kind == Kind::Diligence) {
            break o.result;
        }
        assert!(std::time::Instant::now() < until, "the check never landed");
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    assert!(matches!(landed, Ok(Done::Diligence(_))), "a counterparty checked: {:?}", landed.err().map(|f| f.evidence()));
}
