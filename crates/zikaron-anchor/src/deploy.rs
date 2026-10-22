//! Where the registry is deployed on each chain the product knows: the chain, the registry contract and the
//! block it was deployed at (no registry log comes before it), with the row's name and its two public nodes. Named
//! here once; the desktop's known deployments table (`app::deploy`) takes its rows from here, and the command line's
//! `anchor` takes from here where to start asking whether a hash is already anchored.

/// The registry build's runtime code hash (keccak-256 of the runtime bytecode), `0x` and sixty-four lowercase hex
/// digits, pinned from the protocol base (`base/zikaron-core/contracts/CODEHASH.md`): a read-only network's registry
/// is read only when its nodes give this code at it (`zikaron_send::fingerprint`). Copied here so no build reads
/// `base/`; the desktop's tests compare it with `BASE-LOCK.json`.
pub const CODE_HASH: &str = "0xfa97a1d9b22fab2b52f4e27c9a965b32734c40001b565ab365d05c887118f57d";

/// One deployment row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub name: &'static str,
    pub chain_id: u64,
    /// Registry contract (hex, with `0x`).
    pub registry: &'static str,
    /// The deployment block: there are no logs for the registry contract before it.
    pub from_block: u64,
    /// Two public nodes from different sources (`https://`).
    pub nodes: [&'static str; 2],
}

/// The rows: Ethereum mainnet, the Sepolia testnet, Arbitrum One, OP Mainnet.
pub const ROWS: [Row; 4] = [
    Row { name: "mainnet", chain_id: 1, registry: "0x36Ea8A857a5FE813429d4D9947000C644A88809A", from_block: 26_087_229, nodes: ["https://mainnet.gateway.tenderly.co", "https://rpc.mevblocker.io"] },
    Row { name: "sepolia", chain_id: 11_155_111, registry: "0xC29410B882c4C3b77e33659d2f06ac563e7B08a3", from_block: 11_715_660, nodes: ["https://sepolia.gateway.tenderly.co", "https://rpc.sepolia.ethpandaops.io"] },
    Row { name: "arbitrum-one", chain_id: 42_161, registry: "0x36Ea8A857a5FE813429d4D9947000C644A88809A", from_block: 511_445_184, nodes: ["https://arb1.arbitrum.io/rpc", "https://arbitrum.gateway.tenderly.co"] },
    Row { name: "op-mainnet", chain_id: 10, registry: "0x36Ea8A857a5FE813429d4D9947000C644A88809A", from_block: 157_735_914, nodes: ["https://mainnet.optimism.io", "https://optimism.gateway.tenderly.co"] },
];

/// The block a registry's logs start at on a chain: its row's deployment block, or block 0 for a registry no row
/// names (every block is asked).
pub fn start_of(chain_id: u64, registry: &[u8; 20]) -> u64 {
    let hex = zikaron::hexfmt::encode(registry);
    ROWS.iter().find(|r| r.chain_id == chain_id && r.registry.eq_ignore_ascii_case(&hex)).map(|r| r.from_block).unwrap_or(0)
}
