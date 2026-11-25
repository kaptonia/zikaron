use app::action::{Applied, Followed};
use app::roles::Role;
use app::shell::Shell;

const CHILD: &str = "ZIKARON_KEY_OFFICE_CHILD";
const A: &str = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
const B: &str = "legal winner thank year wave sausage worth useful legal winner thank yellow";
const C: &str = "letter advice cage absurd amount doctor acoustic avoid letter advice cage above";

fn words_key(words: &str, seat: Role, group: u32) -> app::key::Secret {
    let e = app::cryptx::entropy_of(words).expect("the words");
    app::key::Secret::take(app::cryptx::derive(&e, &app::family::path_at(seat, group)).expect("derived")).expect("a key")
}

fn alone(test: &str) -> bool {
    let full = test.to_string();
    if std::env::var(CHILD).as_deref() == Ok(full.as_str()) {
        return false;
    }
    let me = std::env::current_exe().expect("this test binary");
    let out = std::process::Command::new(me).args([full.as_str(), "--exact", "--nocapture", "--test-threads=1"]).env(CHILD, &full).output().expect("the child runs");
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success() && said.contains("1 passed"), "{full}\n{said}\n{}", String::from_utf8_lossy(&out.stderr));
    true
}

fn desk(tag: &str, words: &str) -> (Shell, String) {
    let machine = std::env::temp_dir().join(format!("zk-office-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&machine);
    std::fs::create_dir_all(&machine).expect("a machine folder");
    app::places::set(app::places::Places { key_account: app::places::ACCOUNT.to_string(), machine_dir: Some(machine), user_home: None });
    app::keybox::set_pin("27618394").expect("a vault");
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = Shell::boot(zikaron_ui::skin::dress(&ctx));
    let id = add(&mut shell, words);
    (shell, id)
}

fn add(shell: &mut Shell, words: &str) -> String {
    let before: Vec<String> = app::register::view().map(|r| r.rows.iter().map(|x| x.id.clone()).collect()).unwrap_or_default();
    let got = app::action::apply_settled(shell, app::action::Action::ImportIdentity { form: app::action::ImportForm::Words(words.to_string().into()), seat: Role::Author, label: String::new(), network: app::deploy::CUSTOM.into() });
    assert!(!matches!(got, Applied::Trouble(_)), "{got:?}");
    app::register::view().expect("the register").rows.iter().map(|x| x.id.clone()).find(|id| !before.contains(id)).expect("a row added")
}

fn standing(shell: &Shell) -> (Option<(String, Role)>, Option<(String, Role)>) {
    let listed = app::register::view().ok().as_ref().and_then(app::identity::now_row).map(|(r, s)| (r.id, s));
    let held = shell.identities.as_ref().and_then(app::identity::now_row).map(|(r, s)| (r.id, s));
    (listed, held)
}

fn rows() -> usize {
    app::register::view().map(|r| r.rows.len()).unwrap_or(0)
}

fn made_next(id: &str, seat: Role, k: &app::key::Secret) -> app::key::Address {
    let at = k.address().expect("an address");
    app::key::install(Some(&app::places::key_slot(&at)), k).expect("in the vault");
    app::register::change(|reg| app::identity::set_next(reg, id, seat, Some(at)).map(|_| ())).expect("named next");
    at
}

#[test]
fn a_key_change_written_settles_the_open_desk_on_the_new_key_in_the_same_row() {
    if alone("a_key_change_written_settles_the_open_desk_on_the_new_key_in_the_same_row") {
        let code = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/action/rotate.rs")).expect("rotate.rs");
        let write = code.split("RotateStep::Write =>").nth(1).and_then(|b| b.split("RotateStep::Enroll").next()).expect("the write arm");
        let follow = code.split("pub fn follow_office(").nth(1).and_then(|b| b.split("\n}\n").next()).expect("follow_office");
        for (name, body) in [("the write arm", write), ("follow_office", follow)] {
            assert!(body.contains("office::settle("), "{name} settles through the one place");
            assert!(!body.contains("shell.anchor ="), "{name} sets no key of the desk itself");
        }
        return;
    }
    let (mut shell, id) = desk("write", A);
    let before = (standing(&shell), rows());
    let k = app::key::generate().expect("a key");
    let at = made_next(&id, Role::Author, &k);
    app::action::settle(&mut shell, || {
        app::register::change(|reg| {
            app::identity::take_office(reg, &id, Role::Author)?;
            Ok(reg.clone())
        })
    })
    .expect("settled");
    assert_eq!((standing(&shell), rows()), before, "the same row and seat, no row added");
    assert_eq!(shell.anchor, Some(at), "the desk signs with the new key");
    assert_eq!(shell.anchor_public, k.public(), "the identity card's public key is the new key's");
    assert_eq!(app::register::view().expect("the register").find(&id).and_then(|r| r.current(Role::Author)), Some(at));
}

#[test]
fn the_key_in_office_followed_from_the_ledger_settles_the_open_desk() {
    if alone("the_key_in_office_followed_from_the_ledger_settles_the_open_desk") {
        return;
    }
    let (mut shell, _) = desk("follow", A);
    let before = (standing(&shell), rows());
    let k = words_key(A, Role::Author, 1);
    let at = k.address().expect("an address");
    assert!(app::action::follow_office(&mut shell, Some(&(at.hex(), 1, true))), "followed");
    assert_eq!((standing(&shell), rows()), before, "the same row and seat, no row added");
    assert_eq!((shell.anchor, shell.anchor_public), (Some(at), k.public()), "the desk on the key the ledger names");
}

#[test]
fn no_key_change_of_any_source_adds_a_row_or_switches_the_identity() {
    if alone("no_key_change_of_any_source_adds_a_row_or_switches_the_identity") {
        return;
    }
    let (mut shell, id) = desk("sources", A);
    let before = (standing(&shell), rows());
    for (source, k) in [
        ("same words", words_key(A, Role::Author, 1)),
        ("new words", words_key(C, Role::Author, 0)),
        ("drawn", app::key::generate().expect("drawn")),
    ] {
        let at = made_next(&id, Role::Author, &k);
        app::action::settle(&mut shell, || {
            app::register::change(|reg| {
                app::identity::take_office(reg, &id, Role::Author)?;
                Ok(reg.clone())
            })
        })
        .expect("settled");
        assert_eq!((standing(&shell), rows()), before, "{source}: no row added, the identity kept");
        assert_eq!((shell.anchor, shell.anchor_public), (Some(at), k.public()), "{source}: the desk on the new key");
    }
}

#[test]
fn an_identity_added_is_followed_whole_and_switched_back_by_hand() {
    if alone("an_identity_added_is_followed_whole_and_switched_back_by_hand") {
        return;
    }
    let (mut shell, first) = desk("add", A);
    let first_key = shell.anchor;
    let added = add(&mut shell, B);
    let on = Some((added.clone(), Role::Author));
    assert_eq!(standing(&shell), (on.clone(), on), "followed whole: the register and the desk on the new identity");
    assert_eq!(shell.anchor, app::identity::from_words(B).expect("the words").address(Role::Author), "signing with its key");
    let back = app::action::apply_settled(&mut shell, app::action::Action::SwitchIdentity { id: first.clone() });
    assert!(matches!(back, Applied::IdentitySwitched { .. }), "{back:?}");
    let on = Some((first, Role::Author));
    assert_eq!(standing(&shell), (on.clone(), on), "switched back by hand: both on the first");
    assert_eq!(shell.anchor, first_key, "its key again");
}

#[test]
fn a_register_moved_outside_the_window_is_followed_and_not_put_back() {
    if alone("a_register_moved_outside_the_window_is_followed_and_not_put_back") {
        return;
    }
    let (mut shell, id) = desk("outside", A);
    let i = id.clone();
    std::thread::spawn(move || app::register::change(|reg| app::identity::switch(reg, &i, Role::Grantee).map(|_| ()))).join().expect("the other thread").expect("switched");
    assert_eq!(app::action::follow_register(&mut shell), Followed::Moved);
    let on = Some((id.clone(), Role::Grantee));
    assert_eq!(standing(&shell), (on.clone(), on.clone()), "the desk on the seat the writer chose, the register left there");
    assert_eq!(shell.settings.role, Role::Grantee);
    let k = app::key::generate().expect("a key");
    let at = k.address().expect("an address");
    app::key::install(Some(&app::places::key_slot(&at)), &k).expect("in the vault");
    let i = id.clone();
    std::thread::spawn(move || app::register::change(|reg| app::identity::put_office(reg, &i, Role::Grantee, 1, at).map(|_| ()))).join().expect("the other thread").expect("put in office");
    assert_eq!(app::action::follow_register(&mut shell), Followed::Office);
    assert_eq!((shell.anchor, shell.anchor_public), (Some(at), k.public()), "the desk on the key the writer named");
    assert_eq!(standing(&shell), (on.clone(), on), "still where the writer put it");
    assert_eq!(app::action::follow_register(&mut shell), Followed::Same, "nothing more to follow");
}

#[test]
fn deleting_the_identity_in_use_when_the_next_ones_home_does_not_open_stands_the_desk_with_the_register() {
    if alone("deleting_the_identity_in_use_when_the_next_ones_home_does_not_open_stands_the_desk_with_the_register") {
        return;
    }
    let (mut shell, next) = desk("delete", B);
    let gone = add(&mut shell, A);
    let home = app::register::view().expect("the register").find(&next).and_then(|r| r.home(Role::Author)).expect("its home");
    let _ = std::fs::remove_dir_all(&home);
    std::fs::write(&home, b"no folder").expect("a file in its place");
    let said = shell.faults.len();
    let got = app::action::apply_settled(&mut shell, app::action::Action::DeleteIdentity { id: gone.clone(), pin: "27618394".to_string().into() });
    assert!(!matches!(got, Applied::Trouble(_)), "the delete itself is done: {got:?}");
    assert!(app::register::view().expect("the register").find(&gone).is_none(), "deleted");
    let on = Some((next, Role::Author));
    assert_eq!(standing(&shell), (on.clone(), on), "the desk and the register on one row and seat");
    assert!(shell.home.is_none(), "no home open: the deleted identity's closed, the next one's not opened");
    assert!(shell.faults.len() > said, "the home that did not open is said");
}
