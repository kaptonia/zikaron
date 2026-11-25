use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use zikaron_anchor::settled;
use zikaron_send::node::{Endpoint, NodeAddr};

const ROLE: &str = "ZK_SETTLED_ROLE";
const FILE: &str = "ZK_SETTLED_FILE";
const NODES: &str = "ZK_SETTLED_NODES";

const HEAD: u64 = 0x400;
const CHAIN: u64 = 31337;
const REGISTRY: [u8; 20] = [0x11; 20];

struct FileKeep(PathBuf);

impl FileKeep {
    fn part(&self, part: &str) -> PathBuf {
        let name = self.0.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        self.0.with_file_name(format!("{name}-{part}"))
    }
}

impl settled::Keep for FileKeep {
    fn load(&self, part: &str) -> Option<Vec<u8>> {
        std::fs::read(self.part(part)).ok()
    }
    fn save(&self, part: &str, plain: &[u8]) -> Result<(), settled::Unkept> {
        let to = self.part(part);
        let tmp = to.with_extension("tmp");
        if std::fs::write(&tmp, plain).is_ok() {
            let _ = std::fs::rename(&tmp, &to);
        }
        Ok(())
    }
    fn parts(&self) -> Vec<String> {
        let (Some(dir), Some(name)) = (self.0.parent(), self.0.file_name().map(|n| format!("{}-", n.to_string_lossy()))) else { return Vec::new() };
        std::fs::read_dir(dir).into_iter().flatten().flatten().filter_map(|e| e.file_name().to_string_lossy().strip_prefix(&name).map(str::to_string)).collect()
    }
}

fn whole(file: &Path) -> settled::Book {
    settled::read_whole(&FileKeep(file.to_path_buf()))
}

type Asked = Arc<Mutex<Vec<(u64, u64)>>>;

type Methods = Arc<Mutex<Vec<String>>>;

const RAW: &str = "eth_getRawTransactionByHash";

fn node(limit: Option<u64>, serves_raw: bool) -> (String, Asked, Methods) {
    node_to(HEAD, limit, serves_raw)
}

const WIDER_THAN_IT_TAKES: &str = "{\"code\":-32012,\"data\":{\"code\":\"ErrGetLogsExceededMaxAllowedRange\",\"details\":{\"maxAllowedRange\":30000},\"message\":\"getLogs request exceeded max allowed range\"},\"message\":\"getLogs request exceeded max allowed range\"}";

fn node_to(head: u64, limit: Option<u64>, serves_raw: bool) -> (String, Asked, Methods) {
    node_at(Arc::new(std::sync::atomic::AtomicU64::new(head)), limit, serves_raw)
}

fn node_at(head: Arc<std::sync::atomic::AtomicU64>, limit: Option<u64>, serves_raw: bool) -> (String, Asked, Methods) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let asked: Asked = Default::default();
    let methods: Methods = Default::default();
    let log = asked.clone();
    let names = methods.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let log = log.clone();
            let names = names.clone();
            let head = head.load(std::sync::atomic::Ordering::SeqCst);
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
                let method = body.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                let id = body.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                let block = |k: &str| body.split(&format!("\"{k}\":\"0x")).nth(1).and_then(|r| r.split('"').next()).and_then(|h| u64::from_str_radix(h, 16).ok()).unwrap_or(0);
                names.lock().unwrap().push(method.clone());
                let member = match method.as_str() {
                    "eth_chainId" => format!("\"result\":\"0x{CHAIN:x}\""),
                    "eth_blockNumber" => format!("\"result\":\"0x{head:x}\""),
                    "eth_getBlockByNumber" if body.contains("\"0x10\"") => "\"result\":{\"hash\":\"0xaa\",\"number\":\"0x10\"}".to_string(),
                    RAW if serves_raw => "\"result\":\"0x02f8\"".to_string(),
                    RAW => "\"error\":{\"code\":-32601,\"message\":\"the method eth_getRawTransactionByHash does not exist/is not available\"}".to_string(),
                    "eth_getLogs" => {
                        let (from, to) = (block("fromBlock"), block("toBlock"));
                        log.lock().unwrap().push((from, to));
                        match limit {
                            Some(n) if to - from + 1 > n => format!("\"error\":{WIDER_THAN_IT_TAKES}"),
                            _ if (from..=to).contains(&0x10) => "\"result\":[{\"address\":\"0x1111111111111111111111111111111111111111\",\"blockHash\":\"0xaa\",\"blockNumber\":\"0x10\",\"data\":\"0x01\",\"logIndex\":\"0x0\",\"topics\":[],\"transactionHash\":\"0xbb\"}]".to_string(),
                            _ => "\"result\":[]".to_string(),
                        }
                    }
                    _ => "\"result\":null".to_string(),
                };
                let body = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",{member}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    (url, asked, methods)
}

