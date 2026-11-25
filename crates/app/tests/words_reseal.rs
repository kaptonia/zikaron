use app::roles::Role;
use app::shell::Shell;

const CHILD: &str = "ZIKARON_WORDS_RESEAL_CHILD";
const PIN: &str = "27618394";
const A: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const B: &str = "legal winner thank year wave sausage worth useful legal winner thank yellow";

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

fn machine_with_vault(tag: &str) -> (Shell, std::path::PathBuf) {
    let machine = std::env::temp_dir().join(format!("zk-reseal-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&machine);
    std::fs::create_dir_all(&machine).expect("a machine folder");
    app::places::set(app::places::Places { key_account: app::places::ACCOUNT.to_string(), machine_dir: Some(machine.clone()), user_home: None });
    app::keybox::set_pin(PIN).expect("a vault");
    let ctx = zikaron_ui::egui::Context::default();
    let shell = Shell::boot(zikaron_ui::skin::dress(&ctx));
    (shell, machine)
}

fn desk(tag: &str) -> (Shell, std::path::PathBuf, String, String) {
    let (mut shell, machine) = machine_with_vault(tag);
    let (id, words) = made(&mut shell);
    (shell, machine, id, words)
}

fn made(shell: &mut Shell) -> (String, String) {
    let before: Vec<String> = app::register::view().map(|r| r.rows.iter().map(|x| x.id.clone()).collect()).unwrap_or_default();
    assert!(matches!(app::action::apply_settled(shell, app::action::Action::NewIdentity), app::action::Applied::FreshWords));
    let (list, picks) = shell.new_words.as_ref().map(|f| (f.words(), f.picks)).expect("fresh words");
    let words = list.iter().map(|w| w.expose()).collect::<Vec<_>>().join(" ");
    let answers: Vec<(usize, app::secret::Secret)> = picks.iter().map(|i| (*i, list[*i].clone().into())).collect();
    let got = app::action::apply_settled(shell, app::action::Action::ConfirmIdentity { answers, label: String::new(), network: app::deploy::CUSTOM.into() });
    assert!(!matches!(got, app::action::Applied::Trouble(_)), "{got:?}");
    let reg = app::register::view().expect("the register");
    let row = reg.rows.iter().find(|x| !before.contains(&x.id)).cloned().expect("a row added");
    assert!(row.seats().into_iter().all(|s| app::identity::seat_ledger(&row, s) == app::identity::SeatLedger::None), "a new identity's seats hold no ledger");
    (row.id, words)
}

fn add(shell: &mut Shell, words: &str) -> String {
    let before: Vec<String> = app::register::view().map(|r| r.rows.iter().map(|x| x.id.clone()).collect()).unwrap_or_default();
    let got = app::action::apply_settled(shell, app::action::Action::ImportIdentity { form: app::action::ImportForm::Words(words.to_string().into()), seat: Role::Author, label: String::new(), network: app::deploy::CUSTOM.into() });
    assert!(!matches!(got, app::action::Applied::Trouble(_)), "{got:?}");
    app::register::view().expect("the register").rows.iter().map(|x| x.id.clone()).find(|id| !before.contains(id)).expect("a row added")
}

fn entropy(words: &str) -> Vec<u8> {
    app::cryptx::entropy_of(words).expect("the words").to_vec()
}

fn primary() -> Option<String> {
    app::keybox::primary().expect("the vault reads").map(|(p, _)| p)
}

fn seed(id: &str) -> Option<Vec<u8>> {
    app::keybox::get(&app::places::seed_slot(id)).expect("the vault reads")
}

fn pending(id: &str) -> bool {
    app::keybox::present(&app::places::seed_next_slot(id)).expect("the vault reads")
}

fn owed(id: &str) -> bool {
    app::keybox::present(&app::places::seed_reseal_slot(id)).expect("the vault reads")
}

fn opens_with(words: &str) -> bool {
    app::keybox::lock();
    let ok = app::keybox::recover(&entropy(words), PIN).is_ok();
    if !ok {
        app::keybox::unlock(PIN).expect("open again");
    }
    ok
}

fn pend(id: &str, words: &str) {
    app::keybox::put(&app::places::seed_next_slot(id), &entropy(words)).expect("pending");
}

fn settle() -> Option<app::identity::Registry> {
    app::action::words_settle(&app::register::view().expect("the register")).expect("settled")
}

#[test]
fn a_reseal_stopped_after_the_old_seal_went_writes_the_new_one() {
    if alone("a_reseal_stopped_after_the_old_seal_went_writes_the_new_one") {
        return;
    }
    let (_shell, machine, id, a) = desk("dropped");
    assert_eq!(primary().as_deref(), Some(id.as_str()), "the first identity is the primary");
    pend(&id, B);
    app::keybox::put(&app::places::seed_reseal_slot(&id), &[1]).expect("the record");
    app::keybox::drop_item(&app::places::seed_slot(&id)).expect("the old seed");
    app::keybox::put(&app::places::seed_slot(&id), &entropy(B)).expect("the new seed");
    app::keybox::drop_recovery(&id).expect("the old seal");
    assert_eq!(primary(), None, "the state the death left: no primary");
    assert!(settle().is_some(), "settled");
    assert_eq!(primary().as_deref(), Some(id.as_str()), "the primary again");
    assert_eq!(seed(&id), Some(entropy(B)));
    assert!(!pending(&id) && !owed(&id), "the pending words and the record went");
    assert!(opens_with(B), "the new words open the vault");
    assert!(!opens_with(&a), "the old ones no more");
    let _ = std::fs::remove_dir_all(&machine);
}

#[test]
fn a_seal_not_written_keeps_the_pending_words_and_runs_again() {
    if alone("a_seal_not_written_keeps_the_pending_words_and_runs_again") {
        return;
    }
    let (_shell, machine, id, a) = desk("refused");
    pend(&id, B);
    zikaron_os::pretend_sync_fails_at(Some(machine.clone()));
    let got = app::action::words_settle(&app::register::view().expect("the register"));
    zikaron_os::pretend_sync_fails_at(None);
    assert!(got.is_err(), "the refusal answered: {got:?}");
    assert!(pending(&id), "the pending words stay");
    assert_eq!(primary().as_deref(), Some(id.as_str()));
    assert!(opens_with(&a), "the old seal stands");
    assert!(settle().is_some(), "the next run settles");
    assert!(!pending(&id) && !owed(&id));
    assert!(opens_with(B));
    let _ = std::fs::remove_dir_all(&machine);
}

#[test]
fn a_change_left_only_the_seed_to_take_settles_once_the_vault_opens() {
    if alone("a_change_left_only_the_seed_to_take_settles_once_the_vault_opens") {
        return;
    }
    let (mut shell, machine, id, _) = desk("unlock");
    pend(&id, B);
    app::keybox::lock();
    app::keybox::unlock(PIN).expect("open");
    shell.after_unlock();
    assert_eq!(seed(&id), Some(entropy(B)));
    assert!(!pending(&id) && !owed(&id));
    assert_eq!(primary().as_deref(), Some(id.as_str()));
    assert!(opens_with(B));
    let _ = std::fs::remove_dir_all(&machine);
}

#[test]
fn a_secondary_identity_changing_its_words_takes_no_seal() {
    if alone("a_secondary_identity_changing_its_words_takes_no_seal") {
        return;
    }
    let (mut shell, machine, first, a) = desk("secondary");
    let (other, _) = made(&mut shell);
    assert_eq!(primary().as_deref(), Some(first.as_str()));
    pend(&other, B);
    assert!(settle().is_some(), "settled");
    assert_eq!(seed(&other), Some(entropy(B)));
    assert!(!pending(&other) && !owed(&other));
    assert_eq!(primary().as_deref(), Some(first.as_str()), "the primary stays");
    assert!(opens_with(&a), "its seal stands");
    let _ = std::fs::remove_dir_all(&machine);
}

#[test]
fn a_seat_not_yet_checked_against_the_chain_settles_nothing_and_keeps_the_pending_words() {
    if alone("a_seat_not_yet_checked_against_the_chain_settles_nothing_and_keeps_the_pending_words") {
        return;
    }
    let (mut shell, machine) = machine_with_vault("unchecked");
    let id = add(&mut shell, A);
    let row = app::register::view().expect("the register").rows.into_iter().find(|r| r.id == id).expect("the row");
    assert!(row.seats().into_iter().all(|s| app::identity::seat_ledger(&row, s) == app::identity::SeatLedger::Unchecked), "imported words leave each seat to be checked against the chain");
    pend(&id, B);
    assert!(settle().is_none(), "nothing settles");
    assert!(pending(&id), "the pending words stay");
    assert_eq!(seed(&id), Some(entropy(A)), "the seed stays");
    assert_eq!(primary().as_deref(), Some(id.as_str()));
    assert!(opens_with(A), "the old seal stands");
    let _ = std::fs::remove_dir_all(&machine);
}
