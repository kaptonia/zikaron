//! Boundary cases, one test each. Each test sets a temporary machine directory and its own test account before
//! any vault or shell use; the real machine directory and account are never touched. Nodes are local ports in
//! this process; no network.

use app::fault::Known;
use app::local::{self, Doc, Expect, Ident, Owner, Unread};
use std::path::{Path, PathBuf};
use super::vault_open;




/// A fresh temporary folder for one test.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("zk-gaps-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}


fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn code_only(s: &str) -> String {
    s.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n")
}

fn reason(r: Result<Vec<u8>, app::fault::Fault>) -> Option<Unread> {
    match r {
        Ok(_) => None,
        Err(f) => {
            assert_eq!(f.which(), Some(Known::LocalSeal), "{}", f.said());
            Unread::of(&f)
        }
    }
}

// ───────────────────────── Nodes on local ports ─────────────────────────

/// A JSON-RPC node on a local port answering each question by `answer(method)`, one question per connection.
fn rpc_node(answer: fn(&str) -> Option<String>) -> String {
    serve(move |body| {
        let method = body.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
        let id = body.split("\"id\":").nth(1).and_then(|r| r.split(|c| c == ',' || c == '}').next()).unwrap_or("1").trim().to_string();
        let result = answer(&method)?;
        let body = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",\"result\":{result}}}");
        Some(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()))
    })
}

/// A node on a local port answering every question with this HTTP status and a page that is not the node's
/// word (a gateway's page, a bare refusal).
fn status_node(line: &'static str) -> String {
    serve(move |_| {
        let body = "<html>busy</html>";
        Some(format!("HTTP/1.1 {line}\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()))
    })
}

fn serve(reply: impl Fn(&str) -> Option<String> + Send + Sync + Copy + 'static) -> String {
    use std::io::{Read, Write};
    let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let url = format!("http://{}", l.local_addr().expect("addr"));
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
                let Some(out) = reply(&body) else { return };
                let _ = s.write_all(out.as_bytes());
            });
        }
    });
    url
}

fn eps(urls: &[&String]) -> Vec<app::chainx::Endpoint> {
    urls.iter().map(|u| app::chainx::Endpoint::parse(&format!("31337={u}")).expect("an endpoint")).collect()
}

fn quick() {
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
}

fn node_at_256(m: &str) -> Option<String> {
    Some(match m {
        "eth_chainId" => "\"0x7a69\"".into(),
        "eth_blockNumber" => "\"0x100\"".into(),
        _ => "null".into(),
    })
}
fn node_at_192(m: &str) -> Option<String> {
    Some(match m {
        "eth_blockNumber" => "\"0xc0\"".into(),
        other => return node_at_256(other),
    })
}
fn node_at_191(m: &str) -> Option<String> {
    Some(match m {
        "eth_blockNumber" => "\"0xbf\"".into(),
        other => return node_at_256(other),
    })
}

/// A shell over a fresh home of its own, its writer lock held (as set up for the basis check in `checks.rs`).
fn shell_with_home(name: &str) -> (app::shell::Shell, PathBuf) {
    vault_open();
    let dir = scratch(name);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let home = app::home::Home::open_or_create(&dir).expect("a home");
    shell.lock = Some(app::lock::take(&home).expect("the lock"));
    shell.home = Some(home);
    (shell, dir)
}

// ───────────────────────── a sealed file's head and inner frame ─────────────────────────

