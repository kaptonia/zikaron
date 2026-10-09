//! What counts as a node's answer, and how a question is carried: each against a node started in this
//! process (a local listener, or an in-process endpoint), never the network.
//!
//! An answer is the node's word only when it is a JSON-RPC answer to this question (an object with this
//! `id`, `result` with `error` absent or null, or a non-null `error` without `result`); anything else is read
//! by its HTTP status and never as an empty result. A broadcast is asked exactly once; a read whose
//! connection broke before the answer is asked once more. A receipt wait is one deadline over every
//! question.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zikaron::json::Value;
use zikaron_anchor::rpc::{self, Endpoint, Http, Replay, Trouble};
use zikaron_anchor::said::{self, Refusal};
use zikaron_anchor::wire::{self, W};
use zikaron_anchor::send;

/// Read one whole request; `None` when the client closed first.
fn read_request(s: &mut TcpStream) -> Option<Vec<u8>> {
    let mut raw = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        if let Some(i) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&raw[..i]).to_ascii_lowercase();
            let len: usize = head.lines().find_map(|l| l.strip_prefix("content-length:").and_then(|n| n.trim().parse().ok())).unwrap_or(0);
            if raw.len() >= i + 4 + len {
                return Some(raw[i + 4..i + 4 + len].to_vec());
            }
        }
        match s.read(&mut buf) {
            Ok(0) | Err(_) => return None,
            Ok(n) => raw.extend_from_slice(&buf[..n]),
        }
    }
}

/// What the node did with one question: answer with a status and a body (the request's `id` put in for
/// `{id}`), or drop the connection without a byte.
enum Reply {
    Say(u16, String),
    Drop,
}

/// A node answering each question by `rule` over (method, how many questions before); every method asked is
/// kept, in order.
fn node(rule: impl Fn(&str, usize) -> Reply + Send + Sync + 'static) -> (String, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}/", l.local_addr().expect("addr"));
    let asked: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let (a, rule) = (asked.clone(), Arc::new(rule));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (a, rule) = (a.clone(), rule.clone());
            std::thread::spawn(move || {
                let mut s = s;
                while let Some(body) = read_request(&mut s) {
                    let v = zikaron::json::parse(&body).unwrap_or(Value::Null);
                    let member = |k: &str| match &v {
                        Value::Obj(m) => m.iter().find(|(n, _)| n == k).map(|(_, x)| x.clone()),
                        _ => None,
                    };
                    let id = member("id").map(|i| String::from_utf8_lossy(&zikaron::json::canon_bytes(&i)).to_string()).unwrap_or_default();
                    let method = match member("method") {
                        Some(Value::Str(m)) => m,
                        _ => String::new(),
                    };
                    let n = {
                        let mut g = a.lock().unwrap_or_else(|e| e.into_inner());
                        g.push(method.clone());
                        g.len() - 1
                    };
                    match rule(&method, n) {
                        Reply::Drop => return,
                        Reply::Say(status, body) => {
                            let body = body.replace("{id}", &id);
                            let head = format!("HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                            let _ = s.write_all(head.as_bytes());
                            let _ = s.write_all(body.as_bytes());
                            return;
                        }
                    }
                }
            });
        }
    });
    (url, asked)
}

fn ask_once(status: u16, body: &str) -> Result<W, Trouble> {
    let body = body.to_string();
    let (url, _) = node(move |_, _| Reply::Say(status, body.clone()));
    Http::new(&url).expect("an endpoint").call("eth_getTransactionByHash", &Value::Arr(vec![Value::Str("0x01".into())]))
}

fn shapeless(t: &Result<W, Trouble>) -> bool {
    matches!(t, Err(Trouble::Transport(e)) if rpc::is_shapeless(e))
}

