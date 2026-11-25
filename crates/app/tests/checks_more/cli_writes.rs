use app::action::{apply, apply_settled, Action, Applied};
use app::machine::Lasts;

const PIN: &str = "27618394";

fn shell() -> app::shell::Shell {
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx))
}

fn credential() -> std::path::PathBuf {
    app::home::machine_dir().expect("the machine directory").join(app::places::credential_file())
}

fn codes(shell: &app::shell::Shell) -> Vec<String> {
    shell.faults.iter().map(|f| f.said().split([' ', ':']).next().unwrap_or_default().to_string()).collect()
}

#[test]
fn writing_alone_issues_an_owner_only_credential_of_this_vault_and_off_removes_it() {
    if super::alone_in(module_path!(), "writing_alone_issues_an_owner_only_credential_of_this_vault_and_off_removes_it") {
        return;
    }
    let mut shell = shell();
    let before = app::keybox::tries_left().expect("tries");
    match apply_settled(&mut shell, Action::SetCliAlone { lasts: Lasts::Secs(3_600), pin: PIN.into() }) {
        Applied::CliAloneSet(Some(Some(_))) => {}
        other => panic!("writing alone should be on: {other:?}"),
    }
    let p = credential();
    assert!(zikaron_os::is_owner_only(&p).expect("the credential is there"), "owner-only");
    let v = zikaron::json::parse(&std::fs::read(&p).expect("read")).expect("json");
    let zikaron::json::Value::Obj(m) = v else { panic!("an object") };
    assert_eq!(m.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>(), zikaron_seal::credential::member::ALL, "the closed table");
    assert!(matches!(app::keybox::credential_now(app::lastread::now_secs()), Ok(app::keybox::Credential::Live(st)) if st.rows.is_empty()), "in force, no key yet");
    assert_eq!(app::machine::read().expect("machine").cli_alone, Some(Lasts::Secs(3_600)), "the switch is sealed");
    let plain = std::fs::read_to_string(app::machine::path().expect("path")).expect("plain");
    assert!(!plain.contains("cli_"), "the plain file does not carry it: {plain}");
    assert_eq!(shell.machine.cli_path(), app::machine::CliPath::Alone);
    match apply(&mut shell, Action::SetCliDesktop { on: false }) {
        Applied::Trouble(f) => assert!(f.said().starts_with("CLI_ALONE_ON"), "{}", f.said()),
        other => panic!("the desktop switch should not flip: {other:?}"),
    }
    assert!(shell.machine.cli_desktop, "unchanged");
    match apply(&mut shell, Action::CliAloneOff) {
        Applied::CliAloneSet(None) => {}
        other => panic!("writing alone should be off: {other:?}"),
    }
    assert!(!p.exists(), "the credential is gone");
    assert_eq!(app::machine::read().expect("machine").cli_alone, None);
    match apply(&mut shell, Action::SetCliDesktop { on: false }) {
        Applied::CliDesktopSet(false) => {}
        other => panic!("now it flips: {other:?}"),
    }
    assert_eq!(shell.machine.cli_path(), app::machine::CliPath::Closed);
    assert_eq!(app::keybox::tries_left().expect("tries"), before, "the right passcode counts nothing");
}

#[test]
fn an_expired_credential_is_removed_and_writing_alone_goes_off_said_by_name() {
    if super::alone_in(module_path!(), "an_expired_credential_is_removed_and_writing_alone_goes_off_said_by_name") {
        return;
    }
    let mut shell = shell();
    assert!(matches!(apply_settled(&mut shell, Action::SetCliAlone { lasts: Lasts::Secs(60), pin: PIN.into() }), Applied::CliAloneSet(Some(_))));
    app::keybox::issue_credential(zikaron_seal::credential::Term { issued: 10, expires: Some(70) }).expect("issued");
    shell.credential = Some(zikaron_seal::credential::Term { issued: 10, expires: Some(70) });
    let _ = shell.drain_at(0.0);
    assert!(!credential().exists(), "removed");
    assert_eq!(app::machine::read().expect("machine").cli_alone, None, "the switch went off");
    assert!(codes(&shell).iter().any(|c| c == "CREDENTIAL_EXPIRED"), "said: {:?}", codes(&shell));
}

#[test]
fn a_record_not_in_its_shape_is_taken_back_said_once_and_counts_no_passcode_try() {
    if super::alone_in(module_path!(), "a_record_not_in_its_shape_is_taken_back_said_once_and_counts_no_passcode_try") {
        return;
    }
    let mut shell = shell();
    let before = app::keybox::tries_left().expect("tries");
    let earlier = format!("{{\"expires\":null,\"issued\":1,\"key\":\"0x{}\",\"master\":\"0x{}\"}}", "11".repeat(32), "22".repeat(72));
    for laid in [b"{}".to_vec(), earlier.into_bytes()] {
        assert!(matches!(apply_settled(&mut shell, Action::SetCliAlone { lasts: Lasts::Never, pin: PIN.into() }), Applied::CliAloneSet(Some(None))));
        std::fs::write(credential(), &laid).expect("laid");
        shell.faults.clear();
        shell.reread_credential();
        assert!(!credential().exists(), "removed");
        assert_eq!(app::machine::read().expect("machine").cli_alone, None, "the switch went off");
        assert_eq!(codes(&shell), vec!["CREDENTIAL_RECLAIMED".to_string()], "said once");
        assert_eq!(shell.cli_alone_now(), app::shell::CliAloneNow::Off);
    }
    assert_eq!(app::keybox::tries_left().expect("tries"), before, "a credential failure is no passcode try");
}

