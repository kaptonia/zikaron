use zikaron::json::Value;
use zikaron_glue::book::{self, Law};

const A: [u8; 32] = [0x11; 32];
const B: [u8; 32] = [0x22; 32];

fn addr(k: &[u8; 32]) -> String {
    zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(k).expect("an address"))
}

fn entry(key: &[u8; 32], kind: &str, seq: u64, prev: Option<&str>, body: Value) -> Vec<u8> {
    let sealed = book::seal::<()>(Law::Two, &addr(key), kind, seq, prev, body, |pre, domain| {
        let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
        let (r, s, v) = zikaron::cryptox::sign_digest(key, &digest).ok_or(())?;
        let mut raw = r.to_vec();
        raw.extend_from_slice(&s);
        raw.push(v);
        Ok(zikaron::hexfmt::encode(&raw))
    });
    match sealed {
        book::Sealed::Done(e) => e.bytes,
        _ => panic!("sealed"),
    }
}

fn obj(ms: &[(&str, Value)]) -> Value {
    Value::Obj(ms.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
}

fn genesis(key: &[u8; 32]) -> Vec<u8> {
    entry(key, "genesis", 0, None, obj(&[("statement_md", Value::Str("开端".into()))]))
}

fn note(key: &[u8; 32], seq: u64, prev: &[u8], text: &str) -> Vec<u8> {
    entry(key, "annotation", seq, Some(&zikaron::hexfmt::encode(&book::entry_id(prev))), obj(&[("note_md", Value::Str(text.into()))]))
}

#[test]
fn the_tip_and_what_a_write_would_break_are_one_reading() {
    let g = genesis(&A);
    let one = note(&A, 1, &g, "一");
    let root = addr(&A);
    let line = vec![g.clone(), one.clone()];
    assert_eq!(book::tip(&root, &line), Ok((2, zikaron::hexfmt::encode(&book::entry_id(&one)))));
    let next = note(&A, 2, &one, "二");
    assert_eq!(book::new_findings(&line, &next, None), Ok(Vec::new()), "the next entry after the head breaks nothing");
    let other = note(&A, 1, &g, "另一");
    let forked = vec![g.clone(), one.clone(), other.clone()];
    assert_eq!(book::tip(&root, &forked), Err(book::NoTip::Forked), "a fork at the tip is no place to write");
    assert!(!book::new_findings(&line, &other, None).expect("read").is_empty(), "a second entry at one place is a new finding");
    assert_eq!(book::tip(&root, &[]), Err(book::NoTip::Absent));
}

#[test]
fn a_ledger_is_handed_only_to_a_key_outside_its_lineage() {
    let g = genesis(&A);
    let read = book::read(&[g], &book::empty_fragment()).expect("read");
    assert!(!read.may_succeed(&addr(&A)), "its root");
    assert!(!read.may_succeed(&addr(&A).to_ascii_uppercase().replace("0X", "0x")), "in any letter case");
    assert!(read.may_succeed(&addr(&B)), "a key outside it");
}

#[test]
fn a_copy_behind_the_book_is_no_ledger_to_judge_from() {
    let g = genesis(&A);
    let copy = vec![g.clone()];
    assert!(book::behind_book(&copy, Some(&addr(&B))), "the book names a key the copy does not");
    assert!(!book::behind_book(&copy, Some(&addr(&A))), "the key the copy names");
    assert!(!book::behind_book(&copy, None), "no book read is no judgement");
    let input = obj(&[("pile", Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(&g))])), ("root", Value::Str(addr(&A)))]);
    assert!(book::under_book(Some(&input), Some(&addr(&B))).is_none(), "the input goes");
    assert!(book::under_book(Some(&input), None).is_some(), "kept without a book");
}

