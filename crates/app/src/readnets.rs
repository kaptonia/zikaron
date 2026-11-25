use crate::fault::{classify, Fault, Known};
use crate::key::Address;
use crate::lang::Key;
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};

pub use zikaron_seal::readnets::{FILE, FORM};

const NAMES: [(u64, Key); 8] = [
    (1, Key::DeployMainnet),
    (10, Key::ChainOpMainnet),
    (8453, Key::ChainBase),
    (42161, Key::ChainArbitrumOne),
    (11_155_111, Key::DeploySepolia),
    (11_155_420, Key::ChainOpSepolia),
    (84_532, Key::ChainBaseSepolia),
    (421_614, Key::ChainArbitrumSepolia),
];

pub fn known_name(chain_id: u64) -> Option<Key> {
    NAMES.iter().find(|(c, _)| *c == chain_id).map(|(_, k)| *k)
}

pub fn chain_name(chain_id: u64, typed: Option<&str>) -> String {
    match (known_name(chain_id), typed.map(str::trim).filter(|t| !t.is_empty())) {
        (Some(k), _) => crate::lang::t(k).to_string(),
        (None, Some(t)) => t.to_string(),
        (None, None) => crate::lang::fill1(Key::ChainNumbered, &chain_id.to_string()),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Net {
    pub chain_id: u64,
    pub registry: Address,
    pub from_block: u64,
    pub nodes: Vec<String>,
    pub name: Option<String>,
}

impl Net {
    pub fn name(&self) -> String {
        chain_name(self.chain_id, self.name.as_deref())
    }

    pub fn is(&self, chain_id: u64, registry: &Address) -> bool {
        self.chain_id == chain_id && self.registry == *registry
    }

    pub fn endpoints(&self) -> Vec<crate::chainx::Endpoint> {
        self.nodes.iter().map(|u| crate::chainx::Endpoint::at(self.chain_id, u.as_str())).collect()
    }

    fn value(&self) -> Value {
        let mut m = vec![
            ("chainId".to_string(), Value::Int(self.chain_id)),
            ("fromBlock".to_string(), Value::Int(self.from_block)),
        ];
        if let Some(n) = &self.name {
            m.push(("name".to_string(), Value::Str(n.clone())));
        }
        m.push(("nodes".to_string(), Value::Arr(self.nodes.iter().map(|u| Value::Str(u.clone())).collect())));
        m.push(("registry".to_string(), Value::Str(self.registry.hex())));
        Value::Obj(m)
    }
}

pub fn all_endpoints(main: &[crate::chainx::Endpoint], nets: &[Net]) -> Vec<crate::chainx::Endpoint> {
    main.iter().cloned().chain(nets.iter().flat_map(Net::endpoints)).collect()
}

pub fn cells(name: &str, chain: &str, registry: &str, from_block: &str, nodes: &str) -> Result<Net, Fault> {
    let number = |t: &str, tail: Key| -> Result<u64, Fault> {
        let t = t.trim();
        let digits = !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
        match t.parse::<u64>() {
            Ok(n) if digits && n <= json::MAX_INT => Ok(n),
            _ => Err(Fault::known(Known::SettingsShape, crate::lang::both_of(|| crate::lang::fill1(tail, &format!("{t:?}"))))),
        }
    };
    let chain_id = number(chain, Key::Tail024)?;
    let from_block = number(from_block, Key::Tail025)?;
    let registry = Address::parse(registry).ok_or_else(|| Fault::known(Known::AddressShape, registry.trim().to_string()))?;
    let mut list: Vec<String> = Vec::new();
    for u in nodes.split_whitespace() {
        if zikaron_net::parse(u).is_none() {
            return Err(Fault::known(Known::SettingsShape, crate::lang::both_of(|| crate::chainx::address_said(u))));
        }
        if !list.iter().any(|x| x == u) {
            list.push(u.to_string());
        }
    }
    if list.is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::both_of(|| crate::lang::t(Key::TailReadNetNodes).to_string())));
    }
    let name = match known_name(chain_id) {
        Some(_) => None,
        None => Some(name.trim().to_string()).filter(|n| !n.is_empty()),
    };
    Ok(Net { chain_id, registry, from_block, nodes: list, name })
}

pub fn path_in(machine: &Path) -> PathBuf {
    zikaron_seal::readnets::path_in(machine)
}

pub fn bytes_of(nets: &[Net]) -> Vec<u8> {
    json::canon_bytes(&Value::Obj(vec![
        ("form".to_string(), Value::Str(FORM.to_string())),
        ("networks".to_string(), Value::Arr(nets.iter().map(Net::value).collect())),
    ]))
}

pub fn read(machine: &Path) -> Result<Vec<Net>, Fault> {
    let p = path_in(machine);
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(classify(&e, &p.display().to_string())),
    };
    from_bytes(&bytes, &p.display().to_string())
}

pub fn from_bytes(bytes: &[u8], said: &str) -> Result<Vec<Net>, Fault> {
    let rows = zikaron_seal::readnets::parse(bytes).map_err(|w| match w.as_str() {
        "form" | "networks" => Fault::known(Known::SettingsShape, crate::lang::both_of(|| said.to_string())),
        _ => Fault::known(Known::SettingsShape, crate::lang::both_of(|| format!("{said}: {w}"))),
    })?;
    Ok(rows.into_iter().map(|r| Net { chain_id: r.chain_id, registry: Address(r.registry), from_block: r.from_block, nodes: r.nodes, name: r.name }).collect())
}

