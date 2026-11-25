use app::roles::Role;

#[test]
fn an_existing_key_under_a_root_is_that_roots_identity() {
    let machine = std::env::temp_dir().join(format!("zk-app-rooted-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&machine);
    std::fs::create_dir_all(&machine).expect("a machine folder");
    app::places::set(app::places::Places { key_account: app::places::ACCOUNT.to_string(), machine_dir: Some(machine.clone()), user_home: None });
    app::keybox::set_pin("27618394").expect("a vault");
    let (key, root) = (app::key::generate().expect("a key"), app::key::generate().expect("a root").address().expect("an address"));
    let at = key.address().expect("an address");
    let row = app::register::change(|reg| app::identity::add_existing_rooted(reg, &key, Role::Grantee, true, "", Some(root))).expect("an identity");
    assert_eq!(row.id, root.hex());
    assert_eq!(row.address(Role::Grantee), Some(root));
    assert_eq!(row.current(Role::Grantee), Some(at));
    assert_eq!(row.account(Role::Grantee), Some(app::places::key_slot(&at)));
    assert!(app::keybox::present(&app::places::key_slot(&at)).expect("the vault"));
    assert_eq!(row.current(Role::Author), None, "the other seat stays empty");
    let plain = app::key::generate().expect("a key");
    let own = app::register::change(|reg| app::identity::add_existing_rooted(reg, &plain, Role::Author, false, "", None)).expect("an identity");
    assert_eq!(own.address(Role::Author), plain.address());
    assert_eq!(own.current(Role::Author), plain.address());
    assert!(own.office(Role::Author).is_some_and(|o| o.generation == 0));
    let _ = std::fs::remove_dir_all(&machine);
}
