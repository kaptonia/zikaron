//! The key behind an address, for a record package addressed to it (`recipient::key_of`, and `recipient::key_for`, both
//! ends' one judgement with the read-only networks' fingerprint gate, `fingerprint::gate`): found from a transaction
//! the address itself signed (one node is enough, the key proves itself); not anchored only when every node of every
//! network read everything; otherwise not read. Nodes are plain sockets on 127.0.0.1 in this process; no network.
//! Who is hurt by each: the sender (told "not anchored" for an address that is, or sealing to a key that is not the
//! address's), and the recipient (whose package never reaches them).

use std::io::{Read, Write};
use zikaron::json::Value;
use zikaron_send::node::NodeAddr;
use zikaron_send::fingerprint::{gate, Gate, Why as Miss};
use zikaron_send::recipient::{key_for, key_of, Found, Net};
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

/// A registry transaction anchoring `hash`, signed by `key`: its raw bytes and hash.
fn signed(key: &[u8; 32], hash: [u8; 32], nonce: u64) -> (Vec<u8>, [u8; 32]) {
    zikaron_anchor::send::sign_at(key, CHAIN, zikaron_anchor::send::Form::Registry, Some(REG), &[hash], None, zikaron_anchor::send::Fees::fallback(), nonce).expect("signed")
}

/// A signed type-2 transaction as a node answers it by hash.
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

/// One anchor log of the registry: topic 1 the address it claims, its transaction.
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

/// A plain `http` node of `CHAIN`: its head, its logs (`None`: it refuses the log question), its transactions.
fn node(logs: Option<Vec<String>>, txs: Vec<([u8; 32], String)>) -> NodeAddr {
    node_coded(logs, txs, None)
}

/// [`node`], answering the code at the registry (`None`: it refuses the question).
fn node_coded(logs: Option<Vec<String>>, txs: Vec<([u8; 32], String)>, code: Option<&'static str>) -> NodeAddr {
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
            let first = req.member("params").and_then(|p| p.as_arr()).and_then(|a| a.first().and_then(|v| v.as_str().map(str::to_string)));
            let result = match method.as_str() {
                "eth_chainId" => Some(format!("\"0x{CHAIN:x}\"")),
                "eth_blockNumber" => Some("\"0x20\"".into()),
                "eth_getLogs" => logs.as_ref().map(|l| format!("[{}]", l.join(","))),
                "eth_getTransactionByHash" => Some(txs.iter().find(|(h, _)| Some(hex(h)) == first).map(|(_, j)| j.clone()).unwrap_or_else(|| "null".into())),
                "eth_getCode" => code.map(|c| format!("\"{c}\"")),
                _ => None,
            };
            let reply = match result {
                Some(r) => format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{r}}}"),
                None => format!("{{\"error\":{{\"code\":-32000,\"message\":\"refused\"}},\"id\":{id},\"jsonrpc\":\"2.0\"}}"),
            };
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
        }
    });
    NodeAddr::new(format!("http://127.0.0.1:{port}"))
}

fn net(nodes: Vec<NodeAddr>) -> Net {
    Net { chain: CHAIN, registry: REG, from_block: 0, nodes }
}

/// The node every case starts from: the author's own anchor, and one sent through a relay (its log claims another
/// address, its transaction signed by the author).
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
    // The log claims OTHER, the transaction is the author's: a relay's anchor, passed over; every node read.
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
    // A key found proves itself: a node that holds it is enough, whatever another node says.
    assert!(matches!(key_of(&[net(vec![refusing(), holding()])], &addr(&AUTHOR)), Found::Key(_)));
    assert!(matches!(key_of(&[net(vec![empty()]), net(vec![holding()])], &addr(&AUTHOR)), Found::Key(_)));
}

#[test]
fn a_log_whose_transaction_the_node_does_not_give_is_not_read_as_no_anchor() {
    let (_, h) = signed(&AUTHOR, [0x01; 32], 0);
    let half = node(Some(vec![log(&addr(&AUTHOR), [0x01; 32], &h)]), Vec::new());
    assert!(matches!(key_of(&[net(vec![half])], &addr(&AUTHOR)), Found::Unread(_)));
}

/// The code a registry build is in these scenes, and its pin (keccak of it).
const CODE: &str = "0x6001600055";

fn pin() -> String {
    hex(&zikaron::cryptox::keccak256(&zikaron::hexfmt::decode(CODE).expect("hex")))
}

fn urls(nodes: &[NodeAddr]) -> Vec<String> {
    nodes.iter().map(|n| n.for_transport().to_string()).collect()
}

// Who is hurt: the sender, told "not anchored" because a network whose registry is not the pinned build was read
// (it could emit anything), or whose read-only network was never read.
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

