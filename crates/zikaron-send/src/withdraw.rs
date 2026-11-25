use crate::node::{Endpoint, NodeAddr};
use crate::{one, Code, Trouble};
use zikaron::json::Value;

#[derive(Clone, Debug, PartialEq)]
pub enum Back {
    Counted,
    Unproven,
    NotSeen,
    Unread(String),
}

impl Back {
    pub fn code(&self) -> &'static str {
        match self {
            Back::Counted => "counted",
            Back::Unproven => "unproven",
            Back::NotSeen => "not-seen",
            Back::Unread(_) => "unread",
        }
    }
}

pub const NOT_INCLUDED: &str = "not-included";

#[derive(Clone, Debug, PartialEq)]
pub struct Noticed {
    pub fell: one::Fell,
    pub back: Back,
}

pub fn call_of(chain: u64, entry: &[u8; 32], grant: &[u8; 32]) -> Option<one::Call> {
    Some(one::Call::notice(chain, zikaron_anchor::revoke::at(chain)?, entry, grant))
}

pub fn notice(through: one::Through, eps: &[Endpoint], urls: &[NodeAddr], chain: u64, g: &[u8], entry: &[u8; 32], signer: &dyn one::PlainSigner, wait: std::time::Duration) -> Result<Noticed, Trouble> {
    let grant = zikaron::cryptox::sha256(g);
    let from = signer.from().ok_or_else(|| Trouble::text(Code::KeyMalformed, ""))?;
    let mut call = call_of(chain, entry, &grant).ok_or_else(|| Trouble::text(Code::NoNoticeVenue, chain.to_string()))?;
    let mine: Vec<Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
    if let Ok(r) = crate::notices::read(&mine, chain) {
        if let Some(stood) = stood(g, &r.value) {
            return Ok(stood);
        }
        let applies = crate::issue::member_of(g).is_some_and(|m| m.chain == chain && m.at == call.to);
        if applies && judged(g, &r.value) == Back::Unproven {
            call.again = one::Again::Another;
        }
    }
    let e = crate::batch::estimate(&crate::batch::GasAsk { chain, call: zikaron_anchor::send::estimate_call(&from, &call.to, &call.data), data: call.data.clone(), eps: mine.clone() })?;
    let fell = one::send(through, urls, &call, signer, e.fees, wait)?;
    Ok(Noticed { back: back(&mine, chain, g), fell })
}

pub fn already(eps: &[Endpoint], chain: u64, g: &[u8]) -> Option<Noticed> {
    let n = crate::notices::read(eps, chain).ok()?.value;
    stood(g, &n)
}

fn stood(g: &[u8], n: &Value) -> Option<Noticed> {
    let (tx, block) = counted_in(g, n)?;
    Some(Noticed { fell: one::Fell { tx: zikaron::hexfmt::encode(&tx), block, status: 1, gas_used: None }, back: Back::Counted })
}

pub fn counted_in(g: &[u8], n: &Value) -> Option<([u8; 32], u64)> {
    let (block, tx_index, log_index) = zikaron_glue::book::withdrawing_notice(g, n)?;
    let at = |r: &Value, k: &str| r.member(k).and_then(Value::as_int);
    let rec = n.member(zikaron_anchor::scan::REVOCATIONS).and_then(Value::as_arr)?.iter().find(|r| at(r, "blockNumber") == Some(block) && at(r, "txIndex") == Some(tx_index) && at(r, "logIndex") == Some(log_index))?;
    let tx: [u8; 32] = rec.member("tx").and_then(Value::as_str).and_then(zikaron::hexfmt::decode)?.try_into().ok()?;
    Some((tx, block))
}

pub fn unrecorded(eps: &[Endpoint], pile: &[Vec<u8>], noticed: &[crate::checked::Notice]) -> Vec<crate::checked::Notice> {
    unrecorded_said(eps, pile, noticed).0
}

