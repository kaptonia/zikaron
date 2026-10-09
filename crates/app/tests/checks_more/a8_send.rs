//! Sending a batch end to end (`Action::SendBatch` over a home with work queued, after `Action::EstimateGas`):
//! the balance gate before sending, one signing handed byte for byte to every node with the echo checked, and
//! ambiguous broadcast answers resolved by hash without signing a new nonce.
//!
//! Nodes are fakes on 127.0.0.1 in this process that keep every request, so what was broadcast and what was
//! asked are read where they arrived. Places are set before any vault or shell use (`vault_open`); each test
//! runs alone in its own process ([`super::alone_in`]), and each case uses its own shell, home and nodes. No
//! network.

use super::vault_open;
use app::action::{apply, apply_settled, Action, Applied};
use app::fault::{Fault, Known};
use app::task::{Done, Kind};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ───────────────────────── Nodes ─────────────────────────

/// How a node answers one question.
enum Reply {
    /// `result`, as JSON text.
    Result(String),
    /// `error`, a JSON-RPC error object as JSON text.
    Error(String),
    /// The connection closes with no answer (after the request was read whole).
    Close,
}

/// A JSON-RPC node on a local port answering with `answer(method, body)` and keeping every request body.
#[derive(Clone)]
struct Node {
    url: String,
    log: Arc<Mutex<Vec<String>>>,
}

fn node(answer: impl Fn(&str, &str) -> Reply + Send + Sync + 'static) -> Node {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let log: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let (kept, answer) = (log.clone(), Arc::new(answer));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (log, answer) = (kept.clone(), answer.clone());
            std::thread::spawn(move || {
                let mut s = s;
                let mut raw = Vec::new();
                let mut buf = [0u8; 8192];
                let body = loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => raw.extend_from_slice(&buf[..n]),
                    }
                    let text = String::from_utf8_lossy(&raw).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        let len: usize = text[..i].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|n| n.trim().parse().ok())).unwrap_or(0);
                        if raw.len() >= i + 4 + len {
                            break text[i + 4..].to_string();
                        }
                    }
                };
                log.lock().expect("log").push(body.clone());
                let method = method_of(&body);
                let id = body.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                let reply = match answer(&method, &body) {
                    Reply::Result(v) => format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{v}}}"),
                    Reply::Error(e) => format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{e}}}"),
                    Reply::Close => return,
                };
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
            });
        }
    });
    Node { url, log }
}

fn method_of(body: &str) -> String {
    body.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string()
}

