//! `anchor`'s exit for a refused gas estimate depends on the shape of the node's error, classified by the
//! single table `said::refuses_the_call`: a refusal about the call is `E_GAS_REFUSED`, exit 1; one that says
//! nothing about the call is `E_UNREACHABLE`, exit 4. Neither asks for a nonce or broadcasts. One real-binary
//! test per table entry, against an in-process node that answers fee and head queries normally and the estimate
//! as the case requires.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::{Arc, Mutex};

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
const KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

/// How the node answers one request: a status and a body (with `{id}` substituted), or a closed connection.
#[derive(Clone)]
enum Reply {
    Say(u16, String),
    Close,
}

fn ok(result: &str) -> Reply {
    Reply::Say(200, format!("{{\"jsonrpc\":\"2.0\",\"id\":{{id}},\"result\":{result}}}"))
}

fn error(status: u16, err: &str) -> Reply {
    Reply::Say(status, format!("{{\"jsonrpc\":\"2.0\",\"id\":{{id}},\"error\":{err}}}"))
}

/// A node that answers fee queries normally, `head` for the block-number query after the fees, and `estimate`
/// for `eth_estimateGas`; records every method asked.
fn node(head: Reply, estimate: Reply) -> (String, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let asked: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let a = asked.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (a, head, estimate) = (a.clone(), head.clone(), estimate.clone());
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
                let method = body.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                let id = body.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                let heads_before = {
                    let mut g = a.lock().unwrap_or_else(|e| e.into_inner());
                    g.push(method.clone());
                    g.iter().filter(|m| *m == "eth_blockNumber").count()
                };
                let reply = match method.as_str() {
                    // The first head query is the fee lookup's (fees are read first); the second the estimate's.
                    "eth_blockNumber" if heads_before > 1 => head,
                    "eth_blockNumber" => ok("\"0x40\""),
                    "eth_getBlockByNumber" => ok("{\"baseFeePerGas\":\"0x3b9aca00\",\"number\":\"0x40\",\"timestamp\":\"0x64\"}"),
                    "eth_feeHistory" => ok("{\"oldestBlock\":\"0x2d\",\"reward\":[[\"0x5f5e100\"]]}"),
                    "eth_estimateGas" => estimate,
                    "eth_getTransactionCount" => ok("\"0x0\""),
                    _ => ok("null"),
                };
                if let Reply::Say(status, body) = reply {
                    let body = body.replace("{id}", &id);
                    let _ = s.write_all(format!("HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
                }
            });
        }
    });
    (url, asked)
}

/// A separate user home per run: `anchor` records what it sent under the user's home (`zikaron_cli::sent`), so
/// runs never share a record or touch the real one.
fn own_home() -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!("zk-cli-home-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)))
}

/// `anchor` against that node: exit code, the reason on stdout, and whether a nonce or broadcast was requested.
fn anchor_with(head: Reply, estimate: Reply) -> (i32, String, bool) {
    let (url, asked) = node(head, estimate);
    let hash = format!("0x{}", "77".repeat(32));
    let o = Command::new(BIN)
        .args(["anchor", "--endpoint", &format!("31337={url}"), "--key", KEY, "--form", "bare", "--hash", &hash, "--wait-secs", "0"])
        .env(zikaron_os::HOME_VAR, own_home())
        .output()
        .expect("zikaron runs");
    let out = String::from_utf8_lossy(&o.stdout).to_string();
    let sent = asked.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|m| m == "eth_sendRawTransaction" || m == "eth_getTransactionCount");
    (o.status.code().unwrap_or(-1), out, sent)
}

fn estimate_says(estimate: Reply) -> (i32, String, bool) {
    anchor_with(ok("\"0x40\""), estimate)
}

/// About the call: exit 1, `E_GAS_REFUSED`, nothing sent.
fn the_calls(got: (i32, String, bool)) {
    let (code, out, sent) = got;
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("\"reason\":\"E_GAS_REFUSED\""), "{out}");
    assert!(!sent, "nothing sent: {out}");
}

/// Not about the call: exit 4, `E_UNREACHABLE`, nothing sent.
fn not_the_calls(got: (i32, String, bool)) {
    let (code, out, sent) = got;
    assert_eq!(code, 4, "{out}");
    assert!(out.contains("\"reason\":\"E_UNREACHABLE\""), "{out}");
    assert!(!sent, "nothing sent: {out}");
}

fn coded(code: &str, words: &str) -> Reply {
    error(200, &format!("{{\"code\":{code},\"message\":\"{words}\"}}"))
}

// ───────────────────────── The marker sentences ─────────────────────────

macro_rules! marked {
    ($($name:ident: $said:literal => $family:ident;)*) => {$(
        #[test]
        fn $name() {
            $family(estimate_says(coded("-32000", $said)));
        }
    )*};
}

