use crate::node::{address_said, endpoint_at, Endpoint, Open};
use crate::{Code, Facts, Trouble, Why};
use zikaron::json::Value;
use zikaron_anchor::rpc;
use zikaron_anchor::scan;

pub struct Agreed {
    pub fragment: Value,
    pub unanswered: Vec<Why>,
    pub differed: Vec<Why>,
    pub unproven: Vec<Why>,
    pub asked: usize,
    pub single_source: bool,
    pub single_source_chains: Vec<u64>,
    pub kept_single_chains: Vec<u64>,
}

pub enum Scan {
    Basis(Agreed),
    NoLabel { report: Value, unanswered: Vec<Why>, asked: usize },
}

pub fn scan(eps: &[Endpoint], basis_bytes: &[u8], facts: &dyn Facts) -> Result<Scan, Trouble> {
    scan_picked(eps, basis_bytes, facts, &scan::Pick::All)
}

pub fn scan_picked(eps: &[Endpoint], basis_bytes: &[u8], facts: &dyn Facts, pick: &scan::Pick) -> Result<Scan, Trouble> {
    let got = match by_chain(basis_bytes) {
        Some(parts) => scan_chains(eps, basis_bytes, &parts, facts, pick),
        None => scan_one(eps, basis_bytes, facts, pick),
    };
    got.map(|s| match s {
        Scan::Basis(a) => Scan::Basis(Agreed { unproven: unproven(), ..a }),
        other => other,
    })
}

fn unproven() -> Vec<Why> {
    let mut seen: Vec<(u64, String, rpc::ProofGap)> = Vec::new();
    for x in zikaron_anchor::ahead::unproven() {
        if !seen.contains(&x) {
            seen.push(x);
        }
    }
    seen.into_iter().map(|(chain, place, gap)| Why::AtChain { chain, why: Box::new(crate::node::node_why(&place, &rpc::Trouble::Transport(rpc::Broke::Proof(gap)))) }).collect()
}

fn by_chain(basis_bytes: &[u8]) -> Option<Vec<(u64, Vec<u8>)>> {
    let v = zikaron::json::parse(basis_bytes).ok()?;
    let chains = v.member("chains")?.as_arr()?;
    if chains.len() < 2 {
        return None;
    }
    let Value::Obj(members) = &v else { return None };
    chains
        .iter()
        .map(|c| {
            let id = match c.member("chainId") {
                Some(Value::Int(n)) => *n,
                _ => return None,
            };
            let one = Value::Obj(members.iter().map(|(k, x)| if k == "chains" { (k.clone(), Value::Arr(vec![c.clone()])) } else { (k.clone(), x.clone()) }).collect());
            Some((id, zikaron::json::canon_bytes(&one)))
        })
        .collect()
}

fn scan_chains(eps: &[Endpoint], basis_bytes: &[u8], parts: &[(u64, Vec<u8>)], facts: &dyn Facts, pick: &scan::Pick) -> Result<Scan, Trouble> {
    let (mut read, mut chains, mut unanswered, mut thin, mut kept_single) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut differed: Vec<Why> = Vec::new();
    let (mut asked, mut first) = (0usize, None);
    for (chain, bytes) in parts {
        let mine: Vec<Endpoint> = eps.iter().filter(|e| e.chain == *chain).cloned().collect();
        match scan_one(&mine, bytes, facts, pick) {
            Ok(Scan::Basis(a)) => {
                read.push(a.fragment.clone());
                if let Some(Value::Arr(xs)) = a.fragment.member("basis").and_then(|b| b.member("chains")) {
                    chains.extend(xs.iter().cloned());
                }
                asked += a.asked;
                unanswered.extend(a.unanswered);
                differed.extend(a.differed);
                thin.extend(a.single_source_chains);
                kept_single.extend(a.kept_single_chains);
            }
            Ok(no_label @ Scan::NoLabel { .. }) => return Ok(no_label),
            Err(t) => {
                unanswered.push(Why::AtChain { chain: *chain, why: Box::new(t.why.clone()) });
                first.get_or_insert(t);
            }
        }
    }
    if chains.is_empty() {
        return Err(first.unwrap_or_else(|| Trouble::new(Code::NoEndpoint, Why::NoNodes)));
    }
    let basis = match zikaron::json::parse(basis_bytes) {
        Ok(Value::Obj(members)) => Value::Obj(members.into_iter().map(|(k, x)| if k == "chains" { (k, Value::Arr(chains.clone())) } else { (k, x) }).collect()),
        _ => Value::Obj(vec![("chains".to_string(), Value::Arr(chains))]),
    };
    thin.sort_unstable();
    thin.dedup();
    kept_single.sort_unstable();
    kept_single.dedup();
    let fragment = scan::joined(&read, basis);
    Ok(Scan::Basis(Agreed { fragment, unanswered, differed, unproven: Vec::new(), asked, single_source: !thin.is_empty(), single_source_chains: thin, kept_single_chains: kept_single }))
}

