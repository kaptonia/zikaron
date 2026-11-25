use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::{Arc, Mutex};

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");
const KEY: &str = "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";
const FROM: &str = "0x70997970c51812dc3a010c7d01b50e0d17dc79c8";
fn node(balance: Option<String>) -> (String, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let asked: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let a = asked.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (a, balance) = (a.clone(), balance.clone());
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
                let calls: Vec<String> = if body.trim_start().starts_with('[') { body.split("{\"jsonrpc\"").skip(1).map(|c| format!("{{\"jsonrpc\"{c}")).collect() } else { vec![body.clone()] };
                let mut answers = Vec::new();
                for call in &calls {
                    let method = call.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                    let id = call.split("\"id\":").nth(1).and_then(|r| r.split([',', '}', ']']).next()).unwrap_or("1").trim().to_string();
                    a.lock().unwrap_or_else(|e| e.into_inner()).push(method.clone());
                    let ok = |r: &str| format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{r}}}");
                    let err = |e: &str| format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{e}}}");
                    answers.push(match method.as_str() {
                        "eth_chainId" => ok("\"0x7a69\""),
                        "eth_blockNumber" => ok("\"0x40\""),
                        "eth_getBlockByNumber" => ok("{\"baseFeePerGas\":\"0x3b9aca00\",\"number\":\"0x40\",\"timestamp\":\"0x64\"}"),
                        "eth_feeHistory" => ok("{\"oldestBlock\":\"0x2d\",\"reward\":[[\"0x5f5e100\"]]}"),
                        "eth_estimateGas" => ok("\"0x5208\""),
                        "eth_getLogs" => ok("[]"),
                        "eth_getTransactionCount" => ok("\"0x0\""),
                        "eth_getBalance" => match &balance {
                            Some(b) => ok(&format!("\"{b}\"")),
                            None => err("{\"code\":-32005,\"message\":\"daily request limit exceeded\"}"),
                        },
                        "eth_sendRawTransaction" => err("{\"code\":-32000,\"message\":\"Insufficient funds for gas * price + value\"}"),
                        _ => ok("null"),
                    });
                }
                let body = if calls.len() > 1 { format!("[{}]", answers.join(",")) } else { answers.pop().unwrap_or_default() };
                let _ = s.write_all(format!("HTTP/1.1 200 X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    (url, asked)
}

fn own_home() -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!("zk-cli-funds-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)))
}

fn send(nodes: &[(String, Arc<Mutex<Vec<String>>>)], hashes: usize) -> (i32, String, Vec<String>) {
    let mut args: Vec<String> = vec!["anchor".into()];
    for (url, _) in nodes {
        args.extend(["--endpoint".into(), format!("31337={url}")]);
    }
    args.extend(["--key".into(), KEY.into(), "--form".into(), "registry".into(), "--registry".into(), "0x5fbdb2315678afecb367f032d93f642f64180aa3".into(), "--wait-secs".into(), "0".into()]);
    for i in 0..hashes {
        args.extend(["--hash".into(), format!("0x{:064x}", 0x77 + i)]);
    }
    let o = Command::new(BIN).args(&args).env(zikaron_os::HOME_VAR, own_home()).output().expect("zikaron runs");
    let asked: Vec<String> = nodes.iter().flat_map(|(_, a)| a.lock().unwrap_or_else(|e| e.into_inner()).clone()).collect();
    (o.status.code().unwrap_or(-1), String::from_utf8_lossy(&o.stdout).to_string(), asked)
}

fn member(out: &str, k: &str) -> String {
    zikaron::json::parse(out.as_bytes()).ok().and_then(|v| v.member(k).and_then(|x| x.as_str()).map(str::to_string)).unwrap_or_default()
}

fn broadcast(asked: &[String]) -> bool {
    asked.iter().any(|m| m == "eth_sendRawTransaction")
}

fn need_of(detail: &str) -> u128 {
    detail.split_whitespace().skip_while(|w| *w != "need").nth(1).and_then(|n| n.parse().ok()).unwrap_or(0)
}

#[test]
fn a_short_balance_is_refused_by_name_before_anything_is_broadcast() {
    for (nodes, hashes) in [(1usize, 1usize), (2, 1), (1, 3)] {
        let all: Vec<(String, Arc<Mutex<Vec<String>>>)> = (0..nodes).map(|_| node(Some("0x1".into()))).collect();
        let (code, out, asked) = send(&all, hashes);
        assert_eq!((code, member(&out, "reason").as_str(), member(&out, "token").as_str()), (1, "E_SEND_REFUSED", "INSUFFICIENT_FUNDS"), "{nodes} nodes, {hashes} hashes: {out}");
        let detail = member(&out, "detail");
        assert!(detail.contains("have 1") && detail.contains(FROM) && need_of(&detail) > 1, "{detail}");
        assert!(!broadcast(&asked), "nothing broadcast: {asked:?}");
        let need = need_of(&detail);
        let all: Vec<(String, Arc<Mutex<Vec<String>>>)> = (0..nodes).map(|_| node(Some(format!("0x{need:x}")))).collect();
        let (_, out, asked) = send(&all, hashes);
        assert!(broadcast(&asked), "exactly enough goes on: {out} {asked:?}");
        let all: Vec<(String, Arc<Mutex<Vec<String>>>)> = (0..nodes).map(|_| node(Some(format!("0x{:x}", need - 1)))).collect();
        let (code, out, asked) = send(&all, hashes);
        assert_eq!((code, member(&out, "token").as_str()), (1, "INSUFFICIENT_FUNDS"), "one wei short: {out}");
        assert!(!broadcast(&asked));
    }
}

#[test]
fn a_balance_not_read_is_no_shortage() {
    let all = vec![node(None)];
    let (code, out, asked) = send(&all, 1);
    assert_eq!((code, member(&out, "reason").as_str()), (4, "E_UNREACHABLE"), "{out}");
    assert_ne!(member(&out, "token"), "INSUFFICIENT_FUNDS");
    assert!(!broadcast(&asked), "{asked:?}");
}
