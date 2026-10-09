//! The app's gas estimate end to end (`Action::EstimateGas` over a home with work queued): a refusal the error
//! table reads as about the call is reported as "the estimate was refused" (`GAS_REFUSED`); one that says
//! nothing about the call is reported by its own member; a head the node will not give is never about the call.
//! Places are set before any vault or shell use (`vault_open`); each test runs alone in its own process
//! ([`super::alone_in`]). Nodes are local ports in this process; no network.

use super::vault_open;
use app::action::{apply, apply_settled, Action, Applied};
use app::fault::Known;
use std::io::{Read, Write};

/// A node answering the head and the block as a node does, `head` for the head the estimate is asked at when
/// given, and `estimate` (a JSON-RPC error object) for the estimate.
fn node(head: Option<&'static str>, estimate: &'static str) -> String {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
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
                let method = req.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                let id = req.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                let answer = |member: &str, v: &str| format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"{member}\":{v}}}");
                let body = match method.as_str() {
                    "eth_blockNumber" => match head {
                        Some(e) => answer("error", e),
                        None => answer("result", "\"0x40\""),
                    },
                    "eth_chainId" => answer("result", "\"0x7a69\""),
                    "eth_getBlockByNumber" => answer("result", "{\"baseFeePerGas\":\"0x3b9aca00\",\"number\":\"0x40\",\"timestamp\":\"0x64\"}"),
                    "eth_estimateGas" => answer("error", estimate),
                    _ => answer("result", "null"),
                };
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    url
}

/// A home with a genesis and one work queued, its chain cell, registry and node set; the estimate asked for one
/// entry; the member it is refused with.
fn estimate_refused(name: &str, head: Option<&'static str>, estimate: &'static str) -> Option<Known> {
    estimate_refused_at(name, None, head, estimate)
}

/// [`estimate_refused`] with the node's address given (`None`: the node started here).
fn estimate_refused_at(name: &str, address: Option<&str>, head: Option<&'static str>, estimate: &'static str) -> Option<Known> {
    vault_open();
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let dir = std::env::temp_dir().join(format!("zk-b11-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    answers!(apply(&mut shell, Action::OpenHome { root: dir.join("home").display().to_string() }), Applied::Homed { .. });
    answers!(apply(&mut shell, Action::MakeAnchorKey), Applied::AnchorKey(_));
    let _ = apply_settled(&mut shell, Action::Genesis { statement: "b11".into() });
    let probe = dir.join("probe.txt");
    std::fs::write(&probe, b"b11").expect("a file");
    let _ = apply_settled(&mut shell, Action::TakeContent { source: app::anchorx::Source::File, path: probe.display().to_string() });
    let _ = apply_settled(&mut shell, Action::RecordWork { note_md: String::new(), files: Vec::new(), for_: None });
    shell.settings.chain_id = Some(31337);
    shell.settings.registry = Some(app::key::Address([0x11; 20]));
    let url = address.map(str::to_string).unwrap_or_else(|| node(head, estimate));
    shell.endpoints = vec![app::chainx::Endpoint::at(31337, url)];
    let said = match apply_settled(&mut shell, Action::EstimateGas { count: 1 }) {
        Applied::Trouble(f) => f.which(),
        other => panic!("the estimate was not refused: {other:?}"),
    };
    drop(shell);
    let _ = std::fs::remove_dir_all(&dir);
    said
}

#[test]
fn a_coded_refusal_in_words_not_known_is_the_estimate_refused() {
    if super::alone_in(module_path!(), "a_coded_refusal_in_words_not_known_is_the_estimate_refused") {
        return;
    }
    assert_eq!(estimate_refused("coded", None, "{\"code\":-32000,\"message\":\"gas required exceeds allowance\"}"), Some(Known::GasRefused));
}

#[test]
fn an_internal_error_code_is_the_estimate_refused() {
    if super::alone_in(module_path!(), "an_internal_error_code_is_the_estimate_refused") {
        return;
    }
    assert_eq!(estimate_refused("internal", None, "{\"code\":-32603,\"message\":\"internal error\"}"), Some(Known::GasRefused));
}

#[test]
fn a_code_written_as_text_is_said_as_the_nodes_refusal_not_the_estimates() {
    if super::alone_in(module_path!(), "a_code_written_as_text_is_said_as_the_nodes_refusal_not_the_estimates") {
        return;
    }
    assert_eq!(estimate_refused("text", None, "{\"code\":\"-32000\",\"message\":\"x\"}"), Some(Known::NodeRefused));
}

#[test]
fn a_message_without_a_code_is_said_as_the_nodes_refusal_not_the_estimates() {
    if super::alone_in(module_path!(), "a_message_without_a_code_is_said_as_the_nodes_refusal_not_the_estimates") {
        return;
    }
    assert_eq!(estimate_refused("uncoded", None, "{\"message\":\"upstream unavailable\"}"), Some(Known::NodeRefused));
}

#[test]
fn a_head_refused_with_a_code_is_never_the_estimate_refused() {
    if super::alone_in(module_path!(), "a_head_refused_with_a_code_is_never_the_estimate_refused") {
        return;
    }
    let said = estimate_refused("head", Some("{\"code\":-32000,\"message\":\"gas required exceeds allowance\"}"), "{\"code\":3,\"message\":\"execution reverted\"}");
    assert_eq!(said, Some(Known::NodeRefused));
}

/// Every node address of the chain fails to read: the estimate is never asked, and the refusal is the addresses'
/// shape (`SETTINGS_SHAPE`), never "the transaction would fail".
#[test]
fn addresses_that_do_not_read_are_said_as_their_shape_not_the_estimate_refused() {
    if super::alone_in(module_path!(), "addresses_that_do_not_read_are_said_as_their_shape_not_the_estimate_refused") {
        return;
    }
    assert_eq!(estimate_refused_at("unread", Some("wss://node.example"), None, "{}"), Some(Known::SettingsShape));
}
