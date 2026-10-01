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
