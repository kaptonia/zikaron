//! Shutdown (the app quitting) cuts an exchange in flight within a slice (`SLICE`, never its deadline) and refuses
//! any started afterwards, both with `Fail::Closed`; an exchange that already ended keeps its answer and its
//! connection is not handed out again; reopening lets exchanges through. Its own test binary, since the shutdown
//! flag is process-wide.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};
use zikaron_net::{close_down, cuts, open_up, parse, post_json, set_choice, Ask, Choice, Fail, Kind, Limits, Proxy, SLICE};

/// How long a cut may take: a few slices (the slice is the longest any wait goes before it looks at the flag).
const CUT_WITHIN: Duration = Duration::from_millis(SLICE.as_millis() as u64 * 4);

#[test]
fn closing_down_cuts_what_is_in_flight_and_refuses_what_comes_after() {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let held = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let h = held.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let h = h.clone();
            std::thread::spawn(move || {
                let mut s = s;
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                h.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                // Never answers: only shutdown ends the exchange before its deadline.
                std::thread::sleep(Duration::from_secs(30));
            });
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    let limits = Limits { deadline: Duration::from_secs(20), max_answer: 0 };
    let asking = std::thread::spawn(move || {
        let began = Instant::now();
        (post_json(&t, b"{}", &limits, Ask::Read).err(), began.elapsed())
    });
    while held.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        std::thread::sleep(Duration::from_millis(5));
    }
    let cut_before = cuts();
    close_down();
    let (said, took) = asking.join().expect("the asking thread");
    assert_eq!(said, Some(Fail::Closed));
    assert!(took < CUT_WITHIN, "cut within a slice, not at the deadline: {took:?}");
    assert_eq!(cuts(), cut_before + 1, "the cut leaves its trace: one exchange cut");
    // Not retried after the cut: the peer saw one request.
    assert_eq!(held.load(std::sync::atomic::Ordering::SeqCst), 1);
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    assert_eq!(post_json(&t, b"{}", &limits, Ask::Read).err(), Some(Fail::Closed), "an exchange after closing down never opens");
    assert_eq!(cuts(), cut_before + 1, "refused before it began: not a cut");
    assert_eq!(held.load(std::sync::atomic::Ordering::SeqCst), 1);
    open_up();
    let closed = TcpListener::bind("127.0.0.1:0").expect("bind").local_addr().expect("addr").port();
    let t = parse(&format!("http://127.0.0.1:{closed}/")).expect("address");
    assert!(matches!(post_json(&t, b"{}", &limits, Ask::Read), Err(Fail::Connect(_))), "open again: the exchange goes out");

    // A handshake in flight (the peer accepted and stays silent) is also cut at once.
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let took_it = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let k = took_it.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            k.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::spawn(move || {
                let mut s = s;
                let mut buf = [0u8; 4096];
                while matches!(s.read(&mut buf), Ok(n) if n > 0) {}
            });
        }
    });
    let t = parse(&format!("https://127.0.0.1:{port}/")).expect("address");
    let asking = std::thread::spawn(move || {
        let began = Instant::now();
        (post_json(&t, b"{}", &limits, Ask::Read).err(), began.elapsed())
    });
    while took_it.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(50));
    close_down();
    let (said, took) = asking.join().expect("the asking thread");
    assert_eq!(said, Some(Fail::Closed), "a handshake cut by closing down is said as that");
    assert!(took < CUT_WITHIN, "cut within a slice, not at the deadline: {took:?}");
    open_up();

    // A tunnel in flight (the proxy accepted and stays silent) is also cut at once.
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let proxy = l.local_addr().expect("addr").port();
    let taken = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let k = taken.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            k.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::spawn(move || {
                let mut s = s;
                let mut buf = [0u8; 4096];
                while matches!(s.read(&mut buf), Ok(n) if n > 0) {}
            });
        }
    });
    set_choice(Choice::Manual(Proxy { kind: Kind::Http, host: "127.0.0.1".into(), port: proxy }));
    let t = parse("http://node.invalid:8545/").expect("address");
    let asking = std::thread::spawn(move || {
        let began = Instant::now();
        (post_json(&t, b"{}", &limits, Ask::Read).err(), began.elapsed())
    });
    while taken.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(50));
    close_down();
    let (said, took) = asking.join().expect("the asking thread");
    assert_eq!(said, Some(Fail::Closed), "a tunnel cut by closing down is said as that");
    assert!(took < CUT_WITHIN, "cut within a slice, not at the deadline: {took:?}");
    open_up();
    set_choice(Choice::Off);

    // Half an answer in when the cut comes: closed, not a short answer judged as is.
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let half = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let k = half.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let k = k.clone();
            std::thread::spawn(move || {
                let mut s = s;
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                let _ = s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nab");
                k.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                std::thread::sleep(Duration::from_secs(30));
            });
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    let asking = std::thread::spawn(move || {
        let began = Instant::now();
        (post_json(&t, b"{}", &limits, Ask::Read).err(), began.elapsed())
    });
    while half.load(std::sync::atomic::Ordering::SeqCst) == 0 {
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(50));
    close_down();
    let (said, took) = asking.join().expect("the asking thread");
    assert_eq!(said, Some(Fail::Closed), "half an answer cut by closing down is said as that");
    assert!(took < CUT_WITHIN, "cut within a slice, not at the deadline: {took:?}");
    open_up();

    // An exchange that ended before the cut keeps its answer; its kept connection is dropped by the cut, so the
    // next exchange after reopening comes on a new connection.
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = l.local_addr().expect("addr").port();
    let conns = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let k = conns.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            k.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::thread::spawn(move || {
                let mut s = s;
                let mut buf = [0u8; 4096];
                while matches!(s.read(&mut buf), Ok(n) if n > 0) {
                    if s.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok").is_err() {
                        return;
                    }
                }
            });
        }
    });
    let t = parse(&format!("http://127.0.0.1:{port}/")).expect("address");
    let before = post_json(&t, b"{}", &limits, Ask::Read).map(|a| (a.status, a.body));
    assert_eq!(before, Ok((200, b"ok".to_vec())), "answered before the cut");
    let cut_before = cuts();
    close_down();
    assert_eq!(cuts(), cut_before, "an exchange already over is not cut");
    assert_eq!(before, Ok((200, b"ok".to_vec())), "and kept as answered");
    open_up();
    assert_eq!(post_json(&t, b"{}", &limits, Ask::Read).map(|a| a.body), Ok(b"ok".to_vec()));
    assert_eq!(conns.load(std::sync::atomic::Ordering::SeqCst), 2, "the connection kept before the cut is not handed out after it");
}
