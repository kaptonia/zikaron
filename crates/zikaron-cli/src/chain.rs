//! The chain side: endpoints, scanning, anchoring, audit input. No chain decision is made here: the basis
//! shape belongs to the core, record reading and the three verdicts to the anchoring crate's `scan`, the
//! endpoint rule to its `endpoints`, audit-input assembly to its `input::assemble`, the report to the core's
//! `audit`.
//!
//! This layer does three argument jobs: read `--endpoint <chain>=<url>` into a table, read a recording into
//! replay endpoints, and count which chains have a single source for the endpoint rule (a single source must
//! be flagged).

use crate::codes::Reason;
use crate::out;
use zikaron::json::Value;
use zikaron_anchor::rpc;
use zikaron_anchor::scan;
use zikaron_anchor::wire::{self, Body, W};

/// A declined answer: code and detail, carried up unchanged.
pub struct Refused {
    pub reason: Reason,
    pub code: String,
    pub detail: String,
}

/// `--endpoint <chain>=<url>`, several per chain allowed (the endpoint rule).
pub fn endpoint_specs(raw: &[String]) -> Vec<(u64, String)> {
    crate::seam();
    let mut out_specs = Vec::new();
    for spec in raw {
        let (c, url) = match spec.split_once('=') {
            Some(x) => x,
            None => out::misuse(Reason::Args, spec, out::Said::EndpointShape),
        };
        let id: u64 = match c.parse() {
            Ok(x) => x,
            Err(_) => out::misuse(Reason::Args, c, out::Said::ChainIdNotInt),
        };
        out_specs.push((id, url.to_string()));
    }
    if out_specs.is_empty() {
        out::misuse(Reason::Args, "--endpoint", out::Said::NeedEndpoint);
    }
    out_specs
}

pub fn chains_of(specs: &[(u64, String)]) -> Vec<u64> {
    let mut c: Vec<u64> = specs.iter().map(|(x, _)| *x).collect();
    c.sort_unstable();
    c.dedup();
    c
}

/// How many rounds a scan runs: the largest endpoint count of any chain.
pub fn rounds(specs: &[(u64, String)]) -> usize {
    let mut n = 1;
    for (c, _) in specs {
        n = n.max(specs.iter().filter(|(x, _)| x == c).count());
    }
    n
}

pub fn nth_for(specs: &[(u64, String)], chain: u64, k: usize) -> String {
    let mine: Vec<&String> = specs.iter().filter(|(c, _)| *c == chain).map(|(_, u)| u).collect();
    mine[k.min(mine.len() - 1)].clone()
}

/// Single-source chains: fewer than two distinct endpoints. Endpoints are counted, not rounds: one endpoint
/// agreeing with itself corroborates nothing.
pub fn thin_chains(specs: &[(u64, String)]) -> Vec<u64> {
    chains_of(specs)
        .into_iter()
        .filter(|c| {
            let mut urls: Vec<&String> = specs.iter().filter(|(x, _)| x == c).map(|(_, u)| u).collect();
            urls.sort();
            urls.dedup();
            urls.len() < 2
        })
        .collect()
}

fn wire_of(path: &str) -> (Vec<u8>, W) {
    let b = crate::args::slurp(path);
    match wire::parse(&b) {
        Some(v) => (b, v),
        None => out::misuse(Reason::Unreadable, path, out::Said::NotJson),
    }
}

fn adoptions_of(v: &W) -> Vec<(u64, [u8; 32])> {
    let mut out_pairs = Vec::new();
    for e in v.member("adoptions").and_then(|x| x.as_arr()).unwrap_or(&[]) {
        let (Some(c), Some(t)) = (
            e.member("chainId").and_then(|x| x.as_u64()),
            e.member("tx").and_then(|x| x.as_str()),
        ) else {
            out::misuse(Reason::Args, "--adoptions", out::Said::AdoptionShape)
        };
        out_pairs.push((c, h32(t)));
    }
    out_pairs
}

pub fn h32(x: &str) -> [u8; 32] {
    let b = match zikaron::hexfmt::decode(x) {
        Some(b) => b,
        None => out::misuse(Reason::Args, x, out::Said::NotHex),
    };
    if b.len() != 32 {
        out::misuse(Reason::Args, x, out::Said::Not32);
    }
    let mut o = [0u8; 32];
    o.copy_from_slice(&b);
    o
}

pub fn h20(x: &str) -> [u8; 20] {
    let b = match zikaron::hexfmt::decode(x) {
        Some(b) => b,
        None => out::misuse(Reason::Args, x, out::Said::NotHex),
    };
    if b.len() != 20 {
        out::misuse(Reason::Args, x, out::Said::Not20);
    }
    let mut o = [0u8; 20];
    o.copy_from_slice(&b);
    o
}

/// Replay a recording into a scan fragment (or the core's no-label value). The offline path.
pub fn replay_fragment(path: &str) -> Result<Value, Refused> {
    crate::seam();
    let (src, fx) = wire_of(path);
    let basis_bytes = match fx.member("basis") {
        Some(b) => b.raw(&src).to_vec(),
        None => b"null".to_vec(),
    };
    let adoptions = adoptions_of(&fx);
    let recorded = fx.member("rpc").cloned().unwrap_or(W::of(Body::Obj(Vec::new())));
    let Body::Obj(chains) = &recorded.body else {
        out::misuse(Reason::Args, path, out::Said::RpcNotObject)
    };
    let mut replays: Vec<(u64, rpc::Replay)> = Vec::new();
    for (cid, exchanges) in chains {
        let id: u64 = match cid.parse() {
            Ok(x) => x,
            Err(_) => out::misuse(Reason::Args, cid, out::Said::ChainIdNotInt),
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
    finish(scan::run(&basis_bytes, &adoptions, &mut eps))
}

/// Scan live endpoints once.
pub fn live_fragment(
    basis_path: &str,
    adoptions_path: Option<&str>,
    specs: &[(u64, String)],
    round: usize,
) -> Result<(String, Value), Refused> {
    crate::seam();
    let (basis_src, _) = wire_of(basis_path);
    let adoptions = match adoptions_path {
        Some(p) => adoptions_of(&wire_of(p).1),
        None => Vec::new(),
    };
    let mut https: Vec<(u64, rpc::Http)> = Vec::new();
    for c in chains_of(specs) {
        let url = nth_for(specs, c, round);
        match rpc::Http::new(&url) {
            Some(h) => https.push((c, h)),
            None => out::misuse(Reason::Args, &url, out::Said::EndpointScheme),
        }
    }
    let names: Vec<String> = https
        .iter()
        .map(|(c, h)| {
            use zikaron_anchor::rpc::Endpoint as _;
            format!("{c}={}", h.name())
        })
        .collect();
    let mut eps: Vec<(u64, &mut dyn rpc::Endpoint)> = https
        .iter_mut()
        .map(|(c, h)| (*c, h as &mut dyn rpc::Endpoint))
        .collect();
    finish(scan::run(&basis_src, &adoptions, &mut eps)).map(|v| (names.join(","), v))
}

/// The three scan outcomes in one place: a refusal means no answer; no label is the core's answer, passed on
/// unchanged.
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
