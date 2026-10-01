# zikaron-core

The chain-facing references of the evidence ledger.

- `contracts/`: the ZikaronRegistry horn, pinned for a reproducible
  codeHash (CODEHASH.md), with its forge suite. The law reads the
  registry by the log shape of zikaron/1 section 9.1; the contract is
  the convenience that emits it.
- `fixtures/`: sixty-nine recordings (thirty-two scanned against anvil, twenty-seven needing no node, ten written by hand and signed with test keys) with expected outputs, the
  chain-facing corpus: a scanner replays each recording and must
  reproduce the expected fragment byte for byte. The independent Python
  scanner in `../zikaron-conformance/scan-py/` agrees on all of them.

The Rust crates that once lived under `crates/` (core, store, anchoring,
kit) were retired at commit 1ea0f11; the Desk builds its own from the
criteria, and these fixtures and the contract are what it builds against.
