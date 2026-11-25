use zikaron::cryptox;
use zikaron::entry as v1;
use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::tokens::{Domain as V1Domain, Token};
use zikaron::v2::audit;
use zikaron::v2::entry as v2;

fn key(n: u8) -> [u8; 32] {
    let mut k = [0u8; 32];
    k[31] = n;
    k
}

fn addr(n: u8) -> String {
    hexfmt::encode(&cryptox::address_of_privkey(&key(n)).expect("key"))
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn obj(ms: Vec<(&str, Value)>) -> Value {
    Value::Obj(ms.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn entry(spec: &str, n: u8, etype: &str, seq: u64, prev: Option<&str>, body: Value) -> Vec<u8> {
    let six = obj(vec![
        ("spec", s(spec)),
        ("entryType", s(etype)),
        ("author", s(&addr(n))),
        ("seq", Value::Int(seq)),
        ("prev", prev.map(s).unwrap_or(Value::Null)),
        ("body", body),
    ]);
    let domain = if spec == zikaron::tokens::SPEC { V1Domain::Entry.as_str() } else { zikaron::v2::tokens::Domain::Entry.as_str() };
    let (_, digest) = v1::presig_and_digest(&json::canon_bytes(&six), domain);
    let (r, sg, v) = cryptox::sign_digest(&key(n), &digest).expect("sign");
    let mut sig = r.to_vec();
    sig.extend_from_slice(&sg);
    sig.push(v);
    let Value::Obj(mut ms) = six else { unreachable!() };
    ms.push(("sig".to_string(), s(&hexfmt::encode(&sig))));
    json::canon_bytes(&Value::Obj(ms))
}

fn id(b: &[u8]) -> String {
    hexfmt::encode(&v1::entry_id(b))
}

fn anchor(hash: &str, block: u64, ts: u64, sender: u8) -> Value {
    obj(vec![
        ("chainId", Value::Int(1)),
        ("blockNumber", Value::Int(block)),
        ("blockTimestamp", Value::Int(ts)),
        ("tx", s(&format!("0x{:064x}", block))),
        ("sender", s(&addr(sender))),
        ("hash", s(hash)),
        ("verdict", s("counted")),
    ])
}

fn basis(senders: &[u8]) -> Value {
    let mut ss: Vec<String> = senders.iter().map(|n| addr(*n)).collect();
    ss.sort();
    obj(vec![(
        "chains",
        Value::Arr(vec![obj(vec![
            ("chainId", Value::Int(1)),
            ("fromBlock", Value::Int(0)),
            ("toBlock", Value::Int(1000)),
            ("registries", Value::Arr(vec![s(&format!("0x{}", "11".repeat(20)))])),
            ("senders", Value::Arr(ss.into_iter().map(Value::Str).collect())),
        ])]),
    )])
}

fn input(root: u8, pile: &[&Vec<u8>], anchors: Vec<Value>, basis: Value) -> Value {
    obj(vec![
        ("root", s(&addr(root))),
        ("pile", Value::Arr(pile.iter().map(|b| s(&hexfmt::encode(b))).collect())),
        ("anchors", Value::Arr(anchors)),
        ("basis", basis),
    ])
}

fn ids_of(v: &Value, list: &str, key: Option<&str>) -> Vec<String> {
    v.member(list)
        .and_then(Value::as_arr)
        .into_iter()
        .flatten()
        .filter_map(|r| match key {
            Some(k) => r.member(k).and_then(Value::as_str).map(str::to_string),
            None => r.as_str().map(str::to_string),
        })
        .collect()
}

fn label(v: &Value) -> &str {
    v.member("label").and_then(Value::as_str).unwrap_or("")
}

struct Upgraded {
    g1: Vec<u8>,
    h1: Vec<u8>,
    close: Vec<u8>,
    genesis: Vec<u8>,
    h2: Vec<u8>,
}

fn upgraded() -> Upgraded {
    let g1 = entry("zikaron/1", 1, "genesis", 0, None, obj(vec![("statement_md", s("a ledger"))]));
    let h1 = entry(
        "zikaron/1",
        1,
        "history",
        1,
        Some(&id(&g1)),
        obj(vec![
            ("content", s(&format!("0x{}", "22".repeat(32)))),
            ("mode", obj(vec![("mark", s("hand")), ("toolchain", s(&format!("0x{}", "33".repeat(32))))])),
        ]),
    );
    let close = entry(
        "zikaron/1",
        1,
        "succession",
        2,
        Some(&id(&h1)),
        obj(vec![("to", s(&addr(1))), ("kind", s("migration")), ("effective", Value::Int(0)), ("statement_md", s(""))]),
    );
    let genesis = entry(
        "zikaron/2",
        1,
        "genesis",
        0,
        None,
        obj(vec![("statement_md", s("continued")), ("continues", obj(vec![("root", s(&addr(1))), ("entry_id", s(&id(&close)))]))]),
    );
    let h2 = entry(
        "zikaron/2",
        1,
        "history",
        1,
        Some(&id(&genesis)),
        obj(vec![
            ("content", s(&format!("0x{}", "44".repeat(32)))),
            ("mode", obj(vec![("mark", s("hand")), ("toolchain", s(&format!("0x{}", "33".repeat(32))))])),
        ]),
    );
    Upgraded { g1, h1, close, genesis, h2 }
}

#[test]
fn an_upgraded_ledger_reads_as_one_ledger_of_two_segments() {
    let u = upgraded();
    let anchors = vec![
        anchor(&id(&u.g1), 1, 100, 1),
        anchor(&id(&u.h1), 2, 110, 1),
        anchor(&id(&u.close), 3, 120, 1),
        anchor(&id(&u.genesis), 4, 130, 1),
        anchor(&id(&u.h2), 5, 140, 1),
    ];
    let rep = audit::audit(&input(1, &[&u.g1, &u.h1, &u.close, &u.genesis, &u.h2], anchors, basis(&[1])));
    assert_eq!(label(&rep), "COMPLETE", "{}", String::from_utf8_lossy(&json::canon_bytes(&rep)));
    let c = rep.member("continues").expect("continues");
    assert_eq!(c.member("entry_id").and_then(Value::as_str), Some(id(&u.close).as_str()));
    assert_eq!(c.member("cut").and_then(Value::as_int), Some(120));
    let old = c.member("report").expect("old report");
    let mut old_anchored = ids_of(old, "anchored", Some("entry_id"));
    old_anchored.sort();
    let mut want = vec![id(&u.g1), id(&u.h1), id(&u.close)];
    want.sort();
    assert_eq!(old_anchored, want);
    let mut new_anchored = ids_of(&rep, "anchored", Some("entry_id"));
    new_anchored.sort();
    let mut want = vec![id(&u.genesis), id(&u.h2)];
    want.sort();
    assert_eq!(new_anchored, want);
    assert!(ids_of(&rep, "malformed", Some("entry_id")).is_empty(), "the old segment's bytes are not this law's junk");
}

#[test]
fn without_a_record_of_the_close_nothing_continues() {
    let u = upgraded();
    let rep = audit::audit(&input(1, &[&u.g1, &u.h1, &u.close, &u.genesis, &u.h2], vec![], basis(&[1])));
    assert_eq!(rep.member("continues"), Some(&Value::Null));
    assert_eq!(ids_of(&rep, "malformed", Some("entry_id")).len(), 3);
    assert_eq!(rep.member("entries").and_then(Value::as_int), Some(2));
}

#[test]
fn a_retired_key_writes_nothing_into_the_seat() {
    let g = entry("zikaron/2", 1, "genesis", 0, None, obj(vec![("statement_md", s(""))]));
    let succ = entry(
        "zikaron/2",
        1,
        "succession",
        1,
        Some(&id(&g)),
        obj(vec![("to", s(&addr(2))), ("kind", s("keyrotation")), ("effective", Value::Int(0)), ("statement_md", s(""))]),
    );
    let note = |n: u8, text: &str| {
        entry("zikaron/2", n, "annotation", 2, Some(&id(&succ)), obj(vec![("note_md", s(text))]))
    };
    let by_new = note(2, "new key");
    let by_old = note(1, "stolen old key");
    let anchors = vec![
        anchor(&id(&g), 1, 190, 1),
        anchor(&id(&succ), 2, 200, 1),
        anchor(&id(&by_new), 3, 210, 2),
        anchor(&id(&by_old), 4, 220, 1),
    ];
    let rep = audit::audit(&input(1, &[&g, &succ, &by_new, &by_old], anchors, basis(&[1, 2])));
    assert_eq!(label(&rep), "COMPLETE", "{}", String::from_utf8_lossy(&json::canon_bytes(&rep)));
    assert_eq!(ids_of(&rep, "retired", Some("entry_id")), vec![id(&by_old)]);
    assert_eq!(ids_of(&rep, "discarded", Some("hash")), vec![id(&by_old)]);
    assert!(rep.member("findings").and_then(Value::as_arr).is_some_and(|f| f.is_empty()));
}

fn handing_off() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let g = entry("zikaron/2", 1, "genesis", 0, None, obj(vec![("statement_md", s(""))]));
    let succ = entry(
        "zikaron/2",
        1,
        "succession",
        1,
        Some(&id(&g)),
        obj(vec![("to", s(&addr(2))), ("kind", s("keyrotation")), ("effective", Value::Int(0)), ("statement_md", s(""))]),
    );
    let late = entry("zikaron/2", 1, "annotation", 2, Some(&id(&succ)), obj(vec![("note_md", s("re-sent"))]));
    (g, succ, late)
}

#[test]
fn what_a_key_signs_above_its_retiring_seq_counts_for_nothing_in_that_second() {
    let (g, succ, late) = handing_off();
    for at in [200, 201] {
        let anchors = vec![anchor(&id(&g), 1, 190, 1), anchor(&id(&succ), 2, 200, 1), anchor(&id(&late), 3, at, 1)];
        let rep = audit::audit(&input(1, &[&g, &succ, &late], anchors, basis(&[1, 2])));
        assert_eq!(label(&rep), "COMPLETE", "at {at}: {}", String::from_utf8_lossy(&json::canon_bytes(&rep)));
        assert_eq!(ids_of(&rep, "retired", Some("entry_id")), vec![id(&late)]);
        assert_eq!(ids_of(&rep, "discarded", Some("hash")), vec![id(&late)]);
    }
}

#[test]
fn what_a_key_signed_at_or_below_its_retiring_seq_counts_in_that_second() {
    let (g, succ, _) = handing_off();
    let anchors = vec![anchor(&id(&g), 1, 200, 1), anchor(&id(&succ), 2, 200, 1), anchor(&id(&g), 3, 200, 1)];
    let rep = audit::audit(&input(1, &[&g, &succ], anchors, basis(&[1, 2])));
    assert_eq!(label(&rep), "COMPLETE", "{}", String::from_utf8_lossy(&json::canon_bytes(&rep)));
    assert!(ids_of(&rep, "retired", Some("entry_id")).is_empty());
    assert!(ids_of(&rep, "discarded", Some("hash")).is_empty());
    assert_eq!(rep.member("entries").and_then(Value::as_int), Some(2));
}

#[test]
fn two_handings_off_at_the_retiring_seq_are_a_fork() {
    let (g, succ, _) = handing_off();
    let twin = entry(
        "zikaron/2",
        1,
        "succession",
        1,
        Some(&id(&g)),
        obj(vec![("to", s(&addr(3))), ("kind", s("keyrotation")), ("effective", Value::Int(0)), ("statement_md", s(""))]),
    );
    let anchors = vec![anchor(&id(&succ), 1, 200, 1), anchor(&id(&twin), 2, 200, 1)];
    let rep = audit::audit(&input(1, &[&g, &succ, &twin], anchors, basis(&[1, 2, 3])));
    assert_eq!(label(&rep), "BROKEN_CHAIN", "{}", String::from_utf8_lossy(&json::canon_bytes(&rep)));
}

#[test]
fn the_retiring_seq_is_the_lowest_handing_off_of_that_second() {
    let (g, succ, _) = handing_off();
    let second = entry(
        "zikaron/2",
        1,
        "succession",
        2,
        Some(&id(&succ)),
        obj(vec![("to", s(&addr(3))), ("kind", s("keyrotation")), ("effective", Value::Int(0)), ("statement_md", s(""))]),
    );
    for (a, b) in [(&succ, &second), (&second, &succ)] {
        let anchors = vec![anchor(&id(a), 1, 200, 1), anchor(&id(b), 2, 200, 1)];
        let rep = audit::audit(&input(1, &[&g, &succ, &second], anchors, basis(&[1, 2, 3])));
        assert_eq!(label(&rep), "COMPLETE", "{}", String::from_utf8_lossy(&json::canon_bytes(&rep)));
        assert_eq!(ids_of(&rep, "retired", Some("entry_id")), vec![id(&second)]);
    }
}

#[test]
fn a_record_before_the_retirement_still_makes_it_timely() {
    let (g, succ, late) = handing_off();
    let rival = entry("zikaron/2", 2, "annotation", 2, Some(&id(&succ)), obj(vec![("note_md", s("new key"))]));
    let anchors = vec![anchor(&id(&late), 1, 199, 1), anchor(&id(&succ), 2, 200, 1), anchor(&id(&rival), 3, 201, 2)];
    let rep = audit::audit(&input(1, &[&g, &succ, &late, &rival], anchors, basis(&[1, 2])));
    assert_eq!(label(&rep), "BROKEN_CHAIN", "{}", String::from_utf8_lossy(&json::canon_bytes(&rep)));
    assert!(ids_of(&rep, "retired", Some("entry_id")).is_empty());
}

#[test]
fn another_keys_record_of_that_second_counts_and_bounds_nothing() {
    let (g, succ, late) = handing_off();
    let anchors = vec![anchor(&id(&g), 1, 190, 1), anchor(&id(&succ), 2, 200, 1), anchor(&id(&late), 3, 200, 2)];
    let rep = audit::audit(&input(1, &[&g, &succ, &late], anchors, basis(&[1, 2])));
    assert_eq!(label(&rep), "GAPS", "{}", String::from_utf8_lossy(&json::canon_bytes(&rep)));
    assert_eq!(ids_of(&rep, "retired", Some("entry_id")), vec![id(&late)]);
    assert_eq!(ids_of(&rep, "missing", Some("hash")), vec![id(&late)]);
    assert!(ids_of(&rep, "discarded", Some("hash")).is_empty());
}

#[test]
fn a_keys_record_of_that_second_over_unheld_bytes_counts() {
    let (g, succ, late) = handing_off();
    let anchors = || vec![anchor(&id(&g), 1, 190, 1), anchor(&id(&succ), 2, 200, 1), anchor(&id(&late), 3, 200, 1)];
    let without = audit::audit(&input(1, &[&g, &succ], anchors(), basis(&[1, 2])));
    assert_eq!(label(&without), "GAPS", "{}", String::from_utf8_lossy(&json::canon_bytes(&without)));
    assert_eq!(ids_of(&without, "missing", Some("hash")), vec![id(&late)]);
    assert!(ids_of(&without, "discarded", Some("hash")).is_empty());
    let with = audit::audit(&input(1, &[&g, &succ, &late], anchors(), basis(&[1, 2])));
    assert_eq!(label(&with), "COMPLETE");
    assert_eq!(ids_of(&with, "discarded", Some("hash")), vec![id(&late)]);
}

#[test]
fn a_migration_closes_the_walk() {
    let g = entry("zikaron/2", 1, "genesis", 0, None, obj(vec![("statement_md", s(""))]));
    let mig = entry("zikaron/2", 1, "migration", 1, Some(&id(&g)), obj(vec![("spec", s("zikaron/3")), ("statement_md", s(""))]));
    let after = entry("zikaron/2", 1, "annotation", 2, Some(&id(&mig)), obj(vec![("note_md", s("after"))]));
    let rep = audit::audit(&input(1, &[&g, &mig, &after], vec![], basis(&[1])));
    assert_eq!(label(&rep), "BROKEN_CHAIN");
    let f = &rep.member("findings").and_then(Value::as_arr).expect("findings")[0];
    assert_eq!(f.member("name").and_then(Value::as_str), Some("AUTHORITY_MISMATCH"));
    assert_eq!(f.member("entry_id").and_then(Value::as_str), Some(id(&after).as_str()));
}

#[test]
fn a_v1_basis_has_no_label() {
    let Value::Obj(mut ms) = basis(&[1]) else { unreachable!() };
    ms.push(("bareTx".to_string(), Value::Arr(vec![])));
    let rep = audit::audit(&input(1, &[], vec![], Value::Obj(ms)));
    assert_eq!(rep, audit::no_label());
}

#[test]
fn a_v1_entry_is_e_spec_here() {
    let u = upgraded();
    assert_eq!(v2::check(&u.g1).err(), Some(Token::Spec));
    assert!(v1::check(&u.g1).is_ok());
    assert!(v2::check(&u.genesis).is_ok());
}

#[test]
fn a_continues_without_its_members_is_a_body_fault() {
    let bad = entry("zikaron/2", 1, "genesis", 0, None, obj(vec![("statement_md", s("")), ("continues", obj(vec![("root", s(&addr(1)))]))]));
    assert_eq!(v2::check(&bad).err(), Some(Token::BodyField));
}
