use super::vault_open;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use zikaron_anchor::scan::{Fact, FactKey};
use zikaron_send::checked::{self, Record, Sealed};

const CHAIN: u64 = 31337;
const HEAD: u64 = 0x400;
const REGISTRY: [u8; 20] = [0x11; 20];
const SENDER: [u8; 20] = [0x22; 20];
const HASH: [u8; 32] = [0x33; 32];
const TX: [u8; 32] = [0x44; 32];
const BLOCK: [u8; 32] = [0x55; 32];

fn anchor_log() -> String {
    let word = |b: &[u8]| format!("\"0x{}{}\"", "00".repeat(32 - b.len()), zikaron::hexfmt::encode(b).trim_start_matches("0x"));
    format!(
        "{{\"address\":\"{}\",\"blockHash\":\"{}\",\"blockNumber\":\"0x10\",\"data\":\"0x\",\"logIndex\":\"0x0\",\"topics\":[\"{}\",{},{}],\"transactionHash\":\"{}\"}}",
        zikaron::hexfmt::encode(&REGISTRY),
        zikaron::hexfmt::encode(&BLOCK),
        zikaron::hexfmt::encode(&zikaron_anchor::scan::topic0()),
        word(&SENDER),
        word(&HASH),
        zikaron::hexfmt::encode(&TX)
    )
}

fn the_fact() -> (FactKey, Fact) {
    ((CHAIN, BLOCK, TX, 0), Fact { block_number: 0x10, sender: SENDER, hash: HASH, emitter: REGISTRY, block_timestamp: 1_700_000_000, verdict: zikaron::tokens::Verdict::Counted })
}

type Asked = Arc<Mutex<Vec<String>>>;

fn node() -> (String, Asked) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
    let asked: Asked = Default::default();
    let log = asked.clone();
    std::thread::spawn(move || {
        for s in l.incoming().flatten() {
            let log = log.clone();
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
                let from = body.split("\"fromBlock\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                let member = match method.as_str() {
                    "eth_chainId" => format!("\"result\":\"0x{CHAIN:x}\""),
                    "eth_getBlockByNumber" if body.contains("\"0x10\"") => format!("\"result\":{{\"hash\":\"{}\",\"number\":\"0x10\"}}", zikaron::hexfmt::encode(&BLOCK)),
                    "eth_getLogs" => {
                        let start = u64::from_str_radix(from.trim_start_matches("0x"), 16).unwrap_or(0);
                        if start <= 0x10 { format!("\"result\":[{}]", anchor_log()) } else { "\"result\":[]".to_string() }
                    }
                    _ => "\"result\":null".to_string(),
                };
                log.lock().unwrap().push(if method == "eth_getLogs" { format!("eth_getLogs {from}") } else { method });
                let body = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",{member}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            });
        }
    });
    (url, asked)
}

fn logs_kept(machine: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let dir = machine.join(checked::DIR);
    let mut out: Vec<(String, Vec<u8>)> = checked::files_in(&dir).into_iter().filter(|f| checked::is_logs(f)).map(|f| (f.clone(), std::fs::read(dir.join(&f)).expect("a part"))).collect();
    out.sort();
    out
}

fn basis() -> Vec<u8> {
    let senders = vec![zikaron::hexfmt::encode(&SENDER)];
    let registry = zikaron::hexfmt::encode(&REGISTRY);
    zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&zikaron_glue::basis::Window { chain: CHAIN, registries: std::slice::from_ref(&registry), from_block: 0, to_block: HEAD, senders: &senders }))
}

fn anchors(s: zikaron_send::agree::Scan) -> usize {
    match s {
        zikaron_send::agree::Scan::Basis(a) => a.fragment.member("anchors").and_then(|x| x.as_arr()).map_or(0, |x| x.len()),
        zikaron_send::agree::Scan::NoLabel { .. } => panic!("a basis"),
    }
}

