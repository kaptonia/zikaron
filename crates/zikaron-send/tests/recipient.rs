use std::io::{Read, Write};
use zikaron::json::Value;
use zikaron_send::node::NodeAddr;
use zikaron_send::fingerprint::{gate, Gate, Why as Miss};
use zikaron_send::recipient::{key_for, key_for_root, key_of, Found, Net};
use zikaron_seal::addressed::Why;

const CHAIN: u64 = 31337;
const REG: [u8; 20] = [0x11; 20];
const AUTHOR: [u8; 32] = [0x2a; 32];
const OTHER: [u8; 32] = [0x3b; 32];

fn hex(b: &[u8]) -> String {
    zikaron::hexfmt::encode(b)
}

fn addr(k: &[u8; 32]) -> [u8; 20] {
    zikaron::cryptox::address_of_privkey(k).expect("a key")
}

fn signed(key: &[u8; 32], hash: [u8; 32], nonce: u64) -> (Vec<u8>, [u8; 32]) {
    zikaron_anchor::send::sign_at(key, CHAIN, zikaron_anchor::send::Form::Registry, Some(REG), &[hash], None, zikaron_anchor::send::Fees::fallback(), nonce).expect("signed")
}

fn tx_json(raw: &[u8], h: &[u8; 32]) -> String {
    let items = zikaron_anchor::rlp::decode_all(&raw[1..]).and_then(|i| i.list().map(|x| x.to_vec())).unwrap_or_default();
    let b = |i: usize| items.get(i).and_then(|x| x.bytes()).map(|x| x.to_vec()).unwrap_or_default();
    let qty = |v: Vec<u8>| -> String {
        let t: Vec<u8> = v.into_iter().skip_while(|x| *x == 0).collect();
        if t.is_empty() { "0x0".into() } else { format!("0x{}", hex(&t).trim_start_matches("0x").trim_start_matches('0')) }
    };
    format!(
        r#"{{"accessList":[],"blockNumber":"0x10","chainId":"{}","gas":"{}","hash":"{}","input":"{}","maxFeePerGas":"{}","maxPriorityFeePerGas":"{}","nonce":"{}","r":"{}","s":"{}","to":"{}","type":"0x2","v":"{}","value":"{}","yParity":"{}"}}"#,
        qty(b(0)),
        qty(b(4)),
        hex(h),
        hex(&b(7)),
        qty(b(3)),
        qty(b(2)),
        qty(b(1)),
        hex(&b(10)),
        hex(&b(11)),
        hex(&b(5)),
        qty(b(9)),
        qty(b(6)),
        qty(b(9))
    )
}

fn log(claimed: &[u8; 20], hash: [u8; 32], tx: &[u8; 32]) -> String {
    let mut padded = [0u8; 32];
    padded[12..].copy_from_slice(claimed);
    format!(
        r#"{{"address":"{}","blockNumber":"0x10","data":"0x","logIndex":"0x0","topics":["{}","{}","{}"],"transactionHash":"{}"}}"#,
        hex(&REG),
        hex(&zikaron_anchor::scan::topic0()),
        hex(&padded),
        hex(&hash),
        hex(tx)
    )
}

fn node(logs: Option<Vec<String>>, txs: Vec<([u8; 32], String)>) -> NodeAddr {
    node_coded(logs, txs, None)
}

fn node_coded(logs: Option<Vec<String>>, txs: Vec<([u8; 32], String)>, code: Option<&'static str>) -> NodeAddr {
    node_at(logs, txs, code, None)
}

fn node_logged(logs: Option<Vec<String>>, txs: Vec<([u8; 32], String)>, asked: std::sync::Arc<std::sync::Mutex<Vec<String>>>) -> NodeAddr {
    node_at(logs, txs, None, Some(asked))
}

fn node_at(logs: Option<Vec<String>>, txs: Vec<([u8; 32], String)>, code: Option<&'static str>, asked: Option<std::sync::Arc<std::sync::Mutex<Vec<String>>>>) -> NodeAddr {
    node_saying(logs, txs, code, asked, "{\"code\":-32000,\"message\":\"refused\"}")
}

