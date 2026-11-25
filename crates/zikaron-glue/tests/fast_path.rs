use zikaron::hexfmt;
use zikaron::json::{self, Value};
use zikaron::tokens::Verdict;
use zikaron_anchor::scan::{Claim, Pick};
use zikaron_glue::book::{self, Fast, Hold, Law, Sealed, Undecided};

const A: [u8; 32] = [0x11; 32];
const B: [u8; 32] = [0x22; 32];
const X: [u8; 32] = [0x33; 32];
const CHAIN: u64 = 1;
const REGISTRY: &str = "0x5e00000000000000000000000000000000000001";
const NOW: u64 = 1_000;

fn addr(k: &[u8; 32]) -> String {
    hexfmt::encode(&zikaron::cryptox::address_of_privkey(k).expect("key"))
}

fn addr20(k: &[u8; 32]) -> [u8; 20] {
    zikaron::cryptox::address_of_privkey(k).expect("key")
}

fn sealed(law: Law, key: &[u8; 32], typ: &str, seq: u64, prev: Option<&[u8]>, mut body: Vec<(&str, Value)>) -> Vec<u8> {
    body.sort_by(|a, b| a.0.cmp(b.0));
    sealed_value(law, key, typ, seq, prev, Value::Obj(body.into_iter().map(|(k, v)| (k.to_string(), v)).collect()))
}