fn scan_one(eps: &[Endpoint], basis_bytes: &[u8], facts: &dyn Facts, pick: &scan::Pick) -> Result<Scan, Trouble> {
    if eps.is_empty() {
        return Err(Trouble::new(Code::NoEndpoint, Why::NoNodes));
    }
    let r = match rounds_over(eps, basis_bytes, &known_for(eps, basis_bytes, facts), pick) {
        Ok(r) => r,
        Err(no_label) => return Ok(no_label),
    };
    let sightings = r.sightings.clone();
    let all_settlings = r.settlings.clone();
    let thin = r.thin();
    let held_back = r.held_back.clone();
    let (reading, unanswered, asked, kept, differed) = r.converge()?;
    let runs: Vec<_> = sightings.into_iter().enumerate().filter(|(i, _)| kept.contains(i)).map(|(_, x)| x).collect();
    let settlings: Vec<_> = all_settlings.into_iter().enumerate().filter(|(i, _)| kept.contains(i)).map(|(_, x)| x).collect();
    let kept_single = zikaron_anchor::settled::served_single(&settlings);
    facts.add(&runs, &thin);
    zikaron_anchor::settled::settle(&settlings, &held_back);
    Ok(Scan::Basis(Agreed {
        fragment: reading.fragment,
        unanswered,
        differed,
        unproven: Vec::new(),
        asked,
        single_source: reading.single_source,
        single_source_chains: reading.single_source_chains,
        kept_single_chains: kept_single,
    }))
}

fn known_for(eps: &[Endpoint], basis_bytes: &[u8], facts: &dyn Facts) -> scan::Known {
    let mut chains: Vec<u64> = eps.iter().map(|e| e.chain).collect();
    chains.sort_unstable();
    chains.dedup();
    let lists: Vec<(u64, String)> = chains.iter().map(|c| (*c, node_list(eps, *c))).collect();
    let served_single = scan::served_single_chains(basis_bytes, &lists);
    let alone: Vec<u64> = chains.into_iter().filter(|c| zikaron_anchor::endpoints::distinct_places(eps.iter().filter(|e| e.chain == *c).map(|e| e.url.for_transport())) < 2 || served_single.contains(c)).collect();
    let mut known = facts.known();
    if !alone.is_empty() {
        known.0.extend(facts.known_single().0.into_iter().filter(|(k, _)| alone.contains(&k.0) && !known.0.contains_key(k)).collect::<Vec<_>>());
    }
    known
}

struct Rounds {
    runs: Vec<(String, Value)>,
    places: Vec<Vec<(u64, String)>>,
    unanswered: Vec<Why>,
    refused: Vec<scan::Refusal>,
    answered: Vec<(u64, String)>,
    asked: usize,
    chains: Vec<u64>,
    sightings: Vec<(Vec<u64>, scan::Sightings)>,
    settlings: Vec<Vec<zikaron_anchor::settled::Settling>>,
    held_back: Vec<u64>,
}

