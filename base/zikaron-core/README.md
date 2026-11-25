# zikaron-core

The chain-facing references of the evidence ledger.

- `contracts/`: the ZikaronRegistry horn and the ZikaronRevocation
  notice, each pinned for a reproducible codeHash (CODEHASH.md), with
  their forge suites. The laws read the registry by the log shape of
  zikaron/2 section 9.1 and the revocation contract by that shape for
  its first log and by the notice shape of zikaron.kit/2 section 6.3 for
  its second; the contracts are the conveniences that emit them. The
  registry's suite also witnesses what zikaron.face/1 section 5 leans on:
  `anchor` called with bytes after its word emits the log it emits without
  them.
- `fixtures/`: sixty-nine recordings (thirty-two scanned against anvil, twenty-seven needing no node, ten written by hand and signed with test keys) with expected outputs, the
  chain-facing corpus: a scanner replays each recording and must
  reproduce the expected fragment byte for byte. The independent Python
  scanner in `../zikaron-conformance/scan-py/` agrees on all of them.

The Rust crates that once lived under `crates/` (core, store, anchoring,
kit) were retired at commit 1ea0f11; the Desk builds its own from the
criteria, and these fixtures and the contract are what it builds against.
