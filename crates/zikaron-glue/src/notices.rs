use zikaron::json::Value;
use zikaron_kit::v2::doc::{accept, Accepted};
use zikaron_kit::v2::read::{derived_root, load_notices, member_of, Member};

fn read(g: &[u8]) -> Option<(Accepted, Member)> {
    let a = accept(g).ok()?;
    let m = member_of(&a);
    Some((a, m))
}

pub fn carries(g: &[u8]) -> bool {
    read(g).is_some_and(|(_, m)| m.carried)
}

pub fn chain_of(g: &[u8]) -> Option<u64> {
    read(g).and_then(|(_, m)| m.well_formed.map(|(c, _)| c))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Twice;

fn venue_of(v: &Value) -> Option<(u64, String)> {
    load_notices(v)?;
    let b = v.member("basis")?;
    Some((b.member("chainId")?.as_int()?, b.member("revocationContract")?.as_str()?.to_string()))
}

pub fn once_each(inputs: &[Value]) -> Result<(), Twice> {
    let mut seen = std::collections::BTreeSet::new();
    for v in inputs.iter().filter_map(venue_of) {
        if !seen.insert(v) {
            return Err(Twice);
        }
    }
    Ok(())
}

pub fn applies(g: &[u8], n: &Value) -> bool {
    read(g).is_some_and(|(_, m)| m.applies(load_notices(n).as_ref()))
}

pub fn pick(g: &[u8], inputs: &[Value]) -> Option<usize> {
    if inputs.is_empty() {
        return None;
    }
    let applying = read(g).and_then(|(_, m)| inputs.iter().position(|v| m.applies(load_notices(v).as_ref())));
    Some(applying.unwrap_or(0))
}

pub fn root_for(g: &[u8], notices: Option<&Value>) -> Option<String> {
    let (a, m) = read(g)?;
    Some(derived_root(&a, &m, None, notices.and_then(load_notices).as_ref()))
}

pub fn lineage(g: &[u8], root: &str, notices: Option<&Value>) -> Vec<String> {
    let Some((_, m)) = read(g) else { return Vec::new() };
    match notices.and_then(load_notices).filter(|n| m.applies(Some(n))) {
        Some(n) => n.lineage(root),
        None => vec![root.to_string()],
    }
}

pub fn registries_of(chain: u64, registry: &str) -> Vec<String> {
    match zikaron::hexfmt::decode(registry).and_then(|b| <[u8; 20]>::try_from(b).ok()) {
        Some(r) => zikaron_anchor::revoke::registries(chain, &[r]).iter().map(|a| zikaron::hexfmt::encode(a)).collect(),
        None => {
            let mut out = vec![registry.to_ascii_lowercase()];
            out.extend(zikaron_anchor::revoke::at(chain).map(|a| zikaron::hexfmt::encode(&a)));
            out.sort();
            out.dedup();
            out
        }
    }
}

pub fn copy_stands(g: &[u8], copy: &[Vec<u8>], key: Option<&str>) -> bool {
    carries(g) || !crate::book::behind_book(copy, key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::book::{seal, Law, Sealed};

    const KEY: [u8; 32] = [0x11; 32];

    fn author() -> String {
        zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(&KEY).expect("key"))
    }
    const CONTRACT: &str = "0x506e536d4b463f48bb053fe7c3df265bc21b02fe";

    fn grant(member: Option<Value>) -> Vec<u8> {
        let mut body = vec![
            ("grantee".to_string(), Value::Str(format!("0x{}", "44".repeat(20)))),
            ("terms".to_string(), Value::Str(format!("0x{}", "71".repeat(32)))),
            ("work".to_string(), Value::Str(format!("0x{}", "72".repeat(32)))),
        ];
        if let Some(m) = member {
            body.push((zikaron_kit::v2::tokens::Key::Revocation.as_str().to_string(), m));
        }
        body.sort_by(|a, b| a.0.cmp(&b.0));
        let sign = |pre: &[u8], domain: &str| -> Result<String, ()> {
            let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
            let (r, s, v) = zikaron::cryptox::sign_digest(&KEY, &digest).ok_or(())?;
            let mut sig = r.to_vec();
            sig.extend_from_slice(&s);
            sig.push(v);
            Ok(zikaron::hexfmt::encode(&sig))
        };
        let prev = format!("0x{}", "ab".repeat(32));
        match seal(Law::Two, &author(), "grant", 1, Some(&prev), Value::Obj(body), sign) {
            Sealed::Done(e) => e.bytes,
            _ => panic!("sealed"),
        }
    }

    fn member(chain: u64, at: &str) -> Value {
        Value::Obj(vec![("at".into(), Value::Str(at.into())), ("chain".into(), Value::Int(chain))])
    }

    fn input(chain: u64, contract: &str, accessions: Vec<Value>) -> Value {
        Value::Obj(vec![
            ("accessions".into(), Value::Arr(accessions)),
            (
                "basis".into(),
                Value::Obj(vec![
                    ("book".into(), Value::Str(format!("0x{}", "3d".repeat(20)))),
                    ("bookFromBlock".into(), Value::Int(1)),
                    ("chainId".into(), Value::Int(chain)),
                    ("fromBlock".into(), Value::Int(1)),
                    ("revocationContract".into(), Value::Str(contract.into())),
                    ("toBlock".into(), Value::Int(10)),
                ]),
            ),
            ("revocations".into(), Value::Arr(Vec::new())),
        ])
    }

    #[test]
    fn each_hop_takes_the_input_that_applies_to_it() {
        let a = input(11_155_111, CONTRACT, Vec::new());
        let b = input(10, CONTRACT, Vec::new());
        let on_b = grant(Some(member(10, CONTRACT)));
        assert_eq!(pick(&on_b, &[a.clone(), b.clone()]), Some(1), "the one that applies");
        assert_eq!(pick(&on_b, &[a.clone()]), Some(0), "one input is every hop's");
        assert_eq!(pick(&grant(None), &[b.clone(), a.clone()]), Some(0), "none applying: the first");
        assert_eq!(pick(&on_b, &[]), None);
        assert_eq!(once_each(&[a.clone(), b.clone()]), Ok(()));
        assert_eq!(once_each(&[a.clone(), a.clone()]), Err(Twice), "one venue twice");
        assert_eq!(once_each(&[a, Value::Null, Value::Null]), Ok(()), "a value that is no notice input names no venue");
    }

    #[test]
    fn the_root_and_senders_come_from_the_book_where_it_applies() {
        let g = grant(Some(member(11_155_111, CONTRACT)));
        let author = author();
        let root = format!("0x{}", "99".repeat(20));
        let acc = Value::Obj(vec![
            ("blockNumber".into(), Value::Int(3)),
            ("chainId".into(), Value::Int(11_155_111)),
            ("holder".into(), Value::Str(root.clone())),
            ("logIndex".into(), Value::Int(0)),
            ("root".into(), Value::Str(root.clone())),
            ("to".into(), Value::Str(author.clone())),
            ("txIndex".into(), Value::Int(0)),
        ]);
        let n = input(11_155_111, CONTRACT, vec![acc]);
        assert_eq!(root_for(&g, Some(&n)).as_deref(), Some(root.as_str()));
        assert_eq!(lineage(&g, &root, Some(&n)), vec![root.clone(), author.clone()]);
        assert_eq!(root_for(&g, None).as_deref(), Some(author.as_str()), "no input: the author");
        let elsewhere = input(10, CONTRACT, Vec::new());
        assert_eq!(lineage(&g, &author, Some(&elsewhere)), vec![author.clone()], "an input that does not apply adds no key");
        assert!(carries(&g) && !carries(&grant(None)));
        assert_eq!(chain_of(&g), Some(11_155_111));
        assert_eq!(chain_of(&grant(Some(Value::Null))), None, "a member not of its form names no chain");
        assert!(copy_stands(&g, &[b"x".to_vec()], Some("0x01")), "a member grant's copy stands behind the book");
    }

    #[test]
    fn the_registries_hold_the_revocation_contract_where_it_stands() {
        assert_eq!(registries_of(11_155_111, "0xAB00000000000000000000000000000000000000"), vec![CONTRACT.to_string(), "0xab00000000000000000000000000000000000000".to_string()], "ascending");
        assert_eq!(registries_of(11_155_111, "0x0100000000000000000000000000000000000000"), vec!["0x0100000000000000000000000000000000000000".to_string(), CONTRACT.to_string()]);
        assert_eq!(registries_of(11_155_111, CONTRACT), vec![CONTRACT.to_string()]);
        assert_eq!(registries_of(31_337, "0xab00000000000000000000000000000000000000"), vec!["0xab00000000000000000000000000000000000000".to_string()]);
    }
}
