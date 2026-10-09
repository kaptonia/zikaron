//! The chain and the disk: a home copied from another machine, the writer mark, the strict entry read, one
//! payload file read two ways, and the fingerprint gate's patience. Places are set before any vault or shell
//! use (`vault_open`); each test runs alone in its own process.

use super::vault_open;
use app::action::{apply, Action, Applied};

/// Every file under `dir` (relative name, bytes), in name order: what "not one byte changed" is compared over.
fn files(dir: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    fn walk(root: &std::path::Path, at: &std::path::Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(list) = std::fs::read_dir(at) else { return };
        for e in list.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                let rel = p.strip_prefix(root).map(|r| r.display().to_string()).unwrap_or_default();
                out.push((rel, std::fs::read(&p).unwrap_or_default()));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn copy_tree(from: &std::path::Path, to: &std::path::Path) {
    std::fs::create_dir_all(to).expect("copy root");
    for e in std::fs::read_dir(from).expect("list").flatten() {
        let p = e.path();
        let q = to.join(e.file_name());
        if p.is_dir() {
            copy_tree(&p, &q);
        } else {
            std::fs::copy(&p, &q).expect("copy");
        }
    }
}

/// A home copied from another machine (its label sealed under another key, read as
/// `local::Unread::Unopenable`, like an altered byte) with no writer mark of its own is refused by name at
/// opening, and not one byte of it changes: no writer mark, no lock contents, no label.
#[test]
fn a_home_copied_from_another_machine_is_refused_and_not_written() {
    if super::alone_in(module_path!(), "a_home_copied_from_another_machine_is_refused_and_not_written") {
        return;
    }
    vault_open();
    let base = std::env::temp_dir().join(format!("zk-r20b1-label-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (mine, theirs) = (base.join("mine"), base.join("theirs"));
    let ctx = zikaron_ui::egui::Context::default();
    {
        let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
        answers!(apply(&mut shell, Action::OpenHome { root: mine.display().to_string() }), Applied::Homed { .. });
    }
    copy_tree(&mine, &theirs);
    let label = app::local::label_path(&theirs);
    let mut b = std::fs::read(&label).expect("a label");
    let last = b.len() - 1;
    b[last] ^= 0x01;
    std::fs::write(&label, &b).expect("their label");
    let _ = std::fs::remove_file(app::lock::mark_path(&app::home::Home::open_or_create(&theirs).expect("a home")));
    app::local::forget_labels();
    let before = files(&theirs);
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    match apply(&mut shell, Action::OpenHome { root: theirs.display().to_string() }) {
        Applied::Trouble(f) => assert_eq!(app::local::Unread::of(&f), Some(app::local::Unread::Unopenable), "{f:?}"),
        other => panic!("opened: {other:?}"),
    }
    assert_eq!(files(&theirs), before, "not one byte of their home changed");
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// The writer mark, one line per form: no mark (the writer marks it as this machine's), this machine's mark (a
/// writer), another machine's (read-only, mark kept), a wrong shape, a later version's form, and an unreadable
/// mark (each read-only, its trouble named, the mark not rewritten: [`app::lock::Mark`]). The person's
/// take-over rewrites an unreadable mark to this machine's and writes.
#[test]
fn a_writer_mark_that_does_not_read_opens_read_only_and_is_kept() {
    if super::alone_in(module_path!(), "a_writer_mark_that_does_not_read_opens_read_only_and_is_kept") {
        return;
    }
    vault_open();
    let dir = std::env::temp_dir().join(format!("zk-r20b1-mark-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let home = app::home::Home::open_or_create(&dir).expect("a home");
    let me = app::lock::this_machine().expect("this machine's mark");
    let at = app::lock::mark_path(&home);
    // No mark: the writer marks it this machine's.
    {
        let l = app::lock::take(&home).expect("taken");
        assert_eq!((l.mode(), app::lock::mark_read(&home)), (app::lock::Mode::Writer, app::lock::Mark::Machine(me.clone())), "no mark");
    }
    // This machine's: a writer.
    assert_eq!(app::lock::take(&home).expect("taken").mode(), app::lock::Mode::Writer, "this machine's mark");
    // Another machine's: read-only, no trouble, kept.
    let other = "0123456789abcdef0123456789abcdef\n";
    std::fs::write(&at, other).unwrap();
    {
        let l = app::lock::take(&home).expect("taken");
        assert_eq!((l.mode(), l.mark_trouble().is_some()), (app::lock::Mode::OtherMachine, false), "another machine's");
    }
    assert_eq!(std::fs::read_to_string(&at).unwrap(), other);
    // A wrong shape and a later version's form: read-only, named, kept byte for byte.
    for (form, bytes) in [("wrongShape", "not a mark\n".to_string()), ("laterVersion", format!("{{\"form\":\"writer-mark/2\",\"machine\":\"{me}\"}}\n"))] {
        std::fs::write(&at, &bytes).unwrap();
        let l = app::lock::take(&home).expect("taken");
        assert_eq!(l.mode(), app::lock::Mode::OtherMachine, "{form}");
        let f = l.mark_trouble().cloned().expect("named");
        assert_eq!(f.which(), Some(app::fault::Known::SettingsShape), "{form}");
        assert!(l.holder().contains(&at.display().to_string()), "{form}: the holder words name the mark: {}", l.holder());
        drop(l);
        assert_eq!(std::fs::read_to_string(&at).unwrap(), bytes, "{form}: not rewritten");
    }
    // A mark that cannot be read at all (a folder where the file is): read-only, named, left as it is.
    std::fs::remove_file(&at).unwrap();
    std::fs::create_dir(&at).unwrap();
    {
        let l = app::lock::take(&home).expect("taken");
        assert_eq!((l.mode(), l.mark_trouble().is_some()), (app::lock::Mode::OtherMachine, true), "unreadable");
    }
    assert!(at.is_dir(), "left as it is");
    std::fs::remove_dir(&at).unwrap();
    // The person's take-over of an unreadable mark: this machine's, and a writer.
    std::fs::write(&at, "not a mark\n").unwrap();
    let mut l = app::lock::take(&home).expect("taken");
    l.take_over(&home).expect("taken over");
    assert_eq!((l.mode(), l.mark_trouble().is_some(), app::lock::mark_of(&home)), (app::lock::Mode::Writer, false, Some(me)));
    drop(l);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The strict read and the head read, one line per form: a zero-byte entry file is refused by name
/// (`local::EMPTY` and its name), never skipped; a file of one newline and one of words that are not a sealed
/// entry are refused by name as not sealed; a ledger of whole entries reads.
#[test]
fn an_empty_entry_file_is_named_by_the_strict_read() {
    if super::alone_in(module_path!(), "an_empty_entry_file_is_named_by_the_strict_read") {
        return;
    }
    vault_open();
    let base = std::env::temp_dir().join(format!("zk-r20b1-empty-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    answers!(apply(&mut shell, Action::MakeAnchorKey), Applied::AnchorKey(_));
    answers!(apply(&mut shell, Action::OpenHome { root: base.display().to_string() }), Applied::Homed { .. });
    answers!(apply(&mut shell, Action::Genesis { statement: "a ledger".into() }), Applied::Genesised { .. });
    let home = shell.home.as_ref().expect("a home");
    // Whole entries: both reads go through.
    assert_eq!(home.ledger().and_then(|l| l.pile()).expect("the strict read").items.len(), 1, "whole");
    assert!(app::ledgerx::head(home).expect("the head").is_some(), "whole");
    let ledger = home.dir(app::home::Slot::Ledger);
    let name = format!("{}{}", "ab".repeat(32), zikaron_store::layout::ENTRY_SUFFIX);
    for (form, bytes) in [("empty", &b""[..]), ("newlineOnly", &b"\n"[..]), ("notSealed", &b"{\"spec\":"[..])] {
        std::fs::write(ledger.join(&name), bytes).unwrap();
        let strict = home.ledger().and_then(|l| l.pile()).expect_err(form);
        let head = app::ledgerx::head(home).expect_err(form);
        if form == "empty" {
            for f in [&strict, &head] {
                assert_eq!(f.which(), Some(app::fault::Known::Ledger), "{form}: {f:?}");
                assert_eq!(f.tail(), format!("{} {name}", app::local::EMPTY), "{form}: named");
            }
        } else {
            assert_eq!(app::local::Unread::of(&strict), Some(app::local::Unread::NotSealed), "{form}: {strict:?}");
            assert_eq!(head.which(), Some(app::fault::Known::Ledger), "{form}: {head:?}");
        }
        std::fs::remove_file(ledger.join(&name)).unwrap();
    }
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// One payload file read by both the vault's take (`vaultx::take`) and the check page's (`checkx::hops_of`):
/// with a trailing newline, with line ends and indentation around it, and a payload that does not decode, both
/// give the same answer (the same hops, or the same refusal).
#[test]
fn one_payload_file_reads_the_same_at_both_mouths() {
    if super::alone_in(module_path!(), "one_payload_file_reads_the_same_at_both_mouths") {
        return;
    }
    vault_open();
    let base = std::env::temp_dir().join(format!("zk-r20b1-mouths-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    std::fs::create_dir_all(&base).unwrap();
    // Two grants by one key, the second following the first (as `gaps_b10m::grants` makes them).
    let secret = app::key::Secret::take([0x5b; 32]).expect("a key");
    let genesis = app::entryx::genesis(&secret, "r20b1").expect("genesis");
    let mut gs: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..2u64 {
        let d = app::grantx::Draft {
            grantee: format!("0x{}", "33".repeat(20)),
            work: format!("0x{:064x}", i + 1),
            terms: format!("0x{}", "44".repeat(32)),
            upstream: gs.last().map(|(id, _)| id.clone()).unwrap_or_default(),
            ..Default::default()
        };
        let g = app::entryx::seal(&secret, "grant", i + 1, Some(&genesis.id), app::grantx::grant_body(&d).expect("a body")).expect("a grant");
        gs.push((g.id, g.bytes));
    }
    let code = zikaron_kit::badge::encode(&gs.iter().map(|(_, b)| b.clone()).collect::<Vec<_>>()).expect("a payload");
    let broken = format!("{}zz", zikaron_kit::tokens::BADGE_PREFIX);
    for (form, text) in [("trailingNewline", format!("{code}\n")), ("lineEndsAndIndent", format!("  {code}\r\n\r\n")), ("doesNotDecode", format!("{broken}\n"))] {
        let at = base.join(form);
        std::fs::write(&at, &text).unwrap();
        let typed = at.display().to_string();
        let vault = app::vaultx::take(&typed);
        let check = app::checkx::hops_of(&typed).map(|h| h.hops);
        match (&vault, &check) {
            (Ok(a), Ok(b)) => assert_eq!(a, b, "{form}: the same hops"),
            (Err(a), Err(b)) => assert_eq!((a.which(), a.tail()), (b.which(), b.tail()), "{form}: the same refusal"),
            _ => panic!("{form}: two answers: {vault:?} · {check:?}"),
        }
    }
    let _ = std::fs::remove_dir_all(&base);
}

use std::sync::atomic::{AtomicUsize, Ordering};

static CODE_ONCE: AtomicUsize = AtomicUsize::new(0);
static CHAIN_ONCE: AtomicUsize = AtomicUsize::new(0);
static LIMITED: AtomicUsize = AtomicUsize::new(0);
static REFUSED: AtomicUsize = AtomicUsize::new(0);

/// The read-only networks' fingerprint gate asks both its questions with patience, one line per form: the code
/// question rate-limited once, then answered (the node counts, asked twice); the chain question limited once,
/// then answered (counts); limited to the end of the table (does not count, reported as rate limited, asked once
/// plus once per pause); refused with a status for another reason (does not count, asked once).
#[test]
fn the_fingerprint_gate_waits_out_a_rate_limit() {
    if super::alone_in(module_path!(), "the_fingerprint_gate_waits_out_a_rate_limit") {
        return;
    }
    use app::widex::{gate_said_against, Reading};
    zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
    let pin = super::test_pin();
    let code_once = super::rpc_node(|m| {
        Some(match m {
            "eth_chainId" => "\"0x7a69\"".into(),
            "eth_getCode" if CODE_ONCE.fetch_add(1, Ordering::SeqCst) == 0 => "HTTP 429 Too Many Requests".into(),
            "eth_getCode" => "\"0x60\"".into(),
            _ => "null".into(),
        })
    });
    let chain_once = super::rpc_node(|m| {
        Some(match m {
            "eth_chainId" if CHAIN_ONCE.fetch_add(1, Ordering::SeqCst) == 0 => "HTTP 429 Too Many Requests".into(),
            "eth_chainId" => "\"0x7a69\"".into(),
            "eth_getCode" => "\"0x60\"".into(),
            _ => "null".into(),
        })
    });
    let limited = super::rpc_node(|m| {
        Some(match m {
            "eth_chainId" => "\"0x7a69\"".into(),
            _ => {
                LIMITED.fetch_add(1, Ordering::SeqCst);
                "HTTP 429 Too Many Requests".into()
            }
        })
    });
    let refused = super::rpc_node(|m| {
        Some(match m {
            "eth_chainId" => "\"0x7a69\"".into(),
            _ => {
                REFUSED.fetch_add(1, Ordering::SeqCst);
                "HTTP 403 Forbidden".into()
            }
        })
    });
    let read = |node: &String| gate_said_against(&super::net_of(vec![node.clone()]), &pin);
    assert_eq!(read(&code_once).0, Reading::Single, "the code question limited once");
    assert_eq!(CODE_ONCE.load(Ordering::SeqCst), 2);
    assert_eq!(read(&chain_once).0, Reading::Single, "the chain question limited once");
    assert_eq!(CHAIN_ONCE.load(Ordering::SeqCst), 2);
    let (reading, said) = read(&limited);
    assert_eq!((reading, said.map(|f| f.said().starts_with("RATE_LIMITED"))), (Reading::Down, Some(true)), "limited to the end");
    assert_eq!(LIMITED.load(Ordering::SeqCst), 1 + zikaron_anchor::patience::RATE_LIMITED.len());
    let (reading, _) = read(&refused);
    assert_eq!(reading, Reading::Down, "refused for another reason");
    assert_eq!(REFUSED.load(Ordering::SeqCst), 1, "asked once");
}