fn node_saying(logs: Option<Vec<String>>, txs: Vec<([u8; 32], String)>, code: Option<&'static str>, asked: Option<std::sync::Arc<std::sync::Mutex<Vec<String>>>>, said: &'static str) -> NodeAddr {
    node_bounded(logs, txs, code, asked, said, None)
}

fn node_bounded(logs: Option<Vec<String>>, txs: Vec<([u8; 32], String)>, code: Option<&'static str>, asked: Option<std::sync::Arc<std::sync::Mutex<Vec<String>>>>, said: &'static str, cap: Option<u64>) -> NodeAddr {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = l.local_addr().expect("its address").port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let mut got: Vec<u8> = Vec::new();
            let mut b = [0u8; 8192];
            let body = loop {
                let Ok(n) = s.read(&mut b) else { break String::new() };
                if n == 0 {
                    break String::new();
                }
                got.extend_from_slice(&b[..n]);
                let text = String::from_utf8_lossy(&got).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let length = text[..end].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                    if got.len() >= end + 4 + length {
                        break text[end + 4..].to_string();
                    }
                }
            };
            let req = zikaron::json::parse(body.as_bytes()).unwrap_or(Value::Null);
            let id = req.member("id").map(|v| String::from_utf8_lossy(&zikaron::json::canon_bytes(v)).into_owned()).unwrap_or_else(|| "null".into());
            let method = req.member("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
            let block = |k: &str| req.member("params").and_then(|p| p.as_arr()).and_then(|a| a.first()).and_then(|f| f.member(k)).and_then(|v| v.as_str()).and_then(|h| u64::from_str_radix(h.trim_start_matches("0x"), 16).ok());
            let span = (method == "eth_getLogs").then(|| (block("fromBlock").unwrap_or(0), block("toBlock").unwrap_or(0)));
            if let Some(asked) = &asked {
                let entry = match (span, cap) {
                    (Some((a, b)), Some(_)) => format!("{method} {a}-{b}"),
                    _ => method.clone(),
                };
                asked.lock().unwrap_or_else(|e| e.into_inner()).push(entry);
            }
            let first = req.member("params").and_then(|p| p.as_arr()).and_then(|a| a.first().and_then(|v| v.as_str().map(str::to_string)));
            let wide = matches!((span, cap), (Some((a, b)), Some(c)) if b.saturating_sub(a) + 1 > c);
            let result = match method.as_str() {
                "eth_chainId" => Some(format!("\"0x{CHAIN:x}\"")),
                "eth_blockNumber" if cap.is_some() => Some("\"0x400\"".into()),
                "eth_blockNumber" => Some("\"0x20\"".into()),
                "eth_getLogs" if wide => None,
                "eth_getLogs" => logs.as_ref().map(|l| {
                    let inside = |x: &String| match (span, cap) {
                        (Some((a, b)), Some(_)) => x.split("\"blockNumber\":\"0x").nth(1).and_then(|r| r.split('"').next()).and_then(|h| u64::from_str_radix(h, 16).ok()).is_some_and(|n| a <= n && n <= b),
                        _ => true,
                    };
                    format!("[{}]", l.iter().filter(|x| inside(x)).cloned().collect::<Vec<_>>().join(","))
                }),
                "eth_getTransactionByHash" => Some(txs.iter().find(|(h, _)| Some(hex(h)) == first).map(|(_, j)| j.clone()).unwrap_or_else(|| "null".into())),
                "eth_getCode" => code.map(|c| format!("\"{c}\"")),
                _ => None,
            };
            let reply = match result {
                Some(r) => format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{r}}}"),
                None => format!("{{\"error\":{said},\"id\":{id},\"jsonrpc\":\"2.0\"}}"),
            };
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
        }
    });
    NodeAddr::new(format!("http://127.0.0.1:{port}"))
}

fn net(nodes: Vec<NodeAddr>) -> Net {
    Net { chain: CHAIN, registry: REG, from_block: 0, nodes }
}