pub fn unrecorded_said(eps: &[Endpoint], pile: &[Vec<u8>], noticed: &[crate::checked::Notice]) -> (Vec<crate::checked::Notice>, Vec<u64>) {
    let mut wanted: Vec<(u64, [u8; 32], Vec<u8>)> = Vec::new();
    for e in pile.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).filter(|e| e.kind == zikaron_glue::book::Kind::Revocation) {
        let Some(named) = e.body.member("grant").and_then(Value::as_str) else { continue };
        let Some(p) = pair(pile, named) else { continue };
        let id = zikaron::cryptox::sha256(&p.grant);
        if noticed.iter().any(|n| n.grant == id && n.chain == p.member.chain) || wanted.iter().any(|w| w.1 == id) {
            continue;
        }
        wanted.push((p.member.chain, id, p.grant));
    }
    let mut out = Vec::new();
    let mut unread = Vec::new();
    let mut chains: Vec<u64> = wanted.iter().map(|w| w.0).collect();
    chains.sort_unstable();
    chains.dedup();
    for chain in chains {
        let mine: Vec<Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
        let Ok(r) = crate::notices::read(&mine, chain) else {
            unread.push(chain);
            continue;
        };
        for (_, id, g) in wanted.iter().filter(|w| w.0 == chain) {
            if let Some((tx, block)) = counted_in(g, &r.value) {
                out.push(crate::checked::Notice { grant: *id, chain, tx, block });
            }
        }
    }
    (out, unread)
}

pub fn read_back<E>(eps: &[Endpoint], pile: &[Vec<u8>], waiting: &[String], noticed: &[crate::checked::Notice], mut keep: impl FnMut(crate::checked::Notice) -> Result<bool, E>) -> Result<Vec<u64>, E> {
    let mut chains: Vec<u64> = waiting.iter().filter_map(|g| member_chain(pile, g)).collect();
    chains.sort_unstable();
    chains.dedup();
    let mut unread = Vec::new();
    for chain in chains {
        let mine: Vec<Endpoint> = eps.iter().filter(|e| e.chain == chain).cloned().collect();
        if mine.is_empty() {
            unread.push(chain);
            continue;
        }
        let (found, not) = unrecorded_said(&mine, &on_chain(pile, chain), noticed);
        unread.extend(not);
        for n in found {
            keep(n)?;
        }
    }
    unread.sort_unstable();
    unread.dedup();
    Ok(unread)
}

fn on_chain(pile: &[Vec<u8>], chain: u64) -> Vec<Vec<u8>> {
    pile.iter()
        .filter(|b| match zikaron_glue::book::check(b) {
            Ok(e) if e.kind == zikaron_glue::book::Kind::Revocation => e.body.member("grant").and_then(Value::as_str).and_then(|g| pair(pile, g)).is_none_or(|p| p.member.chain == chain),
            _ => true,
        })
        .cloned()
        .collect()
}

pub fn undecided(w: zikaron_seal::terms::Withheld, pile: &[Vec<u8>], unread: &[u64]) -> zikaron_seal::terms::Withheld {
    let zikaron_seal::terms::Withheld::Clash(ids) = &w else { return w };
    let waits = |g: &String| written(pile, g).is_some() && member_chain(pile, g).is_some_and(|c| unread.contains(&c));
    if !ids.is_empty() && ids.iter().all(waits) {
        return zikaron_seal::terms::Withheld::Undecided(ids.clone());
    }
    w
}

pub fn unsettled(pile: &[Vec<u8>], clash: &[String]) -> Vec<String> {
    clash.iter().filter(|id| written(pile, id).is_some()).cloned().collect()
}

pub fn member_chain(pile: &[Vec<u8>], grant: &str) -> Option<u64> {
    pair(pile, grant).map(|p| p.member.chain)
}

pub fn back(eps: &[Endpoint], chain: u64, g: &[u8]) -> Back {
    match crate::notices::read(eps, chain) {
        Ok(r) => judged(g, &r.value),
        Err(u) => unread(&u),
    }
}

fn unread(u: &crate::notices::Unread) -> Back {
    Back::Unread(match u {
        crate::notices::Unread::Trouble(t) => format!("{} {}", u.code(), t.code.as_str()),
        other => other.code().to_string(),
    })
}