/// A sealed file whose head matches what is asked (kind, version, identity digest) but whose inner frame names
/// another owner, or the same owner at another place, reads as `swapped`; an unreadable inner frame is
/// `unopenable`; an unknown kind tag of the same length is `other-kind`.
#[test]
fn an_inner_frame_of_another_owner_is_swapped_garbage_framing_is_unopenable_an_unknown_tag_is_another_kind() {
    if super::alone_in(module_path!(), "an_inner_frame_of_another_owner_is_swapped_garbage_framing_is_unopenable_an_unknown_tag_is_another_kind") {
        return;
    }
    let key: [u8; 32] = std::array::from_fn(|i| i as u8);
    let nonce: [u8; 24] = std::array::from_fn(|i| 0x40 + i as u8);
    let mine = Owner::Home("000102030405060708090a0b0c0d0e0f".into());
    let logical = "settings/desk.json";
    let asked = Ident { owner: mine.clone(), doc: Doc::Settings, logical: logical.into() };
    let ex = Expect { owner: Some(mine.clone()), doc: Doc::Settings, rel: Some(logical.into()) };
    let plain = b"{\"capBytes\":0}";
    // The head as the asked file's: magic, algorithm (the version written), kind, version, the asked identity's
    // keyed digest, the nonce.
    let tag = Doc::Settings.tag().as_bytes();
    let mut head = local::MAGIC_V2.to_vec();
    head.push(local::ALG_KEYED);
    head.push(tag.len() as u8);
    head.extend_from_slice(tag);
    head.extend_from_slice(&Doc::Settings.version().to_be_bytes());
    head.extend_from_slice(&asked.digest_keyed(&key));
    head.extend_from_slice(&nonce);
    let sealed_with = |body: &[u8]| {
        let ct = app::cryptx::xchacha_seal(&key, &nonce, &head, body).expect("seals");
        let mut out = head.clone();
        out.extend_from_slice(&ct);
        out
    };
    let frame = |owner: &str, logical: &str| {
        let mut b = Vec::new();
        for part in [owner.as_bytes(), logical.as_bytes()] {
            b.extend_from_slice(&(part.len() as u16).to_be_bytes());
            b.extend_from_slice(part);
        }
        b.extend_from_slice(plain);
        b
    };
    // The frame as the envelope writes it opens: the hand-made head is the real one.
    let right = sealed_with(&frame(&mine.text(), logical));
    assert_eq!(right, local::envelope(&key, &asked, &nonce, plain).expect("seals"), "the hand-made envelope is the product's");
    assert_eq!(local::unseal(&key, &ex, &right, "t").expect("opens"), plain);
    // Another home inside, another place inside, an owner that is no owner: another file.
    for (form, owner, at) in [
        ("another home inside", "home:ffffffffffffffffffffffffffffffff".to_string(), logical),
        ("the machine inside", "machine".to_string(), logical),
        ("another place inside", mine.text(), "settings/queue.json"),
        ("no owner's spelling inside", "nobody".to_string(), logical),
    ] {
        assert_eq!(reason(local::unseal(&key, &ex, &sealed_with(&frame(&owner, at)), "t")), Some(Unread::Swapped), "{form}");
    }
    // A frame that does not read: a length past the bytes, nothing at all, a name that is not text.
    for (form, body) in [("a length past the bytes", vec![0xff, 0xff, b'x']), ("empty", Vec::new()), ("not text", vec![0x00, 0x02, 0xc3, 0x28, 0x00, 0x00])] {
        assert_eq!(reason(local::unseal(&key, &ex, &sealed_with(&body), "t")), Some(Unread::Unopenable), "{form}");
    }
    // A kind tag this version does not know, of the same length.
    let unknown = b"zzzzzzzz";
    assert_eq!(unknown.len(), tag.len());
    assert_eq!(Doc::from_tag("zzzzzzzz"), None, "a tag no kind has");
    let mut patched = right.clone();
    let at = local::MAGIC_V2.len() + 2;
    assert_eq!(&patched[at..at + tag.len()], tag);
    patched[at..at + tag.len()].copy_from_slice(unknown);
    assert_eq!(reason(local::unseal(&key, &ex, &patched, "t")), Some(Unread::OtherKind));
}

// ───────────────────────── home labels ─────────────────────────