fn holding() -> NodeAddr {
    let (raw, h) = signed(&AUTHOR, [0x01; 32], 0);
    let (relay_raw, relay_h) = signed(&AUTHOR, [0x02; 32], 1);
    node(Some(vec![log(&addr(&AUTHOR), [0x01; 32], &h), log(&addr(&OTHER), [0x02; 32], &relay_h)]), vec![(h, tx_json(&raw, &h)), (relay_h, tx_json(&relay_raw, &relay_h))])
}

#[test]
fn the_key_recovered_from_the_address_own_transaction_is_found_and_its_address_is_the_one_asked() {
    let k = key_of(&[net(vec![holding()])], &addr(&AUTHOR));
    let Found::Key(key) = k else { panic!("{k:?}") };
    assert_eq!(zikaron::cryptox::address_of_pubkey(&key), addr(&AUTHOR));
    assert_eq!(key[0], 4, "uncompressed");
}

#[test]
fn an_anchor_another_signer_sent_for_the_address_gives_no_key() {
    assert_eq!(key_of(&[net(vec![holding()])], &addr(&OTHER)), Found::NoAnchor);
}

#[test]
fn not_anchored_needs_every_node_of_every_network_read_through() {
    let empty = || node(Some(Vec::new()), Vec::new());
    let refusing = || node(None, Vec::new());
    let nobody = [0x5b; 20];
    assert_eq!(key_of(&[net(vec![empty(), empty()]), net(vec![empty()])], &nobody), Found::NoAnchor);
    assert!(matches!(key_of(&[net(vec![empty(), refusing()])], &nobody), Found::Unread(_)), "a node that did not read");
    assert!(matches!(key_of(&[net(vec![empty()]), net(Vec::new())], &nobody), Found::Unread(_)), "a network with no node");
    assert!(matches!(key_of(&[net(vec![NodeAddr::new("http://127.0.0.1:1")])], &nobody), Found::Unread(_)), "a node not there");
    assert!(matches!(key_of(&[net(vec![refusing(), holding()])], &addr(&AUTHOR)), Found::Key(_)));
    assert!(matches!(key_of(&[net(vec![empty()]), net(vec![holding()])], &addr(&AUTHOR)), Found::Key(_)));
}

#[test]
fn a_node_that_refuses_the_range_leaves_not_anchored_to_the_nodes_that_answered() {
    const TOO_WIDE: &str = "{\"code\":-32012,\"message\":\"ErrGetLogsExceededMaxAllowedRange\",\"data\":\"maxAllowedRange 30000 requestRange 376868\"}";
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let empty = || node(Some(Vec::new()), Vec::new());
    let too_wide = || node_saying(None, Vec::new(), None, None, TOO_WIDE);
    let nobody = [0x5c; 20];
    assert_eq!(key_of(&[net(vec![empty(), too_wide()])], &nobody), Found::NoAnchor, "the reading stands on the node that answered");
    let Found::Unread(said) = key_of(&[net(vec![too_wide()])], &nobody) else { panic!("no node answered: not read") };
    assert!(said.iter().any(|w| w.contains("-32012")), "the node named with its words: {said:?}");
    assert!(matches!(key_of(&[net(vec![empty(), too_wide()]), net(vec![too_wide()])], &nobody), Found::Unread(_)), "a network no node answered");
    assert!(matches!(key_of(&[net(vec![empty(), node(None, Vec::new())])], &nobody), Found::Unread(_)), "a failure of the moment");
    let pinned = zikaron_anchor::deploy::CODE_HASH.to_string();
    assert!(matches!(key_for(&nobody, vec![net(vec![empty(), too_wide()])], &[], Vec::new(), &pinned), Err((Why::NotAnchored, _))), "ADDRESS_NOT_ANCHORED");
}

