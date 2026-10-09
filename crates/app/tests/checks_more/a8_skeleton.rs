//! The app's skeleton, checked by running its own entry points (shell boot, the action layer's `Quit`) or,
//! where the failing branch cannot be reached from outside, by reading the source: a missing font role is
//! reported by name, closing the window waits for writes in flight, and a QR code is handed out only once it
//! reads back. Places are set before any shell use (`vault_open`); each test runs alone in its own process.

use super::{code_only, read_src_file, vault_open};
use app::action::{apply, Action, Applied};
use app::task::{Done, Kind};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn boot(dressed: zikaron_ui::skin::Dressed) -> app::shell::Shell {
    vault_open();
    app::shell::Shell::boot(dressed)
}

/// A missing font role is reported by name (`FONT_MISSING`), never drawn silently as boxes.
#[test]
fn a_missing_font_role_is_said_by_name_when_the_shell_boots() {
    if super::alone_in(module_path!(), "a_missing_font_role_is_said_by_name_when_the_shell_boots") {
        return;
    }
    let ctx = zikaron_ui::egui::Context::default();
    let mut dressed = zikaron_ui::skin::dress(&ctx);
    dressed.found = zikaron_ui::fonts::Found::none();
    dressed.missing = vec![zikaron_ui::fonts::Role::Cjk, zikaron_ui::fonts::Role::Strong];
    let shell = boot(dressed);
    let said: Vec<String> = shell.faults.iter().filter(|f| f.which() == Some(app::fault::Known::FontMissing)).map(|f| f.raw()).collect();
    assert_eq!(said.len(), 2, "one fault per missing role: {said:?}");
    for role in ["cjk", "strong"] {
        assert!(said.iter().any(|s| s.contains(&format!("role {role}"))), "the role {role} is named: {said:?}");
    }
    assert_eq!(shell.font_missing, vec![zikaron_ui::fonts::Role::Cjk, zikaron_ui::fonts::Role::Strong]);
}

/// Quitting returns only after a write in flight has landed; a network task is left running and named, so
/// quitting never hangs on it.
#[test]
fn quitting_returns_only_after_a_write_in_flight_has_landed() {
    if super::alone_in(module_path!(), "quitting_returns_only_after_a_write_in_flight_has_landed") {
        return;
    }
    // The window's close goes through the action layer's quit.
    let window = code_only(&read_src_file("window/mod.rs").expect("window/mod.rs"));
    let at = window.find("fn on_exit(").expect("the window's close");
    let body = &window[at..at + window[at..].find("\n    }\n").expect("its end")];
    assert!(body.contains("apply(&mut self.shell, Action::Quit)"), "the window's close goes through Quit");
    assert!(Kind::Backup.waited_at_quit() && !Kind::Chain.waited_at_quit());

    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = boot(zikaron_ui::skin::dress(&ctx));
    let dir = std::env::temp_dir().join(format!("zk-a8-quit-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("a directory to write into");
    let bytes: Vec<u8> = (0..64 * 1024).map(|i| (i % 251) as u8).collect();
    // The write is held until `go`, released from elsewhere shortly after quitting begins.
    let go = Arc::new(AtomicBool::new(false));
    let (held, d, b) = (go.clone(), dir.clone(), bytes.clone());
    let _ = shell.tasks.spawn(Kind::Backup, move || {
        while !held.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        app::home::put_at(&d, "landed.bin", &b).map(|_| Done::Reconciled { label: String::new(), complete: true, entries: 0 })
    });
    // A network task that never ends by itself while quitting.
    let stay = Arc::new(AtomicBool::new(false));
    let staying = stay.clone();
    let _ = shell.tasks.spawn(Kind::Chain, move || {
        while !staying.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(1));
        }
        Err(app::fault::Fault::known(app::fault::Known::Unreachable, String::new()))
    });
    assert!(shell.tasks.in_flight(Kind::Backup) && shell.tasks.in_flight(Kind::Chain));
    assert!(!dir.join("landed.bin").exists(), "nothing landed before quitting");
    let release = go.clone();
    let later = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        release.store(true, Ordering::SeqCst);
    });
    let started = std::time::Instant::now();
    let reaped = match apply(&mut shell, Action::Quit) {
        Applied::Stopped(r) => r,
        other => panic!("quit answered {other:?}"),
    };
    let took = started.elapsed();
    // When quit returns, the write has landed whole.
    assert_eq!(std::fs::read(dir.join("landed.bin")).ok(), Some(bytes), "the write in flight landed before quit returned");
    assert!(took >= Duration::from_millis(250), "quit waited for it ({took:?})");
    assert!(reaped.waited.contains(&Kind::Backup) && reaped.joined == 1, "the backup was waited for and joined: {reaped:?}");
    assert!(reaped.not_waited.contains(&Kind::Chain), "the network task was left and named: {reaped:?}");
    assert!(!stay.load(Ordering::SeqCst), "quit returned while the network task was still running");
    stay.store(true, Ordering::SeqCst);
    let _ = later.join();
    let _ = std::fs::remove_dir_all(&dir);
}

