use crate::codes::Reason;
use crate::out;
use zikaron::json::Value;
use zikaron_anchor::rpc;
use zikaron_anchor::scan;
use zikaron_anchor::wire::{self, Body, W};

pub struct Refused {
    pub reason: Reason,
    pub code: String,
    pub detail: String,
}

pub fn endpoint_specs(raw: &[(usize, String)]) -> Vec<(u64, String)> {
    crate::seam();
    let mut out_specs = Vec::new();
    for (at, spec) in raw {
        let subject = || out::unechoed_endpoint(*at, spec);
        match zikaron_anchor::rpc::endpoint_spec(spec) {
            Ok((chain, address)) => match address.rsplit_once(zikaron_anchor::rest::MARK) {
                Some((base, lane)) => match (lane.parse::<u64>(), zikaron_anchor::rest::base_of(base)) {
                    (Err(_), _) => out::misuse(Reason::Args, subject(), out::Said::ChainIdNotInt),
                    (Ok(c), _) if c != chain => out::misuse(Reason::Args, subject(), out::Said::RestRowChain),
                    (Ok(_), Ok(_)) => out_specs.push((chain, address)),
                    (Ok(_), Err(zikaron_anchor::rest::NotABase::Address(_))) => out::misuse(Reason::Args, subject(), out::address_said(base)),
                    (Ok(_), Err(_)) => out::misuse(Reason::Args, subject(), out::Said::RestRowBase),
                },
                None => match zikaron_anchor::rpc::read_address(&address) {
                    Ok(_) => out_specs.push((chain, address)),
                    Err(_) => out::misuse(Reason::Args, subject(), out::address_said(&address)),
                },
            },
            Err(zikaron_anchor::rpc::NotAnEndpoint::ChainNotInt) => out::misuse(Reason::Args, subject(), out::Said::ChainIdNotInt),
            Err(_) => out::misuse(Reason::Args, subject(), out::Said::EndpointShape),
        }
    }
    if out_specs.is_empty() {
        out::misuse(Reason::Args, out::Subject::flag("endpoint"), out::Said::NeedEndpoint);
    }
    out_specs
}

pub fn chains_of(specs: &[(u64, String)]) -> Vec<u64> {
    let mut c: Vec<u64> = specs.iter().map(|(x, _)| *x).collect();
    c.sort_unstable();
    c.dedup();
    c
}

fn wire_of(path: &str) -> (Vec<u8>, W) {
    let b = crate::args::slurp(path);
    match wire::parse(&b) {
        Some(v) => (b, v),
        None => out::misuse(Reason::Unreadable, out::typed(path), out::Said::NotJson),
    }
}

pub fn h32(x: &str) -> [u8; 32] {
    let b = match zikaron::hexfmt::decode(x) {
        Some(b) => b,
        None => out::misuse(Reason::Args, out::typed(x), out::Said::NotHex),
    };
    if b.len() != 32 {
        out::misuse(Reason::Args, out::typed(x), out::Said::Not32);
    }
    let mut o = [0u8; 32];
    o.copy_from_slice(&b);
    o
}

pub fn h20(x: &str) -> [u8; 20] {
    let b = match zikaron::hexfmt::decode(x) {
        Some(b) => b,
        None => out::misuse(Reason::Args, out::typed(x), out::Said::NotHex),
    };
    if b.len() != 20 {
        out::misuse(Reason::Args, out::typed(x), out::Said::Not20);
    }
    let mut o = [0u8; 20];
    o.copy_from_slice(&b);
    o
}

pub fn replay_fragment(path: &str) -> Result<Value, Refused> {
    crate::seam();
    let (src, fx) = wire_of(path);
    let basis_bytes = match fx.member("basis") {
        Some(b) => b.raw(&src).to_vec(),
        None => b"null".to_vec(),
    };
    let recorded = fx.member("rpc").cloned().unwrap_or(W::of(Body::Obj(Vec::new())));
    let Body::Obj(chains) = &recorded.body else {
        out::misuse(Reason::Args, out::typed(path), out::Said::RpcNotObject)
    };
    let mut replays: Vec<(u64, rpc::Replay)> = Vec::new();
    for (cid, exchanges) in chains {
        let id: u64 = match cid.parse() {
            Ok(x) => x,
            Err(_) => out::misuse(Reason::Args, out::typed(cid), out::Said::ChainIdNotInt),
        };
        let ex = exchanges.as_arr().unwrap_or(&[]).to_vec();
        match rpc::Replay::new(format!("replay:{id}"), &ex) {
            Ok(r) => replays.push((id, r)),
            Err(e) => {
                return Err(Refused {
                    reason: Reason::Scan,
                    code: "E_RECORDING".to_string(),
                    detail: format!("{e:?}"),
                })
            }
        }
    }
    let mut eps: Vec<(u64, &mut dyn rpc::Endpoint)> = replays
        .iter_mut()
        .map(|(c, r)| (*c, r as &mut dyn rpc::Endpoint))
        .collect();
    finish(scan::run(&basis_bytes, &mut eps))
}

pub fn basis_bytes(path: &str) -> Vec<u8> {
    wire_of(path).0
}

fn finish(
    r: Result<Result<scan::Scanned, Value>, scan::Refusal>,
) -> Result<Value, Refused> {
    match r {
        Err(refusal) => Err(Refused {
            reason: Reason::Scan,
            code: refusal.code().to_string(),
            detail: refusal.detail(),
        }),
        Ok(Err(no_label)) => Ok(no_label),
        Ok(Ok(s)) => Ok(scan::fragment(&s)),
    }
}
