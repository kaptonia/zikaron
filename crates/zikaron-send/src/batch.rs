use crate::gate;
use crate::keep;
use crate::node::{self, Endpoint, Next, NodeAddr};
use crate::{Code, End, Facts, Note, Trouble, Why};
use zikaron::json::Value;
use zikaron_anchor::rpc;
use zikaron_anchor::send::{Confirm, Fees};
use zikaron_seal::queue::{Queue, Queued, Step, RESENDS_MAX};

pub trait Signer {
    fn from(&self) -> Option<[u8; 20]>;
    #[allow(clippy::too_many_arguments)]
    fn sign(&self, chain: u64, registry: Option<[u8; 20]>, hashes: &[[u8; 32]], input: Option<Vec<u8>>, fees: Fees, nonce: u64) -> Result<(Vec<u8>, [u8; 32]), rpc::Trouble>;
}

#[derive(Clone, Debug)]
pub struct Batch {
    pub ids: Vec<String>,
    pub hashes: Vec<[u8; 32]>,
    pub chain: u64,
    pub registry: [u8; 20],
    pub urls: Vec<NodeAddr>,
    pub backoff: Vec<std::time::Duration>,
    pub basis: Vec<Lane>,
    pub carried: Option<Vec<u8>>,
}

impl Batch {
    pub fn calldata(&self) -> Vec<u8> {
        calldata_of(&self.hashes, self.carried.as_deref())
    }
}

pub fn calldata_of(hashes: &[[u8; 32]], carried: Option<&[u8]>) -> Vec<u8> {
    match carried {
        Some(e) => zikaron_anchor::send::carried_calldata(e),
        None => zikaron_anchor::send::registry_calldata(hashes),
    }
}

pub fn carried_of(pile: &[Vec<u8>], ids: &[String], chain: u64) -> Option<Vec<u8>> {
    let [id] = ids else { return None };
    let id = id.to_ascii_lowercase();
    let bytes = pile.iter().find(|b| zikaron::hexfmt::encode(&zikaron::cryptox::sha256(b)) == id)?;
    let e = zikaron_glue::book::check(bytes).ok()?;
    if e.kind != zikaron_glue::book::Kind::Face {
        return None;
    }
    let faces: Vec<zikaron_glue::book::Entry> = pile.iter().filter_map(|b| zikaron_glue::book::check(b).ok()).filter(|e| e.kind == zikaron_glue::book::Kind::Face).collect();
    (zikaron_glue::face::seats_of(&faces).get(&id) == Some(&chain)).then(|| bytes.clone())
}

pub fn face_gate(urls: &[NodeAddr], chain: u64, registry: &[u8; 20], entry: &[u8], pile: &[Vec<u8>], from: &[u8; 20]) -> Result<(), Trouble> {
    let places: Vec<String> = urls.iter().map(|u| u.for_transport().to_string()).collect();
    let (gate, _) = crate::fingerprint::gate_counted(chain, registry, &places, zikaron_anchor::deploy::CODE_HASH);
    if !gate.admits() {
        return Err(Trouble::text(Code::RegistryCode, format!("{chain} {} {}", zikaron::hexfmt::encode(registry), gate.code())));
    }
    let h20 = |s: &str| -> Option<[u8; 20]> { zikaron::hexfmt::decode(s)?.try_into().ok() };
    let author = zikaron_glue::book::check(entry).ok().and_then(|e| h20(&e.author)).ok_or_else(|| Trouble::text(Code::ContentShape, zikaron::hexfmt::encode(&zikaron::cryptox::sha256(entry))))?;
    if author != *from {
        return Err(Trouble::text(Code::AddressMismatch, format!("author={} sender={}", zikaron::hexfmt::encode(&author), zikaron::hexfmt::encode(from))));
    }
    let root = zikaron_glue::book::root_of(pile).ok().as_deref().and_then(h20).ok_or_else(|| Trouble::text(Code::ContentShape, "root".to_string()))?;
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(chain, u.clone())).collect();
    let lineage: Vec<[u8; 20]> = zikaron_glue::book::read(pile, &zikaron_glue::book::empty_fragment()).map(|r| r.lineage.iter().filter_map(|k| h20(k)).collect()).unwrap_or_default();
    crate::book::seated(&eps, chain, &root, from, &lineage)
}

#[derive(Clone, Debug)]
pub struct Lane {
    pub chain: u64,
    pub registry: [u8; 20],
    pub urls: Vec<NodeAddr>,
    pub from_block: u64,
}

#[derive(Clone, Debug)]
pub struct GasAsk {
    pub chain: u64,
    pub call: Value,
    pub data: Vec<u8>,
    pub eps: Vec<Endpoint>,
}

#[derive(Clone, Debug)]
pub struct Estimate {
    pub gas: u64,
    pub calldata: String,
    pub fees: Fees,
    pub fees_left: Vec<Why>,
    pub head_time: Option<u64>,
}

pub fn estimate(a: &GasAsk) -> Result<Estimate, Trouble> {
    let _read = node::OneRead::open();
    use zikaron_anchor::send::NoGas;
    let GasAsk { chain, call, data, eps } = a;
    let chain = *chain;
    node::ask_ahead(eps, chain, &|_, lowest, _| {
        let at = Value::Str(format!("0x{lowest:x}"));
        vec![
            ("eth_estimateGas", Value::Arr(vec![call.clone(), at.clone()])),
            ("eth_getBlockByNumber", Value::Arr(vec![at, Value::Bool(false)])),
            ("eth_feeHistory", zikaron_anchor::send::tip_params(lowest)),
            ("eth_getBlockByNumber", Value::Arr(vec![Value::Str("latest".into()), Value::Bool(false)])),
        ]
    });
    let (eps, left) = serving_eps(eps, chain)?;
    let eps = &eps;
    let gas = zikaron_anchor::send::estimate_gas(
        || node::head_block(eps, chain).map(|(height, _)| height).map_err(|t| (t, false)),
        |params| node::ask_call(eps, "eth_estimateGas", params).map(|r| r.value),
        call.clone(),
        |(_, about_the_call): &(Trouble, bool)| !about_the_call,
    )
    .map_err(|no| match no {
        NoGas::Network((t, _)) => with_left(t, &left),
        NoGas::Refused((t, _)) => Trouble::new(Code::GasRefused, t.why),
        NoGas::NotText(other) => Trouble::new(Code::ChainShape, Why::GasNotText(format!("{:?}", other))),
        NoGas::Unreadable(hex) => Trouble::new(Code::ChainShape, Why::GasNotQty(hex)),
        NoGas::OverCap(n) => Trouble::text(Code::GasRefused, format!("{n} > {}", zikaron_anchor::send::GAS_LIMIT)),
    })?;
    let head_time = node::head_time(eps, chain).ok().map(|(time, _, _)| time);
    let (fees, more_left) = node::fees(eps, chain);
    let fees_left: Vec<Why> = left.into_iter().chain(more_left).collect();
    let fees = fees.with_estimate(gas);
    Ok(Estimate { gas, calldata: zikaron::hexfmt::encode(data), fees, fees_left, head_time })
}

pub fn funds_gate(b: &Batch, signer: &dyn Signer, fees: Fees, note: &dyn Fn(Note)) -> Result<(), Trouble> {
    let who = signer.from().ok_or_else(|| Trouble::new(Code::KeyMalformed, Why::NoAddress))?;
    let eps: Vec<Endpoint> = b.urls.iter().map(|u| Endpoint::at(b.chain, u.clone())).collect();
    note(Note::Balance);
    let (have, _) = node::balance(&eps, b.chain, &who)?;
    funds_check(fees.cap_wei(), have)
}

pub fn funds_check(need: u128, have: u128) -> Result<(), Trouble> {
    if have < need {
        return Err(Trouble::new(Code::InsufficientFunds, Why::Funds { need, have }));
    }
    Ok(())
}

pub fn serving_urls(urls: &[NodeAddr], chain: u64) -> Result<(Vec<NodeAddr>, Vec<Why>), Trouble> {
    if urls.is_empty() {
        return Err(Trouble::text(Code::NoEndpoint, chain.to_string()));
    }
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(chain, u.clone())).collect();
    let (kept, left) = node::serving(&eps, chain)?;
    Ok((kept.into_iter().map(|e| e.url).collect(), left))
}

pub(crate) fn serving_eps(eps: &[Endpoint], chain: u64) -> Result<(Vec<Endpoint>, Vec<Why>), Trouble> {
    let urls: Vec<NodeAddr> = eps.iter().filter(|e| e.chain == chain).map(|e| e.url.clone()).collect();
    let (kept, left) = serving_urls(&urls, chain)?;
    Ok((kept.into_iter().map(|u| Endpoint::at(chain, u)).collect(), left))
}

pub(crate) fn with_left(t: Trouble, left: &[Why]) -> Trouble {
    if left.is_empty() {
        return t;
    }
    let mut all = match t.why.clone() {
        Why::Each(xs) => xs,
        other => vec![other],
    };
    all.extend(left.iter().cloned());
    t.retold(Why::Each(all))
}

