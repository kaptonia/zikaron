//! The direct `anchor` (no `--home`) records what it sent (`zikaron_cli::sent`, under the user's home) and on
//! a rerun asks the node about those hashes before signing anything new. Real-binary runs against an in-process
//! node; runs of one scenario share a user home. A refused broadcast answers with the recorded hash. On rerun:
//! a pending transaction answers "not yet", a receipt answers included, an unreadable answer answers unreadable,
//! all with that hash and nothing sent. Unknown to the node with its nonce used, it is void (`E_TX_VOID`, exit
//! 1, nothing sent) and the next run sends afresh; unknown with its nonce unused, it is re-signed at that nonce.
//! If the hash cannot be recorded nothing is sent; recorded hashes are never removed.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
const KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
const HASH: [u8; 32] = [0x77; 32];

/// The node's state for this run.
#[derive(Clone, Default)]
struct Scene {
    /// Refuse the broadcast (as for insufficient funds).
    refuse: bool,
    /// Answer to a receipt query (`null` when none).
    receipt: Option<String>,
    /// Whether it knows the transactions it is asked about.
    holds: bool,
    /// The account's nonce: `latest` from blocks, `pending` including the mempool.
    latest: u64,
    pending: u64,
}

struct Node {
    url: String,
    scene: Arc<Mutex<Scene>>,
    asked: Arc<Mutex<Vec<String>>>,
    raws: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl Node {
    fn set(&self, s: Scene) {
        *self.scene.lock().unwrap() = s;
        self.asked.lock().unwrap().clear();
    }
    fn asked(&self, m: &str) -> usize {
        self.asked.lock().unwrap().iter().filter(|x| *x == m).count()
    }
    fn broadcasts(&self) -> usize {
        self.raws.lock().unwrap().len()
    }
}

fn node() -> Node {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let scene: Arc<Mutex<Scene>> = Arc::default();
    let asked: Arc<Mutex<Vec<String>>> = Arc::default();
    let raws: Arc<Mutex<Vec<Vec<u8>>>> = Arc::default();
    let (sc, a, r) = (scene.clone(), asked.clone(), raws.clone());
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (sc, a, r) = (sc.clone(), a.clone(), r.clone());
            std::thread::spawn(move || {
                let mut s = s;
                let mut raw = Vec::new();
                let mut buf = [0u8; 4096];
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
                let req = zikaron::json::parse(body.as_bytes()).unwrap_or(zikaron::json::Value::Null);
                let method = req.member("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
                let params: Vec<zikaron::json::Value> = match req.member("params") {
                    Some(zikaron::json::Value::Arr(p)) => p.clone(),
                    _ => Vec::new(),
                };
                let id = body.split("\"id\":").nth(1).and_then(|x| x.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                a.lock().unwrap().push(method.clone());
                let scene = sc.lock().unwrap().clone();
                let result: Result<String, String> = match method.as_str() {
                    "eth_blockNumber" => Ok("\"0x40\"".into()),
                    "eth_getBlockByNumber" => Ok("{\"baseFeePerGas\":\"0x3b9aca00\",\"number\":\"0x40\",\"timestamp\":\"0x64\"}".into()),
                    "eth_feeHistory" => Ok("{\"oldestBlock\":\"0x2d\",\"reward\":[[\"0x5f5e100\"]]}".into()),
                    "eth_estimateGas" => Ok("\"0x7530\"".into()),
                    "eth_getTransactionCount" => {
                        let n = if params.get(1).and_then(|x| x.as_str()) == Some("latest") { scene.latest } else { scene.pending };
                        Ok(format!("\"0x{n:x}\""))
                    }
                    "eth_sendRawTransaction" => {
                        let bytes = params.first().and_then(|x| x.as_str()).and_then(zikaron::hexfmt::decode).unwrap_or_default();
                        let h = zikaron::cryptox::keccak256(&bytes);
                        r.lock().unwrap().push(bytes);
                        if scene.refuse {
                            Err("{\"code\":-32000,\"message\":\"insufficient funds for gas * price + value\"}".into())
                        } else {
                            Ok(format!("\"{}\"", zikaron::hexfmt::encode(&h)))
                        }
                    }
                    "eth_getTransactionReceipt" => Ok(scene.receipt.clone().unwrap_or_else(|| "null".into())),
                    "eth_getTransactionByHash" if scene.holds => {
                        let h = params.first().and_then(|x| x.as_str()).unwrap_or_default().to_string();
                        Ok(format!("{{\"hash\":\"{h}\",\"nonce\":\"0x0\"}}"))
                    }
                    _ => Ok("null".into()),
                };
                let body = match result {
                    Ok(v) => format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{v}}}"),
                    Err(e) => format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{e}}}"),
                };
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    Node { url, scene, asked, raws }
}

struct Ran {
    code: i32,
    out: zikaron::json::Value,
    err: String,
}

impl Ran {
    fn text(&self, k: &str) -> String {
        self.out.member(k).and_then(|v| v.as_str()).unwrap_or("").to_string()
    }
}

fn home(tag: &str) -> PathBuf {
    let h = std::env::temp_dir().join(format!("zk-cli-rerun-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&h);
    std::fs::create_dir_all(&h).expect("a home");
    h
}

/// `anchor` with `home` set through the system's home variable (`zikaron_os::HOME_VAR`), so each run's record
/// lives under its own home on every system.
fn anchor(home: &Path, n: &Node) -> Ran {
    anchor_where(n, |c| {
        c.env(zikaron_os::HOME_VAR, home);
    })
}

/// `anchor` with the home variable set (or unset) by `set`.
fn anchor_where(n: &Node, set: impl FnOnce(&mut Command)) -> Ran {
    let mut c = Command::new(BIN);
    c.args(["anchor", "--endpoint", &format!("31337={}", n.url), "--key", KEY, "--form", "bare", "--hash", &zikaron::hexfmt::encode(&HASH), "--wait-secs", "0"]);
    set(&mut c);
    let o = c.output().expect("zikaron runs");
    Ran { code: o.status.code().unwrap_or(-1), out: zikaron::json::parse(&o.stdout).unwrap_or(zikaron::json::Value::Null), err: String::from_utf8_lossy(&o.stderr).to_string() }
}

/// Where this anchoring's record lives under `home` (`sent::place`).
fn record(home: &Path) -> PathBuf {
    let key = zikaron::hexfmt::scalar32(KEY).expect("a key");
    let from = zikaron::cryptox::address_of_privkey(&key).expect("an address");
    let (machine, _) = zikaron_os::machine::of(home).expect("the machine folder");
    zikaron_cli::sent::place(&machine, 31337, &from, &from, &HASH)
}

fn hex(raw: &[u8]) -> String {
    zikaron::hexfmt::encode(&zikaron::cryptox::keccak256(raw))
}

#[test]
fn a_rerun_asks_about_what_was_sent_before_signing_anything_new() {
    let h = home("story");
    let n = node();
    // A refused broadcast: unanswered, with the hash recorded before broadcasting.
    n.set(Scene { refuse: true, ..Scene::default() });
    let r = anchor(&h, &n);
    assert_eq!((r.code, r.text("reason")), (4, "E_UNREACHABLE".to_string()), "{}", r.err);
    let first = hex(&n.raws.lock().unwrap()[0]);
    assert_eq!(r.text("tx"), first, "the broadcast's trouble carries the hash");
    assert_eq!(zikaron_cli::sent::read(&record(&h)).expect("the record").len(), 1, "landed");
    // Known to the node, no receipt: in flight, with that hash; nothing new signed or sent.
    n.set(Scene { holds: true, ..Scene::default() });
    let r = anchor(&h, &n);
    assert_eq!((r.code, r.text("reason"), r.text("tx")), (4, "E_TX_NOT_YET".to_string(), first.clone()), "in the pool");
    assert_eq!((n.broadcasts(), n.asked("eth_estimateGas"), n.asked("eth_getTransactionCount")), (1, 0, 0), "in the pool: nothing new");
    // A receipt: included, with that hash.
    n.set(Scene { receipt: Some("{\"blockNumber\":\"0x9\",\"status\":\"0x1\"}".into()), ..Scene::default() });
    let r = anchor(&h, &n);
    assert_eq!((r.code, r.text("tx")), (0, first.clone()), "in the block");
    assert_eq!(r.out.member("blockNumber"), Some(&zikaron::json::Value::Int(9)));
    assert_eq!(n.broadcasts(), 1, "in the block: nothing new");
    // An answer of unexpected shape: unanswered, with that hash; nothing new.
    n.set(Scene { receipt: Some("\"0x1\"".into()), ..Scene::default() });
    let r = anchor(&h, &n);
    assert_eq!((r.code, r.text("reason"), r.text("tx")), (4, "E_UNREACHABLE".to_string(), first.clone()), "another shape");
    assert_eq!(n.broadcasts(), 1, "another shape: nothing new");
    // Unknown and its nonce used: void on stdout (exit 1, its hash), nothing sent; the record marks it void.
    n.set(Scene { latest: 1, pending: 1, ..Scene::default() });
    let r = anchor(&h, &n);
    assert_eq!((r.code, r.text("reason"), r.text("tx")), (1, "E_TX_VOID".to_string(), first.clone()), "void: {}", r.err);
    assert_eq!(n.broadcasts(), 1, "void: nothing sent now");
    assert_eq!(zikaron_cli::sent::read(&record(&h)).expect("the record"), vec![], "the void one no longer waited on");
    // Rerun after the void: sent afresh at the current nonce, answered with the new hash.
    n.set(Scene { latest: 1, pending: 1, ..Scene::default() });
    let r = anchor(&h, &n);
    assert_eq!(n.broadcasts(), 2, "sent afresh");
    let second = hex(&n.raws.lock().unwrap()[1]);
    assert_ne!(second, first);
    assert_eq!((r.code, r.text("reason"), r.text("tx")), (4, "E_TX_NOT_YET".to_string(), second.clone()));
    let mut tx2 = [0u8; 32];
    tx2.copy_from_slice(&zikaron::hexfmt::decode(&second).unwrap());
    assert_eq!(zikaron_cli::sent::read(&record(&h)).expect("the record"), vec![(1, tx2)], "the new one at the nonce now");
    let _ = std::fs::remove_dir_all(&h);
}

/// After a void, a refused fresh send is unanswered with the new hash, which the next run asks about first.
#[test]
fn a_void_then_a_send_refused_again_answers_with_the_new_hash() {
    let h = home("void-again");
    let n = node();
    n.set(Scene { refuse: true, ..Scene::default() });
    let _ = anchor(&h, &n);
    n.set(Scene { latest: 1, pending: 1, ..Scene::default() });
    assert_eq!(anchor(&h, &n).text("reason"), "E_TX_VOID");
    n.set(Scene { refuse: true, latest: 1, pending: 1, ..Scene::default() });
    let r = anchor(&h, &n);
    let second = hex(&n.raws.lock().unwrap()[1]);
    assert_eq!((r.code, r.text("reason"), r.text("tx")), (4, "E_UNREACHABLE".to_string(), second), "{}", r.err);
    assert_eq!(zikaron_cli::sent::read(&record(&h)).expect("the record").len(), 1);
    let _ = std::fs::remove_dir_all(&h);
}

/// Two sends at one nonce (the second re-signed at it), one of them mined: included with that hash; nothing
/// void, nothing new.
#[test]
fn of_two_sent_at_one_nonce_the_one_in_a_block_is_the_answer() {
    let h = home("one-included");
    let n = node();
    n.set(Scene { refuse: true, ..Scene::default() });
    let _ = anchor(&h, &n);
    n.set(Scene { latest: 0, pending: 0, ..Scene::default() });
    let _ = anchor(&h, &n);
    assert_eq!(zikaron_cli::sent::read(&record(&h)).expect("the record").len(), 2);
    n.set(Scene { receipt: Some("{\"blockNumber\":\"0x7\",\"status\":\"0x1\"}".into()), latest: 1, pending: 1, ..Scene::default() });
    let r = anchor(&h, &n);
    let first = hex(&n.raws.lock().unwrap()[0]);
    assert_eq!((r.code, r.text("tx")), (0, first));
    assert_eq!(n.broadcasts(), 2, "nothing new");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn held_by_no_node_with_its_nonce_unused_it_is_signed_again_at_that_nonce() {
    let h = home("unused");
    let n = node();
    n.set(Scene { refuse: true, ..Scene::default() });
    let _ = anchor(&h, &n);
    // The pending nonce moved on (another transaction from this account) but the latest did not: re-sign at
    // the recorded nonce, never the pending one.
    n.set(Scene { latest: 0, pending: 5, ..Scene::default() });
    let r = anchor(&h, &n);
    assert_eq!(n.broadcasts(), 2, "{}", r.err);
    let got = zikaron_cli::sent::read(&record(&h)).expect("the record");
    assert_eq!(got.iter().map(|(nonce, _)| *nonce).collect::<Vec<_>>(), vec![0, 0], "both at nonce zero");
    let _ = std::fs::remove_dir_all(&h);
}

/// A read-only record folder: recording the hash fails.
#[cfg(unix)]
#[allow(clippy::permissions_set_readonly_false)]
#[test]
fn a_hash_that_cannot_be_landed_sends_nothing() {
    let h = home("unlanded");
    let n = node();
    let at = record(&h);
    std::fs::create_dir_all(&at).expect("the record's folder");
    let mut p = std::fs::metadata(&at).expect("meta").permissions();
    p.set_readonly(true);
    std::fs::set_permissions(&at, p.clone()).expect("read-only");
    let r = anchor(&h, &n);
    p.set_readonly(false);
    std::fs::set_permissions(&at, p).expect("back");
    assert_eq!((r.code, r.text("reason"), r.text("tx")), (4, "E_UNREACHABLE".to_string(), String::new()), "{}", r.err);
    assert_eq!(n.broadcasts(), 0, "nothing broadcast");
    let _ = std::fs::remove_dir_all(&h);
}

#[test]
fn a_record_that_does_not_read_signs_nothing() {
    let n = node();
    for (form, name, bytes) in [("notJson", "0", &b"not json"[..]), ("notNumbered", "x", &b"{}"[..]), ("noTx", "0", &b"{\"nonce\":\"0x0\"}"[..]), ("nonceNotAQuantity", "0", &b"{\"nonce\":7,\"tx\":\"0x00\"}"[..])] {
        let h = home(form);
        let at = record(&h);
        std::fs::create_dir_all(&at).expect("the record's folder");
        std::fs::write(at.join(name), bytes).expect("a record that does not read");
        let r = anchor(&h, &n);
        assert_eq!((r.code, r.text("reason"), r.text("tx")), (4, "E_UNREACHABLE".to_string(), String::new()), "{form}: {}", r.err);
        assert!(r.text("detail").contains(&at.display().to_string()), "{form}: the record named: {}", r.text("detail"));
        assert_eq!(n.broadcasts(), 0, "{form}: nothing signed or sent");
        let _ = std::fs::remove_dir_all(&h);
    }
}

#[test]
fn a_machine_folder_that_does_not_read_signs_nothing() {
    let h = home("pointer");
    let n = node();
    let ptr = zikaron_os::machine::pointer_dir(&h).join(zikaron_os::machine::POINTER);
    std::fs::create_dir_all(ptr.parent().expect("a parent")).expect("the pointer's folder");
    std::fs::write(&ptr, "not an absolute path\n").expect("a pointer that does not read");
    let r = anchor(&h, &n);
    assert_eq!((r.code, r.text("reason"), r.text("tx")), (4, "E_UNREACHABLE".to_string(), String::new()), "{}", r.err);
    assert_eq!(n.broadcasts(), 0, "nothing signed or sent");
    let _ = std::fs::remove_dir_all(&h);
}

/// Any u64 nonce, including zero and `u64::MAX`, is recorded and read back unchanged.
#[test]
fn a_record_keeps_any_nonce() {
    let h = home("nonces");
    let at = h.join("record");
    zikaron_cli::sent::land(&at, 0, &[1u8; 32]).expect("landed");
    zikaron_cli::sent::land(&at, u64::MAX, &[2u8; 32]).expect("landed");
    assert_eq!(zikaron_cli::sent::read(&at).expect("read"), vec![(0, [1u8; 32]), (u64::MAX, [2u8; 32])]);
    let _ = std::fs::remove_dir_all(&h);
}

/// A transaction mined with a status other than 1 will never anchor: the rerun reports it (`E_TX_STATUS`, its
/// hash, exit 1, nothing sent) and marks it void, so the next run sends afresh instead of repeating the failure.
#[test]
fn a_reverted_inclusion_is_said_once_and_the_next_run_sends_afresh() {
    let h = home("reverted");
    let n = node();
    n.set(Scene { refuse: true, ..Scene::default() });
    let _ = anchor(&h, &n);
    let first = hex(&n.raws.lock().unwrap()[0]);
    n.set(Scene { receipt: Some("{\"blockNumber\":\"0x7\",\"status\":\"0x0\"}".into()), latest: 1, pending: 1, ..Scene::default() });
    let r = anchor(&h, &n);
    assert_eq!((r.code, r.text("reason"), r.text("tx")), (1, "E_TX_STATUS".to_string(), first), "{}", r.err);
    assert_eq!(n.broadcasts(), 1, "nothing sent on the run that found it");
    assert!(zikaron_cli::sent::read(&record(&h)).expect("the record").is_empty(), "marked void");
    n.set(Scene { latest: 1, pending: 1, ..Scene::default() });
    let r = anchor(&h, &n);
    assert_eq!(n.broadcasts(), 2, "sent afresh: {}", r.err);
    let _ = std::fs::remove_dir_all(&h);
}

/// A hidden file left in the record folder by a file browser or sync tool (`.DS_Store`) is skipped; a
/// non-record name that is not hidden is still refused (see above).
#[test]
fn a_hidden_file_beside_the_record_is_not_a_record() {
    let h = home("hidden");
    let n = node();
    let at = record(&h);
    std::fs::create_dir_all(&at).expect("the record's folder");
    std::fs::write(at.join(".DS_Store"), b"\0\0\0\x01Bud1").expect("a hidden file");
    n.set(Scene { refuse: true, ..Scene::default() });
    let r = anchor(&h, &n);
    assert_eq!(n.broadcasts(), 1, "signed and sent: {}", r.err);
    assert_eq!(zikaron_cli::sent::read(&at).expect("the record reads").len(), 1);
    let _ = std::fs::remove_dir_all(&h);
}

/// Each home set through the home variable keeps its own record: a missing home is created and holds the
/// record; a sibling home holds none, and its own run starts from nothing.
#[test]
fn a_home_handed_through_the_systems_variable_keeps_its_record_apart() {
    let h = home("apart");
    let fresh = h.join("not-there-yet");
    let n = node();
    n.set(Scene { refuse: true, ..Scene::default() });
    let r = anchor(&fresh, &n);
    assert_eq!(n.broadcasts(), 1, "signed and sent: {}", r.err);
    assert_eq!(zikaron_cli::sent::read(&record(&fresh)).expect("the record reads").len(), 1, "recorded under the home it was handed");
    let other = home("apart-other");
    assert!(!record(&other).exists(), "nothing under another home");
    let r = anchor(&other, &n);
    assert_eq!(n.broadcasts(), 2, "the other home starts from nothing and sends: {}", r.err);
    assert_eq!(n.asked("eth_getTransactionByHash"), 0, "and asks about nothing it did not send");
    let _ = std::fs::remove_dir_all(&h);
    let _ = std::fs::remove_dir_all(&other);
}

/// A home that cannot be written (a read-only folder) records nothing and so sends nothing.
#[cfg(unix)]
#[allow(clippy::permissions_set_readonly_false)]
#[test]
fn a_home_that_cannot_be_written_sends_nothing() {
    let h = home("unwritable");
    let n = node();
    let mut p = std::fs::metadata(&h).expect("meta").permissions();
    p.set_readonly(true);
    std::fs::set_permissions(&h, p.clone()).expect("read-only");
    let r = anchor(&h, &n);
    p.set_readonly(false);
    std::fs::set_permissions(&h, p).expect("back");
    assert_eq!((r.code, r.text("reason"), r.text("tx")), (4, "E_UNREACHABLE".to_string(), String::new()), "{}", r.err);
    assert_eq!(n.broadcasts(), 0, "nothing broadcast");
    let _ = std::fs::remove_dir_all(&h);
}

/// No home at all (the variable unset or empty, with no other fallback on unix): nothing can be recorded, so
/// nothing is signed or sent. Unix only: on Windows the profile known folder is used, which is the user's real
/// one.
#[cfg(unix)]
#[test]
fn no_home_sends_nothing() {
    let n = node();
    for (form, r) in [
        ("unset", anchor_where(&n, |c| {
            c.env_remove(zikaron_os::HOME_VAR);
        })),
        ("empty", anchor_where(&n, |c| {
            c.env(zikaron_os::HOME_VAR, "");
        })),
    ] {
        assert_eq!((r.code, r.text("reason"), r.text("tx")), (4, "E_UNREACHABLE".to_string(), String::new()), "{form}: {}", r.err);
    }
    assert_eq!(n.broadcasts(), 0, "nothing broadcast");
}
