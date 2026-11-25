use std::io::{Read, Write};
use zikaron_send::agree::{scan, Scan};
use zikaron_send::node::{Endpoint, NodeAddr};

const CHAIN: u64 = 31337;

fn node() -> String {
    node_of("0x7a69", false)
}

fn node_of(chain_id: &'static str, refuses: bool) -> String {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
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
                let method = body.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                let id = body.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                if refuses {
                    let body = format!("{{\"error\":{{\"code\":-32000,\"message\":\"missing api key\"}},\"id\":{id},\"jsonrpc\":\"2.0\"}}");
                    let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
                    return;
                }
                let chain = format!("\"{chain_id}\"");
                let result = match method.as_str() {
                    "eth_chainId" => chain.as_str(),
                    "eth_blockNumber" => "\"0x100\"",
                    "eth_getLogs" => "[]",
                    _ => "null",
                };
                let body = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{result}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    url
}

fn dead() -> String {
    let port = std::net::TcpListener::bind("127.0.0.1:0").ok().and_then(|l| l.local_addr().ok()).map(|a| a.port()).unwrap_or(1);
    format!("http://127.0.0.1:{port}")
}

fn basis() -> Vec<u8> {
    let senders = vec![format!("0x{}", "22".repeat(20))];
    let registry = format!("0x{}", "11".repeat(20));
    let w = zikaron_glue::basis::Window { chain: CHAIN, registries: std::slice::from_ref(&registry), from_block: 0x10, to_block: 0x20, senders: &senders };
    zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&w))
}

fn agreed(urls: &[String]) -> (bool, Vec<u64>) {
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(CHAIN, NodeAddr::new(u.as_str()))).collect();
    match scan(&eps, &basis(), &zikaron_send::Bare) {
        Ok(Scan::Basis(got)) => (got.single_source, got.single_source_chains),
        Ok(Scan::NoLabel { report, .. }) => panic!("a basis: {report:?}"),
        Err(t) => panic!("an agreed scan: {t:?}"),
    }
}

#[test]
fn the_single_source_chains_are_those_fewer_than_two_places_answered() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let (one, two) = (node(), node());
    assert_eq!(agreed(&[one.clone(), dead()]), (true, vec![CHAIN]), "a node that did not answer is no witness");
    assert_eq!(agreed(&[one, two]), (false, Vec::new()));
}

#[test]
fn a_basis_of_two_chains_is_agreed_chain_by_chain() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    const OTHER: u64 = 31338;
    let senders = vec![format!("0x{}", "22".repeat(20))];
    let registry = format!("0x{}", "11".repeat(20));
    let windows = [
        zikaron_glue::basis::Window { chain: CHAIN, registries: std::slice::from_ref(&registry), from_block: 0x10, to_block: 0x20, senders: &senders },
        zikaron_glue::basis::Window { chain: OTHER, registries: std::slice::from_ref(&registry), from_block: 0x10, to_block: 0x20, senders: &senders },
    ];
    let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of_all(&windows));
    let eps = vec![
        Endpoint::at(CHAIN, NodeAddr::new(node())),
        Endpoint::at(CHAIN, NodeAddr::new(node())),
        Endpoint::at(OTHER, NodeAddr::new(node_of("0x7a6a", false))),
        Endpoint::at(OTHER, NodeAddr::new(node_of("0x7a6a", true))),
    ];
    match scan(&eps, &basis, &zikaron_send::Bare) {
        Ok(Scan::Basis(got)) => {
            assert_eq!(got.single_source_chains, vec![OTHER], "the first chain read from two places: {:?}", got.unanswered);
            let read: Vec<Option<&zikaron::json::Value>> = got.fragment.member("basis").and_then(|b| b.member("chains")).and_then(|c| c.as_arr()).map(|cs| cs.iter().map(|c| c.member("chainId")).collect()).unwrap_or_default();
            assert_eq!(read.len(), 2, "both chains in the basis read: {:?}", got.fragment);
        }
        Ok(Scan::NoLabel { report, .. }) => panic!("a basis: {report:?}"),
        Err(t) => panic!("an agreed scan: {t:?}"),
    }
}