fn scratch(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("zk-settled-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("a scratch place");
    p
}

fn child(test: &str, role: &str, nodes: &[&str], file: &Path) -> Vec<String> {
    child_with(test, role, nodes, file, &[])
}

fn child_with(test: &str, role: &str, nodes: &[&str], file: &Path, vars: &[(&str, &str)]) -> Vec<String> {
    let me = std::env::current_exe().expect("this test binary");
    let out = std::process::Command::new(me)
        .args([test, "--exact", "--nocapture", "--test-threads=1"])
        .env(ROLE, role)
        .env(FILE, file)
        .env(NODES, nodes.join(" "))
        .envs(vars.iter().copied())
        .output()
        .expect("the child runs");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success() && text.contains("1 passed"), "{role}: {}\n{text}\n{}", out.status, String::from_utf8_lossy(&out.stderr));
    text.lines().filter_map(|l| l.split_once("SAID ").map(|(_, said)| said.to_string())).collect()
}

fn child_setup() -> (zikaron_anchor::action::Opened, Vec<String>) {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let action = zikaron_anchor::action::open(Some(Arc::new(FileKeep(PathBuf::from(std::env::var(FILE).expect("a record file"))))));
    (action, std::env::var(NODES).expect("nodes").split(' ').map(str::to_string).collect())
}

fn agreed_scan(nodes: &[String]) {
    let eps: Vec<Endpoint> = nodes.iter().map(|u| Endpoint::at(CHAIN, NodeAddr::new(u.as_str()))).collect();
    let senders = vec![zikaron::hexfmt::encode(&[0x22u8; 20])];
    let registry = zikaron::hexfmt::encode(&REGISTRY);
    let w = zikaron_glue::basis::Window { chain: CHAIN, registries: std::slice::from_ref(&registry), from_block: 0, to_block: HEAD, senders: &senders };
    let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&w));
    match zikaron_send::agree::scan(&eps, &basis, &zikaron_send::Bare).expect("the scan") {
        zikaron_send::agree::Scan::Basis(a) => println!("SAID single={}", a.single_source),
        zikaron_send::agree::Scan::NoLabel { .. } => panic!("a basis"),
    }
}

fn froms(asked: &Asked) -> Vec<u64> {
    asked.lock().unwrap().iter().map(|(f, _)| *f).collect()
}

fn take(asked: &Asked) -> Vec<(u64, u64)> {
    std::mem::take(&mut *asked.lock().unwrap())
}

