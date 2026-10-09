//! Known deployments table, compiled into the product; the shipped build reads no external switch.
//!
//! Each row has a name, chain id, registry contract, scan start block, two public nodes from different
//! providers (for cross-checking), and the node pairs the row shipped with before. Chain id, contract, start
//! block and node literals always come from this table: the machine-level settings file records only the row
//! name last chosen (preselected by the wizard and the new-identity sheet), an identity records the row name it
//! chose, and a home that takes a row copies its values into the home's own settings file. The presets of the
//! network editor and of the read-only networks are this table.
//!
//! Four rows: Ethereum mainnet ([`DEFAULT`]), the Sepolia testnet, Arbitrum One and OP Mainnet. The mainnet
//! registry is the same pinned build as the testnet one (runtime codeHash `0xfa97a1d9…f57d`,
//! `base/zikaron-core/contracts/CODEHASH.md`), deployed at block 26087229. On Arbitrum One and OP Mainnet it is
//! the same pinned build at the same address as on mainnet (same deployment bytes, same deployer), deployed at
//! the row's start block.
//!
//! Public node selection: a node qualifies when it holds the full log history from the row's start block. Both
//! nodes of a row returned identical registry logs over the same windows from the start block to the head, and
//! a node refuses an over-wide window with an error (the scanner splits at the limit it names,
//! `zikaron_anchor::scan`) rather than answering short. Every node uses TLS and answers the row's chain id.
//! Rejected: nodes that answer older logs empty (they would read "anchored" as "not anchored"; nodes that keep
//! only the last ten or twenty thousand blocks do this once the start block falls outside that range), nodes
//! that limit log queries to 10 to 1000 blocks (one audit would need hundreds of queries), and paid,
//! rate-limited or discontinued nodes.
//!
//! Earlier node pairs: a home copies a row's nodes when it is filled, so changing the table only reaches homes
//! filled later. A row keeps its earlier pairs ([`Deployment::was`]); opening a home for writing whose nodes
//! exactly match one of them (same text, same order) switches it to today's pair and saves once
//! (`action::open_home_at`). A home with any other nodes was edited by a person and is left alone.

use crate::lang::Key;

/// One known deployment row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deployment {
    /// Row name, as recorded in the machine-level and home settings files.
    pub name: &'static str,
    /// Display name.
    pub label: Key,
    pub chain_id: u64,
    /// Registry contract (hex, with `0x`).
    pub registry: &'static str,
    /// Scan start block: there are no logs for the registry contract before its deployment block.
    pub from_block: u64,
    /// Two public nodes from different sources (`https://`).
    pub nodes: [&'static str; 2],
    /// The pairs this row shipped with before, oldest first (none for a row whose nodes never changed).
    pub was: &'static [[&'static str; 2]],
}

/// The whole table.
pub const KNOWN: [Deployment; 4] = [
    Deployment {
        name: "mainnet",
        label: Key::DeployMainnet,
        chain_id: 1,
        registry: "0x36Ea8A857a5FE813429d4D9947000C644A88809A",
        from_block: 26_087_229,
        nodes: ["https://mainnet.gateway.tenderly.co", "https://rpc.mevblocker.io"],
        // Shipped in 0.1.0: that node keeps the last ten to twenty thousand blocks of logs and answers
        // older windows empty, so every scan from the start block disagreed with the other node.
        was: &[["https://mainnet.gateway.tenderly.co", "https://rpc.flashbots.net"]],
    },
    Deployment {
        name: "sepolia",
        label: Key::DeploySepolia,
        chain_id: 11_155_111,
        registry: "0xC29410B882c4C3b77e33659d2f06ac563e7B08a3",
        from_block: 11_715_660,
        nodes: ["https://sepolia.gateway.tenderly.co", "https://rpc.sepolia.ethpandaops.io"],
        was: &[],
    },
    Deployment {
        name: "arbitrum-one",
        label: Key::ChainArbitrumOne,
        chain_id: 42_161,
        registry: "0x36Ea8A857a5FE813429d4D9947000C644A88809A",
        from_block: 511_445_184,
        nodes: ["https://arb1.arbitrum.io/rpc", "https://arbitrum.gateway.tenderly.co"],
        was: &[],
    },
    Deployment {
        name: "op-mainnet",
        label: Key::ChainOpMainnet,
        chain_id: 10,
        registry: "0x36Ea8A857a5FE813429d4D9947000C644A88809A",
        from_block: 157_735_914,
        nodes: ["https://mainnet.optimism.io", "https://optimism.gateway.tenderly.co"],
        was: &[],
    },
];