fn sealed_value(law: Law, key: &[u8; 32], typ: &str, seq: u64, prev: Option<&[u8]>, body: Value) -> Vec<u8> {
    let prev = prev.map(|p| hexfmt::encode(&book::entry_id(p)));
    let sign = |pre: &[u8], domain: &str| -> Result<String, ()> {
        let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
        let (r, s, v) = zikaron::cryptox::sign_digest(key, &digest).ok_or(())?;
        let mut sig = r.to_vec();
        sig.extend_from_slice(&s);
        sig.push(v);
        Ok(hexfmt::encode(&sig))
    };
    match book::seal(law, &addr(key), typ, seq, prev.as_deref(), body, sign) {
        Sealed::Done(e) => e.bytes,
        _ => panic!("sealed {typ}"),
    }
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn genesis(key: &[u8; 32]) -> Vec<u8> {
    sealed(Law::Two, key, "genesis", 0, None, vec![("statement_md", s("works"))])
}

fn history(key: &[u8; 32], seq: u64, prev: &[u8], n: u8) -> Vec<u8> {
    let mode = Value::Obj(vec![("mark".into(), s("hand")), ("toolchain".into(), s(&hexfmt::encode(&[0x70; 32])))]);
    sealed(Law::Two, key, "history", seq, Some(prev), vec![("content", s(&hexfmt::encode(&[n; 32]))), ("mode", mode)])
}

fn succession(key: &[u8; 32], seq: u64, prev: &[u8], to: &[u8; 32]) -> Vec<u8> {
    sealed(Law::Two, key, "succession", seq, Some(prev), vec![("effective", Value::Int(1)), ("kind", s("keyrotation")), ("statement_md", s("next key")), ("to", s(&addr(to)))])
}

fn grant_body(member: Option<Value>) -> Vec<(&'static str, Value)> {
    let mut b = vec![("grantee", s(&format!("0x{}", "44".repeat(20)))), ("terms", s(&hexfmt::encode(&[0x71; 32]))), ("work", s(&hexfmt::encode(&[0x72; 32])))];
    if let Some(m) = member {
        b.push(("revocation", m));
    }
    b
}

fn grant(key: &[u8; 32], seq: u64, prev: &[u8]) -> Vec<u8> {
    sealed(Law::Two, key, "grant", seq, Some(prev), grant_body(None))
}

fn revocation(key: &[u8; 32], seq: u64, prev: &[u8], g: &[u8]) -> Vec<u8> {
    sealed(Law::Two, key, "revocation", seq, Some(prev), vec![("grant", s(&hexfmt::encode(&book::entry_id(g))))])
}

#[derive(Clone)]
struct Log {
    claim: Claim,
    time: u64,
    verdict: Option<Verdict>,
}

fn log(n: u8, block: u64, sender: &[u8; 32], hash: [u8; 32], verdict: Option<Verdict>) -> Log {
    Log { claim: Claim { chain_id: CHAIN, block_number: block, tx: [n; 32], log_index: 0, sender: addr20(sender), hash }, time: 100 + block, verdict }
}

fn of(e: &[u8]) -> [u8; 32] {
    book::entry_id(e)
}

fn basis() -> Value {
    let mut senders = vec![addr(&A), addr(&B), addr(&X)];
    senders.sort();
    json::parse(
        format!(
            r#"{{"chains":[{{"chainId":{CHAIN},"fromBlock":0,"registries":["{REGISTRY}"],"senders":[{}],"toBlock":10000}}]}}"#,
            senders.iter().map(|x| format!("\"{x}\"")).collect::<Vec<_>>().join(",")
        )
        .as_bytes(),
    )
    .expect("basis")
}

fn claim_value(c: &Claim) -> Value {
    Value::Obj(vec![
        ("blockNumber".into(), Value::Int(c.block_number)),
        ("chainId".into(), Value::Int(c.chain_id)),
        ("hash".into(), s(&hexfmt::encode(&c.hash))),
        ("logIndex".into(), Value::Int(c.log_index)),
        ("sender".into(), s(&hexfmt::encode(&c.sender))),
        ("tx".into(), s(&hexfmt::encode(&c.tx))),
    ])
}

fn fragment(logs: &[Log], tested: &[bool]) -> Value {
    let mut anchors: Vec<(Claim, Value)> = logs
        .iter()
        .zip(tested)
        .filter(|(l, t)| **t && l.verdict.is_some())
        .map(|(l, _)| {
            let c = &l.claim;
            let v = Value::Obj(vec![
                ("blockNumber".into(), Value::Int(c.block_number)),
                ("blockTimestamp".into(), Value::Int(l.time)),
                ("chainId".into(), Value::Int(c.chain_id)),
                ("hash".into(), s(&hexfmt::encode(&c.hash))),
                ("sender".into(), s(&hexfmt::encode(&c.sender))),
                ("tx".into(), s(&hexfmt::encode(&c.tx))),
                ("verdict".into(), s(l.verdict.expect("anchor").as_str())),
            ]);
            (c.clone(), v)
        })
        .collect();
    anchors.sort_by(|a, b| (a.0.block_number, a.0.tx, a.0.hash).cmp(&(b.0.block_number, b.0.tx, b.0.hash)));
    let mut candidates: Vec<Claim> = logs.iter().zip(tested).filter(|(_, t)| !**t).map(|(l, _)| l.claim.clone()).collect();
    candidates.sort();
    let mut members = vec![("anchors".to_string(), Value::Arr(anchors.into_iter().map(|(_, v)| v).collect())), ("basis".to_string(), basis())];
    if !candidates.is_empty() {
        members.push(("candidates".to_string(), Value::Arr(candidates.iter().map(claim_value).collect())));
    }
    Value::Obj(members)
}

fn fast(hold: &Hold, logs: &[Log]) -> (Value, usize) {
    let mut d = book::decision_set(hold).expect("decided");
    for _ in 0..=logs.len() + 1 {
        let Pick::Only(choose) = d.pick() else { panic!("a choice") };
        let tested: Vec<bool> = logs.iter().map(|l| choose(&l.claim.hash)).collect();
        let frag = fragment(logs, &tested);
        match book::fast_input(&mut d, &frag).expect("input") {
            Fast::Ready(input) => return (input, tested.iter().filter(|t| **t).count()),
            Fast::More => continue,
        }
    }
    panic!("the tested set did not settle");
}

fn same(root: &str, pile: &[Vec<u8>], g: &[u8], notices: Option<&Value>, logs: &[Log]) -> usize {
    let whole = book::input_of(root, pile, &fragment(logs, &vec![true; logs.len()])).expect("whole input");
    let (quick, tested) = fast(&Hold { grant: g, root, pile }, logs);
    let check = |i: &Value| json::canon_bytes(&book::grant_check(g, Some(i), notices, Some(NOW)));
    assert_eq!(String::from_utf8(check(&whole)).unwrap(), String::from_utf8(check(&quick)).unwrap(), "the six checks");
    let id = hexfmt::encode(&book::entry_id(g));
    let bound = |i: &Value| zikaron_kit::v2::read::load_view(i, Some(g)).and_then(|v| v.bound(&id));
    assert_eq!(bound(&whole), bound(&quick), "the grant's bound");
    tested
}

fn verdict(input_check: &Value) -> String {
    input_check.member("verdict").and_then(Value::as_str).unwrap_or_default().to_string()
}

fn clean() -> (Vec<Vec<u8>>, Vec<u8>, Vec<Log>) {
    let g0 = genesis(&A);
    let h1 = history(&A, 1, &g0, 1);
    let h2 = history(&A, 2, &h1, 2);
    let h3 = history(&A, 3, &h2, 3);
    let g = grant(&A, 4, &h3);
    let c = Some(Verdict::Counted);
    let logs = vec![log(1, 10, &A, of(&g0), c), log(2, 20, &A, of(&h1), c), log(3, 30, &A, of(&h2), c), log(4, 40, &A, of(&h3), c), log(5, 50, &A, of(&g), c)];
    (vec![g0, h1, h2, h3], g, logs)
}

#[test]
fn a_clean_ledger_is_judged_alike_and_tests_the_grants_log_alone() {
    let (pile, g, logs) = clean();
    let tested = same(&addr(&A), &pile, &g, None, &logs);
    assert_eq!(tested, 1, "the grant's log alone");
    let whole = book::input_of(&addr(&A), &pile, &fragment(&logs, &[true; 5])).unwrap();
    assert_eq!(verdict(&book::grant_check(&g, Some(&whole), None, Some(NOW))), "GREEN");
}

#[test]
fn anchors_that_do_not_count_leave_every_log_of_the_grants_hash_tested() {
    let (pile, g, mut logs) = clean();
    logs[1].verdict = Some(Verdict::Void);
    logs[2].verdict = Some(Verdict::Unproven);
    logs[3].verdict = None;
    logs[4].verdict = Some(Verdict::Void);
    logs.push(log(6, 60, &A, of(&g), None));
    logs.push(log(7, 70, &A, of(&g), Some(Verdict::Counted)));
    logs.push(log(8, 80, &A, of(&g), Some(Verdict::Counted)));
    let tested = same(&addr(&A), &pile, &g, None, &logs);
    assert_eq!(tested, 4, "every log of the grant's hash (the strict reading), none of the history's");
}

#[test]
fn a_handover_is_judged_alike() {
    let g0 = genesis(&A);
    let h1 = history(&A, 1, &g0, 1);
    let s2 = succession(&A, 2, &h1, &B);
    let h3 = history(&B, 3, &s2, 3);
    let g = grant(&B, 4, &h3);
    let c = Some(Verdict::Counted);
    let logs = vec![log(1, 10, &A, of(&g0), c), log(2, 20, &A, of(&h1), c), log(3, 30, &A, of(&s2), c), log(4, 40, &B, of(&h3), c), log(5, 50, &B, of(&g), c)];
    let tested = same(&addr(&A), &[g0, h1, s2, h3], &g, None, &logs);
    assert_eq!(tested, 2, "the succession's log and the grant's");
}

#[test]
fn an_entry_missing_from_the_pile_is_tested_and_judged_alike() {
    let (mut pile, g, logs) = clean();
    pile.remove(2);
    let tested = same(&addr(&A), &pile, &g, None, &logs);
    assert_eq!(tested, 2, "the missing entry's log and the grant's");
    let whole = book::input_of(&addr(&A), &pile, &fragment(&logs, &[true; 5])).unwrap();
    assert_eq!(verdict(&book::grant_check(&g, Some(&whole), None, Some(NOW))), "PARTIAL");
}

#[test]
fn a_grant_no_counted_log_reaches_is_judged_alike() {
    let (pile, g, mut logs) = clean();
    logs[4].verdict = Some(Verdict::Void);
    logs.push(log(6, 60, &A, of(&g), Some(Verdict::Void)));
    let tested = same(&addr(&A), &pile, &g, None, &logs);
    assert_eq!(tested, 2, "both of the grant's logs");
}

#[test]
fn a_later_entry_on_the_grants_line_bounds_it_alike() {
    let (mut pile, g, mut logs) = clean();
    let r = revocation(&A, 5, &g, &g);
    pile.push(g.clone());
    logs[4].verdict = Some(Verdict::Void);
    logs.push(log(6, 60, &A, of(&r), Some(Verdict::Counted)));
    pile.push(r);
    let tested = same(&addr(&A), &pile, &g, None, &logs);
    assert_eq!(tested, 2, "the grant's log and the revocation's");
}

#[test]
fn a_log_that_could_make_a_retired_entry_timely_is_tested() {
    let g0 = genesis(&A);
    let s1 = succession(&A, 1, &g0, &B);
    let h2 = history(&A, 2, &s1, 2);
    let g = grant(&B, 3, &h2);
    let c = Some(Verdict::Counted);
    let logs = vec![log(1, 10, &A, of(&g0), c), log(2, 25, &A, of(&h2), c), log(3, 30, &A, of(&s1), c), log(4, 40, &B, of(&g), c)];
    let tested = same(&addr(&A), &[g0, s1, h2], &g, None, &logs);
    assert_eq!(tested, 3, "the succession's, the grant's, and the log C1 named");
}

#[test]
fn a_log_naming_a_held_entry_outside_the_ledger_is_tested() {
    let (mut pile, g, mut logs) = clean();
    let stranger = history(&X, 1, &genesis(&X), 9);
    logs.push(log(9, 45, &A, of(&stranger), Some(Verdict::Counted)));
    pile.push(stranger);
    let tested = same(&addr(&A), &pile, &g, None, &logs);
    assert_eq!(tested, 2, "the grant's log and the one C2 named");
}

fn notices(contract: &str, accessions: Vec<(u64, &[u8; 32], &[u8; 32])>) -> Value {
    let books: Vec<Value> = accessions
        .iter()
        .enumerate()
        .map(|(i, (block, to, root))| {
            Value::Obj(vec![
                ("blockNumber".into(), Value::Int(*block)),
                ("chainId".into(), Value::Int(CHAIN)),
                ("holder".into(), s(&addr(root))),
                ("logIndex".into(), Value::Int(i as u64)),
                ("root".into(), s(&addr(root))),
                ("to".into(), s(&addr(to))),
                ("txIndex".into(), Value::Int(0)),
            ])
        })
        .collect();
    Value::Obj(vec![
        ("accessions".into(), Value::Arr(books)),
        (
            "basis".into(),
            Value::Obj(vec![
                ("book".into(), s(&format!("0x{}", "3d".repeat(20)))),
                ("bookFromBlock".into(), Value::Int(1)),
                ("chainId".into(), Value::Int(CHAIN)),
                ("fromBlock".into(), Value::Int(1)),
                ("revocationContract".into(), s(contract)),
                ("toBlock".into(), Value::Int(1000)),
            ]),
        ),
        ("revocations".into(), Value::Arr(Vec::new())),
    ])
}

#[test]
fn a_grant_carrying_the_member_tests_its_standing_records_alike() {
    let contract = "0x506e536d4b463f48bb053fe7c3df265bc21b02fe";
    let member = Value::Obj(vec![("at".into(), s(contract)), ("chain".into(), Value::Int(CHAIN))]);
    let g0 = genesis(&A);
    let h1 = history(&A, 1, &g0, 1);
    let g = sealed(Law::Two, &A, "grant", 2, Some(&h1), grant_body(Some(member)));
    let n = notices(contract, vec![(35, &B, &A)]);
    let c = Some(Verdict::Counted);
    let logs = vec![log(1, 10, &A, of(&g0), c), log(2, 20, &A, of(&h1), c), log(3, 40, &A, of(&g), c), log(4, 50, &B, of(&g), c), log(5, 1500, &B, of(&g), c)];
    let tested = same(&addr(&A), &[g0, h1], &g, Some(&n), &logs);
    assert_eq!(tested, 3, "the grant's three logs, none of the history's");
}

#[test]
fn the_old_law_and_a_continued_ledger_are_not_decided() {
    let v1g = sealed(Law::One, &A, "genesis", 0, None, vec![("statement_md", s("works"))]);
    let v1grant = sealed(Law::One, &A, "grant", 1, Some(&v1g), grant_body(None));
    let old = book::decision_set(&Hold { grant: &v1grant, root: &addr(&A), pile: &[v1g.clone()] });
    assert_eq!(old.err(), Some(Undecided::OldLaw));
    let close = sealed_value(Law::One, &A, "succession", 1, Some(&v1g), book::closing_body(&addr(&A), 1, "moved"));
    let close_id = hexfmt::encode(&book::entry_id(&close));
    let body = book::genesis_body("works", Some((&addr(&A), &close_id)));
    let g2 = sealed_value(Law::Two, &A, "genesis", 0, None, body);
    let g = grant(&A, 1, &g2);
    let cont = book::decision_set(&Hold { grant: &g, root: &addr(&A), pile: &[v1g, close, g2] });
    assert_eq!(cont.err(), Some(Undecided::Continued));
}

#[test]
fn a_chain_of_hops_read_in_one_scan_is_judged_alike() {
    let (a_pile, _, _) = clean();
    let up = sealed(Law::Two, &A, "grant", 4, Some(&a_pile[3]), vec![("grantee", s(&addr(&B))), ("terms", s(&hexfmt::encode(&[0x71; 32]))), ("work", s(&hexfmt::encode(&[0x72; 32])))]);
    let b0 = genesis(&B);
    let b1 = history(&B, 1, &b0, 5);
    let down = sealed(Law::Two, &B, "grant", 2, Some(&b1), vec![("grantee", s(&format!("0x{}", "44".repeat(20)))), ("terms", s(&hexfmt::encode(&[0x71; 32]))), ("upstream", s(&hexfmt::encode(&book::entry_id(&up)))), ("work", s(&hexfmt::encode(&[0x72; 32])))]);
    let c = Some(Verdict::Counted);
    let logs = vec![
        log(1, 10, &A, of(&a_pile[0]), c),
        log(2, 20, &A, of(&a_pile[1]), c),
        log(3, 30, &A, of(&a_pile[2]), c),
        log(4, 40, &A, of(&a_pile[3]), c),
        log(5, 50, &A, of(&up), c),
        log(6, 15, &B, of(&b0), c),
        log(7, 25, &B, of(&b1), c),
        log(8, 55, &B, of(&down), c),
    ];
    let b_pile = vec![b0, b1];
    let (ra, rb) = (addr(&A), addr(&B));
    let holds = [Hold { grant: &up, root: &ra, pile: &a_pile }, Hold { grant: &down, root: &rb, pile: &b_pile }];
    let mut ds: Vec<book::Decision> = holds.iter().map(|h| book::decision_set(h).expect("decided")).collect();
    let quick: Vec<Value> = 'read: loop {
        let Pick::Only(choose) = book::pick_of(&ds) else { panic!("a choice") };
        let frag = fragment(&logs, &logs.iter().map(|l| choose(&l.claim.hash)).collect::<Vec<_>>());
        let mut inputs = Vec::new();
        for d in ds.iter_mut() {
            match book::fast_input(d, &frag).expect("input") {
                Fast::Ready(i) => inputs.push(i),
                Fast::More => continue 'read,
            }
        }
        break inputs;
    };
    let all = fragment(&logs, &vec![true; logs.len()]);
    let whole = [book::input_of(&ra, &a_pile, &all).unwrap(), book::input_of(&rb, &b_pile, &all).unwrap()];
    let chain = |inputs: &[Value]| {
        let hops: Vec<(Vec<u8>, Option<Value>, Option<Value>)> = vec![(up.clone(), Some(inputs[0].clone()), None), (down.clone(), Some(inputs[1].clone()), None)];
        String::from_utf8(json::canon_bytes(&book::chain_check(&hops, Some(NOW)))).unwrap()
    };
    assert_eq!(chain(&whole), chain(&quick), "the chain check");
}

#[test]
fn an_entry_anchored_twice_tests_both_logs_and_judges_alike() {
    let (pile, g, mut logs) = clean();
    logs.push(log(6, 60, &A, of(&g), Some(Verdict::Counted)));
    let tested = same(&addr(&A), &pile, &g, None, &logs);
    assert_eq!(tested, 2, "both logs of the grant's hash");
    let (quick, _) = fast(&Hold { grant: &g, root: &addr(&A), pile: &pile }, &logs);
    let records = quick.member("anchors").and_then(Value::as_arr).map(|a| a.iter().filter(|r| r.member("hash").and_then(Value::as_str) == Some(hexfmt::encode(&of(&g)).as_str())).count());
    assert_eq!(records, Some(2), "both carry a record");
}
