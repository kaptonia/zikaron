# ZikaronRegistry test findings

No behavioural defect. Three notes from the forge suite (40 tests):

1. **Non-canonical `anchorMany` encoding.** solc does not require a
   calldata array's head offset to be a multiple of 32, so a caller can
   place an element at an offset such as 69. The event is emitted and the
   law's calldata test (section 9.1) declines the anchor. Safe direction:
   nobody is charged with a hash their bytes did not carry at a readable
   offset. The source comment now says so.
2. **Contract callers.** The event names `msg.sender`; a contract that
   calls the registry is charged as itself, and the law reads only logs
   whose topic 1 is the transaction's sender, so such a log is never an
   anchor. Design, and asserted in both halves.
3. **Harness limit.** `vm.recordLogs` keeps logs from reverted frames, so
   "a reverted call anchors nothing" is asserted through the receipt
   status and the section 9.1 predicate rather than through an empty log
   list.

Gas (warm): `anchor` 2352; `anchorMany` 2776 for one element, 17746 for
ten, 167661 for a hundred.

# ZikaronRevocation test findings

No behavioural defect. Two notes from the forge suite (31 tests):

1. **Two logs, one transaction.** `revoke(entry, grant)` emits the
   `Anchored` log and then the `Revoked` log. Each law reads its own
   topic 0, so the anchor is no notice and the notice no anchor, and the
   two words are both carried at aligned calldata offsets (4 and 36); the
   topic 0 and not the containment keeps the two readings apart. Asserted
   end to end against both form tests.
2. **Harness limit.** As for the registry, `vm.recordLogs` keeps logs
   from reverted frames, so "a reverted call anchors and notices nothing"
   is asserted through the receipt status and the two form predicates
   rather than through an empty log list.

Gas: `revoke` executes in 3,133 gas for every input (the intrinsic cost
and calldata apart, as the FUZZ and VULN seats of the 2026-11-09 round
measured); measured through a warm external call in the suite, where the
registry's `anchor` measures 23,848 on the same basis, it reads 25,907, the
difference one `LOG3` and one calldata word. The registry's figures above
were taken on another basis and are not comparable to these.
