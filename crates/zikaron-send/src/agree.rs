//! A scan reconciled by the endpoint rule. Each round scans every chain at its k-th endpoint, and rounds are
//! compared by the endpoint rule (`zikaron_anchor::endpoints::agree_over`); the single-source flag comes from the
//! endpoints that actually answered. No home, no ledger: this touches only the chain, so the exit gate, the check
//! page (no privileges) and the periodic self-audit share it, each assembling its own ledger half.

use crate::node::{address_said, endpoint_at, Endpoint};
use crate::{Code, Facts, Trouble, Why};
use zikaron::json::Value;
use zikaron_anchor::rpc;
use zikaron_anchor::scan;

/// The reconciled reading.
pub struct Agreed {
    /// The reconciled fragment, unchanged.
    pub fragment: Value,
    pub unanswered: Vec<Why>,
    /// How many endpoints were asked.
    pub asked: usize,
    /// The single-source flag, given by the endpoint rule; this layer does not count.
    pub single_source: bool,
}

/// The two outcomes of a scan: a basis, or the core's validation saying this is not a basis (that no-label
/// result is passed through unchanged).
pub enum Scan {
    Basis(Agreed),
    NoLabel { report: Value, unanswered: Vec<Why>, asked: usize },
}

/// A scan of the given basis bytes, reconciled by the endpoint rule. The facts `facts` holds spare questions;
/// once the endpoints agree, what every run read is handed to it with the chains only one endpoint answered,
/// and it keeps what it judges agreed (failing to keep them only costs extra questions next time, never this
/// scan's answer).
pub fn scan(eps: &[Endpoint], basis_bytes: &[u8], facts: &dyn Facts) -> Result<Scan, Trouble> {
    if eps.is_empty() {
        return Err(Trouble::new(Code::NoEndpoint, Why::NoNodes));
    }
    let r = match rounds_over(eps, basis_bytes, &facts.known()) {
        Ok(r) => r,
        Err(no_label) => return Ok(no_label),
    };
    let runs = r.sightings.clone();
    let thin = thin_from(&r.answered, &r.chains);
    let (reading, unanswered, asked) = r.converge()?;
    facts.add(&runs, &thin);
    Ok(Scan::Basis(Agreed {
        fragment: reading.fragment,
        unanswered,
        asked,
        single_source: reading.single_source,
    }))
}

/// What the rounds of one agreed scan returned, before they are reconciled.
struct Rounds {
    runs: Vec<(String, Value)>,
    /// Which endpoint each run asked on each chain (chain id and url), in run order.
    places: Vec<Vec<(u64, String)>>,
    unanswered: Vec<Why>,
    refused: Vec<scan::Refusal>,
    /// The endpoints that actually answered (chain id and url); the single-source flag counts them.
    answered: Vec<(u64, String)>,
    asked: usize,
    chains: Vec<u64>,
    /// For each run that answered, the chains it read and the facts it read afresh (handed to [`Facts::add`]).
    sightings: Vec<(Vec<u64>, scan::Sightings)>,
}

impl Rounds {
    /// Reconciles by the endpoint rule: no round answering is the scan's refusal; rounds that differ are a
    /// disagreement.
    fn converge(self) -> Result<(zikaron_anchor::endpoints::Reading, Vec<Why>, usize), Trouble> {
        if self.runs.is_empty() {
            return Err(Trouble::new(Code::ScanRefused, Why::Scan { unanswered: self.unanswered, refused: self.refused }));
        }
        let thin = thin_from(&self.answered, &self.chains);
        let empty = answered_empty(&self.runs, &self.places);
        let reading = zikaron_anchor::endpoints::agree_over(self.runs, thin).map_err(|d| {
            // Still a disagreement (no majority, no first-come wins); if one endpoint returned nothing where
            // another returned records, the evidence names it: it lacks history back to the start block (a node
            // that keeps only recent logs answers older windows empty).
            let why = if empty.is_empty() { Why::Differ { count: d.sources.len(), sources: d.sources.join(" ") } } else { Why::NoHistory { nodes: empty.join(" ") } };
            Trouble::new(Code::Disagree, why)
        })?;
        Ok((reading, self.unanswered, self.asked))
    }
}

