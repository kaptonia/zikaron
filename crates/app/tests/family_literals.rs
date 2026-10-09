//! The family literals register (`family-literals.json`, public): every fixed literal this product uses that
//! another product of the family must not reuse, by role. Read here and checked against the code, both ways:
//! each entry's literal is the value at its place, and each place of the table below has its entry. Within the
//! register: roles from its own closed list, no literal twice in one role, a literal in two roles only as a
//! listed pair. (Whether the family's products collide with each other is the gauge's, not this test's.)

use app::roles::Role;
use zikaron::json::Value;

fn text(v: &Value, k: &str) -> String {
    v.member(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}

fn utf8(b: &[u8]) -> String {
    String::from_utf8(b.to_vec()).expect("text")
}

/// Every place a family literal lives in the code, with its value now.
fn places() -> Vec<(&'static str, String)> {
    vec![
        ("app::home::HOME_STEM", app::home::HOME_STEM.into()),
        ("app::backup::APP", app::backup::APP.into()),
        ("app::backup::EXT", app::backup::EXT.into()),
        ("app::mirror::STEM", app::mirror::STEM.into()),
        ("app::home::APP_DIR", app::home::APP_DIR.into()),
        ("zikaron_net::USER_AGENT", zikaron_net::USER_AGENT.into()),
        ("app::backup::MAGIC", utf8(app::backup::MAGIC)),
        ("zikaron_glue::sealed::MAGIC", utf8(zikaron_glue::sealed::MAGIC)),
        ("zikaron_glue::sealed::MAGIC_V2", utf8(zikaron_glue::sealed::MAGIC_V2)),
        ("app::local::GONE", utf8(app::local::GONE)),
        ("zikaron_glue::container::MAGIC", zikaron_glue::container::MAGIC.into()),
        ("app::keybox::SHAPE", app::keybox::SHAPE.into()),
        ("app::keybox::SHAPE_V2", app::keybox::SHAPE_V2.into()),
        ("app::keybox::SHAPE_V1", app::keybox::SHAPE_V1.into()),
        ("app::identity::SHAPE", app::identity::SHAPE.into()),
        ("app::machine::SHAPE", app::machine::SHAPE.into()),
        ("app::recordsx::FORM", app::recordsx::FORM.into()),
        ("app::backup::INDEX_FORM", app::backup::INDEX_FORM.into()),
        ("app::verifiedx::FORM", app::verifiedx::FORM.into()),
        ("app::checkedx::FORM", app::checkedx::FORM.into()),
        ("app::readnets::FORM", app::readnets::FORM.into()),
        ("app::kitsindex::FORM", app::kitsindex::FORM.into()),
        ("zikaron::tokens::SPEC", zikaron::tokens::SPEC.into()),
        ("zikaron_kit::tokens::SPEC_FPM", zikaron_kit::tokens::SPEC_FPM.into()),
        ("zikaron_kit::tokens::SPEC_ACK", zikaron_kit::tokens::SPEC_ACK.into()),
        ("zikaron_kit::tokens::SPEC_KIT", zikaron_kit::tokens::SPEC_KIT.into()),
        ("zikaron::tokens::Domain::Entry", zikaron::tokens::Domain::Entry.as_str().into()),
        ("zikaron::tokens::Domain::Adoption", zikaron::tokens::Domain::Adoption.as_str().into()),
        ("zikaron_kit::tokens::BADGE_PREFIX", zikaron_kit::tokens::BADGE_PREFIX.into()),
        ("app::adoptx::CLAIM_PREFIX", app::adoptx::CLAIM_PREFIX.into()),
        ("zikaron_glue::recording::FAMILY", zikaron_glue::recording::FAMILY.into()),
        ("app::keybox::LOCAL_INFO", utf8(app::keybox::LOCAL_INFO)),
        ("app::keybox::NAMES_INFO", utf8(app::keybox::NAMES_INFO)),
        ("app::keybox::BIND_DOMAIN", utf8(app::keybox::BIND_DOMAIN)),
        ("app::keybox::MEMBER_INFO", utf8(app::keybox::MEMBER_INFO)),
        ("app::local::OWNER_DOMAIN", app::local::OWNER_DOMAIN.into()),
        ("app::local::PLACE_DOMAIN", app::local::PLACE_DOMAIN.into()),
        ("app::local::PLACE_KEY_INFO", app::local::PLACE_KEY_INFO.into()),
        ("app::family::path_text(Author)", app::family::path_text(Role::Author)),
        ("app::family::path_text(Grantee)", app::family::path_text(Role::Grantee)),
        ("app::home::HOME_ENV", app::home::HOME_ENV.into()),
        ("app::trace::SINK_ENV", app::trace::SINK_ENV.into()),
        ("zikaron_os::door::NAME_HEAD", zikaron_os::door::NAME_HEAD.into()),
        ("zikaron_glue::door::FORM", zikaron_glue::door::FORM.into()),
        ("zikaron_os::cli_path::ENV_DIR", zikaron_os::cli_path::ENV_DIR.into()),
        ("zikaron_os::machine::POINTER", zikaron_os::machine::POINTER.into()),
        ("app::kitsindex::DIR/FILE", format!("{}/{}", app::kitsindex::DIR, app::kitsindex::FILE)),
        ("app::kitsindex::DIR/verifiedx::DIR", format!("{}/{}", app::kitsindex::DIR, app::verifiedx::DIR)),
        ("zikaron_glue::names::MANIFEST", zikaron_glue::names::MANIFEST.into()),
    ]
}

#[test]
fn the_register_is_the_codes_literals_both_ways() {
    let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("family-literals.json")).expect("the register");
    let reg = zikaron::json::parse(&bytes).expect("the register reads");
    assert_eq!(text(&reg, "schema"), "zikaron-family-literals/1");
    // The literals of other products this one reads: none (it reads no other product's files).
    assert!(reg.member("foreign").map(|f| matches!(f, Value::Obj(m) if m.is_empty())).unwrap_or(false), "foreign is an empty object");
    let roles: Vec<String> = reg.member("roles").and_then(|r| r.as_arr()).expect("roles").iter().filter_map(|r| r.as_str().map(str::to_string)).collect();
    let entries = reg.member("entries").and_then(|e| e.as_arr()).expect("entries").clone();
    let places = places();
    for e in &entries {
        let (role, literal, at) = (text(e, "role"), text(e, "literal"), text(e, "at"));
        assert!(roles.contains(&role), "{at}: role {role} is not in the register's list");
        assert!(!text(e, "says").is_empty(), "{at}: says what it is");
        let value = places.iter().find(|(p, _)| *p == at).map(|(_, v)| v.clone()).unwrap_or_else(|| panic!("{at}: no such place in the code"));
        assert_eq!(literal, value, "{at}: the register and the code differ");
    }
    for (at, _) in &places {
        assert!(entries.iter().any(|e| text(e, "at") == *at), "{at}: a family literal with no entry");
    }
    literals_within_register(&reg);
}