impl Rounds {
    #[allow(clippy::type_complexity)]
    fn converge(self) -> Result<(zikaron_anchor::endpoints::Reading, Vec<Why>, usize, Vec<usize>, Vec<Why>), Trouble> {
        if self.runs.is_empty() {
            return Err(Trouble::new(Code::ScanRefused, Why::Scan { unanswered: self.unanswered, refused: self.refused }));
        }
        let thin = self.thin();
        let empty = answered_empty(&self.runs, &self.places);
        let names: Vec<String> = self.runs.iter().map(|(n, _)| n.clone()).collect();
        let urls: Vec<Vec<String>> = self.places.iter().map(|p| p.iter().map(|(_, u)| u.clone()).collect()).collect();
        let reading = zikaron_anchor::endpoints::agree_among(self.runs, &urls, thin.clone()).map_err(|d| {
            let why = if empty.is_empty() { Why::Differ { count: d.sources.len(), sources: d.sources.join(" ") } } else { Why::NoHistory { nodes: empty.join(" ") } };
            Trouble::new(Code::Disagree, why)
        })?;
        let left = zikaron_anchor::endpoints::left_out(&names, &reading);
        let kept: Vec<usize> = (0..names.len()).filter(|i| !left.contains(&names[*i])).collect();
        let differed: Vec<Why> = (0..names.len()).filter(|i| left.contains(&names[*i])).flat_map(|i| urls[i].iter().map(|u| Why::Differs { node: zikaron_net::sayable(u) })).collect();
        if kept.len() >= 2 && thin.is_empty() {
            return Ok((reading, Vec::new(), self.asked, kept, Vec::new()));
        }
        Ok((reading, self.unanswered, self.asked, kept, differed))
    }
}

impl Rounds {
    fn thin(&self) -> Vec<u64> {
        let mut thin = thin_from(&self.answered, &self.chains);
        thin.sort_unstable();
        thin.dedup();
        thin
    }
}

fn answered_empty(runs: &[(String, Value)], places: &[Vec<(u64, String)>]) -> Vec<String> {
    let count = |v: &Value, chain: u64| -> usize {
        match v.member("anchors") {
            Some(Value::Arr(a)) => a.iter().filter(|x| matches!(x.member("chainId"), Some(Value::Int(c)) if *c == chain)).count(),
            _ => 0,
        }
    };
    let mut out: Vec<String> = Vec::new();
    for (i, (_, v)) in runs.iter().enumerate() {
        for (chain, url) in places.get(i).map(|p| p.as_slice()).unwrap_or(&[]) {
            let others = runs.iter().enumerate().any(|(j, (_, w))| j != i && places.get(j).map(|p| p.iter().any(|(c, _)| c == chain)).unwrap_or(false) && count(w, *chain) > 0);
            if count(v, *chain) == 0 && others && !out.contains(url) {
                out.push(url.clone());
            }
        }
    }
    out
}

pub fn node_list(eps: &[Endpoint], chain: u64) -> String {
    zikaron_anchor::settled::nodes_of(zikaron_anchor::endpoints::places_of(eps.iter().filter(|e| e.chain == chain).map(|e| e.url.for_transport())).iter().map(String::as_str))
}

pub fn rounds(eps: &[Endpoint]) -> usize {
    let mut n = 1;
    for e in eps {
        n = n.max(eps.iter().filter(|x| x.chain == e.chain).count());
    }
    n
}

fn thin_from(answered: &[(u64, String)], chains: &[u64]) -> Vec<u64> {
    zikaron_anchor::endpoints::thin_chains(answered, chains)
}

pub fn nth_for(eps: &[Endpoint], chain: u64, k: usize) -> Option<String> {
    let mine: Vec<&str> = eps.iter().filter(|e| e.chain == chain).map(|e| e.url.for_transport()).collect();
    mine.get(k.min(mine.len().saturating_sub(1))).map(|u| (*u).to_string())
}

