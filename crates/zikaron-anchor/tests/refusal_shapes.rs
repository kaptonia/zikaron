//! What a node's refusal of a gas estimate says about the call, read by the shape of the node's answer and not
//! by its words alone (`said::refusal_of`, `said::refuses_the_call`): the one table the app and the command line
//! both use. Each test is one form of the closed table, named on it:
//!
//! - the marker sentences, one member each, before any shape;
//! - a numeric `code` (the node's JSON-RPC refusal): 3, a revert's `data` (hex, nested), the two codes the table
//!   names (-32601, -32005) with neutral words, and every other numeric code (-32000 with words not known, with
//!   `data` that is not hex, -32603, the neighbours -32006 and -32004, past 64 bits, the 64-bit floor, written
//!   as a fraction or with an exponent): the call refused, every one;
//! - no numeric `code` (a code written as text, `null`, a boolean, none at all, an error that is a bare text):
//!   not the node's refusal of the call;
//! - the transport: an answer that is not JSON, JSON without the shape of an answer, a 5xx page, a 429 page, a
//!   401 and a 403 page, the deadline passing, a node not listening: never about the call; and a coded error
//!   carried under a 5xx or 4xx status, which is the node's word whatever the status.
//!
//! The answers in the last group come from a node started in this process; nothing reaches the network.

use std::io::{Read, Write};
use std::net::TcpListener;
use zikaron::json::Value;
use zikaron_anchor::rpc::{self, Endpoint, Http, Trouble};
use zikaron_anchor::said::{refusal, refusal_of, refuses_the_call, Refusal};

/// The node's error object as an estimate's `error` member carries it.
fn node(err: &str) -> Trouble {
    Trouble::Node(err.to_string())
}

/// The member the one table reads, and whether it is about the call.
fn read(t: &Trouble) -> (Option<Refusal>, bool) {
    (refusal(t), refuses_the_call(t))
}

/// `Error(string)` with the text `no`, as hex.
fn revert_data() -> String {
    let mut b = vec![0x08, 0xc3, 0x79, 0xa0];
    let word = |n: u8| {
        let mut w = vec![0u8; 31];
        w.push(n);
        w
    };
    b.extend(word(32));
    b.extend(word(2));
    let mut text = b"no".to_vec();
    text.resize(32, 0);
    b.extend(text);
    zikaron::hexfmt::encode(&b)
}

// ───────────────────────── The marker sentences: one member each, about the call or not ─────────────────────────

macro_rules! marked {
    ($($name:ident: $said:literal => $member:expr, $call:literal;)*) => {$(
        #[test]
        fn $name() {
            let t = node(&format!("{{\"code\":-32000,\"message\":\"{}\"}}", $said));
            assert_eq!(read(&t), (Some($member), $call), "{}", $said);
        }
    )*};
}

marked! {
    a_marker_for_funds_is_the_calls: "insufficient funds for gas * price + value" => Refusal::Funds, true;
    a_marker_for_a_used_nonce_is_the_calls: "nonce too low" => Refusal::NonceUsed, true;
    a_marker_for_pending_is_the_calls: "already known" => Refusal::Pending, true;
    a_marker_for_underpriced_is_the_calls: "replacement transaction underpriced" => Refusal::Underpriced, true;
    a_marker_for_gas_too_low_is_the_calls: "intrinsic gas too low" => Refusal::GasTooLow, true;
    a_marker_for_a_revert_is_the_calls: "execution reverted" => Refusal::Reverted, true;
    a_marker_for_a_rate_limit_is_the_nodes: "daily request limit exceeded" => Refusal::RateLimited, false;
    a_marker_for_a_missing_method_is_the_nodes: "the method eth_estimateGas does not exist/is not available" => Refusal::NoMethod, false;
    a_marker_for_credentials_is_the_nodes: "unauthorized: bad api key" => Refusal::Auth, false;
    a_marker_for_a_wrong_chain_is_the_nodes: "invalid chain id for signer" => Refusal::WrongChain, false;
}

/// A marker comes before the shape: a sentence the table knows, in an error without a numeric code, is still
/// its member (here about the call).
#[test]
fn a_marker_without_a_code_is_still_its_member() {
    assert_eq!(read(&node("{\"message\":\"insufficient funds\"}")), (Some(Refusal::Funds), true));
    assert_eq!(read(&node("\"nonce too low\"")), (Some(Refusal::NonceUsed), true));
}

// ───────────────────────── A numeric code: the node's own refusal ─────────────────────────

#[test]
fn code_three_with_data_is_a_revert() {
    let t = node(&format!("{{\"code\":3,\"data\":\"{}\",\"message\":\"x\"}}", revert_data()));
    assert_eq!(read(&t), (Some(Refusal::Reverted), true));
}

