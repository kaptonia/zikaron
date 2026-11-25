use app::roles::Role;
use app::shell::Shell;

const CHILD: &str = "ZIKARON_GATES_SHOWN_CHILD";

fn alone(test: &str) -> bool {
    if std::env::var(CHILD).as_deref() == Ok(test) {
        return false;
    }
    let me = std::env::current_exe().expect("this test binary");
    let out = std::process::Command::new(me).args([test, "--exact", "--nocapture", "--test-threads=1"]).env(CHILD, test).output().expect("the child runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success() && said.contains("1 passed"), "{test}\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
    true
}

fn said(r: &Result<(), app::fault::Fault>) -> Option<(Option<app::fault::Known>, String)> {
    r.as_ref().err().map(|f| (f.which(), f.human().to_string()))
}

#[test]
fn the_gate_shown_follows_the_disk_once_something_lands() {
    if alone("the_gate_shown_follows_the_disk_once_something_lands") {
        return;
    }
    let machine = std::env::temp_dir().join(format!("zk-gates-shown-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&machine);
    std::fs::create_dir_all(&machine).expect("a machine folder");
    app::places::set(app::places::Places { key_account: app::places::ACCOUNT.to_string(), machine_dir: Some(machine.clone()), user_home: None });
    app::keybox::set_pin("27618394").expect("a vault");
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = Shell::boot(zikaron_ui::skin::dress(&ctx));
    let words = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    let got = app::action::apply_settled(&mut shell, app::action::Action::ImportIdentity { form: app::action::ImportForm::Words(words.to_string().into()), seat: Role::Author, label: String::new(), network: app::deploy::CUSTOM.into() });
    assert!(!matches!(got, app::action::Applied::Trouble(_)), "{got:?}");
    let root = shell.home.as_ref().map(|h| h.root().to_path_buf()).expect("a home open");
    let home = app::home::Home::bare(root);
    let marked = said(&shell.may_write_entries_shown());
    assert!(marked.is_some(), "the import's mark refuses");
    assert_eq!(marked, said(&shell.may_write_entries()), "the words a press is told");
    app::restorex::clear(&home).expect("the mark off");
    assert_eq!(said(&shell.may_write_entries_shown()), marked, "nothing landed: the answer judged before");
    let _ = app::action::apply_settled(&mut shell, app::action::Action::ReadLedger);
    assert_eq!(said(&shell.may_write_entries_shown()), None, "landed: the key lit");
    app::restorex::write(&home, &app::restorex::State::Unfetched).expect("marked again");
    assert_eq!(said(&shell.may_write_entries_shown()), None, "nothing landed: the answer judged before");
    let _ = app::action::apply_settled(&mut shell, app::action::Action::ReadLedger);
    let shown = said(&shell.may_write_entries_shown());
    assert!(shown.is_some(), "the mark refuses again: {shown:?}");
    assert_eq!(shown, said(&shell.may_write_entries()), "the words a press is told");
    let _ = std::fs::remove_dir_all(&machine);
}