/// A QR code is read back by our own reader before it is handed out and refused by name otherwise. Checked
/// from the source, since no input reaches the refusing branch.
#[test]
fn a_qr_code_is_handed_out_only_once_it_reads_back() {
    // The shipped part of qr.rs (its tests build damaged codes on purpose).
    let qr = code_only(&read_src_file("qr.rs").expect("qr.rs"));
    let qr = qr.split("#[cfg(test)]").next().unwrap_or("").to_string();
    const READ_BACK: &str = "Ok(back) if back == bytes => Ok(code),";
    const REFUSED: &str = "_ => Err(NotMade::SelfCheck),";
    let mut makers = Vec::new();
    for (at, _) in qr.match_indices("\npub fn ") {
        let sig_end = at + qr[at..].find('{').expect("a body");
        let sig = &qr[at..sig_end];
        if !(sig.contains("-> Result<Code") || sig.contains("-> Option<Code")) {
            continue;
        }
        let body = &qr[sig_end..sig_end + qr[sig_end..].find("\n}\n").expect("its end")];
        let name = sig.trim().trim_start_matches("pub fn ").split('(').next().unwrap_or("").to_string();
        if sig.contains("-> Result<Code") {
            assert!(body.contains("match decode(&code) {") && body.contains(READ_BACK) && body.contains(REFUSED), "{name} hands out a code only once it reads back:\n{body}");
            assert_eq!(body.matches("Ok(").count(), 1 + body.matches("Ok(back)").count(), "{name}: the read-back arm is its only Ok");
        } else {
            assert!(body.trim_start_matches('{').trim().starts_with("make(bytes)"), "{name} is `make` itself:\n{body}");
        }
        makers.push(name);
    }
    makers.sort();
    assert_eq!(makers, vec!["encode", "make", "make_with_mask"], "the public makers of a code");
    assert_eq!(qr.matches("Code { version").count(), 1, "one place draws a code (the private `drawn`)");
    // No shipped source outside qr.rs builds a code by hand (its fields are public).
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(dir).expect("a source folder").flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                out.push(p);
            }
        }
    }
    let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut files = Vec::new();
    for e in std::fs::read_dir(&crates).expect("the crates").flatten() {
        let s = e.path().join("src");
        if s.is_dir() {
            walk(&s, &mut files);
        }
    }
    assert!(files.len() > 50, "the scan reached the sources");
    for f in files.iter().filter(|f| !f.ends_with("app/src/qr.rs")) {
        let code = code_only(&std::fs::read_to_string(f).expect("a source"));
        assert!(!code.contains("qr::Code {") && !code.contains("Code { version"), "{} builds a QR code by hand", f.display());
    }
    // The badge, the one export of a code, reports the refusal in its own sentence.
    let badge = code_only(&read_src_file("badgex.rs").expect("badgex.rs"));
    assert!(badge.contains("crate::qr::NotMade::SelfCheck => crate::lang::Key::Tail237,"), "the badge says the refusal in its own sentence");
    let line = |k: app::lang::Key| app::lang::TABLE.iter().find(|(x, _, _)| *x == k).map(|(_, z, e)| (*z, *e)).expect("the sentence");
    let (zh, en) = line(app::lang::Key::Tail237);
    assert!(!zh.trim().is_empty() && !en.trim().is_empty(), "both languages");
    assert_ne!((zh, en), line(app::lang::Key::Tail088), "not the too-long sentence");
}