#[test]
fn a_network_every_node_of_which_refused_the_range_for_its_width_is_read_in_pieces() {
    const CAPPED: &str = "{\"code\":-32012,\"message\":\"ErrGetLogsExceededMaxAllowedRange\",\"data\":\"maxAllowedRange 256 requestRange 1025\"}";
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    type Asked = std::sync::Arc<std::sync::Mutex<Vec<String>>>;
    let asked = || -> Asked { std::sync::Arc::new(std::sync::Mutex::new(Vec::new())) };
    let pieces = |a: &Asked| -> Vec<(u64, u64)> {
        a.lock().unwrap_or_else(|e| e.into_inner()).iter().filter_map(|m| m.strip_prefix("eth_getLogs ")).filter_map(|r| r.split_once('-')).filter_map(|(x, y)| Some((x.parse::<u64>().ok()?, y.parse::<u64>().ok()?))).filter(|(x, y)| y - x + 1 <= 256).collect()
    };
    let covers = |v: &[(u64, u64)]| {
        let mut next = 0;
        for (a, b) in v {
            if *a != next {
                return false;
            }
            next = b + 1;
        }
        next == 0x401
    };
    let nobody = [0x5d; 20];
    let bounded = |a: &Asked| node_bounded(Some(Vec::new()), Vec::new(), None, Some(a.clone()), CAPPED, Some(256));
    let (one, two) = (asked(), asked());
    assert_eq!(key_of(&[net(vec![bounded(&one), bounded(&two)])], &nobody), Found::NoAnchor, "read in pieces: not anchored");
    assert!(covers(&pieces(&one)) || covers(&pieces(&two)), "a node read the window in pieces of its bound, the other stopped: {:?} {:?}", pieces(&one), pieces(&two));
    let (raw, h) = signed(&AUTHOR, [0x01; 32], 0);
    let holds = node_bounded(Some(vec![log(&addr(&AUTHOR), [0x01; 32], &h)]), vec![(h, tx_json(&raw, &h))], None, Some(asked()), CAPPED, Some(256));
    assert!(matches!(key_of(&[net(vec![holds])], &addr(&AUTHOR)), Found::Key(_)), "the key found in pieces");
    let (beside, wide) = (asked(), asked());
    let open = node_bounded(Some(Vec::new()), Vec::new(), None, Some(wide), CAPPED, Some(u64::MAX));
    assert_eq!(key_of(&[net(vec![open, bounded(&beside)])], &nobody), Found::NoAnchor);
    assert!(pieces(&beside).is_empty(), "a node that read the whole cuts none: {:?}", pieces(&beside));
    let other = asked();
    let open = node_bounded(Some(Vec::new()), Vec::new(), None, Some(asked()), CAPPED, Some(u64::MAX));
    assert_eq!(key_of(&[net(vec![open]), net(vec![bounded(&other)])], &nobody), Found::NoAnchor, "each network cut on its own");
    assert!(covers(&pieces(&other)), "the bounded network read in pieces: {:?}", pieces(&other));
    const CAPPED_64: &str = "{\"code\":-32012,\"message\":\"ErrGetLogsExceededMaxAllowedRange\",\"data\":\"maxAllowedRange 64 requestRange 1025\"}";
    let (wider, narrower) = (asked(), asked());
    let narrow = node_bounded(Some(Vec::new()), Vec::new(), None, Some(narrower.clone()), CAPPED_64, Some(64));
    assert_eq!(key_of(&[net(vec![narrow, bounded(&wider)])], &nobody), Found::NoAnchor, "both cut, the first to read it whole is the reading");
    assert!(covers(&pieces(&wider)) || covers(&pieces(&narrower)), "one of them read the window in pieces it answered: {:?} {:?}", pieces(&wider), pieces(&narrower));
}

#[test]
fn a_log_whose_transaction_the_node_does_not_give_is_not_read_as_no_anchor() {
    let (_, h) = signed(&AUTHOR, [0x01; 32], 0);
    let half = node(Some(vec![log(&addr(&AUTHOR), [0x01; 32], &h)]), Vec::new());
    assert!(matches!(key_of(&[net(vec![half])], &addr(&AUTHOR)), Found::Unread(_)));
}

const CODE: &str = "0x6001600055";

fn pin() -> String {
    hex(&zikaron::cryptox::keccak256(&zikaron::hexfmt::decode(CODE).expect("hex")))
}

fn urls(nodes: &[NodeAddr]) -> Vec<String> {
    nodes.iter().map(|n| n.for_transport().to_string()).collect()
}

