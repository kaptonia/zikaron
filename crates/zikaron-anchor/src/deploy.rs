pub const CODE_HASH: &str = "0xfa97a1d9b22fab2b52f4e27c9a965b32734c40001b565ab365d05c887118f57d";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub name: &'static str,
    pub chain_id: u64,
    pub registry: &'static str,
    pub from_block: u64,
    pub nodes: [&'static str; 2],
}

pub const ROWS: [Row; 4] = [
    Row { name: "mainnet", chain_id: 1, registry: "0x36Ea8A857a5FE813429d4D9947000C644A88809A", from_block: 26_087_229, nodes: ["https://mainnet.gateway.tenderly.co", "https://rpc.mevblocker.io"] },
    Row { name: "sepolia", chain_id: 11_155_111, registry: "0xC29410B882c4C3b77e33659d2f06ac563e7B08a3", from_block: 11_715_660, nodes: ["https://sepolia.gateway.tenderly.co", "https://rpc.sepolia.ethpandaops.io"] },
    Row { name: "arbitrum-one", chain_id: 42_161, registry: "0x36Ea8A857a5FE813429d4D9947000C644A88809A", from_block: 511_445_184, nodes: ["https://arb1.arbitrum.io/rpc", "https://arbitrum.gateway.tenderly.co"] },
    Row { name: "op-mainnet", chain_id: 10, registry: "0x36Ea8A857a5FE813429d4D9947000C644A88809A", from_block: 157_735_914, nodes: ["https://mainnet.optimism.io", "https://optimism.gateway.tenderly.co"] },
];

pub fn start_of(chain_id: u64, registry: &[u8; 20]) -> u64 {
    let hex = zikaron::hexfmt::encode(registry);
    ROWS.iter().find(|r| r.chain_id == chain_id && r.registry.eq_ignore_ascii_case(&hex)).map(|r| r.from_block).unwrap_or(0)
}
