//! App gaps, each through the narrowest public path: the check's start block when a registry is typed, a
//! keystore's `p` of the wrong kind, the grant-file entry's id shape, a small-file write whose directory sync
//! fails, and a badge past the QR code's capacity. The machine directory is a temporary one of this process;
//! nodes are fakes on 127.0.0.1 in this process.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use super::vault_open;


fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("zk-gaps-b10m-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// A JSON-RPC node in this process on 127.0.0.1: answers each method by `answer` (`None`: closes without an
/// answer) and keeps every request body it was sent in `log`.
fn node(answer: fn(&str) -> Option<String>, log: Arc<Mutex<Vec<String>>>) -> String {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
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
                log.lock().expect("log").push(body.clone());
                let method = body.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                let id = body.split("\"id\":").nth(1).and_then(|r| r.split(|c| c == ',' || c == '}').next()).unwrap_or("1").trim().to_string();
                let Some(result) = answer(&method) else { return };
                let reply = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{result}}}");
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
            });
        }
    });
    url
}

/// A chain 31337 at height 100 with no logs.
fn quiet_chain(m: &str) -> Option<String> {
    Some(match m {
        "eth_chainId" => "\"0x7a69\"".into(),
        "eth_blockNumber" => "\"0x64\"".into(),
        "eth_getLogs" => "[]".into(),
        _ => "null".into(),
    })
}

/// The `fromBlock` of every `eth_getLogs` the node was asked.
fn from_blocks(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
    log.lock()
        .expect("log")
        .iter()
        .filter(|b| b.contains("\"eth_getLogs\""))
        .filter_map(|b| b.split("\"fromBlock\"").nth(1).and_then(|r| r.split('"').nth(1)).map(str::to_string))
        .collect()
}

fn settled(shell: &mut app::shell::Shell, k: app::task::Kind) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while shell.tasks.in_flight(k) && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    shell.drain();
}

/// `n` grants by one key, each the upstream of the next, from a genesis; returns the genesis bytes and the
/// grants' (id, bytes) from the root.
fn grants(seed: u8, n: usize) -> (Vec<u8>, Vec<(String, Vec<u8>)>) {
    let secret = app::key::Secret::take([seed; 32]).expect("a key");
    let genesis = app::entryx::genesis(&secret, "b10m").expect("genesis");
    let mut out: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..n {
        let d = app::grantx::Draft {
            grantee: format!("0x{}", "33".repeat(20)),
            work: format!("0x{:064x}", i + 1),
            terms: format!("0x{}", "44".repeat(32)),
            upstream: out.last().map(|(id, _)| id.clone()).unwrap_or_default(),
            ..Default::default()
        };
        let g = app::entryx::seal(&secret, "grant", (i + 1) as u64, Some(&genesis.id), app::grantx::grant_body(&d).expect("a body")).expect("a grant");
        out.push((g.id, g.bytes));
    }
    (genesis.bytes, out)
}

/// With a registry typed on the check page and the start block left empty, the check scans from block 0, not
/// from the home's start block; with nothing typed it uses the home's registry and start block. Checked both in
/// the basis the check returns and in the `fromBlock` the node was asked.
#[test]
fn b3_a_typed_registry_with_the_start_block_empty_checks_from_block_zero() {
    if super::alone_in(module_path!(), "b3_a_typed_registry_with_the_start_block_empty_checks_from_block_zero") {
        return;
    }
    use app::action::{apply, Action, Applied};
    vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    // The home's own basis: another registry, a start block of 7.
    shell.settings.registry = Some(app::key::Address([0x22; 20]));
    shell.settings.from_block = 7;
    let (_, gs) = grants(0x51, 1);
    let payload = zikaron_kit::badge::encode(&[gs[0].1.clone()]).expect("a code");
    let typed_reg = format!("0x{}", "11".repeat(20));
    for (registry, want, form) in [(typed_reg.as_str(), 0u64, "a registry typed, the start block empty"), ("", 7u64, "nothing typed: the home's registry and start block")] {
        let log = Arc::new(Mutex::new(Vec::new()));
        let url = node(quiet_chain, log.clone());
        let a = Action::CheckPayload {
            typed: payload.clone(),
            ledgers: String::new(),
            endpoints: format!("31337={url}"),
            registry: registry.into(),
            from_block: String::new(),
            now: "150".into(),
            file: String::new(),
            terms: String::new(),
        };
        answers!(apply(&mut shell, a), Applied::Started(app::task::Kind::Check), "{form}");
        settled(&mut shell, app::task::Kind::Check);
        let checked = shell.checked.clone().unwrap_or_else(|| panic!("{form}: the check landed"));
        let basis = checked.basis.clone().unwrap_or_else(|e| panic!("{form}: the basis was read: {e}"));
        assert_eq!(basis.from_block, want, "{form}: the basis's start block");
        let asked = from_blocks(&log);
        assert!(!asked.is_empty(), "{form}: the node was asked for logs");
        assert!(asked.iter().all(|f| *f == format!("0x{want:x}")), "{form}: every eth_getLogs from block {want}: {asked:?}");
    }
}