#[allow(clippy::too_many_arguments)]
pub fn on_chain(urls: &[NodeAddr], chain: u64, registry: &[u8; 20], from: &[u8; 20], hashes: &[[u8; 32]], start: u64, lineage: &[[u8; 20]]) -> Result<Option<u64>, Trouble> {
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(chain, u.clone())).collect();
    let nodes = crate::agree::node_list(&eps, chain);
    let lineage: Vec<[u8; 20]> = if lineage.is_empty() { vec![*from] } else { lineage.to_vec() };
    let registries = zikaron_anchor::revoke::registries(chain, &[*registry]);
    let mut found: Vec<zikaron_anchor::send::OnChain> = Vec::new();
    let mut latest: Option<u64> = None;
    let mut heard = false;
    let mut over: Vec<zikaron_anchor::scan::Refusal> = Vec::new();
    let mut said: Vec<Why> = Vec::new();
    let mut first: Option<Trouble> = None;
    let ask = |urls: &[NodeAddr]| {
        let (reg, lin, who, hs) = (registries.clone(), lineage.clone(), *from, hashes.to_vec());
        let list = nodes.clone();
        let names = urls.to_vec();
        node::each_node(urls, &node::endpoint_at, move |_, ep| ep.map(|ep| zikaron_anchor::send::held_among(ep, chain, &reg, &lin, &who, &hs, start, None, &list)), |view| {
            let alike: Vec<(String, Vec<u8>)> = names
                .iter()
                .zip(view)
                .filter_map(|(u, a)| match a {
                    Some(Some(Ok(h))) => Some((u.for_transport().to_string(), format!("{:?}", (&h.found, h.latest)).into_bytes())),
                    _ => None,
                })
                .collect();
            zikaron_anchor::judge::two_places_alike(&alike) || alike.iter().any(|(p, _)| zikaron_anchor::action::splits(&zikaron_anchor::endpoints::place_of(p)))
        })
    };
    let mut each = ask(urls);
    zikaron_anchor::action::cut_and_ask(
        &mut each,
        |i, _| vec![zikaron_anchor::endpoints::place_of(urls[i].for_transport())],
        |a| matches!(a, Some(Some(Ok(_)))),
        |a| matches!(a, Some(Some(Err(n))) if n.cut()),
        |idx, _| ask(&idx.iter().map(|i| urls[*i].clone()).collect::<Vec<NodeAddr>>()),
    );
    for (u, asked) in urls.iter().zip(each) {
        let Some(answer) = asked else {
            said.push(node::node_why(u.for_transport(), &rpc::Trouble::Transport(rpc::Broke::Short)));
            continue;
        };
        let Some(answer) = answer else {
            said.push(node::address_said(u.for_transport()));
            continue;
        };
        match answer {
            Ok(got) => {
                heard = true;
                latest = latest.max(got.latest);
                for x in got.found {
                    match found.iter_mut().find(|f| f.hash == x.hash) {
                        Some(f) if f.block <= x.block => {}
                        Some(f) => *f = x,
                        None => found.push(x),
                    }
                }
            }
            Err(not) => match not.trouble() {
                _ if over_asks(&not).is_some() => {
                    over.extend(over_asks(&not));
                    said.push(Why::Text(format!("{} {}", u.said(), not.evidence())));
                }
                Some(t @ rpc::Trouble::Transport(_)) => {
                    first.get_or_insert_with(|| node::said_trouble(u.for_transport(), &t));
                    said.push(node::node_why(u.for_transport(), &t));
                }
                Some(t) => said.push(node::node_why(u.for_transport(), &t)),
                None => said.push(Why::Text(format!("{} {}", u.said(), not.evidence()))),
            },
        }
    }
    if !found.is_empty() {
        found.sort_by_key(|f| (f.block, f.hash));
        let earliest = found[0];
        let names = found.iter().map(|f| zikaron::hexfmt::encode(&f.hash)).collect();
        return Err(Trouble::new(Code::OnChain, Why::OnChain { names, tx: zikaron::hexfmt::encode(&earliest.tx), block: earliest.block }));
    }
    if !heard {
        return Err(match (first, over.is_empty()) {
            (_, false) => Trouble::new(Code::ScanRefused, Why::Scan { unanswered: said, refused: over }),
            (Some(t), true) => t.retold(Why::Each(said)),
            (None, true) => Trouble::new(Code::Unreachable, Why::Each(said)),
        });
    }
    Ok(latest)
}