marked! {
    funds_words_are_the_calls: "insufficient funds for gas * price + value" => the_calls;
    a_used_nonce_is_the_calls: "nonce too low" => the_calls;
    pending_words_are_the_calls: "already known" => the_calls;
    underpriced_words_are_the_calls: "replacement transaction underpriced" => the_calls;
    gas_too_low_words_are_the_calls: "intrinsic gas too low" => the_calls;
    revert_words_are_the_calls: "execution reverted" => the_calls;
    rate_limit_words_are_not_the_calls: "daily request limit exceeded" => not_the_calls;
    missing_method_words_are_not_the_calls: "the method eth_estimateGas does not exist/is not available" => not_the_calls;
    credentials_words_are_not_the_calls: "unauthorized: bad api key" => not_the_calls;
    wrong_chain_words_are_not_the_calls: "invalid chain id for signer" => not_the_calls;
}

// ───────────────────────── A numeric code ─────────────────────────

#[test]
fn code_three_with_data_is_the_calls() {
    the_calls(estimate_says(error(200, "{\"code\":3,\"data\":\"0x\",\"message\":\"x\"}")));
}

#[test]
fn code_three_without_data_is_the_calls() {
    the_calls(estimate_says(coded("3", "x")));
}

#[test]
fn another_code_with_a_reverts_data_is_the_calls() {
    the_calls(estimate_says(error(200, "{\"code\":-32000,\"data\":\"0x08c379a0\",\"message\":\"VM Exception\"}")));
}

#[test]
fn words_not_known_under_a_code_are_the_calls() {
    the_calls(estimate_says(coded("-32000", "gas required exceeds allowance (30000000)")));
}

#[test]
fn data_that_is_not_hex_under_a_code_is_the_calls() {
    the_calls(estimate_says(error(200, "{\"code\":-32000,\"data\":\"see the logs\",\"message\":\"x\"}")));
}

#[test]
fn an_internal_error_code_is_the_calls() {
    the_calls(estimate_says(coded("-32603", "internal error")));
}

#[test]
fn the_neighbours_of_the_rate_limit_code_are_the_calls() {
    for code in ["-32006", "-32004"] {
        the_calls(estimate_says(coded(code, "x")));
    }
}

#[test]
fn a_code_past_64_bits_is_the_calls() {
    the_calls(estimate_says(coded("9223372036854775808", "x")));
}

#[test]
fn a_code_written_as_a_fraction_is_the_calls() {
    the_calls(estimate_says(coded("-32601.0", "x")));
}

#[test]
fn the_method_code_is_not_the_calls() {
    not_the_calls(estimate_says(coded("-32601", "x")));
}

#[test]
fn the_rate_limit_code_is_not_the_calls() {
    not_the_calls(estimate_says(coded("-32005", "x")));
}

#[test]
fn a_coded_error_under_a_server_error_status_is_the_calls() {
    the_calls(estimate_says(error(500, "{\"code\":-32000,\"message\":\"gas required exceeds allowance\"}")));
}

// ───────────────────────── No numeric code ─────────────────────────

#[test]
fn a_code_written_as_text_is_not_the_calls() {
    not_the_calls(estimate_says(error(200, "{\"code\":\"-32000\",\"message\":\"x\"}")));
}

#[test]
fn a_message_without_a_code_is_not_the_calls() {
    not_the_calls(estimate_says(error(200, "{\"message\":\"upstream unavailable\"}")));
}

#[test]
fn an_error_that_is_bare_text_is_not_the_calls() {
    not_the_calls(estimate_says(error(200, "\"upstream unavailable\"")));
}

// ───────────────────────── The transport ─────────────────────────

#[test]
fn an_answer_that_is_not_json_is_not_the_calls() {
    not_the_calls(estimate_says(Reply::Say(200, "<html>bad gateway</html>".into())));
}

#[test]
fn a_server_error_page_is_not_the_calls() {
    not_the_calls(estimate_says(Reply::Say(502, "<html>bad gateway</html>".into())));
}

#[test]
fn a_rate_limit_page_is_not_the_calls() {
    not_the_calls(estimate_says(Reply::Say(429, "<html>slow down</html>".into())));
}

#[test]
fn a_forbidden_page_is_not_the_calls() {
    not_the_calls(estimate_says(Reply::Say(403, "<html>no</html>".into())));
}

#[test]
fn a_closed_connection_is_not_the_calls() {
    not_the_calls(estimate_says(Reply::Close));
}

/// A refused head query (even with an error code) says nothing about the call itself.
#[test]
fn a_head_refused_with_a_code_is_not_the_calls() {
    not_the_calls(anchor_with(coded("-32000", "gas required exceeds allowance"), ok("\"0x5208\"")));
}