/// The first string parameter of a request (the raw transaction, the hash asked about).
fn first_param(body: &str) -> String {
    body.split("\"params\":[\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string()
}

/// The hash of a raw transaction (`0x` hex), as a node computes it.
fn hash_of(raw_hex: &str) -> String {
    zikaron::hexfmt::encode(&zikaron::cryptox::keccak256(&zikaron::hexfmt::decode(raw_hex).expect("raw hex")))
}

impl Node {
    /// The bodies of the requests asking `method`.
    fn asked(&self, method: &str) -> Vec<String> {
        self.log.lock().expect("log").iter().filter(|b| method_of(b) == method).cloned().collect()
    }

    /// Every raw transaction handed to this node, as sent.
    fn raws(&self) -> Vec<String> {
        self.asked("eth_sendRawTransaction").iter().map(|b| first_param(b)).collect()
    }
}

/// A balance of one ether: more than any batch's fee cap times its gas limit.
const PLENTY: &str = "\"0xde0b6b3a7640000\"";

/// Chain 31337 at height 64, with `balance` as the balance; the fee history has fractional `gasUsedRatio` as
/// real nodes give it; a broadcast echoes the hash of the bytes it took; no transaction is known by hash and
/// there is no receipt yet.
fn chain(method: &str, body: &str, balance: &str) -> Reply {
    Reply::Result(match method {
        "eth_chainId" => "\"0x7a69\"".into(),
        "eth_blockNumber" => "\"0x40\"".into(),
        "eth_getBlockByNumber" => "{\"baseFeePerGas\":\"0x3b9aca00\",\"number\":\"0x40\",\"timestamp\":\"0x64\"}".into(),
        "eth_feeHistory" => "{\"oldestBlock\":\"0x3e\",\"gasUsedRatio\":[0.5,0.25,0.125],\"reward\":[[\"0x5f5e100\"],[\"0x5f5e100\"],[\"0x5f5e100\"]]}".into(),
        "eth_estimateGas" => "\"0xc350\"".into(),
        "eth_getBalance" => balance.into(),
        "eth_getLogs" => "[]".into(),
        "eth_getTransactionCount" => "\"0x5\"".into(),
        "eth_sendRawTransaction" => format!("\"{}\"", hash_of(&first_param(body))),
        _ => "null".into(),
    })
}

/// A transaction a node knows, as `eth_getTransactionByHash` gives it.
fn known_tx(body: &str) -> Reply {
    Reply::Result(format!("{{\"hash\":\"{}\",\"blockNumber\":null,\"from\":\"0x{}\",\"input\":\"0x\"}}", first_param(body), "11".repeat(20)))
}

// ───────────────────────── The bench ─────────────────────────

/// Where this process's homes are made (removed at the end of each test, [`tidy`]).
fn homes() -> std::path::PathBuf {
    std::env::temp_dir().join(format!("zk-a8-send-{}", std::process::id()))
}

/// Remove this process's homes. The shells that wrote them are kept to the end of the test, since a send
/// leaves its receipt wait in flight on its shell.
fn tidy() {
    let _ = std::fs::remove_dir_all(homes());
}

/// A new shell writing its own home with a genesis and one work queued, chain 31337, a registry and `nodes`
/// as its endpoints, no pause between rounds; the batch of one is estimated, so it may be sent.
fn bench(name: &str, nodes: &[&Node]) -> app::shell::Shell {
    vault_open();
    zikaron_anchor::patience::set_waits(Some(Duration::ZERO));
    let dir = homes().join(name);
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    answers!(apply(&mut shell, Action::OpenHome { root: dir.join("home").display().to_string() }), Applied::Homed { .. });
    if shell.anchor.is_none() {
        answers!(apply(&mut shell, Action::MakeAnchorKey), Applied::AnchorKey(_));
    }
    let _ = apply_settled(&mut shell, Action::Genesis { statement: "a8".into() });
    let probe = dir.join("probe.txt");
    std::fs::write(&probe, name.as_bytes()).expect("a file");
    let _ = apply_settled(&mut shell, Action::TakeContent { source: app::anchorx::Source::File, path: probe.display().to_string() });
    let _ = apply_settled(&mut shell, Action::RecordWork { note_md: String::new(), files: Vec::new(), for_: None });
    shell.settings.chain_id = Some(31337);
    shell.settings.registry = Some(app::key::Address([0x11; 20]));
    shell.send_backoff = Vec::new();
    shell.receipt_backoff = Vec::new();
    point(&mut shell, name, nodes);
    shell
}

/// Make `nodes` the shell's endpoints and estimate the batch of one over them, so it may be sent.
fn point(shell: &mut app::shell::Shell, name: &str, nodes: &[&Node]) {
    shell.endpoints = nodes.iter().map(|n| app::chainx::Endpoint::at(31337, n.url.clone())).collect();
    match apply_settled(shell, Action::EstimateGas { count: 1 }) {
        Applied::Gas { count: 1, .. } => {}
        other => panic!("{name}: the batch was not estimated: {other:?}"),
    }
    // The fee reading used the chain's numbers, its tip read past the fee history's fractions.
    let fees = shell.fees.expect("fees read with the estimate");
    assert!(fees.from_chain && fees.priority == 0x5f5e100, "{name}: {fees:?}");
}

/// Press send for the batch of one and return the send's own landing (the first `Anchor` outcome).
fn send(shell: &mut app::shell::Shell) -> Result<Done, Fault> {
    assert_eq!(apply(shell, Action::SendBatch { count: 1 }), Applied::Started(Kind::Anchor));
    let until = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(o) = shell.drain().into_iter().find(|o| o.kind == Kind::Anchor) {
            return o.result;
        }
        assert!(Instant::now() < until, "the send pass never landed");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Nothing was signed or broadcast at any of `nodes`: no nonce asked, no raw transaction handed over.
fn nothing_broadcast(form: &str, nodes: &[&Node]) {
    for n in nodes {
        assert!(n.asked("eth_sendRawTransaction").is_empty(), "{form}: nothing broadcast at {}", n.url);
        assert!(n.asked("eth_getTransactionCount").is_empty(), "{form}: no nonce asked at {}", n.url);
    }
}

/// One signing over `nodes`: the pending nonce asked once of each, and every raw transaction any of them got
/// is the same bytes. Returns those bytes (`0x` hex).
fn signed_once(form: &str, nodes: &[&Node]) -> String {
    for n in nodes {
        assert_eq!(n.asked("eth_getTransactionCount").len(), 1, "{form}: the nonce asked once, one round, at {}", n.url);
    }
    let mut raws: Vec<String> = nodes.iter().flat_map(|n| n.raws()).collect();
    assert!(!raws.is_empty(), "{form}: something was handed over");
    raws.dedup();
    assert_eq!(raws.len(), 1, "{form}: one signing, the same bytes everywhere: {raws:?}");
    raws.remove(0)
}

// ───────────────────────── The tests ─────────────────────────

/// The pre-send balance gate refuses too little as `INSUFFICIENT_FUNDS` (with need and balance) and refuses
/// an unreadable or disputed balance by its own name; in every refused case nothing is signed or broadcast.
#[test]
fn the_balance_gate_refuses_by_name_and_nothing_is_broadcast() {
    if super::alone_in(module_path!(), "the_balance_gate_refuses_by_name_and_nothing_is_broadcast") {
        return;
    }
    let with = |balance: &'static str| node(move |m, b| chain(m, b, balance));
    let mut kept = Vec::new();
    for form in ["short", "notAQuantity", "null", "refused", "closed", "differ", "enough"] {
        let (nodes, want): (Vec<Node>, Option<Known>) = match form {
            "short" => (vec![with("\"0x1\"")], Some(Known::InsufficientFunds)),
            "notAQuantity" => (vec![with("\"0xzz\"")], Some(Known::ChainShape)),
            "null" => (vec![with("null")], Some(Known::ChainShape)),
            "refused" => (
                vec![node(|m, b| match m {
                    "eth_getBalance" => Reply::Error("{\"code\":-32000,\"message\":\"header not found\"}".into()),
                    _ => chain(m, b, PLENTY),
                })],
                None,
            ),
            "closed" => (
                vec![node(|m, b| match m {
                    "eth_getBalance" => Reply::Close,
                    _ => chain(m, b, PLENTY),
                })],
                None,
            ),
            "differ" => (vec![with(PLENTY), with("\"0xde0b6b3a7640001\"")], Some(Known::Disagree)),
            "enough" => (vec![with(PLENTY)], None),
            other => panic!("no form {other}"),
        };
        let refs: Vec<&Node> = nodes.iter().collect();
        let mut shell = bench(form, &refs);
        let need = shell.fees.expect("fees").cap_wei();
        let got = send(&mut shell);
        kept.push(shell);
        if form == "enough" {
            assert!(matches!(got, Ok(Done::Submitted { .. })), "{form}: a balance that covers it is sent: {got:?}");
            signed_once(form, &refs);
            continue;
        }
        let f = got.expect_err("refused before sending");
        assert!(f.which().is_some(), "{form}: refused by name: {f:?}");
        match want {
            Some(k) => assert_eq!(f.which(), Some(k), "{form}: {f:?}"),
            None => assert_ne!(f.which(), Some(Known::InsufficientFunds), "{form}: an unread balance is not guessed: {f:?}"),
        }
        if form == "short" {
            assert_eq!(app::action::funds_of(f.tail()), Some((need, 1)), "{form}: both numbers: {}", f.tail());
        }
        nothing_broadcast(form, &refs);
    }
    tidy();
}

/// One signing goes byte for byte to every node, and only a node whose echoed hash matches counts as having
/// taken it; a node that only echoes wrong hashes never counts, and nothing is signed again.
#[test]
fn one_signing_the_same_bytes_to_every_node_and_the_echo_must_match() {
    if super::alone_in(module_path!(), "one_signing_the_same_bytes_to_every_node_and_the_echo_must_match") {
        return;
    }
    let wrong_echo = || {
        node(|m, b| match m {
            "eth_sendRawTransaction" => Reply::Result(format!("\"0x{}\"", "ab".repeat(32))),
            _ => chain(m, b, PLENTY),
        })
    };
    // Three nodes: the first closes without answering, the second echoes a wrong hash, the third's echo matches.
    let form = "third";
    let silent = node(|m, b| match m {
        "eth_sendRawTransaction" => Reply::Close,
        _ => chain(m, b, PLENTY),
    });
    let wrong = wrong_echo();
    let right = node(|m, b| chain(m, b, PLENTY));
    let refs = [&silent, &wrong, &right];
    let mut shell = bench(form, &refs);
    let got = send(&mut shell);
    let raw = signed_once(form, &refs);
    for n in refs {
        assert_eq!(n.raws(), vec![raw.clone()], "{form}: handed once, the same bytes, at {}", n.url);
    }
    match got {
        Ok(Done::Submitted { tx, url, .. }) => {
            assert_eq!(url, app::chainx::NodeAddr::new(right.url.clone()), "{form}: the node whose echo matched took it");
            assert_eq!(tx, hash_of(&raw), "{form}: the hash of the bytes handed over");
        }
        other => panic!("{form}: sent: {other:?}"),
    }
    // One node, only ever a wrong echo.
    let form = "wrongEchoOnly";
    let wrong = wrong_echo();
    let mut other_shell = bench(form, &[&wrong]);
    let got = send(&mut other_shell);
    let raw = signed_once(form, &[&wrong]);
    assert_eq!(wrong.raws().len(), 1, "{form}: handed once");
    let f = got.expect_err("a wrong echo is not received");
    assert!(f.which().is_some() && f.which() != Some(Known::InsufficientFunds), "{form}: refused by name: {f:?}");
    // Whether the bytes went out was asked by the signed hash, never by the echo.
    let asked: Vec<String> = wrong.asked("eth_getTransactionByHash").iter().map(|b| first_param(b)).collect();
    assert!(!asked.is_empty() && asked.iter().all(|h| *h == hash_of(&raw)), "{form}: asked by the signed hash: {asked:?}");
    drop((shell, other_shell));
    tidy();
}

/// "Already known", "nonce too low" and a connection closed after the bytes were taken are each resolved by
/// asking for the signed hash: known counts as taken, unknown is refused by name; no new nonce is signed.
#[test]
fn already_taken_answers_are_asked_by_hash_and_no_new_nonce_is_signed() {
    if super::alone_in(module_path!(), "already_taken_answers_are_asked_by_hash_and_no_new_nonce_is_signed") {
        return;
    }
    let mut kept = Vec::new();
    for (form, said, refused) in [
        ("pending", (|| Reply::Error("{\"code\":-32000,\"message\":\"already known\"}".into())) as fn() -> Reply, Known::AlreadyPending),
        ("nonceUsed", || Reply::Error("{\"code\":-32000,\"message\":\"nonce too low\"}".into()), Known::NonceUsed),
        ("noAnswer", || Reply::Close, Known::Unreachable),
    ] {
        for known in [true, false] {
            let form = format!("{form}{}", if known { "Known" } else { "Unknown" });
            let n = node(move |m, b| match m {
                "eth_sendRawTransaction" => said(),
                "eth_getTransactionByHash" if known => known_tx(b),
                _ => chain(m, b, PLENTY),
            });
            let mut shell = bench(&form, &[&n]);
            let got = send(&mut shell);
            kept.push(shell);
            let raw = signed_once(&form, &[&n]);
            assert_eq!(n.raws().len(), 1, "{form}: handed over once");
            let hash = hash_of(&raw);
            let asked: Vec<String> = n.asked("eth_getTransactionByHash").iter().map(|b| first_param(b)).collect();
            assert!(!asked.is_empty(), "{form}: asked about by hash");
            assert!(asked.iter().all(|h| *h == hash), "{form}: by the hash signed: {asked:?} against {hash}");
            if known {
                match got {
                    Ok(Done::Submitted { tx, url, .. }) => assert_eq!((tx, url), (hash, app::chainx::NodeAddr::new(n.url.clone())), "{form}: taken"),
                    other => panic!("{form}: a transaction the node knows was taken: {other:?}"),
                }
            } else {
                let f = got.expect_err("not known: refused");
                assert_eq!(f.which(), Some(refused), "{form}: {f:?}");
            }
        }
    }
    tidy();
}
