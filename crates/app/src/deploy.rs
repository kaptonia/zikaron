use crate::lang::Key;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deployment {
    pub name: &'static str,
    pub label: Key,
    pub chain_id: u64,
    pub registry: &'static str,
    pub from_block: u64,
    pub nodes: [&'static str; 2],
    pub own: crate::own::DeployOwn,
}

pub const KNOWN: [Deployment; crate::own::DEPLOYMENTS.len()] = crate::own::DEPLOYMENTS;

pub const DEFAULT: &str = crate::own::DEPLOY_DEFAULT;

pub const CODE_HASH: &str = crate::own::DEPLOY_CODE_HASH;

pub const CUSTOM: &str = "custom";

pub const UNDECIDED: &str = "undecided";

pub fn named(name: &str) -> Option<&'static Deployment> {
    KNOWN.iter().find(|d| d.name == name)
}

pub fn choices() -> Vec<&'static str> {
    KNOWN.iter().map(|d| d.name).chain(std::iter::once(CUSTOM)).collect()
}

pub fn is_choice(name: &str) -> bool {
    name == CUSTOM || named(name).is_some()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Custom {
    pub chain_id: u64,
    pub registry: crate::key::Address,
    pub from_block: u64,
    pub endpoints: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum Network<'a> {
    Known(&'static Deployment),
    Custom(&'a Custom),
}

pub fn resolve<'a>(name: Option<&str>, custom: Option<&'a Custom>) -> Option<Network<'a>> {
    match (name.and_then(named), name, custom) {
        (Some(d), _, _) => Some(Network::Known(d)),
        (None, Some(CUSTOM), Some(c)) => Some(Network::Custom(c)),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cells {
    pub chain: String,
    pub registry: String,
    pub from: String,
    pub endpoints: String,
    pub nodes: String,
}

impl Deployment {
    pub fn registry_address(&self) -> crate::key::Address {
        crate::key::Address::parse(&self.registry.to_lowercase()).expect("已知部署表里的登记合约是一枚合法地址")
    }

    pub fn endpoint_specs(&self) -> Vec<String> {
        specs(self.chain_id, &self.nodes)
    }

    pub fn cells(&self) -> Cells {
        Cells {
            chain: self.chain_id.to_string(),
            registry: self.registry.to_ascii_lowercase(),
            from: self.from_block.to_string(),
            endpoints: self.endpoint_specs().join(" "),
            nodes: self.nodes.join("\n"),
        }
    }
}

fn specs(chain: u64, nodes: &[&str; 2]) -> Vec<String> {
    nodes.iter().map(|u| format!("{chain}={u}")).collect()
}

pub fn of_chain(chain_id: Option<u64>, registry: Option<crate::key::Address>) -> Option<&'static Deployment> {
    KNOWN.iter().find(|d| chain_id == Some(d.chain_id) && registry == Some(d.registry_address()))
}

pub fn same_as_row(chain_id: Option<u64>, registry: Option<crate::key::Address>, from_block: u64, endpoints: &[String]) -> Option<&'static Deployment> {
    let nodes = |specs: &[String]| -> std::collections::BTreeSet<String> {
        specs.iter().map(|x| crate::chainx::Endpoint::parse(x).map(|e| e.spec()).unwrap_or_else(|| x.clone())).collect()
    };
    let have = nodes(endpoints);
    KNOWN.iter().find(|d| {
        chain_id == Some(d.chain_id) && registry == Some(d.registry_address()) && from_block == d.from_block && have == nodes(&d.endpoint_specs())
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_is_well_formed_and_names_are_distinct() {
        for d in KNOWN {
            let _ = d.registry_address();
            assert!(d.nodes.iter().all(|u| u.starts_with("https://")), "公共节点须 TLS");
            assert_ne!(d.nodes[0], d.nodes[1], "两处须不同");
            assert_ne!(d.name, CUSTOM);
        }
        assert_eq!(choices().last(), Some(&CUSTOM));
        assert!(choices().iter().all(|c| is_choice(c)));
        assert!(named(DEFAULT).is_some());
        let mut names: Vec<&str> = KNOWN.iter().map(|d| d.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), KNOWN.len());
    }

    #[test]
    fn a_rows_chain_and_registry_name_it_whatever_its_nodes() {
        for d in KNOWN {
            assert_eq!(of_chain(Some(d.chain_id), Some(d.registry_address())), Some(&d));
            let mut eps = d.endpoint_specs();
            eps.push(format!("{}=https://one-more.example", d.chain_id));
            assert_eq!(same_as_row(Some(d.chain_id), Some(d.registry_address()), d.from_block, &eps), None, "the nodes differ from the row's");
            assert_eq!(of_chain(Some(d.chain_id), Some(d.registry_address())).map(|r| r.name), Some(d.name), "named by its chain all the same");
        }
        let d = &KNOWN[0];
        assert_eq!(of_chain(Some(d.chain_id), Some(crate::key::Address([0x77; 20]))), None, "another registry");
        assert_eq!(of_chain(None, Some(d.registry_address())), None, "no chain");
        assert_eq!(of_chain(Some(424242), Some(d.registry_address())), None, "a chain of no row");
    }

    #[test]
    fn a_network_equal_to_a_row_is_that_row() {
        for d in KNOWN {
            let mut eps = d.endpoint_specs();
            assert_eq!(same_as_row(Some(d.chain_id), Some(d.registry_address()), d.from_block, &eps), Some(&d), "全等即那一行");
            eps.reverse();
            assert_eq!(same_as_row(Some(d.chain_id), Some(d.registry_address()), d.from_block, &eps), Some(&d), "节点不论次序");
            assert_eq!(same_as_row(Some(d.chain_id), Some(d.registry_address()), d.from_block + 1, &eps), None, "起始区块差一格即不是");
            assert_eq!(same_as_row(Some(d.chain_id), Some(d.registry_address()), d.from_block, &eps[..1]), None, "少一处节点即不是");
            assert_eq!(same_as_row(Some(d.chain_id + 1), Some(d.registry_address()), d.from_block, &eps), None, "链号差即不是");
        }
        assert_eq!(same_as_row(None, None, 0, &[]), None);
    }
}
