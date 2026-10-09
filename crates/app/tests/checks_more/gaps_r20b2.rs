//! The window and the command line: importing again, the passcode-taking actions, the recovery words' secret
//! type, the read-only bar, the backup lamp, English first-layer sentences, the record place, the failed-backup
//! mark, the backup package's byte form, and typed node items never echoed. Places are set before any vault or
//! shell use (`vault_open`); each test runs alone in its own process.

use super::vault_open;
use app::action::{apply, Action, Applied};

fn settled(shell: &mut app::shell::Shell, k: app::task::Kind) {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while shell.tasks.in_flight(k) && std::time::Instant::now() < until {
        shell.drain();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    shell.drain();
}

/// One import, landed: the code it was refused with (empty when it landed).
fn import(shell: &mut app::shell::Shell, form: app::action::ImportForm, label: &str) -> String {
    let before = shell.faults.len();
    let started = apply(shell, Action::ImportIdentity { form, seat: app::roles::Role::Author, label: label.into(), network: app::deploy::CUSTOM.into() });
    if let Applied::Trouble(f) = started {
        return f.which().map(|k| k.as_str().to_string()).unwrap_or_default();
    }
    settled(shell, app::task::Kind::Vault);
    shell.faults[before.min(shell.faults.len())..].last().and_then(|f| f.which()).map(|k| k.as_str().to_string()).unwrap_or_default()
}

fn rows() -> Vec<(String, String)> {
    app::register::read().ok().flatten().map(|r| r.rows.iter().map(|x| (x.id.clone(), x.label.clone())).collect()).unwrap_or_default()
}

const WORDS_A: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const WORDS_B: &str = "legal winner thank year wave sausage worth useful legal winner thank yellow";

/// Importing again, one line per form, refused by name with nothing written: the same identity under the same
/// name (`IDENTITY_EXISTS`); the same identity under another name (`IDENTITY_HERE_AS`, the row keeps its name);
/// another identity under a name already used here (`IDENTITY_NAME_TAKEN`, no row added); a key imported again
/// with a key file named (refused before the file is written). A new identity under a new name, and one with no
/// name, land.
#[test]
fn importing_an_identity_again_is_answered_by_name() {
    if super::alone_in(module_path!(), "importing_an_identity_again_is_answered_by_name") {
        return;
    }
    vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let words = |w: &str| app::action::ImportForm::Words(w.to_string().into());
    assert_eq!(import(&mut shell, words(WORDS_A), "alpha"), "", "a new identity lands");
    let one = rows();
    assert_eq!(one.len(), 1);
    assert_eq!(import(&mut shell, words(WORDS_A), "alpha"), app::fault::Known::IdentityExists.as_str(), "same identity, same name");
    assert_eq!(import(&mut shell, words(WORDS_A), "beta"), app::fault::Known::IdentityHereAs.as_str(), "same identity, another name");
    assert_eq!(rows(), one, "not renamed");
    assert_eq!(import(&mut shell, words(WORDS_B), "alpha"), app::fault::Known::IdentityNameTaken.as_str(), "another identity, a name taken");
    assert_eq!(rows(), one, "no row added");
    assert_eq!(import(&mut shell, words(WORDS_B), ""), "", "another identity, no name");
    assert_eq!(rows().len(), 2);
    // A key, imported (secondary: no key file needed), then again with a key file named.
    let s = app::key::generate().expect("a key");
    let mut slot = Some(s);
    let hex = app::key::reveal_once(&mut slot).expect("its hex");
    let key = |file: Option<std::path::PathBuf>| app::action::ImportForm::PrivateKey {
        key: hex.clone().into(),
        keyfile: file.map(|d| app::action::KeyFileOut { password: "a long key file password".to_string().into(), again: "a long key file password".to_string().into(), dir: d.display().to_string() }),
    };
    assert_eq!(import(&mut shell, key(None), "gamma"), "", "a key lands");
    let dir = std::env::temp_dir().join(format!("zk-r20b2-keyfile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    assert_eq!(import(&mut shell, key(Some(dir.clone())), "gamma"), app::fault::Known::IdentityExists.as_str(), "the same key again");
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "no key file written for an import refused");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The actions that take the passcode themselves (set, open, change, reseal, the two recoveries) are named by one
/// closed function (`gate_pin`), apart from the passcode gate (`pin_asked`): each returns the passcode it hands
/// on, and forgetting wipes it (and every other passcode cell the action carries) in place, before anything is
/// handed.
#[test]
fn the_gates_own_mouths_name_the_passcode_they_hand_and_forget_it() {
    let typed = || app::secret::Secret::from(String::from("q7w8e9r0"));
    let mouths = [
        Action::SetPin { pin: typed(), again: typed() },
        Action::Unlock { pin: typed() },
        Action::ChangePin { old: typed(), pin: typed(), again: typed() },
        Action::Reseal { pin: typed() },
        Action::RecoverWords { words: typed(), pin: typed(), again: typed() },
        Action::RecoverKeystore { path: String::new(), password: typed(), pin: typed(), again: typed() },
        // Actions that open the vault with the passcode inside their own task take the passcode themselves too.
        Action::SetPrimary { id: String::new(), pin: typed() },
        Action::RestoreBackup { path: String::new(), password: typed(), how: app::action::RestoreHow::Settings { pin: typed() } },
        Action::RestoreBackup { path: String::new(), password: typed(), how: app::action::RestoreHow::Locked { pin: typed(), again: typed() } },
    ];
    for mut a in mouths {
        assert!(a.pin_asked().is_none(), "{}: the gate itself does not pass the gate", a.name());
        assert_eq!(a.gate_pin().map(|p| p.expose().to_string()), Some("q7w8e9r0".to_string()), "{}", a.name());
        a.forget_pin();
        assert_eq!(a.gate_pin().map(|p| p.expose().len()), Some(0), "{}: wiped", a.name());
    }
    let asked = Action::AttestFor { text: String::new(), pin: typed() };
    assert!(asked.gate_pin().is_none() && asked.pin_asked().is_some(), "an action that passes the gate is not a gate mouth");
}

/// The twelve words shown to a person are held in the secret type (each word one block, zeroed when cleared and
/// when dropped; the type's own tests check the bytes), one line per form: shown after the passcode (twelve
/// words), hidden (gone), shown again then locked (gone), shown again then quit (gone, with the unconfirmed words).
/// The fresh words' display copy is the same type.
#[test]
fn the_words_shown_are_secrets_and_are_let_go() {
    if super::alone_in(module_path!(), "the_words_shown_are_secrets_and_are_let_go") {
        return;
    }
    vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    answers!(apply(&mut shell, Action::NewIdentity), Applied::FreshWords);
    let (list, picks) = shell.new_words.as_ref().map(|f| (f.words(), f.picks)).expect("fresh words");
    let typed: &Vec<app::secret::Secret> = &list;
    assert_eq!(typed.len(), 12);
    let answers: Vec<(usize, app::secret::Secret)> = picks.iter().map(|i| (*i, list[*i].clone())).collect();
    apply(&mut shell, Action::ConfirmIdentity { answers, label: "a2".into(), network: app::deploy::CUSTOM.into() });
    settled(&mut shell, app::task::Kind::Vault);
    let pin = || app::secret::Secret::from(String::from("27618394"));
    let reveal = |shell: &mut app::shell::Shell| {
        apply(shell, Action::RevealWords { pin: pin() });
        settled(shell, app::task::Kind::Vault);
        shell.words.as_ref().map(|w| w.len())
    };
    assert_eq!(reveal(&mut shell), Some(12), "shown");
    apply(&mut shell, Action::HideWords);
    assert!(shell.words.is_none(), "hidden");
    assert_eq!(reveal(&mut shell), Some(12));
    apply(&mut shell, Action::Lock);
    assert!(shell.words.is_none(), "locked");
    apply(&mut shell, Action::Unlock { pin: pin() });
    settled(&mut shell, app::task::Kind::Vault);
    assert_eq!(reveal(&mut shell), Some(12));
    answers!(apply(&mut shell, Action::NewIdentity), Applied::FreshWords);
    apply(&mut shell, Action::Quit);
    assert!(shell.words.is_none() && shell.new_words.is_none(), "quit");
}

/// The bar over a home opened read-only says why: a writer mark naming another machine (`other_machine`), or a
/// writer mark this version cannot read (`mark_unread`, never "another machine writes"); both keep the person's
/// way to write from this machine. The sentence is the table's, in both languages.
#[test]
fn the_bar_says_a_mark_that_does_not_read_as_such() {
    if super::alone_in(module_path!(), "the_bar_says_a_mark_that_does_not_read_as_such") {
        return;
    }
    vault_open();
    let base = std::env::temp_dir().join(format!("zk-r20b2-bar-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    answers!(apply(&mut shell, Action::OpenHome { root: base.display().to_string() }), Applied::Homed { .. });
    let at = app::lock::mark_path(shell.home.as_ref().expect("a home"));
    for (form, bytes, bar) in [("anotherMachine", "0123456789abcdef0123456789abcdef\n", "other_machine"), ("wrongShape", "not a mark\n", "mark_unread")] {
        std::fs::write(&at, bytes).unwrap();
        shell.lock = None;
        let opened = apply(&mut shell, Action::OpenHome { root: base.display().to_string() });
        assert!(matches!(opened, Applied::Homed { .. }), "{form}: {opened:?}");
        assert_eq!(shell.banner().as_str(), bar, "{form}");
    }
    let (zh, en) = app::lang::TABLE.iter().find(|(k, _, _)| *k == app::lang::Key::SetMarkUnread).map(|(_, z, e)| (*z, *e)).expect("the sentence");
    assert!(!zh.contains("另一台机器在写") && !en.contains("Another machine is writing"), "never says another machine writes");
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}

/// The backup lamp's four colours, one line per form, in both the watch table and the setup point: backed up
/// and none behind is green; behind is amber; never backed up is grey (said, not red); a last backup attempt that
/// failed is red, even over a backup on record. A count not yet measured stays grey.
#[test]
fn the_backup_lamp_has_four_colours() {
    use app::firstrun::{backup_point, Shade};
    use app::watchx::{Item, Light, Say};
    let b = |count: u64| app::machine::Backed { at: 1, path: String::new(), count, indexed: false };
    let (two, four) = (b(2), b(4));
    let lamp = |backup: (Option<&app::machine::Backed>, Option<u64>, Option<u64>)| {
        let t = app::watchx::author(None, 0, None, backup, None, None);
        t.into_iter().find(|r| r.item == Item::Backup).map(|r| (r.light, r.gap.map(|g| g.say))).expect("the backup row")
    };
    for (form, backup, light, say, shade) in [
        ("fresh", (Some(&four), Some(4), None), Light::Ok, None, Shade::Green),
        ("behind", (Some(&two), Some(4), None), Light::Warn, Some(Say::BackupBehind), Shade::Amber),
        ("never", (None, Some(4), None), Light::Unknown, Some(Say::BackupNever), Shade::Grey),
        ("failed", (Some(&four), Some(4), Some(9)), Light::Bad, Some(Say::BackupFailed), Shade::Red),
        ("failedNever", (None, Some(4), Some(9)), Light::Bad, Some(Say::BackupFailed), Shade::Red),
        ("unmeasured", (Some(&four), None, None), Light::Unknown, Some(Say::BackupRead), Shade::Grey),
    ] {
        let (l, s) = lamp(backup);
        assert_eq!(l, light, "{form}");
        assert_eq!(s, say, "{form}");
        assert_eq!(backup_point(backup.0, backup.1, backup.2), shade, "{form}");
    }
}

/// The English first layer has no Chinese evidence words, as a class: the Chinese gloss enters a fault only
/// through `said()` / `evidence()`, and the window never calls either (its first layer shows the table's sentence,
/// `human()` / `next()`; the evidence goes under Details and to the log). In English, every first-layer sentence a
/// refusal can give is free of CJK: each known fault's (with a Chinese tail), each refused file's, by fault, by
/// the system's error and by an unknown fault.
#[test]
fn the_english_first_layer_has_no_cjk() {
    if super::alone_in(module_path!(), "the_english_first_layer_has_no_cjk") {
        return;
    }
    let window = super::shipped().into_iter().find(|(n, _)| n == "window.rs").map(|(_, t)| super::code_only(&t)).expect("the window");
    for door in [".said()", ".evidence()"] {
        assert!(!window.contains(door), "the window calls {door}: the evidence is not the first layer");
    }
    struct Back(app::lang::Lang);
    impl Drop for Back {
        fn drop(&mut self) {
            app::lang::set(self.0);
        }
    }
    let _back = Back(app::lang::lang());
    app::lang::set(app::lang::Lang::En);
    let cjk = |s: &str| s.chars().any(|c| ('\u{2e80}'..='\u{9fff}').contains(&c) || ('\u{f900}'..='\u{faff}').contains(&c) || ('\u{ff00}'..='\u{ffef}').contains(&c));
    for k in app::fault::Known::ALL {
        let f = app::fault::Fault::known(k, "节点答:读不成");
        assert!(!cjk(f.human()), "{k:?}: first sentence {:?}", f.human());
        assert!(!cjk(app::lang::t(k.next())), "{k:?}: next sentence");
        let r = app::verifyx::Rejected::of_fault("x.zk1", &f);
        assert!(!cjk(&r.human()), "{k:?}: refused file {:?}", r.human());
    }
    let io = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
    let unknown = app::fault::Fault::unknown("读不成");
    for r in [app::verifyx::Rejected::io("x.zk1", &io), app::verifyx::Rejected::of_fault("x.zk1", &unknown), app::verifyx::Rejected::plain("x.zk1", "读不成")] {
        assert!(!cjk(&r.human()), "{r:?}: {:?}", r.human());
    }
}

/// The record place on the reader and diligence pages is a single place: more than one non-empty line is refused
/// by name (`PLACE_ONE_LINE`, its tail the line count), one line per form: two lines, three lines, two lines
/// ended CRLF, two lines split by a lone CR. One line (with a trailing newline, or blank lines around it) is read
/// as that line. Both pages refuse on the spot, before any task starts.
#[test]
fn a_place_on_more_than_one_line_is_refused_by_name() {
    if super::alone_in(module_path!(), "a_place_on_more_than_one_line_is_refused_by_name") {
        return;
    }
    let code = |r: Result<&str, app::fault::Fault>| r.err().and_then(|f| f.which()).map(|k| (k, String::new()));
    let one_line = app::fault::Known::PlaceOneLine;
    for (form, at, lines) in [("twoLines", "/a\n/b", 2), ("threeLines", "/a\n/b\n/c", 3), ("crlf", "/a\r\n/b\r\n", 2), ("loneCr", "/a\r/b", 2)] {
        let f = app::action::one_place(at).err().unwrap_or_else(|| panic!("{form} refused"));
        assert_eq!(f.which(), Some(one_line), "{form}");
        assert_eq!(f.tail(), lines.to_string(), "{form}");
    }
    for (form, at) in [("one", "/a"), ("trailingNewline", "/a\n"), ("blankLinesRound", "\n  /a  \r\n\n"), ("empty", "")] {
        assert_eq!(code(app::action::one_place(at)), None, "{form}");
        assert_eq!(app::action::one_place(at).ok(), Some(at.trim()), "{form}");
    }
    vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let who = format!("0x{}", "5d".repeat(20));
    for page in ["reader", "diligence"] {
        let a = if page == "reader" {
            Action::ReadBook { address: who.clone(), dir: "/a\n/b".into() }
        } else {
            Action::Diligence { address: who.clone(), dir: "/a\r\n/b".into(), work: String::new(), from: String::new(), to: String::new() }
        };
        match apply(&mut shell, a) {
            Applied::Trouble(f) => assert_eq!(f.which(), Some(one_line), "{page}"),
            other => panic!("{page}: {other:?}"),
        }
    }
}

/// The failed-backup mark is this machine's record of its own attempt: a backup made while an earlier failed
/// attempt's mark stood carries it in its machine settings, and restoring that backup does not bring the red
/// back; an export that lands clears the mark before anything else is written.
#[test]
fn a_restored_backup_brings_no_failed_mark_back() {
    if super::alone_in(module_path!(), "a_restored_backup_brings_no_failed_mark_back") {
        return;
    }
    vault_open();
    let out = std::env::temp_dir().join(format!("zk-r20b3-restore-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    app::machine::update(|m| m.backup_failed = Some(5)).expect("the mark laid");
    let made = app::backup::export(&out, "a long backup password", 1_800_000_000).expect("the export lands");
    assert_eq!(app::machine::read().expect("settings").backup_failed, None, "cleared by the export that landed");
    app::machine::update(|m| m.backup_failed = Some(9)).expect("the mark laid again");
    app::backup::restore(&made.path, "a long backup password", app::backup::From::Settings("27618394")).expect("restored");
    let after = app::machine::read().expect("settings");
    assert_eq!(after.backup_failed, None, "the restored record is the backup that landed, no red");
    assert!(after.backup.is_some());
    let _ = std::fs::remove_dir_all(&out);
}

/// The backup package writes every share of bytes in one place, `backup::transcribe`: in the package's assembly
/// (`package_bytes`) every share (keys, files, machine settings, read-only networks, register) goes through it,
/// with no other byte spelling; the form is the package's own (`0x` and lowercase hex), so packages written by
/// older versions hold the same bytes.
#[test]
fn every_share_of_the_backup_package_is_transcribed_in_one_place() {
    let text = super::read_src_file("backup.rs").expect("backup.rs");
    let code = super::code_only(&text);
    let start = code.find("fn package_bytes(").expect("the package's assembly");
    let body = &code[start..start + code[start..].find("\n}\n").expect("its end")];
    assert_eq!(body.matches("transcribe(").count(), 5, "keys, files, machine, read-only networks, register: {body}");
    for other in ["hex(", "encode(", "b64", "base64", "to_string(", "format!("] {
        let outside = body.replace("transcribe(", "");
        assert!(!outside.contains(other), "a share spelled another way ({other}) in the package's assembly");
    }
    assert_eq!(app::backup::transcribe(&[0x00, 0xab, 0xff]), "0x00abff");
    assert_eq!(app::backup::transcribe(b""), "0x");
}

/// No node item a person types reaches a sentence as typed: on the check page, node lines that do not read (white
/// space in them, a user name in the address, a non-numeric port), each carrying a key, are refused as an
/// addresses' shape error with every line shown as the node reader renders it, so the key appears in neither the
/// sentence, the evidence nor the next step; the settings' node cells (`SetEndpoints`) refuse the same lines the
/// same way.
#[test]
fn a_node_line_typed_wrong_is_refused_without_its_key() {
    if super::alone_in(module_path!(), "a_node_line_typed_wrong_is_refused_without_its_key") {
        return;
    }
    const KEY: &str = "k3yInTheLine77";
    vault_open();
    let lines = [format!("31337=http://127.0.0.1:1/{KEY} x"), format!("31337=http://door:{KEY}@127.0.0.1:1/rpc"), format!("31337=http://127.0.0.1:port/{KEY}")];
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let seen = |f: &app::fault::Fault| format!("{} {} {} {} {}", f.said(), f.tail(), f.evidence(), f.human(), f.raw());
    // The check page, all three lines at once.
    let a = apply(&mut shell, Action::CheckPayload { typed: "0x01".into(), ledgers: String::new(), endpoints: lines.join("\n"), registry: String::new(), from_block: String::new(), now: String::new(), file: String::new(), terms: String::new() });
    let Applied::Trouble(f) = a else { panic!("refused on the spot: {a:?}") };
    assert_eq!(f.which(), Some(app::fault::Known::SettingsShape));
    assert!(!seen(&f).contains(KEY), "the check page says the key: {}", seen(&f));
    for line in &lines {
        assert!(f.tail().contains(&zikaron_net::sayable(line)) || f.tail().contains(&zikaron_net::sayable(line.split_once('=').map(|(_, u)| u).unwrap_or(line))), "each line said as a node is: {}", f.tail());
    }
    // The settings' node cells.
    let base = std::env::temp_dir().join(format!("zk-r20b4-nodes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    answers!(apply(&mut shell, Action::OpenHome { root: base.display().to_string() }), Applied::Homed { .. });
    for line in &lines {
        match apply(&mut shell, Action::SetEndpoints { specs: line.clone() }) {
            Applied::Trouble(f) => assert!(!seen(&f).contains(KEY), "settings say the key: {}", seen(&f)),
            other => panic!("{line}: refused: {other:?}"),
        }
    }
    drop(shell);
    let _ = std::fs::remove_dir_all(&base);
}