pub fn open_nth(eps: &[Endpoint], chain: u64, k: usize) -> Option<Box<dyn rpc::Endpoint + Send>> {
    endpoint_at(&nth_for(eps, chain, k)?)
}

pub fn scan_tail(r: &scan::Refusal) -> String {
    format!("{}:{}", r.code(), r.detail())
}

fn round_refused(used: &[(u64, String)], r: &scan::Refusal) -> Why {
    use scan::{Asked, Refusal};
    let (chain, method, trouble) = match r {
        Refusal::Unanswered { chain, asked: Asked::Declined { method, said, .. } } => (*chain, method, rpc::Trouble::Node(said.clone())),
        Refusal::Unanswered { chain, asked: Asked::Broke { method, broke } } => (*chain, method, rpc::Trouble::Transport(broke.clone())),
        Refusal::Unanswered { chain, asked: Asked::OverAsks { .. } } => {
            return match used.iter().find(|(c, _)| c == chain) {
                Some((_, url)) => Why::Text(format!("{} {}", zikaron_net::sayable(url), scan_tail(r))),
                None => Why::Text(scan_tail(r)),
            };
        }
        _ => return Why::Text(scan_tail(r)),
    };
    match used.iter().find(|(c, _)| *c == chain) {
        Some((_, url)) => Why::AtChain { chain, why: Box::new(Why::Each(vec![Why::Text(method.clone()), crate::node::node_why(url, &trouble)])) },
        None => Why::Text(scan_tail(r)),
    }
}

type RoundGot = (Vec<Why>, Vec<(u64, String)>, Option<Result<Ran, scan::Refusal>>);

type Ran = Result<(scan::Scanned, scan::Emitters, scan::Sightings, Vec<zikaron_anchor::settled::Settling>), Value>;

fn two_rounds_alike(slots: &[Option<RoundGot>]) -> bool {
    let read: Vec<(Vec<String>, Vec<u8>)> = slots
        .iter()
        .flatten()
        .filter_map(|(_, used, got)| match got {
            Some(Ok(Ok((s, ..)))) => Some((used.iter().map(|(_, u)| u.clone()).collect(), zikaron::json::canon_bytes(&scan::fragment(s)))),
            _ => None,
        })
        .collect();
    read.iter().any(|(_, b)| zikaron_anchor::endpoints::distinct_places(read.iter().filter(|(_, x)| x == b).flat_map(|(p, _)| p.iter().map(String::as_str))) >= 2)
        || read.iter().any(|(p, _)| p.iter().any(|u| zikaron_anchor::action::splits(&zikaron_anchor::endpoints::place_of(u))))
}

fn rounds_over(eps: &[Endpoint], basis_bytes: &[u8], known: &scan::Known, pick: &scan::Pick) -> Result<Rounds, Scan> {
    rounds_over_with(eps, basis_bytes, known, &endpoint_at, pick)
}