#[test]
fn the_command_lines_road_reads_and_keeps_the_facts_the_desktop_checked() {
    if super::alone_in(module_path!(), "the_command_lines_road_reads_and_keeps_the_facts_the_desktop_checked") {
        return;
    }
    vault_open();
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let machine = app::home::machine_dir().expect("the machine directory");
    let ((a, asked_a), (b, asked_b)) = (node(), node());
    let eps = vec![app::chainx::Endpoint::at(CHAIN, a.as_str()), app::chainx::Endpoint::at(CHAIN, b.as_str())];
    let tx_asked = |x: &Asked| x.lock().unwrap().iter().filter(|m| *m == "eth_getTransactionByHash").count();
    assert_eq!(anchors(zikaron_send::agree::scan(&eps, &basis(), &zikaron_send::Bare).expect("a scan")), 0);
    assert_eq!((tx_asked(&asked_a), tx_asked(&asked_b)), (1, 1));
    app::checkedx::add(&[the_fact()]).expect("the desktop keeps it");
    let cli = Record(Sealed(machine.clone()));
    assert_eq!(zikaron_send::Facts::known(&cli).0.get(&the_fact().0), Some(&the_fact().1), "the same facts.json, opened");
    assert_eq!(anchors(zikaron_send::agree::scan(&eps, &basis(), &cli).expect("a scan")), 1, "the anchor from the record");
    assert_eq!((tx_asked(&asked_a), tx_asked(&asked_b)), (1, 1), "nothing more asked about it");
    let mut other = the_fact();
    other.0 .3 = 1;
    assert_eq!(checked::add(&Sealed(machine), &[other]).expect("kept"), 1);
    assert_eq!(app::checkedx::read().0.get(&other.0), Some(&other.1));
}

#[test]
fn the_command_lines_road_reads_the_log_reads_the_desktop_settled() {
    if super::alone_in(module_path!(), "the_command_lines_road_reads_the_log_reads_the_desktop_settled") {
        return;
    }
    vault_open();
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let machine = app::home::machine_dir().expect("the machine directory");
    let ((a, asked_a), (b, _)) = (node(), node());
    let eps = vec![app::chainx::Endpoint::at(CHAIN, a.as_str()), app::chainx::Endpoint::at(CHAIN, b.as_str())];
    let logs_asked = |x: &Asked| x.lock().unwrap().iter().filter(|m| m.starts_with("eth_getLogs")).cloned().collect::<Vec<_>>();
    {
        let _desk = zikaron_anchor::action::open(Some(app::checkedx::settled_keeper()));
        let _ = zikaron_send::agree::scan(&eps, &basis(), &zikaron_send::Bare).expect("a scan");
    }
    assert_eq!(logs_asked(&asked_a), vec!["eth_getLogs 0x0"]);
    assert!(!logs_kept(&machine).is_empty(), "the desktop sealed what it settled");
    let _cli = zikaron_anchor::action::open(Some(Arc::new(checked::Logs(Sealed(machine)))));
    let _ = zikaron_send::agree::scan(&eps, &basis(), &zikaron_send::Bare).expect("a scan");
    assert_eq!(logs_asked(&asked_a), vec!["eth_getLogs 0x0".to_string(), format!("eth_getLogs 0x{:x}", HEAD - zikaron_anchor::settled::MARGIN + 1)], "the command line's road asks only after it");
}

