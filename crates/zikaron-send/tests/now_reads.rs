use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use zikaron::json::Value;
use zikaron_send::node::NodeAddr;
use zikaron_send::{batch, Code};

const CHAIN: u64 = 31337;
const REG: [u8; 20] = [0x11; 20];
const ME: [u8; 20] = [0x22; 20];
const HASH: [u8; 32] = [0xa1; 32];

type Answer = Arc<dyn Fn(&str, &Value) -> String + Send + Sync>;

fn node(answer: Answer) -> (String, Arc<Mutex<Vec<String>>>) {
    let l = TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let asked = Arc::new(Mutex::new(Vec::new()));
    let kept = asked.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (answer, kept) = (answer.clone(), kept.clone());
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
                let one = |req: &Value| {
                    let method = req.member("method").and_then(|m| m.as_str()).unwrap_or("").to_string();
                    let params = req.member("params").cloned().unwrap_or(Value::Null);
                    kept.lock().expect("kept").push(method.clone());
                    let id = req.member("id").and_then(Value::as_int).unwrap_or(1);
                    let result = answer(&method, &params);
                    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{result}}}")
                };
                let body = match zikaron::json::parse(body.as_bytes()).unwrap_or(Value::Null) {
                    Value::Arr(reqs) => {
                        kept.lock().expect("kept").push(format!("post:{}", reqs.len()));
                        format!("[{}]", reqs.iter().map(&one).collect::<Vec<_>>().join(","))
                    }
                    req => one(&req),
                };
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    (url, asked)
}

fn hex(b: &[u8]) -> String {
    zikaron::hexfmt::encode(b)
}

fn the_anchor() -> String {
    let mut sender = [0u8; 32];
    sender[12..].copy_from_slice(&ME);
    format!(
        r#"{{"address":"{}","topics":["{}","{}","{}"],"data":"0x","blockNumber":"0x41","transactionHash":"{}","logIndex":"0x0"}}"#,
        hex(&REG),
        hex(&zikaron_anchor::scan::topic0()),
        hex(&sender),
        hex(&HASH),
        hex(&[0x07; 32])
    )
}

fn block_of(v: Option<&Value>) -> Option<u64> {
    v.and_then(Value::as_str).and_then(|s| u64::from_str_radix(s.trim_start_matches("0x"), 16).ok())
}

fn chain_node(head: impl Fn(usize) -> u64 + Send + Sync + 'static) -> (String, Arc<Mutex<Vec<String>>>) {
    let now = Arc::new(Mutex::new(head(0)));
    let asked = AtomicUsize::new(0);
    node(Arc::new(move |method, params| {
        let first = params.as_arr().and_then(|a| a.first());
        match method {
            "eth_chainId" => format!("\"0x{CHAIN:x}\""),
            "eth_blockNumber" => {
                let h = head(asked.fetch_add(1, Ordering::SeqCst));
                *now.lock().expect("now") = h;
                format!("\"0x{h:x}\"")
            }
            "eth_getBlockByNumber" => match block_of(first) {
                Some(n) if n <= *now.lock().expect("now") => format!("{{\"number\":\"0x{n:x}\",\"timestamp\":\"0x{:x}\"}}", 0x64 + 2 * n.saturating_sub(0x40)),
                _ => "null".into(),
            },
            "eth_getLogs" => match block_of(first.and_then(|f| f.member("toBlock"))) {
                Some(to) if to >= 0x41 && *now.lock().expect("now") >= 0x41 => format!("[{}]", the_anchor()),
                _ => "[]".into(),
            },
            _ => "null".into(),
        }
    }))
}

fn urls(nodes: &[&str]) -> Vec<NodeAddr> {
    nodes.iter().map(|u| NodeAddr::new(*u)).collect()
}

fn quiet() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
}

fn asked(a: &Arc<Mutex<Vec<String>>>, method: &str) -> usize {
    a.lock().expect("asked").iter().filter(|m| m.as_str() == method).count()
}

#[test]
fn the_duplicate_check_reads_each_node_up_to_its_own_head() {
    quiet();
    for behind in [0x40u64, 0x41] {
        let (ahead, a) = chain_node(|_| 0x41);
        let (slow, b) = chain_node(move |_| behind);
        let t = batch::on_chain(&urls(&[&ahead, &slow]), CHAIN, &REG, &ME, &[HASH], 0, &[]).expect_err("held already");
        assert_eq!(t.code, Code::OnChain, "{behind:#x}: {t:?}");
        assert_eq!((asked(&a, "eth_blockNumber"), asked(&b, "eth_blockNumber")), (1, 1), "{behind:#x}: one head question each, nothing waited for");
        assert!(asked(&b, "eth_getLogs") >= 1, "{behind:#x}: the node behind is asked too, up to its own head");
    }
}

#[test]
fn the_fence_opens_on_the_first_node_past_the_block_before() {
    quiet();
    let (ahead, _) = chain_node(|_| 0x41);
    let (behind, _) = chain_node(|_| 0x40);
    assert_eq!(batch::fence(&urls(&[&ahead, &behind]), CHAIN, Some(0x40)), Ok(()));
    let (one, _) = chain_node(|_| 0x40);
    let (two, _) = chain_node(|_| 0x40);
    let t = batch::fence(&urls(&[&one, &two]), CHAIN, Some(0x40)).expect_err("no node past it");
    assert_eq!(t.code, Code::TooEarly, "{t:?}");
}

#[test]
fn a_process_that_knows_the_chain_still_asks_what_stands_on_the_head_in_one_post() {
    let (url, asked) = node(Arc::new(|m, _| match m {
        "eth_chainId" => "\"0x7a69\"".into(),
        "eth_blockNumber" => "\"0x40\"".into(),
        "eth_estimateGas" => "\"0x5208\"".into(),
        "eth_getBlockByNumber" => "{\"baseFeePerGas\":\"0x1\",\"number\":\"0x40\",\"timestamp\":\"0x64\"}".into(),
        _ => "null".into(),
    }));
    let eps = vec![zikaron_send::node::Endpoint::at(CHAIN, NodeAddr::new(url.as_str()))];
    let at = Value::Str("0x40".into());
    let one_step = || {
        let _action = zikaron_anchor::action::open(None);
        zikaron_send::node::ask_ahead(&eps, CHAIN, &|_, lowest, _| {
            vec![
                ("eth_estimateGas", Value::Arr(vec![Value::Obj(Vec::new()), Value::Str(format!("0x{lowest:x}"))])),
                ("eth_getBlockByNumber", Value::Arr(vec![at.clone(), Value::Bool(false)])),
            ]
        });
        let _ = zikaron_send::node::serving(&eps, CHAIN).expect("the chain");
        let _ = zikaron_send::node::ask(&eps, "eth_getBlockByNumber", &Value::Arr(vec![at.clone(), Value::Bool(false)])).expect("the block");
        std::mem::take(&mut *asked.lock().expect("asked"))
    };
    let posts = |log: &[String]| {
        let (mut n, mut i) = (0, 0);
        while i < log.len() {
            n += 1;
            i += log[i].strip_prefix("post:").and_then(|k| k.parse::<usize>().ok()).map_or(1, |k| k + 1);
        }
        n
    };
    let first = one_step();
    let second = one_step();
    assert_eq!(second, ["eth_blockNumber", "post:2", "eth_estimateGas", "eth_getBlockByNumber"], "the head alone, then one post; nothing asked again");
    assert_eq!(posts(&second), posts(&first), "as many posts as the first action: {first:?} / {second:?}");
}
