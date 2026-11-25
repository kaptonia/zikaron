use crate::fingerprint::{self, Gate};
use crate::node::{self, Endpoint};
use crate::{Code, Trouble, Why};
use zikaron::json::Value;
use zikaron_anchor::revoke;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NowFrom {
    Injected,
    Chain,
    Absent,
}

impl NowFrom {
    pub fn as_str(self) -> &'static str {
        match self {
            NowFrom::Injected => "injected",
            NowFrom::Chain => "chain",
            NowFrom::Absent => "absent",
        }
    }
}

pub fn now_of(injected: Option<u64>, eps: &[Endpoint], chain: Option<u64>) -> (Option<u64>, NowFrom) {
    match injected {
        Some(n) => (Some(n), NowFrom::Injected),
        None => match chain.filter(|_| !eps.is_empty()).and_then(|c| node::head_time(eps, c).ok()) {
            Some((t, _, _)) => (Some(t), NowFrom::Chain),
            None => (None, NowFrom::Absent),
        },
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Read {
    pub value: Value,
    pub sources: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Unread {
    NoVenue,
    NoNode,
    Contract(Gate),
    Book,
    Trouble(Trouble),
}

impl Unread {
    pub fn code(&self) -> &'static str {
        match self {
            Unread::NoVenue => "no-venue",
            Unread::NoNode => "no-node",
            Unread::Contract(_) => "contract",
            Unread::Book => "book",
            Unread::Trouble(_) => "unread",
        }
    }
}

pub fn read(eps: &[Endpoint], chain: u64) -> Result<Read, Unread> {
    let Some(venue) = revoke::venue(chain) else { return Err(Unread::NoVenue) };
    if !eps.iter().any(|e| e.chain == chain) {
        return Err(Unread::NoNode);
    }
    let _read = node::OneRead::open();
    let (eps, _) = node::serving(eps, chain).map_err(Unread::Trouble)?;
    if eps.is_empty() {
        return Err(Unread::NoNode);
    }
    let (head, _) = node::head_block(&eps, chain).map_err(Unread::Trouble)?;
    let places: Vec<String> = eps.iter().map(|e| e.url.for_transport().to_string()).collect();
    let (gate, counted) = fingerprint::gate_counted(chain, &venue.contract, &places, revoke::CODE_HASH);
    if !gate.admits() {
        return Err(Unread::Contract(gate));
    }
    match crate::book::state(&eps, chain).map_err(Unread::Trouble)? {
        crate::book::State::At(_) => {}
        crate::book::State::Off => return Err(Unread::NoVenue),
        crate::book::State::Wrong => return Err(Unread::Book),
    }
    let mut refused: Vec<Why> = Vec::new();
    let mut opened: Vec<(String, Box<dyn zikaron_anchor::rpc::Endpoint + Send>)> = Vec::new();
    for url in &counted {
        match node::endpoint_at(url) {
            Some(ep) => opened.push((url.clone(), ep)),
            None => refused.push(node::address_said(url)),
        }
    }
    let (urls, eps): (Vec<String>, Vec<Box<dyn zikaron_anchor::rpc::Endpoint + Send>>) = opened.into_iter().unzip();
    let got = zikaron_anchor::scan::notices_agreed(eps, &venue, head);
    let read: Vec<(String, Value)> = got.read.into_iter().map(|(i, v)| (urls[i].clone(), v)).collect();
    for (i, r) in got.refused {
        refused.push(Why::Text(format!("{}: {}", urls[i], crate::agree::scan_tail(&r))));
    }
    let Some((_, first)) = read.first().cloned() else {
        return Err(Unread::Trouble(Trouble::new(Code::Unreachable, Why::Each(refused))));
    };
    let bytes = zikaron::json::canon_bytes(&first);
    if let Some((url, _)) = read.iter().find(|(_, v)| zikaron::json::canon_bytes(v) != bytes) {
        return Err(Unread::Trouble(Trouble::new(Code::Disagree, Why::Text(format!("{} / {url}", read[0].0)))));
    }
    Ok(Read { value: first, sources: read.len() })
}