/// Eight answer forms: a JSON page without `result` or `error`, another request's `id`, or an array is a wrong
/// shape (never an empty result); 429, 403 and 502 are read by status; `result: null` is a valid empty answer;
/// `error: null` beside a `result` is the result.
#[test]
fn only_an_answer_to_this_question_is_the_nodes_word() {
    assert!(shapeless(&ask_once(200, "{\"message\":\"Too Many Requests\"}")), "①");
    assert_eq!(ask_once(429, "{\"message\":\"Too Many Requests\"}").err().as_ref().and_then(said::refusal), Some(Refusal::RateLimited), "②");
    assert_eq!(ask_once(403, "<html>forbidden</html>").err().as_ref().and_then(said::refusal), Some(Refusal::Auth), "③");
    match ask_once(502, "<html>bad gateway</html>") {
        Err(Trouble::Transport(e)) => assert_eq!(rpc::status_of(&e), Some(502), "④ {e}"),
        other => panic!("④ {other:?}"),
    }
    assert!(shapeless(&ask_once(200, "{\"jsonrpc\":\"2.0\",\"id\":99,\"result\":\"0x1\"}")), "⑤");
    assert_eq!(ask_once(200, "{\"id\":{id},\"result\":null}").map(|v| v.is_null()), Ok(true), "⑥");
    assert_eq!(ask_once(200, "{\"id\":{id},\"error\":null,\"result\":\"0x1\"}").ok().and_then(|v| v.as_str().map(str::to_string)), Some("0x1".into()), "⑦");
    assert!(shapeless(&ask_once(200, "[{\"id\":{id},\"result\":\"0x1\"}]")), "⑧");
    // Both, and neither: no answer.
    assert!(shapeless(&ask_once(200, "{\"id\":{id},\"error\":{\"code\":1,\"message\":\"x\"},\"result\":\"0x1\"}")));
    assert!(shapeless(&ask_once(200, "{\"id\":{id}}")));
    // An error the node could not number (`id` null or absent) is still the node's refusal, read by its words;
    // a result never is without this question's `id`.
    assert_eq!(ask_once(200, "{\"id\":null,\"error\":{\"code\":-32005,\"message\":\"rate limit exceeded\"}}").err().as_ref().and_then(said::refusal), Some(Refusal::RateLimited));
    assert_eq!(ask_once(429, "{\"error\":{\"code\":-32600,\"message\":\"invalid request\"}}").err().as_ref().and_then(said::refusal).map(|r| matches!(r, Refusal::Coded(_))), Some(true));
    assert!(shapeless(&ask_once(200, "{\"id\":null,\"result\":\"0x1\"}")));
    assert!(shapeless(&ask_once(200, "{\"result\":\"0x1\"}")));
    assert!(shapeless(&ask_once(200, "{\"id\":7,\"error\":{\"code\":1,\"message\":\"x\"}}")));
    // A well-formed error is the node's word whatever the status; a 2xx page that is not JSON is not JSON.
    assert!(matches!(ask_once(500, "{\"id\":{id},\"error\":{\"code\":-32000,\"message\":\"no\"}}"), Err(Trouble::Node(_))));
    assert!(matches!(ask_once(400, "{\"id\":{id},\"result\":\"0x2\"}"), Ok(_)));
    assert!(matches!(ask_once(200, "<html>ok</html>"), Err(Trouble::Transport(e)) if rpc::is_not_json(&e)));
    // A status without the node's word names the status, the gateway's JSON included.
    assert_eq!(ask_once(503, "{\"message\":\"down\"}").err().and_then(|t| match t { Trouble::Transport(e) => rpc::status_of(&e), _ => None }), Some(503));
}

/// The replay reads a recording as a live answer is read: an exchange holding neither `result` nor `error`
/// holds no answer (the question is not served, never replayed as empty); `error: null` beside a `result` is
/// the result.
#[test]
fn a_recorded_exchange_without_an_answer_is_no_answer() {
    let w = |j: &str| wire::parse(j.as_bytes()).expect("JSON");
    let ex = vec![
        w("{\"method\":\"eth_getTransactionByHash\",\"params\":[\"0x01\"]}"),
        w("{\"error\":null,\"method\":\"eth_chainId\",\"params\":[],\"result\":\"0x1\"}"),
        w("{\"error\":{\"code\":1,\"message\":\"x\"},\"method\":\"eth_blockNumber\",\"params\":[],\"result\":\"0x5\"}"),
    ];
    let mut r = Replay::new("t", &ex).expect("a recording");
    assert!(matches!(r.call("eth_getTransactionByHash", &Value::Arr(vec![Value::Str("0x01".into())])), Err(Trouble::NotServed(_))));
    assert_eq!(r.call("eth_chainId", &Value::Arr(vec![])).ok().and_then(|v| v.as_str().map(str::to_string)), Some("0x1".into()));
    assert!(matches!(r.call("eth_blockNumber", &Value::Arr(vec![])), Err(Trouble::NotServed(_))));
}