/// A home label changed after it was read (a restore, a key change) is read again, even when the new label has
/// the same length and time (as on a volume with coarse times): a file of the new number opens, one of the old
/// number is `swapped`.
#[test]
fn a_relabelled_home_on_a_coarse_time_volume_reads_by_its_new_number() {
    if super::alone_in(module_path!(), "a_relabelled_home_on_a_coarse_time_volume_reads_by_its_new_number") {
        return;
    }
    vault_open();
    let dir = scratch("relabel");
    let home = app::home::Home::open_or_create(&dir).expect("a home");
    let root = home.root().to_path_buf();
    local::check_label(&root, None, true).expect("a writer labels it");
    let old = local::read_label(&root).expect("reads").expect("a label").number;
    let room = home.dir(app::home::Slot::Settings);
    let at = room.join(app::settings::FILE);
    let rel = format!("{}/{}", app::home::Slot::Settings.as_str(), app::settings::FILE);
    let plain = b"{\"capBytes\":0}";
    local::put(&room, app::settings::FILE, Doc::Settings, plain).expect("a settings file under the old number");
    let old_file = std::fs::read(&at).expect("written");
    assert!(local::read(&at, Doc::Settings).expect("reads").is_some());
    // Another number of the same length, landed over the label; the label file's time put back.
    let label_at = local::label_path(&root);
    let before = std::fs::metadata(&label_at).expect("the label");
    let new = local::Label::new(None).expect("a number").number;
    assert_ne!(new, old);
    let relabelled = local::seal(&local::label_ident(), &local::Label { number: new.clone(), whose: None }.to_bytes()).expect("sealed");
    std::fs::write(&label_at, &relabelled).expect("relabelled");
    let f = std::fs::OpenOptions::new().write(true).open(&label_at).expect("the label");
    f.set_modified(before.modified().expect("a time")).expect("the time put back");
    drop(f);
    let after = std::fs::metadata(&label_at).expect("the label");
    assert_eq!((after.len(), after.modified().ok()), (before.len(), before.modified().ok()), "same length, same time: the coarse volume's view");
    // The label on disk is the new one (opened directly: `read_label` would itself refresh what was read).
    let label_ex = Expect { owner: Some(Owner::Label), doc: Doc::HomeLabel, rel: Some(format!("{}/{}", app::home::Slot::Settings.as_str(), local::LABEL_FILE)) };
    let on_disk = local::open_with(&app::keybox::local_key().expect("the key"), &label_ex, &std::fs::read(&label_at).expect("reads"), "label").expect("opens");
    assert_eq!(local::Label::parse(&on_disk).expect("a label").number, new, "the label on disk is the new one");
    // A file of the new number opens.
    let new_file = local::seal(&local::ident_in(Owner::Home(new.clone()), Doc::Settings, &rel, plain).expect("an ident"), plain).expect("sealed");
    std::fs::write(&at, &new_file).expect("written");
    assert_eq!(local::read(&at, Doc::Settings).map(|o| o.is_some()).map_err(|f| f.said().to_string()), Ok(true), "a file of the new number reads");
    // The old number's file is another home's.
    std::fs::write(&at, &old_file).expect("written");
    assert_eq!(reason(local::read(&at, Doc::Settings).map(|o| o.unwrap_or_default())), Some(Unread::Swapped), "the old number's file is another file");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Concurrent first writers of a fresh home never give it two numbers: eight threads end with one label, one
/// number.
#[test]
fn eight_first_writers_of_a_fresh_home_give_it_one_number() {
    if super::alone_in(module_path!(), "eight_first_writers_of_a_fresh_home_give_it_one_number") {
        return;
    }
    vault_open();
    let dir = scratch("eight");
    std::fs::create_dir_all(&dir).expect("a fresh home");
    let gate = std::sync::Arc::new(std::sync::Barrier::new(8));
    let seen: Vec<String> = (0..8)
        .map(|_| {
            let (gate, root) = (gate.clone(), dir.clone());
            std::thread::spawn(move || {
                gate.wait();
                local::check_label(&root, None, true).expect("a writer opens it");
                local::read_label(&root).expect("reads").expect("a label").number
            })
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().expect("the thread"))
        .collect();
    let on_disk = local::read_label(&dir).expect("reads").expect("a label").number;
    assert!(seen.iter().all(|n| *n == on_disk), "one number: {seen:?} on disk {on_disk}");
    let left: Vec<String> = std::fs::read_dir(local::label_path(&dir).parent().unwrap()).expect("the room").flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert_eq!(left, vec![local::LABEL_FILE.to_string()], "one label file, nothing half-written left");
    let _ = std::fs::remove_dir_all(&dir);
}

// ───────────────────────── the lock file, machine settings ─────────────────────────

/// A lock file already open to others (0644) is used as it is: taking the lock while another holds it leaves
/// its contents and permissions; taking it as the writer leaves its permissions. Unix only: Windows does not
/// open files to other accounts (`zikaron_os::open_to_others` refuses by name there).
#[cfg(unix)]
#[test]
fn an_existing_lock_file_open_to_others_is_left_as_it_is() {
    if super::alone_in(module_path!(), "an_existing_lock_file_open_to_others_is_left_as_it_is") {
        return;
    }
    vault_open();
    let dir = scratch("lockfile");
    let home = app::home::Home::open_or_create(&dir).expect("a home");
    let at = app::lock::path_of(&home);
    std::fs::create_dir_all(at.parent().unwrap()).expect("the room");
    std::fs::write(&at, b"pid 4242\n").expect("an older lock file");
    // Open to others the platform's way (the platform module holds the system's words for it).
    zikaron_os::open_to_others(&at, false).expect("open to others");
    let mode = |p: &Path| zikaron_os::is_owner_only(p).expect("there");
    // Another holder: this one is a reader, the file as it was.
    let other = std::fs::File::open(&at).expect("another handle");
    assert!(app::lock::grab(&other), "the other holds it");
    let reader = app::lock::take(&home).expect("taken");
    assert_eq!(reader.mode(), app::lock::Mode::Reader);
    assert_eq!(std::fs::read(&at).expect("reads"), b"pid 4242\n", "not cut");
    assert!(!mode(&at), "permissions as they were (still open to others)");
    drop(reader);
    drop(other);
    // Nobody holds it: the writer writes its words, the permissions as they were.
    let writer = app::lock::take(&home).expect("taken");
    assert_eq!(writer.mode(), app::lock::Mode::Writer);
    assert!(!mode(&at), "permissions as they were (still open to others)");
    drop(writer);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Machine settings change only through `machine::update` (read, change, write in one step): no shipped source
/// outside `machine.rs` calls or imports `machine::write`.
#[test]
fn no_source_outside_machine_rs_writes_the_machine_settings_whole() {
    if super::alone_in(module_path!(), "no_source_outside_machine_rs_writes_the_machine_settings_whole") {
        return;
    }
    fn walk(at: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(at).expect("src/ reads").flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&src(), &mut files);
    assert!(files.len() > 50, "the sources read: {}", files.len());
    let mut found = Vec::new();
    for f in files.iter().filter(|f| *f != &src().join("machine.rs")) {
        let code = code_only(&std::fs::read_to_string(f).expect("a source file reads"));
        for (i, _) in code.match_indices("machine::write") {
            let next = code[i + "machine::write".len()..].chars().next();
            if !matches!(next, Some(c) if c.is_alphanumeric() || c == '_') {
                found.push(format!("{}: {}", f.strip_prefix(src()).unwrap().display(), code[i..].lines().next().unwrap_or_default()));
            }
        }
    }
    assert!(found.is_empty(), "machine settings written whole outside machine.rs (every change goes through machine::update): {found:?}");
}

// ───────────────────────── asking the nodes ─────────────────────────

/// `MAX_HEAD_LAG` is 64, inclusive: heads 256 and 192 are accepted at the smaller head; 256 and 191 are refused
/// `DISAGREE`, naming the node behind, by how much, and the highest.
#[test]
fn heads_sixty_four_apart_are_taken_and_sixty_five_apart_refused() {
    if super::alone_in(module_path!(), "heads_sixty_four_apart_are_taken_and_sixty_five_apart_refused") {
        return;
    }
    quick();
    assert_eq!(app::chainx::MAX_HEAD_LAG, 64);
    let (high, at_lag, past_lag) = (rpc_node(node_at_256), rpc_node(node_at_192), rpc_node(node_at_191));
    assert_eq!(app::chainx::head_block(&eps(&[&high, &at_lag]), 31337).expect("a head"), (192, 2));
    let f = app::chainx::head_block(&eps(&[&high, &past_lag]), 31337).expect_err("a node behind");
    assert_eq!(f.which(), Some(Known::Disagree), "{}", f.said());
    assert!(f.tail().contains(&past_lag) && f.tail().contains("65") && f.tail().contains("256"), "{}", f.tail());
}

/// Two failing nodes (a bare 429, then a gateway's 502): the height is refused with the first node's code
/// (`RATE_LIMITED`) and both nodes' words in the tail; in the other order, `NODE_REFUSED`.
#[test]
fn two_nodes_with_different_troubles_say_the_first_nodes_code() {
    if super::alone_in(module_path!(), "two_nodes_with_different_troubles_say_the_first_nodes_code") {
        return;
    }
    quick();
    let (limited, gateway) = (status_node("429 Too Many Requests"), status_node("502 Bad Gateway"));
    let f = app::chainx::head_block(&eps(&[&limited, &gateway]), 31337).expect_err("nobody answered");
    assert_eq!(f.which(), Some(Known::RateLimited), "the first node's: {} {}", f.said(), f.tail());
    assert!(f.tail().contains(&limited) && f.tail().contains(&gateway), "both named: {}", f.tail());
    let f = app::chainx::head_block(&eps(&[&gateway, &limited]), 31337).expect_err("nobody answered");
    assert_eq!(f.which(), Some(Known::NodeRefused), "the first node's: {} {}", f.said(), f.tail());
}

// ───────────────────────── the main network's check after the nodes ─────────────────────────

#[test]
fn a_grant_window_past_the_ceiling_is_refused_and_nothing_lands() {
    if super::alone_in(module_path!(), "a_grant_window_past_the_ceiling_is_refused_and_nothing_lands") {
        return;
    }
    use app::action::{apply, Action, Applied};
    let (mut shell, dir) = shell_with_home("window");
    let home = app::home::Home::open(shell.home.as_ref().expect("the home").root()).expect("the home");
    let files = |slot: app::home::Slot| -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(home.dir(slot)).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
        v.sort();
        v
    };
    let (ledger, held, queued) = (files(app::home::Slot::Ledger), files(app::home::Slot::GrantsHeld), shell.queue.items.len());
    let draft = app::grantx::Draft {
        grantee: format!("0x{}", "33".repeat(20)),
        work: format!("0x{}", "55".repeat(32)),
        terms: format!("0x{}", "44".repeat(32)),
        from: "0".into(),
        to: "9007199254740992".into(),
        ..Default::default()
    };
    let past: Vec<String> = app::lang::TABLE.iter().filter(|(k, _, _)| *k == app::lang::Key::TailPastIntCeiling).flat_map(|(_, zh, en)| [zh.replace("{0}", "9007199254740992"), en.replace("{0}", "9007199254740992")]).collect();
    match apply(&mut shell, Action::DraftGrant { draft: Box::new(draft), exclusive: false, terms_file: None }) {
        Applied::Trouble(f) => {
            assert!(f.said().starts_with("SETTINGS_SHAPE"), "{}", f.said());
            assert!(past.iter().any(|w| f.tail() == w), "the past-the-ceiling tail: {}", f.tail());
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(files(app::home::Slot::Ledger), ledger, "the ledger as it was");
    assert_eq!(files(app::home::Slot::GrantsHeld), held, "the held grants as they were");
    assert_eq!(shell.queue.items.len(), queued, "nothing queued");
    drop(shell);
    let _ = std::fs::remove_dir_all(&dir);
}

// ───────────────────────── the count behind the backup ─────────────────────────

/// The count since the last whole-machine backup includes held grants: a grant held before the backup is not
/// behind, one held after it is one behind. Runs in its own process ([`alone`]).
#[test]
fn a_grant_held_after_the_backup_is_behind_it_and_one_held_before_is_not() {
    if super::alone_in(module_path!(), "a_grant_held_after_the_backup_is_behind_it_and_one_held_before_is_not") {
        return;
    }
    vault_open();
    use app::action::{apply, Action, Applied};
    let base = scratch("behind-held");
    let behind = |m: &app::machine::Machine| app::machine::backup_behind(m.backup.as_ref(), app::backup::measured(m.backup.as_ref()).ok());
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    answers!(apply(&mut shell, Action::MakeAnchorKey), Applied::AnchorKey(_));
    answers!(apply(&mut shell, Action::OpenHome { root: base.join("a").display().to_string() }), Applied::Homed { .. });
    let home = app::home::Home::open(shell.home.as_ref().expect("the home").root()).expect("the home");
    // Two grants of another's, signed by their own key.
    let secret = app::key::Secret::take([0x47; 32]).expect("a key");
    let genesis = app::entryx::genesis(&secret, "held").expect("genesis");
    let grant = |n: u64| {
        let d = app::grantx::Draft { grantee: format!("0x{}", "33".repeat(20)), work: format!("0x{n:064x}"), terms: format!("0x{}", "44".repeat(32)), ..Default::default() };
        app::entryx::seal(&secret, "grant", 1, Some(&genesis.id), app::grantx::grant_body(&d).expect("a body")).expect("a grant").bytes
    };
    let before_count = app::backup::count_now().expect("counted");
    app::vaultx::store(&home, &grant(1)).expect("held before the backup");
    assert_eq!(app::backup::count_now().expect("counted"), before_count + 1, "a held grant is counted where it lies");
    app::backup::export(&base.join("out"), "zikaron-backup-probe", 1_800_000_000).expect("a backup");
    let m = app::machine::read().expect("machine settings");
    assert!(m.backup.as_ref().map(|b| b.indexed).unwrap_or(false), "{:?}", m.backup);
    assert_eq!(behind(&m), Some(0), "the grant held before is in the backup");
    app::vaultx::store(&home, &grant(2)).expect("held after the backup");
    assert_eq!(behind(&m), Some(1), "the grant held after is one behind; the one before is not");
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

// ───────────────────────── the window's handle ─────────────────────────

/// Says to every frame that the window is maximized, as the windowing layer does for a maximized window.
struct SaysMaximized;

impl zikaron_ui::egui::Plugin for SaysMaximized {
    fn debug_name(&self) -> &'static str {
        "says-maximized"
    }
    fn input_hook(&mut self, input: &mut zikaron_ui::egui::RawInput) {
        let id = input.viewport_id;
        input.viewports.entry(id).or_default().maximized = Some(true);
    }
}

/// A double click on the window's handle restores a maximized window, as a title bar does: it asks
/// `Maximized(false)`, never `Maximized(true)`.
#[test]
fn a_double_click_on_the_handle_of_a_maximized_window_puts_it_back() {
    if super::alone_in(module_path!(), "a_double_click_on_the_handle_of_a_maximized_window_puts_it_back") {
        return;
    }
    use zikaron_ui::egui::{pos2, Event, PointerButton};
    vault_open();
    let at = pos2(zikaron_ui::tokens::RAIL_W + 200.0, 4.0);
    let press = |p, down| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: down, modifiers: Default::default() };
    let ctx = zikaron_ui::egui::Context::default();
    ctx.add_plugin(SaysMaximized);
    let shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let two = vec![(0.1, vec![Event::PointerMoved(at)]), (0.05, vec![press(at, true), press(at, false)]), (0.1, vec![press(at, true), press(at, false)]), (0.5, vec![])];
    let (_, said) = app::window::probe_shell_input(&ctx, shell, app::nav::Place::Home, two, 1180.0, 760.0);
    let sent: Vec<String> = said.into_iter().flat_map(|(_, c)| c).collect();
    assert!(sent.iter().any(|s| s.starts_with("Maximized(false)")), "a maximized window is put back: {sent:?}");
    assert!(!sent.iter().any(|s| s.starts_with("Maximized(true)")), "never maximized again: {sent:?}");
}
