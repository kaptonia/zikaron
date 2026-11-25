use std::io::{Read, Write};
use zikaron::json::Value;
use zikaron_glue::book::{self, Kind, Law};
use zikaron_send::Code;
use zikaron_send::node::NodeAddr;

const BOOK_CODE: &str = include_str!("data/SuccessionBook.runtime.hex");
const REGISTRY_CODE: &str = include_str!("data/ZikaronRegistry.runtime.hex");
const CHAIN: u64 = 11_155_111;
const REGISTRY: [u8; 20] = [0x5e; 20];
const AUTHOR: [u8; 32] = [0x21; 32];
const OTHER: [u8; 32] = [0x43; 32];

fn address(k: &[u8; 32]) -> [u8; 20] {
    zikaron::cryptox::address_of_privkey(k).expect("a key")
}

fn sealed(key: &[u8; 32], entry_type: &str, seq: u64, prev: Option<&str>, body: Value) -> book::Entry {
    let author = zikaron::hexfmt::encode(&address(key));
    let sign = |pre: &[u8], domain: &str| -> Result<String, ()> {
        let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
        let (r, s, v) = zikaron::cryptox::sign_digest(key, &digest).ok_or(())?;
        Ok(zikaron::hexfmt::encode(&[&r[..], &s[..], &[v][..]].concat()))
    };
    match book::seal(Law::Two, &author, entry_type, seq, prev, body, sign) {
        book::Sealed::Done(e) => e,
        _ => panic!("sealed"),
    }
}

fn pile() -> (Vec<Vec<u8>>, Vec<u8>) {
    let genesis = sealed(&AUTHOR, "genesis", 0, None, book::genesis_body("一本账", None));
    let body = zikaron_glue::face::Writer::of(std::slice::from_ref(&genesis.bytes)).expect("a root").persona("Lin").expect("a name");
    let face = sealed(&AUTHOR, Kind::Face.as_str(), 1, Some(&genesis.id_hex()), body);
    (vec![genesis.bytes, face.bytes.clone()], face.bytes)
}

fn node(registry: &'static str) -> NodeAddr {
    let at = zikaron_anchor::book::at(CHAIN).expect("the book's row");
    let author = address(&AUTHOR);
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
            let method = req.member("method").and_then(|m| m.as_str()).unwrap_or("");
            let params: Vec<Value> = req.member("params").and_then(Value::as_arr).cloned().unwrap_or_default();
            let asked = params.first().and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
            let word = format!("\"0x{}{}\"", "0".repeat(24), zikaron::hexfmt::encode(&author).trim_start_matches("0x"));
            let result = match method {
                "eth_chainId" => format!("\"0x{CHAIN:x}\""),
                "eth_blockNumber" => "\"0x20\"".into(),
                "eth_getCode" if asked == zikaron::hexfmt::encode(&at) => format!("\"{}\"", BOOK_CODE.trim()),
                "eth_getCode" if asked == zikaron::hexfmt::encode(&REGISTRY) => format!("\"{}\"", registry.trim()),
                "eth_getCode" => "\"0x\"".into(),
                "eth_call" => word,
                _ => "null".into(),
            };
            let reply = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{result}}}");
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
        }
    });
    NodeAddr::new(format!("http://127.0.0.1:{port}"))
}

fn gate(registry: &'static str, from: &[u8; 32]) -> Result<(), Code> {
    let (pile, entry) = pile();
    zikaron_send::batch::face_gate(&[node(registry)], CHAIN, &REGISTRY, &entry, &pile, &address(from)).map_err(|t| t.code)
}

#[test]
fn a_face_entry_goes_only_to_the_pinned_registry_from_its_author() {
    assert_eq!(gate(REGISTRY_CODE, &AUTHOR), Ok(()));
    assert_eq!(gate("0x6001", &AUTHOR), Err(Code::RegistryCode), "another build");
    assert_eq!(gate("0x", &AUTHOR), Err(Code::RegistryCode), "no code at the address");
    assert_eq!(gate(REGISTRY_CODE, &OTHER), Err(Code::AddressMismatch), "a sender that did not sign it");
}

#[test]
fn a_continued_ledger_writes_and_is_gated_under_the_root_it_continues() {
    let older = zikaron::hexfmt::encode(&[0x7c; 20]);
    let genesis = sealed(&AUTHOR, "genesis", 0, None, book::genesis_body("续接", Some((&older, &format!("0x{}", "ab".repeat(32))))));
    let w = zikaron_glue::face::Writer::of(std::slice::from_ref(&genesis.bytes)).expect("a root");
    let body = w.persona("Lin").expect("a name");
    assert_eq!(body.member("root").and_then(Value::as_str), Some(older.as_str()));
    let face = sealed(&AUTHOR, Kind::Face.as_str(), 1, Some(&genesis.id_hex()), body);
    let pile = vec![genesis.bytes, face.bytes.clone()];
    let r = zikaron_send::batch::face_gate(&[node(REGISTRY_CODE)], CHAIN, &REGISTRY, &face.bytes, &pile, &address(&AUTHOR)).map_err(|t| t.code);
    assert_eq!(r, Err(Code::OfficeElsewhere));
}