#[test]
fn a_new_master_key_takes_the_credential_back_as_it_takes_effect() {
    if super::alone_in(module_path!(), "a_new_master_key_takes_the_credential_back_as_it_takes_effect") {
        return;
    }
    let mut shell = shell();
    assert!(matches!(apply_settled(&mut shell, Action::SetCliAlone { lasts: Lasts::Never, pin: PIN.into() }), Applied::CliAloneSet(Some(None))));
    let key = match apply_settled(&mut shell, Action::CliPassAdd { note: "tool".into(), pin: PIN.into() }) {
        Applied::CliPassAdded { pass, .. } => pass.expose().to_string(),
        other => panic!("a key: {other:?}"),
    };
    assert!(!app::keybox::take_credential_dropped(), "nothing taken back yet");
    let nb = app::keybox::build_new(app::keybox::PinFor::New(PIN), None, &[]).expect("a new vault");
    app::keybox::stage_new(&nb).expect("staged");
    app::keybox::commit_new(nb).expect("taken effect");
    assert!(!credential().exists(), "removed as the new master key took effect");
    let dir = app::home::machine_dir().expect("dir");
    let opened = zikaron_seal::credential::open(&dir, app::places::key_account(), app::lastread::now_secs(), Some(&key)).map(|_| ()).map_err(|t| t.code);
    assert_eq!(opened, Err(zikaron_seal::Code::CredentialMissing), "the key opens nothing");
    assert!(app::keybox::take_credential_dropped(), "told once");
    assert!(!app::keybox::take_credential_dropped(), "once");
}

#[test]
fn a_program_key_is_made_past_the_passcode_shown_once_and_deleted_at_once() {
    if super::alone_in(module_path!(), "a_program_key_is_made_past_the_passcode_shown_once_and_deleted_at_once") {
        return;
    }
    let mut shell = shell();
    assert!(matches!(apply_settled(&mut shell, Action::SetCliAlone { lasts: Lasts::Never, pin: PIN.into() }), Applied::CliAloneSet(Some(None))));
    assert_eq!(shell.cli_alone_now(), app::shell::CliAloneNow::Keyless);
    let before = app::keybox::tries_left().expect("tries");
    match apply_settled(&mut shell, Action::CliPassAdd { note: "tool".into(), pin: "11111111".into() }) {
        Applied::Trouble(f) => assert!(f.said().starts_with("PIN_WRONG"), "{}", f.said()),
        other => panic!("a wrong passcode is refused: {other:?}"),
    }
    assert_eq!(app::keybox::tries_left().expect("tries"), before - 1, "counted as the gate counts");
    assert!(shell.passes.is_empty());
    let (a, row_a) = match apply_settled(&mut shell, Action::CliPassAdd { note: "工具 A".into(), pin: PIN.into() }) {
        Applied::CliPassAdded { pass, row } => {
            let shown = format!("{:?}", Applied::CliPassAdded { pass: pass.clone(), row: row.clone() });
            assert!(!shown.contains(&pass.expose()[4..]), "not in its debug form");
            (pass.expose().to_string(), row)
        }
        other => panic!("a program key: {other:?}"),
    };
    assert!(a.starts_with(zikaron_seal::credential::PASS_PREFIX) && a.len() == 4 + 64, "its text form");
    assert_eq!(row_a.note, "工具 A");
    let b = match apply_settled(&mut shell, Action::CliPassAdd { note: "tool b".into(), pin: PIN.into() }) {
        Applied::CliPassAdded { pass, .. } => pass.expose().to_string(),
        other => panic!("a second key: {other:?}"),
    };
    assert_eq!(shell.passes.iter().map(|r| r.note.as_str()).collect::<Vec<_>>(), ["工具 A", "tool b"]);
    assert_eq!(shell.cli_alone_now(), app::shell::CliAloneNow::Keyed(2));
    let disk = std::fs::read_to_string(credential()).expect("read").to_ascii_lowercase();
    assert!(!disk.contains(&a[4..]) && !disk.contains(&b[4..]) && !disk.contains(zikaron_seal::credential::PASS_PREFIX), "no key on disk");
    let dir = app::home::machine_dir().expect("dir");
    let account = app::places::key_account();
    let opens = |k: &str| zikaron_seal::credential::open(&dir, account, app::lastread::now_secs(), Some(k)).map(|_| ()).map_err(|t| t.code);
    assert_eq!(opens(&a), Ok(()));
    match apply(&mut shell, Action::CliPassDrop { tag: row_a.tag.clone() }) {
        Applied::CliPassDropped { tag } => assert_eq!(tag, row_a.tag),
        other => panic!("deleted: {other:?}"),
    }
    assert_eq!(opens(&a), Err(zikaron_seal::Code::CredentialPassUnknown), "deleted: opens nothing at once");
    assert_eq!(opens(&b), Ok(()), "the other still opens");
    assert_eq!(shell.passes.len(), 1);
    match apply(&mut shell, Action::CliPassDrop { tag: row_a.tag.clone() }) {
        Applied::Trouble(f) => assert!(f.said().starts_with("CREDENTIAL_ROW_GONE"), "{}", f.said()),
        other => panic!("deleted already: {other:?}"),
    }
    shell.reread_credential();
    assert_eq!(shell.passes.iter().map(|r| r.note.as_str()).collect::<Vec<_>>(), ["tool b"]);
}