fn with_member(v: &zikaron::json::Value, path: &[&str], to: Option<zikaron::json::Value>) -> zikaron::json::Value {
    use zikaron::json::Value;
    let Value::Obj(m) = v else { return v.clone() };
    let mut m = m.clone();
    match path {
        [k] => {
            m.retain(|(x, _)| x != k);
            if let Some(t) = to {
                m.push((k.to_string(), t));
            }
        }
        [k, rest @ ..] => {
            for (x, inner) in m.iter_mut() {
                if x == k {
                    *inner = with_member(inner, rest, to.clone());
                }
            }
        }
        [] => {}
    }
    Value::Obj(m)
}

/// A keystore whose `crypto.kdfparams.p` is text or a fraction is refused as a shape error before any work (as
/// `r` as text is); the file as written opens.
#[test]
fn a_keystore_whose_p_is_text_or_a_fraction_is_said_as_its_shape() {
    if super::alone_in(module_path!(), "a_keystore_whose_p_is_text_or_a_fraction_is_said_as_its_shape") {
        return;
    }
    use zikaron::json::Value;
    let secret = app::key::Secret::take([0x42; 32]).expect("a key");
    let ks = app::keystore::encrypt(&secret, "pw-probe", app::keystore::Params::light(), 1_700_000_000).expect("encrypted");
    let v = zikaron::json::parse(&ks.json).expect("json");
    let open = |v: &Value| app::keystore::decrypt(&zikaron::json::canon_bytes(v), "pw-probe").err().map(|f| f.said().split(':').next().unwrap_or("").to_string());
    assert_eq!(open(&v), None, "the file as written opens");
    assert_eq!(open(&with_member(&v, &["crypto", "kdfparams", "p"], Some(Value::Str("1".into())))).as_deref(), Some("KEYSTORE_SHAPE"), "p as text");
    // The value domain has no fractions: the file is written as text with `p` a fraction.
    let text = String::from_utf8(zikaron::json::canon_bytes(&v)).expect("text");
    let at = text.find("\"p\":").expect("p is written") + 4;
    let end = at + text[at..].find(|c: char| !c.is_ascii_digit()).expect("p ends");
    let fraction = format!("{}1.5{}", &text[..at], &text[end..]);
    assert!(fraction.contains("\"p\":1.5"), "{fraction}");
    let said = app::keystore::decrypt(fraction.as_bytes(), "pw-probe").err().map(|f| f.said().split(':').next().unwrap_or("").to_string());
    assert_eq!(said.as_deref(), Some("KEYSTORE_SHAPE"), "p a fraction");
}

/// The grant-file entry checks the grant id by the same shape rule as the copied code and the badge
/// (`badgex::chain_for`): a malformed id is a content-shape refusal naming the id, given before any home is
/// asked for, the same refusal `chain_for` and `code_for` give.
#[test]
fn the_grant_file_entry_refuses_an_id_of_the_wrong_shape_as_the_code_does() {
    if super::alone_in(module_path!(), "the_grant_file_entry_refuses_an_id_of_the_wrong_shape_as_the_code_does") {
        return;
    }
    use app::action::{apply, Action, Applied};
    vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let (genesis, gs) = grants(0x45, 1);
    let pool = vec![genesis, gs[0].1.clone()];
    let wrong = format!("0x{}", &gs[0].0.trim_start_matches("0x")[..63]);
    let by_code = app::badgex::chain_for(&pool, &wrong).expect_err("the code refuses it");
    assert!(by_code.said().starts_with("CONTENT_SHAPE"), "{}", by_code.said());
    match apply(&mut shell, Action::ExportGrantFile { id: wrong.clone(), to: String::new() }) {
        Applied::Trouble(f) => {
            assert_eq!(f.which(), by_code.which(), "the same member: {}", f.said());
            assert_eq!(f.said(), by_code.said(), "the same words");
            assert_eq!(f.tail(), wrong, "said with the id as given");
        }
        other => panic!("the grant-file entry took an id of the wrong shape: {other:?}"),
    }
}