/// The two rules within a register, on any parsed register: no literal twice in one role, and a literal in two
/// roles only as a listed pair (each listed pair a literal in exactly those two roles). Fails by assertion.
fn literals_within_register(reg: &Value) {
    let entries = reg.member("entries").and_then(|e| e.as_arr()).expect("entries").clone();
    let mut seen: Vec<(String, String)> = Vec::new();
    for e in &entries {
        let (role, literal) = (text(e, "role"), text(e, "literal"));
        assert!(!seen.contains(&(role.clone(), literal.clone())), "{literal:?} twice in role {role}");
        seen.push((role, literal));
    }
    // A literal in two roles only as a listed pair.
    let pairs: Vec<(String, Vec<String>)> = reg
        .member("pairs")
        .and_then(|p| p.as_arr())
        .expect("pairs")
        .iter()
        .map(|p| (text(p, "literal"), p.member("roles").and_then(|r| r.as_arr()).map(|r| r.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()))
        .collect();
    for (_, literal) in &seen {
        let mut in_roles: Vec<String> = seen.iter().filter(|(_, l)| l == literal).map(|(r, _)| r.clone()).collect();
        in_roles.sort();
        if in_roles.len() > 1 {
            let listed = pairs.iter().any(|(l, rs)| {
                let mut rs = rs.clone();
                rs.sort();
                l == literal && rs == in_roles
            });
            assert!(listed, "{literal:?} is in {in_roles:?} without a listed pair");
        }
    }
    for (literal, rs) in &pairs {
        assert!(rs.len() == 2 && rs.iter().all(|r| seen.contains(&(r.clone(), literal.clone()))), "the pair {literal:?} is a literal in exactly those two roles");
    }
}

/// Whether the rules within a register fail on it, and with which words.
fn refused(register: &[u8]) -> Option<String> {
    let reg = zikaron::json::parse(register).expect("a small register reads");
    let r = std::panic::catch_unwind(|| literals_within_register(&reg));
    r.err().map(|p| p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default())
}

/// A register with one literal twice in one role fails the rules.
#[test]
fn a_register_with_a_literal_twice_in_one_role_is_refused() {
    let dup = br#"{"entries":[{"role":"magic","literal":"ZK1"},{"role":"magic","literal":"ZK1"}],"pairs":[]}"#;
    let said = refused(dup).expect("refused");
    assert!(said.contains("twice in role magic"), "{said}");
    let apart = br#"{"entries":[{"role":"magic","literal":"ZK1"},{"role":"magic","literal":"ZK2"}],"pairs":[]}"#;
    assert_eq!(refused(apart), None, "two literals in one role pass");
}

/// A register with a literal in two roles and no listed pair fails the rules; the same with the pair listed
/// passes.
#[test]
fn a_register_with_a_cross_role_literal_and_no_pair_is_refused() {
    let unpaired = br#"{"entries":[{"role":"magic","literal":"ZK1"},{"role":"form","literal":"ZK1"}],"pairs":[]}"#;
    let said = refused(unpaired).expect("refused");
    assert!(said.contains("without a listed pair"), "{said}");
    let paired = br#"{"entries":[{"role":"magic","literal":"ZK1"},{"role":"form","literal":"ZK1"}],"pairs":[{"literal":"ZK1","roles":["form","magic"]}]}"#;
    assert_eq!(refused(paired), None, "listed as a pair, it passes");
}