#[test]
fn a_second_process_asks_the_logs_only_after_what_the_first_settled() {
    const TEST: &str = "a_second_process_asks_the_logs_only_after_what_the_first_settled";
    if std::env::var(ROLE).is_ok() {
        let (_action, nodes) = child_setup();
        return agreed_scan(&nodes);
    }
    let dir = scratch("second");
    let file = dir.join("logs.json");
    let ((a, asked_a, _), (b, asked_b, _)) = (node(None, true), node(None, true));
    assert_eq!(child(TEST, "first", &[&a, &b], &file), vec!["single=false"]);
    assert_eq!((froms(&asked_a), froms(&asked_b)), (vec![0], vec![0]), "the first reads the whole range");
    let kept = whole(&file);
    let e = kept.entries.values().next().expect("an entry");
    assert_eq!((e.from, e.settled, e.logs.len(), e.single_source), (0, HEAD - settled::MARGIN, 1, false), "settled agreed, its log kept");
    take(&asked_a);
    take(&asked_b);
    assert_eq!(child(TEST, "second", &[&a, &b], &file), vec!["single=false"], "served from what was kept, still agreed");
    let after = HEAD - settled::MARGIN + 1;
    assert_eq!((take(&asked_a), take(&asked_b)), (vec![(after, HEAD)], vec![(after, HEAD)]), "only the blocks after the settled one");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn another_node_list_reads_the_whole_range() {
    const TEST: &str = "another_node_list_reads_the_whole_range";
    if std::env::var(ROLE).is_ok() {
        let (_action, nodes) = child_setup();
        return agreed_scan(&nodes);
    }
    let dir = scratch("other-list");
    let file = dir.join("logs.json");
    let ((a, asked_a, _), (b, _, _), (c, asked_c, _)) = (node(None, true), node(None, true), node(None, true));
    child(TEST, "first", &[&a, &b], &file);
    take(&asked_a);
    child(TEST, "other", &[&a, &c], &file);
    assert_eq!((froms(&asked_a), froms(&asked_c)), (vec![0], vec![0]), "both nodes of the new list read from the start block");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_node_refusing_the_whole_range_is_asked_once_and_the_reading_stands_on_the_node_that_answered() {
    if std::env::var(ROLE).is_ok() {
        return;
    }
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    const TOP: u64 = 1_208_000;
    let ((limited, asked_limited, _), (open, asked_open, _)) = (node_to(TOP, Some(30_000), true), node_to(TOP, None, true));
    let eps: Vec<Endpoint> = [&limited, &open].iter().map(|u| Endpoint::at(CHAIN, NodeAddr::new(u.as_str()))).collect();
    let senders = vec![zikaron::hexfmt::encode(&[0x22u8; 20])];
    let registry = zikaron::hexfmt::encode(&REGISTRY);
    let w = zikaron_glue::basis::Window { chain: CHAIN, registries: std::slice::from_ref(&registry), from_block: 0, to_block: TOP, senders: &senders };
    let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&w));
    let a = match zikaron_send::agree::scan(&eps, &basis, &zikaron_send::Bare).expect("the reading of the node that answered") {
        zikaron_send::agree::Scan::Basis(a) => a,
        zikaron_send::agree::Scan::NoLabel { .. } => panic!("a basis"),
    };
    assert_eq!((take(&asked_limited), take(&asked_open)), (vec![(0, TOP), (0, TOP)], vec![(0, TOP)]), "the whole range each, the refusing node asked once more; no piece asked");
    assert!(a.single_source && a.single_source_chains == vec![CHAIN], "one node answered");
    let named = a.unanswered.iter().any(|w| match w {
        zikaron_send::Why::AtChain { chain, why } => *chain == CHAIN && matches!(why.as_ref(), zikaron_send::Why::Each(parts) if matches!(parts.as_slice(), [zikaron_send::Why::Text(m), zikaron_send::Why::Node { node, trouble: zikaron_anchor::rpc::Trouble::Node(said) }] if m == "eth_getLogs" && *node == zikaron_net::sayable(&limited) && said.contains("-32012"))),
        _ => false,
    });
    assert!(named, "the refusing node named with its words: {:?}", a.unanswered);
}

fn raw_then_head(nodes: &[String]) {
    let eps: Vec<Endpoint> = nodes.iter().map(|u| Endpoint::at(CHAIN, NodeAddr::new(u.as_str()))).collect();
    let raw = zikaron_send::node::ask(&eps, RAW, &zikaron::json::Value::Arr(vec![zikaron::json::Value::Str(format!("0x{}", "ab".repeat(32)))])).expect("the raw transaction");
    let named: Vec<String> = raw.unanswered.iter().map(|w| match w {
        zikaron_send::Why::Node { node, trouble: zikaron_anchor::rpc::Trouble::Node(said) } if said.contains("-32601") => node.clone(),
        other => format!("{other:?}"),
    }).collect();
    println!("SAID raw sources={} named={}", raw.sources, named.join(","));
    let head = zikaron_send::node::ask(&eps, "eth_blockNumber", &zikaron::json::Value::Arr(Vec::new())).expect("the head");
    println!("SAID head sources={}", head.sources);
}