/// When the directory sync after the rename fails, the write is reported as not done, yet the rename has
/// happened: `put_at` leaves the file with the new bytes, and `rename_over` leaves the target with the staged
/// bytes (the staged name gone).
#[test]
fn a_write_whose_directory_sync_fails_has_still_renamed_the_file_into_place() {
    if super::alone_in(module_path!(), "a_write_whose_directory_sync_fails_has_still_renamed_the_file_into_place") {
        return;
    }
    let dir = scratch("put-sync");
    app::home::put_at(&dir, "a.json", b"1").expect("lands");
    std::fs::write(dir.join(".staged"), b"3").expect("a staged file");
    zikaron_os::pretend_sync_fails_at(Some(dir.clone()));
    let put = app::home::put_at(&dir, "a.json", b"2");
    let over = app::home::rename_over(&dir.join(".staged"), &dir.join("b.json"));
    zikaron_os::pretend_sync_fails_at(None);
    assert!(put.is_err() && over.is_err(), "both said not done");
    assert_eq!(std::fs::read(dir.join("a.json")).expect("a.json is in place"), b"2", "put_at: the new bytes are in place");
    assert_eq!(std::fs::read(dir.join("b.json")).expect("b.json is in place"), b"3", "rename_over: the target exists with the staged bytes");
    assert!(!dir.join(".staged").exists(), "rename_over: the staged name is gone");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A badge whose chain exceeds the code's capacity is refused by name and nothing is written; one within it is
/// made. The refusal comes from the kit crate's cap (`E_BADGE_CAP`), which equals the QR code's capacity at
/// version 40, so the drawing's own "too long" (`Key::Tail088`) is reachable only through `qr::make` itself.
#[test]
fn a_badge_past_the_codes_capacity_is_refused_and_nothing_is_written() {
    if super::alone_in(module_path!(), "a_badge_past_the_codes_capacity_is_refused_and_nothing_is_written") {
        return;
    }
    use app::action::{apply, Action, Applied};
    vault_open();
    assert_eq!(zikaron_kit::tokens::BADGE_CAP, app::qr::capacity(40), "the kit's cap is the code's capacity");
    assert_eq!(app::qr::make(&vec![b'q'; app::qr::capacity(40) + 1]).err(), Some(app::qr::NotMade::TooLong), "the drawing refuses past it");
    let dir = scratch("badge-cap");
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let home = app::home::Home::open_or_create(dir.join("home")).expect("a home");
    shell.lock = Some(app::lock::take(&home).expect("the lock"));
    let log = Arc::new(Mutex::new(Vec::new()));
    let url = node(quiet_chain, log);
    shell.settings.chain_id = Some(31337);
    shell.settings.registry = Some(app::key::Address([0x11; 20]));
    shell.endpoints = vec![app::chainx::Endpoint::parse(&format!("31337={url}")).expect("an endpoint")];
    let (_, long) = grants(0x61, 8);
    for (_, b) in &long {
        app::vaultx::store(&home, b).expect("held");
    }
    let (_, short) = grants(0x62, 1);
    app::vaultx::store(&home, &short[0].1).expect("held");
    shell.home = Some(home);
    let chain: Vec<Vec<u8>> = long.iter().map(|(_, b)| b.clone()).collect();
    assert!(zikaron_kit::badge::encode(&chain).is_err(), "eight hops are past the cap");
    for (grant, past) in [(short[0].0.clone(), false), (long.last().expect("a grant").0.clone(), true)] {
        let out = dir.join(if past { "out-past" } else { "out-within" });
        let before = shell.faults.len();
        let a = apply(&mut shell, Action::ExportBadge { grant: grant.clone(), out: out.display().to_string() });
        assert!(matches!(a, Applied::Started(app::task::Kind::Badge)), "past={past}: {a:?}");
        settled(&mut shell, app::task::Kind::Badge);
        if past {
            let f = shell.faults[before..].last().cloned().expect("refused by name");
            assert!(f.said().starts_with("PAYLOAD_REFUSED") && f.tail().contains("E_BADGE_CAP"), "{} · {}", f.said(), f.tail());
            assert!(shell.badge.is_none(), "no badge made");
            assert!(!out.join(app::badgex::TXT).exists() && !out.join(app::badgex::SVG).exists(), "nothing written");
        } else {
            let f: Vec<String> = shell.faults[before..].iter().map(|f| f.said().to_string()).collect();
            let made = shell.badge.clone().unwrap_or_else(|| panic!("within the cap the badge is made: {f:?}"));
            assert_eq!(made.grant, grant);
            assert!(made.txt.is_file() && made.svg.is_file(), "both written");
        }
    }
    drop(shell);
    let _ = std::fs::remove_dir_all(&dir);
}