/// A read whose connection breaks before any byte is asked once more; a broadcast whose connection breaks is
/// asked exactly once (sending moves on and asks by hash instead).
#[test]
fn a_broadcast_is_asked_exactly_once_and_a_read_once_more() {
    let (url, asked) = node(|m, n| match (m, n) {
        ("eth_blockNumber", 0) => Reply::Drop,
        ("eth_blockNumber", _) => Reply::Say(200, "{\"id\":{id},\"result\":\"0x5\"}".into()),
        _ => Reply::Drop,
    });
    let mut ep = Http::new(&url).expect("an endpoint");
    assert_eq!(ep.call("eth_blockNumber", &Value::Arr(vec![])).ok().and_then(|v| v.as_str().map(str::to_string)), Some("0x5".into()));
    assert!(ep.call(rpc::BROADCAST, &Value::Arr(vec![Value::Str("0x00".into())])).is_err());
    assert_eq!(*asked.lock().expect("asked"), vec!["eth_blockNumber", "eth_blockNumber", rpc::BROADCAST]);
}

/// The command line's anchor: the broadcast is asked exactly once even when its connection breaks, and an
/// estimate refused for the node's own reasons (rate limit, credentials, missing method, wrong chain) is a
/// network trouble; only a refusal about the call itself (a revert, an unrecognised coded error) means "the
/// call would revert".
#[test]
fn an_estimate_is_refused_by_the_one_table_and_a_broadcast_goes_once() {
    let estimate_says = |reply: fn() -> Reply| {
        let (url, asked) = node(move |m, _| match m {
            "eth_blockNumber" => Reply::Say(200, "{\"id\":{id},\"result\":\"0x10\"}".into()),
            "eth_getBlockByNumber" => Reply::Say(200, "{\"id\":{id},\"result\":{\"baseFeePerGas\":\"0x1\"}}".into()),
            "eth_feeHistory" => Reply::Say(200, "{\"id\":{id},\"result\":{\"reward\":[[\"0x1\"]]}}".into()),
            "eth_estimateGas" => reply(),
            "eth_getTransactionCount" => Reply::Say(200, "{\"id\":{id},\"result\":\"0x0\"}".into()),
            _ => Reply::Drop,
        });
        let mut ep = Http::new(&url).expect("an endpoint");
        let got = send::anchor_estimated(&mut ep, &[7u8; 32], 31337, send::Form::Bare, None, &[[0xaa; 32]], None, Duration::ZERO);
        let broadcasts = asked.lock().expect("asked").iter().filter(|m| *m == rpc::BROADCAST).count();
        (got.err(), broadcasts)
    };
    let network = |e: &Option<send::NotSent>| matches!(e, Some(send::NotSent::Gas(send::NoGas::Network(_))));
    let refused = |e: &Option<send::NotSent>| matches!(e, Some(send::NotSent::Gas(send::NoGas::Refused(_))));
    for (form, reply) in [
        ("429", (|| Reply::Say(429, "Too Many Requests".into())) as fn() -> Reply),
        ("rate", || Reply::Say(200, "{\"id\":{id},\"error\":{\"code\":-32005,\"message\":\"daily request limit exceeded\"}}".into())),
        ("auth", || Reply::Say(200, "{\"id\":{id},\"error\":{\"code\":-32000,\"message\":\"unauthorized: bad api key\"}}".into())),
        ("method", || Reply::Say(200, "{\"id\":{id},\"error\":{\"code\":-32601,\"message\":\"the method does not exist/is not available\"}}".into())),
        ("chain", || Reply::Say(200, "{\"id\":{id},\"error\":{\"code\":-32000,\"message\":\"invalid chain id\"}}".into())),
        ("uncoded", || Reply::Say(200, "{\"id\":{id},\"error\":{\"message\":\"node says no\"}}".into())),
    ] {
        let (e, sends) = estimate_says(reply);
        assert!(network(&e), "{form}: {e:?}");
        assert_eq!(sends, 0, "{form}: nothing sent");
    }
    for (form, reply) in [
        ("revert", (|| Reply::Say(200, "{\"id\":{id},\"error\":{\"code\":3,\"message\":\"execution reverted\"}}".into())) as fn() -> Reply),
        ("coded", || Reply::Say(200, "{\"id\":{id},\"error\":{\"code\":-32000,\"message\":\"node says no\"}}".into())),
    ] {
        let (e, sends) = estimate_says(reply);
        assert!(refused(&e), "{form}: {e:?}");
        assert_eq!(sends, 0, "{form}: nothing sent");
    }
    // Estimated: the broadcast's connection breaks, and it is not asked again; the signed hash goes with the
    // trouble (the bytes may be in a pool).
    let (e, sends) = estimate_says(|| Reply::Say(200, "{\"id\":{id},\"result\":\"0x5208\"}".into()));
    assert!(matches!(e, Some(send::NotSent::Broadcast(_, _))), "{e:?}");
    assert_eq!(sends, 1, "the broadcast is asked exactly once");
}

