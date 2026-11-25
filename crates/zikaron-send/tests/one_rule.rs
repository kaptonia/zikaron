use zikaron::json::Value;
use zikaron_glue::book::{self, Law};

const A: [u8; 32] = [0x11; 32];
const S: [u8; 32] = [0x33; 32];

fn addr(k: &[u8; 32]) -> String {
    zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(k).expect("an address"))
}

fn st(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn genesis(key: &[u8; 32]) -> Vec<u8> {
    genesis_under(Law::Two, key)
}

fn old_entry(key: &[u8; 32]) -> Vec<u8> {
    genesis_under(Law::One, key)
}

fn genesis_under(law: Law, key: &[u8; 32]) -> Vec<u8> {
    let sealed = book::seal::<()>(law, &addr(key), "genesis", 0, None, Value::Obj(vec![("statement_md".into(), st("开端"))]), |pre, domain| {
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

fn record(hash: &str, block: u64, sender: &str) -> Value {
    Value::Obj(vec![
        ("blockNumber".into(), Value::Int(block)),
        ("blockTimestamp".into(), Value::Int(100 + block)),
        ("chainId".into(), Value::Int(31337)),
        ("hash".into(), st(hash)),
        ("sender".into(), st(sender)),
        ("tx".into(), st(&format!("0x{block:064x}"))),
        ("verdict".into(), st("counted")),
    ])
}

fn fragment(records: Vec<Value>, senders: &[String]) -> Value {
    let chain = Value::Obj(vec![
        ("chainId".into(), Value::Int(31337)),
        ("fromBlock".into(), Value::Int(0)),
        ("registries".into(), Value::Arr(vec![st(&format!("0x{}", "11".repeat(20)))])),
        ("senders".into(), Value::Arr(senders.iter().map(|x| st(x)).collect())),
        ("toBlock".into(), Value::Int(1000)),
    ]);
    Value::Obj(vec![("anchors".into(), Value::Arr(records)), ("basis".into(), Value::Obj(vec![("chains".into(), Value::Arr(vec![chain]))]))])
}

fn bare(x: &str) -> String {
    x.trim_start_matches("0x").to_ascii_lowercase()
}

#[test]
fn the_gate_counts_missing_as_the_audit_does() {
    let g = genesis(&A);
    let mine = book::entry_id(&g);
    let lost = format!("0x{}", "77".repeat(32));
    let stray = format!("0x{}", "88".repeat(32));
    let mut senders = vec![addr(&A), addr(&S)];
    senders.sort();
    let frag = fragment(vec![record(&zikaron::hexfmt::encode(&mine), 1, &addr(&A)), record(&lost, 2, &addr(&A)), record(&stray, 3, &addr(&S))], &senders);
    let mut anchored = vec![bare(&zikaron::hexfmt::encode(&mine)), bare(&lost), bare(&stray)];
    anchored.sort();
    let verified = vec![bare(&zikaron::hexfmt::encode(&mine))];
    let missing = zikaron_send::gate::missing_of(&[g.clone()], &frag, &[], &anchored, &verified);
    assert_eq!(missing, vec![bare(&lost)], "the lineage's anchor without an entry, never the stranger's");
    let own = zikaron_send::gate::missing_of(&[g.clone()], &frag, &[&addr(&S)], &anchored, &verified);
    assert_eq!(own, vec![bare(&lost), bare(&stray)], "the identity's own key outside the lineage counts");
    let moved = zikaron_send::gate::missing_of(&[g], &frag, &[&addr(&A), &addr(&S)], &anchored, &verified);
    assert_eq!(moved, vec![bare(&lost), bare(&stray)], "the book's key outside the lineage counts");
    assert_eq!(zikaron_send::gate::missing_of(&[], &frag, &[&addr(&A)], &anchored, &[]), anchored, "no ledger yet: every anchor read");
}

#[test]
fn an_anchor_whose_bytes_are_here_is_not_missing() {
    let g = genesis(&A);
    let late = old_entry(&A);
    let (late_id, lost) = (zikaron::hexfmt::encode(&book::entry_id(&late)), format!("0x{}", "77".repeat(32)));
    let frag = fragment(vec![record(&late_id, 1, &addr(&A)), record(&lost, 2, &addr(&A))], &[addr(&A)]);
    let mut anchored = vec![bare(&late_id), bare(&lost)];
    anchored.sort();
    let read = book::read(&[g.clone(), late.clone()], &frag).expect("reads");
    assert!(read.rows("missing").iter().any(|r| r.member("hash").and_then(Value::as_str) == Some(late_id.as_str())), "the audit still lists it");
    assert_eq!(zikaron_send::gate::missing_of(&[g.clone(), late], &frag, &[], &anchored, &[]), vec![bare(&lost)], "held: not missing; not held: missing");
    assert_eq!(zikaron_send::gate::missing_of(&[g], &frag, &[], &anchored, &[]), anchored, "without its bytes it is missing");
}

#[test]
fn the_gate_asks_every_chain_of_the_basis() {
    use zikaron_seal::ground::{Earlier, Ground};
    let ground = Ground { chain_id: Some(2), registry: Some([0x22; 20]), from_block: 50, earlier: vec![Earlier { chain_id: 1, registry: [0x11; 20], from_block: 5 }], ..Default::default() };
    let eps = vec![zikaron_send::node::Endpoint::at(2, "http://127.0.0.1:1")];
    let ask = zikaron_send::gate::ask("/h".into(), &ground, &eps, None).expect("asked");
    let lanes: Vec<(u64, [u8; 20], u64, usize)> = ask.lanes.iter().map(|l| (l.chain, l.registry, l.from_block, l.eps.len())).collect();
    assert_eq!(lanes, vec![(2, [0x22; 20], 50, 1), (1, [0x11; 20], 5, 0)]);
    assert_eq!((ask.from_block_of(1), ask.from_block_of(2)), (Some(5), Some(50)), "a batch's chain is asked from that chain's start");
    assert_eq!(ask.from_block_of(9), None, "a chain the basis does not name has no start, never another chain's");
    assert!(zikaron_send::gate::ask("/h".into(), &ground, &[], None).is_err(), "no node of the anchoring chain");
}

#[test]
fn a_held_ledger_names_its_key_in_office() {
    let root = zikaron::cryptox::address_of_privkey(&A).expect("root");
    let other = [0x44; 20];
    assert_eq!(zikaron_send::recipient::held(&root, &[(root, other)], &[]), Some(other), "a seat's key in office");
    assert_eq!(zikaron_send::recipient::held(&root, &[], &[vec![genesis(&A)]]), Some(root), "a kept ledger's");
    assert_eq!(zikaron_send::recipient::held(&other, &[], &[vec![genesis(&A)]]), None, "none held");
}

#[test]
fn an_anchor_of_the_identitys_key_outside_the_lineage_is_missing_for_the_gate_alone() {
    let g = genesis(&A);
    let mine = book::entry_id(&g);
    let stray = format!("0x{}", "88".repeat(32));
    let mut senders = vec![addr(&A), addr(&S)];
    senders.sort();
    let frag = fragment(vec![record(&zikaron::hexfmt::encode(&mine), 1, &addr(&A)), record(&stray, 3, &addr(&S))], &senders);
    let read = book::read(&[g.clone()], &frag).expect("reads");
    assert!(read.rows("missing").is_empty() && read.rows("unproven").is_empty(), "the ledger reading misses nothing");
    let mut anchored = vec![bare(&zikaron::hexfmt::encode(&mine)), bare(&stray)];
    anchored.sort();
    let verified = vec![bare(&zikaron::hexfmt::encode(&mine))];
    assert!(zikaron_send::gate::missing_of(&[g.clone()], &frag, &[], &anchored, &verified).is_empty(), "no owner outside: nothing lacked");
    assert_eq!(zikaron_send::gate::missing_of(&[g], &frag, &[&addr(&S)], &anchored, &verified), vec![bare(&stray)], "the gate names it");
}