fn old_entry(key: &[u8; 32], kind: &str, seq: u64, prev: Option<&str>, body: Value) -> Vec<u8> {
    let sealed = book::seal::<()>(Law::One, &addr(key), kind, seq, prev, body, |pre, domain| {
        let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
        let (r, s, v) = zikaron::cryptox::sign_digest(key, &digest).ok_or(())?;
        let mut raw = r.to_vec();
        raw.extend_from_slice(&s);
        raw.push(v);
        Ok(zikaron::hexfmt::encode(&raw))
    });
    match sealed {
        book::Sealed::Done(e) => e.bytes,
        _ => panic!("sealed"),
    }
}

#[test]
fn an_old_grant_carrying_the_member_is_read_by_the_new_branch() {
    let g = old_entry(&A, "genesis", 0, None, obj(&[("statement_md", Value::Str("旧".into()))]));
    let prev = zikaron::hexfmt::encode(&book::entry_id(&g));
    let body = |member: bool| {
        let mut ms = vec![
            ("grantee", Value::Str(addr(&B))),
            ("terms", Value::Str(format!("0x{}", "bb".repeat(32)))),
            ("work", Value::Str(format!("0x{}", "aa".repeat(32)))),
        ];
        if member {
            ms.push(("revocation", obj(&[("at", Value::Str(format!("0x{}", "c0".repeat(20)))), ("chain", Value::Int(7))])));
        }
        obj(&ms)
    };
    let plain = old_entry(&A, "grant", 1, Some(&prev), body(false));
    let carrying = old_entry(&A, "grant", 1, Some(&prev), body(true));
    let old = book::grant_check(&plain, None, None, None);
    assert!(old.member("notices").is_none(), "the old law's result: {old:?}");
    let new = book::grant_check(&carrying, None, Some(&Value::Null), None);
    assert_eq!(new.member("notices"), Some(&Value::Null), "the new branch's result: {new:?}");
    let chain = book::chain_check(&[(carrying, None, None)], None);
    assert!(chain.member("hops").and_then(Value::as_arr).and_then(|h| h.first()).is_some_and(|h| h.member("notices").is_some()));
}

const C: [u8; 32] = [0x33; 32];

fn succession(key: &[u8; 32], seq: u64, prev: &[u8], to: &[u8; 32]) -> Vec<u8> {
    let body = obj(&[
        ("effective", Value::Int(1_700_000_000)),
        ("kind", Value::Str("handover".into())),
        ("statement_md", Value::Str("交给".into())),
        ("to", Value::Str(addr(to))),
    ]);
    entry(key, "succession", seq, Some(&zikaron::hexfmt::encode(&book::entry_id(prev))), body)
}

#[test]
fn an_entry_whose_signer_the_ledger_does_not_count_is_barred_and_only_that() {
    let g = genesis(&A);
    assert_eq!(book::new_findings(&[], &g, None), Ok(Vec::new()), "the genesis of an empty ledger");
    let stranger = note(&C, 1, &g, "外人");
    match book::new_findings(&[g.clone()], &stranger, None) {
        Err(book::Barred::Signer { counted, key, office }) => {
            assert_eq!(counted, book::Sender::Outside);
            assert_eq!(counted.as_str(), "NOT_IN_LINEAGE");
            assert!(key.eq_ignore_ascii_case(&addr(&C)), "the signer: {key}");
            assert!(office.eq_ignore_ascii_case(&addr(&A)), "the key in office: {office}");
        }
        other => panic!("an outsider's entry is barred: {other:?}"),
    }
    let s = succession(&A, 1, &g, &B);
    let seated = vec![g.clone(), s.clone()];
    assert_eq!(book::new_findings(&[g.clone()], &s, None), Ok(Vec::new()), "the succession by the key in office");
    assert_eq!(book::new_findings(&seated, &note(&B, 2, &s, "接任"), None), Ok(Vec::new()), "the successor's first entry");
    assert_eq!(book::new_findings(&seated, &note(&A, 2, &s, "旧钥"), None), Ok(vec!["AUTHORITY_MISMATCH".to_string()]), "the key that handed off: its finding, as before");
    assert!(matches!(book::new_findings(&seated, &note(&C, 2, &s, "外人"), None), Err(book::Barred::Signer { .. })), "an outsider after a succession");
}