#[test]
fn a_read_only_network_is_read_only_past_its_fingerprint_gate() {
    let pinned = pin();
    let coded = || node_coded(Some(Vec::new()), Vec::new(), Some(CODE));
    assert_eq!(gate(CHAIN, &REG, &urls(&[coded()]), &pinned), Gate::Single);
    assert_eq!(gate(CHAIN, &REG, &urls(&[coded(), coded()]), &pinned), Gate::Agreed(2));
    let other = node_coded(Some(Vec::new()), Vec::new(), Some("0x00"));
    assert_eq!(gate(CHAIN, &REG, &urls(&[coded(), other]), &pinned), Gate::Fingerprint, "another build closes the network");
    let Gate::Down(misses) = gate(CHAIN, &REG, &["not a node".to_string(), "http://127.0.0.1:1".to_string(), urls(&[node(Some(Vec::new()), Vec::new())])[0].clone()], &pinned) else { panic!("down") };
    assert_eq!(misses.len(), 3);
    assert_eq!(misses[0].why, Miss::NotAddress);
    assert!(matches!(misses[1].why, Miss::Trouble(_)), "{:?}", misses[1]);
    assert!(matches!(misses[2].why, Miss::Trouble(_)), "a node that refuses the code question: {:?}", misses[2]);
    assert!(!Gate::Fingerprint.admits() && Gate::Single.admits());
}

const REVOCATION_CODE: &str = "0x6004361015600b575f80fd5b5f3560e01c63c266461014601d575f80fd5b34607e576040366003190112607e57600435337fee1610244dfc4b116b1433dd4459fd7b1d3d2bf737a05fc3f6d8f173005d71475f80a3602435337f6e70be4be1a4aebd688b5523bd8b6278acac3963d71ebf2bd5ea50757047664b5f80a3005b5f80fd";

#[test]
fn each_contract_passes_the_gate_on_its_own_pin_alone() {
    let revocation_pin = zikaron_anchor::revoke::CODE_HASH;
    assert_eq!(hex(&zikaron::cryptox::keccak256(&zikaron::hexfmt::decode(REVOCATION_CODE).expect("hex"))), revocation_pin, "the code is the pinned revocation build");
    let revocation = || node_coded(Some(Vec::new()), Vec::new(), Some(REVOCATION_CODE));
    assert_eq!(gate(CHAIN, &REG, &urls(&[revocation()]), revocation_pin), Gate::Single, "the revocation contract's place");
    assert_eq!(gate(CHAIN, &REG, &urls(&[revocation(), revocation()]), revocation_pin), Gate::Agreed(2));
    let other = node_coded(Some(Vec::new()), Vec::new(), Some("0x00"));
    assert_eq!(gate(CHAIN, &REG, &urls(&[revocation(), other]), revocation_pin), Gate::Fingerprint, "another build beside it closes the place");
    assert_eq!(gate(CHAIN, &REG, &urls(&[revocation()]), &pin()), Gate::Fingerprint, "the revocation build in a registry's place");
    let registry = || node_coded(Some(Vec::new()), Vec::new(), Some(CODE));
    assert_eq!(gate(CHAIN, &REG, &urls(&[registry()]), revocation_pin), Gate::Fingerprint, "a registry build in the revocation contract's place");
}

#[test]
fn a_node_answering_no_code_does_not_count_and_closes_nothing() {
    let pinned = pin();
    let coded = || node_coded(Some(Vec::new()), Vec::new(), Some(CODE));
    let behind = || node_coded(Some(Vec::new()), Vec::new(), Some("0x"));
    let (g, counted) = zikaron_send::fingerprint::gate_counted(CHAIN, &REG, &urls(&[coded(), behind()]), &pinned);
    assert_eq!(g, Gate::Single, "the node with the pinned code counts; the one behind does not, and closes nothing");
    assert_eq!(counted.len(), 1, "read on the node that counted alone");
    assert_eq!(gate(CHAIN, &REG, &urls(&[behind(), behind()]), &pinned), Gate::Fingerprint);
    let Gate::Down(misses) = gate(CHAIN, &REG, &[urls(&[behind()])[0].clone(), "http://127.0.0.1:1".to_string()], &pinned) else { panic!("down") };
    assert_eq!(misses[0].why, Miss::NoCode);
    let nobody = [0x5b; 20];
    let ro = Net { chain: CHAIN, registry: [0x22; 20], from_block: 0, nodes: vec![coded(), behind()] };
    assert_eq!(key_for(&nobody, vec![net(vec![node(Some(Vec::new()), Vec::new())])], &[ro], Vec::new(), &pinned), Err((Why::NotAnchored, String::new())));
}