fn over_asks(not: &zikaron_anchor::send::NotSaid) -> Option<zikaron_anchor::scan::Refusal> {
    match not {
        zikaron_anchor::send::NotSaid::Logs(r) if r.over_asks().is_some() => Some(r.clone()),
        _ => None,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OnChain {
    read: Vec<(Option<u64>, String)>,
    own: Vec<(u64, String, Option<u64>)>,
}

impl OnChain {
    pub fn of(read: impl IntoIterator<Item = (Option<u64>, String)>, queue: &Queue) -> OnChain {
        let mut own: Vec<(u64, String, Option<u64>)> = queue.blocks.iter().map(|b| (if b.chain == 0 { queue.chain } else { b.chain }, b.id.to_ascii_lowercase(), Some(b.block))).collect();
        for (id, c) in &queue.anchored {
            let c = if *c == 0 { queue.chain } else { *c };
            let id = id.to_ascii_lowercase();
            if !own.iter().any(|(x, y, _)| *x == c && *y == id) {
                own.push((c, id, None));
            }
        }
        OnChain { read: read.into_iter().map(|(c, id)| (c, id.to_ascii_lowercase())).collect(), own }
    }

    pub fn on(&self, chain: u64, id: &str) -> bool {
        let id = id.to_ascii_lowercase();
        self.read.iter().any(|(c, x)| *x == id && c.is_none_or(|c| c == chain)) || self.own.iter().any(|(c, x, _)| *c == chain && *x == id)
    }

    pub fn anywhere(&self, id: &str) -> bool {
        let id = id.to_ascii_lowercase();
        self.read.iter().any(|(_, x)| *x == id) || self.own.iter().any(|(_, x, _)| *x == id)
    }

    pub fn own_latest(&self, chain: u64, ids: &[String]) -> Option<u64> {
        self.own.iter().filter(|(c, x, _)| *c == chain && ids.iter().any(|i| i.eq_ignore_ascii_case(x))).filter_map(|(_, _, b)| *b).max()
    }
}

pub const FENCE_PAUSES: [std::time::Duration; 10] = [std::time::Duration::from_secs(3); 10];

pub fn fence_open(head_time: u64, before: &[(u64, u64)]) -> Result<(), u64> {
    match before.iter().map(|(_, t)| *t).max() {
        Some(after) if head_time <= after => Err(after),
        _ => Ok(()),
    }
}

pub fn before_of(b: &Batch, pile: &[Vec<u8>], queue: &Queue, key: Option<&str>, moved: Option<&str>) -> Result<Vec<(u64, u64)>, Trouble> {
    let own = OnChain::of(std::iter::empty(), queue);
    let places = zikaron_seal::queue::Place::of_pile(pile);
    let mine: Vec<zikaron_seal::queue::Place> = b.ids.iter().filter_map(|id| places.get(&id.to_ascii_lowercase()).copied()).collect();
    let first = if mine.len() < b.ids.len() { None } else { mine.iter().map(|p| (p.law, p.seq)).min() };
    let went_before: Vec<String> = places
        .iter()
        .filter(|(_, p)| first.is_none_or(|f| (p.law, p.seq) < f))
        .filter(|(id, _)| !b.ids.iter().any(|x| x.eq_ignore_ascii_case(id)))
        .map(|(id, _)| id.to_string())
        .collect();
    let hashes: Vec<[u8; 32]> = went_before.iter().filter_map(|id| zikaron::hexfmt::decode(id).and_then(|v| v.try_into().ok())).collect();
    let senders: Vec<[u8; 20]> = zikaron_glue::basis::senders_of(pile).iter().filter_map(|a| zikaron::hexfmt::decode(a).and_then(|v| v.try_into().ok())).collect();
    let lanes: Vec<Lane> = if b.basis.is_empty() { vec![Lane { chain: b.chain, registry: b.registry, urls: b.urls.clone(), from_block: 0 }] } else { b.basis.clone() };
    let lineage = crate::gate::lineage_bytes(&crate::gate::lineage(pile, key, moved));
    let mut out = Vec::new();
    for lane in &lanes {
        let urls = if lane.urls.is_empty() { Vec::new() } else { serving_urls(&lane.urls, lane.chain)?.0 };
        let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(lane.chain, u.clone())).collect();
        let head = node::head_block(&eps, lane.chain).ok().map(|(h, _)| h);
        let nodes = crate::agree::node_list(&eps, lane.chain);
        let mut latest: Option<u64> = None;
        let mut heard = false;
        let mut over: Vec<zikaron_anchor::scan::Refusal> = Vec::new();
        let mut said: Vec<Why> = Vec::new();
        let registries = zikaron_anchor::revoke::registries(lane.chain, &[lane.registry]);
        let ask = |urls: &[NodeAddr]| {
            let (c, from_block, reg, lin, snd, hs, list) = (lane.chain, lane.from_block, registries.clone(), lineage.clone(), senders.clone(), hashes.clone(), nodes.clone());
            let names = urls.to_vec();
            node::each_node(urls, &node::endpoint_at, move |_, ep| ep.map(|ep| zikaron_anchor::send::latest_among(ep, c, &reg, &lin, &snd, &hs, from_block, head, &list)), |view| {
                let alike: Vec<(String, Vec<u8>)> = names
                    .iter()
                    .zip(view)
                    .filter_map(|(u, a)| match a {
                        Some(Some(Ok(l))) => Some((u.for_transport().to_string(), format!("{l:?}").into_bytes())),
                        _ => None,
                    })
                    .collect();
                zikaron_anchor::judge::two_places_alike(&alike) || alike.iter().any(|(p, _)| zikaron_anchor::action::splits(&zikaron_anchor::endpoints::place_of(p)))
            })
        };
        let mut each = ask(&urls);
        zikaron_anchor::action::cut_and_ask(
            &mut each,
            |i, _| vec![zikaron_anchor::endpoints::place_of(urls[i].for_transport())],
            |a| matches!(a, Some(Some(Ok(_)))),
            |a| matches!(a, Some(Some(Err(n))) if n.cut()),
            |idx, _| ask(&idx.iter().map(|i| urls[*i].clone()).collect::<Vec<NodeAddr>>()),
        );
        for (u, asked) in urls.iter().zip(each) {
            match asked {
                None => said.push(node::node_why(u.for_transport(), &rpc::Trouble::Transport(rpc::Broke::Short))),
                Some(None) => said.push(node::address_said(u.for_transport())),
                Some(Some(Ok(l))) => {
                    heard = true;
                    latest = latest.max(l);
                }
                Some(Some(Err(not))) => {
                    over.extend(over_asks(&not));
                    said.push(Why::Text(format!("{} {}", u.said(), not.evidence())));
                }
            }
        }
        if !heard && !over.is_empty() {
            return Err(Trouble::new(Code::ScanRefused, Why::Scan { unanswered: said, refused: over }));
        }
        if !heard {
            return Err(Trouble::new(Code::Unreachable, Why::AtChain { chain: lane.chain, why: Box::new(Why::Each(said)) }));
        }
        latest = latest.max(own.own_latest(lane.chain, &went_before));
        if let Some(block) = latest {
            out.push((lane.chain, node::block_time(&eps, lane.chain, block)?));
        }
    }
    Ok(out)
}

pub fn fence_after(urls: &[NodeAddr], chain: u64, before: &[(u64, u64)]) -> Result<(), Trouble> {
    if before.is_empty() {
        return Ok(());
    }
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(chain, u.clone())).collect();
    let head = || node::head_time_now(&eps, chain);
    match await_fence(before, head) {
        Ok(()) => Ok(()),
        Err(Fenced::Unread(t)) => Err(t),
        Err(Fenced::TooEarly { head_time, after }) => Err(Trouble::new(Code::TooEarly, Why::TooEarly { chain, head_time, after })),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fenced<T> {
    Unread(T),
    TooEarly { head_time: u64, after: u64 },
}

pub fn await_fence<T>(before: &[(u64, u64)], mut head: impl FnMut() -> Result<u64, T>) -> Result<(), Fenced<T>> {
    let pauses: Vec<std::time::Duration> = FENCE_PAUSES.iter().map(|d| zikaron_anchor::patience::waited(*d)).collect();
    let mut pauses = pauses.into_iter();
    let mut head_time = head().map_err(Fenced::Unread)?;
    loop {
        let after = match fence_open(head_time, before) {
            Ok(()) => return Ok(()),
            Err(after) => after,
        };
        let Some(p) = pauses.next() else { return Err(Fenced::TooEarly { head_time, after }) };
        head_time = zikaron_anchor::action::after_a_beat(p, &mut head).map_err(Fenced::Unread)?;
    }
}

pub fn fence(urls: &[NodeAddr], chain: u64, latest: Option<u64>) -> Result<(), Trouble> {
    let Some(block) = latest else { return Ok(()) };
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(chain, u.clone())).collect();
    let after = node::block_time(&eps, chain, block)?;
    let head = || node::head_time_now(&eps, chain);
    match await_fence(&[(chain, after)], head) {
        Ok(()) => Ok(()),
        Err(Fenced::Unread(t)) => Err(t),
        Err(Fenced::TooEarly { head_time, after }) => Err(Trouble::new(Code::TooEarly, Why::TooEarly { chain, head_time, after })),
    }
}

#[derive(Clone, Debug)]
pub struct Sent {
    pub tx: String,
    pub chain: u64,
    pub url: NodeAddr,
    pub ids: Vec<String>,
    pub gas: Option<u64>,
    pub queue: Queue,
    pub earlier: bool,
}

pub fn out_first(q: &Queue) -> Option<(Vec<String>, u64, Vec<String>)> {
    q.submitted().into_iter().next()
}

pub fn again_at(l: &Landed, switch: zikaron_seal::machine::CliAnchor) -> Option<(Stuck, Fees)> {
    match (&l.stuck, switch) {
        (Some(stuck @ Stuck { offer: Offer::Unheld { fees, .. }, .. }), zikaron_seal::machine::CliAnchor::Send) if l.receipt == Receipt::Unused && !l.voided => Some((stuck.clone(), *fees)),
        _ => None,
    }
}

pub fn out_for(q: &Queue, ids: &[String]) -> Option<(Vec<String>, u64, Vec<String>)> {
    out_on(q, ids, q.chain)
}

pub fn out_on(q: &Queue, ids: &[String], chain: u64) -> Option<(Vec<String>, u64, Vec<String>)> {
    let (txs, chain) = ids.iter().find_map(|id| q.step_on(id, chain).and_then(Step::awaited))?;
    let all: Vec<String> = q.items.iter().filter(|x| (x.chain == chain || x.chain == 0) && x.step.awaited().is_some_and(|(t, c)| t == txs && c == chain)).map(|x| x.id.clone()).collect();
    Some((txs, chain, all))
}

#[allow(clippy::too_many_arguments)]
pub fn anchor<E: End + Facts>(end: &E, b: Batch, signer: &dyn Signer, gas: Option<u64>, fees: Fees, ask: gate::Ask) -> Result<Sent, E::Fail> {
    let mut b = b;
    let chain = b.chain;
    let refused = |f: E::Fail, ids: &[String]| -> E::Fail {
        let said = E::code_of(&f).unwrap_or_default();
        let _ = keep::amend(end, |q| q.mark_on(ids, chain, Step::Refused { said }));
        f
    };
    let start = ask.from_block_of(chain).ok_or_else(|| refused(Trouble::text(Code::NoRegistry, chain.to_string()).into(), &b.ids))?;
    let disk = keep::read(end)?;
    if let Some(out) = out_on(&disk, &b.ids, b.chain) {
        return Ok(earlier(out, &b, gas, disk));
    }
    crate::shut::check(end).map_err(|f| refused(f, &b.ids))?;
    b.urls = serving_urls(&b.urls, b.chain).map_err(|t| refused(t.into(), &b.ids))?.0;
    let from = signer.from().ok_or_else(|| refused(Trouble::new(Code::KeyMalformed, Why::NoAddress).into(), &b.ids))?;
    let pile = end.pile().map_err(|f| refused(f, &b.ids))?;
    let moved = gate::moved_to(&ask, &pile);
    let lineage = gate::lineage_bytes(&gate::lineage(&pile, ask.own.as_deref(), moved.as_deref()));
    ahead_of_send(&b, &from, start, &lineage);
    b.carried = carried_of(&pile, &b.ids, b.chain);
    if let Some(entry) = &b.carried {
        face_gate(&b.urls, b.chain, &b.registry, entry, &pile, &from).map_err(|t| refused(t.into(), &b.ids))?;
    }
    on_chain(&b.urls, b.chain, &b.registry, &from, &b.hashes, start, &lineage).map_err(|t| refused(t.into(), &b.ids))?;
    if zikaron_seal::queue::at_boundary(&b.ids, &zikaron_seal::queue::Place::of_pile(&pile)) {
        let before = before_of(&b, &pile, &disk, ask.own.as_deref(), moved.as_deref()).map_err(|t| refused(t.into(), &b.ids))?;
        fence_after(&b.urls, b.chain, &before).map_err(|t| refused(t.into(), &b.ids))?;
    }
    funds_gate(&b, signer, fees, &|n| end.note(n)).map_err(|t| refused(t.into(), &b.ids))?;
    let pass = gate::pass(&ask, end).map_err(|f| refused(f, &b.ids))?;
    end.note(Note::Stage(0));
    let (raw, hash, nonce) = sign_new(signer, &b, fees).map_err(|t| refused(t.into(), &b.ids))?;
    let tx = zikaron::hexfmt::encode(&hash);
    let (out, queue) = keep::amend(end, |q| match out_on(q, &b.ids, b.chain) {
        Some(out) => Some(out),
        None => {
            q.mark_on(&b.ids, b.chain, Step::Submitted { tx: tx.clone(), chain: b.chain, nonce: Some(nonce) });
            None
        }
    })?;
    if let Some(out) = out {
        return Ok(earlier(out, &b, gas, queue));
    }
    match cast(&pass, &b.urls, b.chain, &raw, hash, &b.backoff, None, &|n| end.note(n)) {
        Ok(c) => Ok(Sent { tx, chain: b.chain, url: c.taken, ids: b.ids, gas, queue, earlier: false }),
        Err(t) => {
            if node::next_after(&t) == Next::Stop {
                let f: E::Fail = t.into();
                let said = E::code_of(&f).unwrap_or_default();
                let _ = keep::amend(end, |q| q.moved_by(&b.ids, std::slice::from_ref(&tx), b.chain, Step::Refused { said }));
                return Err(f);
            }
            Err(t.into())
        }
    }
}

fn ahead_of_send(b: &Batch, from: &[u8; 20], start: u64, lineage: &[[u8; 20]]) {
    let eps: Vec<Endpoint> = b.urls.iter().map(|u| Endpoint::at(b.chain, u.clone())).collect();
    let nodes = crate::agree::node_list(&eps, b.chain);
    let lineage: Vec<[u8; 20]> = if lineage.is_empty() { vec![*from] } else { lineage.to_vec() };
    let registries = zikaron_anchor::revoke::registries(b.chain, &[b.registry]);
    node::ask_ahead(&eps, b.chain, &|ep, lowest, own| {
        let mut asks = zikaron_anchor::send::held_among_asks(ep, b.chain, &registries, &lineage, start, own, &nodes);
        asks.push(("eth_getBalance", Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(from)), Value::Str(format!("0x{lowest:x}"))])));
        asks
    });
}