fn rounds_over_with(eps: &[Endpoint], basis_bytes: &[u8], known: &scan::Known, open: &Open<'_>, pick: &scan::Pick) -> Result<Rounds, Scan> {
    let mut chains: Vec<u64> = eps.iter().map(|e| e.chain).collect();
    chains.sort_unstable();
    chains.dedup();

    let mut runs: Vec<(String, Value)> = Vec::new();
    let mut places: Vec<Vec<(u64, String)>> = Vec::new();
    let mut unanswered: Vec<Why> = Vec::new();
    let mut refused: Vec<scan::Refusal> = Vec::new();
    let mut answered: Vec<(u64, String)> = Vec::new();
    let mut asked = 0usize;
    let mut sightings: Vec<(Vec<u64>, scan::Sightings)> = Vec::new();
    let mut settlings: Vec<Vec<zikaron_anchor::settled::Settling>> = Vec::new();
    let mut held_back: Vec<u64> = Vec::new();
    let lists: Vec<(u64, String)> = chains.iter().map(|c| (*c, node_list(eps, *c))).collect();
    type Round = (Vec<Why>, Vec<(u64, String)>, Vec<(u64, Box<dyn rpc::Endpoint + Send>)>);
    let build = |k: usize, held_back: &mut Vec<u64>| -> Round {
        let mut said: Vec<Why> = Vec::new();
        let mut https: Vec<(u64, Box<dyn rpc::Endpoint + Send>)> = Vec::new();
        let mut used: Vec<(u64, String)> = Vec::new();
        for c in &chains {
            let Some(url) = nth_for(eps, *c, k) else { continue };
            match open(&url) {
                Some(h) => {
                    used.push((*c, url));
                    https.push((*c, h));
                }
                None => {
                    held_back.push(*c);
                    said.push(Why::AtChain { chain: *c, why: Box::new(address_said(&url)) })
                }
            }
        }
        (said, used, https)
    };
    let jobs: Vec<Round> = (0..rounds(eps)).map(|k| build(k, &mut held_back)).collect();
    let lost_chain = chains.first().copied().unwrap_or(0);
    let (basis_bytes, known, lists, pick) = (std::sync::Arc::new(basis_bytes.to_vec()), std::sync::Arc::new(known.clone()), std::sync::Arc::new(lists.clone()), std::sync::Arc::new(pick.clone()));
    let run = |jobs: Vec<Round>| {
        let (basis_bytes, known, lists, pick) = (basis_bytes.clone(), known.clone(), lists.clone(), pick.clone());
        zikaron_anchor::endpoints::until_enough(
            jobs,
            move |(said, used, mut https)| {
                if https.is_empty() {
                    return (said, used, None);
                }
                let mut handed: Vec<(u64, &mut dyn rpc::Endpoint)> = https
                    .iter_mut()
                    .map(|(c, h)| (*c, &mut **h as &mut dyn rpc::Endpoint))
                    .collect();
                let got = scan::run_picked(&basis_bytes, &mut handed, &known, &lists, &pick);
                (said, used, Some(got))
            },
            || (Vec::new(), Vec::new(), Some(Err(scan::Refusal::Unanswered { chain: lost_chain, asked: scan::Asked::Short }))),
            two_rounds_alike,
        )
    };
    let mut results: Vec<Option<RoundGot>> = run(jobs);
    zikaron_anchor::action::cut_and_ask(
        &mut results,
        |_, r| r.iter().flat_map(|r| r.1.iter().map(|(_, u)| zikaron_anchor::endpoints::place_of(u))).collect(),
        |r| matches!(r, Some((_, _, Some(Ok(_))))),
        |r| matches!(r, Some((_, _, Some(Err(e)))) if e.cut()),
        |idx, _| run(idx.iter().map(|k| build(*k, &mut held_back)).collect()),
    );
    for (k, (said, used, got)) in results.into_iter().enumerate().filter_map(|(k, r)| r.map(|r| (k, r))) {
        unanswered.extend(said);
        let Some(got) = got else { continue };
        let names: Vec<String> = used.iter().map(|(c, _)| format!("{c}#{k}")).collect();
        asked += used.len();
        match got {
            Ok(Err(no_label)) => return Err(Scan::NoLabel { report: no_label, unanswered, asked }),
            Ok(Ok((s, _, seen, settled))) => {
                answered.extend(used.iter().cloned());
                sightings.push((used.iter().map(|(c, _)| *c).collect(), seen));
                settlings.push(settled);
                places.push(used.clone());
                runs.push((names.join(","), scan::fragment(&s)))
            }
            Err(r) => {
                if !r.stands_aside() {
                    held_back.extend(used.iter().map(|(c, _)| *c));
                }
                unanswered.push(round_refused(&used, &r));
                refused.push(r);
            }
        }
    }
    let anchors = |v: &Value| match v.member("anchors") {
        Some(Value::Arr(a)) => a.clone(),
        _ => Vec::new(),
    };
    let rest_round = |i: usize| places.get(i).is_some_and(|p| !p.is_empty() && p.iter().all(|(_, u)| zikaron_anchor::rest::of_lane(u).is_some()));
    let block_of = |a: &Value| match a.member("blockNumber") {
        Some(Value::Int(n)) => Some(*n),
        _ => None,
    };
    let behind: Vec<usize> = (0..runs.len())
        .filter(|i| rest_round(*i) && (0..runs.len()).any(|j| j != *i && runs[j].1.member("basis") == runs[*i].1.member("basis") && zikaron_anchor::endpoints::only_behind(&anchors(&runs[*i].1), &anchors(&runs[j].1), block_of)))
        .collect();
    if !behind.is_empty() && behind.len() < runs.len() {
        for i in behind.iter().rev() {
            let used = places.remove(*i);
            runs.remove(*i);
            sightings.remove(*i);
            settlings.remove(*i);
            for (c, url) in used {
                held_back.push(c);
                answered.retain(|(ac, au)| !(*ac == c && *au == url));
                unanswered.push(Why::AtChain { chain: c, why: Box::new(crate::node::node_why(&url, &rpc::Trouble::Transport(rpc::Broke::LogsBehind { node: zikaron_net::sayable(&url) }))) });
            }
        }
    }
    held_back.sort_unstable();
    held_back.dedup();
    Ok(Rounds { runs, places, unanswered, refused, answered, asked, chains, sightings, settlings, held_back })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use zikaron_anchor::rpc::Trouble as Said;
    use zikaron_anchor::wire::W;

    type Asked = Arc<Mutex<Vec<String>>>;

    #[derive(Clone)]
    struct Lane {
        name: String,
        bounded: bool,
        asked: Asked,
        fails: bool,
        says: Option<u64>,
    }

    impl rpc::Endpoint for Lane {
        fn call(&mut self, method: &str, params: &Value) -> Result<W, Said> {
            let span = zikaron_anchor::patience::log_span(method, params).unwrap_or(0);
            self.asked.lock().unwrap_or_else(|e| e.into_inner()).push(format!("{} {method} {span}", self.name));
            let text = match method {
                "eth_getLogs" if self.fails => return Err(Said::Transport(rpc::Broke::Late { node: self.name.clone(), deadline: std::time::Duration::from_millis(1) })),
                "eth_chainId" => "\"0x7a69\"",
                "eth_blockNumber" => "\"0x400\"",
                "eth_getLogs" if self.says.is_some_and(|n| span > n) => return Err(Said::Node(format!("{{\"code\":-32000,\"message\":\"exceed maximum block range: {}\"}}", self.says.unwrap_or(0)))),
                "eth_getLogs" if self.bounded && span > 100 => return Err(Said::Node("{\"code\":-32012,\"message\":\"getLogs request exceeded max allowed range\"}".into())),
                "eth_getLogs" => "[]",
                "eth_getCode" => "\"0x\"",
                "eth_getBlockByNumber" => "{\"number\":\"0x400\",\"hash\":\"0xaa\",\"timestamp\":\"0x66e00000\"}",
                _ => "null",
            };
            Ok(zikaron_anchor::wire::parse(text.as_bytes()).expect("an answer"))
        }
        fn name(&self) -> String {
            self.name.clone()
        }
        fn place(&self) -> String {
            self.name.clone()
        }
    }

    #[test]
    fn a_rest_rows_round_runs_with_the_json_rpc_rounds() {
        zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
        let senders = vec![zikaron::hexfmt::encode(&[0x22u8; 20])];
        let registry = zikaron::hexfmt::encode(&[0x11u8; 20]);
        let w = zikaron_glue::basis::Window { chain: 31337, registries: std::slice::from_ref(&registry), from_block: 0, to_block: 0x400, senders: &senders };
        let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&w));
        for (form, bounded, runs) in [("all answer", false, 2), ("one refuses the range", true, 2)] {
            let asked: Asked = Arc::default();
            let lanes: Vec<Lane> = [(format!("a-16-{bounded}"), bounded), (format!("b-16-{bounded}"), false), (format!("row-16-{bounded}"), false)]
                .into_iter()
                .filter(|(name, _)| bounded || !name.starts_with("a-"))
                .map(|(name, bounded)| Lane { name, bounded, asked: asked.clone(), fails: false, says: None })
                .collect();
            let eps: Vec<Endpoint> = lanes.iter().map(|l| Endpoint::at(31337, crate::node::NodeAddr::new(l.name.as_str()))).collect();
            let open = move |u: &str| lanes.iter().find(|l| l.name == u).map(|l| Box::new(l.clone()) as Box<dyn rpc::Endpoint + Send>);
            let Ok(r) = rounds_over_with(&eps, &basis, &scan::Known::default(), &open, &scan::Pick::All) else { panic!("{form}: a basis") };
            let row_asked = asked.lock().unwrap_or_else(|e| e.into_inner()).iter().any(|l| l.starts_with("row-16"));
            assert!(row_asked, "{form}: the row's round runs");
            assert_eq!(r.runs.len(), runs, "{form}");
            let (_, unanswered, _, _, differed) = r.converge().unwrap_or_else(|t| panic!("{form}: {t:?}"));
            assert!(unanswered.is_empty() && differed.is_empty(), "{form}: a scan two rounds read alike names nobody: {unanswered:?}");
        }
    }

    struct Mem(Mutex<std::collections::BTreeMap<String, Vec<u8>>>);

    impl zikaron_anchor::settled::Keep for Mem {
        fn load(&self, part: &str) -> Option<Vec<u8>> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).get(part).cloned()
        }
        fn save(&self, part: &str, plain: &[u8]) -> Result<(), zikaron_anchor::settled::Unkept> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).insert(part.to_string(), plain.to_vec());
            Ok(())
        }
        fn parts(&self) -> Vec<String> {
            self.0.lock().unwrap_or_else(|e| e.into_inner()).keys().cloned().collect()
        }
    }

    #[test]
    fn a_scan_keeps_what_the_answering_rounds_read_when_another_stands_aside() {
        zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
        let senders = vec![zikaron::hexfmt::encode(&[0x22u8; 20])];
        let registry = zikaron::hexfmt::encode(&[0x11u8; 20]);
        let w = zikaron_glue::basis::Window { chain: 31337, registries: std::slice::from_ref(&registry), from_block: 0, to_block: 0x400, senders: &senders };
        let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&w));
        for (form, fails) in [("one refuses the range", false), ("one fails now", true)] {
            let keep = Arc::new(Mem(Mutex::new(Default::default())));
            let asked: Asked = Arc::default();
            let lanes: Vec<Lane> = vec![
                Lane { name: format!("a-aside-{fails}"), bounded: !fails, asked: asked.clone(), fails, says: None },
                Lane { name: format!("b-aside-{fails}"), bounded: false, asked: asked.clone(), fails: false, says: None },
            ];
            let eps: Vec<Endpoint> = lanes.iter().map(|l| Endpoint::at(31337, crate::node::NodeAddr::new(l.name.as_str()))).collect();
            let open = move |u: &str| lanes.iter().find(|l| l.name == u).map(|l| Box::new(l.clone()) as Box<dyn rpc::Endpoint + Send>);
            {
                let _a = zikaron_anchor::action::open(Some(keep.clone()));
                let Ok(r) = rounds_over_with(&eps, &basis, &scan::Known::default(), &open, &scan::Pick::All) else { panic!("{form}: a basis") };
                assert_eq!((r.runs.len(), r.held_back.is_empty()), (1, !fails), "{form}");
                zikaron_anchor::settled::settle(&r.settlings, &r.held_back);
            }
            let kept: Vec<bool> = zikaron_anchor::settled::read_whole(&*keep).entries.values().map(|e| e.single_source).collect();
            if fails {
                assert!(kept.is_empty(), "{form}: nothing kept");
                continue;
            }
            assert!(!kept.is_empty() && kept.iter().all(|single| *single), "{form}: kept, single-source: {kept:?}");
            asked.lock().unwrap_or_else(|e| e.into_inner()).clear();
            {
                let _a = zikaron_anchor::action::open(Some(keep.clone()));
                assert_eq!(scan::served_single_chains(&basis, &[(31337, node_list(&eps, 31337))]), vec![31337], "{form}");
                let Ok(r) = rounds_over_with(&eps, &basis, &scan::Known::default(), &open, &scan::Pick::All) else { panic!("{form}: a basis") };
                assert_eq!(r.runs.len(), 2, "{form}: the short range is every node's");
                assert!(!r.thin().contains(&31337), "{form}: both nodes answered the blocks read now: no single-source reading");
                assert_eq!(zikaron_anchor::settled::served_single(&r.settlings), vec![31337], "{form}: its history named as one node's");
            }
            let spans: Vec<String> = asked.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|l| l.contains("eth_getLogs")).cloned().collect();
            assert!(!spans.is_empty() && spans.iter().all(|l| l.ends_with(" 64")), "{form}: each node asked only the blocks after the kept part: {spans:?}");
        }
    }
    #[test]
    fn every_round_refused_is_read_in_pieces_and_one_read_whole_is_enough() {
        zikaron_anchor::patience::set_waits(Some(std::time::Duration::ZERO));
        let senders = vec![zikaron::hexfmt::encode(&[0x22u8; 20])];
        let registry = zikaron::hexfmt::encode(&[0x11u8; 20]);
        let w = zikaron_glue::basis::Window { chain: 31337, registries: std::slice::from_ref(&registry), from_block: 0, to_block: 0x400, senders: &senders };
        let basis = zikaron::json::canon_bytes(&zikaron_glue::basis::basis_of(&w));
        let asked: Asked = Arc::default();
        let lanes: Vec<Lane> = vec![
            Lane { name: "widest-200".into(), bounded: false, asked: asked.clone(), fails: false, says: Some(200) },
            Lane { name: "widest-300".into(), bounded: false, asked: asked.clone(), fails: false, says: Some(300) },
            Lane { name: "widest-unsaid".into(), bounded: true, asked: asked.clone(), fails: false, says: None },
        ];
        let eps: Vec<Endpoint> = lanes.iter().map(|l| Endpoint::at(31337, crate::node::NodeAddr::new(l.name.as_str()))).collect();
        let open = move |u: &str| lanes.iter().find(|l| l.name == u).map(|l| Box::new(l.clone()) as Box<dyn rpc::Endpoint + Send>);
        let _a = zikaron_anchor::action::open(None);
        let Ok(r) = rounds_over_with(&eps, &basis, &scan::Known::default(), &open, &scan::Pick::All) else { panic!("a basis") };
        assert!(!r.runs.is_empty(), "read: {:?}", r.unanswered);
        assert!(r.held_back.is_empty(), "the others stand aside: {:?}", r.unanswered);
        let spans = |name: &str| -> Vec<u64> { asked.lock().unwrap_or_else(|e| e.into_inner()).iter().filter(|l| l.starts_with(&format!("{name} eth_getLogs "))).filter_map(|l| l.rsplit(' ').next().and_then(|n| n.parse::<u64>().ok())).collect() };
        for name in ["widest-200", "widest-300", "widest-unsaid"] {
            assert_eq!(spans(name)[..2], [0x401, 0x401], "{name} asked the whole range twice first: {:?}", spans(name));
        }
        assert!([("widest-200", 200), ("widest-300", 300), ("widest-unsaid", 100)].iter().any(|(n, bound)| spans(n).iter().filter(|w| *w <= bound).sum::<u64>() == 0x401), "one node read the window in pieces it answered");
    }
}