#[test]
fn both_ends_find_a_key_by_one_judgement() {
    let pinned = pin();
    let author = addr(&AUTHOR);
    let empty_ro = || Net { chain: CHAIN, registry: REG, from_block: 0, nodes: vec![node_coded(Some(Vec::new()), Vec::new(), Some(CODE))] };
    let k = key_for(&author, vec![net(vec![holding()])], &[empty_ro()], Vec::new(), &pinned).expect("found");
    let full = zikaron_seal::crypto::uncompressed(&k).expect("a point");
    assert_eq!(zikaron::cryptox::address_of_pubkey(&full), author);
    let nobody = [0x5b; 20];
    assert_eq!(key_for(&nobody, vec![net(vec![node(Some(Vec::new()), Vec::new())])], &[empty_ro()], Vec::new(), &pinned), Err((Why::NotAnchored, String::new())));
    let other_build = Net { chain: CHAIN, registry: [0x22; 20], from_block: 0, nodes: vec![node_coded(Some(Vec::new()), Vec::new(), Some("0x00"))] };
    let r = key_for(&nobody, vec![net(vec![node(Some(Vec::new()), Vec::new())])], &[other_build], Vec::new(), &pinned);
    assert!(matches!(&r, Err((Why::ChainUnread, w)) if w.contains("fingerprint")), "{r:?}");
    assert_eq!(key_for(&nobody, Vec::new(), &[empty_ro()], Vec::new(), &pinned), Err((Why::NotAnchored, String::new())));
    let r = key_for(&nobody, Vec::new(), &[], Vec::new(), &pinned);
    assert!(matches!(&r, Err((Why::ChainUnread, w)) if w.contains("NO_NETWORK")), "{r:?}");
    let ro_holding = Net { chain: CHAIN, registry: REG, from_block: 0, nodes: vec![holding_coded()] };
    assert!(key_for(&author, Vec::new(), &[ro_holding], Vec::new(), &pinned).is_ok());
}

#[test]
fn a_copy_of_the_main_network_in_the_read_only_table_is_the_main_network() {
    let pinned = pin();
    let nobody = [0x5b; 20];
    let main = || net(vec![node(Some(Vec::new()), Vec::new())]);
    let down = Net { chain: CHAIN, registry: REG, from_block: 0, nodes: vec![NodeAddr::new("http://127.0.0.1:1")] };
    assert_eq!(key_for(&nobody, vec![main()], &[down], Vec::new(), &pinned), Err((Why::NotAnchored, String::new())));
    let other_build = Net { chain: CHAIN, registry: REG, from_block: 0, nodes: vec![node_coded(Some(Vec::new()), Vec::new(), Some("0x00"))] };
    assert_eq!(key_for(&nobody, vec![main()], &[other_build], Vec::new(), &pinned), Err((Why::NotAnchored, String::new())));
    let holding_copy = Net { chain: CHAIN, registry: REG, from_block: 0, nodes: vec![holding_coded()] };
    assert!(key_for(&addr(&AUTHOR), vec![main()], &[holding_copy], Vec::new(), &pinned).is_ok());
}

fn holding_coded() -> NodeAddr {
    let (raw, h) = signed(&AUTHOR, [0x01; 32], 0);
    node_coded(Some(vec![log(&addr(&AUTHOR), [0x01; 32], &h)]), vec![(h, tx_json(&raw, &h))], Some(CODE))
}

#[derive(Clone, Default)]
struct Room(std::sync::Arc<std::sync::Mutex<std::collections::BTreeMap<String, Vec<u8>>>>);

