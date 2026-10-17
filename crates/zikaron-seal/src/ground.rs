//! What a home's settings say about where its anchors are: the chain, the registry contract and the block to scan
//! from (law §9.4's basis fields), and the nodes to ask. The settings file is the desktop's (`settings/desk.json`,
//! sealed as `Doc::Settings`); these four members are read here only, by the desktop and by the command line
//! reading a home with the credential alike.

use zikaron::json::Value;

/// The settings file's name, in a home's settings room.
pub const SETTINGS_FILE: &str = "desk.json";

/// The four members' names.
pub const CHAIN_ID: &str = "chainId";
pub const REGISTRY: &str = "registry";
pub const FROM_BLOCK: &str = "fromBlock";
pub const ENDPOINTS: &str = "endpoints";

/// The four fields as the settings hold them: a chain and a registry not set are `None`, a start block not set
/// is block 0, no nodes is an empty list.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ground {
    pub chain_id: Option<u64>,
    pub registry: Option<[u8; 20]>,
    pub from_block: u64,
    /// The nodes, each as written (`<chain id>=<url>`, the address possibly carrying a key): read as text, their
    /// form judged where they are used, so one that does not read is reported there and never dropped here.
    pub endpoints: Vec<String>,
}

/// Read the four from the settings' value. A member of another shape reads as not set (a registry that is not
/// forty hex digits included); in the node list, an item that is not text is not a node.
pub fn of_settings(v: &Value) -> Ground {
    let registry = match v.member(REGISTRY) {
        Some(Value::Str(s)) => {
            let low = s.trim().to_ascii_lowercase();
            if zikaron::hexfmt::is_hex20(&low) {
                zikaron::hexfmt::decode(&low).and_then(|b| <[u8; 20]>::try_from(b.as_slice()).ok())
            } else {
                None
            }
        }
        _ => None,
    };
    Ground {
        chain_id: match v.member(CHAIN_ID) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        },
        registry,
        from_block: match v.member(FROM_BLOCK) {
            Some(Value::Int(n)) => *n,
            _ => 0,
        },
        endpoints: match v.member(ENDPOINTS) {
            Some(Value::Arr(a)) => a
                .iter()
                .filter_map(|x| match x {
                    Value::Str(s) => Some(s.clone()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_read_as_set_or_not_set() {
        let v = zikaron::json::parse(br#"{"chainId":11155111,"endpoints":["11155111=https://a.example/k","x",7],"fromBlock":7,"registry":"0x36EA8A857A5FE813429D4D9947000C644A88809A"}"#).ok().expect("json");
        let g = of_settings(&v);
        assert_eq!(g.chain_id, Some(11155111));
        assert_eq!(g.from_block, 7);
        assert_eq!(g.registry.map(|r| r[0]), Some(0x36), "大小写不论");
        assert_eq!(g.endpoints, vec!["11155111=https://a.example/k".to_string(), "x".to_string()], "节点照写的原样读,形在用处判;不是字的不算节点");
        let none = of_settings(&zikaron::json::parse(br#"{"chainId":"1","endpoints":"1=http://a","registry":"0x12"}"#).ok().expect("json"));
        assert_eq!(none, Ground::default(), "形不对的读作没设");
    }
}