fn earlier((txs, chain, ids): (Vec<String>, u64, Vec<String>), b: &Batch, gas: Option<u64>, queue: Queue) -> Sent {
    let url = b.urls.first().cloned().unwrap_or_else(|| NodeAddr::new(String::new()));
    Sent { tx: txs.last().cloned().unwrap_or_default(), chain, url, ids, gas, queue, earlier: true }
}

fn sign_new(signer: &dyn Signer, b: &Batch, fees: Fees) -> Result<(Vec<u8>, [u8; 32], u64), Trouble> {
    let urls = &b.urls;
    let first = urls.first().map(NodeAddr::said).unwrap_or_default();
    let open = |url: &NodeAddr| node::endpoint_at(url.for_transport()).ok_or_else(|| Trouble::new(Code::SettingsShape, node::address_said(url.for_transport())));
    let mut last: Option<Trouble> = None;
    let Some(from) = signer.from() else {
        return Err(last.unwrap_or_else(|| Trouble::text(Code::NoEndpoint, first)));
    };
    let params = zikaron_anchor::send::nonce_params(&from);
    let mut nodes: Vec<(String, Box<dyn rpc::Endpoint + Send>)> = Vec::new();
    let mut words: Vec<(usize, Trouble)> = Vec::new();
    for (i, url) in urls.iter().enumerate() {
        match open(url) {
            Ok(h) => nodes.push((url.for_transport().to_string(), node::one_ask(h))),
            Err(t) => words.push((i, t)),
        }
    }
    let answers = node::came(zikaron_anchor::endpoints::ask_each(nodes, "eth_getTransactionCount", &params, node::enough_for("eth_getTransactionCount", &params)));
    match zikaron_anchor::judge::judge("eth_getTransactionCount", &params, answers) {
        Ok(j) => {
            let taker = j.sources.first().cloned().unwrap_or_default();
            match j.value.as_str().and_then(node::wei).and_then(|n| u64::try_from(n).ok()) {
                Some(nonce) => match signer.sign(b.chain, Some(b.registry), &b.hashes, b.carried.as_ref().map(|_| b.calldata()), fees, nonce) {
                    Ok((raw, hash)) => return Ok((raw, hash, nonce)),
                    Err(t) => return Err(node::said_trouble(&taker, &t)),
                },
                None => last = Some(nonce_unread(&taker)),
            }
        }
        Err(zikaron_anchor::judge::NoReading::NoneAnswered(missing)) => {
            for (url, m) in missing {
                let said = match m {
                    zikaron_anchor::judge::Missing::Trouble(t) => node::said_trouble(&url, &t),
                    zikaron_anchor::judge::Missing::Unreadable(_) => nonce_unread(&url),
                };
                let i = urls.iter().position(|u| u.for_transport() == url).unwrap_or(usize::MAX);
                words.push((i, said));
            }
            words.sort_by_key(|(i, _)| *i);
            if let Some((_, t)) = words.iter().find(|(_, t)| node::next_after(t) == Next::Stop) {
                return Err(t.clone());
            }
            last = words.pop().map(|(_, t)| t);
        }
        Err(_) => last = Some(Trouble::text(Code::Unreachable, first.clone())),
    }
    Err(last.unwrap_or_else(|| Trouble::text(Code::NoEndpoint, first)))
}