impl zikaron_send::checked::Room for Room {
    type Fail = ();
    fn read(&self, file: &str) -> Option<Vec<u8>> {
        self.0.lock().unwrap().get(file).cloned()
    }
    fn put(&self, file: &str, plain: &[u8]) -> Result<(), ()> {
        self.0.lock().unwrap().insert(file.to_string(), plain.to_vec());
        Ok(())
    }
}

#[test]
fn a_second_export_to_an_address_asks_nothing_for_its_key() {
    let (raw, h) = signed(&AUTHOR, [0x01; 32], 0);
    let asked: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
    let at = node_logged(Some(vec![log(&addr(&AUTHOR), [0x01; 32], &h)]), vec![(h, tx_json(&raw, &h))], asked.clone());
    let pinned = zikaron_anchor::deploy::CODE_HASH.to_string();
    let room = Room::default();
    let kept = zikaron_send::checked::Record(room.clone());
    let first = key_for_root(&addr(&AUTHOR), Some(addr(&AUTHOR)), vec![net(vec![at.clone()])], &[], Vec::new(), &pinned, Some(&kept)).expect("found on chain").0;
    assert!(!asked.lock().unwrap().is_empty(), "the first export reads the chain");
    let held = zikaron_send::checked::key_of(&room, &addr(&AUTHOR)).expect("the key kept");
    assert_eq!(zikaron::cryptox::address_of_pubkey(&held), addr(&AUTHOR));
    asked.lock().unwrap().clear();
    let again = key_for_root(&addr(&AUTHOR), Some(addr(&AUTHOR)), vec![net(vec![at.clone()])], &[], Vec::new(), &pinned, Some(&kept)).expect("found kept").0;
    assert_eq!(again, first);
    assert!(asked.lock().unwrap().is_empty(), "the second export asks nothing: {:?}", asked.lock().unwrap());
    let plain = key_for(&addr(&AUTHOR), vec![net(vec![at.clone()])], &[], Vec::new(), &pinned).expect("found on chain");
    assert_eq!(plain, first);
    assert!(!asked.lock().unwrap().is_empty(), "the bare look keeps nothing: the chain is read again");
    asked.lock().unwrap().clear();
    let wrong = Room::default();
    let mut other = held;
    other[64] ^= 1;
    let mut forged = zikaron_send::checked::Held::default();
    forged.keys.insert(addr(&AUTHOR), other);
    zikaron_send::checked::Room::put(&wrong, zikaron_send::checked::FILE, &zikaron_send::checked::render(&forged)).expect("written");
    assert_eq!(zikaron_send::checked::key_of(&wrong, &addr(&AUTHOR)), None, "a key not the address's own is no key");
    let read = key_for_root(&addr(&AUTHOR), Some(addr(&AUTHOR)), vec![net(vec![at])], &[], Vec::new(), &pinned, Some(&zikaron_send::checked::Record(wrong.clone()))).expect("found on chain").0;
    assert_eq!(read, first);
    assert!(!asked.lock().unwrap().is_empty(), "the chain read for it");
    assert_eq!(zikaron_send::checked::key_of(&wrong, &addr(&AUTHOR)), Some(held), "the address's own key kept in its place");
}

