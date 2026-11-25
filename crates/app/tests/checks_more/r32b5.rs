use super::a8_send::{bench, chain, node, tidy, PLENTY};
use app::action::{apply, Action, Applied};
use app::fault::Fault;
use app::roles::Role;
use app::task::{Done, Kind};
use std::time::{Duration, Instant};
use zikaron::json::Value;

fn audit_lands(shell: &mut app::shell::Shell, result: Result<Done, Fault>, now: f64) {
    let _ = shell.tasks.spawn(Kind::Audit, move || result);
    let until = Instant::now() + Duration::from_secs(60);
    while !shell.drain_at(now).iter().any(|o| o.kind == Kind::Audit) {
        assert!(Instant::now() < until, "the audit landed");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn report(unanchored: &[String]) -> Done {
    let rows = |ids: &[String]| Value::Arr(ids.iter().map(|i| Value::Str(i.clone())).collect());
    let report = Value::Obj(vec![
        ("anchored".into(), Value::Arr(Vec::new())),
        ("missing".into(), Value::Arr(Vec::new())),
        (zikaron::v2::tokens::Key::Unanchored.as_str().into(), rows(unanchored)),
    ]);
    let label = zikaron::v2::tokens::Label::Complete.as_str().to_string();
    Done::Audited { label, complete: true, broken: false, entries: 2, report, unanswered: Vec::new(), unproven: Vec::new(), asked: 1, single_source: false, fragment: Value::Null, recounted: false }
}

#[test]
fn a_report_behind_the_receipts_is_asked_again_within_its_bound_then_said() {
    if super::alone_in(module_path!(), "a_report_behind_the_receipts_is_asked_again_within_its_bound_then_said") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let mut shell = bench("r32b5-k12", &[&n]);
    let id = shell.next_batch_here(1).first().cloned().expect("a queued entry");
    let (pause, bound) = (app::shell::AUDIT_LAG_AGAIN, app::shell::AUDIT_LAG_BOUND);
    audit_lands(&mut shell, Ok(report(std::slice::from_ref(&id))), 10.0);
    shell.queue.blocks.push(app::queue::Block { id: id.clone(), tx: format!("0x{}", "ab".repeat(32)), chain: 31337, block: 0x30 });
    let first = shell.audit_lag_at().expect("asked again once there is a receipt");
    let mut tries = 0usize;
    let mut last = first;
    while let Some(at) = shell.audit_lag_at() {
        assert!(at <= first + bound, "never asked past the bound: {at}");
        audit_lands(&mut shell, Ok(report(std::slice::from_ref(&id))), at);
        last = at;
        tries += 1;
        assert!(tries < 1000, "bounded");
    }
    assert_eq!(tries, (bound / pause) as usize + 1, "asked every pause until the bound");
    assert!(last + pause > first + bound, "stopped at the bound");
    assert_eq!(shell.audit_lag_stopped(), Some(vec![id.to_ascii_lowercase()]), "the entries said");
    assert!(!shell.audit_lags(last + 10.0 * pause), "no more by itself");
    audit_lands(&mut shell, Ok(report(std::slice::from_ref(&id))), last + 100.0);
    assert!(shell.audit_lag_stopped().is_some());
    audit_lands(&mut shell, Ok(report(&[])), last + 200.0);
    assert_eq!((shell.audit_lag_stopped(), shell.audit_lag_at()), (None, None));
    tidy();
}

fn seat_now(shell: &app::shell::Shell) -> (Option<Role>, Role) {
    (app::register::read().ok().flatten().and_then(|r| r.current.map(|(_, s)| s)), shell.settings.role)
}

fn settled(shell: &mut app::shell::Shell, k: Kind) {
    let until = Instant::now() + Duration::from_secs(60);
    while shell.tasks.in_flight(k) && Instant::now() < until {
        shell.drain();
        std::thread::sleep(Duration::from_millis(10));
    }
    shell.drain();
}

#[test]
fn a_first_identity_made_on_the_grantee_seat_is_current_there() {
    if super::alone_in(module_path!(), "a_first_identity_made_on_the_grantee_seat_is_current_there") {
        return;
    }
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    shell.settings.role = Role::Grantee;
    assert!(app::register::read().ok().flatten().is_none_or(|r| r.current.is_none()), "the register names no current seat");
    let form = app::action::ImportForm::Words("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about".to_string().into());
    let started = apply(&mut shell, Action::ImportIdentity { form, seat: Role::Grantee, label: "g".into(), network: app::deploy::CUSTOM.into() });
    assert!(!matches!(started, Applied::Trouble(_)), "{started:?}");
    settled(&mut shell, Kind::Vault);
    assert_eq!(seat_now(&shell), (Some(Role::Grantee), Role::Grantee), "one seat in the register and the shell");
}

fn seat_home() -> Option<std::path::PathBuf> {
    app::register::now_row_listed().ok().flatten().and_then(|(row, seat)| row.home(seat))
}

#[test]
fn a_change_of_data_folder_moves_the_seats_home_and_one_that_does_not_open_moves_nothing() {
    if super::alone_in(module_path!(), "a_change_of_data_folder_moves_the_seats_home_and_one_that_does_not_open_moves_nothing") {
        return;
    }
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let form = app::action::ImportForm::Words("legal winner thank year wave sausage worth useful legal winner thank yellow".to_string().into());
    let started = apply(&mut shell, Action::ImportIdentity { form, seat: Role::Author, label: "a".into(), network: app::deploy::CUSTOM.into() });
    assert!(!matches!(started, Applied::Trouble(_)), "{started:?}");
    settled(&mut shell, Kind::Vault);
    let old = seat_home().expect("the seat's home");
    let picked_before = app::home::where_is().ok();
    let base = std::env::temp_dir().join(format!("zk-r32b5-k4-{}", std::process::id()));
    let broken = base.join("broken");
    let h = app::home::Home::open_or_create(&broken.display().to_string()).expect("a home laid");
    std::fs::write(h.dir(app::home::Slot::Settings).join(app::settings::FILE), b"not settings").expect("settings spoiled");
    let refused = apply(&mut shell, Action::ChangeHome { root: broken.display().to_string() });
    assert!(matches!(refused, Applied::Trouble(_)), "{refused:?}");
    assert_eq!(seat_home().as_deref(), Some(old.as_path()), "the register names the home left");
    assert_eq!(app::home::where_is().ok(), picked_before, "the choice recorded is the one before");
    let fresh = base.join("fresh");
    std::fs::create_dir_all(&fresh).expect("an empty folder");
    let moved = apply(&mut shell, Action::ChangeHome { root: fresh.display().to_string() });
    assert!(matches!(moved, Applied::Homed { .. }), "{moved:?}");
    assert!(seat_home().is_some_and(|h| app::home::same_place(&h, &fresh)), "the register follows the home: {:?}", seat_home());
    assert!(app::home::where_is().is_ok_and(|p| app::home::same_place(&p, &fresh)), "and so does the choice");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_change_of_data_folder_that_does_not_open_moves_no_other_seat_back() {
    if super::alone_in(module_path!(), "a_change_of_data_folder_that_does_not_open_moves_no_other_seat_back") {
        return;
    }
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let form = app::action::ImportForm::Words("letter advice cage absurd amount doctor acoustic avoid letter advice cage above".to_string().into());
    let started = apply(&mut shell, Action::ImportIdentity { form, seat: Role::Author, label: "a".into(), network: app::deploy::CUSTOM.into() });
    assert!(!matches!(started, Applied::Trouble(_)), "{started:?}");
    settled(&mut shell, Kind::Vault);
    let author = seat_home().expect("the author seat's home");
    let base = std::env::temp_dir().join(format!("zk-r32b5-other-{}", std::process::id()));
    let other = base.join("grantee");
    let h = app::home::Home::open_or_create(&other.display().to_string()).expect("a home laid");
    std::fs::write(h.dir(app::home::Slot::Settings).join(app::settings::FILE), b"not settings").expect("settings spoiled");
    let other_s = other.display().to_string();
    app::register::change_listed(|reg| {
        for r in reg.rows.iter_mut() {
            r.grantee_home = other_s.clone();
        }
        Ok(())
    })
    .expect("the register written");
    let before: Vec<(String, String)> = app::register::read().ok().flatten().expect("a register").rows.iter().map(|r| (r.author_home.clone(), r.grantee_home.clone())).collect();
    let refused = apply(&mut shell, Action::ChangeHome { root: other_s.clone() });
    assert!(matches!(refused, Applied::Trouble(_)), "{refused:?}");
    let after: Vec<(String, String)> = app::register::read().ok().flatten().expect("a register").rows.iter().map(|r| (r.author_home.clone(), r.grantee_home.clone())).collect();
    assert_eq!(after, before, "the register as it was: the author seat back, the grantee seat untouched");
    assert_eq!(seat_home().as_deref(), Some(author.as_path()));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_seat_whose_home_does_not_open_is_not_entered_in_the_register_either() {
    if super::alone_in(module_path!(), "a_seat_whose_home_does_not_open_is_not_entered_in_the_register_either") {
        return;
    }
    super::vault_open();
    let ctx = zikaron_ui::egui::Context::default();
    let mut shell = app::shell::Shell::boot(zikaron_ui::skin::dress(&ctx));
    let import = |shell: &mut app::shell::Shell, words: &str, label: &str| {
        let form = app::action::ImportForm::Words(words.to_string().into());
        let a = apply(shell, Action::ImportIdentity { form, seat: Role::Author, label: label.into(), network: app::deploy::CUSTOM.into() });
        settled(shell, Kind::Vault);
        a
    };
    let first = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
    let second = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
    assert!(!matches!(import(&mut shell, first, "first"), Applied::Trouble(_)));
    let current = || app::register::read().ok().flatten().and_then(|r| r.current.map(|(id, _)| id));
    let a_id = current().expect("the first is current");
    assert!(!matches!(import(&mut shell, second, "second"), Applied::Trouble(_)));
    let b_id = current().expect("the second is current");
    let b_home = seat_home().expect("the second's home");
    let deleted = apply(&mut shell, Action::DeleteIdentity { id: b_id.clone(), pin: app::secret::Secret::from(String::from("27618394")) });
    assert!(!matches!(deleted, Applied::Trouble(_)), "{deleted:?}");
    settled(&mut shell, Kind::Vault);
    settled(&mut shell, Kind::Seat);
    assert_eq!(current().as_deref(), Some(a_id.as_str()));
    let h = app::home::Home::open_or_create(&b_home.display().to_string()).expect("the home there");
    std::fs::write(h.dir(app::home::Slot::Settings).join(app::settings::FILE), b"not settings").expect("settings spoiled");
    let _ = import(&mut shell, second, "second");
    assert_eq!(current().as_deref(), Some(a_id.as_str()), "the register's current seat is the one the shell stands in");
    assert!(seat_home().is_some_and(|h| !app::home::same_place(&h, &b_home)), "the register's current seat home is not the one that did not open");
    assert!(shell.home.as_ref().is_some_and(|h| !app::home::same_place(h.root(), &b_home)), "the shell did not move");
}

#[test]
fn passes_not_drawn_are_not_counted_as_drawn_and_a_language_set_in_a_turn_still_fades() {
    if super::alone_in(module_path!(), "passes_not_drawn_are_not_counted_as_drawn_and_a_language_set_in_a_turn_still_fades") {
        return;
    }
    let n = node(|m, b| chain(m, b, PLENTY));
    let shell = bench("r32b5-k7", &[&n]);
    let ctx = zikaron_ui::egui::Context::default();
    let _ = zikaron_ui::skin::dress(&ctx);
    let mut w = app::window::UnseenProbe::new(shell);
    for _ in 0..3 {
        w.drawn(&ctx, 0.1);
    }
    let before = zikaron_ui::motion::drawn(&ctx);
    for _ in 0..5 {
        w.pass(&ctx, 0.1);
    }
    assert_eq!(zikaron_ui::motion::drawn(&ctx), before, "five passes run hidden: none drawn");
    w.drawn(&ctx, 0.1);
    assert_eq!(zikaron_ui::motion::drawn(&ctx), before + 1, "the next pass drawn is one more");
    let was = app::lang::lang();
    app::lang::set(if was == app::lang::Lang::Zh { app::lang::Lang::En } else { app::lang::Lang::Zh });
    w.drawn(&ctx, 0.01);
    assert!(zikaron_ui::flow::morphing(&ctx), "the words cross-fade");
    app::lang::set(was);
    tidy();
}
