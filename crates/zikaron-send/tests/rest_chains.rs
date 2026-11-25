use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use zikaron_send::node::{self, Endpoint, NodeAddr};
use zikaron_send::recipient::{key_of, Found, Net};

const A: u64 = 1;
const B: u64 = 5;
const KEY: &str = "K-REST-CHAINS";

type Asked = Arc<Mutex<Vec<(String, String)>>>;

fn request(s: &mut impl Read) -> Option<(String, String, String)> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    let split = loop {
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => return None,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    };
    let head = String::from_utf8_lossy(&buf[..split]).to_string();
    let want: usize = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap_or(0))).unwrap_or(0);
    let mut body = buf[split..].to_vec();
    while body.len() < want {
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => return None,
            Ok(n) => body.extend_from_slice(&chunk[..n]),
        }
    }
    let mut line = head.lines().next().unwrap_or("").split(' ');
    Some((line.next().unwrap_or("").to_string(), line.next().unwrap_or("").to_string(), String::from_utf8_lossy(&body).to_string()))
}

fn tls_service(answer: impl Fn(&str, &str, &str) -> (u16, String) + Send + Sync + 'static) -> (u16, Asked) {
    let _ = zikaron_net::drive_trust_root(include_bytes!("../../zikaron-net/tests/fixtures/tls/ca.crt.der"));
    let cfg = Arc::new(
        rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .expect("versions")
            .with_no_client_auth()
            .with_single_cert(
                vec![rustls::pki_types::CertificateDer::from(include_bytes!("../../zikaron-net/tests/fixtures/tls/localhost.crt.der").to_vec())],
                rustls::pki_types::PrivateKeyDer::Pkcs8(rustls::pki_types::PrivatePkcs8KeyDer::from(include_bytes!("../../zikaron-net/tests/fixtures/tls/localhost.key.der").to_vec())),
            )
            .expect("the certificate"),
    );
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let asked: Asked = Arc::new(Mutex::new(Vec::new()));
    let answer = Arc::new(answer);
    let l6 = std::net::TcpListener::bind(("::1", port)).ok();
    for l in std::iter::once(l).chain(l6) {
        let (cfg, asked, answer) = (cfg.clone(), asked.clone(), answer.clone());
        std::thread::spawn(move || {
            for tcp in l.incoming().flatten() {
                let (cfg, asked, answer) = (cfg.clone(), asked.clone(), answer.clone());
                std::thread::spawn(move || {
                    let Ok(conn) = rustls::ServerConnection::new(cfg) else { return };
                    let mut tls = rustls::StreamOwned::new(conn, tcp);
                    let Some((m, t, b)) = request(&mut tls) else { return };
                    let (status, text) = answer(&m, &t, &b);
                    asked.lock().unwrap_or_else(|e| e.into_inner()).push((t, b));
                    let _ = tls.write_all(format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}", text.len()).as_bytes());
                    tls.conn.send_close_notify();
                    let _ = tls.flush();
                });
            }
        });
    }
    (port, asked)
}

fn query<'a>(target: &'a str, k: &str) -> Option<&'a str> {
    target.split_once('?')?.1.split('&').filter_map(|kv| kv.split_once('=')).find(|(x, _)| *x == k).map(|(_, v)| v)
}

fn two_chain_service() -> (u16, Asked) {
    tls_service(|_, t, _| {
        let served = matches!(query(t, "chainid"), Some("1") | Some("5"));
        if !served {
            return (200, r#"{"status":"0","message":"NOTOK","result":"Missing or unsupported chainid parameter"}"#.into());
        }
        match (query(t, "module"), query(t, "action")) {
            (Some("proxy"), Some("eth_blockNumber")) => (200, r#"{"jsonrpc":"2.0","id":1,"result":"0x100"}"#.into()),
            (Some("logs"), _) => (200, r#"{"status":"0","message":"No records found","result":[]}"#.into()),
            _ => (200, r#"{"status":"0","message":"NOTOK","result":"Error! Invalid action"}"#.into()),
        }
    })
}

fn instance_of_a() -> (u16, Asked) {
    tls_service(|m, _, b| {
        if m == "GET" {
            return (200, r#"{"message":"Unknown module","result":null,"status":"0"}"#.into());
        }
        let id = b.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
        let result = if b.contains("eth_chainId") { "\"0x1\"" } else if b.contains("eth_getLogs") { "[]" } else { "\"0x100\"" };
        (200, format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{result}}}"))
    })
}

fn row(port: u16, chain: u64) -> Endpoint {
    Endpoint::at(chain, NodeAddr::rest(&format!("https://localhost:{port}/api"), chain))
}

#[test]
fn a_service_serving_two_chains_is_a_node_of_each_and_an_instance_of_one_is_left_out_of_the_other() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let (multi, multi_asked) = two_chain_service();
    let (inst, _) = instance_of_a();
    let (keyed, _) = two_chain_service();
    zikaron_anchor::rest::hold_keys(vec![(format!("localhost:{multi}"), KEY.into()), (format!("localhost:{inst}"), KEY.into()), (format!("localhost:{keyed}"), KEY.into())]);

    let (kept, left) = node::serving(&[row(multi, A)], A).expect("chain A served");
    assert_eq!((kept.len(), left.len()), (1, 0), "{left:?}");
    let (kept, left) = node::serving(&[row(multi, B)], B).expect("chain B served by the same service");
    assert_eq!((kept.len(), left.len()), (1, 0), "a service serving both chains is a node of the second too: {left:?}");
    let chains_asked: Vec<String> = multi_asked.lock().unwrap_or_else(|e| e.into_inner()).iter().filter_map(|(t, _)| query(t, "chainid").map(str::to_string)).collect();
    assert!(chains_asked.contains(&"1".to_string()) && chains_asked.contains(&"5".to_string()), "each chain asked of the service: {chains_asked:?}");
    let before = multi_asked.lock().unwrap_or_else(|e| e.into_inner()).len();
    let _ = node::serving(&[row(multi, A)], A).expect("A");
    let _ = node::serving(&[row(multi, B)], B).expect("B");
    assert_eq!(multi_asked.lock().unwrap_or_else(|e| e.into_inner()).len(), before, "once per chain per process");

    let (kept, left) = node::serving(&[row(inst, A)], A).expect("the instance serves A");
    assert_eq!((kept.len(), left.len()), (1, 0), "{left:?}");
    match node::serving(&[row(inst, B)], B) {
        Err(t) => {
            assert_eq!(t.code, zikaron_send::Code::WrongChain);
            let said = format!("{:?}", t.why);
            assert!(said.contains("WrongChainNode") && said.contains("serves: 1"), "named as serving its own chain: {said}");
        }
        Ok(x) => panic!("an instance of chain A is no node of chain B: {x:?}"),
    }
    let (kept, left) = node::serving(&[row(multi, B), row(inst, B)], B).expect("B through the service");
    assert_eq!((kept.len(), left.len()), (1, 1), "the instance left out beside the service: {left:?}");

    let nets: Vec<Net> = [A, B].iter().map(|c| Net { chain: *c, registry: [0x11; 20], from_block: 0x10, nodes: vec![NodeAddr::rest(&format!("https://localhost:{keyed}/api"), *c)] }).collect();
    assert_eq!(key_of(&nets, &[0x22; 20]), Found::NoAnchor, "both networks read through by the one service");
}
