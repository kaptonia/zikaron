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

Fork floor: the pinned build (evm_version cancun) emits PUSH0. On a chain
below Shanghai the pinned initcode still deploys, at this same codeHash,
and every call to the deployed code fails without a log; a codeHash match
therefore certifies a working deployment only on chains at or above
Shanghai, and a chain below that needs its own pinned build and its own
recorded codeHash. The law reads registries by the basis's declared
addresses and never by codeHash, so a second build is a deployment
convenience, not a grammar matter.

# ZikaronRevocation codeHash

Runtime bytecode length: 130 bytes.
codeHash (keccak256 of the runtime bytecode): `0x07a2fef228662aa2adc0a273a304ac4372898138ee1a8a8531760207988ddec2`

Settings that produce it: the same foundry.toml as the registry's (solc
0.8.28, evm_version cancun, optimizer on with 200 runs, via_ir true,
bytecode_hash none, cbor_metadata false). Any change to the source or to
these settings changes the codeHash; a reader that pins the contract by
codeHash is pinning exactly this.

Interface read by the laws: `revoke(bytes32 entry, bytes32 grant)`, selector
`0xc2664610`, emits in order `Anchored(address indexed author, bytes32
indexed hash)` with topic 0 `keccak256("Anchored(address,bytes32)")` =
`0xee1610244dfc4b116b1433dd4459fd7b1d3d2bf737a05fc3f6d8f173005d7147` (the
anchor of the revocation entry, zikaron/2 section 9.1, the word at calldata
offset 4) and `Revoked(address indexed author, bytes32 indexed grant)` with
topic 0 `keccak256("Revoked(address,bytes32)")` =
`0x6e70be4be1a4aebd688b5523bd8b6278acac3963d71ebf2bd5ea50757047664b` (the
notice of the grant's withdrawal, zikaron.kit/2 section 6.3, the word at
calldata offset 36). The contract holds no state and has no owner. The
registry above is not replaced: ordinary entries anchor there, and a reader
lists both contracts in its basis.

Fork floor: as the registry's. On a chain below Shanghai the pinned
initcode deploys at this same codeHash and every call fails without a log,
so a codeHash match certifies a working deployment only at or above
Shanghai.
