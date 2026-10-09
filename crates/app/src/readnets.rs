//! Read-only networks: extra networks this machine reads when verifying someone else's material.
//!
//! The main network (from each home's settings) is the only one the app writes to or reads its own ledger
//! on: anchoring, receipts, balance, gas, self-audit, the tail check and the gates. Reading someone else's
//! ledger, kit or grant uses the main network plus every network in this table, so a kit anchored on another
//! chain verifies without the user switching their own network back and forth.
//!
//! ─── Storage ───
//!
//! One file in the machine directory ([`FILE`], form literal [`FORM`]), shared by every identity on this
//! machine and written through `home::put_at` (temporary name then rename, owner-only). It holds no account
//! data, so it is stored plain. An empty table means no file: removing the last network deletes it.
//!
//! ─── Shape ───
//!
//! `{"form":"zikaron.read-networks/1","networks":[{"chainId":n,"fromBlock":n,"name":"…"?,"nodes":["…"],
//! "registry":"0x…"}…]}`, canonical key order, rows in the order they were added.
//!
//! ─── Names ───
//!
//! A node reports a chain id, never a name. Names are for display only and come from [`known_name`]: a known
//! chain uses the table's name and none is stored; any other chain keeps the optional name the user typed
//! (shown as "chain <id>" without one). Names never affect a verdict or a result file.

use crate::fault::{classify, Fault, Known};
use crate::key::Address;
use crate::lang::Key;
use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};

/// The table's form literal.
pub const FORM: &str = "zikaron.read-networks/1";
/// The table's file name in the machine directory.
pub const FILE: &str = "read-networks.json";

/// Chains with built-in display names, and each name's string key.
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

/// The name the table gives a chain, if it knows it.
pub fn known_name(chain_id: u64) -> Option<Key> {
    NAMES.iter().find(|(c, _)| *c == chain_id).map(|(_, k)| *k)
}

/// A chain's display name: the built-in one, else the typed one, else "chain <id>".
pub fn chain_name(chain_id: u64, typed: Option<&str>) -> String {
    match (known_name(chain_id), typed.map(str::trim).filter(|t| !t.is_empty())) {
        (Some(k), _) => crate::lang::t(k).to_string(),
        (None, Some(t)) => t.to_string(),
        (None, None) => crate::lang::fill1(Key::ChainNumbered, &chain_id.to_string()),
    }
}

/// One read-only network.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Net {
    pub chain_id: u64,
    pub registry: Address,
    pub from_block: u64,
    /// Node addresses (`http://` or `https://`), at least one, each once.
    pub nodes: Vec<String>,
    /// The typed name; only stored for chains [`known_name`] does not know.
    pub name: Option<String>,
}

impl Net {
    /// Its display name.
    pub fn name(&self) -> String {
        chain_name(self.chain_id, self.name.as_deref())
    }

    /// Whether this is the network identified by `(chain_id, registry)`.
    pub fn is(&self, chain_id: u64, registry: &Address) -> bool {
        self.chain_id == chain_id && self.registry == *registry
    }

    /// Its nodes as endpoints of its chain.
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

/// Parse the fields the user typed for one network. Each invalid field is reported by name; nothing is
/// written.
pub fn cells(name: &str, chain: &str, registry: &str, from_block: &str, nodes: &str) -> Result<Net, Fault> {
    let number = |t: &str, tail: Key| -> Result<u64, Fault> {
        let t = t.trim();
        let digits = !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
        match t.parse::<u64>() {
            Ok(n) if digits && n <= json::MAX_INT => Ok(n),
            _ => Err(Fault::known(Known::SettingsShape, crate::lang::fill1(tail, &format!("{t:?}")))),
        }
    };
    let chain_id = number(chain, Key::Tail024)?;
    let from_block = number(from_block, Key::Tail025)?;
    let registry = Address::parse(registry).ok_or_else(|| Fault::known(Known::AddressShape, registry.trim().to_string()))?;
    let mut list: Vec<String> = Vec::new();
    for u in nodes.split_whitespace() {
        if zikaron_net::parse(u).is_none() {
            return Err(Fault::known(Known::SettingsShape, crate::chainx::address_said(u)));
        }
        if !list.iter().any(|x| x == u) {
            list.push(u.to_string());
        }
    }
    if list.is_empty() {
        return Err(Fault::known(Known::FieldMissing, crate::lang::t(Key::TailReadNetNodes).to_string()));
    }
    let name = match known_name(chain_id) {
        Some(_) => None,
        None => Some(name.trim().to_string()).filter(|n| !n.is_empty()),
    };
    Ok(Net { chain_id, registry, from_block, nodes: list, name })
}

/// Where the table's file is.
pub fn path_in(machine: &Path) -> PathBuf {
    machine.join(FILE)
}

/// The table's canonical bytes.
pub fn bytes_of(nets: &[Net]) -> Vec<u8> {
    json::canon_bytes(&Value::Obj(vec![
        ("form".to_string(), Value::Str(FORM.to_string())),
        ("networks".to_string(), Value::Arr(nets.iter().map(Net::value).collect())),
    ]))
}

fn member<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    match v {
        Value::Obj(m) => m.iter().find(|(x, _)| x == k).map(|(_, v)| v),
        _ => None,
    }
}

/// Read the table. No file is an empty table; a malformed file is an error (never read as empty, or the next
/// save would drop its contents).
pub fn read(machine: &Path) -> Result<Vec<Net>, Fault> {
    let p = path_in(machine);
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(classify(&e, &p.display().to_string())),
    };
    from_bytes(&bytes, &p.display().to_string())
}

/// Parse the table from bytes (the file's, or a whole-machine backup's); `said` names the source in errors.
pub fn from_bytes(bytes: &[u8], said: &str) -> Result<Vec<Net>, Fault> {
    let bad = || Fault::known(Known::SettingsShape, said.to_string());
    let v = json::parse(bytes).map_err(|t| Fault::known(Known::SettingsShape, format!("{said}: {t:?}")))?;
    if !matches!(member(&v, "form"), Some(Value::Str(f)) if f == FORM) {
        return Err(bad());
    }
    let Some(Value::Arr(rows)) = member(&v, "networks") else { return Err(bad()) };
    let mut out: Vec<Net> = Vec::new();
    for r in rows {
        let int = |k: &str| match member(r, k) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        };
        let text = |k: &str| match member(r, k) {
            Some(Value::Str(s)) => Some(s.clone()),
            _ => None,
        };
        let nodes = match member(r, "nodes") {
            Some(Value::Arr(a)) => a.iter().map(|x| if let Value::Str(s) = x { Some(s.clone()) } else { None }).collect::<Option<Vec<_>>>(),
            _ => None,
        };
        let (Some(chain_id), Some(from_block), Some(registry), Some(nodes)) =
            (int("chainId"), int("fromBlock"), text("registry").and_then(|a| Address::parse(&a)), nodes)
        else {
            return Err(bad());
        };
        if nodes.is_empty() || out.iter().any(|n| n.is(chain_id, &registry)) {
            return Err(bad());
        }
        out.push(Net { chain_id, registry, from_block, nodes, name: text("name") });
    }
    Ok(out)
}

/// Write the table: an empty one removes the file.
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

/// Add a network, or replace the one identified by `was`. A chain and registry already present (in another
/// row, or as the main network `main`) is refused and nothing is written. No chain is contacted. Returns the
/// table as written.
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

/// Remove a network. Removing one that is not in the table is an error.
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

    /// Saved networks read back in order; duplicates (including the main network) are refused; an empty
    /// table deletes the file.
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

    /// Each kind of invalid field is rejected with its own error.
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