#[test]
fn code_three_without_data_is_a_revert() {
    assert_eq!(read(&node("{\"code\":3,\"message\":\"x\"}")), (Some(Refusal::Reverted), true));
}

#[test]
fn another_code_with_a_reverts_data_is_a_revert() {
    let t = node(&format!("{{\"code\":-32000,\"data\":\"{}\",\"message\":\"VM Exception while processing transaction\"}}", revert_data()));
    assert_eq!(read(&t), (Some(Refusal::Reverted), true));
}

#[test]
fn a_reverts_data_nested_in_an_object_is_a_revert() {
    let t = node(&format!("{{\"code\":-32015,\"data\":{{\"data\":\"{}\"}},\"message\":\"VM execution error.\"}}", revert_data()));
    assert_eq!(read(&t), (Some(Refusal::Reverted), true));
}

#[test]
fn an_empty_reverts_data_is_a_revert() {
    assert_eq!(read(&node("{\"code\":-32000,\"data\":\"0x\",\"message\":\"x\"}")), (Some(Refusal::Reverted), true));
}

#[test]
fn data_that_is_not_hex_under_a_code_is_the_calls_refusal() {
    let err = "{\"code\":-32000,\"data\":\"see the logs\",\"message\":\"x\"}";
    assert_eq!(read(&node(err)), (Some(Refusal::Coded(err.into())), true));
}

#[test]
fn words_not_known_under_a_code_are_the_calls_refusal() {
    let err = "{\"code\":-32000,\"message\":\"gas required exceeds allowance (30000000)\"}";
    assert_eq!(read(&node(err)), (Some(Refusal::Coded(err.into())), true), "the original words are kept");
}

#[test]
fn an_internal_error_code_is_the_calls_refusal() {
    let err = "{\"code\":-32603,\"message\":\"internal error\"}";
    assert_eq!(read(&node(err)), (Some(Refusal::Coded(err.into())), true));
}

#[test]
fn the_method_code_with_neutral_words_is_the_nodes() {
    assert_eq!(read(&node("{\"code\":-32601,\"message\":\"x\"}")), (Some(Refusal::NoMethod), false));
}

#[test]
fn the_rate_limit_code_with_neutral_words_is_the_nodes() {
    assert_eq!(read(&node("{\"code\":-32005,\"message\":\"x\"}")), (Some(Refusal::RateLimited), false));
}

#[test]
fn the_neighbours_of_the_rate_limit_code_are_the_calls_refusal() {
    for code in ["-32006", "-32004"] {
        let err = format!("{{\"code\":{code},\"message\":\"x\"}}");
        assert_eq!(read(&node(&err)), (Some(Refusal::Coded(err.clone())), true), "{code}");
    }
}

#[test]
fn a_code_past_64_bits_is_the_calls_refusal() {
    let err = "{\"code\":9223372036854775808,\"message\":\"x\"}";
    assert_eq!(read(&node(err)), (Some(Refusal::Coded(err.into())), true));
}

#[test]
fn the_64_bit_floor_is_the_calls_refusal() {
    let err = "{\"code\":-9223372036854775808,\"message\":\"x\"}";
    assert_eq!(read(&node(err)), (Some(Refusal::Coded(err.into())), true));
}

#[test]
fn a_code_written_as_a_fraction_is_the_calls_refusal_and_not_the_named_code() {
    let err = "{\"code\":-32601.0,\"message\":\"x\"}";
    assert_eq!(read(&node(err)), (Some(Refusal::Coded(err.into())), true));
}

#[test]
fn a_code_written_with_an_exponent_is_the_calls_refusal_and_not_the_named_code() {
    let err = "{\"code\":-3.2601e4,\"message\":\"x\"}";
    assert_eq!(read(&node(err)), (Some(Refusal::Coded(err.into())), true));
}

// ───────────────────────── No numeric code: not the node's refusal of the call ─────────────────────────

#[test]
fn a_code_written_as_text_is_not_the_nodes_refusal() {
    let err = "{\"code\":\"-32000\",\"message\":\"x\"}";
    assert_eq!(read(&node(err)), (Some(Refusal::Other(err.into())), false));
}

#[test]
fn a_named_code_written_as_text_is_not_that_member() {
    let err = "{\"code\":\"-32601\",\"message\":\"x\"}";
    assert_eq!(read(&node(err)), (Some(Refusal::Other(err.into())), false));
}

#[test]
fn a_null_code_is_not_the_nodes_refusal() {
    let err = "{\"code\":null,\"message\":\"x\"}";
    assert_eq!(read(&node(err)), (Some(Refusal::Other(err.into())), false));
}

#[test]
fn a_boolean_code_is_not_the_nodes_refusal() {
    let err = "{\"code\":true,\"message\":\"x\"}";
    assert_eq!(read(&node(err)), (Some(Refusal::Other(err.into())), false));
}

