//! The endpoint rule: a load-bearing reading is green only when several endpoints agree, and a single source
//! is flagged.
//!
//! An endpoint is one source. With one source the reading is still given, but it says so:
//! "unknown" and "one source says yes" look the same in bytes and are worth different things. On disagreement
//! there is no majority and no first-come: the disagreement is named. Endpoints relay the same chain, so two
//! different relays mean at least one of them describes another chain.

use zikaron::json::{canon_bytes, Value};

/// A multi-endpoint reading.
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

/// Convergence for one chain or without chain ids: fewer than two sources is single-source.
pub fn agree(runs: Vec<(String, Value)>) -> Result<Reading, Disagreement> {
    let thin = if runs.len() < 2 { vec![0] } else { Vec::new() };
    agree_over(runs, thin)
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