#[test]
fn a_plain_folders_export_keeps_the_key_in_the_clear_and_asks_nothing_the_second_time() {
    let (raw, h) = signed(&AUTHOR, [0x01; 32], 0);
    let asked: std::sync::Arc<std::sync::Mutex<Vec<String>>> = Default::default();
    let at = node_logged(Some(vec![log(&addr(&AUTHOR), [0x01; 32], &h)]), vec![(h, tx_json(&raw, &h))], asked.clone());
    let pinned = zikaron_anchor::deploy::CODE_HASH.to_string();
    let machine = std::env::temp_dir().join(format!("zikaron-plain-key-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&machine);
    std::fs::create_dir_all(&machine).expect("the machine folder");
    let export = || {
        let _a = zikaron_anchor::action::open(Some(std::sync::Arc::new(zikaron_send::checked::Logs(zikaron_send::checked::Plain(machine.clone())))));
        let kept = zikaron_send::checked::Record(zikaron_send::checked::Plain(machine.clone()));
        key_for_root(&addr(&AUTHOR), None, vec![net(vec![at.clone()])], &[], Vec::new(), &pinned, Some(&kept)).expect("found").0
    };
    let first = export();
    assert!(asked.lock().unwrap().iter().any(|m| m == "eth_getLogs"), "the first export reads the chain");
    let facts = std::fs::read_to_string(machine.join(zikaron_send::checked::PLAIN_DIR).join(zikaron_send::checked::FILE)).expect("kept in the clear");
    assert!(facts.to_ascii_lowercase().contains(&hex(&addr(&AUTHOR)).to_ascii_lowercase()), "the key kept under its address");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(machine.join(zikaron_send::checked::PLAIN_DIR).join(zikaron_send::checked::FILE)).expect("the file").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "its owner's alone");
    }
    asked.lock().unwrap().clear();
    assert_eq!(export(), first);
    assert!(asked.lock().unwrap().is_empty(), "the second export asks nothing: {:?}", asked.lock().unwrap());
    let _ = std::fs::remove_dir_all(&machine);
}

#[test]
fn a_transaction_that_does_not_prove_itself_is_named_not_unread() {
    let pinned = zikaron_anchor::deploy::CODE_HASH.to_string();
    let (_, h) = signed(&AUTHOR, [0x01; 32], 0);
    let (other_raw, _) = signed(&AUTHOR, [0x09; 32], 7);
    let lying = node(Some(vec![log(&addr(&AUTHOR), [0x01; 32], &h)]), vec![(h, tx_json(&other_raw, &h))]);
    let got = key_for(&addr(&AUTHOR), vec![net(vec![lying])], &[], Vec::new(), &pinned);
    assert!(matches!(&got, Err((Why::Unproven, _))), "named as not proving itself: {got:?}");
    assert!(!Why::Unproven.unanswered(), "a refusal, not the absence of an answer");
    let (raw, h2) = signed(&AUTHOR, [0x02; 32], 1);
    let unsigned = tx_json(&raw, &h2).replace(r#","r":"#, r#","rr":"#);
    let bare = node(Some(vec![log(&addr(&AUTHOR), [0x02; 32], &h2)]), vec![(h2, unsigned)]);
    let got = key_for(&addr(&AUTHOR), vec![net(vec![bare])], &[], Vec::new(), &pinned);
    assert!(matches!(&got, Err((Why::Unproven, _))), "a transaction with no signature to rebuild from: {got:?}");
    let (_, h3) = signed(&AUTHOR, [0x03; 32], 2);
    let none = node(Some(vec![log(&addr(&AUTHOR), [0x03; 32], &h3)]), Vec::new());
    assert!(matches!(key_for(&addr(&AUTHOR), vec![net(vec![none])], &[], Vec::new(), &pinned), Err((Why::ChainUnread, _))), "no transaction given: not read");
}

#[test]
fn a_root_whose_book_does_not_read_is_refused_never_sealed_to_the_key_held_here() {
    const BOOK_CHAIN: u64 = 11_155_111;
    let pinned = zikaron_anchor::deploy::CODE_HASH.to_string();
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("a port");
    let port = l.local_addr().expect("its address").port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let mut b = [0u8; 8192];
            let _ = s.read(&mut b);
            let text = String::from_utf8_lossy(&b).to_string();
            let result = if text.contains("eth_chainId") { format!("\"0x{BOOK_CHAIN:x}\"") } else if text.contains("eth_getCode") { "\"0x6001\"".to_string() } else { "\"0x20\"".to_string() };
            let id = text.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
            let reply = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{result}}}");
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
        }
    });
    let wrong_book = Net { chain: BOOK_CHAIN, registry: REG, from_block: 0, nodes: vec![NodeAddr::new(format!("http://127.0.0.1:{port}"))] };
    let got = key_for_root(&addr(&AUTHOR), Some(addr(&OTHER)), vec![wrong_book], &[], Vec::new(), &pinned, None);
    assert!(matches!(&got, Err((Why::ChainUnread, w)) if w.contains("BOOK_CODE")), "the book not read: refused, not sealed to the key held: {got:?}");
}
