//! Every request this crate sends carries exactly one `User-Agent` header with the product name only
//! (`USER_AGENT`; no version, OS or machine), from the single head builder: a POST to a node, a GET of a remote
//! file, and a proxy CONNECT (in-process stubs, no network).

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zikaron_net::{get, parse, post_json, read_system_with, set_choice, Ask, Choice, Kind, Limits, Proxy, SystemProxies, USER_AGENT};

fn limits() -> Limits {
    Limits { deadline: Duration::from_secs(5), max_answer: 0 }
}

/// Read one request head (to its blank line) from `s`.
fn head_of(s: &mut TcpStream) -> String {
    let mut head = Vec::new();
    let mut one = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        if s.read(&mut one).unwrap_or(0) == 0 {
            break;
        }
        head.push(one[0]);
    }
    String::from_utf8_lossy(&head).into_owned()
}

/// A server answering `{}` to every request, recording each request head.
fn place() -> (u16, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let kept = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let kept = kept.clone();
            std::thread::spawn(move || {
                let mut s = s;
                let head = head_of(&mut s);
                let length = head.lines().find_map(|l| l.strip_prefix("Content-Length: ")).and_then(|n| n.trim().parse::<usize>().ok()).unwrap_or(0);
                let mut body = vec![0u8; length];
                let _ = s.read_exact(&mut body);
                kept.lock().expect("kept").push(head);
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
            });
        }
    });
    (port, seen)
}

/// An HTTP CONNECT proxy that pipes every tunnel to `to`, recording each CONNECT head.
fn connect_proxy(to: u16) -> (u16, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let kept = seen.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let kept = kept.clone();
            std::thread::spawn(move || {
                let mut s = s;
                kept.lock().expect("kept").push(head_of(&mut s));
                let _ = s.write_all(b"HTTP/1.1 200 Connection established\r\n\r\n");
                if let Ok(up) = TcpStream::connect(("127.0.0.1", to)) {
                    let (mut a, mut b) = (s.try_clone().expect("clone"), up.try_clone().expect("clone"));
                    let (mut s, mut up) = (s, up);
                    std::thread::spawn(move || {
                        let _ = std::io::copy(&mut a, &mut b);
                    });
                    let _ = std::io::copy(&mut up, &mut s);
                }
            });
        }
    });
    (port, seen)
}

/// The `User-Agent` lines of a head (header names compared case-insensitively).
fn agents(head: &str) -> Vec<String> {
    head.lines()
        .filter_map(|l| l.split_once(':'))
        .filter(|(k, _)| k.eq_ignore_ascii_case("user-agent"))
        .map(|(_, v)| v.trim().to_string())
        .collect()
}

#[test]
fn every_request_carries_the_products_name_and_nothing_more() {
    assert_eq!(USER_AGENT, "ZIKARON", "the product's name alone");
    assert!(!USER_AGENT.chars().any(|c| c.is_ascii_digit() || c == '/' || c == ' '), "no version, system or machine in it");
    read_system_with(|| Some(SystemProxies::default()));
    set_choice(Choice::Off);
    let (node, seen) = place();

    let posted = post_json(&parse(&format!("http://127.0.0.1:{node}/rpc")).expect("address"), b"{}", &limits(), Ask::Once);
    assert!(posted.is_ok(), "post: {:?}", posted.err());
    let fetched = get(&parse(&format!("http://127.0.0.1:{node}/file")).expect("address"), &limits());
    assert!(fetched.is_ok(), "get: {:?}", fetched.err());

    let (proxy, connects) = connect_proxy(node);
    set_choice(Choice::Manual(Proxy { kind: Kind::Http, host: "127.0.0.1".into(), port: proxy }));
    let through = post_json(&parse(&format!("http://node.invalid:{node}/rpc")).expect("address"), b"{}", &limits(), Ask::Once);
    set_choice(Choice::Off);
    assert!(through.is_ok(), "through: {:?}", through.err());

    let heads = seen.lock().expect("seen").clone();
    let connects = connects.lock().expect("connects").clone();
    for (form, head) in [("post", heads.first()), ("get", heads.get(1)), ("postThroughProxy", heads.get(2)), ("connect", connects.first())] {
        let head = head.unwrap_or_else(|| panic!("{form}: no request arrived"));
        assert_eq!(agents(head), vec![USER_AGENT.to_string()], "{form}: one line, the name alone: {head}");
    }
    assert!(connects[0].starts_with(&format!("CONNECT node.invalid:{node} HTTP/1.1\r\n")), "the proxy is asked for the node: {}", connects[0]);
}
