//! The app reads a refused gas estimate with the same table the command line uses (`chainx::ask_call`,
//! `said::refuses_the_call`): whether the refusal is about the call is judged on the same trouble its member is
//! taken from, by the shape of the node's answer. One test per form of the closed table, each against nodes
//! started in this process (no network); the member reported is the same for every form (`said_fault`).

use app::chainx::{ask_call, Endpoint};
use app::fault::Known;
use std::io::{Read, Write};
use zikaron::json::Value;

/// A node on a local port answering every question with this status line and body (`{id}` put in), or closing
/// the connection when the status line is empty.
fn node(status: &'static str, body: String) -> String {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let body = body.clone();
            std::thread::spawn(move || {
                let mut s = s;
                let mut raw = Vec::new();
                let mut buf = [0u8; 4096];
                let req = loop {
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
                if status.is_empty() {
                    return;
                }
                let id = req.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                let body = body.replace("{id}", &id);
                let _ = s.write_all(format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    url
}

/// A node whose every answer is this JSON-RPC error.
fn erring(err: &str) -> String {
    node("200 OK", format!("{{\"jsonrpc\":\"2.0\",\"id\":{{id}},\"error\":{err}}}"))
}

/// The estimate asked of these places: the member said and whether it is about the call.
fn estimate(urls: &[String]) -> (Option<Known>, bool) {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(31337, u.as_str())).collect();
    match ask_call(&eps, "eth_estimateGas", &Value::Arr(Vec::new())) {
        Ok(r) => panic!("answered: {:?}", r.value),
        Err((f, call)) => (f.which(), call),
    }
}

fn one(url: String) -> (Option<Known>, bool) {
    estimate(&[url])
}

#[test]
fn a_marker_about_the_call_is_the_calls() {
    assert_eq!(one(erring("{\"code\":-32000,\"message\":\"insufficient funds for gas\"}")), (Some(Known::InsufficientFunds), true));
}

#[test]
fn a_marker_about_the_node_is_not_the_calls() {
    assert_eq!(one(erring("{\"code\":-32000,\"message\":\"unauthorized: bad api key\"}")), (Some(Known::NodeAuth), false));
}

#[test]
fn a_revert_is_the_calls() {
    assert_eq!(one(erring("{\"code\":3,\"message\":\"execution reverted\"}")), (Some(Known::ContractRefused), true));
}

#[test]
fn another_code_with_a_reverts_data_is_the_calls_revert() {
    assert_eq!(one(erring("{\"code\":-32000,\"data\":\"0x\",\"message\":\"VM Exception\"}")), (Some(Known::ContractRefused), true));
}

#[test]
fn words_not_known_under_a_code_are_the_calls_and_said_as_the_nodes_refusal() {
    assert_eq!(one(erring("{\"code\":-32000,\"message\":\"gas required exceeds allowance\"}")), (Some(Known::NodeRefused), true));
}

#[test]
fn an_internal_error_code_is_the_calls() {
    assert_eq!(one(erring("{\"code\":-32603,\"message\":\"internal error\"}")), (Some(Known::NodeRefused), true));
}

#[test]
fn the_rate_limit_code_is_not_the_calls() {
    assert_eq!(one(erring("{\"code\":-32005,\"message\":\"x\"}")), (Some(Known::RateLimited), false));
}

#[test]
fn a_code_written_as_text_is_not_the_calls() {
    assert_eq!(one(erring("{\"code\":\"-32000\",\"message\":\"x\"}")), (Some(Known::NodeRefused), false));
}

#[test]
fn a_message_without_a_code_is_not_the_calls() {
    assert_eq!(one(erring("{\"message\":\"upstream unavailable\"}")), (Some(Known::NodeRefused), false));
}

#[test]
fn an_error_that_is_bare_text_is_not_the_calls() {
    assert_eq!(one(erring("\"upstream unavailable\"")), (Some(Known::NodeRefused), false));
}

#[test]
fn an_answer_that_is_not_json_is_not_the_calls() {
    assert_eq!(one(node("200 OK", "<html>bad gateway</html>".into())), (Some(Known::AnswerNotJson), false));
}

#[test]
fn a_server_error_page_is_not_the_calls() {
    assert_eq!(one(node("502 Bad Gateway", "<html>busy</html>".into())), (Some(Known::NodeRefused), false));
}

#[test]
fn a_rate_limit_page_is_not_the_calls() {
    assert_eq!(one(node("429 Too Many Requests", "<html>slow</html>".into())), (Some(Known::RateLimited), false));
}

#[test]
fn a_forbidden_page_is_not_the_calls() {
    assert_eq!(one(node("403 Forbidden", "<html>no</html>".into())), (Some(Known::NodeAuth), false));
}

#[test]
fn a_coded_error_under_a_server_error_status_is_the_calls() {
    let url = node("500 Internal Server Error", "{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{\"code\":-32000,\"message\":\"x\"}}".into());
    assert_eq!(one(url), (Some(Known::NodeRefused), true));
}

#[test]
fn a_closed_connection_is_not_the_calls() {
    assert_eq!(one(node("", String::new())).1, false);
}

#[test]
fn no_place_at_all_is_not_the_calls() {
    assert_eq!(estimate(&[]), (Some(Known::NoEndpoint), false));
}

/// Places whose addresses do not read are never asked: the refusal is the addresses' shape, not the call's.
#[test]
fn places_whose_addresses_do_not_read_are_not_the_calls() {
    assert_eq!(estimate(&["wss://node.example".to_string()]), (Some(Known::SettingsShape), false));
}

#[test]
fn places_that_answer_differently_are_not_the_calls() {
    let a = node("200 OK", "{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":\"0x5208\"}".into());
    let b = node("200 OK", "{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":\"0x5209\"}".into());
    assert_eq!(estimate(&[a, b]), (Some(Known::Disagree), false));
}

/// With several places refusing, the member and whether it is about the call are both read off the first
/// place's trouble (in the table's order): the two never come from different places.
#[test]
fn several_refusing_places_are_read_off_the_first() {
    let coded = || erring("{\"code\":-32000,\"message\":\"x\"}");
    let text = || erring("{\"code\":\"-32000\",\"message\":\"x\"}");
    assert_eq!(estimate(&[coded(), text()]), (Some(Known::NodeRefused), true));
    assert_eq!(estimate(&[text(), coded()]), (Some(Known::NodeRefused), false));
    let limited = || erring("{\"code\":-32005,\"message\":\"x\"}");
    assert_eq!(estimate(&[limited(), coded()]), (Some(Known::RateLimited), false));
}