pub fn judged(g: &[u8], n: &Value) -> Back {
    let r = zikaron_glue::book::grant_check(g, None, Some(n), None);
    let state = r.member("checks").and_then(Value::as_arr).and_then(|c| c.get(5)).and_then(|c| c.member("state")).and_then(Value::as_str).unwrap_or_default().to_string();
    match state.as_str() {
        "FAIL" => Back::Counted,
        "UNKNOWN" => Back::Unproven,
        _ => Back::NotSeen,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pair {
    pub member: crate::issue::Member,
    pub grant: Vec<u8>,
    pub old: Option<String>,
}

pub fn pair(pile: &[Vec<u8>], grant: &str) -> Option<Pair> {
    let want = grant.trim().to_ascii_lowercase();
    let id = |b: &[u8]| zikaron::hexfmt::encode(&zikaron::cryptox::sha256(b));
    let sup = |b: &[u8]| zikaron_glue::book::check(b).ok().and_then(|e| e.body.member("supersedes").and_then(Value::as_str).map(str::to_ascii_lowercase));
    let g = pile.iter().find(|b| id(b) == want)?;
    if let Some(member) = crate::issue::member_of(g) {
        return Some(Pair { member, grant: g.clone(), old: sup(g) });
    }
    pile.iter().find_map(|b| {
        let member = crate::issue::member_of(b)?;
        (sup(b).as_deref() == Some(want.as_str())).then(|| Pair { member, grant: b.clone(), old: Some(want.clone()) })
    })
}

pub fn written(pile: &[Vec<u8>], grant: &str) -> Option<String> {
    pile.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).find(|e| e.kind == zikaron_glue::book::Kind::Revocation && e.body.member("grant").and_then(Value::as_str).is_some_and(|g| g.eq_ignore_ascii_case(grant.trim()))).map(|e| zikaron::hexfmt::encode(&e.id))
}

pub fn standing(pile: &[Vec<u8>], noticed: &[crate::checked::Notice]) -> Vec<Vec<u8>> {
    let entries: Vec<(Vec<u8>, Option<zikaron_glue::book::Entry>)> = pile.iter().map(|b| (b.clone(), zikaron_glue::book::check(b).ok())).collect();
    let id_of = |e: &zikaron_glue::book::Entry| zikaron::hexfmt::encode(&e.id);
    let mut unread: Vec<String> = Vec::new();
    for (b, e) in &entries {
        let Some(e) = e else { continue };
        if e.kind != zikaron_glue::book::Kind::Grant {
            continue;
        }
        let Some(m) = crate::issue::member_of(b) else { continue };
        let read = noticed.iter().any(|n| n.grant == e.id && n.chain == m.chain);
        if !read {
            unread.push(id_of(e));
            if let Some(old) = e.body.member("supersedes").and_then(Value::as_str) {
                unread.push(old.to_ascii_lowercase());
            }
        }
    }
    entries
        .into_iter()
        .filter(|(_, e)| match e {
            Some(e) if e.kind == zikaron_glue::book::Kind::Revocation => !e.body.member("grant").and_then(Value::as_str).is_some_and(|g| unread.contains(&g.to_ascii_lowercase())),
            _ => true,
        })
        .map(|(b, _)| b)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use zikaron_glue::book::{seal, Law, Sealed};

    const KEY: [u8; 32] = [0x11; 32];
    const CONTRACT: &str = "0x506e536d4b463f48bb053fe7c3df265bc21b02fe";

    fn sealed(kind: &str, seq: u64, mut body: Vec<(String, Value)>) -> Vec<u8> {
        body.sort_by(|a, b| a.0.cmp(&b.0));
        let author = zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(&KEY).expect("key"));
        let sign = |pre: &[u8], domain: &str| -> Result<String, ()> {
            let (_, digest) = zikaron::entry::presig_and_digest(pre, domain);
            let (r, s, v) = zikaron::cryptox::sign_digest(&KEY, &digest).ok_or(())?;
            let mut sig = r.to_vec();
            sig.extend_from_slice(&s);
            sig.push(v);
            Ok(zikaron::hexfmt::encode(&sig))
        };
        let prev = format!("0x{}", format!("{seq:02x}").repeat(32));
        match seal(Law::Two, &author, kind, seq, Some(&prev), Value::Obj(body), sign) {
            Sealed::Done(e) => e.bytes,
            _ => panic!("sealed"),
        }
    }

    fn grant(seq: u64, member: bool, supersedes: Option<&str>) -> Vec<u8> {
        let mut body = vec![
            ("grantee".to_string(), Value::Str(format!("0x{}", "44".repeat(20)))),
            ("terms".to_string(), Value::Str(format!("0x{}", "71".repeat(32)))),
            ("work".to_string(), Value::Str(format!("0x{}", "72".repeat(32)))),
        ];
        if member {
            body.push(("revocation".to_string(), Value::Obj(vec![("at".into(), Value::Str(CONTRACT.into())), ("chain".into(), Value::Int(11_155_111))])));
        }
        if let Some(s) = supersedes {
            body.push(("supersedes".to_string(), Value::Str(s.to_string())));
        }
        sealed("grant", seq, body)
    }

    fn revocation(seq: u64, of: &[u8]) -> Vec<u8> {
        sealed("revocation", seq, vec![("grant".to_string(), Value::Str(zikaron::hexfmt::encode(&zikaron::cryptox::sha256(of))))])
    }

    #[test]
    fn standing_leaves_out_what_is_not_read_back() {
        let old = grant(1, false, None);
        let old_id = zikaron::hexfmt::encode(&zikaron::cryptox::sha256(&old));
        let new = grant(2, true, Some(&old_id));
        let plain = grant(3, false, None);
        let (r_old, r_new, r_plain) = (revocation(4, &old), revocation(5, &new), revocation(6, &plain));
        let pile = vec![old.clone(), new.clone(), plain.clone(), r_old.clone(), r_new.clone(), r_plain.clone()];
        let none = standing(&pile, &[]);
        assert!(!none.contains(&r_old) && !none.contains(&r_new), "not read back: neither of the pair stands");
        assert!(none.contains(&r_plain) && none.contains(&new) && none.contains(&old), "a plain grant's revocation and every grant stand");
        let read = crate::checked::Notice { grant: zikaron::cryptox::sha256(&new), chain: 11_155_111, tx: [7; 32], block: 9 };
        assert_eq!(standing(&pile, &[read]), pile, "read back: the pair stands");
        let elsewhere = crate::checked::Notice { chain: 1, ..read };
        assert!(!standing(&pile, &[elsewhere]).contains(&r_new), "a notice read on another chain is not the member's");
    }
    fn notices(grant: &[u8], rows: &[(u64, String, &str, u8)]) -> Value {
        let id = zikaron::hexfmt::encode(&zikaron::cryptox::sha256(grant));
        let int = Value::Int;
        let recs = rows
            .iter()
            .map(|(block, sender, verdict, tx)| {
                Value::Obj(vec![
                    ("blockNumber".into(), int(*block)),
                    ("chainId".into(), int(11_155_111)),
                    ("grant".into(), Value::Str(id.clone())),
                    ("logIndex".into(), int(0)),
                    ("sender".into(), Value::Str(sender.clone())),
                    ("tx".into(), Value::Str(format!("0x{}", format!("{tx:02x}").repeat(32)))),
                    ("txIndex".into(), int(0)),
                    ("verdict".into(), Value::Str((*verdict).into())),
                ])
            })
            .collect();
        let basis = Value::Obj(vec![
            ("book".into(), Value::Str(format!("0x{}", "b0".repeat(20)))),
            ("bookFromBlock".into(), int(0)),
            ("chainId".into(), int(11_155_111)),
            ("fromBlock".into(), int(0)),
            ("revocationContract".into(), Value::Str(CONTRACT.into())),
            ("toBlock".into(), int(100)),
        ]);
        Value::Obj(vec![("accessions".into(), Value::Arr(Vec::new())), ("basis".into(), basis), ("revocations".into(), Value::Arr(recs))])
    }

    #[test]
    fn the_withdrawal_is_the_counted_notice_of_the_key_in_office() {
        let g = grant(1, true, None);
        let issuer = zikaron::hexfmt::encode(&zikaron::cryptox::address_of_privkey(&KEY).expect("key"));
        let stranger = format!("0x{}", "99".repeat(20));
        let n = notices(&g, &[(10, stranger.clone(), "counted", 0xaa), (11, issuer.clone(), "VOID", 0xbb), (12, issuer.clone(), "counted", 0xcc)]);
        assert_eq!(counted_in(&g, &n), Some(([0xcc; 32], 12)), "the issuer's counted notice");
        assert_eq!(judged(&g, &n), Back::Counted);
        let n = notices(&g, &[(10, stranger, "counted", 0xaa), (11, issuer, "VOID", 0xbb)]);
        assert_eq!(counted_in(&g, &n), None, "a stranger's and a void notice withdraw nothing");
        assert_eq!(judged(&g, &n), Back::Unproven, "the issuer's void notice: to be sent again");
    }
}