/// A receipt wait is one deadline over every question: a node that never answers cannot hold it past the wait
/// by a deadline of its own, and once the wait is spent no further node is asked.
#[test]
fn a_receipt_wait_is_one_deadline_over_every_question() {
    let held = Arc::new(AtomicUsize::new(0));
    let h = held.clone();
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let silent = format!("http://{}/", l.local_addr().expect("addr"));
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            h.fetch_add(1, Ordering::SeqCst);
            std::thread::spawn(move || {
                let mut s = s;
                let _ = read_request(&mut s);
                std::thread::sleep(Duration::from_secs(10));
            });
        }
    });
    let (other, asked) = node(|_, _| Reply::Say(200, "{\"id\":{id},\"result\":null}".into()));
    // Each endpoint's own deadline is far longer than the wait.
    let mut a = Http::new(&silent).expect("endpoint").with_limits(rpc::Limits { deadline: Duration::from_secs(8), max_answer: 0 });
    let mut b = Http::new(&other).expect("endpoint").with_limits(rpc::Limits { deadline: Duration::from_secs(8), max_answer: 0 });
    let began = Instant::now();
    let got = send::confirm_each(&mut [&mut a, &mut b], &[1u8; 32], Duration::from_millis(500), &[]);
    let took = began.elapsed();
    assert!(took < Duration::from_millis(500) + Duration::from_millis(1500), "took {took:?}");
    assert!(matches!(got, send::Confirm::Unreachable(_)), "{got:?}");
    assert_eq!(held.load(Ordering::SeqCst), 1);
    assert!(asked.lock().expect("asked").is_empty(), "the wait was spent on the first node: the second is not asked");
}

/// The receipt wait's edges: no endpoint is said as none; a wait of zero asks exactly one round; a node
/// that refuses and a node that answers "not yet" make "not yet", not "out of sight".
#[test]
fn a_receipt_wait_of_zero_asks_one_round() {
    struct Count(usize, Result<W, Trouble>);
    impl Endpoint for Count {
        fn call(&mut self, _: &str, _: &Value) -> Result<W, Trouble> {
            self.0 += 1;
            self.1.clone()
        }
        fn name(&self) -> String {
            "count".into()
        }
        /// No address: the place is the name.
        fn place(&self) -> String {
            self.name()
        }
    }
    assert!(matches!(send::confirm_each(&mut [], &[1u8; 32], Duration::ZERO, &[]), send::Confirm::Unreachable(_)));
    let mut refusing = Count(0, Err(Trouble::Node("{\"code\":-32005,\"message\":\"rate limit\"}".into())));
    let mut empty = Count(0, Ok(W::of(wire::Body::Null)));
    let got = send::confirm_each(&mut [&mut refusing, &mut empty], &[1u8; 32], Duration::ZERO, &[]);
    assert!(matches!(got, send::Confirm::NotYet), "{got:?}");
    assert_eq!((refusing.0, empty.0), (1, 1));
}