fn count(methods: &Methods, m: &str) -> usize {
    methods.lock().unwrap().iter().filter(|x| *x == m).count()
}

#[test]
fn the_next_process_does_not_ask_a_node_the_method_it_said_it_lacks() {
    const TEST: &str = "the_next_process_does_not_ask_a_node_the_method_it_said_it_lacks";
    if std::env::var(ROLE).is_ok() {
        let (_action, nodes) = child_setup();
        return raw_then_head(&nodes);
    }
    let dir = scratch("lacks");
    let file = dir.join("logs.json");
    let ((lacking, _, lacking_asked), (full, _, full_asked)) = (node(None, false), node(None, true));
    let said = |named: &str| vec![format!("raw sources=1 named={named}"), "head sources=2".to_string()];
    let name = zikaron_net::sayable(&lacking);
    assert_eq!(child(TEST, "first", &[&lacking, &full], &file), said(&name));
    assert_eq!((count(&lacking_asked, RAW), count(&full_asked, RAW)), (1, 1), "the first process asks both");
    assert_eq!(count(&lacking_asked, "eth_blockNumber"), 1);
    let kept = whole(&file);
    let lack = kept.notes.lacks.get(&(zikaron_anchor::endpoints::place_of(&lacking), RAW.to_string())).expect("the lack kept by place and method");
    assert!(lack.said.contains("-32601") && lack.at > 0, "{lack:?}");
    assert_eq!(child(TEST, "second", &[&lacking, &full], &file), said(&name), "the next process reads the method from the other node and names this one");
    assert_eq!((count(&lacking_asked, RAW), count(&full_asked, RAW)), (1, 2), "the lacking node is not asked the method again; the other is");
    assert_eq!(count(&lacking_asked, "eth_blockNumber"), 2, "every other method is still asked of it");
    let _ = std::fs::remove_dir_all(&dir);
}

fn limiting_node(limits: Arc<std::sync::atomic::AtomicBool>) -> (String, Methods) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let methods: Methods = Default::default();
    let names = methods.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let (names, limits) = (names.clone(), limits.clone());
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
                let method = body.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                let id = body.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                names.lock().unwrap().push(method.clone());
                let member = match method.as_str() {
                    "eth_chainId" => format!("\"result\":\"0x{CHAIN:x}\""),
                    "eth_blockNumber" => format!("\"result\":\"0x{HEAD:x}\""),
                    RAW if limits.load(std::sync::atomic::Ordering::SeqCst) => "\"error\":{\"code\":-32005,\"message\":\"rate limit exceeded\"}".to_string(),
                    RAW => "\"result\":\"0x02f8\"".to_string(),
                    _ => "\"result\":null".to_string(),
                };
                let body = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",{member}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    (url, methods)
}

fn lack_kept(file: &Path, node: &str) -> bool {
    let b = whole(file);
    b.notes.lacks.contains_key(&(zikaron_anchor::endpoints::place_of(node), RAW.to_string()))
}

#[test]
fn processes_rate_limited_past_patience_never_keep_the_method_lacking() {
    const TEST: &str = "processes_rate_limited_past_patience_never_keep_the_method_lacking";
    if std::env::var(ROLE).is_ok() {
        let (_action, nodes) = child_setup();
        return raw_then_head(&nodes);
    }
    let dir = scratch("limited-never-lacks");
    let file = dir.join("logs.json");
    let limits = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let ((limited, asked), (full, _, _)) = (limiting_node(limits), node(None, true));
    for (k, role) in ["first", "second", "third"].into_iter().enumerate() {
        child(TEST, role, &[&limited, &full], &file);
        assert_eq!(count(&asked, RAW), 3 * (k + 1), "{role}: asked with patience, as in every process");
        assert!(!lack_kept(&file, &limited), "{role}: no lack");
    }
    assert_eq!(count(&asked, "eth_blockNumber"), 3, "every other method is still asked of it");
    let _ = std::fs::remove_dir_all(&dir);
}

const KEPT_EACH: usize = 40;

