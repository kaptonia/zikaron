use crate::node;
use zikaron::json::Value;
use zikaron_anchor::rpc;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Gate {
    Agreed(usize),
    Single,
    Down(Vec<Miss>),
    Fingerprint,
}

impl Gate {
    pub fn admits(&self) -> bool {
        matches!(self, Gate::Agreed(_) | Gate::Single)
    }

    pub fn code(&self) -> &'static str {
        match self {
            Gate::Agreed(_) => "agreed",
            Gate::Single => "single",
            Gate::Down(_) => "down",
            Gate::Fingerprint => "fingerprint",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Miss {
    pub url: String,
    pub why: Why,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Why {
    NotAddress,
    Trouble(rpc::Trouble),
    OtherChain(String),
    NoCode,
}

fn code_at(url: &str, chain: u64, registry: &[u8; 20]) -> Result<Vec<u8>, Why> {
    let Some(mut ep) = node::endpoint_at(url) else { return Err(Why::NotAddress) };
    let served = zikaron_anchor::patience::ask(ep.as_mut(), "eth_chainId", &Value::Arr(Vec::new())).map_err(Why::Trouble)?;
    if !zikaron_anchor::scan::endpoint_serves(&served, chain) {
        return Err(Why::OtherChain(served.as_str().unwrap_or("?").to_string()));
    }
    let params = Value::Arr(vec![Value::Str(zikaron::hexfmt::encode(registry)), Value::Str("latest".to_string())]);
    let code = zikaron_anchor::patience::ask(ep.as_mut(), "eth_getCode", &params).map_err(Why::Trouble)?;
    code.as_str().and_then(zikaron::hexfmt::decode).ok_or(Why::NoCode)
}

pub fn gate(chain: u64, registry: &[u8; 20], nodes: &[String], pinned: &str) -> Gate {
    gate_counted(chain, registry, nodes, pinned).0
}

pub fn gate_counted(chain: u64, registry: &[u8; 20], nodes: &[String], pinned: &str) -> (Gate, Vec<String>) {
    let mut counted: Vec<String> = Vec::new();
    let mut empty = 0usize;
    let mut misses: Vec<Miss> = Vec::new();
    let (reg, pin) = (*registry, pinned.to_string());
    let read = zikaron_anchor::endpoints::until_enough(
        nodes.to_vec(),
        move |url| (url.clone(), code_at(&url, chain, &reg).map(|bytes| (!bytes.is_empty()).then(|| zikaron::hexfmt::encode(&zikaron::cryptox::keccak256(&bytes))))),
        || (String::new(), Err(Why::Trouble(rpc::Trouble::Transport(rpc::Broke::Short)))),
        |slots: &[Option<(String, Result<Option<String>, Why>)>]| {
            slots.iter().flatten().any(|(_, h)| matches!(h, Ok(Some(h)) if *h != pin))
                || zikaron_anchor::endpoints::distinct_places(slots.iter().flatten().filter(|(_, h)| matches!(h, Ok(Some(h)) if *h == pin)).map(|(u, _)| u.as_str())) >= 2
        },
    );
    for (url, got) in read.into_iter().flatten() {
        match got {
            Ok(Some(hash)) if hash != pinned => return (Gate::Fingerprint, Vec::new()),
            Ok(Some(_)) => counted.push(url),
            Ok(None) => {
                empty += 1;
                misses.push(Miss { url, why: Why::NoCode });
            }
            Err(why) => misses.push(Miss { url, why }),
        }
    }
    let gate = match zikaron_anchor::endpoints::distinct_places(counted.iter().map(String::as_str)) {
        0 if empty > 0 && empty == misses.len() => Gate::Fingerprint,
        0 => Gate::Down(misses),
        1 => Gate::Single,
        n => Gate::Agreed(n),
    };
    (gate, counted)
}
