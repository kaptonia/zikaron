//! The primary identity's recovery seals: one for every secret that derives its signing keys. A recovery-word
//! primary recovers with its words or with either seat's key (what its key files hold); an existing-key primary
//! with its key. Another identity's key is refused as not the primary's, an unknown one as matching nothing,
//! and neither opens the vault. A vault whose seals are not exactly its primary's kind's set, in order, is
//! refused by name. Each test runs alone in its own process ([`super::alone_in`]).

use app::fault::{Fault, Known};
use app::keybox::{member as m, PrimaryKind, State};
use app::roles::Role;
use zikaron::json::Value;

const NEW_PIN: &str = "48261937";

fn known<T>(r: Result<T, Fault>) -> Option<Known> {
    r.err().and_then(|f| f.which())
}

/// A seat's key as its key file holds it (the vault open).
fn key_of(row: &app::identity::Row, seat: Role) -> Vec<u8> {
    let s = app::key::load(row.account(seat).as_deref()).expect("the vault reads").expect("the seat's key");
    let hex = app::key::reveal_once(&mut Some(s)).expect("the key");
    zikaron::hexfmt::decode(&hex).expect("hex")
}

fn vault() -> Value {
    zikaron::json::parse(&std::fs::read(app::keybox::path().expect("the vault's place")).expect("the vault")).expect("the vault reads as JSON")
}

fn put_vault(v: &Value) {
    std::fs::write(app::keybox::path().expect("the vault's place"), zikaron::json::canon_bytes(v)).expect("the vault written");
}

/// The recovery seals' labels as the file holds them.
fn labels(v: &Value) -> Vec<String> {
    match v.member(m::RECOVERY) {
        Some(Value::Arr(a)) => a
            .iter()
            .map(|s| match s.member(m::LABEL) {
                Some(Value::Str(l)) => l.clone(),
                _ => String::new(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn with_recovery(v: &Value, f: impl FnOnce(&mut Vec<Value>)) -> Value {
    let Value::Obj(mut members) = v.clone() else { panic!("the vault is an object") };
    let Some(Value::Arr(a)) = members.iter_mut().find(|(k, _)| k == m::RECOVERY).map(|(_, x)| x) else { panic!("the seals are a list") };
    f(a);
    Value::Obj(members)
}

#[test]
fn a_recovery_word_primary_recovers_with_its_words_or_either_seat_key() {
    if super::alone_in(module_path!(), "a_recovery_word_primary_recovers_with_its_words_or_either_seat_key") {
        return;
    }
    super::vault_open();
    let words = app::identity::fresh().expect("words");
    let row = app::register::change(|reg| app::identity::add_words(reg, &words, false, "")).expect("the primary");
    let other = app::identity::fresh().expect("words");
    let second = app::register::change(|reg| app::identity::add_words(reg, &other, false, "")).expect("a secondary");
    assert_eq!(app::keybox::primary().ok().flatten(), Some((row.id.clone(), PrimaryKind::Words)));
    assert_eq!(labels(&vault()), ["primary", "primary-author", "primary-grantee"], "one seal per secret");
    let view = app::register::view().expect("the register");
    let phrase = app::identity::words_of(&view, &row.id).expect("the words").iter().map(|w| w.expose().to_string()).collect::<Vec<_>>().join(" ");
    let seats: Vec<(Role, Vec<u8>)> = Role::ALL.into_iter().map(|r| (r, key_of(&row, r))).collect();
    let theirs = key_of(&second, Role::Author);
    let mut pin = NEW_PIN.to_string();
    for (seat, k) in &seats {
        app::keybox::lock();
        app::keybox::recover(k, &pin).unwrap_or_else(|f| panic!("{seat:?}'s key: {f:?}"));
        assert_eq!(app::keybox::state().ok(), Some(State::Open), "{seat:?}'s key opens the vault");
        app::keybox::lock();
        app::keybox::unlock(&pin).expect("the new passcode opens");
        pin = pin.chars().rev().collect();
    }
    app::keybox::lock();
    app::identity::recover_with_words(&phrase, &pin).expect("the words");
    assert_eq!(app::keybox::state().ok(), Some(State::Open), "the words open the vault");
    // Another identity's seat key, and a key of no one here: refused by name, nothing opened or written.
    app::keybox::lock();
    let before = vault();
    assert_eq!(known(app::keybox::recover(&theirs, NEW_PIN)), Some(Known::RecoveryNotPrimary));
    assert_eq!(known(app::keybox::recover(&[0x5a; 32], NEW_PIN)), Some(Known::RecoveryNoMatch));
    assert_ne!(app::keybox::state().ok(), Some(State::Open));
    assert_eq!(vault(), before, "nothing written");
}

#[test]
fn an_existing_key_primary_recovers_with_its_key() {
    if super::alone_in(module_path!(), "an_existing_key_primary_recovers_with_its_key") {
        return;
    }
    super::vault_open();
    let s = app::key::generate().expect("a key");
    let row = app::register::change(|reg| app::identity::add_existing(reg, &s, Role::Author, false, "")).expect("the primary");
    assert_eq!(app::keybox::primary().ok().flatten(), Some((row.id.clone(), PrimaryKind::KeyFile)));
    assert_eq!(labels(&vault()), ["primary"], "one seal: its key");
    let k = key_of(&row, Role::Author);
    app::keybox::lock();
    app::keybox::recover(&k, NEW_PIN).expect("its key");
    assert_eq!(app::keybox::state().ok(), Some(State::Open));
}

/// Seals that are not exactly the primary's kind's set, in order (one missing, one more, two swapped, a label
/// another kind has, seals without a primary) are refused by name; the file as written reads again.
#[test]
fn seals_other_than_the_primarys_set_are_refused() {
    if super::alone_in(module_path!(), "seals_other_than_the_primarys_set_are_refused") {
        return;
    }
    super::vault_open();
    let words = app::identity::fresh().expect("words");
    app::register::change(|reg| app::identity::add_words(reg, &words, false, "")).expect("the primary");
    let good = vault();
    let no_primary = {
        let Value::Obj(members) = good.clone() else { panic!("an object") };
        Value::Obj(members.into_iter().filter(|(k, _)| k != m::PRIMARY).collect())
    };
    for (form, bytes) in [
        ("one missing", with_recovery(&good, |a| drop(a.pop()))),
        ("one more", with_recovery(&good, |a| a.push(a[0].clone()))),
        ("two swapped", with_recovery(&good, |a| a.swap(1, 2))),
        ("a key-file primary's set", with_recovery(&good, |a| a.truncate(1))),
        ("no primary", no_primary),
    ] {
        put_vault(&bytes);
        assert_eq!(known(app::keybox::primary_kind()), Some(Known::KeyboxShape), "{form}");
    }
    put_vault(&good);
    assert_eq!(app::keybox::primary_kind().ok().flatten(), Some(PrimaryKind::Words), "as written");
}