pub fn write(machine: &Path, nets: &[Net]) -> Result<(), Fault> {
    if nets.is_empty() {
        let p = path_in(machine);
        return match std::fs::remove_file(&p) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(classify(&e, &p.display().to_string())),
        };
    }
    crate::home::put_at(machine, FILE, &bytes_of(nets))
}

pub fn save(machine: &Path, was: Option<(u64, Address)>, net: Net, main: Option<(u64, Address)>) -> Result<Vec<Net>, Fault> {
    let mut nets = read(machine)?;
    let at = match was {
        Some((c, r)) => Some(nets.iter().position(|n| n.is(c, &r)).ok_or_else(|| Fault::known(Known::SubjectMissing, format!("{c} {}", r.hex())))?),
        None => None,
    };
    let taken = nets.iter().enumerate().any(|(i, n)| Some(i) != at && n.is(net.chain_id, &net.registry))
        || main.map(|(c, r)| c == net.chain_id && r == net.registry).unwrap_or(false);
    if taken {
        return Err(Fault::known(Known::NetworkListed, format!("{} {}", net.chain_id, net.registry.hex())));
    }
    match at {
        Some(i) => nets[i] = net,
        None => nets.push(net),
    }
    write(machine, &nets)?;
    Ok(nets)
}

pub fn remove(machine: &Path, chain_id: u64, registry: &Address) -> Result<Vec<Net>, Fault> {
    let mut nets = read(machine)?;
    let before = nets.len();
    nets.retain(|n| !n.is(chain_id, registry));
    if nets.len() == before {
        return Err(Fault::known(Known::SubjectMissing, format!("{chain_id} {}", registry.hex())));
    }
    write(machine, &nets)?;
    Ok(nets)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn others_notices_are_read_through_the_main_and_every_read_only_network() {
        let main = vec![crate::chainx::Endpoint::at(1, "https://main.example")];
        let nets = vec![
            Net { chain_id: 11_155_111, registry: Address([0x11; 20]), from_block: 0, nodes: vec!["https://a.example".into(), "https://b.example".into()], name: None },
            Net { chain_id: 10, registry: Address([0x22; 20]), from_block: 0, nodes: vec!["https://c.example".into()], name: None },
        ];
        let got: Vec<(u64, String)> = all_endpoints(&main, &nets).iter().map(|e| (e.chain, e.url.said())).collect();
        let want: Vec<(u64, String)> = [(1, "https://main.example"), (11_155_111, "https://a.example"), (11_155_111, "https://b.example"), (10, "https://c.example")].iter().map(|(c, u)| (*c, crate::chainx::Endpoint::at(*c, *u).url.said())).collect();
        assert_eq!(got, want);
        assert_eq!(all_endpoints(&main, &[]).len(), 1, "no read-only network: the main network's nodes alone");
    }

    #[test]
    fn a_table_reads_back_what_it_wrote_and_an_empty_one_is_no_file() {
        let dir = std::env::temp_dir().join(format!("zk-readnets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let a = cells("", "42161", "0x19C4d7b0a5f3c2e81d6649a0b7c35f2e9d10e2a8", "268400000", "https://a.example\nhttps://b.example https://a.example").unwrap();
        assert_eq!(a.nodes.len(), 2, "同一节点只记一次");
        assert_eq!(a.name, None, "表里有名的链不存名");
        let b = cells("私链", "31337", "0x0d6b94a2e7c1305f8b6d2a49e0c73f51b8a977a1", "0", "http://127.0.0.1:8545").unwrap();
        save(&dir, None, a.clone(), None).unwrap();
        save(&dir, None, b.clone(), None).unwrap();
        assert_eq!(read(&dir).unwrap(), vec![a.clone(), b.clone()]);
        let again = save(&dir, None, a.clone(), None).unwrap_err();
        assert_eq!(again.which(), Some(Known::NetworkListed));
        let main = save(&dir, Some((b.chain_id, b.registry)), b.clone(), Some((b.chain_id, b.registry))).unwrap_err();
        assert_eq!(main.which(), Some(Known::NetworkListed), "与主网络同链同合约即拒");
        remove(&dir, a.chain_id, &a.registry).unwrap();
        remove(&dir, b.chain_id, &b.registry).unwrap();
        assert!(!path_in(&dir).exists(), "表空即无档");
        assert_eq!(read(&dir).unwrap(), Vec::new());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn cells_that_cannot_be_read_are_refused_by_name() {
        let reg = "0x0d6b94a2e7c1305f8b6d2a49e0c73f51b8a977a1";
        assert_eq!(cells("", "x", reg, "0", "https://a").unwrap_err().which(), Some(Known::SettingsShape));
        assert_eq!(cells("", "9007199254740992", reg, "0", "https://a").unwrap_err().which(), Some(Known::SettingsShape));
        assert_eq!(cells("", "1", "0x12", "0", "https://a").unwrap_err().which(), Some(Known::AddressShape));
        assert_eq!(cells("", "1", reg, "-1", "https://a").unwrap_err().which(), Some(Known::SettingsShape));
        assert_eq!(cells("", "1", reg, "0", "  ").unwrap_err().which(), Some(Known::FieldMissing));
        assert_eq!(cells("", "1", reg, "0", "ftp://a").unwrap_err().which(), Some(Known::SettingsShape));
    }
}
