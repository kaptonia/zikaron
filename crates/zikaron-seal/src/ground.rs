use zikaron::json::Value;

pub const SETTINGS_FILE: &str = "desk.json";

pub const CHAIN_ID: &str = "chainId";
pub const REGISTRY: &str = "registry";
pub const FROM_BLOCK: &str = "fromBlock";
pub const ENDPOINTS: &str = "endpoints";
pub const EARLIER: &str = "earlier";
pub const PUBLISH: &str = "publish";
pub const REST: &str = "restApis";
pub const REST_ON: &str = "restApiMode";
pub const LOG_ASKS: &str = "logAsks";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Earlier {
    pub chain_id: u64,
    pub registry: [u8; 20],
    pub from_block: u64,
}

impl Earlier {
    pub fn value(&self) -> Value {
        Value::Obj(vec![
            (CHAIN_ID.to_string(), Value::Int(self.chain_id)),
            (FROM_BLOCK.to_string(), Value::Int(self.from_block)),
            (REGISTRY.to_string(), Value::Str(zikaron::hexfmt::encode(&self.registry))),
        ])
    }
}

pub fn earlier_of(v: &Value) -> Vec<Earlier> {
    let Some(Value::Arr(a)) = v.member(EARLIER) else { return Vec::new() };
    a.iter()
        .filter_map(|x| {
            let chain_id = match x.member(CHAIN_ID) {
                Some(Value::Int(n)) => *n,
                _ => return None,
            };
            let from_block = match x.member(FROM_BLOCK) {
                Some(Value::Int(n)) => *n,
                _ => return None,
            };
            let registry = registry_of(x.member(REGISTRY))?;
            Some(Earlier { chain_id, registry, from_block })
        })
        .collect()
}

fn registry_of(v: Option<&Value>) -> Option<[u8; 20]> {
    match v {
        Some(Value::Str(s)) => {
            let low = s.trim().to_ascii_lowercase();
            if zikaron::hexfmt::is_hex20(&low) {
                zikaron::hexfmt::decode(&low).and_then(|b| <[u8; 20]>::try_from(b.as_slice()).ok())
            } else {
                None
            }
        }
        _ => None,
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ground {
    pub chain_id: Option<u64>,
    pub registry: Option<[u8; 20]>,
    pub from_block: u64,
    pub endpoints: Vec<String>,
    pub earlier: Vec<Earlier>,
    pub publish: Option<String>,
    pub rest: Vec<String>,
    pub rest_on: bool,
    pub log_asks: Option<u64>,
}

impl Ground {
    pub fn other_chains(&self) -> Vec<u64> {
        let mut out: Vec<u64> = Vec::new();
        for e in &self.earlier {
            if Some(e.chain_id) != self.chain_id && !out.contains(&e.chain_id) {
                out.push(e.chain_id);
            }
        }
        out
    }

    pub fn window_of(&self, chain: u64) -> Option<([u8; 20], u64)> {
        if Some(chain) == self.chain_id {
            return self.registry.map(|r| (r, self.from_block));
        }
        self.earlier.iter().find(|e| e.chain_id == chain).map(|e| (e.registry, e.from_block))
    }

    pub fn can_anchor(&self) -> bool {
        self.chain_id.is_some() && (!self.endpoints.is_empty() || !self.rest_asked().is_empty())
    }

    pub fn rest_asked(&self) -> &[String] {
        if self.rest_on { &self.rest } else { &[] }
    }

    pub fn chains(&self) -> Vec<u64> {
        let mut out: Vec<u64> = self.chain_id.into_iter().collect();
        out.extend(self.other_chains());
        out
    }

    pub fn home_chain(&self) -> u64 {
        self.earlier.first().map(|e| e.chain_id).or(self.chain_id).unwrap_or(0)
    }
}

pub fn of_settings(v: &Value) -> Ground {
    let registry = registry_of(v.member(REGISTRY));
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
        endpoints: texts(v.member(ENDPOINTS)),
        earlier: earlier_of(v),
        publish: match v.member(PUBLISH) {
            Some(Value::Str(x)) => Some(x.clone()),
            _ => None,
        },
        rest: texts(v.member(REST)),
        rest_on: matches!(v.member(REST_ON), Some(Value::Bool(true))),
        log_asks: match v.member(LOG_ASKS) {
            Some(Value::Int(n)) => Some(*n),
            _ => None,
        },
    }
}

fn texts(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Arr(a)) => a
            .iter()
            .filter_map(|x| match x {
                Value::Str(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
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