/// The row selected by default in the wizard's network step and the new-identity sheet, until this machine
/// chooses one.
pub const DEFAULT: &str = "mainnet";

/// How "custom" is recorded on disk (the same cell as row names, never colliding).
pub const CUSTOM: &str = "custom";

/// Find a row by name. Names outside the table (including [`CUSTOM`]) give `None`.
pub fn named(name: &str) -> Option<&'static Deployment> {
    KNOWN.iter().find(|d| d.name == name)
}

/// The choices a network can be taken from: every row of the table in order, then [`CUSTOM`]. The wizard's
/// network step, the new-identity sheet and the import sheet offer these names.
pub fn choices() -> Vec<&'static str> {
    KNOWN.iter().map(|d| d.name).chain(std::iter::once(CUSTOM)).collect()
}

/// The names a network cell accepts: a table row, or [`CUSTOM`] (exact, no trimming or case folding).
pub fn is_choice(name: &str) -> bool {
    name == CUSTOM || named(name).is_some()
}

/// A network filled in by hand (the "custom" choice, once all of it is filled in): chain id, registry
/// contract, scan start block and nodes (`chain=url`), as saved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Custom {
    pub chain_id: u64,
    pub registry: crate::key::Address,
    pub from_block: u64,
    pub endpoints: Vec<String>,
}

/// A network a home can take: a row of the table, or one filled in by hand.
#[derive(Clone, Copy, Debug)]
pub enum Network<'a> {
    Known(&'static Deployment),
    Custom(&'a Custom),
}

/// The network a choice names: a table row by its name; [`CUSTOM`] with the hand-filled network, or `None`
/// while there is none; anything else `None`.
pub fn resolve<'a>(name: Option<&str>, custom: Option<&'a Custom>) -> Option<Network<'a>> {
    match (name.and_then(named), name, custom) {
        (Some(d), _, _) => Some(Network::Known(d)),
        (None, Some(CUSTOM), Some(c)) => Some(Network::Custom(c)),
        _ => None,
    }
}

/// The network editor's four cells as a preset fills them (chain id, registry, start block, nodes); saving
/// through the editor writes exactly what the cells say. Both the main network editor and the read-only
/// networks' editor fill from here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cells {
    pub chain: String,
    pub registry: String,
    pub from: String,
    /// The nodes as the main network's editor takes them (`chain=url`, space between).
    pub endpoints: String,
    /// The nodes as the read-only networks' editor takes them (one url per line).
    pub nodes: String,
}

impl Deployment {
    /// The registry contract as an address. The table literals are checked, so a parse failure means the
    /// table itself is wrong and panics at once.
    pub fn registry_address(&self) -> crate::key::Address {
        crate::key::Address::parse(&self.registry.to_lowercase()).expect("已知部署表里的登记合约是一枚合法地址")
    }

    /// The two nodes written as settings file endpoints (`chain=url`).
    pub fn endpoint_specs(&self) -> Vec<String> {
        specs(self.chain_id, &self.nodes)
    }

    /// The pairs this row shipped with before, written as settings file endpoints.
    pub fn was_specs(&self) -> Vec<Vec<String>> {
        self.was.iter().map(|p| specs(self.chain_id, p)).collect()
    }

    /// This row as the network editor's cells.
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

/// The row whose current values a home's network matches exactly: same chain, registry contract and start
/// block, and the same nodes as a set (any order). Any difference gives `None`. Only the network line's
/// display reads it; it does not change what a home or an identity recorded as chosen.
pub fn same_as_row(chain_id: Option<u64>, registry: Option<crate::key::Address>, from_block: u64, endpoints: &[String]) -> Option<&'static Deployment> {
    let nodes = |specs: &[String]| -> std::collections::BTreeSet<String> {
        specs.iter().map(|x| crate::chainx::Endpoint::parse(x).map(|e| e.spec()).unwrap_or_else(|| x.clone())).collect()
    };
    let have = nodes(endpoints);
    KNOWN.iter().find(|d| {
        chain_id == Some(d.chain_id) && registry == Some(d.registry_address()) && from_block == d.from_block && have == nodes(&d.endpoint_specs())
    })
}

/// The row that shipped these endpoints as an earlier pair (same text, same order); `None` if no row did.
pub fn shipped_before(endpoints: &[String]) -> Option<&'static Deployment> {
    KNOWN.iter().find(|d| d.was_specs().iter().any(|p| p.as_slice() == endpoints))
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
            for p in d.was {
                assert_ne!(p, &d.nodes, "从前的一对不是今天的一对");
            }
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
