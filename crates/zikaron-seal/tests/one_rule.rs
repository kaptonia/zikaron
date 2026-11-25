use zikaron::json::Value;
use zikaron_glue::book::{self, Law};
use zikaron_seal::queue::{Queue, Step};

const A: [u8; 32] = [0x11; 32];

fn addr(k: &[u8; 32]) -> String {
    zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(k).expect("an address"))
}

fn obj(ms: &[(&str, Value)]) -> Value {
    Value::Obj(ms.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn h32(b: u8) -> String {
    format!("0x{}", format!("{b:02x}").repeat(32))
}

fn entry(kind: &str, seq: u64, prev: Option<&[u8]>, body: Value) -> Vec<u8> {
    let prev = prev.map(|p| zikaron::hexfmt::encode(&book::entry_id(p)));
    let sealed = book::seal::<()>(Law::Two, &addr(&A), kind, seq, prev.as_deref(), body, |pre, domain| {
        let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
        let (r, s, v) = zikaron::cryptox::sign_digest(&A, &digest).ok_or(())?;
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

fn id(b: &[u8]) -> String {
    zikaron::hexfmt::encode(&book::entry_id(b))
}

fn history(seq: u64, prev: &[u8], content: &str) -> Vec<u8> {
    entry("history", seq, Some(prev), obj(&[("content", s(content)), ("mode", obj(&[("mark", s("v1")), ("toolchain", s(&h32(0x0b)))]))]))
}

fn grant(seq: u64, prev: &[u8], work: &str, window: (u64, u64)) -> Vec<u8> {
    entry("grant", seq, Some(prev), obj(&[("grantee", s(&format!("0x{}", "33".repeat(20)))), ("terms", s(&h32(0x55))), ("window", obj(&[("from", Value::Int(window.0)), ("to", Value::Int(window.1))])), ("work", s(work))]))
}

#[test]
fn what_a_delete_does_to_the_queue_is_one_rule() {
    let (r, d) = (h32(1), h32(2));
    let mut q = Queue { chain: 7, ..Queue::default() };
    q.push(&r, 1);
    let held = q.retracting(&r, None, true).expect("the queue knows it");
    assert!(!held.publish && held.drop, "queued, not published: both stay here, the record leaves the queue");
    q.push(&d, 2);
    assert_eq!(q.retracted(&r, &d, held), 1, "the record left");
    assert!(!q.has(&r) && !q.has(&d), "and the delete the road queued went too");
    assert!(Queue::default().retracting(&r, None, true).is_none(), "nobody can tell: refused");
    assert!(Queue::default().retracting(&r, None, false).is_some(), "a home that never anchors keeps it here");
    let last = vec![r.clone()];
    assert!(Queue::default().retracting(&r, Some(&last), true).expect("told").publish, "anchored per the last audit: published");
    let mut out = Queue { chain: 7, ..Queue::default() };
    out.push(&r, 1);
    out.mark(&[r.clone()], Step::Submitted { tx: h32(9), chain: 7, nonce: Some(1) });
    let flying = out.retracting(&r, None, true).expect("told");
    assert!(flying.publish && !flying.drop, "in flight: published, and left to finish");
}

#[test]
fn a_grant_is_withheld_by_one_gate() {
    use zikaron_seal::terms::{may_grant, Record, Withheld};
    let g = entry("genesis", 0, None, obj(&[("statement_md", s("开端"))]));
    let work = h32(0xc1);
    let h = history(1, &g, &work);
    let first = grant(2, &h, &work, (100, 200));
    let pile = vec![g.clone(), h.clone(), first.clone()];
    let exclusive = vec![Record { grant: id(&first), terms: h32(0x55), exclusive: true, doc: None, name: None }];
    assert_eq!(may_grant(&pile, &exclusive, &work, Some((150, 300)), None), Err(Withheld::Clash(vec![id(&first)])), "overlapping an exclusive grant");
    assert_eq!(may_grant(&pile, &exclusive, &work, Some((201, 300)), None), Ok(()), "apart");
    assert_eq!(may_grant(&pile, &[], &work, Some((150, 300)), None), Ok(()), "not exclusive (a plain folder keeps no record)");
    assert_eq!(may_grant(&pile, &exclusive, &work, Some((100, 200)), Some(&id(&first).to_ascii_uppercase())), Ok(()), "standing in for it");
    assert_eq!(may_grant(&pile, &exclusive, &work, Some((100, 200)), Some(&h32(0x77))), Err(Withheld::Clash(vec![id(&first)])), "standing in for another");
    let gone = entry("retraction", 3, Some(&first), zikaron_glue::retraction::body(&id(&h), ""));
    assert_eq!(may_grant(&[g, h, first, gone], &[], &work, None, None), Err(Withheld::Deleted), "every record of the work deleted");
}

#[test]
fn a_grant_package_carries_its_home_by_one_assembly() {
    let g = entry("genesis", 0, None, obj(&[("statement_md", s("开端"))]));
    let work = h32(0xc1);
    let h = history(1, &g, &work);
    let first = grant(2, &h, &work, (100, 200));
    let pile = vec![g.clone(), h, first.clone()];
    let ground = zikaron_seal::ground::Ground { chain_id: Some(7), registry: Some([0x11; 20]), from_block: 3, publish: Some("https://example.org/p/".into()), ..Default::default() };
    let last = zikaron_seal::lastaudit::Anchored { rows: vec![(id(&g), (7, h32(9)))], at: 1, whole: true };
    let terms = |hid: &str| -> Result<Option<(String, Vec<u8>)>, ()> { Ok((hid == id(&first)).then(|| ("terms/t.txt".to_string(), b"t".to_vec()))) };
    let p = zikaron_seal::terms::package_parts(&[first.clone()], &pile, &pile, &[], "code".into(), terms, &ground, Some(&last)).expect("parts");
    assert_eq!(p.publish.as_deref(), Some("https://example.org/p/"), "the publish address");
    assert_eq!(p.terms.len(), 1, "the kept terms");
    assert_eq!(p.anchors.len(), 1, "the last self-audit's anchors");
    assert_eq!(p.anchored_on.as_ref().map(|a| (a.chain_id, a.from_block)), Some((7, 3)), "where it anchors");
}

#[test]
fn the_original_is_taken_by_one_rule() {
    let dir = std::env::temp_dir().join(format!("zk-take-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("dir");
    let bytes = b"the work".to_vec();
    let work = zikaron::hexfmt::encode(&zikaron_glue::recording::content_of(&bytes));
    let g = entry("genesis", 0, None, obj(&[("statement_md", s("开端"))]));
    let h = history(1, &g, &work);
    let root = id(&g);
    let (gone, changed, here) = (dir.join("gone.txt"), dir.join("changed.txt"), dir.join("here.txt"));
    std::fs::write(&changed, b"other").expect("write");
    std::fs::write(&here, &bytes).expect("write");
    let row = |p: &std::path::Path, seq: u64| zikaron_seal::records::Row { root: root.clone(), content: work.clone(), id: id(&h), seq, name: "x".into(), path: p.display().to_string() };
    let rows = vec![row(&gone, 1), row(&changed, 2), row(&here, 3)];
    let t = zikaron_seal::records::take(&rows, &[g, h], &work);
    assert!(t.original.is_some(), "taken");
    assert_eq!(t.place.as_deref(), Some(here.display().to_string().as_str()));
    assert_eq!(t.left_out, vec![gone.display().to_string(), changed.display().to_string()], "gone, and no longer its bytes");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_plain_export_is_listed_and_the_index_read_by_one_rule() {
    use zikaron_seal::exports::{index_at, listed, Kind};
    let dir = std::env::temp_dir().join(format!("zk-listed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(zikaron_seal::exports::DIR)).expect("dir");
    let p = dir.join("pack.zkgrant");
    std::fs::write(&p, b"one").expect("write");
    let rows = listed(&[], Kind::GrantPackage, &p, 1).expect("listed");
    std::fs::write(&p, b"two").expect("write");
    let rows = listed(&rows, Kind::GrantPackage, &p, 2).expect("listed again");
    assert_eq!(rows.len(), 1, "one row per place");
    assert!(listed(&rows, Kind::Kit, &dir.join("none"), 3).is_none(), "nothing there");
    assert_eq!(index_at(&dir).expect("none written"), None);
    std::fs::create_dir_all(dir.join(zikaron_seal::exports::DIR).join(zikaron_seal::exports::INDEX)).expect("a folder where the index goes");
    assert!(index_at(&dir).is_err(), "an index that does not read is refused");
    let _ = std::fs::remove_dir_all(&dir);
}