fn direct_logs(machine: &Path) -> zikaron_send::sent::Logs {
    zikaron_send::sent::logs_at(machine, CHAIN, &[0x22; 20], &REGISTRY, &[0x07; 32])
}

#[test]
fn two_processes_keeping_reads_at_once_each_keep_theirs() {
    const TEST: &str = "two_processes_keeping_reads_at_once_each_keep_theirs";
    if let Ok(role) = std::env::var(ROLE) {
        let machine = PathBuf::from(std::env::var(FILE).expect("a machine folder"));
        let _action = zikaron_anchor::action::open(Some(Arc::new(direct_logs(&machine))));
        for i in 0..KEPT_EACH {
            let key = settled::Key { chain: CHAIN, nodes: role.clone(), filter: format!("{role}-{i}") };
            settled::keep(&key, settled::Entry { from: 0, settled: 1, logs: Vec::new(), single_source: true });
        }
        return;
    }
    let machine = scratch("two-writers");
    let me = std::env::current_exe().expect("this test binary");
    let start = |role: &str| {
        std::process::Command::new(&me)
            .args([TEST, "--exact", "--nocapture", "--test-threads=1"])
            .env(ROLE, role)
            .env(FILE, &machine)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("the child runs")
    };
    let (one, two) = (start("one"), start("two"));
    for (role, c) in [("one", one), ("two", two)] {
        let out = c.wait_with_output().expect("the child ends");
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        assert!(out.status.success() && text.contains("1 passed"), "{role}: {}\n{text}\n{}", out.status, String::from_utf8_lossy(&out.stderr));
    }
    let book = settled::read_whole(&direct_logs(&machine));
    for role in ["one", "two"] {
        let kept = book.entries.keys().filter(|k| k.nodes == role).count();
        assert_eq!(kept, KEPT_EACH, "{role}: every read it kept is there");
    }
    let dir = machine.join(zikaron_send::sent::DIR);
    let mut left: Vec<String> = std::fs::read_dir(&dir).expect("the folder").filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    left.sort();
    assert_eq!(left.len(), 4 * KEPT_EACH + 1, "a part a key, a part a filter and the lock, no temporary file: {left:?}");
    assert!(left.iter().all(|n| n == "logs.lock" || (n.starts_with("logs-") && !n.ends_with(".tmp"))), "{left:?}");
    let _ = std::fs::remove_dir_all(&machine);
}

#[test]
fn the_bare_roads_keep_their_reads_in_the_clear_for_their_owner_naming_no_node() {
    const TEST: &str = "the_bare_roads_keep_their_reads_in_the_clear_for_their_owner_naming_no_node";
    if std::env::var(ROLE).is_ok() {
        zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
        let machine = PathBuf::from(std::env::var(FILE).expect("a machine folder"));
        let _action = zikaron_anchor::action::open(Some(Arc::new(zikaron_send::checked::Logs(zikaron_send::checked::Plain(machine.clone())))));
        let nodes: Vec<String> = std::env::var(NODES).expect("nodes").split(' ').map(str::to_string).collect();
        let eps: Vec<Endpoint> = nodes.iter().map(|u| Endpoint::at(CHAIN, NodeAddr::new(u.as_str()))).collect();
        let senders = vec![zikaron::hexfmt::encode(&[0x22u8; 20])];
        let registry = zikaron::hexfmt::encode(&REGISTRY);
        let w = zikaron_glue::basis::Window { chain: CHAIN, registries: std::slice::from_ref(&registry), from_block: 0, to_block: HEAD, senders: &senders };
        let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&w));
        let facts = zikaron_send::checked::Record(zikaron_send::checked::Plain(machine));
        match zikaron_send::agree::scan(&eps, &basis, &facts).expect("the scan") {
            zikaron_send::agree::Scan::Basis(a) => println!("SAID single={}", a.single_source),
            zikaron_send::agree::Scan::NoLabel { .. } => panic!("a basis"),
        }
        return;
    }
    let machine = scratch("plain");
    let ((a, asked_a, _), (b, asked_b, _)) = (node(None, true), node(None, true));
    assert_eq!(child(TEST, "first", &[&a, &b], &machine), vec!["single=false"]);
    assert_eq!((froms(&asked_a), froms(&asked_b)), (vec![0], vec![0]), "the first reads the whole range");
    let plain = machine.join(zikaron_send::checked::PLAIN_DIR);
    let files: Vec<PathBuf> = std::fs::read_dir(&plain).expect("the folder").flatten().map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| zikaron_send::checked::is_logs(&n.to_string_lossy()))).collect();
    assert!(!files.is_empty(), "kept in the clear");
    let text: String = files.iter().map(|f| String::from_utf8(std::fs::read(f).expect("a part")).expect("text")).collect();
    assert!(files.iter().all(|f| zikaron_os::is_owner_only(f).expect("its mode")), "for its owner alone");
    for url in [&a, &b] {
        assert!(!text.contains(url.trim_start_matches("http://")), "no node's address: {text}");
    }
    assert!(text.contains("sha256:"), "each node by its digest");
    take(&asked_a);
    take(&asked_b);
    assert_eq!(child(TEST, "second", &[&a, &b], &machine), vec!["single=false"], "served from the record in the clear, still agreed");
    let after = HEAD - settled::MARGIN + 1;
    assert_eq!((take(&asked_a), take(&asked_b)), (vec![(after, HEAD)], vec![(after, HEAD)]), "only the blocks after the settled one");
    assert_eq!(zikaron_send::sent::forget_logs(&machine).ok(), Some(true), "check everything again finds it");
    assert!(files.iter().all(|f| !f.exists()), "and removes it");
    let _ = std::fs::remove_dir_all(&machine);
}