fn nonce_unread(url: &str) -> Trouble {
    Trouble::new(Code::Unreachable, Why::NonceUnread { node: zikaron_net::sayable(url) })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cast {
    pub hash: [u8; 32],
    pub asked: usize,
    pub taken: NodeAddr,
}

#[allow(clippy::too_many_arguments)]
pub fn cast(_pass: &gate::Pass, urls: &[NodeAddr], chain: u64, raw: &[u8], hash: [u8; 32], backoff: &[std::time::Duration], last: Option<Trouble>, note: &dyn Fn(Note)) -> Result<Cast, Trouble> {
    let first = urls.first().map(NodeAddr::said).unwrap_or_default();
    let mut last = last;
    let open = |url: &NodeAddr| node::endpoint_at(url.for_transport()).ok_or_else(|| Trouble::new(Code::SettingsShape, node::address_said(url.for_transport())));
    let waits: Vec<std::time::Duration> = std::iter::once(std::time::Duration::ZERO).chain(backoff.iter().copied()).collect();
    note(Note::Stage(1));
    let mut asked = 0usize;
    let mut handed = false;
    for url in urls {
        let mut http = match open(url) {
            Ok(h) => h,
            Err(t) => {
                last = Some(t);
                continue;
            }
        };
        for w in &waits {
            if !w.is_zero() {
                std::thread::sleep(*w);
            }
            asked += 1;
            match zikaron_anchor::send::submit(http.as_mut(), raw, &hash) {
                Ok(()) => return Ok(Cast { hash, asked, taken: url.clone() }),
                Err(e) => {
                    let t = node::said_trouble(url.for_transport(), &e);
                    if node::already_taken(&t) && node::tx_known(urls, chain, &hash) {
                        return Ok(Cast { hash, asked, taken: url.clone() });
                    }
                    match node::next_after(&t) {
                        Next::Retry => last = Some(t),
                        Next::NextEndpoint => {
                            handed = true;
                            last = Some(t);
                            break;
                        }
                        Next::Stop => return Err(t),
                    }
                }
            }
        }
    }
    if handed && node::tx_known(urls, chain, &hash) {
        let Some(taken) = urls.first().cloned() else { return Err(last.unwrap_or_else(|| Trouble::text(Code::NoEndpoint, first))) };
        return Ok(Cast { hash, asked, taken });
    }
    Err(last.unwrap_or_else(|| Trouble::text(Code::NoEndpoint, first)))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stuck {
    pub txs: Vec<String>,
    pub chain: u64,
    pub from: [u8; 20],
    pub nonce: u64,
    pub to: [u8; 20],
    pub input: Vec<u8>,
    pub sent: Fees,
    pub offer: Offer,
    pub left: Vec<Why>,
    pub unpriced: Option<Unpriced>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unpriced {
    NoNode,
    OtherChain,
    Unread,
}

impl Unpriced {
    fn of(urls: &[NodeAddr], serving: Result<(), Code>) -> Unpriced {
        match serving {
            Err(c) if c.another_chain() => Unpriced::OtherChain,
            _ if urls.is_empty() => Unpriced::NoNode,
            Err(Code::NoEndpoint) => Unpriced::NoNode,
            _ => Unpriced::Unread,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Offer {
    Resend { fees: Fees, base_now: u64 },
    NotAbove { base_now: u64 },
    PriceUnread,
    Spent,
    Unheld { fees: Fees, base_now: u64 },
}

#[allow(clippy::too_many_arguments)]
pub fn resend<E: End + Facts>(end: &E, b: Batch, stuck: Stuck, fees: Fees, signer: &dyn Signer, ask: gate::Ask) -> Result<Sent, E::Fail> {
    let mut b = b;
    let start = ask.from_block_of(b.chain).ok_or_else(|| Trouble::text(Code::NoRegistry, b.chain.to_string()))?;
    crate::shut::check(end)?;
    b.urls = serving_urls(&b.urls, b.chain)?.0;
    let pile = end.pile()?;
    let moved = gate::moved_to(&ask, &pile);
    let lineage = gate::lineage_bytes(&gate::lineage(&pile, ask.own.as_deref(), moved.as_deref()));
    if let Some(entry) = carried_of(&pile, &b.ids, b.chain) {
        face_gate(&b.urls, b.chain, &b.registry, &entry, &pile, &stuck.from)?;
    }
    on_chain(&b.urls, b.chain, &b.registry, &stuck.from, &b.hashes, start, &lineage)?;
    funds_gate(&b, signer, fees, &|n| end.note(n))?;
    let pass = gate::pass(&ask, end)?;
    if signer.from() != Some(stuck.from) {
        return Err(Trouble::text(Code::AddressMismatch, zikaron::hexfmt::encode(&stuck.from)).into());
    }
    end.note(Note::Stage(0));
    let first = b.urls.first().map(|u| u.for_transport().to_string()).unwrap_or_default();
    let (raw, hash) = signer.sign(stuck.chain, Some(stuck.to), &[], Some(stuck.input.clone()), fees, stuck.nonce).map_err(|t| node::said_trouble(&first, &t))?;
    let tx = zikaron::hexfmt::encode(&hash);
    let before = if stuck.txs.len() == 1 {
        Step::Submitted { tx: stuck.txs[0].clone(), chain: b.chain, nonce: Some(stuck.nonce) }
    } else {
        Step::Resent { txs: stuck.txs.clone(), chain: b.chain, nonce: Some(stuck.nonce) }
    };
    let mut txs = stuck.txs.clone();
    if !txs.contains(&tx) {
        txs.push(tx.clone());
    }
    let after = match txs.as_slice() {
        [one] => Step::Submitted { tx: one.clone(), chain: b.chain, nonce: Some(stuck.nonce) },
        _ => Step::Resent { txs: txs.clone(), chain: b.chain, nonce: Some(stuck.nonce) },
    };
    let (_, queue) = keep::amend(end, |q| q.moved_by(&b.ids, &stuck.txs, b.chain, after))?;
    let c = match cast(&pass, &b.urls, b.chain, &raw, hash, &b.backoff, None, &|n| end.note(n)) {
        Ok(c) => c,
        Err(t) => {
            if node::next_after(&t) == Next::Stop {
                let _ = keep::amend(end, |q| q.moved_by(&b.ids, &txs, b.chain, before));
            }
            return Err(t.into());
        }
    };
    Ok(Sent { tx, chain: b.chain, url: c.taken, ids: b.ids, gas: None, queue, earlier: false })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Receipt {
    Included { status: u64, block: u64 },
    NotYet,
    Unheard(String),
    Split { has: Vec<String>, not_yet: Vec<String> },
    Void,
    Unused,
}

#[derive(Clone, Debug)]
pub struct Heard {
    pub tx: String,
    pub chain: u64,
    pub txs: Vec<String>,
    pub ids: Vec<String>,
    pub confirmed: bool,
    pub receipt: Receipt,
    pub gas: Option<u64>,
    pub calldata: String,
    pub stuck: Option<Stuck>,
    pub voided: bool,
    pub waited: u64,
    pub landing: Option<Landing>,
    pub kept: Option<Kept>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
    pub anchors: Vec<zikaron::json::Value>,
    pub single_source: bool,
}

#[derive(Clone, Debug)]
pub struct Landing {
    pub urls: Vec<NodeAddr>,
    pub registry: [u8; 20],
    pub from: [u8; 20],
}

#[derive(Clone, Debug)]
pub struct Landed {
    pub tx: String,
    pub chain: u64,
    pub confirmed: bool,
    pub receipt: Receipt,
    pub sent: usize,
    pub dropped: usize,
    pub queue: Vec<Queued>,
    pub gas: Option<u64>,
    pub calldata: String,
    pub stuck: Option<Stuck>,
    pub voided: bool,
    pub waited: u64,
    pub kept: Option<Kept>,
}

#[derive(Clone, Copy, Debug)]
pub struct Sender {
    pub from: [u8; 20],
    pub registry: [u8; 20],
}

#[allow(clippy::too_many_arguments)]
pub fn wait(
    urls: Vec<NodeAddr>,
    chain: u64,
    txs: Vec<String>,
    ids: Vec<String>,
    gas: Option<u64>,
    wait: std::time::Duration,
    backoff: &[std::time::Duration],
    sender: Option<Sender>,
    nonce: Option<u64>,
    carried: Option<&[u8]>,
) -> Result<Heard, Trouble> {
    let mut hashes: Vec<[u8; 32]> = Vec::with_capacity(txs.len());
    for tx in &txs {
        let h: [u8; 32] = zikaron::hexfmt::decode(tx).and_then(|b| b.try_into().ok()).ok_or_else(|| Trouble::text(Code::ContentShape, tx.clone()))?;
        hashes.push(h);
    }
    if hashes.is_empty() {
        return Err(Trouble::text(Code::QueueEmpty, ids.join(" ")));
    }
    let table = urls;
    let (urls, elsewhere) = if table.is_empty() {
        (Vec::new(), false)
    } else {
        match serving_urls(&table, chain) {
            Ok((kept, _)) => (kept, false),
            Err(t) if t.code.another_chain() => (Vec::new(), true),
            Err(t) => return Err(t),
        }
    };
    let reads = if elsewhere { &table } else { &urls };
    let url = urls.first().map(|u| u.for_transport().to_string()).unwrap_or_default();
    let mut opened: Vec<Box<dyn rpc::Endpoint + Send>> = urls.iter().filter_map(|u| node::endpoint_at(u.for_transport())).collect();
    if opened.is_empty() && !elsewhere {
        return Err(Trouble::new(Code::SettingsShape, node::address_said(&url)));
    }
    let mut refs: Vec<&mut dyn rpc::Endpoint> = opened.iter_mut().map(|b| b.as_mut() as &mut dyn rpc::Endpoint).collect();
    let share = wait / hashes.len() as u32;
    let rank = |c: &Confirm| match c {
        Confirm::Included { .. } => 3,
        Confirm::Split { .. } => 2,
        Confirm::NotYet => 1,
        Confirm::Unreachable(_) => 0,
    };
    let mut confirm: Option<Confirm> = None;
    let mut tx = txs.last().cloned().unwrap_or_default();
    let began = std::time::Instant::now();
    for (h, t) in hashes.iter().zip(&txs).filter(|_| !elsewhere) {
        let c = zikaron_anchor::send::confirm_each(&mut refs, h, share, backoff);
        let included = matches!(c, Confirm::Included { .. });
        if confirm.as_ref().is_none_or(|was| rank(&c) > rank(was)) {
            confirm = Some(c);
            if included {
                tx = t.clone();
                break;
            }
        }
    }
    let confirm = confirm.unwrap_or(Confirm::NotYet);
    let waited = began.elapsed().as_secs();
    let calldata = Queue::hashes(&ids).map(|h| zikaron::hexfmt::encode(&calldata_of(&h, carried))).unwrap_or_default();
    let receipt = match &confirm {
        Confirm::Included { status, block_number } => Receipt::Included { status: *status, block: *block_number },
        Confirm::NotYet => Receipt::NotYet,
        Confirm::Unreachable(e) => Receipt::Unheard(e.to_string()),
        Confirm::Split { has, not_yet } => Receipt::Split { has: has.clone(), not_yet: not_yet.clone() },
    };
    let anchored = matches!(confirm, Confirm::Included { status: 1, .. });
    let held = match &confirm {
        Confirm::NotYet => Some(latest_held(&txs, |t| node::tx_as_held(reads, chain, t))),
        _ => None,
    };
    let unheld = match (&confirm, &held, sender) {
        (Confirm::NotYet, Some(node::AsHeld::Nobody | node::AsHeld::NoNode), Some(who)) => unheld(reads, chain, &txs, &ids, nonce, who, carried),
        _ => Unheld::Wait,
    };
    let receipt = match &unheld {
        Unheld::Void => Receipt::Void,
        Unheld::Again(_) => Receipt::Unused,
        Unheld::Wait | Unheld::Unserved(_) => receipt,
    };
    let voided = matches!(unheld, Unheld::Void);
    let stuck = match unheld {
        Unheld::Again(s) | Unheld::Unserved(s) => Some(s),
        _ => held.and_then(node::AsHeld::held).and_then(|v| stuck_of(reads, chain, &txs, &v)),
    };
    let landing = sender.map(|s| Landing { urls: urls.clone(), registry: s.registry, from: s.from });
    Ok(Heard { tx, chain, txs, ids, confirmed: anchored, receipt, gas, calldata, stuck, voided, waited, landing, kept: None })
}

fn latest_held(txs: &[String], ask: impl Fn(&str) -> node::AsHeld) -> node::AsHeld {
    let mut unread = false;
    let mut no_node = false;
    for t in txs.iter().rev() {
        match ask(t) {
            node::AsHeld::Held(v) => return node::AsHeld::Held(v),
            node::AsHeld::Unread => unread = true,
            node::AsHeld::NoNode => no_node = true,
            node::AsHeld::Nobody => {}
        }
    }
    if unread {
        node::AsHeld::Unread
    } else if no_node {
        node::AsHeld::NoNode
    } else {
        node::AsHeld::Nobody
    }
}

pub fn settle<E: End>(end: &E, h: Heard) -> Result<Landed, E::Fail> {
    let Heard { tx, chain, txs, ids, confirmed, receipt, gas, calldata, stuck, mut voided, waited, landing: _, kept } = h;
    let (dropped, queue) = match &receipt {
        Receipt::Included { status: 1, block } => keep::amend(end, |q| {
            let ours: Vec<String> = ids.iter().filter(|id| q.step_on(id, chain).and_then(Step::awaited).is_some_and(|(t, c)| c == chain && t.contains(&tx))).cloned().collect();
            q.included_out(&ours, &tx, chain, *block)
        })?,
        Receipt::Included { .. } => keep::amend(end, |q| {
            q.moved_by(&ids, &txs, chain, Step::Reverted { tx: tx.clone(), chain });
            0
        })?,
        _ if voided => {
            let ((moved, resent), queue) = keep::amend(end, |q| {
                let moved = q.moved_by(&ids, &txs, chain, Step::Queued);
                let resent = ids.iter().any(|id| q.step_on(id, chain).and_then(Step::awaited).is_some_and(|(t, c)| c == chain && t.iter().any(|x| txs.contains(x))));
                (moved, resent)
            })?;
            voided = moved > 0 || !resent;
            (0, queue)
        }
        _ => (0, keep::read(end)?),
    };
    Ok(Landed { tx, chain, confirmed, receipt, sent: ids.len(), dropped, queue: queue.items, gas, calldata, stuck, voided, waited, kept })
}

pub fn landed_facts<F: Facts>(facts: &F, h: &mut Heard) {
    if let (Receipt::Included { status: 1, block }, Some(l)) = (&h.receipt, &h.landing) {
        h.kept = land(facts, h.chain, l, *block);
    }
}

fn land<F: Facts>(facts: &F, chain: u64, l: &Landing, block: u64) -> Option<Kept> {
    let eps: Vec<Endpoint> = l.urls.iter().map(|u| Endpoint::at(chain, u.clone())).collect();
    let senders = vec![zikaron::hexfmt::encode(&l.from)];
    let registry = zikaron::hexfmt::encode(&l.registry);
    let w = zikaron_glue::basis::Window { chain, registries: std::slice::from_ref(&registry), from_block: block, to_block: block, senders: &senders };
    match crate::agree::scan(&eps, &zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&w)), facts) {
        Ok(crate::agree::Scan::Basis(a)) => Some(Kept { anchors: a.fragment.member("anchors").and_then(zikaron::json::Value::as_arr).cloned().unwrap_or_default(), single_source: a.single_source }),
        _ => None,
    }
}

enum Unheld {
    Void,
    Again(Stuck),
    Wait,
    Unserved(Stuck),
}

#[allow(clippy::too_many_arguments)]
fn unheld(urls: &[NodeAddr], chain: u64, txs: &[String], ids: &[String], nonce: Option<u64>, who: Sender, carried: Option<&[u8]>) -> Unheld {
    let (on_chain, none) = match void_read(urls, chain, &who.from, txs, nonce) {
        Ok(read) => read,
        Err(code) if code == Code::NoEndpoint || code.another_chain() => {
            let why = Unpriced::of(urls, Err(code));
            return match Queue::hashes(ids) {
                Ok(h) => Unheld::Unserved(Stuck { txs: txs.to_vec(), chain, from: who.from, nonce: nonce.unwrap_or_default(), to: who.registry, input: calldata_of(&h, carried), sent: Fees::fallback(), offer: Offer::PriceUnread, left: Vec::new(), unpriced: Some(why) }),
                Err(_) => Unheld::Wait,
            };
        }
        Err(_) => (None, false),
    };
    let mine = nonce.or(on_chain).unwrap_or_default();
    match zikaron_anchor::send::unheld(on_chain, mine, || none) {
        zikaron_anchor::send::Unheld::Void => Unheld::Void,
        zikaron_anchor::send::Unheld::Wait => Unheld::Wait,
        zikaron_anchor::send::Unheld::Unused => match again_of(urls, chain, txs, ids, mine, who, carried) {
            Some(s) => Unheld::Again(s),
            None => Unheld::Wait,
        },
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum VoidSaid {
    Nonce(u64),
    Past(u64, Option<bool>),
    Unread,
}

fn void_read(urls: &[NodeAddr], chain: u64, from: &[u8; 20], txs: &[String], mine: Option<u64>) -> Result<(Option<u64>, bool), Code> {
    let (urls, _) = serving_urls(urls, chain).map_err(|t| t.code)?;
    let params = Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(from)), Value::Str("latest".into())]);
    let txs = txs.to_vec();
    let names = urls.clone();
    let got = node::each_node(
        &urls,
        &node::endpoint_at,
        move |_, ep| {
            let Some(ep) = ep else { return VoidSaid::Unread };
            let n = ep.call_within("eth_getTransactionCount", &params, node::ONE_ASK).ok().and_then(|w| match zikaron_anchor::wire::to_core(&w) {
                Some(Value::Str(x)) => node::wei(&x).and_then(|n| u64::try_from(n).ok()),
                _ => None,
            });
            match (n, mine) {
                (None, _) => VoidSaid::Unread,
                (Some(n), Some(m)) if n > m => {
                    let mut none = Some(true);
                    for t in &txs {
                        match ep.call("eth_getTransactionReceipt", &Value::Arr(vec![Value::Str(t.clone())])) {
                            Ok(w) if w.is_null() => {}
                            Ok(_) => {
                                none = Some(false);
                                break;
                            }
                            Err(_) => none = None,
                        }
                    }
                    VoidSaid::Past(n, none)
                }
                (Some(n), _) => VoidSaid::Nonce(n),
            }
        },
        |view| {
            view.iter().any(|v| matches!(v, Some(VoidSaid::Past(_, Some(false)))))
                || node::two_places(&names, view, |v| matches!(v, VoidSaid::Past(_, Some(true))))
                || node::two_places(&names, view, |v| matches!(v, VoidSaid::Nonce(_)))
        },
    );
    let said: Vec<(&NodeAddr, VoidSaid)> = urls.iter().zip(got).filter_map(|(u, v)| v.map(|v| (u, v))).collect();
    Ok(void_judged(&said))
}

fn void_judged(said: &[(&NodeAddr, VoidSaid)]) -> (Option<u64>, bool) {
    let nonce_of = |v: &VoidSaid| match v {
        VoidSaid::Nonce(n) | VoidSaid::Past(n, _) => Some(*n),
        VoidSaid::Unread => None,
    };
    let places = |pick: &dyn Fn(&VoidSaid) -> bool| -> Vec<&(&NodeAddr, VoidSaid)> { said.iter().filter(|(_, v)| pick(v)).collect() };
    let two = |xs: &[&(&NodeAddr, VoidSaid)]| zikaron_anchor::endpoints::distinct_places(xs.iter().map(|(u, _)| u.for_transport())) >= 2;
    if let Some((_, v)) = said.iter().find(|(_, v)| matches!(v, VoidSaid::Past(_, Some(false)))) {
        return (nonce_of(v), false);
    }
    let gone = places(&|v| matches!(v, VoidSaid::Past(_, Some(true))));
    if two(&gone) {
        return (gone.iter().filter_map(|(_, v)| nonce_of(v)).min(), true);
    }
    let short = places(&|v| matches!(v, VoidSaid::Nonce(_)));
    if two(&short) {
        return (short.iter().filter_map(|(_, v)| nonce_of(v)).min(), false);
    }
    let low = said.iter().filter_map(|(_, v)| nonce_of(v)).min();
    let none = said.iter().filter(|(_, v)| nonce_of(v).is_some()).all(|(_, v)| matches!(v, VoidSaid::Past(_, Some(true))));
    (low, low.is_some() && none)
}

fn again_of(urls: &[NodeAddr], chain: u64, txs: &[String], ids: &[String], nonce: u64, who: Sender, carried: Option<&[u8]>) -> Option<Stuck> {
    let _read = node::OneRead::open();
    let input = calldata_of(&Queue::hashes(ids).ok()?, carried);
    let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(chain, u.clone())).collect();
    let (eps, serving) = match serving_eps(&eps, chain) {
        Ok((kept, _)) => (kept, Ok(())),
        Err(t) => (Vec::new(), Err(t.code)),
    };
    let now = node::fee_reading(&eps, chain);
    let gas = zikaron_anchor::send::estimate_gas(
        || node::head_block(&eps, chain).map(|(height, _)| height).map_err(|t| (t, false)),
        |params| node::ask_call(&eps, "eth_estimateGas", params).map(|r| r.value),
        zikaron_anchor::send::estimate_call(&who.from, &who.registry, &input),
        |(_, about_the_call): &(Trouble, bool)| !about_the_call,
    )
    .ok();
    let offer = match (txs.len() > RESENDS_MAX, now.base, gas) {
        (true, _, _) => Offer::Spent,
        (false, Some(base_now), Some(gas)) => Offer::Unheld { fees: now.fees.with_estimate(gas), base_now },
        (false, _, _) => Offer::PriceUnread,
    };
    let sent = match &offer {
        Offer::Unheld { fees, .. } => *fees,
        _ => now.fees,
    };
    let unpriced = (offer == Offer::PriceUnread).then(|| Unpriced::of(urls, serving));
    Some(Stuck { txs: txs.to_vec(), chain, from: who.from, nonce, to: who.registry, input, sent, offer, left: now.left, unpriced })
}

fn text_member(v: &Value, k: &str) -> Option<String> {
    match v {
        Value::Obj(m) => m.iter().find(|(n, _)| n == k).and_then(|(_, v)| match v {
            Value::Str(x) => Some(x.clone()),
            _ => None,
        }),
        _ => None,
    }
}

fn stuck_of(urls: &[NodeAddr], chain: u64, txs: &[String], held: &Value) -> Option<Stuck> {
    let qty = |k: &str| text_member(held, k).as_deref().and_then(node::wei).and_then(|n| u64::try_from(n).ok());
    let addr = |k: &str| text_member(held, k).as_deref().and_then(zikaron::hexfmt::decode).and_then(|b| <[u8; 20]>::try_from(b).ok());
    let input = text_member(held, "input").as_deref().and_then(zikaron::hexfmt::decode)?;
    let sent = Fees { max_fee: qty("maxFeePerGas")?, priority: qty("maxPriorityFeePerGas")?, gas_limit: qty("gas")?, from_chain: true };
    let (from, nonce, to) = (addr("from")?, qty("nonce")?, addr("to")?);
    let (offer, left) = if txs.len() > RESENDS_MAX {
        (Offer::Spent, Vec::new())
    } else {
        let eps: Vec<Endpoint> = urls.iter().map(|u| Endpoint::at(chain, u.clone())).collect();
        let now = node::fee_reading(&eps, chain);
        let offer = match now.base {
            Some(base_now) if base_now > sent.max_fee => Offer::Resend { fees: Fees::replacing(sent, now.fees), base_now },
            Some(base_now) => Offer::NotAbove { base_now },
            None => Offer::PriceUnread,
        };
        (offer, now.left)
    };
    let unpriced = (offer == Offer::PriceUnread).then(|| if !urls.is_empty() && left.len() == urls.len() { Unpriced::OtherChain } else { Unpriced::of(urls, Ok(())) });
    Some(Stuck { txs: txs.to_vec(), chain, from, nonce, to, input, sent, offer, left, unpriced })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(answer: fn(&str) -> &'static str) -> NodeAddr {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}", l.local_addr().expect("addr"));
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                std::thread::spawn(move || {
                    let mut s = s;
                    let mut raw = Vec::new();
                    let mut buf = [0u8; 4096];
                    let body = loop {
                        match s.read(&mut buf) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => raw.extend_from_slice(&buf[..n]),
                        }
                        let text = String::from_utf8_lossy(&raw).to_string();
                        if let Some(i) = text.find("\r\n\r\n") {
                            let len: usize = text[..i].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|n| n.trim().parse().ok())).unwrap_or(0);
                            if raw.len() >= i + 4 + len {
                                break text[i + 4..].to_string();
                            }
                        }
                    };
                    let method = body.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                    let id = body.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                    let member = if method == "eth_chainId" { r#""result":"0x7a69""# } else { answer(&method) };
                    let body = format!("{{\"id\":{id},\"jsonrpc\":\"2.0\",{member}}}");
                    let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
                });
            }
        });
        NodeAddr::new(url)
    }

    #[test]
    fn a_batch_is_held_by_none_only_on_every_nodes_word() {
        let txs = vec!["0xa".to_string(), "0xb".to_string()];
        assert_eq!(latest_held(&txs, |_| node::AsHeld::Nobody), node::AsHeld::Nobody);
        assert_eq!(latest_held(&txs, |t| if t == "0xa" { node::AsHeld::Unread } else { node::AsHeld::Nobody }), node::AsHeld::Unread);
        assert_eq!(latest_held(&txs, |t| if t == "0xa" { node::AsHeld::Held(Value::Null) } else { node::AsHeld::Unread }), node::AsHeld::Held(Value::Null));
    }

    #[test]
    fn a_void_reads_only_the_nodes_whose_nonce_was_read_past_it() {
        crate::node::forget_serving();
        let past_silent = local(|m| if m == "eth_getTransactionCount" { r#""result":"0x6""# } else { r#""error":{"code":-32000,"message":"busy"}"# });
        let behind = local(|m| if m == "eth_getTransactionCount" { r#""error":{"code":-32000,"message":"busy"}"# } else { r#""result":null"# });
        let past_not_yet = local(|m| if m == "eth_getTransactionCount" { r#""result":"0x6""# } else { r#""result":null"# });
        let who = Sender { from: [0x22; 20], registry: [0x33; 20] };
        let txs = vec![format!("0x{}", "ab".repeat(32))];
        let ids = vec![format!("0x{}", "cd".repeat(32))];
        assert!(matches!(unheld(&[past_silent, behind.clone()], 31337, &txs, &ids, Some(5), who, None), Unheld::Wait), "the node past it silent: it waits");
        assert!(matches!(unheld(&[past_not_yet, behind], 31337, &txs, &ids, Some(5), who, None), Unheld::Void), "every node past it says not yet: void");
    }

    #[test]
    fn a_batch_goes_only_at_a_later_block_time() {
        assert_eq!(fence_open(100, &[(1, 100)]), Err(100), "the same second");
        assert_eq!(fence_open(101, &[(1, 100)]), Ok(()));
        assert_eq!(fence_open(0, &[]), Ok(()), "nothing before");
        assert_eq!(fence_open(150, &[(1, 100), (42161, 150)]), Err(150), "the latest on any chain");
        assert_eq!(fence_open(u64::MAX, &[(1, u64::MAX - 1)]), Ok(()));
        assert_eq!(fence_open(u64::MAX, &[(1, u64::MAX)]), Err(u64::MAX));
        zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
        let mut heads = [100u64, 100, 101].into_iter();
        assert_eq!(await_fence(&[(1, 100)], || Ok::<u64, ()>(heads.next().unwrap_or(100))), Ok(()), "opens once the head moves on");
        let mut asks = 0usize;
        assert_eq!(
            await_fence(&[(1, 100)], || {
                asks += 1;
                Ok::<u64, ()>(100)
            }),
            Err(Fenced::TooEarly { head_time: 100, after: 100 })
        );
        assert_eq!(asks, 1 + FENCE_PAUSES.len(), "bounded");
        assert_eq!(await_fence(&[(1, 100)], || Err::<u64, &str>("down")), Err(Fenced::Unread("down")));
        assert!(FENCE_PAUSES.iter().sum::<std::time::Duration>() >= std::time::Duration::from_secs(24), "two blocks of the slowest shipped chain");
    }

    #[test]
    fn what_a_home_has_out_is_read_by_the_transactions_awaited() {
        let id = |n: u8| zikaron::hexfmt::encode(&[n; 32]);
        let tx = |n: u8| zikaron::hexfmt::encode(&[0xf0 | n; 32]);
        let mut q = Queue { chain: 7, ..Queue::default() };
        for n in [1, 2, 3, 5] {
            let _ = q.push(&id(n), n as u64);
        }
        let _ = q.push_on(&id(4), 8, 4);
        assert_eq!(out_for(&q, &[id(1), id(2)]), None, "nothing out");
        q.mark(&[id(2), id(3)], Step::Submitted { tx: tx(1), chain: 7, nonce: Some(0) });
        q.mark_on(&[id(4)], 8, Step::Submitted { tx: tx(1), chain: 8, nonce: Some(0) });
        assert_eq!(out_for(&q, &[id(1), id(2)]), Some((vec![tx(1)], 7, vec![id(2), id(3)])), "the batch out, every entry of it");
        q.mark(&[id(5)], Step::Resent { txs: vec![tx(2), tx(3)], chain: 7, nonce: Some(1) });
        assert_eq!(out_for(&q, &[id(5)]), Some((vec![tx(2), tx(3)], 7, vec![id(5)])), "a resent batch, all its transactions");
        assert_eq!(out_on(&q, &[id(4)], 8), Some((vec![tx(1)], 8, vec![id(4)])), "the same transaction on another chain is another batch");
        assert_eq!(out_for(&q, &[id(4)]), None, "not out on the anchoring chain");
    }

    struct Mem(std::cell::RefCell<Queue>);

    impl From<zikaron_seal::Trouble> for Trouble {
        fn from(t: zikaron_seal::Trouble) -> Trouble {
            Trouble::text(Code::SettingsShape, format!("{:?} {}", t.code, t.tail))
        }
    }

    impl End for Mem {
        type Fail = Trouble;
        fn code_of(f: &Trouble) -> Option<String> {
            Some(f.code.as_str().to_string())
        }
        fn read_queue(&self) -> Result<Queue, Trouble> {
            Ok(self.0.borrow().clone())
        }
        fn write_queue(&self, q: &Queue) -> Result<(), Trouble> {
            *self.0.borrow_mut() = q.clone();
            Ok(())
        }
        fn pile(&self) -> Result<Vec<Vec<u8>>, Trouble> {
            Ok(Vec::new())
        }
        fn behind(&self, lacks: &[String]) -> Trouble {
            Trouble::text(Code::NewerElsewhere, lacks.len().to_string())
        }
        fn unread(&self, f: Trouble) -> Trouble {
            f
        }
        fn mark(&self) -> Result<Option<zikaron_seal::unfetched::State>, Trouble> {
            Ok(None)
        }
        fn lift(&self) -> Result<(), Trouble> {
            Ok(())
        }
        fn machine(&self) -> Result<std::path::PathBuf, Trouble> {
            Ok(std::path::PathBuf::new())
        }
    }

    impl Facts for Mem {
        fn known(&self) -> zikaron_anchor::scan::Known {
            zikaron_anchor::scan::Known::default()
        }
        fn add(&self, _runs: &[(Vec<u64>, zikaron_anchor::scan::Sightings)], _thin: &[u64]) {}
    }

    #[test]
    fn settling_moves_only_what_still_awaits_its_batch() {
        let id = |n: u8| zikaron::hexfmt::encode(&[n; 32]);
        let tx = |n: u8| zikaron::hexfmt::encode(&[0xf0 | n; 32]);
        let heard = |receipt: Receipt, voided: bool, ids: Vec<String>| Heard {
            tx: tx(1),
            chain: 7,
            txs: vec![tx(1)],
            ids,
            confirmed: matches!(receipt, Receipt::Included { status: 1, .. }),
            receipt,
            gas: None,
            calldata: String::new(),
            stuck: None,
            voided,
            waited: 0,
            landing: None,
            kept: None,
        };
        let fresh = || {
            let mut q = Queue { chain: 7, ..Queue::default() };
            for n in 1..=4 {
                let _ = q.push(&id(n), n as u64);
            }
            q.mark(&[id(1), id(2)], Step::Submitted { tx: tx(1), chain: 7, nonce: Some(0) });
            q.mark(&[id(3)], Step::Resent { txs: vec![tx(1), tx(2)], chain: 7, nonce: Some(0) });
            q.mark(&[id(4)], Step::Submitted { tx: tx(3), chain: 7, nonce: Some(1) });
            Mem(std::cell::RefCell::new(q))
        };
        let all = vec![id(1), id(2), id(3), id(4), id(5)];
        let m = fresh();
        let l = settle(&m, heard(Receipt::Included { status: 1, block: 9 }, false, all.clone())).expect("settled");
        assert_eq!(l.dropped, 3, "the two awaiting it and the one resent over it");
        let q = m.0.borrow();
        assert!(q.anchored_here(&id(1)) && q.anchored_here(&id(3)) && q.block_of(&id(2)).map(|b| b.block) == Some(9));
        assert_eq!(q.step_of(&id(4)), Some(&Step::Submitted { tx: tx(3), chain: 7, nonce: Some(1) }), "another batch stays");
        assert!(!q.anchored_here(&id(5)), "one not in the queue is not recorded");
        drop(q);
        let m = fresh();
        let l = settle(&m, heard(Receipt::Void, true, all.clone())).expect("settled");
        let q = m.0.borrow();
        assert_eq!((q.step_of(&id(1)), q.step_of(&id(2))), (Some(&Step::Queued), Some(&Step::Queued)), "void: back");
        assert!(q.step_of(&id(3)).is_some_and(Step::in_flight) && q.step_of(&id(4)).is_some_and(Step::in_flight), "others stay in flight");
        assert!(l.voided, "entries went back: answered as void");
        drop(q);
        let m = fresh();
        let l = settle(&m, heard(Receipt::Void, true, vec![id(3)])).expect("settled");
        assert!(m.0.borrow().step_of(&id(3)).is_some_and(Step::in_flight), "a resent batch stays as it is");
        assert!(!l.voided, "no entry went back: no void of theirs");
        let m = fresh();
        let first = settle(&m, heard(Receipt::Void, true, all.clone())).expect("settled");
        let second = settle(&m, heard(Receipt::Void, true, vec![id(1), id(2)])).expect("settled again");
        assert!(first.voided && second.voided, "both waits answer void");
        let m = fresh();
        settle(&m, heard(Receipt::Included { status: 0, block: 9 }, false, all.clone())).expect("settled");
        assert_eq!(m.0.borrow().step_of(&id(1)), Some(&Step::Reverted { tx: tx(1), chain: 7 }));
        assert!(m.0.borrow().step_of(&id(3)).is_some_and(Step::in_flight), "a resent batch awaits other transactions");
        let m = fresh();
        let before = m.0.borrow().to_bytes();
        settle(&m, heard(Receipt::NotYet, false, all)).expect("settled");
        assert_eq!(m.0.borrow().to_bytes(), before, "not yet: nothing written");
    }

    #[test]
    fn the_balance_gate_compares_at_its_edges() {
        assert!(funds_check(0, 0).is_ok());
        assert!(funds_check(10, 10).is_ok());
        assert_eq!(funds_check(10, 9), Err(Trouble::new(Code::InsufficientFunds, Why::Funds { need: 10, have: 9 })));
        assert!(funds_check(u128::MAX, u128::MAX).is_ok());
        assert_eq!(funds_check(u128::MAX, u128::MAX - 1).map_err(|t| t.code), Err(Code::InsufficientFunds));
    }

    fn rpc_node(answer: fn(&str) -> Option<&'static str>) -> NodeAddr {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}", l.local_addr().expect("addr"));
        std::thread::spawn(move || {
            for mut s in l.incoming().flatten() {
                let mut raw = Vec::new();
                let mut buf = [0u8; 4096];
                let body = loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => break None,
                        Ok(n) => raw.extend_from_slice(&buf[..n]),
                    }
                    let text = String::from_utf8_lossy(&raw).to_string();
                    if let Some(i) = text.find("\r\n\r\n") {
                        let len: usize = text[..i].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").and_then(|n| n.trim().parse().ok())).unwrap_or(0);
                        if raw.len() >= i + 4 + len {
                            break Some(text[i + 4..].to_string());
                        }
                    }
                };
                let Some(body) = body else { continue };
                let method = body.split("\"method\":\"").nth(1).and_then(|r| r.split('"').next()).unwrap_or("").to_string();
                let id = body.split("\"id\":").nth(1).and_then(|r| r.split([',', '}']).next()).unwrap_or("1").trim().to_string();
                let reply = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{}}}", answer(&method).unwrap_or("null"));
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}", reply.len()).as_bytes());
            }
        });
        NodeAddr::new(url)
    }

    #[test]
    fn a_carried_entry_offered_again_still_carries_its_bytes() {
        let who = Sender { from: [0x22; 20], registry: [0x11; 20] };
        let entry = b"{\"entryType\":\"zikaron.face/1\"}".to_vec();
        let ids = vec![zikaron::hexfmt::encode(&zikaron::cryptox::sha256(&entry))];
        let tx = format!("0x{}", "44".repeat(32));
        let carried = again_of(&[], 31337, std::slice::from_ref(&tx), &ids, 4, who, Some(&entry)).expect("a stuck batch");
        assert_eq!(carried.input, zikaron_anchor::send::carried_calldata(&entry));
        let plain = again_of(&[], 31337, std::slice::from_ref(&tx), &ids, 4, who, None).expect("a stuck batch");
        assert_eq!(plain.input, zikaron_anchor::send::registry_calldata(&[zikaron::cryptox::sha256(&entry)]));
    }

    #[test]
    fn a_stuck_batch_without_a_price_says_whether_no_node_of_the_chain_every_node_elsewhere_or_the_price_unread() {
        let who = Sender { from: [0x22; 20], registry: [0x11; 20] };
        let id = format!("0x{}", "33".repeat(32));
        let tx = format!("0x{}", "44".repeat(32));
        let ids = vec![id.clone()];
        let none = again_of(&[], 31337, std::slice::from_ref(&tx), &ids, 4, who, None).expect("a stuck batch");
        assert_eq!((none.offer.clone(), none.unpriced), (Offer::PriceUnread, Some(Unpriced::NoNode)));
        let elsewhere = rpc_node(|m| (m == "eth_chainId").then_some("\"0x1\""));
        let h = wait(vec![elsewhere], 31337, vec![tx.clone()], ids.clone(), None, std::time::Duration::ZERO, &[], Some(who), Some(4), None).expect("heard");
        let s = h.stuck.expect("a stuck batch");
        assert_eq!((s.offer, s.unpriced, h.receipt), (Offer::PriceUnread, Some(Unpriced::OtherChain), Receipt::NotYet));
        let unread = rpc_node(|m| match m {
            "eth_chainId" => Some("\"0x7a69\""),
            "eth_getTransactionCount" => Some("\"0x4\""),
            "eth_blockNumber" => Some("\"0x40\""),
            _ => None,
        });
        let h = wait(vec![unread], 31337, vec![tx], ids, None, std::time::Duration::ZERO, &[], Some(who), Some(4), None).expect("heard");
        let s = h.stuck.expect("a stuck batch");
        assert_eq!((s.offer, s.unpriced, h.receipt), (Offer::PriceUnread, Some(Unpriced::Unread), Receipt::Unused));
    }

    #[test]
    fn a_batchs_void_takes_two_places_and_a_receipt_anywhere_stops_it() {
        let (a, b, c) = (NodeAddr::new("https://a.test"), NodeAddr::new("https://b.test"), NodeAddr::new("https://c.test"));
        assert_eq!(void_judged(&[(&a, VoidSaid::Past(9, Some(true))), (&b, VoidSaid::Past(8, Some(true)))]), (Some(8), true), "two places gone");
        assert_eq!(void_judged(&[(&a, VoidSaid::Past(9, Some(true))), (&b, VoidSaid::Past(9, Some(false)))]), (Some(9), false), "a receipt there");
        assert_eq!(void_judged(&[(&a, VoidSaid::Nonce(5)), (&c, VoidSaid::Nonce(4))]), (Some(4), false), "two not past");
        assert_eq!(void_judged(&[(&a, VoidSaid::Past(9, Some(true))), (&b, VoidSaid::Nonce(5)), (&c, VoidSaid::Unread)]), (Some(5), false), "fewer than two alike: the lowest");
        assert_eq!(void_judged(&[(&a, VoidSaid::Past(9, Some(true))), (&b, VoidSaid::Past(9, None))]), (Some(9), false), "a receipt question unanswered");
        assert_eq!(void_judged(&[(&a, VoidSaid::Past(9, Some(true)))]), (Some(9), true), "a table of one: as it says");
        assert_eq!(void_judged(&[(&a, VoidSaid::Unread), (&b, VoidSaid::Unread)]), (None, false), "nothing read");
        let a2 = NodeAddr::new("https://a.test");
        assert_eq!(void_judged(&[(&a, VoidSaid::Past(9, Some(true))), (&a2, VoidSaid::Past(9, Some(true))), (&b, VoidSaid::Nonce(5))]), (Some(5), false), "one place twice is one");
    }
}