#[test]
fn the_grant_check_pages_task_reads_and_writes_no_settled_record() {
    if super::alone_in(module_path!(), "the_grant_check_pages_task_reads_and_writes_no_settled_record") {
        return;
    }
    vault_open();
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let machine = app::home::machine_dir().expect("the machine directory");
    let ((a, asked_a), (b, _)) = (node(), node());
    let eps = vec![app::chainx::Endpoint::at(CHAIN, a.as_str()), app::chainx::Endpoint::at(CHAIN, b.as_str())];
    let logs_asked = |x: &Asked| x.lock().unwrap().iter().filter(|m| m.starts_with("eth_getLogs")).cloned().collect::<Vec<_>>();
    let mut tasks = app::task::Tasks::new();
    let run = |tasks: &mut app::task::Tasks, kind: app::task::Kind| {
        let eps = eps.clone();
        let _ = tasks.spawn(kind, move || {
            let _ = app::auditx::scan_agreed_bytes_with(&eps, &basis(), app::auditx::Facts::Bare);
            Err(app::fault::Fault::known(app::fault::Known::Unreachable, String::new()))
        });
        for _ in 0..5000 {
            if tasks.drain_at(1.0).iter().any(|o| o.kind == kind) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        panic!("the task never landed");
    };
    assert!(app::task::Kind::Audit.keeps_reads() && !app::task::Kind::Check.keeps_reads());
    run(&mut tasks, app::task::Kind::Audit);
    assert_eq!(logs_asked(&asked_a), vec!["eth_getLogs 0x0"]);
    let kept = logs_kept(&machine);
    assert!(!kept.is_empty(), "the self-audit's task kept what it settled");
    run(&mut tasks, app::task::Kind::Check);
    assert_eq!(logs_asked(&asked_a), vec!["eth_getLogs 0x0", "eth_getLogs 0x0"], "the page's scan read nothing kept: the whole range");
    assert_eq!(logs_kept(&machine), kept, "the page's scan wrote nothing");
    run(&mut tasks, app::task::Kind::Audit);
    assert_eq!(logs_asked(&asked_a).last().cloned(), Some(format!("eth_getLogs 0x{:x}", HEAD - zikaron_anchor::settled::MARGIN + 1)), "the next desktop task reads after what was kept");
}

#[test]
fn the_verify_pages_reading_takes_one_nodes_facts_and_an_agreed_reading_does_not() {
    if super::alone_in(module_path!(), "the_verify_pages_reading_takes_one_nodes_facts_and_an_agreed_reading_does_not") {
        return;
    }
    vault_open();
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let ((a, asked_a), (b, asked_b)) = (node(), node());
    let tx_asked = |x: &Asked| x.lock().unwrap().iter().filter(|m| *m == "eth_getTransactionByHash").count();
    app::checkedx::add_single(&[the_fact()]).expect("one node's fact kept");
    assert!(app::checkedx::read().0.get(&the_fact().0).is_none(), "not among the agreed facts");
    let (scanned, _) = app::auditx::scan_first(&[app::chainx::Endpoint::at(CHAIN, a.as_str())], &basis()).expect("the verify's reading");
    assert_eq!((scanned.anchors, tx_asked(&asked_a)), (1, 0), "the anchor from one node's record, nothing asked about it");
    let eps = vec![app::chainx::Endpoint::at(CHAIN, a.as_str()), app::chainx::Endpoint::at(CHAIN, b.as_str())];
    assert_eq!(anchors(zikaron_send::agree::scan(&eps, &basis(), &app::sendx::Local).expect("a scan")), 0);
    assert_eq!((tx_asked(&asked_a), tx_asked(&asked_b)), (1, 1), "an agreed reading asks about it again; neither node holds the transaction, so each is asked");
}

#[test]
fn the_grant_check_pages_task_keeps_the_notice_input_alone() {
    if super::alone_in(module_path!(), "the_grant_check_pages_task_keeps_the_notice_input_alone") {
        return;
    }
    vault_open();
    let mut tasks = app::task::Tasks::new();
    let seen: Arc<Mutex<Vec<(app::task::Kind, bool, bool)>>> = Arc::new(Mutex::new(Vec::new()));
    for kind in [app::task::Kind::Check, app::task::Kind::Audit] {
        let seen = seen.clone();
        let _ = tasks.spawn(kind, move || {
            seen.lock().unwrap().push((kind, zikaron_anchor::action::keeper().is_some(), zikaron_anchor::action::judged_keeper().is_some()));
            Err(app::fault::Fault::known(app::fault::Known::Unreachable, String::new()))
        });
        for _ in 0..5000 {
            if tasks.drain_at(1.0).iter().any(|o| o.kind == kind) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }
    assert!(app::task::Kind::Check.keeps_notices() && !app::task::Kind::Check.keeps_reads());
    assert_eq!(*seen.lock().unwrap(), vec![(app::task::Kind::Check, false, true), (app::task::Kind::Audit, true, true)], "the page keeps the notice input alone; the self-audit keeps both");
}

#[test]
fn an_audit_is_kept_in_the_checked_room_and_read_back() {
    if super::alone_in(module_path!(), "an_audit_is_kept_in_the_checked_room_and_read_back") {
        return;
    }
    vault_open();
    let machine = app::home::machine_dir().expect("the machine directory");
    let key = zikaron_send::behind::Key::of(b"root", b"basis", b"pile");
    let value = zikaron::json::Value::Obj(vec![("verdict".into(), zikaron::json::Value::Str("GREEN".into()))]);
    let result = app::checkx::Audit { verdict: "GREEN".into(), label: "COMPLETE".into(), entries: 312, chain: CHAIN, block: 7_214_903, at: Some(1_700_000_000), value, anchored: vec![Some(1_699_999_000), None], ledgers: vec![Some((312, false)), None] };
    let first = app::checkx::Audits::new(Some(Arc::new(app::checkx::AuditStore)));
    let r = result.clone();
    first.ask(key, zikaron_send::behind::Job { keeper: None, work: Box::new(move || Ok(r)) });
    first.run();
    let file = machine.join(checked::DIR).join(checked::audit_file(&key));
    assert!(file.is_file(), "kept as {}", file.display());
    assert!(!std::fs::read(&file).unwrap().windows(8).any(|w| w == b"COMPLETE"), "sealed: no word of it in the clear");
    let again = app::checkx::Audits::new(Some(Arc::new(app::checkx::AuditStore)));
    assert!(again.recall(key));
    again.run();
    assert_eq!(again.state(&key), zikaron_send::behind::State::Done(result.clone()), "read back behind");
    app::keybox::lock();
    let other = zikaron_send::behind::Key::of(b"root", b"basis", b"another pile");
    let locked = app::checkx::Audits::new(Some(Arc::new(app::checkx::AuditStore)));
    let r = result.clone();
    locked.ask(other, zikaron_send::behind::Job { keeper: None, work: Box::new(move || Ok(r)) });
    locked.run();
    assert_eq!(locked.state(&other), zikaron_send::behind::State::Done(result));
    assert!(!machine.join(checked::DIR).join(checked::audit_file(&other)).exists(), "nothing kept while locked");
}

#[test]
fn a_part_the_desktop_cannot_write_for_the_store_locked_is_told_apart() {
    if super::alone_in(module_path!(), "a_part_the_desktop_cannot_write_for_the_store_locked_is_told_apart") {
        return;
    }
    vault_open();
    let keep = app::checkedx::settled_keeper();
    assert_eq!(keep.save("notes", b"{}"), Ok(()), "written while open");
    app::keybox::lock();
    assert_eq!(keep.save("notes", b"{}"), Err(zikaron_anchor::settled::Unkept::Locked), "the lock's write: told apart, never said");
}

#[test]
fn a_part_read_under_a_key_since_replaced_is_dropped_and_said_by_nobody() {
    if super::alone_in(module_path!(), "a_part_read_under_a_key_since_replaced_is_dropped_and_said_by_nobody") {
        return;
    }
    vault_open();
    let keep = app::checkedx::settled_keeper();
    assert_eq!(keep.save("notes", b"{}"), Ok(()), "written under the key it was read under");
    assert!(keep.load("notes").is_some());
    let mut slots: Vec<(String, Vec<u8>)> = Vec::new();
    for a in app::keybox::accounts().expect("the accounts") {
        if let Some(b) = app::keybox::get(&a).expect("a slot") {
            slots.push((a, b));
        }
    }
    let nb = app::keybox::build_new(app::keybox::PinFor::New("48261937"), None, &slots).expect("a new book");
    app::keybox::stage_new(&nb).expect("staged");
    app::keybox::commit_new(nb).expect("the key replaced");
    let _ = zikaron_anchor::settled::unkept();
    assert_eq!(keep.save("notes", b"{}"), Err(zikaron_anchor::settled::Unkept::KeyChanged), "read under the key replaced: dropped");
    assert_eq!(keep.load("notes"), None, "nor read back under the new key");
    assert!(zikaron_anchor::settled::unkept().is_empty(), "said by nobody");
}
