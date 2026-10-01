# ZikaronRegistry codeHash

Runtime bytecode length: 246 bytes.
codeHash (keccak256 of the runtime bytecode): `0xfa97a1d9b22fab2b52f4e27c9a965b32734c40001b565ab365d05c887118f57d`

Settings that produce it (foundry.toml): solc 0.8.28, evm_version cancun,
optimizer on with 200 runs, via_ir true, bytecode_hash none, cbor_metadata
false. Any change to the source or to these settings changes the codeHash;
a reader that pins a registry by codeHash is pinning exactly this.

Interface read by the law (zikaron/1 section 9.1): event
`Anchored(address indexed author, bytes32 indexed hash)`, topic 0
`keccak256("Anchored(address,bytes32)")`; functions `anchor(bytes32)` and
`anchorMany(bytes32[])`. The contract holds no state and has no owner.

Fork floor: the pinned build (evm_version cancun) emits PUSH0, so this
codeHash is deployable only on chains at or above Shanghai. A chain below
that needs its own pinned build and its own recorded codeHash; the law
reads registries by the basis's declared addresses and never by codeHash,
so a second build is a deployment convenience, not a grammar matter.
