//! Known deployments table. Constants compiled into the product: the shipped build reads no external switch.
//!
//! A row's shape: row name (the only thing stored on disk), chain id, registry contract, scan start block,
//! two public nodes from different sources (for cross-checking). The chain id, contract, start block and node
//! literals always come from this table: the machine-level settings file records only the chosen row name,
//! and the disk keeps no second statement; when a home takes its basis from the machine's choice, it copies
//! this row into its own settings file (each home's readers read their own settings).
//!
//! Two rows: Ethereum mainnet ([`DEFAULT`]) and the Sepolia testnet. The mainnet registry is the same pinned
//! build as the testnet one (runtime codeHash `0xfa97a1d9…f57d`, `base/zikaron-core/contracts/CODEHASH.md`),
//! deployed at block 26087229.
//!
//! ─── How the public nodes were chosen ───
//!
//! Every node is TLS and answers the row's chain id; both nodes of a row were asked for the same logs over the
//! same windows and matched entry for entry. Rejected in the same comparison: nodes that answered historical
//! logs empty (they would read "anchored" as "not anchored"), nodes whose log window is limited to 10 to 1000
//! blocks (one audit would need hundreds of queries), and nodes that are paid, rate-limited or discontinued.
//! Mainnet: one node answers wide windows directly; the other refuses windows over 100000 blocks and names its
//! limit, so the scanner splits the window at that limit (`zikaron_anchor::scan`).

use crate::lang::Key;

/// One known deployment row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deployment {
    /// Row name: what the machine-level and home settings files record. One name, one home.
    pub name: &'static str,
    /// The name on the face.
    pub label: Key,
    pub chain_id: u64,
    /// Registry contract (hex, with `0x`).
    pub registry: &'static str,
    /// Scan start block: there are no logs for the registry contract before its deployment block.
    pub from_block: u64,
    /// Two public nodes from different sources (`https://`).
    pub nodes: [&'static str; 2],
}

/// The whole table.
pub const KNOWN: [Deployment; 2] = [
    Deployment {
        name: "mainnet",
        label: Key::DeployMainnet,
        chain_id: 1,
        registry: "0x36Ea8A857a5FE813429d4D9947000C644A88809A",
        from_block: 26_087_229,
        nodes: ["https://mainnet.gateway.tenderly.co", "https://rpc.flashbots.net"],
    },
    Deployment {
        name: "sepolia",
        label: Key::DeploySepolia,
        chain_id: 11_155_111,
        registry: "0xC29410B882c4C3b77e33659d2f06ac563e7B08a3",
        from_block: 11_715_660,
        nodes: ["https://sepolia.gateway.tenderly.co", "https://rpc.sepolia.ethpandaops.io"],
    },
];

/// The row selected by default in the wizard's network step.
pub const DEFAULT: &str = "mainnet";

/// How "custom" is recorded on disk (the same cell as row names, never colliding).
pub const CUSTOM: &str = "custom";

/// Find a row by name. Names outside the table (including [`CUSTOM`]) give `None`.
pub fn named(name: &str) -> Option<&'static Deployment> {
    KNOWN.iter().find(|d| d.name == name)
}

impl Deployment {
    /// The registry contract read as an address (the table literals are checked before compiling; unreadable
    /// means the table is wrong, said at once).
    pub fn registry_address(&self) -> crate::key::Address {
        crate::key::Address::parse(&self.registry.to_lowercase()).expect("已知部署表里的登记合约是一枚合法地址")
    }

    /// The two nodes written as settings file endpoints (`chain=url`).
    pub fn endpoint_specs(&self) -> Vec<String> {
        self.nodes.iter().map(|u| format!("{}={u}", self.chain_id)).collect()
    }
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
        assert!(named(DEFAULT).is_some());
        let mut names: Vec<&str> = KNOWN.iter().map(|d| d.name).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), KNOWN.len());
    }
}
