use zikaron::hexfmt;
use zikaron::json::Value;
use zikaron_glue::book::{self, Law, Sealed};
use zikaron_glue::grantfile;

const A: [u8; 32] = [0x11; 32];
const B: [u8; 32] = [0x22; 32];

fn addr(k: &[u8; 32]) -> String {
    hexfmt::encode(&zikaron::cryptox::address_of_privkey(k).expect("key"))
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn sealed(key: &[u8; 32], typ: &str, seq: u64, prev: Option<&[u8]>, mut body: Vec<(&str, Value)>) -> Vec<u8> {
    body.sort_by(|a, b| a.0.cmp(b.0));
    let body = Value::Obj(body.into_iter().map(|(k, v)| (k.to_string(), v)).collect());
    let prev = prev.map(|p| hexfmt::encode(&book::entry_id(p)));
    let sign = |pre: &[u8], domain: &str| -> Result<String, ()> {
        let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
        let (r, s, v) = zikaron::cryptox::sign_digest(key, &digest).ok_or(())?;
        let mut sig = r.to_vec();
        sig.extend_from_slice(&s);
        sig.push(v);
        Ok(hexfmt::encode(&sig))
    };
    match book::seal(Law::Two, &addr(key), typ, seq, prev.as_deref(), body, sign) {
        Sealed::Done(e) => e.bytes,
        _ => panic!("sealed {typ}"),
    }
}

fn genesis(key: &[u8; 32]) -> Vec<u8> {
    sealed(key, "genesis", 0, None, vec![("statement_md", s("works"))])
}

fn note(key: &[u8; 32], seq: u64, prev: &[u8]) -> Vec<u8> {
    let mode = Value::Obj(vec![("mark".into(), s("hand")), ("toolchain".into(), s(&hexfmt::encode(&[0x70; 32])))]);
    sealed(key, "history", seq, Some(prev), vec![("content", s(&hexfmt::encode(&[0x61; 32]))), ("mode", mode)])
}

fn grant(key: &[u8; 32], seq: u64, prev: &[u8], grantee: &str, upstream: Option<&[u8]>) -> Vec<u8> {
    let member = Value::Obj(vec![("at".into(), s(&format!("0x{}", "5e".repeat(20)))), ("chain".into(), Value::Int(1))]);
    let mut body = vec![("grantee", s(grantee)), ("revocation", member), ("terms", s(&hexfmt::encode(&[0x71; 32]))), ("work", s(&hexfmt::encode(&[0x72; 32])))];
    if let Some(u) = upstream {
        body.push(("upstream", s(&hexfmt::encode(&book::entry_id(u)))));
    }
    sealed(key, "grant", seq, Some(prev), body)
}

#[test]
fn a_package_carries_the_spine_of_each_member_hops_issuer_from_what_its_exporter_holds() {
    let a_gen = genesis(&A);
    let a_note = note(&A, 1, &a_gen);
    let a_grant = grant(&A, 2, &a_note, &addr(&B), None);
    let b_gen = genesis(&B);
    let b_note = note(&B, 1, &b_gen);
    let b_grant = grant(&B, 2, &b_note, &format!("0x{}", "44".repeat(20)), Some(&a_grant));
    let hops = vec![a_grant.clone(), b_grant.clone()];
    let mine = vec![b_gen.clone(), b_note.clone(), b_grant.clone()];
    let kept = vec![vec![a_grant.clone(), a_gen.clone(), a_note.clone()]];
    let out = grantfile::carried(&hops, &mine, &kept);
    assert!(out.contains(&a_gen), "the upstream issuer's genesis");
    assert!(out.contains(&b_gen), "the exporter's own genesis");
    assert!(!out.contains(&a_note) && !out.contains(&b_note), "no other record of either ledger");
    let held_none = grantfile::carried(&hops, &mine, &[]);
    assert!(!held_none.contains(&a_gen), "nothing the exporter does not hold");
    assert!(held_none.contains(&b_gen));
}