fn agreed_scan_to(nodes: &[String], top: u64) {
    let eps: Vec<Endpoint> = nodes.iter().map(|u| Endpoint::at(CHAIN, NodeAddr::new(u.as_str()))).collect();
    let senders = vec![zikaron::hexfmt::encode(&[0x22u8; 20])];
    let registry = zikaron::hexfmt::encode(&REGISTRY);
    let w = zikaron_glue::basis::Window { chain: CHAIN, registries: std::slice::from_ref(&registry), from_block: 0, to_block: top, senders: &senders };
    let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&w));
    match zikaron_send::agree::scan(&eps, &basis, &zikaron_send::Bare).expect("the scan") {
        zikaron_send::agree::Scan::Basis(a) => println!("SAID single={} history={:?}", a.single_source, a.kept_single_chains),
        zikaron_send::agree::Scan::NoLabel { .. } => panic!("a basis"),
    }
}

const TOP: &str = "ZK_SETTLED_TOP";

#[test]
fn a_capped_node_beside_another_reads_two_sources_once_the_history_is_kept() {
    const TEST: &str = "a_capped_node_beside_another_reads_two_sources_once_the_history_is_kept";
    if std::env::var(ROLE).is_ok() {
        let (_action, nodes) = child_setup();
        let top = std::env::var(TOP).ok().and_then(|t| t.parse().ok()).unwrap_or(HEAD);
        return agreed_scan_to(&nodes, top);
    }
    let dir = scratch("capped");
    let file = dir.join("logs.json");
    let head = Arc::new(std::sync::atomic::AtomicU64::new(HEAD));
    let ((a, asked_a, _), (b, asked_b, _)) = (node_at(head.clone(), None, true), node_at(head.clone(), Some(100), true));
    let run = |role: &str, top: u64| child_with(TEST, role, &[&a, &b], &file, &[(TOP, &top.to_string())]);
    assert_eq!(run("first", HEAD), vec!["single=true history=[]"], "the capped node refused the whole range");
    take(&asked_a);
    take(&asked_b);
    assert_eq!(run("second", HEAD), vec![format!("single=false history=[{CHAIN}]")], "both answered the blocks read now");
    let after = HEAD - settled::MARGIN + 1;
    assert_eq!((take(&asked_a), take(&asked_b)), (vec![(after, HEAD)], vec![(after, HEAD)]), "both asked only the blocks after the kept part");
    assert!(whole(&file).entries.values().all(|e| e.single_source), "the record's mark stays: one node read the history");
    head.store(HEAD + 200, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(run("moved", HEAD + 200), vec![format!("single=true history=[{CHAIN}]")], "the new blocks wider than the capped node takes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_node_alone_with_no_record_reads_on_from_another_nodes() {
    const TEST: &str = "a_node_alone_with_no_record_reads_on_from_another_nodes";
    if std::env::var(ROLE).is_ok() {
        let (_action, nodes) = child_setup();
        let filter = zikaron_anchor::scan::LogFilter { addresses: vec![REGISTRY], topics: Vec::new() };
        let top = std::env::var(TOP).ok().and_then(|t| t.parse().ok()).unwrap_or(HEAD);
        for n in &nodes {
            let mut ep = zikaron_send::node::endpoint_at(n).expect("a way to the node");
            let got = zikaron_anchor::scan::logs_over_window(ep.as_mut(), CHAIN, &filter, 0, top);
            println!("SAID {}", got.map(|l| l.len().to_string()).unwrap_or_else(|r| r.code().to_string()));
        }
        return;
    }
    let dir = scratch("alone");
    let file = dir.join("logs.json");
    let head = Arc::new(std::sync::atomic::AtomicU64::new(HEAD));
    let ((a, asked_a, _), (b, asked_b, _)) = (node(None, true), node_at(head.clone(), Some(100), true));
    let first = child(TEST, "first", &[&a, &b], &file);
    assert_eq!(first[0], "1", "the open node reads the whole range: {first:?}");
    assert_eq!(first[1], "1", "the capped node cuts the range on its own and reads it too: {first:?}");
    take(&asked_a);
    take(&asked_b);
    head.store(HEAD + 10, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(child_with(TEST, "second", &[&b], &file, &[(TOP, &(HEAD + 10).to_string())]), vec!["1"], "the capped node answers, the kept log among what it gives");
    let after = HEAD - settled::MARGIN + 1;
    let asked = take(&asked_b);
    assert!(asked.first().map(|r| r.0) == Some(after) && asked.last().map(|r| r.1) == Some(HEAD + 10) && asked.windows(2).all(|w| w[0].1 + 1 == w[1].0), "asked only the blocks after the kept part, end to end: {asked:?}");
    assert_eq!(whole(&file).entries.len(), 2, "each node's record its own");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn what_one_node_read_in_this_action_is_not_lent_to_the_next() {
    const TEST: &str = "what_one_node_read_in_this_action_is_not_lent_to_the_next";
    if std::env::var(ROLE).is_ok() {
        let (_action, nodes) = child_setup();
        let filter = zikaron_anchor::scan::LogFilter { addresses: vec![REGISTRY], topics: Vec::new() };
        let top = std::env::var(TOP).ok().and_then(|t| t.parse().ok()).unwrap_or(HEAD);
        for n in &nodes {
            let mut ep = zikaron_send::node::endpoint_at(n).expect("a way to the node");
            let got = zikaron_anchor::scan::logs_over_window(ep.as_mut(), CHAIN, &filter, 0, top);
            println!("SAID {}", got.map(|l| l.len().to_string()).unwrap_or_else(|r| r.code().to_string()));
        }
        return;
    }
    let dir = scratch("lent-here");
    let file = dir.join("logs.json");
    let head = Arc::new(std::sync::atomic::AtomicU64::new(HEAD));
    let ((a, asked_a, _), (b, asked_b, _)) = (node_at(head.clone(), None, true), node_at(head.clone(), None, true));
    assert_eq!(child(TEST, "first", &[&a], &file), vec!["1"], "the first node reads the whole range");
    take(&asked_a);
    let moved = HEAD + 200;
    head.store(moved, std::sync::atomic::Ordering::SeqCst);
    assert_eq!(child_with(TEST, "second", &[&a, &b], &file, &[(TOP, &moved.to_string())]), vec!["1", "1"], "both nodes give the log");
    let written = HEAD - settled::MARGIN + 1;
    assert_eq!(take(&asked_a), vec![(written, moved)], "the first node reads on from its own record");
    assert_eq!(take(&asked_b), vec![(written, moved)], "the second node is asked from what is written, not from what the first read now");
    let _ = std::fs::remove_dir_all(&dir);
}
