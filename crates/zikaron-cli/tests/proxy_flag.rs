//! `--proxy` on the verbs that contact nodes, on the real binary against an in-process node, HTTP CONNECT stub
//! and SOCKS5 stub (no network: only the stubs resolve the node's name). Cases: a proxy address of either kind
//! is used; `none` resolves locally; `system` and no flag follow the system settings; anything else is misuse
//! (exit 2, nothing on stdout, the flag named).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command;
use std::sync::{Arc, Mutex};

const BIN: &str = env!("CARGO_BIN_EXE_zikaron");

/// A node answering every JSON-RPC request by method, one request per connection.
fn node() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
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
                let id = body.split("\"id\":").nth(1).and_then(|r| r.split(|c| c == ',' || c == '}').next()).unwrap_or("1").trim().to_string();
                let result = if body.contains("eth_chainId") {
                    "\"0x7a69\""
                } else if body.contains("eth_blockNumber") {
                    "\"0x20\""
                } else if body.contains("eth_getLogs") {
                    "[]"
                } else if body.contains("eth_getBlockByNumber") {
                    "{\"number\":\"0x20\",\"timestamp\":\"0x66e00000\"}"
                } else {
                    "null"
                };
                let answer = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{result}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}", answer.len()).as_bytes());
            });
        }
    });
    port
}

fn pipe(a: TcpStream, b: TcpStream) {
    let (mut a2, mut b2) = (a.try_clone().expect("clone"), b.try_clone().expect("clone"));
    let (mut a, mut b) = (a, b);
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut a2, &mut b2);
        let _ = b2.shutdown(std::net::Shutdown::Write);
    });
    let _ = std::io::copy(&mut b, &mut a);
    let _ = a.shutdown(std::net::Shutdown::Write);
}

/// A stub proxy (`socks`: SOCKS5, else HTTP CONNECT) that routes every target to local port `to` and records
/// the targets requested.
fn stub(socks: bool, to: u16) -> (u16, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let seen = s2.clone();
            std::thread::spawn(move || {
                let mut s = s;
                let place = if socks {
                    let mut g = [0u8; 3];
                    if s.read_exact(&mut g).is_err() || s.write_all(&[5, 0]).is_err() {
                        return;
                    }
                    let mut h = [0u8; 5];
                    if s.read_exact(&mut h).is_err() {
                        return;
                    }
                    let mut rest = vec![0u8; h[4] as usize + 2];
                    if s.read_exact(&mut rest).is_err() {
                        return;
                    }
                    let n = rest.len() - 2;
                    let p = u16::from_be_bytes([rest[n], rest[n + 1]]);
                    let _ = s.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
                    format!("{}:{p}", String::from_utf8_lossy(&rest[..n]))
                } else {
                    let mut head = Vec::new();
                    let mut one = [0u8; 1];
                    while !head.ends_with(b"\r\n\r\n") {
                        if s.read(&mut one).unwrap_or(0) == 0 {
                            return;
                        }
                        head.push(one[0]);
                    }
                    let _ = s.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
                    String::from_utf8_lossy(&head).split_whitespace().nth(1).unwrap_or("").to_string()
                };
                seen.lock().expect("seen").push(place);
                if let Ok(up) = TcpStream::connect(("127.0.0.1", to)) {
                    pipe(s, up);
                }
            });
        }
    });
    (port, seen)
}

struct Ran {
    code: i32,
    out: Vec<u8>,
    err: String,
}

fn scan(dir: &std::path::Path, node: u16, proxy: Option<&str>, env: &[(&str, String)]) -> Ran {
    let basis = dir.join("basis.json");
    std::fs::write(
        &basis,
        br#"{"adoptionChains":[],"bareTx":[],"chains":[{"chainId":31337,"fromBlock":0,"registries":["0x5fbdb2315678afecb367f032d93f642f64180aa3"],"senders":["0x70997970c51812dc3a010c7d01b50e0d17dc79c8"],"toBlock":2}]}"#,
    )
    .expect("basis");
    let endpoint = format!("31337=http://node.invalid:{node}");
    let mut args = vec!["scan", "--endpoint", &endpoint, "--basis", basis.to_str().expect("path")];
    if let Some(p) = proxy {
        args.extend(["--proxy", p]);
    }
    let mut c = Command::new(BIN);
    c.args(&args).current_dir(dir).env(zikaron_os::HOME_VAR, own_home()).env_remove("https_proxy").env_remove("HTTPS_PROXY").env_remove("http_proxy").env_remove("HTTP_PROXY").env_remove("all_proxy").env_remove("ALL_PROXY");
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().expect("zikaron runs");
    Ran { code: o.status.code().unwrap_or(-1), out: o.stdout, err: String::from_utf8_lossy(&o.stderr).to_string() }
}