// Who is hurt: the sender whose read-only network has a node behind the registry's deployment (it answers no code
// there): the network could be read on its other node, and was closed whole as "another build".
#[test]
fn a_node_answering_no_code_does_not_count_and_closes_nothing() {
    let pinned = pin();
    let coded = || node_coded(Some(Vec::new()), Vec::new(), Some(CODE));
    let behind = || node_coded(Some(Vec::new()), Vec::new(), Some("0x"));
    let (g, counted) = zikaron_send::fingerprint::gate_counted(CHAIN, &REG, &urls(&[coded(), behind()]), &pinned);
    assert_eq!(g, Gate::Single, "the node with the pinned code counts; the one behind does not, and closes nothing");
    assert_eq!(counted.len(), 1, "read on the node that counted alone");
    // Every node answering no code: nothing there is the pinned build on any node asked.
    assert_eq!(gate(CHAIN, &REG, &urls(&[behind(), behind()]), &pinned), Gate::Fingerprint);
    // No code beside a node that did not answer: down, each node said.
    let Gate::Down(misses) = gate(CHAIN, &REG, &[urls(&[behind()])[0].clone(), "http://127.0.0.1:1".to_string()], &pinned) else { panic!("down") };
    assert_eq!(misses[0].why, Miss::NoCode);
    // Found past that gate, read on the node that counted: the network is read, never "not read".
    let nobody = [0x5b; 20];
    let ro = Net { chain: CHAIN, registry: [0x22; 20], from_block: 0, nodes: vec![coded(), behind()] };
    assert_eq!(key_for(&nobody, vec![net(vec![node(Some(Vec::new()), Vec::new())])], &[ro], Vec::new(), &pinned), Err((Why::NotAnchored, String::new())));
}

// Who is hurt: the sender (a package sealed to a key not the address's, or "not anchored" said of a chain not read),
// on whichever end they export.
#[test]
fn both_ends_find_a_key_by_one_judgement() {
    let pinned = pin();
    let author = addr(&AUTHOR);
    let empty_ro = || Net { chain: CHAIN, registry: REG, from_block: 0, nodes: vec![node_coded(Some(Vec::new()), Vec::new(), Some(CODE))] };
    // Found on the main network: compressed, the address's own.
    let k = key_for(&author, vec![net(vec![holding()])], &[empty_ro()], Vec::new(), &pinned).expect("found");
    let full = zikaron_seal::crypto::uncompressed(&k).expect("a point");
    assert_eq!(zikaron::cryptox::address_of_pubkey(&full), author);
    // Every network read through, none holding: not anchored.
    let nobody = [0x5b; 20];
    assert_eq!(key_for(&nobody, vec![net(vec![node(Some(Vec::new()), Vec::new())])], &[empty_ro()], Vec::new(), &pinned), Err((Why::NotAnchored, String::new())));
    // A read-only network of another build is a network not read: never "not anchored".
    let other_build = Net { chain: CHAIN, registry: [0x22; 20], from_block: 0, nodes: vec![node_coded(Some(Vec::new()), Vec::new(), Some("0x00"))] };
    let r = key_for(&nobody, vec![net(vec![node(Some(Vec::new()), Vec::new())])], &[other_build], Vec::new(), &pinned);
    assert!(matches!(&r, Err((Why::ChainUnread, w)) if w.contains("fingerprint")), "{r:?}");
    // No main network (none set) and a read-only network read through: not anchored; a main network that is not
    // there is no network not read.
    assert_eq!(key_for(&nobody, Vec::new(), &[empty_ro()], Vec::new(), &pinned), Err((Why::NotAnchored, String::new())));
    // No network at all: nothing read, never "not anchored".
    let r = key_for(&nobody, Vec::new(), &[], Vec::new(), &pinned);
    assert!(matches!(&r, Err((Why::ChainUnread, w)) if w.contains("NO_NETWORK")), "{r:?}");
    // A read-only network that holds the key is enough, past its gate.
    let ro_holding = Net { chain: CHAIN, registry: REG, from_block: 0, nodes: vec![holding_coded()] };
    assert!(key_for(&author, Vec::new(), &[ro_holding], Vec::new(), &pinned).is_ok());
}

// Who is hurt: the sender whose read-only table still holds a copy of the main network (the table is the machine's,
// the main network the home's): that copy not read was "the chain not read" for an address truly not anchored.
#[test]
fn a_copy_of_the_main_network_in_the_read_only_table_is_the_main_network() {
    let pinned = pin();
    let nobody = [0x5b; 20];
    let main = || net(vec![node(Some(Vec::new()), Vec::new())]);
    // Turned away at its gate (its node not there): it names nothing; the main network read through says it.
    let down = Net { chain: CHAIN, registry: REG, from_block: 0, nodes: vec![NodeAddr::new("http://127.0.0.1:1")] };
    assert_eq!(key_for(&nobody, vec![main()], &[down], Vec::new(), &pinned), Err((Why::NotAnchored, String::new())));
    let other_build = Net { chain: CHAIN, registry: REG, from_block: 0, nodes: vec![node_coded(Some(Vec::new()), Vec::new(), Some("0x00"))] };
    assert_eq!(key_for(&nobody, vec![main()], &[other_build], Vec::new(), &pinned), Err((Why::NotAnchored, String::new())));
    // Past its gate, its node joins the main network's: a key it holds is found.
    let holding_copy = Net { chain: CHAIN, registry: REG, from_block: 0, nodes: vec![holding_coded()] };
    assert!(key_for(&addr(&AUTHOR), vec![main()], &[holding_copy], Vec::new(), &pinned).is_ok());
}

/// [`holding`], answering the pinned code at the registry.
fn holding_coded() -> NodeAddr {
    let (raw, h) = signed(&AUTHOR, [0x01; 32], 0);
    node_coded(Some(vec![log(&addr(&AUTHOR), [0x01; 32], &h)]), vec![(h, tx_json(&raw, &h))], Some(CODE))
}