/// The endpoints that answered a chain with no records (no anchor, no evidence) while another endpoint answered
/// the same chain with some: each by its URL, once, in run order.
fn answered_empty(runs: &[(String, Value)], places: &[Vec<(u64, String)>]) -> Vec<String> {
    let count = |v: &Value, chain: u64| -> usize {
        ["anchors", "evidence"]
            .iter()
            .filter_map(|k| match v.member(k) {
                Some(Value::Arr(a)) => Some(a.iter().filter(|x| matches!(x.member("chainId"), Some(Value::Int(c)) if *c == chain)).count()),
                _ => None,
            })
            .sum()
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

/// The largest number of endpoints on any chain: how many rounds a scan runs.
pub fn rounds(eps: &[Endpoint]) -> usize {
    let mut n = 1;
    for e in eps {
        n = n.max(eps.iter().filter(|x| x.chain == e.chain).count());
    }
    n
}

/// Which chains had only one endpoint answer this pass (this sets the single-source flag).
///
/// It counts endpoints that answered, not endpoints configured: with two configured and the second down, the
/// reading still has one source, and counting configured ones would wrongly claim that several sources agree.
/// Each chain is computed from the URLs that actually answered for it.
fn thin_from(answered: &[(u64, String)], chains: &[u64]) -> Vec<u64> {
    zikaron_anchor::endpoints::thin_chains(answered, chains)
}

/// The k-th node of a chain as written (the last one when there are fewer), for the transport.
fn nth_for(eps: &[Endpoint], chain: u64, k: usize) -> Option<String> {
    let mine: Vec<&str> = eps.iter().filter(|e| e.chain == chain).map(|e| e.url.for_transport()).collect();
    mine.get(k.min(mine.len().saturating_sub(1))).map(|u| (*u).to_string())
}

/// The k-th node of a chain (the last one when there are fewer), opened (`node::endpoint_at`); `None` when the chain
/// has none or its address does not read.
pub fn open_nth(eps: &[Endpoint], chain: u64, k: usize) -> Option<Box<dyn rpc::Endpoint + Send>> {
    endpoint_at(&nth_for(eps, chain, k)?)
}

/// How a scan refusal is written in evidence (`code:detail`). One answer.
pub fn scan_tail(r: &scan::Refusal) -> String {
    format!("{}:{}", r.code(), r.detail())
}

/// Each round scans every chain at its k-th endpoint. If the core's validation says this is not a basis, the
/// no-label scan comes back unchanged.
fn rounds_over(eps: &[Endpoint], basis_bytes: &[u8], known: &scan::Known) -> Result<Rounds, Scan> {
    let mut chains: Vec<u64> = eps.iter().map(|e| e.chain).collect();
    chains.sort_unstable();
    chains.dedup();

    let mut runs: Vec<(String, Value)> = Vec::new();
    let mut places: Vec<Vec<(u64, String)>> = Vec::new();
    let mut unanswered: Vec<Why> = Vec::new();
    let mut refused: Vec<scan::Refusal> = Vec::new();
    // The endpoints that actually answered this pass (chain id and url); the single-source flag counts them.
    let mut answered: Vec<(u64, String)> = Vec::new();
    let mut asked = 0usize;
    let mut sightings: Vec<(Vec<u64>, scan::Sightings)> = Vec::new();
    // Open every round's nodes in order; the rounds then run concurrently (`zikaron_anchor::endpoints::each`)
    // and results are taken in round order, so the scan's result never depends on which answered first.
    type Round = (Vec<Why>, Vec<(u64, String)>, Vec<(u64, Box<dyn rpc::Endpoint + Send>)>);
    let mut jobs: Vec<Round> = Vec::new();
    for k in 0..rounds(eps) {
        let mut said: Vec<Why> = Vec::new();
        let mut https: Vec<(u64, Box<dyn rpc::Endpoint + Send>)> = Vec::new();
        let mut used: Vec<(u64, String)> = Vec::new();
        for c in &chains {
            let Some(url) = nth_for(eps, *c, k) else { continue };
            match endpoint_at(&url) {
                Some(h) => {
                    used.push((*c, url));
                    https.push((*c, h));
                }
                None => said.push(Why::AtChain { chain: *c, why: Box::new(address_said(&url)) }),
            }
        }
        jobs.push((said, used, https));
    }
    let lost_chain = chains.first().copied().unwrap_or(0);
    let done = zikaron_anchor::endpoints::each(
        jobs,
        |(said, used, mut https)| {
            if https.is_empty() {
                return (said, used, None);
            }
            let mut handed: Vec<(u64, &mut dyn rpc::Endpoint)> = https
                .iter_mut()
                .map(|(c, h)| (*c, &mut **h as &mut dyn rpc::Endpoint))
                .collect();
            // Every round reads the same record: one round's new facts never spare another round a question.
            let got = scan::run_knowing(basis_bytes, &[], &mut handed, known);
            (said, used, Some(got))
        },
        || (Vec::new(), Vec::new(), Some(Err(scan::Refusal::Unanswered { chain: lost_chain, asked: scan::Asked::Short }))),
    );
    for (k, (said, used, got)) in done.into_iter().enumerate() {
        unanswered.extend(said);
        let Some(got) = got else { continue };
        let names: Vec<String> = used.iter().map(|(c, _)| format!("{c}#{k}")).collect();
        asked += used.len();
        match got {
            // The core's validation says this is not a basis: pass the no-label result through unchanged,
            // never wrapped as a successful scan.
            Ok(Err(no_label)) => return Err(Scan::NoLabel { report: no_label, unanswered, asked }),
            Ok(Ok((s, _, seen))) => {
                // This round actually answered, so its URLs count as sources for the single-source flag.
                answered.extend(used.iter().cloned());
                sightings.push((used.iter().map(|(c, _)| *c).collect(), seen));
                places.push(used.clone());
                runs.push((names.join(","), scan::fragment(&s)))
            }
            Err(r) => {
                unanswered.push(Why::Text(scan_tail(&r)));
                refused.push(r);
            }
        }
    }
    Ok(Rounds { runs, places, unanswered, refused, answered, asked, chains, sightings })
}