#[test]
fn a_message_without_a_code_is_not_the_nodes_refusal() {
    let err = "{\"message\":\"upstream unavailable\"}";
    assert_eq!(read(&node(err)), (Some(Refusal::Other(err.into())), false));
}

#[test]
fn an_error_that_is_bare_text_is_not_the_nodes_refusal() {
    let err = "\"upstream unavailable\"";
    assert_eq!(read(&node(err)), (Some(Refusal::Other(err.into())), false));
}

// ───────────────────────── The transport: never about the call ─────────────────────────

/// A node on a local port answering every question once with this status line and body (`{id}` put in).
fn answering(status: &'static str, body: &'static str) -> String {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}/", l.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let mut s = s;
            let mut raw = Vec::new();
            let mut buf = [0u8; 4096];
            let req = loop {
                match s.read(&mut buf) {
                    Ok(0) | Err(_) => break None,
                    Ok(n) => raw.extend_from_slice(&buf[..n]),
                }
                let text = String::from_utf8_lossy(&raw).to_string();
                if let Some(i) = text.find("\r\n\r\n") {
                    let len: usize = text[..i].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|n| n.trim().parse().ok())).unwrap_or(0);
                    if raw.len() >= i + 4 + len {
                        break Some(text[i + 4..].to_string());
                    }
                }
            };
            let Some(req) = req else { continue };
            let id = req.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
            let body = body.replace("{id}", &id);
            let _ = s.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
        }
    });
    url
}

/// The trouble one estimate asked of that node gives (the patience table's pauses set to zero).
fn asked(url: &str) -> Trouble {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    Http::new(url).expect("an endpoint").call("eth_estimateGas", &Value::Arr(Vec::new())).expect_err("refused")
}

#[test]
fn an_answer_that_is_not_json_is_not_about_the_call() {
    let t = asked(&answering("200 OK", "<html>bad gateway</html>"));
    assert!(matches!(&t, Trouble::Transport(e) if rpc::is_not_json(e)), "{t:?}");
    assert_eq!(read(&t), (None, false));
}

#[test]
fn json_without_the_shape_of_an_answer_is_not_about_the_call() {
    let t = asked(&answering("200 OK", "{\"message\":\"x\"}"));
    assert!(matches!(&t, Trouble::Transport(e) if rpc::is_shapeless(e)), "{t:?}");
    assert_eq!(read(&t), (None, false));
}

#[test]
fn a_server_error_page_is_not_about_the_call() {
    for line in ["500 Internal Server Error", "502 Bad Gateway", "503 Service Unavailable"] {
        let t = asked(&answering(line, "<html>busy</html>"));
        assert!(matches!(read(&t), (Some(Refusal::Other(_)), false)), "{line}: {t:?}");
    }
}

#[test]
fn a_rate_limit_page_is_not_about_the_call() {
    let t = asked(&answering("429 Too Many Requests", "<html>slow down</html>"));
    assert_eq!(read(&t), (Some(Refusal::RateLimited), false));
}

#[test]
fn a_credentials_page_is_not_about_the_call() {
    for line in ["401 Unauthorized", "403 Forbidden"] {
        let t = asked(&answering(line, "<html>no</html>"));
        assert_eq!(read(&t), (Some(Refusal::Auth), false), "{line}");
    }
}

#[test]
fn a_coded_error_under_an_error_status_is_the_nodes_word_and_the_calls_refusal() {
    for line in ["500 Internal Server Error", "400 Bad Request"] {
        let t = asked(&answering(line, "{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{\"code\":-32000,\"message\":\"gas required exceeds allowance\"}}"));
        assert!(matches!(read(&t), (Some(Refusal::Coded(_)), true)), "{line}: {t:?}");
    }
}

#[test]
fn the_deadline_passing_is_not_about_the_call() {
    let t = rpc::late("http://n.example", std::time::Duration::from_secs(5));
    assert_eq!(read(&t), (None, false));
}

#[test]
fn a_node_not_listening_is_not_about_the_call() {
    let port = TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port();
    let t = asked(&format!("http://127.0.0.1:{port}/"));
    assert!(matches!(t, Trouble::Transport(_)), "{t:?}");
    assert_eq!(read(&t), (None, false));
}

#[test]
fn a_recordings_hole_is_not_about_the_call() {
    assert_eq!(read(&Trouble::NotServed("eth_estimateGas".into())), (None, false));
    assert_eq!(read(&Trouble::Contradiction("eth_estimateGas".into())), (None, false));
}

/// The member is read once, by `refusal_of`; `refusal` of a node's error is that same reading.
#[test]
fn the_one_table_reads_a_nodes_error_in_one_place() {
    let err = "{\"code\":-32603,\"message\":\"internal error\"}";
    assert_eq!(refusal(&node(err)), Some(refusal_of(err)));
}