#[test]
fn adding_a_program_key_off_with_a_bad_note_or_past_the_limit_is_refused_by_name() {
    if super::alone_in(module_path!(), "adding_a_program_key_off_with_a_bad_note_or_past_the_limit_is_refused_by_name") {
        return;
    }
    let mut shell = shell();
    let said = |a: Applied| match a {
        Applied::Trouble(f) => f.said().split([' ', ':']).next().unwrap_or_default().to_string(),
        other => format!("{other:?}"),
    };
    assert_eq!(said(apply_settled(&mut shell, Action::CliPassAdd { note: "a".into(), pin: PIN.into() })), "CREDENTIAL_MISSING");
    assert!(matches!(apply_settled(&mut shell, Action::SetCliAlone { lasts: Lasts::Never, pin: PIN.into() }), Applied::CliAloneSet(Some(None))));
    for note in ["", "a\nb", &"x".repeat(zikaron_seal::credential::NOTE_MAX + 1)] {
        assert_eq!(said(apply_settled(&mut shell, Action::CliPassAdd { note: note.into(), pin: PIN.into() })), "CREDENTIAL_NOTE", "{note:?}");
    }
    for i in 0..zikaron_seal::credential::PASSES_MAX {
        assert!(matches!(apply_settled(&mut shell, Action::CliPassAdd { note: format!("p{i}"), pin: PIN.into() }), Applied::CliPassAdded { .. }));
    }
    assert_eq!(said(apply_settled(&mut shell, Action::CliPassAdd { note: "one more".into(), pin: PIN.into() })), "CREDENTIAL_FULL");
    assert_eq!(shell.passes.len(), zikaron_seal::credential::PASSES_MAX);
}

#[test]
fn the_credential_record_holds_no_program_key_and_the_key_is_read_in_one_place() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap_or_else(|_| panic!("{p}"));
    let code_of = |t: &str| t.lines().filter(|l| !l.trim_start().starts_with("//")).collect::<Vec<_>>().join("\n");
    let src = code_of(&read("crates/zikaron-seal/src/credential.rs"));
    let src = src.split("#[cfg(test)]").next().unwrap_or_default().to_string();
    let body = |head: &str| {
        let at = src.find(head).unwrap_or_else(|| panic!("{head}"));
        let rest = &src[at..];
        rest[..rest.find("\n}\n").unwrap_or(rest.len())].to_string()
    };
    let bytes_of = body("fn bytes_of(");
    for w in ["Pass", "pass", "text()", "PASS_"] {
        assert!(!bytes_of.contains(w), "the record's one writer touches a key: {w}");
    }
    for m in ["member::EXPIRES", "member::ISSUED", "member::PASSES", "row::ADDED", "row::MASTER", "row::NOTE"] {
        assert!(bytes_of.contains(m), "the record spells {m}");
    }
    assert_eq!(src.matches("land::put_at(").count(), 1, "the record lands through one write");
    assert!(body("fn write(").contains("bytes_of(r)"), "and that write takes the one shape");
    assert!(!body("pub fn add(").contains(".text()"), "adding a key does not spell it");
    let mut readers = Vec::new();
    for dir in ["crates/app/src", "crates/zikaron-cli/src", "crates/zikaron-seal/src", "crates/zikaron-glue/src", "crates/zikaron-send/src", "crates/zikaron-anchor/src", "crates/zikaron-os/src"] {
        let mut stack = vec![root.join(dir)];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).expect("dir").flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "rs") {
                    let t = code_of(&std::fs::read_to_string(&p).expect("src"));
                    let t = t.split("#[cfg(test)]").next().unwrap_or_default();
                    if t.contains("PASS_ENV") && (t.contains("env::var") || t.contains("var_os")) {
                        readers.push(p.strip_prefix(&root).unwrap_or(&p).to_path_buf());
                    }
                }
            }
        }
    }
    assert_eq!(readers, vec![std::path::PathBuf::from("crates/zikaron-cli/src/sealed.rs")], "the key is read in one place");
    let sealed = code_of(&read("crates/zikaron-cli/src/sealed.rs"));
    assert_eq!(sealed.matches("credential::PASS_ENV").count(), 1, "once, in unlock");
    let unlock = &sealed[sealed.find("fn unlock(").expect("unlock")..];
    assert!(unlock[..unlock.find("\n}\n").unwrap_or(unlock.len())].contains("credential::PASS_ENV"), "in unlock");
}
