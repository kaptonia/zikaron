use crate::node::{self, Endpoint};
use crate::{Code, Trouble, Why};
use zikaron::json::Value;
use zikaron_anchor::book::Call;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seat {
    pub root: [u8; 20],
    pub office: [u8; 20],
    pub lineage: Vec<[u8; 20]>,
}

pub fn seat_of(pile: &[Vec<u8>]) -> Option<Seat> {
    let read = zikaron_glue::book::read(pile, &zikaron_glue::book::empty_fragment()).ok()?;
    let h20 = |s: &str| -> Option<[u8; 20]> { zikaron::hexfmt::decode(s)?.try_into().ok() };
    Some(Seat { root: h20(&read.root)?, office: h20(read.office.as_deref()?)?, lineage: read.lineage.iter().filter_map(|k| h20(k)).collect() })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Member {
    pub chain: u64,
    pub at: [u8; 20],
}

impl Member {
    pub fn value(&self) -> Value {
        Value::Obj(vec![("at".to_string(), Value::Str(zikaron::hexfmt::encode(&self.at))), ("chain".to_string(), Value::Int(self.chain))])
    }
}

pub fn check(eps: &[Endpoint], chain: u64, seat: &Seat) -> Result<Member, Trouble> {
    let Some(venue) = zikaron_anchor::revoke::venue(chain) else { return Err(Trouble::text(Code::NoNoticeVenue, chain.to_string())) };
    let _read = node::OneRead::open();
    crate::book::ahead(eps, chain, &[(Call::Current, seat.root), (Call::Root, seat.office)], &|h| vec![("eth_getCode", Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(&seat.office)), Value::Str(format!("0x{h:x}"))]))]);
    let (serving, left) = crate::batch::serving_eps(eps, chain)?;
    let (head, _) = node::head_block(&serving, chain).map_err(|t| crate::batch::with_left(t, &left))?;
    let places: Vec<String> = serving.iter().map(|e| e.url.for_transport().to_string()).collect();
    let (gate, _) = crate::fingerprint::gate_counted(chain, &venue.contract, &places, zikaron_anchor::revoke::CODE_HASH);
    if !gate.admits() {
        return Err(Trouble::text(Code::NoticeContract, format!("{chain} {}", zikaron::hexfmt::encode(&venue.contract))));
    }
    let at = match crate::book::state(&serving, chain)? {
        crate::book::State::At(at) => at,
        _ => return Err(Trouble::text(Code::BookCode, format!("{chain} {}", zikaron::hexfmt::encode(&venue.book)))),
    };
    let current = crate::book::address(&serving, chain, &at, Call::Current, &seat.root)?;
    let said = |c: &[u8; 20]| format!("{chain} root={} office={} book={}", zikaron::hexfmt::encode(&seat.root), zikaron::hexfmt::encode(&seat.office), zikaron::hexfmt::encode(c));
    if current != seat.office {
        let entered = crate::book::entered(&serving, chain, &at, &seat.root)?;
        let code = if current == seat.root && !entered {
            Code::RootNotInBook
        } else if seat.lineage.contains(&current) {
            Code::NotAcceded
        } else {
            Code::OfficeElsewhere
        };
        return Err(Trouble::text(code, said(&current)));
    }
    let root = crate::book::address(&serving, chain, &at, Call::Root, &seat.office)?;
    if root != seat.root {
        return Err(Trouble::text(Code::OfficeElsewhere, said(&root)));
    }
    let code = node::ask(&serving, "eth_getCode", &Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(&seat.office)), Value::Str(format!("0x{head:x}"))]))?;
    let code = code.value.as_str().and_then(zikaron::hexfmt::decode).ok_or_else(|| Trouble::new(Code::ChainShape, Why::Text(format!("eth_getCode {:?}", code.value))))?;
    if !code.is_empty() {
        return Err(Trouble::text(Code::OfficeIsContract, zikaron::hexfmt::encode(&seat.office)));
    }
    Ok(Member { chain, at: venue.contract })
}

pub fn member_of(g: &[u8]) -> Option<Member> {
    let e = zikaron_glue::book::check(g).ok()?;
    let m = e.body.member("revocation")?;
    let chain = m.member("chain").and_then(Value::as_int)?;
    let at: [u8; 20] = m.member("at").and_then(Value::as_str).and_then(zikaron::hexfmt::decode).and_then(|b| b.try_into().ok())?;
    Some(Member { chain, at })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_member_is_written_in_its_one_form() {
        let m = Member { chain: 11_155_111, at: [0xab; 20] };
        assert_eq!(zikaron::json::canon_bytes(&m.value()), format!("{{\"at\":\"0x{}\",\"chain\":11155111}}", "ab".repeat(20)).into_bytes());
    }

    #[test]
    fn a_chain_with_no_revocation_contract_is_refused_before_asking() {
        let seat = Seat { root: [1; 20], office: [1; 20], lineage: vec![[1; 20]] };
        let t = check(&[], 31_337, &seat).unwrap_err();
        assert_eq!((t.code, t.why), (Code::NoNoticeVenue, Why::Text("31337".into())));
    }
}