/// A separate user home per run: `anchor` records what it sent under the user's home (`zikaron_cli::sent`), so
/// runs never share a record or touch the real one.
fn own_home() -> std::path::PathBuf {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!("zk-cli-home-{}-{}", std::process::id(), N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)))
}

fn scratch(tag: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("zk-cli-proxy-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("scratch");
    p
}

/// A proxy address of either kind is used; the proxy resolves the node's name.
#[test]
fn a_proxy_address_is_gone_through() {
    let dir = scratch("through");
    let n = node();
    let (socks, socks_seen) = stub(true, n);
    let (connect, connect_seen) = stub(false, n);
    let r = scan(&dir, n, Some(&format!("socks5://127.0.0.1:{socks}")), &[]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(socks_seen.lock().expect("seen").iter().all(|p| *p == format!("node.invalid:{n}")) && !socks_seen.lock().expect("seen").is_empty());
    let r = scan(&dir, n, Some(&format!("http://127.0.0.1:{connect}")), &[]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(connect_seen.lock().expect("seen").iter().all(|p| *p == format!("node.invalid:{n}")) && !connect_seen.lock().expect("seen").is_empty());
    let _ = std::fs::remove_dir_all(&dir);
}

/// `none` resolves the name locally (`.invalid` never resolves): the scan is unanswered, exit 4.
#[test]
fn none_goes_straight() {
    let dir = scratch("none");
    let n = node();
    let r = scan(&dir, n, Some("none"), &[]);
    assert_eq!(r.code, 4, "{}", r.err);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `system` and no flag follow the system settings: where those are environment variables, the proxy set there
/// is used; elsewhere the machine's own settings apply (no stub is named, so the scan is unanswered or a system
/// proxy refuses it).
#[test]
fn system_and_no_flag_follow_the_system() {
    let dir = scratch("system");
    let n = node();
    let (connect, seen) = stub(false, n);
    let env = [("http_proxy", format!("http://127.0.0.1:{connect}"))];
    for proxy in [Some("system"), None] {
        let r = scan(&dir, n, proxy, &env);
        if cfg!(all(unix, not(target_os = "macos"))) {
            assert_eq!(r.code, 0, "{proxy:?}: {}", r.err);
        } else {
            assert_eq!(r.code, 4, "{proxy:?}: {}", r.err);
        }
    }
    assert_eq!(seen.lock().expect("seen").is_empty(), !cfg!(all(unix, not(target_os = "macos"))));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Anything else is misuse: exit 2, nothing on stdout, the flag named on stderr's first line; no node contacted.
#[test]
fn any_other_proxy_is_misuse() {
    let dir = scratch("misuse");
    let n = node();
    for bad in ["proxy.example:8080", "http://u:p@127.0.0.1:1", "https://127.0.0.1:1", "socks4://127.0.0.1:1", "http://127.0.0.1", "http://127.0.0.1:0", "", "SYSTEM2"] {
        let r = scan(&dir, n, Some(bad), &[]);
        assert_eq!((r.code, r.out.is_empty()), (2, true), "{bad:?}: {}", r.err);
        assert!(r.err.lines().next().unwrap_or("").contains("--proxy"), "{bad:?}: {}", r.err);
    }
    let r = Command::new(BIN).args(["anchor", "--endpoint", "31337=http://127.0.0.1:1", "--key", "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d", "--form", "bare", "--hash", &format!("0x{}", "11".repeat(32)), "--proxy", "proxy.example:8080"]).current_dir(&dir).env(zikaron_os::HOME_VAR, own_home()).output().expect("zikaron runs");
    assert_eq!((r.status.code(), r.stdout.is_empty()), (Some(2), true), "anchor takes the same flag");
    let _ = std::fs::remove_dir_all(&dir);
}
