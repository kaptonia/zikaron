//! The endpoint rule: a load-bearing reading is green only when several endpoints agree, and a single source
//! is flagged.
//!
//! An endpoint is one source. With one source the reading is still given, but it says so:
//! "unknown" and "one source says yes" look the same in bytes and are worth different things. On disagreement
//! there is no majority and no first-come: the disagreement is named. Endpoints relay the same chain, so two
//! different relays mean at least one of them describes another chain.

use zikaron::json::{canon_bytes, Value};

/// A multi-endpoint reading.
#[derive(Clone, Debug)]
pub struct Reading {
    /// The fragment all endpoints agreed on.
    pub fragment: Value,
    /// Endpoint names that gave this reading, in the given order.
    pub sources: Vec<String>,
    /// True when any chain has only one source.
    ///
    /// Two rounds are not two sources: a scan spans several chains, and a chain with one endpoint has no
    /// corroboration even if other chains have two.
    pub single_source: bool,
    /// The single-source chains, by ascending chain id.
    pub single_source_chains: Vec<u64>,
}

/// A disagreement between endpoints.
#[derive(Clone, Debug)]
pub struct Disagreement {
    pub sources: Vec<String>,
    /// The fragment bytes each endpoint gave, in source order.
    pub fragments: Vec<Vec<u8>>,
}

/// Converge several readings: full agreement stands, anything else is a disagreement.
///
/// `thin` lists the chains with a single endpoint, computed by the caller from each chain's endpoint count.
pub fn agree_over(runs: Vec<(String, Value)>, thin: Vec<u64>) -> Result<Reading, Disagreement> {
    crate::seam();
    let sources: Vec<String> = runs.iter().map(|(n, _)| n.clone()).collect();
    let bytes: Vec<Vec<u8>> = runs.iter().map(|(_, v)| canon_bytes(v)).collect();
    let all_same = bytes.windows(2).all(|w| w[0] == w[1]);
    if !all_same {
        return Err(Disagreement { sources, fragments: bytes });
    }
    let fragment = runs.into_iter().next().map(|(_, v)| v).unwrap_or(Value::Null);
    let mut thin = thin;
    thin.sort_unstable();
    thin.dedup();
    Ok(Reading { fragment, sources, single_source: !thin.is_empty(), single_source_chains: thin })
}

/// One endpoint's answer cut down to the facts a decision reads: an object keeps only the named members, in
/// the order named; any other answer (a string, `null`, an array) stands whole.
///
/// Endpoints answer the same transaction or block with members that differ by node (one adds
/// `blockTimestamp`, another orders or pads differently) and with values that move between two asks. Agreement
/// over the whole answer then fails on what no decision reads. The caller names the facts its decision uses
/// and agreement is asked over those alone; [`agree`] itself stays strict. A named member one endpoint lacks
/// is left out of its projection, so a missing fact still disagrees with a present one.
pub fn project(answer: &Value, facts: &[&str]) -> Value {
    match answer {
        Value::Obj(m) => Value::Obj(
            facts
                .iter()
                .filter_map(|f| m.iter().find(|(k, _)| k == f).map(|(k, v)| (k.clone(), v.clone())))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Convergence for one chain or without chain ids: fewer than two sources is single-source.
pub fn agree(runs: Vec<(String, Value)>) -> Result<Reading, Disagreement> {
    let thin = if distinct_places(runs.iter().map(|(n, _)| n.as_str())) < 2 { vec![0] } else { Vec::new() };
    agree_over(runs, thin)
}

/// How many places these names are (`zikaron_net::place_key`: the same node written twice, in another case or
/// with its default port spelled out, is one place). Every single-source verdict counts sources here.
pub fn distinct_places<'a>(names: impl IntoIterator<Item = &'a str>) -> usize {
    let mut keys: Vec<String> = names.into_iter().map(zikaron_net::place_key).collect();
    keys.sort();
    keys.dedup();
    keys.len()
}

/// The chains among `chains` that fewer than two distinct places answered for (`places`: chain id and the
/// address that answered), ascending. The one count behind every "single source" a scan or an audit says.
pub fn thin_chains(places: &[(u64, String)], chains: &[u64]) -> Vec<u64> {
    let mut thin: Vec<u64> = chains
        .iter()
        .copied()
        .filter(|c| distinct_places(places.iter().filter(|(x, _)| x == c).map(|(_, u)| u.as_str())) < 2)
        .collect();
    thin.sort_unstable();
    thin.dedup();
    thin
}

impl Reading {
    /// The reading's byte shape: the fragment plus its sources. The single-source field is always present.
    pub fn value(&self) -> Value {
        Value::Obj(vec![
            ("fragment".into(), self.fragment.clone()),
            ("singleSource".into(), Value::Bool(self.single_source)),
            (
                "singleSourceChains".into(),
                Value::Arr(self.single_source_chains.iter().map(|c| Value::Int(*c)).collect()),
            ),
            ("sources".into(), Value::Arr(self.sources.iter().map(|s| Value::Str(s.clone())).collect())),
        ])
    }
}

/// Run `work` on every item at once, each on a thread of its own, and give the results in the items' order.
/// The one place this crate starts threads: a question asked of every node ([`ask_each`]) and a scan run at
/// every node at once both come here. The system refusing a thread does not lose the item: it is worked on
/// this thread instead (slower, not wrong). A worker that panics gives `lost()` in its place.
pub fn each<T: Send, R: Send>(items: Vec<T>, work: impl Fn(T) -> R + Sync, lost: impl Fn() -> R) -> Vec<R> {
    if items.len() <= 1 {
        return items.into_iter().map(&work).collect();
    }
    let slots: Vec<std::sync::Mutex<Option<T>>> = items.into_iter().map(|t| std::sync::Mutex::new(Some(t))).collect();
    let take = |i: usize| slots[i].lock().unwrap_or_else(|e| e.into_inner()).take();
    std::thread::scope(|s| {
        let (work, take) = (&work, &take);
        let started: Vec<Option<std::thread::ScopedJoinHandle<'_, Option<R>>>> = (0..slots.len())
            .map(|i| std::thread::Builder::new().name(format!("zikaron-ask-{i}")).spawn_scoped(s, move || take(i).map(work)).ok())
            .collect();
        started
            .into_iter()
            .enumerate()
            .map(|(i, h)| match h {
                Some(h) => h.join().ok().flatten().unwrap_or_else(&lost),
                None => take(i).map(work).unwrap_or_else(&lost),
            })
            .collect()
    })
}

/// Ask every node one question at once (each with patience: `patience::ask`), and give the answers in the
/// nodes' order, each with its node's name. Every question starts at the same moment, so all of them share
/// one deadline's worth of time; the slowest node, not the sum of them, sets how long the question takes.
/// A node whose asking stopped short (its worker panicked) keeps its name: the names are put back by place.
pub fn ask_each(nodes: Vec<(String, Box<dyn crate::rpc::Endpoint + Send>)>, method: &str, params: &Value) -> Vec<(String, Result<crate::wire::W, crate::rpc::Trouble>)> {
    let names: Vec<String> = nodes.iter().map(|(n, _)| n.clone()).collect();
    each(
        nodes,
        |(_, mut ep)| crate::patience::ask(ep.as_mut(), method, params),
        || Err(crate::rpc::Trouble::Transport("asking this node stopped short".into())),
    )
    .into_iter()
    .zip(names)
    .map(|(got, name)| (name, got))
    .collect()
}
