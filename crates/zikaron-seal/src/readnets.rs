//! The read-only networks table's bytes, one reading for both ends: the extra networks this machine reads when it
//! reads someone else's material, and where a record package addressed to an address looks for that address's key.
//! The desktop keeps the table (adding, editing, removing, the names shown); the command line reads it here too.
//!
//! One file in the machine directory ([`FILE`], form literal [`FORM`]), plain (it holds no account data):
//! `{"form":"zikaron.read-networks/1","networks":[{"chainId":n,"fromBlock":n,"name":"…"?,"nodes":["…"],
//! "registry":"0x…"}…]}`, rows in the order they were added. No file is an empty table.

use std::path::{Path, PathBuf};
use zikaron::json::{self, Value};

/// The table's form literal.
pub const FORM: &str = "zikaron.read-networks/1";
/// The table's file name in the machine directory.
pub const FILE: &str = "read-networks.json";

/// One read-only network as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub chain_id: u64,
    pub registry: [u8; 20],
    pub from_block: u64,
    /// Node addresses as written, at least one.
    pub nodes: Vec<String>,
    /// The typed name, when one is stored.
    pub name: Option<String>,
}

/// Where the table's file is.
pub fn path_in(machine: &Path) -> PathBuf {
    machine.join(FILE)
}

/// The table read from bytes. `Err` (the member that does not read, or the whole when the form is not this one)
/// for a table not in its shape: a malformed table is never read as empty. Two rows on one chain and registry, or
/// a row with no node, are not in its shape.
pub fn parse(bytes: &[u8]) -> Result<Vec<Row>, String> {
    let v = json::parse(bytes).map_err(|t| format!("{t:?}"))?;
    if !matches!(v.member("form"), Some(Value::Str(f)) if f == FORM) {
        return Err("form".into());
    }
    let Some(Value::Arr(rows)) = v.member("networks") else { return Err("networks".into()) };
    let mut out: Vec<Row> = Vec::new();
    for r in rows {
        let int = |k: &str| match r.member(k) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        };
        let text = |k: &str| match r.member(k) {
            Some(Value::Str(s)) => Some(s.clone()),
            _ => None,
        };
        let nodes = match r.member("nodes") {
            Some(Value::Arr(a)) => a.iter().map(|x| if let Value::Str(s) = x { Some(s.clone()) } else { None }).collect::<Option<Vec<_>>>(),
            _ => None,
        };
        let registry = text("registry").and_then(|a| crate::identities::address(&a));
        let (Some(chain_id), Some(from_block), Some(registry), Some(nodes)) = (int("chainId"), int("fromBlock"), registry, nodes) else {
            return Err("networks".into());
        };
        if nodes.is_empty() || out.iter().any(|n| n.chain_id == chain_id && n.registry == registry) {
            return Err("networks".into());
        }
        out.push(Row { chain_id, registry, from_block, nodes, name: text("name") });
    }
    Ok(out)
}

/// The table in the machine directory `machine`: no file is an empty table; a file that does not read, or is not
/// in its shape, is `Err` with its path and what failed.
pub fn read(machine: &Path) -> Result<Vec<Row>, crate::Trouble> {
    let p = path_in(machine);
    let bytes = match std::fs::read(&p) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(crate::Trouble::io(&e, &p.display().to_string())),
    };
    parse(&bytes).map_err(|w| crate::Trouble::new(crate::Code::SettingsShape, format!("{}: {w}", p.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_reads_its_rows_and_refuses_what_is_not_its_shape() {
        let t = br#"{"form":"zikaron.read-networks/1","networks":[{"chainId":31337,"fromBlock":0,"name":"a","nodes":["http://127.0.0.1:8545"],"registry":"0x0d6b94a2e7c1305f8b6d2a49e0c73f51b8a977a1"}]}"#;
        let rows = parse(t).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].chain_id, 31337);
        assert_eq!(rows[0].name.as_deref(), Some("a"));
        assert!(parse(br#"{"form":"x","networks":[]}"#).is_err());
        assert!(parse(br#"{"form":"zikaron.read-networks/1","networks":[{"chainId":1,"fromBlock":0,"nodes":[],"registry":"0x0d6b94a2e7c1305f8b6d2a49e0c73f51b8a977a1"}]}"#).is_err());
    }
}
